//! Ordinary degree-two inner products, with native inputs and shared-challenge batches.
//! Optimized borrowed, packed, and factored integrations use the same round engine.
pub(crate) mod binary;
mod dense;
pub(crate) mod engine;
pub mod evaluation_form;
pub(crate) mod native;
pub(crate) mod packed;
#[cfg(test)]
pub(crate) mod reference;

use crate::piop::spartan::{SpartanField, absorb_field_elements, squeeze_field};
use crate::sumcheck::{RoundBoundaryPolicy, SumcheckError, SumcheckProof};
use crate::transcript::traits::Transcript;
use field::{BatchMulAcc, FieldOps, PreparedLinearCombination, Reduce, RingOps};

type Elem<F> = <F as RingOps>::Elem;
type Acc<F, T> = <F as BatchMulAcc<Elem<F>, T>>::Accumulator;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InnerSumcheckOutput<E> {
    pub proof: SumcheckProof<E, 3>,
    pub point: Vec<E>,
    pub final_claim: E,
    /// [weight(point), value(point)].
    pub terminal_evaluations: [E; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchedInnerSumcheckOutput<E, const K: usize> {
    pub proofs: [SumcheckProof<E, 3>; K],
    pub point: Vec<E>,
    pub final_claims: [E; K],
    pub terminal_evaluations: [[E; 2]; K],
}

/// Prove s = Σ_i weights[i] * values[i], folding the low Boolean coordinate first.
/// Original integer values are embedded only inside their consuming operations.
pub fn prove_inner_sumcheck<F, T>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claim: Elem<F>,
    values: Vec<T>,
    weights: Vec<Elem<F>>,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<InnerSumcheckOutput<Elem<F>>, SumcheckError>
where
    F: FieldOps
        + PreparedLinearCombination<T>
        + BatchMulAcc<Elem<F>, T>
        + BatchMulAcc<Elem<F>>
        + Sync,
    F: Reduce<Acc<F, T>, Output = Elem<F>> + Reduce<Acc<F, Elem<F>>, Output = Elem<F>>,
    Elem<F>: SpartanField<Config = F>,
    T: Copy + Send + Sync,
{
    let output = prove_batched_inner_sumcheck(
        field,
        transcript,
        &[initial_claim],
        [values],
        [weights],
        boundary,
    )?;
    let [proof] = output.proofs;
    Ok(InnerSumcheckOutput {
        proof,
        point: output.point,
        final_claim: output.final_claims[0],
        terminal_evaluations: output.terminal_evaluations[0],
    })
}

/// Prove K separate dot products with a common challenge per round. No random
/// linear combination is introduced; every initial claim remains independent.
pub fn prove_batched_inner_sumcheck<F, T, const K: usize>(
    field: &F,
    transcript: &mut impl Transcript,
    initial_claims: &[Elem<F>; K],
    values: [Vec<T>; K],
    weights: [Vec<Elem<F>>; K],
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<BatchedInnerSumcheckOutput<Elem<F>, K>, SumcheckError>
where
    F: FieldOps
        + PreparedLinearCombination<T>
        + BatchMulAcc<Elem<F>, T>
        + BatchMulAcc<Elem<F>>
        + Sync,
    F: Reduce<Acc<F, T>, Output = Elem<F>> + Reduce<Acc<F, Elem<F>>, Output = Elem<F>>,
    Elem<F>: SpartanField<Config = F>,
    T: Copy + Send + Sync,
{
    if K == 0 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    let len = values[0].len();
    if !len.is_power_of_two()
        || values.iter().any(|v| v.len() != len)
        || weights.iter().any(|w| w.len() != len)
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    let rounds = len.ilog2() as usize;
    boundary.validate(rounds)?;
    let mut inputs = values.into_iter().zip(weights);
    let mut tables = core::array::from_fn::<_, K, _>(|_| {
        let (v, w) = inputs.next().unwrap();
        dense::Tables::new(v, w)
    });
    let mut coefficients = core::array::from_fn::<_, K, _>(|i| {
        if rounds == 0 {
            [field.zero(); 2]
        } else {
            dense::first_round(field, &tables[i])
        }
    });
    let mut claims = *initial_claims;
    let mut proofs = core::array::from_fn::<_, K, _>(|_| SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds),
    });
    let mut point = Vec::with_capacity(rounds);
    engine::drive(
        field,
        transcript,
        rounds,
        &mut point,
        &mut claims,
        &mut coefficients,
        &mut [[field.zero(); 3]; K],
        |transcript, round, messages| {
            for (proof, message) in proofs.iter_mut().zip(messages) {
                absorb_field_elements(transcript, message, field);
                proof.round_polynomials.push(*message);
            }
            boundary.after_round(transcript, round)?;
            squeeze_field(transcript, field)
        },
        |_, challenge, next| {
            for (tables, coefficients) in tables.iter_mut().zip(next) {
                *coefficients = dense::fold_round(field, tables, challenge);
            }
            Ok(())
        },
    )?;
    let terminal_evaluations = core::array::from_fn(|i| tables[i].terminal(field));
    for (claim, &[w, v]) in claims.iter().zip(&terminal_evaluations) {
        if *claim != field.mul(&w, &v) {
            return Err(SumcheckError::InvalidTerminalClaim);
        }
    }
    Ok(BatchedInnerSumcheckOutput {
        proofs,
        point,
        final_claims: claims,
        terminal_evaluations,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod skipped_experiment;
