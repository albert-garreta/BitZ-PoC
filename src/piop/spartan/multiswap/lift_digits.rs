//! Fixed-traversal digit accumulation for four packed bit columns.

/// Adds at most 64 selected weights. The caller flushes after 4096 rows, so
/// every accumulator remains below 4096 * (2^32 - 1), including partial words.
#[inline]
pub(super) fn accumulate_word(digits: &mut [[u64; 4]; 4], words: [u64; 4], weights: &[[u32; 4]]) {
    debug_assert!(weights.len() <= 64);
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        // SAFETY: NEON is a compile-time target feature. All vector loads below
        // read two elements of fixed four-element arrays; no secret indexes.
        unsafe { accumulate_neon(digits, words, weights) }
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    accumulate_portable(digits, words, weights);
}

#[cfg(any(test, not(all(target_arch = "aarch64", target_feature = "neon"))))]
fn accumulate_portable(digits: &mut [[u64; 4]; 4], words: [u64; 4], weights: &[[u32; 4]]) {
    use field::{CtMask, CtSelect};
    for (bit, weight) in weights.iter().enumerate() {
        for (sum, &word) in digits.iter_mut().zip(&words) {
            let mask = u64::ct_select(&0, &u64::MAX, CtMask::from_lsb(word >> bit));
            for (acc, &digit) in sum.iter_mut().zip(weight) {
                *acc += u64::from(digit) & mask;
            }
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
unsafe fn accumulate_neon(digits: &mut [[u64; 4]; 4], words: [u64; 4], weights: &[[u32; 4]]) {
    use core::arch::aarch64::*;
    // SAFETY: the caller guarantees NEON and at most 64 public row positions.
    // SIMD shifts, masks and additions operate on values, never addresses.
    unsafe {
        let left_words = vld1q_u64(words.as_ptr());
        let right_words = vld1q_u64(words.as_ptr().add(2));
        let zero = vdupq_n_u64(0);
        let one = vdupq_n_u64(1);
        let mut left: [uint64x2_t; 4] =
            core::array::from_fn(|d| vld1q_u64([digits[0][d], digits[1][d]].as_ptr()));
        let mut right: [uint64x2_t; 4] =
            core::array::from_fn(|d| vld1q_u64([digits[2][d], digits[3][d]].as_ptr()));
        for (bit, weight) in weights.iter().enumerate() {
            let shift = vdupq_n_s64(-(bit as i64));
            let lm = vsubq_u64(zero, vandq_u64(vshlq_u64(left_words, shift), one));
            let rm = vsubq_u64(zero, vandq_u64(vshlq_u64(right_words, shift), one));
            for d in 0..4 {
                let value = vdupq_n_u64(u64::from(weight[d]));
                left[d] = vaddq_u64(left[d], vandq_u64(value, lm));
                right[d] = vaddq_u64(right[d], vandq_u64(value, rm));
            }
        }
        for d in 0..4 {
            digits[0][d] = vgetq_lane_u64::<0>(left[d]);
            digits[1][d] = vgetq_lane_u64::<1>(left[d]);
            digits[2][d] = vgetq_lane_u64::<0>(right[d]);
            digits[3][d] = vgetq_lane_u64::<1>(right[d]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_digits_match_portable_for_every_partial_word() {
        let mut state = 0x1378123u64;
        for len in 0..=64 {
            let weights: Vec<[u32; 4]> = (0..len)
                .map(|_| {
                    core::array::from_fn(|_| {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        state as u32
                    })
                })
                .collect();
            for words in [
                [0; 4],
                [u64::MAX; 4],
                [1, 1 << 63, 0xaaaa5555aaaa5555, state],
            ] {
                let mut actual = [[(1 << 40) - 1; 4]; 4];
                let mut expected = actual;
                accumulate_word(&mut actual, words, &weights);
                accumulate_portable(&mut expected, words, &weights);
                assert_eq!(actual, expected, "length {len}, bits {words:x?}");
            }
        }
    }
}
