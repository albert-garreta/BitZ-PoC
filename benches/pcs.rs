//! BitZ PCS benchmark: commit / prove / verify wall-clock, serialized proof
//! size, codec round-trip time, and peak heap per phase, per shape.
//!
//! Plain `harness = false` binary (no criterion — zero extra deps), in the
//! style of flock's `sha2_proof` bench; the peak-memory tracker reports the
//! live-heap high-water mark ("net outstanding bytes"), the same notion as
//! flock's benches and zinc-plus's `f2_int_ligerito_mem`, so numbers compare
//! directly across the three repos.
//!
//! Run:
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs
//! ```
//!
//! Knobs:
//! - `BITZ_BENCH_SHAPES`: space/comma-separated `t:s:W` triples overriding the
//!   default shape list, e.g. `BITZ_BENCH_SHAPES="10:6:1 14:8:1"`.
//! - `BITZ_BENCH_REPS`: timing repetitions per shape (median reported;
//!   default 5).
//! - `BITZ_BENCH_FILL`: witness fill fraction in (0, 1] (default 1.0) — the
//!   trailing `(1 − fill)` of the columns are left ALL ZERO, i.e. the
//!   zero padding a witness of `N = fill·2^n` cells carries. With
//!   `BITZ_COL_ELIDE=1` (the default) the forest skips those trees; set
//!   `BITZ_COL_ELIDE=0` to measure the same instance un-elided. The
//!   printed `proof-fnv` is identical either way (byte-identity pin).
//! - `BITZ_LIG_PROFILE`: Ligerito profile at `m = m_p + 7 ≥ 22` —
//!   `custom:1:4` (DEFAULT; validator-gated Johnson geometry at base RS
//!   rate 1/2, initial_k = 4), `slim` (embedded; fewer queries + 16-bit
//!   grinding at the same 100-bit target, the proof-size profile), `fast`
//!   (base RS rate 1/2), `secure` (120-bit UDR), or any
//!   `custom:<log_inv_rate>:<initial_k>[:<bits>]` — the optional `bits`
//!   sets the round-by-round security target (default 100; e.g.
//!   `custom:3:4:128` for a ~128-bit opener, queries/grinding/OOD
//!   re-solved and flock-validator-gated), or
//!   `udr:<log_inv_rate>:<initial_k>[:<bits>]` — queries-ONLY security
//!   (UDR regime, zero grinding of either kind, zero OOD; ceiling ≈115
//!   bits at n=22 / ≈109 at n=28 from the L0 UDR fold error).
//!   `udrg:3:4` uses matched UDR with fold grinding. Unsupported shapes
//!   are rejected; production requests never fall back to ad-hoc settings.
//!
//! Protocol notes (from the zinc-plus measurement lore): idle the box first;
//! for quotable *time* numbers at big shapes run one shape per process (the
//! peak-memory numbers reset per shape and are fine in one process); quote
//! medians, expect ±5–15 % run-to-run.

mod common;
use clap::builder::TypedValueParser;

use std::hint::black_box;

use bitz::ligerito::packed_vars;
use bitz::ligerito_flock::{OodRoundParams, commit_rs_ligerito_rows, standalone_q_bits};
use bitz::pcs::{IntegerMatrixLayout, smallest_generator};
use flock_core::pcs::ligerito::{ProverConfig as LigPc, VerifierConfig as LigVc};

#[derive(clap::Parser)]
struct Env {
    // n=30/32 stay outside the default sweep for runtime, not memory.
    // At n>=26, measure one shape per process for quotable timings.
    #[arg(long, env = "BITZ_BENCH_SHAPES", default_value = "13:7:1 14:8:1 16:10:1 17:11:1", value_parser = parse_shapes)]
    shapes: common::cli::List<(usize, usize, usize)>,
    #[arg(long, env = "BITZ_BENCH_FILL", default_value_t = 1.0,
        value_parser = str::parse::<f64>.try_map(|fill| {
            if fill > 0.0 && fill <= 1.0 { Ok(fill) } else { Err("expected a fraction in (0, 1]") }
        }))]
    fill: f64,

    #[arg(long, env = "BITZ_LIG_PROFILE", default_value = "custom:1:4")]
    ligerito: String,
}

fn parse_shapes(value: &str) -> Result<Vec<(usize, usize, usize)>, String> {
    common::cli::list::<String>(value)?
        .into_iter()
        .map(|shape| {
            let parts = shape
                .split(':')
                .map(str::parse::<usize>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            let [t, s, w]: [usize; 3] = parts
                .try_into()
                .map_err(|_| "expected t:s:W triple".to_owned())?;
            if w != 1 {
                return Err("Wfbitz commits bits; shape width must be 1".into());
            }
            Ok((t, s, w))
        })
        .collect()
}

/// Resolve the explicitly requested Ligerito policy, or Johnson+OOD by default.
fn bench_lig_configs(
    m_p: usize,
    request: &str,
) -> (
    (LigPc, LigVc),
    String,
    Option<OodRoundParams>,
    bitz::ligerito_flock::ResolvedLigerito,
) {
    let target = if request == "secure" {
        128
    } else {
        request
            .split(':')
            .nth(3)
            .map(|n| n.parse::<usize>().expect("invalid Ligerito target"))
            .unwrap_or(100)
    };
    let resolved = bitz::ligerito_flock::LigeritoSelection::parse(request, target)
        .and_then(|selection| selection.resolve(m_p, target))
        .expect("unsupported Ligerito configuration");
    let ood = resolved
        .round0(target as u32)
        .expect("Round-0 security preflight");
    (
        (resolved.prover().clone(), resolved.verifier().clone()),
        resolved.selection().name(),
        ood,
        resolved,
    )
}

// Peak-heap tracker (wraps System): high-water mark of currently outstanding
// bytes. Negligible overhead (one relaxed atomic op per alloc/dealloc).
#[global_allocator]
static ALLOCATOR: common::peak_memory::PeakAlloc = common::peak_memory::PeakAlloc;

fn reset_peak() {
    common::peak_memory::reset_peak();
}
fn peak_mb() -> f64 {
    common::peak_memory::peak_bytes() as f64 / (1024.0 * 1024.0)
}
fn live_mb() -> f64 {
    common::peak_memory::live_bytes() as f64 / (1024.0 * 1024.0)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn bench_shape(t: usize, s: usize, w: usize, reps: usize, env: &Env) {
    let alpha = smallest_generator();
    let p = IntegerMatrixLayout {
        row_vars: t,
        col_vars: s,
        word_bits: w,
    };
    let q_bits = standalone_q_bits(&p);
    let m_p = packed_vars(&p);
    let lch = 1usize;
    let setup_started_recording =
        bitz::observability::Recording::start(Vec::new()).expect("start operation capture");
    let setup_started = tracing::info_span!("pcs:setup_started").entered();
    let ((pc, vc), lig_tag, ood, resolved) = bench_lig_configs(m_p, &env.ligerito);
    println!(
        "LIGERITO_CONFIG {}",
        common::ligerito_report(&resolved, ood)
    );
    let setup_ms = {
        drop(setup_started);
        bitz::observability::duration(
            &setup_started_recording
                .intervals()
                .expect("complete operation capture"),
            "pcs:setup_started",
        )
        .expect("query completed operation")
    }
    .as_secs_f64()
        * 1e3;

    // Deterministic non-degenerate instance, generated STRAIGHT INTO the
    // per-column bit rows (`repack_leaf_bits` layout: bit `(b<<log₂W)|j`
    // of row `c` = bit `j` of cell `(b,c)`) — the `u128` cell tensor
    // (16 B per cell; 17 GB at n=30) never exists, mirroring the
    // upstream packed-transpose commit restructure.
    let witness_started_recording =
        bitz::observability::Recording::start(Vec::new()).expect("start operation capture");
    let witness_started = tracing::info_span!("pcs:witness_started").entered();
    let mask = if w >= 128 {
        u128::MAX
    } else {
        (1u128 << w) - 1
    };
    let cell = |b: usize, c: usize| -> u128 {
        (p.cell_index(b, c) as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask
    };
    let log_w = w.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let words = row_len.div_ceil(64);
    // Fill fraction: columns `live..2^s` stay ALL ZERO — exactly the
    // padding of a witness with N = fill·2^n cells (the column axis is
    // the high-order index, so a zero-padded witness ends in whole zero
    // columns). `y` below is derived from SET BITS, so it stays correct.
    let fill = env.fill;
    let live = (((p.cols() as f64) * fill).ceil() as usize).clamp(1, p.cols());
    let rows: Vec<Vec<u64>> = (0..p.cols())
        .map(|c| {
            let mut wv = vec![0u64; words];
            if c >= live {
                return wv;
            }
            for b in 0..p.rows() {
                let v = cell(b, c);
                for j in 0..w {
                    if (v >> j) & 1 == 1 {
                        let i = (b << log_w) | j;
                        wv[i >> 6] |= 1u64 << (i & 63);
                    }
                }
            }
            wv
        })
        .collect();
    let witness_ms = {
        drop(witness_started);
        bitz::observability::duration(
            &witness_started_recording
                .intervals()
                .expect("complete operation capture"),
            "pcs:witness_started",
        )
        .expect("query completed operation")
    }
    .as_secs_f64()
        * 1e3;

    let n = t + s;
    println!(
        "\n=== n={n} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}, lig={lig_tag}@r1/{}k{}, data={} KiB, live={live}/{} elide={}) ===",
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        (p.cells() * w).div_ceil(8) >> 10,
        p.cols(),
        std::env::var("BITZ_COL_ELIDE").unwrap_or_else(|_| "1".into())
    );

    // Commit: timed + its own peak window (the commitment/hint stays live).
    reset_peak();
    let (hint, t0) = bitz::observability::measure(tracing::info_span!("pcs:hint"), || {
        commit_rs_ligerito_rows(&p, rows, &pc)
    })
    .expect("measure completed operation");
    let commit_ms = t0.as_secs_f64() * 1e3;
    println!(
        "  commit:  {commit_ms:8.2} ms   peak {:8.2} MB   live-after {:6.2} MB",
        peak_mb(),
        live_mb()
    );

    // The instance: the transcript-sampled prime and point (replayed inside
    // every timed prove/verify), then the claimed μ from the SET BITS of the
    // committed rows (O(popcount) mod-q adds).
    use bitz::piop::spartan::protocol::wfbitz_opener::{
        self, WfbitzLigerito, WfbitzOpener, WfbitzOpeningProof,
    };
    let opening = WfbitzOpener::new(
        p,
        WfbitzLigerito::Selected(resolved.selection()),
        resolved.security().target_security_bits as usize,
    )
    .expect("standalone configuration");
    let y = wfbitz_opener::standalone_evaluation(&opening, &hint).expect("standalone claim");
    println!(
        "  instance: q ∈ [2^{}, 2^{q_bits}) transcript-sampled after the commitment; Round 0 (OOD): {}",
        q_bits - 1,
        match ood {
            Some(round) => format!("executed, {} grinding bits", round.grinding_bits),
            None => "skipped (unique-decoding opener)".to_string(),
        }
    );
    let prove_once = |hint: &bitz::ligerito_flock::FlockCommitHint| {
        wfbitz_opener::prove_standalone(&opening, hint, y).expect("prove")
    };
    let verify_once = |proof: &WfbitzOpeningProof| {
        wfbitz_opener::verify_standalone(&opening, &hint.commitment, y, proof)
    };

    // Warm-up prove (excluded from stats).
    {
        let proof = prove_once(&hint);
        black_box(&proof);
    }

    // Timed reps: prove / serialize / deserialize / verify.
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut ser_us = Vec::new();
    let mut de_us = Vec::new();
    let mut bytes = 0usize;
    let mut proof_fnv = 0u64;
    for _ in 0..reps {
        let (proof, t0) =
            bitz::observability::measure(tracing::info_span!("pcs:proof"), || prove_once(&hint))
                .expect("measure completed operation");
        prove_ms.push(t0.as_secs_f64() * 1e3);

        let (ser, t1) =
            bitz::observability::measure(tracing::info_span!("pcs:ser"), || proof.to_bytes())
                .expect("measure completed operation");
        ser_us.push(t1.as_secs_f64() * 1e6);
        bytes = ser.len();
        // FNV-1a over the serialized proof: the byte-identity pin for
        // `BITZ_COL_ELIDE=0` vs `=1` at the same shape and fill.
        proof_fnv = ser.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });

        let (de, t2) = bitz::observability::measure(tracing::info_span!("pcs:de"), || {
            WfbitzOpeningProof::from_bytes(&ser).expect("codec")
        })
        .expect("measure completed operation");
        de_us.push(t2.as_secs_f64() * 1e6);
        black_box(&de);

        let t3_recording =
            bitz::observability::Recording::start(Vec::new()).expect("start operation capture");
        let t3 = tracing::info_span!("pcs:t3").entered();
        verify_once(&proof).expect("verify");
        verify_ms.push(
            {
                drop(t3);
                bitz::observability::duration(
                    &t3_recording
                        .intervals()
                        .expect("complete operation capture"),
                    "pcs:t3",
                )
                .expect("query completed operation")
            }
            .as_secs_f64()
                * 1e3,
        );
    }

    // Peak + phase split over one prove (heap high-water; the commit hint
    // is live below it). Phase times come from completed Perfetto intervals;
    // trace extraction and querying happen after the heap snapshot.
    // Release flock's cross-prove scratch pool first: the reported prove
    // peak is the production single-prove shape (pool cold), not the
    // reps-warmed pool stacked under the forest. The timed medians above
    // deliberately keep the warm pool — that IS the steady-state timing.
    bitz::ligerito_flock::flock_scratch_clear();
    let recording =
        bitz::observability::Recording::start(Vec::new()).expect("start PCS phase probe");
    reset_peak();

    let split_proof = {
        let proof = prove_once(&hint);
        black_box(&proof);
        proof
    };
    let prove_peak = peak_mb();
    let phases =
        bitz::observability::totals(&recording.intervals().expect("query PCS phase probe"));

    let prove_median = median(prove_ms);
    let verify_median = median(verify_ms);
    println!("  prove:   {prove_median:8.2} ms   peak {prove_peak:8.2} MB      (median of {reps})");
    let mut forest_split = None;
    if !phases.is_empty() {
        let phase_ms = |labels: &[&str]| -> f64 {
            phases
                .iter()
                .filter(|(l, _)| labels.contains(&l.as_str()))
                .map(|(_, s)| s)
                .sum::<f64>()
                * 1e3
        };
        let forest_ms = phase_ms(&[
            "mc:pack",
            "mc:pow2",
            "mc:forest",
            "mc:fold_v",
            "mc:presum_tbls",
            "mc:presum_run",
        ]);
        let open_ms = phase_ms(&["mq:rings", "mq:bcomb", "mq:lig"]);
        println!(
            "  phases:  forest+presum {forest_ms:8.2} ms | ligerito open {open_ms:7.2} ms   (one profiled prove)"
        );
        forest_split = Some((forest_ms, open_ms));
    }
    println!("  verify:  {verify_median:8.2} ms");
    println!(
        "  proof:   {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs   fnv {proof_fnv:016x}",
        bytes as f64 / 1024.0,
        median(ser_us),
        median(de_us)
    );
    // Exact serialized streams, with framing charged to the native transcript.
    let lig_b = split_proof.hints.len();
    let zb = split_proof.to_bytes().len() - lig_b;
    println!(
        "  split:   native transcript and framing {:7.1} KiB | Ligerito hints {:7.1} KiB",
        zb as f64 / 1024.0,
        lig_b as f64 / 1024.0,
    );

    // Unified RESULT line (docs/bench-schema.md). PCS-only: the whole
    // measured prove is Steps 5.1–5.3, so steps 2/3/4/5.0 are `na` and the
    // end-to-end prover is commit + open. The forest/opener detail is
    // populated from one profiled prove.
    let na = common::StepMedians {
        total: 0.0,
        commit: None,
        project: None,
        piop: None,
        bitify: None,
        reduce: None,
        open: None,
        residual: 0.0,
        outer: None,
        bind: None,
        inner: None,
        forest: None,
        opener: None,
    };
    let report = common::BenchReport {
        bench: "pcs",
        shape: format!("{t}:{s}:{w}"),
        extra: vec![
            common::ligerito_identity(&resolved, ood),
            ("n".into(), n.to_string()),
            ("chunks".into(), lch.to_string()),
            ("lig".into(), lig_tag.clone()),
            ("fill".into(), format!("{fill}")),
        ],
        lambda: None,
        lambda_achieved: None,
        lambda_bind: None,
        threads: common::threads(),
        reps,
        seed: None,
        witness_ms,
        setup_ms,
        prover: common::StepMedians {
            total: commit_ms + prove_median,
            commit: Some(commit_ms),
            open: Some(prove_median),
            forest: forest_split.map(|(forest, _)| forest),
            opener: forest_split.map(|(_, open)| open),
            ..na
        },
        verifier: common::StepMedians {
            total: verify_median,
            open: Some(verify_median),
            ..na
        },
        proof: common::ProofBytes {
            piop: 0,
            open: bytes,
        },
    };
    println!("  {}", report.result_line());
}

fn main() {
    common::cli::EnvironmentCli::parse();
    let env: Env = common::cli::environment();
    let reps = common::reps(None, 5);
    bitz::observability::install().expect("install Perfetto subscriber");
    common::enforce_known_env();
    if std::env::var_os("BITZ_BENCH_LAMBDA").is_some() {
        common::warn(
            "BITZ_BENCH_LAMBDA is ignored by the PCS-only bench (no IOP security \
             profile here; the RESULT line reports lambda=na)",
        );
    }
    println!("BitZ PCS bench — commit/prove/verify + serialized size + peak heap per shape.");
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    println!("(target: aarch64 + neon — the NEON GF(2^128) pipeline is active)");

    for &(t, s, w) in &env.shapes {
        bench_shape(t, s, w, reps, &env);
    }
}
