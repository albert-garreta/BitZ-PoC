//! Their fold round (step 3): send the integer column folds, derive the
//! images, take the batching point.

use crypto_bigint::{NonZero, U128};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::cfg_into_iter;

use super::params::{LinearClaim, Shape};
use super::transcript::{ProverState, VerifierState};
use super::{BitZProver, BitZVerifier};
use crate::ligerito::mle_eval;
use crate::pcs::FixedBasePow;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// A fold the prover cannot produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    ShapeMismatch,
}

/// A fold the verifier rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveError {
    /// A record is missing or does not decode.
    MalformedProof,
    /// A fold is above `k_1 (q - 1)`, where its image stops determining it.
    FoldOutOfRange,
    /// The folds do not reconstruct the claimed target.
    TargetMismatch,
}

/// The state both sides hold once the fold round closes. Only `folds` is
/// transmitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold {
    /// The column folds `eta_j`, as integers.
    pub folds: Vec<u128>,
    /// `g^{eta_j}` over `j`, derived on both sides.
    pub images: Vec<Gf>,
    /// `y_i = g^{w_i}`, one per row.
    pub row_images: Vec<Gf>,
    /// The challenge drawn after the folds are bound.
    pub zeta: Vec<Gf>,
    /// The batched output claim: the multilinear extension of the images at
    /// `zeta`.
    pub e0: Gf,
}

/// `eta_j = sum_i w_i f_ij` over the integers, from one column's bit row
/// (64 bits per word, bit `i` of the column at word `i / 64`).
pub fn fold_column(row: &[u64], exponents: &[u128]) -> u128 {
    let mut total = 0u128;
    for (word_index, &word) in row.iter().enumerate() {
        let mut remaining = word;
        while remaining != 0 {
            total += exponents[word_index * 64 + remaining.trailing_zeros() as usize];
            remaining &= remaining - 1;
        }
    }
    total
}

/// Every column's fold, in column order — the same integers
/// [`fold_column`] produces one bit at a time, through digit tables:
/// `T[d][v] = Σ_{i ∈ bits(v)} w_{D·d + i}` for every `D`-bit digit
/// position `d` of the rows, then one pass over the words, a block of
/// columns at a time with every accumulator of the block live — for word
/// `wi` the word's `64/D` tables are in L1 and each column adds one entry
/// per digit, a fixed trip count. (The crate's
/// [`crate::ligerito::fold_values_bits`] streams its whole table once per
/// 32-column block and exits its digit loop on a data-dependent count; at
/// `2^28` bits it measured 11 ms against 7.3 here.)
pub fn fold_columns(shape: &Shape, rows: &[Vec<u64>], exponents: &[u128]) -> Vec<u128> {
    let cols = shape.columns();
    assert_eq!(rows.len(), cols);
    let words = shape.rows() / 64;
    assert!(rows.iter().all(|row| row.len() == words));
    assert_eq!(exponents.len(), shape.rows());
    fold_columns_digits::<FOLD_DIGIT_BITS>(rows, exponents, words, cols)
}

/// The digit width of [`fold_columns`]'s tables: nibbles, 16-entry tables,
/// `rows/4` of them (8 MB at `2^17` rows). Byte tables (64 MB) measured
/// the same speed at `2^28` bits and cost their first touch.
const FOLD_DIGIT_BITS: usize = 4;

/// Columns per block of [`fold_columns`]'s pass: a block's word lines and
/// a task's tables stay in L1 (2048 columns at once measured 9.0 ms
/// against 7.3 with 128 at `2^28` bits).
const FOLD_COLUMN_BLOCK: usize = 128;

fn fold_columns_digits<const D: usize>(
    rows: &[Vec<u64>],
    exponents: &[u128],
    words: usize,
    cols: usize,
) -> Vec<u128> {
    const { assert!(D == 4 || D == 8) };
    let per_word = 64 / D;
    let entries = 1usize << D;
    let digits = words * per_word;
    // The tables, digit-major: `tbl[(d ≪ D) | v]`.
    let mut tbl: Vec<u128> = Vec::with_capacity(digits * entries);
    let spare = &mut tbl.spare_capacity_mut()[..digits * entries];
    crate::cfg_chunks_mut!(spare, entries).enumerate().for_each(|(d, table)| {
        let base = d * D;
        table[0].write(0);
        for v in 1..entries {
            // `T[v] = T[v without its lowest bit] + that bit's weight`.
            let lower = unsafe { table[v & (v - 1)].assume_init() };
            table[v].write(lower.wrapping_add(exponents[base + v.trailing_zeros() as usize]));
        }
    });
    // SAFETY: every entry of every table was written above.
    unsafe { tbl.set_len(digits * entries) };
    let mask = (entries - 1) as u64;

    // One accumulator per column, the words split across tasks (each
    // task's sums merged at the end — exact integers either way).
    let sum_words = |acc: &mut [u128], range: std::ops::Range<usize>| {
        for c0 in (0..cols).step_by(FOLD_COLUMN_BLOCK) {
            let c1 = (c0 + FOLD_COLUMN_BLOCK).min(cols);
            for wi in range.clone() {
                let tables = &tbl[wi * per_word * entries..(wi + 1) * per_word * entries];
                for c in c0..c1 {
                    let w = rows[c][wi];
                    let mut sum = 0u128;
                    for k in 0..per_word {
                        let v = ((w >> (k * D)) & mask) as usize;
                        sum = sum.wrapping_add(tables[(k << D) | v]);
                    }
                    acc[c] = acc[c].wrapping_add(sum);
                }
            }
        }
    };
    #[cfg(feature = "parallel")]
    {
        let threads = rayon::current_num_threads().max(1);
        let chunk = words.div_ceil(4 * threads).max(1);
        let partials: Vec<Vec<u128>> = (0..words.div_ceil(chunk))
            .into_par_iter()
            .map(|i| {
                let mut acc = vec![0u128; cols];
                sum_words(&mut acc, i * chunk..((i + 1) * chunk).min(words));
                acc
            })
            .collect();
        let mut out = vec![0u128; cols];
        for acc in partials {
            for (o, a) in out.iter_mut().zip(acc) {
                *o = o.wrapping_add(a);
            }
        }
        out
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut out = vec![0u128; cols];
        sum_words(&mut out, 0..words);
        out
    }
}

/// `g^{eta_j}`, one per column.
pub fn column_images(comb: &FixedBasePow, folds: &[u128]) -> Vec<Gf> {
    cfg_into_iter!(folds, 64).map(|&fold| comb.pow(fold)).collect()
}

/// `y_i = g^{w_i}`, one per row.
pub fn row_images(comb: &FixedBasePow, exponents: &[u128]) -> Vec<Gf> {
    cfg_into_iter!(exponents, 256)
        .map(|&exponent| comb.pow(exponent))
        .collect()
}

/// The reconstruction `sum_j v^(2)_j pi_q(eta_j)` in `F_q`.
pub fn reconstruct(claim: &LinearClaim, folds: &[u128], q: u128) -> u128 {
    weighted_sum_mod(claim.column_weights(), folds, q)
}

/// `Σ_j weights[j] · (folds[j] mod q) mod q` over `u128` words: each
/// product as a 256-bit `(lo, hi)` pair reduced by one wide remainder,
/// the sum kept reduced.
fn weighted_sum_mod(weights: &[u128], folds: &[u128], q: u128) -> u128 {
    let modulus = NonZero::<U128>::new_unwrap(U128::from_u128(q));
    let mut acc = U128::ZERO;
    for (&weight, &fold) in weights.iter().zip(folds) {
        let product = U128::from_u128(weight).widening_mul(&U128::from_u128(fold % q));
        let term = U128::rem_wide_vartime(product, &modulus);
        acc = acc.add_mod(&term, &modulus);
    }
    u128::from(acc)
}

fn finish(
    shape: &Shape,
    comb: &FixedBasePow,
    claim: &LinearClaim,
    folds: Vec<u128>,
    zeta: Vec<Gf>,
) -> Fold {
    let images = column_images(comb, &folds);
    let row_images = row_images(comb, claim.row_exponents());
    assert_eq!(images.len(), shape.columns());
    let e0 = mle_eval(&images, &zeta);
    Fold {
        folds,
        images,
        row_images,
        zeta,
        e0,
    }
}

impl BitZProver {
    /// Runs the fold round: each fold as one 16-byte record, then `s`
    /// scalar squeezes.
    pub(crate) fn send_fold(
        &self,
        claim: &LinearClaim,
        rows: &[Vec<u64>],
        transcript: &mut ProverState,
    ) -> Result<Fold, SendError> {
        let shape = self.params().shape();
        if rows.len() != shape.columns() {
            return Err(SendError::ShapeMismatch);
        }
        let started = std::time::Instant::now();
        let folds = fold_columns(shape, rows, claim.row_exponents());
        super::trace("  column folds", started);
        for fold in &folds {
            transcript.prover_message(&fold.to_le_bytes());
        }
        let zeta = (0..shape.log_columns())
            .map(|_| transcript.verifier_message::<Gf>())
            .collect();
        let started = std::time::Instant::now();
        let fold = finish(shape, self.comb(), claim, folds, zeta);
        super::trace("  images", started);
        Ok(fold)
    }
}

impl BitZVerifier {
    /// Reads the fold round and checks it: range bound, reconstruction, then
    /// the challenge.
    pub(crate) fn receive_fold(
        &self,
        claim: &LinearClaim,
        transcript: &mut VerifierState<'_>,
    ) -> Result<Fold, ReceiveError> {
        let shape = self.params().shape();
        let folds = (0..shape.columns())
            .map(|_| {
                transcript
                    .prover_message::<[u8; 16]>()
                    .map(u128::from_le_bytes)
                    .map_err(|_| ReceiveError::MalformedProof)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if folds.iter().any(|&fold| fold > self.params().fold_bound()) {
            return Err(ReceiveError::FoldOutOfRange);
        }
        if reconstruct(claim, &folds, self.params().q()) != claim.target() {
            return Err(ReceiveError::TargetMismatch);
        }
        let zeta = (0..shape.log_columns())
            .map(|_| transcript.verifier_message::<Gf>())
            .collect();
        Ok(finish(shape, self.comb(), claim, folds, zeta))
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::weighted_sum_mod;

    fn reference(weights: &[u128], folds: &[u128], q: u128) -> u128 {
        let modulus = BigUint::from(q);
        let mut acc = BigUint::from(0u8);
        for (&weight, &fold) in weights.iter().zip(folds) {
            acc += (BigUint::from(weight) * BigUint::from(fold % q)) % &modulus;
        }
        let digits = (acc % &modulus).to_u64_digits();
        let mut value = 0u128;
        for (i, d) in digits.iter().enumerate().take(2) {
            value |= u128::from(*d) << (64 * i);
        }
        value
    }

    #[test]
    fn digit_folds_match_the_bit_walk() {
        use super::{Shape, fold_column, fold_columns, fold_columns_digits};
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        // Tiny geometries straight into the digit passes, then the
        // protocol's smallest shapes through the entry point.
        for (log_rows, log_cols) in [(6usize, 0usize), (6, 3), (7, 1), (9, 4), (10, 8), (14, 8), (15, 7)] {
            let rows: Vec<Vec<u64>> = (0..1 << log_cols)
                .map(|_| (0..(1usize << log_rows) / 64).map(|_| next()).collect())
                .collect();
            let exponents: Vec<u128> = (0..1 << log_rows)
                .map(|_| (u128::from(next()) << 36) ^ u128::from(next()))
                .collect();
            let want: Vec<u128> = rows.iter().map(|row| fold_column(row, &exponents)).collect();
            let words = (1usize << log_rows) / 64;
            assert_eq!(fold_columns_digits::<4>(&rows, &exponents, words, 1 << log_cols), want, "t {log_rows} s {log_cols}");
            assert_eq!(fold_columns_digits::<8>(&rows, &exponents, words, 1 << log_cols), want, "t {log_rows} s {log_cols}");
            if let Ok(shape) = Shape::new(log_rows, log_cols) {
                assert_eq!(fold_columns(&shape, &rows, &exponents), want, "t {log_rows} s {log_cols}");
            }
        }
    }

    #[test]
    fn wide_reconstruction_matches_the_bigint_reference() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut wide = || (u128::from(next()) << 64) | u128::from(next());
        for q in [(1u128 << 100) - 15, (1u128 << 114) - 11, u128::MAX - 158, 3, 1u128 << 127] {
            for len in [0usize, 1, 2, 17, 256] {
                let weights: Vec<u128> = (0..len).map(|_| wide() % q).collect();
                let folds: Vec<u128> = (0..len).map(|_| wide()).collect();
                assert_eq!(weighted_sum_mod(&weights, &folds, q), reference(&weights, &folds, q), "q {q} len {len}");
                // Unreduced weights too: the product still fits 256 bits.
                let raw: Vec<u128> = (0..len).map(|_| wide()).collect();
                assert_eq!(weighted_sum_mod(&raw, &folds, q), reference(&raw, &folds, q), "raw q {q} len {len}");
            }
        }
    }
}
