//! The end-to-end scheme's raw metrics at one size (stage E): a seeded
//! SHA-256 statement through `f2z::bitz::e2e` — setup (`Prepared::new`),
//! witness, commit, prove (one warm-up, then `reps` timed and verified;
//! medians of the wall time and of every traced phase), verify, proof
//! bytes, peak RSS — printed as one `RESULT` line, the way `bitz_bench`
//! does for the PCS. One size per process; threads are rayon's.
//!
//! `bitz_e2e_bench <sha256-compression|sha256-chain> <blocks> [--seed S]
//! [--reps R]`
use std::time::{Duration, Instant};

use f2z::bitz::e2e::Prepared;
use f2z::bitz::statements::{Sha256Circuit, Sha256Statement};
use f2z::bitz::{record_phases, take_phases};

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
    let circuit = Sha256Circuit::parse(&args[0]).expect("sha256-compression|sha256-chain");
    let blocks: usize = args[1].parse().expect("blocks");
    let mut seed = 7u64;
    let mut reps = 5usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                seed = args[i + 1].parse().expect("--seed");
                i += 2;
            }
            "--reps" => {
                reps = args[i + 1].parse().expect("--reps");
                i += 2;
            }
            other => panic!("unknown option {other}"),
        }
    }
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    println!("bitz_e2e_bench circuit={} blocks={blocks} seed={seed} reps={reps} threads={threads}", circuit.name());

    let statement = Sha256Statement::seeded(circuit, blocks, seed);
    let inputs = statement.input();
    let started = Instant::now();
    let prepared = Prepared::new(statement).expect("prepared");
    let setup = started.elapsed();
    let started = Instant::now();
    let witness = prepared.witness(&inputs).expect("witness");
    let witness_time = started.elapsed();
    println!(
        "setup {:.1} ms ({}; claim shape ({},{}), committed ({},{})); witness {:.1} ms",
        ms(setup),
        prepared.matrices().short_debug_info(),
        prepared.params().shape().log_rows(),
        prepared.params().shape().log_columns(),
        prepared.committed_shape().log_rows(),
        prepared.committed_shape().log_columns(),
        ms(witness_time)
    );

    let mut commit_times = Vec::with_capacity(reps);
    let mut committed = None;
    for _ in 0..reps {
        let started = Instant::now();
        let (root, hint) = prepared.commit(&witness).expect("commit");
        commit_times.push(started.elapsed());
        committed = Some((root, hint));
    }
    let (root, hint) = committed.expect("committed");
    let commit = median(&commit_times);

    let mut prove_times = Vec::with_capacity(reps);
    let mut verify_times = Vec::with_capacity(reps);
    let mut phase_times: Vec<(String, Vec<Duration>)> = Vec::new();
    let mut sizes = (0usize, 0usize, 0usize);
    for rep in 0..=reps {
        record_phases(true);
        let started = Instant::now();
        let proof = prepared.prove(&witness, &hint).expect("prove");
        let elapsed = started.elapsed();
        let phases = take_phases();
        record_phases(false);
        assert_eq!(proof.root, root);
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
        prepared.verify(&proof).expect("verify");
        verify_times.push(started.elapsed());
        sizes = (
            proof.opening.narg_string.len(),
            proof.opening.hints.len(),
            proof.spartan.to_bytes(&proof.terminal).len(),
        );
    }
    let prove = median(&prove_times);
    let verify = median(&verify_times);
    println!("commit: median {:.2} ms; prove: median {:.1} ms (min {:.1}); phases:", ms(commit), ms(prove), ms(*prove_times.iter().min().expect("reps")));
    for (label, v) in &phase_times {
        println!("  {label:<24} {:>8.2} ms", ms(median(v)));
    }
    let phase = |label: &str| -> Duration {
        phase_times.iter().find(|(l, _)| l == label).map_or(Duration::ZERO, |(_, v)| median(v))
    };
    let rss = peak_rss_bytes();
    println!(
        "verify: median {:.2} ms; proof: narg {} B + hints {} B + spartan {} B (out of band) = {} B; peak rss {:.2} GB",
        ms(verify),
        sizes.0,
        sizes.1,
        sizes.2,
        sizes.0 + sizes.1 + sizes.2,
        rss as f64 / 1e9
    );
    println!(
        "RESULT schema=bitz-e2e-bench/1 circuit={} blocks={blocks} seed={seed} threads={threads} reps={reps} setup_ms={:.3} witness_ms={:.3} commit_ms={:.3} prove_ms={:.3} spartan_ms={:.3} fold_ms={:.3} gkr_ms={:.3} transpose_ms={:.3} opening_ms={:.3} verify_ms={:.3} narg_bytes={} hints_bytes={} spartan_bytes={} peak_rss_bytes={rss}",
        circuit.name(),
        ms(setup),
        ms(witness_time),
        ms(commit),
        ms(prove),
        ms(phase("spartan")),
        ms(phase("fold+images")),
        ms(phase("gkr")),
        ms(phase("transpose")),
        ms(phase("opening (all)")),
        ms(verify),
        sizes.0,
        sizes.1,
        sizes.2
    );
}
