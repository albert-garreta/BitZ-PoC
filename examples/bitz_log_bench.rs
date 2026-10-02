//! BitZ logarithmic vs standard BitZ on the raw standalone claim.
//!
//! `bitz_log_bench <n> [--t T] [--a A] [--ladder L] [--mu-ladder M]
//! [--reps R] [--seed S] [--mode std|log1|log2|ab|ab3]` commits random bits at
//! `t × (n − t)` (default `Shape::reference`), then for each mode proves the
//! `bitz` CLI's raw claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = y` once to warm up and
//! `R` times (medians), verifying every timed proof:
//!
//! - `std`: BitZ as in `bitz_bench` (the `2^s` column folds in the proof);
//! - `ab` (std vs log1) / `ab3` (std, log2, log1): the arms alternate every
//!   repetition, rotating which goes first, so all see the same machine;
//! - `log1` / `log2`: the logarithmic scheme of `paper/notes/bitz_logarithmic.tex`
//!   (`bitz::bitz::logarithmic`: a commitment to the folds' bits, a second
//!   product tree, one recursive BitZ instance) with ONE Ligerito run for
//!   both commitments (the fold bits join `f`'s ladder) or two runs.
//!
//! The verifier's `(⋆)` term — the multilinear extension of `(g^{γ_i})_i`,
//! `γ = π_q^{-1}(eq(·, r₁))`: the `eq` table mod `q` (`2^t` mulmods), the
//! `2^t` exponentiations and the evaluation — is reported apart. For `log` it
//! is measured inside the verifier (traced phases `v: (*) …`, and `v: (*')`
//! for the recursive instance's own row images); for `std` the same three
//! computations are timed in isolation at the same sizes (its verifier runs
//! them interleaved with the rest). One size per process; threads = rayon's.
use std::time::{Duration, Instant};

use bitz::bitz::logarithmic::{self, LogOpener};
use bitz::bitz::{FixedBasePow, Shape, WINDOW, record_phases, take_phases};
use bitz::bitz::logarithmic::MuLadder;
use bitz::ligerito_flock::{eq_table_mod_q, standalone_q_bits};
use bitz::piop::spartan::protocol::bitz_opener::{
    BitZLigerito, BitZOpener, prove_standalone, standalone_evaluation, verify_standalone,
};
use field::Gf128 as Gf;

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn median(v: &[Duration]) -> Duration {
    if v.is_empty() {
        return Duration::ZERO;
    }
    let mut sorted = v.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: `getrusage` fills the struct for the calling process.
    let ok = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if ok != 0 {
        return 0;
    }
    // SAFETY: filled by the call above.
    let usage = unsafe { usage.assume_init() };
    usage.ru_maxrss as u64
}

/// Per-label medians of the recorded phases over the timed repetitions.
#[derive(Default)]
struct Phases(Vec<(String, Vec<Duration>)>);

impl Phases {
    fn add(&mut self, phases: Vec<(String, Duration)>) {
        // Sum repeated labels within one run first (a label may fire twice).
        let mut run: Vec<(String, Duration)> = Vec::new();
        for (label, d) in phases {
            match run.iter_mut().find(|(l, _)| *l == label) {
                Some((_, total)) => *total += d,
                None => run.push((label, d)),
            }
        }
        for (label, d) in run {
            match self.0.iter_mut().find(|(l, _)| *l == label) {
                Some((_, v)) => v.push(d),
                None => self.0.push((label, vec![d])),
            }
        }
    }

    fn get(&self, label: &str) -> Duration {
        self.0
            .iter()
            .find(|(l, _)| l == label)
            .map_or(Duration::ZERO, |(_, v)| median(v))
    }

    fn print(&self, prefix: &str) {
        for (label, v) in &self.0 {
            if label.starts_with(prefix) {
                println!("  {label:<28} {:>9.3} ms", ms(median(v)));
            }
        }
    }
}

fn random_gf(state: &mut u64) -> Gf {
    Gf::from_polynomial_words([xorshift(state), xorshift(state)])
}

/// The `(⋆)` computations at the standard verifier's sizes, timed in
/// isolation: `eq(·, r₁)` mod `q`, `2^t` exponentiations, the evaluation.
fn isolated_star(t: usize, q_bits: usize, seed: u64) -> (Duration, Duration, Duration) {
    let q = (1u128 << (q_bits - 1)) | 1; // any odd modulus of the width
    let q = (q..).step_by(2).find(|&c| field::is_probable_prime_public(&field::Uint::from(c))).unwrap();
    let arith = field::FpCtx::from_prime_u128(q);
    let mut state = seed ^ 0x5151;
    let r1: Vec<u128> = (0..t).map(|_| ((u128::from(xorshift(&mut state)) << 64) | u128::from(xorshift(&mut state))) % q).collect();
    let g = bitz::piop::spartan::protocol::bitz_generator();
    let comb = FixedBasePow::new(g, 128, WINDOW);
    let mut eq_t = Vec::new();
    let mut img_t = Vec::new();
    let mut mle_t = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        let gamma = eq_table_mod_q(&arith, &r1);
        eq_t.push(started.elapsed());
        let started = Instant::now();
        let y = bitz::bitz::fold::row_images(&comb, &gamma);
        img_t.push(started.elapsed());
        let alpha: Vec<Gf> = (0..t).map(|_| random_gf(&mut state)).collect();
        let rho: Vec<Gf> = (0..t).map(|_| random_gf(&mut state)).collect();
        let started = Instant::now();
        let v = logarithmic::star_mle(&y, &alpha, &rho);
        mle_t.push(started.elapsed());
        std::hint::black_box(v);
    }
    (median(&eq_t), median(&img_t), median(&mle_t))
}

/// The standard verifier binds its claim by absorbing the expanded weights
/// (`2^t + 2^s` residues, `BitZVerifier::verify`); timed in isolation.
fn isolated_claim_binding(shape: &Shape, q_bits: usize) -> Duration {
    use bitz::bitz::{BitZParams, LinearClaim, build_prover};
    let q = (1u128 << (q_bits - 1)) | 1;
    let q = (q..).step_by(2).find(|&c| field::is_probable_prime_public(&field::Uint::from(c))).unwrap();
    let params = BitZParams::new(*shape, q, bitz::piop::spartan::protocol::bitz_generator()).unwrap();
    let rows: Vec<u128> = (0..shape.rows() as u128).map(|i| (i * 0x9E37_79B9) % q).collect();
    let cols: Vec<u128> = (0..shape.columns() as u128).map(|i| (i * 0x7F4A_7C15) % q).collect();
    let claim = LinearClaim::new(&params, rows, cols, 1).unwrap();
    let mut times = Vec::new();
    for _ in 0..5 {
        let mut state = build_prover(b"bench".as_slice(), b"claim-binding".as_slice());
        let started = Instant::now();
        state.public_message(&claim);
        times.push(started.elapsed());
        std::hint::black_box(state.finish());
    }
    median(&times)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut n: Option<usize> = None;
    let mut t: Option<usize> = None;
    let mut a: Option<usize> = None;
    let mut reps = 5usize;
    let mut seed = 1u64;
    let mut ladder = String::from("custom:1:4");
    let mut mu_ladder = String::from("lad:5:3:3:7");
    let mut mode = String::from("both");
    let mut mu_min: Option<usize> = None;
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().expect("flag value");
        match args[i].as_str() {
            "--t" => { t = Some(next().parse().expect("--t")); i += 2; }
            "--a" => { a = Some(next().parse().expect("--a")); i += 2; }
            "--reps" => { reps = next().parse().expect("--reps"); i += 2; }
            "--seed" => { seed = next().parse().expect("--seed"); i += 2; }
            "--ladder" => { ladder = next(); i += 2; }
            "--mu-ladder" => { mu_ladder = next(); i += 2; }
            "--mode" => { mode = next(); i += 2; }
            "--mu-min" => { mu_min = Some(next().parse().expect("--mu-min")); i += 2; }
            x => { n = Some(x.parse().expect("n")); i += 1; }
        }
    }
    let n = n.expect("usage: bitz_log_bench <n> [--t T] [--a A] [--ladder L] [--mu-ladder M] [--mu-min K] [--reps R] [--seed S] [--mode std|log1|log2|ab|ab3]");
    let shape = match t {
        Some(t) => Shape::new(t, n - t).expect("shape"),
        None => Shape::reference(n).expect("shape"),
    };
    let (t, s) = (shape.log_rows(), shape.log_columns());
    let layout = shape.layout();
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    let q_bits = standalone_q_bits(&layout);
    println!("bitz_log_bench n={n} t={t} s={s} threads={threads} reps={reps} seed={seed} ladder={ladder} mu_ladder={mu_ladder} q_bits={q_bits}");

    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let words = shape.rows() / 64;
    let rows: Vec<Vec<u64>> = (0..shape.columns())
        .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
        .collect();
    let base = BitZOpener::new(layout, BitZLigerito::parse(&ladder, 100).expect("--ladder"), 100)
        .expect("opener");

    // Commit once per rep (median); the last hint is kept for both modes.
    let mut commit_times = Vec::with_capacity(reps);
    let mut committed = None;
    for _ in 0..reps.max(1) {
        let input = rows.clone();
        let started = Instant::now();
        let hint = base.commit(input).expect("commit");
        commit_times.push(started.elapsed());
        committed = Some(hint);
    }
    drop(rows);
    let hint = committed.expect("committed");
    let commit = median(&commit_times);
    println!("commit: median {:.2} ms", ms(commit));

    // Arms: `std` (BitZ), `log2` (two Ligerito runs), `log1` (one run).
    // `ab` interleaves std and log1, `ab3` all three, rotating the order every
    // repetition so every arm sees the same machine state.
    let arms: Vec<&str> = match mode.as_str() {
        "std" => vec!["std"],
        "log" | "log1" => vec!["log1"],
        "log2" => vec!["log2"],
        "both" | "ab" => vec!["std", "log1"],
        "ab3" => vec!["std", "log2", "log1"],
        other => panic!("unknown --mode {other}"),
    };
    let interleave = mode == "ab" || mode == "ab3";
    let make_log = |single: bool| -> LogOpener {
        LogOpener::new(
            layout,
            BitZLigerito::parse(&ladder, 100).expect("--ladder"),
            MuLadder::parse(&mu_ladder, 100).expect("--mu-ladder"),
            a,
            mu_min,
            single,
            100,
        )
        .expect("log opener")
    };
    let log1 = arms.contains(&"log1").then(|| make_log(true));
    let log2 = arms.contains(&"log2").then(|| make_log(false));
    for (name, opener) in [("log1", &log1), ("log2", &log2)] {
        if let Some(opener) = opener {
            let geometry = *opener.geometry();
            let mu_queries: Vec<usize> = opener.mu_security().levels.iter().map(|l| l.queries).collect();
            // One run: only the fold bits' level 0 is their own; the levels
            // after the join are the shared chain's.
            let ladder = match opener.merge_plan() {
                Some(plan) => format!(
                    "one run: fold bits level 0 rate 1/{} k_B={} queries {}, joining after iteration {}; chain queries {:?}, residual 2^{}",
                    1usize << plan.branch.log_inv_rate,
                    plan.branch.initial_k,
                    plan.branch.queries,
                    plan.merge_iter,
                    plan.verifier_config.queries,
                    plan.final_log_n
                ),
                None => format!("two runs: mu ladder {} queries {:?}", opener.mu_ladder().name(), mu_queries),
            };
            println!(
                "{name}: a={} t'={} s'={} s_com={} mu m'={} round0={} (grinding {}); {ladder}",
                geometry.a,
                7 + geometry.a,
                geometry.s - geometry.a,
                geometry.s_com,
                7 + geometry.a + geometry.s_com,
                opener.mu_round_0(),
                opener.mu_round_0_grinding(),
            );
        }
    }
    let std_claim = arms.contains(&"std").then(|| standalone_evaluation(&base, &hint).expect("claimed value"));
    let log1_claim = log1.as_ref().map(|o| logarithmic::standalone_evaluation(o, &hint).expect("claimed value"));
    let log2_claim = log2.as_ref().map(|o| logarithmic::standalone_evaluation(o, &hint).expect("claimed value"));

    #[derive(Default)]
    struct Arm {
        prove: Vec<Duration>,
        verify: Vec<Duration>,
        prove_phases: Phases,
        verify_phases: Phases,
        bytes: usize,
        narg_hints: (usize, usize),
        sizes: logarithmic::LogSizes,
    }
    let mut results: Vec<(String, Arm)> = arms.iter().map(|a| (a.to_string(), Arm::default())).collect();
    let mut run_arm = |name: &str, keep: bool, results: &mut Vec<(String, Arm)>| {
        let arm = &mut results.iter_mut().find(|(n, _)| n == name).expect("arm").1;
        if name == "std" {
            let claimed = std_claim.expect("std claim");
            let started = Instant::now();
            let proof = prove_standalone(&base, &hint, claimed).expect("prove");
            let elapsed = started.elapsed();
            let started = Instant::now();
            verify_standalone(&base, &hint.commitment, claimed, &proof).expect("verify");
            let v = started.elapsed();
            if keep {
                arm.prove.push(elapsed);
                arm.verify.push(v);
                arm.bytes = proof.to_bytes().len();
                arm.narg_hints = (proof.transcript.narg_string.len(), proof.transcript.hints.len());
            }
            return;
        }
        let (opener, claimed) = if name == "log1" {
            (log1.as_ref().expect("log1"), log1_claim.expect("claim"))
        } else {
            (log2.as_ref().expect("log2"), log2_claim.expect("claim"))
        };
        record_phases(true);
        let started = Instant::now();
        let (proof, sizes) = logarithmic::prove_standalone(opener, &hint, claimed).expect("prove");
        let elapsed = started.elapsed();
        let p_phases = take_phases();
        let started = Instant::now();
        logarithmic::verify_standalone(opener, &hint.commitment, claimed, &proof).expect("verify");
        let v = started.elapsed();
        let v_phases = take_phases();
        record_phases(false);
        if keep {
            arm.prove.push(elapsed);
            arm.verify.push(v);
            arm.prove_phases.add(p_phases);
            arm.verify_phases.add(v_phases);
            arm.bytes = proof.to_bytes().len();
            arm.narg_hints = (proof.transcript.narg_string.len(), proof.transcript.hints.len());
            arm.sizes = sizes;
        }
    };
    if interleave {
        for rep in 0..=reps {
            let keep = rep > 0;
            let k = arms.len();
            for j in 0..k {
                run_arm(arms[(j + rep) % k], keep, &mut results);
            }
        }
    } else {
        for name in &arms {
            for rep in 0..=reps {
                run_arm(name, rep > 0, &mut results);
            }
        }
    }

    for (name, arm) in &results {
        let verify = median(&arm.verify);
        let prove = median(&arm.prove);
        if name == "std" {
            let (eq_t, img_t, mle_t) = isolated_star(t, q_bits, seed);
            let star = eq_t + img_t + mle_t;
            let binding = isolated_claim_binding(&shape, q_bits);
            println!("std: prove {:.2} ms (+commit {:.2}) verify {:.3} ms proof {} B (narg {} / ligerito {}) folds {} B",
                ms(prove), ms(commit), ms(verify), arm.bytes, arm.narg_hints.0, arm.narg_hints.1, 16usize << s);
            println!("std: (*) isolated {:.3} ms = eq {:.3} + images {:.3} + mle {:.3}; verify w/o (*) ≈ {:.3} ms; claim binding {:.3} ms",
                ms(star), ms(eq_t), ms(img_t), ms(mle_t), ms(verify.saturating_sub(star)), ms(binding));
            println!("RESULT mode=std n={n} t={t} s={s} threads={threads} interleaved={interleave} commit_ms={:.3} prove_ms={:.3} verify_ms={:.4} star_ms={:.4} star_eq_ms={:.4} star_img_ms={:.4} star_mle_ms={:.4} binding_ms={:.4} proof_bytes={} peak_rss={}",
                ms(commit), ms(prove), ms(verify), ms(star), ms(eq_t), ms(img_t), ms(mle_t), ms(binding), arm.bytes, peak_rss_bytes());
            continue;
        }
        let opener = if name == "log1" { log1.as_ref() } else { log2.as_ref() }.expect("opener");
        let geometry = *opener.geometry();
        let vp = &arm.verify_phases;
        let star = vp.get("v: (*) eq table r1") + vp.get("v: (*) row images") + vp.get("v: (*) mle");
        let star_rec = vp.get("v: (*') rec row images");
        println!("{name} prover phases (medians):");
        arm.prove_phases.print("log:");
        println!("{name} verifier phases (medians):");
        vp.print("v:");
        println!(
            "{name}: prove {:.2} ms (+commit {:.2}) verify {:.3} ms proof {} B (narg {} / hints {})",
            ms(prove), ms(commit), ms(verify), arm.bytes, arm.narg_hints.0, arm.narg_hints.1
        );
        println!(
            "{name}: verify = (*) {:.3} ms [eq {:.3} + images {:.3} + mle {:.3}] + (*') {:.3} ms + rest {:.3} ms",
            ms(star),
            ms(vp.get("v: (*) eq table r1")),
            ms(vp.get("v: (*) row images")),
            ms(vp.get("v: (*) mle")),
            ms(star_rec),
            ms(verify.saturating_sub(star + star_rec))
        );
        let sz = arm.sizes;
        // narg+hints per part; with one run `open_f` / `open_mu` are the two
        // reductions (and the fold bits' own level-0 opening), `chain` the
        // shared Ligerito run.
        println!(
            "{name}: proof parts: head {} gkr_f {} gkr_mu {} rec_folds {} rec_gkr {} open_f {}+{} open_mu {}+{} chain {}+{} (sum {})",
            sz.head, sz.gkr_f, sz.gkr_mu, sz.rec_folds, sz.rec_gkr,
            sz.open_f.0, sz.open_f.1, sz.open_mu.0, sz.open_mu.1, sz.open_chain.0, sz.open_chain.1, sz.total()
        );
        println!("RESULT mode={name} n={n} t={t} s={s} a={} s_com={} threads={threads} interleaved={interleave} commit_ms={:.3} prove_ms={:.3} verify_ms={:.4} star_ms={:.4} star_eq_ms={:.4} star_img_ms={:.4} star_mle_ms={:.4} star_rec_ms={:.4} proof_bytes={} gkr_f={} gkr_mu={} rec_folds={} rec_gkr={} open_f={} open_mu={} open_chain={} peak_rss={}",
            geometry.a, geometry.s_com, ms(commit), ms(prove), ms(verify), ms(star),
            ms(vp.get("v: (*) eq table r1")), ms(vp.get("v: (*) row images")), ms(vp.get("v: (*) mle")),
            ms(star_rec), arm.bytes, sz.gkr_f, sz.gkr_mu, sz.rec_folds, sz.rec_gkr,
            sz.open_f.0 + sz.open_f.1, sz.open_mu.0 + sz.open_mu.1, sz.open_chain.0 + sz.open_chain.1, peak_rss_bytes());
    }
}
