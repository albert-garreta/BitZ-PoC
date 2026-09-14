//! Costs of existing binary arithmetic with no proposed owned replacement.
//! Oracles run before timing; inverse batches contain only nonzero elements.
use crate::{Case, Rng, arithmetic, case_requested, measure};
use f2z::{BinaryFieldB127 as B127, BinaryFieldGF128 as Gf, utils::wide_mul::WideMulAcc};
use flock_core::field::{F8, F128};
use std::hint::black_box;

const B127_MASK: u128 = (1u128 << 127) - 1;

// Test-only bit-serial quotient-ring arithmetic, independent of production
// carryless multiplication and reduction. These oracles are variable-time.
fn gf8_mul_reference(mut a: u8, mut b: u8) -> u8 {
    let mut product = 0;
    for _ in 0..8 {
        if b & 1 != 0 {
            product ^= a;
        }
        let top = a >> 7;
        a = (a << 1) ^ (0x1b * top);
        b >>= 1;
    }
    product
}

fn b127_mul_reference(mut a: u128, mut b: u128) -> u128 {
    let mut product = 0;
    for _ in 0..127 {
        if b & 1 != 0 {
            product ^= a;
        }
        let top = a >> 126;
        a = ((a << 1) & B127_MASK) ^ (3 * top);
        b >>= 1;
    }
    product
}

fn b127_inverse_reference(mut a: u128) -> u128 {
    let mut exponent = B127_MASK - 1;
    let mut product = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            product = b127_mul_reference(product, a);
        }
        a = b127_mul_reference(a, a);
        exponent >>= 1;
    }
    product
}

fn b127_from(value: u128) -> B127 {
    B127::from_words([value as u64, (value >> 64) as u64])
}

fn b127_raw(value: B127) -> u128 {
    value.words()[0] as u128 | (value.words()[1] as u128) << 64
}

fn verify_edges() {
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            assert_eq!((F8(a) + F8(b)).0, a ^ b);
            assert_eq!((F8(a) * F8(b)).0, gf8_mul_reference(a, b));
        }
        if a != 0 {
            assert_eq!(gf8_mul_reference(a, F8(a).inv().0), 1);
        }
    }
    let edges = [
        0,
        1,
        2,
        3,
        u64::MAX as u128,
        1u128 << 64,
        1u128 << 126,
        B127_MASK,
    ];
    for a in edges {
        for b in edges {
            assert_eq!(b127_raw(b127_from(a) + b127_from(b)), a ^ b);
            assert_eq!(
                b127_raw(b127_from(a) * b127_from(b)),
                b127_mul_reference(a, b)
            );
        }
        assert_eq!(b127_raw(b127_from(a).square()), b127_mul_reference(a, a));
        if a != 0 {
            assert_eq!(b127_raw(b127_from(a).inverse()), b127_inverse_reference(a));
        }
    }
}

#[inline(never)]
fn gf8_binary<const MUL: bool>(a: &[F8], b: &[F8], output: &mut [F8]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), output.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(output) {
        *out = if MUL { a * b } else { a + b };
    }
}

#[inline(never)]
fn gf8_inverse(a: &[F8], output: &mut [F8]) {
    assert_eq!(a.len(), output.len());
    for (&a, out) in a.iter().zip(output) {
        *out = a.inv();
    }
}

#[inline(never)]
fn b127_binary<const MUL: bool>(a: &[B127], b: &[B127], output: &mut [B127]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), output.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(output) {
        *out = if MUL { a * b } else { a + b };
    }
}

#[inline(never)]
fn b127_unary<const INVERSE: bool>(a: &[B127], output: &mut [B127]) {
    assert_eq!(a.len(), output.len());
    for (&a, out) in a.iter().zip(output) {
        *out = if INVERSE { a.inverse() } else { a.square() };
    }
}

fn scalar_batches(samples: usize, rng: &mut Rng) {
    for n in [16, 1024] {
        let size = n.to_string();
        if ["gf8_add", "gf8_mul", "gf8_inverse"]
            .iter()
            .any(|f| case_requested(f, &size))
        {
            let a: Vec<_> = (0..n).map(|_| F8(rng.next() as u8)).collect();
            let b: Vec<_> = (0..n).map(|_| F8(rng.next() as u8)).collect();
            let nonzero: Vec<_> = a
                .iter()
                .map(|&a| if a.is_zero() { F8::ONE } else { a })
                .collect();
            let mut output = vec![F8::ZERO; n];
            gf8_binary::<false>(&a, &b, &mut output);
            for ((a, b), out) in a.iter().zip(&b).zip(&output) {
                assert_eq!(out.0, a.0 ^ b.0);
            }
            gf8_binary::<true>(&a, &b, &mut output);
            for ((a, b), out) in a.iter().zip(&b).zip(&output) {
                assert_eq!(out.0, gf8_mul_reference(a.0, b.0));
            }
            gf8_inverse(&nonzero, &mut output);
            for (a, out) in nonzero.iter().zip(&output) {
                assert_eq!(gf8_mul_reference(a.0, out.0), 1);
            }
            macro_rules! binary {
                ($family:literal, $mul:literal) => {
                    measure(
                        $family,
                        &size,
                        &mut [Case::new("flock", n * 3, || {
                            gf8_binary::<$mul>(
                                black_box(&a),
                                black_box(&b),
                                black_box(&mut output),
                            );
                            black_box(&output);
                        })],
                        samples,
                        rng,
                    );
                };
            }
            binary!("gf8_add", false);
            binary!("gf8_mul", true);
            measure(
                "gf8_inverse",
                &size,
                &mut [Case::new("flock", n * 2, || {
                    gf8_inverse(black_box(&nonzero), black_box(&mut output));
                    black_box(&output);
                })],
                samples,
                rng,
            );
        }
        if ["b127_add", "b127_mul", "b127_square", "b127_inverse"]
            .iter()
            .any(|f| case_requested(f, &size))
        {
            let mut values = || {
                (0..n)
                    .map(|_| {
                        b127_from((rng.next() as u128 | (rng.next() as u128) << 64) & B127_MASK)
                    })
                    .collect::<Vec<_>>()
            };
            let a = values();
            let b = values();
            let nonzero: Vec<_> = a
                .iter()
                .map(|&a| if a.is_zero() { B127::one() } else { a })
                .collect();
            let mut output = vec![B127::zero(); n];
            b127_binary::<false>(&a, &b, &mut output);
            for ((a, b), out) in a.iter().zip(&b).zip(&output) {
                assert_eq!(b127_raw(*out), b127_raw(*a) ^ b127_raw(*b));
            }
            b127_binary::<true>(&a, &b, &mut output);
            for ((a, b), out) in a.iter().zip(&b).zip(&output) {
                assert_eq!(
                    b127_raw(*out),
                    b127_mul_reference(b127_raw(*a), b127_raw(*b))
                );
            }
            b127_unary::<false>(&a, &mut output);
            for (a, out) in a.iter().zip(&output) {
                assert_eq!(
                    b127_raw(*out),
                    b127_mul_reference(b127_raw(*a), b127_raw(*a))
                );
            }
            b127_unary::<true>(&nonzero, &mut output);
            for (a, out) in nonzero.iter().zip(&output) {
                assert_eq!(b127_raw(*out), b127_inverse_reference(b127_raw(*a)));
            }
            macro_rules! binary {
                ($family:literal, $mul:literal) => {
                    measure(
                        $family,
                        &size,
                        &mut [Case::new("f2z", n * 48, || {
                            b127_binary::<$mul>(
                                black_box(&a),
                                black_box(&b),
                                black_box(&mut output),
                            );
                            black_box(&output);
                        })],
                        samples,
                        rng,
                    );
                };
            }
            binary!("b127_add", false);
            binary!("b127_mul", true);
            measure(
                "b127_square",
                &size,
                &mut [Case::new("f2z", n * 32, || {
                    b127_unary::<false>(black_box(&a), black_box(&mut output));
                    black_box(&output);
                })],
                samples,
                rng,
            );
            measure(
                "b127_inverse",
                &size,
                &mut [Case::new("f2z", n * 32, || {
                    b127_unary::<true>(black_box(&nonzero), black_box(&mut output));
                    black_box(&output);
                })],
                samples,
                rng,
            );
        }
    }
}

fn gf128_mul_reference(a: Gf, b: Gf) -> Gf {
    let [a0, a1] = *a.words();
    let [b0, b1] = *b.words();
    let product = arithmetic::oracle(F128::new(a0, a1), F128::new(b0, b1));
    Gf::from_words([product.lo, product.hi])
}

fn round_reference(l: &[Gf], r: &[Gf], w: &[Gf]) -> (Gf, Gf, Gf) {
    let (mut c0, mut c1, mut c2) = (Gf::zero(), Gf::zero(), Gf::zero());
    for (i, &w) in w.iter().enumerate() {
        let l0 = gf128_mul_reference(w, l[2 * i]);
        let l1 = gf128_mul_reference(w, l[2 * i + 1]);
        let v0 = gf128_mul_reference(l0, r[2 * i]);
        let v2 = gf128_mul_reference(l0 + l1, r[2 * i] + r[2 * i + 1]);
        c0 += v0;
        c1 += gf128_mul_reference(l1, r[2 * i + 1]) + v0 + v2;
        c2 += v2;
    }
    (c0, c1, c2)
}

fn rounds(samples: usize, rng: &mut Rng) {
    // The selected public hook has an implementation returning Some on both
    // aarch64 and the portable path. A future loss of that hook is an error.
    for n in [0, 1, 3, 16, 17, 1024] {
        if [16, 1024].contains(&n) && !case_requested("gf128_round", &n.to_string()) {
            continue;
        }
        let l: Vec<_> = rng
            .values(n * 2)
            .into_iter()
            .map(|v| Gf::from_words([v.lo, v.hi]))
            .collect();
        let r: Vec<_> = rng
            .values(n * 2)
            .into_iter()
            .map(|v| Gf::from_words([v.lo, v.hi]))
            .collect();
        let w: Vec<_> = rng
            .values(n)
            .into_iter()
            .map(|v| Gf::from_words([v.lo, v.hi]))
            .collect();
        let expected = round_reference(&l, &r, &w);
        assert_eq!(
            Gf::eqf_single_pair_round(&l, &r, &w, n).expect("missing F2Z round hook"),
            expected
        );
        if [16, 1024].contains(&n) {
            measure(
                "gf128_round",
                &n.to_string(),
                &mut [Case::new("f2z_single_pair", n * 80, || {
                    black_box(
                        Gf::eqf_single_pair_round(black_box(&l), black_box(&r), black_box(&w), n)
                            .expect("missing F2Z round hook"),
                    );
                })],
                samples,
                rng,
            );
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    verify_edges();
    scalar_batches(samples, rng);
    rounds(samples, rng);
}
