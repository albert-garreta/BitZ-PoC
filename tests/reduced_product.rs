//! Independent oracle coverage for the bounded product division used by MultiSwap.
use field::{CtEq, IntegerOps, PreparedDivisor, Uint};
use num_bigint::BigUint;

fn big<const L: usize>(value: &Uint<L>) -> BigUint {
    BigUint::from_bytes_le(
        &value
            .as_words()
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

fn qualify<const L: usize>(modulus: Uint<L>) {
    let divisor = PreparedDivisor::new(modulus).unwrap();
    let m = big(&modulus);
    let mut seed = 0xabc789123u64;
    let mut values = vec![Uint::ZERO, modulus.wrapping_sub(&Uint::ONE)];
    for _ in 0..24 {
        let x = Uint::<L>::from_words(core::array::from_fn(|_| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed
        }));
        values.push(divisor.div_rem_ct(&x).1);
    }
    for (index, a) in values.iter().enumerate() {
        let square = divisor.square_div_rem_reduced_ct(a);
        assert!(square.validity().declassify());
        let expected = big(a).pow(2);
        assert_eq!(big(&square.value().0), &expected / &m);
        assert_eq!(big(&square.value().1), &expected % &m);
        for b in [&values[(index + 1) % values.len()], a, &Uint::ZERO] {
            let result = divisor.mul_div_rem_reduced_ct(a, b);
            assert!(result.validity().declassify());
            let (q, r) = result.value();
            let product = big(a) * big(b);
            assert_eq!(big(q), &product / &m);
            assert_eq!(big(r), &product % &m);
        }
    }
    for value in [modulus, Uint::MAX] {
        let square = divisor.square_div_rem_reduced_ct(&value);
        assert!(!square.validity().declassify());
        assert_eq!(square.value(), &(Uint::ZERO, Uint::ZERO));
    }
    // Invalid input must not enter the bounded kernel with an oversized product,
    // even when the other multiplicand is zero or both limbs are maximal.
    for (a, b) in [
        (modulus, Uint::ZERO),
        (Uint::ZERO, modulus),
        (Uint::MAX, Uint::MAX),
    ] {
        let result = divisor.mul_div_rem_reduced_ct(&a, &b);
        assert!(!result.validity().declassify());
        assert!(result.value().0.ct_is_zero().declassify());
        assert!(result.value().1.ct_is_zero().declassify());
    }
}

#[test]
fn wide_squares_preserve_diagonals_and_carries() {
    fn check<const L: usize>() {
        let mut seed = 392817u64;
        let mut values = vec![Uint::<L>::ZERO, Uint::ONE, Uint::MAX];
        for _ in 0..32 {
            values.push(Uint::from_words(core::array::from_fn(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed
            })));
        }
        for value in values {
            let square = IntegerOps.square_wide(&value);
            let (lo, hi) = square.as_parts();
            let bytes: Vec<_> = lo.iter().chain(hi).flat_map(|w| w.to_le_bytes()).collect();
            assert_eq!(BigUint::from_bytes_le(&bytes), big(&value).pow(2));
        }
    }
    check::<1>();
    check::<2>();
    check::<4>();
    check::<6>();
    check::<32>();
}

#[test]
fn reduced_products_match_biguint_at_width_and_modulus_boundaries() {
    for modulus in [1, 2, 3, 1 << 63, (1 << 63) + 1, u64::MAX] {
        qualify(Uint::from_words([modulus]));
        qualify(Uint::from_words([modulus, 0, 0]));
    }
    qualify(Uint::from_words([0, 1]));
    qualify(Uint::from_words([1, 1]));
    qualify(Uint::from_words([u64::MAX; 4]));
    qualify(Uint::from_words([u64::MAX; 32]));
    qualify(bitz::piop::spartan::multiswap::modulus_n());
}
