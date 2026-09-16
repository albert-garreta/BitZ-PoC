//! Their `spartan/src/matrix.rs`: the R1CS matrices lowered to [`Fq`]
//! (their `bigint_to_fq`, canonical residues), the column-chunked nonzero
//! index, `bind_and_batch`, `evaluate_batched`, the canonical constraint
//! digest (`constraint_matrix_digest`, SHA-256 over
//! `bitz/spartan/constraint-matrices/v1`), plus the product tables
//! `Az, Bz, Cz` and the assignment table `h` padded to their Boolean domains.
//!
//! `M` is consumed by the digest only — the Spartan PIOP never reads it —
//! so it is hashed while the vendored matrices are lowered and not kept.

use std::mem::MaybeUninit;
use std::sync::Arc;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use super::super::fq::{Fq, modulus};
use super::poly::eq_table;
use super::sumcheck::Products;
use crate::{cfg_chunks_mut, cfg_into_iter};
use circuit::witgen::PackedWitness;

/// Failures while preparing or evaluating the matrices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixError {
    InvalidR1csShape,
    InvalidAssignmentLength { expected: usize, actual: usize },
    InvalidAssignmentConstant,
    InvalidRowPointLength { expected: usize, actual: usize },
    InvalidColumnPointLength { expected: usize, actual: usize },
}

/// One sparse matrix over `Fq`: per row, `(column, coefficient)` with
/// strictly increasing columns. Every row's entries sit in one buffer
/// (`row_starts[i]..row_starts[i + 1]`), so a pass over the rows streams
/// memory whichever thread lowered them — per-row vectors filled in
/// parallel land in per-thread arenas and cost `bind_and_batch` a factor
/// of two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseMatrix {
    entries: Vec<(usize, Fq)>,
    row_starts: Vec<usize>,
    columns: usize,
}

impl SparseMatrix {
    pub fn new(rows: Vec<Vec<(usize, Fq)>>, columns: usize) -> Self {
        let mut entries = Vec::with_capacity(rows.iter().map(Vec::len).sum());
        let mut row_starts = Vec::with_capacity(rows.len() + 1);
        row_starts.push(0);
        for row in rows {
            entries.extend(row);
            row_starts.push(entries.len());
        }
        Self { entries, row_starts, columns }
    }

    /// The flat form: `row_starts` starts at 0, never decreases, ends at
    /// `entries.len()` and has one element more than there are rows.
    pub fn from_flat(entries: Vec<(usize, Fq)>, row_starts: Vec<usize>, columns: usize) -> Self {
        debug_assert_eq!(row_starts.first(), Some(&0));
        debug_assert_eq!(row_starts.last(), Some(&entries.len()));
        debug_assert!(row_starts.windows(2).all(|pair| pair[0] <= pair[1]));
        Self { entries, row_starts, columns }
    }

    pub fn row(&self, row: usize) -> &[(usize, Fq)] {
        &self.entries[self.row_starts[row]..self.row_starts[row + 1]]
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[(usize, Fq)]> + '_ {
        (0..self.row_count()).map(move |row| self.row(row))
    }

    pub fn row_count(&self) -> usize {
        self.row_starts.len() - 1
    }

    pub fn column_count(&self) -> usize {
        self.columns
    }

    pub fn nonzero_count(&self) -> usize {
        self.entries.len()
    }
}

/// `bind_and_batch` splits the column domain into chunks of `2^16` columns
/// and accumulates each chunk on its own thread (their constant).
const BIND_CHUNK_COLUMN_VARS: usize = 16;

/// Row blocks the integer lowering fills in parallel.
const LOWER_BLOCKS: usize = 64;

/// The entries `[start, end)` of row `row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowSpan {
    row: usize,
    start: usize,
    end: usize,
}

/// The nonzeros of one matrix grouped by column chunk, in row order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnChunkIndex {
    chunk_len: usize,
    spans: Vec<Vec<RowSpan>>,
}

impl ColumnChunkIndex {
    fn new(matrix: &SparseMatrix, chunk_len: usize, chunk_count: usize) -> Self {
        debug_assert!(chunk_len * chunk_count >= matrix.column_count());
        Self::build(matrix.row_count(), chunk_len, chunk_count, |row| matrix.row(row), |entry| entry.0)
    }

    /// From any row-major entry layout: the index depends on the columns
    /// alone, so the integer matrices build it once for every modulus.
    fn build<'a, T: 'a>(
        row_count: usize,
        chunk_len: usize,
        chunk_count: usize,
        row_entries: impl Fn(usize) -> &'a [T],
        column: impl Fn(&T) -> usize,
    ) -> Self {
        debug_assert!(chunk_len.is_power_of_two());
        let mut spans = vec![Vec::new(); chunk_count];
        for row in 0..row_count {
            let entries = row_entries(row);
            let mut start = 0;
            while start < entries.len() {
                // Columns increase within a row, so a chunk's entries are a
                // contiguous run.
                let chunk = column(&entries[start]) / chunk_len;
                let end =
                    start + entries[start..].partition_point(|entry| column(entry) / chunk_len == chunk);
                spans[chunk].push(RowSpan { row, start, end });
                start = end;
            }
        }
        Self { chunk_len, spans }
    }
}

/// The chunk geometry for a column count (their constant, capped at the
/// padded domain).
fn chunk_geometry(num_column_vars: usize) -> (usize, usize) {
    let num_columns = 1usize << num_column_vars;
    let chunk_len = num_columns.min(1usize << BIND_CHUNK_COLUMN_VARS);
    (chunk_len, num_columns / chunk_len)
}

/// Their canonical statement digest, streamed.
struct DigestBuilder(Sha256);

impl DigestBuilder {
    fn new() -> Self {
        let mut hash = Sha256::new();
        hash.update(b"bitz/spartan/constraint-matrices/v1");
        Self(hash)
    }

    fn usize(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    fn m_header(&mut self, rows: usize, columns: usize) {
        self.0.update(b"M");
        self.usize(rows);
        self.usize(columns);
    }

    fn m_row(&mut self, positions: &[usize]) {
        self.usize(positions.len());
        for &column in positions {
            self.usize(column);
        }
    }

    fn matrix_header(&mut self, label: &[u8], rows: usize, columns: usize) {
        self.0.update(label);
        self.usize(rows);
        self.usize(columns);
    }

    fn entry_count(&mut self, count: usize) {
        self.usize(count);
    }

    fn entry(&mut self, column: usize, coefficient: Fq) {
        self.usize(column);
        let bytes = coefficient.to_bytes();
        self.usize(bytes.len());
        self.0.update(bytes);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// Their `bigint_to_fq`: the canonical residue of a signed integer under
/// the installed modulus.
pub fn residue(value: &BigInt) -> Fq {
    let q = modulus();
    if let Some(narrow) = value.to_i128() {
        return Fq::new(narrow.rem_euclid(q as i128) as u128);
    }
    let modulus = BigInt::from(q);
    let mut reduced = value % &modulus;
    if reduced.is_negative() {
        reduced += modulus;
    }
    Fq::new(
        reduced
            .to_u128()
            .expect("a canonical residue always fits a u128"),
    )
}

/// Their `PreparedConstraintMatrices<Fq>`: shape validation, the Boolean
/// domain sizes, the digest and the column chunking, done once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedConstraintMatrices {
    a: SparseMatrix,
    b: SparseMatrix,
    c: SparseMatrix,
    m_rows: usize,
    m_columns: usize,
    column_chunks: Arc<[ColumnChunkIndex; 3]>,
    digest: [u8; 32],
    num_row_vars: usize,
    num_column_vars: usize,
}

fn padded_num_vars(logical_len: usize) -> usize {
    logical_len.max(1).next_power_of_two().ilog2() as usize
}

impl PreparedConstraintMatrices {
    /// Lowers the vendored integer matrices (their `ConstraintGenerator`'s
    /// output) to `Fq`, hashing `M`, `A`, `B`, `C` on the way through.
    pub fn from_vendored(
        matrices: &circuit::constraints::ConstraintMatrices,
    ) -> Result<Self, MatrixError> {
        let (m, a, b, c) = (&matrices.m, &matrices.a, &matrices.b, &matrices.c);
        let rows = a.row_count();
        let columns = a.column_count();
        if b.row_count() != rows
            || c.row_count() != rows
            || b.column_count() != columns
            || c.column_count() != columns
            || columns != m.row_count()
        {
            return Err(MatrixError::InvalidR1csShape);
        }
        let mut digest = DigestBuilder::new();
        digest.m_header(m.row_count(), m.column_count());
        for row in m.rows() {
            digest.m_row(row.positions());
        }
        let mut lower = |label: &[u8], matrix: &circuit::constraints::SparseMatrix<BigInt>| {
            digest.matrix_header(label, matrix.row_count(), matrix.column_count());
            let mut entries =
                Vec::with_capacity(matrix.rows().iter().map(|row| row.entries().len()).sum());
            let mut row_starts = Vec::with_capacity(matrix.row_count() + 1);
            row_starts.push(0);
            for row in matrix.rows() {
                digest.entry_count(row.entries().len());
                for (column, coefficient) in row.entries() {
                    let coefficient = residue(coefficient);
                    digest.entry(*column, coefficient);
                    entries.push((*column, coefficient));
                }
                row_starts.push(entries.len());
            }
            SparseMatrix::from_flat(entries, row_starts, matrix.column_count())
        };
        let a = lower(b"A", a);
        let b = lower(b"B", b);
        let c = lower(b"C", c);
        Self::prepare(a, b, c, m.row_count(), m.column_count(), digest.finish())
    }

    /// The same preparation from `Fq` rows and `M`'s positions (tests and
    /// fixtures); the digest is the one `from_vendored` computes.
    pub fn from_parts(
        m_rows: &[Vec<usize>],
        m_columns: usize,
        a: SparseMatrix,
        b: SparseMatrix,
        c: SparseMatrix,
    ) -> Result<Self, MatrixError> {
        let rows = a.row_count();
        let columns = a.column_count();
        if b.row_count() != rows
            || c.row_count() != rows
            || b.column_count() != columns
            || c.column_count() != columns
            || columns != m_rows.len()
        {
            return Err(MatrixError::InvalidR1csShape);
        }
        let mut digest = DigestBuilder::new();
        digest.m_header(m_rows.len(), m_columns);
        for row in m_rows {
            digest.m_row(row);
        }
        for (label, matrix) in [(b"A", &a), (b"B", &b), (b"C", &c)] {
            digest.matrix_header(label, matrix.row_count(), matrix.column_count());
            for row in matrix.rows() {
                digest.entry_count(row.len());
                for &(column, coefficient) in row {
                    digest.entry(column, coefficient);
                }
            }
        }
        Self::prepare(a, b, c, m_rows.len(), m_columns, digest.finish())
    }

    fn prepare(
        a: SparseMatrix,
        b: SparseMatrix,
        c: SparseMatrix,
        m_rows: usize,
        m_columns: usize,
        digest: [u8; 32],
    ) -> Result<Self, MatrixError> {
        let num_column_vars = padded_num_vars(a.column_count());
        let (chunk_len, chunk_count) = chunk_geometry(num_column_vars);
        let column_chunks =
            Arc::new([&a, &b, &c].map(|matrix| ColumnChunkIndex::new(matrix, chunk_len, chunk_count)));
        Self::prepare_indexed(a, b, c, m_rows, m_columns, digest, column_chunks)
    }

    /// [`Self::prepare`] with the column chunk index already built (the
    /// integer matrices share theirs across every lowering).
    fn prepare_indexed(
        a: SparseMatrix,
        b: SparseMatrix,
        c: SparseMatrix,
        m_rows: usize,
        m_columns: usize,
        digest: [u8; 32],
        column_chunks: Arc<[ColumnChunkIndex; 3]>,
    ) -> Result<Self, MatrixError> {
        let num_row_vars = padded_num_vars(a.row_count());
        let num_column_vars = padded_num_vars(a.column_count());
        debug_assert_eq!(column_chunks[0].chunk_len, chunk_geometry(num_column_vars).0);
        Ok(Self {
            a,
            b,
            c,
            m_rows,
            m_columns,
            column_chunks,
            digest,
            num_row_vars,
            num_column_vars,
        })
    }

    /// The entry buffers, for the next lowering to fill in place
    /// (their pages stay mapped: a fresh 800 MB per proof faults in under
    /// the kernel's map lock and every parallel pass after it runs slower).
    pub fn into_scratch(self) -> LoweredScratch {
        LoweredScratch {
            entries: [self.a.entries, self.b.entries, self.c.entries],
        }
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn num_row_vars(&self) -> usize {
        self.num_row_vars
    }

    pub fn num_column_vars(&self) -> usize {
        self.num_column_vars
    }

    /// The R1CS row count (`A`'s rows).
    pub fn row_count(&self) -> usize {
        self.a.row_count()
    }

    /// The assignment length `h_len` (`A`'s columns = `M`'s rows).
    pub fn column_count(&self) -> usize {
        self.a.column_count()
    }

    /// `M`'s dimensions, `(h_len, f_len)`.
    pub fn m_dimensions(&self) -> (usize, usize) {
        (self.m_rows, self.m_columns)
    }

    pub fn a(&self) -> &SparseMatrix {
        &self.a
    }

    pub fn b(&self) -> &SparseMatrix {
        &self.b
    }

    pub fn c(&self) -> &SparseMatrix {
        &self.c
    }

    /// Their `short_debug_info`.
    pub fn short_debug_info(&self) -> String {
        format!(
            "{} r1cs rows -> 2^{}, {} h entries -> 2^{}, {} nonzeros",
            self.row_count(),
            self.num_row_vars,
            self.column_count(),
            self.num_column_vars,
            self.a.nonzero_count() + self.b.nonzero_count() + self.c.nonzero_count()
        )
    }

    /// `Az, Bz, Cz` over `Fq` from the 0/1 assignment `h`, each padded with
    /// zeros to `2^num_row_vars` (their `build_product_mles`, computed from
    /// the sparse rows: `h` is Boolean, so a row's value is the sum of the
    /// coefficient residues at its set bits — the same residues).
    pub fn products(&self, assignment: &PackedWitness) -> Result<Products, MatrixError> {
        if assignment.bit_len() != self.column_count() {
            return Err(MatrixError::InvalidAssignmentLength {
                expected: self.column_count(),
                actual: assignment.bit_len(),
            });
        }
        let mut products = Products {
            az: Vec::new(),
            bz: Vec::new(),
            cz: Vec::new(),
        };
        self.products_into(assignment, &mut products)?;
        Ok(products)
    }

    /// [`Self::products`] into buffers kept from an earlier call (resized,
    /// never reallocated once large enough).
    pub fn products_into(
        &self,
        assignment: &PackedWitness,
        products: &mut Products,
    ) -> Result<(), MatrixError> {
        if assignment.bit_len() != self.column_count() {
            return Err(MatrixError::InvalidAssignmentLength {
                expected: self.column_count(),
                actual: assignment.bit_len(),
            });
        }
        let padded_len = 1usize << self.num_row_vars;
        let evaluate = |matrix: &SparseMatrix, values: &mut Vec<Fq>| {
            values.clear();
            values.resize(padded_len, Fq::ZERO);
            let rows = matrix.row_count();
            cfg_chunks_mut!(values[..rows], 256)
                .enumerate()
                .for_each(|(chunk, values)| {
                    for (offset, value) in values.iter_mut().enumerate() {
                        *value = matrix
                            .row(chunk * 256 + offset)
                            .iter()
                            .filter(|(column, _)| assignment.bit(*column))
                            .map(|(_, coefficient)| *coefficient)
                            .sum();
                    }
                });
        };
        evaluate(&self.a, &mut products.az);
        evaluate(&self.b, &mut products.bz);
        evaluate(&self.c, &mut products.cz);
        Ok(())
    }

    /// The assignment `h = M(1 ‖ f)` as `Fq` values padded with zeros to
    /// `2^num_column_vars` (their `build_assignment_mle`); bit 0 must be the
    /// constant one.
    pub fn assignment(&self, assignment: &PackedWitness) -> Result<Vec<Fq>, MatrixError> {
        if assignment.bit_len() != self.column_count() {
            return Err(MatrixError::InvalidAssignmentLength {
                expected: self.column_count(),
                actual: assignment.bit_len(),
            });
        }
        let mut values = Vec::new();
        self.assignment_into(assignment, &mut values)?;
        Ok(values)
    }

    /// [`Self::assignment`] into a buffer kept from an earlier call, filled
    /// in parallel.
    pub fn assignment_into(
        &self,
        assignment: &PackedWitness,
        values: &mut Vec<Fq>,
    ) -> Result<(), MatrixError> {
        if assignment.bit_len() != self.column_count() {
            return Err(MatrixError::InvalidAssignmentLength {
                expected: self.column_count(),
                actual: assignment.bit_len(),
            });
        }
        if self.column_count() == 0 || !assignment.bit(0) {
            return Err(MatrixError::InvalidAssignmentConstant);
        }
        values.clear();
        values.resize(1usize << self.num_column_vars, Fq::ZERO);
        let bits = assignment.bit_len();
        cfg_chunks_mut!(values[..bits], 1 << 14)
            .enumerate()
            .for_each(|(chunk, values)| {
                let base = chunk << 14;
                for (offset, value) in values.iter_mut().enumerate() {
                    *value = Fq::from(assignment.bit(base + offset));
                }
            });
        Ok(())
    }

    /// `D(j) = Σ_i eq(i, r_x) (A + ρ B + ρ² C)[i, j]` as a dense table over
    /// the column domain, one column chunk per task (their `bind_and_batch`).
    pub fn bind_and_batch(&self, row_point: &[Fq], rho: Fq) -> Result<Vec<Fq>, MatrixError> {
        if row_point.len() != self.num_row_vars {
            return Err(MatrixError::InvalidRowPointLength {
                expected: self.num_row_vars,
                actual: row_point.len(),
            });
        }
        let row_weights = eq_table(row_point);
        let batched = [
            (&self.a, &self.column_chunks[0], Fq::ONE),
            (&self.b, &self.column_chunks[1], rho),
            (&self.c, &self.column_chunks[2], rho * rho),
        ];
        let chunk_len = self.column_chunks[0].chunk_len;
        let mut evaluations = vec![Fq::ZERO; 1usize << self.num_column_vars];
        cfg_chunks_mut!(evaluations, chunk_len)
            .enumerate()
            .for_each(|(chunk, output)| {
                let base = chunk * chunk_len;
                for (matrix, index, batch_scale) in batched {
                    for span in &index.spans[chunk] {
                        let row_scale = row_weights[span.row] * batch_scale;
                        for &(column, coefficient) in &matrix.row(span.row)[span.start..span.end] {
                            output[column - base] += row_scale * coefficient;
                        }
                    }
                }
            });
        Ok(evaluations)
    }

    /// `D(r_y) = A(r_x, r_y) + ρ B(r_x, r_y) + ρ² C(r_x, r_y)` directly over
    /// the nonzeros (their `evaluate_batched`).
    pub fn evaluate_batched(
        &self,
        row_point: &[Fq],
        rho: Fq,
        column_point: &[Fq],
    ) -> Result<Fq, MatrixError> {
        if row_point.len() != self.num_row_vars {
            return Err(MatrixError::InvalidRowPointLength {
                expected: self.num_row_vars,
                actual: row_point.len(),
            });
        }
        if column_point.len() != self.num_column_vars {
            return Err(MatrixError::InvalidColumnPointLength {
                expected: self.num_column_vars,
                actual: column_point.len(),
            });
        }
        let row_weights = eq_table(row_point);
        let batched = [
            (&self.a, &self.column_chunks[0], Fq::ONE),
            (&self.b, &self.column_chunks[1], rho),
            (&self.c, &self.column_chunks[2], rho * rho),
        ];
        let chunk_len = self.column_chunks[0].chunk_len;
        let (low_point, high_point) = column_point.split_at(chunk_len.ilog2() as usize);
        let low_weights = eq_table(low_point);
        let high_weights = eq_table(high_point);
        debug_assert_eq!(high_weights.len(), self.column_chunks[0].spans.len());
        let partials: Vec<Fq> = cfg_into_iter!(0..high_weights.len())
            .map(|chunk| {
                let base = chunk * chunk_len;
                let mut chunk_sum = Fq::ZERO;
                for (matrix, index, batch_scale) in batched {
                    let mut matrix_sum = Fq::ZERO;
                    for span in &index.spans[chunk] {
                        let span_sum = matrix.row(span.row)[span.start..span.end]
                            .iter()
                            .fold(Fq::ZERO, |sum, &(column, coefficient)| {
                                sum + low_weights[column - base] * coefficient
                            });
                        matrix_sum += row_weights[span.row] * span_sum;
                    }
                    chunk_sum += batch_scale * matrix_sum;
                }
                high_weights[chunk] * chunk_sum
            })
            .collect();
        Ok(partials.into_iter().sum())
    }
}

/// Their `IntegerCoefficient`: an R1CS coefficient kept as the integer the
/// generator produced — signed powers of two (the bit lifts) as their shift,
/// so lowering them under any modulus is a table lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerCoefficient {
    PowerOfTwo { shift: u16, negative: bool },
    Small(i64),
    Big(Box<BigInt>),
}

impl IntegerCoefficient {
    pub fn new(value: &BigInt) -> Self {
        let magnitude = value.magnitude();
        if magnitude.count_ones() == 1 {
            if let Some(shift) = magnitude.trailing_zeros().and_then(|s| u16::try_from(s).ok()) {
                return Self::PowerOfTwo {
                    shift,
                    negative: value.is_negative(),
                };
            }
        }
        match value.to_i64() {
            Some(small) => Self::Small(small),
            None => Self::Big(Box::new(value.clone())),
        }
    }

    fn max_shift(&self) -> u16 {
        match self {
            Self::PowerOfTwo { shift, .. } => *shift,
            _ => 0,
        }
    }

    /// The residue under the installed modulus; `pow2[i] = 2^i` there.
    fn lower(&self, pow2: &[Fq]) -> Fq {
        match self {
            Self::PowerOfTwo { shift, negative } => {
                let value = pow2[usize::from(*shift)];
                if *negative { -value } else { value }
            }
            Self::Small(small) => Fq::new(i128::from(*small).rem_euclid(modulus() as i128) as u128),
            Self::Big(big) => residue(big),
        }
    }
}

/// Their `PreparedIntegerMatrices`: the R1CS over the integers with a
/// digest no prime enters (`bitz/spartan/integer-constraint-matrices/v1`:
/// `M` as in the residue digest, then every entry of `A`, `B`, `C` as its
/// column, a sign byte, and the magnitude's little-endian bytes with their
/// length), lowered under the installed modulus per proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedIntegerMatrices {
    a: Vec<Vec<(usize, IntegerCoefficient)>>,
    b: Vec<Vec<(usize, IntegerCoefficient)>>,
    c: Vec<Vec<(usize, IntegerCoefficient)>>,
    /// Row starts of the flat lowered form, per matrix (the prefix sums of
    /// the row lengths): the same under every modulus.
    row_starts: [Vec<usize>; 3],
    /// The column chunk index, the same under every modulus.
    column_chunks: Arc<[ColumnChunkIndex; 3]>,
    columns: usize,
    m_rows: usize,
    m_columns: usize,
    digest: [u8; 32],
}

/// The entry buffers of a lowered [`PreparedConstraintMatrices`], kept
/// between lowerings ([`PreparedConstraintMatrices::into_scratch`],
/// [`PreparedIntegerMatrices::lower_into`]).
#[derive(Debug, Default)]
pub struct LoweredScratch {
    entries: [Vec<(usize, Fq)>; 3],
}

impl PreparedIntegerMatrices {
    pub fn from_vendored(
        matrices: &circuit::constraints::ConstraintMatrices,
    ) -> Result<Self, MatrixError> {
        let (m, a, b, c) = (&matrices.m, &matrices.a, &matrices.b, &matrices.c);
        let rows = a.row_count();
        let columns = a.column_count();
        if b.row_count() != rows
            || c.row_count() != rows
            || b.column_count() != columns
            || c.column_count() != columns
            || columns != m.row_count()
        {
            return Err(MatrixError::InvalidR1csShape);
        }
        let mut hash = Sha256::new();
        hash.update(b"bitz/spartan/integer-constraint-matrices/v1");
        let put = |hash: &mut Sha256, value: usize| hash.update((value as u64).to_le_bytes());
        hash.update(b"M");
        put(&mut hash, m.row_count());
        put(&mut hash, m.column_count());
        for row in m.rows() {
            put(&mut hash, row.positions().len());
            for &column in row.positions() {
                put(&mut hash, column);
            }
        }
        let mut keep = |label: &[u8], matrix: &circuit::constraints::SparseMatrix<BigInt>| {
            hash.update(label);
            put(&mut hash, matrix.row_count());
            put(&mut hash, matrix.column_count());
            matrix
                .rows()
                .iter()
                .map(|row| {
                    put(&mut hash, row.entries().len());
                    row.entries()
                        .iter()
                        .map(|(column, coefficient)| {
                            put(&mut hash, *column);
                            hash.update([u8::from(coefficient.is_negative())]);
                            let magnitude = coefficient.magnitude().to_bytes_le();
                            put(&mut hash, magnitude.len());
                            hash.update(&magnitude);
                            (*column, IntegerCoefficient::new(coefficient))
                        })
                        .collect()
                })
                .collect::<Vec<Vec<(usize, IntegerCoefficient)>>>()
        };
        let a = keep(b"A", a);
        let b = keep(b"B", b);
        let c = keep(b"C", c);
        let row_starts = [&a, &b, &c].map(|matrix| {
            let mut starts = Vec::with_capacity(matrix.len() + 1);
            starts.push(0usize);
            for row in matrix {
                starts.push(starts[starts.len() - 1] + row.len());
            }
            starts
        });
        let (chunk_len, chunk_count) = chunk_geometry(padded_num_vars(columns));
        let column_chunks = Arc::new([&a, &b, &c].map(|matrix| {
            ColumnChunkIndex::build(
                matrix.len(),
                chunk_len,
                chunk_count,
                |row| matrix[row].as_slice(),
                |entry| entry.0,
            )
        }));
        Ok(Self {
            a,
            b,
            c,
            row_starts,
            column_chunks,
            columns,
            m_rows: m.row_count(),
            m_columns: m.column_count(),
            digest: hash.finalize().into(),
        })
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn row_count(&self) -> usize {
        self.a.len()
    }

    pub fn column_count(&self) -> usize {
        self.columns
    }

    pub fn num_row_vars(&self) -> usize {
        padded_num_vars(self.a.len())
    }

    pub fn num_column_vars(&self) -> usize {
        padded_num_vars(self.columns)
    }

    /// The matrices under the installed modulus, prepared as
    /// [`PreparedConstraintMatrices::from_vendored`] prepares them but
    /// carrying this integer digest (their `lower`).
    pub fn lower(&self) -> Result<PreparedConstraintMatrices, MatrixError> {
        self.lower_into(LoweredScratch::default())
    }

    /// [`Self::lower`] into the buffers of an earlier lowering.
    pub fn lower_into(&self, scratch: LoweredScratch) -> Result<PreparedConstraintMatrices, MatrixError> {
        let max_shift = [&self.a, &self.b, &self.c]
            .iter()
            .flat_map(|matrix| matrix.iter())
            .flat_map(|row| row.iter())
            .map(|(_, coefficient)| coefficient.max_shift())
            .max()
            .unwrap_or(0);
        let mut pow2 = Vec::with_capacity(usize::from(max_shift) + 1);
        let mut power = Fq::ONE;
        for _ in 0..=max_shift {
            pow2.push(power);
            power = power + power;
        }
        let lower = |matrix: &Vec<Vec<(usize, IntegerCoefficient)>>,
                     row_starts: &Vec<usize>,
                     mut entries: Vec<(usize, Fq)>| {
            let total = row_starts[row_starts.len() - 1];
            // One buffer, filled in parallel by blocks of rows: the block's
            // slice is its rows' run of the buffer.
            entries.clear();
            entries.reserve(total);
            let block_rows = matrix.len().div_ceil(LOWER_BLOCKS).max(1);
            let mut pieces: Vec<(usize, usize, &mut [MaybeUninit<(usize, Fq)>])> =
                Vec::with_capacity(LOWER_BLOCKS);
            let mut rest = &mut entries.spare_capacity_mut()[..total];
            let mut row = 0;
            while row < matrix.len() {
                let end = (row + block_rows).min(matrix.len());
                let (head, tail) = rest.split_at_mut(row_starts[end] - row_starts[row]);
                pieces.push((row, end, head));
                rest = tail;
                row = end;
            }
            cfg_into_iter!(pieces).for_each(|(first, end, piece)| {
                let mut offset = 0;
                for row in &matrix[first..end] {
                    for (column, coefficient) in row {
                        piece[offset].write((*column, coefficient.lower(&pow2)));
                        offset += 1;
                    }
                }
                debug_assert_eq!(offset, piece.len());
            });
            // SAFETY: the pieces partition the first `total` spare slots
            // (`row_starts` sums the row lengths) and every slot of every
            // piece was written above.
            unsafe { entries.set_len(total) };
            SparseMatrix::from_flat(entries, row_starts.clone(), self.columns)
        };
        let [entries_a, entries_b, entries_c] = scratch.entries;
        PreparedConstraintMatrices::prepare_indexed(
            lower(&self.a, &self.row_starts[0], entries_a),
            lower(&self.b, &self.row_starts[1], entries_b),
            lower(&self.c, &self.row_starts[2], entries_c),
            self.m_rows,
            self.m_columns,
            self.digest,
            Arc::clone(&self.column_chunks),
        )
    }
}

impl Products {
    /// Whether `Az ∘ Bz = Cz` at every row.
    pub fn satisfied(&self) -> bool {
        self.az
            .iter()
            .zip(&self.bz)
            .zip(&self.cz)
            .all(|((&a, &b), &c)| a * b == c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Three chunks of columns plus one chunk of padding (their fixture).
    const COLUMNS: usize = 3 << BIND_CHUNK_COLUMN_VARS;
    const ROWS: usize = 37;
    const ENTRIES_PER_ROW: usize = 24;

    fn random_sparse_matrix(state: &mut u64) -> SparseMatrix {
        let rows = (0..ROWS)
            .map(|_| {
                let mut columns: Vec<usize> = (0..ENTRIES_PER_ROW)
                    .map(|_| splitmix(state) as usize % COLUMNS)
                    .collect();
                columns.sort_unstable();
                columns.dedup();
                columns
                    .into_iter()
                    .map(|column| (column, Fq::new(u128::from(splitmix(state)))))
                    .collect()
            })
            .collect();
        SparseMatrix::new(rows, COLUMNS)
    }

    fn random_prepared(state: &mut u64) -> PreparedConstraintMatrices {
        let m_rows = vec![Vec::new(); COLUMNS];
        let a = random_sparse_matrix(state);
        let b = random_sparse_matrix(state);
        let c = random_sparse_matrix(state);
        PreparedConstraintMatrices::from_parts(&m_rows, 1, a, b, c).unwrap()
    }

    fn random_point(state: &mut u64, len: usize) -> Vec<Fq> {
        (0..len).map(|_| Fq::new(u128::from(splitmix(state)))).collect()
    }

    /// `D(r_y)` by the direct triple loop over every nonzero.
    fn reference_evaluation(
        prepared: &PreparedConstraintMatrices,
        row_point: &[Fq],
        rho: Fq,
        column_point: &[Fq],
    ) -> Fq {
        let row_weights = eq_table(row_point);
        let column_weights = eq_table(column_point);
        let mut evaluation = Fq::ZERO;
        for (matrix, batch_scale) in [(&prepared.a, Fq::ONE), (&prepared.b, rho), (&prepared.c, rho * rho)] {
            for (row, entries) in matrix.rows().enumerate() {
                for &(column, coefficient) in entries {
                    evaluation += batch_scale * row_weights[row] * column_weights[column] * coefficient;
                }
            }
        }
        evaluation
    }

    #[test]
    fn column_chunks_partition_every_row() {
        let mut state = 7;
        let prepared = random_prepared(&mut state);
        for (matrix, index) in [&prepared.a, &prepared.b, &prepared.c]
            .into_iter()
            .zip(prepared.column_chunks.iter())
        {
            let mut spans: Vec<(usize, RowSpan)> = index
                .spans
                .iter()
                .enumerate()
                .flat_map(|(chunk, spans)| spans.iter().map(move |span| (chunk, *span)))
                .collect();
            spans.sort_by_key(|(_, span)| (span.row, span.start));
            let mut spans = spans.into_iter().peekable();
            for (row, entries) in matrix.rows().enumerate() {
                let mut next_start = 0;
                while let Some((chunk, span)) = spans.next_if(|(_, span)| span.row == row) {
                    assert_eq!(span.start, next_start);
                    assert!(span.end > span.start);
                    for &(column, _) in &entries[span.start..span.end] {
                        assert_eq!(column / index.chunk_len, chunk);
                    }
                    next_start = span.end;
                }
                assert_eq!(next_start, entries.len());
            }
            assert!(spans.next().is_none());
        }
    }

    #[test]
    fn evaluate_batched_matches_reference_and_bound_table() {
        let mut state = 11;
        let prepared = random_prepared(&mut state);
        let row_point = random_point(&mut state, prepared.num_row_vars());
        let column_point = random_point(&mut state, prepared.num_column_vars());
        let rho = Fq::new(u128::from(splitmix(&mut state)));
        let expected = reference_evaluation(&prepared, &row_point, rho, &column_point);
        assert_eq!(prepared.evaluate_batched(&row_point, rho, &column_point).unwrap(), expected);
        let bound = prepared.bind_and_batch(&row_point, rho).unwrap();
        let column_weights = eq_table(&column_point);
        let through_table: Fq = bound.iter().zip(&column_weights).map(|(d, w)| *d * *w).sum();
        assert_eq!(through_table, expected);
    }

    /// Under the default modulus the integer lowering is the residue
    /// lowering, entry for entry; its digest is its own domain.
    #[test]
    fn integer_matrices_lower_to_the_residue_lowering() {
        use circuit::constraints::ConstraintGenerator;
        use circuit::sha256::{COMPRESSION_INPUT_BITS, compression_circuit};
        let mut generator = ConstraintGenerator::new(COMPRESSION_INPUT_BITS);
        let symbolic = generator.boxed_inputs::<COMPRESSION_INPUT_BITS>();
        let _ = compression_circuit(&mut generator, symbolic.as_ref());
        let integer = generator.into_matrices();
        let direct = PreparedConstraintMatrices::from_vendored(&integer).unwrap();
        let kept = PreparedIntegerMatrices::from_vendored(&integer).unwrap();
        let lowered = kept.lower().unwrap();
        assert_eq!(lowered.a(), direct.a());
        assert_eq!(lowered.b(), direct.b());
        assert_eq!(lowered.c(), direct.c());
        assert_eq!(lowered.digest(), kept.digest());
        assert_ne!(lowered.digest(), direct.digest());
        assert_eq!(lowered.num_row_vars(), direct.num_row_vars());
        assert_eq!(lowered.num_column_vars(), direct.num_column_vars());
        assert_eq!((kept.num_row_vars(), kept.num_column_vars()), (8, 15));
    }

    #[test]
    fn residues_are_canonical() {
        let q = modulus();
        assert_eq!(q, crate::bitz::fq::Q, "the default modulus is installed");
        assert_eq!(residue(&BigInt::from(-1)).lift(), q - 1);
        assert_eq!(residue(&BigInt::from(1i128 << 100)).lift(), 15);
        assert_eq!(residue(&-BigInt::from(q)).lift(), 0);
        let big = BigInt::from(q) * BigInt::from(3) + BigInt::from(7);
        assert_eq!(residue(&big).lift(), 7);
        assert_eq!(residue(&-big).lift(), q - 7);
    }
}
