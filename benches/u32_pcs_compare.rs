//! PCS-only comparison for the exact u32 multiplication assignment.
//!
//! Every backend receives the same deterministic integer assignment
//! `f = [e0 | X | Y | Product]`. F2Z commits its exact 32/32/64 bitification,
//! WHIR commits the three exact Goldilocks columns, and Binius64 commits one
//! exact 128-bit packed row per gate before ring switching to BaseFold.

mod common;
use common::mul_witness::u32_digest as witness_digest;
mod integer_pcs_compare {
    pub mod binius;
    pub mod ligerito;
    pub mod whir_goldilocks;
}

use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crypto_primitives::FromWithConfig;
use f2z::ext_proj::ProjArith;
use f2z::pcs::{FQ_MOD, ProjectCanonicalU128};
use f2z::piop::spartan::f2z::{
    PreparedU32TerminalF2zOpening, commit_u32_terminal_f2z_witness,
    prepare_u32_terminal_f2z_opening, prove_u32_terminal_claim_f2z,
    u32_terminal_claim_f2z_proof_bytes, verify_u32_terminal_claim_f2z,
};
use f2z::piop::spartan::{
    PreparedU32MulRelation, ScaledMleEvaluationClaim, SpartanF2zField, U32MulF2zWidth,
    U32MulWitness, commit_u32_mul_witness, spartan_f2z_field_config,
};
use f2z::transcript::Blake3Transcript;
use f2z::transcript::traits::Transcript;
use f2z::utils::prof::ProfileInterval;
use integer_pcs_compare::binius::{self, BiniusBackend};
use integer_pcs_compare::ligerito::{self, LigeritoBackend};
use integer_pcs_compare::whir_goldilocks::{self as whir, Backend as WhirBackend};
use p3_whir::parameters::WhirConfigError;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};

const DEFAULT_SEED: u64 = common::mul_witness::U32_SEED;
const DEFAULT_REPS: usize = 21;
const MIN_EXPONENT: usize = 15;
const MAX_EXPONENT: usize = 25;

const ROOT_SCOPE: &str = "pcs-compare:verified_trial";
const MATERIALIZE_SCOPE: &str = "pcs-compare:materialize";
const COMMIT_SCOPE: &str = "pcs-compare:commit";
const CLAIM_SCOPE: &str = "pcs-compare:claim_setup";
const OPENING_SCOPE: &str = "pcs-compare:opening";
const VERIFY_SCOPE: &str = "pcs-compare:verification";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    F2z,
    Whir,
    Binius,
    /// The F2Z opener on the Binius64 packed rows and the identical bit-MLE
    /// claim: rate 1/8, Johnson-regime Ligerito, grinding, Round 0.
    Ligerito,
}

impl Backend {
    const fn id(self) -> &'static str {
        match self {
            Self::F2z => "f2z",
            Self::Whir => "plonky3-whir",
            Self::Binius => "binius64-basefold",
            Self::Ligerito => "f2z-ligerito-binary",
        }
    }
    const fn display(self) -> &'static str {
        match self {
            Self::F2z => "F2Z",
            Self::Whir => "Plonky3 WHIR",
            Self::Binius => "Binius64 BaseFold",
            Self::Ligerito => "F2Z Ligerito (binary claim)",
        }
    }
    const fn seed_tag(self) -> u64 {
        match self {
            Self::F2z => 0x4632_5a00_5533_0001,
            Self::Whir => 0x5748_4952_5533_0001,
            Self::Binius => 0x4249_4e49_5533_0001,
            Self::Ligerito => 0x4c49_4745_5533_0001,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Trial {
    Warmup,
    Sample(usize),
}

impl Trial {
    fn fragment(self) -> String {
        match self {
            Self::Warmup => "warmup-0".into(),
            Self::Sample(i) => format!("sample-{i}"),
        }
    }
    fn json(self, seed: u64) -> Value {
        match self {
            Self::Warmup => {
                json!({"kind":"warmup","warmup_index":0,"seed":format!("{seed:#018x}")})
            }
            Self::Sample(i) => {
                json!({"kind":"sample","sample_index":i,"seed":format!("{seed:#018x}")})
            }
        }
    }
    const fn word(self) -> u64 {
        match self {
            Self::Warmup => 0,
            Self::Sample(i) => i as u64 + 1,
        }
    }
}

#[derive(Clone, Copy)]
struct Artifacts {
    commitment: usize,
    claim: usize,
    opening: usize,
}

impl Artifacts {
    fn total(self) -> usize {
        self.commitment + self.claim + self.opening
    }
}

struct TraceWriter {
    out: Box<dyn Write>,
    campaign: String,
    git_rev: String,
    dirty: bool,
    threads: usize,
}

struct Run<'a> {
    backend: Backend,
    exponent: usize,
    trial: Trial,
    trial_seed: u64,
    shape_seed: u64,
    digest: &'a str,
    witness_ms: f64,
    setup_ms: f64,
    security: Value,
    artifacts: Artifacts,
}

impl TraceWriter {
    fn new(threads: usize, campaign: String) -> Result<Self, Box<dyn Error>> {
        let out: Box<dyn Write> = match std::env::var_os("F2Z_PCS_COMPARE_TRACE_PATH") {
            Some(path) => {
                let path = Path::new(&path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                Box::new(BufWriter::new(
                    OpenOptions::new().write(true).create_new(true).open(path)?,
                ))
            }
            None => Box::new(BufWriter::new(io::stdout())),
        };
        Ok(Self {
            out,
            campaign,
            git_rev: command_output("git", &["rev-parse", "HEAD"], "unknown"),
            dirty: Command::new("git")
                .args(["status", "--porcelain", "--untracked-files=no"])
                .output()
                .map_or(true, |output| {
                    !output.status.success() || !output.stdout.is_empty()
                }),
            threads,
        })
    }

    fn write(&mut self, run: Run<'_>, intervals: &[ProfileInterval]) -> Result<(), Box<dyn Error>> {
        let root = intervals
            .iter()
            .find(|interval| interval.parent_order.is_none())
            .ok_or("trace has no root interval")?;
        if root.label != ROOT_SCOPE
            || intervals
                .iter()
                .filter(|interval| interval.parent_order.is_none())
                .count()
                != 1
        {
            return Err("trace must contain exactly one verified-trial root".into());
        }
        let run_id = format!(
            "u32-pcs-{}-{}-2p{}-{}",
            self.campaign,
            run.backend.id(),
            run.exponent,
            run.trial.fragment()
        );
        let series_id = format!(
            "u32-pcs-{}-{}-2p{}-{}-{}t-bench",
            self.campaign,
            run.backend.id(),
            run.exponent,
            self.git_rev,
            self.threads
        );
        let field = match run.backend {
            Backend::F2z => json!({
                "committed_encoding":"32 little-endian X bits | 32 Y bits | 64 product bits",
                "evaluation_field":"F_q", "q":FQ_MOD.to_string(),
                "commitment_field":"GF(2^128)"
            }),
            Backend::Whir => json!({
                "committed_encoding":"three native Goldilocks columns X/Y/Product",
                "base_field":"Goldilocks", "base_modulus":18_446_744_069_414_584_321_u64,
                "challenge_extension_degree":whir::CHALLENGE_EXTENSION_DEGREE
            }),
            Backend::Binius | Backend::Ligerito => json!({
                "committed_encoding":"one GF(2^128) row per gate: X[0..32] | Y[32..64] | Product[64..128]",
                "base_field":"GF(2)", "challenge_field":"GF(2^128)-GHASH"
            }),
        };
        let record = json!({
            "schema":"zkperf.trace/v1", "record":"run", "run_id":run_id,
            "series_id":series_id, "root_span_id":span_id(root.order),
            "benchmark":{
                "suite":"f2z-pcs", "name":"u32-pcs-compare",
                "label":format!("{} at 2^{} u32 multiplications",run.backend.display(),run.exponent),
                "algorithm":format!("shared integer u32 assignment / {} opening",run.backend.display()),
                "implementation":run.backend.id(), "git_rev":self.git_rev,
                "git_dirty":self.dirty, "build_profile":"bench"
            },
            "trial":run.trial.json(run.trial_seed),
            "clock":{"id":format!("mono-process-{}-{run_id}",std::process::id()),"kind":"monotonic","unit":"ns","source":"std::time::Instant"},
            "status":"ok", "trace_complete":true,
            "environment":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"cpu":"Apple Silicon","threads":self.threads,"thread_policy":"Rayon pool; no affinity pinning"},
            "parameters":{
                "input":{"log_multiplications":run.exponent,"multiplications":1usize<<run.exponent,"capacity":1usize<<run.exponent,"gate_variables":run.exponent,"assignment_variables":run.exponent+2,"logical_assignment":"[e0 | X | Y | Product]","witness_digest_blake3":run.digest,"witness_generation_ms_excluded":run.witness_ms,"setup_ms_excluded":run.setup_ms},
                "statement":{"terminal_claim":"D * f(x, beta) = V","point_policy":"transcript-derived after commitment","same_integer_witness":true,"independent_native_field_challenges":true},
                "field":field, "security":run.security,
                "recursion":{"max_depth":0,"instance_count":1}, "repetition":{"count":1},
                "seed":format!("{:#018x}",run.shape_seed)
            },
            "artifacts":{"commitment_bytes":run.artifacts.commitment,"public_claim_bytes":run.artifacts.claim,"opening_proof_bytes":run.artifacts.opening,"total_wire_bytes":run.artifacts.total()},
            "tags":{"campaign_id":self.campaign,"root_boundary":"materialization through verified terminal opening; setup and logical witness generation excluded","timeline":"observed half-open intervals"}
        });
        serde_json::to_writer(&mut self.out, &record)?;
        writeln!(self.out)?;

        let parents = intervals
            .iter()
            .map(|interval| (interval.order, interval))
            .collect::<HashMap<_, _>>();
        for interval in intervals {
            let labels = ancestry(interval, &parents);
            let primary = match interval.label {
                ROOT_SCOPE => "end-to-end",
                MATERIALIZE_SCOPE | CLAIM_SCOPE => "preparation",
                COMMIT_SCOPE => "commit",
                OPENING_SCOPE => "opening-proof",
                VERIFY_SCOPE => "verification",
                _ if labels.contains(&VERIFY_SCOPE) => "verification",
                _ if labels.contains(&OPENING_SCOPE) => "opening-proof",
                _ => "proving",
            };
            let mut tags = vec![primary];
            if interval.label != ROOT_SCOPE {
                let boundary_tag = if labels.contains(&VERIFY_SCOPE) {
                    "verification"
                } else {
                    "proving"
                };
                if !tags.contains(&boundary_tag) {
                    tags.push(boundary_tag);
                }
            }
            if labels.contains(&COMMIT_SCOPE) || labels.contains(&OPENING_SCOPE) {
                tags.push("pcs");
            }
            let math = math_for(interval.label, run.backend);
            let mut attributes = json!({
                "scope_kind":if interval.parent_order.is_none(){"scope"}else if is_primary(interval.label){"phase"}else{"procedure"},
                "short_name":short_name(interval.label),
                "primary_sequence":is_primary(interval.label),
                "source_label":interval.label,
            });
            if !math.is_empty() {
                attributes["math_latex"] = json!(math);
            }
            let span = json!({
                "schema":"zkperf.trace/v1","record":"span","run_id":run_id,
                "span_id":span_id(interval.order),"parent_span_id":interval.parent_order.map(span_id),
                "operation":operation(interval.label),"name":name(interval.label),
                "primary_phase":primary,"phase_tags":tags,
                "start_ns":interval.start_ns.to_string(),"end_ns":interval.end_ns.to_string(),
                "duration_ns":interval.end_ns.saturating_sub(interval.start_ns).to_string(),
                "lane":{"process":"benchmark","thread":"control"},"coordinate":{},
                "attributes":attributes
            });
            serde_json::to_writer(&mut self.out, &span)?;
            writeln!(self.out)?;
        }
        self.out.flush()?;
        common::pcs_console::print_trial(
            &run.trial.fragment(),
            intervals,
            run.artifacts.opening,
            run.artifacts.total(),
        );
        Ok(())
    }
}

fn ancestry<'a>(
    interval: &'a ProfileInterval,
    parents: &HashMap<u64, &'a ProfileInterval>,
) -> Vec<&'static str> {
    let mut labels = Vec::new();
    let mut cursor = Some(interval);
    while let Some(current) = cursor {
        labels.push(current.label);
        cursor = current
            .parent_order
            .and_then(|id| parents.get(&id).copied());
    }
    labels
}

fn is_primary(label: &str) -> bool {
    matches!(
        label,
        MATERIALIZE_SCOPE | COMMIT_SCOPE | CLAIM_SCOPE | OPENING_SCOPE | VERIFY_SCOPE
    )
}

fn operation(label: &str) -> String {
    match label {
        ROOT_SCOPE => "pcs_compare.verified_trial".into(),
        MATERIALIZE_SCOPE => "pcs_compare.materialize".into(),
        COMMIT_SCOPE => "pcs_compare.commit".into(),
        CLAIM_SCOPE => "pcs_compare.claim_setup".into(),
        OPENING_SCOPE => "pcs_compare.opening".into(),
        VERIFY_SCOPE => "pcs_compare.verify".into(),
        binius::COMMIT_ORACLE_SCOPE => "binius.basefold.commit_oracle".into(),
        binius::SAMPLE_POINT_SCOPE => "binius.claim.sample_point".into(),
        binius::EVALUATE_CLAIM_SCOPE => "binius.claim.evaluate_bit_mle".into(),
        binius::RING_SWITCH_SCOPE => "binius.opening.ring_switch".into(),
        binius::BASEFOLD_OPEN_SCOPE => "binius.opening.basefold".into(),
        binius::RING_SWITCH_VERIFY_SCOPE => "binius.verify.ring_switch".into(),
        binius::BASEFOLD_VERIFY_SCOPE => "binius.verify.basefold".into(),
        _ => format!(
            "pcs_compare.detail.{}",
            label
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                })
                .collect::<String>()
        ),
    }
}

fn name(label: &str) -> String {
    match label {
        ROOT_SCOPE => "Complete verified terminal opening".into(),
        MATERIALIZE_SCOPE => "Materialize backend-native commitment witness".into(),
        COMMIT_SCOPE => "Commit to the backend-native witness encoding".into(),
        CLAIM_SCOPE => "Derive and bind the terminal MLE claim".into(),
        OPENING_SCOPE => "Prove the prescribed terminal opening".into(),
        VERIFY_SCOPE => "Verify the prescribed terminal opening".into(),
        binius::COMMIT_ORACLE_SCOPE => "Encode and Merkle-commit the BaseFold oracle".into(),
        binius::SAMPLE_POINT_SCOPE => "Sample the binary MLE opening point".into(),
        binius::EVALUATE_CLAIM_SCOPE => "Evaluate the packed witness as a bit-MLE".into(),
        binius::RING_SWITCH_SCOPE => "Reduce the bit-MLE claim by ring switching".into(),
        binius::BASEFOLD_OPEN_SCOPE => "Prove the reduced relation with BaseFold".into(),
        binius::RING_SWITCH_VERIFY_SCOPE => "Verify the ring-switch reduction".into(),
        binius::BASEFOLD_VERIFY_SCOPE => "Verify the BaseFold opening".into(),
        _ => label.replace([':', '_'], " "),
    }
}

fn short_name(label: &str) -> String {
    match label {
        ROOT_SCOPE => "Verified trial".into(),
        MATERIALIZE_SCOPE => "Materialize".into(),
        COMMIT_SCOPE => "Commit".into(),
        CLAIM_SCOPE => "Claim setup".into(),
        OPENING_SCOPE => "Opening".into(),
        VERIFY_SCOPE => "Verify".into(),
        binius::COMMIT_ORACLE_SCOPE => "BaseFold commit".into(),
        binius::SAMPLE_POINT_SCOPE => "Sample point".into(),
        binius::EVALUATE_CLAIM_SCOPE => "Evaluate bit-MLE".into(),
        binius::RING_SWITCH_SCOPE => "Ring switch".into(),
        binius::BASEFOLD_OPEN_SCOPE => "BaseFold opening".into(),
        binius::RING_SWITCH_VERIFY_SCOPE => "Verify ring switch".into(),
        binius::BASEFOLD_VERIFY_SCOPE => "Verify BaseFold".into(),
        _ => label.replace([':', '_'], " "),
    }
}

fn math_for(label: &str, backend: Backend) -> Vec<&'static str> {
    match (label, backend) {
        (MATERIALIZE_SCOPE, Backend::F2z) => {
            vec!["\\mathcal B=\\operatorname{Bit}_{32,32,64}(X,Y,P)"]
        }
        (MATERIALIZE_SCOPE, Backend::Whir) => {
            vec!["U=(X,Y,P)\\in\\mathbb F_{\\mathrm{Goldilocks}}^{3\\times2^g}"]
        }
        (MATERIALIZE_SCOPE, Backend::Binius | Backend::Ligerito) => {
            vec!["w_i=X_i+2^{32}Y_i+2^{64}P_i\\in\\mathbb F_2^{128}"]
        }
        (COMMIT_SCOPE, Backend::Ligerito) => {
            vec!["C\\leftarrow\\operatorname{Merkle}(\\operatorname{RS}_{1/8}(w)),\\;y=\\widetilde w(\\zeta,\\zeta^2,\\ldots)"]
        }
        (OPENING_SCOPE, Backend::Ligerito) => {
            vec!["\\pi\\leftarrow\\operatorname{RingSwitch+Ligerito}_{\\mathrm{Johnson}}(C,r,v)"]
        }
        (COMMIT_SCOPE, Backend::F2z) => {
            vec!["C\\leftarrow\\operatorname{Commit}_{\\mathrm{F2Z}}(\\mathcal B)"]
        }
        (COMMIT_SCOPE, Backend::Whir) => {
            vec!["C\\leftarrow\\operatorname{Commit}_{\\mathrm{WHIR}}(U)"]
        }
        (COMMIT_SCOPE, Backend::Binius) => {
            vec!["C\\leftarrow\\operatorname{Commit}_{\\mathrm{BaseFold}}(w)"]
        }
        (CLAIM_SCOPE, _) => vec!["D\\,\\widetilde f(x,\\beta)=V"],
        (OPENING_SCOPE, Backend::Binius) => {
            vec!["\\pi\\leftarrow\\operatorname{RingSwitch+BaseFold}(C,r,v)"]
        }
        (OPENING_SCOPE, Backend::Whir) => {
            vec!["\\pi\\leftarrow\\operatorname{Open}_{\\mathrm{WHIR}}(C,x)"]
        }
        (OPENING_SCOPE, Backend::F2z) => {
            vec!["\\pi\\leftarrow\\operatorname{Open}_{\\mathrm{F2Z}}(C,x,\\beta,D,V)"]
        }
        (VERIFY_SCOPE | ROOT_SCOPE, _) => vec!["\\operatorname{Verify}(C,x,\\beta,D,V,\\pi)=1"],
        (binius::COMMIT_ORACLE_SCOPE, Backend::Binius) => {
            vec!["C_{\\mathrm{BF}}\\leftarrow\\operatorname{Merkle}(\\operatorname{NTT}(w))"]
        }
        (binius::SAMPLE_POINT_SCOPE, Backend::Binius) => {
            vec!["r\\leftarrow\\mathcal T\\in\\mathbb F_{2^{128}}^{g+7}"]
        }
        (binius::EVALUATE_CLAIM_SCOPE, Backend::Binius) => {
            vec!["v=\\widetilde{\\operatorname{bits}(w)}(r_{\\mathrm{bit}},r_{\\mathrm{gate}})"]
        }
        (binius::RING_SWITCH_SCOPE, Backend::Binius) => {
            vec![
                "\\widetilde{\\operatorname{bits}(w)}(r_{\\mathrm{bit}},r_{\\mathrm{gate}})=v\\;\\Longrightarrow\\;\\langle w,\\rho_r\\rangle=\\widehat v",
            ]
        }
        (binius::BASEFOLD_OPEN_SCOPE, Backend::Binius) => {
            vec![
                "\\pi_{\\mathrm{BF}}\\leftarrow\\operatorname{BaseFold.Open}(C_{\\mathrm{BF}},\\rho_r,\\widehat v)",
            ]
        }
        (binius::RING_SWITCH_VERIFY_SCOPE, Backend::Binius) => {
            vec!["\\operatorname{VerifyRS}(r,v,\\widehat v)=1"]
        }
        (binius::BASEFOLD_VERIFY_SCOPE, Backend::Binius) => {
            vec![
                "\\operatorname{BaseFold.Verify}(C_{\\mathrm{BF}},\\rho_r,\\widehat v,\\pi_{\\mathrm{BF}})=1",
            ]
        }
        _ => Vec::new(),
    }
}

fn exponents() -> Vec<usize> {
    common::shapes(None).map_or_else(
        || (MIN_EXPONENT..=MAX_EXPONENT).collect(),
        |shapes| {
            shapes
                .iter()
                .map(|value| value.parse::<usize>().expect("integer exponent"))
                .inspect(|value| assert!((MIN_EXPONENT..=MAX_EXPONENT).contains(value)))
                .collect()
        },
    )
}

fn selected_backends() -> Vec<Backend> {
    let raw = std::env::var("F2Z_PCS_COMPARE_BACKENDS")
        .unwrap_or_else(|_| "f2z plonky3-whir binius64-basefold f2z-ligerito-binary".into());
    let mut selected = Vec::new();
    for word in raw.split([',', ' ']).filter(|word| !word.is_empty()) {
        let backend = match word {
            "f2z" => Backend::F2z,
            "whir" | "plonky3-whir" => Backend::Whir,
            "binius" | "binius64" | "binius64-basefold" => Backend::Binius,
            "ligerito" | "f2z-ligerito" | "f2z-ligerito-binary" => Backend::Ligerito,
            _ => panic!("unknown backend {word}"),
        };
        if !selected.contains(&backend) {
            selected.push(backend);
        }
    }
    selected
}

fn ordered(selected: &[Backend], exponent: usize) -> Vec<Backend> {
    let order = if exponent.is_multiple_of(2) {
        [Backend::F2z, Backend::Whir, Backend::Binius, Backend::Ligerito]
    } else {
        [Backend::Ligerito, Backend::Binius, Backend::Whir, Backend::F2z]
    };
    order.into_iter().filter(|b| selected.contains(b)).collect()
}

fn mix_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn trial_seed(shape_seed: u64, backend: Backend, trial: Trial) -> u64 {
    mix_seed(shape_seed ^ backend.seed_tag() ^ trial.word().rotate_left(17))
}

fn equality_table(point: &[u128], arith: &ProjArith) -> Vec<u128> {
    let mut table = vec![0; 1usize << point.len()];
    table[0] = 1;
    let mut half = 1;
    for &coordinate in point {
        for index in 0..half {
            let parent = table[index];
            let one = arith.mul(parent, coordinate);
            table[index] = if one == 0 {
                parent
            } else {
                arith.add(parent, arith.q() - one)
            };
            table[index + half] = one;
        }
        half *= 2;
    }
    table
}

fn seed_f2z_transcript(commitment: &[u8], seed: u64) -> Blake3Transcript {
    let mut transcript = Blake3Transcript::new();
    transcript.absorb_slice(b"f2z/u32-pcs-compare/terminal-claim/v1");
    transcript.absorb_slice(&seed.to_le_bytes());
    transcript.absorb_slice(commitment);
    transcript
}

fn derive_f2z_claim(
    witness: &U32MulWitness,
    mut prover: Blake3Transcript,
    mut verifier: Blake3Transcript,
) -> (
    ScaledMleEvaluationClaim<SpartanF2zField>,
    Blake3Transcript,
    Blake3Transcript,
) {
    let config = spartan_f2z_field_config();
    let gate_vars = witness.layout().gate_vars();
    let point = prover.get_field_challenges::<SpartanF2zField>(gate_vars + 2, &config);
    let scale = prover.get_field_challenge::<SpartanF2zField>(&config);
    let verifier_point = verifier.get_field_challenges::<SpartanF2zField>(gate_vars + 2, &config);
    let verifier_scale = verifier.get_field_challenge::<SpartanF2zField>(&config);
    assert_eq!(verifier_point, point);
    assert_eq!(verifier_scale, scale);
    let canonical = point
        .iter()
        .map(ProjectCanonicalU128::canonical_u128)
        .collect::<Vec<_>>();
    let eq = equality_table(&canonical[..gate_vars], &ProjArith::new(FQ_MOD));
    let b0 = canonical[gate_vars];
    let b1 = canonical[gate_vars + 1];
    let arith = ProjArith::new(FQ_MOD);
    let sub_one = |v: u128| if v == 0 { 1 } else { arith.add(1, FQ_MOD - v) };
    let chi = [
        arith.mul(sub_one(b0), sub_one(b1)),
        arith.mul(b0, sub_one(b1)),
        arith.mul(sub_one(b0), b1),
        arith.mul(b0, b1),
    ];
    let mut value = arith.mul(chi[0], eq[0]);
    for gate in 0..witness.layout().capacity() {
        let selected = arith.add(
            arith.add(
                arith.mul(chi[1], u128::from(witness.x_values()[gate])),
                arith.mul(chi[2], u128::from(witness.y_values()[gate])),
            ),
            arith.mul(chi[3], u128::from(witness.product_values()[gate])),
        );
        value = arith.add(value, arith.mul(eq[gate], selected));
    }
    value = arith.mul(scale.canonical_u128(), value);
    (
        ScaledMleEvaluationClaim::new(
            point.into_boxed_slice(),
            scale,
            SpartanF2zField::from_with_cfg(value, &config),
        ),
        prover,
        verifier,
    )
}

fn clear_profile() {
    let _ = f2z::utils::prof::take_intervals();
    let _ = f2z::utils::prof::take_totals();
}

fn finish_profile() -> Vec<ProfileInterval> {
    let intervals = f2z::utils::prof::take_intervals();
    let _ = f2z::utils::prof::take_totals();
    intervals
}

#[allow(clippy::too_many_arguments)]
fn run_f2z(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &U32MulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<(), Box<dyn Error>> {
    let setup = Instant::now();
    let relation = PreparedU32MulRelation::new_with_profile_and_ligerito::<f2z::piop::spartan::Lambda100>(*witness.layout(), common::ligerito_selection(100))?;
    let preflight = commit_u32_mul_witness(&relation, witness.f2z_bit_rows())?;
    let commitment = preflight.commitment.clone();
    drop(preflight);
    flock_core::scratch::clear();
    let prepared: PreparedU32TerminalF2zOpening =
        prepare_u32_terminal_f2z_opening(&relation, &commitment)?;
    let ligerito = common::ligerito_report(relation.ligerito_configuration(), relation.security().ood);
    drop((relation, commitment));
    let setup_ms = common::elapsed_ms(setup);
    eprintln!("    backend_setup_ms={setup_ms:.3}");
    let security = json!({"profile":"F2Z Lambda100 terminal opening","ligerito":ligerito,"target_bits":100,"evaluation_modulus":FQ_MOD.to_string(),"commitment_field":"GF(2^128)","transcript_hash":"BLAKE3"});
    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::F2z, trial);
        clear_profile();
        let root = f2z::utils::prof::scope(ROOT_SCOPE);
        let rows = {
            let _phase = f2z::utils::prof::scope(MATERIALIZE_SCOPE);
            witness.f2z_bit_rows()
        };
        let (hint, encoding, pt, vt) = {
            let _phase = f2z::utils::prof::scope(COMMIT_SCOPE);
            let hint = commit_u32_terminal_f2z_witness(&prepared, rows)?;
            let encoding = bincode::serialize(&hint.commitment)?;
            let pt = seed_f2z_transcript(&encoding, seed);
            let vt = seed_f2z_transcript(&encoding, seed);
            (hint, encoding, pt, vt)
        };
        let (claim, mut pt, mut vt) = {
            let _phase = f2z::utils::prof::scope(CLAIM_SCOPE);
            derive_f2z_claim(witness, pt, vt)
        };
        let proof = {
            let _phase = f2z::utils::prof::scope(OPENING_SCOPE);
            prove_u32_terminal_claim_f2z(&mut pt, &prepared, &hint, &claim)?
        };
        {
            let _phase = f2z::utils::prof::scope(VERIFY_SCOPE);
            verify_u32_terminal_claim_f2z(&mut vt, &prepared, &hint.commitment, &claim, &proof)?;
        }
        drop(root);
        let intervals = finish_profile();
        let proof_bytes = u32_terminal_claim_f2z_proof_bytes(&proof).len();
        writer.write(
            Run {
                backend: Backend::F2z,
                exponent,
                trial,
                trial_seed: seed,
                shape_seed,
                digest,
                witness_ms,
                setup_ms,
                security: security.clone(),
                artifacts: Artifacts {
                    commitment: encoding.len(),
                    claim: (exponent + 4) * 16,
                    opening: proof_bytes,
                },
            },
            &intervals,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_whir(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &U32MulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<(), Box<dyn Error>> {
    let setup = Instant::now();
    let (folding, log_inv_rate, max_pow_bits) = whir_tuning(exponent)?;
    let backend = match WhirBackend::setup_with_params(
        witness.layout().capacity(),
        folding,
        log_inv_rate,
        max_pow_bits,
    ) {
        Ok(value) => value,
        Err(whir::Error::Config(WhirConfigError::PowBitsExceedBudget { required, budget })) => {
            return Err(format!("WHIR needs {required} PoW bits, budget is {budget}").into());
        }
        Err(error) => return Err(error.into()),
    };
    let summary = backend.security_summary();
    let setup_ms = common::elapsed_ms(setup);
    eprintln!("    backend_setup_ms={setup_ms:.3}");
    let security = json!({"profile":format!("WHIR Goldilocks degree {} {}",whir::CHALLENGE_EXTENSION_DEGREE,whir::SECURITY_ASSUMPTION_LABEL),"target_bits":100,"internal_target_bits":summary.target_bits,"challenge_extension_degree":whir::CHALLENGE_EXTENSION_DEGREE,"configured_max_pow_bits":summary.configured_max_pow_bits,"derived_max_pow_bits":summary.derived_max_pow_bits,"folding_factor":summary.folding_factor,"starting_log_inverse_rate":summary.starting_log_inverse_rate,"commitment_ood_samples":summary.commitment_ood_samples,"round_queries":summary.round_queries,"round_pow_bits":summary.round_pow_bits,"final_queries":summary.final_queries,"final_pow_bits":summary.final_pow_bits,"hash":"Poseidon2Goldilocks<8>","hiding":false});
    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Whir, trial);
        clear_profile();
        let root = f2z::utils::prof::scope(ROOT_SCOPE);
        let materialized = {
            let _phase = f2z::utils::prof::scope(MATERIALIZE_SCOPE);
            backend.materialize(
                witness.x_values(),
                witness.y_values(),
                witness.product_values(),
            )?
        };
        let committed = {
            let _phase = f2z::utils::prof::scope(COMMIT_SCOPE);
            backend.commit(materialized, seed)
        };
        let ready = {
            let _phase = f2z::utils::prof::scope(CLAIM_SCOPE);
            backend.derive_and_bind_claim(committed)?
        };
        let opened = {
            let _phase = f2z::utils::prof::scope(OPENING_SCOPE);
            backend.open(ready)
        };
        {
            let _phase = f2z::utils::prof::scope(VERIFY_SCOPE);
            let _ = std::hint::black_box(backend.verify(&opened)?);
        }
        std::hint::black_box(opened.claim());
        drop(root);
        let intervals = finish_profile();
        writer.write(
            Run {
                backend: Backend::Whir,
                exponent,
                trial,
                trial_seed: seed,
                shape_seed,
                digest,
                witness_ms,
                setup_ms,
                security: security.clone(),
                artifacts: Artifacts {
                    commitment: whir::commitment_bytes(opened.commitment())?,
                    claim: (exponent + 4) * 16,
                    opening: whir::proof_bytes(opened.proof())?,
                },
            },
            &intervals,
        )?;
    }
    Ok(())
}

fn tuned_whir_folding(exponent: usize) -> usize {
    (exponent.saturating_mul(3) / 4)
        .saturating_sub(5)
        .clamp(2, 12)
}

fn whir_tuning(exponent: usize) -> Result<(usize, usize, usize), Box<dyn Error>> {
    let folding = match std::env::var("F2Z_WHIR_FOLDING") {
        Ok(value) => value.parse::<usize>()?,
        Err(std::env::VarError::NotPresent) => tuned_whir_folding(exponent),
        Err(error) => return Err(error.into()),
    };
    let log_inv_rate = std::env::var("F2Z_WHIR_LOG_INV_RATE")
        .unwrap_or_else(|_| whir::STARTING_LOG_INV_RATE.to_string())
        .parse::<usize>()?;
    let max_pow_bits = std::env::var("F2Z_WHIR_MAX_POW_BITS")
        .unwrap_or_else(|_| whir::MAX_POW_BITS.to_string())
        .parse::<usize>()?;
    if !(2..=12).contains(&folding) {
        return Err(format!("F2Z_WHIR_FOLDING must be in 2..=12, got {folding}").into());
    }
    if !(1..=8).contains(&log_inv_rate) {
        return Err(format!("F2Z_WHIR_LOG_INV_RATE must be in 1..=8, got {log_inv_rate}").into());
    }
    if max_pow_bits >= whir::SECURITY_BITS {
        return Err(format!(
            "F2Z_WHIR_MAX_POW_BITS must be below {}, got {max_pow_bits}",
            whir::SECURITY_BITS
        )
        .into());
    }
    Ok((folding, log_inv_rate, max_pow_bits))
}

fn pack_binius_rows(witness: &U32MulWitness) -> Vec<u128> {
    witness
        .x_values()
        .iter()
        .zip(witness.y_values())
        .zip(witness.product_values())
        .map(|((x, y), product)| {
            u128::from(*x) | (u128::from(*y) << 32) | (u128::from(*product) << 64)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn run_binius(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &U32MulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<(), Box<dyn Error>> {
    let setup = Instant::now();
    let log_inv_rate = binius_log_inv_rate()?;
    let backend = BiniusBackend::setup(exponent, log_inv_rate);
    let setup_ms = common::elapsed_ms(setup);
    eprintln!("    backend_setup_ms={setup_ms:.3}");
    let security = json!({"profile":"Binius64 ring-switch + BaseFold","target_bits":binius::SECURITY_BITS,"soundness_bound_model":"Diamond-Posen Eq. 42 plus 128-bit SHA-256 cap","estimated_query_soundness_bits":backend.estimated_soundness_bits(),"challenge_field":"GF(2^128)-GHASH","hash":"SHA-256","log_inverse_rate":backend.log_inv_rate(),"test_queries":backend.n_test_queries(),"hiding":false});
    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Binius, trial);
        clear_profile();
        let output = backend.run_trial(|| pack_binius_rows(witness), seed)?;
        let intervals = finish_profile();
        writer.write(
            Run {
                backend: Backend::Binius,
                exponent,
                trial,
                trial_seed: seed,
                shape_seed,
                digest,
                witness_ms,
                setup_ms,
                security: security.clone(),
                artifacts: Artifacts {
                    commitment: output.commitment_bytes,
                    claim: output.public_claim_bytes,
                    opening: output.opening_proof_bytes(),
                },
            },
            &intervals,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_ligerito(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &U32MulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<(), Box<dyn Error>> {
    let setup = Instant::now();
    let backend = LigeritoBackend::setup(exponent)?;
    let setup_ms = common::elapsed_ms(setup);
    eprintln!("    backend_setup_ms={setup_ms:.3}");
    let security = json!({"profile":"F2Z opener: Round 0 + ring switch + Johnson-regime Ligerito","target_bits":ligerito::SECURITY_BITS,"soundness_bound_model":"opener union bound (Round 0, ring switch, every Ligerito level's proximity folds, queries and OOD samples) at the pinned flock constants","achieved_bits":backend.soundness_bits(),"ligerito_component_bits":backend.component_bits(),"challenge_field":"GF(2^128)-GHASH","hash":"BLAKE3","log_inverse_rate":backend.log_inv_rate(),"level0_queries":backend.n_test_queries(),"level0_query_grinding_bits":backend.level0_query_grinding_bits(),"level0_fold_grinding_bits":backend.level0_fold_grinding_bits(),"ood_grinding_bits":backend.ood_grinding_bits(),"hiding":false});
    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Ligerito, trial);
        clear_profile();
        let output = backend.run_trial(|| pack_binius_rows(witness), seed)?;
        let intervals = finish_profile();
        writer.write(
            Run {
                backend: Backend::Ligerito,
                exponent,
                trial,
                trial_seed: seed,
                shape_seed,
                digest,
                witness_ms,
                setup_ms,
                security: security.clone(),
                artifacts: Artifacts {
                    commitment: output.commitment_bytes,
                    claim: output.public_claim_bytes,
                    opening: output.opening_proof_bytes(),
                },
            },
            &intervals,
        )?;
    }
    Ok(())
}

fn binius_log_inv_rate() -> Result<usize, Box<dyn Error>> {
    let value = std::env::var("F2Z_BINIUS_LOG_INV_RATE")
        .unwrap_or_else(|_| binius::DEFAULT_LOG_INV_RATE.to_string())
        .parse::<usize>()?;
    if !(1..=4).contains(&value) {
        return Err(format!("F2Z_BINIUS_LOG_INV_RATE must be in 1..=4, got {value}").into());
    }
    Ok(value)
}

fn campaign_id() -> String {
    std::env::var("F2Z_PCS_COMPARE_CAMPAIGN_ID").unwrap_or_else(|_| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        format!("{nanos}-{}", std::process::id())
    })
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

fn span_id(order: u64) -> String {
    format!("span-{order}")
}

fn main() -> Result<(), Box<dyn Error>> {
    unsafe { std::env::set_var("OBLONG_PROFILE_INTERVALS", "1") };
    let threads = common::init();
    let reps = common::reps(None, DEFAULT_REPS);
    let root_seed = common::seed(None, DEFAULT_SEED);
    let selected = selected_backends();
    let exponents = exponents();
    let mut writer = TraceWriter::new(threads, campaign_id())?;
    eprintln!(
        "u32 PCS comparison: {:?}, {} samples + 1 warmup, {} threads, WHIR degree {}",
        exponents,
        reps,
        threads,
        whir::CHALLENGE_EXTENSION_DEGREE
    );
    common::pcs_console::print_timing_definitions();
    for exponent in exponents {
        flock_core::scratch::clear();
        let shape_seed = common::mul_witness::shape_seed(root_seed, exponent);
        let mut rng = StdRng::seed_from_u64(shape_seed);
        let start = Instant::now();
        let witness =
            U32MulWitness::from_fn_with_f2z_width(1usize << exponent, U32MulF2zWidth::W1, |_| {
                (rng.random::<u32>(), rng.random::<u32>())
            })?;
        let witness_ms = common::elapsed_ms(start);
        let digest = witness_digest(&witness);
        eprintln!(
            "2^{exponent}: shared_witness_generation_ms={witness_ms:.3}, digest {}",
            &digest[..16]
        );
        for backend in ordered(&selected, exponent) {
            eprintln!("  {}", backend.display());
            match backend {
                Backend::F2z => run_f2z(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Whir => run_whir(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Binius => run_binius(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Ligerito => run_ligerito(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
            }
        }
        drop(witness);
        flock_core::scratch::clear();
    }
    Ok(())
}
