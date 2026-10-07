//! Algebraic Falcon-1024 benchmark with distinct original-Falcon inputs.
//! Run with --features falcon-hybrid; input hashing is outside prover timing.
//! witness_commit_ms covers checked witness derivation, packing, and commitment.

#[cfg(feature = "falcon-hybrid")]
#[path = "../benches/common/falcon_degree_inputs.rs"]
mod falcon_degree_inputs;

#[cfg(feature = "falcon-hybrid")]
#[path = "../benches/common/falcon_affinity.rs"]
mod falcon_affinity;

#[cfg(feature = "falcon-hybrid")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bitz::piop::spartan::{
        falcon_profiles::n1024_k11::{decode_public_key, decode_signature_ct, hash_to_point_ct},
        falcon1024_algebraic::{
            FalconAlgebraicStatement, PreparedFalconAlgebraic,
        },
    };
    use clap::Parser;
    use serde_json::json;
    use std::time::Instant;

    #[derive(Parser)]
    struct Options {
        #[arg(long, default_value_t = 32)]
        batch: usize,
        #[arg(long)]
        security: usize,
        #[arg(long, default_value_t = 1)]
        iterations: usize,
        #[arg(long, default_value_t = 1)]
        warmup: usize,
        #[arg(long, env = "RAYON_NUM_THREADS", default_value_t = 1)]
        threads: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Print tracing span timings (ring, norm, binding, BitZ, and PCS).
        #[arg(long)]
        trace: bool,
    }

    let options = Options::parse();
    if options.iterations == 0 || options.threads == 0 {
        return Err("iterations and threads must be positive".into());
    }
    let trials = options
        .warmup
        .checked_add(options.iterations)
        .ok_or("trial count overflow")?;
    let affinity = falcon_affinity::build_global_pool(options.threads)?;
    let observed_threads = rayon::current_num_threads();
    if observed_threads != options.threads {
        return Err("Rayon worker count differs from requested threads".into());
    }
    let auxiliary_threads = flock_core::all_core_pool().current_num_threads();
    if auxiliary_threads != options.threads {
        return Err("set RAYON_NUM_THREADS to match --threads for the auxiliary Flock pool".into());
    }
    if options.trace {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_max_level(tracing::Level::INFO)
            .init();
    }
    falcon_degree_inputs::reject_fixture_overrides()?;
    let start = Instant::now();
    let prepared = PreparedFalconAlgebraic::new(options.batch, options.security)?;
    let prepare_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let cases = falcon_degree_inputs::generate_cases(1024, options.batch, options.seed)?;
    let mut input_hasher = blake3::Hasher::new();
    for case in &cases {
        case.hash_into(&mut input_hasher);
    }
    let input_digest = input_hasher.finalize().to_hex().to_string();
    let mut public = FalconAlgebraicStatement {
        public_keys: Vec::with_capacity(options.batch),
        targets: Vec::with_capacity(options.batch),
    };
    let mut s2 = Vec::with_capacity(options.batch);
    for case in &cases {
        let key = decode_public_key(&case.public_key)?;
        let signature = decode_signature_ct(&case.signature)?;
        let target = hash_to_point_ct(&signature.nonce, &case.message)?;
        public.public_keys.push(*key.h);
        public.targets.push(*target.point);
        s2.push(*signature.s2);
    }
    let external_input_ms = start.elapsed().as_secs_f64() * 1000.0;

    for trial in 0..trials {
        let statement = public.clone();
        let coefficients = s2.clone();
        let total = Instant::now();
        let committed = prepared.commit_from_s2(statement, coefficients)?;
        let witness_commit_ms = total.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let proof = prepared.prove(committed)?;
        let prove_ms = start.elapsed().as_secs_f64() * 1000.0;
        let total_prover_ms = total.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        prepared.verify(&public, &proof)?;
        let verify_ms = start.elapsed().as_secs_f64() * 1000.0;
        let peak_rss_kib: Option<u64> =
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|status| {
                    status.lines().find_map(|line| {
                        line.strip_prefix("VmHWM:")?
                            .split_whitespace()
                            .next()?
                            .parse()
                            .ok()
                    })
                });
        println!(
            "{}",
            json!({
                "relation": "falcon1024-algebraic-public-h-t-private-s1-s2",
                "input_corpus": "distinct-fn-dsa-0.3.0-original-falcon",
                "input_digest": input_digest,
                "batch": options.batch, "security_bits": options.security,
                "trial": if trial < options.warmup { "warmup" } else { "sample" },
                "iteration": trial.checked_sub(options.warmup), "seed": options.seed,
                "threads": observed_threads, "auxiliary_pool_threads": auxiliary_threads,
                "cpu_affinity": affinity, "verification_thread_policy": "configured-global-pool",
                "build_rustflags": option_env!("RUSTFLAGS"),
                "prepare_ms": prepare_ms, "external_input_ms": external_input_ms,
                "preparation_mode": "combined", "witness_commit_ms": witness_commit_ms,
                "prove_ms": prove_ms, "total_prover_ms": total_prover_ms,
                "verify_ms": verify_ms, "proof_payload_bytes": proof.payload_size_bytes(),
                "proof_payload_breakdown": proof.payload_size_breakdown(),
                "live_bits_per_signature": prepared.live_bits_per_signature(),
                "source_bits_per_signature": prepared.source_bits_per_signature(),
                "process_peak_rss_kib": peak_rss_kib,
                "algebraic_security_bits": prepared.security().algebraic_bits,
            })
        );
    }
    Ok(())
}

#[cfg(not(feature = "falcon-hybrid"))]
fn main() {
    eprintln!("falcon_algebraic requires --features falcon-hybrid");
    std::process::exit(2);
}
