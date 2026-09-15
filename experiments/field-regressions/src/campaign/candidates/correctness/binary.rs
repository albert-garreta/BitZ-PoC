use super::super::correctness as oracle;
use super::*;
use field::F2Poly as Poly;

fn gf(x: Gf128) -> Gf {
    Gf::from_polynomial_words([x.lo, x.hi])
}
fn f(x: Gf) -> Gf128 {
    Gf128::new(x.as_words()[0], x.as_words()[1])
}

#[test]
fn gf128_prepared_and_half_multiplication_all_basis_pairs() {
    for i in 0..128 {
        let t = arithmetic::from_u128(1u128 << i);
        let prepared = arithmetic::Prepared::new(t);
        for j in 0..128 {
            let a = arithmetic::from_u128(1u128 << j);
            let expected = arithmetic::oracle(a, t);
            assert_eq!(prepared.mul(a), expected);
            if i < 64 {
                assert_eq!(half(a, t.lo), expected);
            }
        }
    }
    let mut rng = Rng(0x6669_7865_645f_6766);
    for t in [
        Gf128::ZERO,
        Gf128::ONE,
        Gf128::new(u64::MAX, 0),
        Gf128::new(0, 1),
        Gf128::new(u64::MAX, u64::MAX),
    ] {
        for n in [0, 1, 2, 3, 15, 16, 17, 31, 32, 33, 257] {
            let input = rng.values(n);
            let expected: Vec<_> = input.iter().map(|&a| arithmetic::oracle(a, t)).collect();
            let mut out = vec![Gf128::new(u64::MAX, u64::MAX); n];
            fixed(&input, t, &mut out);
            assert_eq!(out, expected);
        }
    }
}

#[test]
fn gf128_deferred_dots_match_bit_serial_products() {
    let mut rng = Rng(0x646f_745f_6766_3132);
    for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 257] {
        let a = rng.values(n);
        let b = rng.values(n);
        let expected = a
            .iter()
            .zip(&b)
            .fold(Gf128::ZERO, |s, (&a, &b)| s + arithmetic::oracle(a, b));
        assert_eq!(arithmetic::wide_dot::<1>(&a, &b), expected);
        assert_eq!(arithmetic::wide_dot::<2>(&a, &b), expected);
        assert_eq!(arithmetic::wide_dot::<4>(&a, &b), expected);
        assert_eq!(arithmetic::wide_dot::<16>(&a, &b), expected);
        assert_eq!(arithmetic::vec2_dot(&a, &b), expected);
    }
}

fn byte_reference(a: u8, b: u8) -> u8 {
    let mut product = 0u16;
    for i in 0..8 {
        if (b >> i) & 1 != 0 {
            product ^= (a as u16) << i;
        }
    }
    for i in (8..16).rev() {
        if (product >> i) & 1 != 0 {
            product ^= 0x11b << (i - 8);
        }
    }
    product as u8
}

#[test]
fn gf8_all_products_and_inverse_simd_tails() {
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut expected = Vec::new();
    for x in 0..=255 {
        for y in 0..=255 {
            a.push(x);
            b.push(y);
            expected.push(byte_reference(x, y));
            assert_eq!(byte_mul(x, y), byte_reference(x, y));
        }
    }
    let mut out = vec![0; a.len()];
    bytes(&a, &b, &mut out);
    assert_eq!(out, expected);
    for n in 0..=33 {
        out.resize(n, 0xff);
        bytes(&a[123..123 + n], &b[123..123 + n], &mut out);
        assert_eq!(out, &expected[123..123 + n]);
    }
    for n in [0, 1, 15, 16, 17, 255, 256, 257, 271] {
        let input: Vec<_> = (0..n).map(|i| i as u8).collect();
        let mut out = vec![0xff; n];
        byte_inverses(&input, &mut out);
        for (&x, &inverse) in input.iter().zip(&out) {
            let expected = if x == 0 {
                0
            } else {
                (1..=255).find(|&y| byte_reference(x, y) == 1).unwrap()
            };
            assert_eq!(inverse, expected);
            assert_eq!(byte_inverse(x), expected);
        }
    }
}

#[test]
fn sumcheck_polynomial_evaluates_to_weighted_affine_products() {
    let mut rng = Rng(0x7375_6d63_6865_636b);
    for n in [0, 1, 2, 3, 15, 16, 17, 127, 128, 129, 257] {
        let l = rng.values(2 * n);
        let r = rng.values(2 * n);
        let w = rng.values(n);
        let lg: Vec<_> = l.iter().copied().map(gf).collect();
        let rg: Vec<_> = r.iter().copied().map(gf).collect();
        let wg: Vec<_> = w.iter().copied().map(gf).collect();
        for (c0, c1, c2) in [round::<1>(&lg, &rg, &wg), round::<2>(&lg, &rg, &wg)] {
            for x in [Gf128::ZERO, Gf128::ONE, Gf128::new(rng.next(), rng.next())] {
                let expected = (0..n).fold(Gf128::ZERO, |s, i| {
                    let left = l[2 * i] + arithmetic::oracle(l[2 * i] + l[2 * i + 1], x);
                    let right = r[2 * i] + arithmetic::oracle(r[2 * i] + r[2 * i + 1], x);
                    s + arithmetic::oracle(w[i], arithmetic::oracle(left, right))
                });
                let actual = f(c0)
                    + arithmetic::oracle(f(c1), x)
                    + arithmetic::oracle(f(c2), arithmetic::oracle(x, x));
                assert_eq!(actual, expected);
            }
        }
    }
}

fn affine(a: Gf128, b: Gf128, x: Gf128) -> Gf128 {
    a + arithmetic::oracle(a + b, x)
}

#[test]
fn fused_fold_preserves_every_output_at_zero_one_and_random_challenges() {
    let mut rng = Rng(0x666f_6c64_5f66_7573);
    for n in [0, 1, 2, 3, 15, 16, 17, 127, 128, 129, 257] {
        let l = rng.values(4 * n);
        let r = rng.values(4 * n);
        let w = rng.values(n);
        let wg: Vec<_> = w.iter().copied().map(gf).collect();
        for rho in [Gf128::ZERO, Gf128::ONE, Gf128::new(rng.next(), rng.next())] {
            let folded = |v: &[Gf128]| {
                v.chunks_exact(2)
                    .map(|p| affine(p[0], p[1], rho))
                    .collect::<Vec<_>>()
            };
            let left = folded(&l);
            let right = folded(&r);
            let mut lg: Vec<_> = l.iter().copied().map(gf).collect();
            let mut rg: Vec<_> = r.iter().copied().map(gf).collect();
            let (c0, c1, c2) =
                Gf::eqf_fused_fold_round(&mut lg, &mut rg, &gf(rho), &wg, n).unwrap();
            assert_eq!(lg[..2 * n].iter().copied().map(f).collect::<Vec<_>>(), left);
            assert_eq!(
                rg[..2 * n].iter().copied().map(f).collect::<Vec<_>>(),
                right
            );
            for x in [Gf128::ZERO, Gf128::ONE, Gf128::new(rng.next(), rng.next())] {
                let expected = (0..n).fold(Gf128::ZERO, |s, i| {
                    s + arithmetic::oracle(
                        w[i],
                        arithmetic::oracle(
                            affine(left[2 * i], left[2 * i + 1], x),
                            affine(right[2 * i], right[2 * i + 1], x),
                        ),
                    )
                });
                assert_eq!(
                    f(c0)
                        + arithmetic::oracle(f(c1), x)
                        + arithmetic::oracle(f(c2), arithmetic::oracle(x, x)),
                    expected
                );
            }
            lg = l.iter().copied().map(gf).collect();
            assert!(Gf::eqf_fold_in_place(&mut lg, &gf(rho), 2 * n));
            assert_eq!(lg[..2 * n].iter().copied().map(f).collect::<Vec<_>>(), left);
        }
    }
}

pub(super) fn check_grid_kernel(kernel: fn(&mut [Gf], &mut [Gf], &[Gf], &[Gf]) -> [Gf; 9]) {
    let mut rng = Rng(0x6772_6964_5f66_7573);
    for n in [0, 1, 2, 3, 127, 128, 129, 257] {
        let l = rng.values(16 * n);
        let r = rng.values(16 * n);
        let w = rng.values(n);
        for point in [
            [Gf128::ZERO, Gf128::ZERO],
            [Gf128::ONE, Gf128::ONE],
            [Gf128::ZERO, Gf128::ONE],
            [
                Gf128::new(rng.next(), rng.next()),
                Gf128::new(rng.next(), rng.next()),
            ],
        ] {
            let fold = |v: &[Gf128]| {
                v.chunks_exact(4)
                    .map(|p| {
                        affine(
                            affine(p[0], p[1], point[0]),
                            affine(p[2], p[3], point[0]),
                            point[1],
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let left = fold(&l);
            let right = fold(&r);
            let mut coefficients = [[Gf128::ZERO; 3]; 3];
            for i in 0..n {
                let coeff =
                    |p: &[Gf128]| [p[0], p[1] + p[0], p[2] + p[0], p[3] + p[2] + p[1] + p[0]];
                let a = coeff(&left[4 * i..4 * i + 4]);
                let b = coeff(&right[4 * i..4 * i + 4]);
                for j in 0..4 {
                    for k in 0..4 {
                        coefficients[(j & 1) + (k & 1)][(j >> 1) + (k >> 1)] +=
                            arithmetic::oracle(w[i], arithmetic::oracle(a[j], b[k]));
                    }
                }
            }
            let expected: [Gf; 9] = std::array::from_fn(|i| {
                let p = coefficients[i / 3];
                gf(match i % 3 {
                    0 => p[0],
                    1 => p[0] + p[1] + p[2],
                    _ => p[2],
                })
            });
            let mut lg: Vec<_> = l.iter().copied().map(gf).collect();
            let mut rg: Vec<_> = r.iter().copied().map(gf).collect();
            let wg: Vec<_> = w.iter().copied().map(gf).collect();
            let pg = point.map(gf);
            assert_eq!(kernel(&mut lg, &mut rg, &pg, &wg), expected);
            assert_eq!(lg[..4 * n].iter().copied().map(f).collect::<Vec<_>>(), left);
            assert_eq!(
                rg[..4 * n].iter().copied().map(f).collect::<Vec<_>>(),
                right
            );
            lg = l.iter().copied().map(gf).collect();
            rg = r.iter().copied().map(gf).collect();
            assert_eq!(
                crate::production_grid::evaluate(&mut lg, &mut rg, &pg, &wg, n).unwrap(),
                expected
            );
            assert_eq!(lg[..4 * n].iter().copied().map(f).collect::<Vec<_>>(), left);
            assert_eq!(
                rg[..4 * n].iter().copied().map(f).collect::<Vec<_>>(),
                right
            );
        }
    }
}

#[test]
fn chunked_grid_checks_nine_values_and_compacted_prefixes() {
    hook_cases(0, &mut Rng(1));
}

type PolyKernel<
    const AB: usize,
    const A: usize,
    const BB: usize,
    const B: usize,
    const OB: usize,
    const O: usize,
> = fn(&[Poly<AB, A>], &[Poly<BB, B>]) -> Poly<OB, O>;

pub(super) fn check_polynomial<
    const AB: usize,
    const A: usize,
    const BB: usize,
    const B: usize,
    const OB: usize,
    const O: usize,
>(
    kernels: &[PolyKernel<AB, A, BB, B, OB, O>],
) {
    let mut rng = Rng(0x706f_6c79_6e6f_6d69);
    for n in [0, 1, 2, 3, 4, 5, 15, 16, 17, 31, 32, 33] {
        for pattern in 0..8 {
            let mut values = |width: usize| {
                (0..n)
                    .map(|i| {
                        (0..width)
                            .map(|j| match pattern {
                                0 => 0,
                                1 => u64::MAX,
                                2 => {
                                    if i < 4 {
                                        0
                                    } else {
                                        rng.next()
                                    }
                                }
                                3 => {
                                    if i >= 4 {
                                        0
                                    } else {
                                        rng.next()
                                    }
                                }
                                4 => {
                                    if (i + j) % 2 == 0 {
                                        1 << 63
                                    } else {
                                        0
                                    }
                                }
                                // Dense prefixes followed by a partially zero
                                // transition row, including the final row.
                                6 | 7 => {
                                    let split = if pattern == 6 { 4 } else { n - 1 };
                                    if i >= split && j == 0 {
                                        0
                                    } else {
                                        rng.next() | 1
                                    }
                                }
                                _ => rng.next(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            };
            let a: Vec<_> = values(A)
                .into_iter()
                .map(|w| Poly::<AB, A>::from_polynomial_words(w.try_into().unwrap()))
                .collect();
            let b: Vec<_> = values(B)
                .into_iter()
                .map(|w| Poly::<BB, B>::from_polynomial_words(w.try_into().unwrap()))
                .collect();
            let mut expected = vec![0; O];
            for (a, b) in a.iter().zip(&b) {
                for (out, p) in
                    expected
                        .iter_mut()
                        .zip(oracle::polynomial(a.as_words(), b.as_words(), O))
                {
                    *out ^= p;
                }
            }
            for kernel in kernels {
                assert_eq!(kernel(&a, &b).as_words().as_slice(), expected);
            }
            let mut aa = a.clone();
            aa.extend_from_slice(&a);
            let mut bb = b.clone();
            bb.extend_from_slice(&b);
            for kernel in kernels {
                assert_eq!(kernel(&aa, &bb).as_words(), &[0; O]);
            }
        }
    }
    // Every limb crossing and the highest representable monomials.
    for i in (0..A * 64).filter(|i| i % 64 == 0 || i % 64 == 63) {
        for j in (0..B * 64).filter(|j| j % 64 == 0 || j % 64 == 63) {
            let a = [Poly::<AB, A>::from_polynomial_words(std::array::from_fn(
                |w| {
                    if w == i / 64 { 1 << (i % 64) } else { 0 }
                },
            ))];
            let b = [Poly::<BB, B>::from_polynomial_words(std::array::from_fn(
                |w| {
                    if w == j / 64 { 1 << (j % 64) } else { 0 }
                },
            ))];
            let expected: [u64; O] = std::array::from_fn(|w| {
                if w == (i + j) / 64 {
                    1 << ((i + j) % 64)
                } else {
                    0
                }
            });
            for kernel in kernels {
                assert_eq!(kernel(&a, &b).as_words(), &expected);
            }
        }
    }
}

#[test]
fn all_polynomial_kernels_match_bit_convolution() {
    let mut rng = Rng(1);
    polynomial::<64, 1, 64, 1, 128, 2>(0, &mut rng);
    polynomial::<192, 3, 448, 7, 640, 10>(0, &mut rng);
    polynomial::<576, 9, 576, 9, 1152, 18>(0, &mut rng);
    polynomial::<192, 3, 448, 7, 704, 11>(0, &mut rng);
}

fn b127_reference(mut a: u128, mut b: u128) -> u128 {
    let mask = (1u128 << 127) - 1;
    let mut out = 0;
    for _ in 0..127 {
        if b & 1 != 0 {
            out ^= a;
        }
        b >>= 1;
        let top = a >> 126;
        a = ((a << 1) & mask) ^ (3 * top);
    }
    out
}

#[test]
fn b127_multiplication_basis_and_dense_values() {
    let mut rng = Rng(0x6231_3237_5f6d_756c);
    let check = |a: u128, b: u128| {
        let left = B127::from_polynomial_words([a as u64, (a >> 64) as u64]);
        let right = B127::from_polynomial_words([b as u64, (b >> 64) as u64]);
        let expected = b127_reference(a, b);
        let mut results = vec![left * right];
        #[cfg(target_arch = "aarch64")]
        {
            results.push(left.mul_karatsuba(right));
            results.push(left.mul_pfold(right));
        }
        for r in results {
            assert_eq!(r.as_words(), &[expected as u64, (expected >> 64) as u64]);
        }
    };
    for i in 0..127 {
        for j in 0..127 {
            check(1u128 << i, 1u128 << j);
        }
    }
    for _ in 0..1024 {
        check(
            rng.next() as u128 | ((rng.next() & (u64::MAX >> 1)) as u128) << 64,
            rng.next() as u128 | ((rng.next() & (u64::MAX >> 1)) as u128) << 64,
        );
    }
}
