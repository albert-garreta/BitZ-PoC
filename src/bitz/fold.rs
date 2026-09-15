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

/// Every column's fold, in column order — the crate's nibble-table fold
/// ([`crate::ligerito::fold_values_bits`]), the same integers
/// [`fold_column`] produces one bit at a time.
pub fn fold_columns(shape: &Shape, rows: &[Vec<u64>], exponents: &[u128]) -> Vec<u128> {
    crate::ligerito::fold_values_bits(&shape.layout(), rows, exponents)
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
        let folds = fold_columns(shape, rows, claim.row_exponents());
        for fold in &folds {
            transcript.prover_message(&fold.to_le_bytes());
        }
        let zeta = (0..shape.log_columns())
            .map(|_| transcript.verifier_message::<Gf>())
            .collect();
        Ok(finish(shape, self.comb(), claim, folds, zeta))
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
