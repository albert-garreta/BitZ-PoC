//! End-to-end benchmark of Limber's MultiSwap Mod-R1CS through F2Z.
//!
//! The circuit is the wired 6209-row RSA-accumulator instance ported from
//! `lucasxia01/limber-impl@benches/multiswap_modp.rs` (`k = 0`, the only
//! configuration Limber's authors mark quotable): four real square-and-
//! multiply chains with 352-bit exponents mod RSA-2048, wired Pocklington
//! hash-to-prime chains, chained Poseidon-cost rows, and full bit
//! decomposition/reconstruction.  F2Z commits the witness and quotient
//! values as raw bits (`2^25` committed bits), Spartan proves the relation
//! over a transcript-sampled 128-bit fingerprint prime, and the terminal
//! assignment claim is reduced by Step 5.0 (exact integer lift plus a
//! grinded fresh 113-bit prime) and opened against the bit commitment.
//!
//! Run single-threaded for numbers comparable to Limber's published table:
//!
//! ```text
//! RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" \
//!   cargo bench --bench multiswap --features unchecked
//! ```
//!
//! `F2Z_MULTISWAP_REPS` selects the measured repetitions (default 5, plus
//! one untimed warmup); every measured proof is verified.

use std::hint::black_box;
use std::time::Instant;

use f2z::piop::spartan::multiswap::{
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs, MultiswapAssignment, MultiswapCircuit, MultiswapDims,
    MultiswapProof, PreparedMultiswapRelation, MULTISWAP_VALUE_BITS,
};
use f2z::transcript::Blake3Transcript;

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    sorted[sorted.len() / 2]
}

fn phase_ms(phases: &[(&'static str, f64)], label: &str) -> f64 {
    phases
        .iter()
        .filter(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
        .sum()
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn main() {
    // Phase attribution uses the crate's scope profiler.
    // SAFETY: set before any other thread can read the environment.
    unsafe { std::env::set_var("OBLONG_PROFILE", "1") };

    let reps = env_usize("F2Z_MULTISWAP_REPS", 5);
    let threads = std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".into());

    let build_started = Instant::now();
    let circuit = MultiswapCircuit::build(MultiswapDims::multiswap(0)).expect("build circuit");
    let witness_ms = build_started.elapsed().as_secs_f64() * 1e3;
    circuit.is_sat_integer().expect("integer relation satisfied");

    let setup_started = Instant::now();
    let prepared = PreparedMultiswapRelation::new(&circuit).expect("prepare relation");
    let assignment = MultiswapAssignment::new(&circuit).expect("build assignment");
    let (pc, vc) = multiswap_lig_configs(prepared.params()).expect("Ligerito configs");
    let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;

    let p = *prepared.params();
    let layout = *prepared.layout();
    println!(
        "MultiSwap through F2Z (Limber k=0 circuit): {} live rows, {} live columns, \
         nnz {} (mods folded into C), capacity 2^{}",
        circuit.live_rows(),
        circuit.dims().num_real_cols(),
        prepared.relation().nnz(),
        layout.gate_vars(),
    );
    println!(
        "  committed bits: 2^{} ({} B) = 2 blocks x 2^{} gates x {} bits | \
         f2z t={} s={} W={} | fingerprint Q in [2^127, 2^128), step5.0 q' in [2^112, 2^113) | \
         threads={threads} reps={reps}",
        p.t + p.s,
        (1usize << (p.t + p.s)) / 8,
        layout.gate_vars(),
        MULTISWAP_VALUE_BITS,
        p.t,
        p.s,
        p.word_bits,
    );

    let mut bitify_samples = Vec::new();
    let mut commit_samples = Vec::new();
    let mut prove_samples = Vec::new();
    let mut verify_samples = Vec::new();
    let mut last: Option<(MultiswapProof, Vec<(&'static str, f64)>, Vec<(&'static str, f64)>)> =
        None;

    for rep in 0..reps + 1 {
        let _ = f2z::utils::prof::take_totals();

        let bitify_started = Instant::now();
        let rows = assignment.f2z_bit_rows();
        let bitify_ms = bitify_started.elapsed().as_secs_f64() * 1e3;

        let commit_started = Instant::now();
        let hint = commit_multiswap_witness(prepared.params(), rows, &pc).expect("commit");
        let commit_ms = commit_started.elapsed().as_secs_f64() * 1e3;
        let _ = f2z::utils::prof::take_totals();

        let prove_started = Instant::now();
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        )
        .expect("prove");
        let prove_ms = prove_started.elapsed().as_secs_f64() * 1e3;
        let prove_phases = f2z::utils::prof::take_totals();

        let verify_started = Instant::now();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_multiswap_mod_r1cs(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
            &vc,
        )
        .expect("verify");
        let verify_ms = verify_started.elapsed().as_secs_f64() * 1e3;
        let verify_phases = f2z::utils::prof::take_totals();

        black_box(&proof);
        if rep == 0 {
            continue; // warmup
        }
        bitify_samples.push(bitify_ms);
        commit_samples.push(commit_ms);
        prove_samples.push(prove_ms);
        verify_samples.push(verify_ms);
        last = Some((proof, prove_phases, verify_phases));
    }

    let (proof, prove_phases, verify_phases) = last.expect("at least one measured repetition");
    let f2z_bytes = proof.f2z().to_bytes().len();
    let spartan_elements = proof.spartan_payload_elements();
    let spartan_bytes = spartan_elements * 16 + proof.mu_prime_bytes() + 8;
    let proof_bytes = f2z_bytes + spartan_bytes;

    let bitify_median = median(&bitify_samples);
    let commit_median = median(&commit_samples);
    let prove_median = median(&prove_samples);
    let verify_median = median(&verify_samples);

    println!(
        "  witness+shape build {witness_ms:.1} ms (one-time) | relation setup {setup_ms:.1} ms \
         (one-time)"
    );
    println!(
        "  bit rows {bitify_median:.1} ms | commit {commit_median:.1} ms | prove \
         {prove_median:.1} ms | verify {verify_median:.1} ms | commit+prove {:.1} ms",
        commit_median + prove_median,
    );
    println!(
        "  prove phases: primes {:.1} ms | projection {:.1} ms | spartan {:.1} ms | bitify \
         {:.1} ms | step5.0 lift+grind {:.1} ms | f2z opening {:.1} ms",
        phase_ms(&prove_phases, "multiswap:fingerprint_prime_prove")
            + phase_ms(&prove_phases, "multiswap:reduction_prime_prove"),
        phase_ms(&prove_phases, "multiswap:relation_projection_prove")
            + phase_ms(&prove_phases, "multiswap:witness_projection_prove"),
        phase_ms(&prove_phases, "multiswap:spartan_prove"),
        phase_ms(&prove_phases, "multiswap:bitify_prove"),
        phase_ms(&prove_phases, "multiswap:integer_lift_prove")
            + phase_ms(&prove_phases, "multiswap:reduction_grinding_prove"),
        phase_ms(&prove_phases, "multiswap:f2z_prove"),
    );
    println!(
        "  verify phases: primes {:.1} ms | projection {:.1} ms | spartan {:.1} ms | bitify \
         {:.1} ms | f2z opening {:.1} ms",
        phase_ms(&verify_phases, "multiswap:fingerprint_prime_verify")
            + phase_ms(&verify_phases, "multiswap:reduction_prime_verify"),
        phase_ms(&verify_phases, "multiswap:relation_projection_verify"),
        phase_ms(&verify_phases, "multiswap:spartan_verify"),
        phase_ms(&verify_phases, "multiswap:bitify_verify"),
        phase_ms(&verify_phases, "multiswap:f2z_verify"),
    );
    println!(
        "  proof: {proof_bytes} B total ({:.1} KB) = f2z {f2z_bytes} B + spartan \
         {spartan_elements} elements / {spartan_bytes} B",
        proof_bytes as f64 / 1e3,
    );
    println!(
        "  RESULT circuit=multiswap_k0 rows={} committed_bits={} threads={threads} \
         witness_ms={witness_ms:.3} setup_ms={setup_ms:.3} bitify_ms={bitify_median:.3} \
         commit_ms={commit_median:.3} prove_ms={prove_median:.3} \
         commit_prove_ms={:.3} verify_ms={verify_median:.3} proof_bytes={proof_bytes} \
         f2z_bytes={f2z_bytes} spartan_bytes={spartan_bytes} verified_samples={reps}",
        circuit.live_rows(),
        1usize << (p.t + p.s),
        commit_median + prove_median,
    );
}
