//! Shared implementation and errors for Falcon-512 and Falcon-1024.

/// Domain identifier for the full Falcon verification protocol.
pub const PROTOCOL_ID: &str = "bitz/falcon/shared-prime/non-zk/v6";

/// Hash public statement bytes without per-coefficient update overhead.
#[cfg(feature = "falcon-hybrid")]
pub(crate) fn hash_statement_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    if bytes.len() >= 1 << 17 {
        hasher.update_rayon(bytes);
    } else {
        hasher.update(bytes);
    }
}

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
    #[error("unsupported Falcon batch size (expected 1..=1024)")]
    InvalidBatchCapacity,
    #[error("the Falcon source layout or opening exceeds its configured capacity")]
    SourceStrideOverflow,
    #[error("Falcon constraint family {family} failed at row {index}")]
    ConstraintViolation { family: &'static str, index: usize },
    #[error("Falcon PIOP failure: {0}")]
    Piop(String),
}

#[cfg(all(test, feature = "falcon-hybrid"))]
mod tests {
    use super::hash_statement_bytes;

    #[test]
    fn bulk_statement_hash_preserves_streaming_transcript_digest() {
        let bytes: Vec<u8> = (0..4 * 1024 * 1024)
            .map(|i| (i * 37 + i / 251) as u8)
            .collect();
        for workers in [1, 2, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for len in [
                    0,
                    1,
                    2048,
                    (1 << 17) - 1,
                    1 << 17,
                    (1 << 17) + 1,
                    bytes.len(),
                ] {
                    // Prefix and suffix represent the surrounding transcript fields;
                    // the coefficient bytes need not start on a BLAKE3 block boundary.
                    let mut reference = blake3::Hasher::new();
                    reference.update(&[0x5a; 173]);
                    let mut bulk = reference.clone();
                    for coefficient in bytes[..len].chunks(2) {
                        reference.update(coefficient);
                    }
                    hash_statement_bytes(&mut bulk, &bytes[..len]);
                    reference.update(b"source-root-and-grinding-schedule");
                    bulk.update(b"source-root-and-grinding-schedule");
                    assert_eq!(
                        reference.finalize(),
                        bulk.finalize(),
                        "len={len}, workers={workers}"
                    );
                }
            });
        }
    }
}
