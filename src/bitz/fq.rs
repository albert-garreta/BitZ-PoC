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
//!
//! The modulus is installed at runtime (their `Fq<RUNTIME>` on branch
//! `bitz-k4-prime`): [`Q`] (`2^100 − 15`) until [`set_modulus`] installs the
//! prime a proof sampled from its transcript. Every operation reads it, so
//! one proof at a time per process, and a value made under one modulus
//! means nothing under another.

use std::iter::{Product, Sum};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use spongefish::Encoding;

use super::transcript::{ProverState, VerifierState};

/// `2^100 − 15`, prime: their `field::Q100`, the modulus until one is installed.
pub const Q: u128 = (1u128 << 100) - 15;

const BITS100: u32 = 128 - Q.leading_zeros();
const MU100: u128 = barrett_mu(Q, BITS100);

static Q_LO: AtomicU64 = AtomicU64::new(Q as u64);
static Q_HI: AtomicU64 = AtomicU64::new((Q >> 64) as u64);
static MU_LO: AtomicU64 = AtomicU64::new(MU100 as u64);
static MU_HI: AtomicU64 = AtomicU64::new((MU100 >> 64) as u64);
static BITS: AtomicU32 = AtomicU32::new(BITS100);

/// Tests that install a modulus hold this: the modulus is process-wide
/// and the test harness runs tests in parallel.
#[cfg(test)]
pub(crate) fn test_modulus_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The installed modulus.
#[inline]
pub fn modulus() -> u128 {
    u128::from(Q_LO.load(Ordering::Relaxed)) | (u128::from(Q_HI.load(Ordering::Relaxed)) << 64)
}

#[inline]
fn mu() -> u128 {
    u128::from(MU_LO.load(Ordering::Relaxed)) | (u128::from(MU_HI.load(Ordering::Relaxed)) << 64)
}

#[inline]
fn bits() -> u32 {
    BITS.load(Ordering::Relaxed)
}

/// A modulus [`set_modulus`] rejects (their `ModulusError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulusError {
    /// Below 3, even, or at or above `2^126` (the Barrett bound).
    OutOfRange,
    /// Fails the probable-prime test.
    NotPrime,
}

/// Installs `q` as the modulus: an odd probable prime below `2^126`, its
/// Barrett constants computed here (their `set_modulus`).
pub fn set_modulus(q: u128) -> Result<(), ModulusError> {
    if q < 3 || q % 2 == 0 || q >= 1u128 << 126 {
        return Err(ModulusError::OutOfRange);
    }
    if !is_prime(q) {
        return Err(ModulusError::NotPrime);
    }
    let bits = 128 - q.leading_zeros();
    let mu = barrett_mu(q, bits);
    Q_LO.store(q as u64, Ordering::Relaxed);
    Q_HI.store((q >> 64) as u64, Ordering::Relaxed);
    MU_LO.store(mu as u64, Ordering::Relaxed);
    MU_HI.store((mu >> 64) as u64, Ordering::Relaxed);
    BITS.store(bits, Ordering::Relaxed);
    Ok(())
}

/// Their probable-prime test (`field::is_probable_prime`): trial division
/// by the first thirteen primes, then Miller–Rabin with those bases —
/// deterministic below `2^81.4`, a strong probable-prime test above, which
/// a transcript-derived candidate cannot be chosen to defeat. The prime a
/// transcript yields is the first candidate this accepts, so it is theirs
/// verbatim.
pub fn is_probable_prime(candidate: u128) -> bool {
    is_prime(candidate)
}

/// The first thirteen primes, their Miller–Rabin base set.
const PRIMALITY_BASES: [u128; 13] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];

const fn add_mod(a: u128, b: u128, q: u128) -> u128 {
    let sum = a + b;
    if sum >= q { sum - q } else { sum }
}

/// `(a * b) mod q` by doubling (their `mul_mod`).
const fn mul_mod(a: u128, b: u128, q: u128) -> u128 {
    let mut result = 0u128;
    let mut addend = a % q;
    let mut remaining = b;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = add_mod(result, addend, q);
        }
        addend = add_mod(addend, addend, q);
        remaining >>= 1;
    }
    result
}

const fn pow_mod(base: u128, exponent: u128, q: u128) -> u128 {
    let mut result = 1u128 % q;
    let mut square = base % q;
    let mut remaining = exponent;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = mul_mod(result, square, q);
        }
        square = mul_mod(square, square, q);
        remaining >>= 1;
    }
    result
}

/// Their `is_prime`, verbatim.
const fn is_prime(candidate: u128) -> bool {
    if candidate < 2 {
        return false;
    }
    let mut index = 0;
    while index < PRIMALITY_BASES.len() {
        let base = PRIMALITY_BASES[index];
        if candidate == base {
            return true;
        }
        if candidate % base == 0 {
            return false;
        }
        index += 1;
    }
    let shift = (candidate - 1).trailing_zeros();
    let odd = (candidate - 1) >> shift;
    let mut index = 0;
    while index < PRIMALITY_BASES.len() {
        let mut witness = pow_mod(PRIMALITY_BASES[index], odd, candidate);
        if witness != 1 && witness != candidate - 1 {
            let mut round = 1;
            loop {
                if round >= shift {
                    return false;
                }
                witness = mul_mod(witness, witness, candidate);
                if witness == candidate - 1 {
                    break;
                }
                round += 1;
            }
        }
        index += 1;
    }
    true
}

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

/// All ones when `condition`, zero otherwise (no branch).
#[inline(always)]
const fn mask(condition: bool) -> u128 {
    0u128.wrapping_sub(condition as u128)
}

/// `value − q` when `value ≥ q`, else `value` — a compare and a masked
/// subtraction, no branch.
#[inline(always)]
const fn conditional_subtract(value: u128, q: u128) -> u128 {
    value.wrapping_sub(q & mask(value >= q))
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
    pub fn new(value: u128) -> Self {
        Self(value % modulus())
    }

    /// Two little-endian `u64` limbs, reduced (their `from_limbs`).
    pub fn from_limbs(low: u64, high: u64) -> Self {
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

    /// Barrett reduction of a 256-bit product below `q²` (their `reduce_wide`).
    #[inline]
    fn reduce_wide(lo: u128, hi: u128) -> Self {
        let q = modulus();
        let k = bits();
        let q1 = shr_wide(lo, hi, k - 1);
        let (q2_lo, q2_hi) = mul_wide(q1, mu());
        let q3 = shr_wide(q2_lo, q2_hi, k + 1);

        let r = lo.wrapping_sub(mul_wide(q3, q).0);
        // The estimate is short by at most two: two conditional
        // subtractions, branchless. With `q = 2^100 − 15` the estimate is
        // almost always exact and a branch predicts; under a prime drawn
        // from the transcript it is short a good fraction of the time and
        // the mispredictions cost the Spartan passes almost a factor of two.
        let r = conditional_subtract(r, q);
        let r = conditional_subtract(r, q);
        Self(r)
    }

    /// The wire form: the canonical residue as 16 little-endian bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }

    /// The inverse of [`Self::to_bytes`]; `None` at or above the modulus
    /// (their `NargDeserialize` rejects non-canonical residues).
    pub fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
        let value = u128::from_le_bytes(bytes);
        if value >= modulus() { None } else { Some(Self(value)) }
    }

    /// Their `TranscriptChallenge`: squeeze `u128`s, reject the incomplete
    /// final interval so every residue has the same number of preimages,
    /// reduce the first accepted one.
    pub fn from_squeezes(mut next_u128: impl FnMut() -> u128) -> Self {
        let q = modulus();
        let rejection_remainder = (u128::MAX % q + 1) % q;
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
        Self((modulus() - self.0) & mask(self.0 != 0))
    }
}

impl Add for Fq {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        // Both operands are below `q < 2^126`, so the sum cannot wrap.
        Self(conditional_subtract(self.0 + rhs.0, modulus()))
    }
}

impl Sub for Fq {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let (difference, borrow) = self.0.overflowing_sub(rhs.0);
        Self(difference.wrapping_add(modulus() & mask(borrow)))
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
        assert_eq!(modulus(), Q);
        assert_eq!(bits(), 100);
        assert_eq!(mu(), (1u128 << 100) + 15, "⌊2^200 / (2^100 − 15)⌋");
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

    /// Their vectors for the probable-prime test.
    #[test]
    fn the_primality_test_is_theirs() {
        for candidate in 0u128..2_000 {
            let expected = candidate >= 2
                && (2..candidate)
                    .take_while(|d| d * d <= candidate)
                    .all(|d| candidate % d != 0);
            assert_eq!(is_probable_prime(candidate), expected, "{candidate}");
        }
        for carmichael in [561u128, 41_041, 825_265] {
            assert!(!is_probable_prime(carmichael));
        }
        assert!(is_probable_prime((1 << 100) - 15));
        assert!(is_probable_prime((1 << 108) - 59));
        assert!(is_probable_prime((1 << 114) - 11));
        assert!(!is_probable_prime(((1u128 << 54) - 33) * ((1u128 << 53) - 111)));
        for offset in (1..11).step_by(2) {
            assert!(!is_probable_prime((1 << 114) - offset));
        }
        assert_eq!(set_modulus(4), Err(ModulusError::OutOfRange));
        assert_eq!(set_modulus(1u128 << 126), Err(ModulusError::OutOfRange));
        assert_eq!(set_modulus(561), Err(ModulusError::NotPrime));
    }

    /// Switches the installed modulus, so it cannot share a process with the
    /// other tests: `cargo test --release --features bitz-parity,parallel
    /// --lib bitz::fq::tests::the_installed_modulus_switches -- --ignored
    /// --test-threads=1`. The sampled-prime sweep exercises the same path
    /// end to end against their bytes.
    #[test]
    #[ignore]
    fn the_installed_modulus_switches() {
        const Q114: u128 = (1 << 114) - 11;
        set_modulus(Q114).unwrap();
        assert_eq!(modulus(), Q114);
        let mut state = 406;
        for _ in 0..2048 {
            let (a, b) = (u128_of(&mut state) % Q114, u128_of(&mut state) % Q114);
            let (x, y) = (Fq::new(a), Fq::new(b));
            let q = BigUint::from(Q114);
            let expected = (BigUint::from(a) * BigUint::from(b)) % q;
            let expected = expected.to_u64_digits().iter().rev().fold(0u128, |acc, &d| (acc << 64) | u128::from(d));
            assert_eq!((x * y).lift(), expected);
            assert_eq!((x + y).lift(), (a + b) % Q114);
            assert_eq!((x - y).lift(), (a + Q114 - b) % Q114);
        }
        set_modulus(251).unwrap();
        for a in 0..251u128 {
            for b in 0..251u128 {
                assert_eq!((Fq::new(a) * Fq::new(b)).lift(), a * b % 251);
            }
        }
        set_modulus(Q).unwrap();
        assert_eq!(Fq::new(Q + 7).lift(), 7);
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
