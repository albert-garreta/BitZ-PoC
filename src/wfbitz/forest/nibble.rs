//! The committed bits regrouped once for the table-driven levels, and the
//! table indices read off them.
//!
//! With `H = t − 4`, every table index the levels below the materialised
//! ones use is a set of bits of one column at rows `y + w·2^H` (`y < 2^H`,
//! `w < 16`): level `ell`'s 8-bit patterns (its JIT round and fold, and
//! level 4's products for `ell = 3`) are eight of those sixteen bits, and
//! each bit round's tuple or pair byte selects among them too (the paired
//! first round reads two adjacent `y`). [`NibbleRows`] stores the sixteen
//! bits of each `(y, column)` as one `u16`, built in one pass over the
//! packed columns (sixteen word loads and two 8×8 block transposes per 64
//! columns), so each pass reads its indices off a sequential stream with a
//! fixed bit selection ([`Selector`]: four 16-entry nibble lookups, 16
//! columns per `TBL`) instead of gathering strided words and transposing
//! them again. [`PatternCache`] keeps a level's pattern blocks for the
//! passes that read the same ones again (the JIT fold after its level's JIT
//! round; level 3's round and fold and level 4's rebuild after the build
//! pass).
//!
//! Entries keep the transposed column order of the block transposes
//! ([`super::kernels::col_of`]), so every selected block is byte-identical
//! to the one the gather-and-transpose path produces.

use std::mem::MaybeUninit;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::transposed_patterns;
use crate::cfg_chunks_mut;

/// The sixteen bits of each `(y, column)`: entry `(y, g, m)` (at
/// `(y·groups + g)·64 + m`) has, as bit `w`, the bit of column
/// `64g + col_of(m)` at row `y + w·2^H`.
pub(crate) struct NibbleRows {
    data: Vec<u16>,
    groups: usize,
}

impl NibbleRows {
    /// One pass over `packed_cols` (`packed_cols[g][row]`: bit `row` of
    /// columns `64g..64g+63`, `2^t` rows). A row's sixteen words lie `2^H`
    /// apart — at our shapes a multiple of the L1 set stride, so all sixteen
    /// share one set and evict each other before the next row reads the rest
    /// of their lines — so a task takes eight consecutive rows and loads each
    /// line once.
    pub(crate) fn new(t: usize, packed_cols: &[Vec<u64>], groups: usize) -> Self {
        let h = t - 4;
        let block = 8usize.min(1 << h);
        let len = (1usize << h) * groups * 64;
        let mut data: Vec<u16> = Vec::with_capacity(len);
        let spare = &mut data.spare_capacity_mut()[..len];
        cfg_chunks_mut!(spare, block * groups * 64)
            .enumerate()
            .for_each(|(i, rows)| {
                let y0 = i * block;
                for g in 0..groups {
                    let col = &packed_cols[g];
                    // lines[w][j]: the word of row y0 + j at nibble position w.
                    let mut lines = [[0u64; 8]; 16];
                    for (w, line) in lines.iter_mut().enumerate() {
                        let start = y0 | (w << h);
                        line[..block].copy_from_slice(&col[start..start + block]);
                    }
                    for j in 0..block {
                        let mut lo: [u64; 8] = std::array::from_fn(|w| lines[w][j]);
                        let mut hi: [u64; 8] = std::array::from_fn(|w| lines[w + 8][j]);
                        let lo = transposed_patterns(&mut lo);
                        let hi = transposed_patterns(&mut hi);
                        let at = (j * groups + g) * 64;
                        interleave_bytes(&lo, &hi, &mut rows[at..at + 64]);
                    }
                }
            });
        // SAFETY: every row chunk was written in full above.
        unsafe { data.set_len(len) };
        Self { data, groups }
    }

    /// Every column's integer fold `Σ_b exponents[b]·bit(c, b)` (wrapping,
    /// the same `u128`s as [`crate::wfbitz::fold::fold_columns`]) off the
    /// rows: entry `(y, c)` holds the bits of rows `y + w·2^H`, so for each
    /// `y` two 256-entry tables of subset sums of those rows' exponents
    /// (`w < 8` and `w ≥ 8`) make it two lookups, and every entry is read
    /// once, in order.
    pub(crate) fn column_folds(&self, t: usize, exponents: &[u128], cols: usize) -> Vec<u128> {
        let h = t - 4;
        let rows = 1usize << h;
        let lanes = self.groups * 64;
        assert_eq!(exponents.len(), 1usize << t);
        assert_eq!(self.data.len(), rows * lanes);
        assert!(cols <= lanes);
        let fold_rows = |range: std::ops::Range<usize>, acc: &mut [u128]| {
            let mut lo = [0u128; 256];
            let mut hi = [0u128; 256];
            for y in range {
                for v in 1..256usize {
                    let w = v.trailing_zeros() as usize;
                    lo[v] = lo[v & (v - 1)].wrapping_add(exponents[y | (w << h)]);
                    hi[v] = hi[v & (v - 1)].wrapping_add(exponents[y | ((w + 8) << h)]);
                }
                let entries = &self.data[y * lanes..(y + 1) * lanes];
                for (a, &e) in acc.iter_mut().zip(entries) {
                    let sum = lo[(e & 0xFF) as usize].wrapping_add(hi[(e >> 8) as usize]);
                    *a = a.wrapping_add(sum);
                }
            }
        };
        #[cfg(feature = "parallel")]
        let acc = {
            let chunk = rows
                .div_ceil(4 * rayon::current_num_threads().max(1))
                .max(1);
            (0..rows.div_ceil(chunk))
                .into_par_iter()
                .map(|i| {
                    let mut acc = vec![0u128; lanes];
                    fold_rows(i * chunk..((i + 1) * chunk).min(rows), &mut acc);
                    acc
                })
                .reduce(
                    || vec![0u128; lanes],
                    |mut a, b| {
                        for (x, y) in a.iter_mut().zip(b) {
                            *x = x.wrapping_add(y);
                        }
                        a
                    },
                )
        };
        #[cfg(not(feature = "parallel"))]
        let acc = {
            let mut acc = vec![0u128; lanes];
            fold_rows(0..rows, &mut acc);
            acc
        };
        // Entry `64g + m` is column `64g + col_of(m)`.
        let mut folds = vec![0u128; cols];
        for (i, &a) in acc.iter().enumerate() {
            let c = (i & !63) | super::kernels::col_of(i & 63);
            if c < cols {
                folds[c] = a;
            }
        }
        folds
    }

    /// Whether these rows are the ones [`NibbleRows::new`] builds for `2^t`
    /// rows and `groups` column groups.
    pub(crate) fn fits(&self, t: usize, groups: usize) -> bool {
        t >= 4 && self.groups == groups && self.data.len() == (1usize << (t - 4)) * groups * 64
    }

    /// The 64 entries of `(y, g)`.
    #[inline(always)]
    pub(crate) fn block(&self, y: usize, g: usize) -> &[u16; 64] {
        let start = (y * self.groups + g) * 64;
        self.data[start..start + 64].try_into().expect("64 entries")
    }
}

/// `out[m] = lo[m] | hi[m] << 8`.
#[inline(always)]
fn interleave_bytes(lo: &[u8; 64], hi: &[u8; 64], out: &mut [MaybeUninit<u16>]) {
    debug_assert_eq!(out.len(), 64);
    for m in 0..64 {
        out[m].write(u16::from(lo[m]) | (u16::from(hi[m]) << 8));
    }
}

/// A fixed selection of bits of a [`NibbleRows`] entry into one byte:
/// output bit `k` is entry bit `w_k` for the `(k, w_k)` it was built from,
/// every other output bit zero. Four 16-entry tables, one per entry nibble,
/// ORed.
#[derive(Clone, Copy)]
pub(crate) struct Selector {
    tables: [[u8; 16]; 4],
}

impl Selector {
    pub(crate) fn new(bits: impl IntoIterator<Item = (usize, usize)>) -> Self {
        let mut tables = [[0u8; 16]; 4];
        for (k, w) in bits {
            debug_assert!(k < 8 && w < 16);
            let (nibble, bit) = (w >> 2, w & 3);
            for (x, entry) in tables[nibble].iter_mut().enumerate() {
                if (x >> bit) & 1 == 1 {
                    *entry |= 1 << k;
                }
            }
        }
        Self { tables }
    }

    /// The selection of one entry (the portable path and the reference).
    #[cfg_attr(
        all(target_arch = "aarch64", target_feature = "neon"),
        allow(dead_code)
    )]
    #[inline(always)]
    pub(crate) fn apply(&self, x: u16) -> u8 {
        let x = x as usize;
        self.tables[0][x & 15]
            | self.tables[1][(x >> 4) & 15]
            | self.tables[2][(x >> 8) & 15]
            | self.tables[3][x >> 12]
    }
}

/// `out[i][m] = sel[i]` applied to entry `m` of `block`.
#[inline(always)]
pub(crate) fn select<const K: usize>(
    block: &[u16; 64],
    sel: &[Selector; K],
    out: &mut [[u8; 64]; K],
) {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    neon::select(block, sel, out);
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    for (s, o) in sel.iter().zip(out.iter_mut()) {
        for m in 0..64 {
            o[m] = s.apply(block[m]);
        }
    }
}

/// `a`'s selection by `sa` ORed with `b`'s by `sb` (the paired first bit
/// round, whose tuple byte holds two adjacent rows).
#[inline(always)]
pub(crate) fn select_pair(a: &[u16; 64], b: &[u16; 64], sa: &Selector, sb: &Selector) -> [u8; 64] {
    let mut x = [[0u8; 64]; 1];
    let mut y = [[0u8; 64]; 1];
    select(a, core::array::from_ref(sa), &mut x);
    select(b, core::array::from_ref(sb), &mut y);
    let mut out = [0u8; 64];
    for m in 0..64 {
        out[m] = x[0][m] | y[0][m];
    }
    out
}

/// Level `ell`'s pattern of corner `p` (`ell + kk = 3`): bit
/// `i = (v << ell) | u` is row `y + v·2^H + p·2^{t−ell−1} + u·2^{t−ell}`,
/// i.e. entry bit `v + p·2^{3−ell} + u·2^{4−ell}`.
pub(crate) fn jit_selectors(ell: usize) -> [Selector; 2] {
    debug_assert!(ell <= 3);
    std::array::from_fn(|p| {
        Selector::new((0..8).map(|i| {
            let (v, u) = (i >> ell, i & ((1 << ell) - 1));
            (i, v + (p << (3 - ell)) + (u << (4 - ell)))
        }))
    })
}

/// Entry bit of pattern bit `i` of bit-round corner `(p, b)` at level
/// `ell` after `kk` folds, for a row `y` with `q = y >> H`: row
/// `y + b·2^{y_bits} + v·2^{low_bits} + p·2^{t−ell−1} + u·2^{t−ell}`.
fn bit_round_w(ell: usize, kk: usize, q: usize, p: usize, b: usize, i: usize) -> usize {
    let (v, u) = (i >> ell, i & ((1 << ell) - 1));
    q + (b << (2 - ell - kk)) + (v << (3 - ell - kk)) + (p << (3 - ell)) + (u << (4 - ell))
}

/// The selectors of bit round `j` of level `ell` (`ell + j ≤ 3`).
pub(crate) enum BitSelectors {
    /// `nb = 1`, rows paired (`ell = 0`, `j = 1`): per `q`, the low nibble
    /// from row `y0` and the high one from `y0 + 1`.
    Paired(Vec<[Selector; 2]>),
    /// `nb ≤ 2`: the tuple byte, per `q`.
    Tuple(Vec<Selector>),
    /// `nb = 4` (`q = 0`): the `ll`, `hh`, `lh`, `hl` pair bytes.
    Pairs([Selector; 4]),
}

impl BitSelectors {
    pub(crate) fn new(ell: usize, j: usize, paired: bool) -> Self {
        let kk = j - 1;
        let nb = 1usize << (ell + kk);
        let qs = 1usize << (2 - ell - kk);
        let corner_bits = |q: usize, corner: usize, shift: usize| {
            let (p, b) = (corner >> 1, corner & 1);
            (0..nb).map(move |i| (shift + i, bit_round_w(ell, kk, q, p, b, i)))
        };
        if paired {
            debug_assert_eq!(nb, 1);
            Self::Paired(
                (0..qs)
                    .map(|q| {
                        let a =
                            Selector::new((0..4).flat_map(|corner| corner_bits(q, corner, corner)));
                        let b = Selector::new(
                            (0..4).flat_map(|corner| corner_bits(q, corner, 4 + corner)),
                        );
                        [a, b]
                    })
                    .collect(),
            )
        } else if nb <= 2 {
            Self::Tuple(
                (0..qs)
                    .map(|q| {
                        Selector::new((0..4).flat_map(|corner| corner_bits(q, corner, corner * nb)))
                    })
                    .collect(),
            )
        } else {
            // E_lo, E_hi, O_lo, O_hi = corners 0, 1, 2, 3; each pair byte
            // has its O pattern low and its E pattern high.
            let pair = |o: usize, e: usize| {
                Selector::new(corner_bits(0, o, 0).chain(corner_bits(0, e, 4)))
            };
            Self::Pairs([pair(2, 0), pair(3, 1), pair(3, 0), pair(2, 1)])
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use core::arch::aarch64::{
        vandq_u8, vdupq_n_u8, vld1q_u8, vorrq_u8, vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vuzp1q_u8,
        vuzp2q_u8,
    };

    use super::Selector;

    /// Sixteen entries at a time: the low and high bytes unzipped, split
    /// into nibbles once, then four `TBL`s and three `ORR`s per selector.
    #[inline(always)]
    pub(super) fn select<const K: usize>(
        block: &[u16; 64],
        sel: &[Selector; K],
        out: &mut [[u8; 64]; K],
    ) {
        // SAFETY: `block` is 128 bytes, each `out[i]` 64; every access
        // below is at an offset `< 128` / `< 64` in 16-byte steps.
        unsafe {
            let tables: [[core::arch::aarch64::uint8x16_t; 4]; K] = std::array::from_fn(|i| {
                std::array::from_fn(|n| vld1q_u8(sel[i].tables[n].as_ptr()))
            });
            let mask = vdupq_n_u8(0x0F);
            let src = block.as_ptr().cast::<u8>();
            for c in 0..4 {
                let b0 = vld1q_u8(src.add(32 * c));
                let b1 = vld1q_u8(src.add(32 * c + 16));
                let lo = vuzp1q_u8(b0, b1);
                let hi = vuzp2q_u8(b0, b1);
                let n0 = vandq_u8(lo, mask);
                let n1 = vshrq_n_u8(lo, 4);
                let n2 = vandq_u8(hi, mask);
                let n3 = vshrq_n_u8(hi, 4);
                for i in 0..K {
                    let t = &tables[i];
                    let v = vorrq_u8(
                        vorrq_u8(vqtbl1q_u8(t[0], n0), vqtbl1q_u8(t[1], n1)),
                        vorrq_u8(vqtbl1q_u8(t[2], n2), vqtbl1q_u8(t[3], n3)),
                    );
                    vst1q_u8(out[i].as_mut_ptr().add(16 * c), v);
                }
            }
        }
    }
}
