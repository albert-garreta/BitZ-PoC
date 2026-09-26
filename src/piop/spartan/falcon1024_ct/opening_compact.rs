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
    words: Vec<(Word, F)>,
}

struct WordSink<'a> {
    field: &'a Cfg,
    instance: usize,
    base: usize,
    words: HashMap<Word, F>,
}

impl<'a> WordSink<'a> {
    fn new(instance: usize, base: usize, field: &'a Cfg) -> Self {
        Self {
            field,
            instance,
            base,
            words: HashMap::new(),
        }
    }

    fn word(&mut self, base: usize, width: usize, kind: WordKind, value: F) {
        let key = Word {
            base: base - self.base,
            width: width as u8,
            kind,
        };
        let slot = self.words.entry(key).or_insert_with(|| self.field.zero());
        *slot = self.field.add(slot, &value);
    }

    fn finish(self) -> CompiledCoefficients {
        let mut words: Vec<_> = self
            .words
            .into_iter()
            .filter(|(_, value)| *value != self.field.zero())
            .collect();
        words.sort_unstable_by_key(|(word, _)| *word);
        CompiledCoefficients { words }
    }
}

impl CoefficientSink for WordSink<'_> {
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
        for &(word, mut scale) in &self.words {
            flush_window(&mut cursor, word.base, &mut pending, base, field, emit)?;
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
                let slot = &mut pending[index & 31];
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
        let end = cursor + 32;
        flush_window(&mut cursor, end, &mut pending, base, field, emit)
    }
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

    pub(super) fn compact_instance(
        &self,
        instance: usize,
    ) -> Result<&CompiledCoefficients, FalconError> {
        let slot = &self.compact_instances[instance];
        if let Some(compiled) = slot.get() {
            return Ok(compiled);
        }
        let field = self.field;
        let layout = self.layout;
        let offsets = layout.offsets();
        let base = instance * layout.signature_stride();
        let common = self.compact_linear.get_or_init(|| {
            let mut sink = WordSink::new(0, 0, field);
            emit_linear_template(&mut sink, &self.local_linear_weights, &offsets, field);
            sink.finish()
        });
        let mut sink = WordSink::new(instance, base, field);
        let alpha = self.ring_instance_weights[instance];
        sink.words.reserve(common.words.len());
        for &(word, value) in &common.words {
            sink.words.insert(word, field.mul(&alpha, &value));
        }
        let ring = self.prover_ring_cache();
        for (i, weight) in ring.adjoints[ring.instance_keys[instance]]
            .iter()
            .enumerate()
        {
            add_signed_source_scaled(
                &mut sink,
                base,
                &offsets,
                i,
                field.mul(&alpha, weight),
                field,
            );
        }
        let prepared = self.prepared_compact_weights()?;
        let mut ignored = field.zero();
        let mut scale = self.eta;
        add_norm_claims_prepared(
            &mut sink,
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
            &mut sink,
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
        let _ = slot.set(sink.finish());
        Ok(slot.get().expect("compiled binding instance"))
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
        for (message, alpha) in self
            .statement
            .messages
            .iter()
            .zip(&self.ring_instance_weights)
        {
            let mut bits = field.zero();
            for bit in 0..256 {
                if (message[bit / 8] >> (bit % 8)) & 1 == 1 {
                    bits = field.add(&bits, &self.local_linear_weights.at(1 + bit));
                }
            }
            constant = field.sub(&constant, &field.mul(alpha, &bits));
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

/// Constants of the repeated hybrid linear template, excluding public messages.
fn linear_constant(point: &[F], field: &Cfg) -> F {
    let mut constant = field.sub(&field.zero(), &interval_sum(point, 0, 1, field));
    let mut row = 1 + 256;
    for bit in 0..8 {
        if (0x5au8 >> bit) & 1 == 1 {
            constant = field.sub(&constant, &interval_sum(point, row + bit, 1, field));
        }
    }
    row += 8 + N;
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
    for bit in 0..8 {
        sink.add(offsets.encoded_signature + bit, next());
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
