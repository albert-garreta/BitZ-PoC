//! `f2z` — CLI runner: commit / prove / verify one mod-`q` MLE-opening
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
//! Usage: `f2z <n> [<t> <s> [<W>]] [options]`
//!
//! - `n` — cell-index MLE variables, `n = t + s` (the committed instance
//!   is `2^n · W` bits). If `t s` are omitted the reference split
//!   `t ≈ 0.6n` is used (clamped to the packing constraint
//!   `t + log₂W ≥ 7`). `W` as a fourth positional sets the cell width
//!   (power of two; equivalent to `--word-bits`, positional wins) — e.g.
//!   the reference W=32 shape: `f2z 12 4 8 32`.
//! - `--threads N` / `-j N` — rayon pool size; `1` = single-threaded.
//!   Default: all cores (or `RAYON_NUM_THREADS`).
//! - `--reps R` — timing repetitions (median reported; default 3).
//! - `--profile P` — Ligerito config: `fast` (default) | `slim` | `secure`
//!   (the embedded profiles) | `custom:<log_inv_rate>:<initial_k>`
//!   (validator-gated Johnson geometry). Below `m = n < 22` every choice
//!   falls back to the ad-hoc test config (UNAUDITED).
//! - `--word-bits W` — cell width (power of two; default 1).
//!
//! Integer-guard mode is a COMPILE-TIME feature: build with
//! `--features unchecked` for release-style plain integer ops (the header
//! reports the active mode and warns otherwise).

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::process::exit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use f2z::ligerito::{LOG_PACKING, packed_vars};
use f2z::ligerito_flock::{
    LigConfig, commit_rs_ligerito_rows, custom_johnson_config, lig_configs,
    mle_eval_mod_q_lig_size_breakdown, prove_mle_eval_mod_q_ligerito,
    verify_mle_eval_mod_q_ligerito,
};
use f2z::pcs::{IntEvalParams, mod_q_num_chunks, smallest_generator};
use f2z::transcript::Blake3Transcript;
use flock_core::pcs::ligerito::LigeritoProfile;

// Peak-heap tracker (wraps System) — the live-heap high-water, the same
// notion as `benches/pcs.rs` / flock's benches, so numbers compare.
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

/// `𝔽_q`, `q = 2^100 − 15` — the reference evaluation field.
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

fn usage() -> ! {
    eprintln!(
        "usage: f2z <n> [<t> <s> [<W>]] [--threads N] [--reps R] \
         [--profile fast|slim|secure|custom:<log_inv_rate>:<initial_k>] [--word-bits W]\n\
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
    word_bits: usize,
}

fn parse_args() -> Opts {
    let mut pos: Vec<usize> = Vec::new();
    let mut o = Opts {
        n: 0,
        t: None,
        s: None,
        threads: None,
        reps: 3,
        profile: "fast".to_string(),
        word_bits: 1,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => usage(),
            "--threads" | "-j" => {
                o.threads = Some(
                    args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
                );
            }
            "--reps" => {
                o.reps = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            }
            "--profile" => {
                o.profile = args.next().unwrap_or_else(|| usage());
            }
            "--word-bits" | "-w" => {
                o.word_bits =
                    args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
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
    match pos.as_slice() {
        [n] => o.n = *n,
        [n, t, s] => {
            o.n = *n;
            o.t = Some(*t);
            o.s = Some(*s);
        }
        [n, t, s, w] => {
            o.n = *n;
            o.t = Some(*t);
            o.s = Some(*s);
            o.word_bits = *w; // positional W wins over --word-bits
        }
        _ => usage(),
    }
    if o.reps == 0 || !o.word_bits.is_power_of_two() {
        usage()
    }
    o
}

/// The reference split `t ≈ 0.6n`, clamped to the packing constraint
/// (`t + log₂W ≥ 7`) and `s ≥ 1`.
fn default_split(n: usize, word_bits: usize) -> (usize, usize) {
    let log_w = word_bits.trailing_zeros() as usize;
    let t_min = 7usize.saturating_sub(log_w);
    let t = ((3 * n).div_ceil(5)).max(t_min).min(n - 1);
    (t, n - t)
}

fn resolve_configs(
    m_p: usize,
    profile: &str,
) -> (
    (
        flock_core::pcs::ligerito::ProverConfig,
        flock_core::pcs::ligerito::VerifierConfig,
    ),
    String,
) {
    if let Some(rest) = profile.strip_prefix("custom:") {
        let mut it = rest.split(':');
        let r0: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| usage());
        let k0: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| usage());
        if m_p + LOG_PACKING >= 22 {
            let cfg = custom_johnson_config(m_p + LOG_PACKING, r0, k0);
            let pair = cfg.to_prover_verifier_configs().expect("custom config pair");
            return (pair, format!("custom-k{k0}"));
        }
        let pair = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
            .expect("adhoc cfg");
        return (pair, "adhoc".to_string());
    }
    let (cfg, tag): (LigConfig, &str) = if m_p + LOG_PACKING >= 22 {
        match profile {
            "fast" => (LigConfig::Embedded(LigeritoProfile::Fast), "fast"),
            "slim" => (LigConfig::Embedded(LigeritoProfile::Slim), "slim"),
            "secure" => (LigConfig::Embedded(LigeritoProfile::Secure), "secure"),
            other => {
                eprintln!("unknown profile: {other}");
                usage()
            }
        }
    } else {
        (LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }, "adhoc")
    };
    (lig_configs(m_p, cfg).expect("lig cfg"), tag.to_string())
}

fn main() {
    let o = parse_args();

    #[cfg(feature = "parallel")]
    if let Some(th) = o.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(th.max(1))
            .build_global()
            .expect("rayon global pool (set --threads before any parallel work)");
    }
    #[cfg(not(feature = "parallel"))]
    if o.threads.is_some_and(|th| th > 1) {
        eprintln!("note: built without the `parallel` feature — running serially");
    }

    let (t, s) = match (o.t, o.s) {
        (Some(t), Some(s)) => (t, s),
        _ => default_split(o.n, o.word_bits),
    };
    let w = o.word_bits;
    let log_w = w.trailing_zeros() as usize;
    if t + s != o.n {
        eprintln!("t + s = {} ≠ n = {}", t + s, o.n);
        exit(2);
    }
    if t + log_w < 7 {
        eprintln!("packing needs t + log₂W ≥ 7 (got t={t}, W={w})");
        exit(2);
    }
    if s == 0 {
        eprintln!("need s ≥ 1");
        exit(2);
    }

    let q_bits = 100usize;
    let p = IntEvalParams { t, s, word_bits: w };
    let m_p = packed_vars(&p);
    let lch = mod_q_num_chunks(&p, q_bits);
    let ((pc, vc), lig_tag) = resolve_configs(m_p, &o.profile);

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
        "f2z: n={} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}) | lig={lig_tag}@r1/{}k{} | \
         threads={threads_eff} | int guards: {}",
        o.n,
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        if f2z::utils::CHECKED { "CHECKED (build with --features unchecked)" } else { "unchecked" },
    );
    if lch > 1 {
        println!("note: {lch} mod-q chunks (t + W > 127 − q_bits) — prove scales ~×{lch}");
    }

    // Deterministic instance straight into per-column bit rows (the
    // memory-honest pattern — the u128 cell tensor never exists).
    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let cell = |b: usize, c: usize| -> u128 {
        (p.cell_index(b, c) as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask
    };
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
    // Claimed y from the set bits (O(popcount) field ops).
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

    // Commit (timed; its own peak window — the hint stays live).
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let commit_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!("commit:  {commit_ms:9.2} ms   peak {:8.2} MB", peak_mb());

    // Warm-up prove (excluded), then timed reps.
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha_of(), &pc);
        black_box(&pr);
    }
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut last_proof = None;
    for _ in 0..o.reps {
        let mut pt = Blake3Transcript::new();
        let t1 = Instant::now();
        let proof =
            prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha_of(), &pc);
        prove_ms.push(t1.elapsed().as_secs_f64() * 1e3);

        let mut vt = Blake3Transcript::new();
        let t2 = Instant::now();
        verify_mle_eval_mod_q_ligerito(
            &mut vt,
            &hint.commitment,
            &proof,
            &p,
            &rw_q,
            &cw,
            alpha_of(),
            y,
            q_bits,
            &vc,
        )
        .expect("proof verifies");
        verify_ms.push(t2.elapsed().as_secs_f64() * 1e3);
        last_proof = Some(proof);
    }

    // Peak over one prove.
    reset_peak();
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha_of(), &pc);
        black_box(&pr);
    }
    let prove_peak = peak_mb();

    let proof = last_proof.expect("reps ≥ 1");
    let bytes = proof.to_bytes().len();
    let (zb, lig_b) = mle_eval_mod_q_lig_size_breakdown(&proof);
    let forest_b = zb.total() - zb.s_v;
    println!(
        "prove:   {:9.2} ms   peak {prove_peak:8.2} MB   (median of {}, verified)",
        median(prove_ms),
        o.reps
    );
    println!("verify:  {:9.2} ms", median(verify_ms));
    println!(
        "proof:   {:9.1} KiB  (forest-side {:.1} | s_v {:.1} | ligerito {:.1})",
        bytes as f64 / 1024.0,
        forest_b as f64 / 1024.0,
        zb.s_v as f64 / 1024.0,
        lig_b as f64 / 1024.0,
    );
}

/// The deterministic generator α (cached — `smallest_generator` scans).
fn alpha_of() -> f2z::poly::univariate::binary_gf128::BinaryFieldGF128 {
    use std::sync::OnceLock;
    static A: OnceLock<f2z::poly::univariate::binary_gf128::BinaryFieldGF128> = OnceLock::new();
    *A.get_or_init(smallest_generator)
}
