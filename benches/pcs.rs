//! F2Z PCS benchmark: commit / prove / verify wall-clock, serialized proof
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
//! - `F2Z_BENCH_SHAPES`: space/comma-separated `t:s:W` triples overriding the
//!   default shape list, e.g. `F2Z_BENCH_SHAPES="10:6:1 14:8:1"`.
//! - `F2Z_BENCH_REPS`: timing repetitions per shape (median reported;
//!   default 5).
//! - `F2Z_BENCH_FILL`: witness fill fraction in (0, 1] (default 1.0) — the
//!   trailing `(1 − fill)` of the columns are left ALL ZERO, i.e. the
//!   zero padding a witness of `N = fill·2^n` cells carries. With
//!   `F2Z_COL_ELIDE=1` (the default) the forest skips those trees; set
//!   `F2Z_COL_ELIDE=0` to measure the same instance un-elided. The
//!   printed `proof-fnv` is identical either way (byte-identity pin).
//! - `F2Z_LIG_PROFILE`: Ligerito profile selection. Named embedded profiles
//!   are available at `m = m_p + 7 ≥ 22` —
//!   `slim` (default; the embedded proof-size profile), `fast`
//!   (base RS rate 1/2), `secure` (120-bit UDR). Explicit `custom:*` profiles
//!   are mechanically derived and validator-gated at m=20..=35; named profiles
//!   below m=22 fall back to the ad-hoc test config. `slim3` and `r8` select
//!   an unaudited rate-1/8 probe at every size.
//! - `OBLONG_PROFILE=1`: record one extra prove's tagged phase breakdown.
//!   The bench prints both the coarse forest/open split and the disjoint
//!   detailed phase values consumed by `scripts/bench_csv.py`.
//! - `F2Z_BENCH_INTERVALS=1`: with `--features interval-profiling`, run one
//!   warmup plus `F2Z_BENCH_REPS` fresh end-to-end trials and write canonical
//!   `zkperf.trace/v1` JSONL to `F2Z_INTERVAL_OUT_DIR`.
//!
//! Protocol notes (from the zinc-plus measurement lore): idle the box first;
//! for quotable *time* numbers at big shapes run one shape per process (the
//! peak-memory numbers reset per shape and are fine in one process); quote
//! medians, expect ±5–15 % run-to-run.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use f2z::ligerito::{LOG_PACKING, packed_vars};
use f2z::ligerito_flock::custom_johnson_config;
use f2z::ligerito_flock::{
    LigConfig, commit_rs_ligerito_rows, lig_configs, prove_mle_eval_mod_q_ligerito,
    verify_mle_eval_mod_q_ligerito,
};
use f2z::pcs::{IntEvalParams, mod_q_num_chunks, smallest_generator};
use flock_core::pcs::ligerito::{LigeritoProfile, ProverConfig as LigPc, VerifierConfig as LigVc};

#[cfg(feature = "interval-profiling")]
mod pcs_intervals;

/// The bench's Ligerito config source: the audited embedded profile chosen
/// by `F2Z_LIG_PROFILE`. Explicit `custom:*` profiles mechanically derive and
/// validate their requested geometry at every supported benchmark size.
/// The returned requested profile and resolved geometry are printed separately;
/// the latter reads its base RS rate and initial k from the actual config.
/// `custom:<log_inv_rate>:<initial_k>` builds a Johnson config at that
/// geometry via [`custom_johnson_config`] (flock-validator-gated).
fn resolved_lig_config(tag: &str, pc: &LigPc) -> String {
    format!(
        "{tag}@r1/{}k{}",
        1usize << pc.log_inv_rates[0],
        pc.initial_k
    )
}

fn bench_lig_configs(m_p: usize) -> ((LigPc, LigVc), String, String) {
    let prof = std::env::var("F2Z_LIG_PROFILE").unwrap_or_default();
    if let Some(rest) = prof.strip_prefix("custom:") {
        let (r0, k0) = rest
            .split_once(':')
            .filter(|(_, k)| !k.contains(':'))
            .and_then(|(r, k)| Some((r.parse().ok()?, k.parse().ok()?)))
            .expect("F2Z_LIG_PROFILE must be custom:<log_inv_rate>:<initial_k>");
        let cfg = custom_johnson_config(m_p + LOG_PACKING, r0, k0);
        let pair = cfg
            .to_prover_verifier_configs()
            .expect("custom config pair");
        let resolved = resolved_lig_config(&format!("custom-k{k0}"), &pair.0);
        return (pair, prof, resolved);
    }
    let profile = if prof.is_empty() { "slim" } else { &prof };
    assert!(
        matches!(profile, "slim" | "slim3" | "fast" | "secure" | "r8"),
        "unknown F2Z_LIG_PROFILE: {profile}"
    );
    let (lig_cfg, tag): (LigConfig, &str) = if profile == "r8" || profile == "slim3" {
        // Base RS rate 1/8 via the ad-hoc UDR generator, at the embedded
        // profiles' interleaving (initial_k = 6). UNAUDITED perf probe (UDR
        // query counts, no grinding/OOD; ~121 L0 queries vs the Johnson
        // analysis' 60 at the same rate).
        (
            LigConfig::Adhoc {
                log_batch: 6,
                log_inv_rate: 3,
            },
            if profile == "r8" {
                "adhoc-udr"
            } else {
                "slim3-adhoc"
            },
        )
    } else if m_p + LOG_PACKING >= 22 {
        match profile {
            "fast" => (LigConfig::Embedded(LigeritoProfile::Fast), "fast"),
            "secure" => (LigConfig::Embedded(LigeritoProfile::Secure), "secure"),
            "slim" => (LigConfig::Embedded(LigeritoProfile::Slim), "slim"),
            "r8" | "slim3" => unreachable!("handled above"),
            _ => unreachable!("profile was validated above"),
        }
    } else {
        (
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
            "adhoc",
        )
    };
    let pair = lig_configs(m_p, lig_cfg).expect("lig cfg");
    let resolved = resolved_lig_config(tag, &pair.0);
    (pair, profile.to_owned(), resolved)
}

// Peak-heap tracker (wraps System): high-water mark of currently outstanding
// bytes. Negligible overhead (one relaxed atomic op per alloc/dealloc).
struct PeakAlloc;
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(c, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
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
fn peak_mb() -> f64 {
    PEAK.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}
fn live_mb() -> f64 {
    CUR.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

/// `𝔽_q`, `q = 2^100 − 15` — the reference evaluation field (any char ≠ 2
/// ring works; see the README).
const Q: u128 = (1u128 << 100) - 15;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Fq(u128);
impl From<u128> for Fq {
    fn from(v: u128) -> Self {
        Fq(v % Q)
    }
}
impl core::ops::Add for Fq {
    type Output = Fq;
    fn add(self, o: Fq) -> Fq {
        let s = self.0 + o.0;
        Fq(if s >= Q { s - Q } else { s })
    }
}
impl core::ops::Mul for Fq {
    type Output = Fq;
    fn mul(self, o: Fq) -> Fq {
        let (mut a, mut b, mut acc) = (self.0, o.0, 0u128);
        while b != 0 {
            if b & 1 == 1 {
                let s = acc + a;
                acc = if s >= Q { s - Q } else { s };
            }
            let d = a << 1;
            a = if d >= Q { d - Q } else { d };
            b >>= 1;
        }
        Fq(acc)
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn bench_shape(t: usize, s: usize, w: usize, reps: usize) {
    let alpha = smallest_generator();
    let q_bits = 100usize;
    let p = IntEvalParams { t, s, word_bits: w };
    let m_p = packed_vars(&p);
    let lch = mod_q_num_chunks(&p, q_bits);
    // The library's boundary, generalized by F2Z_LIG_PROFILE: the embedded
    // profiles at m = m_p + 7 ≥ 22, or validator-gated custom Johnson
    // geometries at every supported sweep size. Hardcoding the tiny ad-hoc
    // config at big shapes is catastrophic (n=28 commit measured 292 s
    // ad-hoc vs the embedded profile's sub-second).
    let ((pc, vc), requested_config, resolved_config) = bench_lig_configs(m_p);

    // Deterministic non-degenerate instance, generated STRAIGHT INTO the
    // per-column bit rows (`repack_leaf_bits` layout: bit `(b<<log₂W)|j`
    // of row `c` = bit `j` of cell `(b,c)`) — the `u128` cell tensor
    // (16 B per cell; 17 GB at n=30) never exists, mirroring the
    // upstream packed-transpose commit restructure.
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
    let fill: f64 = std::env::var("F2Z_BENCH_FILL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    assert!(
        fill > 0.0 && fill <= 1.0,
        "F2Z_BENCH_FILL must be in (0, 1]"
    );
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
    let rw_q: Vec<u128> = (0..p.rows())
        .map(|b| {
            (b as u128)
                .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                .wrapping_add(7)
                % Q
        })
        .collect();
    let cw: Vec<Fq> = (0..p.cols())
        .map(|c| Fq::from(((c as u128).wrapping_mul(5) & 7).wrapping_add(1)))
        .collect();
    // Claimed y from the SET BITS of the rows (O(popcount) field adds,
    // not O(2^n) muls): cell(b,c) contributes rw[b]·2^j per set bit j.
    let rw_fq: Vec<Fq> = rw_q.iter().map(|&x| Fq::from(x)).collect();
    let mut y = Fq::from(0u128);
    for (c, row) in rows.iter().enumerate() {
        let mut acc = Fq::from(0u128);
        for (wi, &word) in row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let i = (wi << 6) | bit;
                let (b, j) = (i >> log_w, i & (w - 1));
                let term = if j == 0 {
                    rw_fq[b]
                } else {
                    rw_fq[b] * Fq::from(1u128 << j)
                };
                acc = acc + term;
            }
        }
        y = y + cw[c] * acc;
    }

    let n = t + s;
    println!(
        "\n=== n={n} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}, lig={resolved_config}, data={} KiB, live={live}/{} elide={}) ===",
        (p.cells() * w).div_ceil(8) >> 10,
        p.cols(),
        std::env::var("F2Z_COL_ELIDE").unwrap_or_else(|_| "1".into())
    );
    println!(
        "  config-detail: requested_ligerito={requested_config} resolved_ligerito={resolved_config}"
    );

    if std::env::var_os("F2Z_BENCH_INTERVALS").is_some() {
        #[cfg(feature = "interval-profiling")]
        {
            let out_dir = std::path::PathBuf::from(
                std::env::var("F2Z_INTERVAL_OUT_DIR")
                    .unwrap_or_else(|_| "target/f2z-intervals".into()),
            );
            let threads = rayon::current_num_threads();
            let git_rev = pcs_intervals::git_revision();
            let capture_id = std::env::var("F2Z_INTERVAL_CAPTURE_ID")
                .expect("F2Z_INTERVAL_CAPTURE_ID is required in interval mode");
            eprintln!(
                "[f2z-interval]\tseries\tversion=1\tn={n}\tt={t}\ts={s}\tword_bits={w}\tm_p={m_p}\tchunks={lch}\tlig={resolved_config}\twarmups=1\treps={reps}"
            );

            for trial in 0..=reps {
                let is_warmup = trial == 0;
                let kind = if is_warmup { "warmup" } else { "sample" };
                let sample_index = if is_warmup { 0 } else { trial - 1 };
                eprintln!(
                    "[f2z-interval]\tsample-begin\tkind={kind}\ttrial={trial}\tsample_index={sample_index}"
                );
                let meta = pcs_intervals::RunMeta {
                    capture_id: &capture_id,
                    n,
                    t,
                    s,
                    word_bits: w,
                    m_p,
                    chunks: lch,
                    requested_config: &requested_config,
                    resolved_config: &resolved_config,
                    hash_id: pc.merkle_hash.as_str(),
                    threads,
                    git_rev: &git_rev,
                    kind,
                    index: sample_index,
                };
                // Fresh owned input for this trial, prepared outside the
                // traced root. The clone is benchmark harness setup, not PCS
                // commit work, and must not inflate either `bench.commit` or
                // the end-to-end protocol interval.
                let trial_rows = rows.clone();
                let ((hint, proof), summary, path) =
                    pcs_intervals::capture_trial(&out_dir, &meta, || {
                        let commit_scope = pcs_intervals::commit_scope();
                        let started = Instant::now();
                        let hint = commit_rs_ligerito_rows(&p, trial_rows, &pc);
                        let commit_ns = started.elapsed().as_nanos();
                        drop(commit_scope);

                        let prove_scope = pcs_intervals::prove_scope();
                        let mut prover_transcript = f2z::transcript::Blake3Transcript::new();
                        let started = Instant::now();
                        let proof = prove_mle_eval_mod_q_ligerito(
                            &mut prover_transcript,
                            &hint,
                            &p,
                            &rw_q,
                            q_bits,
                            alpha,
                            &pc,
                        );
                        let prove_ns = started.elapsed().as_nanos();
                        drop(prove_scope);

                        let serialize_scope = pcs_intervals::serialize_scope();
                        let started = Instant::now();
                        let serialized = proof.to_bytes();
                        let serialize_ns = started.elapsed().as_nanos();
                        drop(serialize_scope);

                        let artifact_scope = pcs_intervals::artifact_scope();
                        let proof_bytes = serialized.len();
                        let proof_fnv =
                            serialized
                                .iter()
                                .fold(0xcbf2_9ce4_8422_2325u64, |hash, &byte| {
                                    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
                                });

                        let (proof_parts, ligerito_bytes) =
                            f2z::ligerito_flock::mle_eval_mod_q_lig_size_breakdown(&proof);
                        let folds_bytes = proof_parts.v;
                        let merged_gkr_bytes = proof_parts
                            .forest_roots
                            .saturating_add(proof_parts.forest_sumchecks)
                            .saturating_add(proof_parts.forest_evals);
                        let presum_bytes = proof_parts.presum;
                        let ring_bytes = proof_parts.s_v;
                        let payload_bytes = folds_bytes
                            .saturating_add(merged_gkr_bytes)
                            .saturating_add(presum_bytes)
                            .saturating_add(ring_bytes)
                            .saturating_add(ligerito_bytes);
                        drop(artifact_scope);

                        let verify_scope = pcs_intervals::verify_scope();
                        let mut verifier_transcript = f2z::transcript::Blake3Transcript::new();
                        let started = Instant::now();
                        verify_mle_eval_mod_q_ligerito(
                            &mut verifier_transcript,
                            &hint.commitment,
                            &proof,
                            &p,
                            &rw_q,
                            &cw,
                            alpha,
                            y,
                            q_bits,
                            &vc,
                        )
                        .expect("interval trial verifies");
                        let verify_ns = started.elapsed().as_nanos();
                        drop(verify_scope);
                        let summary = pcs_intervals::TrialSummary {
                            commit_ns,
                            prove_ns,
                            serialize_ns,
                            verify_ns,
                            proof_bytes,
                            proof_fnv,
                            artifact_folds_bytes: folds_bytes,
                            artifact_merged_gkr_bytes: merged_gkr_bytes,
                            artifact_presum_bytes: presum_bytes,
                            artifact_ring_bytes: ring_bytes,
                            artifact_ligerito_bytes: ligerito_bytes,
                            artifact_framing_bytes: proof_bytes.saturating_sub(payload_bytes),
                        };
                        ((hint, proof), summary)
                    });
                eprintln!(
                    "[f2z-interval]\tsample-end\tkind={kind}\ttrial={trial}\tsample_index={sample_index}\tcommit_ns={}\tprove_ns={}\tserialize_ns={}\tverify_ns={}\tproof_bytes={}\tproof_fnv={:016x}\tartifact_folds_bytes={}\tartifact_merged_gkr_bytes={}\tartifact_presum_bytes={}\tartifact_ring_bytes={}\tartifact_ligerito_bytes={}\tartifact_framing_bytes={}\ttrace={}",
                    summary.commit_ns,
                    summary.prove_ns,
                    summary.serialize_ns,
                    summary.verify_ns,
                    summary.proof_bytes,
                    summary.proof_fnv,
                    summary.artifact_folds_bytes,
                    summary.artifact_merged_gkr_bytes,
                    summary.artifact_presum_bytes,
                    summary.artifact_ring_bytes,
                    summary.artifact_ligerito_bytes,
                    summary.artifact_framing_bytes,
                    path.display(),
                );
                black_box((&hint, &proof));
            }
            return;
        }
        #[cfg(not(feature = "interval-profiling"))]
        {
            panic!(
                "F2Z_BENCH_INTERVALS requires `cargo bench --features interval-profiling --bench pcs`"
            );
        }
    }

    // Commit: timed + its own peak window (the commitment/hint stays live).
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let commit_ms = t0.elapsed().as_secs_f64() * 1e3;
    let commit_peak = peak_mb();
    println!(
        "  commit:  {commit_ms:8.2} ms   peak {:8.2} MB   live-after {:6.2} MB",
        commit_peak,
        live_mb()
    );

    // Warm-up prove (excluded from stats).
    {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
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
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let t0 = Instant::now();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        prove_ms.push(t0.elapsed().as_secs_f64() * 1e3);

        let t1 = Instant::now();
        let ser = proof.to_bytes();
        ser_us.push(t1.elapsed().as_secs_f64() * 1e6);
        bytes = ser.len();
        // FNV-1a over the serialized proof: the byte-identity pin for
        // `F2Z_COL_ELIDE=0` vs `=1` at the same shape and fill.
        proof_fnv = ser.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });

        let t2 = Instant::now();
        let de = f2z::ligerito_flock::IntEvalRsLigModQProof::from_bytes(&ser).expect("codec");
        de_us.push(t2.elapsed().as_secs_f64() * 1e6);
        black_box(&de);

        let mut vt = f2z::transcript::Blake3Transcript::new();
        let t3 = Instant::now();
        verify_mle_eval_mod_q_ligerito(
            &mut vt,
            &hint.commitment,
            &proof,
            &p,
            &rw_q,
            &cw,
            alpha,
            y,
            q_bits,
            &vc,
        )
        .expect("verify");
        verify_ms.push(t3.elapsed().as_secs_f64() * 1e3);
    }

    // Peak + phase split over one prove (heap high-water; the commit hint
    // is live below it). Phase times ride the in-crate prof scaffold and
    // appear only under `OBLONG_PROFILE=1` (the timed medians above then
    // carry ~µs-scale scope overhead — enable it for breakdown runs, not
    // for headline timing); the proof-size split is always available.
    reset_peak();
    let _ = f2z::utils::prof::take_totals(); // drain the timed reps' records
    let split_started = Instant::now();
    let split_proof = {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        black_box(&proof);
        proof
    };
    let profiled_prove_ms = split_started.elapsed().as_secs_f64() * 1e3;
    let prove_peak = peak_mb();
    let phases = f2z::utils::prof::take_totals();
    let prove_p50_ms = median(prove_ms);
    let verify_p50_ms = median(verify_ms);
    let serialize_p50_us = median(ser_us);
    let deserialize_p50_us = median(de_us);

    println!(
        "  prove:   {:8.2} ms   peak {prove_peak:8.2} MB      (median of {reps})",
        prove_p50_ms
    );
    if !phases.is_empty() {
        let phase_ms = |labels: &[&str]| -> f64 {
            phases
                .iter()
                .filter(|(l, _)| labels.contains(l))
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
        let untagged_ms = (profiled_prove_ms - forest_ms - open_ms).max(0.0);
        println!(
            "  phases:  forest+presum {forest_ms:8.2} ms | ligerito open {open_ms:7.2} ms | untagged {untagged_ms:7.2} ms | total {profiled_prove_ms:8.2} ms   (one profiled prove)"
        );
        // These labels are a disjoint partition of the tagged work in this
        // prover path. Keep this line machine-oriented: bench_csv.py consumes
        // the stable key=value fields used by the detailed cost exporter.
        println!(
            "  phase-detail: forest_total_ms={forest_ms:.6} opening_total_ms={open_ms:.6} untagged_ms={untagged_ms:.6} profiled_prove_ms={profiled_prove_ms:.6} pack_ms={:.6} pow2_ms={:.6} forest_core_ms={:.6} fold_v_ms={:.6} presum_tables_ms={:.6} presum_run_ms={:.6} rings_ms={:.6} basis_combine_ms={:.6} ligerito_recursive_ms={:.6}",
            phase_ms(&["mc:pack"]).max(0.0),
            phase_ms(&["mc:pow2"]).max(0.0),
            phase_ms(&["mc:forest"]).max(0.0),
            phase_ms(&["mc:fold_v"]).max(0.0),
            phase_ms(&["mc:presum_tbls"]).max(0.0),
            phase_ms(&["mc:presum_run"]).max(0.0),
            phase_ms(&["mq:rings"]).max(0.0),
            phase_ms(&["mq:bcomb"]).max(0.0),
            phase_ms(&["mq:lig"]).max(0.0),
        );
    }
    println!("  verify:  {verify_p50_ms:8.2} ms");
    println!(
        "  proof:   {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs   fnv {proof_fnv:016x}",
        bytes as f64 / 1024.0,
        serialize_p50_us,
        deserialize_p50_us
    );
    println!(
        "  metric-detail: commit_ms={commit_ms:.6} commit_peak_mib={commit_peak:.6} prove_ms={prove_p50_ms:.6} prove_peak_mib={prove_peak:.6} verify_ms={verify_p50_ms:.6} proof_bytes={bytes} serialize_us={serialize_p50_us:.6} deserialize_us={deserialize_p50_us:.6}"
    );
    // Transmitted-payload accounting split (`mle_eval_mod_q_lig_size_breakdown`):
    // forest side = forest sumchecks/evals + chunk folds + pre-sumchecks;
    // open side = ring-switch `s_v` + the Ligerito proof.
    let (zb, lig_b) = f2z::ligerito_flock::mle_eval_mod_q_lig_size_breakdown(&split_proof);
    let forest_b = zb.total() - zb.s_v;
    let open_b = zb.s_v + lig_b;
    println!(
        "  split:   forest-side {:7.1} KiB | open-side {:7.1} KiB (s_v {:5.1} + lig {:7.1})",
        forest_b as f64 / 1024.0,
        open_b as f64 / 1024.0,
        zb.s_v as f64 / 1024.0,
        lig_b as f64 / 1024.0,
    );
}

fn main() {
    println!("F2Z PCS bench — commit/prove/verify + serialized size + peak heap per shape.");
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    println!("(target: aarch64 + neon — the NEON GF(2^128) pipeline is active)");

    let reps: usize = match std::env::var("F2Z_BENCH_REPS") {
        Ok(value) => value.parse().expect("F2Z_BENCH_REPS must be an integer"),
        Err(_) => 5,
    };
    // Default sweep: the reference W=1 shapes (t ≈ 0.6n, s small — the
    // proof-size-friendly split) + the 2-chunk W=32 regime, through
    // n = 28. The instance is generated straight into bit rows and
    // committed via `commit_rs_ligerito_rows` — no u128 cell tensor —
    // so peaks are the packed/forest scale (measured, M4: n=26 prove
    // peak 330 MB · n=28 1.28 GB · n=30 4.99 GB). n = 30/32 stay OUT
    // of the default sweep for time, not memory (n=30 proves ~15 s on
    // a 16 GB box, memory-pressure-shaded); run them per process:
    //   F2Z_BENCH_SHAPES="18:12:1"                        # n = 30, ~5 GB
    //   F2_FOREST_SCHEDULE=l8 F2Z_BENCH_SHAPES="19:13:1"  # n = 32, ~12 GB class
    // At n ≥ 26 run one shape per process for quotable numbers.
    let default_shapes: Vec<(usize, usize, usize)> = vec![
        (10, 6, 1),
        (12, 6, 1),
        (13, 7, 1),
        (14, 8, 1),
        (16, 10, 1), // n = 26
        (17, 11, 1), // n = 28
        (4, 8, 32),
    ];
    let shapes: Vec<(usize, usize, usize)> = match std::env::var("F2Z_BENCH_SHAPES") {
        Ok(v) => v
            .split([',', ' '])
            .filter(|x| !x.is_empty())
            .map(|trip| {
                let ps: Vec<usize> = trip
                    .split(':')
                    .map(|x| x.parse().expect("F2Z_BENCH_SHAPES: t:s:W triples"))
                    .collect();
                assert_eq!(ps.len(), 3, "F2Z_BENCH_SHAPES: t:s:W triples");
                (ps[0], ps[1], ps[2])
            })
            .collect(),
        Err(_) => default_shapes,
    };
    for &(t, s, w) in &shapes {
        bench_shape(t, s, w, reps);
    }
}
