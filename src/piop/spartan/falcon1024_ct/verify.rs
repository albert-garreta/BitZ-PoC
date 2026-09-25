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
    /// Exact coefficients of `H*S2` in `Z[X]/(X^1024+1)` before reduction.
    pub convolution: Box<[i64; N]>,
    /// Centered short lift `S1 = C - H*S2 (mod q)`.
    pub s1: Box<[i16; N]>,
    /// Exact quotient in `C - H*S2 - S1 = q*K`.
    pub quotient: Box<[i64; N]>,
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
    let mut convolution = Box::new([0i64; N]);
    if fast_convolution {
        let left: Vec<_> = public_key.h.iter().map(|&h| i64::from(h)).collect();
        let right: Vec<_> = signature.s2.iter().map(|&s| i64::from(s)).collect();
        let product = integer_polynomial_product(&left, &right);
        for i in 0..N {
            convolution[i] = product[i] - product[i + N];
        }
    } else {
        for (i, &h) in public_key.h.iter().enumerate() {
            for (j, &s) in signature.s2.iter().enumerate() {
                let product = i64::from(h) * i64::from(s);
                let index = i + j;
                if index < N {
                    convolution[index] += product;
                } else {
                    convolution[index - N] -= product;
                }
            }
        }
    }

    let mut s1 = Box::new([0i16; N]);
    let mut quotient = Box::new([0i64; N]);
    let mut norm = 0u64;
    for i in 0..N {
        let difference = i64::from(hash_to_point.point[i]) - convolution[i];
        let residue = difference.rem_euclid(Q);
        let centered = if residue > Q / 2 {
            residue - Q
        } else {
            residue
        };
        let k = (difference - centered) / Q;
        debug_assert_eq!(difference, centered + Q * k);
        s1[i] = centered as i16;
        quotient[i] = k;
        norm += centered.unsigned_abs().pow(2);
        norm += i64::from(signature.s2[i]).unsigned_abs().pow(2);
    }
    if norm > BETA_SQUARED {
        return Err(FalconError::NormTooLarge { actual: norm });
    }

    Ok(FalconVerificationTrace {
        public_key,
        signature,
        hash_to_point,
        convolution,
        s1,
        quotient,
        norm,
        norm_slack: BETA_SQUARED - norm,
    })
}

/// Exact Karatsuba product. Falcon's bounded 14- and 12-bit inputs at
/// degree 1024 leave ample headroom in i64, including recursive sums.
fn integer_polynomial_product(a: &[i64], b: &[i64]) -> Vec<i64> {
    let n = a.len();
    debug_assert_eq!(n, b.len());
    let mut out = vec![0; 2 * n];
    if n <= 32 {
        for (i, &x) in a.iter().enumerate() {
            for (j, &y) in b.iter().enumerate() {
                out[i + j] += x * y;
            }
        }
        return out;
    }
    let h = n / 2;
    let lo = integer_polynomial_product(&a[..h], &b[..h]);
    let hi = integer_polynomial_product(&a[h..], &b[h..]);
    let asum: Vec<_> = (0..h).map(|i| a[i] + a[h + i]).collect();
    let bsum: Vec<_> = (0..h).map(|i| b[i] + b[h + i]).collect();
    let mid = integer_polynomial_product(&asum, &bsum);
    for i in 0..n {
        out[i] += lo[i];
        out[i + n] += hi[i];
        out[i + h] += mid[i] - lo[i] - hi[i];
    }
    out
}

/// Verifies a Falcon-1024 CT signature.
pub fn verify_falcon1024_ct(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), FalconError> {
    verification_trace(public_key, message, signature).map(drop)
}

#[cfg(test)]
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
        for i in 0..N {
            assert_eq!(
                i64::from(trace.hash_to_point.point[i])
                    - trace.convolution[i]
                    - i64::from(trace.s1[i]),
                Q * trace.quotient[i]
            );
        }
    }

    #[test]
    fn fixture_mutations_are_rejected() {
        let mut message = *MESSAGE;
        message[0] ^= 1;
        assert!(verify_falcon1024_ct(PUBLIC_KEY, &message, SIGNATURE).is_err());

        let mut signature = *SIGNATURE;
        signature[1 + super::super::NONCE_BYTES + 17] ^= 1;
        assert!(verify_falcon1024_ct(PUBLIC_KEY, MESSAGE, &signature).is_err());
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
