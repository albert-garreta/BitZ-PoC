//! End-to-end benchmark for independent SHA-256 compressions through the
//! repeated Spartan relation and virtual F2Z opening. This exercises the
//! runtime-prime protocol: commit before q, transcript-derived 112/113-bit prime,
//! signed local-matrix collapse, and per-round Spartan grinding.
//!
//! Output follows the unified schema (`docs/bench-schema.md`): the
//! end-to-end prover (`prove_ms`) covers bit packing, commitment, the prime
//! draw + grinding, Spartan, bitification, and the PCS opening. Witness
//! synthesis and public relation construction are excluded and reported
//! separately. Every measured proof is verified.
//!
//! Defaults to `2^7, ..., 2^16` with three measured
//! repetitions after one warm-up. Override with, for example:
//!
//! ```text
//! F2Z_BENCH_SHAPES="10 12" F2Z_BENCH_REPS=1 \
//!   cargo bench --bench sha256_compressions --features unchecked
//! ```
//!
//! To size the batch by the packed assignment domain instead, set
//! `F2Z_SHA_MNUMROWS_LOG2S`. For example, `F2Z_SHA_MNUMROWS_LOG2S="24 25"`
//! benchmarks the largest batch fitting in `MnumRows = 2^24` and `2^25`.
//! Every compression occupies 20,456 adjacent assignment cells, all batches
//! share one leading constant cell, and any unused cells are one trailing
//! zero suffix.
//! Power-of-two compression batches of at least 128 instances open the
//! product-layout assignment directly. Batches of 16, 32 and 64 use the
//! packed inner sumcheck. `F2Z_SHA_INNER_PREFIX_VARS=0..4` configures that
//! path and non-power-of-two assignment-row batches.
//! `F2Z_SHA_OPENING_T=<t>` gives every compression-count shape an explicit
//! F2Z split of `2^t` rows (the read-off vector then has `2^(vars - t)`
//! columns; splits above the one-forest cap open with one forest per weight
//! chunk). `F2Z_SHA_OPENING_LAYOUT=inner` (default) takes the inner-sumcheck
//! path; `=product` keeps the direct product opening on the transposed,
//! instance-major product tensor (`t >= 15`).
//!
//! `F2Z_BENCH_LAMBDA=100|128|sha128-reference-schedule` selects the security
//! profile the run measures at (default `Lambda100`; the two-prime
//! `Limber114` profile is MultiSwap-only and is rejected here).
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
        IopSecurityProfile, PreparedSha256CompressionBatch, PrimePolicy,
        SHA256_COMMITMENT_FIELD_BITS, SHA256_CONSTRAINTS, SHA256_DEFAULT_INNER_PREFIX_VARS,
        SHA256_F_INSTANCE_BITS, SHA256_H_INSTANCE_BITS, SHA256_INNER_PREFIX_MAX_VARS,
        SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS, Sha256CompressionInput,
        Sha256CompressionStatement, Sha256ConstraintError, Sha256OpeningLayout, SpartanField,
        commit_sha256_compression_witness_with_config, generate_sha256_compression_witnesses,
        prepare_sha256_compression_batch_for_assignment_rows_with_profile,
        prepare_sha256_compression_batch_with_profile_and_layout,
        prove_sha256_compressions_with_prefix_vars_and_config, sha256_compression_configs,
        verify_sha256_compressions_with_config,
    },
    transcript::Blake3Transcript,
    utils::prof::ProfileInterval,
};
use serde_json::{Value, json};

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
    forests: usize,
}

#[derive(Clone, Copy)]
enum Trial {
    Warmup(usize),
    Sample(usize),
}

#[derive(Clone, Copy)]
enum BenchShape {
    Compressions(usize),
    AssignmentRows(usize),
}

impl BenchShape {
    const fn exponent(self) -> usize {
        match self {
            Self::Compressions(exponent) | Self::AssignmentRows(exponent) => exponent,
        }
    }

    const fn mode(self) -> &'static str {
        match self {
            Self::Compressions(_) => "compressions",
            Self::AssignmentRows(_) => "mnumrows",
        }
    }

    fn slug(self) -> String {
        format!("{}-2p{}", self.mode(), self.exponent())
    }

    /// Public relation preparation under the selected security profile.
    fn prepare<P: IopSecurityProfile>(
        self,
        layout: Sha256OpeningLayout,
    ) -> Result<PreparedSha256CompressionBatch, Sha256ConstraintError> {
        match self {
            Self::Compressions(exponent) => {
                prepare_sha256_compression_batch_with_profile_and_layout::<P>(exponent, layout)
            }
            Self::AssignmentRows(exponent) => {
                assert!(
                    layout == Sha256OpeningLayout::Default,
                    "F2Z_SHA_OPENING_T applies to compression-count shapes only"
                );
                prepare_sha256_compression_batch_for_assignment_rows_with_profile::<P>(exponent)
            }
        }
    }
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
        shape: BenchShape,
        shape_seed: u64,
        prepared: &PreparedSha256CompressionBatch,
        inner_prefix_vars: usize,
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

        let exponent = shape.exponent();
        let compressions = prepared.instances();
        let security = prepared.security();
        let live_source_cells = 1 + SHA256_F_INSTANCE_BITS * compressions;
        let live_assignment_cells = 1 + SHA256_H_INSTANCE_BITS * compressions;
        let assignment_rows = prepared.assignment_params().cells();
        let live_constraint_rows = SHA256_CONSTRAINTS * compressions;
        let constraint_rows = live_constraint_rows.next_power_of_two();
        let shape_slug = shape.slug();
        let trial_fragment = trial.id_fragment();
        let run_id = format!("sha256-{shape_slug}-{trial_fragment}");
        let series_id = format!(
            "sha256-{shape_slug}-{}-{}t-{}",
            self.git_rev, self.threads, self.build_profile,
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
                "name": "sha256-compressions",
                "label": match shape {
                    BenchShape::Compressions(_) => {
                        format!("2^{exponent} SHA-256 compressions")
                    }
                    BenchShape::AssignmentRows(_) => {
                        format!("MnumRows=2^{exponent}; {compressions} SHA-256 compressions")
                    }
                },
                "algorithm": "SHA-256 compression / Spartan + virtual F2Z (runtime prime)",
                "implementation": "f2z runtime-prime SHA-256",
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
                    "shape_mode": shape.mode(),
                    "shape_exponent": exponent,
                    "sha256_compressions": compressions,
                    "sha256_internal_rounds": 64usize * compressions,
                    "witness_bits": live_source_cells,
                    "mnum_rows": assignment_rows,
                    "num_rows": constraint_rows,
                    "num_cols": assignment_rows,
                    "constraints": live_constraint_rows,
                    "trailing_constraint_padding": constraint_rows - live_constraint_rows,
                    "inner_prefix_vars": inner_prefix_vars,
                    "live_source_bits": live_source_cells,
                    "padded_source_cells": prepared.source_params().cells(),
                    "live_assignment_values": live_assignment_cells,
                    "padded_assignment_cells": assignment_rows,
                    "trailing_assignment_padding": assignment_rows - live_assignment_cells,
                    "conceptual_map_nonzeros": prepared.map().nnz(),
                    "conceptual_c_nonzeros": 54_120usize * compressions,
                },
                "security": {
                    "profile": security.profile_name,
                    "target_bits": security.lambda,
                    "achieved_bits": security.accounting.achieved_bits(),
                    "binding_term": security.accounting.binding_term().name,
                    "transcript_hash": "BLAKE3",
                    "commitment_field": format!("GF(2^{SHA256_COMMITMENT_FIELD_BITS})"),
                    "prime_min": security.projection_min.to_string(),
                    "prime_max": security.projection_max.to_string(),
                    "prime_bits": if prepared.log_instance_capacity()
                        == SHA256_MAX_LOG_COMPRESSIONS {112} else {113},
                    "initial_grinding_bits": security.initial_grinding_bits,
                    "inner_round_grinding_bits": security.piop_round_grinding_bits,
                    "terminal_grinding_bits": security.terminal_grinding_bits,
                    "forest_round_grinding_bits": security.forest_round_grinding_bits,
                    "ligerito_target_bits": security.ligerito_target_bits,
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
    let linear_reducer_init = has_fragment("reducer_init_");
    let local_relation_collapse = has_fragment("local_relation_collapse_");
    let product_batch_prepare = has_fragment("product_batch_prepare_");
    let inner_sumcheck = has_fragment("spartan_inner_") || under("spartan:inner_sumcheck");
    let spartan = inner_sumcheck || local_relation_collapse || product_batch_prepare;
    let sumcheck = inner_sumcheck || under("eqf:rounds") || under("mc:presum_run");
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
    } else if opening_prepare || linear_reducer_init || product_batch_prepare {
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
        if linear_reducer_init || product_batch_prepare {
            push_tag(&mut phase_tags, "preparation");
            push_tag(&mut phase_tags, "constraint-proof");
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
        | "sha256:reducer_init_prover"
        | "sha256:reducer_init_verifier"
        | "sha256:local_relation_collapse_prover"
        | "sha256:local_relation_collapse_verifier"
        | "sha256:product_batch_prepare_prover"
        | "sha256:product_batch_prepare_verifier"
        | "sha256:spartan_inner_prove"
        | "sha256:opening_prepare_prover"
        | "sha256:f2z_prove"
        | "sha256:spartan_inner_verify"
        | "sha256:opening_prepare_verifier"
        | "sha256:f2z_verify" => "phase",
        "spartan:round_grinding_prove" | "spartan:round_grinding_verify" => "round",
        _ => "procedure",
    };
    let scope_tag = match interval.label {
        "sha256-trace:verified_trial" => Some("end-to-end"),
        "sha256-trace:end_to_end_prove" => Some("proving"),
        "sha256-trace:witness_generation" => Some("witness-generation"),
        "sha256-trace:commit" => Some("commit"),
        "sha256-trace:verification" => Some("verification"),
        "sha256:reducer_init_prover" | "sha256:product_batch_prepare_prover" => Some("preparation"),
        "sha256:local_relation_collapse_prover" => Some("constraint-proof"),
        "sha256:spartan_inner_prove" => Some("constraint-proof"),
        "sha256:f2z_prove" => Some("opening-proof"),
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
        "sha256-trace:proof" => Some(("Spartan and virtual-F2Z proof", "Proof")),
        "sha256-trace:verification" => Some(("Verify SHA-256 proof", "Verify")),
        "sha256:initial_grinding_prove" => Some(("Initial prover grinding", "Initial PoW")),
        "sha256:runtime_prime_sample_prover" | "sha256:runtime_prime_sample_verifier" => {
            Some(("Sample transcript-derived runtime prime", "Sample q"))
        }
        "step2:project_prove" | "step2:project_verify" => {
            Some(("Runtime-field setup and relation binding", "Field setup"))
        }
        "sha256:reducer_init_prover" | "sha256:reducer_init_verifier" => {
            Some(("Initialize the SHA-256 linear reducer", "Reducer init"))
        }
        "sha256:local_relation_collapse_prover" | "sha256:local_relation_collapse_verifier" => {
            Some((
                "Collapse local SHA-256 relation columns",
                "Local Cᵀ collapse",
            ))
        }
        "sha256:product_batch_prepare_prover" | "sha256:product_batch_prepare_verifier" => Some((
            "Prepare factored instance and column batch",
            "Product batch",
        )),
        "sha256:spartan_inner_prove" => Some(("Prove quadratic inner sumcheck", "Inner sumcheck")),
        "spartan:round_grinding_prove" => Some(("Inner-round prover grinding", "Round PoW")),
        "sha256:public_batch_grinding_prove" => {
            Some(("Public-batch prover grinding", "Public-batch PoW"))
        }
        "sha256:opening_prepare_prover" => {
            Some(("Factorize terminal opening claim", "Opening prep"))
        }
        "sha256:f2z_prove" => Some(("Virtual F2Z opening proof", "Virtual F2Z")),
        "sha256:spartan_inner_verify" => Some(("Verify quadratic inner sumcheck", "Inner verify")),
        "spartan:round_grinding_verify" => Some(("Check inner-round grinding", "Check PoW")),
        "sha256:f2z_verify" => Some(("Verify virtual F2Z opening", "F2Z verify")),
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
        "sha256-trace:witness_generation" => {
            vec!["\\bar h=M\\bar f", "C\\bar h=0\\text{ over }\\mathbb Z"]
        }
        "sha256-trace:commit" => vec!["C_f=\\operatorname{Com}_{\\mathbb F_{2^{128}}}(\\bar f)"],
        "sha256:runtime_prime_sample_prover" | "sha256:runtime_prime_sample_verifier" => {
            vec!["q\\leftarrow\\operatorname{PrimeSample}(\\mathsf{tr},I_t)"]
        }
        "sha256:reducer_init_prover" | "sha256:reducer_init_verifier" => Vec::new(),
        "sha256:local_relation_collapse_prover" | "sha256:local_relation_collapse_verifier" => {
            vec!["\\beta_c=\\sum_{r=0}^{183}\\operatorname{eq}(r,\\xi)C[r,c]"]
        }
        "sha256:product_batch_prepare_prover" | "sha256:product_batch_prepare_verifier" => {
            vec![
                "d_c=\\beta_c+\\alpha_0\\mathbf 1[c=0]+\\alpha_{\\mathrm{pub}}\\sum_{p:c_p=c}\\lambda_p",
                "V(i,c)=u_i d_c",
            ]
        }
        "sha256:spartan_inner_prove" | "sha256:spartan_inner_verify" => {
            vec![
                "\\sum_{y\\in\\{0,1\\}^{t_h+s_h}}\\widetilde V_{\\mathrm{total}}(y)\\,\\widetilde{\\bar h}(y)=c_{\\mathrm{public}}",
            ]
        }
        "spartan:round_grinding_prove"
        | "sha256:initial_grinding_prove"
        | "sha256:public_batch_grinding_prove" => {
            vec!["\\operatorname{lz}(\\operatorname{BLAKE3}(s\\parallel n))\\ge b"]
        }
        "sha256:opening_prepare_prover" | "sha256:opening_prepare_verifier" => {
            vec!["\\widetilde h(r)=\\widetilde M(r,\\cdot)\\widetilde f"]
        }
        "sha256:f2z_prove" | "sha256:f2z_verify" => {
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

fn parse_exponent_list(value: &str, variable: &str) -> Vec<usize> {
    let exponents = value
        .split([',', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<usize>()
                .unwrap_or_else(|_| panic!("{variable} contains non-integer exponents"))
        })
        .collect::<Vec<_>>();
    assert!(!exponents.is_empty(), "{variable} must not be empty");
    exponents
}

fn shapes() -> Vec<BenchShape> {
    if let Ok(value) = std::env::var("F2Z_SHA_MNUMROWS_LOG2S") {
        assert!(
            std::env::var_os("F2Z_BENCH_SHAPES").is_none()
                && std::env::var_os("F2Z_SHA_LOG2S").is_none(),
            "F2Z_SHA_MNUMROWS_LOG2S cannot be combined with compression-count shape variables"
        );
        return parse_exponent_list(&value, "F2Z_SHA_MNUMROWS_LOG2S")
            .into_iter()
            .map(|exponent| {
                assert!(
                    (18..=30).contains(&exponent),
                    "SHA-256 MnumRows exponents must be in 18..=30"
                );
                BenchShape::AssignmentRows(exponent)
            })
            .collect();
    }

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
                "SHA-256 runtime-prime protocol supports exponents 4 through 16"
            );
            BenchShape::Compressions(exponent)
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
    inner_prefix_vars: usize,
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
        generate_sha256_compression_witnesses(prepared, inputs)
            .expect("SHA witness synthesis succeeds")
    };
    let statements = {
        let _scope = f2z::utils::prof::scope("sha256-trace:statement_materialization");
        let statements = inputs
            .iter()
            .zip(witness.outputs())
            .map(|(&input, &output)| Sha256CompressionStatement::new(input, output))
            .collect::<Vec<_>>();
        black_box(&statements);
        statements
    };
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;
    let _ = f2z::utils::prof::take_totals();

    // End-to-end prove: Step 1 commitment plus the runtime-prime proof.
    let prover_scope = f2z::utils::prof::scope("sha256-trace:end_to_end_prove");
    let prove_started = Instant::now();
    let started = Instant::now();
    let hint = {
        let _scope = f2z::utils::prof::scope("sha256-trace:commit");
        commit_sha256_compression_witness_with_config(prepared, &witness, pc)
            .expect("SHA source commitment succeeds")
    };
    let commit_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut prover_transcript = Blake3Transcript::new();
    let proof = {
        let _scope = f2z::utils::prof::scope("sha256-trace:proof");
        prove_sha256_compressions_with_prefix_vars_and_config(
            &mut prover_transcript,
            prepared,
            &statements,
            &witness,
            &hint,
            inner_prefix_vars,
            pc,
        )
        .expect("SHA proof succeeds")
    };
    let forests = proof.f2z().mfs.len();
    if prepared.opening_layout() == Sha256OpeningLayout::Default {
        assert_eq!(
            forests, 1,
            "every production-layout SHA proof uses exactly one merged forest"
        );
    }
    drop(prover_scope);
    let prove_ms = prove_started.elapsed().as_secs_f64() * 1e3;
    let prove_phases = f2z::utils::prof::take_totals();

    let mut verifier_transcript = Blake3Transcript::new();
    let started = Instant::now();
    {
        let _scope = f2z::utils::prof::scope("sha256-trace:verification");
        verify_sha256_compressions_with_config(
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
    let spartan_elements = 3 * proof.inner().round_polynomials.len();
    let field_bytes = proof
        .inner()
        .round_polynomials
        .first()
        .map_or(0, |round| round[0].canonical_element_encoding().len());
    let spartan_bytes = spartan_elements * field_bytes + 8 * (proof.inner_nonces().len() + 2);

    let timing = RepTiming {
        witness_ms,
        commit_ms,
        prove_ms,
        verify_ms,
        prove_phases,
        verify_phases,
        spartan_bytes,
        f2z_bytes,
        forests,
    };
    (timing, intervals)
}

fn bench_shape<P: IopSecurityProfile>(
    shape: BenchShape,
    reps: usize,
    root_seed: u64,
    threads: usize,
    inner_prefix_vars: usize,
    layout: Sha256OpeningLayout,
    trace_writer: &mut Option<TraceWriter>,
) {
    let exponent = shape.exponent();
    let shape_seed = root_seed
        ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ match shape {
            BenchShape::Compressions(_) => 0,
            BenchShape::AssignmentRows(_) => 0x6d6e_756d_726f_7773,
        };

    let setup_started = Instant::now();
    let prepared = match shape.prepare::<P>(layout) {
        Ok(prepared) => prepared,
        Err(
            error @ (Sha256ConstraintError::Profile(_) | Sha256ConstraintError::PrimeProfile(_)),
        ) => {
            println!();
            println!(
                "sha256 {} profile={}: SKIPPED - {error}",
                shape.slug(),
                P::NAME
            );
            return;
        }
        Err(error) => panic!("prepare failed: {error}"),
    };
    let (pc, vc) = sha256_compression_configs(&prepared).expect("valid Ligerito config");
    let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;
    let compressions = prepared.instances();
    let live_source_cells = 1 + SHA256_F_INSTANCE_BITS * compressions;
    let live_assignment_cells = 1 + SHA256_H_INSTANCE_BITS * compressions;
    let source_cells = prepared.source_params().cells();
    let assignment_cells = prepared.assignment_params().cells();

    println!();
    match shape {
        BenchShape::Compressions(_) => {
            println!("=== 2^{exponent} = {compressions} independent SHA-256 compressions ===")
        }
        BenchShape::AssignmentRows(_) => println!(
            "=== MnumRows=2^{exponent} = {assignment_cells}; {compressions} packed SHA-256 compressions ==="
        ),
    }
    println!(
        "  source: {live_source_cells} live / {source_cells} padded cells | derived: {live_assignment_cells} live / {assignment_cells} padded cells",
    );
    println!(
        "  packing: 1 shared constant + {compressions}×({} source, {} assignment) + trailing zeros only",
        SHA256_F_INSTANCE_BITS, SHA256_H_INSTANCE_BITS,
    );
    println!(
        "  linear relation: {} adjacent rows/compression; {} live / {} global rows | inner prefix K={} | packed map nnz={} | setup {}",
        SHA256_CONSTRAINTS,
        SHA256_CONSTRAINTS * compressions,
        (SHA256_CONSTRAINTS * compressions).next_power_of_two(),
        inner_prefix_vars,
        prepared.map().nnz(),
        fmt_ms(setup_ms),
    );
    println!(
        "  security profile: {} (λ={})",
        prepared.security().profile_name,
        prepared.security().lambda,
    );

    let warm_inputs = make_inputs(compressions, shape_seed);
    let (warm, warm_intervals) = run_once(&warm_inputs, &prepared, inner_prefix_vars, &pc, &vc);
    if let Some(writer) = trace_writer {
        writer.write_run(
            shape,
            shape_seed,
            &prepared,
            inner_prefix_vars,
            Trial::Warmup(0),
            &warm_intervals,
        );
    }
    {
        let opening = prepared.opening_params();
        let kind = match prepared.opening_layout() {
            Sha256OpeningLayout::Default if prepared.instances().is_power_of_two() => {
                "direct product opening"
            }
            Sha256OpeningLayout::Default => "inner sumcheck, balanced one-forest split",
            Sha256OpeningLayout::InnerSumcheck { .. } => "inner sumcheck, explicit split",
            Sha256OpeningLayout::ProductTransposed { .. } => {
                "direct product opening, transposed (local bits on rows)"
            }
        };
        println!(
            "  opening layout: {kind} | F2Z rows 2^{} × columns 2^{} | forests {} | read-off ≤ 2^{} integers per forest",
            opening.t, opening.s, warm.forests, opening.s
        );
    }
    black_box(warm);

    let mut prover = common::StepSamples::default();
    let mut verifier = common::StepSamples::default();
    let mut witness_samples = Vec::with_capacity(reps);
    let mut last = None;
    for sample in 0..reps {
        let input_seed = shape_seed ^ ((sample + 1) as u64).wrapping_mul(0xd6e8_feb8_6659_fd93);
        let inputs = make_inputs(compressions, input_seed);
        let (timing, intervals) = run_once(&inputs, &prepared, inner_prefix_vars, &pc, &vc);
        if let Some(writer) = trace_writer {
            writer.write_run(
                shape,
                shape_seed,
                &prepared,
                inner_prefix_vars,
                Trial::Sample(sample),
                &intervals,
            );
        }
        println!(
            "  SAMPLE shape_mode={} exponent={exponent} sample={} compressions={compressions} mnum_rows={assignment_cells} inner_prefix_vars={inner_prefix_vars} witness_ms={:.6} commit_ms={:.6} prove_ms={:.6} verify_ms={:.6} verified=true",
            shape.mode(),
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
        shape: shape.slug(),
        extra: vec![
            ("profile".into(), prepared.security().profile_name.into()),
            ("compressions".into(), compressions.to_string()),
            ("mnum_rows".into(), assignment_cells.to_string()),
            ("shape_mode".into(), shape.mode().to_owned()),
            ("step2_semantics".into(), "runtime_field_setup".into()),
            ("inner_prefix_vars".into(), inner_prefix_vars.to_string()),
            ("throughput_per_s".into(), format!("{throughput:.3}")),
            ("shape_seed".into(), format!("{shape_seed:#018x}")),
        ],
        lambda: Some(prepared.security().lambda),
        lambda_achieved: Some(prepared.security().accounting.achieved_bits()),
        lambda_bind: Some(prepared.security().accounting.binding_term().name.into()),
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
    let inner_prefix_vars = std::env::var("F2Z_SHA_INNER_PREFIX_VARS").map_or(
        SHA256_DEFAULT_INNER_PREFIX_VARS,
        |value| {
            value
                .parse::<usize>()
                .expect("F2Z_SHA_INNER_PREFIX_VARS must be an integer in 0..=4")
        },
    );
    assert!(
        inner_prefix_vars <= SHA256_INNER_PREFIX_MAX_VARS,
        "F2Z_SHA_INNER_PREFIX_VARS must be in 0..={SHA256_INNER_PREFIX_MAX_VARS}"
    );

    let layout = std::env::var("F2Z_SHA_OPENING_T").map_or(Sha256OpeningLayout::Default, |value| {
        let row_vars = value
            .trim()
            .parse::<usize>()
            .expect("F2Z_SHA_OPENING_T must be an integer: the F2Z row variables of the opening");
        match std::env::var("F2Z_SHA_OPENING_LAYOUT").as_deref() {
            Ok("product") => Sha256OpeningLayout::ProductTransposed { row_vars },
            Ok("inner") | Err(_) => Sha256OpeningLayout::InnerSumcheck { row_vars },
            Ok(other) => panic!("F2Z_SHA_OPENING_LAYOUT must be `inner` or `product`, got {other}"),
        }
    });

    let selected = common::security_profile(PrimePolicy::SingleDerived);
    let profile = selected.unwrap_or(common::SecurityProfile::Lambda100);

    println!("SHA-256: flat packed [1|f₀|f₁|…], h=Mf; direct product opening + virtual F2Z");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!(
        "repetitions: {reps}; warmups: 1; inner prefix K={inner_prefix_vars}; root seed: {root_seed:#018x}"
    );
    println!(
        "security profile: {}",
        common::profile_banner(selected, common::SecurityProfile::Lambda100)
    );
    match layout {
        Sha256OpeningLayout::InnerSumcheck { row_vars } => println!(
            "opening layout override: inner sumcheck with 2^{row_vars} F2Z rows (F2Z_SHA_OPENING_T={row_vars})"
        ),
        Sha256OpeningLayout::ProductTransposed { row_vars } => println!(
            "opening layout override: transposed product tensor with 2^{row_vars} F2Z rows (F2Z_SHA_OPENING_LAYOUT=product F2Z_SHA_OPENING_T={row_vars})"
        ),
        Sha256OpeningLayout::Default => {}
    }
    if let Some(path) = std::env::var_os("F2Z_SHA_TRACE_PATH") {
        println!("canonical interval trace: {}", Path::new(&path).display());
    }

    for shape in shapes() {
        flock_core::scratch::clear();
        common::with_profile!(
            profile,
            bench_shape(
                shape,
                reps,
                root_seed,
                threads,
                inner_prefix_vars,
                layout,
                &mut trace_writer,
            )
        );
    }
    flock_core::scratch::clear();
}
