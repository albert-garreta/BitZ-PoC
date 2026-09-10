//! Shared benchmark and proof CLI runner. Run each mode in its own process for
//! meaningful peak-memory comparisons; every mode uses BLAKE3 Merkle hashing.
use binius_circuits::sha256::compress::{State, sha256_compress_2x_seq};
use binius_core::{constraint_system::ValueVec, word::Word};
use binius_frontend::{Circuit, CircuitBuilder, Wire};
use binius_hash::Blake3HashSuite;
use binius_prover::{OptimalPackedB128, Prover};
use binius_transcript::{ProverTranscript, VerifierTranscript, fiat_shamir::HasherChallenger};
use binius_verifier::Verifier;
use f2z::{
    binius_ligerito::Prepared as BiniusLigerito,
    hybrid::{
        CompositionProfile, LIGERITO_COMPONENT_BITS, Parameters, PreparedHybrid, chaining_value,
    },
    piop::spartan::{
        f2z::{PreparedU32MulRelation, commit_u32_mul_witness, prove_u32_mul, verify_u32_mul},
        u32_mul::{U32MulLayout, U32MulMod32Row, U32MulWitness},
    },
    transcript::Blake3Transcript,
    utils::prof,
};
use std::{error::Error, time::Instant};

#[path = "sweep.rs"]
mod sweep;

type Challenger = HasherChallenger<blake3::Hasher>;
type AnyError = Box<dyn Error>;

fn select_ligerito(cli: Option<&str>, target: usize) -> Result<f2z::ligerito_flock::LigeritoSelection, AnyError> {
    let env = std::env::var("F2Z_LIG_PROFILE").ok();
    match cli.or(env.as_deref()) {
        Some(request) => Ok(f2z::ligerito_flock::LigeritoSelection::parse(request, target)?),
        None => Ok(f2z::ligerito_flock::LigeritoSelection::JOHNSON),
    }
}

/// How the native Binius64 circuit is proved: Binius64's own ring switch +
/// BaseFold/FRI, or its PIOP prefix with every oracle committed and opened by
/// the F2Z opener (rate 1/8, Johnson regime, grinding, Round 0; whole-protocol
/// union bound gated at 100 bits).
enum NativeBackend {
    Binius {
        prover: Prover<OptimalPackedB128, Blake3HashSuite>,
        verifier: Verifier<Blake3HashSuite>,
    },
    Ligerito(BiniusLigerito),
}

struct Native {
    circuit: Circuit,
    blocks: Vec<[Wire; 16]>,
    output: [Wire; 8],
    multiplications: Vec<[Wire; 4]>,
    backend: NativeBackend,
}

impl Native {
    fn new(multiplications: usize, compressions: usize, ligerito: bool) -> Result<Self, AnyError> {
        let builder = CircuitBuilder::new();
        let multiplications = (0..multiplications)
            .map(|_| f2z::hybrid::mod32_binius::add_u32_mul_mod32(&builder))
            .collect();
        let blocks: Vec<[Wire; 16]> = (0..compressions)
            .map(|_| std::array::from_fn(|_| builder.add_witness()))
            .collect();
        let output = std::array::from_fn(|_| builder.add_inout());
        let mut state = State::iv(&builder);
        for pair in blocks.chunks_exact(2) {
            state = sha256_compress_2x_seq(&builder, state, [pair[0], pair[1]]);
        }
        let mask = builder.add_constant(Word(u32::MAX as u64));
        for (actual, expected) in state.0.into_iter().zip(output) {
            builder.assert_eq(
                "final_sha_chaining_value",
                builder.band(actual, mask),
                expected,
            );
        }
        let circuit = builder.build();
        let backend = if ligerito {
            NativeBackend::Ligerito(BiniusLigerito::new(circuit.constraint_system())?)
        } else {
            // 112 bits for the FRI component leaves slack for the binary PIOPs
            // and the second proof in the separate mode. No default 96-bit preset.
            let verifier = Verifier::<Blake3HashSuite>::setup_with_security_bits(
                circuit.constraint_system().clone(),
                binius_log_inv_rate(),
                binius_security_bits(),
            )?;
            let prover = Prover::setup(verifier.clone())?;
            NativeBackend::Binius { prover, verifier }
        };
        Ok(Self {
            circuit,
            blocks,
            output,
            multiplications,
            backend,
        })
    }
    fn populate(
        &self,
        inputs: &[U32MulMod32Row],
        blocks: &[[u32; 16]],
    ) -> Result<ValueVec, AnyError> {
        if inputs.len() != self.multiplications.len() || blocks.len() != self.blocks.len() {
            return Err("native witness shape does not match the circuit".into());
        }
        let mut filler = self.circuit.new_witness_filler();
        for (wires, row) in self.multiplications.iter().zip(inputs) {
            for (&wire, value) in wires.iter().zip([row.x, row.y, row.z, row.w]) {
                filler[wire] = Word(value as u64);
            }
        }
        for (wires, block) in self.blocks.iter().zip(blocks) {
            for (&wire, &word) in wires.iter().zip(block) {
                filler[wire] = Word(word as u64);
            }
        }
        for (&wire, word) in self.output.iter().zip(chaining_value(blocks)) {
            filler[wire] = Word(word as u64);
        }
        self.circuit.populate_wire_witness(&mut filler)?;
        Ok(filler.into_value_vec())
    }
    fn prove(&self, witness: &ValueVec) -> Result<Vec<u8>, AnyError> {
        match &self.backend {
            NativeBackend::Binius { prover, .. } => {
                let mut t = ProverTranscript::new(Challenger::default());
                prover.prove(witness, &mut t)?;
                Ok(t.finalize())
            }
            NativeBackend::Ligerito(prepared) => {
                let (proof, _) = prepared.prove(witness)?;
                Ok(proof.to_bytes())
            }
        }
    }
    fn verify(&self, witness: &ValueVec, bytes: Vec<u8>) -> Result<(), AnyError> {
        match &self.backend {
            NativeBackend::Binius { verifier, .. } => {
                let mut t = VerifierTranscript::new(Challenger::default(), bytes);
                verifier.verify(witness.inout(), &mut t)?;
                t.finalize()?;
                Ok(())
            }
            NativeBackend::Ligerito(prepared) => {
                let decoded = prepared.proof_from_bytes(&bytes)?;
                prepared.verify(witness.inout(), &decoded)?;
                Ok(())
            }
        }
    }
    fn setup_line(&self) -> String {
        match &self.backend {
            NativeBackend::Binius { .. } => format!(
                "binius_fri_component_bits={} binius_log_inv_rate={}",
                binius_security_bits(),
                binius_log_inv_rate()
            ),
            NativeBackend::Ligerito(prepared) => {
                let security = prepared.security();
                let witness = prepared.opener(0);
                // Per oracle, per level: (fold-challenge grinding bits, query
                // grinding bits, queries) — the proof-of-work the opener pays.
                let ladders: Vec<String> = (0..prepared.oracle_specs().len())
                    .map(|i| {
                        let levels: Vec<String> = prepared
                            .opener(i)
                            .config()
                            .levels
                            .iter()
                            .map(|l| {
                                format!(
                                    "(k={},fold_grind={},query_grind={},queries={})",
                                    l.k_recursive, l.fold_grinding_bits, l.grinding_bits, l.queries
                                )
                            })
                            .collect();
                        format!("oracle{i}[{}]", levels.join(","))
                    })
                    .collect();
                format!(
                    "ligerito_component_bits={} algebraic_security_bits={:.3} level0_queries={} level0_fold_grinding_bits={} ood_grinding_bits={} log_inv_rate={} oracle_logs={:?} binding_term={} ladders={}",
                    prepared.component_bits(),
                    security.algebraic_bits,
                    witness.level0_queries(),
                    witness.level0_fold_grinding_bits(),
                    witness.ood_grinding_bits(),
                    f2z::binary_pcs::LOG_INV_RATE,
                    prepared
                        .oracle_specs()
                        .iter()
                        .map(|s| s.log_msg_len)
                        .collect::<Vec<_>>(),
                    security
                        .binding_term()
                        .map(|t| format!("{}:{:.2}", t.name, -t.error_bound.log2()))
                        .unwrap_or_default(),
                    ladders.join(" ")
                )
            }
        }
    }
}

fn millis(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn peak_kib() -> u64 {
    // Linux only: this target is also built as a [[bin]], which does not get
    // dev-dependencies, so no libc/getrusage path is available here. On other
    // platforms the sweep's per-case child processes are sampled externally.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("VmHWM:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

/// Binius FRI inverse rate exponent: rate `1/2^k`. Defaults to the historical
/// `1` (rate 1/2); the paper's other Binius64 tables also report rate 1/8.
fn binius_log_inv_rate() -> usize {
    std::env::var("F2Z_HYBRID_BINIUS_LOG_INV_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Binius FRI component security. 112 leaves slack for the binary PIOPs and
/// the second proof in the separate mode; the comparison tables use 100.
fn binius_security_bits() -> usize {
    std::env::var("F2Z_HYBRID_BINIUS_SECURITY_BITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(112)
}

pub fn run() -> Result<(), AnyError> {
    let mut parameters = Parameters::default();
    let mut mode = String::from("hybrid");
    let mut output = None;
    let mut verify_file = None;
    let mut iterations = None;
    let mut sweep_requested = false;
    let mut shapes = None;
    let mut results_dir = None;
    let mut single_shape_requested = false;
    let mut profile: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // Cargo appends this flag even for benchmarks with harness = false.
        if arg == "--bench" {
            continue;
        }
        if arg == "--sweep" {
            sweep_requested = true;
            continue;
        }
        if arg == "--help" {
            println!(
                "hybrid-u32-sha256 [--mode hybrid|separate|all-binius|binius-ligerito] [--mul-log 15..22] [--sha-log 1..16] [--iterations N] [--output PROOF]\nhybrid-u32-sha256 --verify PROOF\nhybrid-u32-sha256 --sweep [--shapes MUL_LOG:SHA_LOG,...] [--mode hybrid|separate|all-binius|binius-ligerito|all] [--iterations N] [--results-dir DIR]\nSingle-run defaults: 2^20 products, 2^16 chained compressions, 5 measured iterations after one warmup.\nSweep defaults: equal packed witnesses (15:7,16:8,17:9,18:10,19:11,20:12), hybrid mode, 5 measured iterations after one warmup.\nSweeps save per-run CSV/logs and summary.csv in a new directory under benches/results/hybrid-u32-sha256/. --results-dir must not already exist.\nbinius-ligerito proves the all-Binius circuit with Binius64's PIOP and the F2Z opener (rate 1/8, Johnson regime, grinding, Round 0; 100-bit union bound).\nNon-ZK, 100-bit composition target. Set RAYON_NUM_THREADS to control threads. --output is for single hybrid proofs."
            );
            return Ok(());
        }
        let value = args.next().ok_or("missing flag value")?;
        match arg.as_str() {
            "--mode" => mode = value,
            "--profile" => profile = Some(value),
            "--mul-log" => {
                single_shape_requested = true;
                let log: u32 = value.parse()?;
                if !(15..=22).contains(&log) {
                    return Err("--mul-log must be 15..22".into());
                }
                parameters.multiplications = 1 << log;
            }
            "--sha-log" => {
                single_shape_requested = true;
                let log: u32 = value.parse()?;
                if !(1..=16).contains(&log) {
                    return Err("--sha-log must be 1..16".into());
                }
                parameters.sha_compressions = 1 << log;
            }
            "--iterations" => {
                let count: usize = value.parse()?;
                if count == 0 {
                    return Err("iterations must be positive".into());
                }
                iterations = Some(count);
            }
            "--shapes" => shapes = Some(sweep::parse_shapes(&value)?),
            "--results-dir" => results_dir = Some(std::path::PathBuf::from(value)),
            "--output" => output = Some(value),
            "--verify" => verify_file = Some(value),
            _ => return Err(format!("unknown flag {arg}").into()),
        }
    }
    if sweep_requested {
        if single_shape_requested || output.is_some() || verify_file.is_some() {
            return Err("--sweep cannot use --mul-log, --sha-log, --output or --verify; select pairs with --shapes MUL_LOG:SHA_LOG,...".into());
        }
        return sweep::run(
            shapes.unwrap_or_else(sweep::equal_witness_shapes),
            &mode,
            iterations.unwrap_or(5),
            results_dir,
            profile.as_deref(),
        );
    }
    if shapes.is_some() || results_dir.is_some() {
        return Err("--shapes and --results-dir require --sweep".into());
    }
    let iterations = iterations.unwrap_or(5);
    if !["hybrid", "separate", "all-binius", "binius-ligerito"].contains(&mode.as_str()) {
        return Err("invalid --mode".into());
    }
    if output.is_some() && mode != "hybrid" {
        return Err("--output requires --mode hybrid".into());
    }
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .init();
    if let Some(path) = verify_file {
        use bincode::Options;
        if mode != "hybrid" || output.is_some() {
            return Err("--verify requires hybrid mode without --output".into());
        }
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err("proof exceeds 64 MiB limit".into());
        }
        let statement_path = format!("{path}.statement.bin");
        if std::fs::metadata(&statement_path)?.len() > 1024 {
            return Err("statement exceeds 1 KiB limit".into());
        }
        let statement_bytes = std::fs::read(statement_path)?;
        let statement: f2z::hybrid::Statement = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(1024)
            .reject_trailing_bytes()
            .deserialize(&statement_bytes)?;
        let prepared = PreparedHybrid::new_with_ligerito(statement.parameters, select_ligerito(profile.as_deref(), 106)?)?;
        let proof = prepared.proof_from_bytes(&statement, &std::fs::read(path)?)?;
        prepared.verify(&statement, &proof)?;
        println!(
            "verified {} multiplications modulo 2^32 (xy=z+2^32*w, four u32 limbs) and {} chained SHA-256 compressions",
            statement.parameters.multiplications, statement.parameters.sha_compressions
        );
        return Ok(());
    }
    let inputs: Vec<_> = (0..parameters.multiplications as u32)
        .map(|i| (i.wrapping_mul(0x9e3779b9), u32::MAX - i))
        .collect();
    let blocks: Vec<[u32; 16]> = (0..parameters.sha_compressions as u32)
        .map(|i| std::array::from_fn(|j| i.wrapping_mul(0x85ebca6b).wrapping_add(j as u32)))
        .collect();
    eprintln!(
        "mode={mode} multiplication_relation=u32_mod_2_32 multiplications={} chained_compressions={} merkle=blake3 non_zk=true threads={}",
        parameters.multiplications,
        parameters.sha_compressions,
        binius_utils::rayon::current_num_threads()
    );
    let setup = Instant::now();
    if mode == "hybrid" {
        prof::force_enable();
        let prepared = PreparedHybrid::new_with_ligerito(parameters, select_ligerito(profile.as_deref(), 106)?)?;
        let request = profile.clone().or_else(|| std::env::var("F2Z_LIG_PROFILE").ok()).unwrap_or_else(|| "custom:3:4".into());
        let report = prepared.ligerito_configuration().report(&request, prepared.ood_round());
        eprintln!("LIGERITO_CONFIG {report}");
        if let Some(path) = &output { std::fs::write(format!("{path}.ligerito.json"), serde_json::to_vec_pretty(&report)?)?; }
        let setup_ms = millis(setup);
        let binding = prepared
            .security()
            .binding_term()
            .map(|term| format!("{}:{:.2}", term.name, -term.error_bound.log2()))
            .unwrap_or_default();
        eprintln!(
            "setup_ms={setup_ms:.3} packed_logs={:?} algebraic_security_bits={:.3} ligerito_component_bits={LIGERITO_COMPONENT_BITS} ood_grinding_bits={} binding_term={binding}",
            prepared.packed_witness_logs(),
            prepared.security().algebraic_bits,
            prepared.ood_round().map_or(0, |p| p.grinding_bits)
        );
        println!(
            "mode,iteration,setup_ms,witness_ms,witness_commit_ms,continuation_ms,total_prover_ms,verify_ms,proof_bytes,peak_rss_kib,piop_ms,iop_ms,mul_piop_ms,sha_piop_ms,mul_opening_ms,joint_sumcheck_ms,shared_opening_ms,ood_round_ms"
        );
        for iteration in 0..=iterations {
            // Discard setup and the preceding verifier's profiling records.
            let _ = prof::take_totals();
            let start = Instant::now();
            let rows: Vec<_> = inputs
                .iter()
                .map(|&(x, y)| f2z::hybrid::U32MulMod32Row::new(x, y))
                .collect();
            // Hybrid fuses assignment synthesis into commit_mod32, so this is
            // the native row construction only; witness_commit_ms below is
            // that plus the commitment.
            let witness_ms = millis(start);
            let committed = prepared.commit_mod32(&rows, &blocks)?;
            let witness_commit_ms = millis(start);
            let continuation = Instant::now();
            let proof = prepared.prove(&committed)?;
            let continuation_ms = millis(continuation);
            let total_ms = millis(start);
            let bytes = proof.to_bytes();
            let phases = prof::take_totals();
            let phase_ms = |name| -> Result<f64, AnyError> {
                phases
                    .iter()
                    .find(|(label, _)| *label == name)
                    .map(|(_, seconds)| seconds * 1000.0)
                    .ok_or_else(|| format!("missing hybrid prover timing: {name}").into())
            };
            // These are disjoint control-thread wall-clock scopes, including
            // their parallel work. Never add their nested profiling records.
            let mul_piop_ms = phase_ms("hybrid:mul_piop")?;
            let sha_piop_ms = phase_ms("hybrid:sha_piop")?;
            let mul_opening_ms = phase_ms("hybrid:mul_opening")?;
            let joint_sumcheck_ms = phase_ms("hybrid:joint_sumcheck")?;
            // Round 0 (the out-of-domain sample, run right after the
            // statement) is part of the shared opening protocol; it is
            // reported on its own and counted in shared_opening_ms.
            let ood_round_ms = phase_ms("hybrid:ood_round")?;
            let shared_opening_ms = phase_ms("hybrid:opening_iop")? + ood_round_ms;
            let piop_ms = mul_piop_ms + sha_piop_ms;
            let iop_ms = mul_opening_ms + joint_sumcheck_ms + shared_opening_ms;
            let verify = Instant::now();
            let decoded = prepared.proof_from_bytes(committed.statement(), &bytes)?;
            prepared.verify(committed.statement(), &decoded)?;
            let verify_ms = millis(verify);
            if iteration == 0 { continue; }
            println!(
                "hybrid,{},{setup_ms:.3},{witness_ms:.3},{witness_commit_ms:.3},{continuation_ms:.3},{total_ms:.3},{verify_ms:.3},{},{},{piop_ms:.3},{iop_ms:.3},{mul_piop_ms:.3},{sha_piop_ms:.3},{mul_opening_ms:.3},{joint_sumcheck_ms:.3},{shared_opening_ms:.3},{ood_round_ms:.3}",
                iteration - 1,
                bytes.len(),
                peak_kib()
            );
            if let Some(path) = &output {
                std::fs::write(path, &bytes)?;
                std::fs::write(
                    format!("{path}.statement.bin"),
                    bincode::serialize(committed.statement())?,
                )?;
                let statement = format!(
                    "protocol=hybrid-u32-mod32-sha256-v5\nmultiplication_relation=xy=z+2^32*w (x,y,z,w are u32)\nparameters={:?}\nroots={:02x?}\nfinal_sha_state={:08x?}\n",
                    committed.statement().parameters,
                    committed.statement().roots,
                    committed.statement().final_sha_state
                );
                std::fs::write(format!("{path}.statement.txt"), statement)?;
            }
        }
    } else {
        let native = Native::new(
            if mode == "separate" { 0 } else { inputs.len() },
            blocks.len(),
            mode == "binius-ligerito",
        )?;
        let separate = if mode == "separate" {
            Some(PreparedU32MulRelation::new_with_profile_and_ligerito::<
                CompositionProfile,
            >(U32MulLayout::new(inputs.len())?, select_ligerito(profile.as_deref(), 112)?)?)
        } else {
            None
        };
        if let Some(p) = &separate {
            let request = profile.clone().or_else(|| std::env::var("F2Z_LIG_PROFILE").ok()).unwrap_or_else(|| "custom:3:4".into());
            eprintln!("LIGERITO_CONFIG {}", p.ligerito_configuration().report(&request, p.security().ood));
        }
        let setup_ms = millis(setup);
        eprintln!("setup_ms={setup_ms:.3} {}", native.setup_line());
        let size_column = if mode == "separate" {
            "proof_payload_bytes_estimate"
        } else {
            "proof_bytes"
        };
        println!("mode,iteration,setup_ms,witness_ms,total_prover_ms,verify_ms,{size_column},peak_rss_kib");
        for iteration in 0..=iterations {
            let start = Instant::now();
            let rows: Vec<_> = inputs
                .iter()
                .map(|&(x, y)| f2z::hybrid::U32MulMod32Row::new(x, y))
                .collect();
            let witness =
                native.populate(if mode == "separate" { &[] } else { &rows }, &blocks)?;
            // Native witness generation: row construction plus the circuit's
            // own witness filling, before any proving work.
            let witness_ms = millis(start);
            let bytes = native.prove(&witness)?;
            let mul = if let Some(relation) = &separate {
                let witness = U32MulWitness::from_mod32_rows(&rows)?;
                let hint = commit_u32_mul_witness(relation, witness.f2z_bit_rows())?;
                let proof = prove_u32_mul(&mut Blake3Transcript::new(), relation, &witness, &hint)?;
                Some((hint, proof))
            } else {
                None
            };
            let total_ms = millis(start);
            // The standalone u32 API has no enclosing wire codec. Its size
            // here is the exact F2Z wire section plus raw Spartan/nonces;
            // report this as a payload estimate rather than invent framing.
            let mut proof_bytes = bytes.len();
            if let Some((hint, proof)) = &mul {
                let relation = separate.as_ref().expect("separate mode");
                proof_bytes += hint.commitment.root.len() + proof.f2z().to_bytes().len()
                    + proof.spartan_payload_elements() * 16
                    + (proof.grinding_nonce_count(relation.security()) - proof.f2z().grinding_nonces.len()) * 8;
            }
            let verify = Instant::now();
            native.verify(&witness, bytes)?;
            if let Some((hint, proof)) = &mul {
                verify_u32_mul(
                    &mut Blake3Transcript::new(),
                    separate.as_ref().expect("separate mode"),
                    &hint.commitment,
                    proof,
                )?;
            }
            if iteration == 0 { continue; }
            println!(
                "{mode},{},{setup_ms:.3},{witness_ms:.3},{total_ms:.3},{:.3},{proof_bytes},{}",
                iteration - 1,
                millis(verify),
                peak_kib()
            );
        }
        if mode == "separate" {
            eprintln!(
                "separate proof_bytes is a payload estimate (no enclosing standalone-u32 framing)"
            );
        }
    }
    Ok(())
}
