//! Known-zero univariate prefix followed by the shared ordinary engine.
use super::{
    api::*,
    engine::RoundState,
    ordinary::{EqualityFactors, R1csProductTableBuffers, continue_field},
    univariate::{UnivariateSkipProof, lagrange_weights_at, prepared_weights},
};
use crate::piop::spartan::{SpartanField, matrix::make_equality_factors};
use crate::sumcheck::arithmetic::SumcheckProductReducer;
use crate::sumcheck::{RoundBoundaryPolicy, SumcheckError, SumcheckProof};
use crate::transcript::traits::Transcript;
use field::{BatchMulAcc, FieldOps, Reduce, RingOps};

/// Field-specific interpolation weights for 1..=4 little-endian prefix bits.
#[derive(Clone, Debug)]
pub struct PreparedUnivariateSkip<E> {
    skip_vars: u8,
    modulus: Vec<u8>,
    finite: Vec<Vec<E>>,
    leading: Vec<E>,
}
impl<E> PreparedUnivariateSkip<E> {
    pub fn skip_vars(&self) -> u8 {
        self.skip_vars
    }
}

pub fn prepare_univariate_skip<F: FieldOps>(
    field: &F,
    skip_vars: u8,
) -> Result<PreparedUnivariateSkip<F::Elem>, SumcheckError>
where
    F::Elem: SpartanField<Config = F>,
{
    if !(1..=4).contains(&skip_vars) {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    // All fixed base/exterior nodes must remain distinct in this field.
    let width = 1usize << skip_vars;
    let mut value = field.zero();
    for _ in 1..=(2 * width - 3) {
        value = field.add(&value, &field.one());
        if <F::Elem as SpartanField>::is_zero(&value) {
            return Err(SumcheckError::InvalidSkipField);
        }
    }
    let (finite, leading) = prepared_weights(usize::from(skip_vars), field)?;
    Ok(PreparedUnivariateSkip {
        skip_vars,
        modulus: F::Elem::canonical_modulus_encoding(field),
        finite,
        leading,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedOuterOutput<E> {
    pub prefix: UnivariateSkipProof<E>,
    pub prefix_challenge: E,
    pub tail: OuterOutput<E>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedOuterVerifierOutput<E> {
    pub prefix_challenge: E,
    pub tail: OuterVerifierOutput<E>,
}
/// Interpolates the known-zero prefix with Lagrange weights, then proves
/// the remaining Boolean rounds. K=n is supported and yields an empty tail.
pub fn prove_outer_zerocheck_with_skip<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    prepared: &PreparedUnivariateSkip<F::Elem>,
    tau_tail: &[F::Elem],
    inputs: OuterInputs<AB, C>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<SkippedOuterOutput<F::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + BatchMulAcc<<F as RingOps>::Elem, AB>
        + BatchMulAcc<<F as RingOps>::Elem, C>
        + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, AB>>::Accumulator,
            Output = <F as RingOps>::Elem,
        > + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, C>>::Accumulator,
            Output = <F as RingOps>::Elem,
        >,
    F::Elem: SpartanField<Config = F>,
    AB: Copy,
    C: Copy,
{
    let (prefix, z, claim, products, low, high) = prepare_prefix(
        field, transcript, prepared, tau_tail, &inputs.ax, &inputs.bx, &inputs.cx, boundary,
    )?;
    drop(inputs);
    let state = RoundState::new(claim, tau_tail.len(), field);
    let factors = EqualityFactors::new(low, high, field);
    let tail = continue_field(
        transcript, field, field, tau_tail, factors, products, state, None, boundary,
    )?
    .into();
    Ok(SkippedOuterOutput {
        prefix,
        prefix_challenge: z,
        tail,
    })
}
/// Interpolates the known-zero prefix with Lagrange weights, then proves
/// the remaining Boolean rounds. K=n is supported and yields an empty tail.
pub fn prove_outer_zerocheck_with_skip_from_slices<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    prepared: &PreparedUnivariateSkip<F::Elem>,
    tau_tail: &[F::Elem],
    ax: &[AB],
    bx: &[AB],
    cx: &[C],
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<SkippedOuterOutput<F::Elem>, SumcheckError>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + BatchMulAcc<<F as RingOps>::Elem, AB>
        + BatchMulAcc<<F as RingOps>::Elem, C>
        + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, AB>>::Accumulator,
            Output = <F as RingOps>::Elem,
        > + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, C>>::Accumulator,
            Output = <F as RingOps>::Elem,
        >,
    F::Elem: SpartanField<Config = F>,
    AB: Copy,
    C: Copy,
{
    let (prefix, z, claim, products, low, high) =
        prepare_prefix(field, transcript, prepared, tau_tail, ax, bx, cx, boundary)?;
    let state = RoundState::new(claim, tau_tail.len(), field);
    let factors = EqualityFactors::new(low, high, field);
    let tail = continue_field(
        transcript, field, field, tau_tail, factors, products, state, None, boundary,
    )?
    .into();
    Ok(SkippedOuterOutput {
        prefix,
        prefix_challenge: z,
        tail,
    })
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn prepare_prefix<F, AB, C>(
    field: &F,
    transcript: &mut impl Transcript,
    prepared: &PreparedUnivariateSkip<F::Elem>,
    tau_tail: &[F::Elem],
    ax: &[AB],
    bx: &[AB],
    cx: &[C],
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<
    (
        UnivariateSkipProof<F::Elem>,
        F::Elem,
        F::Elem,
        R1csProductTableBuffers<F::Elem>,
        Vec<F::Elem>,
        Vec<F::Elem>,
    ),
    SumcheckError,
>
where
    F: FieldOps
        + SumcheckProductReducer<<F as RingOps>::Elem>
        + BatchMulAcc<<F as RingOps>::Elem, AB>
        + BatchMulAcc<<F as RingOps>::Elem, C>
        + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, AB>>::Accumulator,
            Output = <F as RingOps>::Elem,
        > + Reduce<
            <F as BatchMulAcc<<F as RingOps>::Elem, C>>::Accumulator,
            Output = <F as RingOps>::Elem,
        >,
    F::Elem: SpartanField<Config = F>,
    AB: Copy,
    C: Copy,
{
    let n = validate_shape(ax.len(), bx.len(), cx.len())?;
    let k = usize::from(prepared.skip_vars);
    if n < k || tau_tail.len() != n - k {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    if prepared.modulus != F::Elem::canonical_modulus_encoding(field) {
        return Err(SumcheckError::FieldConfigurationMismatch);
    }
    boundary.validate(n - k)?;
    let width = 1usize << k;
    let (low, high) = make_equality_factors(tau_tail, field)
        .map_err(|_| SumcheckError::InvalidEqualityDimensions)?;
    let ab = |weights: &[F::Elem], values: &[AB]| {
        Reduce::reduce(field, field.batch_mul_acc(weights, values))
    };
    let c = |weights: &[F::Elem], values: &[C]| {
        Reduce::reduce(field, field.batch_mul_acc(weights, values))
    };
    let mut message = vec![field.zero(); width - 1];
    for block in 0..ax.len() / width {
        let start = block * width;
        let a = &ax[start..start + width];
        let b = &bx[start..start + width];
        let cv = &cx[start..start + width];
        let eq = field.mul(
            &low.evaluations[block % low.evaluations.len()],
            &high.evaluations[block / low.evaluations.len()],
        );
        for (sum, w) in message.iter_mut().zip(&prepared.finite) {
            let residual = field.sub(&field.mul(&ab(w, a), &ab(w, b)), &c(w, cv));
            *sum = field.add(sum, &field.mul(&eq, &residual));
        }
        let leading = field.mul(&ab(&prepared.leading, a), &ab(&prepared.leading, b));
        message[width - 2] = field.add(&message[width - 2], &field.mul(&eq, &leading));
    }
    let prefix = UnivariateSkipProof::from_ordered_message(k, message)?;
    let reduction = prefix.verify_reduction(transcript, field)?;
    let weights = lagrange_weights_at(&reduction.z, width, field);
    let mut products = R1csProductTableBuffers::filled(ax.len() / width, &field.zero());
    for (block, ((a, b), cv)) in products
        .az
        .iter_mut()
        .zip(&mut products.bz)
        .zip(&mut products.cz)
        .enumerate()
    {
        let start = block * width;
        *a = ab(&weights, &ax[start..start + width]);
        *b = ab(&weights, &bx[start..start + width]);
        *cv = c(&weights, &cx[start..start + width]);
    }
    Ok((
        prefix,
        reduction.z,
        reduction.q_at_z,
        products,
        low.evaluations,
        high.evaluations,
    ))
}

pub fn verify_outer_zerocheck_with_skip<F: FieldOps>(
    field: &F,
    transcript: &mut impl Transcript,
    prepared: &PreparedUnivariateSkip<F::Elem>,
    tau_tail: &[F::Elem],
    prefix: &UnivariateSkipProof<F::Elem>,
    tail: &SumcheckProof<F::Elem, 4>,
    evaluations: OuterEvaluations<F::Elem>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<SkippedOuterVerifierOutput<F::Elem>, SumcheckError>
where
    F::Elem: SpartanField<Config = F>,
{
    if prepared.modulus != F::Elem::canonical_modulus_encoding(field) {
        return Err(SumcheckError::FieldConfigurationMismatch);
    }
    if prefix.skip_vars != prepared.skip_vars {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    prefix.validate_shape(usize::from(prefix.skip_vars) + tau_tail.len())?;
    if tail.round_polynomials.len() != tau_tail.len() {
        return Err(SumcheckError::InvalidRoundCount {
            expected: tau_tail.len(),
            actual: tail.round_polynomials.len(),
        });
    }
    boundary.validate(tau_tail.len())?;
    crate::sumcheck::proof::validate_field_elements(tau_tail, field)?;
    crate::sumcheck::proof::validate_field_elements(
        &[evaluations.ax, evaluations.bx, evaluations.cx],
        field,
    )?;
    for round in &tail.round_polynomials {
        crate::sumcheck::proof::validate_field_elements(round, field)?;
    }
    let reduction = prefix.verify_reduction(transcript, field)?;
    let tail = verify_outer_sumcheck(
        field,
        transcript,
        reduction.q_at_z,
        tau_tail,
        tail,
        evaluations,
        boundary,
    )?;
    Ok(SkippedOuterVerifierOutput {
        prefix_challenge: reduction.z,
        tail,
    })
}
