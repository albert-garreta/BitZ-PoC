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
//! `F2Z_MUL_WORD_BITS=1|8` selects the F2Z word width; the default is `1`.
//! `F2Z_BENCH_PASS=latency|memory|both` separates the measured repetitions
//! from the extra peak-heap proof. Memory measurement requires the
//! benchmark-only `bench-peak-memory` feature.

mod common;

use std::hint::black_box;
use std::time::Instant;

#[cfg(feature = "bench-peak-memory")]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use f2z::ligerito_flock::FlockCommitHint;
use f2z::pcs::mod_q_num_chunks;
use f2z::piop::spartan::{
    IopSecurityProfile, PreparedU32MulRelation, PrimePolicy, SpartanF2zError,
    U32_MUL_UNIVARIATE_SKIP_VARS, U32MulF2zWidth, U32MulProof, U32MulWitness,
    commit_u32_mul_witness, prove_u32_mul, verify_u32_mul,
};
use f2z::transcript::Blake3Transcript;
use rand::{RngExt, SeedableRng, rngs::StdRng};

#[cfg(feature = "bench-peak-memory")]
struct PeakAlloc;

#[cfg(feature = "bench-peak-memory")]
static CURRENT_BYTES: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "bench-peak-memory")]
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "bench-peak-memory")]
unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let current = CURRENT_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK_BYTES.fetch_max(current, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        CURRENT_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            if new_size >= layout.size() {
                let increase = new_size - layout.size();
                let current = CURRENT_BYTES.fetch_add(increase, Ordering::Relaxed) + increase;
                PEAK_BYTES.fetch_max(current, Ordering::Relaxed);
            } else {
                CURRENT_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        resized
    }
}

#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

#[cfg(feature = "bench-peak-memory")]
fn reset_peak() {
    PEAK_BYTES.store(CURRENT_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}

#[cfg(feature = "bench-peak-memory")]
fn live_mib() -> f64 {
    CURRENT_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

#[cfg(feature = "bench-peak-memory")]
fn peak_mib() -> f64 {
    PEAK_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BenchmarkPass {
    Latency,
    Memory,
    Both,
}

impl BenchmarkPass {
    fn from_env() -> Self {
        match std::env::var("F2Z_BENCH_PASS").as_deref() {
            Ok("latency") => Self::Latency,
            Ok("memory") => Self::Memory,
            Ok("both") => Self::Both,
            Err(_) => Self::Latency,
            Ok(value) => panic!("unsupported F2Z_BENCH_PASS={value}; use latency, memory, or both"),
        }
    }

    const fn measures_latency(self) -> bool {
        matches!(self, Self::Latency | Self::Both)
    }

    const fn measures_memory(self) -> bool {
        matches!(self, Self::Memory | Self::Both)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Latency => "latency",
            Self::Memory => "memory",
            Self::Both => "both",
        }
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive decimal integer")),
        Err(_) => default,
    }
}

fn f2z_width() -> U32MulF2zWidth {
    match env_usize("F2Z_MUL_WORD_BITS", U32MulF2zWidth::W1.word_bits()) {
        1 => U32MulF2zWidth::W1,
        8 => U32MulF2zWidth::W8,
        value => panic!("F2Z_MUL_WORD_BITS must be 1 or 8; got {value}"),
    }
}

const PROTOCOL_LABEL: &str = "skip-k3";
const STRATEGY_LABEL: &str = "delayed-barrett";

fn exponents() -> Vec<usize> {
    common::shapes(None).map_or_else(
        || (15..=25).collect(),
        |shapes| {
            shapes
                .iter()
                .map(|part| {
                    let exponent: usize = part.parse().expect("F2Z_BENCH_SHAPES contains integers");
                    assert!(
                        exponent >= 15,
                        "the combined proof requires at least 2^15 gate slots"
                    );
                    exponent
                })
                .collect()
        },
    )
}

/// One end-to-end prove: bit-pack + commit (Step 1) + the combined proof.
fn prove_e2e(
    relation: &PreparedU32MulRelation,
    witness: &U32MulWitness,
) -> (U32MulProof, FlockCommitHint, f64, f64) {
    let prove_started = Instant::now();
    let commit_started = Instant::now();
    let bit_rows = witness.f2z_bit_rows();
    let commitment_hint =
        commit_u32_mul_witness(relation, bit_rows).expect("F2Z commitment succeeds");
    let commit_ms = common::elapsed_ms(commit_started);

    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_u32_mul(&mut prover_transcript, relation, witness, &commitment_hint)
        .expect("combined proving succeeds");
    let prove_ms = common::elapsed_ms(prove_started);
    (proof, commitment_hint, prove_ms, commit_ms)
}

#[allow(clippy::too_many_arguments)]
fn bench_exponent<P: IopSecurityProfile>(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    pass: BenchmarkPass,
    f2z_width: U32MulF2zWidth,
    order: usize,
    threads: usize,
) {
    let multiplications = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("multiplication domain fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);

    // Witness generation (excluded from prove).
    let started = Instant::now();
    let witness = U32MulWitness::from_fn_with_f2z_width(multiplications, f2z_width, |_| {
        (rng.random::<u32>(), rng.random::<u32>())
    })
    .expect("valid u32 multiplication witness");
    let witness_ms = common::elapsed_ms(started);
    let layout = *witness.layout();
    let params = layout.f2z_params();

    // One-time public preprocessing (excluded from prove): q-independent
    // exact matrices plus the instantiated runtime-prime security profile.
    let started = Instant::now();
    let relation = match PreparedU32MulRelation::new_with_profile_and_ligerito::<P>(layout, common::ligerito_selection(P::LIGERITO_TARGET_BITS)) {
        Ok(relation) => relation,
        Err(error @ (SpartanF2zError::Profile(_) | SpartanF2zError::UnsupportedProfile)) => {
            println!();
            println!(
                "u32_mul gates=2^{exponent} profile={}: SKIPPED - {error}",
                P::NAME
            );
            return;
        }
        Err(error) => panic!("prepare failed: {error}"),
    };
    let setup_ms = common::elapsed_ms(started);
    println!("LIGERITO_CONFIG {}", common::ligerito_report(relation.ligerito_configuration(), relation.security().ood));
    let profile_name = relation.security().profile_name;
    let q_bits = (u128::BITS - relation.security().projection_max.leading_zeros()) as usize;
    let f2z_chunks = mod_q_num_chunks(&params, q_bits);

    // Excluded warm-up. This is also the first end-to-end correctness check.
    let (warm_proof, warm_hint, _, _) = prove_e2e(&relation, &witness);
    let mut verifier_transcript = Blake3Transcript::new();
    verify_u32_mul(
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
    let _ = f2z::utils::prof::take_totals();

    let mut latency = None;
    if pass.measures_latency() {
        let mut prover = common::StepSamples::default();
        let mut verifier = common::StepSamples::default();
        let mut last_proof = None;
        for sample_index in 0..reps {
            let _ = f2z::utils::prof::take_totals();
            let (proof, commitment_hint, prove_ms, commit_ms) = prove_e2e(&relation, &witness);
            let prove_phases = f2z::utils::prof::take_totals();

            let mut verifier_transcript = Blake3Transcript::new();
            let started = Instant::now();
            verify_u32_mul(
                &mut verifier_transcript,
                &relation,
                &commitment_hint.commitment,
                &proof,
            )
            .expect("combined verification succeeds");
            let verify_ms = common::elapsed_ms(started);
            let verify_phases = f2z::utils::prof::take_totals();

            println!(
                "  SAMPLE pass=latency profile={profile_name} order={order} protocol={PROTOCOL_LABEL} skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS} strategy={STRATEGY_LABEL} word_bits={} projection_bits={q_bits} f2z_t={} f2z_s={} f2z_chunks={f2z_chunks} exponent={exponent} sample={} multiplications={multiplications} commit_ms={commit_ms:.6} prove_ms={prove_ms:.6} verify_ms={verify_ms:.6} verified=true",
                params.word_bits,
                params.row_vars,
                params.col_vars,
                sample_index + 1,
            );
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
        let _ = f2z::utils::prof::take_totals();
        let live_before_prove = live_mib();
        reset_peak();
        let (peak_proof, peak_hint, _, _) = prove_e2e(&relation, &witness);
        black_box(&peak_proof);
        let peak = peak_mib();
        let _ = f2z::utils::prof::take_totals();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u32_mul(
            &mut verifier_transcript,
            &relation,
            &peak_hint.commitment,
            &peak_proof,
        )
        .expect("peak-memory proof verifies");
        let _ = f2z::utils::prof::take_totals();
        println!(
            "  MEMORY pass=memory profile={profile_name} order={order} protocol={PROTOCOL_LABEL} skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS} strategy={STRATEGY_LABEL} word_bits={} projection_bits={q_bits} f2z_t={} f2z_s={} f2z_chunks={f2z_chunks} exponent={exponent} multiplications={multiplications} peak_heap_mib={peak:.6} live_before_prove_mib={live_before_prove:.6} verified=true",
            params.word_bits, params.row_vars, params.col_vars,
        );
        memory_metrics = Some((live_before_prove, peak));
    }

    println!();
    println!(
        "u32_mul gates=2^{exponent} ({multiplications}) W={} [{}] profile={profile_name} (λ={})  seed={shape_seed:#018x}",
        params.word_bits,
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
        params.word_bits,
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
            - last_proof.f2z().grinding_nonces.len();
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
                ("word_bits".into(), params.word_bits.to_string()),
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
    let threads = common::init();
    let reps = common::reps(None, 5);
    let pass = BenchmarkPass::from_env();
    assert!(
        !pass.measures_memory() || cfg!(feature = "bench-peak-memory"),
        "F2Z_BENCH_PASS=memory|both requires --features bench-peak-memory"
    );
    let f2z_width = f2z_width();
    let order = env_usize("F2Z_BENCH_ORDER", 1);
    assert!(order > 0, "F2Z_BENCH_ORDER must be positive");
    let seed = common::seed(None, 0x5533_326d_756c_0064);
    let selected = common::security_profile(PrimePolicy::SingleDerived);
    let profile = selected.unwrap_or(common::SecurityProfile::Lambda100);

    println!("u32 × u32 → u64: Spartan PIOP + F2Z assignment opening");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!(
        "repetitions: {reps}; root seed: {seed:#018x}; F2Z word bits: {}",
        f2z_width.word_bits(),
    );
    println!(
        "benchmark pass: {}; protocol: {PROTOCOL_LABEL} (skip_vars={U32_MUL_UNIVARIATE_SKIP_VARS}); strategy: {STRATEGY_LABEL}; order: {order}",
        pass.as_str(),
    );
    println!(
        "security profile: {}",
        common::profile_banner(selected, common::SecurityProfile::Lambda100)
    );

    for exponent in exponents() {
        flock_core::scratch::clear();
        common::with_profile!(
            profile,
            bench_exponent(exponent, reps, seed, pass, f2z_width, order, threads)
        );
    }
    flock_core::scratch::clear();
}
