//! Exact integer norms and authentication against the one committed bit source.
//!
//! The native ring reduction and both norm operands are joined only after all
//! their claimed values are fixed. The resulting linear form is streamed into
//! a degree-two sumcheck; its final bit evaluation must be opened by BitZ.

use field::RingOps;

use crate::{
    piop::spartan::{
        SpartanField, absorb_field_elements,
        falcon_bit_layout::{COEFFICIENT_STRIDE, coefficient_bit},
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::eq_table,
        squeeze_field,
    },
    sumcheck::{
        SumcheckError, SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{
            packed::{
                ColumnMajorPackedBits, PackedInput, StreamingCoefficientSource, StreamingMle,
            },
            prove_batched_inner_sumcheck, prove_inner_sumcheck,
        },
        proof::validate_field_elements,
    },
    transcript::traits::Transcript,
};

use super::ring::PreparedClaim;
use super::{BETA_SQUARED, Cfg, F, FalconError, Layout, N, Schedule, Source, WitnessData};

const COEFFICIENT_BITS: usize = 15;
const COEFFICIENT_LOG: usize = super::COEFFICIENT_LOG;
const SLACK_BITS: usize = super::SLACK_BITS;
const NORM_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/norm/v2";
const MERGE_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/merge/v2";
const BINDING_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/binding/v2";

struct NormInstance;
impl GrindingDomain for NormInstance {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/norm-instance/v2";
}
struct NormRound;
impl GrindingDomain for NormRound {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/norm-round/v2";
}
struct Merge;
impl GrindingDomain for Merge {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/merge/v2";
}
struct BindingRound;
impl GrindingDomain for BindingRound {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/binding-round/v2";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NormProof {
    pub instance_nonce: Option<u64>,
    pub claims: [F; 2],
    pub sumchecks: [SumcheckProof<F, 3>; 2],
    /// Each pair is [weighted operand MLE, unweighted operand MLE].
    pub terminal: [[F; 2]; 2],
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Proof {
    pub norm: NormProof,
    pub merge_nonce: Option<u64>,
    pub binding: SumcheckProof<F, 3>,
    pub binding_terminal: [F; 2],
    pub binding_nonces: Vec<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        // Canonical field words, round coefficients, terminal messages and
        // present nonces. Container framing is excluded from this payload size.
        let norm_rounds: usize = self
            .norm
            .sumchecks
            .iter()
            .map(|p| p.round_polynomials.len())
            .sum();
        16 * (2 + 4 + 3 * norm_rounds + 3 * self.binding.round_polynomials.len() + 2)
            + 8 * (usize::from(self.norm.instance_nonce.is_some())
                + self.norm.grinding_nonces.len()
                + usize::from(self.merge_nonce.is_some())
                + self.binding_nonces.len())
    }
}

/// This is a pending PCS obligation, not a standalone proof of the relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BridgeClaim {
    pub point: Vec<F>,
    pub modulus: u128,
    pub value: F,
}

struct NormClaims {
    instance_point: Vec<F>,
    point: Vec<F>,
    terminal: [[F; 2]; 2],
    slack: F,
}

pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &Layout,
    data: &WitnessData,
    source: &Source,
    field: &Cfg,
    ring: &PreparedClaim,
    schedule: &Schedule,
) -> Result<(Proof, Vec<u128>, BridgeClaim), FalconError> {
    if source.layout() != layout {
        return Err(failure("algebraic source layout mismatch"));
    }
    validate_field(layout, field)?;
    let (norm, norm_claims) = prove_norm(transcript, layout, data, field, schedule)?;
    let _binding_span = tracing::info_span!("falcon_algebraic:binding").entered();
    transcript.absorb_slice(MERGE_DOMAIN);
    let merge_nonce = prove_nonce::<Merge>(transcript, schedule.merge_bits)?;
    let eta = sample(transcript, field)?;
    let (form, target) = BindingForm::new(layout, ring, &norm_claims, eta, field)?;

    transcript.absorb_slice(BINDING_DOMAIN);
    let stream = StreamingMle::new(&form);
    let bits = ColumnMajorPackedBits::new(source.rows(), layout.bitz_params().row_vars);
    let mut boundary =
        ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(schedule.binding_bits, 0);
    let output = prove_inner_sumcheck(
        field,
        transcript,
        target,
        PackedInput::new(
            &stream,
            &bits,
            source_rounds(layout),
            layout.source_bits(),
            3,
        ),
        (),
        &mut boundary,
    )
    .map_err(failure)?;
    let binding_nonces = boundary.into_nonces();
    let binding_terminal = output.terminal_evaluations;
    // A dishonest host witness may disagree with the source. Catch that here,
    // while the verifier separately checks the same coefficient evaluation.
    if output.final_claim != field.mul(&binding_terminal[0], &binding_terminal[1])
        || form.evaluate(&output.point)? != binding_terminal[0]
    {
        return Err(failure("algebraic binding terminal mismatch"));
    }
    absorb_field_elements(transcript, &binding_terminal, field);
    let claim = bind_opening(transcript, output.point, binding_terminal[1], field);
    let row_weights = eq_table(&claim.point[..layout.bitz_params().row_vars], field)
        .map_err(failure)?
        .into_iter()
        .map(|value| u128::from(field.to_integer(&value)))
        .collect();
    Ok((
        Proof {
            norm,
            merge_nonce,
            binding: output.proof,
            binding_terminal,
            binding_nonces,
        },
        row_weights,
        claim,
    ))
}

pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &Layout,
    field: &Cfg,
    ring: &PreparedClaim,
    proof: &Proof,
    schedule: &Schedule,
) -> Result<BridgeClaim, FalconError> {
    validate_field(layout, field)?;
    let norm = verify_norm(transcript, layout, &proof.norm, field, schedule)?;
    transcript.absorb_slice(MERGE_DOMAIN);
    verify_nonce::<Merge>(transcript, schedule.merge_bits, proof.merge_nonce)?;
    let eta = sample(transcript, field)?;
    let (form, target) = BindingForm::new(layout, ring, &norm, eta, field)?;
    transcript.absorb_slice(BINDING_DOMAIN);
    let mut boundary = VerifierGrindingRoundBoundary::<BindingRound>::new(
        schedule.binding_bits,
        &proof.binding_nonces,
    );
    let (point, final_claim) = proof
        .binding
        .verify_with_round_boundary(
            transcript,
            target,
            source_rounds(layout),
            field,
            &mut boundary,
        )
        .map_err(failure)?;
    validate_field_elements(&proof.binding_terminal, field).map_err(failure)?;
    if final_claim != field.mul(&proof.binding_terminal[0], &proof.binding_terminal[1])
        || form.evaluate(&point)? != proof.binding_terminal[0]
    {
        return Err(failure("algebraic binding terminal mismatch"));
    }
    absorb_field_elements(transcript, &proof.binding_terminal, field);
    Ok(bind_opening(
        transcript,
        point,
        proof.binding_terminal[1],
        field,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_algebraic:norm")]
fn prove_norm(
    transcript: &mut impl Transcript,
    layout: &Layout,
    data: &WitnessData,
    field: &Cfg,
    schedule: &Schedule,
) -> Result<(NormProof, NormClaims), FalconError> {
    if data.witness.s1.len() != layout.batch()
        || data.witness.s2.len() != layout.batch()
        || data.slacks.len() != layout.batch()
    {
        return Err(failure("algebraic norm witness shape mismatch"));
    }
    transcript.absorb_slice(NORM_DOMAIN);
    let instance_nonce = prove_nonce::<NormInstance>(transcript, schedule.norm_instance_bits)?;
    let instance_point = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let instance_weights = eq_table(&instance_point, field).map_err(failure)?;
    let len = N * layout.capacity();
    let mut values = [vec![field.zero(); len], vec![field.zero(); len]];
    let mut weighted = [vec![field.zero(); len], vec![field.zero(); len]];
    let mut claims = [field.zero(); 2];
    let mut slack = field.zero();
    for (i, weight) in instance_weights.iter().take(layout.batch()).enumerate() {
        for (side, coefficients) in [&data.witness.s1[i], &data.witness.s2[i]]
            .into_iter()
            .enumerate()
        {
            let mut norm = 0u64;
            for (j, &coefficient) in coefficients.iter().enumerate() {
                if !(-16384..16384).contains(&coefficient) {
                    return Err(failure("coefficient does not fit signed 15-bit encoding"));
                }
                let integer = i64::from(coefficient);
                norm += (integer * integer) as u64;
                let value = signed(integer as i128, field);
                values[side][i * N + j] = value;
                weighted[side][i * N + j] = field.mul(weight, &value);
            }
            claims[side] = field.add(
                &claims[side],
                &field.mul(weight, &unsigned(norm as u128, field)),
            );
        }
        if data.slacks[i] >= 1u64 << SLACK_BITS {
            return Err(failure("slack does not fit configured unsigned encoding"));
        }
        slack = field.add(
            &slack,
            &field.mul(weight, &unsigned(data.slacks[i] as u128, field)),
        );
    }
    let expected = norm_target(layout, &instance_weights, field);
    if field.add(&field.add(&claims[0], &claims[1]), &slack) != expected {
        return Err(failure("invalid algebraic integer norm witness"));
    }
    absorb_field_elements(transcript, &[claims[0], claims[1], slack], field);
    let mut boundary =
        ProverGrindingRoundBoundary::<NormRound>::with_round_offset(schedule.norm_round_bits, 0);
    let output =
        prove_batched_inner_sumcheck(field, transcript, &claims, values, weighted, &mut boundary)
            .map_err(failure)?;
    absorb_field_elements(transcript, &output.terminal_evaluations.concat(), field);
    let terminal = output.terminal_evaluations;
    Ok((
        NormProof {
            instance_nonce,
            claims,
            sumchecks: output.proofs,
            terminal,
            grinding_nonces: boundary.into_nonces(),
        },
        NormClaims {
            instance_point,
            point: output.point,
            terminal,
            slack,
        },
    ))
}

fn verify_norm(
    transcript: &mut impl Transcript,
    layout: &Layout,
    proof: &NormProof,
    field: &Cfg,
    schedule: &Schedule,
) -> Result<NormClaims, FalconError> {
    transcript.absorb_slice(NORM_DOMAIN);
    verify_nonce::<NormInstance>(
        transcript,
        schedule.norm_instance_bits,
        proof.instance_nonce,
    )?;
    let instance_point = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let weights = eq_table(&instance_point, field).map_err(failure)?;
    validate_field_elements(&proof.claims, field).map_err(failure)?;
    validate_field_elements(&proof.terminal.concat(), field).map_err(failure)?;
    let slack = field.sub(
        &norm_target(layout, &weights, field),
        &field.add(&proof.claims[0], &proof.claims[1]),
    );
    absorb_field_elements(
        transcript,
        &[proof.claims[0], proof.claims[1], slack],
        field,
    );
    let mut boundary = VerifierGrindingRoundBoundary::<NormRound>::new(
        schedule.norm_round_bits,
        &proof.grinding_nonces,
    );
    let (point, claims) = SumcheckProof::verify_batch_with_round_boundary(
        proof.sumchecks.each_ref(),
        transcript,
        &proof.claims,
        COEFFICIENT_LOG + layout.capacity().ilog2() as usize,
        field,
        &mut boundary,
    )
    .map_err(failure)?;
    for (claim, terminal) in claims.iter().zip(&proof.terminal) {
        if *claim != field.mul(&terminal[0], &terminal[1]) {
            return Err(failure("algebraic norm terminal mismatch"));
        }
    }
    absorb_field_elements(transcript, &proof.terminal.concat(), field);
    Ok(NormClaims {
        instance_point,
        point,
        terminal: proof.terminal,
        slack,
    })
}

fn norm_target(layout: &Layout, weights: &[F], field: &Cfg) -> F {
    let sum = weights[..layout.batch()]
        .iter()
        .fold(field.zero(), |sum, weight| field.add(&sum, weight));
    field.mul(&sum, &unsigned(BETA_SQUARED as u128, field))
}

/// At most two field elements per signature coefficient, not per source bit.
/// The decoder transpose and padding mask are expanded only while streaming.
struct BindingForm<'a> {
    layout: &'a Layout,
    field: &'a Cfg,
    coefficients: [Vec<F>; 2],
    slacks: Vec<F>,
    padding: F,
}

impl<'a> BindingForm<'a> {
    fn new(
        layout: &'a Layout,
        ring: &PreparedClaim,
        norm: &NormClaims,
        eta: F,
        field: &'a Cfg,
    ) -> Result<(Self, F), FalconError> {
        let len = layout.batch() * N;
        if ring.weights_s1.len() != len
            || ring.weights_s2.len() != len
            || norm.point.len() != COEFFICIENT_LOG + layout.capacity().ilog2() as usize
            || norm.instance_point.len() != layout.capacity().ilog2() as usize
        {
            return Err(failure("algebraic binder claim dimension mismatch"));
        }
        validate_field_elements(&ring.weights_s1, field).map_err(failure)?;
        validate_field_elements(&ring.weights_s2, field).map_err(failure)?;
        validate_field_elements(&[ring.target], field).map_err(failure)?;
        let mut scales = [field.one(); 7];
        for i in 1..scales.len() {
            scales[i] = field.mul(&scales[i - 1], &eta);
        }
        let weights = eq_table(&norm.point, field).map_err(failure)?;
        let instance_weights = eq_table(&norm.instance_point, field).map_err(failure)?;
        let mut coefficients = [ring.weights_s1.clone(), ring.weights_s2.clone()];
        let mut target = ring.target;
        for side in 0..2 {
            let weighted_scale = scales[1 + 2 * side];
            let plain_scale = scales[2 + 2 * side];
            target = field.add(
                &target,
                &field.mul(&weighted_scale, &norm.terminal[side][0]),
            );
            target = field.add(&target, &field.mul(&plain_scale, &norm.terminal[side][1]));
            for (instance, weight) in instance_weights.iter().take(layout.batch()).enumerate() {
                let scale = field.add(&field.mul(&weighted_scale, weight), &plain_scale);
                for j in 0..N {
                    let index = instance * N + j;
                    coefficients[side][index] = field.add(
                        &coefficients[side][index],
                        &field.mul(&scale, &weights[index]),
                    );
                }
            }
        }
        target = field.add(&target, &field.mul(&scales[5], &norm.slack));
        let slacks = instance_weights[..layout.batch()]
            .iter()
            .map(|weight| field.mul(&scales[5], weight))
            .collect();
        Ok((
            Self {
                layout,
                field,
                coefficients,
                slacks,
                padding: scales[6],
            },
            target,
        ))
    }

    /// Split bit-lane, coefficient and instance variables. The signed decoder
    /// is evaluated once, rather than once per coefficient or per signature.
    fn evaluate(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != source_rounds(self.layout) {
            return Err(failure("algebraic source opening dimension mismatch"));
        }
        let local_vars = self.layout.signature_stride().ilog2() as usize;
        let lanes = eq_table(&point[..4], self.field).map_err(failure)?;
        let coefficients = eq_table(&point[4..local_vars], self.field).map_err(failure)?;
        let instances = eq_table(&point[local_vars..], self.field).map_err(failure)?;
        let decoder = signed_decoder(&lanes[..COEFFICIENT_BITS], self.field);
        let mut slack_decoder = self.field.zero();
        let mut slack_positions = self.field.zero();
        let mut power = self.field.one();
        for weight in &coefficients[..SLACK_BITS] {
            slack_decoder = self
                .field
                .add(&slack_decoder, &self.field.mul(weight, &power));
            slack_positions = self.field.add(&slack_positions, weight);
            power = self.field.add(&power, &power);
        }
        let slack_decoder = self.field.mul(&slack_decoder, &lanes[15]);
        let mut coefficient_value = self.field.zero();
        let mut slack_value = self.field.zero();
        for (instance, weight) in instances.iter().take(self.layout.batch()).enumerate() {
            let mut value = self.field.zero();
            for side in 0..2 {
                for (j, coefficient_weight) in
                    coefficients[side * N..(side + 1) * N].iter().enumerate()
                {
                    value = self.field.add(
                        &value,
                        &self.field.mul(
                            &self.coefficients[side][instance * N + j],
                            coefficient_weight,
                        ),
                    );
                }
            }
            coefficient_value = self
                .field
                .add(&coefficient_value, &self.field.mul(weight, &value));
            slack_value = self.field.add(
                &slack_value,
                &self.field.mul(weight, &self.slacks[instance]),
            );
        }
        let result = self.field.add(
            &self.field.mul(&decoder, &coefficient_value),
            &self.field.mul(&slack_decoder, &slack_value),
        );
        // Every lane except lane 15 is live. Lane 15 is live only for the
        // first SLACK_BITS coefficients; the rest and all inactive signatures are zero.
        let live_local = self.field.add(
            &self.field.sub(&self.field.one(), &lanes[15]),
            &self.field.mul(&lanes[15], &slack_positions),
        );
        let live_instances = instances[..self.layout.batch()]
            .iter()
            .fold(self.field.zero(), |sum, weight| {
                self.field.add(&sum, weight)
            });
        let padding_weight = self.field.sub(
            &self.field.one(),
            &self.field.mul(&live_local, &live_instances),
        );
        Ok(self
            .field
            .add(&result, &self.field.mul(&self.padding, &padding_weight)))
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
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        for instance in 0..self.layout.capacity() {
            self.for_each_partition(instance, emit)?;
        }
        Ok(())
    }

    fn for_each_partition(
        &self,
        instance: usize,
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        if instance >= self.layout.capacity() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let base = instance * self.layout.signature_stride();
        if instance < self.layout.batch() {
            let mut slack = self.slacks[instance];
            for side in 0..2 {
                for j in 0..N {
                    let mut value = self.coefficients[side][instance * N + j];
                    for bit in 0..COEFFICIENT_BITS {
                        let coefficient = if bit + 1 == COEFFICIENT_BITS {
                            self.field.neg(&value)
                        } else {
                            value
                        };
                        emit(base + coefficient_bit(N, side, j, bit), coefficient)?;
                        value = self.field.add(&value, &value);
                    }
                    let spare = if side == 0 && j < SLACK_BITS {
                        let value = slack;
                        slack = self.field.add(&slack, &slack);
                        value
                    } else {
                        self.padding
                    };
                    emit(base + coefficient_bit(N, side, j, 15), spare)?;
                }
            }
        } else {
            for offset in 0..self.layout.signature_stride() {
                emit(base + offset, self.padding)?;
            }
        }
        Ok(())
    }

    /// Aggregate coefficient weights by witness byte before expanding the
    /// common signed decoder. This replaces fifteen updates per coefficient
    /// with two bucket additions and a small fixed decoder table.
    fn for_each_partition_byte_bucket(
        &self,
        instance: usize,
        read_byte: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
        emit: &mut impl FnMut(u8, &[F; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        Some((|| {
            if instance >= self.layout.capacity() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let base = instance * self.layout.signature_stride();
            let zero = self.field.zero();
            if instance >= self.layout.batch() {
                let mut counts = [0u64; 256];
                for offset in (0..self.layout.signature_stride()).step_by(8) {
                    counts[read_byte(base + offset, 8)? as usize] += 1;
                }
                for (byte, &count) in counts.iter().enumerate().skip(1) {
                    if count != 0 {
                        let value = self
                            .field
                            .mul(&self.padding, &unsigned(count as u128, self.field));
                        emit(byte as u8, &[value; 8])?;
                    }
                }
                return Ok(());
            }
            let mut low = [zero; 256];
            let mut high = [zero; 256];
            let mut spare = [zero; 256];
            let mut occupied = [false; 256];
            let mut slack = self.slacks[instance];
            for side in 0..2 {
                for j in 0..N {
                    let offset = base + coefficient_bit(N, side, j, 0);
                    let low_byte = read_byte(offset, 8)? as usize;
                    let high_byte = read_byte(offset + 8, 8)? as usize;
                    let coefficient = self.coefficients[side][instance * N + j];
                    if low_byte != 0 {
                        low[low_byte] = self.field.add(&low[low_byte], &coefficient);
                        occupied[low_byte] = true;
                    }
                    let spare_value = if side == 0 && j < SLACK_BITS {
                        let value = slack;
                        slack = self.field.add(&slack, &slack);
                        value
                    } else {
                        self.padding
                    };
                    if high_byte != 0 {
                        high[high_byte] = self.field.add(&high[high_byte], &coefficient);
                        spare[high_byte] = self.field.add(&spare[high_byte], &spare_value);
                        occupied[high_byte] = true;
                    }
                }
            }
            let high_scale = unsigned(1 << 8, self.field);
            for byte in 1..256 {
                if !occupied[byte] {
                    continue;
                }
                let mut lo = low[byte];
                let mut hi = self.field.mul(&high[byte], &high_scale);
                let mut values = [zero; 8];
                for lane in 0..7 {
                    values[lane] = if lane == 6 {
                        self.field.sub(&lo, &hi)
                    } else {
                        self.field.add(&lo, &hi)
                    };
                    lo = self.field.add(&lo, &lo);
                    hi = self.field.add(&hi, &hi);
                }
                values[7] = self.field.add(&lo, &spare[byte]);
                emit(byte as u8, &values)?;
            }
            Ok(())
        })())
    }

    /// Bind up to the four lane variables once for each decoder. The replay
    /// then needs one multiplication per surviving coefficient group.
    fn for_each_partition_folded_final(
        &self,
        instance: usize,
        weights: &[F],
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if !weights.len().is_power_of_two() || weights.len() > COEFFICIENT_STRIDE {
            return None;
        }
        Some((|| {
            if instance >= self.layout.capacity() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let width = weights.len();
            let base = instance * self.layout.signature_stride() / width;
            if instance >= self.layout.batch() {
                let weight = weights
                    .iter()
                    .fold(self.field.zero(), |sum, value| self.field.add(&sum, value));
                let value = self.field.mul(&self.padding, &weight);
                for offset in 0..self.layout.signature_stride() / width {
                    emit(base + offset, value)?;
                }
                return Ok(());
            }
            let mut decoder = [self.field.zero(); COEFFICIENT_STRIDE];
            let mut power = self.field.one();
            for lane in 0..COEFFICIENT_BITS {
                let value = if lane + 1 == COEFFICIENT_BITS {
                    self.field.neg(&power)
                } else {
                    power
                };
                decoder[lane / width] = self.field.add(
                    &decoder[lane / width],
                    &self.field.mul(&value, &weights[lane % width]),
                );
                power = self.field.add(&power, &power);
            }
            let groups = COEFFICIENT_STRIDE / width;
            let mut slack = self.slacks[instance];
            for side in 0..2 {
                for j in 0..N {
                    let coefficient = self.coefficients[side][instance * N + j];
                    let spare = if side == 0 && j < SLACK_BITS {
                        let value = slack;
                        slack = self.field.add(&slack, &slack);
                        value
                    } else {
                        self.padding
                    };
                    for (group, decode) in decoder[..groups].iter().enumerate() {
                        let mut value = self.field.mul(&coefficient, decode);
                        if group + 1 == groups {
                            value = self
                                .field
                                .add(&value, &self.field.mul(&spare, &weights[width - 1]));
                        }
                        emit(base + (side * N + j) * groups + group, value)?;
                    }
                }
            }
            Ok(())
        })())
    }
}

fn signed_decoder(weights: &[F], field: &Cfg) -> F {
    let mut value = field.zero();
    let mut power = field.one();
    for (bit, weight) in weights.iter().enumerate() {
        let term = field.mul(weight, &power);
        value = if bit + 1 == COEFFICIENT_BITS {
            field.sub(&value, &term)
        } else {
            field.add(&value, &term)
        };
        power = field.add(&power, &power);
    }
    value
}

fn bind_opening(
    transcript: &mut impl Transcript,
    point: Vec<F>,
    value: F,
    field: &Cfg,
) -> BridgeClaim {
    transcript.absorb_slice(b"bitz/falcon1024-algebraic/source-opening/v2");
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    absorb_field_elements(transcript, &point, field);
    absorb_field_elements(transcript, &[value], field);
    BridgeClaim {
        point,
        modulus: field.modulus_u128(),
        value,
    }
}

fn validate_field(layout: &Layout, field: &Cfg) -> Result<(), FalconError> {
    // Each raw signed15/slack27 norm residual is below 2^40. This bound is
    // applied before random batching, never to the weighted batched residual.
    if field.modulus_u128() <= (1u128 << 40) || layout.source_bits() as u128 >= field.modulus_u128()
    {
        return Err(failure("algebraic integer or padding equation may wrap"));
    }
    Ok(())
}

fn prove_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
) -> Result<Option<u64>, FalconError> {
    if bits == 0 {
        return Ok(None);
    }
    grind_and_absorb(transcript, GrindingRound::<D>::new(0), bits)
        .map(Some)
        .map_err(failure)
}

fn verify_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
    nonce: Option<u64>,
) -> Result<(), FalconError> {
    match (bits, nonce) {
        (0, None) => Ok(()),
        (0, Some(_)) | (_, None) => Err(failure("invalid algebraic grinding nonce shape")),
        (_, Some(nonce)) => {
            verify_and_absorb(transcript, GrindingRound::<D>::new(0), bits, nonce).map_err(failure)
        }
    }
}

fn source_rounds(layout: &Layout) -> usize {
    layout.source_bits().ilog2() as usize
}
fn sample(transcript: &mut impl Transcript, field: &Cfg) -> Result<F, FalconError> {
    squeeze_field(transcript, field).map_err(failure)
}
fn sample_point(
    transcript: &mut impl Transcript,
    count: usize,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    (0..count).map(|_| sample(transcript, field)).collect()
}
fn unsigned(value: u128, field: &Cfg) -> F {
    F::from_with_cfg(value, field)
}
fn signed(value: i128, field: &Cfg) -> F {
    let value_field = unsigned(value.unsigned_abs(), field);
    if value < 0 {
        field.neg(&value_field)
    } else {
        value_field
    }
}
fn failure(error: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::FalconAlgebraicWitness;
    use super::*;
    use crate::piop::spartan::falcon_bit_layout::slack_bit;
    use crate::sumcheck::boundary::UngrindedRoundBoundary;
    use crate::transcript::Blake3Transcript;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    fn schedule() -> Schedule {
        Schedule {
            norm_instance_bits: 0,
            norm_round_bits: 0,
            merge_bits: 0,
            binding_bits: 0,
        }
    }

    fn data(batch: usize) -> WitnessData {
        let mut witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![[0; N]; batch],
        };
        let mut slacks = Vec::new();
        for i in 0..batch {
            witness.s1[i][i] = -(i as i16 + 3);
            witness.s2[i][N - i - 1] = 3000 + i as i16;
            let norm = witness.s1[i]
                .iter()
                .chain(&witness.s2[i])
                .map(|&v| (i64::from(v) * i64::from(v)) as u64)
                .sum::<u64>();
            slacks.push(BETA_SQUARED - norm);
        }
        WitnessData {
            witness,
            slacks,
            quotients: vec![vec![0; N - 1]; batch],
        }
    }

    fn ring(data: &WitnessData, field: &Cfg) -> PreparedClaim {
        let len = data.witness.s1.len() * N;
        let weights_s1: Vec<_> = (0..len)
            .map(|i| unsigned((i % 37 + 1) as u128, field))
            .collect();
        let weights_s2: Vec<_> = (0..len)
            .map(|i| unsigned((i % 43 + 1) as u128, field))
            .collect();
        let target = [&data.witness.s1, &data.witness.s2]
            .into_iter()
            .zip([&weights_s1, &weights_s2])
            .fold(field.zero(), |sum, (values, weights)| {
                values
                    .iter()
                    .flatten()
                    .zip(weights)
                    .fold(sum, |sum, (&value, weight)| {
                        field.add(&sum, &field.mul(weight, &signed(value as i128, field)))
                    })
            });
        PreparedClaim {
            weights_s1,
            weights_s2,
            target,
        }
    }

    fn open_source(source: &Source, point: &[F], field: &Cfg) -> F {
        eq_table(point, field)
            .unwrap()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| source.bit(*i))
            .fold(field.zero(), |sum, (_, weight)| field.add(&sum, &weight))
    }

    fn reference_coefficient(form: &BindingForm<'_>, index: usize) -> F {
        let instance = index / (16 * 2 * N);
        let coefficient = (index / 16) % (2 * N);
        let lane = index % 16;
        if instance >= form.layout.batch() {
            return form.padding;
        }
        if lane == 15 {
            return if coefficient < SLACK_BITS {
                form.field.mul(
                    &form.slacks[instance],
                    &unsigned(1u128 << coefficient, form.field),
                )
            } else {
                form.padding
            };
        }
        let weight = form.coefficients[coefficient / N][instance * N + coefficient % N];
        let digit = if lane == 14 {
            -(1i128 << 14)
        } else {
            1i128 << lane
        };
        form.field.mul(&weight, &signed(digit, form.field))
    }

    #[test]
    fn aligned_binding_matches_dense_evaluation_folds_and_sumcheck() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let form = BindingForm {
            layout: &layout,
            field: &field,
            coefficients: std::array::from_fn(|side| {
                (0..3 * N)
                    .map(|j| signed((j as i128 % 53 - 26) * (side + 1) as i128, &field))
                    .collect()
            }),
            slacks: vec![
                unsigned(13, &field),
                signed(-19, &field),
                unsigned(23, &field),
            ],
            padding: unsigned(29, &field),
        };
        let point: Vec<_> = (0..form.num_vars())
            .map(|j| unsigned((j + 2) as u128, &field))
            .collect();
        let dense_value = eq_table(&point, &field).unwrap().iter().enumerate().fold(
            field.zero(),
            |sum, (index, weight)| {
                field.add(
                    &sum,
                    &field.mul(weight, &reference_coefficient(&form, index)),
                )
            },
        );
        assert_eq!(form.evaluate(&point).unwrap(), dense_value);

        for instance in [0, 2, 3] {
            let base = instance * layout.signature_stride();
            for width in [1usize, 2, 4, 8, 16] {
                let point: Vec<_> = (0..width.ilog2())
                    .map(|j| unsigned((j + 3) as u128, &field))
                    .collect();
                let weights = eq_table(&point, &field).unwrap();
                let mut count = 0;
                form.for_each_partition_folded_final(instance, &weights, &mut |index, value| {
                    let expected =
                        weights
                            .iter()
                            .enumerate()
                            .fold(field.zero(), |sum, (lane, weight)| {
                                field.add(
                                    &sum,
                                    &field.mul(
                                        weight,
                                        &reference_coefficient(&form, index * width + lane),
                                    ),
                                )
                            });
                    assert_eq!(index, base / width + count);
                    assert_eq!(value, expected);
                    count += 1;
                    Ok(())
                })
                .unwrap()
                .unwrap();
                assert_eq!(count, layout.signature_stride() / width);
            }
        }

        // Arbitrary bits include every internal and inactive padding position.
        // The comparison checks the prefix buckets, folded suffix and all
        // transcript rounds, not only evaluations on honest zero padding.
        let bits: Vec<_> = (0..layout.source_bits() / 64)
            .map(|j| {
                (j as u64)
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    .rotate_left(17)
                    ^ 0xa591_f027_c37b_40de
            })
            .collect();
        let claim = (0..layout.source_bits())
            .filter(|&index| bits[index / 64] >> (index % 64) & 1 != 0)
            .fold(field.zero(), |sum, index| {
                field.add(&sum, &reference_coefficient(&form, index))
            });
        let mut actual_transcript = Blake3Transcript::new();
        let actual = prove_inner_sumcheck(
            &field,
            &mut actual_transcript,
            claim,
            PackedInput::new(
                &StreamingMle::new(&form),
                &bits,
                form.num_vars(),
                form.live_len(),
                3,
            ),
            (),
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let mut reference_transcript = Blake3Transcript::new();
        let reference = prove_inner_sumcheck(
            &field,
            &mut reference_transcript,
            claim,
            PackedInput::new(
                &|index| Ok(reference_coefficient(&form, index)),
                &bits,
                form.num_vars(),
                form.live_len(),
                3,
            ),
            (),
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(actual, reference);
        assert_eq!(
            actual_transcript.get_challenge::<u128>(),
            reference_transcript.get_challenge::<u128>()
        );
    }

    #[test]
    fn singleton_and_padded_batch_open_the_same_bits() {
        let field = field();
        for batch in [1, 3] {
            let layout = Layout::new(batch).unwrap();
            let data = data(batch);
            let source = Source::new(layout, &data);
            let ring = ring(&data, &field);
            let (proof, rows, claim) = prove(
                &mut Blake3Transcript::new(),
                &layout,
                &data,
                &source,
                &field,
                &ring,
                &schedule(),
            )
            .unwrap();
            let verified = verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .unwrap();
            assert_eq!(claim, verified);
            assert_eq!(claim.value, open_source(&source, &claim.point, &field));
            assert_eq!(rows.len(), 1 << layout.row_vars());
        }
    }

    #[test]
    fn invalid_slack_and_cross_witness_binding_are_rejected() {
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let mut wrong_slack = data.clone();
        wrong_slack.slacks[0] += 1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &wrong_slack,
                &source,
                &field,
                &ring,
                &schedule()
            )
            .is_err()
        );
        let mut other_witness = data.clone();
        // Sign reversal preserves the norm but must not authenticate to the
        // original source or native-ring claim.
        other_witness.witness.s1[0][0] *= -1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &other_witness,
                &source,
                &field,
                &ring,
                &schedule()
            )
            .is_err()
        );
    }

    #[test]
    fn signatures_cannot_borrow_each_others_norm_budget() {
        let field = field();
        let layout = Layout::new(2).unwrap();
        let excessive = (BETA_SQUARED.isqrt() + 1) as i16;
        let excessive_norm = (excessive as u64).pow(2);
        let mut data = WitnessData {
            witness: FalconAlgebraicWitness {
                s1: vec![[0; N]; 2],
                s2: vec![[0; N]; 2],
            },
            slacks: vec![0, 2 * BETA_SQUARED - excessive_norm],
            quotients: vec![vec![0; N - 1]; 2],
        };
        data.witness.s2[0][0] = excessive;
        // The unweighted batch total is exactly 2B, despite the first
        // signature having squared norm above B.
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &data,
                &source,
                &field,
                &ring,
                &schedule(),
            )
            .is_err()
        );
    }

    #[test]
    fn altered_bits_and_both_kinds_of_padding_are_rejected() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let data = data(3);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        for index in [
            0,
            coefficient_bit(N, 1, 0, 14),
            slack_bit(0),
            slack_bit(SLACK_BITS),
            3 * layout.signature_stride(),
        ] {
            let mut altered = source.clone();
            altered.flip(index);
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &data,
                    &altered,
                    &field,
                    &ring,
                    &schedule()
                )
                .is_err(),
                "bit {index} was not bound"
            );
        }
    }

    #[test]
    fn malformed_messages_and_nonces_are_rejected() {
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let (proof, _, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule(),
        )
        .unwrap();
        let rejects = |proof: &Proof| {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &field,
                    &ring,
                    proof,
                    &schedule()
                )
                .is_err()
            )
        };
        let mut altered = proof.clone();
        altered.norm.terminal[0][0] = field.add(&altered.norm.terminal[0][0], &field.one());
        rejects(&altered);
        let mut altered = proof.clone();
        altered.norm.claims[0] = field.add(&altered.norm.claims[0], &field.one());
        rejects(&altered);
        let mut altered = proof.clone();
        altered.norm.sumchecks[1].round_polynomials.pop();
        rejects(&altered);
        let mut altered = proof.clone();
        altered.binding.round_polynomials.pop();
        rejects(&altered);
        let mut altered = proof.clone();
        altered.binding_terminal[1] = field.add(&altered.binding_terminal[1], &field.one());
        rejects(&altered);
        let mut altered = proof.clone();
        altered.merge_nonce = Some(0);
        rejects(&altered);
        let mut altered = proof.clone();
        altered.binding_nonces.push(0);
        rejects(&altered);
    }

    #[test]
    fn preceding_transcript_domains_cannot_be_replayed() {
        struct LegacyDomains(Blake3Transcript);
        impl Transcript for LegacyDomains {
            fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
                self.0.get_challenge()
            }
            fn begin_sampling(&mut self) {
                self.0.begin_sampling();
            }
            fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
                self.0.fill_sampling_bytes(output);
            }
            fn absorb_inner(&mut self, value: &[u8]) {
                if value.starts_with(b"bitz/falcon1024-algebraic/") && value.ends_with(b"/v2") {
                    let mut legacy = value.to_vec();
                    *legacy.last_mut().unwrap() = b'1';
                    self.0.absorb_inner(&legacy);
                } else {
                    self.0.absorb_inner(value);
                }
            }
        }
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        // Keep the new geometry here to isolate domain separation itself.
        let (proof, _, claim) = prove(
            &mut LegacyDomains(Blake3Transcript::new()),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule(),
        )
        .unwrap();
        assert_eq!(
            verify(
                &mut LegacyDomains(Blake3Transcript::new()),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .unwrap(),
            claim
        );
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .is_err()
        );
    }

    #[test]
    fn grinded_messages_roundtrip_and_require_all_nonces() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let data = data(3);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let schedule = Schedule {
            norm_instance_bits: 2,
            norm_round_bits: 2,
            merge_bits: 2,
            binding_bits: 2,
        };
        let (mut proof, _, claim) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule,
        )
        .unwrap();
        assert_eq!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule
            )
            .unwrap(),
            claim
        );
        proof.norm.grinding_nonces.pop();
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule
            )
            .is_err()
        );
    }
}
