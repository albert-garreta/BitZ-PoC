//! Their `pcs::sumcheck`: the degree-2 sumcheck reducing a factored
//! inner-product claim over the committed bits to one MLE claim. Rounds
//! bind the lowest remaining coordinate (rows before columns) and send the
//! coefficients `[a0, a1, a2]`; the terminal message is the witness
//! evaluation. The prover runs it on the two small tables the bilinear
//! structure gives (see [`prove`]); the verifier replays it their way.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::params::LinearClaimGf;
use super::pcs::{ProveError, VerifyError};
use super::transcript::{ProverState, VerifierState};
use crate::cfg_into_iter;
use crate::ligerito::{xi_combined_rows, xi_combined_rows_packed};
use crate::ligerito_flock::FlockCommitHint;
use crate::pcs::IntegerMatrixLayout;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;
use crate::utils::wide_mul::WideMulAcc;

const PARALLEL_MIN_PAIRS: usize = 1 << 12;

/// A pending evaluation claim over the original committed bit polynomial.
pub(crate) struct MleClaim {
    /// Row coordinates before column coordinates, low-bit-first.
    pub(crate) point: Vec<Gf>,
    /// The witness MLE evaluation at `point`.
    pub(crate) target: Gf,
}

/// One `Gf` per committed bit, bit `column * rows + row`, from the
/// per-column rows (64 bits per word, low bit first).
/// `Σ_b eq[b]·bit(c, b)` for every column `c`: the witness after its row
/// coordinates are bound to the point `eq` tabulates. A nibble table over
/// `eq` turns the per-bit adds into one table add per set nibble.
fn fold_rows_eq(rows: &[Vec<u64>], eq: &[Gf]) -> Vec<Gf> {
    let row_len = eq.len();
    // 2^t · 64 B of tables; past 2^22 rows fall back to per-bit adds.
    if row_len <= 1 << 22 {
        let groups = row_len / 4;
        let table: Vec<[Gf; 16]> = cfg_into_iter!(0..groups, 1 << 10)
            .map(|g| {
                let mut t = [Gf::zero(); 16];
                for nib in 1..16usize {
                    t[nib] = t[nib & (nib - 1)] + eq[(g << 2) | nib.trailing_zeros() as usize];
                }
                t
            })
            .collect();
        cfg_into_iter!(rows, 16)
            .map(|row| {
                let mut acc = Gf::zero();
                for (wi, &word) in row.iter().enumerate() {
                    let mut x = word;
                    let mut nib = 0usize;
                    while x != 0 {
                        let n = (x & 0xF) as usize;
                        if n != 0 {
                            acc += table[(wi << 4) | nib][n];
                        }
                        x >>= 4;
                        nib += 1;
                    }
                }
                acc
            })
            .collect()
    } else {
        cfg_into_iter!(rows, 16)
            .map(|row| {
                let mut acc = Gf::zero();
                for (wi, &word) in row.iter().enumerate() {
                    let mut x = word;
                    while x != 0 {
                        acc += eq[(wi << 6) | x.trailing_zeros() as usize];
                        x &= x - 1;
                    }
                }
                acc
            })
            .collect()
    }
}

/// Their `prove`, computed through the bilinear structure of the claim.
///
/// The weight of bit `(b, c)` is `row_weights[b]·column_weights[c]`, and
/// the rounds bind the row coordinates first. While a round binds a row
/// coordinate the column coordinates are only summed over, so its
/// polynomial is that of the `2^t`-entry table `M̃[b] = Σ_c w2[c]·bit(c, b)`
/// against `row_weights` alone; once the rows are bound the remaining
/// rounds run on the `2^s`-entry table `M'[c] = Σ_b eq(b, r_b)·bit(c, b)`
/// against `w1(r_b)·column_weights`. Every coefficient is the one the dense
/// pass over `2^m` bits produces — the two sums are the same field sum
/// regrouped — so the messages are byte-identical, without a 16-byte-per-bit
/// witness.
pub(crate) fn prove(
    claim: &LinearClaimGf,
    hint: &FlockCommitHint,
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let w1 = claim.row_weights();
    let w2 = claim.column_weights();
    let rows_bits = hint.rows();
    if rows_bits.len() != w2.len() || rows_bits.iter().any(|row| row.len() * 64 != w1.len()) {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }
    let layout = IntegerMatrixLayout {
        row_vars: w1.len().ilog2() as usize,
        col_vars: w2.len().ilog2() as usize,
        word_bits: 1,
    };
    let mut target = claim.target();
    let mut point = Vec::with_capacity(layout.row_vars + layout.col_vars);

    // Rows: the column-combined table against the row weights.
    let mut combined = if hint.packed_cols().is_empty() {
        xi_combined_rows(&layout, rows_bits, w2)
    } else {
        xi_combined_rows_packed(&layout, hint.packed_cols(), w2)
    };
    let mut rows = w1.to_vec();
    while combined.len() > 1 {
        let coefficients = round_polynomial(&combined, &rows);
        // In characteristic two, g(0) + g(1) = a1 + a2.
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.verifier_message::<Gf>();
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut combined, challenge);
        fold(&mut rows, challenge);
    }

    // Columns: the row-folded table against `w1(r_b)·column_weights`.
    let eq_rows = if point.is_empty() {
        vec![Gf::one()]
    } else {
        build_eq_x_r_vec(&point, &()).expect("non-empty row point")
    };
    let mut folded = fold_rows_eq(rows_bits, &eq_rows);
    let row_scalar = rows[0];
    let mut columns: Vec<Gf> = w2.iter().map(|&w| row_scalar * w).collect();
    while folded.len() > 1 {
        let coefficients = round_polynomial(&folded, &columns);
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.verifier_message::<Gf>();
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut folded, challenge);
        fold(&mut columns, challenge);
    }

    let evaluation = folded[0];
    if target != evaluation * columns[0] {
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

/// The coefficients `[a0, a1, a2]` of the round polynomial over pairs
/// `(2k, 2k+1)` of `witness` against the per-element `weights`, computed
/// their way (`a1 = Σ(w0·M0 + w1·M1) + a0 + a2` in characteristic two).
fn round_polynomial(witness: &[Gf], weights: &[Gf]) -> [Gf; 3] {
    let pairs = witness.len() / 2;
    let chunks = pairs.div_ceil(PARALLEL_MIN_PAIRS).max(1);
    let partials: Vec<[Gf; 3]> = cfg_into_iter!(0..chunks)
        .map(|chunk| {
            let start = chunk * PARALLEL_MIN_PAIRS;
            let end = (start + PARALLEL_MIN_PAIRS).min(pairs);
            let zero = Gf::zero();
            let mut at_zero = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut at_one = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut quadratic = <Gf as WideMulAcc>::wide_zero(&zero);
            for pair_index in start..end {
                let index = 2 * pair_index;
                let (m0, m1) = (witness[index], witness[index + 1]);
                let (w0, w1) = (weights[index], weights[index + 1]);
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut at_zero,
                    &<Gf as WideMulAcc>::mul_wide(&m0, &w0),
                );
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut at_one,
                    &<Gf as WideMulAcc>::mul_wide(&m1, &w1),
                );
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut quadratic,
                    &<Gf as WideMulAcc>::mul_wide(&(m0 + m1), &(w0 + w1)),
                );
            }
            [
                <Gf as WideMulAcc>::from_wide(at_zero),
                <Gf as WideMulAcc>::from_wide(at_one),
                <Gf as WideMulAcc>::from_wide(quadratic),
            ]
        })
        .collect();
    let [at_zero, at_one, quadratic] = partials.into_iter().fold(
        [Gf::zero(); 3],
        |[a, b, c], [x, y, z]| [a + x, b + y, c + z],
    );
    [at_zero, at_zero + at_one + quadratic, quadratic]
}

fn evaluate_round([constant, linear, quadratic]: [Gf; 3], challenge: Gf) -> Gf {
    constant + challenge * (linear + challenge * quadratic)
}

/// Fixes the lowest remaining coordinate: `v'[i] = v[2i] + r·(v[2i] + v[2i+1])`.
fn fold(values: &mut Vec<Gf>, challenge: Gf) {
    let len = values.len() / 2;
    let folded: Vec<Gf> = cfg_into_iter!(0..len, PARALLEL_MIN_PAIRS)
        .map(|index| {
            let zero = values[2 * index];
            let one = values[2 * index + 1];
            zero + challenge * (zero + one)
        })
        .collect();
    *values = folded;
}
