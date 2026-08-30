//! Byte-identity pin for the PROTOCOL proof streams (MultiSwap and the
//! paper128 SHA-256 path): proves a fixed deterministic instance per path
//! and prints the BLAKE3 digest of the serialized proof body plus the
//! commitment root. Run before and after any change that claims to be
//! transcript-preserving — matching digests mean byte-identical proofs,
//! commitments, and transcripts. The companion `proof_digest` example pins
//! the base PCS opener alone.
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo run --release --example protocol_digest
//! ```

use blake3::Hasher;
use f2z::piop::spartan::multiswap::{
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs, MultiswapAssignment, MultiswapCircuit, MultiswapDims,
    PreparedMultiswapRelation,
};
use f2z::piop::spartan::{
    commit_sha256_paper128_witness_with_config, generate_sha256_compression_witnesses_exact,
    prepare_sha256_compression_batch_integer, prove_sha256_compressions_paper128_with_config,
    sha256_compression_configs, verify_sha256_compressions_paper128_with_config,
    Sha256CompressionStatement,
};
use f2z::transcript::Blake3Transcript;

fn digest_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

fn multiswap_digest() -> String {
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

    // Every transcript-visible proof component, framed.
    let f2z_bytes = proof.f2z().to_bytes();
    let mu_prime = proof.mu_prime().to_bytes_le();
    let nonce = proof.reduction_nonce().to_le_bytes();
    let spartan = format!("{:?}", proof.spartan());
    digest_hex(&[
        &hint.commitment.root,
        &f2z_bytes,
        &mu_prime,
        &nonce,
        spartan.as_bytes(),
    ])
}

fn sha256_digest() -> String {
    const EXPONENT: usize = 7;
    let prepared = prepare_sha256_compression_batch_integer(EXPONENT).expect("prepare");
    let (pc, vc) = sha256_compression_configs(prepared.source_params()).expect("configs");
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
    let hint = commit_sha256_paper128_witness_with_config(&prepared, &witness, &pc)
        .expect("commit");
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

fn main() {
    println!("multiswap-mini  {}", multiswap_digest());
    println!("sha256-2p7      {}", sha256_digest());
}
