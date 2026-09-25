use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N, Q,
};

/// Logical constraint inventory per Falcon signature. These are exact linear
/// or low-degree relation rows, not generic bit-blasted R1CS rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconConstraintCounts {
    /// Public one/message/header bindings.
    pub public_bit_bindings: usize,
    /// Canonical CT `s2 != -2048` range equalities.
    pub s2_canonical: usize,
    /// Sparse theta/rho/pi parity equations, one per Keccak bit.
    pub keccak_linear_bits: usize,
    /// The two quadratic chi identities, one pair per Keccak bit.
    pub keccak_quadratic_rows: usize,
    /// Division, two ranges, accept-AND and prefix recurrence per candidate.
    pub hash_to_point_linear: usize,
    /// Biased centered-lift range equalities.
    pub s1_ranges: usize,
    /// Coefficients in the exact negacyclic Falcon ring equation.
    pub ring_coefficients: usize,
    /// Terms in the degree-2 norm sumcheck.
    pub norm_terms: usize,
    /// Leaves in each stable-compaction product tree after padding.
    pub compaction_leaves: usize,
    /// Selector, selected-prefix, selected-remainder and accept products.
    pub compaction_product_rows: usize,
}

impl FalconConstraintCounts {
    pub const fn per_signature() -> Self {
        Self {
            public_bit_bindings: 1 + 32 * 8 + 8,
            s2_canonical: N,
            keccak_linear_bits: 20 * 24 * 25 * 64,
            keccak_quadratic_rows: 2 * 20 * 24 * 25 * 64,
            // Division/output tie, two ranges, prefix recurrence, plus the
            // initial-prefix and final-count checks.
            hash_to_point_linear: 4 * HASH_TO_POINT_SAMPLES + 2,
            s1_ranges: N,
            ring_coefficients: N,
            norm_terms: 2 * N,
            compaction_leaves: HASH_TO_POINT_SAMPLES.next_power_of_two(),
            compaction_product_rows: 27 * HASH_TO_POINT_SAMPLES,
        }
    }

    /// Rows batched by the single random linear/ideal-check binder.
    pub const fn linear_rows(self) -> usize {
        self.public_bit_bindings
            + self.s2_canonical
            + self.keccak_linear_bits
            + self.hash_to_point_linear
            + self.s1_ranges
            + self.ring_coefficients
    }

    /// Number of variables in the batched coefficient domain.
    pub fn coefficient_vars(layout: &FalconSourceLayout) -> usize {
        N.trailing_zeros() as usize + layout.capacity().trailing_zeros() as usize
    }

    /// The norm polynomial has degree two in every variable.
    pub const fn norm_round_degree(self) -> usize {
        2
    }

    /// The product-forest layer sumchecks have degree three (`eq * L * R`).
    pub const fn product_round_degree(self) -> usize {
        3
    }
}

/// Independent exact-integer checker for a generated trace.  This is the
/// native mirror of the PIOP relation and is intentionally written as the
/// displayed equations, rather than calling the verifier that generated the
/// trace.
pub fn check_exact_constraints(trace: &FalconVerificationTrace) -> Result<(), FalconError> {
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
        if hash.prefix[i + 1]
            != hash.prefix[i]
                .checked_add(u16::from(hash.accepted[i]))
                .unwrap()
        {
            return violation("hash-prefix", i);
        }
    }
    if usize::from(hash.prefix[HASH_TO_POINT_SAMPLES]) < N {
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

    let mut norm = 0u64;
    for i in 0..N {
        let s1 = i64::from(trace.s1[i]);
        if !(-6_144..=6_144).contains(&s1) {
            return violation("s1-centered-range", i);
        }
        if i64::from(hash.point[i]) - trace.convolution[i] - s1 != Q * trace.quotient[i] {
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

fn violation<T>(family: &'static str, index: usize) -> Result<T, FalconError> {
    Err(FalconError::ConstraintViolation { family, index })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::spartan::falcon1024_ct::verification_trace;

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    #[test]
    fn fixture_satisfies_the_exact_relation() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        check_exact_constraints(&trace).unwrap();
        let counts = FalconConstraintCounts::per_signature();
        assert_eq!(counts.linear_rows(), 776_583);
        assert_eq!(counts.norm_round_degree(), 2);
        assert_eq!(counts.product_round_degree(), 3);
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
}
