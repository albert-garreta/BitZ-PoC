//! Shared core of the integer-MLE-evaluation protocol over an `F_2`
//! commitment (char2-fieldswitch note §9): the shape/field math, the
//! `GF(2^128)` exponent-fold arithmetic, the forest-leaf builders, the
//! mod-`q` limb chunking, and the SHA bit-tensor layout — everything the
//! opener consumes. The opener itself is the flock-backed ring-switch +
//! recursive Ligerito pipeline in [`crate::ligerito_flock`] (the only PCS
//! opener in this crate).
//!
//! # Mechanism
//!
//! The data is a `2^t × 2^s` matrix of `W`-bit words `D[(b,c)]`. The row fold
//! with integer weights `w_b` produces, per column `c`, the *integer*
//! `v_c = Σ_{b∈{0,1}^t} w_b · D[(b,c)]`, and the final read-off over the column
//! variables is `P(r) = G_r · Σ_c e_c · v_c`.
//!
//! The fold is carried out *in the exponent* of `K = GF(2^128)`, where the
//! integer addition becomes a product:
//! `α^{v_c} = ∏_b α^{w_b·D[(b,c)]} = ∏_{b,j} [bit_{(b,c),j} ? α^{w_b·2^j} : 1]`,
//! each factor affine in one committed bit. A `GF(2^128)` GKR grand product
//! (the [merged forest][crate::merged_forest]) binds `α^{v_c}` to the committed
//! bits; a de-black-boxing pre-sumcheck strips the α-power weight factor; and
//! the residual bit-MLE evaluation claim is discharged by the ring-switch +
//! Ligerito opener.
//!
//! Two fields are in play, deliberately:
//! * `K = GF(2^128)` binds `v_c` to the committed bits *in the exponent*; there
//!   is no parity collapse because `v_c` is sent and reused as an *integer* and
//!   is never reduced mod 2.
//! * the *evaluation field* carries the final column combination `Σ_c e_c v_c`
//!   on those sent integers.
//!
//! Soundness needs `max_c v_c < ord(α)` (≈ `2^128`) so that the exponent map is
//! injective on the values that occur (no wraparound); see [`max_fold_magnitude`].

use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;
use crate::transcript::traits::Transcript;

use crate::utils::{cfg_chunks_mut, cfg_into_iter, cfg_iter, cfg_iter_mut};

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
/// injective (cf. [`max_fold_magnitude`]).
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

/// The shared row-bit weight vector `q_rowbit` over the `(b, j)` leaf index, at the
/// forest's shared reduction point `ρ`:
/// `q_rowbit[(b<<log₂W)|j] = eq((b,j),ρ) · (α^{w_b·2^j} − 1)`.
///
/// It is the same for every column, so the per-column leaf claims
/// `ℓ_c − 1 = ⟨q_rowbit, bits_c⟩` reduce, via the de-black-boxing pre-sumcheck,
/// to a residual bit-MLE evaluation the ring-switch + Ligerito opener discharges.
/// (`ℓ_c − 1` because `Σ_{(b,j)} eq((b,j),ρ) = 1` and each leaf is
/// `1 + bit·(α^{w_b·2^j} − 1)`.)
pub fn row_bit_weights(
    p: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    rho: &[Gf],
) -> Vec<Gf> {
    let mut weights = build_eq_x_r_vec(rho, &()).expect("shared reduction point is non-empty");
    // The `2^t` ~q_bits-wide exponentiations dominate this table (it is the
    // verifier's O(2^t) step and was 18% of the mod-q PROVE at nv=16):
    // fixed-base comb (≈8× fewer mults than square-and-multiply) + parallel.
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, alpha.into(), 8);
    let one = Gf::one();
    cfg_chunks_mut!(weights, p.word_bits)
        .enumerate()
        .for_each(|(b, slots)| {
            let w = row_weights[b];
            let mut power = comb.pow_public(&field::Uint::from_words([w as u64, (w >> 64) as u64]));
            // Reuse the equality table as output and the previous bit's
            // power: alpha^(w*2^(j+1)) = (alpha^(w*2^j))^2.
            let last = slots.len() - 1;
            for (j, slot) in slots.iter_mut().enumerate() {
                *slot *= power - one;
                if j != last {
                    power = power.square();
                }
            }
        });
    weights
}

#[cfg(test)]
mod row_bit_weight_tests {
    use super::*;

    #[test]
    fn flat_powers_match_independent_exponentiation() {
        let weights = [0, 1, 2, u128::MAX, 0x5a13_c479_890b_246f];
        for word_bits in [1, 2, 8, 32, 128] {
            let p = IntegerMatrixLayout {
                row_vars: 3,
                col_vars: 1,
                word_bits,
            };
            for alpha in [Gf::zero(), Gf::one(), Gf::from([0x99887766, 0x12345678])] {
                let powers = chunk_pow2_flat(&p, &weights, alpha);
                for (row, &weight) in weights.iter().enumerate() {
                    let base = gf_pow(alpha, weight);
                    for bit in 0..word_bits {
                        assert_eq!(
                            powers.power(row, bit),
                            gf_pow(base, 1u128 << bit),
                            "row={row}, bit={bit}, W={word_bits}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fused_weights_match_independent_powers() {
        for word_bits in [1usize, 2, 8, 32, 128] {
            let p = IntegerMatrixLayout {
                row_vars: 4,
                col_vars: 1,
                word_bits,
            };
            let row_weights: Vec<_> = (0..p.rows())
                .map(|b| match b {
                    0 => 0,
                    1 => 1,
                    2 => u128::MAX,
                    _ => (b as u128).wrapping_mul(0xa3b1_770d_41af_89c2_8211_7759_1234_0fab),
                })
                .collect();
            let rho: Vec<_> = (0..p.row_vars + word_bits.trailing_zeros() as usize)
                .map(|i| Gf::from([i as u64 + 13, i as u64 + 31]))
                .collect();
            let eq = build_eq_x_r_vec(&rho, &()).unwrap();
            for alpha in [Gf::zero(), Gf::one(), Gf::from([0x99887766, 0x12345678])] {
                let bases: Vec<_> = row_weights.iter().map(|&w| gf_pow(alpha, w)).collect();
                let expected: Vec<_> = eq
                    .iter()
                    .enumerate()
                    .map(|(i, &e)| {
                        e * (gf_pow(bases[i / word_bits], 1u128 << (i % word_bits)) - Gf::one())
                    })
                    .collect();
                assert_eq!(row_bit_weights(&p, &row_weights, alpha, &rho), expected);
            }
        }
    }
}

/// `max_c v_c` — the magnitude that must stay below `ord(α)` (≈ `2^128`) for the
/// exponent binding `α^{v_c}` to be injective on the values that occur.
pub fn max_fold_magnitude(v: &[u128]) -> u128 {
    v.iter().copied().max().unwrap_or(0)
}

/// The general §9 read-off `P(r) = G_r · Σ_c e_c · v_c` over an arbitrary
/// **evaluation ring** `R`. The bound integers `v_c` are lifted by the canonical
/// map `R::from`; the column weights `e_c` and the global scalar
/// `G_r = ∏_i(1 − r'_i)` are caller-supplied `R`-elements. `R` must **not** be of
/// characteristic 2 — lifting an integer `v_c` into `GF(2^128)` would collapse to
/// its parity (the very obstruction the exponent fold avoids), so the evaluation
/// field is a *separate*, char-≠2 field (e.g. a Mersenne prime field), distinct
/// from the binding field `K`. The `R = ℤ` (`u128`), `G_r = 1` specialization
/// is the plain integer read-off.
#[allow(clippy::arithmetic_side_effects)] // caller's evaluation-ring operators
pub fn final_eval_ring<R>(v: &[u128], col_weights: &[R], g_r: R) -> R
where
    R: Copy + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let mut acc = R::from(0u128);
    for (&vc, &ec) in v.iter().zip(col_weights.iter()) {
        acc = acc + ec * R::from(vc);
    }
    g_r * acc
}

/// The direct integer evaluation `Σ_{b,c} w_b · e_c · D[(b,c)]`, computed without
/// folding — the completeness reference for the exponent-fold read-off.
#[allow(clippy::arithmetic_side_effects)] // Reference arithmetic; bounded instance.
pub fn direct_eval_int(
    p: &IntegerMatrixLayout,
    data: &[u128],
    row_weights: &[u128],
    col_weights: &[u128],
) -> u128 {
    let mut acc = 0u128;
    for b in 0..p.rows() {
        for c in 0..p.cols() {
            acc += row_weights[b] * col_weights[c] * data[p.cell_index(b, c)];
        }
    }
    acc
}

// =====================================================================
// Committed-bit access over the bit-packed column store. The bits are laid
// out as a `2^s × row_len` single-bit matrix `M` (`row_len = 2^t · W`,
// `M[c][(b ≪ log₂W) | j] = bit_{(b,c),j}`), 64 columns per `u64` lane — the
// layout the opener's forest-leaf builders and the Ligerito packer consume.

/// The committed bit `M[c][i]` from the bit-packed message store: column `c`
/// lives in word `c/64`, lane `c%64`.
#[inline]
pub(crate) fn packed_col_bit(packed_cols: &[Vec<u64>], c: usize, i: usize) -> bool {
    (packed_cols[c >> 6][i] >> (c & 63)) & 1 == 1
}

// =====================================================================
// Multiplicative-group generators of `K = GF(2^128)`. A generator `α` makes
// the exponent binding `α^{v_c} = root_c` injective on the bounded fold values
// `v_c < 2^128 − 1`, so the integer binding is faithful in `GF(2^128)` itself
// (no Mersenne field needed). The end-to-end argument then sends the short
// integer vector `v` (the `2^s` folded values) and reads the evaluation off it
// in the clear: `P(r) = G_r · Σ_c e_c · v_c`.

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

// =====================================================================
// Field-valued MLE evaluation modulo a prime `q` (X-note §6: `sec:chunk` +
// `sec:modq`) — a genuine `MLE[INT(D)](r) = y ∈ 𝔽_q` opening, not just a
// small-integer-weighted fold.
//
// The relation is `y = Σ_{b,c} eq(b,r₁)·eq(c,r₂)·INT(D(b,c)) (mod q)`. Lift the eq
// weights to their canonical integer reps `w_b := eq(b,r₁) mod q ∈ [0,q)` (≤ `q_bits`
// bits) and `w′_c := eq(c,r₂) ∈ 𝔽_q`; then `y ≡ Σ_{b,c} w_b·w′_c·INT(D(b,c))`. The
// row weights `w_b` are now ~`q_bits` ≈ 100-bit, far too wide to ride one exponent
// (`v_c = Σ_b w_b·INT(D) < ord α = 2^128−1` would overflow). The fix is to **chunk
// the row weights only**: with `c_w = 127 − t − W` and `L = ⌈q_bits / c_w⌉`, write
// `w_b = Σ_l W_b^{(l)}·2^{c_w·l}` (`W_b^{(l)} ∈ [0, 2^{c_w})`) and fold each limb on
// its own,
//   `u_c^{(l)} = Σ_b W_b^{(l)}·INT(D(b,c)) ∈ [0, 2^{c_w+t+W}) = [0, 2^127)`,
// which binds injectively in `K=GF(2^128)` exactly as the small-weight fold does.
// The wide value is reassembled **in the clear** in `𝔽_q`:
//   `y = Σ_c w′_c · Σ_l 2^{c_w·l}·u_c^{(l)} (mod q)`,
// with the column weights used at full width, unchunked.
//
// **Key reuse.** The `(l,c)` chunk-fold forest is structurally a batched `(ℓ,c)`
// forest with `L` "virtual columns" that all reference the SAME committed data `D`
// (only the per-chunk row-weight table differs). So the `F_2` commitment is
// **unchanged**, the forest GKR, the shared reduction point `ρ`,
// [`row_bit_weights`], and the ring-switch + Ligerito opening all carry over; the
// genuinely new code is (a) deriving + chunking the weights ([`mod_q_chunk_width`],
// [`chunk_row_weights`]), (b) the mod-`q` recombination read-off
// ([`recombine_read_off`]), (c) the per-chunk cleartext range checks.
//
// **Evaluation field.** Generic over the same char-≠2 ring `R` as
// [`final_eval_ring`]; the wired realisation (test code) is a concrete 100-bit prime
// `q = 2^100 − 15`. A Mersenne `𝔽_{2^127−1}` is the drop-in alternative (weights
// ~127-bit, `L` one larger). `R` must be char ≠ 2 — lifting an integer into
// `GF(2^128)` collapses to parity, the very obstruction the exponent fold avoids.

// ─────────────────────────────────────────────────────────────────────────
// Host-prover bridge: integer-SHA-over-F₂ 𝔽_q read-off plumbing.
// ─────────────────────────────────────────────────────────────────────────
//
// A host integer prover commits its binary witness over F₂ and discharges the
// resulting `𝔽_q` witness-MLE claim through the Ligerito opener. Its eval field
// `F` may be a *config-based* `MontyField` over a fixed ~100-bit prime, while
// the mod-`q` read-off works in a *config-free* `R: From<u128>+Add+Mul`. Both
// share the SAME modulus `q = 2^100 − 15`; convert `F ↔ Q100Element` at the boundary via
// the canonical integer ([`ProjectCanonicalU128`]).

/// The fixed ~100-bit read-off prime `q = 2^100 − 15`. Must equal the host
/// prover's projection prime.
pub const FQ_MOD: u128 = (1u128 << 100).wrapping_sub(15);
/// `⌈log₂ q⌉` — the chunk-count input for the mod-`q` opening (`L = 1` for SHA).
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


/// In-place 64×64 bit-matrix transpose (Hacker's Delight 7-3, widened to 64):
/// output word `j`'s bit `k` = input word `k`'s bit `j`.
#[allow(clippy::arithmetic_side_effects)]
fn transpose64(m: &mut [u64; 64]) {
    let mut j: usize = 32;
    let mut mask: u64 = 0x0000_0000_FFFF_FFFF;
    while j != 0 {
        let mut k: usize = 0;
        while k < 64 {
            let t = ((m[k] >> j) ^ m[k | j]) & mask;
            m[k | j] ^= t;
            m[k] ^= t << j;
            k = ((k | j) + 1) & !j;
        }
        j >>= 1;
        mask ^= mask << j;
    }
}

/// The mod-`q` row-weight **chunk width** `c_w = 127 − t − W` (X-note §6.1,
/// `eq:chunkbound` at `k=128`): the largest limb size for which every per-chunk fold
/// `u_c^{(l)} = Σ_b W_b^{(l)}·INT(D(b,c))` stays `< 2^{c_w+t+W} = 2^127`, so it binds
/// injectively in `K=GF(2^128)` (generator order `2^128−1 > 2^127`). Asserts
/// `t + W ≤ 126` so `c_w ∈ [1, 126]`.
pub fn mod_q_chunk_width(p: &IntegerMatrixLayout) -> usize {
    let tw = p.row_vars.wrapping_add(p.word_bits);
    assert!(
        tw <= 126,
        "mod-q chunking needs t + W ≤ 126 (c_w = 127 − t − W ≥ 1); got t={}, W={}",
        p.row_vars,
        p.word_bits
    );
    127usize.wrapping_sub(tw)
}

/// The number of weight chunks `L = ⌈q_bits / c_w⌉` (X-note §6.3) for an evaluation
/// modulus of bit-length `q_bits = ⌈log₂q⌉`. `L = 1` (no chunking) exactly when
/// `q_bits ≤ c_w`, i.e. `t + W ≤ 127 − q_bits` — the headroom window (`t + W < 28`
/// for a 100-bit prime).
pub fn mod_q_num_chunks(p: &IntegerMatrixLayout, q_bits: usize) -> usize {
    q_bits.div_ceil(mod_q_chunk_width(p)).max(1)
}

/// A public map certifies that every derived cell is smaller than 2^value_bits.
/// The physical stride remains a power of two, including at P=128.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VirtualWordBound {
    layout: IntegerMatrixLayout,
    value_bits: usize,
    map_digest: [u8; 32],
}
impl VirtualWordBound {
    pub(crate) fn new<M: circuit::linear_map::binary::VirtualMap>(
        layout: IntegerMatrixLayout,
        value_bits: usize,
        map: &M,
    ) -> Result<Self, ()> {
        if !layout.word_bits.is_power_of_two()
            || layout.word_bits > 128
            || value_bits == 0
            || value_bits > layout.word_bits
        {
            return Err(());
        }
        let bits = layout
            .row_vars
            .checked_add(layout.col_vars)
            .and_then(|v| v.checked_add(layout.word_bits.ilog2() as usize))
            .ok_or(())?;
        let cells = 1usize
            .checked_shl(u32::try_from(bits).map_err(|_| ())?)
            .ok_or(())?;
        if map.rows() != cells {
            return Err(());
        }
        // Structural support, never the supplied witness, establishes this bound.
        if map
            .output_word_bits(layout.word_bits)
            .is_none_or(|bound| bound > value_bits)
        {
            for column in 0..map.cols() {
                for row in map.column_rows(column).ok_or(())? {
                    if row >= cells || row % layout.word_bits >= value_bits {
                        return Err(());
                    }
                }
            }
        }
        Ok(Self {
            layout,
            value_bits,
            map_digest: map.digest(),
        })
    }
    pub(crate) const fn value_bits(&self) -> usize {
        self.value_bits
    }
    pub(crate) fn matches_layout(&self, p: &IntegerMatrixLayout) -> bool {
        self.layout == *p
    }
    pub(crate) fn matches_map<M: circuit::linear_map::binary::VirtualMap>(
        &self,
        p: &IntegerMatrixLayout,
        map: &M,
    ) -> bool {
        self.matches_layout(p) && self.map_digest == map.digest()
    }
}

/// A validated base-`2^c_w` decomposition of one public mod-`q` row-weight
/// vector.
///
/// The fields are deliberately private: every constructor and range setter
/// validates the public geometry and canonical weight bounds, so the Ligerito
/// core can consume already-prepared limbs without reopening a path for
/// malformed chunk counts, row lengths, or high bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModQWeightChunks {
    chunks: Vec<Vec<u128>>,
    row_count: usize,
    chunk_width: usize,
    q_bits: usize,
    padding: Option<VirtualWordBound>,
}

/// A validated, canonical source of public mod-`q` row weights.
///
/// The virtual opening consumes one base-`2^c_w` limb at a time.  Dense
/// callers can lend an existing limb through [`Self::with_chunk`], while
/// generated callers use the default implementation to materialize and drop
/// exactly one `2^t`-row limb.  This keeps the opening transcript identical to
/// [`ModQWeightChunks`] without requiring all `L` limbs to coexist.
pub(crate) trait ModQWeightSource: Sync {
    fn padding_bound(&self) -> Option<&VirtualWordBound> {
        None
    }

    /// Number of canonical row weights (`2^t`).
    fn row_count(&self) -> usize;

    /// Limb width `c_w = 127 - t - W`.
    fn chunk_width(&self) -> usize;

    /// Number of limbs `L = ceil(q_bits / c_w)`.
    fn chunk_count(&self) -> usize;

    /// Bit length against which every canonical weight is validated.
    fn q_bits(&self) -> usize;

    /// Return canonical row `row` in `[0, 2^q_bits)`.
    fn canonical_weight(&self, row: usize) -> Option<u128>;

    /// Lend limb `chunk_index` to one protocol pass.
    ///
    /// Generated sources inherit this implementation: it evaluates the
    /// canonical source directly into one temporary limb, calls `consume`, and
    /// drops that limb before the next one is produced.
    #[allow(clippy::arithmetic_side_effects)]
    fn with_chunk<R>(
        &self,
        chunk_index: usize,
        consume: impl FnOnce(&[u128]) -> R,
    ) -> Result<R, ()> {
        if chunk_index >= self.chunk_count() {
            return Err(());
        }
        let shift = self.chunk_width().checked_mul(chunk_index).ok_or(())?;
        let remaining_bits = self.q_bits().checked_sub(shift).ok_or(())?;
        let limb_bits = self.chunk_width().min(remaining_bits);
        let limb_mask = (1_u128 << limb_bits).wrapping_sub(1);
        let canonical_bound = 1_u128
            .checked_shl(u32::try_from(self.q_bits()).map_err(|_| ())?)
            .ok_or(())?;
        let mut chunk = vec![0_u128; self.row_count()];
        cfg_iter_mut!(chunk, 256)
            .enumerate()
            .try_for_each(|(row, limb)| {
                let weight = self.canonical_weight(row).ok_or(())?;
                if weight >= canonical_bound {
                    return Err(());
                }
                *limb = (weight >> shift) & limb_mask;
                Ok(())
            })?;
        Ok(consume(&chunk))
    }
}

/// Geometry-checked canonical row-weight generator.
///
/// This is the small adapter used by protocol layers whose equality weights
/// are already represented by compact tensor factors.  The callback evaluates
/// one canonical row on demand; [`ModQWeightSource::with_chunk`] turns it into
/// one temporary limb without ever reconstructing a dense equality table or
/// all limbs.  The callback must be a pure function of `row`, since statement
/// absorption and each limb pass intentionally reevaluate it.
pub(crate) struct GeneratedModQWeightSource<F> {
    canonical_weight: F,
    row_count: usize,
    chunk_width: usize,
    chunk_count: usize,
    q_bits: usize,
}

impl<F> GeneratedModQWeightSource<F> {
    pub(crate) fn new(
        p: &IntegerMatrixLayout,
        q_bits: usize,
        canonical_weight: F,
    ) -> Result<Self, ()> {
        let (row_count, chunk_width, chunk_count) = mod_q_weight_chunk_shape(p, q_bits)?;
        Ok(Self {
            canonical_weight,
            row_count,
            chunk_width,
            chunk_count,
            q_bits,
        })
    }
}

impl<F> ModQWeightSource for GeneratedModQWeightSource<F>
where
    F: Fn(usize) -> Option<u128> + Sync,
{
    fn row_count(&self) -> usize {
        self.row_count
    }

    fn chunk_width(&self) -> usize {
        self.chunk_width
    }

    fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    fn q_bits(&self) -> usize {
        self.q_bits
    }

    fn canonical_weight(&self, row: usize) -> Option<u128> {
        (row < self.row_count)
            .then(|| (self.canonical_weight)(row))
            .flatten()
    }
}

impl ModQWeightChunks {
    pub(crate) fn from_dense_padded<M: circuit::linear_map::binary::VirtualMap>(
        p: &IntegerMatrixLayout,
        weights: &[u128],
        q_bits: usize,
        value_bits: usize,
        map: &M,
    ) -> Result<Self, ()> {
        let padding = VirtualWordBound::new(*p, value_bits, map)?;
        let (row_count, chunk_width, chunk_count) =
            mod_q_weight_chunk_shape_with_width(p, q_bits, value_bits)?;
        if weights.len() != row_count {
            return Err(());
        }
        let mut chunks = Self {
            chunks: vec![vec![0; row_count]; chunk_count],
            row_count,
            chunk_width,
            q_bits,
            padding: Some(padding),
        };
        chunks.set_weight_range(0, weights)?;
        Ok(chunks)
    }

    /// Adopt an owned dense vector directly when the geometry needs exactly
    /// one chunk. This is the production u32 fast path: no zero-fill and no
    /// dense-to-chunk copy are needed.
    pub(crate) fn from_single_chunk(
        p: &IntegerMatrixLayout,
        q_bits: usize,
        weights: Vec<u128>,
    ) -> Result<Self, ()> {
        let (row_count, chunk_width, chunk_count) = mod_q_weight_chunk_shape(p, q_bits)?;
        if chunk_count != 1 || weights.len() != row_count {
            return Err(());
        }
        let bound = 1_u128
            .checked_shl(u32::try_from(q_bits).map_err(|_| ())?)
            .ok_or(())?;
        if weights.iter().any(|&weight| weight >= bound) {
            return Err(());
        }
        Ok(Self {
            chunks: vec![weights],
            row_count,
            chunk_width,
            q_bits,
            padding: None,
        })
    }

    /// Allocate a validated all-zero chunk matrix. Callers can populate
    /// contiguous canonical ranges through [`Self::set_weight_range`] without
    /// constructing a dense weight vector or rescanning generated chunks.
    pub(crate) fn zeroed(p: &IntegerMatrixLayout, q_bits: usize) -> Result<Self, ()> {
        let (row_count, chunk_width, chunk_count) = mod_q_weight_chunk_shape(p, q_bits)?;
        Ok(Self {
            chunks: vec![vec![0_u128; row_count]; chunk_count],
            row_count,
            chunk_width,
            q_bits,
            padding: None,
        })
    }

    /// Set one contiguous range from canonical dense weights, decomposing it
    /// directly into the already-allocated chunk-major representation.
    pub(crate) fn set_weight_range(
        &mut self,
        row_start: usize,
        weights: &[u128],
    ) -> Result<(), ()> {
        let row_end = row_start.checked_add(weights.len()).ok_or(())?;
        if row_end > self.row_count {
            return Err(());
        }
        let bound = 1_u128
            .checked_shl(u32::try_from(self.q_bits).map_err(|_| ())?)
            .ok_or(())?;

        let limb_mask = (1_u128 << self.chunk_width).wrapping_sub(1);
        let mut shift = 0_usize;
        for (chunk_index, chunk) in self.chunks.iter_mut().enumerate() {
            let output = chunk.get_mut(row_start..row_end).ok_or(())?;
            if chunk_index == 0 {
                cfg_iter_mut!(output, 256)
                    .zip(cfg_iter!(weights, 256))
                    .try_for_each(|(limb, weight)| {
                        if *weight >= bound {
                            Err(())
                        } else {
                            *limb = (*weight >> shift) & limb_mask;
                            Ok(())
                        }
                    })?;
            } else {
                cfg_iter_mut!(output, 256)
                    .zip(cfg_iter!(weights, 256))
                    .for_each(|(limb, weight)| *limb = (*weight >> shift) & limb_mask);
            }
            shift = shift.wrapping_add(self.chunk_width);
        }
        Ok(())
    }

    /// Validate and decompose canonical row weights in `[0, 2^q_bits)`.
    pub(crate) fn from_dense(
        p: &IntegerMatrixLayout,
        row_weights_q: &[u128],
        q_bits: usize,
    ) -> Result<Self, ()> {
        let mut chunks = Self::zeroed(p, q_bits)?;
        if row_weights_q.len() != chunks.row_count {
            return Err(());
        }
        chunks.set_weight_range(0, row_weights_q)?;
        Ok(chunks)
    }

    /// Validate an existing chunk-major decomposition.
    ///
    /// Every chunk must have exactly `2^t` rows. Limbs in chunk `l` are
    /// restricted to the remaining `min(c_w, q_bits - c_w*l)` bits. Those
    /// disjoint per-limb bounds already imply reconstruction below
    /// `2^q_bits`, so validation scans each chunk once without rebuilding
    /// every dense row weight.
    #[allow(dead_code)]
    pub(crate) fn from_chunks(
        p: &IntegerMatrixLayout,
        q_bits: usize,
        chunks: Vec<Vec<u128>>,
    ) -> Result<Self, ()> {
        let (row_count, chunk_width, chunk_count) = mod_q_weight_chunk_shape(p, q_bits)?;
        if chunks.len() != chunk_count || chunks.iter().any(|chunk| chunk.len() != row_count) {
            return Err(());
        }

        for (chunk_index, chunk) in chunks.iter().enumerate() {
            let shift = chunk_width.checked_mul(chunk_index).ok_or(())?;
            let limb_bits = chunk_width.min(q_bits.checked_sub(shift).ok_or(())?);
            let limb_bound = 1_u128
                .checked_shl(u32::try_from(limb_bits).map_err(|_| ())?)
                .ok_or(())?;
            if chunk.iter().any(|&limb| limb >= limb_bound) {
                return Err(());
            }
        }

        Ok(Self {
            chunks,
            row_count,
            chunk_width,
            q_bits,
            padding: None,
        })
    }

    /// Chunk-major limbs, `chunks[l][b]`.
    #[allow(dead_code)]
    pub(crate) fn chunks(&self) -> &[Vec<u128>] {
        &self.chunks
    }

    /// Reconstruct the validated canonical row weights in row-major order.
    ///
    /// This is intentionally lazy: statement absorption can bind the same
    /// canonical `u128` sequence accepted by [`Self::from_dense`] without
    /// materializing a second dense row-weight vector beside the chunk-major
    /// representation.
    #[allow(clippy::arithmetic_side_effects)]
    #[allow(dead_code)]
    pub(crate) fn canonical_weights(&self) -> impl ExactSizeIterator<Item = u128> + '_ {
        (0..self.row_count).map(|row| {
            self.chunks
                .iter()
                .enumerate()
                .fold(0_u128, |weight, (chunk_index, chunk)| {
                    weight | (chunk[row] << (self.chunk_width * chunk_index))
                })
        })
    }

    /// Number of row weights in each chunk.
    #[allow(dead_code)]
    pub(crate) const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Limb width `c_w = 127 - t - W`.
    #[allow(dead_code)]
    pub(crate) const fn chunk_width(&self) -> usize {
        self.chunk_width
    }

    /// Number of chunks `L = ceil(q_bits / c_w)`.
    pub(crate) const fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether no chunks are present (always false for a validated value).
    #[allow(dead_code)]
    pub(crate) const fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Bit length against which the decomposition was validated.
    #[allow(dead_code)]
    pub(crate) const fn q_bits(&self) -> usize {
        self.q_bits
    }
}

impl ModQWeightSource for ModQWeightChunks {
    fn padding_bound(&self) -> Option<&VirtualWordBound> {
        self.padding.as_ref()
    }
    fn row_count(&self) -> usize {
        self.row_count
    }

    fn chunk_width(&self) -> usize {
        self.chunk_width
    }

    fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    fn q_bits(&self) -> usize {
        self.q_bits
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn canonical_weight(&self, row: usize) -> Option<u128> {
        if row >= self.row_count {
            return None;
        }
        Some(
            self.chunks
                .iter()
                .enumerate()
                .fold(0_u128, |weight, (chunk_index, chunk)| {
                    weight | (chunk[row] << (self.chunk_width * chunk_index))
                }),
        )
    }

    fn with_chunk<R>(
        &self,
        chunk_index: usize,
        consume: impl FnOnce(&[u128]) -> R,
    ) -> Result<R, ()> {
        self.chunks
            .get(chunk_index)
            .map(|chunk| consume(chunk))
            .ok_or(())
    }
}

fn mod_q_weight_chunk_shape(
    p: &IntegerMatrixLayout,
    q_bits: usize,
) -> Result<(usize, usize, usize), ()> {
    mod_q_weight_chunk_shape_with_width(p, q_bits, p.word_bits)
}

fn mod_q_weight_chunk_shape_with_width(
    p: &IntegerMatrixLayout,
    q_bits: usize,
    value_bits: usize,
) -> Result<(usize, usize, usize), ()> {
    if value_bits == 0 || value_bits > p.word_bits {
        return Err(());
    }
    if !p.word_bits.is_power_of_two()
        || p.word_bits > u128::BITS as usize
        || !(1..=126).contains(&q_bits)
    {
        return Err(());
    }
    let row_count = u32::try_from(p.row_vars)
        .ok()
        .and_then(|t| 1usize.checked_shl(t))
        .ok_or(())?;
    let tw = p.row_vars.checked_add(value_bits).ok_or(())?;
    if tw > 126 {
        return Err(());
    }
    let chunk_width = 127usize.checked_sub(tw).ok_or(())?;
    let chunk_count = q_bits.div_ceil(chunk_width);
    if chunk_count == 0 {
        return Err(());
    }
    Ok((row_count, chunk_width, chunk_count))
}

/// Decompose each ~`q_bits`-bit row weight into base-`2^{c_w}` limbs:
/// `W_chunks[l][b] = (w_b ≫ c_w·l) & (2^{c_w}−1)`, so `w_b = Σ_l W_chunks[l][b]·2^{c_w·l}`
/// (X-note §6.1). `row_weights_q[b] = w_b ∈ [0, q)` is the canonical integer rep of
/// the field eq-weight; prover and verifier derive the same chunking from the public
/// `(c_w, L)`.
pub fn chunk_row_weights(row_weights_q: &[u128], c_w: usize, l_chunks: usize) -> Vec<Vec<u128>> {
    let limb_mask = (1u128 << c_w).wrapping_sub(1); // 2^{c_w} − 1
    let mut shift = 0usize; // c_w·l, kept < q_bits < 128 by the loop bound
    let mut out = Vec::with_capacity(l_chunks);
    for _ in 0..l_chunks {
        out.push(
            row_weights_q
                .iter()
                .map(|&w| (w >> shift) & limb_mask)
                .collect(),
        );
        shift = shift.wrapping_add(c_w);
    }
    out
}

/// The in-the-clear mod-`q` read-off (X-note §6.1 `eq:chunkreadoff` + §6.2):
/// `y = Σ_c w′_c · Σ_l 2^{c_w·l}·u_c^{(l)}` in the evaluation field `R = 𝔽_q`, reading
/// the chunk-folds of polynomial-block `base` (`= (ℓ·L)·2^s` in the batch, `0`
/// single-poly) at `v[base + l·2^s + c]`. The wide `v_c = Σ_l 2^{c_w·l}·u_c^{(l)}` is
/// reassembled **in `R`** — it never materialises as an integer (the whole point of
/// chunking) — and the column weights `w′_c ∈ R` are used at full width, unchunked.
#[allow(clippy::arithmetic_side_effects)] // caller's evaluation-ring operators
pub fn recombine_read_off<R>(
    p: &IntegerMatrixLayout,
    v: &[u128],
    base: usize,
    col_weights: &[R],
    c_w: usize,
    l_chunks: usize,
) -> R
where
    R: Copy + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let base_chunk = R::from(1u128 << c_w); // 2^{c_w} in R (c_w ≤ 126)
    let mut y = R::from(0u128);
    for c in 0..p.cols() {
        let mut v_c = R::from(0u128);
        let mut mult = R::from(1u128); // base_chunk^l
        for l in 0..l_chunks {
            let idx = base.wrapping_add(l << p.col_vars).wrapping_add(c); // base + l·2^s + c
            v_c = v_c + mult * R::from(v[idx]);
            mult = mult * base_chunk;
        }
        y = y + col_weights[c] * v_c;
    }
    y
}

/// Access to a row's place-value powers, independent of their allocation layout.
pub(crate) trait PowerTable: Sync {
    fn power(&self, row: usize, bit: usize) -> Gf;
}

impl PowerTable for [Vec<Gf>] {
    #[inline(always)]
    fn power(&self, row: usize, bit: usize) -> Gf {
        self[row][bit]
    }
}

impl PowerTable for Vec<Vec<Gf>> {
    #[inline(always)]
    fn power(&self, row: usize, bit: usize) -> Gf {
        self[row][bit]
    }
}

pub(crate) struct FlatPowers {
    pub(crate) values: Vec<Gf>,
    pub(crate) word_bits: usize,
}

impl PowerTable for FlatPowers {
    #[inline(always)]
    fn power(&self, row: usize, bit: usize) -> Gf {
        debug_assert!(bit < self.word_bits);
        self.values[row * self.word_bits + bit]
    }
}

/// One allocation for the shared powers; in particular, W=1 needs no per-row Vec.
pub(crate) fn chunk_pow2_flat(
    p: &IntegerMatrixLayout,
    w_chunk: &[u128],
    alpha: Gf,
) -> FlatPowers {
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, alpha.into(), 8);
    let mut values = vec![Gf::zero(); w_chunk.len() * p.word_bits];
    cfg_chunks_mut!(values, p.word_bits)
        .zip(cfg_iter!(w_chunk))
        .for_each(|(chain, &w)| {
            let mut cur =
                Gf::from(comb.pow_public(&field::Uint::from_words([w as u64, (w >> 64) as u64])));
            for value in chain {
                *value = cur;
                cur = cur.square();
            }
        });
    FlatPowers {
        values,
        word_bits: p.word_bits,
    }
}

/// Build the per-chunk place-value table `α^{W_chunks[l][b]·2^j}` by a per-row
/// squaring chain (`α^{w·2^j} = (α^{w·2^{j-1}})²`), one chunk's worth.
pub(crate) fn chunk_pow2_table(
    p: &IntegerMatrixLayout,
    w_chunk: &[u128],
    alpha: Gf,
) -> Vec<Vec<Gf>> {
    // Every row's base `α^{w_b}` shares the same `α`-squaring chain, so build it
    // ONCE as a fixed-base comb (≈ `win`× fewer muls than a per-row `gf_pow`),
    // then continue the per-row place-value chain `α^{w·2^j} = (·)²` for W > 1.
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, alpha.into(), 8);
    cfg_iter!(w_chunk)
        .map(|&w| {
            let mut chain = Vec::with_capacity(p.word_bits);
            let mut cur =
                Gf::from(comb.pow_public(&field::Uint::from_words([w as u64, (w >> 64) as u64])));
            for _ in 0..p.word_bits {
                chain.push(cur);
                cur = cur.square();
            }
            chain
        })
        .collect()
}

/// Per-tree packed leaf-bit halves for the bit-affine lazy forest: column
/// `c`'s `row_len` committed bits as `(bits[..row_len/2], bits[row_len/2..])`,
/// 64 per `u64`, extracted from the hint's 64-column-per-word store with
/// [`transpose64`] blocks (or a direct gather when `row_len < 128`). One
/// pass covers every column — the forest's leaf round then reads each
/// tree's ~KBs of bits instead of regenerating megabytes of `GF(2^128)`
/// leaf values.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn extract_column_bit_halves(
    packed_cols: &[Vec<u64>],
    num_cols: usize,
    row_len: usize,
) -> Vec<(Vec<u64>, Vec<u64>)> {
    let half = row_len >> 1;
    let half_words = half.div_ceil(64).max(1);
    if row_len < 128 {
        // Tiny shapes: a half doesn't fill a word — direct bit gather.
        return (0..num_cols)
            .map(|c| {
                let gather = |lo: usize, hi: usize| -> Vec<u64> {
                    let mut out = vec![0u64; half_words];
                    for (off, i) in (lo..hi).enumerate() {
                        if packed_col_bit(packed_cols, c, i) {
                            out[off >> 6] |= 1u64 << (off & 63);
                        }
                    }
                    out
                };
                (gather(0, half), gather(half, row_len))
            })
            .collect();
    }
    debug_assert_eq!(half % 64, 0, "row_len >= 128 keeps the halves word-aligned");
    let mut out: Vec<(Vec<u64>, Vec<u64>)> = (0..num_cols)
        .map(|_| (vec![0u64; half_words], vec![0u64; half_words]))
        .collect();
    // transpose64 of 64 consecutive position-words yields, per lane j, the
    // position-packed word of column 64g+j — the exact target layout.
    // Column groups write disjoint 64-column output chunks, so the groups
    // parallelize cleanly (measured 33 → ~4 ms at n = 28: the serial scan's
    // 64-line-sparse scatter per block was the forest preamble's whole
    // self-time). Pure data movement — byte-identical.
    let per_group = |(grp, cols): (&Vec<u64>, &mut [(Vec<u64>, Vec<u64>)])| {
        let mut block = [0u64; 64];
        for w in 0..row_len >> 6 {
            block.copy_from_slice(&grp[w << 6..(w + 1) << 6]);
            transpose64(&mut block);
            for (j, word) in block.iter().enumerate().take(cols.len()) {
                let dst = &mut cols[j];
                if w < half_words {
                    dst.0[w] = *word;
                } else {
                    dst.1[w - half_words] = *word;
                }
            }
        }
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        packed_cols
            .par_iter()
            .zip(out.par_chunks_mut(64))
            .for_each(per_group);
    }
    #[cfg(not(feature = "parallel"))]
    packed_cols
        .iter()
        .zip(out.chunks_mut(64))
        .for_each(per_group);
    out
}

/// The bit-affine leaf coefficients of one `pow2` table: leaf `i` equals
/// `1 + bit·τ[i]` with `τ[i] = tbl[i≫log_w][i mod W] + 1` (char 2:
/// `bit = 1 ⇒ 1 + (v+1) = v`, `bit = 0 ⇒ 1`), split into the L/R halves the
/// forest's leaf round consumes. Padded rows have `v = α⁰ = 1 ⇒ τ = 0`.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn leaf_tau_halves(
    p: &IntegerMatrixLayout,
    tbl: &(impl PowerTable + ?Sized),
    one: Gf,
    log_w: usize,
    row_len: usize,
) -> (Vec<Gf>, Vec<Gf>) {
    let mask_w = p.word_bits.wrapping_sub(1);
    let tau = |i: usize| -> Gf { tbl.power(i >> log_w, i & mask_w) + one };
    let h = row_len >> 1;
    ((0..h).map(tau).collect(), (h..row_len).map(tau).collect())
}

/// The shared both-bits-set leaf-pair products `pair_tbl[i] =
/// τ_i · τ_{i+row_len/2}` (leaf VALUES, not affine coefficients), built
/// ONCE per weight set and reused by every column's layer-1 build — the
/// `(1,1)` case was the only per-column `GF(2^128)` multiply left in the
/// bit-affine build (≈¼ of the pairs × 2^s columns of redundant recompute
/// at random density).
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn layer1_pair_table(
    p: &IntegerMatrixLayout,
    tbl: &(impl PowerTable + ?Sized),
    log_w: usize,
    row_len: usize,
) -> Vec<Gf> {
    let mask_w = p.word_bits.wrapping_sub(1);
    let half1 = row_len >> 1;
    cfg_into_iter!(0..half1)
        .map(|i| {
            let ih = i + half1;
            tbl.power(i >> log_w, i & mask_w) * tbl.power(ih >> log_w, ih & mask_w)
        })
        .collect()
}

/// One column-tree's **layer-1 halves**, fused from the bits: `layer1[i] =
/// leaf(i) · leaf(i + row_len/2)` with the leaves' `{1, α^{w·2^j}}` structure
/// exploited — every case is a table copy: the singles from `tbl`, the
/// both-set case from the shared [`layer1_pair_table`]. The `2^d`-leaf
/// layer is never materialised at build, and the build does ZERO
/// per-column multiplies.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_column_layer1_halves(
    p: &IntegerMatrixLayout,
    packed_cols: &[Vec<u64>],
    col: usize,
    tbl: &(impl PowerTable + ?Sized),
    pair_tbl: &[Gf],
    one: Gf,
    log_w: usize,
    row_len: usize,
) -> (Vec<Gf>, Vec<Gf>) {
    let mask_w = p.word_bits.wrapping_sub(1);
    let half1 = row_len >> 1;
    let pair = |i: usize| -> Gf {
        let lo = packed_col_bit(packed_cols, col, i);
        let hi = packed_col_bit(packed_cols, col, i.wrapping_add(half1));
        match (lo, hi) {
            (false, false) => one,
            (true, false) => tbl.power(i >> log_w, i & mask_w),
            (false, true) => {
                let ih = i.wrapping_add(half1);
                tbl.power(ih >> log_w, ih & mask_w)
            }
            (true, true) => pair_tbl[i],
        }
    };
    let hh = half1 >> 1;
    ((0..hh).map(pair).collect(), (hh..half1).map(pair).collect())
}

// ---------------------------------------------------------------------
// Batched mod-`q`: commit `L_polys` polynomials together, open each at its OWN
// 𝔽_q point. The forest spans `L_polys·L·2^s` trees (poly ℓ, chunk l, column c);
// the opening sends `L_polys·L` short messages and shares the `num_openings`
// columns (each carrying `L_polys·2^s` committed bits). Tree / message / read-off
// index `k = (ℓ·L + l)·2^s + c`.

// =====================================================================
// Proof-size accounting (transmitted cryptographic payload, in bytes).
//
// `GF(2^128)` and the integer fold values serialise to 16 bytes; a Merkle node
// is 32 bytes; a packed codeword cell is a `u64` (8 bytes); the opened-column
// index is 8 bytes. (Bookkeeping the verifier re-derives — leaf_index/count — is
// not counted.) `size_bytes` walks an actual proof; `predicted_size_bytes`
// computes the same total from the shape alone (the proof size is fully
// shape-determined), so a sweep over `t` needs no prover run. A test pins
// `size_bytes == predicted_size_bytes`.

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod weight_chunk_tests {
    use super::*;

    #[test]
    fn padded_weights_require_structural_support_and_bind_exact_map() {
        use circuit::linear_map::{CscMatrix, binary::PreparedVirtualMap};
        let p = IntegerMatrixLayout {
            row_vars: 2,
            col_vars: 0,
            word_bits: 128,
        };
        let map = |row| {
            PreparedVirtualMap::new(
                CscMatrix::try_from_columns(512, vec![vec![(row, true)]]).unwrap(),
            )
            .unwrap()
        };
        let good = map(64);
        let bad = map(65);
        let weights = vec![(1_u128 << 99) + 7; p.rows()];
        let chunks = ModQWeightChunks::from_dense_padded(&p, &weights, 100, 65, &good).unwrap();
        assert_eq!(chunks.chunk_width(), 60);
        let bound = chunks.padding_bound().unwrap();
        assert!(bound.matches_map(&p, &good));
        assert!(!bound.matches_map(&p, &map(63)));
        assert!(ModQWeightChunks::from_dense_padded(&p, &weights, 100, 65, &bad).is_err());
        assert!(VirtualWordBound::new(p, 0, &good).is_err());
        assert!(VirtualWordBound::new(p, 129, &good).is_err());
        let wrong_shape = IntegerMatrixLayout { row_vars: 3, ..p };
        assert!(VirtualWordBound::new(wrong_shape, 65, &good).is_err());
        assert!(!bound.matches_layout(&wrong_shape));
    }

    #[test]
    fn validated_mod_q_weight_chunks_round_trip_dense_decomposition() {
        let p = IntegerMatrixLayout {
            row_vars: 3,
            col_vars: 2,
            word_bits: 1,
        };
        let q_bits = 126;
        let dense = (0..p.rows())
            .map(|row| (1u128 << 125) | ((row as u128 + 1) << 61) | row as u128)
            .collect::<Vec<_>>();

        let prepared = ModQWeightChunks::from_dense(&p, &dense, q_bits).unwrap();
        assert_eq!(prepared.row_count(), p.rows());
        assert_eq!(prepared.chunk_width(), 123);
        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared.q_bits(), q_bits);
        assert_eq!(prepared.canonical_weights().collect::<Vec<_>>(), dense);

        let mut generated = ModQWeightChunks::zeroed(&p, q_bits).unwrap();
        generated.set_weight_range(0, &dense).unwrap();
        assert_eq!(generated, prepared);

        let reconstructed =
            ModQWeightChunks::from_chunks(&p, q_bits, prepared.chunks().to_vec()).unwrap();
        assert_eq!(reconstructed, prepared);

        let single_q_bits = 100;
        let single_dense = (0..p.rows())
            .map(|row| (row as u128 + 1) << 61)
            .collect::<Vec<_>>();
        let adopted =
            ModQWeightChunks::from_single_chunk(&p, single_q_bits, single_dense.clone()).unwrap();
        assert_eq!(adopted.chunks()[0], single_dense);
    }

    #[test]
    fn generated_mod_q_weight_source_matches_dense_chunks_without_dense_eq_storage() {
        let p = IntegerMatrixLayout {
            row_vars: 4,
            col_vars: 3,
            word_bits: 1,
        };
        let q_bits = 126;
        let canonical =
            |row: usize| Some((1_u128 << 125) | ((row as u128 + 9) << 65) | row as u128);
        let dense = (0..p.rows())
            .map(|row| canonical(row).unwrap())
            .collect::<Vec<_>>();
        let chunks = ModQWeightChunks::from_dense(&p, &dense, q_bits).unwrap();
        let generated = GeneratedModQWeightSource::new(&p, q_bits, canonical).unwrap();

        assert_eq!(generated.row_count(), chunks.row_count());
        assert_eq!(generated.chunk_width(), chunks.chunk_width());
        assert_eq!(generated.chunk_count(), chunks.len());
        for row in 0..p.rows() {
            assert_eq!(generated.canonical_weight(row), Some(dense[row]));
        }
        for chunk_index in 0..chunks.len() {
            generated
                .with_chunk(chunk_index, |limb| {
                    assert_eq!(limb, &chunks.chunks()[chunk_index]);
                })
                .unwrap();
        }
    }

    #[test]
    fn validated_mod_q_weight_chunks_reject_malformed_shapes_and_limbs() {
        let p = IntegerMatrixLayout {
            row_vars: 3,
            col_vars: 0,
            word_bits: 1,
        };
        let q_bits = 126;
        let dense = vec![7u128; p.rows()];
        let prepared = ModQWeightChunks::from_dense(&p, &dense, q_bits).unwrap();

        assert!(ModQWeightChunks::from_dense(&p, &dense[..dense.len() - 1], q_bits).is_err());
        let mut out_of_range_dense = dense.clone();
        out_of_range_dense[0] = 1u128 << q_bits;
        assert!(ModQWeightChunks::from_dense(&p, &out_of_range_dense, q_bits).is_err());

        let mut wrong_count = prepared.chunks().to_vec();
        wrong_count.pop();
        assert!(ModQWeightChunks::from_chunks(&p, q_bits, wrong_count).is_err());

        let mut wrong_rows = prepared.chunks().to_vec();
        wrong_rows[0].pop();
        assert!(ModQWeightChunks::from_chunks(&p, q_bits, wrong_rows).is_err());

        let mut wide_low_limb = prepared.chunks().to_vec();
        wide_low_limb[0][0] = 1u128 << prepared.chunk_width();
        assert!(ModQWeightChunks::from_chunks(&p, q_bits, wide_low_limb).is_err());

        let mut wide_final_limb = prepared.chunks().to_vec();
        wide_final_limb[1][0] = 1u128 << (q_bits - prepared.chunk_width());
        assert!(ModQWeightChunks::from_chunks(&p, q_bits, wide_final_limb).is_err());
    }


}

/// Sample one codeword-column index from the transcript (power-of-two
/// codeword length). Vendored from the zinc-plus F2 prover.
pub fn sample_column_idx(transcript: &mut impl Transcript, codeword_len: usize) -> usize {
    assert!(
        codeword_len.is_power_of_two(),
        "sample_column_idx requires power-of-two codeword length; got {codeword_len}",
    );
    let raw: u64 = transcript.get_challenge();
    #[allow(clippy::arithmetic_side_effects)]
    let idx = (raw as usize) & (codeword_len - 1);
    idx
}

/// Absorb a slice of `GF128Poly<D>` coefficients as little-endian words.
/// Vendored from the zinc-plus F2 prover.
pub fn absorb_gf128_poly_slice<'a, const D: usize, I>(transcript: &mut impl Transcript, iter: I)
where
    I: IntoIterator<Item = &'a crate::poly::univariate::binary_gf128::GF128Poly<D>>,
{
    let polys: Vec<_> = iter.into_iter().collect();
    if polys.is_empty() {
        return;
    }
    let mut buf = Vec::with_capacity(polys.len() * D * 16);
    for p in &polys {
        for c in p.coeffs.iter() {
            for w in c.as_words() {
                buf.extend_from_slice(&w.to_le_bytes());
            }
        }
    }
    transcript.absorb_slice(&buf);
}
