//! Borrowed MLEs with a tensor-product table and implicit zero padding.

use crate::piop::spartan::SpartanField;
use field::RingOps;

use crate::poly::EvaluationError;

/// Bit evaluations `[leading?, outer[0] * inner[..], outer[1] * inner[..], …]`.
/// The remaining entries of the `2^num_vars` domain are zero.
pub(crate) struct FactoredMultilinearExtension<'a, F: SpartanField> {
    field: F::Config,
    /// Domain size is `2^num_vars`, including zero padding.
    num_vars: usize,
    /// Zero, or one when a leading value precedes the tensor product.
    tensor_start: usize,
    /// Optional entry preceding the tensor product; zero when absent.
    leading_value: F,
    /// One multiplier per contiguous inner-factor block.
    outer_factor: &'a [F],
    /// Entries within each block.
    inner_factor: &'a [F],
    /// `tensor_start + outer_factor.len() * inner_factor.len()`.
    live_len: usize,
}

impl<'a, F: SpartanField> FactoredMultilinearExtension<'a, F> {
    pub(crate) fn from_factors(
        num_vars: usize,
        outer_factor: &'a [F],
        inner_factor: &'a [F],
        cfg: &F::Config,
    ) -> Result<Self, EvaluationError> {
        Self::from_parts(num_vars, None, outer_factor, inner_factor, cfg)
    }

    pub(crate) fn with_leading_value(
        num_vars: usize,
        leading_value: F,
        outer_factor: &'a [F],
        inner_factor: &'a [F],
        cfg: &F::Config,
    ) -> Result<Self, EvaluationError> {
        Self::from_parts(
            num_vars,
            Some(leading_value),
            outer_factor,
            inner_factor,
            cfg,
        )
    }

    pub(crate) fn num_vars(&self) -> usize {
        self.num_vars
    }
    pub(crate) fn live_len(&self) -> usize {
        self.live_len
    }
    pub(crate) fn tensor_start(&self) -> usize {
        self.tensor_start
    }
    pub(crate) fn leading_value(&self) -> &F {
        &self.leading_value
    }
    pub(crate) fn outer_factor(&self) -> &[F] {
        self.outer_factor
    }
    pub(crate) fn inner_factor(&self) -> &[F] {
        self.inner_factor
    }

    pub(crate) fn evaluation_at(&self, index: usize) -> Result<F, EvaluationError> {
        if index >= domain_size(self.num_vars)? {
            return Err(EvaluationError::InvalidShape);
        }
        if index >= self.live_len {
            return Ok(F::zero_with_cfg(&self.field));
        }
        if index < self.tensor_start {
            return Ok(self.leading_value.clone());
        }
        let offset = index - self.tensor_start;
        Ok(self.field.mul(
            &self.outer_factor[offset / self.inner_factor.len()],
            &self.inner_factor[offset % self.inner_factor.len()],
        ))
    }

    pub(crate) fn evaluate(&self, point: &[F], cfg: &F::Config) -> Result<F, EvaluationError> {
        check_point(self.num_vars, point)?;
        if self.tensor_start == 0 && self.inner_factor.len().is_power_of_two() {
            let split = self.inner_factor.len().ilog2() as usize;
            return Ok(cfg.mul(
                &dot_with_equality(self.inner_factor, &point[..split], cfg),
                &dot_with_equality(self.outer_factor, &point[split..], cfg),
            ));
        }
        let weights = EqualityWeights::new(point, cfg);
        let mut value = F::zero_with_cfg(cfg);
        if self.tensor_start != 0 {
            value = cfg.add(&value, &cfg.mul(&self.leading_value, &weights.at(0)));
        }
        for (outer_index, outer) in self.outer_factor.iter().enumerate() {
            for (inner_index, inner) in self.inner_factor.iter().enumerate() {
                let index = self.tensor_start + outer_index * self.inner_factor.len() + inner_index;
                value = cfg.add(&value, &cfg.mul(&cfg.mul(outer, inner), &weights.at(index)));
            }
        }
        Ok(value)
    }

    fn from_parts(
        num_vars: usize,
        leading: Option<F>,
        outer_factor: &'a [F],
        inner_factor: &'a [F],
        cfg: &F::Config,
    ) -> Result<Self, EvaluationError> {
        if outer_factor.is_empty() || inner_factor.is_empty() {
            return Err(EvaluationError::EmptyPolynomial);
        }
        let tensor_start = usize::from(leading.is_some());
        let live_len = outer_factor
            .len()
            .checked_mul(inner_factor.len())
            .and_then(|len| len.checked_add(tensor_start))
            .ok_or(EvaluationError::Overflow)?;
        if live_len > domain_size(num_vars)? {
            return Err(EvaluationError::InvalidShape);
        }
        Ok(Self {
            field: cfg.clone(),
            num_vars,
            tensor_start,
            leading_value: leading.unwrap_or_else(|| F::zero_with_cfg(cfg)),
            outer_factor,
            inner_factor,
            live_len,
        })
    }
}

fn domain_size(num_vars: usize) -> Result<usize, EvaluationError> {
    let shift = u32::try_from(num_vars).map_err(|_| EvaluationError::Overflow)?;
    1usize.checked_shl(shift).ok_or(EvaluationError::Overflow)
}

fn check_point<F>(num_vars: usize, point: &[F]) -> Result<(), EvaluationError> {
    if point.len() != num_vars {
        return Err(EvaluationError::WrongPointWidth {
            expected: num_vars,
            actual: point.len(),
        });
    }
    Ok(())
}

/// Equality weights use O(sqrt(domain)) storage, including for unaligned tails.
pub(crate) struct EqualityWeights<F: SpartanField> {
    field: F::Config,
    low: Vec<F>,
    high: Vec<F>,
}

impl<F: SpartanField> EqualityWeights<F> {
    fn new(point: &[F], cfg: &F::Config) -> Self {
        let split = point.len() / 2;
        Self::from_tables(
            equality_table(&point[..split], cfg),
            equality_table(&point[split..], cfg),
            cfg,
        )
    }
    pub(crate) fn from_tables(low: Vec<F>, high: Vec<F>, field: &F::Config) -> Self {
        assert!(low.len().is_power_of_two() && high.len().is_power_of_two());
        Self {
            low,
            high,
            field: field.clone(),
        }
    }
    pub(crate) fn low(&self) -> &[F] {
        &self.low
    }
    pub(crate) fn high(&self) -> &[F] {
        &self.high
    }
    pub(crate) fn at(&self, index: usize) -> F {
        self.field.mul(
            &self.low[index & (self.low.len() - 1)],
            &self.high[index >> self.low.len().ilog2()],
        )
    }
}

fn equality_table<F: SpartanField>(point: &[F], cfg: &F::Config) -> Vec<F> {
    let len = 1usize
        .checked_shl(point.len() as u32)
        .expect("checked equality table shape");
    let mut out = vec![cfg.zero(); len];
    out[0] = cfg.one();
    for (i, r) in point.iter().enumerate() {
        let half = 1usize << i;
        for j in 0..half {
            let hi = cfg.mul(&out[j], r);
            out[j + half] = hi;
            out[j] = cfg.sub(&out[j], &hi);
        }
    }
    out
}

fn dot_with_equality<F: SpartanField>(values: &[F], point: &[F], cfg: &F::Config) -> F {
    let weights = EqualityWeights::new(point, cfg);
    values
        .iter()
        .enumerate()
        .fold(F::zero_with_cfg(cfg), |mut sum, (index, value)| {
            sum = cfg.add(&sum, &cfg.mul(value, &weights.at(index)));
            sum
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poly::mle::DenseMultilinearExtension;
    use field::{Fp as Prime, Uint};
    type F128 = Prime<2>;

    fn evaluate_dense(
        mle: &DenseMultilinearExtension<F128>,
        point: &[F128],
        cfg: &field::FpCtx<2>,
    ) -> F128 {
        let mut values = mle.evaluations.clone();
        for coordinate in point {
            values = values
                .chunks_exact(2)
                .map(|pair| cfg.add(&pair[0], &cfg.mul(&cfg.sub(&pair[1], &pair[0]), coordinate)))
                .collect();
        }
        values.pop().unwrap()
    }

    #[test]
    fn structured_evaluations_match_dense_with_boundaries_and_padding() {
        let cfg = F128::make_cfg(&Uint::from(97u128)).unwrap();
        let f = |n| F128::from_with_cfg(n, &cfg);
        let high = [f(2u64), f(3), f(5)];
        for width in [1usize, 2, 3, 4] {
            let low: Vec<_> = (0..width).map(|n| f(n as u64 + 7)).collect();
            for leading in [None, Some(f(19))] {
                let mle =
                    FactoredMultilinearExtension::from_parts(5, leading.clone(), &high, &low, &cfg)
                        .unwrap();
                let mut values: Vec<_> = leading.into_iter().collect();
                values.extend(high.iter().flat_map(|a| low.iter().map(|b| cfg.mul(a, b))));
                let dense = DenseMultilinearExtension::from_evaluations_vec(5, values, f(0));
                for (index, expected) in dense.evaluations.iter().enumerate() {
                    assert_eq!(&mle.evaluation_at(index).unwrap(), expected);
                }
                for point in [
                    vec![f(0); 5],
                    vec![f(1); 5],
                    vec![f(11), f(13), f(17), f(23), f(29)],
                ] {
                    assert_eq!(
                        mle.evaluate(&point, &cfg).unwrap(),
                        evaluate_dense(&dense, &point, &cfg)
                    );
                }
                assert!(mle.evaluate(&[f(0)], &cfg).is_err());
                assert!(mle.evaluation_at(32).is_err());
            }
        }
        assert!(FactoredMultilinearExtension::from_factors(0, &high, &high, &cfg).is_err());
        assert!(
            FactoredMultilinearExtension::from_factors(usize::MAX, &high, &high, &cfg).is_err()
        );
        assert!(FactoredMultilinearExtension::from_factors(3, &[], &high, &cfg).is_err());
    }
}
