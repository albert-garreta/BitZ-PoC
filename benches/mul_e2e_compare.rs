//! Native end-to-end multiplication proofs. See docs/native-mul-compare.md.
#[path = "mul_e2e_compare/binius.rs"]
mod binius;
mod common;
#[path = "mul_e2e_compare/f2z.rs"]
mod f2z_backend;
#[path = "mul_e2e_compare/limber.rs"]
mod limber_backend;
#[path = "mul_e2e_compare/memory.rs"]
mod memory;
#[path = "mul_e2e_compare/plonky3.rs"]
mod plonky3;
#[path = "common/trace_capture.rs"]
mod trace_capture;

use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};
use trace_capture::{CaptureLayer, CapturedSpan, TraceCapture};

const BABY_P: u64 = 2_013_265_921;

const MEDIAN_METRICS: &[&str] = &[
    "witness_ms",
    "commit_ms",
    "piop_ms",
    "opening_ms",
    "pcs_ms",
    "online_prover_ms",
    "witness_to_proof_ms",
    "verify_ms",
    "proof_bytes",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workload {
    U32,
    BabyBear,
    U64,
    U128,
}
impl Workload {
    fn parse(slug: &str) -> Self {
        match slug {
            "u32" => Self::U32,
            "babybear" => Self::BabyBear,
            "u64" => Self::U64,
            "u128" => Self::U128,
            other => panic!("unknown workload {other}"),
        }
    }
    fn slug(self) -> &'static str {
        match self {
            Self::U32 => "u32",
            Self::BabyBear => "babybear",
            Self::U64 => "u64",
            Self::U128 => "u128",
        }
    }
    fn algorithm(self) -> &'static str {
        match self {
            Self::U32 => "u32 multiplication",
            Self::BabyBear => "BabyBear multiplication",
            Self::U64 => "u64 multiplication",
            Self::U128 => "u128 multiplication",
        }
    }
    /// Backends with a native arithmetization of this workload.
    fn supports(self, backend: &str) -> bool {
        match self {
            Self::U32 | Self::BabyBear => true,
            // The Plonky3 AIR decomposes 32-bit operands and the Limber
            // program uses u64 linear-combination coefficients; neither has
            // a 64 x 64 -> 128 or 128 x 128 -> 256 path yet.
            Self::U64 | Self::U128 => matches!(backend, "f2z" | "binius64"),
        }
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
            Self::U32 | Self::U64 => product,
            Self::BabyBear => product % u128::from(BABY_P),
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
            Self::U32 => [a, b, output as u64, 0],
            Self::BabyBear => [a, b, output as u64, (a * b) / BABY_P],
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
        let inputs: Vec<_> = (0..1usize << exponent)
            .map(|_| match workload {
                // The 32-bit workloads keep drawing u32 values, so their
                // corpora (and digests) are unchanged by the wider input type.
                Workload::U32 => (
                    u64::from(rng.random::<u32>()),
                    u64::from(rng.random::<u32>()),
                ),
                Workload::BabyBear => {
                    let mut next = || rng.random();
                    (
                        u64::from(f2z::piop::spartan::sample_baby_bear_operand_with(&mut next)),
                        u64::from(f2z::piop::spartan::sample_baby_bear_operand_with(&mut next)),
                    )
                }
                Workload::U64 => (rng.random::<u64>(), rng.random::<u64>()),
                Workload::U128 => unreachable!(),
            })
            .collect();
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
            Operands::Wide(_) => panic!("the {} workload has 128-bit operands", self.workload.slug()),
        }
    }
    /// The operand pairs of the `u128` workload.
    fn wide_inputs(&self) -> &[(u128, u128)] {
        match &self.operands {
            Operands::Wide(inputs) => inputs,
            Operands::Narrow(_) => panic!("the {} workload has 64-bit operands", self.workload.slug()),
        }
    }
    /// The operand pairs of a 32-bit workload as `u32` values.
    fn narrow_inputs(&self) -> Vec<(u32, u32)> {
        narrow(self.inputs())
    }
    fn from_wide_inputs(workload: Workload, inputs: Vec<(u128, u128)>) -> Self {
        use f2z::piop::spartan::U128MulWitness;
        assert!(workload.is_wide(), "{} operands are u64 values", workload.slug());
        let digest = common::mul_witness::u128_digest(
            &U128MulWitness::from_inputs(&inputs).expect("canonical u128 witness"),
        );
        Self {
            workload,
            operands: Operands::Wide(inputs),
            digest,
        }
    }
    fn from_inputs(workload: Workload, inputs: Vec<(u64, u64)>) -> Self {
        use f2z::piop::spartan::{BabyBearMulWitness, U32MulWitness, U64MulWitness};
        let digest = match workload {
            Workload::U32 => common::mul_witness::u32_digest(
                &U32MulWitness::from_inputs(&narrow(&inputs)).expect("canonical u32 witness"),
            ),
            Workload::BabyBear => common::mul_witness::baby_bear_digest(
                &BabyBearMulWitness::from_inputs(&narrow(&inputs))
                    .expect("canonical BabyBear witness"),
            ),
            Workload::U64 => common::mul_witness::u64_digest(
                &U64MulWitness::from_inputs(&inputs).expect("canonical u64 witness"),
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
    fn metrics(&self) -> Value {
        json!({
            "witness_ms": self.union_ms(|p| p.tag=="witness-generation"),
            "commit_ms": self.union_ms(|p| p.tag=="commit"),
            "piop_ms": self.union_ms(|p| p.tag=="constraint-proof"),
            "opening_ms": self.union_ms(|p| p.tag=="opening-proof"),
            "pcs_ms": self.union_ms(|p| matches!(p.tag,"commit"|"opening-proof")),
            "online_prover_ms": self.union_ms(|p| p.name=="online_prover"),
            "witness_to_proof_ms": self.union_ms(|p| p.name=="witness_to_proof"),
            "verify_ms": self.union_ms(|p| p.tag=="verification"),
            "verified_trial_ms": self.union_ms(|p| p.name=="verified_trial"),
            "proof_bytes": self.proof_bytes,
        })
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
    Plonky3(plonky3::Context),
    Limber(limber_backend::Context),
}
impl Context {
    fn setup(backend: &str, corpus: Arc<Corpus>) -> Self {
        match backend {
            "f2z" => Self::F2z(f2z_backend::Context::setup(corpus)),
            "binius64" => Self::Binius(binius::Context::setup(corpus)),
            "plonky3-whir" => Self::Plonky3(plonky3::Context::setup(corpus)),
            "limber" => Self::Limber(limber_backend::Context::setup(corpus)),
            _ => panic!("unknown backend {backend}"),
        }
    }
    fn run(&self, capture: &TraceCapture) -> Timing {
        match self {
            Self::F2z(c) => c.run(),
            Self::Binius(c) => c.run(capture),
            Self::Plonky3(c) => c.run(capture),
            Self::Limber(c) => c.run(capture),
        }
    }
    fn config(&self) -> Value {
        match self {
            Self::F2z(c) => c.config(),
            Self::Binius(c) => c.config(),
            Self::Plonky3(c) => c.config(),
            Self::Limber(c) => c.config(),
        }
    }
}

fn root_seed(workload: Workload) -> u64 {
    match workload {
        Workload::U32 => common::mul_witness::U32_SEED,
        Workload::BabyBear => common::mul_witness::BABY_BEAR_SEED,
        Workload::U64 => common::mul_witness::U64_SEED,
        Workload::U128 => common::mul_witness::U128_SEED,
    }
}

/// Largest accepted exponent: F2Z commits `2^(n+7)` bits for u32,
/// `2^(n+8)` for BabyBear and u64, and `2^(n+9)` for u128, so each wider
/// workload stops one size earlier on a 16 GB machine.
fn max_exponent(workloads: &[String]) -> usize {
    if workloads.iter().any(|w| w == "u128") {
        23
    } else if workloads.iter().any(|w| w == "babybear" || w == "u64") {
        24
    } else {
        25
    }
}

fn check_backend_support(workloads: &[String], backends: &[String]) {
    for workload in workloads {
        let workload = Workload::parse(workload);
        for backend in backends {
            assert!(
                workload.supports(backend),
                "the {backend} adapter has no {} workload; select F2Z_MUL_COMPARE_BACKENDS=\"f2z binius64\" for it",
                workload.slug()
            );
        }
    }
}

fn choices(var: &str, default: &str, allowed: &[&str]) -> Vec<String> {
    let raw = std::env::var(var).unwrap_or_else(|_| default.into());
    let mut result = vec![];
    for s in raw.split([',', ' ']).filter(|s| !s.is_empty()) {
        assert!(
            allowed.contains(&s),
            "{var}: unknown choice {s}; expected {allowed:?}"
        );
        assert!(!result.iter().any(|x| x == s), "{var}: duplicate {s}");
        result.push(s.to_owned());
    }
    assert!(!result.is_empty(), "{var} must not be empty");
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Before starting Rayon or installing a subscriber.
    unsafe {
        std::env::set_var("OBLONG_PROFILE_INTERVALS", "1");
    }
    let threads = common::init();
    let capture = CaptureLayer::install();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--measure-memory") {
        return memory::run_child(&capture, &args[1..]);
    }
    let measure_memory = match std::env::var("F2Z_MUL_COMPARE_MEMORY").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("1") => true,
        Ok("0") => false,
        _ => return Err("F2Z_MUL_COMPARE_MEMORY must be 0 or 1".into()),
    };
    let reps = common::reps(None, 5);
    assert!(reps > 0);
    let workloads = choices(
        "F2Z_MUL_COMPARE_WORKLOADS",
        "u32 babybear",
        &["u32", "babybear", "u64", "u128"],
    );
    let backends = choices(
        "F2Z_MUL_COMPARE_BACKENDS",
        "f2z binius64 plonky3-whir limber",
        &["f2z", "binius64", "plonky3-whir", "limber"],
    );
    check_backend_support(&workloads, &backends);
    let shapes = common::shapes(None).unwrap_or_else(|| vec!["15".into()]);
    let max_exponent = max_exponent(&workloads);
    let mut exponents = vec![];
    for shape in shapes {
        let n: usize = shape.parse()?;
        assert!(
            (4..=max_exponent).contains(&n),
            "F2Z_BENCH_SHAPES must be in 4..={max_exponent} for the selected workloads"
        );
        assert!(
            n >= 15 || !backends.iter().any(|b| b == "f2z"),
            "F2Z requires exponent >=15"
        );
        assert!(!exponents.contains(&n), "duplicate exponent {n}");
        exponents.push(n);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let out = std::env::var_os("F2Z_MUL_COMPARE_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("PerfRuns/{stamp}-native-mul")));
    fs::create_dir_all(&out)?;
    let create = |name: &str| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(name))
    };
    let mut trace = BufWriter::new(create("trace.jsonl")?);
    let mut samples = BufWriter::new(create("samples.jsonl")?);
    let mut memory_samples = BufWriter::new(create("memory.jsonl")?);
    let mut csv = BufWriter::new(create("metrics.csv")?);
    writeln!(
        csv,
        "workload,backend,log_multiplications,samples,setup_ms,{},peak_rss_bytes",
        MEDIAN_METRICS.join(",")
    )?;
    let rev = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let rev = String::from_utf8_lossy(&rev.stdout).trim().to_owned();
    let dirty = !std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()?
        .stdout
        .is_empty();
    let mut summary = vec![];
    for workload in workloads {
        let workload = Workload::parse(&workload);
        let seed = common::seed(None, root_seed(workload));
        for &n in &exponents {
            let shape_seed = common::mul_witness::shape_seed(seed, n);
            let corpus = Arc::new(Corpus::new(workload, n, shape_seed));
            for backend in &backends {
                // Run before constructing the parent's backend context so two large
                // proving keys/witnesses are never live at once.
                let memory_sample = if measure_memory {
                    eprintln!(
                        "{} {backend} 2^{n}: isolated peak-memory pass",
                        workload.slug()
                    );
                    let sample =
                        memory::measure(backend, workload, n, shape_seed, threads, &corpus.digest)?;
                    writeln!(memory_samples, "{}", serde_json::to_string(&sample)?)?;
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
                let audit = audit_backend(backend, &corpus);
                assert_eq!(audit.digest, corpus.digest);
                let started = std::time::Instant::now();
                let context = Context::setup(backend, Arc::clone(&corpus));
                let setup_ms = started.elapsed().as_secs_f64() * 1e3;
                let mut config = context.config();
                config["proof_size_encoding"] = json!(match backend.as_str() {
                    "f2z" =>
                        "commitment root + fixed-width PIOP payload/nonces + canonical F2Z opening",
                    "limber" =>
                        "canonical commitments/opening + shape-derived fixed-width PIOP payload",
                    "binius64" => "native transcript bytes (includes commitment)",
                    "plonky3-whir" => "postcard proof bytes (includes commitment)",
                    _ => unreachable!(),
                });
                let mut measured = vec![];
                for trial in 0..=reps {
                    let timing = context.run(&capture);
                    timing.validate();
                    let metrics = timing.metrics();
                    let trial_json = if trial == 0 {
                        json!({"kind":"warmup","warmup_index":0})
                    } else {
                        json!({"kind":"sample","sample_index":trial-1})
                    };
                    let series = format!("mul-{stamp}-{}-{backend}-{n}", workload.slug());
                    let run = format!("{series}-{trial}");
                    writeln!(
                        trace,
                        "{}",
                        json!({
                            "schema":"zkperf.trace/v1", "record":"run", "run_id":run,"series_id":series,"root_span_id":"0",
                            "benchmark":{"suite":"native-mul","name":"mul_e2e_compare","algorithm":workload.algorithm(),"label":format!("{} 2^{n} {backend}",workload.slug()),"implementation":backend,"git_rev":rev,"git_dirty":dirty,"build_profile":"bench"},
                            "trial":trial_json,"status":"ok","trace_complete":true,
                            "clock":{"id":run,"kind":"monotonic","unit":"ns","source":"std::time::Instant"},
                            "environment":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"threads":threads,"rustflags":std::env::var("RUSTFLAGS").ok()},
                            "parameters":{"input":{"multiplications":1usize<<n,"log_multiplications":n,"witness_digest_blake3":corpus.digest,"seed":shape_seed},"security":config,"setup_ms":setup_ms,"boundary":"regenerate native witness through full proof verification; corpus sampling and public setup excluded"},
                            "validation":{"proof_verified":true,"reference_outputs_checked":true,"native_witness_matches_canonical":true},"witness_audit":{"generation_ms_excluded":audit.generation_ms,"native_representation":audit.representation,"quotient_reconstructed":audit.quotient_reconstructed,"witness_digest_blake3":audit.digest},"metrics":metrics,
                        })
                    )?;
                    for (i, p) in timing.phases.iter().enumerate() {
                        let mut tags = vec![p.tag];
                        if matches!(p.tag, "commit" | "opening-proof") {
                            tags.extend(["pcs", "proving"]);
                        }
                        if p.tag == "constraint-proof" {
                            tags.push("proving");
                        }
                        let root = timing.phases[0].start;
                        writeln!(
                            trace,
                            "{}",
                            json!({
                                "schema":"zkperf.trace/v1","record":"span","run_id":run,"span_id":i.to_string(),"parent_span_id":if i==0 {None} else {Some("0")},
                                "operation":format!("native_mul.{}",p.name),"name":p.name.replace('_'," "),"primary_phase":p.tag,"phase_tags":tags,
                                "start_ns":(p.start-root).to_string(),"end_ns":(p.end-root).to_string(),"duration_ns":(p.end-p.start).to_string(),
                                "attributes":{"scope_kind":if i<3 {"scope"} else {"phase"},"primary_sequence":i>=3,"scope_tag":if i==0 {Some("end-to-end")} else {None}},
                            })
                        )?;
                    }
                    trace.flush()?;
                    writeln!(
                        samples,
                        "{}",
                        json!({"workload":workload.slug(),"backend":backend,"log_multiplications":n,"trial":trial_json,"metrics":metrics})
                    )?;
                    samples.flush()?;
                    eprintln!(
                        "{} {backend} 2^{n} {}: witness {:.3} ms, commit {:.3} ms, PIOP {:.3} ms, PCS {:.3} ms, witness→proof {:.3} ms, proof {} B",
                        workload.slug(),
                        if trial == 0 {
                            "warmup".into()
                        } else {
                            format!("sample {trial}/{reps}")
                        },
                        metrics["witness_ms"].as_f64().unwrap(),
                        metrics["commit_ms"].as_f64().unwrap(),
                        metrics["piop_ms"].as_f64().unwrap(),
                        metrics["pcs_ms"].as_f64().unwrap(),
                        metrics["witness_to_proof_ms"].as_f64().unwrap(),
                        timing.proof_bytes,
                    );
                    if trial > 0 {
                        measured.push(metrics);
                    }
                }
                let medians = summarize(&measured);
                write!(csv, "{},{backend},{n},{reps},{setup_ms}", workload.slug())?;
                for &key in MEDIAN_METRICS {
                    write!(csv, ",{}", medians[key])?;
                }
                let peak_rss_bytes = memory_sample.as_ref().map(|sample| sample.peak_rss_bytes);
                writeln!(
                    csv,
                    ",{}",
                    peak_rss_bytes
                        .map(|bytes| bytes.to_string())
                        .unwrap_or_default()
                )?;
                csv.flush()?;
                println!(
                    "RESULT schema=native-mul/2 workload={} backend={backend} log_multiplications={n} samples={reps} witness_ms={} online_prover_ms={} verify_ms={} proof_bytes={} peak_rss_bytes={}",
                    workload.slug(),
                    medians["witness_ms"],
                    medians["online_prover_ms"],
                    medians["verify_ms"],
                    medians["proof_bytes"],
                    peak_rss_bytes
                        .map(|bytes| bytes.to_string())
                        .unwrap_or_else(|| "na".into())
                );
                summary.push(json!({"workload":workload.slug(),"backend":backend,"log_multiplications":n,"samples":reps,"setup_ms":setup_ms,"config":config,"corpus_digest":corpus.digest,"medians":medians,"peak_rss_bytes":peak_rss_bytes,"memory":memory_sample}));
            }
        }
    }
    serde_json::to_writer_pretty(create("summary.json")?, &summary)?;
    eprintln!("Native multiplication results: {}", out.display());
    Ok(())
}

fn summarize(measured: &[Value]) -> serde_json::Map<String, Value> {
    MEDIAN_METRICS
        .iter()
        .map(|&key| {
            let values: Vec<_> = measured
                .iter()
                .map(|m| m[key].as_f64().expect("required measured metric"))
                .collect();
            (key.into(), json!(common::median(&values)))
        })
        .collect()
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
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
        let summary = summarize(&measured);
        assert_eq!(measured[0]["proof_bytes"], 1024);
        assert_eq!(summary["proof_bytes"], 2048.0);
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
        for (workload, seed, expected) in [
            (
                Workload::U32,
                common::mul_witness::U32_SEED,
                "fa3ea7841a92dcd2a010fc98a8b3d94e90c4dfea5128918ab5630298ee4eada5",
            ),
            (
                Workload::BabyBear,
                common::mul_witness::BABY_BEAR_SEED,
                "89d76aeb6147536c07407653efa8ce852d81e9d194f2adb7d246fcf9aea2830d",
            ),
        ] {
            assert_eq!(
                Corpus::new(workload, 15, common::mul_witness::shape_seed(seed, 15)).digest,
                expected
            );
            let a = Corpus::new(workload, 4, 7);
            let b = Corpus::new(workload, 4, 7);
            assert_eq!(a.digest, b.digest);
            assert_eq!(a.operands, b.operands);
            assert_ne!(a.digest, Corpus::new(workload, 4, 8).digest);
        }
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
        assert!(a.wide_inputs().iter().any(|&(x, _)| x > u128::from(u64::MAX)));
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
        use f2z::piop::spartan::U128MulLayout;
        let inputs = corpus.wide_inputs();
        assert_eq!(rows.len(), inputs.len(), "native witness row count");
        let layout = U128MulLayout::new(rows.len()).expect("canonical u128 layout");
        let capacity = layout.capacity();
        let mut hash = blake3::Hasher::new();
        hash.update(b"f2z/u128-mul-compare/integer-witness/v1");
        hash.update(&(layout.assignment_len() as u64).to_le_bytes());
        let mut entries = vec![(0_u128, 0_u128); layout.assignment_len()];
        entries[0] = (1, 0);
        for (i, (&[x, y, lo, hi], &(expected_x, expected_y))) in rows.iter().zip(inputs).enumerate() {
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
        use f2z::piop::spartan::{BabyBearMulLayout, U32MulLayout, U64MulLayout};
        let (capacity, assignment_len) = match corpus.workload {
            Workload::U32 => {
                let layout = U32MulLayout::new(n).expect("canonical u32 layout");
                (layout.capacity(), layout.assignment_len())
            }
            Workload::BabyBear => {
                let layout = BabyBearMulLayout::new(n).expect("canonical BabyBear layout");
                (layout.capacity(), layout.assignment_len())
            }
            Workload::U64 => {
                let layout = U64MulLayout::new(n).expect("canonical u64 layout");
                (layout.capacity(), layout.assignment_len())
            }
            Workload::U128 => unreachable!("wide corpora are audited by check_wide"),
        };
        let mut assignment = vec![0u64; assignment_len];
        assignment[0] = 1;
        let has_fourth_block = matches!(corpus.workload, Workload::BabyBear | Workload::U64);
        for (i, (&[a, b, c, q], &(expected_a, expected_b))) in
            rows.iter().zip(inputs).enumerate()
        {
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
        let domain: &[u8] = match corpus.workload {
            Workload::U32 => b"f2z/u32-pcs-compare/integer-witness/v1",
            Workload::BabyBear => b"f2z/baby-bear-pcs-compare/integer-witness/v1",
            Workload::U64 => b"f2z/u64-mul-compare/integer-witness/v1",
            Workload::U128 => unreachable!(),
        };
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
fn audit_backend(backend: &str, corpus: &Corpus) -> WitnessAudit {
    match backend {
        "f2z" => f2z_backend::audit(corpus),
        "binius64" => binius::audit(corpus),
        "plonky3-whir" => plonky3::audit(corpus),
        "limber" => limber_backend::audit(corpus),
        _ => unreachable!(),
    }
}

#[allow(dead_code)] // Invoked by the separate mul_witness_compare entry point.
pub(crate) fn witness_main() -> Result<(), Box<dyn std::error::Error>> {
    let threads = common::init();
    let reps = common::reps(None, 5);
    let workloads = choices(
        "F2Z_MUL_COMPARE_WORKLOADS",
        "u32 babybear",
        &["u32", "babybear", "u64", "u128"],
    );
    let backends = choices(
        "F2Z_MUL_COMPARE_BACKENDS",
        "f2z binius64 plonky3-whir limber",
        &["f2z", "binius64", "plonky3-whir", "limber"],
    );
    check_backend_support(&workloads, &backends);
    let shapes = common::shapes(None).unwrap_or_else(|| vec!["10".into()]);
    let exponents: Vec<usize> = shapes
        .iter()
        .map(|s| s.parse().expect("integer exponent"))
        .collect();
    let max_exponent = max_exponent(&workloads);
    assert!(
        exponents.iter().all(|n| (4..=max_exponent).contains(n)),
        "witness exponents must be 4..={max_exponent} for the selected workloads"
    );
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let out = std::env::var_os("F2Z_MUL_COMPARE_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("PerfRuns/{stamp}-mul-witness-compare")));
    fs::create_dir_all(&out)?;
    let create = |name: &str| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(name))
    };
    let mut raw = BufWriter::new(create("witness-checks.jsonl")?);
    let mut summary = vec![];
    for workload in workloads {
        let workload = Workload::parse(&workload);
        let seed = common::seed(None, root_seed(workload));
        for &n in &exponents {
            let shape_seed = common::mul_witness::shape_seed(seed, n);
            let corpus = Corpus::new(workload, n, shape_seed);
            for backend in &backends {
                let mut times = vec![];
                for trial in 0..=reps {
                    let audit = audit_backend(backend, &corpus);
                    let trial = if trial == 0 {
                        json!({"kind":"warmup","warmup_index":0})
                    } else {
                        times.push(audit.generation_ms);
                        json!({"kind":"sample","sample_index":trial-1})
                    };
                    writeln!(
                        raw,
                        "{}",
                        json!({"schema":"native-mul-witness/v1","workload":workload.slug(),"backend":backend,"log_multiplications":n,"threads":threads,"seed":shape_seed,"trial":trial,"witness_generation_ms":audit.generation_ms,"witness_digest_blake3":audit.digest,"expected_digest":corpus.digest,"native_representation":audit.representation,"quotient_reconstructed":audit.quotient_reconstructed,"all_rows_match":true})
                    )?;
                    raw.flush()?;
                }
                let median = common::median(&times);
                eprintln!(
                    "{} {backend} 2^{n}: all rows match, digest {}, witness {median:.3} ms",
                    workload.slug(),
                    corpus.digest
                );
                summary.push(json!({"workload":workload.slug(),"backend":backend,"log_multiplications":n,"samples":reps,"witness_ms":median,"witness_digest_blake3":corpus.digest,"all_rows_match":true}));
            }
        }
    }
    serde_json::to_writer_pretty(create("witness-summary.json")?, &summary)?;
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
        Workload::BabyBear => BABY_P - 1,
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
        for workload in [
            Workload::U32,
            Workload::BabyBear,
            Workload::U64,
            Workload::U128,
        ] {
            let corpus = edge_corpus(workload);
            for backend in ["f2z", "binius64", "plonky3-whir", "limber"] {
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
