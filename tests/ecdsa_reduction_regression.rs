#![cfg(feature = "ecdsa")]

#[path = "../benches/common/peak_memory.rs"]
mod peak_memory;

use bitz::{
    piop::spartan::ecdsa_sha256::{
        OuterMode, Sha256EcdsaStatement, commit_sha256_ecdsa, generate_sha256_ecdsa_witness,
        prepare_sha256_ecdsa, prove_sha256_ecdsa, verify_sha256_ecdsa,
    },
    transcript::{Blake3Transcript, traits::Transcript},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: peak_memory::PeakAlloc = peak_memory::PeakAlloc;

/// Run separately so allocator measurements exclude concurrent tests.
#[test]
#[ignore = "deterministic end-to-end proof compatibility and allocation measurement"]
fn proof_bytes_and_verifier_allocations() {
    for target in [100, 128] {
        for mode in [OuterMode::Split, OuterMode::AllRows] {
            let prepared = prepare_sha256_ecdsa(3, target, mode).unwrap();
            let message: Vec<_> = (0..prepared.message_bytes()).map(|i| i as u8).collect();
            let key = SigningKey::from_bytes((&[7u8; 32]).into()).unwrap();
            let signature: Signature = key.sign(&message);
            let point = key.verifying_key().to_encoded_point(false);
            let (r, s) = signature.split_bytes();
            let statement = Sha256EcdsaStatement {
                log_compressions: 3,
                qx: point.x().unwrap().as_slice().try_into().unwrap(),
                qy: point.y().unwrap().as_slice().try_into().unwrap(),
                r: r.into(),
                s: s.into(),
            };
            let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message).unwrap();
            let hint = commit_sha256_ecdsa(&prepared, &witness).unwrap();
            let mut prover = Blake3Transcript::new();
            let started = Instant::now();
            let proof =
                prove_sha256_ecdsa(&mut prover, &prepared, &statement, &witness, &hint, 4).unwrap();
            let prove_ms = started.elapsed().as_secs_f64() * 1000.;
            let digest = blake3::hash(&proof.to_bytes()).to_hex().to_string();
            let mut verifier = Blake3Transcript::new();
            let live = peak_memory::live_bytes();
            peak_memory::reset_peak();
            let started = Instant::now();
            verify_sha256_ecdsa(
                &mut verifier,
                &prepared,
                &statement,
                &hint.commitment,
                &proof,
            )
            .unwrap();
            let verify_ms = started.elapsed().as_secs_f64() * 1000.;
            let verify_peak = peak_memory::peak_bytes().saturating_sub(live);
            let challenge = prover.get_challenge::<u128>();
            assert_eq!(challenge, verifier.get_challenge::<u128>());
            // Pins are refreshed only after verification, for intentional
            // shared-codec or transcript changes.
            let (expected_digest, expected_challenge) = match (target, mode) {
                (100, OuterMode::Split) => (
                    "c6df4554dda0480a4cf05c935443eec04830afc95f257514dca8caf0b242928c",
                    297091395608428809527576424278876215040,
                ),
                (100, OuterMode::AllRows) => (
                    "146a9b37a07576107676e7fca1a3c3c89d18fa08e24c296a4b46acaf701f8ae5",
                    63194987174164003940923341848144090108,
                ),
                (128, OuterMode::Split) => (
                    "781d3eb594fbaf1186f0b5ef849e26ad9a77f19e09a279fccd27e1764c4b105c",
                    40382681399174221331594612286671065927,
                ),
                (128, OuterMode::AllRows) => (
                    "9aa4fe3fecbfa61937da6aedb9159144c530a3a7edf7b394f1e52312f39ff15e",
                    109026203495331847305993758901892716094,
                ),
                _ => unreachable!(),
            };
            if std::env::var_os("BITZ_RECORD_PINS").is_none() {
                assert_eq!(digest, expected_digest, "target={target} mode={mode:?}");
                assert_eq!(challenge, expected_challenge);
            }
            println!(
                "target={target} mode={mode:?} digest={digest} challenge={challenge} prove_ms={prove_ms:.2} verify_ms={verify_ms:.2} verify_peak={verify_peak}"
            );
        }
    }
}
