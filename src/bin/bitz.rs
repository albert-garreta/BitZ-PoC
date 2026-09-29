//! `bitz` — CLI runner: commit / prove / verify one mod-`q` MLE-opening
//! instance at a chosen shape, with timings, proof-size split, and peak
//! heap. The runnable sibling of `benches/pcs.rs` for one-off shapes.
//!
//! ```text
//! # release + unchecked (the bench convention; the CLI is the only bin,
//! # so plain `cargo run` targets it):
//! RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
//! RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
//!     28 17 11 --threads 1 --reps 5 --profile slim
//! ```
//!
//! Usage: `bitz <n> [<t> <s>] [options]`
//!
//! - `n` — committed bit-index variables, `n = t + s`. Without `t s`,
//!   use `t = ceil(0.6n)`, clamped to the seven-bit packing width.
//! - `--threads N` / `-j N` — rayon pool size; `1` = single-threaded.
//!   Default: all cores (or `RAYON_NUM_THREADS`).
//! - `--reps R` — timing repetitions (median reported; default 3).
//! - `--profile P` — Ligerito config: `custom:1:4` (default; validator-gated
//!   Johnson geometry at rate 1/2, initial_k 4) | `slim` (rate 1/4) | `slim3`
//!   (rate 1/8) | `fast` (rate 1/2) | `secure`
//!   (the embedded profiles) | any `custom:<log_inv_rate>:<initial_k>[:<bits>]`
//!   (validator-gated Johnson geometry; optional `bits` = round-by-round
//!   security target, default 100 — e.g. `custom:3:4:128`) |
//!   `udr:<log_inv_rate>:<initial_k>[:<bits>]` (queries-only UDR: zero
//!   grinding, zero OOD; ceiling ≈115 bits at n=22 / ≈109 at n=28).
//!   `custom`/`udr`/`udrg` need `m = n ≥ 20` (m = 20, 21 are seeded from
//!   flock's m = 22 template, every shape field rebuilt and validator-gated
//!   — see `custom_johnson_config_bits`) and use BLAKE3 Merkle trees (like
//!   every Spartan path in the crate; flock's slim template says sha256);
//!   the embedded profiles need `m ≥ 22` and keep their template's hash.
//!   Below that every choice falls back to the ad-hoc test config
//!   (UNAUDITED — no security claim). The `bitz:` header prints the hash.
//! - `--sweep <lo>-<hi>` (or a list `20,24,28`, or mixed `20-24,28`) — the
//!   PAPER-TABLE mode: run the single-claim path once per `n`, each in a
//!   FRESH child process (`std::env::current_exe()` re-invoked with the same
//!   `--threads/--reps/--profile`; one shape per process is the
//!   bench protocol), stream each child's output, parse its `RESULT` line,
//!   print a summary, and write the LaTeX table to `--latex <path>` (default
//!   `outputs/tables/raw-performance-table.tex` in the crate; the file records the
//!   exact command, machine, date, commit, and every RESULT line, so it is
//!   its own provenance). `t s` positionals do not apply (each `n` uses the
//!   reference split). `--cooldown <s>` idles between children so the OS
//!   reclaims the previous shape's memory and the fanless chip cools — the
//!   n ≥ 29 rows (3–7 GB peaks) swing ±30 % on a busy 16 GB box otherwise.
//!   Example (the paper's raw-performance table):
//!   `bitz --sweep 20-30 --threads 8 --reps 5 --profile custom:1:4`
//! - `--mul <e>` — run the u32 × u32 → u64 INTEGER-MULTIPLICATION SNARK
//!   (`piop::spartan`: one R1CS row per multiplication over ℤ, a
//!   transcript-sampled Step-2 prime, the native Spartan PIOP with the K=3
//!   univariate skip, bitification, and the BitZ opening of the 128 committed
//!   bits per multiplication) for `2^e` multiplications, `e ≥ 15`, at the
//!   `--lambda 100|128` profile (default 100 = `Lambda100`). `--profile custom:<r>:<k>` (default
//!   `custom:1:4`, the raw-performance table's opener) selects the Johnson
//!   Ligerito geometry at the profile's target; `--profile udr` selects the
//!   relation's own default (flock's validated UDR at rate 1/2 — what
//!   `benches/mul_bitz.rs` and the transcript pins run). Same witness seed
//!   as the bench, so numbers compare; prints the paper's step split
//!   (Step 1 commit, Step 2 projection, Step 3 PIOP, Step 4 bitification,
//!   Step 5 opening = grand products / ring switch / Ligerito) and one
//!   `RESULT schema=bitz-cli-mul/1` line. `prove` here is END TO END and
//!   INCLUDES the commitment (the bench-schema convention); `witness` is
//!   the witness generation (the products and the Spartan assignment from
//!   the operand pairs), timed as a median of its own and excluded from
//!   `prove`.
//! - `--mul-sweep <lo>-<hi>` — the paper-table mode for `--mul`: one fresh
//!   child process per `e`, then the LaTeX table (default
//!   `outputs/tables/u32-mul-table.tex`; `--latex <path>` overrides). On a 16 GB box
//!   `e ≤ 22` (BitZ n = e + 7 ≤ 29); `e = 23` peaks near 8 GB. Example:
//!   `bitz --mul-sweep 15-22 --threads 8 --reps 5`
//!
//! The single-claim path always prints a per-step breakdown of the prover
//! and the verifier under the `prove:`/`verify:` lines — medians over the
//! timed reps, combining completed Perfetto intervals with native BitZ
//! phase timers. Build with `span-metrics` and set `PERFETTO_TRACE_PROCESSOR`
//! to the native trace processor.
//!
//! The prover buckets are integer folds plus GKR, sumcheck plus ring switch,
//! and Ligerito. A `security:` line reports the
//! Ligerito config's round-by-round target and achieved bits, flock's
//! notion: minimum over levels and error terms), and finally ONE
//! machine-readable line `RESULT schema=bitz-cli/1 key=value …`
//! (`docs/bench-schema.md`) that `--sweep` consumes.
//!
//! Integer-guard mode is a COMPILE-TIME feature: build with
//! `--features unchecked` for release-style plain integer ops (the header
//! reports the active mode and warns otherwise).

use ::bitz::piop::spartan::protocol;
use ::bitz::piop::spartan::protocol::PreparedRelation;
use ::bitz::piop::spartan::protocol::Proof;
use ::bitz::piop::spartan::protocol::bitz_opener::{
    self, BitZLigerito, BitZOpener, BitZOpeningProof,
};
use bitz::piop::spartan::mul::{MulLayout, MulWitness};

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::hint::black_box;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use bitz::ligerito::packed_vars;
use bitz::ligerito_flock::FlockCommitHint;
use bitz::ligerito_flock::LigeritoSelection;
use bitz::ligerito_flock::commit_rs_ligerito_rows;
use bitz::ligerito_flock::{
    OodRoundParams, ood_round_params, standalone_q_bits, weakest_fold_round_grinding,
};
use bitz::pcs::{IntegerMatrixLayout, smallest_generator};
use bitz::piop::spartan::{IopSecurityProfile, Lambda100, Lambda128};
use bitz::transcript::Blake3Transcript;
use flock_core::pcs::ligerito::LigeritoSecurityConfig;
use rand::{RngExt, SeedableRng, rngs::StdRng};

// Peak-heap tracker (wraps System) — the live-heap high-water, the same
// notion as `benches/pcs.rs` / flock's benches, so numbers compare.
//
// Tracking is SWITCHABLE: the contended `fetch_add`/`fetch_max` on every
// allocation taxes allocation-heavy multi-threaded phases (measured ~20% on
// the merged forest at 8 threads against the untracked `u32_mul` bench), so
// the timed reps run with tracking OFF (one uncontended relaxed load per
// allocation) and the peaks come from tracked probes that run BEFORE the
// untracked windows (warm-up and peak probe first, timed reps after), so
// every persistent buffer — the live rows/hint, flock's scratch arenas —
// was allocated while tracked and is counted in the baseline.
struct PeakAlloc;
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TRACK: AtomicBool = AtomicBool::new(true);
unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() && TRACK.load(Ordering::Relaxed) {
            let c = CUR.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(c, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        if TRACK.load(Ordering::Relaxed) {
            CUR.fetch_sub(l.size(), Ordering::Relaxed);
        }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() && TRACK.load(Ordering::Relaxed) {
            if new >= l.size() {
                let c = CUR.fetch_add(new - l.size(), Ordering::Relaxed) + (new - l.size());
                PEAK.fetch_max(c, Ordering::Relaxed);
            } else {
                CUR.fetch_sub(l.size() - new, Ordering::Relaxed);
            }
        }
        q
    }
}
#[global_allocator]
static ALLOC: PeakAlloc = PeakAlloc;
fn reset_peak() {
    PEAK.store(CUR.load(Ordering::Relaxed), Ordering::Relaxed);
}
/// Switch heap tracking off around timed work (and back on for peak probes).
fn set_heap_tracking(on: bool) {
    TRACK.store(on, Ordering::Relaxed);
}
fn peak_mb() -> f64 {
    PEAK.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Per-step wall-clock samples across reps, keyed by prof scope label
/// (milliseconds, rep-aligned — a label absent from a rep reads as 0).
#[derive(Default)]
struct StepTable(Vec<(String, Vec<f64>)>);

impl StepTable {
    /// Fold one rep's drained completed Perfetto intervals (label, seconds) in.
    fn absorb(&mut self, rep: usize, totals: Vec<(String, f64)>) {
        for (label, secs) in totals {
            let idx = match self.0.iter().position(|(l, _)| *l == label) {
                Some(i) => i,
                None => {
                    self.0.push((label, Vec::new()));
                    self.0.len() - 1
                }
            };
            let slot = &mut self.0[idx].1;
            slot.resize(rep, 0.0);
            slot.push(secs * 1e3);
        }
    }

    fn has(&self, labels: &[&str]) -> bool {
        labels.iter().any(|l| self.0.iter().any(|(k, _)| k == l))
    }

    fn at(&self, label: &str, rep: usize) -> f64 {
        self.0
            .iter()
            .find(|(k, _)| *k == label)
            .and_then(|(_, v)| v.get(rep))
            .copied()
            .unwrap_or(0.0)
    }

    /// Median over reps of `Σ plus − Σ minus`, clamped at 0 (timer jitter).
    fn med(&self, reps: usize, plus: &[&str], minus: &[&str]) -> f64 {
        let series: Vec<f64> = (0..reps)
            .map(|r| {
                let p: f64 = plus.iter().map(|l| self.at(l, r)).sum();
                let m: f64 = minus.iter().map(|l| self.at(l, r)).sum();
                (p - m).max(0.0)
            })
            .collect();
        median(series)
    }
}

/// One printed breakdown row: `Σ plus − Σ minus` of prof labels. `sub` rows
/// are nested detail (indented; their parent's time already includes them).
struct StepRow {
    name: &'static str,
    plus: &'static [&'static str],
    minus: &'static [&'static str],
    sub: bool,
}

const fn step(name: &'static str, plus: &'static [&'static str]) -> StepRow {
    StepRow {
        name,
        plus,
        minus: &[],
        sub: false,
    }
}
const fn substep(
    name: &'static str,
    plus: &'static [&'static str],
    minus: &'static [&'static str],
) -> StepRow {
    StepRow {
        name,
        plus,
        minus,
        sub: true,
    }
}

/// Native subphases; nested opening details do not enter totals twice.
const PROVE_STEP_ROWS: &[StepRow] = &[
    step("integer folds and images", &["fold+images"]),
    step("native GKR", &["gkr"]),
    step("inner-product sumcheck", &["sumcheck"]),
    step("ring switch", &["ring switch"]),
    step("Ligerito opening", &["ligerito"]),
];
const PROVE_TOP_LABELS: &[&str] = &["fold+images", "gkr", "sumcheck", "ring switch", "ligerito"];
const VERIFY_STEP_ROWS: &[StepRow] = &[
    step("integer fold checks", &["v: fold"]),
    step("GKR checks", &["v: gkr"]),
    step("binary PCS checks", &["v: opening"]),
];
const VERIFY_TOP_LABELS: &[&str] = &["v: fold", "v: gkr", "v: opening"];

/// Print one breakdown block under a `prove:`/`verify:` line: per-step
/// medians with their share of the block's median total. Rows whose labels
/// never fired are skipped, as are
/// near-zero derived rows.
fn print_steps(
    steps: &StepTable,
    rows: &[StepRow],
    top_labels: &[&str],
    rep_totals: &[f64],
    total_med: f64,
    decimals: usize,
) {
    if steps.0.is_empty() {
        return;
    }
    let reps = rep_totals.len();
    let pct = |ms: f64| {
        if total_med > 0.0 {
            ms / total_med * 100.0
        } else {
            0.0
        }
    };
    let line = |sub: bool, name: &str, ms: f64| {
        let (indent, width) = if sub { ("      · ", 42) } else { ("    ", 46) };
        println!(
            "{indent}{name:<width$} {ms:>10.decimals$} ms  {p:5.1}%",
            p = pct(ms)
        );
    };
    for row in rows {
        if !steps.has(row.plus) {
            continue;
        }
        let ms = steps.med(reps, row.plus, row.minus);
        if row.name.starts_with('(') && ms < 0.005 {
            continue;
        }
        line(row.sub, row.name, ms);
    }
    let unattributed: Vec<f64> = (0..reps)
        .map(|r| {
            let steps_sum: f64 = top_labels.iter().map(|l| steps.at(l, r)).sum();
            (rep_totals.get(r).copied().unwrap_or(0.0) - steps_sum).max(0.0)
        })
        .collect();
    line(false, "(unattributed)", median(unattributed));
}

fn usage() -> ! {
    eprintln!(
        "usage: bitz <n> [<t> <s>] [--threads N] [--reps R] \
         [--profile slim|slim3|fast|secure|custom:<r>:<k>[:<bits>]|udr:<r>:<k>[:<bits>] (default custom:1:4)] \
       bitz --sweep <lo>-<hi>|<n,n,…> [--threads N | --sweep-threads 1,10] [--reps R] [--profile P] [--cooldown S] [--rep-cooldown S] [--latex <path>]\n\
         (paper-table mode: one fresh process per n, then the LaTeX table is written —\n\
          default outputs/tables/raw-performance-table.tex in the crate; t/s do not apply)\n\
       bitz --mul <e> [--threads N] [--reps R] [--lambda 100|128] [--profile custom:<r>:<k>|udr]\n\
         (2^e u32×u32→u64 multiplications through the Spartan PIOP + BitZ opening; e ≥ 15)\n\
       bitz --mul-sweep <lo>-<hi>|<e,e,…> [--threads N] [--reps R] [--lambda L] [--cooldown S] [--latex <path>]\n\
         (paper-table mode for --mul; default outputs/tables/u32-mul-table.tex)\n\
         (n = t + s; W = cell width, power of two, default 1;\n\
          run with --release and --features unchecked for quotable numbers;\n\
          -C target-cpu=native is load-bearing on aarch64)"
    );
    exit(2)
}

struct Opts {
    n: usize,
    t: Option<usize>,
    s: Option<usize>,
    threads: Option<usize>,
    reps: usize,
    profile: String,

    /// `--sweep`: the shapes to run (each in a fresh child process) and the
    /// spec as typed (reproduced verbatim in the generated table's header).
    sweep: Option<(Vec<usize>, String)>,
    /// `--latex`: where the sweep writes the table.
    latex: Option<String>,
    /// `--mul <e>`: one u32-multiplication shape (`2^e` multiplications).
    mul: Option<usize>,
    /// `--mul-sweep`: the `--mul` shapes to run, plus the spec as typed.
    mul_sweep: Option<(Vec<usize>, String)>,
    /// `--lambda`: the IOP security profile for `--mul` (100 or 128).
    lambda: u32,
    /// `--cooldown <s>`: idle seconds between the sweep's child processes
    /// (lets the OS reclaim the previous shape's memory and the chip cool).
    cooldown_s: u64,
    /// `--sweep-threads 1,10`: run every `--sweep` shape once per listed
    /// thread count (one child per shape and count) and write the split
    /// table: per-step prover columns at the largest count, prover total and
    /// verifier at every count. Overrides `--threads` for `--sweep`.
    sweep_threads: Option<Vec<usize>>,
    /// `--rep-cooldown <s>`: idle seconds between the timed prove/verify reps
    /// inside a child (limits thermal creep on fanless machines; outside all
    /// timers).
    rep_cooldown_s: u64,
}

fn parse_args() -> Opts {
    let mut pos: Vec<usize> = Vec::new();
    let mut profile_explicit = std::env::var_os("BITZ_LIG_PROFILE").is_some();
    let mut o = Opts {
        n: 0,
        t: None,
        s: None,
        threads: None,
        reps: 3,
        profile: std::env::var("BITZ_LIG_PROFILE").unwrap_or_else(|_| "custom:1:4".into()),
        sweep: None,
        latex: None,
        mul: None,
        mul_sweep: None,
        lambda: 100,
        cooldown_s: 0,
        sweep_threads: None,
        rep_cooldown_s: 0,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => usage(),
            "--threads" | "-j" => {
                o.threads = Some(
                    args.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                );
            }
            "--reps" => {
                o.reps = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--profile" => {
                profile_explicit = true;
                o.profile = args.next().unwrap_or_else(|| usage());
            }
            "--sweep" => {
                let spec = args.next().unwrap_or_else(|| usage());
                let ns = parse_sweep_spec(&spec).unwrap_or_else(|e| {
                    eprintln!("--sweep {spec}: {e}");
                    usage()
                });
                o.sweep = Some((ns, spec));
            }
            "--latex" => {
                o.latex = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--mul" => {
                o.mul = Some(
                    args.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                );
            }
            "--mul-sweep" => {
                let spec = args.next().unwrap_or_else(|| usage());
                let es = parse_sweep_spec(&spec).unwrap_or_else(|e| {
                    eprintln!("--mul-sweep {spec}: {e}");
                    usage()
                });
                o.mul_sweep = Some((es, spec));
            }
            "--lambda" => {
                o.lambda = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--cooldown" => {
                o.cooldown_s = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--sweep-threads" => {
                let spec = args.next().unwrap_or_else(|| usage());
                let ts: Option<Vec<usize>> = spec
                    .split(',')
                    .map(|v| v.trim().parse::<usize>().ok().filter(|&t| t > 0))
                    .collect();
                match ts {
                    Some(mut ts) if !ts.is_empty() => {
                        ts.sort_unstable();
                        ts.dedup();
                        o.sweep_threads = Some(ts);
                    }
                    _ => usage(),
                }
            }
            "--rep-cooldown" => {
                o.rep_cooldown_s = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            other => match other.parse::<usize>() {
                Ok(v) => pos.push(v),
                Err(_) => {
                    eprintln!("unrecognized argument: {other}");
                    usage()
                }
            },
        }
    }
    if !profile_explicit && o.lambda != 100 && (o.mul.is_some() || o.mul_sweep.is_some()) {
        o.profile = bitz::ligerito_flock::LigeritoSelection::for_target(o.lambda as usize).name();
    }
    let modes = usize::from(o.sweep.is_some())
        + usize::from(o.mul.is_some())
        + usize::from(o.mul_sweep.is_some());
    if modes > 1 {
        eprintln!("--sweep, --mul and --mul-sweep are mutually exclusive");
        exit(2);
    }
    if modes == 1 {
        if !pos.is_empty() {
            eprintln!(
                "--sweep/--mul/--mul-sweep take their shape from their own argument; drop the positionals"
            );
            exit(2);
        }
    } else {
        match pos.as_slice() {
            [n] => o.n = *n,
            [n, t, s] => {
                o.n = *n;
                o.t = Some(*t);
                o.s = Some(*s);
            }
            _ => usage(),
        }
    }
    if o.reps == 0 {
        usage()
    }
    o
}

/// `--sweep` spec: comma-separated `n` values and/or inclusive `lo-hi`
/// ranges, e.g. `20-30`, `20,24,28`, `20-24,28`.
fn parse_sweep_spec(spec: &str) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let lo: usize = a
                .trim()
                .parse()
                .map_err(|_| format!("bad range start {a:?}"))?;
            let hi: usize = b
                .trim()
                .parse()
                .map_err(|_| format!("bad range end {b:?}"))?;
            if lo > hi {
                return Err(format!("empty range {part:?}"));
            }
            out.extend(lo..=hi);
        } else {
            out.push(part.parse().map_err(|_| format!("bad n {part:?}"))?);
        }
    }
    if out.is_empty() {
        return Err("no shapes".to_string());
    }
    Ok(out)
}

/// The reference split `t ≈ 0.6n`, clamped to the packing constraint
/// (`t ≥ 7`) and `s ≥ 1`.
fn default_split(n: usize) -> (usize, usize) {
    let t = ((3 * n).div_ceil(5)).max(7).min(n - 1);
    (t, n - t)
}

/// Round-by-round security summary of the resolved Ligerito config, in
/// flock's notion (`LigeritoSecurityConfig::validate`): total security is
/// the MINIMUM over levels and error terms, no whole-protocol union bound.
#[derive(Clone, Copy, Debug)]
struct LigSecurity {
    /// The config's declared target (`None` = ad-hoc test config).
    target_bits: Option<usize>,
    /// min over levels of min(query bits + query grinding, proximity-gap
    /// bits + fold grinding, OOD binding bits) — the three inequalities
    /// `validate()` enforces per level.
    achieved_bits: Option<f64>,
    /// L0's implicit post-commit binding (Johnson regime: the list is bound
    /// by the opening's own evaluation claim — `128 − log₂ list − log₂ μ`).
    l0_binding_bits: Option<f64>,
    /// Round 0 (the paper's out-of-domain sample) at the config's own
    /// target: executed with this grinding in the Johnson regime, skipped
    /// (`None`) at unique decoding.
    ood: Option<OodRoundParams>,
}

fn lig_security(cfg: &LigeritoSecurityConfig) -> LigSecurity {
    let mut min = f64::INFINITY;
    for lv in &cfg.levels {
        let q = lv.expected_eps_query_bits + lv.grinding_bits as f64;
        // The weakest fold round: a UDR level's taper leaves its last round
        // `k − 1` bits short of `fold_grinding_bits`.
        let pg = lv.expected_eps_pg_bits + weakest_fold_round_grinding(lv) as f64;
        min = min.min(q).min(pg);
        if let Some(ood) = lv.expected_eps_ood_bits {
            min = min.min(ood);
        }
    }
    LigSecurity {
        target_bits: Some(cfg.target_security_bits),
        achieved_bits: Some(min),
        l0_binding_bits: cfg.levels.first().and_then(|lv| lv.expected_eps_ood_bits),
        ood: u32::try_from(cfg.target_security_bits)
            .ok()
            .and_then(|target| ood_round_params(cfg, cfg.log_n, target)),
    }
}

impl LigSecurity {
    /// The `security:` line. The BitZ-side rounds are quoted from the
    /// protocol: GKR layer rounds are degree-3 sumcheck rounds (3/2^128),
    /// the reduce-to-MLE sumcheck is degree 2 (2/2^128), the ring-switch
    /// check is multilinear in the 7 packing variables (7/2^128); the
    /// evaluation prime is transcript-sampled after the commitment (so no
    /// separate Round-1 projection is needed), and Round 0 (the out-of-
    /// domain sample) runs exactly in the Johnson regime.
    fn describe(&self) -> String {
        let lig = match (self.target_bits, self.achieved_bits) {
            (Some(t), Some(a)) => format!(
                "ligerito round-by-round target {t} b, achieved {a:.1} b (min over levels/terms{})",
                self.l0_binding_bits
                    .map(|b| format!("; L0 post-commit binding {b:.1} b"))
                    .unwrap_or_default()
            ),
            _ => "ligerito ad-hoc test config (UNAUDITED — no security claim)".to_string(),
        };
        let round0 = match self.ood {
            Some(round) => format!(
                "Round 0 (OOD) executed with {} grinding bits",
                round.grinding_bits
            ),
            None => "Round 0 (OOD) skipped (unique-decoding opener)".to_string(),
        };
        format!(
            "{lig} | BitZ rounds: GKR 3/2^128, sumcheck 2/2^128, ring switch 7/2^128 | \
             q transcript-sampled after the commitment (Round 1 not needed) | {round0}"
        )
    }
}

/// Parse `<r0>:<k0>[:<bits>]` (the tail of `custom:`/`udr:`/`udrg:`).
fn parse_rk_bits(rest: &str) -> (usize, usize, Option<usize>) {
    let mut it = rest.split(':');
    let r0: usize = it
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or_else(|| usage());
    let k0: usize = it
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or_else(|| usage());
    let bits: Option<usize> = it.next().map(|x| x.parse().ok().unwrap_or_else(|| usage()));
    (r0, k0, bits)
}

/// Resolve the Ligerito `(ProverConfig, VerifierConfig)`, a short tag for
/// the header line, and the config's security summary. The validator-gated
/// families (`custom`/`udr`/`udrg`) need `m = m_p + 7 ≥ 20` (m = 20, 21 are
/// seeded from flock's m = 22 template — see `custom_johnson_config_bits`
/// / `custom_udr_config_bits`), the embedded profiles `m ≥ 22`; below that
/// unsupported requests are rejected without an ad-hoc fallback.
fn resolve_configs(
    m_p: usize,
    profile: &str,
) -> (
    (
        flock_core::pcs::ligerito::ProverConfig,
        flock_core::pcs::ligerito::VerifierConfig,
    ),
    String,
    LigSecurity,
    bitz::ligerito_flock::ResolvedLigerito,
) {
    let target = if profile == "secure" {
        128
    } else {
        profile
            .split(':')
            .nth(3)
            .map(|n| n.parse::<usize>().unwrap_or_else(|_| usage()))
            .unwrap_or(100)
    };
    let resolved = bitz::ligerito_flock::LigeritoSelection::parse(profile, target)
        .and_then(|selection| selection.resolve(m_p, target))
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            exit(2)
        });
    let sec = lig_security(resolved.security());
    (
        (resolved.prover().clone(), resolved.verifier().clone()),
        resolved.selection().name(),
        sec,
        resolved,
    )
}

fn main() {
    bitz::observability::install().expect("install Perfetto subscriber");
    let o = parse_args();

    // Paper-table modes: the parent only orchestrates child processes.
    if let Some((ns, spec)) = o.sweep.clone() {
        run_sweep(&o, &ns, &spec);
        return;
    }
    if let Some((es, spec)) = o.mul_sweep.clone() {
        run_mul_sweep(&o, &es, &spec);
        return;
    }

    #[cfg(feature = "parallel")]
    if let Some(th) = o.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(th.max(1))
            // 8 MB workers, as flock's `init_perf_thread_pool`: the prover
            // kernels nest large NEON frames under work stealing (virtual
            // reservation only; pages commit on touch).
            .stack_size(8 << 20)
            .build_global()
            .expect("rayon global pool (set --threads before any parallel work)");
    }

    if let Some(e) = o.mul {
        run_mul_shape(&o, e);
        return;
    }
    #[cfg(not(feature = "parallel"))]
    if o.threads.is_some_and(|th| th > 1) {
        eprintln!("note: built without the `parallel` feature — running serially");
    }

    // The single-claim path always reports the per-step prover/verifier
    // breakdown: turn the prof scaffold on before its first scope fires.
    // The timed medians then carry the ~µs/prove scope overhead — orders
    // below the run-to-run band.

    let (t, s) = match (o.t, o.s) {
        (Some(t), Some(s)) => (t, s),
        _ => default_split(o.n),
    };
    let w = 1usize;
    if t + s != o.n {
        eprintln!("t + s = {} ≠ n = {}", t + s, o.n);
        exit(2);
    }
    if t < 7 {
        eprintln!("packing needs t ≥ 7 (got t={t})");
        exit(2);
    }
    if s == 0 {
        eprintln!("need s ≥ 1");
        exit(2);
    }

    let p = IntegerMatrixLayout {
        row_vars: t,
        col_vars: s,
    };
    let q_bits = standalone_q_bits(&p);
    let m_p = packed_vars(&p);
    let lch = 1usize;
    let ((pc, _), lig_tag, lig_sec, resolved) = resolve_configs(m_p, &o.profile);
    // Round 0 (the out-of-domain sample) runs exactly when the opener sits
    // beyond unique decoding; its grinding tops the theorem's bound up to
    // the opener's own round-by-round target.
    let ood: Option<OodRoundParams> = lig_sec.ood;

    let threads_eff: usize = {
        #[cfg(feature = "parallel")]
        {
            rayon::current_num_threads()
        }
        #[cfg(not(feature = "parallel"))]
        {
            1
        }
    };
    println!(
        "bitz: n={} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}) | lig={lig_tag}@r1/{}k{} \
         merkle={} | threads={threads_eff} | int guards: {}",
        o.n,
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        hash_name(pc.merkle_hash),
        if bitz::utils::CHECKED {
            "CHECKED (build with --features unchecked)"
        } else {
            "unchecked"
        },
    );
    if lch > 1 {
        println!("note: {lch} mod-q chunks (t + W > 127 − q_bits) — prove scales ~×{lch}");
    }
    println!(
        "instance: q sampled after the commitment from the primes in [2^{}, 2^{q_bits}) | \
         point (r₁, r₂) ∈ F_q^{{t+s}} sampled after q | Round 0 (OOD): {}",
        q_bits - 1,
        match ood {
            Some(round) => format!("executed, {} grinding bits", round.grinding_bits),
            None => "skipped (unique-decoding opener)".to_string(),
        },
    );

    // Deterministic instance straight into per-column bit rows (the
    // memory-honest pattern — the u128 cell tensor never exists).
    let cell = |b: usize, c: usize| -> u128 {
        (p.cell_index(b, c) as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1
    };
    let words = p.rows().div_ceil(64);
    let mut rows: Vec<Vec<u64>> = (0..p.cols())
        .map(|c| {
            let mut wv = vec![0u64; words];
            for b in 0..p.rows() {
                wv[b >> 6] |= (cell(b, c) as u64) << (b & 63);
            }
            wv
        })
        .collect();
    // Commit: one tracked probe first (its own peak window; the hint that
    // stays live; excluded from the timing), then `reps` timed commits with
    // heap tracking OFF — on clones made outside the timer, hints dropped.
    reset_peak();
    let hint = commit_rs_ligerito_rows(&p, rows.clone(), &pc);
    let commit_peak = peak_mb();

    // The instance: replay the statement prefix once to learn the
    // transcript-sampled prime and point (every timed run re-derives them,
    // so the derivation IS inside the prover's and verifier's timers), then
    // the claimed μ from the set bits (O(popcount) mod-q adds, excluded).
    let opening = BitZOpener::new(
        p,
        BitZLigerito::Selected(resolved.selection()),
        resolved.security().target_security_bits as usize,
    )
    .unwrap_or_else(|error| {
        eprintln!("Round 0 does not match the Ligerito ladder: {error:?}");
        exit(2)
    });
    let y = bitz_opener::standalone_evaluation(&opening, &hint).expect("standalone claim");
    let prove_once =
        |hint: &FlockCommitHint| bitz_opener::prove_standalone(&opening, hint, y).expect("prove");
    let verify_once = |proof: &BitZOpeningProof| {
        bitz_opener::verify_standalone(&opening, &hint.commitment, y, proof)
    };
    set_heap_tracking(false);
    let mut commit_ms_v = Vec::with_capacity(o.reps);
    for i in 0..o.reps {
        let rows_i = if i + 1 == o.reps {
            std::mem::take(&mut rows)
        } else {
            rows.clone()
        };
        let (h, t0) = bitz::observability::measure(tracing::info_span!("bitz:h"), || {
            commit_rs_ligerito_rows(&p, rows_i, &pc)
        })
        .expect("measure completed operation");
        commit_ms_v.push(t0.as_secs_f64() * 1e3);
        black_box(&h);
    }
    set_heap_tracking(true);
    let commit_ms = median(commit_ms_v);
    println!(
        "commit:  {commit_ms:9.2} ms   peak {commit_peak:8.2} MB   (median of {})",
        o.reps
    );

    // Warm-up prove (excluded; tracked, so persistent scratch it allocates
    // is counted), the tracked peak probe (excluded), then the timed reps
    // with heap tracking OFF (see `PeakAlloc`). Draining the prof table
    // right after each prove and each verify splits every rep's scope
    // records cleanly into prover-side and verifier-side step samples.
    {
        let pr = prove_once(&hint);
        black_box(&pr);
    }

    reset_peak();
    {
        let pr = prove_once(&hint);
        black_box(&pr);
    }
    let prove_peak = peak_mb();

    set_heap_tracking(false);
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut prove_steps = StepTable::default();
    let mut verify_steps = StepTable::default();
    let mut last_proof = None;
    for rep in 0..o.reps {
        if rep > 0 && o.rep_cooldown_s > 0 {
            std::thread::sleep(std::time::Duration::from_secs(o.rep_cooldown_s));
        }
        let recording = bitz::observability::Recording::start(Vec::new()).expect("start CLI trial");
        bitz::bitz::record_phases(true);
        let proving = tracing::info_span!("cli:proving").entered();
        let proof = prove_once(&hint);
        drop(proving);
        let native_prove = take_native_phases();

        let verification = tracing::info_span!("cli:verification").entered();
        verify_once(&proof).expect("proof verifies");
        drop(verification);
        let native_verify = take_native_phases();
        bitz::bitz::record_phases(false);
        let intervals = recording.intervals().expect("query CLI trial");
        prove_ms.push(
            bitz::observability::duration(&intervals, "cli:proving")
                .unwrap()
                .as_secs_f64()
                * 1e3,
        );
        verify_ms.push(
            bitz::observability::duration(&intervals, "cli:verification")
                .unwrap()
                .as_secs_f64()
                * 1e3,
        );
        prove_steps.absorb(
            rep,
            bitz::observability::phase_totals(&intervals, "cli:proving")
                .unwrap()
                .into_iter()
                .chain(native_prove)
                .collect(),
        );
        verify_steps.absorb(
            rep,
            bitz::observability::phase_totals(&intervals, "cli:verification")
                .unwrap()
                .into_iter()
                .chain(native_verify)
                .collect(),
        );
        last_proof = Some(proof);
    }

    set_heap_tracking(true);

    let proof = last_proof.expect("reps ≥ 1");
    let bytes = proof.to_bytes().len();
    let lig_b = proof.transcript.hints.len();
    let zb = proof.to_bytes().len() - lig_b;
    let prove_med = median(prove_ms.clone());
    let verify_med = median(verify_ms.clone());
    println!(
        "prove:   {prove_med:9.2} ms   peak {prove_peak:8.2} MB   (median of {}, verified)",
        o.reps
    );
    print_steps(
        &prove_steps,
        PROVE_STEP_ROWS,
        PROVE_TOP_LABELS,
        &prove_ms,
        prove_med,
        2,
    );
    // The paper's prover buckets: per-rep bucket sums, then the median.
    let gp_ms = prove_steps.med(o.reps, PAPER_GP_LABELS, &[]);
    let rs_ms = prove_steps.med(o.reps, PAPER_RS_LABELS, &[]);
    let lig_ms = prove_steps.med(o.reps, PAPER_LIG_LABELS, &[]);
    let residual_ms = prove_med - (gp_ms + rs_ms + lig_ms);
    let pct = |ms: f64| {
        if prove_med > 0.0 {
            ms / prove_med * 100.0
        } else {
            0.0
        }
    };
    println!(
        "    paper buckets: grand products {gp_ms:.2} ms ({:.1}%) | ring switch (incl. sumcheck) \
         {rs_ms:.2} ms ({:.1}%) | Ligerito open {lig_ms:.2} ms ({:.1}%) | residual {residual_ms:.2} ms",
        pct(gp_ms),
        pct(rs_ms),
        pct(lig_ms),
    );
    println!("verify:  {verify_med:9.2} ms");
    print_steps(
        &verify_steps,
        VERIFY_STEP_ROWS,
        VERIFY_TOP_LABELS,
        &verify_ms,
        verify_med,
        3,
    );
    println!(
        "proof:   {:9.1} KiB  (native transcript and framing {:.1} | Ligerito hints {:.1})",
        bytes as f64 / 1024.0,
        zb as f64 / 1024.0,
        lig_b as f64 / 1024.0,
    );
    println!("security: {}", lig_sec.describe());
    let result = CliResult {
        ligerito: resolved.report(&o.profile, ood),
        n: o.n,
        t,
        s,
        w,
        m_p,
        chunks: lch,
        lig: lig_tag.clone(),
        lig_hash: hash_name(pc.merkle_hash),
        lig_target_bits: lig_sec.target_bits,
        lig_achieved_bits: lig_sec.achieved_bits,
        lig_l0_bits: lig_sec.l0_binding_bits,
        q_lo_log2: q_bits - 1,
        q_bits,
        ood_bits: ood.map(|round| round.grinding_bits as usize),
        threads: threads_eff,
        reps: o.reps,
        commit_ms,
        commit_peak_mb: commit_peak,
        prove_ms: prove_med,
        prove_gp_ms: gp_ms,
        prove_rs_ms: rs_ms,
        prove_lig_ms: lig_ms,
        prove_residual_ms: residual_ms,
        prove_peak_mb: prove_peak,
        verify_ms: verify_med,
        proof_bytes: bytes,
        // Host-codec framing (a few length prefixes) counts as non-Ligerito,
        // so the split sums to the total exactly.
        proof_nonlig_bytes: bytes - lig_b,
        proof_lig_bytes: lig_b,
        peak_rss_bytes: process_peak_rss_bytes(),
    };
    println!("{}", result.to_line());
}

// ---------------------------------------------------------------------
// Paper buckets, the RESULT line, and the `--sweep` table mode
// ---------------------------------------------------------------------

/// Disjoint native prover buckets used by the CLI's published tables.
const PAPER_GP_LABELS: &[&str] = &["fold+images", "gkr"];
const PAPER_RS_LABELS: &[&str] = &["sumcheck", "sumcheck (sum)", "ring switch", "mc:ood"];
const PAPER_LIG_LABELS: &[&str] = &["ligerito"];

/// One single-claim run, as the `RESULT schema=bitz-cli/1` line carries it
/// (`docs/bench-schema.md`). All `*_ms` are medians over the timed reps;
/// `prove_residual_ms = prove_ms − (gp + rs + lig)` (signed).
#[derive(Clone, Debug)]
struct CliResult {
    ligerito: serde_json::Value,
    n: usize,
    t: usize,
    s: usize,
    w: usize,
    m_p: usize,
    chunks: usize,
    lig: String,
    lig_hash: String,
    lig_target_bits: Option<usize>,
    lig_achieved_bits: Option<f64>,
    lig_l0_bits: Option<f64>,
    /// The evaluation prime is transcript-sampled from `[2^q_lo_log2, 2^q_bits)`.
    q_lo_log2: usize,
    q_bits: usize,
    /// Round 0 (OOD sample): `Some(grinding bits)` when executed, `None`
    /// when the opener runs at unique decoding.
    ood_bits: Option<usize>,
    threads: usize,
    reps: usize,
    commit_ms: f64,
    commit_peak_mb: f64,
    prove_ms: f64,
    prove_gp_ms: f64,
    prove_rs_ms: f64,
    prove_lig_ms: f64,
    prove_residual_ms: f64,
    prove_peak_mb: f64,
    verify_ms: f64,
    proof_bytes: usize,
    proof_nonlig_bytes: usize,
    proof_lig_bytes: usize,
    /// High-water resident set of the child process in bytes (warm-up, peak
    /// probes and timed reps included); `None` for lines written before the
    /// key existed.
    peak_rss_bytes: Option<u64>,
}

const RESULT_SCHEMA: &str = "bitz-cli/2";

fn na_usize(v: Option<usize>) -> String {
    v.map_or_else(|| "na".to_string(), |x| x.to_string())
}
fn na_f64(v: Option<f64>) -> String {
    v.map_or_else(|| "na".to_string(), |x| format!("{x:.2}"))
}

impl CliResult {
    fn to_line(&self) -> String {
        format!(
            "RESULT schema={RESULT_SCHEMA} ligerito_hex={} n={} t={} s={} W={} m_p={} chunks={} lig={} lig_hash={} \
             lig_target_bits={} lig_achieved_bits={} lig_l0_bits={} q_lo_log2={} q_bits={} \
             ood_bits={} threads={} reps={} \
             commit_ms={:.3} commit_peak_mb={:.2} prove_ms={:.3} prove_gp_ms={:.3} \
             prove_rs_ms={:.3} prove_lig_ms={:.3} prove_residual_ms={:.3} prove_peak_mb={:.2} \
             verify_ms={:.3} proof_bytes={} proof_nonlig_bytes={} proof_lig_bytes={} peak_rss_bytes={}",
            bitz::ligerito_flock::ResolvedLigerito::encode_report(&self.ligerito),
            self.n,
            self.t,
            self.s,
            self.w,
            self.m_p,
            self.chunks,
            self.lig,
            self.lig_hash,
            na_usize(self.lig_target_bits),
            na_f64(self.lig_achieved_bits),
            na_f64(self.lig_l0_bits),
            self.q_lo_log2,
            self.q_bits,
            na_usize(self.ood_bits),
            self.threads,
            self.reps,
            self.commit_ms,
            self.commit_peak_mb,
            self.prove_ms,
            self.prove_gp_ms,
            self.prove_rs_ms,
            self.prove_lig_ms,
            self.prove_residual_ms,
            self.prove_peak_mb,
            self.verify_ms,
            self.proof_bytes,
            self.proof_nonlig_bytes,
            self.proof_lig_bytes,
            self.peak_rss_bytes
                .map_or_else(|| "na".to_string(), |b| b.to_string()),
        )
    }

    fn parse(line: &str) -> Result<Self, String> {
        let rest = line.strip_prefix("RESULT ").ok_or("not a RESULT line")?;
        let kv: HashMap<&str, &str> = rest
            .split_whitespace()
            .filter_map(|tok| tok.split_once('='))
            .collect();
        if kv.get("schema") != Some(&RESULT_SCHEMA) {
            return Err(format!("schema {:?} ≠ {RESULT_SCHEMA}", kv.get("schema")));
        }
        let raw = |k: &str| kv.get(k).copied().ok_or_else(|| format!("missing key {k}"));
        let num = |k: &str| -> Result<f64, String> {
            raw(k)?.parse::<f64>().map_err(|e| format!("{k}: {e}"))
        };
        let int = |k: &str| -> Result<usize, String> {
            raw(k)?.parse::<usize>().map_err(|e| format!("{k}: {e}"))
        };
        let opt_num = |k: &str| -> Result<Option<f64>, String> {
            let v = raw(k)?;
            if v == "na" {
                Ok(None)
            } else {
                v.parse().map(Some).map_err(|e| format!("{k}: {e}"))
            }
        };
        let opt_int = |k: &str| -> Result<Option<usize>, String> {
            let v = raw(k)?;
            if v == "na" {
                Ok(None)
            } else {
                v.parse().map(Some).map_err(|e| format!("{k}: {e}"))
            }
        };
        let ligerito: serde_json::Value =
            bitz::ligerito_flock::ResolvedLigerito::decode_report(raw("ligerito_hex")?)?;
        bitz::ligerito_flock::ResolvedLigerito::validate_report(&ligerito)?;
        Ok(CliResult {
            ligerito,
            n: int("n")?,
            t: int("t")?,
            s: int("s")?,
            w: int("W")?,
            m_p: int("m_p")?,
            chunks: int("chunks")?,
            lig: raw("lig")?.to_string(),
            lig_hash: raw("lig_hash")?.to_string(),
            lig_target_bits: opt_int("lig_target_bits")?,
            lig_achieved_bits: opt_num("lig_achieved_bits")?,
            lig_l0_bits: opt_num("lig_l0_bits")?,
            q_lo_log2: int("q_lo_log2")?,
            q_bits: int("q_bits")?,
            ood_bits: opt_int("ood_bits")?,
            threads: int("threads")?,
            reps: int("reps")?,
            commit_ms: num("commit_ms")?,
            commit_peak_mb: num("commit_peak_mb")?,
            prove_ms: num("prove_ms")?,
            prove_gp_ms: num("prove_gp_ms")?,
            prove_rs_ms: num("prove_rs_ms")?,
            prove_lig_ms: num("prove_lig_ms")?,
            prove_residual_ms: num("prove_residual_ms")?,
            prove_peak_mb: num("prove_peak_mb")?,
            verify_ms: num("verify_ms")?,
            proof_bytes: int("proof_bytes")?,
            proof_nonlig_bytes: int("proof_nonlig_bytes")?,
            proof_lig_bytes: int("proof_lig_bytes")?,
            peak_rss_bytes: match kv.get("peak_rss_bytes") {
                None | Some(&"na") => None,
                Some(v) => Some(
                    v.parse::<u64>()
                        .map_err(|e| format!("peak_rss_bytes: {e}"))?,
                ),
            },
        })
    }
}

/// High-water resident set of this process in bytes: `getrusage` reports
/// bytes on macOS; on Linux read `VmHWM`, since `getrusage` there can reflect
/// the pre-exec image. `None` when unavailable.
fn process_peak_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let kb: u64 = status
            .lines()
            .find_map(|l| l.strip_prefix("VmHWM:"))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()?;
        Some(kb * 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: getrusage writes a complete rusage on success; the return
        // code is checked before the value is read.
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: initialized by the successful call above.
        u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()
    }
}

/// Run one fresh child process per shape (the bench protocol's "one shape
/// per process"), streaming each child's output and collecting its `RESULT`
/// line. `label` names the shape in messages; `args` builds the child's
/// argument list for a shape.
fn run_children(
    exe: &Path,
    shapes: &[usize],
    label: &str,
    cooldown_s: u64,
    args: impl Fn(usize) -> Vec<String>,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(shapes.len());
    for (i, &x) in shapes.iter().enumerate() {
        if i > 0 && cooldown_s > 0 {
            println!("\n(cooldown {cooldown_s} s)");
            std::thread::sleep(std::time::Duration::from_secs(cooldown_s));
        }
        println!("\n=== {label} = {x} ===");
        let mut cmd = Command::new(exe);
        cmd.args(args(x));
        cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
        let mut child = cmd.spawn().unwrap_or_else(|e| {
            eprintln!("spawn {}: {e}", exe.display());
            exit(1)
        });
        let stdout = child.stdout.take().expect("piped stdout");
        let mut result: Option<String> = None;
        for line in BufReader::new(stdout).lines() {
            let line = line.unwrap_or_else(|e| {
                eprintln!("reading child stdout: {e}");
                exit(1)
            });
            if line.starts_with("RESULT ") {
                result = Some(line.clone());
            }
            println!("{line}");
        }
        let status = child.wait().expect("wait for child");
        if !status.success() {
            eprintln!("child for {label}={x} failed: {status}");
            exit(1);
        }
        lines.push(result.unwrap_or_else(|| {
            eprintln!("child for {label}={x} printed no RESULT line");
            exit(1)
        }));
    }
    lines
}

/// The child arguments shared by both sweep modes.
fn common_child_args(o: &Opts) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(th) = o.threads {
        v.push("--threads".to_string());
        v.push(th.to_string());
    }
    v.push("--reps".to_string());
    v.push(o.reps.to_string());
    v
}

fn current_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("current_exe: {e}");
        exit(1)
    })
}

/// `--sweep`: the raw-performance table (one child per `n`).
fn run_sweep(o: &Opts, ns: &[usize], spec: &str) {
    let exe = current_exe();
    let latex_path: PathBuf = o.latex.clone().map_or_else(
        || default_latex_path("raw-performance-table.tex"),
        PathBuf::from,
    );
    println!(
        "bitz sweep: n ∈ {{{}}} | reps={} | profile={} | threads={} | W={} | one fresh process per n | \
         table → {}",
        ns.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        o.reps,
        o.profile,
        o.sweep_threads
            .as_ref()
            .map(|ts| ts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","))
            .or_else(|| o.threads.map(|t| t.to_string()))
            .unwrap_or_else(|| "default".to_string()),
        1usize,
        latex_path.display(),
    );
    // One child per (n, thread count), interleaved by n, under the same
    // cooldown schedule; `--sweep-threads` overrides `--threads` per child.
    let thread_counts: Vec<Option<usize>> = match &o.sweep_threads {
        Some(ts) => ts.iter().map(|&t| Some(t)).collect(),
        None => vec![o.threads],
    };
    let cases: Vec<(usize, Option<usize>)> = ns
        .iter()
        .flat_map(|&n| thread_counts.iter().map(move |&t| (n, t)))
        .collect();
    let case_ids: Vec<usize> = (0..cases.len()).collect();
    let lines = run_children(&exe, &case_ids, "case", o.cooldown_s, |i| {
        let (n, threads) = cases[i];
        println!(
            "(n = {n}, threads = {})",
            threads.map_or_else(|| "default".to_string(), |t| t.to_string())
        );
        let mut common = common_child_args(o);
        if let Some(pos) = common.iter().position(|x| x == "--threads") {
            common.drain(pos..pos + 2);
        }
        let mut a = vec![n.to_string()];
        if let Some(t) = threads {
            a.push("--threads".to_string());
            a.push(t.to_string());
        }
        a.extend(common);
        if o.rep_cooldown_s > 0 {
            a.push("--rep-cooldown".to_string());
            a.push(o.rep_cooldown_s.to_string());
        }
        a.push("--profile".to_string());
        a.push(o.profile.clone());
        a
    });
    let rows: Vec<CliResult> = lines
        .iter()
        .map(|l| {
            CliResult::parse(l).unwrap_or_else(|e| {
                eprintln!("bad RESULT line: {e}");
                exit(1)
            })
        })
        .collect();
    print_sweep_summary(&rows);
    match write_latex_table(&latex_path, &rows, o, spec) {
        Ok(()) => println!("wrote LaTeX table: {}", latex_path.display()),
        Err(e) => {
            eprintln!("writing {}: {e}", latex_path.display());
            exit(1);
        }
    }
}

/// Default table location: `outputs/tables/<name>` in the crate.
fn default_latex_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("outputs")
        .join("tables")
        .join(name)
}

/// Machine/date/commit facts for a generated table's header (best effort).
struct Provenance {
    cpu: String,
    cores: String,
    mem_gb: String,
    date: String,
    commit: String,
    rustc: String,
}

fn probe_provenance() -> Provenance {
    let cpu = probe("sysctl", &["-n", "machdep.cpu.brand_string"])
        .unwrap_or_else(|| "unknown CPU".to_string());
    let ncpu = probe("sysctl", &["-n", "hw.ncpu"]).unwrap_or_else(|| "?".to_string());
    let pcores = probe("sysctl", &["-n", "hw.perflevel0.physicalcpu"]);
    let ecores = probe("sysctl", &["-n", "hw.perflevel1.physicalcpu"]);
    let mem_gb = probe("sysctl", &["-n", "hw.memsize"])
        .and_then(|m| m.parse::<f64>().ok())
        .map_or_else(
            || "?".to_string(),
            |b| format!("{:.0}", b / (1024.0 * 1024.0 * 1024.0)),
        );
    let cores = match (pcores, ecores) {
        (Some(p), Some(e)) => format!("{ncpu} cores: {p} performance + {e} efficiency"),
        _ => format!("{ncpu} cores"),
    };
    Provenance {
        cpu,
        cores,
        mem_gb,
        date: probe("date", &["-u", "+%Y-%m-%d"]).unwrap_or_else(|| "unknown date".to_string()),
        commit: if Path::new(env!("CARGO_MANIFEST_DIR")).join(".git").exists() {
            probe(
                "git",
                &[
                    "-C",
                    env!("CARGO_MANIFEST_DIR"),
                    "describe",
                    "--always",
                    "--dirty",
                    "--abbrev=9",
                ],
            )
            .unwrap_or_else(|| option_env!("BITZ_REVISION").unwrap_or("unknown").to_owned())
        } else {
            option_env!("BITZ_REVISION").unwrap_or("unknown").to_owned()
        },
        rustc: probe("rustc", &["--version"]).unwrap_or_else(|| "rustc ?".to_string()),
    }
}

/// The exact command that regenerates a table (`mode` = `--sweep` or
/// `--mul-sweep`), for the table header.
fn reproduce_cmdline(o: &Opts, mode: &str, spec: &str) -> String {
    use std::fmt::Write as _;
    let mut cmdline = format!(
        "RUSTFLAGS=\"-C target-cpu=native\" cargo run --release --features unchecked -- \\\n%       {mode} {spec}"
    );
    if let (true, Some(ts)) = (mode == "--sweep", &o.sweep_threads) {
        let list = ts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(cmdline, " --sweep-threads {list}");
    } else if let Some(th) = o.threads {
        let _ = write!(cmdline, " --threads {th}");
    }
    let _ = write!(cmdline, " --reps {} --profile {}", o.reps, o.profile);
    if mode != "--sweep" && o.lambda != 100 {
        let _ = write!(cmdline, " --lambda {}", o.lambda);
    }
    if o.cooldown_s > 0 {
        let _ = write!(cmdline, " --cooldown {}", o.cooldown_s);
    }
    if mode == "--sweep" && o.rep_cooldown_s > 0 {
        let _ = write!(cmdline, " --rep-cooldown {}", o.rep_cooldown_s);
    }
    if let Some(l) = &o.latex {
        let _ = write!(cmdline, " --latex {l}");
    }
    cmdline
}

fn print_sweep_summary(rows: &[CliResult]) {
    println!(
        "\nsweep summary (medians; ms unless noted; proof KB = 1000 B; total = commit + prove):"
    );
    println!(
        "  {:>3} {:>3} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8} {:>9} {:>7} {:>7}",
        "n",
        "thr",
        "commit",
        "prove",
        "total",
        "grand-pr",
        "ring-sw",
        "ligerito",
        "verify",
        "proofKB",
        "nonlig",
        "lig",
        "peakMB",
        "rssGB",
        "lig-b"
    );
    for r in rows {
        println!(
            "  {:>3} {:>3} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>8.3} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>7} {:>7}",
            r.n,
            r.threads,
            r.commit_ms,
            r.prove_ms,
            r.commit_ms + r.prove_ms,
            r.prove_gp_ms,
            r.prove_rs_ms,
            r.prove_lig_ms,
            r.verify_ms,
            r.proof_bytes as f64 / 1000.0,
            r.proof_nonlig_bytes as f64 / 1000.0,
            r.proof_lig_bytes as f64 / 1000.0,
            r.prove_peak_mb,
            r.peak_rss_bytes.map_or_else(
                || "na".to_string(),
                |b| format!("{:.2}", b as f64 / (1u64 << 30) as f64)
            ),
            na_f64(r.lig_achieved_bits),
        );
    }
}

/// Lower-case Merkle hash name for headers, RESULT lines and captions.
fn hash_name(h: flock_core::merkle::HashKind) -> String {
    format!("{h:?}").to_lowercase()
}

/// Best-effort shell probe for the table's provenance header (never fails
/// the run).
fn probe(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Milliseconds for the table: 2 decimals below 10, 1 below 100, none above.
fn fmt_ms(v: f64) -> String {
    if v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}
/// Kilobytes (1000 B) for the table: 1 decimal below 100, none above.
fn fmt_kb(bytes: usize) -> String {
    let kb = bytes as f64 / 1000.0;
    if kb >= 100.0 {
        format!("{kb:.0}")
    } else {
        format!("{kb:.1}")
    }
}
/// Gigabytes (2^30 B) for the table: 1 decimal from 10, 2 from 1, else 3.
fn fmt_gb_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / (1u64 << 30) as f64;
    if gb >= 10.0 {
        format!("{gb:.1}")
    } else if gb >= 1.0 {
        format!("{gb:.2}")
    } else {
        format!("{gb:.3}")
    }
}

/// Write the paper's raw-performance table. The file is self-documenting:
/// its header records the exact command, machine, date, commit, and every
/// child's RESULT line.
fn write_latex_table(path: &Path, rows: &[CliResult], o: &Opts, spec: &str) -> std::io::Result<()> {
    use std::fmt::Write as _;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let Provenance {
        cpu,
        cores,
        mem_gb,
        date,
        commit,
        rustc,
    } = probe_provenance();
    let thread_counts: Vec<usize> = {
        let mut v: Vec<usize> = rows.iter().map(|r| r.threads).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let split = thread_counts.len() > 1;
    let threads = thread_counts
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" and ");
    let hi = thread_counts.last().copied().unwrap_or(0);
    let split_note = if split {
        format!(
            " The per-step prover columns (\\emph{{commit}}, \\emph{{grand products}}, \\emph{{ring switch}}, \\emph{{Ligerito}}) are from the {hi}-thread runs; the prover \\emph{{total}} and the verifier time are given for {threads} threads; proof sizes do not depend on the thread count; \\emph{{peak mem.}} is the high-water resident set of the {hi}-thread child process, warm-up and peak probes included ($1$\\,GB $= 2^{{30}}$ bytes).{}",
            if o.rep_cooldown_s > 0 {
                format!(
                    " Timed repetitions are separated by ${}$\\,s of idle time to limit thermal throttling.",
                    o.rep_cooldown_s
                )
            } else {
                String::new()
            }
        )
    } else {
        String::new()
    };
    let reps = rows.first().map_or(o.reps, |r| r.reps);
    let cmdline = reproduce_cmdline(o, "--sweep", spec);
    let lig_tag = rows.first().map_or("?", |r| r.lig.as_str());
    let lig_hash = rows.first().map_or("?", |r| r.lig_hash.as_str());
    let hash_tex = match lig_hash {
        "blake3" => "BLAKE3".to_string(),
        "sha256" => "SHA-256".to_string(),
        other => other.to_string(),
    };
    let target = rows.iter().filter_map(|r| r.lig_target_bits).min();
    let achieved = rows
        .iter()
        .filter_map(|r| r.lig_achieved_bits)
        .fold(f64::INFINITY, f64::min);
    let n_lo = rows.iter().map(|r| r.n).min().unwrap_or(0);
    let n_hi = rows.iter().map(|r| r.n).max().unwrap_or(0);
    let q_bits_min = rows.iter().map(|r| r.q_bits).min().unwrap_or(0);
    let q_bits_max = rows.iter().map(|r| r.q_bits).max().unwrap_or(0);
    let q_desc = if q_bits_min == q_bits_max {
        format!("{q_bits_min}")
    } else {
        format!("{q_bits_min}..{q_bits_max}")
    };
    let ood_bits_min = rows.iter().filter_map(|r| r.ood_bits).min();
    let ood_bits_max = rows.iter().filter_map(|r| r.ood_bits).max();
    let ood_desc = match (ood_bits_min, ood_bits_max) {
        (Some(lo), Some(hi)) if lo == hi => format!("executed with {lo} grinding bits"),
        (Some(lo), Some(hi)) => {
            format!("executed with {lo}..{hi} grinding bits (per shape, ood_bits below)")
        }
        _ => "skipped (unique-decoding opener)".to_string(),
    };
    let q_tex = if q_bits_min == q_bits_max {
        format!(
            "a prime $q$ sampled uniformly (after the commitment) from $[2^{{{}}}, 2^{{{q_bits_min}}})$",
            q_bits_min - 1
        )
    } else {
        format!(
            "a prime $q$ sampled uniformly (after the commitment) from $[2^{{b-1}}, 2^{{b}})$, $b = \\min(113, 127 - \\log \\codedim_1 - W)$ at cell width $W = 1$ ($b = {q_bits_min}, \\ldots, {q_bits_max}$)"
        )
    };
    let round0_tex = match (ood_bits_min, ood_bits_max) {
        (Some(lo), Some(hi)) if lo == hi => {
            format!("Round 0 (the out-of-domain sample) is executed with ${lo}$ bits of grinding")
        }
        (Some(lo), Some(hi)) => format!(
            "Round 0 (the out-of-domain sample) is executed with $\\lceil \\log \\codedim - 23.7 \\rceil$ bits of grinding (${lo}$ to ${hi}$ over the table)"
        ),
        _ => "Round 0 is skipped (unique-decoding opener)".to_string(),
    };
    // Ligerito geometry from the profile string when it is `custom:<r>:<k>`.
    let lig_geometry = o
        .profile
        .strip_prefix("custom:")
        .map(|rest| {
            let (r0, k0, _) = parse_rk_bits(rest);
            format!(
                "over a Reed--Solomon code of rate $1/{}$ over $\\FF_{{2^{{128}}}}$ (initial folding of $2^{{{k0}}}$ rows, Johnson regime, {hash_tex} Merkle trees)",
                1usize << r0
            )
        })
        .unwrap_or_else(|| format!("(profile \\texttt{{{lig_tag}}})"));
    let security = match target {
        Some(t) if achieved.is_finite() => format!(
            "configured for ${t}$ bits of round-by-round security (the minimum over its levels and error terms is ${achieved:.1}$ bits)"
        ),
        _ => "in the UNAUDITED ad-hoc test configuration (no security claim)".to_string(),
    };

    let mut out = String::new();
    let _ = writeln!(
        out,
        "% Raw-performance table of BitZ (c:core_iop) — GENERATED FILE, do not edit by hand."
    );
    let _ = writeln!(
        out,
        "% Generated by `bitz --sweep` (src/bin/bitz.rs) on {date} (UTC) at {commit}; {rustc}."
    );
    let _ = writeln!(
        out,
        "% Regenerate (from the repo root; this file is overwritten):"
    );
    let _ = writeln!(out, "%   {cmdline}");
    let _ = writeln!(
        out,
        "% Machine: {cpu} ({cores}), {mem_gb} GB; {threads} rayon threads; medians of {reps} timed reps"
    );
    let _ = writeln!(
        out,
        "%   after one warm-up prove; every timed proof is verified; commit = median of {reps} commits; the table's prover"
    );
    let _ = writeln!(
        out,
        "%   Total = commit_ms + prove_ms (the Commit column sits inside the prover group)."
    );
    let _ = writeln!(
        out,
        "% Include with \\input{{raw-performance-table}} (relative to outputs/tables/)."
    );
    let _ = writeln!(
        out,
        "% Prover buckets (tracing span labels): grand products = {};",
        PAPER_GP_LABELS.join(" ")
    );
    let _ = writeln!(
        out,
        "%   ring switch incl. its sumcheck = {}; Ligerito = {}.",
        PAPER_RS_LABELS.join(" "),
        PAPER_LIG_LABELS.join(" ")
    );
    let _ = writeln!(
        out,
        "%   Bucket medians need not sum to the total median; the signed residual (transcript glue) is prove_residual_ms below."
    );
    let _ = writeln!(
        out,
        "% Proof split: non-Ligerito = integer folds + GKR messages + sumcheck messages + ring-switch message (+ host-codec framing);"
    );
    let _ = writeln!(
        out,
        "%   Ligerito = the serialized Ligerito proof. KB = 1000 bytes."
    );
    let _ = writeln!(
        out,
        "% Security: Ligerito ({lig_tag}, {lig_hash} Merkle trees) target {} bits round-by-round, achieved (min over rows/levels/terms) {}; per-row values in",
        na_usize(target),
        if achieved.is_finite() {
            format!("{achieved:.2}")
        } else {
            "na".into()
        }
    );
    let _ = writeln!(
        out,
        "%   lig_achieved_bits / lig_l0_bits (L0's implicit post-commit list binding). BitZ-side rounds: GKR 3/2^128, sumcheck 2/2^128,"
    );
    let _ = writeln!(
        out,
        "%   ring switch 7/2^128 (all ≥ 125 bits). The evaluation prime q is sampled from the transcript after the commitment,"
    );
    let _ = writeln!(
        out,
        "%   uniformly among the primes in [2^(b-1), 2^b), b = {q_desc} per shape (q_lo_log2/q_bits below; b = min(113, 127 - t - W), the"
    );
    let _ = writeln!(
        out,
        "%   one-chunk exponent-fold width), and the point (r1, r2) is sampled after q — the instance is derived inside the timers."
    );
    let _ = writeln!(
        out,
        "%   Round 0 (out-of-domain sampling) of c:core_iop: {ood_desc}. Round 1 (random prime projection) is not needed: q is"
    );
    let _ = writeln!(
        out,
        "%   already a transcript-sampled prime of the admissible size. Round 0 costs ride the ring-switch bucket (mc:ood)."
    );
    let _ = writeln!(out, "% RESULT lines (schema={RESULT_SCHEMA}):");
    for r in rows {
        let _ = writeln!(out, "% {}", r.to_line());
    }
    let _ = writeln!(out);
    if split {
        let mut ns: Vec<usize> = rows.iter().map(|r| r.n).collect();
        ns.sort_unstable();
        ns.dedup();
        let find = |n: usize, t: usize| rows.iter().find(|r| r.n == n && r.threads == t);
        let span = thread_counts.len();
        let bold = ">{\\bfseries}r";
        let thr = thread_counts
            .iter()
            .map(|t| format!("{t} thr"))
            .collect::<Vec<_>>()
            .join(" & ");
        let _ = writeln!(out, "\\begin{{table}}[H]");
        let _ = writeln!(out, "  \\centering");
        let _ = writeln!(out, "  \\footnotesize");
        let _ = writeln!(out, "  \\setlength{{\\tabcolsep}}{{3pt}}");
        // Bold columns: prover totals, verifier times and the proof total.
        let _ = writeln!(
            out,
            "  \\begin{{tabular}}{{@{{}}rrrrr{}{}rr>{{\\bfseries}}rr@{{}}}}",
            bold.repeat(span),
            bold.repeat(span)
        );
        let _ = writeln!(out, "    \\toprule");
        let _ = writeln!(
            out,
            "    & \\multicolumn{{4}}{{c}}{{Prover steps, {hi} thr (ms)}} & \\multicolumn{{{span}}}{{c}}{{Prover total (ms)}} & \\multicolumn{{{span}}}{{c}}{{Verifier (ms)}} & \\multicolumn{{3}}{{c}}{{Proof size (KB)}} & Peak mem. \\\\"
        );
        let _ = writeln!(
            out,
            "    \\cmidrule(lr){{2-5}} \\cmidrule(lr){{6-{}}} \\cmidrule(lr){{{}-{}}} \\cmidrule(lr){{{}-{}}}",
            5 + span,
            6 + span,
            5 + 2 * span,
            6 + 2 * span,
            8 + 2 * span
        );
        let _ = writeln!(
            out,
            "    $\\log_2 \\codedim$ & Commit & Grand prod. & Ring switch & Ligerito & {thr} & {thr} & Non-Lig. & Ligerito & Total & (GB) \\\\"
        );
        let _ = writeln!(out, "    \\midrule");
        for &n in &ns {
            let Some(r_hi) = find(n, hi) else { continue };
            for &t in &thread_counts {
                if let Some(r) = find(n, t) {
                    if r.proof_bytes != r_hi.proof_bytes {
                        eprintln!(
                            "warning: n={n}: proof bytes differ between {t} and {hi} threads ({} vs {})",
                            r.proof_bytes, r_hi.proof_bytes
                        );
                    }
                }
            }
            let mut cells = vec![
                n.to_string(),
                fmt_ms(r_hi.commit_ms),
                fmt_ms(r_hi.prove_gp_ms),
                fmt_ms(r_hi.prove_rs_ms),
                fmt_ms(r_hi.prove_lig_ms),
            ];
            for &t in &thread_counts {
                // Prover Total = the commitment plus the end-to-end prove (both medians).
                cells.push(
                    find(n, t)
                        .map_or_else(|| "--".to_string(), |r| fmt_ms(r.commit_ms + r.prove_ms)),
                );
            }
            for &t in &thread_counts {
                cells.push(find(n, t).map_or_else(|| "--".to_string(), |r| fmt_ms(r.verify_ms)));
            }
            cells.push(fmt_kb(r_hi.proof_nonlig_bytes));
            cells.push(fmt_kb(r_hi.proof_lig_bytes));
            cells.push(fmt_kb(r_hi.proof_bytes));
            cells.push(
                r_hi.peak_rss_bytes
                    .map_or_else(|| "--".to_string(), fmt_gb_bytes),
            );
            let _ = writeln!(out, "    {} \\\\", cells.join(" & "));
        }
        let _ = writeln!(out, "    \\bottomrule");
    } else {
        let _ = writeln!(out, "\\begin{{table}}[H]");
        let _ = writeln!(out, "  \\centering");
        let _ = writeln!(out, "  \\small");
        let _ = writeln!(out, "  \\setlength{{\\tabcolsep}}{{4.5pt}}");
        // Bold columns: prover Total (6), Verifier (7), proof-size Total (10).
        let _ = writeln!(
            out,
            "  \\begin{{tabular}}{{@{{}}rrrrr>{{\\bfseries}}r>{{\\bfseries}}rrr>{{\\bfseries}}r@{{}}}}"
        );
        let _ = writeln!(out, "    \\toprule");
        let _ = writeln!(
            out,
            "    & \\multicolumn{{5}}{{c}}{{Prover time (ms)}} & Verifier & \\multicolumn{{3}}{{c}}{{Proof size (KB)}} \\\\"
        );
        let _ = writeln!(out, "    \\cmidrule(lr){{2-6}} \\cmidrule(lr){{8-10}}");
        let _ = writeln!(
            out,
            "    $\\log_2 \\codedim$ & Commit & Grand prod. & Ring switch & Ligerito & Total & (ms) & Non-Lig. & Ligerito & Total \\\\"
        );
        let _ = writeln!(out, "    \\midrule");
        for r in rows {
            let _ = writeln!(
                out,
                "    {} & {} & {} & {} & {} & {} & {} & {} & {} & {} \\\\",
                r.n,
                fmt_ms(r.commit_ms),
                fmt_ms(r.prove_gp_ms),
                fmt_ms(r.prove_rs_ms),
                fmt_ms(r.prove_lig_ms),
                // Prover Total = the commitment plus the end-to-end prove (both medians).
                fmt_ms(r.commit_ms + r.prove_ms),
                fmt_ms(r.verify_ms),
                fmt_kb(r.proof_nonlig_bytes),
                fmt_kb(r.proof_lig_bytes),
                fmt_kb(r.proof_bytes),
            );
        }
        let _ = writeln!(out, "    \\bottomrule");
    }
    let _ = writeln!(out, "  \\end{{tabular}}");
    let _ = writeln!(
        out,
        "  \\caption{{Cost of \\ftwoz\\ (\\cref{{c:core_iop}}) for committing to $\\codedim = 2^{{n}}$ bits, $n = {n_lo}, \\ldots, {n_hi}$, and proving one claim $\\langle \\vv, \\bff\\rangle = \\mu$ over $\\FF_q$ for {q_tex}, $\\vv = \\eq(\\cdot, \\rr_1) \\otimes \\eq(\\cdot, \\rr_2)$ with $(\\rr_1, \\rr_2)$ sampled after $q$, and the tensor split $\\codedim_1 = 2^{{\\lceil 0.6\\, n\\rceil}}$, $\\codedim_1 \\cdot \\codedim_2 = \\codedim$ (\\cref{{s:instantiation}}). The commitment is opened with ring switching and Ligerito~\\cite{{ligerito}} {lig_geometry}, {security}; {round0_tex}; every other round (the GKR, the sumcheck reducing to MLE evaluation claims, the ring switch) has error at most $7 \\cdot 2^{{-128}}$. Prover columns: \\emph{{commit}} is the commitment to the $\\codedim$ bits; \\emph{{grand products}} is computing the integers $\\mu_j$ and the batched GKR for the $\\codedim_2$ grand products in the exponent (\\cref{{s:gkr_low_entropy}}); \\emph{{ring switch}} is the sumcheck reducing the GKR output claims to MLE evaluation claims together with the ring-switching step; \\emph{{Ligerito}} is the Ligerito opening; \\emph{{total}} is the commitment plus the end-to-end proving time (each entry is a median, so the parts need not add up exactly). \\emph{{Non-Ligerito}} proof bytes are the $\\mu_j$, the GKR and sumcheck messages, and the ring-switch message; KB $= 1000$ bytes.{split_note} {cpu} ({cores}), {mem_gb}\\,GB, {threads} threads; medians of {reps} runs after one warm-up.}}"
    );
    let _ = writeln!(out, "  \\label{{tab:bitz-raw-performance}}");
    let _ = writeln!(out, "\\end{{table}}");
    std::fs::write(path, out)
}

// ---------------------------------------------------------------------
// `--mul` / `--mul-sweep`: the u32 × u32 → u64 multiplication SNARK
// ---------------------------------------------------------------------

/// Root seed of `benches/mul_bitz.rs` (`common::seed` default), so the CLI
/// proves the same witnesses as the bench at every exponent.
const MUL_ROOT_SEED: u64 = 0x5533_326d_756c_0064;

/// Prover steps of the multiplication SNARK (paper §2.1 numbering, code
/// order). The Step-5 sub-rows are the paper buckets of the opening.
const MUL_PROVE_STEP_ROWS: &[StepRow] = &[
    step(
        "prime projection (step2:project_prove)",
        &["step2:project_prove"],
    ),
    step("Spartan PIOP (step3:piop_prove)", &["step3:piop_prove"]),
    step("bitification (step4:bitify_prove)", &["step4:bitify_prove"]),
    step("BitZ opening (step5:open_prove)", &["step5:open_prove"]),
    substep("grand products", PAPER_GP_LABELS, &[]),
    substep("ring switch (incl. sumcheck)", PAPER_RS_LABELS, &[]),
    substep("Ligerito open", PAPER_LIG_LABELS, &[]),
];
const MUL_PROVE_TOP_LABELS: &[&str] = &[
    "step2:project_prove",
    "step3:piop_prove",
    "step4:bitify_prove",
    "step5:open_prove",
];
const MUL_VERIFY_STEP_ROWS: &[StepRow] = &[
    step(
        "prime projection (step2:project_verify)",
        &["step2:project_verify"],
    ),
    step("Spartan PIOP (step3:piop_verify)", &["step3:piop_verify"]),
    step(
        "bitification (step4:bitify_verify)",
        &["step4:bitify_verify"],
    ),
    step("BitZ opening (step5:open_verify)", &["step5:open_verify"]),
    substep(
        "claim → row/col weights (bitz_prepare_verifier)",
        &["spartan-bitz:bitz_prepare_verifier"],
        &[],
    ),
    substep("integer fold checks", &["v: fold"], &[]),
    substep("GKR checks", &["v: gkr"], &[]),
    substep("binary PCS checks", &["v: opening"], &[]),
];
const MUL_VERIFY_TOP_LABELS: &[&str] = &[
    "step2:project_verify",
    "step3:piop_verify",
    "step4:bitify_verify",
    "step5:open_verify",
];

/// One `--mul` run, as the `RESULT schema=bitz-cli-mul/1` line carries it.
/// `prove_ms` is END TO END and includes the commitment (bench-schema
/// semantics); `prove_residual_ms = prove_ms − (commit + s2 + s3 + s4 + s5)`.
/// `witness_ms` is the witness generation from the operand pairs (median of
/// `reps`), excluded from `prove_ms`; `setup_ms` the one-time preparation.
#[derive(Clone, Debug)]
struct MulResult {
    ligerito: serde_json::Value,
    e: usize,
    multiplications: usize,
    n: usize,
    t: usize,
    s: usize,
    w: usize,
    chunks: usize,
    profile: String,
    lambda: u32,
    lambda_achieved: f64,
    lambda_bind: String,
    lig_target_bits: usize,
    q_lo_log2: usize,
    q_bits: usize,
    lig_log_inv_rate: usize,
    lig_initial_k: usize,
    lig_regime: String,
    lig_hash: String,
    threads: usize,
    reps: usize,
    witness_ms: f64,
    setup_ms: f64,
    commit_ms: f64,
    prove_ms: f64,
    s2_project_ms: f64,
    s3_piop_ms: f64,
    s4_bitify_ms: f64,
    s5_open_ms: f64,
    s5_gp_ms: f64,
    s5_rs_ms: f64,
    s5_lig_ms: f64,
    prove_residual_ms: f64,
    prove_peak_mb: f64,
    verify_ms: f64,
    proof_bytes: usize,
    proof_piop_bytes: usize,
    proof_open_bytes: usize,
    proof_open_nonlig_bytes: usize,
    proof_open_lig_bytes: usize,
}

const MUL_RESULT_SCHEMA: &str = "bitz-cli-mul/2";

impl MulResult {
    fn to_line(&self) -> String {
        format!(
            "RESULT schema={MUL_RESULT_SCHEMA} ligerito_hex={} e={} multiplications={} n={} t={} s={} W={} chunks={} \
             profile={} lambda={} lambda_achieved={:.2} lambda_bind={} lig_target_bits={} \
             q_lo_log2={} q_bits={} lig_log_inv_rate={} lig_initial_k={} lig_regime={} lig_hash={} \
             threads={} reps={} \
             witness_ms={:.3} setup_ms={:.3} commit_ms={:.3} prove_ms={:.3} s2_project_ms={:.3} \
             s3_piop_ms={:.3} s4_bitify_ms={:.3} s5_open_ms={:.3} s5_gp_ms={:.3} s5_rs_ms={:.3} \
             s5_lig_ms={:.3} prove_residual_ms={:.3} prove_peak_mb={:.2} verify_ms={:.3} \
             proof_bytes={} proof_piop_bytes={} proof_open_bytes={} proof_open_nonlig_bytes={} \
             proof_open_lig_bytes={}",
            bitz::ligerito_flock::ResolvedLigerito::encode_report(&self.ligerito),
            self.e,
            self.multiplications,
            self.n,
            self.t,
            self.s,
            self.w,
            self.chunks,
            self.profile,
            self.lambda,
            self.lambda_achieved,
            self.lambda_bind,
            self.lig_target_bits,
            self.q_lo_log2,
            self.q_bits,
            self.lig_log_inv_rate,
            self.lig_initial_k,
            self.lig_regime,
            self.lig_hash,
            self.threads,
            self.reps,
            self.witness_ms,
            self.setup_ms,
            self.commit_ms,
            self.prove_ms,
            self.s2_project_ms,
            self.s3_piop_ms,
            self.s4_bitify_ms,
            self.s5_open_ms,
            self.s5_gp_ms,
            self.s5_rs_ms,
            self.s5_lig_ms,
            self.prove_residual_ms,
            self.prove_peak_mb,
            self.verify_ms,
            self.proof_bytes,
            self.proof_piop_bytes,
            self.proof_open_bytes,
            self.proof_open_nonlig_bytes,
            self.proof_open_lig_bytes,
        )
    }

    fn parse(line: &str) -> Result<Self, String> {
        let rest = line.strip_prefix("RESULT ").ok_or("not a RESULT line")?;
        let kv: HashMap<&str, &str> = rest
            .split_whitespace()
            .filter_map(|tok| tok.split_once('='))
            .collect();
        if kv.get("schema") != Some(&MUL_RESULT_SCHEMA) {
            return Err(format!(
                "schema {:?} ≠ {MUL_RESULT_SCHEMA}",
                kv.get("schema")
            ));
        }
        let raw = |k: &str| kv.get(k).copied().ok_or_else(|| format!("missing key {k}"));
        let num = |k: &str| -> Result<f64, String> {
            raw(k)?.parse::<f64>().map_err(|e| format!("{k}: {e}"))
        };
        let int = |k: &str| -> Result<usize, String> {
            raw(k)?.parse::<usize>().map_err(|e| format!("{k}: {e}"))
        };
        let ligerito: serde_json::Value =
            bitz::ligerito_flock::ResolvedLigerito::decode_report(raw("ligerito_hex")?)?;
        bitz::ligerito_flock::ResolvedLigerito::validate_report(&ligerito)?;
        Ok(MulResult {
            ligerito,
            e: int("e")?,
            multiplications: int("multiplications")?,
            n: int("n")?,
            t: int("t")?,
            s: int("s")?,
            w: int("W")?,
            chunks: int("chunks")?,
            profile: raw("profile")?.to_string(),
            lambda: raw("lambda")?.parse().map_err(|e| format!("lambda: {e}"))?,
            lambda_achieved: num("lambda_achieved")?,
            lambda_bind: raw("lambda_bind")?.to_string(),
            lig_target_bits: int("lig_target_bits")?,
            q_lo_log2: int("q_lo_log2")?,
            q_bits: int("q_bits")?,
            lig_log_inv_rate: int("lig_log_inv_rate")?,
            lig_initial_k: int("lig_initial_k")?,
            lig_regime: raw("lig_regime")?.to_string(),
            lig_hash: raw("lig_hash")?.to_string(),
            threads: int("threads")?,
            reps: int("reps")?,
            witness_ms: num("witness_ms")?,
            setup_ms: num("setup_ms")?,
            commit_ms: num("commit_ms")?,
            prove_ms: num("prove_ms")?,
            s2_project_ms: num("s2_project_ms")?,
            s3_piop_ms: num("s3_piop_ms")?,
            s4_bitify_ms: num("s4_bitify_ms")?,
            s5_open_ms: num("s5_open_ms")?,
            s5_gp_ms: num("s5_gp_ms")?,
            s5_rs_ms: num("s5_rs_ms")?,
            s5_lig_ms: num("s5_lig_ms")?,
            prove_residual_ms: num("prove_residual_ms")?,
            prove_peak_mb: num("prove_peak_mb")?,
            verify_ms: num("verify_ms")?,
            proof_bytes: int("proof_bytes")?,
            proof_piop_bytes: int("proof_piop_bytes")?,
            proof_open_bytes: int("proof_open_bytes")?,
            proof_open_nonlig_bytes: int("proof_open_nonlig_bytes")?,
            proof_open_lig_bytes: int("proof_open_lig_bytes")?,
        })
    }
}

/// One end-to-end prove of the multiplication SNARK: bit-pack + BitZ commit
/// (Step 1, timed separately) followed by the combined Spartan + BitZ proof.
/// Timings are queried by the caller; memory passes use the same proof body.
fn mul_prove_e2e(
    relation: &PreparedRelation<MulLayout<u32>>,
    witness: &MulWitness<u32>,
) -> (Proof, FlockCommitHint) {
    let proving = tracing::info_span!("cli:mul.proving").entered();
    let commit = tracing::info_span!("cli:mul.commit").entered();
    let rows = witness.bitz_bit_rows();
    let hint = protocol::commit(relation, rows).unwrap_or_else(|err| {
        eprintln!("BitZ commitment failed: {err}");
        exit(1)
    });
    drop(commit);
    let mut tr = Blake3Transcript::new();
    let proof = protocol::prove(&mut tr, relation, witness, &hint).unwrap_or_else(|err| {
        eprintln!("combined prove failed: {err}");
        exit(1)
    });
    drop(proving);
    (proof, hint)
}

/// `--mul <e>`: dispatch on the compile-time security profile.
fn run_mul_shape(o: &Opts, e: usize) {
    match o.lambda {
        100 => mul_shape::<Lambda100>(o, e),
        128 => mul_shape::<Lambda128>(o, e),
        other => {
            eprintln!("--lambda must be 100 or 128 for --mul (got {other})");
            exit(2);
        }
    }
}

fn mul_shape<P: IopSecurityProfile>(o: &Opts, e: usize) {
    if !(15..=40).contains(&e) {
        eprintln!("--mul needs 15 ≤ e ≤ 40 (the combined proof requires at least 2^15 gate slots)");
        exit(2);
    }

    let multiplications = 1usize << e;
    let shape_seed = MUL_ROOT_SEED ^ (e as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);

    // The operand pairs are the application's input (drawn in the bench's
    // order, so the witness is the bench's); generating them is not timed.
    let inputs: Vec<(u32, u32)> = (0..multiplications)
        .map(|_| (rng.random::<u32>(), rng.random::<u32>()))
        .collect();
    drop(rng);

    // Witness generation (excluded from prove; reported on its own): the
    // products x·y and the block-aligned Spartan assignment from the operand
    // pairs. Deterministic and cheap, so it is timed like the proofs: one
    // warm-up, then the median of `reps` fresh constructions. The bit
    // packing of the assignment for the BitZ commitment is part of Step 1
    // (the Commit column), not of this.
    let gen_witness = || {
        MulWitness::<u32>::from_inputs(&inputs).unwrap_or_else(|err| {
            eprintln!("witness: {err}");
            exit(1)
        })
    };
    black_box(gen_witness());
    let mut witness_ms_v = Vec::with_capacity(o.reps);
    let mut witness = None;
    for _ in 0..o.reps.max(1) {
        let t0_recording =
            bitz::observability::Recording::start(Vec::new()).expect("start operation capture");
        let t0 = tracing::info_span!("bitz:t0").entered();
        let w = gen_witness();
        drop(t0);
        witness_ms_v.push(
            bitz::observability::duration(
                &t0_recording.intervals().expect("witness capture"),
                "bitz:t0",
            )
            .expect("witness duration")
            .as_secs_f64()
                * 1e3,
        );
        witness = Some(w);
    }
    let witness = witness.expect("reps ≥ 1");
    let witness_ms = median(witness_ms_v);
    drop(inputs);
    let layout = *witness.layout();
    let params = layout.bitz_params();

    // The Ligerito opener: the raw-performance table's Johnson geometry by
    // default (so the two paper tables share one opener), or the relation's
    // own validated-UDR default (`udr`, what the bench and the pins run).
    let lig =
        LigeritoSelection::parse(&o.profile, P::LIGERITO_TARGET_BITS).unwrap_or_else(|error| {
            eprintln!("{error}");
            exit(2)
        });
    let lig_tag = lig.name();

    // One-time public preprocessing (excluded from prove).
    let t0_recording =
        bitz::observability::Recording::start(Vec::new()).expect("start operation capture");
    let t0 = tracing::info_span!("bitz:t0").entered();
    let relation =
        PreparedRelation::<MulLayout<u32>>::new_with_profile_and_ligerito::<P>(layout, lig)
            .unwrap_or_else(|err| {
                eprintln!("relation preparation failed at profile {}: {err}", P::NAME);
                exit(1)
            });
    drop(t0);
    let setup_ms =
        bitz::observability::duration(&t0_recording.intervals().expect("setup capture"), "bitz:t0")
            .expect("setup duration")
            .as_secs_f64()
            * 1e3;
    let lig_regime = if relation.security().ood.is_some() {
        "johnson"
    } else {
        "udr"
    };
    let sec = relation.security();
    let q_bits = (u128::BITS - sec.projection_max.leading_zeros()) as usize;
    let q_lo_log2 = (u128::BITS - 1 - sec.projection_min.leading_zeros()) as usize;
    let chunks = 1usize;
    let threads_eff: usize = {
        #[cfg(feature = "parallel")]
        {
            rayon::current_num_threads()
        }
        #[cfg(not(feature = "parallel"))]
        {
            1
        }
    };
    println!(
        "bitz --mul: 2^{e} = {multiplications} u32×u32→u64 multiplications | BitZ n={} (t={}, s={}, W={}, \
         chunks={chunks}) | profile={} λ={} | lig={lig_tag} | q ∈ [2^{q_lo_log2}, 2^{q_bits}) sampled \
         after the commit | threads={threads_eff} | int guards: {}",
        params.row_vars + params.col_vars,
        params.row_vars,
        params.col_vars,
        1usize,
        sec.profile_name,
        sec.lambda,
        if bitz::utils::CHECKED {
            "CHECKED (build with --features unchecked)"
        } else {
            "unchecked"
        },
    );
    println!(
        "excluded from prove: witness generation {witness_ms:.2} ms (median of {}; products + Spartan \
         assignment from the operand pairs) | one-time setup {setup_ms:.1} ms",
        o.reps
    );

    // Warm-up (excluded; tracked) — also the first end-to-end correctness
    // check — then the tracked peak probe (excluded), then the timed reps
    // with heap tracking OFF (see `PeakAlloc`).
    {
        let (proof, hint) = mul_prove_e2e(&relation, &witness);
        let mut vt = Blake3Transcript::new();
        protocol::verify(&mut vt, &relation, &hint.commitment, &proof).unwrap_or_else(|err| {
            eprintln!("warm-up verification failed: {err}");
            exit(1)
        });
        black_box(&proof);
    }

    reset_peak();
    {
        let (peak_proof, peak_hint) = mul_prove_e2e(&relation, &witness);
        black_box(&peak_proof);
        black_box(&peak_hint);
    }
    let peak = peak_mb();

    set_heap_tracking(false);

    let mut commit_ms_v = Vec::with_capacity(o.reps);
    let mut prove_ms_v = Vec::with_capacity(o.reps);
    let mut verify_ms_v = Vec::with_capacity(o.reps);
    let mut psteps = StepTable::default();
    let mut vsteps = StepTable::default();
    let mut last: Option<(Proof, usize, usize, String)> = None;
    for rep in 0..o.reps {
        let recording =
            bitz::observability::Recording::start(Vec::new()).expect("start CLI mul trial");
        bitz::bitz::record_phases(true);
        let (proof, hint) = mul_prove_e2e(&relation, &witness);
        let native_prove = take_native_phases();

        let mut vt = Blake3Transcript::new();
        let verification = tracing::info_span!("cli:mul.verification").entered();
        protocol::verify(&mut vt, &relation, &hint.commitment, &proof).unwrap_or_else(|err| {
            eprintln!("verification failed: {err}");
            exit(1)
        });
        drop(verification);
        let native_verify = take_native_phases();
        bitz::bitz::record_phases(false);
        let intervals = recording.intervals().expect("query CLI mul trial");
        let prove_ms = bitz::observability::duration(&intervals, "cli:mul.proving")
            .unwrap()
            .as_secs_f64()
            * 1e3;
        let commit_ms = bitz::observability::duration(&intervals, "cli:mul.commit")
            .unwrap()
            .as_secs_f64()
            * 1e3;
        verify_ms_v.push(
            bitz::observability::duration(&intervals, "cli:mul.verification")
                .unwrap()
                .as_secs_f64()
                * 1e3,
        );
        psteps.absorb(
            rep,
            bitz::observability::phase_totals(&intervals, "cli:mul.proving")
                .unwrap()
                .into_iter()
                .chain(native_prove)
                .collect(),
        );
        vsteps.absorb(
            rep,
            bitz::observability::phase_totals(&intervals, "cli:mul.verification")
                .unwrap()
                .into_iter()
                .chain(native_verify)
                .collect(),
        );

        commit_ms_v.push(commit_ms);
        prove_ms_v.push(prove_ms);
        black_box(&proof);
        last = Some((
            proof,
            hint.commitment.params.log_inv_rate,
            hint.commitment.params.log_batch_size,
            hash_name(hint.commitment.params.merkle_hash),
        ));
    }
    set_heap_tracking(true);
    let (proof, lig_log_inv_rate, lig_initial_k, lig_hash) = last.expect("reps ≥ 1");

    // Bytes (the bench's accounting): Spartan payload + boundary nonces, and
    // the serialized BitZ opening split into non-Ligerito | Ligerito.
    let spartan_elements = proof.spartan_payload_elements();
    let boundary_nonces = proof.grinding_nonce_count(sec) - sec.native_grinding_nonce_count();
    let piop_bytes = spartan_elements * 16 + boundary_nonces * std::mem::size_of::<u64>();
    let open_bytes = proof.bitz().to_bytes().len();
    let open_lig_bytes = proof.bitz().transcript.hints.len();
    let total_bytes = piop_bytes + open_bytes;

    let commit_med = median(commit_ms_v.clone());
    let prove_med = median(prove_ms_v.clone());
    let verify_med = median(verify_ms_v.clone());
    let m = |labels: &[&str]| psteps.med(o.reps, labels, &[]);
    let s2 = m(&["step2:project_prove"]);
    let s3 = m(&["step3:piop_prove"]);
    let s4 = m(&["step4:bitify_prove"]);
    let s5 = m(&["step5:open_prove"]);
    let gp = m(PAPER_GP_LABELS);
    let rs = m(PAPER_RS_LABELS);
    let lig = m(PAPER_LIG_LABELS);
    let residual = prove_med - (commit_med + s2 + s3 + s4 + s5);

    println!(
        "commit:  {commit_med:9.2} ms   (median of {}; bit-pack + BitZ commit, inside prove)",
        o.reps
    );
    println!(
        "prove:   {prove_med:9.2} ms   peak {peak:8.2} MB   (median of {}, end-to-end incl. commit, verified)",
        o.reps
    );
    // Rep totals net of the commit, so "(unattributed)" is the glue only.
    let net: Vec<f64> = prove_ms_v
        .iter()
        .zip(&commit_ms_v)
        .map(|(p, c)| p - c)
        .collect();
    print_steps(
        &psteps,
        MUL_PROVE_STEP_ROWS,
        MUL_PROVE_TOP_LABELS,
        &net,
        prove_med,
        2,
    );
    println!(
        "    paper buckets: commit {commit_med:.2} | PIOP incl. projection {:.2} | bitify {s4:.2} | \
         grand products {gp:.2} | ring switch (incl. sumcheck) {rs:.2} | Ligerito {lig:.2} | \
         residual {residual:.2} ms",
        s2 + s3
    );
    println!("verify:  {verify_med:9.2} ms");
    print_steps(
        &vsteps,
        MUL_VERIFY_STEP_ROWS,
        MUL_VERIFY_TOP_LABELS,
        &verify_ms_v,
        verify_med,
        3,
    );
    println!(
        "proof:   {:9.1} KB  = piop {:.1} + open {:.1} (non-Ligerito {:.1} | Ligerito {:.1})",
        total_bytes as f64 / 1e3,
        piop_bytes as f64 / 1e3,
        open_bytes as f64 / 1e3,
        (open_bytes - open_lig_bytes) as f64 / 1e3,
        open_lig_bytes as f64 / 1e3,
    );
    println!(
        "security: profile {} target λ={} b, achieved {:.1} b (binding term: {}) | ligerito \
         round-by-round target {} b ({lig_regime} regime, rate 1/{}, initial k={}, {lig_hash} Merkle) | \
         q ∈ [2^{q_lo_log2}, 2^{q_bits}) transcript-sampled after the commitment",
        sec.profile_name,
        sec.lambda,
        sec.accounting.achieved_bits(),
        sec.accounting.binding_term().name,
        sec.ligerito_target_bits,
        1usize << lig_log_inv_rate,
        lig_initial_k,
    );
    let r = MulResult {
        ligerito: relation
            .ligerito_configuration()
            .report(&o.profile, sec.ood),
        e,
        multiplications,
        n: params.row_vars + params.col_vars,
        t: params.row_vars,
        s: params.col_vars,
        w: 1usize,
        chunks,
        profile: sec.profile_name.to_string(),
        lambda: sec.lambda,
        lambda_achieved: sec.accounting.achieved_bits(),
        lambda_bind: sec.accounting.binding_term().name.to_string(),
        lig_target_bits: sec.ligerito_target_bits,
        q_lo_log2,
        q_bits,
        lig_log_inv_rate,
        lig_initial_k,
        lig_regime: lig_regime.to_string(),
        lig_hash,
        threads: threads_eff,
        reps: o.reps,
        witness_ms,
        setup_ms,
        commit_ms: commit_med,
        prove_ms: prove_med,
        s2_project_ms: s2,
        s3_piop_ms: s3,
        s4_bitify_ms: s4,
        s5_open_ms: s5,
        s5_gp_ms: gp,
        s5_rs_ms: rs,
        s5_lig_ms: lig,
        prove_residual_ms: residual,
        prove_peak_mb: peak,
        verify_ms: verify_med,
        proof_bytes: total_bytes,
        proof_piop_bytes: piop_bytes,
        proof_open_bytes: open_bytes,
        proof_open_nonlig_bytes: open_bytes - open_lig_bytes,
        proof_open_lig_bytes: open_lig_bytes,
    };
    println!("{}", r.to_line());
}

/// `--mul-sweep`: the multiplication table (one child per `e`).
fn run_mul_sweep(o: &Opts, es: &[usize], spec: &str) {
    let exe = current_exe();
    let latex_path: PathBuf = o
        .latex
        .clone()
        .map_or_else(|| default_latex_path("u32-mul-table.tex"), PathBuf::from);
    println!(
        "bitz mul-sweep: e ∈ {{{}}} (2^e u32×u32→u64 multiplications) | reps={} | λ={} | threads={} | \
         W={} | one fresh process per e | table → {}",
        es.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        o.reps,
        o.lambda,
        o.threads
            .map_or_else(|| "default".to_string(), |t| t.to_string()),
        1usize,
        latex_path.display(),
    );
    let lines = run_children(&exe, es, "e", o.cooldown_s, |e| {
        let mut a = vec!["--mul".to_string(), e.to_string()];
        a.extend(common_child_args(o));
        a.push("--lambda".to_string());
        a.push(o.lambda.to_string());
        a.push("--profile".to_string());
        a.push(o.profile.clone());
        a
    });
    let rows: Vec<MulResult> = lines
        .iter()
        .map(|l| {
            MulResult::parse(l).unwrap_or_else(|err| {
                eprintln!("bad RESULT line: {err}");
                exit(1)
            })
        })
        .collect();
    print_mul_summary(&rows);
    match write_mul_latex_table(&latex_path, &rows, o, spec) {
        Ok(()) => println!("wrote LaTeX table: {}", latex_path.display()),
        Err(err) => {
            eprintln!("writing {}: {err}", latex_path.display());
            exit(1);
        }
    }
}

fn print_mul_summary(rows: &[MulResult]) {
    println!(
        "\nmul-sweep summary (medians; ms unless noted; proof KB = 1000 B; prove = end to end incl. \
         commit, excl. witness generation):"
    );
    println!(
        "  {:>3} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8} {:>8} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>7}",
        "e",
        "witness",
        "commit",
        "piop",
        "bitify",
        "grand-pr",
        "ring-sw",
        "ligerito",
        "prove",
        "verify",
        "proofKB",
        "piopKB",
        "nonlig",
        "lig",
        "peakMB",
        "λ-ach"
    );
    for r in rows {
        println!(
            "  {:>3} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>9.2} {:>8.2} {:>8.2} {:>9.2} {:>8.2} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>7.1}",
            r.e,
            r.witness_ms,
            r.commit_ms,
            r.s2_project_ms + r.s3_piop_ms,
            r.s4_bitify_ms,
            r.s5_gp_ms,
            r.s5_rs_ms,
            r.s5_lig_ms,
            r.prove_ms,
            r.verify_ms,
            r.proof_bytes as f64 / 1000.0,
            r.proof_piop_bytes as f64 / 1000.0,
            r.proof_open_nonlig_bytes as f64 / 1000.0,
            r.proof_open_lig_bytes as f64 / 1000.0,
            r.prove_peak_mb,
            r.lambda_achieved,
        );
    }
}

/// Write the paper's integer-multiplication table (same self-documenting
/// header convention as the raw-performance table).
fn write_mul_latex_table(
    path: &Path,
    rows: &[MulResult],
    o: &Opts,
    spec: &str,
) -> std::io::Result<()> {
    use std::fmt::Write as _;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let Provenance {
        cpu,
        cores,
        mem_gb,
        date,
        commit,
        rustc,
    } = probe_provenance();
    let threads = rows.first().map_or(0, |r| r.threads);
    let reps = rows.first().map_or(o.reps, |r| r.reps);
    let cmdline = reproduce_cmdline(o, "--mul-sweep", spec);
    let first = rows.first();
    let e_lo = rows.iter().map(|r| r.e).min().unwrap_or(0);
    let e_hi = rows.iter().map(|r| r.e).max().unwrap_or(0);
    let lambda = first.map_or(o.lambda, |r| r.lambda);
    let achieved = rows
        .iter()
        .map(|r| r.lambda_achieved)
        .fold(f64::INFINITY, f64::min);
    let bind = first.map_or("?", |r| r.lambda_bind.as_str());
    // Term names carry underscores (`step5_3:…`): escape them for text mode.
    let bind_tex = bind.replace('_', "\\_");
    let profile = first.map_or("?", |r| r.profile.as_str());
    // The profile sets the prime interval [2^(b-1), 2^b) per shape (b is
    // capped by the one-chunk fold width 127 − t − W), so report the range.
    let q_bits_min = rows.iter().map(|r| r.q_bits).min().unwrap_or(0);
    let q_bits_max = rows.iter().map(|r| r.q_bits).max().unwrap_or(0);
    let q_bits_phrase = if q_bits_min == q_bits_max {
        format!("$b = {q_bits_max}$")
    } else {
        format!("$b$ between ${q_bits_min}$ and ${q_bits_max}$ depending on the shape")
    };
    let (lig_rate, lig_k, lig_target) = first.map_or((0, 0, 0), |r| {
        (
            1usize << r.lig_log_inv_rate,
            r.lig_initial_k,
            r.lig_target_bits,
        )
    });
    let lig_regime = first.map_or("?", |r| r.lig_regime.as_str());
    let lig_hash = first.map_or("?", |r| r.lig_hash.as_str());
    let regime_tex = match lig_regime {
        "johnson" => "Johnson regime",
        "udr" => "unique-decoding regime",
        other => other,
    };
    let hash_tex = match lig_hash {
        "blake3" => "BLAKE3".to_string(),
        "sha256" => "SHA-256".to_string(),
        other => other.to_string(),
    };
    let w = 1usize;
    let cell_words = 128usize; // committed cells per multiplication

    let mut out = String::new();
    let _ = writeln!(
        out,
        "% Integer-multiplication table of BitZ (c:iop_pimsat on u32 × u32 → u64) — GENERATED FILE, do not edit by hand."
    );
    let _ = writeln!(
        out,
        "% Generated by `bitz --mul-sweep` (src/bin/bitz.rs) on {date} (UTC) at {commit}; {rustc}."
    );
    let _ = writeln!(
        out,
        "% Regenerate (from the repo root; this file is overwritten):"
    );
    let _ = writeln!(out, "%   {cmdline}");
    let _ = writeln!(
        out,
        "% Machine: {cpu} ({cores}), {mem_gb} GB; {threads} rayon threads; medians of {reps} timed reps after one"
    );
    let _ = writeln!(
        out,
        "%   warm-up prove; every timed proof is verified; the one-time relation preparation (setup_ms) is excluded."
    );
    let _ = writeln!(
        out,
        "% Include with \\input{{u32-mul-table}} (relative to outputs/tables/)."
    );
    let _ = writeln!(
        out,
        "% Same witnesses as benches/mul_bitz.rs (root seed {MUL_ROOT_SEED:#018x}); prove_ms is END TO END and INCLUDES the commitment"
    );
    let _ = writeln!(
        out,
        "%   (docs/bench-schema.md semantics) but NOT the witness generation: witness_ms (the Witness column) is the median of {reps}"
    );
    let _ = writeln!(
        out,
        "%   constructions of the products + Spartan assignment from the operand pairs (drawing the pairs is untimed)."
    );
    let _ = writeln!(
        out,
        "%   Steps: s1 commit (bit-pack + BitZ commit), s2 prime projection, s3 Spartan PIOP,"
    );
    let _ = writeln!(
        out,
        "%   s4 bitification, s5 BitZ opening = grand products ({}) / ring switch incl. its sumcheck ({}) / Ligerito ({}).",
        PAPER_GP_LABELS.join(" "),
        PAPER_RS_LABELS.join(" "),
        PAPER_LIG_LABELS.join(" ")
    );
    let _ = writeln!(
        out,
        "%   Bucket medians need not sum to the total median; the signed residual is prove_residual_ms below."
    );
    let _ = writeln!(
        out,
        "% Proof split: PIOP = Spartan payload + grinding nonces; opening non-Ligerito = folds + GKR + sumcheck + ring-switch"
    );
    let _ = writeln!(
        out,
        "%   messages (+ codec framing); Ligerito = the serialized Ligerito proof. KB = 1000 bytes."
    );
    let _ = writeln!(
        out,
        "% Security: profile {profile} (target λ={lambda}), achieved (min over rows and terms) {achieved:.2} bits, binding term {bind};"
    );
    let _ = writeln!(
        out,
        "%   q sampled after the commitment from [2^(b-1), 2^b), b = {q_bits_min}..{q_bits_max} per shape (q_bits below); Ligerito {lig_regime} regime, rate 1/{lig_rate}, initial k={lig_k}, target {lig_target} bits, {lig_hash} Merkle trees."
    );
    let _ = writeln!(
        out,
        "% Table columns: Witness = witness_ms (outside the prover group, not in Total); PIOP = s2 + s3 + s4 (the bitification"
    );
    let _ = writeln!(
        out,
        "%   step s4 is ~µs and is folded in); prover Total = prove_ms; bold columns = prover Total, Verifier, proof Total."
    );
    let _ = writeln!(out, "% RESULT lines (schema={MUL_RESULT_SCHEMA}):");
    for r in rows {
        let _ = writeln!(out, "% {}", r.to_line());
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "\\begin{{table}}[H]");
    let _ = writeln!(out, "  \\centering");
    let _ = writeln!(out, "  \\footnotesize");
    let _ = writeln!(out, "  \\setlength{{\\tabcolsep}}{{3pt}}");
    // Columns: 1 log N, 2 Witness, 3–8 prover (Commit, PIOP, grand products,
    // ring switch, Ligerito, Total), 9 Verifier, 10–13 proof (PIOP, Non-Lig.,
    // Ligerito, Total). Bold: the two Total columns and the Verifier column
    // (`>{\bfseries}` needs `array`, which siunitx loads). The two widest
    // headers are two-line (`makecell`, bottom-aligned) so that 13 columns
    // fit the text width at this column separation.
    let _ = writeln!(
        out,
        "  \\begin{{tabular}}{{@{{}}rrrrrrr>{{\\bfseries}}r>{{\\bfseries}}rrrr>{{\\bfseries}}r@{{}}}}"
    );
    let _ = writeln!(out, "    \\toprule");
    let _ = writeln!(
        out,
        "    & Witness & \\multicolumn{{6}}{{c}}{{Prover time (ms)}} & Verifier & \\multicolumn{{4}}{{c}}{{Proof size (KB)}} \\\\"
    );
    let _ = writeln!(out, "    \\cmidrule(lr){{3-8}} \\cmidrule(lr){{10-13}}");
    let _ = writeln!(
        out,
        "    $\\log_2 N$ & (ms) & Commit & PIOP & \\makecell[b]{{Grand\\\\prod.}} & \\makecell[b]{{Ring\\\\switch}} & Ligerito & Total & (ms) & PIOP & Non-Lig. & Ligerito & Total \\\\"
    );
    let _ = writeln!(out, "    \\midrule");
    for r in rows {
        let _ = writeln!(
            out,
            "    {} & {} & {} & {} & {} & {} & {} & {} & {} & {} & {} & {} & {} \\\\",
            r.e,
            fmt_ms(r.witness_ms),
            fmt_ms(r.commit_ms),
            fmt_ms(r.s2_project_ms + r.s3_piop_ms + r.s4_bitify_ms),
            fmt_ms(r.s5_gp_ms),
            fmt_ms(r.s5_rs_ms),
            fmt_ms(r.s5_lig_ms),
            fmt_ms(r.prove_ms),
            fmt_ms(r.verify_ms),
            fmt_kb(r.proof_piop_bytes),
            fmt_kb(r.proof_open_nonlig_bytes),
            fmt_kb(r.proof_open_lig_bytes),
            fmt_kb(r.proof_bytes),
        );
    }
    let _ = writeln!(out, "    \\bottomrule");
    let _ = writeln!(out, "  \\end{{tabular}}");
    let _ = writeln!(
        out,
        "  \\caption{{Cost of proving $N = 2^{{n}}$ integer multiplications $x \\cdot y = z$, $n = {e_lo}, \\ldots, {e_hi}$, for random $32$-bit integers $x, y$ (so that $z$ is a $64$-bit integer) with \\cref{{c:iop_pimsat}}: one R1CS constraint per multiplication over $\\ZZ$, projected to a prime $q$ sampled after the commitment from an interval $[2^{{b-1}}, 2^{{b}})$ with {q_bits_phrase}, a Spartan PIOP over $\\FF_q$ (with a $3$-variable univariate skip), bitification, and the \\ftwoz\\ opening of the $128$ bits committed per multiplication ($\\codedim = 2^{{n+7}}$ bits in cells of $W = {w}$ bit{}, i.e.\\ ${cell_words}$ cells per multiplication). Security profile $\\lambda = {lambda}$: every round-by-round error is at most $2^{{-{achieved:.1}}}$, the binding term being \\texttt{{{bind_tex}}}; the commitment is opened with ring switching and Ligerito~\\cite{{ligerito}} over a Reed--Solomon code of rate $1/{lig_rate}$ over $\\FF_{{2^{{128}}}}$ (initial folding of $2^{{{lig_k}}}$ rows, {regime_tex}, {hash_tex} Merkle trees), configured for ${lig_target}$ bits — the same opener as \\cref{{tab:bitz-raw-performance}}. \\emph{{Witness}} is the witness generation (the products $z = x \\cdot y$ and the Spartan assignment) from the operand pairs; it is not part of the prover time. Prover columns: \\emph{{Commit}} is bit packing plus the commitment; \\emph{{PIOP}} is the prime projection, the Spartan PIOP and the (microsecond-scale) bitification step; \\emph{{grand products}}, \\emph{{ring switch}} and \\emph{{Ligerito}} are the three parts of the \\ftwoz\\ opening as in \\cref{{tab:bitz-raw-performance}}; \\emph{{Total}} is end to end and includes the commitment (each entry is a median, so the parts need not add up exactly). Proof columns: \\emph{{PIOP}} is the Spartan messages and grinding nonces; \\emph{{Non-Lig.}} is the opening's integer folds, GKR, sumcheck and ring-switch messages; \\emph{{Ligerito}} is the Ligerito proof; KB $= 1000$ bytes. {cpu} ({cores}), {mem_gb}\\,GB, {threads} threads; medians of {reps} runs after one warm-up; the one-time relation preparation is excluded.}}",
        if w == 1 { "" } else { "s" }
    );
    let _ = writeln!(out, "  \\label{{tab:bitz-u32-mul}}");
    let _ = writeln!(out, "\\end{{table}}");
    std::fs::write(path, out)
}

/// Drain recorded native durations outside the measured operation.
fn take_native_phases() -> Vec<(String, f64)> {
    bitz::bitz::take_phases()
        .into_iter()
        .map(|(label, time)| (label, time.as_secs_f64()))
        .collect()
}
