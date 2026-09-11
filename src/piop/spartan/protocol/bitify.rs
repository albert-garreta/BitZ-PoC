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

use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_uint::Uint};

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

/// Canonical arithmetic modulo the runtime prime, over canonical `u128`
/// residues. [`ProjArith`] is the fast Montgomery path for primes below
/// `2^126`; [`FieldArith`] covers the full-width fingerprint primes.
pub trait Modular: Sync {
    /// A precomputed multiplier.
    type Fixed: Sync;

    fn q(&self) -> u128;
    fn reduce(&self, x: u128) -> u128;
    fn add(&self, a: u128, b: u128) -> u128;
    fn mul(&self, a: u128, b: u128) -> u128;
    fn fixed(&self, b: u128) -> Self::Fixed;
    fn mul_fixed(&self, a: u128, b: &Self::Fixed) -> u128;

    /// `a - b` for canonical residues.
    fn sub(&self, a: u128, b: u128) -> u128 {
        if b == 0 { a } else { self.add(a, self.q() - b) }
    }
}

impl Modular for ProjArith {
    type Fixed = crypto_bigint::modular::FixedMontyForm<{ crypto_bigint::U128::LIMBS }>;

    fn q(&self) -> u128 {
        ProjArith::q(self)
    }

    fn reduce(&self, x: u128) -> u128 {
        ProjArith::reduce(self, x)
    }

    fn add(&self, a: u128, b: u128) -> u128 {
        ProjArith::add(self, a, b)
    }

    fn mul(&self, a: u128, b: u128) -> u128 {
        ProjArith::mul(self, a, b)
    }

    fn fixed(&self, b: u128) -> Self::Fixed {
        self.monty_factor(b)
    }

    fn mul_fixed(&self, a: u128, b: &Self::Fixed) -> u128 {
        self.mul_plain_by(a, b)
    }
}

/// Canonical arithmetic through the runtime Montgomery field itself, for
/// any modulus the Spartan layer accepts (including the full-width
/// fingerprint primes the projection context cannot host).
pub struct FieldArith {
    config: <SpartanF2zField as PrimeField>::Config,
    q: u128,
}

impl FieldArith {
    pub fn new(q: u128, config: <SpartanF2zField as PrimeField>::Config) -> Self {
        Self { config, q }
    }

    fn element(&self, x: u128) -> SpartanF2zField {
        SpartanF2zField::from_with_cfg(x, &self.config)
    }
}

impl Modular for FieldArith {
    type Fixed = u128;

    fn q(&self) -> u128 {
        self.q
    }

    fn reduce(&self, x: u128) -> u128 {
        x % self.q
    }

    fn add(&self, a: u128, b: u128) -> u128 {
        let (sum, overflow) = a.overflowing_add(b);
        if overflow || sum >= self.q {
            sum.wrapping_sub(self.q)
        } else {
            sum
        }
    }

    fn mul(&self, a: u128, b: u128) -> u128 {
        let mut product = self.element(a);
        product *= &self.element(b);
        product.canonical_u128()
    }

    fn fixed(&self, b: u128) -> Self::Fixed {
        b
    }

    fn mul_fixed(&self, a: u128, b: &Self::Fixed) -> u128 {
        self.mul(a, *b)
    }
}

/// The arithmetic of a runtime prime: Montgomery projection below `2^126`,
/// the field itself above.
pub enum Arith {
    Proj(ProjArith),
    Field(FieldArith),
}

impl Arith {
    /// The arithmetic of `q` under the runtime field configuration.
    pub fn new(q: u128, config: &<SpartanF2zField as PrimeField>::Config) -> Self {
        if q < 1_u128 << 126 {
            Self::Proj(ProjArith::new(q))
        } else {
            Self::Field(FieldArith::new(q, config.clone()))
        }
    }
}

pub enum ArithFixed {
    Proj(<ProjArith as Modular>::Fixed),
    Field(u128),
}

impl Modular for Arith {
    type Fixed = ArithFixed;

    fn q(&self) -> u128 {
        match self {
            Self::Proj(arith) => Modular::q(arith),
            Self::Field(arith) => arith.q(),
        }
    }

    fn reduce(&self, x: u128) -> u128 {
        match self {
            Self::Proj(arith) => Modular::reduce(arith, x),
            Self::Field(arith) => arith.reduce(x),
        }
    }

    fn add(&self, a: u128, b: u128) -> u128 {
        match self {
            Self::Proj(arith) => Modular::add(arith, a, b),
            Self::Field(arith) => arith.add(a, b),
        }
    }

    fn mul(&self, a: u128, b: u128) -> u128 {
        match self {
            Self::Proj(arith) => Modular::mul(arith, a, b),
            Self::Field(arith) => arith.mul(a, b),
        }
    }

    fn fixed(&self, b: u128) -> Self::Fixed {
        match self {
            Self::Proj(arith) => ArithFixed::Proj(Modular::fixed(arith, b)),
            Self::Field(arith) => ArithFixed::Field(arith.fixed(b)),
        }
    }

    fn mul_fixed(&self, a: u128, b: &Self::Fixed) -> u128 {
        match (self, b) {
            (Self::Proj(arith), ArithFixed::Proj(b)) => Modular::mul_fixed(arith, a, b),
            (Self::Field(arith), ArithFixed::Field(b)) => arith.mul_fixed(a, b),
            _ => unreachable!("a fixed multiplier belongs to the arithmetic that built it"),
        }
    }
}

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

/// Where a nonzero Spartan scale goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScaleSide {
    /// Onto the folded row factors; the clear column table stays the raw
    /// equality table (the integer-multiplication relations).
    Rows,
    /// Onto the clear column table; the row factors stay unscaled (the
    /// CM-AND relation).
    Columns,
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
pub fn bitify<A: Modular>(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    params: IntegerMatrixLayout,
    gate_vars: usize,
    table: &BlockTable,
    scale_side: ScaleSide,
    arith: &A,
) -> Result<BitifiedClaim, ProtocolError> {
    let q = arith.q();
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
    let sub = |left: u128, right: u128| -> u128 { arith.sub(left, right) };
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

    // At a block point where the variable part vanishes the F2Z protocol
    // still needs a nonempty row functional: a deterministic dummy row with
    // an all-zero clear read-off.
    if factors.iter().all(|factor| *factor == Fq(0)) {
        if adjusted_claim != Fq(0) {
            return Err(ProtocolError::InvalidConstantOnlyClaim);
        }
        return Ok(BitifiedClaim {
            params,
            gate_point,
            rows: BitifiedRows::ConstantDummy,
            col_scale: Fq(0),
            claimed: adjusted_claim,
        });
    }

    let (rows, col_scale) = match scale_side {
        // Put a nonzero Spartan scale on the folded row side, avoiding a
        // dense column-table scaling pass. A zero scale stays on the clear
        // side so it does not erase the row functional that exponent
        // folding certifies.
        ScaleSide::Rows if scale == Fq(0) => (BitifiedRows::Structured(factors), Fq(0)),
        ScaleSide::Rows if scale == one => (BitifiedRows::Structured(factors), one),
        ScaleSide::Rows => (
            BitifiedRows::Structured(factors.into_iter().map(|factor| mul(scale, factor)).collect()),
            one,
        ),
        // The scale rides the clear column table; the rows stay unscaled.
        ScaleSide::Columns => (BitifiedRows::Structured(factors), scale),
    };

    Ok(BitifiedClaim {
        params,
        gate_point,
        rows,
        col_scale,
        claimed: adjusted_claim,
    })
}

/// Little-endian equality table with one fixed-factor multiplication per
/// parent and one allocation for the complete table.
pub fn eq_le_table_fq_fast_with<A: Modular>(point: &[Fq], arith: &A) -> Result<Vec<Fq>, ProtocolError> {
    let table_len = checked_pow2(point.len())?;
    let mut table = vec![Fq(0); table_len];
    table[0] = Fq(1);

    let q = arith.q();
    let mut half = 1_usize;
    for &coordinate in point {
        let active_len = half
            .checked_mul(2)
            .ok_or(ProtocolError::InvalidF2zParameters)?;
        let factor = arith.fixed(coordinate.0);
        let (zero_children, one_children) = table[..active_len].split_at_mut(half);
        let expand = |zero: &mut Fq, one: &mut Fq| {
            let parent = zero.0;
            let one_child = arith.mul_fixed(parent, &factor);
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
/// multiplication of the shared `eq` table.
fn structured_row_weights<A: Modular>(
    params: &IntegerMatrixLayout,
    gate_high: &[Fq],
    table: &BlockTable,
    factors: &[Fq],
    arith: &A,
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
            let factor = arith.fixed(scalar);
            for (row, equality) in rows.iter_mut().zip(&eq_high) {
                *row = arith.mul_fixed(equality.0, &factor);
            }
        });
    Ok(weights)
}

/// The dense canonical row weights of an opening (the dummy row functional
/// is `e_0`).
pub fn dense_row_weights<A: Modular>(
    opening: &BitifiedClaim,
    table: &BlockTable,
    arith: &A,
) -> Result<Vec<u128>, ProtocolError> {
    match &opening.rows {
        BitifiedRows::ConstantDummy => {
            let mut weights = vec![0_u128; checked_pow2(opening.params.row_vars)?];
            weights[0] = 1;
            Ok(weights)
        }
        BitifiedRows::Structured(factors) => {
            structured_row_weights(&opening.params, opening.gate_high(), table, factors, arith)
        }
    }
}

/// Compiles only the folded row functional into the mod-q chunk
/// representation. The prover never reads the clear column weights or the
/// claimed value, so keeping those verifier-only avoids an entire `2^s`
/// equality table on the proving path.
pub fn prepare_chunks<A: Modular>(
    opening: &BitifiedClaim,
    table: &BlockTable,
    q_bits: usize,
    arith: &A,
) -> Result<ModQWeightChunks, ProtocolError> {
    let params = opening.params;
    if opening.gate_point.len() < params.col_vars {
        return Err(ProtocolError::InvalidF2zParameters);
    }

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
            let weights =
                structured_row_weights(&params, opening.gate_high(), table, factors, arith)?;
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
pub fn column_weights<A: Modular>(opening: &BitifiedClaim, arith: &A) -> Result<Vec<Fq>, ProtocolError> {
    let params = opening.params;
    if opening.col_scale == Fq(0) {
        return Ok(vec![Fq(0); checked_pow2(params.col_vars)?]);
    }
    let mut eq_low = eq_le_table_fq_fast_with(opening.gate_low(), arith)?;
    if opening.col_scale != Fq(1) {
        let factor = arith.fixed(opening.col_scale.0);
        cfg_iter_mut!(&mut eq_low, 256).for_each(|weight| {
            weight.0 = arith.mul_fixed(weight.0, &factor);
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
