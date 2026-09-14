use super::super::correctness as oracle;
use super::*;
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};

const PRIMES: [u128; 3] = [(1u128 << 100) + 277, (1u128 << 127) - 1, u128::MAX - 158];

#[test]
fn raw_montgomery_and_delayed_sums_match_biguint() {
    use crate::campaign::prime::{linear_sum, product_sum};
    let mut rng = Rng(0x6d6f_6e74_795f_7261);
    for q in PRIMES.into_iter().chain([(1u128 << 64) + 13]) {
        let config = cfg(q);
        let ctx = Ctx::new(&config);
        let reducer = Reducer::new(&config).unwrap();
        let modulus = BigUint::from(q);
        let radix: BigUint = (BigUint::one() << 128) % &modulus;
        let inverse_radix = oracle::inverse(&radix, &modulus).unwrap();
        let edge = [0, 1, 2, q - 1, q - 2, u64::MAX as u128, 1u128 << 64];
        for a in edge
            .into_iter()
            .chain((0..256).map(|_| (rng.next() as u128 | (rng.next() as u128) << 64) % q))
        {
            for b in edge {
                assert_eq!(
                    ctx.native_residue_u128(a),
                    (BigUint::from(a) * &radix % &modulus).to_u128().unwrap()
                );
                assert_eq!(
                    ctx.mul(a, b),
                    (BigUint::from(a) * BigUint::from(b) * &inverse_radix % &modulus)
                        .to_u128()
                        .unwrap()
                );
                assert_eq!(
                    ctx.add(a, b),
                    ((BigUint::from(a) + BigUint::from(b)) % &modulus)
                        .to_u128()
                        .unwrap()
                );
            }
        }
        for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 63, 64, 65, 257, 1025] {
            for maximum in [false, true] {
                let mut values = || {
                    (0..n)
                        .map(|_| {
                            if maximum {
                                q - 1
                            } else {
                                (rng.next() as u128 | (rng.next() as u128) << 64) % q
                            }
                        })
                        .collect::<Vec<_>>()
                };
                let a = values();
                let b = values();
                let native: Vec<_> = (0..n)
                    .map(|_| if maximum { u64::MAX } else { rng.next() })
                    .collect();
                let dot = a.iter().zip(&b).fold(BigUint::zero(), |s, (&a, &b)| {
                    s + BigUint::from(a) * BigUint::from(b)
                });
                let linear = a.iter().zip(&native).fold(BigUint::zero(), |s, (&a, &b)| {
                    s + BigUint::from(a) * BigUint::from(b)
                });
                macro_rules! check {
                    ($k:literal) => {{
                        let p = product_sum::<$k, false>(q, &a, &b);
                        assert_eq!(
                            p.reduce_raw_mod_q(&reducer),
                            (&dot % &modulus).to_u128().unwrap()
                        );
                        assert_eq!(
                            p.reduce_raw(&reducer),
                            (&dot * &inverse_radix % &modulus).to_u128().unwrap()
                        );
                        let p = linear_sum::<$k>(&a, &native);
                        assert_eq!(oracle::unsigned(&p.limbs()), linear);
                        assert_eq!(
                            p.reduce_raw(&reducer),
                            (&linear % &modulus).to_u128().unwrap()
                        );
                    }};
                }
                check!(1);
                check!(2);
                check!(4);
                check!(8);
            }
        }
    }
}

#[test]
fn batch_inverses_match_extended_euclid_with_zeros() {
    let mut rng = Rng(0x696e_7665_7273_6531);
    for q in PRIMES {
        let config = cfg(q);
        let ctx = Ctx::new(&config);
        let modulus = BigUint::from(q);
        let radix: BigUint = (BigUint::one() << 128) % &modulus;
        for n in [0, 1, 2, 3, 4, 5, 15, 16, 17, 63, 64, 65, 257] {
            for zero_pattern in [false, true] {
                let plain: Vec<_> = (0..n)
                    .map(|i| {
                        if zero_pattern {
                            0
                        } else {
                            match i % 7 {
                                0 => 0,
                                1 => 1,
                                2 => q - 1,
                                3 => q - 2,
                                4 => 2,
                                _ => (rng.next() as u128 | (rng.next() as u128) << 64) % q,
                            }
                        }
                    })
                    .collect();
                let expected: Vec<_> = plain
                    .iter()
                    .map(|&a| {
                        if a == 0 {
                            0
                        } else {
                            (oracle::inverse(&BigUint::from(a), &modulus).unwrap() * &radix
                                % &modulus)
                                .to_u128()
                                .unwrap()
                        }
                    })
                    .collect();
                let input: Vec<_> = plain
                    .iter()
                    .map(|&a| (BigUint::from(a) * &radix % &modulus).to_u128().unwrap())
                    .collect();
                let mut prefix = vec![u128::MAX; n];
                let mut out = vec![u128::MAX; n];
                batch::<true>(&ctx, &input, &mut prefix, &mut out);
                assert_eq!(out, expected);
                prefix.fill(u128::MAX);
                out.fill(u128::MAX);
                batch::<false>(&ctx, &input, &mut prefix, &mut out);
                assert_eq!(out, expected);
                let fields: Vec<_> = input.iter().map(|&a| ctx.field(a)).collect();
                for result in [
                    batch_fields::<true>(&ctx, &fields),
                    batch_fields::<false>(&ctx, &fields),
                    batch_public_nonzero(&ctx, &fields),
                ] {
                    assert_eq!(
                        result.iter().map(|a| ctx.raw(a)).collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn public_powers_match_biguint_at_exponent_boundaries() {
    for q in PRIMES {
        let config = cfg(q);
        let ctx = Ctx::new(&config);
        let modulus = BigUint::from(q);
        let radix: BigUint = (BigUint::one() << 128) % &modulus;
        for a in [0, 1, 2, q - 1, q - 2, u64::MAX as u128] {
            let raw = (BigUint::from(a) * &radix % &modulus).to_u128().unwrap();
            for e in [0, 1, 2, 3, 63, 64, 65, 65537, 1u128 << 127, u128::MAX] {
                let expected =
                    BigUint::from(a).modpow(&BigUint::from(e), &modulus) * &radix % &modulus;
                assert_eq!(public_pow(&ctx, raw, e), expected.to_u128().unwrap());
            }
        }
    }
}

#[test]
#[should_panic]
fn batch_inverse_rejects_short_scratch() {
    let config = cfg(PRIMES[0]);
    let ctx = Ctx::new(&config);
    batch::<true>(&ctx, &[ctx.one()], &mut [], &mut [0]);
}
