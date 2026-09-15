use super::pass;
use crate::production_projection::project_signed;
use crate::{Case, Rng, baseline_raw::RawMontyCtx as Ctx, case_requested, measure};
use circuit::witgen::Z;
use crypto_bigint::{
    Choice, CtSelect, Limb, NonZero, Odd, Reciprocal, Uint, modular::FixedMontyParams,
};
use field::ModRingCtx;
use num_bigint::{BigInt, BigUint};
use num_traits::Zero;
use std::{array, hint::black_box};

#[cfg(test)]
#[path = "correctness/integer.rs"]
mod tests;

fn big<const L: usize>(a: &[u64; L]) -> BigUint {
    BigUint::from_bytes_le(&a.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>())
}

// Independent product chains keep the previous sum out of each multiplication.
// L and K are public shape parameters. The result is modulo 2^(64L).
#[inline(never)]
fn mac<const L: usize, const K: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(a.len(), b.len());
    let mut sums = [Z::zero(); K];
    let mut aa = a.chunks_exact(K);
    let mut bb = b.chunks_exact(K);
    for (a, b) in aa.by_ref().zip(bb.by_ref()) {
        for j in 0..K {
            sums[j] = sums[j] + a[j] * b[j];
        }
    }
    for (&a, &b) in aa.remainder().iter().zip(bb.remainder()) {
        sums[0] = sums[0] + a * b;
    }
    sums.into_iter().fold(Z::zero(), |a, b| a + b)
}

// Delay carries between columns across the batch. Each column receives at
// most 2L-1 u64 terms per pair. This public length bound proves u128 capacity.
#[inline(never)]
fn columns<const L: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(a.len(), b.len());
    assert!(a.len() <= (u64::MAX / (2 * L) as u64) as usize);
    let mut sums = [0u128; L];
    for (a, b) in a.iter().zip(b) {
        for i in 0..L {
            for j in 0..L - i {
                let p = a.as_words()[i] as u128 * b.as_words()[j] as u128;
                sums[i + j] += (p as u64) as u128;
                if i + j + 1 < L {
                    sums[i + j + 1] += p >> 64;
                }
            }
        }
    }
    let mut carry = 0;
    let mut out = [0; L];
    for i in 0..L {
        let v = sums[i] + carry;
        out[i] = v as u64;
        carry = v >> 64;
    }
    crate::arithmetic::integer_from_words(&out)
}

#[inline(always)]
fn mul4(a: &[u64], b: &[u64]) -> (u128, u128) {
    let p00 = a[0] as u128 * b[0] as u128;
    let p01 = a[0] as u128 * b[1] as u128;
    let p10 = a[1] as u128 * b[0] as u128;
    let middle = (p00 >> 64) + (p01 as u64) as u128 + (p10 as u64) as u128;
    let low = (p00 as u64) as u128 | ((middle as u64) as u128) << 64;
    let high = (a[1] as u128 * b[1] as u128)
        .wrapping_add(p01 >> 64)
        .wrapping_add(p10 >> 64)
        .wrapping_add(middle >> 64);
    let al = a[0] as u128 | ((a[1] as u128) << 64);
    let ah = a[2] as u128 | ((a[3] as u128) << 64);
    let bl = b[0] as u128 | ((b[1] as u128) << 64);
    let bh = b[2] as u128 | ((b[3] as u128) << 64);
    (
        low,
        high.wrapping_add(al.wrapping_mul(bh))
            .wrapping_add(ah.wrapping_mul(bl)),
    )
}
#[inline(never)]
fn native4<const L: usize, const K: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(L, 4);
    assert_eq!(a.len(), b.len());
    let mut sums = [(0u128, 0u128); K];
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        let (lo, hi) = mul4(a.as_words(), b.as_words());
        let slot = &mut sums[i % K];
        let (sum, carry) = slot.0.overflowing_add(lo);
        slot.0 = sum;
        slot.1 = slot.1.wrapping_add(hi).wrapping_add(carry as u128);
    }
    let mut out = (0u128, 0u128);
    for (lo, hi) in sums {
        let (sum, carry) = out.0.overflowing_add(lo);
        out = (sum, out.1.wrapping_add(hi).wrapping_add(carry as u128));
    }
    crate::arithmetic::integer_from_words(&[
        out.0 as u64,
        (out.0 >> 64) as u64,
        out.1 as u64,
        (out.1 >> 64) as u64,
    ])
}

#[inline(never)]
fn native_word<const L: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(L, 1);
    assert_eq!(a.len(), b.len());
    let mut sums = [0u64; 4];
    let mut aa = a.chunks_exact(4);
    let mut bb = b.chunks_exact(4);
    for (a, b) in aa.by_ref().zip(bb.by_ref()) {
        for i in 0..4 {
            sums[i] = sums[i].wrapping_add(a[i].as_words()[0].wrapping_mul(b[i].as_words()[0]));
        }
    }
    let mut sum = sums.into_iter().fold(0u64, u64::wrapping_add);
    for (a, b) in aa.remainder().iter().zip(bb.remainder()) {
        sum = sum.wrapping_add(a.as_words()[0].wrapping_mul(b.as_words()[0]));
    }
    crate::arithmetic::integer_from_words(&[sum])
}

fn mac_cases<const L: usize>(samples: usize, rng: &mut Rng) {
    if L == 4 {
        let edge = [
            [0; 4],
            [u64::MAX; 4],
            [1, 0, 0, 0],
            [0, 0, 0, 1 << 63],
            [u64::MAX, 0, u64::MAX, 0],
        ];
        for a in edge {
            for b in edge {
                let (lo, hi) = mul4(&a, &b);
                let words = [lo as u64, (lo >> 64) as u64, hi as u64, (hi >> 64) as u64];
                assert_eq!(
                    big(&words),
                    (big(&a) * big(&b)) % (BigUint::from(1u8) << 256)
                );
            }
        }
    }
    for class in ["signed16", "full"] {
        for n in [0, 1, 3, 17, 16, 1024, 65536] {
            let size = format!("l{L}_{class}_n{n}");
            if n >= 16 && n != 17 && !case_requested("opt_integer_mac", &size) {
                continue;
            }
            let mut inputs = || {
                (0..n)
                    .map(|_| {
                        if class == "signed16" {
                            Z::<L>::from(rng.next() as i16 as i128)
                        } else {
                            crate::arithmetic::integer_from_words(&array::from_fn::<_, L, _>(
                                |_| rng.next(),
                            ))
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let a = inputs();
            let b = inputs();
            let baseline = super::super::integer::existing(&a, &b);
            let oracle = a.iter().zip(&b).fold(BigUint::zero(), |s, (a, b)| {
                s + big(a.as_words()) * big(b.as_words())
            }) % (BigUint::from(1u8) << (64 * L));
            assert_eq!(big(baseline.as_words()), oracle);
            assert_eq!(mac::<L, 2>(&a, &b), baseline);
            assert_eq!(mac::<L, 4>(&a, &b), baseline);
            assert_eq!(columns(&a, &b), baseline);
            if L == 1 {
                assert_eq!(native_word(&a, &b), baseline);
            }
            if L == 4 {
                assert_eq!(native4::<L, 1>(&a, &b), baseline);
                assert_eq!(native4::<L, 2>(&a, &b), baseline);
            }
            if n < 16 || n == 17 {
                continue;
            }
            let mut cases = vec![Case::new("circuit_z", n * L * 16, || {
                black_box(super::super::integer::existing(
                    black_box(&a),
                    black_box(&b),
                ));
            })];
            macro_rules! add {
                ($name:literal, $f:expr) => {
                    cases.push(Case::new($name, n * L * 16, || {
                        black_box($f(black_box(&a), black_box(&b)));
                    }));
                };
            }
            add!("products2", mac::<L, 2>);
            add!("products4", mac::<L, 4>);
            add!("deferred_columns", columns::<L>);
            // Retain the historical winners and the regressing fused implementation.
            add!("incumbent", super::super::integer::mac::<L, false>);
            if L == 1 {
                add!("native_word", native_word::<L>);
            }
            if L == 4 {
                add!("native128x2", native4::<L, 1>);
                add!("native128x2_acc2", native4::<L, 2>);
            }
            measure("opt_integer_mac", &size, &mut cases, samples, rng);
        }
    }
}

/// Runtime public modulus, fixed source width. q > 2^64 ensures each limb
/// is reduced. Horner works directly in canonical coordinates: multiplying
/// by (2^64 R mod q) cancels the reducer's R^-1 factor.
struct Projection<const L: usize> {
    ctx: Ctx,
    modulus: u128,
    radix: u128,
    signed_correction: u128,
}
impl<const L: usize> Projection<L> {
    fn new(q: u128) -> Self {
        assert!(L > 0 && q > u64::MAX as u128 && q & 1 == 1);
        let cfg = FixedMontyParams::new_vartime(
            Odd::new(Uint::<2>::from_words([q as u64, (q >> 64) as u64])).unwrap(),
        );
        let ctx = Ctx::new(&cfg);
        let radix = ctx.native_residue_u128(1u128 << 64);
        let mut signed_correction = 1;
        for _ in 0..L {
            signed_correction = ctx.mul(signed_correction, radix);
        }
        Self {
            ctx,
            modulus: q,
            radix,
            signed_correction,
        }
    }
    #[inline(always)]
    fn reduce(&self, words: &[u64; L]) -> [u64; 2] {
        let mut acc = words[L - 1] as u128;
        for &limb in words[..L - 1].iter().rev() {
            acc = self.ctx.add(self.ctx.mul(acc, self.radix), limb as u128);
        }
        let mask = 0u128.wrapping_sub((words[L - 1] >> 63) as u128);
        let (difference, borrow) = acc.overflowing_sub(self.signed_correction & mask);
        // Always load and mask the modulus. RawMontyCtx::sub's conditional
        // correction compiled to a secret-dependent branch for this caller.
        let addend = 0u128.ct_select(&self.modulus, Choice::from_u8_lsb(borrow as u8));
        acc = difference.wrapping_add(addend);
        [acc as u64, (acc >> 64) as u64]
    }
    #[inline(never)]
    fn batch(&self, input: &[[u64; L]], out: &mut [[u64; 2]]) {
        assert_eq!(input.len(), out.len());
        for (x, o) in input.iter().zip(out) {
            *o = self.reduce(x);
        }
    }
}

fn projection<const L: usize>(samples: usize, rng: &mut Rng) {
    for (bits, q) in [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)] {
        let prepared = Projection::<L>::new(q);
        let old =
            ModRingCtx::<2>::new(field::Uint::from_words([q as u64, (q >> 64) as u64])).unwrap();
        let nz = NonZero::new(Uint::<2>::from_words([q as u64, (q >> 64) as u64])).unwrap();
        for n in [0, 1, 3, 17, 16, 1024] {
            let size = format!("q{bits}_l{L}_n{n}");
            if (n == 16 || n == 1024) && !case_requested("opt_projection", &size) {
                continue;
            }
            let a: Vec<[u64; L]> = (0..n)
                .map(|i| match i {
                    0 => [0; L],
                    1 => [u64::MAX; L],
                    2 => array::from_fn(|j| if j == L - 1 { 1 << 63 } else { 0 }),
                    _ => array::from_fn(|_| rng.next()),
                })
                .collect();
            let stored: Vec<_> = a
                .iter()
                .map(|w| crate::arithmetic::integer_from_words::<L>(w))
                .collect();
            let expected: Vec<_> = stored
                .iter()
                .map(|x| project_signed(&old, x.as_words()))
                .collect();
            for (a, r) in a.iter().zip(&expected) {
                let signed = BigInt::from_signed_bytes_le(
                    &a.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>(),
                );
                let oracle = ((signed % BigInt::from(q)) + BigInt::from(q)) % BigInt::from(q);
                assert_eq!(big(&prepared.reduce(a)), oracle.to_biguint().unwrap());
                assert_eq!(prepared.reduce(a), *r);
            }
            if n != 16 && n != 1024 {
                continue;
            }
            let mut cases = vec![
                pass(
                    "production",
                    n * (L * 8 + 16),
                    &stored,
                    &expected,
                    |a, o| {
                        for (x, o) in a.iter().zip(o) {
                            *o = project_signed(&old, x.as_words());
                        }
                    },
                ),
                pass("public_divisor", n * (L * 8 + 16), &a, &expected, |a, o| {
                    for (w, o) in a.iter().zip(o) {
                        let x = Uint::<L>::from_words(*w);
                        let (_, r) = x.div_rem_vartime(&nz);
                        let rw = r.to_words();
                        let r = rw[0] as u128 | ((rw[1] as u128) << 64);
                        let correction = prepared.signed_correction
                            & 0u128.wrapping_sub((w[L - 1] >> 63) as u128);
                        let r = prepared.ctx.sub(r, correction);
                        *o = [r as u64, (r >> 64) as u64];
                    }
                }),
                pass(
                    "horner_prepared",
                    n * (L * 8 + 16),
                    &a,
                    &expected,
                    |a, o| prepared.batch(a, o),
                ),
                pass(
                    "horner_one_shot",
                    n * (L * 8 + 16),
                    &a,
                    &expected,
                    |a, o| {
                        let p = Projection::<L>::new(black_box(q));
                        p.batch(a, o);
                    },
                ),
            ];
            measure("opt_projection", &size, &mut cases, samples, rng);
        }
        let size = format!("q{bits}_l{L}");
        let mut cases = vec![Case::new("horner", 0, || {
            black_box(Projection::<L>::new(black_box(q)));
        })];
        measure("opt_projection_setup", &size, &mut cases, samples, rng);
    }
}

fn division<const L: usize>(samples: usize, rng: &mut Rng) {
    let d = NonZero::new(Limb(0xffff_ffff_ffff_ffc5)).unwrap();
    let reciprocal = Reciprocal::new(d);
    let divisor = NonZero::new(Uint::<L>::from_words(array::from_fn(|i| {
        if i == 0 { d.get().0 } else { 0 }
    })))
    .unwrap();
    for n in [16, 1024] {
        let size = format!("l{L}_n{n}");
        if !case_requested("opt_divrem64", &size) {
            continue;
        }
        let a: Vec<_> = (0..n)
            .map(|_| Uint::<L>::from_words(array::from_fn(|_| rng.next())))
            .collect();
        let expected: Vec<_> = a.iter().map(|x| x.div_rem(&divisor)).collect();
        let mut cases = vec![
            pass("generic", n * L * 24, &a, &expected, |a, o| {
                for (x, o) in a.iter().zip(o) {
                    *o = x.div_rem(&divisor);
                }
            }),
            pass("public_divisor", n * L * 24, &a, &expected, |a, o| {
                for (x, o) in a.iter().zip(o) {
                    *o = x.div_rem_vartime(&divisor);
                }
            }),
            pass("prepared_reciprocal", n * L * 24, &a, &expected, |a, o| {
                for (x, o) in a.iter().zip(o) {
                    let (q, r) = x.div_rem_limb_with_reciprocal(&reciprocal);
                    *o = (
                        q,
                        Uint::from_words(array::from_fn(|i| if i == 0 { r.0 } else { 0 })),
                    );
                }
            }),
            pass("prepare_batch", n * L * 24, &a, &expected, |a, o| {
                let rcp = Reciprocal::new(black_box(d));
                for (x, o) in a.iter().zip(o) {
                    let (q, r) = x.div_rem_limb_with_reciprocal(&rcp);
                    *o = (
                        q,
                        Uint::from_words(array::from_fn(|i| if i == 0 { r.0 } else { 0 })),
                    );
                }
            }),
        ];
        measure("opt_divrem64", &size, &mut cases, samples, rng);
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    mac_cases::<1>(samples, rng);
    mac_cases::<2>(samples, rng);
    mac_cases::<4>(samples, rng);
    mac_cases::<9>(samples, rng);
    projection::<2>(samples, rng);
    projection::<4>(samples, rng);
    projection::<9>(samples, rng);
    division::<2>(samples, rng);
    division::<4>(samples, rng);
    division::<9>(samples, rng);
    division::<32>(samples, rng);
    division::<64>(samples, rng);
}
