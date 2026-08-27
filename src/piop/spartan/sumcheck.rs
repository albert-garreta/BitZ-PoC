//! Spartan's equality-weighted outer sumcheck and matrix-product inner sumcheck.
//!
//! Round polynomials are stored in coefficient form. The outer proof uses four
//! coefficients (degree three), while the inner proof uses three (degree two).
//! The prover omits the linear coefficient while accumulating a round and
//! reconstructs it from `g(0) + g(1) = current_claim` before absorption.

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    poly::mle::DenseMultilinearExtension,
    transcript::traits::Transcript,
    utils::delayed_reduction::{
        Accumulatable, CryptoBigintMonty128Reducer, DelayedReductionError,
        MontyLinearAccumulator128, MontyProductAccumulator128, OptimizedMonty128Reducer, Reduce,
    },
};
use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_monty::MontyField};
use num_traits::Zero;

use super::{SpartanField, absorb_field_elements, squeeze_field};

/// Product accumulation policy used by the sumcheck prover.
///
/// The immediate implementation stores a reduced field element. Delayed
/// implementations use a wide integer and reduce only after all worker-local
/// accumulators have been merged. Keeping the policy monomorphized avoids a
/// strategy branch in the product loops.
pub(crate) trait SumcheckProductReducer<F>: Sync
where
    F: SpartanField,
{
    type Accumulator: Send;

    /// Whether this monomorphized reducer should factor split equality
    /// weights into two delayed-accumulation levels.
    const FACTOR_SPLIT_EQUALITY: bool;

    fn accumulator_zero(&self) -> Self::Accumulator;
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &F, rhs: &F);
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator);
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<F, SumcheckError>;
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
    fn multiply_accumulate(
        &self,
        accumulator: &mut Self::Accumulator,
        lhs: &MontyField<2>,
        rhs: &u64,
    );
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator);
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<MontyField<2>, SumcheckError>;
}

/// How the native `u64` witness is folded into the field after round zero.
/// Selection happens once per table, before the pair loop.
#[cfg(any(test, feature = "bench-internals"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeWitnessFoldPolicy {
    Immediate,
    Delayed,
}

/// How field-valued inner-round coefficient sums are accumulated.
///
/// The threshold form permits a delayed large-table prefix followed by an
/// immediate tail. The decision is made once per round, outside every MAC
/// loop.
#[cfg(any(test, feature = "bench-internals"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldCoefficientPolicy {
    Immediate,
    Delayed,
    DelayedAtOrAbovePairs(usize),
}

#[cfg(any(test, feature = "bench-internals"))]
impl FieldCoefficientPolicy {
    #[inline]
    const fn use_delayed(self, pair_count: usize) -> bool {
        match self {
            Self::Immediate => false,
            Self::Delayed => true,
            Self::DelayedAtOrAbovePairs(minimum_pairs) => pair_count >= minimum_pairs,
        }
    }
}

trait U32InnerArithmeticPolicy: Sync {
    fn native_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[u64],
        zero: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError>;

    fn fold_native_witness(
        &self,
        input: &[u64],
        output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
        zero: &MontyField<2>,
        field_cfg: &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<(), SumcheckError>;

    fn field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
    ) -> Result<[MontyField<2>; 2], SumcheckError>;

    fn fold_and_compute_field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
        batched_matrix_output: &mut [MontyField<2>],
        witness_output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError>;
}

struct DelayedU32InnerArithmetic<'a, R> {
    reducer: &'a R,
}

impl<R> U32InnerArithmeticPolicy for DelayedU32InnerArithmetic<'_, R>
where
    R: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
{
    #[inline]
    fn native_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[u64],
        zero: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        sum_u32_native_inner_coefficients_without_linear(
            batched_matrix,
            witness,
            zero,
            self.reducer,
        )
    }

    #[inline]
    fn fold_native_witness(
        &self,
        input: &[u64],
        output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
        zero: &MontyField<2>,
        _field_cfg: &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<(), SumcheckError> {
        fold_u64_table_to_field(input, output, challenge, zero, self.reducer)
    }

    #[inline]
    fn field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        sum_inner_round_coefficients_without_linear(batched_matrix, witness, self.reducer)
    }

    #[inline]
    fn fold_and_compute_field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
        batched_matrix_output: &mut [MontyField<2>],
        witness_output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        fold_and_compute_next_inner_round_coefficients_without_linear(
            batched_matrix,
            witness,
            batched_matrix_output,
            witness_output,
            challenge,
            self.reducer,
        )
    }
}

#[cfg(any(test, feature = "bench-internals"))]
struct SelectedU32InnerArithmetic<'a, NR, DR, IR> {
    native_reducer: &'a NR,
    delayed_field_reducer: &'a DR,
    immediate_field_reducer: &'a IR,
    native_fold_policy: NativeWitnessFoldPolicy,
    field_coefficient_policy: FieldCoefficientPolicy,
}

#[cfg(any(test, feature = "bench-internals"))]
impl<NR, DR, IR> U32InnerArithmeticPolicy for SelectedU32InnerArithmetic<'_, NR, DR, IR>
where
    NR: SumcheckLinearReducer,
    DR: SumcheckProductReducer<MontyField<2>>,
    IR: SumcheckProductReducer<MontyField<2>>,
{
    #[inline]
    fn native_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[u64],
        zero: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        sum_u32_native_inner_coefficients_without_linear(
            batched_matrix,
            witness,
            zero,
            self.native_reducer,
        )
    }

    #[inline]
    fn fold_native_witness(
        &self,
        input: &[u64],
        output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
        zero: &MontyField<2>,
        field_cfg: &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<(), SumcheckError> {
        match self.native_fold_policy {
            NativeWitnessFoldPolicy::Immediate => {
                fold_u64_table_to_field_immediate(input, output, challenge, field_cfg);
                Ok(())
            }
            NativeWitnessFoldPolicy::Delayed => {
                fold_u64_table_to_field(input, output, challenge, zero, self.native_reducer)
            }
        }
    }

    #[inline]
    fn field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        sum_inner_round_coefficients_selected(
            batched_matrix,
            witness,
            self.delayed_field_reducer,
            self.immediate_field_reducer,
            self.field_coefficient_policy,
        )
    }

    #[inline]
    fn fold_and_compute_field_coefficients(
        &self,
        batched_matrix: &[MontyField<2>],
        witness: &[MontyField<2>],
        batched_matrix_output: &mut [MontyField<2>],
        witness_output: &mut [MontyField<2>],
        challenge: &MontyField<2>,
    ) -> Result<[MontyField<2>; 2], SumcheckError> {
        fold_and_compute_next_inner_round_selected(
            batched_matrix,
            witness,
            batched_matrix_output,
            witness_output,
            challenge,
            self.delayed_field_reducer,
            self.immediate_field_reducer,
            self.field_coefficient_policy,
        )
    }
}

/// Compatibility policy matching the original eagerly reduced prover.
pub(crate) struct ImmediateSumcheckReducer<F> {
    zero: F,
}

impl<F> ImmediateSumcheckReducer<F>
where
    F: SpartanField,
{
    pub(crate) fn new(field_cfg: &F::Config) -> Self {
        Self {
            zero: F::zero_with_cfg(field_cfg),
        }
    }
}

impl<F> SumcheckProductReducer<F> for ImmediateSumcheckReducer<F>
where
    F: SpartanField + Sync,
{
    type Accumulator = F;
    const FACTOR_SPLIT_EQUALITY: bool = false;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        self.zero.clone()
    }

    #[inline]
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &F, rhs: &F) {
        *accumulator += &mul(lhs, rhs);
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += &other;
    }

    #[inline]
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<F, SumcheckError> {
        Ok(accumulator)
    }
}

pub(crate) struct OptimizedSumcheckReducer {
    reducer: OptimizedMonty128Reducer,
}

impl OptimizedSumcheckReducer {
    pub(crate) fn new(
        field_cfg: &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<Self, SumcheckError> {
        Ok(Self {
            reducer: OptimizedMonty128Reducer::new(field_cfg)?,
        })
    }
}

impl SumcheckProductReducer<MontyField<2>> for OptimizedSumcheckReducer {
    type Accumulator = MontyProductAccumulator128;
    const FACTOR_SPLIT_EQUALITY: bool = true;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        MontyProductAccumulator128::zero()
    }

    #[inline]
    fn multiply_accumulate(
        &self,
        accumulator: &mut Self::Accumulator,
        lhs: &MontyField<2>,
        rhs: &MontyField<2>,
    ) {
        accumulator.multiply_accumulate(lhs, rhs);
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += other;
    }

    #[inline]
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<MontyField<2>, SumcheckError> {
        Ok(accumulator.reduce(&self.reducer)?)
    }
}

impl SumcheckLinearReducer for OptimizedSumcheckReducer {
    type Accumulator = MontyLinearAccumulator128;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        MontyLinearAccumulator128::zero()
    }

    #[inline]
    fn multiply_accumulate(
        &self,
        accumulator: &mut Self::Accumulator,
        lhs: &MontyField<2>,
        rhs: &u64,
    ) {
        accumulator.multiply_accumulate(lhs, rhs);
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += other;
    }

    #[inline]
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<MontyField<2>, SumcheckError> {
        Ok(accumulator.reduce(&self.reducer)?)
    }
}

pub(crate) struct CryptoBigintSumcheckReducer {
    reducer: CryptoBigintMonty128Reducer,
}

impl CryptoBigintSumcheckReducer {
    pub(crate) fn new(
        field_cfg: &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<Self, SumcheckError> {
        Ok(Self {
            reducer: CryptoBigintMonty128Reducer::new(field_cfg)?,
        })
    }
}

impl SumcheckProductReducer<MontyField<2>> for CryptoBigintSumcheckReducer {
    type Accumulator = MontyProductAccumulator128;
    const FACTOR_SPLIT_EQUALITY: bool = true;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        MontyProductAccumulator128::zero()
    }

    #[inline]
    fn multiply_accumulate(
        &self,
        accumulator: &mut Self::Accumulator,
        lhs: &MontyField<2>,
        rhs: &MontyField<2>,
    ) {
        accumulator.multiply_accumulate(lhs, rhs);
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += other;
    }

    #[inline]
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<MontyField<2>, SumcheckError> {
        Ok(accumulator.reduce(&self.reducer)?)
    }
}

impl SumcheckLinearReducer for CryptoBigintSumcheckReducer {
    type Accumulator = MontyLinearAccumulator128;

    #[inline]
    fn accumulator_zero(&self) -> Self::Accumulator {
        MontyLinearAccumulator128::zero()
    }

    #[inline]
    fn multiply_accumulate(
        &self,
        accumulator: &mut Self::Accumulator,
        lhs: &MontyField<2>,
        rhs: &u64,
    ) {
        accumulator.multiply_accumulate(lhs, rhs);
    }

    #[inline]
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator) {
        *accumulator += other;
    }

    #[inline]
    fn reduce(&self, accumulator: Self::Accumulator) -> Result<MontyField<2>, SumcheckError> {
        Ok(accumulator.reduce(&self.reducer)?)
    }
}

/// Failures produced while reducing or checking a sumcheck claim.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SumcheckError {
    #[error("a sumcheck round polynomial must contain at least one coefficient")]
    EmptyRoundPolynomial,
    #[error("sumcheck proof has {actual} rounds, expected {expected}")]
    InvalidRoundCount { expected: usize, actual: usize },
    #[error("sumcheck claim is inconsistent in round {round}")]
    InvalidRoundClaim { round: usize },
    #[error("sumcheck terminal claim is inconsistent")]
    InvalidTerminalClaim,
    #[error("sumcheck product tables have incompatible dimensions")]
    InvalidProductDimensions,
    #[error("sumcheck equality tables have incompatible dimensions")]
    InvalidEqualityDimensions,
    #[error("native u32 product tables contain a multiplicand wider than 32 bits")]
    NativeMultiplicandOutOfRange,
    #[error("invalid dense multilinear-extension table")]
    InvalidMleOperation,
    #[error(transparent)]
    DelayedReduction(#[from] DelayedReductionError),
}

/// Sumcheck round polynomials in coefficient form.
///
/// `COEFFS` is the maximum degree plus one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SumcheckProof<F, const COEFFS: usize> {
    pub round_polynomials: Vec<[F; COEFFS]>,
}

impl<F, const COEFFS: usize> SumcheckProof<F, COEFFS>
where
    F: SpartanField,
{
    /// Verifies all round reductions and returns `(evaluation_point, final_claim)`.
    ///
    /// The protocol using the generic reduction remains responsible for its
    /// terminal identity.
    pub(crate) fn verify(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        expected_rounds: usize,
        field_cfg: &F::Config,
    ) -> Result<(Vec<F>, F), SumcheckError> {
        if COEFFS == 0 {
            return Err(SumcheckError::EmptyRoundPolynomial);
        }

        let actual_rounds = self.round_polynomials.len();
        if actual_rounds != expected_rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: expected_rounds,
                actual: actual_rounds,
            });
        }

        let zero = F::zero_with_cfg(field_cfg);
        let mut current_claim = initial_claim;
        let mut eval_points = Vec::with_capacity(expected_rounds);

        for (round, coefficients) in self.round_polynomials.iter().enumerate() {
            absorb_field_elements(transcript, coefficients);

            let at_zero = coefficients[0].clone();
            let at_one = coefficients
                .iter()
                .fold(zero.clone(), |mut sum, coefficient| {
                    sum += coefficient;
                    sum
                });

            if add(&at_zero, &at_one) != current_claim {
                return Err(SumcheckError::InvalidRoundClaim { round });
            }

            let challenge = squeeze_field(transcript, field_cfg);
            current_claim = evaluate_polynomial(coefficients, &challenge, &zero);
            eval_points.push(challenge);
        }

        Ok((eval_points, current_claim))
    }
}

/// Local output produced while writing a sumcheck proof to the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SumcheckProverOutput<F, const COEFFS: usize> {
    pub proof: SumcheckProof<F, COEFFS>,
    pub eval_points: Vec<F>,
    pub final_claim: F,
}

/// Dense Boolean-row MLEs for `Az`, `Bz`, and `Cz`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct R1csProductMles<F> {
    pub az: DenseMultilinearExtension<F>,
    pub bz: DenseMultilinearExtension<F>,
    pub cz: DenseMultilinearExtension<F>,
}

/// Proof of the equality-weighted R1CS residual sum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuterSumcheckProof<F> {
    /// Cubic rounds in coefficient form `[c0, c1, c2, c3]`.
    pub sumcheck: SumcheckProof<F, 4>,
    pub az_mle_claim: F,
    pub bz_mle_claim: F,
    pub cz_mle_claim: F,
}

/// Prover-local result of the outer sumcheck.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OuterSumcheckOutput<F> {
    pub proof: OuterSumcheckProof<F>,
    pub eval_points: Vec<F>,
    pub final_claim: F,
}

/// Transcript-derived point and terminal evaluations returned by verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OuterSumcheckVerifierOutput<F> {
    pub eval_points: Vec<F>,
    pub az_mle_claim: F,
    pub bz_mle_claim: F,
    pub cz_mle_claim: F,
}

/// Prover-local result of the Spartan inner sumcheck.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InnerSumcheckOutput<F> {
    pub sumcheck: SumcheckProverOutput<F, 3>,
    pub batched_matrix_evaluation: F,
    pub witness_evaluation: F,
}

impl<F> OuterSumcheckProof<F>
where
    F: SpartanField,
{
    /// Verifies the outer reduction and its terminal R1CS identity.
    pub(crate) fn verify(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        tau: &[F],
        field_cfg: &F::Config,
    ) -> Result<OuterSumcheckVerifierOutput<F>, SumcheckError> {
        let (eval_points, final_claim) =
            self.sumcheck
                .verify(transcript, initial_claim, tau.len(), field_cfg)?;

        let terminal_evaluations = [
            self.az_mle_claim.clone(),
            self.bz_mle_claim.clone(),
            self.cz_mle_claim.clone(),
        ];
        absorb_field_elements(transcript, &terminal_evaluations);

        let eq = eq_eval(tau, &eval_points, field_cfg)?;
        let residual = sub(
            &mul(&self.az_mle_claim, &self.bz_mle_claim),
            &self.cz_mle_claim,
        );
        let expected_claim = mul(&eq, &residual);
        if final_claim != expected_claim {
            return Err(SumcheckError::InvalidTerminalClaim);
        }

        Ok(OuterSumcheckVerifierOutput {
            eval_points,
            az_mle_claim: self.az_mle_claim.clone(),
            bz_mle_claim: self.bz_mle_claim.clone(),
            cz_mle_claim: self.cz_mle_claim.clone(),
        })
    }
}

/// Proves the equality-weighted cubic outer sumcheck.
#[cfg(test)]
pub(crate) fn prove_outer_sumcheck<F>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
) -> Result<OuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
{
    let reducer = ImmediateSumcheckReducer::new(field_cfg);
    prove_outer_sumcheck_with_reducer(
        transcript,
        initial_claim,
        (eq_low, eq_high),
        products,
        field_cfg,
        &reducer,
    )
}

pub(crate) fn prove_outer_sumcheck_with_reducer<F, R>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
    reducer: &R,
) -> Result<OuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let num_vars = products.az.num_vars;
    if !has_dense_shape(&products.az)
        || !has_dense_shape(&products.bz)
        || !has_dense_shape(&products.cz)
        || products.bz.num_vars != num_vars
        || products.cz.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    if !has_dense_shape(&eq_low)
        || !has_dense_shape(&eq_high)
        || eq_low
            .num_vars
            .checked_add(eq_high.num_vars)
            .is_none_or(|eq_vars| eq_vars != num_vars)
    {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }

    let zero = F::zero_with_cfg(field_cfg);
    let cubic_interpolation = CubicInterpolation::new(field_cfg);
    let mut eq_low = eq_low.evaluations;
    let mut eq_high = eq_high.evaluations;
    let mut products = R1csProductTableBuffers::from_mles(products);

    // Scratch allocations ping-pong with the active tables after every fold.
    let mut product_scratch = R1csProductTableBuffers::filled(products.len() / 2, &zero);
    let mut eq_low_scratch = vec![zero.clone(); eq_low.len() / 2];
    let mut eq_high_scratch = vec![zero.clone(); eq_high.len() / 2];

    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut coefficients_without_linear = compute_coefficients_without_linear(
        &products,
        EqualityPairs::new(&eq_low, &eq_high),
        &current_claim,
        &cubic_interpolation,
        reducer,
    )?;

    // Bind the low equality variables first. The product traversal folds the
    // active tables and prepares the next round polynomial in one pass.
    while eq_low.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        );

        let next_product_len = products.len() / 2;
        let next_eq_low_len = eq_low.len() / 2;
        product_scratch.truncate(next_product_len);
        eq_low_scratch.truncate(next_eq_low_len);
        fold_table(&eq_low, &mut eq_low_scratch, &challenge);

        if next_product_len > 1 {
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                EqualityPairs::new(&eq_low_scratch, &eq_high),
                &current_claim,
                &cubic_interpolation,
                reducer,
            )?;
        } else {
            debug_assert_eq!(next_product_len, 1);
            fold_product_tables(&products, &mut product_scratch, &challenge);
        }

        products.swap(&mut product_scratch);
        std::mem::swap(&mut eq_low, &mut eq_low_scratch);
    }

    debug_assert_eq!(eq_low.len(), 1);
    debug_assert_eq!(products.len(), eq_high.len());

    while eq_high.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        );

        let next_eq_high_len = eq_high.len() / 2;
        debug_assert_eq!(products.len() / 2, next_eq_high_len);
        product_scratch.truncate(next_eq_high_len);
        eq_high_scratch.truncate(next_eq_high_len);

        if next_eq_high_len == 1 {
            fold_products_and_eq(
                &products,
                &mut product_scratch,
                &eq_high,
                &mut eq_high_scratch,
                &challenge,
            );
        } else {
            fold_table(&eq_high, &mut eq_high_scratch, &challenge);
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                EqualityPairs::new(&eq_low, &eq_high_scratch),
                &current_claim,
                &cubic_interpolation,
                reducer,
            )?;
        }

        products.swap(&mut product_scratch);
        std::mem::swap(&mut eq_high, &mut eq_high_scratch);
    }

    let az_mle_claim = products.az[0].clone();
    let bz_mle_claim = products.bz[0].clone();
    let cz_mle_claim = products.cz[0].clone();
    debug_assert_eq!(
        current_claim,
        mul(
            &mul(&eq_low[0], &eq_high[0]),
            &sub(&mul(&az_mle_claim, &bz_mle_claim), &cz_mle_claim),
        )
    );

    let terminal_evaluations = [
        az_mle_claim.clone(),
        bz_mle_claim.clone(),
        cz_mle_claim.clone(),
    ];
    absorb_field_elements(transcript, &terminal_evaluations);

    Ok(OuterSumcheckOutput {
        proof: OuterSumcheckProof {
            sumcheck: SumcheckProof { round_polynomials },
            az_mle_claim,
            bz_mle_claim,
            cz_mle_claim,
        },
        eval_points,
        final_claim: current_claim,
    })
}

/// Proves the first outer round from exact native u32 relation products, then
/// continues with the ordinary field-valued prover.
///
/// The native tables are consumed at the first Fiat--Shamir challenge. Every
/// folded entry is reduced to the field before it is stored or used in a later
/// multiplication.
pub(crate) fn prove_outer_sumcheck_u32_native_with_reducer<R>(
    transcript: &mut impl Transcript,
    initial_claim: MontyField<2>,
    (eq_low, eq_high): (
        DenseMultilinearExtension<MontyField<2>>,
        DenseMultilinearExtension<MontyField<2>>,
    ),
    products: R1csProductMles<u64>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
    reducer: &R,
) -> Result<OuterSumcheckOutput<MontyField<2>>, SumcheckError>
where
    R: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
{
    let num_vars = products.az.num_vars;
    if !has_dense_shape(&products.az)
        || !has_dense_shape(&products.bz)
        || !has_dense_shape(&products.cz)
        || products.bz.num_vars != num_vars
        || products.cz.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    if products
        .az
        .evaluations
        .iter()
        .chain(&products.bz.evaluations)
        .any(|&value| value > u64::from(u32::MAX))
    {
        return Err(SumcheckError::NativeMultiplicandOutOfRange);
    }
    if !has_dense_shape(&eq_low)
        || !has_dense_shape(&eq_high)
        || eq_low
            .num_vars
            .checked_add(eq_high.num_vars)
            .is_none_or(|eq_vars| eq_vars != num_vars)
    {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }

    let zero = MontyField::<2>::zero_with_cfg(field_cfg);
    let cubic_interpolation = CubicInterpolation::new(field_cfg);
    let mut eq_low = eq_low.evaluations;
    let mut eq_high = eq_high.evaluations;
    let native_products = R1csProductTableBuffers::from_mles(products);
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    if num_vars == 0 {
        let az_mle_claim = native_to_field(native_products.az[0], field_cfg);
        let bz_mle_claim = native_to_field(native_products.bz[0], field_cfg);
        let cz_mle_claim = native_to_field(native_products.cz[0], field_cfg);
        debug_assert_eq!(
            current_claim,
            mul(
                &mul(&eq_low[0], &eq_high[0]),
                &sub(&mul(&az_mle_claim, &bz_mle_claim), &cz_mle_claim),
            )
        );
        absorb_field_elements(
            transcript,
            &[
                az_mle_claim.clone(),
                bz_mle_claim.clone(),
                cz_mle_claim.clone(),
            ],
        );
        return Ok(OuterSumcheckOutput {
            proof: OuterSumcheckProof {
                sumcheck: SumcheckProof { round_polynomials },
                az_mle_claim,
                bz_mle_claim,
                cz_mle_claim,
            },
            eval_points,
            final_claim: current_claim,
        });
    }

    let coefficients_without_linear = compute_u32_native_outer_coefficients_without_linear(
        &native_products,
        EqualityPairs::new(&eq_low, &eq_high),
        &zero,
        reducer,
    )?;
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
        &zero,
        field_cfg,
    );

    let mut products =
        fold_u64_product_tables_to_field(&native_products, &challenge, &zero, field_cfg, reducer)?;
    if eq_low.len() > 1 {
        let mut folded_eq_low = vec![zero.clone(); eq_low.len() / 2];
        fold_table(&eq_low, &mut folded_eq_low, &challenge);
        eq_low = folded_eq_low;
    } else {
        let mut folded_eq_high = vec![zero.clone(); eq_high.len() / 2];
        fold_table(&eq_high, &mut folded_eq_high, &challenge);
        eq_high = folded_eq_high;
    }

    let mut coefficients_without_linear = if products.len() > 1 {
        compute_coefficients_without_linear(
            &products,
            EqualityPairs::new(&eq_low, &eq_high),
            &current_claim,
            &cubic_interpolation,
            reducer,
        )?
    } else {
        std::array::from_fn(|_| zero.clone())
    };

    // From this boundary onward every active table is field-valued. This is
    // the same loop structure as the generic prover, beginning at round one.
    let mut product_scratch = R1csProductTableBuffers::filled(products.len() / 2, &zero);
    let mut eq_low_scratch = vec![zero.clone(); eq_low.len() / 2];
    let mut eq_high_scratch = vec![zero.clone(); eq_high.len() / 2];

    while eq_low.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        );

        let next_product_len = products.len() / 2;
        let next_eq_low_len = eq_low.len() / 2;
        product_scratch.truncate(next_product_len);
        eq_low_scratch.truncate(next_eq_low_len);
        fold_table(&eq_low, &mut eq_low_scratch, &challenge);

        if next_product_len > 1 {
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                EqualityPairs::new(&eq_low_scratch, &eq_high),
                &current_claim,
                &cubic_interpolation,
                reducer,
            )?;
        } else {
            debug_assert_eq!(next_product_len, 1);
            fold_product_tables(&products, &mut product_scratch, &challenge);
        }

        products.swap(&mut product_scratch);
        std::mem::swap(&mut eq_low, &mut eq_low_scratch);
    }

    debug_assert_eq!(eq_low.len(), 1);
    debug_assert_eq!(products.len(), eq_high.len());

    while eq_high.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        );

        let next_eq_high_len = eq_high.len() / 2;
        product_scratch.truncate(next_eq_high_len);
        eq_high_scratch.truncate(next_eq_high_len);

        if next_eq_high_len == 1 {
            fold_products_and_eq(
                &products,
                &mut product_scratch,
                &eq_high,
                &mut eq_high_scratch,
                &challenge,
            );
        } else {
            fold_table(&eq_high, &mut eq_high_scratch, &challenge);
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                EqualityPairs::new(&eq_low, &eq_high_scratch),
                &current_claim,
                &cubic_interpolation,
                reducer,
            )?;
        }

        products.swap(&mut product_scratch);
        std::mem::swap(&mut eq_high, &mut eq_high_scratch);
    }

    let az_mle_claim = products.az[0].clone();
    let bz_mle_claim = products.bz[0].clone();
    let cz_mle_claim = products.cz[0].clone();
    debug_assert_eq!(
        current_claim,
        mul(
            &mul(&eq_low[0], &eq_high[0]),
            &sub(&mul(&az_mle_claim, &bz_mle_claim), &cz_mle_claim),
        )
    );

    absorb_field_elements(
        transcript,
        &[
            az_mle_claim.clone(),
            bz_mle_claim.clone(),
            cz_mle_claim.clone(),
        ],
    );

    Ok(OuterSumcheckOutput {
        proof: OuterSumcheckProof {
            sumcheck: SumcheckProof { round_polynomials },
            az_mle_claim,
            bz_mle_claim,
            cz_mle_claim,
        },
        eval_points,
        final_claim: current_claim,
    })
}

/// Owned evaluation tables used by the fused outer-sumcheck kernels.
struct R1csProductTableBuffers<F> {
    az: Vec<F>,
    bz: Vec<F>,
    cz: Vec<F>,
}

impl<F: Clone> R1csProductTableBuffers<F> {
    fn from_mles(products: R1csProductMles<F>) -> Self {
        Self {
            az: products.az.evaluations,
            bz: products.bz.evaluations,
            cz: products.cz.evaluations,
        }
    }

    fn filled(len: usize, value: &F) -> Self {
        Self {
            az: vec![value.clone(); len],
            bz: vec![value.clone(); len],
            cz: vec![value.clone(); len],
        }
    }

    fn len(&self) -> usize {
        debug_assert_eq!(self.az.len(), self.bz.len());
        debug_assert_eq!(self.az.len(), self.cz.len());
        self.az.len()
    }

    fn truncate(&mut self, len: usize) {
        debug_assert!(self.az.len() >= len);
        debug_assert!(self.bz.len() >= len);
        debug_assert!(self.cz.len() >= len);
        self.az.truncate(len);
        self.bz.truncate(len);
        self.cz.truncate(len);
    }

    fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.az, &mut other.az);
        std::mem::swap(&mut self.bz, &mut other.bz);
        std::mem::swap(&mut self.cz, &mut other.cz);
    }
}

/// Proves the quadratic inner claim
///
/// `initial_claim = sum_y batched_matrix(y) * witness(y)`.
#[cfg(test)]
pub(crate) fn prove_inner_sumcheck<F>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    batched_matrix_mle: DenseMultilinearExtension<F>,
    witness_mle: DenseMultilinearExtension<F>,
    field_cfg: &F::Config,
) -> Result<InnerSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
{
    let reducer = ImmediateSumcheckReducer::new(field_cfg);
    prove_inner_sumcheck_with_reducer(
        transcript,
        initial_claim,
        batched_matrix_mle,
        witness_mle,
        field_cfg,
        &reducer,
    )
}

pub(crate) fn prove_inner_sumcheck_with_reducer<F, R>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    batched_matrix_mle: DenseMultilinearExtension<F>,
    witness_mle: DenseMultilinearExtension<F>,
    field_cfg: &F::Config,
    reducer: &R,
) -> Result<InnerSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let num_vars = batched_matrix_mle.num_vars;
    if !has_dense_shape(&batched_matrix_mle)
        || !has_dense_shape(&witness_mle)
        || witness_mle.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let zero = F::zero_with_cfg(field_cfg);
    let mut batched_matrix = batched_matrix_mle.evaluations;
    let mut witness = witness_mle.evaluations;
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    if num_vars > 0 {
        let mut batched_matrix_scratch = vec![zero.clone(); batched_matrix.len() / 2];
        let mut witness_scratch = vec![zero.clone(); witness.len() / 2];
        let mut coefficients_without_linear =
            sum_inner_round_coefficients_without_linear(&batched_matrix, &witness, reducer)?;

        for _round in 0..num_vars {
            let challenge = recover_full_round_polynomial_and_sample_next_challenge(
                transcript,
                &mut current_claim,
                &coefficients_without_linear,
                &mut round_polynomials,
                &mut eval_points,
                &zero,
                field_cfg,
            );

            let next_len = batched_matrix.len() / 2;
            debug_assert_eq!(witness.len() / 2, next_len);
            debug_assert!(batched_matrix_scratch.len() >= next_len);
            debug_assert!(witness_scratch.len() >= next_len);
            batched_matrix_scratch.truncate(next_len);
            witness_scratch.truncate(next_len);

            if next_len == 1 {
                batched_matrix_scratch[0] =
                    interpolate_pair(&batched_matrix[0], &batched_matrix[1], &challenge);
                witness_scratch[0] = interpolate_pair(&witness[0], &witness[1], &challenge);
            } else {
                coefficients_without_linear =
                    fold_and_compute_next_inner_round_coefficients_without_linear(
                        &batched_matrix,
                        &witness,
                        &mut batched_matrix_scratch,
                        &mut witness_scratch,
                        &challenge,
                        reducer,
                    )?;
            }

            std::mem::swap(&mut batched_matrix, &mut batched_matrix_scratch);
            std::mem::swap(&mut witness, &mut witness_scratch);
        }
    }

    let batched_matrix_evaluation = batched_matrix[0].clone();
    let witness_evaluation = witness[0].clone();
    debug_assert_eq!(
        current_claim,
        mul(&batched_matrix_evaluation, &witness_evaluation)
    );

    Ok(InnerSumcheckOutput {
        sumcheck: SumcheckProverOutput {
            proof: SumcheckProof { round_polynomials },
            eval_points,
            final_claim: current_claim,
        },
        batched_matrix_evaluation,
        witness_evaluation,
    })
}

/// Proves the first inner round with the exact native u32 assignment, then
/// continues with field-valued witness and matrix tables using the fixed
/// production policy: delayed coefficients and delayed native folding.
pub(crate) fn prove_inner_sumcheck_u32_native_with_reducer<R>(
    transcript: &mut impl Transcript,
    initial_claim: MontyField<2>,
    batched_matrix_mle: DenseMultilinearExtension<MontyField<2>>,
    witness_mle: DenseMultilinearExtension<u64>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
    reducer: &R,
) -> Result<InnerSumcheckOutput<MontyField<2>>, SumcheckError>
where
    R: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
{
    let policy = DelayedU32InnerArithmetic { reducer };
    prove_inner_sumcheck_u32_native(
        transcript,
        initial_claim,
        batched_matrix_mle,
        witness_mle,
        field_cfg,
        &policy,
    )
}

/// Benchmark-only inner arithmetic selection used by the controlled policy
/// sweep. Production callers use [`prove_inner_sumcheck_u32_native_with_reducer`].
#[cfg(any(test, feature = "bench-internals"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_inner_sumcheck_u32_native_with_policy<NR, DR, IR>(
    transcript: &mut impl Transcript,
    initial_claim: MontyField<2>,
    batched_matrix_mle: DenseMultilinearExtension<MontyField<2>>,
    witness_mle: DenseMultilinearExtension<u64>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
    native_reducer: &NR,
    delayed_field_reducer: &DR,
    immediate_field_reducer: &IR,
    native_fold_policy: NativeWitnessFoldPolicy,
    field_coefficient_policy: FieldCoefficientPolicy,
) -> Result<InnerSumcheckOutput<MontyField<2>>, SumcheckError>
where
    NR: SumcheckLinearReducer,
    DR: SumcheckProductReducer<MontyField<2>>,
    IR: SumcheckProductReducer<MontyField<2>>,
{
    let policy = SelectedU32InnerArithmetic {
        native_reducer,
        delayed_field_reducer,
        immediate_field_reducer,
        native_fold_policy,
        field_coefficient_policy,
    };
    prove_inner_sumcheck_u32_native(
        transcript,
        initial_claim,
        batched_matrix_mle,
        witness_mle,
        field_cfg,
        &policy,
    )
}

fn prove_inner_sumcheck_u32_native<P>(
    transcript: &mut impl Transcript,
    initial_claim: MontyField<2>,
    batched_matrix_mle: DenseMultilinearExtension<MontyField<2>>,
    witness_mle: DenseMultilinearExtension<u64>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
    policy: &P,
) -> Result<InnerSumcheckOutput<MontyField<2>>, SumcheckError>
where
    P: U32InnerArithmeticPolicy,
{
    let num_vars = batched_matrix_mle.num_vars;
    if !has_dense_shape(&batched_matrix_mle)
        || !has_dense_shape(&witness_mle)
        || witness_mle.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let zero = MontyField::<2>::zero_with_cfg(field_cfg);
    let mut batched_matrix = batched_matrix_mle.evaluations;
    let native_witness = witness_mle.evaluations;
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    if num_vars == 0 {
        let batched_matrix_evaluation = batched_matrix[0].clone();
        let witness_evaluation = native_to_field(native_witness[0], field_cfg);
        debug_assert_eq!(
            current_claim,
            mul(&batched_matrix_evaluation, &witness_evaluation)
        );
        return Ok(InnerSumcheckOutput {
            sumcheck: SumcheckProverOutput {
                proof: SumcheckProof { round_polynomials },
                eval_points,
                final_claim: current_claim,
            },
            batched_matrix_evaluation,
            witness_evaluation,
        });
    }

    let coefficients_without_linear = {
        let _scope = crate::utils::prof::scope("spartan:inner_native_coefficients");
        policy.native_coefficients(&batched_matrix, &native_witness, &zero)?
    };
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
        &zero,
        field_cfg,
    );

    let next_len = batched_matrix.len() / 2;
    let mut folded_matrix = vec![zero.clone(); next_len];
    fold_table(&batched_matrix, &mut folded_matrix, &challenge);
    let mut witness = vec![zero.clone(); next_len];
    {
        let _scope = crate::utils::prof::scope("spartan:inner_native_witness_fold");
        policy.fold_native_witness(&native_witness, &mut witness, &challenge, &zero, field_cfg)?;
    }
    batched_matrix = folded_matrix;

    {
        let _scope = crate::utils::prof::scope("spartan:inner_field_rounds");
        let mut coefficients_without_linear = if next_len > 1 {
            policy.field_coefficients(&batched_matrix, &witness)?
        } else {
            std::array::from_fn(|_| zero.clone())
        };
        let mut batched_matrix_scratch = vec![zero.clone(); batched_matrix.len() / 2];
        let mut witness_scratch = vec![zero.clone(); witness.len() / 2];

        while batched_matrix.len() > 1 {
            let challenge = recover_full_round_polynomial_and_sample_next_challenge(
                transcript,
                &mut current_claim,
                &coefficients_without_linear,
                &mut round_polynomials,
                &mut eval_points,
                &zero,
                field_cfg,
            );

            let next_len = batched_matrix.len() / 2;
            batched_matrix_scratch.truncate(next_len);
            witness_scratch.truncate(next_len);

            if next_len == 1 {
                batched_matrix_scratch[0] =
                    interpolate_pair(&batched_matrix[0], &batched_matrix[1], &challenge);
                witness_scratch[0] = interpolate_pair(&witness[0], &witness[1], &challenge);
            } else {
                coefficients_without_linear = policy.fold_and_compute_field_coefficients(
                    &batched_matrix,
                    &witness,
                    &mut batched_matrix_scratch,
                    &mut witness_scratch,
                    &challenge,
                )?;
            }

            std::mem::swap(&mut batched_matrix, &mut batched_matrix_scratch);
            std::mem::swap(&mut witness, &mut witness_scratch);
        }
    }

    let batched_matrix_evaluation = batched_matrix[0].clone();
    let witness_evaluation = witness[0].clone();
    debug_assert_eq!(
        current_claim,
        mul(&batched_matrix_evaluation, &witness_evaluation)
    );

    Ok(InnerSumcheckOutput {
        sumcheck: SumcheckProverOutput {
            proof: SumcheckProof { round_polynomials },
            eval_points,
            final_claim: current_claim,
        },
        batched_matrix_evaluation,
        witness_evaluation,
    })
}

#[inline]
fn accumulate_inner_pair_coefficients_without_linear<F, R>(
    accumulators: &mut [R::Accumulator; 2],
    matrix_zero: &F,
    matrix_one: &F,
    witness_zero: &F,
    witness_one: &F,
    reducer: &R,
) where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    reducer.multiply_accumulate(&mut accumulators[0], matrix_zero, witness_zero);
    reducer.multiply_accumulate(
        &mut accumulators[1],
        &sub(matrix_one, matrix_zero),
        &sub(witness_one, witness_zero),
    );
}

fn sum_inner_round_coefficients_without_linear<F, R>(
    batched_matrix: &[F],
    witness: &[F],
    reducer: &R,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(batched_matrix.len(), witness.len());
    debug_assert!(batched_matrix.len() >= 2);

    #[cfg(feature = "parallel")]
    if should_parallelize(batched_matrix.len() / 2) {
        let accumulators = batched_matrix
            .par_chunks_exact(2)
            .zip(witness.par_chunks_exact(2))
            .fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |mut accumulators, (matrix, witness)| {
                    accumulate_inner_pair_coefficients_without_linear(
                        &mut accumulators,
                        &matrix[0],
                        &matrix[1],
                        &witness[0],
                        &witness[1],
                        reducer,
                    );
                    accumulators
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        return reduce_two_accumulators(accumulators, reducer);
    }

    let accumulators = batched_matrix
        .chunks_exact(2)
        .zip(witness.chunks_exact(2))
        .fold(
            std::array::from_fn(|_| reducer.accumulator_zero()),
            |mut accumulators, (matrix, witness)| {
                accumulate_inner_pair_coefficients_without_linear(
                    &mut accumulators,
                    &matrix[0],
                    &matrix[1],
                    &witness[0],
                    &witness[1],
                    reducer,
                );
                accumulators
            },
        );
    reduce_two_accumulators(accumulators, reducer)
}

#[cfg(any(test, feature = "bench-internals"))]
fn sum_inner_round_coefficients_selected<DR, IR>(
    batched_matrix: &[MontyField<2>],
    witness: &[MontyField<2>],
    delayed_reducer: &DR,
    immediate_reducer: &IR,
    policy: FieldCoefficientPolicy,
) -> Result<[MontyField<2>; 2], SumcheckError>
where
    DR: SumcheckProductReducer<MontyField<2>>,
    IR: SumcheckProductReducer<MontyField<2>>,
{
    let pair_count = batched_matrix.len() / 2;
    if policy.use_delayed(pair_count) {
        sum_inner_round_coefficients_without_linear(batched_matrix, witness, delayed_reducer)
    } else {
        sum_inner_round_coefficients_without_linear(batched_matrix, witness, immediate_reducer)
    }
}

#[inline]
fn fold_inner_chunk<F, R>(
    batched_matrix: &[F],
    witness: &[F],
    batched_matrix_output: &mut [F],
    witness_output: &mut [F],
    challenge: &F,
    reducer: &R,
) -> [R::Accumulator; 2]
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(batched_matrix.len(), 4);
    debug_assert_eq!(witness.len(), 4);
    debug_assert_eq!(batched_matrix_output.len(), 2);
    debug_assert_eq!(witness_output.len(), 2);

    let folded_matrix = [
        interpolate_pair(&batched_matrix[0], &batched_matrix[1], challenge),
        interpolate_pair(&batched_matrix[2], &batched_matrix[3], challenge),
    ];
    let folded_witness = [
        interpolate_pair(&witness[0], &witness[1], challenge),
        interpolate_pair(&witness[2], &witness[3], challenge),
    ];

    batched_matrix_output.clone_from_slice(&folded_matrix);
    witness_output.clone_from_slice(&folded_witness);
    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    accumulate_inner_pair_coefficients_without_linear(
        &mut accumulators,
        &folded_matrix[0],
        &folded_matrix[1],
        &folded_witness[0],
        &folded_witness[1],
        reducer,
    );
    accumulators
}

/// Folds both active inner tables and prepares the next round's `[c0, c2]`.
fn fold_and_compute_next_inner_round_coefficients_without_linear<F, R>(
    batched_matrix: &[F],
    witness: &[F],
    batched_matrix_output: &mut [F],
    witness_output: &mut [F],
    challenge: &F,
    reducer: &R,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(batched_matrix.len(), witness.len());
    debug_assert!(batched_matrix.len() >= 4);
    debug_assert_eq!(batched_matrix_output.len(), batched_matrix.len() / 2);
    debug_assert_eq!(witness_output.len(), witness.len() / 2);

    #[cfg(feature = "parallel")]
    if should_parallelize(batched_matrix.len() / 4) {
        let accumulators = batched_matrix
            .par_chunks_exact(4)
            .zip(witness.par_chunks_exact(4))
            .zip(batched_matrix_output.par_chunks_exact_mut(2))
            .zip(witness_output.par_chunks_exact_mut(2))
            .fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |accumulators, (((matrix, witness), matrix_output), witness_output)| {
                    let contribution = fold_inner_chunk(
                        matrix,
                        witness,
                        matrix_output,
                        witness_output,
                        challenge,
                        reducer,
                    );
                    merge_accumulators(accumulators, contribution, reducer)
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        return reduce_two_accumulators(accumulators, reducer);
    }

    let accumulators = batched_matrix
        .chunks_exact(4)
        .zip(witness.chunks_exact(4))
        .zip(batched_matrix_output.chunks_exact_mut(2))
        .zip(witness_output.chunks_exact_mut(2))
        .fold(
            std::array::from_fn(|_| reducer.accumulator_zero()),
            |accumulators, (((matrix, witness), matrix_output), witness_output)| {
                let contribution = fold_inner_chunk(
                    matrix,
                    witness,
                    matrix_output,
                    witness_output,
                    challenge,
                    reducer,
                );
                merge_accumulators(accumulators, contribution, reducer)
            },
        );
    reduce_two_accumulators(accumulators, reducer)
}

#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "bench-internals"))]
fn fold_and_compute_next_inner_round_selected<DR, IR>(
    batched_matrix: &[MontyField<2>],
    witness: &[MontyField<2>],
    batched_matrix_output: &mut [MontyField<2>],
    witness_output: &mut [MontyField<2>],
    challenge: &MontyField<2>,
    delayed_reducer: &DR,
    immediate_reducer: &IR,
    policy: FieldCoefficientPolicy,
) -> Result<[MontyField<2>; 2], SumcheckError>
where
    DR: SumcheckProductReducer<MontyField<2>>,
    IR: SumcheckProductReducer<MontyField<2>>,
{
    let next_round_pair_count = batched_matrix_output.len() / 2;
    if policy.use_delayed(next_round_pair_count) {
        fold_and_compute_next_inner_round_coefficients_without_linear(
            batched_matrix,
            witness,
            batched_matrix_output,
            witness_output,
            challenge,
            delayed_reducer,
        )
    } else {
        fold_and_compute_next_inner_round_coefficients_without_linear(
            batched_matrix,
            witness,
            batched_matrix_output,
            witness_output,
            challenge,
            immediate_reducer,
        )
    }
}

#[inline]
fn merge_accumulators<F, R, const COEFFS: usize>(
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
fn reduce_two_accumulators<F, R>(
    accumulators: [R::Accumulator; 2],
    reducer: &R,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let [c0, c2] = accumulators;
    Ok([reducer.reduce(c0)?, reducer.reduce(c2)?])
}

#[inline]
fn reduce_three_accumulators<F, R>(
    accumulators: [R::Accumulator; 3],
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let [c0, c2, c3] = accumulators;
    Ok([
        reducer.reduce(c0)?,
        reducer.reduce(c2)?,
        reducer.reduce(c3)?,
    ])
}

fn sum_product_accumulators<F, R, const COEFFS: usize>(
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
fn native_to_field(
    value: u64,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
) -> MontyField<2> {
    MontyField::<2>::from_with_cfg(value, field_cfg)
}

#[inline]
fn native_u32_product(left: u64, right: u64) -> u64 {
    debug_assert!(left <= u64::from(u32::MAX));
    debug_assert!(right <= u64::from(u32::MAX));
    // Both operands were validated before the transcript was mutated.
    left * right
}

fn sum_linear_accumulators<R, const COEFFS: usize>(
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
fn reduce_three_linear_accumulators<R>(
    accumulators: [<R as SumcheckLinearReducer>::Accumulator; 3],
    reducer: &R,
) -> Result<[MontyField<2>; 3], SumcheckError>
where
    R: SumcheckLinearReducer,
{
    let [c0, c2, c3] = accumulators;
    Ok([
        <R as SumcheckLinearReducer>::reduce(reducer, c0)?,
        <R as SumcheckLinearReducer>::reduce(reducer, c2)?,
        <R as SumcheckLinearReducer>::reduce(reducer, c3)?,
    ])
}

#[inline]
fn accumulate_u32_native_outer_pair<R>(
    accumulators: &mut [<R as SumcheckLinearReducer>::Accumulator; 3],
    products: &R1csProductTableBuffers<u64>,
    index: usize,
    eq_zero: &MontyField<2>,
    eq_one: &MontyField<2>,
    zero: &MontyField<2>,
    reducer: &R,
) where
    R: SumcheckLinearReducer,
{
    let neg_eq_zero = sub(zero, eq_zero);
    let eq_delta = sub(eq_one, eq_zero);
    let neg_eq_delta = sub(zero, &eq_delta);
    let twice_eq_zero = add(eq_zero, eq_zero);
    let twice_eq_one = add(eq_one, eq_one);
    let three_eq_zero = add(&twice_eq_zero, eq_zero);
    let p00_c2_weight = sub(&three_eq_zero, &twice_eq_one);
    let cross_c2_weight = sub(eq_one, &twice_eq_zero);

    let az_zero = products.az[index];
    let az_one = products.az[index + 1];
    let bz_zero = products.bz[index];
    let bz_one = products.bz[index + 1];
    let cz_zero = products.cz[index];
    let cz_one = products.cz[index + 1];
    let p00 = native_u32_product(az_zero, bz_zero);
    let p01 = native_u32_product(az_zero, bz_one);
    let p10 = native_u32_product(az_one, bz_zero);
    let p11 = native_u32_product(az_one, bz_one);

    // c0 = e0 * (p00 - c0).
    <R as SumcheckLinearReducer>::multiply_accumulate(reducer, &mut accumulators[0], eq_zero, &p00);
    <R as SumcheckLinearReducer>::multiply_accumulate(
        reducer,
        &mut accumulators[0],
        &neg_eq_zero,
        &cz_zero,
    );

    // c2 = e0*p11 + (3e0-2e1)*p00 + (e1-2e0)*(p01+p10)
    //      + (e1-e0)*c0 + (e0-e1)*c1.
    for (weight, value) in [
        (eq_zero, p11),
        (&p00_c2_weight, p00),
        (&cross_c2_weight, p01),
        (&cross_c2_weight, p10),
        (&eq_delta, cz_zero),
        (&neg_eq_delta, cz_one),
    ] {
        <R as SumcheckLinearReducer>::multiply_accumulate(
            reducer,
            &mut accumulators[1],
            weight,
            &value,
        );
    }

    // c3 = (e1-e0) * (p11-p01-p10+p00).
    for (weight, value) in [
        (&eq_delta, p11),
        (&eq_delta, p00),
        (&neg_eq_delta, p01),
        (&neg_eq_delta, p10),
    ] {
        <R as SumcheckLinearReducer>::multiply_accumulate(
            reducer,
            &mut accumulators[2],
            weight,
            &value,
        );
    }
}

fn compute_u32_native_outer_coefficients_without_linear<R>(
    products: &R1csProductTableBuffers<u64>,
    equality_pairs: EqualityPairs<'_, MontyField<2>>,
    zero: &MontyField<2>,
    reducer: &R,
) -> Result<[MontyField<2>; 3], SumcheckError>
where
    R: SumcheckLinearReducer + SumcheckProductReducer<MontyField<2>>,
{
    let pair_count = products.len() / 2;
    let low_pair_count = equality_pairs.low_pair_count();

    // The first round uses the same eq_out * (sum eq_in * residual)
    // decomposition, with exact native u32 products feeding field×u64 inner
    // accumulators and field×field outer accumulators.
    if R::FACTOR_SPLIT_EQUALITY && low_pair_count >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS {
        let accumulate_high_bucket = |mut outer: [<R as SumcheckProductReducer<MontyField<2>>>::Accumulator;
                                          3],
                                      high_index: usize|
         -> Result<_, SumcheckError> {
            let mut inner =
                std::array::from_fn(|_| <R as SumcheckLinearReducer>::accumulator_zero(reducer));
            let product_pair_start = high_index * low_pair_count;
            for low_pair_index in 0..low_pair_count {
                let index = 2 * (product_pair_start + low_pair_index);
                let [eq_zero, eq_one] = equality_pairs.low_pair(low_pair_index);
                accumulate_u32_native_outer_pair(
                    &mut inner, products, index, eq_zero, eq_one, zero, reducer,
                );
            }

            let inner = reduce_three_linear_accumulators(inner, reducer)?;
            let high_weight = &equality_pairs.high[high_index];
            for (outer, inner) in outer.iter_mut().zip(&inner) {
                <R as SumcheckProductReducer<MontyField<2>>>::multiply_accumulate(
                    reducer,
                    outer,
                    high_weight,
                    inner,
                );
            }
            Ok(outer)
        };

        #[cfg(feature = "parallel")]
        let accumulators = if should_parallelize(pair_count) {
            (0..equality_pairs.high.len())
                .into_par_iter()
                .try_fold(
                    || {
                        std::array::from_fn(|_| {
                            <R as SumcheckProductReducer<MontyField<2>>>::accumulator_zero(reducer)
                        })
                    },
                    accumulate_high_bucket,
                )
                .try_reduce(
                    || {
                        std::array::from_fn(|_| {
                            <R as SumcheckProductReducer<MontyField<2>>>::accumulator_zero(reducer)
                        })
                    },
                    |left, right| Ok(merge_accumulators(left, right, reducer)),
                )?
        } else {
            let mut accumulators = std::array::from_fn(|_| {
                <R as SumcheckProductReducer<MontyField<2>>>::accumulator_zero(reducer)
            });
            for high_index in 0..equality_pairs.high.len() {
                accumulators = accumulate_high_bucket(accumulators, high_index)?;
            }
            accumulators
        };

        #[cfg(not(feature = "parallel"))]
        let accumulators = {
            let mut accumulators = std::array::from_fn(|_| {
                <R as SumcheckProductReducer<MontyField<2>>>::accumulator_zero(reducer)
            });
            for high_index in 0..equality_pairs.high.len() {
                accumulators = accumulate_high_bucket(accumulators, high_index)?;
            }
            accumulators
        };

        return reduce_three_accumulators(accumulators, reducer);
    }

    if R::FACTOR_SPLIT_EQUALITY
        && equality_pairs.low.len() == 1
        && pair_count >= FACTORED_HIGH_EQUALITY_MIN_PAIRS
    {
        let accumulators = sum_linear_accumulators(
            pair_count,
            |accumulators, pair| {
                let [eq_zero, eq_one] = equality_pairs.high_pair(pair);
                accumulate_u32_native_outer_pair(
                    accumulators,
                    products,
                    2 * pair,
                    eq_zero,
                    eq_one,
                    zero,
                    reducer,
                );
            },
            reducer,
        );
        let evaluations = reduce_three_linear_accumulators(accumulators, reducer)?;
        return delayed_scale_cubic_evaluations(evaluations, &equality_pairs.low[0], reducer);
    }

    let accumulators = sum_linear_accumulators(
        pair_count,
        |accumulators, pair| {
            let index = 2 * pair;
            let [eq_zero, eq_one] = equality_pairs.pair(pair);
            accumulate_u32_native_outer_pair(
                accumulators,
                products,
                index,
                &eq_zero,
                &eq_one,
                zero,
                reducer,
            );
        },
        reducer,
    );
    reduce_three_linear_accumulators(accumulators, reducer)
}

fn sum_u32_native_inner_coefficients_without_linear<R>(
    batched_matrix: &[MontyField<2>],
    witness: &[u64],
    zero: &MontyField<2>,
    reducer: &R,
) -> Result<[MontyField<2>; 2], SumcheckError>
where
    R: SumcheckLinearReducer,
{
    debug_assert_eq!(batched_matrix.len(), witness.len());
    debug_assert!(batched_matrix.len() >= 2);

    let pair_count = batched_matrix.len() / 2;
    let accumulators = sum_linear_accumulators(
        pair_count,
        |accumulators, pair| {
            let index = 2 * pair;
            let matrix_delta = sub(&batched_matrix[index + 1], &batched_matrix[index]);
            let neg_matrix_delta = sub(zero, &matrix_delta);

            <R as SumcheckLinearReducer>::multiply_accumulate(
                reducer,
                &mut accumulators[0],
                &batched_matrix[index],
                &witness[index],
            );
            // (m1-m0)(w1-w0) = (m1-m0)w1 + (m0-m1)w0.
            <R as SumcheckLinearReducer>::multiply_accumulate(
                reducer,
                &mut accumulators[1],
                &matrix_delta,
                &witness[index + 1],
            );
            <R as SumcheckLinearReducer>::multiply_accumulate(
                reducer,
                &mut accumulators[1],
                &neg_matrix_delta,
                &witness[index],
            );
        },
        reducer,
    );

    let [c0, c2] = accumulators;
    Ok([
        <R as SumcheckLinearReducer>::reduce(reducer, c0)?,
        <R as SumcheckLinearReducer>::reduce(reducer, c2)?,
    ])
}

fn fold_u64_table_to_field<R>(
    input: &[u64],
    output: &mut [MontyField<2>],
    challenge: &MontyField<2>,
    zero: &MontyField<2>,
    reducer: &R,
) -> Result<(), SumcheckError>
where
    R: SumcheckLinearReducer,
{
    debug_assert_eq!(input.len(), 2 * output.len());
    let one = MontyField::<2>::one_with_cfg(challenge.cfg());
    let one_minus_challenge = sub(&one, challenge);

    let fold_pair = |pair: &[u64], value: &mut MontyField<2>| -> Result<(), SumcheckError> {
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
        *value = <R as SumcheckLinearReducer>::reduce(reducer, accumulator)?;
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
    debug_assert!(zero.cfg() == challenge.cfg());
    Ok(())
}

#[cfg(any(test, feature = "bench-internals"))]
fn fold_u64_table_to_field_immediate(
    input: &[u64],
    output: &mut [MontyField<2>],
    challenge: &MontyField<2>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
) {
    debug_assert_eq!(input.len(), 2 * output.len());

    let fold_pair = |pair: &[u64], value: &mut MontyField<2>| {
        let low = native_to_field(pair[0], field_cfg);
        let high = native_to_field(pair[1], field_cfg);
        *value = interpolate_pair(&low, &high, challenge);
    };

    #[cfg(feature = "parallel")]
    if should_parallelize(output.len()) {
        input
            .par_chunks_exact(2)
            .zip(output.par_iter_mut())
            .for_each(|(pair, value)| fold_pair(pair, value));
        return;
    }

    for (pair, value) in input.chunks_exact(2).zip(output) {
        fold_pair(pair, value);
    }
}

fn fold_u64_product_tables_to_field<R>(
    input: &R1csProductTableBuffers<u64>,
    challenge: &MontyField<2>,
    zero: &MontyField<2>,
    field_cfg: &crypto_bigint::modular::MontyParams<2>,
    reducer: &R,
) -> Result<R1csProductTableBuffers<MontyField<2>>, SumcheckError>
where
    R: SumcheckLinearReducer,
{
    debug_assert_eq!(challenge.cfg(), field_cfg);
    let mut output = R1csProductTableBuffers::filled(input.len() / 2, zero);
    fold_u64_table_to_field(&input.az, &mut output.az, challenge, zero, reducer)?;
    fold_u64_table_to_field(&input.bz, &mut output.bz, challenge, zero, reducer)?;
    fold_u64_table_to_field(&input.cz, &mut output.cz, challenge, zero, reducer)?;
    Ok(output)
}

#[cfg(feature = "parallel")]
const PARALLEL_SUMCHECK_THRESHOLD: usize = 1 << 12;

#[cfg(feature = "parallel")]
#[inline]
fn should_parallelize(work_items: usize) -> bool {
    work_items >= PARALLEL_SUMCHECK_THRESHOLD && rayon::current_num_threads() > 1
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn accumulate_cubic_evaluations<F, R>(
    accumulators: &mut [R::Accumulator; 3],
    eq_zero: &F,
    eq_one: &F,
    az_zero: &F,
    az_one: &F,
    bz_zero: &F,
    bz_one: &F,
    cz_zero: &F,
    cz_one: &F,
    reducer: &R,
) where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    // Evaluate the cubic at 0, 2, and 3. Compared with expanding its
    // coefficients inside every pair, this saves one reduced field product
    // and one delayed product per pair. The missing value at 1 is recovered
    // from the sumcheck identity after the three accumulators are reduced.
    let residual_zero = sub(&mul(az_zero, bz_zero), cz_zero);
    reducer.multiply_accumulate(&mut accumulators[0], eq_zero, &residual_zero);

    let eq_two = sub(&add(eq_one, eq_one), eq_zero);
    let az_two = sub(&add(az_one, az_one), az_zero);
    let bz_two = sub(&add(bz_one, bz_one), bz_zero);
    let cz_two = sub(&add(cz_one, cz_one), cz_zero);
    let residual_two = sub(&mul(&az_two, &bz_two), &cz_two);
    reducer.multiply_accumulate(&mut accumulators[1], &eq_two, &residual_two);

    let eq_three = add(&eq_two, &sub(eq_one, eq_zero));
    let az_three = add(&az_two, &sub(az_one, az_zero));
    let bz_three = add(&bz_two, &sub(bz_one, bz_zero));
    let cz_three = add(&cz_two, &sub(cz_one, cz_zero));
    let residual_three = sub(&mul(&az_three, &bz_three), &cz_three);
    reducer.multiply_accumulate(&mut accumulators[2], &eq_three, &residual_three);
}

/// Public-modulus constants used to interpolate a cubic from evaluations at
/// 0, 1, 2, and 3. They are prepared once per outer sumcheck, outside every
/// product loop.
struct CubicInterpolation<F> {
    half: F,
    sixth: F,
}

impl<F> CubicInterpolation<F>
where
    F: SpartanField,
{
    fn new(field_cfg: &F::Config) -> Self {
        let one = F::one_with_cfg(field_cfg);
        let two = add(&one, &one);
        let three = add(&two, &one);
        let six = add(&three, &three);
        Self {
            half: one.clone() / &two,
            sixth: one / &six,
        }
    }

    /// Converts `[g(0), g(2), g(3)]` to `[c0, c2, c3]`, deriving
    /// `g(1) = current_claim - g(0)` from the sumcheck relation.
    fn coefficients_without_linear(&self, current_claim: &F, evaluations: [F; 3]) -> [F; 3] {
        let [at_zero, at_two, at_three] = evaluations;
        let at_one = sub(current_claim, &at_zero);
        let three_at_one = add(&add(&at_one, &at_one), &at_one);
        let three_at_two = add(&add(&at_two, &at_two), &at_two);
        let third_difference = sub(
            &add(&sub(&at_three, &three_at_two), &three_at_one),
            &at_zero,
        );
        let c3 = mul(&third_difference, &self.sixth);

        let second_difference = add(&sub(&at_two, &add(&at_one, &at_one)), &at_zero);
        let three_c3 = add(&add(&c3, &c3), &c3);
        let c2 = sub(&mul(&second_difference, &self.half), &three_c3);
        [at_zero, c2, c3]
    }
}

/// Restores the omitted linear coefficient of a round polynomial.
#[inline]
fn reconstruct_round_coefficients<F, const INPUT_COEFFS: usize, const COEFFS: usize>(
    current_claim: &F,
    coefficients_without_linear: &[F; INPUT_COEFFS],
    zero: &F,
) -> [F; COEFFS]
where
    F: SpartanField,
{
    assert!(INPUT_COEFFS >= 1);
    assert_eq!(COEFFS, INPUT_COEFFS + 1);

    let mut coefficients = std::array::from_fn(|_| zero.clone());
    coefficients[0] = coefficients_without_linear[0].clone();
    coefficients[2..].clone_from_slice(&coefficients_without_linear[1..]);

    let at_one_without_c1 = coefficients
        .iter()
        .fold(zero.clone(), |mut sum, coefficient| {
            sum += coefficient;
            sum
        });
    coefficients[1] = sub(&sub(current_claim, &coefficients[0]), &at_one_without_c1);
    coefficients
}

#[inline]
fn evaluate_polynomial<F, const COEFFS: usize>(coefficients: &[F; COEFFS], point: &F, zero: &F) -> F
where
    F: SpartanField,
{
    coefficients
        .iter()
        .rev()
        .fold(zero.clone(), |value, coefficient| {
            add(&mul(&value, point), coefficient)
        })
}

/// Completes and records one sumcheck round, then samples its challenge.
fn recover_full_round_polynomial_and_sample_next_challenge<
    F,
    const INPUT_COEFFS: usize,
    const COEFFS: usize,
>(
    transcript: &mut impl Transcript,
    current_claim: &mut F,
    coefficients_without_linear: &[F; INPUT_COEFFS],
    round_polynomials: &mut Vec<[F; COEFFS]>,
    eval_points: &mut Vec<F>,
    zero: &F,
    field_cfg: &F::Config,
) -> F
where
    F: SpartanField,
{
    let coefficients =
        reconstruct_round_coefficients(current_claim, coefficients_without_linear, zero);
    let at_one = coefficients
        .iter()
        .fold(zero.clone(), |mut sum, coefficient| {
            sum += coefficient;
            sum
        });
    debug_assert_eq!(*current_claim, add(&coefficients[0], &at_one));

    absorb_field_elements(transcript, &coefficients);
    let challenge = squeeze_field(transcript, field_cfg);
    *current_claim = evaluate_polynomial(&coefficients, &challenge, zero);
    round_polynomials.push(coefficients);
    eval_points.push(challenge.clone());
    challenge
}

/// Allocation-free adjacent-pair view of the low/high equality-table product.
struct EqualityPairs<'a, F> {
    low: &'a [F],
    high: &'a [F],
    low_bits: usize,
    low_mask: usize,
}

impl<'a, F> EqualityPairs<'a, F>
where
    F: SpartanField,
{
    fn new(low: &'a [F], high: &'a [F]) -> Self {
        debug_assert!(low.len().is_power_of_two());
        debug_assert!(high.len().is_power_of_two());

        Self {
            low,
            high,
            low_bits: low.len().ilog2() as usize,
            low_mask: low.len() - 1,
        }
    }

    #[inline]
    fn pair(&self, pair: usize) -> [F; 2] {
        if self.low_bits == 0 {
            let index = 2 * pair;
            return [
                mul(&self.low[0], &self.high[index]),
                mul(&self.low[0], &self.high[index + 1]),
            ];
        }

        let low_pair_bits = self.low_bits - 1;
        let low_pair = pair & (self.low_mask >> 1);
        let high_weight = &self.high[pair >> low_pair_bits];
        [
            mul(&self.low[2 * low_pair], high_weight),
            mul(&self.low[2 * low_pair + 1], high_weight),
        ]
    }

    /// Returns the two low-factor evaluations for one adjacent pair.
    ///
    /// This is used by the two-level outer accumulation while low variables
    /// remain active. The corresponding high factor is constant over every
    /// `low_pair_count()` consecutive product pairs.
    #[inline]
    fn low_pair(&self, pair: usize) -> [&F; 2] {
        debug_assert!(self.low_bits > 0);
        [&self.low[2 * pair], &self.low[2 * pair + 1]]
    }

    #[inline]
    fn low_pair_count(&self) -> usize {
        self.low.len() / 2
    }

    #[inline]
    fn high_pair(&self, pair: usize) -> [&F; 2] {
        debug_assert_eq!(self.low_bits, 0);
        [&self.high[2 * pair], &self.high[2 * pair + 1]]
    }
}

/// The Spartan2-style two-level decomposition evaluates the four-factor term
///
/// `eq_out * eq_in * A * B`
///
/// as `eq_out * (sum(eq_in * (A * B - C)))`: first reduce one inner subtotal
/// per `eq_out` bucket, then delayed-MAC that subtotal into the outer sum. This
/// removes the N-scaling immediate `eq_out * eq_in` multiplication. Very short
/// buckets do not amortize the inner reduction, so their tail rounds retain
/// the direct product.
const TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS: usize = 8;

/// Scaling a complete high-half subtotal costs three extra reductions. Avoid
/// that fixed cost once only a handful of product pairs remain.
const FACTORED_HIGH_EQUALITY_MIN_PAIRS: usize = 8;

fn delayed_scale_cubic_evaluations<F, R>(
    evaluations: [F; 3],
    factor: &F,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for (accumulator, evaluation) in accumulators.iter_mut().zip(&evaluations) {
        reducer.multiply_accumulate(accumulator, factor, evaluation);
    }
    reduce_three_accumulators(accumulators, reducer)
}

/// Computes the outer cubic evaluations in two delayed-reduction levels.
///
/// For each `eq_out` (high-factor) bucket, the first level accumulates
/// `eq_in * (A * B - C)` over all low pairs and reduces that subtotal. The
/// second level accumulates `eq_out * subtotal`. At no point is
/// `eq_out * eq_in` formed with an immediate field multiplication.
fn compute_two_level_cubic_evaluations<F, R>(
    products: &R1csProductTableBuffers<F>,
    equality_pairs: &EqualityPairs<'_, F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let low_pair_count = equality_pairs.low_pair_count();
    debug_assert!(low_pair_count > 0);
    debug_assert_eq!(
        products.len() / 2,
        low_pair_count * equality_pairs.high.len()
    );

    let accumulate_high_bucket =
        |mut outer: [R::Accumulator; 3], high_index: usize| -> Result<_, SumcheckError> {
            let mut inner = std::array::from_fn(|_| reducer.accumulator_zero());
            let product_pair_start = high_index * low_pair_count;

            for low_pair_index in 0..low_pair_count {
                let product_index = 2 * (product_pair_start + low_pair_index);
                let [eq_zero, eq_one] = equality_pairs.low_pair(low_pair_index);
                accumulate_cubic_evaluations(
                    &mut inner,
                    eq_zero,
                    eq_one,
                    &products.az[product_index],
                    &products.az[product_index + 1],
                    &products.bz[product_index],
                    &products.bz[product_index + 1],
                    &products.cz[product_index],
                    &products.cz[product_index + 1],
                    reducer,
                );
            }

            let inner = reduce_three_accumulators(inner, reducer)?;
            let high_weight = &equality_pairs.high[high_index];
            for (outer, inner) in outer.iter_mut().zip(&inner) {
                reducer.multiply_accumulate(outer, high_weight, inner);
            }
            Ok(outer)
        };

    #[cfg(feature = "parallel")]
    if should_parallelize(products.len() / 2) {
        let accumulators = (0..equality_pairs.high.len())
            .into_par_iter()
            .try_fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                accumulate_high_bucket,
            )
            .try_reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| Ok(merge_accumulators(left, right, reducer)),
            )?;
        return reduce_three_accumulators(accumulators, reducer);
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for high_index in 0..equality_pairs.high.len() {
        accumulators = accumulate_high_bucket(accumulators, high_index)?;
    }
    reduce_three_accumulators(accumulators, reducer)
}

/// Accumulates against the remaining high equality table first, then applies
/// the already-bound low equality scalar once per cubic evaluation.
fn compute_high_factored_cubic_evaluations<F, R>(
    products: &R1csProductTableBuffers<F>,
    equality_pairs: &EqualityPairs<'_, F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(equality_pairs.low.len(), 1);
    let pair_count = products.len() / 2;
    debug_assert_eq!(pair_count, equality_pairs.high.len() / 2);
    let accumulators = sum_product_accumulators(
        pair_count,
        |accumulators, pair| {
            let index = 2 * pair;
            let [eq_zero, eq_one] = equality_pairs.high_pair(pair);
            accumulate_cubic_evaluations(
                accumulators,
                eq_zero,
                eq_one,
                &products.az[index],
                &products.az[index + 1],
                &products.bz[index],
                &products.bz[index + 1],
                &products.cz[index],
                &products.cz[index + 1],
                reducer,
            );
        },
        reducer,
    );
    let evaluations = reduce_three_accumulators(accumulators, reducer)?;
    delayed_scale_cubic_evaluations(evaluations, &equality_pairs.low[0], reducer)
}

/// Computes `[c0, c2, c3]` from evaluations at 0, 2, and 3 over adjacent
/// pairs in the product tables.
fn compute_coefficients_without_linear<F, R>(
    products: &R1csProductTableBuffers<F>,
    equality_pairs: EqualityPairs<'_, F>,
    current_claim: &F,
    interpolation: &CubicInterpolation<F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let pair_count = products.len() / 2;
    let evaluations = if R::FACTOR_SPLIT_EQUALITY
        && equality_pairs.low_pair_count() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS
    {
        compute_two_level_cubic_evaluations(products, &equality_pairs, reducer)?
    } else if R::FACTOR_SPLIT_EQUALITY
        && equality_pairs.low.len() == 1
        && pair_count >= FACTORED_HIGH_EQUALITY_MIN_PAIRS
    {
        compute_high_factored_cubic_evaluations(products, &equality_pairs, reducer)?
    } else {
        let accumulators = sum_product_accumulators(
            pair_count,
            |accumulators, pair| {
                let index = 2 * pair;
                let [eq_zero, eq_one] = equality_pairs.pair(pair);
                accumulate_cubic_evaluations(
                    accumulators,
                    &eq_zero,
                    &eq_one,
                    &products.az[index],
                    &products.az[index + 1],
                    &products.bz[index],
                    &products.bz[index + 1],
                    &products.cz[index],
                    &products.cz[index + 1],
                    reducer,
                );
            },
            reducer,
        );
        reduce_three_accumulators(accumulators, reducer)?
    };
    Ok(interpolation.coefficients_without_linear(current_claim, evaluations))
}

#[inline]
fn interpolate_pair<F>(zero: &F, one: &F, challenge: &F) -> F
where
    F: SpartanField,
{
    add(zero, &mul(challenge, &sub(one, zero)))
}

#[inline]
fn fold_two_pairs<F>(values: &[F], challenge: &F) -> [F; 2]
where
    F: SpartanField,
{
    debug_assert_eq!(values.len(), 4);
    [
        interpolate_pair(&values[0], &values[1], challenge),
        interpolate_pair(&values[2], &values[3], challenge),
    ]
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn fold_product_chunk<F>(
    az: &[F],
    bz: &[F],
    cz: &[F],
    az_output: &mut [F],
    bz_output: &mut [F],
    cz_output: &mut [F],
    challenge: &F,
) -> [[F; 2]; 3]
where
    F: SpartanField,
{
    debug_assert_eq!(az_output.len(), 2);
    debug_assert_eq!(bz_output.len(), 2);
    debug_assert_eq!(cz_output.len(), 2);

    let folded = [
        fold_two_pairs(az, challenge),
        fold_two_pairs(bz, challenge),
        fold_two_pairs(cz, challenge),
    ];
    az_output.clone_from_slice(&folded[0]);
    bz_output.clone_from_slice(&folded[1]);
    cz_output.clone_from_slice(&folded[2]);
    folded
}

/// Folds one evaluation table into preallocated storage.
fn fold_table<F>(input: &[F], output: &mut [F], challenge: &F)
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
                *value = interpolate_pair(&pair[0], &pair[1], challenge);
            });
        return;
    }

    input
        .chunks_exact(2)
        .zip(output.iter_mut())
        .for_each(|(pair, value)| {
            *value = interpolate_pair(&pair[0], &pair[1], challenge);
        });
}

fn fold_product_tables<F>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
) where
    F: SpartanField,
{
    debug_assert_eq!(input.len(), 2 * output.len());
    fold_table(&input.az, &mut output.az, challenge);
    fold_table(&input.bz, &mut output.bz, challenge);
    fold_table(&input.cz, &mut output.cz, challenge);
}

/// Fused product-table fold and two-level equality accumulation for an active
/// low equality table. Each high bucket owns contiguous input/output ranges,
/// which keeps the parallel path allocation-free and race-free.
fn fold_products_and_compute_next_two_level<F, R>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_pairs: &EqualityPairs<'_, F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let low_pair_count = equality_pairs.low_pair_count();
    debug_assert!(low_pair_count > 0);
    debug_assert_eq!(output.len() / 2, low_pair_count * equality_pairs.high.len());
    let input_values_per_high = 4 * low_pair_count;
    let output_values_per_high = 2 * low_pair_count;

    let accumulate_high_bucket = |mut outer: [R::Accumulator; 3],
                                  high_index: usize,
                                  az: &[F],
                                  bz: &[F],
                                  cz: &[F],
                                  az_output: &mut [F],
                                  bz_output: &mut [F],
                                  cz_output: &mut [F]|
     -> Result<_, SumcheckError> {
        let mut inner = std::array::from_fn(|_| reducer.accumulator_zero());
        for low_pair_index in 0..low_pair_count {
            let input_start = 4 * low_pair_index;
            let output_start = 2 * low_pair_index;
            let [az, bz, cz] = fold_product_chunk(
                &az[input_start..input_start + 4],
                &bz[input_start..input_start + 4],
                &cz[input_start..input_start + 4],
                &mut az_output[output_start..output_start + 2],
                &mut bz_output[output_start..output_start + 2],
                &mut cz_output[output_start..output_start + 2],
                challenge,
            );
            let [eq_zero, eq_one] = equality_pairs.low_pair(low_pair_index);
            accumulate_cubic_evaluations(
                &mut inner, eq_zero, eq_one, &az[0], &az[1], &bz[0], &bz[1], &cz[0], &cz[1],
                reducer,
            );
        }

        let inner = reduce_three_accumulators(inner, reducer)?;
        let high_weight = &equality_pairs.high[high_index];
        for (outer, inner) in outer.iter_mut().zip(&inner) {
            reducer.multiply_accumulate(outer, high_weight, inner);
        }
        Ok(outer)
    };

    #[cfg(feature = "parallel")]
    if should_parallelize(output.len() / 2) {
        let accumulators = (
            input.az.par_chunks_exact(input_values_per_high),
            input.bz.par_chunks_exact(input_values_per_high),
            input.cz.par_chunks_exact(input_values_per_high),
            output.az.par_chunks_exact_mut(output_values_per_high),
            output.bz.par_chunks_exact_mut(output_values_per_high),
            output.cz.par_chunks_exact_mut(output_values_per_high),
        )
            .into_par_iter()
            .enumerate()
            .try_fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |outer, (high_index, (az, bz, cz, az_output, bz_output, cz_output))| {
                    accumulate_high_bucket(
                        outer, high_index, az, bz, cz, az_output, bz_output, cz_output,
                    )
                },
            )
            .try_reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| Ok(merge_accumulators(left, right, reducer)),
            )?;
        return reduce_three_accumulators(accumulators, reducer);
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for high_index in 0..equality_pairs.high.len() {
        let input_start = high_index * input_values_per_high;
        let output_start = high_index * output_values_per_high;
        accumulators = accumulate_high_bucket(
            accumulators,
            high_index,
            &input.az[input_start..input_start + input_values_per_high],
            &input.bz[input_start..input_start + input_values_per_high],
            &input.cz[input_start..input_start + input_values_per_high],
            &mut output.az[output_start..output_start + output_values_per_high],
            &mut output.bz[output_start..output_start + output_values_per_high],
            &mut output.cz[output_start..output_start + output_values_per_high],
        )?;
    }
    reduce_three_accumulators(accumulators, reducer)
}

/// Fused product-table fold for the high-variable phase. The N-scaling loop
/// uses only high equality factors; the common low scalar is applied after
/// the three global subtotals have been reduced.
fn fold_products_and_compute_next_high_factored<F, R>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_pairs: &EqualityPairs<'_, F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(equality_pairs.low.len(), 1);
    let accumulate = |mut accumulators: [R::Accumulator; 3],
                      chunk: usize,
                      az: &[F],
                      bz: &[F],
                      cz: &[F],
                      az_output: &mut [F],
                      bz_output: &mut [F],
                      cz_output: &mut [F]| {
        let [az, bz, cz] =
            fold_product_chunk(az, bz, cz, az_output, bz_output, cz_output, challenge);
        let [eq_zero, eq_one] = equality_pairs.high_pair(chunk);
        accumulate_cubic_evaluations(
            &mut accumulators,
            eq_zero,
            eq_one,
            &az[0],
            &az[1],
            &bz[0],
            &bz[1],
            &cz[0],
            &cz[1],
            reducer,
        );
        accumulators
    };

    let chunk_count = output.len() / 2;

    #[cfg(feature = "parallel")]
    if should_parallelize(chunk_count) {
        let accumulators = (
            input.az.par_chunks_exact(4),
            input.bz.par_chunks_exact(4),
            input.cz.par_chunks_exact(4),
            output.az.par_chunks_exact_mut(2),
            output.bz.par_chunks_exact_mut(2),
            output.cz.par_chunks_exact_mut(2),
        )
            .into_par_iter()
            .enumerate()
            .fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |accumulators, (chunk, (az, bz, cz, az_output, bz_output, cz_output))| {
                    accumulate(
                        accumulators,
                        chunk,
                        az,
                        bz,
                        cz,
                        az_output,
                        bz_output,
                        cz_output,
                    )
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        let evaluations = reduce_three_accumulators(accumulators, reducer)?;
        return delayed_scale_cubic_evaluations(evaluations, &equality_pairs.low[0], reducer);
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for chunk in 0..chunk_count {
        let input_start = 4 * chunk;
        let output_start = 2 * chunk;
        accumulators = accumulate(
            accumulators,
            chunk,
            &input.az[input_start..input_start + 4],
            &input.bz[input_start..input_start + 4],
            &input.cz[input_start..input_start + 4],
            &mut output.az[output_start..output_start + 2],
            &mut output.bz[output_start..output_start + 2],
            &mut output.cz[output_start..output_start + 2],
        );
    }
    let evaluations = reduce_three_accumulators(accumulators, reducer)?;
    delayed_scale_cubic_evaluations(evaluations, &equality_pairs.low[0], reducer)
}

/// Folds all product tables and accumulates the next round polynomial.
fn fold_products_and_compute_next<F, R>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_pairs: EqualityPairs<'_, F>,
    current_claim: &F,
    interpolation: &CubicInterpolation<F>,
    reducer: &R,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(input.len(), 2 * output.len());

    if R::FACTOR_SPLIT_EQUALITY
        && equality_pairs.low_pair_count() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS
    {
        let evaluations = fold_products_and_compute_next_two_level(
            input,
            output,
            challenge,
            &equality_pairs,
            reducer,
        )?;
        return Ok(interpolation.coefficients_without_linear(current_claim, evaluations));
    }

    if R::FACTOR_SPLIT_EQUALITY
        && equality_pairs.low.len() == 1
        && output.len() / 2 >= FACTORED_HIGH_EQUALITY_MIN_PAIRS
    {
        let evaluations = fold_products_and_compute_next_high_factored(
            input,
            output,
            challenge,
            &equality_pairs,
            reducer,
        )?;
        return Ok(interpolation.coefficients_without_linear(current_claim, evaluations));
    }

    let accumulate = |mut accumulators: [R::Accumulator; 3],
                      chunk: usize,
                      az: &[F],
                      bz: &[F],
                      cz: &[F],
                      az_output: &mut [F],
                      bz_output: &mut [F],
                      cz_output: &mut [F]| {
        let [az, bz, cz] =
            fold_product_chunk(az, bz, cz, az_output, bz_output, cz_output, challenge);
        let [eq_zero, eq_one] = equality_pairs.pair(chunk);
        accumulate_cubic_evaluations(
            &mut accumulators,
            &eq_zero,
            &eq_one,
            &az[0],
            &az[1],
            &bz[0],
            &bz[1],
            &cz[0],
            &cz[1],
            reducer,
        );
        accumulators
    };

    let chunk_count = output.len() / 2;

    #[cfg(feature = "parallel")]
    if should_parallelize(chunk_count) {
        let accumulators = (
            input.az.par_chunks_exact(4),
            input.bz.par_chunks_exact(4),
            input.cz.par_chunks_exact(4),
            output.az.par_chunks_exact_mut(2),
            output.bz.par_chunks_exact_mut(2),
            output.cz.par_chunks_exact_mut(2),
        )
            .into_par_iter()
            .enumerate()
            .fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |accumulators, (chunk, (az, bz, cz, az_output, bz_output, cz_output))| {
                    accumulate(
                        accumulators,
                        chunk,
                        az,
                        bz,
                        cz,
                        az_output,
                        bz_output,
                        cz_output,
                    )
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        let evaluations = reduce_three_accumulators(accumulators, reducer)?;
        return Ok(interpolation.coefficients_without_linear(current_claim, evaluations));
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for chunk in 0..chunk_count {
        let input_start = 4 * chunk;
        let output_start = 2 * chunk;
        accumulators = accumulate(
            accumulators,
            chunk,
            &input.az[input_start..input_start + 4],
            &input.bz[input_start..input_start + 4],
            &input.cz[input_start..input_start + 4],
            &mut output.az[output_start..output_start + 2],
            &mut output.bz[output_start..output_start + 2],
            &mut output.cz[output_start..output_start + 2],
        );
    }
    let evaluations = reduce_three_accumulators(accumulators, reducer)?;
    Ok(interpolation.coefficients_without_linear(current_claim, evaluations))
}

/// Folds the high equality factor and all product tables together.
fn fold_products_and_eq<F>(
    products: &R1csProductTableBuffers<F>,
    product_output: &mut R1csProductTableBuffers<F>,
    eq: &[F],
    eq_output: &mut [F],
    challenge: &F,
) where
    F: SpartanField,
{
    debug_assert_eq!(products.len(), eq.len());
    debug_assert_eq!(products.len(), 2 * product_output.len());
    debug_assert_eq!(eq.len(), 2 * eq_output.len());

    fold_product_tables(products, product_output, challenge);
    fold_table(eq, eq_output, challenge);
}

fn eq_eval<F>(left: &[F], right: &[F], field_cfg: &F::Config) -> Result<F, SumcheckError>
where
    F: SpartanField,
{
    if left.len() != right.len() {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }

    let one = F::one_with_cfg(field_cfg);
    let mut result = one.clone();
    for (left_i, right_i) in left.iter().zip(right) {
        // (1 - x) + y * (2x - 1), equivalent to
        // x*y + (1-x)*(1-y), including in characteristic two.
        let two_x_minus_one = sub(&add(left_i, left_i), &one);
        let factor = add(&sub(&one, left_i), &mul(right_i, &two_x_minus_one));
        result *= &factor;
    }
    Ok(result)
}

fn has_dense_shape<F>(mle: &DenseMultilinearExtension<F>) -> bool {
    mle.num_vars < usize::BITS as usize && mle.evaluations.len() == 1usize << mle.num_vars
}

#[inline]
fn add<F: SpartanField>(left: &F, right: &F) -> F {
    left.clone() + right
}

#[inline]
fn sub<F: SpartanField>(left: &F, right: &F) -> F {
    left.clone() - right
}

#[inline]
fn mul<F: SpartanField>(left: &F, right: &F) -> F {
    left.clone() * right
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        FromWithConfig, PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint,
    };

    use crate::{piop::spartan::matrix::eq_table, transcript::Blake3Transcript};

    use super::*;

    const TEST_MODULUS: u128 = (1_u128 << 100) - 15;

    fn config() -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(TEST_MODULUS)).expect("prime test modulus")
    }

    fn field(value: u64, field_cfg: &<F128 as PrimeField>::Config) -> F128 {
        F128::from_with_cfg(value, field_cfg)
    }

    #[test]
    fn one_variable_inner_sumcheck_has_the_expected_quadratic_and_replays() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let batched_matrix = DenseMultilinearExtension::from_evaluations_vec(
            1,
            vec![field(2, &field_cfg), field(5, &field_cfg)],
            zero.clone(),
        );
        let witness = DenseMultilinearExtension::from_evaluations_vec(
            1,
            vec![field(3, &field_cfg), field(7, &field_cfg)],
            zero,
        );
        let initial_claim = field(41, &field_cfg);
        let mut prover_transcript = Blake3Transcript::new();

        let output = prove_inner_sumcheck(
            &mut prover_transcript,
            initial_claim.clone(),
            batched_matrix,
            witness,
            &field_cfg,
        )
        .unwrap();

        assert_eq!(
            output.sumcheck.proof.round_polynomials,
            vec![[
                field(6, &field_cfg),
                field(17, &field_cfg),
                field(12, &field_cfg),
            ]]
        );

        let mut verifier_transcript = Blake3Transcript::new();
        let (point, final_claim) = output
            .sumcheck
            .proof
            .verify(&mut verifier_transcript, initial_claim, 1, &field_cfg)
            .unwrap();
        assert_eq!(point, output.sumcheck.eval_points);
        assert_eq!(final_claim, output.sumcheck.final_claim);
        assert_eq!(
            final_claim,
            mul(
                &output.batched_matrix_evaluation,
                &output.witness_evaluation,
            )
        );
    }

    #[test]
    fn zero_variable_inner_sumcheck_proves_and_verifies_without_moving_transcript() {
        let field_cfg = config();
        let initial_claim = field(35, &field_cfg);
        let mut prover_transcript = Blake3Transcript::new();

        let output = prove_inner_sumcheck(
            &mut prover_transcript,
            initial_claim.clone(),
            DenseMultilinearExtension::zero_vars(field(5, &field_cfg)),
            DenseMultilinearExtension::zero_vars(field(7, &field_cfg)),
            &field_cfg,
        )
        .unwrap();

        assert!(output.sumcheck.proof.round_polynomials.is_empty());
        assert!(output.sumcheck.eval_points.is_empty());
        assert_eq!(output.sumcheck.final_claim, initial_claim);
        assert_eq!(output.batched_matrix_evaluation, field(5, &field_cfg));
        assert_eq!(output.witness_evaluation, field(7, &field_cfg));

        let mut verifier_transcript = Blake3Transcript::new();
        let (point, final_claim) = output
            .sumcheck
            .proof
            .verify(
                &mut verifier_transcript,
                initial_claim.clone(),
                0,
                &field_cfg,
            )
            .unwrap();
        assert!(point.is_empty());
        assert_eq!(final_claim, initial_claim);

        let prover_next = squeeze_field::<F128, _>(&mut prover_transcript, &field_cfg);
        let verifier_next = squeeze_field::<F128, _>(&mut verifier_transcript, &field_cfg);
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<F128, _>(&mut fresh_transcript, &field_cfg);
        assert_eq!(prover_next, fresh_next);
        assert_eq!(verifier_next, fresh_next);
    }

    #[test]
    fn zero_variable_outer_sumcheck_proves_and_verifies() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let one = F128::one_with_cfg(&field_cfg);
        let products = R1csProductMles {
            az: DenseMultilinearExtension::zero_vars(field(2, &field_cfg)),
            bz: DenseMultilinearExtension::zero_vars(field(3, &field_cfg)),
            cz: DenseMultilinearExtension::zero_vars(field(6, &field_cfg)),
        };
        let equality_factors = (
            DenseMultilinearExtension::zero_vars(one.clone()),
            DenseMultilinearExtension::zero_vars(one),
        );
        let mut prover_transcript = Blake3Transcript::new();

        let output = prove_outer_sumcheck(
            &mut prover_transcript,
            zero.clone(),
            equality_factors,
            products,
            &field_cfg,
        )
        .unwrap();

        assert!(output.proof.sumcheck.round_polynomials.is_empty());
        assert!(output.eval_points.is_empty());
        assert_eq!(output.final_claim, zero);
        assert_eq!(output.proof.az_mle_claim, field(2, &field_cfg));
        assert_eq!(output.proof.bz_mle_claim, field(3, &field_cfg));
        assert_eq!(output.proof.cz_mle_claim, field(6, &field_cfg));

        let mut verifier_transcript = Blake3Transcript::new();
        let verified = output
            .proof
            .verify(
                &mut verifier_transcript,
                F128::zero_with_cfg(&field_cfg),
                &[],
                &field_cfg,
            )
            .unwrap();
        assert!(verified.eval_points.is_empty());
        assert_eq!(verified.az_mle_claim, field(2, &field_cfg));
        assert_eq!(verified.bz_mle_claim, field(3, &field_cfg));
        assert_eq!(verified.cz_mle_claim, field(6, &field_cfg));

        assert_eq!(
            squeeze_field::<F128, _>(&mut prover_transcript, &field_cfg),
            squeeze_field::<F128, _>(&mut verifier_transcript, &field_cfg)
        );
    }

    #[test]
    fn sumcheck_rejects_a_tampered_linear_coefficient() {
        let field_cfg = config();
        let proof = SumcheckProof::<F128, 4> {
            round_polynomials: vec![
                [
                    field(10, &field_cfg),
                    field(0, &field_cfg),
                    field(0, &field_cfg),
                    field(0, &field_cfg),
                ],
                [
                    field(1, &field_cfg),
                    field(3, &field_cfg),
                    field(3, &field_cfg),
                    field(3, &field_cfg),
                ],
            ],
        };
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            proof.verify(&mut transcript, field(20, &field_cfg), 2, &field_cfg),
            Err(SumcheckError::InvalidRoundClaim { round: 1 })
        );
    }

    #[test]
    fn outer_sumcheck_rejects_a_bad_terminal_evaluation() {
        let field_cfg = config();
        let proof = OuterSumcheckProof {
            sumcheck: SumcheckProof {
                round_polynomials: Vec::new(),
            },
            az_mle_claim: field(2, &field_cfg),
            bz_mle_claim: field(3, &field_cfg),
            cz_mle_claim: field(5, &field_cfg),
        };
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            proof.verify(
                &mut transcript,
                F128::zero_with_cfg(&field_cfg),
                &[],
                &field_cfg,
            ),
            Err(SumcheckError::InvalidTerminalClaim)
        );
    }

    #[test]
    fn outer_sumcheck_supports_every_equality_factor_split() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let num_vars = 5;
        let table_len = 1 << num_vars;
        let products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 + 2, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(3 * index as u64 + 5, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field((index * index) as u64 + 7, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
        };
        let tau = [2, 4, 6, 8, 10]
            .into_iter()
            .map(|value| field(value, &field_cfg))
            .collect::<Vec<_>>();
        let full_equality = eq_table(&tau, &field_cfg).unwrap();
        let mut initial_claim = zero.clone();
        for (index, equality) in full_equality.iter().enumerate() {
            let residual = sub(
                &mul(
                    &products.az.evaluations[index],
                    &products.bz.evaluations[index],
                ),
                &products.cz.evaluations[index],
            );
            initial_claim += &mul(equality, &residual);
        }

        let mut reference_proof = None;
        for split in 0..=num_vars {
            let (low_point, high_point) = tau.split_at(split);
            let equality_factors = (
                DenseMultilinearExtension {
                    evaluations: eq_table(low_point, &field_cfg).unwrap(),
                    num_vars: low_point.len(),
                },
                DenseMultilinearExtension {
                    evaluations: eq_table(high_point, &field_cfg).unwrap(),
                    num_vars: high_point.len(),
                },
            );
            let mut prover_transcript = Blake3Transcript::new();
            let output = prove_outer_sumcheck(
                &mut prover_transcript,
                initial_claim.clone(),
                equality_factors,
                products.clone(),
                &field_cfg,
            )
            .unwrap();

            assert_eq!(output.proof.sumcheck.round_polynomials.len(), num_vars);
            if let Some(reference_proof) = &reference_proof {
                assert_eq!(&output.proof, reference_proof);
            } else {
                reference_proof = Some(output.proof.clone());
            }

            let mut verifier_transcript = Blake3Transcript::new();
            let verified = output
                .proof
                .verify(
                    &mut verifier_transcript,
                    initial_claim.clone(),
                    &tau,
                    &field_cfg,
                )
                .unwrap();
            assert_eq!(verified.eval_points, output.eval_points);
            assert_eq!(verified.az_mle_claim, output.proof.az_mle_claim);
            assert_eq!(verified.bz_mle_claim, output.proof.bz_mle_claim);
            assert_eq!(verified.cz_mle_claim, output.proof.cz_mle_claim);
            assert_eq!(
                squeeze_field::<F128, _>(&mut prover_transcript, &field_cfg),
                squeeze_field::<F128, _>(&mut verifier_transcript, &field_cfg),
            );
        }
    }

    #[test]
    fn factorized_outer_kernels_match_immediate_proof() {
        fn prove_with_reducer<R>(
            initial_claim: &F128,
            equality_factors: &(
                DenseMultilinearExtension<F128>,
                DenseMultilinearExtension<F128>,
            ),
            products: &R1csProductMles<F128>,
            field_cfg: &<F128 as PrimeField>::Config,
            reducer: &R,
        ) -> (OuterSumcheckOutput<F128>, F128)
        where
            R: SumcheckProductReducer<F128>,
        {
            let mut transcript = Blake3Transcript::new();
            let output = prove_outer_sumcheck_with_reducer(
                &mut transcript,
                initial_claim.clone(),
                equality_factors.clone(),
                products.clone(),
                field_cfg,
                reducer,
            )
            .unwrap();
            let continuation = squeeze_field(&mut transcript, field_cfg);
            (output, continuation)
        }

        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let num_vars = 10;
        let table_len = 1 << num_vars;
        let products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 + 2, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(3 * index as u64 + 5, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field((index * index) as u64 + 7, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
        };
        let tau = (0..num_vars)
            .map(|index| field(2 * index as u64 + 3, &field_cfg))
            .collect::<Vec<_>>();
        let (low_point, high_point) = tau.split_at(num_vars / 2);
        let equality_factors = (
            DenseMultilinearExtension {
                evaluations: eq_table(low_point, &field_cfg).unwrap(),
                num_vars: low_point.len(),
            },
            DenseMultilinearExtension {
                evaluations: eq_table(high_point, &field_cfg).unwrap(),
                num_vars: high_point.len(),
            },
        );

        let full_equality = eq_table(&tau, &field_cfg).unwrap();
        let mut initial_claim = zero;
        for (index, equality) in full_equality.iter().enumerate() {
            let residual = sub(
                &mul(
                    &products.az.evaluations[index],
                    &products.bz.evaluations[index],
                ),
                &products.cz.evaluations[index],
            );
            initial_claim += &mul(equality, &residual);
        }

        let immediate = ImmediateSumcheckReducer::new(&field_cfg);
        let optimized = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let reference = CryptoBigintSumcheckReducer::new(&field_cfg).unwrap();
        let (immediate_output, immediate_continuation) = prove_with_reducer(
            &initial_claim,
            &equality_factors,
            &products,
            &field_cfg,
            &immediate,
        );
        let (optimized_output, optimized_continuation) = prove_with_reducer(
            &initial_claim,
            &equality_factors,
            &products,
            &field_cfg,
            &optimized,
        );
        let (reference_output, reference_continuation) = prove_with_reducer(
            &initial_claim,
            &equality_factors,
            &products,
            &field_cfg,
            &reference,
        );

        assert_eq!(optimized_output.proof, immediate_output.proof);
        assert_eq!(optimized_output.eval_points, immediate_output.eval_points);
        assert_eq!(optimized_output.final_claim, immediate_output.final_claim);
        assert_eq!(optimized_continuation, immediate_continuation);
        assert_eq!(reference_output.proof, immediate_output.proof);
        assert_eq!(reference_output.eval_points, immediate_output.eval_points);
        assert_eq!(reference_output.final_claim, immediate_output.final_claim);
        assert_eq!(reference_continuation, immediate_continuation);

        let mut verifier_transcript = Blake3Transcript::new();
        let verified = immediate_output
            .proof
            .verify(&mut verifier_transcript, initial_claim, &tau, &field_cfg)
            .unwrap();
        assert_eq!(verified.eval_points, immediate_output.eval_points);
        assert_eq!(
            squeeze_field::<F128, _>(&mut verifier_transcript, &field_cfg),
            immediate_continuation
        );
    }

    #[test]
    fn inner_sumcheck_folds_the_lowest_coordinate_first() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let matrix = DenseMultilinearExtension::from_evaluations_vec(
            2,
            [2, 5, 11, 17]
                .into_iter()
                .map(|value| field(value, &field_cfg))
                .collect(),
            zero.clone(),
        );
        let witness = DenseMultilinearExtension::from_evaluations_vec(
            2,
            [3, 7, 13, 19]
                .into_iter()
                .map(|value| field(value, &field_cfg))
                .collect(),
            zero,
        );
        let mut transcript = Blake3Transcript::new();

        let output = prove_inner_sumcheck(
            &mut transcript,
            field(507, &field_cfg),
            matrix,
            witness,
            &field_cfg,
        )
        .unwrap();

        assert_eq!(
            output.sumcheck.proof.round_polynomials[0],
            [
                field(149, &field_cfg),
                field(161, &field_cfg),
                field(48, &field_cfg),
            ]
        );
    }

    #[test]
    fn inner_sumcheck_rejects_mismatched_dimensions_before_absorption() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let matrix = DenseMultilinearExtension::from_evaluations_vec(
            1,
            vec![field(1, &field_cfg), field(2, &field_cfg)],
            zero.clone(),
        );
        let witness = DenseMultilinearExtension::from_evaluations_vec(
            2,
            vec![
                field(1, &field_cfg),
                field(2, &field_cfg),
                field(3, &field_cfg),
                field(4, &field_cfg),
            ],
            zero.clone(),
        );
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            prove_inner_sumcheck(&mut transcript, zero.clone(), matrix, witness, &field_cfg,),
            Err(SumcheckError::InvalidProductDimensions)
        );

        let rejected_next = squeeze_field::<F128, _>(&mut transcript, &field_cfg);
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<F128, _>(&mut fresh_transcript, &field_cfg);
        assert_eq!(rejected_next, fresh_next);
    }

    #[test]
    fn outer_sumcheck_rejects_malformed_dimensions_before_absorption() {
        let field_cfg = config();
        let zero = F128::zero_with_cfg(&field_cfg);
        let one = F128::one_with_cfg(&field_cfg);
        let products = R1csProductMles {
            az: DenseMultilinearExtension::zero_vars(field(1, &field_cfg)),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                1,
                vec![field(2, &field_cfg), field(3, &field_cfg)],
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::zero_vars(field(4, &field_cfg)),
        };
        let equality_factors = (
            DenseMultilinearExtension::zero_vars(one.clone()),
            DenseMultilinearExtension::zero_vars(one),
        );
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            prove_outer_sumcheck(
                &mut transcript,
                zero,
                equality_factors,
                products,
                &field_cfg,
            ),
            Err(SumcheckError::InvalidProductDimensions)
        );

        let rejected_next = squeeze_field::<F128, _>(&mut transcript, &field_cfg);
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<F128, _>(&mut fresh_transcript, &field_cfg);
        assert_eq!(rejected_next, fresh_next);
    }
}
