use std::ops::Index;

use super::*;

#[cfg(not(wasm))]
mod x86_64 {
    use std::{arch::x86_64::*, mem};

    use super::*;

    // SAFETY: Callers must ensure `input` is valid UTF8
    // SAFETY: Callers must ensure AVX2 is available
    #[inline]
    #[target_feature(enable = "avx2")]
    pub(in crate::de) unsafe fn scan<'scan>(
        input: &'scan [u8],
        scan: &mut Scan,
        offsets: &mut Offsets,
    ) {
        test_assert!(scan.input_remaining() > Simd::BLOCK_SIZE * 2);

        // HEURISTIC: we're probably going to be loading a lot of blocks, so it's worth aligning reads

        // check whether the start is aligned
        // it's faster to do aligned loads of our 32byte blocks, so it's worth
        // scanning the leading unaligned portion first
        let aligned_start = input.as_ptr().offset(scan.input_offset) as usize % 32;

        if aligned_start != 0 {
            let read_to =
                ((scan.input_offset as usize + Simd::BLOCK_SIZE) - aligned_start) as isize;

            // scan the leading unaligned portion
            fallback::scan_to(input, scan, offsets, read_to);
        }

        // figure out the start of the last aligned block to read
        // these operations don't need to be
        let aligned_last_block_start = {
            let last_block_start = scan.input_remaining() - Simd::BLOCK_SIZE;
            let offset = last_block_start % Simd::BLOCK_SIZE;

            scan.input_len - (Simd::BLOCK_SIZE + offset)
        } as isize;

        let interest = interest_lo_hi();

        'aligned: while scan.input_offset <= aligned_last_block_start {
            test_assert_eq!(
                0,
                input
                    .as_ptr()
                    .offset(scan.input_offset)
                    .align_offset(Simd::BLOCK_SIZE),
                "the block alignment is incorrect"
            );

            test_assert!((scan.input_offset as usize) + Simd::BLOCK_SIZE <= scan.input_len);

            // we only cast at aligned offsets
            let i = _mm256_load_si256(
                #[allow(clippy::cast_ptr_alignment)]
                {
                    input.as_ptr().offset(scan.input_offset) as *const _
                },
            );

            // first, find quotes and escapes in the input
            // we do this separately to optimize the case where
            // we're inside a big string and don't need to match for other structural chars
            let index_quote = {
                let match_quote = _mm256_cmpeq_epi8(i, _mm256_set1_epi8(b'"' as i8));
                let index_quote = _mm256_movemask_epi8(match_quote);

                let match_escape = _mm256_cmpeq_epi8(i, _mm256_set1_epi8(b'\\' as i8));
                let index_escape = _mm256_movemask_epi8(match_escape);

                index_quote | index_escape
            };

            // HEURISTIC: if there are no quotes or escapes and we're inside a big string then
            // there's no need to look for any other interest chars
            if index_quote != 0 || scan.simd.active_index == ActiveIndex::Interest {
                // use a lookup table to classify characters in the input into groups
                // this is the same approach used by `simd-json`, which makes it possible
                // to identify a large number of characters in a 32byte buffer using only a few
                // instructions
                let index_interest = {
                    let lo = i;
                    let match_interest_lo = _mm256_shuffle_epi8(interest.lo, lo);

                    let hi = _mm256_and_si256(_mm256_srli_epi32(i, 4), _mm256_set1_epi8(0x7f));
                    let match_interest_hi = _mm256_shuffle_epi8(interest.hi, hi);

                    let interest_hi_lo = _mm256_and_si256(match_interest_lo, match_interest_hi);

                    let match_interest = _mm256_cmpeq_epi8(interest_hi_lo, _mm256_set1_epi8(0));
                    let index_interest = _mm256_movemask_epi8(match_interest);

                    !index_interest
                };

                test_assert_eq!(index_interest, index_quote | index_interest);

                scan.set_indexes(Indexes {
                    interest: index_interest,
                    quote: index_quote,
                });

                'block: while scan.simd.indexes.interest != 0 {
                    // advance through the block by shifting over zeros in the mask
                    // this is more efficient than looking at each byte individually
                    let block_offset = scan.simd.indexes[scan.simd.active_index].trailing_zeros();
                    test_assert!(block_offset < 32);

                    let shift = (!0i64 << (block_offset + 1)) as i32;

                    scan.simd.indexes.interest &= shift;
                    scan.simd.indexes.quote &= shift;

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

            scan.input_offset += Simd::BLOCK_SIZE as isize;
        }

        test_assert!(scan.input_len - (scan.input_offset as usize) < 32);

        // finish the input using the fallback byte-by-byte scanning
        fallback::scan(input, scan, offsets);
    }

    impl Simd {
        /**
        The size and alignment of a block to read.
        */
        pub(in crate::de) const BLOCK_SIZE: usize = mem::align_of::<__m256i>();

        /**
        A heuristic threshold for the number of bytes in a document
        before considering vectorization.

        For very small inputs, there's less work in just scanning through
        its bytes than attempting to align and scan it in blocks.
        */
        pub(in crate::de) const VECTORIZATION_THRESHOLD: usize = Self::BLOCK_SIZE * 5;
    }

    impl Scan {
        #[inline(always)]
        fn set_indexes(&mut self, masks: Indexes) {
            self.simd.indexes = masks;

            match self.simd.active_index {
                ActiveIndex::Interest => pre_index_interest(&mut self.simd.indexes),
                ActiveIndex::Quote => pre_index_quote(&mut self.simd.indexes),
            }
        }
    }

    // Tables that match specific characters based on the lo and hi nibbles
    // of an ASCII byte
    struct InterestLoHiMask {
        lo: __m256i,
        hi: __m256i,
    }

    // SAFETY: Callers must ensure AVX2 is available
    #[inline(always)]
    unsafe fn interest_lo_hi() -> InterestLoHiMask {
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
            let lo = {
            _mm256_setr_epi8(
            //  0 1 2 3 4 5 6 7 8 9 a b c   d e f
                U,U,Q,U,U,U,U,U,U,U,C,B,N|E,B,U,U,
            //  0 1 2 3 4 5 6 7 8 9 a b c   d e f
                U,U,Q,U,U,U,U,U,U,U,C,B,N|E,B,U,U,
            )
        };

        #[rustfmt::skip]
            let hi = {
            _mm256_setr_epi8(
            //  0 1 2   3 4 5   6 7 8 9 a b c d e f
                U,U,N|Q,C,U,B|E,U,B,U,U,U,U,U,U,U,U,
            //  0 1 2   3 4 5   6 7 8 9 a b c d e f
                U,U,N|Q,C,U,B|E,U,B,U,U,U,U,U,U,U,U,
            )
        };

        InterestLoHiMask { lo, hi }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn active_mask_has_correct_repr() {
            assert_eq!(0isize, ActiveIndex::Interest as isize);
            assert_eq!(1isize, ActiveIndex::Quote as isize);
        }

        #[test]
        fn block_offset_is_32_bytes() {
            assert_eq!(32, Simd::BLOCK_SIZE);
        }
    }
}

#[cfg(not(wasm))]
pub(super) use x86_64::scan;

#[derive(Debug)]
pub(super) struct Simd {
    indexes: Indexes,
    active_index: ActiveIndex,
}

impl Simd {
    #[inline(always)]
    pub(super) fn new() -> Self {
        Simd {
            indexes: Indexes {
                interest: 0,
                quote: 0,
            },
            active_index: ActiveIndex::Interest,
        }
    }
}

impl Scan {
    #[inline(always)]
    pub(super) fn set_index_quote(&mut self) {
        self.simd.active_index = ActiveIndex::Quote;
        pre_index_quote(&mut self.simd.indexes);
    }

    #[inline(always)]
    pub(super) fn shift_index_quote(&mut self) {
        test_assert_eq!(ActiveIndex::Quote, self.simd.active_index);
        pre_index_quote(&mut self.simd.indexes);
    }

    #[inline(always)]
    pub(super) fn set_index_interest(&mut self) {
        self.simd.active_index = ActiveIndex::Interest;
        pre_index_interest(&mut self.simd.indexes);
    }
}

#[repr(C)]
#[repr(align(4))]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Indexes {
    // note: the order of these fields cannot be changed
    // they must match the set of variants in `ActiveMask`
    interest: i32,
    quote: i32,
}

// note: these fields cannot be changed without `Masks`
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(isize)]
enum ActiveIndex {
    Interest,
    Quote,
}

impl Default for ActiveIndex {
    #[inline(always)]
    fn default() -> Self {
        ActiveIndex::Interest
    }
}

impl Index<ActiveIndex> for Indexes {
    type Output = i32;

    #[inline(always)]
    fn index(&self, id: ActiveIndex) -> &i32 {
        // SAFETY: this is safe because the index is within the range of `Masks`
        unsafe { &*(self as *const Indexes as *const i32).offset(id as isize) }
    }
}

// when the quote index is active, unset all bits in the interest
// mask up to the next quote character
#[inline(always)]
fn pre_index_quote(indexes: &mut Indexes) {
    let offset = indexes.quote.trailing_zeros();

    // Exclude control characters up to the next quote or escape
    let shift = (!0i64 << offset) as i32;
    indexes.interest &= shift;
}

// when the interest index is active, there's no need to
// do anything
#[inline(always)]
fn pre_index_interest(_: &mut Indexes) {}
