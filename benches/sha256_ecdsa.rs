//! One worker process per (compression count, outer mode, security, threads).
//! Signing is fixture preparation; inversion hints belong to timed witness generation.
mod common;

use f2z::{piop::spartan::ecdsa_sha256::*, transcript::Blake3Transcript, utils::prof};
use p256::ecdsa::{
    Signature, SigningKey,
    signature::{Signer, Verifier},
};
use serde_json::json;
use std::{error::Error, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let threads = common::init();
    prof::force_enable();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: sha256_ecdsa EXPONENT split|all SECURITY REPS".into());
    }
    let exponent: usize = args[0].parse()?;
    let mode = match args[1].as_str() {
        "split" => OuterMode::Split,
        "all" => OuterMode::AllRows,
        _ => return Err("mode must be split or all".into()),
    };
    let lambda = args[2].parse()?;
    let reps: usize = args[3].parse()?;
    if reps == 0 {
        return Err("reps must be positive".into());
    }
    let start = Instant::now();
    let prepared = prepare_sha256_ecdsa(exponent, lambda, mode)?;
    let setup_ms = start.elapsed().as_secs_f64() * 1000.;
    let security = prepared.security()?;
    let message: Vec<_> = (0..prepared.message_bytes()).map(|i| i as u8).collect();
    let key = SigningKey::from_bytes((&[7u8; 32]).into())?;
    let signature: Signature = key.sign(&message);
    key.verifying_key().verify(&message, &signature)?;
    let point = key.verifying_key().to_encoded_point(false);
    let (r, s) = signature.split_bytes();
    let statement = Sha256EcdsaStatement {
        log_compressions: exponent as u8,
        qx: point.x().unwrap().as_slice().try_into()?,
        qy: point.y().unwrap().as_slice().try_into()?,
        r: r.into(),
        s: s.into(),
    };
    for trial in 0..=reps {
        let start = Instant::now();
        let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message)?;
        let witness_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let hint = commit_sha256_ecdsa(&prepared, &witness)?;
        let commit_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let proof = prove_sha256_ecdsa(
            &mut Blake3Transcript::new(),
            &prepared,
            &statement,
            &witness,
            &hint,
            4,
        )?;
        let protocol_ms = start.elapsed().as_secs_f64() * 1000.;
        let prove_phases = prof::take_totals();
        // Serialization and decoding are outside proving and verifying timers.
        let bytes = proof.to_bytes();
        let decoded = Sha256EcdsaProof::from_bytes(&bytes)?;
        assert_eq!(decoded.to_bytes(), bytes);
        let start = Instant::now();
        verify_sha256_ecdsa(
            &mut Blake3Transcript::new(),
            &prepared,
            &statement,
            &hint.commitment,
            &decoded,
        )?;
        let verify_ms = start.elapsed().as_secs_f64() * 1000.;
        let verify_phases = prof::take_totals();
        println!(
            "{}",
            json!({
                "schema": "f2z/sha256-ecdsa/v1", "trial": if trial==0 {"warmup"} else {"sample"}, "sample": trial,
                "log_compressions": exponent, "compressions": prepared.compressions(), "message_bytes": message.len(),
                "mode": args[1], "security_target": lambda, "threads": threads,
                "security_model": "round-by-round-economic", "economic_bits": security.economic_bits(), "statistical_bits_lower_bound": security.statistical_bits(),
                "setup_ms": setup_ms, "witness_ms": witness_ms, "commit_ms": commit_ms,
                "prove_ms": commit_ms+protocol_ms, "protocol_ms": protocol_ms, "verify_ms": verify_ms,
                "proof_bytes": bytes.len(), "source_bits": prepared.live_source_bits(), "assignment_bits": prepared.live_assignment_bits(),
                "nonlinear_rows": prepared.nonlinear_rows(), "linear_rows": prepared.linear_rows(), "verified": true,
                "prove_phases_seconds": prove_phases, "verify_phases_seconds": verify_phases,
            })
        );
    }
    Ok(())
}
