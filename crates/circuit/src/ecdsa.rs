//! The ECDSA verifier circuits selectable by the SHA-256 + ECDSA relation.
//!
//! Every circuit takes the digest first, then the four public words `Q.x`,
//! `Q.y`, `r`, `s`, each as a little-endian 256-bit word, so the composed
//! relation aliases the digest and binds the public words the same way for
//! all of them. Circuits may append private hint words after the public ones.


use field::{CanonicalCodec, IntegerOps, Uint};

use crate::Circuit;
use crate::p256::{self, Curve, P256, SECP256K1, secp256k1_matched};

/// Which ECDSA verifier the relation runs after the SHA-256 chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EcdsaCircuit {
    /// The paper's P-256 verifier: the Lean port with complete affine
    /// formulas and joint fixed/variable-base scalar multiplication.
    P256Paper,
    /// secp256k1 on Binius64's stock verifier schedule; see
    /// [`secp256k1_matched`]. For the head-to-head measurement only.
    Secp256k1BiniusMatched,
}

/// A public scalar outside `1..n`.
#[derive(Debug, thiserror::Error)]
#[error("{0} is not a canonical nonzero {1} scalar")]
pub struct InvalidScalar(pub &'static str, pub &'static str);

impl EcdsaCircuit {
    pub const ALL: [Self; 2] = [Self::P256Paper, Self::Secp256k1BiniusMatched];

    /// The curve the circuit verifies over.
    pub const fn curve(self) -> &'static Curve {
        match self {
            Self::P256Paper => &P256,
            Self::Secp256k1BiniusMatched => &SECP256K1,
        }
    }

    /// The curve's benchmark token: `p256` or `secp256k1`.
    pub const fn curve_token(self) -> &'static str {
        match self {
            Self::P256Paper => "p256",
            Self::Secp256k1BiniusMatched => "secp256k1",
        }
    }

    /// The circuit selected for a curve token; each curve has exactly one.
    pub fn for_curve_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.curve_token() == token)
    }

    /// Stable identifier recorded with every benchmark row and checked by
    /// the campaign runner before two rows are compared.
    pub const fn profile(self) -> &'static str {
        match self {
            Self::P256Paper => "sha256-chain-p256/bitz-lean-port/v1",
            Self::Secp256k1BiniusMatched => "sha256-chain-secp256k1/bitz-binius64-matched/v1",
        }
    }

    /// Number of Boolean inputs, public words and hint words included.
    pub const fn input_bits(self) -> usize {
        match self {
            Self::P256Paper => p256::VERIFY_DIGEST_INPUT_BITS,
            Self::Secp256k1BiniusMatched => secp256k1_matched::INPUT_BITS,
        }
    }

    /// Total Boolean witness size of the standalone verifier, inputs included.
    pub const fn witness_bits(self) -> usize {
        match self {
            Self::P256Paper => p256::VERIFY_DIGEST_WITNESS_BITS,
            Self::Secp256k1BiniusMatched => secp256k1_matched::WITNESS_BITS,
        }
    }

    /// Number of packed `M * w` bits, including the implicit constant one.
    pub const fn integer_witness_bits(self) -> usize {
        match self {
            Self::P256Paper => p256::VERIFY_DIGEST_INTEGER_WITNESS_BITS,
            Self::Secp256k1BiniusMatched => secp256k1_matched::INTEGER_WITNESS_BITS,
        }
    }

    /// Number of rank-1 constraints.
    pub const fn r1cs_rows(self) -> usize {
        match self {
            Self::P256Paper => p256::VERIFY_DIGEST_R1CS_ROWS,
            Self::Secp256k1BiniusMatched => secp256k1_matched::R1CS_ROWS,
        }
    }

    /// Builds the verifier over `inputs`, which must hold exactly
    /// [`Self::input_bits`] Booleans.
    pub fn build<CS: Circuit>(self, circuit: &mut CS, inputs: &[CS::Bool]) {
        assert_eq!(inputs.len(), self.input_bits(), "{self:?} input width");
        match self {
            Self::P256Paper => {
                let inputs: &[CS::Bool; p256::VERIFY_DIGEST_INPUT_BITS] =
                    inputs.try_into().expect("checked width");
                p256::verify_digest_circuit(circuit, inputs);
            }
            Self::Secp256k1BiniusMatched => {
                let inputs: &[CS::Bool; secp256k1_matched::INPUT_BITS] =
                    inputs.try_into().expect("checked width");
                secp256k1_matched::verify_digest_circuit(circuit, inputs);
            }
        }
    }

    /// The circuit's input bits for a digest and the four public words, all
    /// as canonical big-endian bytes. The P-256 circuit appends the inverses
    /// of `r` and `s` as hint words; both circuits reject scalars outside
    /// `1..n`, which is what the composed relation's verifier checks natively.
    pub fn input_bits_from_words(
        self,
        digest: &[u8; 32],
        qx: &[u8; 32],
        qy: &[u8; 32],
        r: &[u8; 32],
        s: &[u8; 32],
    ) -> Result<Vec<bool>, InvalidScalar> {
        let curve = self.curve();
        let r_inverse = scalar_inverse_word(curve, r).ok_or(InvalidScalar("r", curve.name))?;
        let s_inverse = scalar_inverse_word(curve, s).ok_or(InvalidScalar("s", curve.name))?;
        let words: Vec<&[u8; 32]> = match self {
            Self::P256Paper => vec![digest, qx, qy, r, s, &r_inverse, &s_inverse],
            Self::Secp256k1BiniusMatched => vec![digest, qx, qy, r, s],
        };
        debug_assert_eq!(words.len() * 256, self.input_bits());
        Ok((0..self.input_bits())
            .map(|bit| words[bit / 256][31 - (bit % 256) / 8] >> (bit % 8) & 1 != 0)
            .collect())
    }
}

/// The big-endian inverse of a canonical nonzero scalar, or `None`.
fn scalar_inverse_word(curve: &'static Curve, value: &[u8; 32]) -> Option<[u8; 32]> {
    let mut bytes = *value;
    bytes.reverse();
    let scalar: Uint<4> = IntegerOps
        .decode_public(&bytes)
        .expect("fixed-width scalar encoding");
    let inverse = curve.scalar_inverse_ct(&scalar);
    if !inverse.validity().declassify() {
        return None;
    }
    IntegerOps.encode_into(inverse.value(), &mut bytes);
    bytes.reverse();
    Some(bytes)
}

/// The word order of [`EcdsaCircuit::input_bits_from_words`], for callers that
/// index the public words: digest, then `Q.x`, `Q.y`, `r`, `s`.
pub const PUBLIC_WORDS: [&str; 5] = ["digest", "qx", "qy", "r", "s"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{Dummy, LeanStats, Stats};

    #[test]
    fn descriptors_agree_with_their_circuits() {
        for circuit in EcdsaCircuit::ALL {
            let mut stats = Stats::new(circuit.input_bits());
            let inputs: Vec<Dummy> = vec![Dummy; circuit.input_bits()];
            circuit.build(&mut stats, &inputs);
            assert_eq!(
                stats.lean_stats(),
                LeanStats {
                    m_rows: circuit.integer_witness_bits(),
                    m_cols: circuit.witness_bits() + 1,
                    r1cs_rows: circuit.r1cs_rows(),
                },
                "{circuit:?}"
            );
            assert_eq!(EcdsaCircuit::for_curve_token(circuit.curve_token()), Some(circuit));
        }
        assert_eq!(EcdsaCircuit::for_curve_token("p-256"), None);
        assert_ne!(
            EcdsaCircuit::P256Paper.profile(),
            EcdsaCircuit::Secp256k1BiniusMatched.profile()
        );
    }

    #[test]
    fn input_words_are_little_endian_and_scalars_are_validated() {
        let n = *SECP256K1.scalar_modulus();
        let mut n_be = [0u8; 32];
        for (i, word) in n.as_words().iter().rev().enumerate() {
            n_be[8 * i..8 * i + 8].copy_from_slice(&word.to_be_bytes());
        }
        let one = {
            let mut w = [0u8; 32];
            w[31] = 1;
            w
        };
        let bits = EcdsaCircuit::Secp256k1BiniusMatched
            .input_bits_from_words(&[0; 32], &[0; 32], &[0; 32], &one, &one)
            .unwrap();
        assert_eq!(bits.len(), 5 * 256);
        assert!(bits[3 * 256] && !bits[3 * 256 + 1] && bits[4 * 256]);
        for circuit in EcdsaCircuit::ALL {
            assert!(circuit.input_bits_from_words(&[0; 32], &[0; 32], &[0; 32], &[0; 32], &one).is_err());
            assert!(circuit.input_bits_from_words(&[0; 32], &[0; 32], &[0; 32], &one, &[0; 32]).is_err());
        }
        assert!(EcdsaCircuit::Secp256k1BiniusMatched
            .input_bits_from_words(&[0; 32], &[0; 32], &[0; 32], &n_be, &one)
            .is_err());
        let p256 = EcdsaCircuit::P256Paper
            .input_bits_from_words(&[0; 32], &[0; 32], &[0; 32], &one, &one)
            .unwrap();
        assert_eq!(p256.len(), 7 * 256);
        // The inverse of one is one, in both appended hint words.
        assert!(p256[5 * 256] && p256[6 * 256] && !p256[5 * 256 + 1]);
    }
}
