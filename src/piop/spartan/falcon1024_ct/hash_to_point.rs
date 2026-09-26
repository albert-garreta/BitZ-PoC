use super::{FalconError, HASH_TO_POINT_SAMPLES, N, Q, keccak::KeccakTrace, shake256_with_trace};

/// Exact-integer witness for Falcon's constant-time HashToPoint reduction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HashToPointTrace {
    pub shake: KeccakTrace,
    pub words: Box<[u16; HASH_TO_POINT_SAMPLES]>,
    pub quotients: Box<[u8; HASH_TO_POINT_SAMPLES]>,
    pub remainders: Box<[u16; HASH_TO_POINT_SAMPLES]>,
    pub accepted: Box<[bool; HASH_TO_POINT_SAMPLES]>,
    /// Number of accepted candidates before each candidate, followed by the
    /// final accepted count.  These ranks drive the compaction fingerprint.
    pub prefix: Box<[u16; HASH_TO_POINT_SAMPLES + 1]>,
    pub point: Box<[u16; N]>,
}

/// Hashes `nonce || message`, draws exactly 1311 big-endian 16-bit words,
/// rejects words at least `5q`, and stably selects the first 1024 residues.
pub fn hash_to_point_ct(nonce: &[u8; 40], message: &[u8]) -> Result<HashToPointTrace, FalconError> {
    let mut input = Vec::with_capacity(nonce.len() + message.len());
    input.extend_from_slice(nonce);
    input.extend_from_slice(message);
    let (bytes, shake) = shake256_with_trace(&input, 2 * HASH_TO_POINT_SAMPLES);
    from_shake_bytes(&bytes, shake)
}

/// Arithmetic mirror shared by the prime and binary SHAKE front ends.
pub(super) fn from_shake_bytes(
    bytes: &[u8],
    shake: KeccakTrace,
) -> Result<HashToPointTrace, FalconError> {
    if bytes.len() != 2 * HASH_TO_POINT_SAMPLES {
        return Err(FalconError::Piop("SHAKE sample length".into()));
    }

    let mut words = Box::new([0u16; HASH_TO_POINT_SAMPLES]);
    let mut quotients = Box::new([0u8; HASH_TO_POINT_SAMPLES]);
    let mut remainders = Box::new([0u16; HASH_TO_POINT_SAMPLES]);
    let mut accepted = Box::new([false; HASH_TO_POINT_SAMPLES]);
    let mut prefix = Box::new([0u16; HASH_TO_POINT_SAMPLES + 1]);
    let mut point = Box::new([0u16; N]);
    let mut count = 0usize;

    for index in 0..HASH_TO_POINT_SAMPLES {
        let word = u16::from_be_bytes([bytes[2 * index], bytes[2 * index + 1]]);
        let quotient = u32::from(word) / Q as u32;
        let remainder = u32::from(word) - quotient * Q as u32;
        let keep = u32::from(word) < 5 * Q as u32;
        words[index] = word;
        quotients[index] = quotient as u8;
        remainders[index] = remainder as u16;
        accepted[index] = keep;
        prefix[index] = count as u16;
        if keep {
            if count < N {
                point[count] = remainder as u16;
            }
            count += 1;
        }
    }
    prefix[HASH_TO_POINT_SAMPLES] = count as u16;
    if count < N {
        return Err(FalconError::HashToPointUnderflow { accepted: count });
    }

    Ok(HashToPointTrace {
        shake,
        words,
        quotients,
        remainders,
        accepted,
        prefix,
        point,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falcon_hash_to_point_matches_rustcrypto_stream() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};
        use sha3::{
            Shake256,
            digest::{ExtendableOutput, Update, XofReader},
        };

        let mut rng = StdRng::seed_from_u64(0x4641_4c43_4f4e);
        for case in 0..10 {
            let (nonce, message) = match case {
                0 => ([0; 40], [0; 32]),
                1 => ([255; 40], [255; 32]),
                _ => (
                    std::array::from_fn(|_| rng.random::<u8>()),
                    std::array::from_fn(|_| rng.random::<u8>()),
                ),
            };
            let mut oracle = Shake256::default();
            oracle.update(&nonce);
            oracle.update(&message);
            let mut bytes = [0; 2622];
            oracle.finalize_xof().read(&mut bytes);
            let words: Vec<_> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            let point: Vec<_> = words
                .iter()
                .copied()
                .filter(|&word| word < 61_445)
                .map(|word| word % 12_289)
                .take(1024)
                .collect();

            let trace = hash_to_point_ct(&nonce, &message).unwrap();
            assert_eq!(trace.words.as_slice(), words, "sample stream, case={case}");
            assert_eq!(point.len(), 1024);
            assert_eq!(trace.point.as_slice(), point, "hash to point, case={case}");
        }
    }

    #[test]
    fn exact_division_and_prefix_constraints_hold() {
        let trace = hash_to_point_ct(&[7u8; 40], &[3u8; 32]).unwrap();
        for i in 0..HASH_TO_POINT_SAMPLES {
            assert_eq!(
                u32::from(trace.words[i]),
                Q as u32 * u32::from(trace.quotients[i]) + u32::from(trace.remainders[i])
            );
            assert!(trace.remainders[i] < Q as u16);
            assert_eq!(trace.accepted[i], trace.quotients[i] < 5);
            assert_eq!(
                trace.prefix[i + 1],
                trace.prefix[i] + u16::from(trace.accepted[i])
            );
        }
        assert!(trace.prefix[HASH_TO_POINT_SAMPLES] >= N as u16);
        let expected: Vec<_> = trace
            .remainders
            .iter()
            .zip(trace.accepted.iter())
            .filter_map(|(&r, &keep)| keep.then_some(r))
            .take(N)
            .collect();
        assert_eq!(trace.point.as_slice(), expected.as_slice());
    }
}
