//! Four row variables of the leaf layer, computed from the packed bits.
//!
//! Each leaf is `1 + bit * (image - 1)`. The cross sums therefore need
//! only single-bit and two-bit marginals of sixteen 256-entry buckets.
//! After the univariate challenge, two byte subset tables evaluate each
//! bound factor. Their bytes select the even and odd corners, respectively,
//! so both bytes are already present in the forest's nibble rows.

use std::mem::MaybeUninit;

use super::{
    Forest, Gf, NibbleRows, bit_sums, eq_table, kernels, transposed_eq, transposed_patterns,
};
use crate::cfg_chunks_mut;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const CORNERS: usize = 16;
/// Lookup granularity for binding the sixteen leaf corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeafTableLayout {
    /// Two 256-entry tables, indexed by the two pattern bytes.
    #[default]
    Byte,
    /// Four 16-entry tables, indexed by each byte's two nibbles.
    Nibble,
}

impl LeafTableLayout {
    fn len(self) -> usize {
        match self {
            Self::Byte => 512,
            Self::Nibble => 64,
        }
    }

    fn is_nibble(self) -> bool {
        self == Self::Nibble
    }
}

/// Two byte subset tables per `(child, remaining row)` after the skip.
pub(super) struct LeafSkipTables {
    data: Vec<Gf>,
    row_bits: usize,
    layout: LeafTableLayout,
}

impl Forest<'_> {
    /// `C[a,b] = Σ_(y,c) eq_y[y] eq_c[c] E_a(y,c) O_b(y,c)`, in
    /// row-major order. `external` is the incoming point in little-endian
    /// order, as in the other forest kernels. The skipped prefix is not
    /// included in these equality weights.
    pub(super) fn leaf_skip_cross(&self, external: &[Gf]) -> [Gf; CORNERS * CORNERS] {
        assert!(
            self.t >= 6,
            "the four-variable skip needs a remaining row bit"
        );
        assert_eq!(external.len(), self.t - 1 + self.s);
        let row_bits = self.t - 5;
        let rows = 1usize << row_bits;
        let groups = (1usize << self.s).div_ceil(64);
        let eq_t = transposed_eq(&eq_table(&external[..self.s]));
        let eq_y = eq_table(&external[self.s..self.s + row_bits]);
        let nibbles = self.nibbles();
        let row = |y: usize, scratch: &mut CrossBuckets, acc: &mut [Gf; 256]| {
            scratch.clear();
            for g in 0..groups {
                let pat = byte_patterns(self, nibbles, y, g);
                let weights = &eq_t[g * 64..(g + 1) * 64];
                for e in 0..4 {
                    let mut indices = [[0u8; 64]; 4];
                    for o in 0..4 {
                        for m in 0..64 {
                            let eb = (pat[e & 1][m] >> (4 * (e >> 1))) & 15;
                            let ob = (pat[2 + (o & 1)][m] >> (4 * (o >> 1))) & 15;
                            indices[o][m] = (eb << 4) | ob;
                        }
                    }
                    let buckets = &mut scratch.data[e * 1024..(e + 1) * 1024];
                    let (b0, rest) = buckets.split_at_mut(256);
                    let (b1, rest) = rest.split_at_mut(256);
                    let (b2, b3) = rest.split_at_mut(256);
                    kernels::scatter_add4([b0, b1, b2, b3], &indices, weights);
                }
            }
            let delta = |p: usize| -> [Gf; 16] {
                std::array::from_fn(|a| {
                    self.images[y | (a << row_bits) | (p << (self.t - 1))] - Gf::one()
                })
            };
            cross_row(&scratch.data, &delta(0), &delta(1), eq_y[y], acc);
        };
        let add = |mut a: [Gf; 256], b: [Gf; 256]| {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
            a
        };
        #[cfg(feature = "parallel")]
        let result = (0..rows)
            .into_par_iter()
            .with_min_len(1)
            .fold(
                || (CrossBuckets::new(), [Gf::zero(); 256]),
                |(mut scratch, mut acc), y| {
                    row(y, &mut scratch, &mut acc);
                    (scratch, acc)
                },
            )
            .map(|(_, acc)| acc)
            .reduce(|| [Gf::zero(); 256], add);
        #[cfg(not(feature = "parallel"))]
        let result = {
            let _ = add;
            let mut scratch = CrossBuckets::new();
            let mut acc = [Gf::zero(); 256];
            for y in 0..rows {
                row(y, &mut scratch, &mut acc);
            }
            acc
        };
        result
    }

    /// The original byte layout, retained as a reference path.
    #[cfg(test)]
    pub(super) fn leaf_skip_tables(&self, weights: &[Gf; 16]) -> LeafSkipTables {
        self.leaf_skip_tables_in(weights, Vec::new(), LeafTableLayout::Byte)
    }

    /// Build the challenge-dependent tables in an existing position-table allocation.
    pub(super) fn leaf_skip_tables_in(
        &self,
        weights: &[Gf; 16],
        mut data: Vec<Gf>,
        layout: LeafTableLayout,
    ) -> LeafSkipTables {
        assert!(
            self.t >= 6,
            "the four-variable skip needs a remaining row bit"
        );
        let row_bits = self.t - 5;
        let rows = 1usize << row_bits;
        let table_len = layout.len();
        let len = 2 * rows * table_len;
        data.clear();
        data.reserve(len);
        let weight_sum = weights.iter().copied().sum();
        cfg_chunks_mut!(&mut data.spare_capacity_mut()[..len], table_len)
            .enumerate()
            .for_each(|(q, table)| {
                self.write_leaf_table(
                    weights,
                    weight_sum,
                    q >> row_bits,
                    q & (rows - 1),
                    layout,
                    table,
                );
            });
        // SAFETY: every disjoint position table was fully initialized.
        unsafe { data.set_len(len) };
        LeafSkipTables {
            data,
            row_bits,
            layout,
        }
    }

    /// Build four local tables, immediately use them for the first ordinary
    /// round, and retain them for the following binding pass.
    pub(super) fn leaf_skip_tables_and_first_round(
        &self,
        weights: &[Gf; 16],
        external: &[Gf],
        send_one: bool,
        reuse: Vec<Gf>,
        layout: LeafTableLayout,
    ) -> (LeafSkipTables, (Gf, Gf)) {
        match layout {
            LeafTableLayout::Byte => {
                self.leaf_skip_tables_and_round_with::<false>(weights, external, send_one, reuse)
            }
            LeafTableLayout::Nibble => {
                self.leaf_skip_tables_and_round_with::<true>(weights, external, send_one, reuse)
            }
        }
    }

    fn leaf_skip_tables_and_round_with<const NIBBLE: bool>(
        &self,
        weights: &[Gf; 16],
        external: &[Gf],
        send_one: bool,
        mut data: Vec<Gf>,
    ) -> (LeafSkipTables, (Gf, Gf)) {
        assert!(
            self.t >= 6,
            "the four-variable skip needs a remaining row bit"
        );
        assert_eq!(external.len(), self.t - 1 + self.s);
        let layout = if NIBBLE {
            LeafTableLayout::Nibble
        } else {
            LeafTableLayout::Byte
        };
        let row_bits = self.t - 5;
        let half_rows = 1usize << (row_bits - 1);
        let table_len = layout.len();
        let len = 4 * half_rows * table_len;
        data.clear();
        data.reserve(len);
        let eq_t = transposed_eq(&eq_table(&external[..self.s]));
        let eq_y = eq_table(&external[self.s..self.s + row_bits - 1]);
        let nibbles = self.nibbles();
        let weight_sum = weights.iter().copied().sum();
        let out = &mut data.spare_capacity_mut()[..len];
        let (e, o) = out.split_at_mut(len / 2);
        let (e0, e1) = e.split_at_mut(len / 4);
        let (o0, o1) = o.split_at_mut(len / 4);
        let chunks = cfg_chunks_mut!(e0, table_len)
            .zip(cfg_chunks_mut!(e1, table_len))
            .zip(cfg_chunks_mut!(o0, table_len))
            .zip(cfg_chunks_mut!(o1, table_len))
            .enumerate();
        let row =
            |y: usize, out: [&mut [MaybeUninit<Gf>]; 4], buckets: &mut kernels::SplitSumBuckets| {
                let [el, eh, ol, oh] = out;
                let tables = [
                    self.write_leaf_table(weights, weight_sum, 0, y, layout, el),
                    self.write_leaf_table(weights, weight_sum, 0, y + half_rows, layout, eh),
                    self.write_leaf_table(weights, weight_sum, 1, y, layout, ol),
                    self.write_leaf_table(weights, weight_sum, 1, y + half_rows, layout, oh),
                ];
                let (end, inf) = table_round_row::<NIBBLE>(
                    self, nibbles, y, half_rows, &eq_t, tables, send_one, buckets,
                );
                (eq_y[y] * end, eq_y[y] * inf)
            };
        #[cfg(feature = "parallel")]
        let partials: Vec<(Gf, Gf)> = chunks
            .map_init(
                || kernels::SplitSumBuckets::for_layout(NIBBLE),
                |buckets, (y, (((el, eh), ol), oh))| row(y, [el, eh, ol, oh], buckets),
            )
            .collect();
        #[cfg(not(feature = "parallel"))]
        let partials: Vec<(Gf, Gf)> = {
            let mut buckets = kernels::SplitSumBuckets::for_layout(NIBBLE);
            chunks
                .map(|(y, (((el, eh), ol), oh))| row(y, [el, eh, ol, oh], &mut buckets))
                .collect()
        };
        // SAFETY: every table in each of the four quarters was written by its row task.
        unsafe { data.set_len(len) };
        let sums = partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (c, d)| (a + c, b + d));
        (
            LeafSkipTables {
                data,
                row_bits,
                layout,
            },
            sums,
        )
    }

    fn write_leaf_table<'a>(
        &self,
        weights: &[Gf; 16],
        weight_sum: Gf,
        p: usize,
        y: usize,
        layout: LeafTableLayout,
        out: &'a mut [MaybeUninit<Gf>],
    ) -> &'a [Gf] {
        assert_eq!(out.len(), layout.len());
        let row_bits = self.t - 5;
        let delta: [Gf; 16] = std::array::from_fn(|a| {
            weights[a] * (self.images[y | (a << row_bits) | (p << (self.t - 1))] - Gf::one())
        });
        let (components, entries) = if layout.is_nibble() {
            (4, 16)
        } else {
            (2, 256)
        };
        let mut subset = [Gf::zero(); 256];
        for component in 0..components {
            subset[0] = if component == 0 {
                weight_sum
            } else {
                Gf::zero()
            };
            out[component * entries].write(subset[0]);
            for pattern in 1..entries {
                let bit = pattern.trailing_zeros() as usize;
                let corner = if layout.is_nibble() {
                    2 * (bit + 4 * (component % 2)) + component / 2
                } else {
                    2 * bit + component
                };
                subset[pattern] = subset[pattern & (pattern - 1)] + delta[corner];
                out[component * entries + pattern].write(subset[pattern]);
            }
        }
        // SAFETY: all `layout.len()` elements above are initialized and
        // MaybeUninit<Gf> has the layout and alignment of Gf.
        unsafe { std::slice::from_raw_parts(out.as_ptr().cast::<Gf>(), out.len()) }
    }
}

impl LeafSkipTables {
    #[inline(always)]
    fn at(&self, p: usize, y: usize) -> &[Gf] {
        let len = self.layout.len();
        let start = ((p << self.row_bits) | y) * len;
        &self.data[start..start + len]
    }

    /// The first ordinary round after the four-variable skip, evaluated
    /// through the byte tables without materializing either factor.
    pub(super) fn first_round_sums(
        &self,
        forest: &Forest<'_>,
        external: &[Gf],
        send_one: bool,
    ) -> (Gf, Gf) {
        match self.layout {
            LeafTableLayout::Byte => {
                self.first_round_sums_with::<false>(forest, external, send_one)
            }
            LeafTableLayout::Nibble => {
                self.first_round_sums_with::<true>(forest, external, send_one)
            }
        }
    }

    fn first_round_sums_with<const NIBBLE: bool>(
        &self,
        forest: &Forest<'_>,
        external: &[Gf],
        send_one: bool,
    ) -> (Gf, Gf) {
        assert_eq!(self.row_bits, forest.t - 5);
        assert_eq!(external.len(), forest.t - 1 + forest.s);
        let half_rows = 1usize << (self.row_bits - 1);
        let eq_t = transposed_eq(&eq_table(&external[..forest.s]));
        let eq_y = eq_table(&external[forest.s..forest.s + self.row_bits - 1]);
        let nibbles = forest.nibbles();
        let row = |y: usize, buckets: &mut kernels::SplitSumBuckets| {
            let tables = [
                self.at(0, y),
                self.at(0, y + half_rows),
                self.at(1, y),
                self.at(1, y + half_rows),
            ];
            let (end, inf) = table_round_row::<NIBBLE>(
                forest, nibbles, y, half_rows, &eq_t, tables, send_one, buckets,
            );
            (eq_y[y] * end, eq_y[y] * inf)
        };
        #[cfg(feature = "parallel")]
        let partials: Vec<(Gf, Gf)> = (0..half_rows)
            .into_par_iter()
            .with_min_len(1)
            .map_init(
                || kernels::SplitSumBuckets::for_layout(NIBBLE),
                |buckets, y| row(y, buckets),
            )
            .collect();
        #[cfg(not(feature = "parallel"))]
        let partials: Vec<(Gf, Gf)> = {
            let mut buckets = kernels::SplitSumBuckets::for_layout(NIBBLE);
            (0..half_rows).map(|y| row(y, &mut buckets)).collect()
        };
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (c, d)| (a + c, b + d))
    }

    /// Binds the first ordinary challenge and computes the following
    /// round's message in the same pass that materializes the factors.
    /// With `WEIGH_LEFT`, the arena retains the column weights in E for
    /// the ordinary dense continuation.
    pub(super) fn bind_first_round_and_sums_into<const WEIGH_LEFT: bool>(
        self,
        forest: &Forest<'_>,
        rho: Gf,
        external: &[Gf],
        send_one: bool,
        arena: &mut Vec<Gf>,
    ) -> (Gf, Gf) {
        // Pre-scale only when the selected lookup layout saves at least
        // half of the folding multiplications at this column width.
        if (1usize << forest.s) >= 4 * self.layout.len() {
            self.bind_first_round_and_sums_with::<true, WEIGH_LEFT>(
                forest, rho, external, send_one, arena,
            )
        } else {
            self.bind_first_round_and_sums_with::<false, WEIGH_LEFT>(
                forest, rho, external, send_one, arena,
            )
        }
    }

    fn bind_first_round_and_sums_with<const PRE_SCALED: bool, const WEIGH_LEFT: bool>(
        self,
        forest: &Forest<'_>,
        rho: Gf,
        external: &[Gf],
        send_one: bool,
        arena: &mut Vec<Gf>,
    ) -> (Gf, Gf) {
        match self.layout {
            LeafTableLayout::Byte => self
                .bind_first_round_and_sums_layout::<false, PRE_SCALED, WEIGH_LEFT>(
                    forest, rho, external, send_one, arena,
                ),
            LeafTableLayout::Nibble => self
                .bind_first_round_and_sums_layout::<true, PRE_SCALED, WEIGH_LEFT>(
                    forest, rho, external, send_one, arena,
                ),
        }
    }

    fn bind_first_round_and_sums_layout<
        const NIBBLE: bool,
        const PRE_SCALED: bool,
        const WEIGH_LEFT: bool,
    >(
        mut self,
        forest: &Forest<'_>,
        rho: Gf,
        external: &[Gf],
        send_one: bool,
        arena: &mut Vec<Gf>,
    ) -> (Gf, Gf) {
        assert!(
            forest.t >= 7,
            "the fused skip continuation needs two remaining row bits"
        );
        assert_eq!(self.row_bits, forest.t - 5);
        assert_eq!(external.len(), forest.t - 1 + forest.s);
        let half_rows = 1usize << (self.row_bits - 1);
        if PRE_SCALED {
            let scale = [Gf::one() - rho, rho];
            cfg_chunks_mut!(self.data, self.layout.len())
                .enumerate()
                .for_each(|(q, table)| {
                    kernels::scale_in_place(table, &scale[(q / half_rows) & 1]);
                });
        }
        let cols = 1usize << forest.s;
        let groups = cols.div_ceil(64);
        let rows = half_rows / 2;
        let eq_t = transposed_eq(&eq_table(&external[..forest.s]));
        let eq_y = eq_table(&external[forest.s..forest.s + self.row_bits - 2]);
        let nibbles = forest.nibbles();
        let len = 4 * rows * cols;
        arena.clear();
        arena.reserve(len);
        let out: &mut [MaybeUninit<Gf>] = &mut arena.spare_capacity_mut()[..len];
        let (e, o) = out.split_at_mut(len / 2);
        let (e0, e1) = e.split_at_mut(len / 4);
        let (o0, o1) = o.split_at_mut(len / 4);
        let partials: Vec<(Gf, Gf)> = cfg_chunks_mut!(e0, cols)
            .zip(cfg_chunks_mut!(e1, cols))
            .zip(cfg_chunks_mut!(o0, cols))
            .zip(cfg_chunks_mut!(o1, cols))
            .enumerate()
            .map(|(y, (((el, eh), ol), oh))| {
                let tab: [&[Gf]; 8] = std::array::from_fn(|i| {
                    let (p, b1, b2) = (i >> 2, (i >> 1) & 1, i & 1);
                    self.at(p, y | (b1 * half_rows) | (b2 * rows))
                });
                let mut sums = kernels::Sums::zero();
                for g in 0..groups {
                    let mut patterns = [[[0u8; 64]; 2]; 8];
                    for b in 0..4 {
                        let yy = y | ((b >> 1) * half_rows) | ((b & 1) * rows);
                        let pat = byte_patterns(forest, nibbles, yy, g);
                        patterns[b] = [pat[0], pat[1]];
                        patterns[4 + b] = [pat[2], pat[3]];
                    }
                    let base = g * 64;
                    let range = base..base + 64.min(cols - base);
                    kernels::split_fold_group_layout::<NIBBLE, PRE_SCALED, WEIGH_LEFT>(
                        tab,
                        &patterns,
                        &rho,
                        &eq_t[base..base + 64],
                        send_one,
                        [&mut el[range.clone()], &mut eh[range.clone()]],
                        [&mut ol[range.clone()], &mut oh[range]],
                        &mut sums,
                    );
                }
                let (end, inf) = sums.finish();
                (eq_y[y] * end, eq_y[y] * inf)
            })
            .collect();
        // SAFETY: the fused kernel initialized each valid column in all
        // four disjoint output quarters before exposing the arena length.
        unsafe { arena.set_len(len) };
        partials
            .into_iter()
            .fold((Gf::zero(), Gf::zero()), |(a, b), (c, d)| (a + c, b + d))
    }

    /// Absorb the first ordinary challenge into the byte tables, then
    /// write the bound E and O halves into the forest's reusable arena.
    /// The output has `2^(t+s-5)` entries in total (five row coordinates,
    /// including the four packed ones, have been bound in each factor).
    pub(super) fn bind_first_round_into(
        mut self,
        forest: &Forest<'_>,
        rho: Gf,
        arena: &mut Vec<Gf>,
    ) {
        assert_eq!(self.row_bits, forest.t - 5);
        let half_rows = 1usize << (self.row_bits - 1);
        let cols = 1usize << forest.s;
        let groups = cols.div_ceil(64);
        let scale = [Gf::one() - rho, rho];
        cfg_chunks_mut!(self.data, self.layout.len())
            .enumerate()
            .for_each(|(q, table)| {
                kernels::scale_in_place(table, &scale[(q / half_rows) & 1]);
            });
        let len = 2 * half_rows * cols;
        arena.clear();
        arena.reserve(len);
        let nibbles = forest.nibbles();
        let (e_out, o_out) = arena.spare_capacity_mut()[..len].split_at_mut(len / 2);
        cfg_chunks_mut!(e_out, cols)
            .zip(cfg_chunks_mut!(o_out, cols))
            .enumerate()
            .for_each(|(y, (e, o))| {
                let tab = [
                    self.at(0, y),
                    self.at(0, y + half_rows),
                    self.at(1, y),
                    self.at(1, y + half_rows),
                ];
                for g in 0..groups {
                    let lo = byte_patterns(forest, nibbles, y, g);
                    let hi = byte_patterns(forest, nibbles, y + half_rows, g);
                    let base = g * 64;
                    for m in 0..64 {
                        let c = base + kernels::col_of(m);
                        if c < cols {
                            let left = table_value(tab[0], lo[0][m], lo[1][m], self.layout)
                                + table_value(tab[1], hi[0][m], hi[1][m], self.layout);
                            let right = table_value(tab[2], lo[2][m], lo[3][m], self.layout)
                                + table_value(tab[3], hi[2][m], hi[3][m], self.layout);
                            e[c].write(left);
                            o[c].write(right);
                        }
                    }
                }
            });
        // SAFETY: every row of each half was filled at every valid column.
        unsafe { arena.set_len(len) };
    }
}

#[allow(clippy::too_many_arguments)]
fn table_round_row<const NIBBLE: bool>(
    forest: &Forest<'_>,
    nibbles: Option<&NibbleRows>,
    y: usize,
    half_rows: usize,
    eq_t: &[Gf],
    tables: [&[Gf]; 4],
    send_one: bool,
    buckets: &mut kernels::SplitSumBuckets,
) -> (Gf, Gf) {
    buckets.clear();
    let groups = (1usize << forest.s).div_ceil(64);
    for g in 0..groups {
        let lo = byte_patterns(forest, nibbles, y, g);
        let hi = byte_patterns(forest, nibbles, y + half_rows, g);
        let patterns = [
            [lo[0], lo[1]],
            [hi[0], hi[1]],
            [lo[2], lo[3]],
            [hi[2], hi[3]],
        ];
        kernels::split_bucket_group_layout::<NIBBLE>(
            [tables[2], tables[3]],
            &patterns,
            &eq_t[g * 64..(g + 1) * 64],
            send_one,
            buckets,
        );
    }
    kernels::split_bucket_finish_layout::<NIBBLE>([tables[0], tables[1]], send_one, buckets)
}

#[inline(always)]
fn table_value(table: &[Gf], even: u8, odd: u8, layout: LeafTableLayout) -> Gf {
    match layout {
        LeafTableLayout::Byte => table[even as usize] + table[256 + odd as usize],
        LeafTableLayout::Nibble => {
            table[(even & 15) as usize]
                + table[16 + (even >> 4) as usize]
                + table[32 + (odd & 15) as usize]
                + table[48 + (odd >> 4) as usize]
        }
    }
}

/// The four bytes `[E_even, E_odd, O_even, O_odd]` of every column, in
/// the same transposed lane order as the ordinary forest kernels.
#[inline]
pub(super) fn byte_patterns(
    forest: &Forest<'_>,
    nibbles: Option<&NibbleRows>,
    y: usize,
    g: usize,
) -> [[u8; 64]; 4] {
    let row_bits = forest.t - 5;
    if let Some(nibbles) = nibbles {
        let even = nibbles.block(y, g);
        let odd = nibbles.block(y | (1usize << row_bits), g);
        let mut out = [[0u8; 64]; 4];
        for m in 0..64 {
            out[0][m] = even[m] as u8;
            out[1][m] = odd[m] as u8;
            out[2][m] = (even[m] >> 8) as u8;
            out[3][m] = (odd[m] >> 8) as u8;
        }
        out
    } else {
        std::array::from_fn(|i| {
            let (p, parity) = (i >> 1, i & 1);
            let mut words: [u64; 8] = std::array::from_fn(|v| {
                forest.packed_cols[g][y | (((2 * v) | parity) << row_bits) | (p << (forest.t - 1))]
            });
            transposed_patterns(&mut words)
        })
    }
}

struct CrossBuckets {
    data: Vec<Gf>,
}

impl CrossBuckets {
    fn new() -> Self {
        Self {
            data: vec![Gf::zero(); 16 * 256],
        }
    }

    fn clear(&mut self) {
        self.data.fill(Gf::zero());
    }
}

/// Nibble group g selects parity g&1 and the lower/upper four bits of
/// that parity's byte. Nibble bit i therefore identifies corner 8(g/2)+2i+(g%2).
#[inline(always)]
fn corner(g: usize, i: usize) -> usize {
    8 * (g >> 1) + 2 * i + (g & 1)
}

fn cross_row(
    bucket: &[Gf],
    delta_e: &[Gf; 16],
    delta_o: &[Gf; 16],
    row_weight: Gf,
    acc: &mut [Gf; 256],
) {
    let zero = Gf::zero();
    let mut total_weight = zero;
    let mut single_e = [zero; 16];
    let mut single_o = [zero; 16];
    let mut both = [[zero; 16]; 16];
    for e_group in 0..4 {
        for o_group in 0..4 {
            let bucket = &bucket[(4 * e_group + o_group) * 256..(4 * e_group + o_group + 1) * 256];
            let mut total = [zero; 16];
            let mut by_o = [[zero; 16]; 4];
            for e_pattern in 0..16 {
                let (sum, bits) = bit_sums(&bucket[16 * e_pattern..16 * (e_pattern + 1)]);
                total[e_pattern] = sum;
                for i in 0..4 {
                    by_o[i][e_pattern] = bits[i];
                }
            }
            for (o_bit, column) in by_o.iter().enumerate() {
                let (sum, bits) = bit_sums(column);
                let beta = corner(o_group, o_bit);
                if e_group == o_group {
                    single_o[beta] = sum;
                }
                for (e_bit, &value) in bits.iter().enumerate() {
                    both[corner(e_group, e_bit)][beta] = value;
                }
            }
            if e_group == o_group {
                let (sum, bits) = bit_sums(&total);
                total_weight = sum;
                for (e_bit, &value) in bits.iter().enumerate() {
                    single_e[corner(e_group, e_bit)] = value;
                }
            }
        }
    }
    let base = row_weight * total_weight;
    let weighted_e: [Gf; 16] = std::array::from_fn(|a| row_weight * delta_e[a]);
    let e_single: [Gf; 16] = std::array::from_fn(|a| weighted_e[a] * single_e[a]);
    let o_single: [Gf; 16] = std::array::from_fn(|b| row_weight * delta_o[b] * single_o[b]);
    for a in 0..16 {
        for b in 0..16 {
            acc[16 * a + b] +=
                base + e_single[a] + o_single[b] + weighted_e[a] * delta_o[b] * both[a][b];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::PatternMode;
    use super::*;

    fn word(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn field(state: &mut u64) -> Gf {
        Gf::from_polynomial_words([word(state), word(state)])
    }

    fn fixture(t: usize, s: usize) -> (Vec<Vec<u64>>, Vec<Gf>, Vec<Gf>, [Gf; 16]) {
        let mut state = 0x1D95_7BF4_39A0_78E1 ^ (t as u64) << 32 ^ s as u64;
        let packed = (0..(1usize << s).div_ceil(64))
            .map(|_| (0..1usize << t).map(|_| word(&mut state)).collect())
            .collect();
        let images = (0..1usize << t)
            .map(|i| {
                if i % 9 == 0 {
                    Gf::one()
                } else {
                    field(&mut state)
                }
            })
            .collect();
        let external = (0..t - 1 + s)
            .map(|i| match i % 5 {
                0 => Gf::zero(),
                1 => Gf::one(),
                _ => field(&mut state),
            })
            .collect();
        let weights = std::array::from_fn(|_| field(&mut state));
        (packed, images, external, weights)
    }

    fn leaf(forest: &Forest<'_>, p: usize, a: usize, y: usize, c: usize) -> Gf {
        let row = y | (a << (forest.t - 5)) | (p << (forest.t - 1));
        if (forest.packed_cols[c >> 6][row] >> (c & 63)) & 1 == 0 {
            Gf::one()
        } else {
            forest.images[row]
        }
    }

    fn bound(forest: &Forest<'_>, weights: &[Gf; 16], p: usize, y: usize, c: usize) -> Gf {
        (0..16).fold(Gf::zero(), |v, a| v + weights[a] * leaf(forest, p, a, y, c))
    }

    #[test]
    fn four_variable_cross_sums_match_dense_leaves() {
        for t in [6, 7] {
            for s in [0, 3, 6, 7] {
                let (packed, images, external, _) = fixture(t, s);
                let plain = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                    nibble: false,
                    cache: false,
                    nibble_from: 0,
                });
                let nibble = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                    nibble: true,
                    cache: false,
                    nibble_from: 0,
                });
                let mut expected = [Gf::zero(); 256];
                let eq_c = eq_table(&external[..s]);
                let eq_y = eq_table(&external[s..s + t - 5]);
                for (y, &wy) in eq_y.iter().enumerate() {
                    for (c, &wc) in eq_c.iter().enumerate() {
                        for a in 0..16 {
                            for b in 0..16 {
                                expected[16 * a + b] +=
                                    wy * wc * leaf(&plain, 0, a, y, c) * leaf(&plain, 1, b, y, c);
                            }
                        }
                    }
                }
                assert_eq!(
                    plain.leaf_skip_cross(&external),
                    expected,
                    "gather t={t} s={s}"
                );
                assert_eq!(
                    nibble.leaf_skip_cross(&external),
                    expected,
                    "nibble t={t} s={s}"
                );
            }
        }
    }

    #[test]
    fn split_byte_tables_round_and_binding_match_dense_leaves() {
        for t in [6, 7, 8] {
            for s in [0, 3, 6, 7] {
                let (packed, images, external, weights) = fixture(t, s);
                for use_nibble in [false, true] {
                    let forest = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                        nibble: use_nibble,
                        cache: false,
                        nibble_from: 0,
                    });
                    let half_rows = 1usize << (t - 6);
                    let cols = 1usize << s;
                    let tables = forest.leaf_skip_tables(&weights);
                    let eq_c = eq_table(&external[..s]);
                    let eq_y = eq_table(&external[s..s + t - 6]);
                    for send_one in [false, true] {
                        let mut expected = (Gf::zero(), Gf::zero());
                        for (y, &wy) in eq_y.iter().enumerate() {
                            for (c, &wc) in eq_c.iter().enumerate() {
                                let e0 = bound(&forest, &weights, 0, y, c);
                                let e1 = bound(&forest, &weights, 0, y + half_rows, c);
                                let o0 = bound(&forest, &weights, 1, y, c);
                                let o1 = bound(&forest, &weights, 1, y + half_rows, c);
                                expected.0 += wy * wc * if send_one { e1 * o1 } else { e0 * o0 };
                                expected.1 += wy * wc * (e1 - e0) * (o1 - o0);
                            }
                        }
                        assert_eq!(
                            tables.first_round_sums(&forest, &external, send_one),
                            expected,
                            "t={t} s={s} nibble={use_nibble} endpoint={send_one}"
                        );
                    }
                    for rho in [Gf::zero(), Gf::one(), weights[0]] {
                        let tables = forest.leaf_skip_tables(&weights);
                        let mut arena = vec![Gf::one(); 4 * half_rows * cols];
                        let capacity = arena.capacity();
                        tables.bind_first_round_into(&forest, rho, &mut arena);
                        assert_eq!(arena.capacity(), capacity, "reuse existing storage");
                        assert_eq!(arena.len(), 2 * half_rows * cols);
                        for p in 0..2 {
                            for y in 0..half_rows {
                                for c in 0..cols {
                                    let lo = bound(&forest, &weights, p, y, c);
                                    let hi = bound(&forest, &weights, p, y + half_rows, c);
                                    assert_eq!(
                                        arena[(p * half_rows + y) * cols + c],
                                        lo + rho * (hi - lo),
                                        "t={t} s={s} nibble={use_nibble} p={p} y={y} c={c}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lookup_layouts_reuse_storage_and_fuse_table_construction() {
        for (t, s) in [(6, 0), (6, 3), (7, 7), (7, 8), (7, 11)] {
            let (packed, images, external, weights) = fixture(t, s);
            for nibble_rows in [false, true] {
                let forest = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                    nibble: nibble_rows,
                    cache: false,
                    nibble_from: 0,
                });
                let rows = 1usize << (t - 5);
                let half_rows = rows / 2;
                let cols = 1usize << s;
                let eq_c = eq_table(&external[..s]);
                let eq_y = eq_table(&external[s..s + t - 6]);
                let factors: [Vec<Gf>; 2] = std::array::from_fn(|p| {
                    (0..rows * cols)
                        .map(|i| bound(&forest, &weights, p, i / cols, i % cols))
                        .collect()
                });
                for send_one in [false, true] {
                    let mut expected = (Gf::zero(), Gf::zero());
                    for (y, &wy) in eq_y.iter().enumerate() {
                        for (c, &wc) in eq_c.iter().enumerate() {
                            let i = y * cols + c;
                            let (e0, e1, o0, o1) = (
                                factors[0][i],
                                factors[0][half_rows * cols + i],
                                factors[1][i],
                                factors[1][half_rows * cols + i],
                            );
                            expected.0 += wy * wc * if send_one { e1 * o1 } else { e0 * o0 };
                            expected.1 += wy * wc * (e1 - e0) * (o1 - o0);
                        }
                    }
                    for layout in [LeafTableLayout::Byte, LeafTableLayout::Nibble] {
                        let reuse = vec![Gf::one(); 2 * rows * 512];
                        let ptr = reuse.as_ptr();
                        let tables = forest.leaf_skip_tables_in(&weights, reuse, layout);
                        assert_eq!(tables.data.as_ptr(), ptr, "reuse allocation");
                        assert_eq!(tables.data.len(), 2 * rows * layout.len());
                        assert_eq!(
                            tables.first_round_sums(&forest, &external, send_one),
                            expected
                        );
                        let reuse = vec![Gf::one(); 2 * rows * 512];
                        let ptr = reuse.as_ptr();
                        let (fused, sums) = forest.leaf_skip_tables_and_first_round(
                            &weights, &external, send_one, reuse, layout,
                        );
                        assert_eq!(fused.data.as_ptr(), ptr, "fused allocation reuse");
                        assert_eq!(fused.data, tables.data, "fused and separate tables");
                        assert_eq!(sums, expected, "fused dense first-round oracle");
                        let rho = weights[0];
                        let mut arena = Vec::new();
                        fused.bind_first_round_into(&forest, rho, &mut arena);
                        for p in 0..2 {
                            for i in 0..half_rows * cols {
                                let lo = factors[p][i];
                                let hi = factors[p][half_rows * cols + i];
                                assert_eq!(arena[p * half_rows * cols + i], lo + rho * (hi - lo));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fused_skip_binding_and_next_round_match_dense_leaves() {
        // The last case also selects pre-scaling through the public
        // width threshold; the others exercise its unscaled branch.
        for (t, s) in [(7, 0), (7, 3), (8, 6), (8, 7), (7, 11)] {
            let (packed, images, external, weights) = fixture(t, s);
            for nibble in [false, true] {
                let forest = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                    nibble,
                    cache: false,
                    nibble_from: 0,
                });
                let cols = 1usize << s;
                let half_rows = 1usize << (t - 6);
                let rows = half_rows / 2;
                let half_len = half_rows * cols;
                let eq_c = eq_table(&external[..s]);
                let eq_y = eq_table(&external[s..s + t - 7]);
                let factors: [Vec<Gf>; 2] = std::array::from_fn(|p| {
                    (0..2 * half_len)
                        .map(|i| bound(&forest, &weights, p, i / cols, i % cols))
                        .collect()
                });
                for rho in [Gf::zero(), Gf::one(), weights[0]] {
                    let mut folded = vec![Gf::zero(); 2 * half_len];
                    for p in 0..2 {
                        for i in 0..half_len {
                            let lo = factors[p][i];
                            let hi = factors[p][half_len + i];
                            folded[p * half_len + i] = lo + rho * (hi - lo);
                        }
                    }
                    for send_one in [false, true] {
                        let mut expected_sums = (Gf::zero(), Gf::zero());
                        for (y, &wy) in eq_y.iter().enumerate() {
                            for (c, &wc) in eq_c.iter().enumerate() {
                                let i = y * cols + c;
                                let e0 = folded[i];
                                let e1 = folded[rows * cols + i];
                                let o0 = folded[half_len + i];
                                let o1 = folded[half_len + rows * cols + i];
                                expected_sums.0 +=
                                    wy * wc * if send_one { e1 * o1 } else { e0 * o0 };
                                expected_sums.1 += wy * wc * (e1 - e0) * (o1 - o0);
                            }
                        }
                        for carry in [false, true] {
                            let mut expected_arena = folded.clone();
                            if carry {
                                for (i, value) in expected_arena[..half_len].iter_mut().enumerate()
                                {
                                    *value *= eq_c[i % cols];
                                }
                            }
                            for layout in [LeafTableLayout::Byte, LeafTableLayout::Nibble] {
                                for pre_scaled in [false, true] {
                                    let tables =
                                        forest.leaf_skip_tables_in(&weights, Vec::new(), layout);
                                    let mut arena = vec![Gf::one(); 4 * half_len];
                                    let capacity = arena.capacity();
                                    let sums = match (pre_scaled, carry) {
                                        (false, false) => tables
                                            .bind_first_round_and_sums_with::<false, false>(
                                                &forest, rho, &external, send_one, &mut arena,
                                            ),
                                        (false, true) => tables
                                            .bind_first_round_and_sums_with::<false, true>(
                                                &forest, rho, &external, send_one, &mut arena,
                                            ),
                                        (true, false) => tables
                                            .bind_first_round_and_sums_with::<true, false>(
                                                &forest, rho, &external, send_one, &mut arena,
                                            ),
                                        (true, true) => tables
                                            .bind_first_round_and_sums_with::<true, true>(
                                                &forest, rho, &external, send_one, &mut arena,
                                            ),
                                    };
                                    assert_eq!(
                                        arena.capacity(),
                                        capacity,
                                        "reuse existing storage"
                                    );
                                    assert_eq!(
                                        arena, expected_arena,
                                        "t={t} s={s} nibble={nibble} pre_scaled={pre_scaled} carry={carry}"
                                    );
                                    assert_eq!(
                                        sums, expected_sums,
                                        "t={t} s={s} nibble={nibble} pre_scaled={pre_scaled} carry={carry} endpoint={send_one}"
                                    );
                                }
                                let tables =
                                    forest.leaf_skip_tables_in(&weights, Vec::new(), layout);
                                let mut arena = Vec::new();
                                let sums = if carry {
                                    tables.bind_first_round_and_sums_into::<true>(
                                        &forest, rho, &external, send_one, &mut arena,
                                    )
                                } else {
                                    tables.bind_first_round_and_sums_into::<false>(
                                        &forest, rho, &external, send_one, &mut arena,
                                    )
                                };
                                assert_eq!(
                                    arena, expected_arena,
                                    "automatic pre-scaling t={t} s={s}"
                                );
                                assert_eq!(
                                    sums, expected_sums,
                                    "automatic pre-scaling t={t} s={s}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
