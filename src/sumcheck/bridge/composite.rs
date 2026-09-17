//! A repeated local factor followed by a corrected circuit tail. This owner
//! is shared by binding and the existing structured inner-sumcheck MLE adapter.
use super::BindingError;
use crate::piop::spartan::{f2z::SpartanF2zField as F, matrix::make_equality_factors};
use crate::poly::mle::{
    CompositeMultilinearExtension, EqualityWeights, FactoredMultilinearExtension,
};
use crate::sumcheck::inner::native::{
    Raw, RawFieldStorage, make_equality_factors_raw, raw_to_words,
};
use circuit::matrix_wengert::ForwardColumns;
use field::RingOps;
type Config = field::FpCtx<2>;
type Result<T> = std::result::Result<T, BindingError>;
/// Owned public matrix MLE. Field elements already store Montgomery evaluations.
/// For `N` SHA instances and `H` local wires, its Bit table is
/// `[sha_local_evaluations ⊗ instance_weights, p256_evaluations]`:
/// entry `i + N*j` is `instance_weights[i] * sha_local_evaluations[j]`,
/// with `constant_weight` added at index zero and implicit zero padding.
pub(crate) struct CompositeCoefficients {
    /// Arithmetic for the cached P-256 evaluations.
    pub(crate) ctx: field::FpCtx<2>,
    /// Assignment domain size is `2^num_vars`, including zero padding.
    pub(crate) num_vars: usize,
    /// `N = 2^log_compressions` instance weights.
    pub(crate) instance_weights: Vec<F>,
    /// `H = SHA_H = 20,457` local evaluations.
    pub(crate) sha_local_evaluations: Vec<F>,
    /// `P = 1,215,663` evaluations for the current P-256 circuit.
    pub(crate) p256_evaluations: Vec<F>,
    /// Weight of the equation `witness[0] = 1`.
    pub(crate) constant_weight: F,
    /// `N · H`, the first P-256 assignment index.
    pub(crate) p256_assignment_offset: usize,
    /// Geometric runs of `p256_evaluations` (from the tape's power groups, split
    /// around the public-bit cells): `(start, len, base)` with
    /// `p256_evaluations[start + k] = base · 2^k`. Prover-side structure only.
    pub(crate) tail_runs: Vec<(usize, usize, F)>,
}

impl CompositeCoefficients {
    pub(crate) fn as_mle(&self, cfg: &Config) -> Result<CompositeMultilinearExtension<'_, F>> {
        CompositeMultilinearExtension::from_parts(
            self.num_vars,
            &self.sha_local_evaluations,
            &self.instance_weights,
            &self.p256_evaluations,
            self.constant_weight.clone(),
            cfg,
        )
        .and_then(|mle| mle.with_tail_runs(&self.tail_runs))
        .map_err(|_| BindingError::InvalidProductDimensions)
    }

    pub(crate) fn evaluate(&self, assignment_point: &[F], cfg: &Config) -> Result<F> {
        let _scope = tracing::info_span!("ecdsa:coefficient_evaluate").entered();
        if self.num_vars != assignment_point.len() {
            return Err(BindingError::InvalidProductDimensions);
        }
        let equality = equality_weights(assignment_point, cfg)?;
        let mut value = evaluate_sha_factors(
            &self.instance_weights,
            &self.sha_local_evaluations,
            assignment_point,
            cfg,
        )?;
        value = cfg.add(
            &(value),
            &(&(cfg.mul(&(self.constant_weight.clone()), &(&equality.at(0))))),
        );
        value = cfg.add(
            &(value),
            &(&evaluate_tail_by_runs(
                cfg,
                &self.ctx,
                self.p256_assignment_offset,
                &self.p256_evaluations,
                &self.tail_runs,
                &equality,
            )),
        );
        Ok(value)
    }
}

/// The tail cells' equality weights as forward-pass column values. A scalar
/// column `j` is `eq(offset + j) = low[(offset + j) mod L] · high[(offset + j) / L]`;
/// a power group's `Σ_{k<len} 2^k · eq(offset + first + k)` uses the backward
/// recurrence of [`evaluate_tail_by_runs`] (`Q[t] = Σ_{i ≥ t} 2^(i − t) · low[i]`,
/// no inverses): the group's intersection with one high block `[lo, hi)`
/// contributes `high[b] · (Q[lo] − 2^(hi − lo) · Q[hi])` times the running
/// power of two, so a group costs a few multiplications per block it spans.
pub(crate) struct TailEqualityColumns {
    equality: RawEqualityWeights,
    offset: usize,
    /// `Q[t] = low[t] + 2 · Q[t + 1]`, `Q[L] = 0`.
    suffix: Vec<Raw>,
    /// `2^k` in Montgomery form for `k ≤ L`.
    pow2: Vec<Raw>,
}

impl TailEqualityColumns {
    /// The equality tables of `point`, split at half the point (as
    /// `make_equality_factors` splits them).
    pub(crate) fn new(ctx: &field::FpCtx<2>, offset: usize, point: &[F]) -> Self {
        let equality = RawEqualityWeights::new(ctx, point);
        let block = equality.low.len();
        let mut suffix = vec![0 as Raw; block + 1];
        for t in (0..block).rev() {
            let doubled = ctx.add_raw(suffix[t + 1], suffix[t + 1]);
            suffix[t] = ctx.add_raw(equality.low[t], doubled);
        }
        let mut pow2 = Vec::with_capacity(block + 1);
        pow2.push(ctx.native_residue(1));
        for k in 0..block {
            pow2.push(ctx.add_raw(pow2[k], pow2[k]));
        }
        Self {
            equality,
            offset,
            suffix,
            pow2,
        }
    }

    /// `eq(point, index)` over the whole assignment domain.
    #[inline]
    pub(crate) fn eq_at(&self, index: usize) -> Raw {
        self.equality.at(index)
    }
}

impl ForwardColumns for TailEqualityColumns {
    fn scalar(&self, column: usize) -> [u64; 2] {
        raw_to_words(self.eq_at(self.offset + column))
    }

    fn power_sum(&self, first: usize, len: usize) -> [u64; 2] {
        let equality = &self.equality;
        let ctx = &equality.ctx;
        let block = equality.low.len();
        let mut sum = 0 as Raw;
        let mut base = self.pow2[0];
        let mut index = self.offset + first;
        let end = index + len;
        while index < end {
            let b = index >> equality.shift;
            let lo = index & equality.mask;
            let hi = (lo + (end - index)).min(block);
            let part = ctx.sub_raw(
                self.suffix[lo],
                ctx.mul_raw(self.pow2[hi - lo], self.suffix[hi]),
            );
            sum = ctx.add_raw(sum, ctx.mul_raw(ctx.mul_raw(base, part), equality.high[b]));
            base = ctx.mul_raw(base, self.pow2[hi - lo]);
            index += hi - lo;
        }
        raw_to_words(sum)
    }
}

/// [`EqualityWeights`] in raw residues: `eq(index) = low[index mod L] · high[index / L]`
/// with the tables split at half the point, as `make_equality_factors` splits
/// them, so every weight is the same residue one raw product later.
pub(crate) struct RawEqualityWeights {
    ctx: field::FpCtx<2>,
    low: Vec<Raw>,
    high: Vec<Raw>,
    shift: u32,
    mask: usize,
}

impl RawEqualityWeights {
    pub(crate) fn new(ctx: &field::FpCtx<2>, point: &[F]) -> Self {
        let (low, high) = make_equality_factors_raw(ctx, point);
        let block = low.len();
        Self {
            ctx: ctx.clone(),
            low,
            high,
            shift: block.ilog2(),
            mask: block - 1,
        }
    }

    #[inline]
    pub(crate) fn at(&self, index: usize) -> Raw {
        self.ctx
            .mul_raw(self.low[index & self.mask], self.high[index >> self.shift])
    }

    /// `Σ_j values[j] · eq(j)`, with the high factor applied once per block:
    /// `Σ_h high[h] · Σ_l low[l] · values[h·L + l]`.
    pub(crate) fn dot(&self, values: &[Raw]) -> Raw {
        let ctx = &self.ctx;
        let mut sum = 0 as Raw;
        for (h, block) in values.chunks(self.low.len()).enumerate() {
            let inner = block
                .iter()
                .zip(&self.low)
                .fold(0 as Raw, |acc, (&value, &low)| {
                    ctx.add_raw(acc, ctx.mul_raw(value, low))
                });
            sum = ctx.add_raw(sum, ctx.mul_raw(inner, self.high[h]));
        }
        sum
    }
}

pub(crate) fn equality_weights(point: &[F], cfg: &Config) -> Result<EqualityWeights<F>> {
    let (low, high) =
        make_equality_factors(point, cfg).map_err(|_| BindingError::InvalidProductDimensions)?;
    Ok(EqualityWeights::from_tables(
        low.evaluations,
        high.evaluations,
        cfg,
    ))
}

pub(crate) fn evaluate_sha_factors(
    instances: &[F],
    sha: &[F],
    point: &[F],
    cfg: &Config,
) -> Result<F> {
    FactoredMultilinearExtension::from_factors(point.len(), sha, instances, cfg)
        .and_then(|mle| mle.evaluate(point, cfg))
        .map_err(|_| BindingError::InvalidProductDimensions)
}

/// `Σ_j tail[j] · eq(point, offset + j)` where `tail` is geometric on `runs`
/// (`tail[start + k] = base · 2^k`) and arbitrary elsewhere. With
/// `eq(offset + j) = low[(offset + j) mod L] · high[(offset + j) / L]`, a run's
/// intersection with one high block `[lo, hi)` contributes
/// `base_t · high[b] · (Q[lo] − 2^(hi − lo) · Q[hi])` where
/// `Q[t] = Σ_{i ≥ t} 2^(i − t) · low[i]` (a backward recurrence, no inverses),
/// so the runs cost a few multiplications each; columns outside every run pay
/// two multiplications.
pub(crate) fn evaluate_tail_by_runs(
    cfg: &Config,
    ctx: &field::FpCtx<2>,
    offset: usize,
    tail: &[F],
    runs: &[(usize, usize, F)],
    equality: &EqualityWeights<F>,
) -> F {
    let (low, high) = (ctx.raw_vec(equality.low()), ctx.raw_vec(equality.high()));
    let block = low.len();
    let shift = block.ilog2();
    let mask = block - 1;
    // Q[t] = low[t] + 2 · Q[t + 1], Q[block] = 0.
    let mut suffix = vec![0 as Raw; block + 1];
    for t in (0..block).rev() {
        let doubled = ctx.add_raw(suffix[t + 1], suffix[t + 1]);
        suffix[t] = ctx.add_raw(low[t], doubled);
    }
    // 2^k in Montgomery form for k ≤ block.
    let mut pow2 = Vec::with_capacity(block + 1);
    pow2.push(ctx.native_residue(1));
    for k in 0..block {
        pow2.push(ctx.add_raw(pow2[k], pow2[k]));
    }
    let mut sum = 0 as Raw;
    let mut cursor = 0usize;
    let scalar = |sum: &mut Raw, from: usize, to: usize| {
        for column in from..to {
            let index = offset + column;
            let weight = ctx.mul_raw(low[index & mask], high[index >> shift]);
            *sum = ctx.add_raw(*sum, ctx.mul_raw(weight, ctx.raw(&tail[column])));
        }
    };
    for &(start, len, ref base) in runs {
        scalar(&mut sum, cursor, start);
        let mut base_t = ctx.raw(base);
        let mut index = offset + start;
        let end = offset + start + len;
        while index < end {
            let b = index >> shift;
            let lo = index & mask;
            let hi = (lo + (end - index)).min(block);
            let part = ctx.sub_raw(suffix[lo], ctx.mul_raw(pow2[hi - lo], suffix[hi]));
            sum = ctx.add_raw(sum, ctx.mul_raw(ctx.mul_raw(base_t, part), high[b]));
            base_t = ctx.mul_raw(base_t, pow2[hi - lo]);
            index += hi - lo;
        }
        cursor = start + len;
    }
    scalar(&mut sum, cursor, tail.len());
    crate::utils::delayed_reduction::element(&cfg, sum)
}

/// Challenge-dependent seeds, already factored by the relation adapter. Public
/// corrections may alias and are added, never overwritten.
pub(crate) struct CompositeRows<'a> {
    pub instances: &'a [Raw],
    pub local: &'a [Raw],
    pub tail_rows: &'a [Raw],
    pub correction_columns: &'a [usize],
    pub corrections: &'a [Raw],
    pub constant: Raw,
}
pub(crate) struct CompositeBinding<'a, 't> {
    pub field: &'a Config,
    pub tape: &'a mut circuit::matrix_wengert::PreparedWengertEvaluator<'t>,
    pub num_vars: usize,
    pub tail_offset: usize,
    pub tail_columns: usize,
}
impl CompositeBinding<'_, '_> {
    fn validate(&self, rows: &CompositeRows<'_>) -> Result<()> {
        let domain = 1usize
            .checked_shl(self.num_vars as u32)
            .ok_or(BindingError::InvalidProductDimensions)?;
        if !rows.instances.len().is_power_of_two()
            || rows.instances.len().checked_mul(rows.local.len()) != Some(self.tail_offset)
            || self
                .tail_offset
                .checked_add(self.tail_columns)
                .is_none_or(|n| n > domain)
            || rows.corrections.len() != rows.correction_columns.len()
            || rows
                .correction_columns
                .iter()
                .any(|&i| i >= self.tail_columns)
            || rows.tail_rows.len() != 3 * self.tape.row_count()
            || self.tail_columns != self.tape.column_count()
        {
            return Err(BindingError::InvalidProductDimensions);
        }
        Ok(())
    }
}
impl super::PreparedBinding<Config, CompositeRows<'_>> for CompositeBinding<'_, '_> {
    type Bound = CompositeCoefficients;
    fn bind_rows(&mut self, rows: &CompositeRows<'_>) -> Result<Self::Bound> {
        let mut out = CompositeCoefficients {
            ctx: self.field.clone(),
            num_vars: self.num_vars,
            instance_weights: Vec::new(),
            sha_local_evaluations: Vec::new(),
            p256_evaluations: Vec::new(),
            constant_weight: self.field.zero(),
            p256_assignment_offset: self.tail_offset,
            tail_runs: Vec::new(),
        };
        self.bind_rows_into(rows, &mut out)?;
        Ok(out)
    }
    fn bind_rows_into(&mut self, rows: &CompositeRows<'_>, out: &mut Self::Bound) -> Result<()> {
        self.validate(rows)?;
        let field = self.field;
        if out.p256_evaluations.capacity() == 0 {
            out.p256_evaluations = field.zero_vec(self.tail_columns);
        } else {
            out.p256_evaluations.resize(self.tail_columns, field.zero());
        }
        self.tape
            .adjoint_map_into(
                rows.tail_rows.len() / 3,
                |r, k| raw_to_words(rows.tail_rows[3 * r + k]),
                &mut out.p256_evaluations,
            )
            .map_err(|_| BindingError::InvalidProductDimensions)?;
        out.tail_runs = split_runs(field, self.tape.power_runs(), rows.correction_columns);
        for (&column, &value) in rows.correction_columns.iter().zip(rows.corrections) {
            out.p256_evaluations[column] = field.add(
                &out.p256_evaluations[column],
                &crate::sumcheck::inner::native::shared_raw(field, value),
            );
        }
        let fill = |src: &[Raw], dst: &mut Vec<F>| {
            dst.clear();
            dst.extend(
                src.iter()
                    .map(|&r| crate::sumcheck::inner::native::shared_raw(field, r)),
            );
        };
        fill(rows.instances, &mut out.instance_weights);
        fill(rows.local, &mut out.sha_local_evaluations);
        out.ctx = field.clone();
        out.num_vars = self.num_vars;
        out.p256_assignment_offset = self.tail_offset;
        out.constant_weight = crate::sumcheck::inner::native::shared_raw(field, rows.constant);
        Ok(())
    }
    fn evaluate_bound(&mut self, rows: &CompositeRows<'_>, point: &[F]) -> Result<F> {
        self.validate(rows)?;
        if point.len() != self.num_vars {
            return Err(BindingError::InvalidProductDimensions);
        }
        let field = self.field;
        let columns = {
            let _s = tracing::info_span!("ecdsa:ce_columns").entered();
            TailEqualityColumns::new(field, self.tail_offset, point)
        };
        let mut value = {
            let _s = tracing::info_span!("ecdsa:ce_sha_eval").entered();
            let split = rows.instances.len().ilog2() as usize;
            let instances = RawEqualityWeights::new(field, &point[..split]);
            let local = RawEqualityWeights::new(field, &point[split..]);
            field.mul_raw(instances.dot(rows.instances), local.dot(rows.local))
        };
        value = field.add_raw(value, field.mul_raw(rows.constant, columns.eq_at(0)));
        let tail = {
            let _s = tracing::info_span!("ecdsa:ce_forward").entered();
            self.tape
                .evaluate_bilinear_map(
                    rows.tail_rows.len() / 3,
                    |r, k| raw_to_words(rows.tail_rows[3 * r + k]),
                    &columns,
                )
                .map_err(|_| BindingError::InvalidProductDimensions)?
        };
        value = field.add_raw(value, crate::sumcheck::inner::native::words_to_raw(&tail));
        for (&column, &weight) in rows.correction_columns.iter().zip(rows.corrections) {
            value = field.add_raw(
                value,
                field.mul_raw(weight, columns.eq_at(self.tail_offset + column)),
            );
        }
        Ok(crate::sumcheck::inner::native::shared_raw(field, value))
    }
}

/// Split geometric runs around corrections. Repeated/cancelling correction
/// indices are harmless: each exceptional coordinate is removed exactly once.
pub(crate) fn split_runs(
    field: &Config,
    runs: Vec<circuit::matrix_wengert::PowerRun>,
    exceptions: &[usize],
) -> Vec<(usize, usize, F)> {
    let mut exceptions = exceptions.to_vec();
    exceptions.sort_unstable();
    exceptions.dedup();
    let mut out = Vec::with_capacity(runs.len() + exceptions.len());
    for run in runs {
        let mut start = run.first_column;
        let end = start + run.len;
        let mut base = field.from_montgomery_integer(field::Uint::from_words(run.base));
        for &cell in &exceptions[exceptions.partition_point(|&i| i < start)..] {
            if cell >= end {
                break;
            }
            if cell > start {
                out.push((start, cell - start, base));
            }
            for _ in start..=cell {
                base = field.add(&base, &base);
            }
            start = cell + 1;
        }
        if start < end {
            out.push((start, end - start, base));
        }
    }
    out
}
