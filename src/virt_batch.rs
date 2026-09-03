//! Prover-side dual-basis batching for packed-source tensor repetitions
//! (the SHA-256 product layout: `global column = 1 + instance·w + local`,
//! one shared constant column, power-of-two instance count).
//!
//! The virtual opening's batching protocol sends the 128 dual-basis plane
//! openings `h_b = Σ_j bit_b(W_j)·P[j»7]·A(e_{j&127})` and then folds the
//! ρ-batched Ligerito basis `a′(y) = Σ_v Φ_ρ(W_{(y,v)})·A(e_v)`, where for a
//! packed-source repetition the source weights factor per chunk `l` as
//! `W_{(i,c)} = Σ_l e_{l,i}·s_{l,c}` (instance `i`, local column `c`;
//! [`crate::ligerito_flock`]'s `VirtColumnWeights::PackedSourceRepeated`).
//! The per-cell kernels pay one weight multiply, one basis multiply and a
//! 128-way bit scatter (resp. 16 table gathers) per source cell.
//!
//! This module restructures both passes around the dual-basis identity
//!
//! ```text
//! bit_b(e·s) = c₀(e·s·A(e_b)) = Σ_a bit_a(e·A(e_b)) · bit_a(ŝ),   ŝ := A⁻¹(s),
//! ```
//!
//! (`c₀(X^u·A(e_v)) = δ_{uv}`, [`crate::dual_basis`]) which separates the
//! instance factor from the local-column factor. With the LOCAL plane
//! packings
//!
//! ```text
//! R_a(y, i) := Σ_{v : cell (y,v) ∈ instance i} bit_a(ŝ_{c(y,v)}) · A(e_v)
//! ```
//!
//! — a bit-plane transpose of the instance's local columns in pack `y`,
//! shared by every instance whose first cell sits at the same pack offset
//! (its *phase*) — both messages become
//!
//! ```text
//! h_b   = Σ_i Σ_a bit_a(e_i·A(e_b)) · Q_i[a],    Q_i[a]  = Σ_y P[y]·R_a(y, i),
//! a′(y) = Σ_i Σ_a ρ′_{i,a} · R_a(y, i),         ρ′_{i,a} = Σ_b ρ_b·bit_a(e_i·A(e_b)),
//! ```
//!
//! so each source cell costs ONE unreduced fixed-scalar GF(2^128) multiply
//! per pass (4 shuffle-free PMULLs into a 2-limb accumulator, one fold per
//! accumulator; a 3-PMULL Karatsuba form with the varying operand pre-split
//! measured as a wash on the M4 — µop-bound), plus `O(128²)` field work per
//! instance: the `Q_i` read-off
//! through 16 byte tables, and `ρ′_i = Σ_{u ∈ e_i} C_u` from the 16
//! byte-indexed tables of `C_{u,a} = Σ_b ρ_b·bit_a(X^u·A(e_b))`, which
//! depend on ρ alone. The a′ pass also emits flock's Ligerito round-0
//! message (`fill_phi_basis_round0`'s contract). Only exact field
//! identities are used, so `h`, `a′` and the round-0 pair are bit-identical
//! to the per-cell kernels (pinned by `virtual_planes_match_cellwise` in
//! `ligerito_flock`). Prover-only: the verifier's reduction is untouched.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    ligerito::{LOG_PACKING, PackedBits, phi_byte_tables, phi_from_words, transpose_8x8_bits},
    poly::univariate::binary_gf128::BinaryFieldGF128 as Gf,
    utils::{cfg_chunks_mut, cfg_into_iter, cfg_iter, wide_mul::WideMulAcc},
};

/// Cells per source pack.
const PACK: usize = 1 << LOG_PACKING;

/// Packs per parallel task (even, so round-0 pairs never straddle tasks):
/// bounds the duplicated instance read-off at task boundaries to one
/// instance per ~2^11 packs while keeping hundreds of tasks at the
/// production shapes.
const TASK_PACKS: usize = 1 << 11;

/// Upper bound on the precomputed plane tables (all chunks, all phases).
const MAX_TABLE_BYTES: usize = 256 << 20;

/// Narrowest instance the engine accepts: below this the per-instance
/// read-offs outweigh the per-cell savings over the streamed kernels.
const MIN_LOCAL_WIDTH: usize = 512;

// ---------------------------------------------------------------------
// Fixed-scalar unreduced multiply-accumulate kernel
// ---------------------------------------------------------------------

/// `acc += x·fixed` with the pass-fixed multiplier preprocessed and the
/// product kept unreduced (`t_lo + X^64·t_mid`, 191 bits), reduced once
/// per accumulator. NEON: `prep_fixed` / `mul_fixed_wide` / `fold_x64` of
/// the field module; elsewhere the generic wide accumulator.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod kernel {
    use core::arch::aarch64::{uint64x2_t, vdupq_n_u64, veorq_u64, vst1q_u64};

    use crate::poly::univariate::binary_gf128::{BinaryFieldGF128 as Gf, neon};

    #[derive(Clone, Copy)]
    pub(super) struct Fixed(uint64x2_t, uint64x2_t);

    #[derive(Clone, Copy)]
    pub(super) struct Acc(uint64x2_t, uint64x2_t);

    #[inline(always)]
    pub(super) fn fixed(x: &Gf) -> Fixed {
        // SAFETY: as `neon::pmull_lo` (NEON+PMULL enabled by the build).
        let (rl, rh) = unsafe { neon::prep_fixed(x) };
        Fixed(rl, rh)
    }

    #[inline(always)]
    pub(super) fn zero() -> Acc {
        // SAFETY: plain NEON constants.
        unsafe { Acc(vdupq_n_u64(0), vdupq_n_u64(0)) }
    }

    #[inline(always)]
    pub(super) fn mul_acc(acc: &mut Acc, x: &Gf, fixed: &Fixed) {
        // SAFETY: as `neon::pmull_lo`.
        unsafe {
            let (lo, mid) = neon::mul_fixed_wide(neon::ld(x), fixed.0, fixed.1);
            acc.0 = veorq_u64(acc.0, lo);
            acc.1 = veorq_u64(acc.1, mid);
        }
    }

    #[inline(always)]
    pub(super) fn reduce(acc: &Acc) -> Gf {
        // SAFETY: as `neon::pmull_lo`; the store targets a valid word pair.
        unsafe {
            let g = vdupq_n_u64(0x87);
            let z = vdupq_n_u64(0);
            let r = neon::fold_x64(acc.0, acc.1, g, z);
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            Gf::from_words(out)
        }
    }
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
mod kernel {
    use crate::{
        poly::univariate::binary_gf128::BinaryFieldGF128 as Gf, utils::wide_mul::WideMulAcc,
    };

    #[derive(Clone, Copy)]
    pub(super) struct Fixed(Gf);

    #[derive(Clone)]
    pub(super) struct Acc(<Gf as WideMulAcc>::Wide);

    #[inline(always)]
    pub(super) fn fixed(x: &Gf) -> Fixed {
        Fixed(*x)
    }

    #[inline(always)]
    pub(super) fn zero() -> Acc {
        Acc(Gf::wide_zero(&Gf::zero()))
    }

    #[inline(always)]
    pub(super) fn mul_acc(acc: &mut Acc, x: &Gf, fixed: &Fixed) {
        Gf::wide_add_assign(&mut acc.0, &Gf::mul_wide(x, &fixed.0));
    }

    #[inline(always)]
    pub(super) fn reduce(acc: &Acc) -> Gf {
        Gf::from_wide(acc.0.clone())
    }
}

// ---------------------------------------------------------------------
// Dual-basis bit kernels
// ---------------------------------------------------------------------

/// Corrections of the dual basis beyond the coordinate reversal, indexed by
/// bits `1..=6` of the slot vector: `A(e_1) = X^127 + X^6 + X`,
/// `A(e_v) = X^{128−v} + X^{7−v}` for `2 ≤ v ≤ 6` (see [`crate::dual_basis`]).
const DUAL_CORR: [u64; 64] = {
    let mut table = [0u64; 64];
    let mut mask = 1usize;
    while mask < 64 {
        let mut corr = 0u64;
        let mut v = 1usize;
        while v <= 6 {
            if (mask >> (v - 1)) & 1 == 1 {
                corr ^= if v == 1 { (1 << 6) | (1 << 1) } else { 1 << (7 - v) };
            }
            v += 1;
        }
        table[mask] = corr;
        mask += 1;
    }
    table
};

/// `Σ_v bit_v(u)·A(e_v)`: the dual-basis packing of a slot-indexed bit
/// vector — the reversal `v ↦ 128 − v` (`v ≥ 1`), `e₀ ↦ 1`, plus the seven
/// XOR corrections.
#[inline(always)]
pub(crate) fn dual_pack(u: [u64; 2]) -> Gf {
    let r0 = u[1].reverse_bits();
    let r1 = u[0].reverse_bits();
    let lo = ((r0 << 1) | (u[0] & 1)) ^ DUAL_CORR[((u[0] >> 1) & 63) as usize];
    let hi = (r1 << 1) | (r0 >> 63);
    Gf::from_words([lo, hi])
}

/// `A⁻¹(s)`: the slot vector with `bit_a = c₀(X^a·s)`, so that
/// `s = Σ_a bit_a(ŝ)·A(e_a)` and `c₀(g·s) = Σ_a bit_a(g)·bit_a(ŝ)`.
pub(crate) fn dual_unpack(s: Gf) -> [u64; 2] {
    let mut z = s;
    let mut out = [0u64; 2];
    for a in 0..PACK {
        out[a >> 6] |= (z.words()[0] & 1) << (a & 63);
        z = z.mul_x();
    }
    out
}

/// 128×128 bit transpose: `out[c]` bit `r` = `rows[r]` bit `c`.
pub(crate) fn transpose_128x128(rows: &[[u64; 2]; PACK]) -> [[u64; 2]; PACK] {
    let mut out = [[0u64; 2]; PACK];
    for row_group in 0..16usize {
        for col_byte in 0..16usize {
            let mut block = 0u64;
            for i in 0..8usize {
                let row = rows[(row_group << 3) | i];
                let byte = (row[col_byte >> 3] >> ((col_byte & 7) << 3)) & 0xFF;
                block |= byte << (i << 3);
            }
            let t = transpose_8x8_bits(block);
            for j in 0..8usize {
                let byte = (t >> (j << 3)) & 0xFF;
                out[(col_byte << 3) | j][row_group >> 3] |= byte << ((row_group & 7) << 3);
            }
        }
    }
    out
}

/// `x << n` on a 128-bit slot vector, `0 ≤ n < 128`.
#[inline(always)]
fn shl128(x: [u64; 2], n: usize) -> [u64; 2] {
    match n {
        0 => x,
        1..=63 => [x[0] << n, (x[1] << n) | (x[0] >> (64 - n))],
        64 => [0, x[0]],
        _ => [0, x[0] << (n - 64)],
    }
}

/// `x >> n` on a 128-bit slot vector, `0 ≤ n < 128`.
#[inline(always)]
fn shr128(x: [u64; 2], n: usize) -> [u64; 2] {
    match n {
        0 => x,
        1..=63 => [(x[0] >> n) | (x[1] << (64 - n)), x[1] >> n],
        64 => [x[1], 0],
        _ => [x[1] >> (n - 64), 0],
    }
}

/// `e·A(e_b)` for `b = 0..128` (the instance factor's images of the dual
/// basis): a multiply-by-`X` chain plus the seven corrections.
fn instance_dual_images(e: Gf) -> [Gf; PACK] {
    let mut pow = [Gf::zero(); PACK]; // pow[n] = e·X^n
    pow[0] = e;
    for n in 1..PACK {
        pow[n] = pow[n - 1].mul_x();
    }
    let mut g = [Gf::zero(); PACK];
    g[0] = e;
    for b in 1..PACK {
        let mut v = pow[PACK - b];
        if (2..=6).contains(&b) {
            v += pow[7 - b];
        }
        if b == 1 {
            v += pow[6] + pow[1];
        }
        g[b] = v;
    }
    g
}

/// [`phi_byte_tables`] into a caller-owned 4096-entry buffer.
fn phi_byte_tables_into(out: &mut [Gf], eq: &[Gf]) {
    debug_assert_eq!(out.len(), 16 * 256);
    debug_assert_eq!(eq.len(), PACK);
    for pos in 0..16usize {
        let tbl = &mut out[pos << 8..(pos + 1) << 8];
        tbl[0] = Gf::zero();
        for j in 0..8usize {
            let base = eq[(pos << 3) | j];
            let half = 1usize << j;
            for k in 0..half {
                tbl[half + k] = tbl[k] + base;
            }
        }
    }
}

/// The monomial `X^u`.
#[inline]
fn monomial(u: usize) -> Gf {
    let mut w = [0u64; 2];
    w[u >> 6] = 1u64 << (u & 63);
    Gf::from_words(w)
}

/// The ρ-only tables of the instance coefficients
/// `ρ′_a(e) = Σ_b ρ_b·bit_a(e·A(e_b)) = Σ_{u : bit_u(e)} C_{u,a}`,
/// `C_{u,a} = Σ_b ρ_b·bit_a(X^u·A(e_b))`, byte-indexed:
/// `T[pos][val][a] = Σ_{j ∈ val} C_{8·pos + j, a}` (8 MiB), so one instance
/// costs 16 row gathers of 128 elements.
struct RhoTables {
    t: Vec<Gf>,
}

impl RhoTables {
    fn new(rho_tables: &[Gf]) -> Self {
        let c: Vec<[Gf; PACK]> = cfg_into_iter!(0..PACK)
            .map(|u| {
                let images = instance_dual_images(monomial(u));
                let mut rows = [[0u64; 2]; PACK];
                for (row, g) in rows.iter_mut().zip(images.iter()) {
                    *row = *g.words();
                }
                let cols = transpose_128x128(&rows);
                let mut out = [Gf::zero(); PACK];
                for (slot, col) in out.iter_mut().zip(cols.iter()) {
                    *slot = phi_from_words(*col, rho_tables);
                }
                out
            })
            .collect();
        let mut t = vec![Gf::zero(); 16 * 256 * PACK];
        cfg_chunks_mut!(t, 256 * PACK)
            .enumerate()
            .for_each(|(pos, block)| {
                for j in 0..8usize {
                    let c_row = &c[(pos << 3) | j];
                    let half = 1usize << j;
                    for k in 0..half {
                        let (lower, upper) = block.split_at_mut((half + k) << LOG_PACKING);
                        let src = &lower[k << LOG_PACKING..(k + 1) << LOG_PACKING];
                        let dst = &mut upper[..PACK];
                        for ((d, s), c_ua) in dst.iter_mut().zip(src).zip(c_row.iter()) {
                            *d = *s + *c_ua;
                        }
                    }
                }
            });
        Self { t }
    }

    /// `ρ′_a(e)` for `a = 0..128` into `out`.
    #[inline]
    fn coefficients(&self, e: Gf, out: &mut [Gf]) {
        debug_assert_eq!(out.len(), PACK);
        let w = e.words();
        let lb = w[0].to_le_bytes();
        let hb = w[1].to_le_bytes();
        out.fill(Gf::zero());
        for pos in 0..16usize {
            let byte = if pos < 8 { lb[pos] } else { hb[pos - 8] } as usize;
            let row = &self.t[((pos << 8) | byte) << LOG_PACKING..][..PACK];
            for (target, value) in out.iter_mut().zip(row) {
                *target += *value;
            }
        }
    }
}

// ---------------------------------------------------------------------
// The plane engine
// ---------------------------------------------------------------------

/// The precomputed plane tables of one packed-source repetition.
pub(crate) struct PackedSourcePlanes {
    /// `w`: nonconstant source cells per instance.
    local_width: usize,
    /// `1 + w·instances`; every later source column has weight zero.
    live_cols: usize,
    /// Per chunk `l`, per instance `i`: `e_{l,i}`.
    eq_inst: Vec<Vec<Gf>>,
    /// Per chunk `l`, per phase `φ` (empty when no instance has that
    /// phase): `R_a(m)` for local pack `m`, stored at `m·128 + a` (the
    /// a′ pass walks one pack's 128 planes).
    r_tables: Vec<Vec<Vec<Gf>>>,
    /// The same tables plane-major, `R_a(m)` at `a·n_m + m` (the `h` pass
    /// sums one plane over an instance's packs).
    r_tables_by_plane: Vec<Vec<Vec<Gf>>>,
    /// Weight of the shared constant column (global column 0).
    constant_weight: Gf,
}

impl PackedSourcePlanes {
    /// Table bytes the engine would allocate for this shape.
    fn table_bytes(local_width: usize, instances: usize, chunks: usize) -> Option<usize> {
        let mut phases = [false; PACK];
        let period = PACK / gcd(local_width % PACK, PACK).max(1);
        for i in 0..instances.min(period) {
            phases[(1 + i * local_width) & (PACK - 1)] = true;
        }
        let mut entries = 0usize;
        for (phase, &used) in phases.iter().enumerate() {
            if used {
                entries = entries.checked_add(Self::local_packs(phase, local_width))?;
            }
        }
        entries
            .checked_mul(PACK)?
            .checked_mul(chunks)?
            .checked_mul(core::mem::size_of::<Gf>())
    }

    /// Whether this shape should use the plane engine: instances wide
    /// enough to amortise the per-instance `O(128²)` read-offs, tables
    /// within budget.
    pub(crate) fn eligible(local_width: usize, instances: usize, chunks: usize) -> bool {
        local_width >= MIN_LOCAL_WIDTH
            && instances >= 2
            && Self::table_bytes(local_width, instances, chunks)
                .is_some_and(|bytes| bytes <= MAX_TABLE_BYTES)
    }

    /// Global packs touched by an instance of phase `phase`.
    #[inline]
    fn local_packs(phase: usize, local_width: usize) -> usize {
        ((phase + local_width - 1) >> LOG_PACKING) + 1
    }

    /// Build the tables. `s[l]` holds the chunk-`l` local-column sums
    /// (index 0 = the constant column, `1..=w` the instance cells) and
    /// `eq_inst[l]` the instance equality table.
    pub(crate) fn new(
        local_width: usize,
        instances: usize,
        eq_inst: Vec<Vec<Gf>>,
        s: &[Vec<Gf>],
        constant_weight: Gf,
    ) -> Self {
        assert!(local_width >= 1 && instances >= 1);
        assert_eq!(eq_inst.len(), s.len());
        assert!(eq_inst.iter().all(|table| table.len() == instances));
        assert!(s.iter().all(|table| table.len() == local_width + 1));
        let live_cols = 1 + local_width * instances;
        let chunks = s.len();

        let mut phases = [false; PACK];
        let period = PACK / gcd(local_width % PACK, PACK).max(1);
        for i in 0..instances.min(period) {
            phases[(1 + i * local_width) & (PACK - 1)] = true;
        }

        // Aligned local bit planes: planes[l][l'][a] bit j = bit_a(ŝ_{l, 1 + 128l' + j}).
        let aligned = local_width.div_ceil(PACK);
        let planes: Vec<Vec<[[u64; 2]; PACK]>> = s
            .iter()
            .map(|s_l| {
                let unpacked: Vec<[u64; 2]> =
                    cfg_iter!(s_l[1..]).map(|&value| dual_unpack(value)).collect();
                cfg_into_iter!(0..aligned)
                    .map(|block| {
                        let mut rows = [[0u64; 2]; PACK];
                        let lo = block << LOG_PACKING;
                        let hi = (lo + PACK).min(local_width);
                        rows[..hi - lo].copy_from_slice(&unpacked[lo..hi]);
                        transpose_128x128(&rows)
                    })
                    .collect()
            })
            .collect();

        let mut jobs = Vec::new();
        for l in 0..chunks {
            for (phase, &used) in phases.iter().enumerate() {
                if used {
                    jobs.push((l, phase));
                }
            }
        }
        let built: Vec<((usize, usize), Vec<Gf>)> = cfg_into_iter!(jobs)
            .map(|(l, phase)| {
                let n_m = Self::local_packs(phase, local_width);
                let planes_l = &planes[l];
                let mut table = vec![Gf::zero(); n_m * PACK];
                for m in 0..n_m {
                    let cur = planes_l.get(m);
                    let prev = if m >= 1 { planes_l.get(m - 1) } else { None };
                    for a in 0..PACK {
                        let cur_a = cur.map_or([0u64; 2], |p| p[a]);
                        let u = if phase == 0 {
                            cur_a
                        } else {
                            let prev_a = prev.map_or([0u64; 2], |p| p[a]);
                            let lo = shl128(cur_a, phase);
                            let hi = shr128(prev_a, PACK - phase);
                            [lo[0] | hi[0], lo[1] | hi[1]]
                        };
                        table[(m << LOG_PACKING) | a] = dual_pack(u);
                    }
                }
                ((l, phase), table)
            })
            .collect();
        let mut r_tables: Vec<Vec<Vec<Gf>>> =
            (0..chunks).map(|_| vec![Vec::new(); PACK]).collect();
        let mut r_tables_by_plane: Vec<Vec<Vec<Gf>>> =
            (0..chunks).map(|_| vec![Vec::new(); PACK]).collect();
        for ((l, phase), table) in built {
            let n_m = table.len() >> LOG_PACKING;
            let mut by_plane = vec![Gf::zero(); table.len()];
            for m in 0..n_m {
                for a in 0..PACK {
                    by_plane[a * n_m + m] = table[(m << LOG_PACKING) | a];
                }
            }
            r_tables[l][phase] = table;
            r_tables_by_plane[l][phase] = by_plane;
        }

        Self {
            local_width,
            live_cols,
            eq_inst,
            r_tables,
            r_tables_by_plane,
            constant_weight,
        }
    }

    /// The instance runs of source pack `y`: `(instance, phase, local pack)`
    /// for every instance with cells in the pack.
    #[inline]
    fn runs(&self, y: usize, mut visit: impl FnMut(usize, usize, usize)) {
        let w = self.local_width;
        let mut column = (y << LOG_PACKING).max(1);
        let end = ((y + 1) << LOG_PACKING).min(self.live_cols);
        while column < end {
            let instance = (column - 1) / w;
            let start = 1 + instance * w;
            visit(instance, start & (PACK - 1), y - (start >> LOG_PACKING));
            column = start + w;
        }
    }

    /// The batching message `h` (128 plane openings). Instance-major within
    /// each task: every `Q_{l,i}[a]` is summed in registers over the
    /// instance's packs (plane-major tables, one fold per plane), then read
    /// off through the byte tables of `Q_{l,i}`. An instance split across
    /// tasks is read off per part — exact by linearity.
    pub(crate) fn hs_fold<T: PackedBits>(&self, p_msg: &[T]) -> Box<[Gf; PACK]> {
        let w = self.local_width;
        let live_packs = self.live_cols.div_ceil(PACK).min(p_msg.len());
        let n_tasks = live_packs.div_ceil(TASK_PACKS);
        let partials: Vec<[Gf; PACK]> = cfg_into_iter!(0..n_tasks)
            .map(|task| {
                let y_lo = task * TASK_PACKS;
                let y_hi = (y_lo + TASK_PACKS).min(live_packs);
                let mut hs = [Gf::zero(); PACK];
                if y_lo == 0 {
                    // The shared constant column: weight W₀, basis A(e₀) = 1.
                    let p0 = Gf::from_words(p_msg[0].bit_words());
                    let w0 = self.constant_weight.words();
                    for (b, h) in hs.iter_mut().enumerate() {
                        if (w0[b >> 6] >> (b & 63)) & 1 == 1 {
                            *h += p0;
                        }
                    }
                }
                let col_lo = (y_lo << LOG_PACKING).max(1);
                let col_hi = (y_hi << LOG_PACKING).min(self.live_cols);
                if col_lo >= col_hi {
                    return hs;
                }
                let mut tables = vec![Gf::zero(); 16 * 256];
                let mut fixed: Vec<kernel::Fixed> = Vec::with_capacity(TASK_PACKS.min(w / PACK + 2));
                let mut reduced = [Gf::zero(); PACK];
                for instance in (col_lo - 1) / w..=(col_hi - 2) / w {
                    let start = 1 + instance * w;
                    let phase = start & (PACK - 1);
                    let y_i = start >> LOG_PACKING;
                    let n_m = Self::local_packs(phase, w);
                    let m_lo = y_lo.max(y_i) - y_i;
                    let m_hi = y_hi.min(y_i + n_m) - y_i;
                    fixed.clear();
                    fixed.extend((m_lo..m_hi).map(|m| {
                        kernel::fixed(&Gf::from_words(p_msg[y_i + m].bit_words()))
                    }));
                    for (l, eq_inst_l) in self.eq_inst.iter().enumerate() {
                        let table = &self.r_tables_by_plane[l][phase];
                        for (a, slot) in reduced.iter_mut().enumerate() {
                            let row = &table[a * n_m + m_lo..a * n_m + m_hi];
                            let mut acc = kernel::zero();
                            for (r, f) in row.iter().zip(fixed.iter()) {
                                kernel::mul_acc(&mut acc, r, f);
                            }
                            *slot = kernel::reduce(&acc);
                        }
                        phi_byte_tables_into(&mut tables, &reduced);
                        let images = instance_dual_images(eq_inst_l[instance]);
                        for (h, g) in hs.iter_mut().zip(images.iter()) {
                            *h += phi_from_words(*g.words(), &tables);
                        }
                    }
                }
                hs
            })
            .collect();
        let mut hs = Box::new([Gf::zero(); PACK]);
        for partial in partials {
            for (target, value) in hs.iter_mut().zip(partial) {
                *target += value;
            }
        }
        hs
    }

    /// The ρ-batched dual-basis Ligerito basis `a′` over `p_msg.len()`
    /// packs, and flock's round-0 pair `(Σ_j P[2j]·a′[2j],
    /// Σ_j (P[2j]+P[2j+1])·(a′[2j]+a′[2j+1]))` (the
    /// `recursive_prover_with_basis_precomputed_round0` contract, exactly
    /// as `fill_phi_basis_round0` computes it).
    pub(crate) fn a_prime<T: PackedBits>(&self, rho: &[Gf], p_msg: &[T]) -> (Vec<Gf>, (Gf, Gf)) {
        let chunks = self.r_tables.len();
        let n_packs = p_msg.len();
        debug_assert!(n_packs.is_multiple_of(2) || n_packs == 1, "flock messages are even-sized");
        let rho_tables = phi_byte_tables(rho, Gf::one());
        let coefficient_tables = RhoTables::new(&rho_tables);
        let mut out = vec![Gf::zero(); n_packs];
        let partials: Vec<(Gf, Gf)> = cfg_chunks_mut!(out, TASK_PACKS)
            .enumerate()
            .map(|(task, slots)| {
                let y_lo = task * TASK_PACKS;
                let mut current: Option<usize> = None;
                let mut rho_p = vec![Gf::zero(); chunks * PACK];
                let mut fixed = vec![kernel::fixed(&Gf::zero()); chunks * PACK];
                for (offset, slot) in slots.iter_mut().enumerate() {
                    let y = y_lo + offset;
                    let mut acc = kernel::zero();
                    if y == 0 {
                        // Φ_ρ(W₀)·A(e₀) = Φ_ρ(W₀).
                        *slot += phi_from_words(*self.constant_weight.words(), &rho_tables);
                    }
                    self.runs(y, |instance, phase, m| {
                        if current != Some(instance) {
                            for l in 0..chunks {
                                let coeffs = &mut rho_p[l << LOG_PACKING..(l + 1) << LOG_PACKING];
                                coefficient_tables.coefficients(self.eq_inst[l][instance], coeffs);
                                for (f, c) in fixed[l << LOG_PACKING..(l + 1) << LOG_PACKING]
                                    .iter_mut()
                                    .zip(coeffs.iter())
                                {
                                    *f = kernel::fixed(c);
                                }
                            }
                            current = Some(instance);
                        }
                        for l in 0..chunks {
                            let r = &self.r_tables[l][phase]
                                [m << LOG_PACKING..(m + 1) << LOG_PACKING];
                            let f = &fixed[l << LOG_PACKING..(l + 1) << LOG_PACKING];
                            for (r_a, f_a) in r.iter().zip(f.iter()) {
                                kernel::mul_acc(&mut acc, r_a, f_a);
                            }
                        }
                    });
                    *slot += kernel::reduce(&acc);
                }
                // Flock's round-0 pair over this task's (even-aligned) pairs.
                let zero = Gf::zero();
                let mut u0 = Gf::wide_zero(&zero);
                let mut u2 = Gf::wide_zero(&zero);
                let mut j = 0usize;
                while j + 1 < slots.len() {
                    let b0 = slots[j];
                    let b1 = slots[j + 1];
                    let f0 = Gf::from_words(p_msg[y_lo + j].bit_words());
                    let f1 = Gf::from_words(p_msg[y_lo + j + 1].bit_words());
                    Gf::wide_add_assign(&mut u0, &Gf::mul_wide(&f0, &b0));
                    Gf::wide_add_assign(&mut u2, &Gf::mul_wide(&(f0 + f1), &(b0 + b1)));
                    j += 2;
                }
                (Gf::from_wide(u0), Gf::from_wide(u2))
            })
            .collect();
        let mut u0 = Gf::zero();
        let mut u2 = Gf::zero();
        for (p0, p2) in &partials {
            u0 += *p0;
            u2 += *p2;
        }
        (out, (u0, u2))
    }
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::dual_basis::{c0_bit, dual_basis_cols};

    fn splitmix(x: u64) -> u64 {
        let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn sample(seed: u64) -> Gf {
        Gf::from_words([splitmix(seed), splitmix(seed ^ 0xD1CE)])
    }

    fn bit(w: &[u64; 2], i: usize) -> u64 {
        (w[i >> 6] >> (i & 63)) & 1
    }

    /// `dual_pack` IS `Σ_v bit_v(u)·A(e_v)` for the tabulated columns.
    #[test]
    fn dual_pack_matches_columns() {
        let cols = dual_basis_cols();
        for t in 0..256u64 {
            let u = [splitmix(0x1000 + t), splitmix(0x2000 + t)];
            let mut expect = Gf::zero();
            for (v, col) in cols.iter().enumerate() {
                if bit(&u, v) == 1 {
                    expect += *col;
                }
            }
            assert_eq!(dual_pack(u), expect, "sample {t}");
        }
        for v in 0..PACK {
            let mut u = [0u64; 2];
            u[v >> 6] = 1u64 << (v & 63);
            assert_eq!(dual_pack(u), cols[v], "unit vector {v}");
        }
    }

    /// `dual_unpack` inverts `dual_pack`, and the bit-extraction identity
    /// `bit_b(e·s) = Σ_a bit_a(e·A(e_b))·bit_a(ŝ)` holds.
    #[test]
    fn dual_unpack_inverts_and_extracts_bits() {
        for t in 0..64u64 {
            let s = sample(0x3000 + t);
            let unpacked = dual_unpack(s);
            assert_eq!(dual_pack(unpacked), s, "round trip {t}");
            let e = sample(0x4000 + t);
            let product = e * s;
            let images = instance_dual_images(e);
            for b in 0..PACK {
                let g = images[b].words();
                let parity =
                    ((g[0] & unpacked[0]).count_ones() + (g[1] & unpacked[1]).count_ones()) & 1;
                assert_eq!(u64::from(parity), bit(product.words(), b), "sample {t} bit {b}");
                assert_eq!(c0_bit(product * dual_basis_cols()[b]), bit(product.words(), b));
            }
        }
    }

    #[test]
    fn instance_dual_images_match_columns() {
        let cols = dual_basis_cols();
        for t in 0..16u64 {
            let e = sample(0x5000 + t);
            let images = instance_dual_images(e);
            for b in 0..PACK {
                assert_eq!(images[b], e * cols[b], "sample {t} column {b}");
            }
        }
    }

    #[test]
    fn transpose_128x128_is_a_transpose() {
        let mut rows = [[0u64; 2]; PACK];
        for (r, row) in rows.iter_mut().enumerate() {
            *row = [splitmix(0x6000 + r as u64), splitmix(0x7000 + r as u64)];
        }
        let cols = transpose_128x128(&rows);
        for r in 0..PACK {
            for c in 0..PACK {
                assert_eq!(bit(&cols[c], r), bit(&rows[r], c), "({r}, {c})");
            }
        }
        assert_eq!(transpose_128x128(&cols), rows);
    }

    #[test]
    fn shifts_agree_with_u128() {
        for t in 0..32u64 {
            let x = [splitmix(0x8000 + t), splitmix(0x9000 + t)];
            let value = u128::from(x[0]) | (u128::from(x[1]) << 64);
            for n in 0..PACK {
                let l = shl128(x, n);
                let r = shr128(x, n);
                let lv = value << n;
                let rv = value >> n;
                assert_eq!(u128::from(l[0]) | (u128::from(l[1]) << 64), lv, "shl {n}");
                assert_eq!(u128::from(r[0]) | (u128::from(r[1]) << 64), rv, "shr {n}");
            }
        }
    }

    #[test]
    fn phi_byte_tables_into_matches() {
        let eq: Vec<Gf> = (0..PACK).map(|i| sample(0xA000 + i as u64)).collect();
        let expect = phi_byte_tables(&eq, Gf::one());
        let mut got = vec![sample(1); 16 * 256];
        phi_byte_tables_into(&mut got, &eq);
        assert_eq!(got, expect);
    }

    /// The fixed-scalar accumulator reproduces `Σ x_k·f` exactly.
    #[test]
    fn kernel_accumulates_products() {
        for t in 0..16u64 {
            let f = sample(0xB000 + t);
            let fixed = kernel::fixed(&f);
            let mut acc = kernel::zero();
            let mut expect = Gf::zero();
            for k in 0..200u64 {
                let x = sample(0xC000 + t * 1000 + k);
                kernel::mul_acc(&mut acc, &x, &fixed);
                expect += x * f;
                assert_eq!(kernel::reduce(&acc), expect, "sample {t} term {k}");
            }
        }
    }

    /// `RhoTables::coefficients` IS `Σ_b ρ_b·bit_a(e·A(e_b))`.
    #[test]
    fn rho_tables_match_transpose() {
        let rho: Vec<Gf> = (0..PACK).map(|i| sample(0xD000 + i as u64)).collect();
        let rho_tables = phi_byte_tables(&rho, Gf::one());
        let tables = RhoTables::new(&rho_tables);
        let mut got = vec![Gf::zero(); PACK];
        for t in 0..8u64 {
            let e = sample(0xE000 + t);
            tables.coefficients(e, &mut got);
            let images = instance_dual_images(e);
            for a in 0..PACK {
                let mut expect = Gf::zero();
                for (b, g) in images.iter().enumerate() {
                    if bit(g.words(), a) == 1 {
                        expect += rho[b];
                    }
                }
                assert_eq!(got[a], expect, "sample {t} coefficient {a}");
            }
        }
    }
}
