//! secp256k1 ECDSA verification on Binius64's schedule, for measurement only.
//!
//! This circuit exists so that the SHA-256 + ECDSA head-to-head against
//! Binius64 compares proof systems rather than circuit engineering. It
//! follows the group-operation schedule and the completeness policy of the
//! pinned Binius64 fork's stock verifier (`ecdsa::bitcoin_verify` over
//! `msm_strauss_endo`, `crates/circuits/src/ecdsa/` at `bc73510`):
//!
//! * `u1 = z / s` and `u2 = r / s` in the scalar field, each one hinted
//!   division checked by one integer relation;
//! * every scalar is split by the GLV endomorphism into two signed 128-bit
//!   halves (`k = k1 + λ·k2 (mod n)`), the split itself hinted and checked;
//! * for each of `±G` and `±Q` a 16-entry table of small multiples is built
//!   in circuit — one doubling and thirteen additions — and the `φ` table is
//!   derived entrywise by one multiplication by `β` and a conditional
//!   negation (Binius64 builds the generator's table in circuit as well,
//!   because its sign depends on the split);
//! * the accumulator starts at the point at infinity and consumes the four
//!   128-bit halves four bits at a time from the top: 31 × 4 doublings and
//!   32 × 4 additions from 16-entry table lookups;
//! * additions handle the point at infinity on either side and return the
//!   identity for opposite points, but the doubling case `P = Q` is asserted
//!   away exactly as Binius64's `add_incomplete` does;
//! * the result must be finite and its `x`-coordinate must equal `r` as
//!   coordinate-field elements (Binius64's `r_diff.is_zero`), with no
//!   reduction of `x` modulo the group order.
//!
//! Recorded divergences (all on the arithmetization side, none on the
//! schedule): the digest is reduced modulo `n` before the division (Binius64's
//! division needs a reduced dividend, and the composed Binius statement
//! reduces it too); `r` and `s` are shown nonzero through hinted inverses
//! instead of word-level zero tests; table lookups use one indicator row per
//! entry instead of a multiplexer tree; and the GLV check is one integer
//! relation with a hinted quotient instead of a scalar-field multiplication.
//!
//! The paper's P-256 verifier in the parent module is untouched by this
//! module; it shares only the curve-parameterised field and point gadgets.

use super::*;
use field::CtOrd;

/// Number of Boolean inputs: digest, public-key coordinates, and signature
/// scalars, as little-endian 256-bit words.
pub const INPUT_BITS: usize = 5 * 256;

/// Total Boolean witness size of the standalone verifier, including inputs.
pub const WITNESS_BITS: usize = 1_007_412;

/// Number of packed `M * w` bits, including the implicit constant one.
pub const INTEGER_WITNESS_BITS: usize = 1_007_413;

/// Number of rank-1 constraints.
pub const R1CS_ROWS: usize = 10_228;

const WINDOW: usize = 4;
const HALF_BITS: usize = 128;
const WINDOWS: usize = HALF_BITS / WINDOW;
const TABLE: usize = 1 << WINDOW;

/// The endomorphism `φ(x, y) = (β·x, y) = λ·(x, y)` and the lattice basis of
/// its scalar decomposition, as used by libsecp256k1 and by Binius64's
/// `Secp256k1EndosplitHint`.
struct Glv {
    lambda: Uint<4>,
    beta: Uint<4>,
    minus_b1: Uint<2>,
    minus_b2: Uint<4>,
    g1: Uint<4>,
    g2: Uint<4>,
    k1_bound: Uint<2>,
    k2_bound: Uint<2>,
}

const GLV: Glv = Glv {
    lambda: Uint::from_words([
        0xdf02967c1b23bd72,
        0x122e22ea20816678,
        0xa5261c028812645a,
        0x5363ad4cc05c30e0,
    ]),
    beta: Uint::from_words([
        0xc1396c28719501ee,
        0x9cf0497512f58995,
        0x6e64479eac3434e9,
        0x7ae96a2b657c0710,
    ]),
    minus_b1: Uint::from_words([0x6f547fa90abfe4c3, 0xe4437ed6010e8828]),
    minus_b2: Uint::from_words([
        0xd765cda83db1562c,
        0x8a280ac50774346d,
        0xfffffffffffffffe,
        0xffffffffffffffff,
    ]),
    g1: Uint::from_words([
        0xe893209a45dbb031,
        0x3daa8a1471e8ca7f,
        0xe86c90e49284eb15,
        0x3086d221a7d46bcd,
    ]),
    g2: Uint::from_words([
        0x1571b4ae8ac47f71,
        0x221208ac9df506c6,
        0x6f547fa90abfe4c4,
        0xe4437ed6010e8828,
    ]),
    k1_bound: Uint::from_words([0x2016d0b917e4dd77, 0xa2a8918ca85bafe2]),
    k2_bound: Uint::from_words([0x2be08846cea267ed, 0x8a65287bd47179fb]),
};

/// One GLV half: its sign flag and the lifted bits of its magnitude (below
/// `2^128`), which drive the window selectors.
struct Split<CS: Circuit> {
    negative: Lc<CS>,
    bits: Vec<Lc<CS>>,
}

/// `⌊value / 2^384⌉` for a 512-bit product: the rounding step of the
/// decomposition (`div_pow2_round` in libsecp256k1).
fn shift_384_round(product: Uint<8>) -> Uint<2> {
    let words = product.as_words();
    let round = words[5] >> 63;
    Uint::from_words([words[6], words[7]]).wrapping_add(&Uint::from_u64(round))
}

fn low_half(value: Uint<4>) -> Uint<2> {
    let words = value.as_words();
    debug_assert!(words[2] == 0 && words[3] == 0);
    Uint::from_words([words[0], words[1]])
}

/// Prover-side GLV decomposition of a scalar `k` (any 256-bit value; it is
/// reduced modulo `n` first): returns `(k1_negative, k2_negative, |k1|, |k2|)`
/// with `k1 + λ·k2 ≡ k (mod n)` and both magnitudes below `2^128`.
fn endosplit(k: Uint<4>) -> HintResult<(bool, bool, Uint<2>, Uint<2>)> {
    let n = *SECP256K1.scalar_modulus();
    let divisor = SECP256K1.scalar().divisor();
    let k = divisor.div_rem_ct(&k).1;
    let rounded = |g: &Uint<4>| {
        shift_384_round(*IntegerOps.mul_wide(&k, g).checked_resize_ct::<8>().value())
    };
    let c1 = *IntegerOps
        .mul_wide(&rounded(&GLV.g1), &GLV.minus_b1)
        .checked_resize_ct::<4>()
        .value();
    let c2 = *IntegerOps
        .mul_wide(&rounded(&GLV.g2), &GLV.minus_b2)
        .checked_resize_ct::<6>()
        .value();
    let k2 = divisor
        .div_rem_ct(&c1.zero_extend::<7>().wrapping_add(&c2.zero_extend::<7>()))
        .1;
    let k2_lambda = divisor
        .div_rem_ct(
            IntegerOps
                .mul_wide(&k2, &GLV.lambda)
                .checked_resize_ct::<8>()
                .value(),
        )
        .1;
    let k1 = divisor
        .div_rem_ct(
            &k.zero_extend::<5>()
                .wrapping_add(&n.zero_extend::<5>())
                .wrapping_sub(&k2_lambda.zero_extend::<5>()),
        )
        .1;
    let half = n.shr(1);
    let magnitude = |value: Uint<4>| {
        if half.ct_lt(&value).declassify() {
            (true, n.wrapping_sub(&value))
        } else {
            (false, value)
        }
    };
    let (k1_negative, k1_abs) = magnitude(k1);
    let (k2_negative, k2_abs) = magnitude(k2);
    if !(k1_abs.ct_lt(&GLV.k1_bound.zero_extend()) & k2_abs.ct_lt(&GLV.k2_bound.zero_extend()))
        .declassify()
    {
        return Err(HintError::new("GLV decomposition exceeds its 128-bit bounds"));
    }
    Ok((k1_negative, k2_negative, low_half(k1_abs), low_half(k2_abs)))
}

/// Hints and lifts the GLV halves of `scalar`, then constrains
/// `k1 + λ·k2 - scalar ≡ 0 (mod n)` through one integer relation with a
/// hinted, biased quotient: `k1 + λ·k2 - scalar + 2^129·n = q·n`.
fn glv_split<CS: Circuit>(circuit: &mut CS, scalar: Rep<CS>) -> [Split<CS>; 2] {
    let scalar_eval = scalar.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 258, 5, _>(move |context| {
        let k = evaluated_uint::<WORD_LIMBS>(scalar_eval.evaluate_words(context), "GLV scalar")?;
        let (k1_negative, k2_negative, k1, k2) = endosplit(k)?;
        Ok(PackedBits::from_fn(|index| match index {
            0 => k1_negative,
            1 => k2_negative,
            2..=129 => k1.as_words()[(index - 2) / 64] >> ((index - 2) % 64) & 1 != 0,
            _ => k2.as_words()[(index - 130) / 64] >> ((index - 130) % 64) & 1 != 0,
        }))
    });
    let negative1 = uint_from_repr::<CS, 1, 1>(circuit, bits.slice::<1, 1>(0)).value;
    let negative2 = uint_from_repr::<CS, 1, 1>(circuit, bits.slice::<1, 1>(1)).value;
    let (k1, k1_bits) = uint_from_repr_with_lifts::<CS, HALF_BITS, 2>(circuit, bits.slice::<HALF_BITS, 2>(2));
    let (k2, k2_bits) =
        uint_from_repr_with_lifts::<CS, HALF_BITS, 2>(circuit, bits.slice::<HALF_BITS, 2>(2 + HALF_BITS));
    // t = negative · |k|, so the signed half is |k| - 2t.
    let signed = |circuit: &mut CS, negative: &Lc<CS>, magnitude: &Lc<CS>| {
        let t = select_rep::<CS, HALF_BITS, 2>(
            circuit,
            negative.clone(),
            Rep {
                value: magnitude.clone(),
                bound: 1,
            },
            rep_u64(0),
            1,
        );
        magnitude
            .clone()
            .sub(t.value.scale_coefficient(P256Coefficient::<CS>::from(2_u64)))
    };
    let k1_signed = signed(circuit, &negative1, &k1.value);
    let k2_signed = signed(circuit, &negative2, &k2.value);
    let n = *SECP256K1.scalar_modulus();
    // bias = 2^129 · n, so the relation's left-hand side is nonnegative and
    // below 2^130 · n whenever the halves are in range.
    let bias = n.zero_extend::<7>().truncating_shl(129);
    let relation = k1_signed
        .add(k2_signed.scale(&GLV.lambda))
        .sub(scalar.value.clone())
        .add(lc_words(bias.as_words()));
    let relation_eval = relation.capture();
    let quotient = circuit.hint::<P256_Z_LIMBS, 130, 3, _>(move |context| {
        let value = evaluated_uint::<7>(relation_eval.evaluate_words(context), "GLV relation")?;
        let (quotient, remainder) = SECP256K1.scalar().divisor().div_rem_ct(&value);
        if !remainder.ct_is_zero().declassify() {
            return Err(HintError::new("GLV halves do not recombine to the scalar"));
        }
        Ok(packed_wide(quotient))
    });
    let quotient = uint_from_repr::<CS, 130, 3>(circuit, quotient).value;
    assert_zero(circuit, relation.sub(quotient.scale(&n)));
    [
        Split {
            negative: negative1,
            bits: k1_bits,
        },
        Split {
            negative: negative2,
            bits: k2_bits,
        },
    ]
}

/// The `width`-bit window of a lifted little-endian bit string.
fn window<CS: Circuit>(bits: &[Lc<CS>], start: usize, width: usize) -> Lc<CS> {
    bits[start..start + width]
        .iter()
        .cloned()
        .enumerate()
        .fold(lc_u64::<CS>(0), |sum, (bit, value)| {
            sum.add(value.scale_coefficient(P256Coefficient::<CS>::from(1_u64 << bit)))
        })
}

/// `-P` when `negative` is one, `P` otherwise: `y ↦ p - y` under a select.
/// Honest coordinates are canonical, so `p - y` is a 256-bit value.
fn negate_if<CS: Circuit>(circuit: &mut CS, negative: Lc<CS>, point: Point<CS>) -> Point<CS> {
    let negated = Rep {
        value: lc_constant::<CS>(SECP256K1.base_modulus()).sub(point.y.value.clone()),
        bound: 2,
    };
    Point {
        x: point.x,
        y: select_canonical(circuit, negative, negated, point.y),
        infinity: point.infinity,
    }
}

/// `b1 XOR b2` for Boolean linear combinations.
fn xor_bit<CS: Circuit>(circuit: &mut CS, x: Lc<CS>, y: Lc<CS>) -> Lc<CS> {
    let both = and_bit(circuit, x.clone(), y.clone());
    x.add(y).sub(both.scale_coefficient(P256Coefficient::<CS>::from(2_u64)))
}

/// Binius64's `add_incomplete`: complete with respect to the point at
/// infinity on either side and to opposite points (which give the identity),
/// but the doubling case is asserted away rather than handled.
fn add_incomplete<CS: Circuit>(circuit: &mut CS, p: Point<CS>, q: Point<CS>) -> Point<CS> {
    let base = SECP256K1.base();
    let dx = rep_sub(base, q.x.clone(), p.x.clone());
    let dy = rep_sub(base, q.y.clone(), p.y.clone());
    let same_x = lazy_zero_test(circuit, base, dx.clone());
    let same_y = lazy_zero_test(circuit, base, dy.clone());
    let finite = and_bit(
        circuit,
        lc_u64::<CS>(1).sub(p.infinity.clone()),
        lc_u64::<CS>(1).sub(q.infinity.clone()),
    );
    let doubling = and3_bit(circuit, finite.clone(), same_x.clone(), same_y.clone());
    assert_zero(circuit, doubling);
    let active = and_bit(circuit, finite.clone(), lc_u64::<CS>(1).sub(same_x.clone()));
    // Inactive cases divide 0 by 1: no unbound slope, no zero divisor.
    let numerator = select_formula(circuit, active.clone(), dy, rep_u64(0));
    let denominator = select_formula(circuit, active.clone(), dx, rep_u64(1));
    let slope = lazy_divide(circuit, base, denominator, numerator);
    let x = lazy_mul_sub_to_elem(
        circuit,
        base,
        slope.clone(),
        slope.clone(),
        rep_add(p.x.clone(), q.x.clone()),
    );
    let x = of_elem(&x);
    let y = lazy_mul_sub_to_elem(
        circuit,
        base,
        slope,
        rep_sub(base, p.x.clone(), x.clone()),
        p.y.clone(),
    );
    let y = of_elem(&y);
    // Binius64's selects: an infinite operand passes the other one through.
    let x = select_canonical(circuit, q.infinity.clone(), p.x.clone(), x);
    let x = select_canonical(circuit, p.infinity.clone(), q.x.clone(), x);
    let y = select_canonical(circuit, q.infinity.clone(), p.y.clone(), y);
    let y = select_canonical(circuit, p.infinity.clone(), q.y.clone(), y);
    let both_infinite = and_bit(circuit, p.infinity, q.infinity);
    let opposite = and3_bit(circuit, finite, same_x, lc_u64::<CS>(1).sub(same_y));
    Point {
        x,
        y,
        infinity: both_infinite.add(opposite),
    }
}

/// `[O, P, 2P, …, 15P]`: one doubling and thirteen additions in circuit.
fn build_table<CS: Circuit>(circuit: &mut CS, point: Point<CS>) -> Vec<Point<CS>> {
    let mut table = Vec::with_capacity(TABLE);
    table.push(infinity());
    table.push(point.clone());
    table.push(double_complete(circuit, &SECP256K1, point.clone()));
    for _ in 3..TABLE {
        let previous = table.last().cloned().expect("nonempty table");
        table.push(add_incomplete(circuit, previous, point.clone()));
    }
    table
}

/// `φ` applied entrywise to a table, then conditionally negated:
/// `m·(±φ(P)) = ±φ(m·P)`.
fn phi_table<CS: Circuit>(circuit: &mut CS, table: &[Point<CS>], negative: &Lc<CS>) -> Vec<Point<CS>> {
    let base = SECP256K1.base();
    table
        .iter()
        .map(|entry| {
            let x = lazy_mul(circuit, base, rep_constant(&GLV.beta), entry.x.clone());
            negate_if(
                circuit,
                negative.clone(),
                Point {
                    x,
                    y: entry.y.clone(),
                    infinity: entry.infinity.clone(),
                },
            )
        })
        .collect()
}

/// Fixed-window double-and-add over the four tables: 32 windows from the
/// top, four doublings between windows, one table addition per window and
/// base point.
fn strauss_accumulate<CS: Circuit>(
    circuit: &mut CS,
    tables: &[Vec<Point<CS>>],
    halves: &[&Split<CS>],
) -> Point<CS> {
    let mut accumulator = infinity();
    for index in (0..WINDOWS).rev() {
        if index != WINDOWS - 1 {
            for _ in 0..WINDOW {
                accumulator = double_complete(circuit, &SECP256K1, accumulator);
            }
        }
        for (table, half) in tables.iter().zip(halves) {
            let digit = window(&half.bits, index * WINDOW, WINDOW);
            let selected = lookup_point(circuit, digit, table);
            accumulator = add_incomplete(circuit, accumulator, selected);
        }
    }
    accumulator
}

/// Proves `value` invertible modulo `modulus` through a hinted inverse.
fn assert_nonzero<CS: Circuit>(circuit: &mut CS, modulus: Modulus, value: Rep<CS>) {
    let value_eval = value.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, WIDTH, WORD_LIMBS, _>(move |context| {
        let words = evaluated_uint::<HINT_LIMBS>(value_eval.evaluate_words(context), "nonzero operand")?;
        let (_, residue) = modulus.div_rem_representative(words);
        let inverse = modular_inverse_u256(residue, modulus)
            .ok_or_else(|| HintError::new("zero or noninvertible scalar"))?;
        Ok(packed_wide(inverse))
    });
    let inverse = uint_from_repr::<CS, WIDTH, WORD_LIMBS>(circuit, bits);
    lazy_assert_mul_eq(circuit, modulus, value, of_elem(&Elem { value: inverse }), rep_u64(1));
}

/// Builds the secp256k1 verifier matched to Binius64's schedule from five
/// little-endian 256-bit input words ordered as digest, Q.x, Q.y, r, s.
pub fn verify_digest_circuit<CS: Circuit>(circuit: &mut CS, inputs: &[CS::Bool; INPUT_BITS]) {
    let words: [<CS::Bool as BoolWitness>::Repr<256, 4>; 5] = array::from_fn(|slot| {
        <CS::Bool as BoolWitness>::Repr::from_array(array::from_fn(|bit| {
            inputs[slot * WIDTH + bit].clone()
        }))
    });
    let (base, scalar) = (SECP256K1.base(), SECP256K1.scalar());
    let mut words = words.into_iter();
    let digest = lift_input_word(circuit, words.next().unwrap());
    let qx = lift_input_word(circuit, words.next().unwrap());
    let qy = lift_input_word(circuit, words.next().unwrap());
    let r = lift_input_word(circuit, words.next().unwrap());
    let s = lift_input_word(circuit, words.next().unwrap());

    let qx = of_u(circuit, base, qx.elem.value);
    let qy = of_u(circuit, base, qy.elem.value);
    let r = of_u(circuit, scalar, r.elem.value);
    let s = of_u(circuit, scalar, s.elem.value);

    assert_on_curve(circuit, &SECP256K1, &qx, &qy);
    assert_nonzero(circuit, scalar, of_elem(&r));
    assert_nonzero(circuit, scalar, of_elem(&s));

    let z = relaxed_reduce_small(circuit, scalar, digest.elem.value.value);
    let u1 = lazy_divide(circuit, scalar, of_elem(&s), of_elem(&z));
    let u2 = lazy_divide(circuit, scalar, of_elem(&s), of_elem(&r));

    let (gx, gy) = SECP256K1.generator();
    let generator = Point {
        x: rep_constant(gx),
        y: rep_constant(gy),
        infinity: lc_u64(0),
    };
    let key = point_from_elems(&qx, &qy);

    let [g1, g2] = glv_split(circuit, u1);
    let [q1, q2] = glv_split(circuit, u2);
    let mut tables = Vec::with_capacity(4);
    for (point, first, second) in [(generator, &g1, &g2), (key, &q1, &q2)] {
        let signed = negate_if(circuit, first.negative.clone(), point);
        let table = build_table(circuit, signed);
        let relative = xor_bit(circuit, first.negative.clone(), second.negative.clone());
        let phi = phi_table(circuit, &table, &relative);
        tables.push(table);
        tables.push(phi);
    }
    let nonce = strauss_accumulate(circuit, &tables, &[&g1, &g2, &q1, &q2]);
    assert_zero(circuit, nonce.infinity);
    let x = lazy_reduce(circuit, base, nonce.x);
    assert_elem_eq(circuit, &x, &r);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{Dummy, LeanStats, Stats};
    use crate::witgen::{ProductWitgen, WitnessOnly};
    use crate::constraints::ConstraintGenerator;
    use k256::ecdsa::{Signature, SigningKey, signature::hazmat::PrehashSigner};
    use num_bigint::{BigInt, BigUint};

    fn biguint(value: &Uint<4>) -> BigUint {
        BigUint::from_bytes_le(
            &value
                .as_words()
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
        )
    }

    fn stored_bigint(value: &[u64]) -> BigInt {
        let bytes = value
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        BigInt::from_signed_bytes_le(&bytes)
    }

    /// Input bits for `(digest, Q.x, Q.y, r, s)` given as big-endian bytes.
    pub(crate) fn input_bits(words: [&[u8; 32]; 5]) -> Box<[bool; INPUT_BITS]> {
        let bits: Box<[bool]> = (0..INPUT_BITS)
            .map(|bit| words[bit / 256][31 - (bit % 256) / 8] >> (bit % 8) & 1 != 0)
            .collect();
        bits.try_into().unwrap()
    }

    fn signed_instance(key_bytes: [u8; 32], digest: [u8; 32]) -> [[u8; 32]; 5] {
        let key = SigningKey::from_bytes((&key_bytes).into()).unwrap();
        let signature: Signature = key.sign_prehash(&digest).unwrap();
        let point = key.verifying_key().to_encoded_point(false);
        let (r, s) = signature.split_bytes();
        [
            digest,
            (*point.x().unwrap()).into(),
            (*point.y().unwrap()).into(),
            r.into(),
            s.into(),
        ]
    }

    #[test]
    fn glv_constants_are_the_endomorphism() {
        let n = biguint(SECP256K1.scalar_modulus());
        let p = biguint(SECP256K1.base_modulus());
        let lambda = biguint(&GLV.lambda);
        let beta = biguint(&GLV.beta);
        assert_eq!(lambda.modpow(&BigUint::from(3u8), &n), BigUint::from(1u8));
        assert_eq!(beta.modpow(&BigUint::from(3u8), &p), BigUint::from(1u8));
        assert_ne!(lambda, BigUint::from(1u8));
        assert_ne!(beta, BigUint::from(1u8));
        // λ·G = (β·G.x, G.y): the scalar and coordinate constants belong to
        // the same endomorphism.
        use k256::elliptic_curve::{PrimeField, sec1::ToEncodedPoint};
        let mut be = [0u8; 32];
        for (i, word) in GLV.lambda.as_words().iter().rev().enumerate() {
            be[8 * i..8 * i + 8].copy_from_slice(&word.to_be_bytes());
        }
        let lambda_scalar = k256::Scalar::from_repr(be.into()).unwrap();
        let point = (k256::ProjectivePoint::GENERATOR * lambda_scalar).to_affine();
        let encoded = point.to_encoded_point(false);
        let (gx, gy) = SECP256K1.generator();
        assert_eq!(BigUint::from_bytes_be(encoded.x().unwrap()), biguint(gx) * &beta % &p);
        assert_eq!(BigUint::from_bytes_be(encoded.y().unwrap()), biguint(gy));
    }

    #[test]
    fn decomposition_recombines_with_short_halves() {
        let n = biguint(SECP256K1.scalar_modulus());
        let lambda = biguint(&GLV.lambda);
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut scalars: Vec<Uint<4>> = vec![
            Uint::from_u64(1),
            Uint::from_u64(2),
            SECP256K1.scalar_modulus().wrapping_sub(&Uint::from_u64(1)),
            GLV.lambda,
            Uint::from_words([u64::MAX; 4]),
        ];
        for _ in 0..2_000 {
            scalars.push(Uint::from_words(array::from_fn(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            })));
        }
        for k in scalars {
            let (negative1, negative2, k1, k2) = endosplit(k).unwrap();
            let k1 = biguint(&k1.zero_extend());
            let k2 = biguint(&k2.zero_extend());
            assert!(k1.bits() <= 128 && k2.bits() <= 128);
            let sign = |negative: bool, value: &BigUint| if negative { &n - value } else { value.clone() };
            let recombined = (sign(negative1, &k1) + sign(negative2, &k2) * &lambda) % &n;
            assert_eq!(recombined, biguint(&k) % &n);
        }
    }

    #[test]
    fn verifier_dimensions_are_pinned() {
        let mut stats = Stats::new(INPUT_BITS);
        verify_digest_circuit(&mut stats, &[Dummy; INPUT_BITS]);
        assert_eq!(
            stats.lean_stats(),
            LeanStats {
                m_rows: INTEGER_WITNESS_BITS,
                m_cols: WITNESS_BITS + 1,
                r1cs_rows: R1CS_ROWS,
            }
        );
    }

    #[test]
    fn third_party_signatures_satisfy_the_circuit_with_the_pinned_witness_size() {
        for (key, digest) in [([17u8; 32], [9u8; 32]), ([2u8; 32], [255u8; 32]), ([0xa5; 32], [0; 32])] {
            let words = signed_instance(key, digest);
            let inputs = input_bits(array::from_fn(|i| &words[i]));
            let mut witness_only = WitnessOnly::with_inputs_and_capacity(inputs.as_ref(), WITNESS_BITS);
            verify_digest_circuit(&mut witness_only, &inputs);
            assert_eq!(witness_only.witness().bit_len(), WITNESS_BITS);
            let mut witgen = ProductWitgen::with_inputs_and_capacity(inputs.as_ref(), WITNESS_BITS);
            verify_digest_circuit(&mut witgen, &inputs);
            assert_eq!(witness_only.witness(), witgen.witness());
            assert_eq!(witgen.integer_witness().bit_len(), INTEGER_WITNESS_BITS);
            assert_eq!(witgen.products().a_mw.len(), R1CS_ROWS);
            for ((a, b), c) in witgen
                .products()
                .a_mw
                .iter()
                .zip(witgen.products().b_mw.iter())
                .zip(witgen.products().c_mw.iter())
            {
                assert_eq!(stored_bigint(a) * stored_bigint(b), stored_bigint(c));
            }
        }
    }

    #[test]
    fn both_s_forms_verify_and_wrong_inputs_are_rejected_by_the_constraints() {
        let mut generator = ConstraintGenerator::new(INPUT_BITS);
        let inputs = generator.boxed_inputs::<INPUT_BITS>();
        verify_digest_circuit(&mut generator, &inputs);
        let matrices = generator.into_matrices();
        let n = biguint(SECP256K1.scalar_modulus());
        let words = signed_instance([17u8; 32], [42u8; 32]);
        let alternate_s = {
            let s = &n - BigUint::from_bytes_be(&words[4]);
            let bytes = s.to_bytes_be();
            let mut out = [0u8; 32];
            out[32 - bytes.len()..].copy_from_slice(&bytes);
            out
        };
        for s in [words[4], alternate_s] {
            let mut instance = words;
            instance[4] = s;
            let inputs = input_bits(array::from_fn(|i| &instance[i]));
            let mut witgen = ProductWitgen::with_inputs_and_capacity(inputs.as_ref(), WITNESS_BITS);
            verify_digest_circuit(&mut witgen, &inputs);
            matrices.check_witness(witgen.witness()).unwrap();
        }
        // A wrong signature or key builds a complete witness whose final
        // coordinate equation fails; a P-256 instance fails its curve check.
        for (slot, delta) in [(1, 1u8), (3, 1), (4, 1), (0, 1)] {
            let mut instance = words;
            instance[slot][31] ^= delta;
            let inputs = input_bits(array::from_fn(|i| &instance[i]));
            let mut witgen = ProductWitgen::with_inputs_and_capacity(inputs.as_ref(), WITNESS_BITS);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                verify_digest_circuit(&mut witgen, &inputs);
                matrices.check_witness(witgen.witness()).is_ok()
            }));
            assert!(!matches!(outcome, Ok(true)), "accepted a corrupted slot {slot}");
        }
    }
}
