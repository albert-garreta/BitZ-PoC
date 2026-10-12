//! Word-level source binder.
//!
//! The committed source stores one 16-bit word per signature coefficient:
//! lanes 0..14 hold its signed15 encoding and lane 15 is a spare lane that
//! carries one norm-slack bit on the first `SLACK_BITS` words of `s1` and
//! padding everywhere else, inactive capacity slots included. The ring
//! projection, the norm endpoint, the slack projection and the padding
//! zero-sum are merged with powers of `eta` into one linear statement about
//! two per-word tables, the decoded coefficient `T(c)` and the spare bit
//! `b(c)`:
//!
//! ```text
//! sum_c Lambda(c) * T(c) + sum_c M(c) * b(c) = target
//! Lambda(c) = ring_weight(c) + eta * eq(norm_point, c)
//! M(c)      = eta^2 * eq(rho, instance(c)) * 2^j      on slack positions
//!           = eta^3                                   elsewhere
//! ```
//!
//! Inactive slots have no ring weight. Their coefficients still enter the
//! norm endpoint, and the integer outer sumcheck forces their squares, hence
//! every signed15 lane, to zero. The quadratic sumcheck runs over the
//! `log2(N) + 1 + d` word variables only; its first round reads the committed
//! words natively, so a source that disagrees with the host witness fails the
//! target check. The terminal evaluations `T(r)` and `b(r)` are merged with a
//! fresh `zeta` into one BitZ bridge statement whose public row weights carry
//! the fixed lane patterns:
//!
//! ```text
//! sum_{lane, c} eq(r, c) * (decoder(lane) + zeta * [lane = 15]) * bit(lane, c)
//!     = T(r) + zeta * b(r)
//! ```

use field::{
    BatchMulAcc, FpProductAcc, FpSignedLinearAcc, MergeAccumulator, PreparedLinearCombination,
    Reduce, RingOps, Z,
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        absorb_field_elements,
        matrix::{eq_eval, eq_table},
        squeeze_field,
    },
    sumcheck::{
        RoundBoundaryPolicy, SumcheckProof, arithmetic::merge_accumulators,
        proof::validate_field_elements,
    },
    transcript::traits::Transcript,
};

use super::arithmetic::NormClaims;
use super::ring::PreparedClaim;
use super::{COEFFICIENT_LOG, Cfg, F, FalconError, Layout, N, SLACK_BITS, Source};

const LANES: usize = 16;
const COEFFICIENT_BITS: usize = 15;
/// Output entries per task in the dense folding passes.
const CHUNK: usize = 1 << 11;

/// The merged word weights, kept factored: ring weights per side, equality
/// weights per coefficient index, side and instance, and the eta powers.
pub(super) struct WordForm<'a> {
    layout: &'a Layout,
    field: &'a Cfg,
    ring: &'a PreparedClaim,
    norm_point: Vec<F>,
    eta: [F; 4],
    coefficient_weights: Vec<F>,
    side_weights: [F; 2],
    norm_instances: Vec<F>,
    slack_instances: Vec<F>,
    parallel: bool,
}

pub(super) struct Output {
    pub proof: SumcheckProof<F, 3>,
    pub point: Vec<F>,
    /// `[T(r), b(r)]`: the decoded coefficient table and the spare-lane table
    /// at the sumcheck's point.
    pub terminal: [F; 2],
}

/// Committed words, read through the column-major source rows.
struct Words<'a> {
    rows: &'a [Vec<u64>],
    per_column: usize,
}

impl Words<'_> {
    /// The 16-bit words at an even word index and its successor.
    #[inline(always)]
    fn pair(&self, c: usize) -> u32 {
        let offset = c % self.per_column;
        (self.rows[c / self.per_column][offset >> 2] >> (16 * (offset & 3))) as u32
    }
}

#[inline(always)]
fn coefficient(lane: u16) -> i64 {
    let value = i64::from(lane & 0x7fff);
    value - ((value >> 14) << 15)
}

#[inline(always)]
fn spare(lane: u16) -> bool {
    lane & 0x8000 != 0
}

/// Nonzero entries of a sparse word table, sorted by index.
type Sparse = Vec<(usize, F)>;

impl<'a> WordForm<'a> {
    pub(super) fn new(
        layout: &'a Layout,
        ring: &'a PreparedClaim,
        norm: &NormClaims,
        eta: F,
        field: &'a Cfg,
        parallel: bool,
    ) -> Result<(Self, F), FalconError> {
        let capacity_vars = layout.capacity().ilog2() as usize;
        let len = layout.batch() * N;
        if ring.weights_s1.len() != len
            || ring.weights_s2.len() != len
            || norm.point.len() != COEFFICIENT_LOG + 1 + capacity_vars
            || norm.instance_point.len() != capacity_vars
        {
            return Err(failure("algebraic binder claim dimension mismatch"));
        }
        validate_field_elements(&[ring.target, norm.terminal, norm.slack], field)
            .map_err(failure)?;
        validate_ring_weights(ring, parallel, field)?;
        let mut powers = [field.one(); 4];
        for i in 1..powers.len() {
            powers[i] = field.mul(&powers[i - 1], &eta);
        }
        let coefficient_weights =
            eq_table(&norm.point[..COEFFICIENT_LOG], field).map_err(failure)?;
        let side_weights = [
            field.sub(&field.one(), &norm.point[COEFFICIENT_LOG]),
            norm.point[COEFFICIENT_LOG],
        ];
        let norm_instances =
            eq_table(&norm.point[COEFFICIENT_LOG + 1..], field).map_err(failure)?;
        let slack_instances = eq_table(&norm.instance_point, field).map_err(failure)?;
        let target = field.add(
            &ring.target,
            &field.add(
                &field.mul(&powers[1], &norm.terminal),
                &field.mul(&powers[2], &norm.slack),
            ),
        );
        Ok((
            Self {
                layout,
                field,
                ring,
                norm_point: norm.point.clone(),
                eta: powers,
                coefficient_weights,
                side_weights,
                norm_instances,
                slack_instances,
                parallel,
            },
            target,
        ))
    }

    pub(super) fn word_vars(&self) -> usize {
        COEFFICIENT_LOG + 1 + self.layout.capacity().ilog2() as usize
    }

    fn ring_weights(&self, side: usize) -> &[F] {
        if side == 0 {
            &self.ring.weights_s1
        } else {
            &self.ring.weights_s2
        }
    }

    /// `eta * eq(norm_point, (., side, instance))`, with the coefficient
    /// factor left to the caller.
    fn scale(&self, side: usize, instance: usize) -> F {
        self.field.mul(
            &self.eta[1],
            &self
                .field
                .mul(&self.side_weights[side], &self.norm_instances[instance]),
        )
    }

    /// Materialize `Lambda` in word order and accumulate the first round:
    /// `[sum Lambda_0 T_0, sum Lambda_1 T_1, sum (Lambda_1 - Lambda_0)(T_1 - T_0)]`
    /// over adjacent word pairs.
    fn construct(&self, words: &Words<'_>, lambda: &mut [F]) -> [F; 3] {
        let field = self.field;
        let batch = self.layout.batch();
        let task = |(instance, chunk): (usize, &mut [F])| -> [FpSignedLinearAcc<2, 1>; 3] {
            let mut acc = [FpSignedLinearAcc::<2, 1>::default(); 3];
            for side in 0..2 {
                let scale = self.scale(side, instance);
                let ring = (instance < batch)
                    .then(|| &self.ring_weights(side)[instance * N..(instance + 1) * N]);
                let out = &mut chunk[side * N..(side + 1) * N];
                let base = instance * 2 * N + side * N;
                for j in (0..N).step_by(2) {
                    let mut w0 = field.mul(&scale, &self.coefficient_weights[j]);
                    let mut w1 = field.mul(&scale, &self.coefficient_weights[j + 1]);
                    if let Some(ring) = ring {
                        w0 = field.add(&w0, &ring[j]);
                        w1 = field.add(&w1, &ring[j + 1]);
                    }
                    out[j] = w0;
                    out[j + 1] = w1;
                    let pair = words.pair(base + j);
                    let t0 = coefficient(pair as u16);
                    let t1 = coefficient((pair >> 16) as u16);
                    if t0 != 0 {
                        field.mul_acc(&mut acc[0], &w0, &t0);
                    }
                    if t1 != 0 {
                        field.mul_acc(&mut acc[1], &w1, &t1);
                    }
                    if t0 != t1 {
                        field.mul_acc(&mut acc[2], &field.sub(&w1, &w0), &(t1 - t0));
                    }
                }
            }
            acc
        };
        let zero = || [FpSignedLinearAcc::<2, 1>::default(); 3];
        #[cfg(feature = "parallel")]
        let acc = if self.parallel {
            lambda
                .par_chunks_mut(2 * N)
                .enumerate()
                .map(task)
                .reduce(zero, merge_accumulators)
        } else {
            lambda
                .chunks_mut(2 * N)
                .enumerate()
                .fold(zero(), |acc, item| merge_accumulators(acc, task(item)))
        };
        #[cfg(not(feature = "parallel"))]
        let acc = lambda
            .chunks_mut(2 * N)
            .enumerate()
            .fold(zero(), |acc, item| merge_accumulators(acc, task(item)));
        acc.map(|sum| field.reduce(sum))
    }

    /// Fold the first variable of `Lambda` and of the committed coefficients,
    /// accumulating the next round's `[c_0, c_2]` from the folded pairs.
    fn fold_source(
        &self,
        words: &Words<'_>,
        lambda: &[F],
        r: &F,
        out_lambda: &mut [F],
        out_values: &mut [F],
    ) -> [F; 2] {
        let field = self.field;
        let zero = field.zero();
        let fold = <Cfg as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(
            field,
            [field.sub(&field.one(), r), *r],
        );
        let task = |chunk: usize, l: &mut [F], v: &mut [F]| {
            let base = chunk * CHUNK;
            for (i, (l, v)) in l.iter_mut().zip(v.iter_mut()).enumerate() {
                let c = 2 * (base + i);
                *l = field.add(
                    &lambda[c],
                    &field.mul(r, &field.sub(&lambda[c + 1], &lambda[c])),
                );
                let pair = words.pair(c);
                let t = [coefficient(pair as u16), coefficient((pair >> 16) as u16)];
                *v = if t == [0, 0] {
                    zero
                } else {
                    <Cfg as PreparedLinearCombination<Z<1>>>::linear_combination(&fold, |k| {
                        Z::from_twos_complement_words([t[k] as u64])
                    })
                };
            }
            next_coefficients(field, l, v)
        };
        fold_outputs(out_lambda, out_values, self.parallel, task).map(|sum| field.reduce(sum))
    }

    /// Fold one variable of both field tables, accumulating the next round.
    fn fold_dense(
        &self,
        lambda: &[F],
        values: &[F],
        r: &F,
        out_lambda: &mut [F],
        out_values: &mut [F],
    ) -> [F; 2] {
        let field = self.field;
        let task = |chunk: usize, l: &mut [F], v: &mut [F]| {
            let base = chunk * CHUNK;
            for (i, (l, v)) in l.iter_mut().zip(v.iter_mut()).enumerate() {
                let c = 2 * (base + i);
                *l = field.add(
                    &lambda[c],
                    &field.mul(r, &field.sub(&lambda[c + 1], &lambda[c])),
                );
                *v = field.add(
                    &values[c],
                    &field.mul(r, &field.sub(&values[c + 1], &values[c])),
                );
            }
            next_coefficients(field, l, v)
        };
        fold_outputs(out_lambda, out_values, self.parallel, task).map(|sum| field.reduce(sum))
    }

    /// Spare-lane bits of the committed source, in word order.
    fn spare_bits(&self, words: &Words<'_>) -> Sparse {
        let one = self.field.one();
        let scan = |instance: usize| -> Sparse {
            let base = instance * 2 * N;
            (0..2 * N)
                .step_by(2)
                .flat_map(|j| {
                    let pair = words.pair(base + j);
                    [
                        spare(pair as u16).then_some((base + j, one)),
                        spare((pair >> 16) as u16).then_some((base + j + 1, one)),
                    ]
                })
                .flatten()
                .collect()
        };
        #[cfg(feature = "parallel")]
        if self.parallel {
            return (0..self.layout.capacity())
                .into_par_iter()
                .map(scan)
                .flatten()
                .collect();
        }
        (0..self.layout.capacity()).flat_map(scan).collect()
    }

    /// `M - eta^3`: the slack projection weights minus the padding weight they
    /// replace, on the slack positions of active instances.
    fn slack_weights(&self) -> Sparse {
        let field = self.field;
        let mut out = Vec::with_capacity(self.layout.batch() * SLACK_BITS);
        for instance in 0..self.layout.batch() {
            let scaled = field.mul(&self.eta[2], &self.slack_instances[instance]);
            let mut power = scaled;
            for j in 0..SLACK_BITS {
                out.push((instance * 2 * N + j, field.sub(&power, &self.eta[3])));
                power = field.add(&power, &power);
            }
        }
        out
    }

    /// `Lambda(point)`: the ring weights' multilinear extension plus the norm
    /// endpoint's equality term, over the whole capacity.
    pub(super) fn coefficient_weight_at(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != self.word_vars() {
            return Err(failure("algebraic source opening dimension mismatch"));
        }
        let field = self.field;
        let local = eq_table(&point[..COEFFICIENT_LOG + 1], field).map_err(failure)?;
        let instances = eq_table(&point[COEFFICIENT_LOG + 1..], field).map_err(failure)?;
        // Each accumulator holds 2N products, independently of the batch.
        let reduce = field.prepare_product_reduction(2 * N);
        let instance = |instance: usize| -> F {
            let mut acc = FpProductAcc::<2>::default();
            for side in 0..2 {
                for (weight, local) in self.ring_weights(side)[instance * N..(instance + 1) * N]
                    .iter()
                    .zip(&local[side * N..(side + 1) * N])
                {
                    acc.accumulate(weight, local);
                }
            }
            field.mul(&reduce.reduce(acc), &instances[instance])
        };
        let batch = self.layout.batch();
        #[cfg(feature = "parallel")]
        let ring = if self.parallel {
            (0..batch)
                .into_par_iter()
                .map(instance)
                .reduce(|| field.zero(), |a, b| field.add(&a, &b))
        } else {
            (0..batch)
                .map(instance)
                .fold(field.zero(), |a, b| field.add(&a, &b))
        };
        #[cfg(not(feature = "parallel"))]
        let ring = (0..batch)
            .map(instance)
            .fold(field.zero(), |a, b| field.add(&a, &b));
        let norm = eq_eval(&self.norm_point, point, field).map_err(failure)?;
        Ok(field.add(&ring, &field.mul(&self.eta[1], &norm)))
    }

    /// `M(point)`: the padding weight everywhere, replaced by the slack
    /// projection weights on the first `SLACK_BITS` words of each active `s1`.
    pub(super) fn spare_weight_at(&self, point: &[F]) -> Result<F, FalconError> {
        if point.len() != self.word_vars() {
            return Err(failure("algebraic source opening dimension mismatch"));
        }
        let field = self.field;
        let coefficients = eq_table(&point[..COEFFICIENT_LOG], field).map_err(failure)?;
        let instances = eq_table(&point[COEFFICIENT_LOG + 1..], field).map_err(failure)?;
        let first_side = field.sub(&field.one(), &point[COEFFICIENT_LOG]);
        let mut decoder = field.zero();
        let mut positions = field.zero();
        let mut power = field.one();
        for weight in &coefficients[..SLACK_BITS] {
            decoder = field.add(&decoder, &field.mul(weight, &power));
            positions = field.add(&positions, weight);
            power = field.add(&power, &power);
        }
        let mut weighted = field.zero();
        let mut live = field.zero();
        for (weight, slack) in instances[..self.layout.batch()]
            .iter()
            .zip(&self.slack_instances)
        {
            weighted = field.add(&weighted, &field.mul(weight, slack));
            live = field.add(&live, weight);
        }
        let slack = field.sub(
            &field.mul(&self.eta[2], &field.mul(&decoder, &weighted)),
            &field.mul(&self.eta[3], &field.mul(&positions, &live)),
        );
        Ok(field.add(&self.eta[3], &field.mul(&first_side, &slack)))
    }
}

fn validate_ring_weights(
    ring: &PreparedClaim,
    parallel: bool,
    field: &Cfg,
) -> Result<(), FalconError> {
    let check = |chunk: &[F]| validate_field_elements(chunk, field).map_err(failure);
    for weights in [&ring.weights_s1, &ring.weights_s2] {
        #[cfg(feature = "parallel")]
        if parallel {
            weights.par_chunks(N).try_for_each(check)?;
            continue;
        }
        weights.chunks(N).try_for_each(check)?;
    }
    #[cfg(not(feature = "parallel"))]
    let _ = parallel;
    Ok(())
}

/// Next-round `[c_0, c_2]` contributions of consecutive folded pairs.
fn next_coefficients(field: &Cfg, lambda: &[F], values: &[F]) -> [FpProductAcc<2>; 2] {
    let mut acc = [FpProductAcc::<2>::default(); 2];
    for (l, v) in lambda.chunks_exact(2).zip(values.chunks_exact(2)) {
        acc[0].accumulate(&l[0], &v[0]);
        acc[1].accumulate(&field.sub(&l[1], &l[0]), &field.sub(&v[1], &v[0]));
    }
    acc
}

/// Run one task per output chunk, merging their accumulators.
fn fold_outputs<A: MergeAccumulator + Send>(
    out_lambda: &mut [F],
    out_values: &mut [F],
    parallel: bool,
    task: impl Fn(usize, &mut [F], &mut [F]) -> [A; 2] + Sync,
) -> [A; 2] {
    #[cfg(feature = "parallel")]
    if parallel {
        return out_lambda
            .par_chunks_mut(CHUNK)
            .zip(out_values.par_chunks_mut(CHUNK))
            .enumerate()
            .map(|(k, (l, v))| task(k, l, v))
            .reduce(|| [A::zero(), A::zero()], merge_accumulators);
    }
    #[cfg(not(feature = "parallel"))]
    let _ = parallel;
    out_lambda
        .chunks_mut(CHUNK)
        .zip(out_values.chunks_mut(CHUNK))
        .enumerate()
        .fold([A::zero(), A::zero()], |acc, (k, (l, v))| {
            merge_accumulators(acc, task(k, l, v))
        })
}

/// Adjacent-pair view of a sparse table: `(pair, value at 2 pair, value at 2 pair + 1)`.
fn pairs(field: &Cfg, list: &Sparse) -> Vec<(usize, F, F)> {
    let zero = field.zero();
    let mut out: Vec<(usize, F, F)> = Vec::with_capacity(list.len());
    for &(index, value) in list {
        let pair = index >> 1;
        match out.last_mut() {
            Some(last) if last.0 == pair => {
                if index & 1 == 0 {
                    last.1 = field.add(&last.1, &value);
                } else {
                    last.2 = field.add(&last.2, &value);
                }
            }
            _ if index & 1 == 0 => out.push((pair, value, zero)),
            _ => out.push((pair, zero, value)),
        }
    }
    out
}

/// `[p(0), p(1), c_2]` of `sum (eta^3 + S)(X) * b(X)` over the current pairs.
fn spare_coefficients(
    field: &Cfg,
    padding: &F,
    spare: &[(usize, F, F)],
    slack: &[(usize, F, F)],
) -> [F; 3] {
    let zero = field.zero();
    let mut at_zero = zero;
    let mut at_one = zero;
    for &(_, b0, b1) in spare {
        at_zero = field.add(&at_zero, &b0);
        at_one = field.add(&at_one, &b1);
    }
    let mut out = [
        field.mul(padding, &at_zero),
        field.mul(padding, &at_one),
        zero,
    ];
    let (mut i, mut j) = (0, 0);
    while i < spare.len() && j < slack.len() {
        match spare[i].0.cmp(&slack[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let (_, b0, b1) = spare[i];
                let (_, s0, s1) = slack[j];
                out[0] = field.add(&out[0], &field.mul(&s0, &b0));
                out[1] = field.add(&out[1], &field.mul(&s1, &b1));
                out[2] = field.add(
                    &out[2],
                    &field.mul(&field.sub(&s1, &s0), &field.sub(&b1, &b0)),
                );
                i += 1;
                j += 1;
            }
        }
    }
    out
}

fn fold_pairs(field: &Cfg, pairs: &[(usize, F, F)], r: &F) -> Sparse {
    pairs
        .iter()
        .map(|&(pair, v0, v1)| (pair, field.add(&v0, &field.mul(r, &field.sub(&v1, &v0)))))
        .collect()
}

/// Prove `sum_c Lambda(c) T(c) + M(c) b(c) = target` over the committed words.
pub(super) fn prove(
    transcript: &mut impl Transcript,
    form: &WordForm<'_>,
    source: &Source,
    target: F,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<Output, FalconError> {
    let field = form.field;
    let rounds = form.word_vars();
    boundary.validate(rounds).map_err(failure)?;
    let words = Words {
        rows: source.rows(),
        per_column: form.layout.bitz_params().rows() / LANES,
    };
    let total = form.layout.capacity() * 2 * N;
    let construction = tracing::info_span!("falcon_algebraic:binding_construction").entered();
    let mut lambda = field.zero_vec(total);
    let [a0, a1, a2] = form.construct(&words, &mut lambda);
    let mut spare = form.spare_bits(&words);
    let mut slack = form.slack_weights();
    drop(construction);
    let _rounds = tracing::info_span!("falcon_algebraic:binding_rounds").entered();
    let padding = form.eta[3];
    let mut proof = SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds),
    };
    let mut point = Vec::with_capacity(rounds);
    let mut values = Vec::new();
    let mut claim = target;
    let mut dense = [a0, a2];
    let mut dense_at_one = Some(a1);
    for round in 0..rounds {
        let spare_pairs = pairs(field, &spare);
        let slack_pairs = pairs(field, &slack);
        let [s0, s1, s2] = spare_coefficients(field, &padding, &spare_pairs, &slack_pairs);
        let c0 = field.add(&dense[0], &s0);
        let c2 = field.add(&dense[1], &s2);
        let c1 = match dense_at_one.take() {
            Some(a1) => {
                let at_one = field.add(&a1, &s1);
                if field.add(&c0, &at_one) != claim {
                    return Err(failure("algebraic binding target mismatch"));
                }
                field.sub(&field.sub(&at_one, &c0), &c2)
            }
            None => field.sub(&field.sub(&claim, &field.add(&c0, &c0)), &c2),
        };
        let coefficients = [c0, c1, c2];
        absorb_field_elements(transcript, &coefficients, field);
        boundary.after_round(transcript, round).map_err(failure)?;
        let r = squeeze_field(transcript, field).map_err(failure)?;
        claim = field.add(&c0, &field.mul(&r, &field.add(&c1, &field.mul(&r, &c2))));
        let half = total >> (round + 1);
        let mut next_lambda = field.zero_vec(half);
        let mut next_values = field.zero_vec(half);
        dense = if round == 0 {
            form.fold_source(&words, &lambda, &r, &mut next_lambda, &mut next_values)
        } else {
            form.fold_dense(&lambda, &values, &r, &mut next_lambda, &mut next_values)
        };
        lambda = next_lambda;
        values = next_values;
        spare = fold_pairs(field, &spare_pairs, &r);
        slack = fold_pairs(field, &slack_pairs, &r);
        point.push(r);
        proof.round_polynomials.push(coefficients);
    }
    let zero = field.zero();
    let terminal = [values[0], spare.first().map_or(zero, |entry| entry.1)];
    let spare_weight = field.add(&padding, &slack.first().map_or(zero, |entry| entry.1));
    let expected = field.add(
        &field.mul(&lambda[0], &terminal[0]),
        &field.mul(&spare_weight, &terminal[1]),
    );
    if claim != expected {
        return Err(failure("algebraic binding terminal mismatch"));
    }
    Ok(Output {
        proof,
        point,
        terminal,
    })
}

/// Bridge row weights `eq(r_low, j) * (decoder(lane) + zeta * [lane = 15])`
/// as canonical integers, for the rows `(j mod 2^(row_vars - 4), lane)`.
pub(super) fn bridge_row_weights(
    point: &[F],
    zeta: &F,
    layout: &Layout,
    field: &Cfg,
) -> Result<Vec<u128>, FalconError> {
    let row_vars = layout.row_vars();
    if point.len() < row_vars - 4 {
        return Err(failure("algebraic source opening dimension mismatch"));
    }
    let local = eq_table(&point[..row_vars - 4], field).map_err(failure)?;
    let mut lanes = [field.zero(); LANES];
    let mut power = field.one();
    for (lane, weight) in lanes.iter_mut().enumerate().take(COEFFICIENT_BITS) {
        *weight = if lane + 1 == COEFFICIENT_BITS {
            field.neg(&power)
        } else {
            power
        };
        power = field.add(&power, &power);
    }
    lanes[LANES - 1] = *zeta;
    let mut out = Vec::with_capacity(1 << row_vars);
    for weight in &local {
        for lane in &lanes {
            out.push(u128::from(field.to_integer(&field.mul(weight, lane))));
        }
    }
    Ok(out)
}

/// The bridge's column variables: the word variables above the row block.
pub(super) fn column_point(point: &[F], layout: &Layout) -> Vec<F> {
    point[layout.row_vars() - 4..].to_vec()
}

fn failure(error: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::{BETA_SQUARED, FalconAlgebraicWitness, WitnessData};
    use super::*;
    use crate::piop::spartan::SpartanField;
    use crate::sumcheck::UngrindedRoundBoundary;
    use crate::transcript::Blake3Transcript;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    fn unsigned(value: u128, field: &Cfg) -> F {
        F::from_with_cfg(value, field)
    }

    fn signed(value: i128, field: &Cfg) -> F {
        let magnitude = unsigned(value.unsigned_abs(), field);
        if value < 0 {
            field.neg(&magnitude)
        } else {
            magnitude
        }
    }

    fn data(batch: usize) -> WitnessData {
        let mut witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![[0; N]; batch],
        };
        let mut slacks = Vec::new();
        for i in 0..batch {
            for j in 0..N {
                witness.s1[i][j] = ((j * 7 + i * 3) % 23) as i16 - 11;
                witness.s2[i][j] = ((j * 5 + i) % 19) as i16 - 9;
            }
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

    fn claims(layout: &Layout, field: &Cfg) -> (PreparedClaim, NormClaims, F) {
        let batch = layout.batch();
        let instance_vars = layout.capacity().ilog2() as usize;
        let ring = PreparedClaim {
            weights_s1: (0..batch * N)
                .map(|j| signed(j as i128 % 53 - 37, field))
                .collect(),
            weights_s2: (0..batch * N)
                .map(|j| signed(j as i128 % 71 - 59, field))
                .collect(),
            target: signed(-103, field),
        };
        let norm = NormClaims {
            instance_point: (0..instance_vars)
                .map(|j| signed(3 * j as i128 - 7, field))
                .collect(),
            point: (0..COEFFICIENT_LOG + 1 + instance_vars)
                .map(|j| signed(7 * j as i128 - 19, field))
                .collect(),
            terminal: signed(-13, field),
            slack: signed(-31, field),
        };
        (ring, norm, signed(-37, field))
    }

    /// Every bit's weight in the merged statement, from the definition.
    fn bit_weight(form: &WordForm<'_>, norm_eq: &[F], index: usize) -> F {
        let field = form.field;
        let c = index / LANES;
        let lane = index % LANES;
        let (j, side, instance) = (c % N, (c / N) % 2, c / (2 * N));
        if lane == LANES - 1 {
            return if side == 0 && j < SLACK_BITS && instance < form.layout.batch() {
                field.mul(
                    &field.mul(&form.eta[2], &form.slack_instances[instance]),
                    &unsigned(1u128 << j, field),
                )
            } else {
                form.eta[3]
            };
        }
        let mut lambda = field.mul(&form.eta[1], &norm_eq[c]);
        if instance < form.layout.batch() {
            lambda = field.add(&lambda, &form.ring_weights(side)[instance * N + j]);
        }
        let digit = if lane + 1 == COEFFICIENT_BITS {
            -(1i128 << lane)
        } else {
            1i128 << lane
        };
        field.mul(&lambda, &signed(digit, field))
    }

    /// The merged statement's value on the committed bits, from the definition.
    fn dense_target(form: &WordForm<'_>, norm_eq: &[F], source: &Source) -> F {
        (0..form.layout.source_bits())
            .filter(|&index| source.bit(index))
            .fold(form.field.zero(), |sum, index| {
                form.field.add(&sum, &bit_weight(form, norm_eq, index))
            })
    }

    fn scrambled_source(layout: &Layout) -> Source {
        let mut source = Source::new(*layout, &data(layout.batch()));
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..layout.source_bits() / 64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            source.flip((state % layout.source_bits() as u64) as usize);
        }
        // Spare lanes and inactive slots must carry weight as well.
        source.flip(LANES * 3 + LANES - 1);
        source.flip(LANES * (SLACK_BITS + 5) + LANES - 1);
        source.flip(layout.batch() * layout.signature_stride() + 5);
        source.flip(layout.source_bits() - 1);
        source
    }

    #[test]
    fn word_binder_matches_the_dense_bit_statement() {
        let field = field();
        for batch in [1, 3] {
            let layout = Layout::new(batch).unwrap();
            let (ring, norm, eta) = claims(&layout, &field);
            let (form, _) = WordForm::new(&layout, &ring, &norm, eta, &field, false).unwrap();
            let norm_eq = eq_table(&norm.point, &field).unwrap();
            let source = scrambled_source(&layout);
            let target = dense_target(&form, &norm_eq, &source);
            let mut prover = Blake3Transcript::new();
            let output = prove(
                &mut prover,
                &form,
                &source,
                target,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            let mut verifier = Blake3Transcript::new();
            let (point, final_claim) = output
                .proof
                .verify(&mut verifier, target, form.word_vars(), &field)
                .unwrap();
            assert_eq!(point, output.point);
            assert_eq!(
                prover.get_challenge::<u128>(),
                verifier.get_challenge::<u128>()
            );
            // Dense terminal tables.
            let eq = eq_table(&point, &field).unwrap();
            let words = Words {
                rows: source.rows(),
                per_column: layout.bitz_params().rows() / LANES,
            };
            let mut decoded = field.zero();
            let mut spare_bits = field.zero();
            let mut lambda = field.zero();
            let mut spare_weight = field.zero();
            for c in 0..layout.capacity() * 2 * N {
                let pair = words.pair(c & !1);
                let lane = if c & 1 == 0 {
                    pair as u16
                } else {
                    (pair >> 16) as u16
                };
                decoded = field.add(
                    &decoded,
                    &field.mul(&eq[c], &signed(coefficient(lane).into(), &field)),
                );
                if spare(lane) {
                    spare_bits = field.add(&spare_bits, &eq[c]);
                }
                lambda = field.add(
                    &lambda,
                    &field.mul(&eq[c], &bit_weight(&form, &norm_eq, LANES * c)),
                );
                spare_weight = field.add(
                    &spare_weight,
                    &field.mul(&eq[c], &bit_weight(&form, &norm_eq, LANES * c + LANES - 1)),
                );
            }
            assert_eq!(output.terminal, [decoded, spare_bits]);
            assert_eq!(form.coefficient_weight_at(&point).unwrap(), lambda);
            assert_eq!(form.spare_weight_at(&point).unwrap(), spare_weight);
            assert_eq!(
                final_claim,
                field.add(
                    &field.mul(&lambda, &decoded),
                    &field.mul(&spare_weight, &spare_bits)
                )
            );
            // The bridge statement with the lane patterns in its row weights.
            let zeta = signed(-41, &field);
            let rows = bridge_row_weights(&point, &zeta, &layout, &field).unwrap();
            let columns = eq_table(&column_point(&point, &layout), &field).unwrap();
            let p = layout.bitz_params();
            assert_eq!(rows.len(), p.rows());
            assert_eq!(columns.len(), p.cols());
            let mut opened = field.zero();
            for index in 0..layout.source_bits() {
                if source.bit(index) {
                    let weight = field.mul(
                        &unsigned(rows[index % p.rows()], &field),
                        &columns[index / p.rows()],
                    );
                    opened = field.add(&opened, &weight);
                }
            }
            assert_eq!(opened, field.add(&decoded, &field.mul(&zeta, &spare_bits)));
            // A wrong target fails in the first round.
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    &form,
                    &source,
                    field.add(&target, &field.one()),
                    &mut UngrindedRoundBoundary,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn serial_and_parallel_binders_agree() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let (ring, norm, eta) = claims(&layout, &field);
        let source = scrambled_source(&layout);
        let (reference, natural_target) =
            WordForm::new(&layout, &ring, &norm, eta, &field, false).unwrap();
        let norm_eq = eq_table(&norm.point, &field).unwrap();
        let target = dense_target(&reference, &norm_eq, &source);
        let mut transcript = Blake3Transcript::new();
        let expected = prove(
            &mut transcript,
            &reference,
            &source,
            target,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let continuation = transcript.get_challenge::<u128>();
        let expected_weight = reference.coefficient_weight_at(&expected.point).unwrap();
        let check = |parallel: bool| {
            let (form, actual_target) =
                WordForm::new(&layout, &ring, &norm, eta, &field, parallel).unwrap();
            assert_eq!(actual_target, natural_target);
            let mut transcript = Blake3Transcript::new();
            let actual = prove(
                &mut transcript,
                &form,
                &source,
                target,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            assert_eq!(actual.proof, expected.proof);
            assert_eq!(actual.point, expected.point);
            assert_eq!(actual.terminal, expected.terminal);
            assert_eq!(transcript.get_challenge::<u128>(), continuation);
            assert_eq!(
                form.coefficient_weight_at(&actual.point).unwrap(),
                expected_weight
            );
        };
        check(false);
        #[cfg(feature = "parallel")]
        for threads in [1, 8] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| check(true));
        }
    }

    #[test]
    fn noncanonical_ring_coefficients_are_rejected() {
        let field = F::make_cfg(&Uint::from((1u128 << 61) - 1)).unwrap();
        let wider = F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap();
        let invalid = wider.neg(&wider.one());
        let layout = Layout::new(3).unwrap();
        let (mut ring, norm, eta) = claims(&layout, &field);
        ring.weights_s2[N + 7] = invalid;
        for parallel in [false, true] {
            assert!(WordForm::new(&layout, &ring, &norm, eta, &field, parallel).is_err());
        }
    }
}
