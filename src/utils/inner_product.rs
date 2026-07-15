use crate::utils::{from_ref::FromRef, mul_by_scalar::MulByScalar};
use crypto_primitives::{FromWithConfig, PrimeField, boolean::Boolean};
use num_traits::CheckedAdd;
use thiserror::Error;

/// A trait for inner product algorithms implementations.
pub trait InnerProduct<Lhs: ?Sized, Rhs, Output> {
    /// The main entry point for the inner product.
    /// `CHECK` determines whether the implementation should check for overflow.
    fn inner_product<const CHECK: bool>(
        lhs: &Lhs,
        rhs: &[Rhs],
        zero: Output,
    ) -> Result<Output, InnerProductError>;
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum InnerProductError {
    #[error("The length of LHS and RHS does not match: LHS={lhs}, RHS={rhs}")]
    LengthMismatch { lhs: usize, rhs: usize },
    #[error("Arithmetic overflow")]
    Overflow,
}

/// An implementation of inner product that piggies back
/// on the `MulByScalar` and `CheckedAdd` traits.
/// It does `mul_by_scalar` for products of terms
/// and then combines the results using either `add` or `checked_add`.
#[derive(Clone, Debug)]
pub struct MBSInnerProduct;

impl<Lhs, Rhs, Out> InnerProduct<[Lhs], Rhs, Out> for MBSInnerProduct
where
    Out: FromRef<Lhs> + for<'a> MulByScalar<&'a Rhs> + CheckedAdd,
{
    /// The mul-by-scalar inner product.
    #[allow(clippy::arithmetic_side_effects)] // Used in unchecked mode
    fn inner_product<const CHECK: bool>(
        lhs: &[Lhs],
        rhs: &[Rhs],
        zero: Out,
    ) -> Result<Out, InnerProductError> {
        if lhs.len() != rhs.len() {
            return Err(InnerProductError::LengthMismatch {
                lhs: lhs.len(),
                rhs: rhs.len(),
            });
        }

        lhs.iter().zip(rhs).try_fold(zero, |acc, (l, r)| {
            let widened = Out::from_ref(l);
            let product = widened
                .mul_by_scalar::<CHECK>(r)
                .ok_or(InnerProductError::Overflow)?;
            if CHECK {
                acc.checked_add(&product).ok_or(InnerProductError::Overflow)
            } else {
                Ok(acc + product)
            }
        })
    }
}

impl MBSInnerProduct {
    #[allow(clippy::arithmetic_side_effects)]
    pub fn inner_product_field<Lhs, F>(
        lhs: &[Lhs],
        rhs: &[F],
        zero: F,
    ) -> Result<F, InnerProductError>
    where
        F: PrimeField + for<'a> FromWithConfig<&'a Lhs>,
    {
        if lhs.len() != rhs.len() {
            return Err(InnerProductError::LengthMismatch {
                lhs: lhs.len(),
                rhs: rhs.len(),
            });
        }
        let cfg = zero.cfg().clone();

        Ok(lhs.iter().zip(rhs).fold(zero, |acc, (a, r)| {
            let product: F = F::from_with_cfg(a, &cfg) * r;
            acc + product
        }))
    }
}

/// The inner product for vectors of length 1 (a.k.a. scalars).
/// Uses `mul_by_scalar` to multiply the only components of vectors
/// to get the result.
#[derive(Clone, Debug)]
pub struct ScalarProduct;

impl<Lhs, Rhs, Out> InnerProduct<Lhs, Rhs, Out> for ScalarProduct
where
    Out: for<'a> MulByScalar<&'a Rhs> + FromRef<Lhs>,
{
    /// A scalar inner product. Assumes `Lhs` is a scalar type
    /// and always asserts that `point` has only one component.
    fn inner_product<const CHECK: bool>(
        lhs: &Lhs,
        point: &[Rhs],
        _zero: Out,
    ) -> Result<Out, InnerProductError> {
        if point.as_ref().len() != 1 {
            Err(InnerProductError::LengthMismatch {
                lhs: 1,
                rhs: point.as_ref().len(),
            })
        } else {
            Ok(Out::from_ref(lhs)
                .mul_by_scalar::<CHECK>(&point[0])
                .ok_or(InnerProductError::Overflow)?)
        }
    }
}

/// The inner product for slices containing `Boolean` elements.
/// Uses `add` or `checked_add` to sum the elements of the RHS that
/// correspond to `true` elements of the boolean slice.
pub struct BooleanInnerProductAdd;

impl<Rhs: Clone, Out: FromRef<Rhs> + CheckedAdd> InnerProduct<[Boolean], Rhs, Out>
    for BooleanInnerProductAdd
{
    /// Boolean inner product.
    #[allow(clippy::arithmetic_side_effects)] // Used in unchecked mode
    fn inner_product<const CHECK: bool>(
        lhs: &[Boolean],
        rhs: &[Rhs],
        zero: Out,
    ) -> Result<Out, InnerProductError> {
        if lhs.len() != rhs.as_ref().len() {
            return Err(InnerProductError::LengthMismatch {
                lhs: lhs.len(),
                rhs: rhs.as_ref().len(),
            });
        }

        (0..lhs.len())
            .filter(|&i| lhs[i].into_inner())
            .try_fold(zero, |acc, i| {
                let rhs = Out::from_ref(&rhs[i]);
                if CHECK {
                    acc.checked_add(&rhs).ok_or(InnerProductError::Overflow)
                } else {
                    Ok(acc + rhs)
                }
            })
    }
}
