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
//! - `F2Z_LIG_PROFILE`: Ligerito profile at `m = m_p + 7 ≥ 22` —
//!   `custom:3:4` (DEFAULT; validator-gated Johnson geometry at base RS
//!   rate 1/8, initial_k = 4), `slim` (embedded; fewer queries + 16-bit
//!   grinding at the same 100-bit target, the proof-size profile), `fast`
//!   (base RS rate 1/2), `secure` (120-bit UDR), or any
//!   `custom:<log_inv_rate>:<initial_k>[:<bits>]` — the optional `bits`
//!   sets the round-by-round security target (default 100; e.g.
//!   `custom:3:4:128` for a ~128-bit opener, queries/grinding/OOD
//!   re-solved and flock-validator-gated), or
//!   `udr:<log_inv_rate>:<initial_k>[:<bits>]` — queries-ONLY security
//!   (UDR regime, zero grinding of either kind, zero OOD; ceiling ≈115
//!   bits at n=22 / ≈109 at n=28 from the L0 UDR fold error). Below
//!   m = 22 every profile falls back to the ad-hoc rate-1/4 config
//!   (unaudited, test-only).
//! - `F2Z_BENCH_EXT`: also run the extension-field arm against the same
//!   commitment — `1`/`gl2` = Goldilocks² (e=2), `bb4` = BabyBear⁴
//!   (X⁴ − 11, the Plonky3 challenge field; e=4), `kb5` = a KoalaBear
//!   quintic (e=5, leanVM's field shape; stand-in X⁵−3 reduction).
//!   Companion Plonky3 baseline:
//!   `~/Plonky3 uni-stark/benches/prove_mul_babybear.rs`.
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
use f2z::ligerito_flock::{
    custom_johnson_config_bits, custom_udr_config_bits, custom_udr_grind_config_bits,
};
use flock_core::pcs::ligerito::{LigeritoProfile, ProverConfig as LigPc, VerifierConfig as LigVc};

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
    let prof =
        std::env::var("F2Z_LIG_PROFILE").unwrap_or_else(|_| "custom:3:4".to_string());
    // Queries-only UDR geometry: udr:<log_inv_rate>:<initial_k>[:<bits>] —
    // zero grinding (either kind), zero OOD; the target is paid entirely in
    // queries. Ceiling = the L0 UDR fold error (≈115 bits at n=22, ≈109 at
    // n=28); above it flock's validator rejects with the shortfall.
    // UDR + fold-grinding: udrg:<log_inv_rate>:<initial_k>[:<bits>] — the
    // pg shortfall is recovered by cheap per-fold PoW (9–16 bits at our
    // shapes), so targets up to 128 validate; queries still pay the full
    // target. `udrg:1:4:128` = the 128-bit configuration.
    if let Some(rest) = prof.strip_prefix("udrg:") {
        let mut it = rest.split(':');
        let r0: usize = it
            .next()
            .and_then(|x| x.parse().ok())
            .expect("udrg:<log_inv_rate>:<initial_k>[:<bits>]");
        let k0: usize = it
            .next()
            .and_then(|x| x.parse().ok())
            .expect("udrg:<log_inv_rate>:<initial_k>[:<bits>]");
        let bits: Option<usize> =
            it.next().map(|x| x.parse().expect("udrg:<log_inv_rate>:<initial_k>:<bits>"));
        if m_p + LOG_PACKING >= 22 {
            let cfg = custom_udr_grind_config_bits(m_p + LOG_PACKING, r0, k0, bits);
            let pair = cfg.to_prover_verifier_configs().expect("udrg config pair");
            let tag = match bits {
                Some(b) => format!("udrg-k{k0}-{b}b"),
                None => format!("udrg-k{k0}"),
            };
            return (pair, tag);
        }
        let pair = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
            .expect("adhoc cfg");
        return (pair, "adhoc".to_string());
    }
    if let Some(rest) = prof.strip_prefix("udr:") {
        let mut it = rest.split(':');
        let r0: usize = it
            .next()
            .and_then(|x| x.parse().ok())
            .expect("udr:<log_inv_rate>:<initial_k>[:<bits>]");
        let k0: usize = it
            .next()
            .and_then(|x| x.parse().ok())
            .expect("udr:<log_inv_rate>:<initial_k>[:<bits>]");
        let bits: Option<usize> =
            it.next().map(|x| x.parse().expect("udr:<log_inv_rate>:<initial_k>:<bits>"));
        if m_p + LOG_PACKING >= 22 {
            let cfg = custom_udr_config_bits(m_p + LOG_PACKING, r0, k0, bits);
            let pair = cfg.to_prover_verifier_configs().expect("udr config pair");
            let tag = match bits {
                Some(b) => format!("udr-k{k0}-{b}b"),
                None => format!("udr-k{k0}"),
            };
            return (pair, tag);
        }
        let pair = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
            .expect("adhoc cfg");
        return (pair, "adhoc".to_string());
    }
    if let Some(rest) = prof.strip_prefix("custom:") {
        let mut it = rest.split(':');
        let r0: usize =
            it.next().and_then(|x| x.parse().ok()).expect("custom:<log_inv_rate>:<initial_k>");
        let k0: usize =
            it.next().and_then(|x| x.parse().ok()).expect("custom:<log_inv_rate>:<initial_k>");
        // Optional round-by-round security target: custom:<r>:<k>:<bits>
        // (e.g. custom:3:4:128). Absent → the slim template's 100.
        let bits: Option<usize> =
            it.next().map(|x| x.parse().expect("custom:<log_inv_rate>:<initial_k>:<bits>"));
        // `custom_johnson_config` needs an embedded template (m = 22..=35);
        // below that, fall back to the ad-hoc config — the same boundary as
        // `sha_lig_configs`, so a `custom:*` sweep mirrors the library default.
        if m_p + LOG_PACKING >= 22 {
            let cfg = custom_johnson_config_bits(m_p + LOG_PACKING, r0, k0, bits);
            let pair = cfg.to_prover_verifier_configs().expect("custom config pair");
            let tag = match bits {
                Some(b) => format!("custom-k{k0}-{b}b"),
                None => format!("custom-k{k0}"),
            };
            return (pair, tag);
        }
        let pair = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
            .expect("adhoc cfg");
        return (pair, "adhoc".to_string());
    }
    let (lig_cfg, tag): (LigConfig, &str) = if prof == "r8" {
        // Base RS rate 1/8 via the ad-hoc UDR generator, at the embedded
        // profiles' interleaving (initial_k = 6). UNAUDITED perf probe (UDR
        // query counts, no grinding/OOD; ~121 L0 queries vs the Johnson
        // analysis' 60 at the same rate).
        (LigConfig::Adhoc { log_batch: 6, log_inv_rate: 3 }, "adhoc-udr")
    } else if m_p + LOG_PACKING >= 22 {
        match prof.as_str() {
            "fast" => (LigConfig::Embedded(LigeritoProfile::Fast), "fast"),
            "slim3" => (LigConfig::Embedded(LigeritoProfile::Slim3), "slim3"),
            "secure" => (LigConfig::Embedded(LigeritoProfile::Secure), "secure"),
            // unrecognized → slim (unset never lands here: the env default
            // is "custom:3:4", handled by the custom branch above)
            _ => (LigConfig::Embedded(LigeritoProfile::Slim), "slim"),
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
    // Fill fraction: columns `live..2^s` stay ALL ZERO — exactly the
    // padding of a witness with N = fill·2^n cells (the column axis is
    // the high-order index, so a zero-padded witness ends in whole zero
    // columns). `y` below is derived from SET BITS, so it stays correct.
    let fill: f64 = std::env::var("F2Z_BENCH_FILL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    assert!(fill > 0.0 && fill <= 1.0, "F2Z_BENCH_FILL must be in (0, 1]");
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
                let term = if j == 0 { rw_fq[b] } else { rw_fq[b] * Fq::from(1u128 << j) };
                acc = acc + term;
            }
        }
        y = y + cw[c] * acc;
    }

    let n = t + s;
    println!(
        "\n=== n={n} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}, lig={lig_tag}@r1/{}k{}, data={} KiB, live={live}/{} elide={}) ===",
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        (p.cells() * w).div_ceil(8) >> 10,
        p.cols(),
        std::env::var("F2Z_COL_ELIDE").unwrap_or_else(|_| "1".into())
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
    // Release flock's cross-prove scratch pool first: the reported prove
    // peak is the production single-prove shape (pool cold), not the
    // reps-warmed pool stacked under the forest. The timed medians above
    // deliberately keep the warm pool — that IS the steady-state timing.
    f2z::ligerito_flock::flock_scratch_clear();
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
        "  proof:   {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs   fnv {proof_fnv:016x}",
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

    // ── Extension-field arm (`F2Z_BENCH_EXT`): the SAME committed
    // instance opened at an extension-field statement (default 100-bit
    // projection primes) — the delta vs the base run above is the
    // extension surcharge in the same process/thermal window.
    // `1`/`gl2` = Goldilocks² (e=2, q_bits=64); `bb4` = BabyBear⁴
    // (X⁴ = 11, the Plonky3 challenge field; e=4, q_bits=31). ──
    match std::env::var("F2Z_BENCH_EXT").as_deref() {
        Ok("1") | Ok("gl2") => bench_ext_arm::<Fp2>(&p, &hint, alpha, reps, &pc, &vc),
        Ok("bb4") => bench_ext_arm::<BbFp4>(&p, &hint, alpha, reps, &pc, &vc),
        Ok("kb5") => bench_ext_arm::<KbFp5>(&p, &hint, alpha, reps, &pc, &vc),
        Ok(other) => panic!("F2Z_BENCH_EXT: unknown arm {other:?} (use 1|gl2|bb4|kb5)"),
        Err(_) => {}
    }
}

/// An evaluation extension field `K = F_q[X]/(h)` for the ext bench arm:
/// the verifier-side ring `R` plus the statement-shaping constants.
trait BenchExtField:
    Copy
    + PartialEq
    + std::fmt::Debug
    + From<u128>
    + std::ops::Add<Output = Self>
    + std::ops::Mul<Output = Self>
{
    /// Extension degree `e = deg(h)`.
    const EXT_DEG: usize;
    /// `⌈log₂ q⌉` of the base characteristic (the ext API's `q_bits`).
    const Q_BITS: usize;
    /// The base characteristic `q`.
    const CHAR: u128;
    const NAME: &'static str;
    /// Element from its canonical coordinate vector (length `EXT_DEG`).
    fn from_coords(c: &[u128]) -> Self;
    /// The module-basis images `[1, X, …, X^{e−1}]`.
    fn basis() -> Vec<Self>;
}

/// Goldilocks p = 2^64 − 2^32 + 1; K = F_p[X]/(X² − 7).
const GL_P: u128 = 0xFFFF_FFFF_0000_0001;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Fp2 {
    c0: u128,
    c1: u128,
}
impl From<u128> for Fp2 {
    fn from(v: u128) -> Self {
        Fp2 { c0: v % GL_P, c1: 0 }
    }
}
impl std::ops::Add for Fp2 {
    type Output = Fp2;
    fn add(self, o: Fp2) -> Fp2 {
        Fp2 { c0: (self.c0 + o.c0) % GL_P, c1: (self.c1 + o.c1) % GL_P }
    }
}
impl std::ops::Mul for Fp2 {
    type Output = Fp2;
    fn mul(self, o: Fp2) -> Fp2 {
        let m = |a: u128, b: u128| (a * b) % GL_P; // operands < 2^64: exact in u128
        Fp2 {
            c0: (m(self.c0, o.c0) + m(7, m(self.c1, o.c1))) % GL_P,
            c1: (m(self.c0, o.c1) + m(self.c1, o.c0)) % GL_P,
        }
    }
}
impl BenchExtField for Fp2 {
    const EXT_DEG: usize = 2;
    const Q_BITS: usize = 64;
    const CHAR: u128 = GL_P;
    const NAME: &'static str = "Goldilocks²";
    fn from_coords(c: &[u128]) -> Self {
        Fp2 { c0: c[0] % GL_P, c1: c[1] % GL_P }
    }
    fn basis() -> Vec<Self> {
        vec![Fp2 { c0: 1, c1: 0 }, Fp2 { c0: 0, c1: 1 }]
    }
}

/// BabyBear p = 2^31 − 2^27 + 1; K = F_p[X]/(X⁴ − 11) — the quartic
/// extension Plonky3 samples its BabyBear challenges from.
const BB_P: u128 = 0x7800_0001;

#[derive(Clone, Copy, PartialEq, Debug)]
struct BbFp4([u128; 4]);
impl From<u128> for BbFp4 {
    fn from(v: u128) -> Self {
        BbFp4([v % BB_P, 0, 0, 0])
    }
}
impl std::ops::Add for BbFp4 {
    type Output = BbFp4;
    fn add(self, o: BbFp4) -> BbFp4 {
        BbFp4(std::array::from_fn(|i| (self.0[i] + o.0[i]) % BB_P))
    }
}
impl std::ops::Mul for BbFp4 {
    type Output = BbFp4;
    fn mul(self, o: BbFp4) -> BbFp4 {
        // Schoolbook: coordinates < 2^31, so every partial product is
        // < 2^62 and the 7 convolution sums stay far below 2^128;
        // X⁴ ≡ 11 folds the top back with an ×11 (< 2^68).
        let mut prod = [0u128; 7];
        for i in 0..4 {
            for j in 0..4 {
                prod[i + j] += self.0[i] * o.0[j];
            }
        }
        BbFp4(std::array::from_fn(|k| (prod[k] + 11 * prod.get(k + 4).copied().unwrap_or(0)) % BB_P))
    }
}
impl BenchExtField for BbFp4 {
    const EXT_DEG: usize = 4;
    const Q_BITS: usize = 31;
    const CHAR: u128 = BB_P;
    const NAME: &'static str = "BabyBear⁴";
    fn from_coords(c: &[u128]) -> Self {
        BbFp4(std::array::from_fn(|i| c[i] % BB_P))
    }
    fn basis() -> Vec<Self> {
        (0..4)
            .map(|d| BbFp4(std::array::from_fn(|i| u128::from(i == d))))
            .collect()
    }
}

/// KoalaBear p = 2^31 − 2^24 + 1; a quintic extension (e = 5) — the shape
/// of leanVM's evaluation field. Stand-in reduction X⁵ = 3 (leanVM's
/// actual quintic is non-binomial; the F2Z-side costs depend only on
/// (e, q_bits), which match).
const KB_P: u128 = 0x7F00_0001;

#[derive(Clone, Copy, PartialEq, Debug)]
struct KbFp5([u128; 5]);
impl From<u128> for KbFp5 {
    fn from(v: u128) -> Self {
        let mut c = [0u128; 5];
        c[0] = v % KB_P;
        KbFp5(c)
    }
}
impl std::ops::Add for KbFp5 {
    type Output = KbFp5;
    fn add(self, o: KbFp5) -> KbFp5 {
        KbFp5(std::array::from_fn(|i| (self.0[i] + o.0[i]) % KB_P))
    }
}
impl std::ops::Mul for KbFp5 {
    type Output = KbFp5;
    fn mul(self, o: KbFp5) -> KbFp5 {
        let mut prod = [0u128; 9];
        for i in 0..5 {
            for j in 0..5 {
                prod[i + j] += self.0[i] * o.0[j];
            }
        }
        KbFp5(std::array::from_fn(|k| {
            (prod[k] + 3 * prod.get(k + 5).copied().unwrap_or(0)) % KB_P
        }))
    }
}
impl BenchExtField for KbFp5 {
    const EXT_DEG: usize = 5;
    const Q_BITS: usize = 31;
    const CHAR: u128 = KB_P;
    const NAME: &'static str = "KoalaBear⁵ (stand-in X⁵−3)";
    fn from_coords(c: &[u128]) -> Self {
        KbFp5(std::array::from_fn(|i| c[i] % KB_P))
    }
    fn basis() -> Vec<Self> {
        (0..5)
            .map(|d| KbFp5(std::array::from_fn(|i| u128::from(i == d))))
            .collect()
    }
}

/// The extension-field opening benchmarked against the SAME commitment as
/// the base arm: prove/verify medians, ext phase scopes on one profiled
/// prove AND one profiled verify, proof size + codec times.
fn bench_ext_arm<K: BenchExtField>(
    p: &IntEvalParams,
    hint: &f2z::ligerito_flock::FlockCommitHint,
    alpha: f2z::BinaryFieldGF128,
    reps: usize,
    pc: &LigPc,
    vc: &LigVc,
) {
    use f2z::ligerito_flock::{prove_mle_eval_ext_ligerito, verify_mle_eval_ext_ligerito};
    let q_bits = K::Q_BITS;
    let ext_deg = K::EXT_DEG;
    let proj = f2z::ext_proj::ExtProjParams::default();
    let basis = K::basis();
    let log_w = p.word_bits.trailing_zeros() as usize;
    let w_mask = p.word_bits - 1;

    // Coordinate-major lift of v⁽¹⁾ ∈ K^{2^t} (arbitrary < q), col weights
    // over K, and the claimed μ from the SET BITS of the committed rows.
    let coords: Vec<Vec<u128>> = (0..ext_deg)
        .map(|d| {
            (0..p.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(d as u128 + 7)
                        % K::CHAR
                })
                .collect()
        })
        .collect();
    let col_w: Vec<K> = (0..p.cols())
        .map(|c| {
            let cs: Vec<u128> = (0..ext_deg)
                .map(|d| {
                    (c as u128)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .wrapping_add((d as u128) << 40)
                        % K::CHAR
                })
                .collect();
            K::from_coords(&cs)
        })
        .collect();
    let v1: Vec<K> = (0..p.rows())
        .map(|b| {
            let cs: Vec<u128> = (0..ext_deg).map(|d| coords[d][b]).collect();
            K::from_coords(&cs)
        })
        .collect();
    let mut y = K::from(0u128);
    for (c, row) in hint.rows().iter().enumerate() {
        let mut acc = K::from(0u128);
        for (wi, &word) in row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let i = (wi << 6) | bit;
                let (b, j) = (i >> log_w, i & w_mask);
                let term =
                    if j == 0 { v1[b] } else { v1[b] * K::from(1u128 << j) };
                acc = acc + term;
            }
        }
        y = y + col_w[c] * acc;
    }

    // Warm-up (excluded).
    {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof =
            prove_mle_eval_ext_ligerito(&mut pt, hint, p, &coords, q_bits, &proj, alpha, pc);
        black_box(&proof);
    }

    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut ser_us = Vec::new();
    let mut de_us = Vec::new();
    let mut bytes = 0usize;
    for _ in 0..reps {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let t0 = Instant::now();
        let proof =
            prove_mle_eval_ext_ligerito(&mut pt, hint, p, &coords, q_bits, &proj, alpha, pc);
        prove_ms.push(t0.elapsed().as_secs_f64() * 1e3);

        let t1 = Instant::now();
        let ser = proof.to_bytes();
        ser_us.push(t1.elapsed().as_secs_f64() * 1e6);
        bytes = ser.len();
        let t2 = Instant::now();
        let de = f2z::ligerito_flock::IntEvalRsLigExtProof::from_bytes(&ser).expect("codec");
        de_us.push(t2.elapsed().as_secs_f64() * 1e6);
        black_box(&de);

        let mut vt = f2z::transcript::Blake3Transcript::new();
        let t3 = Instant::now();
        verify_mle_eval_ext_ligerito(
            &mut vt,
            &hint.commitment,
            &proof,
            p,
            &coords,
            &col_w,
            &basis,
            alpha,
            y,
            q_bits,
            &proj,
            vc,
        )
        .expect("ext verify");
        verify_ms.push(t3.elapsed().as_secs_f64() * 1e3);
    }

    // Ext phase scopes over one profiled prove + one profiled verify
    // (OBLONG_PROFILE=1 to populate).
    let _ = f2z::utils::prof::take_totals();
    let proof = {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof =
            prove_mle_eval_ext_ligerito(&mut pt, hint, p, &coords, q_bits, &proj, alpha, pc);
        black_box(&proof);
        proof
    };
    let prove_phases = f2z::utils::prof::take_totals();
    {
        let mut vt = f2z::transcript::Blake3Transcript::new();
        verify_mle_eval_ext_ligerito(
            &mut vt,
            &hint.commitment,
            &proof,
            p,
            &coords,
            &col_w,
            &basis,
            alpha,
            y,
            q_bits,
            &proj,
            vc,
        )
        .expect("ext verify (profiled)");
    }
    let verify_phases = f2z::utils::prof::take_totals();
    let pick = |phases: &[(&'static str, f64)], label: &str| -> f64 {
        phases.iter().filter(|(l, _)| *l == label).map(|(_, s)| s).sum::<f64>() * 1e3
    };

    println!("  ext(K={}, e={ext_deg}, q_bits={q_bits}, q'={}b):", K::NAME, proj.prime_bits);
    println!("    prove:  {:8.2} ms   (median of {reps})", median(prove_ms));
    if !prove_phases.is_empty() {
        println!(
            "    p-phases: step1_folds {:7.2} ms | sample_prime {:6.2} ms | project {:6.2} ms",
            pick(&prove_phases, "ext:step1_folds"),
            pick(&prove_phases, "ext:sample_prime"),
            pick(&prove_phases, "ext:project"),
        );
    }
    println!("    verify: {:8.2} ms", median(verify_ms));
    if !verify_phases.is_empty() {
        println!(
            "    v-phases: sample_prime {:6.2} ms | project {:6.2} ms | checks {:6.2} ms",
            pick(&verify_phases, "ext:sample_prime"),
            pick(&verify_phases, "ext:project"),
            pick(&verify_phases, "ext:checks"),
        );
    }
    println!(
        "    proof:  {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs",
        bytes as f64 / 1024.0,
        median(ser_us),
        median(de_us)
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
