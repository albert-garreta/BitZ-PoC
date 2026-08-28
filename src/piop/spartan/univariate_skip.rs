//! Known-zero prefix skipping for Spartan's outer sumcheck.
//!
//! The first `K` little-endian row variables are represented by one
//! univariate variable over the base domain `D = {0, ..., 2^K - 1}`.  The
//! resulting residual polynomial vanishes on `D`, so the prover commits to
//! its lower-degree cofactor before the transcript samples the univariate
//! point.  The remaining row variables are then handled by the existing cubic
//! outer sumcheck without changing its proof or transcript format.

use crate::{poly::mle::DenseMultilinearExtension, transcript::traits::Transcript};

use super::{
    SpartanField, absorb_field_elements,
    matrix::{PrefixUnivariateRowFactors, make_equality_factors},
    squeeze_field,
    sumcheck::{
        OuterSumcheckProof, R1csProductMles, SumcheckError, SumcheckProductReducer, SumcheckProof,
        prove_outer_sumcheck_with_reducer,
    },
};

/// The known-zero univariate message preceding the cubic tail sumcheck.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnivariateSkipProof<F> {
    pub skip_vars: u8,
    /// `Q` at the `2^K - 2` balanced exterior nodes, in canonical order.
    pub finite_q_evaluations: Box<[F]>,
    /// The coefficient of `Y^(2 * (2^K - 1))` in `Q`.
    pub q_at_infinity: F,
}

/// A known-zero prefix reduction followed by the existing cubic outer proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnivariateSkipOuterSumcheckProof<F> {
    pub skip: UnivariateSkipProof<F>,
    /// Existing cubic outer sumcheck over the remaining `n - K` variables.
    pub tail: OuterSumcheckProof<F>,
}

/// The univariate-skip outer proof composed with Spartan's existing inner
/// sumcheck proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnivariateSkipSpartanPiopProof<F> {
    pub outer: UnivariateSkipOuterSumcheckProof<F>,
    pub inner: SumcheckProof<F, 3>,
}

/// The row functional produced by a prefix-univariate outer reduction.
///
/// For `M = 2^K`, the weight at row `s + M*x` is
/// `L_s(z) * eq(tail_point, x)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrefixUnivariateRowBinding<F> {
    pub skip_vars: u8,
    pub z: F,
    pub tail_point: Vec<F>,
}

impl<F> PrefixUnivariateRowBinding<F>
where
    F: SpartanField,
{
    /// Builds the small prefix vector and the two tail equality factors.
    pub(crate) fn row_factors(
        &self,
        num_row_vars: usize,
        field_cfg: &F::Config,
    ) -> Result<PrefixUnivariateRowFactors<F>, SumcheckError> {
        let skip_vars = usize::from(self.skip_vars);
        validate_skip_vars(skip_vars, num_row_vars)?;
        if self.tail_point.len() != num_row_vars - skip_vars {
            return Err(SumcheckError::InvalidEqualityDimensions);
        }

        let block_len = 1usize << skip_vars;
        let prefix_weights = lagrange_weights_at(&self.z, block_len, field_cfg);
        let (tail_low, tail_high) = make_equality_factors(&self.tail_point, field_cfg)
            .map_err(|_| SumcheckError::InvalidEqualityDimensions)?;
        PrefixUnivariateRowFactors::new(
            skip_vars,
            prefix_weights,
            tail_low,
            tail_high,
            num_row_vars,
        )
        .map_err(|_| SumcheckError::InvalidEqualityDimensions)
    }

    /// Materializes weights in the matrices' little-endian row order.
    #[allow(dead_code)]
    pub(crate) fn row_weights(
        &self,
        num_row_vars: usize,
        field_cfg: &F::Config,
    ) -> Result<Vec<F>, SumcheckError> {
        Ok(self.row_factors(num_row_vars, field_cfg)?.materialize())
    }
}

/// Prover-local result of the composed univariate-skip outer reduction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnivariateSkipOuterSumcheckOutput<F> {
    pub proof: UnivariateSkipOuterSumcheckProof<F>,
    pub row_binding: PrefixUnivariateRowBinding<F>,
    pub final_claim: F,
}

/// Verifier result of the composed univariate-skip outer reduction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnivariateSkipOuterVerifierOutput<F> {
    pub row_binding: PrefixUnivariateRowBinding<F>,
    pub az_mle_claim: F,
    pub bz_mle_claim: F,
    pub cz_mle_claim: F,
}

/// Output of the known-zero reduction before the cubic tail is checked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnivariateSkipReductionOutput<F> {
    pub z: F,
    pub q_at_z: F,
}

mod sealed {
    pub trait Sealed {}
}

/// Compile-time arithmetic layout for one supported prefix width.
///
/// Native-u32 kernels use signed interpolation sums for `A` and `B`, and a
/// wider signed type for `C` and the residual.  The field-generic kernel below
/// shares the same marker family while performing its arithmetic directly in
/// `F`.
pub(crate) trait PrefixSkipSpec: sealed::Sealed {
    const SKIP_VARS: usize;
    const BLOCK_LEN: usize;

    type InterpolatedAB;
    type Residual;
}

macro_rules! define_prefix_skip_spec {
    ($name:ident, $skip_vars:literal, $block_len:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) struct $name;

        impl sealed::Sealed for $name {}

        impl PrefixSkipSpec for $name {
            const SKIP_VARS: usize = $skip_vars;
            const BLOCK_LEN: usize = $block_len;

            type InterpolatedAB = i64;
            type Residual = i128;
        }
    };
}

define_prefix_skip_spec!(PrefixSkipK1, 1, 2);
define_prefix_skip_spec!(PrefixSkipK2, 2, 4);
define_prefix_skip_spec!(PrefixSkipK3, 3, 8);
define_prefix_skip_spec!(PrefixSkipK4, 4, 16);

const K1_EXTERIOR_NODES: [i32; 0] = [];
const K2_EXTERIOR_NODES: [i32; 2] = [-1, 4];
const K3_EXTERIOR_NODES: [i32; 6] = [-1, 8, -2, 9, -3, 10];
const K4_EXTERIOR_NODES: [i32; 14] = [-1, 16, -2, 17, -3, 18, -4, 19, -5, 20, -6, 21, -7, 22];

const K1_FINITE_LAGRANGE: [[i128; 2]; 0] = exterior_lagrange_coefficients(K1_EXTERIOR_NODES);
const K2_FINITE_LAGRANGE: [[i128; 4]; 2] = exterior_lagrange_coefficients(K2_EXTERIOR_NODES);
const K3_FINITE_LAGRANGE: [[i128; 8]; 6] = exterior_lagrange_coefficients(K3_EXTERIOR_NODES);
const K4_FINITE_LAGRANGE: [[i128; 16]; 14] = exterior_lagrange_coefficients(K4_EXTERIOR_NODES);

const K1_TOP_DIFFERENCE: [i128; 2] = top_difference_coefficients();
const K2_TOP_DIFFERENCE: [i128; 4] = top_difference_coefficients();
const K3_TOP_DIFFERENCE: [i128; 8] = top_difference_coefficients();
const K4_TOP_DIFFERENCE: [i128; 16] = top_difference_coefficients();

const K1_VANISHING_AT_EXTERIOR: [i128; 0] = vanishing_at_nodes::<2, 0>(K1_EXTERIOR_NODES);
const K2_VANISHING_AT_EXTERIOR: [i128; 2] = vanishing_at_nodes::<4, 2>(K2_EXTERIOR_NODES);
const K3_VANISHING_AT_EXTERIOR: [i128; 6] = vanishing_at_nodes::<8, 6>(K3_EXTERIOR_NODES);
const K4_VANISHING_AT_EXTERIOR: [i128; 14] = vanishing_at_nodes::<16, 14>(K4_EXTERIOR_NODES);

const K1_EXTERIOR_INTERPOLATION_DENOMINATORS: [i128; 0] =
    interpolation_denominators(K1_EXTERIOR_NODES);
const K2_EXTERIOR_INTERPOLATION_DENOMINATORS: [i128; 2] =
    interpolation_denominators(K2_EXTERIOR_NODES);
const K3_EXTERIOR_INTERPOLATION_DENOMINATORS: [i128; 6] =
    interpolation_denominators(K3_EXTERIOR_NODES);
const K4_EXTERIOR_INTERPOLATION_DENOMINATORS: [i128; 14] =
    interpolation_denominators(K4_EXTERIOR_NODES);

/// Computes the field-generic skip message in transcript order: all finite
/// exterior coordinates, followed by `Q(infinity)`.
pub(crate) fn compute_field_skip_message<F>(
    skip_vars: usize,
    equality_factors: (&DenseMultilinearExtension<F>, &DenseMultilinearExtension<F>),
    products: &R1csProductMles<F>,
    field_cfg: &F::Config,
) -> Result<Vec<F>, SumcheckError>
where
    F: SpartanField,
{
    validate_products_for_skip(skip_vars, products)?;
    validate_equality_factors(equality_factors, products.az.num_vars - skip_vars)?;

    match skip_vars {
        1 => compute_field_skip_message_for_layout(
            equality_factors,
            products,
            field_cfg,
            &K1_FINITE_LAGRANGE,
            &K1_TOP_DIFFERENCE,
        ),
        2 => compute_field_skip_message_for_layout(
            equality_factors,
            products,
            field_cfg,
            &K2_FINITE_LAGRANGE,
            &K2_TOP_DIFFERENCE,
        ),
        3 => compute_field_skip_message_for_layout(
            equality_factors,
            products,
            field_cfg,
            &K3_FINITE_LAGRANGE,
            &K3_TOP_DIFFERENCE,
        ),
        4 => compute_field_skip_message_for_layout(
            equality_factors,
            products,
            field_cfg,
            &K4_FINITE_LAGRANGE,
            &K4_TOP_DIFFERENCE,
        ),
        _ => Err(SumcheckError::InvalidProductDimensions),
    }
}

/// Folds the first `K` little-endian variables of `Az`, `Bz`, and `Cz` at
/// `z`, leaving field-valued tables over the suffix variables.
pub(crate) fn fold_field_prefix<F>(
    products: R1csProductMles<F>,
    skip_vars: usize,
    z: &F,
    field_cfg: &F::Config,
) -> Result<R1csProductMles<F>, SumcheckError>
where
    F: SpartanField,
{
    validate_products_for_skip(skip_vars, &products)?;
    let block_len = 1usize << skip_vars;
    let tail_vars = products.az.num_vars - skip_vars;
    let tail_len = 1usize << tail_vars;
    let weights = lagrange_weights_at(z, block_len, field_cfg);
    let zero = F::zero_with_cfg(field_cfg);

    let fold_table = |evaluations: &[F]| {
        let mut folded = Vec::with_capacity(tail_len);
        for block in evaluations.chunks_exact(block_len) {
            let mut value = zero.clone();
            for (weight, evaluation) in weights.iter().zip(block) {
                value += &mul(weight, evaluation);
            }
            folded.push(value);
        }
        DenseMultilinearExtension {
            evaluations: folded,
            num_vars: tail_vars,
        }
    };

    Ok(R1csProductMles {
        az: fold_table(&products.az.evaluations),
        bz: fold_table(&products.bz.evaluations),
        cz: fold_table(&products.cz.evaluations),
    })
}

/// Runs the field-generic known-zero prefix reduction and then delegates the
/// suffix to the existing cubic outer-sumcheck prover.
pub(crate) fn prove_univariate_skip_outer_sumcheck_with_reducer<F, R>(
    transcript: &mut impl Transcript,
    skip_vars: usize,
    tau_tail: &[F],
    equality_factors: (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
    reducer: &R,
) -> Result<UnivariateSkipOuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    validate_products_for_skip(skip_vars, &products)?;
    if tau_tail.len() != products.az.num_vars - skip_vars {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    validate_equality_factors((&equality_factors.0, &equality_factors.1), tau_tail.len())?;

    let message = {
        let _scope = crate::utils::prof::scope("spartan:univariate_skip_message");
        compute_field_skip_message(
            skip_vars,
            (&equality_factors.0, &equality_factors.1),
            &products,
            field_cfg,
        )?
    };
    let skip = UnivariateSkipProof::from_ordered_message(skip_vars, message)?;
    let reduction = skip.verify_reduction(transcript, field_cfg)?;
    let folded = {
        let _scope = crate::utils::prof::scope("spartan:univariate_skip_prefix_fold");
        fold_field_prefix(products, skip_vars, &reduction.z, field_cfg)?
    };
    let tail = {
        let _scope = crate::utils::prof::scope("spartan:univariate_skip_tail");
        prove_outer_sumcheck_with_reducer(
            transcript,
            reduction.q_at_z,
            tau_tail,
            equality_factors,
            folded,
            field_cfg,
            reducer,
        )?
    };

    Ok(UnivariateSkipOuterSumcheckOutput {
        proof: UnivariateSkipOuterSumcheckProof {
            skip: skip.clone(),
            tail: tail.proof,
        },
        row_binding: PrefixUnivariateRowBinding {
            skip_vars: skip.skip_vars,
            z: reduction.z,
            tail_point: tail.eval_points,
        },
        final_claim: tail.final_claim,
    })
}

impl<F> UnivariateSkipProof<F>
where
    F: SpartanField,
{
    /// Splits a message ordered as finite coordinates followed by infinity.
    pub(crate) fn from_ordered_message(
        skip_vars: usize,
        mut message: Vec<F>,
    ) -> Result<Self, SumcheckError> {
        let expected =
            skip_message_len(skip_vars).ok_or(SumcheckError::InvalidProductDimensions)?;
        if message.len() != expected {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let q_at_infinity = message
            .pop()
            .ok_or(SumcheckError::InvalidProductDimensions)?;
        Ok(Self {
            skip_vars: u8::try_from(skip_vars)
                .map_err(|_| SumcheckError::InvalidProductDimensions)?,
            finite_q_evaluations: message.into_boxed_slice(),
            q_at_infinity,
        })
    }

    /// Validates the proof shape without modifying a transcript.
    pub(crate) fn validate_shape(&self, num_row_vars: usize) -> Result<(), SumcheckError> {
        let skip_vars = usize::from(self.skip_vars);
        validate_skip_vars(skip_vars, num_row_vars)?;
        if self.finite_q_evaluations.len() != (1usize << skip_vars) - 2 {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }

    /// Commits the skip message, samples `z`, and reconstructs `Q(z)`.
    ///
    /// This verifies only the known-zero reduction.  The cubic tail remains
    /// responsible for binding `Q(z)` to folded `Az`, `Bz`, and `Cz` tables.
    pub(crate) fn verify_reduction(
        &self,
        transcript: &mut impl Transcript,
        field_cfg: &F::Config,
    ) -> Result<UnivariateSkipReductionOutput<F>, SumcheckError> {
        self.validate_shape(usize::from(self.skip_vars))?;
        validate_field_elements(&self.finite_q_evaluations, field_cfg)?;
        validate_field_elements(std::slice::from_ref(&self.q_at_infinity), field_cfg)?;

        let mut message = Vec::with_capacity(self.finite_q_evaluations.len() + 1);
        message.extend_from_slice(&self.finite_q_evaluations);
        message.push(self.q_at_infinity.clone());
        absorb_field_elements(transcript, &message);
        let z = squeeze_field(transcript, field_cfg);
        let q_at_z = {
            let _scope = crate::utils::prof::scope("spartan:univariate_skip_reconstruct");
            self.reconstruct_at(&z, field_cfg)?
        };
        Ok(UnivariateSkipReductionOutput { z, q_at_z })
    }

    /// Reconstructs `Q(z) = Z_D(z) H(z)` without a division depending on `z`.
    pub(crate) fn reconstruct_at(&self, z: &F, field_cfg: &F::Config) -> Result<F, SumcheckError> {
        match usize::from(self.skip_vars) {
            1 => reconstruct_for_layout::<F, 2, 0>(
                &self.finite_q_evaluations,
                &self.q_at_infinity,
                z,
                field_cfg,
                &K1_EXTERIOR_NODES,
                &K1_VANISHING_AT_EXTERIOR,
                &K1_EXTERIOR_INTERPOLATION_DENOMINATORS,
            ),
            2 => reconstruct_for_layout::<F, 4, 2>(
                &self.finite_q_evaluations,
                &self.q_at_infinity,
                z,
                field_cfg,
                &K2_EXTERIOR_NODES,
                &K2_VANISHING_AT_EXTERIOR,
                &K2_EXTERIOR_INTERPOLATION_DENOMINATORS,
            ),
            3 => reconstruct_for_layout::<F, 8, 6>(
                &self.finite_q_evaluations,
                &self.q_at_infinity,
                z,
                field_cfg,
                &K3_EXTERIOR_NODES,
                &K3_VANISHING_AT_EXTERIOR,
                &K3_EXTERIOR_INTERPOLATION_DENOMINATORS,
            ),
            4 => reconstruct_for_layout::<F, 16, 14>(
                &self.finite_q_evaluations,
                &self.q_at_infinity,
                z,
                field_cfg,
                &K4_EXTERIOR_NODES,
                &K4_VANISHING_AT_EXTERIOR,
                &K4_EXTERIOR_INTERPOLATION_DENOMINATORS,
            ),
            _ => Err(SumcheckError::InvalidProductDimensions),
        }
    }
}

impl<F> UnivariateSkipOuterSumcheckProof<F>
where
    F: SpartanField,
{
    /// Validates all structural information needed before transcript mutation.
    pub(crate) fn validate_shape(&self, num_row_vars: usize) -> Result<(), SumcheckError> {
        self.skip.validate_shape(num_row_vars)?;
        let expected_tail_rounds = num_row_vars - usize::from(self.skip.skip_vars);
        let actual_tail_rounds = self.tail.sumcheck.round_polynomials.len();
        if actual_tail_rounds != expected_tail_rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: expected_tail_rounds,
                actual: actual_tail_rounds,
            });
        }
        Ok(())
    }

    /// Verifies the skip reduction and the existing cubic tail reduction.
    ///
    /// The surrounding Spartan verifier remains responsible for evaluating
    /// matrices under the returned row functional and checking the inner
    /// sumcheck and assignment opening.
    pub(crate) fn verify(
        &self,
        transcript: &mut impl Transcript,
        tau_tail: &[F],
        num_row_vars: usize,
        field_cfg: &F::Config,
    ) -> Result<UnivariateSkipOuterVerifierOutput<F>, SumcheckError> {
        self.validate_shape(num_row_vars)?;
        if tau_tail.len() != num_row_vars - usize::from(self.skip.skip_vars) {
            return Err(SumcheckError::InvalidEqualityDimensions);
        }
        validate_field_elements(tau_tail, field_cfg)?;
        validate_field_elements(&self.skip.finite_q_evaluations, field_cfg)?;
        validate_field_elements(std::slice::from_ref(&self.skip.q_at_infinity), field_cfg)?;
        for round in &self.tail.sumcheck.round_polynomials {
            validate_field_elements(round, field_cfg)?;
        }
        validate_field_elements(
            &[
                self.tail.az_mle_claim.clone(),
                self.tail.bz_mle_claim.clone(),
                self.tail.cz_mle_claim.clone(),
            ],
            field_cfg,
        )?;

        let reduction = self.skip.verify_reduction(transcript, field_cfg)?;
        let tail = self
            .tail
            .verify(transcript, reduction.q_at_z, tau_tail, field_cfg)?;

        Ok(UnivariateSkipOuterVerifierOutput {
            row_binding: PrefixUnivariateRowBinding {
                skip_vars: self.skip.skip_vars,
                z: reduction.z,
                tail_point: tail.eval_points,
            },
            az_mle_claim: tail.az_mle_claim,
            bz_mle_claim: tail.bz_mle_claim,
            cz_mle_claim: tail.cz_mle_claim,
        })
    }
}

fn compute_field_skip_message_for_layout<F, const BLOCK_LEN: usize, const FINITE_COUNT: usize>(
    equality_factors: (&DenseMultilinearExtension<F>, &DenseMultilinearExtension<F>),
    products: &R1csProductMles<F>,
    field_cfg: &F::Config,
    finite_lagrange: &[[i128; BLOCK_LEN]; FINITE_COUNT],
    top_difference: &[i128; BLOCK_LEN],
) -> Result<Vec<F>, SumcheckError>
where
    F: SpartanField,
{
    let zero = F::zero_with_cfg(field_cfg);
    let one = F::one_with_cfg(field_cfg);
    let finite_weights = finite_lagrange
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|coefficient| field_from_signed(*coefficient, field_cfg))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut factorial = one.clone();
    for factor in 2..BLOCK_LEN {
        factorial *= &field_from_usize(factor, field_cfg);
    }
    let inverse_factorial = one / &factorial;
    let leading_weights = top_difference
        .iter()
        .map(|coefficient| {
            mul(
                &field_from_signed(*coefficient, field_cfg),
                &inverse_factorial,
            )
        })
        .collect::<Vec<_>>();

    let suffix_count = products.az.evaluations.len() / BLOCK_LEN;
    let mut message = vec![zero.clone(); FINITE_COUNT + 1];
    for suffix in 0..suffix_count {
        let equality = equality_weight(equality_factors, suffix);
        let start = suffix * BLOCK_LEN;
        let az = &products.az.evaluations[start..start + BLOCK_LEN];
        let bz = &products.bz.evaluations[start..start + BLOCK_LEN];
        let cz = &products.cz.evaluations[start..start + BLOCK_LEN];

        for (lane, weights) in finite_weights.iter().enumerate() {
            let a_at = inner_product(weights, az, &zero);
            let b_at = inner_product(weights, bz, &zero);
            let c_at = inner_product(weights, cz, &zero);
            let residual = sub(&mul(&a_at, &b_at), &c_at);
            message[lane] += &mul(&equality, &residual);
        }

        let a_leading = inner_product(&leading_weights, az, &zero);
        let b_leading = inner_product(&leading_weights, bz, &zero);
        let infinity = mul(&equality, &mul(&a_leading, &b_leading));
        message[FINITE_COUNT] += &infinity;
    }
    Ok(message)
}

fn reconstruct_for_layout<F, const BLOCK_LEN: usize, const FINITE_COUNT: usize>(
    finite_q_evaluations: &[F],
    q_at_infinity: &F,
    z: &F,
    field_cfg: &F::Config,
    exterior_nodes: &[i32; FINITE_COUNT],
    vanishing_at_exterior: &[i128; FINITE_COUNT],
    interpolation_denominators: &[i128; FINITE_COUNT],
) -> Result<F, SumcheckError>
where
    F: SpartanField,
{
    if finite_q_evaluations.len() != FINITE_COUNT {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let one = F::one_with_cfg(field_cfg);
    let exterior_differences = exterior_nodes
        .iter()
        .map(|node| sub(z, &field_from_signed(i128::from(*node), field_cfg)))
        .collect::<Vec<_>>();
    let mut prefix_products = Vec::with_capacity(FINITE_COUNT + 1);
    prefix_products.push(one.clone());
    for difference in &exterior_differences {
        prefix_products.push(mul(
            prefix_products.last().expect("prefix starts with one"),
            difference,
        ));
    }
    let mut suffix_products = vec![one.clone(); FINITE_COUNT + 1];
    for index in (0..FINITE_COUNT).rev() {
        suffix_products[index] = mul(&exterior_differences[index], &suffix_products[index + 1]);
    }

    // H has degree FINITE_COUNT.  Its leading coefficient is Q(infinity),
    // while its finite values are Q(lambda) / Z_D(lambda).  Combining both
    // fixed denominators lets one batch inversion serve every finite term.
    let fixed_denominators = vanishing_at_exterior
        .iter()
        .zip(interpolation_denominators)
        .map(|(vanishing, interpolation)| {
            mul(
                &field_from_signed(*vanishing, field_cfg),
                &field_from_signed(*interpolation, field_cfg),
            )
        })
        .collect::<Vec<_>>();
    let inverse_denominators = batch_invert(&fixed_denominators, field_cfg);

    let exterior_product = &prefix_products[FINITE_COUNT];
    let mut h_at_z = mul(q_at_infinity, exterior_product);
    for index in 0..FINITE_COUNT {
        let numerator = mul(&prefix_products[index], &suffix_products[index + 1]);
        let contribution = mul(
            &mul(&finite_q_evaluations[index], &inverse_denominators[index]),
            &numerator,
        );
        h_at_z += &contribution;
    }

    let mut base_vanishing = one;
    for base_node in 0..BLOCK_LEN {
        base_vanishing *= &sub(z, &field_from_usize(base_node, field_cfg));
    }
    Ok(mul(&base_vanishing, &h_at_z))
}

fn validate_products_for_skip<F>(
    skip_vars: usize,
    products: &R1csProductMles<F>,
) -> Result<(), SumcheckError> {
    let num_vars = products.az.num_vars;
    validate_skip_vars(skip_vars, num_vars)?;
    if !has_dense_shape(&products.az)
        || !has_dense_shape(&products.bz)
        || !has_dense_shape(&products.cz)
        || products.bz.num_vars != num_vars
        || products.cz.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    Ok(())
}

fn validate_skip_vars(skip_vars: usize, num_row_vars: usize) -> Result<(), SumcheckError> {
    if !(1..=4).contains(&skip_vars) || skip_vars > num_row_vars {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    Ok(())
}

fn validate_equality_factors<F>(
    equality_factors: (&DenseMultilinearExtension<F>, &DenseMultilinearExtension<F>),
    expected_vars: usize,
) -> Result<(), SumcheckError> {
    if !has_dense_shape(equality_factors.0)
        || !has_dense_shape(equality_factors.1)
        || equality_factors
            .0
            .num_vars
            .checked_add(equality_factors.1.num_vars)
            != Some(expected_vars)
    {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    Ok(())
}

fn validate_field_elements<F>(values: &[F], field_cfg: &F::Config) -> Result<(), SumcheckError>
where
    F: SpartanField,
{
    let expected_modulus = F::canonical_modulus_encoding(field_cfg);
    for value in values {
        if F::canonical_modulus_encoding(value.cfg()) != expected_modulus {
            return Err(SumcheckError::FieldConfigurationMismatch);
        }
        value
            .validate_element()
            .map_err(|_| SumcheckError::NonCanonicalFieldElement)?;
    }
    Ok(())
}

fn has_dense_shape<F>(mle: &DenseMultilinearExtension<F>) -> bool {
    mle.num_vars < usize::BITS as usize && mle.evaluations.len() == (1usize << mle.num_vars)
}

fn skip_message_len(skip_vars: usize) -> Option<usize> {
    (1..=4)
        .contains(&skip_vars)
        .then(|| (1usize << skip_vars) - 1)
}

fn equality_weight<F>(
    equality_factors: (&DenseMultilinearExtension<F>, &DenseMultilinearExtension<F>),
    suffix: usize,
) -> F
where
    F: SpartanField,
{
    let low_len = equality_factors.0.evaluations.len();
    let low_index = suffix & (low_len - 1);
    let high_index = suffix >> equality_factors.0.num_vars;
    mul(
        &equality_factors.0.evaluations[low_index],
        &equality_factors.1.evaluations[high_index],
    )
}

fn lagrange_weights_at<F>(point: &F, block_len: usize, field_cfg: &F::Config) -> Vec<F>
where
    F: SpartanField,
{
    let one = F::one_with_cfg(field_cfg);
    let differences = (0..block_len)
        .map(|node| sub(point, &field_from_usize(node, field_cfg)))
        .collect::<Vec<_>>();
    let mut prefix = Vec::with_capacity(block_len + 1);
    prefix.push(one.clone());
    for difference in &differences {
        prefix.push(mul(
            prefix.last().expect("prefix starts with one"),
            difference,
        ));
    }
    let mut suffix = vec![one.clone(); block_len + 1];
    for index in (0..block_len).rev() {
        suffix[index] = mul(&differences[index], &suffix[index + 1]);
    }

    let degree = block_len - 1;
    let mut factorial = one.clone();
    for factor in 2..=degree {
        factorial *= &field_from_usize(factor, field_cfg);
    }
    let mut inverse_factorials = vec![one.clone(); block_len];
    inverse_factorials[degree] = one / &factorial;
    for index in (1..=degree).rev() {
        inverse_factorials[index - 1] = mul(
            &inverse_factorials[index],
            &field_from_usize(index, field_cfg),
        );
    }

    (0..block_len)
        .map(|index| {
            let numerator = mul(&prefix[index], &suffix[index + 1]);
            let denominator_inverse = mul(
                &inverse_factorials[index],
                &inverse_factorials[degree - index],
            );
            let weight = mul(&numerator, &denominator_inverse);
            if (degree - index) % 2 == 0 {
                weight
            } else {
                sub(&F::zero_with_cfg(field_cfg), &weight)
            }
        })
        .collect()
}

fn inner_product<F>(weights: &[F], values: &[F], zero: &F) -> F
where
    F: SpartanField,
{
    weights
        .iter()
        .zip(values)
        .fold(zero.clone(), |mut result, (weight, value)| {
            result += &mul(weight, value);
            result
        })
}

fn batch_invert<F>(values: &[F], field_cfg: &F::Config) -> Vec<F>
where
    F: SpartanField,
{
    if values.is_empty() {
        return Vec::new();
    }
    let one = F::one_with_cfg(field_cfg);
    let mut prefixes = Vec::with_capacity(values.len());
    let mut product = one.clone();
    for value in values {
        debug_assert!(!F::is_zero(value));
        prefixes.push(product.clone());
        product *= value;
    }

    let mut inverse = one / &product;
    let mut inverses = vec![F::zero_with_cfg(field_cfg); values.len()];
    for index in (0..values.len()).rev() {
        inverses[index] = mul(&inverse, &prefixes[index]);
        inverse *= &values[index];
    }
    inverses
}

fn field_from_usize<F>(value: usize, field_cfg: &F::Config) -> F
where
    F: SpartanField,
{
    field_from_unsigned(value as u128, field_cfg)
}

fn field_from_signed<F>(value: i128, field_cfg: &F::Config) -> F
where
    F: SpartanField,
{
    let magnitude = field_from_unsigned(value.unsigned_abs(), field_cfg);
    if value.is_negative() {
        sub(&F::zero_with_cfg(field_cfg), &magnitude)
    } else {
        magnitude
    }
}

fn field_from_unsigned<F>(mut value: u128, field_cfg: &F::Config) -> F
where
    F: SpartanField,
{
    let mut result = F::zero_with_cfg(field_cfg);
    let mut power = F::one_with_cfg(field_cfg);
    while value != 0 {
        if value & 1 == 1 {
            result += &power;
        }
        value >>= 1;
        if value != 0 {
            power = add(&power, &power);
        }
    }
    result
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

const fn exterior_lagrange_coefficients<const BLOCK_LEN: usize, const FINITE_COUNT: usize>(
    nodes: [i32; FINITE_COUNT],
) -> [[i128; BLOCK_LEN]; FINITE_COUNT] {
    let mut result = [[0_i128; BLOCK_LEN]; FINITE_COUNT];
    let mut lane = 0;
    while lane < FINITE_COUNT {
        let mut basis = 0;
        while basis < BLOCK_LEN {
            let mut numerator = 1_i128;
            let mut denominator = 1_i128;
            let mut node = 0;
            while node < BLOCK_LEN {
                if node != basis {
                    numerator *= nodes[lane] as i128 - node as i128;
                    denominator *= basis as i128 - node as i128;
                }
                node += 1;
            }
            result[lane][basis] = numerator / denominator;
            basis += 1;
        }
        lane += 1;
    }
    result
}

const fn top_difference_coefficients<const BLOCK_LEN: usize>() -> [i128; BLOCK_LEN] {
    let degree = BLOCK_LEN - 1;
    let mut result = [0_i128; BLOCK_LEN];
    let mut binomial = 1_i128;
    let mut index = 0;
    while index < BLOCK_LEN {
        result[index] = if (degree - index) % 2 == 0 {
            binomial
        } else {
            -binomial
        };
        if index < degree {
            binomial = binomial * (degree - index) as i128 / (index + 1) as i128;
        }
        index += 1;
    }
    result
}

const fn vanishing_at_nodes<const BLOCK_LEN: usize, const FINITE_COUNT: usize>(
    nodes: [i32; FINITE_COUNT],
) -> [i128; FINITE_COUNT] {
    let mut result = [0_i128; FINITE_COUNT];
    let mut index = 0;
    while index < FINITE_COUNT {
        let mut value = 1_i128;
        let mut base = 0;
        while base < BLOCK_LEN {
            value *= nodes[index] as i128 - base as i128;
            base += 1;
        }
        result[index] = value;
        index += 1;
    }
    result
}

const fn interpolation_denominators<const FINITE_COUNT: usize>(
    nodes: [i32; FINITE_COUNT],
) -> [i128; FINITE_COUNT] {
    let mut result = [0_i128; FINITE_COUNT];
    let mut index = 0;
    while index < FINITE_COUNT {
        let mut value = 1_i128;
        let mut other = 0;
        while other < FINITE_COUNT {
            if other != index {
                value *= nodes[index] as i128 - nodes[other] as i128;
            }
            other += 1;
        }
        result[index] = value;
        index += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        FromWithConfig, PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint,
    };

    use crate::{piop::spartan::matrix::make_equality_factors, transcript::Blake3Transcript};

    use super::{super::sumcheck::ImmediateSumcheckReducer, *};

    const TEST_MODULUS: u128 = (1_u128 << 100) - 15;

    fn config() -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(TEST_MODULUS)).expect("prime test modulus")
    }

    fn field(value: u64, field_cfg: &<F128 as PrimeField>::Config) -> F128 {
        F128::from_with_cfg(value, field_cfg)
    }

    fn valid_products(
        num_vars: usize,
        field_cfg: &<F128 as PrimeField>::Config,
    ) -> R1csProductMles<F128> {
        let len = 1usize << num_vars;
        let az = (0..len)
            .map(|index| field((7 * index as u64 + 3) % 101, field_cfg))
            .collect::<Vec<_>>();
        let bz = (0..len)
            .map(|index| field((11 * index as u64 + 5) % 103, field_cfg))
            .collect::<Vec<_>>();
        let cz = az
            .iter()
            .zip(&bz)
            .map(|(a, b)| mul(a, b))
            .collect::<Vec<_>>();
        let dense = |evaluations| DenseMultilinearExtension {
            evaluations,
            num_vars,
        };
        R1csProductMles {
            az: dense(az),
            bz: dense(bz),
            cz: dense(cz),
        }
    }

    fn direct_q_at(
        products: &R1csProductMles<F128>,
        equality_factors: (
            &DenseMultilinearExtension<F128>,
            &DenseMultilinearExtension<F128>,
        ),
        skip_vars: usize,
        point: &F128,
        field_cfg: &<F128 as PrimeField>::Config,
    ) -> F128 {
        let block_len = 1usize << skip_vars;
        let weights = lagrange_weights_at(point, block_len, field_cfg);
        let zero = F128::zero_with_cfg(field_cfg);
        let mut result = zero.clone();
        for suffix in 0..products.az.evaluations.len() / block_len {
            let start = suffix * block_len;
            let a = inner_product(
                &weights,
                &products.az.evaluations[start..start + block_len],
                &zero,
            );
            let b = inner_product(
                &weights,
                &products.bz.evaluations[start..start + block_len],
                &zero,
            );
            let c = inner_product(
                &weights,
                &products.cz.evaluations[start..start + block_len],
                &zero,
            );
            result += &mul(
                &equality_weight(equality_factors, suffix),
                &sub(&mul(&a, &b), &c),
            );
        }
        result
    }

    #[test]
    fn static_layout_tables_match_the_requested_domains() {
        assert_eq!(K1_EXTERIOR_NODES, []);
        assert_eq!(K2_EXTERIOR_NODES, [-1, 4]);
        assert_eq!(K3_EXTERIOR_NODES, [-1, 8, -2, 9, -3, 10]);
        assert_eq!(K4_EXTERIOR_NODES[0..4], [-1, 16, -2, 17]);
        assert_eq!(K4_EXTERIOR_NODES[12..14], [-7, 22]);
        assert_eq!(K3_TOP_DIFFERENCE, [-1, 7, -21, 35, -35, 21, -7, 1]);
    }

    #[test]
    fn reconstruction_matches_direct_residual_at_base_exterior_and_random_points() {
        let field_cfg = config();
        for skip_vars in 1..=4 {
            let num_vars = skip_vars + 2;
            let products = valid_products(num_vars, &field_cfg);
            let tau_tail = [field(9, &field_cfg), field(13, &field_cfg)];
            let equality_factors = make_equality_factors(&tau_tail, &field_cfg).unwrap();
            let message = compute_field_skip_message(
                skip_vars,
                (&equality_factors.0, &equality_factors.1),
                &products,
                &field_cfg,
            )
            .unwrap();
            assert_eq!(message.len(), (1usize << skip_vars) - 1);
            let proof = UnivariateSkipProof::from_ordered_message(skip_vars, message).unwrap();

            let mut points = (0..1usize << skip_vars)
                .map(|point| field(point as u64, &field_cfg))
                .collect::<Vec<_>>();
            points.extend(
                exterior_nodes(skip_vars)
                    .iter()
                    .map(|point| field_from_signed(i128::from(*point), &field_cfg)),
            );
            points.push(field(37, &field_cfg));

            for point in points {
                let reconstructed = proof.reconstruct_at(&point, &field_cfg).unwrap();
                let direct = direct_q_at(
                    &products,
                    (&equality_factors.0, &equality_factors.1),
                    skip_vars,
                    &point,
                    &field_cfg,
                );
                assert_eq!(reconstructed, direct, "K={skip_vars}");
            }
        }
    }

    #[test]
    fn infinity_uses_only_the_leading_coefficients_of_a_and_b() {
        let field_cfg = config();
        for skip_vars in 1..=4 {
            let num_vars = skip_vars + 1;
            let products = valid_products(num_vars, &field_cfg);
            let tau_tail = [field(17, &field_cfg)];
            let equality_factors = make_equality_factors(&tau_tail, &field_cfg).unwrap();
            let message = compute_field_skip_message(
                skip_vars,
                (&equality_factors.0, &equality_factors.1),
                &products,
                &field_cfg,
            )
            .unwrap();
            let infinity = message.last().unwrap().clone();

            let mut changed_c = products.clone();
            for (index, value) in changed_c.cz.evaluations.iter_mut().enumerate() {
                *value += &field((index as u64 + 1) * 19, &field_cfg);
            }
            let changed_message = compute_field_skip_message(
                skip_vars,
                (&equality_factors.0, &equality_factors.1),
                &changed_c,
                &field_cfg,
            )
            .unwrap();
            assert_eq!(changed_message.last(), Some(&infinity), "K={skip_vars}");
        }
    }

    #[test]
    fn composed_outer_proof_reuses_the_cubic_tail_for_every_k() {
        let field_cfg = config();
        for skip_vars in 1..=4 {
            let num_vars = skip_vars + 1;
            let products = valid_products(num_vars, &field_cfg);
            let tau_tail = [field(23, &field_cfg)];
            let equality_factors = make_equality_factors(&tau_tail, &field_cfg).unwrap();
            let reducer = ImmediateSumcheckReducer::new(&field_cfg);
            let mut prover_transcript = Blake3Transcript::new();
            let mut verifier_transcript = prover_transcript.clone();
            let mut direct_tail_transcript = prover_transcript.clone();

            let direct_message = compute_field_skip_message(
                skip_vars,
                (&equality_factors.0, &equality_factors.1),
                &products,
                &field_cfg,
            )
            .unwrap();
            let direct_skip =
                UnivariateSkipProof::from_ordered_message(skip_vars, direct_message).unwrap();

            let output = prove_univariate_skip_outer_sumcheck_with_reducer(
                &mut prover_transcript,
                skip_vars,
                &tau_tail,
                equality_factors.clone(),
                products.clone(),
                &field_cfg,
                &reducer,
            )
            .unwrap();
            let direct_reduction = direct_skip
                .verify_reduction(&mut direct_tail_transcript, &field_cfg)
                .unwrap();
            let direct_folded =
                fold_field_prefix(products, skip_vars, &direct_reduction.z, &field_cfg).unwrap();
            let direct_tail = prove_outer_sumcheck_with_reducer(
                &mut direct_tail_transcript,
                direct_reduction.q_at_z,
                &tau_tail,
                equality_factors,
                direct_folded,
                &field_cfg,
                &reducer,
            )
            .unwrap();
            let verified = output
                .proof
                .verify(&mut verifier_transcript, &tau_tail, num_vars, &field_cfg)
                .unwrap();

            assert_eq!(output.proof.skip, direct_skip);
            assert_eq!(output.proof.tail, direct_tail.proof);
            assert_eq!(output.final_claim, direct_tail.final_claim);
            assert_eq!(output.row_binding.z, direct_reduction.z);
            assert_eq!(verified.row_binding, output.row_binding);
            assert_eq!(verified.az_mle_claim, output.proof.tail.az_mle_claim);
            assert_eq!(verified.bz_mle_claim, output.proof.tail.bz_mle_claim);
            assert_eq!(verified.cz_mle_claim, output.proof.tail.cz_mle_claim);
            assert_eq!(
                verified
                    .row_binding
                    .row_weights(num_vars, &field_cfg)
                    .unwrap()
                    .len(),
                1usize << num_vars
            );
            assert_eq!(
                prover_transcript.get_challenge::<u128>(),
                direct_tail_transcript.get_challenge::<u128>()
            );
        }
    }

    #[test]
    fn k_equal_to_the_row_dimension_has_a_zero_round_tail() {
        let field_cfg = config();
        let skip_vars = 3;
        let products = valid_products(skip_vars, &field_cfg);
        let equality_factors = make_equality_factors::<F128>(&[], &field_cfg).unwrap();
        let reducer = ImmediateSumcheckReducer::new(&field_cfg);
        let mut prover_transcript = Blake3Transcript::new();
        let mut verifier_transcript = prover_transcript.clone();

        let output = prove_univariate_skip_outer_sumcheck_with_reducer(
            &mut prover_transcript,
            skip_vars,
            &[],
            equality_factors,
            products,
            &field_cfg,
            &reducer,
        )
        .unwrap();
        assert!(output.proof.tail.sumcheck.round_polynomials.is_empty());
        output
            .proof
            .verify(&mut verifier_transcript, &[], skip_vars, &field_cfg)
            .unwrap();
    }

    #[test]
    fn outer_verifier_rejects_foreign_fields_before_transcript_mutation() {
        let field_cfg = config();
        let other_cfg = F128::make_cfg(&Uint::from((1_u128 << 127) - 1)).unwrap();
        let skip_vars = 2;
        let num_vars = 3;
        let products = valid_products(num_vars, &field_cfg);
        let tau_tail = [field(23, &field_cfg)];
        let equality_factors = make_equality_factors(&tau_tail, &field_cfg).unwrap();
        let reducer = ImmediateSumcheckReducer::new(&field_cfg);
        let output = prove_univariate_skip_outer_sumcheck_with_reducer(
            &mut Blake3Transcript::new(),
            skip_vars,
            &tau_tail,
            equality_factors,
            products,
            &field_cfg,
            &reducer,
        )
        .unwrap();

        let mut malformed = output.proof;
        malformed.skip.q_at_infinity = field(1, &other_cfg);
        let mut actual = Blake3Transcript::new();
        let mut untouched = actual.clone();
        assert_eq!(
            malformed.verify(&mut actual, &tau_tail, num_vars, &field_cfg),
            Err(SumcheckError::FieldConfigurationMismatch)
        );
        assert_eq!(
            actual.get_challenge::<u128>(),
            untouched.get_challenge::<u128>()
        );
    }

    fn exterior_nodes(skip_vars: usize) -> &'static [i32] {
        match skip_vars {
            1 => &K1_EXTERIOR_NODES,
            2 => &K2_EXTERIOR_NODES,
            3 => &K3_EXTERIOR_NODES,
            4 => &K4_EXTERIOR_NODES,
            _ => &[],
        }
    }
}
