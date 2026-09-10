//! Historical kernel experiment; no production security claim.
//! Phase profile of one mod-q prove via the crate's env-gated `utils::prof`
//! scaffold — the crate's only `dump_and_reset` caller. Bench-identical
//! instance; warm-up prove dumped and discarded, then ONE profiled prove per
//! shape.
//!
//! ```text
//! OBLONG_PROFILE=1 PROBE_SHAPES="17:11 18:12" RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --example prof_probe --features unchecked
//! ```

use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito, historical_sha_lig_configs};
use f2z::pcs::{IntEvalParams, smallest_generator};

const Q: u128 = (1u128 << 100) - 15;

fn main() {
    let alpha = smallest_generator();
    let q_bits = 100usize;
    let shapes: Vec<(usize, usize)> = std::env::var("PROBE_SHAPES")
        .map(|v| {
            v.split_whitespace()
                .map(|p| {
                    let mut it = p.split(':');
                    (
                        it.next().unwrap().parse().unwrap(),
                        it.next().unwrap().parse().unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_else(|_| vec![(17, 11), (18, 12)]);

    for (t, s) in shapes {
        let p = IntEvalParams { t, s, word_bits: 1 };
        let m_p = packed_vars(&p);
        let (pc, _vc) = historical_sha_lig_configs(m_p).expect("lig cfg");

        let cell = |b: usize, c: usize| -> u128 {
            (p.cell_index(b, c) as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1
        };
        let words = p.rows().div_ceil(64);
        let rows: Vec<Vec<u64>> = (0..p.cols())
            .map(|c| {
                let mut wv = vec![0u64; words];
                for b in 0..p.rows() {
                    if cell(b, c) & 1 == 1 {
                        wv[b >> 6] |= 1u64 << (b & 63);
                    }
                }
                wv
            })
            .collect();
        let hint = commit_rs_ligerito_rows(&p, rows, &pc);
        let rw_q: Vec<u128> = (0..p.rows())
            .map(|b| {
                (b as u128)
                    .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                    .wrapping_add(7)
                    % Q
            })
            .collect();

        // Warm-up (profile discarded).
        {
            let mut pt = f2z::transcript::Blake3Transcript::new();
            let pr = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            std::hint::black_box(&pr);
            f2z::utils::prof::dump_and_reset("warmup (discard)");
        }
        // Profiled prove.
        let t0 = std::time::Instant::now();
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&proof);
        f2z::utils::prof::dump_and_reset(&format!(
            "n={} (t={t}, s={s}, W=1) — prove {ms:.1} ms",
            t + s
        ));
    }
}
