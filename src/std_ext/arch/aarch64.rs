#[cfg(any(target_feature = "neon", target_feature = "crc"))]
use std::{
    arch::aarch64::*,
};

#[target_feature(enable = "neon")]
#[inline]
// SAFETY: Callers must ensure Neon is available
pub unsafe fn splat(v: [u8; 8]) -> uint8x8_t {
    // Transmuting an array into a `uint8x8_t` is not a valid operation
    // The alignment of an array is less strict
    vld1_u8(v.as_ptr())
}

// Neon doesn't have a built-in equivalent to x86's movemask
// We implement our own by masking each lane to a single bit in the target `u8`
// We then add those bytes across the vector to combine them, producing a single
// value that contains a set bit corresponding to each `ff` value in the original
#[target_feature(enable = "neon")]
#[inline]
// SAFETY: Callers must ensure Neon is available
pub unsafe fn vmovemask_u8(a: uint8x8_t) -> u8 {
    vaddv_u8(vand_u8(
        a,
        splat([
            0b0000_0001,
            0b0000_0010,
            0b0000_0100,
            0b0000_1000,
            0b0001_0000,
            0b0010_0000,
            0b0100_0000,
            0b1000_0000,
        ])
    ))
}
