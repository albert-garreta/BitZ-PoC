//! One method/configuration per process; fixture construction is outside timers.
#[path = "support/sha256_ecdsa_fixture.rs"]
mod shared_fixture;
mod common;

use bincode::Options;
use f2z::{piop::spartan::ecdsa_sha256::*, transcript::Blake3Transcript, utils::prof};
use flock_core::pcs::commit::Commitment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{error::Error, time::Instant};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Args {
    method: String,
    r: usize,
    c: usize,
    target: u32,
    /// Binius-family rate: level-0 inverse-rate exponent, forwarded to the
    /// worker (1 = rate 1/2, 3 = rate 1/8). F2Z rows select their rate
    /// through `F2Z_LIG_PROFILE` (`custom:1:4` / `custom:3:4`) instead.
    log_inv_rate: usize,
    threads: usize,
    reps: usize,
    seed: u64,
    fixture: Option<std::path::PathBuf>,
    export_fixture: Option<std::path::PathBuf>,
    binius64_worker: Option<std::path::PathBuf>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let (mut method, mut r, mut c) = (None, None, None);
        let (mut target, mut threads, mut reps, mut seed) = (100, 1, 3, 0);
        let mut log_inv_rate = 1;
        let (mut fixture, mut export_fixture, mut binius64_worker) = (None, None, None);
        while let Some(flag) = args.next() {
            if flag == "--bench" {
                continue;
            }
            if flag == "--help" {
                println!(
                    "sha256_ecdsa_compare --method f2z-split|f2z-all|spartan-mc|binius64|binius64-ligerito --r R --c C [--target 100|128] [--log-inv-rate 1|3] [--threads N] [--reps N] [--seed N] [--fixture FILE] [--export-fixture FILE] [--binius64-worker PATH]"
                );
                std::process::exit(0);
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--method" => method = Some(value),
                "--r" => r = Some(value.parse::<usize>()?),
                "--c" => c = Some(value.parse::<usize>()?),
                "--target" => target = value.parse()?,
                "--log-inv-rate" => log_inv_rate = value.parse()?,
                "--threads" => threads = value.parse()?,
                "--reps" => reps = value.parse()?,
                "--seed" => seed = value.parse()?,
                "--fixture" => fixture = Some(value.into()),
                "--export-fixture" => export_fixture = Some(value.into()),
                "--binius64-worker" => binius64_worker = Some(value.into()),
                _ => return Err(format!("unknown option {flag}").into()),
            }
        }
        let out = Self {
            method: method.ok_or("--method is required")?,
            r: r.ok_or("--r is required")?,
            c: c.ok_or("--c is required")?,
            target,
            log_inv_rate,
            threads,
            reps,
            seed,
            fixture,
            export_fixture,
            binius64_worker,
        };
        let i = out.r.checked_add(out.c).ok_or("exponent overflow")?;
        if !(3..=16).contains(&i) || ![100, 128].contains(&target) || threads == 0 || reps == 0 {
            return Err("require 3 <= r+c <= 16, target 100/128 and positive threads/reps".into());
        }
        if ![1, 3].contains(&out.log_inv_rate) {
            return Err("require --log-inv-rate 1 (rate 1/2) or 3 (rate 1/8)".into());
        }
        if !["f2z-split", "f2z-all", "spartan-mc", "binius64", "binius64-ligerito"]
            .contains(&out.method.as_str())
        {
            return Err("unknown method".into());
        }
        if out.method == "binius64-ligerito" && out.target != 100 {
            return Err("the F2Z opener's whole-protocol gate is fixed at 100 bits".into());
        }
        Ok(out)
    }
    fn exponent(&self) -> usize {
        self.r + self.c
    }
}

type Fixture = shared_fixture::SignedFixture;

fn fixture(args: &Args) -> Result<Fixture> {
    let fixture = match &args.fixture {
        Some(path) => Fixture::read(path)?,
        None => Fixture::generate(args.exponent() as u8, args.seed)?,
    };
    if fixture.log_compressions as usize != args.exponent() || fixture.seed != args.seed {
        return Err("fixture configuration mismatch".into());
    }
    Ok(fixture)
}

fn statement(fixture: &Fixture) -> Sha256EcdsaStatement {
    Sha256EcdsaStatement {
        log_compressions: fixture.log_compressions,
        qx: fixture.qx,
        qy: fixture.qy,
        r: fixture.r,
        s: fixture.s,
    }
}

fn dispatch_binius(args: &Args) -> Result<()> {
    let worker = args
        .binius64_worker
        .as_ref()
        .ok_or("binius64 requires --binius64-worker PATH")?;
    let mut command = std::process::Command::new(worker);
    command.args([
        "--method",
        &args.method,
        "--r",
        &args.r.to_string(),
        "--c",
        &args.c.to_string(),
        "--target",
        &args.target.to_string(),
        "--log-inv-rate",
        &args.log_inv_rate.to_string(),
        "--threads",
        &args.threads.to_string(),
        "--reps",
        &args.reps.to_string(),
        "--seed",
        &args.seed.to_string(),
    ]);
    if let Some(path) = &args.fixture {
        command.arg("--fixture").arg(path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(not(unix))]
    {
        std::process::exit(command.status()?.code().unwrap_or(1));
    }
}

fn timed<T, E: Into<Box<dyn Error>>>(
    f: impl FnOnce() -> std::result::Result<T, E>,
) -> Result<(T, f64)> {
    let start = Instant::now();
    let value = f().map_err(Into::into)?;
    Ok((value, start.elapsed().as_secs_f64() * 1000.))
}

#[derive(Serialize, Deserialize)]
struct F2zWire {
    commitment: Commitment,
    proof: Vec<u8>,
}

fn spartan_revision() -> Option<&'static str> {
    let package = include_str!("../Cargo.lock")
        .split("[[package]]")
        .find(|p| p.lines().any(|line| line == "name = \"spartan2\""))?;
    package
        .lines()
        .find_map(|l| l.strip_prefix("source = \"git+"))?
        .trim_end_matches('"')
        .rsplit_once('#')
        .map(|(_, rev)| rev)
}

fn emit(args: &Args, fixture: &Fixture, trial: usize, mut row: Value) {
    let object = row.as_object_mut().unwrap();
    object.extend(
        json!({
            "schema": "f2z/sha256-ecdsa-compare/v1", "method": args.method,
            "zk": false, "fixture_profile": shared_fixture::SCHEMA,
            "trial": if trial == 0 {"warmup"} else {"sample"}, "sample": trial,
            "log_compressions": args.exponent(), "compressions": 1usize << args.exponent(),
            "message_bytes": fixture.message.len(), "signatures": 1,
            "r": if args.method == "spartan-mc" {Some(args.r)} else {None},
            "c": if args.method == "spartan-mc" {Some(args.c)} else {None},
            "security_target": if args.method == "spartan-mc" {None} else {Some(args.target)},
            "threads": args.threads, "seed": args.seed, "fixture_id": fixture.id,
            "statement_bytes": 129, "statement": "public-key-signature; witness-message",
            "spartan_revision": spartan_revision(), "verified": true,
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let witness = object["witness_ms"].as_f64().unwrap();
    let commit = object["commit_ms"].as_f64().unwrap();
    let protocol = object["protocol_ms"].as_f64().unwrap();
    object.insert("prove_ms".into(), json!(commit + protocol));
    object.insert(
        "witness_to_proof_ms".into(),
        json!(witness + commit + protocol),
    );
    println!("{row}");
}

fn f2z(args: &Args, fixture: &Fixture, mode: OuterMode) -> Result<()> {
    let statement = statement(fixture);
    // The campaign runner selects the opener rate per case through
    // `F2Z_LIG_PROFILE`; record the request verbatim on every row.
    let ligerito_profile =
        std::env::var("F2Z_LIG_PROFILE").unwrap_or_else(|_| "default-by-target".into());
    let (prepared, setup_ms) = timed(|| {
        prepare_sha256_ecdsa(args.exponent(), args.target, mode)
            .and_then(|p| p.with_ligerito(common::ligerito_selection(args.target as usize)))
    })?;
    let security = prepared.security()?;
    for trial in 0..=args.reps {
        prof::take_totals();
        let e2e = Instant::now();
        let (witness, witness_ms) =
            timed(|| generate_sha256_ecdsa_witness(&prepared, &statement, &fixture.message))?;
        let (hint, commit_ms) = timed(|| commit_sha256_ecdsa(&prepared, &witness))?;
        let (proof, protocol_ms) = timed(|| {
            prove_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &witness,
                &hint,
                4,
            )
        })?;
        let e2e_prover_ms = e2e.elapsed().as_secs_f64() * 1000.;
        let phases = prof::take_totals();
        let codec = Instant::now();
        let proof_bytes = proof.to_bytes();
        let object_bytes = proof_bytes.len();
        let wire = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize(&F2zWire {
                commitment: hint.commitment.clone(),
                proof: proof_bytes,
            })?;
        let decoded: F2zWire = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(wire.len() as u64)
            .reject_trailing_bytes()
            .deserialize(&wire)?;
        let decoded_proof = Sha256EcdsaProof::from_bytes(&decoded.proof)?;
        let codec_ms = codec.elapsed().as_secs_f64() * 1000.;
        let (_, verify_ms) = timed(|| {
            fixture.validate_statement()?;
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &decoded.commitment,
                &decoded_proof,
            )
            .map_err(|e| -> Box<dyn Error> { e.into() })
        })?;
        let phase = |name: &str| {
            phases
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value * 1000.)
        };
        emit(
            args,
            fixture,
            trial,
            json!({
                "setup_ms": setup_ms, "witness_ms": witness_ms, "commit_ms": commit_ms,
                "e2e_prover_ms": e2e_prover_ms, "ligerito_profile": ligerito_profile,
                "protocol_ms": protocol_ms, "verify_ms": verify_ms, "codec_ms": codec_ms,
                "proof_object_bytes": object_bytes, "proof_material_bytes": wire.len(),
                "outer_ms": phase("ecdsa:outer_prove"), "inner_ms": phase("ecdsa:shared_inner_prove"),
                "opening_ms": phase("ecdsa:f2z_prove"), "folding_ms": null,
                "phases_seconds": phases, "verify_phases_seconds": prof::take_totals(),
                "security": {"model": "round-by-round-economic", "economic_bits": security.compute_economic_security_bits(),
                    "statistical_bits_lower_bound": security.compute_statistical_security_bits(), "projection_bits": 113,
                    "ligerito": common::ligerito_report(prepared.ligerito_configuration(), prepared.ligerito_configuration().round0(args.target)?) },
                "circuit": {"nonlinear_rows": prepared.nonlinear_rows(), "linear_rows": prepared.linear_rows(),
                    "outer_active_rows": prepared.outer_rows(), "outer_slots": prepared.outer_domain_size(),
                    "source_bits": prepared.live_source_bits(), "assignment_bits": prepared.live_assignment_bits()},
            }),
        );
    }
    Ok(())
}

fn spartan(args: &Args, fixture: &Fixture) -> Result<()> {
    use spartan2::sha256_ecdsa::{Prepared, Proof, Statement};
    let s = fixture;
    let statement = Statement {
        log_compressions: s.log_compressions,
        qx: s.qx,
        qy: s.qy,
        r: s.r,
        s: s.s,
    };
    let (prepared, setup_ms) = timed(|| Prepared::setup(args.r, args.c))?;
    for trial in 0..=args.reps {
        let e2e = Instant::now();
        let (witness, witness_ms) =
            timed(|| prepared.generate_witness(&statement, &fixture.message))?;
        let (committed, commit_ms) = timed(|| prepared.commit(witness))?;
        let ((proof, phases), protocol_ms) = timed(|| prepared.prove(&statement, &committed))?;
        let e2e_prover_ms = e2e.elapsed().as_secs_f64() * 1000.;
        let codec = Instant::now();
        let bytes = proof.to_bytes()?;
        let decoded = Proof::from_bytes(&bytes)?;
        let codec_ms = codec.elapsed().as_secs_f64() * 1000.;
        let (_, verify_ms) = timed(|| -> Result<()> {
            fixture.validate_statement()?;
            prepared.verify(&statement, &decoded)?;
            Ok(())
        })?;
        emit(
            args,
            fixture,
            trial,
            json!({
                "setup_ms": setup_ms, "witness_ms": witness_ms, "commit_ms": commit_ms,
                "e2e_prover_ms": e2e_prover_ms,
                "protocol_ms": protocol_ms, "verify_ms": verify_ms, "codec_ms": codec_ms,
                "proof_object_bytes": bytes.len(), "proof_material_bytes": bytes.len(),
                "outer_ms": phases.outer_ms, "inner_ms": phases.inner_ms,
                "opening_ms": phases.opening_ms, "folding_ms": if args.c == 0 {None} else {Some(phases.folding_ms)},
                "phases_ms": phases, "circuit": {"sha": prepared.sizes()[0], "p256": prepared.sizes()[1]},
                "security": {"model": "discrete-log-and-fiat-shamir", "group": "T256", "constraint_field": "P256-Fp",
                    "transcript": "Keccak256", "pcs": "Hyrax-direct", "hyrax_columns": 2048,
                    "nominal_group_security_bits": 128, "statistical_bits_lower_bound": null},
            }),
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    if let Some(path) = &args.export_fixture {
        return shared_fixture::SignedFixture::generate(args.exponent() as u8, args.seed)?
            .write(path);
    }
    if args.method.starts_with("binius64") {
        return dispatch_binius(&args);
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;
    prof::force_enable();
    let fixture = fixture(&args)?;
    match args.method.as_str() {
        "f2z-split" => f2z(&args, &fixture, OuterMode::Split),
        "f2z-all" => f2z(&args, &fixture, OuterMode::AllRows),
        "spartan-mc" => spartan(&args, &fixture),
        _ => unreachable!(),
    }
}
