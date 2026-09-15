//! Spartan's equality-weighted outer sumcheck and matrix-product inner sumcheck.
//!
//! Round polynomials are stored in coefficient form. The outer proof uses four
//! coefficients (degree three), while the inner proof uses three (degree two).
//! The prover omits the linear coefficient while accumulating a round and
//! reconstructs it from `g(0) + g(1) = current_claim` before absorption.

use crate::piop::spartan::SpartanField as _;
use crate::utils::delayed_reduction::EncodedMac;
#[cfg(test)]
use field::Uint;
use field::{Fp, RingOps};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use thiserror::Error;

#[cfg(test)]
use crate::utils::delayed_reduction::reference::{Accumulatable, Reduce as ReferenceReduce};
use crate::{
    poly::mle::DenseMultilinearExtension, transcript::traits::Transcript,
    utils::delayed_reduction::DelayedReductionError,
};
use field::{CtMask, CtSelect};
use num_traits::Zero;

use super::{
    SpartanField, absorb_field_elements,
    grinding::{
        GrindingDomain, GrindingError, GrindingRound, MAX_GRINDING_BITS, grind_and_absorb,
        verify_and_absorb,
    },
    squeeze_field,
};

/// Protocol accumulation hook for shared delayed products and the independent
/// test oracle. Workers merge exact products before the shared context reduces.
#[doc(hidden)]
pub trait SumcheckProductReducer<F>: Sync
where
    F: SpartanField,
{
    type Accumulator: Send;

    fn accumulator_zero(&self) -> Self::Accumulator;
    fn multiply_accumulate(&self, accumulator: &mut Self::Accumulator, lhs: &F, rhs: &F);
    fn merge(&self, accumulator: &mut Self::Accumulator, other: Self::Accumulator);
    fn reduce(
        &self,
        accumulator: Self::Accumulator,
        config: &F::Config,
    ) -> Result<F, SumcheckError>;
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

/// Failures produced while reducing or checking a sumcheck claim.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SumcheckError {
    #[error("field challenge sampling exhausted its rejection budget")]
    SamplingExhausted,
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
    #[error("a sumcheck value uses a different field configuration")]
    FieldConfigurationMismatch,
    #[error("a sumcheck value has a noncanonical field representation")]
    NonCanonicalFieldElement,
    #[error(transparent)]
    Grinding(#[from] GrindingError),
    #[error("sumcheck proof has {actual} grinding nonces, expected {expected}")]
    InvalidGrindingNonceCount { expected: usize, actual: usize },
    #[error(transparent)]
    DelayedReduction(#[from] DelayedReductionError),
}

/// Controls the transcript boundary between an absorbed round polynomial and
/// the verifier challenge that follows it.
pub(super) trait RoundBoundaryPolicy {
    /// Validates policy-level proof shape before the transcript is mutated.
    fn validate(&self, _expected_rounds: usize) -> Result<(), SumcheckError> {
        Ok(())
    }

    /// Processes the prover message after absorption and before its challenge.
    fn after_round<T: Transcript>(
        &mut self,
        transcript: &mut T,
        round: usize,
    ) -> Result<(), SumcheckError>;
}

/// Existing sumcheck transcript behavior: no bytes between message and
/// challenge.
pub(super) struct UngrindedRoundBoundary;

impl RoundBoundaryPolicy for UngrindedRoundBoundary {
    fn after_round<T: Transcript>(
        &mut self,
        _transcript: &mut T,
        _round: usize,
    ) -> Result<(), SumcheckError> {
        Ok(())
    }
}

#[allow(dead_code)]
pub(super) struct ProverGrindingRoundBoundary<D> {
    bits: u32,
    round_offset: usize,
    nonces: Vec<u64>,
    _domain: core::marker::PhantomData<fn() -> D>,
}

impl<D> ProverGrindingRoundBoundary<D> {
    #[allow(dead_code)]
    pub(super) fn with_round_offset(bits: u32, round_offset: usize) -> Self {
        Self {
            bits,
            round_offset,
            nonces: Vec::new(),
            _domain: core::marker::PhantomData,
        }
    }

    /// The nonces found so far, in round order (empty at difficulty 0).
    #[allow(dead_code)]
    pub(super) fn into_nonces(self) -> Vec<u64> {
        self.nonces
    }
}

impl<D: GrindingDomain> RoundBoundaryPolicy for ProverGrindingRoundBoundary<D> {
    fn validate(&self, _expected_rounds: usize) -> Result<(), SumcheckError> {
        // Difficulty 0 = the boundary does not exist: no transcript bytes,
        // no nonces (the λ = 100 profiles).
        if self.bits == 0 {
            return Ok(());
        }
        validate_grinding_configuration::<D>(self.bits)
    }

    fn after_round<T: Transcript>(
        &mut self,
        transcript: &mut T,
        round: usize,
    ) -> Result<(), SumcheckError> {
        if self.bits == 0 {
            return Ok(());
        }
        let _scope = tracing::info_span!("spartan:round_grinding_prove").entered();
        let round = self
            .round_offset
            .checked_add(round)
            .and_then(|round| u64::try_from(round).ok())
            .expect("an in-memory sumcheck round index fits in u64");
        let nonce = grind_and_absorb::<D, _>(transcript, GrindingRound::new(round), self.bits)?;
        self.nonces.push(nonce);
        Ok(())
    }
}

struct VerifierGrindingRoundBoundary<'a, D> {
    bits: u32,
    nonces: &'a [u64],
    _domain: core::marker::PhantomData<fn() -> D>,
}

impl<'a, D> VerifierGrindingRoundBoundary<'a, D> {
    fn new(bits: u32, nonces: &'a [u64]) -> Self {
        Self {
            bits,
            nonces,
            _domain: core::marker::PhantomData,
        }
    }
}

impl<D: GrindingDomain> RoundBoundaryPolicy for VerifierGrindingRoundBoundary<'_, D> {
    fn validate(&self, expected_rounds: usize) -> Result<(), SumcheckError> {
        // Difficulty 0: the boundary does not exist, so a canonical proof
        // carries NO nonces.
        let expected = if self.bits == 0 {
            0
        } else {
            validate_grinding_configuration::<D>(self.bits)?;
            expected_rounds
        };
        if self.nonces.len() != expected {
            return Err(SumcheckError::InvalidGrindingNonceCount {
                expected,
                actual: self.nonces.len(),
            });
        }
        Ok(())
    }

    fn after_round<T: Transcript>(
        &mut self,
        transcript: &mut T,
        round: usize,
    ) -> Result<(), SumcheckError> {
        if self.bits == 0 {
            return Ok(());
        }
        let _scope = tracing::info_span!("spartan:round_grinding_verify").entered();
        let round_index =
            u64::try_from(round).expect("an in-memory sumcheck round index fits in u64");
        verify_and_absorb::<D, _>(
            transcript,
            GrindingRound::new(round_index),
            self.bits,
            self.nonces[round],
        )?;
        Ok(())
    }
}

fn validate_grinding_configuration<D: GrindingDomain>(bits: u32) -> Result<(), SumcheckError> {
    if !(1..=MAX_GRINDING_BITS).contains(&bits) {
        return Err(GrindingError::InvalidDifficulty { bits }.into());
    }
    if D::DOMAIN.is_empty() {
        return Err(GrindingError::EmptyDomain.into());
    }
    Ok(())
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
        let mut round_boundary = UngrindedRoundBoundary;
        self.verify_with_round_boundary(
            transcript,
            initial_claim,
            expected_rounds,
            field_cfg,
            &mut round_boundary,
        )
    }

    /// Verifies a sumcheck with one typed proof-of-work boundary between each
    /// absorbed round polynomial and its Fiat--Shamir challenge.
    pub(crate) fn verify_grinded<D: GrindingDomain>(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        expected_rounds: usize,
        field_cfg: &F::Config,
        grinding_nonces: &[u64],
        grinding_bits: u32,
    ) -> Result<(Vec<F>, F), SumcheckError> {
        let mut round_boundary =
            VerifierGrindingRoundBoundary::<D>::new(grinding_bits, grinding_nonces);
        self.verify_with_round_boundary(
            transcript,
            initial_claim,
            expected_rounds,
            field_cfg,
            &mut round_boundary,
        )
    }

    fn verify_with_round_boundary<P>(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        expected_rounds: usize,
        field_cfg: &F::Config,
        round_boundary: &mut P,
    ) -> Result<(Vec<F>, F), SumcheckError>
    where
        P: RoundBoundaryPolicy,
    {
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
        round_boundary.validate(expected_rounds)?;
        validate_field_elements(core::slice::from_ref(&initial_claim), field_cfg)?;
        for coefficients in &self.round_polynomials {
            validate_field_elements(coefficients, field_cfg)?;
        }

        let zero = F::zero_with_cfg(field_cfg);
        let mut current_claim = initial_claim;
        let mut eval_points = Vec::with_capacity(expected_rounds);

        for (round, coefficients) in self.round_polynomials.iter().enumerate() {
            absorb_field_elements(transcript, coefficients, &field_cfg);

            let at_zero = coefficients[0].clone();
            let at_one = coefficients
                .iter()
                .fold(zero.clone(), |mut sum, coefficient| {
                    sum = field_cfg.add(&(sum), &(coefficient));
                    sum
                });

            if (field_cfg).add(&at_zero, &at_one) != current_claim {
                return Err(SumcheckError::InvalidRoundClaim { round });
            }

            round_boundary.after_round(transcript, round)?;
            let challenge = squeeze_field(transcript, field_cfg)?;
            current_claim = evaluate_polynomial(coefficients, &challenge, &zero, &field_cfg);
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

/// Dense Bit-row MLEs for `Az`, `Bz`, and `Cz`.
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
        let mut round_boundary = UngrindedRoundBoundary;
        self.verify_with_round_boundary(
            transcript,
            initial_claim,
            tau,
            field_cfg,
            &mut round_boundary,
        )
    }

    /// Verifies a cubic outer sumcheck with one PoW nonce adjacent to every
    /// round polynomial.
    ///
    /// The nonce at index `i` is checked after round polynomial `i` is
    /// absorbed and before challenge `i` is sampled. `D` supplies the typed,
    /// protocol-specific grinding domain; `grinding_bits` is fixed across all
    /// rounds in this proof.
    #[allow(dead_code)]
    pub(crate) fn verify_grinded<D: GrindingDomain>(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        tau: &[F],
        field_cfg: &F::Config,
        grinding_nonces: &[u64],
        grinding_bits: u32,
    ) -> Result<OuterSumcheckVerifierOutput<F>, SumcheckError> {
        let mut round_boundary =
            VerifierGrindingRoundBoundary::<D>::new(grinding_bits, grinding_nonces);
        self.verify_with_round_boundary(
            transcript,
            initial_claim,
            tau,
            field_cfg,
            &mut round_boundary,
        )
    }

    fn verify_with_round_boundary<P>(
        &self,
        transcript: &mut impl Transcript,
        initial_claim: F,
        tau: &[F],
        field_cfg: &F::Config,
        round_boundary: &mut P,
    ) -> Result<OuterSumcheckVerifierOutput<F>, SumcheckError>
    where
        P: RoundBoundaryPolicy,
    {
        let terminal_evaluations = [
            self.az_mle_claim.clone(),
            self.bz_mle_claim.clone(),
            self.cz_mle_claim.clone(),
        ];
        // All verifier-supplied field elements must be checked before the
        // first proof byte is absorbed. Runtime Montgomery values otherwise
        // carry their own configuration, and mixed-config arithmetic is not
        // a protocol-level field coercion.
        validate_field_elements(tau, field_cfg)?;
        validate_field_elements(&terminal_evaluations, field_cfg)?;

        let (eval_points, final_claim) = self.sumcheck.verify_with_round_boundary(
            transcript,
            initial_claim,
            tau.len(),
            field_cfg,
            round_boundary,
        )?;

        absorb_field_elements(transcript, &terminal_evaluations, &field_cfg);

        let eq = eq_eval(tau, &eval_points, field_cfg)?;
        let residual = (field_cfg).sub(
            &(field_cfg).mul(&self.az_mle_claim, &self.bz_mle_claim),
            &self.cz_mle_claim,
        );
        let expected_claim = (field_cfg).mul(&eq, &residual);
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
///
/// The prover factors the public current-coordinate equality polynomial out of
/// its hot loop, but reconstructs and absorbs the original four cubic
/// coefficients. This is an arithmetic optimization only: proof shape and
/// transcript bytes are unchanged.
#[cfg(test)]
pub(crate) fn prove_outer_sumcheck<F>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    tau: &[F],
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
) -> Result<OuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
{
    let reducer = field_cfg.clone();
    prove_outer_sumcheck_with_reducer(
        transcript,
        initial_claim,
        tau,
        (eq_low, eq_high),
        products,
        field_cfg,
        &reducer,
    )
}

pub(crate) fn prove_outer_sumcheck_with_reducer<F, R>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    tau: &[F],
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
    reducer: &R,
) -> Result<OuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let mut round_boundary = UngrindedRoundBoundary;
    prove_outer_sumcheck_with_reducer_and_round_boundary(
        transcript,
        initial_claim,
        tau,
        (eq_low, eq_high),
        products,
        field_cfg,
        reducer,
        &mut round_boundary,
    )
}

/// Proves the cubic outer sumcheck with one PoW nonce between every absorbed
/// round polynomial and its following Fiat--Shamir challenge.
///
/// Returned nonces are in round order, so `nonces[i]` is adjacent to
/// `proof.sumcheck.round_polynomials[i]`. `D` supplies a typed,
/// protocol-specific grinding domain and `grinding_bits` is fixed for all
/// rounds. The ordinary [`prove_outer_sumcheck_with_reducer`] path remains
/// ungrinded and transcript-compatible with existing proofs.
#[allow(dead_code)]
pub(crate) fn prove_outer_sumcheck_with_reducer_grinded<D, F, R>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    tau: &[F],
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
    reducer: &R,
    grinding_bits: u32,
) -> Result<(OuterSumcheckOutput<F>, Vec<u64>), SumcheckError>
where
    D: GrindingDomain,
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let mut round_boundary = ProverGrindingRoundBoundary::<D>::with_round_offset(grinding_bits, 0);
    let output = prove_outer_sumcheck_with_reducer_and_round_boundary(
        transcript,
        initial_claim,
        tau,
        (eq_low, eq_high),
        products,
        field_cfg,
        reducer,
        &mut round_boundary,
    )?;
    debug_assert_eq!(
        round_boundary.nonces.len(),
        if grinding_bits == 0 {
            0
        } else {
            output.proof.sumcheck.round_polynomials.len()
        }
    );
    Ok((output, round_boundary.nonces))
}

#[allow(clippy::too_many_arguments)]
fn prove_outer_sumcheck_with_reducer_and_round_boundary<F, R, P>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    tau: &[F],
    (eq_low, eq_high): (DenseMultilinearExtension<F>, DenseMultilinearExtension<F>),
    products: R1csProductMles<F>,
    field_cfg: &F::Config,
    reducer: &R,
    round_boundary: &mut P,
) -> Result<OuterSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
    P: RoundBoundaryPolicy,
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
    if tau.len() != num_vars {
        return Err(SumcheckError::InvalidEqualityDimensions);
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
    round_boundary.validate(num_vars)?;

    let zero = F::zero_with_cfg(field_cfg);
    let one = F::one_with_cfg(field_cfg);
    let tau_inverses = batch_invert_nonzero(tau, field_cfg);
    let mut eq_low = eq_low.evaluations;
    let mut eq_high = eq_high.evaluations;
    let mut products = R1csProductTableBuffers::from_mles(products);

    // Scratch allocations ping-pong with the active tables after every fold.
    let mut product_scratch = R1csProductTableBuffers::filled(products.len() / 2, &zero);
    let mut eq_low_scratch = vec![zero.clone(); eq_low.len() / 2];
    let mut eq_high_scratch = vec![zero.clone(); eq_high.len() / 2];

    let mut current_claim = initial_claim;
    let mut bound_equality = one.clone();
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut coefficients_without_linear = if num_vars == 0 {
        std::array::from_fn(|_| zero.clone())
    } else {
        let equality_weights = if eq_low.len() > 1 {
            strip_equality_coordinate(&eq_low, &mut eq_low_scratch, &field_cfg);
            StrippedEqualityWeights::low(&eq_low_scratch, &eq_high)
        } else {
            strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
            StrippedEqualityWeights::high(&eq_high_scratch)
        };
        compute_eq_factored_coefficients_without_linear(
            &products,
            equality_weights,
            &current_claim,
            &tau[0],
            &tau_inverses[0],
            &bound_equality,
            &one,
            reducer,
            &field_cfg,
        )?
    };

    // Bind the low equality variables first. The product traversal folds the
    // active tables and prepares the next round polynomial in one pass.
    while eq_low.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
            round_boundary,
        )?;

        let next_product_len = products.len() / 2;
        product_scratch.truncate(next_product_len);
        let bound_factor = equality_coordinate_evaluation(
            &tau[eval_points.len() - 1],
            &challenge,
            &one,
            &field_cfg,
        );
        bound_equality = (field_cfg).mul(&bound_equality, &bound_factor);
        std::mem::swap(&mut eq_low, &mut eq_low_scratch);

        if next_product_len > 1 {
            let equality_weights = if eq_low.len() > 1 {
                eq_low_scratch.truncate(eq_low.len() / 2);
                strip_equality_coordinate(&eq_low, &mut eq_low_scratch, &field_cfg);
                StrippedEqualityWeights::low(&eq_low_scratch, &eq_high)
            } else {
                eq_high_scratch.truncate(eq_high.len() / 2);
                strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
                StrippedEqualityWeights::high(&eq_high_scratch)
            };
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                equality_weights,
                &current_claim,
                &tau[eval_points.len()],
                &tau_inverses[eval_points.len()],
                &bound_equality,
                &one,
                reducer,
                &field_cfg,
            )?;
        } else {
            debug_assert_eq!(next_product_len, 1);
            fold_product_tables(&products, &mut product_scratch, &challenge, &field_cfg);
        }

        products.swap(&mut product_scratch);
    }

    debug_assert_eq!(eq_low.len(), 1);
    debug_assert_eq!(products.len(), eq_high.len());

    while eq_high.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
            round_boundary,
        )?;

        let next_eq_high_len = eq_high.len() / 2;
        debug_assert_eq!(products.len() / 2, next_eq_high_len);
        product_scratch.truncate(next_eq_high_len);
        let bound_factor = equality_coordinate_evaluation(
            &tau[eval_points.len() - 1],
            &challenge,
            &one,
            &field_cfg,
        );
        bound_equality = (field_cfg).mul(&bound_equality, &bound_factor);
        std::mem::swap(&mut eq_high, &mut eq_high_scratch);

        if next_eq_high_len == 1 {
            fold_product_tables(&products, &mut product_scratch, &challenge, &field_cfg);
        } else {
            eq_high_scratch.truncate(eq_high.len() / 2);
            strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                StrippedEqualityWeights::high(&eq_high_scratch),
                &current_claim,
                &tau[eval_points.len()],
                &tau_inverses[eval_points.len()],
                &bound_equality,
                &one,
                reducer,
                &field_cfg,
            )?;
        }

        products.swap(&mut product_scratch);
    }

    let az_mle_claim = products.az[0].clone();
    let bz_mle_claim = products.bz[0].clone();
    let cz_mle_claim = products.cz[0].clone();
    debug_assert_eq!(eq_low[0], one);
    debug_assert_eq!(eq_high[0], one);
    let expected_terminal_claim = (field_cfg).mul(
        &bound_equality,
        &(field_cfg).sub(
            &(field_cfg).mul(&az_mle_claim, &bz_mle_claim),
            &cz_mle_claim,
        ),
    );
    if current_claim != expected_terminal_claim {
        return Err(SumcheckError::InvalidTerminalClaim);
    }

    let terminal_evaluations = [
        az_mle_claim.clone(),
        bz_mle_claim.clone(),
        cz_mle_claim.clone(),
    ];
    absorb_field_elements(transcript, &terminal_evaluations, &field_cfg);

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

/// Independent direct-cubic oracle for transcript-compatibility tests. This is
/// intentionally scalar and materializes the full equality table so it cannot
/// accidentally share the optimized factor-stripping arithmetic.
#[cfg(test)]
fn prove_outer_sumcheck_direct_reference<F>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    tau: &[F],
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
    if tau.len() != num_vars {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }

    let zero = F::zero_with_cfg(field_cfg);
    let one = F::one_with_cfg(field_cfg);
    let two = (field_cfg).add(&one, &one);
    let three = (field_cfg).add(&two, &one);
    let interpolation = CubicInterpolation::new(field_cfg);
    let mut equality = super::matrix::eq_table(tau, field_cfg)
        .expect("test reference receives valid equality coordinates");
    let mut products = R1csProductTableBuffers::from_mles(products);
    let mut product_scratch = R1csProductTableBuffers::filled(products.len() / 2, &zero);
    let mut equality_scratch = vec![zero.clone(); equality.len() / 2];
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    while products.len() > 1 {
        let mut evaluations = std::array::from_fn(|_| zero.clone());
        for pair in 0..products.len() / 2 {
            let index = 2 * pair;
            for (evaluation, point) in
                evaluations
                    .iter_mut()
                    .zip([zero.clone(), two.clone(), three.clone()])
            {
                let equality_at =
                    interpolate_pair(&equality[index], &equality[index + 1], &point, &field_cfg);
                let az_at = interpolate_pair(
                    &products.az[index],
                    &products.az[index + 1],
                    &point,
                    &field_cfg,
                );
                let bz_at = interpolate_pair(
                    &products.bz[index],
                    &products.bz[index + 1],
                    &point,
                    &field_cfg,
                );
                let cz_at = interpolate_pair(
                    &products.cz[index],
                    &products.cz[index + 1],
                    &point,
                    &field_cfg,
                );
                *evaluation = field_cfg.add(
                    &(*evaluation),
                    &(&(field_cfg).mul(
                        &equality_at,
                        &(field_cfg).sub(&(field_cfg).mul(&az_at, &bz_at), &cz_at),
                    )),
                );
            }
        }

        let coefficients_without_linear =
            interpolation.coefficients_without_linear(&current_claim, evaluations, &field_cfg);
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        )?;

        let next_len = products.len() / 2;
        product_scratch.truncate(next_len);
        equality_scratch.truncate(next_len);
        fold_product_tables(&products, &mut product_scratch, &challenge, &field_cfg);
        fold_table(&equality, &mut equality_scratch, &challenge, &field_cfg);
        products.swap(&mut product_scratch);
        std::mem::swap(&mut equality, &mut equality_scratch);
    }

    let az_mle_claim = products.az[0].clone();
    let bz_mle_claim = products.bz[0].clone();
    let cz_mle_claim = products.cz[0].clone();
    absorb_field_elements(
        transcript,
        &[
            az_mle_claim.clone(),
            bz_mle_claim.clone(),
            cz_mle_claim.clone(),
        ],
        &field_cfg,
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

/// Proves the first outer round from exact native u32 relation products, then
/// continues with the ordinary field-valued prover.
///
/// The native tables are consumed at the first Fiat--Shamir challenge. Every
/// folded entry is reduced to the field before it is stored or used in a later
/// multiplication.
pub(crate) fn prove_outer_sumcheck_u32_native_with_reducer<R>(
    transcript: &mut impl Transcript,
    initial_claim: Fp<2>,
    tau: &[Fp<2>],
    (eq_low, eq_high): (
        DenseMultilinearExtension<Fp<2>>,
        DenseMultilinearExtension<Fp<2>>,
    ),
    products: R1csProductMles<u64>,
    field_cfg: &field::FpCtx<2>,
    reducer: &R,
) -> Result<OuterSumcheckOutput<Fp<2>>, SumcheckError>
where
    R: SumcheckProductReducer<Fp<2>> + SumcheckLinearReducer,
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
    if tau.len() != num_vars {
        return Err(SumcheckError::InvalidEqualityDimensions);
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

    let zero = Fp::<2>::zero_with_cfg(field_cfg);
    let one = Fp::<2>::one_with_cfg(field_cfg);
    let tau_inverses = batch_invert_nonzero(tau, field_cfg);
    let mut eq_low = eq_low.evaluations;
    let mut eq_high = eq_high.evaluations;
    let native_products = R1csProductTableBuffers::from_mles(products);
    let mut current_claim = initial_claim;
    let mut bound_equality = one.clone();
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    if num_vars == 0 {
        let az_mle_claim = native_to_field(native_products.az[0], field_cfg);
        let bz_mle_claim = native_to_field(native_products.bz[0], field_cfg);
        let cz_mle_claim = native_to_field(native_products.cz[0], field_cfg);
        debug_assert_eq!(
            current_claim,
            (field_cfg).mul(
                &bound_equality,
                &(field_cfg).sub(
                    &(field_cfg).mul(&az_mle_claim, &bz_mle_claim),
                    &cz_mle_claim
                ),
            )
        );
        absorb_field_elements(
            transcript,
            &[
                az_mle_claim.clone(),
                bz_mle_claim.clone(),
                cz_mle_claim.clone(),
            ],
            &field_cfg,
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

    let mut eq_low_scratch = vec![zero.clone(); eq_low.len() / 2];
    let mut eq_high_scratch = vec![zero.clone(); eq_high.len() / 2];
    let equality_weights = if eq_low.len() > 1 {
        strip_equality_coordinate(&eq_low, &mut eq_low_scratch, &field_cfg);
        StrippedEqualityWeights::low(&eq_low_scratch, &eq_high)
    } else {
        strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
        StrippedEqualityWeights::high(&eq_high_scratch)
    };
    let coefficients_without_linear = compute_u32_native_eq_factored_coefficients_without_linear(
        &native_products,
        equality_weights,
        &current_claim,
        &tau[0],
        &tau_inverses[0],
        &bound_equality,
        &one,
        &zero,
        reducer,
        &field_cfg,
    )?;
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
        &zero,
        field_cfg,
    )?;

    let bound_factor = equality_coordinate_evaluation(&tau[0], &challenge, &one, &field_cfg);
    bound_equality = (field_cfg).mul(&bound_equality, &bound_factor);

    let mut products =
        fold_u64_product_tables_to_field(&native_products, &challenge, &zero, field_cfg, reducer)?;
    if eq_low.len() > 1 {
        std::mem::swap(&mut eq_low, &mut eq_low_scratch);
    } else {
        std::mem::swap(&mut eq_high, &mut eq_high_scratch);
    }

    let mut coefficients_without_linear = if products.len() > 1 {
        let equality_weights = if eq_low.len() > 1 {
            eq_low_scratch.truncate(eq_low.len() / 2);
            strip_equality_coordinate(&eq_low, &mut eq_low_scratch, &field_cfg);
            StrippedEqualityWeights::low(&eq_low_scratch, &eq_high)
        } else {
            eq_high_scratch.truncate(eq_high.len() / 2);
            strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
            StrippedEqualityWeights::high(&eq_high_scratch)
        };
        compute_eq_factored_coefficients_without_linear(
            &products,
            equality_weights,
            &current_claim,
            &tau[1],
            &tau_inverses[1],
            &bound_equality,
            &one,
            reducer,
            &field_cfg,
        )?
    } else {
        std::array::from_fn(|_| zero.clone())
    };

    // From this boundary onward every active table is field-valued. This is
    // the same loop structure as the generic prover, beginning at round one.
    let mut product_scratch = R1csProductTableBuffers::filled(products.len() / 2, &zero);

    while eq_low.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            field_cfg,
        )?;

        let next_product_len = products.len() / 2;
        product_scratch.truncate(next_product_len);
        let bound_factor = equality_coordinate_evaluation(
            &tau[eval_points.len() - 1],
            &challenge,
            &one,
            &field_cfg,
        );
        bound_equality = (field_cfg).mul(&bound_equality, &bound_factor);
        std::mem::swap(&mut eq_low, &mut eq_low_scratch);

        if next_product_len > 1 {
            let equality_weights = if eq_low.len() > 1 {
                eq_low_scratch.truncate(eq_low.len() / 2);
                strip_equality_coordinate(&eq_low, &mut eq_low_scratch, &field_cfg);
                StrippedEqualityWeights::low(&eq_low_scratch, &eq_high)
            } else {
                eq_high_scratch.truncate(eq_high.len() / 2);
                strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
                StrippedEqualityWeights::high(&eq_high_scratch)
            };
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                equality_weights,
                &current_claim,
                &tau[eval_points.len()],
                &tau_inverses[eval_points.len()],
                &bound_equality,
                &one,
                reducer,
                &field_cfg,
            )?;
        } else {
            debug_assert_eq!(next_product_len, 1);
            fold_product_tables(&products, &mut product_scratch, &challenge, &field_cfg);
        }

        products.swap(&mut product_scratch);
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
        )?;

        let next_eq_high_len = eq_high.len() / 2;
        product_scratch.truncate(next_eq_high_len);
        let bound_factor = equality_coordinate_evaluation(
            &tau[eval_points.len() - 1],
            &challenge,
            &one,
            &field_cfg,
        );
        bound_equality = (field_cfg).mul(&bound_equality, &bound_factor);
        std::mem::swap(&mut eq_high, &mut eq_high_scratch);

        if next_eq_high_len == 1 {
            fold_product_tables(&products, &mut product_scratch, &challenge, &field_cfg);
        } else {
            eq_high_scratch.truncate(eq_high.len() / 2);
            strip_equality_coordinate(&eq_high, &mut eq_high_scratch, &field_cfg);
            coefficients_without_linear = fold_products_and_compute_next(
                &products,
                &mut product_scratch,
                &challenge,
                StrippedEqualityWeights::high(&eq_high_scratch),
                &current_claim,
                &tau[eval_points.len()],
                &tau_inverses[eval_points.len()],
                &bound_equality,
                &one,
                reducer,
                &field_cfg,
            )?;
        }

        products.swap(&mut product_scratch);
    }

    let az_mle_claim = products.az[0].clone();
    let bz_mle_claim = products.bz[0].clone();
    let cz_mle_claim = products.cz[0].clone();
    debug_assert_eq!(eq_low[0], one);
    debug_assert_eq!(eq_high[0], one);
    debug_assert_eq!(
        current_claim,
        (field_cfg).mul(
            &bound_equality,
            &(field_cfg).sub(
                &(field_cfg).mul(&az_mle_claim, &bz_mle_claim),
                &cz_mle_claim
            ),
        )
    );

    absorb_field_elements(
        transcript,
        &[
            az_mle_claim.clone(),
            bz_mle_claim.clone(),
            cz_mle_claim.clone(),
        ],
        &field_cfg,
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
    let reducer = field_cfg.clone();
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
    let mut round_boundary = UngrindedRoundBoundary;
    prove_inner_sumcheck_with_reducer_and_round_boundary(
        transcript,
        initial_claim,
        batched_matrix_mle,
        witness_mle,
        field_cfg,
        reducer,
        &mut round_boundary,
    )
}

/// Proves an ordinary quadratic inner sumcheck with a typed grinding boundary
/// after every round message. `round_offset` lets a caller prepend
/// transcript-identical rounds computed by another prover kernel while keeping
/// the grinding round indices globally consecutive.
#[allow(dead_code)]
pub(crate) fn prove_inner_sumcheck_with_reducer_grinded<D, F, R>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    batched_matrix_mle: DenseMultilinearExtension<F>,
    witness_mle: DenseMultilinearExtension<F>,
    field_cfg: &F::Config,
    reducer: &R,
    grinding_bits: u32,
    round_offset: usize,
) -> Result<(InnerSumcheckOutput<F>, Vec<u64>), SumcheckError>
where
    D: GrindingDomain,
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let mut round_boundary =
        ProverGrindingRoundBoundary::<D>::with_round_offset(grinding_bits, round_offset);
    let output = prove_inner_sumcheck_with_reducer_and_round_boundary(
        transcript,
        initial_claim,
        batched_matrix_mle,
        witness_mle,
        field_cfg,
        reducer,
        &mut round_boundary,
    )?;
    Ok((output, round_boundary.nonces))
}

fn prove_inner_sumcheck_with_reducer_and_round_boundary<F, R, P>(
    transcript: &mut impl Transcript,
    initial_claim: F,
    batched_matrix_mle: DenseMultilinearExtension<F>,
    witness_mle: DenseMultilinearExtension<F>,
    field_cfg: &F::Config,
    reducer: &R,
    round_boundary: &mut P,
) -> Result<InnerSumcheckOutput<F>, SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
    P: RoundBoundaryPolicy,
{
    let num_vars = batched_matrix_mle.num_vars;
    if !has_dense_shape(&batched_matrix_mle)
        || !has_dense_shape(&witness_mle)
        || witness_mle.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    round_boundary.validate(num_vars)?;

    let zero = F::zero_with_cfg(field_cfg);
    let mut batched_matrix = batched_matrix_mle.evaluations;
    let mut witness = witness_mle.evaluations;
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    if num_vars > 0 {
        let mut batched_matrix_scratch = vec![zero.clone(); batched_matrix.len() / 2];
        let mut witness_scratch = vec![zero.clone(); witness.len() / 2];
        let mut coefficients_without_linear = sum_inner_round_coefficients_without_linear(
            &batched_matrix,
            &witness,
            reducer,
            &field_cfg,
        )?;

        for _round in 0..num_vars {
            let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
                transcript,
                &mut current_claim,
                &coefficients_without_linear,
                &mut round_polynomials,
                &mut eval_points,
                &zero,
                field_cfg,
                round_boundary,
            )?;

            let next_len = batched_matrix.len() / 2;
            debug_assert_eq!(witness.len() / 2, next_len);
            debug_assert!(batched_matrix_scratch.len() >= next_len);
            debug_assert!(witness_scratch.len() >= next_len);
            batched_matrix_scratch.truncate(next_len);
            witness_scratch.truncate(next_len);

            if next_len == 1 {
                batched_matrix_scratch[0] = interpolate_pair(
                    &batched_matrix[0],
                    &batched_matrix[1],
                    &challenge,
                    &field_cfg,
                );
                witness_scratch[0] =
                    interpolate_pair(&witness[0], &witness[1], &challenge, &field_cfg);
            } else {
                coefficients_without_linear =
                    fold_and_compute_next_inner_round_coefficients_without_linear(
                        &batched_matrix,
                        &witness,
                        &mut batched_matrix_scratch,
                        &mut witness_scratch,
                        &challenge,
                        reducer,
                        &field_cfg,
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
        (field_cfg).mul(&batched_matrix_evaluation, &witness_evaluation)
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
    initial_claim: Fp<2>,
    batched_matrix_mle: DenseMultilinearExtension<Fp<2>>,
    witness_mle: DenseMultilinearExtension<u64>,
    field_cfg: &field::FpCtx<2>,
    reducer: &R,
) -> Result<InnerSumcheckOutput<Fp<2>>, SumcheckError>
where
    R: SumcheckProductReducer<Fp<2>> + SumcheckLinearReducer,
{
    let num_vars = batched_matrix_mle.num_vars;
    if !has_dense_shape(&batched_matrix_mle)
        || !has_dense_shape(&witness_mle)
        || witness_mle.num_vars != num_vars
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let zero = Fp::<2>::zero_with_cfg(field_cfg);
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
            (field_cfg).mul(&batched_matrix_evaluation, &witness_evaluation)
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
        let _scope = tracing::info_span!("spartan:inner_native_coefficients").entered();
        sum_u32_native_inner_coefficients_without_linear(
            &batched_matrix,
            &native_witness,
            &zero,
            reducer,
            field_cfg,
        )?
    };
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
        &zero,
        field_cfg,
    )?;

    let next_len = batched_matrix.len() / 2;
    let mut folded_matrix = vec![zero.clone(); next_len];
    fold_table(&batched_matrix, &mut folded_matrix, &challenge, &field_cfg);
    let mut witness = vec![zero.clone(); next_len];
    {
        let _scope = tracing::info_span!("spartan:inner_native_witness_fold").entered();
        fold_u64_table_to_field(
            &native_witness,
            &mut witness,
            &challenge,
            &zero,
            reducer,
            field_cfg,
        )?;
    }
    batched_matrix = folded_matrix;

    {
        let _scope = tracing::info_span!("spartan:inner_field_rounds").entered();
        let mut coefficients_without_linear = if next_len > 1 {
            sum_inner_round_coefficients_without_linear(
                &batched_matrix,
                &witness,
                reducer,
                field_cfg,
            )?
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
            )?;

            let next_len = batched_matrix.len() / 2;
            batched_matrix_scratch.truncate(next_len);
            witness_scratch.truncate(next_len);

            if next_len == 1 {
                batched_matrix_scratch[0] = interpolate_pair(
                    &batched_matrix[0],
                    &batched_matrix[1],
                    &challenge,
                    &field_cfg,
                );
                witness_scratch[0] =
                    interpolate_pair(&witness[0], &witness[1], &challenge, &field_cfg);
            } else {
                coefficients_without_linear =
                    fold_and_compute_next_inner_round_coefficients_without_linear(
                        &batched_matrix,
                        &witness,
                        &mut batched_matrix_scratch,
                        &mut witness_scratch,
                        &challenge,
                        reducer,
                        field_cfg,
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
        (field_cfg).mul(&batched_matrix_evaluation, &witness_evaluation)
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
    field_config: &F::Config,
) where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    reducer.multiply_accumulate(&mut accumulators[0], matrix_zero, witness_zero);
    reducer.multiply_accumulate(
        &mut accumulators[1],
        &(field_config).sub(matrix_one, matrix_zero),
        &(field_config).sub(witness_one, witness_zero),
    );
}

fn sum_inner_round_coefficients_without_linear<F, R>(
    batched_matrix: &[F],
    witness: &[F],
    reducer: &R,
    field_config: &F::Config,
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
                        &field_config,
                    );
                    accumulators
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        return reduce_two_accumulators(accumulators, reducer, &field_config);
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
                    &field_config,
                );
                accumulators
            },
        );
    reduce_two_accumulators(accumulators, reducer, &field_config)
}

#[inline]
fn fold_inner_chunk<F, R>(
    batched_matrix: &[F],
    witness: &[F],
    batched_matrix_output: &mut [F],
    witness_output: &mut [F],
    challenge: &F,
    reducer: &R,
    field_config: &F::Config,
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
        interpolate_pair(
            &batched_matrix[0],
            &batched_matrix[1],
            challenge,
            &field_config,
        ),
        interpolate_pair(
            &batched_matrix[2],
            &batched_matrix[3],
            challenge,
            &field_config,
        ),
    ];
    let folded_witness = [
        interpolate_pair(&witness[0], &witness[1], challenge, &field_config),
        interpolate_pair(&witness[2], &witness[3], challenge, &field_config),
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
        &field_config,
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
    field_config: &F::Config,
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
                        &field_config,
                    );
                    merge_accumulators(accumulators, contribution, reducer)
                },
            )
            .reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| merge_accumulators(left, right, reducer),
            );
        return reduce_two_accumulators(accumulators, reducer, &field_config);
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
                    &field_config,
                );
                merge_accumulators(accumulators, contribution, reducer)
            },
        );
    reduce_two_accumulators(accumulators, reducer, &field_config)
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
    config: &F::Config,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let [c0, c2] = accumulators;
    Ok([reducer.reduce(c0, config)?, reducer.reduce(c2, config)?])
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
fn native_to_field(value: u64, field_cfg: &field::FpCtx<2>) -> Fp<2> {
    Fp::<2>::from_with_cfg(value, field_cfg)
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
fn reduce_two_linear_accumulators<R>(
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
fn multiply_accumulate_signed_linear<R>(
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

#[inline]
fn accumulate_u32_native_outer_pair<R>(
    accumulators: &mut [<R as SumcheckLinearReducer>::Accumulator; 2],
    products: &R1csProductTableBuffers<u64>,
    index: usize,
    weight: &Fp<2>,
    negative_weight: &Fp<2>,
    endpoint: FactoredEndpoint,
    reducer: &R,
) where
    R: SumcheckLinearReducer,
{
    let az_zero = products.az[index];
    let az_one = products.az[index + 1];
    let bz_zero = products.bz[index];
    let bz_one = products.bz[index + 1];
    let cz_zero = products.cz[index];
    let cz_one = products.cz[index + 1];
    let (az_endpoint, bz_endpoint, cz_endpoint) = match endpoint {
        FactoredEndpoint::Zero => (az_zero, bz_zero, cz_zero),
        FactoredEndpoint::One => (az_one, bz_one, cz_one),
    };
    let endpoint_residual =
        i128::from(native_u32_product(az_endpoint, bz_endpoint)) - i128::from(cz_endpoint);
    let az_delta = i128::from(az_one) - i128::from(az_zero);
    let bz_delta = i128::from(bz_one) - i128::from(bz_zero);
    let infinity = az_delta * bz_delta;

    // `A_endpoint * B_endpoint` and `C_endpoint` are u64, so their signed
    // difference has magnitude at most `u64::MAX`. Each delta magnitude is at
    // most `u32::MAX`, hence the infinity product also fits u64.
    multiply_accumulate_signed_linear(
        &mut accumulators[0],
        weight,
        negative_weight,
        endpoint_residual,
        reducer,
    );
    multiply_accumulate_signed_linear(
        &mut accumulators[1],
        weight,
        negative_weight,
        infinity,
        reducer,
    );
}

#[allow(clippy::too_many_arguments)]
fn compute_u32_native_eq_factored_coefficients_without_linear<R>(
    products: &R1csProductTableBuffers<u64>,
    equality_weights: StrippedEqualityWeights<'_, Fp<2>>,
    current_claim: &Fp<2>,
    tau: &Fp<2>,
    tau_inverse_or_zero: &Fp<2>,
    bound_equality: &Fp<2>,
    one: &Fp<2>,
    zero: &Fp<2>,
    reducer: &R,
    field_config: &crate::piop::spartan::protocol::FieldConfig,
) -> Result<[Fp<2>; 3], SumcheckError>
where
    R: SumcheckLinearReducer + SumcheckProductReducer<Fp<2>>,
{
    let pair_count = products.len() / 2;
    let endpoint = FactoredEndpoint::for_tau(tau);

    // The first round uses the same eq_out * (sum eq_in * residual)
    // decomposition, with exact native u32 products feeding field×u64 inner
    // accumulators and field×field outer accumulators.
    if equality_weights
        .low_weights()
        .is_some_and(|low| low.len() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS)
    {
        let low_weights = equality_weights
            .low_weights()
            .expect("two-level branch requires stripped low weights");
        let negative_low_weights = low_weights
            .iter()
            .map(|weight| (field_config).sub(zero, weight))
            .collect::<Vec<_>>();
        let low_pair_count = low_weights.len();
        let accumulate_high_bucket = |mut outer: [<R as SumcheckProductReducer<Fp<2>>>::Accumulator;
                                          2],
                                      high_index: usize|
         -> Result<_, SumcheckError> {
            let mut inner =
                std::array::from_fn(|_| <R as SumcheckLinearReducer>::accumulator_zero(reducer));
            let product_pair_start = high_index * low_pair_count;
            for (low_pair_index, (weight, negative_weight)) in
                low_weights.iter().zip(&negative_low_weights).enumerate()
            {
                let index = 2 * (product_pair_start + low_pair_index);
                accumulate_u32_native_outer_pair(
                    &mut inner,
                    products,
                    index,
                    weight,
                    negative_weight,
                    endpoint,
                    reducer,
                );
            }

            let inner = reduce_two_linear_accumulators(inner, reducer, &field_config)?;
            let high_weight = &equality_weights.high[high_index];
            for (outer, inner) in outer.iter_mut().zip(&inner) {
                <R as SumcheckProductReducer<Fp<2>>>::multiply_accumulate(
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
            (0..equality_weights.high.len())
                .into_par_iter()
                .try_fold(
                    || {
                        std::array::from_fn(|_| {
                            <R as SumcheckProductReducer<Fp<2>>>::accumulator_zero(reducer)
                        })
                    },
                    accumulate_high_bucket,
                )
                .try_reduce(
                    || {
                        std::array::from_fn(|_| {
                            <R as SumcheckProductReducer<Fp<2>>>::accumulator_zero(reducer)
                        })
                    },
                    |left, right| Ok(merge_accumulators(left, right, reducer)),
                )?
        } else {
            let mut accumulators = std::array::from_fn(|_| {
                <R as SumcheckProductReducer<Fp<2>>>::accumulator_zero(reducer)
            });
            for high_index in 0..equality_weights.high.len() {
                accumulators = accumulate_high_bucket(accumulators, high_index)?;
            }
            accumulators
        };

        #[cfg(not(feature = "parallel"))]
        let accumulators = {
            let mut accumulators = std::array::from_fn(|_| {
                <R as SumcheckProductReducer<Fp<2>>>::accumulator_zero(reducer)
            });
            for high_index in 0..equality_weights.high.len() {
                accumulators = accumulate_high_bucket(accumulators, high_index)?;
            }
            accumulators
        };

        let evaluations = reduce_two_accumulators(accumulators, reducer, &field_config)?;
        return Ok(reconstruct_eq_factored_cubic_without_linear(
            current_claim,
            tau,
            tau_inverse_or_zero,
            endpoint,
            evaluations,
            bound_equality,
            one,
            &field_config,
        ));
    }

    let accumulators = sum_linear_accumulators(
        pair_count,
        |accumulators, pair| {
            let index = 2 * pair;
            let weight = equality_weights.pair_weight(pair, &field_config);
            let negative_weight = (field_config).sub(zero, &weight);
            accumulate_u32_native_outer_pair(
                accumulators,
                products,
                index,
                &weight,
                &negative_weight,
                endpoint,
                reducer,
            );
        },
        reducer,
    );
    let evaluations = reduce_two_linear_accumulators(accumulators, reducer, &field_config)?;
    Ok(reconstruct_eq_factored_cubic_without_linear(
        current_claim,
        tau,
        tau_inverse_or_zero,
        endpoint,
        evaluations,
        bound_equality,
        one,
        &field_config,
    ))
}

fn sum_u32_native_inner_coefficients_without_linear<R>(
    batched_matrix: &[Fp<2>],
    witness: &[u64],
    zero: &Fp<2>,
    reducer: &R,
    field_config: &crate::piop::spartan::protocol::FieldConfig,
) -> Result<[Fp<2>; 2], SumcheckError>
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
            let matrix_delta =
                (field_config).sub(&batched_matrix[index + 1], &batched_matrix[index]);
            let neg_matrix_delta = (field_config).sub(zero, &matrix_delta);

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
        <R as SumcheckLinearReducer>::reduce(reducer, c0, &field_config)?,
        <R as SumcheckLinearReducer>::reduce(reducer, c2, &field_config)?,
    ])
}

fn fold_u64_table_to_field<R>(
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

fn fold_u64_product_tables_to_field<R>(
    input: &R1csProductTableBuffers<u64>,
    challenge: &Fp<2>,
    zero: &Fp<2>,
    field_cfg: &field::FpCtx<2>,
    reducer: &R,
) -> Result<R1csProductTableBuffers<Fp<2>>, SumcheckError>
where
    R: SumcheckLinearReducer,
{
    let mut output = R1csProductTableBuffers::filled(input.len() / 2, zero);
    fold_u64_table_to_field(
        &input.az,
        &mut output.az,
        challenge,
        zero,
        reducer,
        &field_cfg,
    )?;
    fold_u64_table_to_field(
        &input.bz,
        &mut output.bz,
        challenge,
        zero,
        reducer,
        &field_cfg,
    )?;
    fold_u64_table_to_field(
        &input.cz,
        &mut output.cz,
        challenge,
        zero,
        reducer,
        &field_cfg,
    )?;
    Ok(output)
}

#[cfg(feature = "parallel")]
const PARALLEL_SUMCHECK_THRESHOLD: usize = 1 << 12;

#[cfg(feature = "parallel")]
#[inline]
fn should_parallelize(work_items: usize) -> bool {
    work_items >= PARALLEL_SUMCHECK_THRESHOLD && rayon::current_num_threads() > 1
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FactoredEndpoint {
    Zero,
    One,
}

impl FactoredEndpoint {
    #[inline]
    pub(super) fn for_tau<F>(tau: &F) -> Self
    where
        F: SpartanField,
    {
        if F::is_zero(tau) {
            Self::One
        } else {
            Self::Zero
        }
    }
}

/// Inverts every nonzero entry with one field inversion. Zero entries remain
/// zero and use the endpoint-one recovery path below.
pub(super) fn batch_invert_nonzero<F>(values: &[F], field_cfg: &F::Config) -> Vec<F>
where
    F: SpartanField,
{
    field::BatchFieldOps::batch_invert_or_zero_ct(field_cfg, values)
}

#[inline]
pub(super) fn equality_coordinate_evaluation<F>(
    tau: &F,
    point: &F,
    one: &F,
    field_config: &F::Config,
) -> F
where
    F: SpartanField,
{
    let at_zero = (field_config).sub(one, tau);
    let slope = (field_config).sub(tau, &at_zero);
    (field_config).add(&at_zero, &(field_config).mul(point, &slope))
}

/// Converts one endpoint and the leading coefficient of the quadratic
/// cofactor into `[c0, c2, c3]` for the original equality-weighted cubic.
/// The existing round helper restores `c1`, so the proof and transcript remain
/// exactly `[F; 4]`.
pub(super) fn reconstruct_eq_factored_cubic_without_linear<F>(
    current_claim: &F,
    tau: &F,
    tau_inverse_or_zero: &F,
    endpoint: FactoredEndpoint,
    endpoint_and_infinity: [F; 2],
    bound_equality: &F,
    one: &F,
    field_config: &F::Config,
) -> [F; 3]
where
    F: SpartanField,
{
    let [endpoint_evaluation, infinity] = endpoint_and_infinity;
    let endpoint_evaluation = (field_config).mul(bound_equality, &endpoint_evaluation);
    let infinity = (field_config).mul(bound_equality, &infinity);
    let at_zero = (field_config).sub(one, tau);
    let slope = (field_config).sub(tau, &at_zero);

    let (cofactor_zero, cofactor_one) = match endpoint {
        FactoredEndpoint::Zero => {
            let weighted_zero = (field_config).mul(&at_zero, &endpoint_evaluation);
            let cofactor_one = (field_config).mul(
                &(field_config).sub(current_claim, &weighted_zero),
                tau_inverse_or_zero,
            );
            (endpoint_evaluation, cofactor_one)
        }
        FactoredEndpoint::One => (current_claim.clone(), endpoint_evaluation),
    };

    let linear = (field_config).sub(
        &(field_config).sub(&cofactor_one, &cofactor_zero),
        &infinity,
    );
    let c0 = (field_config).mul(&at_zero, &cofactor_zero);
    let c2 = (field_config).add(
        &(field_config).mul(&at_zero, &infinity),
        &(field_config).mul(&slope, &linear),
    );
    let c3 = (field_config).mul(&slope, &infinity);
    [c0, c2, c3]
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn accumulate_eq_factored_cofactor_evaluations<F, R>(
    accumulators: &mut [R::Accumulator; 2],
    weight: &F,
    endpoint: FactoredEndpoint,
    az_zero: &F,
    az_one: &F,
    bz_zero: &F,
    bz_one: &F,
    cz_zero: &F,
    cz_one: &F,
    reducer: &R,
    field_config: &F::Config,
) where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let endpoint_residual = match endpoint {
        FactoredEndpoint::Zero => {
            (field_config).sub(&(field_config).mul(az_zero, bz_zero), cz_zero)
        }
        FactoredEndpoint::One => (field_config).sub(&(field_config).mul(az_one, bz_one), cz_one),
    };
    reducer.multiply_accumulate(&mut accumulators[0], weight, &endpoint_residual);

    let az_delta = (field_config).sub(az_one, az_zero);
    let bz_delta = (field_config).sub(bz_one, bz_zero);
    let infinity = (field_config).mul(&az_delta, &bz_delta);
    reducer.multiply_accumulate(&mut accumulators[1], weight, &infinity);
}

/// Public-modulus constants used to interpolate a cubic from evaluations at
/// 0, 1, 2, and 3. They are prepared once per outer sumcheck, outside every
/// product loop.
#[cfg(test)]
struct CubicInterpolation<F> {
    half: F,
    sixth: F,
}

#[cfg(test)]
impl<F> CubicInterpolation<F>
where
    F: SpartanField,
{
    fn new(field_cfg: &F::Config) -> Self {
        let one = F::one_with_cfg(field_cfg);
        let two = (field_cfg).add(&one, &one);
        let three = (field_cfg).add(&two, &one);
        let six = (field_cfg).add(&three, &three);
        Self {
            half: *field::FieldOps::inverse_ct(&field_cfg, &two).value(),
            sixth: *field::FieldOps::inverse_ct(field_cfg, &six).value(),
        }
    }

    /// Converts `[g(0), g(2), g(3)]` to `[c0, c2, c3]`, deriving
    /// `g(1) = current_claim - g(0)` from the sumcheck relation.
    fn coefficients_without_linear(
        &self,
        current_claim: &F,
        evaluations: [F; 3],
        field_config: &F::Config,
    ) -> [F; 3] {
        let [at_zero, at_two, at_three] = evaluations;
        let at_one = (field_config).sub(current_claim, &at_zero);
        let three_at_one = (field_config).add(&(field_config).add(&at_one, &at_one), &at_one);
        let three_at_two = (field_config).add(&(field_config).add(&at_two, &at_two), &at_two);
        let third_difference = (field_config).sub(
            &(field_config).add(&(field_config).sub(&at_three, &three_at_two), &three_at_one),
            &at_zero,
        );
        let c3 = (field_config).mul(&third_difference, &self.sixth);

        let second_difference = (field_config).add(
            &(field_config).sub(&at_two, &(field_config).add(&at_one, &at_one)),
            &at_zero,
        );
        let three_c3 = (field_config).add(&(field_config).add(&c3, &c3), &c3);
        let c2 = (field_config).sub(
            &(field_config).mul(&second_difference, &self.half),
            &three_c3,
        );
        [at_zero, c2, c3]
    }
}

/// Restores the omitted linear coefficient of a round polynomial.
#[inline]
fn reconstruct_round_coefficients<F, const INPUT_COEFFS: usize, const COEFFS: usize>(
    current_claim: &F,
    coefficients_without_linear: &[F; INPUT_COEFFS],
    zero: &F,
    field_config: &F::Config,
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
            sum = field_config.add(&(sum), &(coefficient));
            sum
        });
    coefficients[1] = (field_config).sub(
        &(field_config).sub(current_claim, &coefficients[0]),
        &at_one_without_c1,
    );
    coefficients
}

#[inline]
fn evaluate_polynomial<F, const COEFFS: usize>(
    coefficients: &[F; COEFFS],
    point: &F,
    zero: &F,
    field_config: &F::Config,
) -> F
where
    F: SpartanField,
{
    coefficients
        .iter()
        .rev()
        .fold(zero.clone(), |value, coefficient| {
            (field_config).add(&(field_config).mul(&value, point), coefficient)
        })
}

/// Completes and records one sumcheck round, then samples its challenge.
pub(super) fn recover_full_round_polynomial_and_sample_next_challenge<
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
) -> Result<F, SumcheckError>
where
    F: SpartanField,
{
    let mut round_boundary = UngrindedRoundBoundary;
    recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
        transcript,
        current_claim,
        coefficients_without_linear,
        round_polynomials,
        eval_points,
        zero,
        field_cfg,
        &mut round_boundary,
    )
}

/// Completes and records one round under an explicit message/challenge
/// boundary policy.
#[allow(clippy::too_many_arguments)]
pub(super) fn recover_full_round_polynomial_and_sample_next_challenge_with_boundary<
    F,
    P,
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
    round_boundary: &mut P,
) -> Result<F, SumcheckError>
where
    F: SpartanField,
    P: RoundBoundaryPolicy,
{
    let coefficients = reconstruct_round_coefficients(
        current_claim,
        coefficients_without_linear,
        zero,
        &field_cfg,
    );
    let at_one = coefficients
        .iter()
        .fold(zero.clone(), |mut sum, coefficient| {
            sum = field_cfg.add(&(sum), &(coefficient));
            sum
        });
    debug_assert_eq!(*current_claim, (field_cfg).add(&coefficients[0], &at_one));

    absorb_field_elements(transcript, &coefficients, &field_cfg);
    round_boundary.after_round(transcript, round_polynomials.len())?;
    let challenge = squeeze_field(transcript, field_cfg)?;
    *current_claim = evaluate_polynomial(&coefficients, &challenge, zero, &field_cfg);
    round_polynomials.push(coefficients);
    eval_points.push(challenge.clone());
    Ok(challenge)
}

/// Allocation-free view of equality weights with the active coordinate
/// stripped once into the existing half-size scratch buffer.
struct StrippedEqualityWeights<'a, F> {
    low: Option<&'a [F]>,
    high: &'a [F],
}

impl<'a, F> StrippedEqualityWeights<'a, F>
where
    F: SpartanField,
{
    fn low(low: &'a [F], high: &'a [F]) -> Self {
        debug_assert!(!low.is_empty());
        debug_assert!(low.len().is_power_of_two());
        debug_assert!(high.len().is_power_of_two());
        Self {
            low: Some(low),
            high,
        }
    }

    fn high(high: &'a [F]) -> Self {
        debug_assert!(!high.is_empty());
        debug_assert!(high.len().is_power_of_two());
        Self { low: None, high }
    }

    /// Returns the suffix equality weight after stripping the active
    /// coordinate. Bound-coordinate equality factors are tracked separately.
    #[inline]
    fn pair_weight(&self, pair: usize, field_config: &F::Config) -> F {
        if let Some(low) = self.low {
            return (field_config).mul(&low[pair % low.len()], &self.high[pair / low.len()]);
        }

        self.high[pair].clone()
    }

    #[inline]
    fn low_weights(&self) -> Option<&'a [F]> {
        self.low
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
pub(super) const TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS: usize = 8;

/// Computes the cofactor endpoint and leading coefficient in two
/// delayed-reduction levels.
///
/// For each `eq_out` (high-factor) bucket, the first level accumulates
/// `eq_in * (A * B - C)` over all low pairs and reduces that subtotal. The
/// second level accumulates `eq_out * subtotal`. At no point is
/// `eq_out * eq_in` formed with an immediate field multiplication.
fn compute_two_level_cofactor_evaluations<F, R>(
    products: &R1csProductTableBuffers<F>,
    equality_weights: &StrippedEqualityWeights<'_, F>,
    endpoint: FactoredEndpoint,
    reducer: &R,
    field_config: &F::Config,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let low_weights = equality_weights
        .low_weights()
        .expect("two-level accumulation requires stripped low weights");
    let low_pair_count = low_weights.len();
    debug_assert_eq!(
        products.len() / 2,
        low_pair_count * equality_weights.high.len()
    );

    let accumulate_high_bucket =
        |mut outer: [R::Accumulator; 2], high_index: usize| -> Result<_, SumcheckError> {
            let mut inner = std::array::from_fn(|_| reducer.accumulator_zero());
            let product_pair_start = high_index * low_pair_count;

            for (low_pair_index, weight) in low_weights.iter().enumerate() {
                let product_index = 2 * (product_pair_start + low_pair_index);
                accumulate_eq_factored_cofactor_evaluations(
                    &mut inner,
                    weight,
                    endpoint,
                    &products.az[product_index],
                    &products.az[product_index + 1],
                    &products.bz[product_index],
                    &products.bz[product_index + 1],
                    &products.cz[product_index],
                    &products.cz[product_index + 1],
                    reducer,
                    &field_config,
                );
            }

            let inner = reduce_two_accumulators(inner, reducer, &field_config)?;
            let high_weight = &equality_weights.high[high_index];
            for (outer, inner) in outer.iter_mut().zip(&inner) {
                reducer.multiply_accumulate(outer, high_weight, inner);
            }
            Ok(outer)
        };

    #[cfg(feature = "parallel")]
    if should_parallelize(products.len() / 2) {
        let accumulators = (0..equality_weights.high.len())
            .into_par_iter()
            .try_fold(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                accumulate_high_bucket,
            )
            .try_reduce(
                || std::array::from_fn(|_| reducer.accumulator_zero()),
                |left, right| Ok(merge_accumulators(left, right, reducer)),
            )?;
        return reduce_two_accumulators(accumulators, reducer, &field_config);
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for high_index in 0..equality_weights.high.len() {
        accumulators = accumulate_high_bucket(accumulators, high_index)?;
    }
    reduce_two_accumulators(accumulators, reducer, &field_config)
}

/// Computes `[c0, c2, c3]` from one endpoint and the leading coefficient of
/// the quadratic cofactor over adjacent pairs in the product tables.
#[allow(clippy::too_many_arguments)]
fn compute_eq_factored_coefficients_without_linear<F, R>(
    products: &R1csProductTableBuffers<F>,
    equality_weights: StrippedEqualityWeights<'_, F>,
    current_claim: &F,
    tau: &F,
    tau_inverse_or_zero: &F,
    bound_equality: &F,
    one: &F,
    reducer: &R,
    field_config: &F::Config,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let pair_count = products.len() / 2;
    let endpoint = FactoredEndpoint::for_tau(tau);
    let evaluations = if equality_weights
        .low_weights()
        .is_some_and(|low| low.len() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS)
    {
        compute_two_level_cofactor_evaluations(
            products,
            &equality_weights,
            endpoint,
            reducer,
            &field_config,
        )?
    } else {
        let accumulators = sum_product_accumulators(
            pair_count,
            |accumulators, pair| {
                let index = 2 * pair;
                let weight = equality_weights.pair_weight(pair, &field_config);
                accumulate_eq_factored_cofactor_evaluations(
                    accumulators,
                    &weight,
                    endpoint,
                    &products.az[index],
                    &products.az[index + 1],
                    &products.bz[index],
                    &products.bz[index + 1],
                    &products.cz[index],
                    &products.cz[index + 1],
                    reducer,
                    &field_config,
                );
            },
            reducer,
        );
        reduce_two_accumulators(accumulators, reducer, &field_config)?
    };
    Ok(reconstruct_eq_factored_cubic_without_linear(
        current_claim,
        tau,
        tau_inverse_or_zero,
        endpoint,
        evaluations,
        bound_equality,
        one,
        &field_config,
    ))
}

#[inline]
fn interpolate_pair<F>(zero: &F, one: &F, challenge: &F, field_config: &F::Config) -> F
where
    F: SpartanField,
{
    (field_config).add(
        zero,
        &(field_config).mul(challenge, &(field_config).sub(one, zero)),
    )
}

#[inline]
fn fold_two_pairs<F>(values: &[F], challenge: &F, field_config: &F::Config) -> [F; 2]
where
    F: SpartanField,
{
    debug_assert_eq!(values.len(), 4);
    [
        interpolate_pair(&values[0], &values[1], challenge, &field_config),
        interpolate_pair(&values[2], &values[3], challenge, &field_config),
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
    field_config: &F::Config,
) -> [[F; 2]; 3]
where
    F: SpartanField,
{
    debug_assert_eq!(az_output.len(), 2);
    debug_assert_eq!(bz_output.len(), 2);
    debug_assert_eq!(cz_output.len(), 2);

    let folded = [
        fold_two_pairs(az, challenge, &field_config),
        fold_two_pairs(bz, challenge, &field_config),
        fold_two_pairs(cz, challenge, &field_config),
    ];
    az_output.clone_from_slice(&folded[0]);
    bz_output.clone_from_slice(&folded[1]);
    cz_output.clone_from_slice(&folded[2]);
    folded
}

/// Folds one evaluation table into preallocated storage.
fn fold_table<F>(input: &[F], output: &mut [F], challenge: &F, field_config: &F::Config)
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

/// Removes the active equality coordinate while leaving all sampled-coordinate
/// factors in the prover's single `bound_equality` scalar.
fn strip_equality_coordinate<F>(input: &[F], output: &mut [F], field_config: &F::Config)
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
                *value = (field_config).add(&pair[0], &pair[1]);
            });
        return;
    }

    input
        .chunks_exact(2)
        .zip(output.iter_mut())
        .for_each(|(pair, value)| {
            *value = (field_config).add(&pair[0], &pair[1]);
        });
}

fn fold_product_tables<F>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    field_config: &F::Config,
) where
    F: SpartanField,
{
    debug_assert_eq!(input.len(), 2 * output.len());
    fold_table(&input.az, &mut output.az, challenge, &field_config);
    fold_table(&input.bz, &mut output.bz, challenge, &field_config);
    fold_table(&input.cz, &mut output.cz, challenge, &field_config);
}

/// Fused product-table fold and two-level equality accumulation for an active
/// low equality table. Each high bucket owns contiguous input/output ranges,
/// which keeps the parallel path allocation-free and race-free.
fn fold_products_and_compute_next_two_level<F, R>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_weights: &StrippedEqualityWeights<'_, F>,
    endpoint: FactoredEndpoint,
    reducer: &R,
    field_config: &F::Config,
) -> Result<[F; 2], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    let low_weights = equality_weights
        .low_weights()
        .expect("two-level accumulation requires stripped low weights");
    let low_pair_count = low_weights.len();
    debug_assert_eq!(
        output.len() / 2,
        low_pair_count * equality_weights.high.len()
    );
    let input_values_per_high = 4 * low_pair_count;
    let output_values_per_high = 2 * low_pair_count;

    let accumulate_high_bucket = |mut outer: [R::Accumulator; 2],
                                  high_index: usize,
                                  az: &[F],
                                  bz: &[F],
                                  cz: &[F],
                                  az_output: &mut [F],
                                  bz_output: &mut [F],
                                  cz_output: &mut [F]|
     -> Result<_, SumcheckError> {
        let mut inner = std::array::from_fn(|_| reducer.accumulator_zero());
        for (low_pair_index, weight) in low_weights.iter().enumerate() {
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
                &field_config,
            );
            accumulate_eq_factored_cofactor_evaluations(
                &mut inner,
                weight,
                endpoint,
                &az[0],
                &az[1],
                &bz[0],
                &bz[1],
                &cz[0],
                &cz[1],
                reducer,
                &field_config,
            );
        }

        let inner = reduce_two_accumulators(inner, reducer, &field_config)?;
        let high_weight = &equality_weights.high[high_index];
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
        return reduce_two_accumulators(accumulators, reducer, &field_config);
    }

    let mut accumulators = std::array::from_fn(|_| reducer.accumulator_zero());
    for high_index in 0..equality_weights.high.len() {
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
    reduce_two_accumulators(accumulators, reducer, &field_config)
}

/// Folds all product tables and accumulates the next round polynomial.
#[allow(clippy::too_many_arguments)]
fn fold_products_and_compute_next<F, R>(
    input: &R1csProductTableBuffers<F>,
    output: &mut R1csProductTableBuffers<F>,
    challenge: &F,
    equality_weights: StrippedEqualityWeights<'_, F>,
    current_claim: &F,
    tau: &F,
    tau_inverse_or_zero: &F,
    bound_equality: &F,
    one: &F,
    reducer: &R,
    field_config: &F::Config,
) -> Result<[F; 3], SumcheckError>
where
    F: SpartanField,
    R: SumcheckProductReducer<F>,
{
    debug_assert_eq!(input.len(), 2 * output.len());
    let endpoint = FactoredEndpoint::for_tau(tau);

    if equality_weights
        .low_weights()
        .is_some_and(|low| low.len() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS)
    {
        let evaluations = fold_products_and_compute_next_two_level(
            input,
            output,
            challenge,
            &equality_weights,
            endpoint,
            reducer,
            &field_config,
        )?;
        return Ok(reconstruct_eq_factored_cubic_without_linear(
            current_claim,
            tau,
            tau_inverse_or_zero,
            endpoint,
            evaluations,
            bound_equality,
            one,
            &field_config,
        ));
    }

    let accumulate = |mut accumulators: [R::Accumulator; 2],
                      chunk: usize,
                      az: &[F],
                      bz: &[F],
                      cz: &[F],
                      az_output: &mut [F],
                      bz_output: &mut [F],
                      cz_output: &mut [F]| {
        let [az, bz, cz] = fold_product_chunk(
            az,
            bz,
            cz,
            az_output,
            bz_output,
            cz_output,
            challenge,
            &field_config,
        );
        let weight = equality_weights.pair_weight(chunk, &field_config);
        accumulate_eq_factored_cofactor_evaluations(
            &mut accumulators,
            &weight,
            endpoint,
            &az[0],
            &az[1],
            &bz[0],
            &bz[1],
            &cz[0],
            &cz[1],
            reducer,
            &field_config,
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
        let evaluations = reduce_two_accumulators(accumulators, reducer, &field_config)?;
        return Ok(reconstruct_eq_factored_cubic_without_linear(
            current_claim,
            tau,
            tau_inverse_or_zero,
            endpoint,
            evaluations,
            bound_equality,
            one,
            &field_config,
        ));
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
    let evaluations = reduce_two_accumulators(accumulators, reducer, &field_config)?;
    Ok(reconstruct_eq_factored_cubic_without_linear(
        current_claim,
        tau,
        tau_inverse_or_zero,
        endpoint,
        evaluations,
        bound_equality,
        one,
        &field_config,
    ))
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
        let two_x_minus_one = (field_cfg).sub(&(field_cfg).add(left_i, left_i), &one);
        let factor = (field_cfg).add(
            &(field_cfg).sub(&one, left_i),
            &(field_cfg).mul(right_i, &two_x_minus_one),
        );
        result = field_cfg.mul(&(result), &(&factor));
    }
    Ok(result)
}

fn has_dense_shape<F>(mle: &DenseMultilinearExtension<F>) -> bool {
    mle.num_vars < usize::BITS as usize && mle.evaluations.len() == 1usize << mle.num_vars
}

fn validate_field_elements<F>(values: &[F], field_cfg: &F::Config) -> Result<(), SumcheckError>
where
    F: SpartanField,
{
    let expected_modulus = F::canonical_modulus_encoding(field_cfg);
    for value in values {
        value
            .validate_element(&expected_modulus)
            .map_err(|_| SumcheckError::NonCanonicalFieldElement)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::{
        piop::spartan::{
            grinding::{derive_grinding_seed, grinding_nonce_is_valid},
            matrix::eq_table,
        },
        transcript::Blake3Transcript,
    };

    use super::*;

    const TEST_MODULUS: u128 = (1_u128 << 100) - 15;

    fn config() -> <Fp<2> as crate::piop::spartan::SpartanField>::Config {
        Fp::<2>::make_cfg(&Uint::from(TEST_MODULUS)).expect("prime test modulus")
    }

    fn field(
        value: u64,
        field_cfg: &<Fp<2> as crate::piop::spartan::SpartanField>::Config,
    ) -> Fp<2> {
        Fp::<2>::from_with_cfg(value, field_cfg)
    }

    enum TestOuterGrinding {}

    impl GrindingDomain for TestOuterGrinding {
        const DOMAIN: &'static [u8] = b"test/spartan/outer-sumcheck-grinding/v1";
    }

    type OuterTestInstance = (
        Fp<2>,
        Vec<Fp<2>>,
        (
            DenseMultilinearExtension<Fp<2>>,
            DenseMultilinearExtension<Fp<2>>,
        ),
        R1csProductMles<Fp<2>>,
    );

    fn outer_test_instance(
        num_vars: usize,
        field_cfg: &<Fp<2> as crate::piop::spartan::SpartanField>::Config,
    ) -> OuterTestInstance {
        let zero = Fp::<2>::zero_with_cfg(field_cfg);
        let table_len = 1_usize << num_vars;
        let products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 + 2, field_cfg))
                    .collect(),
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(3 * index as u64 + 5, field_cfg))
                    .collect(),
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field((index * index) as u64 + 7, field_cfg))
                    .collect(),
                zero,
            ),
        };
        let tau = (0..num_vars)
            .map(|index| field(2 * index as u64 + 2, field_cfg))
            .collect::<Vec<_>>();
        let full_equality = eq_table(&tau, field_cfg).unwrap();
        let mut initial_claim = Fp::<2>::zero_with_cfg(field_cfg);
        for (index, equality) in full_equality.iter().enumerate() {
            let residual = (field_cfg).sub(
                &(field_cfg).mul(
                    &products.az.evaluations[index],
                    &products.bz.evaluations[index],
                ),
                &products.cz.evaluations[index],
            );
            initial_claim =
                field_cfg.add(&(initial_claim), &(&(field_cfg).mul(equality, &residual)));
        }

        let split = num_vars / 2;
        let (low_point, high_point) = tau.split_at(split);
        let equality_factors = (
            DenseMultilinearExtension {
                evaluations: eq_table(low_point, field_cfg).unwrap(),
                num_vars: low_point.len(),
            },
            DenseMultilinearExtension {
                evaluations: eq_table(high_point, field_cfg).unwrap(),
                num_vars: high_point.len(),
            },
        );
        (initial_claim, tau, equality_factors, products)
    }

    #[test]
    fn one_variable_inner_sumcheck_has_the_expected_quadratic_and_replays() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
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
            (field_cfg).mul(
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

        let prover_next = squeeze_field::<Fp<2>, _>(&mut prover_transcript, &field_cfg).unwrap();
        let verifier_next =
            squeeze_field::<Fp<2>, _>(&mut verifier_transcript, &field_cfg).unwrap();
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<Fp<2>, _>(&mut fresh_transcript, &field_cfg).unwrap();
        assert_eq!(prover_next, fresh_next);
        assert_eq!(verifier_next, fresh_next);
    }

    #[test]
    fn zero_variable_outer_sumcheck_proves_and_verifies() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
        let one = Fp::<2>::one_with_cfg(&field_cfg);
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
            &[],
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
                Fp::<2>::zero_with_cfg(&field_cfg),
                &[],
                &field_cfg,
            )
            .unwrap();
        assert!(verified.eval_points.is_empty());
        assert_eq!(verified.az_mle_claim, field(2, &field_cfg));
        assert_eq!(verified.bz_mle_claim, field(3, &field_cfg));
        assert_eq!(verified.cz_mle_claim, field(6, &field_cfg));

        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut prover_transcript, &field_cfg).unwrap(),
            squeeze_field::<Fp<2>, _>(&mut verifier_transcript, &field_cfg).unwrap()
        );
    }

    #[test]
    fn grinded_outer_sumcheck_replays_and_continues_in_lockstep() {
        const NUM_VARS: usize = 4;
        const GRINDING_BITS: u32 = 8;

        let field_cfg = config();
        let (initial_claim, tau, equality_factors, products) =
            outer_test_instance(NUM_VARS, &field_cfg);
        let reducer = field_cfg.clone();
        let mut prover_transcript = Blake3Transcript::new();
        let (output, nonces) =
            prove_outer_sumcheck_with_reducer_grinded::<TestOuterGrinding, _, _>(
                &mut prover_transcript,
                initial_claim.clone(),
                &tau,
                equality_factors,
                products,
                &field_cfg,
                &reducer,
                GRINDING_BITS,
            )
            .unwrap();

        assert_eq!(nonces.len(), NUM_VARS);
        assert_eq!(nonces.len(), output.proof.sumcheck.round_polynomials.len());

        let mut verifier_transcript = Blake3Transcript::new();
        let verified = output
            .proof
            .verify_grinded::<TestOuterGrinding>(
                &mut verifier_transcript,
                initial_claim,
                &tau,
                &field_cfg,
                &nonces,
                GRINDING_BITS,
            )
            .unwrap();

        assert_eq!(verified.eval_points, output.eval_points);
        assert_eq!(verified.az_mle_claim, output.proof.az_mle_claim);
        assert_eq!(verified.bz_mle_claim, output.proof.bz_mle_claim);
        assert_eq!(verified.cz_mle_claim, output.proof.cz_mle_claim);
        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut prover_transcript, &field_cfg).unwrap(),
            squeeze_field::<Fp<2>, _>(&mut verifier_transcript, &field_cfg).unwrap(),
        );
    }

    #[test]
    fn outer_verifier_rejects_malformed_runtime_field_elements_before_absorption() {
        const NUM_VARS: usize = 2;

        let field_cfg = config();
        let other_cfg = Fp::<2>::make_cfg(&Uint::from((1_u128 << 127) - 1)).unwrap();
        let (initial_claim, tau, equality_factors, products) =
            outer_test_instance(NUM_VARS, &field_cfg);
        let output = prove_outer_sumcheck(
            &mut Blake3Transcript::new(),
            initial_claim.clone(),
            &tau,
            equality_factors,
            products,
            &field_cfg,
        )
        .unwrap();

        let mut foreign_round = output.proof.clone();
        foreign_round.sumcheck.round_polynomials[0][0] =
            crate::piop::spartan::noncanonical_test_value(&field_cfg);
        let mut actual = Blake3Transcript::new();
        let mut untouched = actual.clone();
        assert_eq!(
            foreign_round.verify(&mut actual, initial_claim.clone(), &tau, &field_cfg),
            Err(SumcheckError::NonCanonicalFieldElement)
        );
        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut actual, &field_cfg).unwrap(),
            squeeze_field::<Fp<2>, _>(&mut untouched, &field_cfg).unwrap()
        );

        let mut malformed_terminal = output.proof;
        malformed_terminal.az_mle_claim = field::FpCtx::from_prime_u128(u128::MAX - 158)
            .from_montgomery_integer(*field_cfg.modulus());
        let mut actual = Blake3Transcript::new();
        let mut untouched = actual.clone();
        assert_eq!(
            malformed_terminal.verify(&mut actual, initial_claim, &tau, &field_cfg),
            Err(SumcheckError::NonCanonicalFieldElement)
        );
        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut actual, &field_cfg).unwrap(),
            squeeze_field::<Fp<2>, _>(&mut untouched, &field_cfg).unwrap()
        );
    }

    #[test]
    fn grinded_outer_sumcheck_rejects_invalid_and_miscounted_nonces() {
        const NUM_VARS: usize = 3;
        const GRINDING_BITS: u32 = 8;

        let field_cfg = config();
        let (initial_claim, tau, equality_factors, products) =
            outer_test_instance(NUM_VARS, &field_cfg);
        let reducer = field_cfg.clone();
        let mut prover_transcript = Blake3Transcript::new();
        let (output, nonces) =
            prove_outer_sumcheck_with_reducer_grinded::<TestOuterGrinding, _, _>(
                &mut prover_transcript,
                initial_claim.clone(),
                &tau,
                equality_factors,
                products,
                &field_cfg,
                &reducer,
                GRINDING_BITS,
            )
            .unwrap();

        let mut short_transcript = Blake3Transcript::new();
        let mut untouched_transcript = short_transcript.clone();
        assert_eq!(
            output.proof.verify_grinded::<TestOuterGrinding>(
                &mut short_transcript,
                initial_claim.clone(),
                &tau,
                &field_cfg,
                &nonces[..NUM_VARS - 1],
                GRINDING_BITS,
            ),
            Err(SumcheckError::InvalidGrindingNonceCount {
                expected: NUM_VARS,
                actual: NUM_VARS - 1,
            })
        );
        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut short_transcript, &field_cfg).unwrap(),
            squeeze_field::<Fp<2>, _>(&mut untouched_transcript, &field_cfg).unwrap(),
        );

        // Reconstruct the first boundary seed and choose a nonce that is
        // definitely invalid rather than relying on `valid_nonce + 1`.
        let mut seed_transcript = Blake3Transcript::new();
        absorb_field_elements(
            &mut seed_transcript,
            &output.proof.sumcheck.round_polynomials[0],
            &field_cfg,
        );
        let seed = derive_grinding_seed::<TestOuterGrinding, _>(
            &mut seed_transcript,
            GrindingRound::new(0),
            GRINDING_BITS,
        )
        .unwrap();
        let invalid_nonce = (0..=u64::MAX)
            .find(|&nonce| {
                nonce != nonces[0] && !grinding_nonce_is_valid(&seed, nonce, GRINDING_BITS).unwrap()
            })
            .unwrap();
        let mut invalid_nonces = nonces;
        invalid_nonces[0] = invalid_nonce;

        let mut verifier_transcript = Blake3Transcript::new();
        assert_eq!(
            output.proof.verify_grinded::<TestOuterGrinding>(
                &mut verifier_transcript,
                initial_claim,
                &tau,
                &field_cfg,
                &invalid_nonces,
                GRINDING_BITS,
            ),
            Err(SumcheckError::Grinding(GrindingError::InvalidNonce {
                nonce: invalid_nonce,
                bits: GRINDING_BITS,
            }))
        );
    }

    #[test]
    fn sumcheck_rejects_a_tampered_linear_coefficient() {
        let field_cfg = config();
        let proof = SumcheckProof::<Fp<2>, 4> {
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
                Fp::<2>::zero_with_cfg(&field_cfg),
                &[],
                &field_cfg,
            ),
            Err(SumcheckError::InvalidTerminalClaim)
        );
    }

    #[test]
    fn outer_sumcheck_supports_every_equality_factor_split() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
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
            let residual = (field_cfg).sub(
                &(field_cfg).mul(
                    &products.az.evaluations[index],
                    &products.bz.evaluations[index],
                ),
                &products.cz.evaluations[index],
            );
            initial_claim =
                field_cfg.add(&(initial_claim), &(&(field_cfg).mul(equality, &residual)));
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
                &tau,
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
                squeeze_field::<Fp<2>, _>(&mut prover_transcript, &field_cfg).unwrap(),
                squeeze_field::<Fp<2>, _>(&mut verifier_transcript, &field_cfg).unwrap(),
            );
        }
    }

    #[test]
    fn factorized_outer_kernels_match_immediate_proof() {
        fn prove_with_reducer<R>(
            initial_claim: &Fp<2>,
            tau: &[Fp<2>],
            equality_factors: &(
                DenseMultilinearExtension<Fp<2>>,
                DenseMultilinearExtension<Fp<2>>,
            ),
            products: &R1csProductMles<Fp<2>>,
            field_cfg: &<Fp<2> as crate::piop::spartan::SpartanField>::Config,
            reducer: &R,
        ) -> (OuterSumcheckOutput<Fp<2>>, Fp<2>)
        where
            R: SumcheckProductReducer<Fp<2>>,
        {
            let mut transcript = Blake3Transcript::new();
            let output = prove_outer_sumcheck_with_reducer(
                &mut transcript,
                initial_claim.clone(),
                tau,
                equality_factors.clone(),
                products.clone(),
                field_cfg,
                reducer,
            )
            .unwrap();
            let continuation = squeeze_field(&mut transcript, field_cfg).unwrap();
            (output, continuation)
        }

        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
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
            let residual = (field_cfg).sub(
                &(field_cfg).mul(
                    &products.az.evaluations[index],
                    &products.bz.evaluations[index],
                ),
                &products.cz.evaluations[index],
            );
            initial_claim =
                field_cfg.add(&(initial_claim), &(&(field_cfg).mul(equality, &residual)));
        }

        let immediate = field_cfg.clone();
        let optimized = crate::utils::delayed_reduction::prepare_field(&field_cfg).unwrap();
        let reference = BigUintSumcheckOracle::new(&field_cfg).unwrap();
        let mut direct_transcript = Blake3Transcript::new();
        let direct_output = prove_outer_sumcheck_direct_reference(
            &mut direct_transcript,
            initial_claim.clone(),
            &tau,
            products.clone(),
            &field_cfg,
        )
        .unwrap();
        let direct_continuation = squeeze_field(&mut direct_transcript, &field_cfg).unwrap();
        let (immediate_output, immediate_continuation) = prove_with_reducer(
            &initial_claim,
            &tau,
            &equality_factors,
            &products,
            &field_cfg,
            &immediate,
        );
        let (optimized_output, optimized_continuation) = prove_with_reducer(
            &initial_claim,
            &tau,
            &equality_factors,
            &products,
            &field_cfg,
            &optimized,
        );
        let (reference_output, reference_continuation) = prove_with_reducer(
            &initial_claim,
            &tau,
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
        assert_eq!(immediate_output.proof, direct_output.proof);
        assert_eq!(immediate_output.eval_points, direct_output.eval_points);
        assert_eq!(immediate_output.final_claim, direct_output.final_claim);
        assert_eq!(immediate_continuation, direct_continuation);

        let mut verifier_transcript = Blake3Transcript::new();
        let verified = immediate_output
            .proof
            .verify(&mut verifier_transcript, initial_claim, &tau, &field_cfg)
            .unwrap();
        assert_eq!(verified.eval_points, immediate_output.eval_points);
        assert_eq!(
            squeeze_field::<Fp<2>, _>(&mut verifier_transcript, &field_cfg).unwrap(),
            immediate_continuation
        );
    }

    #[test]
    fn eq_factored_outer_is_direct_cubic_exact_at_tau_edges() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
        let one = Fp::<2>::one_with_cfg(&field_cfg);
        let two = (field_cfg).add(&one, &one);
        let half = *field::FieldOps::inverse_ct(&field_cfg, &two).value();
        let num_vars = 3;
        let table_len = 1 << num_vars;
        let products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 * 5 + 2, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 * 7 + 3, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                (0..table_len)
                    .map(|index| field(index as u64 * 11 + 1, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
        };

        let tau_cases = [
            vec![zero.clone(), zero.clone(), zero.clone()],
            vec![one.clone(), one.clone(), one.clone()],
            vec![zero.clone(), one.clone(), half],
            vec![
                field(2, &field_cfg),
                field(5, &field_cfg),
                field(9, &field_cfg),
            ],
        ];

        for tau in tau_cases {
            let equality = eq_table(&tau, &field_cfg).unwrap();
            let initial_claim =
                equality
                    .iter()
                    .enumerate()
                    .fold(zero.clone(), |mut claim, (index, weight)| {
                        claim = field_cfg.add(
                            &(claim),
                            &(&(field_cfg).mul(
                                weight,
                                &(field_cfg).sub(
                                    &(field_cfg).mul(
                                        &products.az.evaluations[index],
                                        &products.bz.evaluations[index],
                                    ),
                                    &products.cz.evaluations[index],
                                ),
                            )),
                        );
                        claim
                    });

            let mut direct_transcript = Blake3Transcript::new();
            let direct = prove_outer_sumcheck_direct_reference(
                &mut direct_transcript,
                initial_claim.clone(),
                &tau,
                products.clone(),
                &field_cfg,
            )
            .unwrap();
            let direct_continuation =
                squeeze_field::<Fp<2>, _>(&mut direct_transcript, &field_cfg).unwrap();

            for split in 0..=num_vars {
                let (low_tau, high_tau) = tau.split_at(split);
                let equality_factors = (
                    DenseMultilinearExtension {
                        evaluations: eq_table(low_tau, &field_cfg).unwrap(),
                        num_vars: low_tau.len(),
                    },
                    DenseMultilinearExtension {
                        evaluations: eq_table(high_tau, &field_cfg).unwrap(),
                        num_vars: high_tau.len(),
                    },
                );
                let mut transcript = Blake3Transcript::new();
                let output = prove_outer_sumcheck(
                    &mut transcript,
                    initial_claim.clone(),
                    &tau,
                    equality_factors,
                    products.clone(),
                    &field_cfg,
                )
                .unwrap();
                let continuation = squeeze_field::<Fp<2>, _>(&mut transcript, &field_cfg).unwrap();

                assert_eq!(output.proof, direct.proof);
                assert_eq!(output.eval_points, direct.eval_points);
                assert_eq!(output.final_claim, direct.final_claim);
                assert_eq!(continuation, direct_continuation);
            }
        }
    }

    #[test]
    fn native_u32_eq_factoring_matches_direct_cubic_at_signed_extremes() {
        fn prove_with_native_reducer<R>(
            initial_claim: &Fp<2>,
            tau: &[Fp<2>],
            equality_factors: &(
                DenseMultilinearExtension<Fp<2>>,
                DenseMultilinearExtension<Fp<2>>,
            ),
            products: &R1csProductMles<u64>,
            field_cfg: &<Fp<2> as crate::piop::spartan::SpartanField>::Config,
            reducer: &R,
        ) -> (OuterSumcheckOutput<Fp<2>>, Fp<2>)
        where
            R: SumcheckProductReducer<Fp<2>> + SumcheckLinearReducer,
        {
            let mut transcript = Blake3Transcript::new();
            let output = prove_outer_sumcheck_u32_native_with_reducer(
                &mut transcript,
                initial_claim.clone(),
                tau,
                equality_factors.clone(),
                products.clone(),
                field_cfg,
                reducer,
            )
            .unwrap();
            let continuation = squeeze_field::<Fp<2>, _>(&mut transcript, field_cfg).unwrap();
            (output, continuation)
        }

        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
        let one = Fp::<2>::one_with_cfg(&field_cfg);
        let num_vars = 2;
        let max_u32 = u64::from(u32::MAX);
        let native_products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                vec![0, max_u32, max_u32, 0],
                0,
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                vec![0, max_u32, 0, max_u32],
                0,
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                vec![u64::MAX, 0, 0, u64::MAX],
                0,
            ),
        };
        let field_products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                native_products
                    .az
                    .evaluations
                    .iter()
                    .map(|&value| field(value, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                native_products
                    .bz
                    .evaluations
                    .iter()
                    .map(|&value| field(value, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                native_products
                    .cz
                    .evaluations
                    .iter()
                    .map(|&value| field(value, &field_cfg))
                    .collect(),
                zero.clone(),
            ),
        };
        let tau_cases = [
            vec![zero.clone(), one.clone()],
            vec![one.clone(), zero.clone()],
            vec![field(7, &field_cfg), field(11, &field_cfg)],
        ];
        let optimized = crate::utils::delayed_reduction::prepare_field(&field_cfg).unwrap();
        let crypto_bigint = BigUintSumcheckOracle::new(&field_cfg).unwrap();

        for tau in tau_cases {
            let equality = eq_table(&tau, &field_cfg).unwrap();
            let initial_claim =
                equality
                    .iter()
                    .enumerate()
                    .fold(zero.clone(), |mut claim, (index, weight)| {
                        claim = field_cfg.add(
                            &(claim),
                            &(&(field_cfg).mul(
                                weight,
                                &(field_cfg).sub(
                                    &(field_cfg).mul(
                                        &field_products.az.evaluations[index],
                                        &field_products.bz.evaluations[index],
                                    ),
                                    &field_products.cz.evaluations[index],
                                ),
                            )),
                        );
                        claim
                    });
            let mut direct_transcript = Blake3Transcript::new();
            let direct = prove_outer_sumcheck_direct_reference(
                &mut direct_transcript,
                initial_claim.clone(),
                &tau,
                field_products.clone(),
                &field_cfg,
            )
            .unwrap();
            let direct_continuation =
                squeeze_field::<Fp<2>, _>(&mut direct_transcript, &field_cfg).unwrap();

            for split in 0..=num_vars {
                let (low_tau, high_tau) = tau.split_at(split);
                let equality_factors = (
                    DenseMultilinearExtension {
                        evaluations: eq_table(low_tau, &field_cfg).unwrap(),
                        num_vars: low_tau.len(),
                    },
                    DenseMultilinearExtension {
                        evaluations: eq_table(high_tau, &field_cfg).unwrap(),
                        num_vars: high_tau.len(),
                    },
                );

                let optimized_result = prove_with_native_reducer(
                    &initial_claim,
                    &tau,
                    &equality_factors,
                    &native_products,
                    &field_cfg,
                    &optimized,
                );
                let crypto_bigint_result = prove_with_native_reducer(
                    &initial_claim,
                    &tau,
                    &equality_factors,
                    &native_products,
                    &field_cfg,
                    &crypto_bigint,
                );
                for (output, continuation) in [optimized_result, crypto_bigint_result] {
                    assert_eq!(output.proof, direct.proof);
                    assert_eq!(output.eval_points, direct.eval_points);
                    assert_eq!(output.final_claim, direct.final_claim);
                    assert_eq!(continuation, direct_continuation);
                }
            }
        }
    }

    #[test]
    fn inner_sumcheck_folds_the_lowest_coordinate_first() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
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
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
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

        let rejected_next = squeeze_field::<Fp<2>, _>(&mut transcript, &field_cfg).unwrap();
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<Fp<2>, _>(&mut fresh_transcript, &field_cfg).unwrap();
        assert_eq!(rejected_next, fresh_next);
    }

    #[test]
    fn outer_sumcheck_rejects_malformed_dimensions_before_absorption() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
        let one = Fp::<2>::one_with_cfg(&field_cfg);
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
                &[],
                equality_factors,
                products,
                &field_cfg,
            ),
            Err(SumcheckError::InvalidProductDimensions)
        );

        let rejected_next = squeeze_field::<Fp<2>, _>(&mut transcript, &field_cfg).unwrap();
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<Fp<2>, _>(&mut fresh_transcript, &field_cfg).unwrap();
        assert_eq!(rejected_next, fresh_next);
    }

    #[test]
    fn outer_sumcheck_rejects_tau_length_before_absorption() {
        let field_cfg = config();
        let zero = Fp::<2>::zero_with_cfg(&field_cfg);
        let one = Fp::<2>::one_with_cfg(&field_cfg);
        let products = R1csProductMles {
            az: DenseMultilinearExtension::from_evaluations_vec(
                1,
                vec![field(1, &field_cfg), field(2, &field_cfg)],
                zero.clone(),
            ),
            bz: DenseMultilinearExtension::from_evaluations_vec(
                1,
                vec![field(3, &field_cfg), field(4, &field_cfg)],
                zero.clone(),
            ),
            cz: DenseMultilinearExtension::from_evaluations_vec(
                1,
                vec![field(5, &field_cfg), field(6, &field_cfg)],
                zero.clone(),
            ),
        };
        let equality_factors = (
            DenseMultilinearExtension::zero_vars(one),
            DenseMultilinearExtension {
                evaluations: vec![field(7, &field_cfg), field(8, &field_cfg)],
                num_vars: 1,
            },
        );
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            prove_outer_sumcheck(
                &mut transcript,
                zero,
                &[],
                equality_factors,
                products,
                &field_cfg,
            ),
            Err(SumcheckError::InvalidEqualityDimensions)
        );

        let rejected_next = squeeze_field::<Fp<2>, _>(&mut transcript, &field_cfg).unwrap();
        let mut fresh_transcript = Blake3Transcript::new();
        let fresh_next = squeeze_field::<Fp<2>, _>(&mut fresh_transcript, &field_cfg).unwrap();
        assert_eq!(rejected_next, fresh_next);
    }
}
