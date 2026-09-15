//! Their batched grand product, computed without materialising the leaves.
//!
//! Level `ℓ` of the tree (level 0 = leaves, `2^{m−ℓ}` entries, in-tree index
//! high, column low) has entries that are products of `2^ℓ` leaves, each
//! `1` or the row image `A(b)`, so the entry at in-tree position `y` of
//! column `c` is a function of the `2^ℓ` bits of column `c` under it —
//! a per-position table with `2^{2^ℓ}` entries. Folding the sumcheck's first
//! `k` rounds into such a table keeps that shape (`2^{2^{ℓ+k}}` entries), so
//! levels with `ℓ + k ≤ 3` run their first `k` rounds straight off the
//! packed bits through 256-entry tables, and only levels `ℓ ≥ 3` are ever
//! materialised (level 3 from its own 256-entry tables, the rest by
//! products). The round sums are the same field sums their dense pass
//! computes, so every message is byte-identical.

use std::collections::VecDeque;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::eq_factor;
use super::gkr::{Point, eq_table, prove_layer_tensor};
use super::transcript::ProverState;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::utils::wide_mul::WideMulAcc;
use crate::{cfg_chunks_mut, cfg_into_iter};

/// Levels below this run `MATERIALISED_LEVEL − ℓ` rounds off the bits;
/// this level and above are materialised.
const MATERIALISED_LEVEL: usize = 3;
const PARALLEL_MIN_LANES: usize = 1 << 12;

pub(crate) struct Forest<'a> {
    /// `log2` rows (the paper's `t`).
    t: usize,
    /// `log2` columns (`s`).
    s: usize,
    /// `packed_cols[g][b]`: bit `b` of columns `64g..64g+63`.
    packed_cols: &'a [Vec<u64>],
    /// `A(b) = g^{w_b}`, one per row.
    images: &'a [Gf],
}

impl<'a> Forest<'a> {
    pub(crate) fn new(t: usize, s: usize, packed_cols: &'a [Vec<u64>], images: &'a [Gf]) -> Self {
        assert!(t >= MATERIALISED_LEVEL + 1, "the bit-driven levels need t ≥ 4");
        assert_eq!(images.len(), 1 << t);
        assert!(packed_cols.len() >= (1usize << s).div_ceil(64));
        assert!(packed_cols.iter().all(|g| g.len() == 1 << t));
        Self {
            t,
            s,
            packed_cols,
            images,
        }
    }

    /// Their `gpgkr_prove` over this tree: from the claim at `zeta` on the
    /// roots down to the leaf point and the claimed leaf value.
    pub(crate) fn prove(&self, ps: &mut ProverState, zeta: &[Gf]) -> (Vec<Gf>, Gf) {
        // Materialise levels MATERIALISED_LEVEL..t−1, bottom-up.
        let started = std::time::Instant::now();
        let mut levels: Vec<Option<Vec<Gf>>> = (0..self.t).map(|_| None).collect();
        if MATERIALISED_LEVEL < self.t {
            let mut current = self.materialise_level(MATERIALISED_LEVEL);
            for ell in MATERIALISED_LEVEL..self.t {
                let next = if ell + 1 < self.t {
                    Some(level_up(&current))
                } else {
                    None
                };
                levels[ell] = Some(current);
                match next {
                    Some(n) => current = n,
                    None => break,
                }
            }
        }

        super::trace("  levels ≥3", started);
        let mut point: Vec<Gf> = zeta.to_owned();
        point.reverse();
        let mut point: Point = VecDeque::from(point);
        let mut claim = Gf::zero();
        for ell in (0..self.t).rev() {
            let started = std::time::Instant::now();
            (point, claim) = match levels[ell].take() {
                Some(mut wnext) => {
                    let mid = wnext.len() / 2;
                    let (l, r) = wnext.split_at_mut(mid);
                    prove_layer_tensor(ps, point, l, r, 0, Gf::one(), VecDeque::new(), self.s)
                }
                None => self.prove_bit_level(ps, ell, point),
            };
            super::trace(&format!("  level {ell}"), started);
        }
        let mut point = Vec::from(point);
        point.reverse();
        (point, claim)
    }

    /// One level below the materialised ones: `k` rounds off the bits, then
    /// the folded halves are materialised and the dense rounds continue.
    fn prove_bit_level(&self, ps: &mut ProverState, ell: usize, point: Point) -> (Point, Gf) {
        let k = (MATERIALISED_LEVEL - ell).min(self.t - ell - 1);
        let external: Vec<Gf> = point.iter().rev().copied().collect();
        let mut factor = Gf::one();
        let mut next_point = VecDeque::with_capacity(point.len() + 1);
        let mut challenges: Vec<Gf> = Vec::with_capacity(k);
        for j in 1..=k {
            let started = std::time::Instant::now();
            let z = point[j - 1];
            let send_one = z == Gf::zero();
            let (sum_endpoint, sum_inf) = self.bit_round(ell, j, &challenges, &external, send_one);
            super::trace(&format!("    L{ell} bit round {j}"), started);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r: Gf = ps.verifier_message();
            next_point.push_back(r);
            challenges.push(r);
            factor = factor * eq_factor(r, z);
        }
        let started = std::time::Instant::now();
        let mut folded = self.materialise_folded(ell, k, &challenges);
        super::trace(&format!("    L{ell} materialise"), started);
        let half = folded.len() / 2;
        let (l, r) = folded.split_at_mut(half);
        let started = std::time::Instant::now();
        let out = prove_layer_tensor(ps, point, l, r, k, factor, next_point, self.s);
        super::trace(&format!("    L{ell} dense tail"), started);
        out
    }

    /// The per-position value tables of level `ell` after `kk` folds with
    /// `r`: position `q = (p, y)` (`p` the level's product bit on top, `y`
    /// the `t − ell − 1 − kk` unbound in-tree bits), entry index = the
    /// `2^{ell+kk}`-bit pattern of the column's bits at rows
    /// `y | v ≪ (t−ell−1−kk) | p ≪ (t−ell−1) | u ≪ (t−ell)`, bit `v·2^ell + u`.
    fn fold_table(&self, ell: usize, kk: usize, r: &[Gf]) -> Vec<Vec<Gf>> {
        let t = self.t;
        let low_bits = t - ell - 1 - kk;
        let positions = 1usize << (t - ell - kk);
        let sub_entries = 1usize << (1usize << ell);
        let eq_v: Vec<Gf> = if kk == 0 {
            vec![Gf::one()]
        } else {
            let reversed: Vec<Gf> = r[..kk].iter().rev().copied().collect();
            eq_table(&reversed)
        };
        cfg_into_iter!(0..positions, 8)
            .map(|q| {
                let p = q >> low_bits;
                let y = q & ((1usize << low_bits) - 1);
                let mut table: Vec<Gf> = Vec::new();
                for v in 0..(1usize << kk) {
                    let base = y | (v << low_bits) | (p << (t - ell - 1));
                    // W_v[sub] = eq_v[v] · Π_{u ∈ sub} A(base + u·2^{t−ell}).
                    let mut w = vec![Gf::zero(); sub_entries];
                    w[0] = eq_v[v];
                    for u in 0..(1usize << ell) {
                        let a = self.images[base | (u << (t - ell))];
                        let lim = 1usize << u;
                        for sub in 0..lim {
                            w[sub | lim] = w[sub] * a;
                        }
                    }
                    if v == 0 {
                        table = w;
                    } else {
                        // Outer sum: new[x | sub ≪ (v·2^ell)] = table[x] + w[sub].
                        let mut next = Vec::with_capacity(table.len() * sub_entries);
                        for &ws in &w {
                            next.extend(table.iter().map(|&x| x + ws));
                        }
                        table = next;
                    }
                }
                table
            })
            .collect()
    }

    /// Level `ell` in full: index `y·2^s + c`.
    fn materialise_level(&self, ell: usize) -> Vec<Gf> {
        let tables = self.fold_table(ell, 0, &[]);
        let nb = 1usize << ell;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let t = self.t;
        let mut out = vec![Gf::zero(); (1usize << (t - ell)) << self.s];
        cfg_chunks_mut!(out, cols).enumerate().for_each(|(y, chunk)| {
            let table = &tables[y];
            let mut words = [0u64; 8];
            let mut pats = [0u8; 64];
            for g in 0..groups {
                for u in 0..nb {
                    words[u] = self.packed_cols[g][y | (u << (t - ell))];
                }
                patterns(&words[..nb], &mut pats);
                let base_c = g << 6;
                for j0 in 0..64.min(cols - base_c) {
                    chunk[base_c + j0] = table[pats[j0] as usize];
                }
            }
        });
        out
    }

    /// The two halves of level `ell` folded over its first `k` rounds:
    /// index `q·2^s + c`, `q = (p, y_k)`, so the first half is E.
    fn materialise_folded(&self, ell: usize, k: usize, r: &[Gf]) -> Vec<Gf> {
        let tables = self.fold_table(ell, k, r);
        let t = self.t;
        let nb = 1usize << (ell + k);
        let low_bits = t - ell - 1 - k;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let mut out = vec![Gf::zero(); (1usize << (t - ell - k)) << self.s];
        cfg_chunks_mut!(out, cols).enumerate().for_each(|(q, chunk)| {
            let table = &tables[q];
            let p = q >> low_bits;
            let y = q & ((1usize << low_bits) - 1);
            let mut words = [0u64; 8];
            let mut pats = [0u8; 64];
            for g in 0..groups {
                for v in 0..(1usize << k) {
                    for u in 0..(1usize << ell) {
                        let row = y | (v << low_bits) | (p << (t - ell - 1)) | (u << (t - ell));
                        words[(v << ell) | u] = self.packed_cols[g][row];
                    }
                }
                patterns(&words[..nb], &mut pats);
                let base_c = g << 6;
                for j0 in 0..64.min(cols - base_c) {
                    chunk[base_c + j0] = table[pats[j0] as usize];
                }
            }
        });
        out
    }

    /// Round `j` (1-based, `j ≤ k`) of level `ell`'s sumcheck off the bits:
    /// `Σ eq·E_end·O_end` and `Σ eq·(E_hi − E_lo)(O_hi − O_lo)` over the
    /// `(y_j, c)` terms, with `E_·`, `O_·` read from the `(j−1)`-fold tables.
    ///
    /// Within a row `y` each of `E_lo, E_hi, O_lo, O_hi` is a function of the
    /// column's `nb`-bit pattern, so a row's sum is `Σ_{a,b} T_E[a]·T_O[b]·
    /// Σ_{c: pat_E(c)=a, pat_O(c)=b} eq_c[c]`: the terms only bucket `eq_c`
    /// by their `(E, O)` pattern pair — four 16-byte additions per term, no
    /// multiplication — and the `2^{2nb} + 2^{nb}` products of the contraction
    /// are paid once per row (`nb ≤ 4` for every level this runs on).
    fn bit_round(
        &self,
        ell: usize,
        j: usize,
        challenges: &[Gf],
        external: &[Gf],
        send_one: bool,
    ) -> (Gf, Gf) {
        let t = self.t;
        let s = self.s;
        let kk = j - 1;
        let tables = self.fold_table(ell, kk, challenges);
        let nb = 1usize << (ell + kk);
        debug_assert!(nb <= 4, "the pair buckets need nb ≤ 4");
        let entries = 1usize << nb;
        let pairs = entries * entries;
        let y_bits = t - ell - 1 - j;
        let low_bits = y_bits + 1;
        let eq_c = eq_table(&external[..s]);
        let eq_y = eq_table(&external[s..s + y_bits]);
        let cols = 1usize << s;
        let groups = cols.div_ceil(64);
        let zero = Gf::zero();

        // Σ_a tE[a] · Σ_b tO[b] · bucket[a·entries + b], unreduced inner sums.
        let contract = |t_e: &[Gf], t_o: &[Gf], bucket: &[Gf]| -> Gf {
            let mut acc = <Gf as WideMulAcc>::wide_zero(&zero);
            for a in 0..entries {
                let mut inner = <Gf as WideMulAcc>::wide_zero(&zero);
                for b in 0..entries {
                    <Gf as WideMulAcc>::wide_add_assign(
                        &mut inner,
                        &<Gf as WideMulAcc>::mul_wide(&t_o[b], &bucket[a * entries + b]),
                    );
                }
                let inner = <Gf as WideMulAcc>::from_wide(inner);
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut acc,
                    &<Gf as WideMulAcc>::mul_wide(&t_e[a], &inner),
                );
            }
            <Gf as WideMulAcc>::from_wide(acc)
        };

        let partials: Vec<(Gf, Gf)> = cfg_into_iter!(0..(1usize << y_bits), 1)
            .map(|y| {
                // The four (p, bit) corners: table positions and their rows.
                let mut words = [[0u64; 8]; 4];
                let mut pats = [[0u8; 64]; 4];
                let mut qs = [0usize; 4];
                for (corner, q) in qs.iter_mut().enumerate() {
                    let p = corner >> 1;
                    let bit = corner & 1;
                    *q = (p << low_bits) | (bit << y_bits) | y;
                }
                // Buckets keyed by (E pattern, O pattern) for the four
                // (lo/hi, lo/hi) combinations.
                let mut b_ll = vec![zero; pairs];
                let mut b_lh = vec![zero; pairs];
                let mut b_hl = vec![zero; pairs];
                let mut b_hh = vec![zero; pairs];
                for g in 0..groups {
                    for corner in 0..4 {
                        let p = corner >> 1;
                        let bit = corner & 1;
                        let y_prev = y | (bit << y_bits);
                        for v in 0..(1usize << kk) {
                            for u in 0..(1usize << ell) {
                                let row = y_prev
                                    | (v << low_bits)
                                    | (p << (t - ell - 1))
                                    | (u << (t - ell));
                                words[corner][(v << ell) | u] = self.packed_cols[g][row];
                            }
                        }
                        patterns(&words[corner][..nb], &mut pats[corner]);
                    }
                    let base_c = g << 6;
                    for j0 in 0..64.min(cols - base_c) {
                        let (pe_lo, pe_hi) = (pats[0][j0] as usize, pats[1][j0] as usize);
                        let (po_lo, po_hi) = (pats[2][j0] as usize, pats[3][j0] as usize);
                        let ec = eq_c[base_c + j0];
                        b_ll[pe_lo * entries + po_lo] += ec;
                        b_lh[pe_lo * entries + po_hi] += ec;
                        b_hl[pe_hi * entries + po_lo] += ec;
                        b_hh[pe_hi * entries + po_hi] += ec;
                    }
                }
                let (t_e_lo, t_e_hi) = (&tables[qs[0]], &tables[qs[1]]);
                let (t_o_lo, t_o_hi) = (&tables[qs[2]], &tables[qs[3]]);
                let ll = contract(t_e_lo, t_o_lo, &b_ll);
                let hh = contract(t_e_hi, t_o_hi, &b_hh);
                let end = if send_one { hh } else { ll };
                // (E_hi − E_lo)(O_hi − O_lo) expands to the four combinations;
                // characteristic two makes every sign a plus.
                let inf = hh + contract(t_e_hi, t_o_lo, &b_hl) + contract(t_e_lo, t_o_hi, &b_lh) + ll;
                let w = eq_y[y];
                (end * w, inf * w)
            })
            .collect();
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
    }
}

/// One level up: `out[i] = lower[i] · lower[i + half]`.
fn level_up(lower: &[Gf]) -> Vec<Gf> {
    let half = lower.len() / 2;
    let (l, r) = lower.split_at(half);
    cfg_into_iter!(0..half, PARALLEL_MIN_LANES)
        .map(|i| l[i] * r[i])
        .collect()
}

/// Bit-slices `words` (one 64-column word per pattern bit, at most 8) into
/// one pattern per column: `out[c]` bit `i` = bit `c` of `words[i]`. Eight
/// 8×8 bit-block transposes (delta swaps), one per byte lane.
fn patterns(words: &[u64], out: &mut [u8; 64]) {
    debug_assert!(words.len() <= 8);
    // One or two words: walking the set bits (~32 per word) beats eight
    // block transposes; from four words on the transposes win.
    if words.len() <= 2 {
        *out = [0u8; 64];
        for (i, &w) in words.iter().enumerate() {
            let mut x = w;
            while x != 0 {
                let c = x.trailing_zeros() as usize;
                out[c] |= 1 << i;
                x &= x - 1;
            }
        }
        return;
    }
    for k in 0..8 {
        // Row i of the block = byte k of words[i] (rows past `words` are 0).
        let mut block = 0u64;
        for (i, &w) in words.iter().enumerate() {
            block |= ((w >> (8 * k)) & 0xFF) << (8 * i);
        }
        let t = transpose8x8(block);
        for j in 0..8 {
            out[8 * k + j] = ((t >> (8 * j)) & 0xFF) as u8;
        }
    }
}

/// Transposes the 8×8 bit matrix whose row `i` is byte `i` of `x` (bit `j`
/// of that byte = element `(i, j)`): the result's byte `j` holds column `j`,
/// with element `(i, j)` at bit `i`.
#[inline]
fn transpose8x8(mut x: u64) -> u64 {
    // Swap 1×1 elements across the diagonal within each 2×2 block, then
    // 2×2 blocks within 4×4, then 4×4 within 8×8. Row-major bit (8i + j)
    // and column-major bit (8j + i) differ by 7·(j − i) at each scale.
    let mut t = (x ^ (x >> 7)) & 0x00AA_00AA_00AA_00AA;
    x ^= t ^ (t << 7);
    t = (x ^ (x >> 14)) & 0x0000_CCCC_0000_CCCC;
    x ^= t ^ (t << 14);
    t = (x ^ (x >> 28)) & 0x0000_0000_F0F0_F0F0;
    x ^= t ^ (t << 28);
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_patterns(words: &[u64]) -> [u8; 64] {
        let mut out = [0u8; 64];
        for (i, &w) in words.iter().enumerate() {
            for c in 0..64 {
                out[c] |= (((w >> c) & 1) as u8) << i;
            }
        }
        out
    }

    #[test]
    fn transposed_patterns_match_the_bit_loop() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for len in 1..=8 {
            for _ in 0..64 {
                let words: Vec<u64> = (0..len)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        state
                    })
                    .collect();
                let mut out = [0u8; 64];
                patterns(&words, &mut out);
                assert_eq!(out, naive_patterns(&words), "len {len}");
            }
        }
    }
}
