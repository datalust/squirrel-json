use super::*;

use std::arch::x86_64::*;

pub(super) struct AVX2;
impl ScanSimd for AVX2 {
    type Block = __m256i;

    #[inline(always)]
    fn load_block_aligned(ptr: *const u8) -> Self::Block {
        unsafe { _mm256_load_si256(ptr as *const _) }
    }

    #[inline(always)]
    fn mask_quote_escape(block: Self::Block) -> i32 {
        unsafe {
            let match_quote = _mm256_cmpeq_epi8(block, _mm256_set1_epi8(b'"' as i8));
            let mask_quote = _mm256_movemask_epi8(match_quote);

            let match_escape = _mm256_cmpeq_epi8(block, _mm256_set1_epi8(b'\\' as i8));
            let mask_escape = _mm256_movemask_epi8(match_escape);

            mask_quote | mask_escape
        }
    }

    #[inline(always)]
    fn mask_interest(block: Self::Block) -> i32 {
        unsafe {
            // the characters we want to match need to be put into groups
            // where each group corresponds to a set bit in our byte
            // that means in 8 bytes we have 8 possible groups
            // each group must contain a complete set of chars that match
            // the hi and lo nibbles, otherwise there could be false positives
            const C: i8 = 0b0000_0001; // `:`
            const B: i8 = 0b0000_0010; // `{` | `}` | `[` | `]`
            const N: i8 = 0b0000_0100; // `,`
            const E: i8 = 0b0000_1000; // `\`
            const Q: i8 = 0b0001_0000; // `"`
            const U: i8 = 0b0000_0000; // no match

            // once we have groups of characters to classify, each group
            // is set for the indexes below where a character in that group
            // has a hi or lo nibble
            // for example, the character `:` is in group `C` and has the nibbles `0x3a`
            // so the byte in the lo table at index `a` (10 and 26) are set to `C` and
            // the byte in the hi table at index `3` (3 and 19) are set to `C`
            #[rustfmt::skip]
                let interest_lo = {
                _mm256_setr_epi8(
                    U,U,Q,U,U,U,U,U,U,U,C,B,N|E,B,U,U,
                    U,U,Q,U,U,U,U,U,U,U,C,B,N|E,B,U,U,
                )
            };

            #[rustfmt::skip]
                let interest_hi = {
                _mm256_setr_epi8(
                    U,U,N|Q,C,U,B|E,U,B,U,U,U,U,U,U,U,U,
                    U,U,N|Q,C,U,B|E,U,B,U,U,U,U,U,U,U,U,
                )
            };

            // Categorize the low nibble of each input byte
            let lo = block;
            let match_interest_lo = _mm256_shuffle_epi8(interest_lo, lo);

            // Categorize the high nibble of each input byte
            let hi = _mm256_and_si256(_mm256_srli_epi32(block, 4), _mm256_set1_epi8(0x7f));
            let match_interest_hi = _mm256_shuffle_epi8(interest_hi, hi);

            // Combine the lo and hi masks to fully identify each byte
            let interest_hi_lo = _mm256_and_si256(match_interest_lo, match_interest_hi);

            // Pack the vector mask into a bitmask
            let match_interest = _mm256_cmpeq_epi8(interest_hi_lo, _mm256_set1_epi8(0));
            let mask_interest = _mm256_movemask_epi8(match_interest);

            !mask_interest
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
