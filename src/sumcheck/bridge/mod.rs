//! Matrix-MLE contraction, shared by the outer/inner bridge and terminal checks.
//! These operations never touch a transcript or derive a claim from a witness.
use field::RingOps;
pub(crate) mod native;
pub(crate) mod repeated;
pub(crate) type BindingError = crate::sumcheck::SumcheckError;

/// Prepared arithmetic and reusable workspace; every result owns its values.
pub(crate) trait PreparedBinding<F: RingOps, R: ?Sized> {
    type Bound;
    fn bind_rows(&mut self, rows: &R) -> Result<Self::Bound, BindingError>;
    fn bind_rows_into(&mut self, rows: &R, out: &mut Self::Bound) -> Result<(), BindingError>;
    /// Direct bilinear evaluation. Must not construct the column coefficient table.
    fn evaluate_bound(
        &mut self,
        rows: &R,
        column_point: &[F::Elem],
    ) -> Result<F::Elem, BindingError>;
}
#[cfg(feature = "ecdsa")]
pub(crate) mod composite;
pub(crate) mod dense;
#[cfg(test)]
mod tests;
