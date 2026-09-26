//! Complete Falcon hybrid proof benchmark, including all source commitments.
//!
//! Example: `RAYON_NUM_THREADS=16 BITZ_BENCH_LAMBDA=128 cargo bench
//! --features falcon-hybrid --bench falcon_hybrid -- --batch 256 --iterations 2`.
//! Security must be selected explicitly. Each trial commits a fresh batch and
//! verifies the resulting full proof against all roots. A fixture manifest is a
//! JSON array of {"public_key": "path", "signature": "path", "message": "path"},
//! with paths relative to the manifest. Entries cycle across the batch, starting
//! at a different offset each trial; without a manifest the bundled fixture repeats.
use bitz::piop::spartan::falcon1024_ct::{FalconPublicStatement, PreparedFalconHybrid};
use serde_json::json;
use std::{error::Error, fmt::Write, fs, path::PathBuf, time::Instant};

struct Options {
    batch: usize,
    security: usize,
    iterations: usize,
    warmup: usize,
    threads: usize,
    fixture_manifest: Option<PathBuf>,
}

impl Options {
    fn read() -> Result<Self, Box<dyn Error>> {
        let mut batch = env_usize("BITZ_FALCON_BATCH")?.unwrap_or(1);
        let mut security = env_usize("BITZ_BENCH_LAMBDA")?;
        let mut iterations = env_usize("BITZ_BENCH_REPS")?.unwrap_or(3);
        let mut warmup = 1;
        let mut threads = env_usize("RAYON_NUM_THREADS")?.unwrap_or(1);
        let mut fixture_manifest =
            std::env::var_os("BITZ_FALCON_FIXTURE_MANIFEST").map(PathBuf::from);
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--bench" {
                continue;
            }
            if matches!(flag.as_str(), "--help" | "-h") {
                println!(
                    "falcon_hybrid --security 100|128 [--batch 1..1024] [--iterations N] [--warmup N] [--threads N] [--fixture-manifest PATH]\nEnvironment defaults: BITZ_BENCH_LAMBDA, BITZ_FALCON_BATCH, BITZ_BENCH_REPS, RAYON_NUM_THREADS.\nOptional fixture paths: BITZ_FALCON_PUBLIC_KEY, BITZ_FALCON_SIGNATURE, BITZ_FALCON_MESSAGE.\nVaried batches: --fixture-manifest PATH or BITZ_FALCON_FIXTURE_MANIFEST (JSON array of public_key/signature/message paths)."
                );
                std::process::exit(0);
            }
            if flag == "--fixture-manifest" {
                fixture_manifest = Some(args.next().ok_or("missing fixture manifest path")?.into());
                continue;
            }
            if !matches!(
                flag.as_str(),
                "--batch"
                    | "--security"
                    | "--lambda"
                    | "--iterations"
                    | "--reps"
                    | "--warmup"
                    | "--threads"
            ) {
                return Err(format!("unknown argument {flag}").into());
            }
            let value: usize = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?
                .parse()?;
            match flag.as_str() {
                "--batch" => batch = value,
                "--security" | "--lambda" => security = Some(value),
                "--iterations" | "--reps" => iterations = value,
                "--warmup" => warmup = value,
                "--threads" => threads = value,
                _ => unreachable!(),
            }
        }
        let security =
            security.ok_or("select --security 100|128 or BITZ_BENCH_LAMBDA explicitly")?;
        if !matches!(security, 100 | 128) {
            return Err("security must be 100 or 128".into());
        }
        if !(1..=1024).contains(&batch) {
            return Err("batch must be in 1..=1024".into());
        }
        if iterations == 0 || threads == 0 {
            return Err("iterations and threads must be positive".into());
        }
        if warmup.checked_add(iterations).is_none() {
            return Err("trial count overflow".into());
        }
        Ok(Self {
            batch,
            security,
            iterations,
            warmup,
            threads,
            fixture_manifest,
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::read()?;
    #[cfg(feature = "parallel")]
    rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads)
        .build_global()?;
    #[cfg(not(feature = "parallel"))]
    if options.threads != 1 {
        return Err("multiple threads require the parallel feature".into());
    }
    if std::env::var_os("BITZ_FALCON_STAGE_TIMINGS").is_some() {
        use tracing_subscriber::prelude::*;
        tracing_subscriber::registry()
            .with(StageTimings)
            .try_init()?;
    } else {
        bitz::observability::install().expect("install Perfetto subscriber");
    }
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;

    let fixtures = Fixtures::read(&options)?;
    let fixture_hash = fixtures.digest();

    let start = Instant::now();
    let prepared = PreparedFalconHybrid::new(options.batch, options.security)?;
    let prepare_ms = ms(start);
    let security = prepared.security();
    let security_terms: Vec<_> = security
        .terms
        .iter()
        .map(|&(name, error)| {
            json!({
                "name": name,
                "error_bound": error,
                "bits": if error > 0.0 { Some(-error.log2()) } else { None },
            })
        })
        .collect();
    println!(
        "{}",
        json!({
            "schema": "bitz/falcon-hybrid/v1",
            "event": "prepared",
            "batch": options.batch,
            "capacity": prepared.capacity(),
            "security_target": options.security,
            "algebraic_security_bits": security.algebraic_bits,
            "security_terms": security_terms,
            "threads": threads,
            "build_rustflags": option_env!("RUSTFLAGS"),
            "runtime_rustflags": std::env::var("RUSTFLAGS").ok(),
            "target_arch": std::env::consts::ARCH,
            "gf128_kernel": field::gf128::KERNEL,
            "stage_timings": std::env::var_os("BITZ_FALCON_STAGE_TIMINGS").is_some(),
            "compiled_target_features": {
                "pclmulqdq": cfg!(target_feature = "pclmulqdq"),
                "sse4.1": cfg!(target_feature = "sse4.1"),
                "avx2": cfg!(target_feature = "avx2"),
                "aes": cfg!(target_feature = "aes"),
                "avx512f": cfg!(target_feature = "avx512f"),
                "vpclmulqdq": cfg!(target_feature = "vpclmulqdq"),
                "gfni": cfg!(target_feature = "gfni"),
            },
            "prepare_ms": prepare_ms,
            "source_bits_per_signature": prepared.source_bits_per_signature(),
            "fixture_digest": fixture_hash,
            "fixture_repeated": fixtures.0.len() == 1,
            "fixture_count": fixtures.0.len(),
            "fixture_manifest": options.fixture_manifest,
            "fixture_trial_rotation": fixtures.0.len() > 1,
            "warmup_trials": options.warmup,
            "measured_trials": options.iterations,
        })
    );

    let mut samples = Vec::with_capacity(options.iterations);
    for trial in 0..options.warmup + options.iterations {
        let offset = trial % fixtures.0.len();
        let batch: Vec<_> = (0..options.batch)
            .map(|i| &fixtures.0[(offset + i) % fixtures.0.len()])
            .collect();
        let keys: Vec<_> = batch.iter().map(|f| f.public_key.as_slice()).collect();
        let messages: Vec<_> = batch.iter().map(|f| f.message.as_slice()).collect();
        let signatures: Vec<_> = batch.iter().map(|f| f.signature.as_slice()).collect();
        let public = FalconPublicStatement::from_bytes(&keys, &messages)?;
        let mut batch_hash = blake3::Hasher::new();
        for fixture in &batch {
            fixture.hash_into(&mut batch_hash);
        }
        let batch_digest = batch_hash.finalize().to_hex().to_string();
        let phase = tracing::info_span!("falcon_bench:commit", trial).entered();
        let start = Instant::now();
        let committed = prepared.commit(public, &signatures)?;
        let statement = committed.statement.clone();
        let witness_commit_ms = ms(start);
        drop(phase);
        let phase = tracing::info_span!("falcon_bench:prove", trial).entered();
        let start = Instant::now();
        let proof = prepared.prove(committed)?;
        let prove_ms = ms(start);
        drop(phase);
        let phase = tracing::info_span!("falcon_bench:verify", trial).entered();
        let start = Instant::now();
        prepared.verify(&statement, &proof)?;
        let verify_ms = ms(start);
        drop(phase);
        // An exact-build comparison aid, not a stable serialization or wire digest.
        // Stream the complete Debug representation to avoid allocating its string.
        let mut proof_hash = DebugHasher(blake3::Hasher::new());
        write!(&mut proof_hash, "{proof:?}")?;
        let proof_debug_digest = proof_hash.0.finalize().to_hex().to_string();
        let measured = trial >= options.warmup;
        if measured {
            samples.push([witness_commit_ms, prove_ms, verify_ms]);
        }
        println!(
            "{}",
            json!({
                "schema": "bitz/falcon-hybrid/v1",
                "event": "trial",
                "trial": if measured { "sample" } else { "warmup" },
                "sample": if measured { Some(trial - options.warmup + 1) } else { None },
                "batch": options.batch,
                "capacity": prepared.capacity(),
                "security_target": options.security,
                "threads": threads,
                "prepare_ms": prepare_ms,
                "witness_commit_ms": witness_commit_ms,
                "proof_prove_ms": prove_ms,
                "proof_verify_ms": verify_ms,
                "total_prover_ms": witness_commit_ms + prove_ms,
                "end_to_end_ms": witness_commit_ms + prove_ms + verify_ms,
                "process_peak_rss_kib": process_peak_rss_kib(),
                "roots": statement.roots.map(|root| root.iter().map(|byte| format!("{byte:02x}")).collect::<String>()),
                "fixture_offset": offset,
                "batch_fixture_digest": batch_digest,
                "proof_debug_digest": proof_debug_digest,
                "verified": true,
            })
        );
    }
    let witness_commit_ms = median(samples.iter().map(|sample| sample[0]).collect());
    let prove_ms = median(samples.iter().map(|sample| sample[1]).collect());
    let verify_ms = median(samples.iter().map(|sample| sample[2]).collect());
    let total_prover_ms = median(samples.iter().map(|sample| sample[0] + sample[1]).collect());
    println!(
        "{}",
        json!({
            "schema": "bitz/falcon-hybrid/v1",
            "event": "summary",
            "batch": options.batch,
            "capacity": prepared.capacity(),
            "security_target": options.security,
            "threads": threads,
            "samples": samples.len(),
            "prepare_ms": prepare_ms,
            "median_witness_commit_ms": witness_commit_ms,
            "median_proof_prove_ms": prove_ms,
            "median_proof_verify_ms": verify_ms,
            "median_total_prover_ms": total_prover_ms,
            "median_end_to_end_ms": median(samples.iter().map(|sample| sample.iter().sum()).collect()),
            "proof_signatures_per_second": 1000.0 * options.batch as f64 / prove_ms,
            "total_prover_signatures_per_second": 1000.0 * options.batch as f64 / total_prover_ms,
            "process_peak_rss_kib": process_peak_rss_kib(),
            "verified": true,
        })
    );
    Ok(())
}

/// Diagnostic wall times; kept out of the normal benchmark's subscriber so
/// span logging cannot bias the matched latency runs. Nested spans overlap.
struct StageTimings;
impl<S> tracing_subscriber::Layer<S> for StageTimings
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        _: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            if span.name().starts_with("falcon") || span.name() == "spartan:grinding" {
                span.extensions_mut().insert(Instant::now());
            }
        }
    }
    fn on_close(&self, id: tracing::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            if let Some(start) = span.extensions().get::<Instant>() {
                eprintln!(
                    "{}",
                    json!({"event": "stage", "name": span.name(),
                    "parent": span.parent().map(|parent| parent.name()),
                    "span_id": span.id().into_u64(),
                    "ancestor_ids": span.scope().from_root().map(|ancestor| ancestor.id().into_u64()).collect::<Vec<_>>(),
                    "path": span.scope().from_root().map(|ancestor| ancestor.name()).collect::<Vec<_>>(),
                    "elapsed_ms": start.elapsed().as_secs_f64() * 1000.0})
                );
            }
        }
    }
}

/// Hashing Debug is useful for paired builds of the same source revision. It is
/// deliberately not advertised as a canonical proof encoding.
struct DebugHasher(blake3::Hasher);
impl std::fmt::Write for DebugHasher {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0.update(value.as_bytes());
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePaths {
    public_key: PathBuf,
    signature: PathBuf,
    message: PathBuf,
}

struct Fixture {
    public_key: Vec<u8>,
    signature: Vec<u8>,
    message: Vec<u8>,
}

impl Fixture {
    fn hash_into(&self, hash: &mut blake3::Hasher) {
        for part in [&self.public_key, &self.message, &self.signature] {
            hash.update(&(part.len() as u64).to_le_bytes());
            hash.update(part);
        }
    }
}

struct Fixtures(Vec<Fixture>);
impl Fixtures {
    fn read(options: &Options) -> Result<Self, Box<dyn Error>> {
        if let Some(path) = &options.fixture_manifest {
            let entries: Vec<FixturePaths> = serde_json::from_slice(&fs::read(path)?)?;
            if entries.is_empty() {
                return Err("fixture manifest must not be empty".into());
            }
            let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let fixtures = entries
                .into_iter()
                .map(|entry| {
                    Ok(Fixture {
                        public_key: fs::read(directory.join(entry.public_key))?,
                        signature: fs::read(directory.join(entry.signature))?,
                        message: fs::read(directory.join(entry.message))?,
                    })
                })
                .collect::<Result<Vec<_>, std::io::Error>>()?;
            return Ok(Self(fixtures));
        }
        Ok(Self(vec![Fixture {
            public_key: fixture(
                "BITZ_FALCON_PUBLIC_KEY",
                include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/public_key.bin"),
            )?,
            signature: fixture(
                "BITZ_FALCON_SIGNATURE",
                include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/signature_ct.bin"),
            )?,
            message: fixture(
                "BITZ_FALCON_MESSAGE",
                include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/message.bin"),
            )?,
        }]))
    }

    fn digest(&self) -> String {
        let mut hash = blake3::Hasher::new();
        for fixture in &self.0 {
            fixture.hash_into(&mut hash);
        }
        hash.finalize().to_hex().to_string()
    }
}

fn env_usize(name: &str) -> Result<Option<usize>, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value.parse()?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn fixture(name: &str, fallback: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(match std::env::var_os(name) {
        Some(path) => fs::read(path)?,
        None => fallback.to_vec(),
    })
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) * 0.5
    } else {
        values[middle]
    }
}

/// Linux's process-lifetime high-water mark; this is not a per-trial delta.
fn process_peak_rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
