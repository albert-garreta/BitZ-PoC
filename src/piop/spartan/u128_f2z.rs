//! The u128 multiplication relation `x · y = z` (`z < 2^256`) over F2Z.
//!
//! The integer R1CS assignment has four blocks, `[e0 | x | y | z]`, with
//! `x, y < 2^128` and `z < 2^256`, so the assignment MLE needs no padding
//! and the block selector is two coordinates. The commitment stores the
//! 512 bits of one multiplication per gate (`x`, `y`, then `z`). This
//! module describes that relation to the shared protocol of
//! [`super::protocol`]: the Spartan PIOP runs on residues built straight
//! from the witness limbs (both the products and the assignment, since
//! 128- and 256-bit values have no native `u64` first round).

use flock_core::pcs::{commit::Commitment, ligerito::ProverConfig as LigProverConfig};

use crate::{
    ligerito::LOG_PACKING,
    ligerito_flock::{FlockCommitHint, ModQOpeningKind},
    pcs::IntegerMatrixLayout,
    transcript::traits::Transcript,
    utils::{cfg_iter, cfg_iter_mut},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    profile::{IopInstanceFacts, IopSecurityParams},
    protocol::{
        self, BindingHasher, BlockTable, Domains, Kernel, PiopWitness, PreparedRelation, Proof,
        ProtocolError, ProveOptions, RelationSpec, SlotRange, MatrixSource, FieldConfig, checked_pow2, packed_variables,
    },
    raw_monty::{RawMontyCtx, RawProducts, RawWitness},
    u128_mul::{
        U128_MUL_ASSIGNMENT_BLOCKS, U128_MUL_BIT_SLOTS, U128_MUL_OPERAND_BITS,
        U128_MUL_PRODUCT_BITS, U128_MUL_SLOT_VARS, U128_MUL_X_SLOT_START, U128_MUL_Y_SLOT_START,
        U128_MUL_Z_SLOT_START, U128MulError, U128MulLayout, U128MulWitness,
        u128_mul_constraint_matrices,
    },
};

/// Failures in layout validation, claim translation, or either proof system.
pub type U128MulSpartanF2zError = ProtocolError;

/// The factorized claim bound between Spartan and F2Z.
pub type U128MulBitifiedClaim = protocol::BitifiedClaim;

impl From<U128MulError> for ProtocolError {
    fn from(error: U128MulError) -> Self {
        Self::relation(error)
    }
}

const BINDING_DOMAIN: &[u8] = b"f2z/spartan-u128-f2z/assignment/v1-runtime";
const ASSIGNMENT_BLOCK_ORDER: &[u8] = b"e0|x|y|z";

static U128_MUL_DOMAINS: Domains = Domains {
    statement_tag: b"u128-mul-statement",
    prime_sampling: b"f2z/spartan-u128-mul/runtime-prime/v1",
    initial_grinding: b"f2z/spartan-u128-mul/grinding/initial/v1",
    piop_grinding: b"f2z/spartan-u128-mul/grinding/piop/v1",
    terminal_grinding: b"f2z/spartan-u128-mul/grinding/terminal/v1",
    bitified_claim: b"f2z/spartan-u128-f2z/bitified-claim/v1",
    opening: ModQOpeningKind::U128Mul,
    claim_tag: b"",
    reduction_grinding: b"",
    reduction_prime: b"",
    scopes: crate::protocol_scopes!("u128-spartan-f2z"),
};

/// The public statement facts the security-profile derivation consumes for
/// a u128 multiplication batch: per-row integer defects are below `2^258`.
pub fn u128_mul_instance_facts(params: &IntegerMatrixLayout, row_vars: usize) -> IopInstanceFacts {
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

fn validate_layout_geometry(layout: &U128MulLayout) -> Result<(), ProtocolError> {
    let params = layout.f2z_params();
    if params.word_bits != 1
        || params.row_vars < LOG_PACKING
        || params.col_vars > layout.gate_vars()
        || params.row_vars.saturating_add(params.word_bits) > 126
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    let total_vars = params
        .row_vars
        .checked_add(params.col_vars)
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    if total_vars
        != layout
            .gate_vars()
            .checked_add(U128_MUL_SLOT_VARS)
            .ok_or(ProtocolError::InvalidF2zParameters)?
        || U128_MUL_BIT_SLOTS != 1_usize << U128_MUL_SLOT_VARS
        || U128_MUL_BIT_SLOTS != 2 * U128_MUL_OPERAND_BITS + U128_MUL_PRODUCT_BITS
        || layout.assignment_len() != U128_MUL_ASSIGNMENT_BLOCKS * layout.capacity()
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(params.row_vars)?;
    let col_count = checked_pow2(params.col_vars)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    let expected_cells = U128_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    // `packed_variables` counts one packed variable per 128 bits: the nine
    // slot variables leave two extra packed variables on top of the gates.
    if cells != expected_cells
        || packed_variables(&params)? != layout.gate_vars() + (U128_MUL_SLOT_VARS - LOG_PACKING)
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    Ok(())
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

impl RelationSpec for U128MulLayout {
    type Coefficient = bool;
    type Witness = U128MulWitness;
    type Map = crate::f2map::RepeatedVirtualMap;

    fn domains(&self) -> &'static Domains {
        &U128_MUL_DOMAINS
    }

    fn committed_layout(&self) -> IntegerMatrixLayout {
        self.f2z_params()
    }

    fn gate_vars(&self) -> usize {
        U128MulLayout::gate_vars(self)
    }

    fn instance_facts(&self) -> IopInstanceFacts {
        let row_vars = self.multiplications().next_power_of_two().trailing_zeros() as usize;
        u128_mul_instance_facts(&self.f2z_params(), row_vars)
    }

    fn matrices(&self) -> Result<MatrixSource<bool>, ProtocolError> {
        MatrixSource::skeleton(u128_mul_constraint_matrices(self)?)
    }

    fn validate_geometry(&self) -> Result<(), ProtocolError> {
        validate_layout_geometry(self)
    }

    /// Block order is 00=e0, 01=x, 10=y, 11=z with little-endian bit weights
    /// `2^0 … 2^255` on the committed slots.
    fn block_table(&self) -> BlockTable {
        BlockTable::new(
            2,
            vec![
                None,
                Some(SlotRange {
                    bit_slot_start: U128_MUL_X_SLOT_START,
                    bit_count: U128_MUL_OPERAND_BITS,
                }),
                Some(SlotRange {
                    bit_slot_start: U128_MUL_Y_SLOT_START,
                    bit_count: U128_MUL_OPERAND_BITS,
                }),
                Some(SlotRange {
                    bit_slot_start: U128_MUL_Z_SLOT_START,
                    bit_count: U128_MUL_PRODUCT_BITS,
                }),
            ],
        )
        .expect("the u128 block table is complete")
    }

    fn kernel(&self) -> Kernel {
        Kernel::Plain
    }

    fn check_witness(&self, witness: &U128MulWitness) -> Result<(), ProtocolError> {
        if witness.layout() != self {
            return Err(ProtocolError::RelationWitnessLayoutMismatch);
        }
        Ok(())
    }

    fn assignment_binding(
        &self,
        commitment: &Commitment,
        security: &IopSecurityParams,
        _ligerito: &LigProverConfig,
    ) -> Result<[u8; 32], ProtocolError> {
        let p = self.f2z_params();
        let mut hasher = BindingHasher::new();
        hasher
            .bytes(BINDING_DOMAIN)
            .bytes(ASSIGNMENT_BLOCK_ORDER)
            .bytes(&commitment.root);
        hasher.commitment_params(&commitment.params)?;
        hasher
            .u128_le(security.projection_min)
            .u128_le(security.projection_max);
        hasher.u32(security.lambda)?;
        hasher.usize(security.ligerito_target_bits)?;
        hasher.u32(security.initial_grinding_bits)?;
        hasher.u32(security.piop_round_grinding_bits)?;
        hasher.u32(security.terminal_grinding_bits)?;
        hasher.u32(security.forest_round_grinding_bits)?;
        hasher.usizes(&[
            self.multiplications(),
            self.capacity(),
            self.assignment_len(),
            U128MulLayout::gate_vars(self),
            U128_MUL_ASSIGNMENT_BLOCKS,
            U128_MUL_OPERAND_BITS,
            U128_MUL_PRODUCT_BITS,
            U128_MUL_X_SLOT_START,
            U128_MUL_Y_SLOT_START,
            U128_MUL_Z_SLOT_START,
            U128_MUL_BIT_SLOTS,
            p.row_vars,
            p.col_vars,
            p.word_bits,
        ])?;
        Ok(hasher.finalize())
    }

    fn hash_bridge_constants(&self, hasher: &mut BindingHasher) -> Result<(), ProtocolError> {
        let p = self.f2z_params();
        hasher.usizes(&[
            self.multiplications(),
            self.capacity(),
            self.assignment_len(),
            U128MulLayout::gate_vars(self),
            U128_MUL_ASSIGNMENT_BLOCKS,
            p.row_vars,
            p.col_vars,
            p.word_bits,
            U128_MUL_X_SLOT_START,
            U128_MUL_Y_SLOT_START,
            U128_MUL_Z_SLOT_START,
            U128_MUL_OPERAND_BITS,
            U128_MUL_PRODUCT_BITS,
            U128_MUL_BIT_SLOTS,
        ])?;
        // Mapping version one: little-endian bits, e0/x/y/z block order, and
        // nonzero scale normalized onto the folded row factors.
        hasher.bytes(&[1, 0, 1, 2, 3]);
        Ok(())
    }

    /// Both the products and the assignment are raw residues built from the
    /// witness limbs.
    fn piop_witness<'w>(
        &self,
        witness: &'w U128MulWitness,
        config: &FieldConfig,
        _options: ProveOptions,
    ) -> Result<PiopWitness<'w>, ProtocolError> {
        let ctx = &RawMontyCtx::new(config);
        let live = self.multiplications();
        let products = RawProducts::from_native_u128_halves(
            ctx,
            &witness.x_values()[..live],
            &witness.y_values()[..live],
            &witness.z_lo_values()[..live],
            &witness.z_hi_values()[..live],
            live.next_power_of_two(),
        );
        let assignment = raw_assignment(ctx, witness);
        if assignment.len() != self.assignment_len() {
            return Err(ProtocolError::InvalidF2zParameters);
        }
        Ok(PiopWitness::RawProductsRaw {
            products,
            witness: RawWitness::Field(assignment),
        })
    }
}

/// Setup-once, prime-independent bundle for the u128 protocol.
pub type PreparedU128MulRelation = PreparedRelation<U128MulLayout>;

/// A u128 multiplication proof over a transcript-selected prime.
pub type U128MulProof = Proof;

/// Commits prebuilt compact bit rows under the prepared relation's
/// profile-selected Ligerito configuration.
pub fn commit_u128_mul_witness(
    prepared: &PreparedU128MulRelation,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, U128MulSpartanF2zError> {
    protocol::commit(prepared, rows)
}

/// Proves the u128 batch under the prepared relation's security profile.
pub fn prove_u128_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU128MulRelation,
    witness: &U128MulWitness,
    hint: &FlockCommitHint,
) -> Result<U128MulProof, U128MulSpartanF2zError> {
    protocol::prove(transcript, prepared, witness, hint)
}

/// Verifies a u128 multiplication proof, re-deriving the prime from the
/// bound transcript.
pub fn verify_u128_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU128MulRelation,
    commitment: &Commitment,
    proof: &U128MulProof,
) -> Result<(), U128MulSpartanF2zError> {
    protocol::verify(transcript, prepared, commitment, proof)
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
        assert_eq!(proof.piop_nonces(), second.piop_nonces());
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
