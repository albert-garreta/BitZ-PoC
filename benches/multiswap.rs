//! End-to-end benchmark of Limber's MultiSwap Mod-R1CS through F2Z.
//!
//! The circuit is the wired RSA-accumulator instance ported from
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
//! Output follows the unified schema (`docs/bench-schema.md`): the
//! end-to-end prover includes bit-packing, commitment, both prime draws,
//! Spartan, bitification, Step 5.0, and the F2Z opening; witness synthesis
//! and relation preparation are excluded and reported one-time.
//!
//! Run single-threaded for numbers comparable to Limber's published table:
//!
//! ```text
//! RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" \
//!   cargo bench --bench multiswap --features unchecked
//! ```
//!
//! `F2Z_BENCH_REPS` selects the measured repetitions (default 5, plus one
//! untimed warmup; `F2Z_MULTISWAP_REPS` is a deprecated alias).
//! `F2Z_BENCH_SHAPES` selects the Limber `k` parameter (default `0`).
//! Every measured proof is verified.

mod common;

use std::hint::black_box;
use std::time::Instant;

use f2z::piop::spartan::multiswap::{
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs, MultiswapAssignment, MultiswapCircuit, MultiswapDims,
    MULTISWAP_VALUE_BITS, PreparedMultiswapRelation,
};
use f2z::transcript::Blake3Transcript;

fn main() {
    let threads = common::init();
    let reps = common::reps(Some("F2Z_MULTISWAP_REPS"), 5);
    let k = common::shapes(None).map_or(0, |shapes| {
        assert_eq!(shapes.len(), 1, "the MultiSwap bench takes one k shape");
        shapes[0]
            .parse::<usize>()
            .expect("F2Z_BENCH_SHAPES must be the Limber k parameter")
    });

    // Witness generation (excluded from prove): build the wired circuit,
    // check the integer relation, and materialize the assignment tables.
    let witness_started = Instant::now();
    let circuit = MultiswapCircuit::build(MultiswapDims::multiswap(k)).expect("build circuit");
    circuit.is_sat_integer().expect("integer relation satisfied");
    let assignment = MultiswapAssignment::new(&circuit).expect("build assignment");
    let witness_ms = common::elapsed_ms(witness_started);

    // One-time public preprocessing (excluded from prove).
    let setup_started = Instant::now();
    let prepared = PreparedMultiswapRelation::new(&circuit).expect("prepare relation");
    let (pc, vc) = multiswap_lig_configs(prepared.params()).expect("Ligerito configs");
    let setup_ms = common::elapsed_ms(setup_started);

    let p = *prepared.params();
    let layout = *prepared.layout();
    println!(
        "MultiSwap through F2Z (Limber k={k} circuit): {} live rows, {} live columns, \
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

    let mut prover = common::StepSamples::default();
    let mut verifier = common::StepSamples::default();
    let mut last_proof = None;

    for rep in 0..reps + 1 {
        let _ = f2z::utils::prof::take_totals();

        // End-to-end prove: bit-pack + commit (Step 1) + the proof.
        let prove_started = Instant::now();
        let commit_started = Instant::now();
        let rows = assignment.f2z_bit_rows();
        let hint = commit_multiswap_witness(prepared.params(), rows, &pc).expect("commit");
        let commit_ms = common::elapsed_ms(commit_started);
        let _ = f2z::utils::prof::take_totals();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        )
        .expect("prove");
        let prove_ms = common::elapsed_ms(prove_started);
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
        let verify_ms = common::elapsed_ms(verify_started);
        let verify_phases = f2z::utils::prof::take_totals();

        black_box(&proof);
        if rep == 0 {
            continue; // warmup
        }
        prover.record_prove(prove_ms, commit_ms, &prove_phases);
        verifier.record_verify(verify_ms, &verify_phases);
        last_proof = Some(proof);
    }

    let proof = last_proof.expect("at least one measured repetition");
    let f2z_bytes = proof.f2z().to_bytes().len();
    let spartan_elements = proof.spartan_payload_elements();
    let piop_bytes = spartan_elements * 16 + proof.mu_prime_bytes() + 8;

    let report = common::BenchReport {
        bench: "multiswap",
        shape: format!("k{k}"),
        extra: vec![
            ("rows".into(), circuit.live_rows().to_string()),
            ("committed_bits".into(), (1usize << (p.t + p.s)).to_string()),
        ],
        lambda: Some(114),
        threads,
        reps,
        seed: None,
        witness_ms,
        setup_ms,
        prover: prover.medians(),
        verifier: verifier.medians(),
        proof: common::ProofBytes {
            piop: piop_bytes,
            open: f2z_bytes,
        },
    };
    report.print_human();
}
