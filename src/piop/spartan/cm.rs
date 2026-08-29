//! Batched word-AND gates as a CM instance with an F₂-VIRTUAL block —
//! the paper's `\Relation_{CM}` / `r:CM_is_NP_complete` gadget
//! (`s:to_f2_virtual`) wired end to end through Spartan and the virtual
//! F2Z opening.
//!
//! Per gate: 32-bit words `x`, `y`, `z`, `w` with the single LINEAR
//! constraint
//!
//! ```text
//! x + y − w − 2·z = 0        over 𝔽_q (exact over ℤ: all values < 2^33)
//! ```
//!
//! Since `x + y = (x⊕y) + 2·(x∧y)` holds bitwise-exactly, the constraint
//! FORCES `z = x∧y` as soon as `w = x⊕y` bit-for-bit — and that identity
//! is not proven but imposed STRUCTURALLY: the committed vector `f`
//! carries only the `x`/`y`/`z` bits, while the R1CS's bit grid
//! `h = M·f` derives every `w` bit as the XOR of the matching `x` and
//! `y` bits ([`cm_and_map`]). The R1CS is `A = B = 0` with one `C` row
//! per gate — the pure CM shape (linear constraints over ℤ composed
//! with `F₂`-linear derivation), which is NP-complete.
//!
//! Layouts. The derived grid `h` has 128 bit slots per gate
//! (`x@0, y@32, z@64, w@96`); the committed grid `f` has the same shape
//! with slots `[96, 128)` structurally dead (never referenced by `M` or
//! the relation; zero for the honest prover, harmless if not). The
//! Spartan assignment is `[const | x | y | z | w]` — five logical blocks
//! padded to eight, so the terminal claim carries `gate_vars + 3`
//! coordinates. Bitification is the adjoint of the four 32-bit
//! reconstructions, exactly as in the direct `u32_mul` bridge but over
//! a 3-variable block selector.

use blake3::Hasher;
use crypto_primitives::{FromWithConfig, PrimeField};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};
use thiserror::Error;

use crate::{
    f2map::{cell_count, PreparedVirtualMap, PreparedVirtualMapError},
    ligerito::{packed_vars, LOG_PACKING},
    ligerito_flock::{
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual, sha_lig_configs,
        verify_mle_eval_mod_q_ligerito_virtual, FlockCommitHint, FlockRsError,
    },
    pcs::{
        eq_le_table_fq, fq_mul, fq_sub, Fq, IntEvalParams, ProjectCanonicalU128, FQ_BITS, FQ_MOD,
    },
    transcript::traits::Transcript,
};

use super::{
    absorb_spartan_message,
    f2z::{
        f2z_generator, fill_slot_weights, spartan_f2z_field_config, validate_bit_rows,
        validate_commitment, validate_config_pair, SpartanF2zError, SpartanF2zField,
        MIN_PRODUCTION_GATE_VARS,
    },
    matrix::{
        build_assignment_mle, build_product_mles, ConstraintMatrices, PreparedConstraintMatrices,
        ScaledMleEvaluationClaim, SparseMatrix, SpartanMatrixError,
    },
    piop::{prove_spartan_piop, verify_spartan_proof, SpartanError, SpartanPiopProof},
    EvaluatedSpartanAssignment, SpartanF2zProof, SpartanField, Virtualized,
};

/// Word width of every gate operand.
pub const CM_AND_WORD_BITS: usize = 32;
/// First derived bit slot of the left operand.
pub const CM_AND_X_SLOT: usize = 0;
/// First derived bit slot of the right operand.
pub const CM_AND_Y_SLOT: usize = 32;
/// First derived bit slot of the AND output.
pub const CM_AND_Z_SLOT: usize = 64;
/// First derived bit slot of the VIRTUAL XOR word `w = x ⊕ y`.
pub const CM_AND_W_SLOT: usize = 96;
/// Bit slots per gate in the derived grid `h` (a power of two).
pub const CM_AND_H_SLOTS: usize = 128;
/// Live committed bit slots per gate in `f` (`x`, `y`, `z`); the grid is
/// padded to [`CM_AND_H_SLOTS`] with structurally dead cells.
pub const CM_AND_F_LIVE_SLOTS: usize = 96;

/// Logical assignment blocks: `[const | x | y | z | w]`.
const ASSIGNMENT_BLOCKS: usize = 5;
/// Selector coordinates of the padded (8-block) assignment domain.
const SELECTOR_VARS: usize = 3;
/// Same floor as the direct bridge: keeps the compact row packing in its
/// supported geometry even for small test fixtures.
const MIN_CAPACITY: usize = 1 << 8;

const CM_ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/cm-f2z/assignment/v1";
const CM_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/cm-f2z/opening/v1";

/// Failures while constructing the CM-AND relation or its witness.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CmAndError {
    /// A gate batch must contain at least one live gate.
    #[error("a CM-AND batch must not be empty")]
    EmptyBatch,

    /// The padded assignment or bit domain does not fit in `usize`.
    #[error("the CM-AND domain is too large")]
    DomainTooLarge,

    /// The generated relation or projected witness is malformed.
    #[error(transparent)]
    SpartanMatrix(#[from] SpartanMatrixError),

    /// The structural derivation map could not be prepared.
    #[error(transparent)]
    VirtualMap(#[from] PreparedVirtualMapError),
}

/// Failures in the combined CM-AND Spartan + virtual-F2Z pipeline.
#[derive(Debug, Error)]
pub enum CmF2zError {
    #[error(transparent)]
    Relation(#[from] CmAndError),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    /// A reused direct-bridge validator rejected (bit rows, configs,
    /// commitment geometry, or slot-weight fill).
    #[error(transparent)]
    Bridge(#[from] SpartanF2zError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the virtual F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the prepared relation does not use q = 2^100 - 15")]
    UnsupportedFieldModulus,

    #[error("the relation and projected witness use different CM-AND layouts")]
    RelationWitnessLayoutMismatch,

    #[error("the F2Z parameters are invalid for the CM-AND layout")]
    InvalidF2zParameters,

    #[error("the combined CM-AND proof requires at least 2^15 gate slots")]
    UnauditedF2zParameters,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element uses a field other than q = 2^100 - 15")]
    ClaimFieldMismatch,

    #[error("a constant-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOnlyClaim,

    #[error("a host length does not fit the canonical transcript encoding")]
    BindingEncodingOverflow,
}

/// Shared shape of the CM-AND assignment, the derived grid `h`, and the
/// committed grid `f`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CmAndLayout {
    gates: usize,
    capacity: usize,
    gate_vars: usize,
}

impl CmAndLayout {
    /// Creates a layout for `gates` live AND gates. The gate capacity is
    /// `max(256, gates).next_power_of_two()`.
    pub fn new(gates: usize) -> Result<Self, CmAndError> {
        if gates == 0 {
            return Err(CmAndError::EmptyBatch);
        }
        let capacity = gates
            .max(MIN_CAPACITY)
            .checked_next_power_of_two()
            .ok_or(CmAndError::DomainTooLarge)?;
        capacity
            .checked_mul(CM_AND_H_SLOTS)
            .and_then(|cells| cells.checked_mul(2))
            .ok_or(CmAndError::DomainTooLarge)?;
        let gate_vars = capacity.trailing_zeros() as usize;
        Ok(Self {
            gates,
            capacity,
            gate_vars,
        })
    }

    /// Number of live gates.
    pub const fn gates(&self) -> usize {
        self.gates
    }

    /// Power-of-two gate capacity, including zero-padded gates.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of variables selecting a gate.
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }

    /// Logical integer assignment length: five blocks of `capacity`.
    pub const fn assignment_len(&self) -> usize {
        ASSIGNMENT_BLOCKS * self.capacity
    }

    /// The shared F2Z shape of BOTH grids (`W = 1`,
    /// `s = floor(gate_vars/2)`, `t = 7 + gate_vars − s`): `h` and `f`
    /// use the same geometry; they differ only in which slots are live.
    pub const fn f2z_params(&self) -> IntEvalParams {
        let s = self.gate_vars / 2;
        IntEvalParams {
            t: 7 + self.gate_vars - s,
            s,
            word_bits: 1,
        }
    }

    /// Maps `(bit_slot, gate)` to the row-major cell `(b, c)` — identical
    /// convention for `h` and `f` (same shape).
    pub const fn cell(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        if bit_slot >= CM_AND_H_SLOTS || gate >= self.capacity {
            return None;
        }
        let s = self.gate_vars / 2;
        let column_mask = (1usize << s) - 1;
        let b = (bit_slot << (self.gate_vars - s)) | (gate >> s);
        let c = gate & column_mask;
        Some((b, c))
    }

    /// Flat cell index (`(c << t) | b`, the pack/point order shared with
    /// [`PreparedVirtualMap`]) of `(bit_slot, gate)`.
    const fn flat_cell(&self, bit_slot: usize, gate: usize) -> usize {
        let s = self.gate_vars / 2;
        let t = 7 + self.gate_vars - s;
        let b = (bit_slot << (self.gate_vars - s)) | (gate >> s);
        let c = gate & ((1usize << s) - 1);
        (c << t) | b
    }
}

/// Builds the structural derivation map directly in canonical CSC form.
/// Source columns for x/y bits feed both their identity row and matching w
/// row; z bits feed their identity row; padded source slots are empty.
#[allow(clippy::arithmetic_side_effects)]
pub fn cm_and_map(layout: &CmAndLayout) -> Result<PreparedVirtualMap, CmAndError> {
    let p = layout.f2z_params();
    let cells = cell_count(&p);
    let nnz = layout
        .capacity
        .checked_mul(CM_AND_F_LIVE_SLOTS + 2 * CM_AND_WORD_BITS)
        .ok_or(CmAndError::DomainTooLarge)?;

    let s = layout.gate_vars / 2;
    let t = 7 + layout.gate_vars - s;
    let high_bits = layout.gate_vars - s;
    let row_mask = (1usize << t) - 1;
    let gate_high_mask = (1usize << high_bits) - 1;

    let mut column_offsets = Vec::with_capacity(cells + 1);
    let mut row_indices = Vec::with_capacity(nnz);
    column_offsets.push(0);
    for source in 0..cells {
        let b = source & row_mask;
        let c = source >> t;
        let slot = b >> high_bits;
        let gate = ((b & gate_high_mask) << s) | c;
        match slot {
            CM_AND_X_SLOT..CM_AND_Y_SLOT => {
                let bit = slot - CM_AND_X_SLOT;
                row_indices.push(layout.flat_cell(slot, gate));
                row_indices.push(layout.flat_cell(CM_AND_W_SLOT + bit, gate));
            }
            CM_AND_Y_SLOT..CM_AND_Z_SLOT => {
                let bit = slot - CM_AND_Y_SLOT;
                row_indices.push(layout.flat_cell(slot, gate));
                row_indices.push(layout.flat_cell(CM_AND_W_SLOT + bit, gate));
            }
            CM_AND_Z_SLOT..CM_AND_W_SLOT => {
                row_indices.push(layout.flat_cell(slot, gate));
            }
            _ => {}
        }
        column_offsets.push(row_indices.len());
    }
    debug_assert_eq!(row_indices.len(), nnz);

    let matrix = SparseMatrix::try_from_binary_csc(cells, column_offsets, row_indices)
        .map_err(SpartanMatrixError::from)?;
    Ok(PreparedVirtualMap::new(matrix)?)
}
/// Exact integer assignment for a batch of AND gates:
/// `z = [const | x | y | z | w]`, only `z[0]` nonzero in the constant
/// block, unused gates zero everywhere.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CmAndWitness {
    layout: CmAndLayout,
    assignment: Box<[u64]>,
}

impl CmAndWitness {
    /// Honest witness from operand pairs: `z = x ∧ y`, `w = x ⊕ y`.
    pub fn from_inputs(inputs: &[(u32, u32)]) -> Result<Self, CmAndError> {
        Self::from_fn(inputs.len(), |i| inputs[i])
    }

    /// Honest witness without retaining an input buffer.
    pub fn from_fn(
        gates: usize,
        mut input: impl FnMut(usize) -> (u32, u32),
    ) -> Result<Self, CmAndError> {
        Self::from_gate_values(gates, |i| {
            let (x, y) = input(i);
            (x, y, x & y, x ^ y)
        })
    }

    /// Arbitrary `(x, y, z, w)` gate values — no honesty is imposed, so
    /// adversarial fixtures (false relations, wrong XOR words) can be
    /// built through the same layout code.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_gate_values(
        gates: usize,
        mut values: impl FnMut(usize) -> (u32, u32, u32, u32),
    ) -> Result<Self, CmAndError> {
        let layout = CmAndLayout::new(gates)?;
        let capacity = layout.capacity;
        let mut assignment = vec![0u64; layout.assignment_len()];
        assignment[0] = 1;
        for gate in 0..gates {
            let (x, y, z, w) = values(gate);
            assignment[capacity + gate] = u64::from(x);
            assignment[2 * capacity + gate] = u64::from(y);
            assignment[3 * capacity + gate] = u64::from(z);
            assignment[4 * capacity + gate] = u64::from(w);
        }
        Ok(Self {
            layout,
            assignment: assignment.into_boxed_slice(),
        })
    }

    /// Shape shared by the assignment and both bit grids.
    pub const fn layout(&self) -> &CmAndLayout {
        &self.layout
    }

    /// Complete block-aligned integer assignment.
    pub fn assignment(&self) -> &[u64] {
        &self.assignment
    }

    fn block(&self, index: usize) -> &[u64] {
        let capacity = self.layout.capacity;
        &self.assignment[index * capacity..(index + 1) * capacity]
    }

    /// Builds the compact committed rows of `f` (`x`/`y`/`z` bits; slots
    /// `[96, 128)` zero) in the commit layout of the shared shape.
    pub fn f_bit_rows(&self) -> Vec<Vec<u64>> {
        self.bit_rows(&[(CM_AND_X_SLOT, 1), (CM_AND_Y_SLOT, 2), (CM_AND_Z_SLOT, 3)])
    }

    /// Builds synthesized derived rows of `h` (`x`/`y`/`z`/`w` bits).
    pub fn h_bit_rows(&self) -> Vec<Vec<u64>> {
        self.bit_rows(&[
            (CM_AND_X_SLOT, 1),
            (CM_AND_Y_SLOT, 2),
            (CM_AND_Z_SLOT, 3),
            (CM_AND_W_SLOT, 4),
        ])
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn bit_rows(&self, blocks: &[(usize, usize)]) -> Vec<Vec<u64>> {
        let p = self.layout.f2z_params();
        let words_per_row = p.rows() / u64::BITS as usize;
        let mut rows = vec![vec![0u64; words_per_row]; p.cols()];
        for gate in 0..self.layout.gates {
            for &(slot_start, block_index) in blocks {
                let value = self.block(block_index)[gate];
                for bit in 0..CM_AND_WORD_BITS {
                    if value & (1u64 << bit) != 0 {
                        let (b, c) = self
                            .layout
                            .cell(slot_start + bit, gate)
                            .expect("witness bit coordinates are in bounds");
                        rows[c][b / u64::BITS as usize] |= 1u64 << (b % u64::BITS as usize);
                    }
                }
            }
        }
        rows
    }
}

/// Prepared CM-AND statement: the layout, the field-valued Spartan
/// matrices (`A = B = 0`, one `C` row `x + y − 2z − w = 0` per live
/// gate), and the canonical derivation map with its digest.
#[derive(Clone, Debug)]
pub struct PreparedCmAndRelation<F>
where
    F: SpartanField,
{
    layout: CmAndLayout,
    matrices: PreparedConstraintMatrices<F>,
    map: PreparedVirtualMap,
}

impl<F> PreparedCmAndRelation<F>
where
    F: SpartanField,
{
    /// The shared layout.
    pub const fn layout(&self) -> &CmAndLayout {
        &self.layout
    }

    /// Prepared Spartan matrices.
    pub const fn matrices(&self) -> &PreparedConstraintMatrices<F> {
        &self.matrices
    }

    /// The canonical structural derivation map `M`.
    pub const fn map(&self) -> &PreparedVirtualMap {
        &self.map
    }
}

/// Generates and prepares the CM-AND statement over `F`.
#[allow(clippy::arithmetic_side_effects)]
pub fn prepare_cm_and_relation<F>(
    layout: CmAndLayout,
    field_config: &F::Config,
) -> Result<PreparedCmAndRelation<F>, CmAndError>
where
    F: SpartanField,
{
    let capacity = layout.capacity;
    let columns = layout.assignment_len();
    let live = layout.gates;

    let one = F::one_with_cfg(field_config);
    let mut minus_one = F::zero_with_cfg(field_config);
    minus_one -= &one;
    let mut minus_two = minus_one.clone();
    minus_two -= &one;

    let empty = SparseMatrix::try_from_rows(columns, vec![Vec::new(); live])
        .map_err(SpartanMatrixError::from)?;
    let c_rows: Vec<Vec<(usize, F)>> = (0..live)
        .map(|i| {
            vec![
                (capacity + i, one.clone()),
                (2 * capacity + i, one.clone()),
                (3 * capacity + i, minus_two.clone()),
                (4 * capacity + i, minus_one.clone()),
            ]
        })
        .collect();
    let c = SparseMatrix::try_from_rows(columns, c_rows).map_err(SpartanMatrixError::from)?;
    let matrices = PreparedConstraintMatrices::new(
        ConstraintMatrices::new(empty.clone(), empty, c)?,
        field_config,
    )?;
    let map = cm_and_map(&layout)?;
    Ok(PreparedCmAndRelation {
        layout,
        matrices,
        map,
    })
}

/// Field projection of a CM-AND assignment and its (all-linear) R1CS
/// products: `Az = Bz = 0`; `Cz` is COMPUTED from the values, so a false
/// witness produces a nonzero residual and a rejecting proof.
#[derive(Clone, Debug)]
pub struct ProjectedCmAndWitness<F> {
    layout: CmAndLayout,
    spartan: EvaluatedSpartanAssignment<F>,
    h_rows: Vec<Vec<u64>>,
}

impl<F> ProjectedCmAndWitness<F> {
    /// The shared layout.
    pub const fn layout(&self) -> &CmAndLayout {
        &self.layout
    }

    /// Assignment and evaluated matrix products consumed by Spartan.
    pub const fn spartan(&self) -> &EvaluatedSpartanAssignment<F> {
        &self.spartan
    }

    /// Packed synthesized rows consumed by the virtual F2Z prover.
    pub fn h_rows(&self) -> &[Vec<u64>] {
        &self.h_rows
    }

    /// Moves out the layout, Spartan witness bundle, and synthesized rows.
    pub fn into_parts(self) -> (CmAndLayout, EvaluatedSpartanAssignment<F>, Vec<Vec<u64>>) {
        (self.layout, self.spartan, self.h_rows)
    }
}

/// Projects the integer assignment into a Spartan field.
#[allow(clippy::arithmetic_side_effects)]
pub fn project_cm_and_witness<F>(
    witness: &CmAndWitness,
    field_config: &F::Config,
) -> Result<ProjectedCmAndWitness<F>, CmAndError>
where
    F: SpartanField + FromWithConfig<u64>,
{
    F::validate_config(field_config).map_err(SpartanMatrixError::from)?;
    let field_assignment: Vec<F> = witness
        .assignment()
        .iter()
        .copied()
        .map(|value| F::from_with_cfg(value, field_config))
        .collect();

    let live = witness.layout.gates;
    let zero = F::zero_with_cfg(field_config);
    let zeros = vec![zero; live];
    let cz: Vec<F> = (0..live)
        .map(|i| {
            // x + y − 2z − w, in the field.
            let mut acc = field_assignment[witness.layout.capacity + i].clone();
            acc += &field_assignment[2 * witness.layout.capacity + i];
            let mut two_z = field_assignment[3 * witness.layout.capacity + i].clone();
            two_z += &field_assignment[3 * witness.layout.capacity + i];
            acc -= &two_z;
            acc -= &field_assignment[4 * witness.layout.capacity + i];
            acc
        })
        .collect();
    let products = build_product_mles(&zeros, &zeros, &cz, live, field_config)?;
    let assignment = build_assignment_mle(
        &field_assignment,
        witness.layout.assignment_len(),
        field_config,
    )?;
    Ok(ProjectedCmAndWitness {
        layout: witness.layout,
        spartan: EvaluatedSpartanAssignment::new(assignment, products),
        h_rows: witness.h_bit_rows(),
    })
}

/// A terminal virtual-F2Z read-off claim derived from a CM-AND Spartan
/// assignment claim: row weights over the DERIVED grid `h`, clear-column
/// weights, and the constant-adjusted claimed value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CmOpeningClaim {
    row_weights_q: Vec<u128>,
    col_weights: Vec<Fq>,
    claimed: Fq,
}

impl CmOpeningClaim {
    /// Canonical `F_q` representatives of the derived-grid row weights.
    pub fn row_weights_q(&self) -> &[u128] {
        &self.row_weights_q
    }

    /// Clear-column weights of the derived grid.
    pub fn col_weights(&self) -> &[Fq] {
        &self.col_weights
    }

    /// Claimed value after subtracting the public constant-block term.
    pub const fn claimed(&self) -> Fq {
        self.claimed
    }
}

fn checked_pow2(exponent: usize) -> Result<usize, CmF2zError> {
    let exponent = u32::try_from(exponent).map_err(|_| CmF2zError::InvalidF2zParameters)?;
    1usize
        .checked_shl(exponent)
        .ok_or(CmF2zError::InvalidF2zParameters)
}

fn validate_cm_claim_field(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
) -> Result<(), CmF2zError> {
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    let has_expected_field = |value: &SpartanF2zField| {
        SpartanF2zField::canonical_modulus_encoding(value.cfg()) == expected
            && value.validate_element().is_ok()
            && value.canonical_u128() < FQ_MOD
    };
    if !has_expected_field(claim.scale())
        || !has_expected_field(claim.value())
        || claim.point().iter().any(|value| !has_expected_field(value))
    {
        return Err(CmF2zError::ClaimFieldMismatch);
    }
    Ok(())
}

fn validate_cm_layout_geometry(layout: &CmAndLayout) -> Result<(), CmF2zError> {
    let p = layout.f2z_params();
    if p.word_bits != 1
        || p.t < LOG_PACKING
        || p.s > layout.gate_vars()
        || p.t.saturating_add(p.word_bits) > 126
        || CM_AND_H_SLOTS != 1usize << 7
    {
        return Err(CmF2zError::InvalidF2zParameters);
    }
    let total_vars =
        p.t.checked_add(p.s)
            .ok_or(CmF2zError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(7)
            .ok_or(CmF2zError::InvalidF2zParameters)?
        || packed_vars(&p)
            != p.t
                .checked_sub(LOG_PACKING)
                .and_then(|f| f.checked_add(p.s))
                .ok_or(CmF2zError::InvalidF2zParameters)?
    {
        return Err(CmF2zError::InvalidF2zParameters);
    }
    let cells = checked_pow2(p.t)?
        .checked_mul(checked_pow2(p.s)?)
        .ok_or(CmF2zError::InvalidF2zParameters)?;
    if cells
        != CM_AND_H_SLOTS
            .checked_mul(layout.capacity())
            .ok_or(CmF2zError::InvalidF2zParameters)?
    {
        return Err(CmF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_cm_relation(
    relation: &PreparedCmAndRelation<SpartanF2zField>,
) -> Result<(), CmF2zError> {
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    if relation.matrices().field_modulus_encoding() != expected {
        return Err(CmF2zError::UnsupportedFieldModulus);
    }
    let p = relation.layout().f2z_params();
    if relation.map.rows() != cell_count(&p) || relation.map.cols() != cell_count(&p) {
        return Err(CmF2zError::InvalidF2zParameters);
    }
    Ok(())
}

/// Applies the adjoint of the four public 32-bit reconstructions to a
/// terminal scaled assignment-MLE claim, producing weights over the
/// DERIVED grid `h`. Spartan's point is low-coordinate-first: `gate_vars`
/// gate coordinates, then three block-selector coordinates
/// (`const = 0, x = 1, y = 2, z = 3, w = 4`; blocks 5–7 are the zero
/// padding of the assignment domain and contribute nothing).
#[allow(clippy::arithmetic_side_effects)]
pub fn bitify_cm_and_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &CmAndLayout,
) -> Result<CmOpeningClaim, CmF2zError> {
    validate_cm_layout_geometry(layout)?;
    validate_cm_claim_field(claim)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(SELECTOR_VARS) {
        return Err(CmF2zError::InvalidClaimPoint);
    }

    let p = layout.f2z_params();
    let point = claim
        .point()
        .iter()
        .map(|value| Fq(value.canonical_u128()))
        .collect::<Vec<_>>();
    let gate_point = &point[..gate_vars];
    let eq_sel = eq_le_table_fq(&point[gate_vars..]);

    let (gate_low, gate_high) = gate_point.split_at(p.s);
    let eq_low = eq_le_table_fq(gate_low);
    let eq_high = eq_le_table_fq(gate_high);
    if eq_sel.len() != 1 << SELECTOR_VARS
        || eq_low.len() != checked_pow2(p.s)?
        || eq_high.len() != checked_pow2(gate_vars - p.s)?
    {
        return Err(CmF2zError::InvalidF2zParameters);
    }

    let scale = Fq(claim.scale().canonical_u128());
    let value = Fq(claim.value().canonical_u128());
    let constant_evaluation = eq_sel[0] * eq_low[0] * eq_high[0];
    let adjusted_claim = Fq(fq_sub(value.0, fq_mul(scale.0, constant_evaluation.0)));

    let row_count = checked_pow2(p.t)?;
    let high_gate_vars = gate_vars - p.s;
    let mut row_weights_q = vec![0u128; row_count];
    for (block, slot_start) in [
        (1usize, CM_AND_X_SLOT),
        (2, CM_AND_Y_SLOT),
        (3, CM_AND_Z_SLOT),
        (4, CM_AND_W_SLOT),
    ] {
        fill_slot_weights(
            &mut row_weights_q,
            slot_start,
            CM_AND_WORD_BITS,
            eq_sel[block],
            &eq_high,
            high_gate_vars,
        )?;
    }

    // The Spartan scale rides the clear column side, exactly as in the
    // direct bridge; the zero-weight and constant-only degeneracies get
    // the same deterministic handling.
    let mut col_weights = eq_low
        .into_iter()
        .map(|weight| scale * weight)
        .collect::<Vec<_>>();
    if row_weights_q.iter().all(|&weight| weight == 0) {
        if adjusted_claim != Fq(0) {
            return Err(CmF2zError::InvalidConstantOnlyClaim);
        }
        row_weights_q[0] = 1;
        col_weights.fill(Fq(0));
    }

    Ok(CmOpeningClaim {
        row_weights_q,
        col_weights,
        claimed: adjusted_claim,
    })
}

/// Virtualized opening paired with the ordinary Spartan PIOP.
pub type CmF2zProof = SpartanF2zProof<SpartanPiopProof<SpartanF2zField>, Virtualized>;

fn cm_configs(layout: &CmAndLayout) -> Result<(LigProverConfig, LigVerifierConfig), CmF2zError> {
    if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
        return Err(CmF2zError::UnauditedF2zParameters);
    }
    let p = layout.f2z_params();
    sha_lig_configs(packed_vars(&p)).map_err(CmF2zError::LigeritoConfig)
}

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), CmF2zError> {
    let value = u64::try_from(value).map_err(|_| CmF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn cm_assignment_binding(
    layout: &CmAndLayout,
    map_digest: &[u8; 32],
    commitment: &Commitment,
) -> Result<[u8; 32], CmF2zError> {
    use super::f2z::{hash_code, profile_code};
    let p = layout.f2z_params();
    let params = &commitment.params;
    let mut hasher = Hasher::new();
    hasher.update(CM_ASSIGNMENT_BINDING_DOMAIN);
    hasher.update(&commitment.root);
    hash_usize(&mut hasher, params.m)?;
    hash_usize(&mut hasher, params.log_inv_rate)?;
    hash_usize(&mut hasher, params.log_batch_size)?;
    hasher.update(&[profile_code(params.profile)]);
    hasher.update(&[hash_code(params.merkle_hash)]);
    hasher.update(&FQ_MOD.to_le_bytes());
    hash_usize(&mut hasher, layout.gates())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, p.t)?;
    hash_usize(&mut hasher, p.s)?;
    hash_usize(&mut hasher, p.word_bits)?;
    hasher.update(map_digest);
    Ok(*hasher.finalize().as_bytes())
}

fn cm_opening_claim_digest(
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    assignment_binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &CmOpeningClaim,
) -> Result<[u8; 32], CmF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(CM_OPENING_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hasher.update(relation.matrices().digest());
    hash_usize(&mut hasher, terminal_claim.point().len())?;
    for coordinate in terminal_claim.point() {
        hasher.update(&coordinate.canonical_element_encoding());
    }
    hasher.update(&terminal_claim.scale().canonical_element_encoding());
    hasher.update(&terminal_claim.value().canonical_element_encoding());
    hash_usize(&mut hasher, opening.row_weights_q.len())?;
    for weight in &opening.row_weights_q {
        hasher.update(&weight.to_le_bytes());
    }
    hash_usize(&mut hasher, opening.col_weights.len())?;
    for weight in &opening.col_weights {
        hasher.update(&weight.0.to_le_bytes());
    }
    hasher.update(&opening.claimed.0.to_le_bytes());
    Ok(*hasher.finalize().as_bytes())
}

fn absorb_cm_opening_claim<T: Transcript>(
    transcript: &mut T,
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    assignment_binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &CmOpeningClaim,
) -> Result<(), CmF2zError> {
    let digest = cm_opening_claim_digest(relation, assignment_binding, terminal_claim, opening)?;
    absorb_spartan_message(transcript, CM_OPENING_CLAIM_DOMAIN, &digest);
    Ok(())
}

/// Commits prebuilt compact `f` rows (`x`/`y`/`z` bits) under an explicit
/// Ligerito configuration. Configurations below the audited `m ≥ 22`
/// regime are TEST-ONLY, exactly as for the direct bridge.
pub fn commit_cm_and_witness_with_config(
    layout: &CmAndLayout,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, CmF2zError> {
    validate_cm_layout_geometry(layout)?;
    let p = layout.f2z_params();
    validate_bit_rows(&p, &rows)?;
    let hint = commit_rs_ligerito_rows(&p, rows, pc);
    validate_commitment(&p, &hint.commitment, pc)?;
    Ok(hint)
}

/// Commits prebuilt compact `f` rows with the production (validator-gated,
/// `≥ 2^15` gate slots) configuration.
pub fn commit_cm_and_witness(
    layout: &CmAndLayout,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, CmF2zError> {
    let (pc, vc) = cm_configs(layout)?;
    let p = layout.f2z_params();
    validate_config_pair(&p, &pc, &vc)?;
    commit_cm_and_witness_with_config(layout, rows, &pc)
}

/// Proves the CM-AND relation and opens the derived-grid assignment claim
/// against the compact `f` commitment, under an explicit configuration.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_cm_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    witness: ProjectedCmAndWitness<SpartanF2zField>,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<CmF2zProof, CmF2zError> {
    validate_cm_relation(relation)?;
    validate_cm_layout_geometry(relation.layout())?;
    let p = relation.layout().f2z_params();
    validate_bit_rows(&p, hint_f.rows())?;
    validate_commitment(&p, &hint_f.commitment, pc)?;
    if witness.layout() != relation.layout() {
        return Err(CmF2zError::RelationWitnessLayoutMismatch);
    }
    let assignment_binding = cm_assignment_binding(
        relation.layout(),
        &relation.map().digest(),
        &hint_f.commitment,
    )?;
    let (_, spartan_witness, h_rows) = witness.into_parts();
    let (assignment, products) = spartan_witness.into_parts();

    let (spartan, terminal_claim) = {
        let _scope = crate::utils::prof::scope("cm-f2z:spartan_prove");
        prove_spartan_piop(
            transcript,
            relation.matrices(),
            &assignment_binding,
            products,
            assignment,
        )?
    };

    let opening = {
        let _scope = crate::utils::prof::scope("cm-f2z:bitify_prover");
        let opening = bitify_cm_and_claim(&terminal_claim, relation.layout())?;
        absorb_cm_opening_claim(
            transcript,
            relation,
            &assignment_binding,
            &terminal_claim,
            &opening,
        )?;
        opening
    };

    let f2z = {
        let _scope = crate::utils::prof::scope("cm-f2z:f2z_prove");
        prove_mle_eval_mod_q_ligerito_virtual(
            transcript,
            hint_f,
            &h_rows,
            &p,
            &p,
            relation.map(),
            opening.row_weights_q(),
            FQ_BITS,
            f2z_generator(),
            pc,
        )
    };

    Ok(CmF2zProof::new(spartan, f2z))
}

/// Proves with the production (validator-gated) configuration.
pub fn prove_cm_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    witness: ProjectedCmAndWitness<SpartanF2zField>,
    hint_f: &FlockCommitHint,
) -> Result<CmF2zProof, CmF2zError> {
    let (pc, vc) = cm_configs(relation.layout())?;
    let p = relation.layout().f2z_params();
    validate_config_pair(&p, &pc, &vc)?;
    prove_cm_and_f2z_with_config(transcript, relation, witness, hint_f, &pc)
}

/// Verifies both proof systems on one transcript, under an explicit
/// configuration. The terminal Spartan claim is always derived from
/// `proof.spartan`, never trusted from the prover.
#[allow(clippy::arithmetic_side_effects)]
pub fn verify_cm_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    commitment: &Commitment,
    proof: &CmF2zProof,
    vc: &LigVerifierConfig,
) -> Result<(), CmF2zError> {
    validate_cm_relation(relation)?;
    validate_cm_layout_geometry(relation.layout())?;
    let assignment_binding =
        cm_assignment_binding(relation.layout(), &relation.map().digest(), commitment)?;

    let terminal_claim = {
        let _scope = crate::utils::prof::scope("cm-f2z:spartan_verify");
        verify_spartan_proof(
            transcript,
            relation.matrices(),
            &assignment_binding,
            proof.spartan(),
        )?
    };

    let opening = {
        let _scope = crate::utils::prof::scope("cm-f2z:bitify_verifier");
        let opening = bitify_cm_and_claim(&terminal_claim, relation.layout())?;
        absorb_cm_opening_claim(
            transcript,
            relation,
            &assignment_binding,
            &terminal_claim,
            &opening,
        )?;
        opening
    };

    let p = relation.layout().f2z_params();
    let result = {
        let _scope = crate::utils::prof::scope("cm-f2z:f2z_verify");
        verify_mle_eval_mod_q_ligerito_virtual(
            transcript,
            commitment,
            proof.f2z(),
            &p,
            &p,
            relation.map(),
            opening.row_weights_q(),
            opening.col_weights(),
            f2z_generator(),
            opening.claimed(),
            FQ_BITS,
            vc,
        )
    };
    result.map_err(CmF2zError::F2z)
}

/// Verifies with the production (validator-gated) configuration.
pub fn verify_cm_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    relation: &PreparedCmAndRelation<SpartanF2zField>,
    commitment: &Commitment,
    proof: &CmF2zProof,
) -> Result<(), CmF2zError> {
    let (pc, vc) = cm_configs(relation.layout())?;
    let p = relation.layout().f2z_params();
    validate_config_pair(&p, &pc, &vc)?;
    validate_commitment(&p, commitment, &pc)?;
    verify_cm_and_f2z_with_config(transcript, relation, commitment, proof, &vc)
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{crypto_bigint_monty::F128, crypto_bigint_uint::Uint, PrimeField};

    use super::*;

    #[test]
    fn layout_and_cells_are_slot_major() {
        assert_eq!(CmAndLayout::new(0), Err(CmAndError::EmptyBatch));
        for (gates, capacity) in [(1, 256), (200, 256), (257, 512)] {
            let layout = CmAndLayout::new(gates).unwrap();
            assert_eq!(layout.capacity(), capacity);
            assert_eq!(layout.assignment_len(), 5 * capacity);
            let p = layout.f2z_params();
            assert_eq!(p.t + p.s, layout.gate_vars() + 7);
            for slot in [0, 31, 32, 95, 96, 127] {
                for gate in [0, capacity - 1] {
                    let (b, c) = layout.cell(slot, gate).unwrap();
                    assert_eq!(layout.flat_cell(slot, gate), (c << p.t) | b);
                }
            }
            assert!(layout.cell(128, 0).is_none());
        }
    }

    #[test]
    fn map_derives_w_as_xor_and_keeps_dead_slots_unreferenced() {
        let layout = CmAndLayout::new(3).unwrap();
        let map = cm_and_map(&layout).unwrap();
        let p = layout.f2z_params();
        assert_eq!(map.rows(), cell_count(&p));
        assert_eq!(map.cols(), cell_count(&p));
        assert_eq!(map.nnz(), layout.capacity() * (CM_AND_F_LIVE_SLOTS + 64));

        // Source x/y columns feed both their identity row and the matching w row.
        for (slot, gate) in [(0usize, 0usize), (40, 2), (63, 255)] {
            let source = layout.flat_cell(slot, gate);
            let column = map.matrix().column(source).unwrap();
            let bit = slot % CM_AND_WORD_BITS;
            assert_eq!(
                column.row_indices(),
                &[source, layout.flat_cell(CM_AND_W_SLOT + bit, gate)]
            );
        }
        // Source z columns feed only their identity row.
        for (slot, gate) in [(64usize, 0usize), (81, 2), (95, 255)] {
            let source = layout.flat_cell(slot, gate);
            assert_eq!(
                map.matrix().column(source).unwrap().row_indices(),
                &[source]
            );
        }
        // Padded source w slots are structurally dead.
        for (bit, gate) in [(0usize, 0usize), (17, 2), (31, 255)] {
            let source = layout.flat_cell(CM_AND_W_SLOT + bit, gate);
            assert!(map.matrix().column(source).unwrap().is_empty());
        }
    }

    #[test]
    fn witness_blocks_and_bit_rows_are_consistent() {
        let inputs = [(0xdead_beefu32, 0x0f0f_0f0f), (u32::MAX, 1), (7, 7)];
        let witness = CmAndWitness::from_inputs(&inputs).unwrap();
        let capacity = witness.layout().capacity();
        assert_eq!(witness.assignment()[0], 1);
        for (i, (x, y)) in inputs.iter().enumerate() {
            assert_eq!(witness.assignment()[capacity + i], u64::from(*x));
            assert_eq!(witness.assignment()[2 * capacity + i], u64::from(*y));
            assert_eq!(witness.assignment()[3 * capacity + i], u64::from(x & y));
            assert_eq!(witness.assignment()[4 * capacity + i], u64::from(x ^ y));
        }

        let f_rows = witness.f_bit_rows();
        let h_rows = witness.h_bit_rows();
        let layout = witness.layout();
        for (gate, (x, y)) in inputs.iter().enumerate() {
            for (slot_start, value) in [
                (CM_AND_X_SLOT, *x),
                (CM_AND_Y_SLOT, *y),
                (CM_AND_Z_SLOT, x & y),
            ] {
                for bit in 0..CM_AND_WORD_BITS {
                    let (b, c) = layout.cell(slot_start + bit, gate).unwrap();
                    let committed = (f_rows[c][b / 64] >> (b % 64)) & 1;
                    assert_eq!(committed, u64::from((value >> bit) & 1));
                    assert_eq!((h_rows[c][b / 64] >> (b % 64)) & 1, committed);
                }
            }
            // The w block is NOT committed.
            for bit in 0..CM_AND_WORD_BITS {
                let (b, c) = layout.cell(CM_AND_W_SLOT + bit, gate).unwrap();
                assert_eq!((f_rows[c][b / 64] >> (b % 64)) & 1, 0);
                assert_eq!(
                    (h_rows[c][b / 64] >> (b % 64)) & 1,
                    u64::from(((x ^ y) >> bit) & 1)
                );
            }
        }
    }

    #[test]
    fn honest_witness_satisfies_the_relation_and_false_one_does_not() {
        let config = spartan_f2z_field_config();
        let witness = CmAndWitness::from_inputs(&[(3, 5), (0xffff_0000, 0x00ff_00ff)]).unwrap();
        let projected = project_cm_and_witness::<SpartanF2zField>(&witness, &config).unwrap();
        let zero = SpartanF2zField::zero_with_cfg(&config);
        assert!(projected
            .spartan()
            .products()
            .cz
            .evaluations
            .iter()
            .all(|v| v == &zero));

        let bad = CmAndWitness::from_gate_values(2, |i| {
            let (x, y) = [(3u32, 5u32), (0xffff_0000, 0x00ff_00ff)][i];
            let z = if i == 0 { (x & y) ^ 1 } else { x & y };
            (x, y, z, x ^ y)
        })
        .unwrap();
        let projected = project_cm_and_witness::<SpartanF2zField>(&bad, &config).unwrap();
        assert_ne!(projected.spartan().products().cz.evaluations[0], zero);
        assert_eq!(projected.spartan().products().cz.evaluations[1], zero);
    }

    #[test]
    fn relation_coefficients_follow_the_runtime_field_modulus() {
        let config = F128::make_cfg(&Uint::from((1_u128 << 127) - 1)).unwrap();
        let witness = CmAndWitness::from_inputs(&[(3, 5), (0xffff_0000, 0x00ff_00ff)]).unwrap();
        let relation = prepare_cm_and_relation::<F128>(*witness.layout(), &config).unwrap();
        let projected = project_cm_and_witness::<F128>(&witness, &config).unwrap();

        let mut matrix_products = vec![F128::zero_with_cfg(&config); witness.layout().gates()];
        for (column, entries) in relation.matrices().matrices().c().columns().enumerate() {
            for (row, coefficient) in entries {
                let mut term = coefficient.clone();
                term *= &projected.spartan().assignment().evaluations[column];
                matrix_products[row] += &term;
            }
        }

        assert_eq!(
            &projected.spartan().products().cz.evaluations[..witness.layout().gates()],
            matrix_products.as_slice(),
        );
        assert!(matrix_products
            .iter()
            .all(|value| <F128 as PrimeField>::is_zero(value)));
    }

    #[test]
    fn bitification_is_the_adjoint_of_the_blocked_reconstruction() {
        use crypto_primitives::FromWithConfig;
        let witness =
            CmAndWitness::from_inputs(&[(0, u32::MAX), (1, 7), (u32::MAX, u32::MAX)]).unwrap();
        let layout = *witness.layout();
        let p = layout.f2z_params();
        let config = spartan_f2z_field_config();

        let gate_point: Vec<Fq> = (0..layout.gate_vars())
            .map(|i| Fq((i + 2) as u128))
            .collect();
        let sel: [Fq; 3] = [Fq(7), Fq(11), Fq(29)];
        let scale = Fq(13);
        let eq_sel = eq_le_table_fq(&sel);
        let eq_gate = eq_le_table_fq(&gate_point);

        // Z(point) directly from the five logical blocks.
        let mut z_eval = Fq(0);
        for block in 0..ASSIGNMENT_BLOCKS {
            for gate in 0..layout.capacity() {
                z_eval = z_eval
                    + eq_sel[block]
                        * eq_gate[gate]
                        * Fq::from(u128::from(
                            witness.assignment()[block * layout.capacity() + gate],
                        ));
            }
        }
        let value = scale * z_eval;

        let mut point = gate_point.clone();
        point.extend(sel);
        let point_f: Vec<SpartanF2zField> = point
            .iter()
            .map(|c| SpartanF2zField::from_with_cfg(c.0, &config))
            .collect();
        let claim = ScaledMleEvaluationClaim::new(
            point_f.into_boxed_slice(),
            SpartanF2zField::from_with_cfg(scale.0, &config),
            SpartanF2zField::from_with_cfg(value.0, &config),
        );
        let opening = bitify_cm_and_claim(&claim, &layout).unwrap();

        // Read the DERIVED grid h = M·f through the opening weights.
        let h_rows = witness.h_bit_rows();
        let mut read_off = Fq(0);
        for b in 0..p.rows() {
            for c in 0..p.cols() {
                let bit = (h_rows[c][b / 64] >> (b % 64)) & 1;
                read_off = read_off
                    + Fq::from(u128::from(bit))
                        * Fq(opening.row_weights_q()[b])
                        * opening.col_weights()[c];
            }
        }
        assert_eq!(read_off, opening.claimed());
    }

    #[test]
    fn production_entry_points_gate_small_layouts() {
        let layout = CmAndLayout::new(3).unwrap();
        assert!(matches!(
            cm_configs(&layout),
            Err(CmF2zError::UnauditedF2zParameters)
        ));
        let production = CmAndLayout::new(1 << MIN_PRODUCTION_GATE_VARS).unwrap();
        cm_configs(&production).expect("the smallest embedded profile is available");
    }
}
