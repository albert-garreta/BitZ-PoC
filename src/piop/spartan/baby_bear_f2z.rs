//! Succinct opening of BabyBear multiplication assignments through F2Z.
//!
//! The integer R1CS assignment has five logical blocks,
//!
//! ```text
//! [constant block | a block | b block | c block | quotient block],
//! ```
//!
//! and is zero-padded to eight blocks for Spartan's assignment MLE.  The
//! commitment stores four 31-bit little-endian values per multiplication in
//! 128 physical slots; the final four slots are unused and the honest witness
//! builder leaves them zero.  This module applies the transpose of that public
//! reconstruction map to Spartan's terminal assignment claim and opens the
//! resulting compact-bit claim with F2Z.

use std::sync::OnceLock;

use blake3::Hasher;
use crypto_primitives::{PrimeField, crypto_bigint_uint::Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{
        LigeritoProfile, ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig,
    },
};
use thiserror::Error;

use crate::{
    ext_proj::ProjArith,
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, commit_rs_ligerito_rows,
        prove_mle_eval_mod_q_ligerito_prepared_baby_bear_v2, sha_lig_configs,
        verify_mle_eval_mod_q_ligerito_prepared_baby_bear_v2,
    },
    pcs::{FQ_BITS, FQ_MOD, Fq, ProjectCanonicalU128, fq_sub},
    poly::mle::DenseMultilinearExtension,
    transcript::traits::{GenTranscribable, Transcript},
};

use crate::utils::cfg_iter_mut;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    PreparedConstraintMatrices, R1csProductMles, SpartanField,
    baby_bear_mul::{
        BABY_BEAR_MODULUS, BABY_BEAR_MUL_A_SLOT_START, BABY_BEAR_MUL_B_SLOT_START,
        BABY_BEAR_MUL_BIT_SLOTS, BABY_BEAR_MUL_C_SLOT_START, BABY_BEAR_MUL_K_SLOT_START,
        BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS, BABY_BEAR_MUL_VALUE_BITS, BabyBearMulCoefficient,
        BabyBearMulError, BabyBearMulLayout, BabyBearMulWitness,
        project_baby_bear_mul_native_witness, project_baby_bear_mul_witness,
    },
    f2z::{
        SpartanF2zField, f2z_generator, hash_code, profile_code, spartan_f2z_field_config,
    },
    matrix::ScaledMleEvaluationClaim,
    piop::{
        SpartanError, SpartanPiopProof, SpartanReductionStrategy,
        prove_spartan_piop_native_u64_with_strategy, prove_spartan_piop_with_strategy,
        verify_spartan_proof,
    },
};

/// Domain of the BabyBear commitment-and-layout digest used as Spartan's
/// assignment oracle binding.
const ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-baby-bear-f2z/assignment/v1";

/// Domain of the compact, factorized BabyBear claim bound between Spartan and
/// F2Z. Version two replaces the dense row/column statement with its canonical
/// factorization.
const BITIFIED_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-baby-bear-f2z/bitified-claim/v2";

/// Canonical order of the five logical and three padded assignment blocks.
const ASSIGNMENT_BLOCK_ORDER: &[u8] = b"e0|a|b|c|k|zero|zero|zero";

const LOGICAL_ASSIGNMENT_BLOCKS: usize = 5;
const PADDED_ASSIGNMENT_BLOCKS: usize = 8;

/// Embedded, validator-gated Ligerito profiles begin at a 22-variable
/// committed bit MLE: seven slot variables plus fifteen gate variables.
const MIN_PRODUCTION_GATE_VARS: usize = 15;

/// Compact result of applying the public four-by-31-bit reconstruction map.
///
/// Equality tables are deliberately absent. The F2Z boundary compiles these
/// factors directly into its prepared mod-q representation, keeping Bitify
/// logarithmic in the number of multiplication rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BabyBearBitifiedClaim {
    params: crate::pcs::IntEvalParams,
    gate_point: Box<[Fq]>,
    rows: BabyBearBitifiedRows,
    col_scale: Fq,
    claimed: Fq,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BabyBearBitifiedRows {
    Structured { a: Fq, b: Fq, c: Fq, k: Fq },
    ConstantOrPaddingDummy,
}

impl BabyBearBitifiedClaim {
    /// Public F2Z geometry selected by the statement-bound layout.
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

struct PreparedBabyBearBitifiedClaim {
    chunks: crate::pcs::ModQWeightChunks,
    col_weights: Vec<Fq>,
    claimed: Fq,
}

/// The combined BabyBear multiplication proof.
///
/// There is deliberately no combined proof codec.  The two components retain
/// their native encodings so benchmark accounting cannot confuse framing with
/// proof payload.
#[derive(Clone)]
pub struct BabyBearMulSpartanF2zProof {
    /// Spartan's outer and inner sumchecks.
    pub spartan: SpartanPiopProof<SpartanF2zField>,
    /// F2Z opening of the derived compact-bit claim.
    pub f2z: IntEvalRsLigModQProof,
}

impl BabyBearMulSpartanF2zProof {
    /// Spartan proof component, exposed for benchmark payload accounting.
    pub const fn spartan(&self) -> &SpartanPiopProof<SpartanF2zField> {
        &self.spartan
    }

    /// F2Z proof component, exposed for its existing exact byte codec.
    pub const fn f2z(&self) -> &IntEvalRsLigModQProof {
        &self.f2z
    }
}

/// Failures in BabyBear layout validation, claim translation, or either proof
/// system.
#[derive(Debug, Error)]
pub enum BabyBearSpartanF2zError {
    #[error(transparent)]
    Relation(#[from] BabyBearMulError),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the prepared relation does not use q = 2^100 - 15")]
    UnsupportedFieldModulus,

    #[error("the prepared matrices do not match the BabyBear multiplication layout")]
    RelationWitnessLayoutMismatch,

    #[error("the compact bit rows do not match the BabyBear multiplication layout")]
    InvalidBitRows,

    #[error("the F2Z parameters are invalid for the compact BabyBear multiplication layout")]
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

    #[error("a constant-or-padding-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOrPaddingOnlyClaim,

    #[error("a host length does not fit the canonical transcript encoding")]
    BindingEncodingOverflow,
}

/// Commits prebuilt compact `31 + 31 + 31 + 31` BabyBear bit rows.
///
/// Accepting ownership lets benchmarks time bit packing separately and move
/// the packed store into the commitment without retaining a duplicate.  Only
/// layouts with at least `2^15` gate slots are accepted because smaller
/// embedded Ligerito configurations are explicitly test-only.
pub fn commit_baby_bear_mul_witness(
    layout: &BabyBearMulLayout,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, BabyBearSpartanF2zError> {
    let params = layout.f2z_params();
    validate_layout_geometry(layout)?;
    validate_bit_rows(&params, &rows)?;
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&params, &pc, &vc)?;

    let hint = commit_rs_ligerito_rows(&params, rows, &pc);
    validate_commitment(&params, &hint.commitment, &pc)?;
    Ok(hint)
}

/// Applies the adjoint of the public four-by-31-bit reconstruction to a
/// terminal scaled assignment-MLE claim.
///
/// Spartan's point is low-coordinate-first.  Its final three coordinates
/// select the eight padded assignment blocks; the preceding coordinates
/// select a gate.  F2Z places the low gate coordinates on the clear column
/// axis and the high gate coordinates together with the 128 bit slots on the
/// folded row axis.
pub fn bitify_baby_bear_mul_spartan_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &BabyBearMulLayout,
) -> Result<BabyBearBitifiedClaim, BabyBearSpartanF2zError> {
    validate_layout_geometry(layout)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(3) {
        return Err(BabyBearSpartanF2zError::InvalidClaimPoint);
    }

    let params = layout.f2z_params();
    let expected_modulus = Uint::from(FQ_MOD);
    let project = |value: &SpartanF2zField| -> Result<Fq, BabyBearSpartanF2zError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(BabyBearSpartanF2zError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= FQ_MOD {
            return Err(BabyBearSpartanF2zError::ClaimFieldMismatch);
        }
        Ok(Fq(canonical))
    };

    let gate_point = claim.point()[..gate_vars]
        .iter()
        .map(|value| project(value))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let block_low = project(&claim.point()[gate_vars])?;
    let block_mid = project(&claim.point()[gate_vars + 1])?;
    let block_high = project(&claim.point()[gate_vars + 2])?;

    let one = Fq(1);
    let one_minus_low = Fq(fq_sub(one.0, block_low.0));
    let one_minus_mid = Fq(fq_sub(one.0, block_mid.0));
    let one_minus_high = Fq(fq_sub(one.0, block_high.0));
    let arith = f2z_fq_arith();
    let mul = |left: Fq, right: Fq| Fq(arith.mul(left.0, right.0));
    let mul3 = |first: Fq, second: Fq, third: Fq| mul(mul(first, second), third);

    // Little-endian block-selector order is 000=e0, 001=a, 010=b,
    // 011=c, 100=k, and 101..111 are public zero padding.
    let constant_factor = mul3(one_minus_low, one_minus_mid, one_minus_high);
    let a_factor = mul3(block_low, one_minus_mid, one_minus_high);
    let b_factor = mul3(one_minus_low, block_mid, one_minus_high);
    let c_factor = mul3(block_low, block_mid, one_minus_high);
    let k_factor = mul3(one_minus_low, one_minus_mid, block_high);

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point.iter().copied().fold(constant_factor, |acc, coordinate| {
        mul(acc, Fq(fq_sub(one.0, coordinate.0)))
    });
    let adjusted_claim = Fq(fq_sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

    // Normalize nonzero scale onto the folded row factors. The verifier then
    // builds an unscaled clear-column equality table, while scale zero keeps a
    // nonzero row functional and zeros the clear read-off.
    let (rows, col_scale) = if [a_factor, b_factor, c_factor, k_factor]
        .into_iter()
        .all(|factor| factor == Fq(0))
    {
        if adjusted_claim != Fq(0) {
            return Err(BabyBearSpartanF2zError::InvalidConstantOrPaddingOnlyClaim);
        }
        (BabyBearBitifiedRows::ConstantOrPaddingDummy, Fq(0))
    } else if scale == Fq(0) {
        (
            BabyBearBitifiedRows::Structured {
                a: a_factor,
                b: b_factor,
                c: c_factor,
                k: k_factor,
            },
            Fq(0),
        )
    } else {
        let (a, b, c, k) = if scale == one {
            (a_factor, b_factor, c_factor, k_factor)
        } else {
            (
                mul(scale, a_factor),
                mul(scale, b_factor),
                mul(scale, c_factor),
                mul(scale, k_factor),
            )
        };
        (
            BabyBearBitifiedRows::Structured { a, b, c, k },
            one,
        )
    };

    // k is reconstructed raw; p is already bound as matrix C's coefficient.
    Ok(BabyBearBitifiedClaim {
        params,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

/// Proves the BabyBear integer R1CS relation and opens the resulting Spartan
/// assignment claim against the compact bit commitment.
///
/// This compatibility entry point takes already projected field tables and
/// therefore cannot retain the native `u64` witness through round zero. It
/// still uses delayed Barrett for field-valued coefficient accumulations;
/// field-MLE folding remains immediate. Prefer
/// [`prove_baby_bear_mul_spartan_and_f2z_from_witness`] for the complete
/// native-witness production policy. This entry point requires at least
/// `2^15` multiplication slots.
pub fn prove_baby_bear_mul_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    assignment: DenseMultilinearExtension<SpartanF2zField>,
    products: R1csProductMles<SpartanF2zField>,
    hint: &FlockCommitHint,
) -> Result<BabyBearMulSpartanF2zProof, BabyBearSpartanF2zError> {
    let (params, pc, assignment_binding) = prepare_combined_prover(matrices, layout, hint)?;
    let (spartan, terminal_claim) = {
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:spartan_prove");
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
        &params,
        &pc,
        &assignment_binding,
        spartan,
        terminal_claim,
    )
}

/// Proves the complete BabyBear multiplication workflow using the production
/// policy: delayed native coefficients, delayed native witness folding,
/// delayed field coefficients in every later round, and immediate field-MLE
/// folding. This entry point requires at least `2^15` multiplication slots.
pub fn prove_baby_bear_mul_spartan_and_f2z_from_witness<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    witness: &BabyBearMulWitness,
    hint: &FlockCommitHint,
) -> Result<BabyBearMulSpartanF2zProof, BabyBearSpartanF2zError> {
    prove_baby_bear_mul_spartan_and_f2z_with_strategy(
        transcript,
        matrices,
        layout,
        witness,
        hint,
        SpartanReductionStrategy::DelayedBarrett,
    )
}

/// Proves the BabyBear multiplication workflow with one reduction strategy
/// selected before entering Spartan's product loops.
pub fn prove_baby_bear_mul_spartan_and_f2z_with_strategy<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    witness: &BabyBearMulWitness,
    hint: &FlockCommitHint,
    strategy: SpartanReductionStrategy,
) -> Result<BabyBearMulSpartanF2zProof, BabyBearSpartanF2zError> {
    if witness.layout() != layout {
        return Err(BabyBearSpartanF2zError::RelationWitnessLayoutMismatch);
    }
    let (params, pc, assignment_binding) = prepare_combined_prover(matrices, layout, hint)?;

    // Projection stays outside the Spartan timing scope. Immediate mode
    // materializes field tables, while delayed modes retain exact u64 values
    // through the first outer and inner folds.
    let (spartan, terminal_claim) = match strategy {
        SpartanReductionStrategy::Immediate => {
            let (assignment, products) =
                project_baby_bear_mul_witness::<SpartanF2zField>(witness, matrices.config())?;
            let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:spartan_prove");
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
            let (assignment, products) = project_baby_bear_mul_native_witness(witness).into_parts();
            let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:spartan_prove");
            prove_spartan_piop_native_u64_with_strategy(
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
        &params,
        &pc,
        &assignment_binding,
        spartan,
        terminal_claim,
    )
}

fn prepare_combined_prover(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    hint: &FlockCommitHint,
) -> Result<(crate::pcs::IntEvalParams, LigProverConfig, [u8; 32]), BabyBearSpartanF2zError> {
    validate_relation(matrices)?;
    validate_relation_layout(matrices, layout)?;
    validate_layout_geometry(layout)?;
    let params = layout.f2z_params();
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&params, &pc, &vc)?;
    validate_bit_rows(&params, hint.rows())?;
    validate_commitment(&params, &hint.commitment, &pc)?;
    let assignment_binding = assignment_binding(layout, &hint.commitment)?;
    Ok((params, pc, assignment_binding))
}

#[allow(clippy::too_many_arguments)]
fn finish_combined_prover<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    hint: &FlockCommitHint,
    params: &crate::pcs::IntEvalParams,
    pc: &LigProverConfig,
    assignment_binding: &[u8; 32],
    spartan: SpartanPiopProof<SpartanF2zField>,
    terminal_claim: ScaledMleEvaluationClaim<SpartanF2zField>,
) -> Result<BabyBearMulSpartanF2zProof, BabyBearSpartanF2zError> {
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:bitify_prover");
        let opening = bitify_baby_bear_mul_spartan_claim(&terminal_claim, layout)?;
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
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:f2z_prove");
        let chunks = {
            let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:f2z_prepare_prover");
            prepare_baby_bear_bitified_chunks(&opening)?
        };
        prove_mle_eval_mod_q_ligerito_prepared_baby_bear_v2(
            transcript,
            hint,
            params,
            &chunks,
            &bridge_digest,
            FQ_BITS,
            f2z_generator(),
            pc,
        )
        .map_err(BabyBearSpartanF2zError::F2z)?
    };

    Ok(BabyBearMulSpartanF2zProof { spartan, f2z })
}

/// Verifies Spartan and F2Z on one transcript.
///
/// The terminal assignment claim is always derived from `proof.spartan`; it
/// is never supplied by or trusted from the prover.
///
/// `matrices` are the verifier's public R1CS statement. Callers proving the
/// BabyBear relation must construct them with
/// [`super::baby_bear_mul::prepare_baby_bear_mul_relation`]; this verifier
/// validates their field and dimensions and transcript-binds their digest, but
/// does not reinterpret arbitrary same-shaped matrices as BabyBear matrices.
pub fn verify_baby_bear_mul_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
    commitment: &Commitment,
    proof: &BabyBearMulSpartanF2zProof,
) -> Result<(), BabyBearSpartanF2zError> {
    validate_relation(matrices)?;
    validate_relation_layout(matrices, layout)?;
    validate_layout_geometry(layout)?;
    let params = layout.f2z_params();
    let (pc, vc) = configs_for_layout(layout)?;
    validate_config_pair(&params, &pc, &vc)?;
    validate_commitment(&params, commitment, &pc)?;
    validate_f2z_proof_shape(&params, &proof.f2z)?;
    let assignment_binding = assignment_binding(layout, commitment)?;

    let terminal_claim = {
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:spartan_verify");
        verify_spartan_proof(transcript, matrices, &assignment_binding, &proof.spartan)?
    };

    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:bitify_verifier");
        let opening = bitify_baby_bear_mul_spartan_claim(&terminal_claim, layout)?;
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
        let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:f2z_verify");
        let prepared = {
            let _scope = crate::utils::prof::scope("baby-bear-spartan-f2z:f2z_prepare_verifier");
            prepare_baby_bear_bitified_claim(&opening)?
        };
        verify_mle_eval_mod_q_ligerito_prepared_baby_bear_v2(
            transcript,
            commitment,
            &proof.f2z,
            &params,
            &prepared.chunks,
            &prepared.col_weights,
            &bridge_digest,
            f2z_generator(),
            prepared.claimed,
            FQ_BITS,
            &vc,
        )
    };
    result.map_err(BabyBearSpartanF2zError::F2z)
}

fn configs_for_layout(
    layout: &BabyBearMulLayout,
) -> Result<(LigProverConfig, LigVerifierConfig), BabyBearSpartanF2zError> {
    if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
        return Err(BabyBearSpartanF2zError::UnauditedF2zParameters);
    }
    let params = layout.f2z_params();
    let m_p = packed_variables(&params)?;
    sha_lig_configs(m_p).map_err(BabyBearSpartanF2zError::LigeritoConfig)
}

fn validate_layout_geometry(layout: &BabyBearMulLayout) -> Result<(), BabyBearSpartanF2zError> {
    let params = layout.f2z_params();
    if params.word_bits != 1
        || params.t < LOG_PACKING
        || params.s > layout.gate_vars()
        || params.t.saturating_add(params.word_bits) > 126
    {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    let total_vars = params
        .t
        .checked_add(params.s)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(7)
            .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?
        || BABY_BEAR_MUL_BIT_SLOTS != 1_usize << 7
        || BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS != 4 * BABY_BEAR_MUL_VALUE_BITS
        || layout.assignment_len() != LOGICAL_ASSIGNMENT_BLOCKS * layout.capacity()
        || layout.padded_assignment_len() != PADDED_ASSIGNMENT_BLOCKS * layout.capacity()
    {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(params.t)?;
    let col_count = checked_pow2(params.s)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    let expected_cells = BABY_BEAR_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    if cells != expected_cells || packed_variables(&params)? != layout.gate_vars() {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_f2z_proof_shape(
    params: &crate::pcs::IntEvalParams,
    proof: &IntEvalRsLigModQProof,
) -> Result<(), BabyBearSpartanF2zError> {
    if !params.word_bits.is_power_of_two() || params.word_bits > u128::BITS as usize {
        return Err(BabyBearSpartanF2zError::InvalidF2zProofShape);
    }
    let row_bit_vars = params
        .t
        .checked_add(params.word_bits.trailing_zeros() as usize)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zProofShape)?;
    let chunks = crate::pcs::mod_q_num_chunks(params, FQ_BITS);
    let columns = checked_pow2(params.s)?;
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
        return Err(BabyBearSpartanF2zError::InvalidF2zProofShape);
    }
    Ok(())
}

fn validate_bit_rows(
    params: &crate::pcs::IntEvalParams,
    rows: &[Vec<u64>],
) -> Result<(), BabyBearSpartanF2zError> {
    let row_count = checked_pow2(params.t)?;
    let col_count = checked_pow2(params.s)?;
    if row_count % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(BabyBearSpartanF2zError::InvalidBitRows);
    }
    let words_per_col = row_count / u64::BITS as usize;
    if rows.iter().any(|row| row.len() != words_per_col) {
        return Err(BabyBearSpartanF2zError::InvalidBitRows);
    }
    Ok(())
}

fn validate_config_pair(
    params: &crate::pcs::IntEvalParams,
    pc: &LigProverConfig,
    vc: &LigVerifierConfig,
) -> Result<(), BabyBearSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
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
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_commitment(
    params: &crate::pcs::IntEvalParams,
    commitment: &Commitment,
    pc: &LigProverConfig,
) -> Result<(), BabyBearSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    };
    let commitment_params = &commitment.params;
    if commitment_params.m != m_p + LOG_PACKING
        || commitment_params.log_inv_rate != log_inv_rate
        || commitment_params.log_batch_size != pc.initial_k
        || commitment_params.profile != LigeritoProfile::default()
        || commitment_params.merkle_hash != pc.merkle_hash
    {
        return Err(BabyBearSpartanF2zError::CommitmentConfigMismatch);
    }
    Ok(())
}

fn validate_relation(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
) -> Result<(), BabyBearSpartanF2zError> {
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    if matrices.field_modulus_encoding() != expected {
        return Err(BabyBearSpartanF2zError::UnsupportedFieldModulus);
    }
    Ok(())
}

fn validate_relation_layout(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    layout: &BabyBearMulLayout,
) -> Result<(), BabyBearSpartanF2zError> {
    if matrices.matrices().row_count() != layout.multiplications()
        || matrices.matrices().column_count() != layout.assignment_len()
    {
        return Err(BabyBearSpartanF2zError::RelationWitnessLayoutMismatch);
    }
    Ok(())
}

fn prepare_baby_bear_bitified_claim(
    opening: &BabyBearBitifiedClaim,
) -> Result<PreparedBabyBearBitifiedClaim, BabyBearSpartanF2zError> {
    let chunks = prepare_baby_bear_bitified_chunks(opening)?;
    let params = opening.params;
    let col_weights = if opening.col_scale == Fq(0) {
        vec![Fq(0); checked_pow2(params.s)?]
    } else {
        let (gate_low, _) = opening.gate_point.split_at(params.s);
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
    Ok(PreparedBabyBearBitifiedClaim {
        chunks,
        col_weights,
        claimed: opening.claimed,
    })
}

/// Compile the folded row functional directly into validated mod-q chunks.
/// The prover never materializes or hashes a second dense row-weight table.
fn prepare_baby_bear_bitified_chunks(
    opening: &BabyBearBitifiedClaim,
) -> Result<crate::pcs::ModQWeightChunks, BabyBearSpartanF2zError> {
    let params = opening.params;
    let high_vars = params
        .t
        .checked_sub(7)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    let gate_vars = params
        .s
        .checked_add(high_vars)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    if params.word_bits != 1 || opening.gate_point.len() != gate_vars {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    let (_, gate_high) = opening.gate_point.split_at(params.s);

    match opening.rows {
        BabyBearBitifiedRows::ConstantOrPaddingDummy => {
            let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, FQ_BITS)
                .map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)?;
            chunks
                .set_weight_range(0, &[1])
                .map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)?;
            Ok(chunks)
        }
        BabyBearBitifiedRows::Structured { a, b, c, k } => {
            let high_gate_count = checked_pow2(gate_high.len())?;
            let row_count = checked_pow2(params.t)?;
            let blocks = [
                (BABY_BEAR_MUL_A_SLOT_START, a),
                (BABY_BEAR_MUL_B_SLOT_START, b),
                (BABY_BEAR_MUL_C_SLOT_START, c),
                (BABY_BEAR_MUL_K_SLOT_START, k),
            ];
            let mut scratch = vec![0_u128; high_gate_count];

            if crate::pcs::mod_q_num_chunks(&params, FQ_BITS) == 1 {
                let mut weights = Vec::with_capacity(row_count);
                for (slot_start, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        BABY_BEAR_MUL_VALUE_BITS,
                        block_factor,
                        gate_high,
                        |row_start, range| {
                            if row_start != weights.len() {
                                return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
                            }
                            weights.extend_from_slice(range);
                            Ok(())
                        },
                    )?;
                }
                // Slots 124..128 are committed but have public zero weight.
                weights.resize(row_count, 0);
                crate::pcs::ModQWeightChunks::from_single_chunk(&params, FQ_BITS, weights)
                    .map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)
            } else {
                let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, FQ_BITS)
                    .map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)?;
                for (slot_start, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        BABY_BEAR_MUL_VALUE_BITS,
                        block_factor,
                        gate_high,
                        |row_start, range| {
                            chunks
                                .set_weight_range(row_start, range)
                                .map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)
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

fn eq_le_table_fq_fast(point: &[Fq]) -> Result<Vec<Fq>, BabyBearSpartanF2zError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);
    let arith = f2z_fq_arith();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
        let factor = arith.monty_factor(coordinate.0);
        let (zero_children, one_children) = table[..active_len].split_at_mut(half);
        let expand = |zero: &mut Fq, one: &mut Fq| {
            let parent = zero.0;
            let one_child = arith.mul_plain_by(parent, &factor);
            zero.0 = fq_sub(parent, one_child);
            one.0 = one_child;
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
    Ok(table)
}

fn scaled_eq_le_table_fq_into(
    point: &[Fq],
    scale: Fq,
    table: &mut [u128],
) -> Result<(), BabyBearSpartanF2zError> {
    if table.len() != checked_pow2(point.len())? {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    table[0] = scale.0;
    let arith = f2z_fq_arith();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
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

fn fill_block_weight_ranges(
    scratch: &mut [u128],
    bit_slot_start: usize,
    bit_count: usize,
    block_factor: Fq,
    gate_high: &[Fq],
    mut write_range: impl FnMut(usize, &[u128]) -> Result<(), BabyBearSpartanF2zError>,
) -> Result<(), BabyBearSpartanF2zError> {
    let high_gate_count = checked_pow2(gate_high.len())?;
    if scratch.len() != high_gate_count {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    scaled_eq_le_table_fq_into(gate_high, block_factor, scratch)?;

    for bit in 0..bit_count {
        let slot = bit_slot_start
            .checked_add(bit)
            .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
        let row_start = slot
            .checked_mul(high_gate_count)
            .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
        write_range(row_start, scratch)?;
        if bit + 1 != bit_count {
            cfg_iter_mut!(scratch, 256).for_each(|weight| *weight = fq_mul_pow2(*weight));
        }
    }
    Ok(())
}

/// Multiply a canonical q residue by two using `2^100 = 15 (mod q)`.
#[inline]
fn fq_mul_pow2(value: u128) -> u128 {
    debug_assert!(value < FQ_MOD);
    let shifted = value.wrapping_shl(1);
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
    layout: &BabyBearMulLayout,
    commitment: &Commitment,
) -> Result<[u8; 32], BabyBearSpartanF2zError> {
    let f2z_params = layout.f2z_params();
    let commitment_params = &commitment.params;
    let mut hasher = Hasher::new();
    hasher.update(ASSIGNMENT_BINDING_DOMAIN);
    hasher.update(ASSIGNMENT_BLOCK_ORDER);
    hasher.update(&commitment.root);
    hash_usize(&mut hasher, commitment_params.m)?;
    hash_usize(&mut hasher, commitment_params.log_inv_rate)?;
    hash_usize(&mut hasher, commitment_params.log_batch_size)?;
    hasher.update(&[profile_code(commitment_params.profile)]);
    hasher.update(&[hash_code(commitment_params.merkle_hash)]);
    hasher.update(&FQ_MOD.to_le_bytes());
    hasher.update(&BABY_BEAR_MODULUS.to_le_bytes());
    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.padded_assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, LOGICAL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, PADDED_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_VALUE_BITS)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_A_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_B_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_C_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_K_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_BIT_SLOTS)?;
    hash_usize(&mut hasher, f2z_params.t)?;
    hash_usize(&mut hasher, f2z_params.s)?;
    hash_usize(&mut hasher, f2z_params.word_bits)?;
    Ok(*hasher.finalize().as_bytes())
}

fn bitified_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, BabyBearMulCoefficient>,
    assignment_binding: &[u8; 32],
    layout: &BabyBearMulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &BabyBearBitifiedClaim,
) -> Result<[u8; 32], BabyBearSpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(BITIFIED_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hash_usize(&mut hasher, matrices.field_modulus_encoding().len())?;
    hasher.update(matrices.field_modulus_encoding());
    hasher.update(matrices.digest());
    hasher.update(&FQ_MOD.to_le_bytes());
    hasher.update(&BABY_BEAR_MODULUS.to_le_bytes());

    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.padded_assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, LOGICAL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, PADDED_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, opening.params.t)?;
    hash_usize(&mut hasher, opening.params.s)?;
    hash_usize(&mut hasher, opening.params.word_bits)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_A_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_B_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_C_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_K_SLOT_START)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_VALUE_BITS)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS)?;
    hash_usize(&mut hasher, BABY_BEAR_MUL_BIT_SLOTS)?;
    // Mapping version two: little-endian bits, e0/a/b/c/k/zero/zero/zero
    // assignment order, raw k reconstruction, and nonzero scale normalized
    // onto the folded row factors.
    hasher.update(&[2, 0, 1, 2, 3, 4, 5, 6, 7]);

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
        BabyBearBitifiedRows::Structured { a, b, c, k } => {
            hasher.update(&[0]);
            hasher.update(&a.0.to_le_bytes());
            hasher.update(&b.0.to_le_bytes());
            hasher.update(&c.0.to_le_bytes());
            hasher.update(&k.0.to_le_bytes());
        }
        BabyBearBitifiedRows::ConstantOrPaddingDummy => {
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

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), BabyBearSpartanF2zError> {
    let value =
        u64::try_from(value).map_err(|_| BabyBearSpartanF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn packed_variables(params: &crate::pcs::IntEvalParams) -> Result<usize, BabyBearSpartanF2zError> {
    if !params.word_bits.is_power_of_two() || params.word_bits > u128::BITS as usize {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    let row_bit_vars = params
        .t
        .checked_add(params.word_bits.trailing_zeros() as usize)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    let expected = row_bit_vars
        .checked_sub(LOG_PACKING)
        .and_then(|folded| folded.checked_add(params.s))
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)?;
    if packed_vars(params) != expected {
        return Err(BabyBearSpartanF2zError::InvalidF2zParameters);
    }
    Ok(expected)
}

fn checked_pow2(exponent: usize) -> Result<usize, BabyBearSpartanF2zError> {
    let exponent =
        u32::try_from(exponent).map_err(|_| BabyBearSpartanF2zError::InvalidF2zParameters)?;
    1_usize
        .checked_shl(exponent)
        .ok_or(BabyBearSpartanF2zError::InvalidF2zParameters)
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{FromWithConfig, crypto_bigint_uint::Uint};
    use flock_core::pcs::commit::{Commitment, PcsParams};

    use super::*;
    use crate::{
        pcs::eq_le_table_fq,
        piop::spartan::{
            ConstraintMatrices, SparseMatrix,
            baby_bear_mul::{
                BabyBearMulWitness, baby_bear_mul_constraint_matrices,
                prepare_baby_bear_mul_relation,
            },
        },
        transcript::Blake3Transcript,
    };

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
    fn bitification_is_the_adjoint_of_five_block_integer_reconstruction() {
        let modulus = BABY_BEAR_MODULUS as u32;
        let witness = BabyBearMulWitness::from_inputs(&[
            (0, modulus - 1),
            (1, 7),
            (modulus - 1, modulus - 1),
        ])
        .unwrap();
        let layout = witness.layout();
        let params = layout.f2z_params();

        // Non-Boolean selector coordinates exercise all eight multilinear
        // block factors, not merely one concrete block lookup.
        let gate_point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        let block_point = [Fq(7), Fq(11), Fq(17)];
        let block_factors = eq_le_table_fq(&block_point);
        let eq_gate = eq_le_table_fq(&gate_point);
        let scale = Fq(13);

        let assignment = witness.assignment();
        let mut assignment_evaluation = Fq(0);
        for block in 0..PADDED_ASSIGNMENT_BLOCKS {
            for gate in 0..layout.capacity() {
                let integer = if block < LOGICAL_ASSIGNMENT_BLOCKS {
                    assignment[block * layout.capacity() + gate]
                } else {
                    0
                };
                assignment_evaluation = assignment_evaluation
                    + block_factors[block] * eq_gate[gate] * Fq::from(u128::from(integer));
            }
        }
        let value = scale * assignment_evaluation;

        let mut point = gate_point.clone();
        point.extend(block_point);
        let opening =
            bitify_baby_bear_mul_spartan_claim(&terminal_claim(&point, scale, value), layout)
                .unwrap();
        let prepared = prepare_baby_bear_bitified_claim(&opening).unwrap();
        assert_eq!(prepared.chunks.len(), 1);
        let row_weights = &prepared.chunks.chunks()[0];

        let high_gate_count = checked_pow2(layout.gate_vars() - params.s).unwrap();
        let (gate_low, gate_high) = gate_point.split_at(params.s);
        assert_eq!(prepared.col_weights, eq_le_table_fq(gate_low));
        let eq_high = eq_le_table_fq(gate_high);
        for (slot_start, block_index) in [
            (BABY_BEAR_MUL_A_SLOT_START, 1_usize),
            (BABY_BEAR_MUL_B_SLOT_START, 2),
            (BABY_BEAR_MUL_C_SLOT_START, 3),
            (BABY_BEAR_MUL_K_SLOT_START, 4),
        ] {
            for bit in 0..BABY_BEAR_MUL_VALUE_BITS {
                let row_start = (slot_start + bit) * high_gate_count;
                let bit_weight = Fq::from(1_u128 << bit);
                for (high_gate, equality_weight) in eq_high.iter().copied().enumerate() {
                    assert_eq!(
                        Fq(row_weights[row_start + high_gate]),
                        scale * block_factors[block_index] * bit_weight * equality_weight
                    );
                }
            }
        }
        for slot in BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS..BABY_BEAR_MUL_BIT_SLOTS {
            let start = slot * high_gate_count;
            assert!(
                row_weights[start..start + high_gate_count]
                    .iter()
                    .all(|&weight| weight == 0)
            );
        }

        let rows = witness.f2z_bit_rows();
        let mut read_off = Fq(0);
        for b in 0..params.rows() {
            for c in 0..params.cols() {
                let bit = (rows[c][b / u64::BITS as usize] >> (b % u64::BITS as usize)) & 1;
                read_off = read_off
                    + Fq::from(u128::from(bit))
                        * Fq(row_weights[b])
                        * prepared.col_weights[c];
            }
        }
        assert_eq!(read_off, prepared.claimed);
    }

    #[test]
    fn compact_rows_reconstruct_four_values_and_leave_unused_slots_zero() {
        let modulus = BABY_BEAR_MODULUS as u32;
        let witness = BabyBearMulWitness::from_inputs(&[(modulus - 1, modulus - 1)]).unwrap();
        let layout = witness.layout();
        let rows = witness.f2z_bit_rows();

        let bit_at = |slot: usize, gate: usize| {
            let (b, c) = layout.f2z_cell(slot, gate).unwrap();
            (rows[c][b / u64::BITS as usize] >> (b % u64::BITS as usize)) & 1
        };
        let reconstruct = |start: usize| {
            (0..BABY_BEAR_MUL_VALUE_BITS)
                .map(|bit| bit_at(start + bit, 0) << bit)
                .sum::<u64>()
        };

        assert_eq!(
            reconstruct(BABY_BEAR_MUL_A_SLOT_START),
            u64::from(modulus - 1)
        );
        assert_eq!(
            reconstruct(BABY_BEAR_MUL_B_SLOT_START),
            u64::from(modulus - 1)
        );
        assert_eq!(reconstruct(BABY_BEAR_MUL_C_SLOT_START), 1);
        assert_eq!(
            reconstruct(BABY_BEAR_MUL_K_SLOT_START),
            BABY_BEAR_MODULUS - 2
        );
        for slot in BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS..BABY_BEAR_MUL_BIT_SLOTS {
            for gate in 0..layout.capacity() {
                assert_eq!(bit_at(slot, gate), 0);
            }
        }
    }

    #[test]
    fn zero_scale_keeps_a_nonzero_row_functional() {
        let layout = BabyBearMulLayout::new(3).unwrap();
        let mut point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        point.extend([Fq(7), Fq(11), Fq(17)]);
        let opening =
            bitify_baby_bear_mul_spartan_claim(&terminal_claim(&point, Fq(0), Fq(0)), &layout)
                .unwrap();
        let prepared = prepare_baby_bear_bitified_claim(&opening).unwrap();

        assert!(
            prepared
                .chunks
                .chunks()
                .iter()
                .flatten()
                .any(|&weight| weight != 0)
        );
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed(), Fq(0));
    }

    #[test]
    fn constant_and_padding_claims_use_the_deterministic_dummy_functional() {
        let layout = BabyBearMulLayout::new(3).unwrap();
        let gate_point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        let scale = Fq(13);
        let constant_evaluation = eq_le_table_fq(&gate_point)[0];

        let mut constant_point = gate_point.clone();
        constant_point.extend([Fq(0), Fq(0), Fq(0)]);
        let constant = bitify_baby_bear_mul_spartan_claim(
            &terminal_claim(&constant_point, scale, scale * constant_evaluation),
            &layout,
        )
        .unwrap();
        assert_dummy_zero_claim(&constant);

        // Boolean block point 101 selects padded block five.
        let mut padding_point = gate_point;
        padding_point.extend([Fq(1), Fq(0), Fq(1)]);
        let padding = bitify_baby_bear_mul_spartan_claim(
            &terminal_claim(&padding_point, scale, Fq(0)),
            &layout,
        )
        .unwrap();
        assert_dummy_zero_claim(&padding);
    }

    #[test]
    fn tampered_constant_or_padding_only_claim_is_rejected() {
        let layout = BabyBearMulLayout::new(3).unwrap();
        let gate_point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();

        // Boolean block point 110 selects padded block six, whose assignment
        // polynomial is public zero.  A nonzero terminal value cannot be
        // discharged by the dummy F2Z functional.
        let mut point = gate_point;
        point.extend([Fq(0), Fq(1), Fq(1)]);
        assert!(matches!(
            bitify_baby_bear_mul_spartan_claim(&terminal_claim(&point, Fq(1), Fq(1)), &layout,),
            Err(BabyBearSpartanF2zError::InvalidConstantOrPaddingOnlyClaim)
        ));
    }

    fn assert_dummy_zero_claim(opening: &BabyBearBitifiedClaim) {
        let prepared = prepare_baby_bear_bitified_claim(opening).unwrap();
        assert_eq!(prepared.chunks.chunks()[0][0], 1);
        assert!(
            prepared.chunks.chunks()[0][1..]
                .iter()
                .all(|&weight| weight == 0)
        );
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed(), Fq(0));
    }

    #[test]
    fn malformed_claim_residue_is_rejected_before_canonical_projection() {
        let layout = BabyBearMulLayout::new(3).unwrap();
        let config = spartan_f2z_field_config();
        let malformed = SpartanF2zField::new_unchecked(Uint::from(u128::MAX), &config);
        let zero = SpartanF2zField::from_with_cfg(0_u128, &config);
        let mut point = vec![zero.clone(); layout.gate_vars() + 3];
        point[0] = malformed;
        let claim = ScaledMleEvaluationClaim::new(point.into_boxed_slice(), zero.clone(), zero);

        assert!(matches!(
            bitify_baby_bear_mul_spartan_claim(&claim, &layout),
            Err(BabyBearSpartanF2zError::ClaimFieldMismatch)
        ));
    }

    #[test]
    fn compact_digest_binds_factors_layout_and_matrix() {
        let layout = BabyBearMulLayout::new(3).unwrap();
        let field_config = spartan_f2z_field_config();
        let matrices = prepare_baby_bear_mul_relation(layout, &field_config).unwrap();
        let mut point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        point.extend([Fq(2), Fq(3), Fq(5)]);
        let terminal = terminal_claim(&point, Fq(7), Fq(11));
        let opening = bitify_baby_bear_mul_spartan_claim(&terminal, &layout).unwrap();
        let binding = [0x42; 32];
        let digest =
            bitified_claim_digest(&matrices, &binding, &layout, &terminal, &opening).unwrap();
        assert_eq!(
            bitified_claim_digest(&matrices, &binding, &layout, &terminal, &opening).unwrap(),
            digest
        );

        let mut tampered_opening = opening.clone();
        let BabyBearBitifiedRows::Structured { a, .. } = &mut tampered_opening.rows else {
            panic!("non-Boolean block point must have a structured functional");
        };
        *a = *a + Fq(1);
        assert_ne!(
            bitified_claim_digest(
                &matrices,
                &binding,
                &layout,
                &terminal,
                &tampered_opening,
            )
            .unwrap(),
            digest
        );

        let alternate_layout = BabyBearMulLayout::new(4).unwrap();
        assert_eq!(alternate_layout.capacity(), layout.capacity());
        assert_ne!(
            bitified_claim_digest(
                &matrices,
                &binding,
                &alternate_layout,
                &terminal,
                &opening,
            )
            .unwrap(),
            digest
        );

        let original = baby_bear_mul_constraint_matrices(&layout).unwrap();
        let capacity = layout.capacity();
        let c_rows = (0..layout.multiplications())
            .map(|row| {
                vec![
                    (3 * capacity + row, BabyBearMulCoefficient::One),
                    (4 * capacity + row, BabyBearMulCoefficient::One),
                ]
            })
            .collect::<Vec<_>>();
        let tampered_c = SparseMatrix::try_from_rows(layout.assignment_len(), c_rows).unwrap();
        let tampered_relation = ConstraintMatrices::new(
            original.a().clone(),
            original.b().clone(),
            tampered_c,
        )
        .unwrap();
        let tampered_matrices =
            PreparedConstraintMatrices::<SpartanF2zField, BabyBearMulCoefficient>::new(
                tampered_relation,
                &field_config,
            )
            .unwrap();
        assert_ne!(
            bitified_claim_digest(
                &tampered_matrices,
                &binding,
                &layout,
                &terminal,
                &opening,
            )
            .unwrap(),
            digest
        );
    }

    #[test]
    fn baby_bear_prepared_statement_has_a_distinct_lower_domain() {
        let layout = BabyBearMulLayout::new(1 << MIN_PRODUCTION_GATE_VARS).unwrap();
        let params = layout.f2z_params();
        let (pc, _) = configs_for_layout(&layout).unwrap();
        let commitment = Commitment {
            root: [0x5a; 32],
            params: PcsParams {
                m: packed_variables(&params).unwrap() + LOG_PACKING,
                log_inv_rate: pc.log_inv_rates[0],
                log_batch_size: pc.initial_k,
                profile: LigeritoProfile::default(),
                merkle_hash: pc.merkle_hash,
            },
        };
        let bridge_digest = [0xa5; 32];

        let mut baby_bear_transcript = Blake3Transcript::new();
        let _ = crate::ligerito_flock::absorb_baby_bear_mod_q_opening_v2_statement(
            &mut baby_bear_transcript,
            &commitment,
            &params,
            &bridge_digest,
            FQ_BITS,
            f2z_generator(),
            &pc,
        );
        let baby_bear_challenge = baby_bear_transcript.get_challenge::<u128>();

        let mut u32_transcript = Blake3Transcript::new();
        let _ = crate::ligerito_flock::absorb_u32_mod_q_opening_v2_statement(
            &mut u32_transcript,
            &commitment,
            &params,
            &bridge_digest,
            FQ_BITS,
            f2z_generator(),
            &pc,
        );
        assert_ne!(
            u32_transcript.get_challenge::<u128>(),
            baby_bear_challenge
        );
    }

    #[test]
    fn config_pair_validation_binds_recursive_and_query_geometry() {
        let layout = BabyBearMulLayout::new(1 << MIN_PRODUCTION_GATE_VARS).unwrap();
        let params = layout.f2z_params();
        let (pc, mut vc) = configs_for_layout(&layout).unwrap();
        validate_config_pair(&params, &pc, &vc).unwrap();

        vc.queries.pop();
        assert!(matches!(
            validate_config_pair(&params, &pc, &vc),
            Err(BabyBearSpartanF2zError::InvalidF2zParameters)
        ));
    }

    #[test]
    fn combined_protocol_uses_only_validator_gated_production_profiles() {
        let small = BabyBearMulLayout::new(3).unwrap();
        assert!(matches!(
            configs_for_layout(&small),
            Err(BabyBearSpartanF2zError::UnauditedF2zParameters)
        ));

        let production = BabyBearMulLayout::new(1 << MIN_PRODUCTION_GATE_VARS).unwrap();
        configs_for_layout(&production).expect("the smallest embedded profile is available");
    }

    #[test]
    fn compact_baby_bear_piop_strategies_are_byte_exact_and_verify() {
        let modulus = BABY_BEAR_MODULUS as u32;
        let witness = BabyBearMulWitness::from_inputs(&[
            (0, modulus - 1),
            (1, 7),
            (modulus - 1, modulus - 1),
            (17, 19),
        ])
        .unwrap();
        let layout = *witness.layout();
        let field_config = spartan_f2z_field_config();
        let matrices = prepare_baby_bear_mul_relation(layout, &field_config).unwrap();
        let assignment_binding = [0x42; 32];

        let prove_and_verify = |strategy| {
            let mut prover_transcript = Blake3Transcript::new();
            let (proof, claim) = match strategy {
                SpartanReductionStrategy::Immediate => {
                    let (assignment, products) =
                        project_baby_bear_mul_witness::<SpartanF2zField>(&witness, &field_config)
                            .unwrap();
                    prove_spartan_piop_with_strategy(
                        &mut prover_transcript,
                        &matrices,
                        &assignment_binding,
                        products,
                        assignment,
                        strategy,
                    )
                    .unwrap()
                }
                SpartanReductionStrategy::DelayedBarrett
                | SpartanReductionStrategy::DelayedCryptoBigint => {
                    let (assignment, products) =
                        project_baby_bear_mul_native_witness(&witness).into_parts();
                    prove_spartan_piop_native_u64_with_strategy(
                        &mut prover_transcript,
                        &matrices,
                        &assignment_binding,
                        products,
                        assignment,
                        strategy,
                    )
                    .unwrap()
                }
            };
            let prover_continuation = prover_transcript.get_challenge::<u128>();

            let mut verifier_transcript = Blake3Transcript::new();
            let verified_claim = verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &proof,
            )
            .unwrap();
            assert_eq!(verified_claim, claim);
            assert_eq!(
                verifier_transcript.get_challenge::<u128>(),
                prover_continuation
            );

            (proof, claim, prover_continuation)
        };

        let reference = prove_and_verify(SpartanReductionStrategy::Immediate);
        assert_eq!(
            prove_and_verify(SpartanReductionStrategy::DelayedBarrett),
            reference
        );
        assert_eq!(
            prove_and_verify(SpartanReductionStrategy::DelayedCryptoBigint),
            reference
        );
    }

    #[test]
    #[ignore = "runs six production-sized BabyBear Spartan/F2Z proofs"]
    fn combined_strategies_are_byte_exact_and_all_verify() {
        let modulus = BABY_BEAR_MODULUS;
        let witness = BabyBearMulWitness::from_fn(1 << MIN_PRODUCTION_GATE_VARS, |index| {
            let a = ((index as u64).wrapping_mul(0x5BD1_E995) % modulus) as u32;
            let b = ((index as u64)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(0xA5A5_5A5A)
                % modulus) as u32;
            (a, b)
        })
        .unwrap();
        let layout = *witness.layout();
        let field_config = spartan_f2z_field_config();
        let matrices = prepare_baby_bear_mul_relation(layout, &field_config).unwrap();
        let hint = commit_baby_bear_mul_witness(&layout, witness.f2z_bit_rows()).unwrap();

        // The live-row count is transcript-bound even when the padded capacity
        // and commitment geometry remain unchanged.
        let alternate_layout = BabyBearMulLayout::new(layout.multiplications() - 1).unwrap();
        assert_eq!(alternate_layout.capacity(), layout.capacity());
        assert_ne!(
            assignment_binding(&alternate_layout, &hint.commitment).unwrap(),
            assignment_binding(&layout, &hint.commitment).unwrap()
        );

        let verify_and_fingerprint =
            |proof: BabyBearMulSpartanF2zProof,
             continuation: u128|
             -> (SpartanPiopProof<SpartanF2zField>, Vec<u8>, u128) {
                let mut verifier_transcript = Blake3Transcript::new();
                verify_baby_bear_mul_spartan_and_f2z(
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
        let production_proof = prove_baby_bear_mul_spartan_and_f2z_from_witness(
            &mut production_transcript,
            &matrices,
            &layout,
            &witness,
            &hint,
        )
        .unwrap();
        let production_continuation = production_transcript.get_challenge::<u128>();

        let mut wrong_layout_transcript = Blake3Transcript::new();
        assert!(
            verify_baby_bear_mul_spartan_and_f2z(
                &mut wrong_layout_transcript,
                &matrices,
                &alternate_layout,
                &hint.commitment,
                &production_proof,
            )
            .is_err()
        );

        let mut tampered_commitment = hint.commitment.clone();
        tampered_commitment.root[0] ^= 1;
        let mut tampered_commitment_transcript = Blake3Transcript::new();
        assert!(
            verify_baby_bear_mul_spartan_and_f2z(
                &mut tampered_commitment_transcript,
                &matrices,
                &layout,
                &tampered_commitment,
                &production_proof,
            )
            .is_err()
        );

        // Isolate the Spartan-to-F2Z linkage from simple commitment-root
        // binding. Prove and verify against the same altered commitment while
        // retaining the honest integer assignment in Spartan. The derived
        // assignment claim and the committed semantic bit no longer agree, so
        // the freshly generated combined proof must fail verification.
        let mut inconsistent_rows = witness.f2z_bit_rows();
        let (bit_row, bit_column) = layout.f2z_cell(BABY_BEAR_MUL_A_SLOT_START, 0).unwrap();
        inconsistent_rows[bit_column][bit_row / u64::BITS as usize] ^=
            1_u64 << (bit_row % u64::BITS as usize);
        let inconsistent_hint = commit_baby_bear_mul_witness(&layout, inconsistent_rows).unwrap();
        let mut inconsistent_prover_transcript = Blake3Transcript::new();
        let inconsistent_proof = prove_baby_bear_mul_spartan_and_f2z_from_witness(
            &mut inconsistent_prover_transcript,
            &matrices,
            &layout,
            &witness,
            &inconsistent_hint,
        )
        .unwrap();
        let mut inconsistent_verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_baby_bear_mul_spartan_and_f2z(
                &mut inconsistent_verifier_transcript,
                &matrices,
                &layout,
                &inconsistent_hint.commitment,
                &inconsistent_proof,
            )
            .is_err()
        );

        let mut tampered_proof = production_proof.clone();
        tampered_proof.spartan.outer.sumcheck.round_polynomials[0][0] +=
            &SpartanF2zField::from_with_cfg(1_u64, &field_config);
        let mut tampered_proof_transcript = Blake3Transcript::new();
        assert!(
            verify_baby_bear_mul_spartan_and_f2z(
                &mut tampered_proof_transcript,
                &matrices,
                &layout,
                &hint.commitment,
                &tampered_proof,
            )
            .is_err()
        );

        // Replacing the public modulus coefficient in C changes the prepared
        // statement digest, so an existing proof must not verify.
        let original_relation = baby_bear_mul_constraint_matrices(&layout).unwrap();
        let capacity = layout.capacity();
        let columns = layout.assignment_len();
        let tampered_c_rows: Vec<Vec<(usize, BabyBearMulCoefficient)>> = (0..layout
            .multiplications())
            .map(|row| {
                vec![
                    (3 * capacity + row, BabyBearMulCoefficient::One),
                    (4 * capacity + row, BabyBearMulCoefficient::One),
                ]
            })
            .collect();
        let tampered_c = SparseMatrix::try_from_rows(columns, tampered_c_rows).unwrap();
        let tampered_relation = ConstraintMatrices::new(
            original_relation.a().clone(),
            original_relation.b().clone(),
            tampered_c,
        )
        .unwrap();
        let tampered_matrices =
            PreparedConstraintMatrices::<SpartanF2zField, BabyBearMulCoefficient>::new(
                tampered_relation,
                &field_config,
            )
            .unwrap();
        let mut tampered_matrix_transcript = Blake3Transcript::new();
        assert!(
            verify_baby_bear_mul_spartan_and_f2z(
                &mut tampered_matrix_transcript,
                &tampered_matrices,
                &layout,
                &hint.commitment,
                &production_proof,
            )
            .is_err()
        );

        let reference = verify_and_fingerprint(production_proof, production_continuation);

        let (assignment, products) =
            project_baby_bear_mul_witness::<SpartanF2zField>(&witness, &field_config).unwrap();
        let mut compatibility_transcript = Blake3Transcript::new();
        let compatibility_proof = prove_baby_bear_mul_spartan_and_f2z(
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

        for (strategy_index, strategy) in [
            SpartanReductionStrategy::Immediate,
            SpartanReductionStrategy::DelayedBarrett,
            SpartanReductionStrategy::DelayedCryptoBigint,
        ]
        .into_iter()
        .enumerate()
        {
            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_baby_bear_mul_spartan_and_f2z_with_strategy(
                &mut prover_transcript,
                &matrices,
                &layout,
                &witness,
                &hint,
                strategy,
            )
            .unwrap();
            let continuation = prover_transcript.get_challenge::<u128>();

            // The same proof must not verify against commitments obtained by
            // changing an operand, c, k, or even an unused committed bit. The
            // unused slot has no opening weight, but the commitment itself is
            // still part of Spartan's assignment-oracle binding.
            if strategy_index == 0 {
                for slot in [
                    BABY_BEAR_MUL_A_SLOT_START,
                    BABY_BEAR_MUL_C_SLOT_START,
                    BABY_BEAR_MUL_K_SLOT_START,
                    BABY_BEAR_MUL_SEMANTIC_BIT_SLOTS,
                ] {
                    let mut tampered_rows = witness.f2z_bit_rows();
                    let (b, c) = layout.f2z_cell(slot, 0).unwrap();
                    tampered_rows[c][b / u64::BITS as usize] ^= 1_u64 << (b % u64::BITS as usize);
                    let tampered_hint =
                        commit_baby_bear_mul_witness(&layout, tampered_rows).unwrap();
                    let mut tampered_transcript = Blake3Transcript::new();
                    assert!(
                        verify_baby_bear_mul_spartan_and_f2z(
                            &mut tampered_transcript,
                            &matrices,
                            &layout,
                            &tampered_hint.commitment,
                            &proof,
                        )
                        .is_err()
                    );
                }
            }

            let fingerprint = verify_and_fingerprint(proof, continuation);
            assert_eq!(fingerprint, reference);
        }
    }
}
