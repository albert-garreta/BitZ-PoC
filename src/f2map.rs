//! Sparse `F₂`-linear maps between bit-cell grids — the public matrix `M`
//! of the paper's virtualization relation (`s:virtualization`,
//! `\LinBitsDual^{virtual}`): the derived vector `h = M · f` over `F₂` is
//! never committed; claims about `h` are proven against the commitment to
//! `f` by transposing the terminal `𝔽_{2^128}`-linear claim through `Mᵀ`
//! (XOR is addition in characteristic 2, so `⟨z, M·f⟩ = ⟨Mᵀz, f⟩` holds
//! exactly over the commitment field).
//!
//! Cells are addressed flat in the shared tensor order of the PCS: cell
//! `(b, c)` of a `2^{t_w} × 2^s` bit grid (row-bit `b` low, column `c`
//! high) has flat index `(c << t_w) | b` — bit `i` of the flat index is
//! multilinear variable `i`, matching `prove_int_eval_merged_common`'s
//! residual points `(r*, z_c)` and the packed opening's
//! `(r_lo, r_hi) = (point[..7], point[7..])` split.

use blake3::Hasher;

use crate::pcs::IntEvalParams;

/// Number of bit cells (`2^{t+log₂W+s}`) of a shape — the flat domain size.
pub fn cell_count(p: &IntEvalParams) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    1usize << (p.t + log_w + p.s)
}

/// Row-bit width `t + log₂W` of a shape (the low flat-index bits).
pub fn cell_row_bits(p: &IntEvalParams) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    p.t + log_w
}

/// Errors while constructing a sparse `F₂` cell map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum F2CellMapError {
    /// A row lists a source cell outside `[0, cols)`.
    ColOutOfBounds { row: usize, col: u32 },
    /// Source cells within a row must be strictly increasing — this makes
    /// the sparse representation (and therefore the statement digest)
    /// canonical.
    ColsNotStrictlyIncreasing { row: usize },
    /// The declared shape does not fit the flat index type.
    ShapeTooLarge,
}

/// A sparse matrix `M ∈ F₂^{rows × cols}` over flat bit-cell indices, in
/// canonical row-major form: `h_i = ⊕_{j ∈ row(i)} f_j`.
///
/// Rows may be empty (`h_i = 0`). Entries within a row are strictly
/// increasing, so equal maps have equal encodings and equal digests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct F2CellMap {
    rows: usize,
    cols: usize,
    row_offsets: Box<[usize]>,
    entries: Box<[u32]>,
}

impl F2CellMap {
    /// Builds the map from per-row source lists (`h_i = ⊕_j f_{row[i][j]}`).
    pub fn try_from_rows(
        rows: usize,
        cols: usize,
        row_lists: Vec<Vec<u32>>,
    ) -> Result<Self, F2CellMapError> {
        if u32::try_from(cols).is_err() || row_lists.len() != rows {
            return Err(F2CellMapError::ShapeTooLarge);
        }
        for (row, list) in row_lists.iter().enumerate() {
            let mut previous: Option<u32> = None;
            for &col in list {
                if col as usize >= cols {
                    return Err(F2CellMapError::ColOutOfBounds { row, col });
                }
                if let Some(prev) = previous
                    && prev >= col
                {
                    return Err(F2CellMapError::ColsNotStrictlyIncreasing { row });
                }
                previous = Some(col);
            }
        }

        let mut row_offsets = Vec::with_capacity(rows + 1);
        row_offsets.push(0usize);
        let nnz: usize = row_lists.iter().map(Vec::len).sum();
        let mut entries = Vec::with_capacity(nnz);
        for list in row_lists {
            entries.extend(list);
            row_offsets.push(entries.len());
        }
        Ok(Self {
            rows,
            cols,
            row_offsets: row_offsets.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        })
    }

    /// Builds the map directly from flat CSR storage — the memory-honest
    /// constructor for large structured maps (no per-row `Vec` headers).
    ///
    /// `row_offsets` has one start offset per derived cell plus a final
    /// sentinel equal to `entries.len()`; offsets start at zero and are
    /// nondecreasing. Within every row, source cells are strictly
    /// increasing and smaller than `cols`.
    pub fn try_from_csr(
        rows: usize,
        cols: usize,
        row_offsets: Vec<usize>,
        entries: Vec<u32>,
    ) -> Result<Self, F2CellMapError> {
        if u32::try_from(cols).is_err()
            || row_offsets.len() != rows + 1
            || row_offsets.first() != Some(&0)
            || row_offsets.last() != Some(&entries.len())
            || row_offsets.windows(2).any(|b| b[0] > b[1])
        {
            return Err(F2CellMapError::ShapeTooLarge);
        }
        for (row, bounds) in row_offsets.windows(2).enumerate() {
            let mut previous: Option<u32> = None;
            for &col in &entries[bounds[0]..bounds[1]] {
                if col as usize >= cols {
                    return Err(F2CellMapError::ColOutOfBounds { row, col });
                }
                if let Some(prev) = previous
                    && prev >= col
                {
                    return Err(F2CellMapError::ColsNotStrictlyIncreasing { row });
                }
                previous = Some(col);
            }
        }
        Ok(Self {
            rows,
            cols,
            row_offsets: row_offsets.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        })
    }

    /// Number of derived cells (`h`-side).
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Number of source cells (`f`-side).
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// Number of stored nonzeros.
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }

    /// The source cells of derived cell `i`.
    pub fn row(&self, i: usize) -> &[u32] {
        &self.entries[self.row_offsets[i]..self.row_offsets[i + 1]]
    }

    /// Iterator over `(derived cell, source cells)` for the nonempty rows.
    pub fn nonempty_rows(&self) -> impl Iterator<Item = (usize, &[u32])> + '_ {
        self.row_offsets
            .windows(2)
            .enumerate()
            .filter(|(_, b)| b[0] != b[1])
            .map(|(i, b)| (i, &self.entries[b[0]..b[1]]))
    }

    /// Canonical BLAKE3 digest of the map (shape + sparse content). Bound
    /// into the virtual opening's transcript statement.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Hasher::new();
        hash.update(b"f2z/f2-cell-map/v1");
        hash.update(&(self.rows as u64).to_le_bytes());
        hash.update(&(self.cols as u64).to_le_bytes());
        hash.update(&(self.entries.len() as u64).to_le_bytes());
        for offset in self.row_offsets.iter() {
            hash.update(&(*offset as u64).to_le_bytes());
        }
        for entry in self.entries.iter() {
            hash.update(&entry.to_le_bytes());
        }
        *hash.finalize().as_bytes()
    }

    /// Materializes the derived bit rows `h = M·f` in the commit layout of
    /// `p_h` (`rows[c]` = `2^{t+log₂W}` bits, 64 per word), reading `f` in
    /// the commit layout of `p_f`. `f_rows` must have `2^{s_f}` rows of
    /// `2^{t_wf}` bits.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn apply(
        &self,
        p_h: &IntEvalParams,
        p_f: &IntEvalParams,
        f_rows: &[Vec<u64>],
    ) -> Vec<Vec<u64>> {
        assert_eq!(self.rows, cell_count(p_h), "map rows must match p_h cells");
        assert_eq!(self.cols, cell_count(p_f), "map cols must match p_f cells");
        let t_wh = cell_row_bits(p_h);
        let t_wf = cell_row_bits(p_f);
        let h_row_len = 1usize << t_wh;
        let f_mask = (1usize << t_wf) - 1;
        assert_eq!(f_rows.len(), 1usize << p_f.s, "f_rows shape");
        assert!(
            f_rows.iter().all(|r| r.len() == (1usize << t_wf) / 64),
            "f_rows word count"
        );

        let mut h_rows = vec![vec![0u64; h_row_len / 64]; 1usize << p_h.s];
        for (i, sources) in self.nonempty_rows() {
            let mut bit = 0u64;
            for &j in sources {
                let j = j as usize;
                let (c_f, b_f) = (j >> t_wf, j & f_mask);
                bit ^= (f_rows[c_f][b_f >> 6] >> (b_f & 63)) & 1;
            }
            if bit != 0 {
                let (c_h, b_h) = (i >> t_wh, i & (h_row_len - 1));
                h_rows[c_h][b_h >> 6] |= 1u64 << (b_h & 63);
            }
        }
        h_rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(t: usize, s: usize) -> IntEvalParams {
        IntEvalParams { t, s, word_bits: 1 }
    }

    #[test]
    fn construction_validates_bounds_and_order() {
        assert!(F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![]]).is_ok());
        assert_eq!(
            F2CellMap::try_from_rows(1, 4, vec![vec![4]]),
            Err(F2CellMapError::ColOutOfBounds { row: 0, col: 4 })
        );
        assert_eq!(
            F2CellMap::try_from_rows(1, 4, vec![vec![2, 2]]),
            Err(F2CellMapError::ColsNotStrictlyIncreasing { row: 0 })
        );
        assert_eq!(
            F2CellMap::try_from_rows(2, 4, vec![vec![]]),
            Err(F2CellMapError::ShapeTooLarge)
        );
    }

    #[test]
    fn csr_constructor_matches_row_constructor_and_validates() {
        let lists = vec![vec![0u32, 3], vec![], vec![1, 2]];
        let a = F2CellMap::try_from_rows(3, 4, lists).unwrap();
        let b = F2CellMap::try_from_csr(3, 4, vec![0, 2, 2, 4], vec![0, 3, 1, 2]).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.digest(), b.digest());

        assert!(F2CellMap::try_from_csr(3, 4, vec![0, 2, 2], vec![0, 3]).is_err());
        assert!(F2CellMap::try_from_csr(3, 4, vec![0, 2, 1, 2], vec![0, 3]).is_err());
        assert_eq!(
            F2CellMap::try_from_csr(1, 4, vec![0, 2], vec![3, 3]),
            Err(F2CellMapError::ColsNotStrictlyIncreasing { row: 0 })
        );
        assert_eq!(
            F2CellMap::try_from_csr(1, 4, vec![0, 1], vec![4]),
            Err(F2CellMapError::ColOutOfBounds { row: 0, col: 4 })
        );
    }

    #[test]
    fn digest_is_canonical_and_content_sensitive() {
        let a = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![1]]).unwrap();
        let b = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![1]]).unwrap();
        let c = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![2]]).unwrap();
        let d = F2CellMap::try_from_rows(2, 5, vec![vec![0, 3], vec![1]]).unwrap();
        assert_eq!(a.digest(), b.digest());
        assert_ne!(a.digest(), c.digest());
        assert_ne!(a.digest(), d.digest());
    }

    #[test]
    fn apply_matches_naive_bit_xor() {
        // f: shape (7, 2) => 2^9 cells; h: shape (8, 1) => 2^9 cells.
        let p_f = params(7, 2);
        let p_h = params(8, 1);
        let n_f = cell_count(&p_f);
        let n_h = cell_count(&p_h);
        let t_wf = cell_row_bits(&p_f);
        let t_wh = cell_row_bits(&p_h);

        // Deterministic pseudo-random f bits.
        let f_bit = |j: usize| -> u64 {
            let x = (j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5_5A5A;
            (x.rotate_left((j % 63) as u32)) & 1
        };
        let mut f_rows = vec![vec![0u64; (1usize << t_wf) / 64]; 1 << p_f.s];
        for j in 0..n_f {
            if f_bit(j) != 0 {
                let (c, b) = (j >> t_wf, j & ((1 << t_wf) - 1));
                f_rows[c][b >> 6] |= 1u64 << (b & 63);
            }
        }

        // Row i of M: XOR of {i mod n_f, (7i+3) mod n_f} (deduplicated,
        // sorted), with every fifth row empty.
        let mut lists = Vec::with_capacity(n_h);
        for i in 0..n_h {
            if i % 5 == 4 {
                lists.push(vec![]);
                continue;
            }
            let a = (i % n_f) as u32;
            let b = ((7 * i + 3) % n_f) as u32;
            let mut l = if a == b { vec![a] } else { vec![a.min(b), a.max(b)] };
            l.dedup();
            lists.push(l);
        }
        let map = F2CellMap::try_from_rows(n_h, n_f, lists.clone()).unwrap();
        let h_rows = map.apply(&p_h, &p_f, &f_rows);

        for i in 0..n_h {
            let expect = lists[i].iter().fold(0u64, |acc, &j| acc ^ f_bit(j as usize));
            let (c, b) = (i >> t_wh, i & ((1 << t_wh) - 1));
            let got = (h_rows[c][b >> 6] >> (b & 63)) & 1;
            assert_eq!(got, expect, "cell {i}");
        }
    }
}
