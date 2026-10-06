//! Independent original-Falcon inputs shared by benchmarks and integration tests.
//! The deterministic seed is for reproducible benchmark keys only.

use bitz::piop::spartan::falcon1024_ct::{
    FalconSignatureCt, N, NONCE_BYTES, encode_signature_ct, verify_falcon1024_ct,
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
use serde::{Deserialize, Serialize};
use std::{error::Error, fs, io::Write, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    if let Some(directory) = std::env::var_os("BITZ_FALCON_CASE_CACHE") {
        return cached_cases(Path::new(&directory), count, seed);
    }
    generate_cases_uncached(count, seed)
}

const CASE_CACHE_FORMAT: &str = "bitz/falcon-cases/v1;fn-dsa=0.3.0;original-falcon-1024";

#[derive(Serialize, Deserialize)]
struct CaseCache {
    format: String,
    count: usize,
    seed: u64,
    input_digest: [u8; 32],
    cases: Vec<FalconCase>,
}

fn input_digest(cases: &[FalconCase]) -> [u8; 32] {
    let mut hash = blake3::Hasher::new();
    for case in cases {
        case.hash_into(&mut hash);
    }
    *hash.finalize().as_bytes()
}

fn encode_cache(
    count: usize,
    seed: u64,
    cases: Vec<FalconCase>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = bincode::serialize(&CaseCache {
        format: CASE_CACHE_FORMAT.into(),
        count,
        seed,
        input_digest: input_digest(&cases),
        cases,
    })?;
    let checksum = *blake3::hash(&bytes).as_bytes();
    bytes.extend_from_slice(&checksum);
    Ok(bytes)
}

fn decode_cache(bytes: &[u8], count: usize, seed: u64) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    use bincode::Options;

    let payload_len = bytes
        .len()
        .checked_sub(32)
        .ok_or("truncated Falcon input cache")?;
    if blake3::hash(&bytes[..payload_len]).as_bytes() != &bytes[payload_len..] {
        return Err("Falcon input cache checksum mismatch".into());
    }
    let cache: CaseCache = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(payload_len as u64)
        .reject_trailing_bytes()
        .deserialize(&bytes[..payload_len])?;
    if cache.format != CASE_CACHE_FORMAT
        || cache.count != count
        || cache.seed != seed
        || cache.cases.len() != count
        || cache.input_digest != input_digest(&cache.cases)
    {
        return Err("Falcon input cache metadata/digest mismatch".into());
    }
    for (index, case) in cache.cases.iter().enumerate() {
        if case.public_key.len() != vrfy_key_size(FN_DSA_LOGN_1024)
            || case.message.len() != 32
            || case.message[..8] != (index as u64).to_le_bytes()
        {
            return Err("Falcon input cache case shape mismatch".into());
        }
        case.verify_upstream()?;
        if compressed_to_ct(&case.upstream_signature)? != case.signature {
            return Err("Falcon input cache CT signature mismatch".into());
        }
        verify_falcon1024_ct(&case.public_key, &case.message, &case.signature)?;
    }
    Ok(cache.cases)
}

/// Cache only deterministic public cases, with the same upstream/native
/// preflight checks as generation. Cache I/O is outside all proof timings.
fn cached_cases(
    directory: &Path,
    count: usize,
    seed: u64,
) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("fn-dsa-0.3.0-b{count}-seed{seed}.bin"));
    if path.exists() {
        return decode_cache(&fs::read(path)?, count, seed);
    }
    let cases = generate_cases_uncached(count, seed)?;
    let bytes = encode_cache(count, seed, cases.clone())?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    // A concurrent generator may publish the same deterministic fixture. Never
    // overwrite an existing cache; validate the winner before returning it.
    match fs::hard_link(&temporary, &path) {
        Ok(()) => {
            fs::remove_file(temporary)?;
            Ok(cases)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(temporary)?;
            decode_cache(&fs::read(path)?, count, seed)
        }
        Err(error) => {
            let _ = fs::remove_file(temporary);
            Err(error.into())
        }
    }
}

fn generate_cases_uncached(count: usize, seed: u64) -> Result<Vec<FalconCase>, Box<dyn Error>> {
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

#[cfg(test)]
mod cache_tests {
    #[test]
    fn cached_cases_preserve_generated_cases_and_reject_tampering() {
        let cases = super::generate_cases_uncached(2, 42).unwrap();
        let bytes = super::encode_cache(2, 42, cases.clone()).unwrap();
        assert_eq!(super::decode_cache(&bytes, 2, 42).unwrap(), cases);
        assert!(super::decode_cache(&bytes, 1, 42).is_err());
        assert!(super::decode_cache(&bytes, 2, 43).is_err());
        let mut corrupted = bytes.clone();
        corrupted[100] ^= 1;
        assert!(super::decode_cache(&corrupted, 2, 42).is_err());
        let mut invalid = cases;
        invalid[0].signature[20] ^= 1;
        // A recomputed cache checksum cannot hide a mismatch between the
        // upstream signature and its claimed CT encoding.
        assert!(super::decode_cache(&super::encode_cache(2, 42, invalid).unwrap(), 2, 42).is_err());
    }
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
    Ok(encode_signature_ct(&FalconSignatureCt {
        nonce: signature[1..1 + NONCE_BYTES].try_into()?,
        s2: Box::new(s2),
    })?
    .to_vec())
}
