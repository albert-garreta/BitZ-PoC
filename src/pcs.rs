//! Shared binary-matrix layout, generator validation, and reference-prime arithmetic.
//! The integer PCS and its exact exponent bound are implemented in `crate::wfbitz`.

use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::utils::cfg_iter_mut;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// A `2^row_vars × 2^col_vars` matrix of `word_bits`-bit integers.
///
/// Row variables are folded with integer weights; column variables are used
/// for the final evaluation with arbitrary field weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegerMatrixLayout {
    /// Number of folded row variables; there are `2^row_vars` rows.
    pub row_vars: usize,
    /// Number of final column variables; there are `2^col_vars` columns.
    pub col_vars: usize,
    /// Word width `W`: every data cell is an integer in `[0, 2^W)`.
    pub word_bits: usize,
}

impl IntegerMatrixLayout {
    /// Number of rows `2^row_vars`.
    pub fn rows(&self) -> usize {
        1usize << self.row_vars
    }

    /// Number of columns `2^col_vars`.
    pub fn cols(&self) -> usize {
        1usize << self.col_vars
    }

    /// Number of data cells `2^(row_vars + col_vars)`.
    pub fn cells(&self) -> usize {
        1usize << self.row_vars << self.col_vars
    }

    /// Flat index of cell `(b, c)` in row-major (`b` rows, `c` columns) order.
    pub fn cell_index(&self, b: usize, c: usize) -> usize {
        (b << self.col_vars) | c
    }
}

/// `base^exp` in `GF(2^128)` by square-and-multiply over the bits of `exp`.
///
/// Used both to build the per-branch bases `α^{w_b}` and to read off `α^{v_c}`
/// directly for the completeness check. `exp` is a `u128`, which is exactly the
/// width that must dominate every value `v_c` for the exponent map to be
/// injective.
pub fn gf_pow(base: Gf, mut exp: u128) -> Gf {
    let mut acc = Gf::one();
    let mut sq = base;
    while exp != 0 {
        if exp & 1 == 1 {
            acc = acc * sq;
        }
        sq = sq.square();
        exp >>= 1;
    }
    acc
}

/// The distinct prime factors of `|K^×| = 2^128 − 1` (`K = GF(2^128)`).
/// `2^128 − 1 = 3·5·17·257·641·65537·274177·6700417·67280421310721`, squarefree:
/// the five Fermat primes of `2^32 − 1`, then `641·6700417 = 2^32 + 1` and
/// `274177·67280421310721 = 2^64 + 1`. Used by [`is_generator`].
pub const GF128_ORDER_PRIME_FACTORS: [u128; 9] =
    [3, 5, 17, 257, 641, 65537, 274177, 6700417, 67280421310721];

/// The order of the multiplicative group `K^× = GF(2^128)^×`, i.e. `2^128 − 1`
/// (`= u128::MAX`). When `α` generates `K^×` the exponent map `n ↦ α^n` is
/// injective on `[0, 2^128 − 1)`, so every fold value `v_c < 2^128 − 1` (i.e.
/// any `u128` except `u128::MAX`) is bound faithfully — the *rigorous*
/// replacement for the previous `2^127` heuristic guard.
pub const GF128_MULT_ORDER: u128 = u128::MAX;

/// Does `α` generate the full multiplicative group `K^×` (order `2^128 − 1`)?
///
/// Standard primitive-element test: `α ∉ {0, 1}` and `α^{(2^128−1)/p} ≠ 1` for
/// every prime `p ∣ 2^128 − 1` ([`GF128_ORDER_PRIME_FACTORS`]). A generator
/// makes `α^{v_c} = root_c` injective on the bounded fold values, so the integer
/// binding is faithful in `GF(2^128)` itself — no Mersenne field needed. About
/// `∏(1 − 1/p) ≈ 49%` of `K^×` qualifies, so a Fiat–Shamir `α` passes after ~2
/// resamples on average; the verifier rejects a non-generator challenge.
#[allow(clippy::arithmetic_side_effects)] // `/ p`: every p is a nonzero const factor.
pub fn is_generator(alpha: Gf) -> bool {
    if alpha == Gf::zero() || alpha == Gf::one() {
        return false;
    }
    GF128_ORDER_PRIME_FACTORS
        .iter()
        .all(|&p| gf_pow(alpha, GF128_MULT_ORDER / p) != Gf::one())
}

/// The smallest `α = Gf::from(n)`, `n ≥ 2`, that generates `K^×` — a convenient
/// deterministic generator for instantiating the argument in tests/benches. The
/// protocol may use any Fiat–Shamir `α` accepted by [`is_generator`].
pub fn smallest_generator() -> Gf {
    (2u128..)
        .map(Gf::from_polynomial_bits)
        .find(|&a| is_generator(a))
        .expect("GF(2^128)^× is cyclic and has generators")
}

/// The fixed ~100-bit read-off prime `q = 2^100 − 15`. Must equal the host
/// prover's projection prime.
pub const FQ_MOD: u128 = (1u128 << 100).wrapping_sub(15);

/// Bit width of the reference prime.
pub const FQ_BITS: usize = 100;

pub use field::Q100Element;

/// Canonical integer arithmetic for the existing static PCS prime.
#[inline]
pub fn fq_add(a: u128, b: u128) -> u128 {
    (Q100Element::from(a) + Q100Element::from(b)).canonical_u128()
}

#[inline]
pub fn fq_sub(a: u128, b: u128) -> u128 {
    (Q100Element::from(a) - Q100Element::from(b)).canonical_u128()
}

#[inline]
pub fn fq_mul(a: u128, b: u128) -> u128 {
    (Q100Element::from(a) * Q100Element::from(b)).canonical_u128()
}

/// Little-endian `eq` table over `𝔽_q`: `out[idx] = ∏_l (bit_l(idx) ? point[l] :
/// 1 − point[l])` with `point[0] ↔ bit 0` (LSB) — matching
/// `crate::poly::utils::build_eq_x_r_vec`, the convention `compute_lifted_evals`
/// uses for the lifted `bar_u`. This makes the F₂ read-off weights agree with the
/// host PIOP's α-combined witness-MLE claim. (Contrast the standalone tests'
/// `eq_table_fq`, which is big-endian.)
pub fn eq_le_table_fq(point: &[Q100Element]) -> Vec<Q100Element> {
    let table_len = u32::try_from(point.len())
        .ok()
        .and_then(|num_vars| 1usize.checked_shl(num_vars))
        .expect("eq_le_table_fq domain must fit into usize");
    let mut table = vec![Q100Element::from_u128(0); table_len];
    table[0] = Q100Element::from_u128(1);

    let mut half = 1usize;
    for challenge in point {
        let active_len = half
            .checked_mul(2)
            .expect("checked equality-table domain cannot overflow");
        let (zero_children, one_children) = table[..active_len].split_at_mut(half);
        cfg_iter_mut!(zero_children)
            .zip(cfg_iter_mut!(one_children))
            .for_each(|(zero, one)| {
                let parent = *zero;
                let one_child = parent * *challenge;
                *zero = parent - one_child;
                *one = one_child;
            });
        half = active_len;
    }
    table
}

#[cfg(test)]
mod eq_le_table_fq_tests {
    use super::*;

    fn direct_eq(point: &[Q100Element], index: usize) -> Q100Element {
        Q100Element::from_u128(
            point
                .iter()
                .enumerate()
                .fold(1u128, |acc, (bit, challenge)| {
                    let factor = if index >> bit & 1 == 1 {
                        challenge.canonical_u128()
                    } else {
                        fq_sub(1, challenge.canonical_u128())
                    };
                    fq_mul(acc, factor)
                }),
        )
    }

    #[test]
    fn empty_input_is_one() {
        assert_eq!(eq_le_table_fq(&[]), vec![Q100Element::from_u128(1)]);
    }

    #[test]
    fn two_coordinates_use_little_endian_order() {
        let point = [Q100Element::from_u128(2), Q100Element::from_u128(3)];
        // Indices 0, 1, 2, 3 represent [00, 10, 01, 11].
        assert_eq!(
            eq_le_table_fq(&point),
            vec![
                Q100Element::from_u128(2),
                Q100Element::from_u128(FQ_MOD - 4),
                Q100Element::from_u128(FQ_MOD - 3),
                Q100Element::from_u128(6)
            ]
        );
    }

    #[test]
    fn non_boolean_table_matches_direct_formula_and_sums_to_one() {
        let point = [
            Q100Element::from_u128(0),
            Q100Element::from_u128(1),
            Q100Element::from_u128(FQ_MOD - 1),
            Q100Element::from_u128(123_456_789),
        ];
        let table = eq_le_table_fq(&point);
        for (index, &evaluation) in table.iter().enumerate() {
            assert_eq!(evaluation, direct_eq(&point, index), "entry {index}");
        }
        assert_eq!(
            table
                .iter()
                .fold(0u128, |acc, value| fq_add(acc, value.canonical_u128())),
            1
        );
    }
}
