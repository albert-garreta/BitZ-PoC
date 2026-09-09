//! Shared, versioned low-s fixtures for native F2Z and Noir workers.
use p256::ecdsa::{
    signature::{Signer, Verifier},
    Signature, SigningKey, VerifyingKey,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub const SCHEMA: &str = "f2z/sha256-ecdsa-fixture/low-s/v1";

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
        // Preserve the existing campaign's message/key derivation.
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
        let signature = signature.normalize_s().unwrap_or(signature);
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
        let mut h = blake3::Hasher::new();
        h.update(SCHEMA.as_bytes());
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
        let signature = Signature::from_scalars(self.r, self.s)?;
        if signature.normalize_s().is_some() {
            return Err("signature must use low-s".into());
        }
        self.verifying_key()?;
        Ok(())
    }

    fn verifying_key(&self) -> Result<VerifyingKey> {
        let point =
            p256::EncodedPoint::from_affine_coordinates(&self.qx.into(), &self.qy.into(), false);
        Ok(VerifyingKey::from_encoded_point(&point)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA {
            return Err("unknown fixture schema".into());
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
    fn fixture_rejects_mutations_and_high_s() {
        let original = SignedFixture::generate(3, 0).unwrap();
        assert_eq!(original.message.len(), 448);
        assert!(Signature::from_scalars(original.r, original.s)
            .unwrap()
            .normalize_s()
            .is_none());
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
        let signature = Signature::from_scalars(original.r, original.s).unwrap();
        let high_s = -signature.s().as_ref();
        let mut bad = original;
        bad.s = high_s.to_bytes().into();
        assert!(bad.validate_statement().is_err());
    }
}
