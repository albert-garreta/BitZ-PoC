//! Shared implementation and errors for Falcon-512 and Falcon-1024.

/// Domain identifier for the full Falcon verification protocol.
pub const PROTOCOL_ID: &str = "bitz/falcon/shared-prime/non-zk/v6";

/// A malformed encoding or failed Falcon verification.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum FalconError {
    #[error("Falcon public key must contain exactly {expected} bytes")]
    PublicKeyLength { expected: usize },
    #[error("Falcon public key has the wrong header")]
    PublicKeyHeader,
    #[error("Falcon public-key coefficient {index} is not canonical")]
    PublicKeyCoefficient { index: usize },
    #[error("Falcon CT signature must contain exactly {expected} bytes")]
    SignatureLength { expected: usize },
    #[error("Falcon CT signature has the wrong header")]
    SignatureHeader,
    #[error("Falcon CT coefficient {index} uses the forbidden signed-minimum encoding")]
    ForbiddenSignatureCoefficient { index: usize },
    #[error("Falcon CT coefficient {index} is outside its canonical signed range")]
    SignatureCoefficientOutOfRange { index: usize },
    #[error("constant-time HashToPoint produced only {accepted} accepted samples")]
    HashToPointUnderflow { accepted: usize },
    #[error("Falcon squared norm {actual} exceeds {bound}")]
    NormTooLarge { actual: u64, bound: u64 },
    #[error("unsupported Falcon batch size (expected 1..=65536)")]
    InvalidBatchCapacity,
    #[error("the Falcon source layout or opening exceeds its configured capacity")]
    SourceStrideOverflow,
    #[error("Falcon constraint family {family} failed at row {index}")]
    ConstraintViolation { family: &'static str, index: usize },
    #[error("Falcon PIOP failure: {0}")]
    Piop(String),
}

/// Hashes a public byte string in one call. The digest equals that of the
/// same bytes fed through smaller updates, without the per-call cost of
/// hashing coefficient by coefficient; large inputs use blake3's Rayon splitter.
pub(crate) fn hash_statement_bytes(h: &mut blake3::Hasher, bytes: &[u8]) {
    if bytes.len() >= 1 << 17 {
        h.update_rayon(bytes);
    } else {
        h.update(bytes);
    }
}
