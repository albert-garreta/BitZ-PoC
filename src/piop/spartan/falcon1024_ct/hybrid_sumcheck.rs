//! A joint binary sumcheck for several committed, packed sources.
//!
//! Coefficients are sums of tensor products and sparse additive gathers. The
//! first seven rounds operate on packed bits; only then are the coefficient
//! and witness tables materialized, with one field element per packed word.
//! The following lane rounds retain only occupied source lanes. Missing lanes
//! are still zero operands of the same virtual polynomial, not a smaller
//! sumcheck domain. Physical source padding remains in the witness table.
//! The caller must bind all commitments and all claims defining the public
//! coefficients, and sample any claim-batching scalars, before this reduction.
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

    fn axis(&self, source_vars: usize) -> usize {
        self.repeat_start
            .unwrap_or(source_vars - self.high_point.len())
    }

    fn index(&self, local: usize, repeat: usize, start: usize) -> usize {
        (local & ((1 << start) - 1))
            | (repeat << start)
            | ((local >> start) << (start + self.high_point.len()))
    }

    fn in_range(
        &self,
        start: usize,
        range: std::ops::Range<usize>,
        mut emit: impl FnMut(usize, usize, F),
    ) {
        let outer_shift = start + self.high_point.len();
        for outer in range.start >> outer_shift..=((range.end - 1) >> outer_shift) {
            let base = outer << outer_shift;
            let first = range.start.saturating_sub(base);
            let end = (range.end - base).min(1 << outer_shift);
            for repeat in first >> start..=((end - 1) >> start) {
                let segment = base + (repeat << start);
                let first_local = (outer << start) + range.start.saturating_sub(segment);
                let end_local = (outer << start) + (range.end - segment).min(1 << start);
                let first = self.entries.partition_point(|&(i, _)| i < first_local);
                let end = self.entries.partition_point(|&(i, _)| i < end_local);
                for &(index, coefficient) in &self.entries[first..end] {
                    emit(self.index(index, repeat, start), repeat, coefficient);
                }
            }
        }
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
    high: Vec<F>,
    marginals: Vec<F>,
}

impl TensorState {
    fn new(tensor: &Tensor, packed: &[F]) -> Self {
        let consume = 7usize.saturating_sub(tensor.low.len().ilog2() as usize);
        let extension = eq_table(&tensor.high_point[..consume]);
        let low: Vec<_> = extension
            .into_iter()
            .flat_map(|weight| tensor.low.iter().map(move |&value| weight * value))
            .collect();
        let high = eq_table(&tensor.high_point[consume..]);
        let marginals = tensor
            .marginals
            .clone()
            .unwrap_or_else(|| kernels::bit_marginals(packed, &high, low.len() / 128));
        Self {
            low,
            high,
            marginals,
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
    high: Vec<F>,
    low: Vec<F>,
    marginals: Vec<F>,
}

impl GatherState {
    fn new(gather: &Gather, packed: &[F], source_vars: usize) -> Self {
        let high = eq_table(&gather.high_point);
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
            high,
            low,
            marginals,
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

    fn get(values: &[F], column: usize) -> F {
        if column == usize::MAX {
            F::ZERO
        } else {
            values[column]
        }
    }

    fn message(&self, witness: &[F], weights: &[F]) -> [F; 2] {
        let mut message = [F::ZERO; 2];
        for &[left, right] in &self.pairs {
            let x0 = Self::get(witness, left);
            let x1 = Self::get(witness, right);
            let w0 = Self::get(weights, left);
            let w1 = Self::get(weights, right);
            message[0] += x0 * w0;
            message[1] += (x0 + x1) * (w0 + w1);
        }
        message
    }

    /// Fold a compact lane table and compute the next message while the
    /// outputs are in registers. After the support becomes dense, the normal
    /// fused dense kernel takes over without copying or reordering a table.
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
                    let mut message = [F::ZERO; 2];
                    let mut previous = [F::ZERO; 2];
                    for (group, (xo, wo)) in xo
                        .chunks_exact_mut(next_columns)
                        .zip(wo.chunks_exact_mut(next_columns))
                        .enumerate()
                    {
                        let base = (chunk * 64 + group) * columns;
                        let x = &scratch.witness[base..base + columns];
                        let w = &scratch.weights[base..base + columns];
                        let mut xf = [F::ZERO; 16];
                        let mut wf = [F::ZERO; 16];
                        for (column, &[left, right]) in self.pairs.iter().enumerate() {
                            let x0 = Self::get(x, left);
                            let x1 = Self::get(x, right);
                            let w0 = Self::get(w, left);
                            let w1 = Self::get(w, right);
                            xf[column] = x0 + r * (x0 + x1);
                            wf[column] = w0 + r * (w0 + w1);
                            xo[column].write(xf[column]);
                            wo[column].write(wf[column]);
                        }
                        if next.width == 1 {
                            // The next coordinate is a position coordinate,
                            // so its pairs straddle consecutive lane groups.
                            if group & 1 == 0 {
                                previous = [xf[0], wf[0]];
                            } else {
                                message[0] += previous[0] * previous[1];
                                message[1] += (previous[0] + xf[0]) * (previous[1] + wf[0]);
                            }
                        } else {
                            let term = next.message(&xf, &wf);
                            message[0] += term[0];
                            message[1] += term[1];
                        }
                    }
                    message
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

    fn prepare(self, _: &field::Gf128Ops, _: ()) -> Result<Self::State, SumcheckError> {
        let tensors = std::array::from_fn(|branch| {
            self.coefficients[branch]
                .tensors
                .iter()
                .map(|tensor| TensorState::new(tensor, self.sources[branch]))
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
    fn prepare_lanes(&mut self) {
        for tensor in self.tensors.iter_mut().flatten() {
            tensor.marginals = Vec::new();
        }
        for gather in self.gathers.iter_mut().flatten() {
            gather.marginals = Vec::new();
            gather.low = Vec::new();
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
            cfg_chunks_mut!(&mut witness.spare_capacity_mut()[..n], 64 * columns)
                .zip(cfg_chunks_mut!(
                    &mut weights.spare_capacity_mut()[..n],
                    64 * columns
                ))
                .enumerate()
                .map(|(chunk, (xo, wo))| {
                    let first_group = chunk * 64;
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
                                if index < 1 << geometry.logs[branch] {
                                    for tensor in &self.tensors[branch] {
                                        w[dest + lane] += tensor.low[index % tensor.low.len()]
                                            * tensor.high[index / tensor.low.len()];
                                    }
                                }
                            }
                        }
                        let first_bit = (first_group << k) * 128;
                        let bits = geometry.logs[branch] + 7;
                        let end_bit = ((end_group << k) * 128).min(1 << bits);
                        if first_bit >= end_bit {
                            continue;
                        }
                        for (gather, state) in self.input.coefficients[branch]
                            .gathers
                            .iter()
                            .zip(&self.gathers[branch])
                        {
                            gather.in_range(
                                gather.axis(bits),
                                first_bit..end_bit,
                                |index, repeat, coefficient| {
                                    let word = index >> 7;
                                    let dest = ((word >> k) - first_group) * columns
                                        + offsets[branch]
                                        + (word & (width - 1));
                                    w[dest] +=
                                        coefficient * state.high[repeat] * prefix[index & 127];
                                },
                            );
                        }
                    }
                    let mut message = [F::ZERO; 2];
                    for (x, w) in x[..xo.len()]
                        .chunks_exact(columns)
                        .zip(w[..wo.len()].chunks_exact(columns))
                    {
                        let term = layout.message(x, w);
                        message[0] += term[0];
                        message[1] += term[1];
                    }
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
            value += local * equality(&gather.high_point, &point[repeat_start..repeat_end]);
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

#[cfg(test)]
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
