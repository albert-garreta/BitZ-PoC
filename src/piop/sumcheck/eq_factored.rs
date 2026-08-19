//! **Eq-factored sumcheck prover** for combs of the shape
//! `Σ_x Σ_t eq(x; q_t) · Σ_i L_{t,i}(x)·R_{t,i}(x)` — an eq-weighted sum of
//! inner products of multilinear pairs, the shape shared by the GKR
//! product-tree layers (`one group, one pair: eq·L·R`) and the F_2
//! lookup-adder binding (`one group, one (mask, Q) pair per relation`).
//!
//! The eq factors are **never materialised as sumcheck multiplicands and
//! never folded**. Round `j` (fixing variable `j−1`, the LOW bit — the same
//! order as the generic prover) writes each group's contribution as
//!
//! ```text
//!   M_j(c) = Σ_t A_{t,j} · eq1(c; q_t[j−1]) · H_{t,j}(c)
//!   A_{t,j}    = Π_{i<j−1} eq1(ρ_{i+1}; q_t[i])          (prefix scalar)
//!   H_{t,j}(c) = Σ_b V_{t,j}[b] · Σ_i L_i(c,b)·R_i(c,b)  (degree 2 in c)
//!   V_{t,j}[b] = Π_{i≥j} eq1(b_{i−j}; q_t[i])            (suffix tensor)
//! ```
//!
//! Suffix tensors are precomputed back-to-front (total `O(2^k)` per group —
//! the cost of ONE eq build, no divisions: the generic field bounds have no
//! inverse) and read densely; only the `L`/`R` buffers fold. Each `H_t` is
//! quadratic, so its value at the fourth Lagrange node is free in
//! characteristic 2: the nodes `{0, 1, X, X+1}` (`F::from(0..=3)` under the
//! bit-pattern convention) form an affine 2-flat, over which every
//! polynomial of degree ≤ 2 sums to zero ⇒ `H(X+1) = H(0) + H(1) + H(X)`.
//! Char 2 is detected exactly at runtime (`1+1 == 0`); any other field
//! accumulates `H(c3)` in the same pass, keeping the driver field-generic.
//!
//! Two round-message formats share the machinery:
//!
//! - **Generic** (`gruen = false`): **byte-identical** to
//!   [`MLSumcheck::prove_as_subprotocol`] over the materialised
//!   `[eq_1, …, eq_T, all L's, all R's]` with the degree-3 comb
//!   `Σ_t eq_t·Σ_i L_i·R_i`: the same round polynomials evaluated at the same
//!   nodes, the same transcript ops (the `nvars`/`degree` header, the `P(1..)`
//!   tail absorb, the post-draw challenge re-absorb), the same proof layout —
//!   the generic [`MLSumcheck::verify_as_subprotocol`] verifies it unchanged.
//!   Required whenever groups carry DIFFERENT eq points (no common linear
//!   factor exists).
//! - **Gruen** (`gruen = true`, shared-`q` only): Gruen's degree reduction
//!   for a known linear factor (ePrint 2024/108). With every group at one
//!   point, `P_j(X) = eq1(X; q[j−1]) · Ĥ_j(X)` with `Ĥ_j = Σ_t A_t·H_t`
//!   quadratic, and the round message is `Ĥ_j`'s two non-constant monomial
//!   coefficients `(Ĥ1, Ĥ2)` — TWO field elements instead of three. The
//!   verifier ([`verify_eq_inner_sumcheck_gruen`]) reconstructs the constant
//!   one from the running claim via `S_j = P_j(0) + P_j(1) =
//!   Ĥ0 + q[j−1]·(Ĥ1 + Ĥ2)` (an identity over ANY field: the `(1−q)Ĥ0 +
//!   qĤ0` cross terms collapse) and chains `S_{j+1} = eq1(ρ_j; q[j−1]) ·
//!   Ĥ_j(ρ_j)`. Same header, same absorb order; only the tail length
//!   changes. The forest's merged-GKR layers (always shared-point) run this
//!   mode; the sent values differ from the Generic mode, so prover and
//!   verifier must agree on the mode per instance.

use crypto_primitives::FromPrimitiveWithConfig;
use num_traits::Zero;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use crate::transcript::traits::{ConstTranscribable, Transcript};
use crate::utils::{
    cfg_iter, inner_transparent_field::InnerTransparentField, wide_mul::WideMulAcc,
};

use super::prover::{NatEvaluatedPolyWithoutConstant, ProverMsg};
use super::verifier::Subclaim;
use super::{SumCheckError, SumcheckProof};

/// One eq-weighted group: contributes
/// `scale·eq(x; q)·Σ_i pairs[i].0(x)·pairs[i].1(x)` to the proven sum. All
/// groups (and all pair vectors) must share the same number of variables
/// `k = q.len()`, with `2^k`-length pair vectors.
pub struct EqInnerGroup<F> {
    /// The eq point of this group.
    pub q: Vec<F>,
    /// The `(L_i, R_i)` multilinear pairs (evaluation vectors, consumed —
    /// they become the fold buffers).
    pub pairs: Vec<(Vec<F>, Vec<F>)>,
    /// Public scalar multiplying the whole group (a batching challenge in
    /// the batched-forest GKR; `1` otherwise). Costs nothing: it seeds the
    /// group's prefix scalar.
    pub scale: F,
}

/// Buffer payload of one group in the mixed-entry driver.
pub enum GroupBufs<F> {
    /// Materialised `(L, R)` pair vectors — the general case.
    Dense(Vec<(Vec<F>, Vec<F>)>),
    /// **Bit-affine leaf layer** (char-2 forests, single pair): entry `i` of
    /// `L` is *defined* as `1 + lbits[i]·tau_l[i]` (and `R` from `rbits` /
    /// `tau_r`), with the `tau` coefficient arrays shared across groups via
    /// `tau_sets[tau_set]`. Round 1's message is then computed with **zero
    /// multiplications per group** — the suffix-weighted coefficient sums
    /// expand (char 2) into subset-sums of eight tree-shared `w·τ` product
    /// tables, selected by branchless bit masks — and round 1's fold
    /// materialises the `Dense` round-2 buffers from two more shared
    /// `ρ·τ`-style tables. The leaf values themselves are never built.
    /// Bits are packed 64 per `u64`, position `i` at word `i/64`, bit `i%64`.
    LeafBits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
    /// **Bit-selected product layer** (char-2 forests, single pair, one
    /// level ABOVE the leaves): entry `y` of `L` is *defined* as the 2-bit
    /// select `pair_tau_sets[tau_set].te[4y + (bit_lo | bit_hi≪1)]` with
    /// `bit_lo = lbits[y]`, `bit_hi = rbits[y]` — the four cases being
    /// `{1, τ_lo, τ_hi, τ_lo·τ_hi}` of a leaf-pair product — and entry `y`
    /// of `R` the same select at position `y + 2^k` (`.to`, whose position
    /// offset is baked into the table). The bit arrays span `2^{k+1}`
    /// positions — the SAME per-tree leaf bit halves the leaf layer's
    /// [`GroupBufs::LeafBits`] consumes. Round 1's message is three
    /// case-LUT loads + ONE wide multiply per slot (the ΔL·ΔR cross term);
    /// round 1's fold materialises the `Dense` round-2 buffers from two
    /// shared 16-case fold tables. The product-layer values are never
    /// built — combined with generating the NEXT level down from bits,
    /// the whole level `d−1` is skipped.
    Pair2Bits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
    /// **Bit-affine leaf layer, TWO bit-driven rounds** (char-2 forests,
    /// single pair, `k ≥ 3`): the same implicit leaves as
    /// [`GroupBufs::LeafBits`] — entry `i` of `L` is `1 + lbits[i]·tau_l[i]`
    /// — but the dense buffers materialise only after round TWO, halving
    /// the leaf-round working set to `2^{k−2}` entries per side. Round 1 is
    /// the [`LeafTables`] case-LUT round unchanged. Round 1's "fold" keeps
    /// the bits: the per-position 4-case fold tables
    /// (`1 + m_0·(1+ρ₁)τ_0 + m_1·ρ₁τ_1`, exactly [`LeafFoldTables`]) *are*
    /// the round-2 value tables, and their layout is a [`Pair2TauSet`]
    /// (`te = t_l`, `to = t_r`) — so round 2 runs the
    /// [`Pair2Bits`](GroupBufs::Pair2Bits) case-LUT round over them via
    /// [`build_pair2_tables`] (cases = adjacent original-bit pairs, one
    /// aligned 4-bit nibble load per side per slot), and round 2's fold
    /// materialises the `Dense` round-3 buffers from the 16-case
    /// [`Pair2FoldTables`]. All tables are per-position and shared across
    /// the groups of a set (`O(2^k)` per set, amortised over the trees);
    /// every step is an exact char-2 identity, byte-identical to `Dense`.
    Leaf2Bits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
    /// **Bit-affine leaf layer, THREE bit-driven rounds** (`k ≥ 4`):
    /// [`Leaf2Bits`](GroupBufs::Leaf2Bits) one round deeper — rounds 1 and
    /// 2 run its bodies unchanged; round 2's fold keeps the bits and
    /// stashes its [`Pair2FoldTables`] as the round-3 VALUE tables (entry
    /// `p` of the round-3 buffer is `f[(p≪4) | (c0≪2) | c1]` with
    /// `c0, c1` = the adjacent original-bit pairs `4p..4p+2, 4p+2..4p+4`);
    /// round 3's message and fold then read values INLINE (one aligned
    /// byte load per side per slot — no precombined slot tables: the
    /// measured lesson is that multiplies are free, only skipped bytes
    /// pay). Dense buffers materialise only at round 3's fold
    /// (`2^{k−3}`/side — the leaf-round set drops to L/8).
    Leaf3Bits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
    /// **Bit-selected product layer, TWO bit-driven rounds** (`k ≥ 3`):
    /// [`Pair2Bits`](GroupBufs::Pair2Bits) one round deeper — round 1 runs
    /// its case-LUT body unchanged; round 1's fold keeps the bits and
    /// stashes its [`Pair2FoldTables`] as the round-2 value tables (entry
    /// `p` keyed by `(lbits[2p], rbits[2p], lbits[2p+1], rbits[2p+1])`,
    /// O-side at bit offset `2^k`); round 2 reads values inline and its
    /// fold materialises `Dense` (`2^{k−2}`/side).
    Pair3Bits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
    /// **Bit-selected 4-leaf-product layer** (`k ≥ 2`, one level ABOVE
    /// [`Pair2Bits`](GroupBufs::Pair2Bits)): entry `y` of `L`/`R` is the
    /// 16-case select `t4_sets[tau_set][(j≪4) | (cE≪2) | cO]` at position
    /// `j = y` / `j = y + 2^k`, with `cE = lbits.bit(j) | rbits.bit(j)≪1`,
    /// `cO = lbits.bit(j+2^{k+1}) | rbits.bit(j+2^{k+1})≪1` — the same
    /// per-set `T4` table and transposed leaf-bit halves the lazy build
    /// consumes (the arrays span `2^{k+2}` positions each). Round 1 reads
    /// values inline; its fold materialises `Dense` (`2^{k−1}`/side) —
    /// the layer's stored/JIT input level is never needed.
    T4Bits {
        lbits: Vec<u64>,
        rbits: Vec<u64>,
        tau_set: usize,
    },
}

/// Per-position 4-case VALUE tables of one bit-selected product layer
/// (shared across the groups of a set): `te[4y + c]` = the `L`-side value
/// of case `c` at position `y` (`c = bit_lo | bit_hi≪1`, case 0 = `1`),
/// `to[4y + c]` = the `R`-side value (position `y + 2^k` baked in). Both
/// have `4·2^k` entries for a `k`-variable group.
pub struct Pair2TauSet<F> {
    pub te: Vec<F>,
    pub to: Vec<F>,
}

/// Round-1 work the CALLER already did, fused into its buffer generation
/// pass (the forest's JIT layers): either round 1's coefficient triple, or
/// — under the double-fold — the whole bivariate grid for rounds 1 AND 2,
/// which spends both without the driver ever reading the buffers. The grid
/// form needs `k ≥ 3` (round 2 must not be the last round, so the final
/// round still runs a pass that consumes the deferred folds).
pub enum PreRound<F> {
    Coeffs(Vec<(F, F, F)>),
    Grid(Vec<[F; 9]>),
}

impl<F> PreRound<F> {
    fn len(&self) -> usize {
        match self {
            PreRound::Coeffs(v) => v.len(),
            PreRound::Grid(v) => v.len(),
        }
    }
}

/// One group of the mixed-entry driver ([`prove_eq_inner_sumcheck_mixed`]).
pub struct EqInnerGroupMixed<F> {
    pub q: Vec<F>,
    pub scale: F,
    pub bufs: GroupBufs<F>,
}

/// Round-1 message tables for one leaf `tau` set, in **case-LUT** form:
/// the char-2 expansion of each accumulator's per-slot contribution is
/// precombined over the selecting bits, so a group does ONE unconditional
/// table add per accumulator per slot (3 loads + 3 adds — no per-term
/// masking, ~⅓ the table traffic):
///
/// - `t_a0[(b≪2) | (m_{L0} | m_{R0}≪1)]` = the `Σ w·l0·r0` slot term
///   `m_{L0}·wτ_{L0} + m_{R0}·wτ_{R0} + m_{L0}m_{R0}·wτ_{L0}τ_{R0}`;
/// - `t_a1[(b≪2) | (m_{L1} | m_{R1}≪1)]` = the odd (`Σ w·l1·r1`) variant;
/// - `t_a2[(b≪4) | (lp | rp≪2)]` = the `Σ w·ΔL·ΔR` combo of the four
///   cross products `wτ_{Lε}τ_{Rδ}` selected by `m_{Lε}∧m_{Rδ}`.
///
/// Shared by every group of the set: 8 multiplies + ~20 adds per slot,
/// built once.
struct LeafTables<F> {
    t_a0: Vec<F>,
    t_a1: Vec<F>,
    a2: LeafA2<F>,
    w_sum: F,
}

/// The `Σ w·ΔL·ΔR` term's table form.
enum LeafA2<F> {
    /// 16-case precombined ΔΔ table `t_a2[(b≪4) | lp | rp≪2]`: one load +
    /// one add per slot, `16·2^k` entries — 2/3 of the leaf-round table
    /// bytes. Default below the size threshold (see [`leaf_a2_factored`]).
    Precombined(Vec<F>),
    /// The four raw cross products per slot `[p00, p10, p01, p11]` at
    /// `b≪2` — 4× less ΔΔ-table footprint (the leaf-round tables drop
    /// from `24·2^k` to `12·2^k` entries), one sequential 64 B line per
    /// slot, four branchless masked adds in place of the load. Default at
    /// and above the size threshold.
    Factored(Vec<F>),
}

/// ΔΔ-table form choice: `F2Z_LEAF_A2_FACTORED=0/1` forces
/// precombined/factored; unset (the default) picks by footprint —
/// factored iff `half ≥ 2^15`, i.e. once the precombined leaf tables
/// (`384·half` bytes) reach ~12.6 MB and stop co-residing in the
/// P-cluster L2. Measured (fresh box, 3 alternated in-window pairs per
/// shape): n=26 (6.3 MB) factored LOSES ~1.2 ms (an L2-hit gather beats
/// 4 masked selects); n=28 (12.6 MB) factored wins leaf_r1 −13–18 %;
/// n=30 (25 MB) factored wins leaf_r1 2.1× (307→147 ms), prove −6.5 %,
/// and is far less run-to-run volatile. Byte-identical proofs either
/// way. Env read once per process.
fn leaf_a2_factored(half: usize) -> bool {
    static ENV: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    let env = *ENV.get_or_init(|| match std::env::var("F2Z_LEAF_A2_FACTORED") {
        Ok(v) if v == "0" => Some(false),
        Ok(v) if v == "1" => Some(true),
        _ => None,
    });
    env.unwrap_or(half >= 1 << 15)
}

/// Per-slot ΔΔ accumulation against either [`LeafA2`] form. Value-exact:
/// the precombined entry is the subset-sum of the products the masked
/// adds select (field addition is exact and commutative).
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
fn leaf_a2_slot_add<F>(a2: &mut F, tbl: &LeafA2<F>, b: usize, lp: usize, rp: usize, zero: &F)
where
    F: InnerTransparentField + WideMulAcc,
{
    match tbl {
        LeafA2::Precombined(t) => *a2 += &t[(b << 4) | lp | (rp << 2)],
        LeafA2::Factored(p) => {
            // Two-temp tree: the four masked selects reduce pairwise so the
            // `a2` accumulator still takes ONE add per slot (same serial
            // chain profile as the precombined load), with the masked work
            // ILP-parallel beside the a0/a1 gathers.
            let base = b << 2;
            let mut ta = zero.clone();
            let mut tb = zero.clone();
            F::add_assign_masked(&mut ta, &p[base], lp & rp & 1 != 0); // m_{L0}∧m_{R0}
            F::add_assign_masked(&mut ta, &p[base + 1], (lp >> 1) & rp & 1 != 0); // m_{L1}∧m_{R0}
            F::add_assign_masked(&mut tb, &p[base + 2], lp & (rp >> 1) & 1 != 0); // m_{L0}∧m_{R1}
            F::add_assign_masked(&mut tb, &p[base + 3], (lp >> 1) & (rp >> 1) & 1 != 0); // m_{L1}∧m_{R1}
            *a2 += ta + tb;
        }
    }
}

/// Software prefetch on the stash-gather rounds (`pair3_r2`/`leaf3_r3`
/// messages + the two materialising folds): issue `prfm pldl1keep` for
/// slot `b + PRFM_DIST`'s fold-table lines while slot `b` computes — the
/// line-within-window picks are data-dependent (committed bits), which
/// defeats the hardware prefetcher, but the indices are cheaply
/// recomputable ahead from the sequential bit words.
/// `F2Z_LUT_PRFM=0/1` forces off/on; unset (the default) picks by round
/// size — on iff `half ≥ 2^14` (these sites run at rounds 2–3, so this
/// is the n=30-class boundary: stashes `2·32·half·16 B ≥ 16.8 MB`, past
/// the P-cluster L2). Measured (fresh box, alternated in-window pairs):
/// n=30 the four scopes drop 20–25 % and prove −4.8 % (2518 → 2394 ms
/// median); n=28 (8.4 MB stashes, shallow misses) and n=26 it LOSES
/// 1–8 ms — the index recompute + LSU pressure beat L2-hit latency.
/// Semantically inert (a hint), so proofs are byte-identical by
/// construction. Env read once per process.
fn lut_prfm(half: usize) -> bool {
    static ENV: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    let env = *ENV.get_or_init(|| match std::env::var("F2Z_LUT_PRFM") {
        Ok(v) if v == "0" => Some(false),
        Ok(v) if v == "1" => Some(true),
        _ => None,
    });
    env.unwrap_or(half >= 1 << 14)
}

/// Prefetch look-ahead in slots for [`lut_prfm`] (shared by the forest's
/// T4-gather sites in `merged_forest`).
pub(crate) const PRFM_DIST: usize = 16;

/// `prfm pldl1keep` on `&v[idx]` (callers pass in-bounds indices; the
/// hint has no architectural effect either way). No-op off aarch64.
#[inline(always)]
pub(crate) fn prefetch_l1<F>(v: &[F], idx: usize) {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let p = (v.as_ptr() as *const u8).wrapping_add(idx * core::mem::size_of::<F>());
        core::arch::asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) p,
            options(nostack, preserves_flags, readonly)
        );
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = (v, idx);
    }
}

#[allow(clippy::arithmetic_side_effects)]
fn build_leaf_tables<F>(v1: &[F], tau_l: &[F], tau_r: &[F], zero: &F) -> LeafTables<F>
where
    F: InnerTransparentField,
{
    let half = v1.len();
    debug_assert_eq!(tau_l.len(), half << 1);
    debug_assert_eq!(tau_r.len(), half << 1);
    let factored = leaf_a2_factored(half);
    let mut t_a0 = Vec::with_capacity(half << 2);
    let mut t_a1 = Vec::with_capacity(half << 2);
    let mut t_a2 = Vec::with_capacity(if factored { half << 2 } else { half << 4 });
    let mut w_sum = zero.clone();
    for b in 0..half {
        let w = &v1[b];
        let sl0 = w.clone() * &tau_l[b << 1];
        let sl1 = w.clone() * &tau_l[(b << 1) | 1];
        let sr0 = w.clone() * &tau_r[b << 1];
        let sr1 = w.clone() * &tau_r[(b << 1) | 1];
        let p00 = sl0.clone() * &tau_r[b << 1];
        let p11 = sl1.clone() * &tau_r[(b << 1) | 1];
        let p10 = sl1.clone() * &tau_r[b << 1];
        let p01 = sl0.clone() * &tau_r[(b << 1) | 1];
        // 4-case singles+pair combos.
        t_a0.push(zero.clone());
        t_a0.push(sl0.clone());
        t_a0.push(sr0.clone());
        t_a0.push(sl0.clone() + &sr0 + &p00);
        t_a1.push(zero.clone());
        t_a1.push(sl1.clone());
        t_a1.push(sr1.clone());
        t_a1.push(sl1.clone() + &sr1 + &p11);
        if factored {
            // Raw cross products, mask-selected at consumption
            // ([`leaf_a2_slot_add`]).
            t_a2.push(p00);
            t_a2.push(p10);
            t_a2.push(p01);
            t_a2.push(p11);
        } else {
            // 16-case ΔΔ combos: case c = m_{L0} | m_{L1}≪1 | m_{R0}≪2 | m_{R1}≪3.
            for c in 0..16usize {
                let mut v = zero.clone();
                if c & 0b0101 == 0b0101 {
                    v += &p00; // m_{L0}∧m_{R0}
                }
                if c & 0b0110 == 0b0110 {
                    v += &p10; // m_{L1}∧m_{R0}
                }
                if c & 0b1001 == 0b1001 {
                    v += &p01; // m_{L0}∧m_{R1}
                }
                if c & 0b1010 == 0b1010 {
                    v += &p11; // m_{L1}∧m_{R1}
                }
                t_a2.push(v);
            }
        }
        w_sum += w;
    }
    let a2 = if factored { LeafA2::Factored(t_a2) } else { LeafA2::Precombined(t_a2) };
    LeafTables { t_a0, t_a1, a2, w_sum }
}

/// Round-1 fold tables for one leaf `tau` set, in **case-LUT** form: the
/// folded round-2 entry is `v' = v_0 + ρ(v_1 − v_0) = 1 + m_0·(1+ρ)τ_0 +
/// m_1·ρτ_1` (char 2, the 1s of `v_1 − v_0` cancel), precombined over the
/// bit pair — `t[(b≪2) | (m_0 | m_1≪1)]` — so each group's fold is ONE
/// indexed load per entry.
struct LeafFoldTables<F> {
    t_l: Vec<F>,
    t_r: Vec<F>,
}

#[allow(clippy::arithmetic_side_effects)]
fn build_leaf_fold_tables<F>(rho: &F, one: &F, tau_l: &[F], tau_r: &[F]) -> LeafFoldTables<F>
where
    F: InnerTransparentField,
{
    let half = tau_l.len() >> 1;
    let one_plus_rho = one.clone() + rho;
    let build = |tau: &[F]| -> Vec<F> {
        let mut t = Vec::with_capacity(half << 2);
        for b in 0..half {
            let f0 = one_plus_rho.clone() * &tau[b << 1];
            let f1 = rho.clone() * &tau[(b << 1) | 1];
            t.push(one.clone());
            t.push(one.clone() + &f0);
            t.push(one.clone() + &f1);
            t.push(one.clone() + &f0 + &f1);
        }
        t
    };
    LeafFoldTables { t_l: build(tau_l), t_r: build(tau_r) }
}

/// Round-1 message tables for one [`Pair2TauSet`], two interchangeable
/// forms (byte-identical sums either way):
///
/// **`Precombined`** (opt-out, `F2Z_PAIR2_FACTORED=0`) — case-LUT form (16
/// entries per slot each; slot `b` pairs positions `2b, 2b+1`):
/// - `t_a0[b≪4 | (cE0≪2|cO0)]` = `w_b·TE_{2b}[cE0]·TO_{2b}[cO0]` (the
///   `Σ w·L0·R0` term),
/// - `t_a1[…(cE1≪2|cO1)]` = the odd variant,
/// - `t_wde[…(cE0≪2|cE1)]` = `w_b·(TE_{2b}[cE0] + TE_{2b+1}[cE1])` — the
///   weighted ΔL,
/// - `t_do[…(cO0≪2|cO1)]` = the unweighted ΔR;
///
/// the ΔL·ΔR cross term is then ONE wide multiply per slot — but the four
/// tables carry `64·2^k` entries (16 MiB at the deployed forest shapes,
/// past L2), and the four gather streams' misses dominate the round.
///
/// **`Factored`** (the default) — only the suffix-weighted TE array,
/// `wte[i] = v1[i≫3]·te[i]` (layout identical to `te`, `4·2^k` entries);
/// the round body recombines against the set's raw `to`:
/// `a0 += wide(wte[8b|cE0], to[8b|cO0])`, `t11 += wide(wte[8b|4|cE1],
/// to[8b|4|cO1])`, `ΔΔ += wide(wte0 + wte1, to0 + to1)` — three wide
/// multiplies per slot instead of one, against a 4× smaller (L2-resident)
/// table set. The sums are the exact same field elements: the products
/// keep the build's association `(w·TE)·TO`, char-2 `w·TE0 + w·TE1` IS the
/// precombined `w·(TE0+TE1)` (distributivity is exact), and the deferred
/// reduction is F₂-linear, so wide-vs-narrow accumulation reduces to
/// identical values.
enum Pair2Tables<F> {
    Precombined {
        t_a0: Vec<F>,
        t_a1: Vec<F>,
        t_wde: Vec<F>,
        t_do: Vec<F>,
    },
    Factored {
        wte: Vec<F>,
    },
}

/// Factored [`Pair2Tables`] — the default: 4× less table footprint on the
/// 16-case rounds, measured faster at DRAM-scale forest shapes.
/// `F2Z_PAIR2_FACTORED=0` opts out (restores the precombined 16-case
/// tables — diagnostic / A-B measurement). Byte-identical proofs either
/// way. Read once per process.
fn pair2_factored() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("F2Z_PAIR2_FACTORED").map_or(true, |v| v != "0"))
}

#[allow(clippy::arithmetic_side_effects)]
fn build_pair2_tables<F>(v1: &[F], set: &Pair2TauSet<F>) -> Pair2Tables<F>
where
    F: InnerTransparentField,
{
    let half = v1.len();
    debug_assert_eq!(set.te.len(), half << 3, "te = 4·2^k entries");
    debug_assert_eq!(set.to.len(), half << 3, "to = 4·2^k entries");
    if pair2_factored() {
        let wte: Vec<F> =
            set.te.iter().enumerate().map(|(i, t)| v1[i >> 3].clone() * t).collect();
        return Pair2Tables::Factored { wte };
    }
    let mut t_a0 = Vec::with_capacity(half << 4);
    let mut t_a1 = Vec::with_capacity(half << 4);
    let mut t_wde = Vec::with_capacity(half << 4);
    let mut t_do = Vec::with_capacity(half << 4);
    for b in 0..half {
        let w = &v1[b];
        // w·TE at the even (2b) and odd (2b+1) positions, 4 cases each.
        let wte0: Vec<F> = (0..4).map(|c| w.clone() * &set.te[(b << 3) | c]).collect();
        let wte1: Vec<F> = (0..4).map(|c| w.clone() * &set.te[(b << 3) | 4 | c]).collect();
        for ce in 0..4 {
            for co in 0..4 {
                t_a0.push(wte0[ce].clone() * &set.to[(b << 3) | co]);
                t_a1.push(wte1[ce].clone() * &set.to[(b << 3) | 4 | co]);
            }
        }
        for c0 in 0..4 {
            for c1 in 0..4 {
                t_wde.push(wte0[c0].clone() + &wte1[c1]);
                t_do.push(set.to[(b << 3) | c0].clone() + &set.to[(b << 3) | 4 | c1]);
            }
        }
    }
    Pair2Tables::Precombined { t_a0, t_a1, t_wde, t_do }
}

/// The factored 16-case round body shared by the four consumers
/// ([`GroupBufs::Pair2Bits`]/[`GroupBufs::Pair3Bits`] round 1 over the
/// pair tau sets, [`GroupBufs::Leaf2Bits`]/[`GroupBufs::Leaf3Bits`] round
/// 2 over the stashed value sets): per slot, two weighted-even/odd wide
/// products plus the ΔΔ cross product of the in-register sums — the same
/// field values as the precombined tables (see [`Pair2Tables`]).
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
fn pair2_factored_body<F>(
    wte: &[F],
    to: &[F],
    half: usize,
    zero: &F,
    cases: impl Fn(usize) -> (usize, usize, usize, usize),
) -> (F, F, F)
where
    F: InnerTransparentField + WideMulAcc,
{
    let mut a0w = F::wide_zero(zero);
    let mut t11w = F::wide_zero(zero);
    let mut a2w = F::wide_zero(zero);
    for b in 0..half {
        let (ce0, ce1, co0, co1) = cases(b);
        let u0 = &wte[(b << 3) | ce0];
        let u1 = &wte[(b << 3) | 4 | ce1];
        let t0 = &to[(b << 3) | co0];
        let t1 = &to[(b << 3) | 4 | co1];
        F::wide_add_assign(&mut a0w, &F::mul_wide(u0, t0));
        F::wide_add_assign(&mut t11w, &F::mul_wide(u1, t1));
        let du = u0.clone() + u1;
        let dt = t0.clone() + t1;
        F::wide_add_assign(&mut a2w, &F::mul_wide(&du, &dt));
    }
    let a0 = F::from_wide(a0w);
    let t11 = F::from_wide(t11w);
    let a2 = F::from_wide(a2w);
    let a1 = t11 + &a0 + &a2;
    (a0, a1, a2)
}

/// Round-1 fold tables for one [`Pair2TauSet`]: the folded round-2 entry is
/// `v' = v_0 + ρ(v_1 − v_0) = (1+ρ)·v_0 + ρ·v_1` (char 2), precombined
/// over the two positions' cases — `f[b≪4 | (c0≪2|c1)]` — so each group's
/// fold is ONE indexed load per entry.
struct Pair2FoldTables<F> {
    f_e: Vec<F>,
    f_o: Vec<F>,
}

#[allow(clippy::arithmetic_side_effects)]
fn build_pair2_fold_tables<F>(rho: &F, one: &F, set: &Pair2TauSet<F>) -> Pair2FoldTables<F>
where
    F: InnerTransparentField,
{
    let half = set.te.len() >> 3;
    let one_plus_rho = one.clone() + rho;
    let build = |t: &[F]| -> Vec<F> {
        let mut f = Vec::with_capacity(half << 4);
        for b in 0..half {
            let e0: Vec<F> = (0..4).map(|c| one_plus_rho.clone() * &t[(b << 3) | c]).collect();
            let e1: Vec<F> = (0..4).map(|c| rho.clone() * &t[(b << 3) | 4 | c]).collect();
            for c0 in 0..4 {
                for c1 in 0..4 {
                    f.push(e0[c0].clone() + &e1[c1]);
                }
            }
        }
        f
    };
    Pair2FoldTables { f_e: build(&set.te), f_o: build(&set.to) }
}

/// The four 2-bit cases of slot `b`'s entries: `(cE0, cE1)` from the
/// adjacent bit pair at position `2b`, `(cO0, cO1)` from the pair at
/// `2b + H` (`H` even ⇒ the pair never straddles a word).
#[inline]
#[allow(clippy::arithmetic_side_effects)]
fn pair2_cases(lbits: &[u64], rbits: &[u64], b: usize, h_off: usize) -> (usize, usize, usize, usize) {
    let pe = b << 1;
    let lp_e = ((lbits[pe >> 6] >> (pe & 63)) & 3) as usize;
    let rp_e = ((rbits[pe >> 6] >> (pe & 63)) & 3) as usize;
    let po = pe + h_off;
    let lp_o = ((lbits[po >> 6] >> (po & 63)) & 3) as usize;
    let rp_o = ((rbits[po >> 6] >> (po & 63)) & 3) as usize;
    let ce0 = (lp_e & 1) | ((rp_e & 1) << 1);
    let ce1 = (lp_e >> 1) | (rp_e & 2);
    let co0 = (lp_o & 1) | ((rp_o & 1) << 1);
    let co1 = (lp_o >> 1) | (rp_o & 2);
    (ce0, ce1, co0, co1)
}

/// The four 2-bit cases of a [`GroupBufs::Leaf2Bits`] group's round-2 slot
/// `b`: the round-2 entries are per-position 4-case selects keyed by
/// adjacent ORIGINAL-bit pairs, so slot `b` (pairing round-2 positions
/// `2b, 2b+1`) reads original bits `4b..4b+4` — one aligned 4-bit nibble
/// per side (`4b ≡ 0 (mod 4)` never straddles a word). Returns
/// `(cE0, cE1, cO0, cO1)` = the L-side cases of positions `2b, 2b+1` and
/// the R-side same.
#[inline]
#[allow(clippy::arithmetic_side_effects)]
fn leaf2_cases(lbits: &[u64], rbits: &[u64], b: usize) -> (usize, usize, usize, usize) {
    let p = b << 2;
    let nl = ((lbits[p >> 6] >> (p & 63)) & 15) as usize;
    let nr = ((rbits[p >> 6] >> (p & 63)) & 15) as usize;
    (nl & 3, nl >> 2, nr & 3, nr >> 2)
}

/// Value-table index of a [`GroupBufs::Leaf3Bits`] round-3 entry from its
/// raw original-bit nibble (bits `4p..4p+4`): the stashed fold tables are
/// indexed `(c0≪2)|c1` with `c0` the LOW adjacent bit pair — a
/// nibble-half swap.
#[inline]
#[allow(clippy::arithmetic_side_effects)]
fn leaf3_idx(nib: usize) -> usize {
    ((nib & 3) << 2) | (nib >> 2)
}

/// Value-table index of a [`GroupBufs::Pair3Bits`] round-2 entry from the
/// 2-bit windows of `lbits`/`rbits` at its key position: the cases
/// interleave l/r per position — `c0 = l₀|r₀≪1`, `c1 = l₁|r₁≪1`, index
/// `(c0≪2)|c1` (matching [`pair2_cases`]' round-1 fold).
#[inline]
#[allow(clippy::arithmetic_side_effects)]
fn pair3_idx(nl2: usize, nr2: usize) -> usize {
    (((nl2 & 1) | ((nr2 & 1) << 1)) << 2) | ((nl2 >> 1) | ((nr2 >> 1) << 1))
}

/// `T4` select index at position `j` of a [`GroupBufs::T4Bits`] group
/// (`q1 = 2^{k+1}` = the T4 position count): E-pair case from the
/// transposed leaf halves at offset `j`, O-pair case at `j + q1`.
#[inline]
#[allow(clippy::arithmetic_side_effects)]
fn t4bits_idx(lbits: &[u64], rbits: &[u64], j: usize, q1: usize) -> usize {
    let bit = |bits: &[u64], p: usize| -> usize { ((bits[p >> 6] >> (p & 63)) & 1) as usize };
    let ce = bit(lbits, j) | (bit(rbits, j) << 1);
    let co = bit(lbits, j + q1) | (bit(rbits, j + q1) << 1);
    (j << 4) | (ce << 2) | co
}

/// Pass fusion — the DEFAULT: defer each round's fold and run it fused
/// into the NEXT round's message pass (one read of the unfolded buffers
/// instead of fold-read + message-read; measured ~10 % prove at n=26–28).
/// Byte-identical: the same field values in the same transcript order —
/// only the physical pass structure changes. Gated on all-Dense-single-pair
/// groups (the forest's shape). `F2Z_EQF_FUSE=0` opts out (restores the
/// eager two-pass fold path).
fn eqf_fuse_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("F2Z_EQF_FUSE").map_or(true, |v| v != "0"))
}

/// **Double-fold**: bind TWO variables per pass over the dense buffers.
/// One pass accumulates the 3×3 bivariate grid `G(X₁,X₂) = Σ_b
/// V_{j+1}(b)·L(X₁,X₂,b)·R(X₁,X₂,b)`; round `j`'s message is
/// `Σ_{x₂} eq1(x₂; q_{j+1})·G(X₁,x₂)` and round `j+1`'s is `G(ρ_j, X₂)` —
/// derived from nine stored field elements per group, so round `j+1`
/// touches no buffer at all. Halves the number of dense passes: the
/// cascade's traffic drops from `3N` to `1.67N` (`1.5N` when round 1
/// arrives precomputed and the grid can only start at round 2), at
/// 13 multiplies per 4 slots instead of 15. Byte-identical — the same
/// messages in the same transcript order, and every accumulation is
/// `F₂`-linear in the reduction. `F2Z_EQF_DOUBLE=0` opts out. Read once
/// per process.
pub(crate) fn eqf_double() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("F2Z_EQF_DOUBLE").map_or(true, |v| v != "0"))
}

/// Diagnostic (opt-in): bypass the hand-fused NEON whole-buffer kernels
/// (message + fold + fused fold+round), forcing the generic fallback
/// loops — isolates pass-structure gains from kernel quality in A/B runs.
fn eqf_nokernel() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("F2Z_EQF_NOKERNEL").is_some())
}

/// Per-group suffix tensors `V_j` (`j = 1..=k`), built back-to-front:
/// `V_k = [1]`, `V_j[2b' | b0] = eq1(b0; q[j])·V_{j+1}[b']`.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn suffix_tensors<F>(q: &[F], field_cfg: &F::Config) -> Vec<Vec<F>>
where
    F: InnerTransparentField + Send + Sync,
    F::Config: Sync,
{
    let k = q.len();
    let one = F::one_with_cfg(field_cfg);
    let zero = F::zero_with_cfg(field_cfg);
    let mut suffix = Vec::with_capacity(k);
    let mut cur = vec![one.clone()];
    suffix.push(cur.clone());
    for j in (1..k).rev() {
        let e1 = q[j].clone();
        let e0 = one.clone() - &e1;
        let mut next = vec![zero.clone(); cur.len() * 2];
        // Sequential within a group; the caller builds all groups' suffix
        // tensors in parallel (across groups), so nesting parallelism here
        // would only add overhead on these short vectors.
        next.chunks_mut(2).zip(cur.iter()).for_each(|(pair, v)| {
            pair[0] = v.clone() * &e0;
            pair[1] = v.clone() * &e1;
        });
        suffix.push(next.clone());
        cur = next;
    }
    suffix.reverse(); // suffix[j−1] = V_j, length 2^{k−j}
    suffix
}

/// One deferred-fold step — the eager fold's exact formula
/// `v₀ + ρ·(v₁ − v₀)` over the pair at `i`, in registers.
#[inline(always)]
#[allow(clippy::arithmetic_side_effects)]
fn fold1_at<F: InnerTransparentField>(v: &[F], i: usize, rho: &F) -> F {
    let v0 = v[i].clone();
    let d = v[i + 1].clone() - &v0;
    v0 + &(rho.clone() * &d)
}

/// The logical (fully folded) value at logical index `i` of a buffer that
/// still carries `pending.len() ∈ {0,1,2}` deferred challenges — the
/// physical block for `i` is `[i·2^d, (i+1)·2^d)`, `pending[0]` binding
/// the low bit.
#[inline(always)]
#[allow(clippy::arithmetic_side_effects)]
fn fold_logical<F: InnerTransparentField>(v: &[F], i: usize, pending: &[F]) -> F {
    match pending.len() {
        0 => v[i].clone(),
        1 => fold1_at(v, i << 1, &pending[0]),
        _ => {
            let base = i << 2;
            let a = fold1_at(v, base, &pending[0]);
            let b = fold1_at(v, base + 2, &pending[0]);
            a.clone() + &(pending[1].clone() * &(b.clone() - &a))
        }
    }
}

/// The double-fold pass: fold the deferred challenges into the buffer
/// prefix and accumulate this group's bivariate grid over the two
/// variables `j` (buffer bit 0) and `j+1` (bit 1).
///
/// Returns `[A_u[v]]` at `u·3 + v`: the `X₁`-monomial coefficients
/// (`u = 0,1,2`) of `G(·, x₂)` at the three `X₂` nodes `v = 0, 1, ∞`.
/// Both consumers are then nine-element arithmetic ([`grid_this_round`],
/// [`grid_next_round`]) — round `j+1` never reads a buffer.
///
/// Per quad: the four weighted `L` values (`w` folded in before the grid,
/// which is linear in them), the two 3×3 node grids by differences alone
/// (char 2: XORs), nine wide products. Value-exact against two
/// consecutive single-variable passes: the `X₁`-node conversion is the
/// same `a₁ = H(1) − H(0) − H(∞)` the scalar bodies apply, and reduction
/// is `F₂`-linear, so accumulating each node separately and converting
/// after reduction lands on the identical field elements.
#[allow(clippy::arithmetic_side_effects)]
fn dense_grid_pass<F>(l: &mut Vec<F>, r: &mut Vec<F>, pending: &[F], suffix: &[F], quads: usize, zero: &F) -> [F; 9]
where
    F: InnerTransparentField + WideMulAcc,
{
    let d = pending.len();
    debug_assert_eq!(l.len(), (quads << 2) << d, "grid pass reads the unfolded buffers");
    debug_assert_eq!(suffix.len(), quads, "grid weight is the round j+1 suffix tensor");
    let mut acc = core::array::from_fn::<_, 9, _>(|_| F::wide_zero(zero));
    for b in 0..quads {
        let base = b << 2;
        // Logical quad: index (b≪2) | (x₂≪1) | x₁.
        let lv: [F; 4] = core::array::from_fn(|i| fold_logical(l, base | i, pending));
        let rv: [F; 4] = core::array::from_fn(|i| fold_logical(r, base | i, pending));
        // The weight rides the L side only, folded in BEFORE the grid
        // (which is linear in the four values); the buffer keeps the
        // unweighted folds.
        let lw: [F; 4] = core::array::from_fn(|i| suffix[b].clone() * &lv[i]);
        // Node grids {0, 1, ∞}² from the four multilinear values: rows
        // v = x₂ node, columns u = x₁ node.
        let grid = |v: &[F; 4]| -> [F; 9] {
            let d00 = v[1].clone() - &v[0]; // ∂x₁ at x₂ = 0
            let d01 = v[3].clone() - &v[2]; // ∂x₁ at x₂ = 1
            [
                v[0].clone(),
                v[1].clone(),
                d00.clone(),
                v[2].clone(),
                v[3].clone(),
                d01.clone(),
                v[2].clone() - &v[0],
                v[3].clone() - &v[1],
                d01 - &d00,
            ]
        };
        let lg = grid(&lw);
        let rg = grid(&rv);
        for (a, (x, y)) in acc.iter_mut().zip(lg.iter().zip(rg.iter())) {
            F::wide_add_assign(a, &F::mul_wide(x, y));
        }
        // Land the folded quad in the prefix — writes trail the reads
        // (the next quad's first physical index is `(b+1)·2^{d+2}`).
        if d > 0 {
            for (i, (lf, rf)) in lv.into_iter().zip(rv).enumerate() {
                l[base | i] = lf;
                r[base | i] = rf;
            }
        }
    }
    if d > 0 {
        l.truncate(quads << 2);
        r.truncate(quads << 2);
    }
    // Node → X₁-monomial per x₂ node: a₀ = H(0), a₂ = H(∞),
    // a₁ = H(1) − H(0) − H(∞).
    let e: [F; 9] = acc.map(F::from_wide);
    core::array::from_fn(|i| {
        let (u, v) = (i / 3, i % 3);
        let base = v * 3;
        match u {
            0 => e[base].clone(),
            2 => e[base + 2].clone(),
            _ => e[base + 1].clone() - &e[base] - &e[base + 2],
        }
    })
}

/// Round `j`'s coefficient triple from the grid:
/// `H^{(j)}(X₁) = Σ_{x₂} eq1(x₂; q_{j+1})·G(X₁, x₂)`. Nodes `v = 0, 1`
/// ARE those two evaluations, so each `X₁` coefficient is one `eq1` blend.
#[allow(clippy::arithmetic_side_effects)]
fn grid_this_round<F: InnerTransparentField>(g: &[F; 9], q_next: &F, one: &F) -> (F, F, F) {
    let e1 = q_next.clone();
    let e0 = one.clone() - q_next;
    let a = |u: usize| -> F { e0.clone() * &g[u * 3] + &(e1.clone() * &g[u * 3 + 1]) };
    (a(0), a(1), a(2))
}

/// Round `j+1`'s coefficient triple from the same grid:
/// `H^{(j+1)}(X₂) = G(ρ_j, X₂)` — evaluate each `X₂` node's `X₁`
/// polynomial at ρ, then convert the three nodes to monomial
/// coefficients. Nine field elements in, no buffer touched.
#[allow(clippy::arithmetic_side_effects)]
fn grid_next_round<F: InnerTransparentField>(g: &[F; 9], rho: &F) -> (F, F, F) {
    let rho2 = rho.clone() * rho;
    let at = |v: usize| -> F {
        g[v].clone() + &(rho.clone() * &g[3 + v]) + &(rho2.clone() * &g[6 + v])
    };
    let (n0, n1, ninf) = (at(0), at(1), at(2));
    let a1 = n1 - &n0 - &ninf;
    (n0, a1, ninf)
}

/// Fold `pending.len()` deferred challenges into the prefix and
/// accumulate ONE round's coefficient triple — the generic
/// `d`-deferred-challenge form of the fused message pass. Used for the
/// `d = 2` case a double-fold cascade leaves behind at its last rounds
/// (`d = 1` keeps the hand-fused kernel path).
#[allow(clippy::arithmetic_side_effects)]
fn dense_msg_pass_d<F>(l: &mut Vec<F>, r: &mut Vec<F>, pending: &[F], suffix: &[F], half: usize, zero: &F) -> (F, F, F)
where
    F: InnerTransparentField + WideMulAcc,
{
    debug_assert_eq!(l.len(), (half << 1) << pending.len(), "d-fold pass buffer shape");
    let mut a0 = F::wide_zero(zero);
    let mut a1 = F::wide_zero(zero);
    let mut a2 = F::wide_zero(zero);
    for b in 0..half {
        let e = b << 1;
        let fl0 = fold_logical(l, e, pending);
        let fl1 = fold_logical(l, e | 1, pending);
        let fr0 = fold_logical(r, e, pending);
        let fr1 = fold_logical(r, e | 1, pending);
        let w = &suffix[b];
        let l0w = w.clone() * &fl0;
        let l1w = w.clone() * &fl1;
        let wc0 = F::mul_wide(&l0w, &fr0);
        let w11 = F::mul_wide(&l1w, &fr1);
        let dr = fr1.clone() - &fr0;
        let dl = l1w - &l0w;
        let wc2 = F::mul_wide(&dl, &dr);
        F::wide_add_assign(&mut a0, &wc0);
        F::wide_add_assign(&mut a2, &wc2);
        F::wide_add_assign(&mut a1, &w11);
        F::wide_sub_assign(&mut a1, &wc0);
        F::wide_sub_assign(&mut a1, &wc2);
        l[e] = fl0;
        l[e | 1] = fl1;
        r[e] = fr0;
        r[e | 1] = fr1;
    }
    l.truncate(half << 1);
    r.truncate(half << 1);
    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
}

/// Prove `Σ_x Σ_t eq(x; q_t)·Σ_i L_{t,i}(x)·R_{t,i}(x)` (see the module
/// doc). Returns `(proof, point, final_evals)` where `final_evals[t][i] =
/// (L_{t,i}(point), R_{t,i}(point))` — the per-pair multilinear evaluations
/// at the sumcheck point, interpolated from the fold buffers exactly as the
/// generic prover's final-state interpolation does.
#[allow(clippy::arithmetic_side_effects, clippy::type_complexity)]
pub fn prove_eq_inner_sumcheck<F>(
    transcript: &mut impl Transcript,
    groups: Vec<EqInnerGroup<F>>,
    field_cfg: &F::Config,
) -> (SumcheckProof<F>, Vec<F>, Vec<Vec<(F, F)>>)
where
    F: InnerTransparentField + FromPrimitiveWithConfig + WideMulAcc + Send + Sync,
    F::Inner: ConstTranscribable + Zero + Default + Send + Sync,
    F::Modulus: ConstTranscribable,
    F::Config: Sync,
{
    let mixed = groups
        .into_iter()
        .map(|g| EqInnerGroupMixed { q: g.q, scale: g.scale, bufs: GroupBufs::Dense(g.pairs) })
        .collect();
    prove_eq_inner_sumcheck_mixed(transcript, mixed, &[], &[], &[], field_cfg)
}

/// [`prove_eq_inner_sumcheck`] over [`EqInnerGroupMixed`] groups: `Dense`
/// groups run exactly the classic rounds; `LeafBits` groups (char-2,
/// shared-`q`, single-pair, `k ≥ 2`) run round 1 as branchless bit-selected
/// subset-sums over the shared [`LeafTables`] and materialise their dense
/// round-2 buffers in the round-1 fold; `Leaf2Bits` groups (`k ≥ 3`) run
/// rounds 1 AND 2 from the bits (round 2 over the stashed ρ₁-dependent
/// 4-case value tables) and materialise only their round-3 buffers;
/// `Leaf3Bits` (`k ≥ 4`) / `Pair3Bits` (`k ≥ 3`) go one round deeper
/// still (the last bit-driven round reads the stashed fold tables
/// INLINE); `T4Bits` (`k ≥ 2`) runs a product layer's round 1 straight
/// off the shared `t4_sets` table so its input level never exists.
/// Byte-identical to materialising the leaves up front — every step is an
/// exact char-2 algebraic identity.
#[allow(clippy::arithmetic_side_effects, clippy::type_complexity)]
pub fn prove_eq_inner_sumcheck_mixed<F>(
    transcript: &mut impl Transcript,
    groups: Vec<EqInnerGroupMixed<F>>,
    tau_sets: &[(Vec<F>, Vec<F>)],
    pair_tau_sets: &[Pair2TauSet<F>],
    t4_sets: &[Vec<F>],
    field_cfg: &F::Config,
) -> (SumcheckProof<F>, Vec<F>, Vec<Vec<(F, F)>>)
where
    F: InnerTransparentField + FromPrimitiveWithConfig + WideMulAcc + Send + Sync,
    F::Inner: ConstTranscribable + Zero + Default + Send + Sync,
    F::Modulus: ConstTranscribable,
    F::Config: Sync,
{
    prove_eq_inner_sumcheck_mixed_pre(
        transcript,
        groups,
        tau_sets,
        pair_tau_sets,
        t4_sets,
        None,
        false,
        field_cfg,
    )
}

/// [`prove_eq_inner_sumcheck_mixed`] in the **Gruen** round-message format
/// (see the module doc): every round sends the two non-constant coefficients
/// of the quadratic cofactor instead of the three degree-3 tail nodes.
/// Requires all groups at ONE shared eq point (asserted). Verified by
/// [`verify_eq_inner_sumcheck_gruen`], NOT the generic sumcheck verifier.
#[allow(clippy::arithmetic_side_effects, clippy::type_complexity)]
pub fn prove_eq_inner_sumcheck_mixed_gruen<F>(
    transcript: &mut impl Transcript,
    groups: Vec<EqInnerGroupMixed<F>>,
    tau_sets: &[(Vec<F>, Vec<F>)],
    pair_tau_sets: &[Pair2TauSet<F>],
    t4_sets: &[Vec<F>],
    field_cfg: &F::Config,
) -> (SumcheckProof<F>, Vec<F>, Vec<Vec<(F, F)>>)
where
    F: InnerTransparentField + FromPrimitiveWithConfig + WideMulAcc + Send + Sync,
    F::Inner: ConstTranscribable + Zero + Default + Send + Sync,
    F::Modulus: ConstTranscribable,
    F::Config: Sync,
{
    prove_eq_inner_sumcheck_mixed_pre(
        transcript,
        groups,
        tau_sets,
        pair_tau_sets,
        t4_sets,
        None,
        true,
        field_cfg,
    )
}

/// Verify a **Gruen-format** eq-factored sumcheck against the shared eq
/// point `q` (round order — coordinate `i` is round `i`'s). Round `i`'s
/// message carries exactly the two non-constant coefficients `(Ĥ1, Ĥ2)` of
/// the quadratic cofactor `Ĥ_i`, with the full round polynomial
/// `P_i(X) = eq1(X; q[i])·Ĥ_i(X)`. The verifier reconstructs
/// `Ĥ0 = S_i − q[i]·(Ĥ1 + Ĥ2)` from the running claim `S_i` (the identity
/// `S_i = P_i(0) + P_i(1) = Ĥ0 + q[i]·(Ĥ1 + Ĥ2)` holds over any field —
/// no inversion needed) and chains `S_{i+1} = eq1(ρ_i; q[i])·Ĥ_i(ρ_i)`.
/// Transcript ops mirror the prover exactly: the `(k, 3)` header, then per
/// round tail absorb → challenge draw → challenge re-absorb. Restricting
/// the prover to multiples of the public `eq1` factor only shrinks a
/// cheater's message space, so the standard sumcheck round analysis
/// applies unchanged.
///
/// Returns the [`Subclaim`]: the bound point and the expected evaluation of
/// the full summand (all `eq1` factors included) — the same contract as
/// [`MLSumcheck::verify_as_subprotocol`] on the Generic format.
#[allow(clippy::arithmetic_side_effects)]
pub fn verify_eq_inner_sumcheck_gruen<F>(
    transcript: &mut impl Transcript,
    q: &[F],
    proof: &SumcheckProof<F>,
    field_cfg: &F::Config,
) -> Result<Subclaim<F>, SumCheckError<F>>
where
    F: InnerTransparentField + FromPrimitiveWithConfig,
    F::Inner: ConstTranscribable,
    F::Modulus: ConstTranscribable,
{
    let k = q.len();
    assert!(k >= 1, "eq-factored sumcheck needs >= 1 variable");
    let mut buf = vec![0u8; F::Inner::NUM_BYTES];
    // Header — mirror the prover.
    transcript.absorb_random_field(&F::from_with_cfg(k as u64, field_cfg), &mut buf);
    transcript.absorb_random_field(&F::from_with_cfg(3u64, field_cfg), &mut buf);
    if proof.messages.len() != k {
        return Err(SumCheckError::InvalidProofLength { expected: k, got: proof.messages.len() });
    }
    let one = F::one_with_cfg(field_cfg);
    let mut expected = proof.claimed_sum.clone();
    let mut point: Vec<F> = Vec::with_capacity(k);
    for (i, msg) in proof.messages.iter().enumerate() {
        let tail = &msg.0.tail_evaluations;
        if tail.len() != 2 {
            // A Generic-format (or otherwise malformed) round message must
            // not decode under this verifier.
            return Err(SumCheckError::MaxDegreeExceeded);
        }
        transcript.absorb_random_field_slice(tail, &mut buf);
        let rho: F = transcript.get_field_challenge(field_cfg);
        transcript.absorb_random_field(&rho, &mut buf);
        let (c1, c2) = (tail[0].clone(), tail[1].clone());
        let qi = &q[i];
        let c0 = expected - &(qi.clone() * &(c1.clone() + &c2));
        // Horner: Ĥ(ρ) = Ĥ0 + ρ·(Ĥ1 + ρ·Ĥ2).
        let h_at = c0 + &(rho.clone() * &(c1 + &(rho.clone() * &c2)));
        let e1 = (one.clone() - qi) * &(one.clone() - &rho) + &(qi.clone() * &rho);
        expected = e1 * &h_at;
        point.push(rho);
    }
    Ok(Subclaim { point, expected_evaluation: expected })
}

/// [`prove_eq_inner_sumcheck_mixed`] with optionally PRECOMPUTED round-1
/// coefficients: `pre_round1[t] = (A0, A1, A2)` — the suffix-weighted
/// coefficients of group `t`'s round-1 message polynomial, exactly as the
/// round-1 message pass would accumulate them (same weight-folds, same
/// wide products; XOR accumulation is order-free and the reduction is
/// `F_2`-linear, so ANY generation order yields the identical reduced
/// values). When present, round 1 absorbs the same transcript bytes
/// WITHOUT touching the group buffers — the caller computed the
/// coefficients while GENERATING those buffers (the forest's JIT layer),
/// so the buffers' first read is round 2's fused fold+message pass.
/// Requires every group Dense (asserted): the LUT-round shapes build
/// their round-1 tables here and cannot arrive precomputed.
#[allow(clippy::arithmetic_side_effects, clippy::type_complexity)]
pub fn prove_eq_inner_sumcheck_mixed_pre<F>(
    transcript: &mut impl Transcript,
    groups: Vec<EqInnerGroupMixed<F>>,
    tau_sets: &[(Vec<F>, Vec<F>)],
    pair_tau_sets: &[Pair2TauSet<F>],
    t4_sets: &[Vec<F>],
    mut pre_round1: Option<PreRound<F>>,
    gruen: bool,
    field_cfg: &F::Config,
) -> (SumcheckProof<F>, Vec<F>, Vec<Vec<(F, F)>>)
where
    F: InnerTransparentField + FromPrimitiveWithConfig + WideMulAcc + Send + Sync,
    F::Inner: ConstTranscribable + Zero + Default + Send + Sync,
    F::Modulus: ConstTranscribable,
    F::Config: Sync,
{
    if let Some(pre) = &pre_round1 {
        assert_eq!(pre.len(), groups.len(), "one (A0, A1, A2) triple per group");
        assert!(
            groups.iter().all(|g| matches!(g.bufs, GroupBufs::Dense(_))),
            "precomputed round-1 coefficients require all-Dense groups"
        );
    }
    let k = groups.first().map_or(0, |g| g.q.len());
    debug_assert!(k >= 1, "eq-factored sumcheck needs ≥ 1 variable");
    debug_assert!(groups.iter().all(|g| {
        g.q.len() == k
            && match &g.bufs {
                GroupBufs::Dense(pairs) => {
                    pairs.iter().all(|(l, r)| l.len() == 1 << k && r.len() == 1 << k)
                }
                GroupBufs::LeafBits { lbits, rbits, tau_set } => {
                    lbits.len() == (1usize << k).div_ceil(64)
                        && rbits.len() == (1usize << k).div_ceil(64)
                        && *tau_set < tau_sets.len()
                        && tau_sets[*tau_set].0.len() == 1 << k
                        && tau_sets[*tau_set].1.len() == 1 << k
                }
                GroupBufs::Pair2Bits { lbits, rbits, tau_set } => {
                    lbits.len() == (2usize << k).div_ceil(64)
                        && rbits.len() == (2usize << k).div_ceil(64)
                        && *tau_set < pair_tau_sets.len()
                        && pair_tau_sets[*tau_set].te.len() == 4 << k
                        && pair_tau_sets[*tau_set].to.len() == 4 << k
                }
                GroupBufs::Leaf2Bits { lbits, rbits, tau_set }
                | GroupBufs::Leaf3Bits { lbits, rbits, tau_set } => {
                    lbits.len() == (1usize << k).div_ceil(64)
                        && rbits.len() == (1usize << k).div_ceil(64)
                        && *tau_set < tau_sets.len()
                        && tau_sets[*tau_set].0.len() == 1 << k
                        && tau_sets[*tau_set].1.len() == 1 << k
                }
                GroupBufs::Pair3Bits { lbits, rbits, tau_set } => {
                    lbits.len() == (2usize << k).div_ceil(64)
                        && rbits.len() == (2usize << k).div_ceil(64)
                        && *tau_set < pair_tau_sets.len()
                        && pair_tau_sets[*tau_set].te.len() == 4 << k
                        && pair_tau_sets[*tau_set].to.len() == 4 << k
                }
                GroupBufs::T4Bits { lbits, rbits, tau_set } => {
                    lbits.len() == (4usize << k).div_ceil(64)
                        && rbits.len() == (4usize << k).div_ceil(64)
                        && *tau_set < t4_sets.len()
                        && t4_sets[*tau_set].len() == 32 << k
                }
            }
    }));
    let has_leaf = groups.iter().any(|g| matches!(g.bufs, GroupBufs::LeafBits { .. }));
    let has_pair = groups.iter().any(|g| matches!(g.bufs, GroupBufs::Pair2Bits { .. }));
    let has_leaf2 = groups.iter().any(|g| matches!(g.bufs, GroupBufs::Leaf2Bits { .. }));
    let has_leaf3 = groups.iter().any(|g| matches!(g.bufs, GroupBufs::Leaf3Bits { .. }));
    let has_pair3 = groups.iter().any(|g| matches!(g.bufs, GroupBufs::Pair3Bits { .. }));
    let has_t4b = groups.iter().any(|g| matches!(g.bufs, GroupBufs::T4Bits { .. }));
    let one = F::one_with_cfg(field_cfg);
    let zero = F::zero_with_cfg(field_cfg);
    // The generic path's boundary nodes: F::from(2) = X, F::from(3) = X+1
    // (bit-pattern convention; for prime fields these are the integers).
    let c2 = F::from_with_cfg(2u64, field_cfg);
    let c3 = F::from_with_cfg(3u64, field_cfg);
    // Node squares, for the per-group coefficient→node conversion below.
    let c2sq = c2.clone() * &c2;
    let c3sq = c3.clone() * &c3;
    // See the module doc: in char 2 the fourth node is the affine-flat sum.
    let char2 = one.clone() + &one == zero && c3 == c2.clone() + &one;

    // In the batched-forest GKR every group shares the same reduction point `q`
    // (all trees reduce to one point), so the suffix tensors are identical —
    // compute them ONCE in that case, else once per group. (The L·R products,
    // which differ per group, still drive the per-group round-body parallelism.)
    let shared_q = !groups.is_empty() && groups.iter().all(|g| g.q == groups[0].q);
    // The Gruen message format factors ONE eq1 out of the whole round
    // polynomial — meaningless unless every group sits at the same point.
    assert!(!gruen || shared_q, "Gruen-format rounds require a shared eq point");
    if has_leaf || has_pair || has_leaf2 || has_leaf3 || has_pair3 || has_t4b {
        // The bit expansions' 1-cancellations are char-2 identities, the
        // shared tables assume one suffix tensor, and round 1 must have a
        // fold (j < k) to materialise the dense round-2 buffers.
        assert!(char2, "bit-selected groups require characteristic 2");
        assert!(shared_q, "bit-selected groups require a shared eq point");
        assert!(k >= 2, "bit-selected groups need k >= 2 (materialise tiny trees eagerly)");
    }
    if has_leaf2 {
        // Round 2 must have a fold (j = 2 < k) to materialise the dense
        // round-3 buffers; at k = 2 use `LeafBits`.
        assert!(k >= 3, "Leaf2Bits groups need k >= 3 (use LeafBits at k = 2)");
    }
    if has_leaf3 {
        assert!(k >= 4, "Leaf3Bits groups need k >= 4 (use Leaf2Bits at k = 3)");
    }
    if has_pair3 {
        assert!(k >= 3, "Pair3Bits groups need k >= 3 (use Pair2Bits at k = 2)");
    }
    let suffix: Vec<Vec<Vec<F>>> = {
        let _g = crate::utils::prof::scope("eqf:suffix");
        if shared_q {
            vec![suffix_tensors(&groups[0].q, field_cfg)]
        } else {
            cfg_iter!(groups).map(|g| suffix_tensors(&g.q, field_cfg)).collect()
        }
    };
    let qs: Vec<Vec<F>> = groups.iter().map(|g| g.q.clone()).collect();
    let scales: Vec<F> = groups.iter().map(|g| g.scale.clone()).collect();
    // Per-group fold buffers.
    let mut bufs: Vec<GroupBufs<F>> = groups.into_iter().map(|g| g.bufs).collect();
    let num_groups = qs.len();

    let _g = crate::utils::prof::scope("eqf:rounds");
    let mut buf = vec![0u8; F::Inner::NUM_BYTES];
    // Header — mirror `prove_as_subprotocol`.
    transcript.absorb_random_field(&F::from_with_cfg(k as u64, field_cfg), &mut buf);
    transcript.absorb_random_field(&F::from_with_cfg(3u64, field_cfg), &mut buf);

    let mut a_scalars = scales;
    let mut randomness: Vec<F> = Vec::with_capacity(k);
    let mut messages: Vec<ProverMsg<F>> = Vec::with_capacity(k);
    let mut claimed_sum = zero.clone();
    // Leaf2Bits round-2 state: the ρ₁-dependent per-position 4-case VALUE
    // tables (one [`Pair2TauSet`] per tau set), stashed at round 1's fold —
    // they are exactly that fold's [`LeafFoldTables`].
    let mut leaf2_value_sets: Vec<Pair2TauSet<F>> = Vec::new();
    // Pair3Bits round-2 / Leaf3Bits round-3 state: the respective rounds'
    // 16-case fold tables, stashed instead of consumed — they ARE the
    // next round's per-position value tables (read inline).
    let mut pair3_value_sets: Vec<Pair2FoldTables<F>> = Vec::new();
    let mut leaf3_value_sets: Vec<Pair2FoldTables<F>> = Vec::new();
    // Pass fusion: deferred fold challenges — pushed when a round's fold is
    // skipped, consumed by the next pass. One under plain fusion; TWO once
    // the double-fold binds a pair of variables per pass.
    let mut pending: Vec<F> = Vec::new();
    // Double-fold state: the bivariate grid a pass left behind, spending
    // the NEXT round with no buffer pass at all, plus the challenge that
    // round evaluates it at (tracked separately from `pending` so the
    // path is correct with fusion off, where folds are never deferred).
    let mut grid: Option<Vec<[F; 9]>> = None;
    let mut grid_rho: Option<F> = None;

    for j in 1..=k {
        // Buffers at round j have 2^{k−j+1} entries (leaf-bit groups define
        // theirs implicitly at the same size).
        let half = 1usize << (k - j);

        // Shared leaf tables for round 1 (one per tau set; every group of a
        // set only XOR-selects from them).
        let leaf_tables: Vec<LeafTables<F>> = if j == 1 && (has_leaf || has_leaf2 || has_leaf3)
        {
            let _g = crate::utils::prof::scope("eqf:leaf_tables");
            let v1 = &suffix[0][0];
            cfg_iter!(tau_sets).map(|(tl, tr)| build_leaf_tables(v1, tl, tr, &zero)).collect()
        } else {
            Vec::new()
        };
        let pair2_tables: Vec<Pair2Tables<F>> = if j == 1 && (has_pair || has_pair3) {
            let _g = crate::utils::prof::scope("eqf:pair2_tables");
            let v1 = &suffix[0][0];
            cfg_iter!(pair_tau_sets).map(|set| build_pair2_tables(v1, set)).collect()
        } else {
            Vec::new()
        };
        // Leaf2Bits round-2 message tables: the [`Pair2Tables`] of the
        // stashed ρ₁-dependent value sets, weighted by V_2.
        let leaf2_tables: Vec<Pair2Tables<F>> = if j == 2 && (has_leaf2 || has_leaf3) {
            let _g = crate::utils::prof::scope("eqf:leaf2_tables");
            let v2 = &suffix[0][1];
            cfg_iter!(leaf2_value_sets).map(|set| build_pair2_tables(v2, set)).collect()
        } else {
            Vec::new()
        };

        // Per group `t` (independent): accumulate the suffix-weighted *coefficients*
        // of H_t in the round variable — A0 = Σ_b w_b·Σ_i L_i(0)·R_i(0), A1 = Σ_b
        // w_b·Σ_i (cross term), A2 = Σ_b w_b·Σ_i ΔL_i·ΔR_i — then convert to the four
        // node evaluations once below. Working in coefficients keeps the node-X (and
        // node-(X+1)) multiplies OUT of the per-`b` inner loop: each H value is
        // `H(c) = A0 + c·A1 + c²·A2`, exact since H is degree 2 in the round var, so
        // the GKR single-pair case costs 5 muls/`b` (weight folded into L) and the
        // multi-pair case drops the per-pair node-X muls. The three products that
        // feed only the accumulators go through [`WideMulAcc`] — for `GF(2^128)`
        // that defers the polynomial reduction to ONE per accumulator per round
        // (reduction is `F_2`-linear, so the reduced sums are bit-identical), and
        // the cross term becomes ±wide adds instead of a per-`b` chained
        // subtract. (A branchy skip-multiplies-by-one variant for the forest's
        // `{1, α^w}`-structured leaf layers was measured SLOWER — 50/50 random
        // bits make the branches mispredict-bound, costing about what the
        // skipped multiply saves — and was removed.) Parallel **across
        // groups**, with a minimum batch so tiny late-round bodies amortise
        // the rayon dispatch.
        let compute_h = |t: usize, bufs: &[GroupBufs<F>]| -> (F, F, F) {
            let suffix_t = &suffix[if shared_q { 0 } else { t }][j - 1];
            match &bufs[t] {
                GroupBufs::Dense(group_bufs) if group_bufs.len() == 1 => {
                    // Single pair (the GKR forest): fold the weight straight
                    // into L, so the weighted coefficients drop out with no
                    // separate `w·i`. A field's fused kernel (interleaved
                    // independent slot chains — value-exact) takes over when
                    // available.
                    let (l, r) = &group_bufs[0];
                    let kernel_res = if eqf_nokernel() {
                        None
                    } else {
                        F::eqf_single_pair_round(l, r, &suffix_t[..half], half)
                    };
                    if let Some(res) = kernel_res {
                        return res;
                    }
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        let w = &suffix_t[b];
                        let l0w = w.clone() * &l[b << 1];
                        let l1w = w.clone() * &l[(b << 1) | 1];
                        let (r0, r1) = (&r[b << 1], &r[(b << 1) | 1]);
                        let wc0 = F::mul_wide(&l0w, r0);
                        let w11 = F::mul_wide(&l1w, r1);
                        let dr = r1.clone() - r0;
                        let dl = l1w - &l0w;
                        let wc2 = F::mul_wide(&dl, &dr);
                        F::wide_add_assign(&mut a0, &wc0);
                        F::wide_add_assign(&mut a2, &wc2);
                        F::wide_add_assign(&mut a1, &w11);
                        F::wide_sub_assign(&mut a1, &wc0);
                        F::wide_sub_assign(&mut a1, &wc2);
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                }
                GroupBufs::Dense(group_bufs) => {
                    // Two pairs (the fraction-GKR layer combine): a field's
                    // fused kernel (per-pair PMULL chains, weight folded
                    // into each L side, deferred reduction) takes over when
                    // available — value-exact vs the generic loop below.
                    if let [(l0, r0), (l1, r1)] = group_bufs.as_slice() {
                        if let Some(res) = if eqf_nokernel() {
                            None
                        } else {
                            F::eqf_two_pair_round(l0, r0, l1, r1, &suffix_t[..half], half)
                        } {
                            return res;
                        }
                    }
                    // Multiple pairs (e.g. the lookup binding): sum
                    // coefficients over the pairs (wide), then weight by
                    // `w_b` once.
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        let mut i0 = F::wide_zero(&zero);
                        let mut i1 = F::wide_zero(&zero);
                        let mut i2 = F::wide_zero(&zero);
                        for (l, r) in group_bufs {
                            let (l0, l1) = (&l[b << 1], &l[(b << 1) | 1]);
                            let (r0, r1) = (&r[b << 1], &r[(b << 1) | 1]);
                            let wc0 = F::mul_wide(l0, r0);
                            let w11 = F::mul_wide(l1, r1);
                            let dr = r1.clone() - r0;
                            let dl = l1.clone() - l0;
                            let wc2 = F::mul_wide(&dl, &dr);
                            F::wide_add_assign(&mut i0, &wc0);
                            F::wide_add_assign(&mut i2, &wc2);
                            F::wide_add_assign(&mut i1, &w11);
                            F::wide_sub_assign(&mut i1, &wc0);
                            F::wide_sub_assign(&mut i1, &wc2);
                        }
                        let (i0, i1, i2) =
                            (F::from_wide(i0), F::from_wide(i1), F::from_wide(i2));
                        let w = &suffix_t[b];
                        F::wide_add_assign(&mut a0, &F::mul_wide(w, &i0));
                        F::wide_add_assign(&mut a1, &F::mul_wide(w, &i1));
                        F::wide_add_assign(&mut a2, &F::mul_wide(w, &i2));
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                }
                GroupBufs::Pair2Bits { lbits, rbits, tau_set } => {
                    // Bit-selected product layer, round 1 only: `a0` and the
                    // `Σ w·l1·r1` accumulator are ONE case-LUT load per slot;
                    // the ΔL·ΔR cross term is one wide multiply of two
                    // case-LUT loads. Same field values as the dense body
                    // (exact char-2 identities; reduction is F₂-linear).
                    debug_assert_eq!(j, 1, "pair-bit groups are consumed in round 1");
                    let h_off = 1usize << k; // O-side bit-position offset
                    match &pair2_tables[*tau_set] {
                        Pair2Tables::Factored { wte } => pair2_factored_body(
                            wte,
                            &pair_tau_sets[*tau_set].to,
                            half,
                            &zero,
                            |b| pair2_cases(lbits, rbits, b, h_off),
                        ),
                        Pair2Tables::Precombined { t_a0, t_a1, t_wde, t_do } => {
                            let mut a0 = zero.clone();
                            let mut t11 = zero.clone();
                            let mut a2w = F::wide_zero(&zero);
                            for b in 0..half {
                                let (ce0, ce1, co0, co1) =
                                    pair2_cases(lbits, rbits, b, h_off);
                                a0 += &t_a0[(b << 4) | (ce0 << 2) | co0];
                                t11 += &t_a1[(b << 4) | (ce1 << 2) | co1];
                                let wde = &t_wde[(b << 4) | (ce0 << 2) | ce1];
                                let dro = &t_do[(b << 4) | (co0 << 2) | co1];
                                F::wide_add_assign(&mut a2w, &F::mul_wide(wde, dro));
                            }
                            let a2 = F::from_wide(a2w);
                            let a1 = t11 + &a0 + &a2;
                            (a0, a1, a2)
                        }
                    }
                }
                GroupBufs::LeafBits { lbits, rbits, tau_set } => {
                    // Bit-affine leaf layer, round 1 only: each slot's
                    // contribution to `Σ w·l0·r0`, `Σ w·l1·r1` and `Σ w·ΔΔ`
                    // is ONE precombined case-LUT entry (see [`LeafTables`])
                    // selected by the committed bits — zero multiplications
                    // and three unconditional adds per slot. The sums are
                    // the exact same field elements the dense body
                    // accumulates (field addition is exact + commutative;
                    // in char 2, `a1 = Σw11 − Σwc0 − Σwc2 = t11 + a0 + a2`).
                    debug_assert_eq!(j, 1, "leaf-bit groups are consumed in round 1");
                    let lt = &leaf_tables[*tau_set];
                    let mut a0 = zero.clone();
                    let mut t11 = zero.clone();
                    let mut a2 = zero.clone();
                    for b in 0..half {
                        // Positions 2b, 2b+1 of the half share word b/32 at
                        // bit offset 2·(b mod 32).
                        let lp = ((lbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        let rp = ((rbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        a0 += &lt.t_a0[(b << 2) | (lp & 1) | ((rp & 1) << 1)];
                        t11 += &lt.t_a1[(b << 2) | (lp >> 1) | (rp & 2)];
                        leaf_a2_slot_add(&mut a2, &lt.a2, b, lp, rp, &zero);
                    }
                    a0 += &lt.w_sum;
                    t11 += &lt.w_sum;
                    let a1 = t11 + &a0 + &a2;
                    (a0, a1, a2)
                }
                GroupBufs::Leaf2Bits { lbits, rbits, tau_set } if j == 1 => {
                    // Round 1: identical to the LeafBits body (the same
                    // shared [`LeafTables`] — the leaves are the same
                    // implicit `1 + m·τ` values).
                    let lt = &leaf_tables[*tau_set];
                    let mut a0 = zero.clone();
                    let mut t11 = zero.clone();
                    let mut a2 = zero.clone();
                    for b in 0..half {
                        let lp = ((lbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        let rp = ((rbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        a0 += &lt.t_a0[(b << 2) | (lp & 1) | ((rp & 1) << 1)];
                        t11 += &lt.t_a1[(b << 2) | (lp >> 1) | (rp & 2)];
                        leaf_a2_slot_add(&mut a2, &lt.a2, b, lp, rp, &zero);
                    }
                    a0 += &lt.w_sum;
                    t11 += &lt.w_sum;
                    let a1 = t11 + &a0 + &a2;
                    (a0, a1, a2)
                }
                GroupBufs::Leaf2Bits { lbits, rbits, tau_set } => {
                    // Round 2: the Pair2Bits case-LUT round over the
                    // ρ₁-dependent 4-case value tables — the round-2
                    // entries are per-position selects keyed by adjacent
                    // original-bit pairs, so the slot cases are one
                    // aligned nibble load per side (see [`leaf2_cases`]).
                    // Same field values as folding dense round-2 buffers
                    // (exact char-2 identities; reduction is F₂-linear).
                    debug_assert_eq!(j, 2, "leaf2-bit groups are consumed in round 2");
                    match &leaf2_tables[*tau_set] {
                        Pair2Tables::Factored { wte } => pair2_factored_body(
                            wte,
                            &leaf2_value_sets[*tau_set].to,
                            half,
                            &zero,
                            |b| leaf2_cases(lbits, rbits, b),
                        ),
                        Pair2Tables::Precombined { t_a0, t_a1, t_wde, t_do } => {
                            let mut a0 = zero.clone();
                            let mut t11 = zero.clone();
                            let mut a2w = F::wide_zero(&zero);
                            for b in 0..half {
                                let (ce0, ce1, co0, co1) = leaf2_cases(lbits, rbits, b);
                                a0 += &t_a0[(b << 4) | (ce0 << 2) | co0];
                                t11 += &t_a1[(b << 4) | (ce1 << 2) | co1];
                                let wde = &t_wde[(b << 4) | (ce0 << 2) | ce1];
                                let dro = &t_do[(b << 4) | (co0 << 2) | co1];
                                F::wide_add_assign(&mut a2w, &F::mul_wide(wde, dro));
                            }
                            let a2 = F::from_wide(a2w);
                            let a1 = t11 + &a0 + &a2;
                            (a0, a1, a2)
                        }
                    }
                }
                GroupBufs::Leaf3Bits { lbits, rbits, tau_set } if j == 1 => {
                    // Round 1: the LeafBits body (same shared tables).
                    let lt = &leaf_tables[*tau_set];
                    let mut a0 = zero.clone();
                    let mut t11 = zero.clone();
                    let mut a2 = zero.clone();
                    for b in 0..half {
                        let lp = ((lbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        let rp = ((rbits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                        a0 += &lt.t_a0[(b << 2) | (lp & 1) | ((rp & 1) << 1)];
                        t11 += &lt.t_a1[(b << 2) | (lp >> 1) | (rp & 2)];
                        leaf_a2_slot_add(&mut a2, &lt.a2, b, lp, rp, &zero);
                    }
                    a0 += &lt.w_sum;
                    t11 += &lt.w_sum;
                    let a1 = t11 + &a0 + &a2;
                    (a0, a1, a2)
                }
                GroupBufs::Leaf3Bits { lbits, rbits, tau_set } if j == 2 => {
                    // Round 2: the Leaf2Bits body (same shared round-2
                    // tables over the same stashed value sets).
                    match &leaf2_tables[*tau_set] {
                        Pair2Tables::Factored { wte } => pair2_factored_body(
                            wte,
                            &leaf2_value_sets[*tau_set].to,
                            half,
                            &zero,
                            |b| leaf2_cases(lbits, rbits, b),
                        ),
                        Pair2Tables::Precombined { t_a0, t_a1, t_wde, t_do } => {
                            let mut a0 = zero.clone();
                            let mut t11 = zero.clone();
                            let mut a2w = F::wide_zero(&zero);
                            for b in 0..half {
                                let (ce0, ce1, co0, co1) = leaf2_cases(lbits, rbits, b);
                                a0 += &t_a0[(b << 4) | (ce0 << 2) | co0];
                                t11 += &t_a1[(b << 4) | (ce1 << 2) | co1];
                                let wde = &t_wde[(b << 4) | (ce0 << 2) | ce1];
                                let dro = &t_do[(b << 4) | (co0 << 2) | co1];
                                F::wide_add_assign(&mut a2w, &F::mul_wide(wde, dro));
                            }
                            let a2 = F::from_wide(a2w);
                            let a1 = t11 + &a0 + &a2;
                            (a0, a1, a2)
                        }
                    }
                }
                GroupBufs::Leaf3Bits { lbits, rbits, tau_set } => {
                    // Round 3: values INLINE from the stashed 16-case
                    // tables — one aligned byte per side per slot, then
                    // the dense single-pair body (multiplies are free;
                    // only skipped bytes pay). Same field values as the
                    // dense round over materialised round-3 buffers.
                    debug_assert_eq!(j, 3, "leaf3-bit groups are consumed in round 3");
                    let vs = &leaf3_value_sets[*tau_set];
                    let prfm = lut_prfm(half);
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        if prfm && b + PRFM_DIST < half {
                            let bp = b + PRFM_DIST;
                            let pp = bp << 3;
                            let plb = ((lbits[pp >> 6] >> (pp & 63)) & 255) as u32 as usize;
                            let prb = ((rbits[pp >> 6] >> (pp & 63)) & 255) as u32 as usize;
                            let ep = bp << 1;
                            prefetch_l1(&vs.f_e, (ep << 4) | leaf3_idx(plb & 15));
                            prefetch_l1(&vs.f_e, ((ep | 1) << 4) | leaf3_idx(plb >> 4));
                            prefetch_l1(&vs.f_o, (ep << 4) | leaf3_idx(prb & 15));
                            prefetch_l1(&vs.f_o, ((ep | 1) << 4) | leaf3_idx(prb >> 4));
                        }
                        let p = b << 3;
                        let bl = ((lbits[p >> 6] >> (p & 63)) & 255) as u32 as usize;
                        let br = ((rbits[p >> 6] >> (p & 63)) & 255) as u32 as usize;
                        let e = b << 1;
                        let l0 = &vs.f_e[(e << 4) | leaf3_idx(bl & 15)];
                        let l1 = &vs.f_e[((e | 1) << 4) | leaf3_idx(bl >> 4)];
                        let r0 = &vs.f_o[(e << 4) | leaf3_idx(br & 15)];
                        let r1 = &vs.f_o[((e | 1) << 4) | leaf3_idx(br >> 4)];
                        let w = &suffix_t[b];
                        let l0w = w.clone() * l0;
                        let l1w = w.clone() * l1;
                        let wc0 = F::mul_wide(&l0w, r0);
                        let w11 = F::mul_wide(&l1w, r1);
                        let dr = r1.clone() - r0;
                        let dl = l1w - &l0w;
                        let wc2 = F::mul_wide(&dl, &dr);
                        F::wide_add_assign(&mut a0, &wc0);
                        F::wide_add_assign(&mut a2, &wc2);
                        F::wide_add_assign(&mut a1, &w11);
                        F::wide_sub_assign(&mut a1, &wc0);
                        F::wide_sub_assign(&mut a1, &wc2);
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                }
                GroupBufs::Pair3Bits { lbits, rbits, tau_set } if j == 1 => {
                    // Round 1: the Pair2Bits body (same shared tables).
                    let h_off = 1usize << k;
                    match &pair2_tables[*tau_set] {
                        Pair2Tables::Factored { wte } => pair2_factored_body(
                            wte,
                            &pair_tau_sets[*tau_set].to,
                            half,
                            &zero,
                            |b| pair2_cases(lbits, rbits, b, h_off),
                        ),
                        Pair2Tables::Precombined { t_a0, t_a1, t_wde, t_do } => {
                            let mut a0 = zero.clone();
                            let mut t11 = zero.clone();
                            let mut a2w = F::wide_zero(&zero);
                            for b in 0..half {
                                let (ce0, ce1, co0, co1) =
                                    pair2_cases(lbits, rbits, b, h_off);
                                a0 += &t_a0[(b << 4) | (ce0 << 2) | co0];
                                t11 += &t_a1[(b << 4) | (ce1 << 2) | co1];
                                let wde = &t_wde[(b << 4) | (ce0 << 2) | ce1];
                                let dro = &t_do[(b << 4) | (co0 << 2) | co1];
                                F::wide_add_assign(&mut a2w, &F::mul_wide(wde, dro));
                            }
                            let a2 = F::from_wide(a2w);
                            let a1 = t11 + &a0 + &a2;
                            (a0, a1, a2)
                        }
                    }
                }
                GroupBufs::Pair3Bits { lbits, rbits, tau_set } => {
                    // Round 2: values inline from the stashed fold tables
                    // (entry keys = interleaved l/r bit pairs; one aligned
                    // nibble per array per side per slot).
                    debug_assert_eq!(j, 2, "pair3-bit groups are consumed in round 2");
                    let vs = &pair3_value_sets[*tau_set];
                    let h_off = half << 2; // 2^k — absolute O-side bit offset
                    let prfm = lut_prfm(half);
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        if prfm && b + PRFM_DIST < half {
                            let bp = b + PRFM_DIST;
                            let pe2 = bp << 2;
                            let nl2 = ((lbits[pe2 >> 6] >> (pe2 & 63)) & 15) as u32 as usize;
                            let nr2 = ((rbits[pe2 >> 6] >> (pe2 & 63)) & 15) as u32 as usize;
                            let po2 = pe2 + h_off;
                            let ml2 = ((lbits[po2 >> 6] >> (po2 & 63)) & 15) as u32 as usize;
                            let mr2 = ((rbits[po2 >> 6] >> (po2 & 63)) & 15) as u32 as usize;
                            let ep = bp << 1;
                            prefetch_l1(&vs.f_e, (ep << 4) | pair3_idx(nl2 & 3, nr2 & 3));
                            prefetch_l1(&vs.f_e, ((ep | 1) << 4) | pair3_idx(nl2 >> 2, nr2 >> 2));
                            prefetch_l1(&vs.f_o, (ep << 4) | pair3_idx(ml2 & 3, mr2 & 3));
                            prefetch_l1(&vs.f_o, ((ep | 1) << 4) | pair3_idx(ml2 >> 2, mr2 >> 2));
                        }
                        let pe = b << 2;
                        let nl = ((lbits[pe >> 6] >> (pe & 63)) & 15) as u32 as usize;
                        let nr = ((rbits[pe >> 6] >> (pe & 63)) & 15) as u32 as usize;
                        let po = pe + h_off;
                        let ml = ((lbits[po >> 6] >> (po & 63)) & 15) as u32 as usize;
                        let mr = ((rbits[po >> 6] >> (po & 63)) & 15) as u32 as usize;
                        let e = b << 1;
                        let l0 = &vs.f_e[(e << 4) | pair3_idx(nl & 3, nr & 3)];
                        let l1 = &vs.f_e[((e | 1) << 4) | pair3_idx(nl >> 2, nr >> 2)];
                        let r0 = &vs.f_o[(e << 4) | pair3_idx(ml & 3, mr & 3)];
                        let r1 = &vs.f_o[((e | 1) << 4) | pair3_idx(ml >> 2, mr >> 2)];
                        let w = &suffix_t[b];
                        let l0w = w.clone() * l0;
                        let l1w = w.clone() * l1;
                        let wc0 = F::mul_wide(&l0w, r0);
                        let w11 = F::mul_wide(&l1w, r1);
                        let dr = r1.clone() - r0;
                        let dl = l1w - &l0w;
                        let wc2 = F::mul_wide(&dl, &dr);
                        F::wide_add_assign(&mut a0, &wc0);
                        F::wide_add_assign(&mut a2, &wc2);
                        F::wide_add_assign(&mut a1, &w11);
                        F::wide_sub_assign(&mut a1, &wc0);
                        F::wide_sub_assign(&mut a1, &wc2);
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                }
                GroupBufs::T4Bits { lbits, rbits, tau_set } => {
                    // Round 1 only: inline T4 selects — the layer's input
                    // level is never stored nor regenerated (4 selects +
                    // the dense body per slot).
                    debug_assert_eq!(j, 1, "t4-bit groups are consumed in round 1");
                    let t4 = &t4_sets[*tau_set];
                    let q1 = 2usize << k; // T4 position count = 2^{k+1}
                    let h_off = 1usize << k; // O-side POSITION offset
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        let e = b << 1;
                        let l0 = &t4[t4bits_idx(lbits, rbits, e, q1)];
                        let l1 = &t4[t4bits_idx(lbits, rbits, e | 1, q1)];
                        let r0 = &t4[t4bits_idx(lbits, rbits, e + h_off, q1)];
                        let r1 = &t4[t4bits_idx(lbits, rbits, (e | 1) + h_off, q1)];
                        let w = &suffix_t[b];
                        let l0w = w.clone() * l0;
                        let l1w = w.clone() * l1;
                        let wc0 = F::mul_wide(&l0w, r0);
                        let w11 = F::mul_wide(&l1w, r1);
                        let dr = r1.clone() - r0;
                        let dl = l1w - &l0w;
                        let wc2 = F::mul_wide(&dl, &dr);
                        F::wide_add_assign(&mut a0, &wc0);
                        F::wide_add_assign(&mut a2, &wc2);
                        F::wide_add_assign(&mut a1, &w11);
                        F::wide_sub_assign(&mut a1, &wc0);
                        F::wide_sub_assign(&mut a1, &wc2);
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                }
            }
        };
        // Message pass — fused with the deferred fold when one is pending:
        // one pass reads the unfolded buffers, folds ρ_{j−1} in registers
        // into the prefix, and accumulates this round's coefficients from
        // the folded pairs — identical field values, identical transcript
        // order. Every path below yields per-group COEFFICIENT triples
        // `(A0, A1, A2)` of the quadratic `H_t`; the message combination at
        // the end converts them per format (Gruen: the combined triple is
        // the message; Generic: node evaluations).
        // A double-fold pass covers rounds (j, j+1); the grid is only
        // produced when round j+1 is NOT the last, so round k always runs
        // a real pass and the final interpolation never sees a deferred
        // fold.
        let double_now = eqf_double()
            && grid.is_none()
            && j + 1 < k
            && !(j == 1 && pre_round1.is_some())
            && bufs.iter().all(|gb| matches!(gb, GroupBufs::Dense(p) if p.len() == 1));
        let hs: Vec<(F, F, F)> = if let Some(g) = grid.take() {
            // Round j+1 of a double-fold pass: nine field elements per
            // group, evaluated at the challenge just drawn. No pass.
            let rho_prev = grid_rho.take().expect("grid follows its round's challenge");
            g.iter().map(|gg| grid_next_round(gg, &rho_prev)).collect()
        } else if double_now {
            let _g_msg = crate::utils::prof::scope("eqf:grid");
            let quads = half >> 1;
            let pend = core::mem::take(&mut pending);
            let pass = |t: usize, gb: &mut GroupBufs<F>| -> [F; 9] {
                let suffix_t = &suffix[if shared_q { 0 } else { t }][j];
                let GroupBufs::Dense(group_bufs) = gb else {
                    unreachable!("double-fold requires all-Dense single-pair groups")
                };
                let (l, r) = &mut group_bufs[0];
                dense_grid_pass(l, r, &pend, &suffix_t[..quads], quads, &zero)
            };
            #[cfg(feature = "parallel")]
            let out: Vec<[F; 9]> = {
                let min_len = (512usize / quads.max(1)).max(1);
                bufs.par_iter_mut()
                    .enumerate()
                    .with_min_len(min_len)
                    .map(|(t, gb)| pass(t, gb))
                    .collect()
            };
            #[cfg(not(feature = "parallel"))]
            let out: Vec<[F; 9]> =
                bufs.iter_mut().enumerate().map(|(t, gb)| pass(t, gb)).collect();
            let hs = out
                .iter()
                .enumerate()
                .map(|(t, g)| grid_this_round(g, &qs[if shared_q { 0 } else { t }][j], &one))
                .collect();
            grid = Some(out);
            hs
        } else if pending.len() == 2 {
            // Tail of a double-fold cascade: two deferred challenges, one
            // round's message.
            let _g_msg = crate::utils::prof::scope("eqf:fmsg2");
            let pend = core::mem::take(&mut pending);
            let pass = |t: usize, gb: &mut GroupBufs<F>| -> (F, F, F) {
                let suffix_t = &suffix[if shared_q { 0 } else { t }][j - 1];
                let GroupBufs::Dense(group_bufs) = gb else {
                    unreachable!("deferred folds require all-Dense single-pair groups")
                };
                let (l, r) = &mut group_bufs[0];
                dense_msg_pass_d(l, r, &pend, &suffix_t[..half], half, &zero)
            };
            #[cfg(feature = "parallel")]
            let out: Vec<(F, F, F)> = {
                let min_len = (512usize / half.max(1)).max(1);
                bufs.par_iter_mut()
                    .enumerate()
                    .with_min_len(min_len)
                    .map(|(t, gb)| pass(t, gb))
                    .collect()
            };
            #[cfg(not(feature = "parallel"))]
            let out: Vec<(F, F, F)> =
                bufs.iter_mut().enumerate().map(|(t, gb)| pass(t, gb)).collect();
            out
        } else if j == 1 && pre_round1.is_some() {
            // Round-1 work arrived precomputed (fused into the caller's
            // buffer generation): only the coefficient→node conversion
            // runs — the SAME conversion the message passes apply — and
            // the buffers stay untouched. Under the grid form, round 2
            // rides along for free too: the caller's single generation
            // pass has covered BOTH rounds, so the first time the driver
            // reads these buffers is round 3, already folding two
            // challenges.
            match pre_round1.take().expect("checked is_some") {
                PreRound::Coeffs(pre) => pre,
                PreRound::Grid(g) => {
                    assert!(k >= 3, "a pre-round grid needs a non-final round 2");
                    let hs: Vec<(F, F, F)> = g
                        .iter()
                        .enumerate()
                        .map(|(t, gg)| {
                            grid_this_round(gg, &qs[if shared_q { 0 } else { t }][1], &one)
                        })
                        .collect();
                    grid = Some(g);
                    hs
                }
            }
        } else if let Some(rho_prev) = pending.pop() {
            let _g_msg = crate::utils::prof::scope("eqf:fmsg");
            let fused = |t: usize, gb: &mut GroupBufs<F>| -> (F, F, F) {
                let suffix_t = &suffix[if shared_q { 0 } else { t }][j - 1];
                let GroupBufs::Dense(group_bufs) = gb else {
                    unreachable!("fused rounds require all-Dense single-pair groups")
                };
                let (l, r) = &mut group_bufs[0];
                debug_assert_eq!(l.len(), half << 2, "fused round reads unfolded buffers");
                // A field's hand-fused fold+round kernel takes over when
                // available (value-exact; writes the same folded prefix).
                let kernel = if eqf_nokernel() {
                    None
                } else {
                    F::eqf_fused_fold_round(l, r, &rho_prev, &suffix_t[..half], half)
                };
                let (a0, a1, a2) = if let Some(res) = kernel {
                    res
                } else {
                    let mut a0 = F::wide_zero(&zero);
                    let mut a1 = F::wide_zero(&zero);
                    let mut a2 = F::wide_zero(&zero);
                    for b in 0..half {
                        let base = b << 2;
                        // The deferred fold — the eager scalar fold's exact
                        // formula `v0 + ρ·(v1 − v0)`, in registers.
                        let fold1 = |v: &[F], i: usize| -> F {
                            let v0 = v[i].clone();
                            let d = v[i + 1].clone() - &v0;
                            v0 + &(rho_prev.clone() * &d)
                        };
                        let fl0 = fold1(l, base);
                        let fl1 = fold1(l, base + 2);
                        let fr0 = fold1(r, base);
                        let fr1 = fold1(r, base + 2);
                        // The dense single-pair message body over the folded pair.
                        let w = &suffix_t[b];
                        let l0w = w.clone() * &fl0;
                        let l1w = w.clone() * &fl1;
                        let wc0 = F::mul_wide(&l0w, &fr0);
                        let w11 = F::mul_wide(&l1w, &fr1);
                        let dr = fr1.clone() - &fr0;
                        let dl = l1w - &l0w;
                        let wc2 = F::mul_wide(&dl, &dr);
                        F::wide_add_assign(&mut a0, &wc0);
                        F::wide_add_assign(&mut a2, &wc2);
                        F::wide_add_assign(&mut a1, &w11);
                        F::wide_sub_assign(&mut a1, &wc0);
                        F::wide_sub_assign(&mut a1, &wc2);
                        // Land the folded values in the prefix — writes trail
                        // the reads, so in place is safe.
                        let e = b << 1;
                        l[e] = fl0;
                        l[e + 1] = fl1;
                        r[e] = fr0;
                        r[e + 1] = fr1;
                    }
                    (F::from_wide(a0), F::from_wide(a1), F::from_wide(a2))
                };
                l.truncate(half << 1);
                r.truncate(half << 1);
                (a0, a1, a2)
            };
            #[cfg(feature = "parallel")]
            let out: Vec<(F, F, F)> = {
                let min_len = (512usize / half.max(1)).max(1);
                bufs.par_iter_mut()
                    .enumerate()
                    .with_min_len(min_len)
                    .map(|(t, gb)| fused(t, gb))
                    .collect()
            };
            #[cfg(not(feature = "parallel"))]
            let out: Vec<(F, F, F)> =
                bufs.iter_mut().enumerate().map(|(t, gb)| fused(t, gb)).collect();
            out
        } else {
            // Diagnostic-only: split the aggregate `eqf:msg` bucket by round
            // shape (the LUT prefixes vs the dense kernels) — the labels are
            // resolved from the representative group, all forest groups
            // being same-variant.
            let msg_label = match (&bufs[0], j) {
                (GroupBufs::Dense(_), 1) => "eqf:msg:dense_r1",
                (GroupBufs::Dense(_), _) => "eqf:msg:dense_postlut",
                (GroupBufs::LeafBits { .. }, _) => "eqf:msg:leafbits",
                (GroupBufs::Pair2Bits { .. }, _) => "eqf:msg:pair2",
                (GroupBufs::Leaf2Bits { .. }, 1) | (GroupBufs::Leaf3Bits { .. }, 1) => {
                    "eqf:msg:leaf_r1"
                }
                (GroupBufs::Leaf2Bits { .. }, _) => "eqf:msg:leaf2_r2",
                (GroupBufs::Leaf3Bits { .. }, 2) => "eqf:msg:leaf3_r2",
                (GroupBufs::Leaf3Bits { .. }, _) => "eqf:msg:leaf3_r3",
                (GroupBufs::Pair3Bits { .. }, 1) => "eqf:msg:pair3_r1",
                (GroupBufs::Pair3Bits { .. }, _) => "eqf:msg:pair3_r2",
                (GroupBufs::T4Bits { .. }, _) => "eqf:msg:t4bits",
            };
            let _g_msg = crate::utils::prof::scope(msg_label);
            #[cfg(feature = "parallel")]
            let out: Vec<(F, F, F)> = {
                // ≥ ~512 element-pairs per task so late-round tiny bodies don't
                // drown in rayon dispatch overhead.
                let min_len = (512usize / half.max(1)).max(1);
                (0..num_groups)
                    .into_par_iter()
                    .with_min_len(min_len)
                    .map(|t| compute_h(t, &bufs))
                    .collect()
            };
            #[cfg(not(feature = "parallel"))]
            let out: Vec<(F, F, F)> =
                (0..num_groups).map(|t| compute_h(t, &bufs)).collect();
            out
        };

        let tail = if gruen {
            // Gruen format (shared q, asserted): the round polynomial is
            // P(X) = eq1(X; q[j−1]) · Ĥ(X) with Ĥ = Σ_t A_t·H_t quadratic;
            // send Ĥ's two non-constant monomial coefficients. The verifier
            // reconstructs Ĥ0 from the running claim via
            // S = P(0) + P(1) = Ĥ0 + q[j−1]·(Ĥ1 + Ĥ2) (any field: the
            // (1−q)Ĥ0 + qĤ0 cross terms collapse to Ĥ0).
            let qj = &qs[0][j - 1];
            let mut ch = (zero.clone(), zero.clone(), zero.clone());
            for (t, h) in hs.into_iter().enumerate() {
                let a = &a_scalars[t];
                ch.0 += a.clone() * &h.0;
                ch.1 += a.clone() * &h.1;
                ch.2 += a.clone() * &h.2;
            }
            if j == 1 {
                claimed_sum = ch.0.clone() + &(qj.clone() * &(ch.1.clone() + &ch.2));
            }
            vec![ch.1, ch.2]
        } else {
            // Generic format: M(c) = Σ_t A_t · eq1(c; q_t[j−1]) · H_t(c) at
            // the four nodes (per-group coefficient→node conversion, then
            // the eq1-weighted combination — value-identical to converting
            // inside each group body). In char 2 the fourth node is the
            // affine-flat sum H(0)+H(1)+H(X).
            let mut m = (zero.clone(), zero.clone(), zero.clone(), zero.clone());
            for (t, h) in hs.into_iter().enumerate() {
                let qj = &qs[t][j - 1];
                let e0 = one.clone() - qj;
                let e1 = qj.clone();
                let eq1_at =
                    |c: &F| -> F { e0.clone() * &(one.clone() - c) + &(e1.clone() * c) };
                let (a0, a1, a2) = h;
                let h0 = a0.clone();
                let h1 = a0.clone() + &a1 + &a2;
                let h2 = a0.clone() + &(c2.clone() * &a1) + &(c2sq.clone() * &a2);
                let h3 = if char2 {
                    h0.clone() + &h1 + &h2
                } else {
                    a0 + &(c3.clone() * &a1) + &(c3sq.clone() * &a2)
                };
                let a = &a_scalars[t];
                m.0 += a.clone() * &(e0.clone() * &h0);
                m.1 += a.clone() * &(e1.clone() * &h1);
                m.2 += a.clone() * &(eq1_at(&c2) * &h2);
                m.3 += a.clone() * &(eq1_at(&c3) * &h3);
            }
            if j == 1 {
                claimed_sum = m.0.clone() + &m.1;
            }
            vec![m.1, m.2, m.3]
        };
        transcript.absorb_random_field_slice(&tail, &mut buf);
        messages.push(ProverMsg(NatEvaluatedPolyWithoutConstant::new(tail)));

        let rho: F = transcript.get_field_challenge(field_cfg);
        transcript.absorb_random_field(&rho, &mut buf);
        // A grid produced this round is spent by the next one, at ρ_j.
        if grid.is_some() {
            grid_rho = Some(rho.clone());
        }

        // A_{t,j+1} = A_{t,j} · eq1(ρ_j; q_t[j−1]); fold all L, R at ρ_j.
        for (t, a) in a_scalars.iter_mut().enumerate() {
            let qj = &qs[t][j - 1];
            let e = (one.clone() - qj) * &(one.clone() - &rho) + &(qj.clone() * &rho);
            *a = a.clone() * &e;
        }
        if j < k {
            // Pass fusion (the default): defer this round's fold into the
            // next round's message pass when every group is Dense
            // single-pair (the forest shape after any LUT prefix rounds).
            // Stash bookkeeping below only matters for LUT groups, which
            // never reach here fused.
            if eqf_fuse_enabled()
                && bufs.iter().all(|gb| matches!(gb, GroupBufs::Dense(p) if p.len() == 1))
            {
                pending.push(rho.clone());
                randomness.push(rho);
                continue;
            }
            // Shared leaf fold tables (need ρ, so built here) — one per tau
            // set; every leaf group's fold is then two XOR-selects per entry.
            let leaf_fold_tables: Vec<LeafFoldTables<F>> =
                if j == 1 && (has_leaf || has_leaf2 || has_leaf3) {
                    cfg_iter!(tau_sets)
                        .map(|(tl, tr)| build_leaf_fold_tables(&rho, &one, tl, tr))
                        .collect()
                } else {
                    Vec::new()
                };
            let pair2_fold_tables: Vec<Pair2FoldTables<F>> = if j == 1 && (has_pair || has_pair3)
            {
                cfg_iter!(pair_tau_sets)
                    .map(|set| build_pair2_fold_tables(&rho, &one, set))
                    .collect()
            } else {
                Vec::new()
            };
            let leaf2_fold_tables: Vec<Pair2FoldTables<F>> =
                if j == 2 && (has_leaf2 || has_leaf3) {
                    cfg_iter!(leaf2_value_sets)
                        .map(|set| build_pair2_fold_tables(&rho, &one, set))
                        .collect()
                } else {
                    Vec::new()
                };
            // Fold every group's L,R at ρ. Parallel **across groups**; the
            // per-vector fold is sequential (the groups are the big dimension).
            let fold_group = |gb: &mut GroupBufs<F>| match gb {
                GroupBufs::Dense(group_bufs) => {
                    for (l, r) in group_bufs.iter_mut() {
                        // Fold each buffer in place: write index `b` is only ever
                        // read at the earlier iteration `b/2` (its parent), so
                        // overwriting `v[b]` after that read is safe and saves the
                        // per-round `collect()` allocation (large on the deep layers).
                        // A field's fused fold kernel takes over when available.
                        let fold_in_place = |v: &mut Vec<F>| {
                            if eqf_nokernel() || !F::eqf_fold_in_place(v.as_mut_slice(), &rho, half) {
                                for b in 0..half {
                                    let v0 = v[b << 1].clone();
                                    let diff = v[(b << 1) | 1].clone() - &v0;
                                    v[b] = v0 + &(rho.clone() * &diff);
                                }
                            }
                            v.truncate(half);
                        };
                        fold_in_place(l);
                        fold_in_place(r);
                    }
                }
                GroupBufs::LeafBits { lbits, rbits, tau_set } => {
                    // Materialise the dense round-2 buffers straight from the
                    // bits: `v' = 1 + m_0·(1+ρ)τ_0 + m_1·ρτ_1` — the exact
                    // field value of `v_0 + ρ(v_1 − v_0)` over the implicit
                    // leaves (char-2 identity), precombined per bit pair so
                    // each entry is ONE indexed load (see [`LeafFoldTables`]).
                    let ft = &leaf_fold_tables[*tau_set];
                    let build = |bits: &[u64], t: &[F]| -> Vec<F> {
                        (0..half)
                            .map(|b| {
                                let p2 =
                                    ((bits[b >> 5] >> ((b & 31) << 1)) & 3) as u32 as usize;
                                t[(b << 2) | p2].clone()
                            })
                            .collect()
                    };
                    let l = build(lbits, &ft.t_l);
                    let r = build(rbits, &ft.t_r);
                    *gb = GroupBufs::Dense(vec![(l, r)]);
                }
                GroupBufs::Pair2Bits { lbits, rbits, tau_set } => {
                    // Materialise the dense round-2 buffers from the bits via
                    // the 16-case fold tables: `v' = (1+ρ)v_0 + ρv_1` with
                    // both v's 2-bit selects (see [`Pair2FoldTables`]).
                    let ft = &pair2_fold_tables[*tau_set];
                    // Round 1: buffers had 2^k entries ⇒ O-offset 2^k bits.
                    let h_off = 2 * half;
                    let mut l = Vec::with_capacity(half);
                    let mut r = Vec::with_capacity(half);
                    for b in 0..half {
                        let (ce0, ce1, co0, co1) = pair2_cases(lbits, rbits, b, h_off);
                        l.push(ft.f_e[(b << 4) | (ce0 << 2) | ce1].clone());
                        r.push(ft.f_o[(b << 4) | (co0 << 2) | co1].clone());
                    }
                    *gb = GroupBufs::Dense(vec![(l, r)]);
                }
                GroupBufs::Leaf2Bits { lbits, rbits, tau_set } => {
                    if j == 1 {
                        // Round 1's fold keeps the bits: the fold tables
                        // built above ARE the round-2 value tables — they
                        // get stashed as `leaf2_value_sets` below.
                    } else {
                        // Round 2's fold materialises the dense round-3
                        // buffers straight from the bits via the 16-case
                        // fold tables over the ρ₁-dependent value sets:
                        // `v'' = (1+ρ₂)v'_0 + ρ₂v'_1` with both v's
                        // nibble-keyed selects (exact char-2 identity).
                        let ft = &leaf2_fold_tables[*tau_set];
                        let mut l = Vec::with_capacity(half);
                        let mut r = Vec::with_capacity(half);
                        for b in 0..half {
                            let (ce0, ce1, co0, co1) = leaf2_cases(lbits, rbits, b);
                            l.push(ft.f_e[(b << 4) | (ce0 << 2) | ce1].clone());
                            r.push(ft.f_o[(b << 4) | (co0 << 2) | co1].clone());
                        }
                        *gb = GroupBufs::Dense(vec![(l, r)]);
                    }
                }
                GroupBufs::Leaf3Bits { lbits, rbits, tau_set } => {
                    if j <= 2 {
                        // Rounds 1-2 keep the bits; the round-2 fold
                        // tables get stashed as `leaf3_value_sets` below.
                    } else {
                        // Round 3's fold: inline-materialise the dense
                        // round-4 buffers — `v' = v_0 + ρ₃(v_0 + v_1)`
                        // (the canonical one-multiply char-2 fold; equals
                        // `(1+ρ₃)v_0 + ρ₃v_1` exactly by distributivity)
                        // over byte-keyed selects.
                        let vs = &leaf3_value_sets[*tau_set];
                        let prfm = lut_prfm(half);
                        let mut l = Vec::with_capacity(half);
                        let mut r = Vec::with_capacity(half);
                        for b in 0..half {
                            if prfm && b + PRFM_DIST < half {
                                let bp = b + PRFM_DIST;
                                let pp = bp << 3;
                                let plb =
                                    ((lbits[pp >> 6] >> (pp & 63)) & 255) as u32 as usize;
                                let prb =
                                    ((rbits[pp >> 6] >> (pp & 63)) & 255) as u32 as usize;
                                let ep = bp << 1;
                                prefetch_l1(&vs.f_e, (ep << 4) | leaf3_idx(plb & 15));
                                prefetch_l1(&vs.f_e, ((ep | 1) << 4) | leaf3_idx(plb >> 4));
                                prefetch_l1(&vs.f_o, (ep << 4) | leaf3_idx(prb & 15));
                                prefetch_l1(&vs.f_o, ((ep | 1) << 4) | leaf3_idx(prb >> 4));
                            }
                            let p = b << 3;
                            let bl = ((lbits[p >> 6] >> (p & 63)) & 255) as u32 as usize;
                            let br = ((rbits[p >> 6] >> (p & 63)) & 255) as u32 as usize;
                            let e = b << 1;
                            let v0 = &vs.f_e[(e << 4) | leaf3_idx(bl & 15)];
                            let v1 = &vs.f_e[((e | 1) << 4) | leaf3_idx(bl >> 4)];
                            l.push(v0.clone() + &(rho.clone() * &(v0.clone() + v1)));
                            let u0 = &vs.f_o[(e << 4) | leaf3_idx(br & 15)];
                            let u1 = &vs.f_o[((e | 1) << 4) | leaf3_idx(br >> 4)];
                            r.push(u0.clone() + &(rho.clone() * &(u0.clone() + u1)));
                        }
                        *gb = GroupBufs::Dense(vec![(l, r)]);
                    }
                }
                GroupBufs::Pair3Bits { lbits, rbits, tau_set } => {
                    if j == 1 {
                        // Round 1's fold keeps the bits; its fold tables
                        // get stashed as `pair3_value_sets` below.
                    } else {
                        // Round 2's fold: inline-materialise the dense
                        // round-3 buffers from the stashed tables (same
                        // extraction as the round-2 message body), via the
                        // one-multiply fold `v_0 + ρ₂(v_0 + v_1)`.
                        let vs = &pair3_value_sets[*tau_set];
                        let h_off = half << 2; // 2^k at j = 2
                        let prfm = lut_prfm(half);
                        let mut l = Vec::with_capacity(half);
                        let mut r = Vec::with_capacity(half);
                        for b in 0..half {
                            if prfm && b + PRFM_DIST < half {
                                let bp = b + PRFM_DIST;
                                let pe2 = bp << 2;
                                let nl2 =
                                    ((lbits[pe2 >> 6] >> (pe2 & 63)) & 15) as u32 as usize;
                                let nr2 =
                                    ((rbits[pe2 >> 6] >> (pe2 & 63)) & 15) as u32 as usize;
                                let po2 = pe2 + h_off;
                                let ml2 =
                                    ((lbits[po2 >> 6] >> (po2 & 63)) & 15) as u32 as usize;
                                let mr2 =
                                    ((rbits[po2 >> 6] >> (po2 & 63)) & 15) as u32 as usize;
                                let ep = bp << 1;
                                prefetch_l1(&vs.f_e, (ep << 4) | pair3_idx(nl2 & 3, nr2 & 3));
                                prefetch_l1(
                                    &vs.f_e,
                                    ((ep | 1) << 4) | pair3_idx(nl2 >> 2, nr2 >> 2),
                                );
                                prefetch_l1(&vs.f_o, (ep << 4) | pair3_idx(ml2 & 3, mr2 & 3));
                                prefetch_l1(
                                    &vs.f_o,
                                    ((ep | 1) << 4) | pair3_idx(ml2 >> 2, mr2 >> 2),
                                );
                            }
                            let pe = b << 2;
                            let nl = ((lbits[pe >> 6] >> (pe & 63)) & 15) as u32 as usize;
                            let nr = ((rbits[pe >> 6] >> (pe & 63)) & 15) as u32 as usize;
                            let po = pe + h_off;
                            let ml = ((lbits[po >> 6] >> (po & 63)) & 15) as u32 as usize;
                            let mr = ((rbits[po >> 6] >> (po & 63)) & 15) as u32 as usize;
                            let e = b << 1;
                            let v0 = &vs.f_e[(e << 4) | pair3_idx(nl & 3, nr & 3)];
                            let v1 = &vs.f_e[((e | 1) << 4) | pair3_idx(nl >> 2, nr >> 2)];
                            l.push(v0.clone() + &(rho.clone() * &(v0.clone() + v1)));
                            let u0 = &vs.f_o[(e << 4) | pair3_idx(ml & 3, mr & 3)];
                            let u1 = &vs.f_o[((e | 1) << 4) | pair3_idx(ml >> 2, mr >> 2)];
                            r.push(u0.clone() + &(rho.clone() * &(u0.clone() + u1)));
                        }
                        *gb = GroupBufs::Dense(vec![(l, r)]);
                    }
                }
                GroupBufs::T4Bits { lbits, rbits, tau_set } => {
                    // Round 1's fold: inline-materialise the dense round-2
                    // buffers from T4 selects, via the one-multiply fold
                    // `v_0 + ρ(v_0 + v_1)`.
                    let t4 = &t4_sets[*tau_set];
                    let q1 = 4 * half; // 2^{k+1}
                    let h_off = 2 * half; // 2^k
                    let mut l = Vec::with_capacity(half);
                    let mut r = Vec::with_capacity(half);
                    for b in 0..half {
                        let e = b << 1;
                        let v0 = &t4[t4bits_idx(lbits, rbits, e, q1)];
                        let v1 = &t4[t4bits_idx(lbits, rbits, e | 1, q1)];
                        l.push(v0.clone() + &(rho.clone() * &(v0.clone() + v1)));
                        let u0 = &t4[t4bits_idx(lbits, rbits, e + h_off, q1)];
                        let u1 = &t4[t4bits_idx(lbits, rbits, (e | 1) + h_off, q1)];
                        r.push(u0.clone() + &(rho.clone() * &(u0.clone() + u1)));
                    }
                    *gb = GroupBufs::Dense(vec![(l, r)]);
                }
            };
            // Diagnostic-only: split folds by shape — bit-keeping stashes vs
            // the LUT→Dense materialising folds vs plain dense folds.
            let fold_label = match (&bufs[0], j) {
                (GroupBufs::Dense(_), _) => "eqf:fold:dense",
                (GroupBufs::LeafBits { .. }, _) => "eqf:fold:leafmat",
                (GroupBufs::Pair2Bits { .. }, _) => "eqf:fold:pair2mat",
                (GroupBufs::Leaf2Bits { .. }, 1) | (GroupBufs::Leaf3Bits { .. }, 1 | 2) => {
                    "eqf:fold:stash"
                }
                (GroupBufs::Leaf2Bits { .. }, _) => "eqf:fold:leaf2mat",
                (GroupBufs::Leaf3Bits { .. }, _) => "eqf:fold:leaf3mat",
                (GroupBufs::Pair3Bits { .. }, 1) => "eqf:fold:stash",
                (GroupBufs::Pair3Bits { .. }, _) => "eqf:fold:pair3mat",
                (GroupBufs::T4Bits { .. }, _) => "eqf:fold:t4mat",
            };
            let _g_fold = crate::utils::prof::scope(fold_label);
            #[cfg(feature = "parallel")]
            {
                let min_len = (512usize / half.max(1)).max(1);
                bufs.par_iter_mut().with_min_len(min_len).for_each(fold_group);
            }
            #[cfg(not(feature = "parallel"))]
            bufs.iter_mut().for_each(fold_group);
            drop(_g_fold);
            if has_leaf2 || has_leaf3 {
                if j == 1 {
                    // Stash round 1's fold tables as the round-2 value
                    // sets: `t_l[(p≪2)|case]` is exactly the folded entry
                    // at position `p`, and its layout is a
                    // [`Pair2TauSet`] (4·2^{k−1} entries per side).
                    leaf2_value_sets = leaf_fold_tables
                        .into_iter()
                        .map(|ft| Pair2TauSet { te: ft.t_l, to: ft.t_r })
                        .collect();
                } else if j == 2 {
                    if has_leaf3 {
                        // Round 2's fold tables ARE the round-3 value
                        // tables (16-case per position, ρ₁ρ₂-dependent).
                        leaf3_value_sets = leaf2_fold_tables;
                    }
                    leaf2_value_sets = Vec::new();
                } else if j == 3 && has_leaf3 {
                    // All Leaf3Bits groups materialised — free the sets.
                    leaf3_value_sets = Vec::new();
                }
            }
            if has_pair3 {
                if j == 1 {
                    // Round 1's fold tables ARE the Pair3Bits round-2
                    // value tables.
                    pair3_value_sets = pair2_fold_tables;
                } else if j == 2 {
                    pair3_value_sets = Vec::new();
                }
            }
            randomness.push(rho);
        } else {
            // Final interpolation of every pair at ρ_k. (Leaf-bit groups
            // materialised at the round-1 fold — `k ≥ 2` is asserted — so
            // only Dense groups reach here.)
            let interp = |v: &Vec<F>| -> F {
                v[0].clone() + &(rho.clone() * &(v[1].clone() - &v[0]))
            };
            let final_evals: Vec<Vec<(F, F)>> = bufs
                .iter()
                .map(|gb| match gb {
                    GroupBufs::Dense(group_bufs) => {
                        group_bufs.iter().map(|(l, r)| (interp(l), interp(r))).collect()
                    }
                    GroupBufs::LeafBits { .. }
                    | GroupBufs::Pair2Bits { .. }
                    | GroupBufs::Leaf2Bits { .. }
                    | GroupBufs::Leaf3Bits { .. }
                    | GroupBufs::Pair3Bits { .. }
                    | GroupBufs::T4Bits { .. } => {
                        unreachable!("bit-selected groups materialise at their fold (k asserts)")
                    }
                })
                .collect();
            randomness.push(rho);
            return (SumcheckProof { messages, claimed_sum }, randomness, final_evals);
        }
    }
    unreachable!("the final round returns")
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
    use crypto_primitives::Field;

    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }

    /// MLE evaluation by sequential low-variable folds (char-2: `−` = `+`).
    fn mle_fold(v: &[Gf], r: &[Gf]) -> Gf {
        let mut cur = v.to_vec();
        for rho in r {
            cur = (0..cur.len() / 2)
                .map(|i| cur[2 * i] + *rho * (cur[2 * i + 1] + cur[2 * i]))
                .collect();
        }
        cur[0]
    }

    /// Gruen-format roundtrip: shared-point Dense groups (single- and
    /// two-pair, several scales) prove under
    /// [`prove_eq_inner_sumcheck_mixed_gruen`]; the claimed sum equals the
    /// directly computed eq-weighted sum, [`verify_eq_inner_sumcheck_gruen`]
    /// replays the challenges and its subclaim closes on the true MLE
    /// evaluations; malformed shapes (a Generic-format 3-element round, a
    /// dropped round) are rejected.
    #[test]
    fn gruen_roundtrip_and_shape_rejection() {
        use crate::transcript::Blake3Transcript;
        for (k, pair_counts) in
            [(1usize, vec![1usize]), (2, vec![1, 1, 1]), (5, vec![1, 2]), (6, vec![2])]
        {
            let n = 1usize << k;
            let q: Vec<Gf> = (0..k).map(|i| sample(0x4100 + (k * 31 + i) as u64)).collect();
            let mk = |seed: u64| -> Vec<Gf> { (0..n).map(|i| sample(seed + i as u64)).collect() };
            let dense: Vec<(Gf, Vec<(Vec<Gf>, Vec<Gf>)>)> = pair_counts
                .iter()
                .enumerate()
                .map(|(t, &np)| {
                    let scale = sample(0x4200 + t as u64);
                    let pairs = (0..np)
                        .map(|i| {
                            let base = 0x4300 + ((t * 8 + i) * 4 * n) as u64;
                            (mk(base), mk(base + n as u64))
                        })
                        .collect();
                    (scale, pairs)
                })
                .collect();
            let groups: Vec<EqInnerGroupMixed<Gf>> = dense
                .iter()
                .map(|(scale, pairs)| EqInnerGroupMixed {
                    q: q.clone(),
                    scale: *scale,
                    bufs: GroupBufs::Dense(pairs.clone()),
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let (proof, r, finals) =
                prove_eq_inner_sumcheck_mixed_gruen(&mut pt, groups, &[], &[], &[], &());
            assert!(proof.messages.iter().all(|m| m.0.tail_evaluations.len() == 2));

            // The claimed sum is the actual eq-weighted sum.
            let eq_at = |i: usize, pt_: &[Gf]| -> Gf {
                (0..k).fold(Gf::one(), |a, b| {
                    a * if (i >> b) & 1 == 1 { pt_[b] } else { Gf::one() + pt_[b] }
                })
            };
            let want_sum = dense.iter().fold(Gf::zero(), |acc, (scale, pairs)| {
                (0..n).fold(acc, |a, i| {
                    let inner = pairs
                        .iter()
                        .fold(Gf::zero(), |s, (l, rr)| s + l[i] * rr[i]);
                    a + *scale * eq_at(i, &q) * inner
                })
            });
            assert_eq!(proof.claimed_sum, want_sum, "claimed sum k={k}");

            // Verify: same challenges, subclaim closes on the true MLE evals.
            let mut vt = Blake3Transcript::new();
            let sub = verify_eq_inner_sumcheck_gruen(&mut vt, &q, &proof, &()).expect("verify");
            assert_eq!(sub.point, r, "challenge replay k={k}");
            let eq_rq = (0..k).fold(Gf::one(), |a, i| {
                a * (r[i] * q[i] + (Gf::one() + r[i]) * (Gf::one() + q[i]))
            });
            let want_eval = dense.iter().zip(finals.iter()).fold(
                Gf::zero(),
                |a, ((scale, pairs), fin)| {
                    let inner = pairs.iter().zip(fin.iter()).fold(
                        Gf::zero(),
                        |s, ((l, rr), &(fl, fr))| {
                            assert_eq!(fl, mle_fold(l, &r), "final L k={k}");
                            assert_eq!(fr, mle_fold(rr, &r), "final R k={k}");
                            s + fl * fr
                        },
                    );
                    a + *scale * eq_rq * inner
                },
            );
            assert_eq!(sub.expected_evaluation, want_eval, "subclaim closes k={k}");

            // Shape rejections: a Generic-format (3-element) round message,
            // and a dropped round.
            let mut bad = proof.clone();
            bad.messages[0].0.tail_evaluations.push(Gf::one());
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_eq_inner_sumcheck_gruen(&mut vt, &q, &bad, &()).is_err(),
                "3-element round must be rejected (k={k})"
            );
            let mut bad = proof.clone();
            bad.messages.pop();
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_eq_inner_sumcheck_gruen(&mut vt, &q, &bad, &()).is_err(),
                "dropped round must be rejected (k={k})"
            );
        }
    }

    /// The two [`LeafA2`] forms accumulate the identical ΔΔ value for every
    /// `(lp, rp)` case (env-independent — both arms constructed directly).
    #[test]
    fn leaf_a2_forms_agree() {
        let zero = Gf::zero();
        let slots = 5usize;
        let prods: Vec<Gf> = (0..slots << 2).map(|i| sample(0x7A00 + i as u64)).collect();
        let mut pre = Vec::with_capacity(slots << 4);
        for b in 0..slots {
            let (p00, p10, p01, p11) =
                (prods[b << 2], prods[(b << 2) | 1], prods[(b << 2) | 2], prods[(b << 2) | 3]);
            for c in 0..16usize {
                let mut v = zero;
                if c & 0b0101 == 0b0101 {
                    v += p00;
                }
                if c & 0b0110 == 0b0110 {
                    v += p10;
                }
                if c & 0b1001 == 0b1001 {
                    v += p01;
                }
                if c & 0b1010 == 0b1010 {
                    v += p11;
                }
                pre.push(v);
            }
        }
        let precombined = LeafA2::Precombined(pre);
        let factored = LeafA2::Factored(prods);
        for b in 0..slots {
            for lp in 0..4usize {
                for rp in 0..4usize {
                    let mut x = sample(0x8B00 + ((b << 4) | (lp << 2) | rp) as u64);
                    let mut y = x;
                    leaf_a2_slot_add(&mut x, &precombined, b, lp, rp, &zero);
                    leaf_a2_slot_add(&mut y, &factored, b, lp, rp, &zero);
                    assert_eq!(x, y, "slot {b} case ({lp},{rp})");
                }
            }
        }
    }
}
