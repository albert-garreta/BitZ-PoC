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
            // Pins are refreshed only after verification, for intentional
            // shared-codec or transcript changes.
            let (expected_digest, expected_challenge) = match (target, mode) {
                (100, OuterMode::Split) => (
                    "19cd2f6e59ddc4f9f9ca8a0b4753f0f4f03d4dfcd55b0ae923d1ef7c31e3d012",
                    308552237714315817015783313396758004326,
                ),
                (100, OuterMode::AllRows) => (
                    "a70a9a5586911aff7afe065f10c76341d893b5a210f4cd79b4d14b834bdad99d",
                    295269438920658387298075618801376680655,
                ),
                (128, OuterMode::Split) => (
                    "967e4baa20978bbdc32e133adda5fca5682f83fabacc0952afe2554702826572",
                    82361047506537771361838764072169021363,
                ),
                (128, OuterMode::AllRows) => (
                    "9a3c6d324ce88892d0141639b147592c8b2b9462182b306cf791729ad226015c",
                    179707200661787963014989635211131011310,
                ),
                _ => unreachable!(),
            };
            if std::env::var_os("F2Z_RECORD_PINS").is_none() {
                assert_eq!(digest, expected_digest, "target={target} mode={mode:?}");
                assert_eq!(challenge, expected_challenge);
            }
            println!(
                "target={target} mode={mode:?} digest={digest} challenge={challenge} prove_ms={prove_ms:.2} verify_ms={verify_ms:.2} verify_peak={verify_peak}"
            );
        }
    }
}
