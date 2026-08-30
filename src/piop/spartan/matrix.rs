//! Validated R1CS matrices and multilinear-table helpers for Spartan.
//!
//! This module starts at the field-valued R1CS boundary. Constraint generation
//! and the F2-to-integer witness map are intentionally out of scope: callers
//! supply the three matrices `A`, `B`, and `C`, their row products, and the
//! complete assignment consumed by those matrices.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

use blake3::Hasher;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use thiserror::Error;

use super::spliced_digest::{SplicedDigestBuilder, SplicedStreamDigest};
use crate::{
    poly::mle::DenseMultilinearExtension,
    sparse_matrix::{SparseColumn, SparseMatrixError},
};

/// Domain tag of the prepared-statement digest (`v2`), shared verbatim by
/// the entry-wise walk and the skeleton's cached stream.
const CONSTRAINT_MATRIX_DIGEST_DOMAIN: &[u8] = b"f2z/spartan/constraint-matrices/v2";

pub use crate::sparse_matrix::SparseMatrix;

use super::{sumcheck::R1csProductMles, SpartanField, SpartanFieldError};

/// A sparse R1CS coefficient that can act on values in `F`.
///
/// The coefficient's canonical encoding is always the encoding of the
/// corresponding element of `F`. Consequently, a Boolean matrix containing
/// `true` has the same prepared-statement digest as a field-valued matrix
/// containing `F::one_with_cfg(field_config)` at the same coordinates.
pub trait SpartanMatrixCoefficient<F>: Clone + Send + Sync
where
    F: SpartanField,
{
    /// Validates coefficient-specific invariants against the prepared field.
    fn validate(&self, field_modulus_encoding: &[u8]) -> Result<(), SpartanMatrixError>;

    /// Whether this is an explicit zero, which sparse matrices forbid.
    fn is_zero(&self) -> bool;

    /// Canonical encoding of this coefficient as an element of `F`.
    fn canonical_field_encoding<'a>(
        &'a self,
        field_config: &F::Config,
        field_one_encoding: &'a [u8],
    ) -> Cow<'a, [u8]>;

    /// Multiplies a field value by this coefficient.
    fn scale(&self, value: &F, field_config: &F::Config) -> F;

    /// Computes one CSC column's dot product with field-valued row weights.
    fn column_dot(
        column: SparseColumn<'_, Self>,
        row_weights: &[F],
        zero: &F,
        field_config: &F::Config,
    ) -> F
    where
        Self: Sized,
    {
        let mut evaluation = zero.clone();
        for (row, coefficient) in column {
            evaluation += &coefficient.scale(&row_weights[row], field_config);
        }
        evaluation
    }
}

impl<F> SpartanMatrixCoefficient<F> for F
where
    F: SpartanField,
{
    fn validate(&self, field_modulus_encoding: &[u8]) -> Result<(), SpartanMatrixError> {
        validate_element_field(self, field_modulus_encoding)
    }

    fn is_zero(&self) -> bool {
        F::is_zero(self)
    }

    fn canonical_field_encoding<'a>(
        &'a self,
        _field_config: &F::Config,
        _field_one_encoding: &'a [u8],
    ) -> Cow<'a, [u8]> {
        Cow::Owned(self.canonical_element_encoding())
    }

    fn scale(&self, value: &F, _field_config: &F::Config) -> F {
        mul(value, self)
    }
}

impl<F> SpartanMatrixCoefficient<F> for bool
where
    F: SpartanField,
{
    fn validate(&self, _field_modulus_encoding: &[u8]) -> Result<(), SpartanMatrixError> {
        Ok(())
    }

    fn is_zero(&self) -> bool {
        !*self
    }

    fn canonical_field_encoding<'a>(
        &'a self,
        field_config: &F::Config,
        field_one_encoding: &'a [u8],
    ) -> Cow<'a, [u8]> {
        if *self {
            Cow::Borrowed(field_one_encoding)
        } else {
            Cow::Owned(F::zero_with_cfg(field_config).canonical_element_encoding())
        }
    }

    fn scale(&self, value: &F, field_config: &F::Config) -> F {
        if *self {
            value.clone()
        } else {
            F::zero_with_cfg(field_config)
        }
    }

    fn column_dot(
        column: SparseColumn<'_, Self>,
        row_weights: &[F],
        zero: &F,
        field_config: &F::Config,
    ) -> F {
        if let Some((row, true)) = column.single() {
            return row_weights[row].clone();
        }

        let mut evaluation = zero.clone();
        for (row, coefficient) in column {
            evaluation += &coefficient.scale(&row_weights[row], field_config);
        }
        evaluation
    }
}

/// A sparse coefficient whose field action and canonical encoding are the
/// same under every runtime field configuration.
///
/// This is the contract that lets a [`ConstraintMatricesSkeleton`] hoist the
/// per-statement clone, validation, canonical digesting, and selector
/// detection out of the per-proof [`PreparedConstraintMatrices`] construction
/// when the modulus is drawn from the transcript. Implementations promise
/// that, for every configuration accepted by `F::validate_config`:
///
/// - [`SpartanMatrixCoefficient::validate`] succeeds unconditionally,
/// - [`SpartanMatrixCoefficient::canonical_field_encoding`] returns the exact
///   bytes written by [`Self::write_modulus_independent_encoding`], and
/// - [`Self::is_unit`] is `true` exactly when that encoding equals the
///   field's canonical one-encoding.
///
/// Under this contract [`PreparedConstraintMatrices::from_skeleton`] produces
/// the same statement digest as [`PreparedConstraintMatrices::new`] for every
/// accepted configuration.
pub trait ModulusIndependentCoefficient<F>: SpartanMatrixCoefficient<F>
where
    F: SpartanField,
{
    /// Appends the exact bytes
    /// [`SpartanMatrixCoefficient::canonical_field_encoding`] returns under
    /// every accepted configuration.
    fn write_modulus_independent_encoding(&self, out: &mut Vec<u8>);

    /// Whether this coefficient acts as the multiplicative unit under every
    /// accepted configuration.
    fn is_unit(&self) -> bool;
}

/// Failures while constructing or evaluating a Spartan matrix statement.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SpartanMatrixError {
    /// The generic CSC matrix is malformed.
    #[error(transparent)]
    SparseMatrix(#[from] SparseMatrixError),

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

    /// A row-domain weight table has the wrong padded length.
    #[error("row-weight table has length {actual}, expected {expected}")]
    InvalidRowWeightsLength { expected: usize, actual: usize },

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

/// Compact row functional produced by a prefix-univariate outer reduction.
///
/// For `M = 2^K` and `row = s + M * x`, the represented weight is
///
/// `prefix[s] * tail_low[x_low] * tail_high[x_high]`.
///
/// Keeping the three factors separate avoids materializing the complete
/// `2^num_row_vars` row-weight table when the prepared matrix layout supports
/// streamed row binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrefixUnivariateRowFactors<F> {
    skip_vars: usize,
    prefix: Box<[F]>,
    tail_low: Box<[F]>,
    tail_high: Box<[F]>,
    tail_low_vars: usize,
    num_row_vars: usize,
}

impl<F> PrefixUnivariateRowFactors<F>
where
    F: SpartanField,
{
    pub(crate) fn new(
        skip_vars: usize,
        prefix: Vec<F>,
        tail_low: DenseMultilinearExtension<F>,
        tail_high: DenseMultilinearExtension<F>,
        num_row_vars: usize,
    ) -> Result<Self, SpartanMatrixError> {
        let expected_prefix = domain_size(skip_vars)?;
        let expected_low = domain_size(tail_low.num_vars)?;
        let expected_high = domain_size(tail_high.num_vars)?;
        let actual_rows = prefix
            .len()
            .checked_mul(tail_low.evaluations.len())
            .and_then(|length| length.checked_mul(tail_high.evaluations.len()))
            .ok_or(SpartanMatrixError::DomainTooLarge)?;
        let expected_rows = domain_size(num_row_vars)?;
        if prefix.len() != expected_prefix
            || tail_low.evaluations.len() != expected_low
            || tail_high.evaluations.len() != expected_high
            || skip_vars
                .checked_add(tail_low.num_vars)
                .and_then(|width| width.checked_add(tail_high.num_vars))
                != Some(num_row_vars)
            || actual_rows != expected_rows
        {
            return Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: expected_rows,
                actual: actual_rows,
            });
        }

        Ok(Self {
            skip_vars,
            prefix: prefix.into_boxed_slice(),
            tail_low: tail_low.evaluations.into_boxed_slice(),
            tail_high: tail_high.evaluations.into_boxed_slice(),
            tail_low_vars: tail_low.num_vars,
            num_row_vars,
        })
    }

    /// Visits every logical row in little-endian `s + 2^K*x` order.
    fn for_each_row(&self, logical_rows: usize, mut consume: impl FnMut(usize, F)) {
        debug_assert!(logical_rows <= 1usize << self.num_row_vars);
        let block_len = 1usize << self.skip_vars;
        let low_mask = self.tail_low.len() - 1;
        let suffixes = logical_rows.div_ceil(block_len);

        for suffix in 0..suffixes {
            let low_index = suffix & low_mask;
            let high_index = suffix >> self.tail_low_vars;
            let tail_weight = mul(&self.tail_low[low_index], &self.tail_high[high_index]);
            let row_start = suffix * block_len;
            let active = block_len.min(logical_rows - row_start);
            for prefix_index in 0..active {
                consume(
                    row_start + prefix_index,
                    mul(&self.prefix[prefix_index], &tail_weight),
                );
            }
        }
    }

    /// Reference/fallback materialization in canonical row order.
    pub(crate) fn materialize(&self) -> Vec<F> {
        let mut weights = Vec::with_capacity(1usize << self.num_row_vars);
        self.for_each_row(1usize << self.num_row_vars, |row, weight| {
            debug_assert_eq!(row, weights.len());
            weights.push(weight);
        });
        weights
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
#[derive(Clone, Copy, Debug)]
struct DisjointUnitSelectorTriplet {
    rows: usize,
    a_offset: usize,
    b_offset: usize,
    c_offset: usize,
}

/// The modulus-independent core of a prepared R1CS statement: validated,
/// digest-ready matrices plus the padded domain widths and the detected
/// selector layout — everything [`PreparedConstraintMatrices::new`] derives
/// that does not depend on the runtime field configuration.
///
/// Protocols that draw their field modulus from the transcript (the paper
/// Step-2 prime) build this once per relation and instantiate each proof's
/// [`PreparedConstraintMatrices`] with
/// [`PreparedConstraintMatrices::from_skeleton`]. That replaces the
/// per-proof clone, re-validation, selector re-detection, and `O(nnz)`
/// re-digest of the sparse matrices with a shared-ownership handle and an
/// `O(log nnz)` digest replay — while producing bit-identical prepared
/// statements (the digest included) for every accepted configuration.
#[derive(Clone, Debug)]
pub struct ConstraintMatricesSkeleton<F, C = F>
where
    F: SpartanField,
{
    matrices: Arc<ConstraintMatrices<C>>,
    digest_stream: SplicedStreamDigest,
    num_row_vars: usize,
    num_column_vars: usize,
    selector_triplet: Option<DisjointUnitSelectorTriplet>,
    _field: PhantomData<fn() -> F>,
}

impl<F, C> ConstraintMatricesSkeleton<F, C>
where
    F: SpartanField,
    C: ModulusIndependentCoefficient<F>,
{
    /// Validates the matrices and caches the modulus-independent part of the
    /// statement digest.
    pub fn new(matrices: ConstraintMatrices<C>) -> Result<Self, SpartanMatrixError> {
        let num_row_vars = padded_num_vars(matrices.row_count())?;
        let num_column_vars = padded_num_vars(matrices.column_count())?;

        // The digest stream is q-dependent only inside the modulus-encoding
        // hole right after the domain tag and length prefix; every accepted
        // configuration writes the same fixed-width encoding there.
        let modulus_width = F::canonical_encoding_width();
        let hole_start = CONSTRAINT_MATRIX_DIGEST_DOMAIN
            .len()
            .checked_add(8)
            .ok_or(SpartanMatrixError::DomainTooLarge)?;
        let hole_end = hole_start
            .checked_add(modulus_width)
            .ok_or(SpartanMatrixError::DomainTooLarge)?;
        let mut builder = SplicedDigestBuilder::new(hole_start..hole_end);
        stream_constraint_matrix_digest_bytes(&matrices, modulus_width, &mut builder)?;
        let digest_stream = builder
            .finish()
            .map_err(|_| SpartanMatrixError::DomainTooLarge)?;

        let selector_triplet =
            detect_disjoint_unit_selector_triplet_with(&matrices, |coefficient: &C| {
                coefficient.is_unit()
            });
        Ok(Self {
            matrices: Arc::new(matrices),
            digest_stream,
            num_row_vars,
            num_column_vars,
            selector_triplet,
            _field: PhantomData,
        })
    }

    /// Validated `A`, `B`, and `C` matrices.
    pub fn matrices(&self) -> &ConstraintMatrices<C> {
        &self.matrices
    }
}

#[derive(Clone, Debug)]
pub struct PreparedConstraintMatrices<F, C = F>
where
    F: SpartanField,
{
    matrices: Arc<ConstraintMatrices<C>>,
    field_config: F::Config,
    field_modulus_encoding: Vec<u8>,
    digest: [u8; 32],
    num_row_vars: usize,
    num_column_vars: usize,
    selector_triplet: Option<DisjointUnitSelectorTriplet>,
}

impl<F, C> PreparedConstraintMatrices<F, C>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    /// Validates the coefficient field, computes padded domain widths, and
    /// commits to the complete public statement with BLAKE3.
    pub fn new(
        matrices: ConstraintMatrices<C>,
        field_config: &F::Config,
    ) -> Result<Self, SpartanMatrixError> {
        F::validate_config(field_config)?;
        let num_row_vars = padded_num_vars(matrices.row_count())?;
        let num_column_vars = padded_num_vars(matrices.column_count())?;
        let field_modulus_encoding = F::canonical_modulus_encoding(field_config);
        let digest = constraint_matrix_digest(&matrices, field_config, &field_modulus_encoding)?;
        let selector_triplet =
            detect_disjoint_unit_selector_triplet::<F, C>(&matrices, field_config);

        Ok(Self {
            matrices: Arc::new(matrices),
            field_config: field_config.clone(),
            field_modulus_encoding,
            digest,
            num_row_vars,
            num_column_vars,
            selector_triplet,
        })
    }

    /// Instantiates the prepared statement for one runtime field
    /// configuration from a modulus-independent skeleton, sharing the
    /// skeleton's validated matrices instead of cloning them.
    ///
    /// This is bit-identical to [`Self::new`] on the same matrices and
    /// configuration — the statement digest included, by splicing the
    /// configuration's modulus encoding into the cached digest stream — but
    /// runs in `O(log nnz)` instead of `O(nnz)`.
    pub fn from_skeleton(
        skeleton: &ConstraintMatricesSkeleton<F, C>,
        field_config: &F::Config,
    ) -> Result<Self, SpartanMatrixError>
    where
        C: ModulusIndependentCoefficient<F>,
    {
        F::validate_config(field_config)?;
        let field_modulus_encoding = F::canonical_modulus_encoding(field_config);
        let digest = skeleton
            .digest_stream
            .digest_with(&field_modulus_encoding)
            .map_err(|_| SpartanMatrixError::FieldConfigurationMismatch)?;

        Ok(Self {
            matrices: Arc::clone(&skeleton.matrices),
            field_config: field_config.clone(),
            field_modulus_encoding,
            digest,
            num_row_vars: skeleton.num_row_vars,
            num_column_vars: skeleton.num_column_vars,
            selector_triplet: skeleton.selector_triplet,
        })
    }

    /// Validated `A`, `B`, and `C` matrices.
    pub fn matrices(&self) -> &ConstraintMatrices<C> {
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

        let row_weights = eq_table_prover(row_point, &self.field_config)?;
        self.bind_and_batch_with_validated_row_weights(&row_weights, rho)
    }

    /// Constructs the dense column MLE from an explicit field-valued row
    /// functional:
    ///
    /// `D(j) = sum_i row_weights[i] (A[i,j] + rho B[i,j] + rho^2 C[i,j])`.
    ///
    /// `row_weights` covers the complete padded row domain in little-endian
    /// index order. Weights for padding rows are accepted but have no effect
    /// because the sparse matrices contain only logical rows.
    #[allow(dead_code)]
    pub(crate) fn bind_and_batch_with_row_weights(
        &self,
        row_weights: &[F],
        rho: &F,
    ) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError> {
        let expected_row_weights = domain_size(self.num_row_vars)?;
        if row_weights.len() != expected_row_weights {
            return Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: expected_row_weights,
                actual: row_weights.len(),
            });
        }
        validate_elements_field(row_weights, &self.field_modulus_encoding)?;
        validate_element_field(rho, &self.field_modulus_encoding)?;

        self.bind_and_batch_with_validated_row_weights(row_weights, rho)
    }

    /// Binding core for row weights already derived from transcript-validated
    /// points under this matrix configuration.
    ///
    /// Keeping this separate avoids rescanning and re-encoding every entry of
    /// the exponentially sized equality table on the standard prover path.
    pub(crate) fn bind_and_batch_with_validated_row_weights(
        &self,
        row_weights: &[F],
        rho: &F,
    ) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError> {
        debug_assert_eq!(row_weights.len(), 1usize << self.num_row_vars);

        let zero = F::zero_with_cfg(&self.field_config);
        let rho_squared = mul(rho, rho);

        // Column-parallel over the live columns (each column's value is
        // independent, and its inner sums are untouched — identical
        // values in identical order to the sequential zip).
        let live_columns = self
            .matrices
            .a()
            .column_count()
            .min(self.matrices.b().column_count())
            .min(self.matrices.c().column_count());
        let column_evaluation = |index: usize| -> F {
            let a_column = self.matrices.a().column(index).expect("live column");
            let b_column = self.matrices.b().column(index).expect("live column");
            let c_column = self.matrices.c().column(index).expect("live column");
            let mut evaluation =
                sparse_column_dot(a_column, row_weights, &zero, &self.field_config);
            if !b_column.is_empty() {
                let b_evaluation =
                    sparse_column_dot(b_column, row_weights, &zero, &self.field_config);
                evaluation += &mul(rho, &b_evaluation);
            }
            if !c_column.is_empty() {
                let c_evaluation =
                    sparse_column_dot(c_column, row_weights, &zero, &self.field_config);
                evaluation += &mul(&rho_squared, &c_evaluation);
            }
            evaluation
        };
        #[cfg(feature = "parallel")]
        let mut evaluations: Vec<F> =
            if live_columns >= (1 << 12) && rayon::current_num_threads() > 1 {
                (0..live_columns)
                    .into_par_iter()
                    .map(column_evaluation)
                    .collect()
            } else {
                (0..live_columns).map(column_evaluation).collect()
            };
        #[cfg(not(feature = "parallel"))]
        let mut evaluations: Vec<F> = (0..live_columns).map(column_evaluation).collect();
        evaluations.resize(domain_size(self.num_column_vars)?, zero);

        Ok(DenseMultilinearExtension {
            evaluations,
            num_vars: self.num_column_vars,
        })
    }

    /// Constructs the dense batched column MLE from a three-factor
    /// prefix-univariate row functional.
    ///
    /// Disjoint unit-selector matrices are streamed a row block at a time, so
    /// the complete row-weight tensor is never allocated. Other matrix layouts
    /// retain the existing materialized implementation as a correctness- and
    /// latency-preserving fallback.
    pub(crate) fn bind_and_batch_with_prefix_univariate_factors(
        &self,
        factors: &PrefixUnivariateRowFactors<F>,
        rho: &F,
    ) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError> {
        if factors.num_row_vars != self.num_row_vars {
            return Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: domain_size(self.num_row_vars)?,
                actual: domain_size(factors.num_row_vars)?,
            });
        }

        let Some(layout) = self.selector_triplet else {
            let row_weights = factors.materialize();
            return self.bind_and_batch_with_validated_row_weights(&row_weights, rho);
        };

        let zero = F::zero_with_cfg(&self.field_config);
        let rho_squared = mul(rho, rho);
        let mut evaluations = vec![zero; domain_size(self.num_column_vars)?];
        factors.for_each_row(layout.rows, |row, weight| {
            evaluations[layout.a_offset + row] = weight.clone();
            evaluations[layout.b_offset + row] = mul(rho, &weight);
            evaluations[layout.c_offset + row] = mul(&rho_squared, &weight);
        });

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
        self.evaluate_batched_with_validated_row_weights(&row_weights, rho, column_point)
    }

    /// Directly evaluates the batched matrices against an explicit
    /// field-valued row functional and a multilinear column point:
    ///
    /// `sum_i row_weights[i] (A(i, column_point)
    ///     + rho B(i, column_point) + rho^2 C(i, column_point))`.
    ///
    /// `row_weights` covers the complete padded row domain in little-endian
    /// index order.
    #[allow(dead_code)]
    pub(crate) fn evaluate_batched_with_row_weights(
        &self,
        row_weights: &[F],
        rho: &F,
        column_point: &[F],
    ) -> Result<F, SpartanMatrixError> {
        let expected_row_weights = domain_size(self.num_row_vars)?;
        if row_weights.len() != expected_row_weights {
            return Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: expected_row_weights,
                actual: row_weights.len(),
            });
        }
        if column_point.len() != self.num_column_vars {
            return Err(SpartanMatrixError::InvalidColumnPointLength {
                expected: self.num_column_vars,
                actual: column_point.len(),
            });
        }
        validate_elements_field(row_weights, &self.field_modulus_encoding)?;
        validate_elements_field(column_point, &self.field_modulus_encoding)?;
        validate_element_field(rho, &self.field_modulus_encoding)?;

        self.evaluate_batched_with_validated_row_weights(row_weights, rho, column_point)
    }

    /// Evaluation core for row weights and challenges already validated under
    /// this matrix configuration.
    pub(crate) fn evaluate_batched_with_validated_row_weights(
        &self,
        row_weights: &[F],
        rho: &F,
        column_point: &[F],
    ) -> Result<F, SpartanMatrixError> {
        debug_assert_eq!(row_weights.len(), 1usize << self.num_row_vars);
        debug_assert_eq!(column_point.len(), self.num_column_vars);

        let column_weights = eq_table(column_point, &self.field_config)?;
        let zero = F::zero_with_cfg(&self.field_config);
        let rho_squared = mul(rho, rho);
        let mut evaluation = evaluate_sparse_matrix(
            self.matrices.a(),
            row_weights,
            &column_weights,
            &zero,
            &self.field_config,
        );
        let b_evaluation = evaluate_sparse_matrix(
            self.matrices.b(),
            row_weights,
            &column_weights,
            &zero,
            &self.field_config,
        );
        let c_evaluation = evaluate_sparse_matrix(
            self.matrices.c(),
            row_weights,
            &column_weights,
            &zero,
            &self.field_config,
        );
        evaluation += &mul(rho, &b_evaluation);
        evaluation += &mul(&rho_squared, &c_evaluation);

        Ok(evaluation)
    }

    /// Evaluates the batched matrices against a three-factor
    /// prefix-univariate row functional without constructing its tensor
    /// product when the prepared selector layout supports row streaming.
    pub(crate) fn evaluate_batched_with_prefix_univariate_factors(
        &self,
        factors: &PrefixUnivariateRowFactors<F>,
        rho: &F,
        column_point: &[F],
    ) -> Result<F, SpartanMatrixError> {
        if factors.num_row_vars != self.num_row_vars {
            return Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: domain_size(self.num_row_vars)?,
                actual: domain_size(factors.num_row_vars)?,
            });
        }
        if column_point.len() != self.num_column_vars {
            return Err(SpartanMatrixError::InvalidColumnPointLength {
                expected: self.num_column_vars,
                actual: column_point.len(),
            });
        }

        let Some(layout) = self.selector_triplet else {
            let row_weights = factors.materialize();
            return self.evaluate_batched_with_validated_row_weights(
                &row_weights,
                rho,
                column_point,
            );
        };

        let column_weights = eq_table(column_point, &self.field_config)?;
        let zero = F::zero_with_cfg(&self.field_config);
        let mut a_evaluation = zero.clone();
        let mut b_evaluation = zero.clone();
        let mut c_evaluation = zero;
        factors.for_each_row(layout.rows, |row, weight| {
            a_evaluation += &mul(&weight, &column_weights[layout.a_offset + row]);
            b_evaluation += &mul(&weight, &column_weights[layout.b_offset + row]);
            c_evaluation += &mul(&weight, &column_weights[layout.c_offset + row]);
        });

        let rho_squared = mul(rho, rho);
        a_evaluation += &mul(rho, &b_evaluation);
        a_evaluation += &mul(&rho_squared, &c_evaluation);
        Ok(a_evaluation)
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

/// PROVER-side [`eq_table`] with the per-level doubling parallelized
/// (each level's pair expansions are independent; the products — and
/// therefore the table — are identical to the sequential build). The
/// verifier's evaluation path keeps the plain [`eq_table`].
fn eq_table_prover<F>(point: &[F], field_config: &F::Config) -> Result<Vec<F>, SpartanMatrixError>
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
        let expand = |zero_child: &mut F, one_child: &mut F| {
            let parent = zero_child.clone();
            *one_child = mul(&parent, challenge);
            *zero_child = sub(&parent, one_child);
        };
        #[cfg(feature = "parallel")]
        if half >= (1 << 13) && rayon::current_num_threads() > 1 {
            zero_children
                .par_iter_mut()
                .zip(one_children.par_iter_mut())
                .for_each(|(zero_child, one_child)| expand(zero_child, one_child));
            continue;
        }
        for (zero_child, one_child) in zero_children.iter_mut().zip(one_children) {
            expand(zero_child, one_child);
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

fn sparse_column_dot<F, C>(
    column: SparseColumn<'_, C>,
    row_weights: &[F],
    zero: &F,
    field_config: &F::Config,
) -> F
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    C::column_dot(column, row_weights, zero, field_config)
}

fn evaluate_sparse_matrix<F, C>(
    matrix: &SparseMatrix<C>,
    row_weights: &[F],
    column_weights: &[F],
    zero: &F,
    field_config: &F::Config,
) -> F
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    let mut evaluation = zero.clone();
    for (column, entries) in matrix.columns().enumerate() {
        if !entries.is_empty() {
            let column_evaluation = sparse_column_dot(entries, row_weights, zero, field_config);
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
                row_offsets[row + 1] += 1;
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
            .coefficients()
            .first()
            .map_or_else(Vec::new, |coefficient| vec![(0, coefficient); matrix.nnz()]);
        for (column, column_entries) in matrix.columns().enumerate() {
            for (row, coefficient) in column_entries {
                let position = next_entry[row];
                entries[position] = (column, coefficient);
                next_entry[row] += 1;
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

fn validate_matrix_coefficients<F, C>(
    matrix_name: &'static str,
    rows: &CanonicalRows<'_, C>,
    modulus_encoding: &[u8],
) -> Result<(), SpartanMatrixError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    for (row_index, row) in rows.rows().enumerate() {
        for (column, coefficient) in row {
            coefficient.validate(modulus_encoding)?;
            if coefficient.is_zero() {
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

fn detect_disjoint_unit_selector_triplet<F, C>(
    matrices: &ConstraintMatrices<C>,
    field_config: &F::Config,
) -> Option<DisjointUnitSelectorTriplet>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    let field_one_encoding = F::one_with_cfg(field_config).canonical_element_encoding();
    detect_disjoint_unit_selector_triplet_with(matrices, |coefficient: &C| {
        coefficient
            .canonical_field_encoding(field_config, &field_one_encoding)
            .as_ref()
            == field_one_encoding
    })
}

fn detect_disjoint_unit_selector_triplet_with<C>(
    matrices: &ConstraintMatrices<C>,
    is_unit: impl Fn(&C) -> bool + Copy,
) -> Option<DisjointUnitSelectorTriplet> {
    let rows = matrices.row_count();
    let columns = matrices.column_count();
    let a_offset = contiguous_unit_selector_offset_with(matrices.a(), is_unit)?;
    let b_offset = contiguous_unit_selector_offset_with(matrices.b(), is_unit)?;
    let c_offset = contiguous_unit_selector_offset_with(matrices.c(), is_unit)?;

    let a_end = a_offset.checked_add(rows)?;
    let b_end = b_offset.checked_add(rows)?;
    let c_end = c_offset.checked_add(rows)?;
    if a_end > b_offset || b_end > c_offset || c_end > columns {
        return None;
    }

    Some(DisjointUnitSelectorTriplet {
        rows,
        a_offset,
        b_offset,
        c_offset,
    })
}

fn contiguous_unit_selector_offset_with<C>(
    matrix: &SparseMatrix<C>,
    is_unit: impl Fn(&C) -> bool,
) -> Option<usize> {
    let rows = matrix.row_count();
    if matrix.nnz() != rows {
        return None;
    }

    // The first nonzero CSC boundary immediately follows the first occupied
    // column. `partition_point` avoids scanning the potentially enormous
    // empty prefix of selector matrices.
    let first_nonzero_boundary = matrix
        .column_offsets()
        .partition_point(|entry_offset| *entry_offset == 0);
    let offset = first_nonzero_boundary.checked_sub(1)?;
    if offset.checked_add(rows)? > matrix.column_count() {
        return None;
    }

    for row in 0..rows {
        if matrix.column_offsets()[offset + row] != row
            || matrix.column_offsets()[offset + row + 1] != row + 1
        {
            return None;
        }
        let entry_row = matrix.row_indices()[row];
        let coefficient = &matrix.coefficients()[row];
        if entry_row != row || !is_unit(coefficient) {
            return None;
        }
    }

    Some(offset)
}

fn constraint_matrix_digest<F, C>(
    matrices: &ConstraintMatrices<C>,
    field_config: &F::Config,
    field_modulus_encoding: &[u8],
) -> Result<[u8; 32], SpartanMatrixError>
where
    F: SpartanField,
    C: SpartanMatrixCoefficient<F>,
{
    let mut hash = Hasher::new();
    hash.update(CONSTRAINT_MATRIX_DIGEST_DOMAIN);
    hash_bytes(&mut hash, field_modulus_encoding)?;
    hash_usize(&mut hash, matrices.row_count())?;
    hash_usize(&mut hash, matrices.column_count())?;
    let field_one_encoding = F::one_with_cfg(field_config).canonical_element_encoding();

    for (matrix_name, label, matrix) in [
        ("A", b'A', matrices.a()),
        ("B", b'B', matrices.b()),
        ("C", b'C', matrices.c()),
    ] {
        // Construct and drop one flat row-major view at a time. Preparation
        // therefore uses O(rows + nnz(matrix)) auxiliary memory, not three
        // nested row-vector transposes held simultaneously.
        let rows = CanonicalRows::new(matrix);
        validate_matrix_coefficients::<F, C>(matrix_name, &rows, field_modulus_encoding)?;
        hash.update(&[label]);
        for row in rows.rows() {
            hash_usize(&mut hash, row.len())?;
            for (column, coefficient) in row {
                hash_usize(&mut hash, *column)?;
                let encoding =
                    coefficient.canonical_field_encoding(field_config, &field_one_encoding);
                hash_bytes(&mut hash, encoding.as_ref())?;
            }
        }
    }

    Ok(*hash.finalize().as_bytes())
}

/// Streams the exact byte sequence [`constraint_matrix_digest`] hashes into
/// `builder`, with `modulus_width` placeholder zeros in the modulus-encoding
/// hole, using each coefficient's modulus-independent encoding.
///
/// Mirrors [`constraint_matrix_digest`]'s bytes and its validation: explicit
/// zeros are rejected with the identical row-major error coordinates, and by
/// the [`ModulusIndependentCoefficient`] contract per-entry validation cannot
/// fail for any accepted configuration.
fn stream_constraint_matrix_digest_bytes<F, C>(
    matrices: &ConstraintMatrices<C>,
    modulus_width: usize,
    builder: &mut SplicedDigestBuilder,
) -> Result<(), SpartanMatrixError>
where
    F: SpartanField,
    C: ModulusIndependentCoefficient<F>,
{
    let push_usize = |builder: &mut SplicedDigestBuilder,
                      value: usize|
     -> Result<(), SpartanMatrixError> {
        let encoded = u64::try_from(value).map_err(|_| SpartanMatrixError::DomainTooLarge)?;
        builder.push(&encoded.to_le_bytes());
        Ok(())
    };

    builder.push(CONSTRAINT_MATRIX_DIGEST_DOMAIN);
    push_usize(builder, modulus_width)?;
    builder.push(&vec![0; modulus_width]);
    push_usize(builder, matrices.row_count())?;
    push_usize(builder, matrices.column_count())?;

    let mut encoding = Vec::new();
    for (matrix_name, label, matrix) in [
        ("A", b'A', matrices.a()),
        ("B", b'B', matrices.b()),
        ("C", b'C', matrices.c()),
    ] {
        let rows = CanonicalRows::new(matrix);
        builder.push(&[label]);
        for (row_index, row) in rows.rows().enumerate() {
            push_usize(builder, row.len())?;
            for (column, coefficient) in row {
                if coefficient.is_zero() {
                    return Err(SpartanMatrixError::ExplicitZeroCoefficient {
                        matrix: matrix_name,
                        row: row_index,
                        column: *column,
                    });
                }
                push_usize(builder, *column)?;
                encoding.clear();
                coefficient.write_modulus_independent_encoding(&mut encoding);
                push_usize(builder, encoding.len())?;
                builder.push(&encoding);
            }
        }
    }
    Ok(())
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
        crypto_bigint_monty::F128, crypto_bigint_uint::Uint, FromWithConfig, PrimeField,
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
        let column = matrix.column(0).unwrap();
        assert_eq!(column.row_indices(), &[0, 2]);
        assert_eq!(column.coefficients(), &[two, five]);
        assert!(matrix.column(1).unwrap().is_empty());
        let column = matrix.column(2).unwrap();
        assert_eq!(column.row_indices(), &[2]);
        assert_eq!(column.coefficients(), &[seven]);
        let column = matrix.column(3).unwrap();
        assert_eq!(column.row_indices(), &[0]);
        assert_eq!(column.coefficients(), &[three]);
        assert!(matrix.column(4).unwrap().is_empty());
        assert!(matrix.column(5).is_none());
        assert!(matrix.column(usize::MAX).is_none());
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
        let from_csc = SparseMatrix::try_from_csc(
            rows.len(),
            vec![0, 2, 2, 3, 4, 4],
            vec![
                (0, field(2, &config)),
                (2, field(5, &config)),
                (2, field(7, &config)),
                (0, field(3, &config)),
            ],
        )
        .unwrap();

        assert_eq!(from_rows, from_columns);
        assert_eq!(from_rows, from_csc);
    }

    #[test]
    fn constructors_reject_noncanonical_or_out_of_bounds_coordinates() {
        let config = config();
        let value = || field(1, &config);

        assert_eq!(
            SparseMatrix::try_from_rows(2, vec![vec![(1, value()), (1, value())]]),
            Err(SparseMatrixError::ColumnsNotStrictlyIncreasing {
                row: 0,
                previous: 1,
                column: 1,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_rows(2, vec![vec![(2, value())]]),
            Err(SparseMatrixError::ColumnOutOfBounds {
                row: 0,
                column: 2,
                columns: 2,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_columns(2, vec![vec![(1, value()), (0, value())], vec![]]),
            Err(SparseMatrixError::RowsNotStrictlyIncreasing {
                column: 0,
                previous: 1,
                row: 0,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_columns(2, vec![vec![], vec![(2, value())]]),
            Err(SparseMatrixError::RowOutOfBounds {
                column: 1,
                row: 2,
                rows: 2,
            })
        );
        assert_eq!(
            SparseMatrix::<F128>::try_from_csc(2, vec![], vec![]),
            Err(SparseMatrixError::InvalidCscOffsets)
        );
        assert_eq!(
            SparseMatrix::<F128>::try_from_csc(2, vec![1], vec![]),
            Err(SparseMatrixError::InvalidCscOffsets)
        );
        assert_eq!(
            SparseMatrix::try_from_csc(2, vec![0, 2], vec![(0, value())]),
            Err(SparseMatrixError::InvalidCscOffsets)
        );
        assert_eq!(
            SparseMatrix::<F128>::try_from_csc(2, vec![0, 1, 0], vec![]),
            Err(SparseMatrixError::InvalidCscOffsets)
        );
        assert_eq!(
            SparseMatrix::try_from_csc(2, vec![0, 2], vec![(1, value()), (0, value())]),
            Err(SparseMatrixError::RowsNotStrictlyIncreasing {
                column: 0,
                previous: 1,
                row: 0,
            })
        );
        assert_eq!(
            SparseMatrix::try_from_csc(2, vec![0, 1], vec![(2, value())]),
            Err(SparseMatrixError::RowOutOfBounds {
                column: 0,
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

    #[cfg(feature = "parallel")]
    #[test]
    fn prover_equality_table_matches_sequential_at_parallel_threshold() {
        let config = config();
        let point: Vec<_> = (0..14)
            .map(|coordinate| field((coordinate + 2) as u64, &config))
            .collect();
        let expected = eq_table(&point, &config).unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();

        let actual = pool.install(|| eq_table_prover(&point, &config)).unwrap();

        assert_eq!(actual, expected);
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
    fn explicit_equality_row_weights_match_point_based_matrix_operations() {
        let config = config();
        let a = SparseMatrix::try_from_rows(
            3,
            vec![
                vec![(0, field(2, &config)), (2, field(3, &config))],
                vec![(1, field(5, &config))],
                vec![(0, field(7, &config))],
            ],
        )
        .unwrap();
        let b = SparseMatrix::try_from_rows(
            3,
            vec![
                vec![(1, field(11, &config))],
                vec![(2, field(13, &config))],
                vec![],
            ],
        )
        .unwrap();
        let c = SparseMatrix::try_from_rows(
            3,
            vec![
                vec![(0, field(17, &config))],
                vec![],
                vec![(2, field(19, &config))],
            ],
        )
        .unwrap();
        let prepared =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();
        let row_point = [field(23, &config), field(29, &config)];
        let row_weights = eq_table(&row_point, &config).unwrap();
        let column_point = [field(31, &config), field(37, &config)];
        let rho = field(41, &config);

        assert_eq!(
            prepared.bind_and_batch(&row_point, &rho).unwrap(),
            prepared
                .bind_and_batch_with_row_weights(&row_weights, &rho)
                .unwrap()
        );
        assert_eq!(
            prepared
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap(),
            prepared
                .evaluate_batched_with_row_weights(&row_weights, &rho, &column_point)
                .unwrap()
        );
    }

    fn from_skeleton_and_new_agree_semantically(
        matrices: &ConstraintMatrices<bool>,
        config: &<F128 as PrimeField>::Config,
    ) {
        let skeleton =
            ConstraintMatricesSkeleton::<F128, bool>::new(matrices.clone()).unwrap();
        let from_skeleton =
            PreparedConstraintMatrices::<F128, bool>::from_skeleton(&skeleton, config).unwrap();
        let from_new =
            PreparedConstraintMatrices::<F128, bool>::new(matrices.clone(), config).unwrap();

        assert_eq!(
            from_skeleton.digest(),
            from_new.digest(),
            "the skeleton replay must reproduce the entry-wise digest exactly"
        );
        assert_eq!(from_skeleton.matrices(), from_new.matrices());
        assert_eq!(from_skeleton.num_row_vars(), from_new.num_row_vars());
        assert_eq!(from_skeleton.num_column_vars(), from_new.num_column_vars());
        assert_eq!(
            from_skeleton.field_modulus_encoding(),
            from_new.field_modulus_encoding()
        );
        // The two constructors detect the identical selector layout: for
        // Boolean coefficients `is_unit` and the encoding comparison agree
        // under every valid configuration.
        match (from_skeleton.selector_triplet, from_new.selector_triplet) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.rows, b.rows);
                assert_eq!(a.a_offset, b.a_offset);
                assert_eq!(a.b_offset, b.b_offset);
                assert_eq!(a.c_offset, b.c_offset);
            }
            (a, b) => panic!("selector detection diverged: {a:?} vs {b:?}"),
        }

        let rho = field(47, config);
        let row_point: Vec<F128> = (0..from_new.num_row_vars())
            .map(|index| field(3 + index as u64, config))
            .collect();
        let column_point: Vec<F128> = (0..from_new.num_column_vars())
            .map(|index| field(29 + index as u64, config))
            .collect();
        assert_eq!(
            from_skeleton.bind_and_batch(&row_point, &rho).unwrap(),
            from_new.bind_and_batch(&row_point, &rho).unwrap()
        );
        assert_eq!(
            from_skeleton
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap(),
            from_new
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap()
        );
    }

    #[test]
    fn skeleton_preparation_matches_direct_preparation() {
        let config = config();
        let rows = 6;
        let columns = 32;
        let selector = |offset: usize| {
            SparseMatrix::try_from_rows(
                columns,
                (0..rows).map(|row| vec![(offset + row, true)]).collect(),
            )
            .unwrap()
        };
        let selectors =
            ConstraintMatrices::new(selector(8), selector(16), selector(24)).unwrap();
        from_skeleton_and_new_agree_semantically(&selectors, &config);

        let generic = |entries: Vec<Vec<(usize, bool)>>| {
            SparseMatrix::try_from_rows(8, entries).unwrap()
        };
        let irregular = ConstraintMatrices::new(
            generic(vec![vec![(0, true), (7, true)], vec![(3, true)]]),
            generic(vec![vec![(1, true)], vec![(2, true), (5, true)]]),
            generic(vec![vec![(4, true)], vec![]]),
        )
        .unwrap();
        from_skeleton_and_new_agree_semantically(&irregular, &config);
    }

    #[test]
    fn skeleton_replay_reproduces_direct_digest_per_modulus() {
        let config = config();
        let other_config =
            F128::make_cfg(&Uint::from(OTHER_TEST_MODULUS)).expect("odd test modulus");
        let matrix = |column: usize| {
            SparseMatrix::try_from_rows(8, vec![vec![(column, true)], vec![(column + 1, true)]])
                .unwrap()
        };
        let matrices = ConstraintMatrices::new(matrix(0), matrix(1), matrix(2)).unwrap();
        let skeleton =
            ConstraintMatricesSkeleton::<F128, bool>::new(matrices.clone()).unwrap();

        for field_config in [&config, &other_config] {
            let replayed =
                PreparedConstraintMatrices::<F128, bool>::from_skeleton(&skeleton, field_config)
                    .unwrap();
            let direct =
                PreparedConstraintMatrices::<F128, bool>::new(matrices.clone(), field_config)
                    .unwrap();
            assert_eq!(replayed.digest(), direct.digest());
        }

        // The digest still separates moduli and topologies.
        let at_config =
            PreparedConstraintMatrices::<F128, bool>::from_skeleton(&skeleton, &config).unwrap();
        let at_other =
            PreparedConstraintMatrices::<F128, bool>::from_skeleton(&skeleton, &other_config)
                .unwrap();
        assert_ne!(at_config.digest(), at_other.digest());

        let moved = ConstraintMatrices::new(matrix(0), matrix(1), matrix(3)).unwrap();
        let moved_skeleton = ConstraintMatricesSkeleton::<F128, bool>::new(moved).unwrap();
        let moved_prepared =
            PreparedConstraintMatrices::<F128, bool>::from_skeleton(&moved_skeleton, &config)
                .unwrap();
        assert_ne!(at_config.digest(), moved_prepared.digest());
    }

    #[test]
    fn skeleton_rejects_explicit_zero_with_direct_constructor_coordinates() {
        let config = config();
        let with_zero = || {
            let unit = SparseMatrix::try_from_rows(4, vec![vec![(0, true)], vec![(1, true)]])
                .unwrap();
            let zeroed = SparseMatrix::try_from_rows(
                4,
                vec![vec![(0, true)], vec![(2, false), (3, true)]],
            )
            .unwrap();
            ConstraintMatrices::new(unit.clone(), zeroed, unit).unwrap()
        };
        let skeleton_error =
            ConstraintMatricesSkeleton::<F128, bool>::new(with_zero()).unwrap_err();
        let direct_error =
            PreparedConstraintMatrices::<F128, bool>::new(with_zero(), &config).unwrap_err();
        assert_eq!(
            skeleton_error,
            SpartanMatrixError::ExplicitZeroCoefficient {
                matrix: "B",
                row: 1,
                column: 2,
            }
        );
        assert_eq!(skeleton_error, direct_error);
    }

    #[test]
    fn prefix_univariate_factors_stream_disjoint_selectors_exactly() {
        let config = config();
        let rows = 6;
        let columns = 32;
        let selector = |offset: usize| {
            SparseMatrix::try_from_rows(
                columns,
                (0..rows).map(|row| vec![(offset + row, true)]).collect(),
            )
            .unwrap()
        };
        let prepared = PreparedConstraintMatrices::<F128, bool>::new(
            ConstraintMatrices::new(selector(8), selector(16), selector(24)).unwrap(),
            &config,
        )
        .unwrap();
        assert!(prepared.selector_triplet.is_some());

        let tail_point = [field(43, &config)];
        let (tail_low, tail_high) = make_equality_factors(&tail_point, &config).unwrap();
        let factors = PrefixUnivariateRowFactors::new(
            2,
            [2, 3, 5, 7]
                .into_iter()
                .map(|value| field(value, &config))
                .collect(),
            tail_low,
            tail_high,
            prepared.num_row_vars(),
        )
        .unwrap();
        let row_weights = factors.materialize();
        let rho = field(47, &config);
        let column_point = [
            field(53, &config),
            field(59, &config),
            field(61, &config),
            field(67, &config),
            field(71, &config),
        ];

        assert_eq!(
            prepared
                .bind_and_batch_with_prefix_univariate_factors(&factors, &rho)
                .unwrap(),
            prepared
                .bind_and_batch_with_validated_row_weights(&row_weights, &rho)
                .unwrap()
        );
        assert_eq!(
            prepared
                .evaluate_batched_with_prefix_univariate_factors(&factors, &rho, &column_point,)
                .unwrap(),
            prepared
                .evaluate_batched_with_validated_row_weights(&row_weights, &rho, &column_point,)
                .unwrap()
        );
    }

    #[test]
    fn prefix_univariate_factors_preserve_generic_matrix_fallback() {
        let config = config();
        let matrix = || {
            SparseMatrix::try_from_rows(
                8,
                vec![
                    vec![(0, field(2, &config)), (7, field(3, &config))],
                    vec![(2, field(5, &config))],
                    vec![(4, field(7, &config))],
                    vec![],
                ],
            )
            .unwrap()
        };
        let prepared = PreparedConstraintMatrices::new(
            ConstraintMatrices::new(matrix(), matrix(), matrix()).unwrap(),
            &config,
        )
        .unwrap();
        assert!(prepared.selector_triplet.is_none());

        let tail_point = [field(11, &config)];
        let (tail_low, tail_high) = make_equality_factors(&tail_point, &config).unwrap();
        let factors = PrefixUnivariateRowFactors::new(
            1,
            vec![field(13, &config), field(17, &config)],
            tail_low,
            tail_high,
            prepared.num_row_vars(),
        )
        .unwrap();
        let row_weights = factors.materialize();
        let rho = field(19, &config);
        let column_point = [field(23, &config), field(29, &config), field(31, &config)];

        assert_eq!(
            prepared
                .bind_and_batch_with_prefix_univariate_factors(&factors, &rho)
                .unwrap(),
            prepared
                .bind_and_batch_with_validated_row_weights(&row_weights, &rho)
                .unwrap()
        );
        assert_eq!(
            prepared
                .evaluate_batched_with_prefix_univariate_factors(&factors, &rho, &column_point,)
                .unwrap(),
            prepared
                .evaluate_batched_with_validated_row_weights(&row_weights, &rho, &column_point,)
                .unwrap()
        );
    }

    #[test]
    fn explicit_row_weight_operations_validate_length_and_field() {
        let config = config();
        let other_config = F128::make_cfg(&Uint::from(OTHER_TEST_MODULUS)).unwrap();
        let matrix = || {
            SparseMatrix::try_from_rows(1, vec![vec![(0, field(1, &config))], vec![], vec![]])
                .unwrap()
        };
        let prepared = PreparedConstraintMatrices::new(
            ConstraintMatrices::new(matrix(), matrix(), matrix()).unwrap(),
            &config,
        )
        .unwrap();
        let rho = field(2, &config);
        let column_point: [F128; 0] = [];
        let short_weights = vec![field(3, &config); 3];

        assert_eq!(
            prepared.bind_and_batch_with_row_weights(&short_weights, &rho),
            Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: 4,
                actual: 3,
            })
        );
        assert_eq!(
            prepared.evaluate_batched_with_row_weights(&short_weights, &rho, &column_point,),
            Err(SpartanMatrixError::InvalidRowWeightsLength {
                expected: 4,
                actual: 3,
            })
        );

        let foreign_weights = vec![field(3, &other_config); 4];
        assert_eq!(
            prepared.bind_and_batch_with_row_weights(&foreign_weights, &rho),
            Err(SpartanMatrixError::FieldConfigurationMismatch)
        );
        assert_eq!(
            prepared.evaluate_batched_with_row_weights(&foreign_weights, &rho, &column_point,),
            Err(SpartanMatrixError::FieldConfigurationMismatch)
        );
    }

    #[test]
    fn boolean_one_coefficients_match_field_one_statement_and_evaluation() {
        let config = config();
        let one = F128::one_with_cfg(&config);
        let field_matrix = SparseMatrix::try_from_rows(
            4,
            vec![
                vec![(0, one.clone()), (3, one.clone())],
                vec![(1, one.clone())],
                vec![(2, one)],
            ],
        )
        .unwrap();
        let boolean_matrix = SparseMatrix::try_from_rows(
            4,
            vec![vec![(0, true), (3, true)], vec![(1, true)], vec![(2, true)]],
        )
        .unwrap();
        let field_prepared = PreparedConstraintMatrices::new(
            ConstraintMatrices::new(field_matrix.clone(), field_matrix.clone(), field_matrix)
                .unwrap(),
            &config,
        )
        .unwrap();
        let boolean_prepared = PreparedConstraintMatrices::<F128, bool>::new(
            ConstraintMatrices::new(
                boolean_matrix.clone(),
                boolean_matrix.clone(),
                boolean_matrix,
            )
            .unwrap(),
            &config,
        )
        .unwrap();

        assert_eq!(field_prepared.digest(), boolean_prepared.digest());

        let row_point = [field(7, &config), field(11, &config)];
        let column_point = [field(13, &config), field(17, &config)];
        let rho = field(19, &config);
        assert_eq!(
            field_prepared.bind_and_batch(&row_point, &rho).unwrap(),
            boolean_prepared.bind_and_batch(&row_point, &rho).unwrap()
        );
        assert_eq!(
            field_prepared
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap(),
            boolean_prepared
                .evaluate_batched(&row_point, &rho, &column_point)
                .unwrap()
        );
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn indexed_parallel_binding_is_exact_for_generic_coefficients_at_threshold() {
        let config = config();
        let column_count = (1 << 12) + 1;
        let boolean_rows = vec![
            vec![(0, true), (1 << 12, true)],
            vec![(1, true), (1 << 11, true)],
            vec![(2, true)],
            vec![(3, true), ((1 << 12) - 1, true)],
        ];
        let one = F128::one_with_cfg(&config);
        let field_rows: Vec<Vec<(usize, F128)>> = boolean_rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|(column, _)| (*column, one.clone()))
                    .collect()
            })
            .collect();
        let boolean_matrix =
            || SparseMatrix::try_from_rows(column_count, boolean_rows.clone()).unwrap();
        let field_matrix =
            || SparseMatrix::try_from_rows(column_count, field_rows.clone()).unwrap();
        let boolean_prepared = PreparedConstraintMatrices::<F128, bool>::new(
            ConstraintMatrices::new(boolean_matrix(), boolean_matrix(), boolean_matrix()).unwrap(),
            &config,
        )
        .unwrap();
        let field_prepared = PreparedConstraintMatrices::<F128>::new(
            ConstraintMatrices::new(field_matrix(), field_matrix(), field_matrix()).unwrap(),
            &config,
        )
        .unwrap();
        let row_point = [field(7, &config), field(11, &config)];
        let rho = field(13, &config);
        let sequential_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let parallel_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();

        let sequential_boolean = sequential_pool
            .install(|| boolean_prepared.bind_and_batch(&row_point, &rho))
            .unwrap();
        let parallel_boolean = parallel_pool
            .install(|| boolean_prepared.bind_and_batch(&row_point, &rho))
            .unwrap();
        let sequential_field = sequential_pool
            .install(|| field_prepared.bind_and_batch(&row_point, &rho))
            .unwrap();
        let parallel_field = parallel_pool
            .install(|| field_prepared.bind_and_batch(&row_point, &rho))
            .unwrap();

        assert_eq!(parallel_boolean, sequential_boolean);
        assert_eq!(parallel_field, sequential_field);
        assert_eq!(parallel_boolean, parallel_field);
        assert!(parallel_boolean.evaluations[column_count..]
            .iter()
            .all(|value| <F128 as PrimeField>::is_zero(value)));
    }

    #[test]
    fn prepared_boolean_statement_rejects_explicit_false_coefficients() {
        let config = config();
        let false_matrix = SparseMatrix::try_from_rows(1, vec![vec![(0, false)]]).unwrap();
        let true_matrix = SparseMatrix::try_from_rows(1, vec![vec![(0, true)]]).unwrap();
        let matrices =
            ConstraintMatrices::new(false_matrix, true_matrix.clone(), true_matrix).unwrap();

        assert!(matches!(
            PreparedConstraintMatrices::<F128, bool>::new(matrices, &config),
            Err(SpartanMatrixError::ExplicitZeroCoefficient {
                matrix: "A",
                row: 0,
                column: 0,
            })
        ));
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
