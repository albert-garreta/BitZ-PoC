//! Controlled outer-sumcheck benchmark for native u32 multiplication.
//!
//! The benchmark holds the witness, prepared relation, field, reducer, inner
//! sumcheck, and assignment binding fixed while comparing the standard cubic
//! outer sumcheck with the univariate prefix skip for `K=1,2,3,4`; the
//! high-level protocol fixes `K=3`.
//! Every measured proof is verified and its canonical field payload is counted.
//!
//! One excluded warm-up is run per protocol. Measured protocols are rotated to
//! reduce fixed ordering bias. The default is a small `2^15` smoke comparison
//! with five measured repetitions. Override it with, for example:
//!
//! ```text
//! BITZ_BENCH_SHAPES="15 17 19" BITZ_BENCH_REPS=7 \
//!   cargo bench --bench u32_mul_outer_skip
//! ```
//!
//! `BITZ_BENCH_PASS=latency|memory|both` separates repeated timing trials from
//! the extra peak-heap proof. Memory measurement requires
//! `--features bench-peak-memory`; keeping it in a separate invocation avoids
//! allocator accounting in the latency run. `BITZ_BENCH_ORDER` rotates the
//! first measured protocol across fresh processes.

mod common;

use std::{hint::black_box};

#[cfg(feature = "bench-peak-memory")]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use bitz::{
    piop::spartan::{
        PreparedConstraintMatrices, R1csProductMles, SpartanBitzField, SpartanPiopProof,
        U32MulWitness, UnivariateSkipSpartanPiopProof, prepare_u32_mul_relation,
        project_u32_mul_native_witness, prove_spartan_piop_u32_native,
        prove_spartan_piop_u32_native_with_univariate_skip, spartan_bitz_field_config,
        verify_spartan_proof, verify_spartan_univariate_skip_proof,
    },
    poly::mle::DenseMultilinearExtension,
    transcript::Blake3Transcript,
};
use rand::{RngExt, SeedableRng, rngs::StdRng};

const ASSIGNMENT_BINDING: [u8; 32] = [0x73; 32];
const FIELD_ELEMENT_BYTES: usize = 16;
const DEFAULT_EXPONENT: usize = 15;
const DEFAULT_REPETITIONS: usize = 5;

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

use common::cli::BenchmarkPass;



#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Protocol {
    Standard,
    Skip(usize),
}

impl Protocol {
    const ALL: [Self; 5] = [
        Self::Standard,
        Self::Skip(1),
        Self::Skip(2),
        Self::Skip(3),
        Self::Skip(4),
    ];

    const fn index(self) -> usize {
        match self {
            Self::Standard => 0,
            Self::Skip(skip_vars) => skip_vars,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Skip(1) => "skip-k1",
            Self::Skip(2) => "skip-k2",
            Self::Skip(3) => "skip-k3",
            Self::Skip(4) => "skip-k4",
            Self::Skip(_) => unreachable!(),
        }
    }

    const fn skip_vars(self) -> usize {
        match self {
            Self::Standard => 0,
            Self::Skip(skip_vars) => skip_vars,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ProofShape {
    outer_fields: usize,
    proof_fields: usize,
}

#[derive(Clone, Copy, Debug)]
struct PhaseTimings {
    /// Inclusive aggregate. Its nested skip phases are reported separately,
    /// never added to this value.
    outer_total_ms: f64,
    skip_message_ms: Option<f64>,
    reconstruct_prove_ms: Option<f64>,
    reconstruct_verify_ms: Option<f64>,
    prefix_fold_ms: Option<f64>,
    cubic_tail_ms: Option<f64>,
    matrix_bind_ms: f64,
    inner_sumcheck_ms: f64,
}

#[derive(Clone, Copy, Debug)]
struct LatencyTrial {
    prove_ms: f64,
    verify_ms: f64,
    phases: PhaseTimings,
    shape: ProofShape,
}

#[derive(Clone, Copy, Debug)]
struct MemoryTrial {
    live_before_prove_mib: f64,
    peak_heap_mib: f64,
    shape: ProofShape,
}

#[derive(Default)]
struct ProtocolSamples {
    prove_ms: Vec<f64>,
    verify_ms: Vec<f64>,
    outer_total_ms: Vec<f64>,
    skip_message_ms: Vec<f64>,
    reconstruct_prove_ms: Vec<f64>,
    reconstruct_verify_ms: Vec<f64>,
    prefix_fold_ms: Vec<f64>,
    cubic_tail_ms: Vec<f64>,
    matrix_bind_ms: Vec<f64>,
    inner_sumcheck_ms: Vec<f64>,
    shape: Option<ProofShape>,
}

#[derive(Clone, Copy, Debug)]
struct ProtocolMedians {
    prove_ms: f64,
    verify_ms: f64,
    phases: PhaseTimings,
    shape: ProofShape,
}

impl ProtocolSamples {
    fn push(&mut self, trial: LatencyTrial) {
        if let Some(expected) = self.shape {
            assert_eq!(expected.outer_fields, trial.shape.outer_fields);
            assert_eq!(expected.proof_fields, trial.shape.proof_fields);
        }
        self.shape = Some(trial.shape);
        self.prove_ms.push(trial.prove_ms);
        self.verify_ms.push(trial.verify_ms);
        self.outer_total_ms.push(trial.phases.outer_total_ms);
        push_optional(&mut self.skip_message_ms, trial.phases.skip_message_ms);
        push_optional(
            &mut self.reconstruct_prove_ms,
            trial.phases.reconstruct_prove_ms,
        );
        push_optional(
            &mut self.reconstruct_verify_ms,
            trial.phases.reconstruct_verify_ms,
        );
        push_optional(&mut self.prefix_fold_ms, trial.phases.prefix_fold_ms);
        push_optional(&mut self.cubic_tail_ms, trial.phases.cubic_tail_ms);
        self.matrix_bind_ms.push(trial.phases.matrix_bind_ms);
        self.inner_sumcheck_ms.push(trial.phases.inner_sumcheck_ms);
    }

    fn medians(&self, repetitions: usize) -> ProtocolMedians {
        assert_eq!(self.prove_ms.len(), repetitions);
        assert_eq!(self.verify_ms.len(), repetitions);
        assert_eq!(self.outer_total_ms.len(), repetitions);
        assert_eq!(self.matrix_bind_ms.len(), repetitions);
        assert_eq!(self.inner_sumcheck_ms.len(), repetitions);
        ProtocolMedians {
            prove_ms: median(&self.prove_ms),
            verify_ms: median(&self.verify_ms),
            phases: PhaseTimings {
                outer_total_ms: median(&self.outer_total_ms),
                skip_message_ms: optional_median(&self.skip_message_ms, repetitions),
                reconstruct_prove_ms: optional_median(&self.reconstruct_prove_ms, repetitions),
                reconstruct_verify_ms: optional_median(&self.reconstruct_verify_ms, repetitions),
                prefix_fold_ms: optional_median(&self.prefix_fold_ms, repetitions),
                cubic_tail_ms: optional_median(&self.cubic_tail_ms, repetitions),
                matrix_bind_ms: median(&self.matrix_bind_ms),
                inner_sumcheck_ms: median(&self.inner_sumcheck_ms),
            },
            shape: self.shape.expect("at least one measured sample"),
        }
    }
}

fn push_optional(samples: &mut Vec<f64>, value: Option<f64>) {
    if let Some(value) = value {
        samples.push(value);
    }
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    sorted[sorted.len() / 2]
}

fn optional_median(samples: &[f64], repetitions: usize) -> Option<f64> {
    if samples.is_empty() {
        None
    } else {
        assert_eq!(samples.len(), repetitions);
        Some(median(samples))
    }
}

fn optional_ms(value: Option<f64>) -> String {
    value.map_or_else(|| "NA".to_owned(), |value| format!("{value:.9}"))
}

fn phase_ms(phases: &[(String, f64)], label: &str) -> Option<f64> {
    phases
        .iter()
        .find(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
}

fn phase_timings(
    protocol: Protocol,
    prove_phases: &[(String, f64)],
    verify_phases: &[(String, f64)],
) -> PhaseTimings {
    let outer_label = match protocol {
        Protocol::Standard => "spartan:outer_sumcheck",
        Protocol::Skip(_) => "spartan:outer_univariate_skip",
    };
    let timings = PhaseTimings {
        outer_total_ms: phase_ms(prove_phases, outer_label)
            .expect("outer aggregate phase is instrumented"),
        skip_message_ms: phase_ms(prove_phases, "spartan:univariate_skip_message"),
        reconstruct_prove_ms: phase_ms(prove_phases, "spartan:univariate_skip_reconstruct"),
        reconstruct_verify_ms: phase_ms(verify_phases, "spartan:univariate_skip_reconstruct"),
        prefix_fold_ms: phase_ms(prove_phases, "spartan:univariate_skip_prefix_fold"),
        cubic_tail_ms: phase_ms(prove_phases, "spartan:univariate_skip_tail"),
        matrix_bind_ms: phase_ms(prove_phases, "spartan:bind_and_batch")
            .expect("matrix-binding phase is instrumented"),
        inner_sumcheck_ms: phase_ms(prove_phases, "spartan:inner_sumcheck")
            .expect("inner-sumcheck phase is instrumented"),
    };

    match protocol {
        Protocol::Standard => {
            assert!(timings.skip_message_ms.is_none());
            assert!(timings.reconstruct_prove_ms.is_none());
            assert!(timings.reconstruct_verify_ms.is_none());
            assert!(timings.prefix_fold_ms.is_none());
            assert!(timings.cubic_tail_ms.is_none());
        }
        Protocol::Skip(_) => {
            assert!(timings.skip_message_ms.is_some());
            assert!(timings.reconstruct_prove_ms.is_some());
            assert!(timings.reconstruct_verify_ms.is_some());
            assert!(timings.prefix_fold_ms.is_some());
            assert!(timings.cubic_tail_ms.is_some());
        }
    }
    timings
}

fn standard_proof_shape(
    proof: &SpartanPiopProof<SpartanBitzField>,
    row_vars: usize,
    column_vars: usize,
) -> ProofShape {
    assert_eq!(proof.outer.sumcheck.round_polynomials.len(), row_vars);
    assert_eq!(proof.inner.round_polynomials.len(), column_vars);
    let outer_fields = 4 * proof.outer.sumcheck.round_polynomials.len() + 3;
    let proof_fields = outer_fields + 3 * proof.inner.round_polynomials.len();
    assert_eq!(outer_fields, 4 * row_vars + 3);
    assert_eq!(proof_fields, 4 * row_vars + 3 + 3 * column_vars);
    ProofShape {
        outer_fields,
        proof_fields,
    }
}

fn skip_proof_shape(
    proof: &UnivariateSkipSpartanPiopProof<SpartanBitzField>,
    row_vars: usize,
    column_vars: usize,
    skip_vars: usize,
) -> ProofShape {
    assert_eq!(proof.outer.skip.skip_vars, skip_vars as u8);
    assert_eq!(
        proof.outer.skip.finite_q_evaluations.len(),
        (1usize << skip_vars) - 2
    );
    assert_eq!(
        proof.outer.tail.sumcheck.round_polynomials.len(),
        row_vars - skip_vars
    );
    assert_eq!(proof.inner.round_polynomials.len(), column_vars);

    let skip_fields = proof.outer.skip.finite_q_evaluations.len() + 1;
    let tail_fields = 4 * proof.outer.tail.sumcheck.round_polynomials.len() + 3;
    let outer_fields = skip_fields + tail_fields;
    let proof_fields = outer_fields + 3 * proof.inner.round_polynomials.len();
    let expected_outer_fields = 4 * row_vars - 4 * skip_vars + (1usize << skip_vars) + 2;
    assert_eq!(outer_fields, expected_outer_fields);
    assert_eq!(proof_fields, expected_outer_fields + 3 * column_vars);
    ProofShape {
        outer_fields,
        proof_fields,
    }
}

fn run_latency_trial(
    protocol: Protocol,
    relation: &PreparedConstraintMatrices<SpartanBitzField, bool>,
    products: &R1csProductMles<u64>,
    assignment: &DenseMultilinearExtension<u64>,
) -> LatencyTrial {
    let sample_products = products.clone();
    let sample_assignment = assignment.clone();
    let recording = bitz::observability::Recording::start(Vec::new()).expect("start outer-policy trial");

    match protocol {
        Protocol::Standard => {
            let mut prover_transcript = Blake3Transcript::new();
            let proving = tracing::info_span!("benchmark:proving").entered();
            let (proof, claim) = prove_spartan_piop_u32_native(
                &mut prover_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                sample_products,
                sample_assignment,
            )
            .expect("standard proving succeeds");
            drop(proving);

            let mut verifier_transcript = Blake3Transcript::new();
            let verification = tracing::info_span!("benchmark:verification").entered();
            let verified_claim = verify_spartan_proof(
                &mut verifier_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                &proof,
            )
            .expect("standard verification succeeds");
            drop(verification);
            let intervals = recording.intervals().expect("query outer-policy trial");
            let prove_ms = common::span_ms(&intervals, "benchmark:proving");
            let verify_ms = common::span_ms(&intervals, "benchmark:verification");
            let prove_phases = bitz::observability::phase_totals(&intervals, "benchmark:proving").unwrap();
            let verify_phases = bitz::observability::phase_totals(&intervals, "benchmark:verification").unwrap();
            assert_eq!(claim, verified_claim);

            let phases = phase_timings(protocol, &prove_phases, &verify_phases);
            let shape =
                standard_proof_shape(&proof, relation.num_row_vars(), relation.num_column_vars());
            black_box(&proof);
            LatencyTrial {
                prove_ms,
                verify_ms,
                phases,
                shape,
            }
        }
        Protocol::Skip(skip_vars) => {
            let mut prover_transcript = Blake3Transcript::new();
            let proving = tracing::info_span!("benchmark:proving").entered();
            let (proof, claim) = prove_spartan_piop_u32_native_with_univariate_skip(
                &mut prover_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                sample_products,
                sample_assignment,
                skip_vars,
            )
            .expect("univariate-skip proving succeeds");
            drop(proving);

            let mut verifier_transcript = Blake3Transcript::new();
            let verification = tracing::info_span!("benchmark:verification").entered();
            let verified_claim = verify_spartan_univariate_skip_proof(
                &mut verifier_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                &proof,
            )
            .expect("univariate-skip verification succeeds");
            drop(verification);
            let intervals = recording.intervals().expect("query outer-policy trial");
            let prove_ms = common::span_ms(&intervals, "benchmark:proving");
            let verify_ms = common::span_ms(&intervals, "benchmark:verification");
            let prove_phases = bitz::observability::phase_totals(&intervals, "benchmark:proving").unwrap();
            let verify_phases = bitz::observability::phase_totals(&intervals, "benchmark:verification").unwrap();
            assert_eq!(claim, verified_claim);

            let phases = phase_timings(protocol, &prove_phases, &verify_phases);
            let shape = skip_proof_shape(
                &proof,
                relation.num_row_vars(),
                relation.num_column_vars(),
                skip_vars,
            );
            black_box(&proof);
            LatencyTrial {
                prove_ms,
                verify_ms,
                phases,
                shape,
            }
        }
    }
}

fn run_memory_trial(
    protocol: Protocol,
    relation: &PreparedConstraintMatrices<SpartanBitzField, bool>,
    products: &R1csProductMles<u64>,
    assignment: &DenseMultilinearExtension<u64>,
) -> MemoryTrial {
    let sample_products = products.clone();
    let sample_assignment = assignment.clone();

    let live_before_prove_mib = live_mib();
    reset_peak();

    let (peak_heap_mib, shape) = match protocol {
        Protocol::Standard => {
            let mut prover_transcript = Blake3Transcript::new();
            let (proof, claim) = prove_spartan_piop_u32_native(
                &mut prover_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                sample_products,
                sample_assignment,
            )
            .expect("standard peak-memory proving succeeds");
            let peak_heap_mib = peak_mib();

            let mut verifier_transcript = Blake3Transcript::new();
            let verified_claim = verify_spartan_proof(
                &mut verifier_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                &proof,
            )
            .expect("standard peak-memory verification succeeds");

            assert_eq!(claim, verified_claim);
            let shape =
                standard_proof_shape(&proof, relation.num_row_vars(), relation.num_column_vars());
            black_box(&proof);
            (peak_heap_mib, shape)
        }
        Protocol::Skip(skip_vars) => {
            let mut prover_transcript = Blake3Transcript::new();
            let (proof, claim) = prove_spartan_piop_u32_native_with_univariate_skip(
                &mut prover_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                sample_products,
                sample_assignment,
                skip_vars,
            )
            .expect("univariate-skip peak-memory proving succeeds");
            let peak_heap_mib = peak_mib();

            let mut verifier_transcript = Blake3Transcript::new();
            let verified_claim = verify_spartan_univariate_skip_proof(
                &mut verifier_transcript,
                relation,
                &ASSIGNMENT_BINDING,
                &proof,
            )
            .expect("univariate-skip peak-memory verification succeeds");

            assert_eq!(claim, verified_claim);
            let shape = skip_proof_shape(
                &proof,
                relation.num_row_vars(),
                relation.num_column_vars(),
                skip_vars,
            );
            black_box(&proof);
            (peak_heap_mib, shape)
        }
    };

    MemoryTrial {
        live_before_prove_mib,
        peak_heap_mib,
        shape,
    }
}

fn exponents() -> Vec<usize> {
    common::shape_values(None, clap::builder::RangedU64ValueParser::<usize>::new().range(8..=25))
        .unwrap_or_else(|| vec![DEFAULT_EXPONENT])
}

fn rayon_threads() -> usize {
    #[cfg(feature = "parallel")]
    {
        rayon::current_num_threads()
    }
    #[cfg(not(feature = "parallel"))]
    {
        1
    }
}

fn print_latency_sample(
    protocol: Protocol,
    exponent: usize,
    multiplications: usize,
    sample: usize,
    trial: LatencyTrial,
) {
    println!(
        "SAMPLE pass=latency protocol={} skip_vars={} exponent={exponent} multiplications={multiplications} sample={sample} prove_ms={:.9} outer_total_ms={:.9} skip_message_ms={} reconstruct_prove_ms={} prefix_fold_ms={} cubic_tail_ms={} matrix_bind_ms={:.9} inner_sumcheck_ms={:.9} verify_ms={:.9} reconstruct_verify_ms={} outer_proof_fields={} proof_fields={} outer_proof_field_bytes={} proof_field_bytes={} verified=true",
        protocol.name(),
        protocol.skip_vars(),
        trial.prove_ms,
        trial.phases.outer_total_ms,
        optional_ms(trial.phases.skip_message_ms),
        optional_ms(trial.phases.reconstruct_prove_ms),
        optional_ms(trial.phases.prefix_fold_ms),
        optional_ms(trial.phases.cubic_tail_ms),
        trial.phases.matrix_bind_ms,
        trial.phases.inner_sumcheck_ms,
        trial.verify_ms,
        optional_ms(trial.phases.reconstruct_verify_ms),
        trial.shape.outer_fields,
        trial.shape.proof_fields,
        trial.shape.outer_fields * FIELD_ELEMENT_BYTES,
        trial.shape.proof_fields * FIELD_ELEMENT_BYTES,
    );
}

#[allow(clippy::too_many_arguments)]
fn print_latency_result(
    protocol: Protocol,
    exponent: usize,
    multiplications: usize,
    relation: &PreparedConstraintMatrices<SpartanBitzField, bool>,
    integer_witness_values: usize,
    repetitions: usize,
    order: usize,
    shape_seed: u64,
    threads: usize,
    medians: ProtocolMedians,
) {
    println!(
        "RESULT pass=latency order={order} protocol={} skip_vars={} exponent={exponent} multiplications={multiplications} row_vars={} column_vars={} integer_witness_values={integer_witness_values} parallel={} rayon_threads={threads} prove_ms={:.9} outer_total_ms={:.9} skip_message_ms={} reconstruct_prove_ms={} prefix_fold_ms={} cubic_tail_ms={} matrix_bind_ms={:.9} inner_sumcheck_ms={:.9} verify_ms={:.9} reconstruct_verify_ms={} outer_proof_fields={} proof_fields={} outer_proof_field_bytes={} proof_field_bytes={} repetitions={repetitions} warmups=1 shape_seed={shape_seed:#018x} verified_samples={repetitions}",
        protocol.name(),
        protocol.skip_vars(),
        relation.num_row_vars(),
        relation.num_column_vars(),
        cfg!(feature = "parallel"),
        medians.prove_ms,
        medians.phases.outer_total_ms,
        optional_ms(medians.phases.skip_message_ms),
        optional_ms(medians.phases.reconstruct_prove_ms),
        optional_ms(medians.phases.prefix_fold_ms),
        optional_ms(medians.phases.cubic_tail_ms),
        medians.phases.matrix_bind_ms,
        medians.phases.inner_sumcheck_ms,
        medians.verify_ms,
        optional_ms(medians.phases.reconstruct_verify_ms),
        medians.shape.outer_fields,
        medians.shape.proof_fields,
        medians.shape.outer_fields * FIELD_ELEMENT_BYTES,
        medians.shape.proof_fields * FIELD_ELEMENT_BYTES,
    );
}

#[allow(clippy::too_many_arguments)]
fn print_memory_result(
    protocol: Protocol,
    exponent: usize,
    multiplications: usize,
    relation: &PreparedConstraintMatrices<SpartanBitzField, bool>,
    integer_witness_values: usize,
    order: usize,
    shape_seed: u64,
    threads: usize,
    trial: MemoryTrial,
) {
    let peak_delta_mib = (trial.peak_heap_mib - trial.live_before_prove_mib).max(0.0);
    println!(
        "MEMORY pass=memory order={order} protocol={} skip_vars={} exponent={exponent} multiplications={multiplications} live_before_prove_mib={:.9} peak_heap_mib={:.9} peak_delta_mib={peak_delta_mib:.9} verified=true",
        protocol.name(),
        protocol.skip_vars(),
        trial.live_before_prove_mib,
        trial.peak_heap_mib,
    );
    println!(
        "RESULT pass=memory order={order} protocol={} skip_vars={} exponent={exponent} multiplications={multiplications} row_vars={} column_vars={} integer_witness_values={integer_witness_values} parallel={} rayon_threads={threads} live_before_prove_mib={:.9} peak_heap_mib={:.9} peak_delta_mib={peak_delta_mib:.9} outer_proof_fields={} proof_fields={} outer_proof_field_bytes={} proof_field_bytes={} warmups=1 shape_seed={shape_seed:#018x} verified_samples=1",
        protocol.name(),
        protocol.skip_vars(),
        relation.num_row_vars(),
        relation.num_column_vars(),
        cfg!(feature = "parallel"),
        trial.live_before_prove_mib,
        trial.peak_heap_mib,
        trial.shape.outer_fields,
        trial.shape.proof_fields,
        trial.shape.outer_fields * FIELD_ELEMENT_BYTES,
        trial.shape.proof_fields * FIELD_ELEMENT_BYTES,
    );
}

#[allow(clippy::too_many_arguments)]
fn bench_exponent(
    exponent: usize,
    repetitions: usize,
    root_seed: u64,
    pass: BenchmarkPass,
    order: usize,
    threads: usize,
) {
    let multiplications = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("multiplication domain fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);
    let witness = U32MulWitness::from_fn(multiplications, |_| {
        (rng.random::<u32>(), rng.random::<u32>())
    })
    .expect("valid u32 multiplication witness");
    let layout = *witness.layout();
    let field_config = spartan_bitz_field_config();
    let relation = prepare_u32_mul_relation(layout, &field_config).expect("valid relation");
    let (assignment, products) = project_u32_mul_native_witness(&witness).into_parts();

    for protocol in Protocol::ALL {
        let warmup = run_latency_trial(protocol, &relation, &products, &assignment);
        black_box(warmup);
    }

    if pass.measures_latency() {
        let mut metrics: [ProtocolSamples; 5] = std::array::from_fn(|_| ProtocolSamples::default());
        for sample in 0..repetitions {
            for offset in 0..Protocol::ALL.len() {
                let protocol_index = (order - 1 + sample + offset) % Protocol::ALL.len();
                let protocol = Protocol::ALL[protocol_index];
                let trial = run_latency_trial(protocol, &relation, &products, &assignment);
                print_latency_sample(protocol, exponent, multiplications, sample, trial);
                metrics[protocol.index()].push(trial);
            }
        }

        for protocol in Protocol::ALL {
            print_latency_result(
                protocol,
                exponent,
                multiplications,
                &relation,
                layout.assignment_len(),
                repetitions,
                order,
                shape_seed,
                threads,
                metrics[protocol.index()].medians(repetitions),
            );
        }
    }

    if pass.measures_memory() {
        for offset in 0..Protocol::ALL.len() {
            let protocol = Protocol::ALL[(order - 1 + offset) % Protocol::ALL.len()];
            flock_core::scratch::clear();
            let trial = run_memory_trial(protocol, &relation, &products, &assignment);
            print_memory_result(
                protocol,
                exponent,
                multiplications,
                &relation,
                layout.assignment_len(),
                order,
                shape_seed,
                threads,
                trial,
            );
        }
    }
}

fn main() {
    common::cli::EnvironmentCli::parse();
    let repetitions = common::reps(None, DEFAULT_REPETITIONS);
    let pass = BenchmarkPass::from_env();
    let order = common::cli::env::<std::num::NonZeroUsize>("BITZ_BENCH_ORDER")
        .map_or(1, std::num::NonZeroUsize::get);
    let root_seed = common::seed(None, 0x5533_326d_756c_0073);

    let exponents = exponents();
    bitz::observability::install().expect("install Perfetto subscriber");
    common::enforce_known_env();
    let _ = flock_core::init_perf_thread_pool();
    let threads = rayon_threads();

    println!("u32 outer-sumcheck univariate-skip benchmark");
    println!("rayon threads: {threads}");
    println!(
        "protocols: standard, skip-k1, skip-k2, skip-k3, skip-k4; pass: {}; repetitions: {repetitions}; warmups: 1; order: {order}; root seed: {root_seed:#018x}",
        pass.as_str(),
    );

    for exponent in exponents {
        flock_core::scratch::clear();
        bench_exponent(exponent, repetitions, root_seed, pass, order, threads);
    }
    flock_core::scratch::clear();
}
