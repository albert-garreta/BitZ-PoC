//! Golden transcript pins for the protocol proof streams.
//!
//! Each pin proves a fixed deterministic instance under the DEFAULT
//! security profile and asserts the BLAKE3 digest of every
//! transcript-visible proof component. A changed digest means the
//! transcript moved: update a pin only in a commit that *intends* a
//! transcript change, and say so.
//!
//! These digests were first recorded before the `IopSecurityProfile`
//! wiring (commit 565e532's tree) and pin that wiring as byte-identical.
//! They are also the λ-default pins: zero-difficulty profiles must leave
//! every one of these bytes untouched.

use blake3::Hasher;
use f2z::piop::spartan::multiswap::{
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs, MultiswapAssignment, MultiswapCircuit, MultiswapDims,
    PreparedMultiswapRelation,
};
use f2z::piop::spartan::{
    commit_sha256_paper128_witness_with_config, generate_sha256_compression_witnesses_exact,
    prepare_sha256_compression_batch_integer_with_profile,
    prove_sha256_compressions_paper128_with_config, sha256_compression_configs_for,
    verify_sha256_compressions_paper128_with_config, IopSecurityProfile, Lambda100,
    LegacySha128Design, Sha256CompressionStatement,
};
use f2z::transcript::Blake3Transcript;

/// The mini MultiSwap instance under the pinned `Limber114` profile.
const MULTISWAP_MINI_DIGEST: &str =
    "9519afc76f9b5dbaab89942df8673e7a4542d9ee297acb187d3ff439148078b5";

/// The 2^7 SHA-256 batch under the DEFAULT profile (`Lambda100`: no
/// grinding anywhere, Ligerito at 100). Recorded at the deliberate
/// λ = 100 default flip.
const SHA256_2P7_LAMBDA100_DIGEST: &str =
    "5126146691c28d8b0506d8d8613cfefbed5c95694494585fa320c758abc5ccd6";

/// The 2^7 SHA-256 batch under `LegacySha128Design` — the historical
/// 128-design schedule. This digest is the ORIGINAL pre-profile stream
/// (recorded at commit 565e532's tree) and must never move: it proves the
/// legacy profile keeps reproducing the published numbers byte for byte.
const SHA256_2P7_LEGACY_DIGEST: &str =
    "65ecfa707b276483a4b4405b8a16d0d50c92339719ab877492792de7088b7032";

fn digest_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

#[test]
fn multiswap_mini_transcript_is_pinned() {
    let circuit = MultiswapCircuit::build(MultiswapDims::mini()).expect("build circuit");
    circuit.is_sat_integer().expect("relation satisfied");
    let prepared = PreparedMultiswapRelation::new(&circuit).expect("prepare");
    let assignment = MultiswapAssignment::new(&circuit).expect("assignment");
    let (pc, vc) = multiswap_lig_configs(prepared.params()).expect("configs");
    let rows = assignment.f2z_bit_rows();
    let hint = commit_multiswap_witness(prepared.params(), rows, &pc).expect("commit");

    let mut prover_transcript = Blake3Transcript::new();
    let proof =
        prove_multiswap_mod_r1cs(&mut prover_transcript, &prepared, &assignment, &hint, &pc)
            .expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_multiswap_mod_r1cs(&mut verifier_transcript, &prepared, &hint.commitment, &proof, &vc)
        .expect("verify");

    let f2z_bytes = proof.f2z().to_bytes();
    let mu_prime = proof.mu_prime().to_bytes_le();
    let nonce = proof.reduction_nonce().to_le_bytes();
    let spartan = format!("{:?}", proof.spartan());
    let digest = digest_hex(&[
        &hint.commitment.root,
        &f2z_bytes,
        &mu_prime,
        &nonce,
        spartan.as_bytes(),
    ]);
    assert_eq!(digest, MULTISWAP_MINI_DIGEST);
}

#[test]
fn sha256_2p7_default_lambda100_transcript_is_pinned() {
    assert_eq!(sha256_2p7_digest::<Lambda100>(), SHA256_2P7_LAMBDA100_DIGEST);
}

#[test]
fn sha256_2p7_legacy_128_design_transcript_is_pinned() {
    assert_eq!(sha256_2p7_digest::<LegacySha128Design>(), SHA256_2P7_LEGACY_DIGEST);
}

fn sha256_2p7_digest<P: IopSecurityProfile>() -> String {
    const EXPONENT: usize = 7;
    let prepared =
        prepare_sha256_compression_batch_integer_with_profile::<P>(EXPONENT).expect("prepare");
    let (pc, vc) = sha256_compression_configs_for(&prepared).expect("configs");
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
        commit_sha256_paper128_witness_with_config(&prepared, &witness, &pc).expect("commit");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_sha256_compressions_paper128_with_config(
        &mut prover_transcript,
        &prepared,
        &statements,
        &witness,
        &hint,
        &pc,
    )
    .expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_sha256_compressions_paper128_with_config(
        &mut verifier_transcript,
        &prepared,
        &statements,
        &hint.commitment,
        &proof,
        &vc,
    )
    .expect("verify");

    let f2z_bytes = proof.f2z().to_bytes();
    let outer = format!("{:?}", proof.outer());
    let nonces: Vec<u8> = proof
        .outer_nonces()
        .iter()
        .flat_map(|nonce| nonce.to_le_bytes())
        .chain(proof.initial_nonce().to_le_bytes())
        .chain(proof.terminal_nonce().to_le_bytes())
        .collect();
    digest_hex(&[
        &hint.commitment.root,
        &f2z_bytes,
        outer.as_bytes(),
        &nonces,
    ])
}
