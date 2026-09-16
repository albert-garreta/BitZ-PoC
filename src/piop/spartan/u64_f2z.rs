//! The u64 multiplication relation `x · y = z_lo + 2^64 · z_hi` over F2Z.
//!
//! The integer R1CS assignment has five logical blocks,
//!
//! ```text
//! [constant block | x block | y block | z_lo block | z_hi block],
//! ```
//!
//! and is zero-padded to eight blocks for Spartan's assignment MLE. The
//! commitment stores four 64-bit little-endian values per multiplication in
//! 256 physical slots. This module describes that relation to the shared
//! protocol of [`super::protocol`]: the assignment, native operands and
//! split-limb products remain borrowed. Mixed first-round kernels fuse
//! projection with accumulation and folding.

use field::RingOps;
use flock_core::pcs::{commit::Commitment, ligerito::ProverConfig as LigProverConfig};

use crate::{
    ligerito::LOG_PACKING,
    ligerito_flock::{FlockCommitHint, ModQOpeningKind},
    pcs::IntegerMatrixLayout,
    transcript::traits::Transcript,
};

use super::{
    profile::{IopInstanceFacts, IopSecurityParams},
    protocol::{
        self, BindingHasher, BlockTable, Domains, FieldConfig, Kernel, MatrixSource, PiopWitness,
        PreparedRelation, Proof, ProtocolError, RelationSpec, SlotRange, checked_pow2,
        packed_variables,
    },
    raw_monty::NativeWideProducts,
    u64_mul::{
        U64_MUL_BIT_SLOTS, U64_MUL_LIMB_BASE, U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS,
        U64_MUL_PADDED_ASSIGNMENT_BLOCKS, U64_MUL_SLOT_VARS, U64_MUL_VALUE_BITS,
        U64_MUL_X_SLOT_START, U64_MUL_Y_SLOT_START, U64_MUL_Z_HI_SLOT_START,
        U64_MUL_Z_LO_SLOT_START, U64MulCoefficient, U64MulError, U64MulLayout, U64MulWitness,
        u64_mul_constraint_matrices,
    },
};

/// Failures in layout validation, claim translation, or either proof system.
pub type U64MulSpartanF2zError = ProtocolError;

/// The factorized claim bound between Spartan and F2Z.
pub type U64MulBitifiedClaim = protocol::BitifiedClaim;

impl From<U64MulError> for ProtocolError {
    fn from(error: U64MulError) -> Self {
        Self::relation(error)
    }
}

const BINDING_DOMAIN: &[u8] = b"f2z/spartan-u64-f2z/assignment/v1-runtime";
const ASSIGNMENT_BLOCK_ORDER: &[u8] = b"e0|x|y|zlo|zhi|zero|zero|zero";

static U64_MUL_DOMAINS: Domains = Domains {
    statement_tag: b"u64-mul-statement",
    prime_sampling: b"f2z/spartan-u64-mul/runtime-prime/v1",
    initial_grinding: b"f2z/spartan-u64-mul/grinding/initial/v1",
    piop_grinding: b"f2z/spartan-u64-mul/grinding/piop/v1",
    terminal_grinding: b"f2z/spartan-u64-mul/grinding/terminal/v1",
    bitified_claim: b"f2z/spartan-u64-f2z/bitified-claim/v1",
    opening: ModQOpeningKind::U64Mul,
    claim_tag: b"",
    reduction_grinding: b"",
    reduction_prime: b"",
    scopes: crate::protocol_scopes!("u64-spartan-f2z"),
};

/// The public statement facts the security-profile derivation consumes for
/// a u64 multiplication batch: per-row integer defects are below `2^130`.
pub fn u64_mul_instance_facts(params: &IntegerMatrixLayout, row_vars: usize) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: 130,
        lift_arity_log2: params.row_vars as u32,
        opening_t: params.row_vars as u32,
        opening_word_bits: params.word_bits as u32,
        direct_opening: true,
        tau_arity: row_vars.max(1) as u32,
        piop_degree: 3,
        step50_magnitude_log2: 0,
    }
}

fn validate_layout_geometry(layout: &U64MulLayout) -> Result<(), ProtocolError> {
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
            .checked_add(U64_MUL_SLOT_VARS)
            .ok_or(ProtocolError::InvalidF2zParameters)?
        || U64_MUL_BIT_SLOTS != 1_usize << U64_MUL_SLOT_VARS
        || U64_MUL_BIT_SLOTS != 4 * U64_MUL_VALUE_BITS
        || layout.assignment_len() != U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS * layout.capacity()
        || layout.padded_assignment_len() != U64_MUL_PADDED_ASSIGNMENT_BLOCKS * layout.capacity()
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }

    let row_count = checked_pow2(params.row_vars)?;
    let col_count = checked_pow2(params.col_vars)?;
    let cells = row_count
        .checked_mul(col_count)
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    let expected_cells = U64_MUL_BIT_SLOTS
        .checked_mul(layout.capacity())
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    // `packed_variables` counts one packed variable per 128 bits: the eight
    // slot variables leave one extra packed variable on top of the gates.
    if cells != expected_cells
        || packed_variables(&params)? != layout.gate_vars() + (U64_MUL_SLOT_VARS - LOG_PACKING)
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    Ok(())
}

impl RelationSpec for U64MulLayout {
    type Coefficient = U64MulCoefficient;
    type Witness = U64MulWitness;
    type Map = crate::f2map::RepeatedVirtualMap;

    fn domains(&self) -> &'static Domains {
        &U64_MUL_DOMAINS
    }

    fn committed_layout(&self) -> IntegerMatrixLayout {
        self.f2z_params()
    }

    fn gate_vars(&self) -> usize {
        U64MulLayout::gate_vars(self)
    }

    fn instance_facts(&self) -> IopInstanceFacts {
        let row_vars = self.multiplications().next_power_of_two().trailing_zeros() as usize;
        u64_mul_instance_facts(&self.f2z_params(), row_vars)
    }

    fn matrices(&self) -> Result<MatrixSource<U64MulCoefficient>, ProtocolError> {
        MatrixSource::skeleton(u64_mul_constraint_matrices(self)?)
    }

    fn validate_geometry(&self) -> Result<(), ProtocolError> {
        validate_layout_geometry(self)
    }

    /// Little-endian block-selector order is 000=e0, 001=x, 010=y,
    /// 011=z_lo, 100=z_hi, and 101..111 are public zero padding.
    fn block_table(&self) -> BlockTable {
        let block = |bit_slot_start: usize| {
            Some(SlotRange {
                bit_slot_start,
                bit_count: U64_MUL_VALUE_BITS,
            })
        };
        BlockTable::new(
            3,
            vec![
                None,
                block(U64_MUL_X_SLOT_START),
                block(U64_MUL_Y_SLOT_START),
                block(U64_MUL_Z_LO_SLOT_START),
                block(U64_MUL_Z_HI_SLOT_START),
                None,
                None,
                None,
            ],
        )
        .expect("the u64 block table is complete")
    }

    fn kernel(&self) -> Kernel {
        Kernel::Plain
    }

    fn check_witness(&self, witness: &U64MulWitness) -> Result<(), ProtocolError> {
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
        hasher.bytes(&U64_MUL_LIMB_BASE.to_le_bytes());
        hasher.usizes(&[
            self.multiplications(),
            self.capacity(),
            self.assignment_len(),
            self.padded_assignment_len(),
            U64MulLayout::gate_vars(self),
            U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS,
            U64_MUL_PADDED_ASSIGNMENT_BLOCKS,
            U64_MUL_VALUE_BITS,
            U64_MUL_X_SLOT_START,
            U64_MUL_Y_SLOT_START,
            U64_MUL_Z_LO_SLOT_START,
            U64_MUL_Z_HI_SLOT_START,
            U64_MUL_BIT_SLOTS,
            p.row_vars,
            p.col_vars,
            p.word_bits,
        ])?;
        Ok(hasher.finalize())
    }

    fn hash_bridge_constants(&self, hasher: &mut BindingHasher) -> Result<(), ProtocolError> {
        let p = self.f2z_params();
        hasher.bytes(&U64_MUL_LIMB_BASE.to_le_bytes());
        hasher.usizes(&[
            self.multiplications(),
            self.capacity(),
            self.assignment_len(),
            self.padded_assignment_len(),
            U64MulLayout::gate_vars(self),
            U64_MUL_LOGICAL_ASSIGNMENT_BLOCKS,
            U64_MUL_PADDED_ASSIGNMENT_BLOCKS,
            p.row_vars,
            p.col_vars,
            p.word_bits,
            U64_MUL_X_SLOT_START,
            U64_MUL_Y_SLOT_START,
            U64_MUL_Z_LO_SLOT_START,
            U64_MUL_Z_HI_SLOT_START,
            U64_MUL_VALUE_BITS,
            U64_MUL_BIT_SLOTS,
        ])?;
        // Mapping version one: little-endian bits, e0/x/y/zlo/zhi/zero/zero/zero
        // assignment order, raw z_hi reconstruction, and nonzero scale
        // normalized onto the folded row factors.
        hasher.bytes(&[1, 0, 1, 2, 3, 4, 5, 6, 7]);
        Ok(())
    }

    /// Borrow native assignment and split products without projection tables.
    fn piop_witness<'w>(
        &self,
        witness: &'w U64MulWitness,
        _config: &FieldConfig,
    ) -> Result<PiopWitness<'w>, ProtocolError> {
        let live = self.multiplications();
        let products = NativeWideProducts::new(
            &witness.x_values()[..live],
            &witness.y_values()[..live],
            &witness.z_lo_values()[..live],
            &witness.z_hi_values()[..live],
            live.next_power_of_two(),
        );
        Ok(PiopWitness::NativeU64 {
            products,
            assignment: witness.assignment(),
            constant_prefix: Some(super::raw_monty::NativeConstantPrefix::new(
                witness.layout().capacity(),
            )),
        })
    }
}

/// Setup-once, prime-independent bundle for the u64 protocol.
pub type PreparedU64MulRelation = PreparedRelation<U64MulLayout>;

/// A u64 multiplication proof over a transcript-selected prime.
pub type U64MulProof = Proof;

/// Commits prebuilt compact bit rows under the prepared relation's
/// profile-selected Ligerito configuration.
pub fn commit_u64_mul_witness(
    prepared: &PreparedU64MulRelation,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, U64MulSpartanF2zError> {
    protocol::commit(prepared, rows)
}

/// Proves the u64 batch under the prepared relation's security profile.
pub fn prove_u64_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU64MulRelation,
    witness: &U64MulWitness,
    hint: &FlockCommitHint,
) -> Result<U64MulProof, U64MulSpartanF2zError> {
    protocol::prove(transcript, prepared, witness, hint)
}

/// Verifies a u64 multiplication proof, re-deriving the prime from the
/// bound transcript.
pub fn verify_u64_mul<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedU64MulRelation,
    commitment: &Commitment,
    proof: &U64MulProof,
) -> Result<(), U64MulSpartanF2zError> {
    protocol::verify(transcript, prepared, commitment, proof)
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
        assert_eq!(prepared.params().row_vars, 8 + 15 - 7);
        let hint = commit_u64_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_u64_mul(&mut prover_transcript, &prepared, &witness, &hint).unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u64_mul(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
        )
        .unwrap();
        assert!(proof.size_bytes(prepared.security()) > 0);

        let mut second_transcript = Blake3Transcript::new();
        let second = prove_u64_mul(&mut second_transcript, &prepared, &witness, &hint).unwrap();
        assert_eq!(proof.f2z().to_bytes(), second.f2z().to_bytes());
        assert_eq!(proof.piop_nonces(), second.piop_nonces());
    }

    #[test]
    fn u64_paper_path_rejects_a_wrong_product() {
        let _env = crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let honest = witness(1 << 15, 1);
        let layout = *honest.layout();
        let prepared = PreparedU64MulRelation::new(layout).unwrap();

        // Flip the committed z_hi bit of gate 3: the committed bits and the
        // projected products no longer satisfy x·y = z_lo + 2^64·z_hi.
        let mut rows = honest.f2z_bit_rows();
        let (b, c) = layout.f2z_cell(U64_MUL_Z_HI_SLOT_START, 3).unwrap();
        rows[c][b / 64] ^= 1 << (b % 64);
        let hint = commit_u64_mul_witness(&prepared, rows).unwrap();

        // An honest prover with a mismatching commitment must not produce a
        // verifying proof.
        let mut prover_transcript = Blake3Transcript::new();
        let outcome = prove_u64_mul(&mut prover_transcript, &prepared, &honest, &hint);
        if let Ok(proof) = outcome {
            let mut verifier_transcript = Blake3Transcript::new();
            assert!(
                verify_u64_mul(
                    &mut verifier_transcript,
                    &prepared,
                    &hint.commitment,
                    &proof
                )
                .is_err()
            );
        }
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
