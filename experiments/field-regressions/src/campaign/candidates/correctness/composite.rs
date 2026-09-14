use super::*;

fn f(x: Gf) -> F128 {
    F128::new(x.words()[0], x.words()[1])
}
fn gf(x: F128) -> Gf {
    Gf::from_words([x.lo, x.hi])
}

/// Direct affine elimination of adjacent table entries, without equality tables.
fn evaluate(table: &[F128], point: &[Gf]) -> F128 {
    assert_eq!(table.len(), 1 << point.len());
    let mut work = table.to_vec();
    for &z in point {
        work = work
            .chunks_exact(2)
            .map(|p| p[0] + arithmetic::oracle(p[0] + p[1], f(z)))
            .collect();
    }
    work[0]
}

fn check_ood(table: &[F128], point: &[Gf], expected: F128) {
    for result in [
        ood(table, point),
        ood_indexed(table, point),
        ood_collect(table, point),
        ood_vector(table, point),
        ood_prepared(table, point),
        ood_tiled_collect(table, point),
        ood_grouped(table, point),
        ood_tiled::<10>(table, point),
        ood_tiled::<8>(table, point),
        ood_tiled_vector::<10>(table, point),
        ood_tiled_vector::<11>(table, point),
    ] {
        assert_eq!(f(result), expected);
    }
    let lo = point.len().min(12);
    let mut tail = vec![Gf::one(); 1 << lo];
    let mut head = vec![Gf::one(); 1 << (point.len() - lo)];
    assert_eq!(f(ood_reuse(table, point, &mut tail, &mut head)), expected);
    // Scratch reuse must fully overwrite values from an earlier evaluation.
    assert_eq!(
        f(ood_indexed_reuse(table, point, &mut tail, &mut head)),
        expected
    );
}

#[test]
fn ood_matches_affine_elimination_and_boolean_selection() {
    for threads in [1, 10] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut rng = Rng(0x6f6f_645f_7465_7374);
                for log in [0, 1, 2, 3, 8, 11, 12, 13] {
                    let table = rng.values(1 << log);
                    let point: Vec<_> = rng.values(log).into_iter().map(gf).collect();
                    check_ood(&table, &point, evaluate(&table, &point));
                    for index in [
                        0,
                        table.len() / 2,
                        table.len() - 1,
                        4095.min(table.len() - 1),
                        4096.min(table.len() - 1),
                    ] {
                        let point: Vec<_> = (0..log)
                            .map(|i| gf(arithmetic::from_u128(((index >> i) & 1) as u128)))
                            .collect();
                        check_ood(&table, &point, table[index]);
                    }
                    if log <= 3 {
                        for index in 0..table.len() {
                            let mut one_hot = vec![F128::ZERO; table.len()];
                            one_hot[index] = F128::ONE;
                            check_ood(&one_hot, &point, evaluate(&one_hot, &point));
                        }
                    }
                }
            });
    }
}

#[test]
fn equality_weights_match_direct_tensor_products() {
    let mut rng = Rng(0x6571_7561_6c69_7479);
    for log in 0..=8 {
        let point: Vec<_> = rng.values(log).into_iter().map(gf).collect();
        let expected: Vec<_> = (0..1 << log)
            .map(|i| {
                point
                    .iter()
                    .enumerate()
                    .fold(F128::ONE, |weight, (bit, &z)| {
                        arithmetic::oracle(
                            weight,
                            if (i >> bit) & 1 == 0 {
                                F128::ONE + f(z)
                            } else {
                                f(z)
                            },
                        )
                    })
            })
            .map(gf)
            .collect();
        assert_eq!(eq(&point), expected);
        assert_eq!(eq_prepared(&point), expected);
    }
}

#[test]
fn packing_initializes_every_lane_and_padding_slot() {
    for threads in [1, 10] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for position_log in [0, 1, 4, 12] {
                    for a in 0..=3 {
                        for b in 0..=3 {
                            let g = crate::production_packing::Geometry {
                                lane_logs: [a, b],
                                virtual_lane_log: 4,
                                position_log,
                            };
                            let source = |branch: usize, width: usize| {
                                (0..(1 << position_log) * width)
                                    .map(|i| F128::new((i + 1) as u64, u64::MAX - (branch as u64)))
                                    .collect::<Vec<_>>()
                            };
                            let left = source(0, 1 << a);
                            let right = source(1, 1 << b);
                            let sources = [left.as_slice(), right.as_slice()];
                            let expected: Vec<_> = (0..(1 << position_log) * 16)
                                .map(|i| {
                                    let pos = i / 16;
                                    let lane = i % 16;
                                    let branch = lane / 8;
                                    let local = lane % 8;
                                    let width = 1 << g.lane_logs[branch];
                                    if local < width {
                                        sources[branch][pos * width + local]
                                    } else {
                                        F128::ZERO
                                    }
                                })
                                .collect();
                            assert_eq!(pack(&g, sources), expected);
                            assert_eq!(pack_tiled(&g, sources), expected);
                            assert_eq!(g.virtual_packed(sources), expected);
                            if position_log <= 4 {
                                let point = vec![
                                    Gf::from_words([0x12345678, 0xfedcba98]);
                                    position_log + 4
                                ];
                                assert_eq!(
                                    f(ood(&pack_tiled(&g, sources), &point)),
                                    evaluate(&expected, &point)
                                );
                            }
                        }
                    }
                }
            });
    }
}

/// Breadth-first scalar reference using only the independent field oracle.
/// The unchanged production plan supplies twiddles; candidate schedules do not.
fn reference_ntt(ntt: &AdditiveNttF128, input: &[F128], lanes: usize) -> Vec<F128> {
    let mut out = input.to_vec();
    let log = (input.len() / lanes).ilog2() as usize;
    for layer in 0..log {
        let block_positions = 1 << (log - layer);
        for block in 0..1 << layer {
            let t = ntt.twiddle(layer, block);
            for position in 0..block_positions / 2 {
                for lane in 0..lanes {
                    let lo = (block * block_positions + position) * lanes + lane;
                    let hi = lo + block_positions / 2 * lanes;
                    let a = out[lo] + arithmetic::oracle(out[hi], t);
                    let b = out[hi] + a;
                    out[lo] = a;
                    out[hi] = b;
                }
            }
        }
    }
    out
}

#[test]
fn ntt_schedules_preserve_lanes_bases_and_small_domains() {
    for threads in [1, 10] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut rng = Rng(0x6e74_745f_6261_7369);
                for (log, lanes) in [
                    (0, 1),
                    (0, 8),
                    (1, 1),
                    (1, 8),
                    (2, 2),
                    (3, 8),
                    (8, 1),
                    (8, 32),
                    (12, 8),
                ] {
                    for extra in [0, 2] {
                        for high in [false, true] {
                            let basis: Vec<_> = (0..log + extra)
                                .map(|i| F128::new(1 << i, if high { 1 << i } else { 0 }))
                                .collect();
                            let ntt = AdditiveNttF128::new(&basis);
                            for pattern in 0..3 {
                                let len = (1 << log) * lanes;
                                let input = match pattern {
                                    0 => vec![F128::ONE; len],
                                    1 => {
                                        let mut a = vec![F128::ZERO; len];
                                        a[len - 1] = F128::new(u64::MAX, u64::MAX);
                                        a
                                    }
                                    _ => rng.values(len),
                                };
                                let expected = reference_ntt(&ntt, &input, lanes);
                                let mut actual = input.clone();
                                half_depth(&ntt, &mut actual, lanes, 0, 0);
                                assert_eq!(actual, expected);
                                actual.copy_from_slice(&input);
                                tiled_ntt(&ntt, &mut actual, lanes);
                                assert_eq!(actual, expected);
                                actual.copy_from_slice(&input);
                                subtree(&ntt, &mut actual, lanes, 0, 0);
                                assert_eq!(actual, expected);
                                actual.copy_from_slice(&input);
                                incremental_ntt(&ntt, &mut actual, lanes);
                                assert_eq!(actual, expected);
                                actual.copy_from_slice(&input);
                                ntt.forward_transform_interleaved(&mut actual, lanes);
                                assert_eq!(actual, expected);
                                // Invert each lane independently to catch lane mixing/order errors.
                                for lane in 0..lanes {
                                    let mut values: Vec<_> = expected
                                        .iter()
                                        .skip(lane)
                                        .step_by(lanes)
                                        .copied()
                                        .collect();
                                    ntt.inverse_transform(&mut values);
                                    assert_eq!(
                                        values,
                                        input
                                            .iter()
                                            .skip(lane)
                                            .step_by(lanes)
                                            .copied()
                                            .collect::<Vec<_>>()
                                    );
                                }
                            }
                        }
                    }
                }
            });
    }
}

#[test]
#[should_panic]
fn packing_rejects_overwide_branch() {
    let g = crate::production_packing::Geometry {
        lane_logs: [4, 0],
        virtual_lane_log: 4,
        position_log: 0,
    };
    pack_tiled(&g, [&[F128::ZERO; 16], &[F128::ZERO; 1]]);
}

#[test]
#[should_panic]
fn packing_rejects_short_source() {
    let g = crate::production_packing::Geometry {
        lane_logs: [1, 0],
        virtual_lane_log: 4,
        position_log: 0,
    };
    pack_tiled(&g, [&[F128::ZERO; 1], &[F128::ZERO; 1]]);
}

#[test]
fn parallel_ood_tiles_preserve_boolean_indices_and_one_hot_weights() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(10)
        .build()
        .unwrap()
        .install(|| {
            let mut rng = Rng(0x7061_7261_6c6c_656c);
            for log in [17, 18] {
                let mut table = rng.values(1 << log);
                let check = |table: &[F128], point: &[Gf], expected: F128| {
                    for value in [
                        ood_tiled_collect(table, point),
                        ood_grouped(table, point),
                        ood_tiled::<8>(table, point),
                        ood_tiled::<10>(table, point),
                        ood_tiled_vector::<10>(table, point),
                        ood_tiled_vector::<11>(table, point),
                    ] {
                        assert_eq!(f(value), expected);
                    }
                };
                for index in [0, 255, 256, 1023, 1024, table.len() - 1] {
                    let point: Vec<_> = (0..log)
                        .map(|i| gf(arithmetic::from_u128(((index >> i) & 1) as u128)))
                        .collect();
                    check(&table, &point, table[index]);
                }
                table.fill(F128::ZERO);
                let point: Vec<_> = rng.values(log).into_iter().map(gf).collect();
                for index in [0, 1023, 1024, table.len() - 1] {
                    table[index] = F128::ONE;
                    let expected = point.iter().enumerate().fold(F128::ONE, |a, (i, &z)| {
                        arithmetic::oracle(
                            a,
                            f(z) + if (index >> i) & 1 == 0 {
                                F128::ONE
                            } else {
                                F128::ZERO
                            },
                        )
                    });
                    check(&table, &point, expected);
                    table[index] = F128::ZERO;
                }
            }
        });
}
