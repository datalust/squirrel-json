#[cfg(not(wasm))]
mod x86_64 {
    use std::{arch::x86_64::*, mem};

    use crate::unescape::*;

    // SAFETY: Callers must ensure `input` is valid UTF8
    // SAFETY: Callers must ensure `input` does not end with an unescaped `\`
    // SAFETY: Callers must ensure AVX2 is available
    #[inline]
    #[target_feature(enable = "avx2")]
    pub(in crate::unescape) unsafe fn unescape(
        input: &[u8],
        scan: &mut Scan,
        unescaped: &mut Unescaped,
    ) {
        test_assert!(input.len() >= BLOCK_SIZE);

        // HEURISTIC: we're probably not going to be loading a lot of blocks, so we just do unaligned loads

        let last_block_start = (input.len() - BLOCK_SIZE) as isize;

        'aligned: while scan.input_offset <= last_block_start {
            test_assert!((scan.input_offset as usize) + BLOCK_SIZE <= input.len());

            // we explicitly perform an unaligned load
            let i = _mm256_loadu_si256(
                #[allow(clippy::cast_ptr_alignment)]
                {
                    input.as_ptr().offset(scan.input_offset) as *const _
                },
            );

            // find escapes in the input
            let mut mask_escape = {
                let match_escape = _mm256_cmpeq_epi8(i, _mm256_set1_epi8(b'\\' as i8));
                _mm256_movemask_epi8(match_escape)
            };

            'block: while mask_escape != 0 {
                // advance through the block by shifting over zeros in the mask
                // this is more efficient than looking at each byte individually
                let block_offset = mask_escape.trailing_zeros();
                test_assert!(block_offset < 32);

                let shift = (!0i64 << (block_offset + 1)) as i32;
                mask_escape &= shift;

                let curr_offset = scan.input_offset as usize + block_offset as usize;
                test_assert!(curr_offset < input.len() as usize);

                interest_unescape(&mut ScanFnInput {
                    curr_offset,
                    input,
                    scan,
                    unescaped,
                });
            }

            scan.input_offset += BLOCK_SIZE as isize;
        }

        test_assert!(input.len() - (scan.input_offset as usize) < 32);

        // finish the input using the fallback byte-by-byte scanning
        fallback::unescape(input, scan, unescaped);
    }

    /**
    The size and alignment of a block to read.
    */
    pub(in crate::unescape) const BLOCK_SIZE: usize = mem::align_of::<__m256i>();
}

#[cfg(not(wasm))]
pub(super) use x86_64::{unescape, BLOCK_SIZE};
