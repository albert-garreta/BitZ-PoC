use super::*;
use crate::sumcheck::{UngrindedRoundBoundary, inner::prove_inner_sumcheck};
use crate::transcript::{Blake3Transcript, traits::Transcript};

struct IntegerSource {
    values: Vec<Field>,
    partition: usize,
}

impl StreamingCoefficientSource for IntegerSource {
    fn num_vars(&self) -> usize {
        self.values.len().ilog2() as usize
    }
    fn live_len(&self) -> usize {
        self.values.len()
    }
    fn partition_len(&self) -> usize {
        self.partition
    }
    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        for (i, value) in self.values.iter().enumerate() {
            emit(i, *value)?;
        }
        Ok(())
    }
    fn for_each_partition(
        &self,
        partition: usize,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        let start = partition * self.partition;
        for (i, value) in self.values[start..start + self.partition]
            .iter()
            .enumerate()
        {
            emit(start + i, *value)?;
        }
        Ok(())
    }
    fn for_each_partition_block(
        &self,
        partition: usize,
        width: usize,
        emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let start = partition * self.partition;
        Some((|| {
            for (i, block) in self.values[start..start + self.partition]
                .chunks_exact(width)
                .enumerate()
            {
                emit(start + width * i, block)?;
            }
            Ok(())
        })())
    }
}

fn fixture(
    row_vars: usize,
    column_vars: usize,
    f: &FieldConfig,
) -> (IntegerSource, Vec<Field>, Vec<Field>, Vec<u64>) {
    let mut row = (0..1 << row_vars)
        .map(|i| Field::from_with_cfg(43 * i as u64 + 19, f))
        .collect::<Vec<_>>();
    // Include an inactive, padded row even when the table is not truncated.
    if row.len() > 1 {
        *row.last_mut().unwrap() = f.zero();
    }
    let column = (0..1 << column_vars)
        .map(|i| {
            if column_vars != 0 && i % 13 < 4 {
                f.zero()
            } else {
                f.neg(&Field::from_with_cfg(31 * i as u64 + 3, f))
            }
        })
        .collect::<Vec<_>>();
    let len = row.len() * column.len();
    let mut values = Vec::with_capacity(len);
    let mut bits = vec![0u64; len.div_ceil(64)];
    for i in 0..len {
        let active = row.len() == 1 || i / column.len() + 1 < row.len();
        values.push(if active {
            Field::from_with_cfg((i as u64 * 17 + 5) % 251, f)
        } else {
            f.zero()
        });
        let bit = u64::from(active)
            * ((i as u64 * 0x9e37_79b9 ^ (i as u64 >> 2)).count_ones() as u64 & 1);
        bits[i / 64] |= bit << (i % 64);
    }
    (
        IntegerSource {
            values,
            partition: column.len(),
        },
        row,
        column,
        bits,
    )
}

fn dense_oracle(row_vars: usize, column_vars: usize, prefix: usize, scale: Field, f: &FieldConfig) {
    let (source, row, column, bits) = fixture(row_vars, column_vars, f);
    assert_dense_oracle(
        &source,
        &source.values,
        &row,
        &column,
        &bits,
        prefix,
        scale,
        f,
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_dense_oracle(
    source: &impl StreamingCoefficientSource,
    integer_values: &[Field],
    row: &[Field],
    column: &[Field],
    bits: &[u64],
    prefix: usize,
    scale: Field,
    f: &FieldConfig,
) {
    let n = source.num_vars();
    let coefficients = integer_values
        .iter()
        .enumerate()
        .map(|(i, z)| {
            f.add(
                &f.mul(&row[i / column.len()], &column[i % column.len()]),
                &f.mul(&scale, z),
            )
        })
        .collect::<Vec<_>>();
    let h = (0..integer_values.len())
        .map(|i| {
            if bits[i / 64] >> (i % 64) & 1 == 1 {
                f.one()
            } else {
                f.zero()
            }
        })
        .collect::<Vec<_>>();
    let claim = coefficients
        .iter()
        .zip(&h)
        .fold(f.zero(), |sum, (a, b)| f.add(&sum, &f.mul(a, b)));
    let mut actual_transcript = Blake3Transcript::new();
    let actual = prove_inner_sumcheck(
        f,
        &mut actual_transcript,
        claim,
        FactoredOverlayInput::new(
            source,
            bits,
            row,
            column,
            scale,
            n,
            integer_values.len(),
            prefix,
        ),
        (),
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    let mut expected_transcript = Blake3Transcript::new();
    let expected = prove_inner_sumcheck(
        f,
        &mut expected_transcript,
        claim,
        h,
        coefficients,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    assert_eq!(
        actual,
        expected,
        "rows={}, columns={}, prefix={prefix}",
        row.len(),
        column.len()
    );
    assert_eq!(
        actual_transcript.get_challenge::<u128>(),
        expected_transcript.get_challenge::<u128>()
    );
}

#[test]
fn factored_overlay_matches_dense_proof_and_transcript() {
    let f = crate::piop::spartan::u32_mul_relation::spartan_bitz_field_config();
    for (row_vars, column_vars) in [(0, 0), (3, 0), (0, 1), (0, 4), (1, 4), (3, 5), (2, 9)] {
        for prefix in 0..=column_vars.min(4) {
            for scale in [f.zero(), f.one(), f.neg(&Field::from_with_cfg(137u64, &f))] {
                dense_oracle(row_vars, column_vars, prefix, scale, &f);
            }
        }
    }
}

#[test]
fn factored_overlay_byte_buckets_and_task_boundaries_match_dense() {
    let f = crate::prime_sampling::sample_prime_context(
        &mut Blake3Transcript::new(),
        1u128 << 125,
        (1u128 << 126) - 1,
        128,
    )
    .unwrap();
    // Activates byte buckets (column >= 4096) and multiple tail tasks per row.
    for prefix in [3, 4] {
        dense_oracle(2, 12, prefix, f.neg(&Field::from_with_cfg(97u64, &f)), &f);
    }
}

#[test]
fn factored_overlay_keeps_integer_coefficients_unscaled_and_storage_compact() {
    use crate::sumcheck::inner::input::{Input, State as _};
    let f = crate::piop::spartan::u32_mul_relation::spartan_bitz_field_config();
    let (source, row, column, bits) = fixture(2, 6, &f);
    let mut state = FactoredOverlayInput::new(
        &source,
        &bits,
        &row,
        &column,
        Field::from_with_cfg(71u64, &f),
        8,
        256,
        3,
    )
    .prepare(&f, ())
    .unwrap();
    let point = [f.zero(), f.one(), Field::from_with_cfg(29u64, &f)];
    for challenge in point {
        state.fold(&f, &challenge).unwrap();
    }
    let State::K3(state) = state else {
        unreachable!()
    };
    let expected = fold_prefix_v_table_generic::<3, _>(
        8,
        256,
        &|i| Ok(source.values[i]),
        &point,
        &f,
        &f.zero(),
        &f.one(),
    )
    .unwrap();
    assert_eq!(state.table.as_ref().unwrap().values, expected.values);
    assert_eq!(state.column.len(), column.len() / 8);
    assert_eq!(state.row, row);
    // No tensor-sized allocation accompanies the single compact integer table.
    assert_eq!(state.table.unwrap().values.len(), 256 / 8);
}

#[test]
fn factored_overlay_rejects_invalid_shape_and_claim() {
    use crate::sumcheck::inner::input::Input;
    let f = crate::piop::spartan::u32_mul_relation::spartan_bitz_field_config();
    let (source, row, column, bits) = fixture(2, 4, &f);
    for (rows, cols, n, live, prefix) in [
        (&row[..3], column.as_slice(), 6, 64, 3),
        (row.as_slice(), &column[..15], 6, 64, 3),
        (row.as_slice(), column.as_slice(), 5, 64, 3),
        (row.as_slice(), column.as_slice(), 6, 63, 3),
        (row.as_slice(), column.as_slice(), 6, 64, 5),
    ] {
        assert!(
            FactoredOverlayInput::new(&source, &bits, rows, cols, f.one(), n, live, prefix)
                .prepare(&f, ())
                .is_err()
        );
    }
    assert!(
        prove_inner_sumcheck(
            &f,
            &mut Blake3Transcript::new(),
            f.one(),
            FactoredOverlayInput::new(&source, &bits, &row, &column, f.one(), 6, 64, 3),
            (),
            &mut UngrindedRoundBoundary
        )
        .is_err()
    );
}

/// Exercise the source adapters used by the Falcon integer binder, including
/// sparse final coefficients and overlapping, out-of-order additive updates.
struct FoldedSource<'a> {
    source: &'a IntegerSource,
    f: &'a FieldConfig,
    additive: bool,
}

impl FoldedSource<'_> {
    fn replay(
        &self,
        partition: usize,
        weights: &[Field],
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        let start = partition * self.source.partition;
        let mut entries = Vec::new();
        for (i, block) in self.source.values[start..start + self.source.partition]
            .chunks_exact(weights.len())
            .enumerate()
        {
            let value = block
                .iter()
                .zip(weights)
                .fold(self.f.zero(), |sum, (a, b)| {
                    self.f.add(&sum, &self.f.mul(a, b))
                });
            if value != self.f.zero() {
                entries.push((start / weights.len() + i, value));
            }
        }
        if self.additive {
            // Each final value is 2*z + (-z), in reverse index order.
            for (index, value) in entries.into_iter().rev() {
                emit(index, self.f.add(&value, &value))?;
                emit(index, self.f.neg(&value))?;
            }
        } else {
            for (index, value) in entries {
                emit(index, value)?;
            }
        }
        Ok(())
    }
}

impl StreamingCoefficientSource for FoldedSource<'_> {
    fn num_vars(&self) -> usize {
        self.source.num_vars()
    }
    fn live_len(&self) -> usize {
        self.source.live_len()
    }
    fn partition_len(&self) -> usize {
        self.source.partition_len()
    }
    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        self.source.for_each_coefficient(emit)
    }
    fn for_each_partition(
        &self,
        partition: usize,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        self.source.for_each_partition(partition, emit)
    }
    fn for_each_partition_folded(
        &self,
        partition: usize,
        weights: &[Field],
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        self.additive.then(|| self.replay(partition, weights, emit))
    }
    fn for_each_partition_folded_final(
        &self,
        partition: usize,
        weights: &[Field],
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        (!self.additive).then(|| self.replay(partition, weights, emit))
    }
}

#[test]
fn factored_overlay_folded_replays_preserve_sparse_ring_proof_and_transcript() {
    let f = crate::prime_sampling::sample_prime_context(
        &mut Blake3Transcript::new(),
        1u128 << 125,
        (1u128 << 126) - 1,
        128,
    )
    .unwrap();
    let (mut source, row, column, bits) = fixture(2, 8, &f);
    for (index, value) in source.values.iter_mut().enumerate() {
        // Missing coefficient pairs must retain their nonzero ring terms.
        if (index / 32) % 3 == 0 {
            *value = f.zero();
        } else if index % 2 == 0 {
            *value = f.neg(value);
        }
    }
    for additive in [false, true] {
        let folded = FoldedSource {
            source: &source,
            f: &f,
            additive,
        };
        for prefix in [0, 3, 4] {
            for scale in [f.zero(), f.one(), f.neg(&Field::from_with_cfg(137u64, &f))] {
                assert_dense_oracle(
                    &folded,
                    &source.values,
                    &row,
                    &column,
                    &bits,
                    prefix,
                    scale,
                    &f,
                );
            }
        }
    }
}
