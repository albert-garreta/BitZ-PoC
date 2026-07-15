//! Delayed-reduction multiply-accumulate for sumcheck inner loops.
//!
//! The eq-factored sumcheck's round body accumulates many products whose
//! results feed only field *additions* (the round-polynomial coefficient
//! sums), never another multiplication. For fields whose reduction step is
//! a substantial fraction of the multiply — `GF(2^128)` via carryless
//! Karatsuba, where reduction mod `X^128 + X^7 + X^2 + X + 1` costs two
//! word-times-g folds — those products can be accumulated in their
//! **unreduced** wide form (XOR of 256-bit carryless products) and reduced
//! once per accumulator per round. Reduction is `F_2`-linear, so
//! `reduce(Σ wide_i) = Σ reduce(wide_i)` exactly: the final field elements
//! (and every transcript byte downstream) are unchanged.
//!
//! Prime fields take the trivial instance (`Wide = Self`, reduced ops), so
//! the driver stays field-generic with zero overhead there.

use crypto_bigint::modular::ConstMontyParams;
use crypto_primitives::{
    crypto_bigint_const_monty::ConstMontyField, crypto_bigint_monty::MontyField,
};

/// Multiply-accumulate with an opaque (possibly unreduced) accumulator.
///
/// Laws (all exact, no approximation):
/// - `from_wide(wide_zero(zero)) == zero`
/// - `from_wide(w) == x + y` when `w = wide_add(wide_of(x), wide_of(y))`
/// - `from_wide(mul_wide(a, b)) == a * b`
///
/// so any sum of products computed through this trait equals the same sum
/// computed with reduced multiplies — bit-for-bit.
pub trait WideMulAcc: Sized {
    /// The accumulator representation (unreduced for char-2 carryless
    /// fields; `Self` for fields whose multiply is cheapest reduced).
    type Wide: Clone;

    /// The additive-identity accumulator. Takes the field zero so
    /// runtime-config fields can seed config-carrying values.
    fn wide_zero(zero: &Self) -> Self::Wide;

    /// The wide form of an already-reduced field element.
    fn wide_of(x: &Self) -> Self::Wide;

    /// `a · b` in wide form (reduction deferred).
    fn mul_wide(a: &Self, b: &Self) -> Self::Wide;

    /// `acc += x`.
    fn wide_add_assign(acc: &mut Self::Wide, x: &Self::Wide);

    /// `acc -= x`.
    fn wide_sub_assign(acc: &mut Self::Wide, x: &Self::Wide);

    /// Reduce the accumulator to a field element.
    fn from_wide(w: Self::Wide) -> Self;

    /// Conditional accumulate `if mask { *acc += x }` — override with a
    /// BRANCHLESS select where the representation allows (char-2 fields:
    /// `acc ^= (0−mask) & x`), so hot loops driven by unpredictable data
    /// bits pay a constant ~1 ns instead of a coin-flip branch.
    /// Value-exact either way.
    fn add_assign_masked(acc: &mut Self, x: &Self, mask: bool);

    /// Optional fused kernel for the eq-factored sumcheck's single-pair
    /// round body: over `b < half`, with `l0 = l[2b]`, `l1 = l[2b+1]`
    /// (same for `r`), returns
    /// `Some((Σ w_b·l0·r0, Σ (w_b·l1·r1 − w_b·l0·r0 − w_b·Δl·Δr), Σ w_b·Δl·Δr))`
    /// where `Δl = l1 − l0` weighted as `w·l1 − w·l0`. Any override must be
    /// VALUE-EXACT (only reorder exact field ops); return `None` to use the
    /// driver's generic loop. Lets a field ship a hand-scheduled kernel
    /// (e.g. interleaved PMULL chains) without the driver losing genericity.
    fn eqf_single_pair_round(
        _l: &[Self],
        _r: &[Self],
        _w: &[Self],
        _half: usize,
    ) -> Option<(Self, Self, Self)> {
        None
    }

    /// Optional fused kernel for the eq-factored sumcheck's TWO-pair round
    /// body (the fraction-GKR layer combine `L₀·R₀ + L₁·R₁` per slot —
    /// `quotient_gkr`'s `nl·dr + dl·aux`): over `b < half`, with each
    /// pair's entries at `2b, 2b+1` and the shared weight `w_b`, returns
    /// the three round-polynomial coefficients
    /// `Some((Σ_b Σ_p w_b·l0·r0, Σ_b Σ_p (w_b·l1·r1 − w_b·l0·r0 − w_b·Δl·Δr), Σ_b Σ_p w_b·Δl·Δr))`.
    /// Any override must be VALUE-EXACT vs the driver's generic multi-pair
    /// loop (only reorder exact field ops — for char-2 wide accumulation,
    /// `w·(A+B) = w·A + w·B` and reduction is `F₂`-linear); return `None`
    /// to use the generic loop.
    fn eqf_two_pair_round(
        _l0: &[Self],
        _r0: &[Self],
        _l1: &[Self],
        _r1: &[Self],
        _w: &[Self],
        _half: usize,
    ) -> Option<(Self, Self, Self)> {
        None
    }

    /// Optional fused kernel for the eq-factored fold
    /// `v[b] ← v[2b] + ρ·(v[2b+1] − v[2b])` for `b < half` (caller
    /// truncates). Must be value-exact; return `false` to use the generic
    /// in-place loop.
    fn eqf_fold_in_place(_v: &mut [Self], _rho: &Self, _half: usize) -> bool {
        false
    }
}

/// Compile-time-modulus prime fields: reduced representation IS the
/// accumulator.
impl<Mod: ConstMontyParams<LIMBS>, const LIMBS: usize> WideMulAcc for ConstMontyField<Mod, LIMBS> {
    type Wide = Self;

    #[inline(always)]
    fn wide_zero(zero: &Self) -> Self::Wide {
        zero.clone()
    }

    #[inline(always)]
    fn wide_of(x: &Self) -> Self::Wide {
        x.clone()
    }

    #[inline(always)]
    fn mul_wide(a: &Self, b: &Self) -> Self::Wide {
        a.clone() * b
    }

    #[inline(always)]
    fn wide_add_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        *acc += x;
    }

    #[inline(always)]
    fn wide_sub_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        *acc -= x;
    }

    #[inline(always)]
    fn from_wide(w: Self::Wide) -> Self {
        w
    }

    #[inline(always)]
    fn add_assign_masked(acc: &mut Self, x: &Self, mask: bool) {
        if mask {
            *acc += x;
        }
    }
}

/// Prime fields: the reduced representation IS the accumulator.
impl<const LIMBS: usize> WideMulAcc for MontyField<LIMBS> {
    type Wide = Self;

    #[inline(always)]
    fn wide_zero(zero: &Self) -> Self::Wide {
        zero.clone()
    }

    #[inline(always)]
    fn wide_of(x: &Self) -> Self::Wide {
        x.clone()
    }

    #[inline(always)]
    fn mul_wide(a: &Self, b: &Self) -> Self::Wide {
        a.clone() * b
    }

    #[inline(always)]
    fn wide_add_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        *acc += x;
    }

    #[inline(always)]
    fn wide_sub_assign(acc: &mut Self::Wide, x: &Self::Wide) {
        *acc -= x;
    }

    #[inline(always)]
    fn from_wide(w: Self::Wide) -> Self {
        w
    }

    #[inline(always)]
    fn add_assign_masked(acc: &mut Self, x: &Self, mask: bool) {
        if mask {
            *acc += x;
        }
    }
}
