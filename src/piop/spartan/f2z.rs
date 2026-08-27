//! Succinct opening of Spartan's terminal assignment claim through F2Z.
//!
//! Spartan constrains the integer assignment
//!
//! ```text
//! [constant block | x block | y block | product block]
//! ```
//!
//! over `q = 2^100 - 15`.  The commitment contains only the compact
//! `32 + 32 + 64` little-endian bits for each multiplication.  This module
//! applies the transpose of that public bitification map to Spartan's terminal
//! assignment-MLE claim and discharges the resulting claim with F2Z.

use std::sync::OnceLock;

use blake3::Hasher;
use crypto_primitives::{PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint};
use flock_core::{
    merkle::HashKind,
    pcs::{
        commit::Commitment,
        ligerito::{
            LigeritoProfile, ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig,
        },
    },
};
use thiserror::Error;

use crate::{
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, commit_rs_ligerito_rows,
        prove_mle_eval_mod_q_ligerito, sha_lig_configs, verify_mle_eval_mod_q_ligerito,
    },
    pcs::{FQ_BITS, FQ_MOD, Fq, ProjectCanonicalU128, eq_le_table_fq, fq_mul, fq_sub},
    poly::{mle::DenseMultilinearExtension, univariate::binary_gf128::BinaryFieldGF128},
    transcript::traits::Transcript,
};

use super::{
    PreparedConstraintMatrices, R1csProductMles, SpartanField, absorb_spartan_message,
    matrix::ScaledMleEvaluationClaim,
    piop::{
        SpartanError, SpartanPiopProof, SpartanReductionStrategy,
        prove_spartan_piop_u32_native_with_strategy, prove_spartan_piop_with_strategy,
        verify_spartan_proof,
    },
    u32_mul::{
        U32_MUL_BIT_SLOTS, U32_MUL_PRODUCT_BITS, U32_MUL_PRODUCT_SLOT_START, U32_MUL_X_BITS,
        U32_MUL_X_SLOT_START, U32_MUL_Y_BITS, U32_MUL_Y_SLOT_START, U32MulError, U32MulLayout,
        U32MulWitness, project_u32_mul_native_witness, project_u32_mul_witness,
    },
};

/// Domain of the commitment-and-layout digest used as Spartan's assignment
/// oracle binding.
const ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-f2z/assignment/v1";

/// Domain of the translated opening claim absorbed between Spartan and F2Z.
const OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-f2z/opening/v1";

/// Embedded, validator-gated Ligerito profiles begin at a 22-variable
/// committed bit MLE: seven slot variables plus fifteen gate variables.
const MIN_PRODUCTION_GATE_VARS: usize = 15;

/// Runtime-configured Spartan field used by the concrete F2Z adapter.
pub type SpartanF2zField = F128;

/// A terminal F2Z read-off claim obtained from a Spartan assignment claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct F2zOpeningClaim {
    row_weights_q: Vec<u128>,
    col_weights: Vec<Fq>,
    claimed: Fq,
}

impl F2zOpeningClaim {
    pub(crate) fn new(row_weights_q: Vec<u128>, col_weights: Vec<Fq>, claimed: Fq) -> Self {
        Self {
            row_weights_q,
            col_weights,
            claimed,
        }
    }

    /// Canonical `F_q` representatives for the folded F2Z row weights.
    pub fn row_weights_q(&self) -> &[u128] {
        &self.row_weights_q
    }

    /// Clear-column weights used for F2Z's final read-off.
    pub fn col_weights(&self) -> &[Fq] {
        &self.col_weights
    }

    /// Claimed value after subtracting the public constant-block term.
    pub const fn claimed(&self) -> Fq {
        self.claimed
    }
}

/// The combined proof.  There is deliberately no combined proof codec.
#[derive(Clone)]
pub struct U32MulSpartanF2zProof {
    /// Spartan's outer and inner sumchecks.
    pub spartan: SpartanPiopProof<SpartanF2zField>,
    /// F2Z opening of the derived compact-bit claim.
    pub f2z: IntEvalRsLigModQProof,
}

impl U32MulSpartanF2zProof {
    /// Spartan proof component, exposed for benchmark payload accounting.
    pub const fn spartan(&self) -> &SpartanPiopProof<SpartanF2zField> {
        &self.spartan
    }

    /// F2Z proof component, exposed for its existing exact byte codec.
    pub const fn f2z(&self) -> &IntEvalRsLigModQProof {
        &self.f2z
    }
}

/// Failures in layout validation, claim translation, or either proof system.
#[derive(Debug, Error)]
pub enum SpartanF2zError {
    #[error(transparent)]
    Relation(#[from] U32MulError),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the prepared relation does not use q = 2^100 - 15")]
    UnsupportedFieldModulus,

    #[error("the prepared matrices do not match the u32-multiplication layout")]
    RelationWitnessLayoutMismatch,

    #[error("the compact bit rows do not match the u32-multiplication layout")]
    InvalidBitRows,

    #[error("the F2Z parameters are invalid for the compact u32-multiplication layout")]
    InvalidF2zParameters,

    #[error("the combined Spartan/F2Z proof requires at least 2^15 multiplication slots")]
    UnauditedF2zParameters,

    #[error("the commitment parameters do not match the derived F2Z configuration")]
    CommitmentConfigMismatch,

    #[error("the F2Z proof has an invalid top-level shape")]
    InvalidF2zProofShape,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element uses a field other than q = 2^100 - 15")]
    ClaimFieldMismatch,

    #[error("a constant-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOnlyClaim,

    #[error("a host length does not fit the canonical transcript encoding")]
    BindingEncodingOverflow,

    #[error("the bit width {0} is not supported by the Spartan/F2Z bridge")]
    InvalidBitWidth(usize),

    #[error("the direct F2Z tensor geometry does not match the Spartan assignment")]
    InvalidDirectGeometry,

    #[error("the structured virtualization matrix or its tensor layout is invalid")]
    InvalidVirtualizationMatrix,

    #[error("the virtualized witness does not match the public virtualization shape")]
    InvalidVirtualizedWitness,

    #[error("the compiled F2Z component claims do not equal the Spartan terminal claim")]
    VirtualClaimMismatch,

    #[error("the F2Z proof variant does not match the public virtualization mode")]
    ProofModeMismatch,

    #[error("the virtualized F2Z proof has an invalid component-claim shape")]
    InvalidVirtualComponentClaims,
}

/// Constructs the fixed `q = 2^100 - 15` runtime field configuration.
pub fn spartan_f2z_field_config() -> <SpartanF2zField as PrimeField>::Config {
    SpartanF2zField::make_cfg(&Uint::from(FQ_MOD)).expect("FQ_MOD is a valid odd prime modulus")
}

/// Commits prebuilt compact `32 + 32 + 64` bit rows.
///
/// Accepting ownership of `rows` lets benchmarks time bitification separately
/// and move the packed store into the commitment without retaining a duplicate.
/// Only layouts with at least `2^15` gate slots are accepted, because smaller
/// Ligerito configurations in the dependency are explicitly test-only.
pub fn commit_u32_mul_witness(
    layout: &U32MulLayout,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, SpartanF2zError> {
    let p = layout.f2z_params();
    validate_layout_geometry(layout)?;
    validate_bit_rows(&p, &rows)?;
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&p, &pc, &vc)?;

    // All assertion-bearing shape requirements of the low-level commit have
    // been checked above.
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    validate_commitment(&p, &hint.commitment, &pc)?;
    Ok(hint)
}

/// Applies the adjoint of the public 32/32/64-bit reconstruction to a
/// terminal scaled assignment-MLE claim.
///
/// Spartan's point is low-coordinate-first.  Its final two coordinates select
/// the four assignment blocks; the preceding coordinates select a gate.  F2Z
/// places the low gate coordinates on the clear column axis and the high gate
/// coordinates together with the 128 bit slots on the folded row axis.
pub fn bitify_u32_mul_spartan_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &U32MulLayout,
) -> Result<F2zOpeningClaim, SpartanF2zError> {
    validate_layout_geometry(layout)?;
    validate_claim_field(claim)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(2) {
        return Err(SpartanF2zError::InvalidClaimPoint);
    }

    let p = layout.f2z_params();
    let point = claim
        .point()
        .iter()
        .map(|value| Fq(value.canonical_u128()))
        .collect::<Vec<_>>();
    let gate_point = &point[..gate_vars];
    let block_low = point[gate_vars];
    let block_high = point[gate_vars + 1];
    let one = Fq(1);
    let one_minus_low = Fq(fq_sub(one.0, block_low.0));
    let one_minus_high = Fq(fq_sub(one.0, block_high.0));

    // Block order in the integer assignment is 00=constant, 01=x, 10=y,
    // 11=product, with the first block-selector coordinate as the low bit.
    let constant_factor = one_minus_low * one_minus_high;
    let x_factor = block_low * one_minus_high;
    let y_factor = one_minus_low * block_high;
    let product_factor = block_low * block_high;

    let (gate_low, gate_high) = gate_point.split_at(p.s);
    let eq_low = eq_le_table_fq(gate_low);
    let eq_high = eq_le_table_fq(gate_high);
    if eq_low.len() != checked_pow2(p.s)? || eq_high.len() != checked_pow2(gate_vars - p.s)? {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }

    let scale = Fq(claim.scale().canonical_u128());
    let value = Fq(claim.value().canonical_u128());
    let constant_evaluation = constant_factor * eq_low[0] * eq_high[0];
    let adjusted_claim = Fq(fq_sub(value.0, fq_mul(scale.0, constant_evaluation.0)));

    let row_count = checked_pow2(p.t)?;
    let high_gate_vars = gate_vars - p.s;
    let mut row_weights_q = vec![0_u128; row_count];
    fill_slot_weights(
        &mut row_weights_q,
        U32_MUL_X_SLOT_START,
        U32_MUL_X_BITS,
        x_factor,
        &eq_high,
        high_gate_vars,
    )?;
    fill_slot_weights(
        &mut row_weights_q,
        U32_MUL_Y_SLOT_START,
        U32_MUL_Y_BITS,
        y_factor,
        &eq_high,
        high_gate_vars,
    )?;
    fill_slot_weights(
        &mut row_weights_q,
        U32_MUL_PRODUCT_SLOT_START,
        U32_MUL_PRODUCT_BITS,
        product_factor,
        &eq_high,
        high_gate_vars,
    )?;

    // Put the Spartan scale on the clear column side.  Thus a zero scale does
    // not erase the row functional that the exponent-fold protocol certifies.
    let mut col_weights = eq_low
        .into_iter()
        .map(|weight| scale * weight)
        .collect::<Vec<_>>();

    // At block point 00 the variable part is identically zero.  The F2Z
    // prover still needs a nonempty row functional, so use a deterministic
    // dummy row with an all-zero clear read-off.  This requires no division or
    // challenge retry and preserves the zero adjusted claim exactly.
    if row_weights_q.iter().all(|&weight| weight == 0) {
        if adjusted_claim != Fq(0) {
            return Err(SpartanF2zError::InvalidConstantOnlyClaim);
        }
        row_weights_q[0] = 1;
        col_weights.fill(Fq(0));
    }

    Ok(F2zOpeningClaim {
        row_weights_q,
        col_weights,
        claimed: adjusted_claim,
    })
}

/// Proves the integer R1CS relation and succinctly opens the derived Spartan
/// assignment claim against the compact bit commitment.
///
/// This production entry point requires at least `2^15` multiplication slots.
pub fn prove_u32_mul_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    assignment: DenseMultilinearExtension<SpartanF2zField>,
    products: R1csProductMles<SpartanF2zField>,
    hint: &FlockCommitHint,
) -> Result<U32MulSpartanF2zProof, SpartanF2zError> {
    let (p, pc, assignment_binding) = prepare_combined_prover(matrices, layout, hint)?;
    let (spartan, terminal_claim) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
        prove_spartan_piop_with_strategy(
            transcript,
            matrices,
            &assignment_binding,
            products,
            assignment,
            SpartanReductionStrategy::Immediate,
        )?
    };
    finish_combined_prover(
        transcript,
        matrices,
        layout,
        hint,
        &p,
        &pc,
        &assignment_binding,
        spartan,
        terminal_claim,
    )
}

/// Proves the u32 multiplication workflow with a reduction strategy selected
/// once before entering the Spartan product loops.
pub fn prove_u32_mul_spartan_and_f2z_with_strategy<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    witness: &U32MulWitness,
    hint: &FlockCommitHint,
    strategy: SpartanReductionStrategy,
) -> Result<U32MulSpartanF2zProof, SpartanF2zError> {
    if witness.layout() != layout {
        return Err(SpartanF2zError::RelationWitnessLayoutMismatch);
    }
    let (p, pc, assignment_binding) = prepare_combined_prover(matrices, layout, hint)?;

    // Projection is deliberately outside the Spartan timing scope. Immediate
    // mode materializes the original field tables, while delayed modes keep
    // the exact u64 relation through the first outer and inner rounds.
    let (spartan, terminal_claim) = match strategy {
        SpartanReductionStrategy::Immediate => {
            let (assignment, products) =
                project_u32_mul_witness::<SpartanF2zField>(witness, matrices.config())?;
            let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
            prove_spartan_piop_with_strategy(
                transcript,
                matrices,
                &assignment_binding,
                products,
                assignment,
                strategy,
            )?
        }
        SpartanReductionStrategy::DelayedBarrett
        | SpartanReductionStrategy::DelayedCryptoBigint => {
            let (assignment, products) = project_u32_mul_native_witness(witness).into_parts();
            let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
            prove_spartan_piop_u32_native_with_strategy(
                transcript,
                matrices,
                &assignment_binding,
                products,
                assignment,
                strategy,
            )?
        }
    };

    finish_combined_prover(
        transcript,
        matrices,
        layout,
        hint,
        &p,
        &pc,
        &assignment_binding,
        spartan,
        terminal_claim,
    )
}

fn prepare_combined_prover(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    hint: &FlockCommitHint,
) -> Result<(crate::pcs::IntEvalParams, LigProverConfig, [u8; 32]), SpartanF2zError> {
    validate_relation(matrices)?;
    validate_relation_layout(matrices, layout)?;
    validate_layout_geometry(layout)?;
    let p = layout.f2z_params();
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&p, &pc, &vc)?;
    validate_bit_rows(&p, hint.rows())?;
    validate_commitment(&p, &hint.commitment, &pc)?;
    let assignment_binding = assignment_binding(layout, &hint.commitment)?;
    Ok((p, pc, assignment_binding))
}

#[allow(clippy::too_many_arguments)]
fn finish_combined_prover<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    hint: &FlockCommitHint,
    p: &crate::pcs::IntEvalParams,
    pc: &LigProverConfig,
    assignment_binding: &[u8; 32],
    spartan: SpartanPiopProof<SpartanF2zField>,
    terminal_claim: ScaledMleEvaluationClaim<SpartanF2zField>,
) -> Result<U32MulSpartanF2zProof, SpartanF2zError> {
    let opening = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_prover");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout)?;
        absorb_opening_claim(
            transcript,
            matrices,
            &assignment_binding,
            &terminal_claim,
            &opening,
        )?;
        opening
    };

    let f2z = {
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prove");
        prove_mle_eval_mod_q_ligerito(
            transcript,
            hint,
            p,
            opening.row_weights_q(),
            FQ_BITS,
            f2z_generator(),
            pc,
        )
    };

    Ok(U32MulSpartanF2zProof { spartan, f2z })
}

/// Verifies both proof systems on one transcript.
///
/// The terminal Spartan claim is always derived from `proof.spartan`; it is
/// never supplied by or trusted from the prover.
/// This production entry point requires at least `2^15` multiplication slots.
pub fn verify_u32_mul_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    commitment: &Commitment,
    proof: &U32MulSpartanF2zProof,
) -> Result<(), SpartanF2zError> {
    validate_relation(matrices)?;
    validate_relation_layout(matrices, layout)?;
    validate_layout_geometry(layout)?;
    let p = layout.f2z_params();
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&p, &pc, &vc)?;
    validate_commitment(&p, commitment, &pc)?;
    validate_f2z_proof_shape(&p, &proof.f2z)?;
    let assignment_binding = assignment_binding(layout, commitment)?;

    let terminal_claim = {
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_verify");
        verify_spartan_proof(transcript, matrices, &assignment_binding, &proof.spartan)?
    };

    let opening = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_verifier");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout)?;
        absorb_opening_claim(
            transcript,
            matrices,
            &assignment_binding,
            &terminal_claim,
            &opening,
        )?;
        opening
    };

    let result = {
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_verify");
        verify_mle_eval_mod_q_ligerito(
            transcript,
            commitment,
            &proof.f2z,
            &p,
            opening.row_weights_q(),
            opening.col_weights(),
            f2z_generator(),
            opening.claimed(),
            FQ_BITS,
            &vc,
        )
    };
    result.map_err(SpartanF2zError::F2z)
}

fn configs_for_layout(
    layout: &U32MulLayout,
) -> Result<(LigProverConfig, LigVerifierConfig), SpartanF2zError> {
    if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
        return Err(SpartanF2zError::UnauditedF2zParameters);
    }
    let p = layout.f2z_params();
    let m_p = packed_variables(&p)?;
    sha_lig_configs(m_p).map_err(SpartanF2zError::LigeritoConfig)
}

fn validate_layout_geometry(layout: &U32MulLayout) -> Result<(), SpartanF2zError> {
    let p = layout.f2z_params();
    if p.word_bits != 1
        || p.t < LOG_PACKING
        || p.s > layout.gate_vars()
        || p.t.saturating_add(p.word_bits) > 126
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let total_vars =
        p.t.checked_add(p.s)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(7)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?
        || U32_MUL_BIT_SLOTS != 1_usize << 7
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(p.t)?;
    let col_count = checked_pow2(p.s)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    let expected_cells = U32_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if cells != expected_cells || packed_variables(&p)? != layout.gate_vars() {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

pub(crate) fn validate_f2z_proof_shape(
    p: &crate::pcs::IntEvalParams,
    proof: &IntEvalRsLigModQProof,
) -> Result<(), SpartanF2zError> {
    if !p.word_bits.is_power_of_two() || p.word_bits > u128::BITS as usize {
        return Err(SpartanF2zError::InvalidF2zProofShape);
    }
    let row_bit_vars =
        p.t.checked_add(p.word_bits.trailing_zeros() as usize)
            .ok_or(SpartanF2zError::InvalidF2zProofShape)?;
    let chunks = crate::pcs::mod_q_num_chunks(p, FQ_BITS);
    let columns = checked_pow2(p.s)?;
    if proof.mfs.len() != chunks
        || proof.us.len() != chunks
        || proof.presums.len() != chunks
        || proof.rings.len() != chunks
        || proof
            .presums
            .iter()
            .any(|presum| !presum.has_shape(row_bit_vars, &[2]))
        || proof.us.iter().any(|values| values.len() > columns)
        || proof.rings.iter().any(|ring| ring.s_v.len() != 128)
    {
        return Err(SpartanF2zError::InvalidF2zProofShape);
    }
    Ok(())
}

fn validate_bit_rows(
    p: &crate::pcs::IntEvalParams,
    rows: &[Vec<u64>],
) -> Result<(), SpartanF2zError> {
    let row_count = checked_pow2(p.t)?;
    let col_count = checked_pow2(p.s)?;
    if row_count % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    let words_per_col = row_count / u64::BITS as usize;
    if rows.iter().any(|row| row.len() != words_per_col) {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    Ok(())
}

pub(crate) fn validate_config_pair(
    p: &crate::pcs::IntEvalParams,
    pc: &LigProverConfig,
    vc: &LigVerifierConfig,
) -> Result<(), SpartanF2zError> {
    let m_p = packed_variables(p)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(SpartanF2zError::InvalidF2zParameters);
    };
    if log_inv_rate == 0
        || pc.initial_k >= m_p
        || pc.initial_k != vc.initial_k
        || pc.log_inv_rates != vc.log_inv_rates
        || pc.merkle_hash != vc.merkle_hash
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

pub(crate) fn validate_commitment(
    p: &crate::pcs::IntEvalParams,
    commitment: &Commitment,
    pc: &LigProverConfig,
) -> Result<(), SpartanF2zError> {
    let m_p = packed_variables(p)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(SpartanF2zError::InvalidF2zParameters);
    };
    let params = &commitment.params;
    if params.m != m_p + LOG_PACKING
        || params.log_inv_rate != log_inv_rate
        || params.log_batch_size != pc.initial_k
        || params.profile != LigeritoProfile::default()
        || params.merkle_hash != pc.merkle_hash
    {
        return Err(SpartanF2zError::CommitmentConfigMismatch);
    }
    Ok(())
}

fn validate_relation(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
) -> Result<(), SpartanF2zError> {
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    if matrices.field_modulus_encoding() != expected {
        return Err(SpartanF2zError::UnsupportedFieldModulus);
    }
    Ok(())
}

fn validate_relation_layout(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
) -> Result<(), SpartanF2zError> {
    if matrices.matrices().row_count() != layout.multiplications()
        || matrices.matrices().column_count() != layout.assignment_len()
    {
        return Err(SpartanF2zError::RelationWitnessLayoutMismatch);
    }
    Ok(())
}

fn validate_claim_field(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
) -> Result<(), SpartanF2zError> {
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
        return Err(SpartanF2zError::ClaimFieldMismatch);
    }
    Ok(())
}

fn fill_slot_weights(
    row_weights_q: &mut [u128],
    slot_start: usize,
    bit_count: usize,
    block_factor: Fq,
    eq_high: &[Fq],
    high_gate_vars: usize,
) -> Result<(), SpartanF2zError> {
    let high_gate_count = checked_pow2(high_gate_vars)?;
    if eq_high.len() != high_gate_count {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }

    let mut bit_weight = Fq(1);
    for bit in 0..bit_count {
        let slot = slot_start
            .checked_add(bit)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        let row_base = slot
            .checked_mul(high_gate_count)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        for (gate_high, equality_weight) in eq_high.iter().copied().enumerate() {
            let row = row_base
                .checked_add(gate_high)
                .ok_or(SpartanF2zError::InvalidF2zParameters)?;
            let Some(output) = row_weights_q.get_mut(row) else {
                return Err(SpartanF2zError::InvalidF2zParameters);
            };
            *output = (block_factor * bit_weight * equality_weight).0;
        }
        bit_weight = bit_weight + bit_weight;
    }
    Ok(())
}

fn assignment_binding(
    layout: &U32MulLayout,
    commitment: &Commitment,
) -> Result<[u8; 32], SpartanF2zError> {
    let p = layout.f2z_params();
    let params = &commitment.params;
    let mut hasher = Hasher::new();
    hasher.update(ASSIGNMENT_BINDING_DOMAIN);
    hasher.update(&commitment.root);
    hash_usize(&mut hasher, params.m)?;
    hash_usize(&mut hasher, params.log_inv_rate)?;
    hash_usize(&mut hasher, params.log_batch_size)?;
    hasher.update(&[profile_code(params.profile)]);
    hasher.update(&[hash_code(params.merkle_hash)]);
    hasher.update(&FQ_MOD.to_le_bytes());
    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, p.t)?;
    hash_usize(&mut hasher, p.s)?;
    hash_usize(&mut hasher, p.word_bits)?;
    Ok(*hasher.finalize().as_bytes())
}

fn absorb_opening_claim<T: Transcript>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    assignment_binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &F2zOpeningClaim,
) -> Result<(), SpartanF2zError> {
    let digest = opening_claim_digest(matrices, assignment_binding, terminal_claim, opening)?;
    absorb_spartan_message(transcript, OPENING_CLAIM_DOMAIN, &digest);
    Ok(())
}

fn opening_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    assignment_binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &F2zOpeningClaim,
) -> Result<[u8; 32], SpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(OPENING_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hasher.update(matrices.digest());

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

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), SpartanF2zError> {
    let value = u64::try_from(value).map_err(|_| SpartanF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

pub(crate) const fn profile_code(profile: LigeritoProfile) -> u8 {
    match profile {
        LigeritoProfile::Fast => 0,
        LigeritoProfile::Slim => 1,
        LigeritoProfile::Secure => 2,
    }
}

pub(crate) const fn hash_code(hash: HashKind) -> u8 {
    match hash {
        HashKind::Sha256 => 0,
        HashKind::Blake3 => 1,
    }
}

pub(crate) fn packed_variables(p: &crate::pcs::IntEvalParams) -> Result<usize, SpartanF2zError> {
    if !p.word_bits.is_power_of_two() || p.word_bits > u128::BITS as usize {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let row_bit_vars =
        p.t.checked_add(p.word_bits.trailing_zeros() as usize)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    let expected = row_bit_vars
        .checked_sub(LOG_PACKING)
        .and_then(|folded| folded.checked_add(p.s))
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if packed_vars(p) != expected {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    Ok(expected)
}

pub(crate) fn checked_pow2(exponent: usize) -> Result<usize, SpartanF2zError> {
    let exponent = u32::try_from(exponent).map_err(|_| SpartanF2zError::InvalidF2zParameters)?;
    1_usize
        .checked_shl(exponent)
        .ok_or(SpartanF2zError::InvalidF2zParameters)
}

pub(crate) fn f2z_generator() -> BinaryFieldGF128 {
    static GENERATOR: OnceLock<BinaryFieldGF128> = OnceLock::new();
    *GENERATOR.get_or_init(crate::pcs::smallest_generator)
}

#[cfg(test)]
mod tests {
    use crypto_primitives::FromWithConfig;

    use super::*;
    use crate::piop::spartan::u32_mul::{U32MulWitness, prepare_u32_mul_relation};
    use crate::transcript::Blake3Transcript;

    fn terminal_claim(
        point: &[Fq],
        scale: Fq,
        value: Fq,
    ) -> ScaledMleEvaluationClaim<SpartanF2zField> {
        let config = spartan_f2z_field_config();
        let point = point
            .iter()
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate.0, &config))
            .collect::<Vec<_>>();
        ScaledMleEvaluationClaim::new(
            point.into_boxed_slice(),
            SpartanF2zField::from_with_cfg(scale.0, &config),
            SpartanF2zField::from_with_cfg(value.0, &config),
        )
    }

    #[test]
    fn bitification_is_the_adjoint_of_integer_reconstruction() {
        let witness =
            U32MulWitness::from_inputs(&[(0, u32::MAX), (1, 7), (u32::MAX, u32::MAX)]).unwrap();
        let layout = witness.layout();
        let p = layout.f2z_params();

        // Non-Boolean selector coordinates exercise all four assignment
        // blocks, rather than reducing this to a single block lookup.
        let gate_point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        assert_eq!(gate_point.len(), layout.gate_vars());
        let block_low = Fq(7);
        let block_high = Fq(11);
        let scale = Fq(13);
        let one = Fq(1);
        let one_minus_low = Fq(fq_sub(one.0, block_low.0));
        let one_minus_high = Fq(fq_sub(one.0, block_high.0));
        let factors = [
            one_minus_low * one_minus_high,
            block_low * one_minus_high,
            one_minus_low * block_high,
            block_low * block_high,
        ];
        let eq_gate = eq_le_table_fq(&gate_point);

        let assignment = witness.assignment();
        let mut assignment_evaluation = Fq(0);
        for block in 0..4 {
            for gate in 0..layout.capacity() {
                assignment_evaluation = assignment_evaluation
                    + factors[block]
                        * eq_gate[gate]
                        * Fq::from(u128::from(assignment[block * layout.capacity() + gate]));
            }
        }
        let value = scale * assignment_evaluation;

        let mut point = gate_point.to_vec();
        point.extend([block_low, block_high]);
        let terminal = terminal_claim(&point, scale, value);
        let opening = bitify_u32_mul_spartan_claim(&terminal, layout).unwrap();

        let rows = witness.f2z_bit_rows();
        let mut read_off = Fq(0);
        for b in 0..p.rows() {
            for c in 0..p.cols() {
                let bit = (rows[c][b / u64::BITS as usize] >> (b % u64::BITS as usize)) & 1;
                read_off = read_off
                    + Fq::from(u128::from(bit))
                        * Fq(opening.row_weights_q()[b])
                        * opening.col_weights()[c];
            }
        }
        assert_eq!(read_off, opening.claimed());
    }

    #[test]
    fn zero_scale_keeps_a_nonzero_row_functional() {
        let layout = U32MulLayout::new(3).unwrap();
        let mut point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        point.extend([Fq(7), Fq(11)]);
        let opening =
            bitify_u32_mul_spartan_claim(&terminal_claim(&point, Fq(0), Fq(0)), &layout).unwrap();

        assert!(opening.row_weights_q().iter().any(|&weight| weight != 0));
        assert!(opening.col_weights().iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed(), Fq(0));
    }

    #[test]
    fn constant_only_claim_uses_the_deterministic_dummy_functional() {
        let layout = U32MulLayout::new(3).unwrap();
        let gate_point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        let scale = Fq(13);
        let constant_evaluation = eq_le_table_fq(&gate_point)[0];
        let mut point = gate_point.to_vec();
        point.extend([Fq(0), Fq(0)]);

        let opening = bitify_u32_mul_spartan_claim(
            &terminal_claim(&point, scale, scale * constant_evaluation),
            &layout,
        )
        .unwrap();
        assert_eq!(opening.row_weights_q()[0], 1);
        assert!(
            opening.row_weights_q()[1..]
                .iter()
                .all(|&weight| weight == 0)
        );
        assert!(opening.col_weights().iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed(), Fq(0));
    }

    #[test]
    fn malformed_claim_residue_is_rejected_before_canonical_projection() {
        let layout = U32MulLayout::new(3).unwrap();
        let config = spartan_f2z_field_config();
        let malformed = SpartanF2zField::new_unchecked(Uint::from(u128::MAX), &config);
        let zero = SpartanF2zField::from_with_cfg(0_u128, &config);
        let mut point = vec![zero.clone(); layout.gate_vars() + 2];
        point[0] = malformed;
        let claim = ScaledMleEvaluationClaim::new(point.into_boxed_slice(), zero.clone(), zero);

        assert!(matches!(
            bitify_u32_mul_spartan_claim(&claim, &layout),
            Err(SpartanF2zError::ClaimFieldMismatch)
        ));
    }

    #[test]
    fn combined_protocol_uses_only_validator_gated_production_profiles() {
        let small = U32MulLayout::new(3).unwrap();
        assert!(matches!(
            configs_for_layout(&small),
            Err(SpartanF2zError::UnauditedF2zParameters)
        ));

        let production = U32MulLayout::new(1 << MIN_PRODUCTION_GATE_VARS).unwrap();
        configs_for_layout(&production).expect("the smallest embedded profile is available");
    }

    #[test]
    #[ignore = "runs three production-sized Spartan/F2Z proofs"]
    fn combined_strategies_are_byte_exact_and_all_verify() {
        let witness =
            U32MulWitness::from_fn(1 << MIN_PRODUCTION_GATE_VARS, |index| match index & 3 {
                0 => (0, u32::MAX),
                1 => (u32::MAX, u32::MAX),
                _ => {
                    let value = (index as u32).wrapping_mul(0x9E37_79B9);
                    (value, value.rotate_left(13) ^ 0xA5A5_5A5A)
                }
            })
            .unwrap();
        let layout = *witness.layout();
        let field_config = spartan_f2z_field_config();
        let matrices = prepare_u32_mul_relation::<SpartanF2zField>(layout, &field_config).unwrap();
        let hint = commit_u32_mul_witness(&layout, witness.f2z_bit_rows()).unwrap();
        let mut reference: Option<(SpartanPiopProof<SpartanF2zField>, Vec<u8>, u128)> = None;

        for strategy in [
            SpartanReductionStrategy::Immediate,
            SpartanReductionStrategy::DelayedBarrett,
            SpartanReductionStrategy::DelayedCryptoBigint,
        ] {
            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_u32_mul_spartan_and_f2z_with_strategy(
                &mut prover_transcript,
                &matrices,
                &layout,
                &witness,
                &hint,
                strategy,
            )
            .unwrap();
            let continuation = prover_transcript.get_challenge::<u128>();

            let mut verifier_transcript = Blake3Transcript::new();
            verify_u32_mul_spartan_and_f2z(
                &mut verifier_transcript,
                &matrices,
                &layout,
                &hint.commitment,
                &proof,
            )
            .unwrap();

            let f2z_bytes = proof.f2z.to_bytes();
            if let Some((reference_spartan, reference_f2z, reference_continuation)) = &reference {
                assert_eq!(&proof.spartan, reference_spartan);
                assert_eq!(&f2z_bytes, reference_f2z);
                assert_eq!(&continuation, reference_continuation);
            } else {
                reference = Some((proof.spartan, f2z_bytes, continuation));
            }
        }
    }
}
