#![cfg(feature = "falcon-hybrid")]

#[path = "../benches/common/falcon_degree_inputs.rs"]
mod falcon_degree_inputs;

use bitz::piop::spartan::falcon_profiles::n1024_k11::{decode_signature_ct, verify_falcon_ct};
use bitz::piop::spartan::falcon_profiles::{FalconPublicStatement, PreparedFalconHybrid};
bitz::falcon_profile! { pub UpstreamProfile { n:1024, security_bits:100, max_batch:1024, ring_extension:Explicit(11), } }
use falcon_degree_inputs::{compressed_to_ct, generate_cases};
use std::{collections::HashSet, error::Error};

#[test]
fn benchmark_inputs_are_distinct_reproducible_and_interoperable() -> Result<(), Box<dyn Error>> {
    let cases = generate_cases(1024, 32, 42)?;
    assert_eq!(cases.len(), 32);
    assert_eq!(
        cases
            .iter()
            .map(|c| &c.public_key)
            .collect::<HashSet<_>>()
            .len(),
        32
    );
    assert_eq!(
        cases
            .iter()
            .map(|c| &c.message)
            .collect::<HashSet<_>>()
            .len(),
        32
    );
    assert_eq!(&cases[..2], generate_cases(1024, 2, 42)?.as_slice());

    // Both implementations must reject when the signed message changes.
    let mut altered = cases[0].clone();
    altered.message[0] ^= 1;
    assert!(altered.verify_upstream().is_err());
    assert!(verify_falcon_ct(&altered.public_key, &altered.message, &altered.signature).is_err());

    let mut digest = blake3::Hasher::new();
    cases[0].hash_into(&mut digest);
    let original_digest = digest.finalize();
    let mut digest = blake3::Hasher::new();
    altered.hash_into(&mut digest);
    assert_ne!(original_digest, digest.finalize());
    Ok(())
}

#[test]
fn conversion_rejects_noncanonical_compressed_encodings() -> Result<(), Box<dyn Error>> {
    let cases = generate_cases(1024, 1, 7)?;
    let original = &cases[0].upstream_signature;
    assert!(compressed_to_ct(1024, &original[..original.len() - 1]).is_err());
    let mut changed = original.clone();
    changed[0] = 0x39; // Wrong degree.
    assert!(compressed_to_ct(1024, &changed).is_err());
    let mut changed = original.clone();
    // A sign bit, zero low magnitude, and unary terminator encodes forbidden -0.
    changed[41] = 0x80;
    changed[42] |= 0x80;
    assert!(compressed_to_ct(1024, &changed).is_err());
    Ok(())
}

#[test]
fn proof_binds_the_exact_public_upstream_signature() -> Result<(), Box<dyn Error>> {
    use fn_dsa::{
        DOMAIN_NONE, FN_DSA_LOGN_1024, HASH_ID_ORIGINAL_FALCON, KeyPairGenerator,
        KeyPairGenerator1024, SigningKey, SigningKey1024, VerifyingKey, VerifyingKey1024,
        sign_key_size, signature_size, vrfy_key_size,
    };
    use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

    let mut rng = ChaCha20Rng::seed_from_u64(73);
    let mut secret = vec![0; sign_key_size(FN_DSA_LOGN_1024)];
    let mut public_key = vec![0; vrfy_key_size(FN_DSA_LOGN_1024)];
    KeyPairGenerator1024::default().keygen(
        FN_DSA_LOGN_1024,
        &mut rng,
        &mut secret,
        &mut public_key,
    );
    let mut signer = SigningKey1024::decode(&secret).unwrap();
    let verifier = VerifyingKey1024::decode(&public_key).unwrap();
    let message = [5; 32];
    let mut signatures = Vec::new();
    for _ in 0..2 {
        let mut signature = vec![0; signature_size(FN_DSA_LOGN_1024)];
        signer.sign(
            &mut rng,
            &DOMAIN_NONE,
            &HASH_ID_ORIGINAL_FALCON,
            &message,
            &mut signature,
        );
        assert!(verifier.verify(&signature, &DOMAIN_NONE, &HASH_ID_ORIGINAL_FALCON, &message));
        let ct = compressed_to_ct(1024, &signature)?;
        verify_falcon_ct(&public_key, &message, &ct)?;
        signatures.push(ct);
    }
    assert_ne!(signatures[0], signatures[1]);

    let prepared = PreparedFalconHybrid::<UpstreamProfile>::new(1)?;
    let public = FalconPublicStatement::<UpstreamProfile>::from_bytes(
        &[&public_key],
        &[&message],
        &[&signatures[0]],
    )?;
    let committed = prepared.commit(public)?;
    let statement = committed.statement.clone();
    let proof = prepared.prove(committed)?;
    prepared.verify(&statement, &proof)?;

    // The replacement is valid for the same key and message, but is not the
    // exact public signature authenticated by this proof.
    let mut changed = statement.clone();
    changed.public.signatures[0] = decode_signature_ct(&signatures[1])?;
    assert!(prepared.verify(&changed, &proof).is_err());
    Ok(())
}
