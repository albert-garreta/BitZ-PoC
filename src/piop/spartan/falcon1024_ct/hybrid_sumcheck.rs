// A joint binary sumcheck for several committed, packed sources.
//
// Coefficients are sums of tensor products and sparse additive gathers. The
// first seven rounds operate on packed bits; only then are the coefficient
// and witness tables materialized, with one field element per packed word.
// The following lane rounds retain only occupied source lanes. Missing lanes
// are still zero operands of the same virtual polynomial, not a smaller
// sumcheck domain. Physical source padding remains in the witness table.
// The caller must bind all commitments and all claims defining the public
// coefficients, and sample any claim-batching scalars, before this reduction.
use crate::hybrid::{
    Error,
    opening::Geometry,
    sumcheck::{self as kernels, CompressedCodec, Scratch, eq_table},
};
use crate::ligerito::transpose_8x8_bits;
use crate::piop::spartan::grinding::GrindingDomain;
use crate::sumcheck::{
    RoundBoundaryPolicy, SumcheckError,
    boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
    inner::input,
};
use crate::transcript::{Blake3Transcript, traits::Transcript};
use crate::utils::{cfg_chunks, cfg_chunks_mut};
use flock_core::field::Gf128 as F;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// `low[index mod low.len()] * eq(high_point, index / low.len())`.
#[derive(Clone, Debug)]
pub(crate) struct Tensor {
    pub low: Vec<F>,
    pub high_point: Vec<F>,
    /// Prover-only folded witness values, already computed by the Keccak prefix.
    pub marginals: Option<Vec<F>>,
}

/// Sparse additive updates in one source's original bit coordinates.
/// Sorting once allows disjoint parallel writes after the packed prefix.
#[derive(Clone, Debug, Default)]
pub(crate) struct Gather {
    entries: Vec<(usize, F)>,
    high_point: Vec<F>,
    repeat_start: Option<usize>,
    /// Only these initial repeat indices carry equality weights.
    repeat_limit: Option<usize>,
}

impl Gather {
    pub fn new(entries: Vec<(usize, F)>) -> Self {
        Self::repeated(entries, Vec::new())
    }

    /// A sparse local map repeated with high equality weights. The local
    /// domain has `source_bit_log - high_point.len()` variables, at least seven.
    pub fn repeated(mut entries: Vec<(usize, F)>, high_point: Vec<F>) -> Self {
        entries.sort_unstable_by_key(|&(index, _)| index);
        Self {
            entries,
            high_point,
            repeat_start: None,
            repeat_limit: None,
        }
    }

    /// Insert the repeat coordinates at an arbitrary position above the seven
    /// packed bit coordinates. Entries use coordinates with that axis removed.
    pub fn repeated_at(
        entries: Vec<(usize, F)>,
        repeat_point: Vec<F>,
        repeat_start: usize,
    ) -> Self {
        Self {
            repeat_start: Some(repeat_start),
            ..Self::repeated(entries, repeat_point)
        }
    }

    /// Restrict the repeated map to a public prefix of its repeat domain.
    pub fn with_repeat_limit(mut self, limit: usize) -> Self {
        self.repeat_limit = Some(limit);
        self
    }

    fn repeat_weights(&self) -> Vec<F> {
        let mut weights = eq_table(&self.high_point);
        if let Some(limit) = self.repeat_limit {
            weights[limit..].fill(F::ZERO);
        }
        weights
    }

    fn axis(&self, source_vars: usize) -> usize {
        self.repeat_start
            .unwrap_or(source_vars - self.high_point.len())
    }

    fn index(&self, local: usize, repeat: usize, start: usize) -> usize {
        (local & ((1 << start) - 1))
            | (repeat << start)
            | ((local >> start) << (start + self.high_point.len()))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Coefficients {
    pub tensors: Vec<Tensor>,
    pub gathers: Vec<Gather>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Proof {
    pub rounds: Vec<[F; 2]>,
    pub value: F,
    pub nonces: Vec<u64>,
}

struct JointGrinding;
impl GrindingDomain for JointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/joint-binary-sumcheck/grinding/v1";
}

fn observe(t: &mut impl Transcript, values: &[F]) {
    for value in values {
        t.absorb_slice(&value.lo.to_le_bytes());
        t.absorb_slice(&value.hi.to_le_bytes());
    }
}

fn bind<const N: usize>(
    t: &mut impl Transcript,
    geometry: &Geometry<N>,
    target: F,
    grinding_bits: u32,
) {
    t.absorb_slice(b"bitz/falcon/joint-binary-sumcheck/v1");
    for log in geometry.logs {
        t.absorb_slice(&(log as u64).to_le_bytes());
    }
    t.absorb_slice(&grinding_bits.to_le_bytes());
    observe(t, &[target]);
}

fn validate<const N: usize>(
    geometry: &Geometry<N>,
    coefficients: [&Coefficients; N],
) -> Result<(), Error> {
    // Validate before touching the transcript or allocating a tensor table.
    let expected = Geometry::new(geometry.logs)?;
    if geometry.physical_logs != expected.physical_logs
        || geometry.position_log != expected.position_log
        || geometry.lane_logs != expected.lane_logs
        || geometry.virtual_lane_log != expected.virtual_lane_log
        || geometry.offsets != expected.offsets
    {
        return Err(Error::Invalid("joint binary sumcheck geometry"));
    }
    for (branch, coefficients) in coefficients.into_iter().enumerate() {
        let bits = geometry.logs[branch] + 7;
        for tensor in &coefficients.tensors {
            if !tensor.low.len().is_power_of_two()
                || tensor.low.len().ilog2() as usize + tensor.high_point.len() != bits
                || tensor
                    .marginals
                    .as_ref()
                    .is_some_and(|values| values.len() != tensor.low.len().max(128))
            {
                return Err(Error::Invalid("joint binary tensor shape"));
            }
        }
        for gather in &coefficients.gathers {
            if gather.high_point.len() > bits - 7
                || gather.axis(bits) < 7
                || gather
                    .axis(bits)
                    .checked_add(gather.high_point.len())
                    .is_none_or(|end| end > bits)
                || gather
                    .entries
                    .last()
                    .is_some_and(|&(index, _)| index >= 1 << (bits - gather.high_point.len()))
                || gather
                    .repeat_limit
                    .is_some_and(|limit| limit > 1 << gather.high_point.len())
            {
                return Err(Error::Invalid("joint binary gather index"));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "parallel")]
fn sum_pairs(iter: impl ParallelIterator<Item = [F; 2]>) -> [F; 2] {
    iter.reduce(|| [F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]])
}
#[cfg(not(feature = "parallel"))]
fn sum_pairs(iter: impl Iterator<Item = [F; 2]>) -> [F; 2] {
    iter.fold([F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]])
}

struct TensorState {
    low: Vec<F>,
    high: TensorHigh,
    marginals: Vec<F>,
}

enum TensorHigh {
    Dense(Vec<F>),
    /// Cached bit marginals need no full high equality table. Split that
    /// table at a position-tile boundary; after seven rounds the tensor's
    /// arbitrary low factor is one scalar, folded into the tile scale.
    Factored {
        low: Vec<F>,
        high: Vec<F>,
    },
}

const POSITION_TILE_LOG: usize = 6;
const POSITION_TILE: usize = 1 << POSITION_TILE_LOG;

impl TensorState {
    fn new(tensor: &Tensor, packed: &[F], tile_vars: usize) -> Self {
        let consume = 7usize.saturating_sub(tensor.low.len().ilog2() as usize);
        let extension = eq_table(&tensor.high_point[..consume]);
        let low: Vec<_> = extension
            .into_iter()
            .flat_map(|weight| tensor.low.iter().map(move |&value| weight * value))
            .collect();
        let point = &tensor.high_point[consume..];
        let (high, marginals) = match &tensor.marginals {
            Some(marginals) if low.len() == 128 => {
                let split = tile_vars.min(point.len());
                (
                    TensorHigh::Factored {
                        low: eq_table(&point[..split]),
                        high: eq_table(&point[split..]),
                    },
                    marginals.clone(),
                )
            }
            cached => {
                let high = eq_table(point);
                let marginals = cached
                    .clone()
                    .unwrap_or_else(|| kernels::bit_marginals(packed, &high, low.len() / 128));
                (TensorHigh::Dense(high), marginals)
            }
        };
        Self {
            low,
            high,
            marginals,
        }
    }

    /// Add coefficients after the packed prefix for one aligned position tile.
    /// The range excludes physical padding, whose coefficient stays zero.
    fn tile_weights(&self, range: std::ops::Range<usize>, mut emit: impl FnMut(usize, F)) {
        if range.is_empty() {
            return;
        }
        match &self.high {
            TensorHigh::Dense(high) => {
                for word in range {
                    emit(
                        word,
                        self.low[word % self.low.len()] * high[word / self.low.len()],
                    );
                }
            }
            TensorHigh::Factored { low, high } => {
                debug_assert_eq!(self.low.len(), 1);
                let tile = range.start / low.len();
                debug_assert_eq!(tile, (range.end - 1) / low.len());
                let scale = self.low[0] * high[tile];
                let mask = low.len() - 1; // Equality table lengths are powers of two.
                for word in range {
                    emit(word, scale * low[word & mask]);
                }
            }
        }
    }
}

/// A sparse gather only needs the words touched by its local coefficients.
/// Fold the repeat axis once, before the seven packed rounds:
/// `marginals[word, bit] = sum_repeat high[repeat] * source[word, repeat, bit]`.
/// The resulting dense coefficient blocks use the same packed-round kernel as
/// a tensor. Gaps between words do not matter until the dense suffix, which
/// still uses the original gather indices.
struct GatherState {
    /// Sorted local word IDs, retained through all seven bit folds.
    words: Vec<usize>,
    word_axis: usize,
    high: Vec<F>,
    low: Vec<F>,
    marginals: Vec<F>,
}

impl GatherState {
    fn new(gather: &Gather, packed: &[F], source_vars: usize) -> Self {
        let high = gather.repeat_weights();
        let mut words = Vec::new();
        let mut low = Vec::new();
        for &(index, coefficient) in &gather.entries {
            let word = index >> 7;
            if words.last() != Some(&word) {
                words.push(word);
                low.resize(low.len() + 128, F::ZERO);
            }
            low[(words.len() - 1) * 128 + (index & 127)] += coefficient;
        }
        let mut marginals = vec![F::ZERO; low.len()];
        let repeat_start = gather.axis(source_vars);
        // Sixteen words keep each task's marginal accumulators in L1. Each
        // source word is loaded once; transposing eight repeats replaces 128
        // field multiplications with subset-sum lookups and additions.
        cfg_chunks_mut!(marginals, 16 * 128)
            .enumerate()
            .for_each(|(task, output)| {
                let words = &words[task * 16..task * 16 + output.len() / 128];
                for (group, weights) in high.chunks(8).enumerate() {
                    if weights.len() == 8 {
                        let mut subsets = [[F::ZERO; 16]; 2];
                        for half in 0..2 {
                            for bit in 0..4 {
                                let end = 1 << bit;
                                for mask in 0..end {
                                    subsets[half][end + mask] =
                                        subsets[half][mask] + weights[half * 4 + bit];
                                }
                            }
                        }
                        for (&word, accumulators) in words.iter().zip(output.chunks_mut(128)) {
                            let bytes: [[u8; 16]; 8] = std::array::from_fn(|offset| {
                                let source = packed[gather.index(
                                    word * 128,
                                    group * 8 + offset,
                                    repeat_start,
                                ) >> 7];
                                (source.lo as u128 | ((source.hi as u128) << 64)).to_le_bytes()
                            });
                            for byte in 0..16 {
                                let column = bytes
                                    .iter()
                                    .enumerate()
                                    .fold(0u64, |v, (row, b)| v | ((b[byte] as u64) << (8 * row)));
                                for (bit, mask) in transpose_8x8_bits(column)
                                    .to_le_bytes()
                                    .into_iter()
                                    .enumerate()
                                {
                                    accumulators[8 * byte + bit] += subsets[0]
                                        [(mask & 15) as usize]
                                        + subsets[1][(mask >> 4) as usize];
                                }
                            }
                        }
                    } else {
                        for (&word, accumulators) in words.iter().zip(output.chunks_mut(128)) {
                            for (offset, &weight) in weights.iter().enumerate() {
                                let source = packed[gather.index(
                                    word * 128,
                                    group * 8 + offset,
                                    repeat_start,
                                ) >> 7];
                                for (half, mut bits) in
                                    [source.lo, source.hi].into_iter().enumerate()
                                {
                                    while bits != 0 {
                                        accumulators[half * 64 + bits.trailing_zeros() as usize] +=
                                            weight;
                                        bits &= bits - 1;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        Self {
            words,
            word_axis: repeat_start - 7,
            high,
            low,
            marginals,
        }
    }

    /// After the seven packed rounds, each low entry is the complete folded
    /// coefficient of one local word. Insert the unchanged repeat coordinates
    /// in word units; no per-bit coefficients need to be replayed.
    fn folded_in_range(&self, range: std::ops::Range<usize>, mut emit: impl FnMut(usize, F)) {
        debug_assert_eq!(self.words.len(), self.low.len());
        if range.is_empty() || self.words.is_empty() {
            return;
        }
        let start = self.word_axis;
        let outer_shift = start + self.high.len().ilog2() as usize;
        for outer in range.start >> outer_shift..=((range.end - 1) >> outer_shift) {
            let base = outer << outer_shift;
            let first = range.start.saturating_sub(base);
            let end = (range.end - base).min(1 << outer_shift);
            for repeat in first >> start..=((end - 1) >> start) {
                let segment = base + (repeat << start);
                let first_local = (outer << start) + range.start.saturating_sub(segment);
                let end_local = (outer << start) + (range.end - segment).min(1 << start);
                let first = self.words.partition_point(|&i| i < first_local);
                let end = self.words.partition_point(|&i| i < end_local);
                for (&word, &coefficient) in
                    self.words[first..end].iter().zip(&self.low[first..end])
                {
                    let index = segment + (word & ((1 << start) - 1));
                    emit(index, coefficient * self.high[repeat]);
                }
            }
        }
    }
}

/// Apply a prefix equality table to the low `prefix.len()` bits. The table's
/// unused byte positions have zero coefficients, including partial bytes.
#[cfg(test)]
fn prefix_bits(table: &[F], bits: u128, bytes: usize) -> F {
    (0..bytes).fold(F::ZERO, |sum, byte| {
        sum + table[byte * 256 + ((bits >> (8 * byte)) & 255) as usize]
    })
}

#[cfg(test)]
fn gather_round_reference(
    gather: &Gather,
    high: &[F],
    repeat_start: usize,
    packed: &[F],
    prefix: &[F],
    round: usize,
) -> [F; 2] {
    let mut coefficients = [F::ZERO; 128];
    coefficients[..prefix.len()].copy_from_slice(prefix);
    let table = kernels::byte_table(&coefficients);
    let width = 1usize << round;
    let bytes = width.div_ceil(8);
    sum_pairs(cfg_chunks!(&gather.entries, 1024).map(|chunk| {
        let mut sum = [F::ZERO; 2];
        for &(index, coefficient) in chunk {
            let base = (index & 127) & !((2 * width) - 1);
            let scale = coefficient * prefix[index & (width - 1)];
            let mut values = [F::ZERO; 2];
            for (repeat, &weight) in high.iter().enumerate() {
                let word = packed[gather.index(index, repeat, repeat_start) >> 7];
                let word = word.lo as u128 | ((word.hi as u128) << 64);
                let low = word >> base;
                let difference = low ^ (word >> (base + width));
                if index & width == 0 {
                    values[0] += weight * prefix_bits(&table, low, bytes);
                }
                values[1] += weight * prefix_bits(&table, difference, bytes);
            }
            sum[0] += scale * values[0];
            sum[1] += scale * values[1];
        }
        sum
    }))
}

struct Input<'a, const N: usize> {
    geometry: &'a Geometry<N>,
    sources: [&'a [F]; N],
    coefficients: [&'a Coefficients; N],
    scratch: &'a mut Scratch,
}

struct State<'a, const N: usize> {
    input: Input<'a, N>,
    tensors: [Vec<TensorState>; N],
    gathers: [Vec<GatherState>; N],
    point: [F; 7],
    round: usize,
    next: [F; 2],
    lanes: Option<LaneLayout>,
}

/// Compact columns for the occupied lanes of each virtual position. A pair
/// may have a missing operand, represented by `usize::MAX`; it still performs
/// the original Boolean-coordinate fold with that operand equal to zero.
struct LaneLayout {
    width: usize,
    active: Vec<usize>,
    pairs: Vec<[usize; 2]>,
}

impl LaneLayout {
    fn new(width: usize, active: Vec<usize>) -> Self {
        let mut pairs: Vec<[usize; 2]> = Vec::new();
        let mut previous = usize::MAX;
        for (column, &lane) in active.iter().enumerate() {
            if lane / 2 != previous {
                pairs.push([usize::MAX; 2]);
                previous = lane / 2;
            }
            pairs.last_mut().expect("occupied pair")[lane & 1] = column;
        }
        Self {
            width,
            active,
            pairs,
        }
    }

    fn folded(&self) -> Self {
        let mut active: Vec<_> = self.active.iter().map(|lane| lane / 2).collect();
        active.dedup();
        Self::new(self.width / 2, active)
    }

    fn kernel(&self) -> kernels::table::Lanes<'_> {
        kernels::table::Lanes {
            columns: self.active.len(),
            pairs: &self.pairs,
        }
    }

    fn message(&self, witness: &[F], weights: &[F]) -> [F; 2] {
        kernels::table::message(witness, weights, self.kernel())
    }

    /// Fold a compact lane table and compute the next message while the
    /// outputs are in registers. After the support becomes dense, the normal
    /// fused dense kernel takes over without copying or reordering a table.
    #[tracing::instrument(name = "falcon_joint:compact", skip_all)]
    fn fold(&self, scratch: &mut Scratch, r: F) -> (Self, [F; 2]) {
        let next = self.folded();
        let columns = self.active.len();
        let next_columns = next.active.len();
        let groups = scratch.witness.len() / columns;
        let n = groups * next_columns;
        let mut witness = kernels::take_cleared(&mut scratch.spare_x, n);
        let mut weights = kernels::take_cleared(&mut scratch.spare_w, n);
        let message = sum_pairs(
            cfg_chunks_mut!(&mut witness.spare_capacity_mut()[..n], 64 * next_columns)
                .zip(cfg_chunks_mut!(
                    &mut weights.spare_capacity_mut()[..n],
                    64 * next_columns
                ))
                .enumerate()
                .map(|(chunk, (xo, wo))| {
                    let start = chunk * 64 * columns;
                    let end = start + xo.len() / next_columns * columns;
                    kernels::table::lanes(
                        &scratch.witness[start..end],
                        &scratch.weights[start..end],
                        xo,
                        wo,
                        self.kernel(),
                        next.kernel(),
                        next.width == 1,
                        r,
                    )
                }),
        );
        // SAFETY: each output chunk writes all its occupied columns exactly
        // once. The table has `groups * next_columns` initialized entries.
        unsafe {
            witness.set_len(n);
            weights.set_len(n);
        }
        scratch.spare_x = std::mem::replace(&mut scratch.witness, witness);
        scratch.spare_w = std::mem::replace(&mut scratch.weights, weights);
        (next, message)
    }
}

impl<const N: usize> input::sealed::Input for Input<'_, N> {}
impl<'a, const N: usize> input::Input<field::Gf128Ops> for Input<'a, N> {
    type Weights = ();
    type State = State<'a, N>;
    type Codec = CompressedCodec;

    #[tracing::instrument(name = "falcon_joint:packed_setup", skip_all)]
    fn prepare(self, _: &field::Gf128Ops, _: ()) -> Result<Self::State, SumcheckError> {
        let tensors = std::array::from_fn(|branch| {
            self.coefficients[branch]
                .tensors
                .iter()
                .map(|tensor| {
                    TensorState::new(
                        tensor,
                        self.sources[branch],
                        POSITION_TILE_LOG + self.geometry.lane_logs[branch],
                    )
                })
                .collect()
        });
        let gathers = std::array::from_fn(|branch| {
            self.coefficients[branch]
                .gathers
                .iter()
                .map(|gather| {
                    GatherState::new(gather, self.sources[branch], self.geometry.logs[branch] + 7)
                })
                .collect()
        });
        Ok(State {
            input: self,
            tensors,
            gathers,
            point: [F::ZERO; 7],
            round: 0,
            next: [F::ZERO; 2],
            lanes: None,
        })
    }
}

impl<const N: usize> State<'_, N> {
    #[tracing::instrument(name = "falcon_joint:prepare", skip_all)]
    fn prepare_lanes(&mut self) {
        for tensor in self.tensors.iter_mut().flatten() {
            tensor.marginals = Vec::new();
        }
        for gather in self.gathers.iter_mut().flatten() {
            gather.marginals = Vec::new();
        }
        let geometry = self.input.geometry;
        let sources = self.input.sources;
        let scratch = &mut *self.input.scratch;
        let prefix: [F; 128] = eq_table(&self.point).try_into().expect("seven coordinates");
        let table = kernels::byte_table(&prefix);
        let mut active: Vec<_> = (0..N)
            .flat_map(|branch| {
                (0..1 << geometry.lane_logs[branch])
                    .map(move |local| geometry.offset(branch) + local)
            })
            .collect();
        active.sort_unstable();
        let layout = LaneLayout::new(geometry.lanes(), active);
        let columns = layout.active.len();
        let offsets: [usize; N] = std::array::from_fn(|branch| {
            layout
                .active
                .partition_point(|&lane| lane < geometry.offset(branch))
        });
        let n = (1usize << geometry.position_log) * columns;
        let mut witness = kernels::take_cleared(&mut scratch.witness, n);
        let mut weights = kernels::take_cleared(&mut scratch.weights, n);
        // Build one small position tile, including all sparse gathers, before
        // emitting its first lane-round message and writing its tables. This
        // avoids materializing the empty virtual lanes or scanning the newly
        // built tables again just to compute the first round.
        self.next = sum_pairs(
            cfg_chunks_mut!(
                &mut witness.spare_capacity_mut()[..n],
                POSITION_TILE * columns
            )
            .zip(cfg_chunks_mut!(
                &mut weights.spare_capacity_mut()[..n],
                POSITION_TILE * columns
            ))
            .enumerate()
            .map(|(chunk, (xo, wo))| {
                let first_group = chunk * POSITION_TILE;
                let end_group = first_group + xo.len() / columns;
                let mut x = [F::ZERO; 1024];
                let mut w = [F::ZERO; 1024];
                for branch in 0..N {
                    let k = geometry.lane_logs[branch];
                    let width = 1 << k;
                    for group in first_group..end_group {
                        let dest = (group - first_group) * columns + offsets[branch];
                        for lane in 0..width {
                            let index = (group << k) | lane;
                            // Include physical source padding in the
                            // witness; its coefficient is zero. The PCS
                            // separately authenticates that padding.
                            x[dest + lane] = kernels::apply(&table, sources[branch][index]);
                        }
                    }
                    let first_word = first_group << k;
                    let end_word = (end_group << k).min(1 << geometry.logs[branch]);
                    for tensor in &self.tensors[branch] {
                        tensor.tile_weights(first_word..end_word, |word, coefficient| {
                            let dest = ((word >> k) - first_group) * columns
                                + offsets[branch]
                                + (word & (width - 1));
                            w[dest] += coefficient;
                        });
                    }
                    for state in &self.gathers[branch] {
                        state.folded_in_range(first_word..end_word, |word, coefficient| {
                            let dest = ((word >> k) - first_group) * columns
                                + offsets[branch]
                                + (word & (width - 1));
                            w[dest] += coefficient;
                        });
                    }
                }
                let message = layout.message(&x[..xo.len()], &w[..wo.len()]);
                for (dst, &value) in xo.iter_mut().zip(&x) {
                    dst.write(value);
                }
                for (dst, &value) in wo.iter_mut().zip(&w) {
                    dst.write(value);
                }
                message
            }),
        );
        // SAFETY: every tile initializes every occupied lane in both outputs.
        unsafe {
            witness.set_len(n);
            weights.set_len(n);
        }
        self.lanes = (columns < geometry.lanes()).then_some(layout);
        self.tensors = std::array::from_fn(|_| Vec::new());
        self.gathers = std::array::from_fn(|_| Vec::new());
        scratch.witness = witness;
        scratch.weights = weights;
    }
}

impl<const N: usize> input::State<field::Gf128Ops> for State<'_, N> {
    fn num_vars(&self) -> usize {
        self.input.geometry.bit_log()
    }

    fn coefficients(&self, _: &field::Gf128Ops) -> Result<[F; 2], SumcheckError> {
        if self.round >= 7 {
            return Ok(self.next);
        }
        let prefix = eq_table(&self.point[..self.round]);
        let mut sum = [F::ZERO; 2];
        for branch in 0..N {
            for tensor in &self.tensors[branch] {
                let term =
                    kernels::packed_round(&tensor.marginals, &tensor.low, &prefix, self.round);
                sum[0] += term[0];
                sum[1] += term[1];
            }
            for gather in &self.gathers[branch] {
                let term =
                    kernels::packed_round(&gather.marginals, &gather.low, &prefix, self.round);
                sum[0] += term[0];
                sum[1] += term[1];
            }
        }
        Ok(sum)
    }

    fn fold(&mut self, _: &field::Gf128Ops, r: &F) -> Result<(), SumcheckError> {
        if self.round < 7 {
            self.point[self.round] = *r;
            for tensor in self.tensors.iter_mut().flatten() {
                kernels::fold(&mut tensor.low, *r);
            }
            for gather in self.gathers.iter_mut().flatten() {
                kernels::fold(&mut gather.low, *r);
            }
            self.round += 1;
            if self.round == 7 {
                self.prepare_lanes();
            }
        } else if let Some(lanes) = self.lanes.take() {
            let (next_lanes, message) = lanes.fold(self.input.scratch, *r);
            self.next = message;
            self.lanes = (next_lanes.active.len() < next_lanes.width).then_some(next_lanes);
            self.round += 1;
        } else {
            let _span = tracing::info_span!("falcon_joint:dense").entered();
            let s = &mut *self.input.scratch;
            self.next = kernels::dense_fold(
                &mut s.witness,
                &mut s.weights,
                &mut s.spare_x,
                &mut s.spare_w,
                *r,
            );
            self.round += 1;
        }
        Ok(())
    }

    fn terminal(&self, _: &field::Gf128Ops) -> Result<[F; 2], SumcheckError> {
        Ok([self.input.scratch.weights[0], self.input.scratch.witness[0]])
    }
}

pub(crate) fn prove<const N: usize>(
    t: &mut Blake3Transcript,
    geometry: &Geometry<N>,
    sources: [&[F]; N],
    coefficients: [&Coefficients; N],
    target: F,
    grinding_bits: u32,
    scratch: &mut Scratch,
) -> Result<(Proof, Vec<F>), Error> {
    validate(geometry, coefficients)?;
    if (0..N).any(|b| sources[b].len() != 1 << geometry.physical_logs[b]) {
        return Err(Error::Invalid("joint binary source shape"));
    }
    let mut boundary =
        ProverGrindingRoundBoundary::<JointGrinding>::with_round_offset(grinding_bits, 0);
    boundary
        .validate(geometry.bit_log())
        .map_err(|_| Error::Invalid("joint binary grinding difficulty"))?;
    bind(t, geometry, target, grinding_bits);
    let output = crate::sumcheck::inner::prove_inner_sumcheck(
        &field::Gf128Ops,
        t,
        target,
        Input {
            geometry,
            sources,
            coefficients,
            scratch,
        },
        (),
        &mut boundary,
    )
    .map_err(|_| Error::Invalid("joint binary sumcheck claim"))?;
    let value = output.terminal_evaluations[1];
    observe(t, &[value]);
    Ok((
        Proof {
            rounds: output
                .proof
                .round_polynomials
                .into_iter()
                .map(|[a, _, c]| [a, c])
                .collect(),
            value,
            nonces: boundary.into_nonces(),
        },
        output.point,
    ))
}

fn equality(a: &[F], b: &[F]) -> F {
    a.iter()
        .zip(b)
        .fold(F::ONE, |value, (&a, &b)| value * (F::ONE + a + b))
}

/// Sum `eq(a,i) * eq(b,i)` over `i < limit`, without expanding the repeat
/// domain. At each bit, retain the equal-prefix contribution and the sum
/// over all assignments to lower bits. The full prefix is ordinary equality.
fn prefix_equality(a: &[F], b: &[F], limit: usize) -> F {
    debug_assert_eq!(a.len(), b.len());
    debug_assert!(limit <= 1 << a.len());
    if limit == 1 << a.len() {
        return equality(a, b);
    }
    let mut below = F::ZERO;
    let mut all = F::ONE;
    for (bit, (&a, &b)) in a.iter().zip(b).enumerate() {
        let zero = (F::ONE + a) * (F::ONE + b);
        let one = a * b;
        below = if limit >> bit & 1 == 1 {
            zero * all + one * below
        } else {
            zero * below
        };
        all *= zero + one;
    }
    below
}

impl Coefficients {
    fn evaluate(&self, point: &[F]) -> F {
        let mut value = F::ZERO;
        for tensor in &self.tensors {
            let low_vars = tensor.low.len().ilog2() as usize;
            let low = tensor
                .low
                .iter()
                .zip(eq_table(&point[..low_vars]))
                .fold(F::ZERO, |sum, (&value, eq)| sum + value * eq);
            value += low * equality(&tensor.high_point, &point[low_vars..]);
        }
        for gather in &self.gathers {
            if gather.entries.is_empty() {
                continue;
            }
            // Factor the endpoint equality basis: O(sqrt(source bits)) space,
            // including for arbitrary indices, with no field-per-bit table.
            let local_vars = point.len() - gather.high_point.len();
            let repeat_start = gather.axis(point.len());
            let repeat_end = repeat_start + gather.high_point.len();
            let mut local_point = point[..repeat_start].to_vec();
            local_point.extend_from_slice(&point[repeat_end..]);
            let split = local_vars / 2;
            let low = eq_table(&local_point[..split]);
            let high = eq_table(&local_point[split..]);
            let mut local = F::ZERO;
            for &(index, coefficient) in &gather.entries {
                local += coefficient * low[index & (low.len() - 1)] * high[index >> split];
            }
            let repeat_point = &point[repeat_start..repeat_end];
            let repeat = match gather.repeat_limit {
                Some(limit) => prefix_equality(&gather.high_point, repeat_point, limit),
                None => equality(&gather.high_point, repeat_point),
            };
            value += local * repeat;
        }
        value
    }
}

pub(crate) fn verify<const N: usize>(
    t: &mut Blake3Transcript,
    geometry: &Geometry<N>,
    coefficients: [&Coefficients; N],
    target: F,
    grinding_bits: u32,
    proof: &Proof,
) -> Result<Vec<F>, Error> {
    validate(geometry, coefficients)?;
    if proof.rounds.len() != geometry.bit_log() {
        return Err(Error::Invalid("joint binary sumcheck rounds"));
    }
    let mut boundary =
        VerifierGrindingRoundBoundary::<JointGrinding>::new(grinding_bits, &proof.nonces);
    boundary
        .validate(geometry.bit_log())
        .map_err(|_| Error::Invalid("joint binary grinding shape"))?;
    bind(t, geometry, target, grinding_bits);
    let mut value = target;
    let mut point = Vec::with_capacity(proof.rounds.len());
    for (round, &[u0, u2]) in proof.rounds.iter().enumerate() {
        observe(t, &[u0, u2]);
        boundary
            .after_round(t, round)
            .map_err(|_| Error::Invalid("joint binary grinding nonce"))?;
        let r: F = t.get_field_challenge::<F>(&());
        value = u0 + r * (value + u2) + r * r * u2;
        point.push(r);
    }
    let mut coefficient = F::ZERO;
    for branch in 0..N {
        let (original, padding) = geometry.project_point(branch, &point);
        coefficient += padding * coefficients[branch].evaluate(&original);
    }
    if value != coefficient * proof.value {
        return Err(Error::Invalid("joint binary sumcheck terminal"));
    }
    observe(t, &[proof.value]);
    Ok(point)
}
falcon_tests! {
mod tests {
    use super::*;

    struct Random(u64);
    impl Random {
        fn next(&mut self) -> F {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            F {
                lo: self.0,
                hi: self.0.rotate_left(29),
            }
        }
    }

    #[test]
    fn cached_tensor_tile_factors_match_full_high_tables_and_padding() {
        let mut random = Random(0x461966ec);
        for low_vars in [0usize, 5, 7, 8] {
            for high_vars in [0usize, 3, 7, 10] {
                let source_vars = low_vars.max(7) + high_vars;
                let live_words = 1usize << (source_vars - 7);
                let packed: Vec<_> = (0..live_words).map(|_| random.next()).collect();
                let mut tensor = Tensor {
                    low: (0..1 << low_vars).map(|_| random.next()).collect(),
                    high_point: (0..source_vars - low_vars).map(|_| random.next()).collect(),
                    marginals: None,
                };
                let dense = TensorState::new(&tensor, &packed, 0);
                tensor.marginals = Some(dense.marginals.clone());
                let TensorHigh::Dense(high) = &dense.high else {
                    panic!("uncached tensors must retain the dense fallback");
                };
                for tile_vars in [0usize, 2, 6, 9] {
                    for point_kind in 0..3 {
                        let mut actual = TensorState::new(&tensor, &packed, tile_vars);
                        assert_eq!(actual.marginals, dense.marginals);
                        assert_eq!(
                            matches!(actual.high, TensorHigh::Factored { .. }),
                            low_vars <= 7,
                        );
                        let mut reference_low = dense.low.clone();
                        for _ in 0..7 {
                            let r = match point_kind {
                                0 => F::ZERO,
                                1 => F::ONE,
                                _ => random.next(),
                            };
                            kernels::fold(&mut actual.low, r);
                            kernels::fold(&mut reference_low, r);
                        }
                        let tile_words = 1usize << tile_vars;
                        let physical_words = live_words.max(tile_words) * 2;
                        let mut got = vec![F::ZERO; physical_words];
                        for first in (0..physical_words).step_by(tile_words) {
                            actual.tile_weights(
                                first..(first + tile_words).min(live_words),
                                |word, weight| got[word] += weight,
                            );
                        }
                        for (word, &value) in got.iter().enumerate() {
                            let expected = if word < live_words {
                                reference_low[word % reference_low.len()]
                                    * high[word / reference_low.len()]
                            } else {
                                F::ZERO
                            };
                            assert_eq!(
                                value, expected,
                                "low={low_vars}, high={high_vars}, tile={tile_vars}, point={point_kind}, word={word}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cached_keccak_tensor_high_storage_is_bounded_by_tile_factors() {
        let mut total = 0;
        for (high_vars, tile_vars) in [(23, 9), (23, 9), (21, 7), (21, 7)] {
            let tensor = Tensor {
                low: vec![F::ONE; 128],
                high_point: vec![F::ONE; high_vars],
                marginals: Some(vec![F::ZERO; 128]),
            };
            // Cached marginals must not trigger another source scan or a
            // full 2^high_vars allocation during preparation.
            let state = TensorState::new(&tensor, &[], tile_vars);
            let TensorHigh::Factored { low, high } = state.high else {
                panic!("cached Keccak tensor must use factored high weights");
            };
            assert_eq!(low.len(), 1 << tile_vars);
            assert_eq!(high.len(), 1 << (high_vars - tile_vars));
            total += (low.len() + high.len()) * std::mem::size_of::<F>();
        }
        assert_eq!(total, 1_069_056);
    }

    fn transcript(reverse_roots: bool) -> Blake3Transcript {
        let mut t = Blake3Transcript::new();
        t.absorb_slice(b"joint-sumcheck-test/two-committed-roots");
        for root in if reverse_roots { [2, 1] } else { [1, 2] } {
            t.absorb_slice(&[root; 32]);
        }
        t
    }

    fn fixture(logs: [usize; 2]) -> (Geometry, [Vec<F>; 2], [Coefficients; 2]) {
        let geometry = Geometry::new(logs).unwrap();
        let mut random = Random(0xabc123789);
        // Include nonzero physical padding: the public coefficient polynomial
        // is zero there, but the terminal witness still includes those bits.
        let packed = std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|_| random.next())
                .collect()
        });
        let coefficients = std::array::from_fn(|branch| {
            let bits = logs[branch] + 7;
            let tensors = [0, 6, 8]
                .into_iter()
                .map(|low_vars| Tensor {
                    marginals: None,
                    low: (0..1 << low_vars).map(|_| random.next()).collect(),
                    high_point: (0..bits - low_vars).map(|_| random.next()).collect(),
                })
                .collect();
            let mut absolute = vec![(0, random.next()), ((1 << bits) - 1, random.next())];
            let repeated_weight = random.next();
            absolute.extend([
                (129, repeated_weight),
                (129, repeated_weight),
                (129, random.next()),
            ]);
            for _ in 0..1097 {
                let index = random.next().lo as usize & ((1 << bits) - 1);
                absolute.push((index, random.next()));
            }
            let high_point = vec![random.next(), random.next()];
            let local_bits = bits - high_point.len();
            let mut repeated = vec![(0, random.next()), ((1 << local_bits) - 1, random.next())];
            for _ in 0..1113 {
                let index = random.next().lo as usize & ((1 << local_bits) - 1);
                repeated.push((index, random.next()));
            }
            Coefficients {
                tensors,
                gathers: vec![
                    Gather::new(absolute),
                    Gather::repeated(repeated.clone(), high_point.clone()),
                    Gather::repeated_at(repeated, high_point, 7 + branch * 5),
                    Gather::default(),
                ],
            }
        });
        (geometry, packed, coefficients)
    }

    fn dense_tables<const N: usize>(
        geometry: &Geometry<N>,
        packed: [&[F]; N],
        coefficients: [&Coefficients; N],
    ) -> (Vec<F>, Vec<F>, F) {
        let mut witness = vec![F::ZERO; 1 << geometry.bit_log()];
        let mut weights = witness.clone();
        for branch in 0..N {
            let mut local = vec![F::ZERO; 1 << (geometry.logs[branch] + 7)];
            for tensor in &coefficients[branch].tensors {
                let high = eq_table(&tensor.high_point);
                for (index, coefficient) in local.iter_mut().enumerate() {
                    *coefficient +=
                        tensor.low[index % tensor.low.len()] * high[index / tensor.low.len()];
                }
            }
            for gather in &coefficients[branch].gathers {
                let high = eq_table(&gather.high_point);
                let repeat_start = gather.axis(geometry.logs[branch] + 7);
                for (repeat, weight) in high.into_iter().enumerate() {
                    if gather.repeat_limit.is_some_and(|limit| repeat >= limit) {
                        continue;
                    }
                    for &(index, coefficient) in &gather.entries {
                        // Independent insertion oracle, one coordinate at a time.
                        let mut source = 0usize;
                        let mut remaining = index;
                        for coordinate in 0..geometry.logs[branch] + 7 {
                            let bit = if (repeat_start..repeat_start + gather.high_point.len())
                                .contains(&coordinate)
                            {
                                (repeat >> (coordinate - repeat_start)) & 1
                            } else {
                                let bit = remaining & 1;
                                remaining >>= 1;
                                bit
                            };
                            source |= bit << coordinate;
                        }
                        local[source] += weight * coefficient;
                    }
                }
            }
            for (word, value) in packed[branch].iter().enumerate() {
                let value = value.lo as u128 | ((value.hi as u128) << 64);
                for bit in 0..128 {
                    let dest = geometry.embed(branch, word) * 128 + bit;
                    witness[dest] = if value & (1 << bit) == 0 {
                        F::ZERO
                    } else {
                        F::ONE
                    };
                    if word * 128 + bit < local.len() {
                        weights[dest] = local[word * 128 + bit];
                    }
                }
            }
        }
        let target = witness
            .iter()
            .zip(&weights)
            .fold(
                F::ZERO,
                |sum, (&bit, &weight)| {
                    if bit == F::ZERO { sum } else { sum + weight }
                },
            );
        (witness, weights, target)
    }

    fn dense_prove<const N: usize>(
        t: &mut Blake3Transcript,
        geometry: &Geometry<N>,
        mut witness: Vec<F>,
        mut weights: Vec<F>,
        target: F,
        grinding_bits: u32,
    ) -> (Proof, Vec<F>) {
        bind(t, geometry, target, grinding_bits);
        let mut boundary =
            ProverGrindingRoundBoundary::<JointGrinding>::with_round_offset(grinding_bits, 0);
        let mut rounds = Vec::new();
        let mut point = Vec::new();
        while witness.len() > 1 {
            let mut message = [F::ZERO; 2];
            for (x, w) in witness.chunks_exact(2).zip(weights.chunks_exact(2)) {
                message[0] += x[0] * w[0];
                message[1] += (x[0] + x[1]) * (w[0] + w[1]);
            }
            observe(t, &message);
            boundary.after_round(t, rounds.len()).unwrap();
            let r = t.get_field_challenge::<F>(&());
            rounds.push(message);
            point.push(r);
            for table in [&mut witness, &mut weights] {
                for i in 0..table.len() / 2 {
                    table[i] = (F::ONE + r) * table[2 * i] + r * table[2 * i + 1];
                }
                table.truncate(table.len() / 2);
            }
        }
        observe(t, &witness);
        (
            Proof {
                rounds,
                value: witness[0],
                nonces: boundary.into_nonces(),
            },
            point,
        )
    }

    #[test]
    fn prefix_equality_matches_dense_for_every_limit() {
        let mut random = Random(0x7072_6566_6978);
        for vars in 0..=6 {
            let a: Vec<_> = (0..vars).map(|_| random.next()).collect();
            for kind in 0..3 {
                let b: Vec<_> = (0..vars)
                    .map(|i| match kind {
                        0 => F::ZERO,
                        1 if i % 2 == 0 => F::ONE,
                        _ => random.next(),
                    })
                    .collect();
                let products: Vec<_> = eq_table(&a)
                    .into_iter()
                    .zip(eq_table(&b))
                    .map(|(a, b)| a * b)
                    .collect();
                let mut expected = F::ZERO;
                for limit in 0..=1 << vars {
                    assert_eq!(prefix_equality(&a, &b, limit), expected);
                    if limit < products.len() {
                        expected += products[limit];
                    }
                }
            }
        }
    }

    #[test]
    fn masked_gathers_match_subcubes_and_dense_proof() {
        let (geometry, packed, _) = fixture([9, 10]);
        let sources = packed.each_ref().map(Vec::as_slice);
        let mut random = Random(0x6d61_736b_6761_7468);
        for (repeat_vars, live, pinned) in [
            (2, 3, false),
            (3, 7, false),
            (4, 9, false),
            (3, 8, false),
            (1, 1, true), // Batch-one K4's physical signature coordinate.
        ] {
            let repeat: Vec<_> = (0..repeat_vars)
                .map(|_| if pinned { F::ZERO } else { random.next() })
                .collect();
            let mut masked: [Coefficients; 2] = std::array::from_fn(|_| Coefficients::default());
            let mut subcubes = masked.clone();
            for branch in 0..2 {
                let local_bits = geometry.logs[branch] + 7 - repeat_vars;
                let axis = if branch == 0 { 7 } else { local_bits };
                let mut entries: Vec<_> = (0..64)
                    .map(|_| {
                        (
                            random.next().lo as usize & ((1 << local_bits) - 1),
                            random.next(),
                        )
                    })
                    .collect();
                entries.extend([(127, F::ONE), (127, F::ONE), (64, random.next())]);
                masked[branch].gathers.push(
                    Gather::repeated_at(entries.clone(), repeat.clone(), axis)
                        .with_repeat_limit(live),
                );
                for (point, scale) in super::super::hybrid::live_subcubes(&repeat, live) {
                    subcubes[branch].gathers.push(Gather::repeated_at(
                        entries.iter().map(|&(i, w)| (i, w * scale)).collect(),
                        point,
                        axis,
                    ));
                }
            }
            let (witness, weights, target) = dense_tables(&geometry, sources, masked.each_ref());
            let (_, old_weights, old_target) =
                dense_tables(&geometry, sources, subcubes.each_ref());
            assert_eq!(weights, old_weights);
            assert_eq!(target, old_target);
            let actual = prove(
                &mut transcript(false),
                &geometry,
                sources,
                masked.each_ref(),
                target,
                0,
                &mut Scratch::default(),
            )
            .unwrap();
            let old = prove(
                &mut transcript(false),
                &geometry,
                sources,
                subcubes.each_ref(),
                target,
                0,
                &mut Scratch::default(),
            )
            .unwrap();
            assert_eq!(actual, old);
            assert_eq!(
                actual,
                dense_prove(
                    &mut transcript(false),
                    &geometry,
                    witness,
                    weights,
                    target,
                    0
                )
            );
            assert_eq!(
                verify(
                    &mut transcript(false),
                    &geometry,
                    masked.each_ref(),
                    target,
                    0,
                    &actual.0,
                )
                .unwrap(),
                actual.1
            );
            masked[0].gathers[0].repeat_limit = Some((1 << repeat_vars) + 1);
            assert!(validate(&geometry, masked.each_ref()).is_err());
        }
    }

    #[test]
    fn cached_gather_rounds_match_repeat_scan_for_every_axis_and_round() {
        let mut random = Random(0x3894abc012);
        for source_vars in [10usize, 14] {
            // The physical source can extend past the gather's logical domain.
            // Keep that padding nonzero so accidental inclusion changes a sum.
            let packed: Vec<_> = (0..1 << (source_vars - 6)).map(|_| random.next()).collect();
            for repeat_vars in [0, 1, 2, 3, source_vars - 7] {
                let local_bits = source_vars - repeat_vars;
                let mut axes = vec![7, (7 + local_bits) / 2, local_bits];
                axes.sort_unstable();
                axes.dedup();
                for axis in axes {
                    let repeated = random.next();
                    let mut entries = vec![
                        (0, random.next()),
                        (127, random.next()),
                        ((1 << local_bits) - 1, random.next()),
                        (63, repeated),
                        (63, repeated),
                        (63, random.next()),
                        (64, F::ZERO),
                    ];
                    for _ in 0..257 {
                        let index = random.next().lo as usize & ((1 << local_bits) - 1);
                        entries.push((index, random.next()));
                    }
                    let high_point = (0..repeat_vars).map(|_| random.next()).collect();
                    let gather = Gather::repeated_at(entries, high_point, axis);
                    let mut state = GatherState::new(&gather, &packed, source_vars);
                    let mut words: Vec<_> = gather.entries.iter().map(|&(i, _)| i >> 7).collect();
                    words.dedup();
                    assert_eq!(state.marginals.len(), words.len() * 128);
                    let logical =
                        GatherState::new(&gather, &packed[..1 << (source_vars - 7)], source_vars);
                    assert_eq!(state.marginals, logical.marginals);
                    let point: [F; 7] = std::array::from_fn(|_| random.next());
                    for round in 0..7 {
                        let prefix = eq_table(&point[..round]);
                        let actual =
                            kernels::packed_round(&state.marginals, &state.low, &prefix, round);
                        let expected = gather_round_reference(
                            &gather,
                            &state.high,
                            axis,
                            &packed,
                            &prefix,
                            round,
                        );
                        assert_eq!(
                            actual, expected,
                            "source_vars={source_vars}, repeat_vars={repeat_vars}, axis={axis}, round={round}"
                        );
                        kernels::fold(&mut state.low, point[round]);
                    }
                    let prefix = eq_table(&point);
                    let word_count = 1 << (source_vars - 7);
                    let mut expected = vec![F::ZERO; word_count];
                    for (repeat, &weight) in state.high.iter().enumerate() {
                        for &(index, coefficient) in &gather.entries {
                            expected[gather.index(index, repeat, axis) >> 7] +=
                                coefficient * weight * prefix[index & 127];
                        }
                    }
                    for chunk in [1, 3, 64, word_count] {
                        let mut actual = vec![F::ZERO; word_count];
                        for start in (0..word_count).step_by(chunk) {
                            let end = (start + chunk).min(word_count);
                            state.folded_in_range(start..end, |word, coefficient| {
                                assert!((start..end).contains(&word));
                                actual[word] += coefficient;
                            });
                        }
                        assert_eq!(
                            actual, expected,
                            "folded replay: vars={source_vars}, repeat={repeat_vars}, axis={axis}, chunk={chunk}"
                        );
                    }
                    state.folded_in_range(0..0, |_, _| panic!("empty word interval"));
                }
            }
        }
        let empty = GatherState::new(&Gather::default(), &[], 7);
        assert!(empty.marginals.is_empty());
        assert!(empty.low.is_empty());
        assert_eq!(kernels::packed_round(&[], &[], &[F::ONE], 0), [F::ZERO; 2]);
    }

    #[test]
    fn structured_joint_sumcheck_matches_dense_two_root_transcript() {
        for logs in [[9, 9], [9, 10], [10, 9]] {
            let (geometry, packed, coefficients) = fixture(logs);
            let sources = [&packed[0][..], &packed[1][..]];
            let coefficients = [&coefficients[0], &coefficients[1]];
            let (witness, weights, target) = dense_tables(&geometry, sources, coefficients);
            let grinding = if logs == [9, 9] { 3 } else { 0 };
            let mut prover_t = transcript(false);
            let mut scratch = Scratch::default();
            let actual = prove(
                &mut prover_t,
                &geometry,
                sources,
                coefficients,
                target,
                grinding,
                &mut scratch,
            )
            .unwrap();
            let mut dense_t = transcript(false);
            let expected = dense_prove(&mut dense_t, &geometry, witness, weights, target, grinding);
            assert_eq!(actual, expected);
            assert_eq!(
                prover_t.get_field_challenge::<F>(&()),
                dense_t.get_field_challenge::<F>(&())
            );
            let mut verifier_t = transcript(false);
            assert_eq!(
                verify(
                    &mut verifier_t,
                    &geometry,
                    coefficients,
                    target,
                    grinding,
                    &actual.0
                )
                .unwrap(),
                actual.1
            );
            // No buffer reserves columns for unoccupied virtual lanes,
            // including when the same scratch is reused.
            let occupied_words: usize = geometry.physical_logs.iter().map(|log| 1 << log).sum();
            for capacity in [
                scratch.witness.capacity(),
                scratch.weights.capacity(),
                scratch.spare_x.capacity(),
                scratch.spare_w.capacity(),
            ] {
                assert!(capacity <= occupied_words);
            }
            let repeated = prove(
                &mut transcript(false),
                &geometry,
                sources,
                coefficients,
                target,
                grinding,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(actual, repeated);
        }
    }

    #[test]
    fn three_root_joint_sumcheck_and_cached_marginals_match_dense_transcript() {
        for logs in [[9, 11, 9], [9, 11, 10], [9, 13, 10], [9, 12, 10]] {
            let geometry = Geometry::new(logs).unwrap();
            let mut random = Random(0x791abc98);
            let packed: [Vec<F>; 3] = std::array::from_fn(|branch| {
                (0..1 << geometry.physical_logs[branch])
                    .map(|_| random.next())
                    .collect()
            });
            let mut coefficients: [Coefficients; 3] = std::array::from_fn(|branch| Coefficients {
                tensors: vec![Tensor {
                    low: (0..128).map(|_| random.next()).collect(),
                    high_point: (0..logs[branch]).map(|_| random.next()).collect(),
                    marginals: None,
                }],
                gathers: vec![Gather::new(vec![
                    (17, random.next()),
                    ((1 << (logs[branch] + 7)) - 1, random.next()),
                ])],
            });
            let sources = packed.each_ref().map(Vec::as_slice);
            let (witness, weights, target) =
                dense_tables(&geometry, sources, coefficients.each_ref());
            let mut reference = transcript(false);
            let expected = dense_prove(&mut reference, &geometry, witness, weights, target, 0);
            let mut actual_t = transcript(false);
            let actual = prove(
                &mut actual_t,
                &geometry,
                sources,
                coefficients.each_ref(),
                target,
                0,
                &mut Scratch::default(),
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                actual_t.get_challenge::<u128>(),
                reference.get_challenge::<u128>()
            );
            for (branch, c) in coefficients.iter_mut().enumerate() {
                let tensor = &mut c.tensors[0];
                tensor.marginals = Some(kernels::bit_marginals(
                    &sources[branch][..1 << logs[branch]],
                    &eq_table(&tensor.high_point),
                    1,
                ));
            }
            let cached = prove(
                &mut transcript(false),
                &geometry,
                sources,
                coefficients.each_ref(),
                target,
                0,
                &mut Scratch::default(),
            )
            .unwrap();
            assert_eq!(cached, expected);
            assert_eq!(
                verify(
                    &mut transcript(false),
                    &geometry,
                    coefficients.each_ref(),
                    target,
                    0,
                    &cached.0
                )
                .unwrap(),
                cached.1
            );
        }
    }

    #[test]
    fn compact_lane_folds_match_dense_for_every_nonempty_lane_support() {
        let mut random = Random(0x3453ad9);
        for mask in 1usize..1 << 16 {
            let active: Vec<_> = (0..16).filter(|lane| mask & (1 << lane) != 0).collect();
            let layout = LaneLayout::new(16, active.clone());
            let mut x = [F::ZERO; 16];
            let mut w = [F::ZERO; 16];
            for &lane in &active {
                x[lane] = random.next();
                w[lane] = random.next();
            }
            let compact_x: Vec<_> = active.iter().map(|&lane| x[lane]).collect();
            let compact_w: Vec<_> = active.iter().map(|&lane| w[lane]).collect();
            let mut expected = [F::ZERO; 2];
            for (x, w) in x.chunks_exact(2).zip(w.chunks_exact(2)) {
                expected[0] += x[0] * w[0];
                expected[1] += (x[0] + x[1]) * (w[0] + w[1]);
            }
            assert_eq!(layout.message(&compact_x, &compact_w), expected);
            let folded = layout.folded();
            let expected_support: Vec<_> = (0..8)
                .filter(|pair| mask & (3 << (pair * 2)) != 0)
                .collect();
            assert_eq!(folded.active, expected_support);
        }
    }

    fn check_source_count<const N: usize>(logs: [usize; N]) {
        let geometry = Geometry::new(logs).unwrap();
        let mut random = Random(0xdefd317);
        let packed: [Vec<F>; N] = std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|_| random.next())
                .collect()
        });
        let coefficients: [Coefficients; N] = std::array::from_fn(|branch| Coefficients {
            tensors: vec![Tensor {
                low: vec![random.next(); 128],
                high_point: (0..logs[branch]).map(|_| random.next()).collect(),
                marginals: None,
            }],
            gathers: Vec::new(),
        });
        let sources = packed.each_ref().map(Vec::as_slice);
        let claims = coefficients.each_ref();
        let (witness, weights, target) = dense_tables(&geometry, sources, claims);
        let mut reference = transcript(false);
        let expected = dense_prove(&mut reference, &geometry, witness, weights, target, 0);
        let mut actual_t = transcript(false);
        let actual = prove(
            &mut actual_t,
            &geometry,
            sources,
            claims,
            target,
            0,
            &mut Scratch::default(),
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual_t.get_challenge::<u128>(),
            reference.get_challenge::<u128>()
        );
    }

    #[test]
    fn compact_lane_sumcheck_supports_one_and_four_sources() {
        check_source_count([9]);
        check_source_count([9, 10, 9, 12]);
    }

    #[test]
    fn compact_lane_tables_preserve_physical_padding_at_the_opening_endpoint() {
        let (geometry, mut packed, coefficients) = fixture([9, 13]);
        let claims = coefficients.each_ref();
        let (_, _, target) = dense_tables(&geometry, packed.each_ref().map(Vec::as_slice), claims);
        let original = prove(
            &mut transcript(false),
            &geometry,
            packed.each_ref().map(Vec::as_slice),
            claims,
            target,
            0,
            &mut Scratch::default(),
        )
        .unwrap();
        // This bit is outside the logical source but inside its committed
        // physical polynomial. It has zero coefficient, so the initial claim
        // and packed rounds stay fixed; its final PCS evaluation must change.
        assert!(geometry.physical_logs[0] > geometry.logs[0]);
        packed[0][1 << geometry.logs[0]].lo ^= 1;
        let (witness, weights, altered_target) =
            dense_tables(&geometry, packed.each_ref().map(Vec::as_slice), claims);
        assert_eq!(target, altered_target);
        let altered = prove(
            &mut transcript(false),
            &geometry,
            packed.each_ref().map(Vec::as_slice),
            claims,
            target,
            0,
            &mut Scratch::default(),
        )
        .unwrap();
        let expected = dense_prove(
            &mut transcript(false),
            &geometry,
            witness,
            weights,
            target,
            0,
        );
        assert_eq!(altered, expected);
        assert_eq!(&original.0.rounds[..7], &altered.0.rounds[..7]);
        assert_ne!(original.0.value, altered.0.value);
        let mut substituted = altered.0;
        substituted.value = original.0.value;
        assert!(
            verify(
                &mut transcript(false),
                &geometry,
                claims,
                target,
                0,
                &substituted
            )
            .is_err()
        );
    }

    #[test]
    fn sparse_only_joint_sumcheck_handles_physical_padding() {
        let (geometry, packed, mut coefficients) = fixture([9, 13]);
        for coefficients in &mut coefficients {
            coefficients.tensors.clear();
        }
        let coefficients = [&coefficients[0], &coefficients[1]];
        let sources = [&packed[0][..], &packed[1][..]];
        let (witness, weights, target) = dense_tables(&geometry, sources, coefficients);
        let actual = prove(
            &mut transcript(false),
            &geometry,
            sources,
            coefficients,
            target,
            0,
            &mut Scratch::default(),
        )
        .unwrap();
        let expected = dense_prove(
            &mut transcript(false),
            &geometry,
            witness,
            weights,
            target,
            0,
        );
        assert_eq!(actual, expected);
        assert_eq!(
            verify(
                &mut transcript(false),
                &geometry,
                coefficients,
                target,
                0,
                &actual.0
            )
            .unwrap(),
            actual.1
        );
    }

    #[test]
    fn joint_sumcheck_rejects_tampering_and_malformed_inputs() {
        let (geometry, packed, coefficients) = fixture([9, 9]);
        let sources = [&packed[0][..], &packed[1][..]];
        let claims = [&coefficients[0], &coefficients[1]];
        let (_, _, target) = dense_tables(&geometry, sources, claims);
        let (proof, _) = prove(
            &mut transcript(false),
            &geometry,
            sources,
            claims,
            target,
            3,
            &mut Scratch::default(),
        )
        .unwrap();
        for round in [0, 6, proof.rounds.len() - 1] {
            let mut bad = proof.clone();
            bad.rounds[round][0] += F::ONE;
            assert!(verify(&mut transcript(false), &geometry, claims, target, 3, &bad).is_err());
        }
        let mut bad = proof.clone();
        bad.value += F::ONE;
        assert!(verify(&mut transcript(false), &geometry, claims, target, 3, &bad).is_err());
        bad = proof.clone();
        bad.nonces[0] = bad.nonces[0].wrapping_add(1);
        assert!(verify(&mut transcript(false), &geometry, claims, target, 3, &bad).is_err());
        bad = proof.clone();
        bad.nonces.pop();
        assert!(verify(&mut transcript(false), &geometry, claims, target, 3, &bad).is_err());
        assert!(verify(&mut transcript(true), &geometry, claims, target, 3, &proof).is_err());
        assert!(
            verify(
                &mut transcript(false),
                &geometry,
                claims,
                target + F::ONE,
                3,
                &proof
            )
            .is_err()
        );
        assert!(verify(&mut transcript(false), &geometry, claims, target, 0, &proof).is_err());
        let mut changed = coefficients[0].clone();
        changed.gathers[0].entries[0].1 += F::ONE;
        assert!(
            verify(
                &mut transcript(false),
                &geometry,
                [&changed, claims[1]],
                target,
                3,
                &proof
            )
            .is_err()
        );
        let mut invalid = Coefficients::default();
        invalid
            .gathers
            .push(Gather::new(vec![(1 << (geometry.logs[0] + 7), F::ONE)]));
        let mut t = transcript(false);
        assert!(
            prove(
                &mut t,
                &geometry,
                sources,
                [&invalid, claims[1]],
                target,
                3,
                &mut Scratch::default()
            )
            .is_err()
        );
        assert_eq!(
            t.get_field_challenge::<F>(&()),
            transcript(false).get_field_challenge::<F>(&())
        );
        invalid.gathers = vec![Gather::repeated(
            Vec::new(),
            vec![F::ZERO; geometry.logs[0] + 1],
        )];
        assert!(validate(&geometry, [&invalid, claims[1]]).is_err());
        invalid.gathers.clear();
        invalid.tensors.push(Tensor {
            marginals: None,
            low: vec![F::ONE; 3],
            high_point: Vec::new(),
        });
        assert!(validate(&geometry, [&invalid, claims[1]]).is_err());
        assert!(
            prove(
                &mut transcript(false),
                &geometry,
                [&packed[0][1..], sources[1]],
                claims,
                target,
                3,
                &mut Scratch::default()
            )
            .is_err()
        );
        assert!(
            prove(
                &mut transcript(false),
                &geometry,
                sources,
                claims,
                target,
                crate::piop::spartan::grinding::MAX_GRINDING_BITS + 1,
                &mut Scratch::default()
            )
            .is_err()
        );
        assert!(
            prove(
                &mut transcript(false),
                &geometry,
                sources,
                claims,
                target + F::ONE,
                3,
                &mut Scratch::default()
            )
            .is_err()
        );
    }
}

}
