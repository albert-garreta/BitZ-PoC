use crate::piop::spartan::falcon_polynomial::PolynomialWorkspace;
use super::{
    BETA_SQUARED, FalconError, FalconPublicKey, FalconSignatureCt, N, Q, decode_public_key,
    decode_signature_ct,
    hash_to_point::{HashToPointTrace, hash_to_point_ct},
};

/// Native/circuit-mirror witness for one Falcon verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconVerificationTrace {
    pub public_key: FalconPublicKey,
    pub signature: FalconSignatureCt,
    pub hash_to_point: HashToPointTrace,
    /// Centered short lift `S1 = C - H*S2 (mod q)`.
    pub s1: Box<[i16; N]>,
    /// Quotient of the native residual by `X^N + 1` over F_12289.
    /// These are the reduced negatives of the high coefficients of `H*S2`.
    pub ring_quotient: Box<[u16; N - 1]>,
    pub norm: u64,
    pub norm_slack: u64,
}

/// Builds and checks the exact witness used by the PIOP.
pub fn verification_trace(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<FalconVerificationTrace, FalconError> {
    let public_key = decode_public_key(public_key)?;
    let signature = decode_signature_ct(signature)?;
    let hash_to_point = hash_to_point_ct(&signature.nonce, message)?;
    trace_from_parts(public_key, signature, hash_to_point, false)
}

pub(super) fn trace_from_parts(
    public_key: FalconPublicKey,
    signature: FalconSignatureCt,
    hash_to_point: HashToPointTrace,
    fast_convolution: bool,
) -> Result<FalconVerificationTrace, FalconError> {
    if fast_convolution {
        return trace_from_parts_with_workspace(
            public_key,
            signature,
            hash_to_point,
            &mut PolynomialWorkspace::new(N),
        );
    }
    let mut convolution = Box::new([0i64; N]);
    let mut high_product = Box::new([0i64; N - 1]);
    for (i, &h) in public_key.h.iter().enumerate() {
        for (j, &s) in signature.s2.iter().enumerate() {
            let product = i64::from(h) * i64::from(s);
            let index = i + j;
            if index < N {
                convolution[index] += product;
            } else {
                convolution[index - N] -= product;
                high_product[index - N] += product;
            }
        }
    }

    let mut s1 = Box::new([0i16; N]);
    let mut norm = 0u64;
    for i in 0..N {
        let difference = i64::from(hash_to_point.point[i]) - convolution[i];
        let residue = difference.rem_euclid(Q);
        let centered = if residue > Q / 2 {
            residue - Q
        } else {
            residue
        };
        s1[i] = centered as i16;
        norm += centered.unsigned_abs().pow(2);
        norm += i64::from(signature.s2[i]).unsigned_abs().pow(2);
    }
    if norm > BETA_SQUARED {
        return Err(FalconError::NormTooLarge {
            actual: norm,
            bound: BETA_SQUARED,
        });
    }

    Ok(FalconVerificationTrace {
        public_key,
        signature,
        hash_to_point,
        s1,
        ring_quotient: Box::new(std::array::from_fn(|j| {
            (-high_product[j]).rem_euclid(Q) as u16
        })),
        norm,
        norm_slack: BETA_SQUARED - norm,
    })
}

pub(super) fn trace_from_parts_with_workspace(
    public_key: FalconPublicKey,
    signature: FalconSignatureCt,
    hash_to_point: HashToPointTrace,
    workspace: &mut PolynomialWorkspace,
) -> Result<FalconVerificationTrace, FalconError> {
    let product = workspace.product(public_key.h.as_slice(), signature.s2.as_slice());
    let mut s1 = Box::new([0i16; N]);
    let mut ring_quotient = Box::new([0u16; N - 1]);
    let mut norm = 0u64;
    for i in 0..N {
        let difference = i64::from(hash_to_point.point[i]) - product[i] + product[i + N];
        let residue = difference.rem_euclid(Q);
        let centered = if residue > Q / 2 {
            residue - Q
        } else {
            residue
        };
        s1[i] = centered as i16;
        norm += centered.unsigned_abs().pow(2);
        norm += i64::from(signature.s2[i]).unsigned_abs().pow(2);
        if i < N - 1 {
            ring_quotient[i] = (-product[i + N]).rem_euclid(Q) as u16;
        }
    }
    if norm > BETA_SQUARED {
        return Err(FalconError::NormTooLarge {
            actual: norm,
            bound: BETA_SQUARED,
        });
    }
    Ok(FalconVerificationTrace {
        public_key,
        signature,
        hash_to_point,
        s1,
        ring_quotient,
        norm,
        norm_slack: BETA_SQUARED - norm,
    })
}

/// Verifies a Falcon-1024 CT signature.
pub fn verify_falcon_ct(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), FalconError> {
    verification_trace(public_key, message, signature).map(drop)
}

#[cfg(test)]
mod workspace_tests {
    use super::super::{KeccakTrace, hash_to_point::from_shake_words};
    use super::*;

    #[test]
    fn reused_workspace_matches_schoolbook_trace() {
        let mut workspace = PolynomialWorkspace::new(N);
        for seed in 0..4 {
            let public_key = FalconPublicKey {
                h: Box::new(std::array::from_fn(|i| {
                    ((i * 7_919 + seed * 97) % Q as usize) as u16
                })),
            };
            let mut signature = FalconSignatureCt {
                nonce: [seed as u8; 40],
                s2: Box::new([0; N]),
            };
            signature.s2[0] = 2_047;
            signature.s2[N / 2] = seed as i16 - 2;
            signature.s2[N - 1] = -2_047;
            let mut hash = from_shake_words(
                Box::new([0; super::super::HASH_TO_POINT_SAMPLES]),
                KeccakTrace::default(),
            )
            .unwrap();
            // Construct small centered residuals independently from the three
            // nonzero signature coefficients, including the negacyclic wrap.
            let mut convolution = [0i64; N];
            for (i, &h) in public_key.h.iter().enumerate() {
                for j in [0, N / 2, N - 1] {
                    let term = i64::from(h) * i64::from(signature.s2[j]);
                    if i + j < N {
                        convolution[i + j] += term;
                    } else {
                        convolution[i + j - N] -= term;
                    }
                }
            }
            for i in 0..N {
                hash.point[i] = (convolution[i] + (i % 7) as i64 - 3).rem_euclid(Q) as u16;
            }
            let slow = trace_from_parts(public_key.clone(), signature.clone(), hash.clone(), false)
                .unwrap();
            let fast = trace_from_parts_with_workspace(public_key, signature, hash, &mut workspace)
                .unwrap();
            assert_eq!(fast, slow);
        }
    }

    #[test]
    fn fused_trace_preserves_excessive_norm_error() {
        let public_key = FalconPublicKey {
            h: Box::new([0; N]),
        };
        let signature = FalconSignatureCt {
            nonce: [0; 40],
            s2: Box::new([0; N]),
        };
        let mut hash = from_shake_words(
            Box::new([0; super::super::HASH_TO_POINT_SAMPLES]),
            KeccakTrace::default(),
        )
        .unwrap();
        hash.point.fill((Q / 2) as u16);
        let expected = Err(FalconError::NormTooLarge {
            actual: N as u64 * (Q as u64 / 2).pow(2),
            bound: BETA_SQUARED,
        });
        assert_eq!(
            trace_from_parts(public_key.clone(), signature.clone(), hash.clone(), false),
            expected
        );
        assert_eq!(
            trace_from_parts_with_workspace(
                public_key,
                signature,
                hash,
                &mut PolynomialWorkspace::new(N)
            ),
            expected
        );
    }
}

falcon_tests! {
mod tests {
    use super::*;

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    #[test]
    fn pqclean_constant_time_fixture_verifies() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        assert!(trace.norm <= BETA_SQUARED);
        assert_eq!(trace.norm + trace.norm_slack, BETA_SQUARED);
        super::super::check_exact_constraints(&trace).unwrap();
    }

    #[test]
    fn cached_ring_quotient_matches_full_product_and_fast_trace() {
        let slow = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let fast = trace_from_parts(
            slow.public_key.clone(),
            slow.signature.clone(),
            slow.hash_to_point.clone(),
            true,
        )
        .unwrap();
        assert_eq!(slow, fast);
        let mut product = [0i64; 2 * N - 1];
        for (i, &h) in slow.public_key.h.iter().enumerate() {
            for (j, &s) in slow.signature.s2.iter().enumerate() {
                product[i + j] += i64::from(h) * i64::from(s);
            }
        }
        for j in 0..2 * N - 1 {
            let mut residual = -product[j];
            if j < N {
                residual += i64::from(slow.hash_to_point.point[j]) - i64::from(slow.s1[j]);
            }
            let quotient = if j == N - 1 {
                0
            } else {
                slow.ring_quotient[j % N]
            };
            assert_eq!(residual.rem_euclid(Q), i64::from(quotient));
        }
    }

    #[test]
    fn fixture_mutations_are_rejected() {
        let mut message = *MESSAGE;
        message[0] ^= 1;
        assert!(verify_falcon_ct(PUBLIC_KEY, &message, SIGNATURE).is_err());

        let mut signature = *SIGNATURE;
        signature[1 + super::super::NONCE_BYTES + 17] ^= 1;
        assert!(verify_falcon_ct(PUBLIC_KEY, MESSAGE, &signature).is_err());
    }

    #[test]
    fn zero_signature_fails_the_norm_check_for_zero_key() {
        let mut pk = vec![0u8; super::super::PUBLIC_KEY_BYTES];
        pk[0] = 0x0a;
        let mut signature = vec![0u8; super::super::CT_SIGNATURE_BYTES];
        signature[0] = 0x5a;
        assert!(matches!(
            verification_trace(&pk, &[0u8; 32], &signature),
            Err(FalconError::NormTooLarge { .. })
        ));
    }
}

}
