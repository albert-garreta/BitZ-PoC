// Integer Falcon reductions over the prime already selected by the native ring proof.
// Every terminal claim is authenticated against the original bit source by opening.rs.
use super::COEFFICIENT_LOG;
use field::RingOps;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, N,
    hash_to_point_selection::{REJECTION_ROW_LOG, REJECTION_ROWS, REJECTION_STRIDE},
};
use crate::{
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::{eq_eval, eq_table},
        squeeze_field,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{InitialClaims, prove_batched_inner_sumcheck},
        outer::{OuterClaim, OuterEvaluations, OuterRows, prove_outer_sumcheck},
        proof::validate_field_elements,
    },
    transcript::traits::Transcript,
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

#[path = "piop_shared.rs"]
mod shared;
use shared::{CompactSumcheck, verify_round_proofs};
pub(super) use shared::{
    FalconPiopClaimRef, FalconPiopClaims, FalconPiopProof, HashToPointRejectionClaims,
    HashToPointRejectionProof, NormClaims, NormProof, QuadraticClaims, QuadraticRelationProof,
    verify_falcon_piop_in_field,
};

/// Prime-field numerators, excluding the native ring and binary-field stages.
/// The eight binder endpoints are five norm claims and three rejection row claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecurityNumerators {
    pub norm_instances: usize,
    pub norm_rounds: usize,
    pub outer_point: usize,
    pub cubic_rounds: usize,
    pub linear: usize,
    pub binding: usize,
}

impl FalconSecurityNumerators {
    pub const fn for_layout(layout: &FalconSourceLayout) -> Self {
        let d = layout.capacity().trailing_zeros() as usize;
        let rounds = REJECTION_ROW_LOG + d;
        Self {
            norm_instances: d,
            norm_rounds: 4 * (COEFFICIENT_LOG + d),
            outer_point: rounds,
            cubic_rounds: 3 * rounds,
            linear: layout.linear_stride().ilog2() as usize + d + layout.batch() + 8,
            binding: 2 * (layout.signature_stride().ilog2() as usize + d),
        }
    }
}

/// Each prime-field stage receives at most 2^-(target+5) work-normalized error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecuritySchedule {
    pub norm_instance_bits: u32,
    pub outer_point_bits: u32,
    pub quadratic_round_bits: u32,
    pub cubic_round_bits: u32,
    pub linear_point_bits: u32,
    pub binding_round_bits: u32,
}

impl FalconSecuritySchedule {
    pub const fn for_layout(target_bits: usize, layout: &FalconSourceLayout) -> Option<Self> {
        let floor = match target_bits {
            100 => 114,
            128 => 125,
            _ => return None,
        };
        let n = FalconSecurityNumerators::for_layout(layout);
        Some(Self {
            norm_instance_bits: stage_grinding_bits(n.norm_instances, target_bits, floor),
            quadratic_round_bits: stage_grinding_bits(n.norm_rounds, target_bits, floor),
            outer_point_bits: stage_grinding_bits(n.outer_point, target_bits, floor),
            cubic_round_bits: stage_grinding_bits(n.cubic_rounds, target_bits, floor),
            linear_point_bits: stage_grinding_bits(n.linear, target_bits, floor),
            binding_round_bits: stage_grinding_bits(n.binding, target_bits, floor),
        })
    }
}

const fn stage_grinding_bits(numerator: usize, target_bits: usize, prime_floor_bits: u32) -> u32 {
    if numerator == 0 {
        return 0;
    }
    let ceil_log = usize::BITS - (numerator - 1).leading_zeros();
    (ceil_log + target_bits as u32 + 5).saturating_sub(prime_floor_bits)
}

struct NormInstanceGrinding;
impl GrindingDomain for NormInstanceGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/norm-instances/v3";
}
struct QuadraticGrinding;
impl GrindingDomain for QuadraticGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/degree2/v1";
}
struct OuterPointGrinding;
impl GrindingDomain for OuterPointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/h2p-rejection/grinding/row-point/v1";
}
struct CubicGrinding;
impl GrindingDomain for CubicGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/h2p-rejection/grinding/row-round/v1";
}

/// The caller fixes the source commitment before the existing ring proof samples p.
pub(super) fn prove_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<(FalconPiopProof, FalconPiopClaims), FalconError> {
    validate_inputs(layout, traces, target_bits)?;
    validate_shared_field(target_bits, field)?;
    bind_shared_header(transcript, layout, target_bits, field);
    let (norm, norm_claims) = prove_norm(transcript, layout, traces, target_bits, field)?;
    let (h2p_rejection, rejection_claims) =
        prove_rejection(transcript, layout, traces, target_bits, field)?;
    Ok((
        FalconPiopProof {
            norm,
            h2p_rejection,
        },
        FalconPiopClaims {
            modulus: field.modulus_u128(),
            norm: norm_claims,
            h2p_rejection: rejection_claims,
        },
    ))
}
fn validate_inputs(
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
) -> Result<(), FalconError> {
    if traces.len() != layout.batch() || !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid batch or security target"));
    }
    Ok(())
}

fn validate_shared_field(target_bits: usize, field: &Cfg) -> Result<(), FalconError> {
    let (prime_min, prime_max) = super::shared_ring::prime_bounds(target_bits)?;
    let modulus = field.modulus_u128();
    if !(prime_min..=prime_max).contains(&modulus)
        || modulus <= super::constraints::SOURCE_RESIDUAL_BOUND
    {
        return Err(piop("invalid shared-prime integer context"));
    }
    Ok(())
}

fn bind_shared_header(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    target_bits: usize,
    field: &Cfg,
) {
    transcript.absorb_slice(b"bitz/falcon/piop/shared-prime/public-selection/v7");
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&(layout.live_bits() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.occupied_bits() as u64).to_le_bytes());
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:norm")]
fn prove_norm(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<(NormProof, NormClaims), FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/norm/v3");
    let instance_nonce = if target_bits == 128 && layout.capacity() > 1 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<NormInstanceGrinding>::new(0),
                security_schedule(layout, target_bits)?.norm_instance_bits,
            )
            .map_err(|error| piop(error.to_string()))?,
        )
    } else {
        None
    };
    let instance_point = sample_point(
        transcript,
        layout.capacity().trailing_zeros() as usize,
        field,
    )?;
    let instance_weights =
        eq_table(&instance_point, field).map_err(|error| piop(error.to_string()))?;
    let len = N * layout.capacity();
    let zero = field.zero();
    let mut s1 = vec![zero; len];
    let mut s2 = vec![zero; len];
    let mut weighted_s1 = vec![zero; len];
    let mut weighted_s2 = vec![zero; len];
    let contributions: Vec<[F; 3]> = crate::utils::cfg_chunks_mut!(s1, N)
        .zip(crate::utils::cfg_chunks_mut!(s2, N))
        .zip(crate::utils::cfg_chunks_mut!(weighted_s1, N))
        .zip(crate::utils::cfg_chunks_mut!(weighted_s2, N))
        .zip(crate::utils::cfg_iter!(traces))
        .zip(crate::utils::cfg_iter!(&instance_weights[..layout.batch()]))
        .map(
            |(((((s1, s2), weighted_s1), weighted_s2), trace), weight)| {
                let mut sums = [0u64; 2];
                for i in 0..N {
                    let a = i64::from(trace.s1[i]);
                    let b = i64::from(trace.signature.s2[i]);
                    s1[i] = signed(i128::from(a), field);
                    s2[i] = signed(i128::from(b), field);
                    weighted_s1[i] = field.mul(weight, &s1[i]);
                    weighted_s2[i] = field.mul(weight, &s2[i]);
                    // Both coefficients are i16, so each integer sum is <=2^40.
                    // Apply the common signature weight once after summation.
                    sums[0] += (a * a) as u64;
                    sums[1] += (b * b) as u64;
                }
                [sums[0], sums[1], trace.norm_slack]
                    .map(|value| field.mul(weight, &unsigned(u128::from(value), field)))
            },
        )
        .collect();
    let [claim1, claim2, slack] = contributions.into_iter().fold([zero; 3], |sum, value| {
        std::array::from_fn(|i| field.add(&sum[i], &value[i]))
    });
    let claims = [claim1, claim2];
    let beta = unsigned(BETA_SQUARED as u128, field);
    let instance_sum = instance_weights[..layout.batch()]
        .iter()
        .fold(zero, |sum, weight| field.add(&sum, weight));
    let expected = field.mul(&instance_sum, &beta);
    if field.add(&field.add(&claims[0], &claims[1]), &slack) != expected {
        return Err(piop("invalid norm witness"));
    }

    absorb_field_elements(transcript, &[claims[0], claims[1], slack], field);
    let output = if target_bits == 128 {
        let mut boundary = ProverGrindingRoundBoundary::<QuadraticGrinding>::with_round_offset(
            security_schedule(layout, target_bits)?.quadratic_round_bits,
            0,
        );
        let out = prove_batched_inner_sumcheck(
            field,
            transcript,
            &claims,
            [s1, s2],
            [weighted_s1, weighted_s2],
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        (out, boundary.into_nonces())
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let out = prove_batched_inner_sumcheck(
            field,
            transcript,
            InitialClaims::Known(&claims),
            [s1, s2],
            [weighted_s1, weighted_s2],
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        (out, Vec::new())
    };
    let (output, grinding_nonces) = output;
    absorb_field_elements(transcript, &output.terminal_evaluations.concat(), field);
    let terminal = output.terminal_evaluations;
    Ok((
        NormProof {
            instance_nonce,
            claims,
            terminal,
            grinding_nonces,
            sumchecks: output.proofs.map(CompactSumcheck::from_full),
        },
        NormClaims {
            instance_point,
            terminal,
            slack,
            point: output.point,
        },
    ))
}

fn verify_norm(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &NormProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<NormClaims, FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/norm/v3");
    match (
        target_bits == 128 && layout.capacity() > 1,
        proof.instance_nonce,
    ) {
        (true, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<NormInstanceGrinding>::new(0),
            security_schedule(layout, target_bits)?.norm_instance_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string()))?,
        (false, None) => {}
        _ => return Err(piop("invalid norm-instance grinding nonce")),
    }
    let instance_point = sample_point(
        transcript,
        layout.capacity().trailing_zeros() as usize,
        field,
    )?;
    validate_field_elements(&proof.claims, field).map_err(|error| piop(error.to_string()))?;
    validate_field_elements(&proof.terminal.concat(), field)
        .map_err(|error| piop(error.to_string()))?;
    if target_bits == 100 && !proof.grinding_nonces.is_empty() {
        return Err(piop("unexpected norm grinding nonces"));
    }
    let instance_weights =
        eq_table(&instance_point, field).map_err(|error| piop(error.to_string()))?;
    let instance_sum = instance_weights[..layout.batch()]
        .iter()
        .fold(field.zero(), |sum, weight| field.add(&sum, weight));
    let beta = unsigned(BETA_SQUARED as u128, field);
    let expected = field.mul(&instance_sum, &beta);
    let slack = field.sub(&expected, &field.add(&proof.claims[0], &proof.claims[1]));
    absorb_field_elements(
        transcript,
        &[proof.claims[0], proof.claims[1], slack],
        field,
    );
    let rounds = COEFFICIENT_LOG + layout.capacity().trailing_zeros() as usize;
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<QuadraticGrinding>::new(
            security_schedule(layout, target_bits)?.quadratic_round_bits,
            &proof.grinding_nonces,
        );
        verify_round_proofs::<3, 2, 2>(
            proof.sumchecks.each_ref(),
            transcript,
            &proof.claims,
            rounds,
            field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        verify_round_proofs::<3, 2, 2>(
            proof.sumchecks.each_ref(),
            transcript,
            &proof.claims,
            rounds,
            field,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    for (claim, terminal) in final_claims.iter().zip(proof.terminal.iter()) {
        if *claim != field.mul(&terminal[0], &terminal[1]) {
            return Err(piop("norm terminal identity failed"));
        }
    }
    absorb_field_elements(transcript, &proof.terminal.concat(), field);
    Ok(NormClaims {
        instance_point,
        point,
        slack,
        terminal: proof.terminal,
    })
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:hash_to_point_rejection")]
fn prove_rejection(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<(HashToPointRejectionProof, HashToPointRejectionClaims), FalconError> {
    transcript.absorb_slice(b"bitz/falcon/h2p-rejection/rows/v1");
    let rows = RejectionRows {
        layout,
        traces,
        field,
    };
    let (rows, claims) = prove_quadratic(
        transcript,
        rows,
        rejection_row_rounds(layout),
        security_schedule(layout, target_bits)?,
        field,
    )?;
    Ok((
        HashToPointRejectionProof { rows },
        HashToPointRejectionClaims {
            point: claims.point,
            terminal: [claims.terminal.ax, claims.terminal.bx, claims.terminal.cx],
        },
    ))
}

fn verify_rejection(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &HashToPointRejectionProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<HashToPointRejectionClaims, FalconError> {
    transcript.absorb_slice(b"bitz/falcon/h2p-rejection/rows/v1");
    let claims = verify_quadratic(
        transcript,
        rejection_row_rounds(layout),
        &proof.rows,
        security_schedule(layout, target_bits)?,
        field,
    )?;
    Ok(HashToPointRejectionClaims {
        point: claims.point,
        terminal: [claims.terminal.ax, claims.terminal.bx, claims.terminal.cx],
    })
}

/// The only nonlinear HashToPoint relation is q_bit2*q_bit0=e. Each operand
/// reads the same bit that source packing commits; all padded rows are zero.
struct RejectionRows<'a> {
    layout: &'a FalconSourceLayout,
    traces: &'a [FalconVerificationTrace],
    field: &'a Cfg,
}

impl RejectionRows<'_> {
    #[inline(always)]
    fn quotient(&self, row: usize) -> Option<u8> {
        let candidate = row % REJECTION_STRIDE;
        if candidate >= REJECTION_ROWS {
            return None;
        }
        self.traces
            .get(row / REJECTION_STRIDE)
            .map(|trace| trace.hash_to_point.quotients[candidate])
    }

    #[inline(always)]
    fn bit(&self, value: bool) -> F {
        if value {
            self.field.one()
        } else {
            self.field.zero()
        }
    }
}

impl OuterRows for RejectionRows<'_> {
    type AB = F;
    type C = F;
    fn dimensions(&self) -> (usize, usize, usize) {
        let len = REJECTION_STRIDE * self.layout.capacity();
        (len, len, len)
    }
    #[inline(always)]
    fn a(&self, row: usize) -> F {
        self.bit(self.quotient(row).is_some_and(|q| q & 4 != 0))
    }
    #[inline(always)]
    fn b(&self, row: usize) -> F {
        self.bit(self.quotient(row).is_some_and(|q| q & 1 != 0))
    }
    #[inline(always)]
    fn c(&self, row: usize) -> F {
        self.bit(self.quotient(row).is_some_and(|q| q & 5 == 5))
    }
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:rejection_sumcheck")]
fn prove_quadratic(
    transcript: &mut impl Transcript,
    rows: impl OuterRows<AB = F, C = F>,
    rounds: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(QuadraticRelationProof, QuadraticClaims), FalconError> {
    let point_nonce = if security.outer_point_bits != 0 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<OuterPointGrinding>::new(0),
                security.outer_point_bits,
            )
            .map_err(|error| piop(error.to_string()))?,
        )
    } else {
        None
    };
    let tau = sample_point(transcript, rounds, field)?;
    let output = if security.cubic_round_bits != 0 {
        let mut boundary = ProverGrindingRoundBoundary::<CubicGrinding>::with_round_offset(
            security.cubic_round_bits,
            0,
        );
        let out = prove_outer_sumcheck(
            field,
            transcript,
            OuterClaim::RowwiseZero,
            &tau,
            rows,
            None,
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        (out, boundary.into_nonces())
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let out = prove_outer_sumcheck(
            field,
            transcript,
            OuterClaim::RowwiseZero,
            &tau,
            rows,
            None,
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        (out, Vec::new())
    };
    let (output, grinding_nonces) = output;
    let terminal = output.evaluations;
    Ok((
        QuadraticRelationProof {
            point_nonce,
            sumcheck: CompactSumcheck::from_full(output.proof),
            terminal,
            grinding_nonces,
        },
        QuadraticClaims {
            terminal,
            point: output.point,
        },
    ))
}

fn verify_quadratic(
    transcript: &mut impl Transcript,
    rounds: usize,
    proof: &QuadraticRelationProof,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<QuadraticClaims, FalconError> {
    if security.cubic_round_bits == 0 && !proof.grinding_nonces.is_empty() {
        return Err(piop("unexpected rejection-row grinding nonces"));
    }
    match (security.outer_point_bits != 0, proof.point_nonce) {
        (true, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<OuterPointGrinding>::new(0),
            security.outer_point_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string()))?,
        (false, None) => {}
        _ => return Err(piop("invalid outer-point grinding nonce")),
    }
    let tau = sample_point(transcript, rounds, field)?;
    let zero = field.zero();
    validate_field_elements(
        &[proof.terminal.ax, proof.terminal.bx, proof.terminal.cx],
        field,
    )
    .map_err(|error| piop(error.to_string()))?;
    let (point, [final_claim]) = if security.cubic_round_bits != 0 {
        let mut boundary = VerifierGrindingRoundBoundary::<CubicGrinding>::new(
            security.cubic_round_bits,
            &proof.grinding_nonces,
        );
        verify_round_proofs::<4, 3, 1>(
            [&proof.sumcheck],
            transcript,
            &[zero],
            rounds,
            field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        verify_round_proofs::<4, 3, 1>(
            [&proof.sumcheck],
            transcript,
            &[zero],
            rounds,
            field,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    let equality = eq_eval(&tau, &point, field).map_err(|error| piop(error.to_string()))?;
    let residual = field.sub(
        &field.mul(&proof.terminal.ax, &proof.terminal.bx),
        &proof.terminal.cx,
    );
    if final_claim != field.mul(&equality, &residual) {
        return Err(piop("quadratic terminal identity failed"));
    }
    absorb_field_elements(
        transcript,
        &[proof.terminal.ax, proof.terminal.bx, proof.terminal.cx],
        field,
    );
    Ok(QuadraticClaims {
        terminal: proof.terminal,
        point,
    })
}

fn sample_point(
    transcript: &mut impl Transcript,
    rounds: usize,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    (0..rounds).map(|_| squeeze(transcript, field)).collect()
}

fn squeeze(transcript: &mut impl Transcript, field: &Cfg) -> Result<F, FalconError> {
    squeeze_field(transcript, field).map_err(|error| piop(error.to_string()))
}

fn unsigned(value: u128, field: &Cfg) -> F {
    F::from_with_cfg(value, field)
}

fn signed(value: i128, field: &Cfg) -> F {
    let magnitude = unsigned(value.unsigned_abs(), field);
    if value.is_negative() {
        field.sub(&field.zero(), &magnitude)
    } else {
        magnitude
    }
}

pub(super) fn rejection_row_rounds(layout: &FalconSourceLayout) -> usize {
    REJECTION_ROW_LOG + layout.capacity().trailing_zeros() as usize
}

pub(super) fn security_schedule(
    layout: &FalconSourceLayout,
    target_bits: usize,
) -> Result<FalconSecuritySchedule, FalconError> {
    FalconSecuritySchedule::for_layout(target_bits, layout)
        .ok_or_else(|| piop("unsupported Falcon security target"))
}
fn piop(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}
falcon_tests! {
mod tests {
    use super::super::verification_trace;
    use super::*;
    use crate::transcript::Blake3Transcript;
    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] = include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] = include_bytes!("fixtures/signature_ct.bin");
    fn test_field(target: usize) -> Cfg {
        let (minimum, maximum) = super::super::shared_ring::prime_bounds(target).unwrap();
        crate::prime_sampling::sample_prime_context(&mut Blake3Transcript::new(), minimum, maximum, 128).unwrap()
    }
    #[test]
    fn shared_prime_profiles_support_their_bridge_shapes_and_integer_lifts() {
        use super::super::shared_ring::{prime_bounds, prime_floor_bits};

        assert_eq!(
            prime_bounds(100).unwrap(),
            (1u128 << 114, (1u128 << 115) - (1u128 << 102) - 1)
        );
        assert_eq!(
            prime_bounds(128).unwrap(),
            (1u128 << 125, (1u128 << 126) - 1)
        );
        assert_eq!(prime_floor_bits(100).unwrap(), 114);
        assert_eq!(prime_floor_bits(128).unwrap(), 125);
        assert!(prime_bounds(127).is_err());
        assert!(prime_floor_bits(127).is_err());
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let shape = crate::bitz::Shape::new(layout.row_vars(), layout.col_vars()).unwrap();
            let rows = shape.rows() as u128;
            for target in [100, 128] {
                let (prime_min, prime_max) = prime_bounds(target).unwrap();
                assert!(super::super::constraints::SOURCE_RESIDUAL_BOUND < prime_min);
                let lift_bound = 11 * layout.source_bits() as u128 * 12_288u128.pow(2);
                assert!(2 * lift_bound < prime_min);
                if target == 100 {
                    assert!(shape.supports_modulus_bound(prime_max));
                    // The uncapped 115-bit interval cannot satisfy the strict gate.
                    assert!(!shape.supports_modulus_bound((1u128 << 115) - 1));
                } else {
                    // The 126-bit family requires two bounded limbs, with the
                    // extra forest coordinate selecting the unmultiplied limb.
                    assert!(!shape.supports_modulus_bound(prime_max));
                    let width = 126 - layout.row_vars();
                    let limb_bound = 1u128 << width;
                    assert!(shape.supports_modulus_bound(limb_bound));
                    let low_sum = rows * (limb_bound - 1);
                    let high_sum = rows * ((prime_max - 1) >> width);
                    assert!(low_sum < 1u128 << 126);
                    assert!(high_sum < 1u128 << 26);
                    assert!(low_sum < u128::MAX && high_sum < u128::MAX);
                }
            }
        }
    }
    #[test]
    fn norm_verifier_rejects_budget_transfer_between_instances() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let layout = FalconSourceLayout::new(2).unwrap();
        // Both coefficients are individually in range, but signature zero
        // exceeds the norm bound. Signature one's slack makes the old global
        // sum exactly 2*B, despite the two nonzero per-signature residuals.
        let over_norm = 2 * 6_144u64.pow(2);
        let slack_words = [0, 2 * BETA_SQUARED - over_norm];
        assert!(over_norm > BETA_SQUARED);
        assert!(slack_words[1] < 1 << 27);
        assert_eq!(
            over_norm + slack_words.iter().sum::<u64>(),
            2 * BETA_SQUARED
        );
        let mut source = vec![0u8; 2 * N * 2];
        source[..2].copy_from_slice(&6_144u16.to_le_bytes());
        source[2..4].copy_from_slice(&6_144u16.to_le_bytes());
        source.extend(slack_words.into_iter().flat_map(u64::to_le_bytes));
        let commitment = blake3::hash(&source);
        let fresh_transcript = || {
            let mut transcript = Blake3Transcript::new();
            transcript.absorb_slice(commitment.as_bytes());
            transcript
        };
        let mut prover = fresh_transcript();
        prover.absorb_slice(b"bitz/falcon1024-ct/norm/v3");
        let instance_point = sample_point(&mut prover, 1, &field).unwrap();
        let weights = eq_table(&instance_point, &field).unwrap();
        assert_ne!(weights[0], weights[1]);
        let mut s1 = vec![field.zero(); 2 * N];
        s1[..2].fill(unsigned(6_144, &field));
        let weighted: Vec<_> = s1
            .iter()
            .enumerate()
            .map(|(i, value)| field.mul(&weights[i / N], value))
            .collect();
        let claims = [
            field.mul(&weights[0], &unsigned(over_norm.into(), &field)),
            field.zero(),
        ];
        let slack = field.mul(&weights[1], &unsigned(slack_words[1].into(), &field));
        let derived_slack = field.sub(
            &field.mul(
                &field.add(&weights[0], &weights[1]),
                &unsigned(BETA_SQUARED.into(), &field),
            ),
            &field.add(&claims[0], &claims[1]),
        );
        assert_ne!(slack, derived_slack);
        absorb_field_elements(&mut prover, &[claims[0], claims[1], slack], &field);
        // Construct actual valid sumchecks, bypassing only the honest prover's
        // early norm-budget check. Rejection below is by the protocol verifier.
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let output = prove_batched_inner_sumcheck(
            &field,
            &mut prover,
            &claims,
            [s1, vec![field.zero(); 2 * N]],
            [weighted, vec![field.zero(); 2 * N]],
            &mut boundary,
        )
        .unwrap();
        let proof = NormProof {
            instance_nonce: None,
            claims,
            sumchecks: output.proofs.map(CompactSumcheck::from_full),
            terminal: output.terminal_evaluations,
            grinding_nonces: Vec::new(),
        };
        assert!(verify_norm(&mut fresh_transcript(), &layout, &proof, 100, &field).is_err());
    }
    #[test]
    fn norm_roundtrips_padded_batch_and_binds_instance_challenge() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let mut traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        // Distinct norm tables make weighted and unweighted endpoints differ.
        let old = i64::from(traces[1].s1[0]);
        traces[1].s1[0] = 0;
        traces[1].norm_slack += (old * old) as u64;
        let layout = FalconSourceLayout::new(3).unwrap();
        for target in [100, 128] {
            let (proof, claims) = prove_norm(
                &mut Blake3Transcript::new(),
                &layout,
                &traces,
                target,
                &field,
            )
            .unwrap();
            assert_eq!(proof.instance_nonce.is_some(), target == 128);
            assert_ne!(proof.terminal[0][0], proof.terminal[0][1]);
            let weights = eq_table(&claims.instance_point, &field).unwrap();
            let mut reference = [field.zero(); 3];
            for (trace, weight) in traces.iter().zip(&weights) {
                for j in 0..N {
                    for (a, value) in [trace.s1[j], trace.signature.s2[j]].into_iter().enumerate() {
                        let value = signed(value.into(), &field);
                        reference[a] = field.add(
                            &reference[a],
                            &field.mul(weight, &field.mul(&value, &value)),
                        );
                    }
                }
                reference[2] = field.add(
                    &reference[2],
                    &field.mul(weight, &unsigned(trace.norm_slack.into(), &field)),
                );
            }
            assert_eq!(reference, [proof.claims[0], proof.claims[1], claims.slack]);
            let restored = verify_norm(
                &mut Blake3Transcript::new(),
                &layout,
                &proof,
                target,
                &field,
            )
            .unwrap();
            assert_eq!(claims, restored);
            let mut bad = proof.clone();
            bad.terminal[0][0] = bad.terminal[0][1];
            assert!(
                verify_norm(&mut Blake3Transcript::new(), &layout, &bad, target, &field).is_err()
            );
            let mut bad = proof;
            bad.instance_nonce = if target == 128 { None } else { Some(0) };
            assert!(
                verify_norm(&mut Blake3Transcript::new(), &layout, &bad, target, &field).is_err()
            );
        }
    }
    struct RecordingTranscript {
        inner: Blake3Transcript,
        absorbed: Vec<Vec<u8>>,
    }

    impl RecordingTranscript {
        fn new() -> Self { Self { inner: Blake3Transcript::new(), absorbed: Vec::new() } }
    }

    impl Transcript for RecordingTranscript {
        fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
            self.inner.get_challenge()
        }
        fn begin_sampling(&mut self) { self.inner.begin_sampling(); }
        fn fill_sampling_bytes(&mut self, output: &mut [u8]) { self.inner.fill_sampling_bytes(output); }
        fn absorb_inner(&mut self, bytes: &[u8]) {
            self.absorbed.push(bytes.to_vec());
            self.inner.absorb_inner(bytes);
        }
    }

    #[test]
    fn rejection_piop_matches_transcripts_and_uses_the_supplied_prime() {
        for (batch, target) in [(1, 100), (3, 100), (1, 128)] {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); batch];
            let field = test_field(target);
            let mut prover = RecordingTranscript::new();
            let (proof, claims) = prove_falcon_piop_in_field(&mut prover, &layout, &traces, target, &field).unwrap();
            let mut verifier = RecordingTranscript::new();
            let restored = verify_falcon_piop_in_field(&mut verifier, &layout, &proof, target, &field).unwrap();
            assert_eq!(claims, restored);
            assert_eq!(prover.absorbed, verifier.absorbed);
            assert_eq!(prover.get_challenge::<u128>(), verifier.get_challenge::<u128>());
            assert!(!prover.absorbed.iter().any(|bytes| bytes == b"bitz/shared-prime-sampling/v1"));
            assert_eq!(prover.absorbed.iter().filter(|bytes| bytes.as_slice() == b"bitz/falcon/piop/shared-prime/public-selection/v7").count(), 1);
            let d = layout.capacity().ilog2() as usize;
            assert_eq!(proof.h2p_rejection.rows.sumcheck.round_polynomials.len(), REJECTION_ROW_LOG + d);
            assert_eq!(proof.norm.sumchecks[0].round_polynomials.len(), COEFFICIENT_LOG + d);
            let mut nonces = 0;
            proof.visit_grinding_nonces(|category, _| {
                assert!(matches!(category, "prime_norm" | "prime_h2p_rows"));
                nonces += 1;
            });
            let fields = 9 + 4 * (COEFFICIENT_LOG + d) + 3 * (REJECTION_ROW_LOG + d);
            assert_eq!(proof.payload_size_bytes(), 16 * fields + 8 * nonces);
        }
        let small = field::FpCtx::from_prime_u128(7);
        assert!(validate_shared_field(100, &small).is_err());
        let field = test_field(100);
        assert!(validate_shared_field(128, &field).is_err());
        assert!(validate_shared_field(127, &field).is_err());
    }

    #[test]
    fn rejection_rows_match_packed_source_and_zero_padding() {
        let layout = FalconSourceLayout::new(3).unwrap();
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let mut source = super::super::FalconSourceWitness::from_traces(
            layout, &vec![MESSAGE.as_slice(); 3], &vec![SIGNATURE.as_slice(); 3], &traces,
        ).unwrap();
        let field = test_field(100);
        let rows = RejectionRows { layout: &layout, traces: &traces, field: &field };
        let offsets = layout.offsets();
        let decode = |bit| if source.bit(bit) { field.one() } else { field.zero() };
        for instance in 0..layout.capacity() {
            for j in 0..REJECTION_STRIDE {
                let row = instance * REJECTION_STRIDE + j;
                let expected = if instance < layout.batch() && j < REJECTION_ROWS {
                    let base = instance * layout.signature_stride();
                    [decode(base + offsets.hash_quotients + 3*j + 2), decode(base + offsets.hash_quotients + 3*j), decode(base + offsets.hash_accept_ands + j)]
                } else { [field.zero(); 3] };
                assert_eq!([rows.a(row), rows.b(row), rows.c(row)], expected);
                assert_eq!(field.mul(&expected[0], &expected[1]), expected[2]);
            }
        }
        // A changed committed e bit violates the independently decoded row.
        for j in [0, REJECTION_ROWS / 2, REJECTION_ROWS - 1] {
            source.flip_bit(offsets.hash_accept_ands + j);
            let c = if source.bit(offsets.hash_accept_ands + j) { field.one() } else { field.zero() };
            assert_ne!(field.mul(&rows.a(j), &rows.b(j)), c);
            source.flip_bit(offsets.hash_accept_ands + j);
        }
    }

    #[test]
    fn rejection_rows_cover_all_quotient_bits_without_trusting_acceptance_flags() {
        let layout = FalconSourceLayout::new(1).unwrap();
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let field = test_field(100);
        for quotient in 0..8 {
            trace.hash_to_point.quotients.fill(quotient);
            for accepted in [false, true] {
                trace.hash_to_point.accepted.fill(accepted);
                let rows = RejectionRows { layout: &layout, traces: std::slice::from_ref(&trace), field: &field };
                let bit = |value| if value { field.one() } else { field.zero() };
                let expected = [bit(quotient & 4 != 0), bit(quotient & 1 != 0), bit(quotient & 5 == 5)];
                for j in 0..REJECTION_ROWS {
                    assert_eq!([rows.a(j), rows.b(j), rows.c(j)], expected);
                }
            }
        }
    }

    #[test]
    fn rejection_rows_preserve_dense_proof_messages_and_transcript() {
        let layout = FalconSourceLayout::new(3).unwrap();
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        for target in [100, 128] {
            let field = test_field(target);
            let rows = RejectionRows { layout: &layout, traces: &traces, field: &field };
            let len = REJECTION_STRIDE * layout.capacity();
            let mut dense: [Vec<F>; 3] = std::array::from_fn(|_| vec![field.zero(); len]);
            for (instance, trace) in traces.iter().enumerate() {
                for (j, &q) in trace.hash_to_point.quotients.iter().enumerate() {
                    let row = instance * REJECTION_STRIDE + j;
                    dense[0][row] = unsigned(u128::from((q >> 2) & 1), &field);
                    dense[1][row] = unsigned(u128::from(q & 1), &field);
                    dense[2][row] = field.mul(&dense[0][row], &dense[1][row]);
                }
            }
            let security = security_schedule(&layout, target).unwrap();
            let mut packed_transcript = RecordingTranscript::new();
            let packed = prove_quadratic(&mut packed_transcript, rows, rejection_row_rounds(&layout), security, &field).unwrap();
            let mut dense_transcript = RecordingTranscript::new();
            let reference = prove_quadratic(
                &mut dense_transcript,
                crate::sumcheck::outer::OuterSlices { ax: &dense[0], bx: &dense[1], cx: &dense[2] },
                rejection_row_rounds(&layout), security, &field,
            ).unwrap();
            assert_eq!(packed, reference);
            assert_eq!(packed_transcript.absorbed, dense_transcript.absorbed);
            assert_eq!(packed_transcript.get_challenge::<u128>(), dense_transcript.get_challenge::<u128>());
        }
    }

    #[test]
    fn rejection_piop_rejects_changed_messages_shapes_and_nonce_schedules() {
        let layout = FalconSourceLayout::new(1).unwrap();
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        for target in [100, 128] {
            let field = test_field(target);
            let (proof, _) = prove_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, std::slice::from_ref(&trace), target, &field).unwrap();
            let reject = |bad: &FalconPiopProof| assert!(verify_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, bad, target, &field).is_err());
            for endpoint in 0..3 {
                let mut bad = proof.clone();
                let terminal = &mut bad.h2p_rejection.rows.terminal;
                let value = match endpoint { 0 => &mut terminal.ax, 1 => &mut terminal.bx, _ => &mut terminal.cx };
                *value = field.add(value, &field.one());
                reject(&bad);
            }
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.point_nonce = if bad.h2p_rejection.rows.point_nonce.is_some() { None } else { Some(0) };
            reject(&bad);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.grinding_nonces.push(0);
            reject(&bad);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.sumcheck.round_polynomials.pop();
            reject(&bad);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.sumcheck.round_polynomials.push([field.zero(); 3]);
            reject(&bad);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.sumcheck.round_polynomials[0][1] = field.add(&bad.h2p_rejection.rows.sumcheck.round_polynomials[0][1], &field.one());
            reject(&bad);
            let wider = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
            let noncanonical = unsigned(field.modulus_u128(), &wider);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.sumcheck.round_polynomials[0][0] = noncanonical;
            reject(&bad);
            let mut bad = proof.clone();
            bad.h2p_rejection.rows.terminal.cx = noncanonical;
            reject(&bad);
            let mut bad = proof.clone();
            bad.norm.claims[0] = field.add(&bad.norm.claims[0], &field.one());
            reject(&bad);
            let mut bad = proof;
            bad.norm.sumchecks[0].round_polynomials.pop();
            reject(&bad);
        }
    }
    #[test]
    fn security_schedule_budgets_every_rejection_stage_for_both_targets() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let n = FalconSecurityNumerators::for_layout(&layout);
            assert_eq!(n.outer_point, REJECTION_ROW_LOG + layout.capacity().ilog2() as usize);
            assert_eq!(n.cubic_rounds, 3 * n.outer_point);
            assert_eq!(n.linear, layout.linear_stride().ilog2() as usize + layout.capacity().ilog2() as usize + batch + 8);
            for (target, floor) in [(100, 114), (128, 125)] {
                let s = security_schedule(&layout, target).unwrap();
                for (numerator, bits) in [
                    (n.norm_instances, s.norm_instance_bits),
                    (n.norm_rounds, s.quadratic_round_bits),
                    (n.outer_point, s.outer_point_bits),
                    (n.cubic_rounds, s.cubic_round_bits),
                    (n.linear, s.linear_point_bits),
                    (n.binding, s.binding_round_bits),
                ] {
                    assert_eq!(bits, stage_grinding_bits(numerator, target, floor));
                    if numerator != 0 {
                        let log = usize::BITS - (numerator - 1).leading_zeros();
                        assert!(log + target as u32 + 5 <= floor + bits);
                        if bits != 0 { assert!(log + target as u32 + 5 > floor + bits - 1); }
                    }
                }
                if target == 100 {
                    assert_eq!(s.norm_instance_bits, 0);
                    assert_eq!(s.quadratic_round_bits, 0);
                    assert_eq!(s.outer_point_bits, 0);
                    assert_eq!(s.cubic_round_bits, 0);
                }
            }
        }
        assert!(security_schedule(&FalconSourceLayout::new(1).unwrap(), 127).is_err());
        assert!(security_schedule(&FalconSourceLayout::new(1024).unwrap(), 100).unwrap().linear_point_bits > 0);
    }

}
}
