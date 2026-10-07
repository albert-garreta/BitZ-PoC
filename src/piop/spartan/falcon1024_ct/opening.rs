use super::{COEFFICIENT_LOG, COMPACTION_LOG, NORM_BITS, PREFIX_BIAS, PREFIX_BITS, SIGNATURE_BITS};
// Commitment-bound terminal reduction for the Falcon-1024 PIOP.
//
// All exact linear Falcon relations and all terminal values emitted by the
// nonlinear sumchecks are folded into one streamed linear form in the committed
// binary source.  One degree-two inner sumcheck reduces that form to one MLE
// evaluation authenticated by the shared hybrid opening.

use std::{collections::HashMap, sync::OnceLock};

use field::{BatchMulAcc, MergeAccumulator, RingOps, Uint};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
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
                ColumnMajorPackedBits, FactoredOverlayInput, PackedInput,
                StreamingCoefficientSource, StreamingMle,
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
    piop::{FalconPiopClaimRef, prove_falcon_piop, security_schedule, verify_falcon_piop},
};

#[path = "opening_compact.rs"]
mod compact;
#[path = "opening_joined.rs"]
mod joined;
#[path = "opening_native.rs"]
mod native;
use joined::JoinedBinding;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

const COMPACTION_LEAVES: usize = 1 << COMPACTION_LOG;

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
    pub(super) piop: PiopProof,
    pub(super) ring: RingProof,
    pub linear_point_nonce: Option<u64>,
    pub binding: SumcheckProof<F, 3>,
    /// Legacy challenge cache. Empty in shared-prime proofs, which derive the point.
    pub binding_point: Vec<F>,
    /// `[coefficient MLE, source MLE]` at the binding sumcheck's terminal point.
    pub binding_terminal: [F; 2],
    pub binding_nonces: Vec<u64>,
}

#[derive(Clone, Debug)]
pub(super) enum RingProof {
    Native(super::native_ring::NativeRingProof),
    Shared(super::shared_ring::Proof),
}

#[derive(Clone, Debug)]
pub(super) enum PiopProof {
    Native(FalconPiopProof),
    Shared(super::piop::SharedFalconPiopProof),
}

/// Checked arithmetic context passed directly to the binary bridge.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BridgeClaim {
    pub point: Vec<F>,
    pub modulus: u128,
}

/// The caller binds the commitments and statement before this call, proves SHAKE
/// and its source links, and authenticates the returned source claim afterward.
pub(super) fn prove_binding_prefix(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    target_bits: usize,
) -> Result<(FalconBindingPrefixProof, Vec<u128>, BridgeClaim), FalconError> {
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
    let (algebraic, field, ring, native_claim, projected) = if layout.is_shared_prime() {
        let (ring, field, projected) =
            super::shared_ring::prove(transcript, layout, statement, traces, source, target_bits)?;
        let algebraic = super::piop::prove_falcon_piop_in_field(
            transcript,
            layout,
            traces,
            target_bits,
            &field,
        )?;
        (
            algebraic,
            field,
            RingProof::Shared(ring),
            None,
            Some(projected),
        )
    } else {
        let algebraic = prove_falcon_piop(transcript, layout, traces, target_bits)?;
        let field = field_from_modulus(algebraic.modulus)?;
        let (ring, claim) = super::native_ring::prove(
            transcript,
            layout,
            statement,
            traces,
            &field,
            target_bits as u32,
        )?;
        (algebraic, field, RingProof::Native(ring), Some(claim), None)
    };

    let linear_point_nonce = grind_linear_point(transcript, layout, target_bits, None)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let integer = prepare_binding_form_optional(
        transcript,
        layout,
        statement,
        algebraic.as_claim_ref(),
        &linear_point,
        &field,
        native_claim,
    )?;
    let merge = if projected.is_some() {
        transcript.absorb_slice(b"bitz/falcon1024-ct/shared-prime/merge/v1");
        squeeze(transcript, &field)?
    } else {
        field.one()
    };
    let binding = JoinedBinding::new(integer, projected, merge)?;
    let target_span = tracing::info_span!("falcon_arithmetic:binding_target").entered();
    let target = binding.target()?;
    drop(target_span);

    let _binding_span = tracing::info_span!("falcon_arithmetic:binding_inner").entered();
    // Finish the shared cache before coefficient partitions start. In
    // particular, do not make Rayon workers wait on a parallel initializer.
    {
        let _template_span = tracing::info_span!("falcon_arithmetic:binding_template").entered();
        binding.integer.prepared_compact_template()?;
    }
    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let packed_source = ColumnMajorPackedBits::new(source.rows(), layout.row_vars());
    let coefficients = StreamingMle::new(&binding);
    // Three packed rounds reduce ternary-prefix work for the compact source.
    let prefix_vars = 3;
    let input = || {
        PackedInput::new(
            &coefficients,
            &packed_source,
            source_rounds(layout),
            layout.source_bits(),
            prefix_vars,
        )
    };
    let output = if let Some(ring) = &binding.ring {
        prove_binding_inner(
            transcript,
            &field,
            layout,
            target_bits,
            target,
            FactoredOverlayInput::new(
                &binding.integer,
                &packed_source,
                &ring.row,
                &ring.column,
                binding.integer_scale,
                source_rounds(layout),
                layout.source_bits(),
                prefix_vars,
            ),
        )?
    } else {
        prove_binding_inner(transcript, &field, layout, target_bits, target, input())?
    };
    let (output, binding_nonces) = output;
    // The source templates and folded public tables are no longer needed.
    // Release them before compacting the proof and entering the BitZ bridge.
    drop(binding);
    let binding_terminal = output.terminal_evaluations;
    absorb_field_elements(transcript, &binding_terminal, &field);
    bind_opening_claim(
        transcript,
        algebraic.modulus,
        &output.point,
        binding_terminal[1],
        &field,
    );
    let row_weights = canonical_eq_weights(&output.point[..layout.row_vars()], &field)?;
    let bridge_claim = BridgeClaim {
        point: output.point,
        modulus: algebraic.modulus,
    };
    let prefix = FalconBindingPrefixProof {
        piop: if layout.is_shared_prime() {
            PiopProof::Shared(algebraic.into_shared())
        } else {
            PiopProof::Native(algebraic)
        },
        ring,
        linear_point_nonce,
        binding: output.proof,
        binding_point: if layout.is_shared_prime() {
            Vec::new()
        } else {
            bridge_claim.point.clone()
        },
        binding_terminal,
        binding_nonces,
    };
    Ok((prefix, row_weights, bridge_claim))
}

fn prove_binding_inner<V>(
    transcript: &mut impl Transcript,
    field: &Cfg,
    layout: &FalconSourceLayout,
    target_bits: usize,
    target: F,
    input: V,
) -> Result<(crate::sumcheck::inner::InnerSumcheckOutput<F>, Vec<u64>), FalconError>
where
    V: crate::sumcheck::inner::input::Input<Cfg, Weights = ()>,
{
    if target_bits == 128 {
        let mut boundary = ProverGrindingRoundBoundary::<BindingGrinding>::with_round_offset(
            security_schedule(layout, target_bits)?.binding_round_bits,
            0,
        );
        let output = prove_inner_sumcheck(field, transcript, target, input, (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        Ok((output, boundary.into_nonces()))
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let output = prove_inner_sumcheck(field, transcript, target, input, (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        Ok((output, Vec::new()))
    }
}

pub(super) fn verify_binding_prefix(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    proof: &FalconBindingPrefixProof,
    target_bits: usize,
) -> Result<BridgeClaim, FalconError> {
    statement.validate(layout.batch())?;
    if statement.batch() != layout.batch()
        || statement.messages.len() != layout.batch()
        || !matches!(target_bits, 100 | 128)
        || (target_bits == 128 && proof.linear_point_nonce.is_none())
        || (target_bits == 100 && !proof.binding_nonces.is_empty())
    {
        return Err(piop("Falcon prefix proof shape mismatch"));
    }
    let shared_algebraic;
    let (field, native_claim, projected, algebraic) =
        match (&proof.ring, &proof.piop, layout.is_shared_prime()) {
            (RingProof::Shared(ring), PiopProof::Shared(integer), true) => {
                let (field, projected) =
                    super::shared_ring::verify(transcript, layout, statement, ring, target_bits)?;
                shared_algebraic = super::piop::verify_shared_falcon_piop_in_field(
                    transcript,
                    layout,
                    integer,
                    target_bits,
                    &field,
                )?;
                (
                    field,
                    None,
                    Some(projected),
                    shared_algebraic.as_claim_ref(),
                )
            }
            (RingProof::Native(ring), PiopProof::Native(integer), false) => {
                verify_falcon_piop(transcript, layout, integer, target_bits)?;
                let field = field_from_modulus(integer.modulus)?;
                let claim = super::native_ring::verify(
                    transcript,
                    layout,
                    statement,
                    ring,
                    &field,
                    target_bits as u32,
                )?;
                (field, Some(claim), None, integer.as_claim_ref())
            }
            _ => return Err(piop("Falcon ring protocol mismatch")),
        };

    grind_linear_point(transcript, layout, target_bits, proof.linear_point_nonce)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let integer = prepare_binding_form_optional(
        transcript,
        layout,
        statement,
        algebraic,
        &linear_point,
        &field,
        native_claim,
    )?;
    let merge = if projected.is_some() {
        transcript.absorb_slice(b"bitz/falcon1024-ct/shared-prime/merge/v1");
        squeeze(transcript, &field)?
    } else {
        field.one()
    };
    let target = integer.target()?;
    let target = match &projected {
        Some(ring) => field.add(&ring.target, &field.mul(&merge, &target)),
        None => target,
    };
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
    let valid_point = if layout.is_shared_prime() {
        proof.binding_point.is_empty()
    } else {
        point == proof.binding_point
    };
    if !valid_point
        || final_claims[0] != field.mul(&proof.binding_terminal[0], &proof.binding_terminal[1])
    {
        return Err(piop("shared inner terminal mismatch"));
    }
    let coefficient = if let Some(ring) = &projected {
        let split = layout.signature_stride().ilog2() as usize;
        let local = eq_table(&point[..split], &field).map_err(|e| piop(e.to_string()))?;
        let instances = eq_table(&point[split..], &field).map_err(|e| piop(e.to_string()))?;
        field.add(
            &ring.evaluate(layout, &local, &instances, &field)?,
            &field.mul(
                &merge,
                &integer.evaluate_compact_weights(&local, &instances)?,
            ),
        )
    } else {
        integer.evaluate(&point)?
    };
    if coefficient != proof.binding_terminal[0] {
        return Err(piop("shared coefficient MLE mismatch"));
    }
    absorb_field_elements(transcript, &proof.binding_terminal, &field);
    bind_opening_claim(
        transcript,
        field.modulus_u128(),
        &point,
        proof.binding_terminal[1],
        &field,
    );
    Ok(BridgeClaim {
        point,
        modulus: field.modulus_u128(),
    })
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

struct BindingForm<'a> {
    layout: &'a FalconSourceLayout,
    statement: &'a FalconPublicStatement,
    proof: FalconPiopClaimRef<'a>,
    field: &'a Cfg,
    #[cfg(test)]
    linear_weights: crate::poly::mle::EqualityWeights<F>,
    local_linear_weights: crate::poly::mle::EqualityWeights<F>,
    linear_instance_weights: Vec<F>,
    compact_linear: OnceLock<compact::CompiledTemplate>,
    compact_instances: Vec<OnceLock<compact::CompiledCoefficients>>,
    compact_weights: OnceLock<compact::PreparedWeights>,
    local_linear_point: Vec<F>,
    eta: F,
    native_claim: Option<super::native_ring::PreparedNativeClaim>,
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
    fn add_word(&mut self, base: usize, width: usize, mut scale: F, field: &Cfg) {
        if !self.enabled() {
            return;
        }
        for bit in 0..width {
            self.add(base + bit, scale);
            scale = field.add(&scale, &scale);
        }
    }
    fn add_encoded_word(&mut self, base: usize, offset: usize, mut scale: F, field: &Cfg) {
        if !self.enabled() {
            return;
        }
        for bit in 0..SIGNATURE_BITS {
            let stream = offset + SIGNATURE_BITS - 1 - bit;
            let index = base + 8 * (stream / 8) + 7 - stream % 8;
            self.add(
                index,
                if bit == SIGNATURE_BITS - 1 {
                    field.sub(&field.zero(), &scale)
                } else {
                    scale
                },
            );
            scale = field.add(&scale, &scale);
        }
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

#[cfg(test)]
fn prepare_binding_form<'a>(
    transcript: &mut impl Transcript,
    layout: &'a FalconSourceLayout,
    statement: &'a FalconPublicStatement,
    proof: &'a FalconPiopProof,
    linear_point: &[F],
    field: &'a Cfg,
    native_claim: super::native_ring::PreparedNativeClaim,
) -> Result<BindingForm<'a>, FalconError> {
    prepare_binding_form_optional(
        transcript,
        layout,
        statement,
        proof.as_claim_ref(),
        linear_point,
        field,
        Some(native_claim),
    )
}

fn prepare_binding_form_optional<'a>(
    transcript: &mut impl Transcript,
    layout: &'a FalconSourceLayout,
    statement: &'a FalconPublicStatement,
    proof: FalconPiopClaimRef<'a>,
    linear_point: &[F],
    field: &'a Cfg,
    native_claim: Option<super::native_ring::PreparedNativeClaim>,
) -> Result<BindingForm<'a>, FalconError> {
    #[cfg(test)]
    let linear_weights = factored_weights(linear_point, field)?;
    // The preceding linear-point nonce protects one atomic challenge block:
    // its row coordinates followed by eta and, for the shared-prime path,
    // the independent merge scalar. No prover message intervenes. Dropping
    // the native carry endpoint saves one eta power, paying for that scalar
    // within the existing conservative degree bound (13 + m + batch + 12).
    transcript.absorb_slice(b"bitz/falcon1024-ct/terminal-collapse/v1");
    let eta = squeeze(transcript, field)?;
    let local_linear_vars = layout.linear_stride().ilog2() as usize;
    let local_linear_weights = factored_weights(&linear_point[..local_linear_vars], field)?;
    let mut linear_instance_weights = eq_table(&linear_point[local_linear_vars..], field)
        .map_err(|error| piop(error.to_string()))?;
    linear_instance_weights.truncate(layout.batch());
    Ok(BindingForm {
        layout,
        statement,
        proof,
        field,
        #[cfg(test)]
        linear_weights,
        local_linear_weights,
        linear_instance_weights,
        compact_linear: OnceLock::new(),
        compact_instances: (0..layout.batch()).map(|_| OnceLock::new()).collect(),
        compact_weights: OnceLock::new(),
        local_linear_point: linear_point[..local_linear_vars].to_vec(),
        eta,
        native_claim,
    })
}

impl BindingForm<'_> {
    fn target(&self) -> Result<F, FalconError> {
        self.compact_target()
    }

    fn evaluate(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != source_rounds(self.layout) {
            return Err(piop("binding endpoint dimension mismatch"));
        }
        self.evaluate_compact(point)
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

    fn for_each_partition_byte_bucket(
        &self,
        partition: usize,
        read_byte: &mut impl FnMut(usize, usize) -> Result<u8, crate::sumcheck::SumcheckError>,
        emit: &mut impl FnMut(u8, &[F; 8]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Option<Result<(), crate::sumcheck::SumcheckError>> {
        Some((|| {
            if partition >= self.layout.capacity() {
                return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
            }
            if partition >= self.layout.batch() {
                return Ok(());
            }
            self.compact_instance(partition)
                .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?
                .emit_byte_buckets(
                    partition * self.layout.signature_stride(),
                    self.field,
                    read_byte,
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
        for instance in instances {
            self.compact_instance(instance)
                .map_err(|_| crate::sumcheck::SumcheckError::InvalidProductDimensions)?
                .emit(instance * self.layout.signature_stride(), self.field, emit)?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn add_norm_claims_prepared(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: FalconPiopClaimRef<'_>,
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
                    add_value_scaled(coefficients, base + offsets.s1 + 14 * i, weight, field);
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
                    add_value_scaled(coefficients, base + offsets.s1 + 14 * i, weight, field);
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
            NORM_BITS,
            field.mul(scale, &instance_weights[instance]),
            field,
        );
    }
    add_claim_target(target, *scale, proof.norm.slack, constant, field);
    *scale = field.mul(scale, &eta);
    Ok(())
}

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

#[allow(clippy::too_many_arguments)]
fn add_compaction_product_claims_prepared(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: FalconPiopClaimRef<'_>,
    field: &Cfg,
    weights: &crate::poly::mle::EqualityWeights<F>,
) -> Result<(), FalconError> {
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
    Ok(())
}

fn add_value_scaled(values: &mut impl CoefficientSink, base: usize, scale: F, field: &Cfg) {
    values.add_word(base, 14, scale, field);
    values.add(base + 13, mul_i(scale, -4095, field));
}

fn add_claim_target(target: &mut F, scale: F, claimed: F, constant: F, field: &Cfg) {
    *target = field.add(target, &field.sub(&field.mul(&scale, &claimed), &constant));
}

fn add_unsigned_scaled(
    values: &mut impl CoefficientSink,
    base: usize,
    width: usize,
    scale: F,
    field: &Cfg,
) {
    values.add_word(base, width, scale, field);
}

fn add_signed_source_scaled(
    values: &mut impl CoefficientSink,
    base: usize,
    offsets: &FalconSourceOffsets,
    coefficient: usize,
    scale: F,
    field: &Cfg,
) {
    let stream = SIGNATURE_BITS * coefficient;
    values.add_encoded_word(
        base + offsets.encoded_signature + 8 * (1 + super::NONCE_BYTES + stream / 8),
        stream % 8,
        scale,
        field,
    );
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

fn canonical_eq_weights(point: &[F], field: &Cfg) -> Result<Vec<u128>, FalconError> {
    eq_table(point, field)
        .map_err(|error| piop(error.to_string()))
        .map(|weights| {
            weights
                .into_iter()
                .map(|value| u128::from(field.to_integer(&value)))
                .collect()
        })
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

fn piop(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}
falcon_tests! {
#[path = "opening_binding_tests.rs"]
mod binding_tests;

}
