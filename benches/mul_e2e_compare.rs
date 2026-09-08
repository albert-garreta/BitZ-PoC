//! Native end-to-end multiplication proofs. See docs/native-mul-compare.md.
#[path = "mul_e2e_compare/binius.rs"]
mod binius;
mod common;
#[path = "mul_e2e_compare/f2z.rs"]
mod f2z_backend;
#[path = "mul_e2e_compare/limber.rs"]
mod limber_backend;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workload {
    U32,
    BabyBear,
}
impl Workload {
    fn slug(self) -> &'static str {
        match self {
            Self::U32 => "u32",
            Self::BabyBear => "babybear",
        }
    }
    fn algorithm(self) -> &'static str {
        match self {
            Self::U32 => "u32 multiplication",
            Self::BabyBear => "BabyBear multiplication",
        }
    }
    fn output(self, a: u32, b: u32) -> u64 {
        let product = u64::from(a) * u64::from(b);
        match self {
            Self::U32 => product,
            Self::BabyBear => product % BABY_P,
        }
    }
}

struct Corpus {
    workload: Workload,
    inputs: Vec<(u32, u32)>,
    digest: String,
}
impl Corpus {
    fn new(workload: Workload, exponent: usize, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let inputs: Vec<_> = (0..1usize << exponent)
            .map(|_| match workload {
                Workload::U32 => (rng.random(), rng.random()),
                Workload::BabyBear => {
                    let mut next = || rng.random();
                    (
                        f2z::piop::spartan::sample_baby_bear_operand_with(&mut next),
                        f2z::piop::spartan::sample_baby_bear_operand_with(&mut next),
                    )
                }
            })
            .collect();
        Self::from_inputs(workload, inputs)
    }
    fn from_inputs(workload: Workload, inputs: Vec<(u32, u32)>) -> Self {
        use f2z::piop::spartan::{BabyBearMulWitness, U32MulWitness};
        let digest = match workload {
            Workload::U32 => common::mul_witness::u32_digest(
                &U32MulWitness::from_inputs(&inputs).expect("canonical u32 witness"),
            ),
            Workload::BabyBear => common::mul_witness::baby_bear_digest(
                &BabyBearMulWitness::from_inputs(&inputs).expect("canonical BabyBear witness"),
            ),
        };
        Self {
            workload,
            inputs,
            digest,
        }
    }
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
    proof_bytes: Option<usize>,
}
impl Timing {
    fn new(start: u64, witness_end: u64, ready: u64, verify_start: u64, end: u64) -> Self {
        let mut result = Self {
            phases: vec![],
            proof_bytes: None,
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
    let reps = common::reps(None, 5);
    assert!(reps > 0);
    let workloads = choices(
        "F2Z_MUL_COMPARE_WORKLOADS",
        "u32 babybear",
        &["u32", "babybear"],
    );
    let backends = choices(
        "F2Z_MUL_COMPARE_BACKENDS",
        "f2z binius64 plonky3-whir limber",
        &["f2z", "binius64", "plonky3-whir", "limber"],
    );
    let shapes = common::shapes(None).unwrap_or_else(|| vec!["15".into()]);
    let mut exponents = vec![];
    for shape in shapes {
        let n: usize = shape.parse()?;
        assert!((4..=24).contains(&n), "F2Z_BENCH_SHAPES must be in 4..=24");
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
    let mut csv = BufWriter::new(create("metrics.csv")?);
    writeln!(
        csv,
        "workload,backend,log_multiplications,samples,setup_ms,witness_ms,commit_ms,piop_ms,opening_ms,pcs_ms,online_prover_ms,witness_to_proof_ms,verify_ms"
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
        let workload = if workload == "u32" {
            Workload::U32
        } else {
            Workload::BabyBear
        };
        let seed = common::seed(
            None,
            match workload {
                Workload::U32 => common::mul_witness::U32_SEED,
                Workload::BabyBear => common::mul_witness::BABY_BEAR_SEED,
            },
        );
        for &n in &exponents {
            let shape_seed = common::mul_witness::shape_seed(seed, n);
            let corpus = Arc::new(Corpus::new(workload, n, shape_seed));
            for backend in &backends {
                eprintln!("{} {backend} 2^{n}: setup", workload.slug());
                // Check the actual native materialization before accepting any proof timings.
                let audit = audit_backend(backend, &corpus);
                assert_eq!(audit.digest, corpus.digest);
                let started = std::time::Instant::now();
                let context = Context::setup(backend, Arc::clone(&corpus));
                let setup_ms = started.elapsed().as_secs_f64() * 1e3;
                let config = context.config();
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
                        "{} {backend} 2^{n} {}: witness {:.3} ms, commit {:.3} ms, PIOP {:.3} ms, PCS {:.3} ms, witness→proof {:.3} ms",
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
                        metrics["witness_to_proof_ms"].as_f64().unwrap()
                    );
                    if trial > 0 {
                        measured.push(metrics);
                    }
                }
                let mut medians = serde_json::Map::new();
                for key in [
                    "witness_ms",
                    "commit_ms",
                    "piop_ms",
                    "opening_ms",
                    "pcs_ms",
                    "online_prover_ms",
                    "witness_to_proof_ms",
                    "verify_ms",
                ] {
                    let values: Vec<_> =
                        measured.iter().map(|m| m[key].as_f64().unwrap()).collect();
                    medians.insert(key.into(), json!(common::median(&values)));
                }
                write!(csv, "{},{backend},{n},{reps},{setup_ms}", workload.slug())?;
                for key in [
                    "witness_ms",
                    "commit_ms",
                    "piop_ms",
                    "opening_ms",
                    "pcs_ms",
                    "online_prover_ms",
                    "witness_to_proof_ms",
                    "verify_ms",
                ] {
                    write!(csv, ",{}", medians[key])?;
                }
                writeln!(csv)?;
                csv.flush()?;
                summary.push(json!({"workload":workload.slug(),"backend":backend,"log_multiplications":n,"samples":reps,"setup_ms":setup_ms,"config":config,"corpus_digest":corpus.digest,"medians":medians}));
            }
        }
    }
    serde_json::to_writer_pretty(create("summary.json")?, &summary)?;
    eprintln!("Native multiplication results: {}", out.display());
    Ok(())
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_phases_are_unioned() {
        let mut t = Timing {
            phases: vec![],
            proof_bytes: None,
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
            assert_eq!(a.inputs, b.inputs);
            assert_ne!(a.digest, Corpus::new(workload, 4, 8).digest);
        }
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
    fn check(
        corpus: &Corpus,
        rows: Vec<[u64; 4]>,
        generation_ms: f64,
        representation: &'static str,
        quotient_reconstructed: bool,
    ) -> Self {
        assert_eq!(rows.len(), corpus.inputs.len(), "native witness row count");
        let n = rows.len();
        use f2z::piop::spartan::{BabyBearMulLayout, U32MulLayout};
        let (capacity, assignment_len) = match corpus.workload {
            Workload::U32 => {
                let layout = U32MulLayout::new(n).expect("canonical u32 layout");
                (layout.capacity(), layout.assignment_len())
            }
            Workload::BabyBear => {
                let layout = BabyBearMulLayout::new(n).expect("canonical BabyBear layout");
                (layout.capacity(), layout.assignment_len())
            }
        };
        let mut assignment = vec![0u64; assignment_len];
        assignment[0] = 1;
        for (i, (&[a, b, c, q], &(expected_a, expected_b))) in
            rows.iter().zip(&corpus.inputs).enumerate()
        {
            let expected_q = if corpus.workload == Workload::BabyBear {
                (expected_a as u64 * expected_b as u64) / BABY_P
            } else {
                0
            };
            assert_eq!(
                [a, b, c, q],
                [
                    expected_a as u64,
                    expected_b as u64,
                    corpus.workload.output(expected_a, expected_b),
                    expected_q
                ],
                "native witness mismatch at row {i} ({representation})"
            );
            assignment[capacity + i] = a;
            assignment[2 * capacity + i] = b;
            assignment[3 * capacity + i] = c;
            if corpus.workload == Workload::BabyBear {
                assignment[4 * capacity + i] = q;
            }
        }
        let domain: &[u8] = match corpus.workload {
            Workload::U32 => b"f2z/u32-pcs-compare/integer-witness/v1",
            Workload::BabyBear => b"f2z/baby-bear-pcs-compare/integer-witness/v1",
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
        &["u32", "babybear"],
    );
    let backends = choices(
        "F2Z_MUL_COMPARE_BACKENDS",
        "f2z binius64 plonky3-whir limber",
        &["f2z", "binius64", "plonky3-whir", "limber"],
    );
    let shapes = common::shapes(None).unwrap_or_else(|| vec!["10".into()]);
    let exponents: Vec<usize> = shapes
        .iter()
        .map(|s| s.parse().expect("integer exponent"))
        .collect();
    assert!(
        exponents.iter().all(|n| (4..=24).contains(n)),
        "witness exponents must be 4..=24"
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
        let workload = if workload == "u32" {
            Workload::U32
        } else {
            Workload::BabyBear
        };
        let seed = common::seed(
            None,
            match workload {
                Workload::U32 => common::mul_witness::U32_SEED,
                Workload::BabyBear => common::mul_witness::BABY_BEAR_SEED,
            },
        );
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

#[cfg(test)]
#[allow(unused_imports)]
mod witness_tests {
    use super::*;
    #[test]
    fn all_native_witnesses_recover_the_same_assignment() {
        for workload in [Workload::U32, Workload::BabyBear] {
            let max = if workload == Workload::U32 {
                u32::MAX
            } else {
                (BABY_P - 1) as u32
            };
            let corpus =
                Corpus::from_inputs(workload, [(0, 0), (0, max), (1, max), (max, max)].repeat(4));
            for backend in ["f2z", "binius64", "plonky3-whir", "limber"] {
                assert_eq!(audit_backend(backend, &corpus).digest, corpus.digest);
            }
        }
    }
    #[test]
    fn witness_check_rejects_a_changed_native_row() {
        let corpus = Corpus::new(Workload::U32, 4, 7);
        let mut rows: Vec<_> = corpus
            .inputs
            .iter()
            .map(|&(a, b)| [a as u64, b as u64, corpus.workload.output(a, b), 0])
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
