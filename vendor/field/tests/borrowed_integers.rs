use field::*;
use num_bigint::{BigInt, BigUint};

define_prime_field! { pub Tiny { limbs: 1, modulus: [17], element: TinyFp, context: TinyOps } }

fn cases() -> Vec<Vec<u64>> {
    let mut cases = Vec::new();
    for n in [1, 2, 3, 5, 9] {
        cases.push(vec![0; n]);
        cases.push(vec![u64::MAX; n]);
        for i in 0..n {
            let mut positive = vec![0; n];
            positive[i] = 3;
            cases.push(positive.clone());
            positive[n - 1] |= 1 << 63;
            cases.push(positive);
            let mut negative = vec![u64::MAX; n];
            negative[..i].fill(0);
            cases.push(negative);
        }
        let mut minimum = vec![0; n];
        minimum[n - 1] = 1 << 63;
        cases.push(minimum);
    }
    cases
}

fn check<const L: usize>(modulus: Uint<L>) {
    let prime = create_prime_field(modulus);
    let ring = ModRingCtx::new(modulus).unwrap();
    let q = BigUint::from_bytes_le(
        &modulus
            .as_words()
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let signed_q = BigInt::from(q.clone());
    for words in cases() {
        let bytes: Vec<_> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let expected = BigUint::from_bytes_le(&bytes) % &q;
        let actual = prime.to_integer(&prime.from_unsigned_words(&words));
        assert_eq!(
            BigUint::from_bytes_le(
                &actual
                    .as_words()
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .collect::<Vec<_>>()
            ),
            expected
        );
        assert_eq!(actual, ring.to_integer(&ring.from_unsigned_words(&words)));

        let expected = ((BigInt::from_signed_bytes_le(&bytes) % &signed_q) + &signed_q) % &signed_q;
        let actual = prime.to_integer(&prime.from_twos_complement_words(&words));
        assert_eq!(
            BigUint::from_bytes_le(
                &actual
                    .as_words()
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .collect::<Vec<_>>()
            ),
            expected.to_biguint().unwrap()
        );
        assert_eq!(
            actual,
            ring.to_integer(&ring.from_twos_complement_words(&words))
        );
    }
}

#[test]
fn borrowed_projection_matches_big_integer_arithmetic() {
    check(Uint::from_words([17]));
    check(Uint::from_words([17, 0]));
    check(Uint::from_words([277, 1 << 36]));
    check(Uint::from_words([u64::MAX - 158, u64::MAX]));
    let words = [0, 0, 0, 1 << 63];
    let field = TinyOps::new();
    let projected = field.from_twos_complement_words(&words);
    assert_eq!(field.to_integer(&projected), Uint::from_words([8])); // -2^255 mod 17
    let projected = field.from_unsigned_words(&words);
    assert_eq!(field.to_integer(&projected), Uint::from_words([9])); // 2^255 mod 17
}

#[test]
fn composite_projection_matches_big_integer_arithmetic() {
    let ring = ModRingCtx::new(Uint::from_words([18])).unwrap();
    let modulus = BigInt::from(18);
    for words in cases() {
        let bytes: Vec<_> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        for (actual, integer) in [
            (
                ring.from_unsigned_words(&words),
                BigInt::from(BigUint::from_bytes_le(&bytes)),
            ),
            (
                ring.from_twos_complement_words(&words),
                BigInt::from_signed_bytes_le(&bytes),
            ),
        ] {
            let expected = ((integer % &modulus) + &modulus) % &modulus;
            assert_eq!(
                BigInt::from(ring.to_integer(&actual).as_words()[0]),
                expected
            );
        }
    }
}

#[test]
fn empty_words_are_rejected() {
    let prime = create_prime_field(Uint::from_words([17]));
    let ring = ModRingCtx::new(Uint::from_words([18])).unwrap();
    let static_prime = TinyOps::new();
    assert!(std::panic::catch_unwind(|| prime.from_unsigned_words(&[])).is_err());
    assert!(std::panic::catch_unwind(|| prime.from_twos_complement_words(&[])).is_err());
    assert!(std::panic::catch_unwind(|| ring.from_unsigned_words(&[])).is_err());
    assert!(std::panic::catch_unwind(|| ring.from_twos_complement_words(&[])).is_err());
    assert!(std::panic::catch_unwind(|| static_prime.from_unsigned_words(&[])).is_err());
    assert!(std::panic::catch_unwind(|| static_prime.from_twos_complement_words(&[])).is_err());
    assert!(
        std::panic::catch_unwind(|| Z::<1>::checked_from_twos_complement_words_ct(&[])).is_err()
    );
}
