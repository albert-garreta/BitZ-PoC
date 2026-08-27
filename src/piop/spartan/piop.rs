//! Composition of Spartan's outer and inner sumchecks.

use blake3::Hasher;
use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_monty::MontyField};
use thiserror::Error;

use crate::{poly::mle::DenseMultilinearExtension, transcript::traits::Transcript};

use super::{
    SpartanField, absorb_spartan_message,
    matrix::{
        MleClaimError, PreparedConstraintMatrices, ScaledMleEvaluationClaim,
        SpartanMatrixCoefficient, SpartanMatrixError, make_equality_factors,
    },
    squeeze_field,
    sumcheck::{
        CryptoBigintSumcheckReducer, ImmediateSumcheckReducer, InnerSumcheckOutput,
        OptimizedSumcheckReducer, OuterSumcheckProof, R1csProductMles, SumcheckError,
        SumcheckLinearReducer, SumcheckProductReducer, SumcheckProof,
        prove_inner_sumcheck_u32_native_with_reducer, prove_inner_sumcheck_with_reducer,
        prove_outer_sumcheck_u32_native_with_reducer, prove_outer_sumcheck_with_reducer,
    },
};

#[cfg(any(test, feature = "bench-internals"))]
use super::sumcheck::{
    FieldCoefficientPolicy, NativeWitnessFoldPolicy, prove_inner_sumcheck_u32_native_with_policy,
};

/// Domain separator for the native Spartan PIOP transcript.
///
/// Version two identifies the target-native transcript fork: it uses BLAKE3
/// statement digests, runtime field configurations, exact challenge sampling,
/// and an explicit assignment-oracle binding.
pub const SPARTAN_PIOP_DOMAIN: &[u8] = b"f2z/spartan/piop/v2";

/// Domain separator for the assignment-oracle commitment in the PIOP
/// statement.
pub const SPARTAN_ASSIGNMENT_ORACLE_DOMAIN: &[u8] = b"f2z/spartan/assignment-oracle/v1";

const NONSUCCINCT_ASSIGNMENT_DIGEST_DOMAIN: &[u8] = b"f2z/spartan/full-assignment-digest/v1";

/// The cubic outer and quadratic inner sumcheck proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpartanPiopProof<F> {
    pub outer: OuterSumcheckProof<F>,
    pub inner: SumcheckProof<F, 3>,
}

/// Arithmetic policy selected once before entering the Spartan prover.
///
/// Each arm dispatches to a separately monomorphized sumcheck implementation;
/// this enum is never inspected inside a product-accumulation loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpartanReductionStrategy {
    /// Original eager field multiplication and reduction.
    Immediate,
    /// Five-limb accumulation with fixed-schedule Barrett/Montgomery reduction.
    DelayedBarrett,
    /// Five-limb accumulation with a `crypto-bigint` reference remainder.
    DelayedCryptoBigint,
}

/// Native-witness fold policy exposed only for controlled benchmarks.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpartanInnerNativeFold {
    Immediate,
    Delayed,
}

/// Field coefficient-accumulation policy exposed only for controlled
/// benchmarks.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpartanInnerFieldAccumulation {
    Immediate,
    Delayed,
    DelayedAtOrAbovePairs(usize),
}

/// The two independent policy choices varied by the inner-sumcheck sweep.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpartanInnerPolicy {
    pub native_witness_fold: SpartanInnerNativeFold,
    pub field_coefficients: SpartanInnerFieldAccumulation,
}

/// Failures while composing or checking the Spartan PIOP.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SpartanError {
    #[error(transparent)]
    Matrix(#[from] SpartanMatrixError),

    #[error(transparent)]
    Sumcheck(#[from] SumcheckError),

    #[error(transparent)]
    MleClaim(#[from] MleClaimError),

    #[error("Az, Bz, and Cz do not use the prepared row domain")]
    InvalidProductDimensions,

    #[error("the assignment does not use the prepared column domain")]
    InvalidAssignmentDimensions,

    #[error("the assignment has nonzero values outside the logical column range")]
    InvalidAssignmentPadding,

    #[error("a dense multilinear-extension table has invalid shape")]
    InvalidMleShape,

    #[error("a proof or witness value uses a different field configuration")]
    FieldConfigurationMismatch,

    #[error("the supplied opening claim is not the claim derived from the proof")]
    InvalidMleClaim,
}

/// Runs the complete outer and inner Spartan reductions.
///
/// The caller supplies field-valued `A/B/C` products and the complete R1CS
/// assignment. Constraint generation and the eventual succinct opening of the
/// returned assignment-MLE claim are deliberately outside this native PIOP.
/// `assignment_oracle_binding` must canonically commit to that eventual
/// opening oracle (normally a PCS commitment) and is absorbed before any
/// Fiat--Shamir challenge. Use [`prove_spartan_nonsuccinct`] while no PCS is
/// connected; it binds a canonical digest of the complete assignment table.
pub fn prove_spartan_piop<F, C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<F>,
    assignment: DenseMultilinearExtension<F>,
) -> Result<(SpartanPiopProof<F>, ScaledMleEvaluationClaim<F>), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    let reducer = ImmediateSumcheckReducer::new(matrices.config());
    prove_spartan_piop_with_reducer(
        transcript,
        matrices,
        assignment_oracle_binding,
        products,
        assignment,
        &reducer,
    )
}

/// Runs the two-limb runtime-field prover with an explicitly selected
/// reduction strategy.
pub fn prove_spartan_piop_with_strategy<C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, C>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<MontyField<2>>,
    assignment: DenseMultilinearExtension<MontyField<2>>,
    strategy: SpartanReductionStrategy,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
>
where
    C: SpartanMatrixCoefficient<MontyField<2>>,
{
    match strategy {
        SpartanReductionStrategy::Immediate => {
            let reducer = ImmediateSumcheckReducer::new(matrices.config());
            prove_spartan_piop_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
        SpartanReductionStrategy::DelayedBarrett => {
            let reducer = OptimizedSumcheckReducer::new(matrices.config())?;
            prove_spartan_piop_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
        SpartanReductionStrategy::DelayedCryptoBigint => {
            let reducer = CryptoBigintSumcheckReducer::new(matrices.config())?;
            prove_spartan_piop_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
    }
}

/// Runs the production u32 multiplication prover while retaining its exact
/// `u64` products and assignment through the first round of each sumcheck.
/// Native coefficients, native witness folding, and every later field
/// coefficient sum use delayed Barrett reduction. Field-MLE folding remains
/// immediate.
pub fn prove_spartan_piop_u32_native(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
> {
    validate_native_u32_prover_inputs(matrices, &products, &assignment)?;
    let reducer = OptimizedSumcheckReducer::new(matrices.config())?;
    prove_spartan_piop_u32_native_with_reducer(
        transcript,
        matrices,
        assignment_oracle_binding,
        products,
        assignment,
        &reducer,
    )
}

/// Runs the u32 multiplication prover with an explicitly selected reduction
/// strategy. Prefer [`prove_spartan_piop_u32_native`] for production proving.
pub fn prove_spartan_piop_u32_native_with_strategy(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
    strategy: SpartanReductionStrategy,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
> {
    validate_native_u32_prover_inputs(matrices, &products, &assignment)?;

    match strategy {
        SpartanReductionStrategy::Immediate => {
            let products = project_native_products(products, matrices.config());
            let assignment = project_native_mle(assignment, matrices.config());
            let reducer = ImmediateSumcheckReducer::new(matrices.config());
            prove_spartan_piop_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
        SpartanReductionStrategy::DelayedBarrett => {
            let reducer = OptimizedSumcheckReducer::new(matrices.config())?;
            prove_spartan_piop_u32_native_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
        SpartanReductionStrategy::DelayedCryptoBigint => {
            let reducer = CryptoBigintSumcheckReducer::new(matrices.config())?;
            prove_spartan_piop_u32_native_with_reducer(
                transcript,
                matrices,
                assignment_oracle_binding,
                products,
                assignment,
                &reducer,
            )
        }
    }
}

/// Runs the native-u64 Spartan prover with a fixed delayed-Barrett outer
/// sumcheck and a benchmark-selected inner policy.
///
/// This entry point is intentionally benchmark-only: it holds the outer
/// arithmetic and witness representation constant so inner policies can be
/// compared without changing the statement or transcript.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn prove_spartan_piop_u32_native_barrett_with_inner_policy(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
    policy: SpartanInnerPolicy,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
> {
    validate_native_u32_prover_inputs(matrices, &products, &assignment)?;
    let delayed = OptimizedSumcheckReducer::new(matrices.config())?;
    let immediate = ImmediateSumcheckReducer::new(matrices.config());
    let native_fold_policy = match policy.native_witness_fold {
        SpartanInnerNativeFold::Immediate => NativeWitnessFoldPolicy::Immediate,
        SpartanInnerNativeFold::Delayed => NativeWitnessFoldPolicy::Delayed,
    };
    let field_coefficient_policy = match policy.field_coefficients {
        SpartanInnerFieldAccumulation::Immediate => FieldCoefficientPolicy::Immediate,
        SpartanInnerFieldAccumulation::Delayed => FieldCoefficientPolicy::Delayed,
        SpartanInnerFieldAccumulation::DelayedAtOrAbovePairs(minimum_pairs) => {
            FieldCoefficientPolicy::DelayedAtOrAbovePairs(minimum_pairs)
        }
    };

    prove_spartan_piop_u32_native_with_inner_policy(
        transcript,
        matrices,
        assignment_oracle_binding,
        products,
        assignment,
        &delayed,
        &delayed,
        &delayed,
        &immediate,
        native_fold_policy,
        field_coefficient_policy,
    )
}

fn prove_spartan_piop_u32_native_with_reducer<R>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
    reducer: &R,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
>
where
    R: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
{
    prove_spartan_piop_u32_native_with_inner(
        transcript,
        matrices,
        assignment_oracle_binding,
        products,
        assignment,
        reducer,
        |transcript, initial_claim, batched_matrix, assignment, field_config| {
            prove_inner_sumcheck_u32_native_with_reducer(
                transcript,
                initial_claim,
                batched_matrix,
                assignment,
                field_config,
                reducer,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "bench-internals"))]
fn prove_spartan_piop_u32_native_with_inner_policy<OR, NR, DR, IR>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
    outer_reducer: &OR,
    native_inner_reducer: &NR,
    delayed_inner_reducer: &DR,
    immediate_inner_reducer: &IR,
    native_fold_policy: NativeWitnessFoldPolicy,
    field_coefficient_policy: FieldCoefficientPolicy,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
>
where
    OR: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
    NR: SumcheckLinearReducer,
    DR: SumcheckProductReducer<MontyField<2>>,
    IR: SumcheckProductReducer<MontyField<2>>,
{
    prove_spartan_piop_u32_native_with_inner(
        transcript,
        matrices,
        assignment_oracle_binding,
        products,
        assignment,
        outer_reducer,
        |transcript, initial_claim, batched_matrix, assignment, field_config| {
            prove_inner_sumcheck_u32_native_with_policy(
                transcript,
                initial_claim,
                batched_matrix,
                assignment,
                field_config,
                native_inner_reducer,
                delayed_inner_reducer,
                immediate_inner_reducer,
                native_fold_policy,
                field_coefficient_policy,
            )
        },
    )
}

fn prove_spartan_piop_u32_native_with_inner<T, OR, P>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<u64>,
    assignment: DenseMultilinearExtension<u64>,
    outer_reducer: &OR,
    prove_inner: P,
) -> Result<
    (
        SpartanPiopProof<MontyField<2>>,
        ScaledMleEvaluationClaim<MontyField<2>>,
    ),
    SpartanError,
>
where
    T: Transcript,
    OR: SumcheckProductReducer<MontyField<2>> + SumcheckLinearReducer,
    P: FnOnce(
        &mut T,
        MontyField<2>,
        DenseMultilinearExtension<MontyField<2>>,
        DenseMultilinearExtension<u64>,
        &crypto_bigint::modular::MontyParams<2>,
    ) -> Result<InnerSumcheckOutput<MontyField<2>>, SumcheckError>,
{
    absorb_statement(transcript, matrices, assignment_oracle_binding);

    let field_config = matrices.config();
    let tau = (0..matrices.num_row_vars())
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<MontyField<2>>>();
    let equality_factors = make_equality_factors(&tau, field_config)?;
    let outer = {
        let _scope = crate::utils::prof::scope("spartan:outer_sumcheck");
        prove_outer_sumcheck_u32_native_with_reducer(
            transcript,
            MontyField::<2>::zero_with_cfg(field_config),
            equality_factors,
            products,
            field_config,
            outer_reducer,
        )?
    };

    let rho = squeeze_field(transcript, field_config);
    let inner_initial_claim = batched_product_claim(
        &outer.proof.az_mle_claim,
        &outer.proof.bz_mle_claim,
        &outer.proof.cz_mle_claim,
        &rho,
    );
    let batched_matrix = {
        let _scope = crate::utils::prof::scope("spartan:bind_and_batch");
        matrices.bind_and_batch(&outer.eval_points, &rho)?
    };
    let inner = {
        let _scope = crate::utils::prof::scope("spartan:inner_sumcheck");
        prove_inner(
            transcript,
            inner_initial_claim,
            batched_matrix,
            assignment,
            field_config,
        )?
    };

    let claim = ScaledMleEvaluationClaim::new(
        inner.sumcheck.eval_points.into_boxed_slice(),
        inner.batched_matrix_evaluation,
        inner.sumcheck.final_claim,
    );
    let proof = SpartanPiopProof {
        outer: outer.proof,
        inner: inner.sumcheck.proof,
    };
    Ok((proof, claim))
}

fn prove_spartan_piop_with_reducer<F, C, R>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<F>,
    assignment: DenseMultilinearExtension<F>,
    reducer: &R,
) -> Result<(SpartanPiopProof<F>, ScaledMleEvaluationClaim<F>), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
    R: SumcheckProductReducer<F>,
{
    validate_prover_inputs(matrices, &products, &assignment)?;
    absorb_statement(transcript, matrices, assignment_oracle_binding);

    let field_config = matrices.config();
    let tau = (0..matrices.num_row_vars())
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<F>>();
    let equality_factors = make_equality_factors(&tau, field_config)?;
    let outer = {
        let _scope = crate::utils::prof::scope("spartan:outer_sumcheck");
        prove_outer_sumcheck_with_reducer(
            transcript,
            F::zero_with_cfg(field_config),
            equality_factors,
            products,
            field_config,
            reducer,
        )?
    };

    // The outer prover absorbed [Az(r_x), Bz(r_x), Cz(r_x)] before returning.
    let rho = squeeze_field(transcript, field_config);
    let inner_initial_claim = batched_product_claim(
        &outer.proof.az_mle_claim,
        &outer.proof.bz_mle_claim,
        &outer.proof.cz_mle_claim,
        &rho,
    );
    let batched_matrix = {
        let _scope = crate::utils::prof::scope("spartan:bind_and_batch");
        matrices.bind_and_batch(&outer.eval_points, &rho)?
    };
    let inner = {
        let _scope = crate::utils::prof::scope("spartan:inner_sumcheck");
        prove_inner_sumcheck_with_reducer(
            transcript,
            inner_initial_claim,
            batched_matrix,
            assignment,
            field_config,
            reducer,
        )?
    };

    let claim = ScaledMleEvaluationClaim::new(
        inner.sumcheck.eval_points.into_boxed_slice(),
        inner.batched_matrix_evaluation,
        inner.sumcheck.final_claim,
    );
    let proof = SpartanPiopProof {
        outer: outer.proof,
        inner: inner.sumcheck.proof,
    };

    Ok((proof, claim))
}

/// Verifies both sumchecks and returns the terminal scaled assignment claim
/// `D(r_y) * h(r_y) = final_claim`.
pub fn verify_spartan_proof<F, C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment_oracle_binding: &[u8; 32],
    proof: &SpartanPiopProof<F>,
) -> Result<ScaledMleEvaluationClaim<F>, SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    validate_proof(matrices, proof)?;
    absorb_statement(transcript, matrices, assignment_oracle_binding);

    let field_config = matrices.config();
    let tau = (0..matrices.num_row_vars())
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<F>>();
    let outer = proof.outer.verify(
        transcript,
        F::zero_with_cfg(field_config),
        &tau,
        field_config,
    )?;

    // The outer verifier absorbed [Az(r_x), Bz(r_x), Cz(r_x)] before returning.
    let rho = squeeze_field(transcript, field_config);
    let inner_initial_claim = batched_product_claim(
        &outer.az_mle_claim,
        &outer.bz_mle_claim,
        &outer.cz_mle_claim,
        &rho,
    );
    let (column_point, final_claim) = proof.inner.verify(
        transcript,
        inner_initial_claim,
        matrices.num_column_vars(),
        field_config,
    )?;
    let matrix_evaluation = matrices.evaluate_batched(&outer.eval_points, &rho, &column_point)?;

    Ok(ScaledMleEvaluationClaim::new(
        column_point.into_boxed_slice(),
        matrix_evaluation,
        final_claim,
    ))
}

/// Proves the PIOP while binding the complete assignment table itself.
///
/// This is the sound native path until a hiding PCS commitment is available.
/// The digest is binding, not hiding; callers who need witness privacy should
/// use [`prove_spartan_piop`] with a canonical 32-byte binding of the PCS
/// commitment.
pub fn prove_spartan_nonsuccinct<F, C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    products: R1csProductMles<F>,
    assignment: DenseMultilinearExtension<F>,
) -> Result<(SpartanPiopProof<F>, ScaledMleEvaluationClaim<F>), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    validate_prover_inputs(matrices, &products, &assignment)?;
    let assignment_binding = nonsuccinct_assignment_digest(matrices, &assignment)?;
    prove_spartan_piop(
        transcript,
        matrices,
        &assignment_binding,
        products,
        assignment,
    )
}

/// Verifies the PIOP, checks that `mle_claim` is the transcript-derived claim,
/// and discharges it against a complete assignment table.
///
/// This is intentionally nonsuccinct. It is the native integration seam to be
/// replaced by the F2Z PCS opening protocol later.
pub fn verify_spartan_with_mle_claim<F, C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    proof: &SpartanPiopProof<F>,
    mle_claim: &ScaledMleEvaluationClaim<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    validate_assignment(matrices, assignment)?;
    let assignment_binding = nonsuccinct_assignment_digest(matrices, assignment)?;
    let expected_claim = verify_spartan_proof(transcript, matrices, &assignment_binding, proof)?;
    if mle_claim != &expected_claim {
        return Err(SpartanError::InvalidMleClaim);
    }
    mle_claim.nonsuccinct_verify(assignment, matrices.config())?;
    Ok(())
}

fn absorb_statement<F, C>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment_oracle_binding: &[u8; 32],
) where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    absorb_spartan_message(transcript, b"protocol", SPARTAN_PIOP_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"field-modulus",
        matrices.field_modulus_encoding(),
    );
    absorb_spartan_message(transcript, b"matrix-statement", matrices.digest());
    absorb_spartan_message(
        transcript,
        SPARTAN_ASSIGNMENT_ORACLE_DOMAIN,
        assignment_oracle_binding,
    );
}

fn validate_prover_inputs<F, C>(
    matrices: &PreparedConstraintMatrices<F, C>,
    products: &R1csProductMles<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    let row_vars = matrices.num_row_vars();
    if products.az.num_vars != row_vars
        || products.bz.num_vars != row_vars
        || products.cz.num_vars != row_vars
    {
        return Err(SpartanError::InvalidProductDimensions);
    }

    for mle in [&products.az, &products.bz, &products.cz] {
        validate_mle_shape(mle)?;
        validate_elements_field(&mle.evaluations, matrices)?;
    }
    validate_assignment(matrices, assignment)
}

fn validate_native_u32_prover_inputs(
    matrices: &PreparedConstraintMatrices<MontyField<2>, bool>,
    products: &R1csProductMles<u64>,
    assignment: &DenseMultilinearExtension<u64>,
) -> Result<(), SpartanError> {
    let row_vars = matrices.num_row_vars();
    if products.az.num_vars != row_vars
        || products.bz.num_vars != row_vars
        || products.cz.num_vars != row_vars
    {
        return Err(SpartanError::InvalidProductDimensions);
    }
    for mle in [&products.az, &products.bz, &products.cz] {
        validate_mle_shape(mle)?;
    }
    if products
        .az
        .evaluations
        .iter()
        .chain(&products.bz.evaluations)
        .any(|&value| value > u64::from(u32::MAX))
    {
        return Err(SumcheckError::NativeMultiplicandOutOfRange.into());
    }

    if assignment.num_vars != matrices.num_column_vars() {
        return Err(SpartanError::InvalidAssignmentDimensions);
    }
    validate_mle_shape(assignment)?;
    if assignment.evaluations.first() != Some(&1) {
        return Err(SpartanMatrixError::InvalidAssignmentConstant.into());
    }
    if assignment.evaluations[matrices.matrices().column_count()..]
        .iter()
        .any(|&value| value != 0)
    {
        return Err(SpartanError::InvalidAssignmentPadding);
    }
    Ok(())
}

fn project_native_mle(
    mle: DenseMultilinearExtension<u64>,
    field_config: &crypto_bigint::modular::MontyParams<2>,
) -> DenseMultilinearExtension<MontyField<2>> {
    DenseMultilinearExtension {
        evaluations: mle
            .evaluations
            .into_iter()
            .map(|value| MontyField::<2>::from_with_cfg(value, field_config))
            .collect(),
        num_vars: mle.num_vars,
    }
}

fn project_native_products(
    products: R1csProductMles<u64>,
    field_config: &crypto_bigint::modular::MontyParams<2>,
) -> R1csProductMles<MontyField<2>> {
    R1csProductMles {
        az: project_native_mle(products.az, field_config),
        bz: project_native_mle(products.bz, field_config),
        cz: project_native_mle(products.cz, field_config),
    }
}

fn validate_assignment<F, C>(
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    if assignment.num_vars != matrices.num_column_vars() {
        return Err(SpartanError::InvalidAssignmentDimensions);
    }
    validate_mle_shape(assignment)?;
    validate_elements_field(&assignment.evaluations, matrices)?;

    let one = F::one_with_cfg(matrices.config());
    if assignment.evaluations.first() != Some(&one) {
        return Err(SpartanMatrixError::InvalidAssignmentConstant.into());
    }
    let zero = F::zero_with_cfg(matrices.config());
    if assignment.evaluations[matrices.matrices().column_count()..]
        .iter()
        .any(|value| value != &zero)
    {
        return Err(SpartanError::InvalidAssignmentPadding);
    }
    Ok(())
}

fn nonsuccinct_assignment_digest<F, C>(
    matrices: &PreparedConstraintMatrices<F, C>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<[u8; 32], SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    // Keep this helper independently defensive because it is the binding used
    // by both prover and verifier before the first challenge.
    validate_assignment(matrices, assignment)?;

    let mut hasher = Hasher::new();
    hasher.update(NONSUCCINCT_ASSIGNMENT_DIGEST_DOMAIN);
    hash_binding_bytes(&mut hasher, matrices.field_modulus_encoding())?;
    hash_binding_usize(&mut hasher, matrices.matrices().column_count())?;
    hash_binding_usize(&mut hasher, assignment.num_vars)?;
    hash_binding_usize(&mut hasher, assignment.evaluations.len())?;
    for value in &assignment.evaluations {
        hash_binding_bytes(&mut hasher, &value.canonical_element_encoding())?;
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_binding_bytes(hasher: &mut Hasher, bytes: &[u8]) -> Result<(), SpartanError> {
    hash_binding_usize(hasher, bytes.len())?;
    hasher.update(bytes);
    Ok(())
}

fn hash_binding_usize(hasher: &mut Hasher, value: usize) -> Result<(), SpartanError> {
    let value = u64::try_from(value).map_err(|_| SpartanMatrixError::DomainTooLarge)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn validate_proof<F, C>(
    matrices: &PreparedConstraintMatrices<F, C>,
    proof: &SpartanPiopProof<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    if proof.outer.sumcheck.round_polynomials.len() != matrices.num_row_vars() {
        return Err(SumcheckError::InvalidRoundCount {
            expected: matrices.num_row_vars(),
            actual: proof.outer.sumcheck.round_polynomials.len(),
        }
        .into());
    }
    if proof.inner.round_polynomials.len() != matrices.num_column_vars() {
        return Err(SumcheckError::InvalidRoundCount {
            expected: matrices.num_column_vars(),
            actual: proof.inner.round_polynomials.len(),
        }
        .into());
    }

    for round in &proof.outer.sumcheck.round_polynomials {
        validate_elements_field(round, matrices)?;
    }
    validate_elements_field(
        &[
            proof.outer.az_mle_claim.clone(),
            proof.outer.bz_mle_claim.clone(),
            proof.outer.cz_mle_claim.clone(),
        ],
        matrices,
    )?;
    for round in &proof.inner.round_polynomials {
        validate_elements_field(round, matrices)?;
    }
    Ok(())
}

fn validate_mle_shape<F>(mle: &DenseMultilinearExtension<F>) -> Result<(), SpartanError> {
    let expected = 1usize
        .checked_shl(u32::try_from(mle.num_vars).map_err(|_| SpartanError::InvalidMleShape)?)
        .ok_or(SpartanError::InvalidMleShape)?;
    if mle.evaluations.len() != expected {
        return Err(SpartanError::InvalidMleShape);
    }
    Ok(())
}

fn validate_elements_field<F, C>(
    values: &[F],
    matrices: &PreparedConstraintMatrices<F, C>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    for value in values {
        if F::canonical_modulus_encoding(value.cfg()) != matrices.field_modulus_encoding() {
            return Err(SpartanError::FieldConfigurationMismatch);
        }
        value.validate_element().map_err(SpartanMatrixError::from)?;
    }
    Ok(())
}

fn batched_product_claim<F>(az: &F, bz: &F, cz: &F, rho: &F) -> F
where
    F: SpartanField,
{
    let rho_squared = rho.clone() * rho;
    let mut claim = az.clone();
    claim += &(rho.clone() * bz);
    claim += &(rho_squared * cz);
    claim
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        FromWithConfig, PrimeField,
        crypto_bigint_monty::{F128, F192},
        crypto_bigint_uint::Uint,
    };

    use crate::transcript::{Blake3Transcript, traits::Transcript};

    use super::*;
    use crate::piop::spartan::matrix::{
        ConstraintMatrices, SparseMatrix, build_assignment_mle, build_product_mles,
    };
    use crate::piop::spartan::u32_mul::{
        U32MulWitness, prepare_u32_mul_relation, project_u32_mul_native_witness,
    };

    const Q100: u128 = (1_u128 << 100) - 15;
    const ROWS: usize = 5;
    const COLUMNS: usize = 7;

    fn config(modulus: u128) -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(modulus)).expect("odd prime test modulus")
    }

    fn field(value: u128, config: &<F128 as PrimeField>::Config) -> F128 {
        F128::from_with_cfg(value, config)
    }

    fn multiply(
        matrix: &SparseMatrix<F128>,
        assignment: &[F128],
        config: &<F128 as PrimeField>::Config,
    ) -> Vec<F128> {
        assert_eq!(assignment.len(), matrix.column_count());
        let mut products = vec![F128::zero_with_cfg(config); matrix.row_count()];
        for (column, entries) in matrix.columns().enumerate() {
            for (row, coefficient) in entries {
                products[*row] += &(coefficient.clone() * &assignment[column]);
            }
        }
        products
    }

    fn fixture(
        config: &<F128 as PrimeField>::Config,
    ) -> (
        PreparedConstraintMatrices<F128>,
        R1csProductMles<F128>,
        DenseMultilinearExtension<F128>,
    ) {
        let assignment_values: Vec<_> = (0..COLUMNS)
            .map(|column| field(if column == 0 { 1 } else { (column + 1) as u128 }, config))
            .collect();
        let a = SparseMatrix::try_from_rows(
            COLUMNS,
            (0..ROWS)
                .map(|row| {
                    vec![
                        (0, field((row + 2) as u128, config)),
                        (row + 1, field((2 * row + 3) as u128, config)),
                    ]
                })
                .collect(),
        )
        .unwrap();
        let b = SparseMatrix::try_from_rows(
            COLUMNS,
            (0..ROWS)
                .map(|row| {
                    vec![
                        (0, field((row + 5) as u128, config)),
                        (row + 2, field((3 * row + 7) as u128, config)),
                    ]
                })
                .collect(),
        )
        .unwrap();
        let az = multiply(&a, &assignment_values, config);
        let bz = multiply(&b, &assignment_values, config);
        let c = SparseMatrix::try_from_rows(
            COLUMNS,
            az.iter()
                .zip(&bz)
                .map(|(az, bz)| vec![(0, az.clone() * bz)])
                .collect(),
        )
        .unwrap();
        let cz = multiply(&c, &assignment_values, config);
        assert!(
            az.iter()
                .zip(&bz)
                .zip(&cz)
                .all(|((az, bz), cz)| az.clone() * bz == *cz)
        );

        let prepared =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), config)
                .unwrap();
        let products = build_product_mles(&az, &bz, &cz, ROWS, config).unwrap();
        let assignment = build_assignment_mle(&assignment_values, COLUMNS, config).unwrap();
        (prepared, products, assignment)
    }

    fn round_trip(modulus: u128) {
        let config = config(modulus);
        let (matrices, products, assignment) = fixture(&config);
        let assignment_binding = nonsuccinct_assignment_digest(&matrices, &assignment).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();
        assert_eq!(proof.outer.sumcheck.round_polynomials.len(), 3);
        assert_eq!(proof.inner.round_polynomials.len(), 3);

        let mut verifier_transcript = Blake3Transcript::new();
        let expected_claim = verify_spartan_proof(
            &mut verifier_transcript,
            &matrices,
            &assignment_binding,
            &proof,
        )
        .unwrap();
        assert_eq!(claim, expected_claim);
        claim.nonsuccinct_verify(&assignment, &config).unwrap();

        // Equal continuation proves that every challenge and absorption stayed
        // in lockstep across the prover and verifier implementations.
        let prover_continuation: u128 = prover_transcript.get_challenge();
        let verifier_continuation: u128 = verifier_transcript.get_challenge();
        assert_eq!(prover_continuation, verifier_continuation);

        let mut complete_transcript = Blake3Transcript::new();
        verify_spartan_with_mle_claim(
            &mut complete_transcript,
            &matrices,
            &proof,
            &claim,
            &assignment,
        )
        .unwrap();
    }

    fn reduction_strategies_match(modulus: u128) {
        let config = config(modulus);
        let (matrices, products, assignment) = fixture(&config);
        let assignment_binding = nonsuccinct_assignment_digest(&matrices, &assignment).unwrap();
        let mut reference = None;

        for strategy in [
            SpartanReductionStrategy::Immediate,
            SpartanReductionStrategy::DelayedBarrett,
            SpartanReductionStrategy::DelayedCryptoBigint,
        ] {
            let mut transcript = Blake3Transcript::new();
            let (proof, claim) = prove_spartan_piop_with_strategy(
                &mut transcript,
                &matrices,
                &assignment_binding,
                products.clone(),
                assignment.clone(),
                strategy,
            )
            .unwrap();
            let continuation: u128 = transcript.get_challenge();

            if let Some((reference_proof, reference_claim, reference_continuation)) = &reference {
                assert_eq!(&proof, reference_proof);
                assert_eq!(&claim, reference_claim);
                assert_eq!(&continuation, reference_continuation);
            } else {
                reference = Some((proof.clone(), claim.clone(), continuation));
            }

            let mut verifier_transcript = Blake3Transcript::new();
            let verified = verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &proof,
            )
            .unwrap();
            assert_eq!(claim, verified);
        }
    }

    fn native_u32_reduction_strategies_match(modulus: u128) {
        let config = config(modulus);
        let inputs = [
            (0, u32::MAX),
            (1, 1),
            (u32::MAX, u32::MAX),
            (0x8000_0000, 2),
            (17, 19),
        ];
        let witness = U32MulWitness::from_inputs(&inputs).unwrap();
        let matrices = prepare_u32_mul_relation(*witness.layout(), &config).unwrap();
        let native = project_u32_mul_native_witness(&witness);
        let (assignment, products) = native.into_parts();
        let assignment_binding = [0xA5; 32];
        let mut reference = None;

        for strategy in [
            SpartanReductionStrategy::Immediate,
            SpartanReductionStrategy::DelayedBarrett,
            SpartanReductionStrategy::DelayedCryptoBigint,
        ] {
            let mut transcript = Blake3Transcript::new();
            let (proof, claim) = prove_spartan_piop_u32_native_with_strategy(
                &mut transcript,
                &matrices,
                &assignment_binding,
                products.clone(),
                assignment.clone(),
                strategy,
            )
            .unwrap();
            let continuation: u128 = transcript.get_challenge();

            if let Some((reference_proof, reference_claim, reference_continuation)) = &reference {
                assert_eq!(&proof, reference_proof);
                assert_eq!(&claim, reference_claim);
                assert_eq!(&continuation, reference_continuation);
            } else {
                reference = Some((proof.clone(), claim.clone(), continuation));
            }

            let mut verifier_transcript = Blake3Transcript::new();
            let verified = verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &proof,
            )
            .unwrap();
            assert_eq!(claim, verified);
        }
    }

    fn native_u32_inner_policies_match(modulus: u128) {
        let config = config(modulus);
        let inputs = [
            (0, u32::MAX),
            (1, 1),
            (u32::MAX, u32::MAX),
            (0x8000_0000, 2),
            (17, 19),
        ];
        let witness = U32MulWitness::from_inputs(&inputs).unwrap();
        let matrices = prepare_u32_mul_relation(*witness.layout(), &config).unwrap();
        let (assignment, products) = project_u32_mul_native_witness(&witness).into_parts();
        let assignment_binding = [0x6D; 32];
        let delayed = OptimizedSumcheckReducer::new(&config).unwrap();
        let immediate = ImmediateSumcheckReducer::new(&config);
        let native_policies = [
            NativeWitnessFoldPolicy::Immediate,
            NativeWitnessFoldPolicy::Delayed,
        ];
        let field_policies = [
            FieldCoefficientPolicy::Immediate,
            FieldCoefficientPolicy::Delayed,
            FieldCoefficientPolicy::DelayedAtOrAbovePairs(0),
            FieldCoefficientPolicy::DelayedAtOrAbovePairs(4_096),
            FieldCoefficientPolicy::DelayedAtOrAbovePairs(usize::MAX),
        ];
        let mut production_transcript = Blake3Transcript::new();
        let (production_proof, production_claim) = prove_spartan_piop_u32_native(
            &mut production_transcript,
            &matrices,
            &assignment_binding,
            products.clone(),
            assignment.clone(),
        )
        .unwrap();
        let production_continuation: u128 = production_transcript.get_challenge();
        let reference = (
            production_proof.clone(),
            production_claim.clone(),
            production_continuation,
        );

        let mut verifier_transcript = Blake3Transcript::new();
        let verified = verify_spartan_proof(
            &mut verifier_transcript,
            &matrices,
            &assignment_binding,
            &production_proof,
        )
        .unwrap();
        assert_eq!(production_claim, verified);

        for native_policy in native_policies {
            for field_policy in field_policies {
                let mut transcript = Blake3Transcript::new();
                let (proof, claim) = prove_spartan_piop_u32_native_with_inner_policy(
                    &mut transcript,
                    &matrices,
                    &assignment_binding,
                    products.clone(),
                    assignment.clone(),
                    &delayed,
                    &delayed,
                    &delayed,
                    &immediate,
                    native_policy,
                    field_policy,
                )
                .unwrap();
                let continuation: u128 = transcript.get_challenge();

                assert_eq!(proof, reference.0);
                assert_eq!(claim, reference.1);
                assert_eq!(continuation, reference.2);

                let mut verifier_transcript = Blake3Transcript::new();
                let verified = verify_spartan_proof(
                    &mut verifier_transcript,
                    &matrices,
                    &assignment_binding,
                    &proof,
                )
                .unwrap();
                assert_eq!(claim, verified);
            }
        }
    }

    #[test]
    fn piop_is_generic_across_runtime_prime_configurations() {
        round_trip(Q100);
        round_trip((1_u128 << 127) - 1);
    }

    #[test]
    fn delayed_reduction_is_proof_and_transcript_exact() {
        reduction_strategies_match(Q100);
        reduction_strategies_match((1_u128 << 127) - 1);
    }

    #[test]
    fn native_u32_first_round_is_proof_and_transcript_exact_at_boundaries() {
        native_u32_reduction_strategies_match(Q100);
        native_u32_reduction_strategies_match((1_u128 << 127) - 1);
    }

    #[test]
    fn native_u32_inner_policies_are_proof_and_transcript_exact() {
        native_u32_inner_policies_match(Q100);
        native_u32_inner_policies_match((1_u128 << 127) - 1);
    }

    #[test]
    fn native_u32_zero_variable_outer_sumcheck_is_exact() {
        let config = config(Q100);
        let witness = U32MulWitness::from_inputs(&[(u32::MAX, u32::MAX)]).unwrap();
        let matrices = prepare_u32_mul_relation(*witness.layout(), &config).unwrap();
        let (assignment, products) = project_u32_mul_native_witness(&witness).into_parts();
        let assignment_binding = [0x3C; 32];
        let mut reference = None;

        for strategy in [
            SpartanReductionStrategy::Immediate,
            SpartanReductionStrategy::DelayedBarrett,
            SpartanReductionStrategy::DelayedCryptoBigint,
        ] {
            let mut transcript = Blake3Transcript::new();
            let (proof, claim) = prove_spartan_piop_u32_native_with_strategy(
                &mut transcript,
                &matrices,
                &assignment_binding,
                products.clone(),
                assignment.clone(),
                strategy,
            )
            .unwrap();
            assert!(proof.outer.sumcheck.round_polynomials.is_empty());
            let continuation = transcript.get_challenge::<u128>();

            if let Some((reference_proof, reference_claim, reference_continuation)) = &reference {
                assert_eq!(&proof, reference_proof);
                assert_eq!(&claim, reference_claim);
                assert_eq!(&continuation, reference_continuation);
            } else {
                reference = Some((proof.clone(), claim.clone(), continuation));
            }

            let mut verifier_transcript = Blake3Transcript::new();
            let verified = verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &proof,
            )
            .unwrap();
            assert_eq!(claim, verified);
        }
    }

    #[test]
    fn native_u32_first_round_rejects_wide_multiplicands_before_absorption() {
        let config = config(Q100);
        let witness = U32MulWitness::from_inputs(&[(2, 3)]).unwrap();
        let matrices = prepare_u32_mul_relation(*witness.layout(), &config).unwrap();
        let native = project_u32_mul_native_witness(&witness);
        let (assignment, mut products) = native.into_parts();
        products.az.evaluations[0] = u64::from(u32::MAX) + 1;
        let mut rejected_transcript = Blake3Transcript::new();

        assert_eq!(
            prove_spartan_piop_u32_native_with_strategy(
                &mut rejected_transcript,
                &matrices,
                &[0x5A; 32],
                products,
                assignment,
                SpartanReductionStrategy::DelayedBarrett,
            ),
            Err(SpartanError::Sumcheck(
                SumcheckError::NativeMultiplicandOutOfRange
            ))
        );

        let mut fresh_transcript = Blake3Transcript::new();
        assert_eq!(
            rejected_transcript.get_challenge::<u128>(),
            fresh_transcript.get_challenge::<u128>()
        );
    }

    #[test]
    fn piop_is_generic_across_montgomery_limb_widths_and_zero_variable_domains() {
        let modulus = <F192 as crypto_primitives::Field>::Modulus::from(Q100);
        let config = F192::make_cfg(&modulus).unwrap();
        let one = F192::one_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let b = a.clone();
        let c = a.clone();
        let matrices =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();
        let products = build_product_mles(
            std::slice::from_ref(&one),
            std::slice::from_ref(&one),
            std::slice::from_ref(&one),
            1,
            &config,
        )
        .unwrap();
        let assignment = build_assignment_mle(std::slice::from_ref(&one), 1, &config).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();
        assert!(proof.outer.sumcheck.round_polynomials.is_empty());
        assert!(proof.inner.round_polynomials.is_empty());

        let mut verifier_transcript = Blake3Transcript::new();
        verify_spartan_with_mle_claim(
            &mut verifier_transcript,
            &matrices,
            &proof,
            &claim,
            &assignment,
        )
        .unwrap();
    }

    #[test]
    fn tampered_round_and_assignment_are_rejected() {
        let config = config(Q100);
        let (matrices, products, assignment) = fixture(&config);
        let assignment_binding = nonsuccinct_assignment_digest(&matrices, &assignment).unwrap();
        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();

        let mut tampered_proof = proof.clone();
        tampered_proof.outer.sumcheck.round_polynomials[0][0] += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &tampered_proof,
            )
            .is_err()
        );

        let mut tampered_terminal = proof.clone();
        tampered_terminal.outer.az_mle_claim += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &tampered_terminal,
            )
            .is_err()
        );

        let foreign_config = self::config((1_u128 << 127) - 1);
        let mut foreign_proof = proof.clone();
        foreign_proof.outer.sumcheck.round_polynomials[0][0] = field(1, &foreign_config);
        let mut rejected_transcript = Blake3Transcript::new();
        assert_eq!(
            verify_spartan_proof(
                &mut rejected_transcript,
                &matrices,
                &assignment_binding,
                &foreign_proof,
            ),
            Err(SpartanError::FieldConfigurationMismatch)
        );
        let mut untouched_transcript = Blake3Transcript::new();
        assert_eq!(
            rejected_transcript.get_challenge::<u128>(),
            untouched_transcript.get_challenge::<u128>(),
            "foreign-config proofs must fail before transcript mutation"
        );

        let mut tampered_assignment = assignment;
        tampered_assignment.evaluations[1] += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_with_mle_claim(
                &mut verifier_transcript,
                &matrices,
                &proof,
                &claim,
                &tampered_assignment,
            )
            .is_err()
        );
    }

    #[test]
    fn nonsuccinct_verifier_rejects_the_zero_assignment_forgery() {
        let config = config(Q100);
        let one = F128::one_with_cfg(&config);
        let zero = F128::zero_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let b = a.clone();
        let c = SparseMatrix::try_from_rows(1, vec![Vec::new()]).unwrap();
        let matrices =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();

        // Without validating and pre-challenge binding of the assignment,
        // these all-zero terminal claims would discharge against h = [0].
        let forged_proof = SpartanPiopProof {
            outer: OuterSumcheckProof {
                sumcheck: SumcheckProof {
                    round_polynomials: Vec::new(),
                },
                az_mle_claim: zero.clone(),
                bz_mle_claim: zero.clone(),
                cz_mle_claim: zero.clone(),
            },
            inner: SumcheckProof {
                round_polynomials: Vec::new(),
            },
        };
        let forged_claim = ScaledMleEvaluationClaim::new(
            Vec::new().into_boxed_slice(),
            zero.clone(),
            zero.clone(),
        );
        let forged_assignment = DenseMultilinearExtension {
            evaluations: vec![zero],
            num_vars: 0,
        };
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            verify_spartan_with_mle_claim(
                &mut transcript,
                &matrices,
                &forged_proof,
                &forged_claim,
                &forged_assignment,
            ),
            Err(SpartanError::Matrix(
                SpartanMatrixError::InvalidAssignmentConstant
            ))
        );
    }
}
