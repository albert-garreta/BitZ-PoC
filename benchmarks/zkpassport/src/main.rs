//! Native, non-ZK UltraHonk worker. One configuration per process.
#![recursion_limit = "256"]
#[path = "../../../benches/support/sha256_ecdsa_fixture.rs"]
mod fixture;
#[path = "../../../benches/common/output.rs"]
mod output;
use barretenberg_rs::generated_types::ProofSystemSettings;
use bincode::Options;
use fixture::{Result, SignedFixture};
use noir_rs::{
    barretenberg::{api, srs},
    circuit, execute, witness, FieldElement,
};
use noirc_abi::{input_parser::InputValue, Abi, InputMap};
use output::{BenchmarkOutput, FileMode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

const NOIR_RS_REV: &str = "8e516b2ffb126b5cd779d2b22e596c2e1de4e251";

#[derive(Deserialize)]
struct Artifact {
    abi: Abi,
    bytecode: String,
}

struct Args {
    artifact: PathBuf,
    fixture: PathBuf,
    srs_cache: PathBuf,
    revision: String,
    exponent: u8,
    threads: usize,
    reps: usize,
    seed: u64,
    self_test: bool,
    prepare_only: bool,
    offline: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut values = BTreeMap::new();
        let (mut self_test, mut prepare_only, mut offline) = (false, false, false);
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--offline" {
                offline = true;
                continue;
            }
            if flag == "--self-test" {
                self_test = true;
                continue;
            }
            if flag == "--prepare-only" {
                prepare_only = true;
                continue;
            }
            if flag == "--help" {
                println!("zkpassport-bench --artifact FILE --fixture FILE --srs-cache DIR --revision SHA --r EXP --c 0 --threads N --reps N --seed N [--self-test] [--prepare-only] [--offline]");
                std::process::exit(0);
            }
            if ![
                "--artifact",
                "--fixture",
                "--srs-cache",
                "--revision",
                "--r",
                "--c",
                "--threads",
                "--reps",
                "--seed",
                "--method",
            ]
            .contains(&flag.as_str())
            {
                return Err(format!("unknown option {flag}").into());
            }
            let value = args.next().ok_or("missing option value")?;
            if values.insert(flag, value).is_some() {
                return Err("duplicate option".into());
            }
        }
        let get = |key: &str| -> Result<String> {
            values
                .get(key)
                .cloned()
                .ok_or_else(|| format!("missing {key}").into())
        };
        let out = Self {
            artifact: get("--artifact")?.into(),
            fixture: get("--fixture")?.into(),
            srs_cache: get("--srs-cache")?.into(),
            revision: get("--revision")?,
            exponent: get("--r")?.parse()?,
            threads: get("--threads")?.parse()?,
            reps: get("--reps")?.parse()?,
            seed: get("--seed")?.parse()?,
            self_test,
            prepare_only,
            offline,
        };
        if values
            .get("--method")
            .is_some_and(|m| m != "zkpassport-honk")
            || values.get("--c").is_some_and(|c| c != "0")
            || !(3..=16).contains(&out.exponent)
            || out.threads == 0
            || out.reps == 0
            || out.revision.len() != 40
            || !out.revision.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("invalid configuration".into());
        }
        Ok(out)
    }
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}

fn encode(
    abi: &Abi,
    fixture: &SignedFixture,
) -> Result<noir_rs::acir::native_types::WitnessMap<FieldElement>> {
    let bytes = |v: &[u8]| {
        InputValue::Vec(
            v.iter()
                .map(|b| InputValue::Field(FieldElement::from(*b as u128)))
                .collect(),
        )
    };
    let blocks = fixture
        .message
        .chunks_exact(64)
        .map(|block| {
            InputValue::Vec(
                block
                    .chunks_exact(4)
                    .map(|word| {
                        InputValue::Field(FieldElement::from(u32::from_be_bytes(
                            word.try_into().unwrap(),
                        ) as u128))
                    })
                    .collect(),
            )
        })
        .collect();
    let values: InputMap = BTreeMap::from([
        ("message_blocks".into(), InputValue::Vec(blocks)),
        ("qx".into(), bytes(&fixture.qx)),
        ("qy".into(), bytes(&fixture.qy)),
        ("r".into(), bytes(&fixture.r)),
        ("s".into(), bytes(&fixture.s)),
    ]);
    Ok(abi.encode(&values, None)?)
}

fn check_abi(abi: &Abi, exponent: u8) -> Result<()> {
    let integer = |width| json!({"kind":"integer", "sign":"unsigned", "width":width});
    let array = |length, typ| json!({"kind":"array", "length":length, "type":typ});
    let names = ["message_blocks", "qx", "qy", "r", "s"];
    if abi.return_type.is_some() || abi.parameters.len() != names.len() {
        return Err("unexpected ABI".into());
    }
    for (i, parameter) in abi.parameters.iter().enumerate() {
        let expected = if i == 0 {
            array((1u32 << exponent) - 1, array(16, integer(32)))
        } else {
            array(32, integer(8))
        };
        if parameter.name != names[i]
            || serde_json::to_value(&parameter.typ)? != expected
            || parameter.is_public() != (i != 0)
        {
            return Err("circuit ABI/configuration mismatch".into());
        }
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
struct Wire {
    proof: Vec<[u8; 32]>,
}

fn public_inputs(fixture: &SignedFixture) -> Vec<Vec<u8>> {
    fixture
        .qx
        .iter()
        .chain(&fixture.qy)
        .chain(&fixture.r)
        .chain(&fixture.s)
        .map(|b| {
            let mut field = vec![0; 32];
            field[31] = *b;
            field
        })
        .collect()
}

fn verify(
    vk: &[u8],
    settings: &ProofSystemSettings,
    fixture: &SignedFixture,
    wire: Wire,
) -> Result<()> {
    fixture.validate_statement()?;
    let expected = public_inputs(fixture);
    if wire.proof.is_empty() {
        return Err("proof/public-input encoding mismatch".into());
    }
    if !api::circuit_verify(
        vk,
        expected,
        wire.proof.into_iter().map(|f| f.to_vec()).collect(),
        settings,
    )? {
        return Err("proof rejected".into());
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    // Barretenberg caches this on first use; set it before any FFI call.
    std::env::set_var("HARDWARE_CONCURRENCY", args.threads.to_string());
    std::env::set_var("RAYON_NUM_THREADS", args.threads.to_string());
    std::env::set_var("NOIR_SERIALIZATION_FORMAT", "msgpack-compact");
    let fixture = SignedFixture::read(&args.fixture)?;
    if fixture.log_compressions != args.exponent || fixture.seed != args.seed {
        return Err("fixture configuration mismatch".into());
    }
    let artifact_bytes = fs::read(&args.artifact)?;
    let artifact: Artifact = serde_json::from_slice(&artifact_bytes)?;
    check_abi(&artifact.abi, args.exponent)?;
    let artifact_id = blake3::hash(&artifact_bytes).to_hex().to_string();
    let start = Instant::now();
    let (_, acir) = circuit::decode_circuit(&artifact.bytecode)?;
    let mut settings = api::settings_ultra_honk_poseidon2();
    settings.disable_zk = true;
    let stats = api::circuit_stats(&acir, &settings)?;
    let cache_output = BenchmarkOutput::new("");
    BenchmarkOutput::new(&args.srs_cache).create_dir_all()?;
    let srs_path = args
        .srs_cache
        .join(format!("bn254-{}.local", stats.num_gates_dyadic));
    let mut srs_download_ms = 0.;
    if !srs_path.exists() {
        if args.offline {
            return Err(
                format!("SRS cache missing in offline mode: {}", srs_path.display()).into(),
            );
        }
        let download = Instant::now();
        let data = srs::get_srs(stats.num_gates_dyadic, None);
        let temporary = srs_path.with_extension("tmp");
        cache_output.write_bytes(&temporary, &bincode::serialize(&data)?, FileMode::Replace)?;
        cache_output.rename(temporary, &srs_path)?;
        srs_download_ms = ms(download);
    }
    srs::setup_srs(
        stats.num_gates_dyadic,
        Some(srs_path.to_str().ok_or("non-UTF8 SRS path")?),
    )?;
    let vk = api::circuit_compute_vk(&acir, &settings)?.bytes;
    let setup_ms = ms(start) - srs_download_ms;
    if args.prepare_only {
        println!(
            "{}",
            json!({"artifact_id":artifact_id,"num_gates":stats.num_gates,
            "num_gates_dyadic":stats.num_gates_dyadic,"setup_ms":setup_ms,"srs_download_ms":srs_download_ms})
        );
        return Ok(());
    }
    for trial in 0..=args.reps {
        let start = Instant::now();
        let solved = execute::execute(&artifact.bytecode, encode(&artifact.abi, &fixture)?)?;
        let witness_bytes = witness::serialize_witness(solved)?;
        let witness_ms = ms(start);
        let start = Instant::now();
        let proof = api::circuit_prove(&acir, &witness_bytes, &vk, &settings)?;
        let prove_ms = ms(start);
        let start = Instant::now();
        let object_bytes: usize = proof.proof.iter().map(Vec::len).sum();
        if proof.public_inputs != public_inputs(&fixture) {
            return Err("prover returned different public inputs".into());
        }
        // Public inputs are reconstructed from the separately supplied statement.
        let wire = Wire {
            proof: proof
                .proof
                .into_iter()
                .map(|f| f.try_into().map_err(|_| "invalid proof field length"))
                .collect::<std::result::Result<_, _>>()?,
        };
        let encoded = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize(&wire)?;
        let decoded: Wire = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(encoded.len() as u64)
            .reject_trailing_bytes()
            .deserialize(&encoded)?;
        let codec_ms = ms(start);
        let start = Instant::now();
        verify(&vk, &settings, &fixture, decoded)?;
        let verify_ms = ms(start);
        if trial == 0 && args.self_test {
            for mutation in 0..6 {
                let mut bad = fixture.clone();
                match mutation {
                    0 => bad.message[0] ^= 1,
                    1 => bad.r[31] ^= 1,
                    2 => bad.s = [0; 32],
                    3 => bad.qx = [255; 32],
                    4 => bad.r = [0; 32],
                    _ => {
                        let sig = p256::ecdsa::Signature::from_scalars(bad.r, bad.s)?;
                        bad.s = (-sig.s().as_ref()).to_bytes().into();
                    }
                }
                if execute::execute(&artifact.bytecode, encode(&artifact.abi, &bad)?).is_ok() {
                    return Err(format!("invalid witness {mutation} accepted").into());
                }
            }
            let mut bad_public = public_inputs(&fixture);
            bad_public[0][31] ^= 1;
            if matches!(
                api::circuit_verify(
                    &vk,
                    bad_public,
                    wire.proof.iter().map(|f| f.to_vec()).collect(),
                    &settings
                ),
                Ok(true)
            ) {
                return Err("altered public inputs accepted".into());
            }
            let mut wrong_vk = vk.clone();
            // Change a committed selector, leaving the key's framing intact.
            let last = wrong_vk.len() - 1;
            wrong_vk[last] ^= 1;
            if matches!(
                api::circuit_verify(
                    &wrong_vk,
                    public_inputs(&fixture),
                    wire.proof.iter().map(|f| f.to_vec()).collect(),
                    &settings
                ),
                Ok(true)
            ) {
                return Err("altered verification key accepted".into());
            }
            let mut bad = wire;
            bad.proof.pop();
            if verify(&vk, &settings, &fixture, bad).is_ok() {
                return Err("truncated proof accepted".into());
            }
            eprintln!("Noir invalid-witness and proof rejection checks passed");
        }
        println!(
            "{}",
            json!({
                "schema":"f2z/sha256-ecdsa-compare/v1", "method":"zkpassport-honk", "verified":true,
                "trial":if trial==0 {"warmup"} else {"sample"}, "sample":trial,
                "log_compressions":args.exponent,"compressions":1usize<<args.exponent,
                "message_bytes":fixture.message.len(),"signatures":1,"statement_bytes":129,
                "r":null,"c":null,"security_target":null,"threads":args.threads,"seed":args.seed,
                "fixture_id":fixture.id,"spartan_revision":null,"zkpassport_revision":args.revision,
                "noir_rs_revision":NOIR_RS_REV,"barretenberg_version":"5.0.0","artifact_id":artifact_id,
            "zk":false,"setup_ms":setup_ms,"srs_download_ms":srs_download_ms,
            "circuit_profile":"sha256-p256/canonical-low-s/v1",
                "witness_ms":witness_ms,"prove_ms":prove_ms,"witness_to_proof_ms":witness_ms+prove_ms,
                "commit_ms":null,"protocol_ms":null,"outer_ms":null,"inner_ms":null,"opening_ms":null,
                "folding_ms":null,"verify_ms":verify_ms,"codec_ms":codec_ms,
                "proof_object_bytes":object_bytes,"proof_material_bytes":encoded.len(),"verification_key_bytes":vk.len(),
                "circuit":{"num_gates":stats.num_gates,"num_gates_dyadic":stats.num_gates_dyadic},
                "security":{"model":"BN254-KZG-and-Fiat-Shamir","constraint_field":"BN254-Fr","pcs":"KZG",
                    "transcript":"Poseidon2","statistical_bits_lower_bound":null}
            })
        );
    }
    Ok(())
}
