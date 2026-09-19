//! ECDSA-P256 verification over a prehashed 256-bit digest.
//!
//! This is a direct circuit-shape port of Freigen's Lean implementation.  It
//! uses the same lazy integer representatives, quotient widths, complete
//! affine formulas, and joint fixed/variable-base scalar multiplication.

use std::array;
use std::sync::OnceLock;

use field::{
    CheckedArithmetic, CtEq, CtMask, CtSelect, IntegerEmbedding, IntegerOps, ModRingCtx,
    PreparedDivisor, PreparedOddInverse, RingOps, Uint, WideMul,
};
#[cfg(test)]
use num_bigint::BigUint;
use num_traits::{One, Zero};

use crate::{
    BoolRepresentation, BoolWitness, Circuit, HintError, HintResult, PackedBits, WitnessContext,
};

/// Number of Boolean inputs: digest, public-key coordinates, signature
/// scalars, and the two inverse witnesses.
pub const VERIFY_DIGEST_INPUT_BITS: usize = 7 * 256;

/// Total Boolean witness size of the standalone verifier, including inputs.
pub const VERIFY_DIGEST_WITNESS_BITS: usize = 1_068_663;

/// Number of packed `M * w` bits, including the implicit constant one.
pub const VERIFY_DIGEST_INTEGER_WITNESS_BITS: usize = 1_068_664;

/// Number of rank-1 constraints.
pub const VERIFY_DIGEST_R1CS_ROWS: usize = 6_030;

/// The same three dimensions over secp256k1, where [`joint_scalar_mul`] takes
/// the GLV path: the doubling chain halves while additions and table lookups
/// are unchanged, because the scalar material absorbed per round is the same.
pub const SECP256K1_VERIFY_DIGEST_WITNESS_BITS: usize = 829_743;
pub const SECP256K1_VERIFY_DIGEST_INTEGER_WITNESS_BITS: usize = 829_744;
pub const SECP256K1_VERIFY_DIGEST_R1CS_ROWS: usize = 5_606;

/// Signed width used by witness-oriented backends for P-256 intermediates.
/// The widest values are products of 262-bit affine-formula operands.
pub const P256_Z_LIMBS: usize = 9;
// Hint operands are nonnegative representatives below bound * modulus. The
// public bound is a u64 and both moduli are 256-bit, so five limbs suffice.
// Stored circuit integers and the constraint/transcript widths remain nine.
const HINT_LIMBS: usize = 5;

const WIDTH: usize = 256;
const WORD_LIMBS: usize = 4;

type P256Z<CS> = <CS as Circuit>::Z<P256_Z_LIMBS>;
type P256Coefficient<CS> = <CS as Circuit>::Coefficient<P256_Z_LIMBS>;

/// The short-Weierstrass `a` coefficient, restricted to the two shapes this
/// circuit implements. It is not a free parameter: the doubling numerator and
/// the fudge term that keeps the slope well defined at the infinity encoding
/// `(0, 0)` both depend on it. See [`Curve::double_numerator`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoefficientA {
    /// `a = 0` (secp256k1). `3x^2 + a` already vanishes at the encoding.
    Zero,
    /// `a = -3` (P-256). Needs the `-3` term and the infinity fudge.
    MinusThree,
}

/// GLV parameters for a curve carrying an efficiently computable endomorphism
/// `phi(x, y) = (beta*x, y)` that acts on the group as `phi(P) = lambda*P`.
///
/// `a1 + lambda*b1 = a2 + lambda*b2 = 0 (mod n)` is a short lattice basis, so
/// rounding `k` against it splits any scalar into two halves below `2^128`.
/// `b1` is negative and stored as its magnitude; `b2` equals `a1`.
pub struct Endomorphism {
    beta: Uint<4>,
    lambda: Uint<4>,
    a1: Uint<4>,
    minus_b1: Uint<4>,
    a2: Uint<4>,
    /// `ceil(n/2)`: the round-half-up threshold, and the sign cut-off that
    /// tells a small positive residue from a small negative one.
    half_n: Uint<4>,
    phi_generator_table: OnceLock<Vec<(Uint<4>, Uint<4>)>>,
}

/// A prime-order short-Weierstrass curve `y^2 = x^3 + a*x + b` over `F_p`,
/// together with the prepared arithmetic and the public fixed-base table that
/// witness generation needs. Both supported curves have a 256-bit `p` and `n`
/// and cofactor one, which is what fixes every hint width in this module.
pub struct Curve {
    name: &'static str,
    base: Uint<4>,
    scalar: Uint<4>,
    b: Uint<4>,
    generator: (Uint<4>, Uint<4>),
    a: CoefficientA,
    endomorphism: Option<Endomorphism>,
    base_divisor: OnceLock<PreparedDivisor<4>>,
    scalar_divisor: OnceLock<PreparedDivisor<4>>,
    base_inverse: OnceLock<PreparedOddInverse<4>>,
    scalar_inverse: OnceLock<PreparedOddInverse<4>>,
    generator_table: OnceLock<Vec<(Uint<4>, Uint<4>)>>,
    /// `(R, -(2^128 R), -(2^256 R))`: the ladder's starting offset and the
    /// corrections for the GLV and plain schedules. See [`Curve::offsets`].
    offsets: OnceLock<[(Uint<4>, Uint<4>); 3]>,
}

/// NIST P-256 (secp256r1), `a = -3`.
pub static P256: Curve = Curve::new(
    "P-256",
    Uint::from_words([
        0xffffffffffffffff,
        0x00000000ffffffff,
        0x0000000000000000,
        0xffffffff00000001,
    ]),
    Uint::from_words([
        0xf3b9cac2fc632551,
        0xbce6faada7179e84,
        0xffffffffffffffff,
        0xffffffff00000000,
    ]),
    Uint::from_words([
        0x3bce3c3e27d2604b,
        0x651d06b0cc53b0f6,
        0xb3ebbd55769886bc,
        0x5ac635d8aa3a93e7,
    ]),
    (
        Uint::from_words([
            0xf4a13945d898c296,
            0x77037d812deb33a0,
            0xf8bce6e563a440f2,
            0x6b17d1f2e12c4247,
        ]),
        Uint::from_words([
            0xcbb6406837bf51f5,
            0x2bce33576b315ece,
            0x8ee7eb4a7c0f9e16,
            0x4fe342e2fe1a7f9b,
        ]),
    ),
    CoefficientA::MinusThree,
    None,
);

/// secp256k1, `a = 0`, `b = 7`. Unlike P-256 it admits the efficient
/// endomorphism `(x, y) -> (beta*x, y)`, which [`joint_scalar_mul`] exploits.
pub static SECP256K1: Curve = Curve::new(
    "secp256k1",
    Uint::from_words([
        0xfffffffefffffc2f,
        0xffffffffffffffff,
        0xffffffffffffffff,
        0xffffffffffffffff,
    ]),
    Uint::from_words([
        0xbfd25e8cd0364141,
        0xbaaedce6af48a03b,
        0xfffffffffffffffe,
        0xffffffffffffffff,
    ]),
    Uint::from_words([7, 0, 0, 0]),
    (
        Uint::from_words([
            0x59f2815b16f81798,
            0x029bfcdb2dce28d9,
            0x55a06295ce870b07,
            0x79be667ef9dcbbac,
        ]),
        Uint::from_words([
            0x9c47d08ffb10d4b8,
            0xfd17b448a6855419,
            0x5da4fbfc0e1108a8,
            0x483ada7726a3c465,
        ]),
    ),
    CoefficientA::Zero,
    Some(Endomorphism {
        beta: Uint::from_words([
            0xc1396c28719501ee,
            0x9cf0497512f58995,
            0x6e64479eac3434e9,
            0x7ae96a2b657c0710,
        ]),
        lambda: Uint::from_words([
            0xdf02967c1b23bd72,
            0x122e22ea20816678,
            0xa5261c028812645a,
            0x5363ad4cc05c30e0,
        ]),
        a1: Uint::from_words([0xe86c90e49284eb15, 0x3086d221a7d46bcd, 0, 0]),
        minus_b1: Uint::from_words([0x6f547fa90abfe4c3, 0xe4437ed6010e8828, 0, 0]),
        a2: Uint::from_words([0x57c1108d9d44cfd8, 0x14ca50f7a8e2f3f6, 1, 0]),
        half_n: Uint::from_words([
            0xdfe92f46681b20a1,
            0x5d576e7357a4501d,
            0xffffffffffffffff,
            0x7fffffffffffffff,
        ]),
        phi_generator_table: OnceLock::new(),
    }),
);

impl Endomorphism {
    /// `round(v * k / n)`, half-up, as a five-limb integer. `v` is at most 129
    /// bits and `k` is below `n`, so the quotient stays under `2^130`.
    fn rounded_quotient(&self, n: Modulus, v: &Uint<4>, k: &Uint<4>) -> Uint<HINT_LIMBS> {
        use field::CtOrd;
        let product = multiply_wide(v.zero_extend(), k.zero_extend());
        let (quotient, remainder) = n.divisor().div_rem_ct(&product);
        let round_up = !remainder.ct_lt(&self.half_n);
        let bumped = quotient.wrapping_add(&Uint::<10>::ONE);
        let rounded = Uint::ct_select(&quotient, &bumped, round_up);
        Uint::from_words(array::from_fn(|i| rounded.as_words()[i]))
    }

    /// Splits `k` into magnitudes below `2^128` and signs, such that
    /// `(-1)^s1 * m1 + lambda * (-1)^s2 * m2 = k (mod n)`.
    ///
    /// Only the prover runs this; the circuit re-checks the relation and the
    /// magnitudes, so a wrong split yields an unsatisfiable witness, never an
    /// accepted proof.
    fn decompose(&self, n: Modulus, k: &Uint<4>) -> (Uint<4>, bool, Uint<4>, bool) {
        use field::CtOrd;
        let modulus = n.words();
        // b2 == a1, so the first rounding reuses a1.
        let c1 = self.rounded_quotient(n, &self.a1, k);
        let c2 = self.rounded_quotient(n, &self.minus_b1, k);
        let reduce = |wide: &Uint<HINT_LIMBS>, factor: &Uint<4>| -> Uint<4> {
            n.divisor()
                .div_rem_ct(&multiply_wide(*wide, factor.zero_extend()))
                .1
        };
        let sub_mod = |left: &Uint<4>, right: &Uint<4>| -> Uint<4> {
            let difference = left.checked_sub_ct(right);
            let wrapped = left.wrapping_add(&modulus).wrapping_sub(right);
            Uint::ct_select(&wrapped, difference.value(), difference.validity())
        };
        let k1 = sub_mod(&sub_mod(k, &reduce(&c1, &self.a1)), &reduce(&c2, &self.a2));
        let k2 = sub_mod(&reduce(&c1, &self.minus_b1), &reduce(&c2, &self.a1));
        // A residue above n/2 is a small negative value in disguise.
        let split = |value: Uint<4>| {
            let negative = !value.ct_lt(&self.half_n);
            let magnitude = Uint::ct_select(&value, &modulus.wrapping_sub(&value), negative);
            (magnitude, negative.declassify())
        };
        let (m1, s1) = split(k1);
        let (m2, s2) = split(k2);
        (m1, s1, m2, s2)
    }

    /// The public table `i*phi(G) = phi(i*G) = (beta*x_i, y_i)`, built by
    /// scaling the generator table rather than a second addition chain.
    fn phi_generator_table(&'static self, curve: &'static Curve) -> &'static Vec<(Uint<4>, Uint<4>)> {
        self.phi_generator_table.get_or_init(|| {
            let ring = ModRingCtx::new(curve.base).expect("curve base modulus");
            let beta = ring.from_integer(&self.beta);
            curve
                .generator_table()
                .iter()
                .map(|(x, y)| {
                    (
                        ring.to_integer(&ring.mul(&beta, &ring.from_integer(x))),
                        *y,
                    )
                })
                .collect()
        })
    }
}

impl Curve {
    const fn new(
        name: &'static str,
        base: Uint<4>,
        scalar: Uint<4>,
        b: Uint<4>,
        generator: (Uint<4>, Uint<4>),
        a: CoefficientA,
        endomorphism: Option<Endomorphism>,
    ) -> Self {
        Self {
            name,
            base,
            scalar,
            b,
            generator,
            a,
            endomorphism,
            base_divisor: OnceLock::new(),
            scalar_divisor: OnceLock::new(),
            base_inverse: OnceLock::new(),
            scalar_inverse: OnceLock::new(),
            generator_table: OnceLock::new(),
            offsets: OnceLock::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Whether [`verify_digest_circuit_on`] takes the GLV path on this curve.
    pub fn has_endomorphism(&self) -> bool {
        self.endomorphism.is_some()
    }

    /// Boolean witness bits the verifier allocates on this curve.
    pub fn verify_digest_witness_bits(&self) -> usize {
        if self.has_endomorphism() {
            SECP256K1_VERIFY_DIGEST_WITNESS_BITS
        } else {
            VERIFY_DIGEST_WITNESS_BITS
        }
    }

    fn base(&'static self) -> Modulus {
        Modulus {
            curve: self,
            field: Field::Base,
        }
    }

    fn scalar(&'static self) -> Modulus {
        Modulus {
            curve: self,
            field: Field::Scalar,
        }
    }

    /// `3x^2 + a` as a lazy representative, plus the infinity fudge when the
    /// caller is doubling in place (`at_infinity` is the point's flag).
    ///
    /// At the infinity encoding `(0, 0)` the denominator is forced to 1, so the
    /// numerator must vanish for the slope to be 0. With `a = -3` that needs a
    /// compensating `+3*infinity`; with `a = 0` the term `3*0^2` is already 0
    /// and adding the fudge would wrongly make the slope nonzero.
    fn double_numerator<CS: Circuit>(
        &'static self,
        x_squared: Rep<CS>,
        at_infinity: Option<Lc<CS>>,
    ) -> Rep<CS> {
        let scaled = rep_scale(3, x_squared);
        match self.a {
            CoefficientA::Zero => scaled,
            CoefficientA::MinusThree => {
                let shifted = rep_sub(self.base(), scaled, rep_u64::<CS>(3));
                match at_infinity {
                    None => shifted,
                    Some(flag) => rep_add(
                        shifted,
                        Rep {
                            value: flag.scale_coefficient(P256Coefficient::<CS>::from(3_u64)),
                            bound: 1,
                        },
                    ),
                }
            }
        }
    }

    /// `x^3 + a*x + b` as a lazy representative.
    fn curve_equation_rhs<CS: Circuit>(
        &'static self,
        x_cubed: Rep<CS>,
        x: Rep<CS>,
        b: Rep<CS>,
    ) -> Rep<CS> {
        let sum = rep_add(x_cubed, b);
        match self.a {
            CoefficientA::Zero => sum,
            CoefficientA::MinusThree => rep_sub(self.base(), sum, rep_scale(3, x)),
        }
    }

    /// Native counterpart of [`Curve::double_numerator`], for the public table.
    fn native_double_numerator(
        &self,
        ring: &ModRingCtx<4>,
        x: &field::Residue<4>,
    ) -> field::Residue<4> {
        let three = ring.from_integer(&3u64);
        let scaled = ring.mul(&three, &ring.mul(x, x));
        match self.a {
            CoefficientA::Zero => scaled,
            CoefficientA::MinusThree => ring.sub(&scaled, &three),
        }
    }

    /// Native `k*G` by MSB-first double-and-add, for public constants only.
    fn native_generator_multiple(
        &self,
        k: &Uint<4>,
        ring: &ModRingCtx<4>,
        inverse: &PreparedOddInverse<4>,
    ) -> (Uint<4>, Uint<4>) {
        let mut accumulator: Option<(Uint<4>, Uint<4>)> = None;
        for bit in (0..256).rev() {
            if let Some(point) = accumulator {
                accumulator = Some(self.affine_add(&point, &point, ring, inverse));
            }
            if k.as_words()[bit / 64] >> (bit % 64) & 1 == 1 {
                accumulator = Some(match accumulator {
                    None => self.generator,
                    Some(point) => self.affine_add(&point, &self.generator, ring, inverse),
                });
            }
        }
        accumulator.expect("offset scalar is nonzero")
    }

    /// `(R, -(2^128 R), -(2^256 R))`.
    ///
    /// The ladders start at `R` rather than the identity. Starting at the
    /// identity walks the accumulator through *small* multiples of the bases,
    /// and then a public key that is itself a small multiple of `G` makes
    /// `acc = +-T` likely rather than a `2^-128` accident — which is what
    /// [`add_distinct`] must exclude. `R` is a nothing-up-my-sleeve multiple of
    /// `G`, so no honest instance lands in that regime.
    ///
    /// A schedule of `d` doublings leaves `2^d R` added to the true sum, so the
    /// ladder finishes by adding the matching negated constant.
    fn offsets(&'static self) -> &'static [(Uint<4>, Uint<4>); 3] {
        self.offsets.get_or_init(|| {
            let ring = ModRingCtx::new(self.base).expect("curve base modulus");
            let inverse = self.base().inverse();
            let seed = blake3::hash(
                format!("bitz/ecdsa/accumulator-offset/v1/{}", self.name).as_bytes(),
            );
            let mut words = [0u64; 4];
            for (index, word) in words.iter_mut().enumerate() {
                *word = u64::from_le_bytes(
                    seed.as_bytes()[index * 8..index * 8 + 8]
                        .try_into()
                        .expect("eight-byte limb"),
                );
            }
            // Reduce into `[1, n)`; the scalar only has to be structureless.
            let (_, scalar) = self.scalar().divisor().div_rem_ct(&Uint::from_words(words));
            let base = self.native_generator_multiple(&scalar, &ring, &inverse);
            let negate = |(x, y): (Uint<4>, Uint<4>)| {
                (x, ring.to_integer(&ring.sub(&ring.from_integer(&Uint::<4>::ZERO), &ring.from_integer(&y))))
            };
            let mut point = base;
            let mut corrections = Vec::with_capacity(2);
            for doublings in 1..=256 {
                point = self.affine_add(&point, &point, &ring, &inverse);
                if doublings == 128 || doublings == 256 {
                    corrections.push(negate(point));
                }
            }
            [base, corrections[0], corrections[1]]
        })
    }

    /// The public fixed-base table `i*G` for `i` in `[0, 256)`, built once.
    fn generator_table(&'static self) -> &'static Vec<(Uint<4>, Uint<4>)> {
        self.generator_table.get_or_init(|| {
            let ring = ModRingCtx::new(self.base).expect("curve base modulus");
            let inverse = self.base().inverse();
            let mut table = Vec::with_capacity(256);
            table.push((Uint::ZERO, Uint::ZERO));
            let mut point = self.generator;
            table.push(point);
            for _ in 2..256 {
                point = self.affine_add(&point, &self.generator, &ring, inverse);
                table.push(point);
            }
            table
        })
    }

    // Only used to prepare public fixed-base constants.
    fn affine_add(
        &self,
        left: &(Uint<4>, Uint<4>),
        right: &(Uint<4>, Uint<4>),
        ring: &ModRingCtx<4>,
        inverse: &PreparedOddInverse<4>,
    ) -> (Uint<4>, Uint<4>) {
        let (lx, ly) = (ring.from_integer(&left.0), ring.from_integer(&left.1));
        let (rx, ry) = (ring.from_integer(&right.0), ring.from_integer(&right.1));
        let (numerator, denominator) = if left == right {
            (self.native_double_numerator(ring, &lx), ring.add(&ly, &ly))
        } else {
            (ring.sub(&ry, &ly), ring.sub(&rx, &lx))
        };
        let reciprocal = inverse.inverse_ct(&ring.to_integer(&denominator));
        assert!(
            reciprocal.validity().declassify(),
            "public generator table denominator is nonzero"
        );
        let slope = ring.mul(&numerator, &ring.from_integer(reciprocal.value()));
        let x = ring.sub(&ring.sub(&ring.mul(&slope, &slope), &lx), &rx);
        let y = ring.sub(&ring.mul(&slope, &ring.sub(&lx, &x)), &ly);
        (ring.to_integer(&x), ring.to_integer(&y))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Base,
    Scalar,
}

/// One of a curve's two 256-bit moduli, carrying the curve it belongs to so a
/// single code path serves every supported curve.
#[derive(Clone, Copy)]
struct Modulus {
    curve: &'static Curve,
    field: Field,
}

impl Modulus {
    fn value(self) -> &'static Uint<4> {
        match self.field {
            Field::Base => &self.curve.base,
            Field::Scalar => &self.curve.scalar,
        }
    }

    fn words(self) -> Uint<4> {
        *self.value()
    }

    fn divisor(self) -> &'static PreparedDivisor<4> {
        let cache = match self.field {
            Field::Base => &self.curve.base_divisor,
            Field::Scalar => &self.curve.scalar_divisor,
        };
        cache.get_or_init(|| PreparedDivisor::new(self.words()).expect("curve modulus is nonzero"))
    }

    /// Divide any five-limb integer using the curve's two public moduli.
    /// With R = 2^256 and p = R - C, every supported modulus has 0 < C < 2^224.
    /// Each fold replaces h*R + low by h*C + low and adds h to the quotient.
    #[inline]
    fn div_rem_representative(self, value: Uint<HINT_LIMBS>) -> (Uint<HINT_LIMBS>, Uint<4>) {
        let modulus = self.words();
        let complement = modulus.wrapping_neg();
        let mut words = *value.as_words();
        let mut quotient = 0u128;
        // The successive bounds are R + 2^288, 2R, then R + C < 2p.
        // Three folds and one masked subtraction therefore suffice, even for
        // inputs wider than the representatives produced by this circuit.
        for _ in 0..3 {
            let high = words[4];
            quotient += high as u128;
            let mut carry = 0u128;
            for (word, coefficient) in words[..4].iter_mut().zip(complement.as_words()) {
                // A word product plus two word-sized addends fits in u128.
                let sum = high as u128 * *coefficient as u128 + *word as u128 + carry;
                *word = sum as u64;
                carry = sum >> 64;
            }
            words[4] = carry as u64;
        }
        let folded = Uint::from_words(words);
        let difference = folded.checked_sub_ct(&modulus.zero_extend());
        let remainder = Uint::ct_select(&folded, difference.value(), difference.validity());
        quotient += u64::ct_select(&0, &1, difference.validity()) as u128;
        // The quotient is below 2^64 + 2^32 + 3; the remainder is below p.
        (
            Uint::from_words([quotient as u64, (quotient >> 64) as u64, 0, 0, 0]),
            Uint::from_words(array::from_fn(|i| remainder.as_words()[i])),
        )
    }

    fn inverse(self) -> &'static PreparedOddInverse<4> {
        let cache = match self.field {
            Field::Base => &self.curve.base_inverse,
            Field::Scalar => &self.curve.scalar_inverse,
        };
        cache.get_or_init(|| PreparedOddInverse::new(self.words()).expect("curve modulus is odd"))
    }
}

/// Invert a canonical, nonzero P-256 signature scalar using fixed public bounds.
/// Invalid inputs execute the same inverse schedule and return a false mask.
pub fn scalar_inverse_ct(curve: &'static Curve, value: &Uint<4>) -> field::CtValue<Uint<4>> {
    use field::{CtEq, CtOrd};
    let inverse = curve.scalar().inverse().inverse_ct(value);
    let valid = value.ct_lt(&curve.scalar) & !value.ct_is_zero() & inverse.validity();
    field::CtValue::new(*inverse.value(), valid)
}

struct Lc<CS: Circuit> {
    z: P256Z<CS>,
}

#[derive(Clone)]
struct CapturedValue<ZW> {
    z: ZW,
}

impl<ZW> CapturedValue<ZW> {
    fn evaluate_words<'a, BW, C>(&'a self, context: &dyn WitnessContext<ZW, BW, C>) -> &'a [u64] {
        context
            .eval_z_words(&self.z)
            .expect("fixed-width integer hint context required by the P-256 circuit")
    }
}

impl<CS: Circuit> Clone for Lc<CS> {
    fn clone(&self) -> Self {
        Self { z: self.z.clone() }
    }
}

impl<CS: Circuit> Lc<CS> {
    fn capture(&self) -> CapturedValue<P256Z<CS>> {
        CapturedValue { z: self.z.clone() }
    }

    fn add(self, rhs: Self) -> Self {
        Self { z: self.z + rhs.z }
    }

    fn sub(self, rhs: Self) -> Self {
        Self { z: self.z - rhs.z }
    }

    fn scale(self, coefficient: &Uint<4>) -> Self {
        self.scale_coefficient(coefficient_for::<CS>(coefficient))
    }

    fn scale_coefficient(self, coefficient: P256Coefficient<CS>) -> Self {
        Self {
            z: self.z * coefficient,
        }
    }
}

struct UInt<CS: Circuit, const N: usize, const M: usize> {
    bits: <CS::Bool as BoolWitness>::Repr<N, M>,
    value: Lc<CS>,
}

impl<CS: Circuit, const N: usize, const M: usize> Clone for UInt<CS, N, M> {
    fn clone(&self) -> Self {
        Self {
            bits: self.bits.clone(),
            value: self.value.clone(),
        }
    }
}

type UInt256<CS> = UInt<CS, WIDTH, WORD_LIMBS>;

struct Elem<CS: Circuit> {
    value: UInt256<CS>,
}

impl<CS: Circuit> Clone for Elem<CS> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

struct Rep<CS: Circuit> {
    value: Lc<CS>,
    bound: usize,
}

impl<CS: Circuit> Clone for Rep<CS> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            bound: self.bound,
        }
    }
}

struct Point<CS: Circuit> {
    x: Rep<CS>,
    y: Rep<CS>,
    infinity: Lc<CS>,
}

impl<CS: Circuit> Clone for Point<CS> {
    fn clone(&self) -> Self {
        Self {
            x: self.x.clone(),
            y: self.y.clone(),
            infinity: self.infinity.clone(),
        }
    }
}

fn coefficient_for<CS: Circuit>(value: &Uint<4>) -> P256Coefficient<CS> {
    let mut words = [0; P256_Z_LIMBS];
    for (output, word) in words.iter_mut().zip(value.as_words().iter().copied()) {
        *output = word;
    }
    CS::coefficient_from_le_words(&words)
}

fn lc_constant<CS: Circuit>(value: &Uint<4>) -> Lc<CS> {
    Lc {
        z: P256Z::<CS>::from(coefficient_for::<CS>(value)),
    }
}

fn lc_words<CS: Circuit>(words: &[u64]) -> Lc<CS> {
    Lc {
        z: P256Z::<CS>::from(CS::coefficient_from_le_words(words)),
    }
}

fn lc_u64<CS: Circuit>(value: u64) -> Lc<CS> {
    Lc {
        z: P256Z::<CS>::from(P256Coefficient::<CS>::from(value)),
    }
}

fn assert_zero<CS: Circuit>(circuit: &mut CS, value: Lc<CS>) {
    circuit.assert_r1c::<P256_Z_LIMBS>(P256Z::<CS>::zero(), P256Z::<CS>::zero(), value.z);
}

fn repr_bits<BW, const N: usize, const M: usize>(repr: &BW::Repr<N, M>) -> Vec<BW>
where
    BW: BoolWitness,
{
    (0..N).map(|index| repr.bit(index)).collect()
}

fn uint_from_repr<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    bits: <CS::Bool as BoolWitness>::Repr<N, M>,
) -> UInt<CS, N, M> {
    let (z, _) = circuit.bitz_unsigned::<P256_Z_LIMBS, N, M, N>(&bits);
    UInt {
        bits,
        value: Lc { z },
    }
}

fn uint_from_repr_with_lifts<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    bits: <CS::Bool as BoolWitness>::Repr<N, M>,
) -> (UInt<CS, N, M>, Vec<Lc<CS>>) {
    let handles = repr_bits::<CS::Bool, N, M>(&bits);
    let mut lifted = Vec::with_capacity(N);
    let mut z = P256Z::<CS>::zero();
    let mut power = P256Coefficient::<CS>::one();
    for bit in handles {
        let bit_z = circuit.bitz::<P256_Z_LIMBS>(bit);
        z += bit_z.clone() * power.clone();
        lifted.push(Lc { z: bit_z });
        power += power.clone();
    }
    (
        UInt {
            bits,
            value: Lc { z },
        },
        lifted,
    )
}

fn hinted_bits<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    expression: CapturedValue<P256Z<CS>>,
    description: &'static str,
) -> <CS::Bool as BoolWitness>::Repr<N, M> {
    circuit.hint::<P256_Z_LIMBS, N, M, _>(move |context| {
        packed_evaluated(expression.evaluate_words(context), description)
    })
}

fn uint_from_int<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    value: Lc<CS>,
    description: &'static str,
) -> UInt<CS, N, M> {
    let bits = hinted_bits::<CS, N, M>(circuit, value.capture(), description);
    let output = uint_from_repr(circuit, bits);
    assert_zero(circuit, value.sub(output.value.clone()));
    output
}

fn assert_lt<CS: Circuit>(circuit: &mut CS, modulus: Modulus, value: &UInt256<CS>) {
    let bound = modulus.words().wrapping_sub(&Uint::ONE);
    let slack = lc_words::<CS>(bound.as_words()).sub(value.value.clone());
    let _ = uint_from_int::<CS, WIDTH, WORD_LIMBS>(circuit, slack, "range-check slack");
}

fn of_u<CS: Circuit>(circuit: &mut CS, modulus: Modulus, value: UInt256<CS>) -> Elem<CS> {
    assert_lt(circuit, modulus, &value);
    Elem { value }
}

fn of_elem<CS: Circuit>(value: &Elem<CS>) -> Rep<CS> {
    Rep {
        value: value.value.value.clone(),
        bound: 2,
    }
}

fn rep_constant<CS: Circuit>(value: &Uint<4>) -> Rep<CS> {
    Rep {
        value: lc_constant(value),
        bound: 2,
    }
}

fn rep_u64<CS: Circuit>(value: u64) -> Rep<CS> {
    Rep {
        value: lc_u64(value),
        bound: 2,
    }
}

fn rep_add<CS: Circuit>(left: Rep<CS>, right: Rep<CS>) -> Rep<CS> {
    Rep {
        value: left.value.add(right.value),
        bound: left.bound + right.bound,
    }
}

fn rep_sub<CS: Circuit>(modulus: Modulus, left: Rep<CS>, right: Rep<CS>) -> Rep<CS> {
    let bias = modulus_multiple(modulus, right.bound);
    Rep {
        value: left.value.add(lc_words(bias.as_words())).sub(right.value),
        bound: left.bound + right.bound,
    }
}

fn rep_scale<CS: Circuit>(coefficient: usize, value: Rep<CS>) -> Rep<CS> {
    Rep {
        value: value
            .value
            .scale_coefficient(P256Coefficient::<CS>::from(coefficient as u64)),
        bound: coefficient * value.bound,
    }
}

fn split_521<CS: Circuit>(
    circuit: &mut CS,
    bits: <CS::Bool as BoolWitness>::Repr<521, 9>,
) -> (UInt256<CS>, UInt<CS, 265, 5>) {
    let remainder = bits.slice::<256, 4>(0);
    let quotient = bits.slice::<265, 5>(WIDTH);
    (
        uint_from_repr::<CS, WIDTH, WORD_LIMBS>(circuit, remainder),
        uint_from_repr::<CS, 265, 5>(circuit, quotient),
    )
}

fn lazy_mul<CS: Circuit>(circuit: &mut CS, modulus: Modulus, x: Rep<CS>, y: Rep<CS>) -> Rep<CS> {
    let x_eval = x.value.capture();
    let y_eval = y.value.capture();
    let divisor = modulus.divisor();
    let bits = circuit.hint::<P256_Z_LIMBS, 521, 9, _>(move |context| {
        let a = evaluated_uint::<HINT_LIMBS>(
            x_eval.evaluate_words(context),
            "lazy multiplication operand",
        )?;
        let b = evaluated_uint::<HINT_LIMBS>(
            y_eval.evaluate_words(context),
            "lazy multiplication operand",
        )?;
        let product = multiply_wide(a, b);
        let (quotient, remainder) = divisor.div_rem_ct(&product);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let (r, q) = split_521(circuit, bits);
    let rhs = r.value.clone().add(q.value.clone().scale(modulus.value()));
    circuit.assert_r1c::<P256_Z_LIMBS>(x.value.z, y.value.z, rhs.z);
    Rep {
        value: r.value,
        bound: 2,
    }
}

fn lazy_mul_sub_to_elem<CS: Circuit>(
    circuit: &mut CS,
    modulus: Modulus,
    x: Rep<CS>,
    y: Rep<CS>,
    target: Rep<CS>,
) -> Elem<CS> {
    let bias = modulus_multiple(modulus, target.bound);
    let x_eval = x.value.capture();
    let y_eval = y.value.capture();
    let target_eval = target.value.capture();
    let divisor = modulus.divisor();
    let hint_bias = bias;
    let bits = circuit.hint::<P256_Z_LIMBS, 521, 9, _>(move |context| {
        let x = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "affine factor")?;
        let y = evaluated_uint::<HINT_LIMBS>(y_eval.evaluate_words(context), "affine factor")?;
        let target =
            evaluated_uint::<HINT_LIMBS>(target_eval.evaluate_words(context), "affine product target")?;
        let shifted = shifted_dividend(multiply_wide(x, y), hint_bias, target)?;
        let (quotient, remainder) = divisor.div_rem_ct(&shifted);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let (r, q) = split_521(circuit, bits);
    let rhs = r
        .value
        .clone()
        .add(target.value.clone())
        .add(q.value.clone().scale(modulus.value()))
        .sub(lc_words(bias.as_words()));
    circuit.assert_r1c::<P256_Z_LIMBS>(x.value.z, y.value.z, rhs.z);
    Elem { value: r }
}

fn lazy_divide<CS: Circuit>(
    circuit: &mut CS,
    modulus: Modulus,
    denominator: Rep<CS>,
    numerator: Rep<CS>,
) -> Rep<CS> {
    let bias = modulus_multiple(modulus, numerator.bound);
    let denominator_eval = denominator.value.capture();
    let numerator_eval = numerator.value.capture();
    let divisor = modulus.divisor();
    let hint_bias = bias;
    let bits = circuit.hint::<P256_Z_LIMBS, 521, 9, _>(move |context| {
        let a = evaluated_uint::<HINT_LIMBS>(
            denominator_eval.evaluate_words(context),
            "division denominator",
        )?;
        let b = evaluated_uint::<HINT_LIMBS>(numerator_eval.evaluate_words(context), "division numerator")?;
        let (_, denominator) = modulus.div_rem_representative(a);
        let inverse = modular_inverse_u256(denominator, modulus)
            .ok_or_else(|| HintError::new("zero or noninvertible division denominator"))?;
        let (_, numerator) = modulus.div_rem_representative(b);
        // Both residues are below the public 256-bit modulus.
        let inverse_product = IntegerOps.mul_wide(&inverse, &numerator);
        let (_, value) = divisor.div_rem_product_ct(&inverse_product);
        let shifted = shifted_dividend(multiply_wide(value.zero_extend(), a), hint_bias, b)?;
        let (quotient, remainder) = divisor.div_rem_ct(&shifted);
        debug_assert!(remainder.ct_is_zero().declassify());
        Ok(packed_wide_remainder_quotient(value, quotient))
    });
    let (value, q) = split_521(circuit, bits);
    let rhs = numerator
        .value
        .clone()
        .add(q.value.clone().scale(modulus.value()))
        .sub(lc_words(bias.as_words()));
    circuit.assert_r1c::<P256_Z_LIMBS>(value.value.z.clone(), denominator.value.z, rhs.z);
    Rep {
        value: value.value,
        bound: 2,
    }
}

fn lazy_reduce<CS: Circuit>(circuit: &mut CS, modulus: Modulus, x: Rep<CS>) -> Elem<CS> {
    let x_eval = x.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 521, 9, _>(move |context| {
        let value = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "lazy reduction operand")?;
        let (quotient, remainder) = modulus.div_rem_representative(value);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let (r, q) = split_521(circuit, bits);
    let relation = x
        .value
        .sub(r.value.clone().add(q.value.scale(modulus.value())));
    assert_zero(circuit, relation);
    of_u(circuit, modulus, r)
}

fn lazy_reduce_scalar<CS: Circuit>(
    circuit: &mut CS,
    modulus: Modulus,
    x: Rep<CS>,
) -> ScalarElem<CS> {
    let x_eval = x.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 521, 9, _>(move |context| {
        let value = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "lazy reduction operand")?;
        let (quotient, remainder) = modulus.div_rem_representative(value);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let remainder = bits.slice::<256, 4>(0);
    let quotient = bits.slice::<265, 5>(WIDTH);
    let (r, int_bits) = uint_from_repr_with_lifts::<CS, 256, 4>(circuit, remainder);
    let q = uint_from_repr::<CS, 265, 5>(circuit, quotient);
    let relation = x
        .value
        .sub(r.value.clone().add(q.value.scale(modulus.value())));
    assert_zero(circuit, relation);
    assert_lt(circuit, modulus, &r);
    ScalarElem {
        elem: Elem { value: r },
        int_bits,
    }
}

fn lazy_assert_mul_eq<CS: Circuit>(
    circuit: &mut CS,
    modulus: Modulus,
    x: Rep<CS>,
    y: Rep<CS>,
    target: Rep<CS>,
) {
    let bias = modulus_multiple(modulus, target.bound);
    let x_eval = x.value.capture();
    let y_eval = y.value.capture();
    let target_eval = target.value.capture();
    let divisor = modulus.divisor();
    let hint_bias = bias;
    let bits = circuit.hint::<P256_Z_LIMBS, 265, 5, _>(move |context| {
        let x = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "relation factor")?;
        let y = evaluated_uint::<HINT_LIMBS>(y_eval.evaluate_words(context), "relation factor")?;
        let target = evaluated_uint::<HINT_LIMBS>(
            target_eval.evaluate_words(context),
            "modular relation target",
        )?;
        let shifted = shifted_dividend(multiply_wide(x, y), hint_bias, target)?;
        let (quotient, remainder) = divisor.div_rem_ct(&shifted);
        debug_assert!(remainder.ct_is_zero().declassify());
        Ok(packed_wide(quotient))
    });
    let q = uint_from_repr::<CS, 265, 5>(circuit, bits);
    let rhs = target
        .value
        .add(q.value.scale(modulus.value()))
        .sub(lc_words(bias.as_words()));
    circuit.assert_r1c::<P256_Z_LIMBS>(x.value.z, y.value.z, rhs.z);
}

fn relaxed_reduce_small<CS: Circuit>(circuit: &mut CS, modulus: Modulus, x: Lc<CS>) -> Elem<CS> {
    let x_eval = x.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 258, 5, _>(move |context| {
        let value =
            evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "relaxed modular dividend")?;
        let (quotient, remainder) = modulus.div_rem_representative(value);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let r_bits = bits.slice::<256, 4>(0);
    let q_bits = bits.slice::<2, 1>(WIDTH);
    let r = uint_from_repr::<CS, 256, 4>(circuit, r_bits);
    let q = uint_from_repr::<CS, 2, 1>(circuit, q_bits);
    let relation = x.sub(r.value.clone().add(q.value.scale(modulus.value())));
    assert_zero(circuit, relation);
    Elem { value: r }
}

fn relaxed_mul<CS: Circuit>(
    circuit: &mut CS,
    modulus: Modulus,
    x: Elem<CS>,
    y: Elem<CS>,
) -> Elem<CS> {
    let x_eval = x.value.value.capture();
    let y_eval = y.value.value.capture();
    let divisor = modulus.divisor();
    let bits = circuit.hint::<P256_Z_LIMBS, 514, 9, _>(move |context| {
        let a = evaluated_uint::<WORD_LIMBS>(
            x_eval.evaluate_words(context),
            "relaxed multiplication factor",
        )?;
        let b = evaluated_uint::<WORD_LIMBS>(
            y_eval.evaluate_words(context),
            "relaxed multiplication factor",
        )?;
        // Elem stores a UInt256 even when its representative is not canonical.
        let value = *IntegerOps.mul_wide(&a, &b).checked_resize_ct::<8>().value();
        let (quotient, remainder) = divisor.div_rem_ct(&value);
        Ok(packed_wide_remainder_quotient(remainder, quotient))
    });
    let r_bits = bits.slice::<256, 4>(0);
    let q_bits = bits.slice::<258, 5>(WIDTH);
    let r = uint_from_repr::<CS, 256, 4>(circuit, r_bits);
    let q = uint_from_repr::<CS, 258, 5>(circuit, q_bits);
    let rhs = r.value.clone().add(q.value.scale(modulus.value()));
    circuit.assert_r1c::<P256_Z_LIMBS>(x.value.value.z, y.value.value.z, rhs.z);
    Elem { value: r }
}

fn packed_evaluated<const N: usize, const M: usize>(
    words: &[u64],
    description: &str,
) -> HintResult<PackedBits<N, M>> {
    if words.last().is_some_and(|word| word >> 63 != 0) {
        return Err(HintError::new(format!("negative {description}")));
    }
    Ok(PackedBits::from_words(array::from_fn(|index| {
        words.get(index).copied().unwrap_or(0)
    })))
}

fn evaluated_is_one(words: &[u64]) -> bool {
    let difference = words
        .get(1..)
        .unwrap_or(&[])
        .iter()
        .fold(words.first().copied().unwrap_or(0) ^ 1, |acc, word| {
            acc | word
        });
    difference == 0
}

fn evaluated_usize(words: &[u64], description: &str) -> HintResult<usize> {
    let invalid = (words.last().copied().unwrap_or(0) >> 63)
        | words
            .get(1..)
            .unwrap_or(&[])
            .iter()
            .fold(0, |acc, word| acc | word);
    if invalid != 0 {
        return Err(HintError::new(format!("invalid {description}")));
    }
    usize::try_from(words.first().copied().unwrap_or(0))
        .map_err(|_| HintError::new(format!("oversized {description}")))
}

fn packed_flag_inverse_u256(flag: bool, inverse: Uint<4>) -> PackedBits<257, 5> {
    let mut carry = u64::from(flag);
    PackedBits::from_words(array::from_fn(|index| {
        let word = inverse.as_words().get(index).copied().unwrap_or(0);
        let output = (word << 1) | carry;
        carry = word >> 63;
        output
    }))
}

/// Decode a nonnegative, declared-width circuit integer. Scan all source
/// words before reporting malformed input; never trim a private magnitude.
fn evaluated_uint<const L: usize>(words: &[u64], description: &str) -> HintResult<Uint<L>> {
    let negative = words.last().copied().unwrap_or(0) >> 63;
    let excess = words
        .get(L..)
        .unwrap_or(&[])
        .iter()
        .fold(0, |acc, word| acc | word);
    if negative | excess != 0 {
        return Err(HintError::new(format!(
            "invalid {description} width or sign"
        )));
    }
    Ok(Uint::from_words(array::from_fn(|i| {
        words.get(i).copied().unwrap_or(0)
    })))
}

/// Fixed public representative capacity, independent of witness magnitudes.
/// Multiplying two 320-bit bounds requires ten limbs.
fn multiply_wide(left: Uint<HINT_LIMBS>, right: Uint<HINT_LIMBS>) -> Uint<10> {
    *IntegerOps
        .mul_wide(&left, &right)
        .checked_resize_ct::<10>()
        .value()
}

fn shifted_dividend(product: Uint<10>, bias: Uint<HINT_LIMBS>, target: Uint<HINT_LIMBS>) -> HintResult<Uint<10>> {
    let sum = product.checked_add_ct(&bias.zero_extend());
    let difference = sum.value().checked_sub_ct(&target.zero_extend());
    if !(sum.validity() & difference.validity()).declassify() {
        return Err(HintError::new("invalid P-256 dividend bounds"));
    }
    Ok(*difference.value())
}

#[cfg(test)]
fn uint256_biguint(value: Uint<4>) -> BigUint {
    BigUint::new(
        value
            .as_words()
            .iter()
            .flat_map(|word| [*word as u32, (*word >> 32) as u32])
            .collect(),
    )
}

fn modulus_multiple(modulus: Modulus, factor: usize) -> Uint<HINT_LIMBS> {
    let factor = u64::try_from(factor).expect("P-256 representative bound exceeds u64");
    *IntegerOps
        .mul_wide(&modulus.words(), &Uint::<1>::from_u64(factor))
        .checked_resize_ct::<HINT_LIMBS>()
        .value()
}

fn packed_wide<const N: usize, const M: usize, const L: usize>(value: Uint<L>) -> PackedBits<N, M> {
    PackedBits::from_words(array::from_fn(|index| {
        value.as_words().get(index).copied().unwrap_or(0)
    }))
}

fn packed_wide_remainder_quotient<const N: usize, const M: usize, const L: usize>(
    remainder: Uint<4>,
    quotient: Uint<L>,
) -> PackedBits<N, M> {
    PackedBits::from_words(array::from_fn(|index| {
        if index < 4 {
            remainder.as_words()[index]
        } else {
            quotient.as_words().get(index - 4).copied().unwrap_or(0)
        }
    }))
}

fn modular_inverse_u256(value: Uint<4>, modulus: Modulus) -> Option<Uint<4>> {
    let inverse = modulus.inverse().inverse_ct(&value);
    inverse.validity().declassify().then_some(*inverse.value())
}

fn point_from_elems<CS: Circuit>(x: &Elem<CS>, y: &Elem<CS>) -> Point<CS> {
    Point {
        x: of_elem(x),
        y: of_elem(y),
        infinity: lc_u64(0),
    }
}

fn infinity<CS: Circuit>() -> Point<CS> {
    Point {
        x: rep_u64(0),
        y: rep_u64(0),
        infinity: lc_u64(1),
    }
}

fn and_bit<CS: Circuit>(circuit: &mut CS, x: Lc<CS>, y: Lc<CS>) -> Lc<CS> {
    let x_eval = x.capture();
    let y_eval = y.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 1, 1, _>(move |context| {
        let value = evaluated_is_one(x_eval.evaluate_words(context))
            & evaluated_is_one(y_eval.evaluate_words(context));
        Ok(PackedBits::from_array([value]))
    });
    let out = uint_from_repr::<CS, 1, 1>(circuit, bits).value;
    circuit.assert_r1c::<P256_Z_LIMBS>(x.z, y.z, out.z.clone());
    out
}

fn and3_bit<CS: Circuit>(circuit: &mut CS, x: Lc<CS>, y: Lc<CS>, z: Lc<CS>) -> Lc<CS> {
    let xy = and_bit(circuit, x, y);
    and_bit(circuit, z, xy)
}

fn lazy_zero_test<CS: Circuit>(circuit: &mut CS, modulus: Modulus, x: Rep<CS>) -> Lc<CS> {
    let x_eval = x.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 257, 5, _>(move |context| {
        let a = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "zero-test operand")?;
        let (_, value) = modulus.div_rem_representative(a);
        let is_zero = value.ct_is_zero().declassify();
        let inverse = *modulus.inverse().inverse_ct(&value).value();
        Ok(packed_flag_inverse_u256(is_zero, inverse))
    });
    let z_bits = bits.slice::<1, 1>(0);
    let inverse_bits = bits.slice::<256, 4>(1);
    let z = uint_from_repr::<CS, 1, 1>(circuit, z_bits).value;
    let inverse = uint_from_repr::<CS, 256, 4>(circuit, inverse_bits);
    lazy_assert_mul_eq(
        circuit,
        modulus,
        x.clone(),
        of_elem(&Elem { value: inverse }),
        Rep {
            value: lc_u64::<CS>(1).sub(z.clone()),
            bound: 1,
        },
    );

    let z_eval = z.capture();
    let x_eval = x.value.capture();
    let q_bits = circuit.hint::<P256_Z_LIMBS, 9, 1, _>(move |context| {
        let z = evaluated_uint::<1>(z_eval.evaluate_words(context), "zero-test flag")?;
        let x = evaluated_uint::<HINT_LIMBS>(x_eval.evaluate_words(context), "zero-test quotient operand")?;
        // z comes from the one-bit slice above. Its product with x keeps x's
        // public representative width, including when the private flag is zero.
        let product = Uint::ct_select(&Uint::ZERO, &x, z.bit(0).mask());
        let (quotient, remainder) = modulus.div_rem_representative(product);
        debug_assert!(remainder.ct_is_zero().declassify());
        Ok(packed_wide(quotient))
    });
    let q = uint_from_repr::<CS, 9, 1>(circuit, q_bits);
    circuit.assert_r1c::<P256_Z_LIMBS>(z.z.clone(), x.value.z, q.value.scale(modulus.value()).z);
    z
}

fn select_rep<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    choose: Lc<CS>,
    when_one: Rep<CS>,
    when_zero: Rep<CS>,
    out_bound: usize,
) -> Rep<CS> {
    let choose_eval = choose.capture();
    let one_eval = when_one.value.capture();
    let zero_eval = when_zero.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, N, M, _>(move |context| {
        let mask = CtMask::from_lsb(u64::from(evaluated_is_one(
            choose_eval.evaluate_words(context),
        )));
        let one = one_eval.evaluate_words(context);
        let zero = zero_eval.evaluate_words(context);
        let value: [u64; P256_Z_LIMBS] = array::from_fn(|i| {
            u64::ct_select(
                &zero.get(i).copied().unwrap_or(0),
                &one.get(i).copied().unwrap_or(0),
                mask,
            )
        });
        packed_evaluated(&value, "selection")
    });
    let out = uint_from_repr::<CS, N, M>(circuit, bits).value;
    let difference = when_one.value.sub(when_zero.value.clone());
    let output_difference = out.clone().sub(when_zero.value);
    circuit.assert_r1c::<P256_Z_LIMBS>(choose.z, difference.z, output_difference.z);
    Rep {
        value: out,
        bound: out_bound,
    }
}

fn select_canonical<CS: Circuit>(
    circuit: &mut CS,
    choose: Lc<CS>,
    when_one: Rep<CS>,
    when_zero: Rep<CS>,
) -> Rep<CS> {
    select_rep::<CS, 256, 4>(circuit, choose, when_one, when_zero, 2)
}

fn select_formula<CS: Circuit>(
    circuit: &mut CS,
    choose: Lc<CS>,
    when_one: Rep<CS>,
    when_zero: Rep<CS>,
) -> Rep<CS> {
    select_rep::<CS, 262, 5>(circuit, choose, when_one, when_zero, 66)
}

fn double_complete<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    point: Point<CS>,
) -> Point<CS> {
    let x2 = lazy_mul(circuit, curve.base(), point.x.clone(), point.x.clone());
    let numerator = curve.double_numerator(x2, Some(point.infinity.clone()));
    let denominator = rep_add(
        rep_scale(2, point.y.clone()),
        Rep {
            value: point.infinity.clone(),
            bound: 1,
        },
    );
    let slope = lazy_divide(circuit, curve.base(), denominator, numerator);
    let x3 = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope.clone(),
        slope.clone(),
        rep_scale(2, point.x.clone()),
    );
    let x3_rep = of_elem(&x3);
    let y3 = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope,
        rep_sub(curve.base(), point.x, x3_rep.clone()),
        point.y,
    );
    Point {
        x: x3_rep,
        y: of_elem(&y3),
        infinity: point.infinity,
    }
}

struct AddControl<CS: Circuit> {
    same_x: Lc<CS>,
    opposite_y: Lc<CS>,
    finite: Lc<CS>,
    double_case: Lc<CS>,
    active: Lc<CS>,
}

fn add_complete<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    p: Point<CS>,
    q: Point<CS>,
) -> Point<CS> {
    let dx = rep_sub(curve.base(), q.x.clone(), p.x.clone());
    let y_sum = rep_add(p.y.clone(), q.y.clone());
    let same_x = lazy_zero_test(circuit, curve.base(), dx.clone());
    let opposite_y = lazy_zero_test(circuit, curve.base(), y_sum);
    let finite = and_bit(
        circuit,
        lc_u64::<CS>(1).sub(p.infinity.clone()),
        lc_u64::<CS>(1).sub(q.infinity.clone()),
    );
    let double_kind = and_bit(
        circuit,
        same_x.clone(),
        lc_u64::<CS>(1).sub(opposite_y.clone()),
    );
    let double_case = and_bit(circuit, finite.clone(), double_kind);
    let generic_case = and_bit(circuit, finite.clone(), lc_u64::<CS>(1).sub(same_x.clone()));
    let control = AddControl {
        same_x,
        opposite_y,
        finite,
        double_case: double_case.clone(),
        active: double_case.add(generic_case),
    };

    let dy = rep_sub(curve.base(), q.y.clone(), p.y.clone());
    let x2 = lazy_mul(circuit, curve.base(), p.x.clone(), p.x.clone());
    let double_numerator = curve.double_numerator(x2, None);
    let double_denominator = rep_scale(2, p.y.clone());
    let selected_numerator =
        select_formula(circuit, control.double_case.clone(), double_numerator, dy);
    let selected_denominator =
        select_formula(circuit, control.double_case.clone(), double_denominator, dx);
    let numerator = select_formula(
        circuit,
        control.active.clone(),
        selected_numerator,
        rep_u64(0),
    );
    let denominator = select_formula(
        circuit,
        control.active.clone(),
        selected_denominator,
        rep_u64(1),
    );
    let slope = lazy_divide(circuit, curve.base(), denominator, numerator);
    let candidate_x = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope.clone(),
        slope.clone(),
        rep_add(p.x.clone(), q.x.clone()),
    );
    let candidate_x = of_elem(&candidate_x);
    let candidate_y = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope,
        rep_sub(curve.base(), p.x.clone(), candidate_x.clone()),
        p.y.clone(),
    );
    let candidate_y = of_elem(&candidate_y);

    let inactive_x0 = select_canonical(circuit, q.infinity.clone(), p.x.clone(), rep_u64(0));
    let inactive_y0 = select_canonical(circuit, q.infinity.clone(), p.y.clone(), rep_u64(0));
    let inactive_x = select_canonical(circuit, p.infinity.clone(), q.x.clone(), inactive_x0);
    let inactive_y = select_canonical(circuit, p.infinity.clone(), q.y.clone(), inactive_y0);
    let x = select_canonical(circuit, control.active.clone(), candidate_x, inactive_x);
    let y = select_canonical(circuit, control.active, candidate_y, inactive_y);
    let both_infinity = and_bit(circuit, p.infinity, q.infinity);
    let finite_opposite = and3_bit(circuit, control.same_x, control.opposite_y, control.finite);
    Point {
        x,
        y,
        infinity: both_infinity.add(finite_opposite),
    }
}

/// Constrains `value * w = flag (mod p)` for a hinted `w`. When `flag` is one
/// this forces `value != 0`; when it is zero the constraint is vacuous and `w`
/// is simply zero.
fn assert_nonzero_when<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    value: Rep<CS>,
    flag: Lc<CS>,
) {
    let modulus = curve.base();
    let value_eval = value.value.capture();
    let flag_eval = flag.capture();
    let divisor = modulus.divisor();
    let prepared = modulus.inverse();
    let bits = circuit.hint::<P256_Z_LIMBS, WIDTH, WORD_LIMBS, _>(move |context| {
        // The representative carries a bias that is zero mod p; reduce before
        // inverting.
        let raw = evaluated_uint::<HINT_LIMBS>(
            value_eval.evaluate_words(context),
            "nonzero-test operand",
        )?;
        let (_, reduced) = divisor.div_rem_ct(&raw);
        let inverse = prepared.inverse_ct(&reduced);
        let active = evaluated_is_one(flag_eval.evaluate_words(context));
        if active && !inverse.validity().declassify() {
            return Err(HintError::new("point addition operands share an x coordinate"));
        }
        let witness = if active { *inverse.value() } else { Uint::ZERO };
        // `packed_evaluated` reads the sign from the last word, so widen to the
        // circuit's declared limb count rather than passing four limbs whose
        // top bit is an ordinary value bit.
        let mut words = [0u64; P256_Z_LIMBS];
        words[..WORD_LIMBS].copy_from_slice(witness.as_words());
        packed_evaluated(&words, "modular inverse")
    });
    let witness = uint_from_repr::<CS, WIDTH, WORD_LIMBS>(circuit, bits);
    lazy_assert_mul_eq(
        circuit,
        modulus,
        value,
        Rep {
            value: witness.value,
            bound: 1,
        },
        Rep {
            value: flag,
            bound: 1,
        },
    );
}

/// `P + Q` where the caller guarantees the operands are never equal or
/// opposite, so the plain chord formula applies and the doubling and
/// `P = -Q` case analysis can be dropped.
///
/// The ladders satisfy that guarantee because they start at [`Curve::offsets`]'s
/// point `R` instead of the identity: the accumulator is a structureless
/// multiple from the first round, so it never sits in the small-multiple regime
/// where a public key that is itself a small multiple of `G` would collide with
/// a table entry.
///
/// SOUNDNESS. The chord formula needs `x_P != x_Q`. It is not enough that a
/// violation would give the wrong answer: the slope is pinned by
/// `denominator * slope = numerator`, so when `dx` and `dy` are both zero the
/// slope is left completely free and a prover could steer the sum anywhere.
/// This gadget therefore *proves* `dx != 0` — by exhibiting its inverse —
/// whenever both operands are finite. A prover that would hit an exception
/// cannot satisfy that constraint and simply fails to prove; it can never have
/// a wrong sum accepted. Infinity is still handled, because a zero window digit
/// really does select the point at infinity.
///
/// COMPLETENESS. In the joint ladder the accumulator is an unpredictable
/// multiple of the bases, so `acc = +-T` arises with probability about `2^-128`
/// per addition. An adversarial public key can force it, but only to make its
/// own proof impossible.
/// A public curve point as circuit constants; never the identity.
fn constant_point<CS: Circuit>(point: &(Uint<4>, Uint<4>)) -> Point<CS> {
    Point {
        x: rep_constant(&point.0),
        y: rep_constant(&point.1),
        infinity: lc_u64::<CS>(0),
    }
}

fn add_distinct<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    p: Point<CS>,
    q: Point<CS>,
) -> Point<CS> {
    let finite = and_bit(
        circuit,
        lc_u64::<CS>(1).sub(p.infinity.clone()),
        lc_u64::<CS>(1).sub(q.infinity.clone()),
    );
    let dx = rep_sub(curve.base(), q.x.clone(), p.x.clone());
    let dy = rep_sub(curve.base(), q.y.clone(), p.y.clone());
    assert_nonzero_when(circuit, curve, dx.clone(), finite.clone());
    let numerator = select_formula(circuit, finite.clone(), dy, rep_u64(0));
    let denominator = select_formula(circuit, finite.clone(), dx, rep_u64(1));
    let slope = lazy_divide(circuit, curve.base(), denominator, numerator);
    let candidate_x = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope.clone(),
        slope.clone(),
        rep_add(p.x.clone(), q.x.clone()),
    );
    let candidate_x = of_elem(&candidate_x);
    let candidate_y = lazy_mul_sub_to_elem(
        circuit,
        curve.base(),
        slope,
        rep_sub(curve.base(), p.x.clone(), candidate_x.clone()),
        p.y.clone(),
    );
    let candidate_y = of_elem(&candidate_y);
    let inactive_x0 = select_canonical(circuit, q.infinity.clone(), p.x.clone(), rep_u64(0));
    let inactive_y0 = select_canonical(circuit, q.infinity.clone(), p.y.clone(), rep_u64(0));
    let inactive_x = select_canonical(circuit, p.infinity.clone(), q.x.clone(), inactive_x0);
    let inactive_y = select_canonical(circuit, p.infinity.clone(), q.y.clone(), inactive_y0);
    let x = select_canonical(circuit, finite.clone(), candidate_x, inactive_x);
    let y = select_canonical(circuit, finite, candidate_y, inactive_y);
    // `dx != 0` rules out `P = -Q`, so a finite sum is never the identity; only
    // infinity plus infinity is.
    let both_infinity = and_bit(circuit, p.infinity, q.infinity);
    Point {
        x,
        y,
        infinity: both_infinity,
    }
}

fn materialize_multiples<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    q: Point<CS>,
) -> Vec<Point<CS>> {
    let p1 = q;
    let mut output = Vec::with_capacity(16);
    output.push(infinity());
    output.push(p1.clone());
    let mut previous = p1.clone();
    for _ in 2..16 {
        previous = add_complete(circuit, curve, previous, p1.clone());
        output.push(previous.clone());
    }
    output
}

fn indicators_impl<CS: Circuit, const N: usize, const M: usize>(
    circuit: &mut CS,
    digit: Lc<CS>,
) -> (UInt<CS, N, M>, Vec<Lc<CS>>) {
    let digit_eval = digit.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, N, M, _>(move |context| {
        let digit = evaluated_usize(digit_eval.evaluate_words(context), "indicator digit")?;
        Ok(PackedBits::from_fn(|i| digit == i))
    });
    let bit_handles = repr_bits::<CS::Bool, N, M>(&bits);
    let mut lifted = Vec::with_capacity(N);
    let mut full = P256Z::<CS>::zero();
    let mut power = P256Coefficient::<CS>::one();
    for bit in &bit_handles {
        let z = circuit.bitz::<P256_Z_LIMBS>(bit.clone());
        full += z.clone() * power.clone();
        lifted.push(Lc { z });
        power += power.clone();
    }
    let out = UInt {
        bits,
        value: Lc { z: full },
    };
    let sum = lifted.iter().cloned().fold(lc_u64::<CS>(0), Lc::add);
    assert_zero(circuit, sum.sub(lc_u64(1)));
    let weighted = lifted
        .iter()
        .cloned()
        .enumerate()
        .fold(lc_u64::<CS>(0), |sum, (i, bit)| {
            sum.add(bit.scale_coefficient(P256Coefficient::<CS>::from(i as u64)))
        });
    assert_zero(circuit, weighted.sub(digit));
    (out, lifted)
}

fn lookup_rep<CS: Circuit>(
    circuit: &mut CS,
    digit: Lc<CS>,
    indicators: &[Lc<CS>],
    values: &[Rep<CS>],
) -> Rep<CS> {
    let digit_eval = digit.capture();
    let value_evals: Vec<_> = values.iter().map(|value| value.value.capture()).collect();
    let bits = circuit.hint::<P256_Z_LIMBS, 256, 4, _>(move |context| {
        let index = evaluated_usize(digit_eval.evaluate_words(context), "lookup digit")?;
        if index >= value_evals.len() {
            return Err(HintError::new("lookup digit out of range"));
        }
        let mut selected = [0; P256_Z_LIMBS];
        for (i, expression) in value_evals.iter().enumerate() {
            let mask = (i as u64).ct_eq(&(index as u64));
            let words = expression.evaluate_words(context);
            for (j, output) in selected.iter_mut().enumerate() {
                *output = u64::ct_select(output, &words.get(j).copied().unwrap_or(0), mask);
            }
        }
        packed_evaluated(&selected, "lookup value")
    });
    let out = uint_from_repr::<CS, 256, 4>(circuit, bits).value;
    for (indicator, value) in indicators.iter().zip(values) {
        circuit.assert_r1c::<P256_Z_LIMBS>(
            indicator.z.clone(),
            out.clone().sub(value.value.clone()).z,
            P256Z::<CS>::zero(),
        );
    }
    Rep {
        value: out,
        bound: 2,
    }
}

fn lookup_flag<CS: Circuit>(
    circuit: &mut CS,
    digit: Lc<CS>,
    indicators: &[Lc<CS>],
    flags: &[Lc<CS>],
) -> Lc<CS> {
    let digit_eval = digit.capture();
    let flag_evals: Vec<_> = flags.iter().map(Lc::capture).collect();
    let bits = circuit.hint::<P256_Z_LIMBS, 1, 1, _>(move |context| {
        let index = evaluated_usize(digit_eval.evaluate_words(context), "flag lookup digit")?;
        if index >= flag_evals.len() {
            return Err(HintError::new("flag lookup digit out of range"));
        }
        let mut selected = 0u64;
        for (i, expression) in flag_evals.iter().enumerate() {
            let value = u64::from(evaluated_is_one(expression.evaluate_words(context)));
            selected = u64::ct_select(&selected, &value, (i as u64).ct_eq(&(index as u64)));
        }
        Ok(PackedBits::from_array([selected != 0]))
    });
    let out = uint_from_repr::<CS, 1, 1>(circuit, bits).value;
    for (indicator, flag) in indicators.iter().zip(flags) {
        circuit.assert_r1c::<P256_Z_LIMBS>(
            indicator.z.clone(),
            out.clone().sub(flag.clone()).z,
            P256Z::<CS>::zero(),
        );
    }
    out
}

fn lookup_point<CS: Circuit>(circuit: &mut CS, digit: Lc<CS>, table: &[Point<CS>]) -> Point<CS> {
    let (_, indicator_bits) = indicators_impl::<CS, 16, 1>(circuit, digit.clone());
    let xs: Vec<_> = table.iter().map(|point| point.x.clone()).collect();
    let ys: Vec<_> = table.iter().map(|point| point.y.clone()).collect();
    let flags: Vec<_> = table.iter().map(|point| point.infinity.clone()).collect();
    Point {
        x: lookup_rep(circuit, digit.clone(), &indicator_bits, &xs),
        y: lookup_rep(circuit, digit.clone(), &indicator_bits, &ys),
        infinity: lookup_flag(circuit, digit, &indicator_bits, &flags),
    }
}

/// Materializes the fixed-base generator tables used by witness generation.
/// Call this during benchmark or service setup to exclude one-time constant
/// initialization from latency measurements.
pub fn prepare() {
    prepare_curve(&P256);
}

/// [`prepare`] for one specific curve.
pub fn prepare_curve(curve: &'static Curve) {
    let _ = curve.generator_table();
}

fn generator_coefficients<CS: Circuit>(curve: &'static Curve) -> Vec<(P256Coefficient<CS>, P256Coefficient<CS>)> {
    curve.generator_table()
        .iter()
        .map(|(x, y)| (coefficient_for::<CS>(x), coefficient_for::<CS>(y)))
        .collect()
}

fn lookup_generator_byte<CS: Circuit>(
    circuit: &mut CS,
    digit: Lc<CS>,
    table: &[(P256Coefficient<CS>, P256Coefficient<CS>)],
) -> Point<CS> {
    let (_, indicators) = indicators_impl::<CS, 256, 4>(circuit, digit);
    let x = indicators
        .iter()
        .zip(table)
        .fold(lc_u64::<CS>(0), |sum, (bit, (x, _))| {
            sum.add(bit.clone().scale_coefficient(x.clone()))
        });
    let y = indicators
        .iter()
        .zip(table)
        .fold(lc_u64::<CS>(0), |sum, (bit, (_, y))| {
            sum.add(bit.clone().scale_coefficient(y.clone()))
        });
    Point {
        x: Rep { value: x, bound: 2 },
        y: Rep { value: y, bound: 2 },
        infinity: indicators[0].clone(),
    }
}

// A UInt retains the combined integer LC, but Lean's window expressions refer
// to individual intBits. Keep those lifts explicitly for scalar windows.
struct ScalarElem<CS: Circuit> {
    elem: Elem<CS>,
    int_bits: Vec<Lc<CS>>,
}

impl<CS: Circuit> Clone for ScalarElem<CS> {
    fn clone(&self) -> Self {
        Self {
            elem: self.elem.clone(),
            int_bits: self.int_bits.clone(),
        }
    }
}

fn lift_input_word<CS: Circuit>(
    circuit: &mut CS,
    bits: <CS::Bool as BoolWitness>::Repr<256, 4>,
) -> ScalarElem<CS> {
    let handles = repr_bits::<CS::Bool, 256, 4>(&bits);
    let mut lifted = Vec::with_capacity(256);
    let mut z = P256Z::<CS>::zero();
    let mut power = P256Coefficient::<CS>::one();
    for bit in handles {
        let bit_z = circuit.bitz::<P256_Z_LIMBS>(bit.clone());
        z += bit_z.clone() * power.clone();
        lifted.push(Lc { z: bit_z });
        power += power.clone();
    }
    ScalarElem {
        elem: Elem {
            value: UInt {
                bits,
                value: Lc { z },
            },
        },
        int_bits: lifted,
    }
}

fn scalar_window<CS: Circuit>(scalar: &ScalarElem<CS>, start: usize, width: usize) -> Lc<CS> {
    scalar.int_bits[start..start + width]
        .iter()
        .cloned()
        .enumerate()
        .fold(lc_u64::<CS>(0), |sum, (bit, value)| {
            sum.add(value.scale_coefficient(P256Coefficient::<CS>::from(1_u64 << bit)))
        })
}

/// One 128-bit GLV half: its lifted bits, and whether the base point it
/// multiplies must be negated (the sign of the signed sub-scalar).
struct SubScalar<CS: Circuit> {
    int_bits: Vec<Lc<CS>>,
    negate: Lc<CS>,
}

fn sub_window<CS: Circuit>(scalar: &SubScalar<CS>, start: usize, width: usize) -> Lc<CS> {
    scalar.int_bits[start..start + width]
        .iter()
        .cloned()
        .enumerate()
        .fold(lc_u64::<CS>(0), |sum, (bit, value)| {
            sum.add(value.scale_coefficient(P256Coefficient::<CS>::from(1_u64 << bit)))
        })
}

/// Constrains `k = (-1)^s1 * m1 + lambda * (-1)^s2 * m2 (mod n)` with both
/// magnitudes range-checked to 128 bits, and returns the two halves.
///
/// Shortness is what makes GLV pay: the 128-bit range checks are the only thing
/// stopping a prover from handing back a full-width split, which would be a
/// valid congruence but would not halve the ladder.
fn glv_decompose<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    endomorphism: &'static Endomorphism,
    k: &ScalarElem<CS>,
) -> (SubScalar<CS>, SubScalar<CS>) {
    let n = curve.scalar();
    let k_eval = k.elem.value.value.capture();
    let bits = circuit.hint::<P256_Z_LIMBS, 258, 5, _>(move |context| {
        let scalar = evaluated_uint::<4>(k_eval.evaluate_words(context), "GLV scalar")?;
        let (m1, s1, m2, s2) = endomorphism.decompose(n, &scalar);
        Ok(PackedBits::from_words([
            m1.as_words()[0],
            m1.as_words()[1],
            m2.as_words()[0],
            m2.as_words()[1],
            u64::from(s1) | (u64::from(s2) << 1),
        ]))
    });
    let (m1, m1_bits) = uint_from_repr_with_lifts::<CS, 128, 2>(circuit, bits.slice::<128, 2>(0));
    let (m2, m2_bits) = uint_from_repr_with_lifts::<CS, 128, 2>(circuit, bits.slice::<128, 2>(128));
    let s1 = uint_from_repr::<CS, 1, 1>(circuit, bits.slice::<1, 1>(256)).value;
    let s2 = uint_from_repr::<CS, 1, 1>(circuit, bits.slice::<1, 1>(257)).value;
    // Canonical representatives of the signed halves, for the congruence only;
    // the ladder consumes the magnitudes and negates the base points instead.
    let mut signed = |circuit: &mut CS, sign: Lc<CS>, value: &UInt<CS, 128, 2>| {
        let magnitude = Rep {
            value: value.value.clone(),
            bound: 1,
        };
        let negated = rep_sub(n, rep_u64::<CS>(0), magnitude.clone());
        select_canonical(circuit, sign, negated, magnitude)
    };
    let first = signed(circuit, s1.clone(), &m1);
    let second = signed(circuit, s2.clone(), &m2);
    let scaled = lazy_mul(circuit, n, rep_constant(&endomorphism.lambda), second);
    let relation = rep_sub(n, rep_add(first, scaled), of_elem(&k.elem));
    lazy_assert_mul_eq(circuit, n, rep_u64::<CS>(1), relation, rep_u64::<CS>(0));
    (
        SubScalar {
            int_bits: m1_bits,
            negate: s1,
        },
        SubScalar {
            int_bits: m2_bits,
            negate: s2,
        },
    )
}

/// `(x, y) -> (x, -y)` when `negate` is set. The infinity encoding `(0, 0)` is
/// fixed by this map, so the flag needs no special case.
fn negate_y_if<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    negate: Lc<CS>,
    point: Point<CS>,
) -> Point<CS> {
    // `rep_sub` would bias by `bound * p`, putting `2p - y` past the select's
    // 256-bit range check. Coordinates are canonical, so one `p` suffices and
    // `p - y` stays representable; `y = 0` maps to `p`, which is still zero.
    let negated = Rep {
        value: lc_constant::<CS>(curve.base().value()).sub(point.y.value.clone()),
        bound: 2,
    };
    Point {
        x: point.x,
        y: select_canonical(circuit, negate, negated, point.y),
        infinity: point.infinity,
    }
}

/// `phi(i*Q) = (beta * x_i, y_i)`: one modular multiplication per table entry,
/// rather than a second 14-addition chain.
fn phi_table<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    endomorphism: &'static Endomorphism,
    table: &[Point<CS>],
) -> Vec<Point<CS>> {
    table
        .iter()
        .map(|point| Point {
            x: lazy_mul(
                circuit,
                curve.base(),
                rep_constant(&endomorphism.beta),
                point.x.clone(),
            ),
            y: point.y.clone(),
            infinity: point.infinity.clone(),
        })
        .collect()
}

fn phi_generator_coefficients<CS: Circuit>(
    curve: &'static Curve,
    endomorphism: &'static Endomorphism,
) -> Vec<(P256Coefficient<CS>, P256Coefficient<CS>)> {
    endomorphism
        .phi_generator_table(curve)
        .iter()
        .map(|(x, y)| (coefficient_for::<CS>(x), coefficient_for::<CS>(y)))
        .collect()
}

/// `u1*G + u2*Q` with both scalars GLV-split, so the shared doubling chain is
/// 128 long instead of 256. Additions and table lookups are unchanged: the
/// total scalar material absorbed per round is the same, only the accumulator
/// is shifted half as far.
fn glv_scalar_mul<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    endomorphism: &'static Endomorphism,
    u1: ScalarElem<CS>,
    u2: ScalarElem<CS>,
    q: Point<CS>,
) -> Point<CS> {
    let (g_half, phi_g_half) = glv_decompose(circuit, curve, endomorphism, &u1);
    let (q_half, phi_q_half) = glv_decompose(circuit, curve, endomorphism, &u2);
    let base = materialize_multiples(circuit, curve, q);
    let phi_base = phi_table(circuit, curve, endomorphism, &base);
    // Fold each half's sign into its table once, not once per lookup.
    let q_table: Vec<_> = base
        .into_iter()
        .map(|point| negate_y_if(circuit, curve, q_half.negate.clone(), point))
        .collect();
    let phi_q_table: Vec<_> = phi_base
        .into_iter()
        .map(|point| negate_y_if(circuit, curve, phi_q_half.negate.clone(), point))
        .collect();
    let g_table = generator_coefficients::<CS>(curve);
    let phi_g_table = phi_generator_coefficients::<CS>(curve, endomorphism);
    let offsets = curve.offsets();
    let mut accumulator = constant_point(&offsets[0]);
    for i in 0..16 {
        let d_g = sub_window(&g_half, 120 - 8 * i, 8);
        let d_phi_g = sub_window(&phi_g_half, 120 - 8 * i, 8);
        let q_hi = lookup_point(circuit, sub_window(&q_half, 124 - 8 * i, 4), &q_table);
        let q_lo = lookup_point(circuit, sub_window(&q_half, 120 - 8 * i, 4), &q_table);
        let phi_hi = lookup_point(circuit, sub_window(&phi_q_half, 124 - 8 * i, 4), &phi_q_table);
        let phi_lo = lookup_point(circuit, sub_window(&phi_q_half, 120 - 8 * i, 4), &phi_q_table);
        // The public tables cannot be pre-negated, so their lookups are.
        let g = lookup_generator_byte(circuit, d_g, &g_table);
        let g = negate_y_if(circuit, curve, g_half.negate.clone(), g);
        let phi_g = lookup_generator_byte(circuit, d_phi_g, &phi_g_table);
        let phi_g = negate_y_if(circuit, curve, phi_g_half.negate.clone(), phi_g);
        for _ in 0..4 {
            accumulator = double_complete(circuit, curve, accumulator);
        }
        accumulator = add_distinct(circuit, curve, accumulator, q_hi);
        accumulator = add_distinct(circuit, curve, accumulator, phi_hi);
        for _ in 0..4 {
            accumulator = double_complete(circuit, curve, accumulator);
        }
        accumulator = add_distinct(circuit, curve, accumulator, q_lo);
        accumulator = add_distinct(circuit, curve, accumulator, phi_lo);
        accumulator = add_distinct(circuit, curve, accumulator, g);
        accumulator = add_distinct(circuit, curve, accumulator, phi_g);
    }
    // Sixteen rounds of eight doublings leave `2^128 R` on the accumulator.
    // This last addition can legitimately be a doubling or land on the
    // identity, so it stays complete.
    add_complete(circuit, curve, accumulator, constant_point(&offsets[1]))
}

fn joint_scalar_mul<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    u1: ScalarElem<CS>,
    u2: ScalarElem<CS>,
    q: Point<CS>,
) -> Point<CS> {
    let q_table = materialize_multiples(circuit, curve, q);
    let g_table = generator_coefficients::<CS>(curve);
    let offsets = curve.offsets();
    let mut accumulator = constant_point(&offsets[0]);
    for i in 0..32 {
        let d1 = scalar_window(&u1, 248 - 8 * i, 8);
        let d2_hi = scalar_window(&u2, 252 - 8 * i, 4);
        let d2_lo = scalar_window(&u2, 248 - 8 * i, 4);
        let q_hi = lookup_point(circuit, d2_hi, &q_table);
        let q_lo = lookup_point(circuit, d2_lo, &q_table);
        let g = lookup_generator_byte(circuit, d1, &g_table);
        for _ in 0..4 {
            accumulator = double_complete(circuit, curve, accumulator);
        }
        accumulator = add_distinct(circuit, curve, accumulator, q_hi);
        for _ in 0..4 {
            accumulator = double_complete(circuit, curve, accumulator);
        }
        accumulator = add_distinct(circuit, curve, accumulator, q_lo);
        accumulator = add_distinct(circuit, curve, accumulator, g);
    }
    // Thirty-two rounds of eight doublings leave `2^256 R`.
    add_complete(circuit, curve, accumulator, constant_point(&offsets[2]))
}

fn assert_on_curve<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    x: &Elem<CS>,
    y: &Elem<CS>,
) {
    let x = of_elem(x);
    let y = of_elem(y);
    let x2 = lazy_mul(circuit, curve.base(), x.clone(), x.clone());
    let x3 = lazy_mul(circuit, curve.base(), x2, x.clone());
    let rhs = curve.curve_equation_rhs(x3, x, rep_constant(&curve.b));
    lazy_assert_mul_eq(circuit, curve.base(), y.clone(), y, rhs);
}

fn assert_elem_eq<CS: Circuit>(circuit: &mut CS, left: &Elem<CS>, right: &Elem<CS>) {
    assert_zero(
        circuit,
        left.value.value.clone().sub(right.value.value.clone()),
    );
}

/// Builds the standalone P-256 ECDSA verifier from seven little-endian
/// 256-bit input words ordered as digest, Q.x, Q.y, r, s, r^-1, s^-1.
pub fn verify_digest_circuit<CS: Circuit>(
    circuit: &mut CS,
    inputs: &[CS::Bool; VERIFY_DIGEST_INPUT_BITS],
) {
    verify_digest_circuit_on(circuit, &P256, inputs)
}

/// [`verify_digest_circuit`] over an explicit curve.
pub fn verify_digest_circuit_on<CS: Circuit>(
    circuit: &mut CS,
    curve: &'static Curve,
    inputs: &[CS::Bool; VERIFY_DIGEST_INPUT_BITS],
) {
    let words: [<CS::Bool as BoolWitness>::Repr<256, 4>; 7] = array::from_fn(|slot| {
        <CS::Bool as BoolWitness>::Repr::from_array(array::from_fn(|bit| {
            inputs[slot * WIDTH + bit].clone()
        }))
    });
    // These seven U.fromWord calls are deliberately kept in input order.
    let mut words = words.into_iter();
    let digest = lift_input_word(circuit, words.next().unwrap());
    let qx = lift_input_word(circuit, words.next().unwrap());
    let qy = lift_input_word(circuit, words.next().unwrap());
    let r = lift_input_word(circuit, words.next().unwrap());
    let s = lift_input_word(circuit, words.next().unwrap());
    let r_inverse = lift_input_word(circuit, words.next().unwrap());
    let s_inverse = lift_input_word(circuit, words.next().unwrap());

    let qx = of_u(circuit, curve.base(), qx.elem.value);
    let qy = of_u(circuit, curve.base(), qy.elem.value);
    let r = of_u(circuit, curve.scalar(), r.elem.value);
    let s = of_u(circuit, curve.scalar(), s.elem.value);
    let r_inverse = of_u(circuit, curve.scalar(), r_inverse.elem.value);
    let s_inverse = of_u(circuit, curve.scalar(), s_inverse.elem.value);

    assert_on_curve(circuit, curve, &qx, &qy);
    lazy_assert_mul_eq(
        circuit,
        curve.scalar(),
        of_elem(&r),
        of_elem(&r_inverse),
        rep_u64(1),
    );
    lazy_assert_mul_eq(
        circuit,
        curve.scalar(),
        of_elem(&s),
        of_elem(&s_inverse),
        rep_u64(1),
    );

    let z = relaxed_reduce_small(circuit, curve.scalar(), digest.elem.value.value);
    let u1_relaxed = relaxed_mul(circuit, curve.scalar(), z, s_inverse.clone());
    let u2_relaxed = relaxed_mul(circuit, curve.scalar(), r.clone(), s_inverse);
    let u1 = lazy_reduce_scalar(circuit, curve.scalar(), of_elem(&u1_relaxed));
    let u2 = lazy_reduce_scalar(circuit, curve.scalar(), of_elem(&u2_relaxed));

    let q = point_from_elems(&qx, &qy);
    let sum = match &curve.endomorphism {
        Some(endomorphism) => glv_scalar_mul(circuit, curve, endomorphism, u1, u2, q),
        None => joint_scalar_mul(circuit, curve, u1, u2, q),
    };
    assert_zero(circuit, sum.infinity);
    let x_canonical = lazy_reduce(circuit, curve.base(), sum.x);
    let x_mod_n = relaxed_reduce_small(circuit, curve.scalar(), x_canonical.value.value);
    assert_elem_eq(circuit, &x_mod_n, &r);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stats::{Dummy, LeanStats, Stats};
    use crate::witgen::{ProductWitgen, WitnessOnly};
    use num_bigint::BigInt;

    fn stored_bigint(value: &[u64]) -> BigInt {
        let bytes = value
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        BigInt::from_signed_bytes_le(&bytes)
    }

    /// Packs `(digest, Qx, Qy, r, s, r^-1, s^-1)` the way the verifier reads
    /// them: seven little-endian 256-bit words of input bits.
    fn pack_inputs(curve: &'static Curve, words: [[u8; 32]; 5]) -> Box<[bool; VERIFY_DIGEST_INPUT_BITS]> {
        let n = uint256_biguint(curve.scalar);
        let big = |bytes: &[u8; 32]| BigUint::from_bytes_be(bytes);
        let (r, s) = (big(&words[3]), big(&words[4]));
        let values = [
            big(&words[0]),
            big(&words[1]),
            big(&words[2]),
            r.clone(),
            s.clone(),
            r.modinv(&n).expect("r invertible"),
            s.modinv(&n).expect("s invertible"),
        ];
        let bits: Box<[bool]> = (0..VERIFY_DIGEST_INPUT_BITS)
            .map(|index| values[index / 256].bit((index % 256) as u64))
            .collect();
        bits.try_into().unwrap()
    }

    /// Runs the verifier and returns whether every R1CS row is satisfied.
    fn r1cs_holds(curve: &'static Curve, inputs: &[bool; VERIFY_DIGEST_INPUT_BITS]) -> bool {
        let mut witgen =
            ProductWitgen::with_inputs_and_capacity(inputs, VERIFY_DIGEST_WITNESS_BITS);
        verify_digest_circuit_on(&mut witgen, curve, inputs);
        let products = witgen.products();
        products
            .a_mw
            .iter()
            .zip(products.b_mw.iter())
            .zip(products.c_mw.iter())
            .all(|((a, b), c)| stored_bigint(a) * stored_bigint(b) == stored_bigint(c))
    }

    fn words_biguint(words: &[u64]) -> BigUint {
        BigUint::new(
            words
                .iter()
                .flat_map(|word| [*word as u32, (*word >> 32) as u32])
                .collect(),
        )
    }

    fn valid_input() -> Box<[bool; VERIFY_DIGEST_INPUT_BITS]> {
        let r = uint256_biguint(P256.generator.0);
        let s = &r + BigUint::one();
        let values = [
            BigUint::one(),
            uint256_biguint(P256.generator.0),
            uint256_biguint(P256.generator.1),
            r.clone(),
            s.clone(),
            r.modpow(
                &(uint256_biguint(P256.scalar) - 2u32),
                &uint256_biguint(P256.scalar),
            ),
            s.modpow(
                &(uint256_biguint(P256.scalar) - 2u32),
                &uint256_biguint(P256.scalar),
            ),
        ];
        let bits: Box<[bool]> = (0..VERIFY_DIGEST_INPUT_BITS)
            .map(|index| values[index / WIDTH].bit((index % WIDTH) as u64))
            .collect();
        bits.try_into().unwrap()
    }

    #[test]
    fn verifier_dimensions_exactly_match_lean() {
        let mut stats = Stats::new(VERIFY_DIGEST_INPUT_BITS);
        verify_digest_circuit(&mut stats, &[Dummy; VERIFY_DIGEST_INPUT_BITS]);
        assert_eq!(
            stats.lean_stats(),
            LeanStats {
                m_rows: VERIFY_DIGEST_INTEGER_WITNESS_BITS,
                m_cols: VERIFY_DIGEST_INTEGER_WITNESS_BITS,
                r1cs_rows: VERIFY_DIGEST_R1CS_ROWS,
            }
        );
    }

    /// The distinct-operand addition must be unprovable, not merely wrong, when
    /// its operands coincide: with `dx = dy = 0` the slope would otherwise be
    /// unconstrained and the sum forgeable.
    #[test]
    #[should_panic(expected = "share an x coordinate")]
    fn coinciding_addition_operands_cannot_be_witnessed() {
        let mut witgen = WitnessOnly::with_inputs_and_capacity(&[], 4096);
        assert_nonzero_when(
            &mut witgen,
            &SECP256K1,
            rep_u64::<WitnessOnly>(0),
            lc_u64::<WitnessOnly>(1),
        );
    }

    /// The same test with the guard inactive: an infinite operand makes the
    /// constraint vacuous, which is what lets a zero window digit still work.
    #[test]
    fn an_infinite_operand_leaves_the_guard_vacuous() {
        let mut witgen = WitnessOnly::with_inputs_and_capacity(&[], 4096);
        assert_nonzero_when(
            &mut witgen,
            &SECP256K1,
            rep_u64::<WitnessOnly>(0),
            lc_u64::<WitnessOnly>(0),
        );
    }

    /// secp256k1 costs less than P-256 only because of GLV. The moduli and `a`
    /// enter as linear-combination coefficients alone, so a curve swap on its
    /// own is dimension-neutral; the whole delta is the halved doubling chain.
    #[test]
    fn secp256k1_saves_exactly_the_halved_doubling_chain() {
        let mut stats = Stats::new(VERIFY_DIGEST_INPUT_BITS);
        verify_digest_circuit_on(&mut stats, &SECP256K1, &[Dummy; VERIFY_DIGEST_INPUT_BITS]);
        assert_eq!(
            stats.lean_stats(),
            LeanStats {
                m_rows: SECP256K1_VERIFY_DIGEST_INTEGER_WITNESS_BITS,
                m_cols: SECP256K1_VERIFY_DIGEST_INTEGER_WITNESS_BITS,
                r1cs_rows: SECP256K1_VERIFY_DIGEST_R1CS_ROWS,
            }
        );
        // 128 doublings at 2,084 bits each, less the endomorphism overhead.
        let saved = VERIFY_DIGEST_WITNESS_BITS - SECP256K1_VERIFY_DIGEST_WITNESS_BITS;
        assert!(
            (235_000..=245_000).contains(&saved),
            "unexpected GLV saving: {saved} bits"
        );
        // Both ladders run 96 in-loop additions, so dropping the doubling and
        // `P = -Q` case analysis is worth the same absolute amount on each.
        assert_eq!(
            VERIFY_DIGEST_WITNESS_BITS % 2,
            SECP256K1_VERIFY_DIGEST_WITNESS_BITS % 2
        );
    }

    /// Both curves must accept a signature produced by a third-party
    /// implementation and reject a tampered one. This is what actually pins the
    /// constants and the `a`-dependent formulas: an instance built from our own
    /// constants would agree with a wrong curve just as happily.
    #[test]
    fn third_party_signatures_satisfy_the_matching_curve_only() {
        use k256::ecdsa::signature::hazmat::PrehashSigner;

        let digest: [u8; 32] = blake3::hash(b"circuit/ecdsa/oracle").into();

        // secp256k1, signed by k256.
        let key = k256::ecdsa::SigningKey::from_bytes(&[3u8; 32].into()).unwrap();
        let signature: k256::ecdsa::Signature = key.sign_prehash(&digest).unwrap();
        let point = key.verifying_key().to_encoded_point(false);
        let (r, s) = signature.split_bytes();
        let secp = pack_inputs(
            &SECP256K1,
            [
                digest,
                (*point.x().unwrap()).into(),
                (*point.y().unwrap()).into(),
                r.into(),
                s.into(),
            ],
        );
        assert!(r1cs_holds(&SECP256K1, &secp), "secp256k1 rejected a valid k256 signature");
        // The same public words on P-256: Q is not on that curve.
        assert!(!r1cs_holds(&P256, &secp), "P-256 accepted a secp256k1 instance");

        // P-256, signed by p256.
        use p256::ecdsa::signature::hazmat::PrehashSigner as _;
        let key = p256::ecdsa::SigningKey::from_bytes(&[3u8; 32].into()).unwrap();
        let signature: p256::ecdsa::Signature = key.sign_prehash(&digest).unwrap();
        let point = key.verifying_key().to_encoded_point(false);
        let (r, s) = signature.split_bytes();
        let nist = pack_inputs(
            &P256,
            [
                digest,
                (*point.x().unwrap()).into(),
                (*point.y().unwrap()).into(),
                r.into(),
                s.into(),
            ],
        );
        assert!(r1cs_holds(&P256, &nist), "P-256 rejected a valid p256 signature");
        assert!(!r1cs_holds(&SECP256K1, &nist), "secp256k1 accepted a P-256 instance");

        // Tampering with s must break the relation on its own curve.
        let mut forged = *secp;
        forged[4 * 256] ^= true;
        assert!(!r1cs_holds(&SECP256K1, &forged), "accepted a tampered s");
    }

    #[test]
    fn fixed_width_division_matches_biguint() {
        let mut state = 0x4d59_5df4_d0f3_3173_u64;
        for modulus in [P256.base(), P256.scalar()] {
            for _ in 0..1_000 {
                let mut words = [0; 18];
                for word in &mut words {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    *word = state;
                }
                let value = Uint::from_words(words);
                let (quotient, remainder) = modulus.divisor().div_rem_ct(&value);
                let value = words_biguint(&words);
                let expected_quotient = &value / uint256_biguint(modulus.words());
                let expected_remainder = value % uint256_biguint(modulus.words());
                assert_eq!(words_biguint(quotient.as_words()), expected_quotient);
                assert_eq!(uint256_biguint(remainder), expected_remainder);
            }
        }
    }

    #[test]
    fn representative_division_matches_biguint_at_full_capacity() {
        let capacity = BigUint::one() << 320usize;
        let radix = BigUint::one() << 256usize;
        let mut state = 0x7a9d_29a4_d375_198b_u64;
        for modulus in [P256.base(), P256.scalar()] {
            let p = uint256_biguint(modulus.words());
            let complement = &radix - &p;
            assert!(complement > BigUint::zero());
            assert!(complement < (BigUint::one() << 224usize));
            let mut values = vec![BigUint::zero(), &capacity - 1u32];
            for bit in 0..320usize {
                let power = BigUint::one() << bit;
                values.extend([&power - 1u32, power.clone(), &power + 1u32]);
            }
            for multiplier in [0u128, 1, 2, 66, u64::MAX as u128, 1u128 << 64] {
                let multiple = &p * BigUint::from(multiplier);
                if multiple > BigUint::zero() {
                    values.push(&multiple - 1u32);
                }
                values.extend([multiple.clone(), &multiple + 1u32, &multiple + &p - 1u32]);
            }
            // Exercise carry propagation from every possible high-word width,
            // with both zero and maximum lower words.
            for bit in 0..64 {
                for high in [(1u64 << bit) - 1, 1u64 << bit, u64::MAX] {
                    let high_part = BigUint::from(high) << 256usize;
                    values.extend([high_part.clone(), &high_part + &radix - 1u32]);
                }
            }
            for _ in 0..10_000 {
                let words = array::from_fn::<_, 5, _>(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state
                });
                values.push(words_biguint(&words));
            }
            for value in values.into_iter().filter(|value| value < &capacity) {
                let words = value.to_u64_digits();
                let input = Uint::from_words(array::from_fn(|i| words.get(i).copied().unwrap_or(0)));
                let (quotient, remainder) = modulus.div_rem_representative(input);
                assert_eq!(words_biguint(quotient.as_words()), &value / &p, "quotient for {value:x}");
                assert_eq!(uint256_biguint(remainder), &value % &p, "remainder for {value:x}");
            }
        }
    }

    #[test]
    fn valid_signature_generates_the_exact_sized_witness() {
        let inputs = valid_input();
        let mut witness_only =
            WitnessOnly::with_inputs_and_capacity(inputs.as_ref(), VERIFY_DIGEST_WITNESS_BITS);
        verify_digest_circuit(&mut witness_only, &inputs);
        assert_eq!(witness_only.witness().bit_len(), VERIFY_DIGEST_WITNESS_BITS);

        let mut witgen =
            ProductWitgen::with_inputs_and_capacity(inputs.as_ref(), VERIFY_DIGEST_WITNESS_BITS);
        verify_digest_circuit(&mut witgen, &inputs);
        assert_eq!(witness_only.witness(), witgen.witness());
        assert_eq!(witgen.witness().bit_len(), VERIFY_DIGEST_WITNESS_BITS);
        assert_eq!(
            witgen.integer_witness().bit_len(),
            VERIFY_DIGEST_INTEGER_WITNESS_BITS
        );
        assert_eq!(witgen.products().a_mw.len(), VERIFY_DIGEST_R1CS_ROWS);
        assert_eq!(witgen.products().b_mw.len(), VERIFY_DIGEST_R1CS_ROWS);
        assert_eq!(witgen.products().c_mw.len(), VERIFY_DIGEST_R1CS_ROWS);
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
