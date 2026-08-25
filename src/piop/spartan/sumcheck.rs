//! Spartan's equality-weighted outer sumcheck and matrix-product inner sumcheck.
//!
//! Round polynomials are stored in coefficient form. The outer proof uses four
//! coefficients (degree three), while the inner proof uses three (degree two).
//! The prover omits the linear coefficient while accumulating a round and
//! reconstructs it from `g(0) + g(1) = current_claim` before absorption.

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use thiserror::Error;

use crate::{poly::mle::DenseMultilinearExtension, transcript::traits::Transcript};

use super::{SpartanField, absorb_field_elements, squeeze_field};

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
    #[error("invalid dense multilinear-extension table")]
    InvalidMleOperation,
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
        &zero,
    );

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
                &zero,
            );
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
                &zero,
            );
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
            sum_inner_round_coefficients_without_linear(&batched_matrix, &witness, &zero);

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
                        &zero,
                    );
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
fn compute_inner_pair_coefficients_without_linear<F>(
    matrix_zero: &F,
    matrix_one: &F,
    witness_zero: &F,
    witness_one: &F,
) -> [F; 2]
where
    F: SpartanField,
{
    [
        mul(matrix_zero, witness_zero),
        mul(
            &sub(matrix_one, matrix_zero),
            &sub(witness_one, witness_zero),
        ),
    ]
}

fn sum_inner_round_coefficients_without_linear<F>(
    batched_matrix: &[F],
    witness: &[F],
    zero: &F,
) -> [F; 2]
where
    F: SpartanField,
{
    debug_assert_eq!(batched_matrix.len(), witness.len());
    debug_assert!(batched_matrix.len() >= 2);

    #[cfg(feature = "parallel")]
    if should_parallelize(batched_matrix.len() / 2) {
        return batched_matrix
            .par_chunks_exact(2)
            .zip(witness.par_chunks_exact(2))
            .fold(
                || [zero.clone(), zero.clone()],
                |sum, (matrix, witness)| {
                    add_coefficients(
                        sum,
                        compute_inner_pair_coefficients_without_linear(
                            &matrix[0],
                            &matrix[1],
                            &witness[0],
                            &witness[1],
                        ),
                    )
                },
            )
            .reduce(|| [zero.clone(), zero.clone()], add_coefficients::<F, 2>);
    }

    batched_matrix
        .chunks_exact(2)
        .zip(witness.chunks_exact(2))
        .fold([zero.clone(), zero.clone()], |sum, (matrix, witness)| {
            add_coefficients(
                sum,
                compute_inner_pair_coefficients_without_linear(
                    &matrix[0],
                    &matrix[1],
                    &witness[0],
                    &witness[1],
                ),
            )
        })
}

#[inline]
fn fold_inner_chunk<F>(
    batched_matrix: &[F],
    witness: &[F],
    batched_matrix_output: &mut [F],
    witness_output: &mut [F],
    challenge: &F,
) -> [F; 2]
where
    F: SpartanField,
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
    compute_inner_pair_coefficients_without_linear(
        &folded_matrix[0],
        &folded_matrix[1],
        &folded_witness[0],
        &folded_witness[1],
    )
}

/// Folds both active inner tables and prepares the next round's `[c0, c2]`.
fn fold_and_compute_next_inner_round_coefficients_without_linear<F>(
    batched_matrix: &[F],
    witness: &[F],
    batched_matrix_output: &mut [F],
    witness_output: &mut [F],
    challenge: &F,
    zero: &F,
) -> [F; 2]
where
    F: SpartanField,
{
    debug_assert_eq!(batched_matrix.len(), witness.len());
    debug_assert!(batched_matrix.len() >= 4);
    debug_assert_eq!(batched_matrix_output.len(), batched_matrix.len() / 2);
    debug_assert_eq!(witness_output.len(), witness.len() / 2);

    #[cfg(feature = "parallel")]
    if should_parallelize(batched_matrix.len() / 4) {
        return batched_matrix
            .par_chunks_exact(4)
            .zip(witness.par_chunks_exact(4))
            .zip(batched_matrix_output.par_chunks_exact_mut(2))
            .zip(witness_output.par_chunks_exact_mut(2))
            .fold(
                || [zero.clone(), zero.clone()],
                |sum, (((matrix, witness), matrix_output), witness_output)| {
                    add_coefficients(
                        sum,
                        fold_inner_chunk(matrix, witness, matrix_output, witness_output, challenge),
                    )
                },
            )
            .reduce(|| [zero.clone(), zero.clone()], add_coefficients::<F, 2>);
    }

    batched_matrix
        .chunks_exact(4)
        .zip(witness.chunks_exact(4))
        .zip(batched_matrix_output.chunks_exact_mut(2))
        .zip(witness_output.chunks_exact_mut(2))
        .fold(
            [zero.clone(), zero.clone()],
            |sum, (((matrix, witness), matrix_output), witness_output)| {
                add_coefficients(
                    sum,
                    fold_inner_chunk(matrix, witness, matrix_output, witness_output, challenge),
                )
            },
        )
}

#[inline]
fn add_coefficients<F, const COEFFS: usize>(left: [F; COEFFS], right: [F; COEFFS]) -> [F; COEFFS]
where
    F: SpartanField,
{
    std::array::from_fn(|index| add(&left[index], &right[index]))
}

fn sum_coefficients<F, const COEFFS: usize>(
    len: usize,
    contribution: impl Fn(usize) -> [F; COEFFS] + Sync,
    zero: &F,
) -> [F; COEFFS]
where
    F: SpartanField,
{
    #[cfg(feature = "parallel")]
    if should_parallelize(len) {
        return (0..len).into_par_iter().map(&contribution).reduce(
            || std::array::from_fn(|_| zero.clone()),
            add_coefficients::<F, COEFFS>,
        );
    }

    (0..len).fold(std::array::from_fn(|_| zero.clone()), |sum, index| {
        add_coefficients(sum, contribution(index))
    })
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
fn cubic_contribution<F>(
    eq_zero: &F,
    eq_one: &F,
    az_zero: &F,
    az_one: &F,
    bz_zero: &F,
    bz_one: &F,
    cz_zero: &F,
    cz_one: &F,
) -> [F; 3]
where
    F: SpartanField,
{
    let eq_delta = sub(eq_one, eq_zero);
    let az_delta = sub(az_one, az_zero);
    let bz_delta = sub(bz_one, bz_zero);
    let cz_delta = sub(cz_one, cz_zero);

    let residual_zero = sub(&mul(az_zero, bz_zero), cz_zero);
    let residual_linear = sub(
        &add(&mul(az_zero, &bz_delta), &mul(&az_delta, bz_zero)),
        &cz_delta,
    );
    let residual_quadratic = mul(&az_delta, &bz_delta);

    [
        mul(eq_zero, &residual_zero),
        add(
            &mul(eq_zero, &residual_quadratic),
            &mul(&eq_delta, &residual_linear),
        ),
        mul(&eq_delta, &residual_quadratic),
    ]
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
}

/// Computes `[c0, c2, c3]` from adjacent pairs in the product tables.
fn compute_coefficients_without_linear<F>(
    products: &R1csProductTableBuffers<F>,
    equality_pairs: EqualityPairs<'_, F>,
    zero: &F,
) -> [F; 3]
where
    F: SpartanField,
{
    let pair_count = products.len() / 2;
    sum_coefficients(
        pair_count,
        |pair| {
            let index = 2 * pair;
            let [eq_zero, eq_one] = equality_pairs.pair(pair);
            cubic_contribution(
                &eq_zero,
                &eq_one,
                &products.az[index],
                &products.az[index + 1],
                &products.bz[index],
                &products.bz[index + 1],
                &products.cz[index],
                &products.cz[index + 1],
            )
        },
        zero,
    )
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

/// Folds all product tables and accumulates the next round polynomial.
fn fold_products_and_compute_next<F>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_pairs: EqualityPairs<'_, F>,
    zero: &F,
) -> [F; 3]
where
    F: SpartanField,
{
    debug_assert_eq!(input.len(), 2 * output.len());

    let accumulate = |sum: [F; 3],
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
        add_coefficients(
            sum,
            cubic_contribution(
                &eq_zero, &eq_one, &az[0], &az[1], &bz[0], &bz[1], &cz[0], &cz[1],
            ),
        )
    };

    let chunk_count = output.len() / 2;

    #[cfg(feature = "parallel")]
    if should_parallelize(chunk_count) {
        return (
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
                || [zero.clone(), zero.clone(), zero.clone()],
                |sum, (chunk, (az, bz, cz, az_output, bz_output, cz_output))| {
                    accumulate(sum, chunk, az, bz, cz, az_output, bz_output, cz_output)
                },
            )
            .reduce(
                || [zero.clone(), zero.clone(), zero.clone()],
                add_coefficients::<F, 3>,
            );
    }

    let mut sum = [zero.clone(), zero.clone(), zero.clone()];
    for chunk in 0..chunk_count {
        let input_start = 4 * chunk;
        let output_start = 2 * chunk;
        sum = accumulate(
            sum,
            chunk,
            &input.az[input_start..input_start + 4],
            &input.bz[input_start..input_start + 4],
            &input.cz[input_start..input_start + 4],
            &mut output.az[output_start..output_start + 2],
            &mut output.bz[output_start..output_start + 2],
            &mut output.cz[output_start..output_start + 2],
        );
    }
    sum
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
