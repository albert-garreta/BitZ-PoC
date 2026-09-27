//! Commitment-bound terminal reduction for the Falcon-1024 PIOP.
//!
//! All exact linear Falcon relations and all terminal values emitted by the
//! nonlinear sumchecks are folded into one streamed linear form in the committed
//! binary source.  One degree-two inner sumcheck reduces that form to one MLE
//! evaluation, which is authenticated by the ordinary runtime-prime BitZ /
//! Ligerito opening.

use std::{collections::HashMap, sync::OnceLock};

use field::{BatchMulAcc, MergeAccumulator, RingOps, Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig, VerifierConfig},
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    ligerito_flock::{
        FlockCommitHint, IntEvalRsLigModQProof, prove_mle_eval_mod_q_ligerito,
        validate_ligerito_commitment, verify_mle_eval_mod_q_ligerito_runtime,
    },
    pcs::smallest_generator,
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::eq_table,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{
            packed::{
                ColumnMajorPackedBits, PackedInput, StreamingCoefficientSource, StreamingMle,
            },
            prove_inner_sumcheck,
        },
    },
    transcript::traits::Transcript,
};

use super::{
    FalconError, FalconPiopProof, FalconPublicKey, FalconSignatureCt, FalconSourceLayout,
    FalconSourceOffsets, FalconSourceWitness, FalconVerificationTrace, HASH_TO_POINT_SAMPLES, N, Q,
    decode_public_key, decode_signature_ct, encode_signature_ct,
    piop::{prove_falcon_piop, security_schedule, verify_falcon_piop},
};

#[path = "opening_compact.rs"]
mod compact;
#[path = "opening_native.rs"]
mod native;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

#[cfg(test)]
const LINEAR_STRIDE: usize = 1 << 20;
const COMPACTION_LEAVES: usize = 1 << 11;
const KECCAK_PERMUTATIONS: usize = 20;
const KECCAK_ROUNDS: usize = 24;
const KECCAK_LANES: usize = 25;
const RATE_BYTES: usize = 136;

const ROTATION: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

struct LinearPointGrinding;
impl GrindingDomain for LinearPointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/linear-point/v1";
}

struct BindingGrinding;
impl GrindingDomain for BindingGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/binding/v1";
}

/// Public keys, 32-byte messages, and exact signatures verified by the proof.
/// Their witness copies are authenticated by the terminal linear binder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconPublicStatement {
    pub public_keys: Vec<FalconPublicKey>,
    pub messages: Vec<[u8; 32]>,
    pub signatures: Vec<FalconSignatureCt>,
}

impl FalconPublicStatement {
    pub fn from_bytes(
        public_keys: &[&[u8]],
        messages: &[&[u8]],
        signatures: &[&[u8]],
    ) -> Result<Self, FalconError> {
        if public_keys.len() != messages.len()
            || public_keys.len() != signatures.len()
            || public_keys.is_empty()
        {
            return Err(piop("public statement batch mismatch"));
        }
        let public_keys = public_keys
            .iter()
            .map(|bytes| decode_public_key(bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let messages = messages
            .iter()
            .map(|message| {
                (*message)
                    .try_into()
                    .map_err(|_| piop("Falcon benchmark messages must contain 32 bytes"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let signatures = signatures
            .iter()
            .map(|bytes| decode_signature_ct(bytes))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            public_keys,
            messages,
            signatures,
        })
    }

    pub(super) fn validate(&self, batch: usize) -> Result<(), FalconError> {
        if self.batch() != batch || self.messages.len() != batch || self.signatures.len() != batch {
            return Err(piop("public statement batch mismatch"));
        }
        for key in &self.public_keys {
            for (index, &coefficient) in key.h.iter().enumerate() {
                if i64::from(coefficient) >= Q {
                    return Err(FalconError::PublicKeyCoefficient { index });
                }
            }
        }
        for signature in &self.signatures {
            encode_signature_ct(signature)?;
        }
        Ok(())
    }

    pub fn batch(&self) -> usize {
        self.public_keys.len()
    }
}

/// Prime-field reductions through their authenticated-source opening claim.
/// This prefix is sound only when the caller has already bound the source
/// commitment and public statement, and subsequently authenticates its claim.
#[derive(Clone, Debug)]
pub struct FalconBindingPrefixProof {
    pub piop: FalconPiopProof,
    pub native_ring: Option<super::native_ring::NativeRingProof>,
    pub linear_point_nonce: Option<u64>,
    pub binding: SumcheckProof<F, 3>,
    pub binding_point: Vec<F>,
    /// `[coefficient MLE, source MLE]` at `binding_point`.
    pub binding_terminal: [F; 2],
    pub binding_nonces: Vec<u64>,
}

pub(super) struct FalconOpeningClaim {
    pub row_weights: Vec<u128>,
    pub col_weights: Vec<u128>,
    pub value: u128,
    pub modulus: u128,
}

/// One commitment-bound Falcon proof, including the final BitZ opening.
pub struct FalconBitzProof {
    pub prefix: FalconBindingPrefixProof,
    pub opening: IntEvalRsLigModQProof,
}

impl core::ops::Deref for FalconBitzProof {
    type Target = FalconBindingPrefixProof;
    fn deref(&self) -> &Self::Target {
        &self.prefix
    }
}

impl core::ops::DerefMut for FalconBitzProof {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.prefix
    }
}

/// Prove a batch against one already-created binary source commitment.
#[allow(clippy::too_many_arguments)]
pub fn prove_falcon_bitz(
    transcript: &mut (impl Transcript + Send),
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    hint: &FlockCommitHint,
    target_bits: usize,
    pc: &ProverConfig,
) -> Result<FalconBitzProof, FalconError> {
    if layout.is_hybrid() {
        return Err(piop(
            "compact Falcon source requires the hybrid proof adapter",
        ));
    }
    validate_prover_inputs(layout, statement, traces, source, hint, target_bits, pc)?;
    bind_statement(transcript, layout, statement, &hint.commitment, target_bits)?;
    let (prefix, claim) =
        prove_binding_prefix(transcript, layout, statement, traces, source, target_bits)?;
    let opening = prove_mle_eval_mod_q_ligerito(
        transcript,
        hint,
        &layout.bitz_params(),
        &claim.row_weights,
        modulus_bits(claim.modulus),
        smallest_generator(),
        pc,
    );
    Ok(FalconBitzProof { prefix, opening })
}

/// The caller binds all commitments and the statement before this call, and
/// authenticates the returned source claim afterward. Hybrid callers must
/// additionally prove SHAKE and its links to the compact arithmetic source.
pub(super) fn prove_binding_prefix(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    target_bits: usize,
) -> Result<(FalconBindingPrefixProof, FalconOpeningClaim), FalconError> {
    statement.validate(layout.batch())?;
    if statement.batch() != layout.batch()
        || statement.messages.len() != layout.batch()
        || traces.len() != layout.batch()
        || source.layout() != layout
        || !matches!(target_bits, 100 | 128)
        || traces
            .iter()
            .zip(&statement.public_keys)
            .any(|(trace, key)| &trace.public_key != key)
        || traces
            .iter()
            .zip(&statement.signatures)
            .any(|(trace, signature)| &trace.signature != signature)
    {
        return Err(piop("Falcon prefix input shape mismatch"));
    }
    let algebraic = prove_falcon_piop(transcript, layout, traces, target_bits)?;
    let field = field_from_modulus(algebraic.modulus)?;
    let (native_ring, native_claim) = if layout.is_hybrid() {
        let (proof, claim) = super::native_ring::prove(
            transcript,
            layout,
            statement,
            traces,
            &field,
            target_bits as u32,
        )?;
        (Some(proof), Some(claim))
    } else {
        (None, None)
    };

    let linear_point_nonce = grind_linear_point(transcript, layout, target_bits, None)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let binding = prepare_binding_form(
        transcript,
        layout,
        statement,
        &algebraic,
        &linear_point,
        &field,
        native_claim,
    )?;
    let target_span = tracing::info_span!("falcon_arithmetic:binding_target").entered();
    let target = binding.target()?;
    drop(target_span);

    let _binding_span = tracing::info_span!("falcon_arithmetic:binding_inner").entered();
    // Finish the shared cache before coefficient partitions start. In
    // particular, do not make Rayon workers wait on a parallel initializer.
    if !layout.is_hybrid() {
        binding.prover_ring_cache();
    }
    if layout.is_hybrid() {
        let _template_span = tracing::info_span!("falcon_arithmetic:binding_template").entered();
        binding.prepared_compact_template()?;
    }
    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let packed_source = ColumnMajorPackedBits::new(source.rows(), layout.row_vars());
    let coefficients = StreamingMle::new(&binding);
    // Three packed rounds reduce ternary-prefix work for the hybrid source.
    // The larger tail table won the native batch benchmark; this choice has
    // no effect on the sumcheck messages or verifier. Keep the legacy tuning.
    let prefix_vars = if layout.is_hybrid() { 3 } else { 4 };
    let input = || {
        PackedInput::new(
            &coefficients,
            &packed_source,
            source_rounds(layout),
            layout.source_bits(),
            prefix_vars,
        )
    };
    let output = if target_bits == 128 {
        let mut boundary = ProverGrindingRoundBoundary::<BindingGrinding>::with_round_offset(
            security_schedule(layout, target_bits)?.binding_round_bits,
            0,
        );
        let output = prove_inner_sumcheck(&field, transcript, target, input(), (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        (output, boundary.into_nonces())
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let output = prove_inner_sumcheck(&field, transcript, target, input(), (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        (output, Vec::new())
    };
    let (output, binding_nonces) = output;
    let binding_terminal = output.terminal_evaluations;
    absorb_field_elements(transcript, &binding_terminal, &field);
    bind_opening_claim(
        transcript,
        algebraic.modulus,
        &output.point,
        binding_terminal[1],
        &field,
    );
    let claim = opening_claim(layout, &output.point, binding_terminal[1], &field)?;
    let prefix = FalconBindingPrefixProof {
        piop: algebraic,
        native_ring,
        linear_point_nonce,
        binding: output.proof,
        binding_point: output.point,
        binding_terminal,
        binding_nonces,
    };
    Ok((prefix, claim))
}

/// Verify the nonlinear PIOP, rebuild its public linear functional, reduce it
/// to one source MLE value, and authenticate that value against `commitment`.
#[allow(clippy::too_many_arguments)]
pub fn verify_falcon_bitz(
    transcript: &mut (impl Transcript + Send),
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    proof: &FalconBitzProof,
    target_bits: usize,
    vc: &VerifierConfig,
) -> Result<(), FalconError> {
    if layout.is_hybrid() {
        return Err(piop(
            "compact Falcon source requires the hybrid proof adapter",
        ));
    }
    validate_verifier_inputs(layout, statement, commitment, target_bits, vc)?;
    bind_statement(transcript, layout, statement, commitment, target_bits)?;
    let claim = verify_binding_prefix(transcript, layout, statement, &proof.prefix, target_bits)?;
    verify_mle_eval_mod_q_ligerito_runtime(
        transcript,
        commitment,
        &proof.opening,
        &layout.bitz_params(),
        &claim.row_weights,
        &claim.col_weights,
        smallest_generator(),
        claim.value,
        claim.modulus,
        modulus_bits(claim.modulus),
        None,
        vc,
    )
    .map_err(|error| piop(format!("{error:?}")))
}

pub(super) fn verify_binding_prefix(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    proof: &FalconBindingPrefixProof,
    target_bits: usize,
) -> Result<FalconOpeningClaim, FalconError> {
    statement.validate(layout.batch())?;
    if statement.batch() != layout.batch()
        || statement.messages.len() != layout.batch()
        || !matches!(target_bits, 100 | 128)
        || (target_bits == 128 && proof.linear_point_nonce.is_none())
        || (target_bits == 100 && !proof.binding_nonces.is_empty())
    {
        return Err(piop("Falcon prefix proof shape mismatch"));
    }
    verify_falcon_piop(transcript, layout, &proof.piop, target_bits)?;
    let field = field_from_modulus(proof.piop.modulus)?;
    let native_claim = match (layout.is_hybrid(), &proof.native_ring) {
        (true, Some(native)) => Some(super::native_ring::verify(
            transcript,
            layout,
            statement,
            native,
            &field,
            target_bits as u32,
        )?),
        (false, None) => None,
        _ => return Err(piop("native ring proof does not match source layout")),
    };

    grind_linear_point(transcript, layout, target_bits, proof.linear_point_nonce)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let binding = prepare_binding_form(
        transcript,
        layout,
        statement,
        &proof.piop,
        &linear_point,
        &field,
        native_claim,
    )?;
    let target = binding.target()?;
    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<BindingGrinding>::new(
            security_schedule(layout, target_bits)?.binding_round_bits,
            &proof.binding_nonces,
        );
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.binding],
            transcript,
            &[target],
            source_rounds(layout),
            &field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.binding],
            transcript,
            &[target],
            source_rounds(layout),
            &field,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    crate::sumcheck::proof::validate_field_elements(&proof.binding_terminal, &field)
        .map_err(|error| piop(error.to_string()))?;
    if point != proof.binding_point
        || final_claims[0] != field.mul(&proof.binding_terminal[0], &proof.binding_terminal[1])
    {
        return Err(piop("shared inner terminal mismatch"));
    }
    if binding.evaluate(&point)? != proof.binding_terminal[0] {
        return Err(piop("shared coefficient MLE mismatch"));
    }
    absorb_field_elements(transcript, &proof.binding_terminal, &field);
    bind_opening_claim(
        transcript,
        proof.piop.modulus,
        &point,
        proof.binding_terminal[1],
        &field,
    );
    opening_claim(layout, &point, proof.binding_terminal[1], &field)
}

fn opening_claim(
    layout: &FalconSourceLayout,
    point: &[F],
    value: F,
    field: &Cfg,
) -> Result<FalconOpeningClaim, FalconError> {
    Ok(FalconOpeningClaim {
        row_weights: canonical_eq_weights(&point[..layout.row_vars()], field)?,
        col_weights: canonical_eq_weights(&point[layout.row_vars()..], field)?,
        value: canonical(value, field),
        modulus: field.modulus_u128(),
    })
}

fn validate_prover_inputs(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    hint: &FlockCommitHint,
    target_bits: usize,
    pc: &ProverConfig,
) -> Result<(), FalconError> {
    statement.validate(layout.batch())?;
    if statement.batch() != layout.batch()
        || statement.messages.len() != layout.batch()
        || traces.len() != layout.batch()
        || source.layout() != layout
        || !matches!(target_bits, 100 | 128)
    {
        return Err(piop("Falcon prover input shape mismatch"));
    }
    for ((trace, public_key), signature) in traces
        .iter()
        .zip(&statement.public_keys)
        .zip(&statement.signatures)
    {
        if &trace.public_key != public_key || &trace.signature != signature {
            return Err(piop("trace does not match the public Falcon statement"));
        }
    }
    if source.rows() != hint.rows() {
        return Err(piop("source witness does not match the commitment hint"));
    }
    validate_ligerito_commitment(&hint.commitment, pc)
        .map_err(|error| piop(format!("commitment: {error:?}")))
}

fn validate_verifier_inputs(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    target_bits: usize,
    vc: &VerifierConfig,
) -> Result<(), FalconError> {
    statement.validate(layout.batch())?;
    if statement.batch() != layout.batch()
        || statement.messages.len() != layout.batch()
        || !matches!(target_bits, 100 | 128)
    {
        return Err(piop("Falcon verifier input shape mismatch"));
    }
    validate_ligerito_commitment(commitment, vc)
        .map_err(|error| piop(format!("commitment: {error:?}")))
}

fn bind_statement(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    target_bits: usize,
) -> Result<(), FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/commitment-bound/v4");
    transcript.absorb_slice(&[u8::from(layout.is_hybrid())]);
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
    transcript
        .absorb_slice(&bincode::serialize(commitment).map_err(|error| piop(error.to_string()))?);
    for ((public_key, message), signature) in statement
        .public_keys
        .iter()
        .zip(&statement.messages)
        .zip(&statement.signatures)
    {
        for coefficient in public_key.h.iter() {
            transcript.absorb_inner(&coefficient.to_le_bytes());
        }
        transcript.absorb_inner(message);
        transcript.absorb_inner(&encode_signature_ct(signature)?);
    }
    Ok(())
}

fn grind_linear_point(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    target_bits: usize,
    nonce: Option<u64>,
) -> Result<Option<u64>, FalconError> {
    if target_bits == 100 {
        return if nonce.is_none() {
            Ok(None)
        } else {
            Err(piop("unexpected linear-point grinding nonce"))
        };
    }
    match nonce {
        None => grind_and_absorb(
            transcript,
            GrindingRound::<LinearPointGrinding>::new(0),
            security_schedule(layout, target_bits)?.linear_point_bits,
        )
        .map(Some)
        .map_err(|error| piop(error.to_string())),
        Some(nonce) => verify_and_absorb(
            transcript,
            GrindingRound::<LinearPointGrinding>::new(0),
            security_schedule(layout, target_bits)?.linear_point_bits,
            nonce,
        )
        .map(|()| Some(nonce))
        .map_err(|error| piop(error.to_string())),
    }
}

/// A prepared public linear operator. Challenges are consumed only here;
/// every subsequent traversal is deterministic and independent of the witness.
struct BindingForm<'a> {
    layout: &'a FalconSourceLayout,
    statement: &'a FalconPublicStatement,
    proof: &'a FalconPiopProof,
    field: &'a Cfg,
    linear_weights: crate::poly::mle::EqualityWeights<F>,
    local_linear_weights: crate::poly::mle::EqualityWeights<F>,
    ring_row_weights: Vec<F>,
    ring_instance_weights: Vec<F>,
    prover_ring_cache: OnceLock<ProverRingCache>,
    compact_linear: OnceLock<compact::CompiledTemplate>,
    compact_instances: Vec<OnceLock<compact::CompiledCoefficients>>,
    compact_weights: OnceLock<compact::PreparedWeights>,
    local_linear_point: Vec<F>,
    eta: F,
    native_claim: Option<super::native_ring::PreparedNativeClaim>,
}

/// Unscaled adjoints shared by instances with the same complete public key.
/// Only prover coefficient emission initializes this cache.
struct ProverRingCache {
    adjoints: Vec<Vec<F>>,
    instance_keys: Vec<usize>,
}

trait CoefficientSink {
    fn instances(&self, batch: usize) -> std::ops::Range<usize> {
        0..batch
    }
    fn enabled(&self) -> bool {
        true
    }
    fn needs_constants(&self) -> bool {
        true
    }
    fn add(&mut self, index: usize, coefficient: F);
    fn add_word(&mut self, base: usize, width: usize, signed: bool, mut scale: F, field: &Cfg) {
        if !self.enabled() {
            return;
        }
        for bit in 0..width {
            let value = if signed && bit + 1 == width {
                field.sub(&field.zero(), &scale)
            } else {
                scale
            };
            self.add(base + bit, value);
            scale = field.add(&scale, &scale);
        }
    }
    fn add_encoded_word(
        &mut self,
        base: usize,
        offset: usize,
        weighted: bool,
        mut scale: F,
        field: &Cfg,
    ) {
        if !self.enabled() {
            return;
        }
        for bit in 0..12 {
            let stream = offset + 11 - bit;
            let index = base + 8 * (stream / 8) + 7 - stream % 8;
            self.add(
                index,
                if bit == 11 {
                    field.sub(&field.zero(), &scale)
                } else {
                    scale
                },
            );
            if weighted {
                scale = field.add(&scale, &scale);
            }
        }
    }
    fn bind_instances(&self, weights: &[F], _field: &Cfg) -> Vec<(usize, F)> {
        self.instances(weights.len())
            .map(|i| (i, weights[i]))
            .collect()
    }
    fn add_repeated(&mut self, index: usize, coefficient: F) {
        self.add(index, coefficient);
    }
}

struct ConstantsOnly;
impl CoefficientSink for ConstantsOnly {
    fn enabled(&self) -> bool {
        false
    }
    fn add(&mut self, _: usize, _: F) {}
}

struct EvaluatingSink<'a> {
    weights: crate::poly::mle::EqualityWeights<F>,
    local_weights: crate::poly::mle::EqualityWeights<F>,
    instance_weights: Vec<F>,
    field: &'a Cfg,
    value: F,
}
impl CoefficientSink for EvaluatingSink<'_> {
    fn add(&mut self, index: usize, coefficient: F) {
        self.value = self.field.add(
            &self.value,
            &self.field.mul(&coefficient, &self.weights.at(index)),
        );
    }
    fn bind_instances(&self, weights: &[F], field: &Cfg) -> Vec<(usize, F)> {
        let scale = weights
            .iter()
            .zip(&self.instance_weights)
            .fold(field.zero(), |sum, (row, column)| {
                field.add(&sum, &field.mul(row, column))
            });
        vec![(0, scale)]
    }
    fn add_repeated(&mut self, index: usize, coefficient: F) {
        self.value = self.field.add(
            &self.value,
            &self.field.mul(&coefficient, &self.local_weights.at(index)),
        );
    }
}

struct RepeatedScaledSink<'a, S> {
    sink: &'a mut S,
    scale: F,
    field: &'a Cfg,
}
impl<S: CoefficientSink> CoefficientSink for RepeatedScaledSink<'_, S> {
    fn add(&mut self, index: usize, coefficient: F) {
        self.sink
            .add_repeated(index, self.field.mul(&coefficient, &self.scale));
    }
}

fn factored_weights(
    point: &[F],
    field: &Cfg,
) -> Result<crate::poly::mle::EqualityWeights<F>, FalconError> {
    let split = point.len() / 2;
    let low = eq_table(&point[..split], field).map_err(|e| piop(e.to_string()))?;
    let high = eq_table(&point[split..], field).map_err(|e| piop(e.to_string()))?;
    Ok(crate::poly::mle::EqualityWeights::from_tables(
        low, high, field,
    ))
}

fn prepare_binding_form<'a>(
    transcript: &mut impl Transcript,
    layout: &'a FalconSourceLayout,
    statement: &'a FalconPublicStatement,
    proof: &'a FalconPiopProof,
    linear_point: &[F],
    field: &'a Cfg,
    native_claim: Option<super::native_ring::PreparedNativeClaim>,
) -> Result<BindingForm<'a>, FalconError> {
    let linear_weights = factored_weights(linear_point, field)?;
    // The preceding linear-point nonce protects one atomic challenge block:
    // its row coordinates followed by eta. No prover message intervenes.
    // Include eta's degree (batch + 12 in hybrid mode) in that block's bound.
    transcript.absorb_slice(b"bitz/falcon1024-ct/terminal-collapse/v1");
    let eta = squeeze(transcript, field)?;
    let local_linear_vars = layout.linear_stride().ilog2() as usize;
    let local_linear_weights = factored_weights(&linear_point[..local_linear_vars], field)?;
    let ring_start = layout.linear_rows() - N;
    let ring_row_weights = (0..if layout.is_hybrid() { 0 } else { N })
        .map(|i| local_linear_weights.at(ring_start + i))
        .collect();
    let mut ring_instance_weights = eq_table(&linear_point[local_linear_vars..], field)
        .map_err(|error| piop(error.to_string()))?;
    ring_instance_weights.truncate(layout.batch());
    Ok(BindingForm {
        layout,
        statement,
        proof,
        field,
        linear_weights,
        local_linear_weights,
        ring_row_weights,
        ring_instance_weights,
        prover_ring_cache: OnceLock::new(),
        compact_linear: OnceLock::new(),
        compact_instances: (0..layout.batch()).map(|_| OnceLock::new()).collect(),
        compact_weights: OnceLock::new(),
        local_linear_point: linear_point[..local_linear_vars].to_vec(),
        eta,
        native_claim,
    })
}

#[cfg(test)]
fn ring_adjoint(h: &[u16; N], weights: &[F], field: &Cfg) -> Vec<F> {
    let lifted = std::array::from_fn(|i| unsigned(u128::from(h[i]), field));
    ring_adjoint_field(&lifted, weights, field)
}

/// Returns -H^T weights, where H^T is negacyclic multiplication by
/// h*(X)=h[0]-sum_{i>0}h[N-i]X^i. Field-valued keys also support contraction
/// across instances; they must not be reduced modulo the Falcon modulus Q.
/// Karatsuba works in the sampled proof field without roots of unity or CRT.
fn ring_adjoint_field(h: &[F; N], weights: &[F], field: &Cfg) -> Vec<F> {
    let multiplier = PreparedPolynomialProduct::new(weights, field);
    let mut scratch = RingScratch::new(field);
    scratch.adjoint[0] = h[0];
    for i in 1..N {
        scratch.adjoint[i] = field.sub(&field.zero(), &h[N - i]);
    }
    scratch.evaluate(&multiplier)
}

const KARATSUBA_LEAF: usize = 32;

/// The challenge-dependent right operand is shared by every public key.
/// Store its complete Karatsuba tree once, in preorder, instead of rebuilding
/// the same sums for each key. All tree shapes depend only on public lengths.
struct PreparedPolynomialProduct<'a> {
    field: &'a Cfg,
    right_tree: Vec<F>,
    leaf_reducer: field::PreparedProductReduction<'a, 2>,
}

impl<'a> PreparedPolynomialProduct<'a> {
    fn tree_len(n: usize) -> usize {
        if n <= KARATSUBA_LEAF {
            n
        } else {
            n + 3 * Self::tree_len(n / 2)
        }
    }

    fn new(right: &[F], field: &'a Cfg) -> Self {
        assert!(right.len().is_power_of_two());
        fn append_tree(right: &[F], tree: &mut Vec<F>, field: &Cfg) {
            tree.extend_from_slice(right);
            if right.len() <= KARATSUBA_LEAF {
                return;
            }
            let half = right.len() / 2;
            append_tree(&right[..half], tree, field);
            append_tree(&right[half..], tree, field);
            let sum: Vec<_> = (0..half)
                .map(|i| field.add(&right[i], &right[half + i]))
                .collect();
            append_tree(&sum, tree, field);
        }
        let mut right_tree = Vec::with_capacity(Self::tree_len(right.len()));
        append_tree(right, &mut right_tree, field);
        Self {
            field,
            right_tree,
            leaf_reducer: field.prepare_product_reduction(right.len().min(KARATSUBA_LEAF)),
        }
    }

    fn multiply(&self, left: &[F], out: &mut [F], scratch: &mut [F]) {
        self.multiply_node(left, &self.right_tree, out, scratch);
    }

    fn multiply_node(&self, left: &[F], tree: &[F], out: &mut [F], scratch: &mut [F]) {
        let n = left.len();
        debug_assert_eq!(out.len(), 2 * n);
        let field = self.field;
        if n <= KARATSUBA_LEAF {
            // At most 32 canonical products per coefficient. The five-limb
            // accumulator holds this bound even for a full 128-bit modulus;
            // reduction happens once per coefficient, not once per product.
            for (degree, coefficient) in out[..2 * n - 1].iter_mut().enumerate() {
                let mut sum = <Cfg as BatchMulAcc<F>>::Accumulator::zero();
                for i in (degree + 1).saturating_sub(n)..n.min(degree + 1) {
                    field.mul_acc(&mut sum, &left[i], &tree[degree - i]);
                }
                *coefficient = self.leaf_reducer.reduce(sum);
            }
            out[2 * n - 1] = field.zero();
            return;
        }
        let half = n / 2;
        let child_len = Self::tree_len(half);
        let children = &tree[n..];
        self.multiply_node(
            &left[..half],
            &children[..child_len],
            &mut out[..n],
            scratch,
        );
        self.multiply_node(
            &left[half..],
            &children[child_len..2 * child_len],
            &mut out[n..],
            scratch,
        );
        let (left_sum, rest) = scratch.split_at_mut(half);
        let (middle, rest) = rest.split_at_mut(n);
        for i in 0..half {
            left_sum[i] = field.add(&left[i], &left[half + i]);
        }
        self.multiply_node(left_sum, &children[2 * child_len..], middle, rest);
        // Read both low/high products before their overlapping output region
        // is modified by the middle product.
        for i in 0..n {
            middle[i] = field.sub(&field.sub(&middle[i], &out[i]), &out[n + i]);
        }
        for i in 0..n {
            out[half + i] = field.add(&out[half + i], &middle[i]);
        }
    }
}

struct RingScratch {
    adjoint: Vec<F>,
    product: Vec<F>,
    work: Vec<F>,
}

impl RingScratch {
    fn new(field: &Cfg) -> Self {
        Self {
            adjoint: vec![field.zero(); N],
            product: vec![field.zero(); 2 * N],
            // S(n) = 3n/2 + S(n/2) < 3n.
            work: vec![field.zero(); 3 * N],
        }
    }

    fn evaluate(&mut self, multiplier: &PreparedPolynomialProduct<'_>) -> Vec<F> {
        multiplier.multiply(&self.adjoint, &mut self.product, &mut self.work);
        (0..N)
            .map(|i| multiplier.field.sub(&self.product[N + i], &self.product[i]))
            .collect()
    }
}

impl BindingForm<'_> {
    fn prover_ring_cache(&self) -> &ProverRingCache {
        if let Some(cache) = self.prover_ring_cache.get() {
            return cache;
        }
        let _span = tracing::info_span!("falcon_arithmetic:ring_adjoints").entered();
        let mut keys = HashMap::new();
        let mut unique = Vec::new();
        let instance_keys = self
            .statement
            .public_keys
            .iter()
            .map(|key| {
                // Equality compares all coefficients; first occurrence fixes a
                // deterministic index independently of hash or worker ordering.
                *keys.entry(key.h.as_ref()).or_insert_with(|| {
                    let index = unique.len();
                    unique.push(key.h.as_ref());
                    index
                })
            })
            .collect();
        let multiplier = PreparedPolynomialProduct::new(&self.ring_row_weights, self.field);
        let evaluate = |scratch: &mut RingScratch, h: &&[u16; N]| {
            scratch.adjoint[0] = unsigned(u128::from(h[0]), self.field);
            for i in 1..N {
                scratch.adjoint[i] = self.field.sub(
                    &self.field.zero(),
                    &unsigned(u128::from(h[N - i]), self.field),
                );
            }
            scratch.evaluate(&multiplier)
        };
        #[cfg(feature = "parallel")]
        let adjoints = unique
            .par_iter()
            .map_init(|| RingScratch::new(self.field), evaluate)
            .collect();
        #[cfg(not(feature = "parallel"))]
        let adjoints = {
            let mut scratch = RingScratch::new(self.field);
            unique.iter().map(|h| evaluate(&mut scratch, h)).collect()
        };
        // Normal proving prewarms this cache. Other callers may race to build
        // it, but never block a Rayon worker while another initializer needs it.
        let _ = self.prover_ring_cache.set(ProverRingCache {
            adjoints,
            instance_keys,
        });
        self.prover_ring_cache
            .get()
            .expect("prepared ring adjoints")
    }

    fn emit(&self, coefficients: &mut impl CoefficientSink) -> Result<F, FalconError> {
        let Self {
            layout,
            statement,
            proof,
            field,
            linear_weights,
            eta,
            ..
        } = self;
        let linear_constant =
            add_linear_constraints(coefficients, linear_weights, layout, statement, field)?;
        if coefficients.enabled() && !layout.is_hybrid() {
            let ring_cache = self.prover_ring_cache();
            let offsets = layout.offsets();
            for instance in coefficients.instances(layout.batch()) {
                let key = ring_cache.instance_keys[instance];
                for (j, weight) in ring_cache.adjoints[key].iter().enumerate() {
                    add_signed_source_scaled(
                        coefficients,
                        instance * layout.signature_stride(),
                        &offsets,
                        j,
                        field.mul(&self.ring_instance_weights[instance], weight),
                        field,
                    );
                }
            }
        }
        let mut target = field.sub(&field.zero(), &linear_constant);
        let mut scale = *eta;
        add_norm_claims(
            coefficients,
            &mut target,
            &mut scale,
            *eta,
            layout,
            proof,
            field,
        )?;
        add_keccak_claims(
            coefficients,
            &mut target,
            &mut scale,
            *eta,
            layout,
            proof,
            field,
        )?;
        add_compaction_product_claims(
            coefficients,
            &mut target,
            &mut scale,
            *eta,
            layout,
            proof,
            field,
        )?;
        if layout.is_hybrid() {
            native::add_leaf_claims(
                coefficients,
                &mut target,
                &mut scale,
                *eta,
                layout,
                proof,
                field,
            )?;
        }
        add_product_tree_claims(
            coefficients,
            &mut target,
            &mut scale,
            *eta,
            layout,
            proof,
            field,
        )?;
        self.add_native_claim(coefficients, &mut target, scale)?;
        Ok(target)
    }

    fn target(&self) -> Result<F, FalconError> {
        if self.layout.is_hybrid() {
            self.compact_target()
        } else {
            self.emit(&mut ConstantsOnly)
        }
    }

    fn evaluate(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != source_rounds(self.layout) {
            return Err(piop("binding endpoint dimension mismatch"));
        }
        if self.layout.is_hybrid() {
            return self.evaluate_compact(point);
        }
        let field = self.field;
        let layout = self.layout;
        let local_vars = layout.signature_stride().ilog2() as usize;
        let mut sink = EvaluatingSink {
            weights: factored_weights(point, field)?,
            local_weights: factored_weights(&point[..local_vars], field)?,
            instance_weights: eq_table(&point[local_vars..], field)
                .map_err(|e| piop(e.to_string()))?,
            field,
            value: field.zero(),
        };
        // All linear wiring is shared across signatures except H*s2. Contract
        // the instance factor first, then evaluate the local template once.
        let scale = sink.bind_instances(&self.ring_instance_weights, field)[0].1;
        let local_layout = layout.single_instance();
        let local_statement = FalconPublicStatement {
            public_keys: vec![self.statement.public_keys[0].clone()],
            messages: vec![self.statement.messages[0]],
            signatures: vec![self.statement.signatures[0].clone()],
        };
        add_linear_constraints(
            &mut RepeatedScaledSink {
                sink: &mut sink,
                scale,
                field,
            },
            &self.local_linear_weights,
            &local_layout,
            &local_statement,
            field,
        )?;
        // The local signed-bit endpoint is common to every instance, so the
        // ring contribution contracts to one adjoint of sum_s alpha_s beta_s h_s.
        // Only live instances contribute; padding carries no public key.
        let mut combined_key = [field.zero(); N];
        for ((key, row_weight), endpoint_weight) in self
            .statement
            .public_keys
            .iter()
            .zip(&self.ring_instance_weights)
            .zip(&sink.instance_weights)
        {
            let scale = field.mul(row_weight, endpoint_weight);
            for (combined, &coefficient) in combined_key.iter_mut().zip(key.h.iter()) {
                *combined = field.add(
                    combined,
                    &field.mul(&scale, &unsigned(u128::from(coefficient), field)),
                );
            }
        }
        let ring_weights = ring_adjoint_field(&combined_key, &self.ring_row_weights, field);
        let offsets = layout.offsets();
        // The beta_s factors are already in combined_key. Use local endpoint
        // weights here, without multiplying by beta_0 a second time.
        let mut local_sink = RepeatedScaledSink {
            sink: &mut sink,
            scale: field.one(),
            field,
        };
        for (j, &weight) in ring_weights.iter().enumerate() {
            add_signed_source_scaled(&mut local_sink, 0, &offsets, j, weight, field);
        }
        let mut ignored_target = field.zero();
        let mut scale = self.eta;
        add_norm_claims(
            &mut sink,
            &mut ignored_target,
            &mut scale,
            self.eta,
            self.layout,
            self.proof,
            field,
        )?;
        add_keccak_claims(
            &mut sink,
            &mut ignored_target,
            &mut scale,
            self.eta,
            self.layout,
            self.proof,
            field,
        )?;
        add_compaction_product_claims(
            &mut sink,
            &mut ignored_target,
            &mut scale,
            self.eta,
            self.layout,
            self.proof,
            field,
        )?;
        add_product_tree_claims(
            &mut sink,
            &mut ignored_target,
            &mut scale,
            self.eta,
            self.layout,
            self.proof,
            field,
        )?;
        Ok(sink.value)
    }
}

impl StreamingCoefficientSource for BindingForm<'_> {
    fn num_vars(&self) -> usize {
        source_rounds(self.layout)
    }
    fn live_len(&self) -> usize {
        self.layout.source_bits()
    }
    fn partition_len(&self) -> usize {
        self.layout.signature_stride()
    }
    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        self.emit_partition(0..self.layout.batch(), emit)
    }
    fn for_each_partition(
        &self,
        partition: usize,
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        if partition >= self.layout.capacity() {
            return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
        }
        if partition >= self.layout.batch() {
            return Ok(());
        }
        self.emit_partition(partition..partition + 1, emit)
    }

    fn for_each_partition_block(
        &self,
        partition: usize,
        block_len: usize,
        emit: &mut impl FnMut(usize, &[F]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Option<Result<(), crate::sumcheck::SumcheckError>> {
        if !self.layout.is_hybrid() {
            return None;
        }
        Some((|| {
            if partition >= self.layout.capacity() {
                return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
            }
            if partition >= self.layout.batch() {
                return Ok(());
            }
            self.compact_instance(partition)
                .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?
                .emit_blocks(
                    partition * self.layout.signature_stride(),
                    block_len,
                    self.field,
                    emit,
                )
        })())
    }

    fn for_each_partition_folded_final(
        &self,
        partition: usize,
        weights: &[F],
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Option<Result<(), crate::sumcheck::SumcheckError>> {
        if !self.layout.is_hybrid() {
            return None;
        }
        Some((|| {
            if partition >= self.layout.capacity() {
                return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
            }
            if partition >= self.layout.batch() {
                return Ok(());
            }
            self.compact_instance(partition)
                .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?
                .emit_folded(
                    partition * self.layout.signature_stride(),
                    weights,
                    self.field,
                    emit,
                )
        })())
    }
}

impl BindingForm<'_> {
    fn emit_partition(
        &self,
        instances: std::ops::Range<usize>,
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        if self.layout.is_hybrid() {
            for instance in instances {
                self.compact_instance(instance)
                    .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?
                    .emit(instance * self.layout.signature_stride(), self.field, emit)?;
            }
            return Ok(());
        }
        struct Sink<'a, E> {
            instances: std::ops::Range<usize>,
            emit: &'a mut E,
            error: Option<crate::sumcheck::SumcheckError>,
        }
        impl<E: FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>> CoefficientSink
            for Sink<'_, E>
        {
            fn instances(&self, _: usize) -> std::ops::Range<usize> {
                self.instances.clone()
            }
            fn add(&mut self, index: usize, value: F) {
                if self.error.is_none() {
                    self.error = (self.emit)(index, value).err();
                }
            }
        }
        let mut sink = Sink {
            instances,
            emit,
            error: None,
        };
        self.emit(&mut sink)
            .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?;
        sink.error.map_or(Ok(()), Err)
    }
}

/// Emits affine wiring and constants. The key-dependent -H*s2 contribution
/// is emitted separately from the cached adjoints or the contracted key.
fn add_linear_constraints(
    coefficients: &mut impl CoefficientSink,
    weights: &crate::poly::mle::EqualityWeights<F>,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    field: &Cfg,
) -> Result<F, FalconError> {
    let offsets = layout.offsets();
    let mut constant = field.zero();
    let emit = coefficients.enabled();
    for instance in coefficients.instances(layout.batch()) {
        let base = instance * layout.signature_stride();
        let mut row = 0usize;
        let mut residual = |terms: &[(usize, i128)], c: i128| {
            let row_index = row;
            row += 1;
            if !emit && c == 0 {
                return;
            }
            let weight = weights.at(instance * layout.linear_stride() + row_index);
            for &(index, coefficient) in terms.iter().filter(|_| emit) {
                add_coefficient(
                    coefficients,
                    base + index,
                    mul_i(weight, coefficient, field),
                    field,
                );
            }
            if c != 0 {
                constant = field.add(&constant, &mul_i(weight, c, field));
            }
        };

        residual(&[(offsets.shared_one, 1)], -1);
        for bit in 0..256 {
            let expected = (statement.messages[instance][bit / 8] >> (bit % 8)) & 1;
            residual(&[(offsets.message + bit, 1)], -i128::from(expected));
        }
        for (byte, expected) in encode_signature_ct(&statement.signatures[instance])?
            .into_iter()
            .enumerate()
        {
            // The authenticated source contains bits, so each byte sum is in
            // [0,255] and equality in the proof field fixes all eight bits.
            let terms: [(usize, i128); 8] = std::array::from_fn(|bit| {
                (offsets.encoded_signature + 8 * byte + bit, 1i128 << bit)
            });
            residual(&terms, -i128::from(expected));
        }

        for i in 0..if layout.is_hybrid() { 0 } else { N } {
            let mut terms = Vec::with_capacity(16);
            for bit in 0..11 {
                terms.push((s2_bit_index(&offsets, i, bit), 1));
            }
            terms.push((s2_bit_index(&offsets, i, 11), -1));
            for bit in 0..4 {
                terms.push((offsets.s2_non_min_slack + 4 * i + bit, -(1i128 << bit)));
            }
            residual(&terms, 0);
        }

        if !layout.is_hybrid() {
            let mut terms = Vec::with_capacity(8);
            for permutation in 0..KECCAK_PERMUTATIONS {
                for round in 0..KECCAK_ROUNDS {
                    if !emit && (permutation != 0 || round != 0) {
                        for _ in 0..(5 * 64 + 25 * 64) {
                            residual(&[], 0);
                        }
                        continue;
                    }
                    let column_start = (permutation * KECCAK_ROUNDS + round) * 5 * 64;
                    for x in 0..5 {
                        for bit in 0..64 {
                            terms.clear();
                            let mut c = 0;
                            for y in 0..5 {
                                push_round_input(
                                    &mut terms,
                                    &mut c,
                                    &offsets,
                                    permutation,
                                    round,
                                    x,
                                    y,
                                    bit,
                                    1,
                                );
                            }
                            let column = column_start + x * 64 + bit;
                            terms.push((offsets.keccak_column_parities + column, -1));
                            terms.push((offsets.keccak_column_parity_quotients + 2 * column, -2));
                            terms.push((
                                offsets.keccak_column_parity_quotients + 2 * column + 1,
                                -4,
                            ));
                            residual(&terms, c);
                        }
                    }
                    for y in 0..5 {
                        for x in 0..5 {
                            let (source_x, source_y) = rho_pi_source(x, y);
                            let rotation = ROTATION[source_x][source_y] as usize;
                            for destination_bit in 0..64 {
                                let source_bit = (destination_bit + 64 - rotation) & 63;
                                let rotated_bit = (source_bit + 63) & 63;
                                terms.clear();
                                let mut c = 0;
                                push_round_input(
                                    &mut terms,
                                    &mut c,
                                    &offsets,
                                    permutation,
                                    round,
                                    source_x,
                                    source_y,
                                    source_bit,
                                    1,
                                );
                                terms.push((
                                    offsets.keccak_column_parities
                                        + column_start
                                        + ((source_x + 4) % 5) * 64
                                        + source_bit,
                                    1,
                                ));
                                terms.push((
                                    offsets.keccak_column_parities
                                        + column_start
                                        + ((source_x + 1) % 5) * 64
                                        + rotated_bit,
                                    1,
                                ));
                                let gate = keccak_gate(permutation, round, x, y, destination_bit);
                                terms.push((offsets.keccak_chi_inputs + gate, -1));
                                terms.push((offsets.keccak_parity_quotients + gate, -2));
                                residual(&terms, c);
                            }
                        }
                    }
                }
            }
        }

        for i in 0..HASH_TO_POINT_SAMPLES {
            let mut division = Vec::with_capacity(34);
            for bit in 0..16 {
                division.push((shake_word_bit_index(layout, &offsets, i, bit), 1i128 << bit));
            }
            push_unsigned_terms(
                &mut division,
                offsets.hash_quotients + 3 * i,
                3,
                -i128::from(Q),
            );
            push_value_terms(
                &mut division,
                offsets.hash_remainders + 14 * i,
                -1,
                layout.is_hybrid(),
            );
            residual(&division, 0);

            if !layout.is_hybrid() {
                let mut range = Vec::with_capacity(28);
                push_unsigned_terms(&mut range, offsets.hash_remainders + 14 * i, 14, 1);
                push_unsigned_terms(&mut range, offsets.hash_remainder_slack + 14 * i, 14, 1);
                residual(&range, -12_288);

                let mut q_range = Vec::with_capacity(6);
                push_unsigned_terms(&mut q_range, offsets.hash_quotients + 3 * i, 3, 1);
                push_unsigned_terms(&mut q_range, offsets.hash_quotient_slack + 3 * i, 3, 1);
                residual(&q_range, -5);
            }
            let mut prefix = Vec::with_capacity(23);
            push_unsigned_terms(&mut prefix, offsets.hash_prefixes + 11 * (i + 1), 11, 1);
            push_unsigned_terms(&mut prefix, offsets.hash_prefixes + 11 * i, 11, -1);
            prefix.push((offsets.hash_accept_ands + i, 1));
            residual(&prefix, -1);
        }
        let mut prefix_zero = Vec::with_capacity(11);
        push_unsigned_terms(&mut prefix_zero, offsets.hash_prefixes, 11, 1);
        residual(&prefix_zero, 0);
        residual(
            &[(offsets.hash_prefixes + 11 * HASH_TO_POINT_SAMPLES + 10, 1)],
            -1,
        );

        if !layout.is_hybrid() {
            for i in 0..N {
                let mut range = Vec::with_capacity(28);
                push_unsigned_terms(&mut range, offsets.s1 + 14 * i, 14, 1);
                push_unsigned_terms(&mut range, offsets.s1_range_slack + 14 * i, 14, 1);
                residual(&range, -12_288);
            }

            for i in 0..N {
                let mut ring = Vec::with_capacity(51);
                push_unsigned_terms(&mut ring, offsets.hash_point + 14 * i, 14, 1);
                push_unsigned_terms(&mut ring, offsets.s1 + 14 * i, 14, -1);
                push_signed_terms(
                    &mut ring,
                    offsets.ring_quotients + 23 * i,
                    23,
                    -i128::from(Q),
                );
                residual(&ring, 6_144);
            }
        }
        if row != layout.linear_rows() {
            return Err(piop(format!(
                "linear row inventory mismatch: built {row} rows"
            )));
        }
    }
    Ok(constant)
}

fn add_norm_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = factored_weights(&proof.norm.point, field)?;
    let instance_weights =
        eq_table(&proof.norm.instance_point, field).map_err(|error| piop(error.to_string()))?;
    add_norm_claims_prepared(
        coefficients,
        target,
        scale,
        eta,
        layout,
        proof,
        field,
        &weights,
        &instance_weights,
    )
}

#[allow(clippy::too_many_arguments)]
fn add_norm_claims_prepared(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
    weights: &crate::poly::mle::EqualityWeights<F>,
    instance_weights: &[F],
) -> Result<(), FalconError> {
    if instance_weights.len() != layout.capacity()
        || (1usize << proof.norm.point.len()) != N * layout.capacity()
    {
        return Err(piop("norm binding point dimension mismatch"));
    }
    let offsets = layout.offsets();
    for side in 0..2 {
        let mut constant = field.zero();
        for instance in coefficients.instances(layout.batch()) {
            let base = instance * layout.signature_stride();
            for i in 0..N {
                let weight = field.mul(
                    &field.mul(scale, &instance_weights[instance]),
                    &weights.at(instance * N + i),
                );
                if side == 0 {
                    add_value_scaled(
                        coefficients,
                        base + offsets.s1 + 14 * i,
                        weight,
                        layout.is_hybrid(),
                        field,
                    );
                    if coefficients.needs_constants() {
                        constant = field.add(&constant, &mul_i(weight, -6_144, field));
                    }
                } else {
                    add_signed_source_scaled(coefficients, base, &offsets, i, weight, field);
                }
            }
        }
        add_claim_target(
            target,
            *scale,
            proof.norm.terminal[side][0],
            constant,
            field,
        );
        *scale = field.mul(scale, &eta);
        let mut constant = field.zero();
        for instance in coefficients.instances(layout.batch()) {
            let base = instance * layout.signature_stride();
            for i in 0..N {
                let weight = field.mul(scale, &weights.at(instance * N + i));
                if side == 0 {
                    add_value_scaled(
                        coefficients,
                        base + offsets.s1 + 14 * i,
                        weight,
                        layout.is_hybrid(),
                        field,
                    );
                    if coefficients.needs_constants() {
                        constant = field.add(&constant, &mul_i(weight, -6_144, field));
                    }
                } else {
                    add_signed_source_scaled(coefficients, base, &offsets, i, weight, field);
                }
            }
        }
        add_claim_target(
            target,
            *scale,
            proof.norm.terminal[side][1],
            constant,
            field,
        );
        *scale = field.mul(scale, &eta);
    }
    let constant = field.zero();
    for instance in coefficients.instances(layout.batch()) {
        add_unsigned_scaled(
            coefficients,
            instance * layout.signature_stride() + offsets.norm_slack,
            27,
            field.mul(scale, &instance_weights[instance]),
            field,
        );
    }
    add_claim_target(target, *scale, proof.norm.slack, constant, field);
    *scale = field.mul(scale, &eta);
    Ok(())
}

fn add_keccak_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let Some(keccak_chi) = &proof.keccak_chi else {
        return if layout.is_hybrid() {
            Ok(())
        } else {
            Err(piop("missing Keccak binding claims"))
        };
    };
    if layout.is_hybrid() {
        return Err(piop("unexpected Keccak binding claims"));
    }
    let point = &keccak_chi.point;
    let batch_vars = layout.capacity().ilog2() as usize;
    if point.len() != 21 + batch_vars {
        return Err(piop("Keccak terminal point dimension mismatch"));
    }
    let local = factored_weights(&point[..20], field)?;
    let instances =
        eq_table(&point[20..20 + batch_vars], field).map_err(|e| piop(e.to_string()))?;
    let relation = point[20 + batch_vars];
    let scales = [
        *scale,
        field.mul(scale, &eta),
        field.mul(&field.mul(scale, &eta), &eta),
    ];
    let rel0: [F; 3] = scales.map(|s| field.mul(&s, &field.sub(&field.one(), &relation)));
    let rel1: [F; 3] = scales.map(|s| field.mul(&s, &relation));
    let half_c = field.mul(&rel1[2], &unsigned((field.modulus_u128() + 1) / 2, field));
    let gates = KECCAK_PERMUTATIONS * KECCAK_ROUNDS * KECCAK_LANES * 64;
    let mut iota_weight = field.zero();
    for permutation in 0..KECCAK_PERMUTATIONS {
        for round in 0..KECCAK_ROUNDS {
            let mut bits = super::keccak::ROUND_CONSTANTS[round];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                iota_weight = field.add(
                    &iota_weight,
                    &local.at(keccak_gate(permutation, round, 0, 0, bit)),
                );
                bits &= bits - 1;
            }
        }
    }
    let instance_sum = instances[..layout.batch()]
        .iter()
        .fold(field.zero(), |sum, w| field.add(&sum, w));
    let constant = field.mul(
        &instance_sum,
        &field.sub(
            &field.mul(&rel0[0], &eq_prefix_sum(&point[..20], gates, field)),
            &field.mul(&half_c, &iota_weight),
        ),
    );
    let claims = [
        keccak_chi.terminal.ax,
        keccak_chi.terminal.bx,
        keccak_chi.terminal.cx,
    ];
    for coordinate in 0..3 {
        *target = field.add(target, &field.mul(&scales[coordinate], &claims[coordinate]));
    }
    *target = field.sub(target, &constant);
    *scale = field.mul(&scales[2], &eta);
    if !coefficients.enabled() {
        return Ok(());
    }
    let seeds = [
        field.sub(&field.zero(), &rel0[0]),
        rel0[1],
        field.add(&rel1[0], &half_c),
        field.add(&rel0[2], &field.add(&rel1[1], &half_c)),
        half_c,
    ];
    let offsets = layout.offsets();
    for (instance, instance_weight) in
        coefficients.bind_instances(&instances[..layout.batch()], field)
    {
        let base = instance * layout.signature_stride();
        let seeds = seeds.map(|s| field.mul(&s, &instance_weight));
        for gate in 0..gates {
            let word = gate >> 6;
            let bit = gate & 63;
            let lane = word % 25;
            let x = lane % 5;
            let y = lane / 5;
            let round = (word / 25) % 24;
            let chi_base = word - lane + y * 5;
            let indices = [
                offsets.keccak_chi_inputs + 64 * (chi_base + (x + 1) % 5) + bit,
                offsets.keccak_chi_inputs + 64 * (chi_base + (x + 2) % 5) + bit,
                offsets.keccak_chi_inputs + gate,
                offsets.keccak_chi_ands + gate,
                offsets.keccak_round_states + gate,
            ];
            let weight = local.at(gate);
            for coordinate in 0..5 {
                let mut value = field.mul(&seeds[coordinate], &weight);
                if coordinate == 4
                    && !(lane == 0 && (super::keccak::ROUND_CONSTANTS[round] >> bit) & 1 == 1)
                {
                    value = field.sub(&field.zero(), &value);
                }
                coefficients.add_repeated(base + indices[coordinate], value);
            }
        }
    }
    Ok(())
}

/// Sum equality weights in [0, end) without expanding the Boolean cube.
fn eq_prefix_sum(point: &[F], end: usize, field: &Cfg) -> F {
    if end == 1usize << point.len() {
        return field.one();
    }
    let mut prefix = field.one();
    let mut total = field.zero();
    for bit in (0..point.len()).rev() {
        let low = field.mul(&prefix, &field.sub(&field.one(), &point[bit]));
        if (end >> bit) & 1 == 1 {
            total = field.add(&total, &low);
            prefix = field.mul(&prefix, &point[bit]);
        } else {
            prefix = low;
        }
    }
    total
}

fn add_compaction_product_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = factored_weights(&proof.compact_products.point, field)?;
    add_compaction_product_claims_prepared(
        coefficients,
        target,
        scale,
        eta,
        layout,
        proof,
        field,
        &weights,
    )
}

#[allow(clippy::too_many_arguments)]
fn add_compaction_product_claims_prepared(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
    weights: &crate::poly::mle::EqualityWeights<F>,
) -> Result<(), FalconError> {
    if layout.is_hybrid() {
        let offsets = layout.offsets();
        let claims = [
            proof.compact_products.terminal.ax,
            proof.compact_products.terminal.bx,
            proof.compact_products.terminal.cx,
        ];
        for (coordinate, claim) in claims.into_iter().enumerate() {
            for instance in coefficients.instances(layout.batch()) {
                let base = instance * layout.signature_stride();
                for i in 0..HASH_TO_POINT_SAMPLES {
                    let index = match coordinate {
                        0 => offsets.hash_quotients + 3 * i + 2,
                        1 => offsets.hash_quotients + 3 * i,
                        _ => offsets.hash_accept_ands + i,
                    };
                    coefficients.add(
                        base + index,
                        field.mul(scale, &weights.at(instance * COMPACTION_LEAVES + i)),
                    );
                }
            }
            add_claim_target(target, *scale, claim, field.zero(), field);
            *scale = field.mul(scale, &eta);
        }
        return Ok(());
    }
    let stride = COMPACTION_LEAVES * layout.capacity();
    let offsets = layout.offsets();
    let claims = [
        proof.compact_products.terminal.ax,
        proof.compact_products.terminal.bx,
        proof.compact_products.terminal.cx,
    ];
    for coordinate in 0..3 {
        let mut constant = field.zero();
        for instance in coefficients.instances(layout.batch()) {
            let base = instance * layout.signature_stride();
            for i in 0..HASH_TO_POINT_SAMPLES {
                let source_row = instance * COMPACTION_LEAVES + i;
                for block in 0..4 {
                    let weight = field.mul(scale, &weights.at(block * stride + source_row));
                    let reject = offsets.hash_accept_ands + i;
                    let selector = offsets.compact_selectors + i;
                    let (index, width, complemented) = match (block, coordinate) {
                        (0, 0) => (reject, 1, true),
                        (0, 1) => (offsets.hash_prefixes + 11 * i + 10, 1, true),
                        (0, 2) => (selector, 1, false),
                        (1 | 2, 0) => (selector, 1, false),
                        (1, 1) => (offsets.hash_prefixes + 11 * i, 11, false),
                        (1, 2) => (offsets.compact_selected_prefixes + 11 * i, 11, false),
                        (2, 1) => (offsets.hash_remainders + 14 * i, 14, false),
                        (2, 2) => (offsets.compact_selected_remainders + 14 * i, 14, false),
                        (3, 0) => (offsets.hash_quotients + 3 * i + 2, 1, false),
                        (3, 1) => (offsets.hash_quotients + 3 * i, 1, false),
                        (3, 2) => (reject, 1, false),
                        _ => unreachable!(),
                    };
                    let coefficient = if complemented {
                        if coefficients.needs_constants() {
                            constant = field.add(&constant, &weight);
                        }
                        field.sub(&field.zero(), &weight)
                    } else {
                        weight
                    };
                    add_unsigned_scaled(coefficients, base + index, width, coefficient, field);
                }
            }
        }
        add_claim_target(target, *scale, claims[coordinate], constant, field);
        *scale = field.mul(scale, &eta);
    }
    Ok(())
}

fn add_product_tree_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let offsets = layout.offsets();
    let instances = coefficients.instances(layout.batch());
    for _ in 0..(if layout.is_hybrid() { 1 } else { 2 }) * instances.start {
        *scale = field.mul(scale, &eta);
    }
    for instance in instances {
        let base = instance * layout.signature_stride();
        let pair = &proof.compaction[instance];
        for (candidate, tree) in [true, false]
            .into_iter()
            .zip([&pair.candidate, &pair.output])
        {
            if candidate && layout.is_hybrid() {
                continue;
            }
            let weights =
                eq_table(&tree.terminal_point, field).map_err(|error| piop(error.to_string()))?;
            let mut constant = field.zero();
            for (i, &leaf_weight) in weights.iter().enumerate() {
                let weight = field.mul(scale, &leaf_weight);
                if candidate {
                    constant = field.add(&constant, &weight);
                    if i < HASH_TO_POINT_SAMPLES {
                        add_coefficient(
                            coefficients,
                            base + offsets.compact_selectors + i,
                            field.mul(&weight, &field.sub(&proof.compaction_gamma, &field.one())),
                            field,
                        );
                        add_unsigned_scaled(
                            coefficients,
                            base + offsets.compact_selected_prefixes + 11 * i,
                            11,
                            field.mul(&weight, &proof.compaction_rank_scale),
                            field,
                        );
                        add_unsigned_scaled(
                            coefficients,
                            base + offsets.compact_selected_remainders + 14 * i,
                            14,
                            weight,
                            field,
                        );
                    }
                } else if i < N {
                    constant = field.add(
                        &constant,
                        &field.mul(
                            &weight,
                            &field.add(
                                &proof.compaction_gamma,
                                &field
                                    .mul(&proof.compaction_rank_scale, &unsigned(i as u128, field)),
                            ),
                        ),
                    );
                    add_unsigned_scaled(
                        coefficients,
                        base + offsets.hash_point + 14 * i,
                        14,
                        weight,
                        field,
                    );
                } else {
                    constant = field.add(&constant, &weight);
                }
            }
            add_claim_target(target, *scale, tree.terminal_claim, constant, field);
            *scale = field.mul(scale, &eta);
        }
    }
    Ok(())
}

fn add_value_scaled(
    values: &mut impl CoefficientSink,
    base: usize,
    scale: F,
    bounded: bool,
    field: &Cfg,
) {
    values.add_word(base, 14, false, scale, field);
    if bounded {
        values.add(base + 13, mul_i(scale, -4095, field));
    }
}

fn push_value_terms(terms: &mut Vec<(usize, i128)>, base: usize, scale: i128, bounded: bool) {
    for bit in 0..14 {
        let weight = if bounded && bit == 13 {
            4097
        } else {
            1i128 << bit
        };
        terms.push((base + bit, scale * weight));
    }
}

fn add_claim_target(target: &mut F, scale: F, claimed: F, constant: F, field: &Cfg) {
    *target = field.add(target, &field.sub(&field.mul(&scale, &claimed), &constant));
}

fn add_coefficient(values: &mut impl CoefficientSink, index: usize, value: F, _field: &Cfg) {
    values.add(index, value);
}

fn add_unsigned_scaled(
    values: &mut impl CoefficientSink,
    base: usize,
    width: usize,
    scale: F,
    field: &Cfg,
) {
    values.add_word(base, width, false, scale, field);
}

fn add_signed_source_scaled(
    values: &mut impl CoefficientSink,
    base: usize,
    offsets: &FalconSourceOffsets,
    coefficient: usize,
    scale: F,
    field: &Cfg,
) {
    let stream = 12 * coefficient;
    values.add_encoded_word(
        base + offsets.encoded_signature + 8 * (1 + super::NONCE_BYTES + stream / 8),
        stream % 8,
        true,
        scale,
        field,
    );
}

fn push_unsigned_terms(terms: &mut Vec<(usize, i128)>, base: usize, width: usize, scale: i128) {
    for bit in 0..width {
        terms.push((base + bit, scale * (1i128 << bit)));
    }
}

fn push_signed_terms(terms: &mut Vec<(usize, i128)>, base: usize, width: usize, scale: i128) {
    for bit in 0..width - 1 {
        terms.push((base + bit, scale * (1i128 << bit)));
    }
    terms.push((base + width - 1, -scale * (1i128 << (width - 1))));
}

fn push_round_input(
    terms: &mut Vec<(usize, i128)>,
    constant: &mut i128,
    offsets: &FalconSourceOffsets,
    permutation: usize,
    round: usize,
    x: usize,
    y: usize,
    bit: usize,
    scale: i128,
) {
    if round > 0 || permutation > 0 {
        let previous_permutation = if round == 0 {
            permutation - 1
        } else {
            permutation
        };
        let previous_round = if round == 0 {
            KECCAK_ROUNDS - 1
        } else {
            round - 1
        };
        let word = (previous_permutation * KECCAK_ROUNDS + previous_round) * 25 + x + 5 * y;
        terms.push((offsets.keccak_round_states + 64 * word + bit, scale));
        return;
    }
    let byte = (x + 5 * y) * 8 + bit / 8;
    let byte_bit = bit % 8;
    if byte < 40 {
        terms.push((offsets.encoded_signature + 8 * (1 + byte) + byte_bit, scale));
    } else if byte < 72 {
        terms.push((offsets.message + 8 * (byte - 40) + byte_bit, scale));
    } else {
        let value = if byte == 72 {
            0x1f
        } else if byte == RATE_BYTES - 1 {
            0x80
        } else {
            0
        };
        *constant += scale * i128::from((value >> byte_bit) & 1);
    }
}

fn s2_bit_index(offsets: &FalconSourceOffsets, coefficient: usize, bit: usize) -> usize {
    let stream = 12 * coefficient + (11 - bit);
    let byte = 1 + super::NONCE_BYTES + stream / 8;
    let byte_bit = 7 - stream % 8;
    offsets.encoded_signature + 8 * byte + byte_bit
}

fn shake_word_bit_index(
    layout: &FalconSourceLayout,
    offsets: &FalconSourceOffsets,
    sample: usize,
    bit: usize,
) -> usize {
    if layout.is_hybrid() {
        return offsets.hash_words + 16 * sample + bit;
    }
    let output_byte = if bit < 8 { 2 * sample + 1 } else { 2 * sample };
    let byte_bit = bit % 8;
    let permutation = output_byte / RATE_BYTES;
    let within_rate = output_byte % RATE_BYTES;
    let lane = within_rate / 8;
    let lane_bit = 8 * (within_rate % 8) + byte_bit;
    let word = (permutation * KECCAK_ROUNDS + KECCAK_ROUNDS - 1) * 25 + lane;
    offsets.keccak_round_states + 64 * word + lane_bit
}

fn keccak_gate(permutation: usize, round: usize, x: usize, y: usize, bit: usize) -> usize {
    64 * (((permutation * KECCAK_ROUNDS + round) * 25) + x + 5 * y) + bit
}

fn rho_pi_source(destination_x: usize, destination_y: usize) -> (usize, usize) {
    let source_y = destination_x;
    let source_x = (3 * (destination_y + 5 - (3 * source_y) % 5)) % 5;
    (source_x, source_y)
}

#[cfg(test)]
fn dot_packed_source(coefficients: &[F], source: &FalconSourceWitness, field: &Cfg) -> F {
    let row_vars = source.layout().row_vars();
    source
        .rows()
        .iter()
        .enumerate()
        .fold(field.zero(), |mut sum, (column, words)| {
            for (word_index, &packed) in words.iter().enumerate() {
                let mut remaining = packed;
                while remaining != 0 {
                    let bit = remaining.trailing_zeros() as usize;
                    let index = (column << row_vars) + 64 * word_index + bit;
                    sum = field.add(&sum, &coefficients[index]);
                    remaining &= remaining - 1;
                }
            }
            sum
        })
}

#[cfg(test)]
fn evaluate_mle_in_place(values: &mut Vec<F>, point: &[F], field: &Cfg) -> Result<F, FalconError> {
    if values.len() != 1usize << point.len() {
        return Err(piop("MLE evaluation shape mismatch"));
    }
    let mut active = values.len();
    for challenge in point {
        for index in 0..active / 2 {
            let left = values[2 * index];
            values[index] = field.add(
                &left,
                &field.mul(challenge, &field.sub(&values[2 * index + 1], &left)),
            );
        }
        active /= 2;
    }
    Ok(values[0])
}

fn canonical_eq_weights(point: &[F], field: &Cfg) -> Result<Vec<u128>, FalconError> {
    eq_table(point, field)
        .map_err(|error| piop(error.to_string()))
        .map(|weights| {
            weights
                .into_iter()
                .map(|value| canonical(value, field))
                .collect()
        })
}

fn bind_opening_claim(
    transcript: &mut impl Transcript,
    modulus: u128,
    point: &[F],
    value: F,
    field: &Cfg,
) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/source-opening/v1");
    transcript.absorb_slice(&modulus.to_le_bytes());
    for coordinate in point {
        transcript.absorb_slice(&coordinate.canonical_element_encoding(field));
    }
    transcript.absorb_slice(&value.canonical_element_encoding(field));
}

fn sample_point(
    transcript: &mut impl Transcript,
    rounds: usize,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    (0..rounds).map(|_| squeeze(transcript, field)).collect()
}

fn squeeze(transcript: &mut impl Transcript, field: &Cfg) -> Result<F, FalconError> {
    crate::piop::spartan::squeeze_field(transcript, field).map_err(|error| piop(error.to_string()))
}

fn field_from_modulus(modulus: u128) -> Result<Cfg, FalconError> {
    F::make_cfg(&Uint::from(modulus)).map_err(|_| piop("invalid Falcon proof modulus"))
}

fn canonical(value: F, field: &Cfg) -> u128 {
    u128::from(field.to_integer(&value))
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

fn mul_i(mut value: F, coefficient: i128, field: &Cfg) -> F {
    let magnitude = coefficient.unsigned_abs();
    if magnitude == 0 {
        return field.zero();
    }
    if magnitude.is_power_of_two() {
        for _ in 0..magnitude.trailing_zeros() {
            value = field.add(&value, &value);
        }
        if coefficient < 0 {
            field.sub(&field.zero(), &value)
        } else {
            value
        }
    } else {
        field.mul(&value, &signed(coefficient, field))
    }
}

fn linear_rounds(layout: &FalconSourceLayout) -> usize {
    layout.linear_stride().ilog2() as usize + layout.capacity().trailing_zeros() as usize
}

fn source_rounds(layout: &FalconSourceLayout) -> usize {
    layout.row_vars() + layout.col_vars()
}

fn modulus_bits(modulus: u128) -> usize {
    (u128::BITS - modulus.leading_zeros()) as usize
}

fn piop(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        piop::spartan::falcon1024_ct::{
            commit_falcon_source, falcon_ligerito_configs, verification_trace,
        },
        transcript::Blake3Transcript,
    };

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    #[test]
    fn commitment_bound_proof_roundtrips() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let statement =
            FalconPublicStatement::from_bytes(&[PUBLIC_KEY], &[MESSAGE], &[SIGNATURE]).unwrap();
        let source =
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[trace.clone()])
                .unwrap();
        let (pc, vc) = falcon_ligerito_configs(&layout, 100).unwrap();
        let hint = commit_falcon_source(&source, &pc);
        let mut prover = Blake3Transcript::new();
        let mut proof = prove_falcon_bitz(
            &mut prover,
            &layout,
            &statement,
            &[trace],
            &source,
            &hint,
            100,
            &pc,
        )
        .unwrap();
        let mut verifier = Blake3Transcript::new();
        verify_falcon_bitz(
            &mut verifier,
            &layout,
            &statement,
            &hint.commitment,
            &proof,
            100,
            &vc,
        )
        .unwrap();

        let mut wrong_statement = statement.clone();
        wrong_statement.messages[0][0] ^= 1;
        let mut rejecting_verifier = Blake3Transcript::new();
        assert!(
            verify_falcon_bitz(
                &mut rejecting_verifier,
                &layout,
                &wrong_statement,
                &hint.commitment,
                &proof,
                100,
                &vc,
            )
            .is_err()
        );

        let field = field_from_modulus(proof.piop.modulus).unwrap();
        for coordinate in 0..2 {
            let original = proof.binding_terminal[coordinate];
            proof.binding_terminal[coordinate] = field.add(&original, &field.one());
            assert!(
                verify_falcon_bitz(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &statement,
                    &hint.commitment,
                    &proof,
                    100,
                    &vc,
                )
                .is_err()
            );
            proof.binding_terminal[coordinate] = original;
        }
        let mut wrong_key = statement.clone();
        wrong_key.public_keys[0].h[0] = (wrong_key.public_keys[0].h[0] + 1) % Q as u16;
        assert!(
            verify_falcon_bitz(
                &mut Blake3Transcript::new(),
                &layout,
                &wrong_key,
                &hint.commitment,
                &proof,
                100,
                &vc,
            )
            .is_err()
        );
    }

    #[test]
    fn corrupted_committed_column_parity_is_rejected() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let statement =
            FalconPublicStatement::from_bytes(&[PUBLIC_KEY], &[MESSAGE], &[SIGNATURE]).unwrap();
        let mut corrupted = trace.clone();
        corrupted.hash_to_point.shake.column_parities[7] ^= 1;
        let source =
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[corrupted])
                .unwrap();
        let (pc, vc) = falcon_ligerito_configs(&layout, 100).unwrap();
        let hint = commit_falcon_source(&source, &pc);
        if let Ok(proof) = prove_falcon_bitz(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            &[trace],
            &source,
            &hint,
            100,
            &pc,
        ) {
            assert!(
                verify_falcon_bitz(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &statement,
                    &hint.commitment,
                    &proof,
                    100,
                    &vc,
                )
                .is_err()
            );
        }
    }
}

#[cfg(test)]
#[path = "opening_binding_tests.rs"]
mod binding_tests;
