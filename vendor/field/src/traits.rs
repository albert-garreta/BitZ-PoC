//! Operation providers: the element type belongs to the provider type.

use crate::{
    ct::{CtEq, CtSelect, CtValue},
    integer::Uint,
};

pub trait RingOps {
    type Elem: Copy + Send + Sync + CtEq + CtSelect;
    fn zero(&self) -> Self::Elem;
    fn one(&self) -> Self::Elem;
    fn add(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
    fn sub(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
    fn neg(&self, a: &Self::Elem) -> Self::Elem;
    fn mul(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
    fn square(&self, a: &Self::Elem) -> Self::Elem {
        self.mul(a, a)
    }

    /// Scan the complete public exponent width, including leading zeroes.
    fn pow_ct<const E: usize>(&self, base: &Self::Elem, exponent: &Uint<E>) -> Self::Elem {
        let mut result = self.one();
        for i in (0..E * 64).rev() {
            result = self.square(&result);
            let product = self.mul(&result, base);
            result = Self::Elem::ct_select(&result, &product, exponent.bit(i).mask());
        }
        result
    }
}

pub trait FieldOps: RingOps {
    fn inverse_ct(&self, a: &Self::Elem) -> CtValue<Self::Elem>;
    fn div_ct(&self, numerator: &Self::Elem, denominator: &Self::Elem) -> CtValue<Self::Elem> {
        self.inverse_ct(denominator)
            .map(|inverse| self.mul(numerator, &inverse))
    }
}

// Borrowing a context preserves its associated element type and specialized
// scalar implementation. Prepared operations can retain `&field` directly.
impl<C: RingOps + ?Sized> RingOps for &C {
    type Elem = C::Elem;
    fn zero(&self) -> Self::Elem {
        (**self).zero()
    }
    fn one(&self) -> Self::Elem {
        (**self).one()
    }
    fn add(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem {
        (**self).add(a, b)
    }
    fn sub(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem {
        (**self).sub(a, b)
    }
    fn neg(&self, a: &Self::Elem) -> Self::Elem {
        (**self).neg(a)
    }
    fn mul(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem {
        (**self).mul(a, b)
    }
    fn square(&self, a: &Self::Elem) -> Self::Elem {
        (**self).square(a)
    }
    fn pow_ct<const E: usize>(&self, a: &Self::Elem, e: &Uint<E>) -> Self::Elem {
        (**self).pow_ct(a, e)
    }
}
impl<C: FieldOps + ?Sized> FieldOps for &C {
    fn inverse_ct(&self, a: &Self::Elem) -> CtValue<Self::Elem> {
        (**self).inverse_ct(a)
    }
}
impl<C: IntegerEmbedding<T> + ?Sized, T> IntegerEmbedding<T> for &C {
    fn from_integer(&self, value: &T) -> Self::Elem {
        (**self).from_integer(value)
    }
}

pub trait IntegerEmbedding<T>: RingOps {
    fn from_integer(&self, value: &T) -> Self::Elem;
}
pub trait FieldEmbedding<T>: FieldOps {
    fn embed(&self, value: &T) -> Self::Elem;
}
pub trait WideMul<Lhs, Rhs = Lhs> {
    type Product;
    fn mul_wide(&self, lhs: &Lhs, rhs: &Rhs) -> Self::Product;
}
pub trait BatchMulAcc<Lhs, Rhs = Lhs> {
    type Accumulator;
    fn batch_mul_acc(&self, lhs: &[Lhs], rhs: &[Rhs]) -> Self::Accumulator;
    /// Invoke `term` exactly once for each index, in ascending order.
    fn batch_mul_acc_map(
        &self,
        len: usize,
        term: impl FnMut(usize) -> (Lhs, Rhs),
    ) -> Self::Accumulator;
}
/// Exact merge under the documented total-term bound. No count metadata is stored.
pub trait MergeAccumulator: Sized {
    fn zero() -> Self;
    fn merge_assign(&mut self, rhs: &Self);
}
pub trait Reduce<Input> {
    type Output;
    fn reduce(&self, input: Input) -> Self::Output;
}

pub trait BatchFieldOps: FieldOps {
    fn batch_mul_into(&self, lhs: &[Self::Elem], rhs: &[Self::Elem], out: &mut [Self::Elem]) {
        assert_eq!(lhs.len(), rhs.len(), "batch input lengths differ");
        assert_eq!(lhs.len(), out.len(), "batch output length differs");
        for ((a, b), dst) in lhs.iter().zip(rhs).zip(out) {
            *dst = self.mul(a, b);
        }
    }
    fn batch_mul(&self, lhs: &[Self::Elem], rhs: &[Self::Elem]) -> Vec<Self::Elem> {
        let mut out = vec![self.zero(); lhs.len()];
        self.batch_mul_into(lhs, rhs, &mut out);
        out
    }
    /// Scratch must have at least `input.len()` entries. Zero entries stay zero.
    fn batch_invert_or_zero_ct_into(
        &self,
        input: &[Self::Elem],
        out: &mut [Self::Elem],
        scratch: &mut [Self::Elem],
    ) {
        assert_eq!(input.len(), out.len(), "batch output length differs");
        assert!(
            scratch.len() >= input.len(),
            "insufficient inversion scratch"
        );
        if input.is_empty() {
            return;
        }
        let one = self.one();
        let zero = self.zero();
        let mut product = one;
        for (a, prefix) in input.iter().zip(scratch.iter_mut()) {
            *prefix = product;
            let nonzero = Self::Elem::ct_select(a, &one, a.ct_is_zero());
            product = self.mul(&product, &nonzero);
        }
        // Replacing all zero inputs by one makes the product invertible.
        let mut inverse = *self.inverse_ct(&product).value();
        for i in (0..input.len()).rev() {
            let is_zero = input[i].ct_is_zero();
            let value = self.mul(&inverse, &scratch[i]);
            out[i] = Self::Elem::ct_select(&value, &zero, is_zero);
            let factor = Self::Elem::ct_select(&input[i], &one, is_zero);
            inverse = self.mul(&inverse, &factor);
        }
    }
    fn batch_invert_or_zero_ct(&self, input: &[Self::Elem]) -> Vec<Self::Elem> {
        let mut out = vec![self.zero(); input.len()];
        let mut scratch = vec![self.zero(); input.len()];
        self.batch_invert_or_zero_ct_into(input, &mut out, &mut scratch);
        out
    }
}

pub trait CheckedArithmetic: Sized {
    fn checked_add_ct(&self, rhs: &Self) -> CtValue<Self>;
    fn checked_sub_ct(&self, rhs: &Self) -> CtValue<Self>;
    fn checked_mul_ct(&self, rhs: &Self) -> CtValue<Self>;
    fn checked_neg_ct(&self) -> CtValue<Self>;
    fn checked_div_rem_ct(&self, rhs: &Self) -> CtValue<(Self, Self)>;
}
pub trait WrappingArithmetic: Sized {
    fn wrapping_add(&self, rhs: &Self) -> Self;
    fn wrapping_sub(&self, rhs: &Self) -> Self;
    fn wrapping_mul(&self, rhs: &Self) -> Self;
    fn wrapping_neg(&self) -> Self;
}

pub trait FoldPairs<Src, Dst>: RingOps {
    fn fold_pairs_into(
        &self,
        src: &[Src],
        pair_offset: usize,
        dst: &mut [Dst],
        challenge: &Self::Elem,
    );
    fn fold_pairs_map_into(
        &self,
        source_len: usize,
        pair_offset: usize,
        read: impl FnMut(usize) -> Src,
        dst: &mut [Dst],
        challenge: &Self::Elem,
    );
}
