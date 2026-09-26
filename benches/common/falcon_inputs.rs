//! Independent original-Falcon inputs shared by benchmarks and integration tests.
//! The deterministic seed is for reproducible benchmark keys only.

use bitz::piop::spartan::falcon1024_ct::{
    CT_SIGNATURE_BYTES, N, NONCE_BYTES, decode_signature_ct, verify_falcon1024_ct,
};
use fn_dsa::{
    DOMAIN_NONE, FN_DSA_LOGN_1024, HASH_ID_ORIGINAL_FALCON, KeyPairGenerator, KeyPairGenerator1024,
    SigningKey, SigningKey1024, VerifyingKey, VerifyingKey1024, sign_key_size, signature_size,
    vrfy_key_size,
};
use rand_chacha::{
    ChaCha20Rng,
    rand_core::{RngCore, SeedableRng},
};
use std::error::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FalconCase {
    pub public_key: Vec<u8>,
    pub message: Vec<u8>,
    /// The CT encoding consumed by BitZ.
    pub signature: Vec<u8>,
    /// Original padded/compressed bytes consumed by fn-dsa.
    pub upstream_signature: Vec<u8>,
}

impl FalconCase {
    pub fn hash_into(&self, hash: &mut blake3::Hasher) {
        for part in [&self.public_key, &self.message, &self.signature] {
            hash.update(&(part.len() as u64).to_le_bytes());
            hash.update(part);
        }
    }

    /// Includes decoding the encoded public key, as in a detached native verifier.
    pub fn verify_upstream(&self) -> Result<(), Box<dyn Error>> {
        let verifier = VerifyingKey1024::decode(&self.public_key)
            .ok_or("fn-dsa rejected the Falcon-1024 public key")?;
        if !verifier.verify(
            &self.upstream_signature,
            &DOMAIN_NONE,
            &HASH_ID_ORIGINAL_FALCON,
            &self.message,
        ) {
            return Err("fn-dsa rejected its generated Falcon-1024 signature".into());
        }
        Ok(())
    }
}

pub fn reject_fixture_overrides() -> Result<(), Box<dyn Error>> {
    for name in [
        "BITZ_FALCON_FIXTURE_MANIFEST",
        "BITZ_FALCON_PUBLIC_KEY",
        "BITZ_FALCON_SIGNATURE",
        "BITZ_FALCON_MESSAGE",
    ] {
        if std::env::var_os(name).is_some() {
            return Err(format!(
                "{name} is no longer supported; Falcon benchmarks generate distinct fn-dsa inputs"
            )
            .into());
        }
    }
    Ok(())
}

/// Generate distinct keys and 32-byte messages, verify upstream immediately,
/// convert the signature, and preflight the native BitZ verifier.
pub fn generate_cases(count: usize, seed: u64) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut keygen = KeyPairGenerator1024::default();
    let mut cases = Vec::with_capacity(count);
    for index in 0..count {
        let mut secret = vec![0u8; sign_key_size(FN_DSA_LOGN_1024)];
        let mut public_key = vec![0u8; vrfy_key_size(FN_DSA_LOGN_1024)];
        keygen.keygen(FN_DSA_LOGN_1024, &mut rng, &mut secret, &mut public_key);
        let mut message = vec![0u8; 32];
        rng.fill_bytes(&mut message);
        message[..8].copy_from_slice(&(index as u64).to_le_bytes());
        let mut signer =
            SigningKey1024::decode(&secret).ok_or("fn-dsa rejected its generated signing key")?;
        let mut upstream_signature = vec![0u8; signature_size(FN_DSA_LOGN_1024)];
        signer.sign(
            &mut rng,
            &DOMAIN_NONE,
            &HASH_ID_ORIGINAL_FALCON,
            &message,
            &mut upstream_signature,
        );
        let mut case = FalconCase {
            public_key,
            message,
            signature: Vec::new(),
            upstream_signature,
        };
        case.verify_upstream()?;
        case.signature = compressed_to_ct(&case.upstream_signature)?;
        verify_falcon1024_ct(&case.public_key, &case.message, &case.signature)?;
        cases.push(case);
    }
    Ok(cases)
}

/// Preserve the 40-byte nonce and all 1024 signed coefficients, changing only
/// their serialization from upstream's compressed form to Falcon's CT format.
pub fn compressed_to_ct(signature: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if signature.len() != signature_size(FN_DSA_LOGN_1024) || signature[0] != 0x3a {
        return Err("expected a padded original Falcon-1024 signature".into());
    }
    let mut s2 = [0i16; N];
    if !fn_dsa_comm::codec::comp_decode(&signature[1 + NONCE_BYTES..], &mut s2) {
        return Err("noncanonical compressed Falcon signature".into());
    }
    let mut ct = Vec::with_capacity(CT_SIGNATURE_BYTES);
    ct.push(0x5a);
    ct.extend_from_slice(&signature[1..1 + NONCE_BYTES]);
    for pair in s2.chunks_exact(2) {
        let a = (pair[0] as u16) & 0x0fff;
        let b = (pair[1] as u16) & 0x0fff;
        ct.extend_from_slice(&[(a >> 4) as u8, ((a << 4) | (b >> 8)) as u8, b as u8]);
    }
    let decoded = decode_signature_ct(&ct)?;
    if decoded.nonce.as_slice() != &signature[1..1 + NONCE_BYTES] || decoded.s2.as_ref() != &s2 {
        return Err("Falcon CT conversion changed the nonce or signature coefficients".into());
    }
    Ok(ct)
}
