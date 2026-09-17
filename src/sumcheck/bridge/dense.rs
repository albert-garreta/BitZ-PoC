//! Generic field-valued CSC binding; native binding shares the column scheduler.
use crate::{
    piop::spartan::{
        SpartanField,
        matrix::{PreparedConstraintMatrices, SpartanMatrixCoefficient, SpartanMatrixError},
    },
    poly::mle::DenseMultilinearExtension,
};
use field::RingOps;
pub(crate) fn bind_rows<F: SpartanField, C: SpartanMatrixCoefficient<F>>(
    prepared: &PreparedConstraintMatrices<F, C>,
    rows: &[F],
    rho: &F,
) -> Result<DenseMultilinearExtension<F>, SpartanMatrixError> {
    debug_assert_eq!(rows.len(), 1 << prepared.num_row_vars());
    let field = prepared.config();
    let zero = field.zero();
    let rho_squared = field.mul(rho, rho);
    let m = prepared.matrices();
    let mut values = field.zero_vec(1 << prepared.num_column_vars());
    circuit::linear_map::contraction::columns_into(
        &mut values[..m.column_count()],
        cfg!(feature = "parallel"),
        |j| {
            let mut sum = C::column_dot(m.a().column(j).unwrap(), rows, &zero, field);
            for (matrix, scale) in [(m.b(), rho), (m.c(), &rho_squared)] {
                let column = matrix.column(j).unwrap();
                if !column.is_empty() {
                    sum = field.add(
                        &sum,
                        &field.mul(scale, &C::column_dot(column, rows, &zero, field)),
                    );
                }
            }
            sum
        },
    );
    Ok(DenseMultilinearExtension {
        evaluations: values,
        num_vars: prepared.num_column_vars(),
    })
}

/// Explicit-row bridge for any shared-library mixed MAC implementation.
impl<F, C> super::PreparedBinding<F, [F::Elem]>
    for circuit::linear_map::contraction::PreparedSparse<'_, F, C>
where
    F: field::RingOps
        + field::BatchMulAcc<F::Elem, C>
        + field::Reduce<<F as field::BatchMulAcc<F::Elem, C>>::Accumulator, Output = F::Elem>
        + Sync,
    C: Sync,
{
    type Bound = Vec<F::Elem>;
    fn bind_rows(&mut self, rows: &[F::Elem]) -> Result<Self::Bound, super::BindingError> {
        let mut out = Vec::new();
        self.bind_rows_into(rows, &mut out)?;
        Ok(out)
    }
    fn bind_rows_into(
        &mut self,
        rows: &[F::Elem],
        out: &mut Self::Bound,
    ) -> Result<(), super::BindingError> {
        if rows.len() != self.row_count() {
            return Err(super::BindingError::InvalidProductDimensions);
        }
        out.resize(self.column_count(), self.field().zero());
        self.adjoint_into(rows, out, cfg!(feature = "parallel"))
            .map_err(|_| super::BindingError::InvalidProductDimensions)
    }
    fn evaluate_bound(
        &mut self,
        rows: &[F::Elem],
        point: &[F::Elem],
    ) -> Result<F::Elem, super::BindingError> {
        let domain = 1usize
            .checked_shl(point.len() as u32)
            .ok_or(super::BindingError::InvalidProductDimensions)?;
        if domain < self.column_count() {
            return Err(super::BindingError::InvalidProductDimensions);
        }
        let columns = EqualityColumns::new(self.field(), point, self.column_count());
        self.evaluate_bilinear(rows, &columns)
            .map_err(|_| super::BindingError::InvalidProductDimensions)
    }
}
pub(super) struct EqualityColumns<'a, F: RingOps> {
    field: &'a F,
    low: Vec<F::Elem>,
    high: Vec<F::Elem>,
    len: usize,
}
impl<'a, F: RingOps> EqualityColumns<'a, F> {
    pub(super) fn new(field: &'a F, point: &[F::Elem], len: usize) -> Self {
        let table = |p: &[F::Elem]| {
            let mut w = field.zero_vec(1 << p.len());
            w[0] = field.one();
            for (bit, r) in p.iter().enumerate() {
                for i in 0..1 << bit {
                    let t = field.mul(&w[i], r);
                    w[i] = field.sub(&w[i], &t);
                    w[i + (1 << bit)] = t;
                }
            }
            w
        };
        let split = point.len() / 2;
        Self {
            field,
            low: table(&point[..split]),
            high: table(&point[split..]),
            len,
        }
    }
}
impl<F: RingOps + Sync> circuit::linear_map::ColumnValues<F::Elem> for EqualityColumns<'_, F> {
    fn len(&self) -> usize {
        self.len
    }
    fn scalar(&self, i: usize) -> F::Elem {
        self.field.mul(
            &self.low[i % self.low.len()],
            &self.high[i / self.low.len()],
        )
    }
    fn power_sum(&self, first: usize, len: usize) -> F::Elem {
        (first..first + len).rev().fold(self.field.zero(), |s, i| {
            self.field.add(&self.field.add(&s, &s), &self.scalar(i))
        })
    }
}
