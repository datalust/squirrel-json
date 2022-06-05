use std::{mem, ops::Index};

use super::*;

#[cfg(test)]
const MAX_BLOCK_SIZE: usize = 32;

trait ScanSimd {
    type Block: Sized + Clone + Copy;
    const BLOCK_SIZE: usize = mem::size_of::<Self::Block>();

    fn load_block_aligned(ptr: *const u8) -> Self::Block;
    fn mask_quote_escape(block: Self::Block) -> i32;
    fn mask_interest(block: Self::Block) -> i32;
}

#[cfg(target_arch = "x86_64")]
mod x86_64;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `avx2` is available
#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
pub(super) unsafe fn scan_x86_64_avx2<'scan>(
    input: &'scan [u8],
    scan: &mut Scan,
    offsets: &mut Offsets,
) {
    scan_simd::<x86_64::AVX2>(input, scan, offsets)
}

#[cfg(target_arch = "x86_64")]
pub(super) const X86_64_AVX2_VECTORIZATION_THRESHOLD: usize = x86_64::AVX2::BLOCK_SIZE * 5;

#[cfg(target_arch = "aarch64")]
mod aarch64;

// SAFETY: Callers must ensure `input` is valid UTF8
// SAFETY: Callers must ensure `neon` is available
#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "neon")]
pub(super) unsafe fn scan_aarch64_neon<'scan>(
    input: &'scan [u8],
    scan: &mut Scan,
    offsets: &mut Offsets,
) {
    scan_simd::<aarch64::Neon>(input, scan, offsets)
}

#[cfg(target_arch = "aarch64")]
pub(super) const AARCH64_NEON_VECTORIZATION_THRESHOLD: usize = aarch64::Neon::BLOCK_SIZE * 5;

// SAFETY: Callers must ensure `input` is valid UTF8
#[inline(always)]
unsafe fn scan_simd<'scan, V>(input: &'scan [u8], scan: &mut Scan, offsets: &mut Offsets)
where
    V: ScanSimd,
{
    test_assert!(V::BLOCK_SIZE <= MAX_BLOCK_SIZE);
    test_assert!(scan.input_remaining() > V::BLOCK_SIZE * 2);

    // HEURISTIC: we're probably going to be loading a lot of blocks, so it's worth aligning reads

    // check whether the start is aligned
    // on some targets, it's faster to do aligned loads of our blocks, so it's worth
    // scanning the leading unaligned portion first
    let aligned_start = input.as_ptr().offset(scan.input_offset) as usize % V::BLOCK_SIZE;

    if aligned_start != 0 {
        let read_to = ((scan.input_offset as usize + V::BLOCK_SIZE) - aligned_start) as isize;

        // scan the leading unaligned portion
        fallback::scan_to(input, scan, offsets, read_to);
    }

    // figure out the start of the last aligned block to read
    // these operations don't need to be
    let aligned_last_block_start = {
        let last_block_start = scan.input_remaining() - V::BLOCK_SIZE;
        let offset = last_block_start % V::BLOCK_SIZE;

        scan.input_len - (V::BLOCK_SIZE + offset)
    } as isize;

    'aligned: while scan.input_offset <= aligned_last_block_start {
        test_assert_eq!(
            0,
            input
                .as_ptr()
                .offset(scan.input_offset)
                .align_offset(V::BLOCK_SIZE),
            "the block alignment is incorrect"
        );

        test_assert!((scan.input_offset as usize) + V::BLOCK_SIZE <= scan.input_len);

        // we only cast at aligned offsets
        #[allow(clippy::cast_ptr_alignment)]
        let i = V::load_block_aligned(input.as_ptr().offset(scan.input_offset) as *const _);

        // first, find quotes and escapes in the input
        // we do this separately to optimize the case where
        // we're inside a big string and don't need to match for other structural chars
        let mask_quote = V::mask_quote_escape(i);

        // HEURISTIC: if there are no quotes or escapes and we're inside a big string then
        // there's no need to look for any other interest chars
        if mask_quote != 0 || scan.simd.active_mask == ActiveMask::Interest {
            // use a lookup table to classify characters in the input into groups
            // this is the same approach used by `simd-json`, which makes it possible
            // to identify a large number of characters in a multibyte buffer using only a few
            // instructions
            let mask_interest = V::mask_interest(i);

            test_assert_eq!(mask_interest, mask_quote | mask_interest);

            scan.set_masks(Masks {
                interest: mask_interest,
                quote: mask_quote,
            });

            'block: while scan.simd.masks.interest != 0 {
                // advance through the block by shifting over zeros in the mask
                // this is more efficient than looking at each byte individually
                let block_offset = scan.simd.masks[scan.simd.active_mask].trailing_zeros();
                test_assert!(block_offset < MAX_BLOCK_SIZE as u32);

                let shift = (!0i64 << (block_offset + 1)) as i32;

                scan.simd.masks.interest &= shift;
                scan.simd.masks.quote &= shift;

                let input_offset = scan.input_offset as usize + block_offset as usize;
                test_assert!(input_offset < scan.input_len as usize);

                let curr = *get_unchecked!(input, input_offset);

                match_interest(&mut ScanFnInput {
                    curr_offset: input_offset,
                    curr,
                    input,
                    scan,
                    offsets,
                });
            }
        }

        scan.input_offset += V::BLOCK_SIZE as isize;
    }

    test_assert!(scan.input_len - (scan.input_offset as usize) < V::BLOCK_SIZE);

    // finish the input using the fallback byte-by-byte scanning
    fallback::scan(input, scan, offsets);
}

impl Scan {
    #[inline(always)]
    fn set_masks(&mut self, masks: Masks) {
        self.simd.masks = masks;

        match self.simd.active_mask {
            ActiveMask::Interest => pre_mask_interest(&mut self.simd.masks),
            ActiveMask::Quote => pre_mask_quote(&mut self.simd.masks),
        }
    }
}

#[derive(Debug)]
pub(super) struct Simd {
    masks: Masks,
    active_mask: ActiveMask,
}

impl Simd {
    #[inline(always)]
    pub(super) fn new() -> Self {
        Simd {
            masks: Masks {
                interest: 0,
                quote: 0,
            },
            active_mask: ActiveMask::Interest,
        }
    }
}

impl Scan {
    #[inline(always)]
    pub(super) fn set_mask_quote(&mut self) {
        self.simd.active_mask = ActiveMask::Quote;
        pre_mask_quote(&mut self.simd.masks);
    }

    #[inline(always)]
    pub(super) fn shift_mask_quote(&mut self) {
        test_assert_eq!(ActiveMask::Quote, self.simd.active_mask);
        pre_mask_quote(&mut self.simd.masks);
    }

    #[inline(always)]
    pub(super) fn set_mask_interest(&mut self) {
        self.simd.active_mask = ActiveMask::Interest;
        pre_mask_interest(&mut self.simd.masks);
    }
}

#[repr(C)]
#[repr(align(4))]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Masks {
    // note: the order of these fields cannot be changed
    // they must match the set of variants in `ActiveMask`
    interest: i32,
    quote: i32,
}

// note: these fields cannot be changed without `Masks`
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(isize)]
enum ActiveMask {
    Interest,
    Quote,
}

impl Default for ActiveMask {
    #[inline(always)]
    fn default() -> Self {
        ActiveMask::Interest
    }
}

impl Index<ActiveMask> for Masks {
    type Output = i32;

    #[inline(always)]
    fn index(&self, id: ActiveMask) -> &i32 {
        // SAFETY: this is safe because the index is within the range of `Masks`
        unsafe { &*(self as *const Masks as *const i32).offset(id as isize) }
    }
}

// when the quote mask is active, unset all bits in the interest
// mask up to the next quote character
#[inline(always)]
fn pre_mask_quote(masks: &mut Masks) {
    let offset = masks.quote.trailing_zeros();

    // Exclude control characters up to the next quote or escape
    let shift = (!0i64 << offset) as i32;
    masks.interest &= shift;
}

// when the interest mask is active, there's no need to
// do anything
#[inline(always)]
fn pre_mask_interest(_: &mut Masks) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_mask_has_correct_repr() {
        assert_eq!(0isize, ActiveMask::Interest as isize);
        assert_eq!(1isize, ActiveMask::Quote as isize);
    }
}
