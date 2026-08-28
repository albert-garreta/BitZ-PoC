//! Prepared public F₂-linear maps for virtual openings.
//!
//! The sole matrix representation uses the crate-wide canonical CSC topology
//! [`SparseMatrix<bool>`]. Synthesis supplies both the committed source `f`
//! and the derived assignment `h`; this module stores only the public map
//! metadata needed to apply its adjoint during verification.

use blake3::Hasher;
use thiserror::Error;

use crate::{pcs::IntEvalParams, sparse_matrix::SparseMatrix};

/// Number of bit cells (`2^{t+log₂W+s}`) in an integer-evaluation shape.
pub fn cell_count(p: &IntEvalParams) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    1usize << (p.t + log_w + p.s)
}

/// Row-bit width `t + log₂W` of an integer-evaluation shape.
pub fn cell_row_bits(p: &IntEvalParams) -> usize {
    let log_w = p.word_bits.trailing_zeros() as usize;
    p.t + log_w
}

/// Failures while validating public virtual-map metadata.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PreparedVirtualMapError {
    /// A binary sparse map must store only implicit-one coefficients.
    #[error("virtual map contains an explicit false coefficient at row {row}, column {column}")]
    ExplicitFalse { row: usize, column: usize },

    /// The fixed-width canonical digest cannot encode this host shape.
    #[error("virtual map dimensions do not fit the canonical u64 encoding")]
    ShapeTooLarge,
}

/// A validated binary CSC matrix with transcript metadata cached once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedVirtualMap {
    matrix: SparseMatrix<bool>,
    digest: [u8; 32],
    identity: bool,
}

impl PreparedVirtualMap {
    /// Validates a true-only binary CSC matrix and prepares its metadata.
    pub fn new(matrix: SparseMatrix<bool>) -> Result<Self, PreparedVirtualMapError> {
        for (column, entries) in matrix.columns().enumerate() {
            for (row, coefficient) in entries {
                if !*coefficient {
                    return Err(PreparedVirtualMapError::ExplicitFalse { row, column });
                }
            }
        }

        let digest = compute_digest(&matrix)?;
        let identity = compute_identity(&matrix);
        Ok(Self {
            matrix,
            digest,
            identity,
        })
    }

    /// The sole sparse topology backing this prepared map.
    pub const fn matrix(&self) -> &SparseMatrix<bool> {
        &self.matrix
    }

    /// Number of derived cells on the `h` side.
    pub const fn rows(&self) -> usize {
        self.matrix.row_count()
    }

    /// Number of committed source cells on the `f` side.
    pub const fn cols(&self) -> usize {
        self.matrix.column_count()
    }

    /// Number of stored one-coefficients.
    pub const fn nnz(&self) -> usize {
        self.matrix.nnz()
    }

    /// Canonical CSC statement digest.
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Whether this is exactly the square identity matrix.
    pub const fn is_identity(&self) -> bool {
        self.identity
    }
}

fn compute_identity(matrix: &SparseMatrix<bool>) -> bool {
    matrix.row_count() == matrix.column_count()
        && matrix.nnz() == matrix.row_count()
        && matrix
            .columns()
            .enumerate()
            .all(|(column, entries)| matches!(entries.single(), Some((row, true)) if row == column))
}

fn compute_digest(matrix: &SparseMatrix<bool>) -> Result<[u8; 32], PreparedVirtualMapError> {
    let mut hash = Hasher::new();
    hash.update(b"f2z/f2-cell-map/v1");
    for value in [matrix.row_count(), matrix.column_count(), matrix.nnz()] {
        hash.update(
            &u64::try_from(value)
                .map_err(|_| PreparedVirtualMapError::ShapeTooLarge)?
                .to_le_bytes(),
        );
    }
    // Preserve the element-by-element byte stream while avoiding one hasher
    // call per entry on production-sized maps.
    const CHUNK: usize = 1 << 20;
    let mut bytes = Vec::with_capacity(CHUNK * core::mem::size_of::<u64>());
    for values in [matrix.column_offsets(), matrix.row_indices()] {
        for block in values.chunks(CHUNK) {
            bytes.clear();
            for &value in block {
                bytes.extend_from_slice(
                    &u64::try_from(value)
                        .map_err(|_| PreparedVirtualMapError::ShapeTooLarge)?
                        .to_le_bytes(),
                );
            }
            hash.update_rayon(&bytes);
        }
    }
    Ok(*hash.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use crate::sparse_matrix::SparseMatrix;

    use super::*;

    fn prepared(rows: usize, columns: Vec<Vec<(usize, bool)>>) -> PreparedVirtualMap {
        PreparedVirtualMap::new(SparseMatrix::try_from_columns(rows, columns).unwrap()).unwrap()
    }

    #[test]
    fn rejects_explicit_false_coefficients() {
        let matrix = SparseMatrix::try_from_columns(2, vec![vec![(1, false)]]).unwrap();
        assert_eq!(
            PreparedVirtualMap::new(matrix),
            Err(PreparedVirtualMapError::ExplicitFalse { row: 1, column: 0 })
        );
    }

    #[test]
    fn identity_detection_is_exact() {
        assert!(prepared(2, vec![vec![(0, true)], vec![(1, true)]]).is_identity());
        assert!(!prepared(2, vec![vec![(1, true)], vec![(0, true)]]).is_identity());
        assert!(!prepared(2, vec![vec![(0, true)], vec![]]).is_identity());
        assert!(!prepared(2, vec![vec![(0, true)], vec![(1, true)], vec![]]).is_identity());
    }

    #[test]
    fn digest_is_canonical_and_content_sensitive() {
        let a = prepared(
            2,
            vec![vec![(0, true)], vec![(1, true)], vec![], vec![(0, true)]],
        );
        let b = prepared(
            2,
            vec![vec![(0, true)], vec![(1, true)], vec![], vec![(0, true)]],
        );
        let c = prepared(
            2,
            vec![vec![(0, true)], vec![(1, true)], vec![], vec![(1, true)]],
        );
        assert_eq!(
            a.digest(),
            [
                0x7e, 0xd6, 0x0a, 0x91, 0x83, 0x41, 0x5a, 0x44, 0x2b, 0x72, 0x69, 0xcb, 0x21, 0x38,
                0xf8, 0x7a, 0x01, 0xfe, 0x63, 0xa7, 0xe9, 0xff, 0x49, 0x70, 0x7d, 0x1d, 0x1a, 0x2c,
                0x9e, 0xba, 0xda, 0x4e,
            ]
        );
        assert_eq!(a.digest(), b.digest());
        assert_ne!(a.digest(), c.digest());
    }
}
