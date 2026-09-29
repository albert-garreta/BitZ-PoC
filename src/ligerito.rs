//! Shared bit packing, ring switching, and succinct residual-basis kernels.
//!
//! Ring switching follows Flock (Apache-2.0 OR MIT, Succinct Labs / Bünz /
//! Wang), based on Diamond–Posen and bcc-research/bolt-rs. Integer openings
//! live in `crate::wfbitz`; these kernels also serve the joint binary PCS.

use crate::pcs::IntegerMatrixLayout;
use crate::poly::univariate::binary_gf128::{Gf128 as Gf, REDUCTION_LOW_GF128};
use crate::poly::utils::build_eq_x_r_vec;
use crate::transcript::traits::Transcript;
use crate::utils::{cfg_chunks_mut, cfg_into_iter};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Per-column bit rows, 64 bits per `u64` word (row-major over the row-bit
/// index `i = (b<<log₂W)|j`). Local copy — the sibling branches' commit
/// paths pack differently; the Ligerito stack owns its row layout.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn repack_leaf_bits(p: &IntegerMatrixLayout, data: &[u128]) -> Vec<Vec<u64>> {
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let words = (row_len + 63) >> 6;
    cfg_into_iter!(0..p.cols())
        .map(|c| {
            let mut w = vec![0u64; words];
            for b in 0..p.rows() {
                let cell = data[p.cell_index(b, c)];
                for j in 0..p.word_bits {
                    if (cell >> j) & 1 == 1 {
                        let i = (b << log_w) | j;
                        w[i >> 6] |= 1u64 << (i & 63);
                    }
                }
            }
            w
        })
        .collect()
}
/// Number of packed (in-pack) coordinates: 128 bits per `GF(2^128)` element.
pub const LOG_PACKING: usize = 7;

// ---------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------

/// `a·X` in `GF(2^128)`: shift by one with the `0x87` reduction.
#[allow(clippy::arithmetic_side_effects)]
#[inline]
fn mul_by_x(a: Gf) -> Gf {
    let w = a.as_words();
    let carry = w[1] >> 63;
    let hi = (w[1] << 1) | (w[0] >> 63);
    let mut lo = w[0] << 1;
    if carry == 1 {
        lo ^= REDUCTION_LOW_GF128;
    }
    Gf::from_polynomial_words([lo, hi])
}

/// Absorb a `Gf` slice into the transcript with a domain tag (framed by
/// `absorb_slice`; little-endian words, matching the repo's byte layout).
#[allow(clippy::arithmetic_side_effects)]
fn absorb_gf_slice(transcript: &mut impl Transcript, tag: u8, vals: &[Gf]) {
    let mut bytes = Vec::with_capacity(vals.len() * 16 + 1);
    bytes.push(tag);
    for v in vals {
        let w = v.as_words();
        bytes.extend_from_slice(&w[0].to_le_bytes());
        bytes.extend_from_slice(&w[1].to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// Absorb the Round-0 (out-of-domain) value `y = MLE[P](ζ⃗)` (domain tag
/// 0x50) — bound right after the `ζ` draw, before any forest message.
pub(crate) fn absorb_ood_value(transcript: &mut impl Transcript, y: Gf) {
    absorb_gf_slice(transcript, 0x50, &[y]);
}

/// In-place multilinear bind of the LOWEST index bit:
/// `tbl'[i] = tbl[2i] + r·(tbl[2i] + tbl[2i+1])`.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn bind_low(tbl: &mut Vec<Gf>, r: Gf) {
    let half = tbl.len() >> 1;
    for i in 0..half {
        let u = tbl[2 * i];
        let v = tbl[2 * i + 1];
        tbl[i] = u + r * (u + v);
    }
    tbl.truncate(half);
}

/// Evaluate the multilinear with value table `tbl` (index bit `k` ↔
/// `point[k]`) at `point`, by repeated low-bit binds.
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval(tbl: &[Gf], point: &[Gf]) -> Gf {
    assert_eq!(
        tbl.len(),
        1usize << point.len(),
        "table/point size mismatch"
    );
    let mut buf = tbl.to_vec();
    for &r in point {
        bind_low(&mut buf, r);
    }
    buf[0]
}

/// 128×128 bit transpose of `GF(2^128)` elements viewed as `F_2^{128}`
/// vectors: `bit_v(out[u]) = bit_u(inp[v])`. The injective recombination
/// `s_u = Σ_v s_{u,v}·β_v` of the ring-switch (paper Eq. 8).
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn transpose_bits_128(inp: &[Gf]) -> Vec<Gf> {
    assert_eq!(inp.len(), 128);
    let mut out = vec![[0u64; 2]; 128];
    for (v, s) in inp.iter().enumerate() {
        let w = s.as_words();
        for u in 0..128 {
            if (w[u >> 6] >> (u & 63)) & 1 == 1 {
                out[u][v >> 6] |= 1u64 << (v & 63);
            }
        }
    }
    out.into_iter().map(Gf::from_polynomial_words).collect()
}

/// `Σ_u bit_w(X^u·y)·cols[u]` for every `w`: one tensor-algebra
/// right-leg multiplication step. (Shared with the structured-tap MPS
/// closure, [`crate::taps`].)
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn apply_right_mul(cols: &[Gf], y: Gf) -> Vec<Gf> {
    let mut out = vec![Gf::zero(); 128];
    let mut row = y; // X^u · y, starting at u = 0
    for cu in cols.iter().take(128) {
        if !cu.is_zero() {
            let w = row.as_words();
            for wi in 0..2usize {
                let mut bits = w[wi];
                while bits != 0 {
                    let t = bits.trailing_zeros() as usize;
                    out[(wi << 6) | t] += *cu;
                    bits &= bits.wrapping_sub(1);
                }
            }
        }
        row = mul_by_x(row);
    }
    out
}

/// Succinct evaluation of `B̂(chals)` where `B(y) = Φ_{r″}(eq(r_hi, y))` and
/// `Φ_{r″}: β_u ↦ eq_r2[u]` is the `F_2`-linear batching map — the
/// tensor-algebra trick (Diamond–Posen; Flock's `eval_rs_eq`):
/// maintain `E = ∏_k [(1+a_k)⊗(1+h_k) + a_k⊗h_k] ∈ K ⊗_{F_2} K` in the
/// column representation `E = Σ_u e_u ⊗ β_u`, then contract with `Φ`:
/// `B̂(chals) = Σ_u e_u · eq_r2[u]`. Cost `O(len·128²)` bit-conditional adds.
pub fn tensor_eq_phi_eval(chals: &[Gf], r_hi: &[Gf], eq_r2: &[Gf]) -> Gf {
    let evals = residual_b_evals(chals, 0, r_hi, eq_r2);
    evals[0]
}

/// The Ligerito residual-block variant: `B̂(prefix ++ bits(y))` for every
/// boolean tail `y ∈ [0, 2^{yr_log_n})` (tail bit `j` ↔ coordinate
/// `prefix.len() + j`). Shares the tensor prefix across all tails — the
/// boolean tail legs are single-term algebra multiplications, so the whole
/// block costs `O((prefix.len() + 2^{yr_log_n})·128²)` instead of
/// `2^{yr_log_n}` full evaluations. This is the succinct-basis hook the
/// Ligerito verifier calls once at its residual check.
#[allow(clippy::arithmetic_side_effects)]
pub fn residual_b_evals(prefix: &[Gf], yr_log_n: usize, r_hi: &[Gf], eq_r2: &[Gf]) -> Vec<Gf> {
    assert_eq!(
        prefix.len().wrapping_add(yr_log_n),
        r_hi.len(),
        "prefix + tail must cover the point"
    );
    assert_eq!(eq_r2.len(), 128);
    let one = Gf::one();
    let mut cols = vec![Gf::zero(); 128];
    cols[0] = one;
    for (a, h) in prefix.iter().zip(r_hi.iter()) {
        let f0 = apply_right_mul(&cols, one + *h);
        let f1 = apply_right_mul(&cols, *h);
        let la = one + *a;
        for w in 0..128 {
            cols[w] = la * f0[w] + *a * f1[w];
        }
    }
    // Bit tail expansion: at each step j, the left leg is 0 or 1, so
    // E·[(1+a)⊗(1+h) + a⊗h] collapses to the single term E·(1⊗(1+h)) (bit 0)
    // or E·(1⊗h) (bit 1). Tail index bit j is appended at weight 2^j.
    let mut cur: Vec<Vec<Gf>> = vec![cols];
    for j in 0..yr_log_n {
        let h = r_hi[prefix.len() + j];
        let stride = cur.len();
        let mut next: Vec<Vec<Gf>> = Vec::with_capacity(stride * 2);
        next.resize(stride * 2, Vec::new());
        for (i, e) in cur.iter().enumerate() {
            next[i] = apply_right_mul(e, one + h);
            next[i + stride] = apply_right_mul(e, h);
        }
        cur = next;
    }
    cur.iter()
        .map(|cols| {
            cols.iter()
                .zip(eq_r2.iter())
                .fold(Gf::zero(), |acc, (c, e)| acc + *c * *e)
        })
        .collect()
}

// ---------------------------------------------------------------------
// Packed commitment
// ---------------------------------------------------------------------

/// `t + log₂W` — the row-bit index width.
pub(crate) fn row_bit_vars(p: &IntegerMatrixLayout) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    p.row_vars.wrapping_add(log_w)
}

/// Number of packed variables `m_p = (t + log₂W − 7) + s`.
pub fn packed_vars(p: &IntegerMatrixLayout) -> usize {
    row_bit_vars(p)
        .wrapping_sub(LOG_PACKING)
        .wrapping_add(p.col_vars)
}

// ---------------------------------------------------------------------
// Ring-switch validation
// ---------------------------------------------------------------------

/// Errors of the binary ring switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RsOpenError {
    /// Malformed proof shape (lengths inconsistent with the config).
    Shape,
    /// `Σ_v eq(r_lo, v)·s_v ≠ μ`.
    RingSwitchClaim,
}

// ---------------------------------------------------------------------
// Ring-switch
// ---------------------------------------------------------------------

/// Ring-switch message: the 128 partial evaluations `s_v = M̂(r_hi, v)`.
#[derive(Clone, Debug)]
pub struct RingSwitchProof {
    pub s_v: Vec<Gf>,
}

/// Absorb an `s_v` message with the ring-switch domain tag (shared by the
/// single-poly and batched paths — the byte streams must match).
pub(crate) fn absorb_sv(transcript: &mut impl Transcript, s: &[Gf]) {
    absorb_gf_slice(transcript, 0x20, s);
}

/// 128-bit-word view of a packed-message element, so the fold kernels run
/// over both this module's `Gf` and the flock backend's `F128` without a
/// conversion pass over the 2^{m_p}-element message.
pub(crate) trait PackedBits: Sync {
    fn bit_words(&self) -> [u64; 2];
}

impl PackedBits for Gf {
    #[inline(always)]
    fn bit_words(&self) -> [u64; 2] {
        *self.as_words()
    }
}

/// Hacker's Delight §7-3 8×8 bit-matrix transpose stored in a `u64`
/// (bit `r·8 + c` of the input ↦ bit `c·8 + r` of the output).
#[inline(always)]
pub(crate) fn transpose_8x8_bits(mut x: u64) -> u64 {
    let t = (x ^ (x >> 7)) & 0x00AA_00AA_00AA_00AAu64;
    x = x ^ t ^ (t << 7);
    let t = (x ^ (x >> 14)) & 0x0000_CCCC_0000_CCCCu64;
    x = x ^ t ^ (t << 14);
    let t = (x ^ (x >> 28)) & 0x0000_0000_F0F0_F0F0u64;
    x ^ t ^ (t << 28)
}

/// 16-entry subset-sum table over 4 elements:
/// `sums[mask] = Σ_{k : bit_k(mask)} e[k]` (15 additions by doubling).
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
pub(crate) fn subset_sums_4(e: [Gf; 4]) -> [Gf; 16] {
    let mut sums = [Gf::zero(); 16];
    for (i, &v) in e.iter().enumerate() {
        let half = 1usize << i;
        for k in 0..half {
            sums[half + k] = sums[k] + v;
        }
    }
    sums
}

/// Scalar bit-scan tail: `s[j] += e` for every set bit `j` of `w`.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
pub(crate) fn sv_scalar_accum(s: &mut [Gf], w: [u64; 2], e: Gf) {
    for wi in 0..2usize {
        let mut bits = w[wi];
        while bits != 0 {
            let t = bits.trailing_zeros() as usize;
            s[(wi << 6) | t] += e;
            bits &= bits.wrapping_sub(1);
        }
    }
}

/// `s_v[j] = Σ_y eq[y]·bit_j(wit[y])` — the in-pack marginal, computed with
/// the method-of-four-Russians fold: per 8 witness elements, two 16-entry
/// subset-sum tables over their `eq` values, then per byte position one 8×8
/// bit transpose and per output bit **two table lookups + one accumulator
/// RMW**, independent of bit density (the scalar path pays one
/// data-dependent branchy add per set bit — ~64/element at random data).
/// Chunk-parallel with per-chunk partial accumulators; exact field sums, so
/// the result is bit-identical to the scalar scan for any summation order.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::needless_range_loop)] // r_byte loop mirrors flock's kernel 1:1
pub(crate) fn sv_fold_mfr<T: PackedBits>(wit: &[T], eq: &[Gf]) -> Vec<Gf> {
    assert_eq!(wit.len(), eq.len());
    const CHUNK: usize = 1 << 12; // multiple of 8 ⇒ only the global tail is scalar
    let n_chunks = wit.len().div_ceil(CHUNK).max(1);
    let partials: Vec<Vec<Gf>> = cfg_into_iter!(0..n_chunks)
        .map(|c| {
            let lo = c * CHUNK;
            let hi = (lo + CHUNK).min(wit.len());
            let mut s = vec![Gf::zero(); 128];
            let mut y = lo;
            while y + 8 <= hi {
                // Zero words contribute nothing: a block of eight zero
                // words (a virtual layout's unused lanes, zero padding)
                // skips its subset-sum tables and transposes.
                if wit[y..y + 8].iter().all(|w| w.bit_words() == [0, 0]) {
                    y += 8;
                    continue;
                }
                let lo_tbl = subset_sums_4([eq[y], eq[y + 1], eq[y + 2], eq[y + 3]]);
                let hi_tbl = subset_sums_4([eq[y + 4], eq[y + 5], eq[y + 6], eq[y + 7]]);
                let mut m_bytes = [[0u8; 16]; 8];
                for (e, slot) in m_bytes.iter_mut().enumerate() {
                    let w = wit[y + e].bit_words();
                    slot[..8].copy_from_slice(&w[0].to_le_bytes());
                    slot[8..].copy_from_slice(&w[1].to_le_bytes());
                }
                for r_byte in 0..16 {
                    let combined: u64 = (m_bytes[0][r_byte] as u64)
                        | ((m_bytes[1][r_byte] as u64) << 8)
                        | ((m_bytes[2][r_byte] as u64) << 16)
                        | ((m_bytes[3][r_byte] as u64) << 24)
                        | ((m_bytes[4][r_byte] as u64) << 32)
                        | ((m_bytes[5][r_byte] as u64) << 40)
                        | ((m_bytes[6][r_byte] as u64) << 48)
                        | ((m_bytes[7][r_byte] as u64) << 56);
                    let tb = transpose_8x8_bits(combined).to_le_bytes();
                    let base = r_byte * 8;
                    for (p, &mask) in tb.iter().enumerate() {
                        s[base + p] +=
                            lo_tbl[(mask & 0x0F) as usize] + hi_tbl[(mask >> 4) as usize];
                    }
                }
                y += 8;
            }
            while y < hi {
                sv_scalar_accum(&mut s, wit[y].bit_words(), eq[y]);
                y += 1;
            }
            s
        })
        .collect();
    let mut s = vec![Gf::zero(); 128];
    for part in &partials {
        for (a, b) in s.iter_mut().zip(part.iter()) {
            *a += *b;
        }
    }
    s
}

/// `Φ_{r″}` — the F₂-linear batching map on the bit representation:
/// `β_u ↦ eq_r2[u]`, applied to a K element by summing over its set bits.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
pub(crate) fn phi_bit_sum(ev: Gf, eq_r2: &[Gf]) -> Gf {
    let w = ev.as_words();
    let mut acc = Gf::zero();
    for wi in 0..2usize {
        let mut bits = w[wi];
        while bits != 0 {
            let t = bits.trailing_zeros() as usize;
            acc += eq_r2[(wi << 6) | t];
            bits &= bits.wrapping_sub(1);
        }
    }
    acc
}

/// 16 byte-position subset-sum tables of `scale·eq_r2`:
/// `T[pos·256 + v] = Σ_{bit j of v} scale·eq_r2[pos·8 + j]` (64 KB). A
/// `Φ_{r″}` image then costs 16 gathers + a XOR tree ([`phi_from_words`])
/// instead of a data-dependent bit scan, and premultiplying `scale` (the
/// batching `η`) into the tables removes the per-element `η·Φ(…)` field
/// multiply entirely.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn phi_byte_tables(eq_r2: &[Gf], scale: Gf) -> Vec<Gf> {
    debug_assert_eq!(eq_r2.len(), 128);
    let mut t = vec![Gf::zero(); 16 * 256];
    phi_byte_tables_into(&mut t, |i| scale * eq_r2[i]);
    t
}

/// Fill the byte-position subset sums of 128 coefficients, overwriting `out`.
/// The callback lets callers supply precomputed or scaled coefficients without
/// allocating an intermediate table or multiplying the unit-scale case.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn phi_byte_tables_into(out: &mut [Gf], coefficient: impl Fn(usize) -> Gf) {
    debug_assert_eq!(out.len(), 16 * 256);
    for pos in 0..16usize {
        let table = &mut out[pos << 8..(pos + 1) << 8];
        table[0] = Gf::zero();
        for j in 0..8usize {
            let base = coefficient((pos << 3) | j);
            let half = 1usize << j;
            for k in 0..half {
                table[half + k] = table[k] + base;
            }
        }
    }
}

/// `Σ_l T_l[…]` gather for one element: 16 byte-indexed lookups into a
/// [`phi_byte_tables`] table, tree-reduced. Equals
/// `scale·Φ_{r″}(element)` bit-for-bit.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
pub(crate) fn phi_from_words(w: [u64; 2], tables: &[Gf]) -> Gf {
    let lb = w[0].to_le_bytes();
    let hb = w[1].to_le_bytes();
    let p0 = tables[lb[0] as usize] + tables[(1 << 8) | lb[1] as usize];
    let p1 = tables[(2 << 8) | lb[2] as usize] + tables[(3 << 8) | lb[3] as usize];
    let p2 = tables[(4 << 8) | lb[4] as usize] + tables[(5 << 8) | lb[5] as usize];
    let p3 = tables[(6 << 8) | lb[6] as usize] + tables[(7 << 8) | lb[7] as usize];
    let p4 = tables[(8 << 8) | hb[0] as usize] + tables[(9 << 8) | hb[1] as usize];
    let p5 = tables[(10 << 8) | hb[2] as usize] + tables[(11 << 8) | hb[3] as usize];
    let p6 = tables[(12 << 8) | hb[4] as usize] + tables[(13 << 8) | hb[5] as usize];
    let p7 = tables[(14 << 8) | hb[6] as usize] + tables[(15 << 8) | hb[7] as usize];
    ((p0 + p1) + (p2 + p3)) + ((p4 + p5) + (p6 + p7))
}

/// Prover: compute and absorb `s_v`, draw `r″`, and produce the BaseFold
/// weight table `B(y) = Φ_{r″}(eq(r_hi, y))` (plus `eq_r2` and `β₀` for
/// debugging/tests).
#[allow(clippy::arithmetic_side_effects)]
pub fn ring_switch_prove<T: PackedBits>(
    transcript: &mut impl Transcript,
    p_msg: &[T],
    r_hi: &[Gf],
) -> (RingSwitchProof, Vec<Gf>, Gf) {
    ring_switch_prove_with(transcript, p_msg, r_hi, |b| b)
}

/// [`ring_switch_prove`] with the basis `B(y)` written through `convert`
/// as it is produced — a caller that needs it in another bit-compatible
/// element type (flock's `F128`) gets it without a second pass over the
/// 2^m-element table.
pub fn ring_switch_prove_with<T: PackedBits, O: Send>(
    transcript: &mut impl Transcript,
    p_msg: &[T],
    r_hi: &[Gf],
    convert: impl Fn(Gf) -> O + Sync + Send,
) -> (RingSwitchProof, Vec<O>, Gf) {
    let eq_hi = build_eq_x_r_vec(r_hi, &()).expect("non-empty r_hi");
    assert_eq!(eq_hi.len(), p_msg.len());

    // s_v = Σ_y eq_hi[y] · bit_v(P[y]): parallel partial accumulators over
    // y-chunks, merged by field addition (exact, order-independent).
    let s = sv_fold_mfr(p_msg, &eq_hi);
    absorb_sv(transcript, &s);
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2 non-empty");

    // β₀ = Σ_u eq_r2[u]·s_u via the bit transpose.
    let s_u = transpose_bits_128(&s);
    let beta0 = s_u
        .iter()
        .zip(eq_r2.iter())
        .fold(Gf::zero(), |acc, (su, e)| acc + *su * *e);

    // B(y) = Φ_{r″}(eq_hi[y]) = Σ_{u: bit_u(eq_hi[y])} eq_r2[u]. Parallel per y.
    let phi_slow = |y: usize| {
        let w = eq_hi[y].as_words();
        let mut acc = Gf::zero();
        for wi in 0..2usize {
            let mut bits = w[wi];
            while bits != 0 {
                let t = bits.trailing_zeros() as usize;
                acc += eq_r2[(wi << 6) | t];
                bits &= bits.wrapping_sub(1);
            }
        }
        acc
    };
    let tables = phi_byte_tables(&eq_r2, Gf::one());
    let b_tbl: Vec<O> = cfg_into_iter!(0..eq_hi.len())
        .map(|y| convert(phi_from_words(*eq_hi[y].as_words(), &tables)))
        .collect();
    debug_assert_eq!(
        (0..eq_hi.len())
            .zip(p_msg.iter())
            .fold(Gf::zero(), |a, (y, p)| a + phi_slow(y)
                * Gf::from_polynomial_words(p.bit_words())),
        beta0,
        "ring-switch recombination identity"
    );

    (RingSwitchProof { s_v: s }, b_tbl, beta0)
}

/// Verifier: check `Σ_v eq(r_lo,v)·s_v = μ`, absorb, draw `r″`, and return
/// `(eq_r2, β₀)` for the BaseFold stage.
#[allow(clippy::arithmetic_side_effects)]
pub fn ring_switch_verify(
    transcript: &mut impl Transcript,
    proof: &RingSwitchProof,
    mu: Gf,
    r_lo: &[Gf],
) -> Result<(Vec<Gf>, Gf), RsOpenError> {
    if proof.s_v.len() != 128 || r_lo.len() != LOG_PACKING {
        return Err(RsOpenError::Shape);
    }
    let eq_lo = build_eq_x_r_vec(r_lo, &()).expect("r_lo non-empty");
    let claim = proof
        .s_v
        .iter()
        .zip(eq_lo.iter())
        .fold(Gf::zero(), |acc, (s, e)| acc + *s * *e);
    if claim != mu {
        return Err(RsOpenError::RingSwitchClaim);
    }
    absorb_sv(transcript, &proof.s_v);
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2 non-empty");
    let s_u = transpose_bits_128(&proof.s_v);
    let beta0 = s_u
        .iter()
        .zip(eq_r2.iter())
        .fold(Gf::zero(), |acc, (su, e)| acc + *su * *e);
    Ok((eq_r2, beta0))
}

// ---------------------------------------------------------------------
// End-to-end integer-MLE evaluation with the RS opening
// ---------------------------------------------------------------------

/// `eq(c,ξ)`-combined rows: `m_ξ[i] = Σ_c eq_ξ[c]·M[c][i]`, from the packed
/// bit rows. Parallel over 64-entry output blocks (each block scans all
/// rows' matching word — exact field sums, order-independent in char 2).
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn xi_combined_rows(
    p: &IntegerMatrixLayout,
    rows: &[Vec<u64>],
    eq_xi: &[Gf],
) -> Vec<Gf> {
    let t_w = row_bit_vars(p);
    let len = 1usize << t_w;
    debug_assert!(
        len >= 64,
        "row_len below one word is out of scope (t_w >= 7 holds here)"
    );
    let mut m = vec![Gf::zero(); len];
    cfg_chunks_mut!(m, 64).enumerate().for_each(|(wi, block)| {
        for (c, row) in rows.iter().enumerate() {
            let mut bits = row[wi];
            while bits != 0 {
                let t = bits.trailing_zeros() as usize;
                block[t] += eq_xi[c];
                bits &= bits.wrapping_sub(1);
            }
        }
    });
    m
}

/// [`xi_combined_rows`] off the hint's 64-column-per-word store: per lane
/// group `g`, precombine `eq_xi` into 8 byte-position subset-sum tables
/// (`tb[pos][byte] = Σ_{b∈byte} eq_xi[64g + 8·pos + b]`, built by
/// doubling), then every output position is `8·⌈cols/64⌉` table gathers —
/// no per-column scatter, no tz-walk dependency chain (the
/// [`phi_byte_tables`] trick on the ξ axis; ~2× the scatter form at
/// n = 28 and it reads `packed_cols` instead of re-scanning `rows`).
/// Exact char-2 re-association — byte-identical.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn xi_combined_rows_packed(
    p: &IntegerMatrixLayout,
    packed_cols: &[Vec<u64>],
    eq_xi: &[Gf],
) -> Vec<Gf> {
    let t_w = row_bit_vars(p);
    let len = 1usize << t_w;
    let cols = p.cols();
    let groups = cols.div_ceil(64);
    debug_assert!(packed_cols.len() >= groups && packed_cols[0].len() == len);
    let tables: Vec<Vec<Gf>> = cfg_into_iter!(0..groups)
        .map(|g| {
            let mut t = vec![Gf::zero(); 8 << 8];
            for pos in 0..8usize {
                let base_col = (g << 6) | (pos << 3);
                let tb = &mut t[pos << 8..(pos + 1) << 8];
                for b in 0..8usize {
                    let c = base_col + b;
                    if c >= cols {
                        break;
                    }
                    let w = eq_xi[c];
                    let lim = 1usize << b;
                    for m in 0..lim {
                        tb[m | lim] = tb[m] + w;
                    }
                }
            }
            t
        })
        .collect();
    // Group-OUTER accumulation per output chunk: each group's positions
    // stream sequentially, its 32 KB table stays L1-hot, and the chunk's
    // output slots are L1-resident RMW — no cross-group pointer chases in
    // the inner loop and no 256-deep serial add chain per output.
    let mut m = vec![Gf::zero(); len];
    cfg_chunks_mut!(m, 1 << 10)
        .enumerate()
        .for_each(|(ci, chunk)| {
            let base = ci << 10;
            for (g, tg) in tables.iter().enumerate() {
                let src = &packed_cols[g][base..base + chunk.len()];
                for (slot, &x) in chunk.iter_mut().zip(src.iter()) {
                    if x == 0 {
                        continue;
                    }
                    // Entry 0 of every byte table is zero, so each word takes
                    // its eight lookups without a branch, summed in pairs to
                    // keep the additions off one dependency chain.
                    let b = |pos: usize| tg[(pos << 8) | ((x >> (8 * pos)) & 0xFF) as usize];
                    *slot += ((b(0) + b(1)) + (b(2) + b(3))) + ((b(4) + b(5)) + (b(6) + b(7)));
                }
            }
        });
    m
}

/// Classic 64×64 bit-matrix transpose (6 mask/shift rounds): output word
/// `t`'s bit `k` = input word `k`'s bit `t`.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn transpose_64x64(a: &mut [u64; 64]) {
    let mut j = 32usize;
    let mut m = 0x0000_0000_FFFF_FFFFu64;
    while j != 0 {
        let mut k = 0usize;
        while k < 64 {
            let idx = k + j;
            let x = (a[k] ^ (a[idx] << j)) & !m;
            a[k] ^= x;
            a[idx] ^= x >> j;
            let knext = (k + j + 1) & !j;
            k = if (k + 1) & j != 0 { knext } else { k + 1 };
        }
        j >>= 1;
        m ^= m << j;
    }
}

/// Derive the column-lane packing from the row-major packed rows: block
/// (g, wi) of `packed_cols` is the bit-transpose of rows `64g..64g+64` at
/// word `wi`. One pass over the 8-byte-per-64-bits rows instead of a second
/// sweep of the u128 data tensor.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn pack_columns_from_rows(p: &IntegerMatrixLayout, rows: &[Vec<u64>]) -> Vec<Vec<u64>> {
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let words = row_len.div_ceil(64);
    let num_groups = p.cols().div_ceil(64);
    cfg_into_iter!(0..num_groups)
        .map(|g| {
            let mut out = vec![0u64; row_len];
            for wi in 0..words {
                let mut blk = [0u64; 64];
                for k in 0..64usize {
                    let c = (g << 6) | k;
                    if c < p.cols() {
                        blk[k] = rows[c][wi];
                    }
                }
                transpose_64x64(&mut blk);
                // blk[t] now holds, per lane k, bit (64·wi + t) of row 64g+k
                // — wait: transpose maps bit t of blk[k] to bit k of blk[t].
                let base = wi << 6;
                for (t, &v) in blk.iter().enumerate() {
                    if base + t < row_len {
                        out[base + t] = v;
                    }
                }
            }
            out
        })
        .collect()
}

/// Inverse of [`pack_columns_from_rows`]: rebuild the per-column bit rows
/// from the 64-column-lane packed store (`packed_cols[g][i]` bit `k` =
/// column `64g+k`'s bit `i` — the layout `sha_f2_packed_cols` produces).
/// Parallel over groups.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn rows_from_packed_cols(
    p: &IntegerMatrixLayout,
    packed_cols: &[Vec<u64>],
) -> Vec<Vec<u64>> {
    let log_w = p.word_bits.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let words = row_len.div_ceil(64);
    let num_groups = p.cols().div_ceil(64);
    debug_assert_eq!(packed_cols.len(), num_groups);
    let groups: Vec<Vec<Vec<u64>>> = cfg_into_iter!(0..num_groups)
        .map(|g| {
            let lanes = 64.min(p.cols() - (g << 6));
            let mut rows_g: Vec<Vec<u64>> = vec![vec![0u64; words]; lanes];
            for wi in 0..words {
                let mut blk = [0u64; 64];
                for (t, b) in blk.iter_mut().enumerate() {
                    let i = (wi << 6) | t;
                    *b = if i < row_len { packed_cols[g][i] } else { 0 };
                }
                transpose_64x64(&mut blk);
                for (k, row) in rows_g.iter_mut().enumerate() {
                    row[wi] = blk[k];
                }
            }
            rows_g
        })
        .collect();
    groups.into_iter().flatten().collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_polynomial_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }

    /// The packed column combination (eight branch-free byte lookups per
    /// word) equals the per-bit reference on dense, sparse and all-zero
    /// words, including a partial column group.
    #[test]
    fn packed_combination_matches_the_bit_walk() {
        for (t, s) in [(7usize, 3usize), (8, 6), (9, 7), (10, 8)] {
            let p = IntegerMatrixLayout {
                row_vars: t,
                col_vars: s,
                word_bits: 1,
            };
            let (cols, words) = (1usize << s, 1usize << (t - 6));
            let mut state = (t * 131 + s) as u64 | 1;
            let mut next = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            // Column c: dense for c % 3 == 0, sparse for 1, all zero words for 2.
            let rows: Vec<Vec<u64>> = (0..cols)
                .map(|c| {
                    (0..words)
                        .map(|_| match c % 3 {
                            0 => next(),
                            1 => next() & next() & next() & next(),
                            _ => 0,
                        })
                        .collect()
                })
                .collect();
            let groups = cols.div_ceil(64);
            let packed: Vec<Vec<u64>> = (0..groups)
                .map(|g| {
                    (0..1usize << t)
                        .map(|b| {
                            (0..64.min(cols - 64 * g)).fold(0u64, |word, lane| {
                                word | (((rows[64 * g + lane][b / 64] >> (b % 64)) & 1) << lane)
                            })
                        })
                        .collect()
                })
                .collect();
            let eq_xi: Vec<Gf> = (0..cols).map(|c| sample(c as u64 * 7 + 3)).collect();
            assert_eq!(
                xi_combined_rows_packed(&p, &packed, &eq_xi),
                xi_combined_rows(&p, &rows, &eq_xi),
                "t={t} s={s}"
            );
        }
    }

    /// `rows_from_packed_cols` inverts `pack_columns_from_rows` exactly
    /// (incl. non-multiple-of-64 column counts).
    #[test]
    fn packed_cols_row_roundtrip() {
        for (t, s, w) in [(7usize, 3usize, 1usize), (8, 6, 1), (7, 4, 2)] {
            let p = IntegerMatrixLayout {
                row_vars: t,
                col_vars: s,
                word_bits: w,
            };
            let log_w = w.trailing_zeros() as usize;
            let row_len = p.rows() << log_w;
            let words = row_len.div_ceil(64);
            let rows: Vec<Vec<u64>> = (0..p.cols())
                .map(|c| {
                    (0..words)
                        .map(|wd| {
                            (c as u64 + 3)
                                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                .wrapping_add(wd as u64)
                                .rotate_left((c + wd) as u32 & 63)
                        })
                        .collect()
                })
                .collect();
            let packed = pack_columns_from_rows(&p, &rows);
            let back = rows_from_packed_cols(&p, &packed);
            assert_eq!(rows, back, "(t={t},s={s},W={w})");
        }
    }

    #[test]
    fn tensor_eval_matches_bruteforce_and_table() {
        let m_p = 5usize;
        let chals: Vec<Gf> = (0..m_p).map(|i| sample(0x11 + i as u64)).collect();
        let r_hi: Vec<Gf> = (0..m_p).map(|i| sample(0x22 + i as u64)).collect();
        let r2: Vec<Gf> = (0..LOG_PACKING).map(|i| sample(0x33 + i as u64)).collect();
        let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2");
        let eq_hi = build_eq_x_r_vec(&r_hi, &()).expect("hi");

        // Brute force: Σ_y eq(y, chals)·Φ(eq_hi[y]).
        let eq_ch = build_eq_x_r_vec(&chals, &()).expect("ch");
        let mut brute = Gf::zero();
        for y in 0..(1usize << m_p) {
            let w = eq_hi[y].as_words();
            let mut phi = Gf::zero();
            for u in 0..128usize {
                if (w[u >> 6] >> (u & 63)) & 1 == 1 {
                    phi += eq_r2[u];
                }
            }
            brute += eq_ch[y] * phi;
        }
        let fast = tensor_eq_phi_eval(&chals, &r_hi, &eq_r2);
        assert_eq!(fast, brute, "tensor eval ≠ brute force");

        // And equals the MLE of the prover-side B table at chals.
        let b_tbl: Vec<Gf> = eq_hi
            .iter()
            .map(|ev| {
                let w = ev.as_words();
                let mut acc = Gf::zero();
                for u in 0..128usize {
                    if (w[u >> 6] >> (u & 63)) & 1 == 1 {
                        acc += eq_r2[u];
                    }
                }
                acc
            })
            .collect();
        assert_eq!(mle_eval(&b_tbl, &chals), fast, "B-table MLE ≠ tensor eval");
    }

    /// The Ligerito residual-block hook agrees with per-point tensor evals
    /// at every boolean tail (tail bit j ↔ coordinate prefix.len()+j).
    #[test]
    fn residual_b_evals_matches_pointwise() {
        let (pre_len, yr) = (4usize, 3usize);
        let m_p = pre_len + yr;
        let prefix: Vec<Gf> = (0..pre_len).map(|i| sample(0x51 + i as u64)).collect();
        let r_hi: Vec<Gf> = (0..m_p).map(|i| sample(0x61 + i as u64)).collect();
        let r2: Vec<Gf> = (0..LOG_PACKING).map(|i| sample(0x71 + i as u64)).collect();
        let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2");

        let block = residual_b_evals(&prefix, yr, &r_hi, &eq_r2);
        assert_eq!(block.len(), 1usize << yr);
        for y in 0..(1usize << yr) {
            let mut point = prefix.clone();
            for j in 0..yr {
                point.push(if (y >> j) & 1 == 1 {
                    Gf::one()
                } else {
                    Gf::zero()
                });
            }
            assert_eq!(
                block[y],
                tensor_eq_phi_eval(&point, &r_hi, &eq_r2),
                "residual block mismatch at y={y}"
            );
        }
    }

    /// The method-of-four-Russians `s_v` fold equals the scalar bit scan
    /// exactly, at 8-aligned and ragged lengths (incl. the sub-block tail).
    #[test]
    fn sv_fold_mfr_matches_scalar() {
        for &len in &[1usize, 7, 8, 9, 37, 64, 300, 4096, 4104] {
            let wit: Vec<Gf> = (0..len).map(|i| sample(0x3000 + i as u64)).collect();
            let eq: Vec<Gf> = (0..len).map(|i| sample(0x5000 + i as u64)).collect();
            let mut expect = vec![Gf::zero(); 128];
            for y in 0..len {
                sv_scalar_accum(&mut expect, *wit[y].as_words(), eq[y]);
            }
            assert_eq!(sv_fold_mfr(&wit, &eq), expect, "len {len}");
        }
    }

    #[test]
    fn phi_byte_tables_into_reuses_buffer() {
        let mut tables = vec![sample(1); 16 * 256];
        for seed in [0xA000, 0xB000] {
            let coefficients: Vec<_> = (0..128).map(|i| sample(seed + i)).collect();
            phi_byte_tables_into(&mut tables, |i| coefficients[i]);
            for (position, table) in tables.chunks_exact(256).enumerate() {
                for (bits, &actual) in table.iter().enumerate() {
                    let expected = (0..8)
                        .filter(|&bit| bits & (1 << bit) != 0)
                        .fold(Gf::zero(), |sum, bit| {
                            sum + coefficients[8 * position + bit]
                        });
                    assert_eq!(actual, expected, "position {position}, bits {bits}");
                }
            }
        }
    }

    /// The η-premultiplied byte tables reproduce `scale·Φ_{r″}` bit-for-bit.
    #[test]
    fn phi_byte_tables_match_bitscan() {
        let eq_r2: Vec<Gf> = (0..128).map(|i| sample(0x9100 + i as u64)).collect();
        for &scale_seed in &[0u64, 0x42, 0xFFFF] {
            let scale = if scale_seed == 0 {
                Gf::one()
            } else {
                sample(scale_seed)
            };
            let tables = phi_byte_tables(&eq_r2, scale);
            for i in 0..64u64 {
                let v = sample(0xA000 + i);
                let w = *v.as_words();
                let mut acc = Gf::zero();
                for wi in 0..2usize {
                    let mut bits = w[wi];
                    while bits != 0 {
                        let t = bits.trailing_zeros() as usize;
                        acc += eq_r2[(wi << 6) | t];
                        bits &= bits.wrapping_sub(1);
                    }
                }
                assert_eq!(phi_from_words(w, &tables), scale * acc, "elem {i}");
            }
        }
    }
}
