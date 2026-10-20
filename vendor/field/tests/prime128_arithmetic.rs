use field::*;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

define_prime_field! {
    pub Random128 {
        limbs: 2,
        modulus: [0xe829_a5a1_22da_0c0d, 0xe8dc_0598_0109_8bb6],
        element: Random128Element,
        context: Random128Field,
    }
}
define_prime_field! {
    pub Near128 {
        limbs: 2,
        modulus: [u64::MAX - 158, u64::MAX],
        element: Near128Element,
        context: Near128Field,
    }
}
define_prime_field! {
    pub Narrow100 {
        limbs: 2,
        modulus: [Q100 as u64, (Q100 >> 64) as u64],
        element: Narrow100Element,
        context: Narrow100Field,
    }
}
define_prime_field! {
    pub Padded17 {
        limbs: 2,
        modulus: [17, 0],
        element: Padded17Element,
        context: Padded17Field,
    }
}

struct Rng(u64);

impl Rng {
    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }

    fn value(&mut self) -> u128 {
        u128::from(self.word()) | (u128::from(self.word()) << 64)
    }
}

fn residue(value: BigUint, modulus: &BigUint) -> u128 {
    (value % modulus).to_u128().unwrap()
}

fn check_addition<F: RingOps>(
    field: &F,
    p: u128,
    import_montgomery: impl Fn(u128) -> F::Elem,
    export_montgomery: impl Fn(&F::Elem) -> u128,
) {
    let modulus = BigUint::from(p);
    let boundaries = [
        0,
        1,
        2,
        p - 2,
        p - 1,
        u128::from(u64::MAX) - 1,
        u128::from(u64::MAX),
        1u128 << 64,
        (1u128 << 64) + 1,
        (1u128 << 127) - 1,
        1u128 << 127,
    ];
    let check = |a: u128, b: u128| {
        let sum = field.add(&import_montgomery(a), &import_montgomery(b));
        assert_eq!(
            export_montgomery(&sum),
            residue(BigUint::from(a) + BigUint::from(b), &modulus),
            "Montgomery addition: p={p:x}, a={a:x}, b={b:x}",
        );
    };
    // Importing residues directly ensures these exercise the actual stored
    // limb carries, rather than whichever encodings canonical inputs acquire.
    for &a in boundaries.iter().filter(|&&a| a < p) {
        for &b in boundaries.iter().filter(|&&b| b < p) {
            check(a, b);
        }
    }
    if p > 1u128 << 127 {
        assert!((p - 1).overflowing_add(p - 1).1);
        check(p - 1, p - 1);
    }
    let mut rng = Rng(0xa11d_0128);
    for _ in 0..128 {
        check(rng.value() % p, rng.value() % p);
    }
}

fn check_mixed<F, E, T>(
    field: &F,
    p: u128,
    raw_weights: &[u128],
    values: &[T],
    canonical: impl Fn(&E) -> u128,
) where
    E: Copy,
    T: Copy + Into<u128>,
    F: RingOps<Elem = E>
        + IntegerEmbedding<u128>
        + IntegerEmbedding<T>
        + WideMul<E, T>
        + BatchMulAcc<E, T>
        + Reduce<<F as WideMul<E, T>>::Product, Output = E>
        + Reduce<<F as BatchMulAcc<E, T>>::Accumulator, Output = E>,
{
    let modulus = BigUint::from(p);
    let weights: Vec<_> = raw_weights
        .iter()
        .map(|a| <F as IntegerEmbedding<u128>>::from_integer(field, a))
        .collect();
    for (a, weight) in raw_weights.iter().zip(&weights) {
        assert_eq!(canonical(weight), residue(BigUint::from(*a), &modulus));
        for value in values {
            let b: u128 = (*value).into();
            let expected = residue(BigUint::from(*a) * BigUint::from(b), &modulus);
            let projected = <F as IntegerEmbedding<T>>::from_integer(field, value);
            assert_eq!(canonical(&projected), residue(BigUint::from(b), &modulus));
            assert_eq!(canonical(&field.mul(weight, &projected)), expected);
            let eager = field.reduce(field.mul_wide(weight, value));
            assert_eq!(
                canonical(&eager),
                expected,
                "mixed product: p={p:x}, a={a:x}, b={b:x}",
            );
        }
    }

    for len in [0, 1, 2, 3, 17, 65] {
        let lhs: Vec<_> = (0..len).map(|i| weights[i % weights.len()]).collect();
        let rhs = &values[..len];
        let mut eager = field.zero();
        let mut expected = BigUint::from(0u32);
        for (i, (weight, value)) in lhs.iter().zip(rhs).enumerate() {
            eager = field.add(&eager, &field.reduce(field.mul_wide(weight, value)));
            let value: u128 = (*value).into();
            expected += BigUint::from(raw_weights[i % raw_weights.len()]) * BigUint::from(value);
        }
        let expected = residue(expected, &modulus);
        assert_eq!(canonical(&eager), expected, "eager mixed dot, len={len}");
        let delayed = field.reduce(field.batch_mul_acc(&lhs, rhs));
        assert_eq!(
            canonical(&delayed),
            expected,
            "delayed mixed dot, len={len}"
        );
    }
}

fn check_field_dots<F, E>(field: &F, p: u128, raw: &[u128], canonical: impl Fn(&E) -> u128)
where
    E: Copy,
    F: RingOps<Elem = E>
        + IntegerEmbedding<u128>
        + BatchMulAcc<E>
        + Reduce<<F as BatchMulAcc<E>>::Accumulator, Output = E>,
{
    let modulus = BigUint::from(p);
    let values: Vec<_> = raw.iter().map(|a| field.from_integer(a)).collect();
    for len in [0, 1, 2, 3, 17, 65] {
        let lhs = &values[..len];
        let rhs: Vec<_> = values.iter().rev().take(len).copied().collect();
        let mut eager = field.zero();
        let mut expected = BigUint::from(0u32);
        for (i, (a, b)) in lhs.iter().zip(&rhs).enumerate() {
            eager = field.add(&eager, &field.mul(a, b));
            expected += BigUint::from(raw[i]) * BigUint::from(raw[raw.len() - 1 - i]);
        }
        let expected = residue(expected, &modulus);
        assert_eq!(canonical(&eager), expected, "eager field dot, len={len}");
        let delayed = field.reduce(field.batch_mul_acc(lhs, &rhs));
        assert_eq!(
            canonical(&delayed),
            expected,
            "delayed field dot, len={len}"
        );
    }
}

fn check_prime<P: PrimeSpec<2>>() {
    let p = u128::from(P::MODULUS);
    let dynamic = create_prime_field(P::MODULUS);
    let fixed = StaticFpOps::<P, 2>::new();
    let mut raw = vec![
        0,
        1,
        p - 2,
        p - 1,
        p,
        p + 1,
        u128::from(u32::MAX),
        u128::from(u32::MAX) + 1,
        u128::from(u64::MAX),
        1u128 << 64,
        1u128 << 127,
        u128::MAX,
    ];
    let mut rng = Rng(0xb16b_0128);
    raw.extend((0..64).map(|_| rng.value()));
    let values32: Vec<_> = raw.iter().map(|&x| x as u32).collect();
    let values64: Vec<_> = raw.iter().map(|&x| x as u64).collect();

    macro_rules! check_provider {
        ($field:expr) => {{
            let field = $field;
            check_addition(
                field,
                p,
                |a| field.from_montgomery_integer(Uint::from(a)),
                |a| u128::from(*a.as_montgomery_integer()),
            );
            check_mixed(field, p, &raw[..16], &values32, |a| {
                u128::from(field.to_integer(a))
            });
            check_mixed(field, p, &raw[..16], &values64, |a| {
                u128::from(field.to_integer(a))
            });
            check_mixed(field, p, &raw[..16], &raw, |a| {
                u128::from(field.to_integer(a))
            });
            check_field_dots(field, p, &raw, |a| u128::from(field.to_integer(a)));
        }};
    }
    check_provider!(&dynamic);
    check_provider!(&fixed);
}

#[test]
fn sampled_full_width_prime_matches_biguint() {
    check_prime::<Random128>();
}

#[test]
fn near_radix_full_width_prime_matches_biguint() {
    check_prime::<Near128>();
}

#[test]
fn narrower_two_limb_prime_matches_biguint() {
    check_prime::<Narrow100>();
}

#[test]
fn padded_one_limb_prime_matches_biguint() {
    check_prime::<Padded17>();
}
