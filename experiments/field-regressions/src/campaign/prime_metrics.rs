//! Missing-operation baselines for the existing runtime prime implementation.
//!
//! Every family has only an `existing` row: this records its cost, not a speedup
//! for the proposed library. Inversion uses the existing variable-time API on
//! public nonzero fixtures. The inversion loop is not Montgomery batch inversion.
//! Codec rows measure canonical element conversion into preallocated buffers;
//! they exclude protocol framing and the allocating `Vec` codec wrapper.
use crate::{Case, Rng, case_requested, measure, production_raw::RawMontyCtx};
use crypto_bigint::{Odd, Uint, modular::FixedMontyParams};
use crypto_primitives::{
    FromWithConfig, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint,
};
use f2z::{piop::spartan::SpartanField, transcript::traits::GenTranscribable};
use num_bigint::BigUint;
use num_traits::{Inv, One, ToPrimitive};
use std::{hint::black_box, mem::size_of};

type Field = MontyField<2>;
type Config = FixedMontyParams<2>;

// Known Mersenne prime; construction below sets up Montgomery constants and
// deliberately does not benchmark prime sampling or a primality test.
const MODULUS: u128 = (1u128 << 127) - 1;
const EXPONENTS: [(&str, u128, u32); 2] = [
    ("e17_sparse", 65_537, 17),
    (
        "e127_alternating",
        0x5555_5555_5555_5555_5555_5555_5555_5555,
        127,
    ),
];

fn config(modulus: u128) -> Config {
    Config::new_vartime(
        Odd::new(Uint::from_words([modulus as u64, (modulus >> 64) as u64])).unwrap(),
    )
}

fn canonical(value: &Field) -> u128 {
    let words = value.retrieve().to_words();
    u128::from(words[0]) | (u128::from(words[1]) << 64)
}

#[inline(never)]
fn inverse_each(input: &[Field], output: &mut [Field]) {
    assert_eq!(input.len(), output.len());
    for (input, output) in input.iter().zip(output) {
        *output = input.inv().expect("the fixture consists of nonzero units");
    }
}

#[inline(never)]
fn pow_each(input: &[Field], exponent: &FieldUint<2>, exponent_bits: u32, output: &mut [Field]) {
    assert_eq!(input.len(), output.len());
    for (input, output) in input.iter().zip(output) {
        *output = input.pow_bounded_exp(exponent, exponent_bits);
    }
}

#[inline(never)]
fn encode_into(input: &[Field], output: &mut [[u8; 16]]) {
    assert_eq!(input.len(), output.len());
    for (input, output) in input.iter().zip(output) {
        // Exactly the value conversion used by SpartanField's allocating
        // canonical_element_encoding, with the output storage supplied here.
        input.retrieve().write_transcription_bytes_exact(output);
    }
}

#[inline(never)]
fn decode_into(config: &Config, modulus: u128, input: &[[u8; 16]], output: &mut [Field]) -> bool {
    assert_eq!(input.len(), output.len());
    for (input, output) in input.iter().zip(output) {
        // These are the element operations of hybrid/codec.rs::read_q; proof
        // framing, Reader operations and output allocation are excluded.
        let value = u128::from_le_bytes(*input);
        if value >= modulus {
            return false;
        }
        *output = Field::from_with_cfg(value, config);
    }
    true
}

fn inputs(n: usize, config: &Config, rng: &mut Rng) -> (Vec<u128>, Vec<Field>) {
    let edge = [
        1,
        2,
        MODULUS - 1,
        MODULUS - 2,
        u64::MAX as u128,
        1u128 << 64,
    ];
    let plain: Vec<_> = (0..n)
        .map(|i| {
            if i < edge.len() {
                edge[i]
            } else {
                (u128::from(rng.next()) | (u128::from(rng.next()) << 64)) % (MODULUS - 1) + 1
            }
        })
        .collect();
    let values: Vec<_> = plain
        .iter()
        .map(|&value| Field::from_with_cfg(value, config))
        .collect();
    assert!(
        values
            .iter()
            .zip(&plain)
            .all(|(value, &plain)| canonical(value) == plain)
    );
    (plain, values)
}

fn verify_boundaries(config: &Config) {
    let zero = Field::from_with_cfg(0u128, config);
    assert!((&zero).inv().is_none());
    inverse_each(&[], &mut []);
    encode_into(&[], &mut []);
    assert!(decode_into(config, MODULUS, &[], &mut []));

    for value in [0, 1, MODULUS - 1] {
        let input = [value.to_le_bytes()];
        let mut output = [zero.clone()];
        assert!(decode_into(config, MODULUS, &input, &mut output));
        assert_eq!(canonical(&output[0]), value);
    }
    for value in [MODULUS, MODULUS + 1, u128::MAX] {
        assert!(!decode_into(
            config,
            MODULUS,
            &[value.to_le_bytes()],
            &mut [zero.clone()]
        ));
    }
    // Verify exponent bit bounds separately from the two public timed patterns.
    let bases: Vec<_> = [0, 1, 2, MODULUS - 1]
        .map(|value| Field::from_with_cfg(value, config))
        .into();
    for (exponent, bits) in [(0u128, 0), (1, 1), (1u128 << 126, 127), (u128::MAX, 128)] {
        let exponent_words = FieldUint::from_words([exponent as u64, (exponent >> 64) as u64]);
        let mut output = vec![zero.clone(); bases.len()];
        pow_each(&bases, &exponent_words, bits, &mut output);
        for (base, result) in bases.iter().zip(output) {
            let expected = BigUint::from(canonical(base))
                .modpow(&BigUint::from(exponent), &BigUint::from(MODULUS));
            assert_eq!(canonical(&result), expected.to_u128().unwrap());
        }
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    let cfg = config(MODULUS);
    verify_boundaries(&cfg);
    let q = BigUint::from(MODULUS);
    let inverse_exponent = BigUint::from(MODULUS - 2);

    if case_requested("prime_context_setup", "q127") {
        let ctx = RawMontyCtx::new(&cfg);
        assert_eq!(canonical(&ctx.field(ctx.one())), 1);
        let mut cases = vec![Case::new("existing", size_of::<Config>(), || {
            let cfg = config(black_box(MODULUS));
            black_box(RawMontyCtx::new(black_box(&cfg)));
        })];
        measure("prime_context_setup", "q127", &mut cases, samples, rng);
    }

    for n in [16, 1024] {
        let size = format!("q127_n{n}");
        let requested = [
            "prime_inverse_each",
            "prime_canonical_encode_into",
            "prime_canonical_decode_into",
        ]
        .iter()
        .any(|family| case_requested(family, &size))
            || EXPONENTS.iter().any(|(label, _, _)| {
                case_requested("prime_pow_each", &format!("q127_{label}_n{n}"))
            });
        if !requested {
            continue;
        }
        let (plain, input) = inputs(n, &cfg, rng);
        let zero = Field::from_with_cfg(0u128, &cfg);
        let field_bytes = n * size_of::<Field>();

        if case_requested("prime_inverse_each", &size) {
            let mut output = vec![zero.clone(); n];
            inverse_each(&input, &mut output);
            for (&value, inverse) in plain.iter().zip(&output) {
                let expected = BigUint::from(value).modpow(&inverse_exponent, &q);
                assert_eq!(canonical(inverse), expected.to_u128().unwrap());
                assert_eq!((BigUint::from(value) * expected) % &q, BigUint::one());
            }
            let mut cases = vec![Case::new("existing", field_bytes * 2, || {
                inverse_each(black_box(&input), black_box(&mut output));
                black_box(&output);
            })];
            measure("prime_inverse_each", &size, &mut cases, samples, rng);
        }

        for (label, exponent, exponent_bits) in EXPONENTS {
            let size = format!("q127_{label}_n{n}");
            if !case_requested("prime_pow_each", &size) {
                continue;
            }
            let exponent_words = FieldUint::from_words([exponent as u64, (exponent >> 64) as u64]);
            let mut output = vec![zero.clone(); n];
            pow_each(&input, &exponent_words, exponent_bits, &mut output);
            for (&value, result) in plain.iter().zip(&output) {
                let expected = BigUint::from(value).modpow(&BigUint::from(exponent), &q);
                assert_eq!(canonical(result), expected.to_u128().unwrap());
            }
            let mut cases = vec![Case::new("existing", field_bytes * 2, || {
                pow_each(
                    black_box(&input),
                    black_box(&exponent_words),
                    black_box(exponent_bits),
                    black_box(&mut output),
                );
                black_box(&output);
            })];
            measure("prime_pow_each", &size, &mut cases, samples, rng);
        }

        if case_requested("prime_canonical_encode_into", &size)
            || case_requested("prime_canonical_decode_into", &size)
        {
            let mut encoded = vec![[0; 16]; n];
            encode_into(&input, &mut encoded);
            for ((&plain, input), encoded) in plain.iter().zip(&input).zip(&encoded) {
                let mut expected = BigUint::from(plain).to_bytes_le();
                expected.resize(16, 0);
                assert_eq!(encoded.as_slice(), expected);
                assert_eq!(encoded.as_slice(), input.canonical_element_encoding());
            }
            let mut decoded = vec![zero; n];
            assert!(decode_into(&cfg, MODULUS, &encoded, &mut decoded));
            assert!(
                decoded
                    .iter()
                    .zip(&plain)
                    .all(|(value, &plain)| canonical(value) == plain)
            );

            if case_requested("prime_canonical_decode_into", &size) {
                let mut cases = vec![Case::new("existing", field_bytes + n * 16, || {
                    black_box(decode_into(
                        black_box(&cfg),
                        black_box(MODULUS),
                        black_box(&encoded),
                        black_box(&mut decoded),
                    ));
                    black_box(&decoded);
                })];
                measure(
                    "prime_canonical_decode_into",
                    &size,
                    &mut cases,
                    samples,
                    rng,
                );
            }
            if case_requested("prime_canonical_encode_into", &size) {
                let mut cases = vec![Case::new("existing", field_bytes + n * 16, || {
                    encode_into(black_box(&input), black_box(&mut encoded));
                    black_box(&encoded);
                })];
                measure(
                    "prime_canonical_encode_into",
                    &size,
                    &mut cases,
                    samples,
                    rng,
                );
            }
        }
    }
}
