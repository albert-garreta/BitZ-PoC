//! Ordinary-round batching for the native integer prefix.
//!
//! A grid coordinate is 0, 1, or the leading coefficient (infinity). The norm
//! and A*B-C cofactors have degree two in each prefix variable. Rejection prefix
//! EQ factors stay outside that grid and are reinstated for each ordinary round.

use super::*;

pub(super) fn batched_norm_enabled() -> bool {
    #[cfg(feature = "bench-internals")]
    return match std::env::var("BITZ_ARITHMETIC_NATIVE_NORM") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) => match value.as_str() {
            "legacy" => false,
            "batched" => true,
            _ => panic!("BITZ_ARITHMETIC_NATIVE_NORM must be legacy or batched"),
        },
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("BITZ_ARITHMETIC_NATIVE_NORM must be valid Unicode")
        }
    };
    #[cfg(not(feature = "bench-internals"))]
    false
}

pub(super) struct Prefix {
    norm: Vec<F>,
    rejection: Vec<F>,
}

// Extend the Boolean values of a multilinear polynomial to the {0,1,infinity}
// tensor grid. Infinity means a difference, so every value stays native.
fn extend<const K: usize>(
    read: &impl Fn(usize) -> [i16; K],
    offset: usize,
    variables: usize,
    out: &mut [[i64; K]],
) {
    if variables == 0 {
        out[0] = read(offset).map(i64::from);
        return;
    }
    let third = out.len() / 3;
    extend(read, offset, variables - 1, &mut out[..third]);
    extend(
        read,
        offset + (1 << (variables - 1)),
        variables - 1,
        &mut out[third..2 * third],
    );
    for j in 0..third {
        out[2 * third + j] = std::array::from_fn(|k| out[third + j][k] - out[j][k]);
    }
}

impl Prefix {
    pub(super) fn new<const K: usize>(
        field: &Cfg,
        row_vars: usize,
        norm_instances: &[F],
        tau: Option<&[F]>,
        rejection_instances: Option<&[F]>,
        width: usize,
        read: &(impl Fn(usize) -> [i16; K] + Sync),
        batched_norm: bool,
    ) -> Result<Self, SumcheckError> {
        let _span = tracing::info_span!(
            "falcon_integer:native_prefix",
            component = "falcon_integer_native_prefix",
            tag_sumcheck = true,
            tag_constraint_proof = true,
            prefix_rounds = width as u64
        )
        .entered();
        if batched_norm && K == 1 && tau.is_none() && rejection_instances.is_none() {
            return Ok(Self::norm_only(
                field,
                row_vars,
                norm_instances,
                width,
                read,
            ));
        }
        let grid_len = 3usize.pow(width as u32);
        let block_len = 1 << width;
        let blocks = 1 << (row_vars - width);
        let rejection_tail = tau
            .map(|t| eq_table(&t[width..row_vars], field))
            .transpose()?;
        // C is multilinear, so its degree-two coefficient at any infinity
        // coordinate vanishes. The product of two multilinears does not.
        let finite: Vec<_> = (0..grid_len)
            .map(|mut i| {
                for _ in 0..width {
                    if i % 3 == 2 {
                        return false;
                    }
                    i /= 3;
                }
                true
            })
            .collect();
        let zero = field.zero();
        let group = |instance: usize| {
            let mut native_norm = [0i64; 81];
            let mut rejection = vec![zero; grid_len];
            let mut grid = [[0i64; K]; 81];
            for block in 0..blocks {
                extend(
                    read,
                    (instance * blocks + block) * block_len,
                    width,
                    &mut grid[..grid_len],
                );
                for j in 0..grid_len {
                    let x = grid[j][0];
                    // width <= 4 and row_vars <= 20 bound each sum by 2^54:
                    // (2^width * 2^15)^2 * 2^(row_vars-width).
                    native_norm[j] += x * x;
                    if K == 4 {
                        let residual =
                            grid[j][1] * grid[j][2] - if finite[j] { grid[j][3] } else { 0 };
                        let w = rejection_tail.as_ref().unwrap()[block];
                        let term = match residual {
                            0 => zero,
                            1 => w,
                            -1 => field.neg(&w),
                            value => field.reduce(field.mul_wide(&w, &value)),
                        };
                        rejection[j] = field.add(&rejection[j], &term);
                    }
                }
            }
            if let Some(weights) = rejection_instances {
                for value in &mut rejection {
                    *value = field.mul(value, &weights[instance]);
                }
            }
            Self {
                norm: native_norm[..grid_len]
                    .iter()
                    .map(|value| field.reduce(field.mul_wide(&norm_instances[instance], value)))
                    .collect(),
                rejection,
            }
        };
        let combine = |mut a: Self, b: Self| {
            for i in 0..grid_len {
                a.norm[i] = field.add(&a.norm[i], &b.norm[i]);
                a.rejection[i] = field.add(&a.rejection[i], &b.rejection[i]);
            }
            a
        };
        let empty = || Self {
            norm: vec![zero; grid_len],
            rejection: vec![zero; grid_len],
        };
        #[cfg(feature = "parallel")]
        let result = if norm_instances.len() * (1 << row_vars) >= 4096 {
            (0..norm_instances.len())
                .into_par_iter()
                .map(group)
                .reduce(empty, combine)
        } else {
            (0..norm_instances.len()).map(group).fold(empty(), combine)
        };
        #[cfg(not(feature = "parallel"))]
        let result = (0..norm_instances.len()).map(group).fold(empty(), combine);
        Ok(result)
    }

    /// Keep one fixed-size field accumulator per task, rather than allocating
    /// norm and zero rejection vectors for every instance. Each native sum is
    /// still reduced with its own instance weight before combining instances.
    fn norm_only<const K: usize>(
        field: &Cfg,
        row_vars: usize,
        norm_instances: &[F],
        width: usize,
        read: &(impl Fn(usize) -> [i16; K] + Sync),
    ) -> Self {
        const INSTANCES_PER_TASK: usize = 8;
        let grid_len = 3usize.pow(width as u32);
        let block_len = 1 << width;
        let blocks = 1 << (row_vars - width);
        let zero = field.zero();
        let task = |chunk: usize| {
            let mut totals = [zero; 81];
            let start = chunk * INSTANCES_PER_TASK;
            let end = (start + INSTANCES_PER_TASK).min(norm_instances.len());
            for instance in start..end {
                let mut native = [0i64; 81];
                let mut grid = [[0i64; K]; 81];
                for block in 0..blocks {
                    extend(
                        read,
                        (instance * blocks + block) * block_len,
                        width,
                        &mut grid[..grid_len],
                    );
                    for j in 0..grid_len {
                        let x = grid[j][0];
                        // The same per-instance 2^54 bound as the legacy path;
                        // neither task size nor batch size enters this sum.
                        native[j] += x * x;
                    }
                }
                for j in 0..grid_len {
                    let weighted =
                        field.reduce(field.mul_wide(&norm_instances[instance], &native[j]));
                    totals[j] = field.add(&totals[j], &weighted);
                }
            }
            totals
        };
        let combine = |mut a: [F; 81], b: [F; 81]| {
            for j in 0..grid_len {
                a[j] = field.add(&a[j], &b[j]);
            }
            a
        };
        let tasks = norm_instances.len().div_ceil(INSTANCES_PER_TASK);
        #[cfg(feature = "parallel")]
        let norm = if norm_instances.len() * (1 << row_vars) >= 4096 && tasks > 1 {
            (0..tasks)
                .into_par_iter()
                .map(task)
                .reduce(|| [zero; 81], combine)
        } else {
            (0..tasks).map(task).fold([zero; 81], combine)
        };
        #[cfg(not(feature = "parallel"))]
        let norm = (0..tasks).map(task).fold([zero; 81], combine);
        Self {
            norm: norm[..grid_len].to_vec(),
            rejection: Vec::new(),
        }
    }

    pub(super) fn coefficients(
        &self,
        field: &Cfg,
        rejection_point: Option<&[F]>,
    ) -> Result<[[F; 3]; 2], SumcheckError> {
        let mut width = 0;
        let mut len = self.norm.len() / 3;
        while len > 1 {
            width += 1;
            len /= 3;
        }
        let rejection_weights = rejection_point.map(|t| eq_table(t, field)).transpose()?;
        let mut sums = [[field.zero(); 3]; 2];
        for tail in 0..1usize << width {
            let mut ternary = 0;
            let mut stride = 3;
            for j in 0..width {
                ternary += ((tail >> j) & 1) * stride;
                stride *= 3;
            }
            for (component, values) in [&self.norm, &self.rejection].into_iter().enumerate() {
                if component == 1 && rejection_weights.is_none() {
                    continue;
                }
                let q = [
                    values[ternary],
                    field.sub(
                        &field.sub(&values[ternary + 1], &values[ternary]),
                        &values[ternary + 2],
                    ),
                    values[ternary + 2],
                ];
                for (k, mut value) in q.into_iter().enumerate() {
                    if component == 1 {
                        value = field.mul(&value, &rejection_weights.as_ref().unwrap()[tail]);
                    }
                    sums[component][k] = field.add(&sums[component][k], &value);
                }
            }
        }
        Ok(sums)
    }

    pub(super) fn fold(&mut self, field: &Cfg, challenge: F) {
        for values in [&mut self.norm, &mut self.rejection] {
            for i in 0..values.len() / 3 {
                let a = values[3 * i];
                let c = values[3 * i + 2];
                let b = field.sub(&field.sub(&values[3 * i + 1], &a), &c);
                values[i] = field.add(
                    &a,
                    &field.mul(&challenge, &field.add(&b, &field.mul(&challenge, &c))),
                );
            }
            values.truncate(values.len() / 3);
        }
    }
}

pub(super) fn fold_tables<const K: usize>(
    field: &Cfg,
    rounds: usize,
    previous: &[F],
    challenge: F,
    read: &(impl Fn(usize) -> [i16; K] + Sync),
) -> [Vec<F>; K] {
    let mut point = previous.to_vec();
    point.push(challenge);
    let weights = eq_table(&point, field).expect("validated prefix dimensions");
    match weights.len() {
        4 => fold_tables_with::<K, 4>(field, rounds, point.len(), &weights, read),
        8 => fold_tables_with::<K, 8>(field, rounds, point.len(), &weights, read),
        16 => fold_tables_with::<K, 16>(field, rounds, point.len(), &weights, read),
        _ => unreachable!("native prefix width is 2, 3, or 4"),
    }
}

// Byte-sized subset tables cover all Boolean folds with at most 8 KiB.
// These weights are shared by all three rejection columns and all instances.
fn boolean_tables<const N: usize>(field: &Cfg, weights: &[F; N]) -> [Vec<F>; 2] {
    std::array::from_fn(|side| {
        let start = (8 * side).min(N);
        let end = (start + 8).min(N);
        let mut values = vec![field.zero(); 1 << (end - start)];
        for mask in 1usize..values.len() {
            values[mask] = field.add(
                &values[mask & (mask - 1)],
                &weights[start + mask.trailing_zeros() as usize],
            );
        }
        values
    })
}

fn fold_tables_with<const K: usize, const N: usize>(
    field: &Cfg,
    rounds: usize,
    width: usize,
    weights: &[F],
    read: &(impl Fn(usize) -> [i16; K] + Sync),
) -> [Vec<F>; K] {
    let weights: [F; N] = std::array::from_fn(|i| weights[i]);
    let prepared =
        <Cfg as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(field, weights);
    let len = 1usize << (rounds - width);
    let boolean = (K == 4).then(|| boolean_tables(field, &weights));
    std::array::from_fn(|k| {
        let fold = |i: usize| {
            if k != 0 {
                if let Some(tables) = &boolean {
                    let values: [i16; N] = std::array::from_fn(|j| read(i * N + j)[k]);
                    // The Falcon caller supplies bits here, but the shared
                    // engine also accepts general i16 rejection operands.
                    if values.iter().all(|&value| value == 0 || value == 1) {
                        let mask = values
                            .iter()
                            .enumerate()
                            .fold(0usize, |mask, (j, &v)| mask | ((v as usize) << j));
                        return if N <= 8 {
                            tables[0][mask]
                        } else {
                            field.add(&tables[0][mask & 255], &tables[1][mask >> 8])
                        };
                    }
                    return <Cfg as PreparedLinearCombination<Z<1>>>::linear_combination(
                        &prepared,
                        |j| Z::from(values[j]),
                    );
                }
            }
            <Cfg as PreparedLinearCombination<Z<1>>>::linear_combination(&prepared, |j| {
                Z::from(read(i * N + j)[k])
            })
        };
        #[cfg(feature = "parallel")]
        if len >= 4096 {
            return (0..len).into_par_iter().map(fold).collect();
        }
        (0..len).map(fold).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batched_norm_matches_legacy_grids_and_all_prefix_rounds() {
        for modulus in [(1u128 << 61) - 1, (1u128 << 127) - 1] {
            let field = Cfg::from_prime_u128(modulus);
            for instances in [1, 8, 9, 17] {
                let weights: Vec<_> = (0..instances)
                    .map(|i| signed([1, -1, 97, 0][i % 4], &field))
                    .collect();
                let read = |i: usize| {
                    // Include zero instances and both signed endpoints. Keep
                    // instance16 nonzero so the parallel partial task matters.
                    if (13..16).contains(&(i / 256)) {
                        [0]
                    } else {
                        [[i16::MIN, i16::MAX, -1, 0, 1, 211][(i ^ (i >> 4)) % 6]]
                    }
                };
                for width in 2..=4 {
                    for challenge in [field.zero(), field.one(), signed(-239, &field)] {
                        let mut baseline =
                            Prefix::new(&field, 8, &weights, None, None, width, &read, false)
                                .unwrap();
                        let mut candidate =
                            Prefix::new(&field, 8, &weights, None, None, width, &read, true)
                                .unwrap();
                        assert!(candidate.rejection.is_empty());
                        assert!(baseline.rejection.iter().all(|x| *x == field.zero()));
                        for _ in 0..width {
                            assert_eq!(candidate.norm, baseline.norm);
                            assert_eq!(
                                candidate.coefficients(&field, None).unwrap(),
                                baseline.coefficients(&field, None).unwrap()
                            );
                            baseline.fold(&field, challenge);
                            candidate.fold(&field, challenge);
                        }
                        assert_eq!(candidate.norm, baseline.norm);
                    }
                }
            }
        }
    }

    #[test]
    fn signed_native_grid_matches_independent_differences_at_maximum_bound() {
        for width in 1..=4 {
            let read = |i: usize| {
                [if i.count_ones() % 2 == 0 {
                    i16::MIN
                } else {
                    i16::MAX
                }]
            };
            let mut grid = [[0i64; 1]; 81];
            let len = 3usize.pow(width as u32);
            extend(&read, 0, width, &mut grid[..len]);
            for (index, actual) in grid[..len].iter().enumerate() {
                let mut expected = 0i64;
                for bits in 0usize..1 << width {
                    let mut trits = index;
                    let mut sign = 1;
                    for bit in 0..width {
                        match (trits % 3, (bits >> bit) & 1) {
                            (0, 1) | (1, 0) => sign = 0,
                            (2, 0) => sign = -sign,
                            _ => {}
                        }
                        trits /= 3;
                    }
                    expected += sign * i64::from(read(bits)[0]);
                }
                assert_eq!(actual[0], expected);
                // Repeat the worst block through the maximum accepted row_vars.
                let squared_sum = i128::from(expected).pow(2) * (1i128 << (20 - width));
                assert!(squared_sum <= 1i128 << 54);
            }
        }
    }

    fn check_boolean_folds<const N: usize>() {
        let field = Cfg::from_prime_u128((1u128 << 127) - 1);
        let width = N.ilog2() as usize;
        let point: Vec<_> = (0..width)
            .map(|i| signed(3 + 2 * i as i64, &field))
            .collect();
        let weights = eq_table(&point, &field).unwrap();
        // Walking bits and complements cover both byte-table boundaries.
        // Remaining patterns combine bits across both halves of a 16-bit fold.
        let read = |i: usize| {
            let block = i / N;
            let j = i % N;
            std::array::from_fn(|k| {
                if k == 0 {
                    return ((i as u64).wrapping_mul(0x9e37_79b9) >> 16) as i16;
                }
                let mask = match block {
                    0 => 0,
                    1 => u16::MAX,
                    2..=17 => 1 << (block - 2),
                    18..=33 => !(1 << (block - 18)),
                    _ => (block as u16).wrapping_mul(0x9e37).rotate_left(k as u32),
                };
                if k == 3 && block % 11 == 0 && j == N - 1 {
                    // Check fallback after a prefix of valid bits, including
                    // negative values that must never become a lookup index.
                    return if block % 2 == 0 { i16::MIN } else { i16::MAX };
                }
                ((mask >> j) & 1) as i16
            })
        };
        let actual = fold_tables_with::<4, N>(&field, width + 7, width, &weights, &read);
        for block in 0..128 {
            for k in 0..4 {
                let expected = weights
                    .iter()
                    .enumerate()
                    .fold(field.zero(), |sum, (j, weight)| {
                        field.add(
                            &sum,
                            &field.mul(weight, &signed(i64::from(read(block * N + j)[k]), &field)),
                        )
                    });
                assert_eq!(
                    actual[k][block], expected,
                    "N={N}, block={block}, column={k}"
                );
            }
        }
    }

    #[test]
    fn boolean_prefix_folds_match_direct_signed_dot_products() {
        check_boolean_folds::<4>();
        check_boolean_folds::<8>();
        check_boolean_folds::<16>();
    }
}
