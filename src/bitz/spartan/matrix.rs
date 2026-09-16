//! Their `spartan/src/matrix.rs`: the R1CS matrices, the column-chunked
//! nonzero index, `bind_and_batch`, `evaluate_batched`, the canonical
//! constraint digests (`constraint_matrix_digest`, SHA-256 over
//! `bitz/spartan/constraint-matrices/v1` on the residues, and their integer
//! digest over `bitz/spartan/integer-constraint-matrices/v1`), plus the
//! product tables `Az, Bz, Cz` padded to the Boolean row domain.
//!
//! The matrices are kept once, over the integers and compactly: an entry is
//! a `u32` column and a `u16` code, `2·shift + sign` for the coefficient
//! `±2^shift` (every bit lift; their `IntegerCoefficient::PowerOfTwo`), or
//! [`ESCAPE`] with the coefficient in a side table keyed by the entry's
//! index (their `Small` and `Big`: the constants). Lowering under a modulus
//! is a table of `2 · (max shift + 1)` residues plus the escapes' residues —
//! microseconds, whichever prime the transcript drew — and the Spartan
//! passes read the residue through the code, six bytes per nonzero instead
//! of twenty-four. `M` is consumed by the digests only; the PIOP never
//! reads it.

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
    /// A coefficient's shift does not fit a code.
    ShiftTooLarge,
    /// More columns or entries than the compact form addresses.
    TooLarge,
}

/// The code of an entry whose coefficient sits in the escape table.
pub const ESCAPE: u16 = u16::MAX;
/// The largest shift a code carries.
const MAX_CODED_SHIFT: u16 = (ESCAPE >> 1) - 1;

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

    /// The code of this coefficient, or [`ESCAPE`].
    fn code(&self) -> Result<u16, MatrixError> {
        match self {
            Self::PowerOfTwo { shift, negative } => {
                if *shift > MAX_CODED_SHIFT {
                    return Err(MatrixError::ShiftTooLarge);
                }
                Ok((shift << 1) | u16::from(*negative))
            }
            _ => Ok(ESCAPE),
        }
    }

    /// The residue under the installed modulus.
    pub fn residue(&self) -> Fq {
        match self {
            Self::PowerOfTwo { shift, negative } => {
                let mut value = Fq::ONE;
                for _ in 0..*shift {
                    value = value + value;
                }
                if *negative { -value } else { value }
            }
            Self::Small(small) => Fq::new(i128::from(*small).rem_euclid(modulus() as i128) as u128),
            Self::Big(big) => residue(big),
        }
    }

    /// Whether the integer is negative.
    pub fn is_negative(&self) -> bool {
        match self {
            Self::PowerOfTwo { negative, .. } => *negative,
            Self::Small(small) => *small < 0,
            Self::Big(big) => big.is_negative(),
        }
    }

    /// The magnitude's little-endian bytes, no trailing zeros (their
    /// `BigUint::to_bytes_le`; the magnitude is never zero).
    pub fn magnitude_le_bytes(&self) -> Vec<u8> {
        match self {
            Self::PowerOfTwo { shift, .. } => {
                let mut bytes = vec![0u8; usize::from(*shift) / 8 + 1];
                bytes[usize::from(*shift) / 8] = 1 << (shift % 8);
                bytes
            }
            Self::Small(small) => BigInt::from(*small).magnitude().to_bytes_le(),
            Self::Big(big) => big.magnitude().to_bytes_le(),
        }
    }
}

/// One R1CS matrix over the integers in the compact form: rows of
/// `(column, code)` with strictly increasing columns, every row's entries
/// contiguous (`row_starts[i]..row_starts[i + 1]`), the escaped coefficients
/// keyed by entry index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactMatrix {
    row_starts: Vec<usize>,
    columns: Vec<u32>,
    codes: Vec<u16>,
    escapes: Vec<(usize, IntegerCoefficient)>,
    column_count: usize,
    max_shift: u16,
}

/// Builds a [`CompactMatrix`] row by row; the column count is fixed at the
/// end (a generator learns it last).
#[derive(Debug, Clone)]
pub struct CompactMatrixBuilder {
    matrix: CompactMatrix,
    max_column: Option<u32>,
}

impl Default for CompactMatrixBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactMatrixBuilder {
    pub fn new() -> Self {
        Self {
            matrix: CompactMatrix {
                row_starts: vec![0],
                columns: Vec::new(),
                codes: Vec::new(),
                escapes: Vec::new(),
                column_count: 0,
                max_shift: 0,
            },
            max_column: None,
        }
    }

    /// Appends one row: `(column, coefficient)` with strictly increasing
    /// columns, no zero coefficients.
    pub fn push_row<'a>(
        &mut self,
        entries: impl IntoIterator<Item = (usize, &'a IntegerCoefficient)>,
    ) -> Result<(), MatrixError> {
        let matrix = &mut self.matrix;
        for (column, coefficient) in entries {
            let column = u32::try_from(column).map_err(|_| MatrixError::TooLarge)?;
            debug_assert!(
                matrix.row_starts[matrix.row_starts.len() - 1] == matrix.columns.len()
                    || matrix.columns[matrix.columns.len() - 1] < column,
                "columns increase within a row"
            );
            let code = coefficient.code()?;
            if code == ESCAPE {
                matrix.escapes.push((matrix.columns.len(), coefficient.clone()));
            } else {
                matrix.max_shift = matrix.max_shift.max(code >> 1);
            }
            matrix.columns.push(column);
            matrix.codes.push(code);
            self.max_column = Some(self.max_column.map_or(column, |max| max.max(column)));
        }
        matrix.row_starts.push(matrix.columns.len());
        Ok(())
    }

    /// The matrix over `column_count` columns (every entry must fit).
    pub fn finish(mut self, column_count: usize) -> Result<CompactMatrix, MatrixError> {
        if self.max_column.is_some_and(|max| max as usize >= column_count) {
            return Err(MatrixError::InvalidR1csShape);
        }
        self.matrix.column_count = column_count;
        Ok(self.matrix)
    }
}

impl CompactMatrix {
    /// From rows of `(column, coefficient)`.
    pub fn from_rows<'a>(
        rows: impl IntoIterator<Item = &'a [(usize, IntegerCoefficient)]>,
        column_count: usize,
    ) -> Result<Self, MatrixError> {
        let mut builder = CompactMatrixBuilder::new();
        for row in rows {
            builder.push_row(row.iter().map(|(column, coefficient)| (*column, coefficient)))?;
        }
        builder.finish(column_count)
    }

    pub fn row_count(&self) -> usize {
        self.row_starts.len() - 1
    }

    pub fn column_count(&self) -> usize {
        self.column_count
    }

    pub fn nonzero_count(&self) -> usize {
        self.columns.len()
    }

    /// The entry range of a row.
    #[inline]
    pub fn row_range(&self, row: usize) -> std::ops::Range<usize> {
        self.row_starts[row]..self.row_starts[row + 1]
    }

    /// The columns of a row.
    pub fn row_columns(&self, row: usize) -> &[u32] {
        &self.columns[self.row_range(row)]
    }

    /// The integer coefficient at an entry.
    pub fn coefficient(&self, entry: usize) -> IntegerCoefficient {
        let code = self.codes[entry];
        if code == ESCAPE {
            self.escape(entry).1.clone()
        } else {
            IntegerCoefficient::PowerOfTwo {
                shift: code >> 1,
                negative: code & 1 == 1,
            }
        }
    }

    #[inline]
    fn escape(&self, entry: usize) -> &(usize, IntegerCoefficient) {
        let index = self
            .escapes
            .binary_search_by_key(&entry, |(index, _)| *index)
            .expect("an escaped entry is in the escape table");
        &self.escapes[index]
    }

    /// The index of the escape at an entry (the escape table is keyed by
    /// entry index, so this is a binary search; escapes are rare).
    #[inline]
    fn escape_index(&self, entry: usize) -> usize {
        self.escapes
            .binary_search_by_key(&entry, |(index, _)| *index)
            .expect("an escaped entry is in the escape table")
    }

    /// The residues of this matrix's escapes under the installed modulus.
    fn escape_residues(&self) -> Vec<Fq> {
        self.escapes.iter().map(|(_, coefficient)| coefficient.residue()).collect()
    }

    /// The rows as `(column, coefficient)` (tests and the digests).
    pub fn rows(&self) -> impl Iterator<Item = Vec<(usize, IntegerCoefficient)>> + '_ {
        (0..self.row_count()).map(move |row| {
            self.row_range(row)
                .map(|entry| (self.columns[entry] as usize, self.coefficient(entry)))
                .collect()
        })
    }
}

/// The residues of a set of matrices' coefficients under one modulus.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResidueTables {
    /// `table[code] = ±2^shift mod q` for `code = 2·shift + sign`.
    by_code: Vec<Fq>,
    /// Per matrix, the residues of its escapes in escape order.
    escapes: [Vec<Fq>; 3],
}

impl ResidueTables {
    fn new(matrices: &[CompactMatrix; 3]) -> Self {
        let max_shift = matrices.iter().map(|matrix| matrix.max_shift).max().unwrap_or(0);
        let mut by_code = Vec::with_capacity(2 * (usize::from(max_shift) + 1));
        let mut power = Fq::ONE;
        for _ in 0..=max_shift {
            by_code.push(power);
            by_code.push(-power);
            power = power + power;
        }
        Self {
            by_code,
            escapes: [
                matrices[0].escape_residues(),
                matrices[1].escape_residues(),
                matrices[2].escape_residues(),
            ],
        }
    }
}

/// The residue of an entry's coefficient.
#[inline(always)]
fn residue_at(matrix: &CompactMatrix, table: &[Fq], escapes: &[Fq], entry: usize) -> Fq {
    let code = matrix.codes[entry];
    if code == ESCAPE {
        escapes[matrix.escape_index(entry)]
    } else {
        table[usize::from(code)]
    }
}

/// `bind_and_batch` splits the column domain into chunks of `2^16` columns
/// and accumulates each chunk on its own thread (their constant).
const BIND_CHUNK_COLUMN_VARS: usize = 16;

/// The entries `[start, end)` of row `row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowSpan {
    row: u32,
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
    fn new(matrix: &CompactMatrix, chunk_len: usize, chunk_count: usize) -> Self {
        debug_assert!(chunk_len.is_power_of_two());
        debug_assert!(chunk_len * chunk_count >= matrix.column_count());
        let mut spans = vec![Vec::new(); chunk_count];
        for row in 0..matrix.row_count() {
            let range = matrix.row_range(row);
            let columns = &matrix.columns[range.clone()];
            let mut start = 0;
            while start < columns.len() {
                // Columns increase within a row, so a chunk's entries are a
                // contiguous run.
                let chunk = columns[start] as usize / chunk_len;
                let end = start
                    + columns[start..].partition_point(|column| *column as usize / chunk_len == chunk);
                spans[chunk].push(RowSpan {
                    row: row as u32,
                    start: range.start + start,
                    end: range.start + end,
                });
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

    fn m_row(&mut self, positions: &[u32]) {
        self.usize(positions.len());
        for &column in positions {
            self.usize(column as usize);
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

/// `M`'s rows (their positions, ascending), for the digests: CSR built from
/// the map's CSC by scanning the columns in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRows {
    row_starts: Vec<usize>,
    positions: Vec<u32>,
    column_count: usize,
}

impl MapRows {
    /// From CSC (`column_offsets`, `row_indices`), `row_count` rows.
    pub fn from_csc(column_offsets: &[u32], row_indices: &[u32], row_count: usize) -> Self {
        let column_count = column_offsets.len() - 1;
        let mut row_starts = vec![0usize; row_count + 1];
        for &row in row_indices {
            row_starts[row as usize + 1] += 1;
        }
        for row in 0..row_count {
            row_starts[row + 1] += row_starts[row];
        }
        let mut cursors = row_starts[..row_count].to_vec();
        let mut positions = vec![0u32; row_indices.len()];
        for column in 0..column_count {
            for &row in &row_indices[column_offsets[column] as usize..column_offsets[column + 1] as usize] {
                let cursor = &mut cursors[row as usize];
                positions[*cursor] = column as u32;
                *cursor += 1;
            }
        }
        Self {
            row_starts,
            positions,
            column_count,
        }
    }

    /// From explicit rows (tests).
    pub fn from_rows(rows: &[Vec<usize>], column_count: usize) -> Self {
        let mut row_starts = Vec::with_capacity(rows.len() + 1);
        row_starts.push(0);
        let mut positions = Vec::new();
        for row in rows {
            positions.extend(row.iter().map(|&column| column as u32));
            row_starts.push(positions.len());
        }
        Self {
            row_starts,
            positions,
            column_count,
        }
    }

    pub fn row_count(&self) -> usize {
        self.row_starts.len() - 1
    }

    pub fn column_count(&self) -> usize {
        self.column_count
    }

    pub fn row(&self, row: usize) -> &[u32] {
        &self.positions[self.row_starts[row]..self.row_starts[row + 1]]
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

fn padded_num_vars(logical_len: usize) -> usize {
    logical_len.max(1).next_power_of_two().ilog2() as usize
}

/// The matrices, their chunk index, and everything prime-independent the
/// two prepared forms share.
#[derive(Debug, PartialEq, Eq)]
struct Shared {
    matrices: [CompactMatrix; 3],
    column_chunks: [ColumnChunkIndex; 3],
    m_rows: usize,
    m_columns: usize,
    num_row_vars: usize,
    num_column_vars: usize,
}

impl Shared {
    fn new(
        matrices: [CompactMatrix; 3],
        m_rows: usize,
        m_columns: usize,
    ) -> Result<Self, MatrixError> {
        let rows = matrices[0].row_count();
        let columns = matrices[0].column_count();
        if matrices.iter().any(|m| m.row_count() != rows || m.column_count() != columns)
            || columns != m_rows
        {
            return Err(MatrixError::InvalidR1csShape);
        }
        if columns > u32::MAX as usize {
            return Err(MatrixError::TooLarge);
        }
        let num_row_vars = padded_num_vars(rows);
        let num_column_vars = padded_num_vars(columns);
        let (chunk_len, chunk_count) = chunk_geometry(num_column_vars);
        let column_chunks = [0, 1, 2].map(|i| ColumnChunkIndex::new(&matrices[i], chunk_len, chunk_count));
        Ok(Self {
            matrices,
            column_chunks,
            m_rows,
            m_columns,
            num_row_vars,
            num_column_vars,
        })
    }

    /// Their residue digest (`constraint_matrix_digest`) under the
    /// installed modulus.
    fn residue_digest(&self, map: &MapRows) -> [u8; 32] {
        let mut digest = DigestBuilder::new();
        digest.m_header(map.row_count(), map.column_count());
        for row in 0..map.row_count() {
            digest.m_row(map.row(row));
        }
        let tables = ResidueTables::new(&self.matrices);
        for (label, matrix, escapes) in [
            (&b"A"[..], &self.matrices[0], &tables.escapes[0]),
            (&b"B"[..], &self.matrices[1], &tables.escapes[1]),
            (&b"C"[..], &self.matrices[2], &tables.escapes[2]),
        ] {
            digest.matrix_header(label, matrix.row_count(), matrix.column_count());
            for row in 0..matrix.row_count() {
                let range = matrix.row_range(row);
                digest.entry_count(range.len());
                for entry in range {
                    digest.entry(
                        matrix.columns[entry] as usize,
                        residue_at(matrix, &tables.by_code, escapes, entry),
                    );
                }
            }
        }
        digest.finish()
    }

    /// Their integer digest (`bitz/spartan/integer-constraint-matrices/v1`).
    fn integer_digest(&self, map: &MapRows) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"bitz/spartan/integer-constraint-matrices/v1");
        let put = |hash: &mut Sha256, value: usize| hash.update((value as u64).to_le_bytes());
        hash.update(b"M");
        put(&mut hash, map.row_count());
        put(&mut hash, map.column_count());
        for row in 0..map.row_count() {
            let positions = map.row(row);
            put(&mut hash, positions.len());
            for &column in positions {
                put(&mut hash, column as usize);
            }
        }
        for (label, matrix) in [(&b"A"[..], &self.matrices[0]), (&b"B"[..], &self.matrices[1]), (&b"C"[..], &self.matrices[2])] {
            hash.update(label);
            put(&mut hash, matrix.row_count());
            put(&mut hash, matrix.column_count());
            for row in 0..matrix.row_count() {
                let range = matrix.row_range(row);
                put(&mut hash, range.len());
                for entry in range {
                    put(&mut hash, matrix.columns[entry] as usize);
                    let coefficient = matrix.coefficient(entry);
                    hash.update([u8::from(coefficient.is_negative())]);
                    let magnitude = coefficient.magnitude_le_bytes();
                    put(&mut hash, magnitude.len());
                    hash.update(&magnitude);
                }
            }
        }
        hash.finalize().into()
    }
}

/// Their `PreparedConstraintMatrices<Fq>`: the matrices under one modulus
/// (a residue table), the Boolean domain sizes, the digest and the column
/// chunking. Shares the matrices with a [`PreparedIntegerMatrices`] it was
/// lowered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedConstraintMatrices {
    shared: Arc<Shared>,
    residues: ResidueTables,
    digest: [u8; 32],
}

impl PreparedConstraintMatrices {
    /// From the compact matrices and `M`'s rows, digested on the residues
    /// under the installed modulus (their `PreparedConstraintMatrices::new`
    /// on the lowered matrices).
    pub fn from_compact(
        matrices: [CompactMatrix; 3],
        map: &MapRows,
    ) -> Result<Self, MatrixError> {
        let shared = Shared::new(matrices, map.row_count(), map.column_count())?;
        let digest = shared.residue_digest(map);
        let residues = ResidueTables::new(&shared.matrices);
        Ok(Self {
            shared: Arc::new(shared),
            residues,
            digest,
        })
    }

    /// Lowers the vendored integer matrices (their `ConstraintGenerator`'s
    /// output) — tests and probes; the schemes generate the compact form
    /// directly.
    pub fn from_vendored(
        matrices: &circuit::constraints::ConstraintMatrices,
    ) -> Result<Self, MatrixError> {
        let (compact, map) = compact_from_vendored(matrices)?;
        Self::from_compact(compact, &map)
    }

    /// The same preparation from integer rows and `M`'s positions (tests
    /// and fixtures): `m_rows[i]` are the positions of row `i` of `M`.
    pub fn from_parts(
        m_rows: &[Vec<usize>],
        m_columns: usize,
        a: CompactMatrix,
        b: CompactMatrix,
        c: CompactMatrix,
    ) -> Result<Self, MatrixError> {
        let map = MapRows::from_rows(m_rows, m_columns);
        Self::from_compact([a, b, c], &map)
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn num_row_vars(&self) -> usize {
        self.shared.num_row_vars
    }

    pub fn num_column_vars(&self) -> usize {
        self.shared.num_column_vars
    }

    /// The R1CS row count (`A`'s rows).
    pub fn row_count(&self) -> usize {
        self.shared.matrices[0].row_count()
    }

    /// The assignment length `h_len` (`A`'s columns = `M`'s rows).
    pub fn column_count(&self) -> usize {
        self.shared.matrices[0].column_count()
    }

    /// `M`'s dimensions, `(h_len, f_len)`.
    pub fn m_dimensions(&self) -> (usize, usize) {
        (self.shared.m_rows, self.shared.m_columns)
    }

    pub fn a(&self) -> &CompactMatrix {
        &self.shared.matrices[0]
    }

    pub fn b(&self) -> &CompactMatrix {
        &self.shared.matrices[1]
    }

    pub fn c(&self) -> &CompactMatrix {
        &self.shared.matrices[2]
    }

    /// The residue of an entry of matrix `which` (0, 1, 2 for `A`, `B`, `C`).
    pub fn residue(&self, which: usize, entry: usize) -> Fq {
        residue_at(
            &self.shared.matrices[which],
            &self.residues.by_code,
            &self.residues.escapes[which],
            entry,
        )
    }

    /// Their `short_debug_info`.
    pub fn short_debug_info(&self) -> String {
        format!(
            "{} r1cs rows -> 2^{}, {} h entries -> 2^{}, {} nonzeros",
            self.row_count(),
            self.num_row_vars(),
            self.column_count(),
            self.num_column_vars(),
            self.shared.matrices.iter().map(CompactMatrix::nonzero_count).sum::<usize>()
        )
    }

    /// `Az, Bz, Cz` over `Fq` from the 0/1 assignment `h`, each padded with
    /// zeros to `2^num_row_vars` (their `build_product_mles`, computed from
    /// the sparse rows: `h` is Boolean, so a row's value is the sum of the
    /// coefficient residues at its set bits — the same residues).
    pub fn products(&self, assignment: &PackedWitness) -> Result<Products, MatrixError> {
        let mut products = Products::default();
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
        let padded_len = 1usize << self.shared.num_row_vars;
        let table = &self.residues.by_code;
        let evaluate = |which: usize, values: &mut Vec<Fq>| {
            let matrix = &self.shared.matrices[which];
            let escapes = &self.residues.escapes[which];
            values.clear();
            values.resize(padded_len, Fq::ZERO);
            let rows = matrix.row_count();
            cfg_chunks_mut!(values[..rows], 256)
                .enumerate()
                .for_each(|(chunk, values)| {
                    for (offset, value) in values.iter_mut().enumerate() {
                        let mut sum = Fq::ZERO;
                        for entry in matrix.row_range(chunk * 256 + offset) {
                            if assignment.bit(matrix.columns[entry] as usize) {
                                sum += residue_at(matrix, table, escapes, entry);
                            }
                        }
                        *value = sum;
                    }
                });
        };
        evaluate(0, &mut products.az);
        evaluate(1, &mut products.bz);
        evaluate(2, &mut products.cz);
        Ok(())
    }

    /// The assignment `h = M(1 ‖ f)` as `Fq` values padded with zeros to
    /// `2^num_column_vars` (their `build_assignment_mle`; the provers read
    /// the bits directly, this is for checks); bit 0 must be the constant
    /// one.
    pub fn assignment(&self, assignment: &PackedWitness) -> Result<Vec<Fq>, MatrixError> {
        self.check_assignment(assignment)?;
        let mut values = vec![Fq::ZERO; 1usize << self.shared.num_column_vars];
        let bits = assignment.bit_len();
        cfg_chunks_mut!(values[..bits], 1 << 14)
            .enumerate()
            .for_each(|(chunk, values)| {
                let base = chunk << 14;
                for (offset, value) in values.iter_mut().enumerate() {
                    *value = Fq::from(assignment.bit(base + offset));
                }
            });
        Ok(values)
    }

    /// Whether `h` has the assignment length and the constant one at bit 0.
    pub fn check_assignment(&self, assignment: &PackedWitness) -> Result<(), MatrixError> {
        if assignment.bit_len() != self.column_count() {
            return Err(MatrixError::InvalidAssignmentLength {
                expected: self.column_count(),
                actual: assignment.bit_len(),
            });
        }
        if self.column_count() == 0 || !assignment.bit(0) {
            return Err(MatrixError::InvalidAssignmentConstant);
        }
        Ok(())
    }

    /// `D(j) = Σ_i eq(i, r_x) (A + ρ B + ρ² C)[i, j]` as a dense table over
    /// the column domain, one column chunk per task (their `bind_and_batch`).
    pub fn bind_and_batch(&self, row_point: &[Fq], rho: Fq) -> Result<Vec<Fq>, MatrixError> {
        if row_point.len() != self.shared.num_row_vars {
            return Err(MatrixError::InvalidRowPointLength {
                expected: self.shared.num_row_vars,
                actual: row_point.len(),
            });
        }
        let row_weights = eq_table(row_point);
        let table = &self.residues.by_code;
        let batched = [
            (&self.shared.matrices[0], &self.shared.column_chunks[0], &self.residues.escapes[0], Fq::ONE),
            (&self.shared.matrices[1], &self.shared.column_chunks[1], &self.residues.escapes[1], rho),
            (&self.shared.matrices[2], &self.shared.column_chunks[2], &self.residues.escapes[2], rho * rho),
        ];
        let chunk_len = self.shared.column_chunks[0].chunk_len;
        let mut evaluations = vec![Fq::ZERO; 1usize << self.shared.num_column_vars];
        cfg_chunks_mut!(evaluations, chunk_len)
            .enumerate()
            .for_each(|(chunk, output)| {
                let base = chunk * chunk_len;
                for (matrix, index, escapes, batch_scale) in batched {
                    for span in &index.spans[chunk] {
                        let row_scale = row_weights[span.row as usize] * batch_scale;
                        for entry in span.start..span.end {
                            let column = matrix.columns[entry] as usize;
                            output[column - base] += row_scale * residue_at(matrix, table, escapes, entry);
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
        if row_point.len() != self.shared.num_row_vars {
            return Err(MatrixError::InvalidRowPointLength {
                expected: self.shared.num_row_vars,
                actual: row_point.len(),
            });
        }
        if column_point.len() != self.shared.num_column_vars {
            return Err(MatrixError::InvalidColumnPointLength {
                expected: self.shared.num_column_vars,
                actual: column_point.len(),
            });
        }
        let row_weights = eq_table(row_point);
        let table = &self.residues.by_code;
        let batched = [
            (&self.shared.matrices[0], &self.shared.column_chunks[0], &self.residues.escapes[0], Fq::ONE),
            (&self.shared.matrices[1], &self.shared.column_chunks[1], &self.residues.escapes[1], rho),
            (&self.shared.matrices[2], &self.shared.column_chunks[2], &self.residues.escapes[2], rho * rho),
        ];
        let chunk_len = self.shared.column_chunks[0].chunk_len;
        let (low_point, high_point) = column_point.split_at(chunk_len.ilog2() as usize);
        let low_weights = eq_table(low_point);
        let high_weights = eq_table(high_point);
        debug_assert_eq!(high_weights.len(), self.shared.column_chunks[0].spans.len());
        let partials: Vec<Fq> = cfg_into_iter!(0..high_weights.len())
            .map(|chunk| {
                let base = chunk * chunk_len;
                let mut chunk_sum = Fq::ZERO;
                for (matrix, index, escapes, batch_scale) in batched {
                    let mut matrix_sum = Fq::ZERO;
                    for span in &index.spans[chunk] {
                        let mut span_sum = Fq::ZERO;
                        for entry in span.start..span.end {
                            let column = matrix.columns[entry] as usize;
                            span_sum += low_weights[column - base] * residue_at(matrix, table, escapes, entry);
                        }
                        matrix_sum += row_weights[span.row as usize] * span_sum;
                    }
                    chunk_sum += batch_scale * matrix_sum;
                }
                high_weights[chunk] * chunk_sum
            })
            .collect();
        Ok(partials.into_iter().sum())
    }
}

/// Their `PreparedIntegerMatrices`: the R1CS over the integers with a
/// digest no prime enters (`bitz/spartan/integer-constraint-matrices/v1`:
/// `M` as in the residue digest, then every entry of `A`, `B`, `C` as its
/// column, a sign byte, and the magnitude's little-endian bytes with their
/// length), lowered under the installed modulus per proof — a residue
/// table, the matrices shared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedIntegerMatrices {
    shared: Arc<Shared>,
    digest: [u8; 32],
}

impl PreparedIntegerMatrices {
    /// From the compact matrices and `M`'s rows.
    pub fn from_compact(matrices: [CompactMatrix; 3], map: &MapRows) -> Result<Self, MatrixError> {
        let shared = Shared::new(matrices, map.row_count(), map.column_count())?;
        let digest = shared.integer_digest(map);
        Ok(Self {
            shared: Arc::new(shared),
            digest,
        })
    }

    /// From the vendored integer matrices (tests and probes).
    pub fn from_vendored(
        matrices: &circuit::constraints::ConstraintMatrices,
    ) -> Result<Self, MatrixError> {
        let (compact, map) = compact_from_vendored(matrices)?;
        Self::from_compact(compact, &map)
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn row_count(&self) -> usize {
        self.shared.matrices[0].row_count()
    }

    pub fn column_count(&self) -> usize {
        self.shared.matrices[0].column_count()
    }

    pub fn num_row_vars(&self) -> usize {
        self.shared.num_row_vars
    }

    pub fn num_column_vars(&self) -> usize {
        self.shared.num_column_vars
    }

    pub fn a(&self) -> &CompactMatrix {
        &self.shared.matrices[0]
    }

    pub fn b(&self) -> &CompactMatrix {
        &self.shared.matrices[1]
    }

    pub fn c(&self) -> &CompactMatrix {
        &self.shared.matrices[2]
    }

    /// The matrices under the installed modulus, carrying this integer
    /// digest (their `lower`): the residue tables, the matrices shared.
    pub fn lower(&self) -> Result<PreparedConstraintMatrices, MatrixError> {
        Ok(PreparedConstraintMatrices {
            shared: Arc::clone(&self.shared),
            residues: ResidueTables::new(&self.shared.matrices),
            digest: self.digest,
        })
    }
}

/// The vendored matrices in the compact form, with `M`'s rows.
pub fn compact_from_vendored(
    matrices: &circuit::constraints::ConstraintMatrices,
) -> Result<([CompactMatrix; 3], MapRows), MatrixError> {
    let (m, a, b, c) = (&matrices.m, &matrices.a, &matrices.b, &matrices.c);
    let compact = |matrix: &circuit::constraints::SparseMatrix<BigInt>| -> Result<CompactMatrix, MatrixError> {
        let mut builder = CompactMatrixBuilder::new();
        let mut row_buffer: Vec<(usize, IntegerCoefficient)> = Vec::new();
        for row in matrix.rows() {
            row_buffer.clear();
            row_buffer.extend(
                row.entries()
                    .iter()
                    .map(|(column, coefficient)| (*column, IntegerCoefficient::new(coefficient))),
            );
            builder.push_row(row_buffer.iter().map(|(column, coefficient)| (*column, coefficient)))?;
        }
        builder.finish(matrix.column_count())
    };
    let rows: Vec<Vec<usize>> = m.rows().iter().map(|row| row.positions().to_vec()).collect();
    let map = MapRows::from_rows(&rows, m.column_count());
    Ok(([compact(a)?, compact(b)?, compact(c)?], map))
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

    /// Random integer coefficients: signed powers of two, small integers
    /// and wide ones, so every code path is exercised.
    fn random_coefficient(state: &mut u64) -> IntegerCoefficient {
        let draw = splitmix(state);
        let value = match draw % 4 {
            0 | 1 => {
                let shift = (draw >> 8) % 300;
                let power = BigInt::from(1) << shift;
                if draw & 4 == 0 { power } else { -power }
            }
            2 => BigInt::from((draw >> 8) as i64 % 1_000_003 - 500_000),
            _ => (BigInt::from(draw) << 100) + BigInt::from(splitmix(state)),
        };
        IntegerCoefficient::new(&value)
    }

    fn random_matrix(state: &mut u64) -> CompactMatrix {
        let rows: Vec<Vec<(usize, IntegerCoefficient)>> = (0..ROWS)
            .map(|_| {
                let mut columns: Vec<usize> = (0..ENTRIES_PER_ROW)
                    .map(|_| splitmix(state) as usize % COLUMNS)
                    .collect();
                columns.sort_unstable();
                columns.dedup();
                columns
                    .into_iter()
                    .map(|column| (column, random_coefficient(state)))
                    .collect()
            })
            .collect();
        CompactMatrix::from_rows(rows.iter().map(Vec::as_slice), COLUMNS).unwrap()
    }

    fn random_prepared(state: &mut u64) -> PreparedConstraintMatrices {
        let m_rows = vec![Vec::new(); COLUMNS];
        let a = random_matrix(state);
        let b = random_matrix(state);
        let c = random_matrix(state);
        PreparedConstraintMatrices::from_parts(&m_rows, 1, a, b, c).unwrap()
    }

    fn random_point(state: &mut u64, len: usize) -> Vec<Fq> {
        (0..len).map(|_| Fq::new(u128::from(splitmix(state)))).collect()
    }

    /// `D(r_y)` by the direct triple loop over every nonzero, the residues
    /// recomputed from the integers.
    fn reference_evaluation(
        prepared: &PreparedConstraintMatrices,
        row_point: &[Fq],
        rho: Fq,
        column_point: &[Fq],
    ) -> Fq {
        let row_weights = eq_table(row_point);
        let column_weights = eq_table(column_point);
        let mut evaluation = Fq::ZERO;
        for (matrix, batch_scale) in [(prepared.a(), Fq::ONE), (prepared.b(), rho), (prepared.c(), rho * rho)] {
            for (row, entries) in matrix.rows().enumerate() {
                for (column, coefficient) in entries {
                    evaluation +=
                        batch_scale * row_weights[row] * column_weights[column] * coefficient.residue();
                }
            }
        }
        evaluation
    }

    #[test]
    fn codes_round_trip_and_residues_match() {
        let mut state = 3;
        let matrix = random_matrix(&mut state);
        let tables = ResidueTables::new(&[matrix.clone(), matrix.clone(), matrix.clone()]);
        for (row, entries) in matrix.rows().enumerate() {
            for (k, (column, coefficient)) in entries.iter().enumerate() {
                let entry = matrix.row_range(row).start + k;
                assert_eq!(matrix.columns[entry] as usize, *column);
                assert_eq!(
                    residue_at(&matrix, &tables.by_code, &tables.escapes[0], entry),
                    coefficient.residue()
                );
                let as_bigint = match coefficient {
                    IntegerCoefficient::PowerOfTwo { shift, negative } => {
                        let p = BigInt::from(1) << *shift;
                        if *negative { -p } else { p }
                    }
                    IntegerCoefficient::Small(s) => BigInt::from(*s),
                    IntegerCoefficient::Big(b) => (**b).clone(),
                };
                assert_eq!(coefficient.magnitude_le_bytes(), as_bigint.magnitude().to_bytes_le());
                assert_eq!(coefficient.is_negative(), as_bigint.is_negative());
                assert_eq!(coefficient.residue(), residue(&as_bigint));
            }
        }
    }

    #[test]
    fn column_chunks_partition_every_row() {
        let mut state = 7;
        let prepared = random_prepared(&mut state);
        for (matrix, index) in prepared.shared.matrices.iter().zip(prepared.shared.column_chunks.iter()) {
            let mut spans: Vec<(usize, RowSpan)> = index
                .spans
                .iter()
                .enumerate()
                .flat_map(|(chunk, spans)| spans.iter().map(move |span| (chunk, *span)))
                .collect();
            spans.sort_by_key(|(_, span)| (span.row, span.start));
            let mut spans = spans.into_iter().peekable();
            for row in 0..matrix.row_count() {
                let range = matrix.row_range(row);
                let mut next_start = range.start;
                while let Some((chunk, span)) = spans.next_if(|(_, span)| span.row as usize == row) {
                    assert_eq!(span.start, next_start);
                    assert!(span.end > span.start);
                    for entry in span.start..span.end {
                        assert_eq!(matrix.columns[entry] as usize / index.chunk_len, chunk);
                    }
                    next_start = span.end;
                }
                assert_eq!(next_start, range.end);
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
        assert_eq!(lowered.residues, direct.residues);
        assert_eq!(lowered.digest(), kept.digest());
        assert_ne!(lowered.digest(), direct.digest());
        assert_eq!(lowered.num_row_vars(), direct.num_row_vars());
        assert_eq!(lowered.num_column_vars(), direct.num_column_vars());
        assert_eq!((kept.num_row_vars(), kept.num_column_vars()), (8, 15));
        // The compact form reproduces the vendored entries.
        for (theirs, ours) in integer.a.rows().iter().zip(direct.a().rows()) {
            assert_eq!(theirs.entries().len(), ours.len());
            for ((column, coefficient), (our_column, our_coefficient)) in theirs.entries().iter().zip(&ours) {
                assert_eq!(column, our_column);
                assert_eq!(&IntegerCoefficient::new(coefficient), our_coefficient);
            }
        }
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
