#![recursion_limit = "256"]
//! Algebraic Falcon-512/1024 benchmark with distinct original-Falcon inputs.
//! Run with --features falcon-hybrid; input hashing is outside prover timing.
//! witness_commit_ms covers checked witness derivation, packing, and commitment.

#[cfg(feature = "falcon-hybrid")]
#[path = "../benches/common/falcon_degree_inputs.rs"]
mod falcon_degree_inputs;

#[cfg(feature = "falcon-hybrid")]
#[path = "../benches/common/falcon_affinity.rs"]
mod falcon_affinity;

#[cfg(feature = "falcon-hybrid")]
#[path = "../benches/common/falcon_stage_timings.rs"]
mod falcon_stage_timings;

#[cfg(feature = "falcon-hybrid")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::Parser;
    use serde_json::json;
    use std::time::Instant;

    #[derive(Parser)]
    struct Options {
        #[arg(long, default_value_t = 1024)]
        degree: usize,
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
        use tracing_subscriber::prelude::*;
        tracing_subscriber::registry()
            .with(falcon_stage_timings::StageTimings)
            .try_init()?;
    }
    falcon_degree_inputs::reject_fixture_overrides()?;
    macro_rules! run {
        ($algebraic:ident, $native:ident, $degree:literal) => {{
            use bitz::piop::spartan::{
                $algebraic::{FalconAlgebraicStatement, PreparedFalconAlgebraic, PROTOCOL_ID},
                falcon_profiles::$native::{decode_public_key, decode_signature_ct, hash_to_point_ct},
            };
            let start = Instant::now();
            let prepared = PreparedFalconAlgebraic::new(options.batch, options.security)?;
            let prepare_ms = start.elapsed().as_secs_f64() * 1000.0;

            let start = Instant::now();
            let cases = falcon_degree_inputs::generate_cases($degree, options.batch, options.seed)?;
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
                // Exact-build comparison only; hashing is outside every timed phase.
                use std::fmt::Write;
                let mut proof_hash = DebugHasher(blake3::Hasher::new());
                write!(&mut proof_hash, "{proof:?}")?;
                let proof_debug_digest = proof_hash.0.finalize().to_hex().to_string();
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
                        "relation": concat!("falcon", stringify!($degree), "-algebraic-public-h-t-private-s1-s2"),
                        "degree": $degree,
                        "extension_degree": 11,
                        "protocol_id": PROTOCOL_ID,
                        "verified": true,
                        "layout_version": 2,
                        "source_layout": "aligned16-v2",
                        "coefficient_stride": 16,
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
                        "proof_debug_digest": proof_debug_digest,
                        "proof_payload_breakdown": proof.payload_size_breakdown(),
                        "live_bits_per_signature": prepared.live_bits_per_signature(),
                        "source_bits_per_signature": prepared.source_bits_per_signature(),
                        "capacity": prepared.capacity(),
                        "source_bits": prepared.capacity() * prepared.source_bits_per_signature(),
                        "source_packed_bytes": prepared.capacity() * prepared.source_bits_per_signature() / 8,
                        "process_peak_rss_kib": peak_rss_kib,
                        "algebraic_security_bits": prepared.security().algebraic_bits,
                    })
                );
            }
            Ok(())
        }};
    }
    match options.degree {
        512 => run!(falcon512_algebraic, n512_k11, 512),
        1024 => run!(falcon1024_algebraic, n1024_k11, 1024),
        _ => Err("degree must be 512 or 1024".into()),
    }
}

#[cfg(not(feature = "falcon-hybrid"))]
fn main() {
    eprintln!("falcon_algebraic requires --features falcon-hybrid");
    std::process::exit(2);
}

#[cfg(feature = "falcon-hybrid")]
struct DebugHasher(blake3::Hasher);
#[cfg(feature = "falcon-hybrid")]
impl std::fmt::Write for DebugHasher {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0.update(value.as_bytes());
        Ok(())
    }
}
