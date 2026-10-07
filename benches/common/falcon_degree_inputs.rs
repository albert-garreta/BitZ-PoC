//! Independent original-Falcon inputs shared by benchmarks and integration tests.
//! The deterministic seed is for reproducible benchmark keys only.

use bitz::piop::spartan::falcon_parameters::FalconParameters;
use fn_dsa::{
    DOMAIN_NONE, HASH_ID_ORIGINAL_FALCON, KeyPairGenerator, KeyPairGeneratorStandard, SigningKey,
    SigningKeyStandard, VerifyingKey, VerifyingKeyStandard, sign_key_size, signature_size,
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
        let verifier = VerifyingKeyStandard::decode(&self.public_key)
            .ok_or("fn-dsa rejected the Falcon public key")?;
        if !verifier.verify(
            &self.upstream_signature,
            &DOMAIN_NONE,
            &HASH_ID_ORIGINAL_FALCON,
            &self.message,
        ) {
            return Err("fn-dsa rejected its generated Falcon signature".into());
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
pub fn generate_cases(
    degree: usize,
    count: usize,
    seed: u64,
) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    if let Some(directory) = std::env::var_os("BITZ_FALCON_CASE_CACHE") {
        return cached_cases(Path::new(&directory), degree, count, seed);
    }
    generate_cases_uncached(degree, count, seed)
}

const CASE_CACHE_FORMAT: &str = "bitz/falcon-degree-cases/v1;fn-dsa=0.3.0;original-falcon";

#[derive(Serialize, Deserialize)]
struct CaseCache {
    format: String,
    degree: usize,
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
    degree: usize,
    count: usize,
    seed: u64,
    cases: Vec<FalconCase>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = bincode::serialize(&CaseCache {
        format: CASE_CACHE_FORMAT.into(),
        degree,
        count,
        seed,
        input_digest: input_digest(&cases),
        cases,
    })?;
    let checksum = *blake3::hash(&bytes).as_bytes();
    bytes.extend_from_slice(&checksum);
    Ok(bytes)
}

fn decode_cache(
    bytes: &[u8],
    degree: usize,
    count: usize,
    seed: u64,
) -> Result<Vec<FalconCase>, Box<dyn Error>> {
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
        || cache.degree != degree
        || cache.count != count
        || cache.seed != seed
        || cache.cases.len() != count
        || cache.input_digest != input_digest(&cache.cases)
    {
        return Err("Falcon input cache metadata/digest mismatch".into());
    }
    for (index, case) in cache.cases.iter().enumerate() {
        if case.public_key.len() != vrfy_key_size(degree.ilog2())
            || case.message.len() != 32
            || case.message[..8] != (index as u64).to_le_bytes()
        {
            return Err("Falcon input cache case shape mismatch".into());
        }
        case.verify_upstream()?;
        if compressed_to_ct(degree, &case.upstream_signature)? != case.signature {
            return Err("Falcon input cache CT signature mismatch".into());
        }
        verify_native(degree, case)?;
    }
    Ok(cache.cases)
}

/// Cache only deterministic public cases, with the same upstream/native
/// preflight checks as generation. Cache I/O is outside all proof timings.
fn cached_cases(
    directory: &Path,
    degree: usize,
    count: usize,
    seed: u64,
) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("fn-dsa-0.3.0-n{degree}-b{count}-seed{seed}.bin"));
    if path.exists() {
        return decode_cache(&fs::read(path)?, degree, count, seed);
    }
    let cases = generate_cases_uncached(degree, count, seed)?;
    let bytes = encode_cache(degree, count, seed, cases.clone())?;
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
            decode_cache(&fs::read(path)?, degree, count, seed)
        }
        Err(error) => {
            let _ = fs::remove_file(temporary);
            Err(error.into())
        }
    }
}

fn generate_cases_uncached(
    degree: usize,
    count: usize,
    seed: u64,
) -> Result<Vec<FalconCase>, Box<dyn Error>> {
    if !matches!(degree, 512 | 1024) {
        return Err("degree must be 512 or 1024".into());
    }
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut keygen = KeyPairGeneratorStandard::default();
    let mut cases = Vec::with_capacity(count);
    for index in 0..count {
        let mut secret = vec![0u8; sign_key_size(degree.ilog2())];
        let mut public_key = vec![0u8; vrfy_key_size(degree.ilog2())];
        keygen.keygen(degree.ilog2(), &mut rng, &mut secret, &mut public_key);
        let mut message = vec![0u8; 32];
        rng.fill_bytes(&mut message);
        message[..8].copy_from_slice(&(index as u64).to_le_bytes());
        let mut signer = SigningKeyStandard::decode(&secret)
            .ok_or("fn-dsa rejected its generated signing key")?;
        let mut upstream_signature = vec![0u8; signature_size(degree.ilog2())];
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
        case.signature = compressed_to_ct(degree, &case.upstream_signature)?;
        verify_native(degree, &case)?;
        cases.push(case);
    }
    Ok(cases)
}

/// Convert original Falcon's compressed signature to the protocol's CT encoding.
/// This is serialization only: preserve all signed coefficients and the nonce.
pub fn compressed_to_ct(degree: usize, signature: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let params = FalconParameters::for_degree(degree);
    let log = degree.ilog2();
    if signature.len() != signature_size(log) || signature[0] != 0x30 + log as u8 {
        return Err("expected a padded original Falcon signature matching degree".into());
    }
    let mut coefficients = vec![0i16; degree];
    if !fn_dsa_comm::codec::comp_decode(&signature[41..], &mut coefficients) {
        return Err("noncanonical compressed Falcon signature".into());
    }
    let mut encoded = vec![0; params.signature_bytes()];
    encoded[0] = 0x50 + log as u8;
    encoded[1..41].copy_from_slice(&signature[1..41]);
    for (index, coefficient) in coefficients.into_iter().enumerate() {
        for bit in 0..params.signature_bits {
            let offset = index * params.signature_bits + bit;
            encoded[41 + offset / 8] |= (((coefficient as u16) >> (params.signature_bits - 1 - bit)
                & 1) as u8)
                << (7 - offset % 8);
        }
    }
    Ok(encoded)
}

fn verify_native(degree: usize, case: &FalconCase) -> Result<(), Box<dyn Error>> {
    match degree {
        512 => bitz::piop::spartan::falcon_profiles::n512_k9::verify_falcon_ct(
            &case.public_key,
            &case.message,
            &case.signature,
        )
        .map_err(Into::into),
        1024 => bitz::piop::spartan::falcon_profiles::n1024_k9::verify_falcon_ct(
            &case.public_key,
            &case.message,
            &case.signature,
        )
        .map_err(Into::into),
        _ => Err("degree must be 512 or 1024".into()),
    }
}

#[cfg(test)]
mod cache_tests {
    #[test]
    fn both_degrees_preserve_generated_cases_and_reject_tampering() {
        for degree in [512, 1024] {
            let cases = super::generate_cases_uncached(degree, 2, 42).unwrap();
            let bytes = super::encode_cache(degree, 2, 42, cases.clone()).unwrap();
            assert_eq!(super::decode_cache(&bytes, degree, 2, 42).unwrap(), cases);
            assert!(super::decode_cache(&bytes, degree, 1, 42).is_err());
            assert!(super::decode_cache(&bytes, degree, 2, 43).is_err());
            assert!(super::decode_cache(&bytes, 1536 - degree, 2, 42).is_err());
            let mut corrupted = bytes.clone();
            corrupted[100] ^= 1;
            assert!(super::decode_cache(&corrupted, degree, 2, 42).is_err());
            let mut invalid = cases;
            invalid[0].signature[20] ^= 1;
            assert!(
                super::decode_cache(
                    &super::encode_cache(degree, 2, 42, invalid).unwrap(),
                    degree,
                    2,
                    42
                )
                .is_err()
            );
        }
    }
}
