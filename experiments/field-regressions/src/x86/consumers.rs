//! Differential consumer benchmarks; reset cost is included identically.
use super::*;
type Coeff = (Gf, Gf, Gf);
fn add(a: Coeff, b: Coeff) -> Coeff {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}

#[inline(never)]
pub(super) fn round<const K: usize>(l: &[Gf], r: &[Gf], w: &[Gf]) -> Coeff {
    let mut acc = [F256Unreduced::ZERO; 3];
    for ((l, r), &w) in l.chunks_exact(2).zip(r.chunks_exact(2)).zip(w) {
        let l0 = scalar::<K>(flock(w), flock(l[0]));
        let l1 = scalar::<K>(flock(w), flock(l[1]));
        let c0 = l0.mul_unreduced(flock(r[0]));
        let c2 = (l1 + l0).mul_unreduced(flock(r[1]) + flock(r[0]));
        acc[0] ^= c0;
        acc[1] ^= l1.mul_unreduced(flock(r[1])) ^ c0 ^ c2;
        acc[2] ^= c2;
    }
    (
        gf(acc[0].reduce()),
        gf(acc[1].reduce()),
        gf(acc[2].reduce()),
    )
}
#[inline(always)]
fn round_variant<const K: usize, const TWO: bool>(l: &[Gf], r: &[Gf], w: &[Gf]) -> Coeff {
    let n = w.len();
    if K == 6 {
        return super::rounds::round::<TWO>(l, r, w);
    }
    if K == 0 {
        if TWO {
            return Gf::eqf_two_pair_round(
                &l[..2 * n],
                &r[..2 * n],
                &l[2 * n..],
                &r[2 * n..],
                w,
                n,
            )
            .unwrap();
        }
        return Gf::eqf_single_pair_round(l, r, w, n).unwrap();
    }
    let one = round::<K>(&l[..2 * n], &r[..2 * n], w);
    if TWO {
        add(one, round::<K>(&l[2 * n..], &r[2 * n..], w))
    } else {
        one
    }
}

fn reference(l: &[Gf], r: &[Gf], w: &[Gf]) -> Coeff {
    let mut s = (Gf::zero(), Gf::zero(), Gf::zero());
    for ((l, r), &w) in l.chunks_exact(2).zip(r.chunks_exact(2)).zip(w) {
        let c0 = w * l[0] * r[0];
        let c2 = w * (l[1] - l[0]) * (r[1] - r[0]);
        s = add(s, (c0, w * l[1] * r[1] - c0 - c2, c2));
    }
    s
}
#[inline(never)]
fn fold(v: &mut [Gf], rho: Gf, n: usize) {
    let prep = Prepared::new(flock(rho));
    for i in 0..n {
        v[i] = v[2 * i] + gf(prep.mul(flock(v[2 * i + 1] - v[2 * i])));
    }
}
#[inline(never)]
fn fold_round(l: &mut [Gf], r: &mut [Gf], rho: Gf, w: &[Gf]) -> Coeff {
    let prep = Prepared::new(flock(rho));
    let mut acc = [F256Unreduced::ZERO; 3];
    for (i, &w) in w.iter().enumerate() {
        let f = |a: Gf, b: Gf| a + gf(prep.mul(flock(b - a)));
        let l0 = f(l[4 * i], l[4 * i + 1]);
        let l1 = f(l[4 * i + 2], l[4 * i + 3]);
        let r0 = f(r[4 * i], r[4 * i + 1]);
        let r1 = f(r[4 * i + 2], r[4 * i + 3]);
        l[2 * i] = l0;
        l[2 * i + 1] = l1;
        r[2 * i] = r0;
        r[2 * i + 1] = r1;
        let l0 = scalar::<3>(flock(w), flock(l0));
        let l1 = scalar::<3>(flock(w), flock(l1));
        let c0 = l0.mul_unreduced(flock(r0));
        let c2 = (l1 + l0).mul_unreduced(flock(r1 - r0));
        acc[0] ^= c0;
        acc[1] ^= l1.mul_unreduced(flock(r1)) ^ c0 ^ c2;
        acc[2] ^= c2;
    }
    (
        gf(acc[0].reduce()),
        gf(acc[1].reduce()),
        gf(acc[2].reduce()),
    )
}

pub fn run(samples: usize, rng: &mut Rng) {
    // Check SIMD boundaries, scalar tails, zero coefficients and full-width
    // inputs independently of the chosen timing manifest.
    for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 33, 1025] {
        for pattern in 0..3 {
            let mut values = |len| {
                (0..len)
                    .map(|_| match pattern {
                        0 => Gf::zero(),
                        1 => Gf::from_words([u64::MAX; 2]),
                        _ => gf(F128::new(rng.next(), rng.next())),
                    })
                    .collect::<Vec<_>>()
            };
            let (l, r, w) = (values(4 * n), values(4 * n), values(n));
            let one = reference(&l[..2 * n], &r[..2 * n], &w);
            let two = add(one, reference(&l[2 * n..], &r[2 * n..], &w));
            assert_eq!(super::rounds::round::<false>(&l, &r, &w), one);
            assert_eq!(super::rounds::round::<true>(&l, &r, &w), two);
        }
    }
    for n in [
        1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 1023, 1024, 1025, 65536, 1048576,
    ] {
        let size = n.to_string();
        if ![
            "gf_fold",
            "gf_round_single",
            "gf_round_two",
            "gf_fold_round",
        ]
        .iter()
        .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let l: Vec<_> = rng.values(4 * n).into_iter().map(gf).collect();
        let r: Vec<_> = rng.values(4 * n).into_iter().map(gf).collect();
        let w: Vec<_> = rng.values(n).into_iter().map(gf).collect();
        let rho = gf(F128::new(rng.next(), rng.next()));
        for family in ["gf_round_single", "gf_round_two"] {
            if !case_requested(family, &size) {
                continue;
            }
            let want = if family == "gf_round_single" {
                reference(&l[..2 * n], &r[..2 * n], &w)
            } else {
                add(
                    reference(&l[..2 * n], &r[..2 * n], &w),
                    reference(&l[2 * n..], &r[2 * n..], &w),
                )
            };
            let mut cases = Vec::new();
            // Each timed closure has a monomorphized operation. Benchmark
            // name/family comparisons are construction work, not arithmetic.
            macro_rules! variant {
                ($name:literal, $k:literal, $two:literal) => {{
                    let (l, r, w) = (&l, &r, &w);
                    let apply =
                        move || round_variant::<$k, $two>(black_box(l), black_box(r), black_box(w));
                    assert_eq!(apply(), want);
                    cases.push(Case::new(
                        $name,
                        n * if $two { 144 } else { 80 },
                        move || {
                            black_box(apply());
                        },
                    ));
                }};
            }
            if family == "gf_round_single" {
                variant!("production", 0, false);
                variant!("barrett", 3, false);
                variant!("binius", 4, false);
                variant!("native", 6, false);
            } else {
                variant!("production", 0, true);
                variant!("barrett", 3, true);
                variant!("binius", 4, true);
                variant!("native", 6, true);
            }
            measure(family, &size, &mut cases, samples, rng);
        }
        if case_requested("gf_fold", &size) {
            let mut want = l.clone();
            for i in 0..n {
                want[i] = l[2 * i] + rho * (l[2 * i + 1] - l[2 * i]);
            }
            let mut cases = Vec::new();
            for name in ["production", "prepared"] {
                let input = &l;
                let mut out = l.clone();
                let apply = move |out: &mut [Gf]| {
                    out.copy_from_slice(black_box(input));
                    if name == "production" {
                        assert!(Gf::eqf_fold_in_place(out, black_box(&rho), n));
                    } else {
                        fold(out, black_box(rho), n);
                    }
                };
                apply(&mut out);
                assert_eq!(out, want);
                cases.push(Case::new(name, n * 128, move || {
                    apply(black_box(&mut out));
                    black_box(&out);
                }));
            }
            measure("gf_fold", &size, &mut cases, samples, rng);
        }
        if case_requested("gf_fold_round", &size) {
            let mut wl = l.clone();
            let mut wr = r.clone();
            assert!(Gf::eqf_fold_in_place(&mut wl, &rho, 2 * n));
            assert!(Gf::eqf_fold_in_place(&mut wr, &rho, 2 * n));
            let want = reference(&wl[..2 * n], &wr[..2 * n], &w);
            let mut cases = Vec::new();
            for name in ["production", "two_pass", "prepared"] {
                let (l, r, w) = (&l, &r, &w);
                let mut ol = l.clone();
                let mut or = r.clone();
                let apply = move |ol: &mut [Gf], or: &mut [Gf]| {
                    ol.copy_from_slice(black_box(l));
                    or.copy_from_slice(black_box(r));
                    match name {
                        "production" => {
                            Gf::eqf_fused_fold_round(ol, or, black_box(&rho), black_box(w), n)
                                .unwrap()
                        }
                        "two_pass" => {
                            assert!(Gf::eqf_fold_in_place(ol, &rho, 2 * n));
                            assert!(Gf::eqf_fold_in_place(or, &rho, 2 * n));
                            Gf::eqf_single_pair_round(ol, or, w, n).unwrap()
                        }
                        _ => fold_round(ol, or, black_box(rho), black_box(w)),
                    }
                };
                assert_eq!(apply(&mut ol, &mut or), want);
                assert_eq!(ol, wl);
                assert_eq!(or, wr);
                cases.push(Case::new(name, n * 272, move || {
                    black_box(apply(black_box(&mut ol), black_box(&mut or)));
                    black_box((&ol, &or));
                }));
            }
            measure("gf_fold_round", &size, &mut cases, samples, rng);
        }
    }
    for log in [8, 16, 20] {
        let n = 1 << log;
        let size = format!("log{log}");
        if !case_requested("gf_fold_cascade", &size) {
            continue;
        }
        let input: Vec<_> = rng.values(n).into_iter().map(gf).collect();
        let rho = gf(F128::new(rng.next(), rng.next()));
        let mut cases = Vec::new();
        let mut expected = None;
        for name in ["production", "prepared", "reset_only"] {
            let input = &input;
            let mut out = input.clone();
            let apply = move |out: &mut [Gf]| {
                out.copy_from_slice(black_box(input));
                if name != "reset_only" {
                    let mut half = n / 2;
                    while half > 0 {
                        if name == "production" {
                            assert!(Gf::eqf_fold_in_place(out, &rho, half));
                        } else {
                            fold(out, rho, half);
                        }
                        half /= 2;
                    }
                }
                out[0]
            };
            let result = apply(&mut out);
            if name != "reset_only" {
                if let Some(expected) = expected {
                    assert_eq!(result, expected);
                } else {
                    expected = Some(result);
                }
            }
            cases.push(Case::new(name, n * 32, move || {
                black_box(apply(black_box(&mut out)));
                black_box(&out);
            }));
        }
        measure("gf_fold_cascade", &size, &mut cases, samples, rng);
    }
}
