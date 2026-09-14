use super::pass;
use crate::production_delayed::OptimizedMonty128Reducer as Reducer;
use crate::{Case, Rng, case_requested, measure, production_raw::RawMontyCtx as Ctx};
use crypto_bigint::{
    Odd, Uint,
    modular::{FixedMontyForm, FixedMontyParams},
};
use num_traits::Inv;
use std::hint::black_box;

#[cfg(test)]
#[path = "correctness/prime.rs"]
mod tests;

type Config = FixedMontyParams<2>;
fn cfg(q: u128) -> Config {
    Config::new_vartime(Odd::new(Uint::from_words([q as u64, (q >> 64) as u64])).unwrap())
}
fn uint(x: u128) -> Uint<2> {
    Uint::from_words([x as u64, (x >> 64) as u64])
}
fn raw(x: Uint<2>) -> u128 {
    let w = x.to_words();
    w[0] as u128 | ((w[1] as u128) << 64)
}

// The caller establishes the modulus and slice lengths once. Zero handling
// uses masks, including all-zero inputs; the inversion primitive is selectable
// only as a public policy. CT here means fixed source schedule, not certification.
#[inline(never)]
fn batch<const CT: bool>(ctx: &Ctx, input: &[u128], prefix: &mut [u128], out: &mut [u128]) {
    assert_eq!(input.len(), prefix.len());
    assert_eq!(input.len(), out.len());
    let mut p = ctx.one();
    for (&x, t) in input.iter().zip(prefix.iter_mut()) {
        *t = p;
        let mask = 0u128.wrapping_sub((x != 0) as u128);
        let selected = (x & mask) | (ctx.one() & !mask);
        p = ctx.mul(p, selected);
    }
    let mut inv = if CT {
        let f = FixedMontyForm::from_montgomery(uint(p), ctx.config());
        raw(f.invert().unwrap().to_montgomery())
    } else {
        ctx.raw(&ctx.field(p).inv().unwrap())
    };
    for ((&x, &p), out) in input.iter().zip(prefix.iter()).zip(out.iter_mut()).rev() {
        let mask = 0u128.wrapping_sub((x != 0) as u128);
        *out = ctx.mul(inv, p) & mask;
        inv = ctx.mul(inv, (x & mask) | (ctx.one() & !mask));
    }
}

fn batch_fields<const CT: bool>(
    ctx: &Ctx,
    fields: &[crypto_primitives::crypto_bigint_monty::MontyField<2>],
) -> Vec<crypto_primitives::crypto_bigint_monty::MontyField<2>> {
    // Allocate exactly prefix and returned configured output, as the
    // production API does. Raw input is read directly from fields.

    let mut prefixes = Vec::with_capacity(fields.len());
    let mut p = ctx.one();
    for x in fields {
        prefixes.push(p);
        let x = ctx.raw(x);
        let m = 0u128.wrapping_sub((x != 0) as u128);
        p = ctx.mul(p, (x & m) | (ctx.one() & !m));
    }
    let mut inv = if CT {
        raw(FixedMontyForm::from_montgomery(uint(p), ctx.config())
            .invert()
            .unwrap()
            .to_montgomery())
    } else {
        ctx.raw(&ctx.field(p).inv().unwrap())
    };
    let mut output = vec![ctx.field(0); fields.len()];
    for ((x, &p), o) in fields.iter().zip(&prefixes).zip(output.iter_mut()).rev() {
        let x = ctx.raw(x);
        let m = 0u128.wrapping_sub((x != 0) as u128);
        *o = ctx.field(ctx.mul(inv, p) & m);
        inv = ctx.mul(inv, (x & m) | (ctx.one() & !m));
    }
    output
}

// Variable-time public-input API, matching production_one_inverse's contract.
// An all-zero batch needs no inversion or prefix allocation. For a nonempty
// product, omit the first multiplication by one and the last unused update.
fn batch_public_nonzero(
    ctx: &Ctx,
    fields: &[crypto_primitives::crypto_bigint_monty::MontyField<2>],
) -> Vec<crypto_primitives::crypto_bigint_monty::MontyField<2>> {
    let mut output = vec![ctx.field(0); fields.len()];
    let Some(first) = fields.iter().position(|x| ctx.raw(x) != 0) else {
        return output;
    };
    let mut product = ctx.raw(&fields[first]);
    let rest = &fields[first + 1..];
    let mut prefixes = Vec::with_capacity(rest.len());
    for x in rest {
        prefixes.push(product);
        let x = ctx.raw(x);
        if x != 0 {
            product = ctx.mul(product, x);
        }
    }
    let mut inverse = ctx.raw(&ctx.field(product).inv().unwrap());
    for ((x, &prefix), out) in rest
        .iter()
        .zip(&prefixes)
        .zip(&mut output[first + 1..])
        .rev()
    {
        let x = ctx.raw(x);
        if x != 0 {
            *out = ctx.field(ctx.mul(inverse, prefix));
            inverse = ctx.mul(inverse, x);
        }
    }
    output[first] = ctx.field(inverse);
    output
}

fn inversions(samples: usize, rng: &mut Rng) {
    for (bits, q) in [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)] {
        let cfg = cfg(q);
        let ctx = Ctx::new(&cfg);
        for class in ["nonzero", "mixed", "zero"] {
            for n in [0, 1, 3, 17, 16, 1024] {
                let size = format!("q{bits}_{class}_n{n}");
                if (n == 16 || n == 1024)
                    && !["opt_batch_inverse", "opt_batch_inverse_reuse"]
                        .iter()
                        .any(|f| case_requested(f, &size))
                {
                    continue;
                }
                let plain: Vec<_> = (0..n)
                    .map(|i| {
                        if class == "zero" || (class == "mixed" && i % 3 == 0) {
                            0
                        } else {
                            (rng.next() as u128 | ((rng.next() as u128) << 64)) % (q - 1) + 1
                        }
                    })
                    .collect();
                let input: Vec<_> = plain.iter().map(|&x| ctx.native_residue_u128(x)).collect();
                let fields: Vec<_> = input.iter().map(|&x| ctx.field(x)).collect();
                let old = crate::production_batch::batch_invert_nonzero(&fields, &cfg);
                let expected: Vec<_> = old.iter().map(|x| ctx.raw(x)).collect();
                assert_eq!(batch_fields::<true>(&ctx, &fields), old);
                assert_eq!(batch_fields::<false>(&ctx, &fields), old);
                assert_eq!(batch_public_nonzero(&ctx, &fields), old);
                for ((&x, &y), &p) in input.iter().zip(&expected).zip(&plain) {
                    assert_eq!(ctx.mul(x, y), if p == 0 { 0 } else { ctx.one() });
                }
                let mut prefix = vec![0; n];
                let mut out = vec![0; n];
                batch::<true>(&ctx, &input, &mut prefix, &mut out);
                assert_eq!(out, expected);
                batch::<false>(&ctx, &input, &mut prefix, &mut out);
                assert_eq!(out, expected);
                if n != 16 && n != 1024 {
                    continue;
                }
                // Both allocate prefix and output. Output representation conversion is
                // included in the compact candidate, so this is an API-matched call.
                let mut cases = vec![Case::new("production_one_inverse", 0, || {
                    black_box(crate::production_batch::batch_invert_nonzero(
                        black_box(&fields),
                        black_box(&cfg),
                    ));
                })];
                cases.push(Case::new("public_nonzero", 0, || {
                    black_box(batch_public_nonzero(black_box(&ctx), black_box(&fields)));
                }));
                for (name, ct) in [("compact_vartime", false), ("compact_ct", true)] {
                    let fields = &fields;
                    let ctx = &ctx;
                    cases.push(Case::new(name, 0, move || {
                        let output = if ct {
                            batch_fields::<true>(black_box(ctx), black_box(fields))
                        } else {
                            batch_fields::<false>(black_box(ctx), black_box(fields))
                        };
                        black_box(output);
                    }));
                }
                measure("opt_batch_inverse", &size, &mut cases, samples, rng);
                let mut p0 = vec![0; n];
                let mut o0 = vec![0; n];
                let mut p1 = vec![0; n];
                let mut o1 = vec![0; n];
                let mut cases = vec![
                    Case::new("compact_vartime", n * 64, || {
                        batch::<false>(
                            black_box(&ctx),
                            black_box(&input),
                            black_box(&mut p0),
                            black_box(&mut o0),
                        );
                        black_box(&o0);
                    }),
                    Case::new("compact_ct", n * 64, || {
                        batch::<true>(
                            black_box(&ctx),
                            black_box(&input),
                            black_box(&mut p1),
                            black_box(&mut o1),
                        );
                        black_box(&o1);
                    }),
                ];
                measure("opt_batch_inverse_reuse", &size, &mut cases, samples, rng);
            }
        }
    }
}

fn mac(samples: usize, rng: &mut Rng) {
    use super::super::prime::{linear_sum, product_sum};
    for (bits, q) in [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)] {
        let cfg = cfg(q);
        let red = Reducer::new(&cfg).unwrap();
        for n in [16, 1024, 65536] {
            let size = format!("q{bits}_n{n}");
            if !["opt_prime_dot", "opt_prime_linear"]
                .iter()
                .any(|f| case_requested(f, &size))
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
            let mut cases = vec![];
            macro_rules! dot {
                ($k:literal,$name:literal) => {{
                    assert_eq!(
                        product_sum::<$k, false>(q, &a, &b).reduce_raw(&red),
                        product_sum::<1, false>(q, &a, &b).reduce_raw(&red)
                    );
                    cases.push(Case::new($name, n * 32, || {
                        black_box(
                            product_sum::<$k, false>(q, black_box(&a), black_box(&b))
                                .reduce_raw(black_box(&red)),
                        );
                    }));
                }};
            }
            dot!(1, "one_acc");
            dot!(4, "acc4");
            cases.push(Case::new("length_dispatch", n * 32, || {
                let a = black_box(&a);
                let b = black_box(&b);
                let acc = if a.len() < 64 {
                    product_sum::<1, false>(q, a, b)
                } else {
                    product_sum::<4, false>(q, a, b)
                };
                black_box(acc.reduce_raw(black_box(&red)));
            }));
            measure("opt_prime_dot", &size, &mut cases, samples, rng);
            let mut cases = vec![];
            macro_rules! linear {
                ($k:literal,$name:literal) => {{
                    assert_eq!(
                        linear_sum::<$k>(&a, &native).reduce_raw(&red),
                        linear_sum::<1>(&a, &native).reduce_raw(&red)
                    );
                    cases.push(Case::new($name, n * 24, || {
                        black_box(
                            linear_sum::<$k>(black_box(&a), black_box(&native))
                                .reduce_raw(black_box(&red)),
                        );
                    }));
                }};
            }
            linear!(1, "one_acc");
            linear!(4, "acc4");
            measure("opt_prime_linear", &size, &mut cases, samples, rng);
        }
    }
}

// Exponents and their bounds are public. This removes the generic fixed-window
// overhead for sparse public exponents; never dispatch on a private exponent.
fn public_pow(ctx: &Ctx, mut a: u128, mut e: u128) -> u128 {
    let mut out = ctx.one();
    while e != 0 {
        if e & 1 != 0 {
            out = ctx.mul(out, a);
        }
        e >>= 1;
        if e != 0 {
            a = ctx.mul(a, a);
        }
    }
    out
}
fn powers(samples: usize, rng: &mut Rng) {
    let q = (1u128 << 127) - 1;
    let cfg = cfg(q);
    let ctx = Ctx::new(&cfg);
    for (label, e) in [
        ("e17", 65537),
        ("e127", 0x55555555555555555555555555555555u128),
    ] {
        for n in [16, 1024] {
            let size = format!("{label}_n{n}");
            if !case_requested("opt_prime_public_pow", &size) {
                continue;
            }
            let a: Vec<_> = (0..n)
                .map(|_| {
                    ctx.native_residue_u128((rng.next() as u128 | ((rng.next() as u128) << 64)) % q)
                })
                .collect();
            let exp = crypto_primitives::crypto_bigint_uint::Uint::from_words([
                e as u64,
                (e >> 64) as u64,
            ]);
            let expected: Vec<_> = a
                .iter()
                .map(|&a| ctx.raw(&ctx.field(a).pow_bounded_exp(&exp, 128 - e.leading_zeros())))
                .collect();
            let mut cases = vec![
                pass("bounded_window", n * 32, &a, &expected, |a, o| {
                    for (&a, o) in a.iter().zip(o) {
                        *o = ctx.raw(&ctx.field(a).pow_bounded_exp(&exp, 128 - e.leading_zeros()));
                    }
                }),
                pass("public_binary", n * 32, &a, &expected, |a, o| {
                    for (&a, o) in a.iter().zip(o) {
                        *o = public_pow(&ctx, a, e);
                    }
                }),
            ];
            measure("opt_prime_public_pow", &size, &mut cases, samples, rng);
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    mac(samples, rng);
    inversions(samples, rng);
    powers(samples, rng);
}
