//! Spongefish codecs for this crate's types, matching their
//! `field::codec` byte for byte.
//!
//! - `Gf` (their `F128`): 16 bytes, `lo` then `hi`, each little-endian.
//!   Total in both directions, so a challenge is 16 squeezed bytes read
//!   through the same bijection.
//! - A prime-field residue: the canonical representative as 16
//!   little-endian bytes ([`FqWire`]).

use spongefish::Encoding;

use field::Gf128 as Gf;

/// Their `F128::to_bytes`.
pub fn gf_to_bytes(value: Gf) -> [u8; 16] {
    value.to_bytes()
}

/// Their `F128::from_bytes`.
pub fn gf_from_bytes(bytes: [u8; 16]) -> Gf {
    Gf::from_bytes(bytes)
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
