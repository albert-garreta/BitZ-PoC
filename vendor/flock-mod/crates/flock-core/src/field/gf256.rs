//! GF(2^256) as the quadratic extension `GF(2^128)[Y] / (Y² + Y + c)` of
//! [`Gf128`]: the challenge field of Ligerito's extension-field opening
//! ([`crate::pcs::ligerito::ext`]), where every soundness term that divides
//! by the field size gains 128 bits while committed data stays in `Gf128`.
//!
//! `c = X^121` has absolute trace 1 over GF(2) (Newton's identities on
//! `X^128 + X^7 + X^2 + X + 1`: the power sums `s_1 … s_120` vanish and
//! `s_121 = 121 = 1`), so `Y² + Y + c` has no root in GF(2^128) and the
//! quotient is a field. The tests recompute the trace.

use super::Gf128;
use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// `a + b·Y` with `Y² = Y + c`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Gf256 {
    pub a: Gf128,
    pub b: Gf128,
}

impl Gf256 {
    pub const ZERO: Self = Self::new(Gf128::ZERO, Gf128::ZERO);
    pub const ONE: Self = Self::new(Gf128::ONE, Gf128::ZERO);
    /// `c` in `Y² = Y + c`: `X^121`.
    pub const C: Gf128 = Gf128::new(0, 1 << 57);

    pub const fn new(a: Gf128, b: Gf128) -> Self {
        Self { a, b }
    }

    /// The base-field embedding `a ↦ a + 0·Y`.
    pub const fn from_base(a: Gf128) -> Self {
        Self::new(a, Gf128::ZERO)
    }

    pub const fn is_zero(self) -> bool {
        self.a.is_zero() && self.b.is_zero()
    }

    /// Multiply by a base-field scalar: two base multiplications.
    #[inline]
    pub fn mul_base(self, s: Gf128) -> Self {
        Self::new(self.a * s, self.b * s)
    }

    #[inline]
    pub fn square(self) -> Self {
        self * self
    }

    /// 32 bytes: `a` then `b`, each in [`Gf128::to_bytes`]'s encoding.
    pub fn to_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..16].copy_from_slice(&self.a.to_bytes());
        out[16..].copy_from_slice(&self.b.to_bytes());
        out
    }

    /// A slice of extension elements as its `2·len` base coordinates
    /// (`a₀, b₀, a₁, b₁, …`). The layout is `repr(C)` with no padding, so
    /// an extension vector is a base vector with one more (lowest) lane bit:
    /// base-linear maps that act lane by lane (encoding, Merkle leaves)
    /// apply to it unchanged.
    pub fn as_base_slice(values: &[Self]) -> &[Gf128] {
        // SAFETY: `Gf256` is `repr(C)` over two `Gf128`s (size 32, the
        // alignment of `Gf128`, no padding), so `len` extension elements
        // are exactly `2·len` consecutive, aligned `Gf128`s.
        unsafe { core::slice::from_raw_parts(values.as_ptr().cast::<Gf128>(), values.len() * 2) }
    }

    /// Inverse of [`Self::as_base_slice`] for an owned vector of even length.
    pub fn from_base_pairs(values: &[Gf128]) -> Vec<Self> {
        assert!(values.len() % 2 == 0, "base coordinates come in pairs");
        values.chunks_exact(2).map(|p| Self::new(p[0], p[1])).collect()
    }
}

impl From<Gf128> for Gf256 {
    fn from(a: Gf128) -> Self {
        Self::from_base(a)
    }
}

impl Add for Gf256 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.a + rhs.a, self.b + rhs.b)
    }
}

impl AddAssign for Gf256 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Gf256 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self + rhs
    }
}

impl SubAssign for Gf256 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Neg for Gf256 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        self
    }
}

impl Mul for Gf256 {
    type Output = Self;
    /// Karatsuba: `(a₀ + a₁Y)(b₀ + b₁Y) = (p₀ + c·p₁) + (p₂ + p₀)·Y` with
    /// `p₀ = a₀b₀`, `p₁ = a₁b₁`, `p₂ = (a₀ + a₁)(b₀ + b₁)`.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let p0 = self.a * rhs.a;
        let p1 = self.b * rhs.b;
        let p2 = (self.a + self.b) * (rhs.a + rhs.b);
        Self::new(p0 + Self::C * p1, p2 + p0)
    }
}

impl MulAssign for Gf256 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Mul<Gf128> for Gf256 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Gf128) -> Self {
        self.mul_base(rhs)
    }
}

impl std::iter::Sum for Gf256 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, v| acc + v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        }
        fn f128(&mut self) -> Gf128 {
            Gf128::new(self.next_u64(), self.next_u64())
        }
        fn f256(&mut self) -> Gf256 {
            Gf256::new(self.f128(), self.f128())
        }
    }

    /// `Σ_{j<128} c^{2^j}`, the absolute trace of `c`.
    fn trace(c: Gf128) -> Gf128 {
        let mut t = c;
        let mut sum = c;
        for _ in 1..128 {
            t = t * t;
            sum += t;
        }
        sum
    }

    #[test]
    fn c_has_trace_one_so_the_extension_is_a_field() {
        assert_eq!(trace(Gf256::C), Gf128::ONE);
        // X^121 really is the constant.
        let mut x121 = Gf128::ONE;
        for _ in 0..121 {
            x121 = x121 * Gf128::GENERATOR;
        }
        assert_eq!(x121, Gf256::C);
    }

    #[test]
    fn karatsuba_matches_schoolbook() {
        let mut rng = Rng(7);
        for _ in 0..256 {
            let (x, y) = (rng.f256(), rng.f256());
            // (x.a + x.b Y)(y.a + y.b Y) with Y² = Y + c.
            let a = x.a * y.a + Gf256::C * (x.b * y.b);
            let b = x.a * y.b + x.b * y.a + x.b * y.b;
            assert_eq!(x * y, Gf256::new(a, b));
        }
    }

    #[test]
    fn field_axioms() {
        let mut rng = Rng(11);
        for _ in 0..256 {
            let (x, y, z) = (rng.f256(), rng.f256(), rng.f256());
            assert_eq!(x * y, y * x);
            assert_eq!((x * y) * z, x * (y * z));
            assert_eq!(x * (y + z), x * y + x * z);
            assert_eq!(x * Gf256::ONE, x);
            assert_eq!(x * Gf256::ZERO, Gf256::ZERO);
            let s = rng.f128();
            assert_eq!(x * Gf256::from_base(s), x.mul_base(s));
        }
        // Y² = Y + c.
        let y = Gf256::new(Gf128::ZERO, Gf128::ONE);
        assert_eq!(y * y, Gf256::new(Gf256::C, Gf128::ONE));
    }

    #[test]
    fn frobenius_has_order_256() {
        // x^{2^256} = x for every x in a field of 2^256 elements, while
        // x^{2^128} ≠ x for x outside GF(2^128): the extension is GF(2^256).
        let mut rng = Rng(13);
        let x = rng.f256();
        let mut t = x;
        for _ in 0..128 {
            t = t.square();
        }
        assert_ne!(t, x);
        for _ in 0..128 {
            t = t.square();
        }
        assert_eq!(t, x);
    }

    #[test]
    fn base_slice_view_interleaves_coordinates() {
        let mut rng = Rng(17);
        let v: Vec<Gf256> = (0..5).map(|_| rng.f256()).collect();
        let base = Gf256::as_base_slice(&v);
        assert_eq!(base.len(), 10);
        for (i, e) in v.iter().enumerate() {
            assert_eq!(base[2 * i], e.a);
            assert_eq!(base[2 * i + 1], e.b);
        }
        assert_eq!(Gf256::from_base_pairs(base), v);
    }
}
