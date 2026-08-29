//! Step 5.0: reducing the prime modulus of a bitified tensor claim.
//!
//! Implements the paper's Remark "Description of Step 5.0" (Technical
//! Overview, \S{}2.1) for integer constraints.  The bitified terminal claim
//!
//! ```text
//! < u1 (x) u2, bits > = mu   in F_Q,     u1 in F_Q^{2^t},  u2 in F_Q^{2^s},
//! ```
//!
//! lives over the large fingerprint field `F_Q`, which violates the
//! exponent-fold no-wrap condition.  The prover therefore sends the exact
//! integer evaluation
//!
//! ```text
//! mu' = < lift(u1) (x) lift(u2), bits >   in Z,     0 <= mu' < d * Q^2,
//! ```
//!
//! where `lift` is the canonical representative in `[0, Q)` and
//! `d = 2^{t+s}` is the committed cell count.  The verifier checks
//! `mu' = mu (mod Q)` and the magnitude bound, samples a fresh prime `q'`
//! below the no-wrap boundary, and both sides continue with the tensor
//! *factors* reduced modulo `q'` — reduction is a ring homomorphism, so
//! `lift(u1) (x) lift(u2) mod q' = (lift(u1) mod q') (x) (lift(u2) mod q')`
//! entrywise, and the claim keeps the tensor shape the low-entropy grand
//! product needs.
//!
//! Lifting the factors separately (rather than the entries of the reduced
//! tensor, whose bound would be `d * Q` as in the remark's statement) is
//! what preserves the tensor decomposition; the price is the `d * Q^2`
//! magnitude bound used in the soundness accounting: a false claim leaves
//! an integer defect below `2 * d * Q^2 <= 2^282`, which has at most two
//! prime divisors in the `[2^112, 2^113)` reduction interval.

use num_bigint::BigUint;
use num_traits::Zero;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Exact integer evaluation `mu' = sum_{b,c} rw[b] * cw[c] * bit(b,c)`.
///
/// `rows[c]` packs the `2^t` folded-row bits of clear column `c`
/// little-endian into `u64` words, the commitment's own layout.  Row
/// weights are canonical residues below the fingerprint prime, so each
/// column sum fits `2^{128+t}` and is accumulated as a 192-bit integer
/// before one `BigUint` multiply per column.
pub fn step50_integer_lift(
    rows: &[Vec<u64>],
    row_weights: &[u128],
    col_weights: &[u128],
) -> BigUint {
    assert_eq!(rows.len(), col_weights.len(), "one bit column per weight");

    let column_term = |(column, words): (usize, &Vec<u64>)| -> BigUint {
        let weight = col_weights[column];
        if weight == 0 {
            return BigUint::zero();
        }
        let mut low = 0u128;
        let mut high = 0u64;
        for (word_index, &word) in words.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let bit = remaining.trailing_zeros() as usize;
                let row = word_index * u64::BITS as usize + bit;
                let (sum, carry) = low.overflowing_add(row_weights[row]);
                low = sum;
                high += u64::from(carry);
                remaining &= remaining - 1;
            }
        }
        let mut column_sum = BigUint::from(high) << 128;
        column_sum += low;
        column_sum * weight
    };

    #[cfg(feature = "parallel")]
    {
        rows.par_iter()
            .enumerate()
            .map(column_term)
            .reduce(BigUint::zero, |left, right| left + right)
    }
    #[cfg(not(feature = "parallel"))]
    {
        rows.iter()
            .enumerate()
            .map(column_term)
            .fold(BigUint::zero(), |left, right| left + right)
    }
}

/// Exclusive magnitude bound `d * Q^2` on an admissible `mu'`.
pub fn step50_mu_prime_bound(cells: usize, q: u128) -> BigUint {
    let q = BigUint::from(q);
    BigUint::from(cells) * &q * &q
}

/// Whether `mu'` is consistent with the mod-`Q` claim: `mu' = mu (mod Q)`
/// and `mu' < d * Q^2`.
pub fn step50_accepts_lift(mu_prime: &BigUint, mu: u128, q: u128, cells: usize) -> bool {
    if mu_prime >= &step50_mu_prime_bound(cells, q) {
        return false;
    }
    mu_prime % BigUint::from(q) == BigUint::from(mu)
}

/// Reduces the tensor factors and the lifted claim modulo `q'`.
pub fn step50_reduce(
    row_weights: &[u128],
    col_weights: &[u128],
    mu_prime: &BigUint,
    q_prime: u128,
) -> (Vec<u128>, Vec<u128>, u128) {
    let reduce = |weights: &[u128]| weights.iter().map(|&w| w % q_prime).collect::<Vec<_>>();
    let claimed = biguint_mod_u128(mu_prime, q_prime);
    (reduce(row_weights), reduce(col_weights), claimed)
}

fn biguint_mod_u128(value: &BigUint, modulus: u128) -> u128 {
    let residue = value % BigUint::from(modulus);
    let digits = residue.iter_u64_digits().collect::<Vec<_>>();
    match digits.len() {
        0 => 0,
        1 => u128::from(digits[0]),
        2 => u128::from(digits[0]) | (u128::from(digits[1]) << 64),
        _ => unreachable!("a residue modulo a u128 modulus has at most two u64 digits"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense_lift(bits: &[Vec<u64>], rw: &[u128], cw: &[u128]) -> BigUint {
        let mut total = BigUint::zero();
        for (column, words) in bits.iter().enumerate() {
            for row in 0..rw.len() {
                if (words[row / 64] >> (row % 64)) & 1 == 1 {
                    total += BigUint::from(rw[row]) * BigUint::from(cw[column]);
                }
            }
        }
        total
    }

    #[test]
    fn integer_lift_matches_the_dense_reference_and_reduces_consistently() {
        let q: u128 = (1u128 << 127) + 0x2d; // any odd 128-bit modulus works here
        let rw: Vec<u128> = (0..128u128)
            .map(|index| (q - 1).wrapping_sub(index * 0x1234_5678_9abc_def1) % q)
            .collect();
        let cw: Vec<u128> = (0..4u128).map(|index| (index * 0x0fed_cba9) % q).collect();
        let rows: Vec<Vec<u64>> = (0..4)
            .map(|column| vec![0x8000_0001_0000_0111u64 << column, u64::MAX >> column])
            .collect();

        let lifted = step50_integer_lift(&rows, &rw, &cw);
        assert_eq!(lifted, dense_lift(&rows, &rw, &cw));

        let mu = biguint_mod_u128(&lifted, q);
        assert!(step50_accepts_lift(&lifted, mu, q, rw.len() * cw.len()));
        assert!(!step50_accepts_lift(&lifted, mu ^ 1, q, rw.len() * cw.len()));
        assert!(!step50_accepts_lift(
            &(&lifted + step50_mu_prime_bound(rw.len() * cw.len(), q)),
            mu,
            q,
            rw.len() * cw.len(),
        ));

        let q_prime: u128 = (1u128 << 112) + 0x1d; // odd; primality irrelevant here
        let (rw_reduced, cw_reduced, claimed) = step50_reduce(&rw, &cw, &lifted, q_prime);
        assert!(rw_reduced.iter().all(|&w| w < q_prime));
        assert!(cw_reduced.iter().all(|&w| w < q_prime));
        assert_eq!(claimed, biguint_mod_u128(&dense_lift(&rows, &rw, &cw), q_prime));
        // The reduced-factor tensor evaluates to the reduced claim mod q'.
        let reduced_eval = dense_lift(&rows, &rw_reduced, &cw_reduced);
        assert_eq!(biguint_mod_u128(&reduced_eval, q_prime), claimed);
    }
}
