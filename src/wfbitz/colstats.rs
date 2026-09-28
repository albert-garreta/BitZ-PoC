//! Column statistics of the committed bits at the inner-product sumcheck's
//! row point: one pass that serves both the sumcheck's column table and
//! the ring switch's 128 claims.
//!
//! Column `c`'s bits are, in the packed message the ring switch opens,
//! the `B = 2^{t−7}` consecutive elements `P[c·B + i]` whose bit `u` is row
//! bit `128·i + u` ([`crate::ligerito_flock`]'s packing: words `2i` and
//! `2i + 1` of the column's row). With the row point `r_b` (low
//! coordinates first) the statistics are
//!
//! `Z[c][u] = Σ_i eq(r_b[7..t], i)·bit(c, 128·i + u)`,
//!
//! and, because `eq(b, r_b) = eq(b mod 128, r_b[..7])·eq(b div 128, r_b[7..])`
//! and the ring-switch suffix point is `(r_b[7..t], r_c)`:
//!
//! - the row-folded table the column rounds run on is
//!   `M'[c] = Σ_u eq(r_b[..7], u)·Z[c][u]` (`sumcheck::fold_rows_point`);
//! - once the column rounds drew `r_c`, the claims are
//!   `ŝ_v[u] = Σ_c eq(r_c, c)·Z[c][u]` (`fold_1b_rows` over the tensor).
//!
//! Both are the same field sums regrouped, so every message is
//! byte-identical; the one statistics pass (flock's split method-of-four-
//! Russians fold with a column as the block) replaces two passes over the
//! bits. `Z` holds `2^{s+11}` bytes (8 MB at `n = 28`).

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::cfg_into_iter;
use crate::ligerito::{subset_sums_4, transpose_8x8_bits};
use crate::poly::utils::build_eq_x_r_vec;
use crate::utils::wide_mul::WideMulAcc;
use field::Gf128 as Gf;

/// The in-element (packing) coordinates: a packed element holds 128 bits.
const LOG_PACKING: usize = 7;
/// Bits per packed element.
const PACKING: usize = 1 << LOG_PACKING;
/// Packed elements per method-of-four-Russians chunk.
const CHUNK: usize = 16;

/// When the inner-product sumcheck computes [`ColumnStats`], read once
/// from `WFBITZ_ZSTATS`: `0` never, `1` on every grid it fits
/// ([`ColumnStats::fits`]), otherwise (the default) where it pays
/// ([`ColumnStats::pays`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZStats {
    Off,
    Auto,
    Force,
}

impl ZStats {
    pub(crate) fn from_env() -> Self {
        static MODE: std::sync::OnceLock<ZStats> = std::sync::OnceLock::new();
        *MODE.get_or_init(|| match std::env::var("WFBITZ_ZSTATS").as_deref() {
            Ok("0") => Self::Off,
            Ok("1") => Self::Force,
            _ => Self::Auto,
        })
    }

    /// Whether the statistics serve a `2^t × 2^s` grid under this mode.
    pub(crate) fn applies(self, t: usize, s: usize) -> bool {
        match self {
            Self::Off => false,
            Self::Auto => ColumnStats::pays(t, s),
            Self::Force => ColumnStats::fits(t),
        }
    }
}

/// `Z[c][u]` for every column (128 per column, column-major).
pub(crate) struct ColumnStats {
    z: Vec<Gf>,
}

impl ColumnStats {
    /// The pass walks whole 16-element chunks of every column's `2^{t−7}`
    /// packed elements.
    pub(crate) fn fits(t: usize) -> bool {
        t >= LOG_PACKING + 4
    }

    /// Where the pass is cheaper than the two it replaces: the two
    /// contractions cost `2^{s+8}` multiplies, `2^{15−t}` per packed
    /// element (at most four from `t = 13`; `Z` is then at most twice the
    /// packed message), and one task per column wants a few columns per
    /// thread.
    pub(crate) fn pays(t: usize, s: usize) -> bool {
        t >= 13 && s >= 6
    }

    /// The statistics of `rows` (one row of `2^t` bits per column, 64 per
    /// word) at the row point `row_point` (`t` coordinates, low first), and
    /// the row-folded table `M'`.
    pub(crate) fn new(rows: &[Vec<u64>], row_point: &[Gf]) -> (Self, Vec<Gf>) {
        let t = row_point.len();
        assert!(Self::fits(t), "column statistics need t ≥ 11");
        let block = 1usize << (t - LOG_PACKING);
        let eq_packing = build_eq_x_r_vec(&row_point[..LOG_PACKING], &()).expect("seven coordinates");
        let eq_rows = build_eq_x_r_vec(&row_point[LOG_PACKING..], &()).expect("row coordinates");
        debug_assert_eq!(eq_rows.len(), block);
        // The four subset-sum tables of every chunk's 16 row weights, the
        // same for every column: built once.
        let tables: Vec<[[Gf; 16]; 4]> = eq_rows
            .chunks_exact(CHUNK)
            .map(|w| {
                [
                    subset_sums_4([w[0], w[1], w[2], w[3]]),
                    subset_sums_4([w[4], w[5], w[6], w[7]]),
                    subset_sums_4([w[8], w[9], w[10], w[11]]),
                    subset_sums_4([w[12], w[13], w[14], w[15]]),
                ]
            })
            .collect();
        // One task per column: its 128 sums stay in L1 while the column's
        // bits stream past, then go out to `Z` and fold into `M'[c]`.
        let column = |(row, out): (&Vec<u64>, &mut [Gf])| -> Gf {
            debug_assert_eq!(row.len(), 2 * block);
            let mut inner = [Gf::zero(); PACKING];
            for (words, chunk_tables) in row.chunks_exact(2 * CHUNK).zip(&tables) {
                accumulate_chunk(&mut inner, words, chunk_tables);
            }
            out.copy_from_slice(&inner);
            let zero = Gf::zero();
            let mut folded = <Gf as WideMulAcc>::wide_zero(&zero);
            for (value, weight) in inner.iter().zip(&eq_packing) {
                <Gf as WideMulAcc>::wide_add_assign(&mut folded, &<Gf as WideMulAcc>::mul_wide(weight, value));
            }
            <Gf as WideMulAcc>::from_wide(folded)
        };
        let mut z = vec![Gf::zero(); rows.len() * PACKING];
        #[cfg(feature = "parallel")]
        let row_folded: Vec<Gf> = rows
            .par_iter()
            .zip(z.par_chunks_mut(PACKING))
            .with_min_len(4)
            .map(column)
            .collect();
        #[cfg(not(feature = "parallel"))]
        let row_folded: Vec<Gf> = rows.iter().zip(z.chunks_mut(PACKING)).map(column).collect();
        (Self { z }, row_folded)
    }

    /// The ring switch's claims at the point `(r_b, column_point)`:
    /// `ŝ_v[u] = Σ_c eq(column_point, c)·Z[c][u]`.
    pub(crate) fn ring_switch_claims(&self, column_point: &[Gf]) -> Vec<Gf> {
        let columns = self.z.len() / PACKING;
        let eq_columns = if column_point.is_empty() {
            vec![Gf::one()]
        } else {
            build_eq_x_r_vec(column_point, &()).expect("column coordinates")
        };
        assert_eq!(eq_columns.len(), columns, "column point for another grid");
        const COLUMNS_PER_TASK: usize = 64;
        let zero = Gf::zero();
        let partials: Vec<Vec<<Gf as WideMulAcc>::Wide>> =
            cfg_into_iter!(0..columns.div_ceil(COLUMNS_PER_TASK))
                .map(|task| {
                    let mut acc = vec![<Gf as WideMulAcc>::wide_zero(&zero); PACKING];
                    let end = ((task + 1) * COLUMNS_PER_TASK).min(columns);
                    for c in task * COLUMNS_PER_TASK..end {
                        let weight = eq_columns[c];
                        for (slot, value) in acc.iter_mut().zip(&self.z[c * PACKING..(c + 1) * PACKING]) {
                            <Gf as WideMulAcc>::wide_add_assign(
                                slot,
                                &<Gf as WideMulAcc>::mul_wide(&weight, value),
                            );
                        }
                    }
                    acc
                })
                .collect();
        let mut total = vec![<Gf as WideMulAcc>::wide_zero(&zero); PACKING];
        for partial in &partials {
            for (sum, value) in total.iter_mut().zip(partial) {
                <Gf as WideMulAcc>::wide_add_assign(sum, value);
            }
        }
        total.into_iter().map(<Gf as WideMulAcc>::from_wide).collect()
    }
}

/// One 16-element chunk (32 row words) into the column's 128 sums:
/// `inner[u] += Σ_{e : bit u of element e} w_e` through the chunk's four
/// subset-sum tables (flock's `fold_1b_rows_split` inner loop): per byte
/// position two 8×8 bit transposes, then per output bit four lookups and
/// one accumulator update.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
fn accumulate_chunk(inner: &mut [Gf; PACKING], words: &[u64], tables: &[[Gf; 16]; 4]) {
    let [t0, t1, t2, t3] = tables;
    let mut bytes = [[0u8; 16]; CHUNK];
    for (e, slot) in bytes.iter_mut().enumerate() {
        slot[..8].copy_from_slice(&words[2 * e].to_le_bytes());
        slot[8..].copy_from_slice(&words[2 * e + 1].to_le_bytes());
    }
    for byte in 0..16 {
        let lo8 = (0..8).fold(0u64, |acc, k| acc | (u64::from(bytes[k][byte]) << (8 * k)));
        let hi8 = (0..8).fold(0u64, |acc, k| acc | (u64::from(bytes[8 + k][byte]) << (8 * k)));
        let lo = transpose_8x8_bits(lo8).to_le_bytes();
        let hi = transpose_8x8_bits(hi8).to_le_bytes();
        let base = byte * 8;
        for p in 0..8 {
            inner[base + p] += (t0[(lo[p] & 0x0F) as usize] + t1[(lo[p] >> 4) as usize])
                + (t2[(hi[p] & 0x0F) as usize] + t3[(hi[p] >> 4) as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flock_core::pcs::ring_switch::fold_1b_rows_naive;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn random_gf(state: &mut u64) -> Gf {
        Gf::new(xorshift(state), xorshift(state))
    }

    /// `M'` equals the sumcheck's row fold and `ŝ_v` flock's claims over
    /// the materialised suffix tensor, on random bits and points.
    #[test]
    fn statistics_match_the_row_fold_and_the_ring_switch_claims() {
        for (t, s) in [(11usize, 0usize), (11, 3), (12, 5), (13, 2), (14, 4)] {
            let mut state = 0x5eed_c01d ^ (t * 131 + s) as u64;
            let words = (1usize << t) / 64;
            let rows: Vec<Vec<u64>> = (0..1usize << s)
                .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
                .collect();
            let row_point: Vec<Gf> = (0..t).map(|_| random_gf(&mut state)).collect();
            let column_point: Vec<Gf> = (0..s).map(|_| random_gf(&mut state)).collect();

            let (stats, row_folded) = ColumnStats::new(&rows, &row_point);
            assert_eq!(row_folded, super::super::sumcheck::fold_rows_point(&rows, &row_point), "t={t} s={s}");

            let packed: Vec<Gf> = rows
                .iter()
                .flat_map(|row| row.chunks_exact(2).map(|w| Gf::new(w[0], w[1])))
                .collect();
            let suffix: Vec<Gf> = row_point[LOG_PACKING..].iter().chain(&column_point).copied().collect();
            let tensor = build_eq_x_r_vec(&suffix, &()).unwrap();
            assert_eq!(stats.ring_switch_claims(&column_point), fold_1b_rows_naive(&packed, &tensor), "t={t} s={s}");
        }
    }
}
