//! Full-width wrapping MAC schedules. Exact signed arithmetic follows whenever
//! the caller's input/capacity bound excludes overflow; no input value dispatch.
use super::integer;
use crate::{Case, Rng, case_requested, measure};
use circuit::witgen::Z;
use num_bigint::{BigInt, BigUint};
use num_traits::Zero;
use std::hint::black_box;

#[inline(always)]
fn packed<const L: usize>(v: &Z<L>) -> u128 {
    v.words()[0] as u128 | (v.words()[1] as u128) << 64
}
#[inline(always)]
fn from_raw<const L: usize>(v: u128) -> Z<L> {
    Z::from_le_words(&[v as u64, (v >> 64) as u64])
}

#[inline(never)]
fn product_first<const L: usize, const K: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(L, 2);
    assert_eq!(a.len(), b.len());
    let mut sums = [0u128; K];
    let mut aa = a.chunks_exact(K);
    let mut bb = b.chunks_exact(K);
    for (a, b) in aa.by_ref().zip(bb.by_ref()) {
        for lane in 0..K {
            sums[lane] = sums[lane].wrapping_add(packed(&a[lane]).wrapping_mul(packed(&b[lane])));
        }
    }
    let mut sum = sums.into_iter().fold(0u128, u128::wrapping_add);
    for (a, b) in aa.remainder().iter().zip(bb.remainder()) {
        sum = sum.wrapping_add(packed(a).wrapping_mul(packed(b)));
    }
    from_raw(sum)
}

// Seed both lanes with products instead of adding the first pair to zero.
// The same public-length path handles short batches without a tiny-size table.
#[inline(never)]
fn seeded_product<const L: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(L, 2);
    assert_eq!(a.len(), b.len());
    if a.len() < 2 {
        return from_raw(
            a.first()
                .zip(b.first())
                .map_or(0, |(a, b)| packed(a).wrapping_mul(packed(b))),
        );
    }
    let mut sum0 = packed(&a[0]).wrapping_mul(packed(&b[0]));
    let mut sum1 = packed(&a[1]).wrapping_mul(packed(&b[1]));
    let mut aa = a[2..].chunks_exact(2);
    let mut bb = b[2..].chunks_exact(2);
    for (a, b) in aa.by_ref().zip(bb.by_ref()) {
        sum0 = sum0.wrapping_add(packed(&a[0]).wrapping_mul(packed(&b[0])));
        sum1 = sum1.wrapping_add(packed(&a[1]).wrapping_mul(packed(&b[1])));
    }
    let mut sum = sum0.wrapping_add(sum1);
    for (a, b) in aa.remainder().iter().zip(bb.remainder()) {
        sum = sum.wrapping_add(packed(a).wrapping_mul(packed(b)));
    }
    from_raw(sum)
}

#[inline(never)]
fn split_cross<const L: usize, const K: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(L, 2);
    assert_eq!(a.len(), b.len());
    let mut diagonal = [0u128; K];
    let mut cross01 = [0u64; K];
    let mut cross10 = [0u64; K];
    let mut aa = a.chunks_exact(K);
    let mut bb = b.chunks_exact(K);
    for (a, b) in aa.by_ref().zip(bb.by_ref()) {
        for lane in 0..K {
            let (a, b) = (a[lane].words(), b[lane].words());
            diagonal[lane] = diagonal[lane].wrapping_add(a[0] as u128 * b[0] as u128);
            cross01[lane] = cross01[lane].wrapping_add(a[0].wrapping_mul(b[1]));
            cross10[lane] = cross10[lane].wrapping_add(a[1].wrapping_mul(b[0]));
        }
    }
    let d = diagonal.into_iter().fold(0u128, u128::wrapping_add);
    let c = cross01
        .into_iter()
        .chain(cross10)
        .fold(0u64, u64::wrapping_add);
    let mut sum = d.wrapping_add((c as u128) << 64);
    for (a, b) in aa.remainder().iter().zip(bb.remainder()) {
        sum = sum.wrapping_add(packed(a).wrapping_mul(packed(b)));
    }
    from_raw(sum)
}

/// Default for the revised two-limb prototype; selected using development data
/// and then measured on fresh, frozen confirmation processes.
#[inline(always)]
pub(super) fn dot<const L: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    seeded_product(a, b)
}

fn oracle(a: &[Z<2>], b: &[Z<2>]) -> u128 {
    use num_traits::ToPrimitive;
    let sum = a.iter().zip(b).fold(BigUint::zero(), |sum, (a, b)| {
        sum + BigUint::from(packed(a)) * BigUint::from(packed(b))
    });
    (sum % (BigUint::from(1u8) << 128usize)).to_u128().unwrap()
}
fn check(a: &[Z<2>], b: &[Z<2>]) {
    let want = oracle(a, b);
    assert_eq!(packed(&integer::existing(a, b)), want);
    assert_eq!(packed(&integer::legacy_mac::<2, false>(a, b)), want);
    macro_rules! verify {
        ($f:ident,$k:literal) => {
            assert_eq!(packed(&$f::<2, $k>(a, b)), want);
        };
    }
    verify!(product_first, 1);
    verify!(product_first, 2);
    verify!(product_first, 4);
    verify!(split_cross, 1);
    verify!(split_cross, 2);
    verify!(split_cross, 4);
    assert_eq!(packed(&seeded_product(a, b)), want);
    assert_eq!(packed(&dot(a, b)), want);
    assert_eq!(packed(&integer::mac::<2, false>(a, b)), want);
}
fn verify(rng: &mut Rng) {
    let edge = [
        0,
        1,
        u128::MAX,
        1u128 << 127,
        (1u128 << 127) - 1,
        u64::MAX as u128,
        1u128 << 64,
        (u64::MAX as u128) << 64,
        0xaaaa_aaaa_aaaa_aaaa_aaaa_aaaa_aaaa_aaaau128,
    ];
    for &a in &edge {
        for &b in &edge {
            for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 33] {
                check(&vec![from_raw(a); n], &vec![from_raw(b); n]);
            }
        }
    }
    for n in [1, 2, 3, 7, 15, 17, 31, 65, 257, 1025] {
        let a: Vec<_> = (0..n)
            .map(|_| from_raw(rng.next() as u128 | (rng.next() as u128) << 64))
            .collect();
        let b: Vec<_> = (0..n)
            .map(|_| from_raw(rng.next() as u128 | (rng.next() as u128) << 64))
            .collect();
        check(&a, &b);
    }
}
pub(super) fn run(samples: usize, rng: &mut Rng) {
    verify(rng);
    for shape in ["signed16", "full128"] {
        for n in [1, 3, 7, 16, 17, 1024, 65536, 1048576] {
            let size = format!("{shape}_n{n}");
            if !case_requested("two_limb_mac", &size) {
                continue;
            }
            let mut values = || {
                (0..n)
                    .map(|_| {
                        if shape == "signed16" {
                            Z::<2>::from(rng.next() as i16 as i128)
                        } else {
                            from_raw(rng.next() as u128 | (rng.next() as u128) << 64)
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let a = values();
            let b = values();
            check(&a, &b);
            if shape == "signed16" {
                let exact: BigInt = a
                    .iter()
                    .zip(&b)
                    .map(|(a, b)| BigInt::from(packed(a) as i128) * BigInt::from(packed(b) as i128))
                    .sum();
                assert_eq!(BigInt::from(packed(&dot(&a, &b)) as i128), exact);
            }
            let mut cases = vec![
                Case::new("circuit_z", n * 32, || {
                    black_box(integer::existing(black_box(&a), black_box(&b)));
                }),
                Case::new("legacy_fused", n * 32, || {
                    black_box(integer::legacy_mac::<2, false>(
                        black_box(&a),
                        black_box(&b),
                    ));
                }),
            ];
            macro_rules! case {
                ($f:ident,$k:literal,$name:literal) => {
                    cases.push(Case::new($name, n * 32, || {
                        black_box($f::<2, $k>(black_box(&a), black_box(&b)));
                    }));
                };
            }
            case!(product_first, 1, "product1");
            case!(product_first, 2, "product2");
            case!(product_first, 4, "product4");
            cases.push(Case::new("seeded2", n * 32, || {
                black_box(seeded_product(black_box(&a), black_box(&b)));
            }));
            case!(split_cross, 1, "split1");
            case!(split_cross, 2, "split2");
            case!(split_cross, 4, "split4");
            cases.push(Case::new("selected", n * 32, || {
                black_box(integer::mac::<2, false>(black_box(&a), black_box(&b)));
            }));
            measure("two_limb_mac", &size, &mut cases, samples, rng);
        }
    }
}
