//! Their `common::{shape, params, claim}`: the public instance shape, the
//! pre-claim gates, the linear claim and the transcript frames.

use spongefish::Encoding;

use super::codec::{FqWire, gf_to_bytes};
use crate::pcs::is_generator;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// The seven low bits of a row index select the basis coefficients that one
/// packed field element carries.
pub const PACK_BITS: u32 = 7;
/// The commitment size window the opening parameters are fixed for.
pub const MIN_LOG_BITS: usize = 22;
/// The upper end of that window.
pub const MAX_LOG_BITS: usize = 35;

/// A shape one of the admissibility constraints rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeError {
    RowIndexTooNarrow,
    CommitmentSizeOutOfRange,
}

/// How the committed bits are laid out, as the two index widths `t`, `s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    log_rows: usize,
    log_columns: usize,
}

impl Shape {
    pub fn new(log_rows: usize, log_columns: usize) -> Result<Self, ShapeError> {
        if log_rows < PACK_BITS as usize {
            return Err(ShapeError::RowIndexTooNarrow);
        }
        if log_rows > MAX_LOG_BITS
            || log_columns > MAX_LOG_BITS - log_rows
            || log_rows + log_columns < MIN_LOG_BITS
        {
            return Err(ShapeError::CommitmentSizeOutOfRange);
        }
        Ok(Self {
            log_rows,
            log_columns,
        })
    }

    pub fn log_rows(&self) -> usize {
        self.log_rows
    }

    pub fn log_columns(&self) -> usize {
        self.log_columns
    }

    pub fn log_bits(&self) -> usize {
        self.log_rows + self.log_columns
    }

    pub fn log_packed_len(&self) -> usize {
        self.log_bits() - PACK_BITS as usize
    }

    pub fn rows(&self) -> usize {
        1 << self.log_rows
    }

    pub fn columns(&self) -> usize {
        1 << self.log_columns
    }

    /// This crate's view of the same geometry (`W = 1`).
    pub fn layout(&self) -> crate::pcs::IntegerMatrixLayout {
        crate::pcs::IntegerMatrixLayout {
            row_vars: self.log_rows,
            col_vars: self.log_columns,
            word_bits: 1,
        }
    }
}

/// A parameter set one of the pre-claim gates rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamsError {
    /// `(k_1 + 1)(Q - 1)` reaches `ord(g)`.
    FoldBoundExceeded,
    /// The generator's order is not the full group.
    GeneratorOrderNotFull,
}

/// The shape, the modulus and the generator. Their `BitZParams<Q>` carries
/// `Q` in the type; here it is a value, encoded identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitZParams {
    shape: Shape,
    q: u128,
    generator: Gf,
}

impl BitZParams {
    /// Runs the gates that need only the parameters. `q` must be an odd
    /// prime below `2^126` (their `Fq<Q>` asserts that at compile time; the
    /// harness supplies the same constant).
    pub fn new(shape: Shape, q: u128, generator: Gf) -> Result<Self, ParamsError> {
        let Some(gap) = (q - 1).checked_mul(shape.rows() as u128 + 1) else {
            return Err(ParamsError::FoldBoundExceeded);
        };
        if gap == u128::MAX {
            return Err(ParamsError::FoldBoundExceeded);
        }
        if !is_generator(generator) {
            return Err(ParamsError::GeneratorOrderNotFull);
        }
        Ok(Self {
            shape,
            q,
            generator,
        })
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn q(&self) -> u128 {
        self.q
    }

    pub fn generator(&self) -> Gf {
        self.generator
    }

    /// The largest fold the verifier may accept, `k_1 (Q - 1)`.
    pub fn fold_bound(&self) -> u128 {
        (self.shape.rows() as u128) * (self.q - 1)
    }
}

/// Their 48-byte frame: `log_rows`, `log_columns` (u64 LE), `Q` (u128 LE),
/// the generator (16 bytes).
impl Encoding<[u8]> for BitZParams {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut frame = [0u8; 48];
        frame[..8].copy_from_slice(&(self.shape.log_rows() as u64).to_le_bytes());
        frame[8..16].copy_from_slice(&(self.shape.log_columns() as u64).to_le_bytes());
        frame[16..32].copy_from_slice(&self.q.to_le_bytes());
        frame[32..].copy_from_slice(&gf_to_bytes(self.generator));
        frame
    }
}

/// A Merkle root over the committed codeword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Root(pub [u8; 32]);

/// A claim one of the pre-transcript checks rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    RowWeightCountMismatch,
    ColumnWeightCountMismatch,
}

/// Their `LinearClaim<Fq<Q>>`: the caller's `x_core`, weights as canonical
/// residues below `q` and the value they are claimed to give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearClaim {
    row_weights: Vec<u128>,
    column_weights: Vec<u128>,
    target: u128,
}

impl LinearClaim {
    pub fn new(
        params: &BitZParams,
        row_weights: Vec<u128>,
        column_weights: Vec<u128>,
        target: u128,
    ) -> Result<Self, ClaimError> {
        let shape = params.shape();
        if row_weights.len() != shape.rows() {
            return Err(ClaimError::RowWeightCountMismatch);
        }
        if column_weights.len() != shape.columns() {
            return Err(ClaimError::ColumnWeightCountMismatch);
        }
        Ok(Self {
            row_weights,
            column_weights,
            target,
        })
    }

    /// The canonical representatives the fold exponentiates (the residues
    /// themselves, already lifted).
    pub fn row_exponents(&self) -> &[u128] {
        &self.row_weights
    }

    pub fn row_weights(&self) -> &[u128] {
        &self.row_weights
    }

    pub fn column_weights(&self) -> &[u128] {
        &self.column_weights
    }

    pub fn target(&self) -> u128 {
        self.target
    }
}

/// Each weight vector with a little-endian `u64` length, then the target.
impl Encoding<[u8]> for LinearClaim {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut bytes =
            Vec::with_capacity(16 * (self.row_weights.len() + self.column_weights.len() + 2));
        for weights in [&self.row_weights, &self.column_weights] {
            bytes.extend_from_slice(&(weights.len() as u64).to_le_bytes());
            for &weight in weights {
                bytes.extend_from_slice(FqWire(weight).encode().as_ref());
            }
        }
        bytes.extend_from_slice(FqWire(self.target).encode().as_ref());
        bytes
    }
}

/// Their `LinearClaim<F128>`: the factored inner-product claim the opening
/// scheme discharges. Bit `column * rows + row` has weight
/// `row_weights[row] * column_weights[column]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearClaimGf {
    row_weights: Vec<Gf>,
    column_weights: Vec<Gf>,
    target: Gf,
}

impl LinearClaimGf {
    pub fn from_shape(
        shape: &Shape,
        row_weights: Vec<Gf>,
        column_weights: Vec<Gf>,
        target: Gf,
    ) -> Result<Self, ClaimError> {
        if row_weights.len() != shape.rows() {
            return Err(ClaimError::RowWeightCountMismatch);
        }
        if column_weights.len() != shape.columns() {
            return Err(ClaimError::ColumnWeightCountMismatch);
        }
        Ok(Self {
            row_weights,
            column_weights,
            target,
        })
    }

    pub fn row_weights(&self) -> &[Gf] {
        &self.row_weights
    }

    pub fn column_weights(&self) -> &[Gf] {
        &self.column_weights
    }

    pub fn target(&self) -> Gf {
        self.target
    }
}

impl Encoding<[u8]> for LinearClaimGf {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut bytes =
            Vec::with_capacity(16 * (self.row_weights.len() + self.column_weights.len() + 2));
        for weights in [&self.row_weights, &self.column_weights] {
            bytes.extend_from_slice(&(weights.len() as u64).to_le_bytes());
            for &weight in weights {
                bytes.extend_from_slice(&gf_to_bytes(weight));
            }
        }
        bytes.extend_from_slice(&gf_to_bytes(self.target));
        bytes
    }
}
