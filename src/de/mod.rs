/*!
Deserialization for minified JSON objects.

This module contains a parser for previously validated minified JSON input.
It uses a _lot_ of unsafe code, but guarantees UB freedom through its public API.
What it doesn't guarantee is how useful the result will be when the input is not
a single valid JSON map with no whitespace.

The parser proceeds linearly, maintaining a stack and its current position
within the document. It isn't recursive.

There are two implementations:

- an AVX2 vectorized implementation,
- and a byte-by-byte fallback implementation.

Both use the same functions to track offsets in the document, the AVX2 implementation
is just able to skip over sequences of bytes that don't contain any interesting input.
For valid JSON documents, the two implementations will produce the same results, but
for invalid JSON documents their results may diverge.

We don't take special advantage of SIMD intrinsics to perform validation or transform input
in constant-time, which is something `simd_json` does heavily, just because it
would cause our fallback and vectorized implementations to diverge and they
need to be able to work together.
*/

#![allow(overflowing_literals)] // we do this on purpose

mod document;

mod fallback;
mod interest;
mod simd;

use std::{mem, str};

use interest::*;
use simd::Simd;

pub use document::*;

impl<'input> Document<'input> {
    /**
    Scan a JSON object byte buffer into an indexable document.

    # What does _trusted_ mean?

    The parser validates UTF8, but otherwise assumes the input has been previously
    validated as a minified JSON object. While this process doesn't guarantee the
    results it returns on invalid JSON, or when the input is not a document,
    it does guarantee UB freedom. That means any strings returned are valid
    UTF8 and any offsets within the parsed parts are guaranteed to be within the document.
    So the results of invalid input are still usable, even if they're going to be either empty
    or nonsense.

    # What does _valid_ mean?

    A buffer containing a single JSON object with no additional whitespace
    (besides a possible trailing newline) will be parsed as expected.
    Some invalid content may also parse, such as maps that are terminated
    by a `]` instead of a `}`, or invalid atoms like `nool` instead of `null`.

    # Panics

    This method does not panic. If parsing detected an error, then the document
    can still be used even if it's erroneous, but will probably be empty or partially complete.
    */
    #[inline]
    pub fn scan_trusted(input: &'input [u8]) -> Self {
        scan(input, DetachedDocument::default())
    }

    /**
    Scan a JSON byte buffer into an indexable document, re-using the allocations
    from a previous document.

    This method has the same guarantees as [`scan_trusted`].
    */
    #[inline]
    pub fn scan_trusted_attach(input: &'input [u8], detached: DetachedDocument) -> Self {
        scan(input, detached)
    }

    // used by tests and benches
    #[doc(hidden)]
    pub fn scan_trusted_fallback(input: &'input [u8]) -> Self {
        scan_fallback(input, DetachedDocument::default())
    }

    #[cold]
    fn err(input: &'input [u8]) -> Self {
        Document {
            input,
            offsets: Offsets {
                elements: Vec::new(),
                err: true,
                root_size_hint: 0,
            },
            _detached_stack: Vec::new(),
        }
    }

    /**
    Whether or not the parser encountered any invalid content.

    This method isn't necessarily going to return `true` for any invalid input.
    */
    #[inline]
    #[doc(hidden)]
    pub fn is_err(&self) -> bool {
        self.offsets.err
    }

    /**
    Detach the allocations from this document so that they can be reused for parsing other documents.
    */
    #[inline]
    pub fn detach(self) -> DetachedDocument {
        let mut offsets = self.offsets.elements;
        offsets.clear();

        let mut stack = self._detached_stack;
        stack.clear();

        DetachedDocument { offsets, stack }
    }

    /**
    Take the offsets from this document.
    */
    #[inline]
    pub fn into_offsets(self) -> Offsets {
        self.offsets
    }
}

/**
A previously parsed table of offsets.

The offsets can be cached and re-attached to an input buffer to avoid parsing again.
*/
#[derive(Debug, Clone)]
pub struct Offsets {
    elements: Vec<Offset>,
    err: bool,
    root_size_hint: u16,
}

/**
An allocation for offsets that's been detached from a document.

This allocation can be re-used by future documents. They don't need
to be from the same buffer.
*/
pub struct DetachedDocument {
    offsets: Vec<Offset>,
    stack: Vec<ActiveMapArr>,
}

impl Default for DetachedDocument {
    #[inline]
    fn default() -> Self {
        DetachedDocument {
            offsets: Vec::with_capacity(48),
            stack: Vec::with_capacity(6),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Offset {
    kind: OffsetKind,
    position: Part,
    next: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OffsetKind {
    Str(Slice, bool),
    Num(Slice),
    Bool(bool),
    Null,
    Map(u16),
    Arr(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Slice {
    offset: u32,
    len: u32,
}

/**
The position of an element within a document.
*/
// note: these fields cannot be changed without `PrevPartOffsets`
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum Part {
    None,
    Key,
    Value,
    Elem,
}

impl Default for Part {
    #[inline]
    fn default() -> Self {
        Part::None
    }
}

// note: this type must have the same number of fields as `Part` has variants
type PrevPartOffsets = [Option<u16>; 4];

impl Offsets {
    #[inline]
    fn attach(elements: Vec<Offset>) -> Self {
        Offsets {
            elements,
            err: false,
            root_size_hint: 0,
        }
    }

    /**
    Build a document from these offsets and an input buffer without validating
    that the input matches the offsets.

    # Safety

    This method is unsafe because the offsets may point to incorrect positions in
    the input if it is not exactly equal to the input that originally produced the offsets.
    */
    #[inline]
    pub unsafe fn to_document_unchecked<'a>(&self, input: &'a [u8]) -> Document<'a> {
        Document {
            input,
            offsets: self.clone(),
            _detached_stack: Vec::new(),
        }
    }

    #[inline]
    fn push(&mut self, part: Offset) {
        self.elements.push(part);
    }
}

#[inline]
#[cfg(not(wasm))]
fn scan(input: &[u8], detached: DetachedDocument) -> Document {
    let (start, end) = match scan_begin(input) {
        Some(bounds) => bounds,
        None => return Document::err(input),
    };

    let mut scan = Scan::attach(detached.stack, start, end);
    let mut offsets = Offsets::attach(detached.offsets);

    // when avx2 is available, we can vectorize
    // HEURISTIC: small documents aren't worth vectorizing
    if is_x86_feature_detected!("avx2") && scan.input_remaining() > Simd::VECTORIZATION_THRESHOLD {
        // SAFETY: the input is UTF8
        // SAFETY: avx2 is available
        unsafe { simd::scan(input, &mut scan, &mut offsets) };
        return scan_end(input, scan, offsets);
    }

    // when avx2 is not available, we need to fallback
    // SAFETY: the input is UTF8
    unsafe { fallback::scan(input, &mut scan, &mut offsets) };
    scan_end(input, scan, offsets)
}

#[cfg(wasm)]
use self::scan_fallback as scan;

#[inline]
fn scan_fallback(input: &[u8], detached: DetachedDocument) -> Document {
    let (start, end) = match scan_begin(input) {
        Some(bounds) => bounds,
        None => return Document::err(input),
    };

    let mut scan = Scan::attach(detached.stack, start, end);
    let mut offsets = Offsets::attach(detached.offsets);

    unsafe { fallback::scan(input, &mut scan, &mut offsets) };
    scan_end(input, scan, offsets)
}

/**
Validate the input is UTF8 and return the bounds to read within.

The input is expected to be a JSON object. The start and end tokens are omitted.
*/
#[inline]
fn scan_begin(input: &[u8]) -> Option<(isize, usize)> {
    // ensure the input is valid UTF8
    // we mostly scan through 7byte ASCII, but construct strings
    // from offsets within the document
    let input = match str::from_utf8(input) {
        Ok(input) => input.trim_end().as_bytes(),
        _ => return None,
    };

    if input.len() < 2 {
        return None;
    }

    // ensure the input is an object
    // doing this lets us guarantee that lookaheads will always be in bounds
    // because we never look past 1 char, and never lookahead on `}`

    if *get_unchecked!(input, 0) != b'{' {
        return None;
    }

    if *get_unchecked!(input, input.len() - 1) != b'}' {
        return None;
    }

    // ignore the leading and trailing object chars along with any trailing whitespace
    // by ignoring the outer map the parser can avoid an unnecessary item in the offsets,
    // since every document is expected to be a map.
    Some((1, input.len() - 1))
}

/**
Validate the produced output.

There may be some trailing unprocessed input to deal with because the object markers are ignored.
*/
#[inline]
fn scan_end(input: &[u8], mut scan: Scan, mut offsets: Offsets) -> Document {
    // ensure the input is complete
    match scan.stack.active_map_arr.active_primitive.kind {
        // if there's no start kind then we're finished
        ActivePrimitiveKind::None => (),

        // if there's a number then finish it
        // since we trim the leading and trailing `{` `}` characters there may be a trailing
        // number to finish
        ActivePrimitiveKind::Num => {
            let input_offset = scan.input_offset as usize;
            let curr = offset_deref_unchecked!(input, scan.input_offset);

            interest_num_end(ScanFnInput {
                curr_offset: input_offset,
                curr,
                input,
                scan: &mut scan,
                offsets: &mut offsets,
            });
        }

        // if there's a string then the input is truncated
        ActivePrimitiveKind::Str => {
            scan.error = true;
            test_unreachable!("unterminated string");
        }

        // if there's an atom then we're finished
        ActivePrimitiveKind::Atom => (),
    }

    // if the offsets count is greater than `u16::max_value` then we've overflowed
    if offsets.elements.len() > u16::max_value() as usize {
        scan.error = true;
        test_unreachable!("overflowed max offset size");
    }

    // set the root size hint for the document
    offsets.root_size_hint = scan.stack.active_map_arr.len >> 1;

    // only return a document if the parser didn't produce an error
    if !scan.error {
        Document {
            input,
            offsets,
            _detached_stack: scan.stack.bottom,
        }
    } else {
        Document::err(input)
    }
}

/**
The state of our JSON parser.

Some state is specific to the current map or array, and some state
is global to the scanner.
*/
#[derive(Debug)]
struct Scan {
    /**
    The current offset in the input.

    This may be behind the current character being processed in SIMD
    implementations that process the inputs in blocks.
    */
    input_offset: isize,
    /**
    The length of the input buffer to process.
    */
    input_len: usize,
    /**
    Whether or not the next character has been escaped.
    */
    escape: bool,
    /**
    Whether or not the parser has encountered an error.

    The parser doesn't expect to encounter errors so it doesn't check this field until the end.
    */
    error: bool,
    /**
    State specifically for the SIMD implementation.

    Even when the input isn't being processed using SIMD, its state needs to be kept consistent
    so that it can pick up after the fallback implementation.
    */
    simd: Simd,
    /**
    State for tracking the current depth within the input.

    The stack is pushed and popped whenever a map or array is encountered.
    */
    stack: Stack,
}

/**
The state of our JSON parser at a particular depth.

The depth is increased for each map or array.
*/
#[derive(Debug)]
struct Stack {
    active_map_arr: ActiveMapArr,
    bottom: Vec<ActiveMapArr>,
}

/**
An individual level in the stack that corresponds to a map or array.
*/
#[derive(Debug, Clone, Copy)]
struct ActiveMapArr {
    /**
    The start of a multi-byte offset.

    The offset may be escaped.
    */
    active_primitive: ActivePrimitive,
    /**
    The offset this map or array starts from.
    */
    start_from_offset: u16,
    /**
    The current number of offsets in this map or array.
    */
    len: u16,
    /**
    The index of possible parts for this map or array.

    The kind of part used is determined by `len`.
    */
    parts: [Part; 2],
    /**
    The previous parts in this map or array that need to be updated as new parts are pushed.
    */
    prev_part_offsets: PrevPartOffsets,
}

/**
A marker for the current multi-byte part.

The part may be a string, number or atom, as defined by its `StartKind`.
*/
#[derive(Debug, PartialEq, Clone, Copy)]
struct ActivePrimitive {
    /**
    The kind of primitive we're currently in.

    The part may be `None`.
    */
    kind: ActivePrimitiveKind,
    /**
    The offset the primitive begins at.
    */
    input_offset: usize,
    /**
    Whether or not the current primitive is escaped.

    This only makes sense for strings, but since strings that are escaped are likely
    to have many escapes we can avoid branches by always tracking this state.
    */
    escaped: bool,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ActivePrimitiveKind {
    None,
    Str,
    Num,
    Atom,
}

impl Default for ActivePrimitive {
    #[inline]
    fn default() -> Self {
        ActivePrimitive {
            input_offset: 0,
            kind: ActivePrimitiveKind::None,
            escaped: false,
        }
    }
}

impl ActivePrimitive {
    #[inline]
    fn take(&mut self) -> ActivePrimitive {
        mem::take(self)
    }
}

impl Scan {
    #[inline]
    fn attach(stack: Vec<ActiveMapArr>, start: isize, end: usize) -> Self {
        Scan {
            input_offset: start,
            input_len: end,
            escape: false,
            error: false,
            stack: Stack::attach(stack),
            simd: Simd::new(),
        }
    }

    #[inline]
    #[cfg(not(wasm))]
    fn input_remaining(&self) -> usize {
        self.input_len - (self.input_offset as usize)
    }
}

impl Stack {
    /**
    A cap on the maximum depth allowed in the document.

    It makes sure degenerate inputs like `[[[[[[[[[[[[[[[[[[[[[[[[[..`
    aren't potentials for OOM.
    */
    const MAX_DEPTH: usize = 96;

    #[inline]
    fn attach(bottom: Vec<ActiveMapArr>) -> Self {
        Stack {
            active_map_arr: ActiveMapArr {
                active_primitive: Default::default(),
                start_from_offset: 0,
                len: 0,
                parts: [Part::Key, Part::Value],
                prev_part_offsets: [None; 4],
            },
            bottom,
        }
    }
}
