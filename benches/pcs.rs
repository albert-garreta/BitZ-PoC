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
//! - `F2Z_LIG_PROFILE`: embedded Ligerito profile at `m = m_p + 7 ≥ 22` —
//!   `fast` (default; base RS rate 1/2), `slim` (base RS rate 1/4, fewer
//!   queries + 16-bit grinding, same 100-bit target), `secure` (120-bit
//!   UDR). Below m = 22 every profile falls back to the ad-hoc rate-1/4
//!   config (unaudited, test-only).
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
use f2z::ligerito_flock::{
    LigConfig, commit_rs_ligerito_rows, lig_configs, prove_mle_eval_mod_q_ligerito,
    verify_mle_eval_mod_q_ligerito,
};
use f2z::pcs::{IntEvalParams, mod_q_num_chunks, smallest_generator};
use flock_core::pcs::ligerito::{
    LigeritoProfile, LigeritoSecurityConfig, ProverConfig as LigPc, VerifierConfig as LigVc,
    embedded_security_config,
};

/// Build a Johnson-regime config for `(m, base rate 2^-r0, L0 interleave
/// 2^k0)` at the embedded profiles' 100-bit per-level target with 16-bit
/// query grinding, using flock's OWN machinery end to end: the embedded slim
/// config as the field template, `soundness.py`'s ladder rule (rate +1 per
/// level, 3-bit folds until the residual is ≤ 5), queries / fold-grinding /
/// OOD solved against `paper_predicted_bits` / `paper_predicted_ood_bits`
/// (the exact formulas `validate()` re-checks), and the whole config gated
/// by `LigeritoSecurityConfig::validate` before use. Nothing hand-picked.
fn custom_johnson_config(m: usize, r0: usize, k0: usize) -> LigeritoSecurityConfig {
    let slim = embedded_security_config(m, LigeritoProfile::Slim)
        .unwrap_or_else(|| panic!("no embedded slim template for m={m}"));
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim).expect("slim template validates");
    let log_n = cfg.log_n;
    assert!(k0 >= 1 && k0 < log_n, "custom initial_k out of range");
    let tmpl = cfg.levels[0].clone();

    // derive_ladder: (log_msg_cols, log_num_interleaved, k_recursive, rate).
    let mut shapes = vec![(log_n - k0, k0, k0, r0)];
    let mut n_run = log_n - k0;
    let mut rate = r0;
    while n_run > 5 {
        let kr = 3.min(n_run);
        rate += 1;
        shapes.push((n_run - kr, kr, kr, rate));
        n_run -= kr;
    }
    cfg.initial_k = k0;
    cfg.final_block.yr_log_n = n_run;
    cfg.levels = shapes
        .iter()
        .enumerate()
        .map(|(i, &(mc, il, kr, r))| {
            let mut lv = tmpl.clone();
            lv.log_inv_rate = r;
            lv.log_msg_cols = mc;
            lv.log_num_interleaved = il;
            lv.k_recursive = kr;
            lv.ood_samples = if i == 0 { 0 } else { 1 };
            // Queries: smallest Q whose predicted query-phase bits cover
            // target − query-grinding (validate()'s own gate).
            let need_q = (lv.target_security_bits - lv.grinding_bits) as f64;
            lv.queries = (1..=10_000)
                .find(|&q| {
                    lv.queries = q;
                    lv.paper_predicted_bits().1 + 1e-3 >= need_q
                })
                .expect("query search converges");
            let (pg, qb) = lv.paper_predicted_bits();
            lv.fold_grinding_bits =
                (lv.target_security_bits as f64 - pg).ceil().max(0.0) as usize;
            lv.expected_eps_pg_bits = pg;
            lv.expected_eps_query_bits = qb;
            // OOD must clear the target on its own (L0 uses the implicit
            // post-commit binding, s = 0; deeper levels escalate samples).
            loop {
                let ood = lv.paper_predicted_ood_bits().expect("johnson_ood prediction");
                if ood + 1e-3 >= lv.target_security_bits as f64 {
                    lv.expected_eps_ood_bits = Some(ood);
                    break;
                }
                lv.ood_samples += 1;
            }
            lv
        })
        .collect();
    cfg.validate().expect("custom config passes flock's validator");
    cfg
}

/// The bench's Ligerito config source: the audited embedded profile chosen
/// by `F2Z_LIG_PROFILE` at `m = m_p + 7 ≥ 22`, the ad-hoc rate-1/4 config
/// below — the same boundary as `sha_lig_configs`, which this generalizes.
/// The tag prints the PROFILE NAME only; the shape header appends the base
/// RS rate read off the RESOLVED config (`pc.log_inv_rates[0]`), so the
/// label tracks upstream profile regenerations (flock's slim moved from
/// base rate 1/4 to the Johnson rate 1/8 mid-development) instead of lying.
/// `custom:<log_inv_rate>:<initial_k>` builds a Johnson config at that
/// geometry via [`custom_johnson_config`] (flock-validator-gated).
fn bench_lig_configs(m_p: usize) -> ((LigPc, LigVc), String) {
    let prof = std::env::var("F2Z_LIG_PROFILE").unwrap_or_default();
    if let Some(rest) = prof.strip_prefix("custom:") {
        let mut it = rest.split(':');
        let r0: usize =
            it.next().and_then(|x| x.parse().ok()).expect("custom:<log_inv_rate>:<initial_k>");
        let k0: usize =
            it.next().and_then(|x| x.parse().ok()).expect("custom:<log_inv_rate>:<initial_k>");
        let cfg = custom_johnson_config(m_p + LOG_PACKING, r0, k0);
        let pair = cfg.to_prover_verifier_configs().expect("custom config pair");
        return (pair, format!("custom-k{k0}"));
    }
    let (lig_cfg, tag): (LigConfig, &str) = if prof == "r8" {
        // Base RS rate 1/8 via the ad-hoc UDR generator, at the embedded
        // profiles' interleaving (initial_k = 6). UNAUDITED perf probe (UDR
        // query counts, no grinding/OOD; ~121 L0 queries vs the Johnson
        // analysis' 60 at the same rate).
        (LigConfig::Adhoc { log_batch: 6, log_inv_rate: 3 }, "adhoc-udr")
    } else if m_p + LOG_PACKING >= 22 {
        match prof.as_str() {
            "slim" => (LigConfig::Embedded(LigeritoProfile::Slim), "slim"),
            "secure" => (LigConfig::Embedded(LigeritoProfile::Secure), "secure"),
            _ => (LigConfig::Embedded(LigeritoProfile::Fast), "fast"),
        }
    } else {
        (LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }, "adhoc")
    };
    (lig_configs(m_p, lig_cfg).expect("lig cfg"), tag.to_string())
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
    // profiles / validator-gated custom Johnson geometries at
    // m = m_p + 7 ≥ 22, ad-hoc only below. Hardcoding the tiny ad-hoc
    // config at big shapes is catastrophic (n=28 commit measured 292 s
    // ad-hoc vs the embedded profile's sub-second).
    let ((pc, vc), lig_tag) = bench_lig_configs(m_p);

    // Deterministic non-degenerate instance, generated STRAIGHT INTO the
    // per-column bit rows (`repack_leaf_bits` layout: bit `(b<<log₂W)|j`
    // of row `c` = bit `j` of cell `(b,c)`) — the `u128` cell tensor
    // (16 B per cell; 17 GB at n=30) never exists, mirroring the
    // upstream packed-transpose commit restructure.
    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let cell = |b: usize, c: usize| -> u128 {
        (p.cell_index(b, c) as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask
    };
    let log_w = w.trailing_zeros() as usize;
    let row_len = p.rows() << log_w;
    let words = row_len.div_ceil(64);
    let rows: Vec<Vec<u64>> = (0..p.cols())
        .map(|c| {
            let mut wv = vec![0u64; words];
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
                let term = if j == 0 { rw_fq[b] } else { rw_fq[b] * Fq::from(1u128 << j) };
                acc = acc + term;
            }
        }
        y = y + cw[c] * acc;
    }

    let n = t + s;
    println!(
        "\n=== n={n} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}, lig={lig_tag}@r1/{}, data={} KiB) ===",
        1usize << pc.log_inv_rates[0],
        (p.cells() * w).div_ceil(8) >> 10
    );

    // Commit: timed + its own peak window (the commitment/hint stays live).
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let commit_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "  commit:  {commit_ms:8.2} ms   peak {:8.2} MB   live-after {:6.2} MB",
        peak_mb(),
        live_mb()
    );

    // Warm-up prove (excluded from stats).
    {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof =
            prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        black_box(&proof);
    }

    // Timed reps: prove / serialize / deserialize / verify.
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut ser_us = Vec::new();
    let mut de_us = Vec::new();
    let mut bytes = 0usize;
    for _ in 0..reps {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let t0 = Instant::now();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        prove_ms.push(t0.elapsed().as_secs_f64() * 1e3);

        let t1 = Instant::now();
        let ser = proof.to_bytes();
        ser_us.push(t1.elapsed().as_secs_f64() * 1e6);
        bytes = ser.len();

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
    let split_proof = {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof =
            prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        black_box(&proof);
        proof
    };
    let prove_peak = peak_mb();
    let phases = f2z::utils::prof::take_totals();

    println!(
        "  prove:   {:8.2} ms   peak {prove_peak:8.2} MB      (median of {reps})",
        median(prove_ms)
    );
    if !phases.is_empty() {
        let phase_ms = |labels: &[&str]| -> f64 {
            phases.iter().filter(|(l, _)| labels.contains(l)).map(|(_, s)| s).sum::<f64>() * 1e3
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
    }
    println!("  verify:  {:8.2} ms", median(verify_ms));
    println!(
        "  proof:   {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs",
        bytes as f64 / 1024.0,
        median(ser_us),
        median(de_us)
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

    let reps: usize = std::env::var("F2Z_BENCH_REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
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
