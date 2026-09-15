use crate::{Case, Rng, arithmetic::*, case_requested, measure};
use flock_core::field::Gf128;
use num_traits::Inv;
use std::hint::black_box;

#[inline(never)]
fn unary<const SHARED: bool, const INVERSE: bool>(input: &[Gf128], out: &mut [Gf128]) {
    assert_eq!(input.len(), out.len());
    for (&a, b) in input.iter().zip(out) {
        *b = if SHARED {
            let r = if INVERSE {
                shared(a).inv().expect("nonzero benchmark input")
            } else {
                shared(a).square()
            };
            Gf128::new(r.lo, r.hi)
        } else if INVERSE {
            a.inverse_or_zero()
        } else {
            a.square()
        };
    }
}

#[inline(always)]
fn fixed_mul<const K: usize>(a: Gf128, t: Gf128, prepared: Prepared) -> Gf128 {
    match K {
        0 => a * t,
        1 => Shared::mul(a, t),
        2 => ScalarLanes::mul(a, t),
        3 => prepared.mul(a),
        4 => {
            if t == Gf128::ZERO {
                Gf128::ZERO
            } else if t.hi == 0 {
                half_mul(a, t.lo)
            } else {
                prepared.mul(a)
            }
        }
        _ => unreachable!(),
    }
}
#[inline(never)]
fn fixed<const K: usize>(a: &[Gf128], t: Gf128, p: Prepared, out: &mut [Gf128]) {
    for (&a, o) in a.iter().zip(out) {
        *o = fixed_mul::<K>(a, t, p);
    }
}
#[inline(never)]
fn butterfly<const K: usize>(
    a: &[Gf128],
    b: &[Gf128],
    t: Gf128,
    p: Prepared,
    out: &mut [(Gf128, Gf128)],
) {
    for ((&a, &b), o) in a.iter().zip(b).zip(out) {
        let u = a + fixed_mul::<K>(b, t, p);
        *o = (u, b + u);
    }
}

pub fn run(samples: usize, rng: &mut Rng) {
    for inverse in [false, true] {
        let sizes: &[usize] = if inverse {
            &[1, 16, 1024]
        } else {
            &[16, 1024, 65536, 1_048_576]
        };
        let family = if inverse { "inverse" } else { "square" };
        for &n in sizes {
            if !case_requested(family, &n.to_string()) {
                continue;
            }
            let input = rng.values(n);
            assert!(input.iter().all(|a| *a != Gf128::ZERO));
            let expected: Vec<_> = input
                .iter()
                .map(|a| if inverse { a.inverse_or_zero() } else { oracle(*a, *a) })
                .collect();
            if inverse {
                for (a, b) in input.iter().zip(&expected) {
                    assert_eq!(oracle(*a, *b), Gf128::ONE);
                }
            }
            let mut cases = Vec::new();
            for candidate in [false, true] {
                let input = &input;
                let mut out = vec![Gf128::ZERO; n];
                let apply = move |out: &mut [Gf128]| match (candidate, inverse) {
                    (false, false) => unary::<false, false>(black_box(input), out),
                    (true, false) => unary::<true, false>(black_box(input), out),
                    (false, true) => unary::<false, true>(black_box(input), out),
                    (true, true) => unary::<true, true>(black_box(input), out),
                };
                apply(&mut out);
                assert_eq!(out, expected);
                cases.push(Case::new(
                    if candidate { "shared" } else { "flock" },
                    n * 32,
                    move || {
                        apply(black_box(&mut out));
                        black_box(&out);
                    },
                ));
            }
            measure(family, &n.to_string(), &mut cases, samples, rng);
        }
    }
    for (kind, t) in [
        ("zero", Gf128::ZERO),
        ("half", Gf128::new(0xab734216fed91235, 0)),
        ("full", Gf128::new(0xab734216fed91235, 0x9876abc123654fed)),
    ] {
        // Prepared is the existing fixed-multiplier formula copied from F2Z;
        // setup is a diagnostic, not a new algorithm or a promotion candidate.
        let mut setup = vec![Case::new(
            "existing_prepare",
            std::mem::size_of::<Prepared>(),
            || {
                black_box(Prepared::new(black_box(t)));
            },
        )];
        measure("fixed_prepare", kind, &mut setup, samples, rng);
        for n in [16, 1024, 65536, 1_048_576] {
            let size = format!("{kind}_n{n}");
            if !case_requested("fixed", &size) && !case_requested("butterfly", &size) {
                continue;
            }
            let (a, b) = (rng.values(n), rng.values(n));
            let prepared = Prepared::new(t);
            let expected: Vec<_> = a.iter().map(|a| *a * t).collect();
            let expected_b: Vec<_> = a
                .iter()
                .zip(&b)
                .map(|(a, b)| {
                    let u = *a + *b * t;
                    (u, *b + u)
                })
                .collect();
            let mut cases = Vec::new();
            let mut butterflies = Vec::new();
            for k in 0..5 {
                #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
                if k == 2 {
                    continue;
                }
                let (a, b) = (&a, &b);
                let name = ["flock", "shared", "scalar_lanes", "prepared", "specialized"][k];
                let mut out = vec![Gf128::ZERO; n];
                let mut bout = vec![(Gf128::ZERO, Gf128::ZERO); n];
                let apply = move |out: &mut [Gf128]| match k {
                    0 => fixed::<0>(black_box(a), black_box(t), black_box(prepared), out),
                    1 => fixed::<1>(black_box(a), black_box(t), black_box(prepared), out),
                    2 => fixed::<2>(black_box(a), black_box(t), black_box(prepared), out),
                    3 => fixed::<3>(black_box(a), black_box(t), black_box(prepared), out),
                    4 => fixed::<4>(black_box(a), black_box(t), black_box(prepared), out),
                    _ => unreachable!(),
                };
                let bapply = move |out: &mut [(Gf128, Gf128)]| match k {
                    0 => butterfly::<0>(
                        black_box(a),
                        black_box(b),
                        black_box(t),
                        black_box(prepared),
                        out,
                    ),
                    1 => butterfly::<1>(
                        black_box(a),
                        black_box(b),
                        black_box(t),
                        black_box(prepared),
                        out,
                    ),
                    2 => butterfly::<2>(
                        black_box(a),
                        black_box(b),
                        black_box(t),
                        black_box(prepared),
                        out,
                    ),
                    3 => butterfly::<3>(
                        black_box(a),
                        black_box(b),
                        black_box(t),
                        black_box(prepared),
                        out,
                    ),
                    4 => butterfly::<4>(
                        black_box(a),
                        black_box(b),
                        black_box(t),
                        black_box(prepared),
                        out,
                    ),
                    _ => unreachable!(),
                };
                apply(&mut out);
                assert_eq!(out, expected);
                bapply(&mut bout);
                assert_eq!(bout, expected_b);
                cases.push(Case::new(name, n * 32, move || {
                    apply(black_box(&mut out));
                    black_box(&out);
                }));
                butterflies.push(Case::new(name, n * 64, move || {
                    bapply(black_box(&mut bout));
                    black_box(&bout);
                }));
            }
            measure("fixed", &size, &mut cases, samples, rng);
            measure("butterfly", &size, &mut butterflies, samples, rng);
        }
    }
}
