//! Validated R1CS matrices and multilinear-table helpers for Spartan.
//!
//! This module starts at the field-valued R1CS boundary. Constraint generation
//! and the F2-to-integer witness map are intentionally out of scope: callers
//! supply the three matrices `A`, `B`, and `C`, their row products, and the
//! complete assignment consumed by those matrices.

use blake3::Hasher;
use thiserror::Error;

use crate::poly::mle::DenseMultilinearExtension;

use super::{SpartanField, SpartanFieldError, sumcheck::R1csProductMles};

/// Failures while constructing or evaluating a Spartan matrix statement.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SpartanMatrixError {
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

    /// `A`, `B`, and `C` do not describe one common R1CS shape.
    #[error("A, B, and C must have identical, nonempty dimensions")]
    InvalidR1csShape,

    /// A table length cannot be rounded up to a Boolean-hypercube domain.
    #[error("the requested Boolean-hypercube domain is too large")]
    DomainTooLarge,

    /// A supplied value belongs to a different runtime field configuration.
    #[error("a field element does not use the prepared field configuration")]
    FieldConfigurationMismatch,

    /// The selected runtime field is unsafe for Spartan.
    #[error(transparent)]
    InvalidFieldConfiguration(#[from] SpartanFieldError),

    /// Explicit zero entries give a sparse matrix more than one encoding.
    #[error("matrix {matrix} contains an explicit zero at row {row}, column {column}")]
    ExplicitZeroCoefficient {
        matrix: &'static str,
        row: usize,
        column: usize,
    },

    /// One of `Az`, `Bz`, or `Cz` has the wrong logical row count.
    #[error("product table has length {actual}, expected {expected}")]
    InvalidProductLength { expected: usize, actual: usize },

    /// The assignment does not have the declared logical column count.
    #[error("assignment has length {actual}, expected {expected}")]
    InvalidAssignmentLength { expected: usize, actual: usize },

    /// R1CS column zero is the constant-one assignment entry.
    #[error("the assignment must begin with the constant one")]
    InvalidAssignmentConstant,

    /// A row-domain evaluation point has the wrong width.
    #[error("row point has width {actual}, expected {expected}")]
    InvalidRowPointLength { expected: usize, actual: usize },

    /// A column-domain evaluation point has the wrong width.
    #[error("column point has width {actual}, expected {expected}")]
    InvalidColumnPointLength { expected: usize, actual: usize },

    /// A dense MLE's metadata and complete table disagree.
    #[error("invalid dense multilinear-extension table")]
    InvalidMleOperation,
}

/// Why a full MLE table does not discharge a scaled evaluation claim.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum MleClaimError {
    /// The supplied table, point, or field configuration is malformed.
    #[error("invalid multilinear polynomial: {0}")]
    InvalidPolynomial(#[from] SpartanMatrixError),

    /// The table evaluation does not equal the claimed scaled value.
    #[error("the multilinear evaluation does not match the claim")]
    InvalidEvaluation,
}

/// A sparse matrix in compressed sparse column (CSC) form.
///
/// Each column occupies one contiguous range in `entries`. Within a column,
/// row indices are strictly increasing, so the representation of a sparse
/// matrix is canonical. Logical rows and columns are retained even when they
/// are empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseMatrix<F> {
    row_count: usize,
    column_offsets: Box<[usize]>,
    entries: Box<[(usize, F)]>,
}

impl<F> SparseMatrix<F> {
    /// Constructs a CSC matrix from row-local `(column, coefficient)` entries.
    ///
    /// Every row must use strictly increasing in-bounds column indices. This
    /// makes the sparse representation unique and, consequently, makes the
    /// prepared statement digest independent of map iteration order.
    pub fn try_from_rows(
        columns: usize,
        rows: Vec<Vec<(usize, F)>>,
    ) -> Result<Self, SpartanMatrixError> {
        for (row, entries) in rows.iter().enumerate() {
            let mut previous = None;
            for (column, _) in entries {
                if *column >= columns {
                    return Err(SpartanMatrixError::ColumnOutOfBounds {
                        row,
                        column: *column,
                        columns,
                    });
                }
                if let Some(previous) = previous
                    && previous >= *column
                {
                    return Err(SpartanMatrixError::ColumnsNotStrictlyIncreasing {
                        row,
                        previous,
                        column: *column,
                    });
                }
                previous = Some(*column);
            }
        }

        let row_count = rows.len();
        let mut column_entries: Vec<Vec<(usize, F)>> = (0..columns).map(|_| Vec::new()).collect();
        for (row, entries) in rows.into_iter().enumerate() {
            for (column, coefficient) in entries {
                column_entries[column].push((row, coefficient));
            }
        }

        Ok(Self::from_validated_columns(row_count, column_entries))
    }

    /// Constructs a CSC matrix from column-local `(row, coefficient)` entries.
    ///
    /// Every column must use strictly increasing in-bounds row indices. Empty
    /// trailing rows and columns remain part of the logical matrix shape.
    pub fn try_from_columns(
        row_count: usize,
        columns: Vec<Vec<(usize, F)>>,
    ) -> Result<Self, SpartanMatrixError> {
        for (column, entries) in columns.iter().enumerate() {
            let mut previous = None;
            for (row, _) in entries {
                if *row >= row_count {
                    return Err(SpartanMatrixError::RowOutOfBounds {
                        column,
                        row: *row,
                        rows: row_count,
                    });
                }
                if let Some(previous) = previous
                    && previous >= *row
                {
                    return Err(SpartanMatrixError::RowsNotStrictlyIncreasing {
                        column,
                        previous,
                        row: *row,
                    });
                }
                previous = Some(*row);
            }
        }

        Ok(Self::from_validated_columns(row_count, columns))
    }

    fn from_validated_columns(row_count: usize, columns: Vec<Vec<(usize, F)>>) -> Self {
        let mut column_offsets = Vec::with_capacity(columns.len() + 1);
        column_offsets.push(0);

        let entry_count = columns.iter().map(Vec::len).sum();
        let mut entries = Vec::with_capacity(entry_count);
        for column in columns {
            entries.extend(column);
            column_offsets.push(entries.len());
        }

        Self {
            row_count,
            column_offsets: column_offsets.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        }
    }

    /// Entries in `column`, as `(row, coefficient)` pairs.
    ///
    /// The returned slice is a zero-copy view into the CSC storage.
    pub fn column(&self, column: usize) -> Option<&[(usize, F)]> {
        let start = *self.column_offsets.get(column)?;
        let end = *self.column_offsets.get(column.checked_add(1)?)?;
        Some(&self.entries[start..end])
    }

    /// Zero-copy column slices in logical column-index order.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = &[(usize, F)]> + '_ {
        self.column_offsets
            .windows(2)
            .map(|bounds| &self.entries[bounds[0]..bounds[1]])
    }

    /// Number of stored sparse entries.
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }

    /// Number of logical rows before Boolean-domain padding.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Number of logical columns, including constant column zero.
    pub const fn column_count(&self) -> usize {
        self.column_offsets.len() - 1
    }
}

/// The three field-valued matrices defining an R1CS relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintMatrices<F> {
    a: SparseMatrix<F>,
    b: SparseMatrix<F>,
    c: SparseMatrix<F>,
}

impl<F> ConstraintMatrices<F> {
    /// Validates that `A`, `B`, and `C` share one nonempty shape.
    pub fn new(
        a: SparseMatrix<F>,
        b: SparseMatrix<F>,
        c: SparseMatrix<F>,
    ) -> Result<Self, SpartanMatrixError> {
        let rows = a.row_count();
        let columns = a.column_count();
        if rows == 0
            || columns == 0
            || b.row_count() != rows
            || c.row_count() != rows
            || b.column_count() != columns
            || c.column_count() != columns
        {
            return Err(SpartanMatrixError::InvalidR1csShape);
        }

        Ok(Self { a, b, c })
    }

    /// Left R1CS matrix.
    pub const fn a(&self) -> &SparseMatrix<F> {
        &self.a
    }

    /// Right R1CS matrix.
    pub const fn b(&self) -> &SparseMatrix<F> {
        &self.b
    }

    /// Output R1CS matrix.
    pub const fn c(&self) -> &SparseMatrix<F> {
        &self.c
    }

    /// Shared logical row count.
    pub fn row_count(&self) -> usize {
        self.a.row_count()
    }

    /// Shared logical column count.
    pub const fn column_count(&self) -> usize {
        self.a.column_count()
    }
}

/// A validated, digest-bound R1CS matrix statement prepared for reuse.
#[derive(Clone, Debug)]
pub struct PreparedConstraintMatrices<F>
where
    F: SpartanField,
{
    matrices: ConstraintMatrices<F>,
    field_config: F::Config,
    field_modulus_encoding: Vec<u8>,
    digest: [u8; 32],
    num_row_vars: usize,
    num_column_vars: usize,
}

impl<F> PreparedConstraintMatrices<F>
where
    F: SpartanField,
{
    /// Validates the coefficient field, computes padded domain widths, and
    /// commits to the complete public statement with BLAKE3.
    pub fn new(
        matrices: ConstraintMatrices<F>,
        field_config: &F::Config,
    ) -> Result<Self, SpartanMatrixError> {
        F::validate_config(field_config)?;
        let num_row_vars = padded_num_vars(matrices.row_count())?;
        let num_column_vars = padded_num_vars(matrices.column_count())?;
        let field_modulus_encoding = F::canonical_modulus_encoding(field_config);
        let digest = constraint_matrix_digest(&matrices, &field_modulus_encoding)?;

        Ok(Self {
            matrices,
            field_config: field_config.clone(),
            field_modulus_encoding,
            digest,
            num_row_vars,
            num_column_vars,
        })
    }

    /// Validated `A`, `B`, and `C` matrices.
    pub const fn matrices(&self) -> &ConstraintMatrices<F> {
        &self.matrices
    }

    /// Runtime field configuration used by every prepared operation.
    pub const fn config(&self) -> &F::Config {
        &self.field_config
    }

    /// Canonical encoding of the runtime modulus bound into [`Self::digest`].
    pub fn field_modulus_encoding(&self) -> &[u8] {
        &self.field_modulus_encoding
    }

    /// BLAKE3 digest of the field identity, shape, sparse topology, and values.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Number of variables in the padded row domain.
    pub const fn num_row_vars(&self) -> usize {
        self.num_row_vars
    }

    /// Number of variables in the padded column domain.
    pub const fn num_column_vars(&self) -> usize {
        self.num_column_vars
    }

    /// Constructs the dense column MLE
    ///
    /// `D(j) = sum_i eq(i, row_point) (A[i,j] + rho B[i,j] + rho^2 C[i,j])`.
    ///
    /// The resulting table is indexed in little-endian column-coordinate
    /// order and padded with trailing zeros.
    pub fn bind_and_batch(
        &self,
        row_point: &[F],
        rho: &F,
    ) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError> {
        if row_point.len() != self.num_row_vars {
            return Err(SpartanMatrixError::InvalidRowPointLength {
                expected: self.num_row_vars,
                actual: row_point.len(),
            });
        }
        validate_elements_field(row_point, &self.field_modulus_encoding)?;
        validate_element_field(rho, &self.field_modulus_encoding)?;

        let row_weights = eq_table(row_point, &self.field_config)?;
        let zero = F::zero_with_cfg(&self.field_config);
        let rho_squared = mul(rho, rho);
        let mut evaluations = Vec::with_capacity(domain_size(self.num_column_vars)?);

        for ((a_column, b_column), c_column) in self
            .matrices
            .a()
            .columns()
            .zip(self.matrices.b().columns())
            .zip(self.matrices.c().columns())
        {
            let mut evaluation = sparse_column_dot(a_column, &row_weights, &zero);
            if !b_column.is_empty() {
                let b_evaluation = sparse_column_dot(b_column, &row_weights, &zero);
                evaluation += &mul(rho, &b_evaluation);
            }
            if !c_column.is_empty() {
                let c_evaluation = sparse_column_dot(c_column, &row_weights, &zero);
                evaluation += &mul(&rho_squared, &c_evaluation);
            }
            evaluations.push(evaluation);
        }
        evaluations.resize(domain_size(self.num_column_vars)?, zero);

        Ok(DenseMultilinearExtension {
            evaluations,
            num_vars: self.num_column_vars,
        })
    }

    /// Directly evaluates
    ///
    /// `A(row_point, column_point) + rho B(row_point, column_point)
    /// + rho^2 C(row_point, column_point)`.
    ///
    /// This deliberately does not construct or evaluate the dense table from
    /// [`Self::bind_and_batch`], keeping the verifier path independent of the
    /// prover's materialization kernel.
    pub fn evaluate_batched(
        &self,
        row_point: &[F],
        rho: &F,
        column_point: &[F],
    ) -> Result<F, SpartanMatrixError> {
        if row_point.len() != self.num_row_vars {
            return Err(SpartanMatrixError::InvalidRowPointLength {
                expected: self.num_row_vars,
                actual: row_point.len(),
            });
        }
        if column_point.len() != self.num_column_vars {
            return Err(SpartanMatrixError::InvalidColumnPointLength {
                expected: self.num_column_vars,
                actual: column_point.len(),
            });
        }
        validate_elements_field(row_point, &self.field_modulus_encoding)?;
        validate_elements_field(column_point, &self.field_modulus_encoding)?;
        validate_element_field(rho, &self.field_modulus_encoding)?;

        let row_weights = eq_table(row_point, &self.field_config)?;
        let column_weights = eq_table(column_point, &self.field_config)?;
        let zero = F::zero_with_cfg(&self.field_config);
        let rho_squared = mul(rho, rho);
        let mut evaluation =
            evaluate_sparse_matrix(self.matrices.a(), &row_weights, &column_weights, &zero);
        let b_evaluation =
            evaluate_sparse_matrix(self.matrices.b(), &row_weights, &column_weights, &zero);
        let c_evaluation =
            evaluate_sparse_matrix(self.matrices.c(), &row_weights, &column_weights, &zero);
        evaluation += &mul(rho, &b_evaluation);
        evaluation += &mul(&rho_squared, &c_evaluation);

        Ok(evaluation)
    }
}

/// Pads a complete field-valued assignment to the prepared column domain.
///
/// `assignment[0]` must be the R1CS constant one. Values retain their logical
/// order and all padding is appended, so the table uses little-endian index
/// order without a permutation.
pub fn build_assignment_mle<F>(
    assignment: &[F],
    expected_columns: usize,
    field_config: &F::Config,
) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError>
where
    F: SpartanField,
{
    if assignment.len() != expected_columns {
        return Err(SpartanMatrixError::InvalidAssignmentLength {
            expected: expected_columns,
            actual: assignment.len(),
        });
    }
    let modulus_encoding = F::canonical_modulus_encoding(field_config);
    validate_elements_field(assignment, &modulus_encoding)?;
    let one = F::one_with_cfg(field_config);
    if assignment.first() != Some(&one) {
        return Err(SpartanMatrixError::InvalidAssignmentConstant);
    }

    padded_mle(assignment, field_config)
}

/// Convenience conversion for a Boolean assignment beginning with `true`.
pub fn build_boolean_assignment_mle<F>(
    assignment: &[bool],
    expected_columns: usize,
    field_config: &F::Config,
) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError>
where
    F: SpartanField,
{
    if assignment.len() != expected_columns {
        return Err(SpartanMatrixError::InvalidAssignmentLength {
            expected: expected_columns,
            actual: assignment.len(),
        });
    }

    let zero = F::zero_with_cfg(field_config);
    let one = F::one_with_cfg(field_config);
    let field_assignment: Vec<_> = assignment
        .iter()
        .map(|bit| if *bit { one.clone() } else { zero.clone() })
        .collect();
    build_assignment_mle(&field_assignment, expected_columns, field_config)
}

/// Checks and pads the three R1CS product tables to one row domain.
pub fn build_product_mles<F>(
    az: &[F],
    bz: &[F],
    cz: &[F],
    expected_rows: usize,
    field_config: &F::Config,
) -> Result<R1csProductMles<F>, SpartanMatrixError>
where
    F: SpartanField,
{
    for actual in [az.len(), bz.len(), cz.len()] {
        if actual != expected_rows {
            return Err(SpartanMatrixError::InvalidProductLength {
                expected: expected_rows,
                actual,
            });
        }
    }

    let modulus_encoding = F::canonical_modulus_encoding(field_config);
    validate_elements_field(az, &modulus_encoding)?;
    validate_elements_field(bz, &modulus_encoding)?;
    validate_elements_field(cz, &modulus_encoding)?;

    Ok(R1csProductMles {
        az: padded_mle(az, field_config)?,
        bz: padded_mle(bz, field_config)?,
        cz: padded_mle(cz, field_config)?,
    })
}

/// Evaluates the multilinear equality polynomial
///
/// `eq(left, right) = product_i(left_i right_i + (1-left_i)(1-right_i))`.
pub fn eq_eval<F>(
    left: &[F],
    right: &[F],
    field_config: &F::Config,
) -> Result<F, SpartanMatrixError>
where
    F: SpartanField,
{
    if left.len() != right.len() {
        return Err(SpartanMatrixError::InvalidMleOperation);
    }
    let modulus_encoding = F::canonical_modulus_encoding(field_config);
    validate_elements_field(left, &modulus_encoding)?;
    validate_elements_field(right, &modulus_encoding)?;

    let one = F::one_with_cfg(field_config);
    let mut value = one.clone();
    for (left_i, right_i) in left.iter().zip(right) {
        // (1 - left_i) + right_i (2 left_i - 1)
        let mut twice_left_minus_one = left_i.clone();
        twice_left_minus_one += left_i;
        twice_left_minus_one -= &one;
        twice_left_minus_one *= right_i;

        let mut coordinate = one.clone();
        coordinate -= left_i;
        coordinate += &twice_left_minus_one;
        value *= &coordinate;
    }
    Ok(value)
}

/// Builds `eq(boolean_index, point)` in exact little-endian index order.
///
/// Coordinate `i` corresponds to bit `i` of the table index. The empty point
/// therefore has the one-entry table `[1]`.
pub fn eq_table<F>(point: &[F], field_config: &F::Config) -> Result<Vec<F>, SpartanMatrixError>
where
    F: SpartanField,
{
    let modulus_encoding = F::canonical_modulus_encoding(field_config);
    validate_elements_field(point, &modulus_encoding)?;
    let table_len = domain_size(point.len())?;
    let zero = F::zero_with_cfg(field_config);
    let mut table = vec![zero; table_len];
    table[0] = F::one_with_cfg(field_config);

    for (coordinate, challenge) in point.iter().enumerate() {
        let half = domain_size(coordinate)?;
        let (zero_children, one_children) = table[..2 * half].split_at_mut(half);
        for (zero_child, one_child) in zero_children.iter_mut().zip(one_children) {
            let parent = zero_child.clone();
            *one_child = mul(&parent, challenge);
            *zero_child = sub(&parent, one_child);
        }
    }

    Ok(table)
}

/// Materializes the low- and high-coordinate equality factors used by the
/// factored outer sumcheck.
///
/// The low, earlier coordinates are always returned first. Thus the tensor
/// product of the two tables agrees with [`eq_table`]'s little-endian order.
pub fn make_equality_factors<F>(
    point: &[F],
    field_config: &F::Config,
) -> Result<(DenseMultilinearExtension<F>, DenseMultilinearExtension<F>), SpartanMatrixError>
where
    F: SpartanField,
{
    let split = point.len() / 2;
    let (low, high) = point.split_at(split);
    let low = DenseMultilinearExtension {
        evaluations: eq_table(low, field_config)?,
        num_vars: low.len(),
    };
    let high = DenseMultilinearExtension {
        evaluations: eq_table(high, field_config)?,
        num_vars: high.len(),
    };
    Ok((low, high))
}

/// A scaled MLE evaluation claim `scale * polynomial(point) = value`.
///
/// The scale is retained instead of divided away because it may be zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaledMleEvaluationClaim<F> {
    point: Box<[F]>,
    scale: F,
    value: F,
}

impl<F> ScaledMleEvaluationClaim<F>
where
    F: SpartanField,
{
    /// Constructs a scaled claim. Structural checks occur during verification.
    pub fn new(point: Box<[F]>, scale: F, value: F) -> Self {
        Self {
            point,
            scale,
            value,
        }
    }

    /// Claimed evaluation point, in low-coordinate-first order.
    pub fn point(&self) -> &[F] {
        &self.point
    }

    /// Multiplicative scale on the polynomial evaluation.
    pub const fn scale(&self) -> &F {
        &self.scale
    }

    /// Claimed scaled value.
    pub const fn value(&self) -> &F {
        &self.value
    }

    /// Directly discharges the claim against a complete evaluation table.
    ///
    /// This is the witness-aware path used until the assignment opening is
    /// connected to the F2Z PCS.
    pub fn nonsuccinct_verify(
        &self,
        polynomial: &DenseMultilinearExtension<F>,
        field_config: &F::Config,
    ) -> Result<(), MleClaimError> {
        let modulus_encoding = F::canonical_modulus_encoding(field_config);
        validate_element_field(&self.scale, &modulus_encoding)?;
        validate_element_field(&self.value, &modulus_encoding)?;
        let evaluation = evaluate_mle(polynomial, &self.point, field_config)?;
        if mul(&self.scale, &evaluation) != self.value {
            return Err(MleClaimError::InvalidEvaluation);
        }
        Ok(())
    }
}

fn padded_mle<F>(
    values: &[F],
    field_config: &F::Config,
) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError>
where
    F: SpartanField,
{
    let num_vars = padded_num_vars(values.len())?;
    let mut evaluations = values.to_vec();
    evaluations.resize(domain_size(num_vars)?, F::zero_with_cfg(field_config));
    Ok(DenseMultilinearExtension {
        evaluations,
        num_vars,
    })
}

fn evaluate_mle<F>(
    polynomial: &DenseMultilinearExtension<F>,
    point: &[F],
    field_config: &F::Config,
) -> Result<F, SpartanMatrixError>
where
    F: SpartanField,
{
    if point.len() != polynomial.num_vars {
        return Err(SpartanMatrixError::InvalidColumnPointLength {
            expected: polynomial.num_vars,
            actual: point.len(),
        });
    }
    let expected_evaluations = domain_size(polynomial.num_vars)?;
    if polynomial.evaluations.len() != expected_evaluations {
        return Err(SpartanMatrixError::InvalidMleOperation);
    }

    let modulus_encoding = F::canonical_modulus_encoding(field_config);
    validate_elements_field(point, &modulus_encoding)?;
    validate_elements_field(&polynomial.evaluations, &modulus_encoding)?;

    let mut evaluations = polynomial.evaluations.clone();
    let mut active_len = evaluations.len();
    for challenge in point {
        let next_len = active_len / 2;
        for index in 0..next_len {
            let left = evaluations[2 * index].clone();
            let difference = sub(&evaluations[2 * index + 1], &left);
            evaluations[index] = add(&left, &mul(challenge, &difference));
        }
        active_len = next_len;
    }

    evaluations
        .into_iter()
        .next()
        .ok_or(SpartanMatrixError::InvalidMleOperation)
}

fn sparse_column_dot<F>(column: &[(usize, F)], row_weights: &[F], zero: &F) -> F
where
    F: SpartanField,
{
    let mut evaluation = zero.clone();
    for (row, coefficient) in column {
        evaluation += &mul(&row_weights[*row], coefficient);
    }
    evaluation
}

fn evaluate_sparse_matrix<F>(
    matrix: &SparseMatrix<F>,
    row_weights: &[F],
    column_weights: &[F],
    zero: &F,
) -> F
where
    F: SpartanField,
{
    let mut evaluation = zero.clone();
    for (column, entries) in matrix.columns().enumerate() {
        if !entries.is_empty() {
            let column_evaluation = sparse_column_dot(entries, row_weights, zero);
            evaluation += &mul(&column_weights[column], &column_evaluation);
        }
    }
    evaluation
}

struct CanonicalRows<'a, F> {
    row_offsets: Box<[usize]>,
    entries: Box<[(usize, &'a F)]>,
}

impl<'a, F> CanonicalRows<'a, F> {
    fn new(matrix: &'a SparseMatrix<F>) -> Self {
        let mut row_offsets = vec![0; matrix.row_count() + 1];
        for column in matrix.columns() {
            for (row, _) in column {
                row_offsets[*row + 1] += 1;
            }
        }

        let mut entry_count = 0;
        for row_end in row_offsets.iter_mut().skip(1) {
            entry_count += *row_end;
            *row_end = entry_count;
        }
        debug_assert_eq!(entry_count, matrix.nnz());

        let mut next_entry = row_offsets[..matrix.row_count()].to_vec();
        // Seed the flat output with a valid reference so the counting-sort
        // scatter needs neither `F: Clone` nor unsafe uninitialized storage.
        // The final cursor check guarantees that every slot was overwritten.
        let mut entries = matrix
            .entries
            .first()
            .map_or_else(Vec::new, |(_, coefficient)| {
                vec![(0, coefficient); matrix.nnz()]
            });
        for (column, column_entries) in matrix.columns().enumerate() {
            for (row, coefficient) in column_entries {
                let position = next_entry[*row];
                entries[position] = (column, coefficient);
                next_entry[*row] += 1;
            }
        }
        debug_assert_eq!(next_entry.as_slice(), &row_offsets[1..]);

        Self {
            row_offsets: row_offsets.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        }
    }

    fn rows(&self) -> impl ExactSizeIterator<Item = &[(usize, &'a F)]> + '_ {
        self.row_offsets
            .windows(2)
            .map(|bounds| &self.entries[bounds[0]..bounds[1]])
    }
}

fn validate_matrix_field<F>(
    matrix_name: &'static str,
    rows: &CanonicalRows<'_, F>,
    modulus_encoding: &[u8],
) -> Result<(), SpartanMatrixError>
where
    F: SpartanField,
{
    for (row_index, row) in rows.rows().enumerate() {
        for (column, coefficient) in row {
            validate_element_field(*coefficient, modulus_encoding)?;
            if F::is_zero(*coefficient) {
                return Err(SpartanMatrixError::ExplicitZeroCoefficient {
                    matrix: matrix_name,
                    row: row_index,
                    column: *column,
                });
            }
        }
    }
    Ok(())
}

fn validate_elements_field<F>(
    values: &[F],
    modulus_encoding: &[u8],
) -> Result<(), SpartanMatrixError>
where
    F: SpartanField,
{
    for value in values {
        validate_element_field(value, modulus_encoding)?;
    }
    Ok(())
}

fn validate_element_field<F>(value: &F, modulus_encoding: &[u8]) -> Result<(), SpartanMatrixError>
where
    F: SpartanField,
{
    if F::canonical_modulus_encoding(value.cfg()) != modulus_encoding {
        return Err(SpartanMatrixError::FieldConfigurationMismatch);
    }
    value.validate_element()?;
    Ok(())
}

fn constraint_matrix_digest<F>(
    matrices: &ConstraintMatrices<F>,
    field_modulus_encoding: &[u8],
) -> Result<[u8; 32], SpartanMatrixError>
where
    F: SpartanField,
{
    let mut hash = Hasher::new();
    hash.update(b"f2z/spartan/constraint-matrices/v2");
    hash_bytes(&mut hash, field_modulus_encoding)?;
    hash_usize(&mut hash, matrices.row_count())?;
    hash_usize(&mut hash, matrices.column_count())?;

    for (matrix_name, label, matrix) in [
        ("A", b'A', matrices.a()),
        ("B", b'B', matrices.b()),
        ("C", b'C', matrices.c()),
    ] {
        // Construct and drop one flat row-major view at a time. Preparation
        // therefore uses O(rows + nnz(matrix)) auxiliary memory, not three
        // nested row-vector transposes held simultaneously.
        let rows = CanonicalRows::new(matrix);
        validate_matrix_field(matrix_name, &rows, field_modulus_encoding)?;
        hash.update(&[label]);
        for row in rows.rows() {
            hash_usize(&mut hash, row.len())?;
            for (column, coefficient) in row {
                hash_usize(&mut hash, *column)?;
                hash_bytes(&mut hash, &coefficient.canonical_element_encoding())?;
            }
        }
    }

    Ok(*hash.finalize().as_bytes())
}

fn hash_bytes(hash: &mut Hasher, bytes: &[u8]) -> Result<(), SpartanMatrixError> {
    hash_usize(hash, bytes.len())?;
    hash.update(bytes);
    Ok(())
}

fn hash_usize(hash: &mut Hasher, value: usize) -> Result<(), SpartanMatrixError> {
    let encoded = u64::try_from(value).map_err(|_| SpartanMatrixError::DomainTooLarge)?;
    hash.update(&encoded.to_le_bytes());
    Ok(())
}

fn padded_num_vars(logical_len: usize) -> Result<usize, SpartanMatrixError> {
    logical_len
        .max(1)
        .checked_next_power_of_two()
        .map(|length| length.ilog2() as usize)
        .ok_or(SpartanMatrixError::DomainTooLarge)
}

fn domain_size(num_vars: usize) -> Result<usize, SpartanMatrixError> {
    1usize
        .checked_shl(u32::try_from(num_vars).map_err(|_| SpartanMatrixError::DomainTooLarge)?)
        .ok_or(SpartanMatrixError::DomainTooLarge)
}

#[inline]
fn add<F>(left: &F, right: &F) -> F
where
    F: SpartanField,
{
    let mut result = left.clone();
    result += right;
    result
}

#[inline]
fn sub<F>(left: &F, right: &F) -> F
where
    F: SpartanField,
{
    let mut result = left.clone();
    result -= right;
    result
}

#[inline]
fn mul<F>(left: &F, right: &F) -> F
where
    F: SpartanField,
{
    let mut result = left.clone();
    result *= right;
    result
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        FromWithConfig, PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint,
    };

    use super::*;

    const TEST_MODULUS: u128 = (1_u128 << 100) - 15;
    const OTHER_TEST_MODULUS: u128 = (1_u128 << 127) - 1;

    fn config() -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(TEST_MODULUS)).expect("odd test modulus")
    }

    fn field(value: u64, config: &<F128 as PrimeField>::Config) -> F128 {
        F128::from_with_cfg(value, config)
    }

    fn columns_from_rows<F: Clone>(
        column_count: usize,
        rows: &[Vec<(usize, F)>],
    ) -> Vec<Vec<(usize, F)>> {
        let mut columns: Vec<Vec<(usize, F)>> = (0..column_count).map(|_| Vec::new()).collect();
        for (row, entries) in rows.iter().enumerate() {
            for (column, coefficient) in entries {
                columns[*column].push((row, coefficient.clone()));
            }
        }
        columns
    }

    fn legacy_row_major_digest(
        rows: [&[Vec<(usize, F128)>]; 3],
        row_count: usize,
        column_count: usize,
        field_config: &<F128 as PrimeField>::Config,
    ) -> [u8; 32] {
        let mut hash = Hasher::new();
        hash.update(b"f2z/spartan/constraint-matrices/v2");
        hash_bytes(&mut hash, &F128::canonical_modulus_encoding(field_config)).unwrap();
        hash_usize(&mut hash, row_count).unwrap();
        hash_usize(&mut hash, column_count).unwrap();

        for (label, matrix_rows) in [b'A', b'B', b'C'].into_iter().zip(rows) {
            hash.update(&[label]);
            for row in matrix_rows {
                hash_usize(&mut hash, row.len()).unwrap();
                for (column, coefficient) in row {
                    hash_usize(&mut hash, *column).unwrap();
                    hash_bytes(&mut hash, &coefficient.canonical_element_encoding()).unwrap();
                }
            }
        }

        *hash.finalize().as_bytes()
    }

    #[test]
    fn row_constructor_materializes_flat_csc_and_retains_empty_shape() {
        let config = config();
        let two = field(2, &config);
        let three = field(3, &config);
        let five = field(5, &config);
        let seven = field(7, &config);
        let matrix = SparseMatrix::try_from_rows(
            5,
            vec![
                vec![(0, two.clone()), (3, three.clone())],
                vec![],
                vec![(0, five.clone()), (2, seven.clone())],
                vec![],
            ],
        )
        .unwrap();

        assert_eq!(matrix.row_count(), 4);
        assert_eq!(matrix.column_count(), 5);
        assert_eq!(matrix.nnz(), 4);
        assert_eq!(matrix.columns().len(), 5);
        assert_eq!(matrix.column(0), Some(&[(0, two), (2, five)][..]));
        assert_eq!(matrix.column(1), Some(&[][..]));
        assert_eq!(matrix.column(2), Some(&[(2, seven)][..]));
        assert_eq!(matrix.column(3), Some(&[(0, three)][..]));
        assert_eq!(matrix.column(4), Some(&[][..]));
        assert_eq!(matrix.column(5), None);
        assert_eq!(matrix.column(usize::MAX), None);
    }

    #[test]
    fn row_and_column_constructors_produce_identical_canonical_csc() {
        let config = config();
        let rows = vec![
            vec![(0, field(2, &config)), (3, field(3, &config))],
            vec![],
            vec![(0, field(5, &config)), (2, field(7, &config))],
            vec![],
        ];
        let from_rows = SparseMatrix::try_from_rows(5, rows.clone()).unwrap();
        let from_columns =
            SparseMatrix::try_from_columns(rows.len(), columns_from_rows(5, &rows)).unwrap();

        assert_eq!(from_rows, from_columns);
    }

    #[test]
    fn constructors_reject_noncanonical_or_out_of_bounds_coordinates() {
        let config = config();
        let value = || field(1, &config);

        assert_eq!(
            SparseMatrix::try_from_rows(2, vec![vec![(1, value()), (1, value())]]),
            Err(SpartanMatrixError::ColumnsNotStrictlyIncreasing {
                row: 0,
                previous: 1,
                column: 1,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_rows(2, vec![vec![(2, value())]]),
            Err(SpartanMatrixError::ColumnOutOfBounds {
                row: 0,
                column: 2,
                columns: 2,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_columns(2, vec![vec![(1, value()), (0, value())], vec![]]),
            Err(SpartanMatrixError::RowsNotStrictlyIncreasing {
                column: 0,
                previous: 1,
                row: 0,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_columns(2, vec![vec![], vec![(2, value())]]),
            Err(SpartanMatrixError::RowOutOfBounds {
                column: 1,
                row: 2,
                rows: 2,
            })
        );
    }

    #[test]
    fn equality_tables_are_little_endian_and_split_low_coordinates_first() {
        let config = config();
        let r0 = field(2, &config);
        let r1 = field(7, &config);
        let one = F128::one_with_cfg(&config);
        let table = eq_table(&[r0.clone(), r1.clone()], &config).unwrap();

        assert_eq!(
            table,
            vec![
                mul(&sub(&one, &r0), &sub(&one, &r1)),
                mul(&r0, &sub(&one, &r1)),
                mul(&sub(&one, &r0), &r1),
                mul(&r0, &r1),
            ]
        );

        let r2 = field(11, &config);
        let point = [r0, r1, r2];
        let (low, high) = make_equality_factors(&point, &config).unwrap();
        assert_eq!(low.num_vars, 1);
        assert_eq!(low.evaluations, eq_table(&point[..1], &config).unwrap());
        assert_eq!(high.num_vars, 2);
        assert_eq!(high.evaluations, eq_table(&point[1..], &config).unwrap());
    }

    #[test]
    fn bound_table_and_independent_sparse_evaluation_agree() {
        let config = config();
        let a = SparseMatrix::try_from_rows(
            3,
            vec![
                vec![(0, field(2, &config)), (2, field(3, &config))],
                vec![(1, field(5, &config))],
            ],
        )
        .unwrap();
        let b = SparseMatrix::try_from_rows(
            3,
            vec![vec![(1, field(7, &config))], vec![(2, field(11, &config))]],
        )
        .unwrap();
        let c = SparseMatrix::try_from_rows(
            3,
            vec![vec![(0, field(13, &config))], vec![(0, field(17, &config))]],
        )
        .unwrap();
        let prepared =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();
        let row_point = [field(19, &config)];
        let column_point = [field(23, &config), field(29, &config)];
        let rho = field(31, &config);

        let bound = prepared.bind_and_batch(&row_point, &rho).unwrap();
        let dense_evaluation = evaluate_mle(&bound, &column_point, &config).unwrap();
        let sparse_evaluation = prepared
            .evaluate_batched(&row_point, &rho, &column_point)
            .unwrap();
        assert_eq!(dense_evaluation, sparse_evaluation);
    }

    #[test]
    fn csc_binding_matches_dense_oracle_with_empty_and_padded_domains() {
        let config = config();
        let row_count = 3;
        let column_count = 6;
        let zero = F128::zero_with_cfg(&config);

        let a_rows = vec![
            vec![(0, field(2, &config)), (4, field(3, &config))],
            vec![],
            vec![(2, field(5, &config))],
        ];
        let b_rows = vec![
            vec![(1, field(7, &config))],
            vec![(4, field(11, &config))],
            vec![],
        ];
        let c_rows = vec![
            vec![],
            vec![(0, field(13, &config))],
            vec![(3, field(17, &config))],
        ];
        let matrices = ConstraintMatrices::new(
            SparseMatrix::try_from_rows(column_count, a_rows.clone()).unwrap(),
            SparseMatrix::try_from_rows(column_count, b_rows.clone()).unwrap(),
            SparseMatrix::try_from_rows(column_count, c_rows.clone()).unwrap(),
        )
        .unwrap();
        let prepared = PreparedConstraintMatrices::new(matrices, &config).unwrap();
        let row_point = [field(19, &config), field(23, &config)];
        let column_point = [field(29, &config), field(31, &config), field(37, &config)];
        let rho = field(41, &config);
        let rho_squared = mul(&rho, &rho);
        let row_weights = eq_table(&row_point, &config).unwrap();
        let column_weights = eq_table(&column_point, &config).unwrap();

        let coefficient_at = |rows: &[Vec<(usize, F128)>], row: usize, column: usize| {
            rows[row]
                .iter()
                .find_map(|(entry_column, coefficient)| {
                    (*entry_column == column).then(|| coefficient.clone())
                })
                .unwrap_or_else(|| zero.clone())
        };
        let mut expected_bound = Vec::with_capacity(8);
        for column in 0..column_count {
            let mut column_evaluation = zero.clone();
            for row in 0..row_count {
                let mut batched_coefficient = coefficient_at(&a_rows, row, column);
                batched_coefficient += &mul(&rho, &coefficient_at(&b_rows, row, column));
                batched_coefficient += &mul(&rho_squared, &coefficient_at(&c_rows, row, column));
                column_evaluation += &mul(&row_weights[row], &batched_coefficient);
            }
            expected_bound.push(column_evaluation);
        }
        expected_bound.resize(8, zero.clone());

        let bound = prepared.bind_and_batch(&row_point, &rho).unwrap();
        assert_eq!(bound.num_vars, 3);
        assert_eq!(bound.evaluations, expected_bound);
        assert_eq!(bound.evaluations[5], zero);

        let expected_evaluation = expected_bound.iter().zip(&column_weights).fold(
            F128::zero_with_cfg(&config),
            |mut evaluation, (bound_value, column_weight)| {
                evaluation += &mul(bound_value, column_weight);
                evaluation
            },
        );
        assert_eq!(
            prepared
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap(),
            expected_evaluation
        );
    }

    #[test]
    fn csc_preparation_preserves_legacy_row_major_digest() {
        let config = config();
        let column_count = 5;
        let a_rows = vec![
            vec![(0, field(2, &config)), (3, field(3, &config))],
            vec![],
            vec![(1, field(5, &config))],
            vec![],
        ];
        let b_rows = vec![
            vec![],
            vec![(2, field(7, &config)), (4, field(11, &config))],
            vec![],
            vec![(0, field(13, &config))],
        ];
        let c_rows = vec![
            vec![(4, field(17, &config))],
            vec![],
            vec![(0, field(19, &config)), (3, field(23, &config))],
            vec![],
        ];
        let row_count = a_rows.len();

        let from_rows = ConstraintMatrices::new(
            SparseMatrix::try_from_rows(column_count, a_rows.clone()).unwrap(),
            SparseMatrix::try_from_rows(column_count, b_rows.clone()).unwrap(),
            SparseMatrix::try_from_rows(column_count, c_rows.clone()).unwrap(),
        )
        .unwrap();
        let from_columns = ConstraintMatrices::new(
            SparseMatrix::try_from_columns(row_count, columns_from_rows(column_count, &a_rows))
                .unwrap(),
            SparseMatrix::try_from_columns(row_count, columns_from_rows(column_count, &b_rows))
                .unwrap(),
            SparseMatrix::try_from_columns(row_count, columns_from_rows(column_count, &c_rows))
                .unwrap(),
        )
        .unwrap();

        let expected = legacy_row_major_digest(
            [&a_rows, &b_rows, &c_rows],
            row_count,
            column_count,
            &config,
        );
        let row_prepared = PreparedConstraintMatrices::new(from_rows, &config).unwrap();
        let column_prepared = PreparedConstraintMatrices::new(from_columns, &config).unwrap();
        assert_eq!(*row_prepared.digest(), expected);
        assert_eq!(column_prepared.digest(), row_prepared.digest());
    }

    #[test]
    fn csc_validation_preserves_canonical_row_major_error_order() {
        let config = config();
        let zero = F128::zero_with_cfg(&config);
        let one = F128::one_with_cfg(&config);
        // CSC visits (row 1, column 0) before (row 0, column 1), but the
        // statement's canonical validation order remains row-major.
        let a = SparseMatrix::try_from_columns(2, vec![vec![(1, zero.clone())], vec![(0, zero)]])
            .unwrap();
        let valid =
            || SparseMatrix::try_from_columns(2, vec![vec![(0, one.clone())], Vec::new()]).unwrap();

        assert!(matches!(
            PreparedConstraintMatrices::new(
                ConstraintMatrices::new(a, valid(), valid()).unwrap(),
                &config,
            ),
            Err(SpartanMatrixError::ExplicitZeroCoefficient {
                matrix: "A",
                row: 0,
                column: 1,
            })
        ));
    }

    #[test]
    fn checked_builders_pad_only_at_the_end() {
        let config = config();
        let one = F128::one_with_cfg(&config);
        let zero = F128::zero_with_cfg(&config);
        let assignment = [one.clone(), field(2, &config), field(3, &config)];
        let assignment_mle = build_assignment_mle(&assignment, 3, &config).unwrap();
        assert_eq!(assignment_mle.num_vars, 2);
        assert_eq!(
            assignment_mle.evaluations,
            vec![
                assignment[0].clone(),
                assignment[1].clone(),
                assignment[2].clone(),
                zero
            ]
        );

        let products = build_product_mles(
            &[field(4, &config), field(5, &config), field(6, &config)],
            &[field(7, &config), field(8, &config), field(9, &config)],
            &[field(10, &config), field(11, &config), field(12, &config)],
            3,
            &config,
        )
        .unwrap();
        assert_eq!(products.az.num_vars, 2);
        assert_eq!(products.bz.num_vars, 2);
        assert_eq!(products.cz.num_vars, 2);

        let invalid = [field(2, &config), one];
        assert_eq!(
            build_assignment_mle(&invalid, 2, &config),
            Err(SpartanMatrixError::InvalidAssignmentConstant)
        );
    }

    #[test]
    fn scaled_claim_checks_without_dividing_by_the_scale() {
        let config = config();
        let point = [field(2, &config), field(3, &config)];
        let polynomial = DenseMultilinearExtension {
            evaluations: vec![
                field(5, &config),
                field(7, &config),
                field(11, &config),
                field(13, &config),
            ],
            num_vars: 2,
        };
        let evaluation = evaluate_mle(&polynomial, &point, &config).unwrap();
        let scale = field(17, &config);
        let claim = ScaledMleEvaluationClaim::new(
            point.to_vec().into_boxed_slice(),
            scale.clone(),
            mul(&scale, &evaluation),
        );
        claim.nonsuccinct_verify(&polynomial, &config).unwrap();

        let zero = F128::zero_with_cfg(&config);
        let zero_scaled =
            ScaledMleEvaluationClaim::new(point.to_vec().into_boxed_slice(), zero.clone(), zero);
        zero_scaled
            .nonsuccinct_verify(&polynomial, &config)
            .unwrap();
    }

    #[test]
    fn prepared_statement_rejects_coefficients_from_another_modulus() {
        let config = config();
        let other_config = F128::make_cfg(&Uint::from(OTHER_TEST_MODULUS)).unwrap();
        let foreign = field(1, &other_config);
        let local = field(1, &config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, foreign)]]).unwrap();
        let b = SparseMatrix::try_from_rows(1, vec![vec![(0, local.clone())]]).unwrap();
        let c = SparseMatrix::try_from_rows(1, vec![vec![(0, local)]]).unwrap();
        let matrices = ConstraintMatrices::new(a, b, c).unwrap();

        assert!(matches!(
            PreparedConstraintMatrices::new(matrices, &config),
            Err(SpartanMatrixError::FieldConfigurationMismatch)
        ));
    }

    #[test]
    fn prepared_digest_binds_the_runtime_modulus() {
        let config = config();
        let other_config = F128::make_cfg(&Uint::from(OTHER_TEST_MODULUS)).unwrap();

        let prepare = |config: &<F128 as PrimeField>::Config| {
            let matrix =
                || SparseMatrix::try_from_rows(1, vec![vec![(0, field(1, config))]]).unwrap();
            PreparedConstraintMatrices::new(
                ConstraintMatrices::new(matrix(), matrix(), matrix()).unwrap(),
                config,
            )
            .unwrap()
        };
        let prepared = prepare(&config);
        let other_prepared = prepare(&other_config);

        assert_ne!(
            prepared.field_modulus_encoding(),
            other_prepared.field_modulus_encoding()
        );
        assert_ne!(prepared.digest(), other_prepared.digest());
    }

    #[test]
    fn prepared_statement_rejects_unsafe_runtime_fields() {
        let matrices = |config: &<F128 as PrimeField>::Config| {
            let matrix =
                || SparseMatrix::try_from_rows(1, vec![vec![(0, field(1, config))]]).unwrap();
            ConstraintMatrices::new(matrix(), matrix(), matrix()).unwrap()
        };

        let composite = F128::make_cfg(&Uint::from((1_u128 << 100) - 17)).unwrap();
        assert!(matches!(
            PreparedConstraintMatrices::new(matrices(&composite), &composite),
            Err(SpartanMatrixError::InvalidFieldConfiguration(
                SpartanFieldError::CompositeModulus
            ))
        ));

        let undersized = F128::make_cfg(&Uint::from(97_u128)).unwrap();
        assert!(matches!(
            PreparedConstraintMatrices::new(matrices(&undersized), &undersized),
            Err(SpartanMatrixError::InvalidFieldConfiguration(
                SpartanFieldError::ModulusTooSmall { actual_bits: 7 }
            ))
        ));
    }

    #[test]
    fn prepared_statement_rejects_explicit_sparse_zeroes() {
        let config = config();
        let zero = F128::zero_with_cfg(&config);
        let one = F128::one_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, zero)]]).unwrap();
        let b = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let c = SparseMatrix::try_from_rows(1, vec![vec![(0, one)]]).unwrap();

        assert!(matches!(
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config),
            Err(SpartanMatrixError::ExplicitZeroCoefficient {
                matrix: "A",
                row: 0,
                column: 0,
            })
        ));
    }

    #[test]
    fn prepared_statement_rejects_unchecked_noncanonical_residues() {
        let config = config();
        let malformed = F128::new_unchecked(Uint::from(u128::MAX), &config);
        let one = F128::one_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, malformed)]]).unwrap();
        let b = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let c = SparseMatrix::try_from_rows(1, vec![vec![(0, one)]]).unwrap();

        assert!(matches!(
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config),
            Err(SpartanMatrixError::InvalidFieldConfiguration(
                SpartanFieldError::NonCanonicalElement
            ))
        ));
    }
}
