//! Aligned repeated coefficients followed by a compact unrelated circuit tail.
use super::*;
#[cfg(feature = "ecdsa")]
mod compact;
#[cfg(feature = "ecdsa")]
pub(crate) use compact::CompactCompositeMle;
use field::RingOps;

/// One prefix block's intersection with a geometric run (or a lone tail
/// column): tail columns `block · 2^K + lo .. block · 2^K + hi` hold
/// `weight · 2^(i - lo)`, i.e. `weight` is the value at `lo` and the run
/// doubles along the block.
#[derive(Clone, Debug)]
struct TailPiece {
    block: usize,
    lo: usize,
    hi: usize,
    weight: Field,
}

/// `ext[beta][i]` for `beta < 3^K`, `i < 2^K`: the ternary extension of the
/// point-mass at `i`, in the coordinate convention of [`extend_lsb`]
/// (per coordinate, `0` is the slope `high − low`, `1` the value at `0`,
/// `2` the value at `1`; `beta = Σ beta_m 3^m`, `i_m` the m-th bit of `i`).
fn ternary_extension_table<const K: usize>() -> Vec<[i8; 16]> {
    let prefix = 1usize << K;
    let mut table = vec![[0i8; 16]; pow3(K)];
    for (beta, row) in table.iter_mut().enumerate() {
        for (i, cell) in row.iter_mut().enumerate().take(prefix) {
            let mut value = 1i8;
            let mut rest = beta;
            for m in 0..K {
                let bit = (i >> m) & 1 == 1;
                let factor = match rest % 3 {
                    0 => {
                        if bit {
                            1
                        } else {
                            -1
                        }
                    }
                    1 => {
                        if bit {
                            0
                        } else {
                            1
                        }
                    }
                    _ => {
                        if bit {
                            1
                        } else {
                            0
                        }
                    }
                };
                value *= factor;
                rest /= 3;
            }
            *cell = value;
        }
    }
    table
}

/// Per-thread accumulation state for the run-structured tail pass: one
/// accumulator per (piece shape, bit position), where a shape is the
/// `(lo, hi)` range of a piece inside its block.
struct TailShapeState {
    sums: Vec<LinearAccumulator>,
    used: Vec<bool>,
}

impl TailShapeState {
    fn new<const K: usize>() -> Self {
        let prefix = 1usize << K;
        let shapes = (prefix + 1) * (prefix + 1);
        Self {
            sums: (0..shapes * prefix)
                .map(|_| linear_accumulator_zero())
                .collect(),
            used: vec![false; shapes],
        }
    }
}

fn shape_index<const K: usize>(lo: usize, hi: usize) -> usize {
    lo * ((1usize << K) + 1) + hi
}

/// Adds every piece of one block to the shape accumulators: the piece's
/// weight goes to `(shape, i)` for each set bit `i` of the block.
fn accumulate_tail_block<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    state: &mut TailShapeState,
    pieces: &[TailPiece],
    block: usize,
    tail_start: usize,
    tail_len: usize,
    h: &H,
    reducer: &field::FpCtx<2>,
) -> Result<(), SumcheckError> {
    let prefix = 1usize << K;
    let base = block * prefix;
    let active = prefix.min(tail_len - base);
    let mut set_bits = [0usize; 16];
    let mut count = 0;
    // Iterate the set bits without a branch per bit (the bits are random).
    let mut word = h.bits_at(tail_start + base, active)?;
    while word != 0 {
        set_bits[count] = word.trailing_zeros() as usize;
        count += 1;
        word &= word - 1;
    }
    if count == 0 {
        return Ok(());
    }
    for piece in pieces {
        let shape = shape_index::<K>(piece.lo, piece.hi);
        state.used[shape] = true;
        for &i in &set_bits[..count] {
            linear_multiply_accumulate(
                reducer,
                &mut state.sums[shape * prefix + i],
                &piece.weight,
                &1u64,
            );
        }
    }
    Ok(())
}

fn finish_tail_shapes<const K: usize>(state: TailShapeState, cfg: &FieldConfig, zero: &Field) -> Result<Vec<Field>, SumcheckError> {
    let prefix = 1usize << K;
    let shapes = (prefix + 1) * (prefix + 1);
    let TailShapeState { sums, used } = state;
    let mut reduced: Vec<Option<Vec<Field>>> = vec![None; shapes];
    let mut sums = sums.into_iter();
    for (shape, reduced) in reduced.iter_mut().enumerate() {
        let accumulators: Vec<LinearAccumulator> = sums.by_ref().take(prefix).collect();
        if used[shape] {
            *reduced = Some(
                accumulators
                    .into_iter()
                    .map(|acc| linear_reduce(acc, cfg))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
    }

    let ext = ternary_extension_table::<K>();
    let signed_field = |value: i64| -> Field {
        let magnitude = Field::from_with_cfg(value.unsigned_abs(), cfg);
        if value < 0 {
            cfg.sub(&(zero.clone()), &(&magnitude))
        } else {
            magnitude
        }
    };
    let mut beta_values = Vec::with_capacity(pow3(K));
    for row in &ext {
        // g_{lo,hi} = G[hi] − G[lo] with G[t] = Σ_{k<t} ext[k] · 2^k.
        let mut prefix_sums = [0i64; 17];
        for k in 0..prefix {
            prefix_sums[k + 1] = prefix_sums[k] + i64::from(row[k]) * (1i64 << k);
        }
        let mut total = zero.clone();
        for (shape, values) in reduced.iter().enumerate() {
            let Some(values) = values else { continue };
            let (lo, hi) = (shape / (prefix + 1), shape % (prefix + 1));
            // g_{lo,hi} = Σ_{lo ≤ k < hi} ext[k] · 2^(k − lo): the difference of prefix
            // sums is a multiple of 2^lo, so the division is exact.
            let g = (prefix_sums[hi] - prefix_sums[lo]) / (1i64 << lo);
            if g == 0 {
                continue;
            }
            let mut inner = linear_accumulator_zero();
            let mut touched = false;
            for (i, value) in values.iter().enumerate() {
                if row[i] != 0 {
                    linear_multiply_accumulate_signed(
                        cfg,
                        &mut inner,
                        value,
                        i64::from(row[i]),
                        zero,
                    );
                    touched = true;
                }
            }
            if touched {
                let inner = linear_reduce(inner, cfg)?;
                total = cfg.add(&(total), &(&(cfg.mul(&(signed_field(g)), &(&inner)))));
            }
        }
        beta_values.push(total);
    }
    Ok(beta_values)
}

/// Per-thread accumulation state for the weights-first SHA pass: one
/// accumulator per (instance block, bit position).
struct RepeatedState {
    sums: Vec<LinearAccumulator>,
}

/// The `3^K` prefix sums of the repeated part `V[j·N + i] = outer[j] · inner[i]`
/// (N instance weights per local wire, N a multiple of 2^K).
///
/// With blocks of 2^K consecutive instances, `Ṽ(beta, wire j, block p) = outer[j] ·
/// ũ_p(beta)` where `ũ_p` is the ternary extension of the inner factor's block
/// `p`, so the sum over wires factors through
/// `A[p][i] = Σ_j outer[j] · h[j·N + p·2^K + i]`: every bit costs one field
/// addition and the remaining `Σ_p ũ_p(beta) · Σ_i ext[beta][i] · A[p][i]` is
/// independent of the number of wires.
fn repeated_beta_values<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    outer: &[Field],
    inner: &[Field],
    h: &H,
    zero: &Field,
    reducer: &field::FpCtx<2>,
) -> Result<Vec<Field>, SumcheckError> {
    debug_assert!(K > 0 && K <= 4);
    let prefix = 1usize << K;
    let width = inner.len();
    debug_assert_eq!(width % prefix, 0);
    let positions = width / prefix;
    let cells = positions * prefix;
    let accumulate_wire = |state: &mut RepeatedState, wire: usize| -> Result<(), SumcheckError> {
        let weight = &outer[wire];
        let base = wire * width;
        for position in 0..positions {
            let mut word = h.bits_at(base + position * prefix, prefix)?;
            while word != 0 {
                let i = word.trailing_zeros() as usize;
                linear_multiply_accumulate(
                    reducer,
                    &mut state.sums[position * prefix + i],
                    weight,
                    &1u64,
                );
                word &= word - 1;
            }
        }
        Ok(())
    };
    let new_state = || RepeatedState {
        sums: (0..cells).map(|_| linear_accumulator_zero()).collect(),
    };
    #[cfg(feature = "parallel")]
    let state = if outer.len() >= 1 << 8 && rayon::current_num_threads() > 1 {
        (0..outer.len())
            .into_par_iter()
            .try_fold(new_state, |mut state, wire| -> Result<_, SumcheckError> {
                accumulate_wire(&mut state, wire)?;
                Ok(state)
            })
            .try_reduce(new_state, |mut left, right| {
                for (l, r) in left.sums.iter_mut().zip(right.sums) {
                    linear_merge(l, r);
                }
                Ok(left)
            })?
    } else {
        let mut state = new_state();
        for wire in 0..outer.len() {
            accumulate_wire(&mut state, wire)?;
        }
        state
    };
    #[cfg(not(feature = "parallel"))]
    let state = {
        let mut state = new_state();
        for wire in 0..outer.len() {
            accumulate_wire(&mut state, wire)?;
        }
        state
    };
    let table: Vec<Field> = state
        .sums
        .into_iter()
        .map(|acc| linear_reduce(acc, &reducer))
        .collect::<Result<Vec<_>, _>>()?;

    let ext = ternary_extension_table::<K>();
    let mut beta_values = Vec::with_capacity(pow3(K));
    for row in &ext {
        let mut total = zero.clone();
        for position in 0..positions {
            let block = &inner[position * prefix..(position + 1) * prefix];
            let cells = &table[position * prefix..(position + 1) * prefix];
            let mut u = linear_accumulator_zero();
            let mut a = linear_accumulator_zero();
            let mut touched = false;
            for i in 0..prefix {
                if row[i] != 0 {
                    linear_multiply_accumulate_signed(
                        reducer,
                        &mut u,
                        &block[i],
                        i64::from(row[i]),
                        zero,
                    );
                    linear_multiply_accumulate_signed(
                        reducer,
                        &mut a,
                        &cells[i],
                        i64::from(row[i]),
                        zero,
                    );
                    touched = true;
                }
            }
            if touched {
                let u = linear_reduce(u, &reducer)?;
                let a = linear_reduce(a, &reducer)?;
                total = reducer.add(&(total), &(&(reducer.mul(&(u), &(&a)))));
            }
        }
        beta_values.push(total);
    }
    Ok(beta_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closed-form ternary extension table agrees with `extend_lsb`.
    #[test]
    fn ternary_extension_table_matches_extend_lsb() {
        fn check<const K: usize>() {
            let prefix = 1usize << K;
            let table = ternary_extension_table::<K>();
            let mut values: Vec<i64> = (0..prefix).map(|i| (i as i64 * 7 + 3) % 11 - 5).collect();
            let original = values.clone();
            let mut scratch = Vec::new();
            extend_lsb::<i64, K, _>(&mut values, &mut scratch, &0, |high, low| *high - *low);
            for (beta, row) in table.iter().enumerate() {
                let expected: i64 = (0..prefix).map(|i| i64::from(row[i]) * original[i]).sum();
                assert_eq!(values[beta], expected, "K={K} beta={beta}");
            }
        }
        check::<1>();
        check::<2>();
        check::<3>();
        check::<4>();
    }

}
