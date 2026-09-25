//! Additive coefficient streams for a packed inner sumcheck. The stream is
//! replayed once for the native prefix and once for its folded suffix table.
use super::*;

/// Deterministic additive updates to a multilinear coefficient table.
///
/// Replays must emit the same sum at every index. Entries can overlap and arrive
/// in any order; omitted entries, including domain padding, have coefficient zero.
pub(crate) trait StreamingCoefficientSource: Sync {
    fn num_vars(&self) -> usize;
    fn live_len(&self) -> usize;
    fn for_each_coefficient(
        &self,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError>;
}

/// A streaming source deliberately has no random-access coefficient operation.
/// Its two specialized prefix operations avoid replaying the source per index.
pub(crate) struct StreamingMle<'a, S: ?Sized> {
    source: &'a S,
}

impl<'a, S: StreamingCoefficientSource + ?Sized> StreamingMle<'a, S> {
    pub(crate) fn new(source: &'a S) -> Self {
        Self { source }
    }

    fn validate_dimensions<const K: usize>(
        &self,
        num_vars: usize,
        live_len: usize,
    ) -> Result<(), SumcheckError> {
        if K > SHA256_INNER_PREFIX_MAX_VARS
            || K > num_vars
            || num_vars >= usize::BITS as usize
            || num_vars != self.source.num_vars()
            || live_len != self.source.live_len()
            || live_len == 0
            || live_len > 1usize << num_vars
        {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }

    fn visit_checked(
        &self,
        cfg: &FieldConfig,
        mut emit: impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        use field::CtOrd;
        self.source.for_each_coefficient(&mut |index, value| {
            if index >= self.source.live_len() {
                return Err(SumcheckError::InvalidProductDimensions);
            }
            if !value
                .as_montgomery_integer()
                .ct_lt(cfg.modulus())
                .declassify()
            {
                return Err(SumcheckError::NonCanonicalFieldElement);
            }
            emit(index, value)
        })
    }
}

impl<S: StreamingCoefficientSource + ?Sized> InnerSumcheckMleSource for StreamingMle<'_, S> {
    fn declared_num_vars(&self) -> Option<usize> {
        Some(self.source.num_vars())
    }

    fn evaluation_at(&self, _index: usize) -> Result<Field, SumcheckError> {
        Err(SumcheckError::InvalidProductDimensions)
    }

    fn validate_shape(&self, live_len: usize, _cfg: &FieldConfig) -> Result<(), SumcheckError> {
        self.validate_dimensions::<0>(self.source.num_vars(), live_len)
    }

    fn build_prefix_accumulators<const K: usize, H: Sha256InnerBitSource + ?Sized>(
        &self,
        num_vars: usize,
        live_len: usize,
        bits: &H,
        cfg: &FieldConfig,
        zero: &Field,
    ) -> Result<PrefixAccumulators, SumcheckError> {
        self.validate_dimensions::<K>(num_vars, live_len)?;
        if K == 0 {
            return Ok(PrefixAccumulators::new::<K>(zero));
        }
        // A four-way bounded cache combines nearby scatter updates before the
        // ternary prefix extension. Evictions remain exact because every prefix
        // accumulator is linear in the coefficient table for a fixed witness.
        // At K=4 the coefficient cache occupies at most one MiB, regardless of N.
        const SETS: usize = 1024;
        const WAYS: usize = 4;
        let width = 1usize << K;
        let blocks = live_len.div_ceil(width);
        let sets = blocks.next_power_of_two().min(SETS);
        let mut keys = vec![usize::MAX; sets * WAYS];
        let mut replace = vec![0usize; sets];
        let mut values = vec![*zero; keys.len() * width];
        let mut state = PrefixBuildState::new::<K>(zero);
        self.visit_checked(cfg, |index, delta| {
            let block = index >> K;
            let set = block & (sets - 1);
            let start = set * WAYS;
            let slot = match keys[start..start + WAYS]
                .iter()
                .position(|&key| key == block)
            {
                Some(way) => start + way,
                None => {
                    let slot = start + replace[set];
                    replace[set] = (replace[set] + 1) % WAYS;
                    let slice = &mut values[slot * width..(slot + 1) * width];
                    if keys[slot] != usize::MAX {
                        accumulate_block::<K, _>(
                            &mut state, keys[slot], slice, bits, live_len, cfg, zero,
                        )?;
                        slice.fill(*zero);
                    }
                    keys[slot] = block;
                    slot
                }
            };
            let value = &mut values[slot * width + (index & (width - 1))];
            *value = cfg.add(value, &delta);
            Ok(())
        })?;
        for (slot, block) in keys.into_iter().enumerate() {
            if block != usize::MAX {
                accumulate_block::<K, _>(
                    &mut state,
                    block,
                    &values[slot * width..(slot + 1) * width],
                    bits,
                    live_len,
                    cfg,
                    zero,
                )?;
            }
        }
        let beta = state
            .partial_sums
            .into_iter()
            .map(|sum| linear_reduce(sum, cfg))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(scatter_beta_values::<K>(&beta, zero, cfg))
    }

    fn fold_prefix_table<const K: usize>(
        &self,
        num_vars: usize,
        live_len: usize,
        challenges: &[Field],
        cfg: &FieldConfig,
        zero: &Field,
        one: &Field,
    ) -> Result<CompactPrefixVTable, SumcheckError> {
        self.validate_dimensions::<K>(num_vars, live_len)?;
        if challenges.len() != K {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        validate_field_values(challenges, cfg)?;
        let weights = equality_weights_lsb(challenges, zero, one, cfg);
        let suffix_count = live_len.div_ceil(1usize << K);
        let mut table = CompactPrefixVTable {
            values: vec![raw_montgomery(zero); (suffix_count + 1) & !1],
            suffix_count,
        };
        self.visit_checked(cfg, |index, delta| {
            let slot = &mut table.values[index >> K];
            let term = cfg.mul(&delta, &weights[index & ((1usize << K) - 1)]);
            *slot = raw_montgomery(&cfg.add(&field_from_raw(slot, cfg), &term));
            Ok(())
        })?;
        Ok(table)
    }
}

fn accumulate_block<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    state: &mut PrefixBuildState,
    block: usize,
    values: &[Field],
    bits: &H,
    live_len: usize,
    cfg: &FieldConfig,
    zero: &Field,
) -> Result<(), SumcheckError> {
    let width = 1usize << K;
    let base = block * width;
    let active = width.min(live_len - base);
    let word = bits.bits_at(base, active)?;
    if word & !low_bits_mask(active) != 0 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    state.v_values.clear();
    state.v_values.extend_from_slice(values);
    state.h_values.clear();
    state
        .h_values
        .extend((0..width).map(|i| ((word >> i) & 1) as i64));
    extend_lsb::<Field, K, _>(&mut state.v_values, &mut state.v_scratch, zero, |hi, lo| {
        cfg.sub(hi, lo)
    });
    extend_lsb::<i64, K, _>(&mut state.h_values, &mut state.h_scratch, &0, |hi, lo| {
        hi - lo
    });
    for beta in 0..pow3(K) {
        if state.h_values[beta] != 0 {
            linear_multiply_accumulate_signed(
                cfg,
                &mut state.partial_sums[beta],
                &state.v_values[beta],
                state.h_values[beta],
                zero,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::piop::spartan::bitz::spartan_bitz_field_config;
    use crate::sumcheck::UngrindedRoundBoundary;
    use crate::sumcheck::inner::{prove_batched_inner_sumcheck, prove_inner_sumcheck};
    use crate::transcript::{Blake3Transcript, traits::Transcript};

    struct Updates {
        num_vars: usize,
        live_len: usize,
        updates: Vec<(usize, Field)>,
        passes: AtomicUsize,
    }

    impl StreamingCoefficientSource for Updates {
        fn num_vars(&self) -> usize {
            self.num_vars
        }
        fn live_len(&self) -> usize {
            self.live_len
        }
        fn for_each_coefficient(
            &self,
            emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
        ) -> Result<(), SumcheckError> {
            self.passes.fetch_add(1, Ordering::Relaxed);
            for &(index, value) in &self.updates {
                emit(index, value)?;
            }
            Ok(())
        }
    }

    fn fixture(
        num_vars: usize,
        live_len: usize,
        cfg: &FieldConfig,
    ) -> (Updates, Vec<Field>, Vec<u64>, Field) {
        let mut updates = Vec::new();
        let mut dense = vec![cfg.zero(); live_len];
        let mut bits = vec![0u64; live_len.div_ceil(64)];
        for index in 0..live_len {
            // Duplicates, negative terms and holes model independent binding
            // families adding to the same committed source coordinates.
            if index % 7 != 3 {
                for value in [index as u64 + 1, 19, 71] {
                    let value = Field::from_with_cfg(value, cfg);
                    let value = if index % 3 == 0 {
                        cfg.neg(&value)
                    } else {
                        value
                    };
                    updates.push((index, value));
                    dense[index] = cfg.add(&dense[index], &value);
                }
            }
            bits[index / 64] |= (((index * 13 + 5).count_ones() & 1) as u64) << (index % 64);
        }
        // Deterministically reorder updates across prefix and instance boundaries.
        for i in 0..updates.len() {
            let j = (i * 8191 + 17) % updates.len();
            updates.swap(i, j);
        }
        let claim = dense
            .iter()
            .enumerate()
            .fold(cfg.zero(), |sum, (i, value)| {
                if (bits[i / 64] >> (i % 64)) & 1 == 1 {
                    cfg.add(&sum, value)
                } else {
                    sum
                }
            });
        (
            Updates {
                num_vars,
                live_len,
                updates,
                passes: AtomicUsize::new(0),
            },
            dense,
            bits,
            claim,
        )
    }

    #[test]
    fn streaming_coefficients_match_dense_transcript_for_every_prefix_and_padding() {
        let cfg = spartan_bitz_field_config();
        for num_vars in [0usize, 1, 2, 3, 4, 7, 9] {
            let domain = 1usize << num_vars;
            for live_len in [1, domain.saturating_sub(3).max(1), domain] {
                let (updates, dense, bits, claim) = fixture(num_vars, live_len, &cfg);
                let streaming = StreamingMle::new(&updates);
                assert!(streaming.evaluation_at(0).is_err());
                for prefix in 0..=num_vars.min(SHA256_INNER_PREFIX_MAX_VARS) {
                    updates.passes.store(0, Ordering::Relaxed);
                    let mut a = Blake3Transcript::new();
                    let actual = prove_inner_sumcheck(
                        &cfg,
                        &mut a,
                        claim,
                        PackedInput::new(&streaming, &bits, num_vars, live_len, prefix),
                        (),
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap();
                    let mut b = Blake3Transcript::new();
                    let expected = prove_inner_sumcheck(
                        &cfg,
                        &mut b,
                        claim,
                        PackedInput::new(
                            &|index| Ok(dense[index]),
                            &bits,
                            num_vars,
                            live_len,
                            prefix,
                        ),
                        (),
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap();
                    assert_eq!(
                        actual, expected,
                        "n={num_vars}, live={live_len}, k={prefix}"
                    );
                    assert_eq!(a.get_challenge::<u128>(), b.get_challenge::<u128>());
                    assert_eq!(
                        updates.passes.load(Ordering::Relaxed),
                        if prefix == 0 { 1 } else { 2 }
                    );
                }
            }
        }
    }

    #[test]
    fn streaming_prefix_cache_evictions_preserve_overlapping_contributions() {
        let cfg = spartan_bitz_field_config();
        let num_vars = 17;
        let live_len = (1usize << num_vars) - 5;
        let mut updates = Updates {
            num_vars,
            live_len,
            updates: Vec::new(),
            passes: AtomicUsize::new(0),
        };
        let mut dense = vec![cfg.zero(); live_len];
        // Eight distinct blocks collide in one four-way cache set, then recur.
        for pass in 0..3 {
            for block in 0..8 {
                for bit in 0..16 {
                    let index = block * 1024 * 16 + bit;
                    let value = Field::from_with_cfg((pass * 37 + bit + 1) as u64, &cfg);
                    updates.updates.push((index, value));
                    dense[index] = cfg.add(&dense[index], &value);
                }
            }
        }
        let mut bits = vec![0x9537_e820_aedc_159bu64; live_len.div_ceil(64)];
        *bits.last_mut().unwrap() &= low_bits_mask(live_len % 64);
        let streaming = StreamingMle::new(&updates);
        let actual = streaming
            .build_prefix_accumulators::<4, _>(num_vars, live_len, &bits, &cfg, &cfg.zero())
            .unwrap();
        let expected = build_prefix_accumulators_generic::<4, _, _>(
            num_vars,
            live_len,
            &|i| Ok(dense[i]),
            &bits,
            &cfg,
            &cfg.zero(),
        )
        .unwrap();
        assert_eq!(actual.rounds, expected.rounds);
    }

    #[test]
    fn streaming_coefficients_support_shared_challenge_batches() {
        let cfg = spartan_bitz_field_config();
        let (first, dense_first, bits_first, claim_first) = fixture(8, 193, &cfg);
        let (second, dense_second, bits_second, claim_second) = fixture(8, 256, &cfg);
        let first_source = StreamingMle::new(&first);
        let second_source = StreamingMle::new(&second);
        let mut a = Blake3Transcript::new();
        let actual = prove_batched_inner_sumcheck(
            &cfg,
            &mut a,
            &[claim_first, claim_second],
            [
                PackedInput::new(&first_source, &bits_first, 8, 193, 4),
                PackedInput::new(&second_source, &bits_second, 8, 256, 4),
            ],
            [(), ()],
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        // A normal owned dense batch supplies an independent sumcheck path.
        let lift = |bits: &[u64], live: usize| {
            (0..256)
                .map(|i| {
                    if i < live && bits[i / 64] >> (i % 64) & 1 != 0 {
                        cfg.one()
                    } else {
                        cfg.zero()
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut padded_first = dense_first;
        padded_first.resize(256, cfg.zero());
        let mut b = Blake3Transcript::new();
        let expected = prove_batched_inner_sumcheck(
            &cfg,
            &mut b,
            &[claim_first, claim_second],
            [lift(&bits_first, 193), lift(&bits_second, 256)],
            [padded_first, dense_second],
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(a.get_challenge::<u128>(), b.get_challenge::<u128>());
    }

    #[test]
    fn streaming_rejects_invalid_updates_before_transcript_changes() {
        let cfg = spartan_bitz_field_config();
        for prefix in 0..=4 {
            for update in [
                (13, cfg.one()),
                (3, crate::piop::spartan::noncanonical_test_value(&cfg)),
            ] {
                let (mut updates, _, bits, claim) = fixture(4, 13, &cfg);
                updates.updates.push(update);
                let mut transcript = Blake3Transcript::new();
                assert!(
                    prove_inner_sumcheck(
                        &cfg,
                        &mut transcript,
                        claim,
                        PackedInput::new(&StreamingMle::new(&updates), &bits, 4, 13, prefix),
                        (),
                        &mut UngrindedRoundBoundary,
                    )
                    .is_err()
                );
                assert_eq!(
                    transcript.get_challenge::<u128>(),
                    Blake3Transcript::new().get_challenge::<u128>()
                );
            }
        }
    }
}
