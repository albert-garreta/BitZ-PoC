//! Their `field::fq::Fq<Q100>`: arithmetic modulo `2^100 − 15` on canonical
//! `u128` residues — the same schoolbook `mul_wide`, the same Barrett
//! `reduce_wide`, the same 16-byte little-endian wire form
//! (`field/src/codec.rs`) and the same rejection-sampled squeeze
//! (`transcript/src/challenge.rs:37-55`). Any exact arithmetic gives the
//! same values; only the wire form and the squeeze rule can break parity,
//! but porting theirs verbatim is the shortest path to the first pin.
//!
//! Nothing here divides: their `Inv`/`Div` are `unimplemented!()` and the
//! e2e never calls them, so neither does this.

use std::iter::{Product, Sum};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use spongefish::Encoding;

use super::transcript::{ProverState, VerifierState};

/// `2^100 − 15`, prime: their `field::Q100`.
pub const Q: u128 = (1u128 << 100) - 15;

/// Bit length of the modulus, `2^(BITS−1) ≤ Q < 2^BITS`.
const BITS: u32 = 128 - Q.leading_zeros();

/// `⌊2^(2·BITS) / Q⌋`, Barrett's precomputed reciprocal (their `MU`).
const MU: u128 = barrett_mu(Q, BITS);

/// An element of `Z/QZ`, held reduced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fq(u128);

/// Full 128×128 → 256-bit product as `(low, high)` (their `mul_wide`).
#[inline]
pub(crate) const fn mul_wide(a: u128, b: u128) -> (u128, u128) {
    let (a_lo, a_hi) = (a as u64 as u128, a >> 64);
    let (b_lo, b_hi) = (b as u64 as u128, b >> 64);

    let ll = a_lo * b_lo;
    let hh = a_hi * b_hi;
    let (mid, mid_carry) = (a_lo * b_hi).overflowing_add(a_hi * b_lo);

    let (lo, lo_carry) = ll.overflowing_add(mid << 64);
    let hi = hh + (mid >> 64) + ((mid_carry as u128) << 64) + lo_carry as u128;
    (lo, hi)
}

/// `(lo, hi) >> n` for `n < 128`, keeping the low 128 bits (their `shr_wide`).
#[inline]
const fn shr_wide(lo: u128, hi: u128, n: u32) -> u128 {
    if n == 0 {
        lo
    } else {
        (lo >> n) | (hi << (128 - n))
    }
}

/// `⌊2^(2k) / q⌋` by binary long division (their `barrett_mu`).
const fn barrett_mu(q: u128, k: u32) -> u128 {
    let mut rem = 0u128;
    let mut quo = 0u128;
    let mut i = 2 * k;
    loop {
        rem = (rem << 1) | (i == 2 * k) as u128;
        let fits = rem >= q;
        if fits {
            rem -= q;
        }
        quo = (quo << 1) | fits as u128;
        if i == 0 {
            return quo;
        }
        i -= 1;
    }
}

impl Fq {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1);

    /// Reduces its input, so any `u128` is accepted (their `From<u128>`).
    #[inline]
    pub const fn new(value: u128) -> Self {
        Self(value % Q)
    }

    /// Two little-endian `u64` limbs, reduced (their `from_limbs`).
    pub const fn from_limbs(low: u64, high: u64) -> Self {
        Self::new(low as u128 | ((high as u128) << 64))
    }

    /// The least non-negative representative, in `[0, Q)` (their `lift`).
    #[inline]
    pub const fn lift(self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Barrett reduction of a 256-bit product below `Q²` (their `reduce_wide`).
    #[inline]
    const fn reduce_wide(lo: u128, hi: u128) -> Self {
        let k = BITS;
        let q1 = shr_wide(lo, hi, k - 1);
        let (q2_lo, q2_hi) = mul_wide(q1, MU);
        let q3 = shr_wide(q2_lo, q2_hi, k + 1);

        let mut r = lo.wrapping_sub(mul_wide(q3, Q).0);
        if r >= Q {
            r -= Q;
        }
        if r >= Q {
            r -= Q;
        }
        Self(r)
    }

    /// The wire form: the canonical residue as 16 little-endian bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }

    /// The inverse of [`Self::to_bytes`]; `None` at or above `Q` (their
    /// `NargDeserialize` rejects non-canonical residues).
    pub const fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
        let value = u128::from_le_bytes(bytes);
        if value >= Q { None } else { Some(Self(value)) }
    }

    /// Their `TranscriptChallenge`: squeeze `u128`s, reject the incomplete
    /// final interval so every residue has the same number of preimages,
    /// reduce the first accepted one.
    pub fn from_squeezes(mut next_u128: impl FnMut() -> u128) -> Self {
        let rejection_remainder = (u128::MAX % Q + 1) % Q;
        let max_accepted = u128::MAX - rejection_remainder;
        loop {
            let candidate = next_u128();
            if candidate <= max_accepted {
                return Self::new(candidate);
            }
        }
    }
}

impl From<u128> for Fq {
    fn from(value: u128) -> Self {
        Self::new(value)
    }
}

impl From<u64> for Fq {
    fn from(value: u64) -> Self {
        Self(value as u128)
    }
}

impl From<bool> for Fq {
    fn from(value: bool) -> Self {
        if value { Self::ONE } else { Self::ZERO }
    }
}

impl Neg for Fq {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(if self.0 == 0 { 0 } else { Q - self.0 })
    }
}

impl Add for Fq {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        // Both operands are below `Q < 2^126`, so the sum cannot wrap.
        let s = self.0 + rhs.0;
        Self(if s >= Q { s - Q } else { s })
    }
}

impl Sub for Fq {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(if self.0 >= rhs.0 {
            self.0 - rhs.0
        } else {
            self.0 + Q - rhs.0
        })
    }
}

impl Mul for Fq {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let (lo, hi) = mul_wide(self.0, rhs.0);
        Self::reduce_wide(lo, hi)
    }
}

impl AddAssign for Fq {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fq {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fq {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Sum for Fq {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Fq> for Fq {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + *x)
    }
}

impl Product for Fq {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, Mul::mul)
    }
}

/// Their `Encoding<[u8]> for Fq`: `lift().to_le_bytes()`.
impl Encoding<[u8]> for Fq {
    fn encode(&self) -> impl AsRef<[u8]> {
        self.to_bytes()
    }
}

impl ProverState {
    /// Their `ProverState::squeeze::<Fq>()`.
    pub fn squeeze_fq(&mut self) -> Fq {
        Fq::from_squeezes(|| self.verifier_message::<u128>())
    }
}

impl VerifierState<'_> {
    /// Their `VerifierState::squeeze::<Fq>()`.
    pub fn squeeze_fq(&mut self) -> Fq {
        Fq::from_squeezes(|| self.verifier_message::<u128>())
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::super::transcript::{build_prover, build_verifier};
    use super::*;

    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn u128_of(state: &mut u64) -> u128 {
        (u128::from(splitmix(state)) << 64) | u128::from(splitmix(state))
    }

    fn reference_mul(a: u128, b: u128) -> u128 {
        let q = BigUint::from(Q);
        let r = (BigUint::from(a) * BigUint::from(b)) % q;
        r.to_u64_digits()
            .iter()
            .rev()
            .fold(0u128, |acc, &d| (acc << 64) | u128::from(d))
    }

    #[test]
    fn the_modulus_and_the_reciprocal() {
        assert_eq!(BITS, 100);
        assert_eq!(MU, (1u128 << 100) + 15, "⌊2^200 / (2^100 − 15)⌋");
        assert_eq!(Fq::new(Q).lift(), 0);
        assert_eq!(Fq::new(Q + 1).lift(), 1);
        assert_eq!(Fq::new(u128::MAX).lift(), u128::MAX % Q);
        assert_eq!(Fq::from_limbs(0, 1).lift(), (1u128 << 64) % Q);
    }

    #[test]
    fn mul_wide_matches_u128_on_small_inputs() {
        assert_eq!(mul_wide(u128::MAX, u128::MAX), (1, u128::MAX - 1));
        assert_eq!(mul_wide(1u128 << 127, 2), (0, 1));
        let mut state = 401;
        for _ in 0..256 {
            let (a, b) = (u128::from(splitmix(&mut state)), u128::from(splitmix(&mut state)));
            assert_eq!(mul_wide(a, b), (a * b, 0));
        }
    }

    #[test]
    fn barrett_matches_the_bigint_reference() {
        let mut state = 402;
        for _ in 0..4096 {
            let (a, b) = (u128_of(&mut state) % Q, u128_of(&mut state) % Q);
            assert_eq!((Fq::new(a) * Fq::new(b)).lift(), reference_mul(a, b), "{a} * {b}");
        }
        for &a in &[0, 1, Q - 1, Q / 2, Q - 2] {
            for &b in &[0, 1, Q - 1, Q / 2, Q - 2] {
                assert_eq!((Fq::new(a) * Fq::new(b)).lift(), reference_mul(a, b), "{a} * {b}");
            }
        }
    }

    #[test]
    fn ring_axioms() {
        let mut state = 403;
        for _ in 0..512 {
            let a = Fq::new(u128_of(&mut state));
            let b = Fq::new(u128_of(&mut state));
            let c = Fq::new(u128_of(&mut state));
            assert_eq!(a * b, b * a);
            assert_eq!((a * b) * c, a * (b * c));
            assert_eq!(a * (b + c), a * b + a * c);
            assert_eq!((a + b) + c, a + (b + c));
            assert_eq!(a - a, Fq::ZERO);
            assert_eq!(a + b - b, a);
            assert_eq!(a + (-a), Fq::ZERO);
            assert_eq!(a * Fq::ONE, a);
            assert_eq!(a * Fq::ZERO, Fq::ZERO);
            assert_eq!((a - b).lift(), (a.lift() + Q - b.lift()) % Q);
        }
    }

    #[test]
    fn the_wire_form_is_the_little_endian_residue() {
        for v in [0u128, 1, 12345, Q - 1] {
            let a = Fq::new(v);
            assert_eq!(a.to_bytes(), v.to_le_bytes());
            assert_eq!(a.encode().as_ref(), v.to_le_bytes());
            assert_eq!(Fq::from_bytes(a.to_bytes()), Some(a));
        }
        for v in [Q, Q + 1, u128::MAX] {
            assert_eq!(Fq::from_bytes(v.to_le_bytes()), None);
        }
    }

    /// Their `fq_rejects_the_incomplete_final_interval`.
    #[test]
    fn the_squeeze_rejects_the_incomplete_final_interval() {
        let rejection_remainder = (u128::MAX % Q + 1) % Q;
        let max_accepted = u128::MAX - rejection_remainder;
        assert_eq!(max_accepted % Q, Q - 1);
        assert_eq!((max_accepted + 1) % Q, 0);
        let mut candidates = [max_accepted + 1, u128::MAX, max_accepted].into_iter();
        let mut squeezes = 0;
        let challenge = Fq::from_squeezes(|| {
            squeezes += 1;
            candidates.next().unwrap()
        });
        assert_eq!(squeezes, 3);
        assert_eq!(challenge, Fq::new(Q - 1));
    }

    #[test]
    fn prover_and_verifier_squeeze_in_lockstep() {
        let session = b"transcript/typed-challenge/test";
        let instance = b"fq-rejection-sampling";
        let mut prover = build_prover(session, instance);
        let prover_fq = prover.squeeze_fq();
        let prover_gf: crate::poly::univariate::binary_gf128::BinaryFieldGF128 =
            prover.verifier_message();
        let proof = prover.finish();
        let mut verifier = build_verifier(session, instance, &proof);
        assert_eq!(verifier.squeeze_fq(), prover_fq);
        let verifier_gf: crate::poly::univariate::binary_gf128::BinaryFieldGF128 =
            verifier.verifier_message();
        assert_eq!(verifier_gf, prover_gf);
        assert!(prover_fq.lift() < Q);
        verifier.check_eof().unwrap();
    }
}
