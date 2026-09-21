//! Fixed-length SHA-256 chain with Binius64's stock secp256k1 ECDSA verifier.
//!
//! The ECDSA core is the pinned fork's upstream `ecdsa::bitcoin_verify`
//! (GLV-Straus MSM with four-bit windows, incomplete additions, in-circuit
//! tables), unchanged. This module only composes it with the fork's fixed
//! SHA-256 chain and the same public statement as the P-256 relation:
//! `(i, Qx, Qy, r, s)` as one u64 exponent and four integers of four
//! little-endian u64 limbs. Two statement-level checks are added around the
//! stock core: the digest is reduced modulo `n` (the core's division needs a
//! reduced dividend and a standard verifier reduces the hash), and the public
//! key coordinates are asserted canonical.

use binius_circuits::{
    bignum::{BigUint, biguint_lt, sub},
    ecdsa::bitcoin_verify,
    secp256k1::{Secp256k1, Secp256k1Affine},
    sha256::sha256_fixed,
    sha256_ecdsa::{limbs, message_len},
};
use binius_core::word::Word;
use binius_frontend::{CircuitBuilder, Wire, WitnessFiller};

pub const PROFILE: &str = "sha256-chain-secp256k1/binius64-bitcoin-verify/v1";

/// The fixed relation and its input wires; construction never takes an instance.
pub struct Sha256EcdsaSecp256k1 {
    log_compressions: u8,
    exponent: Wire,
    message: Vec<Wire>,
    public: [BigUint; 4],
}

/// Reduces a 256-bit integer modulo n. Since 2^256 < 2n, one subtraction suffices.
fn reduce_scalar(b: &CircuitBuilder, value: &BigUint, modulus: &BigUint) -> BigUint {
    assert_eq!(value.limbs.len(), 4);
    let subtract = b.bnot(biguint_lt(b, value, modulus));
    let result = sub(b, value, &modulus.zero_unless(b, subtract));
    b.assert_true("reduced scalar < n", biguint_lt(b, &result, modulus));
    result
}

impl Sha256EcdsaSecp256k1 {
    pub fn new(b: &CircuitBuilder, log_compressions: u8) -> Result<Self, String> {
        let len = message_len(log_compressions)?;
        let exponent = b.add_inout();
        b.assert_eq(
            "public exponent",
            exponent,
            b.add_constant_64(u64::from(log_compressions)),
        );
        let public = std::array::from_fn(|_| BigUint::new_inout(b, 4));
        let message: Vec<_> = (0..len / 4).map(|_| b.add_witness()).collect();
        let zero = b.add_constant(Word::ZERO);
        for (i, &word) in message.iter().enumerate() {
            b.assert_eq(format!("message word {i} is u32"), b.shr(word, 32), zero);
        }
        let digest = sha256_fixed(&b.subcircuit("SHA-256 chain"), &message, len);
        let digest = BigUint {
            limbs: (0..4)
                .map(|i| b.bxor(digest[7 - 2 * i], b.shl(digest[6 - 2 * i], 32)))
                .collect(),
        };
        let v = b.subcircuit("secp256k1 verification");
        let curve = Secp256k1::new(&v);
        for (name, value) in [("Q.x", &public[0]), ("Q.y", &public[1])] {
            v.assert_true(format!("{name} < p"), biguint_lt(&v, value, curve.f_p().modulus()));
        }
        let z = reduce_scalar(&v, &digest, curve.f_scalar().modulus());
        let key = Secp256k1Affine {
            x: public[0].clone(),
            y: public[1].clone(),
            is_point_at_infinity: v.add_constant(Word::ZERO),
        };
        let valid = bitcoin_verify(&v, key, &z, &public[2], &public[3]);
        v.assert_true("secp256k1 ECDSA", valid);
        Ok(Self {
            log_compressions,
            exponent,
            message,
            public,
        })
    }

    /// Assigns only instance inputs. Circuit execution computes the digest and arithmetic hints.
    pub fn populate(
        &self,
        w: &mut WitnessFiller<'_>,
        message: &[u8],
        qx: &[u8; 32],
        qy: &[u8; 32],
        r: &[u8; 32],
        s: &[u8; 32],
    ) -> Result<(), String> {
        if message.len() != message_len(self.log_compressions)? {
            return Err("incorrect SHA-chain message length".into());
        }
        w[self.exponent] = Word::from_u64(u64::from(self.log_compressions));
        for (input, bytes) in self.public.iter().zip([qx, qy, r, s]) {
            input.populate_limbs(w, &limbs(bytes));
        }
        for (&wire, bytes) in self.message.iter().zip(message.chunks_exact(4)) {
            w[wire] = Word::from_u64(u64::from(u32::from_be_bytes(
                bytes.try_into().expect("four-byte chunk"),
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::{Signature, SigningKey, signature::Signer};

    fn instance(key: [u8; 32], message: &[u8]) -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        let key = SigningKey::from_bytes((&key).into()).unwrap();
        let q = key.verifying_key().to_encoded_point(false);
        let sig: Signature = key.sign(message);
        let (r, s) = sig.split_bytes();
        (
            (*q.x().unwrap()).into(),
            (*q.y().unwrap()).into(),
            r.into(),
            s.into(),
        )
    }

    #[test]
    fn both_s_forms_verify_and_changed_inputs_fail() {
        let builder = CircuitBuilder::new();
        let relation = Sha256EcdsaSecp256k1::new(&builder, 3).unwrap();
        let circuit = builder.build();
        let message = vec![0x5au8; message_len(3).unwrap()];
        let (qx, qy, r, s) = instance([17u8; 32], &message);
        let n = num_bigint::BigUint::parse_bytes(
            b"fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
            16,
        )
        .unwrap();
        let alternate = {
            let v = &n - num_bigint::BigUint::from_bytes_be(&s);
            let raw = v.to_bytes_be();
            let mut out = [0u8; 32];
            out[32 - raw.len()..].copy_from_slice(&raw);
            out
        };
        for s in [s, alternate] {
            let mut w = circuit.new_witness_filler();
            relation.populate(&mut w, &message, &qx, &qy, &r, &s).unwrap();
            circuit.populate_wire_witness(&mut w).unwrap();
            let valid = w.into_value_vec();
            circuit.constraint_system().verify(&valid).unwrap();
        }
        for (which, delta) in [(0usize, 1u8), (2, 1), (3, 1)] {
            let mut values = [qx, qy, r, s];
            values[which][31] ^= delta;
            let mut w = circuit.new_witness_filler();
            relation
                .populate(&mut w, &message, &values[0], &values[1], &values[2], &values[3])
                .unwrap();
            if circuit.populate_wire_witness(&mut w).is_ok() {
                assert!(
                    circuit.constraint_system().verify(&w.into_value_vec()).is_err(),
                    "accepted a changed public word {which}"
                );
            }
        }
        let mut changed = message.clone();
        changed[0] ^= 1;
        let mut w = circuit.new_witness_filler();
        relation.populate(&mut w, &changed, &qx, &qy, &r, &s).unwrap();
        if circuit.populate_wire_witness(&mut w).is_ok() {
            assert!(circuit.constraint_system().verify(&w.into_value_vec()).is_err());
        }
    }
}
