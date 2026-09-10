//! Phase profile of one hybrid (mod-2^32 mul + chained SHA-256) prove via the
//! crate's env-gated `utils::prof` scaffold. Bench-identical inputs; one
//! warm-up prove is dumped and discarded, then `PROBE_REPS` profiled proves
//! (default 1) per shape. The nested tree lands on stderr.
//!
//! ```text
//! OBLONG_PROFILE=1 PROBE_SHAPES="19:11 20:12" RAYON_NUM_THREADS=8 \
//!   RUSTFLAGS="-C target-cpu=native" cargo run --release --example hybrid_probe --features hybrid
//! ```
use f2z::hybrid::{Parameters, PreparedHybrid, U32MulMod32Row};
use std::time::Instant;

fn rss_peak() -> u64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    usage.ru_maxrss as u64
}

fn main() {
    f2z::utils::prof::set_rss_probe(rss_peak);
    let shapes: Vec<(u32, u32)> = std::env::var("PROBE_SHAPES")
        .map(|v| {
            v.split_whitespace()
                .map(|p| {
                    let mut it = p.split(':');
                    (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap())
                })
                .collect()
        })
        .unwrap_or_else(|_| vec![(19, 11)]);
    let reps: usize = std::env::var("PROBE_REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let verify = std::env::var("PROBE_VERIFY").map_or(true, |v| v != "0");
    for (mul_log, sha_log) in shapes {
        let parameters = Parameters { multiplications: 1 << mul_log, sha_compressions: 1 << sha_log };
        let t0 = Instant::now();
        let prepared = PreparedHybrid::new(parameters).expect("prepare");
        eprintln!("setup {}:{} {:.0} ms", mul_log, sha_log, t0.elapsed().as_secs_f64() * 1e3);
        let inputs: Vec<_> = (0..parameters.multiplications as u32)
            .map(|i| (i.wrapping_mul(0x9e3779b9), u32::MAX - i))
            .collect();
        let blocks: Vec<[u32; 16]> = (0..parameters.sha_compressions as u32)
            .map(|i| std::array::from_fn(|j| i.wrapping_mul(0x85ebca6b).wrapping_add(j as u32)))
            .collect();
        for rep in 0..=reps {
            let start = Instant::now();
            let rows: Vec<_> = inputs.iter().map(|&(x, y)| U32MulMod32Row::new(x, y)).collect();
            let committed = prepared.commit_mod32(&rows, &blocks).expect("commit");
            let commit_ms = start.elapsed().as_secs_f64() * 1e3;
            let t1 = Instant::now();
            let proof = prepared.prove(&committed).expect("prove");
            let prove_ms = t1.elapsed().as_secs_f64() * 1e3;
            let bytes = proof.to_bytes();
            let header = if rep == 0 {
                format!("warmup {mul_log}:{sha_log} (discard) commit {commit_ms:.1} ms prove {prove_ms:.1} ms")
            } else {
                format!(
                    "{mul_log}:{sha_log} rep {rep}: commit {commit_ms:.1} ms + prove {prove_ms:.1} ms = {:.1} ms, proof {} B",
                    commit_ms + prove_ms,
                    bytes.len()
                )
            };
            f2z::utils::prof::dump_and_reset(&header);
            if verify {
                let t2 = Instant::now();
                let decoded = prepared.proof_from_bytes(committed.statement(), &bytes).expect("decode");
                prepared.verify(committed.statement(), &decoded).expect("verify");
                let verify_ms = t2.elapsed().as_secs_f64() * 1e3;
                f2z::utils::prof::dump_and_reset(&format!("verify {mul_log}:{sha_log} rep {rep}: {verify_ms:.1} ms"));
            }
            eprintln!("digest {}", blake3::hash(&bytes).to_hex());
        }
    }
}
