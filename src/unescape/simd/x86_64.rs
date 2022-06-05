use super::*;

use std::arch::x86_64::*;

pub(super) struct AVX2;
impl UnescapeSimd for AVX2 {
    type Block = __m256i;

    #[inline(always)]
    fn load_block_unaligned(ptr: *const u8) -> Self::Block {
        unsafe { _mm256_loadu_si256(ptr as *const _) }
    }

    #[inline(always)]
    fn mask_escape(block: Self::Block) -> i32 {
        unsafe {
            let match_escape = _mm256_cmpeq_epi8(block, _mm256_set1_epi8(b'\\' as i8));
            _mm256_movemask_epi8(match_escape)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_offset_is_32_bytes() {
        assert_eq!(32, AVX2::BLOCK_SIZE);
    }
}
