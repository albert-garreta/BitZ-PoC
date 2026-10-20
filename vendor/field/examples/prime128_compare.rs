//! Compare runtime and compile-time moduli using the same 128-bit prime.
//! Run: cargo run --release --manifest-path vendor/field/Cargo.toml --example prime128_compare

#[path = "support/prime128.rs"]
mod prime128;

use field::{IntegerEmbedding, RingOps};
use prime128::{Random128Field, sample_field};
use std::{hint::black_box, time::Instant};

const N: usize = 1024;
const BATCHES: usize = 1000;
const SAMPLES: usize = 9;

fn main() {
    let dynamic = sample_field();
    let fixed = Random128Field::new();

    // Identical full-width inputs; generation and conversion are outside timing.
    let mut state = 0x1234_5678_9abc_def0_u128;
    let raw: Vec<u128> = (0..2 * N)
        .map(|_| {
            state = state
                .wrapping_mul(0x2360_ed05_1fc6_5da4_4385_df64_9fcc_f645)
                .wrapping_add(1);
            state
        })
        .collect();
    let d: Vec<_> = raw.iter().map(|x| dynamic.from_integer(x)).collect();
    let s: Vec<prime128::Random128Element> = raw.iter().map(|x| fixed.from_integer(x)).collect();
    let (dx, dy) = d.split_at(N);
    let (sx, sy) = s.split_at(N);
    let mut dout = vec![dynamic.zero(); N];
    let mut sout = vec![fixed.zero(); N];

    assert_eq!(
        dynamic.to_integer(&mul_chain(&dynamic, dx)),
        fixed.to_integer(&mul_chain(&fixed, sx)),
    );
    mul_batch(&dynamic, dx, dy, &mut dout);
    mul_batch(&fixed, sx, sy, &mut sout);
    for (a, b) in dout.iter().zip(&sout) {
        assert_eq!(dynamic.to_integer(a), fixed.to_integer(b));
    }

    println!("Median ns/mul; setup and conversions excluded");
    compare(
        "mul/chain",
        || mul_chain(black_box(&dynamic), black_box(dx)),
        || mul_chain(&fixed, black_box(sx)),
    );
    compare(
        "mul/batch",
        || {
            mul_batch(black_box(&dynamic), black_box(dx), black_box(dy), &mut dout);
            black_box(&dout);
        },
        || {
            mul_batch(&fixed, black_box(sx), black_box(sy), &mut sout);
            black_box(&sout);
        },
    );
}

// Rust specializes these same loops for each context; there is no trait object.
fn mul_chain<F: RingOps>(field: &F, xs: &[F::Elem]) -> F::Elem {
    let mut acc = field.one();
    for x in xs {
        acc = field.mul(&acc, x);
    }
    acc
}

fn mul_batch<F: RingOps>(field: &F, xs: &[F::Elem], ys: &[F::Elem], out: &mut [F::Elem]) {
    for ((x, y), dst) in xs.iter().zip(ys).zip(out) {
        *dst = field.mul(x, y);
    }
}

fn measure<R>(body: &mut impl FnMut() -> R) -> f64 {
    let start = Instant::now();
    for _ in 0..BATCHES {
        black_box(body());
    }
    start.elapsed().as_secs_f64() * 1e9 / (N * BATCHES) as f64
}

fn compare<D, S>(name: &str, mut dynamic: impl FnMut() -> D, mut fixed: impl FnMut() -> S) {
    // Warm up both implementations, then alternate timing order to limit drift.
    measure(&mut dynamic);
    measure(&mut fixed);
    let mut d = [0.0; SAMPLES];
    let mut s = [0.0; SAMPLES];
    for i in 0..SAMPLES {
        if i % 2 == 0 {
            d[i] = measure(&mut dynamic);
            s[i] = measure(&mut fixed);
        } else {
            s[i] = measure(&mut fixed);
            d[i] = measure(&mut dynamic);
        }
    }
    d.sort_by(f64::total_cmp);
    s.sort_by(f64::total_cmp);
    let (d, s) = (d[SAMPLES / 2], s[SAMPLES / 2]);
    println!(
        "{name:12} dynamic {d:7.3}  static {s:7.3}  dynamic/static {ratio:.3}x",
        ratio = d / s
    );
}
