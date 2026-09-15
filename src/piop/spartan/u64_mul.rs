//! Integer-level R1CS relation for batched `u64 * u64 = u128` multiplication.
//!
//! Each live row proves `x · y = z_lo + 2^64 · z_hi` over the integers for
//! `x, y, z_lo, z_hi < 2^64`. Spartan sees the exact integer assignment
//! `[e0 | x | y | z_lo | z_hi]` (five blocks, zero-padded to eight for the
//! assignment MLE); the product limbs are recombined by matrix `C` with the
//! public coefficient `2^64`, exactly like the BabyBear relation recombines
//! `c + p·k`. The 64/64/64/64-bit representation is materialized separately,
//! in the 256-slot-per-gate layout expected by the F2Z commitment, so bit
//! variables never become part of the R1CS statement.
//!
//! Unlike the u32 relation, a row's products (`x · y` and `z_lo + 2^64 z_hi`,
//! both below `2^128`) do not fit the native `u64` first-round path, so the
//! Spartan PIOP runs on values projected into the runtime field.

use crate::piop::spartan::SpartanField as _;
use field::RingOps;
use std::borrow::Cow;

use thiserror::Error;

use crate::{
    pcs::IntegerMatrixLayout,
    poly::mle::DenseMultilinearExtension,
    sparse_matrix::SparseColumn,
    utils::{cfg_iter, cfg_iter_mut},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    ConstraintMatrices, ModulusIndependentCoefficient, PreparedConstraintMatrices, R1csProductMles,
    SparseMatrix, SpartanF2zField, SpartanField, SpartanMatrixCoefficient, SpartanMatrixError,
    SpartanRelationBackend, slot_rows::pack_slot_major_rows_w1_256,
};

/// Number of committed little-endian bits used for each of `x`, `y`,
/// `z_lo`, and `z_hi`.
pub const U64_MUL_VALUE_BITS: usize = 64;
/// First bit slot occupied by the left operand.
pub const U64_MUL_X_SLOT_START: usize = 0;
/// First bit slot occupied by the right operand.
pub const U64_MUL_Y_SLOT_START: usize = U64_MUL_X_SLOT_START + U64_MUL_VALUE_BITS;
/// First bit slot occupied by the low product limb.
pub const U64_MUL_Z_LO_SLOT_START: usize = U64_MUL_Y_SLOT_START + U64_MUL_VALUE_BITS;
/// First bit slot occupied by the high product limb.
pub const U64_MUL_Z_HI_SLOT_START: usize = U64_MUL_Z_LO_SLOT_START + U64_MUL_VALUE_BITS;
/// Total number of bit slots committed for each multiplication (all of them
/// carry witness bits: there is no padding slot).
pub const U64_MUL_BIT_SLOTS: usize = 4 * U64_MUL_VALUE_BITS;
/// `log2` of [`U64_MUL_BIT_SLOTS`]: the physical slot coordinates on the
/// folded row axis.
pub const U64_MUL_SLOT_VARS: usize = 8;
/// The public limb base `2^64` recombining the product limbs in matrix `C`.
pub const U64_MUL_LIMB_BASE: u128 = 1 << U64_MUL_VALUE_BITS;

/// Little-endian canonical encoding of [`U64_MUL_LIMB_BASE`] as an element of
/// every accepted (at least 100-bit) Spartan field.
const LIMB_BASE_FIELD_ENCODING: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Five logical assignment blocks: `[e0 | x | y | z_lo | z_hi]`.
pub(super) const U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS: usize = 5;
/// The assignment MLE pads the five logical blocks to eight.
pub(super) const U64_MUL_PADDED_ASSIGNMENT_BLOCKS: usize = 8;
// Keep even small relation fixtures in the geometry accepted by the F2Z row
// packer. The combined production proof applies its stricter 2^15 minimum.
const MIN_CAPACITY: usize = 1 << 8;

/// Compact coefficients used by the u64 multiplication matrices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum U64MulCoefficient {
    /// The field element one.
    One,
    /// The public limb base `2^64`.
    LimbBase,
}

impl SpartanMatrixCoefficient<SpartanF2zField> for U64MulCoefficient {
    fn validate(&self, _field_modulus_encoding: &[u8]) -> Result<(), SpartanMatrixError> {
        // Every Spartan field is at least 100 bits, so both public coefficients
        // are nonzero canonical elements in every accepted configuration.
        Ok(())
    }

    fn is_zero(&self) -> bool {
        false
    }

    fn canonical_field_encoding<'a>(
        &'a self,
        _field_config: &<SpartanF2zField as crate::piop::spartan::SpartanField>::Config,
        field_one_encoding: &'a [u8],
    ) -> Cow<'a, [u8]> {
        match self {
            Self::One => Cow::Borrowed(field_one_encoding),
            Self::LimbBase => Cow::Borrowed(&LIMB_BASE_FIELD_ENCODING),
        }
    }

    fn scale(
        &self,
        value: &SpartanF2zField,
        field_config: &<SpartanF2zField as crate::piop::spartan::SpartanField>::Config,
    ) -> SpartanF2zField {
        match self {
            Self::One => value.clone(),
            Self::LimbBase => {
                let coefficient = SpartanF2zField::from_with_cfg(U64_MUL_LIMB_BASE, field_config);
                let mut scaled = value.clone();
                scaled = field_config.mul(&(scaled), &(&coefficient));
                scaled
            }
        }
    }

    fn column_dot(
        column: SparseColumn<'_, Self>,
        row_weights: &[SpartanF2zField],
        zero: &SpartanF2zField,
        field_config: &<SpartanF2zField as crate::piop::spartan::SpartanField>::Config,
    ) -> SpartanF2zField {
        if let Some((row, coefficient)) = column.single() {
            return coefficient.scale(&row_weights[row], field_config);
        }

        let mut evaluation = zero.clone();
        for (row, coefficient) in column {
            evaluation = field_config.add(
                &(evaluation),
                &(&coefficient.scale(&row_weights[row], field_config)),
            );
        }
        evaluation
    }
}

/// Both public coefficients encode to modulus-independent bytes: `One` to
/// the field's canonical one (exactly like a Bit `true`) and `LimbBase`
/// to `2^64`, which is canonical and never the unit in any accepted (at
/// least 100-bit) Spartan field.
impl ModulusIndependentCoefficient<SpartanF2zField> for U64MulCoefficient {
    fn write_modulus_independent_encoding(&self, out: &mut Vec<u8>) {
        match self {
            Self::One => {
                ModulusIndependentCoefficient::<SpartanF2zField>::write_modulus_independent_encoding(
                    &true, out,
                )
            }
            Self::LimbBase => out.extend_from_slice(&LIMB_BASE_FIELD_ENCODING),
        }
    }

    fn is_unit(&self) -> bool {
        matches!(self, Self::One)
    }
}

/// Integer relation backend used by the u64 multiplication prover.
///
/// The witness entries are exact `u64` values; the per-row products are
/// `u128` values, which is why this relation has no native first-round path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct U64MulRelationBackend;

impl SpartanRelationBackend<SpartanF2zField> for U64MulRelationBackend {
    type MatrixCoeff = U64MulCoefficient;
    type Witness = u64;
    type Product = u128;
}

/// Failures while constructing the u64 multiplication relation.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum U64MulError {
    /// A multiplication batch must contain at least one live row.
    #[error("a u64 multiplication batch must not be empty")]
    EmptyBatch,

    /// The padded assignment or committed-bit domain does not fit in `usize`.
    #[error("the u64 multiplication domain is too large")]
    DomainTooLarge,

    /// The generated relation or projected witness is malformed.
    #[error(transparent)]
    SpartanMatrix(#[from] SpartanMatrixError),
}

/// Shared shape of the integer assignment and its compact F2Z bit witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U64MulLayout {
    multiplications: usize,
    capacity: usize,
    gate_vars: usize,
    /// Experiment hook: moves this many gate variables from the F2Z row side
    /// to the column side relative to the default split `s = gate_vars / 2`.
    /// Zero in production; see [`Self::with_split_shift`].
    split_shift: i8,
}

impl U64MulLayout {
    /// Creates a layout for `multiplications` live rows.
    ///
    /// The gate capacity is `max(256, multiplications).next_power_of_two()`.
    /// The combined Spartan/F2Z production API additionally requires at least
    /// `2^15` slots so it can use a validator-gated Ligerito profile.
    pub fn new(multiplications: usize) -> Result<Self, U64MulError> {
        if multiplications == 0 {
            return Err(U64MulError::EmptyBatch);
        }

        let capacity = multiplications
            .max(MIN_CAPACITY)
            .checked_next_power_of_two()
            .ok_or(U64MulError::DomainTooLarge)?;

        capacity
            .checked_mul(U64_MUL_PADDED_ASSIGNMENT_BLOCKS)
            .and_then(|_| capacity.checked_mul(U64_MUL_BIT_SLOTS))
            .ok_or(U64MulError::DomainTooLarge)?;

        let gate_vars = capacity.trailing_zeros() as usize;
        Ok(Self {
            multiplications,
            capacity,
            gate_vars,
            split_shift: 0,
        })
    }

    /// The same layout with the F2Z column-variable count moved by `shift`
    /// from the default `gate_vars / 2` (positive: more columns, fewer rows).
    /// The total `t + s` is unchanged, so the Ligerito instance is too; only
    /// the forest/read-off/verifier split moves. A measurement hook, not a
    /// production setting.
    pub fn with_split_shift(self, shift: i8) -> Result<Self, U64MulError> {
        let s = i64::try_from(self.gate_vars / 2).expect("small") + i64::from(shift);
        if s < 0 || s > i64::try_from(self.gate_vars).expect("small") {
            return Err(U64MulError::DomainTooLarge);
        }
        Ok(Self {
            split_shift: shift,
            ..self
        })
    }

    /// F2Z column variables: `max(gate_vars / 2, gate_vars − 10)`, i.e. the
    /// balanced split with the row side capped at `t = 18`, plus the
    /// experiment shift.
    ///
    /// Measured 2026-09-11 (Apple M5, 8 threads, 2^20–2^22): the forest is
    /// flat in the split for `t ≤ 18` and about 17 % slower at `t = 19`
    /// (2^21: 903 → 751 ms, 2^22: 1762 → 1479 ms when one gate variable moves
    /// to the column side), with the verifier's O(2^t) fold shrinking too.
    /// The price is the in-the-clear read-off, which doubles per extra column
    /// variable: +7 % proof bytes at 2^21, +12 % at 2^22. Below 2^21 the two
    /// rules coincide, so those proofs are unchanged.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
    pub const fn col_vars(&self) -> usize {
        let half = self.gate_vars / 2;
        let capped = self.gate_vars.saturating_sub(10);
        let base = if capped > half { capped } else { half };
        (base as i64 + self.split_shift as i64) as usize
    }

    /// Number of live multiplication rows.
    pub const fn multiplications(&self) -> usize {
        self.multiplications
    }

    /// Power-of-two gate capacity, including zero-padded gates.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of variables selecting one padded gate.
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }

    /// Logical assignment length: five blocks of `capacity` integers.
    pub const fn assignment_len(&self) -> usize {
        U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS * self.capacity
    }

    /// Assignment-MLE length after padding the five logical blocks to eight.
    pub const fn padded_assignment_len(&self) -> usize {
        U64_MUL_PADDED_ASSIGNMENT_BLOCKS * self.capacity
    }

    /// Number of variables in the padded assignment MLE.
    pub const fn assignment_vars(&self) -> usize {
        self.gate_vars + 3
    }

    /// F2Z shape for the slot-major 64/64/64/64-bit witness.
    ///
    /// If `g = log2(capacity)`, the low `s = floor(g/2)` gate coordinates
    /// become F2Z columns. The remaining gate coordinates and the eight
    /// physical slot coordinates become folded row variables, so the
    /// committed tensor has `g + 8` variables (one more than the 128-slot
    /// relations at the same gate count).
    pub const fn f2z_params(&self) -> IntegerMatrixLayout {
        let s = self.col_vars();
        IntegerMatrixLayout {
            row_vars: U64_MUL_SLOT_VARS + self.gate_vars - s,
            col_vars: s,
            word_bits: 1,
        }
    }

    /// Maps `(bit_slot, gate)` to the F2Z row-major cell `(b, c)`.
    ///
    /// `params.cell_index(b, c) == bit_slot * capacity + gate`.
    pub const fn f2z_cell(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        if bit_slot >= U64_MUL_BIT_SLOTS || gate >= self.capacity {
            return None;
        }

        let s = self.col_vars();
        let column_mask = (1usize << s) - 1;
        let gate_high = gate >> s;
        let b = (bit_slot << (self.gate_vars - s)) | gate_high;
        let c = gate & column_mask;
        Some((b, c))
    }
}

/// Exact native assignment for a batch of u64 multiplications.
///
/// The assignment is logically `[e0 | x | y | z_lo | z_hi]`. Only the first
/// constant entry is one; unused gates in every other block are zero. The
/// product limbs come from the exact `u128` product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U64MulWitness {
    layout: U64MulLayout,
    assignment: Box<[u64]>,
}

impl U64MulWitness {
    /// Constructs the exact assignment from explicit operand pairs.
    pub fn from_inputs(inputs: &[(u64, u64)]) -> Result<Self, U64MulError> {
        Self::from_fn(inputs.len(), |index| inputs[index])
    }

    /// Constructs the exact assignment without retaining a separate input
    /// vector, which is useful for large deterministic benchmark batches.
    #[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
    pub fn from_fn(
        multiplications: usize,
        mut input: impl FnMut(usize) -> (u64, u64),
    ) -> Result<Self, U64MulError> {
        let layout = U64MulLayout::new(multiplications)?;
        let capacity = layout.capacity;
        let mut assignment = vec![0_u64; layout.assignment_len()];
        assignment[0] = 1;

        for index in 0..multiplications {
            let (x, y) = input(index);
            let product = u128::from(x) * u128::from(y);
            assignment[capacity + index] = x;
            assignment[2 * capacity + index] = y;
            assignment[3 * capacity + index] = product as u64;
            assignment[4 * capacity + index] = (product >> U64_MUL_VALUE_BITS) as u64;
        }

        Ok(Self {
            layout,
            assignment: assignment.into_boxed_slice(),
        })
    }

    /// The same assignment under a layout whose F2Z split is shifted; see
    /// [`U64MulLayout::with_split_shift`]. The assignment itself depends only
    /// on the capacity, so nothing is recomputed.
    pub fn with_split_shift(mut self, shift: i8) -> Result<Self, U64MulError> {
        self.layout = self.layout.with_split_shift(shift)?;
        Ok(self)
    }

    /// Shape shared by this assignment and its bit representation.
    pub const fn layout(&self) -> &U64MulLayout {
        &self.layout
    }

    /// Complete block-aligned logical integer assignment (five blocks).
    pub fn assignment(&self) -> &[u64] {
        &self.assignment
    }

    /// Padded left-operand block.
    pub fn x_values(&self) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[capacity..2 * capacity]
    }

    /// Padded right-operand block.
    pub fn y_values(&self) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[2 * capacity..3 * capacity]
    }

    /// Padded low-limb block of the products.
    pub fn z_lo_values(&self) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[3 * capacity..4 * capacity]
    }

    /// Padded high-limb block of the products.
    pub fn z_hi_values(&self) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[4 * capacity..5 * capacity]
    }

    /// The exact `u128` product of live row `index`.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn product(&self, index: usize) -> u128 {
        u128::from(self.z_lo_values()[index])
            | (u128::from(self.z_hi_values()[index]) << U64_MUL_VALUE_BITS)
    }

    /// Builds the compact F2Z rows without materializing a cell tensor.
    ///
    /// Row `c` is 256 lanes of `high_gate_count` bits: bit `gate_high` of
    /// lane `slot` is slot `slot` of gate `(gate_high << s) | c`. Whenever a
    /// lane spans whole words (`high_gate_count % 64 == 0`, every production
    /// layout) the rows are built by the block transposes of
    /// [`super::slot_rows`]; smaller layouts take the bitwise path. Both
    /// produce identical rows.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn f2z_bit_rows(&self) -> Vec<Vec<u64>> {
        let params = self.layout.f2z_params();
        let words_per_row = params.rows() / u64::BITS as usize;
        let mut rows = vec![vec![0_u64; words_per_row]; params.cols()];

        let s = self.layout.col_vars();
        let high_gate_count = 1_usize << (self.layout.gate_vars - s);
        if high_gate_count.is_multiple_of(u64::BITS as usize) {
            let x = self.x_values();
            let y = self.y_values();
            let z_lo = self.z_lo_values();
            let z_hi = self.z_hi_values();
            pack_slot_major_rows_w1_256(
                &mut rows,
                s,
                high_gate_count,
                self.layout.multiplications,
                |gate| [x[gate], y[gate], z_lo[gate], z_hi[gate]],
            );
        } else {
            self.write_bit_rows_bitwise(&mut rows);
        }
        rows
    }

    /// Reference packing: one masked read-modify-write per committed bit.
    #[allow(clippy::arithmetic_side_effects)]
    fn write_bit_rows_bitwise(&self, rows: &mut [Vec<u64>]) {
        for gate in 0..self.layout.multiplications {
            for (slot_offset, value) in [
                (U64_MUL_X_SLOT_START, self.x_values()[gate]),
                (U64_MUL_Y_SLOT_START, self.y_values()[gate]),
                (U64_MUL_Z_LO_SLOT_START, self.z_lo_values()[gate]),
                (U64_MUL_Z_HI_SLOT_START, self.z_hi_values()[gate]),
            ] {
                for bit in 0..U64_MUL_VALUE_BITS {
                    if value & (1_u64 << bit) == 0 {
                        continue;
                    }
                    let (b, c) = self
                        .layout
                        .f2z_cell(slot_offset + bit, gate)
                        .expect("witness bit coordinates are in bounds");
                    rows[c][b / u64::BITS as usize] |= 1_u64 << (b % u64::BITS as usize);
                }
            }
        }
    }

    /// Moves out the layout and logical assignment.
    pub fn into_parts(self) -> (U64MulLayout, Box<[u64]>) {
        (self.layout, self.assignment)
    }
}

/// Builds the compact CSC matrices for the u64 integer relation.
///
/// For live row `i` and capacity `M`, the only nonzero entries are
///
/// ```text
/// A[i, M+i]  = 1
/// B[i, 2M+i] = 1
/// C[i, 3M+i] = 1
/// C[i, 4M+i] = 2^64.
/// ```
pub fn u64_mul_constraint_matrices(
    layout: &U64MulLayout,
) -> Result<ConstraintMatrices<U64MulCoefficient>, U64MulError> {
    let a = selector_matrix(layout, 1)?;
    let b = selector_matrix(layout, 2)?;
    let c = output_matrix(layout)?;
    Ok(ConstraintMatrices::new(a, b, c)?)
}

#[allow(clippy::arithmetic_side_effects)]
fn selector_matrix(
    layout: &U64MulLayout,
    block: usize,
) -> Result<SparseMatrix<U64MulCoefficient>, SpartanMatrixError> {
    let columns = layout.assignment_len();
    let rows = layout.multiplications;
    let offset = block * layout.capacity;

    let mut column_offsets = vec![0; columns + 1];
    for (row, boundary) in column_offsets[offset + 1..offset + rows + 1]
        .iter_mut()
        .enumerate()
    {
        *boundary = row + 1;
    }
    column_offsets[offset + rows + 1..].fill(rows);
    let entries = (0..rows)
        .map(|row| (row, U64MulCoefficient::One))
        .collect::<Vec<_>>();

    Ok(SparseMatrix::try_from_csc(rows, column_offsets, entries)?)
}

#[allow(clippy::arithmetic_side_effects)]
fn output_matrix(
    layout: &U64MulLayout,
) -> Result<SparseMatrix<U64MulCoefficient>, SpartanMatrixError> {
    let columns = layout.assignment_len();
    let rows = layout.multiplications;
    let lo_offset = 3 * layout.capacity;
    let hi_offset = 4 * layout.capacity;

    let mut column_offsets = vec![0; columns + 1];
    for (row, boundary) in column_offsets[lo_offset + 1..lo_offset + rows + 1]
        .iter_mut()
        .enumerate()
    {
        *boundary = row + 1;
    }
    column_offsets[lo_offset + rows + 1..hi_offset + 1].fill(rows);
    for (row, boundary) in column_offsets[hi_offset + 1..hi_offset + rows + 1]
        .iter_mut()
        .enumerate()
    {
        *boundary = rows + row + 1;
    }
    column_offsets[hi_offset + rows + 1..].fill(2 * rows);

    let mut entries = Vec::with_capacity(2 * rows);
    entries.extend((0..rows).map(|row| (row, U64MulCoefficient::One)));
    entries.extend((0..rows).map(|row| (row, U64MulCoefficient::LimbBase)));
    Ok(SparseMatrix::try_from_csc(rows, column_offsets, entries)?)
}

/// Generates and prepares the compact u64 matrices over the Spartan/F2Z
/// field.
pub fn prepare_u64_mul_relation(
    layout: U64MulLayout,
    field_config: &<SpartanF2zField as crate::piop::spartan::SpartanField>::Config,
) -> Result<PreparedConstraintMatrices<SpartanF2zField, U64MulCoefficient>, U64MulError> {
    let matrices = u64_mul_constraint_matrices(&layout)?;
    Ok(PreparedConstraintMatrices::new(matrices, field_config)?)
}

/// Projects the exact assignment and products into a Spartan field.
///
/// Every assignment entry is below `2^64` and every product below `2^128`;
/// both are reduced modulo the field's prime, which is the Step-2 projection
/// of the integer relation. The tables are written straight into their padded
/// Spartan layouts (eight assignment blocks, power-of-two product rows) and
/// the conversions run in parallel: at `2^21` multiplications this is the
/// difference between a 0.6 s and a sub-0.1 s projection.
#[allow(clippy::arithmetic_side_effects)]
pub fn project_u64_mul_witness<F>(
    witness: &U64MulWitness,
    field_config: &F::Config,
) -> Result<(DenseMultilinearExtension<F>, R1csProductMles<F>), U64MulError>
where
    F: SpartanField + Send + Sync,
    F::Config: Sync,
{
    F::validate_config(field_config).map_err(SpartanMatrixError::from)?;
    const MIN_LEN: usize = 4096;

    let layout = witness.layout;
    let capacity = layout.capacity;
    let live = layout.multiplications;
    let zero = F::zero_with_cfg(field_config);

    // Assignment MLE: `[e0 | x | y | z_lo | z_hi | 0 | 0 | 0]`, only the live
    // gates of the four value blocks are converted.
    let mut assignment = vec![zero.clone(); layout.padded_assignment_len()];
    assignment[0] = F::one_with_cfg(field_config);
    for (block, values) in [
        (1, witness.x_values()),
        (2, witness.y_values()),
        (3, witness.z_lo_values()),
        (4, witness.z_hi_values()),
    ] {
        let target = &mut assignment[block * capacity..block * capacity + live];
        cfg_iter_mut!(target, MIN_LEN)
            .zip(cfg_iter!(values[..live], MIN_LEN))
            .for_each(|(slot, &value)| *slot = F::from_with_cfg(value, field_config));
    }

    // Products: `Az = x`, `Bz = y` are the converted operand blocks; `Cz` is
    // the exact 128-bit product reduced into the field.
    let product_len = live.next_power_of_two();
    let product_vars = product_len.ilog2() as usize;
    let mut az = vec![zero.clone(); product_len];
    az[..live].clone_from_slice(&assignment[capacity..capacity + live]);
    let mut bz = vec![zero.clone(); product_len];
    bz[..live].clone_from_slice(&assignment[2 * capacity..2 * capacity + live]);
    let mut cz = vec![zero; product_len];
    let z_lo = witness.z_lo_values();
    let z_hi = witness.z_hi_values();
    cfg_iter_mut!(cz[..live], MIN_LEN)
        .zip(cfg_iter!(z_lo[..live], MIN_LEN))
        .zip(cfg_iter!(z_hi[..live], MIN_LEN))
        .for_each(|((slot, &lo), &hi)| {
            let product = u128::from(lo) | (u128::from(hi) << U64_MUL_VALUE_BITS);
            *slot = F::from_with_cfg(product, field_config);
        });

    let mle = |evaluations: Vec<F>, num_vars: usize| DenseMultilinearExtension {
        evaluations,
        num_vars,
    };
    Ok((
        mle(assignment, layout.assignment_vars()),
        R1csProductMles {
            az: mle(az, product_vars),
            bz: mle(bz, product_vars),
            cz: mle(cz, product_vars),
        },
    ))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::piop::spartan::spartan_f2z_field_config;

    fn inputs(n: usize) -> Vec<(u64, u64)> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        (0..n)
            .map(|index| {
                let mut next = || {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state
                };
                match index % 5 {
                    0 => (u64::MAX, u64::MAX),
                    1 => (0, next()),
                    2 => (next(), 1),
                    _ => (next(), next()),
                }
            })
            .collect()
    }

    #[test]
    fn layout_shapes() {
        let layout = U64MulLayout::new(1000).unwrap();
        assert_eq!(layout.capacity(), 1024);
        assert_eq!(layout.gate_vars(), 10);
        assert_eq!(layout.assignment_len(), 5 * 1024);
        assert_eq!(layout.padded_assignment_len(), 8 * 1024);
        assert_eq!(layout.assignment_vars(), 13);
        let p = layout.f2z_params();
        assert_eq!((p.row_vars, p.col_vars, p.word_bits), (8 + 5, 5, 1));
        assert_eq!(p.rows() * p.cols(), U64_MUL_BIT_SLOTS * layout.capacity());
        assert_eq!(U64MulLayout::new(1).unwrap().capacity(), MIN_CAPACITY);
        assert_eq!(U64MulLayout::new(0), Err(U64MulError::EmptyBatch));
        assert_eq!(U64_MUL_BIT_SLOTS, 1 << U64_MUL_SLOT_VARS);
        assert_eq!(
            u128::from_le_bytes(LIMB_BASE_FIELD_ENCODING),
            U64_MUL_LIMB_BASE
        );
    }

    #[test]
    fn witness_products_are_exact() {
        let inputs = inputs(37);
        let witness = U64MulWitness::from_inputs(&inputs).unwrap();
        assert_eq!(witness.assignment()[0], 1);
        for (index, &(x, y)) in inputs.iter().enumerate() {
            assert_eq!(witness.x_values()[index], x);
            assert_eq!(witness.y_values()[index], y);
            assert_eq!(witness.product(index), u128::from(x) * u128::from(y));
        }
        assert!(witness.x_values()[37..].iter().all(|&v| v == 0));
        assert!(witness.z_hi_values()[37..].iter().all(|&v| v == 0));
        assert_eq!(
            witness.product(0),
            u128::from(u64::MAX) * u128::from(u64::MAX)
        );
    }

    #[test]
    fn bit_rows_match_cell_map_and_transposed_packer() {
        // 2^12 gates: s = 6, h = 6, high_gate_count = 64 → the transposed
        // packer runs; compare with the bitwise reference on the same witness.
        let inputs = inputs(3000);
        let witness = U64MulWitness::from_inputs(&inputs).unwrap();
        let layout = *witness.layout();
        assert_eq!(layout.gate_vars(), 12);
        let rows = witness.f2z_bit_rows();
        let params = layout.f2z_params();
        let mut reference = vec![vec![0_u64; params.rows() / 64]; params.cols()];
        witness.write_bit_rows_bitwise(&mut reference);
        assert_eq!(rows, reference);

        let bit = |slot: usize, gate: usize| {
            let (b, c) = layout.f2z_cell(slot, gate).unwrap();
            (rows[c][b / 64] >> (b % 64)) & 1
        };
        for (gate, &(x, y)) in inputs.iter().enumerate().take(300) {
            let product = u128::from(x) * u128::from(y);
            for j in 0..64 {
                assert_eq!(bit(U64_MUL_X_SLOT_START + j, gate), (x >> j) & 1);
                assert_eq!(bit(U64_MUL_Y_SLOT_START + j, gate), (y >> j) & 1);
                assert_eq!(
                    bit(U64_MUL_Z_LO_SLOT_START + j, gate),
                    ((product >> j) & 1) as u64
                );
                assert_eq!(
                    bit(U64_MUL_Z_HI_SLOT_START + j, gate),
                    ((product >> (64 + j)) & 1) as u64
                );
            }
        }
        // Padding gates are all-zero.
        assert_eq!(bit(U64_MUL_X_SLOT_START, 3500), 0);
    }

    #[test]
    fn small_layout_uses_bitwise_path() {
        let inputs = inputs(300);
        let witness = U64MulWitness::from_inputs(&inputs).unwrap();
        let layout = *witness.layout();
        assert_eq!(layout.gate_vars(), 9);
        let rows = witness.f2z_bit_rows();
        let (b, c) = layout.f2z_cell(U64_MUL_Y_SLOT_START + 3, 7).unwrap();
        assert_eq!((rows[c][b / 64] >> (b % 64)) & 1, (inputs[7].1 >> 3) & 1);
    }

    #[test]
    fn matrices_select_the_expected_columns() {
        let layout = U64MulLayout::new(300).unwrap();
        let matrices = u64_mul_constraint_matrices(&layout).unwrap();
        let capacity = layout.capacity();
        assert_eq!(matrices.a().row_count(), 300);
        assert_eq!(matrices.a().column_count(), layout.assignment_len());
        for row in [0, 1, 299] {
            let single = |m: &SparseMatrix<U64MulCoefficient>, column: usize| {
                let (row, coefficient) = m
                    .column(column)
                    .expect("column in range")
                    .single()
                    .expect("one entry");
                (row, *coefficient)
            };
            assert_eq!(
                single(matrices.a(), capacity + row),
                (row, U64MulCoefficient::One)
            );
            assert_eq!(
                single(matrices.b(), 2 * capacity + row),
                (row, U64MulCoefficient::One)
            );
            assert_eq!(
                single(matrices.c(), 3 * capacity + row),
                (row, U64MulCoefficient::One)
            );
            assert_eq!(
                single(matrices.c(), 4 * capacity + row),
                (row, U64MulCoefficient::LimbBase)
            );
        }
        assert!(
            matrices
                .a()
                .column(capacity + 300)
                .unwrap()
                .single()
                .is_none()
        );
        assert!(matrices.c().column(0).unwrap().single().is_none());
    }

    #[test]
    fn projection_satisfies_the_relation_in_the_field() {
        let inputs = inputs(300);
        let witness = U64MulWitness::from_inputs(&inputs).unwrap();
        let config = spartan_f2z_field_config();
        let (assignment, products) =
            project_u64_mul_witness::<SpartanF2zField>(&witness, &config).unwrap();
        assert_eq!(
            assignment.evaluations.len(),
            witness.layout().padded_assignment_len()
        );
        let base = SpartanF2zField::from_with_cfg(U64_MUL_LIMB_BASE, &config);
        for index in 0..300 {
            let mut lhs = products.az.evaluations[index].clone();
            lhs = config.mul(&(lhs), &(&products.bz.evaluations[index]));
            assert_eq!(lhs, products.cz.evaluations[index]);
            let capacity = witness.layout().capacity();
            let mut recombined = assignment.evaluations[4 * capacity + index].clone();
            recombined = config.mul(&(recombined), &(&base));
            recombined = config.add(
                &(recombined),
                &(&assignment.evaluations[3 * capacity + index]),
            );
            assert_eq!(recombined, products.cz.evaluations[index]);
        }
        // The prepared matrices accept the coefficient encoding.
        let layout = *witness.layout();
        let prepared = prepare_u64_mul_relation(layout, &config).unwrap();
        assert_eq!(prepared.matrices().row_count(), 300);
    }
}
