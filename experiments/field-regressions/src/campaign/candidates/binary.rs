use super::pass;
use crate::{Case, Rng, arithmetic, case_requested, measure};
use f2z::{
    poly::univariate::{binary_b127::B127, binary_gf128::Gf128 as Gf},
    utils::wide_mul::WideMulAcc,
};
use flock_core::field::{Gf8, Gf128};
use std::hint::black_box;

#[cfg(test)]
#[path = "correctness/binary.rs"]
mod tests;

#[inline(always)]
pub(super) fn half(a: Gf128, b: u64) -> Gf128 {
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    unsafe {
        use core::arch::aarch64::*;
        let lo = vreinterpretq_u64_p128(vmull_p64(a.lo, b));
        let hi = vreinterpretq_u64_p128(vmull_p64(a.hi, b));
        let fold = vreinterpretq_u64_p128(vmull_p64(vgetq_lane_u64::<1>(hi), 0x87));
        let out = veorq_u64(veorq_u64(lo, vextq_u64::<1>(vdupq_n_u64(0), hi)), fold);
        Gf128::new(vgetq_lane_u64::<0>(out), vgetq_lane_u64::<1>(out))
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
    {
        arithmetic::half_mul(a, b)
    }
}

#[inline(never)]
pub(super) fn fixed(input: &[Gf128], t: Gf128, output: &mut [Gf128]) {
    assert_eq!(input.len(), output.len());
    // t is a public pass-constant; shape dispatch happens once per pass.
    if t == Gf128::ZERO {
        output.fill(Gf128::ZERO);
    } else if t.hi == 0 {
        for (&a, o) in input.iter().zip(output) {
            *o = half(a, t.lo);
        }
    } else {
        let p = arithmetic::Prepared::new(t);
        for (&a, o) in input.iter().zip(output) {
            *o = p.mul(a);
        }
    }
}

fn fixed_cases(samples: usize, rng: &mut Rng) {
    use crate::production_fixed::{BinaryFieldGF128 as Wrapped, FixedGfMul};
    for (label, t) in [
        ("zero", Gf128::ZERO),
        ("half", Gf128::new(0xabcdef0123456789, 0)),
        ("full", Gf128::new(0x123456789abcdef, 0xfedcba9876543210)),
    ] {
        let prep = arithmetic::Prepared::new(t);
        let native = FixedGfMul::new(Wrapped::from_words([t.lo, t.hi]));
        for n in [0, 1, 3, 17, 16, 1024, 65536] {
            let size = format!("{label}_n{n}");
            if (n == 16 || n == 1024 || n == 65536)
                && !["opt_gf_fixed", "opt_gf_butterfly"]
                    .iter()
                    .any(|f| case_requested(f, &size))
            {
                continue;
            }
            let a = rng.values(n);
            let b = rng.values(n);
            let expected: Vec<_> = a.iter().map(|&a| a * t).collect();
            let mut o = vec![Gf128::ZERO; n];
            fixed(&a, t, &mut o);
            assert_eq!(o, expected);
            for (&a, &o) in a.iter().zip(&o).take(256) {
                assert_eq!(o, arithmetic::oracle(a, t));
                assert_eq!(
                    *native.mul(Wrapped::from_words([a.lo, a.hi])).words(),
                    [o.lo, o.hi]
                );
            }
            if n < 16 || n == 17 {
                continue;
            }
            let mut cases = vec![
                pass("actual_fixed_gf", n * 32, &a, &expected, |a, o| {
                    for (&x, o) in a.iter().zip(o) {
                        let w = *native.mul(Wrapped::from_words([x.lo, x.hi])).words();
                        *o = Gf128::new(w[0], w[1]);
                    }
                }),
                pass("prepared_formula", n * 32, &a, &expected, |a, o| {
                    for (&x, o) in a.iter().zip(o) {
                        *o = prep.mul(x);
                    }
                }),
                pass("public_scalar_dispatch", n * 32, &a, &expected, |a, o| {
                    fixed(a, t, o)
                }),
            ];
            measure("opt_gf_fixed", &size, &mut cases, samples, rng);
            let pairs: Vec<_> = a.iter().zip(b).map(|(&a, b)| (a, b)).collect();
            let expected: Vec<_> = pairs
                .iter()
                .map(|&(a, b)| {
                    let x = a + b * t;
                    (x, x + b)
                })
                .collect();
            let mut cases = vec![
                pass("actual_fixed_gf", n * 64, &pairs, &expected, |a, o| {
                    for (&(a, b), o) in a.iter().zip(o) {
                        let w = *native.mul(Wrapped::from_words([b.lo, b.hi])).words();
                        let x = a + Gf128::new(w[0], w[1]);
                        *o = (x, x + b);
                    }
                }),
                pass(
                    "public_scalar_dispatch",
                    n * 64,
                    &pairs,
                    &expected,
                    |a, o| {
                        if t == Gf128::ZERO {
                            for (&(a, b), o) in a.iter().zip(o) {
                                *o = (a, a + b);
                            }
                        } else if t.hi == 0 {
                            for (&(a, b), o) in a.iter().zip(o) {
                                let x = a + half(b, t.lo);
                                *o = (x, x + b);
                            }
                        } else {
                            for (&(a, b), o) in a.iter().zip(o) {
                                let x = a + prep.mul(b);
                                *o = (x, x + b);
                            }
                        }
                    },
                ),
            ];
            measure("opt_gf_butterfly", &size, &mut cases, samples, rng);
        }
    }
}

fn round<const K: usize>(l: &[Gf], r: &[Gf], w: &[Gf]) -> (Gf, Gf, Gf) {
    assert_eq!(l.len(), 2 * w.len());
    assert_eq!(r.len(), l.len());
    let zero = Gf::wide_zero(&Gf::zero());
    let mut acc = [[zero; 3]; K];
    for (i, ((&w, l), r)) in w
        .iter()
        .zip(l.chunks_exact(2))
        .zip(r.chunks_exact(2))
        .enumerate()
    {
        let a = l[0] * w;
        let b = l[1] * w;
        let slot = &mut acc[i % K];
        Gf::wide_add_assign(&mut slot[0], &Gf::mul_wide(&a, &r[0]));
        Gf::wide_add_assign(&mut slot[1], &Gf::mul_wide(&b, &r[1]));
        Gf::wide_add_assign(&mut slot[2], &Gf::mul_wide(&(a + b), &(r[0] + r[1])));
    }
    let mut merged = [zero; 3];
    for a in acc {
        for (i, a) in a.iter().enumerate() {
            Gf::wide_add_assign(&mut merged[i], a);
        }
    }
    let [c0, c1, c2] = merged.map(Gf::from_wide);
    (c0, c1 + c0 + c2, c2)
}
fn rounds(samples: usize, rng: &mut Rng) {
    for n in [0, 1, 3, 17, 16, 1024, 65536] {
        let size = n.to_string();
        if (n == 16 || n == 1024 || n == 65536) && !case_requested("opt_gf_round", &size) {
            continue;
        }
        let mut vals = |n| {
            rng.values(n)
                .iter()
                .map(|x| Gf::from_polynomial_words([x.lo, x.hi]))
                .collect::<Vec<_>>()
        };
        let l = vals(2 * n);
        let r = vals(2 * n);
        let w = vals(n);
        let expected = Gf::eqf_single_pair_round(&l, &r, &w, n).unwrap();
        assert_eq!(round::<1>(&l, &r, &w), expected);
        assert_eq!(round::<2>(&l, &r, &w), expected);
        if n < 16 || n == 17 {
            continue;
        }
        let mut cases = vec![
            Case::new("production_fused", n * 80, || {
                black_box(
                    Gf::eqf_single_pair_round(black_box(&l), black_box(&r), black_box(&w), n)
                        .unwrap(),
                );
            }),
            Case::new("wide1", n * 80, || {
                black_box(round::<1>(black_box(&l), black_box(&r), black_box(&w)));
            }),
            Case::new("wide2", n * 80, || {
                black_box(round::<2>(black_box(&l), black_box(&r), black_box(&w)));
            }),
        ];
        measure("opt_gf_round", &size, &mut cases, samples, rng);
    }
}

fn add_coefficients(a: (Gf, Gf, Gf), b: (Gf, Gf, Gf)) -> (Gf, Gf, Gf) {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}
fn hook_cases(samples: usize, rng: &mut Rng) {
    for n in [16, 1024] {
        #[cfg(test)]
        if samples == 0 {
            tests::check_grid_kernel(chunks);
            tests::check_grid_kernel(super::grid::run::<128>);
            tests::check_grid_kernel(super::grid::run::<4096>);
            return;
        }
        let size = n.to_string();
        if !["opt_gf_two_pair", "opt_gf_fold_round", "opt_gf_grid"]
            .iter()
            .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let mut values = |n| {
            rng.values(n)
                .iter()
                .map(|v| Gf::from_polynomial_words([v.lo, v.hi]))
                .collect::<Vec<_>>()
        };
        let l = values(16 * n);
        let r = values(16 * n);
        let w = values(n);
        let challenges = values(2);
        let rho = challenges[0];
        let reference = Gf::eqf_two_pair_round(
            &l[..2 * n],
            &r[..2 * n],
            &l[2 * n..4 * n],
            &r[2 * n..4 * n],
            &w,
            n,
        )
        .unwrap();
        let separate = || {
            add_coefficients(
                round::<1>(&l[..2 * n], &r[..2 * n], &w),
                round::<1>(&l[2 * n..4 * n], &r[2 * n..4 * n], &w),
            )
        };
        assert_eq!(separate(), reference);
        let mut cases = vec![
            Case::new("production_fused", n * 144, || {
                black_box(
                    Gf::eqf_two_pair_round(
                        black_box(&l[..2 * n]),
                        black_box(&r[..2 * n]),
                        black_box(&l[2 * n..4 * n]),
                        black_box(&r[2 * n..4 * n]),
                        black_box(&w),
                        n,
                    )
                    .unwrap(),
                );
            }),
            Case::new("separate_wide", n * 144, || {
                black_box(add_coefficients(
                    round::<1>(
                        black_box(&l[..2 * n]),
                        black_box(&r[..2 * n]),
                        black_box(&w),
                    ),
                    round::<1>(
                        black_box(&l[2 * n..4 * n]),
                        black_box(&r[2 * n..4 * n]),
                        black_box(&w),
                    ),
                ));
            }),
        ];
        measure("opt_gf_two_pair", &size, &mut cases, samples, rng);
        let mut l0 = l[..4 * n].to_vec();
        let mut r0 = r[..4 * n].to_vec();
        let mut l1 = l0.clone();
        let mut r1 = r0.clone();
        let expected = Gf::eqf_fused_fold_round(&mut l0, &mut r0, &rho, &w, n).unwrap();
        assert!(Gf::eqf_fold_in_place(&mut l1, &rho, 2 * n));
        assert!(Gf::eqf_fold_in_place(&mut r1, &rho, 2 * n));
        assert_eq!(round::<1>(&l1[..2 * n], &r1[..2 * n], &w), expected);
        assert_eq!(&l0[..2 * n], &l1[..2 * n]);
        assert_eq!(&r0[..2 * n], &r1[..2 * n]);
        let mut cases = vec![
            Case::new("production_fused", n * 256, || {
                l0.copy_from_slice(black_box(&l[..4 * n]));
                r0.copy_from_slice(black_box(&r[..4 * n]));
                black_box(
                    Gf::eqf_fused_fold_round(
                        black_box(&mut l0),
                        black_box(&mut r0),
                        black_box(&rho),
                        black_box(&w),
                        n,
                    )
                    .unwrap(),
                );
                black_box((&l0, &r0));
            }),
            Case::new("fold_then_wide", n * 256, || {
                l1.copy_from_slice(black_box(&l[..4 * n]));
                r1.copy_from_slice(black_box(&r[..4 * n]));
                assert!(Gf::eqf_fold_in_place(
                    black_box(&mut l1),
                    black_box(&rho),
                    2 * n
                ));
                assert!(Gf::eqf_fold_in_place(
                    black_box(&mut r1),
                    black_box(&rho),
                    2 * n
                ));
                black_box(round::<1>(
                    black_box(&l1[..2 * n]),
                    black_box(&r1[..2 * n]),
                    black_box(&w),
                ));
                black_box((&l1, &r1));
            }),
        ];
        measure("opt_gf_fold_round", &size, &mut cases, samples, rng);
        let mut l0 = l.clone();
        let mut r0 = r.clone();
        let mut l1 = l.clone();
        let mut r1 = r.clone();
        let expected =
            crate::production_grid::evaluate(&mut l0, &mut r0, &challenges, &w, n).unwrap();
        fn chunks(l: &mut [Gf], r: &mut [Gf], p: &[Gf], w: &[Gf]) -> [Gf; 9] {
            let mut out = [Gf::zero(); 9];
            for start in (0..w.len()).step_by(128) {
                let end = (start + 128).min(w.len());
                let offset = start * 16;
                let c = crate::production_grid::evaluate(
                    &mut l[offset..end * 16],
                    &mut r[offset..end * 16],
                    p,
                    &w[start..end],
                    end - start,
                )
                .unwrap();
                l.copy_within(offset..offset + (end - start) * 4, start * 4);
                r.copy_within(offset..offset + (end - start) * 4, start * 4);
                for (i, c) in c.into_iter().enumerate() {
                    out[i] += c;
                }
            }
            out
        }
        assert_eq!(chunks(&mut l1, &mut r1, &challenges, &w), expected);
        assert_eq!(&l0[..4 * n], &l1[..4 * n]);
        assert_eq!(&r0[..4 * n], &r1[..4 * n]);
        let mut l2 = l.clone();
        let mut r2 = r.clone();
        let mut l3 = l.clone();
        let mut r3 = r.clone();
        assert_eq!(
            super::grid::run::<128>(&mut l2, &mut r2, &challenges, &w),
            expected
        );
        assert_eq!(
            super::grid::run::<4096>(&mut l3, &mut r3, &challenges, &w),
            expected
        );
        assert_eq!(&l2[..4 * n], &l0[..4 * n]);
        assert_eq!(&r2[..4 * n], &r0[..4 * n]);
        let mut cases = vec![
            Case::new("direct_tiles", n * 1024, || {
                l2.copy_from_slice(black_box(&l));
                r2.copy_from_slice(black_box(&r));
                black_box(super::grid::run::<128>(
                    black_box(&mut l2),
                    black_box(&mut r2),
                    black_box(&challenges),
                    black_box(&w),
                ));
                black_box((&l2[..4 * n], &r2[..4 * n]));
            }),
            Case::new("direct_pass", n * 1024, || {
                l3.copy_from_slice(black_box(&l));
                r3.copy_from_slice(black_box(&r));
                black_box(super::grid::run::<4096>(
                    black_box(&mut l3),
                    black_box(&mut r3),
                    black_box(&challenges),
                    black_box(&w),
                ));
                black_box((&l3[..4 * n], &r3[..4 * n]));
            }),
            Case::new("production_fused", n * 1024, || {
                l0.copy_from_slice(black_box(&l));
                r0.copy_from_slice(black_box(&r));
                black_box(
                    crate::production_grid::evaluate(
                        black_box(&mut l0),
                        black_box(&mut r0),
                        black_box(&challenges),
                        black_box(&w),
                        n,
                    )
                    .unwrap(),
                );
                black_box((&l0[..4 * n], &r0[..4 * n]));
            }),
            Case::new("chunked", n * 1024, || {
                l1.copy_from_slice(black_box(&l));
                r1.copy_from_slice(black_box(&r));
                black_box(chunks(
                    black_box(&mut l1),
                    black_box(&mut r1),
                    black_box(&challenges),
                    black_box(&w),
                ));
                black_box((&l1[..4 * n], &r1[..4 * n]));
            }),
        ];
        measure("opt_gf_grid", &size, &mut cases, samples, rng);
    }
}

#[inline(always)]
fn byte_mul(a: u8, b: u8) -> u8 {
    let mut a = a;
    let mut out = 0;
    for i in 0..8 {
        out ^= a & 0u8.wrapping_sub((b >> i) & 1);
        a = (a << 1) ^ (0x1b & 0u8.wrapping_sub(a >> 7));
    }
    out
}
#[inline(never)]
fn bytes(a: &[u8], b: &[u8], out: &mut [u8]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let n = a.len() / 16 * 16;
    #[cfg(target_arch = "aarch64")]
    for ((a, b), o) in a[..n]
        .chunks_exact(16)
        .zip(b[..n].chunks_exact(16))
        .zip(out[..n].chunks_exact_mut(16))
    {
        unsafe {
            use core::arch::aarch64::*;
            let v = flock_core::field::gf8_kernels::neon::gf8_mul_vec16(
                vld1q_u8(a.as_ptr()),
                vld1q_u8(b.as_ptr()),
            );
            vst1q_u8(o.as_mut_ptr(), v);
        }
    }
    #[cfg(not(target_arch = "aarch64"))]
    for ((&a, &b), o) in a[..n].iter().zip(&b[..n]).zip(&mut out[..n]) {
        *o = byte_mul(a, b);
    }
    for ((&a, &b), o) in a[n..].iter().zip(&b[n..]).zip(&mut out[n..]) {
        *o = byte_mul(a, b);
    }
}
#[inline(always)]
fn byte_inverse(x: u8) -> u8 {
    let x2 = byte_mul(x, x);
    let x3 = byte_mul(x2, x);
    let x6 = byte_mul(x3, x3);
    let x12 = byte_mul(x6, x6);
    let x15 = byte_mul(x12, x3);
    let x30 = byte_mul(x15, x15);
    let x60 = byte_mul(x30, x30);
    let x120 = byte_mul(x60, x60);
    let x240 = byte_mul(x120, x120);
    byte_mul(byte_mul(x240, x12), x2)
}
fn byte_inverses(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    let n = input.len() / 16 * 16;
    #[cfg(target_arch = "aarch64")]
    for (a, o) in input[..n]
        .chunks_exact(16)
        .zip(out[..n].chunks_exact_mut(16))
    {
        unsafe {
            use core::arch::aarch64::*;
            use flock_core::field::gf8_kernels::neon::gf8_mul_vec16 as mul;
            let x = vld1q_u8(a.as_ptr());
            let x2 = mul(x, x);
            let x3 = mul(x2, x);
            let x6 = mul(x3, x3);
            let x12 = mul(x6, x6);
            let x15 = mul(x12, x3);
            let x30 = mul(x15, x15);
            let x60 = mul(x30, x30);
            let x120 = mul(x60, x60);
            let x240 = mul(x120, x120);
            let x252 = mul(x240, x12);
            vst1q_u8(o.as_mut_ptr(), mul(x252, x2));
        }
    }
    #[cfg(not(target_arch = "aarch64"))]
    for (&a, o) in input[..n].iter().zip(&mut out[..n]) {
        *o = byte_inverse(a);
    }
    for (&a, o) in input[n..].iter().zip(&mut out[n..]) {
        *o = byte_inverse(a);
    }
}
fn small_fields(samples: usize, rng: &mut Rng) {
    let domain: Vec<_> = (0..=255).collect();
    let mut inverse = vec![0; 256];
    byte_inverses(&domain, &mut inverse);
    for (&a, &b) in domain.iter().zip(&inverse) {
        assert_eq!(b, Gf8(a).inverse_or_zero().0);
        assert_eq!(byte_inverse(a), b);
    }
    for a in 0..=255 {
        for b in 0..=255 {
            assert_eq!(byte_mul(a, b), (Gf8(a) * Gf8(b)).0);
        }
    }
    let basis: Vec<_> = (0..8)
        .map(|i| flock_core::field::embed_gf8(Gf8(1 << i)))
        .collect();
    let embedding = |a: u8| {
        let mut out = Gf128::ZERO;
        for (i, &b) in basis.iter().enumerate() {
            let mask = 0u64.wrapping_sub(((a >> i) & 1) as u64);
            out += Gf128::new(b.lo & mask, b.hi & mask);
        }
        out
    };
    for a in 0..=255 {
        assert_eq!(embedding(a), flock_core::field::embed_gf8(Gf8(a)));
    }
    for n in [0, 1, 15, 17, 16, 1024, 65536] {
        let size = n.to_string();
        if [16, 1024, 65536].contains(&n)
            && !["opt_gf8_mul", "opt_gf8_inverse", "opt_phi8", "opt_b127_mul"]
                .iter()
                .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let a: Vec<_> = (0..n).map(|_| rng.next() as u8).collect();
        let b: Vec<_> = (0..n).map(|_| rng.next() as u8).collect();
        let expected: Vec<_> = a
            .iter()
            .zip(&b)
            .map(|(&a, &b)| (Gf8(a) * Gf8(b)).0)
            .collect();
        let mut out = vec![0; n];
        bytes(&a, &b, &mut out);
        assert_eq!(out, expected);
        if ![16, 1024, 65536].contains(&n) {
            continue;
        }
        let mut cases = vec![
            pass("flock_scalar", n * 3, &a, &expected, |a, o| {
                for ((&a, &b), o) in a.iter().zip(&b).zip(o) {
                    *o = (Gf8(a) * Gf8(b)).0;
                }
            }),
            pass("native_batch", n * 3, &a, &expected, |a, o| bytes(a, &b, o)),
        ];
        measure("opt_gf8_mul", &size, &mut cases, samples, rng);
        let expected: Vec<_> = a.iter().map(|&a| Gf8(a).inverse_or_zero().0).collect();
        let mut cases = vec![
            pass("flock_scalar", n * 2, &a, &expected, |a, o| {
                for (&a, o) in a.iter().zip(o) {
                    *o = Gf8(a).inverse_or_zero().0;
                }
            }),
            pass("vector_chain", n * 2, &a, &expected, |a, o| {
                byte_inverses(a, o)
            }),
        ];
        measure("opt_gf8_inverse", &size, &mut cases, samples, rng);
        let expected: Vec<_> = a.iter().map(|&a| embedding(a)).collect();
        let mut cases = vec![
            pass("table_public_input", n * 17, &a, &expected, |a, o| {
                for (&a, o) in a.iter().zip(o) {
                    *o = flock_core::field::embed_gf8(Gf8(a));
                }
            }),
            pass("fixed_basis", n * 17, &a, &expected, |a, o| {
                for (&a, o) in a.iter().zip(o) {
                    *o = embedding(a);
                }
            }),
        ];
        measure("opt_phi8", &size, &mut cases, samples, rng);
        let a: Vec<_> = rng
            .values(n)
            .iter()
            .map(|v| B127::from_polynomial_words([v.lo, v.hi >> 1]))
            .collect();
        let b: Vec<_> = rng
            .values(n)
            .iter()
            .map(|v| B127::from_polynomial_words([v.lo, v.hi >> 1]))
            .collect();
        let expected: Vec<_> = a.iter().zip(&b).map(|(&a, &b)| a * b).collect();
        let mut cases = vec![pass("production", n * 48, &a, &expected, |a, o| {
            for ((&a, &b), o) in a.iter().zip(&b).zip(o) {
                *o = a * b;
            }
        })];
        #[cfg(target_arch = "aarch64")]
        {
            cases.push(pass("karatsuba", n * 48, &a, &expected, |a, o| {
                for ((a, b), o) in a.iter().zip(&b).zip(o) {
                    *o = a.mul_karatsuba(*b);
                }
            }));
            cases.push(pass("pfold", n * 48, &a, &expected, |a, o| {
                for ((a, b), o) in a.iter().zip(&b).zip(o) {
                    *o = a.mul_pfold(*b);
                }
            }));
        }
        measure("opt_b127_mul", &size, &mut cases, samples, rng);
    }
}

fn polynomial<
    const AB: usize,
    const A: usize,
    const BB: usize,
    const B: usize,
    const OB: usize,
    const O: usize,
>(
    samples: usize,
    rng: &mut Rng,
) {
    use field::F2Poly as Poly;
    fn f2_inner_product<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        use field::BatchMulAcc;
        assert!(O >= A + B);
        let product = field::F2PolyOps.batch_mul_acc(a, b).into_payload();
        let (lo, hi) = product.as_parts();
        let mut out = [0; O];
        out[..A].copy_from_slice(lo);
        out[A..A + B].copy_from_slice(hi);
        Poly::from_polynomial_words(out)
    }
    // Explicitly variable-time, matching the public-input production control.
    // This is a separate entry point from fixed(); it never selects a timing
    // contract by inspecting values. Fuse products into the final accumulator.
    fn public_fused<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            for i in 0..A {
                let x = a.as_words()[i];
                if x == 0 {
                    continue;
                }
                for j in 0..B {
                    let y = b.as_words()[j];
                    if y == 0 {
                        continue;
                    }
                    let p = arithmetic::clmul(x, y);
                    out[i + j] ^= p as u64;
                    out[i + j + 1] ^= (p >> 64) as u64;
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    fn fixed<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            for i in 0..A {
                for j in 0..B {
                    let p = arithmetic::clmul(a.as_words()[i], b.as_words()[j]);
                    out[i + j] ^= p as u64;
                    out[i + j + 1] ^= (p >> 64) as u64;
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    fn public_adaptive<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        // Sampling is allowed only for this explicitly public-input entry
        // point. Both paths are correct for arbitrary later coefficients.
        // Private inputs must call fixed(), which never samples values.
        let sparse = a.iter().take(4).any(|x| x.as_words().contains(&0))
            || b.iter().take(4).any(|x| x.as_words().contains(&0));
        if sparse {
            if B >= 9 {
                public_compact::<AB, A, BB, B, OB, O>(a, b)
            } else {
                public_fused::<AB, A, BB, B, OB, O>(a, b)
            }
        } else {
            fixed::<AB, A, BB, B, OB, O>(a, b)
        }
    }
    fn public_compact<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            let mut values = [0; B];
            let mut indices = [0; B];
            let mut count = 0;
            for (j, &y) in b.as_words().iter().enumerate() {
                if y != 0 {
                    values[count] = y;
                    indices[count] = j;
                    count += 1;
                }
            }
            for (i, &x) in a.as_words().iter().enumerate() {
                if x == 0 {
                    continue;
                }
                for k in 0..count {
                    let p = arithmetic::clmul(x, values[k]);
                    let j = indices[k];
                    out[i + j] ^= p as u64;
                    out[i + j + 1] ^= (p >> 64) as u64;
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    fn public_rows<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            let aw = a.as_words();
            let bw = b.as_words();
            if aw.iter().fold(0, |s, &w| s | w) == 0 {
                continue;
            }
            let dense = aw.iter().all(|&w| w != 0) && bw.iter().all(|&w| w != 0);
            if dense {
                for i in 0..A {
                    for j in 0..B {
                        let p = arithmetic::clmul(aw[i], bw[j]);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            } else if B >= 9 {
                let mut values = [0; B];
                let mut indices = [0; B];
                let mut count = 0;
                for (j, &w) in bw.iter().enumerate() {
                    if w != 0 {
                        values[count] = w;
                        indices[count] = j;
                        count += 1;
                    }
                }
                for (i, &x) in aw.iter().enumerate() {
                    if x == 0 {
                        continue;
                    }
                    for k in 0..count {
                        let p = arithmetic::clmul(x, values[k]);
                        let j = indices[k];
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            } else {
                for (i, &x) in aw.iter().enumerate() {
                    if x == 0 {
                        continue;
                    }
                    for (j, &y) in bw.iter().enumerate() {
                        if y == 0 {
                            continue;
                        }
                        let p = arithmetic::clmul(x, y);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    fn public_classified<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            let aw = a.as_words();
            let bw = b.as_words();
            if aw.iter().fold(0, |s, &w| s | w) == 0 {
                continue;
            }
            let dense = aw.iter().fold(true, |ok, &w| ok & (w != 0))
                & bw.iter().fold(true, |ok, &w| ok & (w != 0));
            if dense {
                for i in 0..A {
                    for j in 0..B {
                        let p = arithmetic::clmul(aw[i], bw[j]);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            } else if B >= 9 {
                let mut values = [0; B];
                let mut indices = [0; B];
                let mut count = 0;
                for (j, &w) in bw.iter().enumerate() {
                    if w != 0 {
                        values[count] = w;
                        indices[count] = j;
                        count += 1;
                    }
                }
                for (i, &x) in aw.iter().enumerate() {
                    if x == 0 {
                        continue;
                    }
                    for k in 0..count {
                        let p = arithmetic::clmul(x, values[k]);
                        let j = indices[k];
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            } else {
                for (i, &x) in aw.iter().enumerate() {
                    if x == 0 {
                        continue;
                    }
                    for (j, &y) in bw.iter().enumerate() {
                        if y == 0 {
                            continue;
                        }
                        let p = arithmetic::clmul(x, y);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    // Classify each public row once. Sparse multiplication visits only set
    // positions instead of testing every possible limb pair repeatedly.
    fn public_masks<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(A < 64 && B < 64 && O >= A + B);
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.iter().zip(b) {
            let mut am = 0u64;
            for (i, &x) in a.as_words().iter().enumerate() {
                am |= ((x != 0) as u64) << i;
            }
            if am == 0 {
                continue;
            }
            let mut bm = 0u64;
            for (j, &y) in b.as_words().iter().enumerate() {
                bm |= ((y != 0) as u64) << j;
            }
            if am == (1 << A) - 1 && bm == (1 << B) - 1 {
                for i in 0..A {
                    for j in 0..B {
                        let p = arithmetic::clmul(a.as_words()[i], b.as_words()[j]);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            } else {
                while am != 0 {
                    let i = am.trailing_zeros() as usize;
                    am &= am - 1;
                    let mut mask = bm;
                    while mask != 0 {
                        let j = mask.trailing_zeros() as usize;
                        mask &= mask - 1;
                        let p = arithmetic::clmul(a.as_words()[i], b.as_words()[j]);
                        out[i + j] ^= p as u64;
                        out[i + j + 1] ^= (p >> 64) as u64;
                    }
                }
            }
        }
        Poly::from_polynomial_words(out)
    }
    fn public_blocks<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
        const BLOCK: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert_eq!(a.len(), b.len());
        let mut out = [0; O];
        for (a, b) in a.chunks(BLOCK).zip(b.chunks(BLOCK)) {
            let dense = a.iter().all(|a| a.as_words().iter().all(|&x| x != 0))
                && b.iter().all(|b| b.as_words().iter().all(|&x| x != 0));
            let part = if dense {
                fixed::<AB, A, BB, B, OB, O>(a, b)
            } else {
                public_fused::<AB, A, BB, B, OB, O>(a, b)
            };
            for (o, &p) in out.iter_mut().zip(part.as_words()) {
                *o ^= p;
            }
        }
        Poly::from_polynomial_words(out)
    }
    // Inspect complete rows, stopping at the first genuinely sparse row.
    // Wholly zero rows do not make a dense batch sparse. No sampled prefix is
    // treated as evidence about unread rows; both kernels accept all inputs.
    fn public_scan<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert_eq!(a.len(), b.len());
        let mut nonzero = false;
        for (x, y) in a.iter().zip(b) {
            if x.as_words().iter().fold(0, |s, &x| s | x) == 0 {
                continue;
            }
            nonzero = true;
            if x.as_words().iter().any(|&x| x == 0) || y.as_words().iter().any(|&x| x == 0) {
                return public_fused::<AB, A, BB, B, OB, O>(a, b);
            }
        }
        if nonzero {
            public_rows::<AB, A, BB, B, OB, O>(a, b)
        } else {
            Poly::from_polynomial_words([0; O])
        }
    }
    // Keep each carryless-product diagonal in SIMD registers across rows.
    // Combining low/high halves once avoids two scalar extracts per product.
    #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
    fn public_simd<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        use core::arch::x86_64::*;
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        // PCLMUL is guaranteed by the compiled feature. Loads/stores below
        // cover local 128-bit values; public zero-skips match public_fused.
        unsafe {
            let mut diagonals = [_mm_setzero_si128(); O];
            for (a, b) in a.iter().zip(b) {
                for i in 0..A {
                    let x = a.as_words()[i];
                    if x == 0 {
                        continue;
                    }
                    let x = _mm_set_epi64x(0, x as i64);
                    for j in 0..B {
                        let y = b.as_words()[j];
                        if y == 0 {
                            continue;
                        }
                        let p = _mm_clmulepi64_si128::<0>(x, _mm_set_epi64x(0, y as i64));
                        diagonals[i + j] = _mm_xor_si128(diagonals[i + j], p);
                    }
                }
            }
            let mut out = [0; O];
            for i in 0..A + B - 1 {
                let mut words = [0u64; 2];
                _mm_storeu_si128(words.as_mut_ptr().cast(), diagonals[i]);
                out[i] ^= words[0];
                out[i + 1] ^= words[1];
            }
            Poly::from_polynomial_words(out)
        }
    }
    // Consume dense/zero public rows directly, then keep the sparse suffix in
    // a separate sparse kernel. Classification never rereads a prefix.
    fn public_prefix<
        const AB: usize,
        const A: usize,
        const BB: usize,
        const B: usize,
        const OB: usize,
        const O: usize,
    >(
        a: &[Poly<AB, A>],
        b: &[Poly<BB, B>],
    ) -> Poly<OB, O> {
        assert!(O >= A + B);
        assert_eq!(a.len(), b.len());
        #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
        if A == 3 && B == 7 {
            // Retain the zero-prefix shortcut before initializing SIMD state.
            // A zero left polynomial contributes nothing, regardless of b.
            let first = a
                .iter()
                .position(|x| x.as_words().iter().fold(0, |bits, &w| bits | w) != 0);
            return match first {
                Some(first) => public_simd::<AB, A, BB, B, OB, O>(&a[first..], &b[first..]),
                None => Poly::from_polynomial_words([0; O]),
            };
        }
        let mut out = [0; O];
        let mut split = a.len();
        for (row, (x, y)) in a.iter().zip(b).enumerate() {
            let aw = x.as_words();
            let bw = y.as_words();
            if aw.iter().fold(0, |s, &w| s | w) == 0 {
                continue;
            }
            if aw.iter().any(|&w| w == 0) || bw.iter().any(|&w| w == 0) {
                split = row;
                break;
            }
            for i in 0..A {
                for j in 0..B {
                    let p = arithmetic::clmul(aw[i], bw[j]);
                    out[i + j] ^= p as u64;
                    out[i + j + 1] ^= (p >> 64) as u64;
                }
            }
        }
        if split < a.len() {
            let suffix = public_fused::<AB, A, BB, B, OB, O>(&a[split..], &b[split..]);
            for (out, &word) in out.iter_mut().zip(suffix.as_words()) {
                *out ^= word;
            }
        }
        Poly::from_polynomial_words(out)
    }
    // Test the actual nested kernels without moving or changing their measured bodies.
    #[cfg(test)]
    if samples == 0 {
        tests::check_polynomial::<AB, A, BB, B, OB, O>(&[
            fixed::<AB, A, BB, B, OB, O>,
            public_fused::<AB, A, BB, B, OB, O>,
            public_compact::<AB, A, BB, B, OB, O>,
            public_adaptive::<AB, A, BB, B, OB, O>,
            public_rows::<AB, A, BB, B, OB, O>,
            public_masks::<AB, A, BB, B, OB, O>,
            public_scan::<AB, A, BB, B, OB, O>,
            public_prefix::<AB, A, BB, B, OB, O>,
            public_classified::<AB, A, BB, B, OB, O>,
            public_blocks::<AB, A, BB, B, OB, O, 8>,
            public_blocks::<AB, A, BB, B, OB, O, 32>,
        ]);
        return;
    }
    // Exercise misleading samples as well as all-zero input: adaptation may
    // change cost, but must never change arithmetic or omit later terms.
    for n in [0, 1, 4, 17] {
        for pattern in 0..4 {
            let mut inputs = |width: usize| -> Vec<Vec<u64>> {
                (0..n)
                    .map(|i| {
                        (0..width)
                            .map(|j| {
                                let zero = match pattern {
                                    0 => true,
                                    1 => i >= 4,
                                    2 => i < 4,
                                    _ => (i + j) % 3 == 0,
                                };
                                if zero { 0 } else { rng.next() }
                            })
                            .collect()
                    })
                    .collect()
            };
            let a: Vec<_> = inputs(A)
                .into_iter()
                .map(|w| Poly::<AB, A>::from_polynomial_words(w.try_into().unwrap()))
                .collect();
            let b: Vec<_> = inputs(B)
                .into_iter()
                .map(|w| Poly::<BB, B>::from_polynomial_words(w.try_into().unwrap()))
                .collect();
            let expected = f2_inner_product::<AB, A, BB, B, OB, O>(&a, &b);
            assert_eq!(fixed::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_fused::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_adaptive::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_rows::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_masks::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_scan::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_prefix::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_classified::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_blocks::<AB, A, BB, B, OB, O, 8>(&a, &b), expected);
            assert_eq!(public_blocks::<AB, A, BB, B, OB, O, 32>(&a, &b), expected);
        }
    }
    for class in [
        "dense",
        "sparse",
        "zero",
        "dense_prefix",
        "sparse_prefix",
        "alternating",
    ] {
        for n in [0, 1, 3, 15, 16, 17, 1023, 1024, 1025] {
            let size = format!("a{A}_b{B}_{class}_n{n}");
            if n >= 15 && !case_requested("opt_f2_poly_dot", &size) {
                continue;
            }
            let mut word = |i: usize| {
                let x = rng.next();
                let zero = match class {
                    "zero" => true,
                    "dense_prefix" => i >= 4 && x & 3 != 0,
                    "sparse_prefix" => i < 4,
                    "alternating" => i % 2 == 0,
                    "sparse" => x & 3 != 0,
                    _ => false,
                };
                if zero { 0 } else { x }
            };
            let a: Vec<_> = (0..n)
                .map(|i| Poly::<AB, A>::from_polynomial_words(std::array::from_fn(|_| word(i))))
                .collect();
            let b: Vec<_> = (0..n)
                .map(|i| Poly::<BB, B>::from_polynomial_words(std::array::from_fn(|_| word(i))))
                .collect();
            let expected = f2_inner_product::<AB, A, BB, B, OB, O>(&a, &b);
            assert_eq!(fixed::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_fused::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_adaptive::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_rows::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_masks::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_scan::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_prefix::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_classified::<AB, A, BB, B, OB, O>(&a, &b), expected);
            assert_eq!(public_blocks::<AB, A, BB, B, OB, O, 8>(&a, &b), expected);
            assert_eq!(public_blocks::<AB, A, BB, B, OB, O, 32>(&a, &b), expected);
            if n < 15 {
                continue;
            }
            let mut cases = vec![
                Case::new("production_zero_skip", n * (A + B) * 8, || {
                    black_box(f2_inner_product::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("fixed_schedule", n * (A + B) * 8, || {
                    black_box(fixed::<AB, A, BB, B, OB, O>(black_box(&a), black_box(&b)));
                }),
                Case::new("public_fused", n * (A + B) * 8, || {
                    black_box(public_fused::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_adaptive", n * (A + B) * 8, || {
                    black_box(public_adaptive::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_blocks8", n * (A + B) * 8, || {
                    black_box(public_blocks::<AB, A, BB, B, OB, O, 8>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_blocks32", n * (A + B) * 8, || {
                    black_box(public_blocks::<AB, A, BB, B, OB, O, 32>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_classified", n * (A + B) * 8, || {
                    black_box(public_classified::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_prefix", n * (A + B) * 8, || {
                    black_box(public_prefix::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_scan", n * (A + B) * 8, || {
                    black_box(public_scan::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("public_masks", n * (A + B) * 8, || {
                    black_box(public_masks::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
                Case::new("row_density", n * (A + B) * 8, || {
                    black_box(public_rows::<AB, A, BB, B, OB, O>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
            ];
            measure("opt_f2_poly_dot", &size, &mut cases, samples, rng);
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    fixed_cases(samples, rng);
    rounds(samples, rng);
    hook_cases(samples, rng);
    small_fields(samples, rng);
    polynomial::<64, 1, 64, 1, 128, 2>(samples, rng);
    polynomial::<192, 3, 448, 7, 640, 10>(samples, rng);
    polynomial::<576, 9, 576, 9, 1152, 18>(samples, rng);
}
