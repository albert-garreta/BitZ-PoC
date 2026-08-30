//! End-to-end benchmark for independent SHA-256 compressions through the
//! repeated Spartan relation and virtual F2Z opening. This exercises the
//! runtime-prime paper path: commit before q, transcript-derived 112/113-bit prime,
//! exact-integer projection, and per-round Spartan grinding.
//!
//! Output follows the unified schema (`docs/bench-schema.md`): the
//! end-to-end prover (`prove_ms`) covers bit packing, commitment, the prime
//! draw + grinding, Spartan, bitification, and the PCS opening. Witness
//! synthesis and public relation construction are excluded and reported
//! separately. Every measured proof is verified.
//!
//! Defaults to the complete paper range `2^7, ..., 2^16` with three measured
//! repetitions after one warm-up. Override with, for example:
//!
//! ```text
//! F2Z_BENCH_SHAPES="10 12" F2Z_BENCH_REPS=1 \
//!   cargo bench --bench sha256_compressions --features unchecked
//! ```
//!
//! (`F2Z_SHA_LOG2S` / `F2Z_SHA_REPS` / `F2Z_SHA_SEED` are deprecated
//! aliases.) Set `F2Z_SHA_TRACE_PATH=/path/to/trace.jsonl` together with
//! `OBLONG_PROFILE_INTERVALS=1` to emit one canonical `zkperf.trace/v1` run per
//! warm-up/sample, including observed nested profiler intervals.

mod common;

use std::{
    collections::HashMap,
    fs::{self, File},
    hint::black_box,
    io::{BufWriter, Write},
    path::Path,
    process::Command,
    time::Instant,
};

use f2z::{
    f2map::VirtualMap,
    piop::spartan::{
        commit_sha256_paper128_witness_with_config, generate_sha256_compression_witnesses_exact,
        prepare_sha256_compression_batch_integer, prove_sha256_compressions_paper128_with_config,
        sha256_compression_configs, verify_sha256_compressions_paper128_with_config,
        PreparedSha256CompressionBatch, Sha256CompressionInput, Sha256CompressionStatement,
        Sha256PrimeProfile, SpartanField, SHA256_COMMITMENT_FIELD_BITS, SHA256_CONSTRAINTS,
        SHA256_CONSTRAINT_STRIDE, SHA256_F_BAR_LIVE_BITS, SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS,
        SHA256_H_STRIDE, SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS,
    },
    transcript::Blake3Transcript,
    utils::prof::ProfileInterval,
};
use serde_json::{json, Value};

/// One rep's raw measurements; step extraction happens in `common`.
struct RepTiming {
    witness_ms: f64,
    commit_ms: f64,
    prove_ms: f64,
    verify_ms: f64,
    prove_phases: Vec<(&'static str, f64)>,
    verify_phases: Vec<(&'static str, f64)>,
    spartan_bytes: usize,
    f2z_bytes: usize,
}

#[derive(Clone, Copy)]
enum Trial {
    Warmup(usize),
    Sample(usize),
}

impl Trial {
    fn id_fragment(self) -> String {
        match self {
            Self::Warmup(index) => format!("warmup-{index}"),
            Self::Sample(index) => format!("sample-{index}"),
        }
    }

    fn json(self) -> Value {
        match self {
            Self::Warmup(index) => json!({"kind": "warmup", "warmup_index": index}),
            Self::Sample(index) => json!({"kind": "sample", "sample_index": index}),
        }
    }
}

struct TraceWriter {
    output: BufWriter<File>,
    git_rev: String,
    git_dirty: bool,
    build_profile: String,
    cpu: String,
    threads: usize,
}

impl TraceWriter {
    fn from_env(threads: usize) -> Option<Self> {
        let path = std::env::var_os("F2Z_SHA_TRACE_PATH")?;
        assert!(
            std::env::var_os("OBLONG_PROFILE_INTERVALS").is_some(),
            "F2Z_SHA_TRACE_PATH requires OBLONG_PROFILE_INTERVALS=1"
        );
        let path = Path::new(&path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).expect("create SHA trace directory");
        }
        let output = BufWriter::new(File::create(path).expect("create SHA trace JSONL"));
        let git_rev = std::env::var("F2Z_SHA_GIT_REV").unwrap_or_else(|_| {
            command_output("git", &["rev-parse", "--short", "HEAD"], "unknown")
        });
        let git_dirty = Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=no"])
            .output()
            .map_or(true, |output| {
                !output.status.success() || !output.stdout.is_empty()
            });
        let cpu = std::env::var("F2Z_SHA_CPU").unwrap_or_else(|_| {
            command_output(
                "sysctl",
                &["-n", "machdep.cpu.brand_string"],
                "Apple Silicon",
            )
        });
        let build_profile =
            std::env::var("F2Z_SHA_BUILD_PROFILE").unwrap_or_else(|_| "bench".to_owned());
        Some(Self {
            output,
            git_rev,
            git_dirty,
            build_profile,
            cpu,
            threads,
        })
    }

    fn write_run(
        &mut self,
        exponent: usize,
        shape_seed: u64,
        trial: Trial,
        intervals: &[ProfileInterval],
    ) {
        let roots = intervals
            .iter()
            .filter(|interval| interval.parent_order.is_none())
            .collect::<Vec<_>>();
        assert_eq!(
            roots.len(),
            1,
            "a traced benchmark run has exactly one root"
        );
        assert_eq!(roots[0].label, "sha256-trace:verified_trial");

        let compressions = 1usize << exponent;
        let profile = Sha256PrimeProfile::new(exponent).expect("paper SHA exponent");
        let trial_fragment = trial.id_fragment();
        let run_id = format!("sha256-paper128-2p{exponent}-{trial_fragment}");
        let series_id = format!(
            "sha256-paper128-2p{exponent}-{}-{}t-{}",
            self.git_rev, self.threads, self.build_profile
        );
        let clock_id = format!("mono-process-{}-{run_id}", std::process::id());
        let root_span_id = span_id(roots[0].order);
        let run = json!({
            "schema": "zkperf.trace/v1",
            "record": "run",
            "run_id": run_id,
            "series_id": series_id,
            "root_span_id": root_span_id,
            "benchmark": {
                "suite": "f2z-pcs",
                "name": "sha256-compressions-paper128",
                "label": format!("2^{exponent} SHA-256 compressions"),
                "algorithm": "SHA-256 compression / Spartan + virtual F2Z (Paper128 runtime prime)",
                "implementation": "f2z runtime-prime Paper128",
                "git_rev": self.git_rev,
                "git_dirty": self.git_dirty,
                "build_profile": self.build_profile,
            },
            "trial": trial.json(),
            "clock": {
                "id": clock_id,
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
                "thread_policy": "Rayon pool sized to performance-core count; no affinity pinning",
            },
            "parameters": {
                "input": {
                    "sha256_compressions": compressions,
                    "sha256_internal_rounds": 64usize * compressions,
                    "witness_bits": SHA256_F_BAR_LIVE_BITS * compressions,
                    "num_rows": SHA256_CONSTRAINT_STRIDE * compressions,
                    "num_cols": SHA256_H_STRIDE * compressions,
                    "constraints": SHA256_CONSTRAINTS * compressions,
                    "live_source_bits": SHA256_F_BAR_LIVE_BITS * compressions,
                    "padded_source_cells": SHA256_F_STRIDE * compressions,
                    "live_assignment_values": SHA256_H_BAR_LIVE_BITS * compressions,
                    "padded_assignment_cells": SHA256_H_STRIDE * compressions,
                    "conceptual_map_nonzeros": 42_361usize * compressions,
                    "conceptual_c_nonzeros": 54_120usize * compressions,
                },
                "security": {
                    "target_bits": 128,
                    "transcript_hash": "BLAKE3",
                    "commitment_field": format!("GF(2^{SHA256_COMMITMENT_FIELD_BITS})"),
                    "prime_min": profile.min_prime().to_string(),
                    "prime_max": profile.max_prime().to_string(),
                    "prime_bits": if exponent == SHA256_MAX_LOG_COMPRESSIONS {112} else {113},
                    "initial_grinding_bits": profile.initial_grinding_bits(),
                    "outer_round_grinding_bits": profile.outer_round_grinding_bits(),
                    "terminal_grinding_bits": profile.terminal_grinding_bits(),
                },
                "recursion": {"max_depth": 0, "instance_count": 1},
                "repetition": {"count": 1},
                "seed": format!("{shape_seed:#018x}"),
            },
            "tags": {
                "root_boundary": "verified trial: prover plus verification; setup and input generation excluded",
                "timeline": "observed half-open intervals",
                "f2z_rs_fast": env_setting("F2Z_RS_FAST", "default:on"),
                "f2z_foldv_lut": env_setting("F2Z_FOLDV_LUT", "default:on"),
                "f2_forest_schedule": env_setting("F2_FOREST_SCHEDULE", "default:l4"),
                "f2z_flat_forest": env_setting("F2Z_FLAT_FOREST", "default:shape-dependent"),
                "f2z_t4_factored": env_setting("F2Z_T4_FACTORED", "default:schedule-dependent"),
                "f2z_jit_r1": env_setting("F2Z_JIT_R1", "default:on"),
                "f2z_jit_grid": env_setting("F2Z_JIT_GRID", "default:on"),
                "f2z_t4_prfm": env_setting("F2Z_T4_PRFM", "default:shape-dependent"),
                "f2z_lut3": env_setting("F2Z_LUT3", "default:on"),
                "f2z_lut4": env_setting("F2Z_LUT4", "default:off"),
                "f2z_col_elide": env_setting("F2Z_COL_ELIDE", "default:on"),
                "f2z_quad": env_setting("F2Z_QUAD", "default:off"),
            },
        });
        serde_json::to_writer(&mut self.output, &run).expect("write SHA trace run");
        writeln!(self.output).expect("terminate SHA trace run");

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
            let key = (interval.parent_order, interval.label);
            let occurrence_index = seen.entry(key).or_default();
            let occurrence_count = totals[&key];
            let descriptor = describe_span(interval, &by_order);
            let coordinate = if interval.label.starts_with("spartan:round_grinding_") {
                json!({
                    "round_index": *occurrence_index,
                    "round_count": occurrence_count,
                })
            } else if occurrence_count > 1 {
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
            });
            if let Some(scope_tag) = descriptor.scope_tag {
                attributes["scope_tag"] = json!(scope_tag);
            }
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
            serde_json::to_writer(&mut self.output, &span).expect("write SHA trace span");
            writeln!(self.output).expect("terminate SHA trace span");
        }
        self.output.flush().expect("flush SHA trace JSONL");
    }
}

struct SpanDescriptor {
    operation: String,
    name: String,
    short_name: String,
    primary_phase: &'static str,
    phase_tags: Vec<&'static str>,
    scope_kind: &'static str,
    scope_tag: Option<&'static str>,
    primary_sequence: bool,
    math_latex: Vec<&'static str>,
}

fn describe_span(
    interval: &ProfileInterval,
    by_order: &HashMap<u64, &ProfileInterval>,
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
    let has_fragment = |fragment: &str| labels.iter().any(|label| label.contains(fragment));
    let root = interval.parent_order.is_none();
    let verifying = under("sha256-trace:verification");
    let witness = under("sha256-trace:witness_generation");
    let committing = under("sha256-trace:commit");
    let opening_prepare = has_fragment("opening_prepare_");
    let f2z_opening = has_fragment("f2z_prove") || has_fragment("f2z_verify");
    let spartan = has_fragment("spartan_outer_");
    let sumcheck = under("sha256-paper128:spartan_outer_prove")
        || under("eqf:rounds")
        || under("mc:presum_run");
    let in_eq_factored = labels.iter().any(|label| label.starts_with("eqf:"));
    let fri = !in_eq_factored && f2z_opening && (under("mc:forest") || under("mc:fold_v"));

    let primary_phase = if root {
        "end-to-end"
    } else if verifying {
        "verification"
    } else if witness {
        "witness-generation"
    } else if committing {
        "commit"
    } else if opening_prepare {
        "preparation"
    } else if sumcheck {
        "sumcheck"
    } else if spartan {
        "constraint-proof"
    } else if f2z_opening {
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
    if !verifying {
        if committing {
            push_tag(&mut phase_tags, "commit");
            push_tag(&mut phase_tags, "pcs");
        }
        if opening_prepare {
            push_tag(&mut phase_tags, "preparation");
            push_tag(&mut phase_tags, "opening-proof");
            push_tag(&mut phase_tags, "pcs");
        }
        if f2z_opening {
            push_tag(&mut phase_tags, "opening-proof");
            push_tag(&mut phase_tags, "pcs");
        }
        if spartan {
            push_tag(&mut phase_tags, "constraint-proof");
        }
        if sumcheck {
            push_tag(&mut phase_tags, "sumcheck");
        }
        if fri {
            push_tag(&mut phase_tags, "fri");
        }
    }

    let (name, short_name) = span_names(interval.label);
    let scope_kind = match interval.label {
        "sha256-trace:verified_trial" => "scope",
        "sha256-trace:end_to_end_prove"
        | "sha256-trace:witness_generation"
        | "sha256-trace:statement_materialization"
        | "sha256-trace:commit"
        | "sha256-trace:proof"
        | "sha256-trace:verification"
        | "sha256-paper128:spartan_outer_prove"
        | "sha256-paper128:opening_prepare_prover"
        | "sha256-paper128:f2z_prove"
        | "sha256-paper128:spartan_outer_verify"
        | "sha256-paper128:opening_prepare_verifier"
        | "sha256-paper128:f2z_verify" => "phase",
        "spartan:round_grinding_prove" | "spartan:round_grinding_verify" => "round",
        _ => "procedure",
    };
    let scope_tag = match interval.label {
        "sha256-trace:verified_trial" => Some("end-to-end"),
        "sha256-trace:end_to_end_prove" => Some("proving"),
        "sha256-trace:witness_generation" => Some("witness-generation"),
        "sha256-trace:commit" => Some("commit"),
        "sha256-trace:verification" => Some("verification"),
        "sha256-paper128:spartan_outer_prove" => Some("constraint-proof"),
        "sha256-paper128:f2z_prove" => Some("opening-proof"),
        _ => None,
    };
    let primary_sequence = matches!(
        interval.label,
        "sha256-trace:witness_generation"
            | "sha256-trace:statement_materialization"
            | "sha256-trace:commit"
            | "sha256-trace:proof"
            | "sha256-trace:verification"
    );
    SpanDescriptor {
        operation: operation_name(interval.label),
        name,
        short_name,
        primary_phase,
        phase_tags,
        scope_kind,
        scope_tag,
        primary_sequence,
        math_latex: span_math(interval.label),
    }
}

fn span_names(label: &str) -> (String, String) {
    let known = match label {
        "sha256-trace:verified_trial" => Some(("Complete verified trial", "Verified trial")),
        "sha256-trace:end_to_end_prove" => Some(("End-to-end prover", "Prover")),
        "sha256-trace:witness_generation" => Some(("Generate exact SHA-256 witness", "Witness")),
        "sha256-trace:statement_materialization" => {
            Some(("Materialize public SHA-256 statements", "Statement"))
        }
        "sha256-trace:commit" => Some(("Commit to packed Boolean source", "Commit")),
        "sha256-trace:proof" => Some(("Paper128 Spartan and virtual-F2Z proof", "Proof")),
        "sha256-trace:verification" => Some(("Verify Paper128 proof", "Verify")),
        "sha256-paper128:initial_grinding_prove" => {
            Some(("Initial prover grinding", "Initial PoW"))
        }
        "sha256-paper128:runtime_prime_sample_prover"
        | "sha256-paper128:runtime_prime_sample_verifier" => {
            Some(("Sample transcript-derived runtime prime", "Sample q"))
        }
        "sha256-paper128:relation_projection_prover"
        | "sha256-paper128:relation_projection_verifier" => Some((
            "Project exact relation coefficients modulo q",
            "Project relation",
        )),
        "sha256-paper128:product_projection_prover" => Some((
            "Project exact witness products modulo q",
            "Project products",
        )),
        "sha256-paper128:spartan_prepare_prover" => {
            Some(("Prepare equality factors and reducer", "Spartan prep"))
        }
        "sha256-paper128:spartan_outer_prove" => Some(("Outer Spartan sumcheck", "Outer Spartan")),
        "spartan:round_grinding_prove" => Some(("Outer-round prover grinding", "Round PoW")),
        "sha256-paper128:terminal_grinding_prove" => {
            Some(("Terminal prover grinding", "Terminal PoW"))
        }
        "sha256-paper128:opening_prepare_prover" => {
            Some(("Factorize terminal opening claim", "Opening prep"))
        }
        "sha256-paper128:f2z_prove" => Some(("Virtual F2Z opening proof", "Virtual F2Z")),
        "sha256-paper128:spartan_outer_verify" => {
            Some(("Verify outer Spartan sumcheck", "Outer verify"))
        }
        "spartan:round_grinding_verify" => Some(("Check outer-round grinding", "Check PoW")),
        "sha256-paper128:f2z_verify" => Some(("Verify virtual F2Z opening", "F2Z verify")),
        _ => None,
    };
    known.map_or_else(
        || {
            let human = label
                .chars()
                .map(|character| {
                    if matches!(character, ':' | '_') {
                        ' '
                    } else {
                        character
                    }
                })
                .collect::<String>();
            (human.clone(), human)
        },
        |(name, short)| (name.to_owned(), short.to_owned()),
    )
}

fn span_math(label: &str) -> Vec<&'static str> {
    match label {
        "sha256-trace:witness_generation" => vec![
            "\\bar h=M\\bar f",
            "(A\\bar h)\\circ(B\\bar h)=C\\bar h\\text{ over }\\mathbb Z",
        ],
        "sha256-trace:commit" => vec!["C_f=\\operatorname{Com}_{\\mathbb F_{2^{128}}}(\\bar f)"],
        "sha256-paper128:runtime_prime_sample_prover"
        | "sha256-paper128:runtime_prime_sample_verifier" => {
            vec!["q\\leftarrow\\operatorname{PrimeSample}(\\mathsf{tr},I_t)"]
        }
        "sha256-paper128:relation_projection_prover"
        | "sha256-paper128:relation_projection_verifier"
        | "sha256-paper128:product_projection_prover" => {
            vec!["\\mathbb Z\\longrightarrow\\mathbb F_q"]
        }
        "sha256-paper128:spartan_outer_prove" => vec![
            "\\sum_{x\\in\\{0,1\\}^{t+8}}\\operatorname{eq}(\\tau,x)\\bigl(Az(x)Bz(x)-Cz(x)\\bigr)=0",
        ],
        "spartan:round_grinding_prove"
        | "sha256-paper128:initial_grinding_prove"
        | "sha256-paper128:terminal_grinding_prove" => {
            vec!["\\operatorname{lz}(\\operatorname{BLAKE3}(s\\parallel n))\\ge b"]
        }
        "sha256-paper128:opening_prepare_prover" | "sha256-paper128:opening_prepare_verifier" => {
            vec!["\\widetilde h(r)=\\widetilde M(r,\\cdot)\\widetilde f"]
        }
        "sha256-paper128:f2z_prove" | "sha256-paper128:f2z_verify" => {
            vec!["\\widetilde{\\bar h}(r)=v\\text{ from committed }\\bar f"]
        }
        _ => Vec::new(),
    }
}

fn push_tag(tags: &mut Vec<&'static str>, tag: &'static str) {
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

fn operation_name(label: &str) -> String {
    let mut output = String::with_capacity(label.len());
    let mut separator = false;
    for character in label.chars() {
        let mapped = match character {
            'A'..='Z' => character.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' | '-' => character,
            _ => '.',
        };
        if mapped == '.' {
            if separator || output.is_empty() {
                continue;
            }
            separator = true;
        } else {
            separator = false;
        }
        output.push(mapped);
    }
    while output.ends_with('.') {
        output.pop();
    }
    if output.is_empty() {
        "scope".to_owned()
    } else {
        output
    }
}

fn span_id(order: u64) -> String {
    format!("span-{order}")
}

fn env_setting(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
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

fn make_inputs(compressions: usize, seed: u64) -> Vec<Sha256CompressionInput> {
    let mut rng = SplitMix64(seed);
    (0..compressions)
        .map(|_| {
            (
                std::array::from_fn(|_| rng.next_u32()),
                std::array::from_fn(|_| rng.next_u32()),
            )
        })
        .collect()
}

fn exponents() -> Vec<usize> {
    let shapes = common::shapes(Some("F2Z_SHA_LOG2S")).unwrap_or_else(|| {
        "7 8 9 10 11 12 13 14 15 16"
            .split(' ')
            .map(str::to_owned)
            .collect()
    });
    shapes
        .iter()
        .map(|part| {
            let exponent = part
                .parse::<usize>()
                .expect("F2Z_BENCH_SHAPES contains integer exponents");
            assert!(
                (SHA256_MIN_LOG_COMPRESSIONS..=SHA256_MAX_LOG_COMPRESSIONS).contains(&exponent),
                "paper SHA runtime-prime profile supports exponents 7 through 16"
            );
            exponent
        })
        .collect()
}

fn fmt_ms(milliseconds: f64) -> String {
    if milliseconds < 1.0 {
        format!("{:8.2} us", milliseconds * 1e3)
    } else if milliseconds < 1_000.0 {
        format!("{milliseconds:8.2} ms")
    } else {
        format!("{:8.2} s ", milliseconds / 1e3)
    }
}

fn run_once(
    inputs: &[Sha256CompressionInput],
    prepared: &PreparedSha256CompressionBatch,
    pc: &flock_core::pcs::ligerito::ProverConfig,
    vc: &flock_core::pcs::ligerito::VerifierConfig,
) -> (RepTiming, Vec<ProfileInterval>) {
    let _ = f2z::utils::prof::take_totals();
    let _ = f2z::utils::prof::take_intervals();
    let verified_trial_scope = f2z::utils::prof::scope("sha256-trace:verified_trial");

    // Witness synthesis and public-statement materialization are excluded
    // from the prover boundary (docs/bench-schema.md).
    let started = Instant::now();
    let witness = {
        let _scope = f2z::utils::prof::scope("sha256-trace:witness_generation");
        generate_sha256_compression_witnesses_exact(
            inputs,
            prepared.source_params(),
            prepared.assignment_params(),
        )
        .expect("SHA witness synthesis succeeds")
    };
    let statements = {
        let _scope = f2z::utils::prof::scope("sha256-trace:statement_materialization");
        let statements = inputs
            .iter()
            .copied()
            .zip(witness.outputs().iter().copied())
            .map(|(input, output)| Sha256CompressionStatement::new(input, output))
            .collect::<Vec<_>>();
        black_box(&statements);
        statements
    };
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;
    let _ = f2z::utils::prof::take_totals();

    // End-to-end prove: Step 1 commit + the paper128 proof.
    let prover_scope = f2z::utils::prof::scope("sha256-trace:end_to_end_prove");
    let prove_started = Instant::now();
    let started = Instant::now();
    let hint = {
        let _scope = f2z::utils::prof::scope("sha256-trace:commit");
        commit_sha256_paper128_witness_with_config(prepared, &witness, pc)
            .expect("SHA source commitment succeeds")
    };
    let commit_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut prover_transcript = Blake3Transcript::new();
    let proof = {
        let _scope = f2z::utils::prof::scope("sha256-trace:proof");
        prove_sha256_compressions_paper128_with_config(
            &mut prover_transcript,
            prepared,
            &statements,
            &witness,
            &hint,
            pc,
        )
        .expect("SHA proof succeeds")
    };
    drop(prover_scope);
    let prove_ms = prove_started.elapsed().as_secs_f64() * 1e3;
    let prove_phases = f2z::utils::prof::take_totals();

    let mut verifier_transcript = Blake3Transcript::new();
    let started = Instant::now();
    {
        let _scope = f2z::utils::prof::scope("sha256-trace:verification");
        verify_sha256_compressions_paper128_with_config(
            &mut verifier_transcript,
            prepared,
            &statements,
            &hint.commitment,
            &proof,
            vc,
        )
        .expect("SHA proof verifies");
    }
    let verify_ms = started.elapsed().as_secs_f64() * 1e3;
    drop(verified_trial_scope);
    let verify_phases = f2z::utils::prof::take_totals();
    let intervals = f2z::utils::prof::take_intervals();
    black_box(&proof);

    let f2z_bytes = proof.f2z().to_bytes().len();
    let spartan_elements = 4 * proof.outer().sumcheck.round_polynomials.len() + 3;
    let field_bytes = proof
        .outer()
        .az_mle_claim
        .canonical_element_encoding()
        .len();
    let spartan_bytes = spartan_elements * field_bytes + 8 * (proof.outer_nonces().len() + 2);

    let timing = RepTiming {
        witness_ms,
        commit_ms,
        prove_ms,
        verify_ms,
        prove_phases,
        verify_phases,
        spartan_bytes,
        f2z_bytes,
    };
    (timing, intervals)
}

fn bench_exponent(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    threads: usize,
    trace_writer: &mut Option<TraceWriter>,
) {
    let compressions = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("compression count fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);

    let setup_started = Instant::now();
    let prepared = prepare_sha256_compression_batch_integer(exponent).expect("valid SHA relation");
    let (pc, vc) =
        sha256_compression_configs(prepared.source_params()).expect("valid Ligerito config");
    let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;

    println!();
    println!("=== 2^{exponent} = {compressions} independent SHA-256 compressions ===");
    println!(
        "  source: live={} padded={} bits/compression | derived: live={} padded={} bits/compression",
        SHA256_F_BAR_LIVE_BITS, SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS, SHA256_H_STRIDE,
    );
    println!(
        "  R1CS: {} live / {} padded rows per compression | repeated map nnz={} | setup {}",
        f2z::piop::spartan::SHA256_CONSTRAINTS,
        SHA256_CONSTRAINT_STRIDE,
        prepared.map().nnz(),
        fmt_ms(setup_ms),
    );

    let warm_inputs = make_inputs(compressions, shape_seed);
    let (warm, warm_intervals) = run_once(&warm_inputs, &prepared, &pc, &vc);
    if let Some(writer) = trace_writer {
        writer.write_run(exponent, shape_seed, Trial::Warmup(0), &warm_intervals);
    }
    black_box(warm);

    let mut prover = common::StepSamples::default();
    let mut verifier = common::StepSamples::default();
    let mut witness_samples = Vec::with_capacity(reps);
    let mut last = None;
    for sample in 0..reps {
        let input_seed = shape_seed ^ ((sample + 1) as u64).wrapping_mul(0xd6e8_feb8_6659_fd93);
        let inputs = make_inputs(compressions, input_seed);
        let (timing, intervals) = run_once(&inputs, &prepared, &pc, &vc);
        if let Some(writer) = trace_writer {
            writer.write_run(exponent, shape_seed, Trial::Sample(sample), &intervals);
        }
        println!(
            "  SAMPLE exponent={exponent} sample={} compressions={compressions} witness_ms={:.6} commit_ms={:.6} prove_ms={:.6} verify_ms={:.6} verified=true",
            sample + 1,
            timing.witness_ms,
            timing.commit_ms,
            timing.prove_ms,
            timing.verify_ms,
        );
        prover.record_prove(timing.prove_ms, timing.commit_ms, &timing.prove_phases);
        verifier.record_verify(timing.verify_ms, &timing.verify_phases);
        witness_samples.push(timing.witness_ms);
        last = Some(timing);
    }
    let last = last.expect("positive repetition count");

    let prover_medians = prover.medians();
    let throughput = compressions as f64 / (prover_medians.total / 1e3);
    println!(
        "  end-to-end prove: {} median | {throughput:10.0} compressions/s",
        fmt_ms(prover_medians.total),
    );
    let report = common::BenchReport {
        bench: "sha256",
        shape: format!("2p{exponent}"),
        extra: vec![
            ("compressions".into(), compressions.to_string()),
            ("throughput_per_s".into(), format!("{throughput:.3}")),
            ("shape_seed".into(), format!("{shape_seed:#018x}")),
        ],
        lambda: Some(128),
        threads,
        reps,
        seed: Some(root_seed),
        witness_ms: common::median(&witness_samples),
        setup_ms,
        prover: prover_medians,
        verifier: verifier.medians(),
        proof: common::ProofBytes {
            piop: last.spartan_bytes,
            open: last.f2z_bytes,
        },
    };
    report.print_human();
}

fn main() {
    let threads = common::init();
    let mut trace_writer = TraceWriter::from_env(threads);
    let reps = common::reps(Some("F2Z_SHA_REPS"), 3);
    let root_seed = common::seed(Some("F2Z_SHA_SEED"), 0x4632_5a5f_5348_4132);

    println!("SHA-256: synthesized [1|f], h=Mf, Ah/Bh/Ch; repeated outer Spartan + virtual F2Z");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!("repetitions: {reps}; warmups: 1; root seed: {root_seed:#018x}");
    if let Some(path) = std::env::var_os("F2Z_SHA_TRACE_PATH") {
        println!("canonical interval trace: {}", Path::new(&path).display());
    }

    for exponent in exponents() {
        flock_core::scratch::clear();
        bench_exponent(exponent, reps, root_seed, threads, &mut trace_writer);
    }
    flock_core::scratch::clear();
}
