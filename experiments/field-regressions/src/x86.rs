//! Host-qualified benchmark adapters. Production dispatch is never changed.
use crate::{
    Case, Rng,
    arithmetic::{Prepared, oracle},
    case_requested, measure, variant_requested,
};
use f2z::{poly::univariate::binary_gf128::BinaryFieldGF128 as Gf, utils::wide_mul::WideMulAcc};
use flock_core::field::{
    F128,
    gf2_128::{F256Unreduced, x86_64 as hw},
};
use std::hint::black_box;
mod consumers;
mod rounds;

pub fn gf(a: F128) -> Gf {
    Gf::from_words([a.lo, a.hi])
}
pub fn flock(a: Gf) -> F128 {
    let w = a.words();
    F128::new(w[0], w[1])
}

#[inline(always)]
pub fn scalar<const K: usize>(a: F128, b: F128) -> F128 {
    // All entry points compile only with PCLMUL and SSE4.1 enabled.
    unsafe {
        match K {
            0 => a * b,
            1 => hw::ghash_mul_schoolbook(a, b),
            2 => hw::ghash_mul_karatsuba(a, b),
            3 => hw::ghash_mul_karatsuba_barrett(a, b),
            4 => hw::ghash_mul_binius(a, b),
            5 => flock(gf(a) * gf(b)),
            _ => unreachable!(),
        }
    }
}
#[inline(never)]
fn products<const K: usize, const U: usize>(a: &[F128], b: &[F128], out: &mut [F128]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let end = a.len() / U * U;
    for ((aa, bb), oo) in a[..end]
        .chunks_exact(U)
        .zip(b[..end].chunks_exact(U))
        .zip(out[..end].chunks_exact_mut(U))
    {
        for i in 0..U {
            oo[i] = scalar::<K>(aa[i], bb[i]);
        }
    }
    for ((&a, &b), o) in a[end..].iter().zip(&b[end..]).zip(&mut out[end..]) {
        *o = scalar::<K>(a, b);
    }
}
#[inline(never)]
fn chain<const K: usize>(a: &[F128]) -> F128 {
    a.iter().fold(F128::ONE, |z, &a| scalar::<K>(z, a))
}
#[inline(never)]
fn wide_dot<const U: usize>(a: &[F128], b: &[F128]) -> F128 {
    assert_eq!(a.len(), b.len());
    let mut sums = [F256Unreduced::ZERO; U];
    for (aa, bb) in a.chunks(U).zip(b.chunks(U)) {
        for i in 0..aa.len() {
            sums[i] ^= aa[i].mul_unreduced(bb[i]);
        }
    }
    sums.into_iter()
        .fold(F256Unreduced::ZERO, |a, b| a ^ b)
        .reduce()
}
#[inline(never)]
fn production_dot(a: &[Gf], b: &[Gf]) -> Gf {
    assert_eq!(a.len(), b.len());
    let mut sum = Gf::wide_zero(&Gf::zero());
    for (a, b) in a.iter().zip(b) {
        Gf::wide_add_assign(&mut sum, &Gf::mul_wide(a, b));
    }
    Gf::from_wide(sum)
}

#[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
#[inline(never)]
fn products_x4(a: &[F128], b: &[F128], out: &mut [F128]) {
    use core::arch::x86_64::*;
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let end = a.len() / 4 * 4;
    // SAFETY: feature gates and complete four-element chunks establish every access.
    unsafe {
        for i in (0..end).step_by(4) {
            let x = _mm512_loadu_si512(a.as_ptr().add(i).cast());
            let y = _mm512_loadu_si512(b.as_ptr().add(i).cast());
            _mm512_storeu_si512(out.as_mut_ptr().add(i).cast(), hw::ghash_mul_x4(x, y));
        }
    }
    // Keep mixed full/tail batches in SIMD registers. Only active u64 lanes
    // are accessed, so this also works when the tail ends at a page boundary.
    let remaining = a.len() - end;
    if end != 0 && remaining != 0 {
        let mask = ((1u16 << (remaining * 2)) - 1) as u8;
        // SAFETY: remaining is 1..=3 and each F128 contains two u64 lanes.
        // The mask enables exactly the initialized elements of each slice.
        unsafe {
            let x = _mm512_maskz_loadu_epi64(mask, a.as_ptr().add(end).cast());
            let y = _mm512_maskz_loadu_epi64(mask, b.as_ptr().add(end).cast());
            _mm512_mask_storeu_epi64(
                out.as_mut_ptr().add(end).cast(),
                mask,
                hw::ghash_mul_x4(x, y),
            );
        }
        return;
    }
    for ((&a, &b), out) in a[end..].iter().zip(&b[end..]).zip(&mut out[end..]) {
        *out = scalar::<0>(a, b);
    }
}
#[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
#[inline(never)]
fn dot_x4(a: &[F128], b: &[F128]) -> F128 {
    use core::arch::x86_64::*;
    assert_eq!(a.len(), b.len());
    let end = a.len() / 4 * 4;
    // SAFETY: feature gates and bounds as in products_x4; fold requires SSE4.1.
    unsafe {
        let mut sum = hw::WideGhashX4::zero();
        for i in (0..end).step_by(4) {
            sum.mul_acc(
                _mm512_loadu_si512(a.as_ptr().add(i).cast()),
                _mm512_loadu_si512(b.as_ptr().add(i).cast()),
            );
        }
        let mut sum = sum.fold();
        for (&a, &b) in a[end..].iter().zip(&b[end..]) {
            sum ^= a.mul_unreduced(b);
        }
        sum.reduce()
    }
}

fn verify(rng: &mut Rng) {
    for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 33, 1025] {
        let a = rng.values(n);
        let b = rng.values(n);
        let want: Vec<_> = a.iter().zip(&b).map(|(&a, &b)| oracle(a, b)).collect();
        let dot = want.iter().fold(F128::ZERO, |s, &a| s + a);
        let mut out = vec![F128::ZERO; n];
        macro_rules! check {
            ($k:literal) => {
                products::<$k, 4>(&a, &b, &mut out);
                assert_eq!(out, want);
            };
        }
        check!(0);
        check!(1);
        check!(2);
        check!(3);
        check!(4);
        check!(5);
        assert_eq!(wide_dot::<1>(&a, &b), dot);
        assert_eq!(wide_dot::<2>(&a, &b), dot);
        assert_eq!(wide_dot::<4>(&a, &b), dot);
        assert_eq!(wide_dot::<8>(&a, &b), dot);
        #[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
        {
            products_x4(&a, &b, &mut out);
            assert_eq!(out, want);
            // Exercise every 16-byte offset within a 64-byte SIMD block.
            // Prefix/suffix sentinels also check masked stores at short tails.
            for offset in 0..4 {
                let mut aa = vec![F128::ONE; n + 4];
                let mut bb = vec![F128::ONE; n + 4];
                let mut cc = vec![F128::ONE; n + 4];
                aa[offset..offset + n].copy_from_slice(&a);
                bb[offset..offset + n].copy_from_slice(&b);
                products_x4(
                    &aa[offset..offset + n],
                    &bb[offset..offset + n],
                    &mut cc[offset..offset + n],
                );
                assert_eq!(&cc[offset..offset + n], &want);
                assert!(
                    cc[..offset]
                        .iter()
                        .chain(&cc[offset + n..])
                        .all(|x| *x == F128::ONE)
                );
            }
            assert_eq!(dot_x4(&a, &b), dot);
        }
    }
}

fn arrays(n: usize, size: &str, samples: usize, rng: &mut Rng) {
    let families = [
        "x86_products",
        "x86_chain",
        "x86_dot",
        "x86_xor",
        "x86_wide",
        "x86_reduce",
        "x86_square",
        "x86_inverse",
    ];
    if !families.iter().any(|f| case_requested(f, size)) {
        return;
    }
    let a = rng.values(n);
    let b = rng.values(n);
    let fa: Vec<_> = a.iter().copied().map(gf).collect();
    let fb: Vec<_> = b.iter().copied().map(gf).collect();
    if case_requested("x86_products", size) {
        let mut cases = Vec::new();
        macro_rules! variant {
            ($name:literal,$k:literal,$u:literal) => {{
                if variant_requested("x86_products", size, $name) {
                    let mut out = vec![F128::ZERO; n];
                    let (a, b) = (&a, &b);
                    products::<$k, $u>(a, b, &mut out);
                    assert!(out.iter().zip(a).zip(b).all(|((&c, &a), &b)| c == a * b));
                    cases.push(Case::new($name, n * 48, move || {
                        products::<$k, $u>(black_box(a), black_box(b), black_box(&mut out));
                        black_box(&out);
                    }));
                }
            }};
        }
        variant!("flock", 0, 1);
        variant!("schoolbook", 1, 1);
        variant!("karatsuba", 2, 1);
        variant!("barrett", 3, 1);
        variant!("binius", 4, 1);
        variant!("f2z", 5, 1);
        variant!("barrett_u2", 3, 2);
        variant!("barrett_u4", 3, 4);
        variant!("barrett_u8", 3, 8);
        #[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
        if variant_requested("x86_products", size, "vpclmul4") {
            let mut out = vec![F128::ZERO; n];
            let (a, b) = (&a, &b);
            products_x4(a, b, &mut out);
            assert!(out.iter().zip(a).zip(b).all(|((&c, &a), &b)| c == a * b));
            cases.push(Case::new("vpclmul4", n * 48, move || {
                products_x4(black_box(a), black_box(b), black_box(&mut out));
                black_box(&out);
            }));
        }
        measure("x86_products", size, &mut cases, samples, rng);
    }
    if case_requested("x86_chain", size) {
        let mut cases = Vec::new();
        macro_rules! v {
            ($name:literal,$k:literal) => {
                cases.push(Case::new($name, n * 16, || {
                    black_box(chain::<$k>(black_box(&a)));
                }));
            };
        }
        v!("flock", 0);
        v!("schoolbook", 1);
        v!("karatsuba", 2);
        v!("barrett", 3);
        v!("binius", 4);
        v!("f2z", 5);
        measure("x86_chain", size, &mut cases, samples, rng);
    }
    if case_requested("x86_dot", size) {
        let expected = flock(production_dot(&fa, &fb));
        let mut cases = vec![Case::new("f2z_wide", n * 32, || {
            black_box(production_dot(black_box(&fa), black_box(&fb)));
        })];
        macro_rules! v {
            ($name:literal,$u:literal) => {{
                assert_eq!(wide_dot::<$u>(&a, &b), expected);
                cases.push(Case::new($name, n * 32, || {
                    black_box(wide_dot::<$u>(black_box(&a), black_box(&b)));
                }));
            }};
        }
        v!("wide1", 1);
        v!("wide2", 2);
        v!("wide4", 4);
        v!("wide8", 8);
        #[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
        {
            assert_eq!(dot_x4(&a, &b), expected);
            cases.push(Case::new("vpclmul4", n * 32, || {
                black_box(dot_x4(black_box(&a), black_box(&b)));
            }));
        }
        measure("x86_dot", size, &mut cases, samples, rng);
    }
    for family in ["x86_xor", "x86_square", "x86_inverse"] {
        if !case_requested(family, size) {
            continue;
        }
        let mut cases = Vec::new();
        for name in ["flock", "f2z"] {
            let mut out = vec![F128::ZERO; n];
            let (a, b) = (&a, &b);
            let apply = move |out: &mut [F128]| match (family, name) {
                ("x86_xor", "flock") => unary::<0, false>(a, b, out),
                ("x86_xor", _) => unary::<0, true>(a, b, out),
                ("x86_square", "flock") => unary::<1, false>(a, b, out),
                ("x86_square", _) => unary::<1, true>(a, b, out),
                (_, "flock") => unary::<2, false>(a, b, out),
                _ => unary::<2, true>(a, b, out),
            };
            apply(&mut out);
            assert!(out.iter().zip(a).zip(b).all(|((&c, &a), &b)| match family {
                "x86_xor" => c == a + b,
                "x86_square" => c == oracle(a, a),
                _ => oracle(c, a) == F128::ONE,
            }));
            cases.push(Case::new(
                name,
                n * if family == "x86_xor" { 48 } else { 32 },
                move || {
                    apply(black_box(&mut out));
                    black_box(&out);
                },
            ));
        }
        measure(family, size, &mut cases, samples, rng);
    }
    if case_requested("x86_wide", size) {
        let mut out = vec![[0; 4]; n];
        let mut other = vec![F256Unreduced::ZERO; n];
        let expected: Vec<_> = fa
            .iter()
            .zip(&fb)
            .map(|(a, b)| Gf::mul_wide(a, b))
            .collect();
        for ((&a, &b), want) in a.iter().zip(&b).zip(&expected) {
            let v = a.mul_unreduced(b);
            assert_eq!([v.r0, v.r1, v.r2, v.r3], *want);
        }
        let mut cases = vec![
            Case::new("f2z_wide", n * 64, || {
                for ((a, b), o) in black_box(&fa)
                    .iter()
                    .zip(black_box(&fb))
                    .zip(black_box(&mut out))
                {
                    *o = Gf::mul_wide(a, b);
                }
                black_box(&out);
            }),
            Case::new("flock_wide", n * 64, || {
                for ((&a, &b), o) in black_box(&a)
                    .iter()
                    .zip(black_box(&b))
                    .zip(black_box(&mut other))
                {
                    *o = a.mul_unreduced(b);
                }
                black_box(&other);
            }),
        ];
        measure("x86_wide", size, &mut cases, samples, rng);
    }
    if case_requested("x86_reduce", size) {
        let input: Vec<[u64; 4]> = (0..n)
            .map(|_| std::array::from_fn(|_| rng.next()))
            .collect();
        let mut cases = Vec::new();
        for name in ["f2z_reduce", "flock_reduce"] {
            let input = &input;
            let mut out = vec![F128::ZERO; n];
            let apply = move |out: &mut [F128]| {
                if name == "f2z_reduce" {
                    reduce_array::<true>(input, out);
                } else {
                    reduce_array::<false>(input, out);
                }
            };
            apply(&mut out);
            assert!(
                out.iter()
                    .zip(input)
                    .all(|(&a, &w)| a == flock(Gf::from_wide(w)))
            );
            cases.push(Case::new(name, n * 48, move || {
                apply(black_box(&mut out));
                black_box(&out);
            }));
        }
        measure("x86_reduce", size, &mut cases, samples, rng);
    }
}

#[inline(never)]
fn unary<const OP: usize, const PROD: bool>(a: &[F128], b: &[F128], out: &mut [F128]) {
    for ((&a, &b), o) in black_box(a).iter().zip(black_box(b)).zip(out) {
        *o = match (OP, PROD) {
            (0, false) => a + b,
            (0, true) => flock(gf(a) + gf(b)),
            (1, false) => a.square(),
            (1, true) => flock(gf(a).square()),
            (_, false) => a.inv(),
            (_, true) => flock(gf(a).inverse()),
        };
    }
}
#[inline(never)]
fn fixed_kernel<const K: usize, const BUTTERFLY: bool>(
    a: &[F128],
    b: &[F128],
    t: F128,
    prepared: Prepared,
    out: &mut [(F128, F128)],
) {
    let t = black_box(t);
    let prepared = black_box(prepared);
    for ((&a, &b), o) in black_box(a).iter().zip(black_box(b)).zip(out) {
        let x = if BUTTERFLY { b } else { a };
        let product = match K {
            0 => scalar::<5>(x, t),
            1 => prepared.mul(x),
            _ => {
                if t == F128::ZERO {
                    F128::ZERO
                } else if t.hi == 0 {
                    crate::arithmetic::half_mul(x, t.lo)
                } else {
                    prepared.mul(x)
                }
            }
        };
        *o = if BUTTERFLY {
            let u = a + product;
            (u, b + u)
        } else {
            (product, F128::ZERO)
        };
    }
}

fn fixed(samples: usize, rng: &mut Rng) {
    for (kind, t) in [
        ("zero", F128::ZERO),
        ("half", F128::new(0xd938_4231_4bd1_372d, 0)),
        (
            "full",
            F128::new(0xd938_4231_4bd1_372d, 0x59a3_797b_923b_8371),
        ),
    ] {
        let prepared = Prepared::new(t);
        if case_requested("x86_fixed_prepare", kind) {
            let mut cases = vec![Case::new("prepared", 32, || {
                black_box(Prepared::new(black_box(t)));
            })];
            measure("x86_fixed_prepare", kind, &mut cases, samples, rng);
        }
        for n in [1, 3, 7, 16, 17, 1024, 65536, 1048576] {
            let size = format!("{kind}_n{n}");
            for family in ["x86_fixed", "x86_butterfly"] {
                if !case_requested(family, &size) {
                    continue;
                }
                let a = rng.values(n);
                let b = rng.values(n);
                let mut cases = Vec::new();
                for name in ["f2z_fixed", "prepared", "specialized"] {
                    let mut out = vec![(F128::ZERO, F128::ZERO); n];
                    let (a, b) = (&a, &b);
                    let apply = move |out: &mut [(F128, F128)]| match (family, name) {
                        ("x86_fixed", "f2z_fixed") => {
                            fixed_kernel::<0, false>(a, b, t, prepared, out)
                        }
                        ("x86_fixed", "prepared") => {
                            fixed_kernel::<1, false>(a, b, t, prepared, out)
                        }
                        ("x86_fixed", _) => fixed_kernel::<2, false>(a, b, t, prepared, out),
                        (_, "f2z_fixed") => fixed_kernel::<0, true>(a, b, t, prepared, out),
                        (_, "prepared") => fixed_kernel::<1, true>(a, b, t, prepared, out),
                        _ => fixed_kernel::<2, true>(a, b, t, prepared, out),
                    };
                    apply(&mut out);
                    assert!(out.iter().zip(a).zip(b).all(|((&(u, v), &a), &b)| {
                        let p = oracle(if family == "x86_fixed" { a } else { b }, t);
                        if family == "x86_fixed" {
                            u == p && v == F128::ZERO
                        } else {
                            u == a + p && v == b + u
                        }
                    }));
                    cases.push(Case::new(name, n * 64, move || {
                        apply(black_box(&mut out));
                        black_box(&out);
                    }));
                }
                measure(family, &size, &mut cases, samples, rng);
            }
        }
    }
}

pub fn run(samples: usize, rng: &mut Rng) {
    verify(rng);
    for n in [
        1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 1023, 1024, 1025, 65536, 1048576,
    ] {
        arrays(n, &n.to_string(), samples, rng);
    }
    // Actual per-candidate arrays exceed twice the selected cache group's L3.
    if let Ok(n) = std::env::var("FIELD_REGRESSION_STREAM_N") {
        let n: usize = n.parse().expect("invalid streaming size");
        arrays(n, &format!("stream_n{n}"), samples, rng);
    }
    for k in [1, 3, 6, 12, 24, 48] {
        for n in [16, 1024] {
            let size = format!("k{k}_n{n}");
            if !case_requested("x86_square_chain", &size) {
                continue;
            }
            let input = rng.values(n);
            let mut cases = Vec::new();
            for name in ["flock", "f2z"] {
                let mut out = vec![F128::ZERO; n];
                let input = &input;
                let apply = move |out: &mut [F128]| {
                    if name == "flock" {
                        square_chain::<false>(input, k, out);
                    } else {
                        square_chain::<true>(input, k, out);
                    }
                };
                apply(&mut out);
                assert!(out.iter().zip(input).all(|(&a, &x)| {
                    let mut x = x;
                    for _ in 0..k {
                        x = oracle(x, x);
                    }
                    a == x
                }));
                cases.push(Case::new(name, n * 32, move || {
                    apply(black_box(&mut out));
                    black_box(&out);
                }));
            }
            measure("x86_square_chain", &size, &mut cases, samples, rng);
        }
    }
    fixed(samples, rng);
    consumers::run(samples, rng);
}

#[inline(never)]
fn reduce_array<const PROD: bool>(input: &[[u64; 4]], out: &mut [F128]) {
    for (&w, o) in black_box(input).iter().zip(out) {
        *o = if PROD {
            flock(Gf::from_wide(w))
        } else {
            F256Unreduced {
                r0: w[0],
                r1: w[1],
                r2: w[2],
                r3: w[3],
            }
            .reduce()
        };
    }
}
#[inline(never)]
fn square_chain<const PROD: bool>(input: &[F128], k: usize, out: &mut [F128]) {
    for (&a, o) in black_box(input).iter().zip(out) {
        let mut a = a;
        for _ in 0..k {
            a = if PROD {
                flock(gf(a).square())
            } else {
                a.square()
            };
        }
        *o = a;
    }
}
