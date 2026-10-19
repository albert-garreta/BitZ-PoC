//! Prime-field binding directly to the compact low8/high4/exception planes.

use super::super::compact::{MAX_EXCEPTIONS, TAIL_STRIDE};
use super::*;
use crate::pcs::IntegerMatrixLayout;
use crate::sumcheck::inner::packed::Sha256InnerBitSource;
use flock_core::field::Gf128 as Gf;

const DOMAIN: &[u8] = b"bitz/falcon-algebraic/compact-integer-binding/v1";
const BRANCHES: [&[u8]; 3] = [b"low8", b"high4", b"exception-and-slack"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CompactBindingProof {
    pub initial_sum: F,
    pub sumcheck: SumcheckProof<F, 3>,
    pub terminal: [F; 2],
    pub nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CompactProof {
    pub norm: NormProof,
    pub merge_nonce: Option<u64>,
    pub bindings: [CompactBindingProof; 3],
}

impl CompactProof {
    pub(in super::super) fn payload_size_bytes(&self) -> usize {
        16 * (2
            + 4 * self.norm.sumcheck.round_polynomials.len()
            + self
                .bindings
                .iter()
                .map(|p| 3 + 3 * p.sumcheck.round_polynomials.len())
                .sum::<usize>())
            + 8 * (usize::from(self.norm.instance_nonce.is_some())
                + self.norm.grinding_nonces.len()
                + usize::from(self.merge_nonce.is_some())
                + self.bindings.iter().map(|p| p.nonces.len()).sum::<usize>())
    }
}

/// Reads a logical prefix without copying the physically padded PCS plane.
struct PlaneBits<'a> {
    packed: &'a [Gf],
    len: usize,
}

impl PlaneBits<'_> {
    fn word(&self, word: usize) -> u64 {
        let packed = self.packed[word / 2];
        if word % 2 == 0 { packed.lo } else { packed.hi }
    }
}

impl Sha256InnerBitSource for PlaneBits<'_> {
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError> {
        self.bits_at(index, 1)
    }

    fn bits_at(&self, index: usize, count: usize) -> Result<u64, SumcheckError> {
        if count > 64 || index.checked_add(count).is_none_or(|end| end > self.len) {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        if count == 0 {
            return Ok(0);
        }
        let shift = index % 64;
        let mut value = self.word(index / 64) >> shift;
        if shift + count > 64 {
            value |= self.word(index / 64 + 1) << (64 - shift);
        }
        Ok(if count == 64 {
            value
        } else {
            value & ((1 << count) - 1)
        })
    }

    fn validate_shape(&self, live_len: usize, table_len: usize) -> Result<(), SumcheckError> {
        if live_len != self.len
            || table_len != self.len
            || self.packed.len() < self.len.div_ceil(128)
        {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }
}

fn dimensions(
    layout: &Layout,
    exceptions: &[Vec<u16>],
    matrices: &[IntegerMatrixLayout; 3],
) -> Result<[usize; 3], FalconError> {
    let lengths = [
        16 * N * layout.capacity(),
        8 * N * layout.capacity(),
        (TAIL_STRIDE * layout.capacity()).max(1 << 16),
    ];
    if exceptions.len() != layout.batch()
        || exceptions.iter().any(|indices| {
            indices.len() > MAX_EXCEPTIONS
                || indices.iter().any(|&j| usize::from(j) >= 2 * N)
                || indices.windows(2).any(|pair| pair[0] >= pair[1])
        })
        || matrices
            .iter()
            .zip(lengths)
            .zip([12, 11, 10])
            .any(|((matrix, len), rows)| {
                matrix.row_vars != rows || matrix.row_vars + matrix.col_vars != len.ilog2() as usize
            })
    {
        return Err(failure(
            "compact integer binding layout or exception indices",
        ));
    }
    Ok(lengths)
}

fn bind_parameters(
    t: &mut impl Transcript,
    layout: &Layout,
    exceptions: &[Vec<u16>],
    matrices: &[IntegerMatrixLayout; 3],
) {
    t.absorb_slice(DOMAIN);
    for value in [
        N,
        layout.batch(),
        layout.capacity(),
        MAX_EXCEPTIONS,
        TAIL_STRIDE,
    ] {
        t.absorb_slice(&(value as u64).to_le_bytes());
    }
    for matrix in matrices {
        t.absorb_slice(&(matrix.row_vars as u64).to_le_bytes());
        t.absorb_slice(&(matrix.col_vars as u64).to_le_bytes());
    }
    for indices in exceptions {
        t.absorb_slice(&(indices.len() as u16).to_le_bytes());
        for index in indices {
            t.absorb_slice(&index.to_le_bytes());
        }
    }
}

pub(in super::super) fn prove_compact(
    t: &mut impl Transcript,
    layout: &Layout,
    data: &WitnessData,
    planes: [&[Gf]; 3],
    exceptions: &[Vec<u16>],
    matrices: &[IntegerMatrixLayout; 3],
    field: &Cfg,
    ring: &PreparedClaim,
    schedule: &Schedule,
) -> Result<(CompactProof, [BridgeClaim; 3]), FalconError> {
    let lengths = dimensions(layout, exceptions, matrices)?;
    validate_field(layout, field)?;
    let bits: [_; 3] = std::array::from_fn(|branch| PlaneBits {
        packed: planes[branch],
        len: lengths[branch],
    });
    for plane in &bits {
        plane
            .validate_shape(plane.len, plane.len)
            .map_err(failure)?;
    }
    bind_parameters(t, layout, exceptions, matrices);
    let (norm, claims) = prove_norm(t, layout, data, field, schedule)?;
    let _span = tracing::info_span!("falcon_algebraic:compact_binding").entered();
    t.absorb_slice(MERGE_DOMAIN);
    let merge_nonce = prove_nonce::<Merge>(t, schedule.merge_bits)?;
    let eta = sample(t, field)?;
    let (form, target) = BindingForm::new(layout, ring, &claims, eta, field)?;
    let forms: [_; 3] = std::array::from_fn(|branch| CompactForm {
        form: &form,
        exceptions,
        branch,
        len: lengths[branch],
    });
    let mut initial = [field.zero(); 3];
    for branch in 0..3 {
        initial[branch] = forms[branch].dot(&bits[branch])?;
    }
    check_initial(field, initial, target)?;
    t.absorb_slice(b"compact-partial-sums");
    absorb_field_elements(t, &initial, field);
    let mut bindings = Vec::with_capacity(3);
    let mut openings = Vec::with_capacity(3);
    for branch in 0..3 {
        t.absorb_slice(BRANCHES[branch]);
        let stream = StreamingMle::new(&forms[branch]);
        let mut boundary = ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(
            schedule.binding_bits,
            0,
        );
        let output = prove_inner_sumcheck(
            field,
            t,
            initial[branch],
            PackedInput::new(
                &stream,
                &bits[branch],
                lengths[branch].ilog2() as usize,
                lengths[branch],
                if branch < 2 {
                    binding_prefix_variables(layout.batch())
                } else {
                    3
                },
            ),
            (),
            &mut boundary,
        )
        .map_err(failure)?;
        let terminal = output.terminal_evaluations;
        if output.final_claim != field.mul(&terminal[0], &terminal[1])
            || forms[branch].evaluate(&output.point)? != terminal[0]
        {
            return Err(failure("compact integer binding terminal mismatch"));
        }
        absorb_field_elements(t, &terminal, field);
        let opening = bind_opening(
            t,
            matrices[branch].row_vars,
            output.point,
            terminal[1],
            field,
        )?;
        openings.push(opening);
        bindings.push(CompactBindingProof {
            initial_sum: initial[branch],
            sumcheck: output.proof,
            terminal,
            nonces: boundary.into_nonces(),
        });
    }
    Ok((
        CompactProof {
            norm,
            merge_nonce,
            bindings: bindings.try_into().unwrap(),
        },
        openings.try_into().unwrap(),
    ))
}

pub(in super::super) fn verify_compact(
    t: &mut impl Transcript,
    layout: &Layout,
    exceptions: &[Vec<u16>],
    matrices: &[IntegerMatrixLayout; 3],
    field: &Cfg,
    ring: &PreparedClaim,
    proof: &CompactProof,
    schedule: &Schedule,
) -> Result<[BridgeClaim; 3], FalconError> {
    let lengths = dimensions(layout, exceptions, matrices)?;
    validate_field(layout, field)?;
    bind_parameters(t, layout, exceptions, matrices);
    let norm = verify_norm(t, layout, &proof.norm, field, schedule)?;
    t.absorb_slice(MERGE_DOMAIN);
    verify_nonce::<Merge>(t, schedule.merge_bits, proof.merge_nonce)?;
    let eta = sample(t, field)?;
    let (form, target) = BindingForm::new(layout, ring, &norm, eta, field)?;
    let initial = proof.bindings.each_ref().map(|binding| binding.initial_sum);
    validate_field_elements(&initial, field).map_err(failure)?;
    check_initial(field, initial, target)?;
    t.absorb_slice(b"compact-partial-sums");
    absorb_field_elements(t, &initial, field);
    let mut openings = Vec::with_capacity(3);
    for branch in 0..3 {
        t.absorb_slice(BRANCHES[branch]);
        let binding = &proof.bindings[branch];
        let mut boundary = VerifierGrindingRoundBoundary::<BindingRound>::new(
            schedule.binding_bits,
            &binding.nonces,
        );
        let (point, final_claim) = binding
            .sumcheck
            .verify_with_round_boundary(
                t,
                initial[branch],
                lengths[branch].ilog2() as usize,
                field,
                &mut boundary,
            )
            .map_err(failure)?;
        validate_field_elements(&binding.terminal, field).map_err(failure)?;
        let compact = CompactForm {
            form: &form,
            exceptions,
            branch,
            len: lengths[branch],
        };
        if final_claim != field.mul(&binding.terminal[0], &binding.terminal[1])
            || compact.evaluate(&point)? != binding.terminal[0]
        {
            return Err(failure("compact integer binding terminal mismatch"));
        }
        absorb_field_elements(t, &binding.terminal, field);
        openings.push(bind_opening(
            t,
            matrices[branch].row_vars,
            point,
            binding.terminal[1],
            field,
        )?);
    }
    Ok(openings.try_into().unwrap())
}

fn check_initial(field: &Cfg, initial: [F; 3], target: F) -> Result<(), FalconError> {
    if initial
        .iter()
        .fold(field.zero(), |sum, value| field.add(&sum, value))
        != target
    {
        return Err(failure("compact integer binding partial sums mismatch"));
    }
    Ok(())
}

struct CompactForm<'a> {
    form: &'a BindingForm<'a>,
    exceptions: &'a [Vec<u16>],
    branch: usize,
    len: usize,
}

impl CompactForm<'_> {
    fn stride(&self) -> usize {
        [16 * N, 8 * N, TAIL_STRIDE][self.branch]
    }

    fn coefficient(&self, instance: usize, index: usize) -> F {
        self.form.coefficients[index / N][instance * N + index % N]
    }

    fn exception(&self, instance: usize, index: usize) -> bool {
        self.exceptions[instance]
            .binary_search(&(index as u16))
            .is_ok()
    }

    fn scale(&self, value: F, scalar: i64) -> F {
        let field = self.form.field;
        let scaled = field.reduce(field.mul_wide(&value, &scalar.unsigned_abs()));
        if scalar < 0 {
            field.neg(&scaled)
        } else {
            scaled
        }
    }

    fn dot(&self, bits: &PlaneBits<'_>) -> Result<F, FalconError> {
        let field = self.form.field;
        let sums = crate::utils::cfg_into_iter!(0..self.len / self.stride())
            .map(|instance| {
                let first_word = instance * self.stride() / 64;
                let words = self.stride() / 64;
                let popcount = || {
                    (0..words)
                        .map(|word| u64::from(bits.word(first_word + word).count_ones()))
                        .sum::<u64>()
                };
                if instance >= self.form.layout.batch() {
                    return Ok(self.scale(self.form.padding, popcount() as i64));
                }
                // Exact small-integer lifts of each packed plane. At most 2*N
                // coefficient terms of magnitude 2^14, or one 27-bit slack,
                // fit comfortably in these 192-bit sums.
                let mut positive = FpLinearAcc::<2, 1>::default();
                let mut negative = FpLinearAcc::<2, 1>::default();
                let mut add = |coefficient: F, digit: i64| {
                    if digit > 0 {
                        field.mul_acc(&mut positive, &coefficient, &(digit as u64));
                    } else if digit < 0 {
                        field.mul_acc(&mut negative, &coefficient, &digit.unsigned_abs());
                    }
                };
                if self.branch < 2 {
                    let digit_bits = if self.branch == 0 { 8 } else { 4 };
                    let per_word = 64 / digit_bits;
                    let mask = (1_u64 << digit_bits) - 1;
                    let mut rank = 0;
                    for word in 0..words {
                        let packed = bits.word(first_word + word);
                        for lane in 0..per_word {
                            let index = word * per_word + lane;
                            let digit = ((packed >> (lane * digit_bits)) & mask) as i64;
                            let digit = if self.branch == 0 {
                                digit
                            } else {
                                let exceptional = self.exceptions[instance]
                                    .get(rank)
                                    .is_some_and(|&j| usize::from(j) == index);
                                rank += usize::from(exceptional);
                                (digit - if !exceptional && digit >= 8 { 16 } else { 0 }) * 256
                            };
                            if digit != 0 {
                                add(self.coefficient(instance, index), digit);
                            }
                        }
                    }
                } else {
                    let upper = bits.word(first_word);
                    let mut live_ones = 0;
                    for (rank, &index) in self.exceptions[instance].iter().enumerate() {
                        let digit = (upper >> (3 * rank)) & 7;
                        live_ones += u64::from(digit.count_ones());
                        let signed = digit as i64 - if digit >= 4 { 8 } else { 0 };
                        add(
                            self.coefficient(instance, usize::from(index)),
                            signed * 4096,
                        );
                    }
                    let slack =
                        bits.bits_at(instance * TAIL_STRIDE + 3 * MAX_EXCEPTIONS, SLACK_BITS)?;
                    live_ones += u64::from(slack.count_ones());
                    add(self.form.slacks[instance], slack as i64);
                    add(self.form.padding, (popcount() - live_ones) as i64);
                }
                Ok::<_, SumcheckError>(field.sub(&field.reduce(positive), &field.reduce(negative)))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?;
        Ok(sums
            .into_iter()
            .fold(field.zero(), |sum, value| field.add(&sum, &value)))
    }

    fn evaluate(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != self.num_vars() {
            return Err(failure("compact binding evaluation shape"));
        }
        let field = self.form.field;
        let local_vars = self.stride().ilog2() as usize;
        let instances = eq_table(&point[local_vars..], field).map_err(failure)?;
        let mut result = field.zero();
        let mut live = field.zero();
        if self.branch < 2 {
            let lanes_log = if self.branch == 0 { 3 } else { 2 };
            let lanes = eq_table(&point[..lanes_log], field).map_err(failure)?;
            let coefficients = eq_table(&point[lanes_log..local_vars], field).map_err(failure)?;
            let decoder = lanes
                .iter()
                .enumerate()
                .fold(field.zero(), |sum, (lane, &weight)| {
                    let digit = if self.branch == 0 {
                        1 << lane
                    } else if lane == 3 {
                        -2048
                    } else {
                        256 << lane
                    };
                    field.add(&sum, &self.scale(weight, digit))
                });
            for (instance, instance_weight) in
                instances.iter().enumerate().take(self.form.layout.batch())
            {
                let mut ordinary = <Cfg as BatchMulAcc<F>>::Accumulator::default();
                for (index, weight) in coefficients.iter().enumerate() {
                    field.mul_acc(&mut ordinary, &self.coefficient(instance, index), weight);
                }
                let mut value = field.mul(&field.reduce(ordinary), &decoder);
                if self.branch == 1 {
                    let mut exceptional = <Cfg as BatchMulAcc<F>>::Accumulator::default();
                    for &index in &self.exceptions[instance] {
                        field.mul_acc(
                            &mut exceptional,
                            &self.coefficient(instance, usize::from(index)),
                            &coefficients[usize::from(index)],
                        );
                    }
                    value = field.add(
                        &value,
                        &field.mul(&field.reduce(exceptional), &self.scale(lanes[3], 4096)),
                    );
                }
                result = field.add(&result, &field.mul(&value, instance_weight));
                live = field.add(&live, instance_weight);
            }
        } else {
            let positions = eq_table(&point[..local_vars], field).map_err(failure)?;
            let slack = positions[3 * MAX_EXCEPTIONS..3 * MAX_EXCEPTIONS + SLACK_BITS]
                .iter()
                .enumerate()
                .fold(field.zero(), |sum, (bit, &weight)| {
                    field.add(&sum, &self.scale(weight, 1 << bit))
                });
            let slack_live = positions[3 * MAX_EXCEPTIONS..3 * MAX_EXCEPTIONS + SLACK_BITS]
                .iter()
                .fold(field.zero(), |sum, weight| field.add(&sum, weight));
            for (instance, instance_weight) in
                instances.iter().enumerate().take(self.form.layout.batch())
            {
                let mut value = field.mul(&self.form.slacks[instance], &slack);
                let mut local_live = slack_live;
                for (rank, &index) in self.exceptions[instance].iter().enumerate() {
                    let mut decoder = field.zero();
                    for (lane, digit) in [4096, 8192, -16384].into_iter().enumerate() {
                        decoder =
                            field.add(&decoder, &self.scale(positions[3 * rank + lane], digit));
                        local_live = field.add(&local_live, &positions[3 * rank + lane]);
                    }
                    value = field.add(
                        &value,
                        &field.mul(&self.coefficient(instance, usize::from(index)), &decoder),
                    );
                }
                result = field.add(&result, &field.mul(&value, instance_weight));
                live = field.add(&live, &field.mul(&local_live, instance_weight));
            }
        }
        Ok(field.add(
            &result,
            &field.mul(&self.form.padding, &field.sub(&field.one(), &live)),
        ))
    }
}

impl StreamingCoefficientSource for CompactForm<'_> {
    fn binding_options(&self) -> BindingOptions {
        BindingOptions {
            optimized: true,
            group_cap: 1,
        }
    }
    fn num_vars(&self) -> usize {
        self.len.ilog2() as usize
    }
    fn live_len(&self) -> usize {
        self.len
    }
    fn partition_len(&self) -> usize {
        self.stride()
    }

    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        for instance in 0..self.len / self.stride() {
            self.for_each_partition(instance, emit)?;
        }
        Ok(())
    }

    fn for_each_partition(
        &self,
        instance: usize,
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        if instance >= self.len / self.stride() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let base = instance * self.stride();
        if instance >= self.form.layout.batch() {
            for offset in 0..self.stride() {
                emit(base + offset, self.form.padding)?;
            }
        } else if self.branch < 2 {
            for index in 0..2 * N {
                let mut value = self.coefficient(instance, index);
                let lanes = if self.branch == 0 { 8 } else { 4 };
                if self.branch == 1 {
                    value = self.scale(value, 256);
                }
                for lane in 0..lanes {
                    let coefficient =
                        if self.branch == 1 && lane == 3 && !self.exception(instance, index) {
                            self.form.field.neg(&value)
                        } else {
                            value
                        };
                    emit(base + index * lanes + lane, coefficient)?;
                    value = self.form.field.add(&value, &value);
                }
            }
        } else {
            for offset in 0..TAIL_STRIDE {
                let value = if offset < 3 * self.exceptions[instance].len() {
                    self.scale(
                        self.coefficient(
                            instance,
                            usize::from(self.exceptions[instance][offset / 3]),
                        ),
                        [4096, 8192, -16384][offset % 3],
                    )
                } else if (3 * MAX_EXCEPTIONS..3 * MAX_EXCEPTIONS + SLACK_BITS).contains(&offset) {
                    self.scale(
                        self.form.slacks[instance],
                        1 << (offset - 3 * MAX_EXCEPTIONS),
                    )
                } else {
                    self.form.padding
                };
                emit(base + offset, value)?;
            }
        }
        Ok(())
    }

    fn for_each_partition_byte_bucket(
        &self,
        instance: usize,
        read: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
        emit: &mut impl FnMut(u8, &[F; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if self.branch == 2 && instance < self.form.layout.batch() {
            return None;
        }
        Some((|| {
            if instance >= self.len / self.stride() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let field = self.form.field;
            let base = instance * self.stride();
            if instance >= self.form.layout.batch() {
                let mut counts = [0_u64; 256];
                for offset in (0..self.stride()).step_by(8) {
                    counts[read(base + offset, 8)? as usize] += 1;
                }
                for (byte, &count) in counts.iter().enumerate().skip(1) {
                    if count != 0 {
                        emit(
                            byte as u8,
                            &[self.scale(self.form.padding, count as i64); 8],
                        )?;
                    }
                }
                return Ok(());
            }
            let mut sums = [[field.zero(); 256]; 2];
            let mut exceptions = [[field.zero(); 256]; 2];
            let mut occupied = [false; 256];
            let per_byte = if self.branch == 0 { 1 } else { 2 };
            for index in (0..2 * N).step_by(per_byte) {
                let byte = read(base + index * 8 / per_byte, 8)? as usize;
                if byte == 0 {
                    continue;
                }
                occupied[byte] = true;
                for part in 0..per_byte {
                    let value = self.coefficient(instance, index + part);
                    sums[part][byte] = field.add(&sums[part][byte], &value);
                    if self.branch == 1 && self.exception(instance, index + part) {
                        exceptions[part][byte] = field.add(&exceptions[part][byte], &value);
                    }
                }
            }
            for byte in 1..256 {
                if !occupied[byte] {
                    continue;
                }
                let mut values = [field.zero(); 8];
                if self.branch == 0 {
                    let mut value = sums[0][byte];
                    for entry in &mut values {
                        *entry = value;
                        value = field.add(&value, &value);
                    }
                } else {
                    for part in 0..2 {
                        let mut value = self.scale(sums[part][byte], 256);
                        for lane in 0..4 {
                            values[part * 4 + lane] = if lane == 3 {
                                field.sub(&self.scale(exceptions[part][byte], 4096), &value)
                            } else {
                                value
                            };
                            value = field.add(&value, &value);
                        }
                    }
                }
                emit(byte as u8, &values)?;
            }
            Ok(())
        })())
    }

    fn for_each_partition_byte_pair_bucket(
        &self,
        instance: usize,
        read: &mut impl FnMut(usize, usize) -> Result<u16, SumcheckError>,
        emit: &mut impl FnMut(usize, u8, &[F; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if self.branch == 2 && instance < self.form.layout.batch() {
            return None;
        }
        Some((|| {
            if instance >= self.len / self.stride() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let field = self.form.field;
            let base = instance * self.stride();
            if instance >= self.form.layout.batch() {
                let mut counts = [[0_u64; 256]; 3];
                for offset in (0..self.stride()).step_by(16) {
                    let pair = read(base + offset, 16)?;
                    let low = usize::from(pair & 255);
                    let high = usize::from(pair >> 8);
                    counts[0][low] += 1;
                    counts[1][high] += 1;
                    counts[2][low] += 1;
                    counts[2][high] += 1;
                }
                for (table, counts) in counts.iter().enumerate() {
                    for (byte, &count) in counts.iter().enumerate().skip(1) {
                        if count != 0 {
                            emit(
                                table,
                                byte as u8,
                                &[self.scale(self.form.padding, count as i64); 8],
                            )?;
                        }
                    }
                }
                return Ok(());
            }
            // With C0,C1 the two coefficient bytes and h0,h1 their witness
            // bytes, table 2 holds C0 grouped by h1 plus C1 grouped by h0.
            // Aggregate coefficient weights before expanding either decoder.
            let mut sums = [[[field.zero(); 256]; 2]; 3];
            let mut exceptions = [[[field.zero(); 256]; 2]; 3];
            let mut occupied = [[false; 256]; 3];
            let per_byte = if self.branch == 0 { 1 } else { 2 };
            for index in (0..2 * N).step_by(2 * per_byte) {
                let pair = read(base + index * 8 / per_byte, 16)?;
                let bytes = [usize::from(pair & 255), usize::from(pair >> 8)];
                for half in 0..2 {
                    for (table, byte) in [(half, bytes[half]), (2, bytes[1 - half])] {
                        if byte == 0 {
                            continue;
                        }
                        occupied[table][byte] = true;
                        for part in 0..per_byte {
                            let coefficient = index + half * per_byte + part;
                            let value = self.coefficient(instance, coefficient);
                            sums[table][part][byte] = field.add(&sums[table][part][byte], &value);
                            if self.branch == 1 && self.exception(instance, coefficient) {
                                exceptions[table][part][byte] =
                                    field.add(&exceptions[table][part][byte], &value);
                            }
                        }
                    }
                }
            }
            for table in 0..3 {
                for byte in 1..256 {
                    if !occupied[table][byte] {
                        continue;
                    }
                    let mut values = [field.zero(); 8];
                    if self.branch == 0 {
                        let mut value = sums[table][0][byte];
                        for output in &mut values {
                            *output = value;
                            value = field.add(&value, &value);
                        }
                    } else {
                        for part in 0..2 {
                            let mut value = self.scale(sums[table][part][byte], 256);
                            for lane in 0..4 {
                                values[4 * part + lane] = if lane == 3 {
                                    field.sub(
                                        &self.scale(exceptions[table][part][byte], 4096),
                                        &value,
                                    )
                                } else {
                                    value
                                };
                                value = field.add(&value, &value);
                            }
                        }
                    }
                    emit(table, byte as u8, &values)?;
                }
            }
            Ok(())
        })())
    }

    fn for_each_partition_block(
        &self,
        instance: usize,
        width: usize,
        emit: &mut impl FnMut(usize, &[F]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if !matches!(width, 8 | 16) {
            return None;
        }
        Some((|| {
            let mut values = [self.form.field.zero(); 16];
            self.for_each_partition(instance, &mut |index, value| {
                values[index % width] = value;
                if index % width + 1 == width {
                    emit(index + 1 - width, &values[..width])?;
                }
                Ok(())
            })
        })())
    }

    fn for_each_partition_folded_final(
        &self,
        instance: usize,
        weights: &[F],
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let width = weights.len();
        if !matches!(width, 8 | 16) {
            return None;
        }
        Some((|| {
            if instance >= self.len / self.stride() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let field = self.form.field;
            let base = instance * self.stride() / width;
            if instance >= self.form.layout.batch() {
                let weight = weights
                    .iter()
                    .fold(field.zero(), |sum, weight| field.add(&sum, weight));
                let value = field.mul(&weight, &self.form.padding);
                for offset in 0..self.stride() / width {
                    emit(base + offset, value)?;
                }
            } else if self.branch < 2 {
                let lanes = if self.branch == 0 { 8 } else { 4 };
                let per_group = width / lanes;
                let mut decoders = [field.zero(); 4];
                let mut corrections = [field.zero(); 4];
                for part in 0..per_group {
                    for lane in 0..lanes {
                        let digit = if self.branch == 0 {
                            1 << lane
                        } else if lane == 3 {
                            -2048
                        } else {
                            256 << lane
                        };
                        decoders[part] = field.add(
                            &decoders[part],
                            &self.scale(weights[part * lanes + lane], digit),
                        );
                    }
                    if self.branch == 1 {
                        corrections[part] = self.scale(weights[4 * part + 3], 4096);
                    }
                }
                for index in (0..2 * N).step_by(per_group) {
                    let mut sum = field.zero();
                    for part in 0..per_group {
                        let weight = if self.branch == 1 && self.exception(instance, index + part) {
                            field.add(&decoders[part], &corrections[part])
                        } else {
                            decoders[part]
                        };
                        let term = field.mul(&self.coefficient(instance, index + part), &weight);
                        sum = if part == 0 {
                            term
                        } else {
                            field.add(&sum, &term)
                        };
                    }
                    emit(base + index / per_group, sum)?;
                }
            } else {
                let mut sum = field.zero();
                self.for_each_partition(instance, &mut |index, value| {
                    sum = field.add(&sum, &field.mul(&value, &weights[index % width]));
                    if index % width + 1 == width {
                        emit(index / width, sum)?;
                        sum = field.zero();
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::FalconAlgebraicWitness;
    use super::*;
    use crate::transcript::Blake3Transcript;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1_u128 << 127) - 1)).unwrap()
    }

    fn fixture(batch: usize, field: &Cfg) -> (Layout, WitnessData, PreparedClaim) {
        let layout = Layout::new(batch).unwrap();
        let mut data = WitnessData {
            witness: FalconAlgebraicWitness {
                s1: vec![[0; N]; batch],
                s2: vec![[0; N]; batch],
            },
            slacks: vec![0; batch],
        };
        for instance in 0..batch {
            data.witness.s1[instance][0] = -2048;
            data.witness.s2[instance][N - 1] = 2048 + instance as i16;
            data.slacks[instance] = BETA_SQUARED
                - data.witness.s1[instance]
                    .iter()
                    .chain(&data.witness.s2[instance])
                    .map(|&value| i64::from(value).pow(2) as u64)
                    .sum::<u64>();
        }
        let weights_s1: Vec<_> = (0..batch * N)
            .map(|i| signed(i as i128 % 31 - 17, field))
            .collect();
        let weights_s2: Vec<_> = (0..batch * N)
            .map(|i| signed(i as i128 % 43 - 29, field))
            .collect();
        let mut target = field.zero();
        for (values, weights) in [
            (&data.witness.s1, &weights_s1),
            (&data.witness.s2, &weights_s2),
        ] {
            for (&value, weight) in values.iter().flatten().zip(weights) {
                target = field.add(
                    &target,
                    &field.mul(&signed(i128::from(value), field), weight),
                );
            }
        }
        (
            layout,
            data,
            PreparedClaim {
                weights_s1,
                weights_s2,
                target,
            },
        )
    }

    fn matrices(layout: &Layout) -> [IntegerMatrixLayout; 3] {
        let lengths = [
            16 * N * layout.capacity(),
            8 * N * layout.capacity(),
            (TAIL_STRIDE * layout.capacity()).max(1 << 16),
        ];
        std::array::from_fn(|branch| IntegerMatrixLayout {
            row_vars: [12, 11, 10][branch],
            col_vars: lengths[branch].ilog2() as usize - [12, 11, 10][branch],
        })
    }

    fn flip(plane: &mut [Gf], bit: usize) {
        if bit % 128 < 64 {
            plane[bit / 128].lo ^= 1 << (bit % 64);
        } else {
            plane[bit / 128].hi ^= 1 << (bit % 64);
        }
    }

    // Independent bitwise encoder: no virtual Source or production packing code.
    fn encode(layout: &Layout, data: &WitnessData) -> ([Vec<Gf>; 3], Vec<Vec<u16>>) {
        let sizes = matrices(layout).map(|p| p.rows() * p.cols());
        let mut planes: [Vec<Gf>; 3] =
            std::array::from_fn(|branch| vec![Gf::ZERO; sizes[branch] / 128]);
        let mut exceptions = vec![Vec::new(); layout.batch()];
        for instance in 0..layout.batch() {
            for (index, &value) in data.witness.s1[instance]
                .iter()
                .chain(&data.witness.s2[instance])
                .enumerate()
            {
                let bits = value as u16 & 0x7fff;
                for lane in 0..8 {
                    if bits >> lane & 1 != 0 {
                        flip(&mut planes[0], (instance * 2 * N + index) * 8 + lane);
                    }
                }
                for lane in 0..4 {
                    if bits >> (8 + lane) & 1 != 0 {
                        flip(&mut planes[1], (instance * 2 * N + index) * 4 + lane);
                    }
                }
                if !(-2048..=2047).contains(&value) {
                    let rank = exceptions[instance].len();
                    exceptions[instance].push(index as u16);
                    for lane in 0..3 {
                        if bits >> (12 + lane) & 1 != 0 {
                            flip(&mut planes[2], instance * TAIL_STRIDE + 3 * rank + lane);
                        }
                    }
                }
            }
            for lane in 0..SLACK_BITS {
                if data.slacks[instance] >> lane & 1 != 0 {
                    flip(
                        &mut planes[2],
                        instance * TAIL_STRIDE + 3 * MAX_EXCEPTIONS + lane,
                    );
                }
            }
        }
        (planes, exceptions)
    }

    fn reference(form: &CompactForm<'_>, index: usize) -> F {
        let instance = index / form.stride();
        let local = index % form.stride();
        let field = form.form.field;
        if instance >= form.form.layout.batch() {
            return form.form.padding;
        }
        let (coefficient, digit) = match form.branch {
            0 => (local / 8, 1_i128 << (local % 8)),
            1 => {
                let position = local / 4;
                let digit =
                    if local % 4 == 3 && !form.exceptions[instance].contains(&(position as u16)) {
                        -2048
                    } else {
                        1_i128 << (8 + local % 4)
                    };
                (position, digit)
            }
            _ if local < 3 * form.exceptions[instance].len() => (
                usize::from(form.exceptions[instance][local / 3]),
                [4096, 8192, -16384][local % 3],
            ),
            _ if (3 * MAX_EXCEPTIONS..3 * MAX_EXCEPTIONS + SLACK_BITS).contains(&local) => {
                return field.mul(
                    &form.form.slacks[instance],
                    &unsigned(1 << (local - 3 * MAX_EXCEPTIONS), field),
                );
            }
            _ => return form.form.padding,
        };
        field.mul(
            &form.form.coefficients[coefficient / N][instance * N + coefficient % N],
            &signed(digit, field),
        )
    }

    #[test]
    fn compact_integer_decoder_preserves_every_signed15_value() {
        let field = field();
        let layout = Layout::new(1).unwrap();
        let mut coefficients = [vec![field.zero(); N], vec![field.zero(); N]];
        coefficients[0][0] = field.one();
        let form = BindingForm {
            layout: &layout,
            field: &field,
            coefficients,
            slacks: vec![field.zero()],
            padding: field.one(),
            options: BindingOptions::REFERENCE,
            folded_decoder: Default::default(),
        };
        let shapes = matrices(&layout);
        let mut decoders = Vec::new();
        for exceptional in [false, true] {
            let exceptions = vec![if exceptional { vec![0] } else { Vec::new() }];
            let mut decoder = [field.zero(); 15];
            for branch in 0..3 {
                let compact = CompactForm {
                    form: &form,
                    exceptions: &exceptions,
                    branch,
                    len: shapes[branch].rows() * shapes[branch].cols(),
                };
                compact
                    .for_each_partition(0, &mut |index, value| {
                        if index < [8, 4, 3][branch] {
                            decoder[[0, 8, 12][branch] + index] = value;
                        }
                        Ok(())
                    })
                    .unwrap();
            }
            decoders.push(decoder);
        }
        for value in -16384_i16..=16383 {
            let exceptional = !(-2048..=2047).contains(&value);
            let encoded = value as u16 & if exceptional { 0x7fff } else { 0x0fff };
            let actual = decoders[usize::from(exceptional)]
                .iter()
                .enumerate()
                .filter(|(lane, _)| encoded >> lane & 1 != 0)
                .fold(field.zero(), |sum, (_, coefficient)| {
                    field.add(&sum, coefficient)
                });
            assert_eq!(actual, signed(i128::from(value), &field), "value={value}");
        }
    }

    #[test]
    fn compact_stream_and_sumchecks_match_independent_dense_coefficients() {
        let field = field();
        let (layout, data, ring) = fixture(3, &field);
        let (_, mut exceptions) = encode(&layout, &data);
        // Exercise both halves of each byte and all four high-nibble
        // positions in a 16-bit block, including the s1/s2 boundary.
        exceptions[0] = vec![
            0,
            1,
            (N - 2) as u16,
            (N - 1) as u16,
            (2 * N - 2) as u16,
            (2 * N - 1) as u16,
        ];
        exceptions[1].clear();
        exceptions[2] = vec![1, N as u16, (2 * N - 1) as u16];
        let norm = NormClaims {
            instance_point: vec![unsigned(5, &field); layout.capacity().ilog2() as usize],
            point: vec![
                unsigned(7, &field);
                COEFFICIENT_LOG + 1 + layout.capacity().ilog2() as usize
            ],
            terminal: signed(-23, &field),
            slack: unsigned(29, &field),
        };
        let (form, _) =
            BindingForm::new(&layout, &ring, &norm, unsigned(13, &field), &field).unwrap();
        let shapes = matrices(&layout);
        for branch in 0..3 {
            let len = shapes[branch].rows() * shapes[branch].cols();
            let compact = CompactForm {
                form: &form,
                exceptions: &exceptions,
                branch,
                len,
            };
            let dense: Vec<_> = (0..len).map(|index| reference(&compact, index)).collect();
            let mut streamed = vec![field.zero(); len];
            compact
                .for_each_coefficient(&mut |index, value| {
                    streamed[index] = field.add(&streamed[index], &value);
                    Ok(())
                })
                .unwrap();
            assert_eq!(streamed, dense);
            let point = vec![unsigned(17, &field); compact.num_vars()];
            let expected = dense
                .iter()
                .zip(eq_table(&point, &field).unwrap())
                .fold(field.zero(), |sum, (a, b)| {
                    field.add(&sum, &field.mul(a, &b))
                });
            assert_eq!(compact.evaluate(&point).unwrap(), expected);
            let packed: Vec<_> = (0..len / 128)
                .map(|word| Gf {
                    lo: (word as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15),
                    hi: (word as u64 + 1).wrapping_mul(0x25b44a15d941a31b),
                })
                .collect();
            let bits = PlaneBits {
                packed: &packed,
                len,
            };
            let target = dense
                .iter()
                .enumerate()
                .filter(|(index, _)| bits.bit_at(*index).unwrap() != 0)
                .fold(field.zero(), |sum, (_, value)| field.add(&sum, value));
            assert_eq!(compact.dot(&bits).unwrap(), target);
            let mut expected_t = Blake3Transcript::new();
            let mut expected_boundary =
                ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(2, 0);
            let expected = prove_inner_sumcheck(
                &field,
                &mut expected_t,
                target,
                PackedInput::new(&|index| Ok(dense[index]), &bits, compact.num_vars(), len, 3),
                (),
                &mut expected_boundary,
            )
            .unwrap();
            let expected_nonces = expected_boundary.into_nonces();
            let continuation = expected_t.get_challenge::<u128>();
            for prefix in [3, 4] {
                let mut actual_t = Blake3Transcript::new();
                let mut actual_boundary =
                    ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(2, 0);
                let actual = prove_inner_sumcheck(
                    &field,
                    &mut actual_t,
                    target,
                    PackedInput::new(
                        &StreamingMle::new(&compact),
                        &bits,
                        compact.num_vars(),
                        len,
                        prefix,
                    ),
                    (),
                    &mut actual_boundary,
                )
                .unwrap();
                assert_eq!(actual, expected, "branch={branch}, prefix={prefix}");
                assert_eq!(actual_boundary.into_nonces(), expected_nonces);
                assert_eq!(actual_t.get_challenge::<u128>(), continuation);
            }
        }
    }

    #[test]
    fn compact_proofs_authenticate_all_branches_and_padding() {
        let field = field();
        let (layout, data, ring) = fixture(3, &field);
        let matrices = matrices(&layout);
        let (planes, exceptions) = encode(&layout, &data);
        let schedule = Schedule {
            norm_instance_bits: 0,
            norm_round_bits: 0,
            merge_bits: 0,
            binding_bits: 0,
            word_binding: false,
            terminal_merge_bits: 0,
            merged_norm: false,
            cubic_binding_bits: 0,
        };
        let mut prover = Blake3Transcript::new();
        let (proof, claims) = prove_compact(
            &mut prover,
            &layout,
            &data,
            planes.each_ref().map(Vec::as_slice),
            &exceptions,
            &matrices,
            &field,
            &ring,
            &schedule,
        )
        .unwrap();
        let mut verifier = Blake3Transcript::new();
        let verified = verify_compact(
            &mut verifier,
            &layout,
            &exceptions,
            &matrices,
            &field,
            &ring,
            &proof,
            &schedule,
        )
        .unwrap();
        assert_eq!(claims, verified);
        assert_eq!(
            prover.get_challenge::<u128>(),
            verifier.get_challenge::<u128>()
        );
        for branch in 0..3 {
            assert_eq!(claims[branch].row_weights.len(), matrices[branch].rows());
            let bits = PlaneBits {
                packed: &planes[branch],
                len: matrices[branch].rows() * matrices[branch].cols(),
            };
            let expected = eq_table(&claims[branch].column_point, &field)
                .unwrap()
                .into_iter()
                .flat_map(|column| {
                    let field = &field;
                    claims[branch]
                        .row_weights
                        .iter()
                        .map(move |row| field.mul(&column, &unsigned(*row, field)))
                })
                .enumerate()
                .filter(|(index, _)| bits.bit_at(*index).unwrap() != 0)
                .fold(field.zero(), |sum, (_, weight)| field.add(&sum, &weight));
            assert_eq!(claims[branch].value, expected);
        }
        for (branch, bit) in [
            (0, 0),
            (1, 3),
            (2, 0),
            (2, 3),
            (2, 3 * MAX_EXCEPTIONS),
            (2, TAIL_STRIDE - 1),
            (0, 3 * 16 * N),
            (1, 3 * 8 * N),
            (2, 3 * TAIL_STRIDE),
            (2, TAIL_STRIDE * layout.capacity()),
        ] {
            let mut altered = planes.clone();
            if bit >= altered[branch].len() * 128 {
                continue;
            }
            flip(&mut altered[branch], bit);
            assert!(
                prove_compact(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &data,
                    altered.each_ref().map(Vec::as_slice),
                    &exceptions,
                    &matrices,
                    &field,
                    &ring,
                    &schedule
                )
                .is_err(),
                "branch={branch}, bit={bit}"
            );
        }
        for mutation in 0..6 {
            let mut altered = proof.clone();
            match mutation {
                0 => {
                    altered.bindings[0].initial_sum =
                        field.add(&altered.bindings[0].initial_sum, &field.one())
                }
                1 => {
                    altered.bindings[0].initial_sum =
                        field.add(&altered.bindings[0].initial_sum, &field.one());
                    altered.bindings[1].initial_sum =
                        field.sub(&altered.bindings[1].initial_sum, &field.one());
                }
                2 => {
                    altered.bindings[1].terminal[0] =
                        field.add(&altered.bindings[1].terminal[0], &field.one())
                }
                3 => {
                    altered.bindings[2].sumcheck.round_polynomials.pop();
                }
                4 => altered.bindings.swap(0, 1),
                _ => altered.bindings[0].nonces.push(0),
            }
            assert!(
                verify_compact(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &exceptions,
                    &matrices,
                    &field,
                    &ring,
                    &altered,
                    &schedule
                )
                .is_err(),
                "mutation={mutation}"
            );
        }
        for indices in [
            vec![0, 0],
            vec![1, 0],
            vec![(2 * N) as u16],
            (0..=MAX_EXCEPTIONS as u16).collect(),
        ] {
            let mut altered = exceptions.clone();
            altered[0] = indices;
            assert!(
                verify_compact(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &altered,
                    &matrices,
                    &field,
                    &ring,
                    &proof,
                    &schedule
                )
                .is_err()
            );
        }
        let mut changed = exceptions.clone();
        changed[0][0] -= 1;
        assert!(
            verify_compact(
                &mut Blake3Transcript::new(),
                &layout,
                &changed,
                &matrices,
                &field,
                &ring,
                &proof,
                &schedule
            )
            .is_err()
        );
    }
}
