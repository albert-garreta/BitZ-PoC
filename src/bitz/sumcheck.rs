//! Their `pcs::sumcheck`: the dense degree-2 sumcheck reducing a factored
//! inner-product claim over the committed bits to one MLE claim. Rounds
//! bind the lowest remaining coordinate (rows before columns) and send the
//! three coefficients `[g(0), g(0)+g(1)+g_2, g_2]`; the terminal message is
//! the witness evaluation.

use super::params::LinearClaimGf;
use super::pcs::{ProveError, VerifyError};
use super::transcript::{ProverState, VerifierState};
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// A pending evaluation claim over the original committed bit polynomial.
pub(crate) struct MleClaim {
    /// Row coordinates before column coordinates, low-bit-first.
    pub(crate) point: Vec<Gf>,
    /// The witness MLE evaluation at `point`.
    pub(crate) target: Gf,
}

/// One `Gf` per committed bit, bit `column * rows + row`, from the
/// per-column rows (64 bits per word, low bit first).
pub(crate) fn dense_witness(rows: &[Vec<u64>]) -> Vec<Gf> {
    let mut witness = Vec::with_capacity(rows.len() * rows.first().map_or(0, |r| r.len() * 64));
    for row in rows {
        for &word in row {
            for bit in 0..64 {
                witness.push(Gf::from(u128::from((word >> bit) & 1)));
            }
        }
    }
    witness
}

pub(crate) fn prove(
    claim: &LinearClaimGf,
    rows_bits: &[Vec<u64>],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let mut rows = claim.row_weights().to_vec();
    let mut columns = claim.column_weights().to_vec();
    let bit_len = rows.len() * columns.len();
    let mut witness = dense_witness(rows_bits);
    if witness.len() != bit_len {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }

    let mut target = claim.target();
    let mut point = Vec::with_capacity(bit_len.ilog2() as usize);
    while witness.len() > 1 {
        let coefficients = round_polynomial(&witness, &rows, &columns);
        // In characteristic two, g(0) + g(1) = a1 + a2.
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.verifier_message::<Gf>();
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut witness, challenge);
        if rows.len() > 1 {
            fold(&mut rows, challenge);
        } else {
            fold(&mut columns, challenge);
        }
    }

    let evaluation = witness[0];
    if target != evaluation * rows[0] * columns[0] {
        return Err(ProveError::InvalidClaim);
    }
    transcript.prover_message(&evaluation);
    Ok(MleClaim {
        point,
        target: evaluation,
    })
}

pub(crate) fn verify(
    claim: &LinearClaimGf,
    transcript: &mut VerifierState<'_>,
) -> Result<MleClaim, VerifyError> {
    let mut rows = claim.row_weights().to_vec();
    let mut columns = claim.column_weights().to_vec();
    let rounds = rows.len().ilog2() as usize + columns.len().ilog2() as usize;
    let mut point = Vec::with_capacity(rounds);
    let mut target = claim.target();
    for _ in 0..rounds {
        let coefficients = transcript
            .prover_message::<[Gf; 3]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        if coefficients[1] + coefficients[2] != target {
            return Err(VerifyError::VerificationFailed);
        }
        let challenge = transcript.verifier_message::<Gf>();
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        if rows.len() > 1 {
            fold(&mut rows, challenge);
        } else {
            fold(&mut columns, challenge);
        }
    }
    let evaluation = transcript
        .prover_message::<Gf>()
        .map_err(|_| VerifyError::MalformedProof)?;
    if target != evaluation * rows[0] * columns[0] {
        return Err(VerifyError::VerificationFailed);
    }
    Ok(MleClaim {
        point,
        target: evaluation,
    })
}

fn round_polynomial(witness: &[Gf], rows: &[Gf], columns: &[Gf]) -> [Gf; 3] {
    let mut at_zero = Gf::zero();
    let mut at_one = Gf::zero();
    let mut quadratic = Gf::zero();
    for (pair_index, pair) in witness.chunks_exact(2).enumerate() {
        let index = 2 * pair_index;
        let weight_zero = rows[index % rows.len()] * columns[index / rows.len()];
        let weight_one = rows[(index + 1) % rows.len()] * columns[(index + 1) / rows.len()];
        at_zero += pair[0] * weight_zero;
        at_one += pair[1] * weight_one;
        quadratic += (pair[0] + pair[1]) * (weight_zero + weight_one);
    }
    [at_zero, at_zero + at_one + quadratic, quadratic]
}

fn evaluate_round([constant, linear, quadratic]: [Gf; 3], challenge: Gf) -> Gf {
    constant + challenge * (linear + challenge * quadratic)
}

/// Fixes the lowest remaining coordinate.
fn fold(values: &mut Vec<Gf>, challenge: Gf) {
    let len = values.len() / 2;
    for index in 0..len {
        let zero = values[2 * index];
        let one = values[2 * index + 1];
        values[index] = zero + challenge * (zero + one);
    }
    values.truncate(len);
}
