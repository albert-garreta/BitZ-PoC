//! A joint binary sumcheck for two committed, packed sources.
//!
//! Coefficients are sums of tensor products and sparse additive gathers. The
//! first seven rounds operate on packed bits; only then are the coefficient
//! and witness tables materialized, with one field element per packed word.
//! The caller must bind both commitments and all claims defining the public
//! coefficients, and sample any claim-batching scalars, before this reduction.
use crate::hybrid::{
    Error,
    opening::Geometry,
    sumcheck::{self as kernels, CompressedCodec, Scratch, eq_table},
};
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

fn bind(t: &mut impl Transcript, geometry: &Geometry, target: F, grinding_bits: u32) {
    t.absorb_slice(b"bitz/falcon/joint-binary-sumcheck/v1");
    for log in geometry.logs {
        t.absorb_slice(&(log as u64).to_le_bytes());
    }
    t.absorb_slice(&grinding_bits.to_le_bytes());
    observe(t, &[target]);
}

fn validate(geometry: &Geometry, coefficients: [&Coefficients; 2]) -> Result<(), Error> {
    // Validate before touching the transcript or allocating a tensor table.
    let expected = Geometry::new(geometry.logs)?;
    if geometry.physical_logs != expected.physical_logs
        || geometry.position_log != expected.position_log
        || geometry.lane_logs != expected.lane_logs
        || geometry.virtual_lane_log != expected.virtual_lane_log
    {
        return Err(Error::Invalid("joint binary sumcheck geometry"));
    }
    for (branch, coefficients) in coefficients.into_iter().enumerate() {
        let bits = geometry.logs[branch] + 7;
        for tensor in &coefficients.tensors {
            if !tensor.low.len().is_power_of_two()
                || tensor.low.len().ilog2() as usize + tensor.high_point.len() != bits
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
        let marginals = kernels::bit_marginals(packed, &high, low.len() / 128);
        Self {
            low,
            high,
            marginals,
        }
    }
}

/// Apply a prefix equality table to the low `prefix.len()` bits. The table's
/// unused byte positions have zero coefficients, including partial bytes.
fn prefix_bits(table: &[F], bits: u128, bytes: usize) -> F {
    (0..bytes).fold(F::ZERO, |sum, byte| {
        sum + table[byte * 256 + ((bits >> (8 * byte)) & 255) as usize]
    })
}

fn gather_round(
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

struct Input<'a> {
    geometry: &'a Geometry,
    sources: [&'a [F]; 2],
    coefficients: [&'a Coefficients; 2],
    scratch: &'a mut Scratch,
}

struct State<'a> {
    input: Input<'a>,
    tensors: [Vec<TensorState>; 2],
    gather_high: [Vec<Vec<F>>; 2],
    point: [F; 7],
    round: usize,
    next: [F; 2],
}

impl input::sealed::Input for Input<'_> {}
impl<'a> input::Input<field::Gf128Ops> for Input<'a> {
    type Weights = ();
    type State = State<'a>;
    type Codec = CompressedCodec;

    fn prepare(self, _: &field::Gf128Ops, _: ()) -> Result<Self::State, SumcheckError> {
        let tensors = std::array::from_fn(|branch| {
            self.coefficients[branch]
                .tensors
                .iter()
                .map(|tensor| TensorState::new(tensor, self.sources[branch]))
                .collect()
        });
        let gather_high = std::array::from_fn(|branch| {
            self.coefficients[branch]
                .gathers
                .iter()
                .map(|gather| eq_table(&gather.high_point))
                .collect()
        });
        Ok(State {
            input: self,
            tensors,
            gather_high,
            point: [F::ZERO; 7],
            round: 0,
            next: [F::ZERO; 2],
        })
    }
}

impl State<'_> {
    fn prepare_dense(&mut self) {
        for tensor in self.tensors.iter_mut().flatten() {
            tensor.marginals = Vec::new();
        }
        let geometry = self.input.geometry;
        let sources = self.input.sources;
        let scratch = &mut *self.input.scratch;
        let prefix: [F; 128] = eq_table(&self.point).try_into().expect("seven coordinates");
        let table = kernels::byte_table(&prefix);
        let n = 1usize << geometry.packed_log();
        let lanes = geometry.lanes();
        let mut witness = kernels::take_cleared(&mut scratch.witness, n);
        let mut weights = kernels::take_cleared(&mut scratch.weights, n);
        {
            let sx = &mut witness.spare_capacity_mut()[..n];
            let sw = &mut weights.spare_capacity_mut()[..n];
            cfg_chunks_mut!(sx, lanes)
                .zip(cfg_chunks_mut!(sw, lanes))
                .enumerate()
                .for_each(|(group, (x, w))| {
                    for lane in 0..lanes {
                        let branch = lane / (lanes / 2);
                        let local_lane = lane % (lanes / 2);
                        let k = geometry.lane_logs[branch];
                        let mut value = F::ZERO;
                        let mut coefficient = F::ZERO;
                        if local_lane < 1 << k {
                            let index = (group << k) | local_lane;
                            value = kernels::apply(&table, sources[branch][index]);
                            if index < 1 << geometry.logs[branch] {
                                for tensor in &self.tensors[branch] {
                                    coefficient += tensor.low[index % tensor.low.len()]
                                        * tensor.high[index / tensor.low.len()];
                                }
                            }
                        }
                        x[lane].write(value);
                        w[lane].write(coefficient);
                    }
                });
        }
        // SAFETY: the disjoint groups above initialize all n slots of both buffers.
        unsafe {
            witness.set_len(n);
            weights.set_len(n);
        }
        // Sorted source indices map monotonically into virtual lane groups.
        // Each task owns whole groups, so arbitrary overlapping gathers can be
        // added without atomics or a second coefficient table.
        cfg_chunks_mut!(weights, 1024)
            .enumerate()
            .for_each(|(chunk, weights)| {
                let start = chunk * 1024;
                let first_group = start / lanes;
                let end_group = (start + weights.len()) / lanes;
                for branch in 0..2 {
                    let k = geometry.lane_logs[branch];
                    let first_bit = (first_group << k) * 128;
                    let bits = geometry.logs[branch] + 7;
                    let end_bit = ((end_group << k) * 128).min(1 << bits);
                    if first_bit >= end_bit {
                        continue;
                    }
                    for (gather, high) in self.input.coefficients[branch]
                        .gathers
                        .iter()
                        .zip(&self.gather_high[branch])
                    {
                        gather.in_range(
                            gather.axis(bits),
                            first_bit..end_bit,
                            |index, repeat, coefficient| {
                                let word = index >> 7;
                                let virtual_word = ((word >> k) << geometry.virtual_lane_log)
                                    + geometry.offset(branch)
                                    + (word & ((1 << k) - 1));
                                weights[virtual_word - start] +=
                                    coefficient * high[repeat] * prefix[index & 127];
                            },
                        );
                    }
                }
            });
        self.next = sum_pairs(
            cfg_chunks!(&witness, 1024)
                .zip(cfg_chunks!(&weights, 1024))
                .map(|(x, w)| {
                    let mut sum = [F::ZERO; 2];
                    for (x, w) in x.chunks_exact(2).zip(w.chunks_exact(2)) {
                        sum[0] += x[0] * w[0];
                        sum[1] += (x[0] + x[1]) * (w[0] + w[1]);
                    }
                    sum
                }),
        );
        self.tensors = [Vec::new(), Vec::new()];
        self.gather_high = [Vec::new(), Vec::new()];
        scratch.witness = witness;
        scratch.weights = weights;
    }
}

impl input::State<field::Gf128Ops> for State<'_> {
    fn num_vars(&self) -> usize {
        self.input.geometry.bit_log()
    }

    fn coefficients(&self, _: &field::Gf128Ops) -> Result<[F; 2], SumcheckError> {
        if self.round >= 7 {
            return Ok(self.next);
        }
        let prefix = eq_table(&self.point[..self.round]);
        let mut sum = [F::ZERO; 2];
        for branch in 0..2 {
            for tensor in &self.tensors[branch] {
                let term =
                    kernels::packed_round(&tensor.marginals, &tensor.low, &prefix, self.round);
                sum[0] += term[0];
                sum[1] += term[1];
            }
            for (gather, high) in self.input.coefficients[branch]
                .gathers
                .iter()
                .zip(&self.gather_high[branch])
            {
                let repeat_start = gather.axis(self.input.geometry.logs[branch] + 7);
                let term = gather_round(
                    gather,
                    high,
                    repeat_start,
                    self.input.sources[branch],
                    &prefix,
                    self.round,
                );
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
            self.round += 1;
            if self.round == 7 {
                self.prepare_dense();
            }
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

pub(crate) fn prove(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    sources: [&[F]; 2],
    coefficients: [&Coefficients; 2],
    target: F,
    grinding_bits: u32,
    scratch: &mut Scratch,
) -> Result<(Proof, Vec<F>), Error> {
    validate(geometry, coefficients)?;
    if (0..2).any(|b| sources[b].len() != 1 << geometry.physical_logs[b]) {
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

pub(crate) fn verify(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    coefficients: [&Coefficients; 2],
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
    for branch in 0..2 {
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

    fn dense_tables(
        geometry: &Geometry,
        packed: [&[F]; 2],
        coefficients: [&Coefficients; 2],
    ) -> (Vec<F>, Vec<F>, F) {
        let mut witness = vec![F::ZERO; 1 << geometry.bit_log()];
        let mut weights = witness.clone();
        for branch in 0..2 {
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

    fn dense_prove(
        t: &mut Blake3Transcript,
        geometry: &Geometry,
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
            // All retained dense buffers are bounded by the packed virtual
            // domain, including when the same scratch is reused.
            for capacity in [
                scratch.witness.capacity(),
                scratch.weights.capacity(),
                scratch.spare_x.capacity(),
                scratch.spare_w.capacity(),
            ] {
                assert!(capacity <= 1 << geometry.packed_log());
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
