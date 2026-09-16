//! Exact accumulation and folding helpers shared by outer and inner kernels.
use super::SumcheckError;
use crate::piop::spartan::SpartanField;
use crate::poly::mle::DenseMultilinearExtension;
#[cfg(test)]
use field::{CtMask, CtSelect};
use field::{Fp, RingOps};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
/// Protocol accumulation hook for shared delayed products and the independent
/// test oracle. Workers merge exact products before the shared context reduces.
#[doc(hidden)]
pub trait SumcheckProductReducer<F>: Sync
where
    F: SpartanField,
{
    type Accumulator: Send;

    type PreparedReduction<'a>: Send + Sync where Self: 'a;
    fn prepare_reduction(&self, max_terms: usize, config: &F::Config) -> Self::PreparedReduction<'_>;
    fn reduce_prepared(&self, accumulator: Self::Accumulator, prepared: &Self::PreparedReduction<'_>, config: &F::Config) -> Result<F, SumcheckError> {
        let _ = prepared;
        self.reduce(accumulator, config)
    }
    fn accumulator_zero(&self) -> Self::Accumulator;
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &F, rhs: &F);
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator);
    fn reduce(
        &self,
        accumulator: Self::Accumulator,
        config: &F::Config,
    ) -> Result<F, SumcheckError>;
    /// The caller bounds every MAC contributing to this exact accumulator,
    /// including merged workers, using public dimensions only.
    fn reduce_bounded(
        &self,
        accumulator: Self::Accumulator,
        max_terms: usize,
        config: &F::Config,
    ) -> Result<F, SumcheckError> {
        let _ = max_terms;
        self.reduce(accumulator, config)
    }
}

/// Native-linear accumulation policy used only at the u32 prover's first
/// sumcheck round and native-to-field fold boundary.
///
/// A separate trait keeps the Montgomery scale visible: these accumulators
/// contain `field * u64` terms (`R` scaling), unlike the `field * field`
/// products (`R^2` scaling) handled by [`SumcheckProductReducer`].
pub(crate) trait SumcheckLinearReducer: Sync {
    type Accumulator: Send;

    fn accumulator_zero(&self) -> Self::Accumulator;
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &Fp<2>, rhs: &u64);
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator);
    fn reduce(
        &self,
        accumulator: Self::Accumulator,
        config: &field::FpCtx<2>,
    ) -> Result<Fp<2>, SumcheckError>;
}

impl<const L: usize> SumcheckProductReducer<Fp<L>> for field::FpCtx<L> {
    type Accumulator = field::FpProductAcc<L>;
    type PreparedReduction<'a> = field::PreparedProductReduction<'a, L>;
    fn prepare_reduction(&self, max_terms: usize, _: &Self) -> Self::PreparedReduction<'_> {
        self.prepare_product_reduction(max_terms)
    }
    #[inline(always)]
    fn reduce_prepared(&self, acc: Self::Accumulator, prepared: &Self::PreparedReduction<'_>, _: &Self) -> Result<Fp<L>, SumcheckError> {
        Ok(prepared.reduce(acc))
    }
    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        Self::Accumulator::default()
    }
    #[inline]
    fn multiply_accumulate(&self, acc: &mut Self::Accumulator, lhs: &Fp<L>, rhs: &Fp<L>) {
        acc.accumulate(lhs, rhs);
    }
    #[inline]
    fn merge(&self, acc: &mut Self::Accumulator, other: Self::Accumulator) {
        *acc += other;
    }
    #[inline]
    fn reduce(&self, acc: Self::Accumulator, _field: &Self) -> Result<Fp<L>, SumcheckError> {
        Ok(field::Reduce::reduce(self, acc))
    }
    #[inline]
    fn reduce_bounded(
        &self,
        acc: Self::Accumulator,
        max_terms: usize,
        _field: &Self,
    ) -> Result<Fp<L>, SumcheckError> {
        Ok(self.reduce_product_with_public_bound(acc, max_terms))
    }
}

impl SumcheckLinearReducer for field::FpCtx<2> {
    type Accumulator = field::FpLinearAcc<2, 1>;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        field::FpLinearAcc::<2, 1>::default()
    }

    #[inline]
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &Fp<2>, rhs: &u64) {
        accumulator.accumulate(lhs, &field::Uint::from_words([*rhs]));
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += other;
    }

    #[inline]
    fn reduce(
        &self,
        accumulator: Self::Accumulator,
        config: &field::FpCtx<2>,
    ) -> Result<Fp<2>, SumcheckError> {
        Ok(field::Reduce::reduce(self, accumulator))
    }
}

/// Independent unbounded integer oracle, used only by differential tests.
#[cfg(test)]
pub(crate) struct BigUintSumcheckOracle {
    modulus: num_bigint::BigUint,
    linear_correction: num_bigint::BigUint,
    product_correction: num_bigint::BigUint,
}
#[cfg(test)]
impl BigUintSumcheckOracle {
    pub(crate) fn new(field: &field::FpCtx<2>) -> Result<Self, SumcheckError> {
        use num_bigint::BigUint;
        let modulus = BigUint::from(field.modulus_u128());
        let r = BigUint::from(1u8) << 128usize;
        let linear_correction = r.modpow(&(&modulus - BigUint::from(2u8)), &modulus);
        let product_correction = (&linear_correction * &linear_correction) % &modulus;
        Ok(Self {
            modulus,
            linear_correction,
            product_correction,
        })
    }
    fn finish(
        &self,
        value: num_bigint::BigUint,
        correction: &num_bigint::BigUint,
        field: &field::FpCtx<2>,
    ) -> Fp<2> {
        let digits = (value * correction % &self.modulus).to_u64_digits();
        let canonical = field::Uint::from_words(std::array::from_fn::<_, 2, _>(|i| {
            digits.get(i).copied().unwrap_or(0)
        }));
        field::IntegerEmbedding::from_integer(field, &canonical)
    }
}
#[cfg(test)]
impl SumcheckProductReducer<Fp<2>> for BigUintSumcheckOracle {
    type Accumulator = num_bigint::BigUint;
    type PreparedReduction<'a> = ();
    fn prepare_reduction(&self, _: usize, _: &field::FpCtx<2>) {}

    fn accumulator_zero(&self) -> Self::Accumulator {
        num_bigint::BigUint::from(0u8)
    }
    fn multiply_accumulate(&self, acc: &mut Self::Accumulator, lhs: &Fp<2>, rhs: &Fp<2>) {
        *acc += num_bigint::BigUint::from(u128::from(*lhs.as_montgomery_integer()))
            * num_bigint::BigUint::from(u128::from(*rhs.as_montgomery_integer()));
    }
    fn merge(&self, acc: &mut Self::Accumulator, other: Self::Accumulator) {
        *acc += other;
    }
    fn reduce(
        &self,
        acc: Self::Accumulator,
        field: &field::FpCtx<2>,
    ) -> Result<Fp<2>, SumcheckError> {
        Ok(self.finish(acc, &self.product_correction, field))
    }
}
#[cfg(test)]
impl SumcheckLinearReducer for BigUintSumcheckOracle {
    type Accumulator = num_bigint::BigUint;
    fn accumulator_zero(&self) -> Self::Accumulator {
        num_bigint::BigUint::from(0u8)
    }
    fn multiply_accumulate(&self, acc: &mut Self::Accumulator, lhs: &Fp<2>, rhs: &u64) {
        *acc += num_bigint::BigUint::from(u128::from(*lhs.as_montgomery_integer())) * rhs;
    }
    fn merge(&self, acc: &mut Self::Accumulator, other: Self::Accumulator) {
        *acc += other;
    }
    fn reduce(
        &self,
        acc: Self::Accumulator,
        field: &field::FpCtx<2>,
    ) -> Result<Fp<2>, SumcheckError> {
        Ok(self.finish(acc, &self.linear_correction, field))
    }
}

#[inline]
pub(crate) fn merge_accumulators<F, R, const COEFFS: usize>(
    mut left: [R::Accumulator; COEFFS],
    right: [R::Accumulator; COEFFS],
    reducer: &R,
) -> [R::Accumulator; COEFFS]
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    for (left, right) in left.iter_mut().zip(right) {
        reducer.merge(left, right);
    }
    left
}

#[inline]
pub(crate) fn reduce_two_accumulators<F, R>(
    accumulators: [R::Accumulator; 2],
    reducer: &R,
    config: &F::Config,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let [c0, c2] = accumulators;
    Ok([reducer.reduce(c0, config)?, reducer.reduce(c2, config)?])
}

#[inline(always)]
pub(crate) fn reduce_two_prepared<F: SpartanField, R: SumcheckProductReducer<F>>(
    [a, b]: [R::Accumulator; 2], reducer: &R, prepared: &R::PreparedReduction<'_>, config: &F::Config,
) -> Result<[F; 2], SumcheckError> {
    Ok([reducer.reduce_prepared(a, prepared, config)?, reducer.reduce_prepared(b, prepared, config)?])
}

#[inline]
pub(crate) fn reduce_two_accumulators_bounded<F, R>(
    accumulators: [R::Accumulator; 2],
    reducer: &R,
    max_terms: usize,
    config: &F::Config,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let [a, b] = accumulators;
    Ok([
        reducer.reduce_bounded(a, max_terms, config)?,
        reducer.reduce_bounded(b, max_terms, config)?,
    ])
}

pub(crate) fn sum_product_accumulators<F, R, const COEFFS: usize>(
    len: usize,
    contribution: impl Fn(&mut [R::Accumulator; COEFFS], usize) + Sync,
    reducer: &R,
) -> [R::Accumulator; COEFFS]
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    #[cfg(feature = "parallel")]
    if should_parallelize(len) {
        return (0..len)
            .into_par_iter()
            .fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |mut accumulators, index| {
                    contribution(&mut accumulators, index);
                    accumulators
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
    }

    (0..len).fold(
        std::array::from_fn(|_| reducer.accumulator_zero()),
        |mut accumulators, index| {
            contribution(&mut accumulators, index);
            accumulators
        },
    )
}

#[inline]
#[cfg(test)]
pub(crate) fn native_to_field(value: u64, field_cfg: &field::FpCtx<2>) -> Fp<2> {
    Fp::<2>::from_with_cfg(value, field_cfg)
}

#[inline]
#[cfg(test)]
pub(crate) fn native_u32_product(left: u64, right: u64) -> u64 {
    debug_assert!(left <= u64::from(u32::MAX));
    debug_assert!(right <= u64::from(u32::MAX));
    // Both operands were validated before the transcript was mutated.
    left * right
}

#[cfg(test)]
pub(crate) fn sum_linear_accumulators<R, const COEFFS: usize>(
    len: usize,
    contribution: impl Fn(&mut [<R as SumcheckLinearReducer>::Accumulator; COEFFS], usize) + Sync,
    reducer: &R,
) -> [<R as SumcheckLinearReducer>::Accumulator; COEFFS]
where
    R: SumcheckLinearReducer,
{
    #[cfg(feature = "parallel")]
    if should_parallelize(len) {
        return (0..len)
            .into_par_iter()
            .fold(
                || std::array::from_fn(|_| <R as SumcheckLinearReducer>::accumulator_zero(reducer)),
                |mut accumulators, index| {
                    contribution(&mut accumulators, index);
                    accumulators
                },
            )
            .reduce(
                || std::array::from_fn(|_| <R as SumcheckLinearReducer>::accumulator_zero(reducer)),
                |mut left, right| {
                    for (left, right) in left.iter_mut().zip(right) {
                        <R as SumcheckLinearReducer>::merge(reducer, left, right);
                    }
                    left
                },
            );
    }

    (0..len).fold(
        std::array::from_fn(|_| <R as SumcheckLinearReducer>::accumulator_zero(reducer)),
        |mut accumulators, index| {
            contribution(&mut accumulators, index);
            accumulators
        },
    )
}

#[inline]
#[cfg(test)]
pub(crate) fn reduce_two_linear_accumulators<R>(
    accumulators: [<R as SumcheckLinearReducer>::Accumulator; 2],
    reducer: &R,
    config: &field::FpCtx<2>,
) -> Result<[Fp<2>; 2], SumcheckError>
where
    R: SumcheckLinearReducer,
{
    let [endpoint, infinity] = accumulators;
    Ok([
        <R as SumcheckLinearReducer>::reduce(reducer, endpoint, config)?,
        <R as SumcheckLinearReducer>::reduce(reducer, infinity, config)?,
    ])
}

#[inline]
#[cfg(test)]
pub(crate) fn multiply_accumulate_signed_linear<R>(
    accumulator: &mut <R as SumcheckLinearReducer>::Accumulator,
    weight: &Fp<2>,
    negative_weight: &Fp<2>,
    value: i128,
    reducer: &R,
) where
    R: SumcheckLinearReducer,
{
    // Branch-free signed magnitude. Both native outer expressions are proven
    // below to have magnitude at most `u64::MAX`.
    let sign_mask = (value >> 127) as u128;
    let magnitude = ((value as u128) ^ sign_mask).wrapping_sub(sign_mask) as u64;
    let is_negative = CtMask::from_lsb(sign_mask as u64);
    let selected_weight = CtSelect::ct_select(weight, negative_weight, is_negative);
    <R as SumcheckLinearReducer>::multiply_accumulate(
        reducer,
        accumulator,
        &selected_weight,
        &magnitude,
    );
}

#[cfg(test)]
pub(crate) fn fold_u64_table_to_field<R>(
    input: &[u64],
    output: &mut [Fp<2>],
    challenge: &Fp<2>,
    zero: &Fp<2>,
    reducer: &R,
    field_config: &crate::piop::spartan::protocol::FieldConfig,
) -> Result<(), SumcheckError>
where
    R: SumcheckLinearReducer,
{
    debug_assert_eq!(input.len(), 2 * output.len());
    let one = Fp::<2>::one_with_cfg(&field_config);
    let one_minus_challenge = (field_config).sub(&one, challenge);

    let fold_pair = |pair: &[u64], value: &mut Fp<2>| -> Result<(), SumcheckError> {
        let mut accumulator = <R as SumcheckLinearReducer>::accumulator_zero(reducer);
        <R as SumcheckLinearReducer>::multiply_accumulate(
            reducer,
            &mut accumulator,
            &one_minus_challenge,
            &pair[0],
        );
        <R as SumcheckLinearReducer>::multiply_accumulate(
            reducer,
            &mut accumulator,
            challenge,
            &pair[1],
        );
        *value = <R as SumcheckLinearReducer>::reduce(reducer, accumulator, &field_config)?;
        Ok(())
    };

    #[cfg(feature = "parallel")]
    if should_parallelize(output.len()) {
        return input
            .par_chunks_exact(2)
            .zip(output.par_iter_mut())
            .try_for_each(|(pair, value)| fold_pair(pair, value));
    }

    for (pair, value) in input.chunks_exact(2).zip(output) {
        fold_pair(pair, value)?;
    }

    Ok(())
}

#[cfg(feature = "parallel")]
pub(crate) const PARALLEL_SUMCHECK_THRESHOLD: usize = 1 << 12;

#[cfg(feature = "parallel")]
#[inline]
pub(crate) fn should_parallelize(work_items: usize) -> bool {
    work_items >= PARALLEL_SUMCHECK_THRESHOLD && rayon::current_num_threads() > 1
}

/// Fused outer folds perform six interpolations per output pair. Smaller
/// public tails still justify four independent 512-pair blocks.
#[cfg(feature = "parallel")]
pub(crate) fn parallel_outer_fold(pairs: usize) -> bool {
    pairs >= 2048 && rayon::current_num_threads() > 1
}
pub(crate) fn outer_fold_grain(pairs: usize) -> usize {
    if pairs < 8192 { 512 } else { 1024 }
}

#[inline]
pub(crate) fn interpolate_pair<F>(zero: &F, one: &F, challenge: &F, field_config: &F::Config) -> F
where
    F: SpartanField,
{
    (field_config).add(
        zero,
        &(field_config).mul(challenge, &(field_config).sub(one, zero)),
    )
}

pub(crate) fn fold_table<F>(input: &[F], output: &mut [F], challenge: &F, field_config: &F::Config)
where
    F: SpartanField,
{
    debug_assert_eq!(input.len(), 2 * output.len());

    #[cfg(feature = "parallel")]
    if should_parallelize(output.len()) {
        input
            .par_chunks_exact(2)
            .zip(output.par_iter_mut())
            .for_each(|(pair, value)| {
                *value = interpolate_pair(&pair[0], &pair[1], challenge, &field_config);
            });
        return;
    }

    input
        .chunks_exact(2)
        .zip(output.iter_mut())
        .for_each(|(pair, value)| {
            *value = interpolate_pair(&pair[0], &pair[1], challenge, &field_config);
        });
}

pub(crate) fn has_dense_shape<F>(mle: &DenseMultilinearExtension<F>) -> bool {
    mle.num_vars < usize::BITS as usize && mle.evaluations.len() == 1usize << mle.num_vars
}

#[inline(always)]
pub(crate) fn fold_two_pairs<F>(values: &[F], challenge: &F, field_config: &F::Config) -> [F; 2]
where
    F: SpartanField,
{
    debug_assert_eq!(values.len(), 4);
    [
        interpolate_pair(&values[0], &values[1], challenge, &field_config),
        interpolate_pair(&values[2], &values[3], challenge, &field_config),
    ]
}
