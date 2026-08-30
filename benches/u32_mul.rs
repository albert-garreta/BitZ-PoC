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
//! excluded and reported one-time. The default profile is `Lambda100`.
//! The univariate-skip protocol variants still run the LEGACY fixed-q
//! path (`q = 2^100 - 15`, `lambda=na`).
//!
//! Defaults to the production sweep `2^15, ..., 2^25` multiplications.
//! Override with `F2Z_BENCH_SHAPES` (deprecated alias `F2Z_MUL_EXPONENTS`):
//!
//! ```text
//! F2Z_BENCH_SHAPES="15 17 19" F2Z_BENCH_REPS=3 \
//!   cargo bench --bench u32_mul --features unchecked
//! ```
//!
//! `F2Z_BENCH_SHAPES=15 F2Z_BENCH_REPS=1` is the smallest production smoke
//! shape. `F2Z_SPARTAN_REDUCTION` selects `immediate`, `delayed-barrett`, or
//! `delayed-crypto-bigint`; the production default is `delayed-barrett`.
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
use f2z::pcs::{FQ_BITS, mod_q_num_chunks};
use f2z::piop::spartan::{
    PreparedConstraintMatrices, PreparedU32MulRelation, SpartanF2zError, SpartanF2zField,
    SpartanReductionStrategy, U32MulF2zWidth, U32MulLayout, U32MulPaperProof,
    U32MulUnivariateSkipSpartanF2zProof, U32MulWitness, commit_u32_mul_witness,
    prepare_u32_mul_relation, prove_u32_mul_paper,
    prove_u32_mul_spartan_and_f2z_with_univariate_skip, spartan_f2z_field_config,
    verify_u32_mul_paper, verify_u32_mul_spartan_and_f2z_with_univariate_skip,
};
use f2z::transcript::Blake3Transcript;
use f2z::{ligerito::packed_vars, ligerito_flock::sha_lig_configs};
use flock_core::pcs::commit::Commitment;
use rand::{Rng, RngExt, SeedableRng, rngs::StdRng};

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

fn reduction_strategy() -> SpartanReductionStrategy {
    match std::env::var("F2Z_SPARTAN_REDUCTION").as_deref() {
        Ok("immediate") => SpartanReductionStrategy::Immediate,
        Ok("delayed-barrett") | Err(_) => SpartanReductionStrategy::DelayedBarrett,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OuterProtocol {
    Standard,
    UnivariateSkip(usize),
}

impl OuterProtocol {
    fn from_env() -> Self {
        match env_usize("F2Z_SPARTAN_OUTER_SKIP", 0) {
            0 => Self::Standard,
            skip_vars @ 1..=4 => Self::UnivariateSkip(skip_vars),
            value => panic!("F2Z_SPARTAN_OUTER_SKIP must be 0 or an integer in 1..=4; got {value}"),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::UnivariateSkip(1) => "skip-k1",
            Self::UnivariateSkip(2) => "skip-k2",
            Self::UnivariateSkip(3) => "skip-k3",
            Self::UnivariateSkip(4) => "skip-k4",
            Self::UnivariateSkip(_) => unreachable!(),
        }
    }

    const fn skip_vars(self) -> usize {
        match self {
            Self::Standard => 0,
            Self::UnivariateSkip(skip_vars) => skip_vars,
        }
    }
}

enum BenchProof {
    Paper(U32MulPaperProof),
    UnivariateSkip(U32MulUnivariateSkipSpartanF2zProof),
}

impl BenchProof {
    fn f2z_bytes(&self) -> usize {
        match self {
            Self::Paper(proof) => proof.f2z().to_bytes().len(),
            Self::UnivariateSkip(proof) => proof.f2z().to_bytes().len(),
        }
    }

    fn spartan_payload_elements(&self) -> usize {
        match self {
            Self::Paper(proof) => proof.spartan_payload_elements(),
            Self::UnivariateSkip(proof) => {
                proof.spartan().outer.skip.finite_q_evaluations.len()
                    + 1
                    + 4 * proof.spartan().outer.tail.sumcheck.round_polynomials.len()
                    + 3
                    + 3 * proof.spartan().inner.round_polynomials.len()
            }
        }
    }
}

/// The prepared relation of whichever path the protocol knob selects: the
/// paper runtime-prime relation (standard outer) or the legacy fixed-q
/// matrices (univariate-skip variants).
enum BenchRelation {
    Paper(PreparedU32MulRelation),
    Legacy(PreparedConstraintMatrices<SpartanF2zField, bool>),
}

fn prove_combined(
    transcript: &mut Blake3Transcript,
    relation: &BenchRelation,
    layout: &U32MulLayout,
    witness: &U32MulWitness,
    commitment_hint: &FlockCommitHint,
    strategy: SpartanReductionStrategy,
    protocol: OuterProtocol,
) -> Result<BenchProof, SpartanF2zError> {
    match relation {
        BenchRelation::Paper(prepared) => {
            prove_u32_mul_paper(transcript, prepared, witness, commitment_hint, strategy)
                .map(BenchProof::Paper)
        }
        BenchRelation::Legacy(matrices) => {
            assert_eq!(
                strategy,
                SpartanReductionStrategy::DelayedBarrett,
                "univariate skip uses the production delayed-Barrett reducer"
            );
            let OuterProtocol::UnivariateSkip(skip_vars) = protocol else {
                unreachable!("the legacy relation is built only for skip variants")
            };
            prove_u32_mul_spartan_and_f2z_with_univariate_skip(
                transcript,
                matrices,
                layout,
                witness,
                commitment_hint,
                skip_vars,
            )
            .map(BenchProof::UnivariateSkip)
        }
    }
}

fn verify_combined(
    transcript: &mut Blake3Transcript,
    relation: &BenchRelation,
    layout: &U32MulLayout,
    commitment: &Commitment,
    proof: &BenchProof,
) -> Result<(), SpartanF2zError> {
    match (relation, proof) {
        (BenchRelation::Paper(prepared), BenchProof::Paper(proof)) => {
            verify_u32_mul_paper(transcript, prepared, commitment, proof)
        }
        (BenchRelation::Legacy(matrices), BenchProof::UnivariateSkip(proof)) => {
            verify_u32_mul_spartan_and_f2z_with_univariate_skip(
                transcript, matrices, layout, commitment, proof,
            )
        }
        _ => unreachable!("relation and proof kinds always match"),
    }
}

fn exponents() -> Vec<usize> {
    common::shapes(Some("F2Z_MUL_EXPONENTS")).map_or_else(
        || (15..=25).collect(),
        |shapes| {
            shapes
                .iter()
                .map(|part| {
                    let exponent: usize =
                        part.parse().expect("F2Z_BENCH_SHAPES contains integers");
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
#[allow(clippy::too_many_arguments)]
fn prove_e2e(
    relation: &BenchRelation,
    layout: &U32MulLayout,
    witness: &U32MulWitness,
    strategy: SpartanReductionStrategy,
    protocol: OuterProtocol,
) -> (BenchProof, FlockCommitHint, f64, f64) {
    let prove_started = Instant::now();
    let commit_started = Instant::now();
    let bit_rows = witness.f2z_bit_rows();
    let commitment_hint =
        commit_u32_mul_witness(layout, bit_rows).expect("F2Z commitment succeeds");
    let commit_ms = common::elapsed_ms(commit_started);

    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_combined(
        &mut prover_transcript,
        relation,
        layout,
        witness,
        &commitment_hint,
        strategy,
        protocol,
    )
    .expect("combined proving succeeds");
    let prove_ms = common::elapsed_ms(prove_started);
    (proof, commitment_hint, prove_ms, commit_ms)
}

#[allow(clippy::too_many_arguments)]
fn bench_exponent(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    pass: BenchmarkPass,
    strategy: SpartanReductionStrategy,
    protocol: OuterProtocol,
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
    let f2z_chunks = mod_q_num_chunks(&params, FQ_BITS);

    // One-time public preprocessing (excluded from prove). The standard
    // protocol prepares the paper runtime-prime relation (q-independent
    // exact matrices + the instantiated security profile); the skip
    // variants prepare the legacy fixed-q matrices.
    let started = Instant::now();
    let relation = match protocol {
        OuterProtocol::Standard => BenchRelation::Paper(
            PreparedU32MulRelation::new(layout).expect("valid paper relation"),
        ),
        OuterProtocol::UnivariateSkip(_) => {
            let field_config = spartan_f2z_field_config();
            BenchRelation::Legacy(
                prepare_u32_mul_relation::<SpartanF2zField>(layout, &field_config)
                    .expect("valid relation"),
            )
        }
    };
    let setup_ms = common::elapsed_ms(started);

    // Excluded warm-up. This is also the first end-to-end correctness check.
    let (warm_proof, warm_hint, _, _) =
        prove_e2e(&relation, &layout, &witness, strategy, protocol);
    let mut verifier_transcript = Blake3Transcript::new();
    verify_combined(
        &mut verifier_transcript,
        &relation,
        &layout,
        &warm_hint.commitment,
        &warm_proof,
    )
    .expect("warm-up combined verification succeeds");
    drop(warm_proof);
    drop(warm_hint);
    let _ = f2z::utils::prof::take_totals();

    let strategy_label = strategy_name(strategy);
    let mut latency = None;
    if pass.measures_latency() {
        let mut prover = common::StepSamples::default();
        let mut verifier = common::StepSamples::default();
        let mut last_proof = None;
        for sample_index in 0..reps {
            let _ = f2z::utils::prof::take_totals();
            let (proof, commitment_hint, prove_ms, commit_ms) =
                prove_e2e(&relation, &layout, &witness, strategy, protocol);
            let prove_phases = f2z::utils::prof::take_totals();

            let mut verifier_transcript = Blake3Transcript::new();
            let started = Instant::now();
            verify_combined(
                &mut verifier_transcript,
                &relation,
                &layout,
                &commitment_hint.commitment,
                &proof,
            )
            .expect("combined verification succeeds");
            let verify_ms = common::elapsed_ms(started);
            let verify_phases = f2z::utils::prof::take_totals();

            println!(
                "  SAMPLE exponent={exponent} sample={} multiplications={multiplications} commit_ms={commit_ms:.6} prove_ms={prove_ms:.6} verify_ms={verify_ms:.6} verified=true",
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
        let (peak_proof, peak_hint, _, _) =
            prove_e2e(&relation, &layout, &witness, strategy, protocol);
        black_box(&peak_proof);
        let peak = peak_mib();
        let _ = f2z::utils::prof::take_totals();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_combined(
            &mut verifier_transcript,
            &relation,
            &layout,
            &peak_hint.commitment,
            &peak_proof,
        )
        .expect("peak-memory proof verifies");
        let _ = f2z::utils::prof::take_totals();
        println!(
            "  MEMORY pass=memory order={order} protocol={} skip_vars={} strategy={strategy_label} word_bits={} exponent={exponent} multiplications={multiplications} peak_heap_mib={peak:.6} live_before_prove_mib={live_before_prove:.6} verified=true",
            protocol.name(),
            protocol.skip_vars(),
            params.word_bits,
        );
        memory_metrics = Some((live_before_prove, peak));
    }

    let (lig_pc, _) = sha_lig_configs(packed_vars(&params)).expect("production Ligerito config");
    println!();
    println!(
        "u32_mul gates=2^{exponent} ({multiplications}) W={} [{}]  seed={shape_seed:#018x}",
        params.word_bits,
        protocol.name(),
    );
    println!(
        "  benchmark: pass={} order={order} protocol={} skip_vars={} strategy={strategy_label} t={} s={} chunks={f2z_chunks}",
        pass.as_str(),
        protocol.name(),
        protocol.skip_vars(),
        params.t,
        params.s,
    );
    println!(
        "  R1CS: rows={} cols={} nnz={}  |  F2Z: t={} s={} W={} chunks={} bits={} ({:.2} MiB)",
        multiplications,
        4 * layout.capacity(),
        3 * multiplications,
        params.t,
        params.s,
        params.word_bits,
        f2z_chunks,
        128usize * layout.capacity(),
        (16usize * layout.capacity()) as f64 / (1024.0 * 1024.0),
    );
    println!(
        "  Ligerito: inverse rate=2^{} initial_k={} hash={:?}",
        lig_pc.log_inv_rates[0], lig_pc.initial_k, lig_pc.merkle_hash,
    );
    if let Some((prover, verifier, last_proof)) = latency {
        let spartan_elements = last_proof.spartan_payload_elements();
        let report = common::BenchReport {
            bench: "u32_mul",
            shape: format!("2p{exponent}"),
            extra: vec![
                ("multiplications".into(), multiplications.to_string()),
                ("protocol".into(), protocol.name().into()),
                ("skip_vars".into(), protocol.skip_vars().to_string()),
                ("strategy".into(), strategy_label.into()),
                ("word_bits".into(), params.word_bits.to_string()),
                ("f2z_t".into(), params.t.to_string()),
                ("f2z_s".into(), params.s.to_string()),
                ("f2z_chunks".into(), f2z_chunks.to_string()),
                ("order".into(), order.to_string()),
                ("shape_seed".into(), format!("{shape_seed:#018x}")),
            ],
            lambda: match &relation {
                BenchRelation::Paper(prepared) => Some(prepared.security().lambda),
                BenchRelation::Legacy(_) => None,
            },
            lambda_achieved: match &relation {
                BenchRelation::Paper(prepared) => {
                    Some(prepared.security().accounting.achieved_bits())
                }
                BenchRelation::Legacy(_) => None,
            },
            lambda_bind: match &relation {
                BenchRelation::Paper(prepared) => {
                    Some(prepared.security().accounting.binding_term().name.into())
                }
                BenchRelation::Legacy(_) => None,
            },
            threads,
            reps,
            seed: Some(root_seed),
            witness_ms,
            setup_ms,
            prover: prover.medians(),
            verifier: verifier.medians(),
            proof: common::ProofBytes {
                piop: spartan_elements * 16,
                open: last_proof.f2z_bytes(),
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
    let strategy = reduction_strategy();
    let protocol = OuterProtocol::from_env();
    let f2z_width = f2z_width();
    let order = env_usize("F2Z_BENCH_ORDER", 1);
    assert!(order > 0, "F2Z_BENCH_ORDER must be positive");
    let seed = common::seed(Some("F2Z_MUL_SEED"), 0x5533_326d_756c_0064);

    println!("u32 × u32 → u64: Spartan PIOP + F2Z assignment opening");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!(
        "repetitions: {reps}; root seed: {seed:#018x}; F2Z word bits: {}",
        f2z_width.word_bits(),
    );
    println!(
        "benchmark pass: {}; protocol: {} (skip_vars={}); strategy: {}; order: {order}",
        pass.as_str(),
        protocol.name(),
        protocol.skip_vars(),
        strategy_name(strategy),
    );

    for exponent in exponents() {
        flock_core::scratch::clear();
        bench_exponent(
            exponent, reps, seed, pass, strategy, protocol, f2z_width, order, threads,
        );
    }
    flock_core::scratch::clear();
}
