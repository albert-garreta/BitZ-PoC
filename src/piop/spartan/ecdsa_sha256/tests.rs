use super::*;
use crate::f2map::VirtualMap;
use num_bigint::{BigInt, BigUint};
use sha2::{Digest, Sha256};

fn word(value: &BigUint) -> [u8; 32] {
    let bytes = value.to_bytes_be();
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

pub(super) fn fixture() -> (Sha256EcdsaStatement, Vec<u8>) {
    let hex = |s: &[u8]| BigUint::parse_bytes(s, 16).unwrap();
    let gx = hex(b"6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296");
    let gy = hex(b"4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5");
    let n = hex(b"ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");
    // Independently computed SHA-256 of bytes[i] = i mod 256, length 448.
    let digest = hex(b"afcdb4646801a7f0c78048754ff01adec0da00eb73b20dc0dde7f089c2c24640");
    let message: Vec<_> = (0..448).map(|i| i as u8).collect();
    assert_eq!(Sha256::digest(&message).as_slice(), word(&digest));
    // Test-only ECDSA key d=1 and nonce k=1: Q=G, r=G.x, s=z+r mod n.
    let s = (&digest + &gx) % n;
    (
        Sha256EcdsaStatement {
            log_compressions: 3,
            qx: word(&gx),
            qy: word(&gy),
            r: word(&gx),
            s: word(&s),
        },
        message,
    )
}

#[test]
fn compact_map_matches_generated_witness_and_exact_constraints() {
    let prepared = prepare_sha256_ecdsa(3, 100, OuterMode::Split).unwrap();
    let (statement, message) = fixture();
    let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message).unwrap();
    assert_eq!(prepared.local.a.len(), 7061);
    let mut mapped = vec![false; prepared.map.rows()];
    let mut nnz = 0;
    for c in 0..prepared.live_source_bits() {
        let row = c & (prepared.p_f.rows() - 1);
        let bit = witness.f_rows[c >> prepared.p_f.t][row / 64] >> (row % 64) & 1 != 0;
        for r in prepared.map.column_rows(c).unwrap() {
            mapped[r] ^= bit;
            nnz += 1;
        }
    }
    assert_eq!(nnz, prepared.map.nnz());
    for (i, bit) in mapped.into_iter().enumerate() {
        assert_eq!(bit, witness.h_bit(i, &prepared.p_h) != 0, "virtual bit {i}");
    }
    let integer = |v: &circuit::matrix_products::StoredInteger| {
        BigInt::from_signed_bytes_le(
            &v.words()
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>(),
        )
    };
    for (i, ((a, b), c)) in witness
        .products
        .a_mw
        .iter()
        .zip(&witness.products.b_mw)
        .zip(&witness.products.c_mw)
        .enumerate()
    {
        assert_eq!(integer(a) * integer(b), integer(c), "P-256 row {i}");
    }
    for bit in 0..1024 {
        assert_eq!(
            witness.h_bit(
                prepared.map.h_offset + prepared.local.public_h[bit],
                &prepared.p_h
            ) != 0,
            statement.bit(bit)
        );
    }
}

#[test]
fn rejects_wrong_message_length_and_zero_scalar() {
    let p = prepare_sha256_ecdsa(3, 100, OuterMode::Split).unwrap();
    let (mut statement, message) = fixture();
    assert!(generate_sha256_ecdsa_witness(&p, &statement, &message[..447]).is_err());
    statement.r = [0; 32];
    assert!(generate_sha256_ecdsa_witness(&p, &statement, &message).is_err());
}

#[test]
fn rejects_invalid_signature_even_with_matching_statement_and_commitment() {
    use crate::transcript::Blake3Transcript;
    let p = prepare_sha256_ecdsa(3, 100, OuterMode::Split).unwrap();
    let (mut statement, message) = fixture();
    statement.s[31] ^= 1;
    let witness = generate_sha256_ecdsa_witness(&p, &statement, &message).unwrap();
    let hint = commit_sha256_ecdsa(&p, &witness).unwrap();
    let result = prove_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &p,
        &statement,
        &witness,
        &hint,
        4,
    );
    if let Ok(proof) = result {
        assert!(
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &p,
                &statement,
                &hint.commitment,
                &proof,
            )
            .is_err()
        );
    }
}

#[test]
fn split_and_all_rows_prove_verify_and_reject_tampering() {
    use crate::transcript::Blake3Transcript;
    use crate::transcript::traits::Transcript;
    let (statement, message) = fixture();
    for mode in [OuterMode::Split, OuterMode::AllRows] {
        let p = prepare_sha256_ecdsa(3, 100, mode).unwrap();
        let witness = generate_sha256_ecdsa_witness(&p, &statement, &message).unwrap();
        let hint = commit_sha256_ecdsa(&p, &witness).unwrap();
        let mut prover_transcript = Blake3Transcript::new();
        let mut verifier_transcript = Blake3Transcript::new();
        let proof =
            prove_sha256_ecdsa(&mut prover_transcript, &p, &statement, &witness, &hint, 4).unwrap();
        let bytes = proof.to_bytes();
        let proof = Sha256EcdsaProof::from_bytes(&bytes).unwrap();
        assert_eq!(proof.to_bytes(), bytes);
        assert!(Sha256EcdsaProof::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        let mut appended = bytes.clone();
        appended.push(0);
        assert!(Sha256EcdsaProof::from_bytes(&appended).is_err());
        verify_sha256_ecdsa(
            &mut verifier_transcript,
            &p,
            &statement,
            &hint.commitment,
            &proof,
        )
        .unwrap();
        assert_eq!(
            prover_transcript.get_challenge::<u128>(),
            verifier_transcript.get_challenge::<u128>()
        );
        let mut changed = statement.clone();
        changed.s[31] ^= 1;
        assert!(
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &p,
                &changed,
                &hint.commitment,
                &proof
            )
            .is_err()
        );
        let other = prepare_sha256_ecdsa(
            3,
            100,
            if mode == OuterMode::Split {
                OuterMode::AllRows
            } else {
                OuterMode::Split
            },
        )
        .unwrap();
        assert!(
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &other,
                &statement,
                &hint.commitment,
                &proof
            )
            .is_err()
        );
    }
}

#[test]
fn security_profiles_cover_both_targets_for_all_shapes() {
    for exponent in 3..=16 {
        for target in [100, 128] {
            for mode in [OuterMode::Split, OuterMode::AllRows] {
                let p = prepare_sha256_ecdsa(exponent, target, mode).unwrap();
                let security = p.security().unwrap();
                assert!(security.economic_bits() >= f64::from(target));
                assert!(security.statistical_bits() < security.economic_bits());
                assert!(security.blocks.iter().all(|b| b.grinding_bits <= 32));
            }
        }
    }
}

#[test]
fn strengthened_128_profile_proves_and_rejects_extra_nonce() {
    use crate::transcript::Blake3Transcript;
    let (statement, message) = fixture();
    let p = prepare_sha256_ecdsa(3, 128, OuterMode::Split).unwrap();
    let witness = generate_sha256_ecdsa_witness(&p, &statement, &message).unwrap();
    let hint = commit_sha256_ecdsa(&p, &witness).unwrap();
    let proof = prove_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &p,
        &statement,
        &witness,
        &hint,
        4,
    )
    .unwrap();
    let mut proof = Sha256EcdsaProof::from_bytes(&proof.to_bytes()).unwrap();
    verify_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &p,
        &statement,
        &hint.commitment,
        &proof,
    )
    .unwrap();
    proof.flock_nonces.push(0);
    assert!(
        verify_sha256_ecdsa(
            &mut Blake3Transcript::new(),
            &p,
            &statement,
            &hint.commitment,
            &proof
        )
        .is_err()
    );
}

#[test]
fn rejects_a_valid_sha_trace_joined_to_an_unrelated_valid_signature_trace() {
    use crate::{pcs::IntEvalParams, transcript::Blake3Transcript};
    let p = prepare_sha256_ecdsa(3, 100, OuterMode::Split).unwrap();
    let (statement, mut message) = fixture();
    let mut witness = generate_sha256_ecdsa_witness(&p, &statement, &message).unwrap();
    message[0] ^= 1;
    let different = generate_sha256_ecdsa_witness(&p, &statement, &message).unwrap();
    let copy_prefix =
        |dst: &mut [Vec<u64>], src: &[Vec<u64>], params: &IntEvalParams, len: usize| {
            for i in 0..len {
                let row = i & (params.rows() - 1);
                let column = i >> params.t;
                let mask = 1u64 << (row % 64);
                dst[column][row / 64] =
                    (dst[column][row / 64] & !mask) | (src[column][row / 64] & mask);
            }
        };
    // Both traces separately satisfy their rows. Only the virtual-map digest
    // alias connects the changed SHA trace to the original P-256 trace.
    copy_prefix(
        &mut witness.f_rows,
        &different.f_rows,
        &p.p_f,
        p.map.f_offset,
    );
    copy_prefix(
        &mut witness.h_rows,
        &different.h_rows,
        &p.p_h,
        p.map.h_offset,
    );
    let hint = commit_sha256_ecdsa(&p, &witness).unwrap();
    let result = prove_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &p,
        &statement,
        &witness,
        &hint,
        4,
    );
    if let Ok(proof) = result {
        assert!(
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &p,
                &statement,
                &hint.commitment,
                &proof
            )
            .is_err()
        );
    }
}
