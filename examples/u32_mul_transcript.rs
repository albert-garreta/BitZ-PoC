//! A reproducible private witness for 32,768 independent full U32 products.
//! The verifier receives a prepared relation, commitment (root + parameters),
//! and U32MulProof. The fixture recipe is diagnostic metadata, not a public input.

use clap::Parser;
use f2z::{
    piop::spartan::{
        Lambda100, PreparedU32MulRelation, U32_MUL_UNIVARIATE_SKIP_DEGREE,
        U32_MUL_UNIVARIATE_SKIP_VARS, U32MulF2zWidth, U32MulWitness, commit_u32_mul_witness,
        protocol::f2z_generator, prove_u32_mul, verify_u32_mul,
    },
    transcript::{
        Blake3Transcript,
        logging::{Hex, Operation, TranscriptLogs, render_text_with_annotations},
    },
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
struct Args {
    /// Fresh artifact directory (must not already exist).
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    fiat_shamir_transcript_logs: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let source = json!({
        "revision": git(&["rev-parse", "HEAD"] )?,
        "dirty": !git(&["status", "--porcelain"] )?.is_empty(),
    });
    let out = args.out.unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        PathBuf::from(format!(
            "target/transcript-logs/u32-mul-{stamp}-{}",
            std::process::id()
        ))
    });
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&out)?;
    let out = out.canonicalize()?;
    let logs = TranscriptLogs::new(args.fiat_shamir_transcript_logs);
    logs.install()?;
    let trial = logs.begin_trial(out.join("transcript.jsonl"))?;
    let root = tracing::info_span!(
        "u32_mul_transcript",
        rows = 1usize << 15,
        word_bits = 1,
        profile = "lambda100"
    )
    .entered();

    let witness = U32MulWitness::from_fn_with_f2z_width(1usize << 15, U32MulF2zWidth::W1, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })?;
    let prepared = PreparedU32MulRelation::new_with_profile::<Lambda100>(*witness.layout())?;
    let hint = {
        let _span = tracing::info_span!("commitment").entered();
        commit_u32_mul_witness(&prepared, witness.f2z_bit_rows())?
    };
    let layout = prepared.layout();
    let params = prepared.params();
    let security = prepared.security();
    let ligerito = prepared.ligerito_configuration();
    let terms: Vec<_> = security.accounting.terms.iter().map(|term| json!({
        "name": term.name, "bits": term.bits, "grinding_bits": term.grinding_bits, "floor": term.floor,
    })).collect();
    let resolved = ligerito.prover();
    let statement = json!({
        "schema": "f2z.u32-mul-statement/v1",
        "relation": {
            "equation": "forall i in [0, N): x_i * y_i = z_i + 2^32 * w_i",
            "range": {"variables": ["x", "y", "z", "w"], "min_inclusive": 0, "max_exclusive": "4294967296"},
            "N": layout.multiplications(),
        },
        "witness": {"variables": ["x", "y", "z", "w"], "visibility": "private", "public_operand_arrays": false},
        "dimensions": {
            "row_vars": params.row_vars, "col_vars": params.col_vars, "word_bits": params.word_bits,
            "bits_per_multiplication": 128, "committed_bits": params.cells() * params.word_bits,
            "layout": "128 little-endian bit slots per row: x32 | y32 | product64 (z32 | w32)",
            "capacity": layout.capacity(), "padded_rows": layout.capacity() - layout.multiplications(),
            "gate_vars": layout.gate_vars(), "assignment_len": layout.assignment_len(),
        },
        "configuration": {
            "packing": "W1", "transcript": "BLAKE3", "initial_transcript": "fresh, empty",
            "piop_skip_vars": U32_MUL_UNIVARIATE_SKIP_VARS, "piop_skip_degree": U32_MUL_UNIVARIATE_SKIP_DEGREE,
            "generator_words": f2z_generator().words(),
            "security": {
                "profile_name": security.profile_name, "lambda": security.lambda,
                "projection_min": security.projection_min.to_string(), "projection_max": security.projection_max.to_string(),
                "projection_full_width": security.projection_full_width,
                "initial_grinding_bits": security.initial_grinding_bits,
                "piop_round_grinding_bits": security.piop_round_grinding_bits,
                "terminal_grinding_bits": security.terminal_grinding_bits,
                "reduction": security.reduction.map(|p| json!({"min":p.min.to_string(), "max":p.max.to_string(), "grinding_bits":p.grinding_bits})),
                "forest_round_grinding_bits": security.forest_round_grinding_bits,
                "ring_switch_grinding_bits": security.ring_switch_grinding_bits,
                "ligerito_target_bits": security.ligerito_target_bits,
                "ood": security.ood.map(|p| json!({"grinding_bits":p.grinding_bits})),
                "accounting": {"target": security.accounting.target, "terms": terms},
            },
            "ligerito": ligerito.report("johnson", security.ood),
            "resolved_ligerito_parameters": {
                "log_inv_rates": resolved.log_inv_rates, "recursive_steps": resolved.recursive_steps,
                "initial_log_msg_cols": resolved.initial_log_msg_cols,
                "initial_log_num_interleaved": resolved.initial_log_num_interleaved,
                "initial_k": resolved.initial_k, "recursive_log_msg_cols": resolved.recursive_log_msg_cols,
                "recursive_ks": resolved.recursive_ks, "queries": resolved.queries,
                "grinding_bits": resolved.grinding_bits, "fold_grinding_bits": resolved.fold_grinding_bits,
                "ood_samples": resolved.ood_samples, "merkle_hash": resolved.merkle_hash,
            },
        },
        "public_instance": {"root_hex": Hex(&hint.commitment.root).to_string(), "commitment_parameters": hint.commitment.params},
        "assignment_binding_hex": Hex(&prepared.assignment_binding(&hint.commitment)?).to_string(),
        "verifier_inputs": ["prepared relation", "commitment root and parameters", "U32MulProof"],
        "derived_protocol_values": ["prime", "challenges", "evaluation points"],
        "diagnostic_metadata_is_absorbed": false,
    });
    save_json(&out.join("statement.json"), &statement)?;
    println!("Public statement: 32,768 rows; x*y = z + 2^32*w; 0 <= x,y,z,w < 2^32.");
    println!("Configuration: W1, Lambda100, Johnson Ligerito rate 1/2, initial k=4.");
    println!(
        "Public instance: commitment root {} and parameters {}",
        Hex(&hint.commitment.root),
        serde_json::to_string(&hint.commitment.params)?
    );

    let mut prover = trial.wrap("prover", Blake3Transcript::new());
    let proof = {
        let _span = tracing::info_span!("prove").entered();
        prove_u32_mul(&mut prover, &prepared, &witness, &hint)?
    };
    let mut verifier = trial.wrap("verifier", Blake3Transcript::new());
    let verified = {
        let _span = tracing::info_span!("verify").entered();
        verify_u32_mul(&mut verifier, &prepared, &hint.commitment, &proof)
    };
    let prover_digest = Hex(&prover.state_digest()).to_string();
    let verifier_digest = Hex(&verifier.state_digest()).to_string();
    drop(prover);
    drop(verifier);
    drop(root);
    let transcript_path = trial.finish()?;
    let transcript_text_path = transcript_path
        .as_ref()
        .map(|path| {
            render_text_with_annotations(path, out.join("transcript.txt"), |event| {
                (event.operation == Operation::Absorb
                    && event.purpose == "statement.assignment_binding_digest"
                    && event.value["bytes_hex"] == statement["assignment_binding_hex"])
                    .then_some((
                        "Underlying public statement (from statement.json; diagnostic JSON).\n\
                         The digest above is absorbed; this description is not the binary hash preimage:",
                        &statement,
                    ))
            })
        })
        .transpose()?;
    // Same length-delimited commitment/F2Z-proof fingerprint as the regression pins.
    let mut proof_pin = blake3::Hasher::new();
    for part in [&hint.commitment.root[..], &proof.f2z().to_bytes()] {
        proof_pin.update(&(part.len() as u64).to_le_bytes());
        proof_pin.update(part);
    }
    save_json(
        &out.join("run.json"),
        &json!({
            "schema": "f2z.u32-mul-run/v1", "source": source,
            "fixture": {
                "rows": 1usize << 15, "rng_seed": null,
                "x": "(i as u32).wrapping_mul(0x9e37_79b9) | 1",
                "y": "(i as u32).wrapping_mul(0x85eb_ca6b) | 1",
                "product": "u64::from(x) * u64::from(y)", "z": "product as u32", "w": "(product >> 32) as u32",
                "recipe_is_part_of_verified_relation": false,
            },
            "verification": {"success": verified.is_ok(), "error": verified.as_ref().err().map(ToString::to_string)},
            "final_transcript_digests": {"prover": prover_digest, "verifier": verifier_digest},
            "commitment_and_f2z_proof_pin": proof_pin.finalize().to_hex().to_string(),
            "fiat_shamir_transcript_logs": args.fiat_shamir_transcript_logs, "transcript_path": transcript_path, "transcript_text_path": transcript_text_path,
        }),
    )?;
    println!(
        "Verification: {}",
        if verified.is_ok() { "passed" } else { "FAILED" }
    );
    for name in ["statement.json", "run.json"] {
        println!("{}", out.join(name).display());
    }
    if let Some(path) = transcript_path {
        println!("{}", path.display());
    }
    if let Some(path) = transcript_text_path {
        println!("{}", path.display());
    }
    verified?;
    Ok(())
}

fn save_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let mut writer = BufWriter::new(File::create_new(path)?);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn git(args: &[&str]) -> std::io::Result<String> {
    let output = Command::new("git")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
