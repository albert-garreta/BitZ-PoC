//! Structured-tap virtual claims (EXPERIMENTAL — `docs/rlc-structured-taps-phase0.md`).
//!
//! A *tap* applies an index-structured `F₂`-linear op to a committed UAIR
//! column: a cyclic bit-rotation `ROT^r` (bit `j` of the output = bit
//! `(j − r) mod W` of the input), a logical bit-shift `SHIFT^r` (same map,
//! bits drop out instead of wrapping), and/or an entry offset `off^o`
//! along the trace axis (`out[row] = in[row − o]`, zero for `row < o`).
//! A tapped virtual vector is a per-position XOR of tap streams; its bit
//! rows are extractable from the committed rows ([`extract_virtual_tap_rows`])
//! and its residual bit-MLE claims reduce to *committed* openings against
//! a **translated-eq basis**: the weight of source position `y` is
//! `eq(ζ, y + Δ)·[valid]`, which is *not* an eq tensor (index translation
//! is not a coordinate permutation — the Phase-0 note records the
//! counterexample) but factors as a **carry matrix product of bond
//! dimension 2** (binary addition, LSB→MSB). This module holds the
//! machinery all tap surfaces share:
//!
//! * the per-class *in-pack tables* `A_β(v)` (the ring-switch claim check),
//! * the per-class *support factor tables* (translated slices of plain eq
//!   tables) that drive the prover's sparse `s_v` walks and basis fills,
//! * the per-class *closure descriptor* + the matrix-product-state
//!   generalization of [`crate::ligerito::residual_b_evals`] (the
//!   verifier's succinct `O(m·128²)` basis evaluation).
//!
//! Classes: the pack cut (committed coordinate 7) and the clear/fold
//! boundary of the trace axis are crossed by at most the trace carry
//! chain; the weight splits as `K̃(v, y) = Σ_β A_β(v)·B_β(y)` over the
//! reachable `β = (γ, c₇)` — `γ` the trace carry at the `s`-bit boundary,
//! `c₇` the carry at the pack cut — at most **3** classes (`(0,0)`,
//! `(1,0)`, `(1,1)`), exactly one nonzero per position. Scope asserts
//! (v1): `x_fold_extra = 0`, `tw + log_cols ≥ 7` (the bit chain never
//! crosses the pack cut), `off < 2^s`.

use crate::pcs::ShaF2Layout;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;
use crate::utils::cfg_into_iter;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// One structured tap: an `F₂`-linear, index-structured operand applied
/// to a committed UAIR column. The identity tap is
/// `{ col, bit_amt: 0, bit_dropout: false, off: 0 }`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TapOp {
    /// Committed UAIR column index (`< layout.num_cols`).
    pub col: usize,
    /// Bit-axis amount `r ∈ [0, W)`: output bit `j` reads input bit
    /// `(j − r) mod W` (rotation) or `j − r` with dropout (shift).
    pub bit_amt: usize,
    /// `false` = `ROT` (cyclic); `true` = `SHIFT` (input bits `≥ W − r`
    /// drop out, output bits `< r` are zero).
    pub bit_dropout: bool,
    /// Entry offset along the trace axis: output row `k` reads input row
    /// `k − off` (zero for `k < off`). Must be `< 2^s` in v1.
    pub off: usize,
}

impl TapOp {
    /// The identity tap on `col` (a plain virtual-column term).
    pub fn ident(col: usize) -> Self {
        Self { col, bit_amt: 0, bit_dropout: false, off: 0 }
    }

    /// Whether this tap is the identity op (plain column slice).
    pub fn is_ident(&self) -> bool {
        self.bit_amt == 0 && !self.bit_dropout && self.off == 0
    }
}

/// Assert the v1 support envelope for tap claims on this layout. The
/// measurement shapes (`log_cols = 1..2`, `tw ≥ 6`) all satisfy it; the
/// boundary is the pack cut falling inside the bit chain (`tw + log_cols
/// < 7`), which would add bit-chain cut classes — not built.
pub(crate) fn assert_tap_layout(layout: &ShaF2Layout) {
    assert_eq!(layout.p.word_bits, 1, "tap claims assume the W=1 SHA layout");
    assert_eq!(layout.x_fold_extra, 0, "tap claims do not support x_fold_extra yet");
    assert!(
        layout.tw + layout.log_cols >= 7,
        "tap claims need tw + log_cols ≥ 7 (bit chain clear of the pack cut); got {} + {}",
        layout.tw,
        layout.log_cols
    );
    assert!(layout.bit_vars >= 1, "tap claims need at least one bit-position variable");
}

/// Validate one tap against the layout.
pub(crate) fn assert_tap(layout: &ShaF2Layout, tap: &TapOp) {
    let w = 1usize << layout.bit_vars;
    assert!(tap.col < layout.num_cols, "tap column {} out of range", tap.col);
    assert!(tap.bit_amt < w, "tap bit amount {} out of range (< {w})", tap.bit_amt);
    assert!(
        tap.off < (1usize << layout.p.s),
        "tap offset {} out of range (< 2^s = {}) — larger offsets not built",
        tap.off,
        1usize << layout.p.s
    );
}

// ---------------------------------------------------------------------
// Extraction: tapped virtual rows from the committed rows
// ---------------------------------------------------------------------

/// XOR the source run `src[src_off .. src_off + len)` (bit offsets into
/// `src`) into `dst[dst_off .. dst_off + len)`, shifted `sh` bits toward
/// higher positions: `dst bit (dst_off + t) ^= src bit (src_off + t − sh)`
/// for `t ∈ [sh, len)`. Offsets and `len` are multiples of 64 when
/// `len ≥ 64` (word path); below that, offsets are multiples of `len` and
/// runs sit inside one word on both sides (the extraction's run geometry).
#[allow(clippy::arithmetic_side_effects)]
fn xor_run_shifted(dst: &mut [u64], dst_off: usize, src: &[u64], src_off: usize, len: usize, sh: usize) {
    if sh >= len {
        return;
    }
    if len >= 64 {
        debug_assert!(
            dst_off.is_multiple_of(64) && src_off.is_multiple_of(64) && len.is_multiple_of(64)
        );
        let nw = len >> 6;
        let dw = dst_off >> 6;
        let sw = src_off >> 6;
        let wsh = sh >> 6;
        let bsh = sh & 63;
        if bsh == 0 {
            for d in wsh..nw {
                dst[dw + d] ^= src[sw + d - wsh];
            }
        } else {
            for d in wsh..nw {
                let hi = src[sw + d - wsh];
                let lo = if d > wsh { src[sw + d - wsh - 1] } else { 0 };
                dst[dw + d] ^= (hi << bsh) | (lo >> (64 - bsh));
            }
        }
    } else {
        let mask = (1u64 << len) - 1;
        let bits = (src[src_off >> 6] >> (src_off & 63)) & mask;
        dst[dst_off >> 6] ^= ((bits << sh) & mask) << (dst_off & 63);
    }
}

/// Extract the per-clear-row bit rows of the tapped virtual vector
/// `x = (⊕_taps op(col)) ⊕ constant-pattern` in the x layout (`2^s` rows
/// of `2^{t'}` bits, `t' = bit_vars + tw`). The committed code is
/// `F₂`-linear, so no new commitment. The identity tap reproduces
/// [`crate::pcs::extract_virtual_xor_rows`]'s base case.
#[allow(clippy::arithmetic_side_effects)]
pub fn extract_virtual_tap_rows(
    layout: &ShaF2Layout,
    rows: &[Vec<u64>],
    taps: &[TapOp],
    constant: u128,
) -> Vec<Vec<u64>> {
    assert_tap_layout(layout);
    for tap in taps {
        assert_tap(layout, tap);
    }
    assert!(!taps.is_empty() || constant != 0, "tap claim needs at least one term");
    let tw = layout.tw;
    let lc = layout.log_cols;
    let bv = layout.bit_vars;
    let s = layout.p.s;
    let w = 1usize << bv;
    let n_lo = 1usize << s;
    let run = 1usize << tw;
    let t_base = bv + tw;
    let base_words = (1usize << t_base).div_ceil(64);
    if bv < 7 {
        assert!(constant < (1u128 << w), "constant pattern wider than the word");
    }
    cfg_into_iter!(0..n_lo)
        .map(|c| {
            let mut out = vec![0u64; base_words];
            for tap in taps {
                // Source clear row and the row_hi borrow for this output row.
                let (src_c, borrow) =
                    if c >= tap.off { (c - tap.off, 0usize) } else { (c + n_lo - tap.off, 1usize) };
                if borrow >= run {
                    continue; // whole row out of range (tw = 0 edge)
                }
                let row = &rows[src_c];
                for jj in 0..w {
                    let src_j = if tap.bit_dropout {
                        if jj < tap.bit_amt {
                            continue;
                        }
                        jj - tap.bit_amt
                    } else {
                        (jj + w - tap.bit_amt) & (w - 1)
                    };
                    let src_off = (src_j << (lc + tw)) | (tap.col << tw);
                    let dst_off = jj << tw;
                    xor_run_shifted(&mut out, dst_off, row, src_off, run, borrow);
                }
            }
            for jj in 0..w {
                if (constant >> jj) & 1 == 1 {
                    let dst = jj << tw;
                    if tw >= 6 {
                        for wd in 0..1usize << (tw - 6) {
                            out[(dst >> 6) + wd] ^= !0u64;
                        }
                    } else {
                        let mask = (1u64 << run) - 1;
                        out[dst >> 6] ^= mask << (dst & 63);
                    }
                }
            }
            out
        })
        .collect()
}

// ---------------------------------------------------------------------
// Classes and the per-class weight tables
// ---------------------------------------------------------------------

/// One bond class of a tap opening's committed-side weight split
/// `K̃(v, y) = Σ_β A_β(v)·B_β(y)`: `gamma` = the trace-chain carry at the
/// clear/fold (`s`-bit) boundary, `cut` = the carry at the pack cut
/// (committed coordinate 7; only live when `tw > 7`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TapClass {
    pub gamma: u8,
    pub cut: u8,
}

/// The reachable classes of a tap on this layout, in canonical order.
/// `(γ, c₇) ∈ {(0,0)} ∪ {(1,0), (1,1) if off > 0}`; `(1,1)` needs the cut
/// inside the row_hi chain (`tw > 7`). Both sides derive the same list.
pub(crate) fn tap_classes(layout: &ShaF2Layout, tap: &TapOp) -> Vec<TapClass> {
    let mut v = vec![TapClass { gamma: 0, cut: 0 }];
    if tap.off > 0 {
        v.push(TapClass { gamma: 1, cut: 0 });
        if layout.tw > 7 {
            v.push(TapClass { gamma: 1, cut: 1 });
        }
    }
    v
}

/// The class's in-pack table `A_β(v)`, `v ∈ [0, 128)` over the committed
/// in-pack coordinates (`row_hi` bits `0..min(tw,7)`, then — when
/// `tw < 7` — the low `7 − tw` column-index bits, pinned to `tap.col`):
/// the trace-chain product with carry-in `γ` at `row_hi` bit 0, ending in
/// carry `cut` at the pack cut (`tw > 7`) or in the trace validity
/// (carry 0 out of `row_hi`'s top, `tw ≤ 7`). `pt_x` is the x-layout
/// exit point (`t' + s` coordinates; `row_hi` coords are `pt_x[..tw]`).
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn tap_inpack_table(
    layout: &ShaF2Layout,
    tap: &TapOp,
    pt_x: &[Gf],
    class: TapClass,
) -> Vec<Gf> {
    let one = Gf::one();
    let tw = layout.tw;
    let hi_bits = tw.min(7);
    let mut out = vec![Gf::zero(); 128];
    for (v, slot) in out.iter_mut().enumerate() {
        if tw < 7 {
            let npin = 7 - tw; // = min(7 − tw, log_cols): the layout assert gives lc ≥ 7 − tw
            let vcol = (v >> tw) & ((1usize << npin) - 1);
            if vcol != tap.col & ((1usize << npin) - 1) {
                continue;
            }
        }
        let mut c = class.gamma;
        let mut acc = one;
        for (i, &h) in pt_x.iter().enumerate().take(hi_bits) {
            let y = ((v >> i) & 1) as u8;
            let z = y ^ c;
            acc *= if z == 1 { h } else { one + h };
            c &= y; // maj(y, 0, c)
        }
        let ok = if tw <= 7 { c == 0 } else { c == class.cut };
        if ok {
            *slot = acc;
        }
    }
    out
}

/// The class's out-pack support factors: translated slices of the plain
/// eq tables over the three source coordinate segments above the pack
/// cut. The support index is `yx = (row_lo' ≪ (bv + hs)) | (j' ≪ hs) | hi`
/// with `hs = max(tw − 7, 0)` — exactly the `x-index ≫ p0` order of the
/// deployed sparse walks — and the flat weight is
/// `B_β(yx) = t_lo[row_lo']·t_bit[j']·t_hi[hi]`, zero outside `lo_band`.
pub(crate) struct TapSupportTables {
    /// Row_hi continuation factor (`2^{hs}` entries; `[1]` when `tw ≤ 7`).
    pub t_hi: Vec<Gf>,
    /// Bit-axis factor (`2^{bit_vars}` entries).
    pub t_bit: Vec<Gf>,
    /// Trace clear-axis factor (`2^s` entries).
    pub t_lo: Vec<Gf>,
    /// Nonzero `row_lo'` band `[start, end)` (the γ-branch support).
    pub lo_band: (usize, usize),
    /// `hs = max(tw − 7, 0)` — the row_hi coordinate count above the cut.
    pub hs: usize,
}

#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn tap_support_tables(
    layout: &ShaF2Layout,
    tap: &TapOp,
    pt_x: &[Gf],
    class: TapClass,
) -> TapSupportTables {
    let tw = layout.tw;
    let bv = layout.bit_vars;
    let s = layout.p.s;
    let w = 1usize << bv;
    let n_lo = 1usize << s;
    let hs = tw.saturating_sub(7);
    let t_hi: Vec<Gf> = if hs == 0 {
        vec![Gf::one()]
    } else {
        let eq_hi = build_eq_x_r_vec(&pt_x[7..tw], &()).expect("tw > 7");
        let c = class.cut as usize;
        (0..1usize << hs)
            .map(|h| if h + c < (1usize << hs) { eq_hi[h + c] } else { Gf::zero() })
            .collect()
    };
    let eq_b = build_eq_x_r_vec(&pt_x[tw..tw + bv], &()).expect("bit_vars >= 1");
    let t_bit: Vec<Gf> = (0..w)
        .map(|j| {
            if tap.bit_dropout {
                if j + tap.bit_amt < w { eq_b[j + tap.bit_amt] } else { Gf::zero() }
            } else {
                eq_b[(j + tap.bit_amt) & (w - 1)]
            }
        })
        .collect();
    let eq_lo = build_eq_x_r_vec(&pt_x[tw + bv..], &()).expect("s >= 1");
    let (lo_start, lo_end) =
        if class.gamma == 0 { (0, n_lo - tap.off) } else { (n_lo - tap.off, n_lo) };
    let t_lo: Vec<Gf> = (0..n_lo)
        .map(|x| {
            if x < lo_start || x >= lo_end {
                Gf::zero()
            } else if class.gamma == 0 {
                eq_lo[x + tap.off]
            } else {
                eq_lo[x + tap.off - n_lo]
            }
        })
        .collect();
    TapSupportTables { t_hi, t_bit, t_lo, lo_band: (lo_start, lo_end), hs }
}

// ---------------------------------------------------------------------
// The succinct closure: matrix-product-state residual evaluation
// ---------------------------------------------------------------------

/// Per-committed-coordinate closure descriptor (coordinates `7..n`, the
/// packed `m_p` coordinates in positional order).
#[derive(Clone, Debug)]
pub(crate) enum TapCoord {
    /// Plain eq factor against a fixed right-leg point (boolean column
    /// pins; also any untwisted coordinate).
    Plain(Gf),
    /// One bit of a carry chain: right-leg point `h`, addend bit `d`;
    /// `entry` pins the carry-in when this bit starts a segment, `exit`
    /// contracts the carry-out when it ends one.
    Chain { h: Gf, d: u8, entry: Option<u8>, exit: Option<TapExit> },
}

/// Segment-end contraction: pin the carry (dropout validity / class
/// match) or sum both branches (cyclic wrap).
#[derive(Clone, Copy, Debug)]
pub(crate) enum TapExit {
    Pin(u8),
    Sum,
}

/// The class's closure descriptor over the `m_p` packed coordinates.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn tap_closure_desc(
    layout: &ShaF2Layout,
    tap: &TapOp,
    pt_x: &[Gf],
    class: TapClass,
) -> Vec<TapCoord> {
    let one = Gf::one();
    let zero = Gf::zero();
    let tw = layout.tw;
    let lc = layout.log_cols;
    let bv = layout.bit_vars;
    let s = layout.p.s;
    let mut out = Vec::with_capacity(layout.p.t + s - 7);
    for (k, &h) in pt_x.iter().enumerate().take(tw).skip(7) {
        out.push(TapCoord::Chain {
            h,
            d: 0,
            entry: if k == 7 { Some(class.cut) } else { None },
            exit: if k == tw - 1 { Some(TapExit::Pin(0)) } else { None },
        });
    }
    for k in tw.max(7)..tw + lc {
        out.push(TapCoord::Plain(if (tap.col >> (k - tw)) & 1 == 1 { one } else { zero }));
    }
    for m in 0..bv {
        out.push(TapCoord::Chain {
            h: pt_x[tw + m],
            d: ((tap.bit_amt >> m) & 1) as u8,
            entry: if m == 0 { Some(0) } else { None },
            exit: if m == bv - 1 {
                Some(if tap.bit_dropout { TapExit::Pin(0) } else { TapExit::Sum })
            } else {
                None
            },
        });
    }
    for m in 0..s {
        out.push(TapCoord::Chain {
            h: pt_x[tw + bv + m],
            d: ((tap.off >> m) & 1) as u8,
            entry: if m == 0 { Some(0) } else { None },
            exit: if m == s - 1 { Some(TapExit::Pin(class.gamma)) } else { None },
        });
    }
    debug_assert_eq!(out.len(), layout.p.t + s - 7);
    out
}

/// One evaluator state: the K⊗K column representation, per live carry
/// value (`None` = the coordinate stream is outside any chain segment).
struct TapState {
    /// `cols[c]` for carry `c`; length 1 (plain) or 2 (inside a chain).
    states: Vec<Vec<Gf>>,
}

#[allow(clippy::arithmetic_side_effects)]
fn tap_state_apply(st: &mut TapState, coord: &TapCoord, left: (Gf, Gf), boolean: Option<u8>) {
    use crate::ligerito::apply_right_mul;
    let one = Gf::one();
    match coord {
        TapCoord::Plain(h) => {
            for cols in st.states.iter_mut() {
                let new = match boolean {
                    Some(b) => {
                        let f = if b == 1 { *h } else { one + *h };
                        apply_right_mul(cols, f)
                    }
                    None => {
                        let f0 = apply_right_mul(cols, one + *h);
                        let f1 = apply_right_mul(cols, *h);
                        let (la, a) = left;
                        (0..128).map(|u| la * f0[u] + a * f1[u]).collect()
                    }
                };
                *cols = new;
            }
        }
        TapCoord::Chain { h, d, entry, exit } => {
            if let Some(c0) = entry {
                debug_assert_eq!(st.states.len(), 1, "chain entry from plain mode");
                let cur = st.states.pop().expect("state");
                let zeros = vec![Gf::zero(); 128];
                st.states = if *c0 == 0 { vec![cur, zeros] } else { vec![zeros, cur] };
            }
            debug_assert_eq!(st.states.len(), 2, "chain bit inside a segment");
            let mut new = vec![vec![Gf::zero(); 128], vec![Gf::zero(); 128]];
            for c in 0..2u8 {
                let cols = &st.states[c as usize];
                if cols.iter().all(|g| g.is_zero()) {
                    continue;
                }
                for y in 0..2u8 {
                    let lscale = match boolean {
                        Some(b) => {
                            if b != y {
                                continue;
                            }
                            one
                        }
                        None => {
                            if y == 0 {
                                left.0
                            } else {
                                left.1
                            }
                        }
                    };
                    let z = y ^ d ^ c;
                    let c2 = (y & *d) | (y & c) | (*d & c);
                    let f = if z == 1 { *h } else { one + *h };
                    let add = apply_right_mul(cols, f);
                    let dstc = &mut new[c2 as usize];
                    for (o, x) in dstc.iter_mut().zip(add.iter()) {
                        *o += lscale * *x;
                    }
                }
            }
            match exit {
                Some(TapExit::Pin(c)) => {
                    let kept = new.swap_remove(*c as usize);
                    st.states = vec![kept];
                }
                Some(TapExit::Sum) => {
                    let (a, b) = (new.remove(0), new.remove(0));
                    st.states =
                        vec![a.iter().zip(b.iter()).map(|(x, y)| *x + *y).collect()];
                }
                None => st.states = new,
            }
        }
    }
}

/// Matrix-product-state generalization of
/// [`crate::ligerito::residual_b_evals`]: `Φ_{r″}∘B_β` evaluated at
/// `(prefix ++ bits(tail))` for every boolean tail, where `B_β` is the
/// class's translated-eq basis described by `desc` (one entry per packed
/// coordinate). For an all-[`TapCoord::Plain`] descriptor this is exactly
/// `residual_b_evals` (2 right-multiplications per coordinate); chain
/// bits cost ≤ 4.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn residual_b_evals_tap(
    prefix: &[Gf],
    yr_log_n: usize,
    desc: &[TapCoord],
    eq_r2: &[Gf],
) -> Vec<Gf> {
    assert_eq!(prefix.len() + yr_log_n, desc.len(), "prefix + tail must cover the coordinates");
    assert_eq!(eq_r2.len(), 128);
    let one = Gf::one();
    let mut init = vec![Gf::zero(); 128];
    init[0] = one;
    let mut st = TapState { states: vec![init] };
    for (a, coord) in prefix.iter().zip(desc.iter()) {
        tap_state_apply(&mut st, coord, (one + *a, *a), None);
    }
    let mut cur: Vec<TapState> = vec![st];
    for j in 0..yr_log_n {
        let coord = &desc[prefix.len() + j];
        let mut next: Vec<TapState> = Vec::with_capacity(cur.len() * 2);
        for b in 0..2u8 {
            for stt in cur.iter() {
                let mut branch = TapState { states: stt.states.to_vec() };
                tap_state_apply(&mut branch, coord, (one, one), Some(b));
                next.push(branch);
            }
        }
        // Tail index bit j is appended at weight 2^j: order [b=0 block, b=1 block].
        cur = next;
    }
    cur.iter()
        .map(|stt| {
            debug_assert_eq!(stt.states.len(), 1, "all chains closed at the end");
            stt.states[0]
                .iter()
                .zip(eq_r2.iter())
                .fold(Gf::zero(), |acc, (c, e)| acc + *c * *e)
        })
        .collect()
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ligerito::mle_eval;
    use crate::pcs::IntEvalParams;

    /// Synthetic tap test layout: 2 UAIR columns of 8 bit positions over
    /// 2^12 trace rows; committed t = 10, s = 6 (n = 16); x tensor
    /// t' = 9, s = 6 (n' = 15). `tw = 6`, `log_cols = 1` — the pack cut
    /// sits inside the column bits (`tw + lc = 7`), the tightest
    /// supported geometry.
    fn tap_layout_tw6() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams { t: 10, s: 6, word_bits: 1 },
            num_cols: 2,
            log_cols: 1,
            bit_vars: 3,
            num_vars: 12,
            tw: 6,
            x_fold_extra: 0,
        }
    }

    /// A `tw = 9 > 7` layout (three-class offsets): 2 columns, 8 bit
    /// positions, 2^13 trace rows, s = 4.
    fn tap_layout_tw9() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams { t: 13, s: 4, word_bits: 1 },
            num_cols: 2,
            log_cols: 1,
            bit_vars: 3,
            num_vars: 13,
            tw: 9,
            x_fold_extra: 0,
        }
    }

    fn test_rows(layout: &ShaF2Layout, seed: u64) -> Vec<Vec<u64>> {
        let words = (1usize << layout.p.t).div_ceil(64);
        (0..1usize << layout.p.s)
            .map(|c| {
                (0..words)
                    .map(|w| {
                        (c as u64 ^ seed)
                            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            .wrapping_add((w as u64).wrapping_mul(0xD134_2543_DE82_EF95))
                            .rotate_left((w % 61) as u32)
                    })
                    .collect()
            })
            .collect()
    }

    fn committed_bit(layout: &ShaF2Layout, rows: &[Vec<u64>], b: usize, c: usize) -> u64 {
        let _ = layout;
        (rows[c][b >> 6] >> (b & 63)) & 1
    }

    /// Naive per-bit tap extraction, straight from the semantics.
    fn extract_naive(
        layout: &ShaF2Layout,
        rows: &[Vec<u64>],
        taps: &[TapOp],
        constant: u128,
    ) -> Vec<Vec<u64>> {
        let tw = layout.tw;
        let lc = layout.log_cols;
        let bv = layout.bit_vars;
        let s = layout.p.s;
        let w = 1usize << bv;
        let nv = layout.num_vars;
        let words = (1usize << (bv + tw)).div_ceil(64);
        let mut out = vec![vec![0u64; words]; 1usize << s];
        for trace in 0..1usize << nv {
            let (row_hi, row_lo) = (trace >> s, trace & ((1 << s) - 1));
            for jj in 0..w {
                let mut bit = ((constant >> jj) & 1) as u64;
                for tap in taps {
                    if trace < tap.off {
                        continue;
                    }
                    let src_trace = trace - tap.off;
                    let src_j = if tap.bit_dropout {
                        if jj < tap.bit_amt {
                            continue;
                        }
                        jj - tap.bit_amt
                    } else {
                        (jj + w - tap.bit_amt) & (w - 1)
                    };
                    let b = (src_j << (lc + tw)) | (tap.col << tw) | (src_trace >> s);
                    bit ^= committed_bit(layout, rows, b, src_trace & ((1 << s) - 1));
                }
                if bit & 1 == 1 {
                    let pos = (jj << tw) | row_hi;
                    out[row_lo][pos >> 6] |= 1u64 << (pos & 63);
                }
            }
        }
        out
    }

    #[test]
    fn tap_extraction_matches_naive() {
        for layout in [tap_layout_tw6(), tap_layout_tw9()] {
            let rows = test_rows(&layout, 7);
            let cases: Vec<Vec<TapOp>> = vec![
                vec![TapOp::ident(0)],
                vec![TapOp { col: 0, bit_amt: 1, bit_dropout: false, off: 0 }],
                vec![TapOp { col: 1, bit_amt: 5, bit_dropout: true, off: 0 }],
                vec![TapOp { col: 0, bit_amt: 0, bit_dropout: false, off: 1 }],
                vec![TapOp { col: 1, bit_amt: 3, bit_dropout: false, off: 2 }],
                // The b_3-style claim: three taps on one column.
                vec![
                    TapOp { col: 0, bit_amt: 1, bit_dropout: false, off: 0 },
                    TapOp { col: 0, bit_amt: 2, bit_dropout: false, off: 1 },
                    TapOp { col: 0, bit_amt: 3, bit_dropout: false, off: 2 },
                ],
                // Cross-column with shift and rot.
                vec![
                    TapOp { col: 0, bit_amt: 3, bit_dropout: true, off: 0 },
                    TapOp { col: 1, bit_amt: 5, bit_dropout: true, off: 1 },
                    TapOp { col: 1, bit_amt: 2, bit_dropout: false, off: 2 },
                ],
            ];
            for taps in &cases {
                let fast = extract_virtual_tap_rows(&layout, &rows, taps, 0);
                let naive = extract_naive(&layout, &rows, taps, 0);
                assert_eq!(fast, naive, "taps {taps:?}");
            }
            let taps = [TapOp { col: 0, bit_amt: 2, bit_dropout: false, off: 1 }];
            let fast = extract_virtual_tap_rows(&layout, &rows, &taps, 0b1010_0101);
            let naive = extract_naive(&layout, &rows, &taps, 0b1010_0101);
            assert_eq!(fast, naive, "constant pattern");
        }
    }

    fn test_point(len: usize, seed: u64) -> Vec<Gf> {
        (0..len)
            .map(|i| {
                let x = (i as u64 ^ seed)
                    .wrapping_mul(0xA24B_AED4_963E_E407)
                    .wrapping_add(0x9FB2_1C65_1E98_DF25);
                Gf::from_words([x, x.rotate_left(17) ^ seed])
            })
            .collect()
    }

    /// The load-bearing split identity: for every source x-index `x'`,
    /// `Σ_β A_β(v)·B_β(yx) = eq(ζ, σ(x'))·[valid]` with
    /// `(v, y) = embed(x', col)` split at the pack, `yx = x' ≫ p0`.
    #[test]
    fn tap_weight_split_recombines() {
        for layout in [tap_layout_tw6(), tap_layout_tw9()] {
            let tw = layout.tw;
            let bv = layout.bit_vars;
            let s = layout.p.s;
            let w = 1usize << bv;
            let n_x = tw + bv + s;
            let pt = test_point(n_x, 0xBEEF ^ (tw as u64));
            let eqx = build_eq_x_r_vec(&pt, &()).unwrap();
            let p0 = 7usize.saturating_sub((7usize.saturating_sub(tw)).min(layout.log_cols));
            for tap in [
                TapOp::ident(1),
                TapOp { col: 0, bit_amt: 3, bit_dropout: false, off: 0 },
                TapOp { col: 1, bit_amt: 5, bit_dropout: true, off: 1 },
                TapOp { col: 0, bit_amt: 6, bit_dropout: false, off: 2 },
                TapOp { col: 0, bit_amt: 0, bit_dropout: false, off: 3 },
            ] {
                let classes = tap_classes(&layout, &tap);
                let plans: Vec<(Vec<Gf>, TapSupportTables)> = classes
                    .iter()
                    .map(|&cl| {
                        (
                            tap_inpack_table(&layout, &tap, &pt, cl),
                            tap_support_tables(&layout, &tap, &pt, cl),
                        )
                    })
                    .collect();
                for xp in 0..1usize << n_x {
                    let (row_hi, rest) = (xp & ((1 << tw) - 1), xp >> tw);
                    let (jj, row_lo) = (rest & (w - 1), rest >> bv);
                    // Forward image σ(x') and validity.
                    let src_trace = (row_hi << s) | row_lo;
                    let dst_trace = src_trace + tap.off;
                    let trace_ok = dst_trace < (1usize << layout.num_vars);
                    let (dst_j, bit_ok) = if tap.bit_dropout {
                        (jj + tap.bit_amt, jj + tap.bit_amt < w)
                    } else {
                        ((jj + tap.bit_amt) & (w - 1), true)
                    };
                    let expected = if trace_ok && bit_ok {
                        let dst_x = ((dst_trace >> s) & ((1 << tw) - 1))
                            | (dst_j << tw)
                            | ((dst_trace & ((1 << s) - 1)) << (tw + bv));
                        // dst row_hi can exceed tw bits only when !trace_ok.
                        eqx[dst_x]
                    } else {
                        Gf::zero()
                    };
                    let z = crate::ligerito_flock::embed_xor_index(&layout, xp, tap.col);
                    let v = z & 127;
                    let yx = xp >> p0;
                    debug_assert_eq!(z >> 7, {
                        let zz = crate::ligerito_flock::embed_xor_index(
                            &layout,
                            (yx << p0) | (xp & ((1 << p0) - 1)),
                            tap.col,
                        );
                        zz >> 7
                    });
                    let mut got = Gf::zero();
                    for (a_tbl, sup) in &plans {
                        let hs = sup.hs;
                        let hi = yx & ((1usize << hs) - 1);
                        let jmid = (yx >> hs) & (w - 1);
                        let lo = yx >> (hs + bv);
                        got += a_tbl[v] * sup.t_lo[lo] * sup.t_bit[jmid] * sup.t_hi[hi];
                    }
                    assert_eq!(got, expected, "tap {tap:?} x'={xp}");
                }
            }
        }
    }

    /// The MPS closure equals the naive Φ∘B multilinear evaluation.
    #[test]
    fn tap_closure_matches_naive() {
        use crate::ligerito::transpose_bits_128;
        let _ = transpose_bits_128; // silence unused when features vary
        for layout in [tap_layout_tw6(), tap_layout_tw9()] {
            let tw = layout.tw;
            let bv = layout.bit_vars;
            let s = layout.p.s;
            let w = 1usize << bv;
            let n = layout.p.t + layout.p.s;
            let m_p = n - 7;
            let n_x = tw + bv + s;
            let pt = test_point(n_x, 0xC0FFEE ^ (tw as u64));
            let eq_r2 = build_eq_x_r_vec(&test_point(7, 99)[..7], &()).unwrap();
            let phi = |g: Gf| -> Gf {
                let wds = g.words();
                let mut acc = Gf::zero();
                for wi in 0..2usize {
                    let mut bits = wds[wi];
                    while bits != 0 {
                        let t = bits.trailing_zeros() as usize;
                        acc += eq_r2[(wi << 6) | t];
                        bits &= bits.wrapping_sub(1);
                    }
                }
                acc
            };
            let p0 = 7usize.saturating_sub((7usize.saturating_sub(tw)).min(layout.log_cols));
            for tap in [
                TapOp::ident(0),
                TapOp { col: 1, bit_amt: 2, bit_dropout: false, off: 0 },
                TapOp { col: 0, bit_amt: 4, bit_dropout: true, off: 1 },
                TapOp { col: 1, bit_amt: 7, bit_dropout: false, off: 2 },
            ] {
                for &cl in &tap_classes(&layout, &tap) {
                    let sup = tap_support_tables(&layout, &tap, &pt, cl);
                    // Naive: scatter Φ(B_β) into the packed y-space and
                    // evaluate its MLE at (prefix ++ tails).
                    let mut bphi = vec![Gf::zero(); 1usize << m_p];
                    for yx in 0..1usize << (n_x - p0) {
                        let hs = sup.hs;
                        let hi = yx & ((1usize << hs) - 1);
                        let jmid = (yx >> hs) & (w - 1);
                        let lo = yx >> (hs + bv);
                        let val = sup.t_lo[lo] * sup.t_bit[jmid] * sup.t_hi[hi];
                        if val.is_zero() {
                            continue;
                        }
                        let z = crate::ligerito_flock::embed_xor_index(
                            &layout,
                            yx << p0,
                            tap.col,
                        );
                        bphi[z >> 7] = phi(val);
                    }
                    let desc = tap_closure_desc(&layout, &tap, &pt, cl);
                    for yr in [0usize, 2] {
                        let prefix = test_point(m_p - yr, 0xF00D + yr as u64);
                        let got = residual_b_evals_tap(&prefix, yr, &desc, &eq_r2);
                        for (tail, &got_t) in got.iter().enumerate() {
                            let mut point = prefix.clone();
                            for tb in 0..yr {
                                point.push(if (tail >> tb) & 1 == 1 {
                                    Gf::one()
                                } else {
                                    Gf::zero()
                                });
                            }
                            let want = mle_eval(&bphi, &point);
                            assert_eq!(got_t, want, "tap {tap:?} class {cl:?} tail {tail}");
                        }
                    }
                }
            }
        }
    }
}
