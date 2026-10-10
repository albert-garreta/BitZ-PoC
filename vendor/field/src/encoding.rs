//! Canonical numeric encoding. Decoding rejects noncanonical values instead of reducing.

use crate::integer::LIMB_BYTES;
use crate::{CtValue, IntegerOps, Uint, Z};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Length { expected: usize, actual: usize },
    NonCanonical,
}
impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "canonical decoding: {self:?}")
    }
}
impl std::error::Error for DecodeError {}

pub trait CanonicalCodec<T> {
    fn encoded_len(&self) -> usize;
    fn encode_into(&self, value: &T, out: &mut [u8]);
    fn decode_ct(&self, input: &[u8]) -> Result<CtValue<T>, DecodeError>;
    fn decode_public(&self, input: &[u8]) -> Result<T, DecodeError> {
        let (value, valid) = self.decode_ct(input)?.into_parts();
        if valid.declassify() {
            Ok(value)
        } else {
            Err(DecodeError::NonCanonical)
        }
    }
}

pub(crate) fn encode_words<const L: usize>(value: &Uint<L>, out: &mut [u8]) {
    assert_eq!(out.len(), L * LIMB_BYTES, "encoding output length differs");
    for (word, bytes) in value
        .as_words()
        .iter()
        .zip(out.chunks_exact_mut(LIMB_BYTES))
    {
        bytes.copy_from_slice(&word.to_le_bytes());
    }
}
pub(crate) fn decode_words<const L: usize>(input: &[u8]) -> Result<Uint<L>, DecodeError> {
    if input.len() != L * LIMB_BYTES {
        return Err(DecodeError::Length {
            expected: L * LIMB_BYTES,
            actual: input.len(),
        });
    }
    Ok(Uint::from_words(core::array::from_fn(|i| {
        u64::from_le_bytes(
            input[i * LIMB_BYTES..(i + 1) * LIMB_BYTES]
                .try_into()
                .unwrap(),
        )
    })))
}
impl<const L: usize> CanonicalCodec<Uint<L>> for IntegerOps {
    fn encoded_len(&self) -> usize {
        L * LIMB_BYTES
    }
    fn encode_into(&self, value: &Uint<L>, out: &mut [u8]) {
        encode_words(value, out);
    }
    fn decode_ct(&self, input: &[u8]) -> Result<CtValue<Uint<L>>, DecodeError> {
        Ok(CtValue::new(decode_words(input)?, crate::CtMask::TRUE))
    }
}
impl<const L: usize> CanonicalCodec<Z<L>> for IntegerOps {
    fn encoded_len(&self) -> usize {
        L * LIMB_BYTES
    }
    fn encode_into(&self, value: &Z<L>, out: &mut [u8]) {
        encode_words(&value.0, out);
    }
    fn decode_ct(&self, input: &[u8]) -> Result<CtValue<Z<L>>, DecodeError> {
        Ok(CtValue::new(Z(decode_words(input)?), crate::CtMask::TRUE))
    }
}
