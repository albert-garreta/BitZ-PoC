// Shared-prime integer messages with derived metadata omitted.
//
// The verifier reconstructs each full round before absorption, preserving the
// prover transcript.
// Only the checked endpoint claims survive verification.

use super::*;
use crate::sumcheck::{
    SumcheckError,
    boundary::RoundBoundaryPolicy,
    proof::{evaluate_polynomial, reconstruct_round_coefficients},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CompactSumcheck<const COEFFS: usize> {
    /// Coefficients [c0, c2, ...]; c1 = claim - 2*c0 - c2 - ... .
    pub(in super::super) round_polynomials: Vec<[F; COEFFS]>,
}

impl<const COEFFS: usize> CompactSumcheck<COEFFS> {
    pub(super) fn from_full<const FULL: usize>(proof: SumcheckProof<F, FULL>) -> Self {
        assert_eq!(FULL, COEFFS + 1);
        Self {
            round_polynomials: proof
                .round_polynomials
                .into_iter()
                .map(|round| std::array::from_fn(|i| round[if i == 0 { 0 } else { i + 1 }]))
                .collect(),
        }
    }
}

/// Reconstruct c1 from the incoming claim and absorb the full coefficient array.
pub(super) fn verify_round_proofs<const FULL: usize, const COMPACT: usize, const K: usize>(
    proofs: [&CompactSumcheck<COMPACT>; K],
    transcript: &mut impl Transcript,
    initial_claims: &[F; K],
    rounds: usize,
    field: &Cfg,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<(Vec<F>, [F; K]), SumcheckError> {
    assert_eq!(FULL, COMPACT + 1);
    if K == 0 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    boundary.validate(rounds)?;
    validate_field_elements(initial_claims, field)?;
    for proof in proofs {
        let actual = proof.round_polynomials.len();
        if actual != rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: rounds,
                actual,
            });
        }
        for coefficients in &proof.round_polynomials {
            validate_field_elements(coefficients, field)?;
        }
    }
    let zero = field.zero();
    let mut claims = *initial_claims;
    let mut point = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let coefficients: [[F; FULL]; K] = std::array::from_fn(|i| {
            reconstruct_round_coefficients(
                &claims[i],
                &proofs[i].round_polynomials[round],
                &zero,
                field,
            )
        });
        for coefficients in &coefficients {
            absorb_field_elements(transcript, coefficients, field);
        }
        boundary.after_round(transcript, round)?;
        let challenge = squeeze_field(transcript, field)?;
        for (coefficients, claim) in coefficients.iter().zip(&mut claims) {
            *claim = evaluate_polynomial(coefficients, &challenge, &zero, field);
        }
        point.push(challenge);
    }
    Ok((point, claims))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct NormClaimRef<'a> {
    pub instance_point: &'a [F],
    pub terminal: &'a [[F; 2]; 2],
    pub slack: F,
    pub point: &'a [F],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct HashToPointRejectionClaimRef<'a> {
    pub point: &'a [F],
    pub terminal: &'a [F; 3],
}

/// Endpoints derived by the prover or verifier for authentication against the
/// original source commitment. Rejection terminals are unweighted row MLEs A,B,C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaimRef<'a> {
    pub modulus: u128,
    pub norm: NormClaimRef<'a>,
    pub h2p_rejection: HashToPointRejectionClaimRef<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct NormClaims {
    pub(in super::super) instance_point: Vec<F>,
    pub(in super::super) terminal: [[F; 2]; 2],
    pub(in super::super) slack: F,
    pub(in super::super) point: Vec<F>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct QuadraticClaims {
    pub(in super::super) terminal: OuterEvaluations<F>,
    pub(in super::super) point: Vec<F>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct HashToPointRejectionClaims {
    pub(in super::super) point: Vec<F>,
    pub(in super::super) terminal: [F; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaims {
    pub(in super::super) modulus: u128,
    pub(in super::super) norm: NormClaims,
    pub(in super::super) h2p_rejection: HashToPointRejectionClaims,
}

impl FalconPiopClaims {
    pub(in super::super) fn as_claim_ref(&self) -> FalconPiopClaimRef<'_> {
        FalconPiopClaimRef {
            modulus: self.modulus,
            norm: NormClaimRef {
                instance_point: &self.norm.instance_point,
                terminal: &self.norm.terminal,
                slack: self.norm.slack,
                point: &self.norm.point,
            },
            h2p_rejection: HashToPointRejectionClaimRef {
                point: &self.h2p_rejection.point,
                terminal: &self.h2p_rejection.terminal,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct NormProof {
    pub(in super::super) instance_nonce: Option<u64>,
    pub(in super::super) claims: [F; 2],
    pub(in super::super) sumchecks: [CompactSumcheck<2>; 2],
    pub(in super::super) terminal: [[F; 2]; 2],
    pub(in super::super) grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct QuadraticRelationProof {
    pub(in super::super) point_nonce: Option<u64>,
    pub(in super::super) sumcheck: CompactSumcheck<3>,
    pub(in super::super) terminal: OuterEvaluations<F>,
    pub(in super::super) grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct HashToPointRejectionProof {
    pub(in super::super) rows: QuadraticRelationProof,
}

/// Only messages and grinding nonces are stored; challenges are reconstructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopProof {
    pub(in super::super) norm: NormProof,
    pub(in super::super) h2p_rejection: HashToPointRejectionProof,
}

impl FalconPiopProof {
    pub(in super::super) fn visit_grinding_nonces(&self, mut visit: impl FnMut(&'static str, u64)) {
        for &nonce in self
            .norm
            .instance_nonce
            .iter()
            .chain(&self.norm.grinding_nonces)
        {
            visit("prime_norm", nonce);
        }
        for &nonce in self
            .h2p_rejection
            .rows
            .point_nonce
            .iter()
            .chain(&self.h2p_rejection.rows.grinding_nonces)
        {
            visit("prime_h2p_rows", nonce);
        }
    }

    /// Canonical scalar payload, excluding container framing.
    pub(in super::super) fn payload_size_bytes(&self) -> usize {
        let norm_rounds: usize = self
            .norm
            .sumchecks
            .iter()
            .map(|p| p.round_polynomials.len())
            .sum();
        let rejection_rounds = self.h2p_rejection.rows.sumcheck.round_polynomials.len();
        let mut nonce_count = 0;
        self.visit_grinding_nonces(|_, _| nonce_count += 1);
        // Norm: two initial claims and four terminals. Rejection: three terminals.
        16 * (9 + 2 * norm_rounds + 3 * rejection_rounds) + 8 * nonce_count
    }
}

pub(in super::super) fn verify_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopClaims, FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid shared PIOP security target"));
    }
    validate_shared_field(target_bits, field)?;
    bind_shared_header(transcript, layout, target_bits, field);
    let norm = verify_norm(transcript, layout, &proof.norm, target_bits, field)?;
    let h2p_rejection =
        verify_rejection(transcript, layout, &proof.h2p_rejection, target_bits, field)?;
    Ok(FalconPiopClaims {
        modulus: field.modulus_u128(),
        norm,
        h2p_rejection,
    })
}
