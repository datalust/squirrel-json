use std::mem;

use super::*;

#[cfg(test)]
const MAX_BLOCK_SIZE: usize = 32;

trait UnescapeSimd {
    type Block: Sized + Clone + Copy;
    const BLOCK_SIZE: usize = mem::size_of::<Self::Block>();

    fn load_block_unaligned(ptr: *const u8) -> Self::Block;
    fn mask_escape(block: Self::Block) -> i32;
}

#[cfg(target_arch = "x86_64")]
mod x86_64;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `avx2` is available
#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
pub(super) unsafe fn unescape_x86_64_avx2(
    input: &[u8],
    scan: &mut Scan,
    unescaped: &mut Unescaped,
) {
    unescape_simd::<x86_64::AVX2>(input, scan, unescaped)
}

#[cfg(target_arch = "x86_64")]
pub(super) const X86_64_AVX2_VECTORIZATION_THRESHOLD: usize = x86_64::AVX2::BLOCK_SIZE;

#[cfg(target_arch = "aarch64")]
mod aarch64;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `neon` is available
#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "neon")]
pub(super) unsafe fn unescape_aarch64_neon(
    input: &[u8],
    scan: &mut Scan,
    unescaped: &mut Unescaped,
) {
    unescape_simd::<aarch64::Neon>(input, scan, unescaped)
}

#[cfg(target_arch = "aarch64")]
pub(super) const AARCH64_NEON_VECTORIZATION_THRESHOLD: usize = aarch64::Neon::BLOCK_SIZE;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `input` does not end with an unescaped `\`
#[inline]
unsafe fn unescape_simd<V>(input: &[u8], scan: &mut Scan, unescaped: &mut Unescaped)
where
    V: UnescapeSimd,
{
    test_assert!(V::BLOCK_SIZE <= MAX_BLOCK_SIZE);
    test_assert!(input.len() >= V::BLOCK_SIZE);

    // HEURISTIC: we're probably not going to be loading a lot of blocks, so we just do unaligned loads

    let last_block_start = (input.len() - V::BLOCK_SIZE) as isize;

    'unaligned: while scan.input_offset <= last_block_start {
        test_assert!((scan.input_offset as usize) + V::BLOCK_SIZE <= input.len());

        // we explicitly perform an unaligned load
        let i = V::load_block_unaligned(input.as_ptr().offset(scan.input_offset));

        // find escapes in the input
        let mut mask_escape = V::mask_escape(i);

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

        scan.input_offset += V::BLOCK_SIZE as isize;
    }

    test_assert!(input.len() - (scan.input_offset as usize) < MAX_BLOCK_SIZE);

    // finish the input using the fallback byte-by-byte scanning
    fallback::unescape(input, scan, unescaped);
}
