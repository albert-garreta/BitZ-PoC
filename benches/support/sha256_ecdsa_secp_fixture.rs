//! Standard secp256k1 fixtures for the SHA-chain signature comparison.
//!
//! Field-for-field the P-256 fixture in `benches/support/sha256_ecdsa_fixture.rs`,
//! including its message and key derivation, so a given (exponent, seed) signs
//! exactly the same bytes on either curve. Only the signature scheme and the
//! schema string differ, and the schema is hashed into `id`, so a fixture of one
//! curve can never be replayed as the other.

use k256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::BufReader, io::BufWriter, path::Path};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub const SCHEMA: &str = "bitz/sha256-ecdsa-fixture/standard-secp256k1/v1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedFixture {
    pub schema: String,
    pub log_compressions: u8,
    pub seed: u64,
    pub message: Vec<u8>,
    pub qx: [u8; 32],
    pub qy: [u8; 32],
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub id: String,
}

impl SignedFixture {
    pub fn generate(exponent: u8, seed: u64) -> Result<Self> {
        if !(3..=16).contains(&exponent) {
            return Err("exponent must be in 3..16".into());
        }
        // Identical to the P-256 fixture's derivation: same message, same key bytes.
        let mut h = blake3::Hasher::new();
        h.update(b"sha256-ecdsa-compare/fixture/v1");
        h.update(&seed.to_le_bytes());
        h.update(&(exponent as u64).to_le_bytes());
        let mut message = vec![0; 64 * ((1usize << exponent) - 1)];
        h.finalize_xof().fill(&mut message);
        let mut bytes = blake3::derive_key("sha256-ecdsa-compare/key/v1", &seed.to_le_bytes());
        bytes[0] &= 0x7f;
        bytes[31] |= 1;
        let key = SigningKey::from_bytes((&bytes).into())?;
        let signature: Signature = key.sign(&message);
        let q = key.verifying_key().to_encoded_point(false);
        let (r, s) = signature.split_bytes();
        let mut fixture = Self {
            schema: SCHEMA.into(),
            log_compressions: exponent,
            seed,
            message,
            qx: (*q.x().ok_or("missing x")?).into(),
            qy: (*q.y().ok_or("missing y")?).into(),
            r: r.into(),
            s: s.into(),
            id: String::new(),
        };
        fixture.id = fixture.compute_id();
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn compute_id(&self) -> String {
        self.compute_id_under(SCHEMA)
    }

    fn compute_id_under(&self, schema: &str) -> String {
        let mut h = blake3::Hasher::new();
        h.update(schema.as_bytes());
        h.update(&[self.log_compressions]);
        h.update(&self.seed.to_le_bytes());
        h.update(&self.message);
        for word in [&self.qx, &self.qy, &self.r, &self.s] {
            h.update(word);
        }
        h.finalize().to_hex().to_string()
    }

    // This public-domain check is also part of measured application verification.
    pub fn validate_statement(&self) -> Result<()> {
        if !(3..=16).contains(&self.log_compressions) {
            return Err("invalid exponent".into());
        }
        Signature::from_scalars(self.r, self.s)?;
        self.verifying_key()?;
        Ok(())
    }

    fn verifying_key(&self) -> Result<VerifyingKey> {
        let point =
            k256::EncodedPoint::from_affine_coordinates(&self.qx.into(), &self.qy.into(), false);
        Ok(VerifyingKey::from_encoded_point(&point)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA {
            return Err("expected standard-secp256k1/v1 fixture; regenerate with --export-fixture".into());
        }
        self.validate_statement()?;
        if self.message.len() != 64 * ((1usize << self.log_compressions) - 1) {
            return Err("incorrect message length".into());
        }
        if self.id != self.compute_id() {
            return Err("fixture hash mismatch".into());
        }
        self.verifying_key()?
            .verify(&self.message, &Signature::from_scalars(self.r, self.s)?)?;
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let fixture: Self = serde_json::from_reader(BufReader::new(File::open(path)?))?;
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        serde_json::to_writer(BufWriter::new(File::create(path)?), self)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_round_trips_and_rejects_mutations() {
        let original = SignedFixture::generate(3, 0).unwrap();
        assert_eq!(original.message.len(), 448);
        original.validate().unwrap();
        let mut bad = original.clone();
        bad.message[0] ^= 1;
        bad.id = bad.compute_id();
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.r = [0; 32];
        assert!(bad.validate_statement().is_err());
        let mut bad = original.clone();
        bad.qx = [255; 32];
        assert!(bad.validate_statement().is_err());
        // The schema is hashed into `id`, so a P-256 fixture can never be
        // replayed as a secp256k1 one even though the fields line up.
        assert!(original.id != original.compute_id_under("bitz/sha256-ecdsa-fixture/standard-p256/v1"));
    }
}
