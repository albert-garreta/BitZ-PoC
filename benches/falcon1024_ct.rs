//! End-to-end Falcon-1024 CT / BitZ benchmark.
mod common;

use bitz::piop::spartan::falcon1024_ct::{
    FalconConstraintCounts, FalconPublicStatement, FalconSecuritySchedule, FalconSourceLayout,
    FalconSourceWitness, check_exact_constraints, commit_falcon_source, falcon_ligerito_configs,
    prove_falcon_bitz, verification_trace, verify_falcon_bitz,
};
use bitz::transcript::Blake3Transcript;
use serde_json::json;
use std::{error::Error, fs, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    bitz::observability::install().expect("install Perfetto subscriber");
    let threads = common::init();
    let public_key = read_env_file("BITZ_FALCON_PUBLIC_KEY")?.unwrap_or_else(|| {
        include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/public_key.bin").to_vec()
    });
    let signature = read_env_file("BITZ_FALCON_SIGNATURE")?.unwrap_or_else(|| {
        include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/signature_ct.bin").to_vec()
    });
    let message = std::env::var_os("BITZ_FALCON_MESSAGE")
        .map(fs::read)
        .transpose()?
        .unwrap_or_else(|| {
            include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/message.bin").to_vec()
        });
    let batch = env_usize("BITZ_FALCON_BATCH", 1)?;
    let reps = env_usize("BITZ_BENCH_REPS", 3)?;
    let layout = FalconSourceLayout::new(batch)?;
    let lambda: usize = std::env::var("BITZ_BENCH_LAMBDA")
        .unwrap_or_else(|_| "128".into())
        .parse()?;
    if !matches!(lambda, 100 | 128) {
        return Err("BITZ_BENCH_LAMBDA must be 100 or 128".into());
    }
    let (pc, vc) = falcon_ligerito_configs(&layout, lambda)?;
    let public_keys: Vec<&[u8]> = (0..batch).map(|_| public_key.as_slice()).collect();
    let public_messages: Vec<&[u8]> = (0..batch).map(|_| message.as_slice()).collect();
    let statement = FalconPublicStatement::from_bytes(&public_keys, &public_messages)?;
    let security = FalconSecuritySchedule::for_target(lambda).expect("validated target");

    for trial in 0..=reps {
        let start = Instant::now();
        let traces: Vec<_> = (0..batch)
            .map(|_| verification_trace(&public_key, &message, &signature))
            .collect::<Result<_, _>>()?;
        for trace in &traces {
            check_exact_constraints(trace)?;
        }
        let signatures: Vec<&[u8]> = (0..batch).map(|_| signature.as_slice()).collect();
        let messages: Vec<&[u8]> = (0..batch).map(|_| message.as_slice()).collect();
        let source = FalconSourceWitness::from_traces(layout, &messages, &signatures, &traces)?;
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
                "schema": "bitz/falcon1024-ct/v1",
                "trial": if trial == 0 { "warmup" } else { "sample" },
                "sample": trial,
                "batch": batch,
                "capacity": layout.capacity(),
                "security_target": lambda,
                "threads": threads,
                "message_bytes": message.len(),
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

fn read_env_file(name: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    std::env::var_os(name)
        .map(fs::read)
        .transpose()
        .map_err(Into::into)
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
