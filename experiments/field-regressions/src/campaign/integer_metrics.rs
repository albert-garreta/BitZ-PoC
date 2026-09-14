//! Existing integer implementation costs. No proposed owned replacement is timed.
use crate::{Case, Rng, case_requested, measure};
use circuit::witgen::Z;
use crypto_bigint::{NonZero, Uint as RawUint};
use crypto_primitives::{crypto_bigint_int::Int, crypto_bigint_uint::Uint};
use num_bigint::{BigInt, BigUint};
use num_traits::{CheckedAdd, CheckedMul, CheckedSub, One};
use std::{array, cmp::Ordering, hint::black_box, mem::size_of};

fn unsigned(words: &[u64]) -> BigUint {
    BigUint::from_bytes_le(
        &words
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}
fn signed(words: &[u64]) -> BigInt {
    BigInt::from_signed_bytes_le(
        &words
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

#[inline(never)]
fn wrapping<const L: usize, const SUB: bool>(a: &[Z<L>], b: &[Z<L>], out: &mut [Z<L>]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = if SUB { a - b } else { a + b };
    }
}
macro_rules! checked_kernel {
    ($name:ident, $ty:ident) => {
        #[inline(never)]
        fn $name<const L: usize, const OP: usize>(
            a: &[$ty<L>],
            b: &[$ty<L>],
            out: &mut [Option<$ty<L>>],
        ) {
            assert_eq!(a.len(), b.len());
            assert_eq!(a.len(), out.len());
            for ((a, b), out) in a.iter().zip(b).zip(out) {
                *out = match OP {
                    0 => a.checked_add(b),
                    1 => a.checked_sub(b),
                    2 => a.checked_mul(b),
                    _ => unreachable!(),
                };
            }
        }
    };
}
checked_kernel!(checked_unsigned, Uint);
checked_kernel!(checked_signed, Int);

#[inline(never)]
fn compare<const L: usize>(a: &[Uint<L>], b: &[Uint<L>], out: &mut [Ordering]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        *out = a.cmp(b);
    }
}

#[inline(never)]
fn wide<const L: usize>(a: &[RawUint<L>], b: &[RawUint<L>], out: &mut [(RawUint<L>, RawUint<L>)]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        *out = a.widening_mul(b);
    }
}

#[inline(never)]
fn divrem<const L: usize>(
    a: &[RawUint<L>],
    divisor: &NonZero<RawUint<L>>,
    out: &mut [(RawUint<L>, RawUint<L>)],
) {
    assert_eq!(a.len(), out.len());
    for (a, out) in a.iter().zip(out) {
        *out = a.div_rem(divisor);
    }
}

fn run_width<const L: usize>(samples: usize, rng: &mut Rng) {
    for n in [16, 1024] {
        let size = format!("l{L}_n{n}");
        let pairs: Vec<([u64; L], [u64; L])> = (0..n)
            .map(|i| {
                let mut a = array::from_fn(|_| rng.next());
                let mut b = array::from_fn(|_| rng.next());
                match i % 8 {
                    0 => {
                        a = [0; L];
                        b = [0; L];
                    }
                    1 => {
                        a = [u64::MAX; L];
                        b = [0; L];
                        b[0] = 1;
                    }
                    2 => {
                        a = [0; L];
                        a[0] = 1;
                        b = [u64::MAX; L];
                    }
                    3 => {
                        a = [0; L];
                        b = [0; L];
                        a[0] = rng.next() & 65535;
                        b[0] = rng.next() & 65535;
                    }
                    4 => {
                        a = [0; L];
                        a[L - 1] = 1 << 63;
                        b = [u64::MAX; L];
                    }
                    5 => {
                        a = [u64::MAX; L];
                        a[L - 1] >>= 1;
                        b = [0; L];
                        b[0] = 1;
                    }
                    _ => {}
                }
                (a, b)
            })
            .collect();
        let za: Vec<_> = pairs
            .iter()
            .map(|(a, _)| Z::<L>::from_le_words(a))
            .collect();
        let zb: Vec<_> = pairs
            .iter()
            .map(|(_, b)| Z::<L>::from_le_words(b))
            .collect();
        let ua: Vec<_> = pairs
            .iter()
            .map(|(a, _)| Uint::<L>::from_words(*a))
            .collect();
        let ub: Vec<_> = pairs
            .iter()
            .map(|(_, b)| Uint::<L>::from_words(*b))
            .collect();
        let ia: Vec<_> = pairs
            .iter()
            .map(|(a, _)| Int::<L>::from_words(*a))
            .collect();
        let ib: Vec<_> = pairs
            .iter()
            .map(|(_, b)| Int::<L>::from_words(*b))
            .collect();
        let ra: Vec<_> = pairs
            .iter()
            .map(|(a, _)| RawUint::<L>::from_words(*a))
            .collect();
        let rb: Vec<_> = pairs
            .iter()
            .map(|(_, b)| RawUint::<L>::from_words(*b))
            .collect();
        let modulus = BigUint::one() << (64 * L);
        let signed_limit = BigInt::one() << (64 * L - 1);
        macro_rules! wrap_case {
            ($sub:literal,$family:literal) => {
                if case_requested($family, &size) {
                    let mut out = vec![Z::<L>::default(); n];
                    wrapping::<L, $sub>(&za, &zb, &mut out);
                    for ((a, b), got) in pairs.iter().zip(&out) {
                        let (a, b) = (unsigned(a), unsigned(b));
                        let want = if $sub {
                            (a + &modulus - b) % &modulus
                        } else {
                            (a + b) % &modulus
                        };
                        assert_eq!(unsigned(got.words()), want);
                    }
                    let mut cases = [Case::new("circuit_z", n * 3 * size_of::<Z<L>>(), || {
                        wrapping::<L, $sub>(black_box(&za), black_box(&zb), black_box(&mut out));
                        black_box(&out);
                    })];
                    measure($family, &size, &mut cases, samples, rng);
                }
            };
        }
        wrap_case!(false, "metrics_integer_add");
        wrap_case!(true, "metrics_integer_sub");
        macro_rules! check_case {
            ($op:literal,$ufamily:literal,$ifamily:literal) => {
                if case_requested($ufamily, &size) {
                    let mut out = vec![None; n];
                    checked_unsigned::<L, $op>(&ua, &ub, &mut out);
                    for ((a, b), got) in pairs.iter().zip(&out) {
                        let (a, b) = (unsigned(a), unsigned(b));
                        let want = match $op {
                            0 => Some(a + b),
                            1 => {
                                if a >= b {
                                    Some(a - b)
                                } else {
                                    None
                                }
                            }
                            2 => Some(a * b),
                            _ => unreachable!(),
                        }
                        .filter(|x| x < &modulus);
                        assert_eq!(got.as_ref().map(|x| unsigned(x.as_words())), want);
                    }
                    let mut cases = [Case::new(
                        "existing",
                        n * (2 * size_of::<Uint<L>>() + size_of::<Option<Uint<L>>>()),
                        || {
                            checked_unsigned::<L, $op>(
                                black_box(&ua),
                                black_box(&ub),
                                black_box(&mut out),
                            );
                            black_box(&out);
                        },
                    )];
                    measure($ufamily, &size, &mut cases, samples, rng);
                }
                if case_requested($ifamily, &size) {
                    let mut out = vec![None; n];
                    checked_signed::<L, $op>(&ia, &ib, &mut out);
                    for ((a, b), got) in pairs.iter().zip(&out) {
                        let (a, b) = (signed(a), signed(b));
                        let result = match $op {
                            0 => a + b,
                            1 => a - b,
                            2 => a * b,
                            _ => unreachable!(),
                        };
                        let want = if result >= -&signed_limit && result < signed_limit {
                            Some(result)
                        } else {
                            None
                        };
                        assert_eq!(got.as_ref().map(|x| signed(x.inner().as_words())), want);
                    }
                    let mut cases = [Case::new(
                        "existing",
                        n * (2 * size_of::<Int<L>>() + size_of::<Option<Int<L>>>()),
                        || {
                            checked_signed::<L, $op>(
                                black_box(&ia),
                                black_box(&ib),
                                black_box(&mut out),
                            );
                            black_box(&out);
                        },
                    )];
                    measure($ifamily, &size, &mut cases, samples, rng);
                }
            };
        }
        check_case!(0, "metrics_uint_checked_add", "metrics_int_checked_add");
        check_case!(1, "metrics_uint_checked_sub", "metrics_int_checked_sub");
        check_case!(2, "metrics_uint_checked_mul", "metrics_int_checked_mul");
        if case_requested("metrics_uint_compare", &size) {
            let mut out = vec![Ordering::Equal; n];
            compare(&ua, &ub, &mut out);
            for ((a, b), got) in pairs.iter().zip(&out) {
                assert_eq!(*got, unsigned(a).cmp(&unsigned(b)));
            }
            let mut cases = [Case::new(
                "existing",
                n * (2 * size_of::<Uint<L>>() + size_of::<Ordering>()),
                || {
                    compare(black_box(&ua), black_box(&ub), black_box(&mut out));
                    black_box(&out);
                },
            )];
            measure("metrics_uint_compare", &size, &mut cases, samples, rng);
        }
        if case_requested("metrics_uint_wide_mul", &size) {
            let mut out = vec![(RawUint::<L>::ZERO, RawUint::<L>::ZERO); n];
            wide(&ra, &rb, &mut out);
            for ((a, b), (lo, hi)) in pairs.iter().zip(&out) {
                assert_eq!(
                    unsigned(lo.as_words()) + unsigned(hi.as_words()) * &modulus,
                    unsigned(a) * unsigned(b)
                );
            }
            let mut cases = [Case::new(
                "existing",
                n * 4 * size_of::<RawUint<L>>(),
                || {
                    wide(black_box(&ra), black_box(&rb), black_box(&mut out));
                    black_box(&out);
                },
            )];
            measure("metrics_uint_wide_mul", &size, &mut cases, samples, rng);
        }
        for (kind, words) in [
            (
                "d64",
                array::from_fn(|i| if i == 0 { 0xffff_ffff_ffff_ffc5 } else { 0 }),
            ),
            (
                "dfull",
                array::from_fn(|i| {
                    if i == L - 1 {
                        0x1fff_ffff_ffff_ffff
                    } else {
                        u64::MAX
                    }
                }),
            ),
        ] {
            let size = format!("l{L}_{kind}_n{n}");
            if !case_requested("metrics_uint_divrem", &size) {
                continue;
            }
            let divisor = NonZero::new(RawUint::<L>::from_words(words)).unwrap();
            let d = unsigned(&words);
            let mut out = vec![(RawUint::<L>::ZERO, RawUint::<L>::ZERO); n];
            divrem(&ra, &divisor, &mut out);
            for ((a, _), (q, r)) in pairs.iter().zip(&out) {
                let a = unsigned(a);
                assert_eq!(unsigned(q.as_words()), &a / &d);
                assert_eq!(unsigned(r.as_words()), a % &d);
            }
            let mut cases = [Case::new(
                "existing_prevalidated_divisor",
                n * 3 * size_of::<RawUint<L>>() + size_of::<RawUint<L>>(),
                || {
                    divrem(black_box(&ra), black_box(&divisor), black_box(&mut out));
                    black_box(&out);
                },
            )];
            measure("metrics_uint_divrem", &size, &mut cases, samples, rng);
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    run_width::<2>(samples, rng);
    run_width::<4>(samples, rng);
    run_width::<9>(samples, rng);
}
