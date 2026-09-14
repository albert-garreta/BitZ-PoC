use super::super::correctness as oracle;
use super::*;
use num_traits::One;

fn check_mac<const L: usize>() {
    let mut rng = Rng(0x6d61_635f_6c69_6d62);
    let modulus = BigUint::one() << (64 * L);
    for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 63, 64, 65, 257, 1025] {
        for class in 0..3 {
            let mut values = || {
                (0..n)
                    .map(|i| {
                        Z::from_le_words(&array::from_fn::<_, L, _>(|j| match class {
                            0 => u64::MAX,
                            1 => {
                                if (i + j) % 2 == 0 {
                                    1 << 63
                                } else {
                                    0
                                }
                            }
                            _ => rng.next(),
                        }))
                    })
                    .collect::<Vec<Z<L>>>()
            };
            let a = values();
            let b = values();
            let expected = a.iter().zip(&b).fold(BigUint::zero(), |s, (a, b)| {
                s + oracle::unsigned(a.words()) * oracle::unsigned(b.words())
            }) % &modulus;
            for result in [
                mac::<L, 1>(&a, &b),
                mac::<L, 2>(&a, &b),
                mac::<L, 4>(&a, &b),
                columns(&a, &b),
            ] {
                assert_eq!(oracle::unsigned(result.words()), expected);
            }
            if L == 1 {
                assert_eq!(oracle::unsigned(native_word(&a, &b).words()), expected);
            }
            if L == 4 {
                assert_eq!(oracle::unsigned(native4::<L, 1>(&a, &b).words()), expected);
                assert_eq!(oracle::unsigned(native4::<L, 2>(&a, &b).words()), expected);
            }
        }
    }
}

#[test]
fn wrapping_macs_match_biguint_including_tails() {
    check_mac::<1>();
    check_mac::<2>();
    check_mac::<4>();
    check_mac::<9>();
}

fn check_projection<const L: usize>() {
    let mut rng = Rng(0x7072_6f6a_6563_7431);
    let mut values = oracle::boundaries::<L>();
    values.extend((0..1024).map(|_| array::from_fn(|_| rng.next())));
    // Projection needs an odd public modulus > 2^64, not a prime modulus.
    for q in [
        (1u128 << 64) + 1,
        (1u128 << 64) + 13,
        (1u128 << 100) + 277,
        (1u128 << 127) - 1,
        u128::MAX - 158,
        u128::MAX,
    ] {
        let p = Projection::<L>::new(q);
        let mut out = vec![[0xdead_beef; 2]; values.len()];
        p.batch(&values, &mut out);
        for (a, result) in values.iter().zip(out) {
            assert_eq!(
                BigInt::from(oracle::unsigned(&result)),
                oracle::modulo(oracle::signed(a), &BigInt::from(q))
            );
        }
    }
}

#[test]
fn signed_projection_matches_euclidean_remainder() {
    check_projection::<1>();
    check_projection::<2>();
    check_projection::<4>();
    check_projection::<9>();
}

#[test]
#[should_panic]
fn projection_rejects_small_modulus() {
    Projection::<2>::new(u64::MAX as u128);
}

#[test]
#[should_panic]
fn projection_rejects_even_modulus() {
    Projection::<2>::new((1u128 << 100) + 2);
}

fn check_division<const L: usize>() {
    let mut rng = Rng(0x7265_6369_7072_6f63);
    let mut values = oracle::boundaries::<L>();
    values.extend((0..128).map(|_| array::from_fn(|_| rng.next())));
    for divisor in [
        1,
        2,
        3,
        1 << 31,
        1 << 32,
        (1 << 63) - 1,
        1 << 63,
        (1 << 63) + 1,
        u64::MAX - 58,
        u64::MAX,
    ] {
        let reciprocal = Reciprocal::new(NonZero::new(Limb(divisor)).unwrap());
        for w in &values {
            let (q, r) = Uint::<L>::from_words(*w).div_rem_limb_with_reciprocal(&reciprocal);
            let value = oracle::unsigned(w);
            let d = BigUint::from(divisor);
            assert_eq!(oracle::unsigned(q.as_words()), &value / &d);
            assert_eq!(BigUint::from(r.0), value % d);
        }
    }
}

#[test]
fn reciprocal_division_checks_quotient_and_remainder() {
    check_division::<2>();
    check_division::<4>();
    check_division::<9>();
    check_division::<32>();
    check_division::<64>();
}
