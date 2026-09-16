//! The paper's raw-performance row for the BitZ parity prover at one size.
//!
//! `bitz_bench <n> [--reps R] [--seed S]` builds a random instance at the
//! reference split (`t = max(⌈3n/5⌉, 7)` capped at `n − 1`, `s = n − t`,
//! `q = 2^100 − 15`, generator `X`, the dump examples' transcript labels),
//! commits it `R` times (median), proves it once to warm up and then `R`
//! times (medians of the wall time and of every traced phase, the paper's
//! buckets summed from them), verifies every timed proof (median), and
//! prints one `RESULT` line. One size per process so the peak resident set
//! is the shape's; the thread count is rayon's (`RAYON_NUM_THREADS`).
//!
//! Buckets, as the paper's table splits the F2Z prover: grand products =
//! `fold+images` + `gkr` (the integer folds, the images, the batched GKR);
//! ring switch incl. its sumcheck = `sumcheck` + `ring switch` (the
//! reduction of the GKR exit claim to one packed evaluation and the
//! ring-switch message); Ligerito = `ligerito`; Total = commit + prove.
//! Proof bytes: narg = every transcript message (non-Ligerito), hints =
//! the serialized Ligerito proof.
use std::time::{Duration, Instant};

use f2z::bitz::fold::{fold_columns, reconstruct};
use f2z::bitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Shape, WINDOW, build_prover,
    build_verifier, record_phases, take_phases,
};
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use flock_core::merkle::HashKind;

/// `2^100 − 15`, the dump examples' prime.
const Q: u128 = (1u128 << 100) - 15;
const SESSION: &str = "bitz-tests";
const INSTANCE: &str = "fold-round-trip";

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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut n: Option<usize> = None;
    let mut reps = 5usize;
    let mut seed = 1u64;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--reps" => {
                reps = args[i + 1].parse().expect("--reps");
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
    let n = n.expect("usage: bitz_bench <n> [--reps R] [--seed S]");
    let t = ((3 * n).div_ceil(5)).max(7).min(n - 1);
    let s = n - t;
    let shape = Shape::new(t, s).expect("shape");
    let params = BitZParams::new(shape, Q, Gf::from_words([2, 0])).expect("params");
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    println!("bitz_bench n={n} t={t} s={s} threads={threads} reps={reps} seed={seed}");

    // The instance.
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let words = (1usize << t) / 64;
    let rows: Vec<Vec<u64>> = (0..1usize << s)
        .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
        .collect();
    let mut residue = || {
        let hi = u128::from(xorshift(&mut state));
        let lo = u128::from(xorshift(&mut state));
        ((hi << 64) | lo) % Q
    };
    let row_weights: Vec<u128> = (0..1usize << t).map(|_| residue()).collect();
    let column_weights: Vec<u128> = (0..1usize << s).map(|_| residue()).collect();
    let folds = fold_columns(&shape, &rows, &row_weights);
    let unresolved = LinearClaim::new(&params, row_weights.clone(), column_weights.clone(), 0).expect("claim");
    let target = reconstruct(&unresolved, &folds, Q);
    let claim = LinearClaim::new(&params, row_weights, column_weights, target).expect("claim");
    let pcs = Pcs::new(&shape, HashKind::Blake3).expect("pcs");

    // Commit: the median of `reps` commits, the last one kept.
    let mut commit_times = Vec::with_capacity(reps);
    let mut committed = None;
    for _ in 0..reps {
        let input = rows.clone();
        let started = Instant::now();
        let (root, hint) = pcs.commit(&shape, input).expect("commit");
        commit_times.push(started.elapsed());
        committed = Some((root, hint));
    }
    drop(rows);
    let (root, hint) = committed.expect("committed");
    let commit = median(&commit_times);
    println!("commit: median {:.2} ms over {reps}", ms(commit));

    // Prove: one warm-up, then `reps` timed and verified.
    let prover = BitZProver::new(params, WINDOW);
    let verifier = BitZVerifier::new(params, WINDOW);
    let mut prove_times = Vec::with_capacity(reps);
    let mut verify_times = Vec::with_capacity(reps);
    let mut phase_times: Vec<(String, Vec<Duration>)> = Vec::new();
    let mut sizes = (0usize, 0usize);
    for rep in 0..=reps {
        record_phases(true);
        let started = Instant::now();
        let mut transcript = build_prover(SESSION, INSTANCE);
        prover.prove(&claim, &pcs, &hint, &mut transcript).expect("prove");
        let proof = transcript.finish();
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
        verifier
            .verify(&claim, &pcs, root, build_verifier(SESSION, INSTANCE, &proof))
            .expect("verify");
        verify_times.push(started.elapsed());
        sizes = (proof.narg_string.len(), proof.hints.len());
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
    println!("verify: median {:.2} ms; proof: narg {} B + hints {} B = {} B", ms(verify), sizes.0, sizes.1, sizes.0 + sizes.1);
    let rss = peak_rss_bytes();
    println!("peak rss: {:.2} GB", rss as f64 / 1e9);
    println!(
        "RESULT schema=bitz-bench/1 n={n} t={t} s={s} threads={threads} reps={reps} seed={seed} commit_ms={:.3} prove_ms={:.3} grand_ms={:.3} ring_ms={:.3} lig_ms={:.3} verify_ms={:.3} narg_bytes={} hints_bytes={} peak_rss_bytes={rss}",
        ms(commit), ms(prove), ms(grand), ms(ring), ms(lig), ms(verify), sizes.0, sizes.1
    );
}
