// Streaming sum of the projected ring tensor and the optimized integer binder.
// Public ring digit coefficients are arbitrary after canonical coordinate lifting.
use super::*;
use crate::sumcheck::SumcheckError;

pub(super) struct JoinedBinding<'a> {
    pub(super) integer: BindingForm<'a>,
    pub(super) ring: Option<super::super::shared_ring::ProjectedClaim>,
    pub(super) integer_scale: F,
    folded_column: OnceLock<(Vec<F>, Vec<F>)>,
}

impl<'a> JoinedBinding<'a> {
    pub(super) fn new(
        integer: BindingForm<'a>,
        ring: Option<super::super::shared_ring::ProjectedClaim>,
        integer_scale: F,
    ) -> Result<Self, FalconError> {
        if let Some(ring) = &ring {
            if ring.row.len() != integer.layout.capacity()
                || ring.column.len() != integer.layout.signature_stride()
                || ring.row[integer.layout.batch()..]
                    .iter()
                    .any(|v| *v != integer.field.zero())
            {
                return Err(piop("projected ring tensor dimensions or padding"));
            }
        }
        Ok(Self {
            integer,
            ring,
            integer_scale,
            folded_column: OnceLock::new(),
        })
    }

    pub(super) fn target(&self) -> Result<F, FalconError> {
        let target = self.integer.target()?;
        Ok(match &self.ring {
            None => target,
            Some(r) => self.integer.field.add(
                &r.target,
                &self.integer.field.mul(&self.integer_scale, &target),
            ),
        })
    }

    pub(super) fn evaluate(&self, point: &[F]) -> Result<F, FalconError> {
        let value = self.integer.evaluate(point)?;
        let Some(ring) = &self.ring else {
            return Ok(value);
        };
        let field = self.integer.field;
        let split = self.partition_len().ilog2() as usize;
        let local = eq_table(&point[..split], field).map_err(|e| piop(e.to_string()))?;
        let instances = eq_table(&point[split..], field).map_err(|e| piop(e.to_string()))?;
        let dot = |a: &[F], b: &[F]| {
            a.iter()
                .zip(b)
                .fold(field.zero(), |s, (x, y)| field.add(&s, &field.mul(x, y)))
        };
        Ok(field.add(
            &field.mul(&dot(&ring.column, &local), &dot(&ring.row, &instances)),
            &field.mul(&self.integer_scale, &value),
        ))
    }

    fn check_partition(&self, partition: usize) -> Result<bool, SumcheckError> {
        if partition >= self.integer.layout.capacity() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(partition < self.integer.layout.batch())
    }

    fn folded_ring(&self, weights: &[F]) -> Result<&[F], SumcheckError> {
        if !weights.len().is_power_of_two() || weights.len() > self.partition_len() {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let field = self.integer.field;
        let cached = self.folded_column.get_or_init(|| {
            let values = self
                .ring
                .as_ref()
                .expect("ring overlay")
                .column
                .chunks_exact(weights.len())
                .map(|chunk| {
                    chunk
                        .iter()
                        .zip(weights)
                        .fold(field.zero(), |s, (x, y)| field.add(&s, &field.mul(x, y)))
                })
                .collect();
            (weights.to_vec(), values)
        });
        if cached.0 != weights {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(&cached.1)
    }
}

impl StreamingCoefficientSource for JoinedBinding<'_> {
    fn num_vars(&self) -> usize {
        self.integer.num_vars()
    }
    fn live_len(&self) -> usize {
        self.integer.live_len()
    }
    fn partition_len(&self) -> usize {
        self.integer.partition_len()
    }

    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        for s in 0..self.integer.layout.batch() {
            self.for_each_partition(s, emit)?;
        }
        Ok(())
    }

    fn for_each_partition(
        &self,
        s: usize,
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        let Some(ring) = &self.ring else {
            return self.integer.for_each_partition(s, emit);
        };
        if !self.check_partition(s)? {
            return Ok(());
        }
        let field = self.integer.field;
        self.integer.for_each_partition(s, &mut |i, value| {
            emit(i, field.mul(&self.integer_scale, &value))
        })?;
        for (h, value) in ring.column.iter().enumerate() {
            if *value != field.zero() {
                emit(s * self.partition_len() + h, field.mul(&ring.row[s], value))?;
            }
        }
        Ok(())
    }

    fn for_each_partition_block(
        &self,
        s: usize,
        width: usize,
        emit: &mut impl FnMut(usize, &[F]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let Some(ring) = &self.ring else {
            return self.integer.for_each_partition_block(s, width, emit);
        };
        Some((|| {
            if !self.check_partition(s)? {
                return Ok(());
            }
            if !width.is_power_of_two()
                || width > 1 << crate::sumcheck::inner::packed::SHA256_INNER_PREFIX_MAX_VARS
            {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            let field = self.integer.field;
            let base = s * self.partition_len();
            let mut next = base;
            let mut block = [field.zero(); 64];
            let mut output = |index: usize, integer: Option<&[F]>| -> Result<(), SumcheckError> {
                for lane in 0..width {
                    block[lane] = field.mul(&ring.row[s], &ring.column[index - base + lane]);
                    if let Some(values) = integer {
                        block[lane] =
                            field.add(&block[lane], &field.mul(&self.integer_scale, &values[lane]));
                    }
                }
                if block[..width].iter().any(|v| *v != field.zero()) {
                    emit(index, &block[..width])?;
                }
                Ok(())
            };
            self.integer
                .for_each_partition_block(s, width, &mut |index, values| {
                    while next < index {
                        output(next, None)?;
                        next += width;
                    }
                    output(index, Some(values))?;
                    next = index + width;
                    Ok(())
                })
                .ok_or(SumcheckError::InvalidProductDimensions)??;
            while next < base + self.partition_len() {
                output(next, None)?;
                next += width;
            }
            Ok(())
        })())
    }

    fn for_each_partition_folded_final(
        &self,
        s: usize,
        weights: &[F],
        emit: &mut impl FnMut(usize, F) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let Some(ring) = &self.ring else {
            return self
                .integer
                .for_each_partition_folded_final(s, weights, emit);
        };
        Some((|| {
            if !self.check_partition(s)? {
                return Ok(());
            }
            let column = self.folded_ring(weights)?;
            let field = self.integer.field;
            let base = s * column.len();
            let mut next = base;
            // Both coefficients are public. At most two products enter each
            // accumulator; the shared 126-bit modulus permits one REDC.
            let reduce = field.prepare_product_reduction(2);
            let mut output = |index: usize, integer: F| -> Result<(), SumcheckError> {
                let ring_column = column[index - base];
                let value = if ring_column == field.zero() {
                    if integer == field.zero() {
                        return Ok(());
                    }
                    field.mul(&self.integer_scale, &integer)
                } else if integer == field.zero() {
                    field.mul(&ring.row[s], &ring_column)
                } else {
                    let mut sum = field::FpProductAcc::<2>::default();
                    sum.accumulate(&ring.row[s], &ring_column);
                    sum.accumulate(&self.integer_scale, &integer);
                    reduce.reduce(sum)
                };
                if value != field.zero() {
                    emit(index, value)?;
                }
                Ok(())
            };
            self.integer
                .for_each_partition_folded_final(s, weights, &mut |index, value| {
                    while next < index {
                        output(next, field.zero())?;
                        next += 1;
                    }
                    output(index, value)?;
                    next = index + 1;
                    Ok(())
                })
                .ok_or(SumcheckError::InvalidProductDimensions)??;
            while next < base + column.len() {
                output(next, field.zero())?;
                next += 1;
            }
            Ok(())
        })())
    }

    fn for_each_partition_byte_bucket(
        &self,
        s: usize,
        read_byte: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
        emit: &mut impl FnMut(u8, &[F; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let Some(ring) = &self.ring else {
            return self
                .integer
                .for_each_partition_byte_bucket(s, read_byte, emit);
        };
        Some((|| {
            if !self.check_partition(s)? {
                return Ok(());
            }
            let field = self.integer.field;
            let mut integer = vec![[field.zero(); 8]; 256];
            self.integer
                .for_each_partition_byte_bucket(s, read_byte, &mut |bucket, values| {
                    integer[usize::from(bucket)] = *values;
                    Ok(())
                })
                .ok_or(SumcheckError::InvalidProductDimensions)??;
            let mut columns = vec![[field.zero(); 8]; 256];
            for (byte, values) in ring.column.chunks_exact(8).enumerate() {
                if values.iter().all(|v| *v == field.zero()) {
                    continue;
                }
                let bucket = usize::from(read_byte(s * self.partition_len() + 8 * byte, 8)?);
                if bucket == 0 {
                    continue;
                }
                for (sum, value) in columns[bucket].iter_mut().zip(values) {
                    *sum = field.add(sum, value);
                }
            }
            for bucket in 1..256 {
                for lane in 0..8 {
                    integer[bucket][lane] = field.add(
                        &field.mul(&self.integer_scale, &integer[bucket][lane]),
                        &field.mul(&ring.row[s], &columns[bucket][lane]),
                    );
                }
                if integer[bucket].iter().any(|v| *v != field.zero()) {
                    emit(bucket as u8, &integer[bucket])?;
                }
            }
            Ok(())
        })())
    }
}
