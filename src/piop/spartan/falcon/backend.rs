// Falcon constant-time verification over authenticated binary witnesses.
//
// The hybrid prover checks the Falcon equation in its native polynomial ring,
// authenticates its integer-polynomial projection,
// and shares one arithmetic binder with HashToPoint and norm reductions. Binary
// Keccak sources and the arithmetic source share the final opening.

mod constraints;
mod format;
mod hash_to_point;
mod hash_to_point_selection;
#[cfg(feature = "falcon-hybrid")]
mod hybrid;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_bridge;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_keccak;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_sumcheck;
mod keccak;
mod layout;
#[cfg(feature = "falcon-hybrid")]
mod opening;
#[cfg(feature = "falcon-hybrid")]
mod piop;
#[cfg(feature = "falcon-hybrid")]
mod ring_field;
#[cfg(feature = "falcon-hybrid")]
mod shared_ring;
mod source;
mod verify;
#[cfg(feature = "falcon-hybrid")]
pub use hybrid::{
    CommittedFalconHybrid, FalconHybridProof, FalconHybridSecurity, FalconHybridStatement,
    PreparedFalconHybrid,
};

pub use format::{
    FalconPublicKey, FalconSignatureCt, decode_public_key, decode_signature_ct, encode_signature_ct,
};
pub use hash_to_point::{HashToPointTrace, hash_to_point_ct};
pub use keccak::{KeccakTrace, shake256_with_trace};
pub use layout::{FalconSourceLayout, FalconTraceCounts};
#[cfg(feature = "falcon-hybrid")]
pub use opening::FalconPublicStatement;
#[cfg(feature = "falcon-hybrid")]
use piop::FalconPiopProof;
#[cfg(feature = "falcon-hybrid")]
pub use piop::FalconSecuritySchedule;
pub use source::{FalconSourceOffsets, FalconSourceWitness};
pub use verify::{FalconVerificationTrace, verification_trace, verify_falcon_ct};

pub const N: usize = DEGREE;
pub const PARAMETERS: crate::piop::spartan::falcon_parameters::FalconParameters =
    crate::piop::spartan::falcon_parameters::FalconParameters::for_degree(N);
pub const Q: i64 = 12_289;
pub const BETA_SQUARED: u64 = PARAMETERS.norm_bound;
pub const PUBLIC_KEY_BYTES: usize = PARAMETERS.public_key_bytes();
pub const CT_SIGNATURE_BYTES: usize = PARAMETERS.signature_bytes();
pub const NONCE_BYTES: usize = 40;
pub const HASH_TO_POINT_SAMPLES: usize = PARAMETERS.samples();
pub const SIGNATURE_BITS: usize = PARAMETERS.signature_bits;
pub const NORM_BITS: usize = PARAMETERS.norm_bits();
pub const COEFFICIENT_LOG: usize = N.ilog2() as usize;
pub const KECCAK_SLABS: usize = PARAMETERS.slab_count();
pub const SOURCE_COUNT: usize = 1 + KECCAK_SLABS;

pub use crate::piop::spartan::falcon::FalconError;
pub use constraints::{FalconConstraintCounts, check_exact_constraints};
