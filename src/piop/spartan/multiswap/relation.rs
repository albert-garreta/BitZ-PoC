//! F2Z embedding of the MultiSwap integer Mod-R1CS.
//!
//! The per-row modulus term is folded into the output matrix: with the
//! quotients placed in their own assignment block, `mods[r] * quos[r]` is the
//! linear term `mods[r] * z[quos_column(r)]`, so the Mod-R1CS becomes the
//! plain R1CS `A·z ∘ B·z = C'·z` over
//!
//! ```text
//! z = [constant block | witness block | quotient block | zero block],
//! ```
//!
//! four blocks of one shared power-of-two gate capacity.  Only `z[0] = 1` is
//! nonzero in the constant block, and the fourth block is identically zero
//! (it carries no committed bits and no matrix entries, so the opening
//! forces it to zero).
//!
//! The F2Z commitment stores, for every gate, the
//! [`MULTISWAP_VALUE_BITS`]-bit little-endian decompositions of its witness
//! and quotient entries: `2^12` bit slots per gate (witness bits first).
//! With `s` low gate coordinates on the clear column axis and the remaining
//! `h` high gate coordinates joining the twelve slot coordinates on the
//! folded row axis, the committed tensor has parameters
//! `t = 12 + h`, `s`, `W = 1`.  The layout keeps `t <= 13` so a 113-bit
//! runtime prime needs exactly one mod-q weight chunk.

use crypto_primitives::FromWithConfig;
use num_bigint::BigUint;
use num_traits::Zero;
use thiserror::Error;

use crate::{pcs::IntEvalParams, poly::mle::DenseMultilinearExtension};

use super::super::{
    ConstraintMatrices, PreparedConstraintMatrices, R1csProductMles, SpartanField,
    SpartanMatrixError, build_assignment_mle, build_product_mles, matrix::SparseMatrix,
};
use super::circuit::{MULTISWAP_VALUE_BITS, MultiswapCircuit};

/// Number of committed bit slots per gate (`witness || quotient`).
pub const MULTISWAP_SLOTS: usize = 2 * MULTISWAP_VALUE_BITS;
/// `log2` of [`MULTISWAP_SLOTS`].
pub const MULTISWAP_SLOT_VARS: usize = 12;
/// First bit slot of the witness value.
pub const MULTISWAP_W_SLOT_START: usize = 0;
/// First bit slot of the quotient value.
pub const MULTISWAP_QUOS_SLOT_START: usize = MULTISWAP_VALUE_BITS;

const ASSIGNMENT_BLOCKS: usize = 4;
/// High gate coordinates kept on the folded row axis whenever the gate
/// domain allows it: `t = 12 + h` stays at 13, the one-chunk boundary for a
/// 113-bit runtime prime.
const MAX_HIGH_GATE_VARS: usize = 1;

/// Failures in layout construction or witness materialization.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum MultiswapLayoutError {
    /// The circuit's padded gate domain does not fit the layout.
    #[error("multiswap gate domain is empty or too large")]
    InvalidGateDomain,

    /// A generated matrix or assignment is malformed.
    #[error(transparent)]
    Matrix(#[from] SpartanMatrixError),
}

/// Shared shape of the block assignment and its compact F2Z bit tensor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiswapLayout {
    capacity: usize,
    gate_vars: usize,
    s: usize,
    h: usize,
}

impl MultiswapLayout {
    /// Derives the layout from the circuit's padded row/column counts.
    pub fn new(circuit: &MultiswapCircuit) -> Result<Self, MultiswapLayoutError> {
        let capacity = circuit.num_cons().max(circuit.num_vars());
        if !capacity.is_power_of_two() || capacity < 2 {
            return Err(MultiswapLayoutError::InvalidGateDomain);
        }
        let gate_vars = capacity.trailing_zeros() as usize;
        capacity
            .checked_mul(ASSIGNMENT_BLOCKS)
            .and_then(|len| len.checked_mul(MULTISWAP_SLOTS))
            .ok_or(MultiswapLayoutError::InvalidGateDomain)?;
        let h = gate_vars.min(MAX_HIGH_GATE_VARS);
        Ok(Self {
            capacity,
            gate_vars,
            s: gate_vars - h,
            h,
        })
    }

    /// Power-of-two gate capacity shared by all four assignment blocks.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of gate-selecting variables.
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }

    /// Number of clear column variables.
    pub const fn column_vars(&self) -> usize {
        self.s
    }

    /// Number of high gate variables on the folded row axis.
    pub const fn high_gate_vars(&self) -> usize {
        self.h
    }

    /// Complete assignment length: four blocks of `capacity` entries.
    pub const fn assignment_len(&self) -> usize {
        ASSIGNMENT_BLOCKS * self.capacity
    }

    /// First assignment index of the witness block.
    pub const fn witness_block_start(&self) -> usize {
        self.capacity
    }

    /// First assignment index of the quotient block.
    pub const fn quotient_block_start(&self) -> usize {
        2 * self.capacity
    }

    /// F2Z shape of the committed bit tensor.
    pub const fn f2z_params(&self) -> IntEvalParams {
        IntEvalParams {
            t: MULTISWAP_SLOT_VARS + self.h,
            s: self.s,
            word_bits: 1,
        }
    }

    /// Maps `(bit_slot, gate)` to the folded row and clear column indices.
    pub const fn f2z_bit_position(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        if bit_slot >= MULTISWAP_SLOTS || gate >= self.capacity {
            return None;
        }
        let column_mask = (1usize << self.s) - 1;
        let row = (bit_slot << self.h) | (gate >> self.s);
        Some((row, gate & column_mask))
    }
}

/// The q-independent integer relation in the block column space.
///
/// Matrices are column-major with the per-row moduli already folded into
/// `C` on the quotient block.  Coefficients are the exact circuit integers;
/// projection modulo the runtime prime happens per proof.
#[derive(Clone, Debug)]
pub struct MultiswapIntegerRelation {
    layout: MultiswapLayout,
    live_rows: usize,
    a: Vec<Vec<(usize, BigUint)>>,
    b: Vec<Vec<(usize, BigUint)>>,
    c: Vec<Vec<(usize, BigUint)>>,
}

impl MultiswapIntegerRelation {
    /// Remaps the circuit's COO matrices into the block column space and
    /// folds `mods` into `C`.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(circuit: &MultiswapCircuit) -> Result<Self, MultiswapLayoutError> {
        let layout = MultiswapLayout::new(circuit)?;
        let live_rows = circuit.live_rows();
        let columns = layout.assignment_len();
        let const_col = circuit.const_col();
        let witness_start = layout.witness_block_start();
        let quotient_start = layout.quotient_block_start();

        let remap = |column: usize| {
            if column == const_col {
                0
            } else {
                witness_start + column
            }
        };

        let collect = |entries: &[(usize, usize, BigUint)]| -> Vec<Vec<(usize, BigUint)>> {
            let mut columns_entries: Vec<Vec<(usize, BigUint)>> = vec![Vec::new(); columns];
            for (row, column, value) in entries {
                columns_entries[remap(*column)].push((*row, value.clone()));
            }
            columns_entries
        };

        let a = collect(circuit.a_entries());
        let b = collect(circuit.b_entries());
        let mut c = collect(circuit.c_entries());
        // `mods[r] * quos[r]` becomes the linear entry `C'[r, quos_col(r)]`.
        for (row, modulus) in circuit.mods().iter().enumerate().take(live_rows) {
            if !modulus.is_zero() {
                c[quotient_start + row].push((row, modulus.clone()));
            }
        }

        let mut relation = Self {
            layout,
            live_rows,
            a,
            b,
            c,
        };
        relation.normalize();
        Ok(relation)
    }

    /// Sorts every column by row and merges duplicate coordinates
    /// additively, matching the additive COO semantics of the source.
    #[allow(clippy::arithmetic_side_effects)]
    fn normalize(&mut self) {
        for matrix in [&mut self.a, &mut self.b, &mut self.c] {
            for column in matrix.iter_mut() {
                column.sort_by_key(|(row, _)| *row);
                let mut merged: Vec<(usize, BigUint)> = Vec::with_capacity(column.len());
                for (row, value) in column.drain(..) {
                    match merged.last_mut() {
                        Some((last_row, last_value)) if *last_row == row => *last_value += value,
                        _ => merged.push((row, value)),
                    }
                }
                *column = merged;
            }
        }
    }

    /// Layout shared with the committed bit tensor.
    pub const fn layout(&self) -> &MultiswapLayout {
        &self.layout
    }

    /// Number of live constraint rows.
    pub const fn live_rows(&self) -> usize {
        self.live_rows
    }

    /// Total stored coefficients across the three folded matrices.
    pub fn nnz(&self) -> usize {
        [&self.a, &self.b, &self.c]
            .iter()
            .map(|matrix| matrix.iter().map(Vec::len).sum::<usize>())
            .sum()
    }

    /// Projects the integer matrices modulo the runtime field.
    ///
    /// Coefficients that reduce to zero are dropped: the projected sparse
    /// matrix is exactly the matrix of the mod-q relation, and prover and
    /// verifier drop identically.
    pub fn project<F>(
        &self,
        field_config: &F::Config,
    ) -> Result<PreparedConstraintMatrices<F, F>, MultiswapLayoutError>
    where
        F: SpartanField + FromWithConfig<u128>,
    {
        let modulus = field_modulus(F::canonical_modulus_encoding(field_config));
        let project_matrix = |matrix: &Vec<Vec<(usize, BigUint)>>| {
            let columns = matrix
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .filter_map(|(row, value)| {
                            let residue = reduce_biguint(value, &modulus);
                            if residue == 0 {
                                None
                            } else {
                                Some((*row, F::from_with_cfg(residue, field_config)))
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            SparseMatrix::try_from_columns(self.live_rows, columns)
        };
        let a = project_matrix(&self.a).map_err(SpartanMatrixError::from)?;
        let b = project_matrix(&self.b).map_err(SpartanMatrixError::from)?;
        let c = project_matrix(&self.c).map_err(SpartanMatrixError::from)?;
        let matrices = ConstraintMatrices::new(a, b, c)?;
        Ok(PreparedConstraintMatrices::new(matrices, field_config)?)
    }
}

/// The q-independent committed witness: block assignment values and the
/// packed F2Z bit rows.
#[derive(Clone, Debug)]
pub struct MultiswapAssignment {
    layout: MultiswapLayout,
    values: Vec<BigUint>,
}

impl MultiswapAssignment {
    /// Materializes the block assignment from the circuit witness.
    pub fn new(circuit: &MultiswapCircuit) -> Result<Self, MultiswapLayoutError> {
        let layout = MultiswapLayout::new(circuit)?;
        let mut values = vec![BigUint::zero(); layout.assignment_len()];
        values[0] = BigUint::from(1u32);
        let witness_start = layout.witness_block_start();
        for (index, value) in circuit.witness().iter().enumerate() {
            values[witness_start + index] = value.clone();
        }
        let quotient_start = layout.quotient_block_start();
        for (index, value) in circuit.quotients().iter().enumerate() {
            values[quotient_start + index] = value.clone();
        }
        Ok(Self { layout, values })
    }

    /// Layout shared with the integer relation.
    pub const fn layout(&self) -> &MultiswapLayout {
        &self.layout
    }

    /// Complete block-aligned integer assignment.
    pub fn values(&self) -> &[BigUint] {
        &self.values
    }

    /// Builds the packed per-column F2Z bit rows.
    ///
    /// Column `c` packs, little-endian within each `u64` word, the
    /// `2^t` folded-row bits of every gate with low coordinates `c`: bit
    /// slot `j` of gate `g` lands at folded row `(j << h) | (g >> s)`.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn f2z_bit_rows(&self) -> Vec<Vec<u64>> {
        let p = self.layout.f2z_params();
        let words_per_column = p.rows() / u64::BITS as usize;
        let mut rows = vec![vec![0u64; words_per_column]; p.cols()];
        let column_mask = (1usize << self.layout.s) - 1;
        let h = self.layout.h;

        let mut write_value = |gate: usize, slot_start: usize, value: &BigUint| {
            if value.is_zero() {
                return;
            }
            let column = &mut rows[gate & column_mask];
            let gate_high = gate >> self.layout.s;
            for (limb_index, limb) in value.iter_u64_digits().enumerate() {
                let mut remaining = limb;
                while remaining != 0 {
                    let bit = remaining.trailing_zeros() as usize;
                    let slot = slot_start + limb_index * u64::BITS as usize + bit;
                    let row = (slot << h) | gate_high;
                    column[row / u64::BITS as usize] |= 1u64 << (row % u64::BITS as usize);
                    remaining &= remaining - 1;
                }
            }
        };

        let witness_start = self.layout.witness_block_start();
        let quotient_start = self.layout.quotient_block_start();
        for gate in 0..self.layout.capacity {
            write_value(
                gate,
                MULTISWAP_W_SLOT_START,
                &self.values[witness_start + gate],
            );
            write_value(
                gate,
                MULTISWAP_QUOS_SLOT_START,
                &self.values[quotient_start + gate],
            );
        }
        rows
    }

    /// Projects the assignment and the three matrix products modulo the
    /// runtime field.
    pub fn project<F>(
        &self,
        relation: &MultiswapIntegerRelation,
        field_config: &F::Config,
    ) -> Result<(DenseMultilinearExtension<F>, R1csProductMles<F>), MultiswapLayoutError>
    where
        F: SpartanField + FromWithConfig<u128>,
    {
        let modulus = field_modulus(F::canonical_modulus_encoding(field_config));
        let zero = F::zero_with_cfg(field_config);
        let field_assignment: Vec<F> = self
            .values
            .iter()
            .map(|value| F::from_with_cfg(reduce_biguint(value, &modulus), field_config))
            .collect();

        let product = |matrix: &Vec<Vec<(usize, BigUint)>>| -> Vec<F> {
            let mut out = vec![zero.clone(); relation.live_rows()];
            for (column, entries) in matrix.iter().enumerate() {
                if entries.is_empty() || F::is_zero(&field_assignment[column]) {
                    continue;
                }
                let z = &field_assignment[column];
                for (row, value) in entries {
                    let residue = reduce_biguint(value, &modulus);
                    if residue == 0 {
                        continue;
                    }
                    let mut term = F::from_with_cfg(residue, field_config);
                    term *= z;
                    out[*row] += &term;
                }
            }
            out
        };

        let az = product(&relation.a);
        let bz = product(&relation.b);
        let cz = product(&relation.c);
        let products = build_product_mles(&az, &bz, &cz, relation.live_rows(), field_config)?;
        let assignment = build_assignment_mle(
            &field_assignment,
            self.layout.assignment_len(),
            field_config,
        )?;
        Ok((assignment, products))
    }
}

/// Canonical `u128` runtime modulus recovered from its field encoding.
fn field_modulus(encoding: Vec<u8>) -> BigUint {
    BigUint::from_bytes_le(&encoding)
}

/// Reduces a nonnegative integer modulo the runtime prime into `u128`.
#[allow(clippy::arithmetic_side_effects)]
fn reduce_biguint(value: &BigUint, modulus: &BigUint) -> u128 {
    let reduced = value % modulus;
    let digits = reduced.iter_u64_digits().collect::<Vec<_>>();
    match digits.len() {
        0 => 0,
        1 => u128::from(digits[0]),
        2 => u128::from(digits[0]) | (u128::from(digits[1]) << 64),
        _ => unreachable!("a residue modulo a 128-bit prime has at most two u64 digits"),
    }
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint};

    use super::super::circuit::MultiswapDims;
    use super::*;

    fn mini() -> (
        MultiswapCircuit,
        MultiswapIntegerRelation,
        MultiswapAssignment,
    ) {
        let circuit = MultiswapCircuit::build(MultiswapDims::mini()).unwrap();
        let relation = MultiswapIntegerRelation::new(&circuit).unwrap();
        let assignment = MultiswapAssignment::new(&circuit).unwrap();
        (circuit, relation, assignment)
    }

    fn test_config() -> <F128 as PrimeField>::Config {
        // The fixed F2Z evaluation prime is a convenient valid runtime field.
        F128::make_cfg(&Uint::from(crate::pcs::FQ_MOD)).unwrap()
    }

    #[test]
    fn every_batch_embedding_preserves_the_canonical_linear_forms() {
        use std::collections::BTreeMap;
        for batch in [1, 2, 4, 8, 16] {
            let circuit =
                MultiswapCircuit::build_batch(MultiswapDims::multiswap(0), batch).unwrap();
            let relation = MultiswapIntegerRelation::new(&circuit).unwrap();
            let layout = relation.layout();
            for (is_c, source, embedded) in [
                (false, circuit.a_entries(), &relation.a),
                (false, circuit.b_entries(), &relation.b),
                (true, circuit.c_entries(), &relation.c),
            ] {
                let mut expected = BTreeMap::<(usize, usize), BigUint>::new();
                for (row, col, value) in source {
                    *expected.entry((*row, *col)).or_default() += value;
                }
                expected.retain(|_, v| !v.is_zero());
                let mut recovered = BTreeMap::<(usize, usize), BigUint>::new();
                let mut quotient_rows = Vec::new();
                for (column, entries) in embedded.iter().enumerate() {
                    for (row, value) in entries {
                        if column >= layout.quotient_block_start() {
                            assert!(is_c);
                            assert_eq!(column - layout.quotient_block_start(), *row);
                            assert!(*row < circuit.live_rows());
                            assert_eq!(value, &circuit.mods()[*row]);
                            quotient_rows.push(*row);
                        } else {
                            let source_column = if column == 0 {
                                circuit.const_col()
                            } else {
                                assert!(column >= layout.witness_block_start());
                                column - layout.witness_block_start()
                            };
                            *recovered.entry((*row, source_column)).or_default() += value;
                        }
                    }
                }
                recovered.retain(|_, v| !v.is_zero());
                assert_eq!(recovered, expected);
                if is_c {
                    quotient_rows.sort_unstable();
                    assert_eq!(
                        quotient_rows,
                        (0..circuit.live_rows())
                            .filter(|r| !circuit.mods()[*r].is_zero())
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }

    #[test]
    fn layout_keeps_one_chunk_geometry() {
        let (_, relation, _) = mini();
        let layout = *relation.layout();
        let p = layout.f2z_params();
        assert_eq!(p.word_bits, 1);
        assert!(p.t <= 13);
        assert_eq!(p.t + p.s, MULTISWAP_SLOT_VARS + layout.gate_vars());
        assert_eq!(
            p.cells(),
            MULTISWAP_SLOTS * layout.capacity(),
            "one bit cell per committed slot"
        );
    }

    #[test]
    fn bit_rows_reconstruct_the_assignment_values() {
        let (_, _, assignment) = mini();
        let layout = *assignment.layout();
        let p = layout.f2z_params();
        let rows = assignment.f2z_bit_rows();
        assert_eq!(rows.len(), p.cols());
        assert!(rows.iter().all(|row| row.len() == p.rows() / 64));

        for gate in 0..layout.capacity() {
            for (slot_start, block_start) in [
                (MULTISWAP_W_SLOT_START, layout.witness_block_start()),
                (MULTISWAP_QUOS_SLOT_START, layout.quotient_block_start()),
            ] {
                let mut reconstructed = BigUint::zero();
                for slot in 0..MULTISWAP_VALUE_BITS {
                    let (row, column) = layout.f2z_bit_position(slot_start + slot, gate).unwrap();
                    let bit = (rows[column][row / 64] >> (row % 64)) & 1;
                    if bit == 1 {
                        reconstructed.set_bit(slot as u64, true);
                    }
                }
                assert_eq!(&reconstructed, &assignment.values()[block_start + gate]);
            }
        }
    }

    #[test]
    fn projected_relation_is_satisfied_row_by_row() {
        let (_, relation, assignment) = mini();
        let config = test_config();
        let matrices = relation.project::<F128>(&config).unwrap();
        let (mle, products) = assignment.project::<F128>(&relation, &config).unwrap();

        assert_eq!(mle.evaluations.len(), relation.layout().assignment_len());
        assert_eq!(matrices.matrices().row_count(), relation.live_rows());
        assert_eq!(
            matrices.matrices().column_count(),
            relation.layout().assignment_len()
        );
        for row in 0..relation.live_rows() {
            let mut left = products.az.evaluations[row].clone();
            left *= &products.bz.evaluations[row];
            assert_eq!(left, products.cz.evaluations[row], "row {row}");
        }
    }

    #[test]
    fn tampering_one_quotient_breaks_the_projected_relation() {
        let (mut circuit, _, _) = {
            let circuit = MultiswapCircuit::build(MultiswapDims::mini()).unwrap();
            (circuit.clone(), (), ())
        };
        // Perturb one live quotient: the folded C' row must move.
        let target = 0;
        let mut quos = circuit.quotients().to_vec();
        quos[target] += BigUint::from(1u32);
        circuit = tampered_with_quotients(circuit, quos);

        let relation = MultiswapIntegerRelation::new(&circuit).unwrap();
        let assignment = MultiswapAssignment::new(&circuit).unwrap();
        let config = test_config();
        let (_, products) = assignment.project::<F128>(&relation, &config).unwrap();
        let mut left = products.az.evaluations[target].clone();
        left *= &products.bz.evaluations[target];
        assert_ne!(left, products.cz.evaluations[target]);
    }

    fn tampered_with_quotients(circuit: MultiswapCircuit, quos: Vec<BigUint>) -> MultiswapCircuit {
        circuit.with_quotients_for_tests(quos)
    }
}
