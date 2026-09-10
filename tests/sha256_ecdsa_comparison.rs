#![cfg(feature = "sha256-ecdsa-compare")]

#[path = "../benches/support/sha256_ecdsa_fixture.rs"]
mod fixture;
#[path = "../benches/support/sha256_ecdsa_test_vectors.rs"]
mod vectors;

use f2z::{piop::spartan::ecdsa_sha256::*, transcript::Blake3Transcript};

#[test]
fn both_s_forms_and_exceptional_nonce_verify_in_all_native_methods() {
    let fixtures = vectors::vectors();
    for mode in [OuterMode::Split, OuterMode::AllRows] {
        let prepared = prepare_sha256_ecdsa(3, 100, mode).unwrap();
        for f in &fixtures {
            let statement = Sha256EcdsaStatement {
                log_compressions: 3,
                qx: f.qx,
                qy: f.qy,
                r: f.r,
                s: f.s,
            };
            let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &f.message).unwrap();
            let hint = commit_sha256_ecdsa(&prepared, &witness).unwrap();
            let proof = prove_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &witness,
                &hint,
                4,
            )
            .unwrap();
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &hint.commitment,
                &proof,
            )
            .unwrap();
        }
    }
    for (r, c) in [(3, 0), (1, 2)] {
        let prepared = spartan2::sha256_ecdsa::Prepared::setup(r, c).unwrap();
        for f in &fixtures {
            let statement = spartan2::sha256_ecdsa::Statement {
                log_compressions: 3,
                qx: f.qx,
                qy: f.qy,
                r: f.r,
                s: f.s,
            };
            let witness = prepared.generate_witness(&statement, &f.message).unwrap();
            let committed = prepared.commit(witness).unwrap();
            let (proof, _) = prepared.prove(&statement, &committed).unwrap();
            prepared.verify(&statement, &proof).unwrap();
        }
    }
}

#[test]
fn fixtures_roundtrip_and_support_full_length_range() {
    for exponent in [11, 16] {
        let f = fixture::SignedFixture::generate(exponent, 1).unwrap();
        assert_eq!(f.message.len(), 64 * ((1usize << exponent) - 1));
        let path = std::env::temp_dir().join(format!(
            "sha256-ecdsa-{}-i{exponent}.json",
            std::process::id()
        ));
        f.write(&path).unwrap();
        let decoded = fixture::SignedFixture::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(decoded.id, f.id);
    }
}
