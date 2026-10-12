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
    // SAFETY: forwarded from this function's contract; the ordinary specialization
    // does not use the zero subtraction vector or empty polynomial.
    unsafe { fill_pair::<K, false>(feedback, starts, PowerCoordinates::ZERO, &[], outputs) };
}

/// Emit one power sequence and one adjoint sequence with canonical coordinates.
///
/// # Safety
/// AVX512F and AVX512BW must be available. `K == 11`, both outputs and
/// `polynomial` must have the same length, and all inputs must be canonical.
/// The degree and length requirements are asserted before any output access.
#[target_feature(enable = "avx512f,avx512bw")]
#[inline(never)]
pub(super) unsafe fn fill_powers_adjoint_pair<const K: usize>(
    feedback: &[u16; K],
    starts: [PowerCoordinates<K>; 2],
    subtract: PowerCoordinates<K>,
    polynomial: &[u16],
    outputs: [&mut [PowerCoordinates<K>]; 2],
) {
    // SAFETY: this function exposes exactly the adjoint specialization's contract.
    unsafe { fill_pair::<K, true>(feedback, starts, subtract, polynomial, outputs) };
}

fn centered_multiplier(value: u16) -> (i16, i16) {
    let centered = i32::from(value) - if value > Q / 2 { i32::from(Q) } else { 0 };
    // Rust truncates toward zero; adding a sign-dependent half rounds nearest.
    let scaled = centered * 32768;
    let quotient = (scaled
        + if scaled >= 0 {
            i32::from(Q / 2)
        } else {
            -i32::from(Q / 2)
        })
        / i32::from(Q);
    (centered as i16, quotient as i16)
}

/// Canonicalize signed lanes in [-16896,29184], with q replicated in `q`.
#[target_feature(enable = "avx512f,avx512bw")]
#[inline]
unsafe fn reduce_adjoint(sum: __m512i, q: __m512i) -> __m512i {
    // k = round(sum / 16384). Its four intervals have exact remainder ranges:
    // [-16896,-8193] -> [-4607,4096]; [-8192,8191] -> [-8192,8191];
    // [8192,24575] -> [-4097,12286]; [24576,29184] -> [-2,4606].
    // Thus -q < sum-k*q < q, requiring only an addition for negative lanes.
    let quotient = _mm512_mulhrs_epi16(sum, _mm512_set1_epi16(2));
    let remainder = _mm512_sub_epi16(sum, _mm512_mullo_epi16(quotient, q));
    _mm512_min_epu16(remainder, _mm512_add_epi16(remainder, q))
}

#[target_feature(enable = "avx512f,avx512bw")]
#[inline]
unsafe fn fill_pair<const K: usize, const ADJOINT: bool>(
    feedback: &[u16; K],
    starts: [PowerCoordinates<K>; 2],
    subtract: PowerCoordinates<K>,
    polynomial: &[u16],
    outputs: [&mut [PowerCoordinates<K>]; 2],
) {
    assert_eq!(K, 11);
    assert_eq!(outputs[0].len(), outputs[1].len());
    if ADJOINT {
        assert_eq!(outputs[0].len(), polynomial.len());
    }
    let mut coefficients = [0i16; 32];
    let mut quotients = [0i16; 32];
    for i in 0..11 {
        (coefficients[i], quotients[i]) = centered_multiplier(feedback[i]);
        coefficients[i + 16] = coefficients[i];
        quotients[i + 16] = quotients[i];
    }
    // The correction is zero in the first sequence's lanes. Preparing it once
    // amortizes the fixed multiplication over the entire public polynomial.
    let mut subtract_coefficients = [0i16; 32];
    let mut subtract_quotients = [0i16; 32];
    if ADJOINT {
        for i in 0..11 {
            (subtract_coefficients[16 + i], subtract_quotients[16 + i]) =
                centered_multiplier(subtract.0[i]);
        }
    }
    // Each source is a complete 64-byte array, with no alignment requirement.
    let feedback = unsafe { _mm512_loadu_si512(coefficients.as_ptr().cast()) };
    let quotients = unsafe { _mm512_loadu_si512(quotients.as_ptr().cast()) };
    let subtract_coefficients =
        unsafe { _mm512_loadu_si512(subtract_coefficients.as_ptr().cast()) };
    let subtract_quotients = unsafe { _mm512_loadu_si512(subtract_quotients.as_ptr().cast()) };
    let q = _mm512_set1_epi16(Q as i16);
    let top_indices = unsafe { _mm512_loadu_si512(TOP_INDICES.as_ptr().cast()) };
    let shift_indices = unsafe { _mm512_loadu_si512(SHIFT_INDICES.as_ptr().cast()) };
    // The degree assertion ensures each input has exactly eleven coordinates.
    let mut value = unsafe { load_starts(starts) };
    let [a, b] = outputs;
    let length = a.len();
    for (j, (a, b)) in a.iter_mut().zip(b).enumerate() {
        // The degree assertion guarantees eleven writable u16 coordinates.
        unsafe { store(value, a, b) };
        if ADJOINT && j + 1 == length {
            break;
        }
        let top = _mm512_permutexvar_epi16(top_indices, value);
        // c = round(centered_feedback * 2^15 / q) has error < 1/2.
        // Since 0 <= top <= 12288, the rounded quotient errs by < 11/16;
        // the exact signed remainder is therefore in [-8448, 8448].
        let quotient = _mm512_mulhrs_epi16(top, quotients);
        let product = _mm512_mullo_epi16(top, feedback);
        let remainder = _mm512_sub_epi16(product, _mm512_mullo_epi16(quotient, q));
        let shifted = _mm512_maskz_permutexvar_epi16(SHIFT, shift_indices, value);
        let sum = _mm512_add_epi16(remainder, shifted);
        if ADJOINT {
            let scalar = _mm512_set1_epi16(polynomial[length - 1 - j] as i16);
            let quotient = _mm512_mulhrs_epi16(scalar, subtract_quotients);
            let product = _mm512_mullo_epi16(scalar, subtract_coefficients);
            let correction = _mm512_sub_epi16(product, _mm512_mullo_epi16(quotient, q));
            // Both exact remainders lie in [-8448,8448] and the predecessor
            // is canonical, so the combined sum fits signed16: [-16896,29184].
            let sum = _mm512_sub_epi16(sum, correction);
            // SAFETY: the bound above and this function's AVX512 features satisfy
            // the fused reduction's contract; `q` contains 12289 in every lane.
            value = unsafe { reduce_adjoint(sum, q) };
        } else {
            // Adding a canonical predecessor gives -8448 <= sum <= 20736.
            // Wrapped negative lanes are larger than sum+q as unsigned words;
            // nonnegative lanes are smaller. Then subtract q at most once.
            let positive = _mm512_min_epu16(sum, _mm512_add_epi16(sum, q));
            value = _mm512_min_epu16(positive, _mm512_sub_epi16(positive, q));
        }
    }
}
/// Interleave independent paired recurrences, sharing feedback preparation.
///
/// # Safety
/// AVX512F+BW must be available, K must be eleven, PAIRS must be positive,
/// all polynomials/outputs must have equal lengths, and all inputs must be
/// canonical. Each mutable output is a distinct slice supplied by the caller.
#[target_feature(enable = "avx512f,avx512bw")]
pub(super) unsafe fn fill_powers_adjoint_pairs<const K: usize, const PAIRS: usize>(
    feedback: &[u16; K],
    starts: [[PowerCoordinates<K>; 2]; PAIRS],
    subtract: [PowerCoordinates<K>; PAIRS],
    polynomials: [&[u16]; PAIRS],
    mut outputs: [[&mut [PowerCoordinates<K>]; 2]; PAIRS],
) {
    assert_eq!(K, 11);
    assert!(PAIRS > 0);
    let len = polynomials[0].len();
    for i in 0..PAIRS {
        assert_eq!(polynomials[i].len(), len);
        assert_eq!(outputs[i][0].len(), len);
        assert_eq!(outputs[i][1].len(), len);
    }
    let mut coefficients = [0i16; 32];
    let mut quotients = [0i16; 32];
    for i in 0..11 {
        (coefficients[i], quotients[i]) = centered_multiplier(feedback[i]);
        coefficients[i + 16] = coefficients[i];
        quotients[i + 16] = quotients[i];
    }
    // These arrays each contain a full vector; unaligned loads are permitted.
    let feedback = unsafe { _mm512_loadu_si512(coefficients.as_ptr().cast()) };
    let quotients = unsafe { _mm512_loadu_si512(quotients.as_ptr().cast()) };
    let correction: [(__m512i, __m512i); PAIRS] = std::array::from_fn(|pair| {
        let mut coefficients = [0i16; 32];
        let mut quotients = [0i16; 32];
        for i in 0..11 {
            (coefficients[16 + i], quotients[16 + i]) = centered_multiplier(subtract[pair].0[i]);
        }
        // Zero lower lanes leave the ordinary power sequence unchanged.
        unsafe {
            (
                _mm512_loadu_si512(coefficients.as_ptr().cast()),
                _mm512_loadu_si512(quotients.as_ptr().cast()),
            )
        }
    });
    // Degree eleven and canonical starts satisfy load_starts' shape contract.
    let mut values = starts.map(|start| unsafe { load_starts(start) });
    let q = _mm512_set1_epi16(Q as i16);
    let top_indices = unsafe { _mm512_loadu_si512(TOP_INDICES.as_ptr().cast()) };
    let shift_indices = unsafe { _mm512_loadu_si512(SHIFT_INDICES.as_ptr().cast()) };
    for j in 0..len {
        for pair in 0..PAIRS {
            let [a, b] = &mut outputs[pair];
            // Each output has len entries of exactly eleven writable u16s.
            unsafe { store(values[pair], &mut a[j], &mut b[j]) };
        }
        if j + 1 == len {
            break;
        }
        for pair in 0..PAIRS {
            let value = values[pair];
            let top = _mm512_permutexvar_epi16(top_indices, value);
            let quotient = _mm512_mulhrs_epi16(top, quotients);
            let product = _mm512_mullo_epi16(top, feedback);
            let remainder = _mm512_sub_epi16(product, _mm512_mullo_epi16(quotient, q));
            let shifted = _mm512_maskz_permutexvar_epi16(SHIFT, shift_indices, value);
            let scalar = _mm512_set1_epi16(polynomials[pair][len - 1 - j] as i16);
            let quotient = _mm512_mulhrs_epi16(scalar, correction[pair].1);
            let product = _mm512_mullo_epi16(scalar, correction[pair].0);
            let subtract = _mm512_sub_epi16(product, _mm512_mullo_epi16(quotient, q));
            let sum = _mm512_sub_epi16(_mm512_add_epi16(remainder, shifted), subtract);
            // As in fill_pair: each remainder lies in [-8448,8448] and the
            // predecessor is canonical, giving sum in [-16896,29184].
            values[pair] = unsafe { reduce_adjoint(sum, q) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PowerBasis, PowerCoordinates, Q12289Extension};
    use super::{
        fill_powers_adjoint_pair, fill_powers_adjoint_pairs, fill_powers_pair, reduce_adjoint,
    };
    use core::arch::x86_64::*;

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

    fn check_adjoint(
        feedback: [u16; 11],
        starts: [PowerCoordinates<11>; 2],
        subtract: PowerCoordinates<11>,
        polynomial: &[u16],
    ) {
        let basis = PowerBasis {
            columns: [[0; 11]; 11],
            inverse: [[0; 11]; 11],
            feedback,
        };
        let len = polynomial.len();
        let mut expected_a = Vec::with_capacity(len);
        let mut expected_b = Vec::with_capacity(len);
        let [mut first, mut second] = starts;
        for j in 0..len {
            expected_a.push(first);
            expected_b.push(second);
            basis.advance(&mut first);
            basis.advance_sub_scaled(&mut second, polynomial[len - 1 - j], subtract);
        }
        let sentinel = PowerCoordinates([u16::MAX; 11]);
        let mut a = vec![sentinel; len + 4];
        let mut b = a.clone();
        // Runtime features were checked by the caller; all inputs are
        // canonical degree-eleven values with matching guarded output lengths.
        unsafe {
            fill_powers_adjoint_pair(
                &feedback,
                starts,
                subtract,
                polynomial,
                [&mut a[2..len + 2], &mut b[2..len + 2]],
            );
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
    fn adjoint_recurrence_matches_portable_with_guarded_tails() {
        if !available() {
            return;
        }
        let lengths = [0, 1, 2, 3, 7, 15, 16, 17, 31, 32, 33, 255, 256, 257, 1024];
        let mut state = 0xfedc_ba98_7654_3210;
        for _ in 0..64 {
            let feedback = std::array::from_fn(|_| sample(&mut state));
            let starts = std::array::from_fn(|_| {
                PowerCoordinates(std::array::from_fn(|_| sample(&mut state)))
            });
            let subtract = PowerCoordinates(std::array::from_fn(|_| sample(&mut state)));
            for len in lengths {
                let polynomial: Vec<_> = (0..len).map(|_| sample(&mut state)).collect();
                check_adjoint(feedback, starts, subtract, &polynomial);
            }
        }
        for feedback in [[0; 11], [1; 11], [6144; 11], [6145; 11], [Q - 1; 11]] {
            for coordinate in [0, 1, 6144, 6145, Q - 2, Q - 1] {
                let starts = [
                    PowerCoordinates([coordinate; 11]),
                    PowerCoordinates([Q - 1 - coordinate; 11]),
                ];
                for subtract in [PowerCoordinates([coordinate; 11]), starts[1]] {
                    let polynomial: Vec<_> = (0..257)
                        .map(|i| [0, 1, 6144, 6145, Q - 2, Q - 1][i % 6])
                        .collect();
                    check_adjoint(feedback, starts, subtract, &polynomial);
                }
            }
        }
    }

    fn check_interleaved<const PAIRS: usize>() {
        let mut state = 0xface_cafe_1234_5678;
        for len in [0, 1, 2, 3, 7, 16, 17, 31, 32, 33, 255, 257, 512, 1024] {
            for case in 0..16 {
                let mut next = || match case {
                    0 => 0,
                    1 => Q - 1,
                    2 => 6144,
                    3 => 6145,
                    _ => sample(&mut state),
                };
                let feedback = std::array::from_fn(|_| next());
                let starts: [[PowerCoordinates<11>; 2]; PAIRS] = std::array::from_fn(|_| {
                    std::array::from_fn(|_| PowerCoordinates(std::array::from_fn(|_| next())))
                });
                let subtract =
                    std::array::from_fn(|_| PowerCoordinates(std::array::from_fn(|_| next())));
                let polynomials: [Vec<u16>; PAIRS] =
                    std::array::from_fn(|_| (0..len).map(|_| next()).collect());
                let basis = PowerBasis {
                    columns: [[0; 11]; 11],
                    inverse: [[0; 11]; 11],
                    feedback,
                };
                let sentinel = PowerCoordinates([u16::MAX; 11]);
                let mut actual: [[Vec<PowerCoordinates<11>>; 2]; PAIRS] =
                    std::array::from_fn(|_| std::array::from_fn(|_| vec![sentinel; len + 4]));
                let mut buffers = actual.iter_mut();
                let outputs = std::array::from_fn(|_| {
                    let [a, b] = buffers.next().unwrap();
                    [&mut a[2..len + 2], &mut b[2..len + 2]]
                });
                // Feature availability is checked by the test; inputs have
                // canonical lanes and all guarded slices have the same length.
                unsafe {
                    fill_powers_adjoint_pairs(
                        &feedback,
                        starts,
                        subtract,
                        std::array::from_fn(|i| polynomials[i].as_slice()),
                        outputs,
                    );
                }
                for pair in 0..PAIRS {
                    let [mut a, mut b] = starts[pair];
                    for j in 0..len {
                        assert_eq!(actual[pair][0][j + 2], a);
                        assert_eq!(actual[pair][1][j + 2], b);
                        basis.advance(&mut a);
                        basis.advance_sub_scaled(
                            &mut b,
                            polynomials[pair][len - 1 - j],
                            subtract[pair],
                        );
                    }
                    for output in &actual[pair] {
                        assert_eq!(&output[..2], &[sentinel; 2]);
                        assert_eq!(&output[len + 2..], &[sentinel; 2]);
                        assert!(
                            output[2..len + 2]
                                .iter()
                                .all(|v| v.0.iter().all(|&c| c < Q))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn interleaved_adjoint_matches_portable_with_guarded_tails() {
        if !available() {
            return;
        }
        check_interleaved::<2>();
        check_interleaved::<4>();
    }

    #[target_feature(enable = "avx512f,avx512bw")]
    unsafe fn check_adjoint_reduction() {
        for base in (-16896_i32..=29184).step_by(32) {
            let input: [i16; 32] =
                std::array::from_fn(|lane| (base + lane as i32).min(29184) as i16);
            let mut output = [0u16; 32];
            // SAFETY: complete 64-byte arrays permit unaligned loads/stores;
            // every input lane is in the reduction's prescribed signed range.
            unsafe {
                let value = _mm512_loadu_si512(input.as_ptr().cast());
                let reduced = reduce_adjoint(value, _mm512_set1_epi16(Q as i16));
                _mm512_storeu_si512(output.as_mut_ptr().cast(), reduced);
            }
            for (&raw, &actual) in input.iter().zip(&output) {
                assert_eq!(actual, i32::from(raw).rem_euclid(i32::from(Q)) as u16);
            }
        }
    }

    #[test]
    fn adjoint_reduction_covers_every_permitted_signed_sum() {
        if !available() {
            return;
        }
        // SAFETY: the runtime feature check covers this exhaustive SIMD helper.
        unsafe { check_adjoint_reduction() };
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
