use crate::production_projection::{RuntimeModulus, StoredInteger};
use crate::{Case, Rng, case_requested, measure, production_p256};
use circuit::witgen::Z;
use crypto_bigint::{NonZero, Uint};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::Zero;
use std::{array, hint::black_box};

fn bigint<const L: usize>(x: Z<L>) -> BigInt {
    BigInt::from_signed_bytes_le(
        &x.words()
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}
fn biguint<const L: usize>(x: [u64; L]) -> BigUint {
    BigUint::from_bytes_le(&x.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}
// Same modulo-2^(64L) contract as existing Z multiplication/addition. Timed
// fixtures separately satisfy an exact signed-integer capacity bound.
#[inline(always)]
fn fused<const L: usize>(out: &mut [u64; L], a: &[u64; L], b: &[u64; L]) {
    for i in 0..L {
        let mut carry = 0u128;
        for j in 0..L - i {
            let sum = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + carry;
            out[i + j] = sum as u64;
            carry = sum >> 64;
        }
    }
}
#[inline(never)]
pub(super) fn existing<const L: usize>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold(Z::zero(), |sum, (&a, &b)| sum + a * b)
}
#[inline(never)]
pub(super) fn legacy_mac<const L: usize, const CHECK: bool>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    assert_eq!(a.len(), b.len());
    // This particular public fixture contract is signed 16-bit operands and
    // at most 2^20 terms, giving a strict magnitude bound below 2^50.
    assert!(a.len() <= 1 << 20);
    let mut out = [0; L];
    for (a, b) in a.iter().zip(b) {
        if CHECK {
            for x in [a, b] {
                let low = x.words()[0] as i64;
                assert!((-32768..=32767).contains(&low));
                let sign = if low < 0 { u64::MAX } else { 0 };
                assert!(x.words()[1..].iter().all(|&w| w == sign));
            }
        }
        fused(&mut out, a.words(), b.words());
    }
    Z::from_le_words(&out)
}
// The unchecked two-limb batch uses its measured product-first schedule.
// Other widths and the per-term-check diagnostic retain their original kernels.
#[inline(always)]
pub(super) fn mac<const L: usize, const CHECK: bool>(a: &[Z<L>], b: &[Z<L>]) -> Z<L> {
    if L == 2 && !CHECK {
        super::two_limb_mac::dot(a, b)
    } else {
        legacy_mac::<L, CHECK>(a, b)
    }
}
fn integers<const L: usize>(samples: usize, rng: &mut Rng) {
    // Full-width wrapping oracle catches carries and truncation, independently
    // of the bounded small signed operands in the timing workload.
    for _ in 0..128 {
        let a = array::from_fn(|_| rng.next());
        let b = array::from_fn(|_| rng.next());
        let start = array::from_fn(|_| rng.next());
        let mut out: [u64; L] = start;
        fused(&mut out, &a, &b);
        let expected =
            (biguint(start) + biguint(a) * biguint(b)) % (BigUint::from(1u8) << (64 * L));
        assert_eq!(biguint(out), expected);
    }
    for n in [16, 1024, 65536] {
        let size = format!("l{L}_n{n}");
        if !case_requested("integer_mac", &size) {
            continue;
        }
        let a: Vec<_> = (0..n)
            .map(|_| Z::<L>::from(rng.next() as i16 as i128))
            .collect();
        let b: Vec<_> = (0..n)
            .map(|_| Z::<L>::from(rng.next() as i16 as i128))
            .collect();
        let expected = a
            .iter()
            .zip(&b)
            .fold(BigInt::zero(), |sum, (&a, &b)| sum + bigint(a) * bigint(b));
        assert_eq!(bigint(existing(&a, &b)), expected);
        assert_eq!(bigint(mac::<L, false>(&a, &b)), expected);
        assert_eq!(bigint(mac::<L, true>(&a, &b)), expected);
        let mut cases = vec![
            Case::new("circuit_z", n * L * 16, || {
                black_box(existing(black_box(&a), black_box(&b)));
            }),
            Case::new("fused", n * L * 16, || {
                black_box(mac::<L, false>(black_box(&a), black_box(&b)));
            }),
            Case::new("checked_per_term", n * L * 16, || {
                black_box(mac::<L, true>(black_box(&a), black_box(&b)));
            }),
        ];
        measure("integer_mac", &size, &mut cases, samples, rng);
    }
}
#[inline(always)]
fn fixed_product<const ACTIVE: usize>(a: [u64; 9], b: [u64; 9]) -> [u64; 18] {
    let mut out = [0; 18];
    for i in 0..ACTIVE {
        let mut carry = 0u128;
        for j in 0..ACTIVE {
            let sum = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + carry;
            out[i + j] = sum as u64;
            carry = sum >> 64;
        }
        out[i + ACTIVE] = carry as u64;
    }
    out
}
#[inline(never)]
fn wide_products<const K: usize>(a: &[[u64; 9]], b: &[[u64; 9]], out: &mut [[u64; 18]]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = match K {
            0 => production_p256::product(a, b),
            4 => fixed_product::<4>(a, b),
            _ => fixed_product::<9>(a, b),
        };
    }
}
fn p256(samples: usize, rng: &mut Rng) {
    for active in [4, 9] {
        for n in [16, 1024, 65536] {
            let size = format!("active{active}_n{n}");
            if !case_requested("p256_product", &size) {
                continue;
            }
            let a: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            let b: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            let mut baseline = vec![[0; 18]; n];
            wide_products::<0>(&a, &b, &mut baseline);
            for ((&a, &b), &out) in a.iter().zip(&b).zip(&baseline).take(256) {
                assert_eq!(biguint(out), biguint(a) * biguint(b));
            }
            let mut full = vec![[0; 18]; n];
            wide_products::<9>(&a, &b, &mut full);
            assert_eq!(full, baseline);
            let mut bound = vec![[0; 18]; n];
            let mut cases = vec![
                Case::new("existing_p256", n * 288, || {
                    wide_products::<0>(black_box(&a), black_box(&b), black_box(&mut baseline));
                    black_box(&baseline);
                }),
                Case::new("fixed9", n * 288, || {
                    wide_products::<9>(black_box(&a), black_box(&b), black_box(&mut full));
                    black_box(&full);
                }),
            ];
            if active == 4 {
                wide_products::<4>(&a, &b, &mut bound);
                assert!(
                    bound
                        .iter()
                        .zip(&a)
                        .zip(&b)
                        .all(|((v, a), b)| *v == production_p256::product(*a, *b))
                );
                cases.push(Case::new("public_bound4", n * 288, || {
                    wide_products::<4>(black_box(&a), black_box(&b), black_box(&mut bound));
                    black_box(&bound);
                }));
            }
            measure("p256_product", &size, &mut cases, samples, rng);
        }
    }
}
#[inline(always)]
fn remainder<const L: usize>(x: Z<L>, q: u128, modulus: &NonZero<Uint<2>>) -> [u64; 2] {
    let negative = x.is_negative();
    let magnitude = if negative { -x } else { x };
    let (_, r) = Uint::<L>::from_words(*magnitude.words()).div_rem_vartime(modulus);
    let w = r.to_words();
    let r = w[0] as u128 | (w[1] as u128) << 64;
    let r = if negative && r != 0 { q - r } else { r };
    [r as u64, (r >> 64) as u64]
}
#[inline(never)]
fn project_existing(modulus: &RuntimeModulus<2>, input: &[StoredInteger], out: &mut [[u64; 2]]) {
    assert_eq!(input.len(), out.len());
    for (x, out) in input.iter().zip(out) {
        *out = modulus.reduce(x);
    }
}
#[inline(never)]
fn project_fixed<const L: usize>(
    q: u128,
    modulus: &NonZero<Uint<2>>,
    input: &[Z<L>],
    out: &mut [[u64; 2]],
) {
    assert_eq!(input.len(), out.len());
    for (&x, out) in input.iter().zip(out) {
        *out = remainder(x, q, modulus);
    }
}
fn projection<const L: usize>(samples: usize, rng: &mut Rng) {
    for (bits, q) in [(100, (1u128 << 100) + 277), (128, u128::MAX - 158)] {
        let modulus = RuntimeModulus::<2>::new(BigUint::from(q)).unwrap();
        let nz = NonZero::new(Uint::from_words([q as u64, (q >> 64) as u64])).unwrap();
        // Signed extrema, zero and -1 verify sign recovery.
        let mut high = [0; L];
        high[L - 1] = 1 << 63;
        for x in [
            Z::zero(),
            Z::from(-1i128),
            Z::from(1u64),
            Z::<L>::from_le_words(&high),
        ] {
            let expected = ((bigint(x) % BigInt::from(q)) + BigInt::from(q)) % BigInt::from(q);
            assert_eq!(
                biguint(remainder(x, q, &nz)),
                expected.to_biguint().unwrap()
            );
            assert_eq!(
                remainder(x, q, &nz),
                modulus.reduce(&StoredInteger::from_fixed(x))
            );
        }
        for n in [16, 1024] {
            let size = format!("q{bits}_l{L}_n{n}");
            if !case_requested("projection", &size) {
                continue;
            }
            let fixed: Vec<_> = (0..n)
                .map(|_| Z::<L>::from_le_words(&array::from_fn::<_, L, _>(|_| rng.next())))
                .collect();
            let stored: Vec<_> = fixed
                .iter()
                .copied()
                .map(StoredInteger::from_fixed)
                .collect();
            let mut original = vec![[0; 2]; n];
            project_existing(&modulus, &stored, &mut original);
            for (&x, &out) in fixed.iter().zip(&original) {
                let expected = ((bigint(x) % BigInt::from(q)) + BigInt::from(q)) % BigInt::from(q);
                assert_eq!(BigInt::from_biguint(Sign::Plus, biguint(out)), expected);
            }
            let mut candidate = vec![[0; 2]; n];
            project_fixed(q, &nz, &fixed, &mut candidate);
            assert_eq!(candidate, original);
            // Payload includes both StoredInteger descriptors and bounded limb
            // storage. This is a declared payload estimate, not resident memory.
            let mut cases = vec![
                Case::new(
                    "runtime_modulus",
                    n * (std::mem::size_of::<StoredInteger>() + L * 8 + 16),
                    || {
                        project_existing(
                            black_box(&modulus),
                            black_box(&stored),
                            black_box(&mut original),
                        );
                        black_box(&original);
                    },
                ),
                Case::new("fixed_crypto_bigint_vartime", n * (L * 8 + 16), || {
                    project_fixed(
                        black_box(q),
                        black_box(&nz),
                        black_box(&fixed),
                        black_box(&mut candidate),
                    );
                    black_box(&candidate);
                }),
            ];
            measure("projection", &size, &mut cases, samples, rng);
        }
    }
}
pub(super) fn run(samples: usize, rng: &mut Rng) {
    integers::<1>(samples, rng);
    integers::<2>(samples, rng);
    integers::<4>(samples, rng);
    integers::<9>(samples, rng);
    p256(samples, rng);
    projection::<2>(samples, rng);
    projection::<4>(samples, rng);
    projection::<9>(samples, rng);
}
