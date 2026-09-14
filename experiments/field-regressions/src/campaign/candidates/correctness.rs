//! Independent test oracles: no candidate/vendored arithmetic in expected values.
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Zero};

pub fn unsigned(words: &[u64]) -> BigUint {
    BigUint::from_bytes_le(
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

pub fn signed(words: &[u64]) -> BigInt {
    BigInt::from_signed_bytes_le(
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

pub fn modulo(value: BigInt, modulus: &BigInt) -> BigInt {
    ((value % modulus) + modulus) % modulus
}

/// Extended Euclid; unlike a Fermat oracle, this does not assume primality.
pub fn inverse(value: &BigUint, modulus: &BigUint) -> Option<BigUint> {
    let modulus = BigInt::from(modulus.clone());
    let (mut r, mut next) = (modulus.clone(), BigInt::from(value.clone()) % &modulus);
    let (mut t, mut next_t) = (BigInt::zero(), BigInt::one());
    while !next.is_zero() {
        let q = &r / &next;
        (r, next) = (next.clone(), r - &q * next);
        (t, next_t) = (next_t.clone(), t - q * next_t);
    }
    (r == BigInt::one()).then(|| modulo(t, &modulus).to_biguint().unwrap())
}

/// Every bit boundary, including full-width carry chains and signed extrema.
pub fn boundaries<const L: usize>() -> Vec<[u64; L]> {
    let mut values = vec![
        [0; L],
        [u64::MAX; L],
        [0xaaaa_aaaa_aaaa_aaaa; L],
        [0x5555_5555_5555_5555; L],
    ];
    for bit in 0..64 * L {
        let p = BigUint::one() << bit;
        for v in [&p - BigUint::one(), p.clone(), p + BigUint::one()] {
            let w = v.to_u64_digits();
            values.push(std::array::from_fn(|i| w.get(i).copied().unwrap_or(0)));
        }
    }
    values
}

/// GF(2)[x] convolution with individual bits, not word multiplication intrinsics.
pub fn polynomial(a: &[u64], b: &[u64], output_words: usize) -> Vec<u64> {
    let mut out = vec![0; output_words];
    for i in 0..a.len() * 64 {
        if (a[i / 64] >> (i % 64)) & 1 == 0 {
            continue;
        }
        for j in 0..b.len() * 64 {
            if (b[j / 64] >> (j % 64)) & 1 != 0 {
                out[(i + j) / 64] ^= 1 << ((i + j) % 64);
            }
        }
    }
    out
}
