//! `GF(2^127)` ("b127"): degree-127 binary extension field with a
//! trinomial modulus.
//!
//! Elements are `F_2[X] / <f(X)>` where
//!
//! ```text
//! f(X) = X^127 + X + 1.
//! ```
//!
//! `f` is irreducible over `F_2` (a classical trinomial; cf. Zierler's
//! tables), so the factor ring is a field of order `2^127`. Two structural
//! facts distinguish it from the GHASH field ([`BinaryFieldGF128`]) and are
//! load-bearing for the exponent-fold PCS:
//!
//! * **The multiplicative group has PRIME order.** `|B^×| = 2^127 − 1` is
//!   the Mersenne prime `M_127` (Lucas, 1876). Every element outside
//!   `{0, 1}` therefore generates the full group — the generator check of
//!   the exponent binding collapses to `α ∉ {0, 1}` ([`is_generator_b127`]),
//!   with no order factorization and no resampling loop (contrast
//!   `GF(2^128)`, where only ≈49 % of `K^×` generates). The exponent map
//!   `n ↦ α^n` is injective on `[0, 2^127 − 1)`.
//! * **127 is prime**, so the only proper subfield is `F_2 = {0, 1}` —
//!   there are no intermediate towers (no `GF(2^8)`-style acceleration, and
//!   no `2^κ`-bit ring-switch packing: `[B : F_2] = 127` is not a power of
//!   two, which is exactly why this field cannot ride the flock Ligerito
//!   opener; see `docs/DESIGN.md`).
//!
//! # The speed trade (and how it measures)
//!
//! The reduction modulo the trinomial is two shifted XORs: writing a
//! carryless product `P = L + X^127·H` (`deg P ≤ 252` for canonical
//! operands, so `deg H ≤ 125`),
//!
//! ```text
//! P ≡ L ⊕ H ⊕ (H << 1)      (mod X^127 + X + 1),
//! ```
//!
//! and `deg((X+1)·H) ≤ 126 < 127` — ONE fold, exact, no carry chain. The
//! GHASH reduction by contrast costs a 3-PMULL fold. On the NEON pipeline
//! that turns a 7-PMULL reduced multiply into a 4-PMULL one (schoolbook
//! product + PMULL-free reduction), and the 5-PMULL GHASH squaring into
//! 2 PMULLs — trading PMULLs for shift/logical µops. **Measured verdict
//! (Apple M4, interleaved-rep harness): the trade LOSES there** — PMULL
//! throughput is abundant enough that b127 lands at 0.88–1.02× of the
//! GHASH pipeline across this repo's hot patterns (winning only the
//! squaring chain, 1.02×). It is the right trade on cores where carryless
//! multiply is port-constrained. Full study: `docs/b127-field.md`.
//!
//! Storage layout: each element is a [`Uint<2>`] (2 × `u64`) bit-packed
//! polynomial of degree `< 127`; bit `64·w + b` (LSB-first per limb) holds
//! the coefficient of `X^{64·w + b}`. **Canonical invariant: bit 127 (word
//! 1, bit 63) is zero.** Constructors that accept raw 128-bit patterns
//! either fold the top bit (`X^127 ≡ X + 1`: [`From<u128>`],
//! [`PrimeField::new_with_cfg`] — a 2-to-1 covering, so uniform 128-bit
//! transcript draws yield uniform field elements) or reject it
//! ([`Self::try_from_words`], the codec-canonicality entry point).
//!
//! Algorithm provenance: the element representation, the Karatsuba product
//! and the two-XOR trinomial fold follow Reilabs'
//! [`ghash-powers-bench`](https://github.com/reilabs/ghash-powers-bench)
//! (`b127`), restructured into this repo's NEON-resident idiom (the
//! products, the vector-register wide accumulators and the fused eq-factored
//! kernels are shared with / mirrored from [`binary_gf128`]; only the
//! reduction differs).
//!
//! [`BinaryFieldGF128`]: crate::poly::univariate::binary_gf128::BinaryFieldGF128
//! [`binary_gf128`]: crate::poly::univariate::binary_gf128

use core::{
    fmt::{Display, Formatter, Result as FmtResult},
    iter::{Product, Sum},
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};
use crypto_primitives::{
    Field, FieldError, PrimeField, Ring, Semiring, crypto_bigint_uint::Uint,
};
use crate::utils::inner_transparent_field::InnerTransparentField;
use num_traits::{
    CheckedAdd, CheckedMul, CheckedNeg, CheckedSub, ConstOne, ConstZero, Inv, One, Pow, Zero,
};

use crate::poly::univariate::binary_gf128::clmul_128x128;
// Only the scalar (non-NEON) squaring pipeline calls the 64×64 base mul —
// and the NEON↔scalar parity tests, which exercise it explicitly.
#[cfg(any(test, not(all(target_arch = "aarch64", target_feature = "neon"))))]
use crate::poly::univariate::binary_gf128::clmul_64x64;

/// Low bits of the reduction polynomial — `g(X) = X + 1`, stored as `0x3`
/// (bits 0 and 1 set). The `X^127` term is implicit in the reduction
/// routines: `X^127 ≡ g(X) mod f`.
pub const REDUCTION_LOW_B127: u64 = 0x3;

/// Word-1 mask clearing bit 127 (the canonical-invariant bit): a reduced
/// element satisfies `words()[1] & !MASK_HI_B127 == 0`.
pub const MASK_HI_B127: u64 = u64::MAX >> 1;

/// `Field::Modulus`-shaped representation of the reduction polynomial.
/// Unlike GHASH's (where the `X^128` term cannot be stored in 128 bits and
/// is left implicit), `f(X) = X^127 + X + 1` fits: bit 127 + bits 1, 0.
pub const B127_MODULUS: Uint<2> = Uint::<2>::from_words([REDUCTION_LOW_B127, 1u64 << 63]);

/// The order of the multiplicative group `B^× = GF(2^127)^×`:
/// `2^127 − 1 = 170141183460469231731687303715884105727`, the Mersenne
/// prime `M_127` (Lucas, 1876; the largest prime found by hand). Prime
/// group order makes EVERY non-identity element a generator, so the
/// exponent map `n ↦ α^n` is injective on `[0, 2^127 − 1)` for any
/// `α ∉ {0, 1}` — the b127 replacement for `GF128_MULT_ORDER` + the
/// 9-factor primitive-element test.
pub const B127_MULT_ORDER: u128 = u128::MAX >> 1;

/// Does `α` generate `B^×`? Prime group order (`M_127`, see
/// [`B127_MULT_ORDER`]) means the subgroup generated by any `α ∉ {0, 1}`
/// has order dividing a prime and exceeding 1 — i.e. the whole group. (And
/// `{0, 1}` is exactly the unique proper subfield `F_2`, since 127 is
/// prime.) A Fiat–Shamir `α` is thus a generator with probability
/// `1 − 2^{-126}`; no resampling loop, no order factorization.
#[inline]
pub fn is_generator_b127(alpha: &BinaryFieldB127) -> bool {
    !alpha.is_zero() && !One::is_one(alpha)
}

/// An element of `GF(2^127) = F_2[X] / <X^127 + X + 1>`.
///
/// Stored as a [`Uint<2>`] with **bit 127 always zero** (the canonical
/// invariant). Bit `64·w + b` (LSB-first within each `u64` limb) holds the
/// coefficient of `X^{64·w + b}`.
///
/// The `Uint<2>` storage is load-bearing for the transcript /
/// `Field::Inner` plumbing, exactly as for [`BinaryFieldGF128`]: `Uint<L>`
/// implements `ConstTranscribable`, which
/// `transcript.get_field_challenge::<F>` requires of `F::Inner`. A raw
/// 128-bit challenge draw is folded into the field by
/// [`PrimeField::new_with_cfg`] (`X^127 ≡ X + 1`), a 2-to-1 map — uniform
/// bytes give uniform elements.
///
/// [`BinaryFieldGF128`]: crate::poly::univariate::binary_gf128::BinaryFieldGF128
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct BinaryFieldB127 {
    uint: Uint<2>,
}

crate::transcript::delegate_const_transcribable!(BinaryFieldB127 { uint: Uint<2> });

impl BinaryFieldB127 {
    #[inline]
    pub const fn zero() -> Self {
        Self { uint: Uint::<2>::ZERO }
    }

    #[inline]
    pub const fn one() -> Self {
        Self {
            uint: Uint::<2>::from_words([1u64, 0]),
        }
    }

    /// Construct from raw bit-packed words. Caller is responsible for the
    /// canonical invariant (bit 127 clear); debug builds assert it. For
    /// untrusted bytes (proof decoding) use [`Self::try_from_words`]; for
    /// uniform-random 128-bit draws use `PrimeField::new_with_cfg` (which
    /// folds the top bit).
    #[inline]
    pub const fn from_words(words: [u64; 2]) -> Self {
        debug_assert!(words[1] >> 63 == 0, "GF(2^127): bit 127 must be zero");
        Self {
            uint: Uint::<2>::from_words(words),
        }
    }

    /// Canonicality-checked construction: `None` iff bit 127 is set. The
    /// proof-codec entry point — a non-canonical 16-byte pattern must be
    /// REJECTED, not silently folded, or two distinct byte streams would
    /// decode to the same element (malleability).
    #[inline]
    pub const fn try_from_words(words: [u64; 2]) -> Option<Self> {
        if words[1] >> 63 != 0 { None } else { Some(Self::from_words(words)) }
    }

    /// Borrow the bit-packed representation. Bit `i` of word `i / 64`
    /// holds the coefficient of `X^i`; bit 127 is always zero.
    #[inline]
    pub const fn words(&self) -> &[u64; 2] {
        self.uint.as_words()
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        let w = self.uint.as_words();
        w[0] == 0 && w[1] == 0
    }

    /// `a · X` — multiply by the field generator `X` (= `from(2)`; a
    /// generator like every non-identity element, prime group order). A
    /// left shift of the 127-bit representation by one, with
    /// `X^127 ≡ X + 1 = 0x3` folded back in when the top coefficient
    /// (`X^126`) overflows. Cheap (a few instructions, no CLMUL) — the
    /// substitute for the general multiply on node-`X` round-message
    /// evaluations, mirroring `BinaryFieldGF128::mul_x`.
    #[inline]
    #[allow(clippy::arithmetic_side_effects)] // wrapping shifts / XOR on packed words
    pub fn mul_x(&self) -> Self {
        let w = self.uint.as_words();
        let hi_shifted = (w[1] << 1) | (w[0] >> 63);
        let carry = hi_shifted >> 63; // coefficient of X^127 after the shift (0 or 1)
        // carry·0x3 reduces the X^127 term: X^127 ≡ X + 1.
        let lo = (w[0] << 1) ^ carry.wrapping_mul(REDUCTION_LOW_B127);
        Self::from_words([lo, hi_shifted & MASK_HI_B127])
    }

    /// `a²` via carryless square + trinomial reduction.
    ///
    /// In characteristic 2 the cross terms of `(a_0 + X^{64}a_1)^2`
    /// cancel, so the square is TWO 64×64 carryless squares plus the
    /// two-XOR fold — no Karatsuba mid, and (unlike GHASH) no reduction
    /// PMULLs: 2 PMULL total vs GHASH's 5. Squaring chains (the α-power
    /// place-value tables, `FixedBasePow` window advances, the inverse
    /// ladder) are where the trinomial wins most.
    #[inline]
    pub fn square(&self) -> Self {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            Self::from_words(neon::square_words(self.uint.as_words()))
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            let w = self.uint.as_words();
            let lo = clmul_64x64(w[0], w[0]);
            let hi = clmul_64x64(w[1], w[1]);
            Self::from_words(reduce_256_to_127([lo[0], lo[1], hi[0], hi[1]]))
        }
    }

    /// Unreduced 256-bit carryless product `a · b` (no reduction step).
    /// Feed [`Self::reduce_wide`]; XOR of wide values is exact addition.
    /// For canonical operands `deg ≤ 252`, so wide sums stay inside the
    /// single-fold reduction contract.
    #[inline]
    pub fn mul_unreduced(&self, rhs: &Self) -> [u64; 4] {
        clmul_128x128(self.uint.as_words(), rhs.uint.as_words())
    }

    /// Reduce a 256-bit carryless accumulator to the field. Reduction is
    /// `F_2`-linear: `reduce(Σ wide_i) = Σ reduce(wide_i)` exactly.
    #[inline]
    pub fn reduce_wide(w: [u64; 4]) -> Self {
        Self::from_words(reduce_256_to_127(w))
    }

    /// `a^{2^k}` — `k` successive squarings, register-resident on NEON:
    /// one load, one store, the whole run in vector registers (each
    /// [`Self::square`] call would bounce through the `Uint` storage).
    /// The building block of the Itoh–Tsujii ladder, whose cost is
    /// dominated by exactly such squaring runs — and b127 squarings are
    /// the field's cheapest op (2 PMULL + the mask-free fold).
    #[inline]
    pub fn square_n(&self, k: usize) -> Self {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            Self::from_words(neon::square_n_words(self.uint.as_words(), k))
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            let mut v = *self;
            for _ in 0..k {
                v = v.square();
            }
            v
        }
    }

    /// `a^{-1}` via Fermat: `a^{-1} = a^{2^127 - 2}` for `a ≠ 0`.
    ///
    /// `2^127 - 2 = 2 · (2^126 - 1)`: compute `T(126) = a^{2^126 - 1}`
    /// by the **Itoh–Tsujii addition chain** on all-ones exponents —
    /// `T(m+n) = T(m)^{2^n} · T(n)` — along
    /// `1 → 2 → 3 → 6 → 12 → 24 → 48 → 96 → 120 → 126`, then square
    /// once. Total: 126 squarings + **9** multiplications (the naive
    /// all-ones ladder costs the same squarings + 125 mults). The
    /// squaring runs ride the register-resident [`Self::square_n`].
    ///
    /// Panics if `self.is_zero()` — `0` has no multiplicative inverse.
    pub fn inverse(&self) -> Self {
        assert!(!self.is_zero(), "GF(2^127): zero has no inverse");
        let a = *self;
        let t2 = a.square_n(1) * a; // a^{2^2 - 1}
        let t3 = t2.square_n(1) * a; // a^{2^3 - 1}
        let t6 = t3.square_n(3) * t3; // a^{2^6 - 1}
        let t12 = t6.square_n(6) * t6;
        let t24 = t12.square_n(12) * t12;
        let t48 = t24.square_n(24) * t24;
        let t96 = t48.square_n(48) * t48;
        let t120 = t96.square_n(24) * t24;
        let t126 = t120.square_n(6) * t6;
        // T(126) = a^{2^126 - 1}; one squaring → a^{2·(2^126-1)} = a^{2^127-2}.
        t126.square()
    }

    /// Reduced multiply via the 3-PMULL Karatsuba product — the
    /// measurement alternative to the schoolbook default behind `Mul`
    /// (with the PMULL-free b127 reduction the PMULL ports are less
    /// contended, so Karatsuba's PMULL-for-XOR trade may land differently
    /// than it does for GHASH). Value-identical to `self * rhs`; the field
    /// bench compares the two.
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[inline]
    pub fn mul_karatsuba(&self, rhs: &Self) -> Self {
        Self::from_words(neon::mul_words_kara(
            self.uint.as_words(),
            rhs.uint.as_words(),
        ))
    }

    /// Reduced multiply via the GHASH-SHAPED reduction — the measurement
    /// complement to [`Self::mul_karatsuba`]: keep the schoolbook product
    /// but buy the reduction ON the PMULL ports. `X^128 ≡ X² + X = 0x6`,
    /// so the high half folds with 3 PMULLs against `0x6`, instruction-
    /// for-instruction GHASH's `0x87` fold (lane-aligned, no cross-lane
    /// bit-127 extraction) — plus the bit-127 canonicalization
    /// (`X^127 ≡ X + 1`) that a degree-128 modulus never owes. The
    /// sequence is therefore GHASH's + the canonicalization tax: parity
    /// with GF128 is its structural ceiling, and the field bench row
    /// measures the tax. Value-identical to `self * rhs`.
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[inline]
    pub fn mul_pfold(&self, rhs: &Self) -> Self {
        Self::from_words(neon::mul_words_pfold(
            self.uint.as_words(),
            rhs.uint.as_words(),
        ))
    }

    /// `self^exp` via binary square-and-multiply. `exp` is the natural-
    /// number exponent in `[0, 2^32)`.
    pub fn pow_u32(&self, mut exp: u32) -> Self {
        if exp == 0 {
            return Self::one();
        }
        let mut acc = Self::one();
        let mut base = *self;
        while exp > 0 {
            if exp & 1 == 1 {
                acc *= &base;
            }
            exp >>= 1;
            if exp > 0 {
                base = base.square();
            }
        }
        acc
    }
}

impl Zero for BinaryFieldB127 {
    fn zero() -> Self {
        Self::zero()
    }
    fn is_zero(&self) -> bool {
        Self::is_zero(self)
    }
}

impl One for BinaryFieldB127 {
    fn one() -> Self {
        Self::one()
    }
    fn is_one(&self) -> bool {
        let w = self.uint.as_words();
        w[0] == 1 && w[1] == 0
    }
}

impl Display for BinaryFieldB127 {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let w = self.uint.as_words();
        write!(f, "B127[{:016x}_{:016x}]", w[1], w[0])
    }
}

// -- additive group (XOR) --------------------------------------------

impl<'a> AddAssign<&'a Self> for BinaryFieldB127 {
    #[inline]
    #[allow(clippy::arithmetic_side_effects, clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: &'a Self) {
        let lw = *self.uint.as_words();
        let rw = rhs.uint.as_words();
        self.uint = Uint::<2>::from_words([lw[0] ^ rw[0], lw[1] ^ rw[1]]);
    }
}

impl AddAssign<Self> for BinaryFieldB127 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        <Self as AddAssign<&Self>>::add_assign(self, &rhs);
    }
}

impl<'a> Add<&'a Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn add(mut self, rhs: &'a Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl Add<Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self += &rhs;
        self
    }
}

// In characteristic 2, subtraction == addition (XOR).
impl<'a> SubAssign<&'a Self> for BinaryFieldB127 {
    #[inline]
    fn sub_assign(&mut self, rhs: &'a Self) {
        <Self as AddAssign<&Self>>::add_assign(self, rhs);
    }
}
impl SubAssign<Self> for BinaryFieldB127 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self += rhs;
    }
}
impl<'a> Sub<&'a Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn sub(mut self, rhs: &'a Self) -> Self::Output {
        self -= rhs;
        self
    }
}
impl Sub<Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= &rhs;
        self
    }
}

impl Neg for BinaryFieldB127 {
    type Output = Self;
    /// In characteristic 2, every element is its own additive inverse.
    #[inline]
    fn neg(self) -> Self::Output {
        self
    }
}

// -- multiplicative group --------------------------------------------

/// Reduced product on word pairs — the NEON-resident schoolbook+trinomial
/// pipeline on aarch64 (no NEON↔GPR bounces on the multiply's critical
/// path), the scalar Karatsuba + two-XOR fold elsewhere. Bit-identical
/// results (pinned by `neon_mul_matches_scalar_pipeline`).
#[inline]
fn mul_words_b127(a: &[u64; 2], b: &[u64; 2]) -> [u64; 2] {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        neon::mul_words(a, b)
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        reduce_256_to_127(clmul_128x128(a, b))
    }
}

impl<'a> MulAssign<&'a Self> for BinaryFieldB127 {
    #[inline]
    fn mul_assign(&mut self, rhs: &'a Self) {
        self.uint =
            Uint::<2>::from_words(mul_words_b127(self.uint.as_words(), rhs.uint.as_words()));
    }
}
impl MulAssign<Self> for BinaryFieldB127 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        <Self as MulAssign<&Self>>::mul_assign(self, &rhs);
    }
}
impl<'a> Mul<&'a Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn mul(mut self, rhs: &'a Self) -> Self::Output {
        self *= rhs;
        self
    }
}
impl Mul<Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn mul(mut self, rhs: Self) -> Self::Output {
        self *= &rhs;
        self
    }
}

// -- division (multiply by inverse) ----------------------------------

impl<'a> DivAssign<&'a Self> for BinaryFieldB127 {
    /// `self /= rhs` ≡ `self *= rhs.inverse()`. Panics if `rhs` is zero.
    #[inline]
    fn div_assign(&mut self, rhs: &'a Self) {
        *self *= rhs.inverse();
    }
}
impl DivAssign<Self> for BinaryFieldB127 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self /= &rhs;
    }
}
impl<'a> Div<&'a Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: &'a Self) -> Self::Output {
        self /= rhs;
        self
    }
}
impl Div<Self> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: Self) -> Self::Output {
        self /= &rhs;
        self
    }
}

// -- num_traits checked-* (degenerate in a field — never fail) -------

impl CheckedAdd for BinaryFieldB127 {
    #[inline]
    fn checked_add(&self, rhs: &Self) -> Option<Self> {
        Some(*self + rhs)
    }
}
impl CheckedSub for BinaryFieldB127 {
    #[inline]
    fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        Some(*self - rhs)
    }
}
impl CheckedMul for BinaryFieldB127 {
    #[inline]
    fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        Some(*self * rhs)
    }
}
impl CheckedNeg for BinaryFieldB127 {
    #[inline]
    fn checked_neg(&self) -> Option<Self> {
        Some(-*self)
    }
}

// -- Sum / Product folds ---------------------------------------------

impl Sum<Self> for BinaryFieldB127 {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
    }
}
impl<'a> Sum<&'a Self> for BinaryFieldB127 {
    #[inline]
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + *x)
    }
}
impl Product<Self> for BinaryFieldB127 {
    #[inline]
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::one(), |acc, x| acc * x)
    }
}
impl<'a> Product<&'a Self> for BinaryFieldB127 {
    #[inline]
    fn product<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::one(), |acc, x| acc * *x)
    }
}

// -- Inv ------------------------------------------------------------

impl Inv for BinaryFieldB127 {
    type Output = Option<Self>;
    #[inline]
    fn inv(self) -> Self::Output {
        if self.is_zero() {
            None
        } else {
            Some(self.inverse())
        }
    }
}

// -- Pow<u32> -------------------------------------------------------

impl Pow<u32> for BinaryFieldB127 {
    type Output = Self;
    #[inline]
    fn pow(self, exp: u32) -> Self::Output {
        self.pow_u32(exp)
    }
}

// -- ConstZero / ConstOne / From<bool> ------------------------------

impl ConstZero for BinaryFieldB127 {
    const ZERO: Self = Self::zero();
}

impl ConstOne for BinaryFieldB127 {
    const ONE: Self = Self::one();
}

impl From<bool> for BinaryFieldB127 {
    #[inline]
    fn from(b: bool) -> Self {
        if b { Self::one() } else { Self::zero() }
    }
}

// -- From<{u8..u128, i8..i128}> --------------------------------------
//
// Semantics: interpret the primitive's bit pattern as the low
// coefficients of a GF(2^127) element, folding the (single possible)
// out-of-range bit: `X^127 ≡ X + 1`. The transcript only uses this map
// for hash absorption / challenge derivation, so any deterministic map
// works; it just needs to be the same on prover and verifier — and to
// land INSIDE the canonical representation (bit 127 clear).

macro_rules! impl_from_unsigned_b127 {
    ($($t:ty),*) => {
        $(
            impl From<$t> for BinaryFieldB127 {
                #[inline]
                fn from(v: $t) -> Self {
                    Self::from_words([v as u64, 0])
                }
            }
        )*
    };
}

impl_from_unsigned_b127!(u8, u16, u32, u64);

impl From<u128> for BinaryFieldB127 {
    #[inline]
    fn from(v: u128) -> Self {
        // Fold the X^127 term: X^127 ≡ X + 1 (mod f). 2-to-1 on the
        // 128-bit patterns, so uniform u128s map to uniform elements.
        let carry = (v >> 127) as u64;
        Self::from_words([
            (v as u64) ^ carry.wrapping_mul(REDUCTION_LOW_B127),
            ((v >> 64) as u64) & MASK_HI_B127,
        ])
    }
}

macro_rules! impl_from_signed_b127 {
    ($($t:ty),*) => {
        $(
            impl From<$t> for BinaryFieldB127 {
                #[inline]
                fn from(v: $t) -> Self {
                    // Two's-complement bit pattern, sign-extended to u128
                    // (then bit-127-folded by From<u128>).
                    Self::from(v as i128 as u128)
                }
            }
        )*
    };
}

impl_from_signed_b127!(i8, i16, i32, i64);

impl From<i128> for BinaryFieldB127 {
    #[inline]
    fn from(v: i128) -> Self {
        Self::from(v as u128)
    }
}

// -- Semiring / Ring marker traits ----------------------------------

impl Semiring for BinaryFieldB127 {}
impl Ring for BinaryFieldB127 {}

// -- Field ----------------------------------------------------------

impl Field for BinaryFieldB127 {
    /// Bit-packed `Uint<2>` (bit 127 zero in canonical elements). `Uint<2>`
    /// rather than `[u64; 2]` so `F::Inner: ConstTranscribable` is
    /// satisfied directly (the Fiat–Shamir challenge representation).
    type Inner = Uint<2>;
    /// The full reduction polynomial `f(X) = X^127 + X + 1` — degree
    /// `< 128`, so (unlike GHASH's) it is stored in full: bit 127 + `0x3`
    /// ([`B127_MODULUS`]).
    type Modulus = Uint<2>;

    #[inline(always)]
    fn inner(&self) -> &Self::Inner {
        &self.uint
    }

    #[inline(always)]
    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.uint
    }

    #[inline(always)]
    fn into_inner(self) -> Self::Inner {
        self.uint
    }
}

// -- PrimeField (degenerate impl) -----------------------------------
//
// `BinaryFieldB127` is a binary extension field, NOT a prime field; the
// degenerate impl exists for the same reason as `BinaryFieldGF128`'s —
// `PrimeField` is the central bound throughout `piop/`, and this is the
// least-invasive way to slot into that machinery. See the GF128 module
// for the full rationale. The ONE substantive difference: `new_with_cfg`
// REDUCES (folds bit 127), because a raw 128-bit transcript draw is not
// in general a canonical GF(2^127) pattern.

impl PrimeField for BinaryFieldB127 {
    type Config = ();

    #[inline(always)]
    fn cfg(&self) -> &Self::Config {
        &()
    }

    #[inline(always)]
    fn is_zero(value: &Self) -> bool {
        Self::is_zero(value)
    }

    #[inline(always)]
    fn modulus(&self) -> Self::Modulus {
        B127_MODULUS
    }

    fn modulus_minus_one_div_two(&self) -> Self::Inner {
        panic!(
            "BinaryFieldB127::modulus_minus_one_div_two: GF(2^127) has no \
             prime modulus; this method is degenerate. Audit the call site \
             before using it with a binary extension field."
        );
    }

    fn make_cfg(modulus: &Self::Modulus) -> Result<Self::Config, FieldError> {
        if modulus == &B127_MODULUS {
            Ok(())
        } else {
            Err(FieldError::InvalidModulus)
        }
    }

    /// Canonicalizing constructor: folds bit 127 (`X^127 ≡ X + 1`). This
    /// is the transcript-challenge entry point (`get_field_challenge`
    /// draws a raw 128-bit `Uint<2>` and hands it here); the fold is a
    /// 2-to-1 covering of the field, so uniform draws stay uniform.
    #[inline(always)]
    fn new_with_cfg(inner: Self::Inner, _cfg: &Self::Config) -> Self {
        let w = inner.as_words();
        let carry = w[1] >> 63;
        Self::from_words([
            w[0] ^ carry.wrapping_mul(REDUCTION_LOW_B127),
            w[1] & MASK_HI_B127,
        ])
    }

    /// Trusting constructor: the caller guarantees the canonical
    /// invariant (debug-asserted).
    #[inline(always)]
    fn new_unchecked_with_cfg(inner: Self::Inner, _cfg: &Self::Config) -> Self {
        debug_assert!(
            inner.as_words()[1] >> 63 == 0,
            "GF(2^127): non-canonical inner (bit 127 set)"
        );
        Self { uint: inner }
    }

    #[inline(always)]
    fn zero_with_cfg(_cfg: &Self::Config) -> Self {
        Self::zero()
    }

    #[inline(always)]
    fn one_with_cfg(_cfg: &Self::Config) -> Self {
        Self::one()
    }
}

// -- InnerTransparentField -------------------------------------------

impl InnerTransparentField for BinaryFieldB127 {
    #[inline]
    fn add_inner(
        lhs: &Self::Inner,
        rhs: &Self::Inner,
        _config: &Self::Config,
    ) -> Self::Inner {
        let lw = lhs.as_words();
        let rw = rhs.as_words();
        Uint::<2>::from_words([lw[0] ^ rw[0], lw[1] ^ rw[1]])
    }

    #[inline]
    fn sub_inner(
        lhs: &Self::Inner,
        rhs: &Self::Inner,
        config: &Self::Config,
    ) -> Self::Inner {
        // Characteristic 2: subtraction is XOR, same as addition.
        Self::add_inner(lhs, rhs, config)
    }

    #[inline]
    fn mul_assign_by_inner(&mut self, rhs: &Self::Inner) {
        // The inner representation is the field element (canonical by the
        // producers of `Inner` values in this crate); lift and reuse the
        // regular `MulAssign<&Self>`.
        debug_assert!(
            rhs.as_words()[1] >> 63 == 0,
            "GF(2^127): non-canonical inner (bit 127 set)"
        );
        let r = Self { uint: *rhs };
        *self *= &r;
    }

    #[inline]
    fn mul_by_node2(&self, node2: &Self) -> Self {
        // from(2) = X here, so the node-2 multiply is the cheap shift+fold.
        debug_assert_eq!(
            node2,
            &Self::from_words([2, 0]),
            "node2 must be the field generator X = from(2)"
        );
        self.mul_x()
    }
}

/// Unreduced 256-bit accumulator held as two NEON vectors `(lo, hi)` —
/// register-resident through the generic driver loops, exactly as
/// [`WideGf128`][crate::poly::univariate::binary_gf128::WideGf128]. The
/// products are the same carryless 256-bit ones; only the drain
/// (`from_wide`) differs — b127's two-XOR trinomial fold instead of the
/// 3-PMULL GHASH fold.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[derive(Clone, Copy)]
pub struct WideB127(
    core::arch::aarch64::uint64x2_t,
    core::arch::aarch64::uint64x2_t,
);

/// Delayed-reduction accumulate: 256-bit unreduced carryless products,
/// XOR-combined, reduced once per accumulator (see `crate::utils::wide_mul`).
/// All trait-reachable wide values are XORs of products of canonical
/// (`deg ≤ 126`) operands, hence `deg ≤ 252` — inside the single-fold
/// reduction contract of [`reduce_256_to_127`].
impl crate::utils::wide_mul::WideMulAcc for BinaryFieldB127 {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    type Wide = WideB127;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    type Wide = [u64; 4];

    #[inline(always)]
    fn wide_zero(_zero: &Self) -> Self::Wide {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            // SAFETY: plain NEON constants (see `binary_gf128::neon::pmull_lo`).
            unsafe {
                use core::arch::aarch64::vdupq_n_u64;
                WideB127(vdupq_n_u64(0), vdupq_n_u64(0))
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            [0; 4]
        }
    }

    #[inline(always)]
    fn wide_of(x: &Self) -> Self::Wide {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            // SAFETY: as `wide_zero`.
            unsafe {
                use core::arch::aarch64::vdupq_n_u64;
                WideB127(neon::ld(x), vdupq_n_u64(0))
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            let w = x.uint.as_words();
            [w[0], w[1], 0, 0]
        }
    }

    #[inline(always)]
    fn mul_wide(a: &Self, b: &Self) -> Self::Wide {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            // SAFETY: as `wide_zero`. The product is modulus-agnostic —
            // shared with the GF128 pipeline.
            unsafe {
                let (lo, hi) = gf128_neon::clmul_256(neon::ld(a), neon::ld(b));
                WideB127(lo, hi)
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            a.mul_unreduced(b)
        }
    }

    #[inline(always)]
    fn wide_add_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            // SAFETY: plain NEON XORs.
            unsafe {
                use core::arch::aarch64::veorq_u64;
                acc.0 = veorq_u64(acc.0, x.0);
                acc.1 = veorq_u64(acc.1, x.1);
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            acc[0] ^= x[0];
            acc[1] ^= x[1];
            acc[2] ^= x[2];
            acc[3] ^= x[3];
        }
    }

    #[inline(always)]
    fn wide_sub_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        // char 2: subtraction = addition = XOR.
        Self::wide_add_assign(acc, x);
    }

    #[inline(always)]
    fn from_wide(w: Self::Wide) -> Self {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            // SAFETY: as `wide_zero`.
            unsafe {
                use core::arch::aarch64::vst1q_u64;
                let r = neon::reduce_256_b127(w.0, w.1);
                let mut out = [0u64; 2];
                vst1q_u64(out.as_mut_ptr(), r);
                Self::from_words(out)
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
            Self::reduce_wide(w)
        }
    }

    #[inline(always)]
    fn add_assign_masked(acc: &mut Self, x: &Self, mask: bool) {
        // Branchless select: XOR in `x` under an all-ones/all-zeros mask —
        // add in char 2, immune to the coin-flip mispredicts a data-bit
        // branch would cost.
        let m = (mask as u64).wrapping_neg();
        let a = acc.uint.as_words();
        let xw = x.uint.as_words();
        *acc = Self::from_words([a[0] ^ (m & xw[0]), a[1] ^ (m & xw[1])]);
    }

    /// Hand-fused round body — the GF128 kernel with the trinomial drain:
    /// two independent slot chains, register-resident 256-bit accumulators,
    /// one (cheap, PMULL-free) reduction per accumulator at the end.
    /// Value-exact vs the generic loop (same carryless products,
    /// XOR-combined; reduction is `F_2`-linear).
    #[allow(clippy::arithmetic_side_effects)]
    fn eqf_single_pair_round(
        l: &[Self],
        r: &[Self],
        w: &[Self],
        half: usize,
    ) -> Option<(Self, Self, Self)> {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            return Some(neon::eqf_single_pair_round(l, r, w, half));
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
        #[inline(always)]
        fn slot(
            w: &BinaryFieldB127,
            l0: &BinaryFieldB127,
            l1: &BinaryFieldB127,
            r0: &BinaryFieldB127,
            r1: &BinaryFieldB127,
            a0: &mut [u64; 4],
            a1: &mut [u64; 4],
            a2: &mut [u64; 4],
        ) {
            let ww = w.uint.as_words();
            let l0w = reduce_256_to_127(clmul_128x128(ww, l0.uint.as_words()));
            let l1w = reduce_256_to_127(clmul_128x128(ww, l1.uint.as_words()));
            let r0w = r0.uint.as_words();
            let r1w = r1.uint.as_words();
            let wc0 = clmul_128x128(&l0w, r0w);
            let w11 = clmul_128x128(&l1w, r1w);
            let dl = [l1w[0] ^ l0w[0], l1w[1] ^ l0w[1]];
            let dr = [r1w[0] ^ r0w[0], r1w[1] ^ r0w[1]];
            let wc2 = clmul_128x128(&dl, &dr);
            let mut i = 0;
            while i < 4 {
                a0[i] ^= wc0[i];
                a2[i] ^= wc2[i];
                a1[i] ^= w11[i] ^ wc0[i] ^ wc2[i];
                i += 1;
            }
        }
        let (mut a0a, mut a1a, mut a2a) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        let (mut a0b, mut a1b, mut a2b) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        let mut b = 0usize;
        while b + 2 <= half {
            slot(&w[b], &l[b << 1], &l[(b << 1) | 1], &r[b << 1], &r[(b << 1) | 1], &mut a0a, &mut a1a, &mut a2a);
            let c = b + 1;
            slot(&w[c], &l[c << 1], &l[(c << 1) | 1], &r[c << 1], &r[(c << 1) | 1], &mut a0b, &mut a1b, &mut a2b);
            b += 2;
        }
        if b < half {
            slot(&w[b], &l[b << 1], &l[(b << 1) | 1], &r[b << 1], &r[(b << 1) | 1], &mut a0a, &mut a1a, &mut a2a);
        }
        let mut i = 0;
        while i < 4 {
            a0a[i] ^= a0b[i];
            a1a[i] ^= a1b[i];
            a2a[i] ^= a2b[i];
            i += 1;
        }
        Some((Self::reduce_wide(a0a), Self::reduce_wide(a1a), Self::reduce_wide(a2a)))
        }
    }

    /// Hand-fused TWO-pair round body (the fraction-GKR layer combine) —
    /// the GF128 kernel with the trinomial drain. Value-exact.
    #[allow(clippy::arithmetic_side_effects)]
    fn eqf_two_pair_round(
        l0: &[Self],
        r0: &[Self],
        l1: &[Self],
        r1: &[Self],
        w: &[Self],
        half: usize,
    ) -> Option<(Self, Self, Self)> {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            return Some(neon::eqf_two_pair_round(l0, r0, l1, r1, w, half));
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
        #[inline(always)]
        fn slot(
            w: &BinaryFieldB127,
            l0: &BinaryFieldB127,
            l1: &BinaryFieldB127,
            r0: &BinaryFieldB127,
            r1: &BinaryFieldB127,
            a0: &mut [u64; 4],
            a1: &mut [u64; 4],
            a2: &mut [u64; 4],
        ) {
            let ww = w.uint.as_words();
            let l0w = reduce_256_to_127(clmul_128x128(ww, l0.uint.as_words()));
            let l1w = reduce_256_to_127(clmul_128x128(ww, l1.uint.as_words()));
            let r0w = r0.uint.as_words();
            let r1w = r1.uint.as_words();
            let wc0 = clmul_128x128(&l0w, r0w);
            let w11 = clmul_128x128(&l1w, r1w);
            let dl = [l1w[0] ^ l0w[0], l1w[1] ^ l0w[1]];
            let dr = [r1w[0] ^ r0w[0], r1w[1] ^ r0w[1]];
            let wc2 = clmul_128x128(&dl, &dr);
            let mut i = 0;
            while i < 4 {
                a0[i] ^= wc0[i];
                a2[i] ^= wc2[i];
                a1[i] ^= w11[i] ^ wc0[i] ^ wc2[i];
                i += 1;
            }
        }
        let (mut a0a, mut a1a, mut a2a) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        let (mut a0b, mut a1b, mut a2b) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        let mut b = 0usize;
        while b < half {
            let e = b << 1;
            slot(&w[b], &l0[e], &l0[e | 1], &r0[e], &r0[e | 1], &mut a0a, &mut a1a, &mut a2a);
            slot(&w[b], &l1[e], &l1[e | 1], &r1[e], &r1[e | 1], &mut a0b, &mut a1b, &mut a2b);
            b += 1;
        }
        let mut i = 0;
        while i < 4 {
            a0a[i] ^= a0b[i];
            a1a[i] ^= a1b[i];
            a2a[i] ^= a2b[i];
            i += 1;
        }
        Some((Self::reduce_wide(a0a), Self::reduce_wide(a1a), Self::reduce_wide(a2a)))
        }
    }

    /// Fused in-place fold `v[b] ← v[2b] ⊕ ρ·(v[2b+1] ⊕ v[2b])`, two
    /// independent entries per iteration. Value-exact per entry.
    #[allow(clippy::arithmetic_side_effects)]
    fn eqf_fold_in_place(v: &mut [Self], rho: &Self, half: usize) -> bool {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            neon::eqf_fold_in_place(v, rho, half);
            return true;
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        {
        let rw = *rho.uint.as_words();
        let mut b = 0usize;
        while b + 2 <= half {
            let v0a = *v[b << 1].uint.as_words();
            let v1a = *v[(b << 1) | 1].uint.as_words();
            let v0b = *v[(b + 1) << 1].uint.as_words();
            let v1b = *v[((b + 1) << 1) | 1].uint.as_words();
            let da = [v1a[0] ^ v0a[0], v1a[1] ^ v0a[1]];
            let db = [v1b[0] ^ v0b[0], v1b[1] ^ v0b[1]];
            let pa = reduce_256_to_127(clmul_128x128(&rw, &da));
            let pb = reduce_256_to_127(clmul_128x128(&rw, &db));
            v[b] = Self::from_words([v0a[0] ^ pa[0], v0a[1] ^ pa[1]]);
            v[b + 1] = Self::from_words([v0b[0] ^ pb[0], v0b[1] ^ pb[1]]);
            b += 2;
        }
        if b < half {
            let v0 = *v[b << 1].uint.as_words();
            let v1 = *v[(b << 1) | 1].uint.as_words();
            let d = [v1[0] ^ v0[0], v1[1] ^ v0[1]];
            let p = reduce_256_to_127(clmul_128x128(&rw, &d));
            v[b] = Self::from_words([v0[0] ^ p[0], v0[1] ^ p[1]]);
        }
        true
        }
    }
}

// -- trinomial reduction ----------------------------------------------
//
// Write the 256-bit input as `P = L + X^127·H` (`L` = bits 0..126, `H` =
// bits 127..). Then `P ≡ L ⊕ H ⊕ (H << 1) (mod X^127 + X + 1)`. For every
// value reachable through this module — products of canonical (`deg ≤ 126`)
// operands and XOR-sums thereof — `deg P ≤ 252`, so `deg H ≤ 125`,
// `deg((X+1)H) ≤ 126 < 127`, and ONE fold is exact. The scalar reducer
// below nevertheless handles the fully general 256-bit case (a second
// ≤4-bit fold), so it doubles as the reference for arbitrary test inputs;
// the NEON reducer keeps the single-fold contract (invariant-guaranteed on
// its call sites).

/// Reduce a 256-bit `F_2[X]` polynomial modulo `f(X) = X^127 + X + 1`.
///
/// Fully general (any 256-bit input): first fold `H = P >> 127` (≤ 129
/// bits) as `L ⊕ H ⊕ (H << 1)`, then re-fold the ≤ 4 bits that land at or
/// above `X^127`. On the module's own inputs the second fold is zero.
#[inline]
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn reduce_256_to_127(prod: [u64; 4]) -> [u64; 2] {
    // H = P >> 127: h = [h0, h1, h2], h2 ∈ {0, 1}.
    let h0 = (prod[1] >> 63) | (prod[2] << 1);
    let h1 = (prod[2] >> 63) | (prod[3] << 1);
    let h2 = prod[3] >> 63;
    // (X + 1)·H = H ⊕ (H << 1), spilled words g = [g0, g1, g2].
    let g0 = h0 ^ (h0 << 1);
    let g1 = h1 ^ (h1 << 1) ^ (h0 >> 63);
    let g2 = h2 ^ (h2 << 1) ^ (h1 >> 63); // bits 128.. of (X+1)·H, ≤ 3 bits
    let t0 = prod[0] ^ g0;
    let t1 = (prod[1] & MASK_HI_B127) ^ g1;
    // Second fold: U = bits ≥ 127 of the partial result (≤ 4 bits);
    // (X + 1)·U lands entirely in word 0.
    let u = (t1 >> 63) | (g2 << 1);
    [t0 ^ u ^ (u << 1), t1 & MASK_HI_B127]
}

// -- NEON-resident multiply pipeline (aarch64) ------------------------
//
// The products (schoolbook 4-PMULL 128×128, and the 2-PMULL char-2
// square) are exactly the GF128 pipeline's — carryless products are
// modulus-agnostic, so `clmul_256` is imported from
// `binary_gf128::neon`. What changes is the reduction: the 3-PMULL GHASH
// fold is replaced by a shift/XOR trinomial fold that issues ZERO PMULLs
// (~12 cheap logical/shift ops, off the PMULL ports) — a reduced multiply
// is 4 PMULL instead of 7, a square 2 instead of 5. A 3-PMULL Karatsuba
// product variant is provided for measurement ([`mul_words_kara`]); the
// default stays schoolbook, matching the GF128 pipeline's measured choice
// on Apple silicon.
//
// Value-exact: the same carryless products and the same modular
// reduction, so callers see bit-identical field elements to the scalar
// pipeline (pinned by `neon_mul_matches_scalar_pipeline`).
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use crate::poly::univariate::binary_gf128::neon as gf128_neon;

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon {
    use core::arch::aarch64::{
        uint64x2_t, vdupq_n_u64, veorq_u64, vextq_u64, vld1q_u64, vshlq_n_u64, vshrq_n_u64,
        vsriq_n_u64, vst1q_u64,
    };
    #[cfg(not(target_feature = "sha3"))]
    use core::arch::aarch64::vandq_u64;
    #[cfg(target_feature = "sha3")]
    use core::arch::aarch64::{vbcaxq_u64, veor3q_u64};

    use super::MASK_HI_B127;
    use super::gf128_neon::{clmul_256, pmull_hi, pmull_lo};

    /// `lo` mask complement: bit 127 — the one bit of `lo` that belongs
    /// to `H` (as its bit 0), not to `L`.
    const TOP_BIT: [u64; 2] = [0, !MASK_HI_B127];
    /// `g` spurious bit: `(P >> 126)`'s bit 0 is `P`'s bit 126, which
    /// belongs to `L`, not to `H << 1`.
    const G0_BIT: [u64; 2] = [1, 0];
    /// `lo` mask: keep bits 0..126 (`L = P & (2^127 − 1)`) — the
    /// non-SHA3 fallback's AND constant.
    #[cfg(not(target_feature = "sha3"))]
    const MASK127: [u64; 2] = [u64::MAX, MASK_HI_B127];
    /// `g` mask (non-SHA3 fallback): clear lane-0 bit 0.
    #[cfg(not(target_feature = "sha3"))]
    const CLEAR_G0: [u64; 2] = [u64::MAX << 1, u64::MAX];

    /// Reduce a 256-bit product `(lo, hi)` modulo `X^127 + X + 1` — the
    /// PMULL-free trinomial fold.
    ///
    /// `H = P >> 127` (`deg ≤ 125` under the module contract: `P` is an
    /// XOR of products of canonical operands), result
    /// `= (P & (2^127−1)) ⊕ H ⊕ (H << 1)`, one fold, exact. Both shifted
    /// views come straight off the product limbs with SRI (shift-right-
    /// insert) — `H` limb-wise is `(hi << 1) ⊕ (pre >> 63)` and
    /// `H << 1 = P >> 126` (bit 0 cleared) is `(hi << 2) ⊕ (pre >> 62)`,
    /// where `pre = [p1, p2]` are the limbs the `X^127`/`X^191` cuts
    /// straddle. With FEAT_SHA3 (Apple M-series; enabled by
    /// `-C target-cpu=native`) both corrective masks fuse into BCAX
    /// (`a ⊕ (b & ~c)`): 7 µops, ~8-cycle chain, no PMULL. The fallback
    /// keeps the AND/EOR form (9 µops, balanced XOR tree).
    #[inline(always)]
    pub(crate) unsafe fn reduce_256_b127(lo: uint64x2_t, hi: uint64x2_t) -> uint64x2_t {
        // SAFETY: plain NEON shifts/XORs (see `binary_gf128::neon::pmull_lo`
        // for the module's target-feature contract).
        unsafe {
            let pre = vextq_u64::<1>(lo, hi); // [p1, p2]
            // H = P >> 127, limb-wise: h0 = (p2<<1)|(p1>>63), h1 = (p3<<1)|(p2>>63).
            let h = vsriq_n_u64::<63>(vshlq_n_u64::<1>(hi), pre);
            // (P >> 126): g0 = (p2<<2)|(p1>>62), g1 = (p3<<2)|(p2>>62);
            // its bit 0 (P's bit 126) is spurious — H<<1 has bit 0 zero.
            let g = vsriq_n_u64::<62>(vshlq_n_u64::<2>(hi), pre);
            #[cfg(target_feature = "sha3")]
            {
                // t = h ⊕ (g & ~bit0); out = t ⊕ (lo & ~bit127).
                let t = vbcaxq_u64(h, g, vld1q_u64(G0_BIT.as_ptr()));
                vbcaxq_u64(t, lo, vld1q_u64(TOP_BIT.as_ptr()))
            }
            #[cfg(not(target_feature = "sha3"))]
            {
                let g = vandq_u64(g, vld1q_u64(CLEAR_G0.as_ptr()));
                let l = vandq_u64(lo, vld1q_u64(MASK127.as_ptr()));
                veorq_u64(veorq_u64(l, h), g)
            }
        }
    }

    /// Fully NEON-resident reduced multiply on word pairs: the shared
    /// schoolbook 4-PMULL product + the PMULL-free trinomial fold.
    #[inline(always)]
    pub(crate) fn mul_words(a: &[u64; 2], b: &[u64; 2]) -> [u64; 2] {
        // SAFETY: as `reduce_256_b127`; loads/stores are on valid 16-byte
        // word pairs.
        unsafe {
            let va = vld1q_u64(a.as_ptr());
            let vb = vld1q_u64(b.as_ptr());
            let (lo, hi) = clmul_256(va, vb);
            let r = reduce_256_b127(lo, hi);
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            out
        }
    }

    /// 128×128 → 256-bit carryless product, KARATSUBA (3 PMULL + the
    /// mid-recombination XOR/EXTs). Provided for measurement against the
    /// schoolbook default: with the PMULL-free b127 reduction the PMULL
    /// ports are less contended than in the GF128 pipeline, so the
    /// trade-off may land differently — the field bench decides.
    #[inline(always)]
    pub(crate) unsafe fn clmul_256_kara(a: uint64x2_t, b: uint64x2_t) -> (uint64x2_t, uint64x2_t) {
        // SAFETY: as `reduce_256_b127`.
        unsafe {
            let t00 = pmull_lo(a, b); // a0·b0
            let t11 = pmull_hi(a, b); // a1·b1
            let asum = veorq_u64(a, vextq_u64::<1>(a, a)); // [a0^a1, ·]
            let bsum = veorq_u64(b, vextq_u64::<1>(b, b)); // [b0^b1, ·]
            let mid = veorq_u64(pmull_lo(asum, bsum), veorq_u64(t00, t11));
            let z = vdupq_n_u64(0);
            let mid_lo = vextq_u64::<1>(z, mid); // mid << 64
            let mid_hi = vextq_u64::<1>(mid, z); // mid >> 64
            (veorq_u64(t00, mid_lo), veorq_u64(t11, mid_hi))
        }
    }

    /// Reduced multiply via the Karatsuba product (bench alternative;
    /// value-identical to [`mul_words`]).
    #[inline(always)]
    pub(crate) fn mul_words_kara(a: &[u64; 2], b: &[u64; 2]) -> [u64; 2] {
        // SAFETY: as `mul_words`.
        unsafe {
            let va = vld1q_u64(a.as_ptr());
            let vb = vld1q_u64(b.as_ptr());
            let (lo, hi) = clmul_256_kara(va, vb);
            let r = reduce_256_b127(lo, hi);
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            out
        }
    }

    /// Reduce a 256-bit product `(lo, hi)` modulo `X^127 + X + 1` via the
    /// GHASH-SHAPED PMULL fold — the alternative that spends the
    /// reduction on the PMULL ports instead of the shift/logical pipes.
    ///
    /// `X^128 ≡ X² + X = 0x6 (mod f)`, so the high 128 bits fold exactly
    /// as GHASH's do against `0x87`: `hi ⊗ 0x6` via two PMULLs (word
    /// layout `w0 = y0, w1 = y1 ^ z0, w2 = z1 ≤ 2 bits`), then the word-2
    /// spill refolds through a third PMULL. Lane-aligned throughout — the
    /// cross-lane bit-127 extraction of [`reduce_256_b127`] never
    /// happens. What GHASH does NOT owe afterwards is the
    /// canonicalization: the folded 128-bit value may set bit 127, which
    /// `X^127 ≡ X + 1` folds back (SHR + EXT + SHL/EOR + BCAX; the
    /// non-SHA3 fallback spends one more EOR). Structurally this is
    /// GHASH's `reduce_256` + that tax, which is the point of measuring
    /// it: it upper-bounds b127-with-PMULL-reduction at GHASH parity.
    #[inline(always)]
    pub(crate) unsafe fn reduce_256_b127_pfold(lo: uint64x2_t, hi: uint64x2_t) -> uint64x2_t {
        // SAFETY: as `reduce_256_b127`.
        unsafe {
            let g = vdupq_n_u64(0x6);
            let p0 = pmull_lo(hi, g); // (X²+X)·p2 = [y0, y1], y1 ≤ 2 bits
            let p1 = pmull_hi(hi, g); // (X²+X)·p3 = [z0, z1], z1 ≤ 2 bits
            let z = vdupq_n_u64(0);
            // r ← lo ⊕ (hi ⊗ 0x6) words 0–1.
            let r = veorq_u64(lo, veorq_u64(p0, vextq_u64::<1>(z, p1)));
            // word-2 spill (z1): X^128·z1 ≡ 0x6·z1, lands in word 0.
            let r = veorq_u64(r, pmull_lo(vextq_u64::<1>(p1, z), g));
            // Canonicalize bit 127: c = r >> 127; r' = (r \ bit127) ⊕ 3c.
            let c = vextq_u64::<1>(vshrq_n_u64::<63>(r), z); // [c, 0]
            let fold = veorq_u64(vshlq_n_u64::<1>(c), c); // [3c, 0]
            #[cfg(target_feature = "sha3")]
            {
                // fold ⊕ (r & ~bit127) in one BCAX.
                vbcaxq_u64(fold, r, vld1q_u64(TOP_BIT.as_ptr()))
            }
            #[cfg(not(target_feature = "sha3"))]
            {
                veorq_u64(vandq_u64(r, vld1q_u64(MASK127.as_ptr())), fold)
            }
        }
    }

    /// Reduced multiply via the GHASH-shaped PMULL fold (bench
    /// alternative; value-identical to [`mul_words`]).
    #[inline(always)]
    pub(crate) fn mul_words_pfold(a: &[u64; 2], b: &[u64; 2]) -> [u64; 2] {
        // SAFETY: as `mul_words`.
        unsafe {
            let va = vld1q_u64(a.as_ptr());
            let vb = vld1q_u64(b.as_ptr());
            let (lo, hi) = clmul_256(va, vb);
            let r = reduce_256_b127_pfold(lo, hi);
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            out
        }
    }

    /// NEON-resident reduced SQUARE: char-2 cross terms cancel, so two
    /// PMULLs + a square-specialized trinomial fold — 2 PMULL total vs
    /// GHASH's 5.
    ///
    /// The fold exploits the spread structure directly: with
    /// `a = a_0 + X^{64} a_1`, `a² = S(a_0) + X^{128}·S(a_1)`
    /// (`S = ` the even-position bit spread, one PMULL each), and
    /// `X^{128} ≡ X² + X`, so
    /// `a² ≡ S(a_0) ⊕ (S(a_1) << 1) ⊕ (S(a_1) << 2)` — and because the
    /// canonical invariant zeroes `a_1`'s bit 63, `deg S(a_1) ≤ 124` and
    /// `(X²+X)·S(a_1)` never reaches `X^127`: one fold, no masks at all
    /// (6 µops with EOR3, 7 without).
    /// One squaring step on a vector-resident value — the spread + the
    /// square-specialized trinomial fold (see [`square_words`] for the
    /// derivation). Canonical in, canonical out.
    #[inline(always)]
    unsafe fn square_step(va: uint64x2_t) -> uint64x2_t {
        // SAFETY: as `reduce_256_b127`.
        unsafe {
            let s_lo = pmull_lo(va, va); // S(a0), even bits, deg ≤ 126
            let s_hi = pmull_hi(va, va); // S(a1), even bits, deg ≤ 124
            let z = vdupq_n_u64(0);
            let e = vextq_u64::<1>(z, s_hi); // [0, s_hi.0] — the lane carries
            let t1 = vsriq_n_u64::<63>(vshlq_n_u64::<1>(s_hi), e); // S(a1) << 1
            let t2 = vsriq_n_u64::<62>(vshlq_n_u64::<2>(s_hi), e); // S(a1) << 2
            #[cfg(target_feature = "sha3")]
            {
                veor3q_u64(s_lo, t1, t2)
            }
            #[cfg(not(target_feature = "sha3"))]
            {
                veorq_u64(veorq_u64(s_lo, t1), t2)
            }
        }
    }

    #[inline(always)]
    pub(crate) fn square_words(a: &[u64; 2]) -> [u64; 2] {
        // SAFETY: as `mul_words`.
        unsafe {
            let r = square_step(vld1q_u64(a.as_ptr()));
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            out
        }
    }

    /// `n` successive squarings with the value held in a vector register
    /// throughout — one load, one store, no per-step `Uint` bounce. The
    /// Itoh–Tsujii ladder's squaring runs.
    #[inline]
    pub(crate) fn square_n_words(a: &[u64; 2], n: usize) -> [u64; 2] {
        // SAFETY: as `mul_words`.
        unsafe {
            let mut v = vld1q_u64(a.as_ptr());
            for _ in 0..n {
                v = square_step(v);
            }
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), v);
            out
        }
    }

    use super::BinaryFieldB127;

    /// Load one field element as a vector.
    #[inline(always)]
    pub(crate) unsafe fn ld(x: &BinaryFieldB127) -> uint64x2_t {
        // SAFETY: `uint.as_words()` is a valid 16-byte word pair.
        unsafe { vld1q_u64(x.uint.as_words().as_ptr()) }
    }

    /// Reduce a 256-bit vector accumulator and build the field element.
    #[inline(always)]
    unsafe fn to_elt(acc: (uint64x2_t, uint64x2_t)) -> BinaryFieldB127 {
        // SAFETY: as `reduce_256_b127`.
        unsafe {
            let r = reduce_256_b127(acc.0, acc.1);
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), r);
            BinaryFieldB127::from_words(out)
        }
    }

    /// One eq-factored round slot, fully NEON-resident — the GF128 slot
    /// with the trinomial weight-fold reductions:
    /// `a0 += (w·l0)·r0`, `a2 += (w·Δl)·Δr`, `a1 += w11 ⊕ wc0 ⊕ wc2`.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    unsafe fn eqf_slot(
        w: uint64x2_t,
        l0: uint64x2_t,
        l1: uint64x2_t,
        r0: uint64x2_t,
        r1: uint64x2_t,
        a0: &mut (uint64x2_t, uint64x2_t),
        a1: &mut (uint64x2_t, uint64x2_t),
        a2: &mut (uint64x2_t, uint64x2_t),
    ) {
        // SAFETY: as `reduce_256_b127`.
        unsafe {
            let (tl, th) = clmul_256(w, l0);
            let l0w = reduce_256_b127(tl, th);
            let (tl, th) = clmul_256(w, l1);
            let l1w = reduce_256_b127(tl, th);
            let (c0l, c0h) = clmul_256(l0w, r0);
            let (c1l, c1h) = clmul_256(l1w, r1);
            let dl = veorq_u64(l1w, l0w);
            let dr = veorq_u64(r1, r0);
            let (c2l, c2h) = clmul_256(dl, dr);
            a0.0 = veorq_u64(a0.0, c0l);
            a0.1 = veorq_u64(a0.1, c0h);
            a2.0 = veorq_u64(a2.0, c2l);
            a2.1 = veorq_u64(a2.1, c2h);
            a1.0 = veorq_u64(a1.0, veorq_u64(c1l, veorq_u64(c0l, c2l)));
            a1.1 = veorq_u64(a1.1, veorq_u64(c1h, veorq_u64(c0h, c2h)));
        }
    }

    /// NEON-resident single-pair round body (two interleaved slot
    /// chains, accumulators in vector registers, one cheap reduction per
    /// accumulator at the end).
    pub(crate) fn eqf_single_pair_round(
        l: &[BinaryFieldB127],
        r: &[BinaryFieldB127],
        w: &[BinaryFieldB127],
        half: usize,
    ) -> (BinaryFieldB127, BinaryFieldB127, BinaryFieldB127) {
        // SAFETY: as `reduce_256_b127`; all indices are in bounds by the
        // driver's contract (`l`, `r` have 2·half entries, `w` half).
        unsafe {
            let z = vdupq_n_u64(0);
            let mut a0a = (z, z);
            let mut a1a = (z, z);
            let mut a2a = (z, z);
            let mut a0b = (z, z);
            let mut a1b = (z, z);
            let mut a2b = (z, z);
            let mut b = 0usize;
            while b + 2 <= half {
                let e = b << 1;
                eqf_slot(
                    ld(&w[b]),
                    ld(&l[e]),
                    ld(&l[e | 1]),
                    ld(&r[e]),
                    ld(&r[e | 1]),
                    &mut a0a,
                    &mut a1a,
                    &mut a2a,
                );
                let e = (b + 1) << 1;
                eqf_slot(
                    ld(&w[b + 1]),
                    ld(&l[e]),
                    ld(&l[e | 1]),
                    ld(&r[e]),
                    ld(&r[e | 1]),
                    &mut a0b,
                    &mut a1b,
                    &mut a2b,
                );
                b += 2;
            }
            if b < half {
                let e = b << 1;
                eqf_slot(
                    ld(&w[b]),
                    ld(&l[e]),
                    ld(&l[e | 1]),
                    ld(&r[e]),
                    ld(&r[e | 1]),
                    &mut a0a,
                    &mut a1a,
                    &mut a2a,
                );
            }
            a0a.0 = veorq_u64(a0a.0, a0b.0);
            a0a.1 = veorq_u64(a0a.1, a0b.1);
            a1a.0 = veorq_u64(a1a.0, a1b.0);
            a1a.1 = veorq_u64(a1a.1, a1b.1);
            a2a.0 = veorq_u64(a2a.0, a2b.0);
            a2a.1 = veorq_u64(a2a.1, a2b.1);
            (to_elt(a0a), to_elt(a1a), to_elt(a2a))
        }
    }

    /// NEON-resident two-pair round body (the fraction-GKR layer): the
    /// two pairs are the two independent chains.
    pub(crate) fn eqf_two_pair_round(
        l0: &[BinaryFieldB127],
        r0: &[BinaryFieldB127],
        l1: &[BinaryFieldB127],
        r1: &[BinaryFieldB127],
        w: &[BinaryFieldB127],
        half: usize,
    ) -> (BinaryFieldB127, BinaryFieldB127, BinaryFieldB127) {
        // SAFETY: as `eqf_single_pair_round`.
        unsafe {
            let z = vdupq_n_u64(0);
            let mut a0a = (z, z);
            let mut a1a = (z, z);
            let mut a2a = (z, z);
            let mut a0b = (z, z);
            let mut a1b = (z, z);
            let mut a2b = (z, z);
            let mut b = 0usize;
            while b < half {
                let e = b << 1;
                let wv = ld(&w[b]);
                eqf_slot(
                    wv,
                    ld(&l0[e]),
                    ld(&l0[e | 1]),
                    ld(&r0[e]),
                    ld(&r0[e | 1]),
                    &mut a0a,
                    &mut a1a,
                    &mut a2a,
                );
                eqf_slot(
                    wv,
                    ld(&l1[e]),
                    ld(&l1[e | 1]),
                    ld(&r1[e]),
                    ld(&r1[e | 1]),
                    &mut a0b,
                    &mut a1b,
                    &mut a2b,
                );
                b += 1;
            }
            a0a.0 = veorq_u64(a0a.0, a0b.0);
            a0a.1 = veorq_u64(a0a.1, a0b.1);
            a1a.0 = veorq_u64(a1a.0, a1b.0);
            a1a.1 = veorq_u64(a1a.1, a1b.1);
            a2a.0 = veorq_u64(a2a.0, a2b.0);
            a2a.1 = veorq_u64(a2a.1, a2b.1);
            (to_elt(a0a), to_elt(a1a), to_elt(a2a))
        }
    }

    /// NEON-resident in-place fold `v[b] ← v[2b] ⊕ ρ·(v[2b+1] ⊕ v[2b])`,
    /// two independent entries per iteration.
    pub(crate) fn eqf_fold_in_place(v: &mut [BinaryFieldB127], rho: &BinaryFieldB127, half: usize) {
        // SAFETY: as `reduce_256_b127`; write index `b` is only ever read
        // at the earlier iteration `b/2`, so in-place is safe.
        unsafe {
            let rv = ld(rho);
            let mut b = 0usize;
            while b + 2 <= half {
                let v0a = ld(&v[b << 1]);
                let v1a = ld(&v[(b << 1) | 1]);
                let v0b = ld(&v[(b + 1) << 1]);
                let v1b = ld(&v[((b + 1) << 1) | 1]);
                let (pl, ph) = clmul_256(rv, veorq_u64(v1a, v0a));
                let pa = reduce_256_b127(pl, ph);
                let (pl, ph) = clmul_256(rv, veorq_u64(v1b, v0b));
                let pb = reduce_256_b127(pl, ph);
                let mut out = [0u64; 2];
                vst1q_u64(out.as_mut_ptr(), veorq_u64(v0a, pa));
                v[b] = BinaryFieldB127::from_words(out);
                vst1q_u64(out.as_mut_ptr(), veorq_u64(v0b, pb));
                v[b + 1] = BinaryFieldB127::from_words(out);
                b += 2;
            }
            if b < half {
                let v0 = ld(&v[b << 1]);
                let v1 = ld(&v[(b << 1) | 1]);
                let (pl, ph) = clmul_256(rv, veorq_u64(v1, v0));
                let p = reduce_256_b127(pl, ph);
                let mut out = [0u64; 2];
                vst1q_u64(out.as_mut_ptr(), veorq_u64(v0, p));
                v[b] = BinaryFieldB127::from_words(out);
            }
        }
    }
}

impl crate::utils::from_ref::FromRef<BinaryFieldB127> for BinaryFieldB127 {
    #[inline]
    fn from_ref(value: &BinaryFieldB127) -> Self {
        *value
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::utils::wide_mul::WideMulAcc;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn rand_elt(rng: &mut StdRng) -> BinaryFieldB127 {
        BinaryFieldB127::from_words([rng.next_u64(), rng.next_u64() & MASK_HI_B127])
    }

    /// Bit-by-bit shift-and-fold reference multiply modulo
    /// `X^127 + X + 1` (both operands canonical `< 2^127`). The
    /// correctness anchor for both pipelines.
    fn mul_reference(a: u128, b: u128) -> u128 {
        debug_assert!(a >> 127 == 0 && b >> 127 == 0);
        let mut acc = 0u128;
        let mut base = a;
        let mut m = b;
        while m != 0 {
            if m & 1 == 1 {
                acc ^= base;
            }
            m >>= 1;
            let carry = (base >> 126) & 1;
            base = (base << 1) & (u128::MAX >> 1);
            if carry == 1 {
                base ^= REDUCTION_LOW_B127 as u128; // X^127 ≡ X + 1
            }
        }
        acc
    }

    fn to_u128(x: &BinaryFieldB127) -> u128 {
        let w = x.words();
        (w[0] as u128) | ((w[1] as u128) << 64)
    }

    #[test]
    fn pipeline_matches_bit_reference() {
        let mut rng = StdRng::seed_from_u64(0xB127_0001);
        for _ in 0..2_000 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            let prod = a * b;
            assert_eq!(to_u128(&prod), mul_reference(to_u128(&a), to_u128(&b)));
            // Canonical invariant preserved.
            assert_eq!(prod.words()[1] >> 63, 0);
        }
        // Edge cases: extremes of the canonical range.
        let top = BinaryFieldB127::from_words([u64::MAX, MASK_HI_B127]);
        for (a, b) in [
            (BinaryFieldB127::zero(), top),
            (BinaryFieldB127::one(), top),
            (top, top),
            (BinaryFieldB127::from_words([2, 0]), top),
        ] {
            assert_eq!(to_u128(&(a * b)), mul_reference(to_u128(&a), to_u128(&b)));
        }
    }

    /// The NEON pipeline (products + trinomial fold + wide accumulators)
    /// is bit-identical to the scalar Karatsuba + shift/XOR pipeline —
    /// the b127 analogue of GF128's `neon_mul_matches_scalar_pipeline`.
    #[test]
    fn neon_mul_matches_scalar_pipeline() {
        let mut rng = StdRng::seed_from_u64(0xB127_0002);
        for _ in 0..2_000 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            let scalar = reduce_256_to_127(clmul_128x128(a.words(), b.words()));
            assert_eq!(*(a * b).words(), scalar, "mul: NEON vs scalar");
            let sq_scalar = {
                let w = a.words();
                let lo = clmul_64x64(w[0], w[0]);
                let hi = clmul_64x64(w[1], w[1]);
                reduce_256_to_127([lo[0], lo[1], hi[0], hi[1]])
            };
            assert_eq!(*a.square().words(), sq_scalar, "square: NEON vs scalar");
            // Wide roundtrip: from_wide(mul_wide(a, b)) == a·b.
            let w = BinaryFieldB127::mul_wide(&a, &b);
            assert_eq!(BinaryFieldB127::from_wide(w), a * b, "wide roundtrip");
        }
        // The Karatsuba NEON product variant agrees with the default.
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            let mut rng = StdRng::seed_from_u64(0xB127_0003);
            for _ in 0..2_000 {
                let a = rand_elt(&mut rng);
                let b = rand_elt(&mut rng);
                assert_eq!(
                    neon::mul_words_kara(a.words(), b.words()),
                    *(a * b).words(),
                    "kara vs schoolbook"
                );
                assert_eq!(
                    neon::mul_words_pfold(a.words(), b.words()),
                    *(a * b).words(),
                    "pfold vs schoolbook"
                );
            }
        }
    }

    #[test]
    fn zero_and_one_are_identities() {
        let mut rng = StdRng::seed_from_u64(0xB127_0004);
        for _ in 0..100 {
            let a = rand_elt(&mut rng);
            assert_eq!(a + BinaryFieldB127::zero(), a);
            assert_eq!(a * BinaryFieldB127::one(), a);
            assert_eq!(a * BinaryFieldB127::zero(), BinaryFieldB127::zero());
        }
    }

    #[test]
    fn add_is_xor_and_self_inverse() {
        let mut rng = StdRng::seed_from_u64(0xB127_0005);
        for _ in 0..100 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            let s = a + b;
            assert_eq!(s.words()[0], a.words()[0] ^ b.words()[0]);
            assert_eq!(s.words()[1], a.words()[1] ^ b.words()[1]);
            assert_eq!(a + a, BinaryFieldB127::zero());
            assert_eq!(a - b, a + b);
            assert_eq!(-a, a);
        }
    }

    #[test]
    fn multiplication_is_commutative_and_associative() {
        let mut rng = StdRng::seed_from_u64(0xB127_0006);
        for _ in 0..200 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            let c = rand_elt(&mut rng);
            assert_eq!(a * b, b * a);
            assert_eq!((a * b) * c, a * (b * c));
        }
    }

    #[test]
    fn distributivity_holds() {
        let mut rng = StdRng::seed_from_u64(0xB127_0007);
        for _ in 0..200 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            let c = rand_elt(&mut rng);
            assert_eq!(a * (b + c), a * b + a * c);
        }
    }

    #[test]
    fn square_matches_self_multiply() {
        let mut rng = StdRng::seed_from_u64(0xB127_0008);
        for _ in 0..500 {
            let a = rand_elt(&mut rng);
            assert_eq!(a.square(), a * a);
        }
    }

    #[test]
    fn frobenius_squaring_is_linear() {
        let mut rng = StdRng::seed_from_u64(0xB127_0009);
        for _ in 0..200 {
            let a = rand_elt(&mut rng);
            let b = rand_elt(&mut rng);
            assert_eq!((a + b).square(), a.square() + b.square());
        }
    }

    /// `a^{2^127} = a` — 127 squarings return every element to itself.
    /// Combined with the (classical) primality of `2^127 − 1`, this is the
    /// full order certificate: `ord(a) | M_127` prime ⇒ every `a ∉ {0, 1}`
    /// has order exactly `M_127` — the [`is_generator_b127`] soundness fact.
    #[test]
    fn frobenius_equals_2_pow_127() {
        let mut rng = StdRng::seed_from_u64(0xB127_000A);
        for _ in 0..25 {
            let a = rand_elt(&mut rng);
            let mut x = a;
            for _ in 0..127 {
                x = x.square();
            }
            assert_eq!(x, a);
        }
        assert_eq!(B127_MULT_ORDER, (1u128 << 127) - 1);
    }

    #[test]
    fn inverse_satisfies_a_times_inv_equals_one() {
        let mut rng = StdRng::seed_from_u64(0xB127_000B);
        for _ in 0..50 {
            let a = rand_elt(&mut rng);
            if a.is_zero() {
                continue;
            }
            assert_eq!(a * a.inverse(), BinaryFieldB127::one());
        }
        assert_eq!(
            BinaryFieldB127::one().inverse(),
            BinaryFieldB127::one()
        );
    }

    /// The Itoh–Tsujii inverse equals the naive Fermat all-ones ladder
    /// (the pre-optimisation implementation, inlined here as the
    /// reference), and `square_n` equals repeated squaring.
    #[test]
    fn itoh_tsujii_matches_fermat_ladder() {
        let mut rng = StdRng::seed_from_u64(0xB127_000E);
        for _ in 0..200 {
            let mut a = rand_elt(&mut rng);
            if a.is_zero() {
                a = BinaryFieldB127::one();
            }
            let fermat = {
                let mut c = a;
                for _ in 1..126 {
                    c = c.square();
                    c *= &a;
                }
                c.square()
            };
            assert_eq!(a.inverse(), fermat, "IT vs Fermat");
            let mut s = a;
            for k in 0..9 {
                assert_eq!(a.square_n(k), s, "square_n({k})");
                s = s.square();
            }
        }
    }

    #[test]
    fn division_by_self_yields_one() {
        let mut rng = StdRng::seed_from_u64(0xB127_000C);
        for _ in 0..25 {
            let a = rand_elt(&mut rng);
            if a.is_zero() {
                continue;
            }
            assert_eq!(a / a, BinaryFieldB127::one());
        }
    }

    #[test]
    fn mul_x_matches_general_multiply_by_generator() {
        let mut rng = StdRng::seed_from_u64(0xB127_000D);
        let x = BinaryFieldB127::from_words([2, 0]);
        for _ in 0..500 {
            let a = rand_elt(&mut rng);
            assert_eq!(a.mul_x(), a * x);
        }
        // And X^127 ≡ X + 1: 127 mul_x steps from 1 give 0x3.
        let mut p = BinaryFieldB127::one();
        for _ in 0..127 {
            p = p.mul_x();
        }
        assert_eq!(p, BinaryFieldB127::from_words([REDUCTION_LOW_B127, 0]));
    }

    /// `new_with_cfg` folds bit 127 exactly as `X^127 ≡ X + 1` demands:
    /// the fold of `v | 2^127` equals the fold of `v` plus `X + 1`.
    #[test]
    fn new_with_cfg_folds_bit_127() {
        let mut rng = StdRng::seed_from_u64(0xB127_000E);
        for _ in 0..200 {
            let lo = rng.next_u64();
            let hi = rng.next_u64() & MASK_HI_B127;
            let plain = BinaryFieldB127::new_with_cfg(Uint::<2>::from_words([lo, hi]), &());
            let topped =
                BinaryFieldB127::new_with_cfg(Uint::<2>::from_words([lo, hi | (1 << 63)]), &());
            assert_eq!(
                topped,
                plain + BinaryFieldB127::from_words([REDUCTION_LOW_B127, 0])
            );
            // Both are canonical.
            assert_eq!(plain.words()[1] >> 63, 0);
            assert_eq!(topped.words()[1] >> 63, 0);
        }
        // From<u128> agrees with new_with_cfg on every pattern shape.
        let v = u128::MAX;
        assert_eq!(
            BinaryFieldB127::from(v),
            BinaryFieldB127::new_with_cfg(Uint::<2>::from_words([v as u64, (v >> 64) as u64]), &())
        );
    }

    #[test]
    fn try_from_words_rejects_noncanonical() {
        assert!(BinaryFieldB127::try_from_words([0, 1 << 63]).is_none());
        assert!(BinaryFieldB127::try_from_words([u64::MAX, MASK_HI_B127]).is_some());
    }

    #[test]
    fn is_generator_rejects_exactly_the_subfield() {
        assert!(!is_generator_b127(&BinaryFieldB127::zero()));
        assert!(!is_generator_b127(&BinaryFieldB127::one()));
        assert!(is_generator_b127(&BinaryFieldB127::from_words([2, 0])));
        assert!(is_generator_b127(&BinaryFieldB127::from_words([3, 0])));
    }

    /// The trait law: sums of products through the wide accumulator equal
    /// the same sums with reduced multiplies — bit-for-bit.
    #[test]
    fn wide_accumulate_matches_reduced_sum() {
        let mut rng = StdRng::seed_from_u64(0xB127_000F);
        for _ in 0..50 {
            let terms: Vec<(BinaryFieldB127, BinaryFieldB127)> =
                (0..17).map(|_| (rand_elt(&mut rng), rand_elt(&mut rng))).collect();
            let mut acc = BinaryFieldB127::wide_zero(&BinaryFieldB127::zero());
            let mut reduced = BinaryFieldB127::zero();
            for (a, b) in &terms {
                BinaryFieldB127::wide_add_assign(&mut acc, &BinaryFieldB127::mul_wide(a, b));
                reduced += *a * *b;
            }
            assert_eq!(BinaryFieldB127::from_wide(acc), reduced);
        }
    }

    /// The fused eq-factored kernels are value-exact against naive field
    /// arithmetic (the driver-contract identities).
    #[test]
    fn eqf_kernels_are_value_exact() {
        let mut rng = StdRng::seed_from_u64(0xB127_0010);
        for half in [1usize, 2, 3, 8, 13] {
            let l: Vec<_> = (0..2 * half).map(|_| rand_elt(&mut rng)).collect();
            let r: Vec<_> = (0..2 * half).map(|_| rand_elt(&mut rng)).collect();
            let l1: Vec<_> = (0..2 * half).map(|_| rand_elt(&mut rng)).collect();
            let r1: Vec<_> = (0..2 * half).map(|_| rand_elt(&mut rng)).collect();
            let w: Vec<_> = (0..half).map(|_| rand_elt(&mut rng)).collect();

            // Single-pair reference.
            let (mut c0, mut c1, mut c2) =
                (BinaryFieldB127::zero(), BinaryFieldB127::zero(), BinaryFieldB127::zero());
            for b in 0..half {
                let (wl0, wl1) = (w[b] * l[2 * b], w[b] * l[2 * b + 1]);
                let (r0v, r1v) = (r[2 * b], r[2 * b + 1]);
                let t0 = wl0 * r0v;
                let t2 = (wl1 - wl0) * (r1v - r0v);
                c0 += t0;
                c2 += t2;
                c1 += wl1 * r1v - t0 - t2;
            }
            let got = BinaryFieldB127::eqf_single_pair_round(&l, &r, &w, half).unwrap();
            assert_eq!(got, (c0, c1, c2), "single-pair, half={half}");

            // Two-pair reference: the same three sums over BOTH pairs.
            let (mut d0, mut d1, mut d2) = (c0, c1, c2);
            for b in 0..half {
                let (wl0, wl1) = (w[b] * l1[2 * b], w[b] * l1[2 * b + 1]);
                let (r0v, r1v) = (r1[2 * b], r1[2 * b + 1]);
                let t0 = wl0 * r0v;
                let t2 = (wl1 - wl0) * (r1v - r0v);
                d0 += t0;
                d2 += t2;
                d1 += wl1 * r1v - t0 - t2;
            }
            let got2 =
                BinaryFieldB127::eqf_two_pair_round(&l, &r, &l1, &r1, &w, half).unwrap();
            assert_eq!(got2, (d0, d1, d2), "two-pair, half={half}");

            // Fold reference.
            let rho = rand_elt(&mut rng);
            let mut v = l.clone();
            let expected: Vec<_> = (0..half)
                .map(|b| v[2 * b] + rho * (v[2 * b + 1] - v[2 * b]))
                .collect();
            assert!(BinaryFieldB127::eqf_fold_in_place(&mut v, &rho, half));
            assert_eq!(&v[..half], &expected[..], "fold, half={half}");
        }
    }

    #[test]
    fn implements_field_and_prime_field_traits() {
        fn assert_field<F: crypto_primitives::Field>() {}
        fn assert_prime_field<F: crypto_primitives::PrimeField>() {}
        assert_field::<BinaryFieldB127>();
        assert_prime_field::<BinaryFieldB127>();
    }

    #[test]
    fn cfg_keyed_constructors_match_const_constructors() {
        assert_eq!(BinaryFieldB127::zero_with_cfg(&()), BinaryFieldB127::zero());
        assert_eq!(BinaryFieldB127::one_with_cfg(&()), BinaryFieldB127::one());
        assert!(BinaryFieldB127::make_cfg(&B127_MODULUS).is_ok());
        assert!(
            BinaryFieldB127::make_cfg(&Uint::<2>::from_words([0x87, 0])).is_err(),
            "the GHASH modulus is not the b127 modulus"
        );
    }

    #[test]
    #[should_panic(expected = "modulus_minus_one_div_two")]
    fn modulus_minus_one_div_two_panics() {
        let _ = BinaryFieldB127::one().modulus_minus_one_div_two();
    }

    /// The general (two-fold) scalar reducer agrees with reduce-then-add
    /// linearity on arbitrary 256-bit inputs, and with the reference on
    /// in-contract products.
    #[test]
    fn scalar_reducer_is_linear_and_total() {
        let mut rng = StdRng::seed_from_u64(0xB127_0011);
        for _ in 0..500 {
            let w: [u64; 4] = core::array::from_fn(|_| rng.next_u64());
            let v: [u64; 4] = core::array::from_fn(|_| rng.next_u64());
            let xor = core::array::from_fn(|i| w[i] ^ v[i]);
            // F_2-linearity of the reduction, even on out-of-contract input.
            let rw = BinaryFieldB127::reduce_wide(w);
            let rv = BinaryFieldB127::reduce_wide(v);
            assert_eq!(BinaryFieldB127::reduce_wide(xor), rw + rv);
            // Canonical output always.
            assert_eq!(rw.words()[1] >> 63, 0);
        }
        // Totality anchor: reducing X^255 = X^128·X^127 ≡ (X+1)·X^128 …
        // — checked against the bit reference through a product that
        // reaches the top: (X^126)² = X^252.
        let x126 = {
            let mut p = BinaryFieldB127::one();
            for _ in 0..126 {
                p = p.mul_x();
            }
            p
        };
        let sq = x126.square();
        assert_eq!(to_u128(&sq), mul_reference(to_u128(&x126), to_u128(&x126)));
    }
}
