//! Succinct opening of Spartan's terminal assignment claim through F2Z.
//!
//! Spartan constrains the integer assignment
//!
//! ```text
//! [constant block | x block | y block | product block]
//! ```
//!
//! over a transcript-selected prime. The commitment contains only the compact
//! `32 + 32 + 64` little-endian bits for each multiplication. This module
//! applies the transpose of that public bitification map to Spartan's terminal
//! assignment-MLE claim and discharges the resulting claim with F2Z. The outer
//! Spartan reduction uses a fixed known-zero univariate-prefix skip.

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
    ext_proj::{PrimeSamplingError, ProjArith, sample_prime_in_interval},
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, ModQOpeningKind,
        commit_rs_ligerito_rows, custom_johnson_config_bits,
        prove_mle_eval_mod_q_ligerito_with_weight_chunks,
        validated_udr_lig_configs_for_target, verify_mle_eval_mod_q_ligerito_with_weight_chunks,
    },
    pcs::{FQ_BITS, FQ_MOD, Fq, ProjectCanonicalU128},
    poly::univariate::binary_gf128::BinaryFieldGF128,
    transcript::traits::{GenTranscribable, Transcript},
};

use crate::utils::cfg_iter_mut;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    ConstraintMatricesSkeleton, PreparedConstraintMatrices, SpartanField, absorb_spartan_message,
    grinding::{GrindingDomain, GrindingError, GrindingRound, grind_and_absorb, verify_and_absorb},
    matrix::ScaledMleEvaluationClaim,
    piop::{
        SpartanError, prove_spartan_piop_u32_native_with_univariate_skip_borrowed,
        verify_spartan_univariate_skip_proof,
    },
    profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100, ProfileError},
    raw_monty::NativeProducts,
    u32_mul::{
        U32_MUL_BIT_SLOTS, U32_MUL_PRODUCT_BITS, U32_MUL_PRODUCT_SLOT_START, U32_MUL_X_BITS,
        U32_MUL_X_SLOT_START, U32_MUL_Y_BITS, U32_MUL_Y_SLOT_START, U32MulError, U32MulLayout,
        U32MulWitness, u32_mul_constraint_matrices,
    },
    univariate_skip::UnivariateSkipSpartanPiopProof,
};

/// Domain of the compact, factorized claim bound between Spartan and F2Z.
const BITIFIED_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-f2z/bitified-claim/v3";

/// Embedded, validator-gated Ligerito profiles begin at a 22-variable
/// committed bit MLE: seven slot variables plus fifteen gate variables.
pub(crate) const MIN_PRODUCTION_GATE_VARS: usize = 15;

/// Runtime-configured Spartan field used by the concrete F2Z adapter.
pub type SpartanF2zField = F128;

/// Compact result of applying the public u32-multiplication bitification map.
///
/// The row and column functionals remain factorized here.  They are compiled
/// directly into the mod-q chunk representation at the F2Z boundary, so the
/// deterministic Bitify reduction does not allocate either dense weight
/// vector.
#[derive(Clone, Debug, Eq, PartialEq)]
struct U32BitifiedClaim {
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

struct PreparedU32BitifiedClaim {
    chunks: crate::pcs::ModQWeightChunks,
    col_weights: Vec<Fq>,
    claimed: Fq,
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

    #[error("the sampled modulus is not supported by the runtime field")]
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

    #[cfg(feature = "bench-internals")]
    #[error("the commitment does not match the prepared terminal-opening statement")]
    PreparedOpeningCommitmentMismatch,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element does not use the sampled runtime field")]
    ClaimFieldMismatch,

    #[error("a constant-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOnlyClaim,

    #[error("a host length does not fit the canonical transcript encoding")]
    BindingEncodingOverflow,

    /// The security profile could not be instantiated at this shape.
    #[error(transparent)]
    Profile(#[from] ProfileError),

    /// Runtime-prime sampling failed.
    #[error(transparent)]
    PrimeSampling(#[from] PrimeSamplingError),

    /// A Fiat--Shamir grinding nonce could not be produced or checked.
    #[error(transparent)]
    Grinding(#[from] GrindingError),

    /// The u32 protocol supports single-prime profiles only (the two-prime
    /// Strategy 2 belongs to the MultiSwap-style adapters).
    #[error("the u32 multiplication protocol requires a single-prime security profile")]
    UnsupportedProfile,

    /// A proof selected a different univariate-prefix width than the one
    /// whose degree was included in the prepared security profile.
    #[error("the u32 proof uses univariate skip K={actual}; expected K={expected}")]
    UnexpectedUnivariateSkipVariables { expected: u8, actual: u8 },

    /// The derived prime interval must keep the row weights to one
    /// exponent-fold chunk (`q_bits <= c_w`); the profile guarantees this,
    /// so a violation is an internal error.
    #[error("the runtime prime produced a multi-chunk row functional")]
    MultiChunkRuntimeWeights,
}

/// Constructs the fixed `q = 2^100 - 15` runtime field configuration.
pub fn spartan_f2z_field_config() -> <SpartanF2zField as PrimeField>::Config {
    SpartanF2zField::make_cfg(&Uint::from(FQ_MOD)).expect("FQ_MOD is a valid odd prime modulus")
}

/// Commits prebuilt compact `32 + 32 + 64` bit rows under the prepared
/// relation's profile-selected Ligerito configuration.
///
/// Accepting ownership of `rows` lets benchmarks time bitification separately
/// and move the packed store into the commitment without retaining a duplicate.
/// Only layouts with at least `2^15` gate slots are accepted, because smaller
/// Ligerito configurations in the dependency are explicitly test-only.
pub fn commit_u32_mul_witness(
    prepared: &PreparedU32MulRelation,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, SpartanF2zError> {
    let layout = prepared.layout();
    let p = layout.f2z_params();
    validate_layout_geometry(layout)?;
    validate_bit_rows(&p, &rows)?;
    validate_config_pair(&p, &prepared.ligerito_pc, &prepared.ligerito_vc)?;

    // All assertion-bearing shape requirements of the low-level commit have
    // been checked above.
    let hint = commit_rs_ligerito_rows(&p, rows, &prepared.ligerito_pc);
    validate_commitment(&p, &hint.commitment, &prepared.ligerito_pc)?;
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
fn bitify_u32_mul_spartan_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &U32MulLayout,
    q: u128,
    arith: &ProjArith,
) -> Result<U32BitifiedClaim, SpartanF2zError> {
    validate_layout_geometry(layout)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(2) {
        return Err(SpartanF2zError::InvalidClaimPoint);
    }

    let p = layout.f2z_params();
    let expected_modulus = Uint::from(q);
    let project = |value: &SpartanF2zField| -> Result<Fq, SpartanF2zError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(SpartanF2zError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= q {
            return Err(SpartanF2zError::ClaimFieldMismatch);
        }
        Ok(Fq(canonical))
    };
    let sub = |left: u128, right: u128| -> u128 {
        if right == 0 {
            left
        } else {
            arith.add(left, q - right)
        }
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
    let one_minus_low = Fq(sub(one.0, block_low.0));
    let one_minus_high = Fq(sub(one.0, block_high.0));
    let mul = |left: Fq, right: Fq| Fq(arith.mul(left.0, right.0));

    // Block order in the integer assignment is 00=constant, 01=x, 10=y,
    // 11=product, with the first block-selector coordinate as the low bit.
    let constant_factor = mul(one_minus_low, one_minus_high);
    let x_factor = mul(block_low, one_minus_high);
    let y_factor = mul(one_minus_low, block_high);
    let product_factor = mul(block_low, block_high);

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point
        .iter()
        .copied()
        .fold(constant_factor, |acc, coordinate| {
            mul(acc, Fq(sub(one.0, coordinate.0)))
        });
    let adjusted_claim = Fq(sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

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
        (U32BitifiedRows::Structured { x, y, product }, one)
    };

    Ok(U32BitifiedClaim {
        params: p,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

/// Which Ligerito opener configuration a [`PreparedU32MulRelation`] runs.
/// The round-by-round target always comes from the security profile
/// ([`IopSecurityParams::ligerito_target_bits`]); this only picks the
/// geometry/regime. Both variants use Blake3 Merkle trees and are bound
/// into the statement, so the two produce different proof bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum U32MulLigerito {
    /// The relation's historical default: flock's validated unique-decoding
    /// configuration with fold grinding at rate 1/2, L0 folding of 2^4 rows
    /// ([`validated_udr_lig_configs_for_target`]). The `u32_mul` bench and
    /// the transcript pins run this.
    ValidatedUdr,
    /// A validator-gated Johnson-regime geometry
    /// ([`custom_johnson_config_bits`]) at the profile's target — the opener
    /// of the `f2z` CLI's raw-performance table (`custom:<log_inv_rate>:
    /// <initial_k>`), so the two paper tables share one Ligerito.
    CustomJohnson {
        /// `log₂` of the inverse Reed–Solomon rate (3 = rate 1/8).
        log_inv_rate: usize,
        /// L0 interleaving / lane fold (`log₂` rows folded at level 0).
        initial_k: usize,
    },
}

fn configs_for_layout_and_target(
    layout: &U32MulLayout,
    target_bits: usize,
    ligerito: U32MulLigerito,
) -> Result<(LigProverConfig, LigVerifierConfig), SpartanF2zError> {
    if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
        return Err(SpartanF2zError::UnauditedF2zParameters);
    }
    let p = layout.f2z_params();
    let m_p = packed_variables(&p)?;
    match ligerito {
        U32MulLigerito::ValidatedUdr => validated_udr_lig_configs_for_target(m_p, target_bits)
            .map_err(SpartanF2zError::LigeritoConfig),
        U32MulLigerito::CustomJohnson {
            log_inv_rate,
            initial_k,
        } => {
            let m = m_p.checked_add(LOG_PACKING).ok_or_else(|| {
                SpartanF2zError::LigeritoConfig("Ligerito variable count overflow".to_owned())
            })?;
            if !(20..=35).contains(&m) {
                return Err(SpartanF2zError::LigeritoConfig(format!(
                    "custom Johnson Ligerito geometry requires m in [20, 35], got {m}"
                )));
            }
            let mut security =
                custom_johnson_config_bits(m, log_inv_rate, initial_k, Some(target_bits));
            // Blake3 Merkle trees, as every Spartan path here (the slim
            // template's field says sha256).
            security.hash = "blake3".into();
            security.validate().map_err(SpartanF2zError::LigeritoConfig)?;
            security
                .to_prover_verifier_configs()
                .map_err(SpartanF2zError::LigeritoConfig)
        }
    }
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
    let total_vars =
        p.t.checked_add(p.word_bits.trailing_zeros() as usize)
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

pub(crate) fn validate_bit_rows(
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

#[cfg(feature = "bench-internals")]
fn validate_u32_prepared_opening_commitment(
    prepared: &PreparedU32TerminalF2zOpening,
    commitment: &Commitment,
) -> Result<(), SpartanF2zError> {
    validate_commitment(&prepared.params, commitment, &prepared.ligerito_pc)?;
    let binding = u32_mul_assignment_binding(
        &prepared.layout,
        commitment,
        &prepared.security,
        &prepared.ligerito_pc,
    )?;
    if binding != prepared.assignment_binding {
        return Err(SpartanF2zError::PreparedOpeningCommitmentMismatch);
    }
    Ok(())
}

fn prepare_u32_bitified_claim(
    opening: &U32BitifiedClaim,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<PreparedU32BitifiedClaim, SpartanF2zError> {
    let chunks = prepare_u32_bitified_chunks(opening, q_bits, arith)?;
    let p = opening.params;
    let col_weights = if opening.col_scale == Fq(0) {
        vec![Fq(0); checked_pow2(p.s)?]
    } else {
        let (gate_low, _) = opening.gate_point.split_at(p.s);
        let mut eq_low = eq_le_table_fq_fast_with(gate_low, arith)?;
        if opening.col_scale != Fq(1) {
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
    q_bits: usize,
    arith: &ProjArith,
) -> Result<crate::pcs::ModQWeightChunks, SpartanF2zError> {
    let p = opening.params;
    let high_vars =
        p.t.checked_add(p.word_bits.trailing_zeros() as usize)
            .and_then(|variables| variables.checked_sub(7))
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    let gate_vars =
        p.s.checked_add(high_vars)
            .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if opening.gate_point.len() != gate_vars {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let (_, gate_high) = opening.gate_point.split_at(p.s);

    match opening.rows {
        U32BitifiedRows::ConstantDummy => {
            let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&p, q_bits)
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
            let pow2_word = arith.monty_factor(arith.reduce(1_u128 << p.word_bits));

            if crate::pcs::mod_q_num_chunks(&p, q_bits) == 1 {
                let mut weights = Vec::with_capacity(row_count);
                for (slot_start, bit_count, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        bit_count,
                        p.word_bits,
                        block_factor,
                        gate_high,
                        arith,
                        &pow2_word,
                        |row_start, range| {
                            if row_start != weights.len() {
                                return Err(SpartanF2zError::InvalidF2zParameters);
                            }
                            weights.extend_from_slice(range);
                            Ok(())
                        },
                    )?;
                }
                crate::pcs::ModQWeightChunks::from_single_chunk(&p, q_bits, weights)
                    .map_err(|_| SpartanF2zError::InvalidF2zParameters)
            } else {
                let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&p, q_bits)
                    .map_err(|_| SpartanF2zError::InvalidF2zParameters)?;
                for (slot_start, bit_count, block_factor) in blocks {
                    fill_block_weight_ranges(
                        &mut scratch,
                        slot_start,
                        bit_count,
                        p.word_bits,
                        block_factor,
                        gate_high,
                        arith,
                        &pow2_word,
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

/// Little-endian equality table with one fixed-factor Montgomery
/// multiplication per parent and one allocation for the complete table.
fn eq_le_table_fq_fast_with(point: &[Fq], arith: &ProjArith) -> Result<Vec<Fq>, SpartanF2zError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);

    let q = arith.q();
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
            zero.0 = if one_child == 0 {
                parent
            } else {
                arith.add(parent, q - one_child)
            };
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

/// Write `scale * eq(., point)` into a reusable canonical-u128 buffer. Seeding
/// the recurrence with the block factor avoids first building an unscaled
/// table and then multiplying all `2^h` entries once per block.
fn scaled_eq_le_table_fq_into(
    point: &[Fq],
    scale: Fq,
    table: &mut [u128],
    arith: &ProjArith,
) -> Result<(), SpartanF2zError> {
    if table.len() != checked_pow2(point.len())? {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    table[0] = scale.0;
    let q = arith.q();

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
            *zero = if one_child == 0 {
                parent
            } else {
                arith.add(parent, q - one_child)
            };
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
    arith: &ProjArith,
    pow2_word: &crypto_bigint::modular::FixedMontyForm<{ crypto_bigint::U128::LIMBS }>,
    mut write_range: impl FnMut(usize, &[u128]) -> Result<(), SpartanF2zError>,
) -> Result<(), SpartanF2zError> {
    if !matches!(word_bits, 1 | 8) || bit_slot_start % word_bits != 0 || bit_count % word_bits != 0
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
    scaled_eq_le_table_fq_into(gate_high, block_factor, scratch, arith)?;

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
                .for_each(|weight| *weight = arith.mul_plain_by(*weight, pow2_word));
        }
    }
    Ok(())
}

pub(crate) fn fill_slot_weights(
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

fn bitified_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    assignment_binding: &[u8; 32],
    layout: &U32MulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &U32BitifiedClaim,
    modulus: u128,
) -> Result<[u8; 32], SpartanF2zError> {
    bitified_claim_digest_from_relation(
        matrices.field_modulus_encoding(),
        matrices.digest(),
        assignment_binding,
        layout,
        terminal_claim,
        opening,
        modulus,
    )
}

#[allow(clippy::too_many_arguments)]
fn bitified_claim_digest_from_relation(
    relation_modulus_encoding: &[u8],
    relation_digest: &[u8; 32],
    assignment_binding: &[u8; 32],
    layout: &U32MulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &U32BitifiedClaim,
    modulus: u128,
) -> Result<[u8; 32], SpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(BITIFIED_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hash_usize(&mut hasher, relation_modulus_encoding.len())?;
    hasher.update(relation_modulus_encoding);
    hasher.update(relation_digest);
    hasher.update(&modulus.to_le_bytes());

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
        LigeritoProfile::Slim3 => 3,
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

/// Number of little-endian row variables represented by the canonical
/// univariate-prefix reduction.
pub const U32_MUL_UNIVARIATE_SKIP_VARS: usize = 3;

/// Maximum degree of the first univariate-skip message polynomial:
/// `2 * (2^K - 1)` for [`U32_MUL_UNIVARIATE_SKIP_VARS`] `K`.
pub const U32_MUL_UNIVARIATE_SKIP_DEGREE: u32 = 2 * ((1_u32 << U32_MUL_UNIVARIATE_SKIP_VARS) - 1);

const U32_MUL_PRIME_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-u32-mul/runtime-prime/v2";
const U32_MUL_BINDING_DOMAIN: &[u8] =
    b"f2z/spartan-u32-mul/assignment/v3-runtime-skip-k3-profiled-ligerito";

enum U32MulInitialGrinding {}

impl GrindingDomain for U32MulInitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u32-mul/grinding/initial/v1";
}

enum U32MulTerminalGrinding {}

impl GrindingDomain for U32MulTerminalGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u32-mul/grinding/terminal/v1";
}

/// The per-challenge PIOP grinding domain of the u32 protocol: at a
/// nonzero difficulty, EVERY challenge the Spartan PIOP draws (τ
/// coordinates, outer/bind/inner round challenges) is preceded by one
/// boundary here. The difficulty is the profile's initial bound
/// `ceil(λ + log2(arity) − (b−1))` — the maximum any single draw needs —
/// so this is conservative but uniformly round-by-round sound, unlike the
/// SHA path's per-boundary-tuned schedule.
enum U32MulPiopGrinding {}

impl GrindingDomain for U32MulPiopGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u32-mul/grinding/piop/v1";
}

/// The public statement facts the security-profile derivation consumes for
/// a u32 multiplication batch: per-row integer defects `|x*y - z|` are
/// below `2^65` (bounded to `2^80` conservatively, far below any sampled
/// prime), the Step-5.1 lift sums `2^t` terms, and the opening is the
/// DIRECT exponent-fold path, so the interval width is capped at
/// `c_w = 127 - t - W` and the row functional always fits ONE chunk.
pub(crate) fn u32_mul_instance_facts(
    p: &crate::pcs::IntEvalParams,
    row_vars: usize,
) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: 80,
        lift_arity_log2: p.t as u32,
        opening_t: p.t as u32,
        opening_word_bits: p.word_bits as u32,
        direct_opening: true,
        tau_arity: row_vars.max(1) as u32,
        piop_degree: U32_MUL_UNIVARIATE_SKIP_DEGREE,
        step50_magnitude_log2: 0,
    }
}

/// Setup-once, prime-independent bundle for the u32 protocol: the exact
/// Boolean constraint matrices with their prime-independent preparation
/// (skeleton digest, padded widths, selector layout — instantiated per
/// transcript draw), the layout, the instantiated security profile, and its
/// validator-gated Ligerito prover/verifier configuration.
pub struct PreparedU32MulRelation {
    skeleton: ConstraintMatricesSkeleton<SpartanF2zField, bool>,
    layout: U32MulLayout,
    security: IopSecurityParams,
    ligerito: U32MulLigerito,
    ligerito_pc: LigProverConfig,
    ligerito_vc: LigVerifierConfig,
}

/// Public, setup-once context for benchmarking only the terminal F2Z opening
/// of a u32 multiplication assignment at the fixed comparison field.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub struct PreparedU32TerminalF2zOpening {
    layout: U32MulLayout,
    params: crate::pcs::IntEvalParams,
    security: IopSecurityParams,
    ligerito_pc: LigProverConfig,
    ligerito_vc: LigVerifierConfig,
    assignment_binding: [u8; 32],
    relation_modulus_encoding: Box<[u8]>,
    relation_digest: [u8; 32],
}

impl PreparedU32MulRelation {
    /// Prepares the relation at the default [`Lambda100`] profile.
    pub fn new(layout: U32MulLayout) -> Result<Self, SpartanF2zError> {
        Self::new_with_profile::<Lambda100>(layout)
    }

    /// Prepares the relation under an explicit single-prime profile with the
    /// relation's default opener ([`U32MulLigerito::ValidatedUdr`]).
    pub fn new_with_profile<P: IopSecurityProfile>(
        layout: U32MulLayout,
    ) -> Result<Self, SpartanF2zError> {
        Self::new_with_profile_and_ligerito::<P>(layout, U32MulLigerito::ValidatedUdr)
    }

    /// Prepares the relation under an explicit single-prime profile and an
    /// explicit Ligerito opener geometry.
    pub fn new_with_profile_and_ligerito<P: IopSecurityProfile>(
        layout: U32MulLayout,
        ligerito: U32MulLigerito,
    ) -> Result<Self, SpartanF2zError> {
        validate_layout_geometry(&layout)?;
        let p = layout.f2z_params();
        let row_vars = layout.gate_vars();
        let security = P::instantiate(&u32_mul_instance_facts(&p, row_vars))?;
        if security.projection_full_width || security.reduction.is_some() {
            return Err(SpartanF2zError::UnsupportedProfile);
        }
        let (ligerito_pc, ligerito_vc) =
            configs_for_layout_and_target(&layout, security.ligerito_target_bits, ligerito)?;
        validate_config_pair(&p, &ligerito_pc, &ligerito_vc)?;
        let raw = u32_mul_constraint_matrices(&layout, true)?;
        let skeleton = ConstraintMatricesSkeleton::new(raw).map_err(SpartanError::from)?;
        Ok(Self {
            skeleton,
            layout,
            security,
            ligerito,
            ligerito_pc,
            ligerito_vc,
        })
    }

    /// The opener geometry this relation was prepared with.
    pub const fn ligerito(&self) -> U32MulLigerito {
        self.ligerito
    }

    /// Statement-bound layout.
    pub const fn layout(&self) -> &U32MulLayout {
        &self.layout
    }

    /// F2Z geometry of the committed bit tensor.
    pub fn params(&self) -> crate::pcs::IntEvalParams {
        self.layout.f2z_params()
    }

    /// The instantiated security parameters and their accounting.
    pub const fn security(&self) -> &IopSecurityParams {
        &self.security
    }
}

/// Prepares a fixed-q, PCS-only terminal-opening context. Relation projection,
/// profile derivation, and statement binding are excluded from all trial
/// timers; the returned context retains only their public digests and configs.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn prepare_u32_terminal_f2z_opening(
    prepared: &PreparedU32MulRelation,
    commitment: &Commitment,
) -> Result<PreparedU32TerminalF2zOpening, SpartanF2zError> {
    let config = spartan_f2z_field_config();
    let matrices = PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
        &prepared.skeleton,
        &config,
    )
    .map_err(SpartanError::from)?;
    let params = prepared.layout.f2z_params();
    validate_commitment(&params, commitment, &prepared.ligerito_pc)?;
    let assignment_binding = u32_mul_assignment_binding(
        &prepared.layout,
        commitment,
        &prepared.security,
        &prepared.ligerito_pc,
    )?;
    Ok(PreparedU32TerminalF2zOpening {
        layout: prepared.layout,
        params,
        security: prepared.security.clone(),
        ligerito_pc: prepared.ligerito_pc.clone(),
        ligerito_vc: prepared.ligerito_vc.clone(),
        assignment_binding,
        relation_modulus_encoding: matrices.field_modulus_encoding().into(),
        relation_digest: *matrices.digest(),
    })
}

/// Commits already-materialized 32/32/64 compact bit rows with the exact
/// configuration retained by the PCS-only context.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn commit_u32_terminal_f2z_witness(
    prepared: &PreparedU32TerminalF2zOpening,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, SpartanF2zError> {
    validate_bit_rows(&prepared.params, &rows)?;
    let hint = commit_rs_ligerito_rows(&prepared.params, rows, &prepared.ligerito_pc);
    validate_u32_prepared_opening_commitment(prepared, &hint.commitment)?;
    Ok(hint)
}

/// Proves one already-derived terminal assignment-MLE claim, with all Spartan
/// work deliberately outside the benchmark boundary.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn prove_u32_terminal_claim_f2z<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU32TerminalF2zOpening,
    hint: &FlockCommitHint,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
) -> Result<IntEvalRsLigModQProof, SpartanF2zError> {
    validate_u32_prepared_opening_commitment(prepared, &hint.commitment)?;
    validate_bit_rows(&prepared.params, hint.rows())?;
    let arith = ProjArith::new(FQ_MOD);
    let opening = bitify_u32_mul_spartan_claim(terminal_claim, &prepared.layout, FQ_MOD, &arith)?;
    let bridge_digest = bitified_claim_digest_from_relation(
        &prepared.relation_modulus_encoding,
        &prepared.relation_digest,
        &prepared.assignment_binding,
        &prepared.layout,
        terminal_claim,
        &opening,
        FQ_MOD,
    )?;
    let chunks = prepare_u32_bitified_chunks(&opening, FQ_BITS, &arith)?;
    if chunks.len() != 1 {
        return Err(SpartanF2zError::MultiChunkRuntimeWeights);
    }
    prove_mle_eval_mod_q_ligerito_with_weight_chunks(
        transcript,
        ModQOpeningKind::U32Mul,
        hint,
        &prepared.params,
        &chunks,
        &bridge_digest,
        FQ_BITS,
        f2z_generator(),
        f2z_round_grinding_bits(&prepared.security),
        &prepared.ligerito_pc,
    )
    .map_err(SpartanF2zError::F2z)
}

/// Verifies the PCS-only u32 terminal opening from public data alone.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn verify_u32_terminal_claim_f2z<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU32TerminalF2zOpening,
    commitment: &Commitment,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    proof: &IntEvalRsLigModQProof,
) -> Result<(), SpartanF2zError> {
    validate_u32_prepared_opening_commitment(prepared, commitment)?;
    let arith = ProjArith::new(FQ_MOD);
    let opening = bitify_u32_mul_spartan_claim(terminal_claim, &prepared.layout, FQ_MOD, &arith)?;
    let bridge_digest = bitified_claim_digest_from_relation(
        &prepared.relation_modulus_encoding,
        &prepared.relation_digest,
        &prepared.assignment_binding,
        &prepared.layout,
        terminal_claim,
        &opening,
        FQ_MOD,
    )?;
    let prepared_claim = prepare_u32_bitified_claim(&opening, FQ_BITS, &arith)?;
    if prepared_claim.chunks.len() != 1 {
        return Err(SpartanF2zError::MultiChunkRuntimeWeights);
    }
    verify_mle_eval_mod_q_ligerito_with_weight_chunks(
        transcript,
        ModQOpeningKind::U32Mul,
        commitment,
        proof,
        &prepared.params,
        &prepared_claim.chunks,
        &prepared_claim.col_weights,
        &bridge_digest,
        f2z_generator(),
        prepared_claim.claimed,
        FQ_BITS,
        f2z_round_grinding_bits(&prepared.security),
        &prepared.ligerito_vc,
    )
    .map_err(SpartanF2zError::F2z)
}

/// Canonical standalone F2Z opening payload bytes.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub fn u32_terminal_claim_f2z_proof_bytes(proof: &IntEvalRsLigModQProof) -> Vec<u8> {
    proof.to_bytes()
}

/// A u32 multiplication proof over a transcript-selected prime.
///
/// The Spartan outer reduction uses the canonical known-zero univariate
/// prefix, followed by the runtime-`q` F2Z opening and any profile-selected
/// grinding nonces.
#[derive(Clone)]
pub struct U32MulProof {
    initial_nonce: u64,
    terminal_nonce: u64,
    piop_nonces: Vec<u64>,
    spartan: UnivariateSkipSpartanPiopProof<SpartanF2zField>,
    f2z: IntEvalRsLigModQProof,
}

impl U32MulProof {
    /// Spartan univariate-prefix, outer-tail, and inner sumcheck proofs.
    pub const fn spartan(&self) -> &UnivariateSkipSpartanPiopProof<SpartanF2zField> {
        &self.spartan
    }

    /// Runtime-prime F2Z opening proof.
    pub const fn f2z(&self) -> &IntEvalRsLigModQProof {
        &self.f2z
    }

    /// Field elements in the Spartan payload, for analytic size accounting.
    pub fn spartan_payload_elements(&self) -> usize {
        self.spartan.outer.skip.finite_q_evaluations.len()
            + 1
            + 4 * self.spartan.outer.tail.sumcheck.round_polynomials.len()
            + 3
            + 3 * self.spartan.inner.round_polynomials.len()
    }

    /// Transmitted grinding nonces (initial + terminal boundaries when the
    /// profile arms them, plus the forest section inside the F2Z stream).
    pub fn grinding_nonce_count(&self, security: &IopSecurityParams) -> usize {
        usize::from(security.initial_grinding_bits > 0)
            + usize::from(security.terminal_grinding_bits > 0)
            + self.piop_nonces.len()
            + self.f2z.grinding_nonces.len()
    }
}

fn grind_boundary_u32<D: GrindingDomain, T: Transcript>(
    transcript: &mut T,
    bits: u32,
) -> Result<u64, GrindingError> {
    if bits == 0 {
        return Ok(0);
    }
    grind_and_absorb::<D, _>(transcript, GrindingRound::new(0), bits)
}

fn check_boundary_u32<D: GrindingDomain, T: Transcript>(
    transcript: &mut T,
    bits: u32,
    nonce: u64,
) -> Result<(), GrindingError> {
    if bits == 0 {
        if nonce != 0 {
            return Err(GrindingError::InvalidNonce { nonce, bits });
        }
        return Ok(());
    }
    verify_and_absorb::<D, _>(transcript, GrindingRound::new(0), bits, nonce)
}

/// Uniform per-draw PIOP grinding difficulty: the maximum requirement of
/// any single drawn challenge. This covers both the degree-14 skip challenge
/// and every cubic tail/inner challenge.
fn piop_wrap_bits(security: &IopSecurityParams) -> u32 {
    security
        .initial_grinding_bits
        .max(security.piop_round_grinding_bits)
}

/// Uniform difficulty for the F2Z opening's wrapped challenges. The shared
/// bridge transcript covers both the forest/GKR draws and the ring-switch
/// draw, so it must satisfy the stronger of their two profile requirements.
fn f2z_round_grinding_bits(security: &IopSecurityParams) -> u32 {
    security
        .forest_round_grinding_bits
        .max(security.ring_switch_grinding_bits)
}

/// Samples the Step-2 prime from the profile interval and builds its
/// runtime field configuration and canonical arithmetic.
fn sample_u32_mul_mod_q(
    transcript: &mut impl Transcript,
    security: &IopSecurityParams,
) -> Result<
    (
        u128,
        usize,
        <SpartanF2zField as PrimeField>::Config,
        ProjArith,
    ),
    SpartanF2zError,
> {
    absorb_spartan_message(transcript, b"prime-domain", U32_MUL_PRIME_SAMPLING_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"prime-min",
        &security.projection_min.to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"prime-max",
        &security.projection_max.to_le_bytes(),
    );
    let q = sample_prime_in_interval(transcript, security.projection_min, security.projection_max)?;
    absorb_spartan_message(transcript, b"prime-q", &q.to_le_bytes());
    let config = SpartanF2zField::make_cfg(&Uint::from(q))
        .map_err(|_| SpartanF2zError::UnsupportedFieldModulus)?;
    SpartanF2zField::validate_config(&config)
        .map_err(|_| SpartanF2zError::UnsupportedFieldModulus)?;
    let q_bits = (u128::BITS - q.leading_zeros()) as usize;
    Ok((q, q_bits, config, ProjArith::new(q)))
}

/// Digest binding the layout, commitment, and the profile's public
/// interval/grinding parameters — everything fixed BEFORE the prime draw.
fn u32_mul_assignment_binding(
    layout: &U32MulLayout,
    commitment: &Commitment,
    security: &IopSecurityParams,
    ligerito_config: &LigProverConfig,
) -> Result<[u8; 32], SpartanF2zError> {
    let p = layout.f2z_params();
    let params = &commitment.params;
    let mut hasher = Hasher::new();
    hasher.update(U32_MUL_BINDING_DOMAIN);
    hasher.update(&commitment.root);
    hash_usize(&mut hasher, params.m)?;
    hash_usize(&mut hasher, params.log_inv_rate)?;
    hash_usize(&mut hasher, params.log_batch_size)?;
    hasher.update(&[profile_code(params.profile)]);
    hasher.update(&[hash_code(params.merkle_hash)]);
    hash_usize(&mut hasher, security.profile_name.len())?;
    hasher.update(security.profile_name.as_bytes());
    hash_usize(&mut hasher, security.lambda as usize)?;
    hasher.update(&security.projection_min.to_le_bytes());
    hasher.update(&security.projection_max.to_le_bytes());
    hasher.update(&[u8::from(security.projection_full_width)]);
    hash_usize(&mut hasher, security.initial_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.piop_round_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.terminal_grinding_bits as usize)?;
    match security.reduction {
        Some(reduction) => {
            hasher.update(&[1]);
            hasher.update(&reduction.min.to_le_bytes());
            hasher.update(&reduction.max.to_le_bytes());
            hash_usize(&mut hasher, reduction.grinding_bits as usize)?;
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hash_usize(&mut hasher, security.forest_round_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.ring_switch_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.ligerito_target_bits)?;
    hash_ligerito_config(&mut hasher, ligerito_config)?;
    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, p.t)?;
    hash_usize(&mut hasher, p.s)?;
    hash_usize(&mut hasher, p.word_bits)?;
    hash_usize(&mut hasher, U32_MUL_UNIVARIATE_SKIP_VARS)?;
    hash_usize(&mut hasher, U32_MUL_UNIVARIATE_SKIP_DEGREE as usize)?;
    Ok(*hasher.finalize().as_bytes())
}

fn hash_ligerito_config(
    hasher: &mut Hasher,
    config: &LigProverConfig,
) -> Result<(), SpartanF2zError> {
    for value in [
        config.recursive_steps,
        config.initial_log_msg_cols,
        config.initial_log_num_interleaved,
        config.initial_k,
    ] {
        hash_usize(hasher, value)?;
    }
    for values in [
        config.log_inv_rates.as_slice(),
        config.recursive_log_msg_cols.as_slice(),
        config.recursive_ks.as_slice(),
        config.queries.as_slice(),
        config.grinding_bits.as_slice(),
        config.fold_grinding_bits.as_slice(),
        config.ood_samples.as_slice(),
    ] {
        hash_usize(hasher, values.len())?;
        for &value in values {
            hash_usize(hasher, value)?;
        }
    }
    hasher.update(&[hash_code(config.merkle_hash)]);
    Ok(())
}

/// Proves the u32 batch under the prepared relation's security profile:
/// commit-before-prime, a transcript-sampled Step-2 prime, the native u64
/// Spartan PIOP over that runtime field, bitification, and the runtime-q
/// F2Z opening. The Spartan outer reduction always uses the canonical
/// univariate-prefix skip and delayed-Barrett arithmetic.
pub fn prove_u32_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU32MulRelation,
    witness: &U32MulWitness,
    hint: &FlockCommitHint,
) -> Result<U32MulProof, SpartanF2zError> {
    let layout = prepared.layout();
    if witness.layout() != layout {
        return Err(SpartanF2zError::RelationWitnessLayoutMismatch);
    }
    let p = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&p, pc, vc)?;
    validate_bit_rows(&p, hint.rows())?;
    validate_commitment(&p, &hint.commitment, pc)?;
    let security = prepared.security();

    let binding = u32_mul_assignment_binding(layout, &hint.commitment, security, pc)?;
    absorb_spartan_message(transcript, b"u32-statement", &binding);

    // Step 2: pre-draw grinding, prime sample, and relation projection into
    // the runtime field.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce =
        grind_boundary_u32::<U32MulInitialGrinding, _>(transcript, security.initial_grinding_bits)?;
    let (q, q_bits, config, arith) = sample_u32_mul_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("spartan-f2z:relation_projection_prove");
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    // Step 3: the native u64 Spartan PIOP over F_q, every drawn challenge
    // preceded by one PIOP grinding boundary at the profile's difficulty
    // (a transparent pass-through at λ = 100).
    let (spartan, terminal_claim, piop_nonces) = {
        let _step3 = crate::utils::prof::scope("step3:piop_prove");
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
        // The exact products are the zero-padded operand blocks of the
        // witness and the assignment is its block table: lend both, no copy.
        let product_len = layout.multiplications().next_power_of_two();
        let products = NativeProducts {
            az: &witness.x_values()[..product_len],
            bz: &witness.y_values()[..product_len],
            cz: &witness.product_values()[..product_len],
        };
        let mut grinder: crate::piop::spartan::grinding::ProverGrindingTranscript<
            _,
            U32MulPiopGrinding,
        > = crate::piop::spartan::grinding::ProverGrindingTranscript::new(
            transcript,
            piop_wrap_bits(security),
        );
        let (spartan, terminal_claim) =
            prove_spartan_piop_u32_native_with_univariate_skip_borrowed(
                &mut grinder,
                &matrices,
                &binding,
                products,
                witness.assignment(),
                U32_MUL_UNIVARIATE_SKIP_VARS,
            )?;
        (spartan, terminal_claim, grinder.finish())
    };

    // Step 4: bitification at the runtime prime, plus the terminal
    // boundary protecting the opening challenges.
    let step4_scope = crate::utils::prof::scope("step4:bitify_prove");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_prover");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    let terminal_nonce = grind_boundary_u32::<U32MulTerminalGrinding, _>(
        transcript,
        security.terminal_grinding_bits,
    )?;
    drop(step4_scope);

    // Steps 5.1–5.3: the runtime-q F2Z opening (one chunk by construction).
    let f2z = {
        let _step5 = crate::utils::prof::scope("step5:open_prove");
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prove");
        let chunks = {
            let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prepare_prover");
            prepare_u32_bitified_chunks(&opening, q_bits, &arith)?
        };
        if chunks.len() != 1 {
            return Err(SpartanF2zError::MultiChunkRuntimeWeights);
        }
        prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            ModQOpeningKind::U32Mul,
            hint,
            &p,
            &chunks,
            &bridge_digest,
            q_bits,
            f2z_generator(),
            f2z_round_grinding_bits(security),
            pc,
        )
        .map_err(SpartanF2zError::F2z)?
    };

    Ok(U32MulProof {
        initial_nonce,
        terminal_nonce,
        piop_nonces,
        spartan,
        f2z,
    })
}

/// Verifies a u32 multiplication proof, re-deriving the prime from the bound
/// transcript and enforcing the canonical univariate-prefix width.
pub fn verify_u32_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU32MulRelation,
    commitment: &Commitment,
    proof: &U32MulProof,
) -> Result<(), SpartanF2zError> {
    let layout = prepared.layout();
    let p = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&p, pc, vc)?;
    validate_commitment(&p, commitment, pc)?;
    let security = prepared.security();

    let actual_skip_vars = proof.spartan.outer.skip.skip_vars;
    let expected_skip_vars = U32_MUL_UNIVARIATE_SKIP_VARS as u8;
    if actual_skip_vars != expected_skip_vars {
        return Err(SpartanF2zError::UnexpectedUnivariateSkipVariables {
            expected: expected_skip_vars,
            actual: actual_skip_vars,
        });
    }

    let binding = u32_mul_assignment_binding(layout, commitment, security, pc)?;
    absorb_spartan_message(transcript, b"u32-statement", &binding);

    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    check_boundary_u32::<U32MulInitialGrinding, _>(
        transcript,
        security.initial_grinding_bits,
        proof.initial_nonce,
    )?;
    let (q, q_bits, config, arith) = sample_u32_mul_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("spartan-f2z:relation_projection_verify");
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    let terminal_claim = {
        let _step3 = crate::utils::prof::scope("step3:piop_verify");
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_verify");
        let mut grinder: crate::piop::spartan::grinding::VerifierGrindingTranscript<
            _,
            U32MulPiopGrinding,
        > = crate::piop::spartan::grinding::VerifierGrindingTranscript::new(
            transcript,
            piop_wrap_bits(security),
            &proof.piop_nonces,
        );
        let terminal_claim = verify_spartan_univariate_skip_proof(
            &mut grinder,
            &matrices,
            &binding,
            &proof.spartan,
        )?;
        grinder.finish()?;
        terminal_claim
    };

    let step4_scope = crate::utils::prof::scope("step4:bitify_verify");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_verifier");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    check_boundary_u32::<U32MulTerminalGrinding, _>(
        transcript,
        security.terminal_grinding_bits,
        proof.terminal_nonce,
    )?;
    drop(step4_scope);

    let _step5 = crate::utils::prof::scope("step5:open_verify");
    let _scope = crate::utils::prof::scope("spartan-f2z:f2z_verify");
    let prepared_claim = {
        let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prepare_verifier");
        prepare_u32_bitified_claim(&opening, q_bits, &arith)?
    };
    if prepared_claim.chunks.len() != 1 {
        return Err(SpartanF2zError::MultiChunkRuntimeWeights);
    }
    let col_weights_q: Vec<u128> = prepared_claim
        .col_weights
        .iter()
        .map(|weight| weight.0)
        .collect();
    crate::ligerito_flock::verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
        transcript,
        ModQOpeningKind::U32Mul,
        commitment,
        &proof.f2z,
        &p,
        &prepared_claim.chunks,
        &col_weights_q,
        &bridge_digest,
        f2z_generator(),
        prepared_claim.claimed.0,
        q,
        q_bits,
        f2z_round_grinding_bits(security),
        vc,
    )
    .map_err(SpartanF2zError::F2z)
}

#[cfg(test)]
mod tests {
    use crypto_primitives::FromWithConfig;

    #[test]
    fn u32_mul_roundtrips_and_is_deterministic() {
        use super::super::profile::{Limber114, ProfileError};
        use super::super::u32_mul::U32MulF2zWidth;

        // Hold the shared env lock so tests that toggle transcript-shaping
        // `F2Z_*` variables cannot flip them between our prove and verify.
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let multiplications = 1usize << 15;
        let witness =
            U32MulWitness::from_fn_with_f2z_width(multiplications, U32MulF2zWidth::W1, |i| {
                let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
                let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
                (x, y)
            })
            .unwrap();
        let layout = *witness.layout();
        let prepared = PreparedU32MulRelation::new(layout).unwrap();

        // The derived interval keeps one exponent-fold chunk and no grinding.
        let security = prepared.security();
        assert_eq!(security.lambda, 100);
        assert_eq!(security.initial_grinding_bits, 0);
        assert_eq!(security.piop_round_grinding_bits, 0);
        assert_eq!(security.forest_round_grinding_bits, 0);
        assert_eq!(
            u32_mul_instance_facts(&layout.f2z_params(), layout.gate_vars()).piop_degree,
            U32_MUL_UNIVARIATE_SKIP_DEGREE
        );
        let p = layout.f2z_params();
        let width = (128 - security.projection_max.leading_zeros()) as usize;
        assert!(width <= 127 - p.t - p.word_bits, "q_bits <= c_w");
        assert_eq!(
            crate::pcs::mod_q_num_chunks(&p, width),
            1,
            "the runtime interval is one-chunk by construction"
        );

        let hint = commit_u32_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();
        let mut prover_transcript = crate::transcript::Blake3Transcript::new();
        let proof = prove_u32_mul(&mut prover_transcript, &prepared, &witness, &hint).unwrap();
        assert_eq!(
            proof.spartan().outer.skip.skip_vars,
            U32_MUL_UNIVARIATE_SKIP_VARS as u8
        );
        assert_eq!(proof.spartan().outer.skip.finite_q_evaluations.len(), 6);
        assert_eq!(
            proof.spartan().outer.tail.sumcheck.round_polynomials.len(),
            12
        );
        assert_eq!(proof.spartan().inner.round_polynomials.len(), 17);
        assert_eq!(proof.spartan_payload_elements(), 109);
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        verify_u32_mul(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
        )
        .unwrap();

        // Determinism: a second prove is byte-identical.
        let mut second_transcript = crate::transcript::Blake3Transcript::new();
        let second = prove_u32_mul(&mut second_transcript, &prepared, &witness, &hint).unwrap();
        assert_eq!(second.f2z().to_bytes(), proof.f2z().to_bytes());
        assert_eq!(second.spartan(), proof.spartan());

        // Tampering with the Spartan claim is rejected.
        let mut tampered = proof.clone();
        let one =
            SpartanF2zField::from_with_cfg(1u64, tampered.spartan.outer.tail.az_mle_claim.cfg());
        tampered.spartan.outer.tail.az_mle_claim += &one;
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        assert!(
            verify_u32_mul(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &tampered
            )
            .is_err()
        );

        let mut tampered_skip = proof.clone();
        tampered_skip.spartan.outer.skip.finite_q_evaluations[0] += &one;
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        assert!(
            verify_u32_mul(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &tampered_skip
            )
            .is_err()
        );

        let mut tampered_infinity = proof.clone();
        tampered_infinity.spartan.outer.skip.q_at_infinity += &one;
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        assert!(
            verify_u32_mul(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &tampered_infinity
            )
            .is_err()
        );

        let mut tampered_tail = proof.clone();
        tampered_tail.spartan.outer.tail.sumcheck.round_polynomials[0][0] += &one;
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        assert!(
            verify_u32_mul(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &tampered_tail
            )
            .is_err()
        );

        // The verifier must not accept a proof-selected skip width under the
        // fixed K=3 security profile, even though the generic PIOP supports
        // K=1 through K=4.
        for actual in [2, 4] {
            let mut wrong_k = proof.clone();
            wrong_k.spartan.outer.skip.skip_vars = actual;
            let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
            assert!(matches!(
                verify_u32_mul(
                    &mut verifier_transcript,
                    &prepared,
                    &hint.commitment,
                    &wrong_k
                ),
                Err(SpartanF2zError::UnexpectedUnivariateSkipVariables {
                    expected: 3,
                    actual: rejected
                }) if rejected == actual
            ));
            assert_eq!(
                verifier_transcript.get_challenge::<u128>(),
                crate::transcript::Blake3Transcript::new().get_challenge::<u128>(),
                "wrong K must be rejected before transcript mutation"
            );
        }

        // Strategy-2 profiles are rejected up front.
        assert!(matches!(
            PreparedU32MulRelation::new_with_profile::<Limber114>(layout),
            Err(SpartanF2zError::UnsupportedProfile)
                | Err(SpartanF2zError::Profile(
                    ProfileError::GrindingTooExpensive { .. }
                ))
        ));

        // λ = 128 is genuine on this path too: the initial boundary, every
        // PIOP draw, and the forest rounds all carry proof-of-work.
        let prepared128 =
            PreparedU32MulRelation::new_with_profile::<super::super::profile::Lambda128>(layout)
                .unwrap();
        assert_eq!(prepared128.security().initial_grinding_bits, 22);
        assert_eq!(prepared128.security().terminal_grinding_bits, 22);
        assert_eq!(prepared128.security().forest_round_grinding_bits, 2);
        assert_eq!(prepared128.security().ring_switch_grinding_bits, 1);
        assert_eq!(f2z_round_grinding_bits(prepared128.security()), 2);
        assert_ne!(
            (
                &prepared.ligerito_pc.queries,
                &prepared.ligerito_pc.fold_grinding_bits,
            ),
            (
                &prepared128.ligerito_pc.queries,
                &prepared128.ligerito_pc.fold_grinding_bits,
            ),
            "the Ligerito configuration must follow the profile target"
        );
        let hint128 = commit_u32_mul_witness(&prepared128, witness.f2z_bit_rows()).unwrap();
        let mut prover_transcript = crate::transcript::Blake3Transcript::new();
        let proof128 =
            prove_u32_mul(&mut prover_transcript, &prepared128, &witness, &hint128).unwrap();
        assert_eq!(prepared128.security().piop_round_grinding_bits, 22);
        assert_eq!(proof128.piop_nonces.len(), 43);
        assert!(!proof128.f2z().grinding_nonces.is_empty());
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        verify_u32_mul(
            &mut verifier_transcript,
            &prepared128,
            &hint128.commitment,
            &proof128,
        )
        .unwrap();
        let mut tampered = proof128.clone();
        tampered.piop_nonces[3] ^= 1;
        let mut verifier_transcript = crate::transcript::Blake3Transcript::new();
        assert!(
            verify_u32_mul(
                &mut verifier_transcript,
                &prepared128,
                &hint128.commitment,
                &tampered
            )
            .is_err()
        );
    }

    use super::*;
    use crate::pcs::{FQ_BITS, eq_le_table_fq, fq_sub};
    use crate::piop::spartan::u32_mul::{U32MulF2zWidth, U32MulWitness};
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

    fn bitify_test_claim(
        claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
        layout: &U32MulLayout,
    ) -> Result<U32BitifiedClaim, SpartanF2zError> {
        bitify_u32_mul_spartan_claim(claim, layout, FQ_MOD, &ProjArith::new(FQ_MOD))
    }

    fn prepare_test_claim(
        opening: &U32BitifiedClaim,
    ) -> Result<PreparedU32BitifiedClaim, SpartanF2zError> {
        prepare_u32_bitified_claim(opening, FQ_BITS, &ProjArith::new(FQ_MOD))
    }

    #[test]
    fn fast_bitify_field_helpers_match_reference_arithmetic() {
        let point = [Fq(0), Fq(1), Fq(FQ_MOD - 1), Fq(123_456_789)];
        assert_eq!(
            eq_le_table_fq_fast_with(&point, &ProjArith::new(FQ_MOD)).unwrap(),
            eq_le_table_fq(&point)
        );
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
            let opening = bitify_test_claim(&terminal, layout).unwrap();
            let prepared = prepare_test_claim(&opening).unwrap();

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
            assert_eq!(read_off, opening.claimed);
        }
    }

    #[test]
    fn zero_scale_keeps_a_nonzero_row_functional() {
        let layout = U32MulLayout::new(3).unwrap();
        let mut point = (0..layout.gate_vars())
            .map(|coordinate| Fq((coordinate + 2) as u128))
            .collect::<Vec<_>>();
        point.extend([Fq(7), Fq(11)]);
        let opening = bitify_test_claim(&terminal_claim(&point, Fq(0), Fq(0)), &layout).unwrap();
        let prepared = prepare_test_claim(&opening).unwrap();

        assert!((0..opening.params.rows()).any(|row| prepared_row_weight(&prepared, row) != 0));
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed, Fq(0));
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

        let opening = bitify_test_claim(
            &terminal_claim(&point, scale, scale * constant_evaluation),
            &layout,
        )
        .unwrap();
        let prepared = prepare_test_claim(&opening).unwrap();
        assert_eq!(prepared_row_weight(&prepared, 0), 1);
        assert!((1..opening.params.rows()).all(|row| prepared_row_weight(&prepared, row) == 0));
        assert!(prepared.col_weights.iter().all(|&weight| weight == Fq(0)));
        assert_eq!(opening.claimed, Fq(0));
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
            bitify_test_claim(&claim, &layout),
            Err(SpartanF2zError::ClaimFieldMismatch)
        ));
    }

    #[test]
    fn combined_protocol_uses_only_validator_gated_production_profiles() {
        let small = U32MulLayout::new(3).unwrap();
        assert!(matches!(
            PreparedU32MulRelation::new(small),
            Err(SpartanF2zError::UnauditedF2zParameters)
        ));

        for width in [U32MulF2zWidth::W1, U32MulF2zWidth::W8] {
            let production =
                U32MulLayout::new_with_f2z_width(1 << MIN_PRODUCTION_GATE_VARS, width).unwrap();
            PreparedU32MulRelation::new(production)
                .expect("the smallest validated profile is available");

            let largest = U32MulLayout::new_with_f2z_width(1 << 25, width).unwrap();
            for target in [100, 128] {
                configs_for_layout_and_target(&largest, target, U32MulLigerito::ValidatedUdr)
                    .expect("the full advertised benchmark range has a validated config");
            }
            // The raw-performance table's Johnson opener at the 100-bit target.
            configs_for_layout_and_target(
                &largest,
                100,
                U32MulLigerito::CustomJohnson {
                    log_inv_rate: 3,
                    initial_k: 4,
                },
            )
            .expect("the custom Johnson geometry validates over the benchmark range");
        }
    }

    #[test]
    #[ignore = "runs one production-sized W=8 Spartan/F2Z proof"]
    fn u32_mul_w8_proof_verifies() {
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
        let prepared = PreparedU32MulRelation::new(layout).unwrap();
        let hint = commit_u32_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_u32_mul(&mut prover_transcript, &prepared, &witness, &hint).unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u32_mul(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
        )
        .unwrap();
    }
}
