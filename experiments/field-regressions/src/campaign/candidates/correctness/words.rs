use super::super::correctness as oracle;
use super::*;
use num_traits::One;

fn primitive<const L: usize>() {
    let mut rng = Rng(0x6877_6361_7272_7931);
    let edge = oracle::boundaries::<L>();
    let mut pairs = Vec::new();
    for a in edge {
        for b in [
            [0; L],
            [u64::MAX; L],
            array::from_fn(|i| u64::from(i == 0)),
            a,
            array::from_fn(|_| rng.next()),
        ] {
            pairs.push((a, b));
        }
    }
    let modulus = BigInt::one() << (64 * L);
    let half = &modulus >> 1;
    for subtract in [false, true] {
        let expected_unsigned: Vec<_> = pairs
            .iter()
            .map(|(a, b)| {
                let (a, b) = (
                    BigInt::from(oracle::unsigned(a)),
                    BigInt::from(oracle::unsigned(b)),
                );
                if subtract { a - b } else { a + b }
            })
            .collect();
        let expected_signed: Vec<_> = pairs
            .iter()
            .map(|(a, b)| {
                if subtract {
                    oracle::signed(a) - oracle::signed(b)
                } else {
                    oracle::signed(a) + oracle::signed(b)
                }
            })
            .collect();
        let mut wrapped = vec![Uint::from_words([0xdead_beef; L]); pairs.len()];
        let mut checked = vec![None; pairs.len()];
        let mut limb_checked = checked.clone();
        let mut signed_checked = checked.clone();
        if subtract {
            wrapping_batch::<L, true>(&pairs, &mut wrapped);
            checked_batch::<L, false, true>(&pairs, &mut checked);
            limb_checked_batch::<L, true>(&pairs, &mut limb_checked);
            checked_batch::<L, true, true>(&pairs, &mut signed_checked);
        } else {
            wrapping_batch::<L, false>(&pairs, &mut wrapped);
            checked_batch::<L, false, false>(&pairs, &mut checked);
            limb_checked_batch::<L, false>(&pairs, &mut limb_checked);
            checked_batch::<L, true, false>(&pairs, &mut signed_checked);
        }
        for (i, ((a, b), (u, s))) in pairs
            .iter()
            .zip(expected_unsigned.iter().zip(&expected_signed))
            .enumerate()
        {
            let reduced = oracle::modulo(u.clone(), &modulus);
            assert_eq!(
                BigInt::from(oracle::unsigned(wrapped[i].as_words())),
                reduced
            );
            let unsigned_overflow = u < &BigInt::zero() || u >= &modulus;
            let signed_overflow = s < &(-&half) || s >= &half;
            assert_eq!(checked[i].is_none(), unsigned_overflow);
            assert_eq!(limb_checked[i], checked[i]);
            assert_eq!(signed_checked[i].is_none(), signed_overflow);
            if let Some(v) = checked[i] {
                assert_eq!(BigInt::from(oracle::unsigned(v.as_words())), *u);
            }
            if let Some(v) = signed_checked[i] {
                assert_eq!(oracle::signed(v.as_words()), *s);
            }
            let (w, c) = if subtract { sub(a, b) } else { add(a, b) };
            assert_eq!(w, wrapped[i].to_words());
            assert_eq!(c, unsigned_overflow);
        }
    }
    wrapping_batch::<L, false>(&[], &mut []);
    checked_batch::<L, true, true>(&[], &mut []);
    limb_checked_batch::<L, false>(&[], &mut []);
}

#[test]
fn all_carry_borrow_and_signed_boundaries() {
    primitive::<1>();
    primitive::<2>();
    primitive::<4>();
    primitive::<9>();
}

fn product<const A: usize, const B: usize, const O: usize>() {
    let mut rng = Rng(0x7072_6f64_7563_7431);
    for i in 0..256 {
        let a = match i % 4 {
            0 => [0; A],
            1 => [u64::MAX; A],
            _ => array::from_fn(|_| rng.next()),
        };
        let b = match i % 5 {
            0 => [u64::MAX; B],
            1 => array::from_fn(|j| u64::from(j == B - 1) << 63),
            _ => array::from_fn(|_| rng.next()),
        };
        assert_eq!(
            oracle::unsigned(&wide::<A, B, O>(&a, &b)),
            oracle::unsigned(&a) * oracle::unsigned(&b)
        );
    }
}

#[test]
fn widening_products_match_biguint() {
    product::<1, 1, 2>();
    product::<2, 2, 4>();
    product::<4, 4, 8>();
    product::<9, 9, 18>();
    product::<2, 9, 11>();
    product::<9, 2, 11>();
    product::<32, 32, 64>();
    product::<64, 64, 128>();
}

fn exact_mac<const L: usize, const O: usize>() {
    let mut rng = Rng(0x6578_6163_745f_6d61);
    for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 63, 64, 65, 257, 1025] {
        for class in 0..4 {
            let mut values = || {
                (0..n)
                    .map(|i| match class {
                        0 => [u64::MAX; L],
                        1 => array::from_fn(|j| u64::from(j == L - 1) << 63),
                        2 => array::from_fn(|j| if (i + j) % 2 == 0 { u64::MAX } else { 0 }),
                        _ => array::from_fn(|_| rng.next()),
                    })
                    .collect::<Vec<_>>()
            };
            let a = values();
            let mut b = values();
            if class == 1 {
                for (i, b) in b.iter_mut().enumerate() {
                    if i % 2 != 0 {
                        *b = [u64::MAX; L];
                    }
                }
            }
            let unsigned = a.iter().zip(&b).fold(BigUint::zero(), |s, (a, b)| {
                s + oracle::unsigned(a) * oracle::unsigned(b)
            });
            let signed = a.iter().zip(&b).fold(BigInt::zero(), |s, (a, b)| {
                s + oracle::signed(a) * oracle::signed(b)
            });
            assert_eq!(oracle::unsigned(&exact_columns::<L, O>(&a, &b)), unsigned);
            assert_eq!(
                oracle::unsigned(&exact::<L, O, false, true>(&a, &b)),
                unsigned
            );
            assert_eq!(
                oracle::unsigned(&exact::<L, O, false, false>(&a, &b)),
                unsigned
            );
            assert_eq!(oracle::signed(&exact::<L, O, true, true>(&a, &b)), signed);
            assert_eq!(oracle::signed(&exact::<L, O, true, false>(&a, &b)), signed);
            assert_eq!(
                oracle::signed(&exact_signed_columns::<L, O>(&a, &b)),
                signed
            );
        }
    }
}

#[test]
fn exact_mac_sign_extension_and_maximum_carries() {
    exact_mac::<1, 3>();
    exact_mac::<2, 5>();
    exact_mac::<4, 9>();
    exact_mac::<9, 19>();
}

#[test]
#[should_panic]
fn exact_mac_rejects_different_lengths() {
    exact_columns::<2, 5>(&[[1, 2]], &[]);
}

#[test]
#[should_panic]
fn exact_mac_rejects_short_output() {
    exact_columns::<2, 4>(&[[1, 2]], &[[3, 4]]);
}

#[test]
fn signed_columns_extreme_products_and_exact_cancellation() {
    fn check<const L: usize, const O: usize>() {
        let mut maximum = [u64::MAX; L];
        maximum[L - 1] >>= 1;
        let mut minimum = [0; L];
        minimum[L - 1] = 1 << 63;
        let mut one = [0; L];
        one[0] = 1;
        for (a, b) in [
            (vec![maximum], vec![minimum]),
            (vec![maximum, maximum], vec![one, [u64::MAX; L]]),
            (vec![minimum, minimum], vec![one, [u64::MAX; L]]),
        ] {
            let expected = a.iter().zip(&b).fold(BigInt::zero(), |s, (a, b)| {
                s + oracle::signed(a) * oracle::signed(b)
            });
            assert_eq!(
                oracle::signed(&exact_signed_columns::<L, O>(&a, &b)),
                expected
            );
        }
    }
    check::<1, 3>();
    check::<2, 5>();
    check::<4, 9>();
    check::<9, 19>();
}
