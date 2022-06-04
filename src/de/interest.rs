use std::{borrow::BorrowMut, fmt, mem};

use super::*;

pub(super) struct ScanFnInput<'a, 'scan> {
    /**
    The complete raw input buffer
    */
    pub(super) input: &'scan [u8],
    /**
    The offset of the current character in the input buffer.

    This isn't necessarily the same as the `input_offset` on `Scan`, which might
    be behind this one.
    */
    pub(super) curr_offset: usize,
    /**
    The current character to process.
    */
    pub(super) curr: u8,
    /**
    The parser state, including a stack and whether or not input is escaped.
    */
    pub(super) scan: &'a mut Scan,
    /**
    The offsets scanned out of the input so far.
    */
    pub(super) offsets: &'a mut Offsets,
}

impl<'a, 'scan> fmt::Debug for ScanFnInput<'a, 'scan> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ScanFnInput")
            .field("scan", &self.scan)
            .field("input_offset", &self.curr_offset)
            .field("curr", &(self.curr as char))
            .finish()
    }
}

impl<'a, 'scan> ScanFnInput<'a, 'scan> {
    /**
    Begin a map by pushing to the stack.
    */
    #[inline(always)]
    fn map_begin(&mut self) {
        self.begin(|start_from_offset| ActiveMapArr {
            active_primitive: Default::default(),
            start_from_offset,
            len: 0,
            parts: [Part::Key, Part::Value],
            prev_part_offsets: [None; 4],
        });
    }

    /**
    Begin an array by pushing to the stack.
    */
    #[inline(always)]
    fn arr_begin(&mut self) {
        self.begin(|start_from_offset| ActiveMapArr {
            active_primitive: Default::default(),
            start_from_offset,
            len: 0,
            parts: [Part::Elem, Part::Elem],
            prev_part_offsets: [None; 4],
        });
    }

    /**
    Complete a map, popping it from the stack.
    */
    #[inline(always)]
    fn map_end(&mut self) {
        self.end(|len| {
            // the map len is the number of entries
            // using `x >> 1` on a non-negative int is the same `floor(x / 2)`, but much faster
            // ignoring any mismatched pairs makes it safe to assume any map
            // with a non-zero length has at least one valid entry
            OffsetKind::Map(len >> 1)
        });
    }

    /**
    Complete an array, popping it from the stack.
    */
    #[inline(always)]
    fn arr_end(&mut self) {
        self.end(OffsetKind::Arr);
    }

    #[inline(always)]
    fn begin(&mut self, f: impl FnOnce(u16) -> ActiveMapArr) {
        // put a hard limit on the depth of the stack
        // since 1 byte of input can cause a 20+byte allocation
        // we don't want to get into any potential OOM situations
        if self.scan.stack.bottom.len() > Stack::MAX_DEPTH {
            self.err();
            return;
        }

        let start_from_offset = self.offsets.elements.len() as u16;

        self.scan.stack.bottom.push(mem::replace(
            &mut self.scan.stack.active_map_arr,
            f(start_from_offset),
        ));
    }

    #[inline(always)]
    fn end(&mut self, f: impl FnOnce(u16) -> OffsetKind) {
        if let Some(last) = self.scan.stack.bottom.pop() {
            let start = self.scan.stack.active_map_arr.start_from_offset as usize - 1;
            let len = self.scan.stack.active_map_arr.len;

            self.scan.stack.active_map_arr = last;

            // record whether or not the complex type contains any data
            get_unchecked_mut!(&mut self.offsets.elements, start).kind = f(len);
        } else {
            self.err();
        }
    }

    /**
    Poison the stack.

    Errors aren't returned immediately because they're not expected to ever happen so are checked
    at the end of the process.
    */
    #[cold]
    fn err(&mut self) {
        self.scan.error = true;
        self.scan.stack.active_map_arr.parts = [Part::None, Part::None];
        self.scan.stack.active_map_arr.prev_part_offsets = [None; 4];

        test_unreachable!("invalid stack operation");
    }

    /**
    Push a part onto the offsets.

    A previous part at the same position and depth will have its next offset updated to point
    to this new part.
    */
    #[inline(always)]
    fn push(&mut self, kind: OffsetKind) {
        let position_offset = self.offsets.elements.len() as u16;
        let (position, prev_position_offset) = self.scan.stack.active_map_arr.part(position_offset);

        if let Some(prev_position_offset) = prev_position_offset {
            let prev =
                get_unchecked_mut!(&mut self.offsets.elements, prev_position_offset as usize);
            test_assert_eq!(position, prev.position);

            prev.next = Some(position_offset);
        }

        self.offsets.push(Offset {
            kind,
            position,
            next: None,
        });
    }
}

impl ActiveMapArr {
    /**
    Get the position and offsets to update the next pointer in a previous part.
    */
    #[inline(always)]
    fn part(&mut self, curr_offset: u16) -> (Part, Option<u16>) {
        let curr_position = *get_unchecked!(self.parts, (self.len % 2) as usize);

        let prev_position_offset = mem::replace(
            get_unchecked_mut!(self.prev_part_offsets, curr_position as usize),
            Some(curr_offset),
        );

        self.len += 1;

        (curr_position, prev_position_offset)
    }
}

#[inline(always)]
pub(super) fn match_interest<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    match i.curr {
        b'"' => interest_str(i),
        b':' => interest_key_end(i),
        b',' => interest_value_elem_end(i),
        b'\\' => interest_escape(i),
        b'{' => interest_map_begin(i),
        b'[' => interest_arr_begin(i),
        b'}' => interest_map_end(i),
        b']' => interest_arr_end(i),
        _ => interest_unreachable(i),
    }
}

#[inline(always)]
pub(super) fn match_primitive<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    match i.curr {
        b'"' | b'{' | b'[' | b'}' | b']' => interest_none(i),
        b'0'..=b'9' | b'-' => interest_num_begin(i),
        b'n' => interest_null(i),
        b't' => interest_true(i),
        b'f' => interest_false(i),
        _ => interest_unreachable(i),
    }
}

#[inline(always)]
pub(super) fn interest_str<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    // if the string is escaped, then return
    if mem::take(&mut i.scan.escape) {
        interest_unescape_now(i);
        return;
    }

    match i.scan.stack.active_map_arr.active_primitive.take() {
        ActivePrimitive {
            input_offset: start,
            kind: ActivePrimitiveKind::Str,
            escaped,
        } => {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            {
                i.scan.set_mask_interest();
            }

            // ignore the trailing `"`
            let end = i.curr_offset;

            i.push(OffsetKind::Str(
                Slice {
                    offset: start as u32,
                    len: (end - start) as u32,
                },
                escaped,
            ));
        }
        _ => {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            {
                i.scan.set_mask_quote();
            }

            // skip over the leading `"`
            i.scan.stack.active_map_arr.active_primitive = ActivePrimitive {
                input_offset: i.curr_offset + 1,
                kind: ActivePrimitiveKind::Str,
                escaped: false,
            };
        }
    }
}

#[inline(always)]
pub(super) fn interest_escape<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    let escaped = i.scan.escape;
    i.scan.escape = !escaped;

    if escaped {
        // if the last character was a `\` then we've already cleared
        // the escape bit
        interest_unescape_now(i)
    } else {
        // peek the escape char
        i.curr_offset += 1;
        i.curr = *get_unchecked!(i.input, i.curr_offset);

        match i.curr {
            // `"` and `\` are interest chars and will be unescaped later
            b'"' | b'\\' => interest_unescape_later(i),
            // all other chars will be unescaped later
            // this includes technically invalid escape sequences
            _ => interest_unescape_now(i),
        }
    }
}

#[inline(always)]
pub(super) fn interest_unescape_now<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    // shift to the next quote or escape
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        i.scan.shift_mask_quote();
    }
    i.scan.stack.active_map_arr.active_primitive.escaped = true;
    i.scan.escape = false;
}

#[inline(always)]
pub(super) fn interest_unescape_later<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut _i: I) {
    let _i = _i.borrow_mut();

    test_assert_eq!(
        ActivePrimitiveKind::Str,
        _i.scan.stack.active_map_arr.active_primitive.kind
    );
}

#[inline(always)]
pub(super) fn interest_num_begin<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    i.scan.stack.active_map_arr.active_primitive = ActivePrimitive {
        input_offset: i.curr_offset,
        kind: ActivePrimitiveKind::Num,
        escaped: false,
    };
}

#[inline(always)]
pub(super) fn interest_num_end<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    if let ActivePrimitive {
        input_offset: start,
        kind: ActivePrimitiveKind::Num,
        ..
    } = i.scan.stack.active_map_arr.active_primitive.take()
    {
        // ignore the control character
        let end = i.curr_offset;

        i.push(OffsetKind::Num(Slice {
            offset: start as u32,
            len: (end - start) as u32,
        }));
    }
}

#[inline(always)]
pub(super) fn interest_null<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );
    i.scan.stack.active_map_arr.active_primitive.kind = ActivePrimitiveKind::Atom;

    i.push(OffsetKind::Null);
}

#[inline(always)]
pub(super) fn interest_true<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );
    i.scan.stack.active_map_arr.active_primitive.kind = ActivePrimitiveKind::Atom;

    i.push(OffsetKind::Bool(true));
}

#[inline(always)]
pub(super) fn interest_false<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );
    i.scan.stack.active_map_arr.active_primitive.kind = ActivePrimitiveKind::Atom;

    i.push(OffsetKind::Bool(false));
}

#[inline(always)]
pub(super) fn interest_map_begin<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    i.push(OffsetKind::Map(0));
    i.map_begin();
}

#[inline(always)]
pub(super) fn interest_arr_begin<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    i.push(OffsetKind::Arr(0));
    i.arr_begin();
    interest_key_elem_begin(i);
}

#[inline(always)]
pub(super) fn interest_key_elem_begin<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    // ignore the control character
    // this is just a peek, it doesn't update the offset
    i.curr_offset += 1;
    i.curr = *get_unchecked!(i.input, i.curr_offset);

    match_primitive(i);
}

#[inline(always)]
pub(super) fn interest_key_end<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    // ignore the control character
    i.curr_offset += 1;
    i.curr = *get_unchecked!(i.input, i.curr_offset);

    match_primitive(i);
}

#[inline(always)]
pub(super) fn interest_value_elem_end<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    interest_num_end(&mut *i);

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    // ignore the control character
    i.curr_offset += 1;
    i.curr = *get_unchecked!(i.input, i.curr_offset);

    match_primitive(i);
}

#[inline(always)]
pub(super) fn interest_map_end<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();
    interest_num_end(&mut *i);

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    i.map_end();
}

#[inline(always)]
pub(super) fn interest_arr_end<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();
    interest_num_end(&mut *i);

    test_assert_eq!(
        i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );

    i.arr_end();
}

#[inline(always)]
pub(super) fn interest_none<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut _i: I) {
    let _i = _i.borrow_mut();

    test_assert_eq!(
        _i.scan.stack.active_map_arr.active_primitive.kind,
        ActivePrimitiveKind::None
    );
}

#[cold]
pub(super) fn interest_unreachable<'a, 'scan, I: BorrowMut<ScanFnInput<'a, 'scan>>>(mut i: I) {
    let i = i.borrow_mut();

    i.scan.error = true;

    test_unreachable!(
        "unexpected {:?} at offset {:?}",
        i.curr as char,
        i.curr_offset
    );
}
