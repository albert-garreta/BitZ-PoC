use crate::baseline_delayed::{
    CryptoBigintMonty128Reducer as Reference, MontyLinearAccumulator128 as Linear,
    MontyProductAccumulator128 as Product, OptimizedMonty128Reducer as Reducer, Reduce,
};
use crate::{Case, Rng, baseline_raw::RawMontyCtx as Ctx, case_requested, measure};
use crypto_bigint::{Odd, Uint, modular::FixedMontyParams};
use crypto_primitives::crypto_bigint_monty::MontyField;
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use std::{hint::black_box, marker::PhantomData};

type Config = FixedMontyParams<2>;
const MODULI: [(u32, u128); 2] = [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)];
fn config(q: u128) -> Config {
    Config::new_vartime(Odd::new(Uint::from_words([q as u64, (q >> 64) as u64])).unwrap())
}
fn random(rng: &mut Rng) -> u128 {
    rng.next() as u128 | (rng.next() as u128) << 64
}

// A transparent, invariant brand models the proposed scalar representation. This
// experiment has no cross-context constructor/API; it tests codegen only.
#[repr(transparent)]
#[derive(Clone, Copy)]
struct Branded<'f> {
    raw: u128,
    brand: PhantomData<fn(&'f mut ()) -> &'f mut ()>,
}
impl Branded<'_> {
    fn new(raw: u128) -> Self {
        Self {
            raw,
            brand: PhantomData,
        }
    }
}

#[inline(never)]
fn products<const CHECK: bool>(ctx: &Ctx, q: u128, a: &[u128], b: &[u128], out: &mut [u128]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        if CHECK {
            assert!(a < q && b < q);
        }
        *out = ctx.mul(a, b);
    }
}
#[inline(never)]
fn branded(ctx: &Ctx, a: &[Branded<'_>], b: &[Branded<'_>], out: &mut [Branded<'_>]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        out.raw = ctx.mul(a.raw, b.raw);
    }
}
#[inline(never)]
fn configured(a: &[MontyField<2>], b: &[MontyField<2>], out: &mut [MontyField<2>]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        *out = a * b;
    }
}
#[inline(never)]
pub(super) fn product_sum<const K: usize, const CHECK: bool>(
    q: u128,
    a: &[u128],
    b: &[u128],
) -> Product {
    assert_eq!(a.len(), b.len());
    // On a 64-bit host the slice length establishes the five-limb capacity:
    // fewer than 2^64 terms, each strictly below 2^256.
    let mut acc = [Product::zero(); K];
    for (aa, bb) in a.chunks(K).zip(b.chunks(K)) {
        for (j, (&a, &b)) in aa.iter().zip(bb).enumerate() {
            if CHECK {
                assert!(a < q && b < q);
            }
            acc[j].multiply_accumulate_raw(a, b);
        }
    }
    acc.into_iter().fold(Product::zero(), |a, b| a + b)
}
#[inline(never)]
pub(super) fn linear_sum<const K: usize>(a: &[u128], b: &[u64]) -> Linear {
    assert_eq!(a.len(), b.len());
    let mut acc = [Linear::zero(); K];
    for (aa, bb) in a.chunks(K).zip(b.chunks(K)) {
        for (j, (&a, &b)) in aa.iter().zip(bb).enumerate() {
            acc[j].multiply_accumulate_raw(a, b);
        }
    }
    acc.into_iter().fold(Linear::zero(), |a, b| a + b)
}
#[inline(never)]
fn eager(ctx: &Ctx, a: &[u128], b: &[u128]) -> u128 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .fold(0, |sum, (&a, &b)| ctx.add(sum, ctx.mul(a, b)))
}
fn verify(rng: &mut Rng) {
    for (_, q) in MODULI
        .into_iter()
        .chain([(65, (1u128 << 64) + 13), (127, (1u128 << 127) - 1)])
    {
        let cfg = config(q);
        let ctx = Ctx::new(&cfg);
        let red = Reducer::new(&cfg).unwrap();
        let reference = Reference::new(&cfg).unwrap();
        let edge = [0, 1, 2, q - 1, q - 2, (1u128 << 64) - 1];
        for a in edge.into_iter().chain((0..128).map(|_| random(rng) % q)) {
            for b in edge {
                let ra = ctx.native_residue_u128(a);
                let rb = ctx.native_residue_u128(b);
                let expected = (BigUint::from(a) * BigUint::from(b) % BigUint::from(q))
                    .to_u128()
                    .unwrap();
                assert_eq!(
                    ctx.field(ctx.mul(ra, rb)).retrieve().to_words(),
                    [expected as u64, (expected >> 64) as u64]
                );
            }
        }
        for n in [0, 1, 2, 3, 7, 8, 9, 17, 1025] {
            let a: Vec<_> = (0..n).map(|_| random(rng) % q).collect();
            let b: Vec<_> = (0..n).map(|_| random(rng) % q).collect();
            let native: Vec<_> = (0..n).map(|_| rng.next()).collect();
            let expected = eager(&ctx, &a, &b);
            macro_rules! check {
                ($k:literal) => {
                    assert_eq!(
                        product_sum::<$k, false>(q, &a, &b).reduce_raw(&red),
                        expected
                    );
                };
            }
            check!(1);
            check!(2);
            check!(4);
            check!(8);
            let ref_field: MontyField<2> = product_sum::<1, false>(q, &a, &b)
                .reduce(&reference)
                .unwrap();
            assert_eq!(ctx.raw(&ref_field), expected);
            let plain_native: Vec<_> = native.iter().map(|&n| ctx.native_residue(n)).collect();
            let expected = eager(&ctx, &a, &plain_native);
            macro_rules! check_linear {
                ($k:literal) => {
                    assert_eq!(linear_sum::<$k>(&a, &native).reduce_raw(&red), expected);
                };
            }
            check_linear!(1);
            check_linear!(2);
            check_linear!(4);
            check_linear!(8);
        }
        // Maximum operands exercise accumulation carries, not just low random words.
        let a = vec![q - 1; 1025];
        let b = vec![u64::MAX; 1025];
        let expected = (BigUint::from(q - 1) * BigUint::from(u64::MAX) * BigUint::from(1025u32)
            % BigUint::from(q))
        .to_u128()
        .unwrap();
        assert_eq!(linear_sum::<4>(&a, &b).reduce_raw(&red), expected);
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    verify(rng);
    extra(samples, rng);
    for (bits, q) in MODULI {
        let cfg = config(q);
        let ctx = Ctx::new(&cfg);
        let red = Reducer::new(&cfg).unwrap();
        let reference = Reference::new(&cfg).unwrap();
        let acc = product_sum::<1, false>(q, &[q - 1; 1024], &[q - 1; 1024]);
        let ref_product: MontyField<2> = acc.reduce(&reference).unwrap();
        assert_eq!(acc.reduce_raw(&red), ctx.raw(&ref_product));
        let mut reduce = vec![
            Case::new("existing_optimized", 40, || {
                black_box(black_box(acc).reduce_raw(black_box(&red)));
            }),
            Case::new("crypto_bigint", 40, || {
                let x: MontyField<2> = black_box(acc).reduce(black_box(&reference)).unwrap();
                black_box(ctx.raw(&x));
            }),
        ];
        measure(
            "prime_reduce_product",
            &bits.to_string(),
            &mut reduce,
            samples,
            rng,
        );
        let linear = linear_sum::<1>(&[q - 1; 1024], &[u64::MAX; 1024]);
        let ref_linear: MontyField<2> = linear.reduce(&reference).unwrap();
        assert_eq!(linear.reduce_raw(&red), ctx.raw(&ref_linear));
        let mut reduce = vec![
            Case::new("existing_optimized", 40, || {
                black_box(black_box(linear).reduce_raw(black_box(&red)));
            }),
            Case::new("crypto_bigint", 40, || {
                let x: MontyField<2> = black_box(linear).reduce(black_box(&reference)).unwrap();
                black_box(ctx.raw(&x));
            }),
        ];
        measure(
            "prime_reduce_linear",
            &bits.to_string(),
            &mut reduce,
            samples,
            rng,
        );
        let mut setup = vec![Case::new(
            "existing_contexts",
            std::mem::size_of::<Config>(),
            || {
                let cfg = config(black_box(q));
                black_box(Ctx::new(&cfg));
                black_box(Reducer::new(&cfg).unwrap());
            },
        )];
        measure("prime_setup", &bits.to_string(), &mut setup, samples, rng);
        for n in [16, 1024, 65536, 1048576] {
            let size = format!("q{bits}_n{n}");
            if !["prime_mul", "prime_dot", "prime_linear"]
                .iter()
                .any(|family| case_requested(family, &size))
            {
                continue;
            }
            let a: Vec<_> = (0..n).map(|_| random(rng) % q).collect();
            let b: Vec<_> = (0..n).map(|_| random(rng) % q).collect();
            if case_requested("prime_mul", &size) {
                let mut expected = vec![0; n];
                products::<false>(&ctx, q, &a, &b, &mut expected);
                let ba: Vec<_> = a.iter().copied().map(Branded::new).collect();
                let bb: Vec<_> = b.iter().copied().map(Branded::new).collect();
                let mut bo = vec![Branded::new(0); n];
                branded(&ctx, &ba, &bb, &mut bo);
                assert!(bo.iter().zip(&expected).all(|(x, &y)| x.raw == y));
                let ca: Vec<_> = a.iter().map(|&v| ctx.field(v)).collect();
                let cb: Vec<_> = b.iter().map(|&v| ctx.field(v)).collect();
                let mut co = vec![ctx.field(0); n];
                configured(&ca, &cb, &mut co);
                assert!(co.iter().zip(&expected).all(|(x, &y)| ctx.raw(x) == y));
                let mut checked = vec![0; n];
                products::<true>(&ctx, q, &a, &b, &mut checked);
                assert_eq!(checked, expected);
                let mut out = vec![0; n];
                let mut cases = vec![
                    Case::new("raw_ctx", n * 48, || {
                        products::<false>(
                            black_box(&ctx),
                            q,
                            black_box(&a),
                            black_box(&b),
                            black_box(&mut out),
                        );
                        black_box(&out);
                    }),
                    Case::new("branded", n * 48, || {
                        branded(
                            black_box(&ctx),
                            black_box(&ba),
                            black_box(&bb),
                            black_box(&mut bo),
                        );
                        black_box(&bo);
                    }),
                    Case::new("checked_per_term", n * 48, || {
                        products::<true>(
                            black_box(&ctx),
                            black_box(q),
                            black_box(&a),
                            black_box(&b),
                            black_box(&mut checked),
                        );
                        black_box(&checked);
                    }),
                    Case::new(
                        "configured_field",
                        n * std::mem::size_of::<MontyField<2>>() * 3,
                        || {
                            configured(black_box(&ca), black_box(&cb), black_box(&mut co));
                            black_box(&co);
                        },
                    ),
                ];
                measure("prime_mul", &size, &mut cases, samples, rng);
            }
            if case_requested("prime_dot", &size) {
                let expected = eager(&ctx, &a, &b);
                let mut cases = Vec::new();
                macro_rules! dot {
                    ($k:literal,$name:literal,$check:literal) => {{
                        assert_eq!(
                            product_sum::<$k, $check>(q, &a, &b).reduce_raw(&red),
                            expected
                        );
                        cases.push(Case::new($name, n * 32, || {
                            black_box(
                                product_sum::<$k, $check>(
                                    black_box(q),
                                    black_box(&a),
                                    black_box(&b),
                                )
                                .reduce_raw(black_box(&red)),
                            );
                        }));
                    }};
                }
                dot!(1, "existing_delayed", false);
                dot!(2, "acc2", false);
                dot!(4, "acc4", false);
                dot!(8, "acc8", false);
                dot!(1, "checked_per_term", true);
                cases.push(Case::new("eager_raw", n * 32, || {
                    black_box(eager(black_box(&ctx), black_box(&a), black_box(&b)));
                }));
                let ref_field: MontyField<2> = product_sum::<1, false>(q, &a, &b)
                    .reduce(&reference)
                    .unwrap();
                assert_eq!(ctx.raw(&ref_field), expected);
                cases.push(Case::new("crypto_bigint_reduce", n * 32, || {
                    let x: MontyField<2> = product_sum::<1, false>(q, black_box(&a), black_box(&b))
                        .reduce(black_box(&reference))
                        .unwrap();
                    black_box(ctx.raw(&x));
                }));
                // Materialization is measured in this case, not hidden in setup.
                cases.push(Case::new("convert_then_delayed", n * 64, || {
                    let aa = black_box(&a).clone();
                    let bb = black_box(&b).clone();
                    black_box(product_sum::<1, false>(q, &aa, &bb).reduce_raw(black_box(&red)));
                }));
                // Inputs already have the raw representation. Borrowing keeps
                // conversion out of both allocation and traversal costs.
                cases.push(Case::new("borrowed_delayed", n * 32, || {
                    black_box(
                        product_sum::<2, false>(q, black_box(&a), black_box(&b))
                            .reduce_raw(black_box(&red)),
                    );
                }));
                measure("prime_dot", &size, &mut cases, samples, rng);
            }
            if case_requested("prime_linear", &size) {
                let b: Vec<_> = (0..n).map(|_| rng.next()).collect();
                let expected = (a.iter().zip(&b).fold(BigUint::zero(), |sum, (&a, &b)| {
                    sum + BigUint::from(a) * BigUint::from(b)
                }) % BigUint::from(q))
                .to_u128()
                .unwrap();
                let mut cases = Vec::new();
                macro_rules! dot {
                    ($k:literal,$name:literal) => {{
                        assert_eq!(linear_sum::<$k>(&a, &b).reduce_raw(&red), expected);
                        cases.push(Case::new($name, n * 24, || {
                            black_box(
                                linear_sum::<$k>(black_box(&a), black_box(&b))
                                    .reduce_raw(black_box(&red)),
                            );
                        }));
                    }};
                }
                dot!(1, "existing_delayed");
                dot!(2, "acc2");
                dot!(4, "acc4");
                dot!(8, "acc8");
                let ref_field: MontyField<2> = linear_sum::<1>(&a, &b).reduce(&reference).unwrap();
                assert_eq!(ctx.raw(&ref_field), expected);
                cases.push(Case::new("crypto_bigint_reduce", n * 24, || {
                    let x: MontyField<2> = linear_sum::<1>(black_box(&a), black_box(&b))
                        .reduce(black_box(&reference))
                        .unwrap();
                    black_box(ctx.raw(&x));
                }));
                measure("prime_linear", &size, &mut cases, samples, rng);
            }
        }
    }
}

#[inline(never)]
fn bit_sum<const K: usize>(ctx: &Ctx, a: &[u128], bits: &[bool]) -> Linear {
    use crate::baseline_delayed::Accumulatable;
    let mut acc = [Linear::zero(); K];
    for (aa, bb) in a.chunks(K).zip(bits.chunks(K)) {
        for j in 0..aa.len() {
            acc[j].multiply_accumulate(&ctx.field(aa[j]), &bb[j]);
        }
    }
    acc.into_iter().fold(Linear::zero(), |a, b| a + b)
}
fn extra(samples: usize, rng: &mut Rng) {
    for (bits, q) in MODULI {
        let cfg = config(q);
        let ctx = Ctx::new(&cfg);
        let red = Reducer::new(&cfg).unwrap();
        let reference = Reference::new(&cfg).unwrap();
        for n in [1, 3, 7, 16, 17, 1024, 65536, 1048576] {
            let size = format!("q{bits}_n{n}");
            if ![
                "prime_add",
                "prime_sub",
                "prime_chain",
                "prime_bits",
                "prime_fold",
            ]
            .iter()
            .any(|f| case_requested(f, &size))
            {
                continue;
            }
            let a: Vec<_> = (0..2 * n).map(|_| random(rng) % q).collect();
            let b: Vec<_> = (0..n).map(|_| random(rng) % q).collect();
            for family in ["prime_add", "prime_sub"] {
                if !case_requested(family, &size) {
                    continue;
                }
                let mut out = vec![0; n];
                let mut configured_out = vec![ctx.field(0); n];
                let fa: Vec<_> = a[..n].iter().map(|&a| ctx.field(a)).collect();
                let fb: Vec<_> = b.iter().map(|&a| ctx.field(a)).collect();
                for ((&a, &b), o) in a.iter().zip(&b).zip(&mut out) {
                    *o = if family == "prime_add" {
                        ctx.add(a, b)
                    } else {
                        ctx.sub(a, b)
                    };
                    let aa = BigUint::from(a);
                    let bb = BigUint::from(b);
                    let qq = BigUint::from(q);
                    let want = if family == "prime_add" {
                        (aa + bb) % &qq
                    } else {
                        (aa + &qq - bb) % &qq
                    };
                    assert_eq!(*o, want.to_u128().unwrap());
                }
                for ((a, b), o) in fa.iter().zip(&fb).zip(&mut configured_out) {
                    *o = if family == "prime_add" { a + b } else { a - b };
                }
                assert!(
                    configured_out
                        .iter()
                        .zip(&out)
                        .all(|(a, &b)| ctx.raw(a) == b)
                );
                let mut cases = vec![
                    Case::new("raw_ctx", n * 48, || {
                        if family == "prime_add" {
                            binary(&a[..n], &b, &mut out, |&a, &b| ctx.add(a, b));
                        } else {
                            binary(&a[..n], &b, &mut out, |&a, &b| ctx.sub(a, b));
                        }
                        black_box(&out);
                    }),
                    Case::new(
                        "configured_field",
                        n * std::mem::size_of::<MontyField<2>>() * 3,
                        || {
                            if family == "prime_add" {
                                binary(&fa, &fb, &mut configured_out, |a, b| a + b);
                            } else {
                                binary(&fa, &fb, &mut configured_out, |a, b| a - b);
                            }
                            black_box(&configured_out);
                        },
                    ),
                ];
                measure(family, &size, &mut cases, samples, rng);
            }
            if case_requested("prime_chain", &size) {
                let fa: Vec<_> = a[..n].iter().map(|&a| ctx.field(a)).collect();
                let raw = || {
                    black_box(&a[..n])
                        .iter()
                        .fold(ctx.one(), |s, &a| ctx.mul(s, a))
                };
                let configured = || {
                    black_box(&fa)
                        .iter()
                        .fold(ctx.field(ctx.one()), |s, a| s * a)
                };
                assert_eq!(raw(), ctx.raw(&configured()));
                let mut cases = vec![
                    Case::new("raw_ctx", n * 16, || {
                        black_box(raw());
                    }),
                    Case::new(
                        "configured_field",
                        n * std::mem::size_of::<MontyField<2>>(),
                        || {
                            black_box(configured());
                        },
                    ),
                ];
                measure("prime_chain", &size, &mut cases, samples, rng);
            }
            if case_requested("prime_bits", &size) {
                let b: Vec<_> = (0..n).map(|_| rng.next() & 1 != 0).collect();
                let expected = (a
                    .iter()
                    .zip(&b)
                    .filter(|(_, b)| **b)
                    .fold(BigUint::zero(), |s, (&a, _)| s + BigUint::from(a))
                    % BigUint::from(q))
                .to_u128()
                .unwrap();
                let mut cases = Vec::new();
                macro_rules! v {
                    ($name:literal,$k:literal) => {{
                        assert_eq!(bit_sum::<$k>(&ctx, &a[..n], &b).reduce_raw(&red), expected);
                        cases.push(Case::new($name, n * 17, || {
                            black_box(
                                bit_sum::<$k>(black_box(&ctx), black_box(&a[..n]), black_box(&b))
                                    .reduce_raw(black_box(&red)),
                            );
                        }));
                    }};
                }
                v!("existing_bits", 1);
                v!("acc2", 2);
                v!("acc4", 4);
                v!("acc8", 8);
                measure("prime_bits", &size, &mut cases, samples, rng);
            }
            if case_requested("prime_fold", &size) {
                let rho = b[0];
                let mut cases = Vec::new();
                let mut expected = Vec::new();
                for name in ["raw_ctx", "unroll4"] {
                    let mut out = vec![0; n];
                    let a = &a;
                    let ctx = &ctx;
                    let apply = move |out: &mut [u128]| {
                        let u = if name == "raw_ctx" { 1 } else { 4 };
                        for (aa, oo) in black_box(a).chunks(2 * u).zip(out.chunks_mut(u)) {
                            for i in 0..oo.len() {
                                oo[i] = ctx.interpolate(aa[2 * i], aa[2 * i + 1], black_box(rho));
                            }
                        }
                    };
                    apply(&mut out);
                    if name == "raw_ctx" {
                        expected = out.clone();
                    } else {
                        assert_eq!(out, expected);
                    }
                    cases.push(Case::new(name, n * 48, move || {
                        apply(black_box(&mut out));
                        black_box(&out);
                    }));
                }
                measure("prime_fold", &size, &mut cases, samples, rng);
            }
        }
        for terms in [1, 1024, 1048576] {
            for pattern in ["uniform", "carry"] {
                let size = format!("q{bits}_{pattern}_n{terms}");
                if !["prime_reduce_product_inputs", "prime_reduce_linear_inputs"]
                    .iter()
                    .any(|f| case_requested(f, &size))
                {
                    continue;
                }
                let mut product = Product::zero();
                let mut linear = Linear::zero();
                let mut ps = BigUint::zero();
                let mut ls = BigUint::zero();
                for _ in 0..terms {
                    let a = if pattern == "carry" {
                        q - 1
                    } else {
                        random(rng) % q
                    };
                    let b = if pattern == "carry" {
                        q - 1
                    } else {
                        random(rng) % q
                    };
                    let c = if pattern == "carry" {
                        u64::MAX
                    } else {
                        rng.next()
                    };
                    product.multiply_accumulate_raw(a, b);
                    linear.multiply_accumulate_raw(a, c);
                    ps += BigUint::from(a) * BigUint::from(b);
                    ls += BigUint::from(a) * BigUint::from(c);
                }
                let pr: MontyField<2> = product.reduce(&reference).unwrap();
                let lr: MontyField<2> = linear.reduce(&reference).unwrap();
                assert_eq!(product.reduce_raw(&red), ctx.raw(&pr));
                assert_eq!(linear.reduce_raw(&red), ctx.raw(&lr));
                assert_eq!(
                    linear.reduce_raw(&red),
                    (ls % BigUint::from(q)).to_u128().unwrap()
                );
                // Multiplying the output by R recovers the unreduced product sum mod q.
                assert_eq!(
                    (BigUint::from(product.reduce_raw(&red)) * (BigUint::from(1u8) << 128usize))
                        % BigUint::from(q),
                    ps % BigUint::from(q)
                );
                let mut pc = vec![
                    Case::new("existing_optimized", 40, || {
                        black_box(black_box(product).reduce_raw(black_box(&red)));
                    }),
                    Case::new("crypto_bigint", 40, || {
                        let x: MontyField<2> =
                            black_box(product).reduce(black_box(&reference)).unwrap();
                        black_box(ctx.raw(&x));
                    }),
                ];
                measure("prime_reduce_product_inputs", &size, &mut pc, samples, rng);
                let mut lc = vec![
                    Case::new("existing_optimized", 40, || {
                        black_box(black_box(linear).reduce_raw(black_box(&red)));
                    }),
                    Case::new("crypto_bigint", 40, || {
                        let x: MontyField<2> =
                            black_box(linear).reduce(black_box(&reference)).unwrap();
                        black_box(ctx.raw(&x));
                    }),
                ];
                measure("prime_reduce_linear_inputs", &size, &mut lc, samples, rng);
            }
        }
    }
}

#[inline(never)]
fn binary<T>(a: &[T], b: &[T], out: &mut [T], op: impl Fn(&T, &T) -> T) {
    for ((a, b), o) in black_box(a).iter().zip(black_box(b)).zip(black_box(out)) {
        *o = op(a, b);
    }
}
