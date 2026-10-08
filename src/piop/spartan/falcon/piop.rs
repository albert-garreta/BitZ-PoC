// One combined integer outer for norm and HashToPoint rejection. Endpoint
// authentication reuses the original digit projections in opening.rs.
use super::{
    BETA_SQUARED, FalconError, FalconSignatureCt, FalconSourceLayout, FalconVerificationTrace, N,
    hash_to_point_selection::{REJECTION_ROW_LOG, REJECTION_ROWS, REJECTION_STRIDE},
};
use crate::{
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements, falcon_integer,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::eq_table,
        squeeze_field,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        proof::validate_field_elements,
    },
    transcript::traits::Transcript,
};
use field::{BatchMulAcc, FpLinearAcc, FpSignedLinearAcc, Reduce, RingOps};
type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;
#[path = "piop_shared.rs"]
mod shared;
use shared::{CompactSumcheck, verify_round_proofs};
pub(super) use shared::{
    FalconPiopClaimRef, FalconPiopClaims, FalconPiopProof, HashToPointRejectionClaims, NormClaims,
};
/// Prime-field numerators, excluding the native ring and binary-field stages.
/// The five binder endpoints are S1, slack, and three rejection row claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecurityNumerators {
    pub norm_instances: usize,
    pub relation_batching: usize,
    pub outer_point: usize,
    pub integer_rounds: usize,
    pub linear: usize,
    pub binding: usize,
}

impl FalconSecurityNumerators {
    pub const fn for_layout(layout: &FalconSourceLayout) -> Self {
        let d = layout.capacity().trailing_zeros() as usize;
        let rounds = REJECTION_ROW_LOG + d;
        Self {
            norm_instances: d,
            relation_batching: 1,
            outer_point: rounds,
            integer_rounds: 3 * rounds,
            linear: layout.linear_stride().ilog2() as usize + d + layout.batch() + 5,
            binding: 2 * (layout.signature_stride().ilog2() as usize + d),
        }
    }
}

/// Each prime-field stage receives at most 2^-(target+5) work-normalized error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecuritySchedule {
    pub norm_instance_bits: u32,
    pub outer_point_bits: u32,
    pub relation_batching_bits: u32,
    pub integer_round_bits: u32,
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
            relation_batching_bits: stage_grinding_bits(n.relation_batching, target_bits, floor),
            outer_point_bits: stage_grinding_bits(n.outer_point, target_bits, floor),
            integer_round_bits: stage_grinding_bits(n.integer_rounds, target_bits, floor),
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
    const DOMAIN: &'static [u8] = b"bitz/falcon/integer/norm-instances/v1";
}
struct OuterPointGrinding;
impl GrindingDomain for OuterPointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/integer/rejection-point/v1";
}
struct MergeGrinding;
impl GrindingDomain for MergeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/integer/family-merge/v1";
}
struct IntegerRoundGrinding;
impl GrindingDomain for IntegerRoundGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/integer/round/v1";
}
fn nonce<D: GrindingDomain>(
    t: &mut impl Transcript,
    bits: u32,
) -> Result<Option<u64>, FalconError> {
    if bits == 0 {
        Ok(None)
    } else {
        grind_and_absorb(t, GrindingRound::<D>::new(0), bits)
            .map(Some)
            .map_err(|e| piop(e.to_string()))
    }
}
fn check_nonce<D: GrindingDomain>(
    t: &mut impl Transcript,
    bits: u32,
    n: Option<u64>,
) -> Result<(), FalconError> {
    match (bits, n) {
        (0, None) => Ok(()),
        (1.., Some(n)) => verify_and_absorb(t, GrindingRound::<D>::new(0), bits, n)
            .map_err(|e| piop(e.to_string())),
        _ => Err(piop("integer outer nonce shape mismatch")),
    }
}
fn validate_shared_field(target: usize, field: &Cfg) -> Result<(), FalconError> {
    let (min, max) = super::shared_ring::prime_bounds(target)?;
    if !(min..=max).contains(&field.modulus_u128())
        || field.modulus_u128() <= super::constraints::SOURCE_RESIDUAL_BOUND
    {
        return Err(piop("invalid shared-prime integer context"));
    }
    Ok(())
}
fn header(
    t: &mut impl Transcript,
    layout: &FalconSourceLayout,
    target: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    validate_shared_field(target, field)?;
    t.absorb_slice(b"bitz/falcon/piop/combined-integer/v1");
    for v in [
        layout.batch(),
        layout.capacity(),
        target,
        layout.live_bits(),
        layout.occupied_bits(),
    ] {
        t.absorb_slice(&(v as u64).to_le_bytes());
    }
    t.absorb_slice(&field.modulus_u128().to_le_bytes());
    Ok(())
}
fn target<'a>(
    signatures: impl Iterator<Item = &'a FalconSignatureCt>,
    weights: &[F],
    field: &Cfg,
) -> F {
    let mut sum = FpSignedLinearAcc::<2, 2>::default();
    for (signature, weight) in signatures.zip(weights) {
        let norm: i128 = signature.s2.iter().map(|&s| i128::from(s).pow(2)).sum();
        field.mul_acc(&mut sum, weight, &(i128::from(BETA_SQUARED) - norm));
    }
    field.reduce(sum)
}
fn claims(field: &Cfg, rho: Vec<F>, point: Vec<F>, terminal: [F; 4], slack: F) -> FalconPiopClaims {
    FalconPiopClaims {
        modulus: field.modulus_u128(),
        norm: NormClaims {
            instance_point: rho,
            point: point.clone(),
            terminal: terminal[0],
            slack,
        },
        h2p_rejection: HashToPointRejectionClaims {
            point,
            terminal: [terminal[1], terminal[2], terminal[3]],
        },
    }
}
#[tracing::instrument(skip_all, name = "falcon_arithmetic:integer_outer")]
pub(super) fn prove_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<(FalconPiopProof, FalconPiopClaims), FalconError> {
    if traces.len() != layout.batch() {
        return Err(piop("invalid integer witness batch"));
    }
    header(transcript, layout, target_bits, field)?;
    let schedule = security_schedule(layout, target_bits)?;
    let instance_nonce = nonce::<NormInstanceGrinding>(transcript, schedule.norm_instance_bits)?;
    let rho = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let weights = eq_table(&rho, field).map_err(|e| piop(e.to_string()))?;
    let point_nonce = nonce::<OuterPointGrinding>(transcript, schedule.outer_point_bits)?;
    let tau = sample_point(transcript, rejection_row_rounds(layout), field)?;
    let mut slack = FpLinearAcc::<2, 1>::default();
    for (trace, weight) in traces.iter().zip(&weights) {
        field.mul_acc(&mut slack, weight, &trace.norm_slack);
    }
    let slack = field.reduce(slack);
    let initial = field.sub(
        &target(traces.iter().map(|t| &t.signature), &weights, field),
        &slack,
    );
    absorb_field_elements(transcript, &[slack], field);
    let merge_nonce = nonce::<MergeGrinding>(transcript, schedule.relation_batching_bits)?;
    let mix = squeeze(transcript, field)?;
    let mut boundary = ProverGrindingRoundBoundary::<IntegerRoundGrinding>::with_round_offset(
        schedule.integer_round_bits,
        0,
    );
    let output = falcon_integer::prove(
        field,
        transcript,
        REJECTION_ROW_LOG,
        &rho,
        Some(&tau),
        mix,
        initial,
        |row| {
            let instance = row / REJECTION_STRIDE;
            let j = row % REJECTION_STRIDE;
            let Some(trace) = traces.get(instance) else {
                return [0; 4];
            };
            let s = if j < N { trace.s1[j] } else { 0 };
            let q = if j < REJECTION_ROWS {
                trace.hash_to_point.quotients[j]
            } else {
                0
            };
            [
                s,
                i16::from((q >> 2) & 1),
                i16::from(q & 1),
                i16::from(((q >> 2) & 1) & (q & 1)),
            ]
        },
        &mut boundary,
    )
    .map_err(|e| piop(e.to_string()))?;
    let claims = claims(field, rho, output.point, output.terminal, slack);
    Ok((
        FalconPiopProof {
            instance_nonce,
            point_nonce,
            merge_nonce,
            slack,
            terminal: output.terminal,
            sumcheck: CompactSumcheck::from_full(output.proof),
            grinding_nonces: boundary.into_nonces(),
        },
        claims,
    ))
}
pub(super) fn verify_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    signatures: &[FalconSignatureCt],
    proof: &FalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopClaims, FalconError> {
    if signatures.len() != layout.batch() {
        return Err(piop("invalid public signature batch"));
    }
    header(transcript, layout, target_bits, field)?;
    validate_field_elements(&[proof.slack], field).map_err(|e| piop(e.to_string()))?;
    validate_field_elements(&proof.terminal, field).map_err(|e| piop(e.to_string()))?;
    let schedule = security_schedule(layout, target_bits)?;
    check_nonce::<NormInstanceGrinding>(
        transcript,
        schedule.norm_instance_bits,
        proof.instance_nonce,
    )?;
    let rho = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let weights = eq_table(&rho, field).map_err(|e| piop(e.to_string()))?;
    check_nonce::<OuterPointGrinding>(transcript, schedule.outer_point_bits, proof.point_nonce)?;
    let tau = sample_point(transcript, rejection_row_rounds(layout), field)?;
    let initial = field.sub(&target(signatures.iter(), &weights, field), &proof.slack);
    absorb_field_elements(transcript, &[proof.slack], field);
    check_nonce::<MergeGrinding>(
        transcript,
        schedule.relation_batching_bits,
        proof.merge_nonce,
    )?;
    let mix = squeeze(transcript, field)?;
    let mut boundary = VerifierGrindingRoundBoundary::<IntegerRoundGrinding>::new(
        schedule.integer_round_bits,
        &proof.grinding_nonces,
    );
    let (point, [claim]) = verify_round_proofs::<4, 3, 1>(
        [&proof.sumcheck],
        transcript,
        &[initial],
        tau.len(),
        field,
        &mut boundary,
    )
    .map_err(|e| piop(e.to_string()))?;
    if claim
        != falcon_integer::terminal(
            field,
            REJECTION_ROW_LOG,
            &rho,
            Some(&tau),
            mix,
            &point,
            &proof.terminal,
        )
    {
        return Err(piop("combined integer outer terminal mismatch"));
    }
    absorb_field_elements(transcript, &proof.terminal, field);
    Ok(claims(field, rho, point, proof.terminal, proof.slack))
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

#[cfg(test)]
fn unsigned(value: u128, field: &Cfg) -> F {
    F::from_with_cfg(value, field)
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
#[cfg(test)] mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;
    use super::super::verification_trace;
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
    fn security_schedule_budgets_every_rejection_stage_for_both_targets() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let n = FalconSecurityNumerators::for_layout(&layout);
            assert_eq!(n.outer_point, REJECTION_ROW_LOG + layout.capacity().ilog2() as usize);
            assert_eq!(n.integer_rounds, 3 * n.outer_point);
            assert_eq!(n.linear, layout.linear_stride().ilog2() as usize + layout.capacity().ilog2() as usize + batch + 5);
            for (target, floor) in [(100, 114), (128, 125)] {
                let s = security_schedule(&layout, target).unwrap();
                for (numerator, bits) in [
                    (n.norm_instances, s.norm_instance_bits),
                    (n.relation_batching, s.relation_batching_bits),
                    (n.outer_point, s.outer_point_bits),
                    (n.integer_rounds, s.integer_round_bits),
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
                    assert_eq!(s.relation_batching_bits, 0);
                    assert_eq!(s.outer_point_bits, 0);
                    assert_eq!(s.integer_round_bits, 0);
                }
            }
        }
        assert!(security_schedule(&FalconSourceLayout::new(1).unwrap(), 127).is_err());
        assert!(security_schedule(&FalconSourceLayout::new(1024).unwrap(), 100).unwrap().linear_point_bits > 0);
    }

    #[test]
    fn combined_outer_roundtrips_and_rejects_tampering() {
        for target in [100, 128] {
            let field = test_field(target);
            for batch in [1, 3] {
                let layout = FalconSourceLayout::new(batch).unwrap();
                let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
                let traces = vec![trace; batch];
                let signatures: Vec<_> = traces.iter().map(|t| t.signature.clone()).collect();
                let mut prover = Blake3Transcript::new();
                let (proof, claims) = prove_falcon_piop_in_field(&mut prover, &layout, &traces, target, &field).unwrap();
                let mut verifier = Blake3Transcript::new();
                let actual = verify_falcon_piop_in_field(&mut verifier, &layout, &signatures, &proof, target, &field).unwrap();
                assert_eq!(claims, actual);
                assert_eq!(prover.get_challenge::<field::Gf128>(), verifier.get_challenge::<field::Gf128>());
                assert_eq!(proof.sumcheck.round_polynomials.len(), REJECTION_ROW_LOG + layout.capacity().ilog2() as usize);
                let verify = |proof: &FalconPiopProof| verify_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, &signatures, proof, target, &field);
                for index in 0..4 {
                    let mut bad = proof.clone(); bad.terminal[index] = field.add(&bad.terminal[index], &field.one());
                    assert!(verify(&bad).is_err());
                }
                let mut bad = proof.clone(); bad.slack = field.add(&bad.slack, &field.one()); assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.sumcheck.round_polynomials.pop(); assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.sumcheck.round_polynomials[0][1] = field.add(&bad.sumcheck.round_polynomials[0][1], &field.one()); assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.grinding_nonces.push(0); assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.merge_nonce = if bad.merge_nonce.is_some() { None } else { Some(0) }; assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.instance_nonce = if bad.instance_nonce.is_some() { None } else { Some(0) }; assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.point_nonce = if bad.point_nonce.is_some() { None } else { Some(0) }; assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.sumcheck.round_polynomials.push([field.zero(); 3]); assert!(verify(&bad).is_err());
                let wider = Cfg::from_prime_u128((1u128 << 127) - 1);
                let noncanonical = unsigned(field.modulus_u128(), &wider);
                let mut bad = proof.clone(); bad.slack = noncanonical; assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.terminal[0] = noncanonical; assert!(verify(&bad).is_err());
                let mut bad = proof.clone(); bad.sumcheck.round_polynomials[0][0] = noncanonical; assert!(verify(&bad).is_err());
                let mut changed = signatures.clone(); changed[0].s2[0] += 1;
                assert!(verify_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, &changed, &proof, target, &field).is_err());
            }
        }
    }
    #[test]
    fn combined_norm_rejects_budget_transfer_between_signatures() {
        let field = test_field(100);
        let layout = FalconSourceLayout::new(2).unwrap();
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let mut traces = vec![trace; 2];
        for trace in &mut traces { trace.s1.fill(0); trace.signature.s2.fill(0); }
        let count = (BETA_SQUARED / 6144u64.pow(2) + 1) as usize;
        traces[0].s1[..count].fill(6144);
        let over = count as u64 * 6144u64.pow(2);
        traces[0].norm_slack = 0;
        traces[1].norm_slack = 2 * BETA_SQUARED - over;
        assert_eq!(over + traces[1].norm_slack, 2 * BETA_SQUARED);
        assert!(prove_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, &traces, 100, &field).is_err());
    }

}
}
