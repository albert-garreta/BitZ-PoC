//! Controlled comparison of the F2Z and Plonky3 WHIR PCS opening paths.
//!
//! Each backend receives the same deterministic integer witness
//! `f = [e0 | A | B | C | K | 0 | 0 | 0]`, while deriving challenges in its
//! native field.  The measured boundary starts with backend materialization
//! and ends after verification of the prescribed terminal MLE claim.

mod common;
use common::mul_witness::baby_bear_digest as witness_digest;
use common::output::{BenchmarkOutput, FileMode, JsonStyle, JsonlWriter};
mod baby_bear_pcs_compare {
    pub mod whir;
}
mod integer_pcs_compare {
    pub mod binius;
    pub mod ligerito;
    pub mod whir_goldilocks;
}

use std::collections::HashMap;
use std::error::Error;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use baby_bear_pcs_compare::whir::{self, SecuritySummary, WhirAdapterError, WhirBackend};
use crypto_primitives::FromWithConfig;
use f2z::ext_proj::ProjArith;
use f2z::pcs::{FQ_MOD, ProjectCanonicalU128};
use f2z::piop::spartan::baby_bear_f2z::{
    PreparedBabyBearTerminalF2zOpening, baby_bear_terminal_claim_f2z_proof_bytes,
    commit_baby_bear_terminal_f2z_witness, prepare_baby_bear_terminal_f2z_opening,
    prove_baby_bear_terminal_claim_f2z, verify_baby_bear_terminal_claim_f2z,
};
use f2z::piop::spartan::{
    BabyBearMulWitness, ScaledMleEvaluationClaim, SpartanF2zField, commit_baby_bear_mul_witness,
    prepare_baby_bear_mul_relation, sample_baby_bear_operand_with, spartan_f2z_field_config,
};
use f2z::transcript::Blake3Transcript;
use f2z::transcript::traits::Transcript;
use f2z::utils::prof::ProfileInterval;
use integer_pcs_compare::binius::{self, BiniusBackend};
use integer_pcs_compare::ligerito::{self, LigeritoBackend};
use p3_whir::parameters::WhirConfigError;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};

const DEFAULT_SEED: u64 = common::mul_witness::BABY_BEAR_SEED;
const DEFAULT_REPS: usize = 21;
const MIN_EXPONENT: usize = 15;
const MAX_EXPONENT: usize = 24;

const ROOT_SCOPE: &str = "pcs-compare:verified_trial";
const MATERIALIZE_SCOPE: &str = "pcs-compare:materialize";
const COMMIT_SCOPE: &str = "pcs-compare:commit";
const CLAIM_SCOPE: &str = "pcs-compare:claim_setup";
const OPENING_SCOPE: &str = "pcs-compare:opening";
const VERIFY_SCOPE: &str = "pcs-compare:verification";

const F2Z_IMPLEMENTATION: &str = "f2z";
const WHIR_IMPLEMENTATION: &str = "plonky3-whir";
const BINIUS_IMPLEMENTATION: &str = "binius64-basefold";
const LIGERITO_IMPLEMENTATION: &str = "f2z-ligerito-binary";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    F2z,
    Whir,
    Binius,
    /// The F2Z opener on the Binius64 packed rows and the identical bit-MLE
    /// claim: rate 1/2, Johnson-regime Ligerito, grinding, Round 0.
    Ligerito,
}

impl Backend {
    const fn implementation(self) -> &'static str {
        match self {
            Self::F2z => F2Z_IMPLEMENTATION,
            Self::Whir => WHIR_IMPLEMENTATION,
            Self::Binius => BINIUS_IMPLEMENTATION,
            Self::Ligerito => LIGERITO_IMPLEMENTATION,
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
            Self::F2z => 0x4632_5a00_0000_0001,
            Self::Whir => 0x5748_4952_0000_0001,
            Self::Binius => 0x4249_4e49_5553_0001,
            Self::Ligerito => 0x4c49_4745_0000_0001,
        }
    }

    const fn challenge_extension_degree(self) -> Option<usize> {
        match self {
            Self::F2z => None,
            Self::Whir => Some(whir::CHALLENGE_EXTENSION_DEGREE),
            Self::Binius | Self::Ligerito => Some(128),
        }
    }

    const fn configured_max_pow_bits(self) -> Option<usize> {
        match self {
            Self::F2z => None,
            Self::Whir => Some(whir::MAX_POW_BITS),
            Self::Binius | Self::Ligerito => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Trial {
    Warmup,
    Sample(usize),
}

impl Trial {
    fn id_fragment(self) -> String {
        match self {
            Self::Warmup => "warmup-0".to_owned(),
            Self::Sample(index) => format!("sample-{index}"),
        }
    }

    fn json(self) -> Value {
        match self {
            Self::Warmup => json!({"kind": "warmup", "warmup_index": 0}),
            Self::Sample(index) => json!({"kind": "sample", "sample_index": index}),
        }
    }

    const fn seed_word(self) -> u64 {
        match self {
            Self::Warmup => 0,
            Self::Sample(index) => index as u64 + 1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ArtifactSizes {
    commitment_bytes: usize,
    public_claim_bytes: usize,
    opening_proof_bytes: usize,
    total_wire_bytes: usize,
}

#[derive(Debug)]
enum CellOutcome {
    Measured {
        derived_max_pow_bits: Option<usize>,
    },
    Unavailable {
        reason: String,
        required_pow_bits: usize,
        budget: usize,
    },
}

#[derive(serde::Serialize)]
struct CampaignCell {
    implementation: &'static str,
    log_multiplications: usize,
    challenge_extension_degree: Option<usize>,
    configured_max_pow_bits: Option<usize>,
    derived_max_pow_bits: Option<usize>,
    #[serde(flatten)]
    status: CellStatus,
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CellStatus {
    Measured,
    Unavailable {
        reason: String,
        required_pow_bits: usize,
        budget: usize,
    },
    NotRequested {
        reason: &'static str,
    },
}

impl CampaignCell {
    fn not_requested(backend: Backend, exponent: usize, reason: &'static str) -> Self {
        Self {
            implementation: backend.implementation(),
            log_multiplications: exponent,
            challenge_extension_degree: backend.challenge_extension_degree(),
            configured_max_pow_bits: backend.configured_max_pow_bits(),
            derived_max_pow_bits: None,
            status: CellStatus::NotRequested { reason },
        }
    }
}

impl CellOutcome {
    fn record(self, backend: Backend, exponent: usize) -> CampaignCell {
        let (derived_max_pow_bits, status) = match self {
            Self::Measured {
                derived_max_pow_bits,
            } => (derived_max_pow_bits, CellStatus::Measured),
            Self::Unavailable {
                reason,
                required_pow_bits,
                budget,
            } => (
                Some(required_pow_bits),
                CellStatus::Unavailable {
                    reason,
                    required_pow_bits,
                    budget,
                },
            ),
        };
        CampaignCell {
            implementation: backend.implementation(),
            log_multiplications: exponent,
            challenge_extension_degree: backend.challenge_extension_degree(),
            configured_max_pow_bits: backend.configured_max_pow_bits(),
            derived_max_pow_bits,
            status,
        }
    }
}
struct CampaignWriter {
    path: Option<PathBuf>,
}

impl CampaignWriter {
    fn from_env() -> Self {
        Self {
            path: std::env::var_os("F2Z_PCS_COMPARE_CAMPAIGN_PATH").map(PathBuf::from),
        }
    }

    fn write(
        self,
        campaign_id: &str,
        root_seed: u64,
        reps: usize,
        requested_exponents: &[usize],
        selected_backends: &[Backend],
        mut cells: Vec<CampaignCell>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            BenchmarkOutput::new(parent).create_dir_all()?;
        }
        for exponent in MIN_EXPONENT..=MAX_EXPONENT {
            for backend in [
                Backend::F2z,
                Backend::Whir,
                Backend::Binius,
                Backend::Ligerito,
            ] {
                let already_present = cells.iter().any(|cell| {
                    cell.implementation == backend.implementation()
                        && cell.log_multiplications == exponent
                });
                if !already_present {
                    cells.push(CampaignCell::not_requested(
                        backend,
                        exponent,
                        "shape omitted by F2Z_BENCH_SHAPES",
                    ));
                }
            }
        }
        cells.sort_by_key(|cell| (cell.log_multiplications, cell.implementation));
        let manifest = json!({
            "schema": "baby-bear-pcs-compare-campaign/v2",
            "campaign_id": campaign_id,
            "trace_schema": "zkperf.trace/v1",
            "trace_path": std::env::var("F2Z_PCS_COMPARE_TRACE_PATH").ok(),
            "root_seed": format!("{root_seed:#018x}"),
            "warmups_per_runnable_cell": 1,
            "samples_per_runnable_cell": reps,
            "requested_log_multiplications": requested_exponents,
            "selected_implementations": selected_backends
                .iter()
                .map(|backend| backend.implementation())
                .collect::<Vec<_>>(),
            "whir_configuration": {
                "challenge_extension_degree": whir::CHALLENGE_EXTENSION_DEGREE,
                "target_bits": whir::SECURITY_BITS,
                "security_assumption": whir::SECURITY_ASSUMPTION_LABEL,
                "folding_factor": std::env::var("F2Z_WHIR_FOLDING").ok(),
                "default_folding_schedule": requested_exponents
                    .iter()
                    .map(|&exponent| json!({
                        "log_multiplications": exponent,
                        "folding_factor": tuned_whir_folding(exponent),
                    }))
                    .collect::<Vec<_>>(),
                "starting_log_inverse_rate": whir::STARTING_LOG_INV_RATE,
                "max_pow_bits": whir::MAX_POW_BITS,
                "hiding": false,
                "plonky3_revision": common::locked_git_revision("p3-whir"),
            },
            "cells": cells,
        });
        BenchmarkOutput::new("").write_json(
            path,
            &manifest,
            FileMode::CreateNew,
            JsonStyle::Pretty,
        )?;
        Ok(())
    }
}

struct F2zClaimFixture {
    claim: ScaledMleEvaluationClaim<SpartanF2zField>,
    prover_transcript: Blake3Transcript,
    verifier_transcript: Blake3Transcript,
}

struct TraceWriter {
    output: JsonlWriter<Box<dyn Write>>,
    campaign_id: String,
    git_rev: String,
    git_dirty: bool,
    build_profile: String,
    cpu: String,
    threads: usize,
}

struct RunMetadata<'a> {
    backend: Backend,
    exponent: usize,
    shape_seed: u64,
    trial_seed: u64,
    trial: Trial,
    capacity: usize,
    gate_vars: usize,
    witness_digest: &'a str,
    witness_ms: f64,
    setup_ms: f64,
    security: &'a Value,
    artifacts: ArtifactSizes,
}

impl TraceWriter {
    fn from_env(threads: usize, campaign_id: &str) -> Result<Self, Box<dyn Error>> {
        let output: Box<dyn Write> = match std::env::var_os("F2Z_PCS_COMPARE_TRACE_PATH") {
            Some(path) => {
                let path = Path::new(&path);
                if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
                    BenchmarkOutput::new(parent).create_dir_all()?;
                }
                Box::new(BenchmarkOutput::new("").buffered(path, FileMode::CreateNew)?)
            }
            None => Box::new(BufWriter::new(io::stdout())),
        };

        let git_rev = std::env::var("F2Z_PCS_COMPARE_GIT_REV")
            .unwrap_or_else(|_| command_output("git", &["rev-parse", "HEAD"], "unknown"));
        let git_dirty = std::env::var("F2Z_PCS_COMPARE_GIT_DIRTY")
            .ok()
            .map(|value| parse_bool(&value, "F2Z_PCS_COMPARE_GIT_DIRTY"))
            .unwrap_or_else(detect_git_dirty);
        let build_profile =
            std::env::var("F2Z_PCS_COMPARE_BUILD_PROFILE").unwrap_or_else(|_| "bench".to_owned());
        let cpu = std::env::var("F2Z_PCS_COMPARE_CPU").unwrap_or_else(|_| {
            command_output("sysctl", &["-n", "machdep.cpu.brand_string"], "unknown CPU")
        });

        Ok(Self {
            output: JsonlWriter::new(output),
            campaign_id: campaign_id.to_owned(),
            git_rev,
            git_dirty,
            build_profile,
            cpu,
            threads,
        })
    }

    fn write_run(
        &mut self,
        metadata: &RunMetadata<'_>,
        intervals: &[ProfileInterval],
    ) -> Result<(), Box<dyn Error>> {
        let roots = intervals
            .iter()
            .filter(|interval| interval.parent_order.is_none())
            .collect::<Vec<_>>();
        if roots.len() != 1 || roots[0].label != ROOT_SCOPE {
            return Err(format!(
                "expected exactly one {ROOT_SCOPE:?} trace root, found {:?}",
                roots.iter().map(|root| root.label).collect::<Vec<_>>()
            )
            .into());
        }

        let backend = metadata.backend;
        let implementation = backend.implementation();
        let trial_fragment = metadata.trial.id_fragment();
        let run_id = format!(
            "baby-bear-pcs-{}-{implementation}-2p{}-{trial_fragment}",
            self.campaign_id, metadata.exponent
        );
        let series_id = format!(
            "baby-bear-pcs-{}-{implementation}-2p{}-{}-{}t-{}",
            self.campaign_id, metadata.exponent, self.git_rev, self.threads, self.build_profile
        );
        let root_span_id = span_id(roots[0].order);
        let algorithm = match backend {
            Backend::F2z => "BabyBear integer assignment / fixed-q F2Z opening",
            Backend::Whir => "BabyBear integer assignment / Plonky3 WHIR prescribed opening",
            Backend::Binius => {
                "BabyBear integer assignment / Binius64 ring-switch + BaseFold opening"
            }
            Backend::Ligerito => {
                "BabyBear integer assignment / F2Z opener: Round 0 + ring switch + Johnson-regime Ligerito"
            }
        };
        let field = match backend {
            Backend::F2z => json!({
                "committed_encoding": "31 little-endian bits per A/B/C/K value",
                "evaluation_field": "F_q",
                "q": FQ_MOD.to_string(),
                "commitment_field": "GF(2^128)",
            }),
            Backend::Whir => json!({
                "committed_encoding": "four native BabyBear columns A/B/C/K",
                "base_field": "BabyBear",
                "base_modulus": 2_013_265_921_u64,
                "challenge_field": format!(
                    "BinomialExtensionField<BabyBear, {}>",
                    whir::CHALLENGE_EXTENSION_DEGREE
                ),
                "challenge_extension_degree": whir::CHALLENGE_EXTENSION_DEGREE,
            }),
            Backend::Binius | Backend::Ligerito => json!({
                "committed_encoding": "one GF(2^128) row per gate: A[0..31] | B[31..62] | C[62..93] | K[93..124]",
                "base_field": "GF(2)",
                "challenge_field": "GF(2^128)-GHASH",
                "challenge_extension_degree": 128,
            }),
        };

        let mut trial_json = metadata.trial.json();
        trial_json["seed"] = json!(format!("{:#018x}", metadata.trial_seed));
        let run = json!({
            "schema": "zkperf.trace/v1",
            "record": "run",
            "run_id": run_id,
            "series_id": series_id,
            "root_span_id": root_span_id,
            "benchmark": {
                "suite": "f2z-pcs",
                "name": "baby-bear-pcs-compare",
                "label": format!("{} at 2^{} BabyBear multiplications", backend.display(), metadata.exponent),
                "algorithm": algorithm,
                "implementation": implementation,
                "git_rev": self.git_rev,
                "git_dirty": self.git_dirty,
                "build_profile": self.build_profile,
            },
            "trial": trial_json,
            "clock": {
                "id": format!("mono-process-{}-{run_id}", std::process::id()),
                "kind": "monotonic",
                "unit": "ns",
                "source": "std::time::Instant",
            },
            "status": "ok",
            "trace_complete": true,
            "environment": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "cpu": self.cpu,
                "threads": self.threads,
                "thread_policy": "Rayon pool; no affinity pinning",
            },
            "parameters": {
                "input": {
                    "log_multiplications": metadata.exponent,
                    "multiplications": 1usize << metadata.exponent,
                    "capacity": metadata.capacity,
                    "gate_variables": metadata.gate_vars,
                    "assignment_variables": metadata.gate_vars + 3,
                    "logical_assignment": "[e0 | A | B | C | K | 0 | 0 | 0]",
                    "witness_digest_blake3": metadata.witness_digest,
                    "witness_generation_ms_excluded": metadata.witness_ms,
                    "setup_ms_excluded": metadata.setup_ms,
                },
                "statement": {
                    "terminal_claim": "D * f(x, beta) = V",
                    "point_policy": "transcript-derived after commitment",
                    "same_integer_witness": true,
                    "independent_native_field_challenges": true,
                },
                "field": field,
                "security": metadata.security,
                "recursion": {"max_depth": 0, "instance_count": 1},
                "repetition": {"count": 1},
                "seed": format!("{:#018x}", metadata.shape_seed),
            },
            "artifacts": {
                "commitment_bytes": metadata.artifacts.commitment_bytes,
                "public_claim_bytes": metadata.artifacts.public_claim_bytes,
                "opening_proof_bytes": metadata.artifacts.opening_proof_bytes,
                "total_wire_bytes": metadata.artifacts.total_wire_bytes,
            },
            "tags": {
                "campaign_id": self.campaign_id,
                "root_boundary": "materialization through verified terminal opening; setup and logical witness generation excluded",
                "timeline": "observed half-open intervals",
                "plonky3_revision": common::locked_git_revision("p3-whir"),
            },
        });
        self.output.write(&run)?;

        let by_order = intervals
            .iter()
            .map(|interval| (interval.order, interval))
            .collect::<HashMap<_, _>>();
        let mut totals = HashMap::<(Option<u64>, &'static str), usize>::new();
        for interval in intervals {
            *totals
                .entry((interval.parent_order, interval.label))
                .or_default() += 1;
        }
        let mut seen = HashMap::<(Option<u64>, &'static str), usize>::new();
        for interval in intervals {
            let descriptor = describe_span(interval, &by_order, backend);
            let occurrence_key = (interval.parent_order, interval.label);
            let occurrence_index = seen.entry(occurrence_key).or_default();
            let occurrence_count = totals[&occurrence_key];
            let coordinate = if occurrence_count > 1 {
                json!({
                    "occurrence_index": *occurrence_index,
                    "occurrence_count": occurrence_count,
                })
            } else {
                json!({})
            };
            *occurrence_index += 1;

            let mut attributes = json!({
                "scope_kind": descriptor.scope_kind,
                "short_name": descriptor.short_name,
                "primary_sequence": descriptor.primary_sequence,
                "source_label": interval.label,
            });
            if !descriptor.math_latex.is_empty() {
                attributes["math_latex"] = json!(descriptor.math_latex);
            }
            let span = json!({
                "schema": "zkperf.trace/v1",
                "record": "span",
                "run_id": run_id,
                "span_id": span_id(interval.order),
                "parent_span_id": interval.parent_order.map(span_id),
                "operation": descriptor.operation,
                "name": descriptor.name,
                "primary_phase": descriptor.primary_phase,
                "phase_tags": descriptor.phase_tags,
                "start_ns": interval.start_ns.to_string(),
                "end_ns": interval.end_ns.to_string(),
                "duration_ns": interval.end_ns.saturating_sub(interval.start_ns).to_string(),
                "lane": {"process": "benchmark", "thread": "control"},
                "coordinate": coordinate,
                "attributes": attributes,
            });
            self.output.write(&span)?;
        }
        self.output.flush()?;
        Ok(())
    }
}

struct SpanDescriptor {
    operation: String,
    name: String,
    short_name: String,
    primary_phase: &'static str,
    phase_tags: Vec<&'static str>,
    scope_kind: &'static str,
    primary_sequence: bool,
    math_latex: Vec<&'static str>,
}

fn describe_span(
    interval: &ProfileInterval,
    by_order: &HashMap<u64, &ProfileInterval>,
    backend: Backend,
) -> SpanDescriptor {
    let mut labels = Vec::new();
    let mut cursor = Some(interval);
    while let Some(current) = cursor {
        labels.push(current.label);
        cursor = current
            .parent_order
            .and_then(|order| by_order.get(&order).copied());
    }
    let under = |label: &str| labels.iter().any(|candidate| *candidate == label);
    let has = |fragment: &str| labels.iter().any(|label| label.contains(fragment));
    let root = interval.parent_order.is_none();
    let verifying = under(VERIFY_SCOPE);
    let materializing = under(MATERIALIZE_SCOPE);
    let committing = under(COMMIT_SCOPE);
    let claim_setup = under(CLAIM_SCOPE);
    let opening = under(OPENING_SCOPE);
    let sumcheck = has("sumcheck") || under("mc:presum_run") || under("eqf:rounds");
    let fri =
        opening && !has("eqf:") && (under("mc:forest") || under("mc:fold_v") || has("ligerito"));

    let primary_phase = if root {
        "end-to-end"
    } else if verifying {
        "verification"
    } else if sumcheck {
        "sumcheck"
    } else if fri {
        "fri"
    } else if materializing || claim_setup {
        "preparation"
    } else if committing {
        "commit"
    } else if opening {
        "opening-proof"
    } else {
        "proving"
    };
    let mut phase_tags = Vec::new();
    push_tag(&mut phase_tags, primary_phase);
    if !root {
        push_tag(
            &mut phase_tags,
            if verifying { "verification" } else { "proving" },
        );
    }
    if committing {
        push_tag(&mut phase_tags, "commit");
        push_tag(&mut phase_tags, "pcs");
    }
    if opening {
        push_tag(&mut phase_tags, "opening-proof");
        push_tag(&mut phase_tags, "pcs");
    }
    if sumcheck {
        push_tag(&mut phase_tags, "sumcheck");
    }
    if fri {
        push_tag(&mut phase_tags, "fri");
    }
    if materializing || claim_setup {
        push_tag(&mut phase_tags, "preparation");
    }

    let (operation, name, short_name, math_latex) = match interval.label {
        ROOT_SCOPE => (
            "pcs_compare.verified_trial".to_owned(),
            "Complete verified terminal opening".to_owned(),
            "Verified trial".to_owned(),
            match backend {
                Backend::F2z => vec![
                    "\\operatorname{Verify}_{\\mathrm{F2Z}}(C_{\\mathrm{F2Z}},x,\\beta,D,V,\\pi_{\\mathrm{F2Z}})=1",
                ],
                Backend::Whir => vec![
                    "\\operatorname{Verify}_{\\mathrm{WHIR}}(C_{\\mathrm{WHIR}},x,\\mathbf{u},\\pi_{\\mathrm{WHIR}})=1",
                    "D\\,\\widetilde f(x,\\beta;\\mathbf{u})=V",
                ],
                Backend::Binius => {
                    vec!["\\operatorname{Verify}_{\\mathrm{BaseFold}}(C_{\\mathbb F_2},r,v,\\pi)=1"]
                }
                Backend::Ligerito => {
                    vec!["\\operatorname{Verify}_{\\mathrm{Ligerito}}(C_{\\mathbb F_2},r,v,\\pi)=1"]
                }
            },
        ),
        MATERIALIZE_SCOPE => (
            "pcs_compare.materialize".to_owned(),
            "Materialize backend-native commitment witness".to_owned(),
            "Materialize".to_owned(),
            match backend {
                Backend::F2z => vec!["\\mathcal{B}=\\operatorname{Bit}_{31}(A,B,C,K)"],
                Backend::Whir => vec!["U=(A,B,C,K)\\in\\mathbb{F}_p^{4\\times 2^g}"],
                Backend::Binius | Backend::Ligerito => {
                    vec!["w_i=A_i+2^{31}B_i+2^{62}C_i+2^{93}K_i\\in\\mathbb F_2^{128}"]
                }
            },
        ),
        COMMIT_SCOPE => (
            "pcs_compare.commit".to_owned(),
            "Commit to the backend-native witness encoding".to_owned(),
            "Commit".to_owned(),
            match backend {
                Backend::F2z => {
                    vec![
                        "C_{\\mathrm{F2Z}}\\leftarrow\\operatorname{Commit}_{\\mathrm{F2Z}}(\\mathcal{B})",
                    ]
                }
                Backend::Whir => {
                    vec!["C_{\\mathrm{WHIR}}\\leftarrow\\operatorname{Commit}_{\\mathrm{WHIR}}(U)"]
                }
                Backend::Binius => {
                    vec![
                        "C_{\\mathrm{BF}}\\leftarrow\\operatorname{Commit}_{\\mathrm{BaseFold}}(w)",
                    ]
                }
                Backend::Ligerito => {
                    vec![
                        "C\\leftarrow\\operatorname{Merkle}(\\operatorname{RS}_{1/8}(w)),\\;y=\\widetilde w(\\zeta,\\zeta^2,\\ldots)",
                    ]
                }
            },
        ),
        CLAIM_SCOPE => (
            "pcs_compare.claim_setup".to_owned(),
            "Derive and bind the terminal MLE claim".to_owned(),
            "Claim setup".to_owned(),
            vec!["D\\,\\widetilde f(x,\\beta)=V"],
        ),
        OPENING_SCOPE => (
            "pcs_compare.opening".to_owned(),
            "Prove the prescribed terminal opening".to_owned(),
            "Opening".to_owned(),
            match backend {
                Backend::F2z => vec![
                    "D\\,\\widetilde f(x,\\beta)=V",
                    "\\pi_{\\mathrm{F2Z}}\\leftarrow\\operatorname{Open}_{\\mathrm{F2Z}}(C_{\\mathrm{F2Z}},x,\\beta,D,V)",
                ],
                Backend::Whir => vec![
                    "u_j=\\widetilde U_j(x)\\quad(j\\in\\{A,B,C,K\\})",
                    "(u_A,u_B,u_C,u_K,\\pi_{\\mathrm{WHIR}})\\leftarrow\\operatorname{Open}_{\\mathrm{WHIR}}(C_{\\mathrm{WHIR}},x)",
                ],
                Backend::Binius => vec![
                    "v=\\widetilde{\\operatorname{bits}(w)}(r_{\\mathrm{bit}},r_{\\mathrm{gate}})",
                    "\\pi\\leftarrow\\operatorname{RingSwitch+BaseFold}(C_{\\mathrm{BF}},r,v)",
                ],
                Backend::Ligerito => vec![
                    "v=\\widetilde{\\operatorname{bits}(w)}(r_{\\mathrm{bit}},r_{\\mathrm{gate}})",
                    "\\pi\\leftarrow\\operatorname{RingSwitch+Ligerito}_{\\mathrm{Johnson}}(C,r,v)",
                ],
            },
        ),
        VERIFY_SCOPE => (
            "pcs_compare.verify".to_owned(),
            "Verify the prescribed terminal opening".to_owned(),
            "Verify".to_owned(),
            match backend {
                Backend::F2z => vec![
                    "\\operatorname{Verify}_{\\mathrm{F2Z}}(C_{\\mathrm{F2Z}},x,\\beta,D,V,\\pi_{\\mathrm{F2Z}})=1",
                ],
                Backend::Whir => vec![
                    "\\operatorname{Verify}_{\\mathrm{WHIR}}(C_{\\mathrm{WHIR}},x,\\mathbf{u},\\pi_{\\mathrm{WHIR}})=1",
                    "D\\,\\widetilde f(x,\\beta;\\mathbf{u})=V",
                ],
                Backend::Binius => {
                    vec!["\\operatorname{Verify}_{\\mathrm{BaseFold}}(C_{\\mathrm{BF}},r,v,\\pi)=1"]
                }
                Backend::Ligerito => {
                    vec!["\\operatorname{Verify}_{\\mathrm{Ligerito}}(C,r,v,\\pi)=1"]
                }
            },
        ),
        binius::COMMIT_ORACLE_SCOPE => (
            "binius.basefold.commit_oracle".to_owned(),
            "Encode and Merkle-commit the BaseFold oracle".to_owned(),
            "BaseFold commit".to_owned(),
            vec![r"C_{\mathrm{BF}}\leftarrow\operatorname{Merkle}(\operatorname{NTT}(w))"],
        ),
        binius::SAMPLE_POINT_SCOPE => (
            "binius.claim.sample_point".to_owned(),
            "Sample the binary MLE opening point".to_owned(),
            "Sample point".to_owned(),
            vec![r"r\leftarrow\mathcal T\in\mathbb F_{2^{128}}^{g+7}"],
        ),
        binius::EVALUATE_CLAIM_SCOPE => (
            "binius.claim.evaluate_bit_mle".to_owned(),
            "Evaluate the packed witness as a bit-MLE".to_owned(),
            "Evaluate bit-MLE".to_owned(),
            vec![r"v=\widetilde{\operatorname{bits}(w)}(r_{\mathrm{bit}},r_{\mathrm{gate}})"],
        ),
        binius::RING_SWITCH_SCOPE => (
            "binius.opening.ring_switch".to_owned(),
            "Reduce the bit-MLE claim by ring switching".to_owned(),
            "Ring switch".to_owned(),
            vec![
                r"\widetilde{\operatorname{bits}(w)}(r_{\mathrm{bit}},r_{\mathrm{gate}})=v\;\Longrightarrow\;\langle w,\rho_r\rangle=\widehat v",
            ],
        ),
        binius::BASEFOLD_OPEN_SCOPE => (
            "binius.opening.basefold".to_owned(),
            "Prove the reduced relation with BaseFold".to_owned(),
            "BaseFold opening".to_owned(),
            vec![
                r"\pi_{\mathrm{BF}}\leftarrow\operatorname{BaseFold.Open}(C_{\mathrm{BF}},\rho_r,\widehat v)",
            ],
        ),
        binius::RING_SWITCH_VERIFY_SCOPE => (
            "binius.verify.ring_switch".to_owned(),
            "Verify the ring-switch reduction".to_owned(),
            "Verify ring switch".to_owned(),
            vec![r"\operatorname{VerifyRS}(r,v,\widehat v)=1"],
        ),
        binius::BASEFOLD_VERIFY_SCOPE => (
            "binius.verify.basefold".to_owned(),
            "Verify the BaseFold opening".to_owned(),
            "Verify BaseFold".to_owned(),
            vec![
                r"\operatorname{BaseFold.Verify}(C_{\mathrm{BF}},\rho_r,\widehat v,\pi_{\mathrm{BF}})=1",
            ],
        ),
        label => {
            let readable = label.replace([':', '_'], " ");
            (
                format!("pcs_compare.detail.{}", sanitize_operation(label)),
                readable.clone(),
                readable,
                Vec::new(),
            )
        }
    };
    let primary_sequence = matches!(
        interval.label,
        MATERIALIZE_SCOPE | COMMIT_SCOPE | CLAIM_SCOPE | OPENING_SCOPE | VERIFY_SCOPE
    );
    SpanDescriptor {
        operation,
        name,
        short_name,
        primary_phase,
        phase_tags,
        scope_kind: if root {
            "scope"
        } else if primary_sequence {
            "phase"
        } else {
            "procedure"
        },
        primary_sequence,
        math_latex,
    }
}

fn push_tag(tags: &mut Vec<&'static str>, tag: &'static str) {
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

fn sanitize_operation(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn campaign_id() -> String {
    let raw = std::env::var("F2Z_PCS_COMPARE_CAMPAIGN_ID").unwrap_or_else(|_| {
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        format!("{epoch_nanos}-{}", std::process::id())
    });
    assert!(
        !raw.is_empty()
            && raw
                .chars()
                .all(|character| character.is_ascii_alphanumeric()
                    || matches!(character, '-' | '_' | '.')),
        "F2Z_PCS_COMPARE_CAMPAIGN_ID must contain only ASCII letters, digits, '.', '-', or '_'"
    );
    raw
}

fn span_id(order: u64) -> String {
    format!("span-{order}")
}

fn parse_bool(value: &str, variable: &str) -> bool {
    match value {
        "1" | "true" | "yes" => true,
        "0" | "false" | "no" => false,
        _ => panic!("{variable} must be one of 0, 1, false, true, no, yes"),
    }
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

fn detect_git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .map_or(true, |output| {
            !output.status.success() || !output.stdout.is_empty()
        })
}

fn exponents() -> Vec<usize> {
    common::shapes(None).map_or_else(
        || (MIN_EXPONENT..=MAX_EXPONENT).collect(),
        |shapes| {
            let mut exponents = Vec::with_capacity(shapes.len());
            for value in shapes.iter() {
                let exponent = value
                    .parse::<usize>()
                    .expect("F2Z_BENCH_SHAPES must contain integer exponents");
                assert!(
                    (MIN_EXPONENT..=MAX_EXPONENT).contains(&exponent),
                    "BabyBear PCS comparison exponents must be in {MIN_EXPONENT}..={MAX_EXPONENT}"
                );
                assert!(
                    !exponents.contains(&exponent),
                    "F2Z_BENCH_SHAPES contains duplicate exponent {exponent}"
                );
                exponents.push(exponent);
            }
            exponents
        },
    )
}

fn selected_backends() -> Vec<Backend> {
    let raw = std::env::var("F2Z_PCS_COMPARE_BACKENDS")
        .unwrap_or_else(|_| "f2z plonky3-whir binius64-basefold f2z-ligerito-binary".to_owned());
    let mut backends = Vec::new();
    for value in raw.split([',', ' ']).filter(|value| !value.is_empty()) {
        let backend = match value {
            "f2z" => Backend::F2z,
            "plonky3-whir" | "whir" => Backend::Whir,
            "binius" | "binius64" | "binius64-basefold" => Backend::Binius,
            "ligerito" | "f2z-ligerito" | "f2z-ligerito-binary" => Backend::Ligerito,
            _ => {
                panic!(
                    "F2Z_PCS_COMPARE_BACKENDS accepts f2z, plonky3-whir, binius64-basefold, and f2z-ligerito-binary; got {value:?}"
                )
            }
        };
        if !backends.contains(&backend) {
            backends.push(backend);
        }
    }
    assert!(
        !backends.is_empty(),
        "F2Z_PCS_COMPARE_BACKENDS must select at least one backend"
    );
    backends
}

fn ordered_backends(selected: &[Backend], exponent: usize) -> Vec<Backend> {
    let requested = std::env::var("F2Z_BENCH_ORDER").unwrap_or_else(|_| "alternate".to_owned());
    let preference = match requested.as_str() {
        "alternate" => {
            if exponent.is_multiple_of(2) {
                [
                    Backend::F2z,
                    Backend::Whir,
                    Backend::Binius,
                    Backend::Ligerito,
                ]
            } else {
                [
                    Backend::Ligerito,
                    Backend::Binius,
                    Backend::Whir,
                    Backend::F2z,
                ]
            }
        }
        "f2z-first" => [
            Backend::F2z,
            Backend::Whir,
            Backend::Binius,
            Backend::Ligerito,
        ],
        "whir-first" => [
            Backend::Whir,
            Backend::Binius,
            Backend::Ligerito,
            Backend::F2z,
        ],
        "binius-first" => [
            Backend::Binius,
            Backend::Ligerito,
            Backend::Whir,
            Backend::F2z,
        ],
        _ => {
            panic!(
                "F2Z_BENCH_ORDER must be alternate, f2z-first, whir-first, or binius-first for this benchmark"
            )
        }
    };
    preference
        .into_iter()
        .filter(|backend| selected.contains(backend))
        .collect()
}

fn mix_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn trial_seed(shape_seed: u64, backend: Backend, trial: Trial) -> u64 {
    mix_seed(shape_seed ^ backend.seed_tag() ^ trial.seed_word().rotate_left(17))
}

fn public_claim_bytes(gate_vars: usize) -> usize {
    // x has gate_vars coordinates, beta has three, followed by D and V.
    (gate_vars + 5) * 16
}

fn checked_artifacts(
    commitment_bytes: usize,
    public_claim_bytes: usize,
    opening_proof_bytes: usize,
) -> Result<ArtifactSizes, Box<dyn Error>> {
    let total_wire_bytes = commitment_bytes
        .checked_add(public_claim_bytes)
        .and_then(|value| value.checked_add(opening_proof_bytes))
        .ok_or("wire-size accounting overflow")?;
    Ok(ArtifactSizes {
        commitment_bytes,
        public_claim_bytes,
        opening_proof_bytes,
        total_wire_bytes,
    })
}

fn sub_mod(arith: &ProjArith, left: u128, right: u128) -> u128 {
    if right == 0 {
        left
    } else {
        arith.add(left, arith.q() - right)
    }
}

fn equality_table(point: &[u128], arith: &ProjArith) -> Vec<u128> {
    let mut table = vec![0; 1usize << point.len()];
    table[0] = 1;
    let mut half = 1;
    for &coordinate in point {
        for index in 0..half {
            let parent = table[index];
            let one_child = arith.mul(parent, coordinate);
            table[index] = sub_mod(arith, parent, one_child);
            table[index + half] = one_child;
        }
        half *= 2;
    }
    table
}

fn selector_factors(beta: &[u128; 3], arith: &ProjArith) -> [u128; 5] {
    let one_minus = [
        sub_mod(arith, 1, beta[0]),
        sub_mod(arith, 1, beta[1]),
        sub_mod(arith, 1, beta[2]),
    ];
    let product3 = |x, y, z| arith.mul(arith.mul(x, y), z);
    [
        product3(one_minus[0], one_minus[1], one_minus[2]),
        product3(beta[0], one_minus[1], one_minus[2]),
        product3(one_minus[0], beta[1], one_minus[2]),
        product3(beta[0], beta[1], one_minus[2]),
        product3(one_minus[0], one_minus[1], beta[2]),
    ]
}

fn seed_f2z_claim_transcript(commitment_encoding: &[u8], seed: u64) -> Blake3Transcript {
    let mut transcript = Blake3Transcript::new();
    transcript.absorb_slice(b"f2z/baby-bear-pcs-compare/terminal-claim/v1");
    transcript.absorb_slice(&seed.to_le_bytes());
    transcript.absorb_slice(commitment_encoding);
    transcript
}

fn derive_f2z_claim(
    witness: &BabyBearMulWitness,
    mut transcript: Blake3Transcript,
    mut verifier_transcript: Blake3Transcript,
) -> F2zClaimFixture {
    let config = spartan_f2z_field_config();
    let gate_vars = witness.layout().gate_vars();
    let point = transcript.get_field_challenges::<SpartanF2zField>(gate_vars + 3, &config);
    let scale = transcript.get_field_challenge::<SpartanF2zField>(&config);
    let verifier_point =
        verifier_transcript.get_field_challenges::<SpartanF2zField>(gate_vars + 3, &config);
    let verifier_scale = verifier_transcript.get_field_challenge::<SpartanF2zField>(&config);
    assert_eq!(
        verifier_point, point,
        "verifier must replay the F2Z claim point"
    );
    assert_eq!(verifier_scale, scale, "verifier must replay the F2Z scale");
    let canonical_point = point
        .iter()
        .map(ProjectCanonicalU128::canonical_u128)
        .collect::<Vec<_>>();
    let gate_point = &canonical_point[..gate_vars];
    let beta: [u128; 3] = canonical_point[gate_vars..]
        .try_into()
        .expect("three block-selector coordinates");
    let arith = ProjArith::new(FQ_MOD);
    let equality = equality_table(gate_point, &arith);
    let selectors = selector_factors(&beta, &arith);

    let mut private = 0;
    for gate in 0..witness.layout().capacity() {
        let selected = arith.add(
            arith.add(
                arith.mul(selectors[1], u128::from(witness.a_values()[gate])),
                arith.mul(selectors[2], u128::from(witness.b_values()[gate])),
            ),
            arith.add(
                arith.mul(selectors[3], u128::from(witness.c_values()[gate])),
                arith.mul(selectors[4], u128::from(witness.k_values()[gate])),
            ),
        );
        private = arith.add(private, arith.mul(equality[gate], selected));
    }
    let f_evaluation = arith.add(arith.mul(selectors[0], equality[0]), private);
    let value = arith.mul(scale.canonical_u128(), f_evaluation);
    let value = SpartanF2zField::from_with_cfg(value, &config);
    let claim = ScaledMleEvaluationClaim::new(point.into_boxed_slice(), scale, value);
    F2zClaimFixture {
        claim,
        prover_transcript: transcript,
        verifier_transcript,
    }
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

fn f2z_security(exponent: usize) -> Result<Value, Box<dyn Error>> {
    let resolved = common::ligerito_selection(100).resolve(exponent, 100)?;
    let config = resolved.prover();
    let total_query_openings = config.queries.iter().sum::<usize>();
    Ok(json!({
        "profile": "fixed-q F2Z/Ligerito",
        "ligerito": common::ligerito_report(&resolved, resolved.round0(100)?),
        "target_bits": 100,
        "evaluation_modulus": FQ_MOD.to_string(),
        "evaluation_modulus_bits": 100,
        "commitment_field": "GF(2^128)",
        "transcript_hash": "BLAKE3",
        "query_schedule": config.queries,
        "total_query_openings": total_query_openings,
        "round_log_inverse_rates": config.log_inv_rates,
        "round_query_pow_bits": config.grinding_bits,
        "round_folding_pow_bits": config.fold_grinding_bits,
        "round_ood_samples": config.ood_samples,
    }))
}

fn whir_security(summary: &SecuritySummary) -> Value {
    let total_query_openings = summary.round_queries.iter().sum::<usize>() + summary.final_queries;
    let mut query_schedule = summary.round_queries.clone();
    query_schedule.push(summary.final_queries);
    json!({
        "profile": format!(
            "Plonky3 WHIR non-hiding {}",
            whir::SECURITY_ASSUMPTION_LABEL
        ),
        "target_bits": summary.target_bits,
        "security_assumption": whir::SECURITY_ASSUMPTION_LABEL,
        "challenge_extension_degree": whir::CHALLENGE_EXTENSION_DEGREE,
        "configured_max_pow_bits": summary.configured_max_pow_bits,
        "derived_max_pow_bits": summary.derived_max_pow_bits,
        "commitment_ood_samples": summary.commitment_ood_samples,
        "starting_folding_pow_bits": summary.starting_folding_pow_bits,
        "folding_schedule": summary.folding_schedule,
        "round_queries": summary.round_queries,
        "round_ood_samples": summary.round_ood_samples,
        "round_folding_factors": summary.round_folding_factors,
        "round_log_inverse_rates": summary.round_log_inverse_rates,
        "round_pow_bits": summary.round_pow_bits,
        "round_folding_pow_bits": summary.round_folding_pow_bits,
        "final_queries": summary.final_queries,
        "final_pow_bits": summary.final_pow_bits,
        "final_sumcheck_rounds": summary.final_sumcheck_rounds,
        "final_folding_pow_bits": summary.final_folding_pow_bits,
        "query_schedule": query_schedule,
        "total_query_openings": total_query_openings,
        "folding_factor": summary.folding_factor,
        "starting_log_inverse_rate": summary.starting_log_inverse_rate,
        "hiding": false,
        "hash": "Poseidon2BabyBear<16>",
        "plonky3_revision": common::locked_git_revision("p3-whir"),
    })
}

fn binius_security(backend: &BiniusBackend) -> Value {
    json!({
        "profile": "Binius64 non-hiding ring-switch + BaseFold",
        "target_bits": binius::SECURITY_BITS,
        "soundness_bound_model": "Diamond-Posen Eq. 42 plus 128-bit SHA-256 cap",
        "estimated_query_soundness_bits": backend.estimated_soundness_bits(),
        "challenge_field": "GF(2^128)-GHASH",
        "hash": "SHA-256",
        "log_inverse_rate": backend.log_inv_rate(),
        "test_queries": backend.n_test_queries(),
        "hiding": false,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_f2z_series(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &BabyBearMulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<CellOutcome, Box<dyn Error>> {
    let setup_started = Instant::now();
    let layout = *witness.layout();
    let preflight_hint =
        f2z::piop::spartan::baby_bear_f2z::commit_baby_bear_mul_witness_with_ligerito(
            &layout,
            witness.f2z_bit_rows(),
            common::ligerito_selection(100),
        )?;
    let preflight_commitment = preflight_hint.commitment.clone();
    drop(preflight_hint);
    // Flock's prover-data drop returns its largest codeword to a process-global
    // pool, so clear it before allocating the shape's relation matrices.
    flock_core::scratch::clear();
    let matrices = prepare_baby_bear_mul_relation(layout, &spartan_f2z_field_config())?;
    let prepared: PreparedBabyBearTerminalF2zOpening =
        f2z::piop::spartan::baby_bear_f2z::prepare_baby_bear_terminal_f2z_opening_with_ligerito(
            &matrices,
            &layout,
            &preflight_commitment,
            common::ligerito_selection(100),
        )?;
    drop((matrices, preflight_commitment));
    let setup_ms = common::elapsed_ms(setup_started);
    let security = f2z_security(exponent)?;
    eprintln!("  F2Z backend_setup_ms={setup_ms:.3}");

    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::F2z, trial);
        clear_profile();
        let (proof, claim_bytes, commitment_bytes, intervals) = {
            let root = f2z::utils::prof::scope(ROOT_SCOPE);
            let rows = {
                let _phase = f2z::utils::prof::scope(MATERIALIZE_SCOPE);
                witness.f2z_bit_rows()
            };
            let (hint, commitment_encoding, prover_transcript, verifier_transcript) = {
                let _phase = f2z::utils::prof::scope(COMMIT_SCOPE);
                let hint = commit_baby_bear_terminal_f2z_witness(&prepared, rows)?;
                let commitment_encoding = bincode::serialize(&hint.commitment)?;
                let prover_transcript = seed_f2z_claim_transcript(&commitment_encoding, seed);
                let verifier_transcript = seed_f2z_claim_transcript(&commitment_encoding, seed);
                (
                    hint,
                    commitment_encoding,
                    prover_transcript,
                    verifier_transcript,
                )
            };
            let mut fixture = {
                let _phase = f2z::utils::prof::scope(CLAIM_SCOPE);
                derive_f2z_claim(witness, prover_transcript, verifier_transcript)
            };
            let (proof, commitment) = {
                let _phase = f2z::utils::prof::scope(OPENING_SCOPE);
                let proof = prove_baby_bear_terminal_claim_f2z(
                    &mut fixture.prover_transcript,
                    &prepared,
                    &hint,
                    &fixture.claim,
                )?;
                let commitment = hint.commitment.clone();
                drop(hint);
                (proof, commitment)
            };
            {
                let _phase = f2z::utils::prof::scope(VERIFY_SCOPE);
                verify_baby_bear_terminal_claim_f2z(
                    &mut fixture.verifier_transcript,
                    &prepared,
                    &commitment,
                    &fixture.claim,
                    &proof,
                )?;
            }
            std::hint::black_box(&proof);
            drop(root);
            let intervals = finish_profile();
            (
                proof,
                public_claim_bytes(layout.gate_vars()),
                commitment_encoding.len(),
                intervals,
            )
        };
        let proof_bytes = baby_bear_terminal_claim_f2z_proof_bytes(&proof).len();
        let artifacts = checked_artifacts(commitment_bytes, claim_bytes, proof_bytes)?;
        let metadata = RunMetadata {
            backend: Backend::F2z,
            exponent,
            shape_seed,
            trial_seed: seed,
            trial,
            capacity: layout.capacity(),
            gate_vars: layout.gate_vars(),
            witness_digest: digest,
            witness_ms,
            setup_ms,
            security: &security,
            artifacts,
        };
        writer.write_run(&metadata, &intervals)?;
        common::pcs_console::print_trial(
            &trial.id_fragment(),
            &intervals,
            proof_bytes,
            artifacts.total_wire_bytes,
        );
        drop(proof);
    }
    Ok(CellOutcome::Measured {
        derived_max_pow_bits: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_whir_series(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &BabyBearMulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<CellOutcome, Box<dyn Error>> {
    let setup_started = Instant::now();
    let (folding, log_inv_rate, max_pow_bits) = whir_tuning(exponent)?;
    let backend = match WhirBackend::setup_with_params(
        witness.layout().capacity(),
        folding,
        log_inv_rate,
        max_pow_bits,
    ) {
        Ok(backend) => backend,
        Err(WhirAdapterError::Config(WhirConfigError::PowBitsExceedBudget {
            required,
            budget,
        })) => {
            let reason = format!(
                "derived proof-of-work of {required} bits exceeds the fixed {budget}-bit budget"
            );
            eprintln!(
                "  WHIR unavailable at 2^{exponent} under the fixed {budget}-bit PoW cap: {reason}"
            );
            return Ok(CellOutcome::Unavailable {
                reason,
                required_pow_bits: required,
                budget,
            });
        }
        Err(error) => return Err(error.into()),
    };
    let summary = backend.security_summary();
    if backend.max_pow_bits() > max_pow_bits {
        return Err(format!(
            "WHIR shape 2^{exponent} derives {} PoW bits, above configured budget {}",
            backend.max_pow_bits(),
            max_pow_bits
        )
        .into());
    }
    let setup_ms = common::elapsed_ms(setup_started);
    let security = whir_security(&summary);
    eprintln!("  WHIR backend_setup_ms={setup_ms:.3}");

    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Whir, trial);
        clear_profile();
        let (opened, claim_bytes, intervals) = {
            let root = f2z::utils::prof::scope(ROOT_SCOPE);
            let materialized = {
                let _phase = f2z::utils::prof::scope(MATERIALIZE_SCOPE);
                backend.materialize(
                    witness.a_values(),
                    witness.b_values(),
                    witness.c_values(),
                    witness.k_values(),
                )?
            };
            let committed = {
                let _phase = f2z::utils::prof::scope(COMMIT_SCOPE);
                backend.commit(materialized, seed)
            };
            let ready = {
                let _phase = f2z::utils::prof::scope(CLAIM_SCOPE);
                backend.derive_and_bind_terminal_claim(committed)?
            };
            let opened = {
                let _phase = f2z::utils::prof::scope(OPENING_SCOPE);
                backend.open(ready)
            };
            {
                let _phase = f2z::utils::prof::scope(VERIFY_SCOPE);
                std::hint::black_box(backend.verify(&opened)?);
            }
            std::hint::black_box(opened.claim());
            drop(root);
            let intervals = finish_profile();
            (
                opened,
                public_claim_bytes(witness.layout().gate_vars()),
                intervals,
            )
        };
        let commitment_bytes = whir::commitment_bytes(opened.commitment())?;
        let proof_bytes = whir::proof_bytes(opened.proof())?;
        let artifacts = checked_artifacts(commitment_bytes, claim_bytes, proof_bytes)?;
        let metadata = RunMetadata {
            backend: Backend::Whir,
            exponent,
            shape_seed,
            trial_seed: seed,
            trial,
            capacity: witness.layout().capacity(),
            gate_vars: witness.layout().gate_vars(),
            witness_digest: digest,
            witness_ms,
            setup_ms,
            security: &security,
            artifacts,
        };
        writer.write_run(&metadata, &intervals)?;
        common::pcs_console::print_trial(
            &trial.id_fragment(),
            &intervals,
            proof_bytes,
            artifacts.total_wire_bytes,
        );
    }
    Ok(CellOutcome::Measured {
        derived_max_pow_bits: Some(summary.derived_max_pow_bits),
    })
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

fn pack_binius_rows(witness: &BabyBearMulWitness) -> Vec<u128> {
    witness
        .a_values()
        .iter()
        .zip(witness.b_values())
        .zip(witness.c_values())
        .zip(witness.k_values())
        .map(|(((a, b), c), k)| {
            u128::from(*a)
                | (u128::from(*b) << 31)
                | (u128::from(*c) << 62)
                | (u128::from(*k) << 93)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn run_binius_series(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &BabyBearMulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<CellOutcome, Box<dyn Error>> {
    let setup_started = Instant::now();
    let log_inv_rate = binius_log_inv_rate()?;
    let backend = BiniusBackend::setup(exponent, log_inv_rate);
    let setup_ms = common::elapsed_ms(setup_started);
    let security = binius_security(&backend);
    eprintln!(
        "  Binius64 backend_setup_ms={setup_ms:.3} (rate 1/{}, {} queries)",
        1usize << backend.log_inv_rate(),
        backend.n_test_queries()
    );

    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Binius, trial);
        clear_profile();
        let output = backend.run_trial(|| pack_binius_rows(witness), seed)?;
        let intervals = finish_profile();
        let artifacts = checked_artifacts(
            output.commitment_bytes,
            output.public_claim_bytes,
            output.opening_proof_bytes(),
        )?;
        if artifacts.total_wire_bytes != output.total_wire_bytes() {
            return Err("Binius wire-size accounting mismatch".into());
        }
        let metadata = RunMetadata {
            backend: Backend::Binius,
            exponent,
            shape_seed,
            trial_seed: seed,
            trial,
            capacity: witness.layout().capacity(),
            gate_vars: witness.layout().gate_vars(),
            witness_digest: digest,
            witness_ms,
            setup_ms,
            security: &security,
            artifacts,
        };
        writer.write_run(&metadata, &intervals)?;
        common::pcs_console::print_trial(
            &trial.id_fragment(),
            &intervals,
            output.proof_bytes,
            artifacts.total_wire_bytes,
        );
    }
    Ok(CellOutcome::Measured {
        derived_max_pow_bits: None,
    })
}

fn ligerito_security(backend: &LigeritoBackend) -> Value {
    json!({
        "profile": "F2Z opener: Round 0 + ring switch + Johnson-regime Ligerito",
        "target_bits": ligerito::SECURITY_BITS,
        "soundness_bound_model": "opener union bound (Round 0, ring switch, every Ligerito level's proximity folds, queries and OOD samples) at the pinned flock constants",
        "achieved_bits": backend.soundness_bits(),
        "ligerito_component_bits": backend.component_bits(),
        "challenge_field": "GF(2^128)-GHASH",
        "hash": "BLAKE3",
        "log_inverse_rate": backend.log_inv_rate(),
        "level0_queries": backend.n_test_queries(),
        "level0_query_grinding_bits": backend.level0_query_grinding_bits(),
        "level0_fold_grinding_bits": backend.level0_fold_grinding_bits(),
        "ood_grinding_bits": backend.ood_grinding_bits(),
        "hiding": false,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_ligerito_series(
    writer: &mut TraceWriter,
    exponent: usize,
    shape_seed: u64,
    witness: &BabyBearMulWitness,
    digest: &str,
    witness_ms: f64,
    reps: usize,
) -> Result<CellOutcome, Box<dyn Error>> {
    let setup_started = Instant::now();
    let backend = LigeritoBackend::setup(exponent)?;
    let setup_ms = common::elapsed_ms(setup_started);
    let security = ligerito_security(&backend);
    eprintln!(
        "  F2Z Ligerito backend_setup_ms={setup_ms:.3} (rate 1/{}, {} level-0 queries, component {} bits, {:.2} bits achieved)",
        1usize << backend.log_inv_rate(),
        backend.n_test_queries(),
        backend.component_bits(),
        backend.soundness_bits()
    );

    for trial in std::iter::once(Trial::Warmup).chain((0..reps).map(Trial::Sample)) {
        let seed = trial_seed(shape_seed, Backend::Ligerito, trial);
        clear_profile();
        let output = backend.run_trial(|| pack_binius_rows(witness), seed)?;
        let intervals = finish_profile();
        let artifacts = checked_artifacts(
            output.commitment_bytes,
            output.public_claim_bytes,
            output.opening_proof_bytes(),
        )?;
        if artifacts.total_wire_bytes != output.total_wire_bytes() {
            return Err("Ligerito wire-size accounting mismatch".into());
        }
        let metadata = RunMetadata {
            backend: Backend::Ligerito,
            exponent,
            shape_seed,
            trial_seed: seed,
            trial,
            capacity: witness.layout().capacity(),
            gate_vars: witness.layout().gate_vars(),
            witness_digest: digest,
            witness_ms,
            setup_ms,
            security: &security,
            artifacts,
        };
        writer.write_run(&metadata, &intervals)?;
        common::pcs_console::print_trial(
            &trial.id_fragment(),
            &intervals,
            output.proof_bytes,
            artifacts.total_wire_bytes,
        );
    }
    Ok(CellOutcome::Measured {
        derived_max_pow_bits: None,
    })
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

fn main() -> Result<(), Box<dyn Error>> {
    // SAFETY: benchmark startup is single-threaded and this is set before the
    // Rayon pool or any profiler scope exists.
    unsafe { std::env::set_var("OBLONG_PROFILE_INTERVALS", "1") };
    let threads = common::init();
    let reps = common::reps(None, DEFAULT_REPS);
    let root_seed = common::seed(None, DEFAULT_SEED);
    let selected = selected_backends();
    let exponents = exponents();
    let campaign_id = campaign_id();
    let campaign_writer = CampaignWriter::from_env();
    let mut campaign_cells = Vec::new();
    let mut writer = TraceWriter::from_env(threads, &campaign_id)?;

    eprintln!("BabyBear terminal-claim PCS comparison");
    eprintln!(
        "  shapes: {}; samples: {} + 1 warmup; threads: {}; seed: {root_seed:#018x}",
        exponents
            .iter()
            .map(|exponent| format!("2^{exponent}"))
            .collect::<Vec<_>>()
            .join(", "),
        reps,
        threads,
    );
    eprintln!(
        "  backends: {}",
        selected
            .iter()
            .map(|backend| backend.display())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if selected.contains(&Backend::Whir) {
        eprintln!(
            "  WHIR: BabyBear extension degree {}, {}, target {} bits, max PoW {} bits, folding auto-tuned per shape",
            whir::CHALLENGE_EXTENSION_DEGREE,
            whir::SECURITY_ASSUMPTION_LABEL,
            whir::SECURITY_BITS,
            whir::MAX_POW_BITS,
        );
    }

    common::pcs_console::print_timing_definitions();
    for &exponent in &exponents {
        for backend in [Backend::F2z, Backend::Whir, Backend::Binius] {
            if !selected.contains(&backend) {
                campaign_cells.push(CampaignCell::not_requested(
                    backend,
                    exponent,
                    "backend omitted by F2Z_PCS_COMPARE_BACKENDS",
                ));
            }
        }
        flock_core::scratch::clear();
        let multiplications = 1usize << exponent;
        let shape_seed = common::mul_witness::shape_seed(root_seed, exponent);
        let mut rng = StdRng::seed_from_u64(shape_seed);
        let witness_started = Instant::now();
        let witness = BabyBearMulWitness::from_fn(multiplications, |_| {
            let a = sample_baby_bear_operand_with(|| rng.random::<u32>());
            let b = sample_baby_bear_operand_with(|| rng.random::<u32>());
            (a, b)
        })?;
        let witness_ms = common::elapsed_ms(witness_started);
        let digest = witness_digest(&witness);
        eprintln!();
        eprintln!(
            "2^{exponent} multiplications: shared_witness_generation_ms={witness_ms:.3}, digest {}",
            &digest[..16]
        );

        for backend in ordered_backends(&selected, exponent) {
            let outcome = match backend {
                Backend::F2z => run_f2z_series(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Whir => run_whir_series(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Binius => run_binius_series(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
                Backend::Ligerito => run_ligerito_series(
                    &mut writer,
                    exponent,
                    shape_seed,
                    &witness,
                    &digest,
                    witness_ms,
                    reps,
                )?,
            };
            campaign_cells.push(outcome.record(backend, exponent));
        }
        drop(witness);
        flock_core::scratch::clear();
    }

    drop(writer);
    campaign_writer.write(
        &campaign_id,
        root_seed,
        reps,
        &exponents,
        &selected,
        campaign_cells,
    )?;

    Ok(())
}

#[cfg(test)]
mod reporting_tests {
    use super::*;
    #[test]
    fn campaign_statuses_preserve_nulls_and_variant_fields() {
        let measured = CellOutcome::Measured {
            derived_max_pow_bits: None,
        }
        .record(Backend::F2z, 4);
        let value = serde_json::to_value(measured).unwrap();
        assert_eq!(
            value,
            json!({
                "implementation":Backend::F2z.implementation(),"log_multiplications":4,
                "challenge_extension_degree":null,"configured_max_pow_bits":null,
                "derived_max_pow_bits":null,"status":"measured"
            })
        );
        let unavailable = CellOutcome::Unavailable {
            reason: "budget".into(),
            required_pow_bits: 20,
            budget: 12,
        }
        .record(Backend::Whir, 5);
        let value = serde_json::to_value(unavailable).unwrap();
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["required_pow_bits"], 20);
        assert_eq!(value["budget"], 12);
        let absent =
            serde_json::to_value(CampaignCell::not_requested(Backend::F2z, 4, "shape")).unwrap();
        assert_eq!(absent["status"], "not_requested");
        assert_eq!(absent["reason"], "shape");
        assert!(absent.get("budget").is_none());
    }
}
