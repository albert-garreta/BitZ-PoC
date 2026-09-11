//! Succinct opening of u64 multiplication assignments through F2Z.
//!
//! The integer R1CS assignment has five logical blocks,
//!
//! ```text
//! [constant block | x block | y block | z_lo block | z_hi block],
//! ```
//!
//! and is zero-padded to eight blocks for Spartan's assignment MLE. The
//! commitment stores four 64-bit little-endian values per multiplication in
//! 256 physical slots. This module applies the transpose of that public
//! reconstruction map to Spartan's terminal assignment claim and opens the
//! resulting compact-bit claim with F2Z, following the paper protocol:
//! commit-before-prime, a transcript-sampled Step-2 prime, the Spartan PIOP
//! over that runtime field (the exact `u64` assignment enters the inner
//! sumcheck natively; only the `2^128`-sized products are reduced into the
//! field, as raw residues built straight from the witness limbs),
//! runtime-q bitification, and the runtime-q F2Z opening.

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
        SpartanError, SpartanPiopProof, prove_spartan_piop_raw_products_native_assignment,
        verify_spartan_proof,
    },
    profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100, ProfileError},
    raw_monty::{RawMontyCtx, RawProducts},
    u64_mul::{
        U64_MUL_BIT_SLOTS, U64_MUL_LIMB_BASE, U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS,
        U64_MUL_PADDED_ASSIGNMENT_BLOCKS, U64_MUL_SLOT_VARS, U64_MUL_VALUE_BITS,
        U64_MUL_X_SLOT_START, U64_MUL_Y_SLOT_START, U64_MUL_Z_HI_SLOT_START,
        U64_MUL_Z_LO_SLOT_START, U64MulCoefficient, U64MulError, U64MulLayout, U64MulWitness,
        u64_mul_constraint_matrices,
    },
};

const PRIME_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-u64-mul/runtime-prime/v1";
const BINDING_DOMAIN: &[u8] = b"f2z/spartan-u64-f2z/assignment/v1-runtime";
/// Domain of the compact, factorized u64 claim bound between Spartan and F2Z.
const BITIFIED_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-u64-f2z/bitified-claim/v1";
/// Canonical order of the five logical and three padded assignment blocks.
const ASSIGNMENT_BLOCK_ORDER: &[u8] = b"e0|x|y|zlo|zhi|zero|zero|zero";

/// Embedded, validator-gated Ligerito profiles begin at a 23-variable
/// committed bit MLE: eight slot variables plus fifteen gate variables.
const MIN_PRODUCTION_GATE_VARS: usize = 15;

enum InitialGrinding {}

impl GrindingDomain for InitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u64-mul/grinding/initial/v1";
}

enum TerminalGrinding {}

impl GrindingDomain for TerminalGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u64-mul/grinding/terminal/v1";
}

/// Per-challenge PIOP grinding domain: at a nonzero difficulty every
/// challenge the Spartan PIOP draws is preceded by one boundary here, at
/// the profile's initial bound (the maximum any single draw needs).
enum PiopGrinding {}

impl GrindingDomain for PiopGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-u64-mul/grinding/piop/v1";
}

/// Compact result of applying the public four-by-64-bit reconstruction map.
///
/// Equality tables are deliberately absent. The F2Z boundary compiles these
/// factors directly into its prepared mod-q representation, keeping Bitify
/// logarithmic in the number of multiplication rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct U64MulBitifiedClaim {
    params: crate::pcs::IntEvalParams,
    gate_point: Box<[Fq]>,
    rows: U64MulBitifiedRows,
    col_scale: Fq,
    claimed: Fq,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum U64MulBitifiedRows {
    Structured { x: Fq, y: Fq, z_lo: Fq, z_hi: Fq },
    ConstantOrPaddingDummy,
}

impl U64MulBitifiedClaim {
    /// Public F2Z geometry selected by the statement-bound layout.
    pub const fn params(&self) -> crate::pcs::IntEvalParams {
        self.params
    }

    /// Claimed value after subtracting the public constant-block term.
    pub const fn claimed(&self) -> Fq {
        self.claimed
    }
}

struct PreparedU64MulBitifiedClaim {
    chunks: crate::pcs::ModQWeightChunks,
    col_weights: Vec<Fq>,
    claimed: Fq,
}

/// Failures in u64 layout validation, claim translation, or either proof
/// system.
#[derive(Debug, Error)]
pub enum U64MulSpartanF2zError {
    #[error(transparent)]
    Relation(#[from] U64MulError),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the runtime prime is not a supported Spartan field modulus")]
    UnsupportedFieldModulus,

    #[error("the witness does not match the prepared u64 multiplication layout")]
    RelationWitnessLayoutMismatch,

    #[error("the compact bit rows do not match the u64 multiplication layout")]
    InvalidBitRows,

    #[error("the F2Z parameters are invalid for the compact u64 multiplication layout")]
    InvalidF2zParameters,

    #[error("the combined Spartan/F2Z proof requires at least 2^15 multiplication slots")]
    UnauditedF2zParameters,

    #[error("the commitment parameters do not match the derived F2Z configuration")]
    CommitmentConfigMismatch,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element uses a field other than the runtime prime")]
    ClaimFieldMismatch,

    #[error("a constant-or-padding-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOrPaddingOnlyClaim,

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
    #[error("the u64 paper path requires a single-prime security profile")]
    UnsupportedProfile,

    /// The derived prime interval must keep the row weights to one
    /// exponent-fold chunk; the profile guarantees this.
    #[error("the runtime prime produced a multi-chunk row functional")]
    MultiChunkRuntimeWeights,
}

/// Applies the adjoint of the public 64/64/64/64-bit reconstruction to a
/// terminal scaled assignment-MLE claim over the runtime modulus `q`.
///
/// Spartan's point is low-coordinate-first. Its final three coordinates
/// select the eight padded assignment blocks; the preceding coordinates
/// select a gate. F2Z places the low gate coordinates on the clear column
/// axis; the high gate coordinates and the eight slot coordinates form the
/// folded row axis.
pub(crate) fn bitify_u64_mul_spartan_claim_with(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &U64MulLayout,
    q: u128,
    arith: &ProjArith,
) -> Result<U64MulBitifiedClaim, U64MulSpartanF2zError> {
    validate_layout_geometry(layout)?;

    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars.saturating_add(3) {
        return Err(U64MulSpartanF2zError::InvalidClaimPoint);
    }

    let params = layout.f2z_params();
    let expected_modulus = Uint::from(q);
    let project = |value: &SpartanF2zField| -> Result<Fq, U64MulSpartanF2zError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(U64MulSpartanF2zError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= q {
            return Err(U64MulSpartanF2zError::ClaimFieldMismatch);
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
    let block_mid = project(&claim.point()[gate_vars + 1])?;
    let block_high = project(&claim.point()[gate_vars + 2])?;

    let one = Fq(1);
    let one_minus_low = Fq(sub(one.0, block_low.0));
    let one_minus_mid = Fq(sub(one.0, block_mid.0));
    let one_minus_high = Fq(sub(one.0, block_high.0));
    let mul = |left: Fq, right: Fq| Fq(arith.mul(left.0, right.0));
    let mul3 = |first: Fq, second: Fq, third: Fq| mul(mul(first, second), third);

    // Little-endian block-selector order is 000=e0, 001=x, 010=y,
    // 011=z_lo, 100=z_hi, and 101..111 are public zero padding.
    let constant_factor = mul3(one_minus_low, one_minus_mid, one_minus_high);
    let x_factor = mul3(block_low, one_minus_mid, one_minus_high);
    let y_factor = mul3(one_minus_low, block_mid, one_minus_high);
    let z_lo_factor = mul3(block_low, block_mid, one_minus_high);
    let z_hi_factor = mul3(one_minus_low, one_minus_mid, block_high);

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point
        .iter()
        .copied()
        .fold(constant_factor, |acc, coordinate| {
            mul(acc, Fq(sub(one.0, coordinate.0)))
        });
    let adjusted_claim = Fq(sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

    // Normalize nonzero scale onto the folded row factors. The verifier then
    // builds an unscaled clear-column equality table, while scale zero keeps a
    // nonzero row functional and zeros the clear read-off.
    let (rows, col_scale) = if [x_factor, y_factor, z_lo_factor, z_hi_factor]
        .into_iter()
        .all(|factor| factor == Fq(0))
    {
        if adjusted_claim != Fq(0) {
            return Err(U64MulSpartanF2zError::InvalidConstantOrPaddingOnlyClaim);
        }
        (U64MulBitifiedRows::ConstantOrPaddingDummy, Fq(0))
    } else if scale == Fq(0) {
        (
            U64MulBitifiedRows::Structured {
                x: x_factor,
                y: y_factor,
                z_lo: z_lo_factor,
                z_hi: z_hi_factor,
            },
            Fq(0),
        )
    } else {
        let (x, y, z_lo, z_hi) = if scale == one {
            (x_factor, y_factor, z_lo_factor, z_hi_factor)
        } else {
            (
                mul(scale, x_factor),
                mul(scale, y_factor),
                mul(scale, z_lo_factor),
                mul(scale, z_hi_factor),
            )
        };
        (U64MulBitifiedRows::Structured { x, y, z_lo, z_hi }, one)
    };

    // z_hi is reconstructed raw; 2^64 is already bound as matrix C's coefficient.
    Ok(U64MulBitifiedClaim {
        params,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

fn validate_layout_geometry(layout: &U64MulLayout) -> Result<(), U64MulSpartanF2zError> {
    let params = layout.f2z_params();
    if params.word_bits != 1
        || params.t < LOG_PACKING
        || params.s > layout.gate_vars()
        || params.t.saturating_add(params.word_bits) > 126
    {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    let total_vars = params
        .t
        .checked_add(params.s)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(U64_MUL_SLOT_VARS)
            .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?
        || U64_MUL_BIT_SLOTS != 1_usize << U64_MUL_SLOT_VARS
        || U64_MUL_BIT_SLOTS != 4 * U64_MUL_VALUE_BITS
        || layout.assignment_len() != U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS * layout.capacity()
        || layout.padded_assignment_len() != U64_MUL_PADDED_ASSIGNMENT_BLOCKS * layout.capacity()
    {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(params.t)?;
    let col_count = checked_pow2(params.s)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    let expected_cells = U64_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    // `packed_variables` counts one packed variable per 128 bits: the eight
    // slot variables leave one extra packed variable on top of the gates.
    if cells != expected_cells
        || packed_variables(&params)? != layout.gate_vars() + (U64_MUL_SLOT_VARS - LOG_PACKING)
    {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_bit_rows(
    params: &crate::pcs::IntEvalParams,
    rows: &[Vec<u64>],
) -> Result<(), U64MulSpartanF2zError> {
    let row_count = checked_pow2(params.t)?;
    let col_count = checked_pow2(params.s)?;
    if row_count % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(U64MulSpartanF2zError::InvalidBitRows);
    }
    let words_per_col = row_count / u64::BITS as usize;
    if rows.iter().any(|row| row.len() != words_per_col) {
        return Err(U64MulSpartanF2zError::InvalidBitRows);
    }
    Ok(())
}

fn validate_config_pair(
    params: &crate::pcs::IntEvalParams,
    pc: &LigProverConfig,
    vc: &LigVerifierConfig,
) -> Result<(), U64MulSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
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
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(())
}

fn validate_commitment(
    params: &crate::pcs::IntEvalParams,
    commitment: &Commitment,
    pc: &LigProverConfig,
) -> Result<(), U64MulSpartanF2zError> {
    let m_p = packed_variables(params)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    };
    let commitment_params = &commitment.params;
    if commitment_params.m != m_p + LOG_PACKING
        || commitment_params.log_inv_rate != log_inv_rate
        || commitment_params.log_batch_size != pc.initial_k
        || commitment_params.profile != LigeritoProfile::default()
        || commitment_params.merkle_hash != pc.merkle_hash
    {
        return Err(U64MulSpartanF2zError::CommitmentConfigMismatch);
    }
    Ok(())
}

fn prepare_bitified_claim_with(
    opening: &U64MulBitifiedClaim,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<PreparedU64MulBitifiedClaim, U64MulSpartanF2zError> {
    let chunks = prepare_bitified_chunks_with(opening, q_bits, arith)?;
    let params = opening.params;
    let col_weights = if opening.col_scale == Fq(0) {
        vec![Fq(0); checked_pow2(params.s)?]
    } else {
        let (gate_low, _) = opening.gate_point.split_at(params.s);
        let mut eq_low = eq_le_table_fq_with(gate_low, arith)?;
        if opening.col_scale != Fq(1) {
            let factor = arith.monty_factor(opening.col_scale.0);
            cfg_iter_mut!(&mut eq_low, 256).for_each(|weight| {
                weight.0 = arith.mul_plain_by(weight.0, &factor);
            });
        }
        eq_low
    };
    Ok(PreparedU64MulBitifiedClaim {
        chunks,
        col_weights,
        claimed: opening.claimed,
    })
}

/// Compile the folded row functional directly into validated mod-q chunks.
/// The prover never materializes or hashes a second dense row-weight table.
fn prepare_bitified_chunks_with(
    opening: &U64MulBitifiedClaim,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<crate::pcs::ModQWeightChunks, U64MulSpartanF2zError> {
    let params = opening.params;
    let high_vars = params
        .t
        .checked_sub(U64_MUL_SLOT_VARS)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    let gate_vars = params
        .s
        .checked_add(high_vars)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    if params.word_bits != 1 || opening.gate_point.len() != gate_vars {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    let (_, gate_high) = opening.gate_point.split_at(params.s);

    match opening.rows {
        U64MulBitifiedRows::ConstantOrPaddingDummy => {
            let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, q_bits)
                .map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)?;
            chunks
                .set_weight_range(0, &[1])
                .map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)?;
            Ok(chunks)
        }
        U64MulBitifiedRows::Structured { x, y, z_lo, z_hi } => {
            let weights = structured_u64_row_weights(
                &params,
                gate_high,
                [
                    (U64_MUL_X_SLOT_START, x),
                    (U64_MUL_Y_SLOT_START, y),
                    (U64_MUL_Z_LO_SLOT_START, z_lo),
                    (U64_MUL_Z_HI_SLOT_START, z_hi),
                ],
                arith,
            )?;
            if crate::pcs::mod_q_num_chunks(&params, q_bits) == 1 {
                crate::pcs::ModQWeightChunks::from_single_chunk(&params, q_bits, weights)
                    .map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)
            } else {
                let mut chunks = crate::pcs::ModQWeightChunks::zeroed(&params, q_bits)
                    .map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)?;
                chunks
                    .set_weight_range(0, &weights)
                    .map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)?;
                Ok(chunks)
            }
        }
    }
}

/// The dense canonical row weights of a structured u64 opening, in F2Z row
/// order `(bit_slot << h) | gate_high`:
///
/// `w[(slot << h) | g] = block_factor(slot) · 2^{slot − block_start} · eq(gate_high_point, g)`
///
/// for the `x`, `y`, `z_lo` and `z_hi` blocks (the 256 bit slots are exactly
/// the four blocks). The 256 per-slot scalars are formed first, then every
/// row is one fixed-factor Montgomery multiplication of the shared `eq`
/// table, in one parallel pass over the whole `2^t` table. The previous
/// per-slot rescaling of a `2^h` scratch dispatched 256 tiny parallel jobs
/// and copied each result out serially, which cost several times the
/// arithmetic at 8 threads on both the prover and the verifier. Same field
/// elements, same canonical residues.
fn structured_u64_row_weights(
    params: &crate::pcs::IntEvalParams,
    gate_high: &[Fq],
    blocks: [(usize, Fq); 4],
    arith: &ProjArith,
) -> Result<Vec<u128>, U64MulSpartanF2zError> {
    let high_gate_count = checked_pow2(gate_high.len())?;
    let row_count = checked_pow2(params.t)?;
    if U64_MUL_BIT_SLOTS
        .checked_mul(high_gate_count)
        .is_none_or(|rows| rows != row_count)
    {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }

    // Per-slot scalars: the block factor times 2^{bit within the block}.
    let two = arith.reduce(2);
    let mut slot_scalars = vec![0_u128; U64_MUL_BIT_SLOTS];
    for (slot_start, block_factor) in blocks {
        let mut scalar = arith.reduce(block_factor.0);
        for slot in slot_start..slot_start.saturating_add(U64_MUL_VALUE_BITS) {
            let Some(entry) = slot_scalars.get_mut(slot) else {
                return Err(U64MulSpartanF2zError::InvalidF2zParameters);
            };
            *entry = scalar;
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
) -> Result<Vec<Fq>, U64MulSpartanF2zError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);
    let q = arith.q();

    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
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
    layout: &U64MulLayout,
    commitment: &Commitment,
    security: &IopSecurityParams,
) -> Result<[u8; 32], U64MulSpartanF2zError> {
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
    hasher.update(&U64_MUL_LIMB_BASE.to_le_bytes());
    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.padded_assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, U64_MUL_PADDED_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, U64_MUL_VALUE_BITS)?;
    hash_usize(&mut hasher, U64_MUL_X_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Y_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Z_LO_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Z_HI_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_BIT_SLOTS)?;
    hash_usize(&mut hasher, f2z_params.t)?;
    hash_usize(&mut hasher, f2z_params.s)?;
    hash_usize(&mut hasher, f2z_params.word_bits)?;
    Ok(*hasher.finalize().as_bytes())
}

fn bitified_claim_digest(
    matrices: &PreparedConstraintMatrices<SpartanF2zField, U64MulCoefficient>,
    assignment_binding: &[u8; 32],
    layout: &U64MulLayout,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &U64MulBitifiedClaim,
    modulus: u128,
) -> Result<[u8; 32], U64MulSpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(BITIFIED_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hash_usize(&mut hasher, matrices.field_modulus_encoding().len())?;
    hasher.update(matrices.field_modulus_encoding());
    hasher.update(matrices.digest());
    hasher.update(&modulus.to_le_bytes());
    hasher.update(&U64_MUL_LIMB_BASE.to_le_bytes());

    hash_usize(&mut hasher, layout.multiplications())?;
    hash_usize(&mut hasher, layout.capacity())?;
    hash_usize(&mut hasher, layout.assignment_len())?;
    hash_usize(&mut hasher, layout.padded_assignment_len())?;
    hash_usize(&mut hasher, layout.gate_vars())?;
    hash_usize(&mut hasher, U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, U64_MUL_PADDED_ASSIGNMENT_BLOCKS)?;
    hash_usize(&mut hasher, opening.params.t)?;
    hash_usize(&mut hasher, opening.params.s)?;
    hash_usize(&mut hasher, opening.params.word_bits)?;
    hash_usize(&mut hasher, U64_MUL_X_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Y_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Z_LO_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_Z_HI_SLOT_START)?;
    hash_usize(&mut hasher, U64_MUL_VALUE_BITS)?;
    hash_usize(&mut hasher, U64_MUL_BIT_SLOTS)?;
    // Mapping version one: little-endian bits, e0/x/y/zlo/zhi/zero/zero/zero
    // assignment order, raw z_hi reconstruction, and nonzero scale
    // normalized onto the folded row factors.
    hasher.update(&[1, 0, 1, 2, 3, 4, 5, 6, 7]);

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
        U64MulBitifiedRows::Structured { x, y, z_lo, z_hi } => {
            hasher.update(&[0]);
            hasher.update(&x.0.to_le_bytes());
            hasher.update(&y.0.to_le_bytes());
            hasher.update(&z_lo.0.to_le_bytes());
            hasher.update(&z_hi.0.to_le_bytes());
        }
        U64MulBitifiedRows::ConstantOrPaddingDummy => {
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

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), U64MulSpartanF2zError> {
    let value = u64::try_from(value).map_err(|_| U64MulSpartanF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn packed_variables(params: &crate::pcs::IntEvalParams) -> Result<usize, U64MulSpartanF2zError> {
    if !params.word_bits.is_power_of_two() || params.word_bits > u128::BITS as usize {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    let row_bit_vars = params
        .t
        .checked_add(params.word_bits.trailing_zeros() as usize)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    let expected = row_bit_vars
        .checked_sub(LOG_PACKING)
        .and_then(|folded| folded.checked_add(params.s))
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)?;
    if packed_vars(params) != expected {
        return Err(U64MulSpartanF2zError::InvalidF2zParameters);
    }
    Ok(expected)
}

fn checked_pow2(exponent: usize) -> Result<usize, U64MulSpartanF2zError> {
    let exponent =
        u32::try_from(exponent).map_err(|_| U64MulSpartanF2zError::InvalidF2zParameters)?;
    1_usize
        .checked_shl(exponent)
        .ok_or(U64MulSpartanF2zError::InvalidF2zParameters)
}

/// The public statement facts the security-profile derivation consumes for
/// a u64 multiplication batch: per-row integer defects
/// `|x·y − z_lo − 2^64·z_hi|` are below `2^128` (bounded to `2^130`
/// conservatively), the Step-5.1 lift sums `2^t` terms, and the opening is
/// the DIRECT exponent-fold path (interval capped at `c_w`, one chunk
/// always). The PIOP runs the standard cubic outer sumcheck.
pub fn u64_mul_instance_facts(
    params: &crate::pcs::IntEvalParams,
    row_vars: usize,
) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: 130,
        lift_arity_log2: params.t as u32,
        opening_t: params.t as u32,
        opening_word_bits: params.word_bits as u32,
        direct_opening: true,
        tau_arity: row_vars.max(1) as u32,
        piop_degree: 3,
        step50_magnitude_log2: 0,
    }
}

/// Setup-once, prime-independent bundle for the u64 paper path, including
/// the profile-selected Ligerito prover/verifier configuration.
pub struct PreparedU64MulRelation {
    skeleton: super::ConstraintMatricesSkeleton<SpartanF2zField, U64MulCoefficient>,
    layout: U64MulLayout,
    security: IopSecurityParams,
    ligerito_configuration: crate::ligerito_flock::ResolvedLigerito,
    ligerito_pc: LigProverConfig,
    ligerito_vc: LigVerifierConfig,
}

impl PreparedU64MulRelation {
    /// Prepares the relation at the default [`Lambda100`] profile.
    pub fn new(layout: U64MulLayout) -> Result<Self, U64MulSpartanF2zError> {
        Self::new_with_profile::<Lambda100>(layout)
    }

    /// Prepares the relation under an explicit single-prime profile with the
    /// validated unique-decoding Ligerito opener at the profile's target.
    pub fn new_with_profile<P: IopSecurityProfile>(
        layout: U64MulLayout,
    ) -> Result<Self, U64MulSpartanF2zError> {
        Self::new_with_profile_and_ligerito::<P>(layout, crate::ligerito_flock::LigeritoSelection::for_target(P::LIGERITO_TARGET_BITS))
    }

    pub fn new_with_profile_and_ligerito<P: IopSecurityProfile>(
        layout: U64MulLayout,
        selection: crate::ligerito_flock::LigeritoSelection,
    ) -> Result<Self, U64MulSpartanF2zError> {
        validate_layout_geometry(&layout)?;
        let params = layout.f2z_params();
        let row_vars = layout
            .multiplications()
            .next_power_of_two()
            .trailing_zeros() as usize;
        let mut security = P::instantiate(&u64_mul_instance_facts(&params, row_vars))?;
        if security.projection_full_width || security.reduction.is_some() {
            return Err(U64MulSpartanF2zError::UnsupportedProfile);
        }
        if layout.gate_vars() < MIN_PRODUCTION_GATE_VARS {
            return Err(U64MulSpartanF2zError::UnauditedF2zParameters);
        }
        let ligerito_configuration = selection.resolve(packed_variables(&params)?, security.ligerito_target_bits)
            .map_err(U64MulSpartanF2zError::LigeritoConfig)?;
        let ligerito_pc = ligerito_configuration.prover().clone();
        let ligerito_vc = ligerito_configuration.verifier().clone();
        security.adopt_ood_round(ligerito_configuration.ood_bits())?;
        validate_config_pair(&params, &ligerito_pc, &ligerito_vc)?;
        let raw = u64_mul_constraint_matrices(&layout)?;
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
    pub const fn layout(&self) -> &U64MulLayout {
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

/// Commits compact u64 bit rows under the prepared relation's selected
/// security profile. Proving and verification use the same retained configs.
pub fn commit_u64_mul_witness(
    prepared: &PreparedU64MulRelation,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, U64MulSpartanF2zError> {
    let params = prepared.params();
    validate_bit_rows(&params, &rows)?;
    let hint = commit_rs_ligerito_rows(&params, rows, &prepared.ligerito_pc);
    validate_commitment(&params, &hint.commitment, &prepared.ligerito_pc)?;
    Ok(hint)
}

/// The u64 multiplication proof: Spartan over the transcript-sampled prime
/// plus the runtime-q F2Z opening and the profile's grinding nonces.
#[derive(Clone)]
pub struct U64MulProof {
    initial_nonce: u64,
    terminal_nonce: u64,
    piop_nonces: Vec<u64>,
    spartan: SpartanPiopProof<SpartanF2zField>,
    f2z: IntEvalRsLigModQProof,
}

impl U64MulProof {
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
    U64MulSpartanF2zError,
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
        .map_err(|_| U64MulSpartanF2zError::UnsupportedFieldModulus)?;
    SpartanF2zField::validate_config(&config)
        .map_err(|_| U64MulSpartanF2zError::UnsupportedFieldModulus)?;
    let q_bits = (u128::BITS - q.leading_zeros()) as usize;
    Ok((q, q_bits, config, ProjArith::new(q)))
}

/// Proves the u64 batch under the prepared relation's security profile:
/// commit-before-prime, a transcript-sampled Step-2 prime, the Spartan PIOP
/// over that runtime field on the projected assignment and products (every
/// drawn challenge preceded by a PIOP grinding boundary at the profile
/// difficulty), runtime-q bitification, and the runtime-q F2Z opening.
pub fn prove_u64_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU64MulRelation,
    witness: &U64MulWitness,
    hint: &FlockCommitHint,
) -> Result<U64MulProof, U64MulSpartanF2zError> {
    let layout = prepared.layout();
    if witness.layout() != layout {
        return Err(U64MulSpartanF2zError::RelationWitnessLayoutMismatch);
    }
    let params = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&params, pc, vc)?;
    validate_bit_rows(&params, hint.rows())?;
    validate_commitment(&params, &hint.commitment, pc)?;
    let security = prepared.security();

    let binding = assignment_binding(layout, &hint.commitment, security)?;
    absorb_spartan_message(transcript, b"u64-mul-statement", &binding);
    prepared.ligerito_configuration.bind(transcript);
    let ood = crate::ligerito_flock::bind_prover_ood(transcript, hint, security.ood);

    // Paper §2.1 Step 2: pre-draw grinding, prime sample, projection.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce =
        grind_boundary::<InitialGrinding, _>(transcript, security.initial_grinding_bits)?;
    let (q, q_bits, config, arith) = sample_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:relation_projection_prove");
        PreparedConstraintMatrices::<SpartanF2zField, U64MulCoefficient>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    // Only the products need residues; the assignment stays native.
    let products = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:witness_projection_prove");
        let live = layout.multiplications();
        let ctx = RawMontyCtx::new(&config);
        RawProducts::from_native_limbs(
            &ctx,
            &witness.x_values()[..live],
            &witness.y_values()[..live],
            &witness.z_lo_values()[..live],
            &witness.z_hi_values()[..live],
            live.next_power_of_two(),
        )
    };
    drop(step2_scope);

    // Step 3: the Spartan PIOP over F_q (raw products, native assignment),
    // grinded per draw.
    let (spartan, terminal_claim, piop_nonces) = {
        let _step3 = crate::utils::prof::scope("step3:piop_prove");
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:spartan_prove");
        let mut grinder: ProverGrindingTranscript<_, PiopGrinding> =
            ProverGrindingTranscript::new(transcript, piop_wrap_bits(security));
        let (spartan, terminal_claim) = prove_spartan_piop_raw_products_native_assignment(
            &mut grinder,
            &matrices,
            &binding,
            products,
            witness.assignment(),
        )?;
        (spartan, terminal_claim, grinder.finish())
    };

    // Step 4: bitification at the runtime prime + the terminal boundary.
    let step4_scope = crate::utils::prof::scope("step4:bitify_prove");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:bitify_prover");
        let opening = bitify_u64_mul_spartan_claim_with(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    let terminal_nonce =
        grind_boundary::<TerminalGrinding, _>(transcript, security.terminal_grinding_bits)?;
    drop(step4_scope);

    // Steps 5.1–5.3: the runtime-q F2Z opening (one chunk by construction).
    let f2z = {
        let _step5 = crate::utils::prof::scope("step5:open_prove");
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:f2z_prove");
        let chunks = {
            let _scope = crate::utils::prof::scope("u64-spartan-f2z:f2z_prepare_prover");
            prepare_bitified_chunks_with(&opening, q_bits, &arith)?
        };
        if chunks.len() != 1 {
            return Err(U64MulSpartanF2zError::MultiChunkRuntimeWeights);
        }
        prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            ModQOpeningKind::U64Mul,
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
        .map_err(U64MulSpartanF2zError::F2z)?
    };

    Ok(U64MulProof {
        initial_nonce,
        terminal_nonce,
        piop_nonces,
        spartan,
        f2z,
    })
}

/// Verifies a u64 multiplication proof, re-deriving the prime from the
/// bound transcript.
pub fn verify_u64_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU64MulRelation,
    commitment: &Commitment,
    proof: &U64MulProof,
) -> Result<(), U64MulSpartanF2zError> {
    let layout = prepared.layout();
    let params = layout.f2z_params();
    let pc = &prepared.ligerito_pc;
    let vc = &prepared.ligerito_vc;
    validate_config_pair(&params, pc, vc)?;
    validate_commitment(&params, commitment, pc)?;
    let security = prepared.security();

    let binding = assignment_binding(layout, commitment, security)?;
    absorb_spartan_message(transcript, b"u64-mul-statement", &binding);
    prepared.ligerito_configuration.bind(transcript);
    let ood = crate::ligerito_flock::bind_verifier_ood(
        transcript, packed_variables(&params)?, security.ood, proof.f2z.ood.as_ref(),
    ).map_err(U64MulSpartanF2zError::F2z)?;

    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    check_boundary::<InitialGrinding, _>(
        transcript,
        security.initial_grinding_bits,
        proof.initial_nonce,
    )?;
    let (q, q_bits, config, arith) = sample_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:relation_projection_verify");
        PreparedConstraintMatrices::<SpartanF2zField, U64MulCoefficient>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    let terminal_claim = {
        let _step3 = crate::utils::prof::scope("step3:piop_verify");
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:spartan_verify");
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

    let step4_scope = crate::utils::prof::scope("step4:bitify_verify");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:bitify_verifier");
        let opening = bitify_u64_mul_spartan_claim_with(&terminal_claim, layout, q, &arith)?;
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

    let _step5 = crate::utils::prof::scope("step5:open_verify");
    let _scope = crate::utils::prof::scope("u64-spartan-f2z:f2z_verify");
    let prepared_claim = {
        let _scope = crate::utils::prof::scope("u64-spartan-f2z:f2z_prepare_verifier");
        prepare_bitified_claim_with(&opening, q_bits, &arith)?
    };
    if prepared_claim.chunks.len() != 1 {
        return Err(U64MulSpartanF2zError::MultiChunkRuntimeWeights);
    }
    let col_weights_q: Vec<u128> = prepared_claim
        .col_weights
        .iter()
        .map(|weight| weight.0)
        .collect();
    verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
        transcript,
        ModQOpeningKind::U64Mul,
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
    .map_err(U64MulSpartanF2zError::F2z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    fn witness(multiplications: usize, salt: u64) -> U64MulWitness {
        let mut state = 0x243f_6a88_85a3_08d3_u64 ^ salt;
        U64MulWitness::from_fn(multiplications, |index| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            match index % 7 {
                0 => (u64::MAX, u64::MAX),
                1 => (next(), 0),
                2 => (1, next()),
                _ => (next(), next()),
            }
        })
        .unwrap()
    }

    #[test]
    fn u64_paper_path_roundtrips_and_is_deterministic() {
        // Hold the shared env lock so tests that toggle transcript-shaping
        // `F2Z_*` variables cannot flip them between our prove and verify.
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let witness = witness(1 << 15, 0);
        let layout = *witness.layout();
        let prepared = PreparedU64MulRelation::new(layout).unwrap();
        assert_eq!(prepared.security().lambda, 100);
        assert_eq!(prepared.params().t, 8 + 15 - 7);
        let hint = commit_u64_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_u64_mul(&mut prover_transcript, &prepared, &witness, &hint).unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u64_mul(&mut verifier_transcript, &prepared, &hint.commitment, &proof).unwrap();
        assert!(proof.size_bytes(prepared.security()) > 0);

        let mut second_transcript = Blake3Transcript::new();
        let second = prove_u64_mul(&mut second_transcript, &prepared, &witness, &hint).unwrap();
        assert_eq!(proof.f2z().to_bytes(), second.f2z().to_bytes());
        assert_eq!(proof.piop_nonces, second.piop_nonces);
    }

    #[test]
    fn u64_paper_path_rejects_a_wrong_product() {
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let honest = witness(1 << 15, 1);
        let layout = *honest.layout();
        let prepared = PreparedU64MulRelation::new(layout).unwrap();

        // Corrupt one high limb: the committed bits and the projected products
        // no longer satisfy x·y = z_lo + 2^64·z_hi.
        let (layout, mut assignment) = honest.clone().into_parts();
        assignment[4 * layout.capacity() + 3] ^= 1;
        let corrupted = U64MulWitness::from_fn(layout.multiplications(), |index| {
            (honest.x_values()[index], honest.y_values()[index])
        })
        .unwrap();
        let mut rows = corrupted.f2z_bit_rows();
        // Flip the committed z_hi bit of gate 3 to match the corrupted value.
        let (b, c) = layout.f2z_cell(U64_MUL_Z_HI_SLOT_START, 3).unwrap();
        rows[c][b / 64] ^= 1 << (b % 64);
        let hint = commit_u64_mul_witness(&prepared, rows).unwrap();

        // An honest prover with a mismatching commitment, or the corrupted
        // assignment, must not produce a verifying proof.
        let mut prover_transcript = Blake3Transcript::new();
        let outcome = prove_u64_mul(&mut prover_transcript, &prepared, &corrupted, &hint);
        if let Ok(proof) = outcome {
            let mut verifier_transcript = Blake3Transcript::new();
            assert!(
                verify_u64_mul(&mut verifier_transcript, &prepared, &hint.commitment, &proof)
                    .is_err()
            );
        }
        drop(assignment);
    }

    #[test]
    fn small_layouts_are_rejected_by_the_production_api() {
        let witness = witness(1 << 10, 2);
        assert!(matches!(
            PreparedU64MulRelation::new(*witness.layout()),
            Err(U64MulSpartanF2zError::UnauditedF2zParameters)
        ));
    }
}
