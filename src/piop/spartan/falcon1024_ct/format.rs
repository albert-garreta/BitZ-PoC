use super::{CT_SIGNATURE_BYTES, FalconError, N, NONCE_BYTES, PUBLIC_KEY_BYTES, Q};

/// Canonically decoded Falcon-1024 public key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconPublicKey {
    pub h: Box<[u16; N]>,
}

/// Canonically decoded Falcon-1024 constant-time signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconSignatureCt {
    pub nonce: [u8; NONCE_BYTES],
    pub s2: Box<[i16; N]>,
}

/// Decodes the `0x0a || h[0..1024]` public-key format.  Coefficients are
/// 14-bit, big-endian bit strings and must lie in `[0,q)`.
pub fn decode_public_key(bytes: &[u8]) -> Result<FalconPublicKey, FalconError> {
    if bytes.len() != PUBLIC_KEY_BYTES {
        return Err(FalconError::PublicKeyLength);
    }
    if bytes[0] != 0x0a {
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
        if i64::from(value) >= Q {
            return Err(FalconError::PublicKeyCoefficient { index });
        }
        *coefficient = value;
        acc &= (1u32 << acc_len).wrapping_sub(1);
    }
    debug_assert_eq!(acc_len, 0);
    Ok(FalconPublicKey { h })
}

/// Decodes the Falcon CT format `0x5a || nonce || trim_i16(s2,12)`.
/// The coefficient payload is a big-endian stream of 12-bit two's-complement
/// values.  Falcon's canonical trim encoding excludes `-2^11`.
pub fn decode_signature_ct(bytes: &[u8]) -> Result<FalconSignatureCt, FalconError> {
    if bytes.len() != CT_SIGNATURE_BYTES {
        return Err(FalconError::SignatureLength);
    }
    if bytes[0] != 0x5a {
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
        while acc_len < 12 {
            acc = (acc << 8) | u32::from(input.next().expect("length checked"));
            acc_len += 8;
        }
        acc_len -= 12;
        let word = ((acc >> acc_len) & 0x0fff) as u16;
        if word == 0x0800 {
            return Err(FalconError::ForbiddenSignatureCoefficient { index });
        }
        *coefficient = if word & 0x0800 == 0 {
            word as i16
        } else {
            (i32::from(word) - 4096) as i16
        };
        acc &= (1u32 << acc_len).wrapping_sub(1);
    }
    debug_assert_eq!(acc_len, 0);
    Ok(FalconSignatureCt { nonce, s2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_decoder_is_msb_first_and_signed() {
        let mut bytes = vec![0u8; CT_SIGNATURE_BYTES];
        bytes[0] = 0x5a;
        let payload = &mut bytes[1 + NONCE_BYTES..];
        // 1, -1, 2047, then zeros: 001 fff 7ff.
        payload[..5].copy_from_slice(&[0x00, 0x1f, 0xff, 0x7f, 0xf0]);
        let decoded = decode_signature_ct(&bytes).unwrap();
        assert_eq!(&decoded.s2[..4], &[1, -1, 2047, 0]);
    }

    #[test]
    fn ct_decoder_rejects_signed_minimum() {
        let mut bytes = vec![0u8; CT_SIGNATURE_BYTES];
        bytes[0] = 0x5a;
        bytes[1 + NONCE_BYTES] = 0x80;
        assert_eq!(
            decode_signature_ct(&bytes),
            Err(FalconError::ForbiddenSignatureCoefficient { index: 0 })
        );
    }

    #[test]
    fn public_key_decoder_rejects_q() {
        let mut bytes = vec![0u8; PUBLIC_KEY_BYTES];
        bytes[0] = 0x0a;
        let q = Q as u16;
        bytes[1] = (q >> 6) as u8;
        bytes[2] = (q << 2) as u8;
        assert_eq!(
            decode_public_key(&bytes),
            Err(FalconError::PublicKeyCoefficient { index: 0 })
        );
    }
}
