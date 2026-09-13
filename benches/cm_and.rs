//! End-to-end benchmark for the CM-AND relation with the F₂-VIRTUAL
//! `w = x ⊕ y` block: Spartan proves `x + y − w − 2z = 0` per gate; the
//! commitment carries only the `x`/`y`/`z` bits; the virtual F2Z opening
//! derives the `w` bits structurally and opens the terminal claim
//! against the compact commitment. Every measured proof is verified.
//!
//! Defaults to the production sweep `2^15, 2^16` gates. Override with
//! `F2Z_CM_EXPONENTS`, e.g.:
//!
//! ```text
//! F2Z_CM_EXPONENTS="15" F2Z_BENCH_REPS=3 \
//!   cargo bench --bench cm_and --features unchecked
//! ```

mod common;
use clap::builder::TypedValueParser;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use f2z::piop::spartan::{
    CM_AND_F_LIVE_SLOTS, CM_AND_H_SLOTS, CmAndWitness, SpartanF2zField,
    prepare_cm_and_relation, project_cm_and_witness, prove_cm_and_f2z, spartan_f2z_field_config,
    verify_cm_and_f2z,
};
use f2z::transcript::Blake3Transcript;
use rand::{RngExt, SeedableRng, rngs::StdRng};

struct PeakAlloc;

static CURRENT_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

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

#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

fn reset_peak() {
    PEAK_BYTES.store(CURRENT_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn live_mib() -> f64 {
    CURRENT_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn peak_mib() -> f64 {
    PEAK_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|left, right| left.total_cmp(right));
    samples[samples.len() / 2]
}

fn phase_ms(phases: &[(String, f64)], label: &str) -> Option<f64> {
    phases
        .iter()
        .find(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
}

#[derive(clap::Parser)]
struct Env {
    #[arg(long, env = "F2Z_CM_EXPONENTS", default_value = "15 16",
        value_parser = common::cli::list::<usize>.try_map(|values| {
            if values.iter().all(|&n| n >= 15) { Ok(values) } else { Err("expected exponents >=15") }
        }))]
    exponents: common::cli::List<usize>,
    #[arg(long, env = "F2Z_CM_SEED", default_value_t = 0x0043_4d5f_414e_4400)]
    seed: u64,
}

fn bench_exponent(exponent: usize, reps: usize, root_seed: u64) {
    let gates = 1usize << exponent;
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);
    let field_config = spartan_f2z_field_config();

    let (witness, started) = f2z::observability::measure(
        tracing::info_span!("cm_and:witness"),
        || CmAndWitness::from_fn(gates, |_| (rng.random::<u32>(), rng.random::<u32>())).unwrap(),
    ).expect("measure completed operation");
    let witness_ms = started.as_secs_f64() * 1e3;
    let layout = *witness.layout();

    let started_recording = f2z::observability::Recording::start(Vec::new()).expect("start operation capture");
    let started = tracing::info_span!("cm_and:started").entered();
    let relation = prepare_cm_and_relation::<SpartanF2zField>(layout, &field_config)
        .and_then(|p| p.with_ligerito(common::ligerito_selection(100)))
        .expect("valid CM-AND relation");
    let resolved = relation.ligerito_configuration().unwrap();
    println!("LIGERITO_CONFIG {}", common::ligerito_report(resolved, resolved.round0(100).unwrap()));
    let relation_ms = { drop(started); f2z::observability::duration(&started_recording.intervals().expect("complete operation capture"), "cm_and:started").expect("query completed operation") }.as_secs_f64() * 1e3;

    let (bit_rows, started) = f2z::observability::measure(
        tracing::info_span!("cm_and:bit_rows"),
        || witness.f_bit_rows(),
    ).expect("measure completed operation");
    let bit_rows_ms = started.as_secs_f64() * 1e3;

    let (hint, started) = f2z::observability::measure(
        tracing::info_span!("cm_and:hint"),
        || f2z::piop::spartan::cm::commit_cm_and_witness_with_config(&layout, bit_rows, relation.ligerito_configuration().unwrap().prover()).expect("F2Z commitment succeeds"),
    ).expect("measure completed operation");
    let commit_ms = started.as_secs_f64() * 1e3;

    // Excluded warm-up; also the first end-to-end correctness check.
    let warm = project_cm_and_witness::<SpartanF2zField>(&witness, &field_config).unwrap();
    let mut pt = Blake3Transcript::new();
    let warm_proof = prove_cm_and_f2z(&mut pt, &relation, warm, &hint).expect("warm-up prove");
    let mut vt = Blake3Transcript::new();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &warm_proof).expect("warm-up verify");
    drop(warm_proof);


    let mut prove_ms = Vec::with_capacity(reps);
    let mut verify_ms = Vec::with_capacity(reps);
    let mut prove_splits: Vec<[f64; 3]> = Vec::with_capacity(reps);
    let mut verify_splits: Vec<[f64; 3]> = Vec::with_capacity(reps);
    let mut last_proof = None;
    for _ in 0..reps {
        let projected = project_cm_and_witness::<SpartanF2zField>(&witness, &field_config).unwrap();
        let mut pt = Blake3Transcript::new();
        let recording = f2z::observability::Recording::start(Vec::new()).expect("start CM prover");
        let proving = tracing::info_span!("benchmark:proving").entered();
        let proof = prove_cm_and_f2z(&mut pt, &relation, projected, &hint).expect("prove");
        drop(proving);
        let intervals = recording.intervals().expect("query CM prover");
        prove_ms.push(common::span_ms(&intervals, "benchmark:proving"));
        let phases = f2z::observability::phase_totals(&intervals, "benchmark:proving").unwrap();
        if let (Some(a), Some(b), Some(c)) = (
            phase_ms(&phases, "cm-f2z:spartan_prove"),
            phase_ms(&phases, "cm-f2z:bitify_prover"),
            phase_ms(&phases, "cm-f2z:f2z_prove"),
        ) {
            prove_splits.push([a, b, c]);
        }

        let mut vt = Blake3Transcript::new();
        let recording = f2z::observability::Recording::start(Vec::new()).expect("start CM verifier");
        let verification = tracing::info_span!("benchmark:verification").entered();
        verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &proof).expect("verify");
        drop(verification);
        let intervals = recording.intervals().expect("query CM verifier");
        verify_ms.push(common::span_ms(&intervals, "benchmark:verification"));
        let phases = f2z::observability::phase_totals(&intervals, "benchmark:verification").unwrap();
        if let (Some(a), Some(b), Some(c)) = (
            phase_ms(&phases, "cm-f2z:spartan_verify"),
            phase_ms(&phases, "cm-f2z:bitify_verifier"),
            phase_ms(&phases, "cm-f2z:f2z_verify"),
        ) {
            verify_splits.push([a, b, c]);
        }
        black_box(&proof);
        last_proof = Some(proof);
    }

    let last_proof = last_proof.expect("at least one repetition");
    let f2z_bytes = last_proof.f2z().to_bytes().len();
    let spartan_elements = 4 * last_proof.spartan().outer.sumcheck.round_polynomials.len()
        + 3
        + 3 * last_proof.spartan().inner.round_polynomials.len();
    drop(last_proof);

    // One extra proof for the peak-heap measurement.
    let projected = project_cm_and_witness::<SpartanF2zField>(&witness, &field_config).unwrap();
    drop(witness);

    let live_before = live_mib();
    reset_peak();
    let mut pt = Blake3Transcript::new();
    let peak_proof = prove_cm_and_f2z(&mut pt, &relation, projected, &hint).expect("peak prove");
    black_box(&peak_proof);
    let peak = peak_mib();
    let mut vt = Blake3Transcript::new();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &peak_proof).expect("peak verify");


    let prove_median = median(prove_ms);
    let verify_median = median(verify_ms);
    let split = |samples: &Vec<[f64; 3]>, k: usize| {
        (samples.len() == reps).then(|| median(samples.iter().map(|s| s[k]).collect()))
    };

    let derived_bits = CM_AND_H_SLOTS * layout.capacity();
    let committed_live_bits = CM_AND_F_LIVE_SLOTS * layout.capacity();
    println!();
    println!("cm_and gates=2^{exponent} ({gates}) [production]  seed={shape_seed:#018x}");
    println!(
        "  R1CS: rows={} cols={} nnz={} (A=B=0, one linear C row per gate)",
        gates,
        layout.assignment_len(),
        4 * gates,
    );
    println!(
        "  bits: derived h = {} (x|y|z|w) | committed live = {} (x|y|z; w VIRTUAL) | map nnz = {}",
        derived_bits,
        committed_live_bits,
        relation.map().nnz(),
    );
    println!("  setup: witness {witness_ms:8.2} ms | relation+map {relation_ms:8.2} ms | bit-pack {bit_rows_ms:8.2} ms | commit {commit_ms:8.2} ms");
    println!("  prove:  {prove_median:9.2} ms   (median of {reps})");
    println!("  verify: {verify_median:9.2} ms");
    if let (Some(a), Some(b), Some(c)) = (
        split(&prove_splits, 0),
        split(&prove_splits, 1),
        split(&prove_splits, 2),
    ) {
        println!(
            "  prove split: Spartan {a:9.2} ms | bitify {b:8.2} ms | virtual F2Z {c:9.2} ms | residual {:7.2} ms",
            (prove_median - a - b - c).max(0.0),
        );
    }
    if let (Some(a), Some(b), Some(c)) = (
        split(&verify_splits, 0),
        split(&verify_splits, 1),
        split(&verify_splits, 2),
    ) {
        println!(
            "  verify split: Spartan {a:8.2} ms | bitify {b:8.2} ms | virtual F2Z {c:9.2} ms | residual {:7.2} ms",
            (verify_median - a - b - c).max(0.0),
        );
    }
    println!(
        "  proof:  virtual F2Z {f2z_bytes:9} B | Spartan canonical field payload {spartan_elements:6} elements / {:6} B",
        spartan_elements * 16,
    );
    println!(
        "  heap:   live before prove {live_before:8.2} MiB | peak {peak:8.2} MiB | delta {:8.2} MiB",
        (peak - live_before).max(0.0),
    );
    if let (Some(pa), Some(pb), Some(pc), Some(va), Some(vb), Some(vc2)) = (
        split(&prove_splits, 0),
        split(&prove_splits, 1),
        split(&prove_splits, 2),
        split(&verify_splits, 0),
        split(&verify_splits, 1),
        split(&verify_splits, 2),
    ) {
        println!(
            "  RESULT exponent={exponent} gates={gates} derived_bits={derived_bits} committed_live_bits={committed_live_bits} map_nnz={} prove_ms={prove_median:.4} spartan_prove_ms={pa:.4} bitify_prove_ms={pb:.4} f2z_prove_ms={pc:.4} verify_ms={verify_median:.4} spartan_verify_ms={va:.4} bitify_verify_ms={vb:.4} f2z_verify_ms={vc2:.4} spartan_bytes={} f2z_bytes={f2z_bytes} peak_mib={peak:.4}",
            relation.map().nnz(),
            spartan_elements * 16,
        );
    }
}

fn main() {
    common::cli::EnvironmentCli::parse();
    let Env { exponents, seed } = common::cli::environment();
    let reps = common::reps(None, 5);


    f2z::observability::install().expect("install Perfetto subscriber");
    common::enforce_known_env();
    let _ = flock_core::init_perf_thread_pool();

    println!("CM-AND: Spartan (A=B=0) + virtual F2Z opening (w = x XOR y derived, not committed)");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {}", rayon::current_num_threads());
    println!("repetitions: {reps}; root seed: {seed:#018x}");

    for exponent in exponents {
        flock_core::scratch::clear();
        bench_exponent(exponent, reps, seed);
    }
    flock_core::scratch::clear();
}
