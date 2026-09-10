//! Isolated non-ZK Binius64 worker for the common SHA-chain/P-256 relation.
#![recursion_limit = "256"]

#[path = "../../../benches/support/sha256_ecdsa_fixture.rs"]
mod fixture;

use binius_circuits::sha256_ecdsa::{PROFILE, Sha256Ecdsa, public_words};
use binius_frontend::CircuitBuilder;
use binius_hash::sha256::Sha256HashSuite;
use binius_prover::{OptimalPackedB128, Prover};
use binius_transcript::{ProverTranscript, VerifierTranscript};
use binius_verifier::{Verifier, config::StdChallenger};
use fixture::{Result, SignedFixture};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use tracing::{Id, Subscriber};
use tracing_subscriber::{Layer, layer::Context, prelude::*, registry::LookupSpan};

const PHASES: [&str; 4] = [
    "Prepare witness",
    "Commit witness",
    "[phase] PCS Opening",
    "[phase] Finish PCS",
];

#[derive(Clone, Default)]
struct Timings(Arc<Mutex<BTreeMap<&'static str, f64>>>);

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Timings {
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Instant::now());
        }
    }
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if let Some(start) = span.extensions_mut().remove::<Instant>() {
                *self.0.lock().unwrap().entry(span.name()).or_default() +=
                    start.elapsed().as_secs_f64() * 1000.;
            }
        }
    }
}

impl Timings {
    fn take(&self) -> BTreeMap<&'static str, f64> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

struct Args {
    exponent: u8,
    threads: usize,
    target: usize,
    reps: usize,
    seed: u64,
    fixture: Option<PathBuf>,
    self_test: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let (mut r, mut c) = (None::<u8>, None::<u8>);
        let mut out = Self {
            exponent: 0,
            threads: 1,
            target: 100,
            reps: 3,
            seed: 0,
            fixture: None,
            self_test: false,
        };
        while let Some(flag) = args.next() {
            if flag == "--self-test" {
                out.self_test = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--method" if value == "binius64" => {}
                "--r" => r = Some(value.parse()?),
                "--c" => c = Some(value.parse()?),
                "--target" => out.target = value.parse()?,
                "--threads" => out.threads = value.parse()?,
                "--reps" => out.reps = value.parse()?,
                "--seed" => out.seed = value.parse()?,
                "--fixture" => out.fixture = Some(value.into()),
                _ => return Err(format!("unknown argument {flag} {value}").into()),
            }
        }
        out.exponent = r
            .ok_or("--r required")?
            .checked_add(c.ok_or("--c required")?)
            .ok_or("exponent overflow")?;
        binius_circuits::sha256_ecdsa::message_len(out.exponent)?;
        if ![100, 128].contains(&out.target) || out.threads == 0 || out.reps == 0 {
            return Err("require target 100/128 and positive threads/reps".into());
        }
        Ok(out)
    }
}

fn build_info() -> serde_json::Value {
    json!({"binius_revision":env!("BINIUS_REVISION"), "lock_sha256":env!("LOCK_SHA256"),
        "source_sha256":env!("SOURCE_SHA256"), "rustc":env!("BUILD_RUSTC"), "rustflags":env!("BUILD_RUSTFLAGS"),
        "circuit_profile":PROFILE, "fixture_profile":fixture::SCHEMA, "zk":false})
}

fn verify(
    verifier: &Verifier<Sha256HashSuite>,
    fixture: &SignedFixture,
    proof: Vec<u8>,
) -> Result<()> {
    fixture.validate_statement()?;
    let expected = public_words(
        fixture.log_compressions,
        &fixture.qx,
        &fixture.qy,
        &fixture.r,
        &fixture.s,
    );
    let mut transcript = VerifierTranscript::new(StdChallenger::default(), proof);
    verifier.verify(&expected, &mut transcript)?;
    transcript.finalize()?;
    Ok(())
}

fn main() -> Result<()> {
    if matches!(std::env::args().nth(1).as_deref(), Some("--help" | "-h")) {
        println!(
            "binius64-sha256-ecdsa --r R --c C [--target 100|128] [--threads N] [--reps N] [--seed N] [--fixture PATH] [--self-test]\n\
            Standard P-256, non-ZK; 3 <= R+C <= 16. --build-info prints pinned source/build metadata."
        );
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--build-info") {
        println!("{}", build_info());
        return Ok(());
    }
    let args = Args::parse()?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;
    let timings = Timings::default();
    tracing_subscriber::registry()
        .with(
            timings
                .clone()
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    PHASES.contains(&meta.name())
                })),
        )
        .try_init()?;
    let fixture = match &args.fixture {
        Some(path) => SignedFixture::read(path)?,
        None => SignedFixture::generate(args.exponent, args.seed)?,
    };
    if fixture.log_compressions != args.exponent || fixture.seed != args.seed {
        return Err("fixture configuration mismatch".into());
    }
    let setup = Instant::now();
    let builder = CircuitBuilder::new();
    let relation = Sha256Ecdsa::new(&builder, args.exponent)?;
    let circuit = builder.build();
    let verifier = Verifier::<Sha256HashSuite>::setup_with_security_bits(
        circuit.constraint_system().clone(),
        1,
        args.target,
    )?;
    let prover = Prover::<OptimalPackedB128, Sha256HashSuite>::setup(verifier.clone())?;
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
    let circuit_id =
        blake3::hash(format!("{PROFILE}:{}:{}", args.exponent, env!("SOURCE_SHA256")).as_bytes())
            .to_hex()
            .to_string();
    for trial in 0..=args.reps {
        timings.take();
        let e2e = Instant::now();
        let witness_start = Instant::now();
        let mut filler = circuit.new_witness_filler();
        relation.populate(
            &mut filler,
            &fixture.message,
            &fixture.qx,
            &fixture.qy,
            &fixture.r,
            &fixture.s,
        )?;
        circuit.populate_wire_witness(&mut filler)?;
        let witness = filler.into_value_vec();
        let assignment_ms = witness_start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let mut transcript = ProverTranscript::new(StdChallenger::default());
        prover.prove(&witness, &mut transcript)?;
        let bytes = transcript.finalize();
        let prove_call_ms = start.elapsed().as_secs_f64() * 1000.;
        let e2e_prover_ms = e2e.elapsed().as_secs_f64() * 1000.;
        let phases = timings.take();
        let phase = |key: &str| {
            phases
                .get(key)
                .copied()
                .ok_or_else(|| format!("missing timing phase {key}"))
        };
        let packing = phase(PHASES[0])?;
        let commit_ms = phase(PHASES[1])?;
        let opening_ms = phase(PHASES[2])? + phase(PHASES[3])?;
        let witness_ms = assignment_ms + packing;
        let protocol_ms = prove_call_ms - packing - commit_ms;
        if !protocol_ms.is_finite() || protocol_ms < opening_ms {
            return Err("overlapping or invalid phase measurements".into());
        }
        // Transcript bytes already are the proof wire format. Copying accounts for transport decoding.
        let codec = Instant::now();
        let proof_bytes = bytes.len();
        let decoded = bytes.clone();
        let codec_ms = codec.elapsed().as_secs_f64() * 1000.;
        drop(witness);
        let start = Instant::now();
        verify(&verifier, &fixture, decoded)?;
        let verify_ms = start.elapsed().as_secs_f64() * 1000.;
        if args.self_test && trial == 0 {
            for which in 0..5 {
                let mut other = fixture.clone();
                match which {
                    0 => other.qx[31] ^= 1,
                    1 => other.qy[31] ^= 1,
                    2 => other.r[31] ^= 1,
                    3 => other.s[31] ^= 1,
                    _ => other.log_compressions += 1,
                }
                if verify(&verifier, &other, bytes.clone()).is_ok() {
                    return Err("accepted altered public statement".into());
                }
            }
            let mut corrupt = bytes.clone();
            corrupt[0] ^= 1;
            if verify(&verifier, &fixture, corrupt).is_ok() {
                return Err("accepted corrupt proof".into());
            }
            let mut extra = bytes.clone();
            extra.push(0);
            if verify(&verifier, &fixture, extra).is_ok() {
                return Err("accepted trailing proof byte".into());
            }
            if verify(&verifier, &fixture, bytes[..bytes.len() - 1].to_vec()).is_ok() {
                return Err("accepted truncated proof".into());
            }
        }
        println!(
            "{}",
            json!({
                "schema":"f2z/sha256-ecdsa-compare/v1", "method":"binius64", "zk":false,
                "trial":if trial == 0 {"warmup"} else {"sample"}, "sample":trial,
                "log_compressions":args.exponent, "compressions":1usize << args.exponent, "message_bytes":fixture.message.len(),
                "signatures":1, "r":null, "c":null, "security_target":args.target, "threads":args.threads, "seed":args.seed,
                "fixture_id":fixture.id, "fixture_profile":fixture::SCHEMA, "statement_bytes":129,
                "statement":"public-key-signature; witness-message", "binius_revision":env!("BINIUS_REVISION"),
                "circuit_profile":PROFILE, "circuit_id":circuit_id, "verified":true,
                "setup_ms":setup_ms, "witness_ms":witness_ms, "commit_ms":commit_ms, "protocol_ms":protocol_ms,
                "prove_ms":commit_ms+protocol_ms, "witness_to_proof_ms":witness_ms+commit_ms+protocol_ms, "e2e_prover_ms":e2e_prover_ms,
                "verify_ms":verify_ms, "codec_ms":codec_ms, "opening_ms":opening_ms,
                "outer_ms":null, "inner_ms":null, "folding_ms":null,
                "proof_object_bytes":proof_bytes, "proof_material_bytes":proof_bytes, "phases_ms":phases,
                "security":{"model":"Binius IOP and BaseFold FRI; query target only", "pcs":"BaseFold", "fri_query_target_bits":args.target,
                "fri_queries":verifier.fri_params().n_test_queries(), "log_inv_rate":1, "merkle_hash":"SHA-256", "transcript":"StdChallenger",
                "fri_fold_arities":verifier.fri_params().fold_arities(), "fri_log_message_len":verifier.fri_params().log_msg_len(),
                "fri_final_challenges":verifier.fri_params().n_final_challenges(), "extension_field_bits":128,
                    "statistical_bits_lower_bound":null},
                "circuit":{"gates":circuit.n_gates(), "bitand":circuit.constraint_system().and_constraints.len(),
                    "intmul":circuit.constraint_system().imul_constraints.len(), "public_words":17}
            })
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../benches/support/sha256_ecdsa_test_vectors.rs"]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_larger_than_u16_satisfies_the_circuit() {
        let fixture = SignedFixture::generate(11, 1).unwrap();
        assert_eq!(fixture.message.len(), 131_008);
        let builder = CircuitBuilder::new();
        let relation = Sha256Ecdsa::new(&builder, 11).unwrap();
        let circuit = builder.build();
        let mut filler = circuit.new_witness_filler();
        relation
            .populate(
                &mut filler,
                &fixture.message,
                &fixture.qx,
                &fixture.qy,
                &fixture.r,
                &fixture.s,
            )
            .unwrap();
        circuit.populate_wire_witness(&mut filler).unwrap();
        circuit
            .constraint_system()
            .verify(&filler.into_value_vec())
            .unwrap();
    }

    #[test]
    fn explicit_fri_targets_and_invalid_parameters() {
        let builder = CircuitBuilder::new();
        let input = builder.add_witness();
        let output = builder.add_inout();
        builder.assert_eq("product", builder.band(input, input), output);
        let cs = builder.build().constraint_system().clone();
        for (rate, target) in [(0, 100), (1, 0), (1, 129)] {
            assert!(
                Verifier::<Sha256HashSuite>::setup_with_security_bits(cs.clone(), rate, target)
                    .is_err()
            );
        }
        for target in [100, 128] {
            let verifier =
                Verifier::<Sha256HashSuite>::setup_with_security_bits(cs.clone(), 1, target)
                    .unwrap();
            assert_eq!(
                verifier.fri_params().n_test_queries(),
                binius_verifier::fri::calculate_n_test_queries(target, 1)
            );
        }
        let default = Verifier::<Sha256HashSuite>::setup(cs, 1).unwrap();
        assert_eq!(
            default.fri_params().n_test_queries(),
            binius_verifier::fri::calculate_n_test_queries(96, 1)
        );
    }

    #[test]
    fn shared_standard_and_exceptional_fixtures_prove_and_bind_public_inputs() {
        let builder = CircuitBuilder::new();
        let relation = Sha256Ecdsa::new(&builder, 3).unwrap();
        let circuit = builder.build();
        let verifier = Verifier::<Sha256HashSuite>::setup_with_security_bits(
            circuit.constraint_system().clone(),
            1,
            100,
        )
        .unwrap();
        let prover = Prover::<OptimalPackedB128, Sha256HashSuite>::setup(verifier.clone()).unwrap();
        for fixture in vectors::vectors() {
            let mut filler = circuit.new_witness_filler();
            relation
                .populate(
                    &mut filler,
                    &fixture.message,
                    &fixture.qx,
                    &fixture.qy,
                    &fixture.r,
                    &fixture.s,
                )
                .unwrap();
            circuit.populate_wire_witness(&mut filler).unwrap();
            let witness = filler.into_value_vec();
            circuit.constraint_system().verify(&witness).unwrap();
            let mut transcript = ProverTranscript::new(StdChallenger::default());
            prover.prove(&witness, &mut transcript).unwrap();
            let bytes = transcript.finalize();
            drop(witness);
            verify(&verifier, &fixture, bytes.clone()).unwrap();
            let mut other = fixture.clone();
            other.s = (-p256::ecdsa::Signature::from_scalars(other.r, other.s)
                .unwrap()
                .s()
                .as_ref())
            .to_bytes()
            .into();
            assert!(verify(&verifier, &other, bytes.clone()).is_err());
            other = fixture.clone();
            other.log_compressions = 4;
            assert!(verify(&verifier, &other, bytes.clone()).is_err());
            let mut extra = bytes.clone();
            extra.push(0);
            assert!(verify(&verifier, &fixture, extra).is_err());
            assert!(verify(&verifier, &fixture, bytes[..bytes.len() - 1].to_vec()).is_err());
        }
    }
}
