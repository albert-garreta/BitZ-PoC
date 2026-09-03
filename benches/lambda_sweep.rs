//! The λ sweep: one binary proving the SAME SHA-256 witness under each
//! named security profile — `Lambda100` (the default), `Sha128ReferenceSchedule`
//! (the historical 128-design PIOP schedule, no forest grinding, ~126.4-bit
//! floor), and `Lambda128` (every controllable term ≥ 128) — in the unified
//! output format. This is the prover-time / proof-size tradeoff table that
//! shows what 128 actually costs.
//!
//! Profiles are compile-time types; the sweep instantiates all three in one
//! binary (a generic body over the profile). `F2Z_BENCH_LAMBDA=100|128|
//! sha128-reference-schedule` restricts a run to ONE of them — the same
//! binary, one monomorphized body — for a single-profile measurement. The
//! `Limber114` profile is MultiSwap-only (Strategy 2) — its row comes from
//! `cargo bench --bench multiswap`, which pins it.
//!
//! ```text
//! F2Z_BENCH_SHAPES=12 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
//!   cargo bench --bench lambda_sweep --features unchecked
//! ```
//!
//! One exponent per run (default `2^12`); reps via `F2Z_BENCH_REPS`
//! (default 3, plus one warm-up per profile). Every measured proof is
//! verified.

mod common;

use std::hint::black_box;
use std::time::Instant;

use f2z::piop::spartan::{
    IopSecurityProfile, Lambda100, Lambda128, PrimePolicy, SHA256_MAX_LOG_COMPRESSIONS,
    SHA256_MIN_LOG_COMPRESSIONS, Sha128ReferenceSchedule, Sha256CompressionInput,
    Sha256CompressionStatement, SpartanField, commit_sha256_compression_witness_with_config,
    generate_sha256_compression_witnesses, prepare_sha256_compression_batch_with_profile,
    prove_sha256_compressions_with_config, sha256_compression_configs,
    verify_sha256_compressions_with_config,
};
use f2z::transcript::Blake3Transcript;

fn make_inputs(compressions: usize, seed: u64) -> Vec<Sha256CompressionInput> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (value ^ (value >> 31)) as u32
    };
    (0..compressions)
        .map(|_| {
            (
                std::array::from_fn(|_| next()),
                std::array::from_fn(|_| next()),
            )
        })
        .collect()
}

fn sweep_profile<P: IopSecurityProfile>(
    exponent: usize,
    inputs: &[Sha256CompressionInput],
    reps: usize,
    threads: usize,
    seed: u64,
) {
    let compressions = 1usize << exponent;
    let setup_started = Instant::now();
    let prepared = prepare_sha256_compression_batch_with_profile::<P>(exponent)
        .expect("profile instantiates at this shape");
    let (pc, vc) = sha256_compression_configs(&prepared).expect("Ligerito configs");
    let setup_ms = common::elapsed_ms(setup_started);
    let security = prepared.security().clone();

    println!();
    println!(
        "--- profile {} (λ={}) at 2^{exponent} = {compressions} compressions ---",
        P::NAME,
        security.lambda
    );
    println!(
        "  grinding: initial {} | outer/round {} | terminal {} | forest/round {} | \
         ring-switch {} | ligerito target {}",
        security.initial_grinding_bits,
        security.piop_round_grinding_bits,
        security.terminal_grinding_bits,
        security.forest_round_grinding_bits,
        security.ring_switch_grinding_bits,
        security.ligerito_target_bits,
    );
    for term in &security.accounting.terms {
        println!(
            "    term {:32} {:>7.1} bits{}{}",
            term.name,
            term.bits,
            if term.grinding_bits > 0 {
                format!("  (+{} grind)", term.grinding_bits)
            } else {
                String::new()
            },
            if term.floor { "  [floor]" } else { "" },
        );
    }

    let witness_started = Instant::now();
    let witness = generate_sha256_compression_witnesses(&prepared, inputs).expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let witness_ms = common::elapsed_ms(witness_started);

    let mut prover = common::StepSamples::default();
    let mut verifier = common::StepSamples::default();
    let mut last = None;
    for rep in 0..reps + 1 {
        let _ = f2z::utils::prof::take_totals();
        let prove_started = Instant::now();
        let commit_started = Instant::now();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc)
            .expect("commit");
        let commit_ms = common::elapsed_ms(commit_started);
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_with_config(
            &mut prover_transcript,
            &prepared,
            &statements,
            &witness,
            &hint,
            &pc,
        )
        .expect("prove");
        let prove_ms = common::elapsed_ms(prove_started);
        let prove_phases = f2z::utils::prof::take_totals();

        let verify_started = Instant::now();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_sha256_compressions_with_config(
            &mut verifier_transcript,
            &prepared,
            &statements,
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
        last = Some(proof);
    }
    let proof = last.expect("at least one measured repetition");

    let f2z_bytes = proof.f2z().to_bytes().len();
    let spartan_elements = 3 * proof.inner().round_polynomials.len();
    let field_bytes = proof
        .inner()
        .round_polynomials
        .first()
        .map_or(0, |round| round[0].canonical_element_encoding().len());
    let grinding_nonce_count = proof.inner_nonces().len()
        + usize::from(security.initial_grinding_bits > 0)
        + usize::from(security.terminal_grinding_bits > 0);
    let spartan_bytes = spartan_elements * field_bytes + 8 * grinding_nonce_count;

    let report = common::BenchReport {
        bench: "sha256",
        shape: format!("2p{exponent}"),
        extra: vec![
            ("profile".into(), P::NAME.into()),
            ("compressions".into(), compressions.to_string()),
            (
                "forest_grinding_nonces".into(),
                proof.f2z().grinding_nonces.len().to_string(),
            ),
        ],
        lambda: Some(security.lambda),
        lambda_achieved: Some(security.accounting.achieved_bits()),
        lambda_bind: Some(security.accounting.binding_term().name.into()),
        threads,
        reps,
        seed: Some(seed),
        witness_ms,
        setup_ms,
        prover: prover.medians(),
        verifier: verifier.medians(),
        proof: common::ProofBytes {
            piop: spartan_bytes,
            open: f2z_bytes,
        },
    };
    report.print_human();
}

fn main() {
    let threads = common::init();
    let reps = common::reps(None, 3);
    let seed = common::seed(None, 0x4632_5a5f_5357_4550);
    let exponent = common::shapes(None).map_or(12, |shapes| {
        assert_eq!(shapes.len(), 1, "the λ sweep takes one exponent per run");
        let exponent: usize = shapes[0].parse().expect("integer exponent");
        assert!(
            (SHA256_MIN_LOG_COMPRESSIONS..=SHA256_MAX_LOG_COMPRESSIONS).contains(&exponent),
            "SHA-256 runtime-prime protocol supports exponents 7 through 16"
        );
        exponent
    });
    let inputs = make_inputs(1usize << exponent, seed);
    let selected = common::security_profile(PrimePolicy::SingleDerived);

    match selected {
        None => {
            println!(
                "λ sweep: one SHA-256 witness (2^{exponent} compressions), three compile-time \
                 profiles; threads={threads} reps={reps}"
            );
            println!(
                "(the Limber114 row comes from `cargo bench --bench multiswap`; \
                 F2Z_BENCH_LAMBDA restricts the sweep to one profile)"
            );
            sweep_profile::<Lambda100>(exponent, &inputs, reps, threads, seed);
            sweep_profile::<Sha128ReferenceSchedule>(exponent, &inputs, reps, threads, seed);
            sweep_profile::<Lambda128>(exponent, &inputs, reps, threads, seed);
        }
        Some(profile) => {
            println!(
                "λ sweep: one SHA-256 witness (2^{exponent} compressions), one compile-time \
                 profile: {} (λ={}, F2Z_BENCH_LAMBDA={}); threads={threads} reps={reps}",
                profile.name(),
                profile.lambda(),
                profile.knob_value(),
            );
            common::with_profile!(
                profile,
                sweep_profile(exponent, &inputs, reps, threads, seed)
            );
        }
    }
    flock_core::scratch::clear();
}
