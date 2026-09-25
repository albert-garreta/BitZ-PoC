//! Inner products: runtime/static 128-bit prime × u32/u64/u128/field.
//!
//! cargo run --release --manifest-path vendor/field/Cargo.toml \
//!   --example prime128_inner_product -- 10 28 5 16384 > inner-products.csv
//!
//! Arguments: minimum exponent, maximum exponent, odd sample count, maximum
//! resident input MiB. Defaults: 10 28 5 16384. Oversized cases emit skipped rows.
//! Memory limit counts input arrays, not allocator overhead or process RSS.
//! Generation, conversion, prime search, and correctness checks are not timed.
//! Each operand type runs both delayed reduction and eager reduction. The CSV
//! `reduction` column identifies the case; all four results must agree.

#[path = "support/prime128.rs"]
mod prime128;

use field::{BatchMulAcc, Fp, FpCtx, IntegerEmbedding, Reduce, RingOps, Uint, WideMul};
use prime128::{Random128Element, Random128Field, sample_field};
use rand_core::{RngCore, SeedableRng};
use rand_pcg::Pcg64;
use std::{hint::black_box, io, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "prime128_inner_product [min_log=10] [max_log=28] [samples=5] [max_input_mib=16384]"
        );
        println!(
            "CSV on stdout; setup diagnostics on stderr. Oversized cases are marked skipped_memory."
        );
        return Ok(());
    }
    if args.len() > 4 {
        return Err("expected at most four arguments; see --help".into());
    }
    let mut config = [10usize, 28, 5, 16384];
    for (value, arg) in config.iter_mut().zip(args) {
        *value = arg.parse()?;
    }
    let [min_log, max_log, samples, max_input_mib] = config;
    if min_log > max_log || max_log > 30 || samples == 0 || samples % 2 == 0 {
        return Err("require 0 <= min_log <= max_log <= 30 and a positive odd sample count".into());
    }
    let max_bytes = max_input_mib
        .checked_mul(1 << 20)
        .ok_or("memory limit overflow")?;
    let dynamic = sample_field();
    let fixed = Random128Field::new();
    let mut bench = Bench {
        dynamic: &dynamic,
        fixed: &fixed,
        samples,
        csv: csv::Writer::from_writer(io::stdout()),
    };
    bench.csv.write_record([
        "log2_n",
        "n",
        "operand",
        "reduction",
        "status",
        "input_bytes",
        "iterations",
        "dynamic_ns",
        "static_ns",
        "dynamic_ns_per_element",
        "static_ns_per_element",
        "dynamic_over_static",
    ])?;

    for log in min_log..=max_log {
        let n = 1usize << log;
        // Two weight arrays (32 bytes/term); shared integer or two field RHS arrays.
        if n * 36 > max_bytes {
            for (kind, bytes_per_term) in [("u32", 36), ("u64", 40), ("u128", 48), ("field", 64)] {
                bench.skip(log, kind, n * bytes_per_term)?;
            }
            continue;
        }
        eprintln!("Measuring 2^{log} = {n} elements");
        let mut rng = Pcg64::seed_from_u64(42);
        let (dw, sw): (Vec<_>, Vec<_>) = (0..n)
            .map(|_| {
                let x = next_u128(&mut rng);
                (dynamic.from_integer(&x), fixed.from_integer(&x))
            })
            .unzip();

        macro_rules! mixed {
            ($integer:ty) => {{
                let kind = stringify!($integer);
                let bytes = n * (32 + size_of::<$integer>());
                if bytes > max_bytes {
                    bench.skip(log, kind, bytes)?;
                } else {
                    let mut rng = Pcg64::seed_from_u64(43);
                    let values: Vec<$integer> =
                        (0..n).map(|_| next_u128(&mut rng) as $integer).collect();
                    let delayed = bench.compare(
                        log,
                        kind,
                        "delayed",
                        bytes,
                        || {
                            let f = black_box(&dynamic);
                            f.reduce(f.batch_mul_acc(black_box(&dw), black_box(&values)))
                        },
                        || fixed.reduce(fixed.batch_mul_acc(black_box(&sw), black_box(&values))),
                    )?;
                    // The mixed product is reduced directly; integers stay native.
                    let eager = bench.compare(
                        log,
                        kind,
                        "eager",
                        bytes,
                        || {
                            inner_product_eager_mixed(
                                black_box(&dynamic),
                                black_box(&dw),
                                black_box(&values),
                            )
                        },
                        || inner_product_eager_mixed(&fixed, black_box(&sw), black_box(&values)),
                    )?;
                    assert_eq!(
                        eager, delayed,
                        "eager/delayed mismatch for {kind} at 2^{log}"
                    );
                }
            }};
        }
        mixed!(u32);
        mixed!(u64);
        mixed!(u128);

        let bytes = n * 64;
        if bytes > max_bytes {
            bench.skip(log, "field", bytes)?;
        } else {
            let mut rng = Pcg64::seed_from_u64(43);
            let (dv, sv): (Vec<_>, Vec<_>) = (0..n)
                .map(|_| {
                    let x = next_u128(&mut rng);
                    (dynamic.from_integer(&x), fixed.from_integer(&x))
                })
                .unzip();
            let delayed = bench.compare(
                log,
                "field",
                "delayed",
                bytes,
                || {
                    let f = black_box(&dynamic);
                    f.reduce(f.batch_mul_acc(black_box(&dw), black_box(&dv)))
                },
                || fixed.reduce(fixed.batch_mul_acc(black_box(&sw), black_box(&sv))),
            )?;
            let eager = bench.compare(
                log,
                "field",
                "eager",
                bytes,
                || inner_product_eager(black_box(&dynamic), black_box(&dw), black_box(&dv)),
                || inner_product_eager(&fixed, black_box(&sw), black_box(&sv)),
            )?;
            assert_eq!(
                eager, delayed,
                "eager/delayed mismatch for field at 2^{log}"
            );
        }
    }
    Ok(())
}

/// Both operations return reduced field elements; no wide sum is retained.
fn inner_product_eager<F: RingOps>(field: &F, weights: &[F::Elem], values: &[F::Elem]) -> F::Elem {
    assert_eq!(weights.len(), values.len());
    let mut sum = field.zero();
    for (weight, value) in weights.iter().zip(values) {
        let product = field.mul(weight, value); // Reduced modulo p.
        sum = field.add(&sum, &product); // Reduced modulo p again.
    }
    sum
}

/// Native integers need no preconverted field array. Reduce each mixed product
/// before adding it to the reduced field sum.
fn inner_product_eager_mixed<F, E, T, P>(field: &F, weights: &[E], values: &[T]) -> E
where
    F: RingOps<Elem = E> + WideMul<E, T, Product = P> + Reduce<P, Output = E>,
{
    assert_eq!(weights.len(), values.len());
    let mut sum = field.zero();
    for (weight, value) in weights.iter().zip(values) {
        let product = field.reduce(field.mul_wide(weight, value));
        sum = field.add(&sum, &product);
    }
    sum
}

struct Bench<'a> {
    dynamic: &'a FpCtx<2>,
    fixed: &'a Random128Field,
    samples: usize,
    csv: csv::Writer<io::Stdout>,
}

impl Bench<'_> {
    fn skip(&mut self, log: usize, kind: &str, bytes: usize) -> csv::Result<()> {
        for reduction in ["delayed", "eager"] {
            self.csv.serialize((
                log,
                1usize << log,
                kind,
                reduction,
                "skipped_memory",
                bytes,
                0,
                "",
                "",
                "",
                "",
                "",
            ))?;
        }
        self.csv.flush()?;
        Ok(())
    }

    fn compare(
        &mut self,
        log: usize,
        kind: &str,
        reduction: &str,
        bytes: usize,
        mut dynamic: impl FnMut() -> Fp<2>,
        mut fixed: impl FnMut() -> Random128Element,
    ) -> csv::Result<Uint<2>> {
        // These untimed calls also warm up both implementations.
        let result = self.dynamic.to_integer(&dynamic());
        assert_eq!(result, self.fixed.to_integer(&fixed()));
        let n = 1usize << log;
        // Amortize timer overhead at small N without repeating huge cases excessively.
        let iterations = ((1usize << 20) / n).max(1);
        let mut d = vec![0.0; self.samples];
        let mut s = vec![0.0; self.samples];
        for i in 0..self.samples {
            if i % 2 == 0 {
                d[i] = measure(iterations, &mut dynamic);
                s[i] = measure(iterations, &mut fixed);
            } else {
                s[i] = measure(iterations, &mut fixed);
                d[i] = measure(iterations, &mut dynamic);
            }
        }
        d.sort_by(f64::total_cmp);
        s.sort_by(f64::total_cmp);
        let (d, s) = (d[self.samples / 2], s[self.samples / 2]);
        self.csv.serialize((
            log,
            n,
            kind,
            reduction,
            "ok",
            bytes,
            iterations,
            d,
            s,
            d / n as f64,
            s / n as f64,
            d / s,
        ))?;
        self.csv.flush()?;
        Ok(result)
    }
}

fn measure<R>(iterations: usize, body: &mut impl FnMut() -> R) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(body());
    }
    start.elapsed().as_secs_f64() * 1e9 / iterations as f64
}

fn next_u128(rng: &mut impl RngCore) -> u128 {
    u128::from(rng.next_u64()) | (u128::from(rng.next_u64()) << 64)
}
