//! Digest helpers shared by every relation's statement binding and bridge
//! digest: one BLAKE3 hasher with the canonical field encodings.

use blake3::Hasher;
use flock_core::{
    merkle::HashKind,
    pcs::{
        commit::{Commitment, PcsParams},
        ligerito::{LigeritoProfile, ProverConfig as LigProverConfig},
    },
};

use super::{ProtocolError, SpartanBitzField};
use crate::piop::spartan::profile::IopSecurityParams;

/// Stable one-byte code of a flock Ligerito profile.
pub const fn profile_code(profile: LigeritoProfile) -> u8 {
    match profile {
        LigeritoProfile::Fast => 0,
        LigeritoProfile::Slim => 1,
        LigeritoProfile::Secure => 2,
        LigeritoProfile::Slim3 => 3,
    }
}

/// Stable one-byte code of a flock Merkle hash.
pub const fn hash_code(hash: HashKind) -> u8 {
    match hash {
        HashKind::Sha256 => 0,
        HashKind::Blake3 => 1,
    }
}

/// A BLAKE3 hasher with the encodings every binding uses: raw bytes, host
/// lengths as `u64` little-endian words, `u128` little-endian words and
/// canonical 16-byte field elements.
pub struct BindingHasher {
    hasher: Hasher,
}

impl Default for BindingHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingHasher {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }

    /// Raw bytes, no framing.
    pub fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.hasher.update(bytes);
        self
    }

    /// One raw byte.
    pub fn byte(&mut self, byte: u8) -> &mut Self {
        self.hasher.update(&[byte]);
        self
    }

    /// A host length or count as a `u64` little-endian word.
    pub fn usize(&mut self, value: usize) -> Result<&mut Self, ProtocolError> {
        let value = u64::try_from(value).map_err(|_| ProtocolError::BindingEncodingOverflow)?;
        self.hasher.update(&value.to_le_bytes());
        Ok(self)
    }

    /// Several host lengths in order.
    pub fn usizes(&mut self, values: &[usize]) -> Result<&mut Self, ProtocolError> {
        for &value in values {
            self.usize(value)?;
        }
        Ok(self)
    }

    /// A `u32` parameter, encoded like a host length.
    pub fn u32(&mut self, value: u32) -> Result<&mut Self, ProtocolError> {
        self.usize(value as usize)
    }

    /// A `u64` little-endian word.
    pub fn u64_le(&mut self, value: u64) -> &mut Self {
        self.hasher.update(&value.to_le_bytes());
        self
    }

    /// A `u128` little-endian word.
    pub fn u128_le(&mut self, value: u128) -> &mut Self {
        self.hasher.update(&value.to_le_bytes());
        self
    }

    /// A canonical 16-byte field element.
    pub fn element(&mut self, value: &SpartanBitzField, field: &super::FieldConfig) -> &mut Self {
        let encoding = u128::from(field.to_integer(value)).to_le_bytes();
        self.hasher.update(&encoding);
        self
    }

    /// A length-prefixed byte string.
    pub fn prefixed(&mut self, bytes: &[u8]) -> Result<&mut Self, ProtocolError> {
        self.usize(bytes.len())?;
        self.hasher.update(bytes);
        Ok(self)
    }

    /// The public commitment parameters, in the order every binding uses.
    pub fn commitment_params(&mut self, params: &PcsParams) -> Result<&mut Self, ProtocolError> {
        self.usize(params.m)?;
        self.usize(params.log_inv_rate)?;
        self.usize(params.log_batch_size)?;
        self.byte(profile_code(params.profile));
        self.byte(hash_code(params.merkle_hash));
        Ok(self)
    }

    /// The complete Ligerito prover configuration.
    pub fn ligerito_config(
        &mut self,
        config: &LigProverConfig,
    ) -> Result<&mut Self, ProtocolError> {
        for value in [
            config.recursive_steps,
            config.initial_log_msg_cols,
            config.initial_log_num_interleaved,
            config.initial_k,
        ] {
            self.usize(value)?;
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
            self.usize(values.len())?;
            for &value in values {
                self.usize(value)?;
            }
        }
        self.byte(hash_code(config.merkle_hash));
        Ok(self)
    }

    pub fn finalize(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
}

/// `RelationSpec::check_witness`'s one real check, shared verbatim by every
/// relation: the witness's own layout must equal the layout it's being
/// checked against.
pub fn check_witness_layout<L: PartialEq>(
    layout: &L,
    witness_layout: &L,
) -> Result<(), ProtocolError> {
    if witness_layout != layout {
        return Err(ProtocolError::RelationWitnessLayoutMismatch);
    }
    Ok(())
}

/// The `assignment_binding` preamble shared by the BitZ mul relations whose
/// statement binding needs no per-relation security knobs beyond the common
/// ones (projection bounds, lambda, target bits, the three grinding-bit
/// counts): domain, block order, commitment root and params, then those
/// fields, in the fixed order every one of those relations already used.
/// `middle` appends whatever's relation-specific (width constants, the
/// layout's own dimensions, an optional `bind_packing` call) and the digest
/// is finalized after it returns.
///
/// Not shared by every `RelationSpec` impl: the u32 relation's binding
/// additionally covers profile name, optional reduction/OOD parameters and
/// the full Ligerito config, so it builds its own hasher from scratch
/// instead of calling this.
pub fn bind_assignment(
    binding_domain: &[u8],
    assignment_block_order: &[u8],
    commitment: &Commitment,
    security: &IopSecurityParams,
    middle: impl FnOnce(&mut BindingHasher) -> Result<(), ProtocolError>,
) -> Result<[u8; 32], ProtocolError> {
    let mut hasher = BindingHasher::new();
    hasher
        .bytes(binding_domain)
        .bytes(assignment_block_order)
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
    middle(&mut hasher)?;
    Ok(hasher.finalize())
}
