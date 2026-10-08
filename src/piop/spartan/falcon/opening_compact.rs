// Word-sized storage for the hybrid binder. The public operator is compiled
// once and replayed in source order; no dense field table of source bits is kept.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum WordKind {
    Unsigned,
    Signed,
    /// A CT payload byte, mapped into aligned little-endian coefficient digits.
    SignatureByte {
        offset: u8,
    },
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
        let width = words
            .iter()
            .map(|word| usize::from(word.width))
            .max()
            .unwrap_or(0);
        // A prefix has at most 16 weights. Reuse their doubles across all
        // words instead of multiplying by the same powers of two per bit.
        let mut powers = Vec::with_capacity(width * weights.len());
        for bit in 0..width {
            for (index, &weight) in weights.iter().enumerate() {
                powers.push(if bit == 0 {
                    weight
                } else {
                    let previous = &powers[(bit - 1) * weights.len() + index];
                    field.add(previous, previous)
                });
            }
        }
        let mut starts = Vec::with_capacity(words.len() + 1);
        let mut terms = Vec::new();
        // Aligned coefficients repeat the same decoder on the low four
        // variables. Compile each relative shape once, including mapped bytes.
        let mut shapes = HashMap::<Word, Vec<(usize, F)>>::new();
        for &word in words {
            starts.push(terms.len());
            let base = word.base >> vars;
            let relative = Word {
                base: word.base & (weights.len() - 1),
                ..word
            };
            let shape = shapes.entry(relative).or_insert_with(|| {
                let mut sums = [field.zero(); 32];
                for bit in 0..usize::from(relative.width) {
                    let (index, negative) = relative.bit_position(bit);
                    let weight = powers[bit * weights.len() + (index & (weights.len() - 1))];
                    let slot = &mut sums[index >> vars];
                    *slot = if negative {
                        field.sub(slot, &weight)
                    } else {
                        field.add(slot, &weight)
                    };
                }
                sums.into_iter()
                    .enumerate()
                    .filter(|(_, value)| *value != field.zero())
                    .collect()
            });
            terms.extend(shape.iter().map(|&(offset, value)| (base + offset, value)));
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
            WordKind::Signed => (self.base + bit, bit + 1 == usize::from(self.width)),
            WordKind::SignatureByte { offset } => {
                let stream = usize::from(offset) + 7 - bit;
                (
                    self.base + 16 * (stream / SIGNATURE_BITS) + SIGNATURE_BITS
                        - 1
                        - stream % SIGNATURE_BITS,
                    false,
                )
            }
        }
    }

    fn evaluate(self, local: &[F], field: &Cfg) -> F {
        let mut sum = field.zero();
        for bit in (0..usize::from(self.width)).rev() {
            let (index, negative) = self.bit_position(bit);
            sum = field.add(&sum, &sum);
            sum = if negative {
                field.sub(&sum, &local[index])
            } else {
                field.add(&sum, &local[index])
            };
        }
        sum
    }
}

struct LocalEvaluation<'a> {
    local: &'a [F],
    field: &'a Cfg,
    sum: F,
}

impl<'a> LocalEvaluation<'a> {
    fn new(local: &'a [F], field: &'a Cfg) -> Self {
        Self {
            local,
            field,
            sum: field.zero(),
        }
    }

    fn word(&mut self, base: usize, width: usize, kind: WordKind, scale: F) {
        let value = Word {
            base,
            width: width as u8,
            kind,
        }
        .evaluate(self.local, self.field);
        self.sum = self.field.add(&self.sum, &self.field.mul(&scale, &value));
    }
}

impl CoefficientSink for LocalEvaluation<'_> {
    fn add(&mut self, index: usize, coefficient: F) {
        self.sum = self
            .field
            .add(&self.sum, &self.field.mul(&coefficient, &self.local[index]));
    }

    fn add_word(&mut self, base: usize, width: usize, scale: F, _: &Cfg) {
        self.word(base, width, WordKind::Unsigned, scale);
    }

    fn add_signed_word(&mut self, base: usize, width: usize, scale: F, _: &Cfg) {
        self.word(base, width, WordKind::Signed, scale);
    }
}

/// All instances share the public operator and the same dynamic addition order.
/// Compile their word slots once, then accumulate each instance by slot index.
/// Word descriptors are also shared by the cached instance coefficient vectors.
pub(super) struct CompiledTemplate {
    words: std::sync::Arc<[Word]>,
    common: Vec<(usize, F)>,
    rejection: Vec<(usize, F)>,
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
    fn new(
        common: CompiledCoefficients,
        rejection: CompiledCoefficients,
        dynamic_words: Vec<Word>,
    ) -> Self {
        let mut words = common.words.to_vec();
        words.extend_from_slice(&rejection.words);
        words.extend_from_slice(&dynamic_words);
        words.sort_unstable();
        words.dedup();
        let common = common
            .words
            .iter()
            .zip(common.coefficients)
            .map(|(word, value)| (words.binary_search(word).expect("common word slot"), value))
            .collect();
        let rejection = rejection
            .words
            .iter()
            .zip(rejection.coefficients)
            .map(|(word, value)| {
                (
                    words.binary_search(word).expect("rejection word slot"),
                    value,
                )
            })
            .collect();
        let dynamic_slots = dynamic_words
            .iter()
            .map(|word| words.binary_search(word).expect("dynamic word slot"))
            .collect();
        Self {
            words: words.into(),
            common,
            rejection,
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
        rejection_scale: F,
        field: &'a Cfg,
    ) -> WordSink<'a, IndexedWords<'a>> {
        let mut coefficients = vec![field.zero(); self.words.len()];
        for &(slot, value) in &self.common {
            coefficients[slot] = field.mul(&alpha, &value);
        }
        for &(slot, value) in &self.rejection {
            coefficients[slot] =
                field.add(&coefficients[slot], &field.mul(&rejection_scale, &value));
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

    fn add_signed_word(&mut self, base: usize, width: usize, scale: F, _: &Cfg) {
        self.word(base, width, WordKind::Signed, scale);
    }

    fn add_signature_byte(&mut self, layout: &FalconSourceLayout, byte: usize, scale: F, _: &Cfg) {
        let header_bytes = 1 + super::super::NONCE_BYTES;
        if byte < header_bytes {
            self.word(layout.signature_bit(byte, 0), 8, WordKind::Unsigned, scale);
        } else {
            let stream = 8 * (byte - header_bytes);
            self.word(
                layout.s2_bit(stream / SIGNATURE_BITS, 0),
                8,
                WordKind::SignatureByte {
                    offset: (stream % SIGNATURE_BITS) as u8,
                },
                scale,
            );
        }
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
        self.emit_word_buckets::<1>(
            base,
            field,
            &mut |base, lanes| read_byte(base, lanes).map(u16::from),
            &mut |_, byte, values| emit(byte, values),
        )
    }

    pub(super) fn emit_byte_pair_buckets(
        &self,
        base: usize,
        field: &Cfg,
        read_pair: &mut impl FnMut(usize, usize) -> Result<u16, crate::sumcheck::SumcheckError>,
        emit: &mut impl FnMut(usize, u8, &[F; 8]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        self.emit_word_buckets::<3>(base, field, read_pair, emit)
    }

    fn emit_word_buckets<const TABLES: usize>(
        &self,
        base: usize,
        field: &Cfg,
        read: &mut impl FnMut(usize, usize) -> Result<u16, crate::sumcheck::SumcheckError>,
        emit: &mut impl FnMut(usize, u8, &[F; 8]) -> Result<(), crate::sumcheck::SumcheckError>,
    ) -> Result<(), crate::sumcheck::SumcheckError> {
        use crate::sumcheck::SumcheckError;
        use field::{CtOrd, Reduce};
        let width = if TABLES == 3 { 16 } else { 8 };
        if base % width != 0 {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let compiled = self.byte_runs.get_or_init(|| ByteRuns::new(&self.words));
        // Each accumulator receives fewer than 2^64 field-by-u64 products;
        // native coefficient widths are at most 27 bits. The extra accumulator
        // limb therefore preserves exact sums for every supported batch.
        let mut differences = vec![field::FpLinearAcc::<2, 1>::zero(); TABLES * 256 * 8];
        let mut occupied = vec![false; TABLES * 256];
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
                let side = if TABLES == 3 { (run.base % 16) / 8 } else { 0 };
                let word = read(
                    base + (run.base & !(width - 1)),
                    8 * side + usize::from(run.lane + run.len),
                )?;
                let destinations = if TABLES == 3 {
                    [
                        (side, (word >> (8 * side)) as u8),
                        (2, (word >> (8 * (1 - side))) as u8),
                    ]
                } else {
                    [(0, word as u8), (0, 0)]
                };
                let lane = usize::from(run.lane);
                let len = usize::from(run.len);
                let (positive, negative) = if run.negative {
                    (&negative, &scale)
                } else {
                    (&scale, &negative)
                };
                for (table, byte) in destinations {
                    if byte == 0 {
                        continue;
                    }
                    let bucket = table * 256 + usize::from(byte);
                    occupied[bucket] = true;
                    let values = &mut differences[8 * bucket..][..8];
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
                emit(pattern / 256, (pattern % 256) as u8, &values)?;
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
    rejection: rejection::RejectionWeights,
}

impl BindingForm<'_> {
    fn prepared_compact_weights(&self) -> Result<&PreparedWeights, FalconError> {
        if let Some(weights) = self.compact_weights.get() {
            return Ok(weights);
        }
        let field = self.field;
        let weights = PreparedWeights {
            norm: factored_weights(self.proof.norm.point, field)?,
            norm_instances: eq_table(self.proof.norm.instance_point, field)
                .map_err(|error| piop(error.to_string()))?,
            rejection: rejection::RejectionWeights::new(self.layout, self.proof, self.eta, field)?,
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
        emit_linear_template(&mut common, &self.local_linear_weights, self.layout, field);
        {
            for j in 0..N {
                common.add_word(
                    self.layout.public_key_bit(j, 0),
                    14,
                    self.local_linear_weights.at(
                        super::super::FalconConstraintCounts::per_signature().linear_rows() - N + j,
                    ),
                    field,
                );
            }
        }
        let prepared = self.prepared_compact_weights()?;
        let mut rejection = WordSink::new(0, 0, field);
        prepared
            .rejection
            .emit_local(&mut rejection, self.layout, field);
        let mut dynamic = WordSink {
            field,
            instance: 0,
            base: 0,
            accumulator: RecordWords::default(),
        };
        self.emit_compact_instance_terms(0, &mut dynamic, prepared)?;
        let template =
            CompiledTemplate::new(common.finish(), rejection.finish(), dynamic.accumulator.0);
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
            self.prepared_compact_weights()?
                .rejection
                .instance_weights()[instance],
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
        debug_assert_eq!(sink.instances(layout.batch()), instance..instance + 1);
        let base = instance * layout.signature_stride();
        let alpha = self.linear_instance_weights[instance];
        let offsets = layout.offsets();
        let cutoff = self.selection.cutoff(instance);
        let indices = self.selection.indices(instance);
        let mut selected = 0;
        // The slot inventory includes every candidate, including zero weights,
        // so distinct public masks share the same compiled word topology.
        for j in 0..HASH_TO_POINT_SAMPLES {
            let mask_weight = if j <= cutoff {
                field.mul(
                    &alpha,
                    &self.local_linear_weights.at(selection_mask_row() + j),
                )
            } else {
                field.zero()
            };
            sink.add(base + offsets.hash_accept_ands + j, mask_weight);
            let weight = if selected < N && usize::from(indices[selected]) == j {
                let weight = field.neg(
                    &field.mul(
                        &alpha,
                        &self
                            .local_linear_weights
                            .at(selection_output_row() + selected),
                    ),
                );
                selected += 1;
                weight
            } else {
                field.zero()
            };
            add_value_scaled(sink, base + offsets.hash_remainders + 14 * j, weight, field);
        }
        debug_assert_eq!(selected, N);
        Ok(())
    }

    #[cfg(test)]
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
        if local.len() != self.layout.signature_stride()
            || instances.len() != self.layout.capacity()
        {
            return Err(piop("binding coefficient endpoint dimension mismatch"));
        }
        let prepared = self.prepared_compact_weights()?;
        let template = self.prepared_compact_template()?;
        let endpoints: Vec<F> = template
            .words
            .iter()
            .map(|&word| word.evaluate(local, field))
            .collect();
        let local_dot = |terms: &[(usize, F)]| -> F {
            let mut sum = <Cfg as BatchMulAcc<F>>::Accumulator::zero();
            for &(index, value) in terms {
                field.mul_acc(&mut sum, &value, &endpoints[index]);
            }
            field.reduce(sum)
        };
        let linear = local_dot(&template.common);
        let rejection = local_dot(&template.rejection);

        // Four repeated forms cover common linear rows, rejection, S1 and
        // slack. Public selection contributes an additional sparse routing
        // form evaluated directly from the same local coefficient endpoints.
        // In particular the verifier never prepares per-signature word tables.
        let norm_local = eq_table(&self.proof.norm.point[..COEFFICIENT_LOG], field)
            .map_err(|error| piop(error.to_string()))?;
        let norm_instances = eq_table(&self.proof.norm.point[COEFFICIENT_LOG + 1..], field)
            .map_err(|error| piop(error.to_string()))?;
        let mut s1 = LocalEvaluation::new(local, field);
        let mut slack = LocalEvaluation::new(local, field);
        for (i, &weight) in norm_local.iter().enumerate() {
            add_value_scaled(&mut s1, self.layout.s1_bit(i, 0), weight, field);
        }
        let mut power = field.one();
        for bit in 0..NORM_BITS {
            slack.add(self.layout.norm_slack_bit(bit), power);
            power = field.add(&power, &power);
        }
        let mut linear_weight = field.zero();
        let mut rejection_weight = field.zero();
        let mut norm_weight = field.zero();
        let mut slack_weight = field.zero();
        let offsets = self.layout.offsets();
        let mut mask_prefix = vec![field.zero(); HASH_TO_POINT_SAMPLES + 1];
        let mut remainders = Vec::with_capacity(HASH_TO_POINT_SAMPLES);
        for j in 0..HASH_TO_POINT_SAMPLES {
            mask_prefix[j + 1] = field.add(
                &mask_prefix[j],
                &field.mul(
                    &self.local_linear_weights.at(selection_mask_row() + j),
                    &local[offsets.hash_accept_ands + j],
                ),
            );
            let mut endpoint = LocalEvaluation::new(local, field);
            add_value_scaled(
                &mut endpoint,
                offsets.hash_remainders + 14 * j,
                field.one(),
                field,
            );
            remainders.push(endpoint.sum);
        }
        let selection_weights: Vec<_> = (0..N)
            .map(|k| self.local_linear_weights.at(selection_output_row() + k))
            .collect();
        let mut selection = field.zero();
        for (s, &instance) in instances.iter().take(self.layout.batch()).enumerate() {
            let alpha = field.mul(&instance, &self.linear_instance_weights[s]);
            linear_weight = field.add(&linear_weight, &alpha);
            let mut routed = <Cfg as BatchMulAcc<F>>::Accumulator::zero();
            for (&index, weight) in self.selection.indices(s).iter().zip(&selection_weights) {
                field.mul_acc(&mut routed, weight, &remainders[usize::from(index)]);
            }
            let routing = field.sub(
                &mask_prefix[self.selection.cutoff(s) + 1],
                &field.reduce(routed),
            );
            selection = field.add(&selection, &field.mul(&alpha, &routing));
            rejection_weight = field.add(
                &rejection_weight,
                &field.mul(&instance, &prepared.rejection.instance_weights()[s]),
            );
            let norm = field.mul(&instance, &norm_instances[s]);
            norm_weight = field.add(&norm_weight, &norm);
            slack_weight = field.add(
                &slack_weight,
                &field.mul(&instance, &prepared.norm_instances[s]),
            );
        }
        let eta2 = field.mul(&self.eta, &self.eta);
        let side_zero = field.sub(&field.one(), &self.proof.norm.point[COEFFICIENT_LOG]);
        let s1_weight = field.mul(&self.eta, &field.mul(&side_zero, &norm_weight));
        let mut sum = <Cfg as BatchMulAcc<F>>::Accumulator::zero();
        for (value, weight) in [
            (linear, linear_weight),
            (rejection, rejection_weight),
            (s1.sum, s1_weight),
            (slack.sum, field.mul(&eta2, &slack_weight)),
        ] {
            field.mul_acc(&mut sum, &value, &weight);
        }
        Ok(field.add(&field.reduce(sum), &selection))
    }

    pub(super) fn compact_target(&self) -> Result<F, FalconError> {
        let field = self.field;
        let proof = self.proof;
        let batch_vars = self.layout.capacity().ilog2() as usize;
        if proof.norm.point.len() != COEFFICIENT_LOG + 1 + batch_vars
            || proof.norm.instance_point.len() != batch_vars
        {
            return Err(piop("hybrid binding terminal dimensions mismatch"));
        }
        let mut constant = linear_constant(&self.local_linear_weights, field);
        let instance_sum = self
            .linear_instance_weights
            .iter()
            .fold(field.zero(), |sum, weight| field.add(&sum, weight));
        constant = field.mul(&constant, &instance_sum);
        let mut mask_prefix = vec![field.zero(); HASH_TO_POINT_SAMPLES + 1];
        for j in 0..HASH_TO_POINT_SAMPLES {
            mask_prefix[j + 1] = field.add(
                &mask_prefix[j],
                &self.local_linear_weights.at(selection_mask_row() + j),
            );
        }
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
                    // Fewer than 2^12 public byte/key terms, each below 2^14,
                    // fit the mixed accumulator before one final reduction.
                    let mut public_words = field::FpLinearAcc::<2, 1>::zero();
                    // The common header constant is already in linear_constant().
                    for (byte, &value) in encoded.iter().enumerate().skip(1) {
                        field.mul_acc(
                            &mut public_words,
                            &self.local_linear_weights.at(1 + 256 + byte),
                            &u64::from(value),
                        );
                    }

                    let mut expected_mask = mask_prefix[self.selection.cutoff(s) + 1];
                    for &index in self.selection.indices(s) {
                        expected_mask = field.sub(
                            &expected_mask,
                            &self
                                .local_linear_weights
                                .at(selection_mask_row() + usize::from(index)),
                        );
                    }
                    for (j, &value) in self.statement.public_keys[s].h.iter().enumerate() {
                        field.mul_acc(
                            &mut public_words,
                            &self.local_linear_weights.at(
                                super::super::FalconConstraintCounts::per_signature()
                                    .linear_rows()
                                    - N
                                    + j,
                            ),
                            &u64::from(value),
                        );
                    }
                    let signature_bytes = field.add(&field.reduce(public_words), &expected_mask);
                    Ok(field.mul(alpha, &field.add(&bits, &signature_bytes)))
                })
                .collect();
        for contribution in public_constants? {
            constant = field.sub(&constant, &contribution);
        }
        let mut target = field.sub(&field.zero(), &constant);
        let norm_instances = eq_table(&proof.norm.point[COEFFICIENT_LOG + 1..], field)
            .map_err(|error| piop(error.to_string()))?;
        let norm_sum = norm_instances
            .iter()
            .take(self.layout.batch())
            .fold(field.zero(), |s, v| field.add(&s, v));
        let side_zero = field.sub(&field.one(), &proof.norm.point[COEFFICIENT_LOG]);
        let constant = mul_i(
            field.mul(&self.eta, &field.mul(&norm_sum, &side_zero)),
            -6_144,
            field,
        );
        add_claim_target(&mut target, self.eta, proof.norm.terminal, constant, field);
        add_claim_target(
            &mut target,
            field.mul(&self.eta, &self.eta),
            proof.norm.slack,
            field.zero(),
            field,
        );
        Ok(field.add(
            &target,
            &self.prepared_compact_weights()?.rejection.target(),
        ))
    }
}

/// Constants of the repeated hybrid linear template, excluding public messages
/// and the instance-specific nonce and signature coefficient bytes.
fn linear_constant(weights: &crate::poly::mle::EqualityWeights<F>, field: &Cfg) -> F {
    field.sub(
        &field.neg(&weights.at(0)),
        &mul_i(weights.at(257), (0x50 + COEFFICIENT_LOG) as i128, field),
    )
}

fn selection_mask_row() -> usize {
    1 + 256 + super::super::CT_SIGNATURE_BYTES + HASH_TO_POINT_SAMPLES
}

fn selection_output_row() -> usize {
    selection_mask_row() + HASH_TO_POINT_SAMPLES
}

fn emit_linear_template(
    sink: &mut impl CoefficientSink,
    weights: &crate::poly::mle::EqualityWeights<F>,
    layout: &FalconSourceLayout,
    field: &Cfg,
) {
    let offsets = layout.offsets();
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
        sink.add_signature_byte(layout, byte, next(), field);
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
    }
    // Public masks determine the rejection rows' support per signature.
    for _ in 0..HASH_TO_POINT_SAMPLES {
        next();
    }
    for k in 0..N {
        sink.add_word(layout.hash_point_bit(k, 0), 14, next(), field);
    }
    debug_assert_eq!(
        row,
        super::super::FalconConstraintCounts::per_signature().linear_rows() - N
    );
}
falcon_tests! {
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
                .chain((1..=15).map(|width| Word { base, width, kind: WordKind::Signed }))
                .chain((0..SIGNATURE_BITS as u8).map(|offset| Word {
                    base,
                    width: 8,
                    kind: WordKind::SignatureByte { offset },
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
    fn compact_signature_bytes_match_layout_addresses_and_dense_expansion() {
        let layout = FalconSourceLayout::new(1).unwrap();
        let field = crate::piop::spartan::spartan_bitz_field_config();
        let mut compact = WordSink::new(0, 0, &field);
        let mut dense = Dense { values: vec![field.zero(); layout.signature_stride()], field: &field };
        for byte in 0..super::super::super::CT_SIGNATURE_BYTES {
            let weight = unsigned((byte + 17) as u128, &field);
            compact.add_signature_byte(&layout, byte, weight, &field);
            dense.add_signature_byte(&layout, byte, weight, &field);
        }
        let compact = compact.finish();
        let mut actual = vec![field.zero(); layout.signature_stride()];
        compact.emit(0, &field, &mut |index, value| {
            actual[index] = field.add(&actual[index], &value);
            Ok(())
        }).unwrap();
        assert_eq!(actual, dense.values);
        assert_eq!(compact.words.len(), super::super::super::CT_SIGNATURE_BYTES);
        for width in [1usize, 2, 4, 8, 16] {
            let point = (0..width.ilog2()).map(|i| unsigned(19 + u128::from(i), &field)).collect::<Vec<_>>();
            let weights = eq_table(&point, &field).unwrap();
            let expected = dense.values.chunks_exact(width).map(|chunk| {
                chunk.iter().zip(&weights).fold(field.zero(), |sum, (value, weight)| {
                    field.add(&sum, &field.mul(value, weight))
                })
            }).collect::<Vec<_>>();
            let mut actual = vec![field.zero(); expected.len()];
            compact.emit_folded(0, &weights, &field, &mut |index, value| {
                actual[index] = value;
                Ok(())
            }).unwrap();
            assert_eq!(actual, expected, "prefix width {width}");
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
    fn direct_byte_pair_buckets_match_dense_overlapping_word_expansion() {
        for field in [
            crate::piop::spartan::spartan_bitz_field_config(),
            field::FpCtx::from_prime_u128((1u128 << 127) - 1),
            field::FpCtx::from_prime_u128(7),
        ] {
            for base in [0, 256] {
                let mut sink = WordSink::new(base / 256, base, &field);
                overlapping_words(&mut sink, base, &field);
                let compact = sink.finish();
                let mut dense = Dense { values: vec![field.zero(); 512], field: &field };
                overlapping_words(&mut dense, base, &field);
                for pattern in [0u16, 1, 0x0100, 0x00ff, 0xff00, 0xffff, 0xaa55, 0x55aa, 0x8001, 0x0180, 0x1994] {
                    let pair = |index: usize| pattern.rotate_left(((index / 16) % 16) as u32);
                    let mut expected = vec![[field.zero(); 8]; 3 * 256];
                    for index in (base..base + 256).step_by(16) {
                        let witness = pair(index);
                        let bytes = [(witness & 255) as usize, (witness >> 8) as usize];
                        for lane in 0..8 {
                            let c0 = dense.values[index + lane];
                            let c1 = dense.values[index + 8 + lane];
                            for (table, byte, value) in [(0, bytes[0], c0), (1, bytes[1], c1), (2, bytes[1], c0), (2, bytes[0], c1)] {
                                if byte != 0 {
                                    expected[table * 256 + byte][lane] = field.add(&expected[table * 256 + byte][lane], &value);
                                }
                            }
                        }
                    }
                    let mut actual = vec![[field.zero(); 8]; 3 * 256];
                    let mut next = 0;
                    compact.emit_byte_pair_buckets(base, &field, &mut |index, lanes| {
                        assert_eq!(index % 16, 0);
                        assert!((base..base + 256).contains(&index));
                        assert!((1..=16).contains(&lanes));
                        Ok(pair(index))
                    }, &mut |table, byte, values| {
                        assert!(table < 3);
                        let bucket = table * 256 + usize::from(byte);
                        assert!(bucket >= next);
                        next = bucket + 1;
                        actual[bucket] = *values;
                        Ok(())
                    }).unwrap();
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
        assert!(compact.emit_byte_pair_buckets(8, &field, &mut |_, _| Ok(0), &mut |_, _, _| Ok(())).is_err());
        compact.coefficients[0] = crate::piop::spartan::noncanonical_test_value(&field);
        // Malformed coefficients are rejected even for an all-zero witness.
        assert!(
            compact
                .emit_byte_buckets(0, &field, &mut |_, _| Ok(0), &mut |_, _| Ok(()))
                .is_err()
        );
        assert!(compact.emit_byte_pair_buckets(0, &field, &mut |_, _| Ok(0), &mut |_, _, _| Ok(())).is_err());
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
            sink.add_signed_word(base + local + 2, SIGNATURE_BITS, scale, field);
            sink.add(base + local + 26, field.neg(&scale));
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
        let mut rejection = WordSink::new(0, 0, &field);
        rejection.add_word(300, 8, unsigned(17, &field), &field);
        rejection.add_word(310, 13, unsigned(23, &field), &field);
        let template = CompiledTemplate::new(common.finish(), rejection.finish(), record.accumulator.0);
        for (instance, alpha, dynamic_scale) in [
            (0, field.zero(), field.zero()),
            (1, field.zero(), unsigned(91, &field)),
            (0, field.one(), field.neg(&unsigned(37, &field))),
            (1, unsigned(23, &field), unsigned(91, &field)),
        ] {
            let base = instance * 512;
            let mut indexed = template.sink(instance, base, alpha, dynamic_scale, &field);
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
            expected.add_word(base + 300, 8, field.mul(&dynamic_scale, &unsigned(17, &field)), &field);
            expected.add_word(base + 310, 13, field.mul(&dynamic_scale, &unsigned(23, &field)), &field);
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

}
