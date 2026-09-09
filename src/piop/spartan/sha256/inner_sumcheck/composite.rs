//! Aligned repeated coefficients followed by a compact unrelated circuit tail.
use super::*;

pub(crate) struct CompositeCoefficients<'a> {
    repeated: Sha256FactoredBlockCoefficients<'a>,
    tail: &'a [Field],
    constant: Field,
}

impl<'a> CompositeCoefficients<'a> {
    pub fn new(
        high: &'a [Field],
        low: &'a [Field],
        tail: &'a [Field],
        constant: Field,
        cfg: &FieldConfig,
    ) -> Result<Self, SumcheckError> {
        let mut repeated =
            Sha256FactoredBlockCoefficients::new(Field::zero_with_cfg(cfg), high, low, cfg)?;
        repeated.start = 0;
        repeated.live_len -= 1;
        if !low.len().is_power_of_two() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        for x in tail.iter().chain([&constant]) {
            validate_field_value(x, cfg)?;
        }
        Ok(Self {
            repeated,
            tail,
            constant,
        })
    }
    fn live_len(&self) -> usize {
        self.repeated.live_len + self.tail.len()
    }
}

impl Sha256InnerCoefficientSource for CompositeCoefficients<'_> {
    fn coefficient_at(&self, i: usize) -> Result<Field, SumcheckError> {
        if i >= self.live_len() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let mut v = if i < self.repeated.live_len {
            self.repeated.coefficient(i)?
        } else {
            self.tail[i - self.repeated.live_len].clone()
        };
        if i == 0 {
            v += &self.constant;
        }
        Ok(v)
    }
    fn coefficients_prevalidated(&self) -> bool {
        true
    }
    fn validate_shape(&self, live_len: usize, cfg: &FieldConfig) -> Result<(), SumcheckError> {
        if live_len != self.live_len() || self.constant.cfg() != cfg {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }
    fn build_prefix_accumulators<const K: usize, H: Sha256InnerBitSource + ?Sized>(
        &self,
        num_vars: usize,
        live_len: usize,
        h: &H,
        cfg: &FieldConfig,
        zero: &Field,
        reducer: &OptimizedSumcheckReducer,
    ) -> Result<PrefixAccumulators, SumcheckError> {
        if self.repeated.block_coefficients.len() < 1 << K {
            return build_prefix_accumulators_generic::<K, _, _>(
                num_vars, live_len, self, h, cfg, zero, reducer,
            );
        }
        let mut result = build_factored_prefix_accumulators::<K, _>(
            num_vars,
            self.repeated.live_len,
            &self.repeated,
            h,
            cfg,
            zero,
            reducer,
        )?;
        let tail = build_prefix_accumulators_generic::<K, _, _>(
            num_vars,
            self.tail.len(),
            &|i: usize| Ok(self.tail[i].clone()),
            &|i: usize| h.bit_at(self.repeated.live_len + i),
            cfg,
            zero,
            reducer,
        )?;
        let constant = build_prefix_accumulators_generic::<K, _, _>(
            num_vars,
            1 << K,
            &|i: usize| {
                Ok(if i == 0 {
                    self.constant.clone()
                } else {
                    zero.clone()
                })
            },
            h,
            cfg,
            zero,
            reducer,
        )?;
        for other in [tail, constant] {
            for (dst, src) in result.rounds.iter_mut().zip(other.rounds) {
                for (dst, src) in dst.iter_mut().zip(src) {
                    dst[0] += &src[0];
                    dst[1] += &src[1];
                }
            }
        }
        Ok(result)
    }
    fn fold_prefix_table<const K: usize>(
        &self,
        num_vars: usize,
        live_len: usize,
        challenges: &[Field],
        cfg: &FieldConfig,
        zero: &Field,
        one: &Field,
        reducer: &OptimizedSumcheckReducer,
    ) -> Result<CompactPrefixVTable, SumcheckError> {
        let width = self.repeated.block_coefficients.len();
        let prefix = 1 << K;
        if width < prefix {
            return fold_prefix_v_table_generic::<K, _>(
                num_vars, live_len, self, challenges, cfg, zero, one, reducer,
            );
        }
        let weights = equality_weights_lsb(challenges, zero, one);
        // Fold the low factor just once. Every SHA local wire reuses it.
        let low: Vec<Field> = self
            .repeated
            .block_coefficients
            .chunks_exact(prefix)
            .map(|chunk| {
                let mut sum = product_accumulator_zero(reducer);
                for (a, b) in weights.iter().zip(chunk) {
                    product_multiply_accumulate(reducer, &mut sum, a, b);
                }
                product_reduce(reducer, sum)
            })
            .collect::<Result<_, _>>()?;
        let repeated_suffixes = self.repeated.live_len / prefix;
        let suffix_count = live_len.div_ceil(prefix);
        let mut table = CompactPrefixVTable {
            values: vec![raw_montgomery(zero); (suffix_count + 1) & !1],
            suffix_count,
        };
        let fill = |(suffix, out): (usize, &mut RawMontgomery)| -> Result<(), SumcheckError> {
            let mut value = if suffix < repeated_suffixes {
                low[suffix % low.len()].clone()
                    * &self.repeated.instance_weights[suffix / low.len()]
            } else {
                let start = (suffix - repeated_suffixes) * prefix;
                let mut sum = product_accumulator_zero(reducer);
                for (a, b) in weights
                    .iter()
                    .zip(&self.tail[start..self.tail.len().min(start + prefix)])
                {
                    product_multiply_accumulate(reducer, &mut sum, a, b);
                }
                product_reduce(reducer, sum)?
            };
            if suffix == 0 {
                value += &(weights[0].clone() * &self.constant);
            }
            *out = raw_montgomery(&value);
            Ok(())
        };
        #[cfg(feature = "parallel")]
        table.values[..suffix_count]
            .par_iter_mut()
            .enumerate()
            .try_for_each(fill)?;
        #[cfg(not(feature = "parallel"))]
        table.values[..suffix_count]
            .iter_mut()
            .enumerate()
            .try_for_each(fill)?;
        Ok(table)
    }
}

pub(crate) fn prove_composite_inner_sumcheck<T: Transcript, H: Sha256InnerBitSource + ?Sized>(
    transcript: &mut T,
    initial: Field,
    num_vars: usize,
    coefficients: &CompositeCoefficients<'_>,
    h: &H,
    prefix: usize,
    cfg: &FieldConfig,
    reducer: &OptimizedSumcheckReducer,
    grinding_bits: u32,
) -> Result<Sha256InnerSumcheckOutput, SumcheckError> {
    prove_sha256_inner_sumcheck_with_source(
        transcript,
        initial,
        num_vars,
        coefficients.live_len(),
        coefficients,
        h,
        prefix,
        cfg,
        reducer,
        grinding_bits,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;
    use crypto_primitives::FromWithConfig;

    #[test]
    fn composite_prefix_matches_generic_with_constant_tail_and_boundary() {
        let cfg = crate::piop::spartan::f2z::spartan_f2z_field_config();
        let reducer = OptimizedSumcheckReducer::new(&cfg).unwrap();
        let f = |x| Field::from_with_cfg(x, &cfg);
        for width in [8usize, 16, 32] {
            let high = [f(5u64), f(13), f(0)];
            let low: Vec<_> = (0..width).map(|i| f(i as u64 + 17)).collect();
            let tail: Vec<_> = (0..19).map(|i| f(i + 101)).collect();
            let coefficients = CompositeCoefficients::new(&high, &low, &tail, f(29), &cfg).unwrap();
            let live = coefficients.live_len();
            let num_vars = live.next_power_of_two().ilog2() as usize;
            let bit = |i| Ok(u64::from((i * 17 + 5) % 11 < 5));
            let mut claim = f(0);
            for i in 0..live {
                if bit(i).unwrap() != 0 {
                    claim += &coefficients.coefficient_at(i).unwrap();
                }
            }
            for prefix in 0..=4 {
                let actual = prove_composite_inner_sumcheck(
                    &mut Blake3Transcript::new(),
                    claim.clone(),
                    num_vars,
                    &coefficients,
                    &bit,
                    prefix,
                    &cfg,
                    &reducer,
                    0,
                )
                .unwrap();
                let expected = prove_sha256_inner_sumcheck(
                    &mut Blake3Transcript::new(),
                    claim.clone(),
                    num_vars,
                    live,
                    &|i| coefficients.coefficient_at(i),
                    &bit,
                    prefix,
                    &cfg,
                    &reducer,
                    0,
                )
                .unwrap();
                assert!(actual == expected, "width {width}, prefix {prefix}");
            }
        }
    }
}
