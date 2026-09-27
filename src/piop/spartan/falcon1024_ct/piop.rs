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
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{InitialClaims, prove_batched_inner_sumcheck},
        outer::{
            OuterClaim, OuterEvaluations, OuterRows, prove_outer_sumcheck, verify_outer_sumcheck,
        },
        proof::{
            recover_full_round_polynomial_and_sample_next_challenge_with_boundary,
            validate_field_elements,
        },
    },
    transcript::traits::Transcript,
};

use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N, keccak::ROUND_CONSTANTS,
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

const PRIME_MIN: u128 = 1u128 << 125;
const PRIME_MAX: u128 = (1u128 << 126) - 1;
const KECCAK_BITS: usize = 20 * 24 * 25 * 64;
const KECCAK_GATE_STRIDE: usize = 1 << 20;
const COMPACTION_LEAVES: usize = 1 << 11;
// Each 128-bit block has about four bits of margin before the union bound
// across norm, cubic-product, fingerprint, linear-collapse and final-binding
// failures. The occurrence counts use the maximum supported batch of 32.
const QUADRATIC_GRINDING_BITS: u32 = 13;
// The per-instance norm residual has degree at most five at batch 32.
const NORM_INSTANCE_GRINDING_BITS: u32 = 14;
const OUTER_POINT_GRINDING_BITS: u32 = 14;
const CUBIC_GRINDING_BITS: u32 = 21;
const FINGERPRINT_GRINDING_BITS: u32 = 23;
pub(super) const LINEAR_POINT_GRINDING_BITS: u32 = 14;
pub(super) const BINDING_GRINDING_BITS: u32 = 13;

/// Proof-of-work schedule used to lift the 126-bit projection field to the
/// requested computational soundness target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecuritySchedule {
    pub norm_instance_bits: u32,
    pub outer_point_bits: u32,
    pub quadratic_round_bits: u32,
    pub cubic_round_bits: u32,
    pub fingerprint_bits: u32,
    pub linear_point_bits: u32,
    pub binding_round_bits: u32,
}

impl FalconSecuritySchedule {
    /// Batch-specific hybrid schedule. At target 128, each of seven prime
    /// reduction groups receives at most 2^-(target+5) work-normalized error.
    /// Target 100 needs no grinding: its whole unground prime bound already
    /// exceeds 103 bits, including at the maximum supported batch.
    pub const fn for_layout(target_bits: usize, layout: &FalconSourceLayout) -> Option<Self> {
        if target_bits != 128 || !layout.is_hybrid() {
            return Self::for_target(target_bits);
        }
        let d = layout.capacity().trailing_zeros() as usize;
        let batch = layout.batch();
        Some(Self {
            // Independent instance batching for norms and candidate leaves.
            norm_instance_bits: component_grinding_bits(target_bits, 2 * d),
            quadratic_round_bits: component_grinding_bits(target_bits, 4 * (10 + d)),
            outer_point_bits: component_grinding_bits(target_bits, 11 + d),
            // One shared difficulty covers rejection and leaf cubic rounds and the complete
            // product forest: 55 cubic rounds, ten tree-combination draws,
            // and eleven line draws for each of the 2*batch trees.
            cubic_round_bits: component_grinding_bits(
                target_bits,
                6 * (11 + d) + 165 + 10 * (2 * batch - 1) + 22 * batch,
            ),
            fingerprint_bits: component_grinding_bits(target_bits, 2048 * batch),
            linear_point_bits: component_grinding_bits(target_bits, 13 + d + batch + 12),
            binding_round_bits: component_grinding_bits(target_bits, 2 * (17 + d)),
        })
    }

    pub const fn for_target(target_bits: usize) -> Option<Self> {
        match target_bits {
            100 => Some(Self {
                norm_instance_bits: 0,
                outer_point_bits: 0,
                quadratic_round_bits: 0,
                cubic_round_bits: 0,
                fingerprint_bits: 0,
                linear_point_bits: 0,
                binding_round_bits: 0,
            }),
            128 => Some(Self {
                norm_instance_bits: NORM_INSTANCE_GRINDING_BITS,
                outer_point_bits: OUTER_POINT_GRINDING_BITS,
                quadratic_round_bits: QUADRATIC_GRINDING_BITS,
                cubic_round_bits: CUBIC_GRINDING_BITS,
                fingerprint_bits: FINGERPRINT_GRINDING_BITS,
                linear_point_bits: LINEAR_POINT_GRINDING_BITS,
                binding_round_bits: BINDING_GRINDING_BITS,
            }),
            _ => None,
        }
    }
}

const fn component_grinding_bits(target_bits: usize, numerator: usize) -> u32 {
    if numerator == 0 {
        return 0;
    }
    let ceil_log2 = usize::BITS - (numerator - 1).leading_zeros();
    (target_bits as u32 + 5 + ceil_log2).saturating_sub(125)
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
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-batch/v2";
}

struct ForestLineGrinding;
impl GrindingDomain for ForestLineGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-line/v2";
}

struct ForestRoundGrinding;
impl GrindingDomain for ForestRoundGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/forest-round/v2";
}

struct LeafInstanceGrinding;
impl GrindingDomain for LeafInstanceGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/leaf-instances/v1";
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
    /// Absent only for the root's direct two-child multiplication.
    pub sumcheck: Option<SumcheckProof<F, 4>>,
    /// Candidate then output for each signature, at one shared point.
    pub evaluations: Vec<[F; 2]>,
    pub grinding_nonces: Vec<u64>,
    pub batching_nonce: Option<u64>,
    pub line_nonce: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeProductForestProof {
    pub layers: Vec<ProductForestLayerProof>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeProductTreeProof {
    pub root: F,
    /// Point and value of the original leaf MLE after all layer reductions.
    pub terminal_point: Vec<F>,
    pub terminal_claim: F,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionProof {
    pub candidate: PrimeProductTreeProof,
    pub output: PrimeProductTreeProof,
}

/// Authenticates the candidate forest leaves without committed selected-value
/// columns. Operand C includes the instance and forest evaluation weights;
/// it is the MLE of that weighted table, not a product of operand MLEs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionLeafProof {
    pub instance_point: Vec<F>,
    pub instance_nonce: Option<u64>,
    pub sumcheck: SumcheckProof<F, 4>,
    /// MLE evaluations of 1-d, 1-P_10, and beta*eq(forest)*(gamma-1+rho*P+r).
    /// All three tables vanish on padded candidates and signatures.
    pub terminal: [F; 3],
    /// Local candidate coordinates first, followed by instance coordinates.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

/// Proof through the norm, Keccak chi, compaction-product, and compaction
/// witness-product layers.  The terminal claims must subsequently be
/// collapsed and opened against the source commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FalconPiopProof {
    pub modulus: u128,
    pub norm: NormProof,
    pub keccak_chi: Option<QuadraticRelationProof>,
    pub compact_products: QuadraticRelationProof,
    pub fingerprint_nonce: Option<u64>,
    pub compaction_gamma: F,
    pub compaction_rank_scale: F,
    pub compaction: Vec<CompactionProof>,
    pub compaction_forest: PrimeProductForestProof,
    pub compaction_leaf: Option<CompactionLeafProof>,
}

/// Proves the nonlinear Falcon relation layers.  `target_bits` controls the
/// per-round grinding needed above the 126-bit prime-field ceiling: target
/// 100 uses none. Target 128 uses a conservative schedule that accounts for
/// all supported batch-32 sumchecks and the degree-2048 fingerprints.
pub fn prove_falcon_piop(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
) -> Result<FalconPiopProof, FalconError> {
    validate_inputs(layout, traces, target_bits)?;
    bind_header(transcript, layout, target_bits);
    let field = sample_field(transcript)?;

    let norm = prove_norm(transcript, layout, traces, target_bits, &field)
        .map_err(|error| piop(format!("norm: {error}")))?;
    let keccak_chi = if layout.is_hybrid() {
        None
    } else {
        Some(
            prove_keccak_chi(transcript, layout, traces, target_bits, &field)
                .map_err(|error| piop(format!("keccak chi: {error}")))?,
        )
    };
    let compact_products =
        prove_compaction_products(transcript, layout, traces, target_bits, &field)
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
    let gamma = squeeze(transcript, &field)?;
    let rank_scale = squeeze(transcript, &field)?;
    let mut leaves = Vec::with_capacity(2 * layout.batch());
    for trace in traces {
        let (candidate, output) = compaction_leaves(trace, gamma, rank_scale, &field);
        leaves.extend([candidate, output]);
    }
    let (compaction_forest, compaction) = prove_product_forest(
        transcript,
        leaves,
        target_bits,
        security_schedule(layout, target_bits)?,
        &field,
    )?;
    let compaction_leaf = if layout.is_hybrid() {
        Some(prove_compaction_leaf(
            transcript,
            layout,
            traces,
            &compaction,
            gamma,
            rank_scale,
            target_bits,
            &field,
        )?)
    } else {
        None
    };

    Ok(FalconPiopProof {
        modulus: field.modulus_u128(),
        norm,
        keccak_chi,
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
    if !matches!(target_bits, 100 | 128) || proof.compaction.len() != layout.batch() {
        return Err(piop("invalid PIOP shape"));
    }
    bind_header(transcript, layout, target_bits);
    let field = sample_field(transcript)?;
    if field.modulus_u128() != proof.modulus {
        return Err(piop("transcript prime mismatch"));
    }
    verify_norm(transcript, layout, &proof.norm, target_bits, &field)?;
    match (layout.is_hybrid(), &proof.keccak_chi) {
        (false, Some(keccak_chi)) => {
            transcript.absorb_slice(b"bitz/falcon1024-ct/keccak-chi/v1");
            verify_quadratic(
                transcript,
                keccak_rounds(layout),
                keccak_chi,
                target_bits,
                security_schedule(layout, target_bits)?,
                &field,
            )?;
        }
        (true, None) => {}
        _ => return Err(piop("Keccak proof does not match the source layout")),
    }
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    verify_quadratic(
        transcript,
        compaction_product_rounds(layout),
        &proof.compact_products,
        target_bits,
        security_schedule(layout, target_bits)?,
        &field,
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
    let gamma = squeeze(transcript, &field)?;
    let rank_scale = squeeze(transcript, &field)?;
    if gamma != proof.compaction_gamma || rank_scale != proof.compaction_rank_scale {
        return Err(piop("compaction fingerprint challenge mismatch"));
    }
    verify_product_forest(
        transcript,
        &proof.compaction_forest,
        &proof.compaction,
        target_bits,
        security_schedule(layout, target_bits)?,
        &field,
    )?;
    match (layout.is_hybrid(), &proof.compaction_leaf) {
        (true, Some(leaf)) => verify_compaction_leaf(
            transcript,
            layout,
            &proof.compaction,
            leaf,
            target_bits,
            &field,
        )?,
        (false, None) => {}
        _ => {
            return Err(piop(
                "compaction leaf proof does not match the source layout",
            ));
        }
    }
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
    transcript.absorb_slice(if layout.is_hybrid() {
        b"bitz/falcon1024-ct/piop/native-ring/v1"
    } else {
        b"bitz/falcon1024-ct/piop/v3"
    });
    transcript.absorb_slice(&[u8::from(layout.is_hybrid())]);
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
}

fn sample_field(transcript: &mut impl Transcript) -> Result<Cfg, FalconError> {
    crate::ext_proj::sample_prime_context(transcript, PRIME_MIN, PRIME_MAX, 128)
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
    let mut claims = [zero; 2];
    let mut slack = zero;
    for (instance, trace) in traces.iter().enumerate() {
        for i in 0..N {
            let index = instance * N + i;
            s1[index] = signed(i128::from(trace.s1[i]), field);
            s2[index] = signed(i128::from(trace.signature.s2[i]), field);
            weighted_s1[index] = field.mul(&instance_weights[instance], &s1[index]);
            weighted_s2[index] = field.mul(&instance_weights[instance], &s2[index]);
            claims[0] = field.add(&claims[0], &field.mul(&weighted_s1[index], &s1[index]));
            claims[1] = field.add(&claims[1], &field.mul(&weighted_s2[index], &s2[index]));
        }
        slack = field.add(
            &slack,
            &field.mul(
                &instance_weights[instance],
                &unsigned(u128::from(trace.norm_slack), field),
            ),
        );
    }
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
    if instance_point != proof.instance_point {
        return Err(piop("norm instance point mismatch"));
    }
    validate_field_elements(&[proof.claims[0], proof.claims[1], proof.slack], field)
        .map_err(|error| piop(error.to_string()))?;
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
    if field.add(&field.add(&proof.claims[0], &proof.claims[1]), &proof.slack) != expected {
        return Err(piop("norm claim does not equal the Falcon bound"));
    }
    absorb_field_elements(
        transcript,
        &[proof.claims[0], proof.claims[1], proof.slack],
        field,
    );
    let rounds = 10 + layout.capacity().trailing_zeros() as usize;
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<QuadraticGrinding>::new(
            security_schedule(layout, target_bits)?.quadratic_round_bits,
            &proof.grinding_nonces,
        );
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.sumchecks[0], &proof.sumchecks[1]],
            transcript,
            &proof.claims,
            rounds,
            field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.sumchecks[0], &proof.sumchecks[1]],
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
    if point != proof.point {
        return Err(piop("norm terminal point mismatch"));
    }
    for (claim, terminal) in final_claims.iter().zip(proof.terminal.iter()) {
        if *claim != field.mul(&terminal[0], &terminal[1]) {
            return Err(piop("norm terminal identity failed"));
        }
    }
    absorb_field_elements(transcript, &proof.terminal.concat(), field);
    Ok(())
}

fn prove_keccak_chi(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<QuadraticRelationProof, FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/keccak-chi/v1");
    let rows = KeccakRows {
        traces,
        half: KECCAK_GATE_STRIDE * layout.capacity(),
        field,
        two_inv: unsigned((field.modulus_u128() + 1) / 2, field),
    };
    prove_quadratic(
        transcript,
        rows,
        keccak_rounds(layout),
        target_bits,
        security_schedule(layout, target_bits)?,
        field,
    )
}

/// Read the original row operands from packed trace words. Only the first
/// challenge fold allocates field tables; padding never needs source storage.
struct KeccakRows<'a> {
    traces: &'a [FalconVerificationTrace],
    half: usize,
    field: &'a Cfg,
    two_inv: F,
}

impl KeccakRows<'_> {
    fn gate(&self, row: usize) -> Option<(&super::keccak::KeccakTrace, usize, usize, bool)> {
        let xor = row >= self.half;
        let index = row % self.half;
        let gate = index % KECCAK_GATE_STRIDE;
        let trace = self.traces.get(index / KECCAK_GATE_STRIDE)?;
        (gate < KECCAK_BITS).then_some((&trace.hash_to_point.shake, gate >> 6, gate & 63, xor))
    }
}

impl OuterRows for KeccakRows<'_> {
    type AB = F;
    type C = F;

    fn dimensions(&self) -> (usize, usize, usize) {
        (2 * self.half, 2 * self.half, 2 * self.half)
    }

    fn a(&self, row: usize) -> F {
        let Some((shake, word, bit, xor)) = self.gate(row) else {
            return self.field.zero();
        };
        if xor {
            bit_field(shake.chi_inputs[word], bit, self.field)
        } else {
            let neighbor = word - word % 5 + (word + 1) % 5;
            self.field.sub(
                &self.field.one(),
                &bit_field(shake.chi_inputs[neighbor], bit, self.field),
            )
        }
    }

    fn b(&self, row: usize) -> F {
        let Some((shake, word, bit, xor)) = self.gate(row) else {
            return self.field.zero();
        };
        let value = if xor {
            shake.chi_ands[word]
        } else {
            shake.chi_inputs[word - word % 5 + (word + 2) % 5]
        };
        bit_field(value, bit, self.field)
    }

    fn c(&self, row: usize) -> F {
        let Some((shake, word, bit, xor)) = self.gate(row) else {
            return self.field.zero();
        };
        let z = bit_field(shake.chi_ands[word], bit, self.field);
        if !xor {
            return z;
        }
        let round = (word / 25) % 24;
        let y = shake.round_states[word]
            ^ if word % 25 == 0 {
                ROUND_CONSTANTS[round]
            } else {
                0
            };
        self.field.mul(
            &self.field.sub(
                &self
                    .field
                    .add(&bit_field(shake.chi_inputs[word], bit, self.field), &z),
                &bit_field(y, bit, self.field),
            ),
            &self.two_inv,
        )
    }
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
        rejection_only: layout.is_hybrid(),
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

/// Four word relations per candidate: selection, selected rank, selected
/// remainder, and rejection. All words already have bounded binary source
/// encodings, so word equality is equivalent to the former bitwise products.
/// Original operands stay packed until the outer sumcheck's first fold.
struct CompactionRows<'a> {
    traces: &'a [FalconVerificationTrace],
    stride: usize,
    rejection_only: bool,
    field: &'a Cfg,
}

impl CompactionRows<'_> {
    fn candidate(&self, row: usize) -> Option<(&super::HashToPointTrace, usize, usize)> {
        let block = if self.rejection_only {
            3
        } else {
            row / self.stride
        };
        let index = row % self.stride;
        let candidate = index % COMPACTION_LEAVES;
        let trace = self.traces.get(index / COMPACTION_LEAVES)?;
        (candidate < HASH_TO_POINT_SAMPLES).then_some((&trace.hash_to_point, candidate, block))
    }
}

impl OuterRows for CompactionRows<'_> {
    type AB = F;
    type C = F;

    fn dimensions(&self) -> (usize, usize, usize) {
        let rows = if self.rejection_only {
            self.stride
        } else {
            4 * self.stride
        };
        (rows, rows, rows)
    }

    fn a(&self, row: usize) -> F {
        let Some((hash, i, block)) = self.candidate(row) else {
            return self.field.zero();
        };
        let value = match block {
            0 => u128::from(hash.accepted[i]),
            1 | 2 => u128::from(hash.accepted[i] && hash.prefix[i] < N as u16),
            3 => u128::from((hash.quotients[i] >> 2) & 1),
            _ => unreachable!(),
        };
        unsigned(value, self.field)
    }

    fn b(&self, row: usize) -> F {
        let Some((hash, i, block)) = self.candidate(row) else {
            return self.field.zero();
        };
        let value = match block {
            0 => u128::from(1 - ((hash.prefix[i] >> 10) & 1)),
            1 => u128::from(hash.prefix[i]),
            2 => u128::from(hash.remainders[i]),
            3 => u128::from(hash.quotients[i] & 1),
            _ => unreachable!(),
        };
        unsigned(value, self.field)
    }

    fn c(&self, row: usize) -> F {
        let Some((hash, i, block)) = self.candidate(row) else {
            return self.field.zero();
        };
        let selected = hash.accepted[i] && hash.prefix[i] < N as u16;
        let value = match block {
            0 => u128::from(selected),
            1 => {
                if selected {
                    u128::from(hash.prefix[i])
                } else {
                    0
                }
            }
            2 => {
                if selected {
                    u128::from(hash.remainders[i])
                } else {
                    0
                }
            }
            3 => u128::from(!hash.accepted[i]),
            _ => unreachable!(),
        };
        unsigned(value, self.field)
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
    let output = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<CubicGrinding>::new(
            security.cubic_round_bits,
            &proof.grinding_nonces,
        );
        verify_outer_sumcheck(
            field,
            transcript,
            zero,
            &tau,
            &proof.sumcheck,
            proof.terminal,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        verify_outer_sumcheck(
            field,
            transcript,
            zero,
            &tau,
            &proof.sumcheck,
            proof.terminal,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    if output.point != proof.point {
        return Err(piop("quadratic terminal point mismatch"));
    }
    Ok(())
}

fn compaction_leaves(
    trace: &FalconVerificationTrace,
    gamma: F,
    rank_scale: F,
    field: &Cfg,
) -> (Vec<F>, Vec<F>) {
    let one = field.one();
    let zero = field.zero();
    let mut candidate = vec![one; COMPACTION_LEAVES];
    let mut output = vec![one; COMPACTION_LEAVES];
    for i in 0..HASH_TO_POINT_SAMPLES {
        let selected = trace.hash_to_point.accepted[i] && trace.hash_to_point.prefix[i] < 1024;
        let selector = if selected { one } else { zero };
        let selected_prefix = if selected {
            unsigned(u128::from(trace.hash_to_point.prefix[i]), field)
        } else {
            zero
        };
        let selected_remainder = if selected {
            unsigned(u128::from(trace.hash_to_point.remainders[i]), field)
        } else {
            zero
        };
        candidate[i] = field.add(
            &field.add(
                &field.add(&one, &field.mul(&selector, &field.sub(&gamma, &one))),
                &field.mul(&rank_scale, &selected_prefix),
            ),
            &selected_remainder,
        );
    }
    for (rank, leaf) in output.iter_mut().take(N).enumerate() {
        *leaf = field.add(
            &field.add(
                &gamma,
                &field.mul(&rank_scale, &unsigned(rank as u128, field)),
            ),
            &unsigned(u128::from(trace.hash_to_point.point[rank]), field),
        );
    }
    (candidate, output)
}

/// The legacy forest uses at most 64 trees. Ten batching draws have degree
/// <=63, 55 sumcheck draws have degree three, and eleven line draws have degree
/// one per tree. With the 21-bit grind, their union bound is at most
/// (10*63 + 55*3 + 11*64) / (2^125 * 2^21) < 2^-135. No tree's root equality
/// is replaced by an equality between products across different signatures.
/// The hybrid profile allows 2048 trees and derives its difficulty from the
/// actual tree count together with the H2P cubic-round numerator.
#[tracing::instrument(skip_all, name = "falcon_arithmetic:compaction_forest")]
fn prove_product_forest(
    transcript: &mut impl Transcript,
    leaves: Vec<Vec<F>>,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(PrimeProductForestProof, Vec<CompactionProof>), FalconError> {
    if leaves.is_empty()
        || leaves.len() > 2048
        || leaves.len() % 2 != 0
        || leaves.iter().any(|tree| tree.len() != COMPACTION_LEAVES)
    {
        return Err(piop("invalid compaction forest shape"));
    }
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
    let roots: Vec<F> = trees.iter().map(|tree| tree[0][0]).collect();
    if roots.chunks_exact(2).any(|pair| pair[0] != pair[1]) {
        return Err(piop("compaction product roots differ"));
    }
    bind_forest(transcript, &roots, field);
    let mut claims = roots.clone();
    let mut point = Vec::new();
    let mut layers = Vec::with_capacity(11);
    for level in 0..11 {
        transcript.absorb_slice(&(level as u64).to_le_bytes());
        let mut groups: Vec<[Vec<F>; 2]> = crate::utils::cfg_iter_mut!(trees)
            .map(|tree| {
                let mut child = std::mem::take(&mut tree[level + 1]);
                let half = child.len() / 2;
                let right = child.split_off(half);
                [child, right]
            })
            .collect();
        let (sumcheck, evaluations, grinding_nonces, batching_nonce, next_point) = if level == 0 {
            (
                None,
                groups.iter().map(|g| [g[0][0], g[1][0]]).collect(),
                Vec::new(),
                None,
                Vec::new(),
            )
        } else {
            // The roots and every previous layer's evaluations already bind all
            // current claims before the fresh random combination is selected.
            let batching_nonce = prove_forest_nonce::<ForestBatchGrinding>(
                transcript,
                level,
                target_bits,
                security,
            )?;
            let rho = squeeze(transcript, field)?;
            let scales = powers(rho, groups.len(), field);
            let initial = weighted_sum(&claims, &scales, field);
            let (proof, evaluations, next_point, nonces) = prove_forest_layer(
                transcript,
                &mut groups,
                &point,
                &scales,
                initial,
                target_bits,
                security,
                field,
            )?;
            (Some(proof), evaluations, nonces, batching_nonce, next_point)
        };
        absorb_field_elements(transcript, &evaluations.concat(), field);
        let line_nonce =
            prove_forest_nonce::<ForestLineGrinding>(transcript, level, target_bits, security)?;
        let lambda = squeeze(transcript, field)?;
        claims = evaluations
            .iter()
            .map(|[left, right]| affine(*left, *right, lambda, field))
            .collect();
        point = next_point;
        point.push(lambda);
        layers.push(ProductForestLayerProof {
            sumcheck,
            evaluations,
            grinding_nonces,
            batching_nonce,
            line_nonce,
        });
    }
    let terminals: Vec<_> = roots
        .iter()
        .zip(&claims)
        .map(|(&root, &terminal_claim)| PrimeProductTreeProof {
            root,
            terminal_point: point.clone(),
            terminal_claim,
        })
        .collect();
    let mut terminals = terminals.into_iter();
    let mut compaction = Vec::with_capacity(trees.len() / 2);
    while let Some(candidate) = terminals.next() {
        compaction.push(CompactionProof {
            candidate,
            output: terminals.next().expect("paired trees"),
        });
    }
    Ok((PrimeProductForestProof { layers }, compaction))
}

fn bind_forest(transcript: &mut impl Transcript, roots: &[F], field: &Cfg) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/forest/v2");
    transcript.absorb_slice(&(roots.len() as u64).to_le_bytes());
    absorb_field_elements(transcript, roots, field);
}

fn powers(value: F, len: usize, field: &Cfg) -> Vec<F> {
    let mut power = field.one();
    (0..len)
        .map(|_| {
            let current = power;
            power = field.mul(&power, &value);
            current
        })
        .collect()
}

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

/// Sumcheck of eq(point,x) * sum_t scales[t] L_t(x) R_t(x).
/// The tables contain at most 2048*2048 elements, independent of source padding.
/// The ordinary outer engine has one A*B-C terminal triple, and the batched
/// inner engine is degree two. This cubic sum of products therefore supplies
/// its own arithmetic, using the shared sumcheck transcript/round helper.
fn prove_forest_layer(
    transcript: &mut impl Transcript,
    groups: &mut [[Vec<F>; 2]],
    point: &[F],
    scales: &[F],
    mut claim: F,
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(SumcheckProof<F, 4>, Vec<[F; 2]>, Vec<F>, Vec<u64>), FalconError> {
    let mut equality = eq_table(point, field).map_err(|error| piop(error.to_string()))?;
    let mut boundary = ProverGrindingRoundBoundary::<ForestRoundGrinding>::with_round_offset(
        if target_bits == 128 {
            security.cubic_round_bits
        } else {
            0
        },
        0,
    );
    let mut round_polynomials = Vec::with_capacity(point.len());
    let mut next_point = Vec::with_capacity(point.len());
    for _ in 0..point.len() {
        // Equality differences are shared by every tree. Apply each tree's
        // batching scalar once to its polynomial, outside the coefficient loop.
        let equality_pairs: Vec<_> = equality
            .chunks_exact(2)
            .map(|e| [e[0], field.sub(&e[1], &e[0])])
            .collect();
        let polynomials: Vec<_> = crate::utils::cfg_iter!(groups)
            .zip(scales)
            .map(|(group, scale)| {
                let mut polynomial = [field.zero(); 4];
                for i in 0..equality.len() / 2 {
                    let [e0, ed] = equality_pairs[i];
                    let l0 = group[0][2 * i];
                    let ld = field.sub(&group[0][2 * i + 1], &l0);
                    let r0 = group[1][2 * i];
                    let rd = field.sub(&group[1][2 * i + 1], &r0);
                    let product = [
                        field.mul(&l0, &r0),
                        field.add(&field.mul(&l0, &rd), &field.mul(&ld, &r0)),
                        field.mul(&ld, &rd),
                    ];
                    for j in 0..3 {
                        polynomial[j] = field.add(&polynomial[j], &field.mul(&e0, &product[j]));
                        polynomial[j + 1] =
                            field.add(&polynomial[j + 1], &field.mul(&ed, &product[j]));
                    }
                }
                polynomial.map(|coefficient| field.mul(&coefficient, scale))
            })
            .collect();
        let polynomial = polynomials.into_iter().fold([field.zero(); 4], |sum, p| {
            std::array::from_fn(|i| field.add(&sum[i], &p[i]))
        });
        let at_one = polynomial.iter().fold(field.zero(), |sum, coefficient| {
            field.add(&sum, coefficient)
        });
        if field.add(&polynomial[0], &at_one) != claim {
            return Err(piop("product forest round claim mismatch"));
        }
        let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
            transcript,
            &mut claim,
            &[polynomial[0], polynomial[2], polynomial[3]],
            &mut round_polynomials,
            &mut next_point,
            &field.zero(),
            field,
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        fold_table(&mut equality, challenge, field);
        crate::utils::cfg_iter_mut!(groups).for_each(|group| {
            fold_table(&mut group[0], challenge, field);
            fold_table(&mut group[1], challenge, field);
        });
    }
    let evaluations = groups
        .iter()
        .map(|group| [group[0][0], group[1][0]])
        .collect();
    Ok((
        SumcheckProof { round_polynomials },
        evaluations,
        next_point,
        boundary.into_nonces(),
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
            security.cubic_round_bits,
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
            security.cubic_round_bits,
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
    compaction: &[CompactionProof],
    target_bits: usize,
    security: FalconSecuritySchedule,
    field: &Cfg,
) -> Result<(), FalconError> {
    if compaction.is_empty() || compaction.len() > 1024 || forest.layers.len() != 11 {
        return Err(piop("invalid compaction forest shape"));
    }
    let terminals: Vec<_> = compaction
        .iter()
        .flat_map(|pair| [&pair.candidate, &pair.output])
        .collect();
    let roots: Vec<_> = terminals.iter().map(|tree| tree.root).collect();
    validate_field_elements(&roots, field).map_err(|error| piop(error.to_string()))?;
    if roots.chunks_exact(2).any(|pair| pair[0] != pair[1]) {
        return Err(piop("compaction product roots differ"));
    }
    bind_forest(transcript, &roots, field);
    let mut claims = roots;
    let mut point = Vec::new();
    for (level, layer) in forest.layers.iter().enumerate() {
        if layer.evaluations.len() != terminals.len() {
            return Err(piop("product forest evaluation count mismatch"));
        }
        validate_field_elements(&layer.evaluations.concat(), field)
            .map_err(|error| piop(error.to_string()))?;
        transcript.absorb_slice(&(level as u64).to_le_bytes());
        let next_point = if level == 0 {
            if layer.sumcheck.is_some()
                || !layer.grinding_nonces.is_empty()
                || layer.batching_nonce.is_some()
                || claims
                    .iter()
                    .zip(&layer.evaluations)
                    .any(|(claim, [left, right])| *claim != field.mul(left, right))
            {
                return Err(piop("product forest root layer failed"));
            }
            Vec::new()
        } else {
            let sumcheck = layer
                .sumcheck
                .as_ref()
                .ok_or_else(|| piop("missing product forest sumcheck"))?;
            verify_forest_nonce::<ForestBatchGrinding>(
                transcript,
                level,
                target_bits,
                security,
                layer.batching_nonce,
            )?;
            let rho = squeeze(transcript, field)?;
            let scales = powers(rho, terminals.len(), field);
            let initial = weighted_sum(&claims, &scales, field);
            let mut boundary = VerifierGrindingRoundBoundary::<ForestRoundGrinding>::new(
                if target_bits == 128 {
                    security.cubic_round_bits
                } else {
                    0
                },
                &layer.grinding_nonces,
            );
            let (next_point, final_claim) = sumcheck
                .verify_with_round_boundary(transcript, initial, level, field, &mut boundary)
                .map_err(|error| piop(error.to_string()))?;
            let products: Vec<_> = layer
                .evaluations
                .iter()
                .map(|[left, right]| field.mul(left, right))
                .collect();
            let eq =
                eq_eval(&point, &next_point, field).map_err(|error| piop(error.to_string()))?;
            if final_claim != field.mul(&eq, &weighted_sum(&products, &scales, field)) {
                return Err(piop("product forest terminal identity failed"));
            }
            next_point
        };
        absorb_field_elements(transcript, &layer.evaluations.concat(), field);
        verify_forest_nonce::<ForestLineGrinding>(
            transcript,
            level,
            target_bits,
            security,
            layer.line_nonce,
        )?;
        let lambda = squeeze(transcript, field)?;
        claims = layer
            .evaluations
            .iter()
            .map(|[left, right]| affine(*left, *right, lambda, field))
            .collect();
        point = next_point;
        point.push(lambda);
    }
    if terminals
        .iter()
        .zip(&claims)
        .any(|(tree, claim)| tree.terminal_point != point || tree.terminal_claim != *claim)
    {
        return Err(piop("product forest terminal mismatch"));
    }
    Ok(())
}

fn bind_compaction_leaf(
    transcript: &mut impl Transcript,
    compaction: &[CompactionProof],
    field: &Cfg,
) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/leaf/v1");
    let claims: Vec<_> = compaction
        .iter()
        .map(|pair| pair.candidate.terminal_claim)
        .collect();
    absorb_field_elements(transcript, &claims, field);
}

fn leaf_initial_claim(compaction: &[CompactionProof], weights: &[F], field: &Cfg) -> F {
    compaction
        .iter()
        .zip(weights)
        .fold(field.zero(), |sum, (pair, weight)| {
            field.add(
                &sum,
                &field.mul(
                    weight,
                    &field.sub(&pair.candidate.terminal_claim, &field.one()),
                ),
            )
        })
}

#[tracing::instrument(skip_all, name = "falcon_arithmetic:compaction_leaf")]
fn prove_compaction_leaf(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    compaction: &[CompactionProof],
    gamma: F,
    rank_scale: F,
    target_bits: usize,
    field: &Cfg,
) -> Result<CompactionLeafProof, FalconError> {
    let security = security_schedule(layout, target_bits)?;
    bind_compaction_leaf(transcript, compaction, field);
    let instance_rounds = layout.capacity().trailing_zeros() as usize;
    let instance_nonce = if target_bits == 128 && instance_rounds != 0 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<LeafInstanceGrinding>::new(0),
                security.norm_instance_bits,
            )
            .map_err(|error| piop(error.to_string()))?,
        )
    } else {
        None
    };
    let instance_point = sample_point(transcript, instance_rounds, field)?;
    let weights = eq_table(&instance_point, field).map_err(|error| piop(error.to_string()))?;
    let forest_weights = eq_table(&compaction[0].candidate.terminal_point, field)
        .map_err(|error| piop(error.to_string()))?;
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
    let mut claim = leaf_initial_claim(compaction, &weights, field);
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
        instance_nonce,
        sumcheck: SumcheckProof { round_polynomials },
        terminal,
        point,
        grinding_nonces: boundary.into_nonces(),
    })
}

fn verify_compaction_leaf(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    compaction: &[CompactionProof],
    proof: &CompactionLeafProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(), FalconError> {
    let security = security_schedule(layout, target_bits)?;
    bind_compaction_leaf(transcript, compaction, field);
    let instance_rounds = layout.capacity().trailing_zeros() as usize;
    match (
        target_bits == 128 && instance_rounds != 0,
        proof.instance_nonce,
    ) {
        (true, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<LeafInstanceGrinding>::new(0),
            security.norm_instance_bits,
            nonce,
        )
        .map_err(|error| piop(error.to_string()))?,
        (false, None) => {}
        _ => return Err(piop("invalid leaf-instance grinding nonce")),
    }
    let instance_point = sample_point(transcript, instance_rounds, field)?;
    if instance_point != proof.instance_point {
        return Err(piop("compaction leaf instance point mismatch"));
    }
    validate_field_elements(&proof.terminal, field).map_err(|error| piop(error.to_string()))?;
    let weights = eq_table(&instance_point, field).map_err(|error| piop(error.to_string()))?;
    let initial = leaf_initial_claim(compaction, &weights, field);
    let mut boundary = VerifierGrindingRoundBoundary::<LeafRoundGrinding>::new(
        if target_bits == 128 {
            security.cubic_round_bits
        } else {
            0
        },
        &proof.grinding_nonces,
    );
    let (point, final_claim) = proof
        .sumcheck
        .verify_with_round_boundary(
            transcript,
            initial,
            11 + instance_rounds,
            field,
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
    if point != proof.point
        || final_claim
            != field.mul(
                &field.mul(&proof.terminal[0], &proof.terminal[1]),
                &proof.terminal[2],
            )
    {
        return Err(piop("compaction leaf terminal identity failed"));
    }
    absorb_field_elements(transcript, &proof.terminal, field);
    Ok(())
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

fn bit_field(word: u64, bit: usize, field: &Cfg) -> F {
    unsigned(u128::from((word >> bit) & 1), field)
}

fn affine(left: F, right: F, point: F, field: &Cfg) -> F {
    field.add(&left, &field.mul(&point, &field.sub(&right, &left)))
}

fn keccak_rounds(layout: &FalconSourceLayout) -> usize {
    21 + layout.capacity().trailing_zeros() as usize
}

fn compaction_product_rounds(layout: &FalconSourceLayout) -> usize {
    (if layout.is_hybrid() { 11 } else { 13 }) + layout.capacity().trailing_zeros() as usize
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

    #[test]
    fn hybrid_rejection_rows_match_dense_padded_relation() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let layout = FalconSourceLayout::new_hybrid(3).unwrap();
        let stride = COMPACTION_LEAVES * layout.capacity();
        let rows = CompactionRows {
            traces: &traces,
            stride,
            rejection_only: true,
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
    fn hybrid_piop_leaf_roundtrips_and_rejects_tampering() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        for (batch, target) in [(1, 100), (3, 100), (3, 128)] {
            let layout = FalconSourceLayout::new_hybrid(batch).unwrap();
            let mut prover = Blake3Transcript::new();
            let proof =
                prove_falcon_piop(&mut prover, &layout, &vec![trace.clone(); batch], target)
                    .unwrap();
            let field = field::FpCtx::from_prime_u128(proof.modulus);
            let leaf = proof.compaction_leaf.as_ref().unwrap();
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
                let value = &mut bad.compaction_leaf.as_mut().unwrap().terminal[operand];
                *value = field.add(value, &field.one());
                reject(&bad);
            }
            let mut bad = proof.clone();
            bad.compaction_leaf = None;
            reject(&bad);
            let mut bad = proof.clone();
            bad.compaction_leaf.as_mut().unwrap().point[0] = field.zero();
            reject(&bad);
            let mut bad = proof.clone();
            bad.compaction_leaf
                .as_mut()
                .unwrap()
                .sumcheck
                .round_polynomials[0][0] = field.zero();
            reject(&bad);
            let mut bad = proof.clone();
            bad.compaction_leaf.as_mut().unwrap().grinding_nonces = vec![0];
            reject(&bad);
            if batch > 1 {
                let mut bad = proof.clone();
                bad.compaction_leaf.as_mut().unwrap().instance_point[0] = field.zero();
                reject(&bad);
                let mut bad = proof.clone();
                bad.compaction_leaf.as_mut().unwrap().instance_nonce =
                    if target == 128 { None } else { Some(0) };
                reject(&bad);
            }
            let mut bad = proof.clone();
            bad.compaction[0].candidate.terminal_claim = field.zero();
            reject(&bad);
        }
    }

    #[test]
    fn compaction_leaf_terminals_match_masked_weighted_tables() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let layout = FalconSourceLayout::new_hybrid(3).unwrap();
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let gamma = unsigned(17, &field);
        let rho = unsigned(31, &field);
        let forest_point: Vec<_> = (0..11).map(|i| unsigned(13 + i, &field)).collect();
        let forest_weights = eq_table(&forest_point, &field).unwrap();
        let compaction: Vec<_> = traces
            .iter()
            .map(|trace| {
                let (candidate, output) = compaction_leaves(trace, gamma, rho, &field);
                CompactionProof {
                    candidate: PrimeProductTreeProof {
                        root: field.zero(),
                        terminal_point: forest_point.clone(),
                        terminal_claim: weighted_sum(&candidate, &forest_weights, &field),
                    },
                    output: PrimeProductTreeProof {
                        root: field.zero(),
                        terminal_point: forest_point.clone(),
                        terminal_claim: weighted_sum(&output, &forest_weights, &field),
                    },
                }
            })
            .collect();
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
        bad[1].candidate.terminal_claim = field.add(&bad[1].candidate.terminal_claim, &field.one());
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
    fn parallel_forest_rounds_match_direct_cubic_evaluations() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let point: Vec<_> = (0..5).map(|i| unsigned(19 + i, &field)).collect();
        let mut equality = eq_table(&point, &field).unwrap();
        let groups: Vec<[Vec<F>; 2]> = (0..6)
            .map(|tree| {
                std::array::from_fn(|side| {
                    (0..32)
                        .map(|i| unsigned((1 << 120) + 311 * tree + 37 * side as u128 + i, &field))
                        .collect()
                })
            })
            .collect();
        let scales = powers(unsigned(127, &field), groups.len(), &field);
        let mut initial = field.zero();
        for (group, scale) in groups.iter().zip(&scales) {
            for i in 0..32 {
                initial = field.add(
                    &initial,
                    &field.mul(
                        scale,
                        &field.mul(&equality[i], &field.mul(&group[0][i], &group[1][i])),
                    ),
                );
            }
        }
        let (proof, _, challenges, _) = prove_forest_layer(
            &mut Blake3Transcript::new(),
            &mut groups.clone(),
            &point,
            &scales,
            initial,
            100,
            FalconSecuritySchedule::for_target(100).unwrap(),
            &field,
        )
        .unwrap();
        let mut groups = groups;
        for (polynomial, challenge) in proof.round_polynomials.iter().zip(challenges) {
            // Four independent evaluations determine the entire cubic. This
            // oracle interpolates operands directly instead of expanding them.
            for x in 0..4 {
                let x = unsigned(x, &field);
                let mut expected = field.zero();
                for (group, scale) in groups.iter().zip(&scales) {
                    for i in 0..equality.len() / 2 {
                        let e = affine(equality[2 * i], equality[2 * i + 1], x, &field);
                        let l = affine(group[0][2 * i], group[0][2 * i + 1], x, &field);
                        let r = affine(group[1][2 * i], group[1][2 * i + 1], x, &field);
                        expected = field.add(
                            &expected,
                            &field.mul(scale, &field.mul(&e, &field.mul(&l, &r))),
                        );
                    }
                }
                let actual = polynomial
                    .iter()
                    .rev()
                    .fold(field.zero(), |acc, c| field.add(&field.mul(&acc, &x), c));
                assert_eq!(actual, expected);
            }
            fold_table(&mut equality, challenge, &field);
            for group in &mut groups {
                for table in group {
                    fold_table(table, challenge, &field);
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
    fn lazy_compaction_word_rows_match_dense_proof_with_padding() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
        let layout = FalconSourceLayout::new(3).unwrap();
        let stride = COMPACTION_LEAVES * layout.capacity();
        let mut dense = OuterInputs {
            ax: vec![field.zero(); 4 * stride],
            bx: vec![field.zero(); 4 * stride],
            cx: vec![field.zero(); 4 * stride],
        };
        for (instance, trace) in traces.iter().enumerate() {
            let hash = &trace.hash_to_point;
            for i in 0..HASH_TO_POINT_SAMPLES {
                let reject = !hash.accepted[i];
                let selector = hash.accepted[i] && hash.prefix[i] < 1024;
                let selected_prefix = if selector { hash.prefix[i] } else { 0 };
                let selected_remainder = if selector { hash.remainders[i] } else { 0 };
                let operands = [
                    [
                        u128::from(!reject),
                        u128::from((hash.prefix[i] & 1024) == 0),
                        u128::from(selector),
                    ],
                    [
                        u128::from(selector),
                        hash.prefix[i].into(),
                        selected_prefix.into(),
                    ],
                    [
                        u128::from(selector),
                        hash.remainders[i].into(),
                        selected_remainder.into(),
                    ],
                    [
                        u128::from((hash.quotients[i] & 4) != 0),
                        u128::from((hash.quotients[i] & 1) != 0),
                        u128::from(reject),
                    ],
                ];
                for (block, values) in operands.into_iter().enumerate() {
                    let index = block * stride + instance * COMPACTION_LEAVES + i;
                    dense.ax[index] = unsigned(values[0], &field);
                    dense.bx[index] = unsigned(values[1], &field);
                    dense.cx[index] = unsigned(values[2], &field);
                }
            }
        }
        let rows = CompactionRows {
            traces: &traces,
            stride,
            rejection_only: false,
            field: &field,
        };
        for row in 0..4 * stride {
            assert_eq!(
                [rows.a(row), rows.b(row), rows.c(row)],
                [dense.ax[row], dense.bx[row], dense.cx[row]]
            );
        }
        let mut dense_transcript = Blake3Transcript::new();
        dense_transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
        let dense_proof = prove_quadratic(
            &mut dense_transcript,
            dense,
            15,
            100,
            FalconSecuritySchedule::for_target(100).unwrap(),
            &field,
        )
        .unwrap();
        let mut lazy_transcript = Blake3Transcript::new();
        let lazy_proof =
            prove_compaction_products(&mut lazy_transcript, &layout, &traces, 100, &field).unwrap();
        assert_eq!(lazy_proof.point.len(), 13 + 2);
        assert_eq!(lazy_proof, dense_proof);
        assert_eq!(
            squeeze(&mut lazy_transcript, &field).unwrap(),
            squeeze(&mut dense_transcript, &field).unwrap()
        );
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
        assert!(
            !proof
                .keccak_chi
                .as_ref()
                .unwrap()
                .grinding_nonces
                .is_empty()
        );
        assert!(proof.keccak_chi.as_ref().unwrap().point_nonce.is_some());
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
            55
        );
        assert!(
            proof
                .compaction_forest
                .layers
                .iter()
                .all(|layer| layer.line_nonce.is_some())
        );
        assert!(
            proof.compaction_forest.layers[1..]
                .iter()
                .all(|layer| layer.batching_nonce.is_some())
        );
        let mut bad = proof.clone();
        bad.compaction_forest.layers[1].batching_nonce = None;
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
    fn lazy_keccak_rows_preserve_dense_proof_and_transcript() {
        let traces = [verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap()];
        let layout = FalconSourceLayout::new(1).unwrap();
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let half = KECCAK_GATE_STRIDE;
        let mut dense = OuterInputs {
            ax: vec![field.zero(); 2 * half],
            bx: vec![field.zero(); 2 * half],
            cx: vec![field.zero(); 2 * half],
        };
        // Independent original dense construction, including iota and the
        // linear XOR operand rather than substituting the honest product.
        let shake = &traces[0].hash_to_point.shake;
        let two_inv = unsigned((field.modulus_u128() + 1) / 2, &field);
        for gate in 0..KECCAK_BITS {
            let word = gate / 64;
            let bit = gate % 64;
            let lane = word % 25;
            let round = word / 25 % 24;
            let neighbor = |offset| word - lane + lane / 5 * 5 + (lane + offset) % 5;
            let b = bit_field(shake.chi_inputs[word], bit, &field);
            let z = bit_field(shake.chi_ands[word], bit, &field);
            let mut y = shake.round_states[word] >> bit & 1;
            if lane == 0 {
                y ^= ROUND_CONSTANTS[round] >> bit & 1;
            }
            dense.ax[gate] = field.sub(
                &field.one(),
                &bit_field(shake.chi_inputs[neighbor(1)], bit, &field),
            );
            dense.bx[gate] = bit_field(shake.chi_inputs[neighbor(2)], bit, &field);
            dense.cx[gate] = z;
            dense.ax[half + gate] = b;
            dense.bx[half + gate] = z;
            dense.cx[half + gate] = field.mul(
                &field.sub(&field.add(&b, &z), &unsigned(y.into(), &field)),
                &two_inv,
            );
        }
        let mut dense_transcript = Blake3Transcript::new();
        dense_transcript.absorb_slice(b"bitz/falcon1024-ct/keccak-chi/v1");
        let dense_proof = prove_quadratic(
            &mut dense_transcript,
            dense,
            keccak_rounds(&layout),
            100,
            FalconSecuritySchedule::for_target(100).unwrap(),
            &field,
        )
        .unwrap();
        let mut lazy_transcript = Blake3Transcript::new();
        let lazy_proof =
            prove_keccak_chi(&mut lazy_transcript, &layout, &traces, 100, &field).unwrap();
        assert_eq!(lazy_proof, dense_proof);
        assert_eq!(
            squeeze(&mut lazy_transcript, &field).unwrap(),
            squeeze(&mut dense_transcript, &field).unwrap()
        );

        // A non-power-of-two batch has both per-signature padding and a
        // completely empty capacity slot, including in the XOR row block.
        let traces = vec![traces[0].clone(); 3];
        let rows = KeccakRows {
            traces: &traces,
            half: 4 * half,
            field: &field,
            two_inv,
        };
        for row in [
            KECCAK_BITS,
            3 * half,
            rows.half + KECCAK_BITS,
            rows.half + 3 * half,
        ] {
            assert_eq!([rows.a(row), rows.b(row), rows.c(row)], [field.zero(); 3]);
        }
    }

    #[test]
    fn forest_batches_distinct_trees_and_rejects_tampering() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        for batch in [1, 3, 32] {
            let mut leaves = Vec::new();
            for instance in 0..batch {
                let candidate: Vec<_> = (0..COMPACTION_LEAVES)
                    .map(|i| unsigned((2 + i + 3 * instance) as u128, &field))
                    .collect();
                let output = candidate.iter().rev().copied().collect();
                leaves.extend([candidate, output]);
            }
            let original_leaves = leaves.clone();
            let mut prover = Blake3Transcript::new();
            let (forest, compaction) = prove_product_forest(
                &mut prover,
                leaves,
                100,
                FalconSecuritySchedule::for_target(100).unwrap(),
                &field,
            )
            .unwrap();
            let mut verifier = Blake3Transcript::new();
            verify_product_forest(
                &mut verifier,
                &forest,
                &compaction,
                100,
                FalconSecuritySchedule::for_target(100).unwrap(),
                &field,
            )
            .unwrap();
            assert_eq!(
                squeeze(&mut prover, &field).unwrap(),
                squeeze(&mut verifier, &field).unwrap()
            );
            for (tree, leaves) in compaction
                .iter()
                .flat_map(|pair| [&pair.candidate, &pair.output])
                .zip(original_leaves)
            {
                let weights = eq_table(&tree.terminal_point, &field).unwrap();
                assert_eq!(tree.terminal_claim, weighted_sum(&leaves, &weights, &field));
            }
            let reject = |forest: &PrimeProductForestProof, trees: &[CompactionProof]| {
                assert!(
                    verify_product_forest(
                        &mut Blake3Transcript::new(),
                        forest,
                        trees,
                        100,
                        FalconSecuritySchedule::for_target(100).unwrap(),
                        &field
                    )
                    .is_err()
                );
            };
            let mut bad = compaction.clone();
            bad[0].candidate.root = field.add(&bad[0].candidate.root, &field.one());
            reject(&forest, &bad);
            let mut bad = compaction.clone();
            let first = &mut bad[0];
            std::mem::swap(&mut first.candidate, &mut first.output);
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
            bad.layers[1].sumcheck.as_mut().unwrap().round_polynomials[0][0] = field.zero();
            reject(&bad, &compaction);
        }
    }
}
