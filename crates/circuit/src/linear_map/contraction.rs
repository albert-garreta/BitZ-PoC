//! CSC contraction scheduling and monomorphized arithmetic kernels.
use super::{SparseColumn, SparseMatrix};
use field::{BatchMulAcc, MergeAccumulator, Reduce, RingOps};
use rayon::prelude::*;

/// Disjoint contiguous column ranges; sparse backends retain control of each
/// dot product, so implicit ones and singleton columns never require a MAC.
pub fn columns_into<E: Send>(out: &mut [E], parallel: bool, evaluate: impl Fn(usize) -> E + Sync) {
    const BLOCK: usize = 1 << 12;
    if parallel && out.len() >= BLOCK && rayon::current_num_threads() > 1 {
        out.par_chunks_mut(BLOCK)
            .enumerate()
            .for_each(|(b, chunk)| {
                for (i, dst) in chunk.iter_mut().enumerate() {
                    *dst = evaluate(b * BLOCK + i);
                }
            });
    } else {
        for (i, dst) in out.iter_mut().enumerate() {
            *dst = evaluate(i);
        }
    }
}

/// Σ_i c_i w[row_i], preserving the caller's coefficient-specific fast paths.
#[inline]
pub fn column_dot<C, E: Copy>(
    column: SparseColumn<'_, C>,
    zero: E,
    mut scale: impl FnMut(usize, &C) -> E,
    mut add: impl FnMut(E, E) -> E,
) -> E {
    let mut entries = column.into_iter();
    let Some((row, c)) = entries.next() else {
        return zero;
    };
    let mut sum = scale(row, c);
    for (row, c) in entries {
        sum = add(sum, scale(row, c));
    }
    sum
}

/// Signed native contraction. The public maximum column length bounds every
/// accumulator; workers own separate columns and never merge accumulators.
/// Signs act on field weights, so i64::MIN is handled without signed overflow.
pub fn signed_native_into<F>(
    field: &F,
    matrix: &SparseMatrix<i64>,
    weights: &[F::Elem],
    out: &mut [F::Elem],
    parallel: bool,
) where
    F: RingOps
        + BatchMulAcc<F::Elem, u64>
        + Reduce<<F as BatchMulAcc<F::Elem, u64>>::Accumulator, Output = F::Elem>
        + Sync,
{
    assert!(weights.len() >= matrix.row_count());
    assert_eq!(out.len(), matrix.column_count());
    let max_terms = matrix
        .column_offsets()
        .windows(2)
        .map(|w| w[1] - w[0])
        .max()
        .unwrap_or(0);
    let reduce = field.prepare_reduce(max_terms.max(1));
    columns_into(out, parallel, |j| {
        let column = matrix.column(j).unwrap();
        if let Some((row, c)) = column.single() {
            match *c {
                1 => return weights[row],
                -1 => return field.neg(&weights[row]),
                _ => {}
            }
        }
        let mut acc = <F as BatchMulAcc<F::Elem, u64>>::Accumulator::zero();
        for (row, c) in column {
            let weight = if *c < 0 {
                field.neg(&weights[row])
            } else {
                weights[row]
            };
            field.mul_acc(&mut acc, &weight, &c.unsigned_abs());
        }
        reduce(acc)
    });
}

/// A field provider plus a validated CSC map. Native coefficients retain their
/// declared type; field coefficients use the field×field accumulator. No entry
/// is projected or allocated in a column traversal.
pub struct PreparedSparse<'a, F: RingOps, C> {
    field: &'a F,
    matrix: &'a SparseMatrix<C>,
}
impl<'a, F: RingOps, C> PreparedSparse<'a, F, C> {
    pub fn new(field: &'a F, matrix: &'a SparseMatrix<C>) -> Self {
        Self { field, matrix }
    }
    pub fn field(&self) -> &'a F {
        self.field
    }
    pub fn row_count(&self) -> usize {
        self.matrix.row_count()
    }
    pub fn column_count(&self) -> usize {
        self.matrix.column_count()
    }
}
impl<F, C> PreparedSparse<'_, F, C>
where
    F: RingOps
        + BatchMulAcc<F::Elem, C>
        + Reduce<<F as BatchMulAcc<F::Elem, C>>::Accumulator, Output = F::Elem>
        + Sync,
    C: Sync,
{
    pub fn adjoint_into(
        &self,
        weights: &[F::Elem],
        out: &mut [F::Elem],
        parallel: bool,
    ) -> Result<(), super::LinearMapError> {
        self.check(weights.len(), out.len())?;
        let max_terms = self
            .matrix
            .column_offsets()
            .windows(2)
            .map(|w| w[1] - w[0])
            .max()
            .unwrap_or(0);
        let reduce = self.field.prepare_reduce(max_terms.max(1));
        columns_into(out, parallel, |j| {
            let mut acc = <F as BatchMulAcc<F::Elem, C>>::Accumulator::zero();
            for (row, c) in self.matrix.column(j).unwrap() {
                self.field.mul_acc(&mut acc, &weights[row], c);
            }
            reduce(acc)
        });
        Ok(())
    }
    pub fn evaluate_bilinear(
        &self,
        weights: &[F::Elem],
        columns: &impl super::ColumnValues<F::Elem>,
    ) -> Result<F::Elem, super::LinearMapError> {
        self.check(weights.len(), columns.len())?;
        let max_terms = self
            .matrix
            .column_offsets()
            .windows(2)
            .map(|w| w[1] - w[0])
            .max()
            .unwrap_or(0);
        let reduce = self.field.prepare_reduce(max_terms.max(1));
        let mut total = self.field.zero();
        for (j, column) in self.matrix.columns().enumerate() {
            let mut acc = <F as BatchMulAcc<F::Elem, C>>::Accumulator::zero();
            for (row, c) in column {
                self.field.mul_acc(&mut acc, &weights[row], c);
            }
            total = self
                .field
                .add(&total, &self.field.mul(&reduce(acc), &columns.scalar(j)));
        }
        Ok(total)
    }
    fn check(&self, rows: usize, cols: usize) -> Result<(), super::LinearMapError> {
        for (kind, expected, actual) in [
            ("row weights", self.row_count(), rows),
            ("column values", self.column_count(), cols),
        ] {
            if expected != actual {
                return Err(super::LinearMapError::Length {
                    kind,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    }
}
