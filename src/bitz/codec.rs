//! Spongefish codecs for this crate's types, matching their
//! `field::codec` byte for byte.
//!
//! - `Gf` (their `F128`): 16 bytes, `lo` then `hi`, each little-endian.
//!   Total in both directions, so a challenge is 16 squeezed bytes read
//!   through the same bijection.
//! - A prime-field residue: the canonical representative as 16
//!   little-endian bytes ([`FqWire`]).

use spongefish::{ByteArray, Decoding, Encoding, NargDeserialize, VerificationResult};

use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// Their `F128::to_bytes`.
pub fn gf_to_bytes(value: Gf) -> [u8; 16] {
    let words = value.words();
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&words[0].to_le_bytes());
    out[8..].copy_from_slice(&words[1].to_le_bytes());
    out
}

/// Their `F128::from_bytes`.
pub fn gf_from_bytes(bytes: [u8; 16]) -> Gf {
    let lo = u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"));
    let hi = u64::from_le_bytes(bytes[8..].try_into().expect("8 bytes"));
    Gf::from_words([lo, hi])
}

impl Encoding<[u8]> for Gf {
    fn encode(&self) -> impl AsRef<[u8]> {
        gf_to_bytes(*self)
    }
}

impl Decoding<[u8]> for Gf {
    type Repr = ByteArray<16>;

    fn decode(buf: Self::Repr) -> Self {
        gf_from_bytes(*buf.as_ref())
    }
}

impl NargDeserialize for Gf {
    fn deserialize_from_narg(buf: &mut &[u8]) -> VerificationResult<Self> {
        <[u8; 16]>::deserialize_from_narg(buf).map(gf_from_bytes)
    }
}

/// A residue of their `Fq<Q>` on the wire: the lifted value, 16
/// little-endian bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FqWire(pub u128);

impl Encoding<[u8]> for FqWire {
    fn encode(&self) -> impl AsRef<[u8]> {
        self.0.to_le_bytes()
    }
}
