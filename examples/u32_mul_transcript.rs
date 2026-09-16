//! A reproducible private witness for 32,768 independent full U32 products.
//! The verifier receives a prepared relation, commitment (root + parameters),
//! and U32MulProof. The fixture recipe is diagnostic metadata, not a public input.

use clap::{Parser, ValueEnum};
use f2z::{
    piop::spartan::{
        Lambda100, PreparedU32MulRelation, U32_MUL_UNIVARIATE_SKIP_DEGREE,
        U32_MUL_UNIVARIATE_SKIP_VARS, U32MulF2zWidth, U32MulWitness,
        protocol::{self, PreparedRelation, RelationSpec, f2z_generator},
        u32_plain::PlainU32Relation,
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

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum Variant {
    JohnsonSkip,
    PlainUdr,
}

#[derive(Parser)]
struct Args {
    /// Stop after ordinary Spartan; no commitment-opening messages.
    #[arg(long)]
    through_spartan: bool,
    /// Optional deterministic operand fixture: edges or an integer seed.
    #[arg(long, default_value = "canonical")]
    fixture: String,
    #[arg(long, value_enum, default_value_t = Variant::JohnsonSkip)]
    variant: Variant,
    /// Fresh artifact directory (must not already exist).
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    fiat_shamir_transcript_logs: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.through_spartan && args.variant != Variant::PlainUdr {
        return Err("--through-spartan requires --variant plain-udr".into());
    }
    if !args.through_spartan && args.fixture != "canonical" {
        return Err("noncanonical fixtures require --through-spartan".into());
    }
    let fixture = operand_fixture(&args.fixture)?;
    let witness =
        U32MulWitness::from_fn_with_f2z_width(1usize << 15, U32MulF2zWidth::W1, |i| fixture[i])?;
    match args.variant {
        Variant::JohnsonSkip => {
            let prepared =
                PreparedU32MulRelation::new_with_profile::<Lambda100>(*witness.layout())?;
            run(args, witness, prepared)
        }
        Variant::PlainUdr => {
            let prepared = PlainU32Relation::prepare(*witness.layout())?;
            run(args, witness, prepared)
        }
    }
}

fn run<S: RelationSpec<Witness = U32MulWitness>>(
    args: Args,
    witness: U32MulWitness,
    prepared: PreparedRelation<S>,
) -> Result<(), Box<dyn std::error::Error>> {
    let plain = args.variant == Variant::PlainUdr;
    let profile = if plain { "udrg:1:4" } else { "johnson" };
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

    let hint = {
        let _span = tracing::info_span!("commitment").entered();
        protocol::commit(&prepared, witness.f2z_bit_rows())?
    };
    let layout = witness.layout();
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
            "variant": if plain { "plain-udr" } else { "johnson-skip" },
            "piop_skip_vars": if plain { 0 } else { U32_MUL_UNIVARIATE_SKIP_VARS },
            "piop_skip_degree": if plain { 0 } else { U32_MUL_UNIVARIATE_SKIP_DEGREE },
            "piop_degree_bound": if plain { 3 } else { U32_MUL_UNIVARIATE_SKIP_DEGREE },
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
            "ligerito": ligerito.report(profile, security.ood),
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
    println!("Configuration: W1, Lambda100, {profile}, initial rate 1/2, initial k=4.");
    println!(
        "Public instance: commitment root {} and parameters {}",
        Hex(&hint.commitment.root),
        serde_json::to_string(&hint.commitment.params)?
    );

    if args.through_spartan {
        use f2z::piop::spartan::SpartanField;
        let mut prover = trial.wrap("prover", Blake3Transcript::new());
        let proved = protocol::prove_bound_piop(&mut prover, &prepared, &witness, &hint)?;
        let mut verifier = trial.wrap("verifier", Blake3Transcript::new());
        let verified = protocol::verify_bound_piop(
            &mut verifier,
            &prepared,
            &hint.commitment,
            &proved.messages,
        )?;
        if proved.terminal_claim != verified.terminal_claim {
            return Err("Spartan claims differ".into());
        }
        let encode = |v: &f2z::piop::spartan::SpartanF2zField| {
            let bytes: [u8; 16] = v
                .canonical_element_encoding()
                .try_into()
                .expect("128-bit field");
            format!("0x{:032x}", u128::from_le_bytes(bytes))
        };
        let claim = json!({"point":proved.terminal_claim.point().iter().map(encode).collect::<Vec<_>>(),
            "scale":encode(proved.terminal_claim.scale()),"value":encode(proved.terminal_claim.value())});
        let digests = json!({"prover":Hex(&prover.state_digest()).to_string(),"verifier":Hex(&verifier.state_digest()).to_string()});
        drop(prover);
        drop(verifier);
        drop(root);
        let path = trial.finish()?;
        if let Some(path) = path.as_ref() {
            let mut counts = [0usize; 2];
            for event in f2z::transcript::logging::read_events(path)? {
                let f2z::transcript::logging::Event::Logical(event) = event? else {
                    return Err("expected logical capture".into());
                };
                let role = match event.role.as_str() {
                    "prover" => 0,
                    "verifier" => 1,
                    _ => return Err("unknown role".into()),
                };
                counts[role] += 1;
            }
            if counts != [88, 88] {
                return Err(format!("unexpected Spartan event counts: {counts:?}").into());
            }
        }
        save_json(
            &out.join("run.json"),
            &json!({
                "schema":"f2z.spartan-prefix-run/v1","boundary":"end-spartan","complete":true,
                "source":source,"fixture":args.fixture,"events_per_role":88,
                "root_hex":statement["public_instance"]["root_hex"],
                "assignment_binding_hex":statement["assignment_binding_hex"],
                "policy_digest_hex":statement["configuration"]["ligerito"]["configuration_fingerprint"],
                "matrix_digest_hex":Hex(&proved.matrices_digest).to_string(),
                "prime_hex":format!("0x{:032x}",proved.prime.q),
                "terminal_claims":{"prover":claim,"verifier":claim},
                "final_transcript_digests":digests,"verification":{"prefix":true,"opening":false},
                "fiat_shamir_transcript_logs":args.fiat_shamir_transcript_logs
            }),
        )?;
        println!("Spartan prefix complete: {}", out.display());
        return Ok(());
    }

    let mut prover = trial.wrap("prover", Blake3Transcript::new());
    let proof = {
        let _span = tracing::info_span!("prove").entered();
        protocol::prove(&mut prover, &prepared, &witness, &hint)?
    };
    let mut verifier = trial.wrap("verifier", Blake3Transcript::new());
    let verified = {
        let _span = tracing::info_span!("verify").entered();
        protocol::verify(&mut verifier, &prepared, &hint.commitment, &proof)
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

fn operand_fixture(name: &str) -> Result<Vec<(u32, u32)>, Box<dyn std::error::Error>> {
    let mut state = if matches!(name, "canonical" | "edges") {
        0
    } else {
        name.parse::<u64>()?
    };
    Ok((0..1usize << 15)
        .map(|i| match name {
            "canonical" => (
                (i as u32).wrapping_mul(0x9e3779b9) | 1,
                (i as u32).wrapping_mul(0x85ebca6b) | 1,
            ),
            "edges" => [
                (0, 0),
                (0, u32::MAX),
                (1, u32::MAX),
                (u32::MAX, u32::MAX),
                (1 << 31, 1 << 31),
            ][i % 5],
            _ => {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let x = (state >> 32) as u32;
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (x, (state >> 32) as u32)
            }
        })
        .collect())
}
