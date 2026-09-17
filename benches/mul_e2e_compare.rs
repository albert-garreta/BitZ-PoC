//! Native end-to-end multiplication proofs. See docs/native-mul-compare.md.
use f2z::piop::spartan::mul::{MulLayout, MulWitness};
#[path = "mul_e2e_compare/binius.rs"]
mod binius;
#[path = "mul_e2e_compare/binius_ligerito.rs"]
mod binius_ligerito;
mod common;
#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static HEAP_ALLOCATOR: common::peak_memory::PeakAlloc = common::peak_memory::PeakAlloc;

use common::output::{BenchmarkOutput, FileMode, JsonStyle};
#[path = "mul_e2e_compare/f2z.rs"]
mod f2z_backend;
#[path = "mul_e2e_compare/limber.rs"]
mod limber;
#[path = "mul_e2e_compare/memory.rs"]
mod memory;
#[path = "mul_e2e_compare/report.rs"]
mod report;
use report::{Medians, Metrics};
#[path = "mul_e2e_compare/mod32.rs"]
mod mod32;
#[path = "mul_e2e_compare/mod32_air.rs"]
mod mod32_air;
#[path = "mul_e2e_compare/plonky3.rs"]
mod plonky3;
#[path = "mul_e2e_compare/plonky3_whir.rs"]
mod plonky3_whir;
#[path = "common/trace_capture.rs"]
mod trace_capture;

use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use trace_capture::CapturedSpan;

const MEASUREMENT_POLICY: &str = "warm-process/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Workload {
    #[value(name = "u32-mod32", alias = "u32")]
    U32,
    U64,
    U128,
}
impl Workload {
    fn slug(self) -> &'static str {
        match self {
            Self::U32 => "u32-mod32",
            Self::U64 => "u64",
            Self::U128 => "u128",
        }
    }
    fn algorithm(self) -> &'static str {
        match self {
            Self::U32 => "independent multiplication modulo 2^32",
            Self::U64 => "u64 multiplication",
            Self::U128 => "u128 multiplication",
        }
    }
    /// Backends with a native arithmetization of this workload.
    fn supports(self, backend: Backend) -> bool {
        // Plonky3's AIR only decomposes 32-bit operands. The other adapters
        // also support the u64 and u128 relations.
        self == Self::U32 || !matches!(backend, Backend::Plonky3Fri | Backend::Plonky3Whir)
    }
    /// Whether the operands are 128-bit values (the `u128` workload) rather
    /// than `u64` values.
    fn is_wide(self) -> bool {
        self == Self::U128
    }
    /// The exact integer output of one gate (`a*b`, or `a*b mod p`) of a
    /// 64-bit-or-narrower workload.
    fn output(self, a: u64, b: u64) -> u128 {
        let product = u128::from(a) * u128::from(b);
        match self {
            Self::U32 => product & u128::from(u32::MAX),
            Self::U64 => product,
            Self::U128 => panic!("the u128 workload has 128-bit operands"),
        }
    }
    /// The four native witness values of one `u128` gate: operands, then the
    /// low and high 128-bit halves of the exact 256-bit product.
    fn wide_row(self, x: u128, y: u128) -> [u128; 4] {
        assert!(self.is_wide(), "{} operands are u64 values", self.slug());
        let (lo, hi) = f2z::piop::spartan::mul_u128_full(x, y);
        [x, y, lo, hi]
    }
    /// The four native witness values of one gate: operands, then the
    /// output (`c` or `z_lo`) and the auxiliary value (`k`, `z_hi`, or 0).
    fn native_row(self, a: u64, b: u64) -> [u64; 4] {
        let output = self.output(a, b);
        match self {
            Self::U32 => [a, b, output as u64, (a * b) >> 32],
            Self::U64 => [a, b, output as u64, (output >> 64) as u64],
            Self::U128 => panic!("the u128 workload has 128-bit operands"),
        }
    }
}

/// The operand pairs of one corpus: `u64` values for the 64-bit-or-narrower
/// workloads, `u128` values for the `u128` workload.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Operands {
    Narrow(Vec<(u64, u64)>),
    Wide(Vec<(u128, u128)>),
}

struct Corpus {
    workload: Workload,
    operands: Operands,
    digest: String,
}
impl Corpus {
    fn new(workload: Workload, exponent: usize, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        if workload.is_wide() {
            let inputs = (0..1usize << exponent)
                .map(|_| (rng.random::<u128>(), rng.random::<u128>()))
                .collect();
            return Self::from_wide_inputs(workload, inputs);
        }
        let inputs = if workload == Workload::U32 {
            mod32::inputs(exponent, seed)
        } else {
            (0..1usize << exponent)
                .map(|_| (rng.random::<u64>(), rng.random::<u64>()))
                .collect()
        };
        Self::from_inputs(workload, inputs)
    }
    /// Number of multiplications.
    fn len(&self) -> usize {
        match &self.operands {
            Operands::Narrow(inputs) => inputs.len(),
            Operands::Wide(inputs) => inputs.len(),
        }
    }
    /// The operand pairs of a 64-bit-or-narrower workload.
    fn inputs(&self) -> &[(u64, u64)] {
        match &self.operands {
            Operands::Narrow(inputs) => inputs,
            Operands::Wide(_) => {
                panic!("the {} workload has 128-bit operands", self.workload.slug())
            }
        }
    }
    /// The operand pairs of the `u128` workload.
    fn wide_inputs(&self) -> &[(u128, u128)] {
        match &self.operands {
            Operands::Wide(inputs) => inputs,
            Operands::Narrow(_) => {
                panic!("the {} workload has 64-bit operands", self.workload.slug())
            }
        }
    }
    /// The operand pairs of a 32-bit workload as `u32` values.
    fn narrow_inputs(&self) -> Vec<(u32, u32)> {
        narrow(self.inputs())
    }
    fn from_wide_inputs(workload: Workload, inputs: Vec<(u128, u128)>) -> Self {
        assert!(
            workload.is_wide(),
            "{} operands are u64 values",
            workload.slug()
        );
        let digest = common::mul_witness::u128_digest(
            &MulWitness::<u128>::from_inputs(&inputs).expect("canonical u128 witness"),
        );
        Self {
            workload,
            operands: Operands::Wide(inputs),
            digest,
        }
    }
    fn from_inputs(workload: Workload, inputs: Vec<(u64, u64)>) -> Self {
        let digest = match workload {
            Workload::U32 => mod32::digest_rows(
                inputs.iter().map(|&(a, b)| {
                    let a = u32::try_from(a).expect("u32 operand");
                    let b = u32::try_from(b).expect("u32 operand");
                    [u64::from(a), u64::from(b), u64::from(a.wrapping_mul(b))]
                }),
                inputs.len(),
            ),
            Workload::U64 => common::mul_witness::u64_digest(
                &MulWitness::<u64>::from_inputs(&inputs).expect("canonical u64 witness"),
            ),
            Workload::U128 => panic!("the u128 workload has 128-bit operands"),
        };
        Self {
            workload,
            operands: Operands::Narrow(inputs),
            digest,
        }
    }
}

fn narrow(inputs: &[(u64, u64)]) -> Vec<(u32, u32)> {
    inputs
        .iter()
        .map(|&(a, b)| {
            (
                u32::try_from(a).expect("32-bit workload operand"),
                u32::try_from(b).expect("32-bit workload operand"),
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
struct Phase {
    name: &'static str,
    tag: &'static str,
    start: u64,
    end: u64,
}
#[derive(Clone, Debug)]
struct Timing {
    phases: Vec<Phase>,
    proof_bytes: usize,
}
impl Timing {
    fn from_trial(trial: &trace_capture::TrialScopes<'_>, proof_bytes: usize) -> Self {
        let mut t = Timing {
            phases: vec![],
            proof_bytes,
        };
        for (name, tag, span) in [
            ("verified_trial", "end-to-end", trial.verified),
            ("witness_to_proof", "proving", trial.witness_to_proof),
        ] {
            t.add(name, tag, span.start_ns, span.end_ns);
        }
        t.add(
            "online_prover",
            "proving",
            trial.witness.end_ns,
            trial.witness_to_proof.end_ns,
        );
        t.add(
            "witness",
            "witness-generation",
            trial.witness.start_ns,
            trial.witness.end_ns,
        );
        t.add(
            "verify",
            "verification",
            trial.verification.start_ns,
            trial.verification.end_ns,
        );
        t.add(
            "post_proof",
            "proof-accounting",
            trial.witness_to_proof.end_ns,
            trial.verification.start_ns,
        );
        t
    }

    fn new(
        start: u64,
        witness_end: u64,
        ready: u64,
        verify_start: u64,
        end: u64,
        proof_bytes: usize,
    ) -> Self {
        let mut result = Self {
            phases: vec![],
            proof_bytes,
        };
        result.add("verified_trial", "end-to-end", start, end);
        result.add("witness_to_proof", "proving", start, ready);
        result.add("online_prover", "proving", witness_end, ready);
        result.add("witness", "witness-generation", start, witness_end);
        result.add("verify", "verification", verify_start, end);
        result.add("post_proof", "proof-accounting", ready, verify_start);
        result
    }
    fn add(&mut self, name: &'static str, tag: &'static str, start: u64, end: u64) {
        assert!(end >= start, "reversed {name} interval");
        self.phases.push(Phase {
            name,
            tag,
            start,
            end,
        });
    }
    fn union_ms(&self, pred: impl Fn(&Phase) -> bool) -> f64 {
        let mut intervals: Vec<_> = self
            .phases
            .iter()
            .filter(|p| pred(p))
            .map(|p| (p.start, p.end))
            .collect();
        intervals.sort_unstable();
        let (mut total, mut end) = (0, 0);
        for (lo, hi) in intervals {
            if hi > end {
                total += hi - lo.max(end);
                end = hi;
            }
        }
        total as f64 / 1e6
    }
    fn metrics(&self) -> Metrics {
        Metrics {
            witness_ms: self.union_ms(|p| p.tag == "witness-generation"),
            commit_ms: self.union_ms(|p| p.tag == "commit"),
            piop_ms: self.union_ms(|p| p.tag == "constraint-proof"),
            opening_ms: self.union_ms(|p| p.tag == "opening-proof"),
            pcs_ms: self.union_ms(|p| matches!(p.tag, "commit" | "opening-proof")),
            online_prover_ms: self.union_ms(|p| p.name == "online_prover"),
            witness_to_proof_ms: self.union_ms(|p| p.name == "witness_to_proof"),
            verify_ms: self.union_ms(|p| p.tag == "verification"),
            verified_trial_ms: self.union_ms(|p| p.name == "verified_trial"),
            post_proof_ms: self.union_ms(|p| p.name == "post_proof"),
            proof_bytes: self.proof_bytes,
        }
    }
    fn validate(&self) {
        assert!(self.proof_bytes > 0, "missing proof size");
        let root = &self.phases[0];
        for p in &self.phases {
            assert!(p.start >= root.start && p.end <= root.end);
        }
        for tag in [
            "witness-generation",
            "commit",
            "constraint-proof",
            "opening-proof",
            "verification",
        ] {
            assert!(
                self.phases.iter().any(|p| p.tag == tag && p.end > p.start),
                "missing measured {tag} phase"
            );
        }
    }
}

fn captured<'a>(raw: &'a [CapturedSpan], name: &str, lo: u64, hi: u64) -> &'a CapturedSpan {
    raw.iter()
        .filter(|s| {
            (s.name == name || s.component.as_deref() == Some(name))
                && s.start_ns >= lo
                && s.end_ns <= hi
        })
        .min_by_key(|s| s.start_ns)
        .unwrap_or_else(|| panic!("missing native prover span {name}"))
}

enum Context {
    F2z(f2z_backend::Context),
    Binius(binius::Context),
    BiniusLigerito(binius_ligerito::Context),
    Plonky3Fri(plonky3::Context),
    Plonky3Whir(plonky3_whir::Context),
    Limber(limber::Context),
}
impl Context {
    fn setup(
        backend: Backend,
        corpus: Arc<Corpus>,
        params: Option<common::whir_tuning::Params>,
    ) -> Self {
        match backend {
            Backend::F2z => Self::F2z(f2z_backend::Context::setup(corpus)),
            Backend::Binius => Self::Binius(binius::Context::setup(corpus)),
            Backend::BiniusLigerito => {
                Self::BiniusLigerito(binius_ligerito::Context::setup(corpus))
            }
            Backend::Plonky3Fri => Self::Plonky3Fri(plonky3::Context::setup(corpus)),
            Backend::Limber => Self::Limber(limber::Context::setup(corpus)),
            Backend::Plonky3Whir => Self::Plonky3Whir(
                plonky3_whir::Context::setup_with_params(corpus, params.expect("selected WHIR params"))
                    .expect("selected WHIR config remains eligible"),
            ),
        }
    }
    fn run(&self) -> Timing {
        match self {
            Self::F2z(c) => c.run(),
            Self::Binius(c) => c.run(),
            Self::BiniusLigerito(c) => c.run(),
            Self::Plonky3Fri(c) => c.run(),
            Self::Plonky3Whir(c) => c.run(),
            Self::Limber(c) => c.run(),
        }
    }
    fn config(&self) -> Value {
        match self {
            Self::F2z(c) => c.config(),
            Self::Binius(c) => c.config(),
            Self::BiniusLigerito(c) => c.config(),
            Self::Plonky3Fri(c) => c.config(),
            Self::Plonky3Whir(c) => c.config(),
            Self::Limber(c) => c.config(),
        }
    }
}

fn root_seed(workload: Workload) -> u64 {
    match workload {
        Workload::U32 => common::mul_witness::U32_SEED,
        Workload::U64 => common::mul_witness::U64_SEED,
        Workload::U128 => common::mul_witness::U128_SEED,
    }
}

/// Representation limit only. Backend domain checks decide eligibility;
/// available memory is not inferred from a particular benchmark machine.
fn max_exponent() -> usize {
    (usize::BITS as usize).saturating_sub(11)
}

fn check_backend_support(workloads: &[Workload], backends: &[Backend]) {
    for workload in workloads {
        for backend in backends {
            assert!(
                workload.supports(*backend),
                "the {} adapter has no {} workload; select F2Z_MUL_COMPARE_BACKENDS=\"f2z binius64\" for it",
                backend.slug(), workload.slug()
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Backend {
    F2z,
    #[value(name = "binius64")]
    Binius,
    #[value(name = "binius64-ligerito")]
    BiniusLigerito,
    Plonky3Fri,
    Plonky3Whir,
    Limber,
}

impl Backend {
    fn slug(self) -> &'static str {
        match self {
            Self::F2z => "f2z", Self::Binius => "binius64",
            Self::BiniusLigerito => "binius64-ligerito", Self::Plonky3Fri => "plonky3-fri",
            Self::Plonky3Whir => "plonky3-whir", Self::Limber => "limber",
        }
    }
}

#[derive(clap::Parser)]
struct CompareEnv {
    #[arg(env = "F2Z_BENCH_REPS", default_value_t = 5, value_parser = common::cli::positive)]
    reps: usize,
    #[arg(env = "F2Z_BENCH_SEED", value_parser = common::cli::seed)]
    seed: Option<u64>,
    #[arg(env = "F2Z_MUL_COMPARE_WORKLOADS", default_value = "u32-mod32", value_parser = common::pcs_cli::enum_list::<Workload>)]
    workloads: common::cli::List<Workload>,
    #[arg(env = "F2Z_MUL_COMPARE_BACKENDS", default_value = "f2z binius64 binius64-ligerito plonky3-fri", value_parser = common::pcs_cli::enum_list::<Backend>)]
    backends: common::cli::List<Backend>,
    #[arg(env = "F2Z_MUL_COMPARE_OUTPUT_DIR")]
    output: Option<PathBuf>,
}

impl CompareEnv {
    fn read() -> Self {
        let args = common::cli::environment::<Self>();
        for (i, workload) in args.workloads.iter().enumerate() {
            assert!(!args.workloads[..i].contains(workload), "duplicate workload {}", workload.slug());
        }
        for (i, backend) in args.backends.iter().enumerate() {
            assert!(!args.backends[..i].contains(backend), "duplicate backend {}", backend.slug());
        }
        check_backend_support(&args.workloads, &args.backends);
        args
    }
    fn proof_exponents(&self) -> Vec<usize> {
        let minimum = if self.backends.contains(&Backend::F2z) { 15 } else { 4 };
        let exponents = common::shape_values(None, clap::builder::RangedU64ValueParser::<usize>::new().range(minimum..=max_exponent() as u64))
            .unwrap_or_else(|| vec![15]);
        for (index, &n) in exponents.iter().enumerate() {
            assert!(!exponents[..index].contains(&n), "duplicate exponent {n}");
        }
        exponents
    }

    fn witness_exponents(&self) -> Vec<usize> {
        common::shape_values(None, clap::builder::RangedU64ValueParser::<usize>::new().range(4..=max_exponent() as u64))
            .unwrap_or_else(|| vec![10])
    }

}

#[derive(clap::Parser)]
struct Args {
    #[command(flatten)]
    cargo: common::cli::CargoArgs,
    #[arg(long, hide = true, num_args = 5, value_names = ["BACKEND", "WORKLOAD", "EXPONENT", "SEED", "PARAMS"])]
    measure_memory: Option<Vec<String>>,
}

#[derive(clap::Parser)]
struct MemoryEnv {
    #[arg(env = "F2Z_MUL_COMPARE_MEMORY", default_value = "1", value_parser = ["0", "1"])]
    enabled: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "bench-peak-memory")]
    let _heap_report = common::heap_run::Report::start();

    let args = <Args as clap::Parser>::parse();
    if let Some(args) = args.measure_memory {
        return memory::run_child(args);
    }
    let config = CompareEnv::read();
    let exponents = config.proof_exponents();
    let CompareEnv { workloads, backends, output, reps, seed } = config;
    let measure_memory = common::cli::environment::<MemoryEnv>().enabled == "1";
    let threads = common::init();
    f2z::observability::install()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let out = output.unwrap_or_else(|| PathBuf::from(format!("PerfRuns/{stamp}-native-mul")));
    let output = BenchmarkOutput::new(&out);
    output.create_dir_all()?;
    let mut trace = output.jsonl("trace.jsonl", FileMode::CreateNew)?;
    let mut samples = output.jsonl("samples.jsonl", FileMode::CreateNew)?;
    let mut memory_samples = output.jsonl("memory.jsonl", FileMode::CreateNew)?;
    let mut csv = output.csv("metrics.csv", FileMode::CreateNew)?;
    csv.write_record(report::CsvRow::HEADER)?;
    let rev = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let rev = String::from_utf8_lossy(&rev.stdout).trim().to_owned();
    let dirty = !std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()?
        .stdout
        .is_empty();
    let environment = common::environment::metadata(threads);
    common::whir_tuning::save(&out.join("environment.json"), &environment)?;
    let mut summary = vec![];
    for workload in workloads {
        let seed = seed.unwrap_or_else(|| root_seed(workload));
        for &n in &exponents {
            let shape_seed = if workload == Workload::U32 {
                seed
            } else {
                common::mul_witness::shape_seed(seed, n)
            };
            let corpus = Arc::new(Corpus::new(workload, n, shape_seed));
            for &selected_backend in &backends {
                let backend = selected_backend.slug();
                let whir_params = if selected_backend == Backend::Plonky3Whir {
                    let degrees: &[usize] = &[2, 5];
                    let selection = common::whir_tuning::tune(
                        degrees,
                        common::whir_tuning::replay()?,
                        |params| {
                            plonky3_whir::Context::setup_with_params(Arc::clone(&corpus), params)
                        },
                        |context| context.run().metrics().witness_to_proof_ms,
                        plonky3_whir::Context::security,
                    );
                    let path = out.join(format!("whir-{}-{n}.json", workload.slug()));
                    match selection {
                        Ok((params, mut record)) => {
                            record.workload = Some(workload.slug().to_owned());
                            record.exponent = Some(n);
                            record.corpus_digest = Some(corpus.digest.clone());
                            common::whir_tuning::save(&path, &record)?;
                            Some(params)
                        }
                        Err(reason) => {
                            eprintln!("WHIR {} 2^{n}: unavailable: {reason}", workload.slug());
                            let record = report::Ineligible {
                                workload: workload.slug(),
                                backend,
                                log_multiplications: n,
                                status: "ineligible",
                                reason,
                            };
                            common::whir_tuning::save(&path, &record)?;
                            summary.push(report::Summary::Ineligible(record));
                            continue;
                        }
                    }
                } else {
                    None
                };
                // Run before constructing the parent's backend context so two large
                // proving keys/witnesses are never live at once.
                let memory_sample = if measure_memory {
                    eprintln!(
                        "{} {backend} 2^{n}: isolated peak-memory pass",
                        workload.slug()
                    );
                    let sample = memory::measure(
                        backend,
                        workload,
                        n,
                        shape_seed,
                        threads,
                        &corpus.digest,
                        whir_params,
                    )?;
                    memory_samples.write(&sample)?;
                    memory_samples.flush()?;
                    eprintln!(
                        "{} {backend} 2^{n}: peak RSS {:.2} MiB",
                        workload.slug(),
                        sample.peak_rss_bytes as f64 / 1048576.0
                    );
                    Some(sample)
                } else {
                    None
                };
                eprintln!("{} {backend} 2^{n}: setup", workload.slug());
                // Check the actual native materialization before accepting any proof timings.
                let audit = audit_backend(selected_backend, &corpus);
                assert_eq!(audit.digest, corpus.digest);
                let (context, started) = f2z::observability::measure(
                    tracing::info_span!("mul_e2e_compare:context"),
                    || Context::setup(selected_backend, Arc::clone(&corpus), whir_params),
                ).expect("measure completed operation");
                let setup_ms = started.as_secs_f64() * 1e3;
                let mut config = context.config();
                config["proof_size_encoding"] = json!(match backend {
                    "f2z" =>
                        "commitment root + fixed-width PIOP payload/nonces + canonical F2Z opening",
                    "binius64" => "native transcript bytes (includes commitment)",
                    "binius64-ligerito" =>
                        "PIOP messages + oracle roots/Round 0 + canonical F2Z openings (includes commitment)",
                    "plonky3-fri" | "plonky3-whir" => "postcard proof bytes (includes commitment)",
                    "limber" =>
                        "serialized commitments and Brakedown batch opening + counted fixed-width PIOP payload",
                    _ => unreachable!(),
                });
                let mut measured = vec![];
                for trial in 0..=reps {
                    #[cfg(feature = "bench-perfetto")]
                    let recording = common::perfetto::Recording::start(output.buffered(
                        format!("{}-{backend}-{n}-trial-{trial}.pftrace", workload.slug()),
                        FileMode::CreateNew,
                    )?)?;
                    #[cfg(feature = "bench-perfetto")]
                    let trial_span = tracing::info_span!(
                        "benchmark_trial",
                        component = "native_mul.trial",
                        workload = workload.slug(),
                        backend = backend,
                        exponent = n,
                        trial,
                        warmup = trial == 0,
                    )
                    .entered();
                    let timing = context.run();
                    #[cfg(feature = "bench-perfetto")]
                    {
                        drop(trial_span);
                        recording.finish()?;
                    }
                    timing.validate();
                    let metrics = timing.metrics();
                    let trial_json = report::Trial::new(trial);
                    let series = format!("mul-{stamp}-{}-{backend}-{n}", workload.slug());
                    let run = format!("{series}-{trial}");
                    trace.write(&json!({
                            "schema":"zkperf.trace/v1", "record":"run", "run_id":run,"series_id":series,"root_span_id":"0",
                            "benchmark":{"suite":"native-mul","name":"mul_e2e_compare","algorithm":workload.algorithm(),"label":format!("{} 2^{n} {backend}",workload.slug()),"implementation":backend,"git_rev":rev,"git_dirty":dirty,"build_profile":"bench"},
                            "trial":trial_json,"status":"ok","trace_complete":true,
                            "clock":{"id":run,"kind":"monotonic","unit":"ns","source":"Perfetto SDK"},
                            "environment":environment,
                            "parameters":{"input":{"multiplications":1usize<<n,"log_multiplications":n,"witness_digest_blake3":corpus.digest,"seed":shape_seed},"security":config,"setup_ms":setup_ms,"primary_metric":"witness_to_proof_ms","boundary":"start native witness generation through complete PCS proof; verification, serialization, and reusable setup reported separately"},
                            "validation":{"proof_verified":true,"reference_outputs_checked":true,"native_witness_matches_canonical":true},"witness_audit":{"generation_ms_excluded":audit.generation_ms,"native_representation":audit.representation,"quotient_reconstructed":audit.quotient_reconstructed,"witness_digest_blake3":audit.digest},"metrics":metrics,
                        }))?;
                    for (i, p) in timing.phases.iter().enumerate() {
                        let mut tags = vec![p.tag];
                        if matches!(p.tag, "commit" | "opening-proof") {
                            tags.extend(["pcs", "proving"]);
                        }
                        if p.tag == "constraint-proof" {
                            tags.push("proving");
                        }
                        let root = timing.phases[0].start;
                        trace.write(&json!({
                                "schema":"zkperf.trace/v1","record":"span","run_id":run,"span_id":i.to_string(),"parent_span_id":if i==0 {None} else {Some("0")},
                                "operation":format!("native_mul.{}",p.name),"name":p.name.replace('_'," "),"primary_phase":p.tag,"phase_tags":tags,
                                "start_ns":(p.start-root).to_string(),"end_ns":(p.end-root).to_string(),"duration_ns":(p.end-p.start).to_string(),
                                "attributes":{"scope_kind":if i<3 {"scope"} else {"phase"},"primary_sequence":i>=3,"scope_tag":if i==0 {Some("end-to-end")} else {None}},
                            }))?;
                    }
                    trace.flush()?;
                    samples.write(&report::Sample {
                        schema: "native-mul-sample/v2",
                        workload: workload.slug(),
                        backend,
                        log_multiplications: n,
                        multiplications: 1usize << n,
                        corpus_digest: &corpus.digest,
                        threads,
                        seed: shape_seed,
                        trial: trial_json,
                        setup_ms,
                        config: &config,
                        measurement_policy: MEASUREMENT_POLICY,
                        proof_verified: true,
                        metrics: &metrics,
                    })?;
                    samples.flush()?;
                    eprintln!(
                        "{} {backend} 2^{n} {}: witness {:.3} ms, commit {:.3} ms, PIOP {:.3} ms, PCS {:.3} ms, witness→proof {:.3} ms, proof {} B",
                        workload.slug(),
                        if trial == 0 {
                            "warmup".into()
                        } else {
                            format!("sample {trial}/{reps}")
                        },
                        metrics.witness_ms,
                        metrics.commit_ms,
                        metrics.piop_ms,
                        metrics.pcs_ms,
                        metrics.witness_to_proof_ms,
                        timing.proof_bytes,
                    );
                    if trial > 0 {
                        measured.push(metrics);
                    }
                }
                let medians = Medians::from_samples(&measured);
                let peak_rss_bytes = memory_sample.as_ref().map(|sample| sample.peak_rss_bytes);
                csv.serialize(report::CsvRow {
                    workload: workload.slug(),
                    backend,
                    log_multiplications: n,
                    samples: reps,
                    setup_ms,
                    witness_ms: medians.witness_ms,
                    commit_ms: medians.commit_ms,
                    piop_ms: medians.piop_ms,
                    opening_ms: medians.opening_ms,
                    pcs_ms: medians.pcs_ms,
                    online_prover_ms: medians.online_prover_ms,
                    witness_to_proof_ms: medians.witness_to_proof_ms,
                    post_proof_ms: medians.post_proof_ms,
                    verify_ms: medians.verify_ms,
                    proof_bytes: medians.proof_bytes,
                    peak_rss_bytes,
                })?;
                csv.flush()?;
                println!(
                    "RESULT schema=native-mul/2 workload={} backend={backend} log_multiplications={n} samples={reps} witness_ms={} online_prover_ms={} verify_ms={} proof_bytes={} peak_rss_bytes={}",
                    workload.slug(),
                    serde_json::to_string(&medians.witness_ms)?,
                    serde_json::to_string(&medians.online_prover_ms)?,
                    serde_json::to_string(&medians.verify_ms)?,
                    serde_json::to_string(&medians.proof_bytes)?,
                    peak_rss_bytes
                        .map(|bytes| bytes.to_string())
                        .unwrap_or_else(|| "na".into())
                );
                summary.push(report::Summary::Measured(report::MeasuredSummary {
                    schema: "native-mul-summary/v2",
                    workload: workload.slug(),
                    backend,
                    log_multiplications: n,
                    multiplications: 1usize << n,
                    samples: reps,
                    warmups: 1,
                    threads,
                    seed: shape_seed,
                    setup_ms,
                    config,
                    corpus_digest: corpus.digest.clone(),
                    measurement_policy: MEASUREMENT_POLICY,
                    proof_verified: true,
                    medians,
                    peak_rss_bytes,
                    memory: memory_sample,
                }));
            }
        }
    }
    trace.finish()?;
    samples.finish()?;
    memory_samples.finish()?;
    csv.flush()?;
    output.write_json(
        "summary.json",
        &summary,
        FileMode::CreateNew,
        JsonStyle::Pretty,
    )?;
    eprintln!("Native multiplication results: {}", out.display());
    Ok(())
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires PERFETTO_TRACE_PROCESSOR; exercises native measurement backends"]
    fn native_adapters_report_repeated_perfetto_trials() {
        let _trace = super::common::test_tracing();
        use tracing_subscriber::prelude::*;
        tracing::subscriber::with_default(
            tracing_subscriber::registry().with(f2z::observability::layer()),
            || {
                for backend in [
                    Backend::Binius,
                    Backend::Plonky3Fri,
                    Backend::Plonky3Whir,
                    Backend::Limber,
                ] {
                    let context = Context::setup(
                        backend,
                        Arc::new(Corpus::new(Workload::U32, 4, 7)),
                        Some(common::whir_tuning::Params::default()),
                    );
                    let memory_bytes = tracing::subscriber::with_default(
                        tracing::subscriber::NoSubscriber::default(),
                        || match &context {
                            Context::Binius(c) => c.prove_and_verify(),
                            Context::Plonky3Fri(c) => c.prove_and_verify(),
                            Context::Plonky3Whir(c) => c.prove_and_verify(),
                            Context::Limber(c) => c.prove_and_verify(),
                            _ => unreachable!(),
                        },
                    );
                    // One warmup and five samples; each query sees only its trial.
                    for _ in 0..6 {
                        let timing = context.run();
                        timing.validate();
                        assert_eq!(timing.proof_bytes, memory_bytes, "{backend:?}");
                        let metrics = timing.metrics();
                        assert!(metrics.witness_ms > 0.0, "{backend:?}");
                        assert!(metrics.verify_ms > 0.0, "{backend:?}");
                    }
                }
            },
        );
    }
    /// BLAKE3 digest of the canonical 2^15 u128 corpus (`U128_SEED`), pinned
    /// when the workload was added.
    #[allow(dead_code)] // `cargo bench` sets cfg(test) without running the #[test] callers.
    const U128_CORPUS_2P15_DIGEST: &str =
        "0964e84115dfbfa7e8046a548ac4d30a3edbffdd279a0c44dde201b659a54879";
    #[test]
    fn proof_sizes_survive_sample_and_summary_export() {
        let measured: Vec<_> = [1024, 4096, 2048]
            .into_iter()
            .map(|bytes| {
                let mut timing = Timing::new(0, 10, 40, 40, 50, bytes);
                timing.add("commit", "commit", 10, 20);
                timing.add("piop", "constraint-proof", 20, 30);
                timing.add("opening", "opening-proof", 30, 40);
                timing.validate();
                timing.metrics()
            })
            .collect();
        let summary = Medians::from_samples(&measured);
        assert_eq!(measured[0].proof_bytes, 1024);
        assert_eq!(summary.proof_bytes, 2048.0);
        let sample = serde_json::to_value(measured[0]).unwrap();
        let summary_json = serde_json::to_value(summary).unwrap();
        assert!(sample["proof_bytes"].is_u64());
        assert!(summary_json["proof_bytes"].is_f64());
        assert!(sample.get("verified_trial_ms").is_some());
        assert!(summary_json.get("verified_trial_ms").is_none());
        // Preserve the existing upper-middle median, including even sample counts.
        assert_eq!(Medians::from_samples(&measured[..2]).proof_bytes, 4096.0);
        assert_eq!(
            serde_json::to_value(report::Trial::new(0)).unwrap(),
            json!({"kind":"warmup","index":0})
        );
        assert_eq!(
            serde_json::to_value(report::Trial::new(1)).unwrap(),
            json!({"kind":"sample","index":0})
        );
    }
    #[test]
    #[should_panic(expected = "missing proof size")]
    fn zero_proof_size_cannot_be_reported_as_success() {
        Timing::new(0, 10, 40, 40, 50, 0).validate();
    }
    #[test]
    fn overlapping_phases_are_unioned() {
        let mut t = Timing {
            phases: vec![],
            proof_bytes: 0,
        };
        t.add("commit", "commit", 10, 30);
        t.add("opening", "opening-proof", 20, 50);
        assert_eq!(t.union_ms(|_| true), 40.0 / 1e6);
    }
    #[test]
    fn deterministic_corpus_matches_saved_pcs_witnesses() {
        let a = Corpus::new(Workload::U32, 4, 7);
        assert_eq!(a.digest, Corpus::new(Workload::U32, 4, 7).digest);
        assert_ne!(a.digest, Corpus::new(Workload::U32, 4, 8).digest);
        let a = Corpus::new(Workload::U64, 4, 7);
        assert_eq!(a.digest, Corpus::new(Workload::U64, 4, 7).digest);
        assert_ne!(a.digest, Corpus::new(Workload::U64, 4, 8).digest);
        assert!(a.inputs().iter().any(|&(x, _)| x > u64::from(u32::MAX)));
        assert_eq!(
            Corpus::new(
                Workload::U64,
                15,
                common::mul_witness::shape_seed(common::mul_witness::U64_SEED, 15)
            )
            .digest,
            "7cd5974fd9a40005cbc916f667a078e3717ddb7f4a0cf107557ca8e3ce646425"
        );
        let a = Corpus::new(Workload::U128, 4, 7);
        assert_eq!(a.digest, Corpus::new(Workload::U128, 4, 7).digest);
        assert_ne!(a.digest, Corpus::new(Workload::U128, 4, 8).digest);
        assert!(
            a.wide_inputs()
                .iter()
                .any(|&(x, _)| x > u128::from(u64::MAX))
        );
        assert_eq!(
            Corpus::new(
                Workload::U128,
                15,
                common::mul_witness::shape_seed(common::mul_witness::U128_SEED, 15)
            )
            .digest,
            U128_CORPUS_2P15_DIGEST
        );
    }
}

#[derive(Debug)]
struct WitnessAudit {
    digest: String,
    generation_ms: f64,
    representation: &'static str,
    quotient_reconstructed: bool,
}
impl WitnessAudit {
    /// Audits the native rows `[x, y, z_lo, z_hi]` of the `u128` workload
    /// against the corpus digest (32-byte entries, see `u128_digest`).
    fn check_wide(
        corpus: &Corpus,
        rows: Vec<[u128; 4]>,
        generation_ms: f64,
        representation: &'static str,
    ) -> Self {
        let inputs = corpus.wide_inputs();
        assert_eq!(rows.len(), inputs.len(), "native witness row count");
        let layout = MulLayout::<u128>::new(rows.len()).expect("canonical u128 layout");
        let capacity = layout.capacity();
        let mut hash = blake3::Hasher::new();
        hash.update(b"f2z/u128-mul-compare/integer-witness/v1");
        hash.update(&(layout.assignment_len() as u64).to_le_bytes());
        let mut entries = vec![(0_u128, 0_u128); layout.assignment_len()];
        entries[0] = (1, 0);
        for (i, (&[x, y, lo, hi], &(expected_x, expected_y))) in rows.iter().zip(inputs).enumerate()
        {
            assert_eq!(
                [x, y, lo, hi],
                corpus.workload.wide_row(expected_x, expected_y),
                "native witness mismatch at row {i} ({representation})"
            );
            entries[capacity + i] = (x, 0);
            entries[2 * capacity + i] = (y, 0);
            entries[3 * capacity + i] = (lo, hi);
        }
        for (lo, hi) in entries {
            hash.update(&lo.to_le_bytes());
            hash.update(&hi.to_le_bytes());
        }
        let digest = hash.finalize().to_hex().to_string();
        assert_eq!(digest, corpus.digest, "recovered native assignment digest");
        Self {
            digest,
            generation_ms,
            representation,
            quotient_reconstructed: false,
        }
    }
    fn check(
        corpus: &Corpus,
        rows: Vec<[u64; 4]>,
        generation_ms: f64,
        representation: &'static str,
        quotient_reconstructed: bool,
    ) -> Self {
        let inputs = corpus.inputs();
        assert_eq!(rows.len(), inputs.len(), "native witness row count");
        let n = rows.len();
        if corpus.workload == Workload::U32 {
            for (i, (row, &(x, y))) in rows.iter().zip(inputs).enumerate() {
                assert_eq!(
                    &row[..3],
                    &corpus.workload.native_row(x, y)[..3],
                    "native modular witness mismatch at row {i} ({representation})"
                );
            }
            let digest = mod32::digest_rows(rows.iter().map(|r| [r[0], r[1], r[2]]), n);
            assert_eq!(digest, corpus.digest, "recovered native assignment digest");
            return Self {
                digest,
                generation_ms,
                representation,
                quotient_reconstructed,
            };
        }
        assert_eq!(corpus.workload, Workload::U64);
        let layout = MulLayout::<u64>::new(n).expect("canonical u64 layout");
        let (capacity, assignment_len) = (layout.capacity(), layout.assignment_len());
        let mut assignment = vec![0u64; assignment_len];
        assignment[0] = 1;
        let has_fourth_block = true;
        for (i, (&[a, b, c, q], &(expected_a, expected_b))) in rows.iter().zip(inputs).enumerate() {
            assert_eq!(
                [a, b, c, q],
                corpus.workload.native_row(expected_a, expected_b),
                "native witness mismatch at row {i} ({representation})"
            );
            assignment[capacity + i] = a;
            assignment[2 * capacity + i] = b;
            assignment[3 * capacity + i] = c;
            if has_fourth_block {
                assignment[4 * capacity + i] = q;
            }
        }
        let domain: &[u8] = b"f2z/u64-mul-compare/integer-witness/v1";
        let mut hash = blake3::Hasher::new();
        hash.update(domain);
        hash.update(&(assignment.len() as u64).to_le_bytes());
        for value in assignment {
            hash.update(&value.to_le_bytes());
        }
        let digest = hash.finalize().to_hex().to_string();
        assert_eq!(digest, corpus.digest, "recovered native assignment digest");
        Self {
            digest,
            generation_ms,
            representation,
            quotient_reconstructed,
        }
    }
}
fn audit_backend(backend: Backend, corpus: &Corpus) -> WitnessAudit {
    match backend {
        Backend::F2z => f2z_backend::audit(corpus),
        // The same Binius64 circuit and witness filler; only the opener differs.
        Backend::Binius | Backend::BiniusLigerito => binius::audit(corpus),
        Backend::Plonky3Fri | Backend::Plonky3Whir => mod32_air::audit(corpus),
        Backend::Limber => limber::audit(corpus),
    }
}

#[allow(dead_code)] // Invoked by the separate mul_witness_compare entry point.
pub(crate) fn witness_main() -> Result<(), Box<dyn std::error::Error>> {
    common::cli::EnvironmentCli::parse();
    let config = CompareEnv::read();
    let exponents = config.witness_exponents();
    let CompareEnv { workloads, backends, output, reps, seed } = config;
    f2z::observability::install()?;
    let threads = common::init();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let out = output.unwrap_or_else(|| PathBuf::from(format!("PerfRuns/{stamp}-mul-witness-compare")));
    let output = BenchmarkOutput::new(&out);
    output.create_dir_all()?;
    let mut raw = output.jsonl("witness-checks.jsonl", FileMode::CreateNew)?;
    let mut summary = vec![];
    for workload in workloads {
        let seed = seed.unwrap_or_else(|| root_seed(workload));
        for &n in &exponents {
            let shape_seed = if workload == Workload::U32 {
                seed
            } else {
                common::mul_witness::shape_seed(seed, n)
            };
            let corpus = Corpus::new(workload, n, shape_seed);
            for &selected_backend in &backends {
                let backend = selected_backend.slug();
                let mut times = vec![];
                for trial in 0..=reps {
                    let audit = audit_backend(selected_backend, &corpus);
                    if trial > 0 {
                        times.push(audit.generation_ms);
                    }
                    raw.write(&report::WitnessSample {
                        schema: "native-mul-witness/v1",
                        workload: workload.slug(),
                        backend,
                        log_multiplications: n,
                        threads,
                        seed: shape_seed,
                        trial: report::Trial::new(trial),
                        witness_generation_ms: audit.generation_ms,
                        witness_digest_blake3: &audit.digest,
                        expected_digest: &corpus.digest,
                        native_representation: audit.representation,
                        quotient_reconstructed: audit.quotient_reconstructed,
                        all_rows_match: true,
                    })?;
                    raw.flush()?;
                }
                let median = common::median(&times);
                eprintln!(
                    "{} {backend} 2^{n}: all rows match, digest {}, witness {median:.3} ms",
                    workload.slug(),
                    corpus.digest
                );
                summary.push(report::WitnessSummary {
                    workload: workload.slug(),
                    backend,
                    log_multiplications: n,
                    samples: reps,
                    witness_ms: median,
                    witness_digest_blake3: corpus.digest.clone(),
                    all_rows_match: true,
                });
            }
        }
    }
    raw.finish()?;
    output.write_json(
        "witness-summary.json",
        &summary,
        FileMode::CreateNew,
        JsonStyle::Pretty,
    )?;
    eprintln!("Witness comparison: {}", out.display());
    Ok(())
}

/// Sixteen boundary gates of a workload: zero, one, and the largest
/// operand in every combination.
#[cfg(test)]
#[allow(dead_code)] // `cargo bench` sets cfg(test) without running the #[test] callers.
fn edge_corpus(workload: Workload) -> Corpus {
    if workload.is_wide() {
        let max = u128::MAX;
        return Corpus::from_wide_inputs(
            workload,
            [(0, 0), (0, max), (1, max), (max, max)].repeat(4),
        );
    }
    let max = match workload {
        Workload::U32 => u64::from(u32::MAX),
        Workload::U64 => u64::MAX,
        Workload::U128 => unreachable!(),
    };
    Corpus::from_inputs(workload, [(0, 0), (0, max), (1, max), (max, max)].repeat(4))
}

#[cfg(test)]
#[allow(unused_imports)]
mod witness_tests {
    use super::*;
    #[test]
    fn all_native_witnesses_recover_the_same_assignment() {
        let _trace = common::test_tracing();
        for workload in [Workload::U32, Workload::U64, Workload::U128] {
            let corpus = edge_corpus(workload);
            for backend in [
                Backend::F2z,
                Backend::Binius,
                Backend::BiniusLigerito,
                Backend::Plonky3Fri,
                Backend::Plonky3Whir,
            ] {
                if !workload.supports(backend) {
                    continue;
                }
                assert_eq!(audit_backend(backend, &corpus).digest, corpus.digest);
            }
        }
    }
    #[test]
    fn witness_check_rejects_a_changed_native_row() {
        let corpus = Corpus::new(Workload::U32, 4, 7);
        let mut rows: Vec<_> = corpus
            .inputs()
            .iter()
            .map(|&(a, b)| corpus.workload.native_row(a, b))
            .collect();
        rows[0][2] ^= 1;
        assert!(
            std::panic::catch_unwind(|| WitnessAudit::check(
                &corpus,
                rows,
                0.0,
                "corrupted native witness",
                false
            ))
            .is_err()
        );
    }
}

#[cfg(test)]
mod ligerito_isolation_tests {
    use super::*;

    // Separate processes avoid racing other tests over process-global settings.
    #[test]
    fn configuration_probe() {
        if std::env::var_os("F2Z_TEST_CONFIGURATION_PROBE").is_none() {
            return;
        }
        let corpus = Arc::new(Corpus::new(Workload::U32, 15, 7));
        let f2z = f2z_backend::Context::setup(Arc::clone(&corpus)).config();
        let small = Arc::new(edge_corpus(Workload::U32));
        let binius = binius::Context::setup(Arc::clone(&small)).config();
        let fri = plonky3::Context::setup(Arc::clone(&small)).config();
        let whir =
            plonky3_whir::Context::setup_with_params(small, common::whir_tuning::Params::default())
                .unwrap()
                .config();
        println!(
            "CONFIG_PROBE {}",
            json!({"f2z":f2z,"binius":binius,"fri":fri,"whir":whir})
        );
    }

    #[test]
    fn ligerito_selector_leaves_competing_configurations_unchanged() {
        let probe = |profile| {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "benchmark::ligerito_isolation_tests::configuration_probe",
                    "--nocapture",
                ])
                .env("F2Z_TEST_CONFIGURATION_PROBE", "1")
                .env("F2Z_LIG_PROFILE", profile)
                .env("RAYON_NUM_THREADS", "2")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).unwrap();
            serde_json::from_str::<Value>(
                stdout
                    .lines()
                    .find_map(|l| l.strip_prefix("CONFIG_PROBE "))
                    .expect("configuration probe output"),
            )
            .unwrap()
        };
        let johnson = probe("custom:1:4");
        let udr = probe("udrg:1:4");
        assert_ne!(
            johnson["f2z"]["ligerito"]["configuration_fingerprint"],
            udr["f2z"]["ligerito"]["configuration_fingerprint"]
        );
        for backend in ["binius", "fri", "whir"] {
            assert_eq!(johnson[backend], udr[backend], "{backend}");
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn schemas_and_memory_transport() {
        Args::command().debug_assert();
        CompareEnv::command().debug_assert();
        MemoryEnv::command().debug_assert();
        assert!(Args::try_parse_from(["mul", "--bench"]).unwrap().measure_memory.is_none());
        assert_eq!(Args::try_parse_from(["mul", "--measure-memory", "f2z", "u32", "15", "0", "null"]).unwrap().measure_memory.unwrap().len(), 5);
        assert!(Args::try_parse_from(["mul", "--measure-memory", "f2z", "u32", "15", "0"]).is_err());
        assert_eq!(common::pcs_cli::enum_list::<Workload>("u32 u32-mod32").unwrap(), [Workload::U32, Workload::U32]);
    }
}


#[cfg(test)]
mod cli_environment_tests {
    use super::*;

    #[test]
    fn configuration_probe() {
        let Ok(mode) = std::env::var("F2Z_MUL_CLI_TEST_MODE") else {
            return;
        };
        let config = CompareEnv::read();
        let exponents = if mode == "proof" { config.proof_exponents() } else { config.witness_exponents() };
        let memory = (mode == "proof").then(|| common::cli::environment::<MemoryEnv>().enabled == "1");
        println!("MUL_CONFIG {}", json!({"exponents":exponents,"reps":config.reps,"seed":config.seed,"memory":memory,
            "workloads":config.workloads.iter().map(|w| w.slug()).collect::<Vec<_>>(),
            "backends":config.backends.iter().map(|b| b.slug()).collect::<Vec<_>>()}));
    }

    fn child(mode: &str, settings: &[(&str, &str)]) -> std::process::Output {
        let test = concat!(module_path!(), "::configuration_probe");
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env_clear().env("F2Z_MUL_CLI_TEST_MODE", mode).envs(settings.iter().copied())
            .output().unwrap()
    }

    fn config(mode: &str, settings: &[(&str, &str)]) -> Value {
        let out = child(mode, settings);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines()
            .find_map(|s| s.strip_prefix("MUL_CONFIG ")).unwrap()).unwrap()
    }

    #[test]
    fn proof_and_witness_preserve_defaults_and_distinct_shape_rules() {
        let proof = config("proof", &[]);
        let witness = config("witness", &[]);
        assert_eq!(proof["exponents"], json!([15]));
        assert_eq!(witness["exponents"], json!([10]));
        assert_eq!(proof["reps"], 5);
        assert_eq!(proof["seed"], Value::Null);
        assert_eq!(proof["memory"], true);
        assert_eq!(witness["memory"], Value::Null);
        assert_eq!(proof["workloads"], json!(["u32-mod32"]));
        assert_eq!(proof["backends"], json!(["f2z","binius64","binius64-ligerito","plonky3-fri"]));
        assert_eq!(witness["backends"], proof["backends"]);
        for shapes in ["4", "14", "15 15"] {
            assert!(!child("proof", &[("F2Z_BENCH_SHAPES", shapes)]).status.success());
            assert!(child("witness", &[("F2Z_BENCH_SHAPES", shapes)]).status.success());
        }
        assert_eq!(config("proof", &[("F2Z_BENCH_SHAPES", "4"),
            ("F2Z_MUL_COMPARE_BACKENDS", "binius64")])["exponents"], json!([4]));
        let upper = max_exponent().to_string();
        let above = (max_exponent() + 1).to_string();
        for mode in ["proof", "witness"] {
            assert!(child(mode, &[("F2Z_BENCH_SHAPES", &upper)]).status.success());
            for bad in ["3", &above] { assert!(!child(mode, &[("F2Z_BENCH_SHAPES", bad)]).status.success()); }
        }
    }

    #[test]
    fn campaign_overrides_alias_duplicates_and_memory_switch() {
        let config = config(
            "proof",
            &[
                ("F2Z_MUL_COMPARE_WORKLOADS", "u64 u128"),
                ("F2Z_MUL_COMPARE_BACKENDS", "f2z binius64"),
                ("F2Z_BENCH_REPS", "3"),
                ("F2Z_BENCH_SEED", "0Xff"),
                ("F2Z_MUL_COMPARE_MEMORY", "0"),
            ],
        );
        assert_eq!(config["workloads"], json!(["u64", "u128"]));
        assert_eq!(config["reps"], 3);
        assert_eq!(config["seed"], 255);
        assert_eq!(config["memory"], false);
        for settings in [
            vec![("F2Z_MUL_COMPARE_WORKLOADS", "u32 u32-mod32")],
            vec![("F2Z_MUL_COMPARE_BACKENDS", "f2z f2z")],
            vec![
                ("F2Z_MUL_COMPARE_WORKLOADS", "u64"),
                ("F2Z_MUL_COMPARE_BACKENDS", "plonky3-fri"),
            ],
            vec![("F2Z_BENCH_REPS", "0")],
            vec![("F2Z_MUL_COMPARE_MEMORY", "true")],
        ] {
            assert!(
                !child("proof", &settings).status.success(),
                "accepted {settings:?}"
            );
        }
        assert!(
            child("witness", &[("F2Z_MUL_COMPARE_MEMORY", "unused")])
                .status
                .success()
        );
    }
}
