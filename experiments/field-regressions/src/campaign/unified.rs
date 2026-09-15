//! Actual shared API against the verbatim production references.
use crate::baseline_delayed::OptimizedMonty128Reducer;
use crate::{Case, Rng, baseline_raw::RawMontyCtx, case_requested, measure};
use crypto_bigint::{Odd, Uint as CryptoUint, modular::FixedMontyParams};
use field::{BatchFieldOps, BatchMulAcc, Fp, FpCtx, IntegerEmbedding, Reduce, RingOps, Uint};
use std::hint::black_box;

#[inline(never)]
fn typed_mul(ctx: &FpCtx<2>, a: &[Fp<2>], b: &[Fp<2>], out: &mut [Fp<2>]) {
    ctx.batch_mul_into(a, b, out);
}
#[inline(never)]
fn raw_mul(ctx: &RawMontyCtx, a: &[u128], b: &[u128], out: &mut [u128]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        *out = ctx.mul(*a, *b);
    }
}
#[inline(never)]
fn typed_dot(ctx: &FpCtx<2>, a: &[Fp<2>], b: &[Fp<2>]) -> Fp<2> {
    ctx.reduce(ctx.batch_mul_acc(a, b))
}
#[inline(never)]
fn typed_linear(ctx: &FpCtx<2>, a: &[Fp<2>], b: &[u64]) -> Fp<2> {
    ctx.reduce(ctx.batch_mul_acc(a, b))
}

pub(crate) fn run(samples: usize, rng: &mut Rng) {
    for (bits, q) in [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)] {
        let words = [q as u64, (q >> 64) as u64];
        let cfg = FixedMontyParams::new_vartime(Odd::new(CryptoUint::from_words(words)).unwrap());
        let old = RawMontyCtx::new(&cfg);
        let reducer = OptimizedMonty128Reducer::new(&cfg).unwrap();
        let field = field::create_prime_field(Uint::from_words(words));
        for n in [16, 1024, 65536] {
            let size = format!("q{bits}_n{n}");
            if !["unified_mul", "unified_dot", "unified_linear"]
                .iter()
                .any(|family| case_requested(family, &size))
            {
                continue;
            }
            let a: Vec<_> = (0..n)
                .map(|_| (rng.next() as u128 | ((rng.next() as u128) << 64)) % q)
                .collect();
            let b: Vec<_> = (0..n)
                .map(|_| (rng.next() as u128 | ((rng.next() as u128) << 64)) % q)
                .collect();
            let native: Vec<_> = (0..n).map(|_| rng.next()).collect();
            let fa: Vec<_> = a.iter().map(|a| field.from_integer(a)).collect();
            let fb: Vec<_> = b.iter().map(|b| field.from_integer(b)).collect();
            let ra: Vec<_> = a.iter().map(|a| old.native_residue_u128(*a)).collect();
            let rb: Vec<_> = b.iter().map(|b| old.native_residue_u128(*b)).collect();
            let mut fo = vec![field.zero(); n];
            let mut ro = vec![0; n];
            let mut ro_repeat = vec![0; n];
            typed_mul(&field, &fa, &fb, &mut fo);
            raw_mul(&old, &ra, &rb, &mut ro);
            for (f, r) in fo.iter().zip(&ro) {
                assert_eq!(
                    *field.to_integer(f).as_words(),
                    old.field(*r).retrieve().to_words()
                );
            }
            let result = super::prime::product_sum::<1, false>(q, &ra, &rb).reduce_raw(&reducer);
            assert_eq!(
                *field.to_integer(&typed_dot(&field, &fa, &fb)).as_words(),
                old.field(result).retrieve().to_words()
            );
            let result = super::prime::linear_sum::<1>(&ra, &native).reduce_raw(&reducer);
            assert_eq!(
                *field
                    .to_integer(&typed_linear(&field, &fa, &native))
                    .as_words(),
                old.field(result).retrieve().to_words()
            );
            measure(
                "unified_mul",
                &size,
                &mut [
                    Case::new("production", n * 48, || {
                        raw_mul(
                            black_box(&old),
                            black_box(&ra),
                            black_box(&rb),
                            black_box(&mut ro),
                        );
                    }),
                    Case::new("production_repeat", n * 48, || {
                        raw_mul(
                            black_box(&old),
                            black_box(&ra),
                            black_box(&rb),
                            black_box(&mut ro_repeat),
                        );
                    }),
                    Case::new("typed", n * 48, || {
                        typed_mul(
                            black_box(&field),
                            black_box(&fa),
                            black_box(&fb),
                            black_box(&mut fo),
                        );
                    }),
                ],
                samples,
                rng,
            );
            measure(
                "unified_dot",
                &size,
                &mut [
                    Case::new("production", n * 32, || {
                        black_box(
                            super::prime::product_sum::<1, false>(
                                q,
                                black_box(&ra),
                                black_box(&rb),
                            )
                            .reduce_raw(black_box(&reducer)),
                        );
                    }),
                    Case::new("production_repeat", n * 32, || {
                        black_box(
                            super::prime::product_sum::<1, false>(
                                q,
                                black_box(&ra),
                                black_box(&rb),
                            )
                            .reduce_raw(black_box(&reducer)),
                        );
                    }),
                    Case::new("typed", n * 32, || {
                        black_box(typed_dot(black_box(&field), black_box(&fa), black_box(&fb)));
                    }),
                ],
                samples,
                rng,
            );
            measure(
                "unified_linear",
                &size,
                &mut [
                    Case::new("production", n * 24, || {
                        black_box(
                            super::prime::linear_sum::<1>(black_box(&ra), black_box(&native))
                                .reduce_raw(black_box(&reducer)),
                        );
                    }),
                    Case::new("production_repeat", n * 24, || {
                        black_box(
                            super::prime::linear_sum::<1>(black_box(&ra), black_box(&native))
                                .reduce_raw(black_box(&reducer)),
                        );
                    }),
                    Case::new("typed", n * 24, || {
                        black_box(typed_linear(
                            black_box(&field),
                            black_box(&fa),
                            black_box(&native),
                        ));
                    }),
                ],
                samples,
                rng,
            );
        }
    }
}
