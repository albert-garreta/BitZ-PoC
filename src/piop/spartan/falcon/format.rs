use super::{FalconError, NONCE_BYTES};
use crate::piop::spartan::falcon_parameters::{FalconParameters, Q};

/// Canonically decoded Falcon public key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconPublicKey<const N: usize> {
    pub h: Box<[u16; N]>,
}

/// Canonically decoded Falcon constant-time signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconSignatureCt<const N: usize> {
    pub nonce: [u8; NONCE_BYTES],
    pub s2: Box<[i16; N]>,
}

impl<const N: usize> FalconSignatureCt<N> {
    fn write_ct(&self, bytes: &mut [u8]) -> Result<(), FalconError> {
        let parameters = FalconParameters::for_degree(N);
        let bits = parameters.signature_bits;
        debug_assert_eq!(bytes.len(), parameters.signature_bytes());
        bytes[0] = 0x50 + N.ilog2() as u8;
        bytes[1..1 + NONCE_BYTES].copy_from_slice(&self.nonce);
        for (index, &coefficient) in self.s2.iter().enumerate() {
            if coefficient <= -(1 << (bits - 1)) || coefficient >= 1 << (bits - 1) {
                return Err(FalconError::SignatureCoefficientOutOfRange { index });
            }
        }
        for (i, &coefficient) in self.s2.iter().enumerate() {
            for bit in 0..bits {
                let offset = i * bits + bit;
                let value = ((coefficient as u16) >> (bits - 1 - bit)) & 1;
                bytes[1 + NONCE_BYTES + offset / 8] |= (value as u8) << (7 - offset % 8);
            }
        }
        Ok(())
    }
}

// Concrete degrees preserve fixed-size results without generic const expressions.
macro_rules! signature_encoders {
    ($($degree:literal),+ $(,)?) => {$ (
        impl FalconSignatureCt<$degree> {
            /// Encodes the nonce and coefficients in canonical Falcon CT form.
            pub fn encode_ct(
                &self,
            ) -> Result<[u8; FalconParameters::for_degree($degree).signature_bytes()], FalconError> {
                let mut bytes = [0u8; FalconParameters::for_degree($degree).signature_bytes()];
                self.write_ct(&mut bytes)?;
                Ok(bytes)
            }
        }
    )+};
}
signature_encoders!(512, 1024);

/// Decodes the `log2(N) || h[0..N]` public-key format.  Coefficients are
/// 14-bit, big-endian bit strings and must lie in `[0,q)`.
pub fn decode_public_key<const N: usize>(bytes: &[u8]) -> Result<FalconPublicKey<N>, FalconError> {
    let parameters = FalconParameters::for_degree(N);
    if bytes.len() != parameters.public_key_bytes() {
        return Err(FalconError::PublicKeyLength {
            expected: parameters.public_key_bytes(),
        });
    }
    if bytes[0] != N.ilog2() as u8 {
        return Err(FalconError::PublicKeyHeader);
    }
    let mut h = Box::new([0u16; N]);
    let mut acc = 0u32;
    let mut acc_len = 0usize;
    let mut input = bytes[1..].iter().copied();
    for (index, coefficient) in h.iter_mut().enumerate() {
        while acc_len < 14 {
            acc = (acc << 8) | u32::from(input.next().expect("length checked"));
            acc_len += 8;
        }
        acc_len -= 14;
        let value = ((acc >> acc_len) & 0x3fff) as u16;
        if u64::from(value) >= Q {
            return Err(FalconError::PublicKeyCoefficient { index });
        }
        *coefficient = value;
        acc &= (1u32 << acc_len).wrapping_sub(1);
    }
    debug_assert_eq!(acc_len, 0);
    debug_assert!(input.next().is_none());
    Ok(FalconPublicKey { h })
}

/// Decodes the Falcon CT format `(0x50 + log2(N)) || nonce || trim_i16(s2, bits)`.
/// The coefficient payload is a big-endian stream of signed two's-complement
/// values.  Falcon's canonical trim encoding excludes the minimum signed value.
pub fn decode_signature_ct<const N: usize>(
    bytes: &[u8],
) -> Result<FalconSignatureCt<N>, FalconError> {
    let parameters = FalconParameters::for_degree(N);
    let bits = parameters.signature_bits;
    if bytes.len() != parameters.signature_bytes() {
        return Err(FalconError::SignatureLength {
            expected: parameters.signature_bytes(),
        });
    }
    if bytes[0] != 0x50 + N.ilog2() as u8 {
        return Err(FalconError::SignatureHeader);
    }
    let mut nonce = [0u8; NONCE_BYTES];
    nonce.copy_from_slice(&bytes[1..1 + NONCE_BYTES]);

    let payload = &bytes[1 + NONCE_BYTES..];
    let mut s2 = Box::new([0i16; N]);
    let mut acc = 0u32;
    let mut acc_len = 0usize;
    let mut input = payload.iter().copied();
    for (index, coefficient) in s2.iter_mut().enumerate() {
        while acc_len < bits {
            acc = (acc << 8) | u32::from(input.next().expect("length checked"));
            acc_len += 8;
        }
        acc_len -= bits;
        let word = ((acc >> acc_len) & ((1 << bits) - 1)) as u16;
        if word == (1 << (bits - 1)) {
            return Err(FalconError::ForbiddenSignatureCoefficient { index });
        }
        *coefficient = if word & (1 << (bits - 1)) == 0 {
            word as i16
        } else {
            (i32::from(word) - (1 << bits)) as i16
        };
        acc &= (1u32 << acc_len).wrapping_sub(1);
    }
    debug_assert_eq!(acc_len, 0);
    debug_assert!(input.next().is_none());
    Ok(FalconSignatureCt { nonce, s2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_encoder_preserves_fixture() {
        let fixture = include_bytes!("fixtures/signature_ct.bin");
        let signature = decode_signature_ct::<1024>(fixture).unwrap();
        let encoded: [u8; 1577] = signature.encode_ct().unwrap();
        assert_eq!(&encoded, fixture);
    }

    macro_rules! codec_tests {
        ($module:ident, $degree:literal) => {
            mod $module {
                use super::*;
                const N: usize = $degree;
                const PARAMETERS: FalconParameters = FalconParameters::for_degree(N);

                #[test]
                fn ct_roundtrip_has_fixed_size_and_signed_coefficients() {
                    let signature = FalconSignatureCt::<N> {
                        nonce: std::array::from_fn(|i| i as u8),
                        s2: Box::new(std::array::from_fn(|i| [0, 1, -1, 2047, -2047][i % 5])),
                    };
                    let encoded: [u8; PARAMETERS.signature_bytes()] =
                        signature.encode_ct().unwrap();
                    assert_eq!(decode_signature_ct::<N>(&encoded).unwrap(), signature);
                    assert_eq!(encoded[0], 0x50 + N.ilog2() as u8);
                    assert_eq!(&encoded[1..1 + NONCE_BYTES], &signature.nonce);
                }

                #[test]
                fn ct_encoder_rejects_noncanonical_coefficients() {
                    let mut signature = FalconSignatureCt::<N> {
                        nonce: [0; NONCE_BYTES],
                        s2: Box::new([0; N]),
                    };
                    for invalid in [-2048, 2048, i16::MIN, i16::MAX] {
                        signature.s2[17] = invalid;
                        assert_eq!(
                            signature.encode_ct(),
                            Err(FalconError::SignatureCoefficientOutOfRange { index: 17 })
                        );
                    }
                }

                #[test]
                fn ct_decoder_is_msb_first_and_signed() {
                    let mut bytes = [0u8; PARAMETERS.signature_bytes()];
                    bytes[0] = 0x50 + N.ilog2() as u8;
                    // Independent packed encoding of 1, -1, 2047, then zeros.
                    bytes[1 + NONCE_BYTES..1 + NONCE_BYTES + 5]
                        .copy_from_slice(&[0x00, 0x1f, 0xff, 0x7f, 0xf0]);
                    let decoded = decode_signature_ct::<N>(&bytes).unwrap();
                    assert_eq!(&decoded.s2[..4], &[1, -1, 2047, 0]);
                }

                #[test]
                fn ct_decoder_rejects_malformed_encodings() {
                    let mut bytes = [0u8; PARAMETERS.signature_bytes()];
                    bytes[0] = 0x50 + N.ilog2() as u8;
                    assert_eq!(
                        decode_signature_ct::<N>(&bytes[..bytes.len() - 1]),
                        Err(FalconError::SignatureLength {
                            expected: bytes.len()
                        })
                    );
                    bytes[0] ^= 1;
                    assert_eq!(
                        decode_signature_ct::<N>(&bytes),
                        Err(FalconError::SignatureHeader)
                    );
                    bytes[0] ^= 1;
                    bytes[1 + NONCE_BYTES] = 0x80;
                    assert_eq!(
                        decode_signature_ct::<N>(&bytes),
                        Err(FalconError::ForbiddenSignatureCoefficient { index: 0 })
                    );
                }

                #[test]
                fn public_key_decoder_checks_length_header_and_coefficients() {
                    let mut bytes = [0u8; PARAMETERS.public_key_bytes()];
                    bytes[0] = N.ilog2() as u8;
                    assert_eq!(decode_public_key::<N>(&bytes).unwrap().h.as_ref(), &[0; N]);
                    assert_eq!(
                        decode_public_key::<N>(&bytes[..bytes.len() - 1]),
                        Err(FalconError::PublicKeyLength {
                            expected: bytes.len()
                        })
                    );
                    bytes[0] ^= 1;
                    assert_eq!(
                        decode_public_key::<N>(&bytes),
                        Err(FalconError::PublicKeyHeader)
                    );
                    bytes[0] ^= 1;
                    let q = Q as u16;
                    bytes[1] = (q >> 6) as u8;
                    bytes[2] = (q << 2) as u8;
                    assert_eq!(
                        decode_public_key::<N>(&bytes),
                        Err(FalconError::PublicKeyCoefficient { index: 0 })
                    );
                }
            }
        };
    }
    codec_tests!(degree512, 512);
    codec_tests!(degree1024, 1024);
}
