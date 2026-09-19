//! Fixed-length SHA-256 chain with standard secp256k1 signature verification.
//!
//! The secp256k1 counterpart of `binius_circuits::sha256_ecdsa`, built here from
//! that crate's public gadgets so the pinned Binius64 revision stays untouched.
//! Public statement, witness layout, message derivation and the SHA-256 chain
//! subcircuit are copied verbatim from the P-256 relation; only the signature
//! verifier differs, so the two profiles are directly comparable.
//!
//! The verifier is Binius64's own secp256k1 path (`ecdsa::bitcoin_verify`):
//! GLV endomorphism-split Straus MSM over a one-limb pseudo-Mersenne
//! coordinate field. That is the curve Binius64 is optimized for, so these rows
//! report the cheapest SHA+ECDSA proof it can produce, not a curve-only delta
//! against P-256's four-limb field and plain 4-bit joint window.

use binius_circuits::{
    bignum::{BigUint, biguint_lt, sub},
    ecdsa::bitcoin_verify,
    secp256k1::{Secp256k1, Secp256k1Affine},
    sha256::sha256_fixed,
    sha256_ecdsa::{limbs, message_len},
};
use binius_core::word::Word;
use binius_frontend::{CircuitBuilder, Wire, WitnessFiller};

pub const PROFILE: &str = "sha256-chain-secp256k1/standard/v1";

/// The fixed relation and its input wires; construction never takes an instance.
pub struct Sha256EcdsaSecp256k1 {
    log_compressions: u8,
    exponent: Wire,
    message: Vec<Wire>,
    public: [BigUint; 4],
}

/// Conditional single subtraction into `[0, modulus)`.
///
/// A copy of the P-256 relation's private helper, which is curve-agnostic: it
/// is sound whenever `value < 2 * modulus`, and both a 256-bit digest and an
/// affine x-coordinate are below `2n` for secp256k1's `n > 2^255`.
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
        let sub_b = b.subcircuit("secp256k1 verification");
        let curve = Secp256k1::new(&sub_b);
        // `assert_on_curve` inside the gadget checks the curve equation and
        // rejects the point at infinity, but not the coordinate range; P-256's
        // verifier does, so constrain it here to keep the statements matched.
        for (name, coordinate) in [("Q.x", &public[0]), ("Q.y", &public[1])] {
            sub_b.assert_true(
                format!("{name} < p"),
                biguint_lt(&sub_b, coordinate, curve.f_p().modulus()),
            );
        }
        // `bitcoin_verify` requires a dividend already in `[0, n)`.
        let z = reduce_scalar(&sub_b, &digest, curve.f_scalar().modulus());
        let pk = Secp256k1Affine {
            x: public[0].clone(),
            y: public[1].clone(),
            is_point_at_infinity: sub_b.add_constant(Word::ZERO),
        };
        // The gadget returns validity rather than asserting it; r != 0, s != 0,
        // r < n, s < n, R != O and R.x == r are all folded into this wire.
        let valid = bitcoin_verify(&sub_b, pk, &z, &public[2], &public[3]);
        sub_b.assert_true("secp256k1 signature valid", valid);
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
