//! Integer-level R1CS relation for batched `u32 * u32 = u64` multiplication.
//!
//! Spartan sees only integer-valued assignment entries. The 32/32/64-bit
//! representation is materialized separately, in the layout expected by the
//! F2Z commitment, so bit variables never become part of the R1CS statement.

use thiserror::Error;

use crate::{pcs::IntEvalParams, poly::mle::DenseMultilinearExtension};

use super::{
    ConstraintMatrices, PreparedConstraintMatrices, R1csProductMles, SparseMatrix, SpartanField,
    SpartanMatrixError, SpartanRelationBackend,
    slot_rows::{pack_slot_major_rows_w1, pack_slot_major_rows_w8},
};

/// Number of committed little-endian bits used for each left operand.
pub const U32_MUL_X_BITS: usize = 32;
/// Number of committed little-endian bits used for each right operand.
pub const U32_MUL_Y_BITS: usize = 32;
/// Number of committed little-endian bits used for each product.
pub const U32_MUL_PRODUCT_BITS: usize = 64;
/// First bit slot occupied by the left operand.
pub const U32_MUL_X_SLOT_START: usize = 0;
/// First bit slot occupied by the right operand.
pub const U32_MUL_Y_SLOT_START: usize = U32_MUL_X_SLOT_START + U32_MUL_X_BITS;
/// First bit slot occupied by the product.
pub const U32_MUL_PRODUCT_SLOT_START: usize = U32_MUL_Y_SLOT_START + U32_MUL_Y_BITS;
/// Total number of bit slots committed for each multiplication.
pub const U32_MUL_BIT_SLOTS: usize = U32_MUL_X_BITS + U32_MUL_Y_BITS + U32_MUL_PRODUCT_BITS;

const ASSIGNMENT_BLOCKS: usize = 4;
// Keep even small relation fixtures in the geometry accepted by the F2Z row
// packer. The combined production proof applies its stricter 2^15 minimum.
const MIN_CAPACITY: usize = 1 << 8;

/// Logical F2Z word width used to pack the 128 committed bits for each
/// multiplication.
///
/// `W1` retains the original one-bit-cell layout. `W8` packs each consecutive
/// group of eight global bit slots into one little-endian byte-sized cell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum U32MulF2zWidth {
    /// One committed bit per logical F2Z cell.
    #[default]
    W1 = 1,
    /// Eight committed bits per logical F2Z cell.
    W8 = 8,
}

impl U32MulF2zWidth {
    /// Number of committed bits in each logical F2Z cell.
    pub const fn word_bits(self) -> usize {
        self as usize
    }
}

/// Integer relation backend used by the u32 multiplication Spartan prover.
///
/// Boolean selector matrices act on an exact u64 assignment and produce exact
/// u64 matrix products. Field conversion is deferred to the first sumcheck
/// fold boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct U32MulRelationBackend;

impl<F> SpartanRelationBackend<F> for U32MulRelationBackend
where
    F: SpartanField,
{
    type MatrixCoeff = bool;
    type Witness = u64;
    type Product = u64;
}

/// Failures while constructing the integer multiplication relation.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum U32MulError {
    /// A multiplication batch must contain at least one live row.
    #[error("a u32 multiplication batch must not be empty")]
    EmptyBatch,

    /// The padded assignment or committed-bit domain does not fit in `usize`.
    #[error("the u32 multiplication domain is too large")]
    DomainTooLarge,

    /// The generated relation or projected witness is malformed.
    #[error(transparent)]
    SpartanMatrix(#[from] SpartanMatrixError),
}

/// Shared shape of the integer assignment and its compact F2Z bit witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U32MulLayout {
    multiplications: usize,
    capacity: usize,
    gate_vars: usize,
    f2z_width: U32MulF2zWidth,
}

impl U32MulLayout {
    /// Creates a layout for `multiplications` live rows.
    ///
    /// The gate capacity is `max(256, multiplications).next_power_of_two()`.
    /// The minimum keeps the compact row packing in its supported geometry.
    /// The combined Spartan/F2Z production API additionally requires at least
    /// `2^15` slots so it can use a validator-gated Ligerito profile.
    pub fn new(multiplications: usize) -> Result<Self, U32MulError> {
        Self::new_with_f2z_width(multiplications, U32MulF2zWidth::W1)
    }

    /// Creates a layout with an explicit logical F2Z word width.
    ///
    /// The integer assignment and Spartan relation are independent of this
    /// choice. Only the compact committed-bit tensor changes: `W8` groups each
    /// eight consecutive global bit slots into one little-endian logical cell.
    pub fn new_with_f2z_width(
        multiplications: usize,
        f2z_width: U32MulF2zWidth,
    ) -> Result<Self, U32MulError> {
        if multiplications == 0 {
            return Err(U32MulError::EmptyBatch);
        }

        let capacity = multiplications
            .max(MIN_CAPACITY)
            .checked_next_power_of_two()
            .ok_or(U32MulError::DomainTooLarge)?;

        capacity
            .checked_mul(ASSIGNMENT_BLOCKS)
            .and_then(|_| capacity.checked_mul(U32_MUL_BIT_SLOTS))
            .ok_or(U32MulError::DomainTooLarge)?;

        let gate_vars = capacity.trailing_zeros() as usize;
        Ok(Self {
            multiplications,
            capacity,
            gate_vars,
            f2z_width,
        })
    }

    /// Number of live multiplication rows.
    pub const fn multiplications(&self) -> usize {
        self.multiplications
    }

    /// Power-of-two gate capacity, including zero-padded gates.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of variables selecting a gate in the padded gate domain.
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }

    /// Logical word width used by the compact F2Z commitment.
    pub const fn f2z_width(&self) -> U32MulF2zWidth {
        self.f2z_width
    }

    /// Logical integer assignment length: four blocks of `capacity` values.
    pub const fn assignment_len(&self) -> usize {
        ASSIGNMENT_BLOCKS * self.capacity
    }

    /// F2Z shape for the compact 32/32/64-bit witness.
    ///
    /// If `g = log2(capacity)`, the low `s = floor(g/2)` gate coordinates
    /// become F2Z columns and `h = g - s` high gate coordinates remain on the
    /// row axis. Packing `W` consecutive bit slots into one logical cell leaves
    /// `7 - log2(W)` word-slot coordinates, so
    /// `t = h + 7 - log2(W)`. In both supported layouts,
    /// [`crate::ligerito::packed_vars`] is exactly `g`.
    pub const fn f2z_params(&self) -> IntEvalParams {
        let s = self.gate_vars / 2;
        let h = self.gate_vars - s;
        let word_bits = self.f2z_width.word_bits();
        let log_word_bits = word_bits.trailing_zeros() as usize;
        IntEvalParams {
            t: h + 7 - log_word_bits,
            s,
            word_bits,
        }
    }

    /// Maps `(bit_slot, gate)` to the F2Z bit position `(b, c, j)`.
    ///
    /// `b` is the folded-row index, `c` is the clear-column index, and `j` is
    /// the little-endian bit within the `W`-bit logical cell. Gate coordinates
    /// use little-endian Boolean-index order on both axes.
    pub const fn f2z_bit_position(
        &self,
        bit_slot: usize,
        gate: usize,
    ) -> Option<(usize, usize, usize)> {
        if bit_slot >= U32_MUL_BIT_SLOTS || gate >= self.capacity {
            return None;
        }

        let s = self.gate_vars / 2;
        let h = self.gate_vars - s;
        let column_mask = (1usize << s) - 1;
        let gate_high = gate >> s;
        let word_bits = self.f2z_width.word_bits();
        let word_slot = bit_slot / word_bits;
        let b = (word_slot << h) | gate_high;
        let c = gate & column_mask;
        let j = bit_slot % word_bits;
        Some((b, c, j))
    }

    /// Maps `(bit_slot, gate)` to its containing F2Z row-major cell `(b, c)`.
    ///
    /// For `W8`, eight consecutive bit slots intentionally share one cell.
    /// Prefer [`Self::f2z_bit_position`] whenever the within-cell bit index
    /// matters.
    pub const fn f2z_cell(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        match self.f2z_bit_position(bit_slot, gate) {
            Some((b, c, _)) => Some((b, c)),
            None => None,
        }
    }
}

/// Exact native assignment for a batch of integer multiplications.
///
/// The assignment is block aligned as
///
/// `z = [constant block | x block | y block | product block]`.
///
/// Only `z[0]` is one in the constant block. Unused gates in all other blocks
/// are zero. Live products are computed in `u64`, without field reduction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U32MulWitness {
    layout: U32MulLayout,
    assignment: Box<[u64]>,
}

/// Native MLE tables retained before Spartan's first field-valued fold.
///
/// The assignment and the three row products preserve the exact integer
/// values of the u32 multiplication relation. They are padded to the same
/// Boolean domains as their field-valued counterparts, but no modular
/// projection has occurred yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U32MulNativeMles {
    assignment: DenseMultilinearExtension<u64>,
    products: R1csProductMles<u64>,
}

impl U32MulNativeMles {
    /// Complete native assignment MLE.
    pub const fn assignment(&self) -> &DenseMultilinearExtension<u64> {
        &self.assignment
    }

    /// Native `Az`, `Bz`, and `Cz` MLEs.
    pub const fn products(&self) -> &R1csProductMles<u64> {
        &self.products
    }

    /// Moves out the assignment and product MLEs.
    pub fn into_parts(self) -> (DenseMultilinearExtension<u64>, R1csProductMles<u64>) {
        (self.assignment, self.products)
    }
}

impl U32MulWitness {
    /// Constructs the exact assignment from explicit operand pairs.
    pub fn from_inputs(inputs: &[(u32, u32)]) -> Result<Self, U32MulError> {
        Self::from_inputs_with_f2z_width(inputs, U32MulF2zWidth::W1)
    }

    /// Constructs the exact assignment with an explicit logical F2Z word
    /// width.
    pub fn from_inputs_with_f2z_width(
        inputs: &[(u32, u32)],
        f2z_width: U32MulF2zWidth,
    ) -> Result<Self, U32MulError> {
        Self::from_fn_with_f2z_width(inputs.len(), f2z_width, |index| inputs[index])
    }

    /// Constructs the exact assignment without retaining a separate input
    /// vector, which is useful for large deterministic benchmark batches.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_fn(
        multiplications: usize,
        input: impl FnMut(usize) -> (u32, u32),
    ) -> Result<Self, U32MulError> {
        Self::from_fn_with_f2z_width(multiplications, U32MulF2zWidth::W1, input)
    }

    /// Constructs the exact assignment with an explicit logical F2Z word
    /// width, without retaining a separate input vector.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_fn_with_f2z_width(
        multiplications: usize,
        f2z_width: U32MulF2zWidth,
        mut input: impl FnMut(usize) -> (u32, u32),
    ) -> Result<Self, U32MulError> {
        let layout = U32MulLayout::new_with_f2z_width(multiplications, f2z_width)?;
        let capacity = layout.capacity;
        let mut assignment = vec![0_u64; layout.assignment_len()];
        assignment[0] = 1;

        for index in 0..multiplications {
            let (x, y) = input(index);
            let x = u64::from(x);
            let y = u64::from(y);
            assignment[capacity + index] = x;
            assignment[2 * capacity + index] = y;
            assignment[3 * capacity + index] = x * y;
        }

        Ok(Self {
            layout,
            assignment: assignment.into_boxed_slice(),
        })
    }

    /// Shape shared by this assignment and its bit representation.
    pub const fn layout(&self) -> &U32MulLayout {
        &self.layout
    }

    /// Complete block-aligned integer assignment.
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

    /// Padded exact-product block.
    pub fn product_values(&self) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[3 * capacity..4 * capacity]
    }

    /// Live `Az` values for the generated selector matrix.
    pub fn az(&self) -> &[u64] {
        &self.x_values()[..self.layout.multiplications]
    }

    /// Live `Bz` values for the generated selector matrix.
    pub fn bz(&self) -> &[u64] {
        &self.y_values()[..self.layout.multiplications]
    }

    /// Live `Cz` values for the generated selector matrix.
    pub fn cz(&self) -> &[u64] {
        &self.product_values()[..self.layout.multiplications]
    }

    /// Builds the compact F2Z rows without materializing a `u128` cell tensor.
    ///
    /// The result has `p.cols()` rows and `p.rows() * W / 64` words per row.
    /// Bit `b * W + j` of row `c` is bit `j` of logical cell `(b,c)`. Global
    /// slots `0..32`, `32..64`, and `64..128` contain little-endian bits of
    /// `x`, `y`, and `product`.
    ///
    /// Row `c` is `128 / W` lanes of `high_gate_count` `W`-bit cells (see
    /// [`Self::layout`]'s [`U32MulLayout::f2z_bit_position`]). Whenever a
    /// lane spans whole words — every layout at `W8`, and `W1` from `2^11`
    /// gate slots up — the rows are built by the block transposes of
    /// [`super::slot_rows`]; otherwise by the bitwise path. Both produce
    /// identical rows.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn f2z_bit_rows(&self) -> Vec<Vec<u64>> {
        let params = self.layout.f2z_params();
        let bits_per_row = params.rows() * params.word_bits;
        let words_per_row = bits_per_row / u64::BITS as usize;
        let mut rows = vec![vec![0_u64; words_per_row]; params.cols()];

        let s = self.layout.gate_vars / 2;
        let high_gate_count = 1_usize << (self.layout.gate_vars - s);
        let live = self.layout.multiplications;
        let x_values = self.x_values();
        let y_values = self.y_values();
        let product_values = self.product_values();
        let gate_slots =
            |gate: usize| pack_gate_slots(x_values[gate], y_values[gate], product_values[gate]);
        match self.layout.f2z_width {
            U32MulF2zWidth::W1 if high_gate_count.is_multiple_of(u64::BITS as usize) => {
                pack_slot_major_rows_w1(&mut rows, s, high_gate_count, live, gate_slots);
            }
            U32MulF2zWidth::W8 if high_gate_count.is_multiple_of(u8::BITS as usize) => {
                pack_slot_major_rows_w8(&mut rows, s, high_gate_count, live, gate_slots);
            }
            _ => self.write_bit_rows_bitwise(&mut rows),
        }
        rows
    }

    /// Reference packing: one masked read-modify-write per set bit (`W1`)
    /// or nonzero byte (`W8`).
    #[allow(clippy::arithmetic_side_effects)]
    fn write_bit_rows_bitwise(&self, rows: &mut [Vec<u64>]) {
        let s = self.layout.gate_vars / 2;
        let high_gate_count = 1_usize << (self.layout.gate_vars - s);
        let column_mask = (1_usize << s) - 1;
        let width = self.layout.f2z_width;
        let x_values = self.x_values();
        let y_values = self.y_values();
        let product_values = self.product_values();

        for gate in 0..self.layout.multiplications {
            let column = gate & column_mask;
            let gate_high = gate >> s;
            let row = &mut rows[column];
            write_compact_value(
                row,
                width,
                high_gate_count,
                gate_high,
                U32_MUL_X_SLOT_START,
                U32_MUL_X_BITS,
                x_values[gate],
            );
            write_compact_value(
                row,
                width,
                high_gate_count,
                gate_high,
                U32_MUL_Y_SLOT_START,
                U32_MUL_Y_BITS,
                y_values[gate],
            );
            write_compact_value(
                row,
                width,
                high_gate_count,
                gate_high,
                U32_MUL_PRODUCT_SLOT_START,
                U32_MUL_PRODUCT_BITS,
                product_values[gate],
            );
        }
    }

    /// Moves out the layout and exact assignment.
    pub fn into_parts(self) -> (U32MulLayout, Box<[u64]>) {
        (self.layout, self.assignment)
    }
}

/// Packs one gate's `x`, `y`, and product into its 128 committed slots
/// (`x | y << 32 | product << 64`; the operands masked to their 32 committed
/// bits) and returns the words for slots `0..64` and `64..128`.
#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
const fn pack_gate_slots(x: u64, y: u64, product: u64) -> (u64, u64) {
    const X_MASK: u128 = (1 << U32_MUL_X_BITS) - 1;
    const Y_MASK: u128 = (1 << U32_MUL_Y_BITS) - 1;
    let packed = ((x as u128) & X_MASK) << U32_MUL_X_SLOT_START
        | ((y as u128) & Y_MASK) << U32_MUL_Y_SLOT_START
        | (product as u128) << U32_MUL_PRODUCT_SLOT_START;
    (packed as u64, (packed >> 64) as u64)
}

#[allow(clippy::arithmetic_side_effects)]
fn write_compact_value(
    row: &mut [u64],
    width: U32MulF2zWidth,
    high_gate_count: usize,
    gate_high: usize,
    slot_offset: usize,
    bit_width: usize,
    value: u64,
) {
    match width {
        U32MulF2zWidth::W1 => {
            let mut remaining = if bit_width == u64::BITS as usize {
                value
            } else {
                value & ((1_u64 << bit_width) - 1)
            };
            while remaining != 0 {
                let bit = remaining.trailing_zeros() as usize;
                let packed_bit = (slot_offset + bit) * high_gate_count + gate_high;
                row[packed_bit / u64::BITS as usize] |= 1_u64 << (packed_bit % u64::BITS as usize);
                remaining &= remaining - 1;
            }
        }
        U32MulF2zWidth::W8 => {
            let word_slot_start = slot_offset / u8::BITS as usize;
            for byte in 0..bit_width / u8::BITS as usize {
                let byte_value = (value >> (byte * u8::BITS as usize)) & u8::MAX as u64;
                if byte_value == 0 {
                    continue;
                }
                let byte_index = (word_slot_start + byte) * high_gate_count + gate_high;
                row[byte_index / 8] |= byte_value << ((byte_index % 8) * u8::BITS as usize);
            }
        }
    }
}

/// Builds the three generic CSC selector matrices for this relation.
///
/// Each live row has one nonzero in each matrix:
///
/// `A[i,M+i] = 1`, `B[i,2M+i] = 1`, and `C[i,3M+i] = 1`.
pub fn u32_mul_constraint_matrices<C: Clone>(
    layout: &U32MulLayout,
    one: C,
) -> Result<ConstraintMatrices<C>, U32MulError> {
    let a = selector_matrix(layout, 1, &one)?;
    let b = selector_matrix(layout, 2, &one)?;
    let c = selector_matrix(layout, 3, &one)?;
    Ok(ConstraintMatrices::new(a, b, c)?)
}

#[allow(clippy::arithmetic_side_effects)]
fn selector_matrix<C: Clone>(
    layout: &U32MulLayout,
    block: usize,
    one: &C,
) -> Result<SparseMatrix<C>, SpartanMatrixError> {
    let columns = layout.assignment_len();
    let rows = layout.multiplications;
    let offset = block * layout.capacity;

    // Build the flat CSC representation directly. A nested vector would add
    // one allocation header per one of the 4M logical columns and duplicate
    // every entry while flattening, which is prohibitive at benchmark scale.
    let mut column_offsets = vec![0; columns + 1];
    for (row, boundary) in column_offsets[offset + 1..offset + rows + 1]
        .iter_mut()
        .enumerate()
    {
        *boundary = row + 1;
    }
    column_offsets[offset + rows + 1..].fill(rows);
    let row_indices = (0..rows).collect();
    let coefficients = (0..rows).map(|_| one.clone()).collect();

    Ok(SparseMatrix::try_from_csc_parts(
        rows,
        column_offsets,
        row_indices,
        coefficients,
    )?)
}

/// Generates and prepares the field-valued selector matrices.
pub fn prepare_u32_mul_relation<F>(
    layout: U32MulLayout,
    field_config: &F::Config,
) -> Result<PreparedConstraintMatrices<F, bool>, U32MulError>
where
    F: SpartanField,
{
    let matrices = u32_mul_constraint_matrices(&layout, true)?;
    Ok(PreparedConstraintMatrices::new(matrices, field_config)?)
}

/// Pads the exact integer assignment and products without projecting them to
/// the Spartan field.
///
/// This is the input boundary for a native first sumcheck round. A prover must
/// reduce each resulting round claim to `F` before transcript absorption and
/// must fold these tables into field-valued MLEs before a later multiplication.
pub fn project_u32_mul_native_witness(witness: &U32MulWitness) -> U32MulNativeMles {
    let assignment = DenseMultilinearExtension {
        evaluations: witness.assignment().to_vec(),
        num_vars: witness.assignment().len().ilog2() as usize,
    };

    let product_len = witness.layout.multiplications.next_power_of_two();
    let product_vars = product_len.ilog2() as usize;
    let padded_product = |values: &[u64]| {
        let mut evaluations = values.to_vec();
        evaluations.resize(product_len, 0);
        DenseMultilinearExtension {
            evaluations,
            num_vars: product_vars,
        }
    };
    let products = R1csProductMles {
        az: padded_product(witness.az()),
        bz: padded_product(witness.bz()),
        cz: padded_product(witness.cz()),
    };

    U32MulNativeMles {
        assignment,
        products,
    }
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint};

    use super::*;

    const TEST_MODULUS: u128 = (1_u128 << 100) - 15;

    fn config() -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(TEST_MODULUS)).expect("odd test modulus")
    }

    #[test]
    fn layout_pads_to_a_power_of_two_and_maps_bits_for_each_word_width() {
        assert_eq!(U32MulLayout::new(0), Err(U32MulError::EmptyBatch));

        for (multiplications, capacity) in [(1, 256), (3, 256), (256, 256), (257, 512)] {
            assert_eq!(
                U32MulLayout::new(multiplications).unwrap(),
                U32MulLayout::new_with_f2z_width(multiplications, U32MulF2zWidth::W1).unwrap()
            );

            for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
                let layout = U32MulLayout::new_with_f2z_width(multiplications, width).unwrap();
                assert_eq!(layout.multiplications(), multiplications);
                assert_eq!(layout.capacity(), capacity);
                assert_eq!(layout.assignment_len(), 4 * capacity);
                assert_eq!(layout.f2z_width(), width);

                let p = layout.f2z_params();
                let word_bits = width.word_bits();
                let s = layout.gate_vars() / 2;
                let h = layout.gate_vars() - s;
                assert_eq!(p.word_bits, word_bits);
                assert_eq!(p.t, h + 7 - word_bits.trailing_zeros() as usize);
                assert_eq!(p.cells() * word_bits, U32_MUL_BIT_SLOTS * capacity);
                assert_eq!(layout.f2z_bit_position(U32_MUL_BIT_SLOTS, 0), None);
                assert_eq!(layout.f2z_bit_position(0, capacity), None);
                for slot in 0..U32_MUL_BIT_SLOTS {
                    for gate in 0..capacity {
                        let (b, c, j) = layout.f2z_bit_position(slot, gate).unwrap();
                        assert_eq!(b, ((slot / word_bits) << h) | (gate >> s));
                        assert_eq!(c, gate & ((1usize << s) - 1));
                        assert_eq!(j, slot % word_bits);
                        assert_eq!(p.cell_index(b, c), (slot / word_bits) * capacity + gate);
                        assert_eq!(layout.f2z_cell(slot, gate), Some((b, c)));
                    }
                }
            }
        }
    }

    #[test]
    fn exact_witness_uses_block_layout_and_zero_padding() {
        let inputs = [(0, u32::MAX), (1, 7), (u32::MAX, u32::MAX)];
        let witness = U32MulWitness::from_inputs(&inputs).unwrap();
        let capacity = witness.layout().capacity();

        assert_eq!(capacity, 256);
        assert_eq!(witness.layout().f2z_width(), U32MulF2zWidth::W1);
        assert_eq!(witness.assignment()[0], 1);
        assert!(
            witness.assignment()[1..capacity]
                .iter()
                .all(|&value| value == 0)
        );
        assert_eq!(&witness.x_values()[..3], &[0, 1, u64::from(u32::MAX)]);
        assert_eq!(
            &witness.y_values()[..3],
            &[u64::from(u32::MAX), 7, u64::from(u32::MAX)]
        );
        assert_eq!(
            &witness.product_values()[..3],
            &[0, 7, u64::from(u32::MAX) * u64::from(u32::MAX)]
        );
        assert!(witness.x_values()[3..].iter().all(|&value| value == 0));
        assert!(witness.y_values()[3..].iter().all(|&value| value == 0));
        assert!(
            witness.product_values()[3..]
                .iter()
                .all(|&value| value == 0)
        );
        assert_eq!(witness.az(), &witness.x_values()[..3]);
        assert_eq!(witness.bz(), &witness.y_values()[..3]);
        assert_eq!(witness.cz(), &witness.product_values()[..3]);
    }

    #[test]
    fn from_fn_generates_each_input_once_without_an_input_buffer() {
        let mut calls = Vec::new();
        let witness = U32MulWitness::from_fn_with_f2z_width(5, U32MulF2zWidth::W8, |index| {
            calls.push(index);
            (index as u32, (index + 1) as u32)
        })
        .unwrap();

        assert_eq!(calls, (0..5).collect::<Vec<_>>());
        assert_eq!(witness.cz(), &[0, 2, 6, 12, 20]);
        assert_eq!(witness.layout().f2z_width(), U32MulF2zWidth::W8);
    }

    #[test]
    fn native_mles_preserve_values_and_pad_only_the_row_domain() {
        let inputs = [(2, 3), (u32::MAX, u32::MAX), (11, 13)];
        let witness = U32MulWitness::from_inputs(&inputs).unwrap();
        let native = project_u32_mul_native_witness(&witness);

        assert_eq!(
            native.assignment().num_vars,
            witness.layout().gate_vars() + 2
        );
        assert_eq!(native.assignment().evaluations, witness.assignment());
        assert_eq!(native.products().az.num_vars, 2);
        assert_eq!(native.products().bz.num_vars, 2);
        assert_eq!(native.products().cz.num_vars, 2);
        assert_eq!(&native.products().az.evaluations[..3], witness.az());
        assert_eq!(&native.products().bz.evaluations[..3], witness.bz());
        assert_eq!(&native.products().cz.evaluations[..3], witness.cz());
        assert_eq!(native.products().az.evaluations[3], 0);
        assert_eq!(native.products().bz.evaluations[3], 0);
        assert_eq!(native.products().cz.evaluations[3], 0);
    }

    #[test]
    fn generic_matrices_are_exact_csc_selectors() {
        let layout = U32MulLayout::new(3).unwrap();
        let matrices = u32_mul_constraint_matrices(&layout, 1_u64).unwrap();
        let capacity = layout.capacity();

        assert_eq!(matrices.row_count(), 3);
        assert_eq!(matrices.column_count(), 4 * capacity);
        for matrix in [matrices.a(), matrices.b(), matrices.c()] {
            assert_eq!(matrix.nnz(), 3);
        }

        for row in 0..layout.multiplications() {
            for column in [
                matrices.a().column(capacity + row).unwrap(),
                matrices.b().column(2 * capacity + row).unwrap(),
                matrices.c().column(3 * capacity + row).unwrap(),
            ] {
                assert_eq!(column.row_indices(), &[row]);
                assert_eq!(column.coefficients(), &[1]);
            }
        }
        assert!(matrices.a().column(0).unwrap().is_empty());
        assert!(matrices.a().column(2 * capacity).unwrap().is_empty());
        assert!(matrices.b().column(capacity).unwrap().is_empty());
        assert!(matrices.c().column(2 * capacity).unwrap().is_empty());
    }

    #[test]
    fn packed_rows_reconstruct_the_32_32_64_bit_witness_for_each_word_width() {
        let inputs = [(0x8000_0001, 3), (u32::MAX, u32::MAX), (17, 19)];
        for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
            let witness = U32MulWitness::from_inputs_with_f2z_width(&inputs, width).unwrap();
            let layout = witness.layout();
            let p = layout.f2z_params();
            let rows = witness.f2z_bit_rows();

            assert_eq!(rows.len(), p.cols());
            assert!(
                rows.iter()
                    .all(|row| row.len() == p.rows() * p.word_bits / 64)
            );

            for gate in 0..layout.capacity() {
                let values = [
                    (
                        U32_MUL_X_SLOT_START,
                        U32_MUL_X_BITS,
                        witness.x_values()[gate],
                    ),
                    (
                        U32_MUL_Y_SLOT_START,
                        U32_MUL_Y_BITS,
                        witness.y_values()[gate],
                    ),
                    (
                        U32_MUL_PRODUCT_SLOT_START,
                        U32_MUL_PRODUCT_BITS,
                        witness.product_values()[gate],
                    ),
                ];
                for (slot_offset, bit_width, value) in values {
                    for bit in 0..bit_width {
                        let (b, c, j) = layout.f2z_bit_position(slot_offset + bit, gate).unwrap();
                        let packed_bit = b * p.word_bits + j;
                        let committed_bit = (rows[c][packed_bit / 64] >> (packed_bit % 64)) & 1;
                        assert_eq!(committed_bit, (value >> bit) & 1);
                    }
                }
            }
        }
    }

    #[test]
    fn transposed_bit_rows_match_the_bitwise_packing_for_each_word_width() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};
        // gate_vars 10 (W1 bitwise, W8 transposed), 11 (one W1 word per
        // lane), 13, and 15/16 (production layouts); live counts off the
        // power of two exercise the zero padding.
        for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
            for (multiplications, seed) in [
                (700, 1),
                (1500, 2),
                (5000, 3),
                (1 << 15, 4),
                ((1 << 15) + 37, 5),
            ] {
                let mut rng = StdRng::seed_from_u64(seed);
                let witness = U32MulWitness::from_fn_with_f2z_width(multiplications, width, |_| {
                    (rng.random::<u32>(), rng.random::<u32>())
                })
                .unwrap();
                let p = witness.layout().f2z_params();
                let mut expected = vec![vec![0_u64; p.rows() * p.word_bits / 64]; p.cols()];
                witness.write_bit_rows_bitwise(&mut expected);
                assert_eq!(
                    witness.f2z_bit_rows(),
                    expected,
                    "width={width:?} multiplications={multiplications}"
                );
            }
        }
    }

    #[test]
    fn prepared_u32_relation_uses_boolean_selectors() {
        let config = config();
        let layout = U32MulLayout::new(3).unwrap();
        let relation = prepare_u32_mul_relation::<F128>(layout, &config).unwrap();
        let capacity = layout.capacity();

        for (column, row) in [
            (relation.matrices().a().column(capacity).unwrap(), 0),
            (relation.matrices().b().column(2 * capacity + 1).unwrap(), 1),
            (relation.matrices().c().column(3 * capacity + 2).unwrap(), 2),
        ] {
            assert_eq!(column.row_indices(), &[row]);
            assert_eq!(column.coefficients(), &[true]);
        }
    }
}
