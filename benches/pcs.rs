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
//!
//! Protocol notes (from the zinc-plus measurement lore): idle the box first;
//! for quotable *time* numbers at big shapes run one shape per process (the
//! peak-memory numbers reset per shape and are fine in one process); quote
//! medians, expect ±5–15 % run-to-run.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{
    LigConfig, commit_rs_flock_with, lig_configs, prove_mle_eval_mod_q_ligerito,
    verify_mle_eval_mod_q_ligerito,
};
use f2z::pcs::{IntEvalParams, mod_q_num_chunks, smallest_generator};

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
    let (pc, vc) =
        lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }).expect("lig cfg");

    // Deterministic non-degenerate instance (mirrors examples/reference_measure).
    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let data: Vec<u128> =
        (0..p.cells()).map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask).collect();
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
    let mut y = Fq::from(0u128);
    for c in 0..p.cols() {
        let mut acc = Fq::from(0u128);
        for b in 0..p.rows() {
            acc = acc + Fq::from(rw_q[b]) * Fq::from(data[p.cell_index(b, c)]);
        }
        y = y + cw[c] * acc;
    }

    let n = t + s;
    println!(
        "\n=== n={n} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}, data={} KiB) ===",
        (p.cells() * w).div_ceil(8) >> 10
    );

    // Commit: timed + its own peak window (the commitment/hint stays live).
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
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

    // Peak over one prove (heap high-water; the commit hint is live below it).
    reset_peak();
    {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof =
            prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        black_box(&proof);
    }
    let prove_peak = peak_mb();

    println!(
        "  prove:   {:8.2} ms   peak {prove_peak:8.2} MB      (median of {reps})",
        median(prove_ms)
    );
    println!("  verify:  {:8.2} ms", median(verify_ms));
    println!(
        "  proof:   {bytes:8} B ({:.1} KiB)   serialize {:.0} µs / deserialize {:.0} µs",
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
    // proof-size-friendly split) + the 2-chunk W=32 regime.
    let default_shapes: Vec<(usize, usize, usize)> =
        vec![(10, 6, 1), (12, 6, 1), (13, 7, 1), (14, 8, 1), (4, 8, 32)];
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
