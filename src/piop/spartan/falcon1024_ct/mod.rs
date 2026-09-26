//! Falcon-1024 constant-time verification, expressed as an exact-integer
//! trace over one binary source witness.
//!
//! Native trace construction stays independent of the proof modulus: every
//! value is decoded from bits and every arithmetic identity is formed over
//! the integers before projection. The commitment-bound adapter then runs the
//! Spartan reductions and opens their shared terminal against the same BitZ
//! binary commitment.

mod constraints;
mod format;
mod hash_to_point;
#[cfg(feature = "falcon-hybrid")]
mod hybrid;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_bridge;
#[cfg(feature = "falcon-hybrid")]
pub mod hybrid_hash_to_point;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_keccak;
#[cfg(feature = "falcon-hybrid")]
mod hybrid_sumcheck;
mod keccak;
mod layout;
mod opening;
mod piop;
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
pub use opening::{FalconBitzProof, FalconPublicStatement, prove_falcon_bitz, verify_falcon_bitz};
pub use piop::{FalconPiopProof, FalconSecuritySchedule, prove_falcon_piop, verify_falcon_piop};
pub use source::{
    FalconSourceOffsets, FalconSourceWitness, commit_falcon_source, falcon_ligerito_configs,
};
pub use verify::{FalconVerificationTrace, verification_trace, verify_falcon1024_ct};

/// Falcon-1024 cyclotomic degree.
pub const N: usize = 1024;
/// Falcon modulus.
pub const Q: i64 = 12_289;
/// Falcon-1024 squared norm bound.
pub const BETA_SQUARED: u64 = 70_265_242;
/// Encoded Falcon-1024 public-key size.
pub const PUBLIC_KEY_BYTES: usize = 1_793;
/// Encoded Falcon-1024 constant-time signature size.
pub const CT_SIGNATURE_BYTES: usize = 1_577;
/// Falcon nonce size.
pub const NONCE_BYTES: usize = 40;
/// Constant-time HashToPoint sample count (`1024 + 287`).
pub const HASH_TO_POINT_SAMPLES: usize = 1_311;

/// A malformed encoding or failed Falcon verification.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum FalconError {
    #[error("Falcon-1024 public key must contain exactly {PUBLIC_KEY_BYTES} bytes")]
    PublicKeyLength,
    #[error("Falcon-1024 public key has the wrong header")]
    PublicKeyHeader,
    #[error("Falcon public-key coefficient {index} is not canonical")]
    PublicKeyCoefficient { index: usize },
    #[error("Falcon-1024 CT signature must contain exactly {CT_SIGNATURE_BYTES} bytes")]
    SignatureLength,
    #[error("Falcon-1024 CT signature has the wrong header")]
    SignatureHeader,
    #[error("Falcon CT coefficient {index} uses the forbidden -2048 encoding")]
    ForbiddenSignatureCoefficient { index: usize },
    #[error("Falcon CT coefficient {index} is outside -2047..=2047")]
    SignatureCoefficientOutOfRange { index: usize },
    #[error("constant-time HashToPoint produced only {accepted} accepted samples")]
    HashToPointUnderflow { accepted: usize },
    #[error("Falcon squared norm {actual} exceeds {BETA_SQUARED}")]
    NormTooLarge { actual: u64 },
    #[error("unsupported Falcon batch size (legacy: 1..=32; hybrid: 1..=1024)")]
    InvalidBatchCapacity,
    #[error("the Falcon source layout or opening exceeds its configured capacity")]
    SourceStrideOverflow,
    #[error("Falcon constraint family {family} failed at row {index}")]
    ConstraintViolation { family: &'static str, index: usize },
    #[error("Falcon PIOP failure: {0}")]
    Piop(String),
}
pub use constraints::{FalconConstraintCounts, check_exact_constraints};
