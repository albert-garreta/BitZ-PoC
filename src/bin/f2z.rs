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
//! - `--profile P` — Ligerito config: `slim` (default, rate 1/4) | `slim3`
//!   (rate 1/8) | `fast` (rate 1/2) | `secure`
//!   (the embedded profiles) | `custom:<log_inv_rate>:<initial_k>`
//!   (validator-gated Johnson geometry). Below `m = n < 22` every choice
//!   falls back to the ad-hoc test config (UNAUDITED).
//! - `--word-bits W` — cell width (power of two; default 1).
//! - `--family j2|j3|j4|j2s|j3s|j4s` — run the EXPERIMENTAL mod-q RLC
//!   claim FAMILY at this `n` instead of the single-claim opening: `j2` =
//!   the XOR triple (k = 3 claims on m₁, m₂, m₁⊕m₂ at per-claim row
//!   points), `j3` = k = 4 (m₁, m₂, m₃, ⊕-all), `j4` = k = 5. The `s`
//!   presets are the SHARED-POINT maximal families — the full XOR-closure
//!   of the j columns at ONE point (`j2s` = k = 3, `j3s` = k = 7, `j4s` =
//!   k = 15) through the collapsed-absorb shared-point API. W is fixed at
//!   1 and the shape is the measured A/B layout (4 UAIR columns, x-tensor
//!   split t' ≈ s — the README's 2026-07-26/27 RLC notes); `t s W`
//!   positionals do not apply. Every rep is verified. Example:
//!   `f2z 26 --family j4s --reps 5`
//! - `--taps vx|family|collapse|rotxor|sched` — run the EXPERIMENTAL
//!   structured-taps paths at this `n` (32-bit words along the ENTRY
//!   axis of W=1 bit-vectors, g = 5; 2 UAIR columns; ALL claims at ONE
//!   shared point): `vx` = the j=2 k=6 ROT/SHIFT/word-offset instance
//!   through the batched tap-claims (extraction + translated-eq
//!   openings) path; `family` = the same instance through the clustered
//!   stream family ({b1,b3,b5}/{b2,b4,b6}); `collapse` = the instance's
//!   13 deduped streams as 13 SINGLE-TAP claims through the
//!   weight-transform collapse (≤ 4 inner claims); `rotxor` = 8
//!   uniform-op-of-XOR-set claims (rotations + a word-offset of a₁⊕a₂,
//!   single-column rotations) through the same collapse (4 inner
//!   claims); `sched` = 48 schedule-shaped claims `off^t(x)` of ONE
//!   σ-style mixed combination through the COMPOSED collapse (2 inner
//!   tap bodies total; rounds clip to the shape's offset envelope, 48
//!   needs n ≥ 22). Every rep is verified.
//!   Example: `f2z 24 --taps sched --reps 5`
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
         [--profile slim|slim3|fast|secure|custom:<log_inv_rate>:<initial_k>] [--word-bits W] \
         [--family j2|j3|j4|j2s|j3s|j4s] [--taps vx|family|collapse|rotxor|sched]\n\
         (n = t + s; W = cell width, power of two, default 1;\n\
          --family runs the mod-q RLC claim family at the A/B layout — j2 = the\n\
          XOR triple, j3/j4 the wider families, j2s/j3s/j4s the SHARED-POINT\n\
          maximal families (full XOR-closure at one point, k = 3/7/15);\n\
          --taps runs the structured-taps instance (32-bit entry-axis words,\n\
          one shared point) — vx = batched tap claims, family = the clustered\n\
          stream family, collapse = 13 single-tap claims via the collapse,\n\
          rotxor = 8 uniform-op-of-XOR-set claims via the collapse,\n\
          sched = 48 off^t(σ-combo) claims via the COMPOSED collapse;\n\
          t/s/W do not apply there;\n\
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
    family: Option<String>,
    taps: Option<String>,
}

fn parse_args() -> Opts {
    let mut pos: Vec<usize> = Vec::new();
    let mut o = Opts {
        n: 0,
        t: None,
        s: None,
        threads: None,
        reps: 3,
        profile: "slim".to_string(),
        word_bits: 1,
        family: None,
        taps: None,
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
            "--family" => {
                o.family = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--taps" => {
                o.taps = Some(args.next().unwrap_or_else(|| usage()));
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
            "slim3" => (LigConfig::Embedded(LigeritoProfile::Slim3), "slim3"),
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

    if let Some(fam) = o.family.clone() {
        if o.t.is_some() || o.s.is_some() || o.word_bits != 1 {
            eprintln!("--family fixes W = 1 and derives the shape from n; drop t/s/W");
            exit(2);
        }
        run_family(&o, &fam);
        return;
    }
    if let Some(mode) = o.taps.clone() {
        if o.t.is_some() || o.s.is_some() || o.word_bits != 1 {
            eprintln!("--taps fixes W = 1 and derives the shape from n; drop t/s/W");
            exit(2);
        }
        run_taps(&o, &mode);
        return;
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

/// The measured A/B family layout for `n`: 4 UAIR columns
/// (`log_cols = 2`) of 32-bit words (`bit_vars = 5`), the remaining
/// variables split `t' ≈ s` (matches `examples/rlc_ab.rs`, so numbers
/// compare with the README's RLC notes).
fn family_layout(n: usize) -> f2z::pcs::ShaF2Layout {
    let log_cols = 2usize;
    let bit_vars = 5usize;
    let t_x = (n - log_cols) / 2;
    let s = n - log_cols - t_x;
    let tw = t_x - bit_vars;
    f2z::pcs::ShaF2Layout {
        p: IntEvalParams { t: bit_vars + log_cols + tw, s, word_bits: 1 },
        num_cols: 1 << log_cols,
        log_cols,
        bit_vars,
        num_vars: tw + s,
        tw,
        x_fold_extra: 0,
    }
}

/// The `--family` runner: commit once, then prove/verify the preset's RLC
/// claim family (every rep verified), reporting medians, proof size and
/// peak heap.
fn run_family(o: &Opts, fam: &str) {
    use f2z::ligerito_flock::{
        RlcFamilyClaim, RlcSharedClaim, mle_eval_mod_q_lig_rlc_family_proof_size_bytes,
        prove_mle_eval_mod_q_ligerito_rlc_family,
        prove_mle_eval_mod_q_ligerito_rlc_family_shared_point,
        verify_mle_eval_mod_q_ligerito_rlc_family,
        verify_mle_eval_mod_q_ligerito_rlc_family_shared_point,
    };
    use f2z::pcs::{FQ_MOD, Fq as PcsFq, extract_virtual_xor_rows, virtual_xor_params};

    // `s`-suffixed presets are the SHARED-POINT maximal families (the full
    // XOR-closure of the j columns at ONE point, k = 2^j − 1).
    let (j, k, forms, shared): (usize, usize, Vec<usize>, bool) = match fam {
        "j2" => (2, 3, vec![0b01, 0b10, 0b11], false),
        "j3" => (3, 4, vec![0b001, 0b010, 0b100, 0b111], false),
        "j4" => (4, 5, vec![0b0001, 0b0010, 0b0100, 0b1000, 0b1111], false),
        "j2s" => (2, 3, (1..1 << 2).collect(), true),
        "j3s" => (3, 7, (1..1 << 3).collect(), true),
        "j4s" => (4, 15, (1..1 << 4).collect(), true),
        other => {
            eprintln!("unknown family preset: {other} (expected j2|j3|j4|j2s|j3s|j4s)");
            exit(2);
        }
    };
    if o.n < 15 {
        eprintln!("--family needs n ≥ 15 (t' = (n−2)/2 ≥ 6 for the x-tensor presum)");
        exit(2);
    }
    let layout = family_layout(o.n);
    let p = layout.p;
    let p_x = virtual_xor_params(&layout);
    let m_p = packed_vars(&p);
    let ((pc, vc), lig_tag) = resolve_configs(m_p, &o.profile);
    let family_cols: Vec<usize> = (0..j).collect();

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
        "f2z --family {fam}: n={} (t'={}, s={}, j={j}, k={k}) | lig={lig_tag}@r1/{}k{} | \
         threads={threads_eff} | int guards: {}",
        o.n,
        p_x.t,
        p_x.s,
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        if f2z::utils::CHECKED { "CHECKED (build with --features unchecked)" } else { "unchecked" },
    );

    // Deterministic committed bit rows (memory-honest packed-rows path).
    let words = p.rows().div_ceil(64);
    let rows: Vec<Vec<u64>> = (0..p.cols())
        .map(|c| {
            (0..words)
                .map(|w| {
                    ((c as u64) << 32 | w as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .rotate_left(((c + w) & 63) as u32)
                })
                .collect()
        })
        .collect();
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let commit_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!("commit:  {commit_ms:9.2} ms   peak {:8.2} MB", peak_mb());

    // Statement: per-claim row weights (distinct row points) for the
    // general presets, ONE row point for the `s` presets; shared column
    // weights; claimed values from the committed data.
    let rws: Vec<Vec<u128>> = (0..if shared { 1 } else { k })
        .map(|i| {
            (0..p_x.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(11 + i as u128)
                        % FQ_MOD
                })
                .collect()
        })
        .collect();
    let colw: Vec<PcsFq> = (0..p_x.cols())
        .map(|c| PcsFq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
        .collect();
    let cs: Vec<u128> = forms
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let rw = &rws[if shared { 0 } else { i }];
            let cols: Vec<usize> =
                (0..j).filter(|&fi| (f >> fi) & 1 == 1).map(|fi| family_cols[fi]).collect();
            let a_rows = extract_virtual_xor_rows(&layout, hint.rows(), &cols, 0, None);
            let mut y = PcsFq::from(0u128);
            for (c, row) in a_rows.iter().enumerate() {
                let mut acc = PcsFq::from(0u128);
                for (wi, &word) in row.iter().enumerate() {
                    let mut bits = word;
                    while bits != 0 {
                        let t = bits.trailing_zeros() as usize;
                        acc = acc + PcsFq::from(rw[(wi << 6) | t]);
                        bits &= bits.wrapping_sub(1);
                    }
                }
                y = y + colw[c] * acc;
            }
            y.0
        })
        .collect();
    let claims: Vec<RlcFamilyClaim<'_>> = (0..k)
        .map(|i| RlcFamilyClaim {
            form: forms[i],
            row_weights_q: &rws[if shared { 0 } else { i }],
            claimed: cs[i],
        })
        .collect();
    let sh_claims: Vec<RlcSharedClaim> = (0..k)
        .map(|i| RlcSharedClaim { form: forms[i], claimed: cs[i] })
        .collect();

    // Warm-up (excluded), then timed reps — every rep verified.
    let prove_once = |pt: &mut Blake3Transcript| {
        if shared {
            prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                pt, &hint, &layout, &family_cols, &rws[0], &sh_claims, alpha_of(), &pc,
            )
        } else {
            prove_mle_eval_mod_q_ligerito_rlc_family(
                pt, &hint, &layout, &family_cols, &claims, alpha_of(), &pc,
            )
        }
    };
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_once(&mut pt);
        black_box(&pr);
    }
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut last = None;
    for _ in 0..o.reps {
        let mut pt = Blake3Transcript::new();
        let t1 = Instant::now();
        let proof = prove_once(&mut pt);
        prove_ms.push(t1.elapsed().as_secs_f64() * 1e3);
        let mut vt = Blake3Transcript::new();
        let t2 = Instant::now();
        if shared {
            verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rws[0], &sh_claims,
                &colw, alpha_of(), &vc,
            )
            .expect("shared-point family proof verifies");
        } else {
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw,
                alpha_of(), &vc,
            )
            .expect("family proof verifies");
        }
        verify_ms.push(t2.elapsed().as_secs_f64() * 1e3);
        last = Some(proof);
    }
    reset_peak();
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_once(&mut pt);
        black_box(&pr);
    }
    let prove_peak = peak_mb();
    let proof = last.expect("reps ≥ 1");
    let bytes = mle_eval_mod_q_lig_rlc_family_proof_size_bytes(&proof);
    println!(
        "prove:   {:9.2} ms   peak {prove_peak:8.2} MB   ({k} claims, median of {}, verified)",
        median(prove_ms),
        o.reps
    );
    println!("verify:  {:9.2} ms", median(verify_ms));
    println!(
        "proof:   {:9.1} KiB  ({:.2} KiB/claim)",
        bytes as f64 / 1024.0,
        bytes as f64 / 1024.0 / k as f64,
    );
}

/// The structured-taps layout for `n` (matches `examples/taps_ab.rs`, so
/// numbers compare with the README's structured-taps note): 2 UAIR
/// bit-columns (`log_cols = 1`, W = 1, `bit_vars = 0`), 32-bit words
/// along the ENTRY axis (g = 5), x split `t' vs s` as even as `tw ≥ 6`
/// allows.
fn taps_layout(n: usize) -> f2z::pcs::ShaF2Layout {
    let log_cols = 1usize;
    let tw = ((n - log_cols) / 2).max(6);
    let s = n - log_cols - tw;
    let delta: usize = std::env::var("F2Z_TAPS_DELTA").map_or(0, |v| v.parse().unwrap());
    f2z::pcs::ShaF2Layout {
        p: IntEvalParams { t: log_cols + tw, s, word_bits: 1 },
        num_cols: 1 << log_cols,
        log_cols,
        bit_vars: 0,
        num_vars: tw + s,
        tw,
        x_fold_extra: delta,
    }
}

/// The `--taps` runner: the j=2 k=6 ROT/SHIFT/word-offset instance (or
/// its 13 streams as single-tap claims), all claims at ONE shared point,
/// through the chosen path — every rep verified.
#[allow(clippy::arithmetic_side_effects)]
fn run_taps(o: &Opts, mode: &str) {
    use f2z::ligerito_flock::{
        RlcFamilyClaim, TapClaim, TapComposedClaim, TapFamilyCluster, TapPointClaim,
        TapVerifyClaim, mle_eval_mod_q_lig_tap_family_size_breakdown,
        mle_eval_mod_q_lig_tap_size_breakdown, mle_eval_mod_q_lig_xor_proof_size_bytes,
        prove_mle_eval_mod_q_ligerito_tap_claims, prove_mle_eval_mod_q_ligerito_tap_collapse,
        prove_mle_eval_mod_q_ligerito_tap_composed, prove_mle_eval_mod_q_ligerito_tap_family,
        verify_mle_eval_mod_q_ligerito_tap_claims, verify_mle_eval_mod_q_ligerito_tap_collapse,
        verify_mle_eval_mod_q_ligerito_tap_composed, verify_mle_eval_mod_q_ligerito_tap_family,
    };
    use f2z::pcs::{FQ_BITS, FQ_MOD, Fq as PcsFq, virtual_xor_params};
    use f2z::taps::{TapOp, extract_virtual_tap_rows};

    const GRP: usize = 5;
    if !matches!(mode, "vx" | "family" | "collapse" | "rotxor" | "sched") {
        eprintln!("unknown taps mode: {mode} (expected vx|family|collapse|rotxor|sched)");
        exit(2);
    }
    if o.n < 14 {
        eprintln!("--taps needs n ≥ 14 (tw ≥ 6 and s ≥ 7 for the 32-bit group field)");
        exit(2);
    }
    let layout = taps_layout(o.n);
    if layout.x_fold_extra > 0 && !matches!(mode, "vx" | "sched") {
        eprintln!("F2Z_TAPS_DELTA applies to the vx|sched modes only (δ-envelope)");
        exit(2);
    }
    let p = layout.p;
    let p_x = virtual_xor_params(&layout);
    let m_p = packed_vars(&p);
    let ((pc, vc), lig_tag) = resolve_configs(m_p, &o.profile);

    // The instance's tap lists (identities, two 3-tap rotation
    // convolutions, a cross-column mix, the lossy-SHIFT claim) and the
    // pinned 2-cluster stream split.
    let rot =
        |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: false, off };
    let shl = |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: true, off };
    let claim_taps: Vec<Vec<TapOp>> = vec![
        vec![TapOp::ident(0)],
        vec![TapOp::ident(1)],
        vec![rot(0, 1, 0), rot(0, 2, 1), rot(0, 3, 2)],
        vec![rot(1, 2, 0), rot(1, 5, 1), rot(1, 7, 2)],
        vec![rot(0, 1, 0), rot(1, 4, 0), rot(0, 6, 1)],
        vec![shl(0, 3, 0), shl(1, 5, 1), rot(1, 2, 2)],
    ];
    let streams: Vec<Vec<TapOp>> = vec![
        vec![
            TapOp::ident(0),
            rot(0, 1, 0),
            rot(0, 2, 1),
            rot(0, 3, 2),
            rot(1, 4, 0),
            rot(0, 6, 1),
        ],
        vec![
            TapOp::ident(1),
            rot(1, 2, 0),
            rot(1, 5, 1),
            rot(1, 7, 2),
            shl(0, 3, 0),
            shl(1, 5, 1),
            rot(1, 2, 2),
        ],
    ];
    let forms: Vec<Vec<usize>> = vec![vec![0b000001, 0b001110, 0b110010], vec![
        0b0000001, 0b0001110, 0b1110000,
    ]];
    let members: Vec<Vec<usize>> = vec![vec![0, 2, 4], vec![1, 3, 5]];

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
    // The composed-collapse schedule preset's source (σ-style mixed
    // combination) and round count (48, clipped to the shape's offset
    // envelope; the FOLDED baseline lists also eat the source's own
    // word offset).
    let sched_src: Vec<TapOp> = vec![
        rot(0, 7, 0),
        rot(0, 18, 0),
        shl(0, 3, 0),
        rot(1, 0, 1),
    ];
    let sched_max_src_off = sched_src.iter().map(|t| t.off).max().unwrap_or(0);
    let sched_rounds =
        48usize.min((1usize << (layout.p.s - GRP)) - sched_max_src_off);
    let k_desc = match mode {
        "collapse" => "13 single-tap claims".to_string(),
        "rotxor" => "8 op(xor-set) claims".to_string(),
        "sched" => format!("{sched_rounds} off^t(σ-combo) claims"),
        _ => "k=6 instance".to_string(),
    };
    println!(
        "f2z --taps {mode}: n={} (t'={}, s={}, g={GRP}, {k_desc}, one shared point) | \
         lig={lig_tag}@r1/{}k{} | threads={threads_eff} | int guards: {}",
        o.n,
        p_x.t,
        p_x.s,
        1usize << pc.log_inv_rates[0],
        pc.initial_k,
        if f2z::utils::CHECKED { "CHECKED (build with --features unchecked)" } else { "unchecked" },
    );

    let words = p.rows().div_ceil(64);
    let rows: Vec<Vec<u64>> = (0..p.cols())
        .map(|c| {
            (0..words)
                .map(|w| {
                    ((c as u64) << 32 | w as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .rotate_left(((c + w) & 63) as u32)
                })
                .collect()
        })
        .collect();
    reset_peak();
    let t0 = Instant::now();
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let commit_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!("commit:  {commit_ms:9.2} ms   peak {:8.2} MB", peak_mb());

    // ONE shared evaluation point for every claim.
    let rw: Vec<u128> = (0..p_x.rows())
        .map(|b| {
            (b as u128)
                .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                .wrapping_add(11)
                % FQ_MOD
        })
        .collect();
    let colw: Vec<PcsFq> = (0..p_x.cols())
        .map(|c| PcsFq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
        .collect();
    let eval_taps = |taps: &[TapOp]| -> u128 {
        let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), taps);
        let mut y = PcsFq::from(0u128);
        for (c, row) in a_rows.iter().enumerate() {
            let mut acc = PcsFq::from(0u128);
            for (wi, &word) in row.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let t = bits.trailing_zeros() as usize;
                    acc = acc + PcsFq::from(rw[(wi << 6) | t]);
                    bits &= bits.wrapping_sub(1);
                }
            }
            y = y + colw[c] * acc;
        }
        y.0
    };

    // Statement + per-mode prove/verify/size closures.
    let cs: Vec<u128> = claim_taps.iter().map(|t| eval_taps(t)).collect();
    let tclaims: Vec<TapClaim<'_>> = claim_taps
        .iter()
        .map(|taps| TapClaim { taps, row_weights_q: &rw })
        .collect();
    let tvclaims: Vec<TapVerifyClaim<'_, PcsFq>> = claim_taps
        .iter()
        .zip(cs.iter())
        .map(|(taps, &c)| TapVerifyClaim {
            taps,
            row_weights_q: &rw,
            col_weights: &colw,
            claimed: PcsFq::from(c),
        })
        .collect();
    let cluster_claims: Vec<Vec<RlcFamilyClaim<'_>>> = (0..2)
        .map(|ci| {
            forms[ci]
                .iter()
                .zip(members[ci].iter())
                .map(|(&form, &bi)| RlcFamilyClaim {
                    form,
                    row_weights_q: &rw,
                    claimed: cs[bi],
                })
                .collect()
        })
        .collect();
    let clusters: Vec<TapFamilyCluster<'_>> = (0..2)
        .map(|ci| TapFamilyCluster { streams: &streams[ci], claims: &cluster_claims[ci] })
        .collect();
    // Point-claim sets for the collapse-style modes: `collapse` = the 13
    // deduped streams as singleton XOR sets; `rotxor` = uniform ops
    // applied OUTSIDE XOR sets (rotations/offsets of a₁⊕a₂ plus a few
    // single-column ops) — `op(⊕ cols)` claims.
    let all_streams: Vec<TapOp> = streams.iter().flatten().copied().collect();
    let uni = |amt: usize, dropout: bool, off: usize| f2z::taps::TapUniOp {
        grp_log2: GRP,
        bit_amt: amt,
        bit_dropout: dropout,
        off,
    };
    let (pc_sets, pc_ops): (Vec<Vec<usize>>, Vec<f2z::taps::TapUniOp>) = match mode {
        "collapse" => (
            all_streams.iter().map(|t| vec![t.col]).collect(),
            all_streams.iter().map(|t| t.uni()).collect(),
        ),
        "rotxor" => (
            vec![
                vec![0, 1],
                vec![0, 1],
                vec![0, 1],
                vec![0, 1],
                vec![0, 1],
                vec![0],
                vec![1],
                vec![1],
            ],
            vec![
                uni(1, false, 0),
                uni(5, false, 0),
                uni(11, false, 0),
                uni(19, false, 0),
                uni(2, false, 1),
                uni(3, false, 0),
                uni(7, false, 0),
                uni(9, false, 0),
            ],
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let pclaims: Vec<TapPointClaim<'_>> = pc_sets
        .iter()
        .zip(pc_ops.iter())
        .map(|(set, &op)| {
            let taps: Vec<TapOp> = set.iter().map(|&c| op.with_col(c)).collect();
            TapPointClaim { cols: set, op, claimed: eval_taps(&taps) }
        })
        .collect();
    // Schedule claims: `off^t` of the fixed source; the claim values run
    // through the offset-FOLDED lists (the independent extraction route
    // the composed weight-transform algebra must reproduce).
    let sched_folded: Vec<Vec<TapOp>> = if mode == "sched" {
        (0..sched_rounds)
            .map(|t| {
                sched_src
                    .iter()
                    .map(|tap| {
                        let mut tap = *tap;
                        tap.off += t;
                        tap
                    })
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    let cclaims: Vec<TapComposedClaim<'_>> = sched_folded
        .iter()
        .enumerate()
        .map(|(t, folded)| TapComposedClaim {
            source: &sched_src,
            outer: uni(0, false, t),
            claimed: eval_taps(folded),
        })
        .collect();

    enum TapProof {
        Vx(f2z::ligerito_flock::IntEvalRsLigModQTapProof),
        Fam(f2z::ligerito_flock::IntEvalRsLigTapFamilyProof),
        Clp(f2z::ligerito_flock::IntEvalRsLigModQXorProof),
        Cmp(f2z::ligerito_flock::IntEvalRsLigModQTapProof),
    }
    let prove_once = |pt: &mut Blake3Transcript| -> TapProof {
        match mode {
            "vx" => TapProof::Vx(prove_mle_eval_mod_q_ligerito_tap_claims(
                pt, &hint, &layout, FQ_BITS, &tclaims, alpha_of(), &pc,
            )),
            "family" => TapProof::Fam(prove_mle_eval_mod_q_ligerito_tap_family(
                pt, &hint, &layout, &clusters, alpha_of(), &pc,
            )),
            "sched" => TapProof::Cmp(prove_mle_eval_mod_q_ligerito_tap_composed(
                pt, &hint, &layout, &rw, &colw, &cclaims, alpha_of(), &pc,
            )),
            _ => TapProof::Clp(prove_mle_eval_mod_q_ligerito_tap_collapse(
                pt, &hint, &layout, &rw, &colw, &pclaims, alpha_of(), &pc,
            )),
        }
    };
    let verify_once = |vt: &mut Blake3Transcript, proof: &TapProof| match proof {
        TapProof::Vx(pr) => verify_mle_eval_mod_q_ligerito_tap_claims(
            vt, &hint.commitment, pr, &layout, alpha_of(), FQ_BITS, &tvclaims, &vc,
        )
        .expect("tap claims verify"),
        TapProof::Fam(pr) => verify_mle_eval_mod_q_ligerito_tap_family(
            vt, &hint.commitment, pr, &layout, &clusters, &colw, alpha_of(), &vc,
        )
        .expect("stream family verifies"),
        TapProof::Cmp(pr) => verify_mle_eval_mod_q_ligerito_tap_composed(
            vt, &hint.commitment, pr, &layout, &rw, &colw, &cclaims, alpha_of(), &vc,
        )
        .expect("composed schedule verifies"),
        TapProof::Clp(pr) => verify_mle_eval_mod_q_ligerito_tap_collapse(
            vt, &hint.commitment, pr, &layout, &rw, &colw, &pclaims, alpha_of(), &vc,
        )
        .expect("collapse verifies"),
    };
    let size_of = |proof: &TapProof| -> usize {
        match proof {
            TapProof::Vx(pr) | TapProof::Cmp(pr) => {
                let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(pr);
                b.total() + lig
            }
            TapProof::Fam(pr) => {
                let (b, lig) = mle_eval_mod_q_lig_tap_family_size_breakdown(pr);
                b.total() + lig
            }
            TapProof::Clp(pr) => mle_eval_mod_q_lig_xor_proof_size_bytes(pr),
        }
    };
    let k = match mode {
        "collapse" | "rotxor" => pclaims.len(),
        "sched" => cclaims.len(),
        _ => tclaims.len(),
    };

    // Warm-up (excluded), then timed reps — every rep verified.
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_once(&mut pt);
        black_box(&pr);
    }
    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut last = None;
    for _ in 0..o.reps {
        let mut pt = Blake3Transcript::new();
        let t1 = Instant::now();
        let proof = prove_once(&mut pt);
        prove_ms.push(t1.elapsed().as_secs_f64() * 1e3);
        let mut vt = Blake3Transcript::new();
        let t2 = Instant::now();
        verify_once(&mut vt, &proof);
        verify_ms.push(t2.elapsed().as_secs_f64() * 1e3);
        last = Some(proof);
    }
    reset_peak();
    {
        let mut pt = Blake3Transcript::new();
        let pr = prove_once(&mut pt);
        black_box(&pr);
    }
    let prove_peak = peak_mb();
    let proof = last.expect("reps ≥ 1");
    let bytes = size_of(&proof);
    println!(
        "prove:   {:9.2} ms   peak {prove_peak:8.2} MB   ({k} claims, median of {}, verified)",
        median(prove_ms),
        o.reps
    );
    println!("verify:  {:9.2} ms", median(verify_ms));
    println!(
        "proof:   {:9.1} KiB  ({:.2} KiB/claim)",
        bytes as f64 / 1024.0,
        bytes as f64 / 1024.0 / k as f64,
    );
}

/// The deterministic generator α (cached — `smallest_generator` scans).
fn alpha_of() -> f2z::poly::univariate::binary_gf128::BinaryFieldGF128 {
    use std::sync::OnceLock;
    static A: OnceLock<f2z::poly::univariate::binary_gf128::BinaryFieldGF128> = OnceLock::new();
    *A.get_or_init(smallest_generator)
}
