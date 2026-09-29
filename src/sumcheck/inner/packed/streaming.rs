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

    /// Disjoint, power-of-two aligned intervals that can be replayed independently.
    fn partition_len(&self) -> usize {
        1usize.checked_shl(self.num_vars() as u32).unwrap_or(0)
    }
    fn for_each_partition(
        &self,
        partition: usize,
        emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        if partition != 0 {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        self.for_each_coefficient(emit)
    }

    /// Optional replay as aligned, strictly increasing blocks of final sums.
    /// Each slice has `block_len` entries; omitted blocks and entries beyond
    /// `live_len` are zero. Unlike the additive stream, blocks cannot overlap.
    /// Return `None` without invoking `emit` when this interface is unsupported.
    fn for_each_partition_block(
        &self,
        _partition: usize,
        _block_len: usize,
        _emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        None
    }

    /// Optional final coefficient sums grouped by their aligned witness byte.
    /// Bucket `b` contains, lane by lane, the sum of every eight-coefficient
    /// block whose witness byte is `b`. Read bytes only through `read_byte`,
    /// which checks the partition and masks its partial final block. Its second
    /// argument is the highest occupied coefficient lane plus one (1..=8),
    /// allowing it to reject coefficients in domain padding. Emit each
    /// bucket at most once, in increasing order; omitted buckets are zero.
    /// Bucket zero may be omitted since its witness extension is identically
    /// zero. Return `None` without invoking either callback when unsupported.
    fn for_each_partition_byte_bucket(
        &self,
        _partition: usize,
        _read_byte: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
        _emit: &mut impl FnMut(u8, &[Field; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        None
    }

    /// Optional additive replay after binding the low variables with the given
    /// equality weights. Indices are in the original domain divided by the
    /// weights' length; padding and omitted entries remain zero.
    fn for_each_partition_folded(
        &self,
        _partition: usize,
        _weights: &[Field],
        _emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        None
    }

    /// Optional folded replay of final sums in strictly increasing index order.
    /// Indices use the same folded domain as `for_each_partition_folded`.
    /// Each index occurs at most once; omitted coefficients are zero. This
    /// allows consumers to assign directly and finish adjacent pairs during
    /// replay. Return `None` without invoking `emit` when unsupported.
    fn for_each_partition_folded_final(
        &self,
        _partition: usize,
        _weights: &[Field],
        _emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        None
    }
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
            || !self.source.partition_len().is_power_of_two()
            || self.source.partition_len() < 1usize << K
            || self.source.partition_len() > 1usize << num_vars
        {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }

    fn visit_checked(
        &self,
        partition: usize,
        cfg: &FieldConfig,
        mut emit: impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Result<(), SumcheckError> {
        use field::CtOrd;
        let start = partition * self.source.partition_len();
        let end = (start + self.source.partition_len()).min(self.source.live_len());
        self.source
            .for_each_partition(partition, &mut |index, value| {
                if !(start..end).contains(&index) {
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

    fn visit_blocks_checked<const K: usize>(
        &self,
        partition: usize,
        cfg: &FieldConfig,
        mut emit: impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let width = 1usize << K;
        let start = partition * self.source.partition_len();
        let end = (start + self.source.partition_len()).min(self.source.live_len());
        let mut next = start;
        self.source
            .for_each_partition_block(partition, width, &mut |base, values| {
                if values.len() != width || base % width != 0 || base < next || base >= end {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                validate_field_values(values, cfg)?;
                let active = width.min(end - base);
                if values[active..].iter().any(|value| *value != cfg.zero()) {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                next = base + width;
                emit(base, values)
            })
    }

    fn visit_folded_checked<const K: usize>(
        &self,
        partition: usize,
        weights: &[Field],
        cfg: &FieldConfig,
        mut emit: impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let start = partition * self.source.partition_len();
        let end = (start + self.source.partition_len()).min(self.source.live_len());
        let range = (start >> K)..end.div_ceil(1 << K);
        self.source
            .for_each_partition_folded(partition, weights, &mut |index, value| {
                if !range.contains(&index) {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                validate_field_values(std::slice::from_ref(&value), cfg)?;
                emit(index, value)
            })
    }

    fn visit_folded_final_checked<const K: usize>(
        &self,
        partition: usize,
        weights: &[Field],
        cfg: &FieldConfig,
        mut emit: impl FnMut(usize, Field) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        let start = partition * self.source.partition_len();
        let end = (start + self.source.partition_len()).min(self.source.live_len());
        let mut next = start >> K;
        let end = end.div_ceil(1 << K);
        self.source
            .for_each_partition_folded_final(partition, weights, &mut |index, value| {
                if index < next || index >= end {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                validate_field_values(std::slice::from_ref(&value), cfg)?;
                next = index + 1;
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
        let count = live_len.div_ceil(self.source.partition_len());
        let partials: Vec<_> = crate::utils::cfg_into_iter!(0..count)
            .map(|partition| self.build_partition::<K, _>(partition, live_len, bits, cfg, zero))
            .collect();
        let mut result = PrefixAccumulators::new::<K>(zero);
        for partial in partials {
            for (out, input) in result.rounds.iter_mut().zip(partial?.rounds) {
                for (out, input) in out.iter_mut().zip(input) {
                    for j in 0..2 {
                        out[j] = cfg.add(&out[j], &input[j]);
                    }
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
        let width = self.source.partition_len() >> K;
        crate::utils::cfg_chunks_mut!(table.values, width)
            .enumerate()
            .try_for_each(|(partition, values)| {
                // The final even-length padding slot has no source coefficient.
                if partition * width >= suffix_count {
                    return Ok(());
                }
                if let Some(result) = self.visit_folded_final_checked::<K>(
                    partition,
                    &weights,
                    cfg,
                    |index, value| {
                        values[index - partition * width] = raw_montgomery(&value);
                        Ok(())
                    },
                ) {
                    return result;
                }
                if let Some(result) =
                    self.visit_folded_checked::<K>(partition, &weights, cfg, |index, value| {
                        let slot = &mut values[index - partition * width];
                        *slot = raw_montgomery(&cfg.add(&field_from_raw(slot, cfg), &value));
                        Ok(())
                    })
                {
                    return result;
                }
                if let Some(result) =
                    self.visit_blocks_checked::<K>(partition, cfg, |base, block| {
                        let value = if K == 0 {
                            block[0]
                        } else {
                            let mut sum = product_accumulator_zero();
                            for (weight, value) in weights.iter().zip(block) {
                                product_multiply_accumulate(cfg, &mut sum, weight, value);
                            }
                            product_reduce(sum, cfg)?
                        };
                        values[(base >> K) - partition * width] = raw_montgomery(&value);
                        Ok(())
                    })
                {
                    return result;
                }
                self.visit_checked(partition, cfg, |index, delta| {
                    let slot = &mut values[(index >> K) - partition * width];
                    let term = cfg.mul(&delta, &weights[index & ((1usize << K) - 1)]);
                    *slot = raw_montgomery(&cfg.add(&field_from_raw(slot, cfg), &term));
                    Ok(())
                })
            })?;
        Ok(table)
    }

    #[allow(clippy::too_many_arguments)]
    fn fold_prefix_table_and_prepare_tail<const K: usize, H: Sha256InnerBitSource + ?Sized>(
        &self,
        num_vars: usize,
        live_len: usize,
        challenges: &[Field],
        prefix_weights: &PreparedPrefixWeights,
        bits: &H,
        cfg: &FieldConfig,
        zero: &Field,
        one: &Field,
    ) -> Result<(CompactPrefixVTable, [Field; 2]), SumcheckError> {
        self.validate_dimensions::<K>(num_vars, live_len)?;
        if challenges.len() != K || prefix_weights.len() != 1 << K {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        validate_field_values(challenges, cfg)?;
        let width = self.source.partition_len() >> K;
        let suffix_count = live_len.div_ceil(1usize << K);
        let separate_scan = width == 1 || num_vars == K;
        // With only a few large source partitions, the ordinary first-tail
        // scan can distribute their pairs across more workers than the replay.
        // Keep that parallelism for large tables; small scans stay fused.
        #[cfg(feature = "parallel")]
        let separate_scan = separate_scan
            || (suffix_count.div_ceil(2) >= 1 << 10
                && rayon::current_num_threads() > live_len.div_ceil(self.source.partition_len()));
        // A one-suffix partition splits the first tail pair across workers.
        // Keep the ordinary fold for that uncommon shape and for K = num_vars.
        if separate_scan {
            let table =
                self.fold_prefix_table::<K>(num_vars, live_len, challenges, cfg, zero, one)?;
            let next = if num_vars > K {
                sum_first_tail_round::<K, _>(
                    &table,
                    live_len,
                    bits,
                    prefix_weights,
                    cfg,
                    zero,
                    one,
                )?
            } else {
                [*zero; 2]
            };
            return Ok((table, next));
        }

        let mut table = CompactPrefixVTable {
            values: vec![raw_montgomery(zero); (suffix_count + 1) & !1],
            suffix_count,
        };
        let partials: Vec<_> = crate::utils::cfg_chunks_mut!(table.values, width)
            .enumerate()
            .map(|(partition, values)| -> Result<_, SumcheckError> {
                let offset = partition * width;
                let mut accumulators = std::array::from_fn(|_| product_accumulator_zero());
                let accumulate = |accumulators: &mut [ProductAccumulator; 2],
                                  values: &[RawMontgomery],
                                  pair: usize| {
                    accumulate_first_tail_values::<K, _>(
                        accumulators,
                        &values[2 * pair..2 * pair + 2],
                        suffix_count,
                        offset / 2 + pair,
                        live_len,
                        bits,
                        prefix_weights,
                        cfg,
                        zero,
                        one,
                    )
                };

                // Final folded coefficients complete a pair as soon as the
                // next pair arrives. Missing coefficients retain their zero
                // slots; entirely omitted pairs contribute zero to both
                // round coefficients and need no witness loads or products.
                let mut pending_pair = None;
                if let Some(result) = self.visit_folded_final_checked::<K>(
                    partition,
                    prefix_weights,
                    cfg,
                    |index, value| {
                        let suffix = index - offset;
                        let pair = suffix / 2;
                        if let Some(previous) = pending_pair {
                            if previous != pair {
                                accumulate(&mut accumulators, values, previous)?;
                            }
                        }
                        pending_pair = Some(pair);
                        values[suffix] = raw_montgomery(&value);
                        Ok(())
                    },
                ) {
                    result?;
                    if let Some(pair) = pending_pair {
                        accumulate(&mut accumulators, values, pair)?;
                    }
                    return Ok(accumulators);
                }

                if let Some(result) = self.visit_folded_checked::<K>(
                    partition,
                    prefix_weights,
                    cfg,
                    |index, value| {
                        let slot = &mut values[index - offset];
                        *slot = raw_montgomery(&cfg.add(&field_from_raw(slot, cfg), &value));
                        Ok(())
                    },
                ) {
                    result?;
                    for pair in 0..values.len() / 2 {
                        accumulate(&mut accumulators, values, pair)?;
                    }
                    return Ok(accumulators);
                }

                // Ordered final-sum blocks complete each adjacent V pair as
                // soon as the next pair arrives. Consume it while it is hot;
                // omitted pairs have V0 = V1 = 0 and contribute nothing.
                // The control flow depends only on coefficient wiring, never
                // on the committed witness bits.
                let mut pending_pair = None;
                if let Some(result) =
                    self.visit_blocks_checked::<K>(partition, cfg, |base, block| {
                        let suffix = (base >> K) - offset;
                        let pair = suffix / 2;
                        if let Some(previous) = pending_pair {
                            if previous != pair {
                                accumulate(&mut accumulators, values, previous)?;
                            }
                        }
                        pending_pair = Some(pair);
                        let value = if K == 0 {
                            block[0]
                        } else {
                            let mut sum = product_accumulator_zero();
                            for (weight, value) in prefix_weights.iter().zip(block) {
                                product_multiply_accumulate(cfg, &mut sum, weight, value);
                            }
                            product_reduce(sum, cfg)?
                        };
                        values[suffix] = raw_montgomery(&value);
                        Ok(())
                    })
                {
                    result?;
                    if let Some(pair) = pending_pair {
                        accumulate(&mut accumulators, values, pair)?;
                    }
                    return Ok(accumulators);
                }

                // Additive streams may revisit any entry. Complete this
                // partition before accumulating its pairs, without a second
                // pass over the entire batch's compact coefficient table.
                self.visit_checked(partition, cfg, |index, delta| {
                    let slot = &mut values[(index >> K) - offset];
                    let term = cfg.mul(&delta, &prefix_weights[index & ((1usize << K) - 1)]);
                    *slot = raw_montgomery(&cfg.add(&field_from_raw(slot, cfg), &term));
                    Ok(())
                })?;
                for pair in 0..values.len() / 2 {
                    accumulate(&mut accumulators, values, pair)?;
                }
                Ok(accumulators)
            })
            .collect();
        let mut accumulators = std::array::from_fn(|_| product_accumulator_zero());
        for partial in partials {
            accumulators = merge_accumulators(accumulators, partial?);
        }
        let next = reduce_product_accumulators(accumulators, cfg)?;
        Ok((table, next))
    }
}

impl<S: StreamingCoefficientSource + ?Sized> StreamingMle<'_, S> {
    fn build_partition<const K: usize, H: Sha256InnerBitSource + ?Sized>(
        &self,
        partition: usize,
        live_len: usize,
        bits: &H,
        cfg: &FieldConfig,
        zero: &Field,
    ) -> Result<PrefixAccumulators, SumcheckError> {
        let mut state = PrefixBuildState::new::<K>(zero);
        // For a fixed witness byte, every prefix accumulator is linear in its
        // eight coefficient values. Sum those values before the ternary
        // extension, so a large partition extends at most 255 blocks instead
        // of one block for every eight source positions. Small partitions keep
        // the direct path to avoid initializing the 32 KiB bucket table.
        if K == 3 && self.source.partition_len() >= 1 << 12 {
            let start = partition * self.source.partition_len();
            let end = (start + self.source.partition_len()).min(live_len);
            let mut next_bucket = 0usize;
            if let Some(result) = self.source.for_each_partition_byte_bucket(
                partition,
                &mut |base, occupied_lanes| {
                    if base % 8 != 0
                        || !(start..end).contains(&base)
                        || occupied_lanes == 0
                        || occupied_lanes > 8
                        || occupied_lanes > end - base
                    {
                        return Err(SumcheckError::InvalidProductDimensions);
                    }
                    let active = 8.min(end - base);
                    let word = bits.bits_at(base, active)?;
                    if word & !low_bits_mask(active) != 0 {
                        return Err(SumcheckError::InvalidProductDimensions);
                    }
                    Ok(word as u8)
                },
                &mut |word, values| {
                    if usize::from(word) < next_bucket {
                        return Err(SumcheckError::InvalidProductDimensions);
                    }
                    next_bucket = usize::from(word) + 1;
                    validate_field_values(values, cfg)?;
                    if word != 0 {
                        accumulate_three_variable_block(
                            &mut state,
                            values,
                            usize::from(word),
                            cfg,
                            zero,
                        );
                    }
                    Ok(())
                },
            ) {
                result?;
                return finish_partition::<K>(state, cfg, zero);
            }
            let mut buckets = vec![*zero; 256 * 8];
            let mut occupied = [false; 256];
            if let Some(result) = self.visit_blocks_checked::<K>(partition, cfg, |base, values| {
                let active = 8.min(live_len - base);
                let word = bits.bits_at(base, active)?;
                if word & !low_bits_mask(active) != 0 {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                // The multilinear extension of an all-zero witness block is
                // zero everywhere, including at the ternary prefix points.
                if word != 0 {
                    occupied[word as usize] = true;
                    for (sum, value) in buckets[8 * word as usize..][..8].iter_mut().zip(values) {
                        *sum = cfg.add(sum, value);
                    }
                }
                Ok(())
            }) {
                result?;
                for (word, used) in occupied.into_iter().enumerate() {
                    if used {
                        accumulate_three_variable_block(
                            &mut state,
                            &buckets[8 * word..][..8],
                            word,
                            cfg,
                            zero,
                        );
                    }
                }
                return finish_partition::<K>(state, cfg, zero);
            }
        }
        if let Some(result) = self.visit_blocks_checked::<K>(partition, cfg, |base, values| {
            accumulate_block::<K, _>(&mut state, base >> K, values, bits, live_len, cfg, zero)
        }) {
            result?;
            return finish_partition::<K>(state, cfg, zero);
        }
        // A four-way bounded cache combines nearby scatter updates before the
        // ternary prefix extension. Evictions remain exact because every prefix
        // accumulator is linear in the coefficient table for a fixed witness.
        // At K=4 the coefficient cache occupies at most one MiB, regardless of N.
        const SETS: usize = 1024;
        const WAYS: usize = 4;
        let width = 1usize << K;
        let blocks = self.source.partition_len().div_ceil(width);
        let sets = blocks.next_power_of_two().min(SETS);
        let mut keys = vec![usize::MAX; sets * WAYS];
        let mut replace = vec![0usize; sets];
        let mut values = vec![*zero; keys.len() * width];
        self.visit_checked(partition, cfg, |index, delta| {
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
        finish_partition::<K>(state, cfg, zero)
    }
}

fn finish_partition<const K: usize>(
    state: PrefixBuildState,
    cfg: &FieldConfig,
    zero: &Field,
) -> Result<PrefixAccumulators, SumcheckError> {
    let beta = state
        .partial_sums
        .into_iter()
        .map(|sum| linear_reduce(sum, cfg))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(scatter_beta_values::<K>(&beta, zero, cfg))
}

/// Three-variable bit blocks have only 256 possible ternary extensions. Each
/// entry is an integer in [-4, 4], so the whole table occupies only 6.75 KiB.
/// As in the existing zero-skipping accumulator, accesses depend on witness
/// bits; this packed prover does not provide constant-time witness processing.
static BIT_EXTENSIONS_3: [[i8; 27]; 256] = bit_extensions_3();

fn accumulate_three_variable_block(
    state: &mut PrefixBuildState,
    values: &[Field],
    word: usize,
    cfg: &FieldConfig,
    zero: &Field,
) {
    state.v_values.clear();
    state.v_values.extend_from_slice(values);
    extend_lsb::<Field, 3, _>(&mut state.v_values, &mut state.v_scratch, zero, |hi, lo| {
        cfg.sub(hi, lo)
    });
    for (beta, &coefficient) in BIT_EXTENSIONS_3[word].iter().enumerate() {
        if coefficient != 0 {
            linear_multiply_accumulate_signed(
                cfg,
                &mut state.partial_sums[beta],
                &state.v_values[beta],
                i64::from(coefficient),
                zero,
            );
        }
    }
}

const fn bit_extensions_3() -> [[i8; 27]; 256] {
    let mut table = [[0; 27]; 256];
    let mut word = 0;
    while word < 256 {
        let mut beta = 0;
        while beta < 27 {
            let mut index = 0;
            while index < 8 {
                let mut term = ((word >> index) & 1) as i8;
                let mut digits = beta;
                let mut axis = 0;
                while axis < 3 {
                    let bit = ((index >> axis) & 1) as i8;
                    // The ternary coordinates are (slope, at zero, at one).
                    term *= match digits % 3 {
                        0 => 2 * bit - 1,
                        1 => 1 - bit,
                        _ => bit,
                    };
                    digits /= 3;
                    axis += 1;
                }
                table[word][beta] += term;
                index += 1;
            }
            beta += 1;
        }
        word += 1;
    }
    table
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
    if K == 3 {
        accumulate_three_variable_block(state, values, word as usize, cfg, zero);
        return Ok(());
    }
    state.v_values.clear();
    state.v_values.extend_from_slice(values);
    extend_lsb::<Field, K, _>(&mut state.v_values, &mut state.v_scratch, zero, |hi, lo| {
        cfg.sub(hi, lo)
    });
    state.h_values.clear();
    state
        .h_values
        .extend((0..width).map(|i| ((word >> i) & 1) as i64));
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
    use crate::piop::spartan::spartan_bitz_field_config;
    use crate::sumcheck::UngrindedRoundBoundary;
    use crate::sumcheck::inner::{prove_batched_inner_sumcheck, prove_inner_sumcheck};
    use crate::transcript::{Blake3Transcript, traits::Transcript};

    #[test]
    fn cached_three_variable_bits_match_recursive_extension_exhaustively() {
        for (word, cached) in BIT_EXTENSIONS_3.iter().enumerate() {
            let mut reference: Vec<i64> = (0..8).map(|i| ((word >> i) & 1) as i64).collect();
            extend_lsb::<i64, 3, _>(&mut reference, &mut Vec::new(), &0, |hi, lo| hi - lo);
            assert_eq!(
                cached.map(i64::from).as_slice(),
                reference,
                "witness byte {word}",
            );
            assert!(cached.iter().all(|value| (-4..=4).contains(value)));
        }
    }

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
    fn partitioned_stream_matches_serial_for_partial_tail_and_every_prefix() {
        struct Partitioned<'a>(&'a Updates);
        impl StreamingCoefficientSource for Partitioned<'_> {
            fn num_vars(&self) -> usize {
                self.0.num_vars
            }
            fn live_len(&self) -> usize {
                self.0.live_len
            }
            fn partition_len(&self) -> usize {
                16
            }
            fn for_each_coefficient(
                &self,
                emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                self.0.for_each_coefficient(emit)
            }
            fn for_each_partition(
                &self,
                partition: usize,
                emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                for &(i, value) in &self.0.updates {
                    if i / 16 == partition {
                        emit(i, value)?;
                    }
                }
                Ok(())
            }
        }
        let cfg = spartan_bitz_field_config();
        let (updates, _, bits, claim) = fixture(8, 193, &cfg);
        let partitions = Partitioned(&updates);
        for prefix in 0..=4 {
            let mut serial = Blake3Transcript::new();
            let expected = prove_inner_sumcheck(
                &cfg,
                &mut serial,
                claim,
                PackedInput::new(&StreamingMle::new(&updates), &bits, 8, 193, prefix),
                (),
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            let mut parallel = Blake3Transcript::new();
            let actual = prove_inner_sumcheck(
                &cfg,
                &mut parallel,
                claim,
                PackedInput::new(&StreamingMle::new(&partitions), &bits, 8, 193, prefix),
                (),
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                serial.get_challenge::<u128>(),
                parallel.get_challenge::<u128>()
            );
        }
    }

    struct Blocks<'a> {
        values: &'a [Field],
        zero: Field,
        num_vars: usize,
        partition_len: usize,
    }
    impl StreamingCoefficientSource for Blocks<'_> {
        fn num_vars(&self) -> usize {
            self.num_vars
        }
        fn live_len(&self) -> usize {
            self.values.len()
        }
        fn partition_len(&self) -> usize {
            self.partition_len
        }
        fn for_each_coefficient(
            &self,
            _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
        ) -> Result<(), SumcheckError> {
            panic!("ordered block source used the scatter path")
        }
        fn for_each_partition_block(
            &self,
            partition: usize,
            block_len: usize,
            emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
        ) -> Option<Result<(), SumcheckError>> {
            Some((|| {
                let start = self.partition_len * partition;
                let end = (start + self.partition_len).min(self.values.len());
                for base in (start..end).step_by(block_len) {
                    let mut block = [self.zero; 16];
                    let active = block_len.min(end - base);
                    block[..active].copy_from_slice(&self.values[base..base + active]);
                    if block[..block_len].iter().any(|value| *value != self.zero) {
                        emit(base, &block[..block_len])?;
                    }
                }
                Ok(())
            })())
        }
    }

    struct FinalFolded<'a> {
        blocks: Blocks<'a>,
        cfg: &'a FieldConfig,
    }

    impl StreamingCoefficientSource for FinalFolded<'_> {
        fn num_vars(&self) -> usize {
            self.blocks.num_vars()
        }
        fn live_len(&self) -> usize {
            self.blocks.live_len()
        }
        fn partition_len(&self) -> usize {
            self.blocks.partition_len()
        }
        fn for_each_coefficient(
            &self,
            _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
        ) -> Result<(), SumcheckError> {
            panic!("final folded source used the scatter path")
        }
        fn for_each_partition_block(
            &self,
            partition: usize,
            width: usize,
            emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
        ) -> Option<Result<(), SumcheckError>> {
            self.blocks.for_each_partition_block(partition, width, emit)
        }
        fn for_each_partition_folded(
            &self,
            _: usize,
            _: &[Field],
            _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
        ) -> Option<Result<(), SumcheckError>> {
            panic!("final folded source used the additive folded path")
        }
        fn for_each_partition_folded_final(
            &self,
            partition: usize,
            weights: &[Field],
            emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
        ) -> Option<Result<(), SumcheckError>> {
            Some((|| {
                let start = partition * self.partition_len();
                let end = (start + self.partition_len()).min(self.live_len());
                for base in (start..end).step_by(weights.len()) {
                    let value = self.blocks.values[base..end.min(base + weights.len())]
                        .iter()
                        .zip(weights)
                        .fold(self.cfg.zero(), |sum, (value, weight)| {
                            self.cfg.add(&sum, &self.cfg.mul(value, weight))
                        });
                    if value != self.cfg.zero() {
                        emit(base / weights.len(), value)?;
                    }
                }
                Ok(())
            })())
        }
    }

    #[test]
    fn ordered_final_folded_replay_matches_complete_dense_proofs() {
        let cfg = spartan_bitz_field_config();
        for live_len in [1usize, 13, 32, 33, 193, 256, 2045, 2048, 16377] {
            // The largest one-partition case retains the separate parallel
            // tail scan at K=3; smaller partitions exercise the fused path.
            let domain = live_len.next_power_of_two().max(2048);
            let num_vars = domain.ilog2() as usize;
            let (_, mut coefficients, bits, _) = fixture(num_vars, live_len, &cfg);
            for (i, value) in coefficients.iter_mut().enumerate() {
                // Empty partitions, missing adjacent pairs, only-high and
                // only-low pairs, and a partial last word with a live value.
                if i + 1 != live_len && matches!((i / 8) % 16, 0..=7 | 10 | 13) {
                    *value = cfg.zero();
                }
            }
            let witness: Vec<_> = (0..domain)
                .map(|i| {
                    if i < live_len && bits[i / 64] >> (i % 64) & 1 != 0 {
                        cfg.one()
                    } else {
                        cfg.zero()
                    }
                })
                .collect();
            let mut dense = coefficients.clone();
            dense.resize(domain, cfg.zero());
            let claim = dense
                .iter()
                .zip(&witness)
                .fold(cfg.zero(), |sum, (a, b)| cfg.add(&sum, &cfg.mul(a, b)));
            let mut reference_transcript = Blake3Transcript::new();
            let expected = prove_inner_sumcheck(
                &cfg,
                &mut reference_transcript,
                claim,
                witness,
                dense,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            let reference_challenge = reference_transcript.get_challenge::<u128>();
            for partition_len in [16, 64, domain] {
                let source = FinalFolded {
                    blocks: Blocks {
                        values: &coefficients,
                        zero: cfg.zero(),
                        num_vars,
                        partition_len,
                    },
                    cfg: &cfg,
                };
                for prefix in 0..=4 {
                    let mut transcript = Blake3Transcript::new();
                    let actual = prove_inner_sumcheck(
                        &cfg,
                        &mut transcript,
                        claim,
                        PackedInput::new(
                            &StreamingMle::new(&source),
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
                        "live={live_len}, partition={partition_len}, prefix={prefix}"
                    );
                    assert_eq!(transcript.get_challenge::<u128>(), reference_challenge);
                }
            }
        }
    }

    #[test]
    fn ordered_final_folded_replay_skips_absent_pairs_and_handles_partial_last_word() {
        struct CountedBits(AtomicUsize);
        impl Sha256InnerBitSource for CountedBits {
            fn bit_at(&self, _: usize) -> Result<u64, SumcheckError> {
                unreachable!("the packed byte interface must be used")
            }
            fn bits_at(&self, _: usize, count: usize) -> Result<u64, SumcheckError> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(low_bits_mask(count))
            }
        }
        let cfg = spartan_bitz_field_config();
        let zero = cfg.zero();
        let one = cfg.one();
        let mut coefficients = vec![zero; 65];
        for i in [16, 40, 64] {
            coefficients[i] = Field::from_with_cfg(i as u64 + 1, &cfg);
        }
        let source = FinalFolded {
            blocks: Blocks {
                values: &coefficients,
                zero,
                num_vars: 7,
                partition_len: 32,
            },
            cfg: &cfg,
        };
        let challenges = [Field::from_with_cfg(3u64, &cfg); 3];
        let weights =
            PreparedPrefixWeights::new(equality_weights_lsb(&challenges, &zero, &one, &cfg), &cfg);
        let bits = CountedBits(AtomicUsize::new(0));
        let (table, round) = StreamingMle::new(&source)
            .fold_prefix_table_and_prepare_tail::<3, _>(
                7,
                65,
                &challenges,
                &weights,
                &bits,
                &cfg,
                &zero,
                &one,
            )
            .unwrap();
        // Two occupied pairs need both witness bytes, even when one side's
        // coefficient is absent. The last pair has one partial witness byte.
        assert_eq!(bits.0.load(Ordering::Relaxed), 5);
        let reference = StreamingMle::new(&source.blocks)
            .fold_prefix_table::<3>(7, 65, &challenges, &cfg, &zero, &one)
            .unwrap();
        assert_eq!(table.values, reference.values);
        assert_eq!(
            round,
            sum_first_tail_round::<3, _>(
                &reference,
                65,
                &[u64::MAX, 1][..],
                &weights,
                &cfg,
                &zero,
                &one,
            )
            .unwrap()
        );
    }

    #[test]
    fn cached_bit_blocks_match_dense_sumcheck_for_every_byte() {
        let cfg = spartan_bitz_field_config();
        let mut bits = vec![0u64; 32];
        for word in 0..256 {
            bits[word / 8] |= (word as u64) << (8 * (word % 8));
        }
        let coefficients: Vec<_> = (0..2048)
            .map(|i| {
                let value = Field::from_with_cfg((i * 8191 + 17) as u64, &cfg);
                if i % 3 == 0 { cfg.neg(&value) } else { value }
            })
            .collect();
        let witness: Vec<_> = (0..2048)
            .map(|i| {
                if bits[i / 64] >> (i % 64) & 1 == 0 {
                    cfg.zero()
                } else {
                    cfg.one()
                }
            })
            .collect();
        let claim = coefficients
            .iter()
            .zip(&witness)
            .fold(cfg.zero(), |sum, (a, b)| cfg.add(&sum, &cfg.mul(a, b)));
        let blocks = Blocks {
            values: &coefficients,
            zero: cfg.zero(),
            num_vars: 11,
            partition_len: 64,
        };
        let mut actual_transcript = Blake3Transcript::new();
        let actual = prove_inner_sumcheck(
            &cfg,
            &mut actual_transcript,
            claim,
            PackedInput::new(&StreamingMle::new(&blocks), &bits, 11, 2048, 3),
            (),
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let mut reference_transcript = Blake3Transcript::new();
        let reference = prove_inner_sumcheck(
            &cfg,
            &mut reference_transcript,
            claim,
            witness,
            coefficients,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(actual, reference);
        assert_eq!(
            actual_transcript.get_challenge::<u128>(),
            reference_transcript.get_challenge::<u128>(),
        );
    }

    #[test]
    fn grouped_witness_patterns_match_dense_with_repetition_cancellation_and_padding() {
        let cfg = spartan_bitz_field_config();
        // Repeat every witness pattern within each partition. Coefficients
        // include negatives and cancellation, while the last partial block and
        // capacity tail exercise independent coefficient/witness padding.
        let num_vars = 14;
        let live_len: usize = (1 << num_vars) - 13;
        let mut bits = vec![0u64; live_len.div_ceil(64)];
        let mut coefficients = vec![cfg.zero(); 1 << num_vars];
        let mut witness = vec![cfg.zero(); 1 << num_vars];
        for i in 0..live_len {
            let word = (i / 8) % 256;
            if word >> (i % 8) & 1 != 0 {
                bits[i / 64] |= 1 << (i % 64);
                witness[i] = cfg.one();
            }
            let value = Field::from_with_cfg((i % 2048 + 1) as u64, &cfg);
            coefficients[i] = if i / 2048 % 2 == 0 {
                value
            } else {
                cfg.neg(&value)
            };
        }
        let claim = coefficients
            .iter()
            .zip(&witness)
            .fold(cfg.zero(), |sum, (a, b)| cfg.add(&sum, &cfg.mul(a, b)));
        let blocks = Blocks {
            values: &coefficients[..live_len],
            zero: cfg.zero(),
            num_vars,
            partition_len: 1 << 12,
        };
        let mut actual_transcript = Blake3Transcript::new();
        let actual = prove_inner_sumcheck(
            &cfg,
            &mut actual_transcript,
            claim,
            PackedInput::new(&StreamingMle::new(&blocks), &bits, num_vars, live_len, 3),
            (),
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let mut reference_transcript = Blake3Transcript::new();
        let reference = prove_inner_sumcheck(
            &cfg,
            &mut reference_transcript,
            claim,
            witness,
            coefficients,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(actual, reference);
        assert_eq!(
            actual_transcript.get_challenge::<u128>(),
            reference_transcript.get_challenge::<u128>(),
        );
    }

    #[test]
    fn ordered_blocks_match_scatter_proofs_with_partition_tails_and_zero_blocks() {
        let cfg = spartan_bitz_field_config();
        for live_len in [1, 13, 32, 33, 193, 256] {
            let (mut updates, mut dense, bits, _) = fixture(8, live_len, &cfg);
            // Empty first/middle partitions and holes exercise omitted blocks.
            updates.updates.retain(|(i, _)| i / 32 % 3 != 0);
            for (i, value) in dense.iter_mut().enumerate() {
                if i / 32 % 3 == 0 {
                    *value = cfg.zero();
                }
            }
            let claim = dense
                .iter()
                .enumerate()
                .fold(cfg.zero(), |sum, (i, value)| {
                    if bits[i / 64] >> (i % 64) & 1 != 0 {
                        cfg.add(&sum, value)
                    } else {
                        sum
                    }
                });
            let blocks = Blocks {
                values: &dense,
                zero: cfg.zero(),
                num_vars: 8,
                partition_len: 32,
            };
            for prefix in 0..=4 {
                let mut a = Blake3Transcript::new();
                let actual = prove_inner_sumcheck(
                    &cfg,
                    &mut a,
                    claim,
                    PackedInput::new(&StreamingMle::new(&blocks), &bits, 8, live_len, prefix),
                    (),
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                let mut b = Blake3Transcript::new();
                let expected = prove_inner_sumcheck(
                    &cfg,
                    &mut b,
                    claim,
                    PackedInput::new(&StreamingMle::new(&updates), &bits, 8, live_len, prefix),
                    (),
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                assert_eq!(actual, expected, "live={live_len}, prefix={prefix}");
                assert_eq!(a.get_challenge::<u128>(), b.get_challenge::<u128>());
            }
        }
    }

    #[test]
    fn direct_folded_replays_match_dense_transcripts_across_partition_tails() {
        struct Folded<'a> {
            blocks: Blocks<'a>,
            cfg: &'a FieldConfig,
        }
        impl StreamingCoefficientSource for Folded<'_> {
            fn num_vars(&self) -> usize {
                self.blocks.num_vars()
            }
            fn live_len(&self) -> usize {
                self.blocks.live_len()
            }
            fn partition_len(&self) -> usize {
                self.blocks.partition_len()
            }
            fn for_each_coefficient(
                &self,
                _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                panic!("unexpected scalar replay")
            }
            fn for_each_partition_block(
                &self,
                partition: usize,
                width: usize,
                emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                self.blocks.for_each_partition_block(partition, width, emit)
            }
            fn for_each_partition_folded(
                &self,
                partition: usize,
                weights: &[Field],
                emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                Some((|| {
                    let start = partition * self.partition_len();
                    let end = (start + self.partition_len()).min(self.live_len());
                    // Repeat destinations and emit in reverse order, which
                    // catches accidentally treating additive updates as final.
                    for i in (start..end).rev() {
                        emit(
                            i / weights.len(),
                            self.cfg
                                .mul(&self.blocks.values[i], &weights[i % weights.len()]),
                        )?;
                    }
                    Ok(())
                })())
            }
        }
        let cfg = spartan_bitz_field_config();
        for live_len in [1, 13, 32, 33, 193, 256] {
            let (_, coefficients, bits, _) = fixture(8, live_len, &cfg);
            let mut witness = vec![cfg.zero(); 256];
            for i in 0..live_len {
                if bits[i / 64] >> (i % 64) & 1 != 0 {
                    witness[i] = cfg.one();
                }
            }
            let mut dense = coefficients.clone();
            dense.resize(256, cfg.zero());
            let claim = dense
                .iter()
                .zip(&witness)
                .fold(cfg.zero(), |sum, (a, b)| cfg.add(&sum, &cfg.mul(a, b)));
            for partition_len in [16, 64, 256] {
                let source = Folded {
                    blocks: Blocks {
                        values: &coefficients,
                        zero: cfg.zero(),
                        num_vars: 8,
                        partition_len,
                    },
                    cfg: &cfg,
                };
                for prefix in 0..=4 {
                    let mut actual_transcript = Blake3Transcript::new();
                    let actual = prove_inner_sumcheck(
                        &cfg,
                        &mut actual_transcript,
                        claim,
                        PackedInput::new(&StreamingMle::new(&source), &bits, 8, live_len, prefix),
                        (),
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap();
                    let mut expected_transcript = Blake3Transcript::new();
                    let expected = prove_inner_sumcheck(
                        &cfg,
                        &mut expected_transcript,
                        claim,
                        witness.clone(),
                        dense.clone(),
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap();
                    assert_eq!(
                        actual, expected,
                        "live={live_len}, partition={partition_len}, prefix={prefix}"
                    );
                    assert_eq!(
                        actual_transcript.get_challenge::<u128>(),
                        expected_transcript.get_challenge::<u128>()
                    );
                }
            }
        }
    }

    #[test]
    fn fused_streaming_transition_matches_separate_fold_at_falcon_field_widths() {
        fn check<const K: usize>(cfg: &FieldConfig) {
            let zero = cfg.zero();
            let one = cfg.one();
            for live_len in [1, 13, 16, 17, 31, 32, 33, 193, 1021, 1024] {
                let (_, mut dense, bits, _) = fixture(10, live_len, cfg);
                for (i, value) in dense.iter_mut().enumerate() {
                    // Whole missing pairs, one missing side, and cancellation
                    // within an emitted block all appear in the same replay.
                    if (i / 16) % 7 <= 2 {
                        *value = zero;
                    } else if i % 2 == 0 {
                        *value = cfg.neg(value);
                    }
                }
                for partition_len in [16, 32, 128, 1024] {
                    let source = Blocks {
                        values: &dense,
                        zero,
                        num_vars: 10,
                        partition_len,
                    };
                    let stream = StreamingMle::new(&source);
                    let final_source = FinalFolded {
                        blocks: Blocks {
                            values: &dense,
                            zero,
                            num_vars: 10,
                            partition_len,
                        },
                        cfg,
                    };
                    for challenge_kind in 0..3 {
                        let challenges: Vec<_> = (0..K)
                            .map(|i| match challenge_kind {
                                0 => zero,
                                1 => one,
                                _ => cfg.neg(&Field::from_with_cfg(101 + i as u64, cfg)),
                            })
                            .collect();
                        let weights = PreparedPrefixWeights::new(
                            equality_weights_lsb(&challenges, &zero, &one, cfg),
                            cfg,
                        );
                        let expected_table = stream
                            .fold_prefix_table::<K>(10, live_len, &challenges, cfg, &zero, &one)
                            .unwrap();
                        let expected_round = sum_first_tail_round::<K, _>(
                            &expected_table,
                            live_len,
                            &bits,
                            &weights,
                            cfg,
                            &zero,
                            &one,
                        )
                        .unwrap();
                        let (actual_table, actual_round) = stream
                            .fold_prefix_table_and_prepare_tail::<K, _>(
                                10,
                                live_len,
                                &challenges,
                                &weights,
                                &bits,
                                cfg,
                                &zero,
                                &one,
                            )
                            .unwrap();
                        assert_eq!(actual_table.values, expected_table.values);
                        assert_eq!(actual_table.suffix_count, expected_table.suffix_count);
                        assert_eq!(actual_round, expected_round);
                        let (final_table, final_round) = StreamingMle::new(&final_source)
                            .fold_prefix_table_and_prepare_tail::<K, _>(
                                10,
                                live_len,
                                &challenges,
                                &weights,
                                &bits,
                                cfg,
                                &zero,
                                &one,
                            )
                            .unwrap();
                        assert_eq!(final_table.values, expected_table.values);
                        assert_eq!(final_table.suffix_count, expected_table.suffix_count);
                        assert_eq!(final_round, expected_round);
                    }
                }
            }
        }

        for bit_width in [100, 125, 126] {
            let cfg = crate::prime_sampling::sample_prime_context(
                &mut Blake3Transcript::new(),
                1u128 << (bit_width - 1),
                (1u128 << bit_width) - 1,
                128,
            )
            .unwrap();
            check::<0>(&cfg);
            check::<1>(&cfg);
            check::<2>(&cfg);
            check::<3>(&cfg);
            check::<4>(&cfg);

            // Compare the entire proof and subsequent transcript challenge
            // against the random-access source, including its separate tail.
            let (_, dense, bits, claim) = fixture(10, 1021, &cfg);
            let blocks = Blocks {
                values: &dense,
                zero: cfg.zero(),
                num_vars: 10,
                partition_len: 128,
            };
            for prefix in 0..=4 {
                let mut a = Blake3Transcript::new();
                let actual = prove_inner_sumcheck(
                    &cfg,
                    &mut a,
                    claim,
                    PackedInput::new(&StreamingMle::new(&blocks), &bits, 10, 1021, prefix),
                    (),
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                let mut b = Blake3Transcript::new();
                let expected = prove_inner_sumcheck(
                    &cfg,
                    &mut b,
                    claim,
                    PackedInput::new(&|i| Ok(dense[i]), &bits, 10, 1021, prefix),
                    (),
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(a.get_challenge::<u128>(), b.get_challenge::<u128>());
            }
        }
    }

    #[test]
    fn ordered_blocks_reject_invalid_replays_before_transcript_changes() {
        struct InvalidBlocks {
            fault: usize,
            cfg: FieldConfig,
        }
        impl StreamingCoefficientSource for InvalidBlocks {
            fn num_vars(&self) -> usize {
                5
            }
            fn live_len(&self) -> usize {
                29
            }
            fn partition_len(&self) -> usize {
                16
            }
            fn for_each_coefficient(
                &self,
                _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                unreachable!()
            }
            fn for_each_partition_block(
                &self,
                partition: usize,
                width: usize,
                emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                let mut block = vec![self.cfg.zero(); width];
                let base = 16 * partition;
                Some(match self.fault {
                    0 => {
                        block[0] = crate::piop::spartan::noncanonical_test_value(&self.cfg);
                        emit(base, &block)
                    }
                    1 => emit(base, &block[..width - 1]),
                    2 => emit(base + 16, &block), // next partition is outside this replay
                    3 => emit(base, &block).and_then(|_| emit(base, &block)),
                    4 => emit(if width > 1 { base + 1 } else { usize::MAX }, &block),
                    5 if partition == 1 => {
                        block[29 % width] = self.cfg.one();
                        emit(29 / width * width, &block) // nonzero domain padding
                    }
                    5 => Ok(()),
                    _ => unreachable!(),
                })
            }
        }
        let cfg = spartan_bitz_field_config();
        for fault in 0..6 {
            for prefix in 0..=4 {
                let source = InvalidBlocks {
                    fault,
                    cfg: cfg.clone(),
                };
                let mut transcript = Blake3Transcript::new();
                assert!(
                    prove_inner_sumcheck(
                        &cfg,
                        &mut transcript,
                        cfg.zero(),
                        PackedInput::new(&StreamingMle::new(&source), &[0u64][..], 5, 29, prefix),
                        (),
                        &mut UngrindedRoundBoundary,
                    )
                    .is_err(),
                    "fault={fault}, prefix={prefix}"
                );
                assert_eq!(
                    transcript.get_challenge::<u128>(),
                    Blake3Transcript::new().get_challenge::<u128>()
                );
            }
        }
    }

    #[test]
    fn byte_buckets_reject_invalid_reads_order_and_residues_before_transcript_changes() {
        struct InvalidBuckets<'a> {
            cfg: &'a FieldConfig,
            fault: usize,
        }
        impl StreamingCoefficientSource for InvalidBuckets<'_> {
            fn num_vars(&self) -> usize {
                13
            }
            fn live_len(&self) -> usize {
                4099
            }
            fn partition_len(&self) -> usize {
                4096
            }
            fn for_each_coefficient(
                &self,
                _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                unreachable!()
            }
            fn for_each_partition_byte_bucket(
                &self,
                partition: usize,
                read_byte: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
                emit: &mut impl FnMut(u8, &[Field; 8]) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                let mut values = [self.cfg.zero(); 8];
                let base = partition * self.partition_len();
                Some(match self.fault {
                    0 => {
                        // Noncanonical bucket, including the omitted-zero pattern.
                        values[0] = crate::piop::spartan::noncanonical_test_value(self.cfg);
                        emit(0, &values)
                    }
                    1 => emit(2, &values).and_then(|_| emit(2, &values)),
                    2 => emit(2, &values).and_then(|_| emit(1, &values)),
                    3 => read_byte(base + 1, 1).map(|_| ()),
                    4 => read_byte(base + 4096, 1).map(|_| ()),
                    5 => read_byte(usize::MAX, 1).map(|_| ()),
                    6 => read_byte(base, 0).map(|_| ()),
                    7 => read_byte(base, 9).map(|_| ()),
                    8 if partition == 1 => read_byte(base, 4).map(|_| ()),
                    8 => Ok(()),
                    9 if partition == 1 => read_byte(base - 8, 8).map(|_| ()),
                    9 => Ok(()),
                    _ => unreachable!(),
                })
            }
        }
        let cfg = spartan_bitz_field_config();
        for fault in 0..10 {
            let source = InvalidBuckets { cfg: &cfg, fault };
            let mut transcript = Blake3Transcript::new();
            assert!(
                prove_inner_sumcheck(
                    &cfg,
                    &mut transcript,
                    cfg.zero(),
                    PackedInput::new(&StreamingMle::new(&source), &vec![0u64; 65], 13, 4099, 3),
                    (),
                    &mut UngrindedRoundBoundary,
                )
                .is_err(),
                "fault={fault}"
            );
            assert_eq!(
                transcript.get_challenge::<u128>(),
                Blake3Transcript::new().get_challenge::<u128>()
            );
        }
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

    #[test]
    fn ordered_final_folded_replays_reject_bad_order_bounds_and_residues() {
        struct InvalidFinal<'a> {
            cfg: &'a FieldConfig,
            fault: usize,
        }
        impl StreamingCoefficientSource for InvalidFinal<'_> {
            fn num_vars(&self) -> usize {
                6
            }
            fn live_len(&self) -> usize {
                37
            }
            fn partition_len(&self) -> usize {
                32
            }
            fn for_each_coefficient(
                &self,
                _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                panic!("invalid final replay fell back to scalar replay")
            }
            fn for_each_partition_folded_final(
                &self,
                partition: usize,
                weights: &[Field],
                emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                let first = partition * 32 / weights.len();
                Some(match self.fault {
                    0 => emit(first, self.cfg.one()).and_then(|_| emit(first, self.cfg.one())),
                    1 => emit(first + 1, self.cfg.one()).and_then(|_| emit(first, self.cfg.one())),
                    2 => emit(first + 32 / weights.len(), self.cfg.one()),
                    3 => emit(usize::MAX, self.cfg.one()),
                    4 => emit(
                        first,
                        crate::piop::spartan::noncanonical_test_value(self.cfg),
                    ),
                    5 if partition == 1 => emit(first - 1, self.cfg.one()),
                    6 if partition == 1 => emit(37usize.div_ceil(weights.len()), self.cfg.one()),
                    5 | 6 => Ok(()),
                    _ => unreachable!(),
                })
            }
        }
        fn check<const K: usize>(source: &InvalidFinal<'_>) {
            let cfg = source.cfg;
            let zero = cfg.zero();
            let one = cfg.one();
            let challenges = [one; K];
            let weights = PreparedPrefixWeights::new(
                equality_weights_lsb(&challenges, &zero, &one, cfg),
                cfg,
            );
            let stream = StreamingMle::new(source);
            assert!(
                stream
                    .fold_prefix_table::<K>(6, 37, &challenges, cfg, &zero, &one)
                    .is_err()
            );
            assert!(
                stream
                    .fold_prefix_table_and_prepare_tail::<K, _>(
                        6,
                        37,
                        &challenges,
                        &weights,
                        &[0u64][..],
                        cfg,
                        &zero,
                        &one,
                    )
                    .is_err()
            );
        }
        let cfg = spartan_bitz_field_config();
        for fault in 0..7 {
            let source = InvalidFinal { cfg: &cfg, fault };
            check::<0>(&source);
            check::<1>(&source);
            check::<3>(&source);
            check::<4>(&source);
        }
    }

    #[test]
    fn direct_folded_replays_reject_wrong_partition_and_noncanonical_values() {
        struct InvalidFolded<'a> {
            cfg: &'a FieldConfig,
            fault: usize,
        }
        impl StreamingCoefficientSource for InvalidFolded<'_> {
            fn num_vars(&self) -> usize {
                5
            }
            fn live_len(&self) -> usize {
                29
            }
            fn partition_len(&self) -> usize {
                16
            }
            fn for_each_coefficient(
                &self,
                _: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Result<(), SumcheckError> {
                panic!("unexpected scalar replay")
            }
            fn for_each_partition_folded(
                &self,
                partition: usize,
                weights: &[Field],
                emit: &mut impl FnMut(usize, Field) -> Result<(), SumcheckError>,
            ) -> Option<Result<(), SumcheckError>> {
                let first = partition * 16 / weights.len();
                Some(match self.fault {
                    0 => emit(first + 16 / weights.len(), self.cfg.one()),
                    1 => emit(usize::MAX, self.cfg.one()),
                    2 => emit(
                        first,
                        crate::piop::spartan::noncanonical_test_value(self.cfg),
                    ),
                    _ => unreachable!(),
                })
            }
        }
        let cfg = spartan_bitz_field_config();
        for fault in 0..3 {
            let source = InvalidFolded { cfg: &cfg, fault };
            let stream = StreamingMle::new(&source);
            macro_rules! check {
                ($k:expr) => {
                    assert!(
                        stream
                            .fold_prefix_table::<$k>(
                                5,
                                29,
                                &[cfg.one(); $k],
                                &cfg,
                                &cfg.zero(),
                                &cfg.one(),
                            )
                            .is_err()
                    );
                };
            }
            check!(0);
            check!(1);
            check!(3);
            check!(4);
        }
    }
}
