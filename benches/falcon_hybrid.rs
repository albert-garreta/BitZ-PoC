#![recursion_limit = "256"]
//! Complete Falcon hybrid proof benchmark, including all source commitments.
//!
//! Example: `RAYON_NUM_THREADS=16 BITZ_BENCH_LAMBDA=128 cargo bench
//! --features falcon-hybrid --bench falcon_hybrid -- --batch 32 --iterations 2`.
//! Security must be selected explicitly. Each trial commits a fresh batch and
//! verifies the resulting full proof against its joint source root. fn-dsa 0.3.0
//! generates distinct original Falcon-512 or Falcon-1024 keypairs/messages/signatures once,
//! outside prover timing, and the same batch is reused across trials.
#[path = "common/falcon_affinity.rs"]
mod falcon_affinity;
#[path = "common/falcon_degree_inputs.rs"]
mod falcon_degree_inputs;
#[path = "common/falcon_stage_timings.rs"]
mod falcon_stage_timings;
use bitz::piop::spartan::falcon_parameters::{auto_k, ring_budget_ok};
use bitz::piop::spartan::falcon_profiles::{BackendSelection, FalconBackend};
use falcon_degree_inputs::{generate_cases, reject_fixture_overrides};
use falcon_stage_timings::StageTimings;
use serde_json::json;
use std::{error::Error, fmt::Write, fs, time::Instant};

struct Options {
    degree: usize,
    extension: usize,
    extension_selection: String,
    batch: usize,
    security: usize,
    iterations: usize,
    warmup: usize,
    threads: usize,
    seed: u64,
}

impl Options {
    fn read() -> Result<Self, Box<dyn Error>> {
        let mut degree = 1024;
        let mut extension_selection = String::from("auto");
        let mut batch = env_usize("BITZ_FALCON_BATCH")?.unwrap_or(32);
        let mut security = env_usize("BITZ_BENCH_LAMBDA")?;
        let mut iterations = env_usize("BITZ_BENCH_REPS")?.unwrap_or(3);
        let mut warmup = 1;
        let mut threads = env_usize("RAYON_NUM_THREADS")?.unwrap_or(1);
        let mut seed = std::env::var("BITZ_FALCON_SEED")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()?
            .unwrap_or(42);
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--bench" {
                continue;
            }
            if matches!(flag.as_str(), "--help" | "-h") {
                println!(
                    "falcon_hybrid --security 100|128 [--degree 512|1024] [--k auto|9|10|11] [--batch 1..1024] [--seed U64] [--iterations N] [--warmup N] [--threads N]\nInputs: distinct fn-dsa 0.3.0 original Falcon-512 or Falcon-1024 keys and signatures; generation is outside prover timing. Defaults: degree 1024, K auto, max batch 1024, batch 32, seed 42.\nEnvironment defaults: BITZ_BENCH_LAMBDA, BITZ_FALCON_BATCH, BITZ_FALCON_SEED, BITZ_BENCH_REPS, RAYON_NUM_THREADS."
                );
                std::process::exit(0);
            }
            if flag == "--k" {
                extension_selection = args.next().ok_or("missing K selection")?;
                continue;
            }
            if flag == "--seed" {
                seed = args.next().ok_or("missing seed")?.parse()?;
                continue;
            }
            if !matches!(
                flag.as_str(),
                "--batch" | "--degree" | "--security" | "--iterations" | "--warmup" | "--threads"
            ) {
                return Err(format!("unknown argument {flag}").into());
            }
            let value: usize = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?
                .parse()?;
            match flag.as_str() {
                "--batch" => batch = value,
                "--degree" => degree = value,
                "--security" => security = Some(value),
                "--iterations" => iterations = value,
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
        if !matches!(degree, 512 | 1024) {
            return Err("degree must be 512 or 1024".into());
        }
        let extension = if extension_selection.eq_ignore_ascii_case("auto") {
            extension_selection = String::from("auto");
            auto_k(degree, security, 1024)
        } else {
            let explicit = extension_selection.parse::<usize>()?;
            if !(9..=11).contains(&explicit) {
                return Err("explicit K must be in 9..=11".into());
            }
            extension_selection = format!("explicit({explicit})");
            explicit
        };
        if !ring_budget_ok(degree, extension, security, 1024) {
            return Err("explicit K misses the requested ring security budget".into());
        }
        Ok(Self {
            degree,
            extension,
            extension_selection,
            batch,
            security,
            iterations,
            warmup,
            threads,
            seed,
        })
    }
}

macro_rules! backend_runner {
    ($name:ident, $module:ident, $degree:expr, $extension:expr) => {
        fn $name(
            options: Options,
            mut cpu_affinity: Option<falcon_affinity::AffinityReport>,
            threads: usize,
        ) -> Result<(), Box<dyn Error>> {
            use bitz::piop::spartan::falcon_profiles::$module::FalconPublicStatement;
    let start = Instant::now();
    let cases = generate_cases(options.degree, options.batch, options.seed)?;
    let input_setup_ms = ms(start);
    let mut input_hash = blake3::Hasher::new();
    for case in &cases {
        case.hash_into(&mut input_hash);
    }
    let input_digest = input_hash.finalize().to_hex().to_string();
    let keys: Vec<_> = cases
        .iter()
        .map(|case| case.public_key.as_slice())
        .collect();
    let messages: Vec<_> = cases.iter().map(|case| case.message.as_slice()).collect();
    let signatures: Vec<_> = cases.iter().map(|case| case.signature.as_slice()).collect();

    let start = Instant::now();
    let prepared = <BackendSelection<$degree, $extension> as FalconBackend>::prepare(
        options.batch, options.security, 1024,
    )?;
    let protocol = bitz::piop::spartan::falcon::PROTOCOL_ID;
    let prepare_ms = ms(start);
    assert_eq!(prepared.capacity(), options.batch.next_power_of_two());
    let pcs_query_shape = pcs_query_shape(&prepared.source_bits_per_signature(), prepared.capacity(), options.security)?;
    let security = prepared.security();
    assert_eq!(security.target_bits, options.security);
    assert!(security.algebraic_bits >= options.security as f64);
    assert!(security.terms.iter().all(|(_, error)| error.is_finite() && *error >= 0.0));
    let (prime_min, prime_max) = prepared.prime_modulus_bounds();
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
            "schema": "bitz/falcon-hybrid/profile-v2",
            "source_layout": "aligned16-h2p-selection-v4",
            "degree": options.degree,
            "ring_extension": options.extension,
            "ring_extension_selection": options.extension_selection,
            "max_batch": 1024,
            "protocol": protocol,
            "integer_bridge": prepared.integer_bridge_name(),
            "arithmetic_prime_bits": u128::BITS - prime_max.leading_zeros(),
            "arithmetic_prime_min": prime_min.to_string(),
            "arithmetic_prime_max": prime_max.to_string(),
            "arithmetic_live_bits_per_signature": prepared.live_arithmetic_bits_per_signature(),
            "event": "prepared",
            "batch": options.batch,
            "capacity": prepared.capacity(),
            "security_target": options.security,
            "algebraic_security_bits": security.algebraic_bits,
            "security_terms": security_terms,
            "threads": threads,
            "cpu_affinity": cpu_affinity,
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
            "input_source": "pornin/rust-fn-dsa",
            "input_implementation_version": "0.3.0",
            "input_mode": format!("original-falcon-{}", options.degree),
            "input_seed": options.seed,
            "input_rng": "rand_chacha 0.3.1 ChaCha20Rng::seed_from_u64",
            "input_count": cases.len(),
            "input_digest": input_digest,
            "input_setup_ms": input_setup_ms,
            "native_verify_includes_public_key_decode": true,
            "public_inputs": ["public_key", "message", "signature_nonce", "signature_s2"],
            "source_bits_per_signature": prepared.source_bits_per_signature(),
            "pcs_query_shape": pcs_query_shape,
            "warmup_trials": options.warmup,
            "measured_trials": options.iterations,
        })
    );

    let mut samples = Vec::with_capacity(options.iterations);
    for trial in 0..options.warmup + options.iterations {
        let public = FalconPublicStatement::from_bytes(&keys, &messages, &signatures)?;
        let start = Instant::now();
        for case in &cases {
            std::hint::black_box(case).verify_upstream()?;
        }
        let native_verify_ms = ms(start);
        let phase = tracing::info_span!("falcon_bench:commit", trial).entered();
        let start = Instant::now();
        let committed = prepared.commit(public)?;
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
        let flock_verifier_affinity = falcon_affinity::verify_after_trial(&mut cpu_affinity)?;
        // An exact-build comparison aid, not a stable serialization or wire digest.
        // Stream the complete Debug representation to avoid allocating its string.
        let mut proof_hash = DebugHasher(blake3::Hasher::new());
        write!(&mut proof_hash, "{proof:?}")?;
        let proof_debug_digest = proof_hash.0.finalize().to_hex().to_string();
        // Stored nonce accounting is deliberately outside all timed sections.
        let grinding_diagnostics: Vec<_> = proof
            .grinding_diagnostics()
            .into_iter()
            .map(|(category, boundaries, prefix_sum)| {
                json!({
                    "category": category,
                    "stored_nonce_boundaries": boundaries,
                    "serial_nonce_prefix_sum": prefix_sum.to_string(),
                })
            })
            .collect();
        let payload_breakdown: std::collections::BTreeMap<_, _> =
            proof.payload_size_breakdown().into_iter().collect();
        assert_eq!(payload_breakdown.values().sum::<usize>(), proof.payload_size_bytes());
        let measured = trial >= options.warmup;
        if measured {
            samples.push(([witness_commit_ms, prove_ms, verify_ms], native_verify_ms));
        }
        println!(
            "{}",
            json!({
                "schema": "bitz/falcon-hybrid/profile-v2",
                "source_layout": "aligned16-h2p-selection-v4",
            "degree": options.degree,
            "ring_extension": options.extension,
            "ring_extension_selection": options.extension_selection,
            "max_batch": 1024,
                "event": "trial",
                "trial": if measured { "sample" } else { "warmup" },
                "sample": if measured { Some(trial - options.warmup + 1) } else { None },
                "batch": options.batch,
                "capacity": prepared.capacity(),
                "security_target": options.security,
                "threads": threads,
                "prepare_ms": prepare_ms,
                "input_setup_ms": input_setup_ms,
                "native_verify_ms": native_verify_ms,
                "witness_commit_ms": witness_commit_ms,
                "proof_prove_ms": prove_ms,
                "proof_verify_ms": verify_ms,
                "flock_verifier_affinity": flock_verifier_affinity,
                "total_prover_ms": witness_commit_ms + prove_ms,
                "end_to_end_ms": witness_commit_ms + prove_ms + verify_ms,
                "process_peak_rss_kib": process_peak_rss_kib(),
                "source_root": statement.source_root.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
                "input_digest": input_digest,
                "proof_debug_digest": proof_debug_digest,
                "grinding_diagnostics": {
                    "definition": "sum(nonce+1) over stored nonce entries; decimal strings preserve u128; includes zero-difficulty placeholders; excludes parallel/SIMD overscan; not executed hashes",
                    "categories": grinding_diagnostics,
                },
                "proof_payload_bytes": proof.payload_size_bytes(),
                "proof_payload_breakdown": payload_breakdown,
                "proof_payload_definition": "canonical stored payload; excludes Falcon framing and public statement",
                "verified": true,
            })
        );
    }
    let witness_commit_ms = median(samples.iter().map(|sample| sample.0[0]).collect());
    let prove_ms = median(samples.iter().map(|sample| sample.0[1]).collect());
    let verify_ms = median(samples.iter().map(|sample| sample.0[2]).collect());
    let total_prover_ms = median(
        samples
            .iter()
            .map(|sample| sample.0[0] + sample.0[1])
            .collect(),
    );
    let native_verify_ms = median(samples.iter().map(|sample| sample.1).collect());
    println!(
        "{}",
        json!({
            "schema": "bitz/falcon-hybrid/profile-v2",
            "source_layout": "aligned16-h2p-selection-v4",
            "degree": options.degree,
            "ring_extension": options.extension,
            "ring_extension_selection": options.extension_selection,
            "max_batch": 1024,
            "event": "summary",
            "batch": options.batch,
            "capacity": prepared.capacity(),
            "security_target": options.security,
            "threads": threads,
            "samples": samples.len(),
            "prepare_ms": prepare_ms,
            "input_setup_ms": input_setup_ms,
            "median_native_verify_ms": native_verify_ms,
            "median_witness_commit_ms": witness_commit_ms,
            "median_proof_prove_ms": prove_ms,
            "median_proof_verify_ms": verify_ms,
            "median_total_prover_ms": total_prover_ms,
            "median_end_to_end_ms": median(samples.iter().map(|sample| sample.0.iter().sum()).collect()),
            "proof_signatures_per_second": 1000.0 * options.batch as f64 / prove_ms,
            "total_prover_signatures_per_second": 1000.0 * options.batch as f64 / total_prover_ms,
            "process_peak_rss_kib": process_peak_rss_kib(),
            "verified": true,
        })
    );
    Ok(())
}
    };
}
backend_runner!(run_512_9, n512_k9, 512, 9);
backend_runner!(run_512_10, n512_k10, 512, 10);
backend_runner!(run_512_11, n512_k11, 512, 11);
backend_runner!(run_1024_9, n1024_k9, 1024, 9);
backend_runner!(run_1024_10, n1024_k10, 1024, 10);
backend_runner!(run_1024_11, n1024_k11, 1024, 11);

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::read()?;
    reject_fixture_overrides()?;
    let cpu_affinity = falcon_affinity::build_global_pool(options.threads)?;
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

    match (options.degree, options.extension) {
        (512, 9) => run_512_9(options, cpu_affinity, threads),
        (512, 10) => run_512_10(options, cpu_affinity, threads),
        (512, 11) => run_512_11(options, cpu_affinity, threads),
        (1024, 9) => run_1024_9(options, cpu_affinity, threads),
        (1024, 10) => run_1024_10(options, cpu_affinity, threads),
        (1024, 11) => run_1024_11(options, cpu_affinity, threads),
        _ => Err("unsupported benchmark backend".into()),
    }
}

/// Public-shape reconstruction for untimed accounting. This repeats the
/// registered Geometry::new policy, which the source audit checks in each build.
/// Only degrees 512/1024 (three/four sources) are exposed by this benchmark.
fn pcs_query_shape(
    source_bits: &[usize],
    capacity: usize,
    security: usize,
) -> Result<serde_json::Value, Box<dyn Error>> {
    assert!(source_bits.len() >= 3);
    let logs: Vec<_> = source_bits
        .iter()
        .map(|&bits| {
            let total = bits.checked_mul(capacity).expect("source size");
            assert!(total.is_power_of_two());
            total.ilog2() as usize - 7
        })
        .collect();
    let mut position_log = logs.iter().copied().max().expect("sources") - 3;
    while logs
        .iter()
        .map(|&log| 1usize << log.saturating_sub(position_log))
        .sum::<usize>()
        > 16
    {
        position_log += 1;
    }
    let lane_widths: Vec<_> = logs
        .iter()
        .map(|&log| 1usize << log.saturating_sub(position_log))
        .collect();
    let occupied_lanes: usize = lane_widths.iter().sum();
    let resolved =
        bitz::ligerito_flock::LigeritoSelection::MATCHED_UDR.resolve(position_log + 4, security)?;
    let config = resolved.prover();
    Ok(json!({
        "source_packed_logs": logs,
        "source_lane_widths": lane_widths,
        "position_log": position_log,
        "virtual_lane_log": 4,
        "occupied_lanes": occupied_lanes,
        "initial_opened_row_width": occupied_lanes,
        "recursive_opened_row_widths": config.recursive_ks[..config.recursive_steps - 1].iter().map(|&k| 1usize << k).collect::<Vec<_>>(),
        "queries": config.queries,
        "log_inv_rates": config.log_inv_rates,
        "initial_log_msg_cols": config.initial_log_msg_cols,
        "initial_log_num_interleaved": config.initial_log_num_interleaved,
        "initial_k": config.initial_k,
        "recursive_steps": config.recursive_steps,
        "recursive_log_msg_cols": config.recursive_log_msg_cols,
        "recursive_ks": config.recursive_ks,
        "query_grinding_bits": config.grinding_bits,
        "fold_grinding_bits": config.fold_grinding_bits,
        "ood_samples": config.ood_samples,
        "merkle_hash": format!("{:?}", config.merkle_hash),
        "configuration_fingerprint": resolved.digest().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
    }))
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

fn env_usize(name: &str) -> Result<Option<usize>, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value.parse()?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
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
