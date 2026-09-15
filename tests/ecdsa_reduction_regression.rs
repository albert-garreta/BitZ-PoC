#![cfg(feature = "ecdsa")]

#[path = "../benches/common/peak_memory.rs"]
mod peak_memory;

use f2z::{
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
            // The 128-bit pins predate shared-MLE extraction. The 100-bit pins
            // were refreshed from commit 2733f5b, before the Wengert conversion change.
            let (expected_digest, expected_challenge) = match (target, mode) {
                (100, OuterMode::Split) => (
                    "96000cbe490d85af4041757e13a2765fe9a7d460731e647dc756a90405cdb491",
                    290968065569492799185031104448506609145,
                ),
                (100, OuterMode::AllRows) => (
                    "c9db6967611127613f6a615743824ca325237831ca35526b29c4e6e159eb3d7b",
                    228532452576271292614043867469500230475,
                ),
                (128, OuterMode::Split) => (
                    "d12a3c9c05bc060e1fec4a3c5e05a1195e66a0a8bb3f05f5a69f023ad91f5fcd",
                    214205501247843185117625529746659525401,
                ),
                (128, OuterMode::AllRows) => (
                    "a1604d666fc982c17a351521d4394e6926d374cd35af8c9ff49fbc2c15cfc9f8",
                    229917605542768170864421690634840036012,
                ),
                _ => unreachable!(),
            };
            assert_eq!(digest, expected_digest, "target={target} mode={mode:?}");
            assert_eq!(challenge, expected_challenge);
            println!(
                "target={target} mode={mode:?} digest={digest} challenge={challenge} prove_ms={prove_ms:.2} verify_ms={verify_ms:.2} verify_peak={verify_peak}"
            );
        }
    }
}
