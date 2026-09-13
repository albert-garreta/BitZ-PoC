//! Succinct opening of u128 multiplication assignments through F2Z.
//!
//! The integer R1CS assignment has four blocks, `[e0 | x | y | z]`, with
//! `x, y < 2^128` and `z < 2^256`, so the assignment MLE needs no padding
//! and the block selector is two coordinates. The commitment stores the
//! 512 bits of one multiplication per gate (`x`, `y`, then `z`). This
//! module applies the transpose of that reconstruction map — weights
//! `2^0 … 2^255` on the committed bits — to Spartan's terminal assignment
//! claim and opens the resulting compact-bit claim with F2Z, following the
//! paper protocol: commit-before-prime, a transcript-sampled Step-2 prime,
//! the Spartan PIOP over that runtime field on residues built straight from
//! the witness limbs (both the products and the assignment, since 128- and
//! 256-bit values have no native `u64` first round), runtime-q
//! bitification, and the runtime-q F2Z opening.

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
    ext_proj::{PrimeSamplingError, ProjArith, sample_prime_in_interval},
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, ModQOpeningKind,
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_with_weight_chunks,

        verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime,
    },
    pcs::{Fq, ProjectCanonicalU128},
    transcript::traits::{GenTranscribable, Transcript},
};

use crate::utils::{cfg_chunks_mut, cfg_iter, cfg_iter_mut};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    PreparedConstraintMatrices, SpartanField, absorb_spartan_message,
    f2z::{SpartanF2zField, f2z_generator, hash_code, profile_code},
    grinding::{
        GrindingDomain, GrindingError, GrindingRound, ProverGrindingTranscript,
        VerifierGrindingTranscript, grind_and_absorb, verify_and_absorb,
    },
    matrix::ScaledMleEvaluationClaim,
    piop::{
        SpartanError, SpartanPiopProof, prove_spartan_piop_raw_products_raw_witness,
        verify_spartan_proof,
    },
    profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100, ProfileError},
    raw_monty::{RawMontyCtx, RawProducts, RawWitness},
    u128_mul::{
        U128_MUL_ASSIGNMENT_BLOCKS, U128_MUL_BIT_SLOTS, U128_MUL_OPERAND_BITS,
        U128_MUL_PRODUCT_BITS, U128_MUL_SLOT_VARS, U128_MUL_X_SLOT_START, U128_MUL_Y_SLOT_START,
        U128_MUL_Z_SLOT_START, U128MulError, U128MulLayout, U128MulWitness,
        u128_mul_constraint_matrices,
    },
};

const PRIME_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-u128-mul/runtime-prime/v1";
const BINDING_DOMAIN: &[u8] = b"f2z/spartan-u128-f2z/assignment/v1-runtime";
/// Domain of the compact, factorized u128 claim bound between Spartan and F2Z.
const BITIFIED_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-u128-f2z/bitified-claim/v1";
/// Canonical order of the four assignment blocks.
const ASSIGNMENT_BLOCK_ORDER: &[u8] = b"e0|x|y|z";

/// Embedded, validator-gated Ligerito profiles begin at a 24-variable
/// committed bit MLE: nine slot variables plus fifteen gate variables.
const MIN_PRODUCTION_GATE_VARS: usize = 15;

enum InitialGrinding {}

impl GrindingDomain for InitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u128-mul/grinding/initial/v1";
}

enum TerminalGrinding {}

impl GrindingDomain for TerminalGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u128-mul/grinding/terminal/v1";
}

/// Per-challenge PIOP grinding domain: at a nonzero difficulty every
/// challenge the Spartan PIOP draws is preceded by one boundary here.
enum PiopGrinding {}

impl GrindingDomain for PiopGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u128-mul/grinding/piop/v1";
}

/// Compact result of applying the public 128/128/256-bit reconstruction map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U128MulBitifiedClaim {
    params: crate::pcs::IntegerMatrixLayout,
    gate_point: Box<[Fq]>,
    rows: U128MulBitifiedRows,
    col_scale: Fq,
    claimed: Fq,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum U128MulBitifiedRows {
    Structured { x: Fq, y: Fq, z: Fq },
    ConstantDummy,
}

impl U128MulBitifiedClaim {
    /// Public F2Z geometry selected by the statement-bound layout.
    pub const fn params(&self) -> crate::pcs::IntegerMatrixLayout {
        self.params
    }

    /// Claimed value after subtracting the public constant-block term.
    pub const fn claimed(&self) -> Fq {
        self.claimed
    }
}

struct PreparedU128MulBitifiedClaim {
    chunks: crate::pcs::ModQWeightChunks,
    col_weights: Vec<Fq>,
    claimed: Fq,
}

/// Failures in u128 layout validation, claim translation, or either proof
/// system.
#[derive(Debug, Error)]
pub enum U128MulSpartanF2zError {
    #[error(transparent)]
    Relation(#[from] U128MulError),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the runtime prime is not a supported Spartan field modulus")]
    UnsupportedFieldModulus,

    #[error("the witness does not match the prepared u128 multiplication layout")]
    RelationWitnessLayoutMismatch,

    #[error("the compact bit rows do not match the u128 multiplication layout")]
    InvalidBitRows,

    #[error("the F2Z parameters are invalid for the compact u128 multiplication layout")]
    InvalidF2zParameters,

    #[error("the combined Spartan/F2Z proof requires at least 2^15 multiplication slots")]
    UnauditedF2zParameters,

    #[error("the commitment parameters do not match the derived F2Z configuration")]
    CommitmentConfigMismatch,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element uses a field other than the runtime prime")]
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

    /// The paper path supports single-prime profiles only.
    #[error("the u128 paper path requires a single-prime security profile")]
    UnsupportedProfile,

    /// The derived prime interval must keep the row weights to one
    /// exponent-fold chunk; the profile guarantees this.
    #[error("the runtime prime produced a multi-chunk row functional")]
    MultiChunkRuntimeWeights,
}

/// Applies the adjoint of the public 128/128/256-bit reconstruction to a
/// terminal scaled assignment-MLE claim over the runtime modulus `q`.
///
/// Spartan's point is low-coordinate-first. Its final two coordinates
/// select the four assignment blocks; the preceding coordinates select a
/// gate. F2Z places the low gate coordinates on the clear column axis; the
/// high gate coordinates and the nine slot coordinates form the folded row
/// axis.
pub(crate) fn bitify_u128_mul_spartan_claim_with(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &U128MulLayout,
    q: u128,
    arith: &ProjArith,
) -> Result<U128MulBitifiedClaim, U128MulSpartanF2zError> {
    validate_layout_geometry(layout)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(2) {
        return Err(U128MulSpartanF2zError::InvalidClaimPoint);
    }

    let params = layout.f2z_params();
    let expected_modulus = Uint::from(q);
    let project = |value: &SpartanF2zField| -> Result<Fq, U128MulSpartanF2zError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(U128MulSpartanF2zError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= q {
            return Err(U128MulSpartanF2zError::ClaimFieldMismatch);
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

    // Little-endian block-selector order is 00=e0, 01=x, 10=y, 11=z.
    let constant_factor = mul(one_minus_low, one_minus_high);
    let x_factor = mul(block_low, one_minus_high);
    let y_factor = mul(one_minus_low, block_high);
    let z_factor = mul(block_low, block_high);

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point
        .iter()
        .copied()
        .fold(constant_factor, |acc, coordinate| {
            mul(acc, Fq(sub(one.0, coordinate.0)))
        });
    let adjusted_claim = Fq(sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

    let (rows, col_scale) = if [x_factor, y_factor, z_factor]
        .into_iter()
        .all(|factor| factor == Fq(0))
    {
        if adjusted_claim != Fq(0) {
            return Err(U128MulSpartanF2zError::InvalidConstantOnlyClaim);
        }
        (U128MulBitifiedRows::ConstantDummy, Fq(0))
    } else if scale == Fq(0) {
        (
            U128MulBitifiedRows::Structured {
                x: x_factor,
                y: y_factor,
                z: z_factor,
            },
            Fq(0),
        )
    } else {
        let (x, y, z) = if scale == one {
            (x_factor, y_factor, z_factor)
        } else {
            (
                mul(scale, x_factor),
                mul(scale, y_factor),
                mul(scale, z_factor),
            )
        };
        (U128MulBitifiedRows::Structured { x, y, z }, one)
    };

    Ok(U128MulBitifiedClaim {
        params,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

fn validate_layout_geometry(layout: &U128MulLayout) -> Result<(), U128MulSpartanF2zError> {
    let params = layout.f2z_params();
    if params.word_bits != 1
        || params.row_vars < LOG_PACKING
        || params.col_vars > layout.gate_vars()
        || params.row_vars.saturating_add(params.word_bits) > 126
    {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    let total_vars = params
        .row_vars
        .checked_add(params.col_vars)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(U128_MUL_SLOT_VARS)
            .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?
        || U128_MUL_BIT_SLOTS != 1_usize << U128_MUL_SLOT_VARS
        || U128_MUL_BIT_SLOTS != 2 * U128_MUL_OPERAND_BITS + U128_MUL_PRODUCT_BITS
        || layout.assignment_len() != U128_MUL_ASSIGNMENT_BLOCKS * layout.capacity()
    {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(params.row_vars)?;
    let col_count = checked_pow2(params.col_vars)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    let expected_cells = U128_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    // `packed_variables` counts one packed variable per 128 bits: the nine
    // slot variables leave two extra packed variables on top of the gates.
    if cells != expected_cells
        || packed_variables(&params)? != layout.gate_vars() + (U128_MUL_SLOT_VARS - LOG_PACKING)
    {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_bit_rows(
    params: &crate::pcs::IntegerMatrixLayout,
    rows: &[Vec<u64>],
) -> Result<(), U128MulSpartanF2zError> {
    let row_count = checked_pow2(params.row_vars)?;
    let col_count = checked_pow2(params.col_vars)?;
    if row_count % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(U128MulSpartanF2zError::InvalidBitRows);
    }
    let words_per_col = row_count / u64::BITS as usize;
    if rows.iter().any(|row| row.len() != words_per_col) {
        return Err(U128MulSpartanF2zError::InvalidBitRows);
    }
    Ok(())
}

fn validate_config_pair(
    params: &crate::pcs::IntegerMatrixLayout,
    pc: &LigProverConfig,
    vc: &LigVerifierConfig,
) -> Result<(), U128MulSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
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
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_commitment(
    params: &crate::pcs::IntegerMatrixLayout,
    commitment: &Commitment,
    pc: &LigProverConfig,
) -> Result<(), U128MulSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    };
    let commitment_params = &commitment.params;
    if commitment_params.m != m_p + LOG_PACKING
        || commitment_params.log_inv_rate != log_inv_rate
        || commitment_params.log_batch_size != pc.initial_k
        || commitment_params.profile != LigeritoProfile::default()
        || commitment_params.merkle_hash != pc.merkle_hash
    {
        return Err(U128MulSpartanF2zError::CommitmentConfigMismatch);
    }
    Ok(())
}

fn prepare_bitified_claim_with(
    opening: &U128MulBitifiedClaim,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<PreparedU128MulBitifiedClaim, U128MulSpartanF2zError> {
    let chunks = prepare_bitified_chunks_with(opening, q_bits, arith)?;
    let params = opening.params;
    let col_weights = if opening.col_scale == Fq(0) {
        vec![Fq(0); checked_pow2(params.col_vars)?]
    } else {
        let (gate_low, _) = opening.gate_point.split_at(params.col_vars);
        let mut eq_low = eq_le_table_fq_with(gate_low, arith)?;
        if opening.col_scale != Fq(1) {
            let factor = arith.monty_factor(opening.col_scale.0);
            cfg_iter_mut!(&mut eq_low, 256).for_each(|weight| {
                weight.0 = arith.mul_plain_by(weight.0, &factor);
            });
        }
        eq_low
    };
    Ok(PreparedU128MulBitifiedClaim {
        chunks,
        col_weights,
        claimed: opening.claimed,
    })
}

/// Compile the folded row functional directly into validated mod-q chunks.
fn prepare_bitified_chunks_with(
    opening: &U128MulBitifiedClaim,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<crate::pcs::ModQWeightChunks, U128MulSpartanF2zError> {
    let params = opening.params;
    let high_vars = params
        .row_vars
        .checked_sub(U128_MUL_SLOT_VARS)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    let gate_vars = params
        .col_vars
        .checked_add(high_vars)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    if params.word_bits != 1 || opening.gate_point.len() != gate_vars {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    let (_, gate_high) = opening.gate_point.split_at(params.col_vars);

    match opening.rows {
        U128MulBitifiedRows::ConstantDummy => {
            let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, q_bits)
                .map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)?;
            chunks
                .set_weight_range(0, &[1])
                .map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)?;
            Ok(chunks)
        }
        U128MulBitifiedRows::Structured { x, y, z } => {
            let weights = structured_row_weights(&params, gate_high, [x, y, z], arith)?;
            if crate::pcs::mod_q_num_chunks(&params, q_bits) == 1 {
                crate::pcs::ModQWeightChunks::from_single_chunk(&params, q_bits, weights)
                    .map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)
            } else {
                let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, q_bits)
                    .map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)?;
                chunks
                    .set_weight_range(0, &weights)
                    .map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)?;
                Ok(chunks)
            }
        }
    }
}

/// The dense canonical row weights of a structured opening, in F2Z row
/// order `(slot << h) | gate_high`:
///
/// `w[(slot << h) | g] = block_factor(slot) · 2^{slot − block_start} · eq(gate_high_point, g)`
///
/// for the `x`, `y`, and `z` blocks (the 512 slots are exactly the blocks).
/// The per-slot scalars are formed first, then every row is one
/// fixed-factor Montgomery multiplication of the shared `eq` table, in one
/// parallel pass over the whole `2^t` table.
fn structured_row_weights(
    params: &crate::pcs::IntegerMatrixLayout,
    gate_high: &[Fq],
    [x, y, z]: [Fq; 3],
    arith: &ProjArith,
) -> Result<Vec<u128>, U128MulSpartanF2zError> {
    let high_gate_count = checked_pow2(gate_high.len())?;
    let row_count = checked_pow2(params.row_vars)?;
    if U128_MUL_BIT_SLOTS
        .checked_mul(high_gate_count)
        .is_none_or(|rows| rows != row_count)
    {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }

    let two = arith.reduce(2);
    let mut slot_scalars = vec![0_u128; U128_MUL_BIT_SLOTS];
    for (slot_start, bit_count, block_factor) in [
        (U128_MUL_X_SLOT_START, U128_MUL_OPERAND_BITS, x),
        (U128_MUL_Y_SLOT_START, U128_MUL_OPERAND_BITS, y),
        (U128_MUL_Z_SLOT_START, U128_MUL_PRODUCT_BITS, z),
    ] {
        let mut scalar = arith.reduce(block_factor.0);
        for slot in slot_start..slot_start + bit_count {
            slot_scalars[slot] = scalar;
            scalar = arith.mul(scalar, two);
        }
    }

    let eq_high = eq_le_table_fq_with(gate_high, arith)?;
    let mut weights = vec![0_u128; row_count];
    cfg_chunks_mut!(weights, high_gate_count)
        .zip(cfg_iter!(slot_scalars))
        .for_each(|(rows, &scalar)| {
            let factor = arith.monty_factor(scalar);
            for (row, equality) in rows.iter_mut().zip(&eq_high) {
                *row = arith.mul_plain_by(equality.0, &factor);
            }
        });
    Ok(weights)
}

fn eq_le_table_fq_with(
    point: &[Fq],
    arith: &ProjArith,
) -> Result<Vec<Fq>, U128MulSpartanF2zError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);
    let q = arith.q();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
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

/// Digest binding the layout, commitment, and the profile's public
/// parameters — everything fixed BEFORE the prime draw.
fn assignment_binding(
    layout: &U128MulLayout,
    commitment: &Commitment,
    security: &IopSecurityParams,
) -> Result<[u8; 32], U128MulSpartanF2zError> {
    let f2z_params = layout.f2z_params();
    let commitment_params = &commitment.params;
    let mut hasher = Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(ASSIGNMENT_BLOCK_ORDER);
    hasher.update(&commitment.root);
    hash_usize(&mut hasher, commitment_params.m)?;
    hash_usize(&mut hasher, commitment_params.log_inv_rate)?;
    hash_usize(&mut hasher, commitment_params.log_batch_size)?;
    hasher.update(&[profile_code(commitment_params.profile)]);
    hasher.update(&[hash_code(commitment_params.merkle_hash)]);
    hasher.update(&security.projection_min.to_le_bytes());
    hasher.update(&security.projection_max.to_le_bytes());
    hash_usize(&mut hasher, security.lambda as usize)?;
    hash_usize(&mut hasher, security.ligerito_target_bits)?;
    hash_usize(&mut hasher, security.initial_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.piop_round_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.terminal_grinding_bits as usize)?;
    hash_usize(&mut hasher, security.forest_round_grinding_bits as usize)?;
    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, U128_MUL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, U128_MUL_OPERAND_BITS)?;
    hash_usize(&mut hasher, U128_MUL_PRODUCT_BITS)?;
    hash_usize(&mut hasher, U128_MUL_X_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_Y_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_Z_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_BIT_SLOTS)?;
    hash_usize(&mut hasher, f2z_params.row_vars)?;
    hash_usize(&mut hasher, f2z_params.col_vars)?;
    hash_usize(&mut hasher, f2z_params.word_bits)?;
    Ok(*hasher.finalize().as_bytes())
}

fn bitified_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, bool>,
    assignment_binding: &[u8; 32],
    layout: &U128MulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &U128MulBitifiedClaim,
    modulus: u128,
) -> Result<[u8; 32], U128MulSpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(BITIFIED_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hash_usize(&mut hasher, matrices.field_modulus_encoding().len())?;
    hasher.update(matrices.field_modulus_encoding());
    hasher.update(matrices.digest());
    hasher.update(&modulus.to_le_bytes());

    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, U128_MUL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, opening.params.row_vars)?;
    hash_usize(&mut hasher, opening.params.col_vars)?;
    hash_usize(&mut hasher, opening.params.word_bits)?;
    hash_usize(&mut hasher, U128_MUL_X_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_Y_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_Z_SLOT_START)?;
    hash_usize(&mut hasher, U128_MUL_OPERAND_BITS)?;
    hash_usize(&mut hasher, U128_MUL_PRODUCT_BITS)?;
    hash_usize(&mut hasher, U128_MUL_BIT_SLOTS)?;
    // Mapping version one: little-endian bits, e0/x/y/z block order, and
    // nonzero scale normalized onto the folded row factors.
    hasher.update(&[1, 0, 1, 2, 3]);

    hash_usize(&mut hasher, terminal_claim.point().len())?;
    for coordinate in terminal_claim.point() {
        hash_spartan_f2z_element(&mut hasher, coordinate);
    }
    hash_spartan_f2z_element(&mut hasher, terminal_claim.scale());
    hash_spartan_f2z_element(&mut hasher, terminal_claim.value());

    let (gate_low, gate_high) = opening.gate_point.split_at(opening.params.col_vars);
    hash_usize(&mut hasher, gate_low.len())?;
    for coordinate in gate_low {
        hasher.update(&coordinate.0.to_le_bytes());
    }
    hash_usize(&mut hasher, gate_high.len())?;
    for coordinate in gate_high {
        hasher.update(&coordinate.0.to_le_bytes());
    }
    match opening.rows {
        U128MulBitifiedRows::Structured { x, y, z } => {
            hasher.update(&[0]);
            hasher.update(&x.0.to_le_bytes());
            hasher.update(&y.0.to_le_bytes());
            hasher.update(&z.0.to_le_bytes());
        }
        U128MulBitifiedRows::ConstantDummy => {
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

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), U128MulSpartanF2zError> {
    let value = u64::try_from(value).map_err(|_| U128MulSpartanF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn packed_variables(
    params: &crate::pcs::IntegerMatrixLayout,
) -> Result<usize, U128MulSpartanF2zError> {
    if !params.word_bits.is_power_of_two() || params.word_bits > u128::BITS as usize {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    let row_bit_vars = params
        .row_vars
        .checked_add(params.word_bits.trailing_zeros() as usize)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    let expected = row_bit_vars
        .checked_sub(LOG_PACKING)
        .and_then(|folded| folded.checked_add(params.col_vars))
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)?;
    if packed_vars(params) != expected {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(expected)
}

fn checked_pow2(exponent: usize) -> Result<usize, U128MulSpartanF2zError> {
    let exponent =
        u32::try_from(exponent).map_err(|_| U128MulSpartanF2zError::InvalidF2zParameters)?;
    1_usize
        .checked_shl(exponent)
        .ok_or(U128MulSpartanF2zError::InvalidF2zParameters)
}

/// The public statement facts the security-profile derivation consumes for
/// a u128 multiplication batch: per-row integer defects `|x·y − z|` are
/// below `2^256` (bounded to `2^258` conservatively), the Step-5.1 lift sums
/// `2^t` terms, and the opening is the DIRECT exponent-fold path (interval
/// capped at `c_w`, one chunk always). The PIOP runs the standard cubic
/// outer sumcheck.
pub fn u128_mul_instance_facts(
    params: &crate::pcs::IntegerMatrixLayout,
    row_vars: usize,
) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: 258,
        lift_arity_log2: params.row_vars as u32,
        opening_t: params.row_vars as u32,
        opening_word_bits: params.word_bits as u32,
        direct_opening: true,
        tau_arity: row_vars.max(1) as u32,
        piop_degree: 3,
        step50_magnitude_log2: 0,
    }
}

/// Setup-once, prime-independent bundle for the u128 paper path, including
/// the profile-selected Ligerito prover/verifier configuration.
pub struct PreparedU128MulRelation {
    skeleton: super::ConstraintMatricesSkeleton<SpartanF2zField, bool>,
    layout: U128MulLayout,
    security: IopSecurityParams,
    ligerito_configuration: crate::ligerito_flock::ResolvedLigerito,
    ligerito_pc: LigProverConfig,
    ligerito_vc: LigVerifierConfig,
}

impl PreparedU128MulRelation {
    /// Prepares the relation at the default [`Lambda100`] profile.
    pub fn new(layout: U128MulLayout) -> Result<Self, U128MulSpartanF2zError> {
        Self::new_with_profile::<Lambda100>(layout)
    }

    /// Prepares the relation under an explicit single-prime profile with the
    /// validated unique-decoding Ligerito opener at the profile's target.
    pub fn new_with_profile<P: IopSecurityProfile>(
        layout: U128MulLayout,
    ) -> Result<Self, U128MulSpartanF2zError> {
        Self::new_with_profile_and_ligerito::<P>(layout, crate::ligerito_flock::LigeritoSelection::for_target(P::LIGERITO_TARGET_BITS))
    }

    pub fn new_with_profile_and_ligerito<P: IopSecurityProfile>(
        layout: U128MulLayout,
        selection: crate::ligerito_flock::LigeritoSelection,
    ) -> Result<Self, U128MulSpartanF2zError> {
        validate_layout_geometry(&layout)?;
        let params = layout.f2z_params();
        let row_vars = layout
            .multiplications()
            .next_power_of_two()
            .trailing_zeros() as usize;
        let mut security = P::instantiate(&u128_mul_instance_facts(&params, row_vars))?;
        if security.projection_full_width || security.reduction.is_some() {
            return Err(U128MulSpartanF2zError::UnsupportedProfile);
        }
        if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
            return Err(U128MulSpartanF2zError::UnauditedF2zParameters);
        }
        let ligerito_configuration = selection.resolve(packed_variables(&params)?, security.ligerito_target_bits)
            .map_err(U128MulSpartanF2zError::LigeritoConfig)?;
        let ligerito_pc = ligerito_configuration.prover().clone();
        let ligerito_vc = ligerito_configuration.verifier().clone();
        security.adopt_ood_round(ligerito_configuration.ood_bits())?;
        validate_config_pair(&params, &ligerito_pc, &ligerito_vc)?;
        let raw = u128_mul_constraint_matrices(&layout)?;
        let skeleton = super::ConstraintMatricesSkeleton::new(raw).map_err(SpartanError::from)?;
        Ok(Self {
            skeleton,
            layout,
            security,
            ligerito_configuration,
            ligerito_pc,
            ligerito_vc,
        })
    }

    pub fn ligerito_configuration(&self) -> &crate::ligerito_flock::ResolvedLigerito {
        &self.ligerito_configuration
    }

    /// Statement-bound layout.
    pub const fn layout(&self) -> &U128MulLayout {
        &self.layout
    }

    /// F2Z geometry of the committed bit tensor.
    pub fn params(&self) -> crate::pcs::IntegerMatrixLayout {
        self.layout.f2z_params()
    }

    /// The instantiated security parameters and their accounting.
    pub const fn security(&self) -> &IopSecurityParams {
        &self.security
    }
}

/// Commits compact u128 bit rows under the prepared relation's selected
/// security profile. Proving and verification use the same retained configs.
pub fn commit_u128_mul_witness(
    prepared: &PreparedU128MulRelation,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, U128MulSpartanF2zError> {
    let params = prepared.params();
    validate_bit_rows(&params, &rows)?;
    let hint = commit_rs_ligerito_rows(&params, rows, &prepared.ligerito_pc);
    validate_commitment(&params, &hint.commitment, &prepared.ligerito_pc)?;
    Ok(hint)
}

/// The u128 multiplication proof: Spartan over the transcript-sampled prime
/// plus the runtime-q F2Z opening and the profile's grinding nonces.
#[derive(Clone)]
pub struct U128MulProof {
    initial_nonce: u64,
    terminal_nonce: u64,
    piop_nonces: Vec<u64>,
    spartan: SpartanPiopProof<SpartanF2zField>,
    f2z: IntEvalRsLigModQProof,
}

impl U128MulProof {
    /// Spartan outer and inner sumcheck proofs.
    pub const fn spartan(&self) -> &SpartanPiopProof<SpartanF2zField> {
        &self.spartan
    }

    /// Runtime-prime F2Z opening proof.
    pub const fn f2z(&self) -> &IntEvalRsLigModQProof {
        &self.f2z
    }

    /// Field elements in the Spartan payload, for analytic size accounting.
    pub fn spartan_payload_elements(&self) -> usize {
        4 * self.spartan.outer.sumcheck.round_polynomials.len()
            + 3
            + 3 * self.spartan.inner.round_polynomials.len()
    }

    /// Transmitted grinding nonces (initial/terminal boundaries when armed,
    /// the per-draw PIOP nonces, and the forest section in the F2Z stream).
    pub fn grinding_nonce_count(&self, security: &IopSecurityParams) -> usize {
        usize::from(security.initial_grinding_bits > 0)
            + usize::from(security.terminal_grinding_bits > 0)
            + self.piop_nonces.len()
            + self.f2z.grinding_nonces.len()
    }

    /// Serialized size in bytes of the proof: the Spartan payload as 16-byte
    /// field elements, the nonces as 8-byte words, and the F2Z opening's
    /// exact codec bytes.
    pub fn size_bytes(&self, security: &IopSecurityParams) -> usize {
        self.spartan_payload_elements() * 16
            + (self.grinding_nonce_count(security) - self.f2z.grinding_nonces.len()) * 8
            + self.f2z.to_bytes().len()
    }
}

fn grind_boundary<D: GrindingDomain, T: Transcript>(
    transcript: &mut T,
    bits: u32,
) -> Result<u64, GrindingError> {
    if bits == 0 {
        return Ok(0);
    }
    grind_and_absorb::<D, _>(transcript, GrindingRound::new(0), bits)
}

fn check_boundary<D: GrindingDomain, T: Transcript>(
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

/// Uniform per-draw PIOP grinding difficulty (the τ/initial bound).
fn piop_wrap_bits(security: &IopSecurityParams) -> u32 {
    security
        .initial_grinding_bits
        .max(security.piop_round_grinding_bits)
}

/// Samples the Step-2 prime from the profile interval and builds its
/// runtime field configuration and canonical arithmetic.
fn sample_mod_q(
    transcript: &mut impl Transcript,
    security: &IopSecurityParams,
) -> Result<
    (
        u128,
        usize,
        <SpartanF2zField as PrimeField>::Config,
        ProjArith,
    ),
    U128MulSpartanF2zError,
> {
    absorb_spartan_message(transcript, b"prime-domain", PRIME_SAMPLING_DOMAIN);
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
        .map_err(|_| U128MulSpartanF2zError::UnsupportedFieldModulus)?;
    SpartanF2zField::validate_config(&config)
        .map_err(|_| U128MulSpartanF2zError::UnsupportedFieldModulus)?;
    let q_bits = (u128::BITS - q.leading_zeros()) as usize;
    Ok((q, q_bits, config, ProjArith::new(q)))
}

/// Raw residues of the whole assignment `[e0 | x | y | z]` over the padded
/// column domain, built in parallel straight from the witness limbs.
fn raw_assignment(ctx: &RawMontyCtx, witness: &U128MulWitness) -> Vec<u128> {
    const MIN_LEN: usize = 4096;
    let layout = witness.layout();
    let capacity = layout.capacity();
    let live = layout.multiplications();
    let two_pow_128 = ctx.two_pow_128_residue();
    let mut table = vec![0_u128; layout.assignment_len()];
    table[0] = ctx.native_residue(1);
    for (block, values) in [(1, witness.x_values()), (2, witness.y_values())] {
        let target = &mut table[block * capacity..block * capacity + live];
        cfg_iter_mut!(target, MIN_LEN)
            .zip(cfg_iter!(values[..live], MIN_LEN))
            .for_each(|(slot, &value)| *slot = ctx.native_residue_u128(value));
    }
    let target = &mut table[3 * capacity..3 * capacity + live];
    cfg_iter_mut!(target, MIN_LEN)
        .zip(cfg_iter!(witness.z_lo_values()[..live], MIN_LEN))
        .zip(cfg_iter!(witness.z_hi_values()[..live], MIN_LEN))
        .for_each(|((slot, &lo), &hi)| *slot = ctx.native_residue_u256(lo, hi, two_pow_128));
    table
}

/// Proves the u128 batch under the prepared relation's security profile:
/// commit-before-prime, a transcript-sampled Step-2 prime, the Spartan PIOP
/// over that runtime field on residues built from the witness limbs (every
/// drawn challenge preceded by a PIOP grinding boundary at the profile
/// difficulty), runtime-q bitification, and the runtime-q F2Z opening.
pub fn prove_u128_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU128MulRelation,
    witness: &U128MulWitness,
    hint: &FlockCommitHint,
) -> Result<U128MulProof, U128MulSpartanF2zError> {
    let layout = prepared.layout();
    if witness.layout() != layout {
        return Err(U128MulSpartanF2zError::RelationWitnessLayoutMismatch);
    }
    let params = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&params, pc, vc)?;
    validate_bit_rows(&params, hint.rows())?;
    validate_commitment(&params, &hint.commitment, pc)?;
    let security = prepared.security();

    let binding = assignment_binding(layout, &hint.commitment, security)?;
    absorb_spartan_message(transcript, b"u128-mul-statement", &binding);
    prepared.ligerito_configuration.bind(transcript);
    let ood = crate::ligerito_flock::bind_prover_ood(transcript, hint, security.ood);

    // Paper §2.1 Step 2: pre-draw grinding, prime sample, projection.
    let step2_scope = tracing::info_span!("step2:project_prove").entered();
    let initial_nonce =
        grind_boundary::<InitialGrinding, _>(transcript, security.initial_grinding_bits)?;
    let (q, q_bits, config, arith) = sample_mod_q(transcript, security)?;
    let matrices = {
        let _scope = tracing::info_span!("u128-spartan-f2z:relation_projection_prove").entered();
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    let (products, assignment) = {
        let _scope = tracing::info_span!("u128-spartan-f2z:witness_projection_prove").entered();
        let live = layout.multiplications();
        let ctx = RawMontyCtx::new(&config);
        let products = RawProducts::from_native_u128_halves(
            &ctx,
            &witness.x_values()[..live],
            &witness.y_values()[..live],
            &witness.z_lo_values()[..live],
            &witness.z_hi_values()[..live],
            live.next_power_of_two(),
        );
        (products, raw_assignment(&ctx, witness))
    };
    if assignment.len() != 1_usize << matrices.num_column_vars() {
        return Err(U128MulSpartanF2zError::InvalidF2zParameters);
    }
    drop(step2_scope);

    // Step 3: the Spartan PIOP over F_q on raw residues, grinded per draw.
    let (spartan, terminal_claim, piop_nonces) = {
        let _step3 = tracing::info_span!("step3:piop_prove").entered();
        let _scope = tracing::info_span!("u128-spartan-f2z:spartan_prove").entered();
        let mut grinder: ProverGrindingTranscript<_, PiopGrinding> =
            ProverGrindingTranscript::new(transcript, piop_wrap_bits(security));
        let (spartan, terminal_claim) = prove_spartan_piop_raw_products_raw_witness(
            &mut grinder,
            &matrices,
            &binding,
            products,
            RawWitness::Field(assignment),
        )?;
        (spartan, terminal_claim, grinder.finish())
    };

    // Step 4: bitification at the runtime prime + the terminal boundary.
    let step4_scope = tracing::info_span!("step4:bitify_prove").entered();
    let (opening, bridge_digest) = {
        let _scope = tracing::info_span!("u128-spartan-f2z:bitify_prover").entered();
        let opening = bitify_u128_mul_spartan_claim_with(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    let terminal_nonce =
        grind_boundary::<TerminalGrinding, _>(transcript, security.terminal_grinding_bits)?;
    drop(step4_scope);

    // Steps 5.1–5.3: the runtime-q F2Z opening (one chunk by construction).
    let f2z = {
        let _step5 = tracing::info_span!("step5:open_prove").entered();
        let _scope = tracing::info_span!("u128-spartan-f2z:f2z_prove").entered();
        let chunks = {
            let _scope = tracing::info_span!("u128-spartan-f2z:f2z_prepare_prover").entered();
            prepare_bitified_chunks_with(&opening, q_bits, &arith)?
        };
        if chunks.len() != 1 {
            return Err(U128MulSpartanF2zError::MultiChunkRuntimeWeights);
        }
        prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            ModQOpeningKind::U128Mul,
            hint,
            &params,
            &chunks,
            &bridge_digest,
            q_bits,
            f2z_generator(),
            security.forest_round_grinding_bits,
            ood,
            pc,
        )
        .map_err(U128MulSpartanF2zError::F2z)?
    };

    Ok(U128MulProof {
        initial_nonce,
        terminal_nonce,
        piop_nonces,
        spartan,
        f2z,
    })
}

/// Verifies a u128 multiplication proof, re-deriving the prime from the
/// bound transcript.
pub fn verify_u128_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU128MulRelation,
    commitment: &Commitment,
    proof: &U128MulProof,
) -> Result<(), U128MulSpartanF2zError> {
    let layout = prepared.layout();
    let params = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&params, pc, vc)?;
    validate_commitment(&params, commitment, pc)?;
    let security = prepared.security();

    let binding = assignment_binding(layout, commitment, security)?;
    absorb_spartan_message(transcript, b"u128-mul-statement", &binding);
    prepared.ligerito_configuration.bind(transcript);
    let ood = crate::ligerito_flock::bind_verifier_ood(
        transcript, packed_variables(&params)?, security.ood, proof.f2z.ood.as_ref(),
    ).map_err(U128MulSpartanF2zError::F2z)?;

    let step2_scope = tracing::info_span!("step2:project_verify").entered();
    check_boundary::<InitialGrinding, _>(
        transcript,
        security.initial_grinding_bits,
        proof.initial_nonce,
    )?;
    let (q, q_bits, config, arith) = sample_mod_q(transcript, security)?;
    let matrices = {
        let _scope = tracing::info_span!("u128-spartan-f2z:relation_projection_verify").entered();
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    let terminal_claim = {
        let _step3 = tracing::info_span!("step3:piop_verify").entered();
        let _scope = tracing::info_span!("u128-spartan-f2z:spartan_verify").entered();
        let mut grinder: VerifierGrindingTranscript<_, PiopGrinding> =
            VerifierGrindingTranscript::new(
                transcript,
                piop_wrap_bits(security),
                &proof.piop_nonces,
            );
        let terminal_claim =
            verify_spartan_proof(&mut grinder, &matrices, &binding, &proof.spartan)?;
        grinder.finish()?;
        terminal_claim
    };

    let step4_scope = tracing::info_span!("step4:bitify_verify").entered();
    let (opening, bridge_digest) = {
        let _scope = tracing::info_span!("u128-spartan-f2z:bitify_verifier").entered();
        let opening = bitify_u128_mul_spartan_claim_with(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    check_boundary::<TerminalGrinding, _>(
        transcript,
        security.terminal_grinding_bits,
        proof.terminal_nonce,
    )?;
    drop(step4_scope);

    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let _scope = tracing::info_span!("u128-spartan-f2z:f2z_verify").entered();
    let prepared_claim = {
        let _scope = tracing::info_span!("u128-spartan-f2z:f2z_prepare_verifier").entered();
        prepare_bitified_claim_with(&opening, q_bits, &arith)?
    };
    if prepared_claim.chunks.len() != 1 {
        return Err(U128MulSpartanF2zError::MultiChunkRuntimeWeights);
    }
    let col_weights_q: Vec<u128> = prepared_claim
        .col_weights
        .iter()
        .map(|weight| weight.0)
        .collect();
    verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
        transcript,
        ModQOpeningKind::U128Mul,
        commitment,
        &proof.f2z,
        &params,
        &prepared_claim.chunks,
        &col_weights_q,
        &bridge_digest,
        f2z_generator(),
        prepared_claim.claimed.0,
        q,
        q_bits,
        security.forest_round_grinding_bits,
        ood,
        vc,
    )
    .map_err(U128MulSpartanF2zError::F2z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    fn witness(multiplications: usize, salt: u64) -> U128MulWitness {
        let mut state = 0x243f_6a88_85a3_08d3_u64 ^ salt;
        U128MulWitness::from_fn(multiplications, |index| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let wide = |a: u64, b: u64| (u128::from(a) << 64) | u128::from(b);
            match index % 7 {
                0 => (u128::MAX, u128::MAX),
                1 => (wide(next(), next()), 0),
                2 => (1, wide(next(), next())),
                3 => (u128::from(next()), u128::from(next())),
                _ => (wide(next(), next()), wide(next(), next())),
            }
        })
        .unwrap()
    }

    #[test]
    fn u128_paper_path_roundtrips_and_is_deterministic() {
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let witness = witness(1 << 15, 0);
        let layout = *witness.layout();
        let prepared = PreparedU128MulRelation::new(layout).unwrap();
        assert_eq!(prepared.security().lambda, 100);
        assert_eq!(prepared.params().row_vars, 9 + 15 - 7);
        let hint = commit_u128_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_u128_mul(&mut prover_transcript, &prepared, &witness, &hint).unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u128_mul(&mut verifier_transcript, &prepared, &hint.commitment, &proof).unwrap();
        assert!(proof.size_bytes(prepared.security()) > 0);

        let mut second_transcript = Blake3Transcript::new();
        let second = prove_u128_mul(&mut second_transcript, &prepared, &witness, &hint).unwrap();
        assert_eq!(proof.f2z().to_bytes(), second.f2z().to_bytes());
        assert_eq!(proof.piop_nonces, second.piop_nonces);
    }

    #[test]
    fn u128_paper_path_rejects_a_wrong_product() {
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let honest = witness(1 << 15, 1);
        let layout = *honest.layout();
        let prepared = PreparedU128MulRelation::new(layout).unwrap();

        // Flip one committed bit of the product's high half at gate 3; the
        // commitment no longer matches the honest assignment.
        let mut rows = honest.f2z_bit_rows();
        let (b, c) = layout.f2z_cell(U128_MUL_Z_SLOT_START + 200, 3).unwrap();
        rows[c][b / 64] ^= 1 << (b % 64);
        let hint = commit_u128_mul_witness(&prepared, rows).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let outcome = prove_u128_mul(&mut prover_transcript, &prepared, &honest, &hint);
        if let Ok(proof) = outcome {
            let mut verifier_transcript = Blake3Transcript::new();
            assert!(
                verify_u128_mul(&mut verifier_transcript, &prepared, &hint.commitment, &proof)
                    .is_err()
            );
        }
    }

    #[test]
    fn small_layouts_are_rejected_by_the_production_api() {
        let witness = witness(1 << 10, 2);
        assert!(matches!(
            PreparedU128MulRelation::new(*witness.layout()),
            Err(U128MulSpartanF2zError::UnauditedF2zParameters)
        ));
    }
}
