//! End-to-end Falcon-1024 CT / BitZ benchmark.
#![recursion_limit = "256"]

mod common;
#[path = "common/falcon_inputs.rs"]
mod falcon_inputs;

use bitz::piop::spartan::falcon1024_ct::{
    FalconConstraintCounts, FalconPublicStatement, FalconSecuritySchedule, FalconSourceLayout,
    FalconSourceWitness, check_exact_constraints, commit_falcon_source, falcon_ligerito_configs,
    prove_falcon_bitz, verification_trace, verify_falcon_bitz,
};
use bitz::transcript::Blake3Transcript;
use falcon_inputs::{generate_cases, reject_fixture_overrides};
use serde_json::json;
use std::{error::Error, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    reject_fixture_overrides()?;
    bitz::observability::install().expect("install Perfetto subscriber");
    let threads = common::init();
    let batch = env_usize("BITZ_FALCON_BATCH", 32)?;
    let reps = env_usize("BITZ_BENCH_REPS", 3)?;
    if reps == 0 {
        return Err("BITZ_BENCH_REPS must be positive".into());
    }
    let layout = FalconSourceLayout::new(batch)?;
    let seed = std::env::var("BITZ_FALCON_SEED")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(42);
    let start = Instant::now();
    let cases = generate_cases(batch, seed)?;
    let input_setup_ms = start.elapsed().as_secs_f64() * 1_000.0;
    let mut input_hash = blake3::Hasher::new();
    for case in &cases {
        case.hash_into(&mut input_hash);
    }
    let input_digest = input_hash.finalize().to_hex().to_string();
    let lambda: usize = std::env::var("BITZ_BENCH_LAMBDA")
        .unwrap_or_else(|_| "128".into())
        .parse()?;
    if !matches!(lambda, 100 | 128) {
        return Err("BITZ_BENCH_LAMBDA must be 100 or 128".into());
    }
    let (pc, vc) = falcon_ligerito_configs(&layout, lambda)?;
    let public_keys: Vec<&[u8]> = cases
        .iter()
        .map(|case| case.public_key.as_slice())
        .collect();
    let public_messages: Vec<&[u8]> = cases.iter().map(|case| case.message.as_slice()).collect();
    let signatures: Vec<&[u8]> = cases.iter().map(|case| case.signature.as_slice()).collect();
    let statement = FalconPublicStatement::from_bytes(&public_keys, &public_messages, &signatures)?;
    let security = FalconSecuritySchedule::for_target(lambda).expect("validated target");

    for trial in 0..=reps {
        let start = Instant::now();
        for case in &cases {
            std::hint::black_box(case).verify_upstream()?;
        }
        let native_verify_ms = start.elapsed().as_secs_f64() * 1_000.0;
        let start = Instant::now();
        let traces: Vec<_> = cases
            .iter()
            .map(|case| verification_trace(&case.public_key, &case.message, &case.signature))
            .collect::<Result<_, _>>()?;
        for trace in &traces {
            check_exact_constraints(trace)?;
        }
        let source =
            FalconSourceWitness::from_traces(layout, &public_messages, &signatures, &traces)?;
        let witness_ms = start.elapsed().as_secs_f64() * 1_000.0;
        let start = Instant::now();
        let hint = commit_falcon_source(&source, &pc);
        let commit_ms = start.elapsed().as_secs_f64() * 1_000.0;
        let start = Instant::now();
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_falcon_bitz(
            &mut prover_transcript,
            &layout,
            &statement,
            &traces,
            &source,
            &hint,
            lambda,
            &pc,
        )?;
        let proof_prove_ms = start.elapsed().as_secs_f64() * 1_000.0;
        let start = Instant::now();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_falcon_bitz(
            &mut verifier_transcript,
            &layout,
            &statement,
            &hint.commitment,
            &proof,
            lambda,
            &vc,
        )?;
        let proof_verify_ms = start.elapsed().as_secs_f64() * 1_000.0;
        let counts = FalconConstraintCounts::per_signature();
        println!(
            "{}",
            json!({
                "schema": "bitz/falcon1024-ct/v2",
                "protocol": "bitz/falcon1024-ct/commitment-bound/v4",
                "trial": if trial == 0 { "warmup" } else { "sample" },
                "sample": trial,
                "batch": batch,
                "capacity": layout.capacity(),
                "security_target": lambda,
                "threads": threads,
                "message_bytes": cases[0].message.len(),
                "input_source": "pornin/rust-fn-dsa",
                "input_implementation_version": "0.3.0",
                "input_mode": "original-falcon-1024",
                "input_seed": seed,
                "input_rng": "rand_chacha 0.3.1 ChaCha20Rng::seed_from_u64",
                "input_count": cases.len(),
                "input_digest": input_digest,
                "input_setup_ms": input_setup_ms,
                "native_verify_ms": native_verify_ms,
                "native_verify_includes_public_key_decode": true,
                "public_inputs": ["public_key", "message", "signature_nonce", "signature_s2"],
                "source_live_bits_per_signature": FalconSourceLayout::counts().total(),
                "source_capacity_bits": layout.source_bits(),
                "source_row_vars": layout.row_vars(),
                "source_col_vars": layout.col_vars(),
                "linear_rows_per_signature": counts.linear_rows(),
                "norm_terms_per_signature": counts.norm_terms,
                "norm_sumcheck_rounds": FalconConstraintCounts::coefficient_vars(&layout),
                "norm_round_degree": counts.norm_round_degree(),
                "compaction_product_leaves_per_tree": counts.compaction_leaves,
                "product_round_degree": counts.product_round_degree(),
                "witness_ms": witness_ms,
                "commit_ms": commit_ms,
                "proof_prove_ms": proof_prove_ms,
                "proof_verify_ms": proof_verify_ms,
                "total_prover_ms": witness_ms + commit_ms + proof_prove_ms,
                "projection_modulus_bits": 128 - proof.piop.modulus.leading_zeros(),
                "norm_instance_grinding_bits": if layout.capacity() > 1 { security.norm_instance_bits } else { 0 },
                "outer_point_grinding_bits": security.outer_point_bits,
                "quadratic_round_grinding_bits": security.quadratic_round_bits,
                "cubic_round_grinding_bits": security.cubic_round_bits,
                "fingerprint_grinding_bits": security.fingerprint_bits,
                "linear_point_grinding_bits": security.linear_point_bits,
                "binding_round_grinding_bits": security.binding_round_bits,
                "commitment_root": hex(&hint.commitment.root),
                "verified": true,
                "implementation_stage": "commitment-bound-piop-bitz",
            })
        );
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
