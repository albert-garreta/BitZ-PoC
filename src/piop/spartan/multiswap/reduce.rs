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

use field::RingOps;
use field::{CanonicalCodec, CtMask, CtOrd, CtSelect, IntegerOps, PreparedDivisor, Uint, WideMul};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Exact lift storage: 256 product bits plus 64 bits of public cell-count
/// headroom. A shape with fewer than 2^64 cells cannot overflow this type.
pub fn step50_integer_lift(
    rows: &[Vec<u64>],
    row_weights: &[u128],
    col_weights: &[u128],
) -> Uint<5> {
    assert_eq!(rows.len(), col_weights.len(), "one bit column per weight");
    row_weights
        .len()
        .checked_mul(col_weights.len())
        .expect("lift cell count exceeds usize");
    assert!(
        rows.iter()
            .all(|words| words.len() == row_weights.len().div_ceil(64)),
        "packed column has the wrong public length"
    );

    let column_term = |(column, words): (usize, &Vec<u64>)| -> Uint<5> {
        // The loop and its addresses depend only on the public shape. In
        // particular, zero words and zero weights execute the same work.
        let mut low = 0u128;
        let mut high = 0u64;
        for (row, &weight) in row_weights.iter().enumerate() {
            let bit = CtMask::from_lsb(words[row / 64] >> (row % 64));
            let selected = u64::ct_select(&0, &(weight as u64), bit) as u128
                | ((u64::ct_select(&0, &((weight >> 64) as u64), bit) as u128) << 64);
            let (sum, carry) = low.overflowing_add(selected);
            low = sum;
            high += u64::from(carry);
        }
        let sum = Uint::from_words([low as u64, (low >> 64) as u64, high]);
        let product = IntegerOps.mul_wide(&sum, &Uint::<2>::from(col_weights[column]));
        // An exact 3-by-2-limb product occupies exactly five limbs.
        *product.checked_resize_ct::<5>().value()
    };

    #[cfg(feature = "parallel")]
    {
        rows.par_iter()
            .enumerate()
            .map(column_term)
            .reduce(|| Uint::ZERO, |left, right| left.wrapping_add(&right))
    }
    #[cfg(not(feature = "parallel"))]
    {
        rows.iter()
            .enumerate()
            .map(column_term)
            .fold(Uint::ZERO, |left, right| left.wrapping_add(&right))
    }
}

/// Exclusive magnitude bound `cells * q^2`, with the same five-limb bound.
pub fn step50_mu_prime_bound(cells: usize, q: u128) -> Uint<5> {
    let square = IntegerOps.mul_wide(&Uint::<2>::from(q), &Uint::<2>::from(q));
    let square = *square.checked_resize_ct::<4>().value();
    *IntegerOps
        .mul_wide(&square, &Uint::<1>::from(cells as u64))
        .checked_resize_ct::<5>()
        .value()
}

/// Validate the now-public proof message against its magnitude and mod-Q claim.
pub fn step50_accepts_lift(mu_prime: &Uint<5>, mu: u128, q: u128, cells: usize) -> bool {
    mu_prime
        .ct_lt(&step50_mu_prime_bound(cells, q))
        .declassify()
        && lift_mod_u128(mu_prime, q) == mu
}

pub fn step50_reduce(
    row_weights: &[u128],
    col_weights: &[u128],
    mu_prime: &Uint<5>,
    q_prime: u128,
) -> (Vec<u128>, Vec<u128>, u128) {
    let divisor = PreparedDivisor::new(Uint::<2>::from(q_prime)).expect("nonzero public modulus");
    let reduce = |weights: &[u128]| {
        weights
            .iter()
            .map(|&w| {
                let (_, r) = divisor.div_rem_ct(&Uint::<2>::from(w));
                u128::from(r)
            })
            .collect()
    };
    let (_, claimed) = divisor.div_rem_ct(mu_prime);
    (
        reduce(row_weights),
        reduce(col_weights),
        u128::from(claimed),
    )
}

pub fn encode_integer_lift(value: &Uint<5>) -> [u8; 40] {
    let mut bytes = [0; 40];
    IntegerOps.encode_into(value, &mut bytes);
    bytes
}

fn lift_mod_u128(value: &Uint<5>, modulus: u128) -> u128 {
    let divisor = PreparedDivisor::new(Uint::<2>::from(modulus)).expect("nonzero public modulus");
    u128::from(divisor.div_rem_ct(value).1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use num_traits::Zero;

    fn oracle(value: &Uint<5>) -> BigUint {
        BigUint::from_bytes_le(&encode_integer_lift(value))
    }
    fn biguint_mod_u128(value: &BigUint, modulus: u128) -> u128 {
        let digits = (value % BigUint::from(modulus)).to_u64_digits();
        digits.first().copied().unwrap_or(0) as u128
            | ((digits.get(1).copied().unwrap_or(0) as u128) << 64)
    }

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
        assert_eq!(oracle(&lifted), dense_lift(&rows, &rw, &cw));

        let mu = lift_mod_u128(&lifted, q);
        assert!(step50_accepts_lift(&lifted, mu, q, rw.len() * cw.len()));
        assert!(!step50_accepts_lift(
            &lifted,
            mu ^ 1,
            q,
            rw.len() * cw.len()
        ));
        assert!(!step50_accepts_lift(
            &lifted.wrapping_add(&step50_mu_prime_bound(rw.len() * cw.len(), q)),
            mu,
            q,
            rw.len() * cw.len(),
        ));

        let q_prime: u128 = (1u128 << 112) + 0x1d; // odd; primality irrelevant here
        let (rw_reduced, cw_reduced, claimed) = step50_reduce(&rw, &cw, &lifted, q_prime);
        assert!(rw_reduced.iter().all(|&w| w < q_prime));
        assert!(cw_reduced.iter().all(|&w| w < q_prime));
        assert_eq!(
            claimed,
            biguint_mod_u128(&dense_lift(&rows, &rw, &cw), q_prime)
        );
        // The reduced-factor tensor evaluates to the reduced claim mod q'.
        let reduced_eval = dense_lift(&rows, &rw_reduced, &cw_reduced);
        assert_eq!(biguint_mod_u128(&reduced_eval, q_prime), claimed);
    }
}
