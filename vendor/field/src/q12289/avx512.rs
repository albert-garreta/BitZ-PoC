//! Two public degree-11 power sequences in one AVX-512 vector.
use super::PowerCoordinates;
use core::arch::x86_64::*;

const Q: u16 = 12_289;
const SHIFT: u32 = 0x07fe_07fe;
const TOP_INDICES: [u16; 32] = [
    10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 26, 26, 26, 26, 26, 26, 26, 26,
    26, 26, 26, 26, 26, 26, 26, 26,
];
const SHIFT_INDICES: [u16; 32] = [
    0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 16, 17, 18, 19, 20, 21, 22, 23, 24,
    25, 26, 27, 28, 29, 30,
];

#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn load_starts<const K: usize>(starts: [PowerCoordinates<K>; 2]) -> __m512i {
    let mut packed = [0u16; 32];
    packed[..11].copy_from_slice(&starts[0].0);
    packed[16..27].copy_from_slice(&starts[1].0);
    // `packed` is a complete 64-byte array.
    unsafe { _mm512_loadu_si512(packed.as_ptr().cast()) }
}

#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn store<const K: usize>(
    value: __m512i,
    a: &mut PowerCoordinates<K>,
    b: &mut PowerCoordinates<K>,
) {
    // Only eleven enabled 16-bit stores: the 22-byte destination is not padded.
    unsafe { _mm512_mask_storeu_epi16(a.0.as_mut_ptr().cast(), 0x07ff, value) };
    let upper = _mm512_shuffle_i64x2::<0x4e>(value, value);
    unsafe { _mm512_mask_storeu_epi16(b.0.as_mut_ptr().cast(), 0x07ff, upper) };
}

/// Emit both sequences with canonical coordinates, including their starts.
///
/// # Safety
/// AVX512F and AVX512BW must be available. `K == 11`, the two output
/// lengths must agree, and feedback/start coordinates must be canonical.
/// The degree and length requirements are asserted before any output access.
/// Feedback preparation may branch because this basis belongs to a public
/// generator; the recurrence uses data-independent instructions and addresses.
#[target_feature(enable = "avx512f,avx512bw")]
#[inline(never)]
pub(super) unsafe fn fill_powers_pair<const K: usize>(
    feedback: &[u16; K],
    starts: [PowerCoordinates<K>; 2],
    outputs: [&mut [PowerCoordinates<K>]; 2],
) {
    assert_eq!(K, 11);
    assert_eq!(outputs[0].len(), outputs[1].len());
    let mut coefficients = [0i16; 32];
    let mut quotients = [0i16; 32];
    for i in 0..11 {
        let centered = i32::from(feedback[i]) - if feedback[i] > Q / 2 { i32::from(Q) } else { 0 };
        coefficients[i] = centered as i16;
        coefficients[i + 16] = coefficients[i];
        // Rust truncates toward zero; adding a sign-dependent half rounds nearest.
        let scaled = centered * 32768;
        quotients[i] = ((scaled
            + if scaled >= 0 {
                i32::from(Q / 2)
            } else {
                -i32::from(Q / 2)
            })
            / i32::from(Q)) as i16;
        quotients[i + 16] = quotients[i];
    }
    // Each source is a complete 64-byte array, with no alignment requirement.
    let feedback = unsafe { _mm512_loadu_si512(coefficients.as_ptr().cast()) };
    let quotients = unsafe { _mm512_loadu_si512(quotients.as_ptr().cast()) };
    let q = _mm512_set1_epi16(Q as i16);
    let top_indices = unsafe { _mm512_loadu_si512(TOP_INDICES.as_ptr().cast()) };
    let shift_indices = unsafe { _mm512_loadu_si512(SHIFT_INDICES.as_ptr().cast()) };
    // The degree assertion ensures each input has exactly eleven coordinates.
    let mut value = unsafe { load_starts(starts) };
    let [a, b] = outputs;
    for (a, b) in a.iter_mut().zip(b) {
        // The degree assertion guarantees eleven writable u16 coordinates.
        unsafe { store(value, a, b) };
        let top = _mm512_permutexvar_epi16(top_indices, value);
        // c = round(centered_feedback * 2^15 / q) has error < 1/2.
        // Since 0 <= top <= 12288, the rounded quotient errs by < 11/16;
        // the exact signed remainder is therefore in [-8448, 8448].
        let quotient = _mm512_mulhrs_epi16(top, quotients);
        let product = _mm512_mullo_epi16(top, feedback);
        let remainder = _mm512_sub_epi16(product, _mm512_mullo_epi16(quotient, q));
        let shifted = _mm512_maskz_permutexvar_epi16(SHIFT, shift_indices, value);
        let sum = _mm512_add_epi16(remainder, shifted);
        // Adding a canonical predecessor gives -8448 <= sum <= 20736.
        // Wrapped negative lanes are larger than sum+q as unsigned words;
        // nonnegative lanes are smaller. Then subtract q at most once.
        let positive = _mm512_min_epu16(sum, _mm512_add_epi16(sum, q));
        value = _mm512_min_epu16(positive, _mm512_sub_epi16(positive, q));
    }
}
#[cfg(test)]
mod tests {
    use super::super::{PowerBasis, PowerCoordinates, Q12289Extension};
    use super::fill_powers_pair;

    const Q: u16 = 12_289;
    const ZERO: PowerCoordinates<11> = PowerCoordinates::ZERO;

    fn available() -> bool {
        std::arch::is_x86_feature_detected!("avx512f")
            && std::arch::is_x86_feature_detected!("avx512bw")
    }

    fn sample(state: &mut u64) -> u16 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state % u64::from(Q)) as u16
    }

    fn check(feedback: [u16; 11], starts: [PowerCoordinates<11>; 2], len: usize) {
        // Advancing only reads feedback. Arbitrary canonical coefficients let
        // this test exercise more reductions than generated bases alone.
        let basis = PowerBasis {
            columns: [[0; 11]; 11],
            inverse: [[0; 11]; 11],
            feedback,
        };
        let mut expected_a = vec![ZERO; len];
        let mut expected_b = vec![ZERO; len];
        basis.fill_powers(starts[0], &mut expected_a);
        basis.fill_powers(starts[1], &mut expected_b);

        let sentinel = PowerCoordinates([u16::MAX; 11]);
        let mut a = vec![sentinel; len + 4];
        let mut b = vec![sentinel; len + 4];
        // Each test checks runtime features before reaching this helper; the
        // feedback/starts are canonical and both slices have length `len`.
        unsafe {
            fill_powers_pair(&feedback, starts, [&mut a[2..len + 2], &mut b[2..len + 2]]);
        }
        assert_eq!(&a[2..len + 2], expected_a.as_slice());
        assert_eq!(&b[2..len + 2], expected_b.as_slice());
        for output in [&a, &b] {
            assert_eq!(&output[..2], &[sentinel; 2]);
            assert_eq!(&output[len + 2..], &[sentinel; 2]);
            assert!(
                output[2..len + 2]
                    .iter()
                    .all(|value| value.0.iter().all(|&coordinate| coordinate < Q))
            );
        }
    }

    #[test]
    fn paired_recurrence_matches_portable_with_guarded_tails() {
        if !available() {
            return;
        }
        assert_eq!(std::mem::size_of::<PowerCoordinates<11>>(), 22);
        let lengths = [0, 1, 2, 3, 7, 15, 16, 17, 31, 32, 33, 255, 256, 257, 1024];
        let mut state = 0x1234_5678_9abc_def0;
        for _ in 0..256 {
            let feedback = std::array::from_fn(|_| sample(&mut state));
            let starts = std::array::from_fn(|_| {
                PowerCoordinates(std::array::from_fn(|_| sample(&mut state)))
            });
            for len in lengths {
                check(feedback, starts, len);
            }
        }
        for feedback in [[0; 11], [1; 11], [6144; 11], [6145; 11], [Q - 1; 11]] {
            for coordinate in [0, 1, 6144, 6145, Q - 2, Q - 1] {
                let starts = [
                    PowerCoordinates([coordinate; 11]),
                    PowerCoordinates([Q - 1 - coordinate; 11]),
                ];
                for len in lengths {
                    check(feedback, starts, len);
                }
            }
        }
        // These sparse cases cross the 128-bit lane boundary at index eight
        // and include nonzero first/last coefficients.
        for coordinate in 0..11 {
            let mut sparse = [0; 11];
            sparse[coordinate] = Q - 1;
            check(
                sparse,
                [PowerCoordinates(sparse), PowerCoordinates([Q - 1; 11])],
                4096,
            );
        }
    }

    #[test]
    fn paired_recurrence_matches_fixed_basis_multiplication() {
        if !available() {
            return;
        }
        for seed in 0..12 {
            let alpha = Q12289Extension(std::array::from_fn(|i| {
                if seed == 0 {
                    u16::from(i == 1)
                } else {
                    ((seed * 1237 + i * 97) % usize::from(Q)) as u16
                }
            }));
            let basis = PowerBasis::<11>::try_new(alpha).unwrap();
            let starts = [
                Q12289Extension::ONE,
                Q12289Extension(std::array::from_fn(|i| {
                    ((seed * 73 + i * 931) % usize::from(Q)) as u16
                })),
            ];
            let mut a = vec![ZERO; 1024];
            let mut b = vec![ZERO; 1024];
            // Available AVX512F+BW, canonical prepared inputs, degree eleven.
            unsafe {
                fill_powers_pair(
                    &basis.feedback,
                    starts.map(|start| basis.encode(start)),
                    [&mut a, &mut b],
                );
            }
            for (mut expected, output) in starts.into_iter().zip([a, b]) {
                for actual in output {
                    assert_eq!(basis.decode(actual), expected);
                    assert_eq!(actual, basis.encode(expected));
                    expected = expected.mul(alpha);
                }
            }
        }
    }

    #[test]
    #[ignore = "exhaustive: every feedback/top pair with predecessor endpoints"]
    fn paired_reduction_exhaustive() {
        if !available() {
            return;
        }
        for coefficient in 0..Q {
            // Repeating feedback includes both predecessor endpoints in lane one;
            // lane zero always has predecessor zero.
            let feedback = [coefficient; 11];
            let basis = PowerBasis {
                columns: [[0; 11]; 11],
                inverse: [[0; 11]; 11],
                feedback,
            };
            for top in 0..Q {
                let mut a = ZERO;
                let mut b = PowerCoordinates([Q - 1; 11]);
                a.0[10] = top;
                b.0[10] = top;
                let starts = [a, b];
                basis.advance(&mut a);
                basis.advance(&mut b);
                let mut out_a = [ZERO; 2];
                let mut out_b = [ZERO; 2];
                // All operands are canonical and both outputs have length two.
                unsafe {
                    fill_powers_pair(&feedback, starts, [&mut out_a, &mut out_b]);
                }
                assert_eq!(
                    out_a,
                    [starts[0], a],
                    "coefficient={coefficient}, top={top}"
                );
                assert_eq!(
                    out_b,
                    [starts[1], b],
                    "coefficient={coefficient}, top={top}"
                );
            }
        }
    }
}
