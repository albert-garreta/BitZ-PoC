//! Generate, prove, verify, and account for both proof-size comparison workloads.
//!
//! Run with `cargo run --release --features falcon-hybrid,ecdsa,bench-internals
//! --example proof_size_comparison -- --json` to save machine-readable results.

#[path = "../benches/common/falcon_inputs.rs"]
mod falcon_inputs;

use bitz::{
    ligerito_flock::LigeritoSelection,
    piop::spartan::{
        ecdsa_sha256::{
            OuterMode, Sha256EcdsaProof, Sha256EcdsaStatement, commit_sha256_ecdsa,
            generate_sha256_ecdsa_witness, prepare_sha256_ecdsa, prove_sha256_ecdsa,
            verify_sha256_ecdsa,
        },
        falcon1024_ct::{FalconPublicStatement, HASH_TO_POINT_SAMPLES, PreparedFalconHybrid},
    },
    transcript::Blake3Transcript,
};
use clap::{Parser, ValueEnum};
use p256::ecdsa::{
    Signature, SigningKey,
    signature::{Signer, Verifier},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{error::Error, time::Instant};

const SECURITY_TARGET: usize = 128;
// The 40-byte nonce and 32-byte message fit in one SHAKE256 absorb block.
const KECCAK_PER_SIGNATURE: usize = (2 * HASH_TO_POINT_SAMPLES).div_ceil(136);

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Workload {
    All,
    Falcon,
    Sha256Ecdsa,
}

#[derive(Parser)]
#[command(about = "Generate and verify Falcon and SHA-256/P-256 proofs, then count their bytes")]
struct Args {
    #[arg(long, value_enum, default_value = "all")]
    workload: Workload,
    /// Number of distinct Falcon-1024 signatures (each message is 32 bytes).
    #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u16).range(1..=1024))]
    falcon_batch: u16,
    /// Log2 of the number of SHA-256 compressions in ONE chain with ONE ECDSA signature.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(3..=16))]
    sha_log_compressions: u32,
    #[arg(long, default_value_t = 16, value_parser = positive_threads)]
    threads: usize,
    /// Falcon input-generation seed only; SHA uses the standalone benchmark key/message.
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Emit one JSON object on stdout (progress is always written to stderr).
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct Component {
    name: &'static str,
    bytes: usize,
}

#[derive(Serialize)]
struct Measurement {
    workload: &'static str,
    details: Value,
    accounting: &'static str,
    proof_bytes: usize,
    components: Vec<Component>,
    native_verified: bool,
    proof_verified: bool,
    codec_roundtrip_verified: Option<bool>,
    elapsed_seconds: f64,
}

impl Measurement {
    fn print(&self) {
        println!("\n{}", self.workload);
        println!("{}", self.details);
        println!("{}", self.accounting);
        println!("{:<48} {:>12} {:>10}", "Component", "Bytes", "KiB");
        for component in &self.components {
            println!(
                "{:<48} {:>12} {:>10.3}",
                component.name,
                component.bytes,
                component.bytes as f64 / 1024.0,
            );
        }
        println!(
            "{:<48} {:>12} {:>10.3}",
            "TOTAL",
            self.proof_bytes,
            self.proof_bytes as f64 / 1024.0,
        );
        println!(
            "{:.3} MiB; native and proof verification passed; component sum checked.",
            self.proof_bytes as f64 / (1024.0 * 1024.0),
        );
        if self.codec_roundtrip_verified == Some(true) {
            println!("Serialization roundtrip and verification of decoded proof passed.");
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;
    if !args.json {
        println!(
            "Security target: {SECURITY_TARGET} bits; Ligerito: UDR, rate 1/2, initial k=4; threads: {}.",
            args.threads,
        );
        println!("Both totals exclude public statements and separately supplied commitments.");
    }
    let mut measurements = Vec::new();
    if matches!(args.workload, Workload::All | Workload::Falcon) {
        let measurement = measure_falcon(&args)?;
        if !args.json {
            measurement.print();
        }
        measurements.push(measurement);
    }
    if matches!(args.workload, Workload::All | Workload::Sha256Ecdsa) {
        let measurement = measure_sha256_ecdsa(&args)?;
        if !args.json {
            measurement.print();
        }
        measurements.push(measurement);
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema": "bitz/proof-size-comparison/v1",
                "security_target": SECURITY_TARGET,
                "ligerito": "udrg:1:4",
                "threads": args.threads,
                "excluded": ["public statements", "separately supplied commitments"],
                "measurements": measurements,
            }))?,
        );
    }
    Ok(())
}

fn measure_falcon(args: &Args) -> Result<Measurement, Box<dyn Error>> {
    let start = Instant::now();
    let batch = usize::from(args.falcon_batch);
    falcon_inputs::reject_fixture_overrides()?;
    eprintln!(
        "Falcon: generating and natively verifying {batch} distinct Falcon-1024 signatures..."
    );
    // This helper independently verifies with fn-dsa and the native BitZ verifier.
    let cases = falcon_inputs::generate_cases(batch, args.seed)?;
    let mut input_hash = blake3::Hasher::new();
    for case in &cases {
        case.hash_into(&mut input_hash);
    }
    let keys: Vec<_> = cases
        .iter()
        .map(|case| case.public_key.as_slice())
        .collect();
    let messages: Vec<_> = cases.iter().map(|case| case.message.as_slice()).collect();
    let signatures: Vec<_> = cases.iter().map(|case| case.signature.as_slice()).collect();
    let public = FalconPublicStatement::from_bytes(&keys, &messages, &signatures)?;
    let prepared = PreparedFalconHybrid::new(batch, SECURITY_TARGET)?;
    let capacity = prepared.capacity();
    eprintln!("Falcon: generating and committing the witness...");
    let committed = prepared.commit(public)?;
    let committed_source_bits = committed.committed_source_bits();
    let encoded_source_bits = committed.encoded_source_bits();
    let source_domain_bits = prepared
        .source_bits_per_signature()
        .map(|bits| bits * capacity);
    for branch in 0..3 {
        assert!(committed_source_bits[branch] >= source_domain_bits[branch]);
        assert_eq!(
            encoded_source_bits[branch],
            2 * committed_source_bits[branch]
        );
    }
    let commitment_root_bits = committed.statement.source_root.len() * 8;
    let statement = committed.statement.clone();
    eprintln!("Falcon: proving...");
    let proof = prepared.prove(committed)?;
    eprintln!("Falcon: verifying and checking the proof-size breakdown...");
    prepared.verify(&statement, &proof)?;
    let proof_bytes = proof.payload_size_bytes();
    let components = checked_components(proof.payload_size_breakdown(), proof_bytes);
    let measurement = Measurement {
        workload: "Falcon-1024 signature batch",
        details: json!({
            "signatures": batch,
            "padded_capacity": capacity,
            "message_bytes_per_signature": 32,
            "logical_keccak_permutations_per_signature": KECCAK_PER_SIGNATURE,
            "logical_keccak_permutations": batch * KECCAK_PER_SIGNATURE,
            "input_seed": args.seed,
            "input_digest": input_hash.finalize().to_hex().to_string(),
            "source_order": ["arithmetic", "keccak_first_16", "keccak_last_4"],
            "source_domain_bits": source_domain_bits,
            "committed_source_bits": committed_source_bits,
            "committed_source_bits_total": committed_source_bits.iter().sum::<usize>(),
            "encoded_source_bits": encoded_source_bits,
            "encoded_source_bits_total": encoded_source_bits.iter().sum::<usize>(),
            "commitment_root_bits": commitment_root_bits,
        }),
        accounting: "Stored payload; excludes Falcon transport framing and public statement.",
        proof_bytes,
        components,
        native_verified: true,
        proof_verified: true,
        codec_roundtrip_verified: None,
        elapsed_seconds: start.elapsed().as_secs_f64(),
    };
    // Release the prepared buffers and the PCS pool before running the other workload.
    drop(proof);
    drop(prepared);
    flock_core::scratch::clear();
    Ok(measurement)
}

fn measure_sha256_ecdsa(args: &Args) -> Result<Measurement, Box<dyn Error>> {
    let start = Instant::now();
    let exponent = args.sha_log_compressions as usize;
    eprintln!(
        "SHA-256/P-256: preparing ONE chain of {} compressions and ONE signature...",
        1usize << exponent,
    );
    let prepared = prepare_sha256_ecdsa(exponent, SECURITY_TARGET as u32, OuterMode::Split)?
        .with_ligerito(LigeritoSelection::MATCHED_UDR)?;
    let message: Vec<_> = (0..prepared.message_bytes()).map(|i| i as u8).collect();
    // Match benches/sha256_ecdsa.rs exactly. This key is only a benchmark fixture.
    let key = SigningKey::from_bytes((&[7u8; 32]).into())?;
    let signature: Signature = key.sign(&message);
    key.verifying_key().verify(&message, &signature)?;
    let point = key.verifying_key().to_encoded_point(false);
    let (r, s) = signature.split_bytes();
    let statement = Sha256EcdsaStatement {
        log_compressions: exponent as u8,
        qx: (*point.x().ok_or("missing P-256 x coordinate")?).into(),
        qy: (*point.y().ok_or("missing P-256 y coordinate")?).into(),
        r: r.into(),
        s: s.into(),
    };
    eprintln!("SHA-256/P-256: generating and committing the witness...");
    let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message)?;
    let hint = commit_sha256_ecdsa(&prepared, &witness)?;
    let committed_source_bits = hint.packed_message().len() * 128;
    let encoded_source_bits = hint.flock_prover_data().codeword.len() * 128;
    let live_source_bits = prepared.live_source_bits();
    let sha_source_bits = prepared.compressions() * (512 + circuit::sha256::COMPRESSION_HINT_BITS);
    let ecdsa_source_bits = live_source_bits - sha_source_bits - 1;
    assert_eq!(
        committed_source_bits,
        hint.rows()
            .iter()
            .map(|column| column.len() * 64)
            .sum::<usize>(),
    );
    assert_eq!(committed_source_bits, 1usize << hint.commitment.params.m);
    assert_eq!(encoded_source_bits, 2 * committed_source_bits);
    assert!(committed_source_bits >= live_source_bits);
    eprintln!("SHA-256/P-256: proving...");
    let proof = prove_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &prepared,
        &statement,
        &witness,
        &hint,
        4,
    )?;
    eprintln!("SHA-256/P-256: verifying, roundtripping, and checking the proof-size breakdown...");
    verify_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &prepared,
        &statement,
        &hint.commitment,
        &proof,
    )?;
    let bytes = proof.to_bytes();
    let components = checked_components(proof.size_breakdown(), bytes.len());
    let decoded = Sha256EcdsaProof::from_bytes(&bytes)?;
    assert_eq!(
        decoded.to_bytes(),
        bytes,
        "SHA proof codec roundtrip changed bytes"
    );
    verify_sha256_ecdsa(
        &mut Blake3Transcript::new(),
        &prepared,
        &statement,
        &hint.commitment,
        &decoded,
    )?;
    Ok(Measurement {
        workload: "One SHA-256 chain + one P-256 ECDSA signature",
        details: json!({
            "sha256_chains": 1,
            "sha256_compressions": prepared.compressions(),
            "log_compressions": exponent,
            "ecdsa_signatures": 1,
            "curve": "P-256",
            "message_bytes": message.len(),
            "outer_mode": "split",
            "input_fixture": "benches/sha256_ecdsa.rs fixed key and sequential message bytes",
            "live_source_bits": live_source_bits,
            "sha256_source_bits": sha_source_bits,
            "ecdsa_tail_source_bits": ecdsa_source_bits,
            "shared_constant_bits": 1,
            "padding_bits": committed_source_bits - live_source_bits,
            "committed_source_bits_total": committed_source_bits,
            "encoded_source_bits_total": encoded_source_bits,
            "commitment_root_bits": hint.root().len() * 8,
        }),
        accounting: "Complete serialized proof; excludes separate commitment and public statement.",
        proof_bytes: bytes.len(),
        components,
        native_verified: true,
        proof_verified: true,
        codec_roundtrip_verified: Some(true),
        elapsed_seconds: start.elapsed().as_secs_f64(),
    })
}

fn checked_components(parts: Vec<(&'static str, usize)>, expected: usize) -> Vec<Component> {
    assert_eq!(
        parts.iter().map(|(_, bytes)| bytes).sum::<usize>(),
        expected,
        "component sum differs from the existing proof-size API",
    );
    parts
        .into_iter()
        .map(|(name, bytes)| Component { name, bytes })
        .collect()
}

fn positive_threads(value: &str) -> Result<usize, String> {
    let threads = value.parse::<usize>().map_err(|error| error.to_string())?;
    if threads == 0 {
        return Err("threads must be positive".into());
    }
    Ok(threads)
}
