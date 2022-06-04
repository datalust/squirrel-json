use super::*;

use crate::std_ext::arch::aarch64::*;
use std::arch::aarch64::*;

pub(super) struct Neon;
impl UnescapeSimd for Neon {
    type Block = uint8x8_t;

    #[inline(always)]
    fn load_block_unaligned(ptr: *const u8) -> Self::Block {
        // SAFETY: In this module, Neon is always available
        unsafe { vld1_u8(ptr) }
    }

    #[inline(always)]
    fn mask_escape(block: Self::Block) -> i32 {
        // SAFETY: In this module, Neon is always available
        unsafe {
            let mask = vceq_u8(
                block,
                splat([b'\\', b'\\', b'\\', b'\\', b'\\', b'\\', b'\\', b'\\']),
            );

            vmovemask_u8(mask) as i32
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
