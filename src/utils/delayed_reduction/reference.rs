//! Delayed modular reduction for two-limb runtime Montgomery fields.
//!
//! The accumulators in this module deliberately preserve their Montgomery
//! scaling in their Rust types:
//!
//! - [`MontyProductAccumulator128`] contains sums of products of two raw
//!   Montgomery residues, and therefore has scale `R^2`.
//! - [`MontyLinearAccumulator128`] contains sums of a raw Montgomery residue
//!   times a native `u64` or `bool`, and therefore has scale `R`.
//!
//! Both accumulators have one carry limb. A single field product occupies at
//! most four limbs, so five limbs can hold any sum of fewer than `2^64` such
//! products. Reduction is intentionally explicit and consumes the accumulator:
//! an unreduced value must never be stored in an MLE, transcript, or proof.

use core::{
    marker::PhantomData,
    ops::{Add, AddAssign},
};

use crypto_bigint::{NonZero, Uint as CryptoUint};
use crypto_primitives::{crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint};
use num_traits::Zero;

/// An accumulator that can add an unreduced `lhs * rhs` product.
pub trait Accumulatable<Lhs, Rhs>: Zero + AddAssign {
    /// Adds `lhs * rhs` without performing modular reduction.
    fn multiply_accumulate(&mut self, lhs: &Lhs, rhs: &Rhs);
}

/// Reduces an accumulator into its destination field.
pub trait Reduce<F, Reducer: ?Sized>: Sized {
    /// Consumes and reduces `self` using prepared public-modulus constants.
    fn reduce(self, reducer: &Reducer) -> Result<F, DelayedReductionError>;
}

use super::DelayedReductionError;

/// Optimized fixed-schedule Barrett/Montgomery reduction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OptimizedReduction;

/// `crypto-bigint` mixed-width remainder reference reduction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct CryptoBigintReduction;

mod private {
    pub trait Sealed {}
}

/// Marker implemented by the supported monomorphized reduction backends.
pub(crate) trait Monty128ReductionBackend:
    private::Sealed + Clone + Copy + Send + Sync + 'static
{
}

impl private::Sealed for OptimizedReduction {}
impl Monty128ReductionBackend for OptimizedReduction {}
#[cfg(test)]
impl private::Sealed for CryptoBigintReduction {}
#[cfg(test)]
impl Monty128ReductionBackend for CryptoBigintReduction {}

/// Public-modulus constants prepared once for delayed reduction.
///
/// Preparation may use variable-time division because the modulus is public.
/// Neither optimized reduction path performs division or remainder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Monty128Reducer<B: Monty128ReductionBackend> {
    config: crypto_bigint::modular::FixedMontyParams<2>,
    modulus: [u64; 2],
    montgomery_one: [u64; 2],
    r2: [u64; 2],
    barrett_mu: [u64; 3],
    montgomery_neg_inv: u64,
    _backend: PhantomData<B>,
}

/// The optimized delayed-reduction context.
pub(crate) type OptimizedMonty128Reducer = Monty128Reducer<OptimizedReduction>;

/// The `crypto-bigint` reference delayed-reduction context.
#[cfg(test)]
pub(crate) type CryptoBigintMonty128Reducer = Monty128Reducer<CryptoBigintReduction>;

impl<B: Monty128ReductionBackend> Monty128Reducer<B> {
    /// Prepares constants for an odd runtime modulus in `(2^64, 2^128)`.
    ///
    /// The input is a `FixedMontyParams<2>`, so oddness and the 128-bit upper bound
    /// are already enforced by `crypto-bigint`. The lower bound is the one
    /// required by the fixed `k = 2` Barrett construction used here.
    pub(crate) fn new(
        config: &crypto_bigint::modular::FixedMontyParams<2>,
    ) -> Result<Self, DelayedReductionError> {
        let modulus_value = config.modulus().get();
        let modulus = words2(&modulus_value);
        if modulus[1] == 0 {
            return Err(DelayedReductionError::UnsupportedModulus);
        }

        // mu = floor(2^256 / modulus). This setup division depends only on the
        // public modulus and is never invoked from an accumulation hot loop.
        let numerator = CryptoUint::<5>::from_words([0, 0, 0, 0, 1]);
        let nonzero_modulus = NonZero::new(CryptoUint::<2>::from_words(modulus))
            .expect("FixedMontyParams always contains a nonzero modulus");
        let (mu, _) = numerator.div_rem_vartime(&nonzero_modulus);
        let mu_words = mu.to_words();
        debug_assert_eq!(mu_words[3], 0);
        debug_assert_eq!(mu_words[4], 0);

        let q0_inverse = inverse_odd_u64(modulus[0]);
        let montgomery_neg_inv = q0_inverse.wrapping_neg();
        debug_assert_eq!(modulus[0].wrapping_mul(montgomery_neg_inv), u64::MAX);

        Ok(Self {
            config: *config,
            modulus,
            montgomery_one: words2(config.one()),
            r2: words2(config.r2()),
            barrett_mu: [mu_words[0], mu_words[1], mu_words[2]],
            montgomery_neg_inv,
            _backend: PhantomData,
        })
    }
}

/// A five-limb sum of raw Montgomery-residue products (`R^2` scaling).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MontyProductAccumulator128 {
    limbs: [u64; 5],
}

/// A five-limb sum of Montgomery-residue × native products (`R` scaling).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MontyLinearAccumulator128 {
    limbs: [u64; 5],
}

macro_rules! impl_accumulator_arithmetic {
    ($accumulator:ty) => {
        impl AddAssign for $accumulator {
            #[inline(always)]
            fn add_assign(&mut self, rhs: Self) {
                let mut carry = 0u64;
                let mut i = 0;
                while i < 5 {
                    let sum = (self.limbs[i] as u128) + (rhs.limbs[i] as u128) + (carry as u128);
                    self.limbs[i] = sum as u64;
                    carry = (sum >> 64) as u64;
                    i += 1;
                }
                debug_assert_eq!(carry, 0, "five-limb delayed accumulator overflow");
            }
        }

        impl Add for $accumulator {
            type Output = Self;

            #[inline(always)]
            fn add(mut self, rhs: Self) -> Self::Output {
                self += rhs;
                self
            }
        }

        impl Zero for $accumulator {
            #[inline(always)]
            fn zero() -> Self {
                Self::default()
            }

            #[inline(always)]
            fn is_zero(&self) -> bool {
                (self.limbs[0] | self.limbs[1] | self.limbs[2] | self.limbs[3] | self.limbs[4]) == 0
            }
        }
    };
}

impl_accumulator_arithmetic!(MontyProductAccumulator128);
impl_accumulator_arithmetic!(MontyLinearAccumulator128);

impl Accumulatable<MontyField<2>, MontyField<2>> for MontyProductAccumulator128 {
    #[inline(always)]
    fn multiply_accumulate(&mut self, lhs: &MontyField<2>, rhs: &MontyField<2>) {
        multiply_accumulate_2x2(&mut self.limbs, raw_words(lhs), raw_words(rhs));
    }
}

impl Accumulatable<MontyField<2>, u64> for MontyLinearAccumulator128 {
    #[inline(always)]
    fn multiply_accumulate(&mut self, lhs: &MontyField<2>, rhs: &u64) {
        multiply_accumulate_2x1(&mut self.limbs, raw_words(lhs), *rhs);
    }
}

impl Accumulatable<MontyField<2>, bool> for MontyLinearAccumulator128 {
    #[inline(always)]
    fn multiply_accumulate(&mut self, lhs: &MontyField<2>, rhs: &bool) {
        // Avoid a witness-derived branch for Boolean matrix coefficients.
        let mask = 0u64.wrapping_sub(u64::from(*rhs));
        let words = raw_words(lhs);
        add_masked_2(&mut self.limbs, words, mask);
    }
}

impl MontyProductAccumulator128 {
    /// Adds the product of two raw two-limb Montgomery residues (the packed
    /// limbs of [`MontyField::as_montgomery`]).
    #[inline(always)]
    pub(crate) fn multiply_accumulate_raw(&mut self, lhs: u128, rhs: u128) {
        multiply_accumulate_2x2(&mut self.limbs, split_u128(lhs), split_u128(rhs));
    }

    /// The optimized reduction, returned as the packed canonical residue
    /// (`R` scaling) instead of a configured field element.
    #[inline]
    pub(crate) fn reduce_raw(self, reducer: &OptimizedMonty128Reducer) -> u128 {
        let (folded, fold_carry) = fold_fifth_limb(self.limbs, reducer.r2);
        let redc = montgomery_reduce_4(folded, reducer);
        let canonical = barrett_reduce_4([redc[0], redc[1], redc[2], 0], reducer);
        join_u128(add_mod_masked(
            canonical,
            reducer.montgomery_one,
            fold_carry,
            reducer.modulus,
        ))
    }

    /// The plain remainder of the five-limb sum modulo `q`, without the
    /// Montgomery division: for a sum of `raw × plain` products (scale `R`)
    /// this is the canonical raw residue of the field sum.
    #[inline]
    pub(crate) fn reduce_raw_mod_q(self, reducer: &OptimizedMonty128Reducer) -> u128 {
        let (folded, fold_carry) = fold_fifth_limb(self.limbs, reducer.r2);
        let canonical = barrett_reduce_4(folded, reducer);
        join_u128(add_mod_masked(
            canonical,
            reducer.r2,
            fold_carry,
            reducer.modulus,
        ))
    }
}

impl MontyLinearAccumulator128 {
    /// The five little-endian limbs of the unreduced sum.
    #[inline(always)]
    pub(crate) const fn limbs(&self) -> [u64; 5] {
        self.limbs
    }

    /// Adds the product of a raw two-limb Montgomery residue and a native
    /// `u64`.
    #[inline(always)]
    pub(crate) fn multiply_accumulate_raw(&mut self, lhs: u128, rhs: u64) {
        multiply_accumulate_2x1(&mut self.limbs, split_u128(lhs), rhs);
    }

    /// The optimized reduction, returned as the packed canonical residue
    /// (`R` scaling) instead of a configured field element.
    #[inline]
    pub(crate) fn reduce_raw(self, reducer: &OptimizedMonty128Reducer) -> u128 {
        let (folded, fold_carry) = fold_fifth_limb(self.limbs, reducer.r2);
        let canonical = barrett_reduce_4(folded, reducer);
        join_u128(add_mod_masked(
            canonical,
            reducer.r2,
            fold_carry,
            reducer.modulus,
        ))
    }
}

impl Reduce<MontyField<2>, OptimizedMonty128Reducer> for MontyProductAccumulator128 {
    #[inline]
    fn reduce(
        self,
        reducer: &OptimizedMonty128Reducer,
    ) -> Result<MontyField<2>, DelayedReductionError> {
        Ok(field_from_raw(
            split_u128(self.reduce_raw(reducer)),
            &reducer.config,
        ))
    }
}

impl Reduce<MontyField<2>, OptimizedMonty128Reducer> for MontyLinearAccumulator128 {
    #[inline]
    fn reduce(
        self,
        reducer: &OptimizedMonty128Reducer,
    ) -> Result<MontyField<2>, DelayedReductionError> {
        Ok(field_from_raw(
            split_u128(self.reduce_raw(reducer)),
            &reducer.config,
        ))
    }
}

#[cfg(test)]
impl Reduce<MontyField<2>, CryptoBigintMonty128Reducer> for MontyProductAccumulator128 {
    fn reduce(
        self,
        reducer: &CryptoBigintMonty128Reducer,
    ) -> Result<MontyField<2>, DelayedReductionError> {
        let remainder = reference_remainder(self.limbs, reducer);

        // `remainder` has R^2 scaling. Wrapping it once makes a field value
        // whose canonical integer has R scaling; retrieve that integer and
        // wrap it as the desired raw R-scaled field representation.
        let twice_scaled = field_from_raw(remainder, &reducer.config);
        Ok(MontyField::from_montgomery(
            twice_scaled.retrieve(),
            &reducer.config,
        ))
    }
}

#[cfg(test)]
impl Reduce<MontyField<2>, CryptoBigintMonty128Reducer> for MontyLinearAccumulator128 {
    fn reduce(
        self,
        reducer: &CryptoBigintMonty128Reducer,
    ) -> Result<MontyField<2>, DelayedReductionError> {
        Ok(field_from_raw(
            reference_remainder(self.limbs, reducer),
            &reducer.config,
        ))
    }
}

#[inline(always)]
const fn split_u128(value: u128) -> [u64; 2] {
    [value as u64, (value >> 64) as u64]
}

#[inline(always)]
const fn join_u128(words: [u64; 2]) -> u128 {
    (words[0] as u128) | ((words[1] as u128) << 64)
}

#[inline(always)]
fn words2(value: &CryptoUint<2>) -> [u64; 2] {
    let words = value.as_words();
    [words[0], words[1]]
}

#[inline(always)]
fn raw_words(value: &MontyField<2>) -> [u64; 2] {
    let words = value.as_montgomery().as_words();
    [words[0], words[1]]
}

#[inline(always)]
fn field_from_raw(
    words: [u64; 2],
    config: &crypto_bigint::modular::FixedMontyParams<2>,
) -> MontyField<2> {
    MontyField::from_montgomery(FieldUint::from_words(words), config)
}

#[inline(always)]
fn inverse_odd_u64(value: u64) -> u64 {
    // Newton iteration doubles the number of correct low bits each round.
    let mut inverse = 1u64;
    let mut i = 0;
    while i < 6 {
        inverse = inverse.wrapping_mul(2u64.wrapping_sub(value.wrapping_mul(inverse)));
        i += 1;
    }
    inverse
}

#[inline(always)]
fn mac(accumulator: u64, lhs: u64, rhs: u64, carry: u64) -> (u64, u64) {
    let value = (lhs as u128) * (rhs as u128) + (accumulator as u128) + (carry as u128);
    (value as u64, (value >> 64) as u64)
}

#[inline(always)]
fn multiply_accumulate_2x2(accumulator: &mut [u64; 5], lhs: [u64; 2], rhs: [u64; 2]) {
    // Finish both schoolbook rows before propagating into the two high limbs.
    // This avoids walking the high accumulator once per row.
    let (word_0, carry_00) = mac(accumulator[0], lhs[0], rhs[0], 0);
    let (row_0_word_1, row_0_carry) = mac(accumulator[1], lhs[0], rhs[1], carry_00);
    let (word_1, carry_10) = mac(row_0_word_1, lhs[1], rhs[0], 0);

    let column_2 = (accumulator[2] as u128) + (row_0_carry as u128);
    let (word_2, carry_11) = mac(column_2 as u64, lhs[1], rhs[1], carry_10);
    let column_3 = (accumulator[3] as u128) + (carry_11 as u128) + (column_2 >> 64);
    let column_4 = (accumulator[4] as u128) + (column_3 >> 64);

    accumulator[0] = word_0;
    accumulator[1] = word_1;
    accumulator[2] = word_2;
    accumulator[3] = column_3 as u64;
    accumulator[4] = column_4 as u64;
    debug_assert_eq!(column_4 >> 64, 0, "five-limb delayed accumulator overflow");
}

#[inline(always)]
fn multiply_accumulate_2x1(accumulator: &mut [u64; 5], lhs: [u64; 2], rhs: u64) {
    let product_0 = (lhs[0] as u128) * (rhs as u128);
    let product_1 = (lhs[1] as u128) * (rhs as u128);

    let column_0 = (accumulator[0] as u128) + ((product_0 as u64) as u128);
    let column_1 = (accumulator[1] as u128)
        + (product_0 >> 64)
        + ((product_1 as u64) as u128)
        + (column_0 >> 64);
    let column_2 = (accumulator[2] as u128) + (product_1 >> 64) + (column_1 >> 64);
    let column_3 = (accumulator[3] as u128) + (column_2 >> 64);

    accumulator[0] = column_0 as u64;
    accumulator[1] = column_1 as u64;
    accumulator[2] = column_2 as u64;
    accumulator[3] = column_3 as u64;
    // Fewer than 2^64 products of a 128-bit value and a u64 fit in four
    // limbs, so the fifth limb remains unused for a valid linear accumulator.
    debug_assert_eq!(
        ((column_3 >> 64) as u64) | accumulator[4],
        0,
        "five-limb delayed accumulator overflow"
    );
}

#[inline(always)]
fn add_masked_2(accumulator: &mut [u64; 5], value: [u64; 2], mask: u64) {
    let column_0 = (accumulator[0] as u128) + ((value[0] & mask) as u128);
    let column_1 = (accumulator[1] as u128) + ((value[1] & mask) as u128) + (column_0 >> 64);
    let column_2 = (accumulator[2] as u128) + (column_1 >> 64);

    accumulator[0] = column_0 as u64;
    accumulator[1] = column_1 as u64;
    accumulator[2] = column_2 as u64;
    // Fewer than 2^64 selected 128-bit values fit in three limbs.
    debug_assert_eq!(
        ((column_2 >> 64) as u64) | accumulator[3] | accumulator[4],
        0,
        "five-limb delayed accumulator overflow"
    );
}

/// Folds `limbs[4] * 2^256` into four limbs via `2^256 mod q = R^2 mod q`.
#[inline(always)]
fn fold_fifth_limb(limbs: [u64; 5], r2: [u64; 2]) -> ([u64; 4], u64) {
    let high = limbs[4];
    let mut out = [limbs[0], limbs[1], limbs[2], limbs[3]];

    let (word0, carry) = mac(out[0], high, r2[0], 0);
    out[0] = word0;
    let (word1, carry) = mac(out[1], high, r2[1], carry);
    out[1] = word1;

    let sum = (out[2] as u128) + (carry as u128);
    out[2] = sum as u64;
    let sum = (out[3] as u128) + (sum >> 64);
    out[3] = sum as u64;
    (out, (sum >> 64) as u64)
}

/// Two fixed Montgomery-elimination rounds. The three-limb result is not
/// assumed canonical: for a 100-bit modulus it may be many multiples of q and
/// is routed through Barrett reduction by the caller.
#[inline(always)]
fn montgomery_reduce_4(input: [u64; 4], reducer: &OptimizedMonty128Reducer) -> [u64; 3] {
    let mut value = [input[0], input[1], input[2], input[3], 0u64];
    let mut i = 0;
    while i < 2 {
        let multiplier = value[i].wrapping_mul(reducer.montgomery_neg_inv);
        let (word0, carry) = mac(value[i], multiplier, reducer.modulus[0], 0);
        value[i] = word0;
        let (word1, carry) = mac(value[i + 1], multiplier, reducer.modulus[1], carry);
        value[i + 1] = word1;

        let mut propagation = carry;
        let mut k = i + 2;
        while k < 5 {
            let sum = (value[k] as u128) + (propagation as u128);
            value[k] = sum as u64;
            propagation = (sum >> 64) as u64;
            k += 1;
        }
        debug_assert_eq!(propagation, 0);
        i += 1;
    }

    [value[2], value[3], value[4]]
}

/// Fixed `k = 2` Barrett reduction for any four-limb input.
#[inline(always)]
fn barrett_reduce_4(input: [u64; 4], reducer: &OptimizedMonty128Reducer) -> [u64; 2] {
    // q1 = floor(input / b), q2 = q1 * mu, q3 = floor(q2 / b^3).
    let q1 = [input[1], input[2], input[3]];
    let q2 = multiply_3x3(q1, reducer.barrett_mu);
    let q3 = [q2[3], q2[4], q2[5]];

    // Only the low k+1 limbs are needed for the textbook Barrett remainder.
    let product = multiply_3x2_low3(q3, reducer.modulus);
    let mut remainder = wrapping_sub_3([input[0], input[1], input[2]], product);

    // For mu=floor(b^(2k)/q), the candidate is below 3q. Execute both
    // correction steps unconditionally and select with borrow masks.
    remainder = conditional_subtract_3(remainder, reducer.modulus);
    remainder = conditional_subtract_3(remainder, reducer.modulus);
    debug_assert_eq!(remainder[2], 0);
    debug_assert!(less_than_2([remainder[0], remainder[1]], reducer.modulus));
    [remainder[0], remainder[1]]
}

#[inline(always)]
fn multiply_3x3(lhs: [u64; 3], rhs: [u64; 3]) -> [u64; 6] {
    let mut out = [0u64; 6];
    let mut i = 0;
    while i < 3 {
        let mut carry = 0u64;
        let mut j = 0;
        while j < 3 {
            let (word, next_carry) = mac(out[i + j], lhs[i], rhs[j], carry);
            out[i + j] = word;
            carry = next_carry;
            j += 1;
        }
        let sum = (out[i + 3] as u128) + (carry as u128);
        out[i + 3] = sum as u64;
        let mut propagation = (sum >> 64) as u64;
        let mut k = i + 4;
        while k < 6 {
            let sum = (out[k] as u128) + (propagation as u128);
            out[k] = sum as u64;
            propagation = (sum >> 64) as u64;
            k += 1;
        }
        debug_assert_eq!(propagation, 0);
        i += 1;
    }
    out
}

#[inline(always)]
fn multiply_3x2_low3(lhs: [u64; 3], rhs: [u64; 2]) -> [u64; 3] {
    let mut out = [0u64; 3];
    let mut i = 0;
    while i < 3 {
        let mut carry = 0u64;
        let mut j = 0;
        while j < 2 && i + j < 3 {
            let (word, next_carry) = mac(out[i + j], lhs[i], rhs[j], carry);
            out[i + j] = word;
            carry = next_carry;
            j += 1;
        }
        if i + j < 3 {
            let sum = (out[i + j] as u128) + (carry as u128);
            out[i + j] = sum as u64;
        }
        i += 1;
    }
    out
}

#[inline(always)]
fn wrapping_sub_3(lhs: [u64; 3], rhs: [u64; 3]) -> [u64; 3] {
    let (word0, borrow0) = lhs[0].overflowing_sub(rhs[0]);
    let (word1, borrow1a) = lhs[1].overflowing_sub(rhs[1]);
    let (word1, borrow1b) = word1.overflowing_sub(u64::from(borrow0));
    let borrow1 = borrow1a | borrow1b;
    let (word2, _) = lhs[2].overflowing_sub(rhs[2]);
    let (word2, _) = word2.overflowing_sub(u64::from(borrow1));
    [word0, word1, word2]
}

#[inline(always)]
fn conditional_subtract_3(value: [u64; 3], modulus: [u64; 2]) -> [u64; 3] {
    let (word0, borrow0) = value[0].overflowing_sub(modulus[0]);
    let (word1, borrow1a) = value[1].overflowing_sub(modulus[1]);
    let (word1, borrow1b) = word1.overflowing_sub(u64::from(borrow0));
    let borrow1 = borrow1a | borrow1b;
    let (word2, borrow2) = value[2].overflowing_sub(u64::from(borrow1));

    // Select the difference iff the complete three-limb subtraction did not
    // borrow. `0 - bit` creates an all-zero/all-one mask without a branch.
    let select_difference = 0u64.wrapping_sub(u64::from(!borrow2));
    [
        (word0 & select_difference) | (value[0] & !select_difference),
        (word1 & select_difference) | (value[1] & !select_difference),
        (word2 & select_difference) | (value[2] & !select_difference),
    ]
}

#[inline(always)]
fn add_mod_masked(value: [u64; 2], addend: [u64; 2], bit: u64, modulus: [u64; 2]) -> [u64; 2] {
    let mask = 0u64.wrapping_sub(bit);
    let sum = (value[0] as u128) + ((addend[0] & mask) as u128);
    let low = sum as u64;
    let sum = (value[1] as u128) + ((addend[1] & mask) as u128) + (sum >> 64);
    let candidate = [low, sum as u64, (sum >> 64) as u64];
    let reduced = conditional_subtract_3(candidate, modulus);
    debug_assert_eq!(reduced[2], 0);
    [reduced[0], reduced[1]]
}

#[inline(always)]
fn less_than_2(lhs: [u64; 2], rhs: [u64; 2]) -> bool {
    lhs[1] < rhs[1] || (lhs[1] == rhs[1] && lhs[0] < rhs[0])
}

fn reference_remainder<B: Monty128ReductionBackend>(
    limbs: [u64; 5],
    reducer: &Monty128Reducer<B>,
) -> [u64; 2] {
    let value = CryptoUint::<5>::from_words(limbs);
    let modulus = NonZero::new(CryptoUint::<2>::from_words(reducer.modulus))
        .expect("prepared modulus is nonzero");
    words2(&value.rem(&modulus))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_bigint::{Odd, Uint as CryptoUint};
    use crypto_primes::{Flavor, is_prime};
    use crypto_primitives::{FromWithConfig, PrimeField};
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    type F = MontyField<2>;

    fn config(modulus: u128) -> crypto_bigint::modular::FixedMontyParams<2> {
        let words = [modulus as u64, (modulus >> 64) as u64];
        let modulus =
            Odd::new(CryptoUint::<2>::from_words(words)).expect("test modulus must be odd");
        crypto_bigint::modular::FixedMontyParams::new_vartime(modulus)
    }

    fn assert_backends(
        config: &crypto_bigint::modular::FixedMontyParams<2>,
        fields: &[(F, F, u64, bool)],
    ) {
        let optimized = OptimizedMonty128Reducer::new(config).unwrap();
        let reference = CryptoBigintMonty128Reducer::new(config).unwrap();

        let mut products = MontyProductAccumulator128::zero();
        let mut linear = MontyLinearAccumulator128::zero();
        let mut boolean = MontyLinearAccumulator128::zero();
        let mut expected_products = F::zero_with_cfg(config);
        let mut expected_linear = F::zero_with_cfg(config);
        let mut expected_boolean = F::zero_with_cfg(config);

        for (lhs, rhs, native, bit) in fields {
            products.multiply_accumulate(lhs, rhs);
            linear.multiply_accumulate(lhs, native);
            boolean.multiply_accumulate(lhs, bit);

            expected_products += lhs.clone() * rhs;
            expected_linear += lhs.clone() * F::from_with_cfg(*native, config);
            expected_boolean += lhs.clone() * F::from_with_cfg(*bit, config);
        }

        assert_eq!(products.reduce(&optimized).unwrap(), expected_products);
        assert_eq!(products.reduce(&reference).unwrap(), expected_products);
        assert_eq!(linear.reduce(&optimized).unwrap(), expected_linear);
        assert_eq!(linear.reduce(&reference).unwrap(), expected_linear);
        assert_eq!(boolean.reduce(&optimized).unwrap(), expected_boolean);
        assert_eq!(boolean.reduce(&reference).unwrap(), expected_boolean);
    }

    #[test]
    fn rejects_one_limb_moduli() {
        let config = config((1u128 << 64) - 59);
        assert_eq!(
            OptimizedMonty128Reducer::new(&config),
            Err(DelayedReductionError::UnsupportedModulus)
        );
    }

    #[test]
    fn accumulators_are_exactly_five_limbs() {
        assert_eq!(core::mem::size_of::<MontyProductAccumulator128>(), 5 * 8);
        assert_eq!(core::mem::size_of::<MontyLinearAccumulator128>(), 5 * 8);
    }

    #[test]
    fn zero_accumulators_reduce_to_zero() {
        let config = config((1u128 << 100) - 15);
        let optimized = OptimizedMonty128Reducer::new(&config).unwrap();
        let reference = CryptoBigintMonty128Reducer::new(&config).unwrap();
        let zero = F::zero_with_cfg(&config);

        assert_eq!(
            MontyProductAccumulator128::zero()
                .reduce(&optimized)
                .unwrap(),
            zero
        );
        assert_eq!(
            MontyProductAccumulator128::zero()
                .reduce(&reference)
                .unwrap(),
            zero
        );
        assert_eq!(
            MontyLinearAccumulator128::zero()
                .reduce(&optimized)
                .unwrap(),
            zero
        );
        assert_eq!(
            MontyLinearAccumulator128::zero()
                .reduce(&reference)
                .unwrap(),
            zero
        );
    }

    #[test]
    fn edge_values_match_immediate_arithmetic() {
        for modulus in [(1u128 << 100) - 15, u128::MAX - 158] {
            let config = config(modulus);
            let values = [0u128, 1, 2, modulus / 2, modulus - 2, modulus - 1];
            let mut cases = Vec::new();
            for (i, lhs) in values.into_iter().enumerate() {
                let rhs = values[values.len() - 1 - i];
                cases.push((
                    F::from_with_cfg(lhs, &config),
                    F::from_with_cfg(rhs, &config),
                    [0, 1, u64::MAX][i % 3],
                    i % 2 == 0,
                ));
            }
            assert_backends(&config, &cases);
        }
    }

    #[test]
    fn randomized_differential_reduction() {
        let mut rng = StdRng::seed_from_u64(0xD3_1A_7E_D0);
        for modulus in [(1u128 << 100) - 15, u128::MAX - 158] {
            let config = config(modulus);
            for length in [1usize, 2, 3, 17, 64, 257] {
                let cases = (0..length)
                    .map(|_| {
                        (
                            F::from_with_cfg(rng.random::<u128>(), &config),
                            F::from_with_cfg(rng.random::<u128>(), &config),
                            rng.random::<u64>(),
                            rng.random::<bool>(),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_backends(&config, &cases);
            }
        }
    }

    #[test]
    fn seeded_random_supported_primes_match_immediate_arithmetic() {
        let mut rng = StdRng::seed_from_u64(0x1280_0DDB_A223_7E57);
        let mut tested = 0;
        while tested < 8 {
            let bits = rng.random_range(100_u32..=128);
            let mask = if bits == 128 {
                u128::MAX
            } else {
                (1_u128 << bits) - 1
            };
            let modulus = (rng.random::<u128>() & mask) | (1_u128 << (bits - 1)) | 1;
            let candidate =
                CryptoUint::<2>::from_words([modulus as u64, (modulus >> u64::BITS) as u64]);
            if !is_prime(Flavor::Any, &candidate) {
                continue;
            }

            let config = config(modulus);
            let mut cases = vec![
                (
                    F::from_with_cfg(0_u128, &config),
                    F::from_with_cfg(modulus - 1, &config),
                    0,
                    false,
                ),
                (
                    F::from_with_cfg(modulus - 1, &config),
                    F::from_with_cfg(modulus - 1, &config),
                    u64::MAX,
                    true,
                ),
            ];
            cases.extend((0..31).map(|_| {
                (
                    F::from_with_cfg(rng.random::<u128>(), &config),
                    F::from_with_cfg(rng.random::<u128>(), &config),
                    rng.random::<u64>(),
                    rng.random::<bool>(),
                )
            }));
            assert_backends(&config, &cases);
            tested += 1;
        }
    }

    #[test]
    fn arbitrary_carry_heavy_limbs_match_reference_backend() {
        let mut rng = StdRng::seed_from_u64(0xCA_22_1E_5);
        for modulus in [(1u128 << 64) + 13, (1u128 << 100) - 15, u128::MAX - 158] {
            let config = config(modulus);
            let optimized = OptimizedMonty128Reducer::new(&config).unwrap();
            let reference = CryptoBigintMonty128Reducer::new(&config).unwrap();

            let mut cases = vec![[0; 5], [u64::MAX; 5]];
            cases.extend((0..10_000).map(|_| rng.random::<[u64; 5]>()));
            for limbs in cases {
                let product = MontyProductAccumulator128 { limbs };
                assert_eq!(
                    product.reduce(&optimized).unwrap(),
                    product.reduce(&reference).unwrap(),
                );

                let linear = MontyLinearAccumulator128 { limbs };
                assert_eq!(
                    linear.reduce(&optimized).unwrap(),
                    linear.reduce(&reference).unwrap(),
                );
            }
        }
    }

    #[test]
    fn merged_accumulators_match_single_accumulator() {
        let config = config((1u128 << 100) - 15);
        let optimized = OptimizedMonty128Reducer::new(&config).unwrap();
        let mut rng = StdRng::seed_from_u64(0xA11C_E55);
        let cases = (0..129)
            .map(|_| {
                (
                    F::from_with_cfg(rng.random::<u128>(), &config),
                    F::from_with_cfg(rng.random::<u128>(), &config),
                )
            })
            .collect::<Vec<_>>();

        let mut whole = MontyProductAccumulator128::zero();
        let mut left = MontyProductAccumulator128::zero();
        let mut right = MontyProductAccumulator128::zero();
        for (index, (lhs, rhs)) in cases.iter().enumerate() {
            whole.multiply_accumulate(lhs, rhs);
            if index < 61 {
                left.multiply_accumulate(lhs, rhs);
            } else {
                right.multiply_accumulate(lhs, rhs);
            }
        }
        left += right;
        assert_eq!(left, whole);
        assert_eq!(left.reduce(&optimized), whole.reduce(&optimized));
    }

    #[test]
    fn bool_accumulation_is_value_exact() {
        let config = config((1u128 << 100) - 15);
        let value = F::from_with_cfg(u128::MAX, &config);
        let optimized = OptimizedMonty128Reducer::new(&config).unwrap();
        let mut accumulator = MontyLinearAccumulator128::zero();
        accumulator.multiply_accumulate(&value, &false);
        accumulator.multiply_accumulate(&value, &true);
        assert_eq!(accumulator.reduce(&optimized).unwrap(), value);
    }
}
