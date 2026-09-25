//! Algebraic Falcon PIOP layers over the transcript-selected prime field.
//!
//! This file deliberately stops at terminal MLE claims.  Those claims are
//! linearized into the one committed source witness by `opening.rs`; keeping
//! that boundary explicit prevents a native trace check from being mistaken
//! for a commitment-bound proof.

use field::RingOps;

use crate::{
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{InitialClaims, prove_batched_inner_sumcheck},
        outer::{
            OuterClaim, OuterEvaluations, OuterInputs, prove_outer_sumcheck, verify_outer_sumcheck,
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
const CUBIC_GRINDING_BITS: u32 = 21;
const FINGERPRINT_GRINDING_BITS: u32 = 23;
pub(super) const LINEAR_POINT_GRINDING_BITS: u32 = 14;
pub(super) const BINDING_GRINDING_BITS: u32 = 13;

/// Proof-of-work schedule used to lift the 126-bit projection field to the
/// requested computational soundness target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSecuritySchedule {
    pub quadratic_round_bits: u32,
    pub cubic_round_bits: u32,
    pub fingerprint_bits: u32,
    pub linear_point_bits: u32,
    pub binding_round_bits: u32,
}

impl FalconSecuritySchedule {
    pub const fn for_target(target_bits: usize) -> Option<Self> {
        match target_bits {
            100 => Some(Self {
                quadratic_round_bits: 0,
                cubic_round_bits: 0,
                fingerprint_bits: 0,
                linear_point_bits: 0,
                binding_round_bits: 0,
            }),
            128 => Some(Self {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormProof {
    pub claims: [F; 2],
    pub slack: F,
    pub sumchecks: [SumcheckProof<F, 3>; 2],
    /// `[weight(r), value(r)]` for `S1` and `S2`.
    pub terminal: [[F; 2]; 2],
    /// Shared evaluation point of the two norm sumchecks.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadraticRelationProof {
    pub sumcheck: SumcheckProof<F, 4>,
    pub terminal: OuterEvaluations<F>,
    /// Terminal point returned by the outer sumcheck.
    pub point: Vec<F>,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductLayerProof {
    /// Absent only for the root's direct two-child multiplication.
    pub sumcheck: Option<SumcheckProof<F, 4>>,
    pub left: F,
    pub right: F,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeProductTreeProof {
    pub root: F,
    pub layers: Vec<ProductLayerProof>,
    /// Point and value of the original leaf MLE after all layer reductions.
    pub terminal_point: Vec<F>,
    pub terminal_claim: F,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionProof {
    pub candidate: PrimeProductTreeProof,
    pub output: PrimeProductTreeProof,
}

/// Proof through the norm, Keccak chi, compaction-product, and compaction
/// witness-product layers.  The terminal claims must subsequently be
/// collapsed and opened against the source commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FalconPiopProof {
    pub modulus: u128,
    pub norm: NormProof,
    pub keccak_chi: QuadraticRelationProof,
    pub compact_products: QuadraticRelationProof,
    pub fingerprint_nonce: Option<u64>,
    pub compaction_gamma: F,
    pub compaction_rank_scale: F,
    pub compaction: Vec<CompactionProof>,
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
    let keccak_chi = prove_keccak_chi(transcript, layout, traces, target_bits, &field)
        .map_err(|error| piop(format!("keccak chi: {error}")))?;
    let compact_products =
        prove_compaction_products(transcript, layout, traces, target_bits, &field)
            .map_err(|error| piop(format!("compaction products: {error}")))?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/fingerprint/v1");
    let fingerprint_nonce = if target_bits == 128 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<FingerprintGrinding>::new(0),
                FINGERPRINT_GRINDING_BITS,
            )
            .map_err(|error| piop(error.to_string()))?,
        )
    } else {
        None
    };
    let gamma = squeeze(transcript, &field)?;
    let rank_scale = squeeze(transcript, &field)?;
    let mut compaction = Vec::with_capacity(layout.batch());
    for trace in traces {
        let (candidate, output) = compaction_leaves(trace, gamma, rank_scale, &field);
        let candidate = prove_product_tree(transcript, candidate, target_bits, &field)?;
        let output = prove_product_tree(transcript, output, target_bits, &field)?;
        if candidate.root != output.root {
            return Err(piop("compaction product roots differ"));
        }
        compaction.push(CompactionProof { candidate, output });
    }

    Ok(FalconPiopProof {
        modulus: field.modulus_u128(),
        norm,
        keccak_chi,
        compact_products,
        fingerprint_nonce,
        compaction_gamma: gamma,
        compaction_rank_scale: rank_scale,
        compaction,
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
    transcript.absorb_slice(b"bitz/falcon1024-ct/keccak-chi/v1");
    verify_quadratic(
        transcript,
        keccak_rounds(layout),
        &proof.keccak_chi,
        target_bits,
        &field,
    )?;
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v1");
    verify_quadratic(
        transcript,
        compaction_product_rounds(layout),
        &proof.compact_products,
        target_bits,
        &field,
    )?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/fingerprint/v1");
    match (target_bits, proof.fingerprint_nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<FingerprintGrinding>::new(0),
            FINGERPRINT_GRINDING_BITS,
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
    for pair in &proof.compaction {
        let _candidate = verify_product_tree(transcript, &pair.candidate, target_bits, &field)?;
        let _output = verify_product_tree(transcript, &pair.output, target_bits, &field)?;
        if pair.candidate.root != pair.output.root {
            return Err(piop("compaction product roots differ"));
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
    transcript.absorb_slice(b"bitz/falcon1024-ct/piop/v1");
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
}

fn sample_field(transcript: &mut impl Transcript) -> Result<Cfg, FalconError> {
    crate::ext_proj::sample_prime_context(transcript, PRIME_MIN, PRIME_MAX, 128)
        .map_err(|error| piop(error.to_string()))
}

fn prove_norm(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<NormProof, FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/norm/v1");
    let len = N * layout.capacity();
    let zero = field.zero();
    let mut s1 = vec![zero; len];
    let mut s2 = vec![zero; len];
    let mut claims = [zero; 2];
    let mut slack = zero;
    for (instance, trace) in traces.iter().enumerate() {
        for i in 0..N {
            let index = instance * N + i;
            s1[index] = signed(i128::from(trace.s1[i]), field);
            s2[index] = signed(i128::from(trace.signature.s2[i]), field);
            claims[0] = field.add(&claims[0], &field.mul(&s1[index], &s1[index]));
            claims[1] = field.add(&claims[1], &field.mul(&s2[index], &s2[index]));
        }
        slack = field.add(&slack, &unsigned(u128::from(trace.norm_slack), field));
    }
    let beta = unsigned(BETA_SQUARED as u128, field);
    let expected = field.mul(&unsigned(traces.len() as u128, field), &beta);
    if field.add(&field.add(&claims[0], &claims[1]), &slack) != expected {
        return Err(piop("invalid norm witness"));
    }

    let output = if target_bits == 128 {
        let mut boundary = ProverGrindingRoundBoundary::<QuadraticGrinding>::with_round_offset(
            QUADRATIC_GRINDING_BITS,
            0,
        );
        let out = prove_batched_inner_sumcheck(
            field,
            transcript,
            &claims,
            [s1.clone(), s2.clone()],
            [s1, s2],
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
            [s1.clone(), s2.clone()],
            [s1, s2],
            &mut boundary,
        )
        .map_err(|error| piop(error.to_string()))?;
        (out, Vec::new())
    };
    let (output, grinding_nonces) = output;
    absorb_field_elements(transcript, &output.terminal_evaluations.concat(), field);
    Ok(NormProof {
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
    transcript.absorb_slice(b"bitz/falcon1024-ct/norm/v1");
    let beta = unsigned(BETA_SQUARED as u128, field);
    let expected = field.mul(&unsigned(layout.batch() as u128, field), &beta);
    if field.add(&field.add(&proof.claims[0], &proof.claims[1]), &proof.slack) != expected {
        return Err(piop("norm claim does not equal the Falcon bound"));
    }
    let rounds = 10 + layout.capacity().trailing_zeros() as usize;
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<QuadraticGrinding>::new(
            QUADRATIC_GRINDING_BITS,
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
    let half = KECCAK_GATE_STRIDE * layout.capacity();
    let zero = field.zero();
    let one = field.one();
    let two_inv = unsigned((field.modulus_u128() + 1) / 2, field);
    let mut rows = OuterInputs {
        ax: vec![zero; 2 * half],
        bx: vec![zero; 2 * half],
        cx: vec![zero; 2 * half],
    };
    for (instance, trace) in traces.iter().enumerate() {
        let shake = &trace.hash_to_point.shake;
        for gate in 0..KECCAK_BITS {
            let word = gate >> 6;
            let bit = gate & 63;
            let lane_in_round = word % 25;
            let round = (word / 25) % 24;
            let bx = bit_field(shake.chi_inputs[word], bit, field);
            let b1 = bit_field(
                shake.chi_inputs
                    [word - lane_in_round + (lane_in_round / 5) * 5 + (lane_in_round + 1) % 5],
                bit,
                field,
            );
            let b2 = bit_field(
                shake.chi_inputs
                    [word - lane_in_round + (lane_in_round / 5) * 5 + (lane_in_round + 2) % 5],
                bit,
                field,
            );
            let z = bit_field(shake.chi_ands[word], bit, field);
            let mut y = ((shake.round_states[word] >> bit) & 1) != 0;
            if lane_in_round == 0 && (ROUND_CONSTANTS[round] >> bit) & 1 == 1 {
                y = !y;
            }
            let y = unsigned(u128::from(y), field);
            let index = instance * KECCAK_GATE_STRIDE + gate;
            rows.ax[index] = field.sub(&one, &b1);
            rows.bx[index] = b2;
            rows.cx[index] = z;

            let xor_index = half + index;
            rows.ax[xor_index] = bx;
            rows.bx[xor_index] = z;
            rows.cx[xor_index] = field.mul(&field.sub(&field.add(&bx, &z), &y), &two_inv);
        }
    }
    prove_quadratic(transcript, rows, keccak_rounds(layout), target_bits, field)
}

fn prove_compaction_products(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    target_bits: usize,
    field: &Cfg,
) -> Result<QuadraticRelationProof, FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v1");
    let candidate_stride = COMPACTION_LEAVES;
    let instances_stride = candidate_stride * layout.capacity();
    let relation_blocks = 32;
    let zero = field.zero();
    let one = field.one();
    let mut rows = OuterInputs {
        ax: vec![zero; relation_blocks * instances_stride],
        bx: vec![zero; relation_blocks * instances_stride],
        cx: vec![zero; relation_blocks * instances_stride],
    };
    for (instance, trace) in traces.iter().enumerate() {
        let hash = &trace.hash_to_point;
        for i in 0..HASH_TO_POINT_SAMPLES {
            let base = instance * candidate_stride + i;
            let accepted = unsigned(u128::from(hash.accepted[i]), field);
            let prefix_msb = unsigned(u128::from((hash.prefix[i] >> 10) & 1), field);
            let selected = hash.accepted[i] && hash.prefix[i] < 1024;
            rows.ax[base] = accepted;
            rows.bx[base] = field.sub(&one, &prefix_msb);
            rows.cx[base] = unsigned(u128::from(selected), field);
            for bit in 0..11 {
                let index = (1 + bit) * instances_stride + base;
                rows.ax[index] = rows.cx[base];
                rows.bx[index] = unsigned(u128::from((hash.prefix[i] >> bit) & 1), field);
                rows.cx[index] = if selected { rows.bx[index] } else { zero };
            }
            for bit in 0..14 {
                let index = (12 + bit) * instances_stride + base;
                rows.ax[index] = rows.cx[base];
                rows.bx[index] = unsigned(u128::from((hash.remainders[i] >> bit) & 1), field);
                rows.cx[index] = if selected { rows.bx[index] } else { zero };
            }
            let accept_index = 26 * instances_stride + base;
            let quotient = hash.quotients[i];
            rows.ax[accept_index] = unsigned(u128::from((quotient >> 2) & 1), field);
            rows.bx[accept_index] = unsigned(u128::from(quotient & 1), field);
            rows.cx[accept_index] = unsigned(u128::from(!hash.accepted[i]), field);
        }
    }
    prove_quadratic(
        transcript,
        rows,
        compaction_product_rounds(layout),
        target_bits,
        field,
    )
}

fn prove_quadratic(
    transcript: &mut impl Transcript,
    rows: OuterInputs<F>,
    rounds: usize,
    target_bits: usize,
    field: &Cfg,
) -> Result<QuadraticRelationProof, FalconError> {
    let tau = sample_point(transcript, rounds, field)?;
    let output = if target_bits == 128 {
        let mut boundary =
            ProverGrindingRoundBoundary::<CubicGrinding>::with_round_offset(CUBIC_GRINDING_BITS, 0);
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
    field: &Cfg,
) -> Result<(), FalconError> {
    let tau = sample_point(transcript, rounds, field)?;
    let zero = field.zero();
    let output = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<CubicGrinding>::new(
            CUBIC_GRINDING_BITS,
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

fn prove_product_tree(
    transcript: &mut impl Transcript,
    leaves: Vec<F>,
    target_bits: usize,
    field: &Cfg,
) -> Result<PrimeProductTreeProof, FalconError> {
    let mut tree = vec![leaves];
    while tree.last().expect("leaf layer").len() > 1 {
        let child = tree.last().expect("child layer");
        let half = child.len() / 2;
        let parent = (0..half)
            .map(|i| field.mul(&child[i], &child[i + half]))
            .collect();
        tree.push(parent);
    }
    tree.reverse();
    let root = tree[0][0];
    absorb_field_elements(transcript, &[root], field);
    let mut claim = root;
    let mut point = Vec::new();
    let mut layers = Vec::with_capacity(tree.len() - 1);
    for level in 0..tree.len() - 1 {
        let children = &tree[level + 1];
        let half = children.len() / 2;
        let left = &children[..half];
        let right = &children[half..];
        if level == 0 {
            if claim != field.mul(&left[0], &right[0]) {
                return Err(piop("product root mismatch"));
            }
            absorb_field_elements(transcript, &[left[0], right[0]], field);
            let lambda = squeeze(transcript, field)?;
            claim = affine(left[0], right[0], lambda, field);
            point = vec![lambda];
            layers.push(ProductLayerProof {
                sumcheck: None,
                left: left[0],
                right: right[0],
                grinding_nonces: Vec::new(),
            });
            continue;
        }
        let zero = field.zero();
        let rows = OuterInputs {
            ax: left.to_vec(),
            bx: right.to_vec(),
            cx: vec![zero; half],
        };
        let out = if target_bits == 128 {
            let mut boundary = ProverGrindingRoundBoundary::<CubicGrinding>::with_round_offset(
                CUBIC_GRINDING_BITS,
                0,
            );
            let out = prove_outer_sumcheck(
                field,
                transcript,
                OuterClaim::Sum(claim),
                &point,
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
                OuterClaim::Sum(claim),
                &point,
                rows,
                None,
                &mut boundary,
            )
            .map_err(|error| piop(error.to_string()))?;
            (out, Vec::new())
        };
        let lambda = squeeze(transcript, field)?;
        claim = affine(out.0.evaluations.ax, out.0.evaluations.bx, lambda, field);
        point = out.0.point;
        point.push(lambda);
        layers.push(ProductLayerProof {
            sumcheck: Some(out.0.proof),
            left: out.0.evaluations.ax,
            right: out.0.evaluations.bx,
            grinding_nonces: out.1,
        });
    }
    Ok(PrimeProductTreeProof {
        root,
        layers,
        terminal_point: point,
        terminal_claim: claim,
    })
}

fn verify_product_tree(
    transcript: &mut impl Transcript,
    proof: &PrimeProductTreeProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<(Vec<F>, F), FalconError> {
    if proof.layers.len() != 11 {
        return Err(piop("product-tree depth mismatch"));
    }
    absorb_field_elements(transcript, &[proof.root], field);
    let mut claim = proof.root;
    let mut point = Vec::new();
    for (level, layer) in proof.layers.iter().enumerate() {
        if level == 0 {
            if layer.sumcheck.is_some() || claim != field.mul(&layer.left, &layer.right) {
                return Err(piop("product root layer failed"));
            }
            absorb_field_elements(transcript, &[layer.left, layer.right], field);
            let lambda = squeeze(transcript, field)?;
            claim = affine(layer.left, layer.right, lambda, field);
            point = vec![lambda];
            continue;
        }
        let sumcheck = layer
            .sumcheck
            .as_ref()
            .ok_or_else(|| piop("missing product layer sumcheck"))?;
        let evaluations = OuterEvaluations {
            ax: layer.left,
            bx: layer.right,
            cx: field.zero(),
        };
        let output = if target_bits == 128 {
            let mut boundary = VerifierGrindingRoundBoundary::<CubicGrinding>::new(
                CUBIC_GRINDING_BITS,
                &layer.grinding_nonces,
            );
            verify_outer_sumcheck(
                field,
                transcript,
                claim,
                &point,
                sumcheck,
                evaluations,
                &mut boundary,
            )
        } else {
            let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
            verify_outer_sumcheck(
                field,
                transcript,
                claim,
                &point,
                sumcheck,
                evaluations,
                &mut boundary,
            )
        }
        .map_err(|error| piop(error.to_string()))?;
        let lambda = squeeze(transcript, field)?;
        claim = affine(layer.left, layer.right, lambda, field);
        point = output.point;
        point.push(lambda);
    }
    if point != proof.terminal_point || claim != proof.terminal_claim {
        return Err(piop("product-tree terminal mismatch"));
    }
    Ok((point, claim))
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
    16 + layout.capacity().trailing_zeros() as usize
}

fn piop(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{piop::spartan::falcon1024_ct::verification_trace, transcript::Blake3Transcript};

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

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
        assert!(!proof.keccak_chi.grinding_nonces.is_empty());
        let mut verifier = Blake3Transcript::new();
        verify_falcon_piop(&mut verifier, &layout, &proof, 128).unwrap();
    }
}
