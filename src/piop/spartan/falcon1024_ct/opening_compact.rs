//! Word-sized storage for the hybrid binder. The public operator is compiled
//! once and replayed in source order; no dense field table of source bits is kept.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum WordKind {
    Unsigned,
    Encoded { offset: u8 },
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
    folded: std::sync::Arc<std::sync::OnceLock<FoldedWords>>,
    byte_runs: std::sync::Arc<std::sync::OnceLock<ByteRuns>>,
}

/// Each run occupies one physical byte and has coefficients a, 2a, 4a, ... .
/// Encoded signed words split at both byte boundaries and their sign bit.
struct ByteRun {
    base: usize,
    lane: u8,
    len: u8,
    shift: u8,
    negative: bool,
}

struct ByteRuns {
    starts: Vec<usize>,
    runs: Vec<ByteRun>,
}

impl ByteRuns {
    fn new(words: &[Word]) -> Self {
        let mut starts = Vec::with_capacity(words.len() + 1);
        let mut runs = Vec::new();
        for &word in words {
            starts.push(runs.len());
            let mut bit = 0;
            while bit < usize::from(word.width) {
                let (index, negative) = word.bit_position(bit);
                let mut len = 1;
                while bit + len < usize::from(word.width)
                    && index % 8 + len < 8
                    && word.bit_position(bit + len) == (index + len, negative)
                {
                    len += 1;
                }
                runs.push(ByteRun {
                    base: index & !7,
                    lane: (index % 8) as u8,
                    len: len as u8,
                    shift: bit as u8,
                    negative,
                });
                bit += len;
            }
        }
        starts.push(runs.len());
        Self { starts, runs }
    }
}

/// Binding low variables is linear in each word's coefficient. The physical
/// bit-to-word map is shared across signatures, so compile its weighted image
/// once, then replay whole-word contributions instead of expanding every bit.
struct FoldedWords {
    weights: Vec<F>,
    starts: Vec<usize>,
    terms: Vec<(usize, F)>,
}

impl FoldedWords {
    fn new(words: &[Word], weights: &[F], field: &Cfg) -> Self {
        let vars = weights.len().ilog2() as usize;
        let mut starts = Vec::with_capacity(words.len() + 1);
        let mut terms = Vec::new();
        for &word in words {
            starts.push(terms.len());
            let base = word.base >> vars;
            let mut sums = [field.zero(); 32];
            let mut power = field.one();
            for bit in 0..usize::from(word.width) {
                let (index, negative) = word.bit_position(bit);
                let weight = field.mul(&power, &weights[index & (weights.len() - 1)]);
                let slot = &mut sums[(index >> vars) - base];
                *slot = if negative {
                    field.sub(slot, &weight)
                } else {
                    field.add(slot, &weight)
                };
                power = field.add(&power, &power);
            }
            terms.extend(sums.into_iter().enumerate().filter_map(|(offset, value)| {
                (value != field.zero()).then_some((base + offset, value))
            }));
        }
        starts.push(terms.len());
        Self {
            weights: weights.to_vec(),
            starts,
            terms,
        }
    }
}

impl Word {
    fn bit_position(self, bit: usize) -> (usize, bool) {
        match self.kind {
            WordKind::Unsigned => (self.base + bit, false),
            WordKind::Encoded { offset } => {
                let stream = usize::from(offset) + 11 - bit;
                (self.base + 8 * (stream / 8) + 7 - stream % 8, bit == 11)
            }
        }
    }
}

/// All instances share the public operator and the same dynamic addition order.
/// Compile their word slots once, then accumulate each instance by slot index.
/// Word descriptors are also shared by the cached instance coefficient vectors.
pub(super) struct CompiledTemplate {
    words: std::sync::Arc<[Word]>,
    common: Vec<(usize, F)>,
    dynamic_slots: Vec<usize>,
    folded: std::sync::Arc<std::sync::OnceLock<FoldedWords>>,
    byte_runs: std::sync::Arc<std::sync::OnceLock<ByteRuns>>,
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
            folded: Default::default(),
            byte_runs: Default::default(),
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
            folded: Default::default(),
            byte_runs: Default::default(),
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
            folded: std::sync::Arc::clone(&self.accumulator.template.folded),
            byte_runs: std::sync::Arc::clone(&self.accumulator.template.byte_runs),
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

    fn add_word(&mut self, base: usize, width: usize, scale: F, _: &Cfg) {
        self.word(base, width, WordKind::Unsigned, scale);
    }

    fn add_encoded_word(&mut self, base: usize, offset: usize, scale: F, _: &Cfg) {
        self.word(
            base,
            12,
            WordKind::Encoded {
                offset: offset as u8,
            },
            scale,
        );
    }
}

impl CompiledCoefficients {
    /// Group word contributions directly by witness byte, without expanding
    /// every coefficient bit. A geometric run starting at lane i contributes
    /// +a at i and -2^len*a at its exclusive end to a difference array. The
    /// recurrence value[j] = 2*value[j-1] + difference[j] reconstructs it.
    /// Linearity makes this exact for overlapping words and signed corrections.
    pub(super) fn emit_byte_buckets(
        &self,
        base: usize,
        field: &Cfg,
        read_byte: &mut impl FnMut(usize, usize) -> Result<u8, crate::sumcheck::SumcheckError>,
        emit: &mut impl FnMut(u8, &[F; 8]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        use crate::sumcheck::SumcheckError;
        use field::{CtOrd, Reduce};
        if base % 8 != 0 {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let compiled = self.byte_runs.get_or_init(|| ByteRuns::new(&self.words));
        // Each accumulator receives fewer than 2^64 field-by-u64 products;
        // native coefficient widths are at most 27 bits. The extra accumulator
        // limb therefore preserves exact sums for every supported batch.
        let mut differences = vec![field::FpLinearAcc::<2, 1>::zero(); 256 * 8];
        let mut occupied = [false; 256];
        for (word, &scale) in self.coefficients.iter().enumerate() {
            if !scale
                .as_montgomery_integer()
                .ct_lt(field.modulus())
                .declassify()
            {
                return Err(SumcheckError::NonCanonicalFieldElement);
            }
            if scale == field.zero() {
                continue;
            }
            let negative = field.neg(&scale);
            for run in &compiled.runs[compiled.starts[word]..compiled.starts[word + 1]] {
                let pattern =
                    usize::from(read_byte(base + run.base, usize::from(run.lane + run.len))?);
                if pattern == 0 {
                    continue;
                }
                occupied[pattern] = true;
                let values = &mut differences[8 * pattern..][..8];
                let lane = usize::from(run.lane);
                let len = usize::from(run.len);
                let (positive, negative) = if run.negative {
                    (&negative, &scale)
                } else {
                    (&scale, &negative)
                };
                field.mul_acc(&mut values[lane], positive, &(1u64 << run.shift));
                if lane + len < 8 {
                    field.mul_acc(
                        &mut values[lane + len],
                        negative,
                        &(1u64 << (usize::from(run.shift) + len)),
                    );
                }
            }
        }
        for (pattern, used) in occupied.into_iter().enumerate() {
            if used {
                let mut values = [field.zero(); 8];
                let mut previous = field.zero();
                for (value, &difference) in values.iter_mut().zip(&differences[8 * pattern..][..8])
                {
                    let difference = field.reduce(difference);
                    previous = field.add(&field.add(&previous, &previous), &difference);
                    *value = previous;
                }
                emit(pattern as u8, &values)?;
            }
        }
        Ok(())
    }

    /// Emit each folded coefficient's final sum once in increasing index order.
    pub(super) fn emit_folded(
        &self,
        base: usize,
        weights: &[F],
        field: &Cfg,
        emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        if !weights.len().is_power_of_two()
            || weights.len() > 1 << crate::sumcheck::inner::packed::SHA256_INNER_PREFIX_MAX_VARS
            || base % weights.len() != 0
        {
            return Err(crate::sumcheck::SumcheckError::InvalidProductDimensions);
        }
        // Initialization does no Rayon work: workers can share one compiled
        // map without nested parallel initialization or per-instance copies.
        let cached = self
            .folded
            .get_or_init(|| FoldedWords::new(&self.words, weights, field));
        let alternative;
        let folded = if cached.weights == weights {
            cached
        } else {
            // Reference tests may reuse a source with another prefix. A source
            // belongs to one proving invocation in the production binder.
            alternative = FoldedWords::new(&self.words, weights, field);
            &alternative
        };
        type Accumulator = <Cfg as BatchMulAcc<F>>::Accumulator;
        let base = base / weights.len();
        let vars = weights.len().ilog2() as usize;
        let mut pending = std::array::from_fn(|_| Accumulator::zero());
        let mut occupied = [false; 32];
        let mut cursor = 0;
        for (word, scale) in self.coefficients.iter().enumerate() {
            if *scale == field.zero() {
                continue;
            }
            flush_folded(
                &mut cursor,
                self.words[word].base >> vars,
                &mut pending,
                &mut occupied,
                base,
                field,
                emit,
            )?;
            for &(index, weight) in &folded.terms[folded.starts[word]..folded.starts[word + 1]] {
                field.mul_acc(&mut pending[index & 31], scale, &weight);
                occupied[index & 31] = true;
            }
        }
        let end = cursor + 32;
        flush_folded(
            &mut cursor,
            end,
            &mut pending,
            &mut occupied,
            base,
            field,
            emit,
        )
    }

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

#[allow(clippy::too_many_arguments)]
fn flush_folded(
    cursor: &mut usize,
    end: usize,
    pending: &mut [<Cfg as BatchMulAcc<F>>::Accumulator; 32],
    occupied: &mut [bool; 32],
    base: usize,
    field: &Cfg,
    emit: &mut impl FnMut(usize, F) -> Result<(), crate::sumcheck::SumcheckError>,
) -> Result<(), crate::sumcheck::SumcheckError> {
    use field::Reduce;
    for index in *cursor..end.min(*cursor + 32) {
        let slot = index & 31;
        if occupied[slot] {
            let sum = std::mem::replace(
                &mut pending[slot],
                <Cfg as BatchMulAcc<F>>::Accumulator::zero(),
            );
            emit(base + index, field.reduce(sum))?;
            occupied[slot] = false;
        }
    }
    *cursor = end;
    Ok(())
}

fn accumulate_word<const W: usize>(word: Word, mut scale: F, pending: &mut [F; W], field: &Cfg) {
    for bit in 0..usize::from(word.width) {
        let (index, negative) = word.bit_position(bit);
        let slot = &mut pending[index & (W - 1)];
        *slot = if negative {
            field.sub(slot, &scale)
        } else {
            field.add(slot, &scale)
        };
        scale = field.add(&scale, &scale);
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
    leaf: native::LeafWeights,
    tree_scales: Vec<F>,
}

impl BindingForm<'_> {
    fn prepared_compact_weights(&self) -> Result<&PreparedWeights, FalconError> {
        if let Some(weights) = self.compact_weights.get() {
            return Ok(weights);
        }
        let field = self.field;
        let mut tree_scales = Vec::new();
        let mut scale = self.eta;
        for _ in 0..11 {
            scale = field.mul(&scale, &self.eta);
        }
        for _ in self.proof.compaction {
            tree_scales.push(scale);
            scale = field.mul(&scale, &self.eta);
        }
        let weights = PreparedWeights {
            norm: factored_weights(&self.proof.norm.point, field)?,
            norm_instances: eq_table(&self.proof.norm.instance_point, field)
                .map_err(|error| piop(error.to_string()))?,
            products: factored_weights(&self.proof.compact_products.point, field)?,
            leaf: native::LeafWeights::new(self.layout, self.proof, field)?,
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
        if let Some(offset) = self.layout.public_key_offset() {
            for j in 0..N {
                common.add_word(
                    offset + 14 * j,
                    14,
                    self.local_linear_weights.at(4458 + j),
                    field,
                );
            }
        }
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
            self.linear_instance_weights[instance],
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
        native::add_leaf_claims_prepared(
            sink,
            &mut ignored,
            &mut scale,
            self.eta,
            layout,
            self.proof,
            field,
            &prepared.leaf,
        )?;
        let weights = &prepared.leaf.forest;
        let tree_scale = prepared.tree_scales[instance];
        for (i, &weight) in weights.iter().take(N).enumerate() {
            sink.add_word(
                base + offsets.hash_point + 14 * i,
                14,
                field.mul(&tree_scale, &weight),
                field,
            );
        }
        let native_scale = field.mul(prepared.tree_scales.last().expect("live batch"), &self.eta);
        self.add_native_claim(sink, &mut ignored, native_scale)?;
        Ok(())
    }

    pub(super) fn evaluate_compact(&self, point: &[F]) -> Result<F, FalconError> {
        let field = self.field;
        let local_vars = self.layout.signature_stride().ilog2() as usize;
        let local = eq_table(&point[..local_vars], field).map_err(|e| piop(e.to_string()))?;
        let instances = eq_table(&point[local_vars..], field).map_err(|e| piop(e.to_string()))?;
        self.evaluate_compact_weights(&local, &instances)
    }

    pub(super) fn evaluate_compact_weights(
        &self,
        local: &[F],
        instances: &[F],
    ) -> Result<F, FalconError> {
        use field::Reduce;
        let field = self.field;
        let template = self.prepared_compact_template()?;
        let endpoints: Vec<F> = template
            .words
            .iter()
            .map(|&word| {
                let mut sum = field.zero();
                let mut power = field.one();
                for bit in 0..usize::from(word.width) {
                    let (index, negative) = word.bit_position(bit);
                    let value = field.mul(&power, &local[index]);
                    sum = if negative {
                        field.sub(&sum, &value)
                    } else {
                        field.add(&sum, &value)
                    };
                    power = field.add(&power, &power);
                }
                sum
            })
            .collect();
        let values: Result<Vec<F>, FalconError> =
            crate::utils::cfg_into_iter!(0..self.layout.batch())
                .map(|s| {
                    let coefficients = self.compact_instance(s)?;
                    let mut sum = <Cfg as BatchMulAcc<F>>::Accumulator::zero();
                    for (coefficient, endpoint) in coefficients.coefficients.iter().zip(&endpoints)
                    {
                        field.mul_acc(&mut sum, coefficient, endpoint);
                    }
                    let value: F = field.reduce(sum);
                    Ok(field.mul(&instances[s], &value))
                })
                .collect();
        Ok(values?.iter().fold(field.zero(), |s, v| field.add(&s, v)))
    }

    pub(super) fn compact_target(&self) -> Result<F, FalconError> {
        let field = self.field;
        let proof = self.proof;
        let batch_vars = self.layout.capacity().ilog2() as usize;
        if proof.norm.point.len() != 10 + batch_vars
            || proof.norm.instance_point.len() != batch_vars
            || proof.compact_products.point.len() != 11 + batch_vars
            || proof.compaction.len() != self.layout.batch()
            || proof.compaction.iter().any(|pair| {
                [&pair.candidate, &pair.output]
                    .iter()
                    .any(|tree| tree.terminal_point.len() != 11)
            })
        {
            return Err(piop("hybrid binding terminal dimensions mismatch"));
        }
        let mut constant = linear_constant(&self.local_linear_point, field);
        let instance_sum = self
            .linear_instance_weights
            .iter()
            .fold(field.zero(), |sum, weight| field.add(&sum, weight));
        constant = field.mul(&constant, &instance_sum);
        let public_constants: Result<Vec<F>, FalconError> =
            crate::utils::cfg_into_iter!(0..self.layout.batch())
                .map(|s| {
                    let message = &self.statement.messages[s];
                    let alpha = &self.linear_instance_weights[s];
                    let mut bits = field.zero();
                    for bit in 0..256 {
                        if (message[bit / 8] >> (bit % 8)) & 1 == 1 {
                            bits = field.add(&bits, &self.local_linear_weights.at(1 + bit));
                        }
                    }
                    let encoded = super::super::encode_signature_ct(&self.statement.signatures[s])?;
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
                    if self.layout.is_shared_prime() {
                        for (j, &value) in self.statement.public_keys[s].h.iter().enumerate() {
                            signature_bytes = field.add(
                                &signature_bytes,
                                &mul_i(
                                    self.local_linear_weights.at(4458 + j),
                                    i128::from(value),
                                    field,
                                ),
                            );
                        }
                    }
                    Ok(field.mul(alpha, &field.add(&bits, &signature_bytes)))
                })
                .collect();
        for contribution in public_constants? {
            constant = field.sub(&constant, &contribution);
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
        for claim in [
            proof.compact_products.terminal.ax,
            proof.compact_products.terminal.bx,
            proof.compact_products.terminal.cx,
        ] {
            add_claim_target(&mut target, scale, claim, field.zero(), field);
            scale = field.mul(&scale, &self.eta);
        }
        let leaf = &proof.compaction_leaf;
        let weights = &self.prepared_compact_weights()?.leaf;
        let local_sum = weights.local[..HASH_TO_POINT_SAMPLES]
            .iter()
            .fold(field.zero(), |s, w| field.add(&s, w));
        let instance_sum = weights.instances[..self.layout.batch()]
            .iter()
            .fold(field.zero(), |s, w| field.add(&s, w));
        let ordinary_constant = field.mul(&local_sum, &instance_sum);
        let local_weighted = weights
            .local
            .iter()
            .zip(&weights.forest)
            .take(HASH_TO_POINT_SAMPLES)
            .fold(field.zero(), |s, (a, b)| field.add(&s, &field.mul(a, b)));
        let instance_weighted = weights
            .instances
            .iter()
            .zip(&weights.beta)
            .take(self.layout.batch())
            .fold(field.zero(), |s, (a, b)| field.add(&s, &field.mul(a, b)));
        let third_constant = field.mul(
            &field.mul(&local_weighted, &instance_weighted),
            &field.sub(&proof.compaction_gamma, &field.one()),
        );
        for (claim, c) in
            leaf.terminal
                .iter()
                .zip([ordinary_constant, ordinary_constant, third_constant])
        {
            add_claim_target(&mut target, scale, *claim, field.mul(&scale, &c), field);
            scale = field.mul(&scale, &self.eta);
        }
        for pair in proof.compaction {
            let point = &pair.output.terminal_point;
            let rank = point[..10]
                .iter()
                .enumerate()
                .fold(field.zero(), |sum, (bit, &v)| {
                    field.add(&sum, &mul_i(v, 1 << bit, field))
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
        let Some(native) = &self.native_claim else {
            return Ok(target);
        };
        let native_sum = native
            .weights
            .iter()
            .fold(field.zero(), |s, w| field.add(&s, w));
        let constant = mul_i(field.mul(&scale, &native_sum), 6144, field);
        add_claim_target(&mut target, scale, native.target, constant, field);
        Ok(target)
    }
}

fn interval_sum(point: &[F], start: usize, count: usize, field: &Cfg) -> F {
    field.sub(
        &eq_prefix_sum(point, start + count, field),
        &eq_prefix_sum(point, start, field),
    )
}

/// Constants of the repeated hybrid linear template, excluding public messages
/// and the instance-specific nonce and signature coefficient bytes.
fn linear_constant(point: &[F], field: &Cfg) -> F {
    let mut constant = field.neg(&interval_sum(point, 0, 1, field));
    constant = field.sub(
        &constant,
        &mul_i(interval_sum(point, 257, 1, field), 0x5a, field),
    );
    let start = 1 + 256 + super::super::CT_SIGNATURE_BYTES;
    for i in 0..HASH_TO_POINT_SAMPLES {
        constant = field.sub(&constant, &interval_sum(point, start + 2 * i + 1, 1, field));
    }
    field.sub(
        &constant,
        &interval_sum(point, start + 2 * HASH_TO_POINT_SAMPLES + 1, 1, field),
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
        let w = weights.at(row);
        row += 1;
        w
    };
    sink.add(offsets.shared_one, next());
    for bit in 0..256 {
        sink.add(offsets.message + bit, next());
    }
    for byte in 0..super::super::CT_SIGNATURE_BYTES {
        sink.add_word(offsets.encoded_signature + 8 * byte, 8, next(), field);
    }
    for i in 0..HASH_TO_POINT_SAMPLES {
        let weight = next();
        sink.add_word(offsets.hash_words + 16 * i, 16, weight, field);
        sink.add_word(
            offsets.hash_quotients + 3 * i,
            3,
            mul_i(weight, -i128::from(Q), field),
            field,
        );
        add_value_scaled(
            sink,
            offsets.hash_remainders + 14 * i,
            field.neg(&weight),
            field,
        );
        let weight = next();
        sink.add_word(offsets.hash_prefixes + 11 * (i + 1), 11, weight, field);
        sink.add_word(
            offsets.hash_prefixes + 11 * i,
            11,
            field.neg(&weight),
            field,
        );
        sink.add(offsets.hash_accept_ands + i, weight);
    }
    sink.add_word(offsets.hash_prefixes, 11, next(), field);
    sink.add(
        offsets.hash_prefixes + 11 * HASH_TO_POINT_SAMPLES + 10,
        next(),
    );
    debug_assert_eq!(row, 4458);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_run_geometry_matches_unsigned_and_signed_word_bits() {
        for base in 0..8 {
            let words = (1..=27)
                .map(|width| Word {
                    base,
                    width,
                    kind: WordKind::Unsigned,
                })
                .chain((0..8).map(|offset| Word {
                    base,
                    width: 12,
                    kind: WordKind::Encoded { offset },
                }))
                .collect::<Vec<_>>();
            let compiled = ByteRuns::new(&words);
            for (i, &word) in words.iter().enumerate() {
                let mut expected = [0i64; 64];
                let mut actual = [0i64; 64];
                for bit in 0..usize::from(word.width) {
                    let (index, negative) = word.bit_position(bit);
                    expected[index] += if negative { -(1 << bit) } else { 1 << bit };
                }
                for run in &compiled.runs[compiled.starts[i]..compiled.starts[i + 1]] {
                    assert_eq!(run.base % 8, 0);
                    assert!(run.len > 0 && run.lane + run.len <= 8);
                    for j in 0..usize::from(run.len) {
                        let value = 1 << (usize::from(run.shift) + j);
                        actual[run.base + usize::from(run.lane) + j] +=
                            if run.negative { -value } else { value };
                    }
                }
                assert_eq!(actual, expected, "{word:?}");
            }
        }
    }

    #[test]
    fn direct_byte_buckets_match_dense_overlaps_all_patterns_and_runtime_primes() {
        for field in [
            crate::piop::spartan::spartan_bitz_field_config(),
            field::FpCtx::from_prime_u128((1u128 << 127) - 1),
            field::FpCtx::from_prime_u128(7),
        ] {
            for base in [0, 256] {
                let mut sink = WordSink::new(base / 256, base, &field);
                overlapping_words(&mut sink, base, &field);
                let compact = sink.finish();
                let mut dense = Dense {
                    values: vec![field.zero(); 512],
                    field: &field,
                };
                overlapping_words(&mut dense, base, &field);
                for pattern in 0u8..=255 {
                    let byte = |index: usize| {
                        pattern
                            .wrapping_add((index / 8) as u8)
                            .rotate_left((index % 3) as u32)
                    };
                    let mut expected = vec![[field.zero(); 8]; 256];
                    for index in (base..base + 256).step_by(8) {
                        for (sum, coefficient) in expected[usize::from(byte(index))]
                            .iter_mut()
                            .zip(&dense.values[index..index + 8])
                        {
                            *sum = field.add(sum, coefficient);
                        }
                    }
                    // The zero witness byte contributes nothing to the prefix.
                    expected[0].fill(field.zero());
                    let mut actual = vec![[field.zero(); 8]; 256];
                    let mut next = 0usize;
                    compact
                        .emit_byte_buckets(
                            base,
                            &field,
                            &mut |index, occupied_lanes| {
                                assert!(occupied_lanes > 0 && occupied_lanes <= 8);
                                assert_eq!(index % 8, 0);
                                assert!((base..base + 256).contains(&index));
                                Ok(byte(index))
                            },
                            &mut |pattern, values| {
                                assert!(usize::from(pattern) >= next);
                                next = usize::from(pattern) + 1;
                                actual[usize::from(pattern)] = *values;
                                Ok(())
                            },
                        )
                        .unwrap();
                    assert_eq!(actual, expected, "base={base}, pattern={pattern}");
                }
            }
        }
    }

    #[test]
    fn direct_byte_buckets_reject_noncanonical_words_and_unaligned_base() {
        let field = crate::piop::spartan::spartan_bitz_field_config();
        let mut sink = WordSink::new(0, 0, &field);
        sink.add_word(0, 14, field.one(), &field);
        let mut compact = sink.finish();
        assert!(
            compact
                .emit_byte_buckets(1, &field, &mut |_, _| Ok(0), &mut |_, _| Ok(()))
                .is_err()
        );
        compact.coefficients[0] = crate::piop::spartan::noncanonical_test_value(&field);
        // Malformed coefficients are rejected even for an all-zero witness.
        assert!(
            compact
                .emit_byte_buckets(0, &field, &mut |_, _| Ok(0), &mut |_, _| Ok(()))
                .is_err()
        );
    }

    fn overlapping_words(sink: &mut impl CoefficientSink, base: usize, field: &Cfg) {
        overlapping_words_scaled(sink, base, unsigned(37, field), field);
    }

    fn overlapping_words_scaled(
        sink: &mut impl CoefficientSink,
        base: usize,
        scale: F,
        field: &Cfg,
    ) {
        // Word and isolated-bit terms overlap across every window/block boundary.
        for local in [0, 15, 31, 63, 127, 224] {
            sink.add_word(base + local, 27, scale, field);
            sink.add_word(base + local + 1, 14, scale, field);
            sink.add(base + local + 26, field.neg(&scale));
        }
        // CT encoding has a reversed bit order and alternates nibble offsets.
        for (local, offset) in [(8, 0), (24, 4), (56, 4), (120, 0), (224, 4)] {
            sink.add_encoded_word(base + local, offset, scale, field);
        }
        sink.add_word(base + 192, 8, scale, field);
        sink.add_word(base + 192, 8, field.neg(&scale), field);
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
        let field = crate::piop::spartan::spartan_bitz_field_config();
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
                for point_kind in 0..3 {
                    let point: Vec<_> = (0..width.ilog2())
                        .map(|i| match point_kind {
                            0 => field.zero(),
                            1 => field.one(),
                            _ => unsigned(19 + u128::from(i), &field),
                        })
                        .collect();
                    let weights = eq_table(&point, &field).unwrap();
                    let reference: Vec<_> = expected
                        .values
                        .chunks(width)
                        .map(|block| {
                            block
                                .iter()
                                .zip(&weights)
                                .fold(field.zero(), |sum, (v, w)| {
                                    field.add(&sum, &field.mul(v, w))
                                })
                        })
                        .collect();
                    let mut folded = vec![field.zero(); reference.len()];
                    let mut next = base / width;
                    compact
                        .emit_folded(base, &weights, &field, &mut |index, value| {
                            assert!(index >= next, "folded replay must contain final sums");
                            next = index + 1;
                            folded[index] = value;
                            Ok(())
                        })
                        .unwrap();
                    assert_eq!(
                        folded, reference,
                        "base={base}, width={width}, point={point_kind}"
                    );
                }
            }
        }
    }

    #[test]
    fn indexed_word_template_matches_dense_expansion_across_instances_and_zero_scales() {
        let field = crate::piop::spartan::spartan_bitz_field_config();
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
        record.add_word(300, 8, field.zero(), &field);
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
            indexed.add_word(base + 300, 8, dynamic_scale, &field);
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
            expected.add_word(base + 300, 8, dynamic_scale, &field);
            let mut actual = vec![field.zero(); 1024];
            compact
                .emit(base, &field, &mut |index, value| {
                    actual[index] = value;
                    Ok(())
                })
                .unwrap();
            assert_eq!(actual, expected.values);
            let weights = eq_table(&[unsigned(7, &field); 3], &field).unwrap();
            let mut folded = vec![field.zero(); expected.values.len() / weights.len()];
            compact
                .emit_folded(base, &weights, &field, &mut |index, value| {
                    folded[index] = field.add(&folded[index], &value);
                    Ok(())
                })
                .unwrap();
            let reference: Vec<_> = expected
                .values
                .chunks(weights.len())
                .map(|block| {
                    block
                        .iter()
                        .zip(&weights)
                        .fold(field.zero(), |sum, (v, w)| {
                            field.add(&sum, &field.mul(v, w))
                        })
                })
                .collect();
            assert_eq!(folded, reference);
            assert!(std::sync::Arc::ptr_eq(&compact.folded, &template.folded));
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
        let field = crate::piop::spartan::spartan_bitz_field_config();
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
            assert!(
                compact
                    .emit_folded(base, &vec![field.one(); width], &field, &mut |_, _| Ok(()))
                    .is_err()
            );
        }
    }
}
