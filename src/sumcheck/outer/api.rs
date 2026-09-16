//! Owned and borrowed inputs for equality-weighted outer sumchecks.
use super::{engine::RoundState, ordinary::*};
use crate::piop::spartan::{SpartanField, matrix::make_equality_factors};
use crate::sumcheck::arithmetic::{
    SumcheckProductReducer, reduce_two_accumulators, sum_product_accumulators,
};
use crate::sumcheck::{
    RoundBoundaryPolicy, SumcheckError, SumcheckProof, proof::OuterSumcheckOutput,
};
use crate::transcript::traits::Transcript;
use field::{FieldOps, FoldPairs, Reduce, RingOps, WideMul};

/// A and B have the same input type; C can hold wider exact products.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuterInputs<AB, C = AB> {
    pub ax: Vec<AB>,
    pub bx: Vec<AB>,
    pub cx: Vec<C>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OuterEvaluations<E> {
    pub ax: E,
    pub bx: E,
    pub cx: E,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuterOutput<E> {
    pub proof: SumcheckProof<E, 4>,
    pub point: Vec<E>,
    pub final_claim: E,
    pub evaluations: OuterEvaluations<E>,
}
impl<E> From<OuterSumcheckOutput<E>> for OuterOutput<E> {
    fn from(out: OuterSumcheckOutput<E>) -> Self {
        Self {
            proof: out.proof.sumcheck,
            point: out.eval_points,
            final_claim: out.final_claim,
            evaluations: OuterEvaluations {
                ax: out.proof.az_mle_claim,
                bx: out.proof.bz_mle_claim,
                cx: out.proof.cz_mle_claim,
            },
        }
    }
}

pub(super) fn validate_shape(a: usize, b: usize, c: usize) -> Result<usize, SumcheckError> {
    if !a.is_power_of_two() || a != b || a != c {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    Ok(a.ilog2() as usize)
}
/// Proves claim = Σₓ eq(tau,x) (A(x)B(x)−C(x)). A zero claim is allowed
/// without assuming that the residual vanishes at each row.
pub fn prove_outer_sumcheck<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claim: <F as RingOps>::Elem,
    tau: &[<F as RingOps>::Elem],
    inputs: OuterInputs<AB, C>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<OuterOutput<<F as RingOps>::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + WideMul<<F as RingOps>::Elem, AB>
        + WideMul<<F as RingOps>::Elem, C>
        + FoldPairs<AB, <F as RingOps>::Elem>
        + FoldPairs<C, <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, AB>>::Product, Output = <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, C>>::Product, Output = <F as RingOps>::Elem>,
    <F as RingOps>::Elem: SpartanField<Config = F>,
    AB: Copy + Send + Sync,
    C: Copy + Send + Sync,
{
    let (state, factors, products) = prepare_first(
        field,
        transcript,
        initial_claim,
        tau,
        &inputs.ax,
        &inputs.bx,
        &inputs.cx,
        false,
        boundary,
    )?;
    drop(inputs); // Native tables need not coexist with all subsequent fold scratch.
    Ok(continue_field(
        transcript, field, field, tau, factors, products, state, None, boundary,
    )?
    .into())
}

/// Proves claim = Σₓ eq(tau,x) (A(x)B(x)−C(x)). A zero claim is allowed
/// without assuming that the residual vanishes at each row.
pub fn prove_outer_sumcheck_from_slices<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claim: <F as RingOps>::Elem,
    tau: &[<F as RingOps>::Elem],
    ax: &[AB],
    bx: &[AB],
    cx: &[C],
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<OuterOutput<<F as RingOps>::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + WideMul<<F as RingOps>::Elem, AB>
        + WideMul<<F as RingOps>::Elem, C>
        + FoldPairs<AB, <F as RingOps>::Elem>
        + FoldPairs<C, <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, AB>>::Product, Output = <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, C>>::Product, Output = <F as RingOps>::Elem>,
    <F as RingOps>::Elem: SpartanField<Config = F>,
    AB: Copy + Send + Sync,
    C: Copy + Send + Sync,
{
    let (state, factors, products) = prepare_first(
        field,
        transcript,
        initial_claim,
        tau,
        ax,
        bx,
        cx,
        false,
        boundary,
    )?;
    Ok(continue_field(
        transcript, field, field, tau, factors, products, state, None, boundary,
    )?
    .into())
}

/// Proves a rowwise zero relation, using H(X)=h₂X(X−1) in the first round.
/// This requires A(x)B(x)=C(x) at every Boolean row, not merely a zero weighted sum.
pub fn prove_outer_zerocheck<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    tau: &[<F as RingOps>::Elem],
    inputs: OuterInputs<AB, C>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<OuterOutput<<F as RingOps>::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + WideMul<<F as RingOps>::Elem, AB>
        + WideMul<<F as RingOps>::Elem, C>
        + FoldPairs<AB, <F as RingOps>::Elem>
        + FoldPairs<C, <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, AB>>::Product, Output = <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, C>>::Product, Output = <F as RingOps>::Elem>,
    <F as RingOps>::Elem: SpartanField<Config = F>,
    AB: Copy + Send + Sync,
    C: Copy + Send + Sync,
{
    let (state, factors, products) = prepare_first(
        field,
        transcript,
        field.zero(),
        tau,
        &inputs.ax,
        &inputs.bx,
        &inputs.cx,
        true,
        boundary,
    )?;
    drop(inputs); // Native tables need not coexist with all subsequent fold scratch.
    Ok(continue_field(
        transcript, field, field, tau, factors, products, state, None, boundary,
    )?
    .into())
}

/// Proves a rowwise zero relation, using H(X)=h₂X(X−1) in the first round.
/// This requires A(x)B(x)=C(x) at every Boolean row, not merely a zero weighted sum.
pub fn prove_outer_zerocheck_from_slices<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    tau: &[<F as RingOps>::Elem],
    ax: &[AB],
    bx: &[AB],
    cx: &[C],
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<OuterOutput<<F as RingOps>::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + WideMul<<F as RingOps>::Elem, AB>
        + WideMul<<F as RingOps>::Elem, C>
        + FoldPairs<AB, <F as RingOps>::Elem>
        + FoldPairs<C, <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, AB>>::Product, Output = <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, C>>::Product, Output = <F as RingOps>::Elem>,
    <F as RingOps>::Elem: SpartanField<Config = F>,
    AB: Copy + Send + Sync,
    C: Copy + Send + Sync,
{
    let (state, factors, products) = prepare_first(
        field,
        transcript,
        field.zero(),
        tau,
        ax,
        bx,
        cx,
        true,
        boundary,
    )?;
    Ok(continue_field(
        transcript, field, field, tau, factors, products, state, None, boundary,
    )?
    .into())
}

#[allow(clippy::too_many_arguments)]
fn prepare_first<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claim: <F as RingOps>::Elem,
    tau: &[<F as RingOps>::Elem],
    ax: &[AB],
    bx: &[AB],
    cx: &[C],
    known_zero: bool,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<
    (
        RoundState<<F as RingOps>::Elem>,
        EqualityFactors<<F as RingOps>::Elem>,
        R1csProductTableBuffers<<F as RingOps>::Elem>,
    ),
    SumcheckError,
>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + WideMul<<F as RingOps>::Elem, AB>
        + WideMul<<F as RingOps>::Elem, C>
        + FoldPairs<AB, <F as RingOps>::Elem>
        + FoldPairs<C, <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, AB>>::Product, Output = <F as RingOps>::Elem>
        + Reduce<<F as WideMul<<F as RingOps>::Elem, C>>::Product, Output = <F as RingOps>::Elem>,
    <F as RingOps>::Elem: SpartanField<Config = F>,
    AB: Copy + Send + Sync,
    C: Copy + Send + Sync,
{
    let n = validate_shape(ax.len(), bx.len(), cx.len())?;
    if tau.len() != n {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    boundary.validate(n)?;
    let (low, high) =
        make_equality_factors(tau, field).map_err(|_| SumcheckError::InvalidEqualityDimensions)?;
    let mut factors = EqualityFactors::new(low.evaluations, high.evaluations, field);
    let mut state = RoundState::new(initial_claim, n, field);
    // Multiplication by one uses the operand's exact-product/reduction contract.
    // No field-valued input table is allocated before the first fold.
    let ab = |v: &AB| Reduce::reduce(field, field.mul_wide(&field.one(), v));
    let c = |v: &C| Reduce::reduce(field, field.mul_wide(&field.one(), v));
    if n == 0 {
        return Ok((
            state,
            factors,
            R1csProductTableBuffers {
                az: vec![ab(&ax[0])],
                bz: vec![ab(&bx[0])],
                cz: vec![c(&cx[0])],
            },
        ));
    }
    factors.strip(field);
    let weights = factors.weights();
    let endpoint = FactoredEndpoint::for_tau(&tau[0]);
    let accumulators = sum_product_accumulators(
        ax.len() / 2,
        |acc, pair| {
            let i = 2 * pair;
            let a0 = ab(&ax[i]);
            let a1 = ab(&ax[i + 1]);
            let b0 = ab(&bx[i]);
            let b1 = ab(&bx[i + 1]);
            let weight = weights.pair_weight(pair, field);
            let leading = field.mul(&field.sub(&a1, &a0), &field.sub(&b1, &b0));
            field.multiply_accumulate(&mut acc[1], &weight, &leading);
            if !known_zero {
                let (a, b, index) = match endpoint {
                    FactoredEndpoint::Zero | FactoredEndpoint::KnownZero => (a0, b0, i),
                    FactoredEndpoint::One => (a1, b1, i + 1),
                };
                let residual = field.sub(&field.mul(&a, &b), &c(&cx[index]));
                field.multiply_accumulate(&mut acc[0], &weight, &residual);
            }
        },
        field,
    );
    let evaluations = reduce_two_accumulators(accumulators, field, field)?;
    let coefficients = if known_zero {
        // (1−tau+(2tau−1)X) h₂ X(X−1), stored as [c₀,c₂,c₃].
        let e0 = field.sub(&field.one(), &tau[0]);
        let e1 = field.sub(&field.add(&tau[0], &tau[0]), &field.one());
        [
            field.zero(),
            field.mul(&field.sub(&e0, &e1), &evaluations[1]),
            field.mul(&e1, &evaluations[1]),
        ]
    } else {
        let inv = *field.inverse_ct(&tau[0]).value();
        reconstruct_eq_factored_cubic_without_linear(
            &state.claim,
            &tau[0],
            &inv,
            endpoint,
            evaluations,
            &field.one(),
            &field.one(),
            field,
        )
    };
    let challenge = state.sample(field, transcript, tau, &coefficients, boundary)?;
    let mut products = R1csProductTableBuffers::filled(ax.len() / 2, &field.zero());
    field.fold_pairs_into(ax, &mut products.az, &challenge);
    field.fold_pairs_into(bx, &mut products.bz, &challenge);
    field.fold_pairs_into(cx, &mut products.cz, &challenge);
    Ok((state, factors, products))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuterVerifierOutput<E> {
    pub point: Vec<E>,
    pub final_claim: E,
    pub evaluations: OuterEvaluations<E>,
}
/// Verifies the same four-coefficient proof for ordinary and zero first rounds.
pub fn verify_outer_sumcheck<F>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claim: <F as RingOps>::Elem,
    tau: &[<F as RingOps>::Elem],
    proof: &SumcheckProof<<F as RingOps>::Elem, 4>,
    evaluations: OuterEvaluations<<F as RingOps>::Elem>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<OuterVerifierOutput<<F as RingOps>::Elem>, SumcheckError>
where
    F: FieldOps,
    <F as RingOps>::Elem: SpartanField<Config = F>,
{
    use crate::sumcheck::proof::{eq_eval, validate_field_elements};
    validate_field_elements(tau, field)?;
    validate_field_elements(&[evaluations.ax, evaluations.bx, evaluations.cx], field)?;
    let (point, final_claim) =
        proof.verify_with_round_boundary(transcript, initial_claim, tau.len(), field, boundary)?;
    let eq = eq_eval(tau, &point, field)?;
    let residual = field.sub(
        &field.mul(&evaluations.ax, &evaluations.bx),
        &evaluations.cx,
    );
    if final_claim != field.mul(&eq, &residual) {
        return Err(SumcheckError::InvalidTerminalClaim);
    }
    crate::piop::spartan::absorb_field_elements(
        transcript,
        &[evaluations.ax, evaluations.bx, evaluations.cx],
        field,
    );
    Ok(OuterVerifierOutput {
        point,
        final_claim,
        evaluations,
    })
}
