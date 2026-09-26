//! Word-sized storage for the hybrid binder. The public operator is compiled
//! once and replayed in source order; no dense field table of source bits is kept.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum WordKind {
    Unsigned,
    Signed,
    Encoded { offset: u8, weighted: bool },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Word {
    base: usize,
    width: u8,
    kind: WordKind,
}

pub(super) struct CompiledCoefficients {
    words: std::sync::Arc<[Word]>,
    coefficients: Vec<F>,
}

/// All instances share the public operator and the same dynamic addition order.
/// Compile their word slots once, then accumulate each instance by slot index.
/// Word descriptors are also shared by the cached instance coefficient vectors.
pub(super) struct CompiledTemplate {
    words: std::sync::Arc<[Word]>,
    common: Vec<(usize, F)>,
    dynamic_slots: Vec<usize>,
}

trait WordAccumulator {
    fn add(&mut self, word: Word, value: F, field: &Cfg);
}

#[derive(Default)]
struct MapWords(HashMap<Word, F>);

impl WordAccumulator for MapWords {
    fn add(&mut self, word: Word, value: F, field: &Cfg) {
        let slot = self.0.entry(word).or_insert_with(|| field.zero());
        *slot = field.add(slot, &value);
    }
}

#[derive(Default)]
struct RecordWords(Vec<Word>);

impl WordAccumulator for RecordWords {
    fn add(&mut self, word: Word, _: F, _: &Cfg) {
        // Include zero coefficients: topology must not depend on challenges.
        self.0.push(word);
    }
}

struct IndexedWords<'a> {
    template: &'a CompiledTemplate,
    coefficients: Vec<F>,
    next: usize,
}

impl WordAccumulator for IndexedWords<'_> {
    fn add(&mut self, word: Word, value: F, field: &Cfg) {
        let index = self.template.dynamic_slots[self.next];
        // This also guards future edits that accidentally make the addition
        // order instance-dependent. No word lookup or sorting is needed here.
        assert_eq!(
            word, self.template.words[index],
            "compact word topology changed"
        );
        self.next += 1;
        let slot = &mut self.coefficients[index];
        *slot = field.add(slot, &value);
    }
}

struct WordSink<'a, A = MapWords> {
    field: &'a Cfg,
    instance: usize,
    base: usize,
    accumulator: A,
}

impl<'a> WordSink<'a> {
    fn new(instance: usize, base: usize, field: &'a Cfg) -> Self {
        Self {
            field,
            instance,
            base,
            accumulator: MapWords::default(),
        }
    }

    fn finish(self) -> CompiledCoefficients {
        let mut words: Vec<_> = self
            .accumulator
            .0
            .into_iter()
            .filter(|(_, value)| *value != self.field.zero())
            .collect();
        words.sort_unstable_by_key(|(word, _)| *word);
        let (words, coefficients): (Vec<_>, Vec<_>) = words.into_iter().unzip();
        CompiledCoefficients {
            words: words.into(),
            coefficients,
        }
    }
}

impl<A: WordAccumulator> WordSink<'_, A> {
    fn word(&mut self, base: usize, width: usize, kind: WordKind, value: F) {
        self.accumulator.add(
            Word {
                base: base - self.base,
                width: width as u8,
                kind,
            },
            value,
            self.field,
        );
    }
}

impl CompiledTemplate {
    fn new(common: CompiledCoefficients, dynamic_words: Vec<Word>) -> Self {
        let mut words = common.words.to_vec();
        words.extend_from_slice(&dynamic_words);
        words.sort_unstable();
        words.dedup();
        let common = common
            .words
            .iter()
            .zip(common.coefficients)
            .map(|(word, value)| (words.binary_search(word).expect("common word slot"), value))
            .collect();
        let dynamic_slots = dynamic_words
            .iter()
            .map(|word| words.binary_search(word).expect("dynamic word slot"))
            .collect();
        Self {
            words: words.into(),
            common,
            dynamic_slots,
        }
    }

    fn sink<'a>(
        &'a self,
        instance: usize,
        base: usize,
        alpha: F,
        field: &'a Cfg,
    ) -> WordSink<'a, IndexedWords<'a>> {
        let mut coefficients = vec![field.zero(); self.words.len()];
        for &(slot, value) in &self.common {
            coefficients[slot] = field.mul(&alpha, &value);
        }
        WordSink {
            field,
            instance,
            base,
            accumulator: IndexedWords {
                template: self,
                coefficients,
                next: 0,
            },
        }
    }
}

impl WordSink<'_, IndexedWords<'_>> {
    fn finish(self) -> CompiledCoefficients {
        assert_eq!(
            self.accumulator.next,
            self.accumulator.template.dynamic_slots.len(),
            "incomplete compact word topology"
        );
        CompiledCoefficients {
            words: std::sync::Arc::clone(&self.accumulator.template.words),
            coefficients: self.accumulator.coefficients,
        }
    }
}

impl<A: WordAccumulator> CoefficientSink for WordSink<'_, A> {
    fn instances(&self, _: usize) -> std::ops::Range<usize> {
        self.instance..self.instance + 1
    }

    fn needs_constants(&self) -> bool {
        false
    }

    fn add(&mut self, index: usize, value: F) {
        self.word(index, 1, WordKind::Unsigned, value);
    }

    fn add_word(&mut self, base: usize, width: usize, signed: bool, scale: F, _: &Cfg) {
        self.word(
            base,
            width,
            if signed {
                WordKind::Signed
            } else {
                WordKind::Unsigned
            },
            scale,
        );
    }

    fn add_encoded_word(&mut self, base: usize, offset: usize, weighted: bool, scale: F, _: &Cfg) {
        self.word(
            base,
            12,
            WordKind::Encoded {
                offset: offset as u8,
                weighted,
            },
            scale,
        );
    }
}

impl CompiledCoefficients {
    pub(super) fn emit(
        &self,
        base: usize,
        field: &Cfg,
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        // Every ordinary word has <=27 bits; an encoded 12-bit word spans
        // <=16 physical bit positions. A sliding window combines overlapping
        // words and isolated-bit corrections before emitting each bit once.
        let mut pending = [field.zero(); 32];
        let mut cursor = 0;
        for (&word, &scale) in self.words.iter().zip(&self.coefficients) {
            if scale == field.zero() {
                continue;
            }
            flush_window(&mut cursor, word.base, &mut pending, base, field, emit)?;
            accumulate_word(word, scale, &mut pending, field);
        }
        let end = cursor + 32;
        flush_window(&mut cursor, end, &mut pending, base, field, emit)
    }

    pub(super) fn emit_blocks(
        &self,
        base: usize,
        block_len: usize,
        field: &Cfg,
        emit: &mut impl FnMut(usize, &[F]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        if !block_len.is_power_of_two()
            || block_len > 1 << crate::sumcheck::inner::packed::SHA256_INNER_PREFIX_MAX_VARS
            || base % block_len != 0
        {
            return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
        }
        // Keep a whole prefix block until every overlapping word has arrived.
        // A word spans at most 27 positions and can start 15 positions into a
        // block, so a 64-field ring covers the pending interval without aliasing.
        let mut pending = [field.zero(); 64];
        let mut cursor = 0;
        for (&word, &scale) in self.words.iter().zip(&self.coefficients) {
            if scale == field.zero() {
                continue;
            }
            flush_blocks(
                &mut cursor,
                word.base & !(block_len - 1),
                block_len,
                &mut pending,
                base,
                field,
                emit,
            )?;
            accumulate_word(word, scale, &mut pending, field);
        }
        let end = cursor + pending.len();
        flush_blocks(&mut cursor, end, block_len, &mut pending, base, field, emit)
    }
}

fn accumulate_word<const W: usize>(word: Word, mut scale: F, pending: &mut [F; W], field: &Cfg) {
    for bit in 0..usize::from(word.width) {
        let (index, negative, weighted) = match word.kind {
            WordKind::Unsigned => (word.base + bit, false, true),
            WordKind::Signed => (word.base + bit, bit + 1 == usize::from(word.width), true),
            WordKind::Encoded { offset, weighted } => {
                let stream = usize::from(offset) + 11 - bit;
                (
                    word.base + 8 * (stream / 8) + 7 - stream % 8,
                    bit == 11,
                    weighted,
                )
            }
        };
        let slot = &mut pending[index & (W - 1)];
        *slot = if negative {
            field.sub(slot, &scale)
        } else {
            field.add(slot, &scale)
        };
        if weighted {
            scale = field.add(&scale, &scale);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn flush_blocks(
    cursor: &mut usize,
    end: usize,
    block_len: usize,
    pending: &mut [F; 64],
    base: usize,
    field: &Cfg,
    emit: &mut impl FnMut(usize, &[F]) -> Result<(), crate::sumcheck::SumcheckError>,
) -> Result<(), crate::sumcheck::SumcheckError> {
    for index in (*cursor..end.min(*cursor + pending.len())).step_by(block_len) {
        let start = index & (pending.len() - 1);
        let block = &mut pending[start..start + block_len];
        if block.iter().any(|value| *value != field.zero()) {
            emit(base + index, block)?;
            block.fill(field.zero());
        }
    }
    *cursor = end;
    Ok(())
}

fn flush_window(
    cursor: &mut usize,
    end: usize,
    pending: &mut [F; 32],
    base: usize,
    field: &Cfg,
    emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
) -> Result<(), crate::sumcheck::SumcheckError> {
    for index in *cursor..end.min(*cursor + 32) {
        let slot = &mut pending[index & 31];
        if *slot != field.zero() {
            emit(base + index, *slot)?;
            *slot = field.zero();
        }
    }
    *cursor = end;
    Ok(())
}

pub(super) struct PreparedWeights {
    norm: crate::poly::mle::EqualityWeights<F>,
    norm_instances: Vec<F>,
    products: crate::poly::mle::EqualityWeights<F>,
    trees: Vec<Vec<F>>,
    tree_indices: Vec<usize>,
    tree_scales: Vec<F>,
}

impl BindingForm<'_> {
    fn prepared_compact_weights(&self) -> Result<&PreparedWeights, FalconError> {
        if let Some(weights) = self.compact_weights.get() {
            return Ok(weights);
        }
        let field = self.field;
        let mut trees = Vec::new();
        let mut tree_indices = Vec::new();
        let mut tree_scales = Vec::new();
        let mut known = HashMap::new();
        let mut scale = self.eta;
        for _ in 0..8 {
            scale = field.mul(&scale, &self.eta);
        }
        for pair in &self.proof.compaction {
            for tree in [&pair.candidate, &pair.output] {
                let key: Vec<_> = tree
                    .terminal_point
                    .iter()
                    .map(|&x| canonical(x, field))
                    .collect();
                let index = match known.get(&key) {
                    Some(&index) => index,
                    None => {
                        let index = trees.len();
                        trees.push(
                            eq_table(&tree.terminal_point, field)
                                .map_err(|error| piop(error.to_string()))?,
                        );
                        known.insert(key, index);
                        index
                    }
                };
                tree_indices.push(index);
                tree_scales.push(scale);
                scale = field.mul(&scale, &self.eta);
            }
        }
        let weights = PreparedWeights {
            norm: factored_weights(&self.proof.norm.point, field)?,
            norm_instances: eq_table(&self.proof.norm.instance_point, field)
                .map_err(|error| piop(error.to_string()))?,
            products: factored_weights(&self.proof.compact_products.point, field)?,
            trees,
            tree_indices,
            tree_scales,
        };
        // Concurrent first partitions may prepare a small duplicate, but never
        // block Rayon workers while one initializer launches parallel work.
        let _ = self.compact_weights.set(weights);
        Ok(self
            .compact_weights
            .get()
            .expect("prepared binding weights"))
    }

    pub(super) fn prepared_compact_template(&self) -> Result<&CompiledTemplate, FalconError> {
        if let Some(template) = self.compact_linear.get() {
            return Ok(template);
        }
        let field = self.field;
        let mut common = WordSink::new(0, 0, field);
        emit_linear_template(
            &mut common,
            &self.local_linear_weights,
            &self.layout.offsets(),
            field,
        );
        let mut dynamic = WordSink {
            field,
            instance: 0,
            base: 0,
            accumulator: RecordWords::default(),
        };
        self.emit_compact_instance_terms(0, &mut dynamic, self.prepared_compact_weights()?)?;
        let template = CompiledTemplate::new(common.finish(), dynamic.accumulator.0);
        let _ = self.compact_linear.set(template);
        Ok(self
            .compact_linear
            .get()
            .expect("compiled binding template"))
    }

    pub(super) fn compact_instance(
        &self,
        instance: usize,
    ) -> Result<&CompiledCoefficients, FalconError> {
        let slot = &self.compact_instances[instance];
        if let Some(compiled) = slot.get() {
            return Ok(compiled);
        }
        let template = self.prepared_compact_template()?;
        let mut sink = template.sink(
            instance,
            instance * self.layout.signature_stride(),
            self.ring_instance_weights[instance],
            self.field,
        );
        self.emit_compact_instance_terms(instance, &mut sink, self.prepared_compact_weights()?)?;
        let _ = slot.set(sink.finish());
        Ok(slot.get().expect("compiled binding instance"))
    }

    fn emit_compact_instance_terms(
        &self,
        instance: usize,
        sink: &mut impl CoefficientSink,
        prepared: &PreparedWeights,
    ) -> Result<(), FalconError> {
        let field = self.field;
        let layout = self.layout;
        let offsets = layout.offsets();
        let base = instance * layout.signature_stride();
        let alpha = self.ring_instance_weights[instance];
        let ring = self.prover_ring_cache();
        for (i, weight) in ring.adjoints[ring.instance_keys[instance]]
            .iter()
            .enumerate()
        {
            add_signed_source_scaled(sink, base, &offsets, i, field.mul(&alpha, weight), field);
        }
        let mut ignored = field.zero();
        let mut scale = self.eta;
        add_norm_claims_prepared(
            sink,
            &mut ignored,
            &mut scale,
            self.eta,
            layout,
            self.proof,
            field,
            &prepared.norm,
            &prepared.norm_instances,
        )?;
        add_compaction_product_claims_prepared(
            sink,
            &mut ignored,
            &mut scale,
            self.eta,
            layout,
            self.proof,
            field,
            &prepared.products,
        )?;
        for side in 0..2 {
            let index = 2 * instance + side;
            let weights = &prepared.trees[prepared.tree_indices[index]];
            let scale = prepared.tree_scales[index];
            for (i, &weight) in
                weights
                    .iter()
                    .enumerate()
                    .take(if side == 0 { HASH_TO_POINT_SAMPLES } else { N })
            {
                let weight = field.mul(&scale, &weight);
                if side == 0 {
                    sink.add(
                        base + offsets.compact_selectors + i,
                        field.mul(
                            &weight,
                            &field.sub(&self.proof.compaction_gamma, &field.one()),
                        ),
                    );
                    sink.add_word(
                        base + offsets.compact_selected_prefixes + 11 * i,
                        11,
                        false,
                        field.mul(&weight, &self.proof.compaction_rank_scale),
                        field,
                    );
                    sink.add_word(
                        base + offsets.compact_selected_remainders + 14 * i,
                        14,
                        false,
                        weight,
                        field,
                    );
                } else {
                    sink.add_word(base + offsets.hash_point + 14 * i, 14, false, weight, field);
                }
            }
        }
        Ok(())
    }

    pub(super) fn compact_target(&self) -> Result<F, FalconError> {
        let field = self.field;
        let proof = self.proof;
        let batch_vars = self.layout.capacity().ilog2() as usize;
        if proof.norm.point.len() != 10 + batch_vars
            || proof.norm.instance_point.len() != batch_vars
            || proof.compact_products.point.len() != 13 + batch_vars
            || proof.compaction.len() != self.layout.batch()
            || proof.compaction.iter().any(|pair| {
                [&pair.candidate, &pair.output]
                    .iter()
                    .any(|tree| tree.terminal_point.len() != 11)
            })
            || proof.keccak_chi.is_some()
        {
            return Err(piop("hybrid binding terminal dimensions mismatch"));
        }
        let mut constant = linear_constant(&self.local_linear_point, field);
        let instance_sum = self
            .ring_instance_weights
            .iter()
            .fold(field.zero(), |sum, weight| field.add(&sum, weight));
        constant = field.mul(&constant, &instance_sum);
        for ((message, signature), alpha) in self
            .statement
            .messages
            .iter()
            .zip(&self.statement.signatures)
            .zip(&self.ring_instance_weights)
        {
            let mut bits = field.zero();
            for bit in 0..256 {
                if (message[bit / 8] >> (bit % 8)) & 1 == 1 {
                    bits = field.add(&bits, &self.local_linear_weights.at(1 + bit));
                }
            }
            constant = field.sub(&constant, &field.mul(alpha, &bits));
            let encoded = super::super::encode_signature_ct(signature)?;
            let mut signature_bytes = field.zero();
            // The common header constant is already in linear_constant().
            for (byte, &value) in encoded.iter().enumerate().skip(1) {
                signature_bytes = field.add(
                    &signature_bytes,
                    &mul_i(
                        self.local_linear_weights.at(1 + 256 + byte),
                        i128::from(value),
                        field,
                    ),
                );
            }
            constant = field.sub(&constant, &field.mul(alpha, &signature_bytes));
        }
        let mut target = field.sub(&field.zero(), &constant);
        let norm_instances =
            eq_table(&proof.norm.point[10..], field).map_err(|error| piop(error.to_string()))?;
        let norm_outer =
            eq_table(&proof.norm.instance_point, field).map_err(|error| piop(error.to_string()))?;
        let mut norm_sum = field.zero();
        let mut weighted_norm_sum = field.zero();
        for (left, right) in norm_instances
            .iter()
            .zip(&norm_outer)
            .take(self.layout.batch())
        {
            norm_sum = field.add(&norm_sum, left);
            weighted_norm_sum = field.add(&weighted_norm_sum, &field.mul(left, right));
        }
        let mut scale = self.eta;
        for side in 0..2 {
            for (coordinate, sum) in [weighted_norm_sum, norm_sum].into_iter().enumerate() {
                let constant = if side == 0 {
                    mul_i(field.mul(&scale, &sum), -6_144, field)
                } else {
                    field.zero()
                };
                add_claim_target(
                    &mut target,
                    scale,
                    proof.norm.terminal[side][coordinate],
                    constant,
                    field,
                );
                scale = field.mul(&scale, &self.eta);
            }
        }
        add_claim_target(&mut target, scale, proof.norm.slack, field.zero(), field);
        scale = field.mul(&scale, &self.eta);
        let point = &proof.compact_products.point;
        let mut complement = field.mul(
            &eq_prefix_sum(&point[..11], HASH_TO_POINT_SAMPLES, field),
            &eq_prefix_sum(&point[11..11 + batch_vars], self.layout.batch(), field),
        );
        for coordinate in &point[11 + batch_vars..] {
            complement = field.mul(&complement, &field.sub(&field.one(), coordinate));
        }
        let claims = [
            proof.compact_products.terminal.ax,
            proof.compact_products.terminal.bx,
            proof.compact_products.terminal.cx,
        ];
        for (coordinate, claim) in claims.into_iter().enumerate() {
            let constant = if coordinate < 2 {
                field.mul(&scale, &complement)
            } else {
                field.zero()
            };
            add_claim_target(&mut target, scale, claim, constant, field);
            scale = field.mul(&scale, &self.eta);
        }
        for pair in &proof.compaction {
            add_claim_target(
                &mut target,
                scale,
                pair.candidate.terminal_claim,
                scale,
                field,
            );
            scale = field.mul(&scale, &self.eta);
            let point = &pair.output.terminal_point;
            let rank = point[..10]
                .iter()
                .enumerate()
                .fold(field.zero(), |sum, (bit, &coordinate)| {
                    field.add(&sum, &mul_i(coordinate, 1 << bit, field))
                });
            let lower = field.add(
                &proof.compaction_gamma,
                &field.mul(&proof.compaction_rank_scale, &rank),
            );
            let constant = field.mul(
                &scale,
                &field.add(
                    &field.mul(&field.sub(&field.one(), &point[10]), &lower),
                    &point[10],
                ),
            );
            add_claim_target(
                &mut target,
                scale,
                pair.output.terminal_claim,
                constant,
                field,
            );
            scale = field.mul(&scale, &self.eta);
        }
        Ok(target)
    }
}

fn interval_sum(point: &[F], start: usize, count: usize, field: &Cfg) -> F {
    field.sub(
        &eq_prefix_sum(point, start + count, field),
        &eq_prefix_sum(point, start, field),
    )
}

fn strided_sum(point: &[F], start: usize, count: usize, field: &Cfg) -> F {
    let mut low = field.one();
    for (bit, coordinate) in point[..2].iter().enumerate() {
        low = field.mul(
            &low,
            &if (start >> bit) & 1 == 1 {
                *coordinate
            } else {
                field.sub(&field.one(), coordinate)
            },
        );
    }
    field.mul(&low, &interval_sum(&point[2..], start >> 2, count, field))
}

/// Constants of the repeated hybrid linear template, excluding public messages
/// and the instance-specific nonce and signature coefficient bytes.
fn linear_constant(point: &[F], field: &Cfg) -> F {
    let mut constant = field.sub(&field.zero(), &interval_sum(point, 0, 1, field));
    let mut row = 1 + 256;
    constant = field.sub(
        &constant,
        &mul_i(interval_sum(point, row, 1, field), 0x5a, field),
    );
    row += super::super::CT_SIGNATURE_BYTES + N;
    for (offset, value) in [(1, -12_288), (2, -5), (3, -1)] {
        constant = field.add(
            &constant,
            &mul_i(
                strided_sum(point, row + offset, HASH_TO_POINT_SAMPLES, field),
                value,
                field,
            ),
        );
    }
    row += 4 * HASH_TO_POINT_SAMPLES;
    constant = field.sub(&constant, &interval_sum(point, row + 1, 1, field));
    row += 2;
    constant = field.add(
        &constant,
        &mul_i(interval_sum(point, row, N, field), -12_288, field),
    );
    row += N;
    field.add(
        &constant,
        &mul_i(interval_sum(point, row, N, field), 6_144, field),
    )
}

fn emit_linear_template(
    sink: &mut impl CoefficientSink,
    weights: &crate::poly::mle::EqualityWeights<F>,
    offsets: &FalconSourceOffsets,
    field: &Cfg,
) {
    let mut row = 0;
    let mut next = || {
        let weight = weights.at(row);
        row += 1;
        weight
    };
    sink.add(offsets.shared_one, next());
    for bit in 0..256 {
        sink.add(offsets.message + bit, next());
    }
    for byte in 0..super::super::CT_SIGNATURE_BYTES {
        sink.add_word(
            offsets.encoded_signature + 8 * byte,
            8,
            false,
            next(),
            field,
        );
    }
    for i in 0..N {
        let weight = next();
        let stream = 12 * i;
        sink.add_encoded_word(
            offsets.encoded_signature + 8 * (1 + super::super::NONCE_BYTES + stream / 8),
            stream % 8,
            false,
            weight,
            field,
        );
        sink.add_word(
            offsets.s2_non_min_slack + 4 * i,
            4,
            false,
            field.sub(&field.zero(), &weight),
            field,
        );
    }
    for i in 0..HASH_TO_POINT_SAMPLES {
        let weight = next();
        sink.add_word(offsets.hash_words + 16 * i, 16, false, weight, field);
        sink.add_word(
            offsets.hash_quotients + 3 * i,
            3,
            false,
            mul_i(weight, -i128::from(Q), field),
            field,
        );
        sink.add_word(
            offsets.hash_remainders + 14 * i,
            14,
            false,
            field.sub(&field.zero(), &weight),
            field,
        );
        let weight = next();
        sink.add_word(offsets.hash_remainders + 14 * i, 14, false, weight, field);
        sink.add_word(
            offsets.hash_remainder_slack + 14 * i,
            14,
            false,
            weight,
            field,
        );
        let weight = next();
        sink.add_word(offsets.hash_quotients + 3 * i, 3, false, weight, field);
        sink.add_word(offsets.hash_quotient_slack + 3 * i, 3, false, weight, field);
        let weight = next();
        sink.add_word(
            offsets.hash_prefixes + 11 * (i + 1),
            11,
            false,
            weight,
            field,
        );
        sink.add_word(
            offsets.hash_prefixes + 11 * i,
            11,
            false,
            field.sub(&field.zero(), &weight),
            field,
        );
        sink.add(offsets.hash_accept_ands + i, weight);
    }
    sink.add_word(offsets.hash_prefixes, 11, false, next(), field);
    sink.add(
        offsets.hash_prefixes + 11 * HASH_TO_POINT_SAMPLES + 10,
        next(),
    );
    for i in 0..N {
        let weight = next();
        sink.add_word(offsets.s1 + 14 * i, 14, false, weight, field);
        sink.add_word(offsets.s1_range_slack + 14 * i, 14, false, weight, field);
    }
    for i in 0..N {
        let weight = next();
        sink.add_word(offsets.hash_point + 14 * i, 14, false, weight, field);
        sink.add_word(
            offsets.s1 + 14 * i,
            14,
            false,
            field.sub(&field.zero(), &weight),
            field,
        );
        sink.add_word(
            offsets.ring_quotients + 23 * i,
            23,
            true,
            mul_i(weight, -i128::from(Q), field),
            field,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlapping_words(sink: &mut impl CoefficientSink, base: usize, field: &Cfg) {
        overlapping_words_scaled(sink, base, unsigned(37, field), field);
    }

    fn overlapping_words_scaled(
        sink: &mut impl CoefficientSink,
        base: usize,
        scale: F,
        field: &Cfg,
    ) {
        // Signed and ordinary words cross every sliding-window/block boundary.
        for local in [0, 15, 31, 63, 127, 224] {
            sink.add_word(base + local, 27, true, scale, field);
            sink.add_word(base + local + 1, 14, false, scale, field);
            sink.add(base + local + 26, field.neg(&scale));
        }
        // CT encoding has a reversed bit order and alternates nibble offsets.
        for (local, offset) in [(8, 0), (24, 4), (56, 4), (120, 0), (224, 4)] {
            sink.add_encoded_word(base + local, offset, true, scale, field);
            sink.add_encoded_word(base + local, offset, false, scale, field);
        }
        sink.add_word(base + 192, 8, false, scale, field);
        sink.add_word(base + 192, 8, false, field.neg(&scale), field);
    }

    struct Dense<'a> {
        values: Vec<F>,
        field: &'a Cfg,
    }

    impl CoefficientSink for Dense<'_> {
        fn add(&mut self, index: usize, value: F) {
            self.values[index] = self.field.add(&self.values[index], &value);
        }
    }

    #[test]
    fn compact_blocks_match_bit_stream_and_dense_signed_word_expansion() {
        let field = crate::piop::spartan::bitz::spartan_bitz_field_config();
        for base in [0, 256] {
            let mut sink = WordSink::new(base / 256, base, &field);
            overlapping_words(&mut sink, base, &field);
            let compact = sink.finish();
            let mut expected = Dense {
                values: vec![field.zero(); 512],
                field: &field,
            };
            overlapping_words(&mut expected, base, &field);
            let mut generic = vec![field.zero(); 512];
            compact
                .emit(base, &field, &mut |i, value| {
                    generic[i] = field.add(&generic[i], &value);
                    Ok(())
                })
                .unwrap();
            assert_eq!(generic, expected.values);
            for width in [1, 2, 4, 8, 16] {
                let mut actual = vec![field.zero(); 512];
                let mut next = base;
                compact
                    .emit_blocks(base, width, &field, &mut |start, block| {
                        assert!(start >= next);
                        assert_eq!(start % width, 0);
                        assert_eq!(block.len(), width);
                        assert!(start + width <= base + 256);
                        assert!(block.iter().any(|value| *value != field.zero()));
                        actual[start..start + width].copy_from_slice(block);
                        next = start + width;
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(actual, expected.values, "base={base}, width={width}");
            }
        }
    }

    #[test]
    fn indexed_word_template_matches_dense_expansion_across_instances_and_zero_scales() {
        let field = crate::piop::spartan::bitz::spartan_bitz_field_config();
        let mut common = WordSink::new(0, 0, &field);
        overlapping_words(&mut common, 0, &field);
        let mut record = WordSink {
            field: &field,
            instance: 0,
            base: 0,
            accumulator: RecordWords::default(),
        };
        // Build with all-zero dynamic scales, then replay nonzero instances.
        // Slots cannot be dropped just because this first instance is zero.
        overlapping_words_scaled(&mut record, 0, field.zero(), &field);
        record.add_word(300, 8, false, field.zero(), &field);
        let template = CompiledTemplate::new(common.finish(), record.accumulator.0);
        for (instance, alpha, dynamic_scale) in [
            (0, field.zero(), field.zero()),
            (1, field.zero(), unsigned(91, &field)),
            (0, field.one(), field.neg(&unsigned(37, &field))),
            (1, unsigned(23, &field), unsigned(91, &field)),
        ] {
            let base = instance * 512;
            let mut indexed = template.sink(instance, base, alpha, &field);
            overlapping_words_scaled(&mut indexed, base, dynamic_scale, &field);
            indexed.add_word(base + 300, 8, false, dynamic_scale, &field);
            let compact = indexed.finish();
            assert!(std::sync::Arc::ptr_eq(&compact.words, &template.words));
            let mut expected = Dense {
                values: vec![field.zero(); 1024],
                field: &field,
            };
            overlapping_words_scaled(
                &mut expected,
                base,
                field.mul(&alpha, &unsigned(37, &field)),
                &field,
            );
            overlapping_words_scaled(&mut expected, base, dynamic_scale, &field);
            expected.add_word(base + 300, 8, false, dynamic_scale, &field);
            let mut actual = vec![field.zero(); 1024];
            compact
                .emit(base, &field, &mut |index, value| {
                    actual[index] = value;
                    Ok(())
                })
                .unwrap();
            assert_eq!(actual, expected.values);
            for width in [1, 2, 4, 8, 16] {
                actual.fill(field.zero());
                compact
                    .emit_blocks(base, width, &field, &mut |start, values| {
                        actual[start..start + width].copy_from_slice(values);
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(
                    actual, expected.values,
                    "instance={instance}, width={width}"
                );
            }
        }
    }

    #[test]
    fn compact_empty_blocks_and_invalid_block_dimensions() {
        let field = crate::piop::spartan::bitz::spartan_bitz_field_config();
        let compact = WordSink::new(0, 0, &field).finish();
        for width in [1, 2, 4, 8, 16] {
            compact
                .emit_blocks(256, width, &field, &mut |_, _| {
                    panic!("empty compact source emitted a block")
                })
                .unwrap();
        }
        for (base, width) in [(0, 0), (0, 3), (0, 32), (1, 16)] {
            assert!(
                compact
                    .emit_blocks(base, width, &field, &mut |_, _| Ok(()))
                    .is_err()
            );
        }
    }
}
