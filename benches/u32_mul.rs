//! End-to-end benchmark for a batch of integer `u32 * u32 = u64` constraints.
//!
//! Spartan proves the integer-valued R1CS relation. Its terminal assignment-MLE
//! claim is then discharged against the compact 32/32/64-bit witness with F2Z.
//! Every measured proof is verified through the combined verifier.
//!
//! Output follows the unified schema (`docs/bench-schema.md`): the
//! end-to-end prover (`prove_ms`) covers bit packing, commitment, the
//! transcript-sampled Step-2 prime (commit-before-prime, Zaratan order),
//! Spartan over that runtime field, bitification, and the F2Z opening,
//! re-run per repetition. Witness generation and relation preparation are
//! excluded and reported one-time. `F2Z_BENCH_LAMBDA=100|128|sha128-reference-schedule`
//! selects the security profile (default `Lambda100`; the two-prime
//! `Limber114` profile is MultiSwap-only and is rejected here); the
//! canonical Spartan outer reduction uses the K=3 univariate-prefix skip.
//!
//! Defaults to the production sweep `2^15, ..., 2^25` multiplications.
//! Override with `F2Z_BENCH_SHAPES`:
//!
//! ```text
//! F2Z_BENCH_SHAPES="15 17 19" F2Z_BENCH_REPS=3 \
//!   cargo bench --bench u32_mul --features unchecked
//! ```
//!
//! `F2Z_BENCH_SHAPES=15 F2Z_BENCH_REPS=1` is the smallest production smoke
//! shape. The production reducer is delayed Barrett.
//! `F2Z_MUL_WORD_BITS` selects the F2Z word width; the default is `1`.
//! `F2Z_BENCH_PASS=latency|memory|both` separates the measured repetitions
//! from the extra peak-heap proof. Memory measurement requires the
//! benchmark-only `bench-peak-memory` feature.

use ::f2z::piop::spartan::protocol;
use ::f2z::piop::spartan::protocol::PreparedRelation;
use ::f2z::piop::spartan::protocol::Proof;
use ::f2z::piop::spartan::protocol::ProtocolError;
use f2z::piop::spartan::mul::{MulLayout, MulWitness};

mod common;

use std::hint::black_box;

use f2z::ligerito_flock::FlockCommitHint;
use f2z::piop::spartan::{IopSecurityProfile, PrimePolicy, U32_MUL_UNIVARIATE_SKIP_VARS};
use f2z::transcript::Blake3Transcript;
use rand::{RngExt, SeedableRng, rngs::StdRng};

#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static ALLOCATOR: common::peak_memory::PeakAlloc = common::peak_memory::PeakAlloc;

#[cfg(feature = "bench-peak-memory")]
fn reset_peak() {
    common::peak_memory::reset_peak();
}

#[cfg(feature = "bench-peak-memory")]
fn live_mib() -> f64 {
    common::peak_memory::live_bytes() as f64 / (1024.0 * 1024.0)
}

#[cfg(feature = "bench-peak-memory")]
fn peak_mib() -> f64 {
    common::peak_memory::peak_bytes() as f64 / (1024.0 * 1024.0)
}

#[cfg(not(feature = "bench-peak-memory"))]
fn reset_peak() {
    unreachable!("memory pass requires the bench-peak-memory feature")
}

#[cfg(not(feature = "bench-peak-memory"))]
fn live_mib() -> f64 {
    unreachable!("memory pass requires the bench-peak-memory feature")
}

#[cfg(not(feature = "bench-peak-memory"))]
fn peak_mib() -> f64 {
    unreachable!("memory pass requires the bench-peak-memory feature")
}

use common::cli::BenchmarkPass;

fn f2z_width() -> usize {
    common::cli::value(
        "F2Z_MUL_WORD_BITS",
        std::env::var_os("F2Z_MUL_WORD_BITS").unwrap_or_else(|| "1".into()),
        |value: &str| match value.parse::<usize>() {
            Ok(width @ 1..=126) => Ok(width),
            _ => Err("expected a width from 1 to 126"),
        },
    )
}

const PROTOCOL_LABEL: &str = "skip-k3";
const STRATEGY_LABEL: &str = "delayed-barrett";

fn exponents() -> Vec<usize> {
    common::shape_values(None, clap::builder::RangedU64ValueParser::<usize>::new().range(15..))
        .unwrap_or_else(|| (15..=25).collect())
}

/// One end-to-end prove: bit-pack + commit (Step 1) + the combined proof.
fn prove_e2e(
    relation: &PreparedRelation<MulLayout<u32>>,
    witness: &MulWitness<u32>,
) -> (Proof, FlockCommitHint) {
    let proving = tracing::info_span!("benchmark:proving").entered();
    let commit = tracing::info_span!("benchmark:commit").entered();
    let bit_rows = witness.f2z_bit_rows();
    let commitment_hint = protocol::commit(relation, bit_rows).expect("F2Z commitment succeeds");
    drop(commit);

    let mut prover_transcript = Blake3Transcript::new();
    let proof = protocol::prove(&mut prover_transcript, relation, witness, &commitment_hint)
        .expect("combined proving succeeds");
    drop(proving);
    (proof, commitment_hint)
}

#[allow(clippy::too_many_arguments)]
fn bench_exponent<P: IopSecurityProfile>(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    pass: BenchmarkPass,
    f2z_width: usize,
    order: usize,
    threads: usize,
) {
    let multiplications = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("multiplication domain fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);

    // Witness generation (excluded from prove).
    let (witness, started) =
        f2z::observability::measure(tracing::info_span!("u32_mul:witness"), || {
            MulWitness::<u32>::from_fn_with_word_bits(multiplications, f2z_width, |_| {
                (rng.random::<u32>(), rng.random::<u32>())
            })
            .expect("valid u32 multiplication witness")
        })
        .expect("measure completed operation");
    let witness_ms = started.as_secs_f64() * 1e3;
    let layout = *witness.layout();
    let params = layout.f2z_params();

    // One-time public preprocessing (excluded from prove): q-independent
    // exact matrices plus the instantiated runtime-prime security profile.
    let started_recording =
        f2z::observability::Recording::start(Vec::new()).expect("start operation capture");
    let started = tracing::info_span!("u32_mul:started").entered();
    let relation = match PreparedRelation::<MulLayout<u32>>::new_with_profile_and_ligerito::<P>(
        layout,
        common::ligerito_selection(P::LIGERITO_TARGET_BITS),
    ) {
        Ok(relation) => relation,
        Err(error @ (ProtocolError::Profile(_) | ProtocolError::UnsupportedProfile)) => {
            println!();
            println!(
                "u32_mul gates=2^{exponent} profile={}: SKIPPED - {error}",
                P::NAME
            );
            return;
        }
        Err(error) => panic!("prepare failed: {error}"),
    };
    let setup_ms = { drop(started); f2z::observability::duration(&started_recording.intervals().expect("complete operation capture"), "u32_mul:started").expect("query completed operation") }.as_secs_f64() * 1e3;
    println!("LIGERITO_CONFIG {}", common::ligerito_report(relation.ligerito_configuration(), relation.security().ood));
    let profile_name = relation.security().profile_name;
    let q_bits = (u128::BITS - relation.security().projection_max.leading_zeros()) as usize;
    let f2z_chunks = q_bits.div_ceil(127 - params.row_vars - layout.word_bits());

    // Excluded warm-up. This is also the first end-to-end correctness check.
    let (warm_proof, warm_hint) = prove_e2e(&relation, &witness);
    let mut verifier_transcript = Blake3Transcript::new();
    protocol::verify(
        &mut verifier_transcript,
        &relation,
        &warm_hint.commitment,
        &warm_proof,
    )
    .expect("warm-up combined verification succeeds");
    let ligerito_log_inv_rate = warm_hint.commitment.params.log_inv_rate;
    let ligerito_initial_k = warm_hint.commitment.params.log_batch_size;
    let ligerito_hash = warm_hint.commitment.params.merkle_hash;
    drop(warm_proof);
    drop(warm_hint);

    let mut latency = None;
    if pass.measures_latency() {
        let mut prover = common::StepSamples::default();
        let mut verifier = common::StepSamples::default();
        let mut last_proof = None;
        for sample_index in 0..reps {

            let recording = f2z::observability::Recording::start(Vec::new()).expect("start u32 trial");
            let (proof, commitment_hint) = prove_e2e(&relation, &witness);

            let mut verifier_transcript = Blake3Transcript::new();
            let verification = tracing::info_span!("benchmark:verification").entered();
            protocol::verify(
                &mut verifier_transcript,
                &relation,
                &commitment_hint.commitment,
                &proof,
            )
            .expect("combined verification succeeds");
            drop(verification);
            let intervals = recording.intervals().expect("query u32 trial");
            let commit_ms = common::span_ms(&intervals, "benchmark:commit");
            let prove_ms = common::span_ms(&intervals, "benchmark:proving");
            let verify_ms = common::span_ms(&intervals, "benchmark:verification");
            let prove_phases = f2z::observability::phase_totals(&intervals, "benchmark:proving").unwrap();
            let verify_phases = f2z::observability::phase_totals(&intervals, "benchmark:verification").unwrap();

            println!(
                "  SAMPLE pass=latency profile={profile_name} order={order} protocol={PROTOCOL_LABEL} skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS} strategy={STRATEGY_LABEL} word_bits={} projection_bits={q_bits} f2z_t={} f2z_s={} f2z_chunks={f2z_chunks} exponent={exponent} sample={} multiplications={multiplications} commit_ms={commit_ms:.6} prove_ms={prove_ms:.6} verify_ms={verify_ms:.6} verified=true",
                layout.word_bits(),
                params.row_vars,
                params.col_vars,
                sample_index + 1,
            );
            common::print_regression_phases(&prove_phases);
            prover.record_prove(prove_ms, commit_ms, &prove_phases);
            verifier.record_verify(verify_ms, &verify_phases);
            black_box(&proof);
            last_proof = Some(proof);
        }

        let last_proof = last_proof.expect("at least one benchmark repetition");
        latency = Some((prover, verifier, last_proof));
    }

    let mut memory_metrics = None;
    if pass.measures_memory() {
        // This proof runs only in the explicit memory pass used by the runner.
        // `both` retains the old direct `cargo bench` behavior for convenience.

        let live_before_prove = live_mib();
        reset_peak();
        let (peak_proof, peak_hint) = prove_e2e(&relation, &witness);
        black_box(&peak_proof);
        let peak = peak_mib();

        let mut verifier_transcript = Blake3Transcript::new();
        protocol::verify(
            &mut verifier_transcript,
            &relation,
            &peak_hint.commitment,
            &peak_proof,
        )
        .expect("peak-memory proof verifies");

        println!(
            "  MEMORY pass=memory profile={profile_name} order={order} protocol={PROTOCOL_LABEL} skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS} strategy={STRATEGY_LABEL} word_bits={} projection_bits={q_bits} f2z_t={} f2z_s={} f2z_chunks={f2z_chunks} exponent={exponent} multiplications={multiplications} peak_heap_mib={peak:.6} live_before_prove_mib={live_before_prove:.6} verified=true",
            layout.word_bits(),
            params.row_vars,
            params.col_vars,
        );
        memory_metrics = Some((live_before_prove, peak));
    }

    println!();
    println!(
        "u32_mul gates=2^{exponent} ({multiplications}) W={} [{}] profile={profile_name} (λ={})  seed={shape_seed:#018x}",
        layout.word_bits(),
        PROTOCOL_LABEL,
        relation.security().lambda,
    );
    println!(
        "  benchmark: pass={} order={order} protocol={PROTOCOL_LABEL} skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS} strategy={STRATEGY_LABEL} t={} s={} chunks={f2z_chunks}",
        pass.as_str(),
        params.row_vars,
        params.col_vars,
    );
    println!(
        "  R1CS: rows={} cols={} nnz={}  |  F2Z: t={} s={} W={} chunks={} bits={} ({:.2} MiB)",
        multiplications,
        4 * layout.capacity(),
        3 * multiplications,
        params.row_vars,
        params.col_vars,
        layout.word_bits(),
        f2z_chunks,
        128usize * layout.capacity(),
        (16usize * layout.capacity()) as f64 / (1024.0 * 1024.0),
    );
    println!(
        "  Ligerito: inverse rate=2^{} initial_k={} hash={:?}",
        ligerito_log_inv_rate, ligerito_initial_k, ligerito_hash,
    );
    if let Some((prover, verifier, last_proof)) = latency {
        let spartan_elements = last_proof.spartan_payload_elements();
        let boundary_nonces = last_proof.grinding_nonce_count(relation.security())
            - last_proof.opening_grinding_nonces().len();
        let report = common::BenchReport {
            bench: "u32_mul",
            shape: format!("2p{exponent}"),
            extra: vec![
            common::ligerito_identity(relation.ligerito_configuration(), relation.security().ood),
                ("profile".into(), profile_name.into()),
                ("pass".into(), pass.as_str().into()),
                ("multiplications".into(), multiplications.to_string()),
                ("protocol".into(), PROTOCOL_LABEL.into()),
                ("skip_vars".into(), U32_MUL_UNIVARIATE_SKIP_VARS.to_string()),
                ("strategy".into(), STRATEGY_LABEL.into()),
                ("word_bits".into(), layout.word_bits().to_string()),
                ("projection_bits".into(), q_bits.to_string()),
                ("f2z_t".into(), params.row_vars.to_string()),
                ("f2z_s".into(), params.col_vars.to_string()),
                ("f2z_chunks".into(), f2z_chunks.to_string()),
                ("exponent".into(), exponent.to_string()),
                ("order".into(), order.to_string()),
                ("shape_seed".into(), format!("{shape_seed:#018x}")),
            ],
            lambda: Some(relation.security().lambda),
            lambda_achieved: Some(relation.security().accounting.achieved_bits()),
            lambda_bind: Some(relation.security().accounting.binding_term().name.into()),
            threads,
            reps,
            seed: Some(root_seed),
            witness_ms,
            setup_ms,
            prover: prover.medians(),
            verifier: verifier.medians(),
            proof: common::ProofBytes {
                piop: spartan_elements * 16 + boundary_nonces * std::mem::size_of::<u64>(),
                open: last_proof.f2z().to_bytes().len(),
            },
        };
        report.print_human();
    }
    if let Some((live_before_prove, peak)) = memory_metrics {
        println!(
            "  heap:   live before prove {live_before_prove:8.2} MiB | peak {peak:8.2} MiB | delta {:8.2} MiB",
            (peak - live_before_prove).max(0.0),
        );
    }
}

fn main() {
    common::cli::EnvironmentCli::parse();
    let reps = common::reps(None, 5);
    let pass = BenchmarkPass::from_env();
    let f2z_width = f2z_width();
    let order = common::cli::env::<std::num::NonZeroUsize>("F2Z_BENCH_ORDER")
        .map_or(1, std::num::NonZeroUsize::get);
    let seed = common::seed(None, 0x5533_326d_756c_0064);
    let selected = common::security_profile(PrimePolicy::SingleDerived);
    let profile = selected.unwrap_or(common::SecurityProfile::Lambda100);

    let exponents = exponents();
    f2z::observability::install().expect("install Perfetto subscriber");
    let threads = common::init();

    println!("u32 × u32 → u64: Spartan PIOP + F2Z assignment opening");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!(
        "repetitions: {reps}; root seed: {seed:#018x}; F2Z word bits: {}",
        f2z_width,
    );
    println!(
        "benchmark pass: {}; protocol: {PROTOCOL_LABEL} (skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS}); strategy: {STRATEGY_LABEL}; order: {order}",
        pass.as_str(),
    );
    println!(
        "security profile: {}",
        common::profile_banner(selected, common::SecurityProfile::Lambda100)
    );

    for exponent in exponents {
        flock_core::scratch::clear();
        common::with_profile!(
            profile,
            bench_exponent(exponent, reps, seed, pass, f2z_width, order, threads)
        );
    }
    flock_core::scratch::clear();
}
