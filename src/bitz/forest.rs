//! The batched grand product, computed without materialising the leaves.
//!
//! Level `ℓ` of the tree (level 0 = leaves, `2^{m−ℓ}` entries, in-tree index
//! high, column low) has entries that are products of `2^ℓ` leaves, each
//! `1` or the row image `A(b)`, so the entry at in-tree position `y` of
//! column `c` is a function of the `2^ℓ` bits of column `c` under it —
//! a per-position table with `2^{2^ℓ}` entries. Folding the sumcheck's first
//! `k` rounds into such a table keeps that shape (`2^{2^{ℓ+k}}` entries), so
//! levels with `ℓ + k ≤ 3` run their first `k` rounds straight off the
//! packed bits through 256-entry tables ([`Forest::bit_round`]), their next
//! two rounds through table lookups ([`Forest::jit_round_sums`],
//! [`Forest::jit_fold_round_with`] — the second writes the once-folded halves
//! into one arena shared by all these levels), and only then hand a
//! materialised table to the dense rounds. Level 3 is never materialised
//! either: its dense rounds start the same way, and level 4 is built as
//! pairwise products of its table values; the levels above are products of
//! the level below. The round sums are the same field sums their dense pass
//! computes, so every message is byte-identical.
//!
//! Every table index these levels read is eight (or fewer) bits of one
//! column at rows `y + w·2^{t−4}`, so the packed columns are regrouped once
//! per proof into [`nibble::NibbleRows`] (the sixteen such bits of each
//! `(y, column)` as one `u16`) and each pass selects its indices from them
//! with nibble lookups, instead of gathering eight strided words and
//! transposing them per 64 columns in every pass, on grids of `2^23` bits
//! and more. Smaller grids retain the gather kernel.
//!
//! Levels 0 and 1 have more than one bit round, and the buckets of their
//! last one already pair every E corner with every O corner of the bits
//! those rounds bind, so all of a level's bit rounds are read off that one
//! pass ([`Forest::one_pass_bit_rounds`]).

use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::sync::OnceLock;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub(crate) use self::nibble::NibbleRows;
use self::nibble::{BitSelectors, Selector};
use super::eq_factor;
use super::gkr::{Point, Weighing, eq_table, prove_dense_rounds, prove_layer_tensor, weighable};
use super::kernels;
use super::transcript::ProverState;
use crate::{cfg_chunks_mut, cfg_into_iter};
use field::Gf128 as Gf;

mod nibble;

/// Levels below this run `MATERIALISED_LEVEL − ℓ` rounds off the bits;
/// this level's own rounds read its tables; the levels above are
/// materialised.
const MATERIALISED_LEVEL: usize = 3;
/// How many levels above [`MATERIALISED_LEVEL`]` + 1` the build pass emits
/// from the rows it just wrote, while they are still in cache; the levels
/// above those are products of the level below, read back from memory.
const FUSED_UPPER_LEVELS: usize = 3;
const PARALLEL_MIN_LANES: usize = 1 << 12;
/// `log2` of the smallest grid (`t + s`) that reads its indices off the
/// nibble rows. At `n = 22` (`t = 13`, `s = 9`) they measured 0.92× the
/// gathers at one thread but 1.08× at ten, the excess also landing in
/// phases outside the forest (column folds, Ligerito, ring switch) with
/// the page faults unchanged; from `n = 23` on they win at every thread
/// count.
const NIBBLE_FROM: usize = 23;

/// The per-position value tables of one level after some folds, flat:
/// position `q` owns `data[q·len..(q+1)·len]`.
pub(crate) struct Tables {
    data: Vec<Gf>,
    len: usize,
}

impl Tables {
    fn at(&self, q: usize) -> &[Gf] {
        &self.data[q * self.len..(q + 1) * self.len]
    }

    /// Absorb the next fold into each position table once, so columns can
    /// evaluate `(1-rho)·T0[a] + rho·T1[b]` with two lookups and an XOR.
    fn scale_fold(&mut self, low_bits: usize, rho: Gf) {
        let side_len = (1usize << (low_bits - 1)) * self.len;
        let chunk_len = PARALLEL_MIN_LANES.min(side_len);
        let weights = [Gf::one() - rho, rho];
        cfg_chunks_mut!(self.data, chunk_len)
            .enumerate()
            .for_each(|(i, values)| {
                let side = (i * chunk_len / side_len) & 1;
                kernels::scale_in_place(values, &weights[side]);
            });
    }
}

/// How the table-driven levels obtain their table indices (the proof is the
/// same either way).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PatternMode {
    /// Read every index off the [`NibbleRows`] built once per proof instead
    /// of gathering and transposing the packed words in every pass.
    pub(crate) nibble: bool,
    /// The nibble rows serve grids of `2^{nibble_from}` bits and more;
    /// smaller ones gather.
    pub(crate) nibble_from: usize,
}

impl PatternMode {
    /// The measured defaults, with overrides confined to kernel tests.
    const fn measured() -> Self {
        Self {
            nibble: true,
            nibble_from: NIBBLE_FROM,
        }
    }
}

/// The nibble rows a forest over this grid reads its indices off (the rule
/// of [`Forest::nibbles`] for the environment's [`PatternMode`]), built
/// ahead of it so the column folds can read them too
/// ([`NibbleRows::column_folds`]); `None` where the forest gathers.
pub(crate) fn nibble_rows_for(t: usize, s: usize, packed_cols: &[Vec<u64>]) -> Option<NibbleRows> {
    let mode = PatternMode::measured();
    (mode.nibble && t >= MATERIALISED_LEVEL + 3 && t + s >= mode.nibble_from)
        .then(|| NibbleRows::new(t, packed_cols, (1usize << s).div_ceil(64)))
}

pub(crate) struct Forest<'a> {
    /// `log2` rows (the paper's `t`).
    t: usize,
    /// `log2` columns (`s`).
    s: usize,
    /// `packed_cols[g][b]`: bit `b` of columns `64g..64g+63`.
    packed_cols: &'a [Vec<u64>],
    /// `A(b) = g^{w_b}`, one per row.
    images: &'a [Gf],
    mode: PatternMode,
    /// Levels 0 and 1 run their bit rounds off one pass
    /// ([`Forest::one_pass_bit_rounds`]).
    one_pass: bool,
    /// Whether the dense rounds keep the column weights in the left half
    /// ([`Weighing`]); the table-driven levels' JIT fold writes it so.
    weighing: Weighing,
    /// The packed columns regrouped for the table-driven levels, built on
    /// first use when `mode.nibble`.
    nibble_rows: OnceLock<NibbleRows>,
}

impl<'a> Forest<'a> {
    pub(crate) fn new(t: usize, s: usize, packed_cols: &'a [Vec<u64>], images: &'a [Gf]) -> Self {
        assert!(
            t >= MATERIALISED_LEVEL + 1,
            "the bit-driven levels need t ≥ 4"
        );
        assert_eq!(images.len(), 1 << t);
        assert!(packed_cols.len() >= (1usize << s).div_ceil(64));
        assert!(packed_cols.iter().all(|g| g.len() == 1 << t));
        Self {
            t,
            s,
            packed_cols,
            images,
            mode: PatternMode::measured(),
            one_pass: true,
            weighing: Weighing::On,
            nibble_rows: OnceLock::new(),
        }
    }

    /// The same forest with its indices read the way `mode` says.
    #[cfg(test)]
    pub(crate) fn with_patterns(mut self, mode: PatternMode) -> Self {
        self.mode = mode;
        self
    }

    /// The same forest with the nibble rows [`nibble_rows_for`] built for its
    /// grid (the column folds read them first).
    pub(crate) fn with_nibble_rows(mut self, rows: NibbleRows) -> Self {
        assert!(
            rows.fits(self.t, (1usize << self.s).div_ceil(64)),
            "nibble rows of another grid"
        );
        self.nibble_rows = OnceLock::from(rows);
        self
    }

    /// The same forest with its bit rounds taken from one pass or not.
    #[cfg(test)]
    pub(crate) fn with_one_pass(mut self, one_pass: bool) -> Self {
        self.one_pass = one_pass;
        self
    }

    /// The same forest with its dense rounds weighing the way `weighing`
    /// says (`On` or `Off`).
    #[cfg(test)]
    pub(crate) fn with_weighing(mut self, weighing: Weighing) -> Self {
        assert_ne!(weighing, Weighing::Carried, "a forest's layers start plain");
        self.weighing = weighing;
        self
    }

    /// The nibble rows, when the mode uses them, the grid has at least
    /// `2^{nibble_from}` bits and the table-driven levels run their JIT
    /// rounds (`t ≥ 6`; the small-`t` path gathers). Call outside parallel
    /// regions: the first call builds them.
    fn nibbles(&self) -> Option<&NibbleRows> {
        (self.mode.nibble && self.jit() && self.t + self.s >= self.mode.nibble_from).then(|| {
            self.nibble_rows.get_or_init(|| {
                NibbleRows::new(self.t, self.packed_cols, (1usize << self.s).div_ceil(64))
            })
        })
    }

    /// Corners `(0, y)` and `(1, y)` of level `ell`'s JIT patterns
    /// (`ell + kk = 3`, `y < 2^{t−4}`) in group `g`: selected from the
    /// nibble rows with level `ell`'s selectors, or gathered and transposed.
    #[inline(always)]
    fn jit_patterns(
        &self,
        ell: usize,
        y: usize,
        g: usize,
        nib: Option<(&NibbleRows, &[Selector; 2])>,
    ) -> [[u8; 64]; 2] {
        let mut out = [[0u8; 64]; 2];
        match nib {
            Some((rows, sel)) => nibble::select(rows.block(y, g), sel, &mut out),
            None => {
                let mut words = [0u64; 8];
                for (p, block) in out.iter_mut().enumerate() {
                    self.corner_words(ell, MATERIALISED_LEVEL - ell, p, y, g, &mut words);
                    *block = transposed_patterns(&mut words);
                }
            }
        }
        out
    }

    /// Whether the table-driven levels have two in-tree rounds left after
    /// their bit rounds (`t − 4 ≥ 2`); below that they are materialised.
    fn jit(&self) -> bool {
        self.t >= MATERIALISED_LEVEL + 3
    }

    /// Their `gpgkr_prove` over this tree: from the claim at `zeta` on the
    /// roots down to the leaf point and the claimed leaf value.
    pub(crate) fn prove(&self, ps: &mut ProverState, zeta: &[Gf]) -> (Vec<Gf>, Gf) {
        let t = self.t;
        let started = std::time::Instant::now();
        // The materialised path for tiny `t`: level 3 in full, the levels
        // above it by products.
        let mut levels: Vec<Option<Vec<Gf>>> = (0..t).map(|_| None).collect();
        // One arena of `2^{t−4+s}` entries, touched once: it first holds
        // levels 5..t−1 (built from level-4 rows that live only in cache),
        // is rebuilt as level 4 once those are proved, and then takes every
        // table-driven level's once-folded halves. Nothing larger than the
        // level-3 tables is allocated after it, so the prover's peak is the
        // arena plus those tables. (Writing level 4 in the same pass into
        // a second buffer instead saves the 6.3 ms rebuild less the 2.6 ms
        // of writes it skips — 1.2 % of the prove with a warm allocator —
        // for `2^{t−4+s}` more entries of peak: 256 MB here, 4 GB at
        // n = 32.)
        let mut arena: Vec<Gf> = Vec::new();
        let prebuilt = self.nibble_rows.get().is_some();
        if self.nibbles().is_some() && !prebuilt {
            super::trace("    nibble rows", started);
        }
        let mut tables3 = if self.jit() {
            let started = std::time::Instant::now();
            let tables = self.fold_table(MATERIALISED_LEVEL, 0, &[]);
            super::trace("    L3 value tables", started);
            let started = std::time::Instant::now();
            arena = Vec::with_capacity(1usize << (t - MATERIALISED_LEVEL - 1 + self.s));
            let fused = (t - MATERIALISED_LEVEL - 2).min(FUSED_UPPER_LEVELS);
            self.upper_levels_into(&tables, &mut arena, fused);
            super::trace(
                &format!("    levels {}..{} build", MATERIALISED_LEVEL + 2, t - 1),
                started,
            );
            Some(tables)
        } else {
            levels[MATERIALISED_LEVEL] = Some(self.materialise_level(MATERIALISED_LEVEL));
            for ell in MATERIALISED_LEVEL..t - 1 {
                let next = level_up(levels[ell].as_deref().expect("built"));
                levels[ell + 1] = Some(next);
            }
            None
        };
        super::trace("  levels ≥4", started);

        let mut point: Vec<Gf> = zeta.to_owned();
        point.reverse();
        let mut point: Point = VecDeque::from(point);
        let mut claim = Gf::zero();
        // Each table-driven level's tables have `2^{t+5}` entries; the buffer
        // goes from level 3 down to level 0 instead of a fresh one per level.
        let mut spare_tables: Vec<Gf> = Vec::new();
        for ell in (0..t).rev() {
            let started = std::time::Instant::now();
            (point, claim) = if let Some(mut wnext) = levels[ell].take() {
                let mid = wnext.len() / 2;
                let (l, r) = wnext.split_at_mut(mid);
                prove_layer_tensor(
                    ps,
                    point,
                    l,
                    r,
                    0,
                    Gf::one(),
                    VecDeque::new(),
                    self.s,
                    self.weighing,
                )
            } else if ell >= MATERIALISED_LEVEL + 2 {
                let region = &mut arena[self.upper_region(ell)];
                let mid = region.len() / 2;
                let (l, r) = region.split_at_mut(mid);
                prove_layer_tensor(
                    ps,
                    point,
                    l,
                    r,
                    0,
                    Gf::one(),
                    VecDeque::new(),
                    self.s,
                    self.weighing,
                )
            } else if ell == MATERIALISED_LEVEL + 1 {
                // Level 4 rebuilt in place of the proved upper levels. (Its
                // first round fused into the rebuild was measured slower:
                // the rebuild is compute-bound, so the round's slots no
                // longer hide behind a memory stream.)
                let rebuilt = std::time::Instant::now();
                arena.clear();
                self.product_level_into(tables3.as_ref().expect("jit"), &mut arena);
                super::trace("    L4 rebuild", rebuilt);
                let mid = arena.len() / 2;
                let (l, r) = arena.split_at_mut(mid);
                prove_layer_tensor(
                    ps,
                    point,
                    l,
                    r,
                    0,
                    Gf::one(),
                    VecDeque::new(),
                    self.s,
                    self.weighing,
                )
            } else if self.jit() {
                let tables = if ell == MATERIALISED_LEVEL {
                    tables3.take()
                } else {
                    None
                };
                self.prove_jit_level(ps, ell, point, tables, &mut spare_tables, &mut arena)
            } else {
                self.prove_bit_level(ps, ell, point)
            };
            super::trace(&format!("  level {ell}"), started);
        }
        let mut point = Vec::from(point);
        point.reverse();
        (point, claim)
    }

    /// Where level `ell ≥ 5` lives in the arena while the upper levels are
    /// proved: level 5 first, each level after the one below it.
    fn upper_region(&self, ell: usize) -> std::ops::Range<usize> {
        let t = self.t;
        debug_assert!(ell >= MATERIALISED_LEVEL + 2 && ell < t);
        let cols = 1usize << self.s;
        let offset = ((1usize << (t - MATERIALISED_LEVEL - 1)) - (1usize << (t - ell + 1))) * cols;
        offset..offset + ((1usize << (t - ell)) * cols)
    }

    /// The `k = 3 − ℓ` bit rounds of level `ℓ`; returns the challenges, the
    /// accumulated eq factor and the next point so far.
    fn bit_rounds(
        &self,
        ps: &mut ProverState,
        ell: usize,
        k: usize,
        point: &Point,
        external: &[Gf],
    ) -> (Vec<Gf>, Gf, VecDeque<Gf>) {
        if self.one_pass && k >= 2 {
            return self.one_pass_bit_rounds(ps, ell, k, point, external);
        }
        let mut factor = Gf::one();
        let mut next_point = VecDeque::with_capacity(point.len() + 1);
        let mut challenges: Vec<Gf> = Vec::with_capacity(k);
        for j in 1..=k {
            let started = std::time::Instant::now();
            let z = point[j - 1];
            let send_one = z == Gf::zero();
            let (sum_endpoint, sum_inf) = self.bit_round(ell, j, &challenges, external, send_one);
            super::trace(&format!("    L{ell} bit round {j}"), started);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r: Gf = ps.native_scalar(super::grinding::Stage::GkrRound);
            next_point.push_back(r);
            challenges.push(r);
            factor = factor * eq_factor(r, z);
        }
        (challenges, factor, next_point)
    }

    /// [`Forest::bit_rounds`] for a level with `k ≥ 2` of them, all read off
    /// one pass.
    ///
    /// Round `j`'s two sums are bilinear in E and O folded over the
    /// challenges `r_1..r_{j−1}`, so they are fixed combinations — weights
    /// `eq(r, ·)` on both corners' already-bound bits, `eq(z, ·)` on the bits
    /// still to come — of the cross sums
    /// `Ĉ[α][β] = Σ_{y,c} eq_y[y]·eq_c[c]·E_α(y, c)·O_β(y, c)` over the
    /// corners `α, β ∈ {0,1}^k` of the bits these rounds bind
    /// ([`Forest::cross_sums`]). Those need only the last round's scatters,
    /// done once before the first message: the messages are the per-round
    /// ones, byte for byte (the small-value accumulators of Bagad, Dao,
    /// Domb and Thaler, "Speeding Up Sum-Check Proving", Algorithm 4, read
    /// off buckets).
    fn one_pass_bit_rounds(
        &self,
        ps: &mut ProverState,
        ell: usize,
        k: usize,
        point: &Point,
        external: &[Gf],
    ) -> (Vec<Gf>, Gf, VecDeque<Gf>) {
        let started = std::time::Instant::now();
        let cross = self.cross_sums(ell, k, external);
        super::trace(&format!("    L{ell} one-pass bit rounds"), started);
        let z: Vec<Gf> = (0..k).map(|i| point[i]).collect();
        let mut factor = Gf::one();
        let mut next_point = VecDeque::with_capacity(point.len() + 1);
        let mut challenges: Vec<Gf> = Vec::with_capacity(k);
        for j in 1..=k {
            let send_one = z[j - 1] == Gf::zero();
            let (sum_endpoint, sum_inf) = cross_round_sums(&cross, k, j, &z, &challenges, send_one);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r: Gf = ps.native_scalar(super::grinding::Stage::GkrRound);
            next_point.push_back(r);
            challenges.push(r);
            factor = factor * eq_factor(r, z[j - 1]);
        }
        (challenges, factor, next_point)
    }

    /// The cross sums of level `ell`'s `k = 3 − ell ≥ 2` bit rounds, entry
    /// `α·2^k + β`, corner `α = (b_1 … b_k)` with `b_1` (the first round's
    /// bit) high.
    ///
    /// A term of the last round is a row `y` of `t − 4` bits and a column:
    /// sixteen leaves, the `2^{ℓ+k−1}` of each of its four corners
    /// `(p, b_k)`. Each of that round's pair bytes holds the E pattern of
    /// every corner with `b_k = x` and the O pattern of every corner with
    /// `b_k = x'` (corner `(v, x)`'s leaves `u` at bits `(v << ℓ) | u` of its
    /// nibble), so scattering `eq_c` by the four bytes, exactly as that round
    /// does, and contracting each row's buckets with the unfolded corner
    /// tables ([`cross_row_leaves`], [`cross_row_pairs`]) gives every
    /// `Ĉ[α][β]`.
    fn cross_sums(&self, ell: usize, k: usize, external: &[Gf]) -> [Gf; 64] {
        debug_assert!(ell + k == MATERIALISED_LEVEL && k >= 2);
        let t = self.t;
        let s = self.s;
        let kk = k - 1;
        let y_bits = t - 4;
        let groups = (1usize << s).div_ceil(64);
        let eq_t = transposed_eq(&eq_table(&external[..s]));
        let eq_y = eq_table(&external[s..s + y_bits]);
        let rows_nib = self.nibbles();
        let selectors = rows_nib.map(|_| BitSelectors::new(ell, k, false));
        let nib = rows_nib.zip(selectors.as_ref().map(|sel| match sel {
            BitSelectors::Pairs(sel) => sel,
            _ => unreachable!("the last bit round reads pair bytes"),
        }));
        let row = |y: usize, bk: &mut Buckets, acc: &mut [Gf; 64]| {
            bk.reset(4);
            for g in 0..groups {
                let idx = self.pair_bytes(ell, kk, y, y_bits, g, nib);
                let eq_g = &eq_t[g << 6..(g + 1) << 6];
                kernels::scatter_add4([&mut bk.ll, &mut bk.hh, &mut bk.lh, &mut bk.hl], &idx, eq_g);
            }
            // Corner `α = (v, x)`'s leaf `u` for half `p`.
            let image = |p: usize, alpha: usize, u: usize| {
                let (v, x) = (alpha >> 1, alpha & 1);
                self.images[y
                    | (x << y_bits)
                    | (v << (y_bits + 1))
                    | (p << (t - ell - 1))
                    | (u << (t - ell))]
            };
            if ell == 0 {
                let delta = |p: usize| -> [Gf; 8] {
                    std::array::from_fn(|alpha| image(p, alpha, 0) - Gf::one())
                };
                cross_row_leaves(bk, &delta(0), &delta(1), eq_y[y], acc);
            } else {
                let tables = |p: usize| -> [[Gf; 4]; 4] {
                    std::array::from_fn(|alpha| {
                        let (a0, a1) = (image(p, alpha, 0), image(p, alpha, 1));
                        [Gf::one(), a0, a1, a0 * a1]
                    })
                };
                cross_row_pairs(bk, &tables(0), &tables(1), eq_y[y], acc);
            }
        };
        let rows = 1usize << y_bits;
        let add = |mut a: [Gf; 64], b: [Gf; 64]| {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
            a
        };
        #[cfg(feature = "parallel")]
        let cross = (0..rows)
            .into_par_iter()
            .with_min_len(1)
            .fold(
                || (Buckets::new(), [Gf::zero(); 64]),
                |(mut bk, mut acc), y| {
                    row(y, &mut bk, &mut acc);
                    (bk, acc)
                },
            )
            .map(|(_, acc)| acc)
            .reduce(|| [Gf::zero(); 64], add);
        #[cfg(not(feature = "parallel"))]
        let cross = {
            let _ = add;
            let mut bk = Buckets::new();
            let mut acc = [Gf::zero(); 64];
            for y in 0..rows {
                row(y, &mut bk, &mut acc);
            }
            acc
        };
        cross
    }

    /// The four pair bytes `ll, hh, lh, hl` of the last bit round (`nb = 4`)
    /// of level `ell` at row `y`, group `g` — each its O pattern low and its
    /// E pattern high, as [`Forest::bit_row`] builds them.
    #[inline(always)]
    fn pair_bytes(
        &self,
        ell: usize,
        kk: usize,
        y: usize,
        y_bits: usize,
        g: usize,
        nib: Option<(&NibbleRows, &[Selector; 4])>,
    ) -> [[u8; 64]; 4] {
        let mut idx = [[0u8; 64]; 4];
        if let Some((rows, sel)) = nib {
            // The last round's rows are below `2^{t−4}`: `q = 0`.
            nibble::select(rows.block(y, g), sel, &mut idx);
            return idx;
        }
        let corner_y = |corner: usize| y | ((corner & 1) << y_bits);
        let mut tmp = [0u64; 8];
        let mut words = [0u64; 8];
        for (out, e_corner, o_corner) in [(0usize, 0usize, 2usize), (1, 1, 3)] {
            self.corner_words(ell, kk, o_corner >> 1, corner_y(o_corner), g, &mut tmp);
            words[..4].copy_from_slice(&tmp[..4]);
            self.corner_words(ell, kk, e_corner >> 1, corner_y(e_corner), g, &mut tmp);
            words[4..8].copy_from_slice(&tmp[..4]);
            idx[out] = transposed_patterns(&mut words);
        }
        for m in 0..64 {
            idx[2][m] = (idx[0][m] & 0xF0) | (idx[1][m] & 0x0F);
            idx[3][m] = (idx[1][m] & 0xF0) | (idx[0][m] & 0x0F);
        }
        idx
    }

    /// A level at or below [`MATERIALISED_LEVEL`]: `k` rounds off the bits,
    /// two rounds through the `k`-fold tables (the second writing the
    /// once-folded halves into `arena`), then the dense rounds on the arena.
    #[allow(clippy::too_many_arguments)]
    fn prove_jit_level(
        &self,
        ps: &mut ProverState,
        ell: usize,
        point: Point,
        tables: Option<Tables>,
        spare_tables: &mut Vec<Gf>,
        arena: &mut Vec<Gf>,
    ) -> (Point, Gf) {
        let t = self.t;
        let s = self.s;
        let k = MATERIALISED_LEVEL - ell;
        let external: Vec<Gf> = point.iter().rev().copied().collect();
        let (challenges, mut factor, mut next_point) =
            self.bit_rounds(ps, ell, k, &point, &external);
        let low_bits = t - ell - 1 - k;
        debug_assert!(low_bits >= 2);

        let started = std::time::Instant::now();
        let mut tables = match tables {
            Some(tables) if k == 0 => tables,
            _ => self.fold_table_in(ell, k, &challenges, std::mem::take(spare_tables)),
        };
        let eq_c = eq_table(&external[..s]);
        super::trace(&format!("    L{ell} tables"), started);

        // Round k + 1 off the tables.
        let started = std::time::Instant::now();
        let z = point[k];
        let send_one = z == Gf::zero();
        let eq_y = eq_table(&external[s..s + low_bits - 1]);
        let (sum_endpoint, sum_inf) = self.jit_round_sums(&tables, ell, k, &eq_c, &eq_y, send_one);
        super::trace(&format!("    L{ell} jit round"), started);
        ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
        let r1: Gf = ps.native_scalar(super::grinding::Stage::GkrRound);
        next_point.push_back(r1);
        factor = factor * eq_factor(r1, z);

        // Round k + 2 writes the folded halves to the arena. On wide rows,
        // pre-scale the shared tables instead of multiplying every column;
        // narrow rows retain the in-register fold to avoid the extra pass.
        let started = std::time::Instant::now();
        let z = point[k + 1];
        let send_one = z == Gf::zero();
        let eq_y = eq_table(&external[s..s + low_bits - 2]);
        // Scaling costs one multiply per table entry; the original fold
        // costs one per pair of tables per column. Require at least a 2x
        // reduction in those multiplies to offset the table read/write pass.
        let pre_scaled = (1usize << s) >= 4 * tables.len;
        if pre_scaled {
            tables.scale_fold(low_bits, r1);
        }
        // Carrying the column weights, the fold stores `eq_c·E'` (its
        // slots' own two multiplies, moved) and the dense rounds skip them.
        let carry = self.weighing != Weighing::Off && weighable(&external, s);
        let fold = |arena: &mut Vec<Gf>| match (pre_scaled, carry) {
            (true, true) => self.jit_fold_round_with::<true, true>(
                &tables, ell, k, r1, &eq_c, &eq_y, send_one, arena,
            ),
            (true, false) => self.jit_fold_round_with::<true, false>(
                &tables, ell, k, r1, &eq_c, &eq_y, send_one, arena,
            ),
            (false, true) => self.jit_fold_round_with::<false, true>(
                &tables, ell, k, r1, &eq_c, &eq_y, send_one, arena,
            ),
            (false, false) => self.jit_fold_round_with::<false, false>(
                &tables, ell, k, r1, &eq_c, &eq_y, send_one, arena,
            ),
        };
        let (sum_endpoint, sum_inf) = fold(arena);
        super::trace(&format!("    L{ell} jit fold"), started);
        ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
        let r2: Gf = ps.native_scalar(super::grinding::Stage::GkrRound);
        next_point.push_back(r2);
        factor = factor * eq_factor(r2, z);
        // The tables are done with: their buffer serves the next level.
        *spare_tables = std::mem::take(&mut tables.data);

        let started = std::time::Instant::now();
        let half = arena.len() / 2;
        let (l, r) = arena.split_at_mut(half);
        let weighing = if carry {
            Weighing::Carried
        } else {
            self.weighing
        };
        let out = prove_dense_rounds(
            ps,
            point,
            l,
            r,
            k + 2,
            Some(r2),
            factor,
            next_point,
            s,
            weighing,
        );
        super::trace(&format!("    L{ell} dense tail"), started);
        out
    }

    /// The pre-arena path for tiny `t`: `k` rounds off the bits, then the
    /// folded halves are materialised and the dense rounds continue.
    fn prove_bit_level(&self, ps: &mut ProverState, ell: usize, point: Point) -> (Point, Gf) {
        let k = (MATERIALISED_LEVEL - ell).min(self.t - ell - 1);
        let external: Vec<Gf> = point.iter().rev().copied().collect();
        let (challenges, factor, next_point) = self.bit_rounds(ps, ell, k, &point, &external);
        let mut folded = self.materialise_folded(ell, k, &challenges);
        let half = folded.len() / 2;
        let (l, r) = folded.split_at_mut(half);
        prove_layer_tensor(
            ps,
            point,
            l,
            r,
            k,
            factor,
            next_point,
            self.s,
            self.weighing,
        )
    }

    /// The row of corner `(p, y)` of level `ell` after `kk` folds, pattern
    /// bit `(v, u)`.
    #[inline]
    fn corner_row(&self, ell: usize, kk: usize, p: usize, y: usize, v: usize, u: usize) -> usize {
        let t = self.t;
        let low_bits = t - ell - 1 - kk;
        y | (v << low_bits) | (p << (t - ell - 1)) | (u << (t - ell))
    }

    /// The `2^{ell+kk}` words of column group `g` at corner `(p, y)`, in
    /// pattern-bit order.
    #[inline]
    fn corner_words(
        &self,
        ell: usize,
        kk: usize,
        p: usize,
        y: usize,
        g: usize,
        words: &mut [u64; 8],
    ) {
        let col = &self.packed_cols[g];
        for v in 0..(1usize << kk) {
            for u in 0..(1usize << ell) {
                words[(v << ell) | u] = col[self.corner_row(ell, kk, p, y, v, u)];
            }
        }
    }

    /// The per-position value tables of level `ell` after `kk` folds with
    /// `r`: position `q = (p, y)` (`p` the level's product bit on top, `y`
    /// the `t − ell − 1 − kk` unbound in-tree bits), entry index = the
    /// `2^{ell+kk}`-bit pattern of the column's bits at rows
    /// `y | v ≪ (t−ell−1−kk) | p ≪ (t−ell−1) | u ≪ (t−ell)`, bit `v·2^ell + u`.
    fn fold_table(&self, ell: usize, kk: usize, r: &[Gf]) -> Tables {
        self.fold_table_in(ell, kk, r, Vec::new())
    }

    /// [`Forest::fold_table`] into `buf`'s allocation (its contents are
    /// dropped): each position's table is built in a stack buffer — the
    /// `2^kk` factor tables, then their outer sum in place — and written
    /// out once.
    fn fold_table_in(&self, ell: usize, kk: usize, r: &[Gf], mut buf: Vec<Gf>) -> Tables {
        let t = self.t;
        let low_bits = t - ell - 1 - kk;
        let positions = 1usize << (t - ell - kk);
        let sub_entries = 1usize << (1usize << ell);
        let len = 1usize << (1usize << (ell + kk));
        assert!(len <= 256, "a position table has at most 256 entries");
        let eq_v: Vec<Gf> = if kk == 0 {
            vec![Gf::one()]
        } else {
            let reversed: Vec<Gf> = r[..kk].iter().rev().copied().collect();
            eq_table(&reversed)
        };
        buf.clear();
        buf.reserve(positions * len);
        let spare = &mut buf.spare_capacity_mut()[..positions * len];
        cfg_chunks_mut!(spare, len)
            .enumerate()
            .for_each(|(q, out)| {
                let p = q >> low_bits;
                let y = q & ((1usize << low_bits) - 1);
                let mut table = [Gf::zero(); 256];
                let mut w = [Gf::zero(); 256];
                let mut filled = 1usize;
                for v in 0..(1usize << kk) {
                    let base = y | (v << low_bits) | (p << (t - ell - 1));
                    // W_v[sub] = eq_v[v] · Π_{u ∈ sub} A(base + u·2^{t−ell}).
                    w[0] = eq_v[v];
                    for u in 0..(1usize << ell) {
                        let a = self.images[base | (u << (t - ell))];
                        let lim = 1usize << u;
                        for sub in 0..lim {
                            w[sub | lim] = w[sub] * a;
                        }
                    }
                    if v == 0 {
                        table[..sub_entries].copy_from_slice(&w[..sub_entries]);
                    } else {
                        // Outer sum: new[x | sub ≪ (v·2^ell)] = table[x] + w[sub],
                        // the highest `sub` first so every block reads the old
                        // first one.
                        for sub in (0..sub_entries).rev() {
                            for x in 0..filled {
                                table[sub * filled + x] = table[x] + w[sub];
                            }
                        }
                    }
                    filled *= sub_entries;
                }
                debug_assert_eq!(filled, len);
                for (slot, &value) in out.iter_mut().zip(&table[..len]) {
                    slot.write(value);
                }
            });
        // SAFETY: every one of the `positions · len` slots was written above.
        unsafe { buf.set_len(positions * len) };
        Tables { data: buf, len }
    }

    /// Level `ell` in full: index `y·2^s + c`.
    fn materialise_level(&self, ell: usize) -> Vec<Gf> {
        let tables = self.fold_table(ell, 0, &[]);
        let nb = 1usize << ell;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let mut out = vec![Gf::zero(); (1usize << (self.t - ell)) << self.s];
        cfg_chunks_mut!(out, cols)
            .enumerate()
            .for_each(|(y, chunk)| {
                let table = tables.at(y);
                let mut words = [0u64; 8];
                let mut pats = [0u8; 64];
                for g in 0..groups {
                    self.corner_words(ell, 0, 0, y, g, &mut words);
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
        cfg_chunks_mut!(out, cols)
            .enumerate()
            .for_each(|(q, chunk)| {
                let table = tables.at(q);
                let p = q >> low_bits;
                let y = q & ((1usize << low_bits) - 1);
                let mut words = [0u64; 8];
                let mut pats = [0u8; 64];
                for g in 0..groups {
                    self.corner_words(ell, k, p, y, g, &mut words);
                    patterns(&words[..nb], &mut pats);
                    let base_c = g << 6;
                    for j0 in 0..64.min(cols - base_c) {
                        chunk[base_c + j0] = table[pats[j0] as usize];
                    }
                }
            });
        out
    }

    /// Rebuilds level 4 into the reusable arena.
    fn product_level_into(&self, tables: &Tables, out: &mut Vec<Gf>) {
        let t = self.t;
        let ell = MATERIALISED_LEVEL;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let rows = 1usize << (t - ell - 1);
        let len = rows * cols;
        assert!(out.is_empty() && out.capacity() >= len);
        let sel = nibble::jit_selectors(ell);
        let nib = self.nibbles().map(|rows| (rows, &sel));
        let spare = &mut out.spare_capacity_mut()[..len];
        cfg_chunks_mut!(spare, cols)
            .enumerate()
            .for_each(|(y, chunk)| {
                let tab = [tables.at(y), tables.at(y | (1 << (t - ell - 1)))];
                for g in 0..groups {
                    let pats = self.jit_patterns(ell, y, g, nib);
                    let base_c = g << 6;
                    let width = 64.min(cols - base_c);
                    kernels::jit_product_group(tab, &pats, &mut chunk[base_c..base_c + width]);
                }
            });
        // SAFETY: every slot of every row chunk was written by the kernel.
        unsafe { out.set_len(len) };
    }

    /// Levels 5..t−1 into the (empty, pre-sized) `arena`, each at its
    /// [`Forest::upper_region`]: the first `fused` of them from level-4
    /// rows computed 64 columns at a time into a task-local buffer and never
    /// stored — a task owns level 4's rows `y0 + b·2^{t−4−fused}` for every
    /// `b`, the whole subtree above row `y0` of level `4 + fused`, so every
    /// product it emits reads rows it computed itself — and the levels
    /// above those as products of the level below. Level 4 itself is
    /// rebuilt into the arena when its turn comes
    /// ([`Forest::product_level_into`]).
    fn upper_levels_into(&self, tables: &Tables, arena: &mut Vec<Gf>, fused: usize) {
        let t = self.t;
        let ell = MATERIALISED_LEVEL;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let rows = 1usize << (t - ell - 1);
        let total = self.upper_region(t - 1).end;
        assert!(arena.is_empty() && arena.capacity() >= total);
        assert!(
            fused >= 1 && fused + ell + 2 <= t,
            "level {} does not exist",
            ell + 1 + fused
        );
        let sel = nibble::jit_selectors(ell);
        let nib = self.nibbles().map(|rows| (rows, &sel));
        let spare = &mut arena.spare_capacity_mut()[..total];
        // Rows of level `4 + fused` = tasks; level `4 + i` has `sub ≪ (fused − i)`.
        let sub = rows >> fused;
        // One pointer per level, each carved once off the consecutive
        // regions of levels 5..4+fused.
        debug_assert_eq!(self.upper_region(ell + 2).start, 0);
        let mut rest = &mut spare[..self.upper_region(ell + 1 + fused).end];
        let above: Vec<RowsPtr> = (1..=fused)
            .map(|i| {
                let (level, tail) =
                    std::mem::take(&mut rest).split_at_mut(self.upper_region(ell + 1 + i).len());
                rest = tail;
                RowsPtr::new(level, cols)
            })
            .collect();
        cfg_into_iter!(0..sub, 1).for_each(|y0| {
            let mut l4 = [[MaybeUninit::<Gf>::uninit(); 64]; 1 << FUSED_UPPER_LEVELS];
            for g in 0..groups {
                let base_c = g << 6;
                let width = 64.min(cols - base_c);
                let range = base_c..base_c + width;
                for (b, row) in l4.iter_mut().enumerate().take(1 << fused) {
                    let y = y0 + b * sub;
                    let tab = [tables.at(y), tables.at(y | (1 << (t - ell - 1)))];
                    // This task owns rows `y0 + b·sub`.
                    let pats = self.jit_patterns(ell, y, g, nib);
                    kernels::jit_product_group(tab, &pats, &mut row[..width]);
                }
                for i in 1..=fused {
                    let half = 1usize << (fused - i);
                    for bp in 0..half {
                        let y = y0 + bp * sub;
                        // SAFETY: the level-4 segments were written just
                        // above (their first `width` slots); slots `range` of
                        // rows `y` and `y + half·sub` of level `3 + i > 4`
                        // were written by this task in this group's
                        // iteration; row `y` of level `4 + i` belongs to it
                        // alone.
                        unsafe {
                            let (a, b): (&[Gf], &[Gf]) = if i == 1 {
                                (
                                    init_prefix(&l4[bp], width),
                                    init_prefix(&l4[bp + half], width),
                                )
                            } else {
                                let below = &above[i - 2];
                                (
                                    below.row_init(y, range.clone()),
                                    below.row_init(y + half * sub, range.clone()),
                                )
                            };
                            let o = above[i - 1].row(y, range.clone());
                            kernels::product_into(a, b, o);
                        }
                    }
                }
            }
        });
        // The levels above `4 + fused`, each the products of the one below.
        for lower in ell + 1 + fused..t - 1 {
            let (below, rest) = spare.split_at_mut(self.upper_region(lower + 1).start);
            let below = &below[self.upper_region(lower)];
            // SAFETY: level `lower` was fully written (by the pass or the
            // previous iteration) and `MaybeUninit<Gf>` has `Gf`'s layout.
            let below =
                unsafe { std::slice::from_raw_parts(below.as_ptr().cast::<Gf>(), below.len()) };
            let len = self.upper_region(lower + 1).len();
            level_up_into(below, &mut rest[..len]);
        }
        // SAFETY: the tasks partition the rows of levels 5..4+fused and the
        // loop above writes every level after them in full.
        unsafe { arena.set_len(total) };
    }

    /// Weighted round sums through the JIT value tables.
    #[allow(clippy::too_many_arguments)]
    fn jit_round_sums(
        &self,
        tables: &Tables,
        ell: usize,
        k: usize,
        eq_c: &[Gf],
        eq_y: &[Gf],
        send_one: bool,
    ) -> (Gf, Gf) {
        debug_assert_eq!(ell + k, MATERIALISED_LEVEL);
        let low_bits = self.t - ell - 1 - k;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let rows = 1usize << (low_bits - 1);
        debug_assert_eq!(eq_y.len(), rows);
        let eq_t = transposed_eq(eq_c);
        let sel = nibble::jit_selectors(ell);
        let nib = self.nibbles().map(|rows| (rows, &sel));
        let row = |y1: usize, bk: &mut kernels::SumBuckets| {
            bk.clear();
            let mut pats = [[0u8; 64]; 4];
            let tab: [&[Gf]; 4] = std::array::from_fn(|corner| {
                let (p, b1) = (corner >> 1, corner & 1);
                tables.at((p << low_bits) | (b1 << (low_bits - 1)) | y1)
            });
            for g in 0..groups {
                // Corner `(p, b1)` is `2p + b1`; this task owns rows `y1`
                // and `y1 + 2^{low_bits−1}`.
                for b1 in 0..2 {
                    let [p0, p1] = self.jit_patterns(ell, y1 | (b1 << (low_bits - 1)), g, nib);
                    pats[b1] = p0;
                    pats[2 + b1] = p1;
                }
                kernels::jit_bucket_group(
                    [tab[2], tab[3]],
                    &pats,
                    &eq_t[g << 6..(g + 1) << 6],
                    send_one,
                    bk,
                );
            }
            let (end, inf) = kernels::jit_bucket_finish([tab[0], tab[1]], send_one, bk);
            let w = eq_y[y1];
            (end * w, inf * w)
        };
        #[cfg(feature = "parallel")]
        let partials: Vec<(Gf, Gf)> = (0..rows)
            .into_par_iter()
            .with_min_len(1)
            .map_init(kernels::SumBuckets::new, |bk, y1| row(y1, bk))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let partials: Vec<(Gf, Gf)> = {
            let mut bk = kernels::SumBuckets::new();
            (0..rows).map(|y1| row(y1, &mut bk)).collect()
        };
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
    }

    /// Folds into the reusable arena, optionally carrying column weights.
    #[allow(clippy::too_many_arguments)]
    fn jit_fold_round_with<const PRE_SCALED: bool, const WEIGH_LEFT: bool>(
        &self,
        tables: &Tables,
        ell: usize,
        k: usize,
        rho: Gf,
        eq_c: &[Gf],
        eq_y: &[Gf],
        send_one: bool,
        arena: &mut Vec<Gf>,
    ) -> (Gf, Gf) {
        let low_bits = self.t - ell - 1 - k;
        let cols = 1usize << self.s;
        let rows = 1usize << (low_bits - 2);
        let half = 2 * rows * cols;
        let len = 2 * half;
        debug_assert_eq!(eq_y.len(), rows);
        assert!(arena.capacity() >= len, "arena too small");
        if arena.len() != len {
            // First fill: through the spare capacity, no memset.
            arena.clear();
            let spare = &mut arena.spare_capacity_mut()[..len];
            let sums = self.jit_fold_into::<PRE_SCALED, WEIGH_LEFT>(
                tables, ell, k, rho, eq_c, eq_y, send_one, spare,
            );
            // SAFETY: `jit_fold_into` writes every slot of both halves.
            unsafe { arena.set_len(len) };
            sums
        } else {
            let slots = &mut arena[..len];
            // SAFETY: `MaybeUninit<Gf>` has `Gf`'s layout and only
            // initialised values are ever written through the view.
            let view = unsafe {
                std::slice::from_raw_parts_mut(slots.as_mut_ptr().cast::<MaybeUninit<Gf>>(), len)
            };
            self.jit_fold_into::<PRE_SCALED, WEIGH_LEFT>(
                tables, ell, k, rho, eq_c, eq_y, send_one, view,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn jit_fold_into<const PRE_SCALED: bool, const WEIGH_LEFT: bool>(
        &self,
        tables: &Tables,
        ell: usize,
        k: usize,
        rho: Gf,
        eq_c: &[Gf],
        eq_y: &[Gf],
        send_one: bool,
        out: &mut [MaybeUninit<Gf>],
    ) -> (Gf, Gf) {
        debug_assert_eq!(ell + k, MATERIALISED_LEVEL);
        let low_bits = self.t - ell - 1 - k;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let rows = 1usize << (low_bits - 2);
        let half = out.len() / 2;
        let (e_half, o_half) = out.split_at_mut(half);
        let (e_lo, e_hi) = e_half.split_at_mut(half / 2);
        let (o_lo, o_hi) = o_half.split_at_mut(half / 2);
        let eq_t = transposed_eq(eq_c);
        let sel = nibble::jit_selectors(ell);
        let nib = self.nibbles().map(|rows| (rows, &sel));
        let partials: Vec<(Gf, Gf)> = cfg_chunks_mut!(e_lo, cols)
            .zip(cfg_chunks_mut!(e_hi, cols))
            .zip(cfg_chunks_mut!(o_lo, cols))
            .zip(cfg_chunks_mut!(o_hi, cols))
            .enumerate()
            .map(|(y2, (((el, eh), ol), oh))| {
                debug_assert!(y2 < rows);
                let mut sums = kernels::Sums::zero();
                let mut pats = [[0u8; 64]; 8];
                let tab: [&[Gf]; 8] = std::array::from_fn(|corner| {
                    let (p, b1, b2) = (corner >> 2, (corner >> 1) & 1, corner & 1);
                    tables
                        .at((p << low_bits) | (b1 << (low_bits - 1)) | (b2 << (low_bits - 2)) | y2)
                });
                for g in 0..groups {
                    // Corner `(p, b1, b2)` is `4p + 2b1 + b2`.
                    for b in 0..4 {
                        let (b1, b2) = (b >> 1, b & 1);
                        let y = y2 | (b2 << (low_bits - 2)) | (b1 << (low_bits - 1));
                        let [p0, p1] = self.jit_patterns(ell, y, g, nib);
                        pats[b] = p0;
                        pats[4 + b] = p1;
                    }
                    let base_c = g << 6;
                    let width = 64.min(cols - base_c);
                    let range = base_c..base_c + width;
                    kernels::jit_fold_group::<PRE_SCALED, WEIGH_LEFT>(
                        tab,
                        &pats,
                        &rho,
                        &eq_t[g << 6..(g + 1) << 6],
                        send_one,
                        [&mut el[range.clone()], &mut eh[range.clone()]],
                        [&mut ol[range.clone()], &mut oh[range]],
                        &mut sums,
                    );
                }
                let (end, inf) = sums.finish();
                let w = eq_y[y2];
                (end * w, inf * w)
            })
            .collect();
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
    }

    /// Round `j` (1-based, `j ≤ k`) of level `ell`'s sumcheck off the bits:
    /// `Σ eq·E_end·O_end` and `Σ eq·(E_hi − E_lo)(O_hi − O_lo)` over the
    /// `(y_j, c)` terms, with `E_·`, `O_·` read from the `(j−1)`-fold tables.
    ///
    /// Within a row `y` each of `E_lo, E_hi, O_lo, O_hi` is a function of the
    /// column's `nb`-bit pattern, so a row's sum is `Σ_{a,b} T_E[a]·T_O[b]·
    /// Σ_{c: pat_E(c)=a, pat_O(c)=b} eq_c[c]`: the terms only bucket `eq_c`
    /// by their `(E, O)` pattern pair — one 16-byte addition per term for
    /// widths 1 and 2 (the whole `(E_lo, E_hi, O_lo, O_hi)` tuple, one byte
    /// straight out of the bit-block transpose, marginalised afterwards),
    /// four for width 4 (the pair bytes come out of two transposes, the
    /// cross pairs by swapping nibbles) — and the `2^{2nb} + 2^{nb}`
    /// products of the contraction are paid once per row.
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
        let y_bits = t - ell - 1 - j;
        let eq_t = transposed_eq(&eq_table(&external[..s]));
        let eq_y = eq_table(&external[s..s + y_bits]);
        let rows = 1usize << y_bits;
        // Width 1: two rows share one byte index, so one scatter serves
        // two terms ([`Forest::bit_row_pair`]).
        let paired = nb == 1 && rows >= 2;
        let tasks = if paired { rows / 2 } else { rows };
        let rows_nib = self.nibbles();
        let selectors = rows_nib.map(|_| BitSelectors::new(ell, j, paired));
        let nib = rows_nib.zip(selectors.as_ref());
        let task = |i: usize, buckets: &mut Buckets| {
            if paired {
                self.bit_row_pair(
                    &tables,
                    ell,
                    kk,
                    2 * i,
                    y_bits,
                    &eq_t,
                    &eq_y,
                    send_one,
                    buckets,
                    nib,
                )
            } else {
                let (end, inf) = self.bit_row(
                    &tables, ell, kk, nb, i, y_bits, &eq_t, send_one, buckets, nib,
                );
                let w = eq_y[i];
                (end * w, inf * w)
            }
        };
        #[cfg(feature = "parallel")]
        let partials: Vec<(Gf, Gf)> = (0..tasks)
            .into_par_iter()
            .with_min_len(1)
            .map_init(Buckets::new, |buckets, i| task(i, buckets))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let partials: Vec<(Gf, Gf)> = {
            let mut buckets = Buckets::new();
            (0..tasks).map(|i| task(i, &mut buckets)).collect()
        };
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
    }

    /// Rows `y0` and `y0 + 1` of a width-1 [`Forest::bit_round`] together,
    /// weighted: the two rows' 4-bit tuples `(E_lo, E_hi, O_lo, O_hi)` fill
    /// one byte index out of one block transpose (row `y0` in the low
    /// nibble), so a column's weight is scattered once for both rows; the
    /// per-row coefficient of a tuple is `w·E_end·O_end` (`w·(E_hi + E_lo)
    /// (O_hi + O_lo)` for the ∞ sum), and since the pair's coefficient is
    /// the sum of the two rows' the contraction only needs the two 16-entry
    /// marginals of the 256 buckets.
    #[allow(clippy::too_many_arguments)]
    fn bit_row_pair(
        &self,
        tables: &Tables,
        ell: usize,
        kk: usize,
        y0: usize,
        y_bits: usize,
        eq_t: &[Gf],
        eq_y: &[Gf],
        send_one: bool,
        bk: &mut Buckets,
        nib: Option<(&NibbleRows, &BitSelectors)>,
    ) -> (Gf, Gf) {
        debug_assert_eq!(ell + kk, 0);
        let low_bits = y_bits + 1;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        let zero = Gf::zero();
        bk.tuples.fill(zero);
        let corner_y = |y: usize, corner: usize| y | ((corner & 1) << y_bits);
        let corner_p = |corner: usize| corner >> 1;
        let mut tmp = [0u64; 8];
        let mut words = [0u64; 8];
        // Nibble rows `y0` and `y0 + 1` (`y0` even) share `q = y0 >> H`.
        let h = self.t - 4;
        let (q, yr) = (y0 >> h, y0 & ((1usize << h) - 1));
        for g in 0..groups {
            let idx = match nib {
                Some((rows, BitSelectors::Paired(sel))) => nibble::select_pair(
                    rows.block(yr, g),
                    rows.block(yr + 1, g),
                    &sel[q][0],
                    &sel[q][1],
                ),
                _ => {
                    for i in 0..2 {
                        for corner in 0..4 {
                            self.corner_words(
                                ell,
                                kk,
                                corner_p(corner),
                                corner_y(y0 + i, corner),
                                g,
                                &mut tmp,
                            );
                            words[4 * i + corner] = tmp[0];
                        }
                    }
                    transposed_patterns(&mut words)
                }
            };
            kernels::scatter_add(&mut bk.tuples, &idx, &eq_t[g << 6..(g + 1) << 6]);
        }
        // The marginals: row `y0`'s tuple is the low nibble.
        let mut marginal = [[zero; 16]; 2];
        for (k, &v) in bk.tuples.iter().enumerate() {
            marginal[0][k & 15] += v;
            marginal[1][k >> 4] += v;
        }
        let (mut end, mut inf) = (zero, zero);
        for (i, m) in marginal.iter().enumerate() {
            let y = y0 + i;
            let q = |corner: usize| (corner_p(corner) << low_bits) | corner_y(y, corner);
            let (t_e_lo, t_e_hi) = (tables.at(q(0)), tables.at(q(1)));
            let (t_o_lo, t_o_hi) = (tables.at(q(2)), tables.at(q(3)));
            let w = eq_y[y];
            let (t_e_end, t_o_end) = if send_one {
                (t_e_hi, t_o_hi)
            } else {
                (t_e_lo, t_o_lo)
            };
            let we: [Gf; 2] = [w * t_e_end[0], w * t_e_end[1]];
            let mut c_end = [zero; 16];
            let mut c_inf = [zero; 16];
            for (t, (c_end_t, c_inf_t)) in c_end.iter_mut().zip(c_inf.iter_mut()).enumerate() {
                let (e_lo, e_hi, o_lo, o_hi) = (t & 1, (t >> 1) & 1, (t >> 2) & 1, (t >> 3) & 1);
                let (e_end, o_end) = if send_one { (e_hi, o_hi) } else { (e_lo, o_lo) };
                *c_end_t = we[e_end] * t_o_end[o_end];
                *c_inf_t = w * (t_e_hi[e_hi] + t_e_lo[e_lo]) * (t_o_hi[o_hi] + t_o_lo[o_lo]);
            }
            end = end + kernels::dot(&c_end, m);
            inf = inf + kernels::dot(&c_inf, m);
        }
        (end, inf)
    }

    /// One row of [`Forest::bit_round`]: bucket the row's columns, then
    /// contract with the four corner tables.
    #[allow(clippy::too_many_arguments)]
    fn bit_row(
        &self,
        tables: &Tables,
        ell: usize,
        kk: usize,
        nb: usize,
        y: usize,
        y_bits: usize,
        eq_t: &[Gf],
        send_one: bool,
        bk: &mut Buckets,
        nib: Option<(&NibbleRows, &BitSelectors)>,
    ) -> (Gf, Gf) {
        let low_bits = y_bits + 1;
        let entries = 1usize << nb;
        let pairs = entries * entries;
        let cols = 1usize << self.s;
        let groups = cols.div_ceil(64);
        bk.reset(nb);
        // The four (p, bit) corners: E_lo, E_hi, O_lo, O_hi.
        let corner_y = |corner: usize| y | ((corner & 1) << y_bits);
        let corner_p = |corner: usize| corner >> 1;
        let mut tmp = [0u64; 8];
        let mut words = [0u64; 8];
        let mut lh = [0u8; 64];
        let mut hl = [0u8; 64];
        let h = self.t - 4;
        let (q, yr) = (y >> h, y & ((1usize << h) - 1));
        for g in 0..groups {
            let eq_g = &eq_t[g << 6..(g + 1) << 6];
            if let Some((rows, BitSelectors::Tuple(sel))) = nib {
                let mut idx = [[0u8; 64]; 1];
                nibble::select(rows.block(yr, g), core::array::from_ref(&sel[q]), &mut idx);
                kernels::scatter_add(&mut bk.tuples, &idx[0], eq_g);
            } else if let Some((rows, BitSelectors::Pairs(sel))) = nib {
                let mut idx = [[0u8; 64]; 4];
                nibble::select(rows.block(yr, g), sel, &mut idx);
                kernels::scatter_add4([&mut bk.ll, &mut bk.hh, &mut bk.lh, &mut bk.hl], &idx, eq_g);
            } else if nb <= 2 {
                // Tuple byte: corner `i`'s pattern at bits `i·nb..`.
                words = [0u64; 8];
                for corner in 0..4 {
                    self.corner_words(ell, kk, corner_p(corner), corner_y(corner), g, &mut tmp);
                    words[corner * nb..(corner + 1) * nb].copy_from_slice(&tmp[..nb]);
                }
                let idx = transposed_patterns(&mut words);
                kernels::scatter_add(&mut bk.tuples, &idx, eq_g);
            } else {
                // Pair bytes `pat_E·16 + pat_O`: the O words in the low
                // nibble, the E words in the high one.
                let mut idx = [[0u8; 64]; 2];
                for (out, e_corner, o_corner) in [(0usize, 0usize, 2usize), (1, 1, 3)] {
                    self.corner_words(ell, kk, corner_p(o_corner), corner_y(o_corner), g, &mut tmp);
                    words[..4].copy_from_slice(&tmp[..4]);
                    self.corner_words(ell, kk, corner_p(e_corner), corner_y(e_corner), g, &mut tmp);
                    words[4..8].copy_from_slice(&tmp[..4]);
                    idx[out] = transposed_patterns(&mut words);
                }
                for m in 0..64 {
                    lh[m] = (idx[0][m] & 0xF0) | (idx[1][m] & 0x0F);
                    hl[m] = (idx[1][m] & 0xF0) | (idx[0][m] & 0x0F);
                }
                kernels::scatter_add(&mut bk.ll, &idx[0], eq_g);
                kernels::scatter_add(&mut bk.hh, &idx[1], eq_g);
                kernels::scatter_add(&mut bk.lh, &lh, eq_g);
                kernels::scatter_add(&mut bk.hl, &hl, eq_g);
            }
        }
        if nb <= 2 {
            // Marginalise the tuple buckets into the four pair buckets.
            let mask = entries - 1;
            for (tuple, &value) in bk.tuples[..1 << (4 * nb)].iter().enumerate() {
                let pe_lo = tuple & mask;
                let pe_hi = (tuple >> nb) & mask;
                let po_lo = (tuple >> (2 * nb)) & mask;
                let po_hi = (tuple >> (3 * nb)) & mask;
                bk.ll[pe_lo * entries + po_lo] += value;
                bk.lh[pe_lo * entries + po_hi] += value;
                bk.hl[pe_hi * entries + po_lo] += value;
                bk.hh[pe_hi * entries + po_hi] += value;
            }
        }
        let q = |corner: usize| (corner_p(corner) << low_bits) | corner_y(corner);
        let (t_e_lo, t_e_hi) = (tables.at(q(0)), tables.at(q(1)));
        let (t_o_lo, t_o_hi) = (tables.at(q(2)), tables.at(q(3)));
        let ll = kernels::contract(t_e_lo, t_o_lo, &bk.ll[..pairs]);
        let hh = kernels::contract(t_e_hi, t_o_hi, &bk.hh[..pairs]);
        let end = if send_one { hh } else { ll };
        // (E_hi − E_lo)(O_hi − O_lo) expands to the four combinations;
        // characteristic two makes every sign a plus.
        let inf = hh
            + kernels::contract(t_e_hi, t_o_lo, &bk.hl[..pairs])
            + kernels::contract(t_e_lo, t_o_hi, &bk.lh[..pairs])
            + ll;
        (end, inf)
    }
}

/// The per-task bucket store of the bit rounds: 256 entries each so every
/// byte index is in bounds; only the used prefixes are cleared per row.
struct Buckets {
    tuples: Vec<Gf>,
    ll: Vec<Gf>,
    lh: Vec<Gf>,
    hl: Vec<Gf>,
    hh: Vec<Gf>,
}

impl Buckets {
    fn new() -> Self {
        let fresh = || vec![Gf::zero(); 256];
        Self {
            tuples: fresh(),
            ll: fresh(),
            lh: fresh(),
            hl: fresh(),
            hh: fresh(),
        }
    }

    fn reset(&mut self, nb: usize) {
        let pairs = 1usize << (2 * nb);
        let zero = Gf::zero();
        if nb <= 2 {
            self.tuples[..1 << (4 * nb)].fill(zero);
        }
        self.ll[..pairs].fill(zero);
        self.lh[..pairs].fill(zero);
        self.hl[..pairs].fill(zero);
        self.hh[..pairs].fill(zero);
    }
}

/// The last bit round's pair buckets with the half of each side's corners
/// they hold: `(bucket, x, x')` for E corners with `b_k = x` and O corners
/// with `b_k = x'`.
fn pair_buckets(bk: &Buckets) -> [(&[Gf], usize, usize); 4] {
    [
        (bk.ll.as_slice(), 0, 0),
        (bk.hh.as_slice(), 1, 1),
        (bk.lh.as_slice(), 0, 1),
        (bk.hl.as_slice(), 1, 0),
    ]
}

/// The sum of 16 values and, for each index bit `b`, the sum of those whose
/// index has bit `b` set: one tree, 26 additions.
#[inline(always)]
fn bit_sums(f: &[Gf]) -> (Gf, [Gf; 4]) {
    let p1: [Gf; 8] = std::array::from_fn(|i| f[2 * i] + f[2 * i + 1]);
    let p2: [Gf; 4] = std::array::from_fn(|i| p1[2 * i] + p1[2 * i + 1]);
    let (p3_lo, p3_hi) = (p2[0] + p2[1], p2[2] + p2[3]);
    let b0 = f[1] + f[3] + f[5] + f[7] + f[9] + f[11] + f[13] + f[15];
    let b1 = p1[1] + p1[3] + p1[5] + p1[7];
    (p3_lo + p3_hi, [b0, b1, p2[1] + p2[3], p3_hi])
}

/// One row of level 0's cross sums: every corner is one leaf,
/// `E_α = 1 + e_α·δ_α` with `δ_α = A_α − 1` (`delta_e`, `delta_o`), so
/// `Σ_c eq_c E_α O_β = W + δ_α·S^E_α + δ_β·S^O_β + δ_α·δ_β·P_αβ` with `W`
/// the row's total weight, `S` the weight of the columns whose corner bit is
/// set and `P` of those with both set — single-bit marginals of the
/// buckets. Adds `eps` times them to `acc`.
fn cross_row_leaves(
    bk: &Buckets,
    delta_e: &[Gf; 8],
    delta_o: &[Gf; 8],
    eps: Gf,
    acc: &mut [Gf; 64],
) {
    let zero = Gf::zero();
    let mut w = zero;
    let mut s_e = [zero; 8];
    let mut s_o = [zero; 8];
    let mut both = [[zero; 8]; 8];
    for (bucket, x, xo) in pair_buckets(bk) {
        // Per E pattern: its total and its sums by O bit.
        let mut total = [zero; 16];
        let mut by_o = [[zero; 16]; 4];
        for e in 0..16 {
            let (sum, bits) = bit_sums(&bucket[16 * e..16 * e + 16]);
            total[e] = sum;
            for (v2, &b) in bits.iter().enumerate() {
                by_o[v2][e] = b;
            }
        }
        for (v2, column) in by_o.iter().enumerate() {
            let (sum, bits) = bit_sums(column);
            let beta = (v2 << 1) | xo;
            if x == xo {
                s_o[beta] = sum;
            }
            for (v, &b) in bits.iter().enumerate() {
                both[(v << 1) | x][beta] = b;
            }
        }
        if x == xo {
            let (sum, bits) = bit_sums(&total);
            w = sum;
            for (v, &b) in bits.iter().enumerate() {
                s_e[(v << 1) | x] = b;
            }
        }
    }
    let ew = eps * w;
    let a: [Gf; 8] = std::array::from_fn(|alpha| eps * delta_e[alpha]);
    let ea: [Gf; 8] = std::array::from_fn(|alpha| a[alpha] * s_e[alpha]);
    let eb: [Gf; 8] = std::array::from_fn(|beta| eps * delta_o[beta] * s_o[beta]);
    for alpha in 0..8 {
        for beta in 0..8 {
            acc[8 * alpha + beta] +=
                ew + ea[alpha] + eb[beta] + a[alpha] * delta_o[beta] * both[alpha][beta];
        }
    }
}

/// One row of level 1's cross sums: corner `(v, x)` has the two leaves at
/// nibble bits `2v`, `2v + 1` and the 4-entry table `tab_e[α]` (`tab_o`),
/// so each bucket is marginalised onto every (E corner, O corner) pair of
/// patterns and contracted with the two tables. Adds `eps` times the sums
/// to `acc`.
fn cross_row_pairs(
    bk: &Buckets,
    tab_e: &[[Gf; 4]; 4],
    tab_o: &[[Gf; 4]; 4],
    eps: Gf,
    acc: &mut [Gf; 64],
) {
    let zero = Gf::zero();
    for (bucket, x, xo) in pair_buckets(bk) {
        // by_o[v2][e][b]: the E pattern `e` with O corner `v2`'s pattern `b`.
        let mut by_o = [[[zero; 4]; 16]; 2];
        for e in 0..16 {
            let r = &bucket[16 * e..16 * e + 16];
            for b in 0..4 {
                by_o[0][e][b] = r[b] + r[b + 4] + r[b + 8] + r[b + 12];
                by_o[1][e][b] = r[4 * b] + r[4 * b + 1] + r[4 * b + 2] + r[4 * b + 3];
            }
        }
        for v in 0..2 {
            for (v2, by) in by_o.iter().enumerate() {
                let (alpha, beta) = ((v << 1) | x, (v2 << 1) | xo);
                let mut c = zero;
                for a in 0..4 {
                    let mut inner = zero;
                    for b in 0..4 {
                        let m = if v == 0 {
                            by[a][b] + by[a + 4][b] + by[a + 8][b] + by[a + 12][b]
                        } else {
                            by[4 * a][b] + by[4 * a + 1][b] + by[4 * a + 2][b] + by[4 * a + 3][b]
                        };
                        inner += tab_o[beta][b] * m;
                    }
                    c += tab_e[alpha][a] * inner;
                }
                acc[4 * alpha + beta] += eps * c;
            }
        }
    }
}

/// Round `j`'s `(Σ eq·E_end·O_end, Σ eq·(E_hi − E_lo)(O_hi − O_lo))` from
/// the cross sums of a level's `k` bit rounds ([`Forest::cross_sums`]):
/// both corners' bits `b_1..b_{j−1}` weighted by `eq(r, ·)`, their shared
/// bits `b_{j+1}..b_k` by `eq(z, ·)` (`z[i − 1]` is round `i`'s coordinate),
/// and bit `b_j` at the endpoint on both sides or summed over all four
/// combinations.
fn cross_round_sums(
    cross: &[Gf; 64],
    k: usize,
    j: usize,
    z: &[Gf],
    r: &[Gf],
    send_one: bool,
) -> (Gf, Gf) {
    let one = Gf::one();
    let corners = 1usize << k;
    let lo_bits = k - j;
    let eq_bit = |b: usize, x: Gf| if b == 1 { x } else { one - x };
    // Bit `j − 1 − i` of `hi` is `b_i`; bit `k − i` of `lo` is `b_i`.
    let w: Vec<Gf> = (0..1usize << (j - 1))
        .map(|hi| {
            (1..j).fold(one, |acc, i| {
                acc * eq_bit((hi >> (j - 1 - i)) & 1, r[i - 1])
            })
        })
        .collect();
    let ez: Vec<Gf> = (0..1usize << lo_bits)
        .map(|lo| ((j + 1)..=k).fold(one, |acc, i| acc * eq_bit((lo >> (k - i)) & 1, z[i - 1])))
        .collect();
    let end_bit = usize::from(send_one);
    let (mut end, mut inf) = (Gf::zero(), Gf::zero());
    for (hi, &w_e) in w.iter().enumerate() {
        for (hi2, &w_o) in w.iter().enumerate() {
            let ww = w_e * w_o;
            for (lo, &e_z) in ez.iter().enumerate() {
                let corner = |h: usize, b: usize| (h << (lo_bits + 1)) | (b << lo_bits) | lo;
                let at = |b: usize, b2: usize| cross[corner(hi, b) * corners + corner(hi2, b2)];
                let weight = ww * e_z;
                end += weight * at(end_bit, end_bit);
                inf += weight * (at(0, 0) + at(0, 1) + at(1, 0) + at(1, 1));
            }
        }
    }
    (end, inf)
}

/// One level up: `out[i] = lower[i] · lower[i + half]`.
fn level_up(lower: &[Gf]) -> Vec<Gf> {
    let half = lower.len() / 2;
    let mut out: Vec<Gf> = Vec::with_capacity(half);
    level_up_into(lower, &mut out.spare_capacity_mut()[..half]);
    // SAFETY: `level_up_into` writes every one of the `half` slots.
    unsafe { out.set_len(half) };
    out
}

/// [`level_up`] into pre-sized slots.
fn level_up_into(lower: &[Gf], out: &mut [MaybeUninit<Gf>]) {
    let half = lower.len() / 2;
    assert_eq!(out.len(), half);
    let (l, r) = lower.split_at(half);
    cfg_chunks_mut!(out, PARALLEL_MIN_LANES)
        .enumerate()
        .for_each(|(i, chunk)| {
            let start = i * PARALLEL_MIN_LANES;
            kernels::product_into(
                &l[start..start + chunk.len()],
                &r[start..start + chunk.len()],
                chunk,
            );
        });
}

/// The first `len` slots of `buf` as values.
///
/// # Safety
/// Those slots must have been written.
#[inline]
unsafe fn init_prefix(buf: &[MaybeUninit<Gf>; 64], len: usize) -> &[Gf] {
    debug_assert!(len <= 64);
    // SAFETY: in bounds; `MaybeUninit<Gf>` has `Gf`'s layout and the prefix
    // is initialised per the contract.
    unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<Gf>(), len) }
}

/// Row access into an uninitialised level for tasks that own disjoint
/// rows: `row(y, range)` is row `y`'s slots in the column range `range`,
/// `row_init(y, range)` the same slots once written, so no value view
/// covers a slot still unwritten.
#[derive(Clone, Copy)]
struct RowsPtr {
    ptr: *mut MaybeUninit<Gf>,
    rows: usize,
    cols: usize,
}

// SAFETY: the pointer is only dereferenced through the `unsafe` accessors,
// whose callers keep the rows they touch disjoint across tasks.
unsafe impl Send for RowsPtr {}
unsafe impl Sync for RowsPtr {}

impl RowsPtr {
    fn new(slots: &mut [MaybeUninit<Gf>], cols: usize) -> Self {
        debug_assert_eq!(slots.len() % cols, 0);
        Self {
            ptr: slots.as_mut_ptr(),
            rows: slots.len() / cols,
            cols,
        }
    }

    /// Row `y`'s slots `range`.
    ///
    /// # Safety
    /// No other live reference to those slots may exist while this one does.
    #[inline]
    unsafe fn row<'b>(&self, y: usize, range: std::ops::Range<usize>) -> &'b mut [MaybeUninit<Gf>] {
        assert!(y < self.rows && range.start <= range.end && range.end <= self.cols);
        // SAFETY: in bounds by the assertion; exclusivity is the caller's.
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr.add(y * self.cols + range.start), range.len())
        }
    }

    /// Row `y`'s slots `range` as initialised values.
    ///
    /// # Safety
    /// Those slots must have been written, and no `&mut` to them may be
    /// live.
    #[inline]
    unsafe fn row_init<'b>(&self, y: usize, range: std::ops::Range<usize>) -> &'b [Gf] {
        assert!(y < self.rows && range.start <= range.end && range.end <= self.cols);
        // SAFETY: in bounds by the assertion; `MaybeUninit<Gf>` has `Gf`'s
        // layout and the slots are initialised per the contract.
        unsafe {
            std::slice::from_raw_parts(
                self.ptr.add(y * self.cols + range.start).cast::<Gf>(),
                range.len(),
            )
        }
    }
}

/// Transposes the eight 8×8 bit blocks of `w` in place, one per byte lane:
/// afterwards bit `i` of byte `k` of `w[j]` is bit `8k + j` of the original
/// `w[i]`, so byte `k` of `w[j]` is the pattern of column `8k + j`. Twelve
/// delta swaps (4-, 2-, then 1-bit sub-blocks).
#[inline]
pub(crate) fn transpose_blocks(w: &mut [u64; 8]) {
    for i in 0..4 {
        let t = ((w[i] >> 4) ^ w[i + 4]) & 0x0F0F_0F0F_0F0F_0F0F;
        w[i + 4] ^= t;
        w[i] ^= t << 4;
    }
    for i in [0, 1, 4, 5] {
        let t = ((w[i] >> 2) ^ w[i + 2]) & 0x3333_3333_3333_3333;
        w[i + 2] ^= t;
        w[i] ^= t << 2;
    }
    for i in [0, 2, 4, 6] {
        let t = ((w[i] >> 1) ^ w[i + 1]) & 0x5555_5555_5555_5555;
        w[i + 1] ^= t;
        w[i] ^= t << 1;
    }
}

/// The patterns of a group's 64 columns in transposed order: position
/// `8j + k` holds column `8k + j` ([`kernels::col_of`]). Consumes `words`.
#[inline]
fn transposed_patterns(words: &mut [u64; 8]) -> [u8; 64] {
    transpose_blocks(words);
    let mut out = [0u8; 64];
    for (j, w) in words.iter().enumerate() {
        out[8 * j..8 * j + 8].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// The column weights regrouped the way the transposed pattern blocks
/// visit them: entry `64g + m` is `eq_c[64g + col_of(m)]`, zero past the
/// last column.
fn transposed_eq(eq_c: &[Gf]) -> Vec<Gf> {
    let groups = eq_c.len().div_ceil(64);
    let mut out = vec![Gf::zero(); groups << 6];
    for g in 0..groups {
        for m in 0..64 {
            let c = (g << 6) | kernels::col_of(m);
            if c < eq_c.len() {
                out[(g << 6) | m] = eq_c[c];
            }
        }
    }
    out
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
#[path = "forest_tests.rs"]
mod parity_tests;

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
    fn block_transpose_matches_the_bit_loop() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..64 {
            let mut words = [0u64; 8];
            for w in words.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *w = state;
            }
            let want = naive_patterns(&words);
            let got = transposed_patterns(&mut words.clone());
            for m in 0..64 {
                assert_eq!(got[m], want[kernels::col_of(m)], "position {m}");
            }
        }
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

    /// A random grid: `2^t` rows, `2^s` columns packed 64 to a word (bits
    /// past the last column zero), and random row images.
    fn random_grid(t: usize, s: usize, seed: u64) -> (Vec<Vec<u64>>, Vec<Gf>) {
        let mut state = seed | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let cols = 1usize << s;
        let groups = cols.div_ceil(64);
        let packed = (0..groups)
            .map(|g| {
                let live = (cols - 64 * g).min(64);
                let mask = if live == 64 {
                    u64::MAX
                } else {
                    (1u64 << live) - 1
                };
                (0..1usize << t).map(|_| next() & mask).collect()
            })
            .collect();
        let images = (0..1usize << t).map(|_| Gf::new(next(), next())).collect();
        (packed, images)
    }

    /// Every index the nibble rows produce — each level's JIT patterns and
    /// every bit round's tuple, pair and paired bytes — equals the one the
    /// gather-and-transpose path produces.
    #[test]
    fn nibble_selections_match_the_gathered_patterns() {
        // `t = 5, 6` build their rows 2 and 4 at a time, the rest 8 (`t = 4`,
        // one row, has no second row for the paired round's read; the
        // column-fold test covers its build).
        for (t, s) in [(5usize, 6usize), (6, 3), (6, 6), (7, 7), (9, 8)] {
            let (packed, images) = random_grid(t, s, (t * 100 + s) as u64);
            let forest = Forest::new(t, s, &packed, &images);
            let groups = (1usize << s).div_ceil(64);
            let rows = NibbleRows::new(t, &packed, groups);
            let h = t - 4;
            let mut words = [0u64; 8];
            for ell in 0..=MATERIALISED_LEVEL {
                let sel = nibble::jit_selectors(ell);
                for y in 0..1usize << h {
                    for g in 0..groups {
                        let mut got = [[0u8; 64]; 2];
                        nibble::select(rows.block(y, g), &sel, &mut got);
                        for (p, block) in got.iter().enumerate() {
                            forest.corner_words(ell, MATERIALISED_LEVEL - ell, p, y, g, &mut words);
                            assert_eq!(
                                *block,
                                transposed_patterns(&mut words),
                                "jit t={t} s={s} ell={ell} p={p} y={y} g={g}"
                            );
                        }
                    }
                }
            }
            for (ell, j) in [(0usize, 1usize), (0, 2), (0, 3), (1, 1), (1, 2), (2, 1)] {
                let kk = j - 1;
                let nb = 1usize << (ell + kk);
                let y_bits = t - ell - 1 - j;
                let paired = nb == 1;
                let selectors = BitSelectors::new(ell, j, paired);
                let corner_y = |y: usize, corner: usize| y | ((corner & 1) << y_bits);
                for y in (0..1usize << y_bits).step_by(if paired { 2 } else { 1 }) {
                    let (q, yr) = (y >> h, y & ((1usize << h) - 1));
                    for g in 0..groups {
                        let mut tmp = [0u64; 8];
                        let mut words = [0u64; 8];
                        match &selectors {
                            BitSelectors::Paired(sel) => {
                                for i in 0..2 {
                                    for corner in 0..4 {
                                        forest.corner_words(
                                            ell,
                                            kk,
                                            corner >> 1,
                                            corner_y(y + i, corner),
                                            g,
                                            &mut tmp,
                                        );
                                        words[4 * i + corner] = tmp[0];
                                    }
                                }
                                let got = nibble::select_pair(
                                    rows.block(yr, g),
                                    rows.block(yr + 1, g),
                                    &sel[q][0],
                                    &sel[q][1],
                                );
                                assert_eq!(
                                    got,
                                    transposed_patterns(&mut words),
                                    "paired t={t} s={s} y={y} g={g}"
                                );
                            }
                            BitSelectors::Tuple(sel) => {
                                for corner in 0..4 {
                                    forest.corner_words(
                                        ell,
                                        kk,
                                        corner >> 1,
                                        corner_y(y, corner),
                                        g,
                                        &mut tmp,
                                    );
                                    words[corner * nb..(corner + 1) * nb]
                                        .copy_from_slice(&tmp[..nb]);
                                }
                                let mut got = [[0u8; 64]; 1];
                                nibble::select(
                                    rows.block(yr, g),
                                    core::array::from_ref(&sel[q]),
                                    &mut got,
                                );
                                assert_eq!(
                                    got[0],
                                    transposed_patterns(&mut words),
                                    "tuple t={t} s={s} ell={ell} j={j} y={y} g={g}"
                                );
                            }
                            BitSelectors::Pairs(sel) => {
                                let mut want = [[0u8; 64]; 2];
                                for (out, e_corner, o_corner) in
                                    [(0usize, 0usize, 2usize), (1, 1, 3)]
                                {
                                    forest.corner_words(
                                        ell,
                                        kk,
                                        o_corner >> 1,
                                        corner_y(y, o_corner),
                                        g,
                                        &mut tmp,
                                    );
                                    words[..4].copy_from_slice(&tmp[..4]);
                                    forest.corner_words(
                                        ell,
                                        kk,
                                        e_corner >> 1,
                                        corner_y(y, e_corner),
                                        g,
                                        &mut tmp,
                                    );
                                    words[4..8].copy_from_slice(&tmp[..4]);
                                    want[out] = transposed_patterns(&mut words);
                                }
                                let lh: [u8; 64] = std::array::from_fn(|m| {
                                    (want[0][m] & 0xF0) | (want[1][m] & 0x0F)
                                });
                                let hl: [u8; 64] = std::array::from_fn(|m| {
                                    (want[1][m] & 0xF0) | (want[0][m] & 0x0F)
                                });
                                let mut got = [[0u8; 64]; 4];
                                nibble::select(rows.block(yr, g), sel, &mut got);
                                assert_eq!(
                                    got,
                                    [want[0], want[1], lh, hl],
                                    "pairs t={t} s={s} ell={ell} j={j} y={y} g={g}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Every position table is the definition's sum over `v` of
    /// `eq(v, r)·Π_{u ∈ sub_v} A(base_v + u·2^{t−ℓ})`, also when built into a
    /// buffer that held another level's tables.
    #[test]
    fn fold_tables_match_the_definition() {
        let t = 7usize;
        let (packed, images) = random_grid(t, 6, 77);
        let forest = Forest::new(t, 6, &packed, &images);
        let mut state = 0xF01D_7AB1u64;
        let r: Vec<Gf> = (0..3).map(|_| random_gf(&mut state)).collect();
        let stale: Vec<Gf> = (0..1usize << 13).map(|_| random_gf(&mut state)).collect();
        for (ell, kk) in [
            (0usize, 0usize),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 0),
            (1, 1),
            (1, 2),
            (2, 0),
            (2, 1),
            (3, 0),
        ] {
            let reversed: Vec<Gf> = r[..kk].iter().rev().copied().collect();
            let eq_v = if kk == 0 {
                vec![Gf::one()]
            } else {
                eq_table(&reversed)
            };
            let low_bits = t - ell - 1 - kk;
            let nb = 1usize << ell;
            for tables in [
                forest.fold_table(ell, kk, &r),
                forest.fold_table_in(ell, kk, &r, stale.clone()),
            ] {
                assert_eq!(tables.len, 1usize << (nb << kk));
                for q in 0..1usize << (t - ell - kk) {
                    let (p, y) = (q >> low_bits, q & ((1usize << low_bits) - 1));
                    for (x, &got) in tables.at(q).iter().enumerate() {
                        let mut want = Gf::zero();
                        for v in 0..1usize << kk {
                            let base = y | (v << low_bits) | (p << (t - ell - 1));
                            let mut term = eq_v[v];
                            for u in 0..nb {
                                if (x >> ((v << ell) | u)) & 1 == 1 {
                                    term = term * images[base | (u << (t - ell))];
                                }
                            }
                            want += term;
                        }
                        assert_eq!(got, want, "ell={ell} kk={kk} q={q} x={x}");
                    }
                }
            }
        }
    }

    /// The column folds off the nibble rows are the integers the bit walk
    /// gives, at every grid shape the rows are built for.
    #[test]
    fn column_folds_match_the_bits() {
        for (t, s) in [
            (4usize, 0usize),
            (4, 5),
            (5, 6),
            (6, 3),
            (7, 7),
            (9, 8),
            (10, 6),
        ] {
            let (packed, _) = random_grid(t, s, (t * 31 + s) as u64);
            let mut state = 0xC0DE_F01D ^ (t * 7 + s) as u64;
            let mut word = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            // Exponents near 2^127, so the sums wrap.
            let exponents: Vec<u128> = (0..1usize << t)
                .map(|_| (u128::from(word()) << 64 | u128::from(word())) >> 1)
                .collect();
            let cols = 1usize << s;
            let rows = NibbleRows::new(t, &packed, cols.div_ceil(64));
            let want: Vec<u128> = (0..cols)
                .map(|c| {
                    (0..1usize << t)
                        .filter(|&b| (packed[c / 64][b] >> (c % 64)) & 1 == 1)
                        .fold(0u128, |acc, b| acc.wrapping_add(exponents[b]))
                })
                .collect();
            assert_eq!(rows.column_folds(t, &exponents, cols), want, "t={t} s={s}");
        }
    }

    /// A random field element from an xorshift state.
    fn random_gf(state: &mut u64) -> Gf {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        Gf::new(*state, state.rotate_left(29))
    }

    /// Every bit round's two sums of levels 0 and 1 — either endpoint, any
    /// challenges — equal the ones combined from the one pass's cross sums.
    #[test]
    fn cross_sums_give_every_bit_round() {
        for (t, s) in [(4usize, 0usize), (4, 6), (5, 3), (6, 7), (7, 6), (9, 8)] {
            let (packed, images) = random_grid(t, s, (t * 7919 + s) as u64);
            let mut state = 0xC0FF_EE00 ^ (t * 131 + s) as u64;
            for nibble in [false, true] {
                let mode = PatternMode {
                    nibble,
                    nibble_from: 0,
                };
                let forest = Forest::new(t, s, &packed, &images).with_patterns(mode);
                for ell in 0..2 {
                    let k = MATERIALISED_LEVEL - ell;
                    let point: Vec<Gf> = (0..t - ell - 1 + s)
                        .map(|_| random_gf(&mut state))
                        .collect();
                    let external: Vec<Gf> = point.iter().rev().copied().collect();
                    let cross = forest.cross_sums(ell, k, &external);
                    let r: Vec<Gf> = (0..k).map(|_| random_gf(&mut state)).collect();
                    for j in 1..=k {
                        for send_one in [false, true] {
                            let want = forest.bit_round(ell, j, &r[..j - 1], &external, send_one);
                            let got =
                                cross_round_sums(&cross, k, j, &point[..k], &r[..j - 1], send_one);
                            assert_eq!(
                                got, want,
                                "t={t} s={s} nibble={nibble} ell={ell} j={j} send_one={send_one}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Taking the bit rounds from one pass proves the same: same point,
    /// claim and transcript bytes, on both index paths.
    #[test]
    fn one_pass_bit_rounds_prove_identically() {
        use crate::bitz::transcript::build_kernel_prover;
        for (t, s) in [
            (4usize, 0usize),
            (4, 5),
            (5, 6),
            (6, 0),
            (6, 3),
            (6, 6),
            (7, 7),
            (9, 6),
            (10, 8),
            (11, 7),
        ] {
            let (packed, images) = random_grid(t, s, (t * 1000 + s) as u64);
            let mut state = 0x5151_5eed ^ (t * 31 + s) as u64;
            let zeta: Vec<Gf> = (0..s).map(|_| random_gf(&mut state)).collect();
            for nibble in [false, true] {
                let mode = PatternMode {
                    nibble,
                    nibble_from: 0,
                };
                let prove = |one_pass: bool| {
                    let forest = Forest::new(t, s, &packed, &images)
                        .with_patterns(mode)
                        .with_one_pass(one_pass);
                    let mut ps = build_kernel_prover(b"forest-one-pass/v1", b"instance");
                    let (point, claim) = forest.prove(&mut ps, &zeta);
                    (point, claim, ps.finish().narg_string)
                };
                let (per_round, one_pass) = (prove(false), prove(true));
                assert_eq!(
                    (&one_pass.0, &one_pass.1),
                    (&per_round.0, &per_round.1),
                    "t={t} s={s} nibble={nibble}"
                );
                assert!(
                    one_pass.2 == per_round.2,
                    "transcript t={t} s={s} nibble={nibble}"
                );
            }
        }
    }

    /// Keeping the column weights in the dense rounds' left half proves the
    /// same: same point, claim and transcript bytes, on the table-driven
    /// levels (whose JIT fold writes the weighted half) and the materialised
    /// ones, with and without the one-pass bit rounds.
    #[test]
    fn weighing_proves_identically() {
        use crate::bitz::transcript::build_kernel_prover;
        for (t, s) in [
            (4usize, 0usize),
            (4, 5),
            (5, 6),
            (6, 0),
            (6, 1),
            (6, 3),
            (7, 7),
            (9, 6),
            (10, 8),
            (11, 7),
        ] {
            let (packed, images) = random_grid(t, s, (t * 5003 + s) as u64);
            let mut state = 0x7e16_4ed0 ^ (t * 37 + s) as u64;
            let zeta: Vec<Gf> = (0..s).map(|_| random_gf(&mut state)).collect();
            for nibble in [false, true] {
                let mode = PatternMode {
                    nibble,
                    nibble_from: 0,
                };
                for one_pass in [false, true] {
                    let prove = |weighing: Weighing| {
                        let forest = Forest::new(t, s, &packed, &images)
                            .with_patterns(mode)
                            .with_one_pass(one_pass)
                            .with_weighing(weighing);
                        let mut ps = build_kernel_prover(b"forest-weighing/v1", b"instance");
                        let (point, claim) = forest.prove(&mut ps, &zeta);
                        (point, claim, ps.finish().narg_string)
                    };
                    let (off, on) = (prove(Weighing::Off), prove(Weighing::On));
                    let label = format!("t={t} s={s} nibble={nibble} one_pass={one_pass}");
                    assert_eq!((&on.0, &on.1), (&off.0, &off.1), "{label}");
                    assert!(on.2 == off.2, "transcript {label}");
                }
            }
        }
    }

    /// The two pattern modes prove the same: same point, claim and
    /// transcript bytes.
    #[test]
    fn pattern_modes_prove_identically() {
        use crate::bitz::transcript::build_kernel_prover;
        for (t, s) in [
            (4usize, 5usize),
            (5, 6),
            (6, 3),
            (6, 6),
            (7, 7),
            (9, 6),
            (10, 8),
            (11, 7),
        ] {
            let (packed, images) = random_grid(t, s, (t * 1000 + s) as u64);
            let mut state = 0x5151_5eed ^ (t * 31 + s) as u64;
            let zeta: Vec<Gf> = (0..s)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    Gf::new(state, state.rotate_left(29))
                })
                .collect();
            let mut runs = Vec::new();
            for nibble in [false, true] {
                {
                    // `nibble_from: 0`: these small grids would gather otherwise.
                    let mode = PatternMode {
                        nibble,
                        nibble_from: 0,
                    };
                    let forest = Forest::new(t, s, &packed, &images).with_patterns(mode);
                    let mut ps = build_kernel_prover(b"forest-patterns/v1", b"instance");
                    let (point, claim) = forest.prove(&mut ps, &zeta);
                    runs.push((nibble, point, claim, ps.finish().narg_string));
                }
            }
            let (_, point, claim, narg) = &runs[0];
            for (nibble, p, c, n) in &runs[1..] {
                assert_eq!((p, c), (point, claim), "t={t} s={s} nibble={nibble}");
                assert!(n == narg, "transcript t={t} s={s} nibble={nibble}");
            }
        }
    }
}
