//! Shared standard fixtures for SHA-chain signature comparisons: one key and
//! one signature per (curve, exponent, seed), signed and validated with the
//! RustCrypto `p256` / `k256` crates outside every timer.
#[path = "../common/output.rs"]
mod output;
use output::{BenchmarkOutput, FileMode, JsonStyle};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::BufReader, path::Path};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// The P-256 fixture profile; `SignedFixture::schema` names the curve.
pub const SCHEMA: &str = "bitz/sha256-ecdsa-fixture/standard-p256/v1";
pub const SECP256K1_SCHEMA: &str = "bitz/sha256-ecdsa-fixture/standard-secp256k1/v1";

/// The curve of a fixture. P-256 keeps the original key derivation so its
/// fixtures are unchanged; secp256k1 keys use their own derivation context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Curve {
    P256,
    Secp256k1,
}

impl Curve {
    pub const ALL: [Self; 2] = [Self::P256, Self::Secp256k1];

    pub fn token(self) -> &'static str {
        match self {
            Self::P256 => "p256",
            Self::Secp256k1 => "secp256k1",
        }
    }

    pub fn parse(token: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|c| c.token() == token)
            .ok_or_else(|| format!("unknown curve {token}; expected p256 or secp256k1").into())
    }

    pub fn schema(self) -> &'static str {
        match self {
            Self::P256 => SCHEMA,
            Self::Secp256k1 => SECP256K1_SCHEMA,
        }
    }

    fn key_context(self) -> &'static str {
        match self {
            Self::P256 => "sha256-ecdsa-compare/key/v1",
            Self::Secp256k1 => "sha256-ecdsa-compare/key/secp256k1/v1",
        }
    }
}

fn default_curve() -> Curve {
    Curve::P256
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedFixture {
    pub schema: String,
    #[serde(default = "default_curve")]
    pub curve: Curve,
    pub log_compressions: u8,
    pub seed: u64,
    pub message: Vec<u8>,
    pub qx: [u8; 32],
    pub qy: [u8; 32],
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub id: String,
}

/// `(Q.x, Q.y, r, s)` of a fresh signature over `message` under a key
/// derived from `bytes`, on the requested curve.
fn sign(curve: Curve, bytes: &[u8; 32], message: &[u8]) -> Result<[[u8; 32]; 4]> {
    macro_rules! with {
        ($curve:ident) => {{
            use $curve::ecdsa::{Signature, SigningKey, signature::Signer};
            let key = SigningKey::from_bytes(bytes.into())?;
            let signature: Signature = key.sign(message);
            let q = key.verifying_key().to_encoded_point(false);
            let (r, s) = signature.split_bytes();
            Ok([
                (*q.x().ok_or("missing x")?).into(),
                (*q.y().ok_or("missing y")?).into(),
                r.into(),
                s.into(),
            ])
        }};
    }
    match curve {
        Curve::P256 => with!(p256),
        Curve::Secp256k1 => with!(k256),
    }
}

impl SignedFixture {
    pub fn generate(curve: Curve, exponent: u8, seed: u64) -> Result<Self> {
        if !(3..=16).contains(&exponent) {
            return Err("exponent must be in 3..16".into());
        }
        // Preserve the existing campaign's message/key derivation; the
        // message is the same on both curves.
        let mut h = blake3::Hasher::new();
        h.update(b"sha256-ecdsa-compare/fixture/v1");
        h.update(&seed.to_le_bytes());
        h.update(&(exponent as u64).to_le_bytes());
        let mut message = vec![0; 64 * ((1usize << exponent) - 1)];
        h.finalize_xof().fill(&mut message);
        let mut bytes = blake3::derive_key(curve.key_context(), &seed.to_le_bytes());
        bytes[0] &= 0x7f;
        bytes[31] |= 1;
        let [qx, qy, r, s] = sign(curve, &bytes, &message)?;
        let mut fixture = Self {
            schema: curve.schema().into(),
            curve,
            log_compressions: exponent,
            seed,
            message,
            qx,
            qy,
            r,
            s,
            id: String::new(),
        };
        fixture.id = fixture.compute_id();
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn compute_id(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(self.curve.schema().as_bytes());
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
        self.check(None)
    }

    /// Statement validity, and the signature over `message` when given.
    /// Both `s` forms are valid statements on both curves (as in the
    /// circuits); `k256` verifies low-s only, so its check normalizes `s`.
    fn check(&self, message: Option<&[u8]>) -> Result<()> {
        macro_rules! with {
            ($curve:ident) => {{
                use $curve::ecdsa::{Signature, VerifyingKey, signature::Verifier};
                let signature = Signature::from_scalars(self.r, self.s)?;
                let signature = signature.normalize_s().unwrap_or(signature);
                let point = $curve::EncodedPoint::from_affine_coordinates(
                    &self.qx.into(),
                    &self.qy.into(),
                    false,
                );
                let key = VerifyingKey::from_encoded_point(&point)?;
                if let Some(message) = message {
                    key.verify(message, &signature)?;
                }
                Ok(())
            }};
        }
        match self.curve {
            Curve::P256 => with!(p256),
            Curve::Secp256k1 => with!(k256),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != self.curve.schema() {
            return Err(format!(
                "expected {} fixture; regenerate legacy fixtures with --export-fixture",
                self.curve.schema()
            )
            .into());
        }
        self.validate_statement()?;
        if self.message.len() != 64 * ((1usize << self.log_compressions) - 1) {
            return Err("incorrect message length".into());
        }
        if self.id != self.compute_id() {
            return Err("fixture hash mismatch".into());
        }
        self.check(Some(&self.message))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let fixture: Self = serde_json::from_reader(BufReader::new(File::open(path)?))?;
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        BenchmarkOutput::new("").write_json(path, self, FileMode::Replace, JsonStyle::Compact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_rejects_mutations_and_accepts_both_s_forms() {
        let original = SignedFixture::generate(Curve::P256, 3, 0).unwrap();
        assert_eq!(original.message.len(), 448);
        assert_eq!(original.curve, Curve::P256);
        assert_eq!(original.schema, SCHEMA);
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
        let signature = p256::ecdsa::Signature::from_scalars(original.r, original.s).unwrap();
        let alternate_s = -signature.s().as_ref();
        let mut bad = original;
        bad.s = alternate_s.to_bytes().into();
        bad.id = bad.compute_id();
        bad.validate().unwrap();
        bad.schema = "bitz/sha256-ecdsa-fixture/low-s/v1".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn secp256k1_fixtures_are_distinct_and_curve_checked() {
        let p = SignedFixture::generate(Curve::P256, 3, 0).unwrap();
        let k = SignedFixture::generate(Curve::Secp256k1, 3, 0).unwrap();
        assert_eq!(p.message, k.message);
        assert_ne!(p.id, k.id);
        assert_ne!(p.qx, k.qx);
        assert_eq!(k.schema, SECP256K1_SCHEMA);
        assert_eq!(Curve::parse("secp256k1").unwrap(), Curve::Secp256k1);
        assert!(Curve::parse("secp256r1").is_err());
        // A P-256 statement is not a secp256k1 one and vice versa.
        let mut swapped = k.clone();
        swapped.curve = Curve::P256;
        swapped.schema = SCHEMA.into();
        swapped.id = swapped.compute_id();
        assert!(swapped.validate().is_err());
        let mut swapped = p.clone();
        swapped.curve = Curve::Secp256k1;
        swapped.schema = SECP256K1_SCHEMA.into();
        swapped.id = swapped.compute_id();
        assert!(swapped.validate().is_err());
        let signature = k256::ecdsa::Signature::from_scalars(k.r, k.s).unwrap();
        let mut high_s = k;
        high_s.s = (-signature.s().as_ref()).to_bytes().into();
        high_s.id = high_s.compute_id();
        high_s.validate().unwrap();
    }
}
