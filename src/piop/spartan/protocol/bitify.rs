//! Bitification: the adjoint of a relation's public bit-reconstruction map,
//! applied to Spartan's terminal scaled assignment-MLE claim.
//!
//! Every block-structured relation lays its integer assignment out as
//! `2^selector_vars` blocks of `capacity` gates, block 0 being the constant
//! block `[1, 0, …]` and the others integer values reconstructed from
//! contiguous bit-slot ranges of the committed tensor. Spartan's terminal
//! point is low-coordinate-first: its last `selector_vars` coordinates select
//! the block, the preceding ones select a gate. F2Z places the low gate
//! coordinates on the clear column axis; the high gate coordinates and the
//! word slots form the folded row axis.

use crypto_primitives::{PrimeField, crypto_bigint_uint::Uint};

use crate::{
    ext_proj::ProjArith,
    pcs::{Fq, IntegerMatrixLayout, ModQWeightChunks, ProjectCanonicalU128, mod_q_num_chunks},
    utils::{cfg_chunks_mut, cfg_iter, cfg_iter_mut},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    ProtocolError, SpartanF2zField, SpartanField, binding::BindingHasher, checked_pow2,
    matrix::ScaledMleEvaluationClaim,
};

/// A contiguous range of bit slots of one gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotRange {
    pub bit_slot_start: usize,
    pub bit_count: usize,
}

/// How assignment blocks map to bit slots of the committed tensor.
///
/// `blocks[code]` describes the block selected by the little-endian block
/// code `code` (the first selector coordinate is the low bit): `None` for
/// the constant block (code 0) and for public zero padding blocks, `Some`
/// for a block whose values are reconstructed from that slot range with
/// little-endian bit weights.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockTable {
    selector_vars: usize,
    blocks: Vec<Option<SlotRange>>,
}

impl BlockTable {
    pub fn new(selector_vars: usize, blocks: Vec<Option<SlotRange>>) -> Result<Self, ProtocolError> {
        if blocks.len() != checked_pow2(selector_vars)? || blocks[0].is_some() {
            return Err(ProtocolError::InvalidBlockTable);
        }
        Ok(Self {
            selector_vars,
            blocks,
        })
    }

    pub fn selector_vars(&self) -> usize {
        self.selector_vars
    }

    /// The variable blocks in block-code order.
    pub fn variable_blocks(&self) -> impl Iterator<Item = (usize, SlotRange)> + '_ {
        self.blocks
            .iter()
            .enumerate()
            .filter_map(|(code, range)| range.map(|range| (code, range)))
    }
}

/// The factorized claim bound between Spartan and F2Z: the row functional is
/// one factor per variable block times the slot weights, the column
/// functional is the equality table of the low gate coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitifiedClaim {
    pub params: IntegerMatrixLayout,
    pub gate_point: Box<[Fq]>,
    pub rows: BitifiedRows,
    pub col_scale: Fq,
    pub claimed: Fq,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BitifiedRows {
    /// One factor per variable block, in block-code order.
    Structured(Vec<Fq>),
    /// The variable part vanishes: a deterministic dummy row with an all-zero
    /// clear read-off.
    ConstantDummy,
}

impl BitifiedClaim {
    pub fn gate_low(&self) -> &[Fq] {
        &self.gate_point[..self.params.col_vars]
    }

    pub fn gate_high(&self) -> &[Fq] {
        &self.gate_point[self.params.col_vars..]
    }
}

/// Applies the adjoint of the block reconstruction map to a terminal claim
/// over the runtime prime `q`.
pub fn bitify(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    params: IntegerMatrixLayout,
    gate_vars: usize,
    table: &BlockTable,
    q: u128,
    arith: &ProjArith,
) -> Result<BitifiedClaim, ProtocolError> {
    if claim.point().len() != gate_vars.saturating_add(table.selector_vars)
        || params.col_vars > gate_vars
    {
        return Err(ProtocolError::InvalidClaimPoint);
    }

    let expected_modulus = Uint::from(q);
    let project = |value: &SpartanF2zField| -> Result<Fq, ProtocolError> {
        let modulus = Uint::new(value.cfg().modulus().get());
        if modulus != expected_modulus || value.validate_element().is_err() {
            return Err(ProtocolError::ClaimFieldMismatch);
        }
        let canonical = value.canonical_u128();
        if canonical >= q {
            return Err(ProtocolError::ClaimFieldMismatch);
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
    let mul = |left: Fq, right: Fq| Fq(arith.mul(left.0, right.0));

    let gate_point = claim.point()[..gate_vars]
        .iter()
        .map(|value| project(value))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let selector = claim.point()[gate_vars..]
        .iter()
        .map(|value| project(value))
        .collect::<Result<Vec<_>, _>>()?;
    let one = Fq(1);
    let one_minus: Vec<Fq> = selector
        .iter()
        .map(|coordinate| Fq(sub(one.0, coordinate.0)))
        .collect();

    // The block factor is the equality-table entry of the selector point at
    // the block code: coordinate `i` contributes `s_i` when bit `i` of the
    // code is set and `1 - s_i` otherwise.
    let block_factor = |code: usize| -> Fq {
        selector
            .iter()
            .zip(&one_minus)
            .enumerate()
            .fold(one, |acc, (bit, (set, clear))| {
                mul(acc, if (code >> bit) & 1 == 1 { *set } else { *clear })
            })
    };

    let scale = project(claim.scale())?;
    let value = project(claim.value())?;
    let constant_evaluation = gate_point
        .iter()
        .copied()
        .fold(block_factor(0), |acc, coordinate| {
            mul(acc, Fq(sub(one.0, coordinate.0)))
        });
    let adjusted_claim = Fq(sub(value.0, arith.mul(scale.0, constant_evaluation.0)));

    let factors: Vec<Fq> = table
        .variable_blocks()
        .map(|(code, _)| block_factor(code))
        .collect();

    // Put a nonzero Spartan scale on the folded row side, avoiding a dense
    // column-table scaling pass. A zero scale stays on the clear side so it
    // does not erase the row functional that exponent folding certifies.
    let (rows, col_scale) = if factors.iter().all(|factor| *factor == Fq(0)) {
        if adjusted_claim != Fq(0) {
            return Err(ProtocolError::InvalidConstantOnlyClaim);
        }
        (BitifiedRows::ConstantDummy, Fq(0))
    } else if scale == Fq(0) {
        (BitifiedRows::Structured(factors), Fq(0))
    } else {
        let factors = if scale == one {
            factors
        } else {
            factors.into_iter().map(|factor| mul(scale, factor)).collect()
        };
        (BitifiedRows::Structured(factors), one)
    };

    Ok(BitifiedClaim {
        params,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

/// Little-endian equality table with one fixed-factor Montgomery
/// multiplication per parent and one allocation for the complete table.
pub fn eq_le_table_fq_fast_with(point: &[Fq], arith: &ProjArith) -> Result<Vec<Fq>, ProtocolError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);

    let q = arith.q();
    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(ProtocolError::InvalidF2zParameters)?;
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

/// The dense canonical row weights of a structured opening, in F2Z row
/// order `(word_slot << h) | gate_high`:
///
/// `w[(word_slot << h) | g] = block_factor(word_slot) · 2^{W · (word_slot − block_word_start)} · eq(gate_high_point, g)`
///
/// for every variable block, zero for word slots outside every block. The
/// per-word scalars are formed first, then every row is one fixed-factor
/// Montgomery multiplication of the shared `eq` table.
fn structured_row_weights(
    params: &IntegerMatrixLayout,
    gate_high: &[Fq],
    table: &BlockTable,
    factors: &[Fq],
    arith: &ProjArith,
) -> Result<Vec<u128>, ProtocolError> {
    let word_bits = params.word_bits;
    if !word_bits.is_power_of_two() {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    let high_gate_count = checked_pow2(gate_high.len())?;
    let row_count = checked_pow2(params.row_vars)?;
    if !row_count.is_multiple_of(high_gate_count) {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    let word_slots = row_count / high_gate_count;

    // Per-word scalars: the block factor times 2^{W·(word within the block)}.
    let pow2_word = arith.reduce(1_u128 << word_bits);
    let mut word_scalars = vec![0_u128; word_slots];
    for ((_, range), block_factor) in table.variable_blocks().zip(factors) {
        if !range.bit_slot_start.is_multiple_of(word_bits)
            || !range.bit_count.is_multiple_of(word_bits)
        {
            return Err(ProtocolError::InvalidF2zParameters);
        }
        let mut scalar = arith.reduce(block_factor.0);
        for word in range.bit_slot_start / word_bits
            ..(range.bit_slot_start + range.bit_count) / word_bits
        {
            let Some(slot) = word_scalars.get_mut(word) else {
                return Err(ProtocolError::InvalidF2zParameters);
            };
            *slot = scalar;
            scalar = arith.mul(scalar, pow2_word);
        }
    }

    let eq_high = eq_le_table_fq_fast_with(gate_high, arith)?;
    let mut weights = vec![0_u128; row_count];
    cfg_chunks_mut!(weights, high_gate_count)
        .zip(cfg_iter!(word_scalars))
        .for_each(|(rows, &scalar)| {
            let factor = arith.monty_factor(scalar);
            for (row, equality) in rows.iter_mut().zip(&eq_high) {
                *row = arith.mul_plain_by(equality.0, &factor);
            }
        });
    Ok(weights)
}

/// Compiles only the folded row functional into the mod-q chunk
/// representation. The prover never reads the clear column weights or the
/// claimed value, so keeping those verifier-only avoids an entire `2^s`
/// equality table on the proving path.
pub fn prepare_chunks(
    opening: &BitifiedClaim,
    table: &BlockTable,
    q_bits: usize,
    arith: &ProjArith,
) -> Result<ModQWeightChunks, ProtocolError> {
    let params = opening.params;
    if opening.gate_point.len() < params.col_vars {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    let gate_high = opening.gate_high();

    match &opening.rows {
        BitifiedRows::ConstantDummy => {
            let mut chunks = ModQWeightChunks::zeroed(&params, q_bits)
                .map_err(|_| ProtocolError::InvalidF2zParameters)?;
            chunks
                .set_weight_range(0, &[1])
                .map_err(|_| ProtocolError::InvalidF2zParameters)?;
            Ok(chunks)
        }
        BitifiedRows::Structured(factors) => {
            let weights = structured_row_weights(&params, gate_high, table, factors, arith)?;
            if mod_q_num_chunks(&params, q_bits) == 1 {
                ModQWeightChunks::from_single_chunk(&params, q_bits, weights)
                    .map_err(|_| ProtocolError::InvalidF2zParameters)
            } else {
                let mut chunks = ModQWeightChunks::zeroed(&params, q_bits)
                    .map_err(|_| ProtocolError::InvalidF2zParameters)?;
                chunks
                    .set_weight_range(0, &weights)
                    .map_err(|_| ProtocolError::InvalidF2zParameters)?;
                Ok(chunks)
            }
        }
    }
}

/// The clear column weights `col_scale · eq(gate_low, ·)` (all zero when the
/// clear read-off is switched off).
pub fn column_weights(opening: &BitifiedClaim, arith: &ProjArith) -> Result<Vec<Fq>, ProtocolError> {
    let params = opening.params;
    if opening.col_scale == Fq(0) {
        return Ok(vec![Fq(0); checked_pow2(params.col_vars)?]);
    }
    let mut eq_low = eq_le_table_fq_fast_with(opening.gate_low(), arith)?;
    if opening.col_scale != Fq(1) {
        let factor = arith.monty_factor(opening.col_scale.0);
        cfg_iter_mut!(&mut eq_low, 256).for_each(|weight| {
            weight.0 = arith.mul_plain_by(weight.0, &factor);
        });
    }
    Ok(eq_low)
}

/// The digest binding the terminal Spartan claim and its bitification to the
/// relation, the statement binding and the runtime prime:
///
/// `domain ‖ binding ‖ len(modulus) ‖ modulus ‖ relation digest ‖ q ‖ <relation constants> ‖ claim ‖ gate point ‖ rows ‖ col_scale ‖ claimed`
///
/// where the relation writes its own constants section (layout numbers,
/// slot constants, mapping version) through `constants`.
pub fn bridge_digest(
    domain: &[u8],
    assignment_binding: &[u8; 32],
    relation_modulus_encoding: &[u8],
    relation_digest: &[u8; 32],
    modulus: u128,
    constants: impl FnOnce(&mut BindingHasher) -> Result<(), ProtocolError>,
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &BitifiedClaim,
) -> Result<[u8; 32], ProtocolError> {
    let mut hasher = BindingHasher::new();
    hasher.bytes(domain).bytes(assignment_binding);
    hasher.prefixed(relation_modulus_encoding)?;
    hasher.bytes(relation_digest).u128_le(modulus);
    constants(&mut hasher)?;

    hasher.usize(terminal_claim.point().len())?;
    for coordinate in terminal_claim.point() {
        hasher.element(coordinate);
    }
    hasher.element(terminal_claim.scale());
    hasher.element(terminal_claim.value());

    hasher.usize(opening.gate_low().len())?;
    for coordinate in opening.gate_low() {
        hasher.u128_le(coordinate.0);
    }
    hasher.usize(opening.gate_high().len())?;
    for coordinate in opening.gate_high() {
        hasher.u128_le(coordinate.0);
    }
    match &opening.rows {
        BitifiedRows::Structured(factors) => {
            hasher.byte(0);
            for factor in factors {
                hasher.u128_le(factor.0);
            }
        }
        BitifiedRows::ConstantDummy => {
            hasher.byte(1);
        }
    }
    hasher.u128_le(opening.col_scale.0);
    hasher.u128_le(opening.claimed.0);
    Ok(hasher.finalize())
}
