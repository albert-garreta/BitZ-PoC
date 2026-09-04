//! Shared harness for the protocol benches: one accounting model, one
//! printer, one machine-readable `RESULT` line (`schema=f2z/1`).
//!
//! The full schema — timing semantics, the paper §2.1 step taxonomy, key
//! names, and env-var conventions — is documented in `docs/bench-schema.md`.
//! Keep that file and this module in lockstep.
//!
//! Summary of the semantics implemented here:
//! - `prove_ms` is the **end-to-end prover**: everything after the prover
//!   holds a witness (bit-packing, commitment, projection, prime sampling +
//!   grinding, PIOP, bitification, Step 5.0, the F2Z opening).
//! - Witness generation and one-time public preprocessing are excluded and
//!   reported separately (`witness_ms`, `setup_ms`).
//! - Per-step splits come from the crate's umbrella profiler scopes
//!   (`step2:*` … `step5:*`); a signed residual makes each split sum to its
//!   total exactly.
//! - Steps that do not run in a path print `na`, never `0.00`.

#![allow(dead_code)] // each bench uses a subset of the harness

use std::time::Instant;

// ---------------------------------------------------------------------
// Environment: canonical knobs, deprecated aliases, strict unknown check
// ---------------------------------------------------------------------

/// Every `F2Z_*` variable any binary in this repo understands. An exported
/// `F2Z_*` variable outside this list aborts the bench so a typo'd knob can
/// never silently do nothing. Keep sorted; add new knobs here.
pub const KNOWN_F2Z_ENV: &[&str] = &[
    // A/B example harness knobs (examples/taps_ab.rs, examples/rlc_ab.rs).
    "F2Z_AB_B3FAM",
    "F2Z_AB_B3OPEN",
    "F2Z_AB_BLAKE3",
    "F2Z_AB_COLLAPSE",
    "F2Z_AB_COLS4",
    "F2Z_AB_COLS4_FAM",
    "F2Z_AB_FAM_DELTA",
    "F2Z_AB_J3",
    "F2Z_AB_MIX6",
    "F2Z_AB_N",
    "F2Z_AB_NO_FAMILY",
    "F2Z_AB_OPEN8",
    "F2Z_AB_OPEN_DELTA",
    "F2Z_AB_REPS",
    "F2Z_AB_ROUNDS",
    "F2Z_AB_SCHED",
    "F2Z_AB_SHARED",
    "F2Z_AB_SINGLES",
    "F2Z_AB_STMTS",
    // Deprecated aliases (kept working; see `reps`/`shapes`/`seed`).
    "F2Z_BABY_BEAR_MUL_EXPONENTS",
    "F2Z_BABY_BEAR_MUL_SEED",
    // Canonical bench knobs.
    "F2Z_BENCH_EXT",
    "F2Z_BENCH_FILL",
    "F2Z_BENCH_ORDER",
    "F2Z_BENCH_PASS",
    "F2Z_BENCH_REPS",
    "F2Z_BENCH_SEED",
    "F2Z_BENCH_SHAPES",
    "F2Z_CM_EXPONENTS",
    "F2Z_CM_SEED",
    // Prover-path toggles (transcript-preserving optimization knobs).
    "F2Z_COL_ELIDE",
    "F2Z_EQF_DOUBLE",
    "F2Z_EQF_DOUBLE_MIN",
    "F2Z_EQF_FUSE",
    "F2Z_EQF_NOKERNEL",
    "F2Z_EQ_TABLE_SAMPLES",
    "F2Z_FIXED_SCALAR",
    "F2Z_FLAT_FOREST",
    "F2Z_FOLDV_LUT",
    "F2Z_INNER_FIELD_ACCUM",
    "F2Z_INNER_NATIVE_FOLD",
    "F2Z_JIT_GRID",
    "F2Z_JIT_R1",
    "F2Z_LEAF8",
    "F2Z_LEAF_A2_FACTORED",
    "F2Z_LEAF_TILE",
    "F2Z_LIG_PROFILE",
    "F2Z_LUT3",
    "F2Z_LUT4",
    "F2Z_LUT_PRFM",
    "F2Z_MATS_PRE",
    "F2Z_MATS_TILE",
    "F2Z_MATS_TILE_B",
    "F2Z_MAT_GRID",
    // Deprecated aliases.
    "F2Z_MULTISWAP_REPS",
    // U32-specific bench knob.
    "F2Z_MUL_WORD_BITS",
    "F2Z_PAIR2_FACTORED",
    "F2Z_PAR_CHUNK",
    "F2Z_QUAD",
    "F2Z_QUAD_KERNEL",
    "F2Z_RLC_EAGER",
    "F2Z_RLC_J34_LAZY",
    "F2Z_RS_FAST",
    // SHA trace-writer knobs.
    "F2Z_SHA_BUILD_PROFILE",
    "F2Z_SHA_CPU",
    "F2Z_SHA_GIT_REV",
    "F2Z_SHA_INNER_PREFIX_VARS",
    "F2Z_SHA_LOG2S",
    "F2Z_SHA_MNUMROWS_LOG2S",
    "F2Z_SHA_PRODUCT_TS",
    "F2Z_SHA_REPS",
    "F2Z_SHA_RESULT_PATH",
    "F2Z_SHA_SEED",
    "F2Z_SHA_TRACE_PATH",
    "F2Z_SPARTAN_REDUCTION",
    "F2Z_T4_FACTORED",
    "F2Z_T4_PRFM",
    "F2Z_TAPS_DELTA",
    "F2Z_TAPS_GRP",
    "F2Z_TAPS_SEED",
    "F2Z_VIRT_ID_FAST",
];

/// Aborts on any exported `F2Z_*` variable the repo does not know.
pub fn enforce_known_env() {
    let mut unknown: Vec<String> = std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|key| key.starts_with("F2Z_") && !KNOWN_F2Z_ENV.contains(&key.as_str()))
        .collect();
    if unknown.is_empty() {
        return;
    }
    unknown.sort();
    eprintln!(
        "error: unknown F2Z_* environment variable(s): {}",
        unknown.join(", ")
    );
    eprintln!("       known knobs (docs/bench-schema.md):");
    for chunk in KNOWN_F2Z_ENV.chunks(4) {
        eprintln!("         {}", chunk.join(" "));
    }
    std::process::exit(2);
}

/// Reads `canonical`, falling back to `alias` with a deprecation warning.
/// Setting both to different values is an error.
fn env_with_alias(canonical: &str, alias: Option<&str>) -> Option<String> {
    let canonical_value = std::env::var(canonical).ok();
    let alias_value = alias.and_then(|name| std::env::var(name).ok());
    match (canonical_value, alias_value) {
        (Some(main), Some(old)) => {
            if main != old {
                eprintln!(
                    "error: {canonical}={main} and deprecated alias {}={old} disagree",
                    alias.unwrap_or_default()
                );
                std::process::exit(2);
            }
            Some(main)
        }
        (Some(main), None) => Some(main),
        (None, Some(old)) => {
            eprintln!(
                "warning: {} is deprecated; use {canonical}",
                alias.unwrap_or_default()
            );
            Some(old)
        }
        (None, None) => None,
    }
}

/// Measured repetitions (one extra untimed warm-up is always run).
pub fn reps(alias: Option<&str>, default: usize) -> usize {
    let value = env_with_alias("F2Z_BENCH_REPS", alias)
        .map(|value| {
            value
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("F2Z_BENCH_REPS must be a positive integer"))
        })
        .unwrap_or(default);
    assert!(value > 0, "F2Z_BENCH_REPS must be positive");
    value
}

/// Bench-specific shape list (meaning documented per bench).
pub fn shapes(alias: Option<&str>) -> Option<Vec<String>> {
    env_with_alias("F2Z_BENCH_SHAPES", alias).map(|value| {
        let shapes: Vec<String> = value
            .split([',', ' '])
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
        assert!(!shapes.is_empty(), "F2Z_BENCH_SHAPES must not be empty");
        shapes
    })
}

/// Root seed (decimal or 0x-hex).
pub fn seed(alias: Option<&str>, default: u64) -> u64 {
    env_with_alias("F2Z_BENCH_SEED", alias).map_or(default, |value| {
        let parsed = if let Some(hex) = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
        {
            u64::from_str_radix(hex, 16).ok()
        } else {
            value.parse().ok()
        };
        parsed.unwrap_or_else(|| panic!("F2Z_BENCH_SEED must be a decimal or 0x-hex u64"))
    })
}

/// Shared bench startup: force the phase profiler on (the step split must
/// always be populated), validate the environment, and size the perf pool.
/// Call first in `main`, before any thread is spawned or any profiler scope
/// opens. Returns the rayon thread count.
pub fn init() -> usize {
    // SAFETY: called before any other thread can read the environment.
    unsafe { std::env::set_var("OBLONG_PROFILE", "1") };
    enforce_known_env();
    let _ = flock_core::init_perf_thread_pool();
    #[cfg(feature = "parallel")]
    {
        rayon::current_num_threads()
    }
    #[cfg(not(feature = "parallel"))]
    {
        1
    }
}

/// Rayon thread count without the full [`init`] (for benches that keep the
/// phase profiler opt-in, like `pcs`).
pub fn threads() -> usize {
    #[cfg(feature = "parallel")]
    {
        rayon::current_num_threads()
    }
    #[cfg(not(feature = "parallel"))]
    {
        1
    }
}

// ---------------------------------------------------------------------
// Step taxonomy (paper §2.1, code order)
// ---------------------------------------------------------------------

/// Umbrella scope labels: the step totals. Step 1 (commit) is bench-timed
/// wall clock, not a scope.
pub const STEP2_PROVE: &str = "step2:project_prove";
pub const STEP3_PROVE: &str = "step3:piop_prove";
pub const STEP4_PROVE: &str = "step4:bitify_prove";
pub const STEP5_0_PROVE: &str = "step5_0:reduce_prove";
pub const STEP5_PROVE: &str = "step5:open_prove";
pub const STEP2_VERIFY: &str = "step2:project_verify";
pub const STEP3_VERIFY: &str = "step3:piop_verify";
pub const STEP4_VERIFY: &str = "step4:bitify_verify";
pub const STEP5_0_VERIFY: &str = "step5_0:reduce_verify";
pub const STEP5_VERIFY: &str = "step5:open_verify";

/// Shared detail-label table (nested under the umbrellas). Sums of these
/// feed the optional detail keys; they never enter the step totals.
const S3_OUTER: &[&str] = &[
    "spartan:outer_sumcheck",
    "spartan:outer_univariate_skip",
    "sha256:spartan_outer_prove",
    "sha256:spartan_outer_verify",
];
const S3_BIND: &[&str] = &["spartan:bind_and_batch"];
const S3_INNER: &[&str] = &[
    "spartan:inner_sumcheck",
    "sha256:spartan_inner_prove",
    "sha256:spartan_inner_verify",
];
/// Step 5.2: exponent tables + merged GKR forest + presum discharge.
const S5_FOREST: &[&str] = &[
    "mc:pack",
    "mc:pow2",
    "mc:forest",
    "mc:fold_v",
    "mc:presum_tbls",
    "mc:presum_run",
    "mqv:pack",
];
/// Step 5.3: ring switch + recursive Ligerito (including the virtual
/// batching machinery: derived weights, the h_i fold, the a′ build).
const S5_OPENER: &[&str] = &[
    "mq:rings",
    "mq:bcomb",
    "mq:lig",
    "mq:rings_main",
    "mq:bcomb_main",
    "mqv:wprep",
    "mqv:hs",
    "mqv:aprime",
    "mqv:lig",
    "mqv:vwprep",
    "mqv:vaprime",
    "mqv:vlig",
];

fn label_sum_ms(phases: &[(&'static str, f64)], labels: &[&str]) -> Option<f64> {
    let mut sum = 0.0;
    let mut seen = false;
    for (label, seconds) in phases {
        if labels.contains(label) {
            sum += seconds * 1e3;
            seen = true;
        }
    }
    seen.then_some(sum)
}

fn label_ms(phases: &[(&'static str, f64)], label: &str) -> Option<f64> {
    label_sum_ms(phases, &[label])
}

/// One side's per-step samples across reps. Every value is `Some` only when
/// the corresponding scope fired in **every** rep (otherwise `na`).
#[derive(Default)]
pub struct StepSamples {
    total: Vec<f64>,
    commit: Vec<Option<f64>>,
    project: Vec<Option<f64>>,
    piop: Vec<Option<f64>>,
    bitify: Vec<Option<f64>>,
    reduce: Vec<Option<f64>>,
    open: Vec<Option<f64>>,
    outer: Vec<Option<f64>>,
    bind: Vec<Option<f64>>,
    inner: Vec<Option<f64>>,
    forest: Vec<Option<f64>>,
    opener: Vec<Option<f64>>,
}

impl StepSamples {
    /// Records one prover rep: the end-to-end wall time, the bench-timed
    /// Step 1 (bit-pack + commit) wall time, and the profiler totals drained
    /// after the prove call.
    pub fn record_prove(&mut self, total_ms: f64, commit_ms: f64, phases: &[(&'static str, f64)]) {
        self.total.push(total_ms);
        self.commit.push(Some(commit_ms));
        self.record_scopes(
            phases,
            [
                STEP2_PROVE,
                STEP3_PROVE,
                STEP4_PROVE,
                STEP5_0_PROVE,
                STEP5_PROVE,
            ],
        );
    }

    /// Records one verifier rep (no Step 1: the verifier holds a commitment).
    pub fn record_verify(&mut self, total_ms: f64, phases: &[(&'static str, f64)]) {
        self.total.push(total_ms);
        self.commit.push(None);
        self.record_scopes(
            phases,
            [
                STEP2_VERIFY,
                STEP3_VERIFY,
                STEP4_VERIFY,
                STEP5_0_VERIFY,
                STEP5_VERIFY,
            ],
        );
    }

    fn record_scopes(&mut self, phases: &[(&'static str, f64)], steps: [&str; 5]) {
        self.project.push(label_ms(phases, steps[0]));
        self.piop.push(label_ms(phases, steps[1]));
        self.bitify.push(label_ms(phases, steps[2]));
        self.reduce.push(label_ms(phases, steps[3]));
        self.open.push(label_ms(phases, steps[4]));
        self.outer.push(label_sum_ms(phases, S3_OUTER));
        self.bind.push(label_sum_ms(phases, S3_BIND));
        self.inner.push(label_sum_ms(phases, S3_INNER));
        self.forest.push(label_sum_ms(phases, S5_FOREST));
        self.opener.push(label_sum_ms(phases, S5_OPENER));
    }

    /// Medians. The residual is the signed difference between the total
    /// median and the sum of the present step medians, so the printed split
    /// sums to the printed total exactly.
    pub fn medians(&self) -> StepMedians {
        let total = median(&self.total);
        let commit = optional_median(&self.commit);
        let project = optional_median(&self.project);
        let piop = optional_median(&self.piop);
        let bitify = optional_median(&self.bitify);
        let reduce = optional_median(&self.reduce);
        let open = optional_median(&self.open);
        let steps_sum: f64 = [commit, project, piop, bitify, reduce, open]
            .iter()
            .flatten()
            .sum();
        StepMedians {
            total,
            commit,
            project,
            piop,
            bitify,
            reduce,
            open,
            residual: total - steps_sum,
            outer: optional_median(&self.outer),
            bind: optional_median(&self.bind),
            inner: optional_median(&self.inner),
            forest: optional_median(&self.forest),
            opener: optional_median(&self.opener),
        }
    }
}

/// Median step split of one side, in milliseconds. `None` = `na`.
#[derive(Clone, Copy, Debug)]
pub struct StepMedians {
    pub total: f64,
    pub commit: Option<f64>,
    pub project: Option<f64>,
    pub piop: Option<f64>,
    pub bitify: Option<f64>,
    pub reduce: Option<f64>,
    pub open: Option<f64>,
    pub residual: f64,
    pub outer: Option<f64>,
    pub bind: Option<f64>,
    pub inner: Option<f64>,
    pub forest: Option<f64>,
    pub opener: Option<f64>,
}

// ---------------------------------------------------------------------
// Report + printer
// ---------------------------------------------------------------------

/// Transmitted proof size split.
#[derive(Clone, Copy, Debug)]
pub struct ProofBytes {
    /// Spartan payload + grinding nonces + the Step 5.0 integer lift.
    pub piop: usize,
    /// The serialized F2Z opening (`to_bytes` of the codec proof).
    pub open: usize,
}

impl ProofBytes {
    pub const fn total(&self) -> usize {
        self.piop + self.open
    }
}

/// Everything the uniform printer needs for one measured configuration.
pub struct BenchReport {
    /// `bench=` value: `multiswap`, `sha256`, `u32_mul`, `pcs`, …
    pub bench: &'static str,
    /// `shape=` value: one compact token unique within the bench.
    pub shape: String,
    /// Bench-specific keys, printed between `shape=` and `lambda=`.
    pub extra: Vec<(String, String)>,
    /// Security target the run was measured at; `None` prints `na`.
    pub lambda: Option<u32>,
    /// Achieved bits (min over every soundness term, floors included).
    pub lambda_achieved: Option<f64>,
    /// Name of the binding soundness term.
    pub lambda_bind: Option<String>,
    pub threads: usize,
    pub reps: usize,
    /// Root seed; `None` prints `na` (deterministic benches).
    pub seed: Option<u64>,
    /// One-time, excluded from `prove_ms`.
    pub witness_ms: f64,
    /// One-time public preprocessing, excluded from `prove_ms`.
    pub setup_ms: f64,
    pub prover: StepMedians,
    pub verifier: StepMedians,
    pub proof: ProofBytes,
}

fn fmt_opt(value: Option<f64>) -> String {
    value.map_or_else(|| "na".to_owned(), |value| format!("{value:.3}"))
}

fn fmt_row(value: Option<f64>) -> String {
    value.map_or_else(
        || "     n/a   ".to_owned(),
        |value| format!("{value:9.2} ms"),
    )
}

impl BenchReport {
    /// The uniform human block (step rows sum to the totals exactly).
    pub fn print_human(&self) {
        if let (Some(lambda), Some(achieved)) = (self.lambda, self.lambda_achieved) {
            println!(
                "  security: target λ={lambda} | achieved {achieved:.1} bits (binding term: {})",
                self.lambda_bind.as_deref().unwrap_or("unknown")
            );
        }
        println!(
            "  one-time (excluded from prove): witness {:.1} ms | setup {:.1} ms",
            self.witness_ms, self.setup_ms
        );
        println!(
            "  prove (end-to-end, median of {}): {:.2} ms",
            self.reps, self.prover.total
        );
        let p = &self.prover;
        println!("    step 1   commit        {}", fmt_row(p.commit));
        let step2_label = if self.bench == "sha256" {
            "field setup"
        } else {
            "project"
        };
        println!("    step 2   {step2_label:<13}{}", fmt_row(p.project));
        println!(
            "    step 3   piop          {}   (outer {} | bind {} | inner {})",
            fmt_row(p.piop),
            fmt_opt(p.outer),
            fmt_opt(p.bind),
            fmt_opt(p.inner)
        );
        println!("    step 4   bitify        {}", fmt_row(p.bitify));
        println!("    step 5.0 reduce        {}", fmt_row(p.reduce));
        println!(
            "    step 5.* open          {}   (5.2 forest {} | 5.3 ring-switch+Ligerito {})",
            fmt_row(p.open),
            fmt_opt(p.forest),
            fmt_opt(p.opener)
        );
        println!("    residual               {:9.2} ms", p.residual);
        println!("  verify (median): {:.2} ms", self.verifier.total);
        let v = &self.verifier;
        println!("    step 2   {step2_label:<13}{}", fmt_row(v.project));
        println!(
            "    step 3   piop          {}   (outer {})",
            fmt_row(v.piop),
            fmt_opt(v.outer)
        );
        println!("    step 4   bitify        {}", fmt_row(v.bitify));
        println!("    step 5.0 reduce        {}", fmt_row(v.reduce));
        println!("    step 5.* open          {}", fmt_row(v.open));
        println!("    residual               {:9.2} ms", v.residual);
        println!(
            "  proof: {} B ({:.1} KB) = piop {} B + open {} B",
            self.proof.total(),
            self.proof.total() as f64 / 1e3,
            self.proof.piop,
            self.proof.open
        );
        println!("  {}", self.result_line());
    }

    /// The machine-readable line (`docs/bench-schema.md`).
    pub fn result_line(&self) -> String {
        let mut line = format!(
            "RESULT schema=f2z/1 bench={} shape={}",
            self.bench, self.shape
        );
        for (key, value) in &self.extra {
            line.push_str(&format!(" {key}={value}"));
        }
        let lambda = self
            .lambda
            .map_or_else(|| "na".to_owned(), |bits| bits.to_string());
        let lambda_achieved = self
            .lambda_achieved
            .map_or_else(|| "na".to_owned(), |bits| format!("{bits:.1}"));
        let lambda_bind = self.lambda_bind.clone().unwrap_or_else(|| "na".to_owned());
        let seed = self
            .seed
            .map_or_else(|| "na".to_owned(), |seed| format!("{seed:#018x}"));
        let p = &self.prover;
        let v = &self.verifier;
        line.push_str(&format!(
            " lambda={lambda} lambda_achieved={lambda_achieved} lambda_bind={lambda_bind} \
             threads={} reps={} warmups=1 seed={seed} \
             witness_ms={:.3} setup_ms={:.3} prove_ms={:.3} s1_commit_ms={} \
             s2_project_ms={} s3_piop_ms={} s4_bitify_ms={} s5_0_reduce_ms={} \
             s5_open_ms={} prove_residual_ms={:.3} s3_outer_ms={} s3_bind_ms={} \
             s3_inner_ms={} s5_forest_ms={} s5_opener_ms={} verify_ms={:.3} \
             v2_project_ms={} v3_piop_ms={} v4_bitify_ms={} v5_0_reduce_ms={} \
             v5_open_ms={} verify_residual_ms={:.3} proof_bytes={} \
             proof_piop_bytes={} proof_open_bytes={} verified_samples={}",
            self.threads,
            self.reps,
            self.witness_ms,
            self.setup_ms,
            p.total,
            fmt_opt(p.commit),
            fmt_opt(p.project),
            fmt_opt(p.piop),
            fmt_opt(p.bitify),
            fmt_opt(p.reduce),
            fmt_opt(p.open),
            p.residual,
            fmt_opt(p.outer),
            fmt_opt(p.bind),
            fmt_opt(p.inner),
            fmt_opt(p.forest),
            fmt_opt(p.opener),
            v.total,
            fmt_opt(v.project),
            fmt_opt(v.piop),
            fmt_opt(v.bitify),
            fmt_opt(v.reduce),
            fmt_opt(v.open),
            v.residual,
            self.proof.total(),
            self.proof.piop,
            self.proof.open,
            self.reps,
        ));
        line
    }
}

// ---------------------------------------------------------------------
// Small shared utilities
// ---------------------------------------------------------------------

pub fn median(samples: &[f64]) -> f64 {
    assert!(!samples.is_empty(), "median of an empty sample set");
    let mut sorted = samples.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    sorted[sorted.len() / 2]
}

/// Median over reps of an optional per-rep value; `Some` only when the value
/// was present in every rep.
fn optional_median(samples: &[Option<f64>]) -> Option<f64> {
    let values: Option<Vec<f64>> = samples.iter().copied().collect();
    values
        .filter(|values| !values.is_empty())
        .map(|values| median(&values))
}

/// Milliseconds elapsed since `start`.
pub fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}
