use super::*;

use crate::std_ext::arch::aarch64::*;
use std::arch::aarch64::*;

pub(super) struct Neon;
impl ScanSimd for Neon {
    type Block = uint8x8_t;

    #[inline(always)]
    fn load_block_aligned(ptr: *const u8) -> Self::Block {
        // SAFETY: In this module, Neon is always available
        unsafe { vld1_u8(ptr) }
    }

    #[inline(always)]
    fn mask_quote_escape(block: Self::Block) -> i32 {
        // SAFETY: In this module, Neon is always available
        unsafe {
            let mask_quote = vceq_u8(
                block,
                splat([b'"', b'"', b'"', b'"', b'"', b'"', b'"', b'"']),
            );

            let mask_escape = vceq_u8(
                block,
                splat([b'\\', b'\\', b'\\', b'\\', b'\\', b'\\', b'\\', b'\\']),
            );

            let mask = vorr_u8(mask_quote, mask_escape);

            vmovemask_u8(mask) as i32
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
            const C: u8 = 0b0000_0001; // `:`
            const B: u8 = 0b0000_0010; // `{` | `}` | `[` | `]`
            const N: u8 = 0b0000_0100; // `,`
            const E: u8 = 0b0000_1000; // `\`
            const Q: u8 = 0b0001_0000; // `"`
            const U: u8 = 0b0000_0000; // no match

            // the characters we want to match need to be put into groups
            // where each group corresponds to a set bit in our byte
            // that means in 8 bytes we have 8 possible groups
            // each group must contain a complete set of chars that match
            // the hi and lo nibbles, otherwise there could be false positives
            #[rustfmt::skip]
                let interest_hi = uint8x8x4_t(
                splat([U,U,Q|N,C,U,E|B,U,B]),
                splat([U,U,U,U,U,U,U,U]),
                splat([U,U,U,U,U,U,U,U]),
                splat([U,U,U,U,U,U,U,U]),
            );

            // once we have groups of characters to classify, each group
            // is set for the indexes below where a character in that group
            // has a hi or lo nibble
            // for example, the character `:` is in group `C` and has the nibbles `0x3a`
            // so the byte in the lo table at index `a` (10 and 26) are set to `C` and
            // the byte in the hi table at index `3` (3 and 19) are set to `C`
            #[rustfmt::skip]
                let interest_lo = uint8x8x4_t(
                splat([U,U,Q,U,U,U,U,U]),
                splat([U,U,C,B,N|E,B,U,U]),
                splat([U,U,U,U,U,U,U,U]),
                splat([U,U,U,U,U,U,U,U]),
            );

            // Categorize the low nibble of each input byte
            let lo = vtbl4_u8(
                interest_lo,
                vand_u8(
                    block,
                    splat([0x0f, 0x0f, 0x0f, 0x0f, 0x0f, 0x0f, 0x0f, 0x0f]),
                ),
            );

            // Categorize the high nibble of each input byte
            let hi = vtbl4_u8(interest_hi, vshr_n_u8(block, 4));

            // Combine the lo and hi masks to fully identify each byte
            let interest_hi_lo = vmvn_u8(vceq_u8(
                vand_u8(lo, hi),
                splat([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            ));

            // Pack the vector mask into a bitmask
            vmovemask_u8(interest_hi_lo) as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_offset_is_8_bytes() {
        assert_eq!(8, Neon::BLOCK_SIZE);
    }
}
