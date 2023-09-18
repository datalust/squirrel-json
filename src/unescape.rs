/*!
String unescaping for JSON documents.

This module contains a vectorized implementation for unescaping JSON strings.

It's not a general-purpose implementation, it requires strings come from a previously parsed
JSON document.

This implementation follows the same basic design as `de` for supporting a vectorized and
fallback implementation using a shared set of functions. It's docs have some more details.
*/

use std::{borrow::BorrowMut, ptr, str};

mod fallback;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
mod simd;

// SAFETY: The string must not end with a `\` unless it's been escaped
// This is guaranteed for strings parsed from JSON, because string boundaries
// with a leading `\` are considered escapes and won't terminate the string
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(crate) unsafe fn unescape_trusted(input: &str) -> String {
    let input = input.as_bytes();

    let mut scan = Scan {
        input_offset: 0,
        escape: false,
        start: 0,
        first_surrogate: None,
    };

    let mut unescaped = Unescaped {
        buf: Vec::with_capacity(input.len()),
    };

    // when SIMD is available, we can vectorize
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2")
            && input.len() > simd::X86_64_AVX2_VECTORIZATION_THRESHOLD
        {
            // SAFETY: the input is UTF8
            // SAFETY: avx2 is available
            simd::unescape_x86_64_avx2(input, &mut scan, &mut unescaped);
            return unescape_end(input, scan, unescaped);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon")
            && input.len() > simd::AARCH64_NEON_VECTORIZATION_THRESHOLD
        {
            // SAFETY: the input is UTF8
            // SAFETY: neon is available
            simd::unescape_aarch64_neon(input, &mut scan, &mut unescaped);
            return unescape_end(input, scan, unescaped);
        }
    }

    // when avx2 is not available, we need to fallback
    // SAFETY: the input is UTF8
    fallback::unescape(input, &mut scan, &mut unescaped);
    unescape_end(input, scan, unescaped)
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub(crate) unsafe fn unescape_trusted(input: &str) -> String {
    let input = input.as_bytes();

    let mut scan = Scan {
        input_offset: 0,
        escape: false,
        start: 0,
        first_surrogate: None,
    };

    let mut unescaped = Unescaped {
        buf: Vec::with_capacity(input.len()),
    };

    // SAFETY: the input is UTF8
    fallback::unescape(input, &mut scan, &mut unescaped);
    unescape_end(input, scan, unescaped)
}

#[inline]
fn unescape_end(input: &[u8], mut scan: Scan, mut unescaped: Unescaped) -> String {
    flush(input, input.len(), &mut scan, &mut unescaped);

    owned_from_utf8_unchecked!(unescaped.buf)
}

struct Scan {
    /**
    The current byte offset into the input.
    */
    input_offset: isize,
    /**
    The position to start copying from.
    */
    start: isize,
    /**
    Whether or not the current character is escaped.
    */
    escape: bool,
    /**
    A previously parsed `\u` escape that should be a surrogate pair.
    */
    first_surrogate: Option<u16>,
}

struct Unescaped {
    buf: Vec<u8>,
}

struct ScanFnInput<'a> {
    input: &'a [u8],
    curr_offset: usize,
    scan: &'a mut Scan,
    unescaped: &'a mut Unescaped,
}

#[inline(always)]
fn flush(input: &[u8], flush_to: usize, scan: &mut Scan, unescaped: &mut Unescaped) {
    // if a string starts with an escape then we'll try flush 0 bytes
    if flush_to == scan.start as usize {
        return;
    }

    let cnt = flush_to - scan.start as usize;

    test_assert!(cnt > 0);
    test_assert!(unescaped.buf.len() + cnt <= unescaped.buf.capacity());

    // manually copy into the vec, knowing the slices don't overlap
    // this is more efficient than `extend_from_slice` and friends,
    // because those methods can't guarantee there's no overlapping

    // SAFETY: The `src` and `cnt` slice is within `input`,
    // and the `dst` and `cnt` slice is within `buf`'s capacity.
    // SAFETY: We're only copying bytes, that are `Copy`.
    unsafe {
        let src = input.as_ptr().offset(scan.start);
        let dst = unescaped.buf.as_mut_ptr().add(unescaped.buf.len());

        ptr::copy_nonoverlapping(src, dst, cnt);
        unescaped.buf.set_len(unescaped.buf.len() + cnt);
    }

    scan.start = flush_to as isize;
}

impl<'a> ScanFnInput<'a> {
    #[inline(always)]
    fn flush(&mut self) {
        flush(self.input, self.curr_offset, self.scan, self.unescaped);

        // skip over the `\`
        self.scan.start += 1;
    }

    #[inline(always)]
    fn push_unescaped_byte(&mut self, b: u8) {
        self.unescaped.buf.push(b);

        // skip over the escape char
        self.scan.start += 1;
    }

    #[inline]
    fn push_unescaped_char(&mut self, c: char) {
        let mut buf = [0; 4];

        let encoded = c.encode_utf8(&mut buf);
        self.unescaped.buf.extend(encoded.as_bytes());

        // skip over the escape chars
        self.scan.start += 4;
    }

    #[inline]
    fn begin_surrogate_pair(&mut self, first: u16) {
        self.scan.first_surrogate = Some(first);

        // skip over the escape chars
        self.scan.start += 4;
    }
}

#[inline(always)]
fn interest_unescape<'a, I: BorrowMut<ScanFnInput<'a>>>(mut i: I) {
    let i = i.borrow_mut();

    let escaped = i.scan.escape;
    i.scan.escape = !escaped;

    if escaped {
        // if the last character was a `\` then we've already cleared
        // the escape bit, all that needs to be done is for a `\` to be pushed
        i.push_unescaped_byte(b'\\');
    } else {
        i.flush();

        // peek the escape char
        i.curr_offset += 1;
        let escaped = *get_unchecked!(i.input, i.curr_offset);

        match escaped {
            b'n' => i.push_unescaped_byte(b'\n'),
            b'"' => i.push_unescaped_byte(b'"'),
            b'\\' => return, // `\` will be unescaped later
            b'r' => i.push_unescaped_byte(b'\r'),
            b't' => i.push_unescaped_byte(b'\t'),
            b'f' => i.push_unescaped_byte(0x0c),
            b'b' => i.push_unescaped_byte(0x08),
            b'u' => {
                // skip over the escape char
                i.scan.start += 1;
                i.curr_offset += 1;

                // we have at least 4 bytes left for an escape code
                if i.input
                    .len()
                    .checked_sub(4usize)
                    .map(|start| i.curr_offset <= start)
                    .unwrap_or(false)
                {
                    let mut unescape = || {
                        let digits = str::from_utf8(offset_from_raw_parts!(
                            i.input.as_ptr(),
                            i.input.len(),
                            i.curr_offset,
                            4
                        ))
                        .map_err(|_| ())?;
                        let code = u16::from_str_radix(digits, 16).map_err(|_| ())?;

                        // if we get this far then we're looking at a hex number
                        // we guarantee there are no `\` in the 4 bytes we've just looked through
                        // NOTE: only attempting to match the surrogate here means we'll accept `\u`
                        // escapes with other characters between them, but still guarantee valid UTF8
                        match i.scan.first_surrogate.take() {
                            // if we had a surrogate pair, then attempt to map it to a multibyte
                            Some(first) => {
                                let ch = crate::std_ext::char::try_from_utf16_surrogate_pair(
                                    first, code,
                                )
                                .map_err(|_| ())?;
                                i.push_unescaped_char(ch);
                            }
                            // if we didn't have a surrogate pair,
                            // then attempt to interpret the code as a 2-4 byte character
                            None => match char::try_from(code as u32) {
                                Ok(ch) => i.push_unescaped_char(ch),
                                Err(_) => i.begin_surrogate_pair(code),
                            },
                        }

                        Ok::<(), ()>(())
                    };

                    let _ = unescape();
                }
            }
            // fallback case
            // we don't expect invalid escapes to reach here,
            // so if something does then we just ignore the `\`
            // the bytes following the unescaped `\` are valid UTF8
            // so we'll append them to the string later
            _ => (),
        }

        i.scan.escape = false;
    }
}
