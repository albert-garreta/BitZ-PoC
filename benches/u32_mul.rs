//! End-to-end benchmark for a batch of integer `u32 * u32 = u64` constraints.
//!
//! Spartan proves the integer-valued R1CS relation. Its terminal assignment-MLE
//! claim is then discharged against the compact 32/32/64-bit witness with F2Z.
//! Every measured proof is verified through the combined verifier.
//!
//! Defaults to the production sweep `2^15, ..., 2^25` multiplications. Override
//! it with `F2Z_MUL_EXPONENTS`, for example:
//!
//! ```text
//! F2Z_MUL_EXPONENTS="15 17 19" F2Z_BENCH_REPS=3 \
//!   cargo bench --bench u32_mul --features unchecked
//! ```
//!
//! `F2Z_MUL_EXPONENTS=15 F2Z_BENCH_REPS=1` is the smallest production smoke
//! shape. `F2Z_SPARTAN_REDUCTION` selects `immediate`, `delayed-barrett`, or
//! `delayed-crypto-bigint`. `F2Z_BENCH_PASS=latency|memory|both` separates the
//! measured repetitions from the extra peak-heap proof. Memory measurement
//! requires the benchmark-only `bench-peak-memory` feature. The default is
//! latency-only so an ordinary invocation has no allocator instrumentation.

use std::hint::black_box;
use std::time::Instant;

#[cfg(feature = "bench-peak-memory")]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use f2z::piop::spartan::{
    SpartanF2zField, SpartanReductionStrategy, U32MulSpartanF2zProof, U32MulWitness,
    commit_u32_mul_witness, prepare_u32_mul_relation, project_u32_mul_witness,
    prove_u32_mul_spartan_and_f2z_with_strategy, spartan_f2z_field_config,
    verify_u32_mul_spartan_and_f2z,
};
use f2z::transcript::Blake3Transcript;
use f2z::{ligerito::packed_vars, ligerito_flock::sha_lig_configs};
use rand::{Rng, SeedableRng, rngs::StdRng};

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

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|left, right| left.total_cmp(right));
    samples[samples.len() / 2]
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

fn reduction_strategy() -> SpartanReductionStrategy {
    match std::env::var("F2Z_SPARTAN_REDUCTION").as_deref() {
        Ok("immediate") | Err(_) => SpartanReductionStrategy::Immediate,
        Ok("delayed-barrett") => SpartanReductionStrategy::DelayedBarrett,
        Ok("delayed-crypto-bigint") => SpartanReductionStrategy::DelayedCryptoBigint,
        Ok(value) => panic!(
            "unsupported F2Z_SPARTAN_REDUCTION={value}; use immediate, delayed-barrett, or delayed-crypto-bigint"
        ),
    }
}

const fn strategy_name(strategy: SpartanReductionStrategy) -> &'static str {
    match strategy {
        SpartanReductionStrategy::Immediate => "immediate",
        SpartanReductionStrategy::DelayedBarrett => "delayed-barrett",
        SpartanReductionStrategy::DelayedCryptoBigint => "delayed-crypto-bigint",
    }
}

const fn reduction_backend(strategy: SpartanReductionStrategy) -> &'static str {
    match strategy {
        SpartanReductionStrategy::Immediate => "immediate",
        SpartanReductionStrategy::DelayedBarrett => "barrett",
        SpartanReductionStrategy::DelayedCryptoBigint => "crypto-bigint",
    }
}

fn optional_ms(value: Option<f64>) -> String {
    value.map_or_else(|| "NA".to_owned(), |value| format!("{value:.9}"))
}

#[derive(Default)]
struct PhaseSamples {
    spartan_prove_ms: Vec<f64>,
    spartan_outer_ms: Vec<f64>,
    spartan_bind_ms: Vec<f64>,
    spartan_inner_ms: Vec<f64>,
    bitify_prove_ms: Vec<f64>,
    f2z_prove_ms: Vec<f64>,
    prove_residual_ms: Vec<f64>,
    spartan_verify_ms: Vec<f64>,
    bitify_verify_ms: Vec<f64>,
    f2z_verify_ms: Vec<f64>,
    verify_residual_ms: Vec<f64>,
}

#[derive(Clone, Copy)]
struct PhaseMedians {
    spartan_prove_ms: f64,
    spartan_outer_ms: Option<f64>,
    spartan_bind_ms: Option<f64>,
    spartan_inner_ms: Option<f64>,
    bitify_prove_ms: f64,
    f2z_prove_ms: f64,
    prove_residual_ms: f64,
    spartan_verify_ms: f64,
    bitify_verify_ms: f64,
    f2z_verify_ms: f64,
    verify_residual_ms: f64,
}

fn phase_ms(phases: &[(&'static str, f64)], label: &str) -> Option<f64> {
    phases
        .iter()
        .find(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
}

impl PhaseSamples {
    fn record_prove(&mut self, phases: &[(&'static str, f64)], total_ms: f64) {
        let (Some(spartan), Some(bitify), Some(f2z)) = (
            phase_ms(phases, "spartan-f2z:spartan_prove"),
            phase_ms(phases, "spartan-f2z:bitify_prover"),
            phase_ms(phases, "spartan-f2z:f2z_prove"),
        ) else {
            return;
        };
        self.spartan_prove_ms.push(spartan);
        if let Some(value) = phase_ms(phases, "spartan:outer_sumcheck") {
            self.spartan_outer_ms.push(value);
        }
        if let Some(value) = phase_ms(phases, "spartan:bind_and_batch") {
            self.spartan_bind_ms.push(value);
        }
        if let Some(value) = phase_ms(phases, "spartan:inner_sumcheck") {
            self.spartan_inner_ms.push(value);
        }
        self.bitify_prove_ms.push(bitify);
        self.f2z_prove_ms.push(f2z);
        self.prove_residual_ms
            .push((total_ms - spartan - bitify - f2z).max(0.0));
    }

    fn record_verify(&mut self, phases: &[(&'static str, f64)], total_ms: f64) {
        let (Some(spartan), Some(bitify), Some(f2z)) = (
            phase_ms(phases, "spartan-f2z:spartan_verify"),
            phase_ms(phases, "spartan-f2z:bitify_verifier"),
            phase_ms(phases, "spartan-f2z:f2z_verify"),
        ) else {
            return;
        };
        self.spartan_verify_ms.push(spartan);
        self.bitify_verify_ms.push(bitify);
        self.f2z_verify_ms.push(f2z);
        self.verify_residual_ms
            .push((total_ms - spartan - bitify - f2z).max(0.0));
    }

    fn medians(&self, reps: usize) -> Option<PhaseMedians> {
        let complete = [
            self.spartan_prove_ms.len(),
            self.bitify_prove_ms.len(),
            self.f2z_prove_ms.len(),
            self.prove_residual_ms.len(),
            self.spartan_verify_ms.len(),
            self.bitify_verify_ms.len(),
            self.f2z_verify_ms.len(),
            self.verify_residual_ms.len(),
        ]
        .into_iter()
        .all(|count| count == reps);
        complete.then(|| PhaseMedians {
            spartan_prove_ms: median(self.spartan_prove_ms.clone()),
            spartan_outer_ms: (self.spartan_outer_ms.len() == reps)
                .then(|| median(self.spartan_outer_ms.clone())),
            spartan_bind_ms: (self.spartan_bind_ms.len() == reps)
                .then(|| median(self.spartan_bind_ms.clone())),
            spartan_inner_ms: (self.spartan_inner_ms.len() == reps)
                .then(|| median(self.spartan_inner_ms.clone())),
            bitify_prove_ms: median(self.bitify_prove_ms.clone()),
            f2z_prove_ms: median(self.f2z_prove_ms.clone()),
            prove_residual_ms: median(self.prove_residual_ms.clone()),
            spartan_verify_ms: median(self.spartan_verify_ms.clone()),
            bitify_verify_ms: median(self.bitify_verify_ms.clone()),
            f2z_verify_ms: median(self.f2z_verify_ms.clone()),
            verify_residual_ms: median(self.verify_residual_ms.clone()),
        })
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

fn parse_seed(value: &str) -> u64 {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).expect("F2Z_MUL_SEED contains a valid hexadecimal u64")
    } else {
        value
            .parse()
            .expect("F2Z_MUL_SEED contains a valid decimal u64")
    }
}

fn exponents() -> Vec<usize> {
    match std::env::var("F2Z_MUL_EXPONENTS") {
        Ok(value) => value
            .split([',', ' '])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let exponent: usize = part.parse().expect("F2Z_MUL_EXPONENTS contains integers");
                assert!(
                    exponent >= 15,
                    "the combined proof requires at least 2^15 gate slots"
                );
                exponent
            })
            .collect(),
        Err(_) => (15..=25).collect(),
    }
}

fn spartan_payload_elements(proof: &U32MulSpartanF2zProof) -> usize {
    4 * proof.spartan.outer.sumcheck.round_polynomials.len()
        + 3
        + 3 * proof.spartan.inner.round_polynomials.len()
}

fn bench_exponent(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    pass: BenchmarkPass,
    strategy: SpartanReductionStrategy,
    order: usize,
) {
    let multiplications = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("multiplication domain fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);

    let started = Instant::now();
    let witness = U32MulWitness::from_fn(multiplications, |_| {
        (rng.random::<u32>(), rng.random::<u32>())
    })
    .expect("valid u32 multiplication witness");
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;
    let layout = *witness.layout();

    let field_config = spartan_f2z_field_config();
    let started = Instant::now();
    let relation =
        prepare_u32_mul_relation::<SpartanF2zField>(layout, &field_config).expect("valid relation");
    let relation_ms = started.elapsed().as_secs_f64() * 1e3;

    let started = Instant::now();
    let projected = project_u32_mul_witness::<SpartanF2zField>(&witness, &field_config)
        .expect("field projection succeeds");
    black_box(&projected);
    let projection_ms = started.elapsed().as_secs_f64() * 1e3;
    drop(projected);

    let started = Instant::now();
    let bit_rows = witness.f2z_bit_rows();
    let bit_rows_ms = started.elapsed().as_secs_f64() * 1e3;

    let started = Instant::now();
    let commitment_hint =
        commit_u32_mul_witness(&layout, bit_rows).expect("F2Z commitment succeeds");
    let commit_ms = started.elapsed().as_secs_f64() * 1e3;

    // Excluded warm-up. This is also the first end-to-end correctness check.
    let mut prover_transcript = Blake3Transcript::new();
    let warm_proof = prove_u32_mul_spartan_and_f2z_with_strategy(
        &mut prover_transcript,
        &relation,
        &layout,
        &witness,
        &commitment_hint,
        strategy,
    )
    .expect("warm-up proving succeeds");
    let mut verifier_transcript = Blake3Transcript::new();
    verify_u32_mul_spartan_and_f2z(
        &mut verifier_transcript,
        &relation,
        &layout,
        &commitment_hint.commitment,
        &warm_proof,
    )
    .expect("warm-up combined verification succeeds");
    drop(warm_proof);
    let _ = f2z::utils::prof::take_totals();

    let strategy_label = strategy_name(strategy);
    let backend_label = reduction_backend(strategy);
    let mut latency_metrics = None;
    if pass.measures_latency() {
        let mut prove_ms = Vec::with_capacity(reps);
        let mut verify_ms = Vec::with_capacity(reps);
        let mut phase_samples = PhaseSamples::default();
        let mut last_proof = None;
        for sample_index in 0..reps {
            let mut prover_transcript = Blake3Transcript::new();
            let started = Instant::now();
            let proof = prove_u32_mul_spartan_and_f2z_with_strategy(
                &mut prover_transcript,
                &relation,
                &layout,
                &witness,
                &commitment_hint,
                strategy,
            )
            .expect("combined proving succeeds");
            let measured_prove_ms = started.elapsed().as_secs_f64() * 1e3;
            prove_ms.push(measured_prove_ms);
            let prove_phases = f2z::utils::prof::take_totals();
            phase_samples.record_prove(&prove_phases, measured_prove_ms);

            let mut verifier_transcript = Blake3Transcript::new();
            let started = Instant::now();
            verify_u32_mul_spartan_and_f2z(
                &mut verifier_transcript,
                &relation,
                &layout,
                &commitment_hint.commitment,
                &proof,
            )
            .expect("combined verification succeeds");
            let measured_verify_ms = started.elapsed().as_secs_f64() * 1e3;
            verify_ms.push(measured_verify_ms);
            let verify_phases = f2z::utils::prof::take_totals();
            phase_samples.record_verify(&verify_phases, measured_verify_ms);

            let sample_spartan = optional_ms(phase_ms(&prove_phases, "spartan-f2z:spartan_prove"));
            let sample_outer = optional_ms(phase_ms(&prove_phases, "spartan:outer_sumcheck"));
            let sample_bind = optional_ms(phase_ms(&prove_phases, "spartan:bind_and_batch"));
            let sample_inner = optional_ms(phase_ms(&prove_phases, "spartan:inner_sumcheck"));
            let sample_bitify = optional_ms(phase_ms(&prove_phases, "spartan-f2z:bitify_prover"));
            let sample_f2z = optional_ms(phase_ms(&prove_phases, "spartan-f2z:f2z_prove"));
            let sample_spartan_verify =
                optional_ms(phase_ms(&verify_phases, "spartan-f2z:spartan_verify"));
            let sample_bitify_verify =
                optional_ms(phase_ms(&verify_phases, "spartan-f2z:bitify_verifier"));
            let sample_f2z_verify = optional_ms(phase_ms(&verify_phases, "spartan-f2z:f2z_verify"));
            println!(
                "  SAMPLE pass=latency order={order} strategy={strategy_label} reduction_backend={backend_label} exponent={exponent} multiplications={multiplications} sample={sample_index} prove_ms={measured_prove_ms:.9} spartan_prove_ms={sample_spartan} spartan_outer_ms={sample_outer} spartan_bind_ms={sample_bind} spartan_inner_ms={sample_inner} bitify_prove_ms={sample_bitify} f2z_prove_ms={sample_f2z} verify_ms={measured_verify_ms:.9} spartan_verify_ms={sample_spartan_verify} bitify_verify_ms={sample_bitify_verify} f2z_verify_ms={sample_f2z_verify} verified=true"
            );

            black_box(&proof);
            last_proof = Some(proof);
        }

        let last_proof = last_proof.expect("at least one benchmark repetition");
        let f2z_proof_bytes = last_proof.f2z.to_bytes().len();
        let spartan_elements = spartan_payload_elements(&last_proof);
        let spartan_payload_bytes = spartan_elements * 16;
        let prove_median_ms = median(prove_ms);
        let verify_median_ms = median(verify_ms);
        let phase_medians = phase_samples.medians(reps);
        latency_metrics = Some((
            prove_median_ms,
            verify_median_ms,
            phase_medians,
            f2z_proof_bytes,
            spartan_elements,
            spartan_payload_bytes,
        ));
    }

    let mut memory_metrics = None;
    if pass.measures_memory() {
        // This proof runs only in the explicit memory pass used by the runner.
        // `both` retains the old direct `cargo bench` behavior for convenience.
        let _ = f2z::utils::prof::take_totals();
        let live_before_prove = live_mib();
        reset_peak();
        let mut prover_transcript = Blake3Transcript::new();
        let peak_proof = prove_u32_mul_spartan_and_f2z_with_strategy(
            &mut prover_transcript,
            &relation,
            &layout,
            &witness,
            &commitment_hint,
            strategy,
        )
        .expect("peak proving succeeds");
        black_box(&peak_proof);
        let peak = peak_mib();
        let _ = f2z::utils::prof::take_totals();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_u32_mul_spartan_and_f2z(
            &mut verifier_transcript,
            &relation,
            &layout,
            &commitment_hint.commitment,
            &peak_proof,
        )
        .expect("peak-memory proof verifies");
        let _ = f2z::utils::prof::take_totals();
        println!(
            "  MEMORY pass=memory order={order} strategy={strategy_label} reduction_backend={backend_label} exponent={exponent} multiplications={multiplications} peak_heap_mib={peak:.6} live_before_prove_mib={live_before_prove:.6} verified=true"
        );
        memory_metrics = Some((live_before_prove, peak));
    }

    let params = layout.f2z_params();
    let (lig_pc, _) = sha_lig_configs(packed_vars(&params)).expect("production Ligerito config");
    println!();
    println!(
        "u32_mul gates=2^{exponent} ({multiplications}) [production]  seed={shape_seed:#018x}"
    );
    println!(
        "  benchmark: pass={} order={order} strategy={strategy_label} reduction={backend_label}",
        pass.as_str(),
    );
    println!(
        "  R1CS: rows={} cols={} nnz={}  |  F2Z: t={} s={} W={} bits={} ({:.2} MiB)",
        multiplications,
        4 * layout.capacity(),
        3 * multiplications,
        params.t,
        params.s,
        params.word_bits,
        128usize * layout.capacity(),
        (16usize * layout.capacity()) as f64 / (1024.0 * 1024.0),
    );
    println!(
        "  witness: {} integer values | {} compact bits / {} bytes | 1 multiplication per row",
        layout.assignment_len(),
        128usize * layout.capacity(),
        16usize * layout.capacity(),
    );
    println!(
        "  Ligerito: inverse rate=2^{} initial_k={} hash={:?}",
        lig_pc.log_inv_rates[0], lig_pc.initial_k, lig_pc.merkle_hash,
    );
    println!(
        "  setup: witness {witness_ms:9.2} ms | relation {relation_ms:9.2} ms | projection {projection_ms:9.2} ms"
    );
    println!("         bit-pack {bit_rows_ms:8.2} ms | commitment {commit_ms:9.2} ms");
    if let Some((
        prove_median_ms,
        verify_median_ms,
        phase_medians,
        f2z_proof_bytes,
        spartan_elements,
        spartan_payload_bytes,
    )) = latency_metrics
    {
        println!("  prove:  {prove_median_ms:9.2} ms   (median of {reps})");
        println!("  verify: {verify_median_ms:9.2} ms");
        if let Some(phases) = phase_medians {
            println!(
                "  Spartan split: outer {} ms | bind {} ms | inner {} ms",
                optional_ms(phases.spartan_outer_ms),
                optional_ms(phases.spartan_bind_ms),
                optional_ms(phases.spartan_inner_ms),
            );
            println!(
                "  prove split: Spartan {:9.2} ms | bitify {:8.2} ms | F2Z PCS {:9.2} ms | residual {:7.2} ms",
                phases.spartan_prove_ms,
                phases.bitify_prove_ms,
                phases.f2z_prove_ms,
                phases.prove_residual_ms,
            );
            println!(
                "  verify split: Spartan {:8.2} ms | bitify {:8.2} ms | F2Z PCS {:9.2} ms | residual {:7.2} ms",
                phases.spartan_verify_ms,
                phases.bitify_verify_ms,
                phases.f2z_verify_ms,
                phases.verify_residual_ms,
            );
        }
        println!(
            "  proof:  F2Z {:9} B | Spartan canonical field payload {:6} elements / {:6} B",
            f2z_proof_bytes, spartan_elements, spartan_payload_bytes,
        );
        if let Some(phases) = phase_medians {
            let outer = optional_ms(phases.spartan_outer_ms);
            let bind = optional_ms(phases.spartan_bind_ms);
            let inner = optional_ms(phases.spartan_inner_ms);
            println!(
                "  RESULT pass=latency order={order} strategy={strategy_label} reduction_backend={backend_label} exponent={exponent} multiplications={multiplications} r1cs_rows={multiplications} r1cs_columns={} r1cs_nnz={} integer_witness_values={} compact_witness_bits={} compact_witness_bytes={} prove_ms={prove_median_ms:.9} spartan_prove_ms={:.9} spartan_outer_ms={outer} spartan_bind_ms={bind} spartan_inner_ms={inner} bitify_prove_ms={:.9} f2z_prove_ms={:.9} verify_ms={verify_median_ms:.9} spartan_verify_ms={:.9} bitify_verify_ms={:.9} f2z_verify_ms={:.9} spartan_proof_payload_bytes={spartan_payload_bytes} f2z_proof_bytes={f2z_proof_bytes} shape_seed={shape_seed:#018x} verified_samples={reps}",
                4 * layout.capacity(),
                3 * multiplications,
                layout.assignment_len(),
                128usize * layout.capacity(),
                16usize * layout.capacity(),
                phases.spartan_prove_ms,
                phases.bitify_prove_ms,
                phases.f2z_prove_ms,
                phases.spartan_verify_ms,
                phases.bitify_verify_ms,
                phases.f2z_verify_ms,
            );
        }
    }
    if let Some((live_before_prove, peak)) = memory_metrics {
        println!(
            "  heap:   live before prove {live_before_prove:8.2} MiB | peak {peak:8.2} MiB | delta {:8.2} MiB",
            (peak - live_before_prove).max(0.0),
        );
    }
}

fn main() {
    let _ = flock_core::init_perf_thread_pool();
    let reps = env_usize("F2Z_BENCH_REPS", 5);
    assert!(reps > 0, "F2Z_BENCH_REPS must be positive");
    let pass = BenchmarkPass::from_env();
    assert!(
        !pass.measures_memory() || cfg!(feature = "bench-peak-memory"),
        "F2Z_BENCH_PASS=memory|both requires --features bench-peak-memory"
    );
    let strategy = reduction_strategy();
    let order = env_usize("F2Z_BENCH_ORDER", 1);
    assert!(order > 0, "F2Z_BENCH_ORDER must be positive");
    let seed = std::env::var("F2Z_MUL_SEED")
        .ok()
        .map(|value| parse_seed(&value))
        .unwrap_or(0x5533_326d_756c_0064);

    println!("u32 × u32 → u64: Spartan PIOP + F2Z assignment opening");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {}", rayon::current_num_threads());
    println!("repetitions: {reps}; root seed: {seed:#018x}");
    println!(
        "benchmark pass: {}; strategy: {}; order: {order}",
        pass.as_str(),
        strategy_name(strategy),
    );

    for exponent in exponents() {
        flock_core::scratch::clear();
        bench_exponent(exponent, reps, seed, pass, strategy, order);
    }
    flock_core::scratch::clear();
}
