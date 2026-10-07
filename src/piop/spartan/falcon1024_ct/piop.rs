//! Algebraic Falcon PIOP layers over the transcript-selected prime field.
//!
//! This file deliberately stops at terminal MLE claims.  Those claims are
//! linearized into the one committed source witness by `opening.rs`; keeping
//! that boundary explicit prevents a native trace check from being mistaken
//! for a commitment-bound proof.

use field::RingOps;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::{eq_eval, eq_table},
        squeeze_field,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{
            ProverGrindingRoundBoundary, RoundBoundaryPolicy, VerifierGrindingRoundBoundary,
        },
        inner::{InitialClaims, prove_batched_inner_sumcheck},
        outer::{OuterClaim, OuterEvaluations, OuterRows, prove_outer_sumcheck},
        proof::{
            recover_full_round_polynomial_and_sample_next_challenge_with_boundary,
            validate_field_elements,
        },
    },
    transcript::traits::Transcript,
};

use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N,
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

#[path = "piop_shared.rs"]
mod shared;
pub(super) use shared::{
    FalconPiopClaimRef, SharedFalconPiopProof, verify_shared_falcon_piop_in_field,
};
use shared::{LeafPayload, NormPayload, QuadraticPayload, verify_round_proofs};

pub(super) const PRIME_MIN: u128 = 1u128 << 125;
pub(super) const PRIME_MAX: u128 = (1u128 << 126) - 1;
const COMPACTION_LEAVES: usize = 1 << 11;

/// Proof-of-work schedule for the selected projection field and requested
/// computational soundness target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecuritySchedule {
    pub norm_instance_bits: u32,
    pub outer_point_bits: u32,
    pub quadratic_round_bits: u32,
    /// Degree-three rejection and candidate-leaf sumcheck challenges.
    pub cubic_round_bits: u32,
    /// Joint forest weighted quadratic sumcheck challenges.
    pub forest_round_bits: u32,
    /// Root signature batching and forest line reductions.
    pub forest_claim_bits: u32,
    pub fingerprint_bits: u32,
    pub linear_point_bits: u32,
    pub binding_round_bits: u32,
}

impl FalconSecuritySchedule {
    /// At target 128, each of seven prime-reduction groups receives at most
    /// 2^-(target+5) work-normalized error. Target 100 needs no grinding.
    pub const fn for_layout(target_bits: usize, layout: &FalconSourceLayout) -> Option<Self> {
        if target_bits == 100 {
            return Some(Self {
                norm_instance_bits: 0,
                outer_point_bits: 0,
                quadratic_round_bits: 0,
                cubic_round_bits: 0,
                forest_round_bits: 0,
                forest_claim_bits: 0,
                fingerprint_bits: 0,
                linear_point_bits: 0,
                binding_round_bits: 0,
            });
        }
        if target_bits != 128 {
            return None;
        }
        let d = layout.capacity().trailing_zeros() as usize;
        let (cubic_round_bits, forest_round_bits, forest_claim_bits) = compaction_grinding_bits(d);
        Some(Self {
            // Independent instance batching for norms. Forest leaves inherit their point.
            // Batch one has no instance challenge, so its difficulty stays 0.
            norm_instance_bits: if d == 0 {
                0
            } else {
                component_grinding_bits(d)
            },
            quadratic_round_bits: component_grinding_bits(4 * (10 + d)),
            outer_point_bits: component_grinding_bits(11 + d),
            cubic_round_bits,
            forest_round_bits,
            forest_claim_bits,
            // Fix one incorrect signature before the fingerprint challenge.
            // Root batching and forest losses are accounted separately.
            fingerprint_bits: component_grinding_bits(2048),
            linear_point_bits: component_grinding_bits(13 + d + layout.batch() + 12),
            binding_round_bits: component_grinding_bits(2 * (17 + d)),
        })
    }
}

// At target 128 both profiles have p >= 2^125 and target+5 = 133: each
// group's weighted numerator must be at most 2^-8 before division by p.
const PRIME_GROUP_MARGIN: u32 = 8;

const fn component_grinding_bits(numerator: usize) -> u32 {
    if numerator == 0 {
        return 0;
    }
    PRIME_GROUP_MARGIN + usize::BITS - (numerator - 1).leading_zeros()
}

/// Allocate one shared 2^-133 budget to cubic relations, weighted quadratic
/// forest rounds, and root/line reductions. All arithmetic is exact.
const fn compaction_grinding_bits(d: usize) -> (u32, u32, u32) {
    COMPACTION_GRINDING_BITS[d]
}

// FalconSourceLayout accepts 1..=1024 signatures and pads capacity to a
// power of two, so every constructible layout has capacity log in 0..=10.
// Run the unchanged exact optimizer at compile time, preserving its tie order
// and all security parameters without repeating its search during proofs.
const COMPACTION_GRINDING_BITS: [(u32, u32, u32); 11] = {
    let mut table = [(0, 0, 0); 11];
    let mut d = 0;
    while d < table.len() {
        table[d] = solve_compaction_grinding_bits(d);
        d += 1;
    }
    table
};

const fn solve_compaction_grinding_bits(d: usize) -> (u32, u32, u32) {
    let cubic_rounds = 2 * (11 + d);
    let forest_rounds = 55 + 11 * (d + 1);
    let claim_draws = 11 + if d == 0 { 0 } else { 1 };
    let cubic_degree = (3 * cubic_rounds) as u128;
    let forest_degree = (2 * forest_rounds) as u128;
    let claim_degree = (d + 11) as u128;
    let uniform = component_grinding_bits((cubic_degree + forest_degree + claim_degree) as usize);
    // Every group has >=11 draws and all groups together have <=230. A
    // difficulty >=uniform+5 cannot improve on the feasible uniform choice.
    let denominator_bits = uniform + 5;
    let budget = 1u128 << (denominator_bits - PRIME_GROUP_MARGIN);
    let mut best = (uniform, uniform, uniform);
    let mut work = ((cubic_rounds + forest_rounds + claim_draws) as u128) << uniform;
    let mut cubic_bits = 0;
    while cubic_bits <= denominator_bits {
        let mut forest_bits = 0;
        while forest_bits <= denominator_bits {
            let mut claim_bits = 0;
            while claim_bits <= denominator_bits {
                let error = (cubic_degree << (denominator_bits - cubic_bits))
                    + (forest_degree << (denominator_bits - forest_bits))
                    + (claim_degree << (denominator_bits - claim_bits));
                let candidate_work = ((cubic_rounds as u128) << cubic_bits)
                    + ((forest_rounds as u128) << forest_bits)
                    + ((claim_draws as u128) << claim_bits);
                if error <= budget && candidate_work < work {
                    best = (cubic_bits, forest_bits, claim_bits);
                    work = candidate_work;
                }
                claim_bits += 1;
            }
            forest_bits += 1;
        }
        cubic_bits += 1;
    }
    best
}

struct NormInstanceGrinding;
impl GrindingDomain for NormInstanceGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/norm-instances/v3";
}

struct OuterPointGrinding;
impl GrindingDomain for OuterPointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/outer-point/v3";
}

struct QuadraticGrinding;
impl GrindingDomain for QuadraticGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/degree2/v1";
}

struct CubicGrinding;
impl GrindingDomain for CubicGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/degree3/v1";
}

struct FingerprintGrinding;
impl GrindingDomain for FingerprintGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/fingerprint/v1";
}

struct ForestBatchGrinding;
impl GrindingDomain for ForestBatchGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-root/weighted/v1";
}

struct ForestLineGrinding;
impl GrindingDomain for ForestLineGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-line/weighted/v1";
}

struct ForestRoundGrinding;
impl GrindingDomain for ForestRoundGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-round/weighted/v1";
}

struct LeafRoundGrinding;
impl GrindingDomain for LeafRoundGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/leaf-round/v1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormProof {
    /// Random instance weights are sampled after the source commitment and
    /// before any norm claims. Padding instances contribute zero.
    pub instance_point: Vec<F>,
    pub instance_nonce: Option<u64>,
    pub claims: [F; 2],
    pub slack: F,
    pub sumchecks: [SumcheckProof<F, 3>; 2],
    /// `[MLE(lambda_s * S)(r), MLE(S)(r)]` for `S1` and `S2`.
    pub terminal: [[F; 2]; 2],
    /// Shared evaluation point of the two norm sumchecks.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadraticRelationProof {
    pub point_nonce: Option<u64>,
    pub sumcheck: SumcheckProof<F, 4>,
    pub terminal: OuterEvaluations<F>,
    /// Terminal point returned by the outer sumcheck.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductForestLayerProof {
    /// Two coefficients of each quadratic weighted-sumcheck message.
    pub round_polynomials: Vec<[F; 2]>,
    /// One child pair for the entire forest, including its signature index.
    pub evaluations: [F; 2],
    pub grinding_nonces: Vec<u64>,
    pub line_nonce: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeProductForestProof {
    pub root_nonce: Option<u64>,
    pub layers: Vec<ProductForestLayerProof>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionProof {
    pub instance_point: Vec<F>,
    pub terminal_point: Vec<F>,
    pub candidate: F,
    pub output: F,
}

/// Authenticates the candidate forest leaves without committed selected-value
/// columns. Operand C includes the instance and forest evaluation weights;
/// it is the MLE of that weighted table, not a product of operand MLEs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionLeafProof {
    pub instance_point: Vec<F>,
    pub sumcheck: SumcheckProof<F, 4>,
    /// MLE evaluations of 1-d, 1-P_10, and beta*eq(forest)*(gamma-1+rho*P+r).
    /// All three tables vanish on padded candidates and signatures.
    pub terminal: [F; 3],
    /// Local candidate coordinates first, followed by instance coordinates.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

/// Proof through the norm, rejection, and compaction
/// witness-product layers.  The terminal claims must subsequently be
/// collapsed and opened against the source commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FalconPiopProof {
    pub modulus: u128,
    pub norm: NormProof,
    pub compact_products: QuadraticRelationProof,
    pub fingerprint_nonce: Option<u64>,
    pub compaction_gamma: F,
    pub compaction_rank_scale: F,
    pub compaction: CompactionProof,
    pub compaction_forest: PrimeProductForestProof,
    pub compaction_leaf: CompactionLeafProof,
}

/// Proves the nonlinear Falcon relation layers. Target 100 uses no grinding;
/// target 128 allocates the prime reductions a batch-specific error budget.
pub fn prove_falcon_piop(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
) -> Result<FalconPiopProof, FalconError> {
    validate_inputs(layout, traces, target_bits)?;
    if layout.is_shared_prime() {
        return Err(piop("shared-prime PIOP requires its supplied field"));
    }
    bind_header(transcript, layout, target_bits);
    let field = sample_field(transcript)?;
    prove_falcon_piop_body(transcript, layout, traces, target_bits, &field)
}

/// Runs the retained integer reductions using the prime already sampled after
/// the shared ring polynomial. The caller must obtain `field` from that sampler
/// and bind the source commitments before starting either branch.
pub(super) fn prove_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopProof, FalconError> {
    validate_inputs(layout, traces, target_bits)?;
    validate_shared_field(layout, target_bits, field)?;
    bind_shared_header(transcript, layout, target_bits, field);
    prove_falcon_piop_body(transcript, layout, traces, target_bits, field)
}

fn prove_falcon_piop_body(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopProof, FalconError> {
    let norm = prove_norm(transcript, layout, traces, target_bits, field)
        .map_err(|error| piop(format!("norm: {error}")))?;
    let compact_products =
        prove_compaction_products(transcript, layout, traces, target_bits, field)
            .map_err(|error| piop(format!("compaction products: {error}")))?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/fingerprint/v1");
    let fingerprint_nonce = if target_bits == 128 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<FingerprintGrinding>::new(0),
                security_schedule(layout, target_bits)?.fingerprint_bits,
            )
            .map_err(|error| piop(error.to_string()))?,
        )
    } else {
        None
    };
    let gamma = squeeze(transcript, field)?;
    let rank_scale = squeeze(transcript, field)?;
    let ranks = compaction_ranks(gamma, rank_scale, field);
    let leaves: Vec<_> = crate::utils::cfg_iter!(traces)
        .map(|trace| {
            let (candidate, output) = compaction_leaves_with_ranks(trace, &ranks, field);
            [candidate, output]
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();
    let (compaction_forest, compaction) = prove_product_forest(
        transcript,
        leaves,
        target_bits,
        security_schedule(layout, target_bits)?,
        field,
    )?;
    let compaction_leaf = prove_compaction_leaf(
        transcript,
        layout,
        traces,
        &compaction,
        gamma,
        rank_scale,
        target_bits,
        field,
    )?;

    Ok(FalconPiopProof {
        modulus: field.modulus_u128(),
        norm,
        compact_products,
        fingerprint_nonce,
        compaction_gamma: gamma,
        compaction_rank_scale: rank_scale,
        compaction,
        compaction_forest,
        compaction_leaf,
    })
}

/// Verifies the algebraic PIOP and returns successfully only after every
/// sumcheck and product-tree reduction is consistent.  The source-opening
/// adapter additionally binds the returned terminal evaluations to bits.
pub fn verify_falcon_piop(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    target_bits: usize,
) -> Result<(), FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid PIOP shape"));
    }
    if layout.is_shared_prime() {
        return Err(piop("shared-prime PIOP requires its supplied field"));
    }
    bind_header(transcript, layout, target_bits);
    let field = sample_field(transcript)?;
    if field.modulus_u128() != proof.modulus {
        return Err(piop("transcript prime mismatch"));
    }
    verify_falcon_piop_body(transcript, layout, proof, target_bits, &field)
}

/// Verifies using the authenticated shared-prime context; this entry point does
/// not draw a prime. Its transcript header separates it from the legacy PIOP.
pub(super) fn verify_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid PIOP shape"));
    }
    validate_shared_field(layout, target_bits, field)?;
    if field.modulus_u128() != proof.modulus {
        return Err(piop("shared transcript prime mismatch"));
    }
    bind_shared_header(transcript, layout, target_bits, field);
    verify_falcon_piop_body(transcript, layout, proof, target_bits, field)
}

fn verify_falcon_piop_body(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    verify_norm(transcript, layout, &proof.norm, target_bits, field)?;
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    verify_quadratic(
        transcript,
        compaction_product_rounds(layout),
        &proof.compact_products,
        target_bits,
        security_schedule(layout, target_bits)?,
        field,
    )?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/fingerprint/v1");
    match (target_bits, proof.fingerprint_nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<FingerprintGrinding>::new(0),
            security_schedule(layout, target_bits)?.fingerprint_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string()))?,
        (100, None) => {}
        _ => return Err(piop("invalid fingerprint grinding nonce")),
    }
    let gamma = squeeze(transcript, field)?;
    let rank_scale = squeeze(transcript, field)?;
    if gamma != proof.compaction_gamma || rank_scale != proof.compaction_rank_scale {
        return Err(piop("compaction fingerprint challenge mismatch"));
    }
    verify_product_forest(
        transcript,
        &proof.compaction_forest,
        &proof.compaction,
        layout.batch(),
        target_bits,
        security_schedule(layout, target_bits)?,
        field,
    )?;
    verify_compaction_leaf(
        transcript,
        layout,
        &proof.compaction,
        &proof.compaction_leaf,
        target_bits,
        field,
    )?;
    Ok(())
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

fn bind_header(transcript: &mut impl Transcript, layout: &FalconSourceLayout, target_bits: usize) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/piop/native-ring/v5");
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
}

fn validate_shared_field(
    layout: &FalconSourceLayout,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    let (prime_min, prime_max) = super::shared_ring::prime_bounds(target_bits)?;
    let modulus = field.modulus_u128();
    if !layout.is_shared_prime()
        || !(prime_min..=prime_max).contains(&modulus)
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
    transcript.absorb_slice(b"bitz/falcon1024-ct/piop/shared-prime/v4");
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&(layout.live_bits() as u64).to_le_bytes());
}

fn sample_field(transcript: &mut impl Transcript) -> Result<Cfg, FalconError> {
    crate::prime_sampling::sample_prime_context(transcript, PRIME_MIN, PRIME_MAX, 128)
        .map_err(|error| piop(error.to_string()))
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:norm")]
fn prove_norm(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<NormProof, FalconError> {
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
    Ok(NormProof {
        instance_point,
        instance_nonce,
        claims,
        slack,
        sumchecks: output.proofs,
        terminal: output.terminal_evaluations,
        point: output.point,
        grinding_nonces,
    })
}

fn verify_norm(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &NormProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    verify_norm_payload(transcript, layout, proof.into(), target_bits, field).map(|_| ())
}

fn verify_norm_payload(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: NormPayload<'_>,
    target_bits: usize,
    field: &Cfg,
) -> Result<(Vec<F>, Vec<F>, F), FalconError> {
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
    if proof
        .instance_point
        .is_some_and(|stored| instance_point != stored)
    {
        return Err(piop("norm instance point mismatch"));
    }
    validate_field_elements(&proof.claims, field).map_err(|error| piop(error.to_string()))?;
    if let Some(slack) = proof.slack {
        validate_field_elements(&[slack], field).map_err(|error| piop(error.to_string()))?;
    }
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
    if proof.slack.is_some_and(|stored| stored != slack) {
        return Err(piop("norm claim does not equal the Falcon bound"));
    }
    absorb_field_elements(
        transcript,
        &[proof.claims[0], proof.claims[1], slack],
        field,
    );
    let rounds = 10 + layout.capacity().trailing_zeros() as usize;
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<QuadraticGrinding>::new(
            security_schedule(layout, target_bits)?.quadratic_round_bits,
            &proof.grinding_nonces,
        );
        verify_round_proofs(
            proof.sumchecks,
            transcript,
            &proof.claims,
            rounds,
            field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        verify_round_proofs(
            proof.sumchecks,
            transcript,
            &proof.claims,
            rounds,
            field,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    if point.len() != rounds {
        return Err(piop("norm point length mismatch"));
    }
    if proof.point.is_some_and(|stored| point != stored) {
        return Err(piop("norm terminal point mismatch"));
    }
    for (claim, terminal) in final_claims.iter().zip(proof.terminal.iter()) {
        if *claim != field.mul(&terminal[0], &terminal[1]) {
            return Err(piop("norm terminal identity failed"));
        }
    }
    absorb_field_elements(transcript, &proof.terminal.concat(), field);
    Ok((instance_point, point, slack))
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:hash_to_point_products")]
fn prove_compaction_products(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<QuadraticRelationProof, FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    let rows = CompactionRows {
        traces,
        stride: COMPACTION_LEAVES * layout.capacity(),
        field,
    };
    prove_quadratic(
        transcript,
        rows,
        compaction_product_rounds(layout),
        target_bits,
        security_schedule(layout, target_bits)?,
        field,
    )
}

/// Rejection relation q_bit2 * q_bit0 = d. Original operands stay packed
/// until the outer sumcheck's first fold; padded candidates contribute zero.
struct CompactionRows<'a> {
    traces: &'a [FalconVerificationTrace],
    stride: usize,
    field: &'a Cfg,
}

impl CompactionRows<'_> {
    fn candidate(&self, row: usize) -> Option<(&super::HashToPointTrace, usize)> {
        let candidate = row % COMPACTION_LEAVES;
        let trace = self.traces.get(row / COMPACTION_LEAVES)?;
        (candidate < HASH_TO_POINT_SAMPLES).then_some((&trace.hash_to_point, candidate))
    }
}

impl OuterRows for CompactionRows<'_> {
    type AB = F;
    type C = F;

    fn dimensions(&self) -> (usize, usize, usize) {
        (self.stride, self.stride, self.stride)
    }

    fn a(&self, row: usize) -> F {
        self.candidate(row).map_or(self.field.zero(), |(hash, i)| {
            unsigned(u128::from((hash.quotients[i] >> 2) & 1), self.field)
        })
    }

    fn b(&self, row: usize) -> F {
        self.candidate(row).map_or(self.field.zero(), |(hash, i)| {
            unsigned(u128::from(hash.quotients[i] & 1), self.field)
        })
    }

    fn c(&self, row: usize) -> F {
        self.candidate(row).map_or(self.field.zero(), |(hash, i)| {
            unsigned(u128::from(!hash.accepted[i]), self.field)
        })
    }
}

fn prove_quadratic(
    transcript: &mut impl Transcript,
    rows: impl OuterRows<AB = F, C = F>,
    rounds: usize,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<QuadraticRelationProof, FalconError> {
    let point_nonce = if target_bits == 128 {
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
    let output = if target_bits == 128 {
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
    Ok(QuadraticRelationProof {
        point_nonce,
        sumcheck: output.0.proof,
        terminal: output.0.evaluations,
        point: output.0.point,
        grinding_nonces: output.1,
    })
}

fn verify_quadratic(
    transcript: &mut impl Transcript,
    rounds: usize,
    proof: &QuadraticRelationProof,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(), FalconError> {
    verify_quadratic_payload(
        transcript,
        rounds,
        proof.into(),
        target_bits,
        security,
        field,
    )
    .map(|_| ())
}

fn verify_quadratic_payload(
    transcript: &mut impl Transcript,
    rounds: usize,
    proof: QuadraticPayload<'_>,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    match (target_bits, proof.point_nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<OuterPointGrinding>::new(0),
            security.outer_point_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string()))?,
        (100, None) => {}
        _ => return Err(piop("invalid outer-point grinding nonce")),
    }
    let tau = sample_point(transcript, rounds, field)?;
    let zero = field.zero();
    validate_field_elements(
        &[proof.terminal.ax, proof.terminal.bx, proof.terminal.cx],
        field,
    )
    .map_err(|error| piop(error.to_string()))?;
    let (point, [final_claim]) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<CubicGrinding>::new(
            security.cubic_round_bits,
            &proof.grinding_nonces,
        );
        verify_round_proofs(
            [proof.sumcheck],
            transcript,
            &[zero],
            rounds,
            field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        verify_round_proofs(
            [proof.sumcheck],
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
    if proof.point.is_some_and(|stored| point != stored) {
        return Err(piop("quadratic terminal point mismatch"));
    }
    Ok(point)
}

#[cfg(test)]
fn compaction_leaves(
    trace: &FalconVerificationTrace,
    gamma: F,
    rank_scale: F,
    field: &Cfg,
) -> (Vec<F>, Vec<F>) {
    compaction_leaves_with_ranks(trace, &compaction_ranks(gamma, rank_scale, field), field)
}

fn compaction_ranks(gamma: F, rank_scale: F, field: &Cfg) -> [F; N] {
    let mut next = gamma;
    std::array::from_fn(|_| {
        let value = next;
        next = field.add(&next, &rank_scale);
        value
    })
}

fn compaction_leaves_with_ranks(
    trace: &FalconVerificationTrace,
    ranks: &[F; N],
    field: &Cfg,
) -> (Vec<F>, Vec<F>) {
    let one = field.one();
    let mut candidate = vec![one; COMPACTION_LEAVES];
    let mut output = vec![one; COMPACTION_LEAVES];
    for i in 0..HASH_TO_POINT_SAMPLES {
        let selected = trace.hash_to_point.accepted[i] && trace.hash_to_point.prefix[i] < 1024;
        if selected {
            candidate[i] = field.add(
                &ranks[usize::from(trace.hash_to_point.prefix[i])],
                &unsigned(u128::from(trace.hash_to_point.remainders[i]), field),
            );
        }
    }
    for (rank, leaf) in output.iter_mut().take(N).enumerate() {
        *leaf = field.add(
            &ranks[rank],
            &unsigned(u128::from(trace.hash_to_point.point[rank]), field),
        );
    }
    (candidate, output)
}

/// A root difference is batched across signatures, then reduced jointly over
/// signature, side and position coordinates. Dummy signatures have all-one
/// leaves on both sides, so they contribute zero to the initial difference.
#[tracing::instrument(skip_all, name = "falcon_arithmetic:compaction_forest")]
fn prove_product_forest(
    transcript: &mut impl Transcript,
    mut leaves: Vec<Vec<F>>,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(PrimeProductForestProof, CompactionProof), FalconError> {
    if leaves.is_empty()
        || leaves.len() > 2048
        || leaves.len() % 2 != 0
        || leaves.iter().any(|tree| tree.len() != COMPACTION_LEAVES)
    {
        return Err(piop("invalid compaction forest shape"));
    }
    let batch = leaves.len() / 2;
    let capacity = batch.next_power_of_two();
    let d = capacity.ilog2() as usize;
    leaves.resize_with(2 * capacity, || vec![field.one(); COMPACTION_LEAVES]);
    let mut trees: Vec<Vec<Vec<F>>> = crate::utils::cfg_into_iter!(leaves)
        .map(|leaves| {
            let mut tree = vec![leaves];
            while tree.last().expect("leaf layer").len() > 1 {
                let child = tree.last().expect("child layer");
                let half = child.len() / 2;
                tree.push(
                    (0..half)
                        .map(|i| field.mul(&child[i], &child[i + half]))
                        .collect(),
                );
            }
            tree.reverse();
            tree
        })
        .collect();
    bind_forest(transcript, batch);
    let root_nonce = if d == 0 {
        None
    } else {
        prove_forest_nonce::<ForestBatchGrinding>(transcript, 0, target_bits, security)?
    };
    let mut instance_point = sample_point(transcript, d, field)?;
    let mut tree_weights: Vec<_> = instance_point
        .iter()
        .copied()
        .map(ForestWeight::Equality)
        .collect();
    tree_weights.push(ForestWeight::Difference);
    let mut local_point = Vec::new();
    let mut claim = field.zero();
    let mut layers = Vec::with_capacity(11);
    let mut endpoints = [field.zero(); 2];
    for level in 0..11 {
        transcript.absorb_slice(&(level as u64).to_le_bytes());
        let mut groups: Vec<[Vec<F>; 2]> = crate::utils::cfg_iter_mut!(trees)
            .map(|tree| {
                let mut child = std::mem::take(&mut tree[level + 1]);
                let right = child.split_off(child.len() / 2);
                [child, right]
            })
            .collect();
        let (mut layer, point, side_pairs) = prove_forest_layer(
            transcript,
            &mut groups,
            &local_point,
            &tree_weights,
            claim,
            target_bits,
            security,
            field,
        )?;
        absorb_field_elements(transcript, &layer.evaluations, field);
        layer.line_nonce =
            prove_forest_nonce::<ForestLineGrinding>(transcript, level, target_bits, security)?;
        let lambda = squeeze(transcript, field)?;
        claim = affine(layer.evaluations[0], layer.evaluations[1], lambda, field);
        local_point = point[..level].to_vec();
        local_point.push(lambda);
        instance_point = point[level..level + d].to_vec();
        tree_weights = point[level..]
            .iter()
            .copied()
            .map(ForestWeight::Equality)
            .collect();
        endpoints = side_pairs.map(|[left, right]| affine(left, right, lambda, field));
        layers.push(layer);
    }
    absorb_field_elements(transcript, &endpoints, field);
    let compaction = CompactionProof {
        instance_point,
        terminal_point: local_point,
        candidate: endpoints[0],
        output: endpoints[1],
    };
    Ok((PrimeProductForestProof { root_nonce, layers }, compaction))
}

fn bind_forest(transcript: &mut impl Transcript, batch: usize) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/forest/joint-weighted/v1");
    transcript.absorb_slice(&(batch as u64).to_le_bytes());
    transcript.absorb_slice(&(batch.next_power_of_two() as u64).to_le_bytes());
}

#[derive(Clone, Copy)]
enum ForestWeight {
    Equality(F),
    Difference,
}

impl ForestWeight {
    fn endpoints(self, field: &Cfg) -> [F; 2] {
        match self {
            Self::Equality(r) => [field.sub(&field.one(), &r), r],
            Self::Difference => [field.one(), field.neg(&field.one())],
        }
    }

    fn encode(self, polynomial: [F; 3]) -> [F; 2] {
        match self {
            Self::Equality(_) => [polynomial[1], polynomial[2]],
            Self::Difference => [polynomial[0], polynomial[2]],
        }
    }

    /// Recover a quadratic from its weighted endpoint sum. No division or
    /// exclusion of zero/one challenges is needed, including signed weights.
    fn recover(self, claim: F, message: [F; 2], field: &Cfg) -> [F; 3] {
        let [a, b] = message;
        match self {
            Self::Equality(r) => [field.sub(&claim, &field.mul(&r, &field.add(&a, &b))), a, b],
            Self::Difference => [a, field.neg(&field.add(&claim, &b)), b],
        }
    }
}

fn forest_weights(weights: &[ForestWeight], field: &Cfg) -> Vec<F> {
    let mut table = vec![field.one()];
    for weight in weights {
        let [left, right] = weight.endpoints(field);
        let half = table.len();
        table.resize(2 * half, field.zero());
        for i in 0..half {
            table[i + half] = field.mul(&table[i], &right);
            table[i] = field.mul(&table[i], &left);
        }
    }
    table
}

#[cfg(test)]
fn weighted_sum(values: &[F], scales: &[F], field: &Cfg) -> F {
    values
        .iter()
        .zip(scales)
        .fold(field.zero(), |sum, (value, scale)| {
            field.add(&sum, &field.mul(value, scale))
        })
}

fn fold_table(table: &mut Vec<F>, challenge: F, field: &Cfg) {
    for i in 0..table.len() / 2 {
        table[i] = affine(table[2 * i], table[2 * i + 1], challenge, field);
    }
    table.truncate(table.len() / 2);
}

fn evaluate_quadratic(polynomial: &[F; 3], point: F, field: &Cfg) -> F {
    field.add(
        &polynomial[0],
        &field.mul(
            &point,
            &field.add(&polynomial[1], &field.mul(&point, &polynomial[2])),
        ),
    )
}

/// Sum products with the current coordinate's weight omitted. Previously
/// sampled coordinates contribute no equality factor to this polynomial.
fn weighted_product_polynomial(
    left: &[F],
    right: &[F],
    future_weights: &[F],
    field: &Cfg,
) -> [F; 3] {
    let mut polynomial = [field.zero(); 3];
    for (i, weight) in future_weights.iter().enumerate() {
        let l0 = left[2 * i];
        let ld = field.sub(&left[2 * i + 1], &l0);
        let r0 = right[2 * i];
        let rd = field.sub(&right[2 * i + 1], &r0);
        let product = [
            field.mul(&l0, &r0),
            field.add(&field.mul(&l0, &rd), &field.mul(&ld, &r0)),
            field.mul(&ld, &rd),
        ];
        for j in 0..3 {
            polynomial[j] = field.add(&polynomial[j], &field.mul(weight, &product[j]));
        }
    }
    polynomial
}

/// Fold positions first while retaining tree-level parallelism, then fold the
/// small signature/side tables. The final side coordinate is kept last so its
/// two leaf evaluations can be recovered without another pass over the leaves.
fn prove_forest_layer(
    transcript: &mut impl Transcript,
    groups: &mut [[Vec<F>; 2]],
    point: &[F],
    tree_weights: &[ForestWeight],
    mut claim: F,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(ProductForestLayerProof, Vec<F>, [[F; 2]; 2]), FalconError> {
    let mut weights: Vec<_> = point.iter().copied().map(ForestWeight::Equality).collect();
    weights.extend_from_slice(tree_weights);
    let scales = forest_weights(tree_weights, field);
    let capacity = groups.len() / 2;
    let mut boundary = ProverGrindingRoundBoundary::<ForestRoundGrinding>::with_round_offset(
        if target_bits == 128 {
            security.forest_round_bits
        } else {
            0
        },
        0,
    );
    let mut round_polynomials = Vec::with_capacity(weights.len());
    let mut next_point = Vec::with_capacity(weights.len());
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut side_pairs = [[field.zero(); 2]; 2];
    for (round, &weight) in weights.iter().enumerate() {
        let polynomial = if round < point.len() {
            let future = forest_weights(&weights[round + 1..point.len()], field);
            let polynomials: Vec<_> = crate::utils::cfg_iter!(groups)
                .enumerate()
                .map(|(tree, group)| {
                    let scale = scales[tree / 2 + (tree % 2) * capacity];
                    weighted_product_polynomial(&group[0], &group[1], &future, field)
                        .map(|c| field.mul(&c, &scale))
                })
                .collect();
            polynomials.into_iter().fold([field.zero(); 3], |sum, p| {
                std::array::from_fn(|i| field.add(&sum[i], &p[i]))
            })
        } else {
            if round == point.len() {
                // Existing storage alternates candidate/output. The joint
                // table puts signature bits first and the side bit last.
                left = (0..groups.len())
                    .map(|i| groups[2 * (i % capacity) + i / capacity][0][0])
                    .collect();
                right = (0..groups.len())
                    .map(|i| groups[2 * (i % capacity) + i / capacity][1][0])
                    .collect();
            }
            if left.len() == 2 {
                side_pairs = [[left[0], right[0]], [left[1], right[1]]];
            }
            let future = forest_weights(&weights[round + 1..], field);
            weighted_product_polynomial(&left, &right, &future, field)
        };
        let [w0, w1] = weight.endpoints(field);
        let at_one = polynomial
            .iter()
            .fold(field.zero(), |sum, c| field.add(&sum, c));
        if field.add(&field.mul(&w0, &polynomial[0]), &field.mul(&w1, &at_one)) != claim {
            return Err(piop("weighted product forest round claim mismatch"));
        }
        let message = weight.encode(polynomial);
        absorb_field_elements(transcript, &polynomial, field);
        boundary
            .after_round(transcript, round)
            .map_err(|error| piop(error.to_string()))?;
        let challenge = squeeze(transcript, field)?;
        claim = evaluate_quadratic(&polynomial, challenge, field);
        round_polynomials.push(message);
        next_point.push(challenge);
        if round < point.len() {
            crate::utils::cfg_iter_mut!(groups).for_each(|group| {
                fold_table(&mut group[0], challenge, field);
                fold_table(&mut group[1], challenge, field);
            });
        } else {
            fold_table(&mut left, challenge, field);
            fold_table(&mut right, challenge, field);
        }
    }
    let evaluations = [left[0], right[0]];
    if claim != field.mul(&evaluations[0], &evaluations[1]) {
        return Err(piop("weighted product forest terminal mismatch"));
    }
    Ok((
        ProductForestLayerProof {
            round_polynomials,
            evaluations,
            grinding_nonces: boundary.into_nonces(),
            line_nonce: None,
        },
        next_point,
        side_pairs,
    ))
}

fn prove_forest_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    level: usize,
    target_bits: usize,
    security: FalconSecuritySchedule,
) -> Result<Option<u64>, FalconError> {
    if target_bits == 128 {
        grind_and_absorb(
            transcript,
            GrindingRound::<D>::new(level as u64),
            security.forest_claim_bits,
        )
        .map(Some)
        .map_err(|error| piop(error.to_string()))
    } else {
        Ok(None)
    }
}

fn verify_forest_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    level: usize,
    target_bits: usize,
    security: FalconSecuritySchedule,
    nonce: Option<u64>,
) -> Result<(), FalconError> {
    match (target_bits, nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<D>::new(level as u64),
            security.forest_claim_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string())),
        (100, None) => Ok(()),
        _ => Err(piop("invalid product forest grinding nonce")),
    }
}

fn verify_product_forest(
    transcript: &mut impl Transcript,
    forest: &PrimeProductForestProof,
    compaction: &CompactionProof,
    batch: usize,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(), FalconError> {
    let expected = verify_product_forest_payload(
        transcript,
        &forest.layers,
        forest.root_nonce,
        [compaction.candidate, compaction.output],
        batch,
        target_bits,
        security,
        field,
    )?;
    if *compaction != expected {
        return Err(piop("product forest terminal point mismatch"));
    }
    Ok(())
}

fn verify_product_forest_payload(
    transcript: &mut impl Transcript,
    layers: &[ProductForestLayerProof],
    root_nonce: Option<u64>,
    endpoints: [F; 2],
    batch: usize,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<CompactionProof, FalconError> {
    if !(1..=1024).contains(&batch) || layers.len() != 11 || !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid compaction forest shape"));
    }
    validate_field_elements(&endpoints, field).map_err(|error| piop(error.to_string()))?;
    let d = batch.next_power_of_two().ilog2() as usize;
    bind_forest(transcript, batch);
    if d == 0 {
        if root_nonce.is_some() {
            return Err(piop("unexpected forest root nonce"));
        }
    } else {
        verify_forest_nonce::<ForestBatchGrinding>(
            transcript,
            0,
            target_bits,
            security,
            root_nonce,
        )?;
    }
    let mut tree_point = sample_point(transcript, d, field)?;
    let mut tree_weights: Vec<_> = tree_point
        .iter()
        .copied()
        .map(ForestWeight::Equality)
        .collect();
    tree_weights.push(ForestWeight::Difference);
    let mut local_point = Vec::new();
    let mut claim = field.zero();
    for (level, layer) in layers.iter().enumerate() {
        transcript.absorb_slice(&(level as u64).to_le_bytes());
        let mut weights: Vec<_> = local_point
            .iter()
            .copied()
            .map(ForestWeight::Equality)
            .collect();
        weights.extend_from_slice(&tree_weights);
        if layer.round_polynomials.len() != weights.len() {
            return Err(piop("weighted forest round count mismatch"));
        }
        validate_field_elements(&layer.evaluations, field)
            .map_err(|error| piop(error.to_string()))?;
        let mut boundary = VerifierGrindingRoundBoundary::<ForestRoundGrinding>::new(
            if target_bits == 128 {
                security.forest_round_bits
            } else {
                0
            },
            &layer.grinding_nonces,
        );
        boundary
            .validate(weights.len())
            .map_err(|error| piop(error.to_string()))?;
        let mut next_point = Vec::with_capacity(weights.len());
        for (round, (weight, message)) in weights.iter().zip(&layer.round_polynomials).enumerate() {
            validate_field_elements(message, field).map_err(|error| piop(error.to_string()))?;
            let polynomial = weight.recover(claim, *message, field);
            absorb_field_elements(transcript, &polynomial, field);
            boundary
                .after_round(transcript, round)
                .map_err(|error| piop(error.to_string()))?;
            let challenge = squeeze(transcript, field)?;
            claim = evaluate_quadratic(&polynomial, challenge, field);
            next_point.push(challenge);
        }
        if claim != field.mul(&layer.evaluations[0], &layer.evaluations[1]) {
            return Err(piop("weighted product forest terminal identity failed"));
        }
        absorb_field_elements(transcript, &layer.evaluations, field);
        verify_forest_nonce::<ForestLineGrinding>(
            transcript,
            level,
            target_bits,
            security,
            layer.line_nonce,
        )?;
        let lambda = squeeze(transcript, field)?;
        claim = affine(layer.evaluations[0], layer.evaluations[1], lambda, field);
        local_point = next_point[..level].to_vec();
        local_point.push(lambda);
        tree_point = next_point[level..].to_vec();
        tree_weights = tree_point
            .iter()
            .copied()
            .map(ForestWeight::Equality)
            .collect();
    }
    if claim != affine(endpoints[0], endpoints[1], tree_point[d], field) {
        return Err(piop("product forest candidate/output split mismatch"));
    }
    absorb_field_elements(transcript, &endpoints, field);
    Ok(CompactionProof {
        instance_point: tree_point[..d].to_vec(),
        terminal_point: local_point,
        candidate: endpoints[0],
        output: endpoints[1],
    })
}

fn bind_compaction_leaf(
    transcript: &mut impl Transcript,
    compaction: &CompactionProof,
    field: &Cfg,
) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/leaf/inherited-point/v2");
    absorb_field_elements(transcript, &[compaction.candidate], field);
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:compaction_leaf")]
fn prove_compaction_leaf(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    compaction: &CompactionProof,
    gamma: F,
    rank_scale: F,
    target_bits: usize,
    field: &Cfg,
) -> Result<CompactionLeafProof, FalconError> {
    let security = security_schedule(layout, target_bits)?;
    bind_compaction_leaf(transcript, compaction, field);
    let instance_rounds = layout.capacity().trailing_zeros() as usize;
    let instance_point = compaction.instance_point.clone();
    let weights = eq_table(&instance_point, field).map_err(|error| piop(error.to_string()))?;
    let forest_weights =
        eq_table(&compaction.terminal_point, field).map_err(|error| piop(error.to_string()))?;
    let gamma_minus_one = field.sub(&gamma, &field.one());
    let mut values = vec![[field.zero(); 3]; COMPACTION_LEAVES * layout.capacity()];
    #[cfg(feature = "parallel")]
    let chunks = values.par_chunks_mut(COMPACTION_LEAVES);
    #[cfg(not(feature = "parallel"))]
    let chunks = values.chunks_mut(COMPACTION_LEAVES);
    chunks.enumerate().for_each(|(instance, values)| {
        let Some(trace) = traces.get(instance) else {
            return;
        };
        let hash = &trace.hash_to_point;
        for i in 0..HASH_TO_POINT_SAMPLES {
            let fingerprint = field.add(
                &field.add(
                    &gamma_minus_one,
                    &field.mul(&rank_scale, &unsigned(hash.prefix[i].into(), field)),
                ),
                &unsigned(hash.remainders[i].into(), field),
            );
            values[i] = [
                unsigned(u128::from(hash.accepted[i]), field),
                unsigned(u128::from(1 - ((hash.prefix[i] >> 10) & 1)), field),
                field.mul(
                    &field.mul(&weights[instance], &forest_weights[i]),
                    &fingerprint,
                ),
            ];
        }
    });
    let mut claim = field.sub(&compaction.candidate, &field.one());
    let mut boundary = ProverGrindingRoundBoundary::<LeafRoundGrinding>::with_round_offset(
        if target_bits == 128 {
            security.cubic_round_bits
        } else {
            0
        },
        0,
    );
    let rounds = 11 + instance_rounds;
    let mut point = Vec::with_capacity(rounds);
    let mut round_polynomials = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        // The linear coefficient is recovered from g(0)+g(1)=claim. Computing
        // only coefficients 0, 2 and 3 saves three field products per pair.
        let blocks: Vec<_> = (0..values.len() / 2).step_by(4096).collect();
        let polynomials: Vec<[F; 3]> = crate::utils::cfg_iter!(blocks)
            .map(|&start| {
                let mut polynomial = [field.zero(); 3];
                for i in start..(start + 4096).min(values.len() / 2) {
                    let [a, b, c] = values[2 * i];
                    let [ar, br, cr] = values[2 * i + 1];
                    let ad = field.sub(&ar, &a);
                    let bd = field.sub(&br, &b);
                    let cd = field.sub(&cr, &c);
                    let ab = field.mul(&a, &b);
                    let cross = field.add(&field.mul(&a, &bd), &field.mul(&ad, &b));
                    let high = field.mul(&ad, &bd);
                    polynomial[0] = field.add(&polynomial[0], &field.mul(&ab, &c));
                    polynomial[1] = field.add(
                        &polynomial[1],
                        &field.add(&field.mul(&cross, &cd), &field.mul(&high, &c)),
                    );
                    polynomial[2] = field.add(&polynomial[2], &field.mul(&high, &cd));
                }
                polynomial
            })
            .collect();
        let polynomial = polynomials.into_iter().fold([field.zero(); 3], |sum, p| {
            std::array::from_fn(|i| field.add(&sum[i], &p[i]))
        });
        let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
            transcript,
            &mut claim,
            &polynomial,
            &mut round_polynomials,
            &mut point,
            &field.zero(),
            field,
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        let mut next = vec![[field.zero(); 3]; values.len() / 2];
        crate::utils::cfg_iter_mut!(next)
            .enumerate()
            .for_each(|(i, row)| {
                *row = std::array::from_fn(|j| {
                    affine(values[2 * i][j], values[2 * i + 1][j], challenge, field)
                });
            });
        values = next;
    }
    let terminal = values[0];
    if claim != field.mul(&field.mul(&terminal[0], &terminal[1]), &terminal[2]) {
        return Err(piop("compaction leaf terminal identity failed"));
    }
    absorb_field_elements(transcript, &terminal, field);
    Ok(CompactionLeafProof {
        instance_point,
        sumcheck: SumcheckProof { round_polynomials },
        terminal,
        point,
        grinding_nonces: boundary.into_nonces(),
    })
}

fn verify_compaction_leaf(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    compaction: &CompactionProof,
    proof: &CompactionLeafProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    verify_compaction_leaf_payload(
        transcript,
        layout,
        compaction,
        proof.into(),
        target_bits,
        field,
    )
    .map(|_| ())
}

fn verify_compaction_leaf_payload(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    compaction: &CompactionProof,
    proof: LeafPayload<'_>,
    target_bits: usize,
    field: &Cfg,
) -> Result<(Vec<F>, Vec<F>), FalconError> {
    let security = security_schedule(layout, target_bits)?;
    bind_compaction_leaf(transcript, compaction, field);
    let instance_rounds = layout.capacity().trailing_zeros() as usize;
    let instance_point = compaction.instance_point.clone();
    if proof
        .instance_point
        .is_some_and(|stored| instance_point != stored)
    {
        return Err(piop("compaction leaf instance point mismatch"));
    }
    validate_field_elements(&proof.terminal, field).map_err(|error| piop(error.to_string()))?;
    let initial = field.sub(&compaction.candidate, &field.one());
    let mut boundary = VerifierGrindingRoundBoundary::<LeafRoundGrinding>::new(
        if target_bits == 128 {
            security.cubic_round_bits
        } else {
            0
        },
        &proof.grinding_nonces,
    );
    let (point, [final_claim]) = verify_round_proofs(
        [proof.sumcheck],
        transcript,
        &[initial],
        11 + instance_rounds,
        field,
        &mut boundary,
    )
    .map_err(|error| piop(error.to_string()))?;
    if proof.point.is_some_and(|stored| point != stored)
        || final_claim
            != field.mul(
                &field.mul(&proof.terminal[0], &proof.terminal[1]),
                &proof.terminal[2],
            )
    {
        return Err(piop("compaction leaf terminal identity failed"));
    }
    absorb_field_elements(transcript, &proof.terminal, field);
    Ok((instance_point, point))
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

fn affine(left: F, right: F, point: F, field: &Cfg) -> F {
    field.add(&left, &field.mul(&point, &field.sub(&right, &left)))
}

fn compaction_product_rounds(layout: &FalconSourceLayout) -> usize {
    11 + layout.capacity().trailing_zeros() as usize
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumcheck::outer::OuterInputs;
    use crate::{piop::spartan::falcon1024_ct::verification_trace, transcript::Blake3Transcript};

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    struct SamplingAudit {
        inner: Blake3Transcript,
        prime_draws: usize,
        shared_headers: usize,
    }

    impl SamplingAudit {
        fn new() -> Self {
            Self {
                inner: Blake3Transcript::new(),
                prime_draws: 0,
                shared_headers: 0,
            }
        }
    }

    impl Transcript for SamplingAudit {
        fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
            self.inner.get_challenge()
        }

        fn begin_sampling(&mut self) {
            self.inner.begin_sampling();
        }

        fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
            self.inner.fill_sampling_bytes(output);
        }

        fn absorb_inner(&mut self, bytes: &[u8]) {
            self.prime_draws += usize::from(bytes == b"bitz/shared-prime-sampling/v1");
            self.shared_headers += usize::from(bytes == b"bitz/falcon1024-ct/piop/shared-prime/v4");
            self.inner.absorb_inner(bytes);
        }
    }

    #[test]
    fn supplied_field_piop_roundtrips_without_a_second_prime_draw() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new_shared_prime(3).unwrap();
        let (prime_min, prime_max) = super::super::shared_ring::prime_bounds(100).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut Blake3Transcript::new(),
            prime_min,
            prime_max,
            128,
        )
        .unwrap();
        let mut prover = SamplingAudit::new();
        let proof = prove_falcon_piop_in_field(
            &mut prover,
            &layout,
            &vec![trace.clone(); layout.batch()],
            100,
            &field,
        )
        .unwrap();
        assert_eq!(proof.modulus, field.modulus_u128());
        assert_eq!(prover.prime_draws, 0);
        assert_eq!(prover.shared_headers, 1);
        let mut verifier = SamplingAudit::new();
        verify_falcon_piop_in_field(&mut verifier, &layout, &proof, 100, &field).unwrap();
        assert_eq!(verifier.prime_draws, 0);
        assert_eq!(verifier.shared_headers, 1);
        assert_eq!(
            prover.get_challenge::<u128>(),
            verifier.get_challenge::<u128>()
        );

        let mut changed = proof.clone();
        changed.modulus ^= 2;
        assert!(
            verify_falcon_piop_in_field(&mut SamplingAudit::new(), &layout, &changed, 100, &field,)
                .is_err()
        );
        assert!(verify_falcon_piop(&mut SamplingAudit::new(), &layout, &proof, 100,).is_err());
        let legacy = FalconSourceLayout::new(layout.batch()).unwrap();
        assert!(
            verify_falcon_piop_in_field(&mut SamplingAudit::new(), &legacy, &proof, 100, &field,)
                .is_err()
        );
        let small = field::FpCtx::from_prime_u128(7);
        assert!(validate_shared_field(&layout, 100, &small).is_err());
        let wide = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        assert!(validate_shared_field(&layout, 128, &wide).is_err());
        let legacy_field = sample_field(&mut Blake3Transcript::new()).unwrap();
        assert!(validate_shared_field(&layout, 100, &legacy_field).is_err());
        assert!(validate_shared_field(&layout, 128, &legacy_field).is_ok());
        assert!(validate_shared_field(&layout, 128, &field).is_err());
        assert!(validate_shared_field(&layout, 127, &field).is_err());
    }

    #[test]
    fn shared_128_ring_projection_budget_preserves_native_bound_exactly() {
        use num_bigint::BigUint;

        let q = BigUint::from(12_289u32);
        let extension = q.pow(11);
        let old_denominator = &extension - &q;
        let extension_product = &extension * &old_denominator;
        assert_eq!(super::super::shared_ring::projection_grinding_bits(128), 14);
        for batch in 1usize..=1024 {
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let m = layout.capacity().ilog2() as usize;
            // Instance batching, degree-2046 quotient check, cubic outer
            // sumcheck, and degree-three endpoint batching. No inner sumcheck.
            let ring_numerator = 4 * m + 2_049;
            let baseline_numerator = m + 2_046;
            let preserves_baseline = |grind: usize| {
                let candidate_exponent =
                    super::super::shared_ring::prime_floor_bits(128).unwrap() + grind;
                let baseline_exponent = 125 + 12;
                let scale = candidate_exponent.max(baseline_exponent);
                // Cross-multiply positive denominators, including the E-term
                // increase that projection-only accounting would miss at g=13.
                let candidate = (BigUint::from(ring_numerator) * &old_denominator << scale)
                    + (BigUint::from(20u8) << (scale - candidate_exponent)) * &extension_product;
                let baseline = (BigUint::from(baseline_numerator) * &extension << scale)
                    + (BigUint::from(10u8) << (scale - baseline_exponent)) * &extension_product;
                candidate <= baseline
            };
            assert!(preserves_baseline(14), "batch {batch}");
            assert!((0..14).all(|g| !preserves_baseline(g)), "batch {batch}");
            let legacy = FalconSourceLayout::new(batch).unwrap();
            assert_eq!(layout.linear_stride(), legacy.linear_stride());
            assert_eq!(layout.source_bits(), legacy.source_bits());
        }
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
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
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
    fn shared_composition_meets_both_targets_with_exact_rational_bounds() {
        use num_bigint::BigUint;

        assert_eq!(super::super::shared_ring::projection_grinding_bits(100), 2);
        let extension = BigUint::from(12_289u32).pow(11);
        // A common power-of-two denominator for every configured prime term.
        const SCALE: usize = 144;
        for batch in 1usize..=1024 {
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let d = layout.capacity().ilog2() as usize;
            for target in [100, 128] {
                let schedule = FalconSecuritySchedule::for_layout(target, &layout).unwrap();
                // The existing PCS allocator bounds its total by 2^-(target+2).
                // Six binary components each receive 2^-(target+8): the bridge,
                // two Keccak prefixes, wiring, joint sumcheck, and ring switch.
                // The unchanged sampler contributes at most 2^-144.
                let mut dyadic = BigUint::from(1u8)
                    + (BigUint::from(1u8) << (SCALE - target - 2))
                    + (BigUint::from(6u8) << (SCALE - target - 8));
                for (numerator, bits) in [
                    (d, schedule.norm_instance_bits),
                    (4 * (10 + d), schedule.quadratic_round_bits),
                    (11 + d, schedule.outer_point_bits),
                    (6 * (11 + d), schedule.cubic_round_bits),
                    (2 * (55 + 11 * (d + 1)), schedule.forest_round_bits),
                    (d + 11, schedule.forest_claim_bits),
                    (2048, schedule.fingerprint_bits),
                    (13 + d + batch + 12, schedule.linear_point_bits),
                    (2 * (17 + d), schedule.binding_round_bits),
                    (
                        20,
                        super::super::shared_ring::projection_grinding_bits(target),
                    ),
                ] {
                    let exponent = super::super::shared_ring::prime_floor_bits(target).unwrap()
                        + bits as usize;
                    assert!(exponent <= SCALE);
                    dyadic += BigUint::from(numerator) << (SCALE - exponent);
                }
                let candidate = dyadic * &extension + (BigUint::from(4 * d + 2049) << SCALE);
                let budget = &extension << (SCALE - target);
                assert!(candidate <= budget, "batch {batch}, target {target}");
            }
        }
    }

    #[test]
    fn compile_time_compaction_schedule_matches_solver_for_every_capacity() {
        let largest = FalconSourceLayout::new(1024).unwrap();
        assert_eq!(
            COMPACTION_GRINDING_BITS.len(),
            largest.capacity().ilog2() as usize + 1
        );
        assert!(FalconSourceLayout::new(1025).is_err());
        for d in 0..COMPACTION_GRINDING_BITS.len() {
            assert_eq!(
                compaction_grinding_bits(d),
                solve_compaction_grinding_bits(std::hint::black_box(d)),
                "capacity log {d}"
            );
        }
    }

    #[test]
    fn current_grinding_schedule_meets_each_prime_group_budget() {
        let expected = [
            (16, 16, 14),
            (16, 16, 15),
            (17, 16, 15),
            (17, 16, 16),
            (18, 16, 17),
            (16, 17, 15),
            (16, 17, 16),
            (17, 17, 15),
            (17, 17, 15),
            (17, 17, 16),
            (17, 17, 17),
        ];
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let d = layout.capacity().ilog2() as usize;
            let schedule = FalconSecuritySchedule::for_layout(128, &layout).unwrap();
            assert_eq!(
                (
                    schedule.cubic_round_bits,
                    schedule.forest_round_bits,
                    schedule.forest_claim_bits
                ),
                expected[d]
            );
            for (numerator, bits) in [
                (d, schedule.norm_instance_bits),
                (4 * (10 + d), schedule.quadratic_round_bits),
                (11 + d, schedule.outer_point_bits),
                (2048, schedule.fingerprint_bits),
                (13 + d + batch + 12, schedule.linear_point_bits),
                (2 * (17 + d), schedule.binding_round_bits),
            ] {
                assert!((numerator as u128) << 8 <= 1u128 << bits);
            }
            let denominator = schedule
                .cubic_round_bits
                .max(schedule.forest_round_bits)
                .max(schedule.forest_claim_bits);
            let error = (((6 * (11 + d)) as u128) << (denominator - schedule.cubic_round_bits))
                + (((2 * (55 + 11 * (d + 1))) as u128)
                    << (denominator - schedule.forest_round_bits))
                + (((d + 11) as u128) << (denominator - schedule.forest_claim_bits));
            assert!(error << 8 <= 1u128 << denominator, "batch {batch}");
            let shared_layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let shared = FalconSecuritySchedule::for_layout(128, &shared_layout).unwrap();
            // Target 128 now uses the same field floor and arithmetic
            // schedule for both profiles and versioned joint-forest transcripts.
            assert_eq!(shared, schedule);
            let unground = FalconSecuritySchedule::for_layout(100, &layout).unwrap();
            assert_eq!(
                FalconSecuritySchedule::for_layout(100, &shared_layout).unwrap(),
                unground
            );
            assert_eq!(
                unground,
                FalconSecuritySchedule {
                    norm_instance_bits: 0,
                    outer_point_bits: 0,
                    quadratic_round_bits: 0,
                    cubic_round_bits: 0,
                    forest_round_bits: 0,
                    forest_claim_bits: 0,
                    fingerprint_bits: 0,
                    linear_point_bits: 0,
                    binding_round_bits: 0,
                }
            );
        }
        let layout = FalconSourceLayout::new(1).unwrap();
        assert!(FalconSecuritySchedule::for_layout(127, &layout).is_none());
    }

    #[test]
    fn forest_rejects_a_proof_made_under_either_weaker_grinding_schedule() {
        let layout = FalconSourceLayout::new(32).unwrap();
        let expected = FalconSecuritySchedule::for_layout(128, &layout).unwrap();
        assert_ne!(expected.forest_round_bits, expected.forest_claim_bits);
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let leaves = vec![vec![field.one(); COMPACTION_LEAVES]; 2 * layout.batch()];
        for weaken_rounds in [false, true] {
            let mut weaker = expected;
            if weaken_rounds {
                weaker.forest_round_bits -= 1;
            } else {
                weaker.forest_claim_bits -= 1;
            }
            let (proof, terminals) = prove_product_forest(
                &mut Blake3Transcript::new(),
                leaves.clone(),
                128,
                weaker,
                &field,
            )
            .unwrap();
            verify_product_forest(
                &mut Blake3Transcript::new(),
                &proof,
                &terminals,
                layout.batch(),
                128,
                weaker,
                &field,
            )
            .unwrap();
            assert!(
                verify_product_forest(
                    &mut Blake3Transcript::new(),
                    &proof,
                    &terminals,
                    layout.batch(),
                    128,
                    expected,
                    &field,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejection_rows_match_dense_padded_relation() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let layout = FalconSourceLayout::new(3).unwrap();
        let stride = COMPACTION_LEAVES * layout.capacity();
        let rows = CompactionRows {
            traces: &traces,
            stride,
            field: &field,
        };
        assert_eq!(rows.dimensions(), (stride, stride, stride));
        let mut dense = OuterInputs {
            ax: vec![field.zero(); stride],
            bx: vec![field.zero(); stride],
            cx: vec![field.zero(); stride],
        };
        for (s, trace) in traces.iter().enumerate() {
            for i in 0..HASH_TO_POINT_SAMPLES {
                let q = trace.hash_to_point.quotients[i];
                dense.ax[s * COMPACTION_LEAVES + i] = unsigned(((q >> 2) & 1).into(), &field);
                dense.bx[s * COMPACTION_LEAVES + i] = unsigned((q & 1).into(), &field);
                dense.cx[s * COMPACTION_LEAVES + i] =
                    unsigned(u128::from(!trace.hash_to_point.accepted[i]), &field);
            }
        }
        for i in 0..stride {
            assert_eq!(
                [rows.a(i), rows.b(i), rows.c(i)],
                [dense.ax[i], dense.bx[i], dense.cx[i]]
            );
        }
        let mut dense_transcript = Blake3Transcript::new();
        dense_transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
        let expected = prove_quadratic(
            &mut dense_transcript,
            dense,
            13,
            100,
            security_schedule(&layout, 100).unwrap(),
            &field,
        )
        .unwrap();
        let actual =
            prove_compaction_products(&mut Blake3Transcript::new(), &layout, &traces, 100, &field)
                .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn piop_leaf_roundtrips_and_rejects_tampering() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        for (batch, target) in [(1, 100), (3, 100), (3, 128)] {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let mut prover = Blake3Transcript::new();
            let proof =
                prove_falcon_piop(&mut prover, &layout, &vec![trace.clone(); batch], target)
                    .unwrap();
            let field = field::FpCtx::from_prime_u128(proof.modulus);
            let leaf = &proof.compaction_leaf;
            assert_eq!(
                leaf.point.len(),
                11 + layout.capacity().trailing_zeros() as usize
            );
            assert_eq!(proof.compact_products.point.len(), leaf.point.len());
            let mut verifier = Blake3Transcript::new();
            verify_falcon_piop(&mut verifier, &layout, &proof, target).unwrap();
            assert_eq!(
                squeeze(&mut prover, &field).unwrap(),
                squeeze(&mut verifier, &field).unwrap()
            );
            let reject = |bad: &FalconPiopProof| {
                assert!(
                    verify_falcon_piop(&mut Blake3Transcript::new(), &layout, bad, target).is_err()
                );
            };
            for operand in 0..3 {
                let mut bad = proof.clone();
                let value = &mut bad.compaction_leaf.terminal[operand];
                *value = field.add(value, &field.one());
                reject(&bad);
            }
            let mut bad = proof.clone();
            bad.compaction_leaf.point[0] = field.zero();
            reject(&bad);
            let mut bad = proof.clone();
            bad.compaction_leaf.sumcheck.round_polynomials[0][0] = field.zero();
            reject(&bad);
            let mut bad = proof.clone();
            bad.compaction_leaf.grinding_nonces = vec![0];
            reject(&bad);
            if batch > 1 {
                let mut bad = proof.clone();
                bad.compaction_leaf.instance_point[0] = field.zero();
                reject(&bad);
            }
            let mut bad = proof.clone();
            bad.compaction.candidate = field.zero();
            reject(&bad);
        }
    }

    #[test]
    fn compaction_leaf_terminals_match_masked_weighted_tables() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let layout = FalconSourceLayout::new(3).unwrap();
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let gamma = unsigned(17, &field);
        let rho = unsigned(31, &field);
        let forest_point: Vec<_> = (0..11).map(|i| unsigned(13 + i, &field)).collect();
        let forest_weights = eq_table(&forest_point, &field).unwrap();
        let instance_point = vec![unsigned(17, &field), unsigned(29, &field)];
        let beta = eq_table(&instance_point, &field).unwrap();
        let mut endpoints = [field.one(); 2];
        for (trace, scale) in traces.iter().zip(beta) {
            let (candidate, output) = compaction_leaves(trace, gamma, rho, &field);
            for (value, leaves) in endpoints.iter_mut().zip([candidate, output]) {
                *value = field.add(
                    value,
                    &field.mul(
                        &scale,
                        &field.sub(
                            &weighted_sum(&leaves, &forest_weights, &field),
                            &field.one(),
                        ),
                    ),
                );
            }
        }
        let compaction = CompactionProof {
            instance_point,
            terminal_point: forest_point.clone(),
            candidate: endpoints[0],
            output: endpoints[1],
        };
        let proof = prove_compaction_leaf(
            &mut Blake3Transcript::new(),
            &layout,
            &traces,
            &compaction,
            gamma,
            rho,
            100,
            &field,
        )
        .unwrap();
        verify_compaction_leaf(
            &mut Blake3Transcript::new(),
            &layout,
            &compaction,
            &proof,
            100,
            &field,
        )
        .unwrap();
        let weights = eq_table(&proof.point, &field).unwrap();
        let beta = eq_table(&proof.instance_point, &field).unwrap();
        let mut expected = [field.zero(); 3];
        for (s, trace) in traces.iter().enumerate() {
            for i in 0..HASH_TO_POINT_SAMPLES {
                let hash = &trace.hash_to_point;
                let values = [
                    unsigned(u128::from(hash.accepted[i]), &field),
                    unsigned(u128::from((hash.prefix[i] & 1024) == 0), &field),
                    field.mul(
                        &field.mul(&beta[s], &forest_weights[i]),
                        &field.add(
                            &field.add(
                                &field.sub(&gamma, &field.one()),
                                &field.mul(&rho, &unsigned(hash.prefix[i].into(), &field)),
                            ),
                            &unsigned(hash.remainders[i].into(), &field),
                        ),
                    ),
                ];
                for k in 0..3 {
                    expected[k] = field.add(
                        &expected[k],
                        &field.mul(&weights[s * COMPACTION_LEAVES + i], &values[k]),
                    );
                }
            }
        }
        assert_eq!(proof.terminal, expected);
        let mut bad = compaction;
        bad.candidate = field.add(&bad.candidate, &field.one());
        assert!(
            prove_compaction_leaf(
                &mut Blake3Transcript::new(),
                &layout,
                &traces,
                &bad,
                gamma,
                rho,
                100,
                &field
            )
            .is_err()
        );
    }

    #[test]
    fn weighted_forest_quadratics_match_direct_evaluation_and_degenerate_weights() {
        let field = field::FpCtx::from_prime_u128(7);
        let left: Vec<_> = [1, 4, 0, 2, 6, 3, 2, 2]
            .map(|v| unsigned(v, &field))
            .to_vec();
        let right: Vec<_> = [0, 5, 4, 3, 1, 1, 6, 0]
            .map(|v| unsigned(v, &field))
            .to_vec();
        let future = forest_weights(
            &[
                ForestWeight::Equality(field.zero()),
                ForestWeight::Difference,
            ],
            &field,
        );
        let polynomial = weighted_product_polynomial(&left, &right, &future, &field);
        for x in 0..7 {
            let x = unsigned(x, &field);
            let expected = future.iter().enumerate().fold(field.zero(), |sum, (i, w)| {
                field.add(
                    &sum,
                    &field.mul(
                        w,
                        &field.mul(
                            &affine(left[2 * i], left[2 * i + 1], x, &field),
                            &affine(right[2 * i], right[2 * i + 1], x, &field),
                        ),
                    ),
                )
            });
            assert_eq!(evaluate_quadratic(&polynomial, x, &field), expected);
        }
        // Exhaust all quadratics and every equality weight, including 0, 1,
        // and 1/2; signed weights have endpoint sum zero but remain invertible
        // in the chosen coefficient encoding.
        for a0 in 0..7 {
            for a1 in 0..7 {
                for a2 in 0..7 {
                    let p = [a0, a1, a2].map(|v| unsigned(v, &field));
                    for weight in (0..7)
                        .map(|r| ForestWeight::Equality(unsigned(r, &field)))
                        .chain([ForestWeight::Difference])
                    {
                        let [w0, w1] = weight.endpoints(&field);
                        let c = field.add(
                            &field.mul(&w0, &p[0]),
                            &field.mul(&w1, &evaluate_quadratic(&p, field.one(), &field)),
                        );
                        assert_eq!(weight.recover(c, weight.encode(p), &field), p);
                    }
                    if p != [field.zero(); 3] {
                        assert!(
                            (0..7)
                                .filter(|&r| evaluate_quadratic(&p, unsigned(r, &field), &field)
                                    == field.zero())
                                .count()
                                <= 2
                        );
                    }
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
            instance_point,
            instance_nonce: None,
            claims,
            slack,
            sumchecks: output.proofs,
            terminal: output.terminal_evaluations,
            point: output.point,
            grinding_nonces: Vec::new(),
        };
        let error = verify_norm(&mut fresh_transcript(), &layout, &proof, 100, &field).unwrap_err();
        assert_eq!(error, piop("norm claim does not equal the Falcon bound"));
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
            let proof = prove_norm(
                &mut Blake3Transcript::new(),
                &layout,
                &traces,
                target,
                &field,
            )
            .unwrap();
            assert_eq!(proof.instance_nonce.is_some(), target == 128);
            assert_ne!(proof.terminal[0][0], proof.terminal[0][1]);
            let weights = eq_table(&proof.instance_point, &field).unwrap();
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
            assert_eq!(reference, [proof.claims[0], proof.claims[1], proof.slack]);
            verify_norm(
                &mut Blake3Transcript::new(),
                &layout,
                &proof,
                target,
                &field,
            )
            .unwrap();
            let mut bad = proof.clone();
            bad.instance_point[0] = field.add(&bad.instance_point[0], &field.one());
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

    #[test]
    fn algebraic_piop_roundtrips() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let mut prover = Blake3Transcript::new();
        let proof = prove_falcon_piop(&mut prover, &layout, &[trace], 100).unwrap();
        let mut verifier = Blake3Transcript::new();
        verify_falcon_piop(&mut verifier, &layout, &proof, 100).unwrap();
    }

    #[test]
    fn algebraic_piop_roundtrips_at_128_bits() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let mut prover = Blake3Transcript::new();
        let proof = prove_falcon_piop(&mut prover, &layout, &[trace], 128).unwrap();
        assert!(!proof.norm.grinding_nonces.is_empty());
        assert!(proof.compact_products.point_nonce.is_some());
        let mut verifier = Blake3Transcript::new();
        verify_falcon_piop(&mut verifier, &layout, &proof, 128).unwrap();
        assert_eq!(
            proof
                .compaction_forest
                .layers
                .iter()
                .map(|layer| layer.grinding_nonces.len())
                .sum::<usize>(),
            66
        );
        assert!(
            proof
                .compaction_forest
                .layers
                .iter()
                .all(|layer| layer.line_nonce.is_some())
        );
        assert!(proof.compaction_forest.root_nonce.is_none());
        let mut bad = proof.clone();
        bad.compaction_forest.root_nonce = Some(0);
        assert!(verify_falcon_piop(&mut Blake3Transcript::new(), &layout, &bad, 128).is_err());
        let mut bad = proof.clone();
        bad.compaction_forest.layers[0].line_nonce = None;
        assert!(verify_falcon_piop(&mut Blake3Transcript::new(), &layout, &bad, 128).is_err());
        let mut bad = proof.clone();
        bad.compact_products.point_nonce = None;
        assert!(verify_falcon_piop(&mut Blake3Transcript::new(), &layout, &bad, 128).is_err());
        let mut bad = proof;
        bad.compaction_forest.layers[1].grinding_nonces.clear();
        assert!(verify_falcon_piop(&mut Blake3Transcript::new(), &layout, &bad, 128).is_err());
    }

    #[test]
    fn forest_batches_distinct_trees_and_rejects_tampering() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        for batch in [1usize, 3, 32] {
            let security =
                FalconSecuritySchedule::for_layout(100, &FalconSourceLayout::new(batch).unwrap())
                    .unwrap();
            let mut leaves = Vec::new();
            for instance in 0..batch {
                let candidate: Vec<_> = (0..COMPACTION_LEAVES)
                    .map(|i| unsigned((2 + i + 3 * instance) as u128, &field))
                    .collect();
                let output = candidate.iter().rev().copied().collect();
                leaves.extend([candidate, output]);
            }
            let original = leaves.clone();
            let mut prover = Blake3Transcript::new();
            let (forest, compaction) =
                prove_product_forest(&mut prover, leaves, 100, security, &field).unwrap();
            let mut verifier = Blake3Transcript::new();
            verify_product_forest(
                &mut verifier,
                &forest,
                &compaction,
                batch,
                100,
                security,
                &field,
            )
            .unwrap();
            assert_eq!(
                squeeze(&mut prover, &field).unwrap(),
                squeeze(&mut verifier, &field).unwrap()
            );
            let local = eq_table(&compaction.terminal_point, &field).unwrap();
            let instances = eq_table(&compaction.instance_point, &field).unwrap();
            for side in 0..2 {
                let expected = (0..batch).fold(field.one(), |sum, s| {
                    field.add(
                        &sum,
                        &field.mul(
                            &instances[s],
                            &field.sub(
                                &weighted_sum(&original[2 * s + side], &local, &field),
                                &field.one(),
                            ),
                        ),
                    )
                });
                assert_eq!([compaction.candidate, compaction.output][side], expected);
            }
            let rounds = 55 + 11 * (batch.next_power_of_two().ilog2() as usize + 1);
            assert_eq!(
                forest
                    .layers
                    .iter()
                    .map(|l| l.round_polynomials.len())
                    .sum::<usize>(),
                rounds
            );
            let reject = |forest: &PrimeProductForestProof, endpoints: &CompactionProof| {
                assert!(
                    verify_product_forest(
                        &mut Blake3Transcript::new(),
                        forest,
                        endpoints,
                        batch,
                        100,
                        security,
                        &field
                    )
                    .is_err()
                );
            };
            let mut bad = compaction.clone();
            bad.candidate = field.add(&bad.candidate, &field.one());
            reject(&forest, &bad);
            let mut bad = compaction.clone();
            std::mem::swap(&mut bad.candidate, &mut bad.output);
            reject(&forest, &bad);
            let mut bad = compaction.clone();
            bad.terminal_point[0] = field.add(&bad.terminal_point[0], &field.one());
            reject(&forest, &bad);
            let mut bad = forest.clone();
            bad.layers.swap(1, 2);
            reject(&bad, &compaction);
            let mut bad = forest.clone();
            bad.layers[2].evaluations.swap(0, 1);
            reject(&bad, &compaction);
            let mut bad = forest.clone();
            bad.layers[0].line_nonce = Some(0);
            reject(&bad, &compaction);
            let mut bad = forest.clone();
            bad.root_nonce = Some(0);
            reject(&bad, &compaction);
            let mut bad = forest.clone();
            bad.layers[0].round_polynomials[0][0] =
                field.add(&bad.layers[0].round_polynomials[0][0], &field.one());
            reject(&bad, &compaction);
            let mut bad = forest.clone();
            bad.layers[0].round_polynomials.pop();
            reject(&bad, &compaction);
        }
    }

    #[test]
    fn joint_forest_batch1024_payload_is_5984_bytes() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let batch = 1024;
        let security =
            FalconSecuritySchedule::for_layout(100, &FalconSourceLayout::new(batch).unwrap())
                .unwrap();
        let leaves = vec![vec![field.one(); COMPACTION_LEAVES]; 2 * batch];
        let (forest, endpoints) =
            prove_product_forest(&mut Blake3Transcript::new(), leaves, 100, security, &field)
                .unwrap();
        verify_product_forest(
            &mut Blake3Transcript::new(),
            &forest,
            &endpoints,
            batch,
            100,
            security,
            &field,
        )
        .unwrap();
        let rounds: usize = forest
            .layers
            .iter()
            .map(|layer| layer.round_polynomials.len())
            .sum();
        assert_eq!(rounds, 176);
        assert_eq!(16 * (2 * rounds + 2 * forest.layers.len()), 5_984);
        assert_eq!([endpoints.candidate, endpoints.output], [field.one(); 2]);
        assert!(forest.root_nonce.is_none());
        assert!(
            forest
                .layers
                .iter()
                .all(|layer| layer.grinding_nonces.is_empty() && layer.line_nonce.is_none())
        );
        // The separately stored candidate/output split adds another 32 bytes.
        assert_eq!(16 * (2 * rounds + 2 * forest.layers.len() + 2), 6_016);
    }

    #[test]
    fn joint_forest_rejects_cross_signature_cancellation() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let security =
            FalconSecuritySchedule::for_layout(100, &FalconSourceLayout::new(2).unwrap()).unwrap();
        let mut first = vec![field.one(); COMPACTION_LEAVES];
        first[0] = unsigned(2, &field);
        let mut second = vec![field.one(); COMPACTION_LEAVES];
        second[0] = unsigned(3, &field);
        // The product across signatures matches, but neither individual pair
        // matches. The signed root reduction must retain this difference.
        let swapped = vec![first.clone(), second.clone(), second, first];
        assert!(
            prove_product_forest(&mut Blake3Transcript::new(), swapped, 100, security, &field)
                .is_err()
        );
    }

    #[test]
    fn forest_equality_batching_preserves_nonzero_six_tree_error_vectors() {
        // Six live trees occupy a three-variable cube with two zero entries.
        // Boolean evaluations recover every error coordinate, so padding and
        // arbitrary cancellation between live errors cannot make its MLE zero.
        let field = field::FpCtx::from_prime_u128(7);
        let errors: Vec<Vec<F>> = (0..6)
            .map(|bad| {
                (0..6)
                    .map(|i| unsigned(u128::from(i == bad), &field))
                    .collect()
            })
            .chain([
                vec![1, 6, 2, 5, 3, 4]
                    .into_iter()
                    .map(|v| unsigned(v, &field))
                    .collect(),
                vec![field.one(); 6],
            ])
            .collect();
        for error in errors {
            assert!(error.iter().any(|value| *value != field.zero()));
            for vertex in 0..8 {
                let point: Vec<_> = (0..3)
                    .map(|i| unsigned((vertex >> i & 1) as u128, &field))
                    .collect();
                let scales = eq_table(&point, &field).unwrap();
                assert_eq!(
                    weighted_sum(&error, &scales[..6], &field),
                    error.get(vertex).copied().unwrap_or(field.zero())
                );
            }
            // Enumerate a complete small field, including challenge collisions,
            // and check the total-degree-three Schwartz–Zippel bound directly.
            let mut zeros = 0;
            for x in 0..7 {
                for y in 0..7 {
                    for z in 0..7 {
                        let point = [x, y, z].map(|value| unsigned(value, &field));
                        let scales = eq_table(&point, &field).unwrap();
                        zeros +=
                            usize::from(weighted_sum(&error, &scales[..6], &field) == field.zero());
                    }
                }
            }
            assert!(zeros <= 3 * 7 * 7);
        }
    }

    #[test]
    fn forest_common_line_has_at_most_one_root_for_a_nonzero_error_vector() {
        let field = field::FpCtx::from_prime_u128(7);
        // Exhaust all endpoint errors in two distinct coordinates of a six-tree
        // vector. Errors may have different roots or share one root. Acceptance
        // needs every coordinate to vanish, so their union of roots is irrelevant.
        for values in 1usize..7usize.pow(4) {
            let mut remaining = values;
            let endpoints: [F; 4] = std::array::from_fn(|_| {
                let value = unsigned((remaining % 7) as u128, &field);
                remaining /= 7;
                value
            });
            let mut left = [field.zero(); 6];
            let mut right = [field.zero(); 6];
            [left[1], left[5], right[1], right[5]] = endpoints;
            let common_roots = (0..7)
                .filter(|&challenge| {
                    let challenge = unsigned(challenge, &field);
                    left.iter().zip(&right).all(|(&left, &right)| {
                        affine(left, right, challenge, &field) == field.zero()
                    })
                })
                .count();
            assert!(common_roots <= 1);
        }
    }

    #[test]
    fn shared_compaction_ranks_match_direct_leaf_formulas() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let original = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let mut boundary_cases = original.clone();
        for i in 0..HASH_TO_POINT_SAMPLES {
            boundary_cases.hash_to_point.accepted[i] = i % 5 != 4;
            boundary_cases.hash_to_point.prefix[i] = [0, 1023, 1024, 1311, 1023][i % 5];
            boundary_cases.hash_to_point.remainders[i] = (i * 41 % 12289) as u16;
        }
        let challenge_pairs = [
            [field.zero(), field.zero()],
            [field.one(), field.one()],
            [unsigned(73, &field), unsigned(91, &field)],
            [
                unsigned(field.modulus_u128() - 2, &field),
                unsigned(field.modulus_u128() - 1, &field),
            ],
        ];
        for trace in [&original, &boundary_cases] {
            for [gamma, rho] in challenge_pairs {
                let ranks = compaction_ranks(gamma, rho, &field);
                let (candidate, output) = compaction_leaves_with_ranks(trace, &ranks, &field);
                let mut expected_candidate = vec![field.one(); COMPACTION_LEAVES];
                let mut expected_output = expected_candidate.clone();
                for (i, expected) in expected_candidate
                    .iter_mut()
                    .take(HASH_TO_POINT_SAMPLES)
                    .enumerate()
                {
                    let prefix = trace.hash_to_point.prefix[i];
                    let selected = unsigned(
                        u128::from(trace.hash_to_point.accepted[i] && prefix < 1024),
                        &field,
                    );
                    let term = field.add(
                        &field.add(
                            &field.sub(&gamma, &field.one()),
                            &field.mul(&rho, &unsigned(prefix.into(), &field)),
                        ),
                        &unsigned(trace.hash_to_point.remainders[i].into(), &field),
                    );
                    *expected = field.add(&field.one(), &field.mul(&selected, &term));
                }
                for (rank, expected) in expected_output.iter_mut().take(N).enumerate() {
                    *expected = field.add(
                        &field.add(&gamma, &field.mul(&rho, &unsigned(rank as u128, &field))),
                        &unsigned(trace.hash_to_point.point[rank].into(), &field),
                    );
                }
                assert_eq!(candidate, expected_candidate);
                assert_eq!(output, expected_output);
            }
        }
    }
}
