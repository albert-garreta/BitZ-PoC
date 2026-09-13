//! Claim-equivalent native end-to-end SHA-256 compression comparison.
//!
//! All backends prove the public relation
//! `H_hat[i] = Compress_SHA256(IV, M[i])` for the same deterministic corpus.
//! Inputs are independent public raw blocks; padding and chaining are out of
//! scope. Every backend uses its own native arithmetization and witness layout.

mod common;
use common::output::{BenchmarkOutput, FileMode, JsonStyle, JsonlWriter};
use common::whir_tuning;
#[path = "common/trace_capture.rs"]
mod trace_capture;
use trace_capture::{
    BiniusLigeritoPhases, BiniusLigeritoTrial, CaptureLayer, CapturedSpan, TraceCapture,
};
#[path = "sha256_e2e_compare/integer_limber.rs"]
mod integer_limber_backend;
#[path = "sha256_e2e_compare/plonky3.rs"]
mod plonky3_backend;

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hint::black_box,
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use binius_circuits::sha256::{State as BiniusShaState, ref_compress, sha256_compress_2x};
use binius_core::{constraint_system::ValueVec, word::Word};
use binius_frontend::{Circuit, CircuitBuilder, Wire};
use binius_hash::StdHashSuite;
use binius_iop::fri::calculate_n_test_queries;
use binius_prover::{OptimalPackedB128, Prover as BiniusProver};
use binius_verifier::{
    Verifier as BiniusVerifier,
    config::StdChallenger,
    transcript::{ProverTranscript, VerifierTranscript},
};
use f2z::binius_ligerito::Prepared as BiniusLigerito;
use f2z::{
    piop::spartan::{
        PreparedSha256CompressionBatch, SHA256_DEFAULT_INNER_PREFIX_VARS, Sha256CompressionInput,
        Sha256CompressionStatement, SpartanField, commit_sha256_compression_witness_with_config,
        generate_sha256_compression_witnesses, prepare_sha256_compression_batch,
        prove_sha256_compressions_with_prefix_vars_and_config, sha256_compression_configs,
        verify_sha256_compressions_with_config,
    },
    transcript::Blake3Transcript,
    utils::prof::ProfileInterval,
};
use serde_json::{Value, json};

const DEFAULT_ROOT_SEED: u64 = 0x5348_4132_3545_3245;
const DEFAULT_EXPONENTS: &str = "7 8 10 11 12 13 14 15 16";
const DEFAULT_REPS: usize = 21;
const DEFAULT_PILOT_REPS: usize = 5;
const BINIUS_SECURITY_BITS: usize = 100;
const F2Z_COMMITMENT_BYTES: usize = 32;
const SHA256_IV: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Backend {
    F2z,
    Plonky3Whir,
    Binius,
    /// Binius64's circuit and PIOP with the F2Z opener (rate 1/2, Johnson
    /// regime, grinding, Round 0; whole-protocol union bound at 100 bits).
    BiniusLigerito,
    Limber,
}

const ALL_BACKENDS: [Backend; 5] = [
    Backend::F2z,
    Backend::Plonky3Whir,
    Backend::Binius,
    Backend::BiniusLigerito,
    Backend::Limber,
];

impl Backend {
    const fn name(self) -> &'static str {
        match self {
            Self::F2z => "F2Z",
            Self::Plonky3Whir => "Plonky3-WHIR",
            Self::Binius => "Binius64",
            Self::BiniusLigerito => "Binius64-Ligerito",
            Self::Limber => "Limber-Brakedown",
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::F2z => "f2z",
            Self::Plonky3Whir => "plonky3-whir",
            Self::Binius => "binius64",
            Self::BiniusLigerito => "binius64-ligerito",
            Self::Limber => "limber",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        ALL_BACKENDS
            .into_iter()
            .find(|backend| backend.slug() == value)
    }
}

#[derive(Clone, Copy, Debug)]
enum Trial {
    Warmup,
    Sample(usize),
    Pilot(usize),
    Preflight,
}

impl Trial {
    fn json(self) -> Value {
        match self {
            Self::Warmup => json!({"kind": "warmup", "warmup_index": 0}),
            Self::Sample(index) => json!({"kind": "sample", "sample_index": index}),
            Self::Pilot(index) => json!({"kind": "pilot", "sample_index": index}),
            Self::Preflight => json!({"kind": "preflight", "sample_index": 0}),
        }
    }

    fn slug(self) -> String {
        match self {
            Self::Warmup => "warmup-0".to_owned(),
            Self::Sample(index) => format!("sample-{index}"),
            Self::Pilot(index) => format!("pilot-{index}"),
            Self::Preflight => "preflight".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
struct CompressionCase {
    block: [u32; 16],
    output: [u32; 8],
}

impl CompressionCase {
    const fn input(&self) -> Sha256CompressionInput {
        (SHA256_IV, self.block)
    }

    const fn statement(&self) -> Sha256CompressionStatement {
        Sha256CompressionStatement::new(self.input(), self.output)
    }
}

#[derive(Clone, Debug)]
struct Corpus {
    cases: Vec<CompressionCase>,
    digest: String,
}

impl Corpus {
    fn new(compressions: usize, seed: u64) -> Self {
        let mut rng = SplitMix64(seed);
        let cases = (0..compressions)
            .map(|_| {
                let block = std::array::from_fn(|_| rng.next_u32());
                let output = ref_compress(SHA256_IV, block);
                CompressionCase { block, output }
            })
            .collect::<Vec<_>>();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"native-fixed-iv-sha256-corpus/v2");
        hasher.update(&(compressions as u64).to_be_bytes());
        for case in &cases {
            for word in case.block.iter().chain(&case.output) {
                hasher.update(&word.to_be_bytes());
            }
        }
        let digest = hasher.finalize().to_hex().to_string();
        Self { cases, digest }
    }

    fn inputs(&self) -> Vec<Sha256CompressionInput> {
        self.cases.iter().map(CompressionCase::input).collect()
    }

    fn statements(&self) -> Vec<Sha256CompressionStatement> {
        self.cases.iter().map(CompressionCase::statement).collect()
    }
}

#[derive(Clone, Debug)]
struct SemanticSpan {
    id: String,
    parent: Option<String>,
    operation: String,
    name: String,
    short_name: String,
    primary_phase: &'static str,
    phase_tags: Vec<&'static str>,
    start_ns: u64,
    end_ns: u64,
    scope_kind: &'static str,
    scope_tag: Option<&'static str>,
    primary_sequence: bool,
    math_latex: Vec<&'static str>,
}

impl SemanticSpan {
    fn duration_ns(&self) -> u64 {
        self.end_ns.saturating_sub(self.start_ns)
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
struct TrialMetrics {
    witness_ms: f64,
    commit_ms: f64,
    piop_ms: f64,
    #[serde(rename = "iop_ms")]
    opening_ms: f64,
    #[serde(rename = "online_prover_ms")]
    total_prover_ms: f64,
    witness_to_proof_ms: f64,
    verifier_ms: f64,
    serialization_ms: Option<f64>,
    proof_bytes: usize,
}

impl TrialMetrics {
    fn from_spans(spans: &[SemanticSpan], proof_bytes: usize) -> Self {
        Self {
            witness_ms: union_ms(spans, |span| {
                span.phase_tags.contains(&"witness-generation")
            }),
            commit_ms: union_ms(spans, |span| span.scope_tag == Some("commit")),
            piop_ms: union_ms(spans, |span| span.scope_tag == Some("constraint-proof")),
            opening_ms: union_ms(spans, |span| span.scope_tag == Some("opening-proof")),
            total_prover_ms: union_ms(spans, |span| span.scope_tag == Some("proving")),
            witness_to_proof_ms: union_ms(spans, |span| {
                span.operation.ends_with("witness-to-proof")
                    || span.short_name == "Witness to proof"
            }),
            verifier_ms: union_ms(spans, |span| span.scope_tag == Some("verification")),
            serialization_ms: spans
                .iter()
                .any(|s| s.scope_tag == Some("serialization"))
                .then(|| union_ms(spans, |s| s.scope_tag == Some("serialization"))),
            proof_bytes,
        }
    }
}

#[derive(Clone, Debug)]
struct BackendAggregate {
    backend: Backend,
    exponent: usize,
    setup_ms: f64,
    config: String,
    whir_tuning: Option<whir_tuning::TuningReport>,
    samples: Vec<TrialMetrics>,
}

impl BackendAggregate {
    fn median(&self) -> TrialMetrics {
        let med = |f: fn(&TrialMetrics) -> f64| {
            let values = self.samples.iter().map(f).collect::<Vec<_>>();
            common::median(&values)
        };
        TrialMetrics {
            witness_ms: med(|m| m.witness_ms),
            commit_ms: med(|m| m.commit_ms),
            piop_ms: med(|m| m.piop_ms),
            opening_ms: med(|m| m.opening_ms),
            total_prover_ms: med(|m| m.total_prover_ms),
            witness_to_proof_ms: med(|m| m.witness_to_proof_ms),
            verifier_ms: med(|m| m.verifier_ms),
            serialization_ms: {
                let samples: Vec<_> = self
                    .samples
                    .iter()
                    .filter_map(|m| m.serialization_ms)
                    .collect();
                (!samples.is_empty()).then(|| common::median(&samples))
            },
            proof_bytes: common::median(
                &self
                    .samples
                    .iter()
                    .map(|m| m.proof_bytes as f64)
                    .collect::<Vec<_>>(),
            ) as usize,
        }
    }
}

struct F2zContext {
    prepared: PreparedSha256CompressionBatch,
    statements: Vec<Sha256CompressionStatement>,
    inputs: Vec<Sha256CompressionInput>,
    pc: flock_core::pcs::ligerito::ProverConfig,
    vc: flock_core::pcs::ligerito::VerifierConfig,
    setup_ms: f64,
    inner_prefix_vars: usize,
}

impl F2zContext {
    fn setup(exponent: usize, corpus: &Corpus, inner_prefix_vars: usize) -> Self {
        let started = Instant::now();
        let prepared = prepare_sha256_compression_batch(exponent)
            .and_then(|p| p.with_ligerito(common::ligerito_selection(100)))
            .expect("valid SHA batch");
        let (pc, vc) = sha256_compression_configs(&prepared).expect("valid F2Z PCS config");
        let setup_ms = started.elapsed().as_secs_f64() * 1e3;
        Self {
            prepared,
            statements: corpus.statements(),
            inputs: corpus.inputs(),
            pc,
            vc,
            setup_ms,
            inner_prefix_vars,
        }
    }

    fn run(&self) -> (TrialMetrics, Vec<SemanticSpan>) {
        let _ = f2z::utils::prof::take_totals();
        let _ = f2z::utils::prof::take_intervals();
        let root = f2z::utils::prof::scope("sha256-compare:verified_trial");
        let witness_to_proof = f2z::utils::prof::scope("sha256-compare:witness_to_proof");

        let witness = {
            let _scope = f2z::utils::prof::scope("sha256-compare:witness_generation");
            generate_sha256_compression_witnesses(&self.prepared, &self.inputs)
                .expect("F2Z witness generation succeeds")
        };
        assert_eq!(
            witness.outputs(),
            self.statements
                .iter()
                .map(|statement| statement.claimed_output)
                .collect::<Vec<_>>(),
            "F2Z and reference SHA outputs differ"
        );

        let total = f2z::utils::prof::scope("sha256-compare:total_prover");

        let hint = {
            let _scope = f2z::utils::prof::scope("sha256-compare:commit");
            commit_sha256_compression_witness_with_config(&self.prepared, &witness, &self.pc)
                .expect("F2Z commitment succeeds")
        };
        let mut prover_transcript = Blake3Transcript::new();
        let proof = {
            let _scope = f2z::utils::prof::scope("sha256-compare:proof");
            prove_sha256_compressions_with_prefix_vars_and_config(
                &mut prover_transcript,
                &self.prepared,
                &self.statements,
                &witness,
                &hint,
                self.inner_prefix_vars,
                &self.pc,
            )
            .expect("F2Z proof succeeds")
        };
        drop(total);
        drop(witness_to_proof);

        let mut verifier_transcript = Blake3Transcript::new();
        {
            let _scope = f2z::utils::prof::scope("sha256-compare:verification");
            verify_sha256_compressions_with_config(
                &mut verifier_transcript,
                &self.prepared,
                &self.statements,
                &hint.commitment,
                &proof,
                &self.vc,
            )
            .expect("F2Z proof verifies");
        }
        drop(root);
        let _ = f2z::utils::prof::take_totals();
        let raw = f2z::utils::prof::take_intervals();

        let piop_bytes = 3
            * proof.inner().round_polynomials.len()
            * proof
                .inner()
                .round_polynomials
                .first()
                .map_or(0, |round| round[0].canonical_element_encoding().len())
            + 8 * (proof.inner_nonces().len() + 2);
        let proof_bytes = F2Z_COMMITMENT_BYTES + piop_bytes + proof.f2z().to_bytes().len();
        black_box((&proof, &hint));

        let spans = f2z_semantic_spans(&raw);
        (TrialMetrics::from_spans(&spans, proof_bytes), spans)
    }
}

#[derive(Clone)]
struct BiniusWires {
    pairs: Vec<BiniusPairWires>,
}

#[derive(Clone)]
struct BiniusPairWires {
    block: [Wire; 16],
    output: [Wire; 8],
}

struct BiniusContext {
    circuit: Circuit,
    wires: BiniusWires,
    verifier: BiniusVerifier<StdHashSuite>,
    prover: BiniusProver<OptimalPackedB128, StdHashSuite>,
    corpus: Corpus,
    setup_ms: f64,
    circuit_build_ms: f64,
    log_inv_rate: usize,
    query_count: usize,
}

impl BiniusContext {
    fn setup(corpus: &Corpus, log_inv_rate: usize) -> Self {
        let (circuit, wires, circuit_build_ms) = build_binius_sha_circuit(corpus);

        let setup_started = Instant::now();
        let verifier = BiniusVerifier::<StdHashSuite>::setup_with_security_bits(
            circuit.constraint_system().clone(),
            log_inv_rate,
            BINIUS_SECURITY_BITS,
        )
        .expect("Binius verifier setup succeeds");
        let query_count = verifier.fri_params().n_test_queries();
        assert_eq!(
            query_count,
            calculate_n_test_queries(BINIUS_SECURITY_BITS, log_inv_rate),
            "comparison setup must use the selected FRI query target"
        );
        let prover = BiniusProver::setup(verifier.clone()).expect("Binius prover setup succeeds");
        let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;
        Self {
            circuit,
            wires,
            verifier,
            prover,
            corpus: corpus.clone(),
            setup_ms,
            circuit_build_ms,
            log_inv_rate,
            query_count,
        }
    }

    fn populate(&self) -> ValueVec {
        let mut filler = self.circuit.new_witness_filler();
        for (pair_index, wires) in self.wires.pairs.iter().enumerate() {
            let low = &self.corpus.cases[2 * pair_index];
            let high = &self.corpus.cases[2 * pair_index + 1];
            for word in 0..8 {
                filler[wires.output[word]] = Word(pack_lanes(low.output[word], high.output[word]));
            }
            for word in 0..16 {
                filler[wires.block[word]] = Word(pack_lanes(low.block[word], high.block[word]));
            }
        }
        self.circuit
            .populate_wire_witness(&mut filler)
            .expect("Binius witness satisfies SHA relation");
        filler.into_value_vec()
    }

    fn run(&self, capture: &TraceCapture) -> (TrialMetrics, Vec<SemanticSpan>, Vec<u8>, ValueVec) {
        capture.begin();
        let root_start = capture.now_ns();
        let witness_to_proof_start = root_start;
        let witness_start = capture.now_ns();
        let witness = self.populate();
        let witness_end = capture.now_ns();
        let total_start = witness_end;

        let mut prover_transcript = ProverTranscript::new(StdChallenger::default());
        self.prover
            .prove(&witness, &mut prover_transcript)
            .expect("Binius proof succeeds");
        let proof_bytes = prover_transcript.finalize();
        let total_end = capture.now_ns();

        let verify_start = capture.now_ns();
        let mut verifier_transcript =
            VerifierTranscript::new(StdChallenger::default(), proof_bytes.clone());
        self.verifier
            .verify(witness.inout(), &mut verifier_transcript)
            .expect("Binius proof verifies");
        verifier_transcript
            .finalize()
            .expect("Binius verifier consumes the complete transcript");
        let verify_end = capture.now_ns();
        let root_end = verify_end;

        let raw = capture.finish();
        let spans = binius_semantic_spans(
            &raw,
            root_start,
            root_end,
            total_start,
            total_end,
            witness_to_proof_start,
            witness_start,
            witness_end,
            verify_start,
            verify_end,
        );
        let metrics = TrialMetrics::from_spans(&spans, proof_bytes.len());
        black_box(&proof_bytes);
        (metrics, spans, proof_bytes, witness)
    }
}

/// The two-lane Binius64 SHA-256 circuit shared by the `binius64` and
/// `binius64-ligerito` backends: one `sha256_compress_2x` per pair of
/// compressions, public blocks and outputs.
fn build_binius_sha_circuit(corpus: &Corpus) -> (Circuit, BiniusWires, f64) {
    assert!(corpus.cases.len().is_multiple_of(2));
    let build_started = Instant::now();
    let builder = CircuitBuilder::new();
    let pairs = (0..corpus.cases.len() / 2)
        .map(|pair_index| {
            let pair_builder = builder.subcircuit(format!("sha256_pair[{pair_index}]"));
            let state = std::array::from_fn(|word| {
                pair_builder.add_constant(Word(pack_lanes(SHA256_IV[word], SHA256_IV[word])))
            });
            let block = std::array::from_fn(|_| pair_builder.add_inout());
            let output = std::array::from_fn(|_| pair_builder.add_inout());
            let actual = sha256_compress_2x(&pair_builder, BiniusShaState::new(state), block);
            for word in 0..8 {
                pair_builder.assert_eq(
                    format!("public_output[{word}]"),
                    actual.0[word],
                    output[word],
                );
            }
            BiniusPairWires { block, output }
        })
        .collect();
    let circuit = builder.build();
    let circuit_build_ms = build_started.elapsed().as_secs_f64() * 1e3;
    (circuit, BiniusWires { pairs }, circuit_build_ms)
}

/// Binius64's SHA-256 circuit and PIOP prefix, with the witness committed
/// and opened by the F2Z opener: rate 1/2, Johnson-regime Ligerito with fold
/// and query grinding and Round 0, gated at 100 bits by a whole-protocol
/// union bound (the yardstick of the F2Z row).
struct BiniusLigeritoContext {
    circuit: Circuit,
    wires: BiniusWires,
    prepared: BiniusLigerito,
    corpus: Corpus,
    setup_ms: f64,
    circuit_build_ms: f64,
}

impl BiniusLigeritoContext {
    fn setup(corpus: &Corpus) -> Self {
        let (circuit, wires, circuit_build_ms) = build_binius_sha_circuit(corpus);
        let setup_started = Instant::now();
        let prepared = BiniusLigerito::new(circuit.constraint_system())
            .expect("binius64-ligerito setup reaches the 100-bit gate");
        let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;
        Self {
            circuit,
            wires,
            prepared,
            corpus: corpus.clone(),
            setup_ms,
            circuit_build_ms,
        }
    }

    fn config_label(&self) -> String {
        format!(
            "rate3-johnson-ood-queries{}-component{}b-union{:.1}b",
            self.prepared.opener(0).level0_queries(),
            self.prepared.component_bits(),
            self.prepared.security().algebraic_bits
        )
    }

    fn populate(&self) -> ValueVec {
        let mut filler = self.circuit.new_witness_filler();
        for (pair_index, wires) in self.wires.pairs.iter().enumerate() {
            let low = &self.corpus.cases[2 * pair_index];
            let high = &self.corpus.cases[2 * pair_index + 1];
            for word in 0..8 {
                filler[wires.output[word]] = Word(pack_lanes(low.output[word], high.output[word]));
            }
            for word in 0..16 {
                filler[wires.block[word]] = Word(pack_lanes(low.block[word], high.block[word]));
            }
        }
        self.circuit
            .populate_wire_witness(&mut filler)
            .expect("Binius witness satisfies SHA relation");
        filler.into_value_vec()
    }

    fn run(&self, capture: &TraceCapture) -> (TrialMetrics, Vec<SemanticSpan>, Vec<u8>) {
        capture.begin();
        let trial = tracing::info_span!(
            "Verified trial",
            component = "binius-ligerito.verified-trial",
            scope_kind = "scope",
            tag_end_to_end = true,
        )
        .entered();
        let proving = tracing::info_span!(
            "Witness to proof",
            component = "binius-ligerito.witness-to-proof",
            scope_kind = "scope",
        )
        .entered();
        let witness = tracing::info_span!(
            "Witness generation",
            component = "binius-ligerito.witness-evaluation",
            scope_kind = "phase",
            tag_witness_generation = true,
        )
        .in_scope(|| self.populate());
        let proof = self
            .prepared
            .prove(&witness)
            .expect("binius64-ligerito proof succeeds");
        let proof_bytes = proof.to_bytes();
        drop(proving);
        let verification = tracing::info_span!(
            "Verification",
            component = "binius-ligerito.verification",
            scope_kind = "phase",
            tag_verification = true,
        )
        .entered();
        let decoded = self
            .prepared
            .proof_from_bytes(&proof_bytes)
            .expect("binius64-ligerito proof decodes");
        self.prepared
            .verify(witness.inout(), &decoded)
            .expect("binius64-ligerito proof verifies");
        drop(verification);
        drop(trial);
        let spans = binius_ligerito_semantic_spans(&capture.finish());
        let metrics = TrialMetrics::from_spans(&spans, proof_bytes.len());
        black_box(&proof_bytes);
        (metrics, spans, proof_bytes)
    }
}

/// Place measured phases without moving interleaved Round 0 work to the end.
fn binius_ligerito_semantic_spans(raw: &[CapturedSpan]) -> Vec<SemanticSpan> {
    let trial = BiniusLigeritoTrial::from_spans(raw);
    let phases = BiniusLigeritoPhases::from_spans(raw);
    let span = |id: &str,
                parent: Option<&str>,
                operation: &str,
                name: &str,
                short_name: &str,
                primary_phase: &'static str,
                phase_tags: Vec<&'static str>,
                start_ns: u64,
                end_ns: u64,
                scope_kind: &'static str,
                scope_tag: Option<&'static str>,
                primary_sequence: bool,
                math_latex: Vec<&'static str>| SemanticSpan {
        id: id.to_owned(),
        parent: parent.map(str::to_owned),
        operation: operation.to_owned(),
        name: name.to_owned(),
        short_name: short_name.to_owned(),
        primary_phase,
        phase_tags,
        start_ns,
        end_ns,
        scope_kind,
        scope_tag,
        primary_sequence,
        math_latex,
    };
    let mut spans = vec![
        span(
            "binius-ligerito-root",
            None,
            "binius-ligerito.verified-trial",
            "Complete verified Binius64-Ligerito trial",
            "Verified trial",
            "end-to-end",
            vec!["end-to-end"],
            trial.verified.start_ns,
            trial.verified.end_ns,
            "scope",
            Some("end-to-end"),
            false,
            vec![],
        ),
        span(
            "binius-ligerito-witness-to-proof",
            Some("binius-ligerito-root"),
            "binius-ligerito.witness-to-proof",
            "Binius64-Ligerito witness generation through proof readiness",
            "Witness to proof",
            "proving",
            vec!["proving"],
            trial.witness_to_proof.start_ns,
            trial.witness_to_proof.end_ns,
            "phase",
            None,
            false,
            vec![
                r"T_{\mathrm{witness\rightarrow proof}}=T_{\mathrm{witness}\rightarrow\mathrm{proof\ ready}}",
            ],
        ),
        span(
            "binius-ligerito-total-prover",
            Some("binius-ligerito-root"),
            "binius-ligerito.total-prover",
            "Binius64-Ligerito total prover",
            "Total prover",
            "proving",
            vec!["proving"],
            trial.witness.end_ns,
            trial.witness_to_proof.end_ns,
            "phase",
            Some("proving"),
            false,
            vec![r"T_{\mathrm{online}}=T_{\mathrm{commit}\rightarrow\mathrm{proof\ ready}}"],
        ),
        span(
            "binius-ligerito-witness-eval",
            Some("binius-ligerito-witness-to-proof"),
            "binius-ligerito.witness-evaluation",
            "Evaluate and pack the two-lane SHA witness",
            "Witness eval",
            "witness-generation",
            vec!["witness-generation"],
            trial.witness.start_ns,
            trial.witness.end_ns,
            "phase",
            None,
            true,
            vec![r"(x_i,x_{i+1})\mapsto x_i+2^{32}x_{i+1}"],
        ),
        span(
            "binius-ligerito-commit",
            Some("binius-ligerito-total-prover"),
            "binius-ligerito.commit",
            "Commit the packed witness (F2Z opener)",
            "Commit",
            "commit",
            vec!["commit", "pcs", "proving"],
            phases.commit.0,
            phases.commit.1,
            "phase",
            Some("commit"),
            true,
            vec![r"C_w=\operatorname{Merkle}(\operatorname{RS}_{1/8}(w))"],
        ),
        span(
            "binius-ligerito-verification",
            Some("binius-ligerito-root"),
            "binius-ligerito.verification",
            "Decode and verify the Binius64-Ligerito proof",
            "Verify",
            "verification",
            vec!["verification"],
            trial.verification.start_ns,
            trial.verification.end_ns,
            "phase",
            Some("verification"),
            true,
            vec![],
        ),
    ];
    for (id, operation, name, tag, tags, intervals) in [
        (
            "binius-ligerito-piop-reductions",
            "binius-ligerito.piop-reductions",
            "PIOP reductions",
            "constraint-proof",
            vec!["constraint-proof", "proving"],
            phases.piop,
        ),
        (
            "binius-ligerito-opening",
            "binius-ligerito.pcs-opening",
            "PCS opening",
            "opening-proof",
            vec!["opening-proof", "pcs", "proving"],
            phases.opening,
        ),
    ] {
        for (index, (start, end)) in intervals.into_iter().enumerate() {
            spans.push(span(
                &format!("{id}-{index}"),
                Some("binius-ligerito-total-prover"),
                operation,
                name,
                name,
                tag,
                tags.clone(),
                start,
                end,
                "phase",
                Some(tag),
                true,
                vec![],
            ));
        }
    }
    spans
}

fn pack_lanes(low: u32, high: u32) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}

fn f2z_semantic_spans(raw: &[ProfileInterval]) -> Vec<SemanticSpan> {
    let by_order = raw
        .iter()
        .map(|span| (span.order, span))
        .collect::<HashMap<_, _>>();
    let label_under = |span: &ProfileInterval, needle: &str| {
        let mut cursor = Some(span);
        while let Some(current) = cursor {
            if current.label == needle {
                return true;
            }
            cursor = current
                .parent_order
                .and_then(|parent| by_order.get(&parent).copied());
        }
        false
    };
    let id = |order| format!("f2z-{order}");
    let mut spans = raw
        .iter()
        .map(|span| {
            let (primary_phase, tags, scope_tag, primary_sequence, short_name, math) = match span
                .label
            {
                "sha256-compare:verified_trial" => (
                    "end-to-end",
                    vec!["end-to-end"],
                    Some("end-to-end"),
                    false,
                    "Verified trial",
                    vec![],
                ),
                "sha256-compare:total_prover" => (
                    "proving",
                    vec!["proving"],
                    Some("proving"),
                    false,
                    "Total prover",
                    vec![r"T_{\mathrm{online}}=T_{\mathrm{commit}\rightarrow\mathrm{proof\ ready}}"],
                ),
                "sha256-compare:witness_to_proof" => (
                    "proving",
                    vec!["proving"],
                    None,
                    false,
                    "Witness to proof",
                    vec![r"T_{\mathrm{witness\rightarrow proof}}=T_{\mathrm{witness}\rightarrow\mathrm{proof\ ready}}"],
                ),
                "sha256-compare:witness_generation" => (
                    "witness-generation",
                    vec!["witness-generation"],
                    None,
                    true,
                    "Witness",
                    vec![r"\widehat H_i=\operatorname{Compress}_{\mathrm{SHA256}}(\mathrm{IV},M_i)"],
                ),
                "sha256-compare:commit" => (
                    "commit",
                    vec!["commit", "pcs", "proving"],
                    Some("commit"),
                    true,
                    "Commit",
                    vec![r"C_f=\operatorname{Com}_{\mathbb F_{2^{128}}}(f)"],
                ),
                "sha256-compare:proof" => (
                    "proving",
                    vec!["proving"],
                    None,
                    false,
                    "PIOP + PCS",
                    vec![],
                ),
                "sha256-compare:verification" => (
                    "verification",
                    vec!["verification"],
                    Some("verification"),
                    true,
                    "Verify",
                    vec![],
                ),
                _ if label_under(span, "sha256-compare:witness_generation") => (
                    "witness-generation",
                    vec!["witness-generation"],
                    None,
                    false,
                    "Witness detail",
                    vec![],
                ),
                _ if label_under(span, "sha256-compare:commit") => (
                    "commit",
                    vec!["commit", "pcs", "proving"],
                    None,
                    false,
                    "Commit detail",
                    vec![],
                ),
                _ if label_under(span, "sha256-compare:verification") => (
                    "verification",
                    vec!["verification"],
                    None,
                    false,
                    "Verify detail",
                    vec![],
                ),
                _ if label_under(span, "sha256:f2z_prove")
                    || label_under(span, "sha256:opening_prepare_prover") =>
                {
                    (
                        "opening-proof",
                        vec!["opening-proof", "pcs", "proving"],
                        None,
                        false,
                        "F2Z PCS",
                        vec![r"\widetilde f(r)=v"],
                    )
                }
                _ if label_under(span, "sha256-compare:proof") => (
                    "proving",
                    vec!["proving"],
                    None,
                    false,
                    "PIOP detail",
                    vec![],
                ),
                _ => ("proving", vec!["proving"], None, false, "Procedure", vec![]),
            };
            SemanticSpan {
                id: id(span.order),
                parent: span.parent_order.map(id),
                operation: operation(span.label),
                name: humanize(span.label),
                short_name: short_name.to_owned(),
                primary_phase,
                phase_tags: tags,
                start_ns: span.start_ns,
                end_ns: span.end_ns,
                scope_kind: if span.depth <= 1 {
                    "phase"
                } else {
                    "procedure"
                },
                scope_tag,
                primary_sequence,
                math_latex: math,
            }
        })
        .collect::<Vec<_>>();

    let proof = raw
        .iter()
        .find(|span| span.label == "sha256-compare:proof")
        .expect("F2Z proof span exists");
    let opening_start = raw
        .iter()
        .filter(|span| {
            span.label.contains("opening_prepare_prover") || span.label == "sha256:f2z_prove"
        })
        .map(|span| span.start_ns)
        .min()
        .expect("F2Z opening boundary exists");
    spans.push(SemanticSpan {
        id: "f2z-piop-union".to_owned(),
        parent: Some(id(proof.order)),
        operation: "f2z.piop".to_owned(),
        name: "F2Z Spartan PIOP".to_owned(),
        short_name: "PIOP".to_owned(),
        primary_phase: "constraint-proof",
        phase_tags: vec!["constraint-proof", "proving"],
        start_ns: proof.start_ns,
        end_ns: opening_start,
        scope_kind: "phase",
        scope_tag: Some("constraint-proof"),
        primary_sequence: true,
        math_latex: vec![r"\sum_{x\in\{0,1\}^m}\operatorname{eq}(r,x)(A z)(x)(B z)(x)=(C z)(x)"],
    });
    spans.push(SemanticSpan {
        id: "f2z-opening-union".to_owned(),
        parent: Some(id(proof.order)),
        operation: "f2z.pcs-opening".to_owned(),
        name: "F2Z PCS opening".to_owned(),
        short_name: "F2Z opening".to_owned(),
        primary_phase: "opening-proof",
        phase_tags: vec!["opening-proof", "pcs", "proving"],
        start_ns: opening_start,
        end_ns: proof.end_ns,
        scope_kind: "phase",
        scope_tag: Some("opening-proof"),
        primary_sequence: true,
        math_latex: vec![
            r"\widetilde f(r)=v",
            r"\operatorname{Open}_{\mathrm{F2Z/Ligerito}}(C_f,r,v)",
        ],
    });
    spans
}

#[allow(clippy::too_many_arguments)]
fn binius_semantic_spans(
    raw: &[CapturedSpan],
    root_start: u64,
    root_end: u64,
    _total_start: u64,
    total_end: u64,
    witness_to_proof_start: u64,
    witness_start: u64,
    witness_end: u64,
    verify_start: u64,
    verify_end: u64,
) -> Vec<SemanticSpan> {
    let total_start = raw
        .iter()
        .filter(|span| span.component.as_deref() == Some("commit_witness"))
        .map(|span| span.start_ns)
        .min()
        .expect("Binius initial witness commitment span");
    let kept = raw
        .iter()
        .filter(|span| {
            span.component.is_some()
                && ((span.start_ns >= total_start && span.end_ns <= total_end)
                    || span.component.as_deref() == Some("prepare_witness"))
        })
        .map(|span| span.id)
        .collect::<HashSet<_>>();
    let by_id = raw
        .iter()
        .map(|span| (span.id, span))
        .collect::<HashMap<_, _>>();
    let parent_of = |span: &CapturedSpan| {
        let mut cursor = span.parent;
        while let Some(parent) = cursor {
            if kept.contains(&parent) {
                return Some(format!("binius-{parent}"));
            }
            cursor = by_id.get(&parent).and_then(|ancestor| ancestor.parent);
        }
        if span.start_ns < total_start {
            Some("binius-witness-to-proof".to_owned())
        } else {
            Some("binius-total-prover".to_owned())
        }
    };
    let under_component = |span: &CapturedSpan, target: &str| {
        let mut cursor = Some(span.id);
        while let Some(id) = cursor {
            let Some(current) = by_id.get(&id) else {
                break;
            };
            if current.component.as_deref() == Some(target) {
                return true;
            }
            cursor = current.parent;
        }
        false
    };

    let mut spans = vec![
        SemanticSpan {
            id: "binius-witness-to-proof".to_owned(),
            parent: Some("binius-root".to_owned()),
            operation: "binius.witness-to-proof".to_owned(),
            name: "Binius witness generation through proof readiness".to_owned(),
            short_name: "Witness to proof".to_owned(),
            primary_phase: "proving",
            phase_tags: vec!["proving"],
            start_ns: witness_to_proof_start,
            end_ns: total_end,
            scope_kind: "phase",
            scope_tag: None,
            primary_sequence: false,
            math_latex: vec![
                r"T_{\mathrm{witness\rightarrow proof}}=T_{\mathrm{witness}\rightarrow\mathrm{proof\ ready}}",
            ],
        },
        SemanticSpan {
            id: "binius-root".to_owned(),
            parent: None,
            operation: "binius.verified-trial".to_owned(),
            name: "Complete verified Binius trial".to_owned(),
            short_name: "Verified trial".to_owned(),
            primary_phase: "end-to-end",
            phase_tags: vec!["end-to-end"],
            start_ns: root_start,
            end_ns: root_end,
            scope_kind: "scope",
            scope_tag: Some("end-to-end"),
            primary_sequence: false,
            math_latex: vec![],
        },
        SemanticSpan {
            id: "binius-total-prover".to_owned(),
            parent: Some("binius-root".to_owned()),
            operation: "binius.total-prover".to_owned(),
            name: "Binius total prover".to_owned(),
            short_name: "Total prover".to_owned(),
            primary_phase: "proving",
            phase_tags: vec!["proving"],
            start_ns: total_start,
            end_ns: total_end,
            scope_kind: "phase",
            scope_tag: Some("proving"),
            primary_sequence: false,
            math_latex: vec![
                r"T_{\mathrm{online}}=T_{\mathrm{commit}\rightarrow\mathrm{proof\ ready}}",
            ],
        },
        SemanticSpan {
            id: "binius-witness-eval".to_owned(),
            parent: Some("binius-witness-to-proof".to_owned()),
            operation: "binius.witness-evaluation".to_owned(),
            name: "Evaluate and pack the two-lane SHA witness".to_owned(),
            short_name: "Witness eval".to_owned(),
            primary_phase: "witness-generation",
            phase_tags: vec!["witness-generation"],
            start_ns: witness_start,
            end_ns: witness_end,
            scope_kind: "phase",
            scope_tag: None,
            primary_sequence: true,
            math_latex: vec![r"(x_i,x_{i+1})\mapsto x_i+2^{32}x_{i+1}"],
        },
        SemanticSpan {
            id: "binius-verification".to_owned(),
            parent: Some("binius-root".to_owned()),
            operation: "binius.verification".to_owned(),
            name: "Verify Binius proof and consume transcript".to_owned(),
            short_name: "Verify".to_owned(),
            primary_phase: "verification",
            phase_tags: vec!["verification"],
            start_ns: verify_start,
            end_ns: verify_end,
            scope_kind: "phase",
            scope_tag: Some("verification"),
            primary_sequence: true,
            math_latex: vec![],
        },
    ];

    for span in raw.iter().filter(|span| kept.contains(&span.id)) {
        let component = span.component.as_deref().unwrap_or("procedure");
        let (phase, tags, scope_tag, primary, short, math) = if component == "prove" {
            ("proving", vec!["proving"], None, false, "IOP + PCS", vec![])
        } else if under_component(span, "prepare_witness") {
            (
                "witness-generation",
                vec!["witness-generation"],
                None,
                true,
                "Witness pack",
                vec![r"\mathbb F_2^{128}\ni f_j=\operatorname{pack}(w_{2j},w_{2j+1})"],
            )
        } else if under_component(span, "commit_witness") {
            (
                "commit",
                vec!["commit", "pcs", "proving"],
                Some("commit"),
                true,
                "Commit",
                vec![r"C_w=\operatorname{Merkle}(\operatorname{Encode}(w))"],
            )
        } else if under_component(span, "ring_switching")
            || under_component(span, "basefold_opening")
            || under_component(span, "finish_pcs")
        {
            (
                "opening-proof",
                vec!["opening-proof", "pcs", "proving"],
                None,
                component == "ring_switching",
                "PCS opening",
                vec![
                    r"\widetilde w(r)=v",
                    r"\operatorname{Open}_{\mathrm{BaseFold/FRI}}(C_w,r,v)",
                ],
            )
        } else {
            (
                "constraint-proof",
                vec!["constraint-proof", "sumcheck", "proving"],
                None,
                false,
                "PIOP",
                vec![r"\sum_x \operatorname{eq}(r,x)\,g(x)=0"],
            )
        };
        spans.push(SemanticSpan {
            id: format!("binius-{}", span.id),
            parent: parent_of(span),
            operation: format!("binius.{}", operation(component)),
            name: span.name.clone(),
            short_name: short.to_owned(),
            primary_phase: phase,
            phase_tags: tags,
            start_ns: span.start_ns,
            end_ns: span.end_ns,
            scope_kind: "phase",
            scope_tag,
            primary_sequence: primary,
            math_latex: math,
        });
    }

    let component = |name: &str| {
        raw.iter()
            .filter(|span| span.component.as_deref() == Some(name))
            .collect::<Vec<_>>()
    };
    let public = component("observe_public_input");
    let commit = component("commit_witness");
    let ring = component("ring_switching");
    let basefold = component("basefold_opening");
    assert_eq!(public.len(), 1, "one public-input absorption span");
    assert_eq!(commit.len(), 1, "one initial commitment span");
    assert_eq!(ring.len(), 1, "one ring-switching span");
    assert_eq!(basefold.len(), 1, "one BaseFold opening span");

    spans.push(SemanticSpan {
        id: "binius-piop-public".to_owned(),
        parent: Some("binius-witness-to-proof".to_owned()),
        operation: "binius.piop-public-input".to_owned(),
        name: "Binius PIOP public-input absorption".to_owned(),
        short_name: "Public input".to_owned(),
        primary_phase: "constraint-proof",
        phase_tags: vec!["constraint-proof", "proving"],
        start_ns: public[0].start_ns,
        end_ns: public[0].end_ns,
        scope_kind: "phase",
        scope_tag: Some("constraint-proof"),
        primary_sequence: true,
        math_latex: vec![r"\mathsf{tr}\leftarrow\mathsf{tr}\parallel(H,M,\widehat H)"],
    });
    spans.push(SemanticSpan {
        id: "binius-piop-reductions".to_owned(),
        parent: Some("binius-total-prover".to_owned()),
        operation: "binius.piop-reductions".to_owned(),
        name: "Binius constraint reductions".to_owned(),
        short_name: "PIOP reductions".to_owned(),
        primary_phase: "constraint-proof",
        phase_tags: vec!["constraint-proof", "sumcheck", "proving"],
        start_ns: commit[0].end_ns,
        end_ns: ring[0].start_ns,
        scope_kind: "phase",
        scope_tag: Some("constraint-proof"),
        primary_sequence: true,
        math_latex: vec![
            "A(x)B(x)-C(x)=0",
            r"\sum_x\operatorname{eq}(r,x)(A(x)B(x)-C(x))=0",
        ],
    });
    spans.push(SemanticSpan {
        id: "binius-opening-union".to_owned(),
        parent: Some("binius-total-prover".to_owned()),
        operation: "binius.pcs-opening".to_owned(),
        name: "Binius ring switching and BaseFold/FRI opening".to_owned(),
        short_name: "PCS opening".to_owned(),
        primary_phase: "opening-proof",
        phase_tags: vec!["opening-proof", "pcs", "fri", "proving"],
        start_ns: ring[0].start_ns,
        end_ns: basefold[0].end_ns,
        scope_kind: "phase",
        scope_tag: Some("opening-proof"),
        primary_sequence: true,
        math_latex: vec![r"\widetilde w(r)=v", r"\operatorname{FRI.Open}(C_w,r,v)"],
    });
    spans
}

struct TraceWriter {
    output: JsonlWriter<BufWriter<File>>,
    path: PathBuf,
    f2z_git: String,
    binius_git: String,
    plonky3_git: String,
    limber_git: String,
    f2z_dirty: bool,
    environment: Value,
    threads: usize,
    campaign_id: String,
}

struct RunMetadata<'a> {
    backend: Backend,
    exponent: usize,
    trial: Trial,
    corpus: &'a Corpus,
    setup_ms: f64,
    circuit_build_ms: Option<f64>,
    log_inv_rate: Option<usize>,
    query_count: Option<usize>,
    config_label: &'a str,
    security: Option<Value>,
}

impl TraceWriter {
    fn new(path: PathBuf, threads: usize) -> Self {
        if let Some(parent) = path.parent() {
            BenchmarkOutput::new(parent)
                .create_dir_all()
                .expect("create trace directory");
        }
        let output = BenchmarkOutput::new("")
            .jsonl(&path, FileMode::CreateNew)
            .expect("create fresh canonical trace");
        Self {
            output,
            path,
            f2z_git: command_output("git", &["rev-parse", "--short", "HEAD"], "unknown"),
            binius_git: common::locked_git_revision("binius-verifier").to_owned(),
            plonky3_git: common::locked_git_revision("p3-whir").to_owned(),
            limber_git: common::locked_git_revision("limber").to_owned(),
            f2z_dirty: git_dirty("."),
            environment: common::environment::metadata(threads),
            threads,
            campaign_id: format!(
                "sha256-claim-equivalent-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock")
                    .as_nanos()
            ),
        }
    }

    fn write_run(&mut self, metadata: RunMetadata<'_>, spans: &[SemanticSpan]) {
        let root = spans
            .iter()
            .find(|span| span.parent.is_none())
            .expect("one root span");
        assert_eq!(
            spans.iter().filter(|span| span.parent.is_none()).count(),
            1,
            "canonical run has exactly one root"
        );
        let run_id = format!(
            "{}-{}-sha256-2p{}-{}",
            self.campaign_id,
            metadata.backend.slug(),
            metadata.exponent,
            metadata.trial.slug()
        );
        let git_rev = match metadata.backend {
            Backend::F2z => &self.f2z_git,
            Backend::Plonky3Whir => &self.plonky3_git,
            Backend::Binius | Backend::BiniusLigerito => &self.binius_git,
            Backend::Limber => &self.limber_git,
        };
        let git_dirty = match metadata.backend {
            Backend::F2z => self.f2z_dirty,
            // External backends are built from the locked Git sources.
            _ => false,
        };
        let run = json!({
            "schema": "zkperf.trace/v1",
            "record": "run",
            "run_id": run_id,
            "series_id": format!("{}-{}-sha256-2p{}-{}-{}t", self.campaign_id, metadata.backend.slug(), metadata.exponent, metadata.config_label, self.threads),
            "root_span_id": root.id,
            "benchmark": {
                "suite": "sha256-claim-equivalent",
                "name": "sha256-e2e-compare",
                "label": format!(
                    "{} · 2^{} independent raw SHA-256 compressions",
                    metadata.backend.name(),
                    metadata.exponent,
                ),
                "algorithm": "SHA-256",
                "implementation": metadata.backend.name(),
                "git_rev": git_rev,
                "git_dirty": git_dirty,
                "build_profile": "bench",
            },
            "trial": metadata.trial.json(),
            "clock": {"id": run_id, "kind": "monotonic", "unit": "ns", "source": "std::time::Instant"},
            "status": "ok",
            "trace_complete": true,
            "environment": self.environment,
            "parameters": {
                "input": {
                    "sha256_compressions": 1usize << metadata.exponent,
                    "sha256_internal_rounds": 64usize << metadata.exponent,
                    "public_words_per_compression": 24,
                    "corpus_digest": metadata.corpus.digest,
                    "word_order": "M[0..16],Hhat[0..8] (big-endian u32 words)",
                    "input_state": "standard SHA-256 IV (fixed)",
                },
                "security": match metadata.backend {
                    Backend::F2z => metadata.security.clone().expect("resolved F2Z Ligerito policy"),
                    Backend::Plonky3Whir => metadata.security.clone().expect("WHIR security report"),
                    Backend::Binius => json!({
                        "profile": "100-bit FRI query-phase target",
                        "target_bits": BINIUS_SECURITY_BITS,
                        "fri_query_count": metadata.query_count,
                        "log_inv_rate": metadata.log_inv_rate,
                        "claim": "FRI query phase only; not a complete protocol union bound",
                    }),
                    Backend::BiniusLigerito => json!({
                        "profile": "F2Z opener: Johnson-regime Ligerito with fold/query grinding and Round 0",
                        "target_bits": 100,
                        "level0_query_count": metadata.query_count,
                        "log_inv_rate": metadata.log_inv_rate,
                        "claim": "whole-protocol union bound over the Binius64 PIOP, Round 0, ring switch and Ligerito terms",
                    }),
                    Backend::Limber => integer_limber_backend::security_metadata(),
                },
                "recursion": {"max_depth": 0, "instance_count": 1},
                "repetition": {"count": 1},
                "setup": {"setup_ms": metadata.setup_ms, "circuit_build_ms": metadata.circuit_build_ms},
                "selected_configuration": metadata.config_label,
            },
            "tags": {
                "campaign_id": self.campaign_id,
                "relation": "forall i<N: Hhat_i = Compress_SHA256(IV,M_i)",
                "padding": "out-of-scope",
                "chaining": "out-of-scope",
                "binius_lane_layout": "low32=compression 2j; high32=compression 2j+1",
            },
        });
        self.output.write(&run).expect("write run record");
        for span in spans {
            let mut attributes = json!({
                "scope_kind": span.scope_kind,
                "short_name": span.short_name,
                "primary_sequence": span.primary_sequence,
            });
            if let Some(scope_tag) = span.scope_tag {
                attributes["scope_tag"] = json!(scope_tag);
            }
            if !span.math_latex.is_empty() {
                attributes["math_latex"] = json!(span.math_latex);
            }
            let record = json!({
                "schema": "zkperf.trace/v1",
                "record": "span",
                "run_id": run_id,
                "span_id": span.id,
                "parent_span_id": span.parent,
                "operation": span.operation,
                "name": span.name,
                "primary_phase": span.primary_phase,
                "phase_tags": span.phase_tags,
                "start_ns": span.start_ns.to_string(),
                "end_ns": span.end_ns.to_string(),
                "duration_ns": span.duration_ns().to_string(),
                "lane": {"process": metadata.backend.slug(), "thread": "control"},
                "coordinate": {},
                "attributes": attributes,
            });
            self.output.write(&record).expect("write span record");
        }
        self.output.flush().expect("flush canonical trace");
    }
}

fn union_ms(spans: &[SemanticSpan], select: impl Fn(&SemanticSpan) -> bool) -> f64 {
    let mut intervals = spans
        .iter()
        .filter(|span| select(span))
        .map(|span| (span.start_ns, span.end_ns))
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    let mut total = 0u64;
    let mut current: Option<(u64, u64)> = None;
    for (start, end) in intervals {
        current = match current {
            None => Some((start, end)),
            Some((lo, hi)) if start <= hi => Some((lo, hi.max(end))),
            Some((lo, hi)) => {
                total = total.saturating_add(hi.saturating_sub(lo));
                Some((start, end))
            }
        };
    }
    if let Some((lo, hi)) = current {
        total = total.saturating_add(hi.saturating_sub(lo));
    }
    total as f64 / 1e6
}

fn run_binius_trial(
    context: &BiniusContext,
    capture: &TraceCapture,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    let (metrics, spans, _, _) = context.run(capture);
    if let Some(trace) = trace {
        let config_label = format!(
            "rate{}-queries{}-100b-fri-query-target",
            context.log_inv_rate, context.query_count
        );
        trace.write_run(
            RunMetadata {
                backend: Backend::Binius,
                exponent,
                trial,
                corpus: &context.corpus,
                setup_ms: context.setup_ms,
                circuit_build_ms: Some(context.circuit_build_ms),
                log_inv_rate: Some(context.log_inv_rate),
                query_count: Some(context.query_count),
                config_label: &config_label,
                security: None,
            },
            &spans,
        );
    }
    metrics
}

fn run_binius_ligerito_trial(
    context: &BiniusLigeritoContext,
    capture: &TraceCapture,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    let (metrics, spans, _) = context.run(capture);
    if let Some(trace) = trace {
        let config_label = context.config_label();
        trace.write_run(
            RunMetadata {
                backend: Backend::BiniusLigerito,
                exponent,
                trial,
                corpus: &context.corpus,
                setup_ms: context.setup_ms,
                circuit_build_ms: Some(context.circuit_build_ms),
                log_inv_rate: Some(f2z::binary_pcs::LOG_INV_RATE),
                query_count: Some(context.prepared.opener(0).level0_queries()),
                config_label: &config_label,
                security: None,
            },
            &spans,
        );
    }
    metrics
}

fn run_f2z_trial(
    context: &F2zContext,
    corpus: &Corpus,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    let (metrics, spans) = context.run();
    if let Some(trace) = trace {
        let config_label = format!("lambda100-prefix{}", context.inner_prefix_vars);
        trace.write_run(
            RunMetadata {
                backend: Backend::F2z,
                exponent,
                trial,
                corpus,
                setup_ms: context.setup_ms,
                circuit_build_ms: None,
                log_inv_rate: None,
                query_count: None,
                config_label: &config_label,
                security: Some(json!({"profile":"Lambda100", "target_bits":100, "ligerito":common::ligerito_report(context.prepared.ligerito_configuration().unwrap(), context.prepared.security().ood)})),
            },
            &spans,
        );
    }
    metrics
}

fn run_plonky3_trial(
    context: &plonky3_backend::Context,
    params: plonky3_backend::Params,
    capture: &TraceCapture,
    corpus: &Corpus,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    let (metrics, spans) = context.run(capture);
    if let Some(trace) = trace {
        let label = params.label();
        trace.write_run(
            RunMetadata {
                backend: Backend::Plonky3Whir,
                exponent,
                trial,
                corpus,
                setup_ms: context.setup_ms(),
                circuit_build_ms: None,
                log_inv_rate: Some(params.starting_log_inv_rate),
                query_count: None,
                config_label: &label,
                security: Some(context.security()),
            },
            &spans,
        );
    }
    metrics
}

fn run_integer_limber_trial(
    context: &integer_limber_backend::Context,
    params: integer_limber_backend::Params,
    capture: &TraceCapture,
    corpus: &Corpus,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    let (metrics, spans) = context.run(capture);
    if let Some(trace) = trace {
        let label = params.label();
        trace.write_run(
            RunMetadata {
                backend: Backend::Limber,
                exponent,
                trial,
                corpus,
                setup_ms: context.setup_ms(),
                circuit_build_ms: None,
                log_inv_rate: None,
                query_count: None,
                config_label: &label,
                security: None,
            },
            &spans,
        );
    }
    metrics
}

fn choose_binius_rate(
    capture: &TraceCapture,
    corpus: &Corpus,
    exponent: usize,
    reps: usize,
) -> usize {
    let mut rates = Vec::new();
    println!("\nBinius inverse-rate pilot at 2^{exponent} (100-bit FRI query-phase target):");
    for log_inv_rate in 1..=3 {
        let context = BiniusContext::setup(corpus, log_inv_rate);
        black_box(run_binius_trial(
            &context,
            capture,
            exponent,
            Trial::Warmup,
            None,
        ));
        let samples = (0..reps)
            .map(|sample| {
                run_binius_trial(&context, capture, exponent, Trial::Pilot(sample), None)
                    .total_prover_ms
            })
            .collect::<Vec<_>>();
        let median = common::median(&samples);
        println!(
            "  log_inv_rate={log_inv_rate}: median total prover {:.3} ms ({} queries, n={reps})",
            median, context.query_count
        );
        rates.push((median, log_inv_rate));
    }
    rates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let selected = rates[0].1;
    println!("  selected log_inv_rate={selected}\n");
    selected
}

fn choose_f2z_prefix(corpus: &Corpus, exponent: usize, reps: usize) -> usize {
    println!("\nF2Z inner-prefix pilot at 2^{exponent} (Lambda100):");
    let mut candidates = Vec::new();
    for prefix in 0..=4 {
        let context = F2zContext::setup(exponent, corpus, prefix);
        black_box(run_f2z_trial(
            &context,
            corpus,
            exponent,
            Trial::Warmup,
            None,
        ));
        let samples = (0..reps)
            .map(|sample| {
                run_f2z_trial(&context, corpus, exponent, Trial::Pilot(sample), None)
                    .total_prover_ms
            })
            .collect::<Vec<_>>();
        let median = common::median(&samples);
        println!("  prefix={prefix}: median online prover {median:.3} ms");
        candidates.push((median, prefix));
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("  selected prefix={}\n", candidates[0].1);
    candidates[0].1
}

fn choose_plonky3_params(
    capture: &TraceCapture,
    corpus: &Corpus,
) -> Result<(plonky3_backend::Params, whir_tuning::TuningReport), String> {
    let explicit =
        whir_tuning::replay()?.or_else(|| has_plonky3_override().then(env_plonky3_params));
    whir_tuning::tune(
        &[4, 5],
        explicit,
        |params| plonky3_backend::Context::setup(corpus, params),
        |context| context.run(capture).0.witness_to_proof_ms,
        plonky3_backend::Context::security,
    )
}

fn choose_integer_limber_params(
    capture: &TraceCapture,
    corpus: &Corpus,
    exponent: usize,
    reps: usize,
) -> integer_limber_backend::Params {
    use integer_limber_backend::Params;
    println!("\nInteger Limber pilot at 2^{exponent}:");
    let mut preflight = Vec::new();
    for k in 7..=13 {
        let params = Params { k };
        match integer_limber_backend::Context::setup(corpus, params) {
            Ok(context) => {
                let elapsed = run_integer_limber_trial(
                    &context,
                    params,
                    capture,
                    corpus,
                    exponent,
                    Trial::Preflight,
                    None,
                )
                .total_prover_ms;
                println!("  {}: preflight {elapsed:.3} ms", params.label());
                preflight.push((elapsed, params));
            }
            Err(error) => println!("  {}: ineligible ({error})", params.label()),
        }
    }
    preflight.sort_by(|a, b| a.0.total_cmp(&b.0));
    preflight.truncate(4);
    assert!(
        !preflight.is_empty(),
        "at least one eligible integer Limber configuration"
    );
    let mut finalists = Vec::new();
    for (_, params) in preflight {
        let context = integer_limber_backend::Context::setup(corpus, params)
            .expect("preflight-approved integer Limber configuration remains valid");
        black_box(run_integer_limber_trial(
            &context,
            params,
            capture,
            corpus,
            exponent,
            Trial::Warmup,
            None,
        ));
        let samples = (0..reps)
            .map(|sample| {
                run_integer_limber_trial(
                    &context,
                    params,
                    capture,
                    corpus,
                    exponent,
                    Trial::Pilot(sample),
                    None,
                )
                .total_prover_ms
            })
            .collect::<Vec<_>>();
        let median = common::median(&samples);
        println!("  finalist {}: median {median:.3} ms", params.label());
        finalists.push((median, params));
    }
    finalists.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("  selected {}\n", finalists[0].1.label());
    finalists[0].1
}

fn preflight_backend(
    backend: Backend,
    exponent: usize,
    selected: &SelectedConfigs,
    seed: u64,
) -> bool {
    println!(
        "Preflighting {} at 2^{exponent} in an isolated process...",
        backend.name(),
    );
    let status = std::env::current_exe()
        .and_then(|executable| {
            Command::new(executable)
                .env("F2Z_SHA_COMPARE_PREFLIGHT_CHILD", backend.slug())
                .env("F2Z_SHA_COMPARE_PREFLIGHT_EXPONENT", exponent.to_string())
                .env(
                    "F2Z_SHA_COMPARE_F2Z_PREFIX",
                    selected.f2z_prefix.to_string(),
                )
                .env(
                    "F2Z_SHA_COMPARE_LOG_INV_RATE",
                    selected.binius_rate.to_string(),
                )
                .env(
                    "F2Z_SHA_COMPARE_P3_EXTENSION_DEGREE",
                    selected.plonky3.extension_degree.to_string(),
                )
                .env(
                    "F2Z_SHA_COMPARE_P3_FOLDING",
                    selected.plonky3.folding.to_string(),
                )
                .env(
                    "F2Z_SHA_COMPARE_P3_LOG_INV_RATE",
                    selected.plonky3.starting_log_inv_rate.to_string(),
                )
                .env(
                    "F2Z_SHA_COMPARE_P3_MAX_POW_BITS",
                    selected.plonky3.max_pow_bits.to_string(),
                )
                .env("F2Z_SHA_COMPARE_LIMBER_ENGINE", "brakedown")
                .env("F2Z_SHA_COMPARE_LIMBER_K", selected.limber.k.to_string())
                .env("F2Z_SHA_COMPARE_SEED", seed.to_string())
                .status()
        })
        .expect("launch isolated preflight child");
    assert!(
        status.success(),
        "{} preflight at 2^{exponent} failed with {status}; inspect the child diagnostics (not classified as a resource limit)",
        backend.name()
    );
    true
}

#[derive(Clone, Copy)]

struct SelectedConfigs {
    f2z_prefix: usize,
    binius_rate: usize,
    plonky3: plonky3_backend::Params,
    limber: integer_limber_backend::Params,
}

struct SizeContexts {
    f2z: Option<F2zContext>,
    plonky3: Option<plonky3_backend::Context>,
    binius: Option<BiniusContext>,
    binius_ligerito: Option<BiniusLigeritoContext>,
    limber: Option<integer_limber_backend::Context>,
}

#[allow(clippy::too_many_arguments)]
fn run_native_trial(
    backend: Backend,
    contexts: &SizeContexts,
    selected: &SelectedConfigs,
    capture: &TraceCapture,
    corpus: &Corpus,
    exponent: usize,
    trial: Trial,
    trace: Option<&mut TraceWriter>,
) -> TrialMetrics {
    #[cfg(feature = "bench-perfetto")]
    let recording = trace
        .as_ref()
        .map(|trace| {
            let output = BenchmarkOutput::new(trace.path.parent().unwrap_or(Path::new(".")));
            common::perfetto::Recording::start(output.buffered(
                format!(
                    "sha256-{}-{exponent}-{}.pftrace",
                    backend.slug(),
                    trial.slug()
                ),
                FileMode::CreateNew,
            )?)
        })
        .transpose()
        .expect("start Perfetto recording");
    #[cfg(feature = "bench-perfetto")]
    let trial_span = tracing::info_span!(
        "benchmark_trial",
        component = "native_sha256.trial",
        backend = backend.slug(),
        exponent,
        trial = trial.slug(),
        warmup = matches!(trial, Trial::Warmup),
    )
    .entered();
    let metrics = match backend {
        Backend::F2z => run_f2z_trial(
            contexts.f2z.as_ref().expect("F2Z context"),
            corpus,
            exponent,
            trial,
            trace,
        ),
        Backend::Plonky3Whir => run_plonky3_trial(
            contexts.plonky3.as_ref().expect("Plonky3 context"),
            selected.plonky3,
            capture,
            corpus,
            exponent,
            trial,
            trace,
        ),
        Backend::Binius => run_binius_trial(
            contexts.binius.as_ref().expect("Binius context"),
            capture,
            exponent,
            trial,
            trace,
        ),
        Backend::BiniusLigerito => run_binius_ligerito_trial(
            contexts
                .binius_ligerito
                .as_ref()
                .expect("Binius64-Ligerito context"),
            capture,
            exponent,
            trial,
            trace,
        ),
        Backend::Limber => run_integer_limber_trial(
            contexts.limber.as_ref().expect("Limber context"),
            selected.limber,
            capture,
            corpus,
            exponent,
            trial,
            trace,
        ),
    };
    #[cfg(feature = "bench-perfetto")]
    {
        drop(trial_span);
        if let Some(recording) = recording {
            recording.finish().expect("finish Perfetto recording");
        }
    }
    metrics
}

fn run_campaign(
    capture: &TraceCapture,
    trace: &mut TraceWriter,
    exponents: &[usize],
    reps: usize,
    root_seed: u64,
    selected: &SelectedConfigs,
    requested: &HashSet<Backend>,
    allowed_at_16: &HashSet<Backend>,
    output_dir: &Path,
) -> Vec<BackendAggregate> {
    let mut aggregates = Vec::new();
    for &exponent in exponents {
        let compressions = 1usize << exponent;
        let corpus = Corpus::new(compressions, shape_seed(root_seed, exponent));
        println!(
            "\n=== 2^{exponent} = {compressions} public SHA-256 compressions | corpus {} ===",
            &corpus.digest[..16]
        );
        let enabled = |backend| {
            requested.contains(&backend) && (exponent != 16 || allowed_at_16.contains(&backend))
        };
        let mut selected = *selected;
        let mut tuning = None;
        let plonky3 = if enabled(Backend::Plonky3Whir) {
            let path = output_dir.join(format!("whir-sha256-{exponent}.json"));
            match choose_plonky3_params(capture, &corpus) {
                Ok((params, mut record)) => {
                    selected.plonky3 = params;
                    record.workload = Some("sha256".to_owned());
                    record.exponent = Some(exponent);
                    record.corpus_digest = Some(corpus.digest.clone());
                    whir_tuning::save(&path, &record).expect("save per-size WHIR tuning");
                    tuning = Some(record);
                    Some(
                        plonky3_backend::Context::setup(&corpus, params)
                            .expect("selected WHIR config"),
                    )
                }
                Err(reason) => {
                    eprintln!("WHIR sha256 2^{exponent}: unavailable: {reason}");
                    whir_tuning::save(
                        &path,
                        &json!({"workload":"sha256", "exponent":exponent,
                        "status":"ineligible", "reason":reason}),
                    )
                    .expect("save WHIR ineligibility");
                    None
                }
            }
        } else {
            None
        };
        let selected = &selected;
        let contexts = SizeContexts {
            f2z: enabled(Backend::F2z)
                .then(|| F2zContext::setup(exponent, &corpus, selected.f2z_prefix)),
            plonky3,
            binius: enabled(Backend::Binius)
                .then(|| BiniusContext::setup(&corpus, selected.binius_rate)),
            binius_ligerito: enabled(Backend::BiniusLigerito)
                .then(|| BiniusLigeritoContext::setup(&corpus)),
            limber: enabled(Backend::Limber).then(|| {
                integer_limber_backend::Context::setup(&corpus, selected.limber)
                    .expect("frozen integer Limber config remains valid")
            }),
        };
        let active = ALL_BACKENDS
            .into_iter()
            .filter(|backend| {
                enabled(*backend)
                    && (*backend != Backend::Plonky3Whir || contexts.plonky3.is_some())
            })
            .collect::<Vec<_>>();
        let mut samples = HashMap::<Backend, Vec<TrialMetrics>>::new();

        for &backend in &active {
            black_box(run_native_trial(
                backend,
                &contexts,
                selected,
                capture,
                &corpus,
                exponent,
                Trial::Warmup,
                Some(trace),
            ));
        }
        for sample in 0..reps {
            for offset in 0..active.len() {
                let backend = active[(sample + offset) % active.len()];
                let metrics = run_native_trial(
                    backend,
                    &contexts,
                    selected,
                    capture,
                    &corpus,
                    exponent,
                    Trial::Sample(sample),
                    Some(trace),
                );
                samples.entry(backend).or_default().push(metrics);
            }
            println!(
                "  completed sample {}/{reps} across {} active backends",
                sample + 1,
                active.len()
            );
        }
        for backend in active {
            let (setup_ms, config) = match backend {
                Backend::F2z => (
                    contexts.f2z.as_ref().unwrap().setup_ms,
                    format!("lambda100-prefix{}", selected.f2z_prefix),
                ),
                Backend::Plonky3Whir => (
                    contexts.plonky3.as_ref().unwrap().setup_ms(),
                    selected.plonky3.label(),
                ),
                Backend::Binius => {
                    let context = contexts.binius.as_ref().unwrap();
                    (
                        context.setup_ms + context.circuit_build_ms,
                        format!(
                            "rate{}-queries{}",
                            context.log_inv_rate, context.query_count
                        ),
                    )
                }
                Backend::BiniusLigerito => {
                    let context = contexts.binius_ligerito.as_ref().unwrap();
                    (
                        context.setup_ms + context.circuit_build_ms,
                        context.config_label(),
                    )
                }
                Backend::Limber => (
                    contexts.limber.as_ref().unwrap().setup_ms(),
                    selected.limber.label(),
                ),
            };
            aggregates.push(BackendAggregate {
                backend,
                exponent,
                setup_ms,
                config,
                whir_tuning: (backend == Backend::Plonky3Whir)
                    .then(|| tuning.clone().expect("WHIR tuning record")),
                samples: samples.remove(&backend).unwrap_or_default(),
            });
        }
    }
    aggregates
}

fn print_tables(aggregates: &[BackendAggregate]) {
    for backend in ALL_BACKENDS {
        println!("\n{} medians:", backend.name());
        println!(
            "| N | config | witness ms | commit ms | PIOP ms | IOP ms | online ms | witness->proof ms | throughput /s | verifier ms | proof bytes | setup ms |"
        );
        println!("|---:|:---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        let mut rows = aggregates
            .iter()
            .filter(|row| row.backend == backend)
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.exponent);
        for row in rows {
            let m = row.median();
            let throughput = (1usize << row.exponent) as f64 * 1e3 / m.witness_to_proof_ms;
            println!(
                "| 2^{} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {:.3} |",
                row.exponent,
                row.config,
                m.witness_ms,
                m.commit_ms,
                m.piop_ms,
                m.opening_ms,
                m.total_prover_ms,
                m.witness_to_proof_ms,
                throughput,
                m.verifier_ms,
                m.proof_bytes,
                row.setup_ms,
            );
        }
    }

    println!(
        "\nCombined medians (F2Z / Plonky3-WHIR / Binius64 / Binius64-Ligerito / Limber-Brakedown):"
    );
    println!(
        "| N | witness ms | commit ms | PIOP ms | IOP ms | online ms | witness->proof ms | throughput /s | verifier ms | proof bytes | setup ms |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut exponents = aggregates
        .iter()
        .map(|row| row.exponent)
        .collect::<Vec<_>>();
    exponents.sort_unstable();
    exponents.dedup();
    for exponent in exponents {
        let medians = ALL_BACKENDS.map(|backend| {
            aggregates
                .iter()
                .find(|row| row.exponent == exponent && row.backend == backend)
                .map(BackendAggregate::median)
        });
        let metric = |f: fn(&TrialMetrics) -> f64| {
            format_backends(
                medians.each_ref().map(|value| value.as_ref().map(|m| f(m))),
                false,
                3,
            )
        };
        let throughput = format_backends(
            medians.each_ref().map(|value| {
                value
                    .as_ref()
                    .map(|m| (1usize << exponent) as f64 * 1e3 / m.witness_to_proof_ms)
            }),
            true,
            3,
        );
        let proof = format_backends(
            medians
                .each_ref()
                .map(|value| value.as_ref().map(|m| m.proof_bytes as f64)),
            false,
            0,
        );
        let setup_values = ALL_BACKENDS.map(|backend| {
            aggregates
                .iter()
                .find(|row| row.exponent == exponent && row.backend == backend)
                .map(|row| row.setup_ms)
        });
        println!(
            "| 2^{exponent} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            metric(|m| m.witness_ms),
            metric(|m| m.commit_ms),
            metric(|m| m.piop_ms),
            metric(|m| m.opening_ms),
            metric(|m| m.total_prover_ms),
            metric(|m| m.witness_to_proof_ms),
            throughput,
            metric(|m| m.verifier_ms),
            proof,
            format_backends(setup_values, false, 3),
        );
    }
    println!(
        "Security labels: F2Z Lambda100; Plonky3 analyzed WHIR >=100 bits; Binius 100-bit FRI query-phase target only; Binius64-Ligerito 100-bit whole-protocol union bound (F2Z opener, rate 1/2, Johnson regime, grinding, Round 0); integer Limber uses Brakedown with its native 114-bit column-opening target. These are the existing classical security estimates, not quantum-bit guarantees."
    );
}

#[derive(serde::Serialize)]
struct SummaryMetrics {
    #[serde(flatten)]
    metrics: TrialMetrics,
    throughput_compressions_per_s: f64,
}

#[derive(serde::Serialize)]
struct SummaryRow<'a> {
    backend: &'static str,
    backend_slug: &'static str,
    compression_exponent: usize,
    compressions: usize,
    configuration: &'a str,
    limber_security: Option<Value>,
    whir_tuning: &'a Option<whir_tuning::TuningReport>,
    measured_samples: usize,
    setup_ms: f64,
    median: SummaryMetrics,
}

impl BackendAggregate {
    fn summary_row(&self) -> SummaryRow<'_> {
        let median = self.median();
        SummaryRow {
            backend: self.backend.name(),
            backend_slug: self.backend.slug(),
            compression_exponent: self.exponent,
            compressions: 1usize << self.exponent,
            configuration: &self.config,
            limber_security: (self.backend == Backend::Limber)
                .then(integer_limber_backend::security_metadata),
            whir_tuning: &self.whir_tuning,
            measured_samples: self.samples.len(),
            setup_ms: self.setup_ms,
            median: SummaryMetrics {
                throughput_compressions_per_s: (1usize << self.exponent) as f64 * 1e3
                    / median.witness_to_proof_ms,
                metrics: median,
            },
        }
    }
}

#[derive(serde::Serialize)]
struct CsvRow<'a> {
    backend: &'static str,
    compression_exponent: usize,
    compressions: usize,
    configuration: &'a str,
    sample_index: usize,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    witness_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    commit_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    piop_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    iop_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    online_prover_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    witness_to_proof_ms: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    throughput_compressions_per_s: f64,
    #[serde(serialize_with = "common::output::csv_format::nine_decimals")]
    verifier_ms: f64,
    proof_bytes: usize,
    #[serde(serialize_with = "common::output::csv_format::display")]
    setup_ms: f64,
}
impl CsvRow<'_> {
    const HEADER: [&'static str; 15] = [
        "backend",
        "compression_exponent",
        "compressions",
        "configuration",
        "sample_index",
        "witness_ms",
        "commit_ms",
        "piop_ms",
        "iop_ms",
        "online_prover_ms",
        "witness_to_proof_ms",
        "throughput_compressions_per_s",
        "verifier_ms",
        "proof_bytes",
        "setup_ms",
    ];
}

#[derive(serde::Serialize)]
struct SummaryDocument<'a> {
    schema: &'static str,
    commitment_policy: &'static str,
    primary_metric: &'static str,
    environment: Value,
    statement: &'static str,
    padding: bool,
    chaining: bool,
    backend_order: [&'static str; ALL_BACKENDS.len()],
    unavailable_backends: Vec<&'static str>,
    rows: Vec<SummaryRow<'a>>,
}

fn write_aggregate_artifacts(
    aggregates: &[BackendAggregate],
    requested: &HashSet<Backend>,
    runnable: &HashSet<Backend>,
    output_dir: &Path,
) {
    let output = BenchmarkOutput::new(output_dir);
    output
        .create_dir_all()
        .expect("create benchmark output directory");
    let summary_path = output_dir.join("summary.json");
    let summary_doc = SummaryDocument {
        schema: "native-sha256-comparison/v3",
        commitment_policy: "hash-based-only",
        primary_metric: "witness_to_proof_ms",
        environment: common::environment::metadata(rayon::current_num_threads()),
        statement: "forall i<N: Hhat_i = Compress_SHA256(IV,M_i)",
        padding: false,
        chaining: false,
        backend_order: ALL_BACKENDS.map(Backend::name),
        unavailable_backends: ALL_BACKENDS
            .into_iter()
            .filter(|backend| requested.contains(backend) && !runnable.contains(backend))
            .map(Backend::name)
            .collect(),
        rows: aggregates
            .iter()
            .map(BackendAggregate::summary_row)
            .collect(),
    };
    output
        .write_json(
            "summary.json",
            &summary_doc,
            FileMode::Replace,
            JsonStyle::Pretty,
        )
        .expect("write summary.json");

    let metrics_path = output_dir.join("metrics.csv");
    let mut metrics = output
        .csv("metrics.csv", FileMode::Replace)
        .expect("create metrics.csv");
    metrics
        .write_record(CsvRow::HEADER)
        .expect("write CSV header");
    for row in aggregates {
        for (sample_index, sample) in row.samples.iter().enumerate() {
            let throughput = (1usize << row.exponent) as f64 * 1e3 / sample.witness_to_proof_ms;
            metrics
                .serialize(CsvRow {
                    backend: row.backend.slug(),
                    compression_exponent: row.exponent,
                    compressions: 1usize << row.exponent,
                    configuration: &row.config,
                    sample_index,
                    witness_ms: sample.witness_ms,
                    commit_ms: sample.commit_ms,
                    piop_ms: sample.piop_ms,
                    iop_ms: sample.opening_ms,
                    online_prover_ms: sample.total_prover_ms,
                    witness_to_proof_ms: sample.witness_to_proof_ms,
                    throughput_compressions_per_s: throughput,
                    verifier_ms: sample.verifier_ms,
                    proof_bytes: sample.proof_bytes,
                    setup_ms: row.setup_ms,
                })
                .expect("write CSV row");
        }
    }
    metrics.flush().expect("flush metrics.csv");
    println!("summary: {}", summary_path.display());
    println!("metrics: {}", metrics_path.display());
}

fn format_backends(
    values: [Option<f64>; ALL_BACKENDS.len()],
    higher_is_better: bool,
    decimals: usize,
) -> String {
    let winner = values.iter().flatten().copied().reduce(|best, value| {
        if higher_is_better {
            best.max(value)
        } else {
            best.min(value)
        }
    });
    values
        .into_iter()
        .map(|value| match (value, winner) {
            (Some(value), Some(winner)) => {
                let rendered = format!("{value:.decimals$}");
                if (value - winner).abs() <= f64::EPSILON * winner.abs().max(1.0) {
                    format!("**{rendered}**")
                } else {
                    rendered
                }
            }
            (None, _) => "unavailable".to_owned(),
            _ => "n/a".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn security_api_self_test() {
    let builder = CircuitBuilder::new();
    let public_a = builder.add_inout();
    let public_b = builder.add_inout();
    builder.assert_eq("security setup smoke test", public_a, public_b);
    let circuit = builder.build();
    let legacy = BiniusVerifier::<StdHashSuite>::setup(circuit.constraint_system().clone(), 1)
        .expect("legacy setup succeeds");
    let comparison = BiniusVerifier::<StdHashSuite>::setup_with_security_bits(
        circuit.constraint_system().clone(),
        1,
        BINIUS_SECURITY_BITS,
    )
    .expect("comparison setup succeeds");
    assert_eq!(
        legacy.fri_params().n_test_queries(),
        calculate_n_test_queries(96, 1)
    );
    assert_eq!(
        comparison.fri_params().n_test_queries(),
        calculate_n_test_queries(BINIUS_SECURITY_BITS, 1)
    );
}

fn f2z_public_tamper_self_test() {
    // The smallest supported batch exercises the same public-statement
    // transcript binding as every measured size without slowing startup.
    let corpus = Corpus::new(1 << 7, DEFAULT_ROOT_SEED ^ 0x4632_5a5f_5441_4d50);
    let context = F2zContext::setup(7, &corpus, SHA256_DEFAULT_INNER_PREFIX_VARS);
    let witness = generate_sha256_compression_witnesses(&context.prepared, &context.inputs)
        .expect("F2Z tamper-test witness");
    let hint =
        commit_sha256_compression_witness_with_config(&context.prepared, &witness, &context.pc)
            .expect("F2Z tamper-test commitment");
    let mut prover_transcript = Blake3Transcript::new();
    let proof = prove_sha256_compressions_with_prefix_vars_and_config(
        &mut prover_transcript,
        &context.prepared,
        &context.statements,
        &witness,
        &hint,
        SHA256_DEFAULT_INNER_PREFIX_VARS,
        &context.pc,
    )
    .expect("F2Z tamper-test proof");

    for class in 0..2 {
        let mut statements = context.statements.clone();
        match class {
            0 => statements[0].block[0] ^= 1,
            1 => statements[0].claimed_output[0] ^= 1,
            _ => unreachable!(),
        }
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_sha256_compressions_with_config(
                &mut verifier_transcript,
                &context.prepared,
                &statements,
                &hint.commitment,
                &proof,
                &context.vc,
            )
            .is_err(),
            "tampering with an F2Z public block or output must fail"
        );
    }
}

fn binius_tamper_self_test(capture: &TraceCapture) {
    let corpus = Corpus::new(2, DEFAULT_ROOT_SEED ^ 0x5441_4d50_4552);
    let context = BiniusContext::setup(&corpus, 1);
    let (metrics, _, proof, witness) = context.run(capture);
    assert!(metrics.witness_ms > 0.0);
    assert!(metrics.commit_ms > 0.0);
    assert!(metrics.piop_ms > 0.0);
    assert!(metrics.opening_ms > 0.0);

    let pair = &context.wires.pairs[0];
    let low = &corpus.cases[0];
    let high = &corpus.cases[1];
    assert_eq!(
        witness[context.circuit.witness_index(pair.block[0])].as_u64(),
        pack_lanes(low.block[0], high.block[0]),
        "two-lane adapter preserves distinct low/high block words"
    );
    assert_eq!(
        witness[context.circuit.witness_index(pair.output[0])].as_u64(),
        pack_lanes(low.output[0], high.output[0]),
        "two-lane adapter preserves distinct low/high output words"
    );

    for wire in [
        context.wires.pairs[0].block[0],
        context.wires.pairs[0].output[0],
    ] {
        let mut tampered = witness.clone();
        let index = context.circuit.witness_index(wire);
        tampered[index] = Word(tampered[index].as_u64() ^ 1);
        let mut transcript = VerifierTranscript::new(StdChallenger::default(), proof.clone());
        assert!(
            context
                .verifier
                .verify(tampered.inout(), &mut transcript)
                .is_err()
                || transcript.finalize().is_err(),
            "tampering with a public block or output must fail"
        );
    }

    let mut tampered_proof = proof.clone();
    let midpoint = tampered_proof.len() / 2;
    tampered_proof[midpoint] ^= 1;
    let mut transcript = VerifierTranscript::new(StdChallenger::default(), tampered_proof);
    assert!(
        context
            .verifier
            .verify(witness.inout(), &mut transcript)
            .is_err()
            || transcript.finalize().is_err(),
        "proof-byte tampering must fail"
    );

    let mut trailing = proof;
    trailing.push(0);
    let mut transcript = VerifierTranscript::new(StdChallenger::default(), trailing);
    let verified = context
        .verifier
        .verify(witness.inout(), &mut transcript)
        .is_ok();
    assert!(
        !verified || transcript.finalize().is_err(),
        "trailing transcript data must fail"
    );
}

fn parse_exponents() -> Vec<usize> {
    let text = std::env::var("F2Z_SHA_COMPARE_EXPONENTS")
        .or_else(|_| std::env::var("F2Z_BENCH_SHAPES"))
        .unwrap_or_else(|_| DEFAULT_EXPONENTS.to_owned());
    let mut exponents = text
        .split([',', ' '])
        .filter(|piece| !piece.is_empty())
        .map(|piece| piece.parse::<usize>().expect("integer SHA exponent"))
        .collect::<Vec<_>>();
    assert!(!exponents.is_empty());
    let minimum = if env_bool("F2Z_SHA_COMPARE_ALLOW_SMALL", false) {
        0
    } else {
        7
    };
    assert!(
        exponents
            .iter()
            .all(|exponent| (minimum..=16).contains(exponent))
    );
    exponents.sort_unstable();
    exponents.dedup();
    exponents
}

fn parse_backends() -> HashSet<Backend> {
    let value = std::env::var("F2Z_SHA_COMPARE_BACKENDS")
        .unwrap_or_else(|_| ALL_BACKENDS.map(Backend::slug).join(","));
    let backends = value
        .split([',', ' '])
        .filter(|piece| !piece.is_empty())
        .map(|piece| {
            Backend::parse(piece).unwrap_or_else(|| panic!("unknown backend slug {piece}"))
        })
        .collect::<HashSet<_>>();
    assert!(!backends.is_empty(), "select at least one backend");
    backends
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |value| {
        value
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("{name} must be an integer"))
    })
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |value| match value.as_str() {
        "1" | "true" | "yes" => true,
        "0" | "false" | "no" => false,
        _ => panic!("{name} must be 0/1, true/false, or yes/no"),
    })
}

fn env_plonky3_params() -> plonky3_backend::Params {
    plonky3_backend::Params {
        extension_degree: env_usize("F2Z_SHA_COMPARE_P3_EXTENSION_DEGREE", 5),
        folding: env_usize("F2Z_SHA_COMPARE_P3_FOLDING", 4),
        starting_log_inv_rate: env_usize("F2Z_SHA_COMPARE_P3_LOG_INV_RATE", 1),
        max_pow_bits: env_usize("F2Z_SHA_COMPARE_P3_MAX_POW_BITS", 12),
        max_round_log_inv_rate: None,
    }
}

fn env_limber_params() -> integer_limber_backend::Params {
    let engine =
        std::env::var("F2Z_SHA_COMPARE_LIMBER_ENGINE").unwrap_or_else(|_| "brakedown".to_owned());
    integer_limber_backend::validate_engine(&engine).expect("SHA commitment policy");
    integer_limber_backend::Params {
        k: env_usize("F2Z_SHA_COMPARE_LIMBER_K", 9),
    }
}

fn has_plonky3_override() -> bool {
    [
        "F2Z_SHA_COMPARE_P3_EXTENSION_DEGREE",
        "F2Z_SHA_COMPARE_P3_FOLDING",
        "F2Z_SHA_COMPARE_P3_LOG_INV_RATE",
        "F2Z_SHA_COMPARE_P3_MAX_POW_BITS",
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some())
}

fn has_limber_override() -> bool {
    std::env::var_os("F2Z_SHA_COMPARE_LIMBER_K").is_some()
}

fn shape_seed(root: u64, exponent: usize) -> u64 {
    root ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn operation(label: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in label.chars() {
        let character = match character {
            'A'..='Z' => character.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' | '-' => character,
            _ => '.',
        };
        if character == '.' {
            if !separator && !output.is_empty() {
                output.push(character);
            }
            separator = true;
        } else {
            output.push(character);
            separator = false;
        }
    }
    output.trim_end_matches('.').to_owned()
}

fn humanize(label: &str) -> String {
    label
        .replace([':', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_output(program: &str, args: &[&str], fallback: &str) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_owned())
        .filter(|output| !output.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn git_dirty(path: &str) -> bool {
    Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output()
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true)
}

#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
}

fn main() {
    // SAFETY: set before the Rayon pool or any profiler scope is created.
    unsafe {
        std::env::set_var("OBLONG_PROFILE", "1");
        std::env::set_var("OBLONG_PROFILE_INTERVALS", "1");
    }
    let _ = flock_core::init_perf_thread_pool();
    let threads = rayon::current_num_threads();
    let expected_threads = env_usize("F2Z_SHA_COMPARE_THREADS", threads);
    assert_eq!(
        threads, expected_threads,
        "set RAYON_NUM_THREADS={expected_threads} for the controlled comparison"
    );
    let limber_params = env_limber_params();
    let capture = CaptureLayer::install();

    if let Ok(backend_slug) = std::env::var("F2Z_SHA_COMPARE_PREFLIGHT_CHILD") {
        let backend = Backend::parse(&backend_slug).expect("valid preflight backend slug");
        let exponent = env_usize("F2Z_SHA_COMPARE_PREFLIGHT_EXPONENT", 16);
        let seed = env_usize("F2Z_SHA_COMPARE_SEED", DEFAULT_ROOT_SEED as usize) as u64;
        let corpus = Corpus::new(1 << exponent, shape_seed(seed, exponent));
        let metrics = match backend {
            Backend::F2z => {
                let context = F2zContext::setup(
                    exponent,
                    &corpus,
                    env_usize(
                        "F2Z_SHA_COMPARE_F2Z_PREFIX",
                        SHA256_DEFAULT_INNER_PREFIX_VARS,
                    ),
                );
                run_f2z_trial(&context, &corpus, exponent, Trial::Preflight, None)
            }
            Backend::Plonky3Whir => {
                let params = env_plonky3_params();
                let context = plonky3_backend::Context::setup(&corpus, params)
                    .expect("preflight WHIR configuration is eligible");
                run_plonky3_trial(
                    &context,
                    params,
                    &capture,
                    &corpus,
                    exponent,
                    Trial::Preflight,
                    None,
                )
            }
            Backend::Binius => {
                let context =
                    BiniusContext::setup(&corpus, env_usize("F2Z_SHA_COMPARE_LOG_INV_RATE", 1));
                run_binius_trial(&context, &capture, exponent, Trial::Preflight, None)
            }
            Backend::BiniusLigerito => {
                let context = BiniusLigeritoContext::setup(&corpus);
                run_binius_ligerito_trial(&context, &capture, exponent, Trial::Preflight, None)
            }
            Backend::Limber => {
                let params = limber_params;
                let context = integer_limber_backend::Context::setup(&corpus, params)
                    .expect("preflight integer Limber configuration is eligible");
                run_integer_limber_trial(
                    &context,
                    params,
                    &capture,
                    &corpus,
                    exponent,
                    Trial::Preflight,
                    None,
                )
            }
        };
        println!(
            "PREFLIGHT_OK backend={} online_ms={:.6} witness_to_proof_ms={:.6} proof_bytes={}",
            backend.name(),
            metrics.total_prover_ms,
            metrics.witness_to_proof_ms,
            metrics.proof_bytes
        );
        return;
    }

    let exponents = parse_exponents();
    let requested = parse_backends();
    let reps = env_usize("F2Z_SHA_COMPARE_REPS", DEFAULT_REPS);
    assert!(reps > 0, "measured repetitions must be positive");
    let pilot_reps = env_usize("F2Z_SHA_COMPARE_PILOT_REPS", DEFAULT_PILOT_REPS);
    let root_seed = std::env::var("F2Z_SHA_COMPARE_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ROOT_SEED);
    let campaign_stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let output_dir = std::env::var_os("F2Z_SHA_COMPARE_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new("benchmark-results").join(format!("native-sha256-{campaign_stamp}"))
        });
    let trace_path = std::env::var_os("F2Z_SHA_COMPARE_TRACE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| output_dir.join("trace.jsonl"));
    BenchmarkOutput::new(&output_dir)
        .create_dir_all()
        .expect("create artifact directory");
    let mut trace = TraceWriter::new(trace_path, threads);
    whir_tuning::save(
        &output_dir.join("environment.json"),
        &common::environment::metadata(threads),
    )
    .expect("save environment");

    println!("Native SHA-256 compression comparison across five hash-based prover configurations");
    println!("relation: forall i<N: Hhat_i = Compress_SHA256(IV,M_i)");
    println!(
        "threads={threads}; samples={reps}; warmups=1; trace={}",
        trace.path.display()
    );
    println!("build requirement: -C target-cpu=native, fat LTO, codegen-units=1");

    if env_bool("F2Z_SHA_COMPARE_SELF_TESTS", true) {
        security_api_self_test();
        if requested.contains(&Backend::F2z) {
            f2z_public_tamper_self_test();
        }
        if requested.contains(&Backend::Binius) {
            binius_tamper_self_test(&capture);
        }
        if requested.contains(&Backend::Plonky3Whir) {
            plonky3_backend::tamper_self_test();
        }
        if requested.contains(&Backend::Limber) {
            integer_limber_backend::constraint_self_test();
        }
    }

    let pilot_enabled = env_bool("F2Z_SHA_COMPARE_PILOT", true);
    let pilot_exponent = env_usize("F2Z_SHA_COMPARE_PILOT_EXPONENT", exponents[0].max(7));
    assert!((7..=16).contains(&pilot_exponent));
    let pilot = pilot_enabled.then(|| {
        Corpus::new(
            1usize << pilot_exponent,
            shape_seed(root_seed, pilot_exponent),
        )
    });

    let f2z_prefix = if let Ok(prefix) = std::env::var("F2Z_SHA_COMPARE_F2Z_PREFIX") {
        let prefix = prefix.parse::<usize>().expect("F2Z prefix is an integer");
        assert!(prefix <= 4);
        prefix
    } else if requested.contains(&Backend::F2z)
        && let Some(pilot) = &pilot
    {
        choose_f2z_prefix(pilot, pilot_exponent, pilot_reps)
    } else {
        SHA256_DEFAULT_INNER_PREFIX_VARS
    };

    let binius_rate = if let Ok(rate) = std::env::var("F2Z_SHA_COMPARE_LOG_INV_RATE") {
        let rate = rate
            .parse::<usize>()
            .expect("log inverse rate is an integer");
        assert!((1..=3).contains(&rate));
        rate
    } else if requested.contains(&Backend::Binius)
        && let Some(pilot) = &pilot
    {
        choose_binius_rate(&capture, pilot, pilot_exponent, pilot_reps)
    } else {
        1
    };

    // WHIR is selected separately at each measured size inside run_campaign.
    let plonky3 = env_plonky3_params();
    let preliminary_limber = limber_params;
    let mut runnable = requested.clone();
    if runnable.contains(&Backend::Limber) && env_bool("F2Z_SHA_COMPARE_HEAVY_PREFLIGHT", true) {
        let preliminary = SelectedConfigs {
            f2z_prefix,
            binius_rate,
            plonky3,
            limber: preliminary_limber,
        };
        if !preflight_backend(
            Backend::Limber,
            *exponents.first().expect("at least one exponent"),
            &preliminary,
            root_seed,
        ) {
            runnable.remove(&Backend::Limber);
        }
    }
    let limber = if pilot_enabled && runnable.contains(&Backend::Limber) && !has_limber_override() {
        choose_integer_limber_params(
            &capture,
            pilot.as_ref().unwrap(),
            pilot_exponent,
            pilot_reps,
        )
    } else {
        preliminary_limber
    };
    let selected = SelectedConfigs {
        f2z_prefix,
        binius_rate,
        plonky3,
        limber,
    };
    println!(
        "configs: F2Z prefix={}; Binius rate={} ({} FRI queries); WHIR tunes at each size (explicit default {}); Limber {}",
        selected.f2z_prefix,
        selected.binius_rate,
        calculate_n_test_queries(BINIUS_SECURITY_BITS, selected.binius_rate),
        selected.plonky3.label(),
        selected.limber.label(),
    );

    let allowed_at_16 = if exponents.contains(&16) && env_bool("F2Z_SHA_COMPARE_PREFLIGHT", true) {
        runnable
            .iter()
            .copied()
            .filter(|backend| {
                *backend == Backend::Plonky3Whir
                    || preflight_backend(*backend, 16, &selected, root_seed)
            })
            .collect::<HashSet<_>>()
    } else {
        runnable.clone()
    };
    let aggregates = run_campaign(
        &capture,
        &mut trace,
        &exponents,
        reps,
        root_seed,
        &selected,
        &runnable,
        &allowed_at_16,
        &output_dir,
    );
    print_tables(&aggregates);
    write_aggregate_artifacts(&aggregates, &requested, &runnable, &output_dir);
    println!("canonical trace: {}", trace.path.display());
}

#[cfg(test)]
mod native_whir_tests {
    #[test]
    fn binius_sha_binds_public_blocks_outputs_and_proof() {
        super::binius_tamper_self_test(&super::CaptureLayer::install());
    }

    #[test]
    fn comparison_has_the_five_requested_backends() {
        assert_eq!(
            super::ALL_BACKENDS.map(super::Backend::slug),
            [
                "f2z",
                "plonky3-whir",
                "binius64",
                "binius64-ligerito",
                "limber"
            ]
        );
        assert!(super::Backend::parse("spartan-hyrax").is_none());
    }

    #[test]
    fn default_sha_sizes_have_eligible_security_schedules() {
        super::plonky3_backend::security_schedule_self_test();
    }

    #[test]
    fn sha_proof_binds_public_blocks_outputs_and_order() {
        super::plonky3_backend::tamper_self_test();
    }
}

#[cfg(test)]
mod ligerito_isolation_tests {
    #[test]
    fn limber_configuration_probe() {
        if std::env::var_os("F2Z_TEST_CONFIGURATION_PROBE").is_none() {
            return;
        }
        println!(
            "CONFIG_PROBE {}",
            super::integer_limber_backend::security_metadata()
        );
    }
    #[test]
    fn ligerito_selector_leaves_limbers_native_security_unchanged() {
        let probe = |profile| {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "benchmark::ligerito_isolation_tests::limber_configuration_probe",
                    "--nocapture",
                ])
                .env("F2Z_TEST_CONFIGURATION_PROBE", "1")
                .env("F2Z_LIG_PROFILE", profile)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).unwrap();
            serde_json::from_str::<serde_json::Value>(
                stdout
                    .lines()
                    .find_map(|l| l.strip_prefix("CONFIG_PROBE "))
                    .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(probe("custom:1:4"), probe("udrg:1:4"));
    }
}

#[cfg(test)]
mod reporting_tests {
    use super::*;

    #[test]
    fn binius_ligerito_metrics_union_real_opening_intervals() {
        let spans = binius_ligerito_semantic_spans(&trace_capture::phase_tests::trial_fixture());
        let metrics = TrialMetrics::from_spans(&spans, 123);
        assert_eq!(metrics.witness_ms, 8.0 / 1e6);
        assert_eq!(metrics.commit_ms, 10.0 / 1e6);
        assert_eq!(metrics.piop_ms, 55.0 / 1e6);
        assert_eq!(metrics.opening_ms, 50.0 / 1e6);
        assert_eq!(metrics.total_prover_ms, 130.0 / 1e6);
        assert_eq!(metrics.witness_to_proof_ms, 139.0 / 1e6);
        assert_eq!(metrics.verifier_ms, 10.0 / 1e6);
        let openings: Vec<_> = spans
            .iter()
            .filter(|s| s.scope_tag == Some("opening-proof"))
            .map(|s| (s.start_ns, s.end_ns))
            .collect();
        assert_eq!(openings, [(30, 40), (65, 75), (70, 80), (105, 130)]);
        assert_eq!(
            spans.iter().map(|s| &s.id).collect::<HashSet<_>>().len(),
            spans.len()
        );
        for s in &spans {
            if let Some(parent) = &s.parent {
                let parent = spans.iter().find(|p| &p.id == parent).unwrap();
                assert!(parent.start_ns <= s.start_ns && s.end_ns <= parent.end_ns);
            }
        }
    }

    #[test]
    fn span_metrics_cover_repeated_verified_sha_trials() {
        use tracing_subscriber::prelude::*;
        // Thirty-two compressions reach the opener's minimum packed log of 13.
        let context = BiniusLigeritoContext::setup(&Corpus::new(32, DEFAULT_ROOT_SEED));
        let layer = CaptureLayer::default();
        let capture = layer.capture();
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), || {
            for _ in 0..6 {
                let (metrics, spans, _) = context.run(&capture);
                assert!(
                    metrics.commit_ms > 0.0 && metrics.piop_ms > 0.0 && metrics.opening_ms > 0.0
                );
                assert!(metrics.witness_to_proof_ms >= metrics.total_prover_ms);
                assert_eq!(
                    spans
                        .iter()
                        .filter(|s| s.scope_tag == Some("opening-proof"))
                        .count(),
                    context.prepared.oracle_specs().len() + 1
                );
                for s in &spans {
                    if let Some(parent) = &s.parent {
                        let parent = spans.iter().find(|p| &p.id == parent).unwrap();
                        assert!(parent.start_ns <= s.start_ns && s.end_ns <= parent.end_ns);
                    }
                }
            }
        });
    }

    #[test]
    fn csv_contract_keeps_nine_decimals_and_escapes_configuration() {
        let mut csv = common::output::csv_writer(Vec::new());
        csv.write_record(CsvRow::HEADER).unwrap();
        csv.flush().unwrap();
        let header = "backend,compression_exponent,compressions,configuration,sample_index,witness_ms,commit_ms,piop_ms,iop_ms,online_prover_ms,witness_to_proof_ms,throughput_compressions_per_s,verifier_ms,proof_bytes,setup_ms\n";
        assert_eq!(csv.get_ref(), header.as_bytes());
        csv.serialize(CsvRow {
            backend: "f2z",
            compression_exponent: 3,
            compressions: 8,
            configuration: "comma,quote\"\nline",
            sample_index: 0,
            witness_ms: 1.2345678916,
            commit_ms: 2.0,
            piop_ms: 3.0,
            iop_ms: 4.0,
            online_prover_ms: 9.0,
            witness_to_proof_ms: 10.0,
            throughput_compressions_per_s: 800.0,
            verifier_ms: 5.0,
            proof_bytes: 1024,
            setup_ms: 6.0,
        })
        .unwrap();
        assert_eq!(
            String::from_utf8(csv.into_inner().unwrap()).unwrap(),
            format!(
                "{header}f2z,3,8,\"comma,quote\"\"\nline\",0,1.234567892,2.000000000,3.000000000,4.000000000,9.000000000,10.000000000,800.000000000,5.000000000,1024,6\n"
            )
        );
    }
    #[test]
    fn summary_keeps_metric_names_nulls_and_integer_bytes() {
        let aggregate = BackendAggregate {
            backend: Backend::F2z,
            exponent: 3,
            setup_ms: 1.0,
            config: "config".into(),
            whir_tuning: None,
            samples: vec![TrialMetrics {
                witness_ms: 2.0,
                commit_ms: 3.0,
                piop_ms: 4.0,
                opening_ms: 5.0,
                total_prover_ms: 12.0,
                witness_to_proof_ms: 16.0,
                verifier_ms: 6.0,
                serialization_ms: None,
                proof_bytes: 123,
            }],
        };
        let row = serde_json::to_value(aggregate.summary_row()).unwrap();
        assert_eq!(
            row["median"],
            json!({
                "witness_ms":2.0,"commit_ms":3.0,"piop_ms":4.0,"iop_ms":5.0,
                "online_prover_ms":12.0,"witness_to_proof_ms":16.0,"verifier_ms":6.0,
                "serialization_ms":null,"proof_bytes":123,"throughput_compressions_per_s":500.0
            })
        );
        assert!(row["median"]["proof_bytes"].is_u64());
        assert!(row.get("limber_security").unwrap().is_null());
        assert!(row.get("whir_tuning").unwrap().is_null());
        assert_eq!(row["measured_samples"], 1);
    }
}
