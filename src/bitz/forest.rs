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
use super::gkr::{Point, eq_table, prove_layer_from};
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

        let mut point: Vec<Gf> = zeta.to_owned();
        point.reverse();
        let mut point: Point = VecDeque::from(point);
        let mut claim = Gf::zero();
        for ell in (0..self.t).rev() {
            (point, claim) = match levels[ell].take() {
                Some(mut wnext) => {
                    let mid = wnext.len() / 2;
                    let (l, r) = wnext.split_at_mut(mid);
                    prove_layer_from(ps, point, l, r, 0, Gf::one(), VecDeque::new())
                }
                None => self.prove_bit_level(ps, ell, point),
            };
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
            let z = point[j - 1];
            let send_one = z == Gf::zero();
            let (sum_endpoint, sum_inf) = self.bit_round(ell, j, &challenges, &external, send_one);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r: Gf = ps.verifier_message();
            next_point.push_back(r);
            challenges.push(r);
            factor = factor * eq_factor(r, z);
        }
        let mut folded = self.materialise_folded(ell, k, &challenges);
        let half = folded.len() / 2;
        let (l, r) = folded.split_at_mut(half);
        prove_layer_from(ps, point, l, r, k, factor, next_point)
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
        let y_bits = t - ell - 1 - j;
        let low_bits = y_bits + 1;
        let eq_c = eq_table(&external[..s]);
        let eq_y = eq_table(&external[s..s + y_bits]);
        let cols = 1usize << s;
        let groups = cols.div_ceil(64);
        let zero = Gf::zero();

        let partials: Vec<(Gf, Gf)> = cfg_into_iter!(0..(1usize << y_bits), 1)
            .map(|y| {
                let mut acc_end = <Gf as WideMulAcc>::wide_zero(&zero);
                let mut acc_inf = <Gf as WideMulAcc>::wide_zero(&zero);
                // The four (p, bit) corners: table positions and their rows.
                let mut words = [[0u64; 8]; 4];
                let mut pats = [[0u8; 64]; 4];
                let mut qs = [0usize; 4];
                for (corner, q) in qs.iter_mut().enumerate() {
                    let p = corner >> 1;
                    let bit = corner & 1;
                    *q = (p << low_bits) | (bit << y_bits) | y;
                }
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
                        let e_lo = tables[qs[0]][pats[0][j0] as usize];
                        let e_hi = tables[qs[1]][pats[1][j0] as usize];
                        let o_lo = tables[qs[2]][pats[2][j0] as usize];
                        let o_hi = tables[qs[3]][pats[3][j0] as usize];
                        let (e_end, o_end) = if send_one { (e_hi, o_hi) } else { (e_lo, o_lo) };
                        let ec = eq_c[base_c + j0];
                        let ee = ec * e_end;
                        <Gf as WideMulAcc>::wide_add_assign(
                            &mut acc_end,
                            &<Gf as WideMulAcc>::mul_wide(&ee, &o_end),
                        );
                        let ed = ec * (e_hi - e_lo);
                        <Gf as WideMulAcc>::wide_add_assign(
                            &mut acc_inf,
                            &<Gf as WideMulAcc>::mul_wide(&ed, &(o_hi - o_lo)),
                        );
                    }
                }
                let w = eq_y[y];
                (
                    <Gf as WideMulAcc>::from_wide(acc_end) * w,
                    <Gf as WideMulAcc>::from_wide(acc_inf) * w,
                )
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

/// Bit-slices `words` (one 64-column word per pattern bit) into one
/// pattern per column: `out[c]` bit `i` = bit `c` of `words[i]`.
fn patterns(words: &[u64], out: &mut [u8; 64]) {
    *out = [0u8; 64];
    for (i, &w) in words.iter().enumerate() {
        let mut x = w;
        while x != 0 {
            let c = x.trailing_zeros() as usize;
            out[c] |= 1 << i;
            x &= x - 1;
        }
    }
}
