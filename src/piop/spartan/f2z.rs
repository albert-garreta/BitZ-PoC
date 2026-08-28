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
    ext_proj::ProjArith,
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, commit_rs_ligerito_rows,
        prove_mle_eval_mod_q_ligerito_prepared_u32_v2, sha_lig_configs,
        verify_mle_eval_mod_q_ligerito_prepared_u32_v2,
    },
    pcs::{FQ_BITS, FQ_MOD, Fq, ProjectCanonicalU128, fq_sub},
    poly::{mle::DenseMultilinearExtension, univariate::binary_gf128::BinaryFieldGF128},
    transcript::traits::{GenTranscribable, Transcript},
};

use crate::utils::cfg_iter_mut;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    PreparedConstraintMatrices, R1csProductMles, SpartanField,
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

/// Domain of the compact, factorized claim bound between Spartan and F2Z.
const BITIFIED_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-f2z/bitified-claim/v3";

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

/// Compact result of applying the public u32-multiplication bitification map.
///
/// The row and column functionals remain factorized here.  They are compiled
/// directly into the mod-q chunk representation at the F2Z boundary, so the
/// deterministic Bitify reduction does not allocate either dense weight
/// vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U32BitifiedClaim {
    params: crate::pcs::IntEvalParams,
    gate_point: Box<[Fq]>,
    rows: U32BitifiedRows,
    col_scale: Fq,
    claimed: Fq,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum U32BitifiedRows {
    Structured { x: Fq, y: Fq, product: Fq },
    ConstantDummy,
}

impl U32BitifiedClaim {
    /// Public F2Z geometry selected by the statement-bound u32 layout.
    pub const fn params(&self) -> crate::pcs::IntEvalParams {
        self.params
    }

    /// Claimed value after subtracting the public constant-block term.
    pub const fn claimed(&self) -> Fq {
        self.claimed
    }

    /// Configured logical word width.
    pub const fn word_bits(&self) -> usize {
        self.params.word_bits
    }
}

struct PreparedU32BitifiedClaim {
    chunks: crate::pcs::ModQWeightChunks,
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
/// places the low gate coordinates on the clear column axis. The high gate
/// coordinates and logical word slots form the folded row axis; for `W = 8`,
/// the remaining three bit coordinates live inside each logical cell.
pub fn bitify_u32_mul_spartan_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &U32MulLayout,
) -> Result<U32BitifiedClaim, SpartanF2zError> {
    validate_layout_geometry(layout)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(2) {
        return Err(SpartanF2zError::InvalidClaimPoint);
    }

    let p = layout.f2z_params();
    let expected_modulus = Uint::from(FQ_MOD);
    let project = |value: &SpartanF2zField| -> Result<Fq, SpartanF2zError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(SpartanF2zError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= FQ_MOD {
            return Err(SpartanF2zError::ClaimFieldMismatch);
        }
        Ok(Fq(canonical))
    };

    // Project each terminal-claim element once during claim construction. The
    // previous path first validated every value through an allocating modulus
    // encoding and then retrieved every canonical residue a second time.
    let gate_point = claim.point()[..gate_vars]
        .iter()
        .map(|value| project(value))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let block_low = project(&claim.point()[gate_vars])?;
    let block_high = project(&claim.point()[gate_vars + 1])?;
    let one = Fq(1);
    let one_minus_low = Fq(fq_sub(one.0, block_low.0));
    let one_minus_high = Fq(fq_sub(one.0, block_high.0));
    let arith = f2z_fq_arith();
    let mul = |left: Fq, right: Fq| Fq(arith.mul(left.0, right.0));

    // Block order in the integer assignment is 00=constant, 01=x, 10=y,
    // 11=product, with the first block-selector coordinate as the low bit.
    let constant_factor = mul(one_minus_low, one_minus_high);
    let x_factor = mul(block_low, one_minus_high);
    let y_factor = mul(one_minus_low, block_high);
    let product_factor = mul(block_low, block_high);

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point.iter().copied().fold(constant_factor, |acc, coordinate| {
        mul(acc, Fq(fq_sub(one.0, coordinate.0)))
    });
    let adjusted_claim = Fq(fq_sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

    // Put a nonzero Spartan scale on the folded row side, avoiding a dense
    // column-table scaling pass. A zero scale remains on the clear side so it
    // does not erase the row functional that exponent folding certifies.
    let (rows, col_scale) = if x_factor == Fq(0) && y_factor == Fq(0) && product_factor == Fq(0) {
        // At block point 00 the variable part is identically zero.  The F2Z
        // prover still needs a nonempty row functional, so use a deterministic
        // dummy row with an all-zero clear read-off.
        if adjusted_claim != Fq(0) {
            return Err(SpartanF2zError::InvalidConstantOnlyClaim);
        }
        (U32BitifiedRows::ConstantDummy, Fq(0))
    } else if scale == Fq(0) {
        // Keep a nonzero row functional for the exponent-fold protocol while
        // making the clear read-off identically zero.
        (
            U32BitifiedRows::Structured {
                x: x_factor,
                y: y_factor,
                product: product_factor,
            },
            Fq(0),
        )
    } else {
        // Move the nonzero Spartan scale to the three row factors. This turns
        // the clear-column side into the raw equality table and removes one
        // field multiplication for every one of its 2^s entries.
        let (x, y, product) = if scale == one {
            (x_factor, y_factor, product_factor)
        } else {
            (
                mul(scale, x_factor),
                mul(scale, y_factor),
                mul(scale, product_factor),
            )
        };
        (
            U32BitifiedRows::Structured { x, y, product },
            one,
        )
    };

    Ok(U32BitifiedClaim {
        params: p,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

/// Proves the integer R1CS relation and succinctly opens the derived Spartan
/// assignment claim against the compact bit commitment.
///
/// This compatibility entry point accepts already-projected field tables and
/// therefore cannot retain the native `u64` witness through round zero. It
/// still uses delayed Barrett for field-valued coefficient accumulations;
/// field-MLE folding remains immediate. Prefer
/// [`prove_u32_mul_spartan_and_f2z_from_witness`] for the complete
/// native-witness production policy. This entry point requires at least
/// `2^15` multiplication slots.
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
            SpartanReductionStrategy::DelayedBarrett,
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

/// Proves the complete u32 multiplication workflow using the measured
/// production policy: delayed native coefficients, delayed native witness
/// folding, delayed field coefficients in every later round, and immediate
/// field-MLE folding. This entry point requires at least `2^15`
/// multiplication slots.
pub fn prove_u32_mul_spartan_and_f2z_from_witness<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    layout: &U32MulLayout,
    witness: &U32MulWitness,
    hint: &FlockCommitHint,
) -> Result<U32MulSpartanF2zProof, SpartanF2zError> {
    prove_u32_mul_spartan_and_f2z_with_strategy(
        transcript,
        matrices,
        layout,
        witness,
        hint,
        SpartanReductionStrategy::DelayedBarrett,
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
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_prover");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout)?;
        let bridge_digest = bitified_claim_digest(
            matrices,
            assignment_binding,
            layout,
            &terminal_claim,
            &opening,
        )?;
        (opening, bridge_digest)
    };

    let f2z = {
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prove");
        let chunks = {
            let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prepare_prover");
            prepare_u32_bitified_chunks(&opening)?
        };
        prove_mle_eval_mod_q_ligerito_prepared_u32_v2(
            transcript,
            hint,
            p,
            &chunks,
            &bridge_digest,
            FQ_BITS,
            f2z_generator(),
            pc,
        )
        .map_err(SpartanF2zError::F2z)?
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

    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_verifier");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout)?;
        let bridge_digest = bitified_claim_digest(
            matrices,
            &assignment_binding,
            layout,
            &terminal_claim,
            &opening,
        )?;
        (opening, bridge_digest)
    };

    let result = {
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_verify");
        let prepared = {
            let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prepare_verifier");
            prepare_u32_bitified_claim(&opening)?
        };
        verify_mle_eval_mod_q_ligerito_prepared_u32_v2(
            transcript,
            commitment,
            &proof.f2z,
            &p,
            &prepared.chunks,
            &prepared.col_weights,
            &bridge_digest,
            f2z_generator(),
            prepared.claimed,
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
    if !matches!(p.word_bits, 1 | 8)
        || p.t < LOG_PACKING
        || p.s > layout.gate_vars()
        || p.t.saturating_add(p.word_bits) > 126
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let total_vars = p
        .t
        .checked_add(p.word_bits.trailing_zeros() as usize)
        .and_then(|value| value.checked_add(p.s))
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
    let committed_bits = row_count
        .checked_mul(col_count)
        .and_then(|cells| cells.checked_mul(p.word_bits))
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    let expected_bits = U32_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if committed_bits != expected_bits || packed_variables(&p)? != layout.gate_vars() {
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
    let row_bits = row_count
        .checked_mul(p.word_bits)
        .ok_or(SpartanF2zError::InvalidBitRows)?;
    if row_bits % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    let words_per_col = row_bits / u64::BITS as usize;
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
        || pc.recursive_steps != vc.recursive_steps
        || pc.initial_log_msg_cols != vc.initial_log_msg_cols
        || pc.initial_log_num_interleaved != vc.initial_log_num_interleaved
        || pc.recursive_log_msg_cols != vc.recursive_log_msg_cols
        || pc.recursive_ks != vc.recursive_ks
        || pc.queries != vc.queries
        || pc.grinding_bits != vc.grinding_bits
        || pc.fold_grinding_bits != vc.fold_grinding_bits
        || pc.ood_samples != vc.ood_samples
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

fn prepare_u32_bitified_claim(
    opening: &U32BitifiedClaim,
) -> Result<PreparedU32BitifiedClaim, SpartanF2zError> {
    let chunks = prepare_u32_bitified_chunks(opening)?;
    let p = opening.params;
    let col_weights = if opening.col_scale == Fq(0) {
        vec![Fq(0); checked_pow2(p.s)?]
    } else {
        let (gate_low, _) = opening.gate_point.split_at(p.s);
        let mut eq_low = eq_le_table_fq_fast(gate_low)?;
        if opening.col_scale != Fq(1) {
            let arith = f2z_fq_arith();
            let factor = arith.monty_factor(opening.col_scale.0);
            cfg_iter_mut!(&mut eq_low, 256).for_each(|weight| {
                weight.0 = arith.mul_plain_by(weight.0, &factor);
            });
        }
        eq_low
    };

    Ok(PreparedU32BitifiedClaim {
        chunks,
        col_weights,
        claimed: opening.claimed,
    })
}

/// Compile only the folded row functional. The prover never reads the clear
/// column weights or the claimed value, so keeping those verifier-only avoids
/// an entire `2^s` equality table on the proving path.
fn prepare_u32_bitified_chunks(
    opening: &U32BitifiedClaim,
) -> Result<crate::pcs::ModQWeightChunks, SpartanF2zError> {
    let p = opening.params;
    let high_vars = p
        .t
        .checked_add(p.word_bits.trailing_zeros() as usize)
        .and_then(|variables| variables.checked_sub(7))
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    let gate_vars = p
        .s
        .checked_add(high_vars)
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if opening.gate_point.len() != gate_vars {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let (_, gate_high) = opening.gate_point.split_at(p.s);

    match opening.rows {
        U32BitifiedRows::ConstantDummy => {
            let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&p, FQ_BITS)
                .map_err(|_| SpartanF2zError::InvalidF2zParameters)?;
            chunks
                .set_weight_range(0, &[1])
                .map_err(|_| SpartanF2zError::InvalidF2zParameters)?;
            Ok(chunks)
        }
        U32BitifiedRows::Structured { x, y, product } => {
            let high_gate_count = checked_pow2(gate_high.len())?;
            let row_count = checked_pow2(p.t)?;
            let blocks = [
                (U32_MUL_X_SLOT_START, U32_MUL_X_BITS, x),
                (U32_MUL_Y_SLOT_START, U32_MUL_Y_BITS, y),
                (U32_MUL_PRODUCT_SLOT_START, U32_MUL_PRODUCT_BITS, product),
            ];
            let mut scratch = vec![0_u128; high_gate_count];

            if crate::pcs::mod_q_num_chunks(&p, FQ_BITS) == 1 {
                let mut weights = Vec::with_capacity(row_count);
                for (slot_start, bit_count, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        bit_count,
                        p.word_bits,
                        block_factor,
                        gate_high,
                        |row_start, range| {
                            if row_start != weights.len() {
                                return Err(SpartanF2zError::InvalidF2zParameters);
                            }
                            weights.extend_from_slice(range);
                            Ok(())
                        },
                    )?;
                }
                crate::pcs::ModQWeightChunks::from_single_chunk(&p, FQ_BITS, weights)
                    .map_err(|_| SpartanF2zError::InvalidF2zParameters)
            } else {
                let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&p, FQ_BITS)
                    .map_err(|_| SpartanF2zError::InvalidF2zParameters)?;
                for (slot_start, bit_count, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        bit_count,
                        p.word_bits,
                        block_factor,
                        gate_high,
                        |row_start, range| {
                            chunks
                                .set_weight_range(row_start, range)
                                .map_err(|_| SpartanF2zError::InvalidF2zParameters)
                        },
                    )?;
                }
                Ok(chunks)
            }
        }
    }
}

fn f2z_fq_arith() -> &'static ProjArith {
    static ARITH: OnceLock<ProjArith> = OnceLock::new();
    ARITH.get_or_init(|| ProjArith::new(FQ_MOD))
}

/// Little-endian equality table with one fixed-factor Montgomery
/// multiplication per parent and one allocation for the complete table.
fn eq_le_table_fq_fast(point: &[Fq]) -> Result<Vec<Fq>, SpartanF2zError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);
    let arith = f2z_fq_arith();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        let factor = arith.monty_factor(coordinate.0);
        let (zero_children, one_children) = table[..active_len].split_at_mut(half);
        let expand = |zero: &mut Fq, one: &mut Fq| {
            let parent = zero.0;
            let one_child = arith.mul_plain_by(parent, &factor);
            zero.0 = fq_sub(parent, one_child);
            one.0 = one_child;
        };
        if half < 256 {
            zero_children.iter_mut().zip(one_children.iter_mut()).for_each(
                |(zero, one)| expand(zero, one),
            );
        } else {
            cfg_iter_mut!(zero_children, 256)
                .zip(cfg_iter_mut!(one_children, 256))
                .for_each(|(zero, one)| expand(zero, one));
        }
        half = active_len;
    }
    Ok(table)
}

/// Write `scale * eq(., point)` into a reusable canonical-u128 buffer. Seeding
/// the recurrence with the block factor avoids first building an unscaled
/// table and then multiplying all `2^h` entries once per block.
fn scaled_eq_le_table_fq_into(
    point: &[Fq],
    scale: Fq,
    table: &mut [u128],
) -> Result<(), SpartanF2zError> {
    if table.len() != checked_pow2(point.len())? {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    table[0] = scale.0;
    let arith = f2z_fq_arith();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        let factor = arith.monty_factor(coordinate.0);
        let (zero_children, one_children) = table[..active_len].split_at_mut(half);
        let expand = |zero: &mut u128, one: &mut u128| {
            let parent = *zero;
            let one_child = arith.mul_plain_by(parent, &factor);
            *zero = fq_sub(parent, one_child);
            *one = one_child;
        };
        if half < 256 {
            zero_children
                .iter_mut()
                .zip(one_children.iter_mut())
                .for_each(|(zero, one)| expand(zero, one));
        } else {
            cfg_iter_mut!(zero_children, 256)
                .zip(cfg_iter_mut!(one_children, 256))
                .for_each(|(zero, one)| expand(zero, one));
        }
        half = active_len;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fill_block_weight_ranges(
    scratch: &mut [u128],
    bit_slot_start: usize,
    bit_count: usize,
    word_bits: usize,
    block_factor: Fq,
    gate_high: &[Fq],
    mut write_range: impl FnMut(usize, &[u128]) -> Result<(), SpartanF2zError>,
) -> Result<(), SpartanF2zError> {
    if !matches!(word_bits, 1 | 8)
        || bit_slot_start % word_bits != 0
        || bit_count % word_bits != 0
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let word_slot_start = bit_slot_start / word_bits;
    let word_count = bit_count / word_bits;
    let high_gate_count = checked_pow2(gate_high.len())?;
    if scratch.len() != high_gate_count {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }

    // Seed the equality recurrence with this block's factor, then sweep
    // word-major contiguous row ranges. The old gate-major loop jumped between
    // 32--64 distant blocks for every gate and defeated the cache.
    scaled_eq_le_table_fq_into(gate_high, block_factor, scratch)?;

    for word in 0..word_count {
        let word_slot = word_slot_start
            .checked_add(word)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        let row_start = word_slot
            .checked_mul(high_gate_count)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
        write_range(row_start, scratch)?;

        if word + 1 != word_count {
            cfg_iter_mut!(scratch, 256)
                .for_each(|weight| *weight = fq_mul_pow2_small(*weight, word_bits));
        }
    }
    Ok(())
}

/// Multiply a canonical `Fq` value by `2^bits` for `bits <= 8`, using
/// `2^100 = 15 (mod q)`. One pseudo-Mersenne fold suffices because the input
/// is below `q` and the shifted value is below `2^108`.
#[inline]
fn fq_mul_pow2_small(value: u128, bits: usize) -> u128 {
    debug_assert!(value < FQ_MOD);
    debug_assert!(bits <= 8);
    let shifted = value.wrapping_shl(bits as u32);
    let low_mask = (1_u128 << FQ_BITS).wrapping_sub(1);
    let low = shifted & low_mask;
    let high = shifted >> FQ_BITS;
    let folded = low.wrapping_add(high.wrapping_mul(15));
    if folded >= FQ_MOD {
        folded.wrapping_sub(FQ_MOD)
    } else {
        folded
    }
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

fn bitified_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    assignment_binding: &[u8; 32],
    layout: &U32MulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &U32BitifiedClaim,
) -> Result<[u8; 32], SpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(BITIFIED_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hash_usize(&mut hasher, matrices.field_modulus_encoding().len())?;
    hasher.update(matrices.field_modulus_encoding());
    hasher.update(matrices.digest());
    hasher.update(&FQ_MOD.to_le_bytes());

    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, opening.params.t)?;
    hash_usize(&mut hasher, opening.params.s)?;
    hash_usize(&mut hasher, opening.params.word_bits)?;
    hash_usize(&mut hasher, U32_MUL_X_SLOT_START)?;
    hash_usize(&mut hasher, U32_MUL_X_BITS)?;
    hash_usize(&mut hasher, U32_MUL_Y_SLOT_START)?;
    hash_usize(&mut hasher, U32_MUL_Y_BITS)?;
    hash_usize(&mut hasher, U32_MUL_PRODUCT_SLOT_START)?;
    hash_usize(&mut hasher, U32_MUL_PRODUCT_BITS)?;
    // Mapping version 2: little-endian bits, 00/01/10/11 block order, and
    // canonical nonzero-scale normalization onto the folded row functional.
    hasher.update(&[2, 0, 0, 1, 2, 3]);

    hash_usize(&mut hasher, terminal_claim.point().len())?;
    for coordinate in terminal_claim.point() {
        hash_spartan_f2z_element(&mut hasher, coordinate);
    }
    hash_spartan_f2z_element(&mut hasher, terminal_claim.scale());
    hash_spartan_f2z_element(&mut hasher, terminal_claim.value());

    let (gate_low, gate_high) = opening.gate_point.split_at(opening.params.s);
    hash_usize(&mut hasher, gate_low.len())?;
    for coordinate in gate_low {
        hasher.update(&coordinate.0.to_le_bytes());
    }
    hash_usize(&mut hasher, gate_high.len())?;
    for coordinate in gate_high {
        hasher.update(&coordinate.0.to_le_bytes());
    }
    match opening.rows {
        U32BitifiedRows::Structured { x, y, product } => {
            hasher.update(&[0]);
            hasher.update(&x.0.to_le_bytes());
            hasher.update(&y.0.to_le_bytes());
            hasher.update(&product.0.to_le_bytes());
        }
        U32BitifiedRows::ConstantDummy => {
            hasher.update(&[1]);
        }
    }
    hasher.update(&opening.col_scale.0.to_le_bytes());
    hasher.update(&opening.claimed.0.to_le_bytes());
    Ok(*hasher.finalize().as_bytes())
}

#[inline]
fn hash_spartan_f2z_element(hasher: &mut Hasher, value: &SpartanF2zField) {
    let canonical = value.retrieve();
    let mut encoding = [0_u8; 16];
    canonical.write_transcription_bytes_exact(&mut encoding);
    hasher.update(&encoding);
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
    use crate::pcs::{eq_le_table_fq, fq_add};
    use crate::piop::spartan::u32_mul::{
        U32MulF2zWidth, U32MulWitness, prepare_u32_mul_relation,
    };
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

    fn prepared_row_weight(prepared: &PreparedU32BitifiedClaim, row: usize) -> u128 {
        let mut value = 0_u128;
        let mut shift = 0_usize;
        for chunk in prepared.chunks.chunks() {
            value |= chunk[row] << shift;
            shift += prepared.chunks.chunk_width();
        }
        value
    }

    #[test]
    fn fast_bitify_field_helpers_match_reference_arithmetic() {
        let point = [Fq(0), Fq(1), Fq(FQ_MOD - 1), Fq(123_456_789)];
        assert_eq!(eq_le_table_fq_fast(&point).unwrap(), eq_le_table_fq(&point));

        for value in [0, 1, 2, FQ_MOD / 2, FQ_MOD - 2, FQ_MOD - 1] {
            for bits in [1, 8] {
                let mut expected = value;
                for _ in 0..bits {
                    expected = fq_add(expected, expected);
                }
                assert_eq!(fq_mul_pow2_small(value, bits), expected);
            }
        }
    }

    #[test]
    fn bitification_is_the_adjoint_of_integer_reconstruction() {
        for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
            let witness = U32MulWitness::from_inputs_with_f2z_width(
                &[(0, u32::MAX), (1, 7), (u32::MAX, u32::MAX)],
                width,
            )
            .unwrap();
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
            let prepared = prepare_u32_bitified_claim(&opening).unwrap();

            let rows = witness.f2z_bit_rows();
            let mut read_off = Fq(0);
            for b in 0..p.rows() {
                for c in 0..p.cols() {
                    let mut cell = 0_u128;
                    for j in 0..p.word_bits {
                        let packed_bit = b * p.word_bits + j;
                        let bit = (rows[c][packed_bit / u64::BITS as usize]
                            >> (packed_bit % u64::BITS as usize))
                            & 1;
                        cell |= u128::from(bit) << j;
                    }
                    read_off = read_off
                        + Fq::from(cell)
                            * Fq(prepared_row_weight(&prepared, b))
                            * prepared.col_weights[c];
                }
            }
            assert_eq!(read_off, opening.claimed());
        }
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
        let prepared = prepare_u32_bitified_claim(&opening).unwrap();

        assert!((0..opening.params.rows()).any(|row| prepared_row_weight(&prepared, row) != 0));
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
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
        let prepared = prepare_u32_bitified_claim(&opening).unwrap();
        assert_eq!(prepared_row_weight(&prepared, 0), 1);
        assert!((1..opening.params.rows()).all(|row| prepared_row_weight(&prepared, row) == 0));
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
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

        for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
            let production = U32MulLayout::new_with_f2z_width(
                1 << MIN_PRODUCTION_GATE_VARS,
                width,
            )
            .unwrap();
            configs_for_layout(&production).expect("the smallest embedded profile is available");
        }
    }

    #[test]
    #[ignore = "runs one production-sized W=8 Spartan/F2Z proof"]
    fn combined_w8_proof_verifies() {
        let witness = U32MulWitness::from_fn_with_f2z_width(
            1 << MIN_PRODUCTION_GATE_VARS,
            U32MulF2zWidth::W8,
            |index| {
                let value = (index as u32).wrapping_mul(0x9E37_79B9);
                (value, value.rotate_left(13) ^ 0xA5A5_5A5A)
            },
        )
        .unwrap();
        let layout = *witness.layout();
        let field_config = spartan_f2z_field_config();
        let matrices = prepare_u32_mul_relation::<SpartanF2zField>(layout, &field_config).unwrap();
        let hint = commit_u32_mul_witness(&layout, witness.f2z_bit_rows()).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_u32_mul_spartan_and_f2z_from_witness(
            &mut prover_transcript,
            &matrices,
            &layout,
            &witness,
            &hint,
        )
        .unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u32_mul_spartan_and_f2z(
            &mut verifier_transcript,
            &matrices,
            &layout,
            &hint.commitment,
            &proof,
        )
        .unwrap();
    }

    #[test]
    #[ignore = "runs five production-sized Spartan/F2Z proofs"]
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
        let verify_and_fingerprint =
            |proof: U32MulSpartanF2zProof,
             continuation: u128|
             -> (SpartanPiopProof<SpartanF2zField>, Vec<u8>, u128) {
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
                (proof.spartan, f2z_bytes, continuation)
            };

        let mut production_transcript = Blake3Transcript::new();
        let production_proof = prove_u32_mul_spartan_and_f2z_from_witness(
            &mut production_transcript,
            &matrices,
            &layout,
            &witness,
            &hint,
        )
        .unwrap();
        let production_continuation = production_transcript.get_challenge::<u128>();
        let reference = verify_and_fingerprint(production_proof, production_continuation);

        let (assignment, products) =
            project_u32_mul_witness::<SpartanF2zField>(&witness, &field_config).unwrap();
        let mut compatibility_transcript = Blake3Transcript::new();
        let compatibility_proof = prove_u32_mul_spartan_and_f2z(
            &mut compatibility_transcript,
            &matrices,
            &layout,
            assignment,
            products,
            &hint,
        )
        .unwrap();
        let compatibility_continuation = compatibility_transcript.get_challenge::<u128>();
        let compatibility = verify_and_fingerprint(compatibility_proof, compatibility_continuation);
        assert_eq!(compatibility, reference);

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
            let fingerprint = verify_and_fingerprint(proof, continuation);
            assert_eq!(fingerprint, reference);
        }
    }
}
