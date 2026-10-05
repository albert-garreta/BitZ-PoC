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
    assert_matches_dense(&source, &row, &column, &bits, prefix, scale, f);
}

fn assert_matches_dense(
    source: &IntegerSource,
    row: &[Field],
    column: &[Field],
    bits: &[u64],
    prefix: usize,
    scale: Field,
    f: &FieldConfig,
) {
    let n = source.num_vars();
    let coefficients = source
        .values
        .iter()
        .enumerate()
        .map(|(i, z)| {
            f.add(
                &f.mul(&row[i / column.len()], &column[i % column.len()]),
                &f.mul(&scale, z),
            )
        })
        .collect::<Vec<_>>();
    let h = (0..source.values.len())
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
    let expected_challenge = expected_transcript.get_challenge::<u128>();
    for contraction in [None, Some(false), Some(true)] {
        let mut input = FactoredOverlayInput::new(
            source,
            bits,
            row,
            column,
            scale,
            n,
            source.values.len(),
            prefix,
        );
        if let Some(enabled) = contraction {
            input = input.with_ring_contraction_for_test(enabled);
        }
        let mut transcript = Blake3Transcript::new();
        let actual = prove_inner_sumcheck(
            f,
            &mut transcript,
            claim,
            input,
            (),
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(
            actual,
            expected,
            "rows={}, columns={}, prefix={prefix}, contraction={contraction:?}",
            row.len(),
            column.len()
        );
        assert_eq!(transcript.get_challenge::<u128>(), expected_challenge);
    }
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
    dense_oracle(2, 12, 3, f.neg(&Field::from_with_cfg(97u64, &f)), &f);
}

#[test]
fn factored_overlay_contraction_matches_dense_across_blocks_and_tiles() {
    let f = crate::prime_sampling::sample_prime_context(
        &mut Blake3Transcript::new(),
        1u128 << 125,
        (1u128 << 126) - 1,
        128,
    )
    .unwrap();
    // Includes one remaining column round, multiple row blocks (>64 rows),
    // and multiple 256-suffix contraction tiles. Fixtures include a padded row.
    for (row_vars, column_vars) in [(0, 4), (1, 4), (2, 5), (3, 4), (7, 5), (3, 12)] {
        dense_oracle(row_vars, column_vars, 3, f.neg(&f.one()), &f);
    }
    for pattern in [0u64, u64::MAX, 0x0011_aa55_00ff_ffff] {
        let (mut source, mut row, mut column, mut bits) = fixture(3, 5, &f);
        for (i, z) in source.values.iter_mut().enumerate() {
            *z = f.neg(&Field::from_with_cfg(i as u64 + 1, &f));
        }
        row.fill(f.neg(&f.one()));
        row[3] = f.zero();
        column.fill(f.neg(&f.one()));
        bits.fill(pattern);
        // A zero ring weight need not imply zero bits or integer coefficients.
        for scale in [f.zero(), f.one(), f.neg(&f.one())] {
            assert_matches_dense(&source, &row, &column, &bits, 3, scale, &f);
        }
        column.fill(f.zero());
        assert_matches_dense(&source, &row, &column, &bits, 3, f.one(), &f);
    }
}

#[test]
fn factored_overlay_contraction_state_walk_preserves_column_to_row_transition() {
    let f = crate::piop::spartan::u32_mul_relation::spartan_bitz_field_config();
    for (row_vars, column_vars) in [(0, 4), (1, 4), (3, 4), (7, 5), (3, 12)] {
        let (source, row, column, bits) = fixture(row_vars, column_vars, &f);
        for schedule in 0..3 {
            let input = |enabled| {
                FactoredOverlayInput::new(
                    &source,
                    &bits,
                    &row,
                    &column,
                    f.neg(&f.one()),
                    row_vars + column_vars,
                    source.values.len(),
                    3,
                )
                .with_ring_contraction_for_test(enabled)
            };
            let mut plain = OverlayState::<_, _, 3>::new(input(false), &f).unwrap();
            let mut contracted = OverlayState::<_, _, 3>::new(input(true), &f).unwrap();
            for round in 0..row_vars + column_vars {
                assert_eq!(
                    plain.coefficients(&f).unwrap(),
                    contracted.coefficients(&f).unwrap(),
                    "rows={row_vars}, columns={column_vars}, schedule={schedule}, round={round}"
                );
                let r = match schedule {
                    0 => f.zero(),
                    1 => f.one(),
                    _ => [f.zero(), f.one(), f.neg(&Field::from_with_cfg(37u64, &f))][round % 3],
                };
                plain.fold(&f, &r).unwrap();
                contracted.fold(&f, &r).unwrap();
                assert_eq!(plain.row, contracted.row);
                assert_eq!(plain.column, contracted.column);
                if let (Some(a), Some(b)) = (&plain.table, &contracted.table) {
                    // The optimization changes only the ring round sums;
                    // integer coefficients and the one shared bit fold agree.
                    assert_eq!(a.values, b.values);
                    assert_eq!(a.suffix_count, b.suffix_count);
                }
                if round + 1 >= 3 && round + 1 < column_vars {
                    assert_eq!(
                        contracted.contracted_bits.as_ref().unwrap().len(),
                        column.len() >> (round + 1)
                    );
                } else {
                    assert!(contracted.contracted_bits.is_none());
                }
            }
            assert_eq!(
                plain.terminal(&f).unwrap(),
                contracted.terminal(&f).unwrap()
            );
        }
    }
}

#[test]
fn factored_overlay_weighted_byte_lookup_matches_direct_field_products() {
    let f = crate::prime_sampling::sample_prime_context(
        &mut Blake3Transcript::new(),
        1u128 << 125,
        (1u128 << 126) - 1,
        128,
    )
    .unwrap();
    for point in [
        [f.zero(); 3],
        [f.one(); 3],
        [
            f.neg(&f.one()),
            Field::from_with_cfg(31u64, &f),
            f.neg(&Field::from_with_cfg(17u64, &f)),
        ],
    ] {
        let weights = equality_weights_lsb(&point, &f.zero(), &f.one(), &f);
        for row in [f.zero(), f.one(), f.neg(&f.one())] {
            let lookup = weighted_byte_sums(&row, &weights, &f);
            for (pattern, actual) in lookup.iter().enumerate() {
                let sum = weights
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| pattern >> i & 1 == 1)
                    .fold(f.zero(), |sum, (_, weight)| f.add(&sum, weight));
                assert_eq!(*actual, f.mul(&row, &sum));
            }
        }
    }
}

#[test]
fn factored_overlay_contraction_rejects_non_byte_source_values() {
    struct InvalidByte;
    impl Sha256InnerBitSource for InvalidByte {
        fn bit_at(&self, _: usize) -> Result<u64, SumcheckError> {
            Ok(0)
        }
        fn bits_at(&self, _: usize, _: usize) -> Result<u64, SumcheckError> {
            Ok(256)
        }
    }
    let f = crate::piop::spartan::u32_mul_relation::spartan_bitz_field_config();
    let prefix = PreparedPrefixWeights::new(vec![f.one(); 8], &f);
    assert!(
        contract_rows_and_sum_integer(
            &vec![raw_montgomery(&f.one()); 4],
            &[f.one(); 2],
            2,
            &InvalidByte,
            &prefix,
            &f,
        )
        .is_err()
    );
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
        for contraction in [false, true] {
            assert!(
                FactoredOverlayInput::new(&source, &bits, rows, cols, f.one(), n, live, prefix)
                    .with_ring_contraction_for_test(contraction)
                    .prepare(&f, ())
                    .is_err()
            );
        }
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
