use super::{
    BETA_SQUARED, FalconError, FalconPublicKey, FalconSourceLayout, FalconSourceWitness,
    FalconVerificationTrace, HASH_TO_POINT_SAMPLES, N, Q,
};
use super::{NORM_BITS, SIGNATURE_BITS};

/// Maximum absolute exact source residual, before relying on any other
/// constraint. Raw signed-12 S2 bits may decode to -2048. The norm dominates
/// division, public selection, and public-input residuals in both layouts.
pub(super) const SOURCE_RESIDUAL_BOUND: u128 = N as u128
    * (6_144u128.pow(2) + (1u128 << (SIGNATURE_BITS - 1)).pow(2))
    + ((1u128 << NORM_BITS) - 1)
    - BETA_SQUARED as u128;

/// Logical constraint inventory per Falcon signature. These are exact linear
/// or low-degree relation rows, not generic bit-blasted R1CS rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconConstraintCounts {
    /// Public one/message-bit/signature-byte bindings.
    pub public_input_bindings: usize,
    /// Candidate divisions, public-mask acceptance, and selected-coefficient rows.
    pub hash_to_point_linear: usize,
    /// Terms in the degree-2 norm sumcheck.
    pub norm_terms: usize,
    /// Rejection bits as products of the two quotient bits.
    pub hash_to_point_quadratic: usize,
}

impl FalconConstraintCounts {
    /// Scalar relation inventory for the source layout. Native ring membership
    /// and the quadratic rejection-bit reduction are separate obligations.
    pub const fn per_signature() -> Self {
        Self {
            public_input_bindings: 1 + 32 * 8 + super::CT_SIGNATURE_BYTES + N,
            hash_to_point_linear: 2 * HASH_TO_POINT_SAMPLES + N,
            norm_terms: 2 * N,
            hash_to_point_quadratic: HASH_TO_POINT_SAMPLES,
        }
    }

    /// Rows batched by the single random linear/ideal-check binder.
    pub const fn linear_rows(self) -> usize {
        self.public_input_bindings + self.hash_to_point_linear
    }

    /// Number of variables in the batched coefficient domain.
    pub fn coefficient_vars(layout: &FalconSourceLayout) -> usize {
        N.trailing_zeros() as usize + layout.capacity().trailing_zeros() as usize
    }

    /// The norm polynomial has degree two in every variable.
    pub const fn norm_round_degree(self) -> usize {
        2
    }

    /// Quadratic row sumchecks have degree three (`eq * (A * B - C)`).
    pub const fn product_round_degree(self) -> usize {
        3
    }
}

/// Independent exact-integer checker for a generated trace.  This is the
/// native mirror of the PIOP relation and is intentionally written as the
/// displayed equations, rather than calling the verifier that generated the
/// trace.
pub fn check_exact_constraints(trace: &FalconVerificationTrace) -> Result<(), FalconError> {
    check_keccak_constraints(trace)?;
    let hash = &trace.hash_to_point;
    for i in 0..HASH_TO_POINT_SAMPLES {
        let word = i64::from(hash.words[i]);
        let quotient = i64::from(hash.quotients[i]);
        let remainder = i64::from(hash.remainders[i]);
        if word != Q * quotient + remainder {
            return violation("hash-division", i);
        }
        if !(0..Q).contains(&remainder) {
            return violation("hash-remainder-range", i);
        }
        if !(0..=5).contains(&quotient) {
            return violation("hash-quotient-range", i);
        }
        let reject_and = (hash.quotients[i] >> 2 & 1) & (hash.quotients[i] & 1);
        if hash.accepted[i] != (reject_and == 0) {
            return violation("hash-accept-and", i);
        }
    }
    if hash.accepted.iter().filter(|&&accepted| accepted).count() < N {
        return violation("hash-accepted-count", HASH_TO_POINT_SAMPLES);
    }
    let compacted: Vec<_> = hash
        .remainders
        .iter()
        .zip(hash.accepted.iter())
        .filter_map(|(&value, &accepted)| accepted.then_some(value))
        .take(N)
        .collect();
    if compacted.as_slice() != hash.point.as_slice() {
        return violation("hash-stable-compaction", 0);
    }

    // Recompute the product from the public key and signature so this checker
    // remains independent of the prover's cached native-ring witness.
    for (i, &s2) in trace.signature.s2.iter().enumerate() {
        if s2 <= -(1 << (SIGNATURE_BITS - 1)) || s2 >= 1 << (SIGNATURE_BITS - 1) {
            return violation("s2-canonical", i);
        }
    }
    let mut product = [0i64; 2 * N];
    for (i, &h) in trace.public_key.h.iter().enumerate() {
        if i64::from(h) >= Q {
            return violation("public-key-range", i);
        }
        for (j, &s2) in trace.signature.s2.iter().enumerate() {
            product[i + j] += i64::from(h) * i64::from(s2);
        }
    }
    for i in 0..N - 1 {
        if i64::from(trace.ring_quotient[i]) != (-product[N + i]).rem_euclid(Q) {
            return violation("native-quotient", i);
        }
    }

    let mut norm = 0u64;
    for i in 0..N {
        let s1 = i64::from(trace.s1[i]);
        if !(-6_144..=6_144).contains(&s1) {
            return violation("s1-centered-range", i);
        }
        let convolution = product[i] - product[i + N];
        if (i64::from(hash.point[i]) - convolution - s1).rem_euclid(Q) != 0 {
            return violation("falcon-ring", i);
        }
        norm = norm
            .checked_add(s1.unsigned_abs().pow(2))
            .and_then(|sum| sum.checked_add(i64::from(trace.signature.s2[i]).unsigned_abs().pow(2)))
            .ok_or(FalconError::ConstraintViolation {
                family: "norm-overflow",
                index: i,
            })?;
    }
    if norm != trace.norm
        || norm.checked_add(trace.norm_slack) != Some(BETA_SQUARED)
        || norm > BETA_SQUARED
    {
        return violation("falcon-norm", 0);
    }
    Ok(())
}

/// Native mirror of the additional shared-prime H rows. This checks decoded
/// source bits; the proof still authenticates these rows through its binder.
pub(super) fn check_public_key_bindings(
    source: &FalconSourceWitness,
    public_keys: &[FalconPublicKey],
) -> Result<(), FalconError> {
    let layout = source.layout();
    if public_keys.len() != layout.batch() {
        return Err(FalconError::InvalidBatchCapacity);
    }
    for (s, key) in public_keys.iter().enumerate() {
        for (j, &expected) in key.h.iter().enumerate() {
            if i64::from(expected) >= Q {
                return Err(FalconError::PublicKeyCoefficient { index: j });
            }
            let base = s * layout.signature_stride() + layout.public_key_bit(j, 0);
            let actual = (0..14).fold(0u16, |value, bit| {
                value | (u16::from(source.bit(base + bit)) << bit)
            });
            if actual != expected {
                return violation("public-key-source", s * N + j);
            }
        }
    }
    Ok(())
}

fn check_keccak_constraints(trace: &FalconVerificationTrace) -> Result<(), FalconError> {
    let shake = &trace.hash_to_point.shake;
    shake.validate_falcon_shape()?;
    // The 32 message bytes are statement-bound by the commitment adapter.
    for byte in 0..200 {
        if (40..72).contains(&byte) {
            continue;
        }
        let expected = match byte {
            0..40 => trace.signature.nonce[byte],
            72 => 0x1f,
            135 => 0x80,
            _ => 0,
        };
        if (shake.permutation_inputs[0][byte / 8] >> (8 * (byte % 8))) as u8 != expected {
            return violation("keccak-input", byte);
        }
    }
    // Forward rho/pi indexing makes this independent of the binder's inverse.
    const ROTATION: [[u32; 5]; 5] = [
        [0, 36, 3, 41, 18],
        [1, 44, 10, 45, 2],
        [62, 6, 43, 15, 61],
        [28, 55, 25, 21, 56],
        [27, 20, 39, 8, 14],
    ];
    let bit = |word: u64, index: usize| ((word >> index) & 1) as u16;
    for permutation in 0..super::PARAMETERS.permutations() {
        if permutation > 0
            && shake.permutation_inputs[permutation]
                != shake.round_states[(permutation * 24 - 1) * 25..permutation * 24 * 25]
        {
            return violation("keccak-squeeze-state", permutation);
        }
        for round in 0..24 {
            let round_index = permutation * 24 + round;
            let input: &[u64] = if round == 0 {
                &shake.permutation_inputs[permutation]
            } else {
                &shake.round_states[(round_index - 1) * 25..round_index * 25]
            };
            let columns = &shake.column_parities[round_index * 5..(round_index + 1) * 5];
            for x in 0..5 {
                for z in 0..64 {
                    let index = (round_index * 5 + x) * 64 + z;
                    let sum: u16 = (0..5).map(|y| bit(input[x + 5 * y], z)).sum();
                    if sum
                        != bit(columns[x], z) + 2 * u16::from(shake.column_parity_quotients[index])
                    {
                        return violation("keccak-column-parity", index);
                    }
                }
            }
            for y in 0..5 {
                for x in 0..5 {
                    let destination_lane = y + 5 * ((2 * x + 3 * y) % 5);
                    let output_word = round_index * 25 + destination_lane;
                    for z in 0..64 {
                        let destination_bit = (z + ROTATION[x][y] as usize) & 63;
                        let index = output_word * 64 + destination_bit;
                        let sum = bit(input[x + 5 * y], z)
                            + bit(columns[(x + 4) % 5], z)
                            + bit(columns[(x + 1) % 5], (z + 63) & 63);
                        if sum
                            != bit(shake.chi_inputs[output_word], destination_bit)
                                + 2 * u16::from(shake.parity_quotients[index])
                        {
                            return violation("keccak-theta-parity", index);
                        }
                    }
                    let index = round_index * 25 + x + 5 * y;
                    let expected_and = !shake.chi_inputs[round_index * 25 + (x + 1) % 5 + 5 * y]
                        & shake.chi_inputs[round_index * 25 + (x + 2) % 5 + 5 * y];
                    if shake.chi_ands[index] != expected_and {
                        return violation("keccak-chi", index);
                    }
                    let mut expected_state = shake.chi_inputs[index] ^ shake.chi_ands[index];
                    if x == 0 && y == 0 {
                        expected_state ^= super::keccak::ROUND_CONSTANTS[round];
                    }
                    if shake.round_states[index] != expected_state {
                        return violation("keccak-round-state", index);
                    }
                }
            }
        }
    }
    let output_byte = |byte: usize| {
        let permutation = byte / 136;
        let within_rate = byte % 136;
        let word = (permutation * 24 + 23) * 25 + within_rate / 8;
        (shake.round_states[word] >> (8 * (within_rate % 8))) as u8
    };
    for i in 0..HASH_TO_POINT_SAMPLES {
        if trace.hash_to_point.words[i]
            != u16::from_be_bytes([output_byte(2 * i), output_byte(2 * i + 1)])
        {
            return violation("keccak-output", i);
        }
    }
    Ok(())
}

fn violation<T>(family: &'static str, index: usize) -> Result<T, FalconError> {
    Err(FalconError::ConstraintViolation { family, index })
}
falcon_tests! {
mod tests {
    use super::*;
    use crate::piop::spartan::falcon_profiles::n1024_k11::verification_trace;

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    #[test]
    fn fixture_satisfies_the_exact_relation() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        check_exact_constraints(&trace).unwrap();
    }

    #[test]
    fn scalar_counts_include_public_keys_and_exclude_nonlinear_reductions() {
        let counts = FalconConstraintCounts::per_signature();
        assert_eq!(counts.public_input_bindings, 2_858);
        assert_eq!(counts.hash_to_point_linear, 3_646);
        assert_eq!(counts.linear_rows(), 6_504);
        assert_eq!(counts.hash_to_point_quadratic, 1_311);
        assert_eq!(counts.norm_terms, 2_048);
        assert_eq!(counts.norm_round_degree(), 2);
        assert_eq!(counts.product_round_degree(), 3);

    }

    #[test]
    fn source_residual_bound_covers_all_decoded_assignments() {
        // No range/public-input equation is assumed while deriving these.
        let norm_max = N as u128 * (6_144u128.pow(2) + 2_048u128.pow(2)) + ((1u128 << 27) - 1)
            - u128::from(BETA_SQUARED);
        let division_max = 7 * Q as u128 + 12_288;
        let other_bounds = [
            u128::from(BETA_SQUARED), // minimum norm residual is -BETA_SQUARED
            division_max,
            65_535,
            16_383, // selected unsigned-14 C minus bounded14 residue
            16_383, // unsigned-14 H minus a canonical public coefficient
            255,    // public byte
            1,      // Boolean rejection product
        ];
        assert_eq!(division_max, 98_311);
        assert_eq!(norm_max, 43_013_625_445);
        assert_eq!(SOURCE_RESIDUAL_BOUND, norm_max);
        assert!(other_bounds.into_iter().all(|b| b <= SOURCE_RESIDUAL_BOUND));
        assert!(SOURCE_RESIDUAL_BOUND < 1 << 36);
    }

    #[test]
    fn checker_binds_the_public_key_and_ring_quotient() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let mut corrupted = trace.clone();
        corrupted.public_key.h[0] = (corrupted.public_key.h[0] + 1) % Q as u16;
        assert!(check_exact_constraints(&corrupted).is_err());

        let mut corrupted = trace;
        corrupted.ring_quotient[0] ^= 1;
        assert!(matches!(
            check_exact_constraints(&corrupted),
            Err(FalconError::ConstraintViolation {
                family: "native-quotient",
                index: 0,
            })
        ));
    }

    #[test]
    fn corrupted_auxiliary_witness_is_rejected() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        trace.hash_to_point.quotients[17] ^= 1;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "hash-division",
                index: 17
            })
        ));
    }

    #[test]
    fn corrupted_selected_point_is_rejected() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        trace.hash_to_point.point[0] ^= 1;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "hash-stable-compaction",
                index: 0
            })
        ));
    }

    #[test]
    fn corrupted_keccak_parity_witnesses_are_rejected() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        trace.hash_to_point.shake.column_parities[5] ^= 1;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-column-parity",
                index: 320,
            })
        ));
        trace.hash_to_point.shake.column_parities[5] ^= 1;

        trace.hash_to_point.shake.column_parity_quotients[320] ^= 1;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-column-parity" | "keccak-column-quotient-range",
                index: 320,
            })
        ));
        trace.hash_to_point.shake.column_parity_quotients[320] ^= 1;

        trace.hash_to_point.shake.parity_quotients[1_600] ^= 1;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-theta-parity",
                index: 1_600,
            })
        ));
    }

    #[test]
    fn malformed_keccak_auxiliary_shapes_and_ranges_are_rejected() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let parity = trace.hash_to_point.shake.column_parities.pop().unwrap();
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-shape",
                ..
            })
        ));
        trace.hash_to_point.shake.column_parities.push(parity);

        let quotient = trace.hash_to_point.shake.column_parity_quotients[0];
        trace.hash_to_point.shake.column_parity_quotients[0] = 3;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-column-quotient-range",
                index: 0,
            })
        ));
        trace.hash_to_point.shake.column_parity_quotients[0] = quotient;

        trace.hash_to_point.shake.parity_quotients[0] = 2;
        assert!(matches!(
            check_exact_constraints(&trace),
            Err(FalconError::ConstraintViolation {
                family: "keccak-theta-quotient-range",
                index: 0,
            })
        ));
    }
}

}
