//! A tensor query plus a scaled streaming query over one packed bit table.
//!
//! The integer query is never scaled or mixed into the tensor entry by entry.
//! Linearity combines their round polynomials; both queries use the same bit
//! fold and the ordinary degree-two transcript codec.
use super::*;
use crate::sumcheck::inner::input;

pub(crate) struct FactoredOverlayInput<'a, S: ?Sized, H: ?Sized> {
    integer: &'a S,
    bits: &'a H,
    row: &'a [Field],
    column: &'a [Field],
    integer_scale: Field,
    num_vars: usize,
    live_len: usize,
    prefix: usize,
    contract_ring: bool,
}

impl<'a, S: ?Sized, H: ?Sized> FactoredOverlayInput<'a, S, H> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        integer: &'a S,
        bits: &'a H,
        row: &'a [Field],
        column: &'a [Field],
        integer_scale: Field,
        num_vars: usize,
        live_len: usize,
        prefix: usize,
    ) -> Self {
        #[cfg(feature = "parallel")]
        let threads = rayon::current_num_threads();
        #[cfg(not(feature = "parallel"))]
        let threads = 1;
        Self {
            integer,
            bits,
            row,
            column,
            integer_scale,
            num_vars,
            live_len,
            prefix,
            // Matched Falcon measurements favor contraction only for larger
            // single-thread batches; retain the streamed path otherwise.
            contract_ring: prefix == 3 && row.len() >= 32 && column.len() > 8 && threads == 1,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_ring_contraction_for_test(mut self, enabled: bool) -> Self {
        self.contract_ring = enabled && self.prefix == 3 && self.column.len() > 8;
        self
    }
}

pub(crate) struct OverlayState<'a, S: ?Sized, H: ?Sized, const K: usize> {
    input: FactoredOverlayInput<'a, S, H>,
    round: usize,
    accumulators: Option<PrefixAccumulators>,
    lagrange: Vec<Field>,
    point: [Field; SHA256_INNER_PREFIX_MAX_VARS],
    table: Option<CompactPrefixVTable>,
    prefix_weights: PreparedPrefixWeights,
    row: Vec<Field>,
    column: Vec<Field>,
    /// While local variables remain, sum of each signature's bit table
    /// weighted by the ring row. High-variable rounds use the shared table.
    contracted_bits: Option<Vec<Field>>,
    next: [Field; 2],
    stride: usize,
}

impl<'a, S: StreamingCoefficientSource + ?Sized, H: Sha256InnerBitSource + ?Sized, const K: usize>
    OverlayState<'a, S, H, K>
{
    fn new(input: FactoredOverlayInput<'a, S, H>, f: &FieldConfig) -> Result<Self, SumcheckError> {
        validate_inputs::<K, _>(input.num_vars, input.live_len, input.bits)?;
        // This representation covers a complete, aligned tensor domain. Any
        // inactive signatures are represented by zero rows and zero source bits.
        if !input.row.len().is_power_of_two()
            || !input.column.len().is_power_of_two()
            || input.column.len() < 1 << K
            || input.row.len().checked_mul(input.column.len()) != Some(1 << input.num_vars)
            || input.live_len != 1 << input.num_vars
            || input.integer.partition_len() != input.column.len()
            || input.integer.num_vars() != input.num_vars
            || input.integer.live_len() != input.live_len
        {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        validate_field_values(
            input
                .row
                .iter()
                .chain(input.column)
                .chain([&input.integer_scale]),
            f,
        )?;
        let source = StreamingMle::new(input.integer);
        source.validate_shape(input.live_len, f)?;
        let zero = f.zero();
        let accumulators = if K == 0 {
            None
        } else {
            let _span = tracing::info_span!("inner_overlay:prefix_accumulators").entered();
            let mut integer = source.build_prefix_accumulators::<K, _>(
                input.num_vars,
                input.live_len,
                input.bits,
                f,
                &zero,
            )?;
            let ring = ring_prefix::<K, _>(input.row, input.column, input.bits, f)?;
            for (integer, ring) in integer.rounds.iter_mut().zip(ring.rounds) {
                for (integer, ring) in integer.iter_mut().zip(ring) {
                    for (integer, ring) in integer.iter_mut().zip(ring) {
                        *integer = f.add(&ring, &f.mul(&input.integer_scale, integer));
                    }
                }
            }
            Some(integer)
        };
        let mut state = Self {
            input,
            round: 0,
            accumulators,
            lagrange: if K == 0 { Vec::new() } else { vec![f.one()] },
            point: [zero; SHA256_INNER_PREFIX_MAX_VARS],
            table: None,
            prefix_weights: PreparedPrefixWeights::default(),
            row: Vec::new(),
            column: Vec::new(),
            contracted_bits: None,
            next: [zero; 2],
            stride: 1,
        };
        if K == 0 {
            state.prepare_tail(f)?;
        }
        Ok(state)
    }

    #[tracing::instrument(skip_all, name = "inner_overlay:prepare_tail")]
    fn prepare_tail(&mut self, f: &FieldConfig) -> Result<(), SumcheckError> {
        self.accumulators = None;
        self.lagrange = Vec::new();
        let (zero, one) = (f.zero(), f.one());
        self.prefix_weights =
            PreparedPrefixWeights::new(equality_weights_lsb(&self.point[..K], &zero, &one, f), f);
        let mut table = StreamingMle::new(self.input.integer).fold_prefix_table::<K>(
            self.input.num_vars,
            self.input.live_len,
            &self.point[..K],
            f,
            &zero,
            &one,
        )?;
        self.row = self.input.row.to_vec();
        self.column = self
            .input
            .column
            .chunks_exact(1 << K)
            .map(|block| {
                let mut acc = product_accumulator_zero();
                for (value, weight) in block.iter().zip(self.prefix_weights.iter()) {
                    product_multiply_accumulate(f, &mut acc, value, weight);
                }
                product_reduce(acc, f)
            })
            .collect::<Result<_, _>>()?;
        if self.input.num_vars > K {
            if self.input.contract_ring {
                let (bits, integer) = contract_rows_and_sum_integer(
                    &table.values,
                    &self.row,
                    self.column.len(),
                    self.input.bits,
                    &self.prefix_weights,
                    f,
                )?;
                let ring = contracted_round(&self.column, &bits, f)?;
                self.next = combine_rounds(ring, integer, &self.input.integer_scale, f);
                self.contracted_bits = Some(bits);
            } else {
                let (fast, slow) = ring_axes(&self.row, &self.column);
                self.next = sum_pairs(
                    &mut table.values,
                    2,
                    fast,
                    slow,
                    &self.input.integer_scale,
                    f,
                    |pair, values| {
                        // Every input partition is complete, so both suffixes
                        // exist whenever a tail round remains.
                        let h0 = folded_packed_h::<K, _>(
                            pair * 2,
                            self.input.live_len,
                            self.input.bits,
                            &self.prefix_weights,
                            f,
                            &zero,
                            &one,
                        )?;
                        let h1 = folded_packed_h::<K, _>(
                            pair * 2 + 1,
                            self.input.live_len,
                            self.input.bits,
                            &self.prefix_weights,
                            f,
                            &zero,
                            &one,
                        )?;
                        Ok([
                            [field_from_raw(&values[0], f), h0],
                            [field_from_raw(&values[1], f), h1],
                        ])
                    },
                )?;
            }
        }
        self.table = Some(table);
        Ok(())
    }

    fn coefficients(&self, f: &FieldConfig) -> Result<[Field; 2], SumcheckError> {
        if self.round < K {
            let [c2, c0] = self.accumulators.as_ref().unwrap().evaluate_round(
                self.round,
                &self.lagrange,
                f,
            )?;
            Ok([c0, c2])
        } else {
            Ok(self.next)
        }
    }

    fn fold(&mut self, f: &FieldConfig, challenge: &Field) -> Result<(), SumcheckError> {
        let (zero, one) = (f.zero(), f.one());
        if self.round < K {
            self.point[self.round] = *challenge;
            extend_lagrange_coefficients(&mut self.lagrange, challenge, &one, &zero, f);
            self.round += 1;
            if self.round == K {
                self.prepare_tail(f)?;
            }
            return Ok(());
        }
        let _span = tracing::info_span!("inner_overlay:fold_tail", round = self.round).entered();
        if let Some(bits) = &mut self.contracted_bits {
            fold_factor(bits, challenge, f);
        }
        if self.column.len() > 1 {
            fold_factor(&mut self.column, challenge, f);
        } else {
            fold_factor(&mut self.row, challenge, f);
        }
        // The contraction sums over Boolean signature indices. Once those
        // become the sumcheck variables, use the individual bit evaluations
        // from the same table that the integer claim has been folding.
        if self.column.len() == 1 {
            self.contracted_bits = None;
        }
        let table = self.table.as_mut().unwrap();
        let more = self.round + 1 < self.input.num_vars;
        if !more {
            if self.round == K {
                fold_first_tail_round_in_place::<K, _>(
                    table,
                    self.input.live_len,
                    self.input.bits,
                    &self.prefix_weights,
                    challenge,
                    f,
                    &zero,
                    &one,
                )?;
            } else {
                fold_interleaved_in_place(&mut table.values, self.stride, challenge, f, &zero);
            }
        } else if let Some(bits) = &self.contracted_bits {
            let integer = if self.round == K {
                fold_first_tail_round_and_prepare_next_in_place::<K, _>(
                    table,
                    self.input.live_len,
                    self.input.bits,
                    &self.prefix_weights,
                    challenge,
                    f,
                    &zero,
                    &one,
                    f,
                )?
            } else {
                fold_interleaved_and_prepare_next_round_in_place(
                    &mut table.values,
                    self.stride,
                    challenge,
                    f,
                    &zero,
                )?
            };
            let ring = contracted_round(&self.column, bits, f)?;
            self.next = combine_rounds(ring, integer, &self.input.integer_scale, f);
        } else {
            let (fast, slow) = ring_axes(&self.row, &self.column);
            if self.round == K {
                let suffix_count = table.suffix_count;
                self.next = sum_pairs(
                    &mut table.values,
                    4,
                    fast,
                    slow,
                    &self.input.integer_scale,
                    f,
                    |pair, values| {
                        let (low, high) = values.split_at_mut(2);
                        let a = fold_first_tail_pair_in_place::<K, _>(
                            pair * 2,
                            low,
                            suffix_count,
                            self.input.live_len,
                            self.input.bits,
                            &self.prefix_weights,
                            challenge,
                            f,
                            &zero,
                            &one,
                        )?;
                        let b = fold_first_tail_pair_in_place::<K, _>(
                            pair * 2 + 1,
                            high,
                            suffix_count,
                            self.input.live_len,
                            self.input.bits,
                            &self.prefix_weights,
                            challenge,
                            f,
                            &zero,
                            &one,
                        )?;
                        Ok([a, b])
                    },
                )?;
                table.suffix_count /= 2;
            } else {
                let stride = self.stride;
                self.next = sum_pairs(
                    &mut table.values,
                    8 * stride,
                    fast,
                    slow,
                    &self.input.integer_scale,
                    f,
                    |_, values| {
                        let (low, high) = values.split_at_mut(4 * stride);
                        Ok([
                            fold_interleaved_chunk_in_place(low, stride, challenge, f, &zero),
                            fold_interleaved_chunk_in_place(high, stride, challenge, f, &zero),
                        ])
                    },
                )?;
            }
        }
        if self.round > K {
            self.stride *= 2;
        }
        self.round += 1;
        Ok(())
    }

    fn terminal(&self, f: &FieldConfig) -> Result<[Field; 2], SumcheckError> {
        let table = self.table.as_ref().unwrap();
        let integer = field_from_raw(&table.values[0], f);
        let bits = if self.input.num_vars == K {
            folded_packed_h::<K, _>(
                0,
                self.input.live_len,
                self.input.bits,
                &self.prefix_weights,
                f,
                &f.zero(),
                &f.one(),
            )?
        } else {
            field_from_raw(&table.values[1], f)
        };
        Ok([
            f.add(
                &f.mul(&self.row[0], &self.column[0]),
                &f.mul(&self.input.integer_scale, &integer),
            ),
            bits,
        ])
    }
}

fn ring_axes<'a>(row: &'a [Field], column: &'a [Field]) -> (&'a [Field], &'a [Field]) {
    if column.len() > 1 {
        (column, row)
    } else {
        (row, column)
    }
}

fn fold_factor(values: &mut Vec<Field>, challenge: &Field, f: &FieldConfig) {
    for pair in 0..values.len() / 2 {
        values[pair] = f.add(
            &values[2 * pair],
            &f.mul(challenge, &f.sub(&values[2 * pair + 1], &values[2 * pair])),
        );
    }
    values.truncate(values.len() / 2);
}

fn combine_rounds(
    ring: [Field; 2],
    integer: [Field; 2],
    scale: &Field,
    f: &FieldConfig,
) -> [Field; 2] {
    std::array::from_fn(|i| f.add(&ring[i], &f.mul(scale, &integer[i])))
}

/// Contract the signature axis after the three packed prefix challenges.
///
/// For byte b, H_s(prefix,j) = sum_l prefix_weight[l] bit_l(b). Build
/// row[s]*H_s(prefix,j) by subset additions from eight weighted entries.
/// This reuses the initial integer-round scan and never expands source bits.
#[tracing::instrument(skip_all, name = "inner_overlay:contract_ring_rows")]
fn contract_rows_and_sum_integer<H: Sha256InnerBitSource + ?Sized>(
    integer: &[RawMontgomery],
    rows: &[Field],
    local_suffixes: usize,
    bits: &H,
    prefix: &PreparedPrefixWeights,
    f: &FieldConfig,
) -> Result<(Vec<Field>, [Field; 2]), SumcheckError> {
    debug_assert_eq!(prefix.len(), 8);
    debug_assert_eq!(integer.len(), rows.len() * local_suffixes);
    debug_assert!(local_suffixes >= 2 && local_suffixes.is_power_of_two());
    let unweighted = prefix.byte_sums.as_ref().expect("three-variable prefix");
    // One 4-KiB lookup per signature, with bounded scratch independent of
    // batch size. Each task owns a disjoint range of contraction outputs.
    const ROW_BLOCK: usize = 64;
    const LOCAL_TILE: usize = 256;
    let mut contracted: Vec<_> = (0..local_suffixes)
        .map(|_| linear_accumulator_zero())
        .collect();
    let mut integer_sum = std::array::from_fn(|_| product_accumulator_zero());
    let mut lookups = Vec::with_capacity(ROW_BLOCK);
    for (block, row_weights) in rows.chunks(ROW_BLOCK).enumerate() {
        lookups.clear();
        lookups.extend(
            row_weights
                .iter()
                .map(|row| weighted_byte_sums(row, prefix, f)),
        );
        let first_row = block * ROW_BLOCK;
        let process = |(tile, accumulators): (usize, &mut [LinearAccumulator])| {
            let first_local = tile * LOCAL_TILE;
            let mut sums = std::array::from_fn(|_| product_accumulator_zero());
            for (row, lookup) in lookups.iter().enumerate() {
                let first = (first_row + row) * local_suffixes + first_local;
                for pair in 0..accumulators.len() / 2 {
                    let i = pair * 2;
                    let lo = bits.bits_at((first + i) * 8, 8)?;
                    let hi = bits.bits_at((first + i + 1) * 8, 8)?;
                    if lo > 255 || hi > 255 {
                        return Err(SumcheckError::InvalidProductDimensions);
                    }
                    let (lo, hi) = (lo as usize, hi as usize);
                    // Weighted lookup values are canonical Montgomery
                    // residues. A linear accumulator keeps their scale R.
                    linear_multiply_accumulate(f, &mut accumulators[i], &lookup[lo], &1u64);
                    linear_multiply_accumulate(f, &mut accumulators[i + 1], &lookup[hi], &1u64);
                    let (h0, h1) = (unweighted[lo], unweighted[hi]);
                    let z0 = field_from_raw(&integer[first + i], f);
                    let z1 = field_from_raw(&integer[first + i + 1], f);
                    product_multiply_accumulate(f, &mut sums[0], &z0, &h0);
                    product_multiply_accumulate(
                        f,
                        &mut sums[1],
                        &f.sub(&z1, &z0),
                        &f.sub(&h1, &h0),
                    );
                }
            }
            Ok::<_, SumcheckError>(sums)
        };
        let empty = || std::array::from_fn(|_| product_accumulator_zero());
        let serial = |accumulators: &mut [LinearAccumulator]| {
            accumulators
                .chunks_mut(LOCAL_TILE)
                .enumerate()
                .try_fold(empty(), |sum, tile| {
                    Ok::<_, SumcheckError>(merge_accumulators(sum, process(tile)?))
                })
        };
        #[cfg(feature = "parallel")]
        let sums =
            if local_suffixes * row_weights.len() >= 1 << 10 && rayon::current_num_threads() > 1 {
                contracted
                    .par_chunks_mut(LOCAL_TILE)
                    .enumerate()
                    .map(process)
                    .try_reduce(empty, |a, b| Ok(merge_accumulators(a, b)))?
            } else {
                serial(&mut contracted)?
            };
        #[cfg(not(feature = "parallel"))]
        let sums = serial(&mut contracted)?;
        integer_sum = merge_accumulators(integer_sum, sums);
    }
    // At Falcon's largest batch, each accumulator sums <=1024 residues
    // below a 126-bit modulus, hence is below 2^136. For any validated
    // 64-bit tensor, n<=63 and column>=16 imply rows<=2^59, so the sum
    // is below 2^187, within the 256-bit linear accumulator. Its 32-byte
    // representation needs 512 KiB at Falcon's 16,384 local suffixes.
    let contracted = crate::utils::cfg_into_iter!(contracted)
        .map(|sum| linear_reduce(sum, f))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((contracted, reduce_product_accumulators(integer_sum, f)?))
}

fn weighted_byte_sums(row: &Field, prefix: &[Field], f: &FieldConfig) -> [Field; 256] {
    debug_assert_eq!(prefix.len(), 8);
    let weights: [Field; 8] = std::array::from_fn(|i| f.mul(row, &prefix[i]));
    let mut sums = [f.zero(); 256];
    for pattern in 1usize..256 {
        sums[pattern] = f.add(
            &sums[pattern & (pattern - 1)],
            &weights[pattern.trailing_zeros() as usize],
        );
    }
    sums
}

/// Low-variable ring rounds need only two column-sized tables after the
/// signature contraction; no batch-sized ring products remain in this phase.
fn contracted_round(
    column: &[Field],
    bits: &[Field],
    f: &FieldConfig,
) -> Result<[Field; 2], SumcheckError> {
    debug_assert_eq!(column.len(), bits.len());
    debug_assert!(column.len() >= 2);
    let mut sums = std::array::from_fn(|_| product_accumulator_zero());
    for (column, bits) in column.chunks_exact(2).zip(bits.chunks_exact(2)) {
        product_multiply_accumulate(f, &mut sums[0], &column[0], &bits[0]);
        product_multiply_accumulate(
            f,
            &mut sums[1],
            &f.sub(&column[1], &column[0]),
            &f.sub(&bits[1], &bits[0]),
        );
    }
    reduce_product_accumulators(sums, f)
}

/// Form both polynomials while visiting each new [integer, bit] pair once.
/// A task stays inside one tensor row: the outer factor multiplies just two
/// reduced sums instead of every tensor entry. The bit fold runs in `read`.
#[allow(clippy::too_many_arguments)]
fn sum_pairs(
    values: &mut [RawMontgomery],
    pair_span: usize,
    fast: &[Field],
    slow: &[Field],
    scale: &Field,
    f: &FieldConfig,
    read: impl Fn(usize, &mut [RawMontgomery]) -> Result<[[Field; 2]; 2], SumcheckError> + Sync,
) -> Result<[Field; 2], SumcheckError> {
    let pairs_per_task = (fast.len() / 2).min(256);
    debug_assert!(pairs_per_task > 0);
    let process = |(task, chunk): (usize, &mut [RawMontgomery])| {
        let start = task * pairs_per_task;
        let first_index = (2 * start) & (fast.len() - 1);
        let mut integer = std::array::from_fn(|_| product_accumulator_zero());
        let mut ring = std::array::from_fn(|_| product_accumulator_zero());
        for (local, values) in chunk.chunks_exact_mut(pair_span).enumerate() {
            let pair = start + local;
            let [[z0, h0], [z1, h1]] = read(pair, values)?;
            let dh = f.sub(&h1, &h0);
            product_multiply_accumulate(f, &mut integer[0], &z0, &h0);
            product_multiply_accumulate(f, &mut integer[1], &f.sub(&z1, &z0), &dh);
            let index = first_index + 2 * local;
            product_multiply_accumulate(f, &mut ring[0], &fast[index], &h0);
            product_multiply_accumulate(
                f,
                &mut ring[1],
                &f.sub(&fast[index + 1], &fast[index]),
                &dh,
            );
        }
        let ring = reduce_product_accumulators(ring, f)?;
        let outer = &slow[(2 * start) / fast.len()];
        let mut weighted = std::array::from_fn(|_| product_accumulator_zero());
        for i in 0..2 {
            product_multiply_accumulate(f, &mut weighted[i], &ring[i], outer);
        }
        Ok::<_, SumcheckError>((integer, weighted))
    };
    let empty = || {
        (
            std::array::from_fn(|_| product_accumulator_zero()),
            std::array::from_fn(|_| product_accumulator_zero()),
        )
    };
    let merge = |(z0, r0), (z1, r1)| (merge_accumulators(z0, z1), merge_accumulators(r0, r1));
    let serial = |values: &mut [RawMontgomery]| {
        values
            .chunks_mut(pair_span * pairs_per_task)
            .enumerate()
            .try_fold(empty(), |sum, chunk| {
                Ok::<_, SumcheckError>(merge(sum, process(chunk)?))
            })
    };
    // Reduce worker accumulators directly: retaining one result for every
    // 256-pair task would add several MiB at the largest Falcon batch.
    #[cfg(feature = "parallel")]
    let (integer, ring) = if values.len() / pair_span >= 1 << 10 && rayon::current_num_threads() > 1
    {
        values
            .par_chunks_mut(pair_span * pairs_per_task)
            .enumerate()
            .map(process)
            .try_reduce(empty, |a, b| Ok(merge(a, b)))?
    } else {
        serial(values)?
    };
    #[cfg(not(feature = "parallel"))]
    let (integer, ring) = serial(values)?;
    let integer = reduce_product_accumulators(integer, f)?;
    let ring = reduce_product_accumulators(ring, f)?;
    Ok(combine_rounds(ring, integer, scale, f))
}

/// Reuse the streaming byte buckets for an unscaled copy of the small column.
/// Only the tiny prefix accumulator is multiplied by the corresponding row.
fn ring_prefix<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    row: &[Field],
    column: &[Field],
    bits: &H,
    f: &FieldConfig,
) -> Result<PrefixAccumulators, SumcheckError> {
    let zero = f.zero();
    let width = 1 << K;
    let blocks: Vec<_> = column
        .chunks_exact(width)
        .enumerate()
        .filter_map(|(i, block)| block.iter().any(|x| *x != zero).then_some(i * width))
        .collect();
    let source = ColumnSource {
        values: column,
        blocks: &blocks,
        width,
        f,
    };
    let parts: Vec<_> = crate::utils::cfg_iter!(row)
        .enumerate()
        .map(|(i, weight)| {
            let mut acc = if *weight == zero {
                PrefixAccumulators::new::<K>(&zero)
            } else {
                let offset = OffsetBits {
                    bits,
                    offset: i * column.len(),
                };
                StreamingMle::new(&source).build_prefix_accumulators::<K, _>(
                    column.len().ilog2() as usize,
                    column.len(),
                    &offset,
                    f,
                    &zero,
                )?
            };
            for value in acc.rounds.iter_mut().flatten().flatten() {
                *value = f.mul(value, weight);
            }
            Ok::<_, SumcheckError>(acc)
        })
        .collect();
    let mut result = PrefixAccumulators::new::<K>(&zero);
    for part in parts {
        for (dst, src) in result.rounds.iter_mut().zip(part?.rounds) {
            for (dst, src) in dst.iter_mut().zip(src) {
                for (dst, src) in dst.iter_mut().zip(src) {
                    *dst = f.add(dst, &src);
                }
            }
        }
    }
    Ok(result)
}

struct OffsetBits<'a, H: ?Sized> {
    bits: &'a H,
    offset: usize,
}
impl<H: Sha256InnerBitSource + ?Sized> Sha256InnerBitSource for OffsetBits<'_, H> {
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError> {
        self.bits.bit_at(self.offset + index)
    }
    fn bits_at(&self, index: usize, count: usize) -> Result<u64, SumcheckError> {
        self.bits.bits_at(self.offset + index, count)
    }
}

struct ColumnSource<'a> {
    values: &'a [Field],
    blocks: &'a [usize],
    width: usize,
    f: &'a FieldConfig,
}
impl StreamingCoefficientSource for ColumnSource<'_> {
    fn num_vars(&self) -> usize {
        self.values.len().ilog2() as usize
    }
    fn live_len(&self) -> usize {
        self.values.len()
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
    fn for_each_partition_block(
        &self,
        partition: usize,
        width: usize,
        emit: &mut impl FnMut(usize, &[Field]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if partition != 0 || width != self.width {
            return None;
        }
        Some((|| {
            for &base in self.blocks {
                emit(base, &self.values[base..base + width])?;
            }
            Ok(())
        })())
    }
    fn for_each_partition_byte_bucket(
        &self,
        partition: usize,
        read: &mut impl FnMut(usize, usize) -> Result<u8, SumcheckError>,
        emit: &mut impl FnMut(u8, &[Field; 8]) -> Result<(), SumcheckError>,
    ) -> Option<Result<(), SumcheckError>> {
        if partition != 0 || self.width != 8 {
            return None;
        }
        Some((|| {
            let mut buckets = vec![[self.f.zero(); 8]; 256];
            let mut occupied = [false; 256];
            for &base in self.blocks {
                let byte = usize::from(read(base, 8)?);
                if byte == 0 {
                    continue;
                }
                occupied[byte] = true;
                for (dst, value) in buckets[byte].iter_mut().zip(&self.values[base..base + 8]) {
                    *dst = self.f.add(dst, value);
                }
            }
            for (byte, values) in buckets.iter().enumerate() {
                if occupied[byte] {
                    emit(byte as u8, values)?;
                }
            }
            Ok(())
        })())
    }
}

pub(crate) enum State<'a, S: ?Sized, H: ?Sized> {
    K0(OverlayState<'a, S, H, 0>),
    K1(OverlayState<'a, S, H, 1>),
    K2(OverlayState<'a, S, H, 2>),
    K3(OverlayState<'a, S, H, 3>),
    K4(OverlayState<'a, S, H, 4>),
}
macro_rules! dispatch {
    ($state:expr, $s:ident, $body:expr) => {
        match $state {
            State::K0($s) => $body,
            State::K1($s) => $body,
            State::K2($s) => $body,
            State::K3($s) => $body,
            State::K4($s) => $body,
        }
    };
}
impl<S: ?Sized, H: ?Sized> input::sealed::Input for FactoredOverlayInput<'_, S, H> {}
impl<'a, S: StreamingCoefficientSource + ?Sized, H: Sha256InnerBitSource + ?Sized>
    input::Input<FieldConfig> for FactoredOverlayInput<'a, S, H>
{
    type Weights = ();
    type State = State<'a, S, H>;
    type Codec = input::Canonical;
    fn prepare(self, f: &FieldConfig, _: ()) -> Result<Self::State, SumcheckError> {
        Ok(match self.prefix {
            0 => State::K0(OverlayState::new(self, f)?),
            1 => State::K1(OverlayState::new(self, f)?),
            2 => State::K2(OverlayState::new(self, f)?),
            3 => State::K3(OverlayState::new(self, f)?),
            4 => State::K4(OverlayState::new(self, f)?),
            _ => return Err(SumcheckError::InvalidProductDimensions),
        })
    }
}
impl<S: StreamingCoefficientSource + ?Sized, H: Sha256InnerBitSource + ?Sized>
    input::State<FieldConfig> for State<'_, S, H>
{
    fn num_vars(&self) -> usize {
        dispatch!(self, s, s.input.num_vars)
    }
    fn validate_claim(&self, f: &FieldConfig, claim: &Field) -> Result<(), SumcheckError> {
        validate_field_value(claim, f)
    }
    fn coefficients(&self, f: &FieldConfig) -> Result<[Field; 2], SumcheckError> {
        dispatch!(self, s, s.coefficients(f))
    }
    fn fold(&mut self, f: &FieldConfig, challenge: &Field) -> Result<(), SumcheckError> {
        dispatch!(self, s, s.fold(f, challenge))
    }
    fn terminal(&self, f: &FieldConfig) -> Result<[Field; 2], SumcheckError> {
        dispatch!(self, s, s.terminal(f))
    }
}

#[cfg(test)]
mod tests;
