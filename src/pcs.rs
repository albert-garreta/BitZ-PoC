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

use crate::poly::mle::DenseMultilinearExtension;
use crate::poly::univariate::binary::BinaryPoly;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;
use crate::transcript::traits::Transcript;

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

/// The shared row-bit weight vector `q_rowbit` over the `(b, j)` leaf index, at the
/// forest's shared reduction point `ρ`:
/// `q_rowbit[(b<<log₂W)|j] = eq((b,j),ρ) · (α^{w_b·2^j} − 1)`.
///
/// It is the same for every column, so the per-column leaf claims
/// `ℓ_c − 1 = ⟨q_rowbit, bits_c⟩` reduce, via the de-black-boxing pre-sumcheck,
/// to a residual bit-MLE evaluation the ring-switch + Ligerito opener discharges.
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
        .map(Gf::from)
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
// share the SAME modulus `q = 2^100 − 15`; convert `F ↔ Fq` at the boundary via
// the canonical integer ([`ProjectCanonicalU128`]).

/// The fixed ~100-bit read-off prime `q = 2^100 − 15`. Must equal the host
/// prover's projection prime.
pub const FQ_MOD: u128 = (1u128 << 100).wrapping_sub(15);
/// `⌈log₂ q⌉` — the chunk-count input for the mod-`q` opening (`L = 1` for SHA).
pub const FQ_BITS: usize = 100;

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

// =====================================================================
// Mod-q RLC claim families (EXPERIMENTAL — docs/rlc-family-note-prompt.md).
//
// k claims `MLE[INT(a_i)](r_i) = c_i ∈ 𝔽_q`, each `a_i` a public F₂-linear
// form `L_i` of j committed bit-columns (`a_i[pos] = L_i(m_1[pos], …,
// m_j[pos])`, `L_i(m) = ⊕_{i'∈form_i} m_{i'}`), all claims sharing the
// COLUMN point. After the γ-RLC (γ's drawn post-statement), the combined
// row weight at position b depends only on the CASE `m ∈ {0,1}^j` of the j
// bits: `W_b(m) = (Σ_i γ_i·w_{i,b}·L_i(m)) mod q` — 2^j reduced values per
// row instead of k weight functions. ONE forest per chunk binds
// `α^{W_b^{(l)}(m(pos))}` with a 2^j-case leaf select; the presum splits
// into 2^j − 1 channels `R_S = eq ⊙ τ_S` against the bit monomials
// `Π_{i∈S} m_i` (τ_S the char-2 subset zeta-transform of the case
// α-powers); |S| ≥ 2 channels discharge through one η-batched
// degree-(j+1) eq-sumcheck. The 𝔽_q here is the crate's fixed
// `q = 2^100 − 15` ([`FQ_MOD`]) — the γ arithmetic is protocol-internal,
// so this family is NOT generic over the evaluation ring.

/// `2^128 mod q` for `q = ` [`FQ_MOD`]: `2^128 = 2^28·(q + 15) ≡ 15·2^28`.
const FQ_R128: u128 = 15u128 << 28;

/// One uniform-enough `𝔽_q` challenge: two 128-bit transcript draws
/// combined as a 256-bit integer reduced mod `q` (statistical distance
/// ≤ `q/2^256` ≈ 2^-156 from uniform — the plain 128-bit draw would be
/// ~2^-28-biased at the 100-bit `q`).
pub fn fq_challenge(transcript: &mut impl Transcript) -> u128 {
    let to_u128 = |g: Gf| -> u128 {
        let w = g.words();
        u128::from(w[0]) | (u128::from(w[1]) << 64)
    };
    let lo: Gf = transcript.get_field_challenge(&());
    let hi: Gf = transcript.get_field_challenge(&());
    fq_add(to_u128(lo) % FQ_MOD, fq_mul(to_u128(hi), FQ_R128))
}

/// The RLC case-weight table: per row position `b` and case `m ∈ {0,1}^j`,
/// `W_b(m) = (Σ_i γ_i·w_{i,b}·L_i(m)) mod q ∈ [0, q)` with
/// `L_i(m) = parity(form_i & m)`. Returns `[2^{t'}][2^j]`. `W_b(0) = 0`
/// always (the forms are linear), so case 0's α-power is 1 — the padded /
/// no-bits-set leaf.
#[allow(clippy::arithmetic_side_effects)] // bounded case/claim loops; fq_* reduce
pub fn rlc_case_weights(
    claim_weights: &[&[u128]],
    gammas: &[u128],
    forms: &[usize],
    j: usize,
) -> Vec<Vec<u128>> {
    let k = claim_weights.len();
    // j ≤ 4 for the committed-column family (kernel budget); up to 7 for
    // the structured-tap STREAM families (eager forest, clustered).
    assert!((1..=7).contains(&j), "RLC case tables support j ∈ [1, 7]");
    assert_eq!(gammas.len(), k, "one γ per claim");
    assert_eq!(forms.len(), k, "one form per claim");
    let cases = 1usize << j;
    let rows = claim_weights.first().map_or(0, |w| w.len());
    for w in claim_weights {
        assert_eq!(w.len(), rows, "claim row-weight lengths must agree");
    }
    for &f in forms {
        assert!(f != 0 && f < cases, "forms must be nonzero bitmasks over [j]");
    }
    cfg_into_iter!(0..rows)
        .map(|b| {
            let d: Vec<u128> =
                (0..k).map(|i| fq_mul(gammas[i], claim_weights[i][b])).collect();
            (0..cases)
                .map(|m| {
                    let mut acc = 0u128;
                    for (i, &di) in d.iter().enumerate() {
                        if (forms[i] & m).count_ones() & 1 == 1 {
                            acc = fq_add(acc, di);
                        }
                    }
                    acc
                })
                .collect()
        })
        .collect()
}

/// The SHARED-POINT case coefficients `Γ(m) = (Σ_i γ_i·L_i(m)) mod q`,
/// `m ∈ {0,1}^j` — when every claim sits at ONE row point the case-weight
/// table is rank-1, `W_b(m) = (w_b·Γ(m)) mod q`, and these `2^j` values
/// are its whole case content. `Γ(0) = 0` (the forms are linear).
#[allow(clippy::arithmetic_side_effects)] // bounded case/claim loops; fq_* reduce
pub fn rlc_gamma_cases(gammas: &[u128], forms: &[usize], j: usize) -> Vec<u128> {
    assert!((1..=4).contains(&j), "RLC family supports j ∈ [1, 4] (2^j-case tables)");
    assert_eq!(gammas.len(), forms.len(), "one γ per claim");
    let cases = 1usize << j;
    for &f in forms {
        assert!(f != 0 && f < cases, "forms must be nonzero bitmasks over [j]");
    }
    (0..cases)
        .map(|m| {
            forms
                .iter()
                .zip(gammas.iter())
                .filter(|&(&f, _)| (f & m).count_ones() & 1 == 1)
                .fold(0u128, |acc, (_, &g)| fq_add(acc, g))
        })
        .collect()
}

/// The rank-1 shared-point case-weight table `W[b][m] = (w_b·Γ(m)) mod q`
/// from ONE row-weight vector and the [`rlc_gamma_cases`] coefficients:
/// one `fq_mul` per (row, case) — the collapse of [`rlc_case_weights`]
/// when all `k` claims share the row point. Pinned equal to the general
/// build by a test below.
#[allow(clippy::arithmetic_side_effects)] // bounded case loop; fq_mul reduces
pub fn rlc_case_weights_shared_point(
    row_weights_q: &[u128],
    gamma_cases: &[u128],
) -> Vec<Vec<u128>> {
    cfg_iter!(row_weights_q)
        .map(|&w| gamma_cases.iter().map(|&g| fq_mul(w, g)).collect())
        .collect()
}

/// Chunk the case-weight table into base-`2^{c_w}` limbs (the
/// [`chunk_row_weights`] analogue): `out[l][b][m] = (W_b(m) ≫ c_w·l) &
/// (2^{c_w}−1)`.
#[allow(clippy::arithmetic_side_effects)] // shift bounded by l_chunks·c_w < 128
pub fn rlc_chunk_case_weights(
    case_w: &[Vec<u128>],
    c_w: usize,
    l_chunks: usize,
) -> Vec<Vec<Vec<u128>>> {
    let limb_mask = (1u128 << c_w).wrapping_sub(1);
    (0..l_chunks)
        .map(|l| {
            let shift = c_w * l;
            case_w
                .iter()
                .map(|row| row.iter().map(|&w| (w >> shift) & limb_mask).collect())
                .collect()
        })
        .collect()
}

/// One chunk's case α-power table (the [`chunk_pow2_table`] analogue for
/// 2^j-case leaves, W = 1): `out[b][m] = α^{W_b^{(l)}(m)}` via the shared
/// fixed-base comb. Case 0 is `α^0 = 1` by construction.
pub(crate) fn rlc_case_pow_table(w_cases: &[Vec<u128>], alpha: Gf) -> Vec<Vec<Gf>> {
    let comb = FixedBasePow::new(alpha, 128, 8);
    cfg_iter!(w_cases).map(|row| row.iter().map(|&w| comb.pow(w)).collect()).collect()
}

/// The presum channel tables `τ_S` from one chunk's case α-powers: the
/// char-2 subset zeta-transform `τ_S = Σ_{T⊆S} α^{W(1_T)}`, returned as
/// `[2^j][2^{t'}]` (slot `S = 0` is the ∅-transform, identically 1 — not a
/// channel). The 2^j-case leaf is multilinear in the j committed bits:
/// `leaf(m) = 1 + Σ_{∅≠S} τ_S·Π_{i∈S} m_i` (pinned by a test below).
#[allow(clippy::arithmetic_side_effects)] // bounded 2^j-case loops
pub(crate) fn rlc_tau_tables(case_pow: &[Vec<Gf>]) -> Vec<Vec<Gf>> {
    let cases = case_pow.first().map_or(1, |r| r.len());
    let j = cases.trailing_zeros() as usize;
    let rows = case_pow.len();
    let mut out: Vec<Vec<Gf>> =
        (0..cases).map(|s| (0..rows).map(|b| case_pow[b][s]).collect()).collect();
    for d in 0..j {
        for s in 0..cases {
            if (s >> d) & 1 == 1 {
                let src = out[s ^ (1usize << d)].clone();
                for (o, x) in out[s].iter_mut().zip(src) {
                    *o += x;
                }
            }
        }
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
mod rlc_tests {
    use super::*;

    /// The 2^j-case leaf select is multilinear in the j bits:
    /// `α^{W(m)} = 1 + Σ_{∅≠S⊆[j]} τ_S · Π_{i∈S} m_i` for every case `m`,
    /// with `τ_S` the subset zeta-transform of the case α-powers — the
    /// identity the RLC-family forest leaves and presum channels rely on.
    #[test]
    fn rlc_tau_leaf_multilinearity() {
        let alpha = smallest_generator();
        for j in 1usize..=4 {
            let cases = 1usize << j;
            let rows = 8usize;
            // Synthetic case weights with W(0) = 0 (linear forms).
            let case_w: Vec<Vec<u128>> = (0..rows)
                .map(|b| {
                    (0..cases)
                        .map(|m| {
                            if m == 0 {
                                0
                            } else {
                                ((b as u128 + 3) * (m as u128 + 11) * 0x9E37_79B9) % FQ_MOD
                            }
                        })
                        .collect()
                })
                .collect();
            let pow = rlc_case_pow_table(&case_w, alpha);
            let tau = rlc_tau_tables(&pow);
            for b in 0..rows {
                for m in 0..cases {
                    let mut acc = Gf::one();
                    for (s, tau_s) in tau.iter().enumerate().skip(1) {
                        // Π_{i∈S} m_i = 1 iff S ⊆ m (as bitmasks).
                        if s & m == s {
                            acc += tau_s[b];
                        }
                    }
                    assert_eq!(acc, pow[b][m], "leaf identity at b={b}, m={m:#b}, j={j}");
                }
            }
        }
    }

    /// `rlc_case_weights` matches the direct definition
    /// `W_b(m) = Σ_i γ_i·w_{i,b}·parity(form_i & m) mod q`, and case 0 is 0.
    #[test]
    fn rlc_case_weights_match_direct() {
        let j = 2usize;
        let k = 3usize;
        let rows = 16usize;
        let forms = [0b01usize, 0b10, 0b11];
        let weights: Vec<Vec<u128>> = (0..k)
            .map(|i| {
                (0..rows)
                    .map(|b| ((i as u128 + 2) * (b as u128 + 7) * 0xDEAD_BEEF_CAFE) % FQ_MOD)
                    .collect()
            })
            .collect();
        let gammas: Vec<u128> = (0..k).map(|i| (i as u128 + 1) * 0x1234_5678_9ABC % FQ_MOD).collect();
        let w_refs: Vec<&[u128]> = weights.iter().map(|w| &w[..]).collect();
        let cw = rlc_case_weights(&w_refs, &gammas, &forms, j);
        for b in 0..rows {
            for m in 0..(1usize << j) {
                let mut want = 0u128;
                for i in 0..k {
                    if (forms[i] & m).count_ones() & 1 == 1 {
                        want = fq_add(want, fq_mul(gammas[i], weights[i][b]));
                    }
                }
                assert_eq!(cw[b][m], want, "case weight at b={b}, m={m:#b}");
            }
            assert_eq!(cw[b][0], 0, "linear forms vanish at the zero case");
        }
        // Chunking recomposes.
        let c_w = 40usize;
        let lch = 3usize;
        let chunks = rlc_chunk_case_weights(&cw, c_w, lch);
        for b in 0..rows {
            for m in 0..(1usize << j) {
                let mut acc = 0u128;
                for (l, ch) in chunks.iter().enumerate() {
                    acc += ch[b][m] << (c_w * l);
                }
                assert_eq!(acc, cw[b][m], "chunk recomposition at b={b}, m={m:#b}");
            }
        }
    }

    /// The rank-1 shared-point build (`rlc_case_weights_shared_point` over
    /// `Γ = rlc_gamma_cases`) equals the general `rlc_case_weights` when
    /// every claim carries the SAME row-weight vector — for the maximal
    /// families at j = 2, 3, 4 (every nonzero form once) and a duplicated-
    /// form variant.
    #[test]
    fn rlc_shared_point_case_weights_match_general() {
        let rows = 32usize;
        let w: Vec<u128> = (0..rows)
            .map(|b| ((b as u128 + 3) * 0xFEED_FACE_CAFE_BEEF) % FQ_MOD)
            .collect();
        for j in 1..=4usize {
            let forms: Vec<usize> = (1..1usize << j).collect(); // maximal family
            let k = forms.len();
            let gammas: Vec<u128> =
                (0..k).map(|i| ((i as u128 + 5) * 0x0123_4567_89AB_CDEF) % FQ_MOD).collect();
            let w_refs: Vec<&[u128]> = (0..k).map(|_| &w[..]).collect();
            let general = rlc_case_weights(&w_refs, &gammas, &forms, j);
            let gcases = rlc_gamma_cases(&gammas, &forms, j);
            assert_eq!(gcases[0], 0, "Γ(0) = 0 for linear forms");
            let shared = rlc_case_weights_shared_point(&w, &gcases);
            assert_eq!(shared, general, "rank-1 build diverges at j={j} (maximal family)");
        }
        // A non-maximal family with a repeated γ-weighted form pattern.
        let forms = [0b01usize, 0b11, 0b11];
        let gammas: Vec<u128> = vec![7, 11, 13];
        let w_refs: Vec<&[u128]> = (0..3).map(|_| &w[..]).collect();
        let general = rlc_case_weights(&w_refs, &gammas, &forms, 2);
        let shared =
            rlc_case_weights_shared_point(&w, &rlc_gamma_cases(&gammas, &forms, 2));
        assert_eq!(shared, general, "rank-1 build diverges on repeated forms");
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
