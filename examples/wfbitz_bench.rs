//! The paper's raw-performance row for the BitZ parity prover at one size.
//!
//! `bitz_bench <n> [--reps R] [--seed S] [--ladder L]` commits random bits at
//! the scheme's split (`Shape::reference`: `t = ⌈3n/5⌉ − 1`, `s = n − t`) and
//! proves the `bitz` CLI's raw claim through the standalone wfbitz opening
//! ([`wfbitz_opener::prove_standalone`]): the statement frame, Round 0 for a
//! Johnson ladder, a transcript-sampled prime of `min(113, 127 − t − 1)` bits
//! and point, the claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = y`, then BitZ's scheme
//! on a forked transcript. It commits `R` times (median), proves once to
//! warm up and then `R` times (medians of the wall time and of every traced
//! phase, the paper's buckets summed from them), verifies every timed proof
//! (median), and prints one `RESULT` line (with the source revision, as the
//! header does). The claimed value is computed once, outside the timers (as
//! the CLI does). One size per process so the peak resident set is the
//! shape's; the thread count is rayon's (`RAYON_NUM_THREADS`).
//!
//! Buckets, as the paper's table splits the F2Z prover: grand products =
//! `fold+images` + `gkr` (the integer folds, the images, the batched GKR);
//! ring switch incl. its sumcheck = `sumcheck` + `ring switch` (the
//! reduction of the GKR exit claim to one packed evaluation and the
//! ring-switch message); Ligerito = `ligerito`; Total = commit + prove (the
//! statement, Round 0 and the prime and point draws are in the prove time,
//! outside the buckets). Proof bytes: narg = every transcript message
//! (non-Ligerito), hints = the serialized Ligerito proof, ood = the Round-0
//! value and nonce.
use std::time::{Duration, Instant};

use bitz::ligerito_flock::standalone_q_bits;
use bitz::piop::spartan::protocol::wfbitz_opener::{
    WfbitzLigerito, WfbitzOpener, prove_standalone, standalone_evaluation,
    verify_standalone,
};
use bitz::wfbitz::{Shape, record_phases, take_phases};

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn median(v: &[Duration]) -> Duration {
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

/// The source revision, probed as the `bitz` CLI's generated tables record
/// it (`git describe --always --dirty --abbrev=9` of the crate, else the
/// build's `BITZ_REVISION`, else `unknown`).
fn revision() -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(root)
        .join(".git")
        .exists()
        .then(|| {
            std::process::Command::new("git")
                .args(["-C", root, "describe", "--always", "--dirty", "--abbrev=9"])
                .output()
                .ok()
        })
        .flatten()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| option_env!("BITZ_REVISION").unwrap_or("unknown").to_owned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut n: Option<usize> = None;
    let mut reps = 5usize;
    let mut seed = 1u64;
    // One of the crate's validated selections, resolved at 100 bits
    // (`custom:1:4`, the default as in the `bitz` CLI = the paper's
    // rate-1/2 Johnson ladder, `custom:3:4` = rate 1/8), or `fast` =
    // flock's embedded ladder as shipped. A Johnson ladder runs Round 0; a
    // unique-decoding one (`udr`, `udrg`) does not need it.
    let mut ladder = String::from("custom:1:4");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--reps" => {
                reps = args[i + 1].parse().expect("--reps");
                i += 2;
            }
            "--ladder" => {
                ladder = args[i + 1].clone();
                i += 2;
            }
            "--seed" => {
                seed = args[i + 1].parse().expect("--seed");
                i += 2;
            }
            x => {
                n = Some(x.parse().expect("n"));
                i += 1;
            }
        }
    }
    let n = n.expect("usage: bitz_bench <n> [--reps R] [--seed S] [--ladder custom:r:k|fast]");
    let shape = Shape::reference(n).expect("shape");
    let (t, s) = (shape.log_rows(), shape.log_columns());
    let layout = shape.layout();
    let opener = WfbitzOpener::new(
        layout,
        WfbitzLigerito::parse(&ladder, 100).expect("--ladder"),
        100,
    )
    .expect("opener");
    let q_bits = standalone_q_bits(&layout);
    let round0 = opener.ood_bits().is_some();
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    // The revision the rows come from, probed before any timer.
    let rev = revision();
    println!(
        "bitz_bench n={n} t={t} s={s} threads={threads} reps={reps} seed={seed} ladder={ladder} q_bits={q_bits} round0={round0} git_revision={rev}"
    );

    // The committed bits.
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let words = (1usize << t) / 64;
    let rows: Vec<Vec<u64>> = (0..1usize << s)
        .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
        .collect();

    // Commit: the median of `reps` commits, the last one kept.
    let mut commit_times = Vec::with_capacity(reps);
    let mut committed = None;
    for _ in 0..reps {
        let input = rows.clone();
        let started = Instant::now();
        let hint = opener.commit(input).expect("commit");
        commit_times.push(started.elapsed());
        committed = Some(hint);
    }
    drop(rows);
    let hint = committed.expect("committed");
    let commit = median(&commit_times);
    println!("commit: median {:.2} ms over {reps}", ms(commit));

    // The claimed value, once, outside the timers.
    let claimed = standalone_evaluation(&opener, &hint).expect("claimed value");

    // Prove: one warm-up, then `reps` timed and verified.
    let mut prove_times = Vec::with_capacity(reps);
    let mut verify_times = Vec::with_capacity(reps);
    let mut phase_times: Vec<(String, Vec<Duration>)> = Vec::new();
    let mut sizes = (0usize, 0usize, 0usize, 0usize);
    for rep in 0..=reps {
        record_phases(true);
        let started = Instant::now();
        let proof = prove_standalone(&opener, &hint, claimed).expect("prove");
        let elapsed = started.elapsed();
        let phases = take_phases();
        record_phases(false);
        if rep == 0 {
            continue;
        }
        prove_times.push(elapsed);
        for (label, d) in phases {
            match phase_times.iter_mut().find(|(l, _)| *l == label) {
                Some((_, v)) => v.push(d),
                None => phase_times.push((label, vec![d])),
            }
        }
        let started = Instant::now();
        verify_standalone(&opener, &hint.commitment, claimed, &proof).expect("verify");
        verify_times.push(started.elapsed());
        let total = proof.to_bytes().len();
        let ood = proof.ood.map_or(0, |round| 16 + round.nonce.map_or(0, |_| 8));
        sizes = (proof.narg.len(), proof.hints.len(), ood, total);
    }
    let prove = median(&prove_times);
    let verify = median(&verify_times);
    let phase = |label: &str| -> Duration {
        phase_times
            .iter()
            .find(|(l, _)| l == label)
            .map_or(Duration::ZERO, |(_, v)| median(v))
    };
    let grand = phase("fold+images") + phase("gkr");
    let ring = phase("sumcheck") + phase("ring switch");
    let lig = phase("ligerito");
    println!("prove: median {:.1} ms over {reps} (min {:.1}); phases:", ms(prove), ms(*prove_times.iter().min().expect("reps")));
    for (label, v) in &phase_times {
        println!("  {label:<24} {:>8.2} ms", ms(median(v)));
    }
    println!(
        "buckets: grand products {:.1} | ring switch incl. sumcheck {:.1} | ligerito {:.1} | total {:.1} (commit {:.2} + prove {:.1})",
        ms(grand), ms(ring), ms(lig), ms(commit + prove), ms(commit), ms(prove)
    );
    println!(
        "verify: median {:.2} ms; proof: narg {} B + hints {} B + ood {} B = {} B (encoded {} B)",
        ms(verify), sizes.0, sizes.1, sizes.2, sizes.0 + sizes.1 + sizes.2, sizes.3
    );
    let rss = peak_rss_bytes();
    println!("peak rss: {:.2} GB", rss as f64 / 1e9);
    println!(
        "RESULT schema=bitz-bench/2 git_revision={rev} n={n} t={t} s={s} threads={threads} reps={reps} seed={seed} ladder={ladder} q_bits={q_bits} round0={} commit_ms={:.3} prove_ms={:.3} grand_ms={:.3} ring_ms={:.3} lig_ms={:.3} verify_ms={:.3} narg_bytes={} hints_bytes={} ood_bytes={} encoded_bytes={} peak_rss_bytes={rss}",
        u8::from(round0), ms(commit), ms(prove), ms(grand), ms(ring), ms(lig), ms(verify), sizes.0, sizes.1, sizes.2, sizes.3
    );
}
