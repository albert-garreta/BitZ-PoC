//! Their `pcs::sumcheck`: the degree-2 sumcheck reducing a factored
//! inner-product claim over the committed bits to one MLE claim. Rounds
//! bind the lowest remaining coordinate (rows before columns) and send the
//! coefficients `[a0, a1, a2]`; the terminal message is the witness
//! evaluation. The prover runs it on the two small tables the bilinear
//! structure gives (see [`prove`]); the verifier replays it their way.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::params::{LinearClaimGf, SumClaimGf};
use super::pcs::{ProveError, VerifyError};
use super::transcript::{ProverState, VerifierState};
use crate::{cfg_into_iter, cfg_iter_mut};
use crate::ligerito::{
    xi_combined_rows, xi_combined_rows_packed, xi_combined_rows_packed_live,
    xi_combined_rows_packed_many,
};

use crate::pcs::IntegerMatrixLayout;
use field::Gf128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;
use crate::utils::wide_mul::WideMulAcc;
use std::sync::OnceLock;

const PARALLEL_MIN_PAIRS: usize = 1 << 12;

/// Transcript-preserving shortcuts of [`prove`] and [`prove_sum`]. Each has
/// an environment switch (`0` or `1`), read once; tests set them directly.
/// Every one regroups the same exact field sums, so the messages do not
/// change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SumcheckMode {
    /// `WFBITZ_GROUP_SKIP` (default on): a sum's column combination skips,
    /// per term, the 64-column groups whose weights are all zero.
    pub(crate) skip_groups: bool,
    /// `WFBITZ_TERM_BATCH` (default on): a sum's column combination runs
    /// every term in one parallel region, and each row round folds every
    /// term and takes its next round polynomial in one pass, the terms
    /// spread over the thread pool, instead of a parallel call per term for
    /// the combination and three per term and round.
    pub(crate) batch_terms: bool,
    /// `WFBITZ_ONE_COLUMN` (default on): a single-column claim reads the
    /// row-folded witness off its bound column-combined table instead of
    /// folding the committed rows again.
    pub(crate) one_column: bool,
}

impl SumcheckMode {
    /// Everything off: the messages computed as before the shortcuts.
    #[cfg(test)]
    pub(crate) const PLAIN: Self = Self {
        skip_groups: false,
        batch_terms: false,
        one_column: false,
    };

    pub(crate) fn from_env() -> Self {
        static MODE: OnceLock<SumcheckMode> = OnceLock::new();
        *MODE.get_or_init(|| {
            let flag = |name: &str| !matches!(std::env::var(name).as_deref(), Ok("0"));
            SumcheckMode {
                skip_groups: flag("WFBITZ_GROUP_SKIP"),
                batch_terms: flag("WFBITZ_TERM_BATCH"),
                one_column: flag("WFBITZ_ONE_COLUMN"),
            }
        })
    }
}

/// The witness after its row coordinates are bound, read off a bound
/// single-column table: the row rounds fold `M̃ = w2[0]·bit(0, ·)`
/// linearly, so `bound = w2[0]·bit~(r_b)` exactly and `bit~(r_b)` is
/// `bound / w2[0]` — the value [`fold_rows_point`] sums up again. `None`
/// when the column weight is zero (the table carries no information).
fn one_column_value(column_weight: Gf, bound: Gf) -> Option<Gf> {
    if column_weight == Gf::one() {
        Some(bound)
    } else if column_weight == Gf::zero() {
        None
    } else {
        Some(bound * column_weight.inverse_or_zero())
    }
}

/// A pending evaluation claim over the original committed bit polynomial.
pub(crate) struct MleClaim {
    /// Row coordinates before column coordinates, low-bit-first.
    pub(crate) point: Vec<Gf>,
    /// The witness MLE evaluation at `point`.
    pub(crate) target: Gf,
}

/// `Σ_b eq(b, point)·bit(c, b)` for every column `c`: the witness after its
/// row coordinates are bound to `point` (low-bit-first). The eq weight
/// factors over the coordinates, `eq(b, point) = eq(b mod 64, point[..6]) ·
/// eq(b div 64, point[6..])`, so a column's sum is `Σ_w eq_hi[w] · Σ_{j set
/// in word w} eq_lo[j]`: the inner sums come out of one 32 KB byte table
/// over `eq_lo` (eight lookups per word), the outer one is a multiply per
/// word. Exact: the same field products regrouped.
fn fold_rows_point(rows: &[Vec<u64>], point: &[Gf]) -> Vec<Gf> {
    const LOW: usize = 6;
    if point.len() < LOW {
        let eq = if point.is_empty() {
            vec![Gf::one()]
        } else {
            build_eq_x_r_vec(point, &()).expect("non-empty row point")
        };
        return fold_rows_eq(rows, &eq);
    }
    let eq_lo = build_eq_x_r_vec(&point[..LOW], &()).expect("six coordinates");
    let eq_hi = if point.len() == LOW {
        vec![Gf::one()]
    } else {
        build_eq_x_r_vec(&point[LOW..], &()).expect("high coordinates")
    };
    // table[pos][byte] = Σ_{bits b of byte} eq_lo[8·pos + b].
    let mut table = vec![[Gf::zero(); 256]; 8];
    for (pos, tb) in table.iter_mut().enumerate() {
        for byte in 1..256usize {
            tb[byte] = tb[byte & (byte - 1)] + eq_lo[(pos << 3) | byte.trailing_zeros() as usize];
        }
    }
    cfg_into_iter!(rows, 16)
        .map(|row| {
            debug_assert_eq!(row.len(), eq_hi.len());
            let zero = Gf::zero();
            let mut acc = <Gf as WideMulAcc>::wide_zero(&zero);
            for (word, &weight) in row.iter().zip(&eq_hi) {
                let mut inner = zero;
                for (pos, tb) in table.iter().enumerate() {
                    inner += tb[((word >> (8 * pos)) & 0xFF) as usize];
                }
                <Gf as WideMulAcc>::wide_add_assign(&mut acc, &<Gf as WideMulAcc>::mul_wide(&weight, &inner));
            }
            <Gf as WideMulAcc>::from_wide(acc)
        })
        .collect()
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
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    prove_with(claim, rows_bits, packed_cols, transcript, SumcheckMode::from_env())
}

/// [`prove`] under an explicit [`SumcheckMode`].
pub(crate) fn prove_with(
    claim: &LinearClaimGf,
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
    mode: SumcheckMode,
) -> Result<MleClaim, ProveError> {
    let w1 = claim.row_weights();
    let w2 = claim.column_weights();
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
    let started = std::time::Instant::now();
    let mut combined = if packed_cols.is_empty() {
        xi_combined_rows(&layout, rows_bits, w2)
    } else {
        xi_combined_rows_packed(&layout, packed_cols, w2)
    };
    super::trace("    sc combine cols", started);
    let started = std::time::Instant::now();
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

    super::trace("    sc row rounds", started);

    // Columns: the row-folded table against `w1(r_b)·column_weights`.
    let started = std::time::Instant::now();
    let read = if mode.one_column && w2.len() == 1 {
        one_column_value(w2[0], combined[0])
    } else {
        None
    };
    let mut folded = match read {
        Some(value) => vec![value],
        None => fold_rows_point(rows_bits, &point),
    };
    super::trace("    sc fold rows", started);
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

/// [`prove`] for a sum of factored claims: the row rounds run one
/// column-combined table per term against that term's row weights and send
/// the summed round polynomials; once the rows are bound every term is the
/// scalar `w1_k(r_b)`, so the column rounds run the one row-folded table
/// against `Σ_k w1_k(r_b)·cols_k`, exactly the single-term column rounds.
pub(crate) fn prove_sum(
    claim: &SumClaimGf,
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    prove_sum_with(claim, rows_bits, packed_cols, transcript, SumcheckMode::from_env())
}

/// [`prove_sum`] under an explicit [`SumcheckMode`].
pub(crate) fn prove_sum_with(
    claim: &SumClaimGf,
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
    mode: SumcheckMode,
) -> Result<MleClaim, ProveError> {
    let terms = claim.terms();
    let (n_rows, n_cols) = (claim.rows(), claim.columns());
    if rows_bits.len() != n_cols || rows_bits.iter().any(|row| row.len() * 64 != n_rows) {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }
    let layout = IntegerMatrixLayout {
        row_vars: n_rows.ilog2() as usize,
        col_vars: n_cols.ilog2() as usize,
        word_bits: 1,
    };
    let mut target = claim.target();
    let mut point = Vec::with_capacity(layout.row_vars + layout.col_vars);

    let started = std::time::Instant::now();
    let mut combined: Vec<Vec<Gf>> = if mode.batch_terms && !packed_cols.is_empty() {
        let weights: Vec<&[Gf]> = terms.iter().map(|(_, w2)| w2.as_slice()).collect();
        xi_combined_rows_packed_many(&layout, packed_cols, &weights, mode.skip_groups)
    } else {
        terms
            .iter()
            .map(|(_, w2)| {
                if packed_cols.is_empty() {
                    xi_combined_rows(&layout, rows_bits, w2)
                } else if mode.skip_groups {
                    xi_combined_rows_packed_live(&layout, packed_cols, w2)
                } else {
                    xi_combined_rows_packed(&layout, packed_cols, w2)
                }
            })
            .collect()
    };
    super::trace("    sc combine cols (sum)", started);
    let started = std::time::Instant::now();
    let mut rows: Vec<Vec<Gf>> = terms.iter().map(|(w1, _)| w1.clone()).collect();
    if mode.batch_terms {
        let mut coefficients = round_polynomial_terms(&combined, &rows);
        while combined[0].len() > 1 {
            if coefficients[1] + coefficients[2] != target {
                return Err(ProveError::InvalidClaim);
            }
            transcript.prover_message(&coefficients);
            let challenge = transcript.verifier_message::<Gf>();
            target = evaluate_round(coefficients, challenge);
            point.push(challenge);
            coefficients = fold_terms(&mut combined, &mut rows, challenge);
        }
    } else {
        while combined[0].len() > 1 {
            let mut coefficients = [Gf::zero(); 3];
            for (table, weights) in combined.iter().zip(&rows) {
                let part = round_polynomial(table, weights);
                for (sum, value) in coefficients.iter_mut().zip(part) {
                    *sum += value;
                }
            }
            if coefficients[1] + coefficients[2] != target {
                return Err(ProveError::InvalidClaim);
            }
            transcript.prover_message(&coefficients);
            let challenge = transcript.verifier_message::<Gf>();
            target = evaluate_round(coefficients, challenge);
            point.push(challenge);
            for table in &mut combined {
                fold(table, challenge);
            }
            for weights in &mut rows {
                fold(weights, challenge);
            }
        }
    }
    super::trace("    sc row rounds (sum)", started);

    let started = std::time::Instant::now();
    // One column: every term's bound table is `w2_k[0]·bit~(r_b)`.
    let read = if mode.one_column && n_cols == 1 {
        terms
            .iter()
            .zip(&combined)
            .find_map(|((_, w2), table)| one_column_value(w2[0], table[0]))
    } else {
        None
    };
    let mut folded = match read {
        Some(value) => vec![value],
        None => fold_rows_point(rows_bits, &point),
    };
    super::trace("    sc fold rows", started);
    let mut columns = merged_columns(terms, &rows);
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

/// `Σ_k w1_k(r_b)·cols_k`: the column weights once every term's row weights
/// are folded to the scalar `w1_k(r_b)`.
fn merged_columns(terms: &[(Vec<Gf>, Vec<Gf>)], bound_rows: &[Vec<Gf>]) -> Vec<Gf> {
    let mut columns = vec![Gf::zero(); terms[0].1.len()];
    for ((_, w2), bound) in terms.iter().zip(bound_rows) {
        let scalar = bound[0];
        for (sum, &weight) in columns.iter_mut().zip(w2) {
            *sum += scalar * weight;
        }
    }
    columns
}

pub(crate) fn verify_sum(
    claim: &SumClaimGf,
    transcript: &mut VerifierState<'_>,
) -> Result<MleClaim, VerifyError> {
    let terms = claim.terms();
    let row_rounds = claim.rows().ilog2() as usize;
    let col_rounds = claim.columns().ilog2() as usize;
    let mut rows: Vec<Vec<Gf>> = terms.iter().map(|(w1, _)| w1.clone()).collect();
    let mut columns = if row_rounds == 0 { merged_columns(terms, &rows) } else { Vec::new() };
    let mut point = Vec::with_capacity(row_rounds + col_rounds);
    let mut target = claim.target();
    for round in 0..row_rounds + col_rounds {
        let coefficients = transcript
            .prover_message::<[Gf; 3]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        if coefficients[1] + coefficients[2] != target {
            return Err(VerifyError::VerificationFailed);
        }
        let challenge = transcript.verifier_message::<Gf>();
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        if round < row_rounds {
            for weights in &mut rows {
                fold(weights, challenge);
            }
            if round + 1 == row_rounds {
                columns = merged_columns(terms, &rows);
            }
        } else {
            fold(&mut columns, challenge);
        }
    }
    let evaluation = transcript
        .prover_message::<Gf>()
        .map_err(|_| VerifyError::MalformedProof)?;
    if target != evaluation * columns[0] {
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
            round_sums(witness, weights, start..end)
        })
        .collect();
    let [at_zero, at_one, quadratic] = partials.into_iter().fold(
        [Gf::zero(); 3],
        |[a, b, c], [x, y, z]| [a + x, b + y, c + z],
    );
    [at_zero, at_zero + at_one + quadratic, quadratic]
}

/// `[Σ M0·w0, Σ M1·w1, Σ (M0+M1)(w0+w1)]` over the pairs in `range`.
fn round_sums(witness: &[Gf], weights: &[Gf], range: core::ops::Range<usize>) -> [Gf; 3] {
    let zero = Gf::zero();
    let mut at_zero = <Gf as WideMulAcc>::wide_zero(&zero);
    let mut at_one = <Gf as WideMulAcc>::wide_zero(&zero);
    let mut quadratic = <Gf as WideMulAcc>::wide_zero(&zero);
    for pair_index in range {
        let index = 2 * pair_index;
        let (m0, m1) = (witness[index], witness[index + 1]);
        let (w0, w1) = (weights[index], weights[index + 1]);
        <Gf as WideMulAcc>::wide_add_assign(&mut at_zero, &<Gf as WideMulAcc>::mul_wide(&m0, &w0));
        <Gf as WideMulAcc>::wide_add_assign(&mut at_one, &<Gf as WideMulAcc>::mul_wide(&m1, &w1));
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
}

/// [`round_polynomial`] summed over the terms of a sum claim, one serial
/// pass per term and the terms spread over the thread pool.
fn round_polynomial_terms(tables: &[Vec<Gf>], weights: &[Vec<Gf>]) -> [Gf; 3] {
    let partials: Vec<[Gf; 3]> = cfg_into_iter!(0..tables.len())
        .map(|k| round_sums(&tables[k], &weights[k], 0..tables[k].len() / 2))
        .collect();
    let [at_zero, at_one, quadratic] = partials.into_iter().fold(
        [Gf::zero(); 3],
        |[a, b, c], [x, y, z]| [a + x, b + y, c + z],
    );
    [at_zero, at_zero + at_one + quadratic, quadratic]
}

/// Binds every term's table and row weights to `challenge` in place (as
/// [`fold`]) and returns the next round's [`round_polynomial_terms`], each
/// term folded and summed in one task while its halves are cache-hot.
fn fold_terms(tables: &mut [Vec<Gf>], weights: &mut [Vec<Gf>], challenge: Gf) -> [Gf; 3] {
    let fold_in_place = |values: &mut Vec<Gf>| {
        let len = values.len() / 2;
        for index in 0..len {
            let zero = values[2 * index];
            let one = values[2 * index + 1];
            values[index] = zero + challenge * (zero + one);
        }
        values.truncate(len);
    };
    let partials: Vec<[Gf; 3]> = cfg_iter_mut!(tables)
        .zip(cfg_iter_mut!(weights))
        .map(|(table, weights)| {
            fold_in_place(table);
            fold_in_place(weights);
            round_sums(table, weights, 0..table.len() / 2)
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

#[cfg(test)]
mod sum_tests {
    use super::*;
    use crate::wfbitz::params::{LinearClaimGf, Shape, SumClaimGf};
    use crate::wfbitz::{build_prover, build_verifier};

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn random_gf(state: &mut u64) -> Gf {
        Gf::new(xorshift(state), xorshift(state))
    }

    /// `Σ_k Σ_{b,c} rows_k[b]·cols_k[c]·bit(b, c)` over the bits.
    fn dense_target(rows_bits: &[Vec<u64>], terms: &[(Vec<Gf>, Vec<Gf>)]) -> Gf {
        let mut total = Gf::zero();
        for (c, column) in rows_bits.iter().enumerate() {
            for (word_index, &word) in column.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let b = word_index * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    for (rows, cols) in terms {
                        total += rows[b] * cols[c];
                    }
                }
            }
        }
        total
    }

    fn grid(state: &mut u64, shape: &Shape) -> Vec<Vec<u64>> {
        (0..shape.columns())
            .map(|_| (0..shape.rows() / 64).map(|_| xorshift(state)).collect())
            .collect()
    }

    fn terms(state: &mut u64, shape: &Shape, count: usize) -> Vec<(Vec<Gf>, Vec<Gf>)> {
        (0..count)
            .map(|_| {
                (
                    (0..shape.rows()).map(|_| random_gf(state)).collect(),
                    (0..shape.columns()).map(|_| random_gf(state)).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn a_sum_of_tensors_reduces_to_the_witness_evaluation() {
        let shape = Shape::new(8, 5).unwrap();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let rows_bits = grid(&mut state, &shape);
        let terms = terms(&mut state, &shape, 3);
        let target = dense_target(&rows_bits, &terms);
        let claim = SumClaimGf::from_shape(&shape, terms, target, b"sum-test".to_vec()).unwrap();
        let mut prover = build_prover(b"sum-test/v1", b"instance");
        let reduced = prove_sum(&claim, &rows_bits, &[], &mut prover).unwrap();
        let proof = prover.finish();
        let mut verifier = build_verifier(b"sum-test/v1", b"instance", &proof);
        let checked = verify_sum(&claim, &mut verifier).unwrap();
        assert_eq!(reduced.point, checked.point);
        assert_eq!(reduced.target, checked.target);
        // The reduced value is the witness MLE at the point: the dense sum of
        // eq weights over the set bits.
        let eq = crate::poly::utils::build_eq_x_r_vec(&reduced.point, &()).unwrap();
        let mut evaluation = Gf::zero();
        for (c, column) in rows_bits.iter().enumerate() {
            for (word_index, &word) in column.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let b = word_index * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    evaluation += eq[c * shape.rows() + b];
                }
            }
        }
        assert_eq!(reduced.target, evaluation);
        // A wrong target is caught by the prover's own consistency check.
        let wrong = SumClaimGf::from_shape(&shape, claim.terms().to_vec(), target + Gf::one(), Vec::new()).unwrap();
        let mut prover = build_prover(b"sum-test/v1", b"instance");
        assert!(prove_sum(&wrong, &rows_bits, &[], &mut prover).is_err());
    }

    #[test]
    fn one_term_matches_the_single_claim_messages() {
        let shape = Shape::new(7, 6).unwrap();
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let rows_bits = grid(&mut state, &shape);
        let mut terms = terms(&mut state, &shape, 1);
        let (rows, cols) = terms.pop().unwrap();
        let target = dense_target(&rows_bits, &[(rows.clone(), cols.clone())]);
        let single = LinearClaimGf::from_shape(&shape, rows.clone(), cols.clone(), target).unwrap();
        let sum = SumClaimGf::from_shape(&shape, vec![(rows, cols)], target, Vec::new()).unwrap();
        let mut a = build_prover(b"sum-test/v1", b"one");
        let reduced_single = prove(&single, &rows_bits, &[], &mut a).unwrap();
        let mut b = build_prover(b"sum-test/v1", b"one");
        let reduced_sum = prove_sum(&sum, &rows_bits, &[], &mut b).unwrap();
        assert_eq!(a.finish().narg_string, b.finish().narg_string);
        assert_eq!(reduced_single.point, reduced_sum.point);
        assert_eq!(reduced_single.target, reduced_sum.target);
    }

    /// Every [`SumcheckMode`], applied alone or together.
    fn modes() -> Vec<SumcheckMode> {
        (0..8u8)
            .map(|bits| SumcheckMode {
                skip_groups: bits & 1 != 0,
                batch_terms: bits & 2 != 0,
                one_column: bits & 4 != 0,
            })
            .collect()
    }

    /// Terms shaped like the chained opening's: column vectors on one
    /// 64-column group, on one column, on the groups past the first, and
    /// dense; some groups of the grid carry no weight at all.
    fn chained_like_terms(state: &mut u64, shape: &Shape) -> Vec<(Vec<Gf>, Vec<Gf>)> {
        let (rows, cols) = (shape.rows(), shape.columns());
        let row = |state: &mut u64| (0..rows).map(|_| random_gf(state)).collect::<Vec<_>>();
        let masked = |state: &mut u64, keep: &dyn Fn(usize) -> bool| {
            (0..cols)
                .map(|c| if keep(c) { random_gf(state) } else { Gf::zero() })
                .collect::<Vec<_>>()
        };
        let last = cols - 1;
        vec![
            (row(state), masked(state, &|c| c < 64)),
            (row(state), masked(state, &|c| c < 64 && c % 2 == 1)),
            (row(state), masked(state, &|c| c == last)),
            (row(state), masked(state, &|c| c == last)),
            (row(state), masked(state, &|c| c >= 64)),
            (row(state), masked(state, &|c| c == 0)),
            (row(state), masked(state, &|_| true)),
        ]
    }

    /// The shortcuts send the same messages and reduce to the same claim
    /// as the plain passes, on the packed and the unpacked witness.
    #[test]
    fn sum_shortcuts_prove_identically() {
        for (t, s) in [(7usize, 0usize), (8, 1), (7, 6), (9, 7), (8, 8), (11, 9)] {
            let shape = Shape::new(t, s).unwrap();
            let mut state = 0x6a09_e667_f3bc_c908u64 ^ (t * 131 + s) as u64;
            let rows_bits = grid(&mut state, &shape);
            let terms = if s >= 7 {
                chained_like_terms(&mut state, &shape)
            } else {
                let mut terms = terms(&mut state, &shape, 4);
                // A term with a zero column weight: the one-column read
                // must pass over it.
                terms[0].1.iter_mut().for_each(|w| *w = Gf::zero());
                terms
            };
            let target = dense_target(&rows_bits, &terms);
            let claim = SumClaimGf::from_shape(&shape, terms, target, Vec::new()).unwrap();
            let layout = IntegerMatrixLayout { row_vars: t, col_vars: s, word_bits: 1 };
            let packed = crate::ligerito::pack_columns_from_rows(&layout, &rows_bits);
            let unpacked: Vec<Vec<u64>> = Vec::new();
            for packed_cols in [packed.as_slice(), unpacked.as_slice()] {
                let run = |mode: SumcheckMode| {
                    let mut prover = build_prover(b"sum-modes/v1", b"instance");
                    let reduced = prove_sum_with(&claim, &rows_bits, packed_cols, &mut prover, mode).unwrap();
                    (reduced.point, reduced.target, prover.finish().narg_string)
                };
                let plain = run(SumcheckMode::PLAIN);
                for mode in modes() {
                    let shortcut = run(mode);
                    assert_eq!((&shortcut.0, &shortcut.1), (&plain.0, &plain.1), "t={t} s={s} {mode:?}");
                    assert!(shortcut.2 == plain.2, "transcript t={t} s={s} {mode:?}");
                }
            }
        }
    }

    /// A single-column claim reads its row-folded witness off the bound
    /// table, for a unit, a random and a zero column weight.
    #[test]
    fn one_column_claims_prove_identically() {
        for t in [7usize, 10, 13] {
            let shape = Shape::new(t, 0).unwrap();
            let mut state = 0xbb67_ae85_84ca_a73bu64 ^ t as u64;
            let rows_bits = grid(&mut state, &shape);
            for weight in [Gf::one(), random_gf(&mut state), Gf::zero()] {
                let rows: Vec<Gf> = (0..shape.rows()).map(|_| random_gf(&mut state)).collect();
                let target = dense_target(&rows_bits, &[(rows.clone(), vec![weight])]);
                let claim = LinearClaimGf::from_shape(&shape, rows, vec![weight], target).unwrap();
                let run = |mode: SumcheckMode| {
                    let mut prover = build_prover(b"one-column/v1", b"instance");
                    let reduced = prove_with(&claim, &rows_bits, &[], &mut prover, mode).unwrap();
                    (reduced.point, reduced.target, prover.finish().narg_string)
                };
                let plain = run(SumcheckMode::PLAIN);
                let read = run(SumcheckMode { one_column: true, ..SumcheckMode::PLAIN });
                assert_eq!((&read.0, &read.1), (&plain.0, &plain.1), "t={t} weight={weight:?}");
                assert!(read.2 == plain.2, "transcript t={t} weight={weight:?}");
            }
        }
    }

    /// The group-skipping and the batched combinations are the full one.
    #[test]
    fn live_group_combination_matches_the_full_one() {
        for (t, s) in [(7usize, 7usize), (8, 8), (9, 10), (11, 6)] {
            let shape = Shape::new(t, s).unwrap();
            let mut state = 0x3c6e_f372_fe94_f82bu64 ^ (t * 7 + s) as u64;
            let rows_bits = grid(&mut state, &shape);
            let layout = IntegerMatrixLayout { row_vars: t, col_vars: s, word_bits: 1 };
            let packed = crate::ligerito::pack_columns_from_rows(&layout, &rows_bits);
            let terms: Vec<Vec<Gf>> = if s >= 7 {
                chained_like_terms(&mut state, &shape).into_iter().map(|(_, cols)| cols).collect()
            } else {
                terms(&mut state, &shape, 3).into_iter().map(|(_, cols)| cols).collect()
            };
            let weights: Vec<Vec<Gf>> =
                terms.into_iter().chain([vec![Gf::zero(); shape.columns()]]).collect();
            let full: Vec<Vec<Gf>> =
                weights.iter().map(|w| xi_combined_rows_packed(&layout, &packed, w)).collect();
            for (w, expected) in weights.iter().zip(&full) {
                assert_eq!(&xi_combined_rows_packed_live(&layout, &packed, w), expected, "t={t} s={s}");
            }
            let refs: Vec<&[Gf]> = weights.iter().map(Vec::as_slice).collect();
            for skip in [false, true] {
                assert_eq!(
                    xi_combined_rows_packed_many(&layout, &packed, &refs, skip),
                    full,
                    "t={t} s={s} skip={skip}"
                );
            }
        }
    }
}
