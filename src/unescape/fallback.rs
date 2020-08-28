use super::*;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `input` does not end with an unescaped `\`
#[inline(always)]
pub(super) unsafe fn unescape(input: &[u8], scan: &mut Scan, unescaped: &mut Unescaped) {
    let read_to = input.len() as isize;
    unescape_block(ScanBlockInput {
        input,
        scan,
        unescaped,
        read_to,
    });
}

#[inline(always)]
fn unescape_block(i: ScanBlockInput) {
    'interest: while i.scan.input_offset < i.read_to {
        let curr_offset = i.scan.input_offset as usize;
        let curr = offset_deref_unchecked!(i.input, i.scan.input_offset);

        if let b'\\' = curr {
            interest_unescape(ScanFnInput {
                curr_offset,
                input: i.input,
                scan: i.scan,
                unescaped: i.unescaped,
            });
        }

        i.scan.input_offset += 1;
    }

    test_assert_eq!(i.read_to, i.scan.input_offset);
}

struct ScanBlockInput<'a> {
    input: &'a [u8],
    scan: &'a mut Scan,
    unescaped: &'a mut Unescaped,
    read_to: isize,
}
