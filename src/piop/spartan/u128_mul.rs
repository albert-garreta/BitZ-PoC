//! Integer-level R1CS relation for batched `u128 * u128 = u256` multiplication.
//!
//! Each live row proves `x · y = z` over the integers for `x, y < 2^128` and
//! `z < 2^256`. Unlike the u64 relation, the assignment entries are the
//! integers themselves — `[e0 | x | y | z]`, four blocks, so the assignment
//! MLE needs no padding block — and the three matrices are plain Boolean
//! selectors: the Step-2 projection reduces `x`, `y`, `z` modulo the sampled
//! prime and the bitification carries the weights `2^0 … 2^255` of the
//! committed bits (512 per gate: `x` in slots `0..128`, `y` in `128..256`,
//! `z` in `256..512`). No matrix coefficient ever exceeds one, so the
//! prime-independent skeleton applies unchanged.
//!
//! The witness stores every value as `u64` limbs (`x`, `y` as two limbs,
//! `z` as four) purely as host representation; the relation is over `ℤ`.

use crypto_primitives::FromWithConfig;
use thiserror::Error;

use crate::{
    pcs::IntEvalParams,
    poly::mle::DenseMultilinearExtension,
    utils::{cfg_iter, cfg_iter_mut},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    ConstraintMatrices, PreparedConstraintMatrices, R1csProductMles, SparseMatrix, SpartanField,
    SpartanMatrixError, SpartanRelationBackend, slot_rows::pack_slot_major_rows_w1_words,
};

/// Committed little-endian bits of each operand.
pub const U128_MUL_OPERAND_BITS: usize = 128;
/// Committed little-endian bits of each product.
pub const U128_MUL_PRODUCT_BITS: usize = 256;
/// First bit slot occupied by the left operand.
pub const U128_MUL_X_SLOT_START: usize = 0;
/// First bit slot occupied by the right operand.
pub const U128_MUL_Y_SLOT_START: usize = U128_MUL_X_SLOT_START + U128_MUL_OPERAND_BITS;
/// First bit slot occupied by the product.
pub const U128_MUL_Z_SLOT_START: usize = U128_MUL_Y_SLOT_START + U128_MUL_OPERAND_BITS;
/// Total number of bit slots committed for each multiplication.
pub const U128_MUL_BIT_SLOTS: usize = U128_MUL_Z_SLOT_START + U128_MUL_PRODUCT_BITS;
/// `log2` of [`U128_MUL_BIT_SLOTS`]: the physical slot coordinates on the
/// folded row axis.
pub const U128_MUL_SLOT_VARS: usize = 9;
/// Words of 64 committed bits per gate.
const WORDS_PER_GATE: usize = U128_MUL_BIT_SLOTS / 64;

/// Four assignment blocks: `[e0 | x | y | z]`.
pub(super) const U128_MUL_ASSIGNMENT_BLOCKS: usize = 4;
// Keep even small relation fixtures in the geometry accepted by the F2Z row
// packer. The combined production proof applies its stricter 2^15 minimum.
const MIN_CAPACITY: usize = 1 << 8;

/// Integer relation backend used by the u128 multiplication prover.
///
/// The matrices are Boolean selectors; the assignment holds 128- and
/// 256-bit integers, so neither the witness nor the products have a native
/// `u64` first round — the prover works on residues built from the limbs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct U128MulRelationBackend;

impl<F> SpartanRelationBackend<F> for U128MulRelationBackend
where
    F: SpartanField,
{
    type MatrixCoeff = bool;
    type Witness = u128;
    type Product = u128;
}

/// Failures while constructing the u128 multiplication relation.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum U128MulError {
    /// A multiplication batch must contain at least one live row.
    #[error("a u128 multiplication batch must not be empty")]
    EmptyBatch,

    /// The padded assignment or committed-bit domain does not fit in `usize`.
    #[error("the u128 multiplication domain is too large")]
    DomainTooLarge,

    /// The generated relation or projected witness is malformed.
    #[error(transparent)]
    SpartanMatrix(#[from] SpartanMatrixError),
}

/// Shared shape of the integer assignment and its compact F2Z bit witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U128MulLayout {
    multiplications: usize,
    capacity: usize,
    gate_vars: usize,
}

impl U128MulLayout {
    /// Creates a layout for `multiplications` live rows.
    ///
    /// The gate capacity is `max(256, multiplications).next_power_of_two()`.
    /// The combined Spartan/F2Z production API additionally requires at least
    /// `2^15` slots so it can use a validator-gated Ligerito profile.
    pub fn new(multiplications: usize) -> Result<Self, U128MulError> {
        if multiplications == 0 {
            return Err(U128MulError::EmptyBatch);
        }

        let capacity = multiplications
            .max(MIN_CAPACITY)
            .checked_next_power_of_two()
            .ok_or(U128MulError::DomainTooLarge)?;

        capacity
            .checked_mul(U128_MUL_ASSIGNMENT_BLOCKS)
            .and_then(|_| capacity.checked_mul(U128_MUL_BIT_SLOTS))
            .ok_or(U128MulError::DomainTooLarge)?;

        let gate_vars = capacity.trailing_zeros() as usize;
        Ok(Self {
            multiplications,
            capacity,
            gate_vars,
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

    /// Number of variables selecting one padded gate.
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }

    /// Assignment length: four blocks of `capacity` integers (already a
    /// power of two, so this is also the assignment-MLE length).
    pub const fn assignment_len(&self) -> usize {
        U128_MUL_ASSIGNMENT_BLOCKS * self.capacity
    }

    /// Number of variables in the assignment MLE.
    pub const fn assignment_vars(&self) -> usize {
        self.gate_vars + 2
    }

    /// F2Z shape for the slot-major 128/128/256-bit witness.
    ///
    /// If `g = log2(capacity)`, the low `s = floor(g/2)` gate coordinates
    /// become F2Z columns. The remaining gate coordinates and the nine
    /// physical slot coordinates become folded row variables, so the
    /// committed tensor has `g + 9` variables.
    pub const fn f2z_params(&self) -> IntEvalParams {
        let s = self.gate_vars / 2;
        IntEvalParams {
            t: U128_MUL_SLOT_VARS + self.gate_vars - s,
            s,
            word_bits: 1,
        }
    }

    /// Maps `(bit_slot, gate)` to the F2Z row-major cell `(b, c)`.
    ///
    /// `params.cell_index(b, c) == bit_slot * capacity + gate`.
    pub const fn f2z_cell(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        if bit_slot >= U128_MUL_BIT_SLOTS || gate >= self.capacity {
            return None;
        }

        let s = self.gate_vars / 2;
        let column_mask = (1usize << s) - 1;
        let gate_high = gate >> s;
        let b = (bit_slot << (self.gate_vars - s)) | gate_high;
        let c = gate & column_mask;
        Some((b, c))
    }
}

/// The exact 256-bit product of two 128-bit integers as `(low, high)`
/// 128-bit halves.
#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
pub const fn mul_u128_full(x: u128, y: u128) -> (u128, u128) {
    let (x0, x1) = (x as u64 as u128, x >> 64);
    let (y0, y1) = (y as u64 as u128, y >> 64);
    let p00 = x0 * y0;
    let p01 = x0 * y1;
    let p10 = x1 * y0;
    let p11 = x1 * y1;
    // Middle terms: p01 + p10 < 2^129, split around bit 64.
    let (middle, middle_carry) = p01.overflowing_add(p10);
    let middle_low = middle << 64;
    let middle_high = (middle >> 64) | ((middle_carry as u128) << 64);
    let (low, low_carry) = p00.overflowing_add(middle_low);
    // p11 + middle_high + carry < 2^128: the full product is below 2^256.
    let high = p11 + middle_high + low_carry as u128;
    (low, high)
}

/// Exact native assignment for a batch of u128 multiplications, stored as
/// per-block tables of host integers: `x`, `y` as `u128`, `z` as its
/// low and high 128-bit halves. Only the first constant entry is one; unused
/// gates in every other block are zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U128MulWitness {
    layout: U128MulLayout,
    x: Box<[u128]>,
    y: Box<[u128]>,
    z_lo: Box<[u128]>,
    z_hi: Box<[u128]>,
}

impl U128MulWitness {
    /// Constructs the exact assignment from explicit operand pairs.
    pub fn from_inputs(inputs: &[(u128, u128)]) -> Result<Self, U128MulError> {
        Self::from_fn(inputs.len(), |index| inputs[index])
    }

    /// Constructs the exact assignment without retaining a separate input
    /// vector, which is useful for large deterministic benchmark batches.
    pub fn from_fn(
        multiplications: usize,
        mut input: impl FnMut(usize) -> (u128, u128),
    ) -> Result<Self, U128MulError> {
        let layout = U128MulLayout::new(multiplications)?;
        let capacity = layout.capacity;
        let mut x = vec![0_u128; capacity];
        let mut y = vec![0_u128; capacity];
        let mut z_lo = vec![0_u128; capacity];
        let mut z_hi = vec![0_u128; capacity];
        for index in 0..multiplications {
            let (a, b) = input(index);
            let (lo, hi) = mul_u128_full(a, b);
            x[index] = a;
            y[index] = b;
            z_lo[index] = lo;
            z_hi[index] = hi;
        }
        Ok(Self {
            layout,
            x: x.into_boxed_slice(),
            y: y.into_boxed_slice(),
            z_lo: z_lo.into_boxed_slice(),
            z_hi: z_hi.into_boxed_slice(),
        })
    }

    /// Shape shared by this assignment and its bit representation.
    pub const fn layout(&self) -> &U128MulLayout {
        &self.layout
    }

    /// Padded left-operand block.
    pub fn x_values(&self) -> &[u128] {
        &self.x
    }

    /// Padded right-operand block.
    pub fn y_values(&self) -> &[u128] {
        &self.y
    }

    /// Padded low halves of the products.
    pub fn z_lo_values(&self) -> &[u128] {
        &self.z_lo
    }

    /// Padded high halves of the products.
    pub fn z_hi_values(&self) -> &[u128] {
        &self.z_hi
    }

    /// The exact product of live row `index` as `(low, high)` halves.
    pub fn product(&self, index: usize) -> (u128, u128) {
        (self.z_lo[index], self.z_hi[index])
    }

    /// The eight 64-bit committed words of one gate, in slot order.
    #[allow(clippy::cast_possible_truncation)]
    fn gate_words(&self, gate: usize) -> [u64; WORDS_PER_GATE] {
        let split = |value: u128| [value as u64, (value >> 64) as u64];
        let [x0, x1] = split(self.x[gate]);
        let [y0, y1] = split(self.y[gate]);
        let [z0, z1] = split(self.z_lo[gate]);
        let [z2, z3] = split(self.z_hi[gate]);
        [x0, x1, y0, y1, z0, z1, z2, z3]
    }

    /// Builds the compact F2Z rows without materializing a cell tensor.
    ///
    /// Row `c` is 512 lanes of `high_gate_count` bits: bit `gate_high` of
    /// lane `slot` is slot `slot` of gate `(gate_high << s) | c`. Whenever a
    /// lane spans whole words (every production layout) the rows are built
    /// by the block transposes of [`super::slot_rows`]; smaller layouts take
    /// the bitwise path. Both produce identical rows.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn f2z_bit_rows(&self) -> Vec<Vec<u64>> {
        let params = self.layout.f2z_params();
        let words_per_row = params.rows() / u64::BITS as usize;
        let mut rows = vec![vec![0_u64; words_per_row]; params.cols()];

        let s = self.layout.gate_vars / 2;
        let high_gate_count = 1_usize << (self.layout.gate_vars - s);
        if high_gate_count.is_multiple_of(u64::BITS as usize) {
            pack_slot_major_rows_w1_words::<WORDS_PER_GATE, _>(
                &mut rows,
                s,
                high_gate_count,
                self.layout.multiplications,
                |gate| self.gate_words(gate),
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
            for (word, value) in self.gate_words(gate).into_iter().enumerate() {
                for bit in 0..64 {
                    if value & (1_u64 << bit) == 0 {
                        continue;
                    }
                    let (b, c) = self
                        .layout
                        .f2z_cell(word * 64 + bit, gate)
                        .expect("witness bit coordinates are in bounds");
                    rows[c][b / u64::BITS as usize] |= 1_u64 << (b % u64::BITS as usize);
                }
            }
        }
    }
}

/// Builds the three Boolean selector matrices for this relation.
///
/// Each live row has one nonzero in each matrix:
///
/// `A[i,M+i] = 1`, `B[i,2M+i] = 1`, and `C[i,3M+i] = 1`.
pub fn u128_mul_constraint_matrices(
    layout: &U128MulLayout,
) -> Result<ConstraintMatrices<bool>, U128MulError> {
    let a = selector_matrix(layout, 1)?;
    let b = selector_matrix(layout, 2)?;
    let c = selector_matrix(layout, 3)?;
    Ok(ConstraintMatrices::new(a, b, c)?)
}

#[allow(clippy::arithmetic_side_effects)]
fn selector_matrix(layout: &U128MulLayout, block: usize) -> Result<SparseMatrix<bool>, SpartanMatrixError> {
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
    let row_indices = (0..rows).collect();
    let coefficients = vec![true; rows];

    Ok(SparseMatrix::try_from_csc_parts(
        rows,
        column_offsets,
        row_indices,
        coefficients,
    )?)
}

/// Generates and prepares the Boolean selector matrices over a Spartan field.
pub fn prepare_u128_mul_relation<F>(
    layout: U128MulLayout,
    field_config: &F::Config,
) -> Result<PreparedConstraintMatrices<F, bool>, U128MulError>
where
    F: SpartanField,
{
    let matrices = u128_mul_constraint_matrices(&layout)?;
    Ok(PreparedConstraintMatrices::new(matrices, field_config)?)
}

/// Projects the exact assignment and products into a Spartan field: `x`,
/// `y`, and the 256-bit `z` reduced modulo the field's prime (the Step-2
/// projection of the integer relation), written straight into the Spartan
/// layouts in parallel.
#[allow(clippy::arithmetic_side_effects)]
pub fn project_u128_mul_witness<F>(
    witness: &U128MulWitness,
    field_config: &F::Config,
) -> Result<(DenseMultilinearExtension<F>, R1csProductMles<F>), U128MulError>
where
    F: SpartanField + FromWithConfig<u64> + FromWithConfig<u128> + Send + Sync,
    F::Config: Sync,
{
    F::validate_config(field_config).map_err(SpartanMatrixError::from)?;
    const MIN_LEN: usize = 4096;

    let layout = witness.layout;
    let capacity = layout.capacity;
    let live = layout.multiplications;
    let zero = F::zero_with_cfg(field_config);
    // 2^128 as a field element: 2^127 fits a u128, then double.
    let two_pow_128 = {
        let mut value = F::from_with_cfg(1_u128 << 127, field_config);
        let two = F::from_with_cfg(2_u64, field_config);
        value *= &two;
        value
    };
    let reduce_z = |lo: u128, hi: u128| -> F {
        let mut value = F::from_with_cfg(hi, field_config);
        value *= &two_pow_128;
        value += &F::from_with_cfg(lo, field_config);
        value
    };

    let mut assignment = vec![zero.clone(); layout.assignment_len()];
    assignment[0] = F::one_with_cfg(field_config);
    for (block, values) in [(1, witness.x_values()), (2, witness.y_values())] {
        let target = &mut assignment[block * capacity..block * capacity + live];
        cfg_iter_mut!(target, MIN_LEN)
            .zip(cfg_iter!(values[..live], MIN_LEN))
            .for_each(|(slot, &value)| *slot = F::from_with_cfg(value, field_config));
    }
    {
        let target = &mut assignment[3 * capacity..3 * capacity + live];
        cfg_iter_mut!(target, MIN_LEN)
            .zip(cfg_iter!(witness.z_lo_values()[..live], MIN_LEN))
            .zip(cfg_iter!(witness.z_hi_values()[..live], MIN_LEN))
            .for_each(|((slot, &lo), &hi)| *slot = reduce_z(lo, hi));
    }

    let product_len = live.next_power_of_two();
    let product_vars = product_len.ilog2() as usize;
    let mut az = vec![zero.clone(); product_len];
    az[..live].clone_from_slice(&assignment[capacity..capacity + live]);
    let mut bz = vec![zero.clone(); product_len];
    bz[..live].clone_from_slice(&assignment[2 * capacity..2 * capacity + live]);
    let mut cz = vec![zero; product_len];
    cz[..live].clone_from_slice(&assignment[3 * capacity..3 * capacity + live]);

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
    use num_bigint::BigUint;

    use super::*;
    use crate::piop::spartan::{SpartanF2zField, spartan_f2z_field_config};

    fn inputs(n: usize) -> Vec<(u128, u128)> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        (0..n)
            .map(|index| {
                let wide = |a: u64, b: u64| (u128::from(a) << 64) | u128::from(b);
                match index % 5 {
                    0 => (u128::MAX, u128::MAX),
                    1 => (0, wide(next(), next())),
                    2 => (wide(next(), next()), 1),
                    3 => (u128::from(next()), u128::from(next())),
                    _ => (wide(next(), next()), wide(next(), next())),
                }
            })
            .collect()
    }

    #[test]
    fn full_product_matches_bigint() {
        for (x, y) in inputs(500) {
            let (lo, hi) = mul_u128_full(x, y);
            let expected = BigUint::from(x) * BigUint::from(y);
            let actual = (BigUint::from(hi) << 128) | BigUint::from(lo);
            assert_eq!(actual, expected, "{x} * {y}");
        }
    }

    #[test]
    fn layout_shapes() {
        let layout = U128MulLayout::new(1000).unwrap();
        assert_eq!(layout.capacity(), 1024);
        assert_eq!(layout.gate_vars(), 10);
        assert_eq!(layout.assignment_len(), 4 * 1024);
        assert_eq!(layout.assignment_vars(), 12);
        let p = layout.f2z_params();
        assert_eq!((p.t, p.s, p.word_bits), (9 + 5, 5, 1));
        assert_eq!(p.rows() * p.cols(), U128_MUL_BIT_SLOTS * layout.capacity());
        assert_eq!(U128_MUL_BIT_SLOTS, 1 << U128_MUL_SLOT_VARS);
        assert_eq!(U128MulLayout::new(0), Err(U128MulError::EmptyBatch));
    }

    #[test]
    fn bit_rows_match_cell_map_and_transposed_packer() {
        // 2^12 gates: s = 6, h = 6, high_gate_count = 64 → the transposed
        // packer runs; compare with the bitwise reference on the same witness.
        let inputs = inputs(3000);
        let witness = U128MulWitness::from_inputs(&inputs).unwrap();
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
        for (gate, &(x, y)) in inputs.iter().enumerate().take(200) {
            let (lo, hi) = mul_u128_full(x, y);
            for j in 0..128 {
                assert_eq!(bit(U128_MUL_X_SLOT_START + j, gate), ((x >> j) & 1) as u64);
                assert_eq!(bit(U128_MUL_Y_SLOT_START + j, gate), ((y >> j) & 1) as u64);
                assert_eq!(bit(U128_MUL_Z_SLOT_START + j, gate), ((lo >> j) & 1) as u64);
                assert_eq!(bit(U128_MUL_Z_SLOT_START + 128 + j, gate), ((hi >> j) & 1) as u64);
            }
        }
        assert_eq!(bit(U128_MUL_X_SLOT_START, 3500), 0);
    }

    #[test]
    fn matrices_select_the_expected_columns() {
        let layout = U128MulLayout::new(300).unwrap();
        let matrices = u128_mul_constraint_matrices(&layout).unwrap();
        let capacity = layout.capacity();
        assert_eq!(matrices.a().row_count(), 300);
        assert_eq!(matrices.a().column_count(), layout.assignment_len());
        for row in [0, 1, 299] {
            let single = |m: &SparseMatrix<bool>, column: usize| {
                let (row, coefficient) = m
                    .column(column)
                    .expect("column in range")
                    .single()
                    .expect("one entry");
                (row, *coefficient)
            };
            assert_eq!(single(matrices.a(), capacity + row), (row, true));
            assert_eq!(single(matrices.b(), 2 * capacity + row), (row, true));
            assert_eq!(single(matrices.c(), 3 * capacity + row), (row, true));
        }
        assert!(matrices.c().column(0).unwrap().single().is_none());
    }

    #[test]
    fn projection_satisfies_the_relation_in_the_field() {
        let inputs = inputs(300);
        let witness = U128MulWitness::from_inputs(&inputs).unwrap();
        let config = spartan_f2z_field_config();
        let (assignment, products) =
            project_u128_mul_witness::<SpartanF2zField>(&witness, &config).unwrap();
        assert_eq!(assignment.evaluations.len(), witness.layout().assignment_len());
        let capacity = witness.layout().capacity();
        for index in 0..300 {
            let mut lhs = products.az.evaluations[index].clone();
            lhs *= &products.bz.evaluations[index];
            assert_eq!(lhs, products.cz.evaluations[index]);
            assert_eq!(assignment.evaluations[3 * capacity + index], products.cz.evaluations[index]);
        }
        let prepared = prepare_u128_mul_relation::<SpartanF2zField>(*witness.layout(), &config).unwrap();
        assert_eq!(prepared.matrices().row_count(), 300);
    }
}
