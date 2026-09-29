//! Standard (dense-leaf) GKR vs the low-entropy bit-driven GKR for the
//! batched grand products in the exponent. Same protocol, same sumcheck
//! engine, byte-identical transcripts; only the prover algorithm differs.
//!
//! ```text
//! GKR_SHAPES="17:10" GKR_MODE=std|low|check GKR_REPS=5 RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --example gkr_std_vs_lowent --features unchecked
//! ```
//! `std`: materialise the 2^n leaves `bit ? α^{w_b} : 1`, then the eager
//! prover (full product tree + dense sumchecks on every layer). `low`: the
//! default lazy prover from the packed bits. `check`: both, asserts equality.

use bitz::merged_forest::{prove_merged_forest, prove_merged_forest_lazy};
use bitz::pcs::IntegerMatrixLayout;
use bitz::poly::univariate::binary_gf128::Gf128 as Gf;
use bitz::transcript::Blake3Transcript;
use bitz::transcript::traits::Transcript;
use std::time::Instant;

fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let mode = std::env::var("GKR_MODE").unwrap_or_else(|_| "check".into());
    let reps: usize = std::env::var("GKR_REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let shapes: Vec<(usize, usize)> = std::env::var("GKR_SHAPES")
        .unwrap_or_else(|_| "10:6".into())
        .split_whitespace()
        .map(|p| {
            let mut it = p.split(':');
            (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap())
        })
        .collect();

    for (t, s) in shapes {
        let p = IntegerMatrixLayout { row_vars: t, col_vars: s, word_bits: 1 };
        let (rows_n, cols_n) = (1usize << t, 1usize << s);
        // Per-row base α^{w_b} (values free for cost purposes).
        let pow2: Vec<Vec<Gf>> = (0..rows_n)
            .map(|b| {
                let lo = mix(b as u64) as u128;
                let hi = mix(b as u64 ^ 0xABCD) as u128;
                vec![Gf::from_polynomial_bits(lo | (hi << 64))]
            })
            .collect();
        // Uniform random bits: cell (b, c).
        let bit = |b: usize, c: usize| (mix(((c as u64) << 32) ^ b as u64 ^ 0x5151) & 1) == 1;
        // Packed layout: packed[c>>6][b] bit (c&63).
        let packed: Vec<Vec<u64>> = (0..cols_n.div_ceil(64))
            .map(|g| {
                (0..rows_n)
                    .map(|b| {
                        let mut w = 0u64;
                        for k in 0..64.min(cols_n - 64 * g) {
                            if bit(b, 64 * g + k) {
                                w |= 1 << k;
                            }
                        }
                        w
                    })
                    .collect()
            })
            .collect();
        let one = Gf::from_polynomial_bits(1);
        let make_leaves = || -> Vec<Gf> {
            let mut v = vec![one; rows_n << s];
            #[cfg(feature = "parallel")]
            {
                use rayon::prelude::*;
                v.par_chunks_mut(rows_n).enumerate().for_each(|(c, tree)| {
                    let (g, k) = (c >> 6, c & 63);
                    for (b, x) in tree.iter_mut().enumerate() {
                        if (packed[g][b] >> k) & 1 == 1 {
                            *x = pow2[b][0];
                        }
                    }
                });
            }
            v
        };

        let run_std = || {
            let t0 = Instant::now();
            let leaves = make_leaves();
            let t1 = Instant::now();
            let mut tr = Blake3Transcript::new();
            let out = prove_merged_forest(&mut tr, &leaves, t, s);
            let t2 = Instant::now();
            let c: Gf = tr.get_field_challenge(&());
            drop(leaves);
            ((t1 - t0).as_secs_f64() * 1e3, (t2 - t1).as_secs_f64() * 1e3, out, c)
        };
        let run_low = || {
            let t0 = Instant::now();
            let mut tr = Blake3Transcript::new();
            let out = prove_merged_forest_lazy(&mut tr, &p, &packed, &pow2, cols_n);
            let dt = t0.elapsed().as_secs_f64() * 1e3;
            let c: Gf = tr.get_field_challenge(&());
            (dt, out, c)
        };

        let n = t + s;
        match mode.as_str() {
            "check" => {
                let (_, _, a, ca) = run_std();
                let (_, b, cb) = run_low();
                assert_eq!(a.0, b.0, "roots");
                assert_eq!(a.1.layers.len(), b.1.layers.len());
                for (x, y) in a.1.layers.iter().zip(&b.1.layers) {
                    assert_eq!(x.sc_x, y.sc_x);
                    assert_eq!(x.sc_c, y.sc_c);
                    assert_eq!(x.pair, y.pair);
                }
                assert_eq!(a.2, b.2);
                assert_eq!(a.3, b.3);
                assert_eq!(ca, cb, "transcript state");
                println!("n={n} t={t} s={s}: std == low (byte-identical)");
            }
            "std" => {
                let _ = run_std();
                let (mut l, mut f) = (vec![], vec![]);
                for _ in 0..reps {
                    let (a, b, _, _) = run_std();
                    l.push(a);
                    f.push(b);
                }
                let tot: Vec<f64> = l.iter().zip(&f).map(|(a, b)| a + b).collect();
                println!(
                    "RESULT std n={n} t={t} s={s} leaves_ms={:.1} gkr_ms={:.1} total_ms={:.1}",
                    median(l), median(f), median(tot)
                );
            }
            "low" => {
                let _ = run_low();
                let v: Vec<f64> = (0..reps).map(|_| run_low().0).collect();
                println!("RESULT low n={n} t={t} s={s} total_ms={:.1}", median(v));
            }
            m => panic!("GKR_MODE={m}?"),
        }
    }
}
