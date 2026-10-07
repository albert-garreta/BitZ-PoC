// Falcon constant-time verification over authenticated binary witnesses.
//
// The hybrid prover checks the Falcon equation in its native polynomial ring,
// authenticates the ideal projection through bounded integer coordinate carries,
// and shares one arithmetic binder with HashToPoint and norm reductions. Binary
// Keccak sources and the arithmetic source share the final opening.

mod constraints;
mod format;
mod hash_to_point;
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
mod native_ring;
mod opening;
mod piop;
mod shared_ring;
mod source;
mod verify;
#[cfg(feature = "falcon-hybrid")]
pub use hybrid::{
    CommittedFalconHybrid, FalconHybridProof, FalconHybridSecurity, FalconHybridStatement,
    FalconProtocol, PreparedFalconHybrid,
};

pub use format::{
    FalconPublicKey, FalconSignatureCt, decode_public_key, decode_signature_ct, encode_signature_ct,
};
pub use hash_to_point::{HashToPointTrace, hash_to_point_ct};
pub use keccak::{KeccakTrace, shake256_with_trace};
pub use layout::{FalconSourceLayout, FalconTraceCounts};
pub use native_ring::{Ext as FalconNativeExtension, NativeRingProof as FalconNativeRingProof};
pub use opening::FalconPublicStatement;
pub use piop::{FalconPiopProof, FalconSecuritySchedule, prove_falcon_piop, verify_falcon_piop};
pub use source::{FalconSourceOffsets, FalconSourceWitness};
pub use verify::{FalconVerificationTrace, verification_trace, verify_falcon1024_ct};

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
pub const PREFIX_BIAS: usize = PARAMETERS.prefix_bias();
pub const PREFIX_BITS: usize = PARAMETERS.prefix_bits();
pub const NORM_BITS: usize = PARAMETERS.norm_bits();
pub const COEFFICIENT_LOG: usize = N.ilog2() as usize;
pub const COMPACTION_LOG: usize = PARAMETERS.compaction_log();
pub const KECCAK_SLABS: usize = PARAMETERS.slab_count();
pub const SOURCE_COUNT: usize = 1 + KECCAK_SLABS;

/// A malformed encoding or failed Falcon verification.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum FalconError {
    #[error("Falcon public key must contain exactly {PUBLIC_KEY_BYTES} bytes")]
    PublicKeyLength,
    #[error("Falcon public key has the wrong header")]
    PublicKeyHeader,
    #[error("nonzero unused encoding bits")]
    NonCanonicalPadding,
    #[error("Falcon public-key coefficient {index} is not canonical")]
    PublicKeyCoefficient { index: usize },
    #[error("Falcon CT signature must contain exactly {CT_SIGNATURE_BYTES} bytes")]
    SignatureLength,
    #[error("Falcon CT signature has the wrong header")]
    SignatureHeader,
    #[error("Falcon CT coefficient {index} uses the forbidden signed-minimum encoding")]
    ForbiddenSignatureCoefficient { index: usize },
    #[error("Falcon CT coefficient {index} is outside its canonical signed range")]
    SignatureCoefficientOutOfRange { index: usize },
    #[error("constant-time HashToPoint produced only {accepted} accepted samples")]
    HashToPointUnderflow { accepted: usize },
    #[error("Falcon squared norm {actual} exceeds {BETA_SQUARED}")]
    NormTooLarge { actual: u64 },
    #[error("unsupported Falcon batch size (expected 1..=1024)")]
    InvalidBatchCapacity,
    #[error("the Falcon source layout or opening exceeds its configured capacity")]
    SourceStrideOverflow,
    #[error("Falcon constraint family {family} failed at row {index}")]
    ConstraintViolation { family: &'static str, index: usize },
    #[error("Falcon PIOP failure: {0}")]
    Piop(String),
}
pub use constraints::{FalconConstraintCounts, check_exact_constraints};
