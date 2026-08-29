//! Sparse matrices with canonical compressed sparse column (CSC) topology.
//!
//! The topology and coefficients are stored in parallel arrays. This keeps
//! binary matrices compact while retaining one generic representation for
//! Spartan constraints and F₂ virtualization maps.

use core::{iter::Zip, slice};

use thiserror::Error;

/// Failures while constructing a canonical sparse matrix.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SparseMatrixError {
    /// A sparse entry refers to a column outside the declared matrix width.
    #[error("column {column} in row {row} is outside a {columns}-column matrix")]
    ColumnOutOfBounds {
        row: usize,
        column: usize,
        columns: usize,
    },

    /// Sparse rows must have a unique canonical order.
    #[error("columns in row {row} are not strictly increasing: {previous}, then {column}")]
    ColumnsNotStrictlyIncreasing {
        row: usize,
        previous: usize,
        column: usize,
    },

    /// A sparse entry refers to a row outside the declared matrix height.
    #[error("row {row} in column {column} is outside a {rows}-row matrix")]
    RowOutOfBounds {
        column: usize,
        row: usize,
        rows: usize,
    },

    /// CSC columns must have a unique canonical order.
    #[error("rows in column {column} are not strictly increasing: {previous}, then {row}")]
    RowsNotStrictlyIncreasing {
        column: usize,
        previous: usize,
        row: usize,
    },

    /// Raw CSC offsets must start at zero, be nondecreasing, and end at the
    /// number of stored entries.
    #[error("invalid CSC column offsets")]
    InvalidCscOffsets,

    /// CSC row-index and coefficient arrays must have equal lengths.
    #[error("CSC row-index and coefficient arrays have different lengths")]
    InvalidCscEntryCount,
}

/// A zero-copy view of one sparse CSC column.
#[derive(Debug)]
pub struct SparseColumn<'a, C> {
    row_indices: &'a [usize],
    coefficients: &'a [C],
}

impl<C> Copy for SparseColumn<'_, C> {}

impl<C> Clone for SparseColumn<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, C> SparseColumn<'a, C> {
    /// Row indices in strictly increasing order.
    pub const fn row_indices(self) -> &'a [usize] {
        self.row_indices
    }

    /// Coefficients aligned one-for-one with [`Self::row_indices`].
    pub const fn coefficients(self) -> &'a [C] {
        self.coefficients
    }

    /// Number of stored entries in the column.
    pub const fn len(self) -> usize {
        self.row_indices.len()
    }

    /// Whether the column has no stored entries.
    pub const fn is_empty(self) -> bool {
        self.row_indices.is_empty()
    }

    /// The sole `(row, coefficient)` entry, if this is a singleton column.
    pub fn single(self) -> Option<(usize, &'a C)> {
        if self.len() == 1 {
            Some((self.row_indices[0], &self.coefficients[0]))
        } else {
            None
        }
    }

    /// Iterates over aligned `(row, coefficient)` entries.
    pub fn iter(self) -> SparseColumnIter<'a, C> {
        SparseColumnIter {
            inner: self.row_indices.iter().zip(self.coefficients.iter()),
        }
    }
}

/// Iterator over one [`SparseColumn`].
pub struct SparseColumnIter<'a, C> {
    inner: Zip<slice::Iter<'a, usize>, slice::Iter<'a, C>>,
}

impl<'a, C> Iterator for SparseColumnIter<'a, C> {
    type Item = (usize, &'a C);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(row, coefficient)| (*row, coefficient))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<C> ExactSizeIterator for SparseColumnIter<'_, C> {}

impl<'a, C> IntoIterator for SparseColumn<'a, C> {
    type Item = (usize, &'a C);
    type IntoIter = SparseColumnIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A sparse matrix with canonical CSC ordering.
///
/// Construction canonicalizes topology (strictly ordered, unique indices),
/// but does not interpret coefficients or reject explicit algebraic zeroes.
/// Protocol-specific prepared statements enforce coefficient invariants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseMatrix<C> {
    row_count: usize,
    column_offsets: Box<[usize]>,
    row_indices: Box<[usize]>,
    coefficients: Box<[C]>,
}

impl<C> SparseMatrix<C> {
    /// Constructs a CSC matrix from row-local `(column, coefficient)` entries.
    pub fn try_from_rows(
        columns: usize,
        rows: Vec<Vec<(usize, C)>>,
    ) -> Result<Self, SparseMatrixError> {
        for (row, entries) in rows.iter().enumerate() {
            let mut previous = None;
            for (column, _) in entries {
                if *column >= columns {
                    return Err(SparseMatrixError::ColumnOutOfBounds {
                        row,
                        column: *column,
                        columns,
                    });
                }
                if let Some(previous) = previous
                    && previous >= *column
                {
                    return Err(SparseMatrixError::ColumnsNotStrictlyIncreasing {
                        row,
                        previous,
                        column: *column,
                    });
                }
                previous = Some(*column);
            }
        }

        let row_count = rows.len();
        let mut column_entries: Vec<Vec<(usize, C)>> = (0..columns).map(|_| Vec::new()).collect();
        for (row, entries) in rows.into_iter().enumerate() {
            for (column, coefficient) in entries {
                column_entries[column].push((row, coefficient));
            }
        }

        Ok(Self::from_validated_columns(row_count, column_entries))
    }

    /// Constructs a CSC matrix from column-local `(row, coefficient)` entries.
    pub fn try_from_columns(
        row_count: usize,
        columns: Vec<Vec<(usize, C)>>,
    ) -> Result<Self, SparseMatrixError> {
        for (column, entries) in columns.iter().enumerate() {
            validate_column(row_count, column, entries.iter().map(|(row, _)| *row))?;
        }

        Ok(Self::from_validated_columns(row_count, columns))
    }

    /// Constructs a matrix directly from tuple-valued flat CSC storage.
    pub fn try_from_csc(
        row_count: usize,
        column_offsets: Vec<usize>,
        entries: Vec<(usize, C)>,
    ) -> Result<Self, SparseMatrixError> {
        let (row_indices, coefficients) = entries.into_iter().unzip();
        Self::try_from_csc_parts(row_count, column_offsets, row_indices, coefficients)
    }

    /// Constructs a matrix directly from compact parallel CSC arrays.
    pub fn try_from_csc_parts(
        row_count: usize,
        column_offsets: Vec<usize>,
        row_indices: Vec<usize>,
        coefficients: Vec<C>,
    ) -> Result<Self, SparseMatrixError> {
        if row_indices.len() != coefficients.len() {
            return Err(SparseMatrixError::InvalidCscEntryCount);
        }
        if column_offsets.first() != Some(&0)
            || column_offsets.last() != Some(&row_indices.len())
            || column_offsets
                .windows(2)
                .any(|bounds| bounds[0] > bounds[1])
        {
            return Err(SparseMatrixError::InvalidCscOffsets);
        }

        for (column, bounds) in column_offsets.windows(2).enumerate() {
            validate_column(
                row_count,
                column,
                row_indices[bounds[0]..bounds[1]].iter().copied(),
            )?;
        }

        Ok(Self {
            row_count,
            column_offsets: column_offsets.into_boxed_slice(),
            row_indices: row_indices.into_boxed_slice(),
            coefficients: coefficients.into_boxed_slice(),
        })
    }

    fn from_validated_columns(row_count: usize, columns: Vec<Vec<(usize, C)>>) -> Self {
        let entry_count = columns.iter().map(Vec::len).sum();
        let mut column_offsets = Vec::with_capacity(columns.len() + 1);
        let mut row_indices = Vec::with_capacity(entry_count);
        let mut coefficients = Vec::with_capacity(entry_count);
        column_offsets.push(0);
        for column in columns {
            for (row, coefficient) in column {
                row_indices.push(row);
                coefficients.push(coefficient);
            }
            column_offsets.push(row_indices.len());
        }

        Self {
            row_count,
            column_offsets: column_offsets.into_boxed_slice(),
            row_indices: row_indices.into_boxed_slice(),
            coefficients: coefficients.into_boxed_slice(),
        }
    }

    /// Returns one logical column.
    pub fn column(&self, column: usize) -> Option<SparseColumn<'_, C>> {
        let start = *self.column_offsets.get(column)?;
        let end = *self.column_offsets.get(column.checked_add(1)?)?;
        Some(SparseColumn {
            row_indices: &self.row_indices[start..end],
            coefficients: &self.coefficients[start..end],
        })
    }

    /// Iterates over every logical column in index order.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = SparseColumn<'_, C>> + '_ {
        self.column_offsets.windows(2).map(|bounds| SparseColumn {
            row_indices: &self.row_indices[bounds[0]..bounds[1]],
            coefficients: &self.coefficients[bounds[0]..bounds[1]],
        })
    }

    /// Number of stored sparse entries.
    pub const fn nnz(&self) -> usize {
        self.row_indices.len()
    }

    /// Number of logical rows.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Number of logical columns.
    pub const fn column_count(&self) -> usize {
        self.column_offsets.len() - 1
    }

    /// Canonical CSC column offsets.
    pub const fn column_offsets(&self) -> &[usize] {
        &self.column_offsets
    }

    /// Canonical CSC row indices.
    pub const fn row_indices(&self) -> &[usize] {
        &self.row_indices
    }

    /// Coefficients aligned with [`Self::row_indices`].
    pub const fn coefficients(&self) -> &[C] {
        &self.coefficients
    }
}

impl SparseMatrix<bool> {
    /// Constructs a true-only binary matrix from compact CSC topology.
    pub fn try_from_binary_csc(
        row_count: usize,
        column_offsets: Vec<usize>,
        row_indices: Vec<usize>,
    ) -> Result<Self, SparseMatrixError> {
        let coefficients = vec![true; row_indices.len()];
        Self::try_from_csc_parts(row_count, column_offsets, row_indices, coefficients)
    }
}

fn validate_column(
    row_count: usize,
    column: usize,
    rows: impl IntoIterator<Item = usize>,
) -> Result<(), SparseMatrixError> {
    let mut previous = None;
    for row in rows {
        if row >= row_count {
            return Err(SparseMatrixError::RowOutOfBounds {
                column,
                row,
                rows: row_count,
            });
        }
        if let Some(previous) = previous
            && previous >= row
        {
            return Err(SparseMatrixError::RowsNotStrictlyIncreasing {
                column,
                previous,
                row,
            });
        }
        previous = Some(row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_produce_the_same_compact_csc() {
        let rows = vec![vec![(0, 2u8), (3, 3)], vec![], vec![(0, 5), (2, 7)]];
        let from_rows = SparseMatrix::try_from_rows(5, rows).unwrap();
        let from_columns = SparseMatrix::try_from_columns(
            3,
            vec![
                vec![(0, 2), (2, 5)],
                vec![],
                vec![(2, 7)],
                vec![(0, 3)],
                vec![],
            ],
        )
        .unwrap();
        let from_parts = SparseMatrix::try_from_csc_parts(
            3,
            vec![0, 2, 2, 3, 4, 4],
            vec![0, 2, 2, 0],
            vec![2, 5, 7, 3],
        )
        .unwrap();

        assert_eq!(from_rows, from_columns);
        assert_eq!(from_rows, from_parts);
        let first = from_rows.column(0).unwrap();
        assert_eq!(first.row_indices(), &[0, 2]);
        assert_eq!(first.coefficients(), &[2, 5]);
        assert_eq!(from_rows.column(4).unwrap().len(), 0);
    }

    #[test]
    fn malformed_csc_is_rejected() {
        assert_eq!(
            SparseMatrix::<u8>::try_from_csc_parts(2, vec![], vec![], vec![]),
            Err(SparseMatrixError::InvalidCscOffsets)
        );
        assert_eq!(
            SparseMatrix::try_from_csc_parts(2, vec![0, 1], vec![0], Vec::<u8>::new()),
            Err(SparseMatrixError::InvalidCscEntryCount)
        );
        assert!(matches!(
            SparseMatrix::try_from_binary_csc(2, vec![0, 2], vec![1, 0]),
            Err(SparseMatrixError::RowsNotStrictlyIncreasing { .. })
        ));
    }

    #[test]
    fn empty_dimensions_and_empty_columns_are_retained() {
        let empty = SparseMatrix::<u8>::try_from_csc_parts(0, vec![0], vec![], vec![]).unwrap();
        assert_eq!(empty.row_count(), 0);
        assert_eq!(empty.column_count(), 0);
        assert_eq!(empty.nnz(), 0);

        let columns = SparseMatrix::try_from_binary_csc(0, vec![0, 0, 0], vec![]).unwrap();
        assert_eq!(columns.row_count(), 0);
        assert_eq!(columns.column_count(), 2);
        assert!(columns.columns().all(SparseColumn::is_empty));
    }
}
