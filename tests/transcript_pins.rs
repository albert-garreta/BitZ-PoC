//! Golden transcript pins for the protocol proof streams.
//!
//! Each pin proves a fixed deterministic instance under a named security
//! profile and asserts the BLAKE3 digest of every
//! transcript-visible proof component. A changed digest means the
//! transcript moved: update a pin only in a commit that *intends* a
//! transcript change, and say so.
//!
//! The SHA pins below intentionally identify the canonical runtime-prime
//! protocol version and its profile binding.

use blake3::Hasher;
use f2z::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, PreparedMultiswapRelation,
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs,
};
use f2z::piop::spartan::{
    IopSecurityProfile, Lambda100, Sha128ReferenceSchedule, Sha256CompressionStatement,
    commit_sha256_chain_witness, commit_sha256_compression_witness,
    generate_sha256_chain_witnesses, generate_sha256_compression_witnesses,
    prepare_sha256_chain_batch_with_profile, prepare_sha256_compression_batch_with_profile,
    prove_sha256_chain, prove_sha256_compressions, verify_sha256_chain,
    verify_sha256_compressions,
};
use f2z::piop::spartan::{
    PreparedU32MulRelation, U32MulF2zWidth, U32MulWitness, commit_u32_mul_witness, prove_u32_mul,
    verify_u32_mul,
};
use f2z::transcript::Blake3Transcript;

/// The mini MultiSwap instance under the pinned `Limber114` profile.
const MULTISWAP_MINI_DIGEST: &str =
    "9519afc76f9b5dbaab89942df8673e7a4542d9ee297acb187d3ff439148078b5";

/// The 2^7 SHA-256 batch under the DEFAULT profile (`Lambda100`: no
/// grinding anywhere, Ligerito at 100). Recorded at the deliberate native
/// row/column packing and direct product-opening migration.
const SHA256_2P7_LAMBDA100_DIGEST: &str =
    "2fbfcf08fa4a99441e0a2b2898e1073e8729c1c68d3e9f50574bf2934335f2a8";

/// The 2^7 SHA-256 batch under the explicit historical comparison schedule.
/// This pins the grinded boundary schedule within the direct product-opening
/// protocol.
const SHA256_2P7_REFERENCE_DIGEST: &str =
    "93a359989d75f95c4eb3472b8b742a070dcdb8a6b15d172186d70316775ce5ad";

/// The 2^7 CHAINED SHA-256 batch (the Merkle–Damgård chain from the
/// standard initial state, intermediate states as witness) under the
/// default `Lambda100` profile, on the chained virtual map.
const SHA256_CHAIN_2P7_LAMBDA100_DIGEST: &str =
    "e97e985ccc2c424a64d67ed60a2339604b0cdd2de47809d36902048bc7ffc924";

/// The 2^15 u32-multiplication batch under the canonical runtime-prime,
/// K=3 univariate-skip protocol and its default `Lambda100` profile.
const U32_MUL_2P15_DIGEST: &str =
    "1838bee571c691584bb4f4524ec9d760b73ec2df0b16e55dd669a76bdb8e8bf1";

#[test]
fn u32_mul_2p15_transcript_is_pinned() {
    let witness = U32MulWitness::from_fn_with_f2z_width(1usize << 15, U32MulF2zWidth::W1, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness");
    let layout = *witness.layout();
    let prepared = PreparedU32MulRelation::new(layout).expect("prepare");
    let hint = commit_u32_mul_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_u32_mul(&mut prover_transcript, &prepared, &witness, &hint).expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_u32_mul(
        &mut verifier_transcript,
        &prepared,
        &hint.commitment,
        &proof,
    )
    .expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let spartan = format!("{:?}", proof.spartan());
    let digest = digest_hex(&[&hint.commitment.root, &f2z_bytes, spartan.as_bytes()]);
    assert_eq!(digest, U32_MUL_2P15_DIGEST);
}

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
    verify_multiswap_mod_r1cs(
        &mut verifier_transcript,
        &prepared,
        &hint.commitment,
        &proof,
        &vc,
    )
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
    assert_eq!(
        sha256_2p7_digest::<Lambda100>(),
        SHA256_2P7_LAMBDA100_DIGEST
    );
}

#[test]
fn sha256_2p7_reference_schedule_transcript_is_pinned() {
    assert_eq!(
        sha256_2p7_digest::<Sha128ReferenceSchedule>(),
        SHA256_2P7_REFERENCE_DIGEST
    );
}

#[test]
fn sha256_chain_2p7_default_lambda100_transcript_is_pinned() {
    assert_eq!(
        sha256_chain_2p7_digest::<Lambda100>(),
        SHA256_CHAIN_2P7_LAMBDA100_DIGEST
    );
}

fn sha256_chain_2p7_digest<P: IopSecurityProfile>() -> String {
    const EXPONENT: usize = 7;
    let prepared = prepare_sha256_chain_batch_with_profile::<P>(EXPONENT).expect("prepare");
    let blocks: Vec<[u32; 16]> = (0..1usize << EXPONENT)
        .map(|i| std::array::from_fn(|j| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32)))
        .collect();
    let witness = generate_sha256_chain_witnesses(&prepared, &blocks).expect("witness");
    let statement = witness.statement();
    let hint = commit_sha256_chain_witness(&prepared, &witness).expect("commit");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_sha256_chain(&mut prover_transcript, &prepared, &statement, &witness, &hint)
        .expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_sha256_chain(
        &mut verifier_transcript,
        &prepared,
        &statement,
        &hint.commitment,
        &proof,
    )
    .expect("verify");

    let f2z_bytes = proof.f2z().to_bytes();
    let nonces: Vec<u8> = proof
        .initial_nonce()
        .to_le_bytes()
        .into_iter()
        .chain(proof.terminal_nonce().to_le_bytes())
        .collect();
    let digest_words: Vec<u8> = statement
        .digest
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    digest_hex(&[&hint.commitment.root, &f2z_bytes, &nonces, &digest_words])
}

fn sha256_2p7_digest<P: IopSecurityProfile>() -> String {
    const EXPONENT: usize = 7;
    let prepared = prepare_sha256_compression_batch_with_profile::<P>(EXPONENT).expect("prepare");
    let inputs: Vec<_> = (0..1usize << EXPONENT)
        .map(|i| {
            let word = |j: usize| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32);
            (
                std::array::from_fn(|j| word(j)),
                std::array::from_fn(|j| word(j + 16)),
            )
        })
        .collect();
    let witness = generate_sha256_compression_witnesses(&prepared, &inputs).expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let hint = commit_sha256_compression_witness(&prepared, &witness).expect("commit");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_sha256_compressions(
        &mut prover_transcript,
        &prepared,
        &statements,
        &witness,
        &hint,
    )
    .expect("prove");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_sha256_compressions(
        &mut verifier_transcript,
        &prepared,
        &statements,
        &hint.commitment,
        &proof,
    )
    .expect("verify");

    let f2z_bytes = proof.f2z().to_bytes();
    let inner = format!("{:?}", proof.inner());
    let nonces: Vec<u8> = proof
        .inner_nonces()
        .iter()
        .flat_map(|nonce| nonce.to_le_bytes())
        .chain(proof.initial_nonce().to_le_bytes())
        .chain(proof.terminal_nonce().to_le_bytes())
        .collect();
    digest_hex(&[&hint.commitment.root, &f2z_bytes, inner.as_bytes(), &nonces])
}
