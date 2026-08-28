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
#[cfg(feature = "parallel")]
use rayon::prelude::*;

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
    /// The declared shape (or the entry count — offsets are stored as
    /// `u32`, so `nnz ≤ u32::MAX`) does not fit the flat index types.
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
    row_offsets: Box<[u32]>,
    entries: Box<[u32]>,
    digest: [u8; 32],
    identity: bool,
}

impl F2CellMap {
    /// Builds the map from per-row source lists (`h_i = ⊕_j f_{row[i][j]}`).
    pub fn try_from_rows(
        rows: usize,
        cols: usize,
        row_lists: Vec<Vec<u32>>,
    ) -> Result<Self, F2CellMapError> {
        let Some(offset_count) = rows.checked_add(1) else {
            return Err(F2CellMapError::ShapeTooLarge);
        };
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

        let Some(nnz) = row_lists
            .iter()
            .try_fold(0usize, |acc, list| acc.checked_add(list.len()))
        else {
            return Err(F2CellMapError::ShapeTooLarge);
        };
        if u32::try_from(nnz).is_err() {
            return Err(F2CellMapError::ShapeTooLarge);
        }
        let mut row_offsets = Vec::with_capacity(offset_count);
        row_offsets.push(0u32);
        let mut entries = Vec::with_capacity(nnz);
        for list in row_lists {
            entries.extend(list);
            row_offsets.push(entries.len() as u32);
        }
        Ok(Self::sealed(
            rows,
            cols,
            row_offsets.into_boxed_slice(),
            entries.into_boxed_slice(),
        ))
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
        row_offsets: Vec<u32>,
        entries: Vec<u32>,
    ) -> Result<Self, F2CellMapError> {
        let Some(offset_count) = rows.checked_add(1) else {
            return Err(F2CellMapError::ShapeTooLarge);
        };
        if u32::try_from(cols).is_err()
            || u32::try_from(entries.len()).is_err()
            || row_offsets.len() != offset_count
            || row_offsets.first() != Some(&0)
            || row_offsets.last().map(|&o| o as usize) != Some(entries.len())
            || row_offsets.iter().any(|&offset| offset as usize > entries.len())
        {
            return Err(F2CellMapError::ShapeTooLarge);
        }
        // One parallel validation sweep (offset monotonicity plus per-row
        // bounds and strict increase); the canonical first-error scan
        // reruns sequentially only when something is invalid.
        const RANGE: usize = 1 << 16;
        let n_ranges = rows.div_ceil(RANGE).max(1);
        let all_valid = crate::cfg_into_iter!(0..n_ranges).all(|ri| {
            let lo = ri * RANGE;
            let hi = ((ri + 1) * RANGE).min(rows);
            let mut start = row_offsets[lo];
            for r in lo..hi {
                let end = row_offsets[r + 1];
                if end < start {
                    return false;
                }
                let mut previous: Option<u32> = None;
                for &col in &entries[start as usize..end as usize] {
                    if col as usize >= cols {
                        return false;
                    }
                    if let Some(prev) = previous
                        && prev >= col
                    {
                        return false;
                    }
                    previous = Some(col);
                }
                start = end;
            }
            true
        });
        if !all_valid {
            // Reproduce the old scan's error precedence exactly.
            if row_offsets.windows(2).any(|b| b[0] > b[1]) {
                return Err(F2CellMapError::ShapeTooLarge);
            }
            for (row, bounds) in row_offsets.windows(2).enumerate() {
                let mut previous: Option<u32> = None;
                for &col in &entries[bounds[0] as usize..bounds[1] as usize] {
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
            unreachable!("parallel validation rejected but the canonical scan found no error");
        }
        Ok(Self::sealed(
            rows,
            cols,
            row_offsets.into_boxed_slice(),
            entries.into_boxed_slice(),
        ))
    }

    /// Finishes construction: the map is immutable, so its canonical
    /// digest and the identity flag are computed once here and cached.
    fn sealed(rows: usize, cols: usize, row_offsets: Box<[u32]>, entries: Box<[u32]>) -> Self {
        let mut map = Self {
            rows,
            cols,
            row_offsets,
            entries,
            digest: [0; 32],
            identity: false,
        };
        map.digest = map.compute_digest();
        map.identity = map.compute_identity();
        map
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
        &self.entries[self.row_offsets[i] as usize..self.row_offsets[i + 1] as usize]
    }

    /// Whether the map is the identity (`h = f`: square, every row `r`
    /// exactly `[r]`). Cached at construction; the virtual opening's
    /// fast path is gated on it.
    pub const fn is_identity(&self) -> bool {
        self.identity
    }

    fn compute_identity(&self) -> bool {
        self.rows == self.cols
            && self.entries.len() == self.rows
            && self.row_offsets.iter().enumerate().all(|(i, &o)| o as usize == i)
            && self.entries.iter().enumerate().all(|(i, &e)| e as usize == i)
    }

    /// The raw CSR storage `(row_offsets, entries)` — for streaming
    /// scans that walk consecutive rows (one offset load per row instead
    /// of two bounds-checked [`Self::row`] loads).
    pub(crate) fn csr(&self) -> (&[u32], &[u32]) {
        (&self.row_offsets, &self.entries)
    }

    /// Iterator over `(derived cell, source cells)` for the nonempty rows.
    pub fn nonempty_rows(&self) -> impl Iterator<Item = (usize, &[u32])> + '_ {
        self.row_offsets
            .windows(2)
            .enumerate()
            .filter(|(_, b)| b[0] != b[1])
            .map(|(i, b)| (i, &self.entries[b[0] as usize..b[1] as usize]))
    }

    /// Canonical BLAKE3 digest of the map (shape + sparse content). Bound
    /// into the virtual opening's transcript statement.
    ///
    /// The stream is the little-endian concatenation of the header, every
    /// offset (as `u64`), and every entry (as `u32`). Large maps hash it
    /// through bulk staging buffers (BLAKE3 is a byte stream, so this is
    /// bit-identical to per-element updates) with multi-threaded hashing —
    /// a per-element `update` loop costs ~10 ns of call overhead per
    /// element, ~100 ms at a 2^22-cell map.
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    fn compute_digest(&self) -> [u8; 32] {
        let mut hash = Hasher::new();
        hash.update(b"f2z/f2-cell-map/v1");
        hash.update(&(self.rows as u64).to_le_bytes());
        hash.update(&(self.cols as u64).to_le_bytes());
        hash.update(&(self.entries.len() as u64).to_le_bytes());

        // The SAME byte stream as ever (every offset as u64 LE, every
        // entry as u32 LE — BLAKE3 is a byte stream, so chunked staging
        // is bit-identical to one giant buffer), without the
        // former whole-array transient copies.
        const CHUNK: usize = 1 << 21;
        let mut buffer = Vec::with_capacity(CHUNK * 8);
        for block in self.row_offsets.chunks(CHUNK) {
            buffer.clear();
            for &offset in block {
                buffer.extend_from_slice(&(offset as u64).to_le_bytes());
            }
            hash.update_rayon(&buffer);
        }
        #[cfg(target_endian = "little")]
        {
            // u32 LE entries are exactly their in-memory bytes.
            // SAFETY: a &[u32] reinterpreted as its underlying bytes.
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    self.entries.as_ptr().cast::<u8>(),
                    self.entries.len() * 4,
                )
            };
            hash.update_rayon(bytes);
        }
        #[cfg(not(target_endian = "little"))]
        for block in self.entries.chunks(CHUNK * 2) {
            buffer.clear();
            for &entry in block {
                buffer.extend_from_slice(&entry.to_le_bytes());
            }
            hash.update_rayon(&buffer);
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

        // Derived columns own disjoint flat-index ranges (`(c << t_wh) | b`),
        // so each output row is filled independently, in parallel. The
        // offsets stream (one load per cell) and each 64-bit output word
        // accumulates in a register with ONE store — bit-identical to the
        // per-bit RMW form.
        let mut h_rows = vec![vec![0u64; h_row_len / 64]; 1usize << p_h.s];
        crate::cfg_iter_mut!(h_rows)
            .enumerate()
            .for_each(|(c_h, out_row)| {
                let base = c_h << t_wh;
                let offs = &self.row_offsets[base..=base + h_row_len];
                let mut start = offs[0];
                for (wi, out_word) in out_row.iter_mut().enumerate() {
                    let mut acc = 0u64;
                    for k in 0..64usize {
                        let end = offs[(wi << 6) + k + 1];
                        let row_start = start;
                        start = end;
                        if end == row_start {
                            continue;
                        }
                        let mut bit = 0u64;
                        for &j in &self.entries[row_start as usize..end as usize] {
                            let j = j as usize;
                            let (c_f, b_f) = (j >> t_wf, j & f_mask);
                            bit ^= (f_rows[c_f][b_f >> 6] >> (b_f & 63)) & 1;
                        }
                        acc |= bit << k;
                    }
                    *out_word = acc;
                }
            });
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
            F2CellMap::try_from_csr(2, 4, vec![0, 3, 0], vec![]),
            Err(F2CellMapError::ShapeTooLarge)
        );
        assert_eq!(
            F2CellMap::try_from_csr(usize::MAX, 0, vec![], vec![]),
            Err(F2CellMapError::ShapeTooLarge)
        );
        assert_eq!(
            F2CellMap::try_from_rows(usize::MAX, 0, vec![]),
            Err(F2CellMapError::ShapeTooLarge)
        );
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
    fn is_identity_detects_exactly_the_identity() {
        let id = |n: usize| {
            F2CellMap::try_from_rows(n, n, (0..n as u32).map(|i| vec![i]).collect()).unwrap()
        };
        assert!(id(1).is_identity());
        assert!(id(8).is_identity());
        // One empty row.
        let m = F2CellMap::try_from_rows(2, 2, vec![vec![0], vec![]]).unwrap();
        assert!(!m.is_identity());
        // A permutation.
        let m = F2CellMap::try_from_rows(2, 2, vec![vec![1], vec![0]]).unwrap();
        assert!(!m.is_identity());
        // One extra source.
        let m = F2CellMap::try_from_rows(2, 2, vec![vec![0, 1], vec![1]]).unwrap();
        assert!(!m.is_identity());
        // Non-square.
        let m = F2CellMap::try_from_rows(2, 3, vec![vec![0], vec![1]]).unwrap();
        assert!(!m.is_identity());
    }

    #[test]
    fn digest_is_canonical_and_content_sensitive() {
        let a = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![1]]).unwrap();
        let b = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![1]]).unwrap();
        let c = F2CellMap::try_from_rows(2, 4, vec![vec![0, 3], vec![2]]).unwrap();
        let d = F2CellMap::try_from_rows(2, 5, vec![vec![0, 3], vec![1]]).unwrap();
        assert_eq!(
            a.digest(),
            [
                0x9c, 0xc6, 0xbd, 0x42, 0xf5, 0xfb, 0x6f, 0x4c, 0x91, 0x6b, 0xfa, 0x53, 0x4d,
                0x92, 0xe4, 0xad, 0x90, 0x51, 0xb0, 0x39, 0x54, 0x82, 0x93, 0x2b, 0x45, 0xd0,
                0x5e, 0x4b, 0x9c, 0x66, 0xf5, 0x55,
            ]
        );
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
