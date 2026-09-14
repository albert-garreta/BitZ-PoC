//! Native-layout scalar GF128 metrics, with the actual F2Z implementation as
//! baseline. Input conversion and output allocation happen before timing.
//! These loops measure independent scalar operations; they are not fused batch
//! inversion, delayed MAC, or a proposed-library speedup claim.

use crate::{Case, Rng, arithmetic, case_requested, measure};
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as F2z;
use flock_core::field::F128 as Flock;
use num_traits::Inv;
use std::hint::black_box;

/// Benchmark-only monomorphization over each implementation's native storage.
trait Native: Copy {
    fn from_flock(value: Flock) -> Self;
    fn to_flock(self) -> Flock;
    fn add(self, rhs: Self) -> Self;
    fn mul(self, rhs: Self) -> Self;
    fn square(self) -> Self;
    fn inverse(self) -> Self;
}

impl Native for F2z {
    fn from_flock(value: Flock) -> Self {
        Self::from_words([value.lo, value.hi])
    }
    fn to_flock(self) -> Flock {
        Flock::new(self.words()[0], self.words()[1])
    }
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        self + rhs
    }
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        self * rhs
    }
    #[inline(always)]
    fn square(self) -> Self {
        F2z::square(&self)
    }
    #[inline(always)]
    fn inverse(self) -> Self {
        F2z::inverse(&self)
    }
}

impl Native for Flock {
    fn from_flock(value: Flock) -> Self {
        value
    }
    fn to_flock(self) -> Flock {
        self
    }
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        self + rhs
    }
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        self * rhs
    }
    #[inline(always)]
    fn square(self) -> Self {
        Flock::square(self)
    }
    #[inline(always)]
    fn inverse(self) -> Self {
        self.inv()
    }
}

impl Native for field::F128 {
    fn from_flock(value: Flock) -> Self {
        Self::new(value.lo, value.hi)
    }
    fn to_flock(self) -> Flock {
        Flock::new(self.lo, self.hi)
    }
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        self + rhs
    }
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        self * rhs
    }
    #[inline(always)]
    fn square(self) -> Self {
        field::F128::square(self)
    }
    #[inline(always)]
    fn inverse(self) -> Self {
        self.inv().expect("nonzero benchmark input")
    }
}

#[inline(never)]
fn binary<F: Native, const MULTIPLY: bool>(a: &[F], b: &[F], out: &mut [F]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = if MULTIPLY { a.mul(b) } else { a.add(b) };
    }
}

#[inline(never)]
fn unary<F: Native, const INVERSE: bool>(a: &[F], out: &mut [F]) {
    assert_eq!(a.len(), out.len());
    for (&a, out) in a.iter().zip(out) {
        *out = if INVERSE { a.inverse() } else { a.square() };
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
#[inline(never)]
fn scalar_lanes(a: &[field::F128], b: &[field::F128], out: &mut [field::F128]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = crate::scalar_kernel::mul(a, b);
    }
}

fn assert_output<F: Native>(output: &[F], expected: &[Flock]) {
    assert_eq!(output.len(), expected.len());
    for (&actual, &expected) in output.iter().zip(expected) {
        assert_eq!(actual.to_flock(), expected);
    }
}

fn binary_case<'a, F: Native + 'a, const MULTIPLY: bool>(
    name: &'static str,
    a: &'a [F],
    b: &'a [F],
    expected: &[Flock],
) -> Case<'a> {
    let mut output = vec![F::from_flock(Flock::new(u64::MAX, u64::MAX)); a.len()];
    binary::<F, MULTIPLY>(a, b, &mut output);
    assert_output(&output, expected);
    Case::new(name, a.len() * 48, move || {
        binary::<F, MULTIPLY>(black_box(a), black_box(b), black_box(&mut output));
        black_box(&output);
    })
}

fn unary_case<'a, F: Native + 'a, const INVERSE: bool>(
    name: &'static str,
    a: &'a [F],
    expected: &[Flock],
) -> Case<'a> {
    let mut output = vec![F::from_flock(Flock::new(u64::MAX, u64::MAX)); a.len()];
    unary::<F, INVERSE>(a, &mut output);
    assert_output(&output, expected);
    Case::new(name, a.len() * 32, move || {
        unary::<F, INVERSE>(black_box(a), black_box(&mut output));
        black_box(&output);
    })
}

fn verify_native<F: Native>(a: &[Flock], b: &[Flock]) {
    let native_a: Vec<_> = a.iter().copied().map(F::from_flock).collect();
    let native_b: Vec<_> = b.iter().copied().map(F::from_flock).collect();
    let mut output = vec![F::from_flock(Flock::new(u64::MAX, u64::MAX)); a.len()];
    let expected_add: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(a, b)| Flock::new(a.lo ^ b.lo, a.hi ^ b.hi))
        .collect();
    binary::<F, false>(&native_a, &native_b, &mut output);
    assert_output(&output, &expected_add);
    let expected_mul: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(&a, &b)| arithmetic::oracle(a, b))
        .collect();
    binary::<F, true>(&native_a, &native_b, &mut output);
    assert_output(&output, &expected_mul);
    let expected_square: Vec<_> = a.iter().map(|&a| arithmetic::oracle(a, a)).collect();
    unary::<F, false>(&native_a, &mut output);
    assert_output(&output, &expected_square);
    for &a in &native_a {
        if a.to_flock() != Flock::ZERO {
            assert_eq!(
                arithmetic::oracle(a.to_flock(), a.inverse().to_flock()),
                Flock::ONE
            );
        }
    }
}

fn verify() {
    // Include empty/ragged slices, basis bits, reduction edges, and full words.
    let mut values = vec![
        Flock::ZERO,
        Flock::ONE,
        Flock::new(0x87, 0),
        Flock::new(u64::MAX, u64::MAX),
        Flock::new(u64::MAX, 0),
        Flock::new(0, u64::MAX),
        Flock::new(0xaaaa_aaaa_aaaa_aaaa, 0x5555_5555_5555_5555),
    ];
    values.extend((0..128).map(|bit| arithmetic::from_u128(1u128 << bit)));
    for n in [0, 1, 2, 3, 7, 17, 135] {
        let a = &values[..n];
        let b: Vec<_> = a.iter().copied().rev().collect();
        verify_native::<F2z>(a, &b);
        verify_native::<Flock>(a, &b);
        verify_native::<field::F128>(a, &b);
        // Flock exposes addition but no subtraction operator. The actual
        // subtraction APIs available in F2Z/shared implement the same XOR.
        for (&a, &b) in a.iter().zip(&b) {
            let expected = Flock::new(a.lo ^ b.lo, a.hi ^ b.hi);
            assert_eq!(
                (F2z::from_flock(a) - F2z::from_flock(b)).to_flock(),
                expected
            );
            assert_eq!(
                (field::F128::from_flock(a) - field::F128::from_flock(b)).to_flock(),
                expected
            );
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
        {
            let native_a: Vec<_> = a.iter().copied().map(field::F128::from_flock).collect();
            let native_b: Vec<_> = b.iter().copied().map(field::F128::from_flock).collect();
            let expected: Vec<_> = a
                .iter()
                .zip(&b)
                .map(|(&a, &b)| arithmetic::oracle(a, b))
                .collect();
            let mut output = vec![field::F128::new(u64::MAX, u64::MAX); n];
            scalar_lanes(&native_a, &native_b, &mut output);
            assert_output(&output, &expected);
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    verify();
    for n in [16, 1024] {
        let size = n.to_string();
        for family in [
            "metrics_gf_mul",
            "metrics_gf_add",
            "metrics_gf_square",
            "metrics_gf_inverse",
        ] {
            if !case_requested(family, &size) {
                continue;
            }
            let a = rng.values(n);
            let b = rng.values(n);
            let fa: Vec<_> = a.iter().copied().map(F2z::from_flock).collect();
            let fb: Vec<_> = b.iter().copied().map(F2z::from_flock).collect();
            let sa: Vec<_> = a.iter().copied().map(field::F128::from_flock).collect();
            let sb: Vec<_> = b.iter().copied().map(field::F128::from_flock).collect();
            let mut cases;
            match family {
                "metrics_gf_mul" => {
                    let expected: Vec<_> = a
                        .iter()
                        .zip(&b)
                        .map(|(&a, &b)| arithmetic::oracle(a, b))
                        .collect();
                    cases = vec![
                        binary_case::<F2z, true>("f2z", &fa, &fb, &expected),
                        binary_case::<Flock, true>("flock", &a, &b, &expected),
                        binary_case::<field::F128, true>("shared", &sa, &sb, &expected),
                    ];
                    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
                    {
                        let (sa, sb) = (&sa, &sb);
                        let mut output = vec![field::F128::new(u64::MAX, u64::MAX); n];
                        scalar_lanes(sa, sb, &mut output);
                        assert_output(&output, &expected);
                        cases.push(Case::new("scalar_lanes", n * 48, move || {
                            scalar_lanes(black_box(sa), black_box(sb), black_box(&mut output));
                            black_box(&output);
                        }));
                    }
                }
                "metrics_gf_add" => {
                    let expected: Vec<_> = a
                        .iter()
                        .zip(&b)
                        .map(|(a, b)| Flock::new(a.lo ^ b.lo, a.hi ^ b.hi))
                        .collect();
                    cases = vec![
                        binary_case::<F2z, false>("f2z", &fa, &fb, &expected),
                        binary_case::<Flock, false>("flock", &a, &b, &expected),
                        binary_case::<field::F128, false>("shared", &sa, &sb, &expected),
                    ];
                }
                "metrics_gf_square" => {
                    let expected: Vec<_> = a.iter().map(|&a| arithmetic::oracle(a, a)).collect();
                    cases = vec![
                        unary_case::<F2z, false>("f2z", &fa, &expected),
                        unary_case::<Flock, false>("flock", &a, &expected),
                        unary_case::<field::F128, false>("shared", &sa, &expected),
                    ];
                }
                "metrics_gf_inverse" => {
                    // Existing APIs differ at zero: F2Z panics, shared returns
                    // None, Flock returns zero. Compare the common nonzero domain
                    // without changing any implementation's public API checks.
                    assert!(a.iter().all(|&a| a != Flock::ZERO));
                    let expected: Vec<_> = a.iter().map(|a| a.inv()).collect();
                    for (&a, &inverse) in a.iter().zip(&expected) {
                        assert_eq!(arithmetic::oracle(a, inverse), Flock::ONE);
                    }
                    cases = vec![
                        unary_case::<F2z, true>("f2z", &fa, &expected),
                        unary_case::<Flock, true>("flock", &a, &expected),
                        unary_case::<field::F128, true>("shared", &sa, &expected),
                    ];
                }
                _ => unreachable!(),
            }
            measure(family, &size, &mut cases, samples, rng);
        }
    }
    eprintln!("correctness: native GF128 multiply/square/inverse/XOR and boundary slices passed");
}
