//! One-round, characteristic-2 Brakedown instantiation of the integer-MLE
//! evaluation protocol over an `F_2` commitment.
//!
//! This realises §9 ("A one-round, characteristic-2 Brakedown instantiation")
//! of the char2-fieldswitch note: prove an *integer* multilinear-extension
//! evaluation of data committed over a characteristic-2 (`F_2`) Brakedown
//! commitment, by folding the `t` "row" variables in the exponent of a binary
//! field and reading the remaining `s` "column" variables off in the clear.
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
//! each factor affine in one committed bit. A GF(2^128) GKR grand product
//! (milestone M1) binds `α^{v_c}` to the committed bits, and a generalized
//! `F_2`-Brakedown opening (M2) discharges the resulting leaf claims.
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
//!
//! # Milestones
//!
//! * **M0** (this revision): the reference fold arithmetic and completeness
//!   relations below, with tests. No cryptographic argument yet — this pins the
//!   protocol math the later milestones must reproduce.
//! * **M1**: the GF(2^128) GKR grand product binding `α^{v_c}`.
//! * **M2**: the generalized `F_2`-Brakedown opening discharging the leaf claims.
//! * **M3**: the in-the-clear final read-off and end-to-end prove/verify.

use crate::piop::lookup::gkr_product::{
    ForestLayerProof, ForestLeafBits, ProductForestProof, prove_product_forest,
    prove_product_forest_lazy,
    verify_product_forest,
};
use crate::piop::sumcheck::SumcheckProof;
use crate::poly::mle::DenseMultilinearExtension;
use crate::poly::univariate::F2PackU64;
use crate::poly::univariate::binary::BinaryPoly;
use crate::poly::univariate::binary_gf128::{BinaryFieldGF128 as Gf, GF128Poly};
use crate::poly::utils::build_eq_x_r_vec;
use crate::transcript::traits::Transcript;
use crate::code::F2LinearOpener;
use crate::merkle::{MerkleProof, MerkleTree, MtHash};

use crate::utils::{cfg_into_iter, cfg_iter};

use crypto_primitives::crypto_bigint_monty::MontyField;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Shape of one integer-MLE-evaluation instance.
///
/// The `n = t + s` boolean variables are split into `t` *folded* (row)
/// variables — combined in one round, with integer weights — and `s` *final*
/// (column) variables, read off in the clear with arbitrary (field) weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntEvalParams {
    /// Number of folded (row) variables; there are `2^t` row branches.
    pub t: usize,
    /// Number of final (column) variables; there are `2^s` columns.
    pub s: usize,
    /// Word width `W`: every data cell is an integer in `[0, 2^W)`.
    pub word_bits: usize,
}

impl IntEvalParams {
    /// Number of row branches `2^t`.
    pub fn rows(&self) -> usize {
        1usize << self.t
    }

    /// Number of columns `2^s`.
    pub fn cols(&self) -> usize {
        1usize << self.s
    }

    /// Number of data cells `2^{t+s}`.
    pub fn cells(&self) -> usize {
        1usize << self.t << self.s
    }

    /// Flat index of cell `(b, c)` in row-major (`b` rows, `c` columns) order.
    pub fn cell_index(&self, b: usize, c: usize) -> usize {
        (b << self.s) | c
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

/// Fixed-base comb table for raising one **fixed** base `α` to many different
/// exponents (the `α^{w_b}` over all `2^t` rows of a chunk).
///
/// [`gf_pow`]`(α, e)` re-runs the whole `α, α², α⁴, …` squaring chain on every
/// call, but for a fixed base that chain is identical across calls. This
/// precomputes, per `win`-bit window `i`, the table `T[i][d] = α^{d·2^{win·i}}`
/// (the `α`-squaring chain, shared once); then `α^e = ∏_i T[i][digit_i(e)]` is
/// `⌈bits/win⌉` muls with **no per-exponent squarings** — ~`win`× fewer field
/// muls than `gf_pow` on a dense ~`c_w`-bit exponent.
pub struct FixedBasePow {
    /// `table[i][d] = α^{d · 2^{win·i}}`, `d ∈ [0, 2^win)`.
    table: Vec<Vec<Gf>>,
    win: usize,
}

impl FixedBasePow {
    /// Build the comb for base `alpha` covering exponents up to `max_bits` bits,
    /// with `win`-bit windows (`win ≤ 16`).
    #[allow(clippy::arithmetic_side_effects)] // window/digit counters, bounded by max_bits/win
    pub fn new(alpha: Gf, max_bits: usize, win: usize) -> Self {
        debug_assert!((1..=16).contains(&win));
        let num_windows = max_bits.div_ceil(win);
        let radix = 1usize << win;
        let mut table = Vec::with_capacity(num_windows);
        let mut base_i = alpha; // α^{2^{win·0}} = α
        for _ in 0..num_windows {
            let mut row = Vec::with_capacity(radix);
            let mut cur = Gf::one();
            for _ in 0..radix {
                row.push(cur);
                cur = cur * base_i;
            }
            table.push(row);
            for _ in 0..win {
                base_i = base_i.square(); // advance to α^{2^{win·(i+1)}}
            }
        }
        Self { table, win }
    }

    /// `α^exp` — one table lookup per non-zero `win`-bit window, multiplied.
    #[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
    pub fn pow(&self, mut exp: u128) -> Gf {
        let mask = (1u128 << self.win).wrapping_sub(1);
        let mut acc = Gf::one();
        let mut i = 0usize;
        while exp != 0 {
            let d = (exp & mask) as usize; // < 2^win ≤ 2^16
            if d != 0 {
                acc = acc * self.table[i][d];
            }
            exp >>= self.win;
            i = i.wrapping_add(1);
        }
        acc
    }
}

/// The integer row-fold `v_c = Σ_{b∈{0,1}^t} w_b · D[(b,c)]`, one value per column.
///
/// `data[cell_index(b, c)]` is the word at row `b`, column `c` (in `[0, 2^W)`);
/// `row_weights[b] = w_b` are nonnegative integer weights (length `2^t`).
///
/// `data` is row-major (`cell_index(b,c) = b·2^s + c`), so `data.chunks(cols)`
/// yields the rows contiguously. We sweep **row-major** — each row read
/// sequentially, accumulating `w_b · row_b` into a `cols`-sized vector that stays
/// hot in cache — instead of the old column-major `data[(b<<s)|c]` gather, which
/// strides by `2^s` (a cache miss per cell, the dominant `modq:vfold` cost). The
/// accumulation is split over **row-blocks in parallel** (per-block partial sums,
/// then merged); the `wrapping_*` ops are exact because the instance is bounded so
/// every accumulator stays below `2^128` (cf. [`max_fold_magnitude`]).
#[allow(clippy::arithmetic_side_effects)]
pub fn fold_values(p: &IntEvalParams, data: &[u128], row_weights: &[u128]) -> Vec<u128> {
    let cols = p.cols();
    let fold_row = |mut acc: Vec<u128>, (row, &w): (&[u128], &u128)| -> Vec<u128> {
        for (a, &d) in acc.iter_mut().zip(row) {
            *a = a.wrapping_add(w.wrapping_mul(d));
        }
        acc
    };
    #[cfg(feature = "parallel")]
    {
        data.par_chunks(cols)
            .zip(row_weights.par_iter())
            .fold(|| vec![0u128; cols], fold_row)
            .reduce(
                || vec![0u128; cols],
                |mut a, b| {
                    for (x, &y) in a.iter_mut().zip(b.iter()) {
                        *x = x.wrapping_add(y);
                    }
                    a
                },
            )
    }
    #[cfg(not(feature = "parallel"))]
    {
        data.chunks(cols).zip(row_weights.iter()).fold(vec![0u128; cols], fold_row)
    }
}

/// [`fold_values`] reading the cell bits from the commit hint's **bit-packed**
/// store (`packed_cols`) instead of a separate `data` tensor — `v_c = Σ_b
/// w_b·INT(D(b,c))`, `D(b,c) = Σ_j 2^j·bit`. Lets the open compute the chunk-folds
/// WITHOUT rebuilding/holding the `2^n`-cell `data` (16 bytes per W=1 bit, the
/// same bits already committed in `packed_cols`) — it sits alongside the forest's
/// `2N` trees, so dropping it cuts the open's peak memory by `2^n·16` (≈ a third
/// at large `n`, the difference between fitting RAM and swapping).
#[allow(clippy::arithmetic_side_effects)]
fn fold_values_packed(
    p: &IntEvalParams,
    packed_cols: &[Vec<u64>],
    col0: usize,
    row_weights: &[u128],
) -> Vec<u128> {
    // Fold this poly's `2^s` columns, which start at global committed column `col0`
    // (`0` single-poly, `ℓ·2^s` for batch poly ℓ).
    let cols = p.cols();
    if p.word_bits == 1 {
        let mut v = vec![0u128; cols];
        // W=1: column (col0+lc)'s bit at row b is lane (col0+lc)%64 of the packed
        // store. When `col0` and `2^s` are 64-aligned (s ≥ 6, or single-poly), this
        // poly's columns span whole groups `[col0/64, …)` — sweep each group's rows
        // and scan set lanes, adding `w_b` (sequential packed read, hot ≤64-wide
        // accumulator; parallel over groups, each a disjoint slice of `v`).
        if col0 & 63 == 0 && cols & 63 == 0 {
            let g0 = col0 >> 6;
            let fold_group = |gi: usize, vchunk: &mut [u128]| {
                let prow = &packed_cols[g0 + gi];
                for (b, &w) in row_weights.iter().enumerate() {
                    let mut word = prow[b];
                    while word != 0 {
                        let k = word.trailing_zeros() as usize;
                        vchunk[k] = vchunk[k].wrapping_add(w);
                        word &= word.wrapping_sub(1);
                    }
                }
            };
            #[cfg(feature = "parallel")]
            v.par_chunks_mut(64).enumerate().for_each(|(gi, vc)| fold_group(gi, vc));
            #[cfg(not(feature = "parallel"))]
            v.chunks_mut(64).enumerate().for_each(|(gi, vc)| fold_group(gi, vc));
            return v;
        }
        // Unaligned (small s): per-column bit reads.
        for (lc, vc) in v.iter_mut().enumerate() {
            let c = col0.wrapping_add(lc);
            let mut acc = 0u128;
            for (b, &w) in row_weights.iter().enumerate() {
                if packed_col_bit(packed_cols, c, b) {
                    acc = acc.wrapping_add(w);
                }
            }
            *vc = acc;
        }
        return v;
    }
    // W>1: reconstruct each W-bit word from its committed bits.
    let log_w = p.word_bits.trailing_zeros() as usize;
    let mut v = vec![0u128; cols];
    for (lc, vc) in v.iter_mut().enumerate() {
        let c = col0.wrapping_add(lc);
        let mut acc = 0u128;
        for (b, &w) in row_weights.iter().enumerate() {
            let mut word = 0u128;
            for j in 0..p.word_bits {
                if packed_col_bit(packed_cols, c, (b << log_w) | j) {
                    word |= 1u128 << j;
                }
            }
            acc = acc.wrapping_add(w.wrapping_mul(word));
        }
        *vc = acc;
    }
    v
}

/// `α^{v_c}` for every column, computed the *protocol* way: as the product over
/// the `2^t` row branches `∏_b α^{w_b·D[(b,c)]} = ∏_b (α^{w_b})^{D[(b,c)]}`.
///
/// This is the object the GKR grand product (M1) will bind to the committed
/// bits; here we compute it in the clear to check it equals `α^{v_c}`.
pub fn exponent_fold(p: &IntEvalParams, data: &[u128], row_weights: &[u128], alpha: Gf) -> Vec<Gf> {
    // Per-branch bases β_b = α^{w_b}; then α^{w_b·D} = β_b^{D}.
    let bases: Vec<Gf> = row_weights.iter().map(|&w| gf_pow(alpha, w)).collect();
    let mut out = vec![Gf::one(); p.cols()];
    for c in 0..p.cols() {
        let mut acc = Gf::one();
        for b in 0..p.rows() {
            acc = acc * gf_pow(bases[b], data[p.cell_index(b, c)]);
        }
        out[c] = acc;
    }
    out
}

/// The bit-level grand-product leaves for column `c`: indexed by
/// `(b ∈ {0,1}^t, j ∈ {0,1}^{log₂ W})`, the leaf is `bit_{(b,c),j} ? α^{w_b·2^j} : 1`.
/// Their product over all `(b,j)` is `α^{v_c}` (the [`exponent_fold`] value), and
/// each leaf is affine in one committed bit — the shape the generalized Brakedown
/// opening (M2) discharges. The GKR grand product binds this product to the
/// committed bits and reduces to an evaluation of the leaf MLE at a random point.
/// Requires `word_bits` to be a power of two.
pub fn column_leaves(
    p: &IntEvalParams,
    data: &[u128],
    row_weights: &[u128],
    alpha: Gf,
    c: usize,
) -> Vec<Gf> {
    assert!(p.word_bits.is_power_of_two(), "word_bits must be a power of two");
    let log_w = p.word_bits.trailing_zeros() as usize;
    // β_b = α^{w_b}; the set-bit factor at position j is β_b^{2^j}.
    let bases: Vec<Gf> = row_weights.iter().map(|&w| gf_pow(alpha, w)).collect();
    let mut leaves = vec![Gf::one(); p.rows() << log_w];
    for b in 0..p.rows() {
        let cell = data[p.cell_index(b, c)];
        for j in 0..p.word_bits {
            if (cell >> j) & 1 == 1 {
                leaves[(b << log_w) | j] = gf_pow(bases[b], 1u128 << j);
            }
        }
    }
    leaves
}

/// Prove the exponent fold of *all* `2^s` columns at once via the forest GKR.
///
/// Returns the forest proof, the per-column roots `α^{v_c}` (each binds the sent
/// integer `v_c` via `α^{v_c} = root_c`), the **shared** reduction point `ρ`, and
/// the per-column leaf-MLE claims `ℓ_c = ṽ_c(ρ)`. Because every column tree has the
/// same depth `t + log₂W`, the forest reduces them all to one point `ρ` — the
/// property that makes the batched leaf functional tensor-structured (see
/// [`row_bit_weights`]).
pub fn prove_fold_forest(
    transcript: &mut impl Transcript,
    p: &IntEvalParams,
    data: &[u128],
    row_weights: &[u128],
    alpha: Gf,
) -> (ProductForestProof<Gf>, Vec<Gf>, Vec<Gf>, Vec<Gf>) {
    // Precompute the column-independent table `pow2[b][j] = α^{w_b·2^j}` ONCE
    // via a per-row squaring chain (`α^{w_b·2^j} = (α^{w_b·2^{j-1}})²`): O(2^t·W)
    // total, replacing the per-leaf `gf_pow(bases[b], 2^j)` (O(W²)/cell) that was
    // also recomputed for every one of the `2^s` columns. The per-column leaf
    // build is then pure table lookups, parallel across the independent columns.
    let log_w = p.word_bits.trailing_zeros() as usize;
    let pow2: Vec<Vec<Gf>> = row_weights
        .iter()
        .map(|&w| {
            let mut chain = Vec::with_capacity(p.word_bits);
            let mut cur = gf_pow(alpha, w);
            for _ in 0..p.word_bits {
                chain.push(cur);
                cur = cur.square();
            }
            chain
        })
        .collect();
    let one = Gf::one();
    let leaves: Vec<Vec<Gf>> = {
        let _g = crate::utils::prof::scope("prove:leaves");
        cfg_into_iter!(0..p.cols())
            .map(|c| {
                let mut leaves = vec![one; p.rows() << log_w];
                for b in 0..p.rows() {
                    let cell = data[p.cell_index(b, c)];
                    for j in 0..p.word_bits {
                        if (cell >> j) & 1 == 1 {
                            leaves[(b << log_w) | j] = pow2[b][j];
                        }
                    }
                }
                leaves
            })
            .collect()
    };
    let (proof, claims) = prove_product_forest(transcript, leaves, &());
    let roots = proof.roots.clone();
    // All trees share the reduction point; take it from the first tree's claim.
    let rho = claims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
    let leaf_evals: Vec<Gf> = claims.iter().map(|(_, ev)| *ev).collect();
    (proof, roots, rho, leaf_evals)
}

/// The shared row-bit weight vector `q_rowbit` over the `(b, j)` leaf index, at the
/// forest's shared reduction point `ρ`:
/// `q_rowbit[(b<<log₂W)|j] = eq((b,j),ρ) · (α^{w_b·2^j} − 1)`.
///
/// It is the same for every column, so the per-column leaf claims
/// `ℓ_c − 1 = ⟨q_rowbit, bits_c⟩` batch (under `γ_c`) into the single tensor
/// functional `⟨γ ⊗ q_rowbit, bits⟩` that the Brakedown opening (M2b) discharges.
/// (`ℓ_c − 1` because `Σ_{(b,j)} eq((b,j),ρ) = 1` and each leaf is
/// `1 + bit·(α^{w_b·2^j} − 1)`.)
pub fn row_bit_weights(p: &IntEvalParams, row_weights: &[u128], alpha: Gf, rho: &[Gf]) -> Vec<Gf> {
    let log_w = p.word_bits.trailing_zeros() as usize;
    let mask = p.word_bits.wrapping_sub(1);
    let eq = build_eq_x_r_vec(rho, &()).expect("shared reduction point is non-empty");
    // The `2^t` ~q_bits-wide exponentiations dominate this table (it is the
    // verifier's O(2^t) step and was 18% of the mod-q PROVE at nv=16):
    // fixed-base comb (≈8× fewer mults than square-and-multiply) + parallel.
    let comb = FixedBasePow::new(alpha, 128, 8);
    let bases: Vec<Gf> = cfg_iter!(row_weights).map(|&w| comb.pow(w)).collect();
    let one = Gf::one();
    cfg_into_iter!(0..eq.len())
        .map(|i| {
            let b = i >> log_w;
            let j = i & mask;
            eq[i] * (gf_pow(bases[b], 1u128 << j) - one)
        })
        .collect()
}

/// `max_c v_c` — the magnitude that must stay below `ord(α)` (≈ `2^128`) for the
/// exponent binding `α^{v_c}` to be injective on the values that occur.
pub fn max_fold_magnitude(v: &[u128]) -> u128 {
    v.iter().copied().max().unwrap_or(0)
}

/// The in-the-clear final read-off over the column variables in the evaluation
/// field, here taken to be the integers: `Σ_c e_c · v_c`. The global scalar
/// `G_r = ∏_i (1 - r'_i)` is applied by the caller.
#[allow(clippy::arithmetic_side_effects)] // Reference arithmetic; bounded instance.
pub fn final_eval_int(v: &[u128], col_weights: &[u128]) -> u128 {
    let mut acc = 0u128;
    for (vc, ec) in v.iter().zip(col_weights.iter()) {
        acc += vc * ec;
    }
    acc
}

/// The general §9 read-off `P(r) = G_r · Σ_c e_c · v_c` over an arbitrary
/// **evaluation ring** `R`. The bound integers `v_c` are lifted by the canonical
/// map `R::from`; the column weights `e_c` and the global scalar
/// `G_r = ∏_i(1 − r'_i)` are caller-supplied `R`-elements. `R` must **not** be of
/// characteristic 2 — lifting an integer `v_c` into `GF(2^128)` would collapse to
/// its parity (the very obstruction the exponent fold avoids), so the evaluation
/// field is a *separate*, char-≠2 field (e.g. a Mersenne prime field), distinct
/// from the binding field `K`. [`final_eval_int`] is the `R = ℤ` (`u128`),
/// `G_r = 1` specialization.
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

/// Draw one Fiat–Shamir challenge `γ` and return its powers `[1, γ, …, γ^{n−1}]`,
/// used as the per-tree/per-column batching weights `γ_c = γ^c`. Geometric powers
/// of a single challenge (one transcript squeeze + `n` field muls) replace `n`
/// independent squeezes — sound by Schwartz–Zippel (a nonzero degree-`n`
/// combination in `γ` vanishes with probability `≤ n/|K| ≈ 2^{-112}`), and mirrors
/// the forest GKR's own `ρ^t` batching. (At `n = 2^s` this removes the `2^s`
/// sequential transcript squeezes that the cost profile flagged as ~17% of prove.)
fn challenge_powers(transcript: &mut impl Transcript, n: usize) -> Vec<Gf> {
    let gamma: Gf = transcript.get_field_challenge(&());
    let mut acc = Gf::one();
    (0..n)
        .map(|_| {
            let cur = acc;
            acc = acc * gamma;
            cur
        })
        .collect()
}

/// The direct integer evaluation `Σ_{b,c} w_b · e_c · D[(b,c)]`, computed without
/// folding, for the completeness cross-check against `final_eval_int ∘ fold_values`.
#[allow(clippy::arithmetic_side_effects)] // Reference arithmetic; bounded instance.
pub fn direct_eval_int(
    p: &IntEvalParams,
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
// M2b — the committed characteristic-2 Brakedown opening.
//
// M2a validated the reduction *in the clear*: the forest GKR collapses all
// `2^s` columns to a shared point `ρ`, binds each `α^{v_c}`, and the batched
// leaf claim is the tensor functional `Σ_c γ_c(ℓ_c − 1) = ⟨γ ⊗ q_rowbit, bits⟩`
// (see `forest_reduces_to_tensor_functional`). M2b replaces the in-the-clear
// right-hand side `⟨γ ⊗ q_rowbit, bits⟩` with a genuine commitment opening: the
// bits are committed once over the `F_2`-RAA code (before any challenge), and
// the verifier reads the linear functional `⟨m, q_rowbit⟩` off the γ-combined
// message `m = Σ_c γ_c · M[c]`, bound to the committed columns by a standard
// Brakedown proximity spot-check. No integer combination ever passes through
// the code — only the `F_2`/`K`-linear queries (§9 "Commitment").
//
// **Layout (locked).** The bits are laid out *flat* as a `2^s × row_len`
// single-bit matrix `M`, with `row_len = 2^t · W` and
// `M[c][(b ≪ log₂W) | j] = bit_{(b,c),j}`. Rows index the data columns `c`
// (the `γ`-fold direction); columns index the `(b, j)` leaf coordinates (the
// `q_rowbit` functional direction). One message-row per data column makes the
// batched leaf functional tensor-structured, exactly as M2a established.

/// Error returned by [`verify_open`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntEvalOpenError {
    /// `⟨m, q_rowbit⟩` did not equal the claimed `Σ_c γ_c(ℓ_c − 1)`.
    ClaimMismatch,
    /// A sent column index disagreed with the transcript-derived index.
    ColumnIndexMismatch { sent: usize, expected: usize },
    /// A Merkle path failed to verify against the committed root.
    Merkle { idx: usize },
    /// `Σ_c γ_c · cw_c[j'] ≠ enc_K(m)[j']` at an opened codeword column.
    EncodingMismatch { idx: usize },
    /// Proof / instance shape inconsistency.
    Shape,
}

/// Commitment to the message matrix `M` (output of [`commit_bits`]). The root
/// stands in for the `ZipPlusCommitment` of the SHA path; `cw_columns` and the
/// bit-packed `packed_cols` are the prover's opening hint.
#[derive(Clone, Debug)]
pub struct CommitHint {
    /// The committed message matrix `M`, stored **bit-packed 64 columns per word**:
    /// `packed_cols[g][i]` is a `u64` whose lane `k` holds `M[64·g + k][i]` (the
    /// committed bit of column `64·g + k` at coordinate `i = (b≪log₂W)|j`).
    /// `⌈num_cols/64⌉` groups; lanes `≥ num_cols` in the last group are 0. This is
    /// the representation the `F_2`-RAA encoder consumes directly (64 lanes encode
    /// in parallel via `u64` ops — vs one `BinaryF2Poly<1>` per bit, the prior 64×
    /// memory blow-up). Read individual bits with [`packed_col_bit`].
    packed_cols: Vec<Vec<u64>>,
    /// Number of real committed columns (`2^s` single-poly, `L·2^s` batched).
    num_cols: usize,
    /// `cw_columns[j]` = codeword position `j`'s `num_cols` bits **packed**
    /// 64-per-cell into `⌈num_cols/64⌉` `BinaryPoly<64>` words — the Merkle leaf
    /// pre-image for column `j` (bit `c` lives in word `c/64`, position `c%64`).
    cw_columns: Vec<Vec<BinaryPoly<64>>>,
    /// Merkle tree over the `codeword_len` codeword columns.
    tree: MerkleTree,
    /// Commitment root.
    pub root: MtHash,
    /// `codeword_len = row_len · REP`.
    pub codeword_len: usize,
}

/// The committed bit `M[c][i]` from the bit-packed message store: column `c`
/// lives in word `c/64`, lane `c%64`.
#[inline]
pub(crate) fn packed_col_bit(packed_cols: &[Vec<u64>], c: usize, i: usize) -> bool {
    (packed_cols[c >> 6][i] >> (c & 63)) & 1 == 1
}

/// One opened codeword column: its index, the `2^s` committed bits at that
/// codeword position, and the Merkle path proving them against the root.
pub struct OpenedColumn {
    pub idx: usize,
    /// The opened codeword column's `2^s` bits, packed 64-per-`BinaryPoly<64>`
    /// cell (`⌈2^s/64⌉` cells); bit `c` is word `c/64`, position `c%64`.
    pub col_packed: Vec<BinaryPoly<64>>,
    pub merkle_proof: MerkleProof,
}

/// The committed-opening proof: the γ-combined `K`-message `m` and the opened
/// codeword columns.
pub struct OpenProof {
    /// `m[i] = Σ_c γ_c · M[c][i] ∈ K`, length `row_len`.
    pub m: Vec<Gf>,
    pub opened: Vec<OpenedColumn>,
}

/// Encode the bit-packed message groups (`packed_cols[g]` = 64 message-columns
/// per word) and Merkle-commit the codeword columns. Shared by [`commit_bits`]
/// (`num_cols = 2^s`) and [`commit_batch`] (`num_cols = L·2^s`). Each group's
/// `row_len` words encode all 64 of its columns in parallel via the `F_2`-RAA
/// packed-`u64` path; `cw_columns[j]` is then a **direct gather** of the groups'
/// position-`j` words — already in the Merkle-leaf packing, so there is no
/// separate transpose/pack pass.
fn commit_message_rows<Code: F2LinearOpener + Sync>(
    code: &Code,
    packed_cols: Vec<Vec<u64>>,
    num_cols: usize,
) -> CommitHint {
    // Encode each group (64 columns/word) — the commit hot spot, parallel over groups.
    let packed_cw: Vec<Vec<u64>> = {
        let _g = crate::utils::prof::scope("commit:encode");
        cfg_iter!(packed_cols).map(|row| code.encode_f2_packed_open(row)).collect()
    };
    let codeword_len = packed_cw.first().map_or(0, Vec::len);
    // cw_columns[j][g] = packed_cw[g][j]: position `j`'s committed bits, already
    // packed 64-per-word (word `g` = columns 64g..64g+63) — just gather over groups.
    let cw_columns: Vec<Vec<BinaryPoly<64>>> = {
        let _g = crate::utils::prof::scope("commit:pack");
        cfg_into_iter!(0..codeword_len)
            .map(|j| packed_cw.iter().map(|cw| BinaryPoly::<64>::unpack_u64(cw[j])).collect())
            .collect()
    };
    let (tree, root) = {
        let _g = crate::utils::prof::scope("commit:merkle");
        let col_refs: Vec<&[BinaryPoly<64>]> = cw_columns.iter().map(Vec::as_slice).collect();
        let tree = MerkleTree::new_from_columns(&col_refs);
        let root = tree.root();
        (tree, root)
    };
    CommitHint { packed_cols, num_cols, cw_columns, tree, root, codeword_len }
}

/// Commit the integer-MLE instance's bit matrix, **bit-packed 64 columns per
/// word**: `packed_cols[g][i]` (lane `k` = `bit_{(64g+k, ·), j}` at `i =
/// (b≪log₂W)|j`). `F_2`-RAA encodes each group (64 columns in parallel) and
/// Merkle-commits the codeword columns. The same packed encoder is the `F_2`
/// restriction of the `K`-encode applied to the combined message in
/// [`verify_open`]; because the code is `F_2`-linear, encoding the 64 lanes
/// together is bit-identical to encoding each column alone, so the root and
/// `cw_columns` match the per-column commitment exactly.
#[allow(clippy::arithmetic_side_effects)] // bounded index math over the data tensor
pub fn commit_bits<Code: F2LinearOpener + Sync>(
    code: &Code,
    p: &IntEvalParams,
    data: &[u128],
) -> CommitHint {
    assert!(p.word_bits.is_power_of_two(), "word_bits must be a power of two");
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w; // 2^t · W
    let cols = p.cols(); // 2^s columns
    let bit_mask = p.word_bits.wrapping_sub(1);
    let num_groups = cols.div_ceil(64);

    // packed_cols[g][i] = u64 with lane k = bit of column `64g+k` at `i`. For each
    // group + coordinate `i=(b,j)`, read the 64 (≤) contiguous data cells of this
    // group's columns (`data[b·cols + 64g .. +lanes]`) and pack their bit `j` —
    // a contiguous sub-row read + one `u64` write per (group, i), 64× less write
    // traffic than the prior one-`BinaryF2Poly<1>`-per-bit transpose.
    let mut packed_cols: Vec<Vec<u64>> = (0..num_groups).map(|_| vec![0u64; row_len]).collect();
    {
        let _g = crate::utils::prof::scope("commit:rows");
        const CHUNK: usize = 512;
        for (g, prow) in packed_cols.iter_mut().enumerate() {
            let c0 = g << 6;
            let lanes = 64.min(cols.wrapping_sub(c0));
            let fill = |i0: usize, ch: &mut [u64]| {
                for (off, w) in ch.iter_mut().enumerate() {
                    let i = i0.wrapping_add(off);
                    let base = (i >> log_w).wrapping_mul(cols).wrapping_add(c0); // cell_index(b, c0)
                    let j = i & bit_mask;
                    let mut word = 0u64;
                    for k in 0..lanes {
                        let set = (data[base.wrapping_add(k)] >> j) & 1 == 1;
                        word |= u64::from(set) << k;
                    }
                    *w = word;
                }
            };
            #[cfg(feature = "parallel")]
            prow.par_chunks_mut(CHUNK).enumerate().for_each(|(ci, ch)| fill(ci.wrapping_mul(CHUNK), ch));
            #[cfg(not(feature = "parallel"))]
            prow.chunks_mut(CHUNK).enumerate().for_each(|(ci, ch)| fill(ci.wrapping_mul(CHUNK), ch));
        }
    }

    commit_message_rows(code, packed_cols, cols)
}

/// Prove the committed opening: send the `γ`-combined `K`-message
/// `m = Σ_c γ_c · M[c]` and open `num_openings` transcript-sampled codeword
/// columns with their Merkle paths.
pub fn prove_open(
    transcript: &mut impl Transcript,
    hint: &CommitHint,
    gammas: &[Gf],
    num_openings: usize,
) -> OpenProof {
    let cols = hint.num_cols;
    assert_eq!(gammas.len(), cols, "one gamma per data column");
    let row_len = hint.packed_cols.first().map_or(0, Vec::len);

    // m[i] = Σ_c γ_c · M[c][i] — lift each committed bit into K (0/1) and fold
    // the columns with the random batch weights. Parallel over the row
    // coordinate `i`: each m[i] is an independent reduction across columns,
    // reading the bits from the bit-packed store.
    let packed = &hint.packed_cols;
    let m: Vec<Gf> = cfg_into_iter!(0..row_len)
        .map(|i| {
            let mut acc = Gf::zero();
            for c in 0..cols {
                if packed_col_bit(packed, c, i) {
                    acc = acc + gammas[c];
                }
            }
            acc
        })
        .collect();

    // Absorb m, then sample the opened columns. Sampling is transcript-driven
    // and stays sequential so the verifier's mirror matches; once the indices
    // are pinned the per-opening Merkle proofs are independent → parallel.
    let m_poly: Vec<GF128Poly<1>> = m.iter().map(|&x| GF128Poly::<1>::new([x])).collect();
    absorb_gf128_poly_slice::<1, _>(transcript, m_poly.iter());

    let indices: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, hint.codeword_len)).collect();
    let opened: Vec<OpenedColumn> = cfg_iter!(indices)
        .map(|&idx| {
            let merkle_proof = hint.tree.prove(idx).expect("in-range codeword column");
            OpenedColumn { idx, col_packed: hint.cw_columns[idx].clone(), merkle_proof }
        })
        .collect();

    OpenProof { m, opened }
}

/// Verify the committed opening against `root` and the claimed value
/// `claimed = Σ_c γ_c(ℓ_c − 1)`.
///
/// Three checks: (i) the eval read-off `⟨m, q_rowbit⟩ = claimed`; (ii) every
/// opened column's Merkle path; (iii) proximity consistency
/// `Σ_c γ_c · cw_c[j'] = enc_K(m)[j']`, which binds `m` to the committed bits.
/// The encoder is reconstructed from the public code passed by the caller.
pub fn verify_open<Code: F2LinearOpener>(
    transcript: &mut impl Transcript,
    code: &Code,
    root: &MtHash,
    proof: &OpenProof,
    claimed: Gf,
    gammas: &[Gf],
    q_rowbit: &[Gf],
    num_openings: usize,
) -> Result<(), IntEvalOpenError> {
    let row_len = proof.m.len();
    if q_rowbit.len() != row_len || proof.opened.len() != num_openings {
        return Err(IntEvalOpenError::Shape);
    }

    // (i) eval read-off: ⟨m, q_rowbit⟩ must equal the claim.
    let eval: Gf = cfg_iter!(proof.m).zip(cfg_iter!(q_rowbit)).map(|(mi, qi)| *mi * *qi).sum();
    if eval != claimed {
        return Err(IntEvalOpenError::ClaimMismatch);
    }

    // Re-encode the K-message: for an honest prover enc_K(m)[j] = Σ_c γ_c·cw_c[j]
    // because the F_2-RAA code is F_2-linear and the γ-combination commutes
    // through it.
    let m_poly: Vec<GF128Poly<1>> = proof.m.iter().map(|&x| GF128Poly::<1>::new([x])).collect();
    let enc = code.encode_gf128_lin_open::<1>(&m_poly);
    let codeword_len = enc.len();

    // Re-derive the opened columns from the transcript (mirror `prove_open`).
    absorb_gf128_poly_slice::<1, _>(transcript, m_poly.iter());
    let expected_indices: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, codeword_len)).collect();

    // Per-opening checks are mutually independent → parallel. The transcript
    // draws above stay sequential; here we only read the pinned indices.
    cfg_iter!(proof.opened)
        .zip(cfg_iter!(expected_indices))
        .try_for_each(|(opened, expected_idx)| {
            if opened.idx != *expected_idx {
                return Err(IntEvalOpenError::ColumnIndexMismatch {
                    sent: opened.idx,
                    expected: *expected_idx,
                });
            }
            if opened.col_packed.len() != gammas.len().div_ceil(64) {
                return Err(IntEvalOpenError::Shape);
            }
            // (ii) Merkle path: recompute the leaf hash from the sent packed bits.
            opened
                .merkle_proof
                .verify(root, &opened.col_packed, opened.idx)
                .map_err(|_| IntEvalOpenError::Merkle { idx: opened.idx })?;
            // (iii) proximity consistency: Σ_c γ_c·cw_c[j'] vs enc_K(m)[j'] — unpack
            // bit `c` from word `c/64`, position `c%64`.
            let mut acc = Gf::zero();
            for (c, g) in gammas.iter().enumerate() {
                if (opened.col_packed[c >> 6].pack_u64() >> (c & 63)) & 1 == 1 {
                    acc = acc + *g;
                }
            }
            if acc != enc[opened.idx].coeffs[0] {
                return Err(IntEvalOpenError::EncodingMismatch { idx: opened.idx });
            }
            Ok(())
        })?;
    Ok(())
}

// =====================================================================
// M3 — the end-to-end integer-MLE-evaluation argument.
//
// Wraps M2a (the forest GKR binding `α^{v_c}` and reducing to the shared point
// `ρ` + leaf claims `ℓ_c`) and M2b (the committed opening discharging the leaf
// claims against the `F_2` commitment) into one `prove`/`verify`, and adds the
// two pieces that complete §9: the prover *sends* the short integer vector `v`
// (the `2^s` folded values, the `√N`-sized message), and the verifier (i) binds
// each `v_c` by `α^{v_c} = root_c` — faithful because `α^{(·)}` is injective on
// `[0, ord α)` when `2^k > V` (no wraparound) — and (ii) reads the evaluation
// off `v` in the clear: `P(r) = G_r · Σ_c e_c · v_c`. The integer weights live
// only in the exponent fold (M2a); the column read-off takes the sent integers
// `v` directly, so the parity obstruction of an `F_2` commitment never arises.

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
        .map(Gf::from)
        .find(|&a| is_generator(a))
        .expect("GF(2^128)^× is cyclic and has generators")
}

/// End-to-end integer-MLE-evaluation proof: the M2a forest GKR, the sent folded
/// integers `v` (bound by `α^{v_c} = root_c`, the roots living in `forest`), and
/// the M2b committed opening.
pub struct IntEvalProof {
    pub forest: ProductForestProof<Gf>,
    /// The folded integers `v_c = Σ_b w_b · D[(b,c)]` (the `√N`-sized message).
    pub v: Vec<u128>,
    pub open: OpenProof,
}

/// Error returned by [`verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntEvalError {
    /// The forest grand-product verifier rejected, or `v`'s length was wrong.
    Forest,
    /// `α` does not generate `K^×`, so `α^{(·)}` is not injective on the bounded
    /// fold values — the integer binding would be unsound. Resample `α`.
    ChallengeNotGenerator,
    /// `α^{v_c} ≠ root_c`: a sent integer is not the one bound by the forest.
    RootBinding { c: usize },
    /// `max_c v_c` reached the group order [`GF128_MULT_ORDER`] (wraparound risk).
    Magnitude { max: u128 },
    /// A sent chunk-fold `u_c^{(l)}` (tree index `k`) failed the free range check
    /// `0 ≤ u_c^{(l)} < 2^{c_w+t+W}` of the mod-`q` construction — the cleartext
    /// magnitude check that, with the generator binding, pins it (X-note §6.1).
    ChunkRange { k: usize },
    /// The committed opening (M2b) failed.
    Open(IntEvalOpenError),
    /// The in-the-clear read-off `G_r·Σ_c e_c·v_c` disagreed with the claim.
    ReadOff,
}

/// All `2^s` column trees share the depth `t + log₂W`; recover it from the
/// per-tree leaf count `2^t · W` without an integer add (clippy-friendly).
fn forest_tree_depth(p: &IntEvalParams) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    (p.rows() << log_w).trailing_zeros() as usize
}

/// Prove an integer-MLE evaluation of data committed by [`commit_bits`].
///
/// `hint` is the commitment produced *before* any challenge; its root is
/// published and absorbed by the caller ahead of this call. Runs the M2a forest
/// GKR, records the folded integers `v`, draws the per-column batch weights `γ`,
/// and produces the M2b opening.
pub fn prove(
    transcript: &mut impl Transcript,
    hint: &CommitHint,
    p: &IntEvalParams,
    data: &[u128],
    row_weights: &[u128],
    alpha: Gf,
    num_openings: usize,
) -> IntEvalProof {
    let (forest, _roots, _rho, _leaf_evals) =
        prove_fold_forest(transcript, p, data, row_weights, alpha);
    let v = {
        let _g = crate::utils::prof::scope("prove:vfold");
        fold_values(p, data, row_weights)
    };
    let gammas: Vec<Gf> = challenge_powers(transcript, p.cols());
    let open = {
        let _g = crate::utils::prof::scope("prove:open");
        prove_open(transcript, hint, &gammas, num_openings)
    };
    IntEvalProof { forest, v, open }
}

/// Verify an integer-MLE evaluation against the published commitment `root` and
/// the claimed value `claimed_eval = G_r · Σ_c e_c · v_c`, with the column
/// weights `col_weights = e_c` and global scalar `g_r = G_r = ∏_i(1 − r'_i)` in
/// an arbitrary (char-≠2) **evaluation ring** `R` — see [`final_eval_ring`].
/// (`R = u128`, `g_r = 1` recovers the plain integer evaluation.)
///
/// Four stages: (1) the forest GKR (→ shared `ρ`, leaf claims `ℓ_c`); (2) require
/// `α` to generate `K^×` and bind each sent `v_c` by `α^{v_c} = root_c`;
/// (3) discharge the leaf claims through the M2b committed opening; (4) read off
/// `G_r · Σ_c e_c · v_c` in the evaluation ring and match the claim.
pub fn verify<Code, R>(
    transcript: &mut impl Transcript,
    code: &Code,
    root: &MtHash,
    proof: &IntEvalProof,
    p: &IntEvalParams,
    row_weights: &[u128],
    col_weights: &[R],
    g_r: R,
    alpha: Gf,
    claimed_eval: R,
    num_openings: usize,
) -> Result<(), IntEvalError>
where
    Code: F2LinearOpener,
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    // (1) Forest grand product → shared point ρ + per-column leaf claims ℓ_c.
    let depths = vec![forest_tree_depth(p); p.cols()];
    let vclaims =
        verify_product_forest(transcript, &proof.forest, &depths, &()).map_err(|_| IntEvalError::Forest)?;
    if proof.v.len() != p.cols() || proof.forest.roots.len() != p.cols() {
        return Err(IntEvalError::Forest);
    }
    let rho: Vec<Gf> = vclaims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
    let leaf_evals: Vec<Gf> = vclaims.iter().map(|(_, e)| *e).collect();

    // (2) Bind the sent integers: α^{v_c} = root_c. Faithful iff α generates K^×
    // (so n ↦ α^n is injective on [0, 2^128−1)) and no v_c reaches the order.
    if !is_generator(alpha) {
        return Err(IntEvalError::ChallengeNotGenerator);
    }
    let max = max_fold_magnitude(&proof.v);
    if max >= GF128_MULT_ORDER {
        return Err(IntEvalError::Magnitude { max });
    }
    // Per-column `α^{v_c} = root_c` checks are independent → parallel.
    cfg_into_iter!(0..proof.v.len()).try_for_each(|c| {
        if gf_pow(alpha, proof.v[c]) != proof.forest.roots[c] {
            Err(IntEvalError::RootBinding { c })
        } else {
            Ok(())
        }
    })?;

    // (3) Re-derive γ, q_rowbit, and the batched leaf claim, then discharge the
    // committed opening (binds the committed bits to ℓ_c, hence to the roots).
    let gammas: Vec<Gf> = challenge_powers(transcript, p.cols());
    let q = row_bit_weights(p, row_weights, alpha, &rho);
    let one = Gf::one();
    let claimed = leaf_evals
        .iter()
        .zip(&gammas)
        .fold(Gf::zero(), |acc, (l, g)| acc + *g * (*l - one));
    verify_open(transcript, code, root, &proof.open, claimed, &gammas, &q, num_openings)
        .map_err(IntEvalError::Open)?;

    // (4) In-the-clear read-off on the bound integers, in the evaluation ring.
    let computed = final_eval_ring(&proof.v, col_weights, g_r);
    if computed != claimed_eval {
        return Err(IntEvalError::ReadOff);
    }
    Ok(())
}

// =====================================================================
// Batched PCS — commit `L` polynomials together, open each at its OWN point.
//
// "Separate randomness, commit together": the `L` polynomials `D_ℓ` (uniform
// shape `t, s, W`) are committed as one interleaved `F_2`-RAA Merkle tree, and
// each is opened at its own evaluation point `r_ℓ` (its own integer row-weights
// `w_b^{(ℓ)}` and field column-weights `e_c^{(ℓ)}`), under a single shared `α`.
// The forest of `L·2^s` exponent-trees still reduces to ONE shared point `ρ`, so
// one merged GKR + one set of `t` column openings (shared Merkle paths) serve
// the whole batch; only the `L` short combined messages `m_ℓ`, the per-poly
// proximity slices, and the `L` read-offs are batch-indexed (see the design
// note in the ledger). Column `k = ℓ·2^s + c` is poly `ℓ`'s data column `c`.

/// Batched integer-MLE-evaluation proof over a shared commitment.
pub struct BatchProof {
    /// One forest GKR over all `L·2^s` exponent-trees.
    pub forest: ProductForestProof<Gf>,
    /// Folded integers `v[ℓ·2^s + c] = v_{ℓ,c}`, poly-major; bound by `α^{v}=root`.
    pub v: Vec<u128>,
    /// The `L` γ-combined messages `m_ℓ = Σ_c γ_{ℓ,c}·M_ℓ[c]` (each length `row_len`).
    pub messages: Vec<Vec<Gf>>,
    /// The shared opened codeword columns (each carries all `L·2^s` bits).
    pub opened: Vec<OpenedColumn>,
}

/// Commit `L` polynomials together (one interleaved Merkle tree). `data[ℓ]` is
/// poly `ℓ`'s `2^t × 2^s` matrix (W-bit cells in `cell_index(b,c)` layout).
/// The `L·2^s` message-rows (`L` need **not** be a power of two — only the
/// per-poly `2^s` is) are committed as one tree, row `k = ℓ·2^s + c` being
/// `M_ℓ[c]`. The returned [`CommitHint`] is shared by `prove_batch`/`verify_batch`.
#[allow(clippy::arithmetic_side_effects)] // bounded index math over the data tensors
pub fn commit_batch<Code: F2LinearOpener + Sync>(
    code: &Code,
    p: &IntEvalParams,
    data: &[Vec<u128>],
) -> CommitHint {
    assert!(p.word_bits.is_power_of_two(), "word_bits must be a power of two");
    let l = data.len();
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let bit_mask = p.word_bits.wrapping_sub(1);
    let col_mask = p.cols().wrapping_sub(1);
    let total = l << p.s; // L·2^s columns (L arbitrary; 2^s a power of two)
    let num_groups = total.div_ceil(64);

    // packed_cols[g][i] lane m = bit of global column `k = 64g+m` at `i`, where
    // column `k = ℓ·2^s + c` holds `M_ℓ[c]` (ℓ = k≫s, c = k & col_mask). A group's
    // 64 columns stay within one poly when `2^s ≥ 64`; the per-lane ℓ/c handles
    // any `s`.
    let mut packed_cols: Vec<Vec<u64>> = (0..num_groups).map(|_| vec![0u64; row_len]).collect();
    {
        const CHUNK: usize = 512;
        for (g, prow) in packed_cols.iter_mut().enumerate() {
            let k0 = g << 6;
            let lanes = 64.min(total.wrapping_sub(k0));
            let fill = |i0: usize, ch: &mut [u64]| {
                for (off, w) in ch.iter_mut().enumerate() {
                    let i = i0.wrapping_add(off);
                    let b = i >> log_w;
                    let j = i & bit_mask;
                    let mut word = 0u64;
                    for m in 0..lanes {
                        let k = k0.wrapping_add(m);
                        let set = (data[k >> p.s][p.cell_index(b, k & col_mask)] >> j) & 1 == 1;
                        word |= u64::from(set) << m;
                    }
                    *w = word;
                }
            };
            #[cfg(feature = "parallel")]
            prow.par_chunks_mut(CHUNK).enumerate().for_each(|(ci, ch)| fill(ci.wrapping_mul(CHUNK), ch));
            #[cfg(not(feature = "parallel"))]
            prow.chunks_mut(CHUNK).enumerate().for_each(|(ci, ch)| fill(ci.wrapping_mul(CHUNK), ch));
        }
    }
    commit_message_rows(code, packed_cols, total)
}

/// Prove an integer-MLE evaluation of every committed polynomial at its own
/// point. `row_weights[ℓ]` are poly `ℓ`'s integer row-weights `w_b^{(ℓ)}`;
/// `alpha` is the shared generator.
pub fn prove_batch(
    transcript: &mut impl Transcript,
    hint: &CommitHint,
    p: &IntEvalParams,
    data: &[Vec<u128>],
    row_weights: &[Vec<u128>],
    alpha: Gf,
    num_openings: usize,
) -> BatchProof {
    let l = data.len();
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let cols = p.cols();
    let total = l << p.s; // L·2^s trees / columns
    let mask = cols.wrapping_sub(1);

    // Per-poly base table `pow2[ℓ][b][j] = α^{w_b^{(ℓ)}·2^j}` (squaring chains).
    let pow2: Vec<Vec<Vec<Gf>>> = row_weights
        .iter()
        .map(|rw| {
            rw.iter()
                .map(|&w| {
                    let mut chain = Vec::with_capacity(p.word_bits);
                    let mut cur = gf_pow(alpha, w);
                    for _ in 0..p.word_bits {
                        chain.push(cur);
                        cur = cur.square();
                    }
                    chain
                })
                .collect()
        })
        .collect();

    // Leaves for all `L·2^s` trees; tree `k = ℓ·2^s + c` uses poly `ℓ`'s table.
    let one = Gf::one();
    let leaves: Vec<Vec<Gf>> = cfg_into_iter!(0..total)
        .map(|k| {
            let ell = k >> p.s;
            let c = k & mask;
            let tbl = &pow2[ell];
            let d = &data[ell];
            let mut lv = vec![one; row_len];
            for b in 0..p.rows() {
                let cell = d[p.cell_index(b, c)];
                for j in 0..p.word_bits {
                    if (cell >> j) & 1 == 1 {
                        lv[(b << log_w) | j] = tbl[b][j];
                    }
                }
            }
            lv
        })
        .collect();
    let (forest, _claims) = prove_product_forest(transcript, leaves, &());

    // Folded integers, poly-major.
    let mut v = vec![0u128; total];
    for (ell, (d, rw)) in data.iter().zip(row_weights.iter()).enumerate() {
        let vc = fold_values(p, d, rw);
        for c in 0..cols {
            v[(ell << p.s) | c] = vc[c];
        }
    }

    // γ over all trees; then the `L` combined messages from the committed rows.
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let messages: Vec<Vec<Gf>> = cfg_into_iter!(0..l)
        .map(|ell| {
            let mut m = vec![Gf::zero(); row_len];
            for c in 0..cols {
                let k = (ell << p.s) | c;
                let g = gammas[k];
                for (i, mi) in m.iter_mut().enumerate() {
                    if packed_col_bit(&hint.packed_cols, k, i) {
                        *mi = *mi + g;
                    }
                }
            }
            m
        })
        .collect();

    // Absorb the messages (in poly order), then sample the shared columns.
    for m in &messages {
        let m_poly: Vec<GF128Poly<1>> = m.iter().map(|&x| GF128Poly::<1>::new([x])).collect();
        absorb_gf128_poly_slice::<1, _>(transcript, m_poly.iter());
    }
    let indices: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, hint.codeword_len)).collect();
    let opened: Vec<OpenedColumn> = cfg_iter!(indices)
        .map(|&idx| {
            let merkle_proof = hint.tree.prove(idx).expect("in-range codeword column");
            OpenedColumn { idx, col_packed: hint.cw_columns[idx].clone(), merkle_proof }
        })
        .collect();

    BatchProof { forest, v, messages, opened }
}

/// Verify a batched proof. `row_weights`/`col_weights`/`g_r`/`claimed` are the
/// `L` per-polynomial points and claimed evaluations `y_ℓ = G_{r_ℓ}·Σ_c e_c·v_{ℓ,c}`.
pub fn verify_batch<Code: F2LinearOpener>(
    transcript: &mut impl Transcript,
    code: &Code,
    root: &MtHash,
    proof: &BatchProof,
    p: &IntEvalParams,
    row_weights: &[Vec<u128>],
    col_weights: &[Vec<u128>],
    g_r: &[u128],
    alpha: Gf,
    claimed: &[u128],
    num_openings: usize,
) -> Result<(), IntEvalError> {
    let l = row_weights.len();
    let log_w = p.word_bits.trailing_zeros() as usize;
    let cols = p.cols();
    let total = l << p.s;

    // (1) one forest GKR → shared ρ + per-tree leaf claims.
    let depth = (p.rows() << log_w).trailing_zeros() as usize;
    let depths = vec![depth; total];
    let vclaims =
        verify_product_forest(transcript, &proof.forest, &depths, &()).map_err(|_| IntEvalError::Forest)?;
    if proof.v.len() != total
        || proof.forest.roots.len() != total
        || proof.messages.len() != l
        || col_weights.len() != l
        || g_r.len() != l
        || claimed.len() != l
    {
        return Err(IntEvalError::Forest);
    }
    let rho: Vec<Gf> = vclaims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
    let leaf_evals: Vec<Gf> = vclaims.iter().map(|(_, e)| *e).collect();

    // (2) bind every sent integer: shared α a generator, α^{v}=root, V<ord α.
    if !is_generator(alpha) {
        return Err(IntEvalError::ChallengeNotGenerator);
    }
    let max = max_fold_magnitude(&proof.v);
    if max >= GF128_MULT_ORDER {
        return Err(IntEvalError::Magnitude { max });
    }
    cfg_into_iter!(0..total).try_for_each(|k| {
        if gf_pow(alpha, proof.v[k]) != proof.forest.roots[k] {
            Err(IntEvalError::RootBinding { c: k })
        } else {
            Ok(())
        }
    })?;

    // (3) per-poly column functionals q^{(ℓ)} (shared ρ, per-poly weights), γ,
    // the batched leaf claim, and the eval Σ_ℓ⟨m_ℓ, q^{(ℓ)}⟩.
    let q: Vec<Vec<Gf>> = row_weights.iter().map(|rw| row_bit_weights(p, rw, alpha, &rho)).collect();
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let one = Gf::one();
    let claim = leaf_evals
        .iter()
        .zip(&gammas)
        .fold(Gf::zero(), |acc, (le, g)| acc + *g * (*le - one));
    let mut eval = Gf::zero();
    for ell in 0..l {
        for (mi, qi) in proof.messages[ell].iter().zip(q[ell].iter()) {
            eval = eval + *mi * *qi;
        }
    }
    if eval != claim {
        return Err(IntEvalError::Open(IntEvalOpenError::ClaimMismatch));
    }

    // Re-encode the L messages; absorb (poly order) and re-sample columns.
    let m_polys: Vec<Vec<GF128Poly<1>>> = proof
        .messages
        .iter()
        .map(|m| m.iter().map(|&x| GF128Poly::<1>::new([x])).collect())
        .collect();
    let encs: Vec<Vec<GF128Poly<1>>> =
        m_polys.iter().map(|mp| code.encode_gf128_lin_open::<1>(mp)).collect();
    let codeword_len = encs.first().map_or(0, Vec::len);
    for mp in &m_polys {
        absorb_gf128_poly_slice::<1, _>(transcript, mp.iter());
    }
    let expected: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, codeword_len)).collect();
    if proof.opened.len() != num_openings {
        return Err(IntEvalError::Open(IntEvalOpenError::Shape));
    }

    // (5) per opened column: one Merkle path + L per-poly proximity slices.
    cfg_iter!(proof.opened).zip(cfg_iter!(expected)).try_for_each(|(opened, expected_idx)| {
        if opened.idx != *expected_idx {
            return Err(IntEvalOpenError::ColumnIndexMismatch {
                sent: opened.idx,
                expected: *expected_idx,
            });
        }
        if opened.col_packed.len() != total.div_ceil(64) {
            return Err(IntEvalOpenError::Shape);
        }
        opened
            .merkle_proof
            .verify(root, &opened.col_packed, opened.idx)
            .map_err(|_| IntEvalOpenError::Merkle { idx: opened.idx })?;
        for ell in 0..l {
            // Σ_c γ_{ℓ,c}·cw_{ℓ,c}[j']: slice the column to poly ℓ's rows.
            let mut acc = Gf::zero();
            for c in 0..cols {
                let k = (ell << p.s) | c;
                if (opened.col_packed[k >> 6].pack_u64() >> (k & 63)) & 1 == 1 {
                    acc = acc + gammas[k];
                }
            }
            if acc != encs[ell][opened.idx].coeffs[0] {
                return Err(IntEvalOpenError::EncodingMismatch { idx: opened.idx });
            }
        }
        Ok(())
    }).map_err(IntEvalError::Open)?;

    // (6) per-poly in-the-clear read-off.
    for ell in 0..l {
        let vc: Vec<u128> = (0..cols).map(|c| proof.v[(ell << p.s) | c]).collect();
        if final_eval_ring(&vc, &col_weights[ell], g_r[ell]) != claimed[ell] {
            return Err(IntEvalError::ReadOff);
        }
    }
    Ok(())
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
// **Key reuse.** The `(l,c)` chunk-fold forest is structurally the batched `(ℓ,c)`
// forest with `L` "virtual columns" that all reference the SAME committed data `D`
// (only the per-chunk row-weight table differs). So the `F_2` commitment is
// **unchanged** ([`commit_bits`] / [`commit_batch`]), the forest GKR, the shared
// reduction point `ρ`, [`row_bit_weights`], and the committed Brakedown opening all
// carry over; the genuinely new code is (a) deriving + chunking the weights, (b) the
// mod-`q` recombination read-off, (c) the per-chunk cleartext range checks.
//
// **Evaluation field.** Generic over the same char-≠2 ring `R` as
// [`final_eval_ring`]; the wired realisation (test code) is a concrete 100-bit prime
// `q = 2^100 − 15`. A Mersenne `𝔽_{2^127−1}` is the drop-in alternative (weights
// ~127-bit, `L` one larger). `R` must be char ≠ 2 — lifting an integer into
// `GF(2^128)` collapses to parity, the very obstruction the exponent fold avoids.

// ─────────────────────────────────────────────────────────────────────────
// M3 bridge: integer-SHA-over-F₂ host-prover plumbing (design `f2-int-sha-*`)
// ─────────────────────────────────────────────────────────────────────────
//
// The Zinc+ integer prover (`ZincPlusPiop`) commits its binary witness over F₂
// and discharges the resulting `𝔽_q` witness-MLE claim with
// `prove_mle_eval_mod_q`. Its eval field `F` is a *config-based* `MontyField`
// over a fixed ~100-bit prime, but the mod-`q` read-off works in a *config-free*
// `R: From<u128>+Add+Mul`. Both share the SAME modulus `q = 2^100 − 15`; convert
// `F ↔ Fq` at the boundary via the canonical integer (`ProjectCanonicalU128`).

/// The fixed ~100-bit read-off prime `q = 2^100 − 15`. Must equal the host
/// prover's projection prime (`crate::fixed_prime::q100_field_cfg`).
pub const FQ_MOD: u128 = (1u128 << 100).wrapping_sub(15);
/// `⌈log₂ q⌉` — the chunk-count input for the mod-`q` opening (`L = 1` for SHA).
pub const FQ_BITS: usize = 100;

/// Column-opening count for the integer-SHA-over-F₂ base opening (the deployed
/// Brakedown soundness, matching the standalone `f2_int_eval` harnesses). Used
/// identically at commit (layout `t`), prove, and verify.
pub const F2_OPEN_NUM_COLUMNS: usize = 987;

/// Config-free element of `𝔽_q`, `q = 2^100 − 15` (invariant `0 ≤ .0 < q`). The
/// mod-`q` read-off ring `R` for the integer-SHA-over-F₂ opening; pure `u128`
/// arithmetic (no `crypto-bigint`), so it lives in the generic library where the
/// host prover's step-7 branch runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fq(pub u128);

/// `(a + b) mod q` for `a, b ∈ [0, q)`.
#[inline]
pub fn fq_add(a: u128, b: u128) -> u128 {
    let s = a.wrapping_add(b);
    if s >= FQ_MOD { s.wrapping_sub(FQ_MOD) } else { s }
}

/// `(a − b) mod q` for `a, b ∈ [0, q)`.
#[inline]
pub fn fq_sub(a: u128, b: u128) -> u128 {
    if a >= b { a.wrapping_sub(b) } else { a.wrapping_add(FQ_MOD).wrapping_sub(b) }
}

/// `(a · b) mod q` by Russian-peasant doubling (no 256-bit dep; `q < 2^100`, so
/// every `fq_add` stays below `2^101`).
#[inline]
pub fn fq_mul(a: u128, b: u128) -> u128 {
    let mut a = a % FQ_MOD;
    let mut b = b % FQ_MOD;
    let mut r = 0u128;
    while b != 0 {
        if b & 1 == 1 {
            r = fq_add(r, a);
        }
        a = fq_add(a, a);
        b >>= 1;
    }
    r
}

impl From<u128> for Fq {
    #[inline]
    fn from(x: u128) -> Self {
        Fq(x % FQ_MOD)
    }
}
impl core::ops::Add for Fq {
    type Output = Fq;
    #[inline]
    fn add(self, o: Fq) -> Fq {
        Fq(fq_add(self.0, o.0))
    }
}
impl core::ops::Mul for Fq {
    type Output = Fq;
    #[inline]
    fn mul(self, o: Fq) -> Fq {
        Fq(fq_mul(self.0, o.0))
    }
}

/// Little-endian `eq` table over `𝔽_q`: `out[idx] = ∏_l (bit_l(idx) ? point[l] :
/// 1 − point[l])` with `point[0] ↔ bit 0` (LSB) — matching
/// `crate::poly::utils::build_eq_x_r_vec`, the convention `compute_lifted_evals`
/// uses for the lifted `bar_u`. This makes the F₂ read-off weights agree with the
/// host PIOP's α-combined witness-MLE claim. (Contrast the standalone tests'
/// `eq_table_fq`, which is big-endian.)
pub fn eq_le_table_fq(point: &[Fq]) -> Vec<Fq> {
    let one = Fq(1u128 % FQ_MOD);
    let mut acc = vec![one];
    for r in point {
        let one_minus_r = Fq(fq_sub(one.0, r.0));
        let mut next = Vec::with_capacity(acc.len().wrapping_mul(2));
        for &e in &acc {
            next.push(e * one_minus_r);
        }
        for &e in &acc {
            next.push(e * *r);
        }
        acc = next;
    }
    acc
}

/// Extract the canonical integer (standard, *not* Montgomery, form) of a host
/// eval-field element as a `u128`. Implemented for every `MontyField<LIMBS>` via
/// `retrieve()`; only the low 128 bits are taken (the integer-SHA-over-F₂
/// read-off values are all `< q < 2^100`, so this is exact on that path).
pub trait ProjectCanonicalU128 {
    fn canonical_u128(&self) -> u128;
}

impl<const LIMBS: usize> ProjectCanonicalU128 for MontyField<LIMBS> {
    #[inline]
    fn canonical_u128(&self) -> u128 {
        let reduced = self.retrieve();
        let words = reduced.as_words();
        let lo = words.first().copied().map_or(0u128, u128::from);
        let hi = words.get(1).copied().map_or(0u128, u128::from);
        lo | (hi << 64)
    }
}

// ── SHA-witness ↔ bit-tensor layout (shared by the host prover & verifier) ──
//
// The binary witness is `num_cols` columns of `BinaryPoly<D>` cells over
// `2^num_vars` trace rows. The α-combination couples (column `i`, bit `j`) with
// per-`(i,j)` random weights, so those axes are FOLDED (carried in the exponent),
// and only the low trace bits stay CLEAR. MLE-variable budget
// `n = log₂D + ⌈log₂ num_cols⌉ + num_vars`; the folded index packs
// `b = (j ≪ (log_cols+tw)) | (i ≪ tw) | row_hi`, the clear index is `c = row_lo`,
// and `trace_row = (row_hi ≪ s) | row_lo`.

/// Geometry of the W=1 bit tensor for a SHA binary witness trace.
#[derive(Clone, Copy, Debug)]
pub struct ShaF2Layout {
    /// `{t, s, word_bits: 1}` for the fieldswitch opening.
    pub p: IntEvalParams,
    /// Actual committed columns (= `sample_alphas` batch size; padded to `2^log_cols`).
    pub num_cols: usize,
    /// Column-index fold width `⌈log₂ num_cols⌉`.
    pub log_cols: usize,
    /// Bit-position fold width `log₂ D` (5 for 32-bit words).
    pub bit_vars: usize,
    /// Trace MLE variables `log₂(#rows)`.
    pub num_vars: usize,
    /// Folded trace bits `tw = t − log₂D − log_cols = num_vars − s`.
    pub tw: usize,
    /// Extra x-tensor fold variables (the `s_x` knob): the LOW `δ` clear
    /// (column) variables of the VIRTUAL-claim tensor join its folded
    /// side, so `t' = bit_vars + tw + δ`, `s_x = s − δ` — per-claim sent
    /// folds (`us`) shrink `2^δ`× while the `O(2^{t'})` q_rowbit tables
    /// grow `2^δ`×. Because the moved variables are the LOW column bits,
    /// the flat residual-index order is unchanged and the embedding maps
    /// (`embed_xor_index`/`embed_xor_point`/`constant_residual`) are
    /// δ-independent. `0` = the natural split. Committed geometry is
    /// untouched.
    pub x_fold_extra: usize,
}

/// Derive the proof-size-optimal W=1 tensor geometry. `bit_vars = log₂D`,
/// `num_vars = log₂(#rows)`. Keeps every bit/column variable folded
/// (`t ≥ log₂D + log_cols`, so the clear width `s ≤ num_vars`) and otherwise
/// targets [`proof_size_optimal_t`]. For SHA the result has `L = 1`.
pub fn sha_f2_layout(
    num_cols: usize,
    num_vars: usize,
    bit_vars: usize,
    num_openings: usize,
) -> ShaF2Layout {
    let log_cols = num_cols.next_power_of_two().trailing_zeros() as usize;
    let n = bit_vars.wrapping_add(log_cols).wrapping_add(num_vars);
    let t_min = bit_vars.wrapping_add(log_cols);
    let t_opt = proof_size_optimal_t(n, 1, 1, num_openings);
    // Measurement knob: `F2_LAYOUT_T` overrides the split (t = log₂ rows,
    // s = n − t) so harnesses can sweep the prover-time / proof-size
    // frontier. Prover and verifier both derive the layout here, so a
    // same-process round trip stays consistent.
    let t_opt = std::env::var("F2_LAYOUT_T")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(t_opt);
    let t = t_opt.clamp(t_min, n.wrapping_sub(1));
    let s = n.wrapping_sub(t);
    let tw = num_vars.wrapping_sub(s);
    // Measurement knob: `F2_X_FOLD_EXTRA` moves δ clear variables of the
    // VIRTUAL-claim tensor to its folded side (see
    // [`ShaF2Layout::x_fold_extra`]); 0 = natural split. Same-process
    // prover/verifier round trips stay consistent (both derive it here).
    let x_fold_extra = std::env::var("F2_X_FOLD_EXTRA")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(s.wrapping_sub(1));
    ShaF2Layout {
        p: IntEvalParams { t, s, word_bits: 1 },
        num_cols,
        log_cols,
        bit_vars,
        num_vars,
        tw,
        x_fold_extra,
    }
}

/// Lay the binary witness columns into the flat W=1 single-bit tensor
/// `data[(b ≪ s) | c]` that [`commit_bits`] / [`prove_mle_eval_mod_q`] consume.
/// Padding columns (`i ≥ num_cols`) and padding rows stay 0.
pub fn sha_f2_bit_tensor<const D: usize>(
    layout: &ShaF2Layout,
    cols: &[DenseMultilinearExtension<BinaryPoly<D>>],
) -> Vec<u128> {
    let p = &layout.p;
    let s = p.s;
    let shift = layout.log_cols.wrapping_add(layout.tw);
    let row_lo_mask = (1usize << s).wrapping_sub(1);
    let mut data = vec![0u128; p.cells()];
    for (i, col) in cols.iter().enumerate() {
        for (trace_row, cell) in col.iter().enumerate() {
            let row_hi = trace_row >> s;
            let row_lo = trace_row & row_lo_mask;
            for (j, coeff) in cell.iter().enumerate() {
                if coeff.into_inner() {
                    let b = (j << shift) | (i << layout.tw) | row_hi;
                    data[(b << s) | row_lo] = 1;
                }
            }
        }
    }
    data
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

/// Build [`commit_bits`]' bit-packed message store straight from the packed
/// `BinaryPoly<D>` witness columns — no `2^n`-cell `u128` tensor in between.
///
/// Produces exactly `commit_bits(code, p, sha_f2_bit_tensor(layout, cols))`'s
/// `packed_cols`: group `g`, coordinate `b`, lane `k` holds the tensor bit
/// `data[(b ≪ s) | (64g + k)]`, which under the SHA layout is coefficient
/// `j = b ≫ (log_cols + tw)` of trace cell
/// `cols[i][(row_hi ≪ s) | (64g + k)]` with `i = (b ≫ tw) & col_mask`,
/// `row_hi = b & (2^tw − 1)`. Each 64-trace-row block of a `(column, row_hi)`
/// pair is one 64×64 bit transpose of the cells' packed words — the whole
/// build touches only the real witness bits (`num_cols · 2^nv` words), not
/// the 128×-inflated one-bit-per-`u128` tensor whose streaming dominated
/// `commit:rows`.
#[allow(clippy::arithmetic_side_effects)] // bounded index math over the layout
pub fn sha_f2_packed_cols<const D: usize>(
    layout: &ShaF2Layout,
    cols: &[DenseMultilinearExtension<BinaryPoly<D>>],
) -> Vec<Vec<u64>> {
    use crate::poly::univariate::F2PackU64;
    let p = &layout.p;
    assert_eq!(p.word_bits, 1, "sha_f2_packed_cols is the W=1 SHA layout builder");
    let s = p.s;
    let tw = layout.tw;
    let shift = layout.log_cols + tw;
    let col_mask = (1usize << layout.log_cols) - 1;
    let row_len = p.rows(); // 2^t (W = 1)
    let num_groups = p.cols().div_ceil(64);
    let bit_lanes = D.min(1usize << layout.bit_vars).min(64);
    debug_assert!(row_len >= 1usize << shift, "b must cover every (j, i, row_hi)");

    let mut packed_cols: Vec<Vec<u64>> = (0..num_groups).map(|_| vec![0u64; row_len]).collect();
    let fill_group = |g: usize, prow: &mut Vec<u64>| {
        let lanes = 64.min(p.cols() - (g << 6));
        let mut block = [0u64; 64];
        for (i, col) in cols.iter().enumerate() {
            debug_assert!(i <= col_mask, "column index exceeds the layout's log_cols");
            debug_assert_eq!(
                col.evaluations.len(),
                1usize << layout.num_vars,
                "sha_f2_packed_cols expects full-length columns"
            );
            for row_hi in 0..1usize << tw {
                let base_row = (row_hi << s) | (g << 6);
                for (k, w) in block.iter_mut().enumerate() {
                    *w = if k < lanes { col.evaluations[base_row + k].pack_u64() } else { 0 };
                }
                transpose64(&mut block);
                let b_base = (i << tw) | row_hi;
                for (j, w) in block.iter().enumerate().take(bit_lanes) {
                    prow[(j << shift) | b_base] = *w;
                }
            }
        }
    };
    #[cfg(feature = "parallel")]
    packed_cols.par_iter_mut().enumerate().for_each(|(g, prow)| fill_group(g, prow));
    #[cfg(not(feature = "parallel"))]
    packed_cols.iter_mut().enumerate().for_each(|(g, prow)| fill_group(g, prow));

    packed_cols
}

/// [`commit_bits`] variant taking the bit-packed message store directly
/// (e.g. from [`sha_f2_packed_cols`]) — identical commitment, no tensor.
pub fn commit_bits_packed<Code: F2LinearOpener + Sync>(
    code: &Code,
    p: &IntEvalParams,
    packed_cols: Vec<Vec<u64>>,
) -> CommitHint {
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    debug_assert_eq!(packed_cols.len(), p.cols().div_ceil(64));
    debug_assert!(packed_cols.iter().all(|r| r.len() == row_len));
    commit_message_rows(code, packed_cols, p.cols())
}

/// Build the mod-`q` opening weights from the host point `r₀` and the
/// α-combination, all in the config-free [`Fq`]. `r0_fq[l] = canonical(r₀[l])`
/// (little-endian, len `num_vars`); `alpha_canon[i][j] =
/// canonical(F::from_with_cfg(α_{i,j}))`. Returns
/// `(row_weights_q[b] = α_{i,j}·eq(row_hi,r₀_fold) mod q, col_weights[c] =
/// eq(c, r₀_clear))` so the read-off equals the α-combined witness-MLE
/// `Σ_{i,j} α_{i,j}·MLE[bitⱼ(colᵢ)](r₀) = eval_f`.
pub fn sha_f2_weights(
    layout: &ShaF2Layout,
    r0_fq: &[Fq],
    alpha_canon: &[Vec<u128>],
) -> (Vec<u128>, Vec<Fq>) {
    let p = &layout.p;
    let s = p.s;
    let tw = layout.tw;
    let shift = layout.log_cols.wrapping_add(tw);
    let col_mask = (1usize << layout.log_cols).wrapping_sub(1);
    let fold_mask = (1usize << tw).wrapping_sub(1);

    // Little-endian eq split: clear = low s coords of r₀, fold = high tw coords.
    let col_weights = eq_le_table_fq(&r0_fq[..s]);
    let eq_fold = eq_le_table_fq(&r0_fq[s..]);

    let mut row_weights_q = vec![0u128; p.rows()];
    for (b, w) in row_weights_q.iter_mut().enumerate() {
        let j = b >> shift;
        let i = (b >> tw) & col_mask;
        let row_hi = b & fold_mask;
        if i < layout.num_cols {
            let aij = alpha_canon.get(i).and_then(|a| a.get(j)).copied().unwrap_or(0);
            *w = fq_mul(aij, eq_fold[row_hi].0);
        }
    }
    (row_weights_q, col_weights)
}

/// Tensor geometry of a VIRTUAL per-bit XOR of UAIR columns under `layout`:
/// the XOR is indexed by `(bit j, trace row)` alone — the column-index fold
/// coordinates drop out — so the folded width shrinks to
/// `t' = bit_vars + tw = t − log_cols` while the clear split `s` (and hence
/// the trace-row alignment with the committed tensor) is unchanged.
/// `n' = t' + s = num_vars + bit_vars`, W = 1. The virtual MLE index is
/// `(b' ≪ s) | row_lo` with `b' = (j ≪ tw) | row_hi`, i.e. flat bit index
/// `(j ≪ num_vars) | trace_row` — trace-row coordinates low, bit-position
/// coordinates high.
///
/// With `layout.x_fold_extra = δ > 0` the LOW `δ` clear variables join the
/// folded side: `t' += δ`, `s_x = s − δ`, new row index
/// `b″ = (row_lo_low ≪ (bit_vars+tw)) | b'`, new clear index
/// `c″ = row_lo ≫ δ`. The flat index order is UNCHANGED, so the residual
/// point simply re-splits and the embedding maps stay δ-independent; row
/// weights then cover the coordinate list `[b' coords ++ low-δ clear
/// coords]` and column weights the remaining `s − δ`.
pub fn virtual_xor_params(layout: &ShaF2Layout) -> IntEvalParams {
    assert!(layout.x_fold_extra < layout.p.s, "x_fold_extra must leave a clear variable");
    IntEvalParams {
        t: layout.bit_vars.wrapping_add(layout.tw).wrapping_add(layout.x_fold_extra),
        s: layout.p.s.wrapping_sub(layout.x_fold_extra),
        word_bits: 1,
    }
}

/// Extract the per-column bit rows of the virtual vector
/// `x = (xor of cols) + constant-pattern + external`. `constant` is a word
/// pattern XORed into EVERY trace row (bit `j` flips bit-position `j`
/// everywhere — the all-ones pattern gives complements); `external` is an
/// optional uncommitted term given directly in the x layout (`2^s` rows of
/// `2^{t'}` bits). Base case, `x = ⊕_k cols[k]`
/// from the committed per-column rows (row `c` = `2^t` bits, 64 per `u64`).
/// Under [`sha_f2_layout`] a UAIR column `i` is a slice of each committed
/// row — `2^bit_vars` contiguous `2^tw`-bit runs at offsets
/// `(j ≪ (log_cols+tw)) | (i ≪ tw)` — so the virtual row set is a wordwise
/// XOR extraction pass (word-aligned at `tw ≥ 6`, masked sub-word runs
/// below), run `j` landing at offset `j ≪ tw` of the `2^{t'}`-bit x-row.
/// No new commitment: the committed code is `F_2`-linear in the rows.
#[allow(clippy::arithmetic_side_effects)] // bounded index math over the layout
pub fn extract_virtual_xor_rows(
    layout: &ShaF2Layout,
    rows: &[Vec<u64>],
    xor_cols: &[usize],
    constant: u128,
    external: Option<&[Vec<u64>]>,
) -> Vec<Vec<u64>> {
    let tw = layout.tw;
    let p_x = virtual_xor_params(layout);
    let delta = layout.x_fold_extra;
    let t_base = layout.bit_vars.wrapping_add(tw); // natural-split row width
    let base_words = (1usize << t_base).div_ceil(64);
    assert!(
        !xor_cols.is_empty() || constant != 0 || external.is_some(),
        "virtual XOR needs at least one term"
    );
    assert!(delta == 0 || t_base >= 6, "x_fold_extra needs word-aligned base rows (t' ≥ 6)");
    for &i in xor_cols {
        assert!(i < layout.num_cols, "XORed column {i} out of range (< {})", layout.num_cols);
    }
    if layout.bit_vars < 7 {
        assert!(
            constant < (1u128 << (1usize << layout.bit_vars)),
            "constant pattern wider than the word"
        );
    }
    if let Some(e_rows) = external {
        assert_eq!(e_rows.len(), p_x.cols(), "external rows: one per clear column");
    }
    // Base extraction at the NATURAL split (one row per committed row).
    let base: Vec<Vec<u64>> = cfg_into_iter!(0..layout.p.cols())
        .map(|c| {
            let row = &rows[c];
            let mut out = vec![0u64; base_words];
            for j in 0..(1usize << layout.bit_vars) {
                let dst = j << tw;
                for &i in xor_cols {
                    let src = (j << (layout.log_cols + tw)) | (i << tw);
                    if tw >= 6 {
                        // Word-aligned run of 2^{tw−6} words.
                        let (sw, dw) = (src >> 6, dst >> 6);
                        for w in 0..1usize << (tw - 6) {
                            out[dw + w] ^= row[sw + w];
                        }
                    } else {
                        // Sub-word run: 2^tw ≤ 32 bits, within one word on
                        // both sides (offsets are multiples of the run size).
                        let mask = (1u64 << (1usize << tw)) - 1;
                        let bits = (row[src >> 6] >> (src & 63)) & mask;
                        out[dst >> 6] ^= bits << (dst & 63);
                    }
                }
                if (constant >> j) & 1 == 1 {
                    // Flip this bit position's whole 2^tw-bit run.
                    if tw >= 6 {
                        for w in 0..1usize << (tw - 6) {
                            out[(dst >> 6) + w] ^= !0u64;
                        }
                    } else {
                        let mask = (1u64 << (1usize << tw)) - 1;
                        out[dst >> 6] ^= mask << (dst & 63);
                    }
                }
            }
            out
        })
        .collect();
    // Re-split for x_fold_extra: new row c″ = the 2^δ CONSECUTIVE base
    // rows (c″ ≪ δ)|c_lo concatenated (the moved variables are the LOW
    // clear bits, which land at the TOP of the new row index).
    let mut out: Vec<Vec<u64>> = if delta == 0 {
        base
    } else {
        let mut regrouped = Vec::with_capacity(p_x.cols());
        let mut it = base.into_iter();
        for _ in 0..p_x.cols() {
            let mut row = it.next().expect("base rows cover the regroup");
            row.reserve_exact(((1usize << delta) - 1) * base_words);
            for _ in 1..(1usize << delta) {
                row.extend_from_slice(&it.next().expect("base rows cover the regroup"));
            }
            regrouped.push(row);
        }
        regrouped
    };
    if let Some(e_rows) = external {
        for (o_row, e_row) in out.iter_mut().zip(e_rows.iter()) {
            for (o, e) in o_row.iter_mut().zip(e_row.iter()) {
                *o ^= *e;
            }
        }
    }
    out
}

/// The all-ones word pattern of the layout (`2^bit_vars` ones): XORing it
/// into a virtual claim complements every bit of the virtual vector.
pub fn xor_ones_pattern(layout: &ShaF2Layout) -> u128 {
    let w = 1usize << layout.bit_vars;
    if w >= 128 { u128::MAX } else { (1u128 << w).wrapping_sub(1) }
}

/// XOR-canonical form of a virtual claim's column list: sorted, with
/// duplicate PAIRS cancelled (a column XORed in twice contributes nothing).
pub fn xor_canonical_cols(cols: &[usize]) -> Vec<usize> {
    let mut v = cols.to_vec();
    v.sort_unstable();
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0usize;
    while i < v.len() {
        if i.wrapping_add(1) < v.len() && v[i] == v[i.wrapping_add(1)] {
            i = i.wrapping_add(2);
        } else {
            out.push(v[i]);
            i = i.wrapping_add(1);
        }
    }
    out
}

/// Complement-claim elision (statement-level): claim `j` DERIVES from an
/// earlier claim `i` when their virtual vectors satisfy `x_j = ¬x_i` —
/// same XORed columns (canonically), same row weights, no external term
/// on either side, and constants differing by the all-ones pattern —
/// because the bit-MLE of the all-ones vector is 1, so
/// `y_j + y_i = (Σ_b w_b)·(Σ_c w′_c)` in the evaluation field. A derived
/// claim needs NO forest, folds, rings or read-off: the verifier checks
/// the identity directly. Returns, per claim, `Some(first such i)` or
/// `None` (the claim runs its own machinery). Prover and verifier compute
/// this from statement data they share (the prover never sees column
/// weights — the verifier checks those separately for derived claims), so
/// the proof shape is well-defined on both sides.
///
/// Claims are given as `(cols, constant, has_external, row_weights)`.
pub fn complement_elision(
    layout: &ShaF2Layout,
    claims: &[(&[usize], u128, bool, &[u128])],
) -> Vec<Option<usize>> {
    let ones = xor_ones_pattern(layout);
    let canon: Vec<Vec<usize>> = claims.iter().map(|(c, ..)| xor_canonical_cols(c)).collect();
    let mut out: Vec<Option<usize>> = Vec::with_capacity(claims.len());
    for (j, &(_, cj, ext_j, wj)) in claims.iter().enumerate() {
        let mut found = None;
        if !ext_j {
            for (i, &(_, ci, ext_i, wi)) in claims.iter().enumerate().take(j) {
                if !ext_i && canon[i] == canon[j] && (ci ^ cj) == ones && wi == wj {
                    found = Some(i);
                    break;
                }
            }
        }
        out.push(found);
    }
    out
}

/// The mod-`q` row-weight **chunk width** `c_w = 127 − t − W` (X-note §6.1,
/// `eq:chunkbound` at `k=128`): the largest limb size for which every per-chunk fold
/// `u_c^{(l)} = Σ_b W_b^{(l)}·INT(D(b,c))` stays `< 2^{c_w+t+W} = 2^127`, so it binds
/// injectively in `K=GF(2^128)` (generator order `2^128−1 > 2^127`). Asserts
/// `t + W ≤ 126` so `c_w ∈ [1, 126]`.
pub fn mod_q_chunk_width(p: &IntEvalParams) -> usize {
    let tw = p.t.wrapping_add(p.word_bits);
    assert!(
        tw <= 126,
        "mod-q chunking needs t + W ≤ 126 (c_w = 127 − t − W ≥ 1); got t={}, W={}",
        p.t,
        p.word_bits
    );
    127usize.wrapping_sub(tw)
}

/// The number of weight chunks `L = ⌈q_bits / c_w⌉` (X-note §6.3) for an evaluation
/// modulus of bit-length `q_bits = ⌈log₂q⌉`. `L = 1` (no chunking) exactly when
/// `q_bits ≤ c_w`, i.e. `t + W ≤ 127 − q_bits` — the headroom window (`t + W < 28`
/// for a 100-bit prime).
pub fn mod_q_num_chunks(p: &IntEvalParams, q_bits: usize) -> usize {
    q_bits.div_ceil(mod_q_chunk_width(p)).max(1)
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
        out.push(row_weights_q.iter().map(|&w| (w >> shift) & limb_mask).collect());
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
    p: &IntEvalParams,
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
            let idx = base.wrapping_add(l << p.s).wrapping_add(c); // base + l·2^s + c
            v_c = v_c + mult * R::from(v[idx]);
            mult = mult * base_chunk;
        }
        y = y + col_weights[c] * v_c;
    }
    y
}

/// Proof of a field-valued (mod-`q`) MLE evaluation — same shape as [`BatchProof`],
/// but the `v` are the `L·2^s` (single) / `L_polys·L·2^s` (batch) bounded
/// **chunk-folds** `u_c^{(l)} < 2^{c_w+t+W}`, and `messages` are the per-`(poly,chunk)`
/// combined rows. One struct serves both the single-poly and batched provers (single
/// = batch with `L_polys = 1`).
pub struct MleEvalModQProof {
    /// Forest GKR over all chunk-trees (tree-major; `L·2^s` or `L_polys·L·2^s`).
    pub forest: ProductForestProof<Gf>,
    /// Chunk-folds `u_c^{(l)}`, each range-checked `< 2^{c_w+t+W}`; bound by `α^{u}=root`.
    pub v: Vec<u128>,
    /// The per-`(poly,chunk)` γ-combined messages (each length `row_len = 2^t·W`).
    pub messages: Vec<Vec<Gf>>,
    /// The shared opened codeword columns.
    pub opened: Vec<OpenedColumn>,
}

/// Build the per-chunk place-value table `α^{W_chunks[l][b]·2^j}` by a per-row
/// squaring chain (`α^{w·2^j} = (α^{w·2^{j-1}})²`), one chunk's worth.
pub(crate) fn chunk_pow2_table(p: &IntEvalParams, w_chunk: &[u128], alpha: Gf) -> Vec<Vec<Gf>> {
    // Every row's base `α^{w_b}` shares the same `α`-squaring chain, so build it
    // ONCE as a fixed-base comb (≈ `win`× fewer muls than a per-row `gf_pow`),
    // then continue the per-row place-value chain `α^{w·2^j} = (·)²` for W > 1.
    let comb = FixedBasePow::new(alpha, 128, 8);
    cfg_iter!(w_chunk)
        .map(|&w| {
            let mut chain = Vec::with_capacity(p.word_bits);
            let mut cur = comb.pow(w);
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
    let mut out: Vec<(Vec<u64>, Vec<u64>)> =
        (0..num_cols).map(|_| (vec![0u64; half_words], vec![0u64; half_words])).collect();
    // transpose64 of 64 consecutive position-words yields, per lane j, the
    // position-packed word of column 64g+j — the exact target layout.
    let mut block = [0u64; 64];
    for (g, grp) in packed_cols.iter().enumerate() {
        let lanes = 64.min(num_cols.saturating_sub(g << 6));
        if lanes == 0 {
            continue;
        }
        for w in 0..row_len >> 6 {
            block.copy_from_slice(&grp[w << 6..(w + 1) << 6]);
            transpose64(&mut block);
            for (j, word) in block.iter().enumerate().take(lanes) {
                let dst = &mut out[(g << 6) | j];
                if w < half_words {
                    dst.0[w] = *word;
                } else {
                    dst.1[w - half_words] = *word;
                }
            }
        }
    }
    out
}

/// The bit-affine leaf coefficients of one `pow2` table: leaf `i` equals
/// `1 + bit·τ[i]` with `τ[i] = tbl[i≫log_w][i mod W] + 1` (char 2:
/// `bit = 1 ⇒ 1 + (v+1) = v`, `bit = 0 ⇒ 1`), split into the L/R halves the
/// forest's leaf round consumes. Padded rows have `v = α⁰ = 1 ⇒ τ = 0`.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn leaf_tau_halves(
    p: &IntEvalParams,
    tbl: &[Vec<Gf>],
    one: Gf,
    log_w: usize,
    row_len: usize,
) -> (Vec<Gf>, Vec<Gf>) {
    let mask_w = p.word_bits.wrapping_sub(1);
    let tau = |i: usize| -> Gf { tbl[i >> log_w][i & mask_w] + one };
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
    p: &IntEvalParams,
    tbl: &[Vec<Gf>],
    log_w: usize,
    row_len: usize,
) -> Vec<Gf> {
    let mask_w = p.word_bits.wrapping_sub(1);
    let half1 = row_len >> 1;
    cfg_into_iter!(0..half1)
        .map(|i| {
            let ih = i + half1;
            tbl[i >> log_w][i & mask_w] * tbl[ih >> log_w][ih & mask_w]
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
    p: &IntEvalParams,
    packed_cols: &[Vec<u64>],
    col: usize,
    tbl: &[Vec<Gf>],
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
            (true, false) => tbl[i >> log_w][i & mask_w],
            (false, true) => {
                let ih = i.wrapping_add(half1);
                tbl[ih >> log_w][ih & mask_w]
            }
            (true, true) => pair_tbl[i],
        }
    };
    let hh = half1 >> 1;
    ((0..hh).map(pair).collect(), (hh..half1).map(pair).collect())
}

/// Prove an MLE evaluation `MLE[INT(D)](r) = y ∈ 𝔽_q` over the existing `F_2`
/// commitment (X-note §6.2). `hint` is [`commit_bits`]`(code, p, data)` — **the
/// commitment is unchanged**; the wide row eq-weights enter only through the chunked
/// exponent fold. `row_weights_q[b] = w_b ∈ [0, q)` is the canonical integer rep of
/// `eq(b, r₁) mod q` (≤ `q_bits` bits); the column weights `e_c = eq(c, r₂) ∈ 𝔽_q` are
/// applied by the verifier in the clear. Builds the `L·2^s`-tree forest binding each
/// chunk-fold `u_c^{(l)}`, sends them, and discharges one `F_2`-Brakedown opening.
/// Build the `L` γ-combined messages `m_l[i] = Σ_c γ_{l,c}·M[c][i]` over the
/// shared `2^s` committed message-rows. Parallelised over **output row-chunks**
/// (each task reads the contiguous slice `M[c][lo..hi]` for every `c` and writes
/// `m[lo..hi]` — cache-friendly, no cross-thread reduce), so it stays parallel
/// even when `L = 1` (the SHA case), where the natural per-`l` parallelism is a
/// single task and the inner `cols × row_len` loop would otherwise run serial.
fn build_combined_messages(
    hint: &CommitHint,
    gammas: &[Gf],
    l_chunks: usize,
    cols: usize,
    row_len: usize,
    s: usize,
) -> Vec<Vec<Gf>> {
    let chunk = row_len.div_ceil(64).max(1);
    (0..l_chunks)
        .map(|l| {
            let base = l << s;
            let mut m = vec![Gf::zero(); row_len];
            let build = |ci: usize, m_chunk: &mut [Gf]| {
                let lo = ci.wrapping_mul(chunk);
                for c in 0..cols {
                    let g = gammas[base.wrapping_add(c)];
                    for (off, mc) in m_chunk.iter_mut().enumerate() {
                        if packed_col_bit(&hint.packed_cols, c, lo.wrapping_add(off)) {
                            *mc = *mc + g;
                        }
                    }
                }
            };
            #[cfg(feature = "parallel")]
            m.par_chunks_mut(chunk).enumerate().for_each(|(ci, mc)| build(ci, mc));
            #[cfg(not(feature = "parallel"))]
            m.chunks_mut(chunk).enumerate().for_each(|(ci, mc)| build(ci, mc));
            m
        })
        .collect()
}

pub fn prove_mle_eval_mod_q(
    transcript: &mut impl Transcript,
    hint: &CommitHint,
    p: &IntEvalParams,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    num_openings: usize,
) -> MleEvalModQProof {
    let c_w = mod_q_chunk_width(p);
    let l_chunks = mod_q_num_chunks(p, q_bits);
    let w_chunks = chunk_row_weights(row_weights_q, c_w, l_chunks);
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let cols = p.cols();
    let total = l_chunks << p.s; // L·2^s chunk-trees
    let mask = cols.wrapping_sub(1);

    let pow2: Vec<Vec<Vec<Gf>>> = {
        let _g = crate::utils::prof::scope("modq:base");
        w_chunks.iter().map(|wc| chunk_pow2_table(p, wc, alpha)).collect()
    };

    // Generators for the L·2^s trees; tree k = l·2^s + c uses chunk l, data
    // column c (data shared across chunks — only the row-weight table differs).
    // The **lazy** forest never materialises the `2^d`-leaf layer at all:
    // `gen_layer1` fuses leaf generation into the first product layer (with the
    // `{1, α^w}` multiply shortcuts), and the final GKR round consumes the
    // committed bits directly (bit-affine leaves: shared τ tables + per-tree
    // packed bits — see `ForestLeafBits`).
    let one = Gf::one();
    let pair_tbls: Vec<Vec<Gf>> =
        pow2.iter().map(|tbl| layer1_pair_table(p, tbl, log_w, row_len)).collect();
    let gen_layer1 = |k: usize| -> (Vec<Gf>, Vec<Gf>) {
        let l = k >> p.s;
        let c = k & mask;
        build_column_layer1_halves(
            p, &hint.packed_cols, c, &pow2[l], &pair_tbls[l], one, log_w, row_len,
        )
    };
    let (tau_sets, col_bits) = {
        let _g = crate::utils::prof::scope("modq:leafbits");
        let tau_sets: Vec<(Vec<Gf>, Vec<Gf>)> =
            pow2.iter().map(|tbl| leaf_tau_halves(p, tbl, one, log_w, row_len)).collect();
        let col_bits = extract_column_bit_halves(&hint.packed_cols, cols, row_len);
        (tau_sets, col_bits)
    };
    let bits_of = |k: usize| col_bits[k & mask].clone();
    let tau_set_of = |k: usize| k >> p.s;
    let (forest, _claims) = {
        let _g = crate::utils::prof::scope("modq:forest");
        prove_product_forest_lazy(
            transcript,
            total,
            gen_layer1,
            ForestLeafBits { bits_of: &bits_of, tau_sets: &tau_sets, tau_set_of: &tau_set_of },
            &(),
        )
    };

    // Chunk-folds u_c^{(l)} = Σ_b W_chunks[l][b]·INT(D(b,c)), layout k = l·2^s + c.
    // Read the bits from the committed `packed_cols` — the open never builds/holds
    // the `2^n`-cell `data` tensor (it would sit alongside the forest's 2N trees).
    let mut v = vec![0u128; total];
    {
        let _g = crate::utils::prof::scope("modq:vfold");
        for (l, wc) in w_chunks.iter().enumerate() {
            let vl = fold_values_packed(p, &hint.packed_cols, 0, wc);
            for c in 0..cols {
                v[(l << p.s) | c] = vl[c];
            }
        }
    }

    // γ over all chunk-trees; the L combined messages reuse the SHARED 2^s rows.
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let messages: Vec<Vec<Gf>> = {
        let _g = crate::utils::prof::scope("modq:msg");
        build_combined_messages(hint, &gammas, l_chunks, cols, row_len, p.s)
    };

    // Absorb messages (chunk order), then sample the shared columns and open.
    for m in &messages {
        let m_poly: Vec<GF128Poly<1>> = m.iter().map(|&x| GF128Poly::<1>::new([x])).collect();
        absorb_gf128_poly_slice::<1, _>(transcript, m_poly.iter());
    }
    let indices: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, hint.codeword_len)).collect();
    let opened: Vec<OpenedColumn> = {
        let _g = crate::utils::prof::scope("modq:gather");
        cfg_iter!(indices)
            .map(|&idx| {
                let merkle_proof = hint.tree.prove(idx).expect("in-range codeword column");
                OpenedColumn { idx, col_packed: hint.cw_columns[idx].clone(), merkle_proof }
            })
            .collect()
    };

    MleEvalModQProof { forest, v, messages, opened }
}

/// Verify a single-polynomial mod-`q` MLE evaluation against the published `root`
/// and claimed `y ∈ R = 𝔽_q` (X-note §6.2). `row_weights_q[b] = w_b ∈ [0, q)`,
/// `col_weights[c] = e_c ∈ R`, `q_bits = ⌈log₂q⌉`. Stages: (1) forest GKR → shared
/// `ρ` + leaf claims; (2) require `α` a generator, **range-check** each chunk-fold
/// `u_c^{(l)} < 2^{c_w+t+W}`, and bind `α^{u_c^{(l)}} = root` (both, per X-note §6.1);
/// (3) the `L`-slice committed opening; (4) the mod-`q` recombination read-off
/// `y = Σ_c e_c·Σ_l 2^{c_w·l} u_c^{(l)}`.
pub fn verify_mle_eval_mod_q<Code, R>(
    transcript: &mut impl Transcript,
    code: &Code,
    root: &MtHash,
    proof: &MleEvalModQProof,
    p: &IntEvalParams,
    row_weights_q: &[u128],
    col_weights: &[R],
    q_bits: usize,
    alpha: Gf,
    claimed: R,
    num_openings: usize,
) -> Result<(), IntEvalError>
where
    Code: F2LinearOpener,
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let c_w = mod_q_chunk_width(p);
    let l_chunks = mod_q_num_chunks(p, q_bits);
    let log_w = p.word_bits.trailing_zeros() as usize;
    let cols = p.cols();
    let total = l_chunks << p.s;

    // (1) forest GKR → shared ρ + per-tree leaf claims.
    let depth = (p.rows() << log_w).trailing_zeros() as usize;
    let depths = vec![depth; total];
    let vclaims = verify_product_forest(transcript, &proof.forest, &depths, &())
        .map_err(|_| IntEvalError::Forest)?;
    if proof.v.len() != total
        || proof.forest.roots.len() != total
        || proof.messages.len() != l_chunks
        || col_weights.len() != cols
        || row_weights_q.len() != p.rows()
    {
        return Err(IntEvalError::Forest);
    }
    let rho: Vec<Gf> = vclaims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
    let leaf_evals: Vec<Gf> = vclaims.iter().map(|(_, e)| *e).collect();

    // (2) α a generator; per-chunk range check u < 2^{c_w+t+W}; α^{u}=root binding.
    if !is_generator(alpha) {
        return Err(IntEvalError::ChallengeNotGenerator);
    }
    let range_bits = c_w.wrapping_add(p.t).wrapping_add(p.word_bits); // = 127
    cfg_into_iter!(0..total).try_for_each(|k| {
        if range_bits >= 128 || (proof.v[k] >> range_bits) != 0 {
            return Err(IntEvalError::ChunkRange { k });
        }
        if gf_pow(alpha, proof.v[k]) != proof.forest.roots[k] {
            return Err(IntEvalError::RootBinding { c: k });
        }
        Ok(())
    })?;

    // (3) per-chunk q^{(l)} (shared ρ), γ, batched leaf claim, eval = Σ_l⟨m_l, q^{(l)}⟩.
    let w_chunks = chunk_row_weights(row_weights_q, c_w, l_chunks);
    let q: Vec<Vec<Gf>> = w_chunks.iter().map(|wc| row_bit_weights(p, wc, alpha, &rho)).collect();
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let one = Gf::one();
    let claim = leaf_evals
        .iter()
        .zip(&gammas)
        .fold(Gf::zero(), |acc, (le, g)| acc + *g * (*le - one));
    let mut eval = Gf::zero();
    for (l, ql) in q.iter().enumerate() {
        for (mi, qi) in proof.messages[l].iter().zip(ql.iter()) {
            eval = eval + *mi * *qi;
        }
    }
    if eval != claim {
        return Err(IntEvalError::Open(IntEvalOpenError::ClaimMismatch));
    }

    // Re-encode the L messages; absorb (chunk order) and re-sample columns.
    let m_polys: Vec<Vec<GF128Poly<1>>> = proof
        .messages
        .iter()
        .map(|m| m.iter().map(|&x| GF128Poly::<1>::new([x])).collect())
        .collect();
    let encs: Vec<Vec<GF128Poly<1>>> =
        m_polys.iter().map(|mp| code.encode_gf128_lin_open::<1>(mp)).collect();
    let codeword_len = encs.first().map_or(0, Vec::len);
    for mp in &m_polys {
        absorb_gf128_poly_slice::<1, _>(transcript, mp.iter());
    }
    let expected: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, codeword_len)).collect();
    if proof.opened.len() != num_openings {
        return Err(IntEvalError::Open(IntEvalOpenError::Shape));
    }

    // Per opened column: one Merkle path + L proximity slices over the shared 2^s bits.
    cfg_iter!(proof.opened)
        .zip(cfg_iter!(expected))
        .try_for_each(|(opened, expected_idx)| {
            if opened.idx != *expected_idx {
                return Err(IntEvalOpenError::ColumnIndexMismatch {
                    sent: opened.idx,
                    expected: *expected_idx,
                });
            }
            if opened.col_packed.len() != cols.div_ceil(64) {
                return Err(IntEvalOpenError::Shape);
            }
            opened
                .merkle_proof
                .verify(root, &opened.col_packed, opened.idx)
                .map_err(|_| IntEvalOpenError::Merkle { idx: opened.idx })?;
            for l in 0..l_chunks {
                let mut acc = Gf::zero();
                for c in 0..cols {
                    if (opened.col_packed[c >> 6].pack_u64() >> (c & 63)) & 1 == 1 {
                        acc = acc + gammas[(l << p.s) | c];
                    }
                }
                if acc != encs[l][opened.idx].coeffs[0] {
                    return Err(IntEvalOpenError::EncodingMismatch { idx: opened.idx });
                }
            }
            Ok(())
        })
        .map_err(IntEvalError::Open)?;

    // (4) mod-q recombination read-off in R = 𝔽_q.
    if recombine_read_off(p, &proof.v, 0, col_weights, c_w, l_chunks) != claimed {
        return Err(IntEvalError::ReadOff);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Batched mod-`q`: commit `L_polys` polynomials together, open each at its OWN
// 𝔽_q point. The forest spans `L_polys·L·2^s` trees (poly ℓ, chunk l, column c);
// the opening sends `L_polys·L` short messages and shares the `num_openings`
// columns (each carrying `L_polys·2^s` committed bits). Tree / message / read-off
// index `k = (ℓ·L + l)·2^s + c`.

/// Prove a batched mod-`q` MLE evaluation over a shared [`commit_batch`] commitment.
/// `data[ℓ]` is poly `ℓ`'s `2^t×2^s` matrix; `row_weights_q[ℓ][b] = w_b^{(ℓ)} ∈ [0, q)`
/// is poly `ℓ`'s point's canonical row eq-weight. One shared `α`, one merged forest.
pub fn prove_batch_mle_eval_mod_q(
    transcript: &mut impl Transcript,
    hint: &CommitHint,
    p: &IntEvalParams,
    row_weights_q: &[Vec<u128>],
    q_bits: usize,
    alpha: Gf,
    num_openings: usize,
) -> MleEvalModQProof {
    let l_polys = row_weights_q.len();
    let c_w = mod_q_chunk_width(p);
    let l_chunks = mod_q_num_chunks(p, q_bits);
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let cols = p.cols();
    let total = l_polys.wrapping_mul(l_chunks) << p.s; // L_polys·L·2^s
    let num_messages = l_polys.wrapping_mul(l_chunks);
    let mask = cols.wrapping_sub(1);

    // Per-(poly,chunk) limbs and place-value tables.
    let w_chunks: Vec<Vec<Vec<u128>>> =
        row_weights_q.iter().map(|rw| chunk_row_weights(rw, c_w, l_chunks)).collect();
    let pow2: Vec<Vec<Vec<Vec<Gf>>>> = w_chunks
        .iter()
        .map(|wcs| wcs.iter().map(|wc| chunk_pow2_table(p, wc, alpha)).collect())
        .collect();

    // Generators; tree k = (ℓ·L + l)·2^s + c uses poly ℓ's column c, chunk l.
    // Lazy forest: layer 1 fused from the bits at build; the final GKR round
    // consumes the committed bits directly (bit-affine leaves, one τ set per
    // (ℓ, l) — bigger absolute saving here, the batch has L_polys× the trees).
    let one = Gf::one();
    // Poly ℓ's column `c` lives at interleaved committed column ℓ·2^s + c.
    let decode = |k: usize| -> (usize, usize, usize) {
        let pc = k >> p.s; // ℓ·L + l
        let c = k & mask;
        let ell = pc.checked_div(l_chunks).expect("l_chunks ≥ 1");
        let l = pc.checked_rem(l_chunks).expect("l_chunks ≥ 1");
        ((ell << p.s) | c, ell, l)
    };
    let pair_tbls: Vec<Vec<Vec<Gf>>> = pow2
        .iter()
        .map(|wcs| wcs.iter().map(|tbl| layer1_pair_table(p, tbl, log_w, row_len)).collect())
        .collect();
    let gen_layer1 = |k: usize| -> (Vec<Gf>, Vec<Gf>) {
        let (col, ell, l) = decode(k);
        build_column_layer1_halves(
            p, &hint.packed_cols, col, &pow2[ell][l], &pair_tbls[ell][l], one, log_w, row_len,
        )
    };
    // τ sets in `pc = ℓ·L + l` order, matching `tau_set_of = k ≫ s`.
    let tau_sets: Vec<(Vec<Gf>, Vec<Gf>)> = pow2
        .iter()
        .flat_map(|wcs| wcs.iter().map(|tbl| leaf_tau_halves(p, tbl, one, log_w, row_len)))
        .collect();
    let col_bits = extract_column_bit_halves(&hint.packed_cols, hint.num_cols, row_len);
    let bits_of = |k: usize| {
        let (col, _, _) = decode(k);
        col_bits[col].clone()
    };
    let tau_set_of = |k: usize| k >> p.s;
    let (forest, _claims) = prove_product_forest_lazy(
        transcript,
        total,
        gen_layer1,
        ForestLeafBits { bits_of: &bits_of, tau_sets: &tau_sets, tau_set_of: &tau_set_of },
        &(),
    );

    // Chunk-folds, layout k = (ℓ·L + l)·2^s + c. Read poly ℓ's bits from the
    // committed `packed_cols` (global columns ℓ·2^s..) — no per-poly `data` tensor.
    let mut v = vec![0u128; total];
    for (ell, wcs) in w_chunks.iter().enumerate() {
        for (l, wc) in wcs.iter().enumerate() {
            let vl = fold_values_packed(p, &hint.packed_cols, ell << p.s, wc);
            let base = ell.wrapping_mul(l_chunks).wrapping_add(l) << p.s; // (ℓ·L + l)·2^s
            for c in 0..cols {
                v[base | c] = vl[c];
            }
        }
    }

    // γ over all trees; the L_polys·L messages re-combine the committed rows.
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let messages: Vec<Vec<Gf>> = cfg_into_iter!(0..num_messages)
        .map(|pl| {
            let ell = pl.checked_div(l_chunks).expect("l_chunks ≥ 1");
            let tree_base = pl << p.s; // (ℓ·L + l)·2^s
            let row_base = ell << p.s; // committed column offset ℓ·2^s
            let mut m = vec![Gf::zero(); row_len];
            for c in 0..cols {
                let g = gammas[tree_base | c];
                for (i, mi) in m.iter_mut().enumerate() {
                    if packed_col_bit(&hint.packed_cols, row_base | c, i) {
                        *mi = *mi + g;
                    }
                }
            }
            m
        })
        .collect();

    for m in &messages {
        let m_poly: Vec<GF128Poly<1>> = m.iter().map(|&x| GF128Poly::<1>::new([x])).collect();
        absorb_gf128_poly_slice::<1, _>(transcript, m_poly.iter());
    }
    let indices: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, hint.codeword_len)).collect();
    let opened: Vec<OpenedColumn> = cfg_iter!(indices)
        .map(|&idx| {
            let merkle_proof = hint.tree.prove(idx).expect("in-range codeword column");
            OpenedColumn { idx, col_packed: hint.cw_columns[idx].clone(), merkle_proof }
        })
        .collect();

    MleEvalModQProof { forest, v, messages, opened }
}

/// Verify a batched mod-`q` proof. `row_weights_q[ℓ]`/`col_weights[ℓ]`/`claimed[ℓ]`
/// are poly `ℓ`'s canonical row eq-weights, `𝔽_q` column weights, and claimed
/// evaluation `y_ℓ`. Mirrors [`verify_mle_eval_mod_q`] across the poly dimension.
pub fn verify_batch_mle_eval_mod_q<Code, R>(
    transcript: &mut impl Transcript,
    code: &Code,
    root: &MtHash,
    proof: &MleEvalModQProof,
    p: &IntEvalParams,
    row_weights_q: &[Vec<u128>],
    col_weights: &[Vec<R>],
    q_bits: usize,
    alpha: Gf,
    claimed: &[R],
    num_openings: usize,
) -> Result<(), IntEvalError>
where
    Code: F2LinearOpener,
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let l_polys = row_weights_q.len();
    let c_w = mod_q_chunk_width(p);
    let l_chunks = mod_q_num_chunks(p, q_bits);
    let log_w = p.word_bits.trailing_zeros() as usize;
    let cols = p.cols();
    let total = l_polys.wrapping_mul(l_chunks) << p.s;
    let num_messages = l_polys.wrapping_mul(l_chunks);

    // (1) one forest GKR → shared ρ + leaf claims.
    let depth = (p.rows() << log_w).trailing_zeros() as usize;
    let depths = vec![depth; total];
    let vclaims = verify_product_forest(transcript, &proof.forest, &depths, &())
        .map_err(|_| IntEvalError::Forest)?;
    if proof.v.len() != total
        || proof.forest.roots.len() != total
        || proof.messages.len() != num_messages
        || col_weights.len() != l_polys
        || claimed.len() != l_polys
        || row_weights_q.iter().any(|rw| rw.len() != p.rows())
        || col_weights.iter().any(|cw| cw.len() != cols)
    {
        return Err(IntEvalError::Forest);
    }
    let rho: Vec<Gf> = vclaims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
    let leaf_evals: Vec<Gf> = vclaims.iter().map(|(_, e)| *e).collect();

    // (2) generator; per-chunk range check; α^{u}=root binding.
    if !is_generator(alpha) {
        return Err(IntEvalError::ChallengeNotGenerator);
    }
    let range_bits = c_w.wrapping_add(p.t).wrapping_add(p.word_bits);
    cfg_into_iter!(0..total).try_for_each(|k| {
        if range_bits >= 128 || (proof.v[k] >> range_bits) != 0 {
            return Err(IntEvalError::ChunkRange { k });
        }
        if gf_pow(alpha, proof.v[k]) != proof.forest.roots[k] {
            return Err(IntEvalError::RootBinding { c: k });
        }
        Ok(())
    })?;

    // (3) per-(poly,chunk) q^{(ℓ,l)} (shared ρ, message order pl = ℓ·L + l), γ, the
    // batched leaf claim, and the eval Σ_{ℓ,l}⟨m_{ℓ,l}, q^{(ℓ,l)}⟩.
    let w_chunks: Vec<Vec<Vec<u128>>> =
        row_weights_q.iter().map(|rw| chunk_row_weights(rw, c_w, l_chunks)).collect();
    let q: Vec<Vec<Gf>> = w_chunks
        .iter()
        .flat_map(|wcs| wcs.iter().map(|wc| row_bit_weights(p, wc, alpha, &rho)))
        .collect();
    let gammas: Vec<Gf> = challenge_powers(transcript, total);
    let one = Gf::one();
    let claim = leaf_evals
        .iter()
        .zip(&gammas)
        .fold(Gf::zero(), |acc, (le, g)| acc + *g * (*le - one));
    let mut eval = Gf::zero();
    for (pl, ql) in q.iter().enumerate() {
        for (mi, qi) in proof.messages[pl].iter().zip(ql.iter()) {
            eval = eval + *mi * *qi;
        }
    }
    if eval != claim {
        return Err(IntEvalError::Open(IntEvalOpenError::ClaimMismatch));
    }

    // Re-encode the L_polys·L messages; absorb and re-sample columns.
    let m_polys: Vec<Vec<GF128Poly<1>>> = proof
        .messages
        .iter()
        .map(|m| m.iter().map(|&x| GF128Poly::<1>::new([x])).collect())
        .collect();
    let encs: Vec<Vec<GF128Poly<1>>> =
        m_polys.iter().map(|mp| code.encode_gf128_lin_open::<1>(mp)).collect();
    let codeword_len = encs.first().map_or(0, Vec::len);
    for mp in &m_polys {
        absorb_gf128_poly_slice::<1, _>(transcript, mp.iter());
    }
    let expected: Vec<usize> =
        (0..num_openings).map(|_| sample_column_idx(transcript, codeword_len)).collect();
    if proof.opened.len() != num_openings {
        return Err(IntEvalError::Open(IntEvalOpenError::Shape));
    }
    let words_per_col = (l_polys << p.s).div_ceil(64); // L_polys·2^s committed bits/col

    // Per opened column: one Merkle path + L_polys·L proximity slices. Slice (ℓ,l)
    // reads poly ℓ's 2^s committed bits {ℓ·2^s + c} with chunk-l gammas.
    cfg_iter!(proof.opened)
        .zip(cfg_iter!(expected))
        .try_for_each(|(opened, expected_idx)| {
            if opened.idx != *expected_idx {
                return Err(IntEvalOpenError::ColumnIndexMismatch {
                    sent: opened.idx,
                    expected: *expected_idx,
                });
            }
            if opened.col_packed.len() != words_per_col {
                return Err(IntEvalOpenError::Shape);
            }
            opened
                .merkle_proof
                .verify(root, &opened.col_packed, opened.idx)
                .map_err(|_| IntEvalOpenError::Merkle { idx: opened.idx })?;
            for ell in 0..l_polys {
                let row_base = ell << p.s;
                for l in 0..l_chunks {
                    let pl = ell.wrapping_mul(l_chunks).wrapping_add(l);
                    let tree_base = pl << p.s;
                    let mut acc = Gf::zero();
                    for c in 0..cols {
                        let bit = row_base | c;
                        if (opened.col_packed[bit >> 6].pack_u64() >> (bit & 63)) & 1 == 1 {
                            acc = acc + gammas[tree_base | c];
                        }
                    }
                    if acc != encs[pl][opened.idx].coeffs[0] {
                        return Err(IntEvalOpenError::EncodingMismatch { idx: opened.idx });
                    }
                }
            }
            Ok(())
        })
        .map_err(IntEvalError::Open)?;

    // (4) per-poly mod-q recombination read-off.
    for ell in 0..l_polys {
        let base = ell.wrapping_mul(l_chunks) << p.s; // (ℓ·L)·2^s
        if recombine_read_off(p, &proof.v, base, &col_weights[ell], c_w, l_chunks) != claimed[ell] {
            return Err(IntEvalError::ReadOff);
        }
    }
    Ok(())
}

/// Serialised size (bytes) of an [`MleEvalModQProof`] (single or batched) — walks
/// the actual proof; same per-term accounting as [`batch_proof_size_bytes`].
#[allow(clippy::arithmetic_side_effects)] // summation of nonneg term sizes
pub fn mle_eval_mod_q_proof_size_bytes(proof: &MleEvalModQProof) -> usize {
    let mut s = proof.forest.roots.len() * GF_BYTES;
    for layer in &proof.forest.layers {
        if let Some(sc) = &layer.sumcheck_proof {
            s += GF_BYTES; // claimed_sum
            for m in &sc.messages {
                s += m.0.tail_evaluations.len() * GF_BYTES;
            }
        }
        s += layer.evals.len() * 2 * GF_BYTES; // (l, r) per active tree
    }
    s += proof.v.len() * GF_BYTES; // sent chunk-folds
    for m in &proof.messages {
        s += m.len() * GF_BYTES; // the L_polys·L combined messages
    }
    for oc in &proof.opened {
        s += IDX_BYTES + oc.col_packed.len() * WORD_BYTES + oc.merkle_proof.siblings.len() * HASH_BYTES;
    }
    s
}

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

const GF_BYTES: usize = 16;
const HASH_BYTES: usize = 32;
const WORD_BYTES: usize = 8;
const IDX_BYTES: usize = 8;

/// Serialised size (bytes) of a [`BatchProof`] — walks the actual proof.
#[allow(clippy::arithmetic_side_effects)] // summation of nonneg term sizes
pub fn batch_proof_size_bytes(proof: &BatchProof) -> usize {
    let mut s = proof.forest.roots.len() * GF_BYTES;
    for layer in &proof.forest.layers {
        if let Some(sc) = &layer.sumcheck_proof {
            s += GF_BYTES; // claimed_sum
            for m in &sc.messages {
                s += m.0.tail_evaluations.len() * GF_BYTES;
            }
        }
        s += layer.evals.len() * 2 * GF_BYTES; // (l, r) per active tree
    }
    s += proof.v.len() * GF_BYTES; // sent integer folds
    for m in &proof.messages {
        s += m.len() * GF_BYTES; // the L combined messages
    }
    for oc in &proof.opened {
        s += IDX_BYTES + oc.col_packed.len() * WORD_BYTES + oc.merkle_proof.siblings.len() * HASH_BYTES;
    }
    s
}

/// Predicted [`BatchProof`] size from the instance shape alone (`L` polynomials,
/// `num_openings` columns) — no prover run. Matches [`batch_proof_size_bytes`].
#[allow(clippy::arithmetic_side_effects)] // shape arithmetic over small bounded values
pub fn predicted_batch_proof_size_bytes(p: &IntEvalParams, l: usize, num_openings: usize) -> usize {
    let total = l << p.s; // L·2^s trees / columns / fold values
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w; // 2^t·W
    let codeword_len = row_len << 2; // rate 1/4
    let max_d = row_len.trailing_zeros() as usize; // forest tree depth = t + log₂W
    let merkle_depth = codeword_len.trailing_zeros() as usize; // = max_d + 2
    let words_per_col = total.div_ceil(64);

    let roots = total * GF_BYTES;
    // Every one of the `max_d` layers carries `total` (l, r) eval pairs.
    let forest_evals = max_d * total * 2 * GF_BYTES;
    // Layer k (k = 1..max_d−1) has a k-round sumcheck: k messages × 3 evals + claimed_sum.
    let mut sumcheck = 0usize;
    for k in 1..max_d {
        sumcheck += (3 * k + 1) * GF_BYTES;
    }
    let v = total * GF_BYTES;
    let messages = l * row_len * GF_BYTES; // L messages of row_len each
    let opened = num_openings * (IDX_BYTES + words_per_col * WORD_BYTES + merkle_depth * HASH_BYTES);

    roots + forest_evals + sumcheck + v + messages + opened
}

/// The proof-size-optimal number of folded variables `t` for an `n`-variable
/// instance (word width `word_bits`, `l` batched polynomials, `num_openings`
/// columns): the `t ∈ [1, n)` minimising [`predicted_batch_proof_size_bytes`].
/// (At W=1 this is `≈ 0.6n` — see the ledger.) The caller must still respect the
/// magnitude constraint `Σ_{i∈[t]} log₂(1+|w-weights|) < log₂(ord α)`.
pub fn proof_size_optimal_t(n: usize, l: usize, word_bits: usize, num_openings: usize) -> usize {
    (1..n)
        .min_by_key(|&t| {
            predicted_batch_proof_size_bytes(
                &IntEvalParams { t, s: n.wrapping_sub(t), word_bits },
                l,
                num_openings,
            )
        })
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small bounded instance: `2^2 × 2^2` matrix of 4-bit words.
    fn tiny_instance() -> (IntEvalParams, Vec<u128>, Vec<u128>, Vec<u128>) {
        let p = IntEvalParams { t: 2, s: 2, word_bits: 4 };
        // Deterministic pseudo-random 4-bit cells.
        let data: Vec<u128> = (0..p.cells())
            .map(|i| (i as u128).wrapping_mul(0x9E37_79B9) & 0xF)
            .collect();
        let row_weights = vec![1u128, 3, 5, 2]; // 2^t = 4 integer row weights
        let col_weights = vec![7u128, 1, 4, 2]; // 2^s = 4 column weights
        (p, data, row_weights, col_weights)
    }

    // --- M2b concrete-code wiring (test-only) -------------------------
    // The opening functions are generic over `Code: F2LinearOpener`; here we
    // instantiate the real `F_2`-RAA code. Naming `RaaF2Code` needs a concrete
    // `ZipTypes`, whose associated types pull in `crypto-bigint` — a
    // dev-dependency — so this wiring lives in test code. Mirrors the
    // `LocalBinPolyF2ZipTypes` recipe in `f2_prove`'s tests.
    use crate::poly::univariate::binary::BinaryPolyInnerProduct;
    use crate::poly::univariate::dense::{DensePolyInnerProduct, DensePolynomial};
    use crate::utils::primality::MillerRabin;
    use crate::transcript::Blake3Transcript;
    use crate::utils::inner_product::MBSInnerProduct;
    use crypto_primitives::crypto_bigint_int::Int;
    use crypto_primitives::crypto_bigint_uint::Uint;
    use crate::code::raa::RaaConfig;
    use crate::code::raa_f2::RaaF2Code;
    use crate::code::zip_types::ZipTypes;

    #[derive(Debug, Clone)]
    struct LocalF2ZipTypes<const D: usize> {}
    impl<const D: usize> ZipTypes for LocalF2ZipTypes<D> {
        const NUM_COLUMN_OPENINGS: usize = 147;
        type Eval = BinaryPoly<D>;
        type Cw = BinaryPoly<D>;
        type Fmod = Uint<{ crypto_bigint::U64::LIMBS * 4 }>;
        type PrimeTest = MillerRabin;
        type Chal = i128;
        type Pt = i128;
        type CombR = Int<{ crypto_bigint::U64::LIMBS * 8 }>;
        type Comb = DensePolynomial<Self::CombR, D>;
        type EvalDotChal = BinaryPolyInnerProduct<Self::Chal, D>;
        type CombDotChal =
            DensePolyInnerProduct<Self::CombR, Self::Chal, Self::CombR, MBSInnerProduct, D>;
        type ArrCombRDotChal = MBSInnerProduct;
    }

    #[derive(Copy, Clone)]
    struct LocalRaaConfig;
    impl RaaConfig for LocalRaaConfig {
        const PERMUTE_IN_PLACE: bool = false;
        const CHECK_FOR_OVERFLOWS: bool = false;
    }

    type IntEvalCode = RaaF2Code<LocalF2ZipTypes<64>, LocalRaaConfig, 4>;

    /// The forest tree depth `t + log₂W` (all columns share it).
    fn forest_depth(p: &IntEvalParams) -> usize {
        p.t + p.word_bits.trailing_zeros() as usize
    }

    /// Deterministic 64-bit LCG for test data (no `rand` dependency).
    fn lcg(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *state
    }

    #[test]
    fn transpose64_matches_naive() {
        let mut state = 0xdead_beef_1234_5678u64;
        let mut m = [0u64; 64];
        for w in m.iter_mut() {
            *w = lcg(&mut state);
        }
        let orig = m;
        transpose64(&mut m);
        for j in 0..64 {
            for k in 0..64 {
                assert_eq!(
                    (m[j] >> k) & 1,
                    (orig[k] >> j) & 1,
                    "transpose mismatch at (j={j}, k={k})"
                );
            }
        }
    }

    /// The direct packed-transpose commit build must reproduce the tensor
    /// path bit-for-bit: same `packed_cols`, same root. Covers full and
    /// non-power-of-two column counts, `2^s` both ≥ and < 64.
    #[test]
    fn sha_f2_packed_cols_matches_tensor_commit() {
        use crate::poly::mle::DenseMultilinearExtension;
        const D: usize = 32;
        let mut state = 0x0123_4567_89ab_cdefu64;
        for &(num_cols, num_vars) in &[(14usize, 8usize), (3, 6), (14, 10)] {
            let layout = sha_f2_layout(num_cols, num_vars, 5, F2_OPEN_NUM_COLUMNS);
            let cols: Vec<DenseMultilinearExtension<BinaryPoly<D>>> = (0..num_cols)
                .map(|_| {
                    let evals: Vec<BinaryPoly<D>> = (0..1usize << num_vars)
                        .map(|_| BinaryPoly::<D>::from(lcg(&mut state) as u32))
                        .collect();
                    DenseMultilinearExtension::from_evaluations_vec(
                        num_vars,
                        evals,
                        BinaryPoly::default(),
                    )
                })
                .collect();

            let code = code_for(&layout.p);
            let data = sha_f2_bit_tensor::<D>(&layout, &cols);
            let legacy = commit_bits(&code, &layout.p, &data);

            let packed = sha_f2_packed_cols::<D>(&layout, &cols);
            assert_eq!(
                packed, legacy.packed_cols,
                "packed transpose diverges from tensor path (cols={num_cols}, nv={num_vars})"
            );
            let fast = commit_bits_packed(&code, &layout.p, packed);
            assert_eq!(fast.root, legacy.root, "commit root diverges");
        }
    }

    /// The virtual-XOR row extraction agrees with a naive per-bit XOR of the
    /// committed column slices, on both the word-aligned (`tw ≥ 6`) and the
    /// masked sub-word path, for k = 1..6 XORed columns — at the natural
    /// split and with `x_fold_extra ∈ {1, 2}` (new row `c″` = the
    /// consecutive base rows `(c″ ≪ δ)|c_lo` concatenated).
    #[test]
    fn extract_virtual_xor_rows_matches_naive() {
        let mut state = 0x5eed_f00d_9abc_def1u64;
        // (num_vars, s, δ): tw = num_vars − s ∈ {1, 2, 6} — sub-word ×2,
        // word-aligned ×1. num_cols = 20 (log_cols = 5), bit_vars = 5.
        for &(num_vars, s, x_fold_extra) in
            &[(7usize, 6usize, 0usize), (8, 6, 0), (12, 6, 0), (8, 6, 1), (12, 6, 2)]
        {
            let tw = num_vars - s;
            let (bit_vars, log_cols, num_cols) = (5usize, 5usize, 20usize);
            let t = bit_vars + log_cols + tw;
            let layout = ShaF2Layout {
                p: IntEvalParams { t, s, word_bits: 1 },
                num_cols,
                log_cols,
                bit_vars,
                num_vars,
                tw,
                x_fold_extra,
            };
            let p_x = virtual_xor_params(&layout);
            let rows: Vec<Vec<u64>> = (0..1usize << s)
                .map(|_| (0..(1usize << t) / 64).map(|_| lcg(&mut state)).collect())
                .collect();
            let t_base = bit_vars + tw;
            let x_words = (1usize << p_x.t).div_ceil(64);
            let e_rows: Vec<Vec<u64>> = (0..p_x.cols())
                .map(|_| (0..x_words).map(|_| lcg(&mut state)).collect())
                .collect();
            // (cols, constant pattern, external?) — covers the plain XOR,
            // the constant flip, and the uncommitted-term composition.
            for (xor_cols, constant, external) in [
                (&[4usize][..], 0u128, false),
                (&[0, 19][..], 0, false),
                (&[1, 3, 7][..], 0, false),
                (&[0, 2, 5, 9, 11, 16][..], 0, false),
                (&[0, 19][..], 0xFFFF_FFFF, false), // complement-style
                (&[7][..], 0xA5A5_5A5A, true),
            ] {
                let ext = external.then_some(&e_rows[..]);
                let x_rows = extract_virtual_xor_rows(&layout, &rows, xor_cols, constant, ext);
                assert_eq!(x_rows.len(), p_x.cols());
                for (ch, x_row) in x_rows.iter().enumerate() {
                    assert_eq!(x_row.len(), x_words);
                    for bb in 0..1usize << p_x.t {
                        // New row index bb = (c_lo ≪ t_base) | b_x; source
                        // committed row c = (ch ≪ δ) | c_lo.
                        let (c_lo, b_x) = (bb >> t_base, bb & ((1 << t_base) - 1));
                        let c = (ch << x_fold_extra) | c_lo;
                        let (j, row_hi) = (b_x >> tw, b_x & ((1 << tw) - 1));
                        let mut expect = 0u64;
                        for &i in xor_cols {
                            let src = (j << (log_cols + tw)) | (i << tw) | row_hi;
                            expect ^= (rows[c][src >> 6] >> (src & 63)) & 1;
                        }
                        expect ^= u64::from((constant >> j) & 1 == 1);
                        if external {
                            expect ^= (e_rows[ch][bb >> 6] >> (bb & 63)) & 1;
                        }
                        assert_eq!(
                            (x_row[bb >> 6] >> (bb & 63)) & 1,
                            expect,
                            "x bit mismatch at c″={ch} b″={bb} (tw={tw}, δ={x_fold_extra}, k={}, const={constant:#x}, ext={external})",
                            xor_cols.len()
                        );
                    }
                }
            }
        }
    }

    /// The complement-elision predicate: pairs elide exactly when the
    /// canonical columns AND row weights coincide, neither side has an
    /// external term, and the constants differ by the all-ones pattern —
    /// with the FIRST such earlier claim as the base, order-independently
    /// of which of the pair carries the ones pattern.
    #[test]
    fn complement_elision_predicate() {
        let (bit_vars, log_cols, num_cols, tw, s) = (5usize, 1usize, 2usize, 2usize, 7usize);
        let layout = ShaF2Layout {
            p: IntEvalParams { t: bit_vars + log_cols + tw, s, word_bits: 1 },
            num_cols,
            log_cols,
            bit_vars,
            num_vars: tw + s,
            tw,
            x_fold_extra: 0,
        };
        let ones = xor_ones_pattern(&layout);
        assert_eq!(ones, 0xFFFF_FFFF);
        assert_eq!(xor_canonical_cols(&[1, 0, 1]), vec![0]); // pairs cancel
        let w1: Vec<u128> = (0..8).map(|i| 100 + i as u128).collect();
        let w2: Vec<u128> = (0..8).map(|i| 200 + i as u128).collect();
        let claims: Vec<(&[usize], u128, bool, &[u128])> = vec![
            (&[0, 1], 0, false, &w1),        // 0: a⊕W
            (&[1, 0], ones, false, &w1),     // 1: a⊕¬W, same weights → derives from 0
            (&[0, 1], ones, false, &w2),     // 2: same cols, DIFFERENT weights → active
            (&[0], ones, false, &w1),        // 3: different cols → active
            (&[0], 0, true, &w1),            // 4: external ⇒ never elides vs 3
            (&[0, 1], 0, false, &w1),        // 5: complement of 1 (= identical to 0)
        ];
        assert_eq!(
            complement_elision(&layout, &claims),
            vec![None, Some(0), None, None, None, Some(1)]
        );
    }

    /// Build the `F_2`-RAA code over the `row_len = 2^t · W` message length.
    fn code_for(p: &IntEvalParams) -> IntEvalCode {
        IntEvalCode::new(p.rows() << (p.word_bits.trailing_zeros() as usize))
    }

    /// Run the full prover flow (commit → forest GKR → γ → open).
    fn prover_side(
        p: &IntEvalParams,
        data: &[u128],
        rw: &[u128],
        alpha: Gf,
        num_openings: usize,
    ) -> (IntEvalCode, CommitHint, ProductForestProof<Gf>, OpenProof) {
        let code = code_for(p);
        let hint = commit_bits(&code, p, data);
        let mut pt = Blake3Transcript::new();
        let (fproof, _roots, _rho, _leaf_evals) = prove_fold_forest(&mut pt, p, data, rw, alpha);
        let gammas: Vec<Gf> = pt.get_field_challenges(p.cols(), &());
        let proof = prove_open(&mut pt, &hint, &gammas, num_openings);
        (code, hint, fproof, proof)
    }

    /// Re-derive the verifier's `(transcript, gammas, q_rowbit, claimed)` by
    /// replaying the forest on a fresh transcript — mirroring the prover so the
    /// γ challenges and sampled columns coincide. The returned transcript is
    /// positioned exactly where `verify_open` should resume.
    fn verifier_inputs(
        p: &IntEvalParams,
        rw: &[u128],
        alpha: Gf,
        fproof: &ProductForestProof<Gf>,
    ) -> (Blake3Transcript, Vec<Gf>, Vec<Gf>, Gf) {
        let mut vt = Blake3Transcript::new();
        let depths = vec![forest_depth(p); p.cols()];
        let vclaims = verify_product_forest(&mut vt, fproof, &depths, &()).expect("forest verifies");
        let rho: Vec<Gf> = vclaims.first().map(|(pt, _)| pt.clone()).unwrap_or_default();
        let leaf_evals: Vec<Gf> = vclaims.iter().map(|(_, e)| *e).collect();
        let gammas: Vec<Gf> = vt.get_field_challenges(p.cols(), &());
        let q = row_bit_weights(p, rw, alpha, &rho);
        let one = Gf::one();
        let claimed = leaf_evals
            .iter()
            .zip(&gammas)
            .fold(Gf::zero(), |acc, (l, g)| acc + *g * (*l - one));
        (vt, gammas, q, claimed)
    }

    /// M2b: an honest committed opening verifies — the eval read-off matches the
    /// forest's batched leaf claim, and every opened column is proximity- and
    /// Merkle-consistent.
    #[test]
    fn int_eval_open_roundtrips() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;

        let (code, hint, fproof, proof) = prover_side(&p, &data, &rw, alpha, num_openings);
        let (mut vt, gammas, q, claimed) = verifier_inputs(&p, &rw, alpha, &fproof);

        verify_open(&mut vt, &code, &hint.root, &proof, claimed, &gammas, &q, num_openings)
            .expect("honest committed opening verifies");
    }

    /// A flipped committed bit in an opened column breaks its Merkle path.
    #[test]
    fn int_eval_open_rejects_tampered_column() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;

        let (code, hint, fproof, mut proof) = prover_side(&p, &data, &rw, alpha, num_openings);
        let (mut vt, gammas, q, claimed) = verifier_inputs(&p, &rw, alpha, &fproof);

        // Flip one committed bit in the first opened column's first packed word.
        let cell = &mut proof.opened[0].col_packed[0];
        *cell = BinaryPoly::<64>::unpack_u64(cell.pack_u64() ^ 1);

        let err = verify_open(&mut vt, &code, &hint.root, &proof, claimed, &gammas, &q, num_openings)
            .expect_err("tampered column must be rejected");
        assert!(matches!(err, IntEvalOpenError::Merkle { .. }), "got {err:?}");
    }

    /// A claim other than the forest's batched leaf claim fails the read-off.
    #[test]
    fn int_eval_open_rejects_wrong_claim() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;

        let (code, hint, fproof, proof) = prover_side(&p, &data, &rw, alpha, num_openings);
        let (mut vt, gammas, q, claimed) = verifier_inputs(&p, &rw, alpha, &fproof);

        let wrong = claimed + Gf::one();
        let err = verify_open(&mut vt, &code, &hint.root, &proof, wrong, &gammas, &q, num_openings)
            .expect_err("wrong claim must be rejected");
        assert_eq!(err, IntEvalOpenError::ClaimMismatch);
    }

    /// The message `m` is bound to both the transcript and the commitment:
    /// tampering it (even with a claim adjusted so the read-off still passes)
    /// is caught — the absorbed `m` reseeds the column sampling and the
    /// proximity spot-check no longer matches the committed columns.
    #[test]
    fn int_eval_open_rejects_tampered_message() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;

        let (code, hint, fproof, mut proof) = prover_side(&p, &data, &rw, alpha, num_openings);
        let (mut vt, gammas, q, _claimed) = verifier_inputs(&p, &rw, alpha, &fproof);

        proof.m[0] = proof.m[0] + Gf::one();
        // Adjust the claim to the tampered eval so the read-off check passes;
        // the commitment binding must still reject.
        let tampered_claim = proof
            .m
            .iter()
            .zip(q.iter())
            .fold(Gf::zero(), |acc, (m, qi)| acc + *m * *qi);

        let err =
            verify_open(&mut vt, &code, &hint.root, &proof, tampered_claim, &gammas, &q, num_openings)
                .expect_err("message inconsistent with the commitment must be rejected");
        assert!(
            matches!(
                err,
                IntEvalOpenError::ColumnIndexMismatch { .. } | IntEvalOpenError::EncodingMismatch { .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn params_shape() {
        let p = IntEvalParams { t: 3, s: 4, word_bits: 32 };
        assert_eq!(p.rows(), 8);
        assert_eq!(p.cols(), 16);
        assert_eq!(p.cells(), 128);
        assert_eq!(p.cell_index(5, 9), (5 << 4) | 9);
    }

    #[test]
    fn gf_pow_agrees_with_repeated_multiply() {
        let g = Gf::from(0x9E37_79B9_7F4A_7C15u128);
        // α^0 = 1, α^1 = α, α^5 = α·α·α·α·α
        assert_eq!(gf_pow(g, 0), Gf::one());
        assert_eq!(gf_pow(g, 1), g);
        let mut acc = Gf::one();
        for _ in 0..5 {
            acc = acc * g;
        }
        assert_eq!(gf_pow(g, 5), acc);
        // Exponent law: α^a · α^b = α^{a+b}.
        assert_eq!(gf_pow(g, 7) * gf_pow(g, 11), gf_pow(g, 18));
    }

    /// Completeness of the row fold + column read-off: folding then combining
    /// columns equals the direct evaluation `Σ_{b,c} w_b e_c D[(b,c)]`.
    #[test]
    fn fold_then_columns_equals_direct() {
        let (p, data, rw, cw) = tiny_instance();
        let v = fold_values(&p, &data, &rw);
        assert_eq!(final_eval_int(&v, &cw), direct_eval_int(&p, &data, &rw, &cw));
    }

    /// The protocol product `∏_b α^{w_b·D}` equals `α^{v_c}` for every column —
    /// the relation the GKR grand product must enforce.
    #[test]
    fn exponent_fold_matches_direct_power() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let v = fold_values(&p, &data, &rw);
        let folded = exponent_fold(&p, &data, &rw, alpha);
        for c in 0..p.cols() {
            assert_eq!(folded[c], gf_pow(alpha, v[c]), "column {c}");
        }
    }

    /// The fold magnitudes stay well within the field for a bounded instance, so
    /// the exponent map is injective and there is no wraparound.
    #[test]
    fn magnitude_within_field() {
        let (p, data, rw, _cw) = tiny_instance();
        let v = fold_values(&p, &data, &rw);
        assert!(max_fold_magnitude(&v) < (1u128 << 96));
    }

    /// M1: the GKR grand product binds `α^{v_c}` to the column's bit-leaves, and
    /// its reduced claim equals the honest leaf-MLE (the value M2 binds to the
    /// committed bits).
    #[test]
    fn gkr_binds_alpha_v_c() {
        use crate::piop::lookup::gkr_product::{prove_product_tree, verify_product_tree};
        use crate::transcript::Blake3Transcript;

        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let v = fold_values(&p, &data, &rw);
        let log_w = p.word_bits.trailing_zeros() as usize;

        for c in 0..p.cols() {
            let leaves = column_leaves(&p, &data, &rw, alpha, c);
            assert_eq!(leaves.len(), p.rows() << log_w);
            let num_vars = leaves.len().trailing_zeros() as usize;

            let mut pt = Blake3Transcript::new();
            let (proof, point, leaf_eval) = prove_product_tree(&mut pt, leaves.clone(), &());

            // The grand-product root is exactly α^{v_c}: the fold is bound.
            assert_eq!(proof.root, gf_pow(alpha, v[c]), "root must equal alpha^(v_c), column {c}");

            // The reduced leaf claim equals the honest leaf-MLE at `point`.
            let eq = build_eq_x_r_vec(&point, &()).expect("nonempty point");
            let mle = eq
                .iter()
                .zip(leaves.iter())
                .fold(Gf::zero(), |acc, (e, l)| acc + *e * *l);
            assert_eq!(mle, leaf_eval, "leaf claim must equal honest leaf-MLE, column {c}");

            // Verifier roundtrip re-derives the same point and leaf claim.
            let mut vt = Blake3Transcript::new();
            let (vpoint, veval) =
                verify_product_tree(&mut vt, &proof, num_vars, &()).expect("verifier accepts");
            assert_eq!(vpoint, point);
            assert_eq!(veval, leaf_eval);
        }
    }

    /// A claimed fold value other than `α^{v_c}` is rejected by the verifier.
    #[test]
    fn gkr_wrong_fold_rejected() {
        use crate::piop::lookup::gkr_product::{prove_product_tree, verify_product_tree};
        use crate::transcript::Blake3Transcript;

        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let leaves = column_leaves(&p, &data, &rw, alpha, 0);
        let num_vars = leaves.len().trailing_zeros() as usize;

        let mut pt = Blake3Transcript::new();
        let (mut proof, _point, _leaf_eval) = prove_product_tree(&mut pt, leaves, &());
        proof.root = proof.root + alpha; // corrupt the claimed product α^{v_0}

        let mut vt = Blake3Transcript::new();
        assert!(verify_product_tree(&mut vt, &proof, num_vars, &()).is_err());
    }

    /// M2a: the forest GKR reduces all columns to a shared point, binds every
    /// `α^{v_c}`, and its batched leaf claims equal the tensor functional of the
    /// committed bits — `Σ_c γ_c(ℓ_c−1) = Σ_{(b,j)} q_rowbit·Σ_c γ_c·bit_{(b,c),j}`.
    /// This is the full M2 reduction, validated in the clear (M2b replaces the
    /// right-hand side with the committed Brakedown opening).
    #[test]
    fn forest_reduces_to_tensor_functional() {
        use crate::piop::lookup::gkr_product::verify_product_forest;
        use crate::transcript::Blake3Transcript;

        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let v = fold_values(&p, &data, &rw);
        let log_w = p.word_bits.trailing_zeros() as usize;
        let mask = p.word_bits.wrapping_sub(1);
        let one = Gf::one();

        let mut pt = Blake3Transcript::new();
        let (proof, roots, rho, leaf_evals) = prove_fold_forest(&mut pt, &p, &data, &rw, alpha);

        // (i) per-column roots bind α^{v_c}.
        for c in 0..p.cols() {
            assert_eq!(roots[c], gf_pow(alpha, v[c]), "root must equal alpha^(v_c), column {c}");
        }

        // (ii) the forest verifier accepts and re-derives the SAME shared point
        // for every column — the linchpin that makes the functional tensor-structured.
        let mut vt = Blake3Transcript::new();
        let depths = vec![rho.len(); p.cols()];
        let vclaims =
            verify_product_forest(&mut vt, &proof, &depths, &()).expect("forest verifies");
        for (vpt, _vev) in &vclaims {
            assert_eq!(vpt, &rho, "all trees share the reduction point");
        }
        assert_eq!(
            vclaims.iter().map(|(_, e)| *e).collect::<Vec<_>>(),
            leaf_evals,
            "verifier leaf claims match prover"
        );

        // (iii) batched leaf claims equal the tensor functional of the bits.
        let q = row_bit_weights(&p, &rw, alpha, &rho);
        let gammas: Vec<Gf> = (0..p.cols())
            .map(|c| Gf::from((c as u128).wrapping_mul(0x1234_5678).wrapping_add(1)))
            .collect();
        let lhs = leaf_evals
            .iter()
            .zip(&gammas)
            .fold(Gf::zero(), |acc, (l, g)| acc + *g * (*l - one));
        let mut rhs = Gf::zero();
        for (i, &qi) in q.iter().enumerate() {
            let b = i >> log_w;
            let j = i & mask;
            let mut col_sum = Gf::zero();
            for c in 0..p.cols() {
                if (data[p.cell_index(b, c)] >> j) & 1 == 1 {
                    col_sum = col_sum + gammas[c];
                }
            }
            rhs = rhs + qi * col_sum;
        }
        assert_eq!(lhs, rhs, "batched leaf claims must equal the tensor functional of the bits");
    }

    /// M3: an honest end-to-end integer-MLE evaluation verifies — the forest
    /// binding, the `α^{v_c}` integer binding, the committed opening, and the
    /// in-the-clear read-off all agree on `Σ_{b,c} w_b·e_c·D[(b,c)]`.
    #[test]
    fn int_eval_end_to_end() {
        let (p, data, rw, cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);
        let y = direct_eval_int(&p, &data, &rw, &cw); // = Σ_c e_c·v_c

        let mut pt = Blake3Transcript::new();
        let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);

        let mut vt = Blake3Transcript::new();
        verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, 1u128, alpha, y, num_openings)
            .expect("end-to-end integer-MLE evaluation verifies");
    }

    /// A wrong claimed evaluation is caught by the in-the-clear read-off.
    #[test]
    fn int_eval_rejects_wrong_evaluation() {
        let (p, data, rw, cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);
        let y = direct_eval_int(&p, &data, &rw, &cw);

        let mut pt = Blake3Transcript::new();
        let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);

        let mut vt = Blake3Transcript::new();
        let err = verify(
            &mut vt,
            &code,
            &hint.root,
            &proof,
            &p,
            &rw,
            &cw,
            1u128,
            alpha,
            y.wrapping_add(1),
            num_openings,
        )
        .expect_err("wrong evaluation must be rejected");
        assert_eq!(err, IntEvalError::ReadOff);
    }

    /// Tampering a sent fold value `v_c` breaks its `α^{v_c} = root_c` binding.
    #[test]
    fn int_eval_rejects_tampered_v() {
        let (p, data, rw, cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);
        let y = direct_eval_int(&p, &data, &rw, &cw);

        let mut pt = Blake3Transcript::new();
        let mut proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
        proof.v[0] = proof.v[0].wrapping_add(1);

        let mut vt = Blake3Transcript::new();
        let err = verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, 1u128, alpha, y, num_openings)
            .expect_err("tampered v_c must be rejected");
        assert_eq!(err, IntEvalError::RootBinding { c: 0 });
    }

    /// Multi-word leaf packing: `2^s = 128 > 64` columns ⇒ `⌈2^s/64⌉ = 2` packed
    /// `BinaryPoly<64>` words per Merkle leaf, exercising the `c/64`,`c%64`
    /// bit-addressing across words in both `commit_bits` and the spot-check.
    #[test]
    fn int_eval_open_roundtrips_multiword() {
        let p = IntEvalParams { t: 2, s: 7, word_bits: 4 };
        let mask = (1u128 << p.word_bits).wrapping_sub(1);
        let data: Vec<u128> =
            (0..p.cells()).map(|i| (i as u128).wrapping_mul(0x9E37_79B9) & mask).collect();
        let rw: Vec<u128> =
            (0..p.rows()).map(|b| ((b as u128).wrapping_mul(3) & 7).wrapping_add(1)).collect();
        let cw: Vec<u128> =
            (0..p.cols()).map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1)).collect();
        let alpha = smallest_generator();
        let num_openings = 24;

        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);
        let y = direct_eval_int(&p, &data, &rw, &cw);

        let mut pt = Blake3Transcript::new();
        let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
        assert_eq!(proof.open.opened[0].col_packed.len(), 2, "two packed words per column");

        let mut vt = Blake3Transcript::new();
        verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, 1u128, alpha, y, num_openings)
            .expect("multiword-packed opening verifies");
    }

    /// `is_generator` / order-factorization sanity: the prime list multiplies to
    /// `2^128 − 1` (squarefree), `smallest_generator()` is a generator, and the
    /// trivial elements `0`/`1` are not.
    #[test]
    fn generator_and_order_factorization() {
        // ∏ p = 2^128 − 1 = u128::MAX (so the factor list is complete & squarefree).
        let product = GF128_ORDER_PRIME_FACTORS
            .iter()
            .fold(1u128, |acc, &p| acc.wrapping_mul(p));
        assert_eq!(product, u128::MAX, "prime factors must multiply to 2^128 − 1");
        assert_eq!(GF128_MULT_ORDER, u128::MAX);

        assert!(is_generator(smallest_generator()));
        assert!(!is_generator(Gf::one()));
        assert!(!is_generator(Gf::zero()));
    }

    /// `verify` rejects a challenge `α` that does not generate `K^×`: the integer
    /// binding `α^{v_c} = root_c` would not be injective.
    #[test]
    fn int_eval_rejects_non_generator_challenge() {
        let (p, data, rw, cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);
        let y = direct_eval_int(&p, &data, &rw, &cw);

        let mut pt = Blake3Transcript::new();
        let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);

        // Verify with a non-generator (the identity) — rejected before any binding.
        let mut vt = Blake3Transcript::new();
        let err = verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, 1u128, Gf::one(), y, num_openings)
            .expect_err("non-generator challenge must be rejected");
        assert_eq!(err, IntEvalError::ChallengeNotGenerator);
    }

    // A tiny prime field F_p, p = 2^61 − 1 (Mersenne M61, char ≠ 2), standing in
    // for the §9 "evaluation field". Products of two `< 2^61` reps fit in `u128`,
    // so `wrapping_*` never wraps before the `rem_euclid` reduction.
    const FP_MOD: u128 = 2_305_843_009_213_693_951; // 2^61 − 1
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct Fp(u128);
    impl From<u128> for Fp {
        fn from(x: u128) -> Self {
            Fp(x.rem_euclid(FP_MOD))
        }
    }
    impl core::ops::Add for Fp {
        type Output = Fp;
        fn add(self, o: Fp) -> Fp {
            Fp(self.0.wrapping_add(o.0).rem_euclid(FP_MOD))
        }
    }
    impl core::ops::Mul for Fp {
        type Output = Fp;
        fn mul(self, o: Fp) -> Fp {
            Fp(self.0.wrapping_mul(o.0).rem_euclid(FP_MOD))
        }
    }

    /// (c) Field-valued read-off: the column weights `e_c` and the global scalar
    /// `G_r` live in a non-char-2 evaluation field `F_p` (M61), *distinct* from
    /// the binding field `K = GF(2^128)`. The same `prove` output verifies against
    /// the `F_p` claim `G_r·Σ_c e_c·v_c`; a wrong `G_r` is rejected by the read-off.
    #[test]
    fn int_eval_field_valued_readoff() {
        let (p, data, rw, _cw) = tiny_instance();
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);

        // Arbitrary F_p column weights + a non-unit G_r.
        let e: Vec<Fp> =
            (0..p.cols()).map(|c| Fp::from((c as u128).wrapping_mul(11).wrapping_add(2))).collect();
        let g_r = Fp::from(7);
        // Independent F_p claim on the honest folds: G_r·Σ_c e_c·v_c.
        let v = fold_values(&p, &data, &rw);
        let mut acc = Fp::from(0);
        for (vc, ec) in v.iter().zip(e.iter()) {
            acc = acc + *ec * Fp::from(*vc);
        }
        let claimed = g_r * acc;

        let mut pt = Blake3Transcript::new();
        let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);

        let mut vt = Blake3Transcript::new();
        verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &e, g_r, alpha, claimed, num_openings)
            .expect("field-valued read-off verifies");

        // A wrong G_r is rejected.
        let mut vt2 = Blake3Transcript::new();
        let err = verify(
            &mut vt2, &code, &hint.root, &proof, &p, &rw, &e, g_r + Fp::from(1), alpha, claimed,
            num_openings,
        )
        .expect_err("wrong G_r must be rejected");
        assert_eq!(err, IntEvalError::ReadOff);
    }

    /// Cost profile of `commit` and `prove`: runs the *real* `commit_bits` /
    /// `prove` under the `prof` scopes (commit:encode/pack/merkle,
    /// prove:leaves/gkr:*/vfold/open) and dumps the nested wall-clock tree. Run:
    /// `OBLONG_PROFILE=1 cargo test ... --release -- --ignored --nocapture cost_profile`.
    /// Without `OBLONG_PROFILE` the scopes are inert and this just times nothing.
    #[test]
    #[ignore = "manual cost profile (set OBLONG_PROFILE=1)"]
    fn cost_profile() {
        let alpha = smallest_generator();
        let num_openings = 987;
        for s in [12usize, 16] {
            let p = IntEvalParams { t: 4, s, word_bits: 8 };
            let mask = (1u128 << p.word_bits).wrapping_sub(1);
            let data: Vec<u128> = (0..p.cells())
                .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
                .collect();
            let rw: Vec<u128> =
                (0..p.rows()).map(|b| ((b as u128).wrapping_mul(3) & 7).wrapping_add(1)).collect();
            let code = code_for(&p);

            let hint = {
                let _g = crate::utils::prof::scope("commit");
                commit_bits(&code, &p, &data)
            };
            let mut pt = Blake3Transcript::new();
            let proof = {
                let _g = crate::utils::prof::scope("prove");
                prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings)
            };
            let _ = &proof;
            crate::utils::prof::dump_and_reset(&format!(
                "int-eval n={} (t={}, s={}, W={})",
                p.t + p.s,
                p.t,
                p.s,
                p.word_bits
            ));
        }
    }

    /// Scaling sweep over the number of MLE variables `n = t + s` (fixed small
    /// `t`, growing the column side `s`). Run:
    /// `cargo test ... --release -- --ignored --nocapture scaling_sweep`.
    /// Prints median-of-runs commit / prove / verify wall-clock per size.
    #[test]
    #[ignore = "manual scaling sweep over n = t + s (up to 20 vars)"]
    fn scaling_sweep() {
        use std::time::{Duration, Instant};
        const W: usize = 1; // 1-bit cells (binary data)
        let alpha = smallest_generator();
        let num_openings = 987;
        let med = |mut v: Vec<Duration>| {
            v.sort_unstable();
            v[v.len() / 2].as_secs_f64() * 1000.0
        };

        eprintln!("W=1, single-poly, proof-size-optimal t per n");
        eprintln!("  n  t*  s   cells       commit_ms   prove_ms  verify_ms");
        for n in 9..=24usize {
            let t = proof_size_optimal_t(n, 1, W, num_openings);
            let s = n - t;
            let p = IntEvalParams { t, s, word_bits: W };
            let mask = (1u128 << W).wrapping_sub(1);
            let data: Vec<u128> = (0..p.cells())
                .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
                .collect();
            let rw: Vec<u128> =
                (0..p.rows()).map(|b| ((b as u128).wrapping_mul(3) & 7).wrapping_add(1)).collect();
            let cw: Vec<u128> =
                (0..p.cols()).map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1)).collect();
            let code = code_for(&p);
            let y = direct_eval_int(&p, &data, &rw, &cw);

            // 1 warm-up (discarded) + a few timed runs; fewer at large sizes.
            let runs = if n >= 18 { 2 } else { 3 };
            let (mut c_t, mut p_t, mut v_t) = (Vec::new(), Vec::new(), Vec::new());
            for run in 0..=runs {
                let t0 = Instant::now();
                let hint = commit_bits(&code, &p, &data);
                let dc = t0.elapsed();

                let mut pt = Blake3Transcript::new();
                let t1 = Instant::now();
                let proof = prove(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
                let dp = t1.elapsed();

                let mut vt = Blake3Transcript::new();
                let t2 = Instant::now();
                verify(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, 1u128, alpha, y, num_openings)
                    .expect("scaling-sweep verify");
                let dv = t2.elapsed();

                if run > 0 {
                    c_t.push(dc);
                    p_t.push(dp);
                    v_t.push(dv);
                }
            }
            eprintln!(
                "{:>3} {:>3} {:>3} {:>11} {:>11.3} {:>10.3} {:>10.3}",
                n,
                t,
                s,
                p.cells(),
                med(c_t),
                med(p_t),
                med(v_t),
            );
        }
    }

    // --- Batched PCS tests -------------------------------------------
    /// `L=4` polynomials, distinct data + distinct per-poly row/column weights
    /// (separate randomness), committed together; `g_r = 1`, integer read-off.
    fn batch_instance() -> (
        IntEvalParams,
        usize,
        IntEvalCode,
        Vec<Vec<u128>>,
        Vec<Vec<u128>>,
        Vec<Vec<u128>>,
        Vec<u128>,
        Vec<u128>,
    ) {
        let p = IntEvalParams { t: 2, s: 2, word_bits: 4 };
        let l = 4usize;
        let code = code_for(&p); // row_len = 2^t·W is independent of s and L
        let data: Vec<Vec<u128>> = (0..l)
            .map(|ell| {
                (0..p.cells())
                    .map(|i| {
                        (i as u128)
                            .wrapping_mul(0x9E37_79B9)
                            .wrapping_add((ell as u128).wrapping_mul(7))
                            & 0xF
                    })
                    .collect()
            })
            .collect();
        let rw: Vec<Vec<u128>> = (0..l)
            .map(|ell| {
                (0..p.rows())
                    .map(|b| ((b as u128).wrapping_add(ell as u128) & 3).wrapping_add(1))
                    .collect()
            })
            .collect();
        let cw: Vec<Vec<u128>> = (0..l)
            .map(|ell| {
                (0..p.cols())
                    .map(|c| {
                        ((c as u128).wrapping_add((ell as u128).wrapping_mul(2)) & 3).wrapping_add(1)
                    })
                    .collect()
            })
            .collect();
        let g_r = vec![1u128; l];
        let claimed: Vec<u128> =
            (0..l).map(|ell| direct_eval_int(&p, &data[ell], &rw[ell], &cw[ell])).collect();
        (p, l, code, data, rw, cw, g_r, claimed)
    }

    #[test]
    fn batch_roundtrips() {
        let (p, _l, code, data, rw, cw, g_r, claimed) = batch_instance();
        let alpha = smallest_generator();
        let n = 32;
        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, n);
        let mut vt = Blake3Transcript::new();
        verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &claimed, n)
            .expect("batched roundtrip verifies");
    }

    /// A wrong per-poly claim is caught by that polynomial's read-off.
    #[test]
    fn batch_rejects_wrong_claim() {
        let (p, _l, code, data, rw, cw, g_r, claimed) = batch_instance();
        let alpha = smallest_generator();
        let n = 32;
        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, n);

        let mut bad = claimed.clone();
        bad[2] = bad[2].wrapping_add(1);
        let mut vt = Blake3Transcript::new();
        let err = verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &bad, n)
            .expect_err("wrong claim must be rejected");
        assert_eq!(err, IntEvalError::ReadOff);
    }

    /// A tampered committed bit breaks the shared Merkle path (one tree for all L).
    #[test]
    fn batch_rejects_tampered_column() {
        let (p, _l, code, data, rw, cw, g_r, claimed) = batch_instance();
        let alpha = smallest_generator();
        let n = 32;
        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let mut proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, n);

        let cell = &mut proof.opened[0].col_packed[0];
        *cell = BinaryPoly::<64>::unpack_u64(cell.pack_u64() ^ 1);
        let mut vt = Blake3Transcript::new();
        let err = verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &claimed, n)
            .expect_err("tampered column must be rejected");
        assert!(matches!(err, IntEvalError::Open(IntEvalOpenError::Merkle { .. })), "got {err:?}");
    }

    /// Batched-vs-`L`-separate amortization: for growing batch size `L` (fixed
    /// per-poly shape), time the batched path against `L` independent single-poly
    /// proofs. Run:
    /// `cargo test ... --release -- --ignored --nocapture batch_amortization`.
    #[test]
    #[ignore = "manual: batched vs L-separate amortization"]
    fn batch_amortization() {
        use std::time::{Duration, Instant};
        const T: usize = 4;
        const W: usize = 8;
        let alpha = smallest_generator();
        let num_openings = 987;
        let med = |mut v: Vec<Duration>| {
            v.sort_unstable();
            v[v.len() / 2].as_secs_f64() * 1000.0
        };

        for &s in &[4usize, 6] {
            let p = IntEvalParams { t: T, s, word_bits: W };
            let code = code_for(&p);
            eprintln!("--- per-poly t={T} s={s} W={W} (n={}) ---", T + s);
            eprintln!("  L | batched c/p/v (ms)      | separate c/p/v (ms)     | prove×  verify×");
            for &ll in &[1usize, 2, 4, 8, 16] {
                let data: Vec<Vec<u128>> = (0..ll)
                    .map(|ell| {
                        (0..p.cells())
                            .map(|i| {
                                (i as u128)
                                    .wrapping_mul(0x9E37_79B9)
                                    .wrapping_add((ell as u128).wrapping_mul(7))
                                    & 0xFF
                            })
                            .collect()
                    })
                    .collect();
                let rw: Vec<Vec<u128>> = (0..ll)
                    .map(|ell| {
                        (0..p.rows())
                            .map(|b| ((b as u128).wrapping_add(ell as u128) & 7).wrapping_add(1))
                            .collect()
                    })
                    .collect();
                let cw: Vec<Vec<u128>> = (0..ll)
                    .map(|_| (0..p.cols()).map(|c| ((c as u128) & 7).wrapping_add(1)).collect())
                    .collect();
                let g_r = vec![1u128; ll];
                let claimed: Vec<u128> =
                    (0..ll).map(|ell| direct_eval_int(&p, &data[ell], &rw[ell], &cw[ell])).collect();

                let runs = 3;
                let (mut bc, mut bp, mut bv) = (Vec::new(), Vec::new(), Vec::new());
                let (mut sc, mut sp, mut sv) = (Vec::new(), Vec::new(), Vec::new());
                for r in 0..=runs {
                    // batched
                    let t0 = Instant::now();
                    let hint = commit_batch(&code, &p, &data);
                    let dc = t0.elapsed();
                    let mut pt = Blake3Transcript::new();
                    let t1 = Instant::now();
                    let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
                    let dp = t1.elapsed();
                    let mut vt = Blake3Transcript::new();
                    let t2 = Instant::now();
                    verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &claimed, num_openings)
                        .expect("batched verify");
                    let dv = t2.elapsed();

                    // L independent single-poly proofs
                    let (mut sc1, mut sp1, mut sv1) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
                    for ell in 0..ll {
                        let u0 = Instant::now();
                        let h = commit_bits(&code, &p, &data[ell]);
                        sc1 += u0.elapsed();
                        let mut p1 = Blake3Transcript::new();
                        let u1 = Instant::now();
                        let pr = prove(&mut p1, &h, &p, &data[ell], &rw[ell], alpha, num_openings);
                        sp1 += u1.elapsed();
                        let mut v1 = Blake3Transcript::new();
                        let u2 = Instant::now();
                        verify(&mut v1, &code, &h.root, &pr, &p, &rw[ell], &cw[ell], 1u128, alpha, claimed[ell], num_openings)
                            .expect("single verify");
                        sv1 += u2.elapsed();
                    }
                    if r > 0 {
                        bc.push(dc);
                        bp.push(dp);
                        bv.push(dv);
                        sc.push(sc1);
                        sp.push(sp1);
                        sv.push(sv1);
                    }
                }
                let (bcm, bpm, bvm) = (med(bc), med(bp), med(bv));
                let (scm, spm, svm) = (med(sc), med(sp), med(sv));
                eprintln!(
                    "{:>3} | {:>6.2}/{:>7.2}/{:>6.2} | {:>6.2}/{:>7.2}/{:>6.2} | {:>5.1}  {:>5.1}",
                    ll, bcm, bpm, bvm, scm, spm, svm, spm / bpm, svm / bvm,
                );
            }
        }
    }

    /// A **non-power-of-two** batch (`L=5`) verifies — the batched commit handles
    /// any `L` (only the per-poly `2^s` must be a power of two).
    #[test]
    fn batch_roundtrips_non_pow2() {
        let p = IntEvalParams { t: 2, s: 2, word_bits: 4 };
        let l = 5usize;
        let alpha = smallest_generator();
        let n = 32;
        let code = code_for(&p);
        let data: Vec<Vec<u128>> = (0..l)
            .map(|ell| {
                (0..p.cells())
                    .map(|i| (i as u128).wrapping_mul(0x9E37_79B9).wrapping_add((ell as u128).wrapping_mul(7)) & 0xF)
                    .collect()
            })
            .collect();
        let rw: Vec<Vec<u128>> = (0..l)
            .map(|ell| (0..p.rows()).map(|b| ((b as u128).wrapping_add(ell as u128) & 3).wrapping_add(1)).collect())
            .collect();
        let cw: Vec<Vec<u128>> =
            (0..l).map(|_| (0..p.cols()).map(|c| ((c as u128) & 3).wrapping_add(1)).collect()).collect();
        let g_r = vec![1u128; l];
        let claimed: Vec<u128> =
            (0..l).map(|ell| direct_eval_int(&p, &data[ell], &rw[ell], &cw[ell])).collect();

        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, n);
        let mut vt = Blake3Transcript::new();
        verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &claimed, n)
            .expect("L=5 batched roundtrip verifies");
    }

    /// Batch-of-`L` scaling over `n = t + s` (fixed `L`, per-poly `t`,`W`). Run:
    /// `cargo test ... --release -- --ignored --nocapture batch_sweep`.
    #[test]
    #[ignore = "manual: batch-of-L scaling over n = t + s"]
    fn batch_sweep() {
        use std::time::{Duration, Instant};
        const W: usize = 1;
        const L: usize = 5;
        let alpha = smallest_generator();
        let num_openings = 987;
        let med = |mut v: Vec<Duration>| {
            v.sort_unstable();
            v[v.len() / 2].as_secs_f64() * 1000.0
        };

        eprintln!("L={L}, W=1, proof-size-optimal t per n");
        eprintln!("  n  t*  s   cells/poly   commit_ms   prove_ms  verify_ms");
        for n in 9..=24usize {
            let t = proof_size_optimal_t(n, L, W, num_openings);
            let s = n - t;
            let p = IntEvalParams { t, s, word_bits: W };
            let code = code_for(&p);
            let data: Vec<Vec<u128>> = (0..L)
                .map(|ell| {
                    (0..p.cells())
                        .map(|i| {
                            (i as u128).wrapping_mul(0x9E37_79B9).wrapping_add((ell as u128).wrapping_mul(7)) & 1
                        })
                        .collect()
                })
                .collect();
            let rw: Vec<Vec<u128>> = (0..L)
                .map(|ell| (0..p.rows()).map(|b| ((b as u128).wrapping_add(ell as u128) & 7).wrapping_add(1)).collect())
                .collect();
            let cw: Vec<Vec<u128>> =
                (0..L).map(|_| (0..p.cols()).map(|c| ((c as u128) & 7).wrapping_add(1)).collect()).collect();
            let g_r = vec![1u128; L];
            let claimed: Vec<u128> =
                (0..L).map(|ell| direct_eval_int(&p, &data[ell], &rw[ell], &cw[ell])).collect();

            let runs = if n >= 18 { 2 } else { 3 };
            let (mut c_t, mut p_t, mut v_t) = (Vec::new(), Vec::new(), Vec::new());
            for r in 0..=runs {
                let t0 = Instant::now();
                let hint = commit_batch(&code, &p, &data);
                let dc = t0.elapsed();
                let mut pt = Blake3Transcript::new();
                let t1 = Instant::now();
                let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
                let dp = t1.elapsed();
                let mut vt = Blake3Transcript::new();
                let t2 = Instant::now();
                verify_batch(&mut vt, &code, &hint.root, &proof, &p, &rw, &cw, &g_r, alpha, &claimed, num_openings)
                    .expect("batch-sweep verify");
                let dv = t2.elapsed();
                if r > 0 {
                    c_t.push(dc);
                    p_t.push(dp);
                    v_t.push(dv);
                }
            }
            eprintln!(
                "{:>3} {:>3} {:>3} {:>11} {:>11.3} {:>10.3} {:>10.3}",
                n,
                t,
                s,
                p.cells(),
                med(c_t),
                med(p_t),
                med(v_t),
            );
        }
    }

    /// The shape-only proof-size formula matches a walked real proof (W=1 batch).
    #[test]
    fn proof_size_formula_matches() {
        let p = IntEvalParams { t: 4, s: 4, word_bits: 1 };
        let l = 2usize;
        let alpha = smallest_generator();
        let num_openings = 24;
        let code = code_for(&p);
        let data: Vec<Vec<u128>> = (0..l)
            .map(|ell| (0..p.cells()).map(|i| (i as u128).wrapping_add(ell as u128) & 1).collect())
            .collect();
        let rw: Vec<Vec<u128>> =
            (0..l).map(|_| (0..p.rows()).map(|b| ((b as u128) & 3).wrapping_add(1)).collect()).collect();
        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let proof = prove_batch(&mut pt, &hint, &p, &data, &rw, alpha, num_openings);
        assert_eq!(
            batch_proof_size_bytes(&proof),
            predicted_batch_proof_size_bytes(&p, l, num_openings),
            "shape formula must equal the walked proof size",
        );
    }

    /// Proof-size-optimal `t` over `n` for the L=5, W=1 batch (shape formula —
    /// no prover run, so any `n`). Prints `t*`, the size there, and the
    /// breakdown, vs the magnitude-default `t=1`. Run:
    /// `cargo test ... --release -- --ignored --nocapture batch_proof_size_table`.
    #[test]
    #[ignore = "manual: proof-size-optimal t over n (W=1, L=5)"]
    fn batch_proof_size_table() {
        const W: usize = 1;
        const L: usize = 5;
        let num_openings = 987;
        let kb = |b: usize| (b as f64) / 1024.0;
        eprintln!("W={W}, L={L}, openings={num_openings}");
        eprintln!("  n | t*  s* | proof@t* | proof@t=1 | merkle | 2^s-terms | msgs   (KB)");
        for n in [12usize, 16, 20, 24, 28] {
            let (mut best_t, mut best) = (1usize, usize::MAX);
            for t in 1..n {
                let p = IntEvalParams { t, s: n - t, word_bits: W };
                let sz = predicted_batch_proof_size_bytes(&p, L, num_openings);
                if sz < best {
                    best = sz;
                    best_t = t;
                }
            }
            let p1 = IntEvalParams { t: 1, s: n - 1, word_bits: W };
            let sz1 = predicted_batch_proof_size_bytes(&p1, L, num_openings);
            // breakdown at t*
            let merkle = num_openings * (best_t + 2) * 32; // depth = t*+2 (W=1)
            let msgs = L * (1usize << best_t) * 16; // L messages of row_len=2^t*
            let rest = best.saturating_sub(merkle).saturating_sub(msgs); // forest+v+roots+open+sumcheck
            eprintln!(
                "{:>3} | {:>2} {:>3} | {:>8.1} | {:>9.1} | {:>6.1} | {:>9.1} | {:>6.1}",
                n,
                best_t,
                n - best_t,
                kb(best),
                kb(sz1),
                kb(merkle),
                kb(rest),
                kb(msgs),
            );
        }
    }

    // =================================================================
    // Field-valued (mod-`q`) MLE-evaluation tests (X-note §6).
    //
    // The wired evaluation field is a concrete 100-bit prime `q = 2^100 − 15`
    // (char ≠ 2), backed by `u128` with a Russian-peasant `mulmod` so a product of
    // two `< q` reps never needs a 256-bit intermediate (`q < 2^127` keeps every
    // `addmod`/doubling inside `u128`). It exercises `R = 𝔽_q` of
    // `verify_mle_eval_mod_q<…, R>` exactly as `Fp` (M61) exercises the bounded read-off.

    /// `q = 2^100 − 15` (a 100-bit prime; ∏-free reduction not needed — Russian
    /// peasant). `FQ_BITS = ⌈log₂q⌉ = 100`.
    const FQ_MOD: u128 = (1u128 << 100).wrapping_sub(15);
    const FQ_BITS: usize = 100;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct Fq(u128); // invariant: 0 ≤ .0 < FQ_MOD

    /// `(a + b) mod q` for `a, b < q`. `q < 2^127` ⟹ `a + b < 2^128` fits in `u128`.
    fn fq_add(a: u128, b: u128) -> u128 {
        let s = a.wrapping_add(b);
        if s >= FQ_MOD { s.wrapping_sub(FQ_MOD) } else { s }
    }
    /// `(a · b) mod q` by Russian-peasant doubling — every double stays `< 2^128`
    /// since `q < 2^127`, so no 256-bit product is needed.
    fn fq_mul(a: u128, b: u128) -> u128 {
        let mut a = a.rem_euclid(FQ_MOD);
        let mut b = b.rem_euclid(FQ_MOD);
        let mut r = 0u128;
        while b != 0 {
            if b & 1 == 1 {
                r = fq_add(r, a);
            }
            a = fq_add(a, a);
            b >>= 1;
        }
        r
    }
    impl From<u128> for Fq {
        fn from(x: u128) -> Self {
            Fq(x.rem_euclid(FQ_MOD))
        }
    }
    impl core::ops::Add for Fq {
        type Output = Fq;
        fn add(self, o: Fq) -> Fq {
            Fq(fq_add(self.0, o.0))
        }
    }
    impl core::ops::Sub for Fq {
        type Output = Fq;
        fn sub(self, o: Fq) -> Fq {
            Fq(fq_add(self.0, FQ_MOD.wrapping_sub(o.0)))
        }
    }
    impl core::ops::Mul for Fq {
        type Output = Fq;
        fn mul(self, o: Fq) -> Fq {
            Fq(fq_mul(self.0, o.0))
        }
    }

    /// The eq-tensor `eq(b, point)` over `𝔽_q`, `point[i]` ↔ bit `i` of `b`
    /// (little-endian): `acc[b] = ∏_i (bit_i(b) ? point_i : 1 − point_i)`.
    fn eq_table_fq(point: &[Fq]) -> Vec<Fq> {
        let one = Fq::from(1u128);
        let mut acc = vec![one];
        for &r in point {
            let mut next = Vec::with_capacity(acc.len().wrapping_mul(2));
            for &e in &acc {
                next.push(e * (one - r));
                next.push(e * r);
            }
            acc = next;
        }
        acc
    }

    /// The canonical integer row weights `w_b = eq(b, r₁) mod q ∈ [0, q)`.
    fn row_weights_q_from(r1: &[Fq]) -> Vec<u128> {
        eq_table_fq(r1).iter().map(|w| w.0).collect()
    }
    /// The 𝔽_q column weights `e_c = eq(c, r₂)`.
    fn col_weights_from(r2: &[Fq]) -> Vec<Fq> {
        eq_table_fq(r2)
    }

    /// **M0 reference**: the genuine integer MLE evaluation reduced mod `q`,
    /// `y = Σ_{b,c} eq(b,r₁)·eq(c,r₂)·INT(D(b,c))` in `𝔽_q` (every product through
    /// `fq_mul`, so no big-int).
    fn mle_eval_mod_q_ref(p: &IntEvalParams, data: &[u128], r1: &[Fq], r2: &[Fq]) -> Fq {
        let eq1 = eq_table_fq(r1);
        let eq2 = eq_table_fq(r2);
        let mut acc = Fq::from(0u128);
        for b in 0..p.rows() {
            for c in 0..p.cols() {
                acc = acc + eq1[b] * eq2[c] * Fq::from(data[p.cell_index(b, c)]);
            }
        }
        acc
    }

    /// Deterministic pseudo-random `𝔽_q` evaluation point (full-width ~100-bit weights).
    fn fq_point(seed: u128, len: usize) -> Vec<Fq> {
        (0..len)
            .map(|i| {
                Fq::from(
                    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .wrapping_add((i as u128).wrapping_mul(0xD1B5_4A32_D192_ED03))
                        .wrapping_add(0xA5A5_5A5A_1234_5678),
                )
            })
            .collect()
    }

    /// W-bit pseudo-random cells.
    fn mod_q_data(p: &IntEvalParams, seed: u128) -> Vec<u128> {
        let mask = (1u128 << p.word_bits).wrapping_sub(1);
        (0..p.cells())
            .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(seed) & mask)
            .collect()
    }

    /// `c_w = 127 − t − W` and `L = ⌈q_bits / c_w⌉` over a range of shapes.
    #[test]
    fn mod_q_chunk_params() {
        // ((t, s, W), expected c_w, expected L) at q_bits = 100.
        let cases: [((usize, usize, usize), usize, usize); 5] = [
            ((2, 2, 1), 124, 1),  // 127−3 = 124 ≥ 100 ⇒ L = 1 (headroom window)
            ((4, 4, 32), 91, 2),  // 127−36 = 91  ⇒ ⌈100/91⌉ = 2
            ((6, 6, 32), 89, 2),  // 127−38 = 89  ⇒ ⌈100/89⌉ = 2
            ((20, 2, 1), 106, 1), // 127−21 = 106 ⇒ L = 1
            ((27, 1, 1), 99, 2),  // 127−28 = 99  ⇒ ⌈100/99⌉ = 2 (t+W = 28, just past the window)
        ];
        for ((t, s, word_bits), exp_cw, exp_l) in cases {
            let p = IntEvalParams { t, s, word_bits };
            assert_eq!(mod_q_chunk_width(&p), exp_cw, "c_w for t={t}, W={word_bits}");
            assert_eq!(mod_q_num_chunks(&p, FQ_BITS), exp_l, "L for t={t}, W={word_bits}");
            assert_eq!(exp_l, FQ_BITS.div_ceil(exp_cw), "L = ⌈q_bits / c_w⌉");
        }
    }

    /// M0 identity 1: chunk a wide weight, integer-recombine `Σ_l W^{(l)}·2^{c_w·l}` → identity.
    #[test]
    fn mod_q_chunk_recombine_is_identity() {
        let p = IntEvalParams { t: 4, s: 4, word_bits: 32 }; // c_w = 91, L = 2
        let c_w = mod_q_chunk_width(&p);
        let l = mod_q_num_chunks(&p, FQ_BITS);
        for &w in &[0u128, 1, 2, FQ_MOD.wrapping_sub(1), 0x1234_5678_9ABC_DEF0_1122, 1u128 << 99] {
            let w = w.rem_euclid(FQ_MOD);
            let chunks = chunk_row_weights(&[w], c_w, l);
            let mut acc = 0u128;
            let mut shift = 0usize;
            for ch in &chunks {
                acc = acc.wrapping_add(ch[0].wrapping_mul(1u128 << shift));
                shift = shift.wrapping_add(c_w);
            }
            assert_eq!(acc, w, "chunk ∘ integer-recombine must be identity for w={w}");
        }
    }

    /// M0 identity 2: `recombine_read_off` (the in-the-clear 𝔽_q read-off) equals the
    /// direct mod-`q` MLE evaluation — no cryptography, just the chunking algebra.
    #[test]
    fn mod_q_recombine_matches_reference() {
        let p = IntEvalParams { t: 3, s: 2, word_bits: 16 };
        let data = mod_q_data(&p, 11);
        let r1 = fq_point(4, p.t);
        let r2 = fq_point(5, p.s);
        let rw_q = row_weights_q_from(&r1);
        let cw = col_weights_from(&r2);
        let c_w = mod_q_chunk_width(&p);
        let l = mod_q_num_chunks(&p, FQ_BITS);
        let w_chunks = chunk_row_weights(&rw_q, c_w, l);
        let mut v = vec![0u128; l << p.s];
        for (li, wc) in w_chunks.iter().enumerate() {
            let vl = fold_values(&p, &data, wc);
            for c in 0..p.cols() {
                v[(li << p.s) | c] = vl[c];
            }
        }
        let got = recombine_read_off(&p, &v, 0, &cw, c_w, l);
        let want = mle_eval_mod_q_ref(&p, &data, &r1, &r2);
        assert_eq!(got, want, "recombine read-off must equal the direct mod-q MLE eval");
    }

    /// Single-poly roundtrip vs. the M0 reference + wrong-`y` rejection.
    fn run_mod_q_roundtrip(p: &IntEvalParams, exp_l: usize, seed: u128) {
        let data = mod_q_data(p, seed);
        let r1 = fq_point(seed.wrapping_add(1), p.t);
        let r2 = fq_point(seed.wrapping_add(2), p.s);
        let rw_q = row_weights_q_from(&r1);
        let cw = col_weights_from(&r2);
        let y = mle_eval_mod_q_ref(p, &data, &r1, &r2);
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(p);
        let hint = commit_bits(&code, p, &data);

        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q(&mut pt, &hint, p, &rw_q, FQ_BITS, alpha, num_openings);
        assert_eq!(mod_q_num_chunks(p, FQ_BITS), exp_l, "expected L");
        assert_eq!(proof.v.len(), exp_l << p.s, "L·2^s chunk-folds");
        assert_eq!(proof.messages.len(), exp_l, "L combined messages");

        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, p, &rw_q, &cw, FQ_BITS, alpha, y, num_openings)
            .expect("mod-q roundtrip verifies against the M0 reference");

        // Wrong y → ReadOff.
        let mut vt2 = Blake3Transcript::new();
        let err = verify_mle_eval_mod_q(
            &mut vt2, &code, &hint.root, &proof, p, &rw_q, &cw, FQ_BITS, alpha,
            y + Fq::from(1u128), num_openings,
        )
        .expect_err("wrong y must be rejected");
        assert_eq!(err, IntEvalError::ReadOff);
    }

    /// L = 1 (no chunking): `(t,s,W) = (2,2,1)`, a 100-bit prime fits one exponent.
    #[test]
    fn mod_q_roundtrip_l1() {
        run_mod_q_roundtrip(&IntEvalParams { t: 2, s: 2, word_bits: 1 }, 1, 100);
    }

    /// Forced chunking L = 2: `(t,s,W) = (4,4,32)` (word witness ⇒ `c_w < q_bits`).
    #[test]
    fn mod_q_roundtrip_forced_chunk() {
        run_mod_q_roundtrip(&IntEvalParams { t: 4, s: 4, word_bits: 32 }, 2, 200);
    }

    /// Mid-size L = 2: `(t,s,W) = (6,6,32)`, n = 12.
    #[test]
    fn mod_q_roundtrip_midsize() {
        run_mod_q_roundtrip(&IntEvalParams { t: 6, s: 6, word_bits: 32 }, 2, 300);
    }

    /// A tampered chunk-fold is rejected two ways: in-range-but-wrong by the
    /// `α^{u}=root` binding, out-of-range by the free magnitude (range) check.
    #[test]
    fn mod_q_rejects_tampered_chunk() {
        // L = 1, small W ⇒ honest u_c < 2^{100+t+W} ≪ 2^127, so +1 stays in range.
        let p = IntEvalParams { t: 2, s: 2, word_bits: 1 };
        let data = mod_q_data(&p, 5);
        let r1 = fq_point(1, p.t);
        let r2 = fq_point(2, p.s);
        let rw_q = row_weights_q_from(&r1);
        let cw = col_weights_from(&r2);
        let y = mle_eval_mod_q_ref(&p, &data, &r1, &r2);
        let alpha = smallest_generator();
        let num_openings = 24;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);

        // (a) in-range but wrong → RootBinding.
        let mut pt = Blake3Transcript::new();
        let mut proof = prove_mle_eval_mod_q(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, num_openings);
        proof.v[0] = proof.v[0].wrapping_add(1);
        let mut vt = Blake3Transcript::new();
        let err = verify_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, &p, &rw_q, &cw, FQ_BITS, alpha, y, num_openings)
            .expect_err("in-range wrong chunk-fold must be rejected");
        assert_eq!(err, IntEvalError::RootBinding { c: 0 });

        // (b) out of range (≥ 2^{c_w+t+W} = 2^127) → ChunkRange.
        let mut pt2 = Blake3Transcript::new();
        let mut proof2 = prove_mle_eval_mod_q(&mut pt2, &hint, &p, &rw_q, FQ_BITS, alpha, num_openings);
        proof2.v[0] = 1u128 << 127;
        let mut vt2 = Blake3Transcript::new();
        let err2 = verify_mle_eval_mod_q(&mut vt2, &code, &hint.root, &proof2, &p, &rw_q, &cw, FQ_BITS, alpha, y, num_openings)
            .expect_err("out-of-range chunk-fold must be rejected");
        assert_eq!(err2, IntEvalError::ChunkRange { k: 0 });
    }

    /// A non-generator `α` is rejected (the chunk-fold binding would not be injective).
    #[test]
    fn mod_q_rejects_non_generator() {
        let p = IntEvalParams { t: 3, s: 3, word_bits: 16 };
        let data = mod_q_data(&p, 7);
        let r1 = fq_point(1, p.t);
        let r2 = fq_point(2, p.s);
        let rw_q = row_weights_q_from(&r1);
        let cw = col_weights_from(&r2);
        let y = mle_eval_mod_q_ref(&p, &data, &r1, &r2);
        let alpha = smallest_generator();
        let num_openings = 24;
        let code = code_for(&p);
        let hint = commit_bits(&code, &p, &data);

        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, num_openings);
        let mut vt = Blake3Transcript::new();
        let err = verify_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, &p, &rw_q, &cw, FQ_BITS, Gf::one(), y, num_openings)
            .expect_err("non-generator α must be rejected");
        assert_eq!(err, IntEvalError::ChallengeNotGenerator);
    }

    /// Batched mod-`q`: `L_polys = 3` distinct polynomials, each opened at its OWN
    /// 𝔽_q point, committed together; roundtrip vs. the M0 reference + wrong-claim.
    #[test]
    fn mod_q_batch_roundtrip() {
        let p = IntEvalParams { t: 4, s: 4, word_bits: 32 }; // L = 2
        let l_polys = 3usize;
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);

        let data: Vec<Vec<u128>> =
            (0..l_polys).map(|ell| mod_q_data(&p, (ell as u128).wrapping_mul(101).wrapping_add(1))).collect();
        let r1s: Vec<Vec<Fq>> = (0..l_polys).map(|ell| fq_point((ell as u128).wrapping_add(7), p.t)).collect();
        let r2s: Vec<Vec<Fq>> = (0..l_polys).map(|ell| fq_point((ell as u128).wrapping_add(70), p.s)).collect();
        let rw_q: Vec<Vec<u128>> = r1s.iter().map(|r| row_weights_q_from(r)).collect();
        let cw: Vec<Vec<Fq>> = r2s.iter().map(|r| col_weights_from(r)).collect();
        let claimed: Vec<Fq> =
            (0..l_polys).map(|ell| mle_eval_mod_q_ref(&p, &data[ell], &r1s[ell], &r2s[ell])).collect();

        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let proof = prove_batch_mle_eval_mod_q(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, num_openings);
        assert_eq!(mod_q_num_chunks(&p, FQ_BITS), 2);
        assert_eq!(proof.v.len(), l_polys * 2 * p.cols(), "L_polys·L·2^s chunk-folds");
        assert_eq!(proof.messages.len(), l_polys * 2, "L_polys·L messages");

        let mut vt = Blake3Transcript::new();
        verify_batch_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, &p, &rw_q, &cw, FQ_BITS, alpha, &claimed, num_openings)
            .expect("batched mod-q roundtrip verifies");

        // A wrong per-poly claim is caught by that poly's read-off.
        let mut bad = claimed.clone();
        bad[1] = bad[1] + Fq::from(1u128);
        let mut vt2 = Blake3Transcript::new();
        let err = verify_batch_mle_eval_mod_q(&mut vt2, &code, &hint.root, &proof, &p, &rw_q, &cw, FQ_BITS, alpha, &bad, num_openings)
            .expect_err("wrong per-poly claim must be rejected");
        assert_eq!(err, IntEvalError::ReadOff);
    }

    /// Batched mod-`q`: a tampered committed bit breaks the shared Merkle path.
    #[test]
    fn mod_q_batch_rejects_tampered_column() {
        let p = IntEvalParams { t: 4, s: 4, word_bits: 32 };
        let l_polys = 2usize;
        let alpha = smallest_generator();
        let num_openings = 32;
        let code = code_for(&p);

        let data: Vec<Vec<u128>> =
            (0..l_polys).map(|ell| mod_q_data(&p, (ell as u128).wrapping_mul(13).wrapping_add(1))).collect();
        let r1s: Vec<Vec<Fq>> = (0..l_polys).map(|ell| fq_point((ell as u128).wrapping_add(3), p.t)).collect();
        let r2s: Vec<Vec<Fq>> = (0..l_polys).map(|ell| fq_point((ell as u128).wrapping_add(33), p.s)).collect();
        let rw_q: Vec<Vec<u128>> = r1s.iter().map(|r| row_weights_q_from(r)).collect();
        let cw: Vec<Vec<Fq>> = r2s.iter().map(|r| col_weights_from(r)).collect();
        let claimed: Vec<Fq> =
            (0..l_polys).map(|ell| mle_eval_mod_q_ref(&p, &data[ell], &r1s[ell], &r2s[ell])).collect();

        let hint = commit_batch(&code, &p, &data);
        let mut pt = Blake3Transcript::new();
        let mut proof = prove_batch_mle_eval_mod_q(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, num_openings);

        let cell = &mut proof.opened[0].col_packed[0];
        *cell = BinaryPoly::<64>::unpack_u64(cell.pack_u64() ^ 1);
        let mut vt = Blake3Transcript::new();
        let err = verify_batch_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, &p, &rw_q, &cw, FQ_BITS, alpha, &claimed, num_openings)
            .expect_err("tampered column must be rejected");
        assert!(matches!(err, IntEvalError::Open(IntEvalOpenError::Merkle { .. })), "got {err:?}");
    }

    /// Mod-`q` scaling print: `L`, message size, proof size, prove/verify ms vs. `n`
    /// (W=1 at the proof-size-optimal `t`, so the chunked fold reaches large `t`
    /// regardless of weight width), plus one forced-`L=2` (W=32) row. Confirms
    /// `L = ⌈q_bits / c_w⌉`. Run:
    /// `cargo test … --release -- --ignored --nocapture mod_q_scaling_sweep`.
    #[test]
    #[ignore = "manual: mod-q scaling over n (confirms L = ⌈q_bits / c_w⌉)"]
    fn mod_q_scaling_sweep() {
        use std::time::Instant;
        let alpha = smallest_generator();
        let num_openings = 987;
        let run = |p: &IntEvalParams, seed: u128| {
            let c_w = mod_q_chunk_width(p);
            let l = mod_q_num_chunks(p, FQ_BITS);
            assert_eq!(l, FQ_BITS.div_ceil(c_w), "L = ⌈q_bits / c_w⌉");
            let data = mod_q_data(p, seed);
            let r1 = fq_point(1, p.t);
            let r2 = fq_point(2, p.s);
            let rw_q = row_weights_q_from(&r1);
            let cw = col_weights_from(&r2);
            let y = mle_eval_mod_q_ref(p, &data, &r1, &r2);
            let code = code_for(p);
            let hint = commit_bits(&code, p, &data);
            let mut pt = Blake3Transcript::new();
            let t1 = Instant::now();
            let proof = prove_mle_eval_mod_q(&mut pt, &hint, p, &rw_q, FQ_BITS, alpha, num_openings);
            let dp = t1.elapsed().as_secs_f64() * 1000.0;
            let mut vt = Blake3Transcript::new();
            let t2 = Instant::now();
            verify_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, p, &rw_q, &cw, FQ_BITS, alpha, y, num_openings)
                .expect("mod-q scaling verify");
            let dv = t2.elapsed().as_secs_f64() * 1000.0;
            let msg_gf: usize = proof.messages.iter().map(Vec::len).sum();
            let proof_kb = mle_eval_mod_q_proof_size_bytes(&proof) as f64 / 1024.0;
            eprintln!(
                "{:>3} {:>3} {:>3} {:>5} {:>3} {:>7} {:>9.1} {:>9.2} {:>10.2}",
                p.t + p.s, p.t, p.s, c_w, l, msg_gf, proof_kb, dp, dv,
            );
        };

        eprintln!("mod-q  q = 2^100 − 15  (q_bits = {FQ_BITS})");
        eprintln!("  n  t*  s   c_w   L   msgGf  proofKB   prove_ms  verify_ms");
        for n in [12usize, 16, 20] {
            let t = proof_size_optimal_t(n, 1, 1, num_openings);
            run(&IntEvalParams { t, s: n - t, word_bits: 1 }, 9);
        }
        // Forced L = 2 (word witness W = 32).
        run(&IntEvalParams { t: 4, s: 8, word_bits: 32 }, 9);
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
            for w in c.words() {
                buf.extend_from_slice(&w.to_le_bytes());
            }
        }
    }
    transcript.absorb_slice(&buf);
}
