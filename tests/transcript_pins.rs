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
    commit_sha256_compression_witness, generate_sha256_compression_witnesses,
    prepare_sha256_compression_batch_with_profile, prove_sha256_compressions,
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
/// row/column packing and rank-one public-batching migration.
const SHA256_2P7_LAMBDA100_DIGEST: &str =
    "22f1e151f990f7a5538358f7a8d898033a75b4ab76d1802729711b6b5c8df79e";

/// The 2^7 SHA-256 batch under the explicit historical comparison schedule.
/// This pins the grinded quadratic schedule within the packed flat-linear
/// protocol.
const SHA256_2P7_REFERENCE_DIGEST: &str =
    "a4a9e756ecbd63826216e57c68def151047ec063e47a68eae916913b30254c8f";

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
