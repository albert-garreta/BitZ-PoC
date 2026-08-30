//! End-to-end coverage of the B.6 forest/opening grinding hooks: at
//! `Lambda128` every challenge drawn in the F2Z opening region carries a
//! two-bit proof-of-work boundary, the nonces ride the proof stream, and
//! tampering with any of them (or their count) is rejected.

use f2z::ligerito_flock::IntEvalRsLigVirtProof;
use f2z::piop::spartan::{
    commit_sha256_paper128_witness_with_config, generate_sha256_compression_witnesses_exact,
    prepare_sha256_compression_batch_integer,
    prepare_sha256_compression_batch_integer_with_profile,
    prove_sha256_compressions_paper128_with_config, sha256_compression_configs_for,
    verify_sha256_compressions_paper128_with_config, Lambda128, PreparedSha256CompressionBatch,
    Sha256CompressionStatement,
};
use f2z::transcript::Blake3Transcript;

const EXPONENT: usize = 7;

fn prove_under(
    prepared: &PreparedSha256CompressionBatch,
) -> (
    Vec<u8>,
    usize,
    f2z::piop::spartan::Sha256Paper128Proof,
    flock_core::pcs::commit::Commitment,
    flock_core::pcs::ligerito::VerifierConfig,
    Vec<Sha256CompressionStatement>,
) {
    let (pc, vc) = sha256_compression_configs_for(prepared).expect("configs");
    let inputs: Vec<_> = (0..1usize << EXPONENT)
        .map(|i| {
            let word = |j: usize| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32);
            (
                std::array::from_fn(|j| word(j)),
                std::array::from_fn(|j| word(j + 16)),
            )
        })
        .collect();
    let witness = generate_sha256_compression_witnesses_exact(
        &inputs,
        prepared.source_params(),
        prepared.assignment_params(),
    )
    .expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let hint =
        commit_sha256_paper128_witness_with_config(prepared, &witness, &pc).expect("commit");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_sha256_compressions_paper128_with_config(
        &mut prover_transcript,
        prepared,
        &statements,
        &witness,
        &hint,
        &pc,
    )
    .expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_sha256_compressions_paper128_with_config(
        &mut verifier_transcript,
        prepared,
        &statements,
        &hint.commitment,
        &proof,
        &vc,
    )
    .expect("verify");
    let bytes = proof.f2z().to_bytes();
    let nonces = proof.f2z().grinding_nonces.len();
    (bytes, nonces, proof, hint.commitment, vc, statements)
}

#[test]
fn lambda128_grinds_every_opening_round_and_gates_the_nonces() {
    let legacy = prepare_sha256_compression_batch_integer(EXPONENT).expect("legacy prepare");
    let lambda128 =
        prepare_sha256_compression_batch_integer_with_profile::<Lambda128>(EXPONENT)
            .expect("lambda128 prepare");
    assert_eq!(lambda128.security().forest_round_grinding_bits, 2);

    let (legacy_bytes, legacy_nonces, ..) = prove_under(&legacy);
    assert_eq!(legacy_nonces, 0, "the default profile grinds nothing");

    let (bytes, nonce_count, proof, commitment, vc, statements) = prove_under(&lambda128);
    assert!(nonce_count > 0, "λ=128 must grind the opening rounds");
    // The proof grows by the trailing nonce section (8-byte count prefix +
    // 8 bytes per boundary) — grinding costs bytes, not time. The residual
    // difference is Ligerito query-path wiggle: the two runs draw different
    // challenges, so Merkle path deduplication differs by a few hundred
    // bytes in either direction.
    let section = 8 + 8 * nonce_count;
    let delta = bytes.len() as i64 - legacy_bytes.len() as i64;
    assert!(
        (delta - section as i64).unsigned_abs() < 2048,
        "delta {delta} B vs nonce section {section} B ({nonce_count} boundaries)"
    );
    println!(
        "forest grinding at 2^{EXPONENT}: {nonce_count} grinded boundaries, \
         nonce section {section} B, total proof delta {delta} B"
    );

    // Codec round-trip preserves the section byte-for-byte.
    let decoded = IntEvalRsLigVirtProof::from_bytes(&bytes).expect("codec");
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(decoded.grinding_nonces.len(), nonce_count);

    // A tampered nonce is rejected.
    let mut tampered = proof.clone();
    tampered.f2z_mut_for_tests().grinding_nonces[0] ^= 1;
    let mut verifier_transcript = Blake3Transcript::new();
    assert!(verify_sha256_compressions_paper128_with_config(
        &mut verifier_transcript,
        &lambda128,
        &statements,
        &commitment,
        &tampered,
        &vc,
    )
    .is_err());

    // A truncated nonce list is rejected.
    let mut truncated = proof.clone();
    truncated
        .f2z_mut_for_tests()
        .grinding_nonces
        .truncate(nonce_count - 1);
    let mut verifier_transcript = Blake3Transcript::new();
    assert!(verify_sha256_compressions_paper128_with_config(
        &mut verifier_transcript,
        &lambda128,
        &statements,
        &commitment,
        &truncated,
        &vc,
    )
    .is_err());

    // A λ=128 proof does not verify under the legacy (0-difficulty)
    // preparation: the grinding boundaries are transcript-visible.
    let mut verifier_transcript = Blake3Transcript::new();
    assert!(verify_sha256_compressions_paper128_with_config(
        &mut verifier_transcript,
        &legacy,
        &statements,
        &commitment,
        &proof,
        &vc,
    )
    .is_err());
}
