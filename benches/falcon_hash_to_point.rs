//! Experimental Binius64 HashToPoint proof, measured separately from Falcon.
//!
//! This benchmark includes witness generation, commitment and proving. SHAKE
//! and the Falcon arithmetic proof are excluded: the exact sample/output arrays
//! are the standalone proof's public statement, not a composed Falcon proof.
use bitz::piop::spartan::falcon1024_ct::{
    hybrid_hash_to_point::BinaryHashToPoint, verification_trace,
};
use clap::Parser;
use serde_json::json;
use std::{error::Error, time::Instant};

#[derive(Parser)]
struct Options {
    #[arg(long, default_value_t = 1)]
    batch: usize,
    #[arg(long)]
    security: usize,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    #[arg(long, default_value_t = 3)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    warmup: usize,
    /// Build the circuit and report its size without allocating proof buffers.
    #[arg(long)]
    prepare_only: bool,
    #[arg(long, hide = true)]
    bench: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse();
    if options.threads == 0 || options.iterations == 0 {
        return Err("threads and iterations must be positive".into());
    }
    if !matches!(options.security, 100 | 128) {
        return Err("select --security 100 or 128".into());
    }
    let trials = options
        .warmup
        .checked_add(options.iterations)
        .ok_or("trial count overflow")?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads)
        .build_global()?;
    let trace = verification_trace(
        include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/public_key.bin"),
        include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/message.bin"),
        include_bytes!("../src/piop/spartan/falcon1024_ct/fixtures/signature_ct.bin"),
    )?;
    let start = Instant::now();
    let prepared = BinaryHashToPoint::new(options.batch, options.security)?;
    let prepare_ms = ms(start);
    println!(
        "{}",
        json!({
            "schema": "bitz/falcon-binary-hash-to-point/v1",
            "event": "prepared",
            "experimental": true,
            "batch": options.batch,
            "fri_security_target": options.security,
            "full_composition_security_accounted": false,
            "threads": options.threads,
            "prepare_ms": prepare_ms,
            "stats": prepared.stats(),
            "scope": "standalone HashToPoint with public sample/output arrays; excludes SHAKE and Falcon arithmetic",
            "fixture_repeated": true,
        })
    );
    if options.prepare_only {
        return Ok(());
    }
    let samples = vec![*trace.hash_to_point.words; options.batch];
    let points = vec![*trace.hash_to_point.point; options.batch];
    let mut prover = Vec::with_capacity(options.iterations);
    let mut verifier = Vec::with_capacity(options.iterations);
    for trial in 0..trials {
        let start = Instant::now();
        let (proof, timings) = prepared.prove(&samples, &points)?;
        let total_prover_ms = ms(start);
        let start = Instant::now();
        prepared.verify(&samples, &points, &proof)?;
        let verify_ms = ms(start);
        let measured = trial >= options.warmup;
        if measured {
            prover.push(total_prover_ms);
            verifier.push(verify_ms);
        }
        println!(
            "{}",
            json!({
                "schema": "bitz/falcon-binary-hash-to-point/v1",
                "event": "trial",
                "trial": if measured { "sample" } else { "warmup" },
                "batch": options.batch,
                "timings": timings,
                "total_prover_ms": total_prover_ms,
                "total_prover_ms_per_signature": total_prover_ms / options.batch as f64,
                "verify_ms": verify_ms,
                "verified": true,
            })
        );
    }
    let total_prover_ms = median(prover);
    println!(
        "{}",
        json!({
            "schema": "bitz/falcon-binary-hash-to-point/v1",
            "event": "summary",
            "batch": options.batch,
            "fri_security_target": options.security,
            "threads": options.threads,
            "samples": options.iterations,
            "median_total_prover_ms": total_prover_ms,
            "median_total_prover_ms_per_signature": total_prover_ms / options.batch as f64,
            "median_verify_ms": median(verifier),
            "verified": true,
        })
    );
    Ok(())
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let i = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[i - 1] + values[i]) / 2.0
    } else {
        values[i]
    }
}
