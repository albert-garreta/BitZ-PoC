//! Prepared public F₂-linear maps for virtual openings.
//!
//! The sole matrix representation uses the crate-wide canonical CSC topology
//! [`SparseMatrix<bool>`]. Synthesis supplies both the committed source `f`
//! and the derived assignment `h`; this module stores only the public map
//! metadata needed to apply its adjoint during verification.

use blake3::Hasher;
use thiserror::Error;

use core::slice;

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

    /// A repeated map needs at least one instance and all derived dimensions
    /// must fit the host index type.
    #[error("virtual map repetition has invalid or overflowing geometry")]
    InvalidRepetition,
}

/// Read-only interface needed by the virtual-opening protocol.
///
/// Arbitrary maps use [`PreparedVirtualMap`]'s canonical CSC storage. Large
/// batches can instead expose an implicit tensor repetition without allocating
/// the fully expanded CSC column-offset array.
pub trait VirtualMap: Sync {
    /// Iterator over the derived rows touched by one source column.
    type ColumnRows<'a>: ExactSizeIterator<Item = usize>
    where
        Self: 'a;

    /// Number of derived cells.
    fn rows(&self) -> usize;

    /// Number of committed source cells.
    fn cols(&self) -> usize;

    /// Number of implicit-one entries.
    fn nnz(&self) -> usize;

    /// Canonical statement digest.
    fn digest(&self) -> [u8; 32];

    /// Whether the complete map is exactly the identity.
    fn is_identity(&self) -> bool;

    /// Derived row indices for one source column, in increasing order.
    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>>;
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

impl VirtualMap for PreparedVirtualMap {
    type ColumnRows<'a> = core::iter::Copied<slice::Iter<'a, usize>>;

    fn rows(&self) -> usize {
        self.rows()
    }

    fn cols(&self) -> usize {
        self.cols()
    }

    fn nnz(&self) -> usize {
        self.nnz()
    }

    fn digest(&self) -> [u8; 32] {
        self.digest()
    }

    fn is_identity(&self) -> bool {
        self.is_identity()
    }

    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
        Some(self.matrix.column(column)?.row_indices().iter().copied())
    }
}

/// Tensor repetition of one local CSC map over independent instances.
///
/// Global bit-cell indices follow the virtual protocol's row-low ordering:
/// `global = local * instances + instance`. Consequently a local edge
/// `local_source -> local_derived` becomes, for every `instance`,
/// `local_source * instances + instance -> local_derived * instances + instance`.
/// Only the local CSC is stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepeatedVirtualMap {
    local: PreparedVirtualMap,
    instances: usize,
    instance_bits: Option<u32>,
    rows: usize,
    cols: usize,
    nnz: usize,
    digest: [u8; 32],
}

impl RepeatedVirtualMap {
    /// Prepares an implicit tensor repetition of `local`.
    pub fn new(
        local: PreparedVirtualMap,
        instances: usize,
    ) -> Result<Self, PreparedVirtualMapError> {
        if instances == 0 {
            return Err(PreparedVirtualMapError::InvalidRepetition);
        }
        let rows = local
            .rows()
            .checked_mul(instances)
            .ok_or(PreparedVirtualMapError::InvalidRepetition)?;
        let cols = local
            .cols()
            .checked_mul(instances)
            .ok_or(PreparedVirtualMapError::InvalidRepetition)?;
        let nnz = local
            .nnz()
            .checked_mul(instances)
            .ok_or(PreparedVirtualMapError::InvalidRepetition)?;

        let mut hash = Hasher::new();
        hash.update(b"f2z/repeated-virtual-map/v1");
        hash.update(&local.digest());
        for value in [instances, rows, cols, nnz] {
            hash.update(
                &u64::try_from(value)
                    .map_err(|_| PreparedVirtualMapError::ShapeTooLarge)?
                    .to_le_bytes(),
            );
        }
        let digest = *hash.finalize().as_bytes();

        Ok(Self {
            local,
            instances,
            instance_bits: instances.is_power_of_two().then(|| instances.ilog2()),
            rows,
            cols,
            nnz,
            digest,
        })
    }

    /// Local canonical CSC map repeated by this view.
    pub const fn local(&self) -> &PreparedVirtualMap {
        &self.local
    }

    /// Number of independent repetitions.
    pub const fn instances(&self) -> usize {
        self.instances
    }
}

/// Row iterator for one column of a [`RepeatedVirtualMap`].
pub struct RepeatedColumnRows<'a> {
    rows: slice::Iter<'a, usize>,
    instance: usize,
    instances: usize,
}

impl Iterator for RepeatedColumnRows<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.rows
            .next()
            .map(|row| row * self.instances + self.instance)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}

impl ExactSizeIterator for RepeatedColumnRows<'_> {}

impl VirtualMap for RepeatedVirtualMap {
    type ColumnRows<'a> = RepeatedColumnRows<'a>;

    fn rows(&self) -> usize {
        self.rows
    }

    fn cols(&self) -> usize {
        self.cols
    }

    fn nnz(&self) -> usize {
        self.nnz
    }

    fn digest(&self) -> [u8; 32] {
        self.digest
    }

    fn is_identity(&self) -> bool {
        self.local.is_identity() && self.local.rows() == self.local.cols()
    }

    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
        if column >= self.cols {
            return None;
        }
        let (local_column, instance) = match self.instance_bits {
            Some(bits) => (column >> bits, column & (self.instances - 1)),
            None => (column / self.instances, column % self.instances),
        };
        let rows = self
            .local
            .matrix()
            .column(local_column)?
            .row_indices()
            .iter();
        Some(RepeatedColumnRows {
            rows,
            instance,
            instances: self.instances,
        })
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

    #[test]
    fn repeated_map_is_an_implicit_tensor_product() {
        let local = prepared(3, vec![vec![(0, true), (2, true)], vec![(1, true)]]);
        let repeated = RepeatedVirtualMap::new(local.clone(), 4).unwrap();
        assert_eq!(repeated.rows(), 12);
        assert_eq!(repeated.cols(), 8);
        assert_eq!(repeated.nnz(), 12);
        assert_eq!(repeated.instances(), 4);
        assert_eq!(
            repeated.column_rows(2).unwrap().collect::<Vec<_>>(),
            vec![2, 10]
        );
        assert_eq!(
            repeated.column_rows(7).unwrap().collect::<Vec<_>>(),
            vec![7]
        );
        assert!(repeated.column_rows(8).is_none());
        assert_ne!(repeated.digest(), local.digest());
        assert_ne!(
            repeated.digest(),
            RepeatedVirtualMap::new(local, 2).unwrap().digest()
        );
    }

    #[test]
    fn repeated_map_rejects_zero_instances_and_preserves_identity() {
        let identity = prepared(2, vec![vec![(0, true)], vec![(1, true)]]);
        assert_eq!(
            RepeatedVirtualMap::new(identity.clone(), 0),
            Err(PreparedVirtualMapError::InvalidRepetition)
        );
        assert!(RepeatedVirtualMap::new(identity, 3).unwrap().is_identity());
    }
}
