//! Collapse a local relation once; callers keep its instance factor separate.
use super::BindingError;
use field::RingOps;
pub(crate) fn collapse_signed_columns(
    relation: &crate::sparse_matrix::SparseMatrix<i64>,
    weights: &[field::Fp<2>],
    field: &field::FpCtx<2>,
) -> Result<Vec<field::Fp<2>>, BindingError> {
    if weights.len() < relation.row_count() {
        return Err(BindingError::InvalidProductDimensions);
    }
    let mut out = field.zero_vec(relation.column_count());
    circuit::linear_map::contraction::signed_native_into(
        field,
        relation,
        weights,
        &mut out,
        cfg!(feature = "parallel"),
    );
    Ok(out)
}
