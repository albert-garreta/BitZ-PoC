use super::super::super::FalconAlgebraicWitness;
use super::*;
use crate::transcript::Blake3Transcript;
use field::Uint;

fn test_field() -> Cfg {
    F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
}

fn value(x: i64, f: &Cfg) -> F {
    let magnitude = F::from_with_cfg(x.unsigned_abs() as u128, f);
    if x < 0 { f.neg(&magnitude) } else { magnitude }
}

fn source_fixture(layout: &Layout) -> Source {
    // The binder accepts arbitrary signed15 words; norm validity belongs to the
    // outer proof. Extremes here exercise signed mixed products and bit 14.
    let cases = [-16384, -8193, -1, 0, 1, 8191, 16383];
    let witness = FalconAlgebraicWitness {
        s1: (0..layout.batch())
            .map(|i| std::array::from_fn(|j| cases[(j + i) % cases.len()]))
            .collect(),
        s2: (0..layout.batch())
            .map(|i| std::array::from_fn(|j| cases[(3 * j + i + 2) % cases.len()]))
            .collect(),
    };
    Source::new(
        *layout,
        &WitnessData {
            witness,
            slacks: (0..layout.batch())
                .map(|i| (0x0555_5555u64 ^ (i as u64 * 17)) & ((1u64 << SLACK_BITS) - 1))
                .collect(),
        },
    )
}

fn form_fixture<'a>(layout: &'a Layout, f: &'a Cfg) -> BindingForm<'a> {
    BindingForm {
        layout,
        field: f,
        coefficients: std::array::from_fn(|side| {
            (0..layout.batch() * N)
                .map(|j| value((j as i64 * 29 + side as i64 * 11) % 101 - 73, f))
                .collect()
        }),
        slacks: (0..layout.batch())
            .map(|i| {
                let weight = 17 + i as i64 * 31;
                value(if i % 2 == 0 { weight } else { -weight }, f)
            })
            .collect(),
        padding: value(-47, f),
        options: BindingOptions::default(),
        folded_decoder: Default::default(),
    }
}

fn dummy_norm(f: &Cfg) -> NormProof {
    NormProof {
        instance_nonce: None,
        slack: f.zero(),
        sumcheck: SumcheckProof {
            round_polynomials: Vec::new(),
        },
        terminal: f.zero(),
        grinding_nonces: Vec::new(),
    }
}

fn test_schedule(grinding: u32) -> Schedule {
    Schedule {
        norm_instance_bits: 0,
        norm_round_bits: 0,
        merge_bits: 0,
        binding_bits: grinding,
        word_binding: true,
        terminal_merge_bits: grinding,
    }
}

/// Materialize the mathematical statement from individual committed bits.
/// This intentionally does not call the optimized word decoder or weight code.
struct Reference {
    weights: [Vec<F>; 3],
    tables: [Vec<F>; 3],
}

impl Reference {
    fn new(form: &BindingForm<'_>, source: &Source, f: &Cfg) -> Self {
        let mut result = Self {
            weights: std::array::from_fn(|_| Vec::new()),
            tables: std::array::from_fn(|_| Vec::new()),
        };
        for c in 0..form.layout.source_bits() / 16 {
            let instance = c / (2 * N);
            let side = c / N % 2;
            let j = c % N;
            let active = instance < form.layout.batch();
            let d = (0..15)
                .filter(|&lane| source.bit(16 * c + lane))
                .map(|lane| {
                    if lane == 14 {
                        -(1i64 << lane)
                    } else {
                        1i64 << lane
                    }
                })
                .sum();
            let h = i64::from(source.bit(16 * c + 15));
            let count = (0..15).filter(|&lane| source.bit(16 * c + lane)).count() as i64;
            let wd = if active {
                form.coefficients[side][instance * N + j]
            } else {
                f.zero()
            };
            let wh = if active && side == 0 && j < SLACK_BITS {
                f.mul(&form.slacks[instance], &value(1i64 << j, f))
            } else {
                form.padding
            };
            let wc = if active { f.zero() } else { form.padding };
            for (k, (w, x)) in [wd, wh, wc].into_iter().zip([d, h, count]).enumerate() {
                result.weights[k].push(w);
                result.tables[k].push(value(x, f));
            }
        }
        result
    }

    fn target(&self, f: &Cfg) -> F {
        self.weights
            .iter()
            .zip(&self.tables)
            .flat_map(|(w, t)| w.iter().zip(t))
            .fold(f.zero(), |sum, (w, x)| f.add(&sum, &f.mul(w, x)))
    }

    fn polynomial(&self, f: &Cfg) -> [F; 3] {
        let mut sum = [f.zero(); 3];
        for (weights, table) in self.weights.iter().zip(&self.tables) {
            for (w, x) in weights.chunks_exact(2).zip(table.chunks_exact(2)) {
                let dw = f.sub(&w[1], &w[0]);
                let dx = f.sub(&x[1], &x[0]);
                let terms = [
                    f.mul(&w[0], &x[0]),
                    f.add(&f.mul(&w[0], &dx), &f.mul(&dw, &x[0])),
                    f.mul(&dw, &dx),
                ];
                for k in 0..3 {
                    sum[k] = f.add(&sum[k], &terms[k]);
                }
            }
        }
        sum
    }

    fn fold(&mut self, r: F, f: &Cfg) {
        for table in self.weights.iter_mut().chain(&mut self.tables) {
            *table = table
                .chunks_exact(2)
                .map(|x| f.add(&f.mul(&f.sub(&f.one(), &r), &x[0]), &f.mul(&r, &x[1])))
                .collect();
        }
    }
}

#[test]
fn word_proof_matches_dense_reference_rounds_and_terminals() {
    let f = test_field();
    for (batch, grinding) in [(1, 0), (3, 0), (3, 2)] {
        let layout = Layout::new(batch).unwrap();
        let source = source_fixture(&layout);
        let form = form_fixture(&layout, &f);
        let mut reference = Reference::new(&form, &source, &f);
        let target = reference.target(&f);
        let schedule = test_schedule(grinding);
        let mut prover_transcript = Blake3Transcript::new();
        let (proof, row_weights, bridge) = prove(
            &mut prover_transcript,
            &layout,
            &source,
            form,
            target,
            dummy_norm(&f),
            None,
            &schedule,
            &f,
        )
        .unwrap();

        let mut replay = Blake3Transcript::new();
        replay.absorb_slice(DOMAIN);
        let mut boundary =
            ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(grinding, 0);
        for (round, polynomial) in proof.binding.round_polynomials.iter().enumerate() {
            assert_eq!(
                *polynomial,
                reference.polynomial(&f),
                "batch={batch}, round={round}"
            );
            absorb_field_elements(&mut replay, polynomial, &f);
            boundary.after_round(&mut replay, round).unwrap();
            let r = sample(&mut replay, &f).unwrap();
            assert_eq!(r, bridge.point[round]);
            reference.fold(r, &f);
        }
        assert_eq!(boundary.into_nonces(), proof.binding_nonces);
        let terminal_count = if batch == 1 { 2 } else { 3 };
        assert_eq!(proof.binding_terminal.len(), terminal_count);
        assert_eq!(
            proof.binding.round_polynomials.len(),
            COEFFICIENT_LOG + 1 + layout.capacity().ilog2() as usize
        );
        for k in 0..terminal_count {
            assert_eq!(proof.binding_terminal[k], reference.tables[k][0]);
        }
        assert_eq!(row_weights, bridge.row_weights(&layout, &f).unwrap());
        let verifier_form = form_fixture(&layout, &f);
        let mut verifier_transcript = Blake3Transcript::new();
        assert_eq!(
            verify(
                &mut verifier_transcript,
                &layout,
                &verifier_form,
                target,
                &proof,
                &schedule,
                &f
            )
            .unwrap(),
            bridge
        );
        assert_eq!(
            verifier_transcript.state_digest(),
            prover_transcript.state_digest()
        );

        for k in 0..terminal_count {
            let mut altered = proof.clone();
            altered.binding_terminal[k] = f.add(&altered.binding_terminal[k], &f.one());
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &verifier_form,
                    target,
                    &altered,
                    &schedule,
                    &f
                )
                .is_err()
            );
        }
        let mut wrong_shape = proof.clone();
        if terminal_count == 2 {
            wrong_shape.binding_terminal.push(f.zero());
        } else {
            wrong_shape.binding_terminal.pop();
        }
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &verifier_form,
                target,
                &wrong_shape,
                &schedule,
                &f
            )
            .is_err()
        );
    }
}

#[test]
fn first_word_fold_matches_independent_signed_bit_reference() {
    for modulus in [(1u128 << 61) - 1, (1u128 << 127) - 1] {
        let f = F::make_cfg(&Uint::from(modulus)).unwrap();
        for batch in [1, 3, 17] {
            let layout = Layout::new(batch).unwrap();
            let source = source_fixture(&layout);
            let form = form_fixture(&layout, &f);
            for r in [
                f.zero(),
                f.one(),
                value(-1, &f),
                F::from_with_cfg(modulus / 2, &f),
            ] {
                let mut reference = Reference::new(&form, &source, &f);
                reference.fold(r, &f);
                let expected: Vec<_> = reference.weights[0]
                    .iter()
                    .zip(&reference.tables[0])
                    .map(|(&w, &d)| [w, d])
                    .collect();
                let expected_sums =
                    expected
                        .chunks_exact(2)
                        .fold([f.zero(); 2], |mut sum, pair| {
                            sum[0] = f.add(&sum[0], &f.mul(&pair[0][0], &pair[0][1]));
                            sum[1] = f.add(
                                &sum[1],
                                &f.mul(
                                    &f.sub(&pair[1][0], &pair[0][0]),
                                    &f.sub(&pair[1][1], &pair[0][1]),
                                ),
                            );
                            sum
                        });
                let actual = first_fold(&form, &source, r, &f);
                assert_eq!(actual.0, expected, "modulus={modulus}, batch={batch}");
                assert_eq!(actual.1, expected_sums, "modulus={modulus}, batch={batch}");
            }
        }
    }
}

#[test]
fn word_kernels_match_dense_reference_at_boolean_challenges() {
    let f = test_field();
    let layout = Layout::new(3).unwrap();
    let source = source_fixture(&layout);
    for mode in 0..3 {
        let form = form_fixture(&layout, &f);
        let mut reference = Reference::new(&form, &source, &f);
        let mut weights = PublicWeights::new(&form);
        let mut spare = Sparse::from_source(&source, &layout, &f).unwrap();
        let mut aggregate_spare = Some(AggregateSpare::new(&spare, &weights, &f));
        let (mut next, _) = first_round(&form, &source, &f);
        let mut dense = Vec::new();
        let mut stride = 1;
        let mut point = Vec::new();
        let mut count = None;
        for round in 0..rounds(&layout) {
            if round == COEFFICIENT_LOG + 1 {
                count = Some(counts(&source, &layout, &point, &f).unwrap());
                assert_eq!(count.as_ref().unwrap(), &reference.tables[2]);
            }
            let spare_sums = aggregate_spare.as_ref().map_or_else(
                || spare.sums(&weights, round, &f),
                |aggregate| aggregate.sums(&weights, round, &f),
            );
            let mut actual = add_pair(next, spare_sums, &f);
            if let Some(c) = &count {
                actual = add_pair(actual, count_sums(c, &weights, &f), &f);
            }
            let expected = reference.polynomial(&f);
            assert_eq!(
                actual,
                [expected[0], expected[2]],
                "mode={mode}, round={round}"
            );
            let r = match mode {
                0 => f.zero(),
                1 => f.one(),
                _ => value(if round % 2 == 0 { -3 } else { 5 }, &f),
            };
            point.push(r);
            if round == 0 {
                (dense, next) = first_fold(&form, &source, r, &f);
            } else {
                next = dense_fold(&mut dense, stride, r, &f);
                stride *= 2;
            }
            if round < COEFFICIENT_LOG {
                aggregate_spare.as_mut().unwrap().fold(r, &f);
            } else if round == COEFFICIENT_LOG {
                spare.project_instances(&point, &f).unwrap();
                aggregate_spare = None;
            } else {
                spare.fold(r, &f);
            }
            if let Some(c) = &mut count {
                fold_small(c, r, &f);
            }
            weights.fold(round, r, &f);
            reference.fold(r, &f);
        }
        assert_eq!(dense[0], [reference.weights[0][0], reference.tables[0][0]]);
        assert_eq!(spare.terminal(&f), reference.tables[1][0]);
        assert_eq!(
            weights.spare(0, rounds(&layout), &f),
            reference.weights[1][0]
        );
        assert_eq!(weights.count(0, &f), reference.weights[2][0]);
        assert_eq!(count.unwrap()[0], reference.tables[2][0]);
    }
}

#[test]
fn raw_word_masks_and_bridge_cover_malformed_padding() {
    let f = test_field();
    for batch in [1, 3] {
        let layout = Layout::new(batch).unwrap();
        let mut source = source_fixture(&layout);
        // Unused spare lanes on both sides and, when present, low/sign/spare
        // lanes of inactive instances must retain their original bit weights.
        for c in [SLACK_BITS, N, 2 * N - 1] {
            source.flip(16 * c + 15);
        }
        if batch < layout.capacity() {
            for lane in [0, 14, 15] {
                source.flip(16 * batch * 2 * N + lane);
            }
        }
        assert!(Sparse::from_source(&source, &layout, &f).is_err());
        let form = form_fixture(&layout, &f);
        let mut reference = Reference::new(&form, &source, &f);
        let mut bit_target = f.zero();
        for bit in 0..layout.source_bits() {
            if !source.bit(bit) {
                continue;
            }
            let c = bit / 16;
            let lane = bit % 16;
            let i = c / (2 * N);
            let local = c % (2 * N);
            let weight = if i >= batch {
                form.padding
            } else if lane == 15 {
                if local < SLACK_BITS {
                    f.mul(&form.slacks[i], &value(1i64 << local, &f))
                } else {
                    form.padding
                }
            } else {
                let digit = if lane == 14 {
                    -(1i64 << lane)
                } else {
                    1i64 << lane
                };
                f.mul(
                    &form.coefficients[local / N][i * N + local % N],
                    &value(digit, &f),
                )
            };
            bit_target = f.add(&bit_target, &weight);
        }
        assert_eq!(reference.target(&f), bit_target);

        let point: Vec<_> = (0..rounds(&layout))
            .map(|j| value(j as i64 * 3 - 17, &f))
            .collect();
        for &r in &point {
            reference.fold(r, &f);
        }
        let zeta = value(-7, &f);
        let mut expected = f.add(
            &reference.tables[0][0],
            &f.mul(&zeta, &reference.tables[1][0]),
        );
        if batch < layout.capacity() {
            expected = f.add(
                &expected,
                &f.mul(&f.mul(&zeta, &zeta), &reference.tables[2][0]),
            );
        }
        let bridge = WordBridgeClaim {
            point,
            modulus: f.modulus_u128(),
            value: expected,
            zeta,
            with_padding: batch < layout.capacity(),
        };
        let rows = bridge.row_weights(&layout, &f).unwrap();
        let columns = eq_table(bridge.column_point(&layout), &f).unwrap();
        let mut actual = f.zero();
        for bit in 0..layout.source_bits() {
            if source.bit(bit) {
                actual = f.add(
                    &actual,
                    &f.mul(
                        &F::from_with_cfg(rows[bit % rows.len()], &f),
                        &columns[bit / rows.len()],
                    ),
                );
            }
        }
        assert_eq!(actual, expected);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_sparse_source_scan_preserves_entries_and_padding_errors() {
    let f = test_field();
    let layout = Layout::new(65).unwrap();
    let source = source_fixture(&layout);
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let parallel = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    let expected = serial.install(|| Sparse::from_source(&source, &layout, &f).unwrap().0);
    let actual = parallel.install(|| Sparse::from_source(&source, &layout, &f).unwrap().0);
    assert_eq!(actual, expected);
    assert!(actual.windows(2).all(|pair| pair[0].0 < pair[1].0));

    let spare_padding = 16 * (17 * 2 * N + N) + 15;
    let inactive_padding = 16 * (layout.batch() * 2 * N) + 14;
    for flips in [
        vec![spare_padding],
        vec![inactive_padding],
        vec![spare_padding, inactive_padding],
    ] {
        let mut malformed = source.clone();
        for bit in flips {
            malformed.flip(bit);
        }
        let serial_error = serial.install(|| {
            Sparse::from_source(&malformed, &layout, &f)
                .err()
                .unwrap()
                .to_string()
        });
        let parallel_error = parallel.install(|| {
            Sparse::from_source(&malformed, &layout, &f)
                .err()
                .unwrap()
                .to_string()
        });
        assert_eq!(parallel_error, serial_error);
    }
}

fn factored_fixture<'a>(
    layout: &'a Layout,
    f: &'a Cfg,
    mode: u8,
) -> (BindingForm<'a>, WordFactors, BindingForm<'a>) {
    factored_fixture_with_norm(layout, f, mode, value(-29, f), value(-37, f))
}

fn factored_fixture_with_norm<'a>(
    layout: &'a Layout,
    f: &'a Cfg,
    mode: u8,
    eta: F,
    first: F,
) -> (BindingForm<'a>, WordFactors, BindingForm<'a>) {
    let mut ring = form_fixture(layout, f);
    let mut dense = form_fixture(layout, f);
    let mut factors = WordFactors::default();
    if mode & 2 != 0 {
        let rows: Vec<_> = (0..layout.batch())
            .map(|i| value(i as i64 * 7 - 11, f))
            .collect();
        let columns: Vec<_> = (0..N).map(|j| value(j as i64 % 31 - 17, f)).collect();
        for i in 0..layout.batch() {
            for j in 0..N {
                dense.coefficients[0][i * N + j] = f.mul(&rows[i], &columns[j]);
            }
        }
        ring.coefficients[0].clear();
        factors.s1 = Some(FactoredS1 { rows, columns });
    }
    if mode & 1 != 0 {
        let norm = NormClaims {
            instance_point: vec![f.zero(); layout.capacity().ilog2() as usize],
            point: (0..rounds(layout))
                .map(|i| {
                    if i == 0 {
                        first
                    } else {
                        value(i as i64 * 13 - 37, f)
                    }
                })
                .collect(),
            terminal: f.zero(),
            slack: f.zero(),
        };
        let weights = eq_table(&norm.point, f).unwrap();
        for side in 0..2 {
            for i in 0..layout.batch() {
                for j in 0..N {
                    let correction = f.mul(&eta, &weights[i * 2 * N + side * N + j]);
                    dense.coefficients[side][i * N + j] =
                        f.add(&dense.coefficients[side][i * N + j], &correction);
                }
            }
        }
        factors.norm = Some(FactoredNorm::new(layout, &norm, eta, f).unwrap());
    }
    (ring, factors, dense)
}

#[test]
fn deferred_norm_handles_zero_scale_and_boolean_first_coordinate() {
    let f = test_field();
    let layout = Layout::new(3).unwrap();
    let source = source_fixture(&layout);
    for mode in [1, 3] {
        for eta in [f.zero(), value(-29, &f)] {
            for first in [f.zero(), f.one()] {
                let (form, factors, dense) =
                    factored_fixture_with_norm(&layout, &f, mode, eta, first);
                let reference = Reference::new(&dense, &source, &f);
                let mut initial = f.zero();
                let mut sums = [f.zero(); 2];
                for (w, x) in reference.weights[0]
                    .chunks_exact(2)
                    .zip(reference.tables[0].chunks_exact(2))
                {
                    let even = f.mul(&w[0], &x[0]);
                    initial = f.add(&initial, &f.add(&even, &f.mul(&w[1], &x[1])));
                    sums[0] = f.add(&sums[0], &even);
                    sums[1] = f.add(&sums[1], &f.mul(&f.sub(&w[1], &w[0]), &f.sub(&x[1], &x[0])));
                }
                for reuse in [false, true] {
                    assert_eq!(
                        first_round_with_factors(&form, &source, &factors, &f, reuse),
                        (sums, initial),
                        "mode={mode}, eta={eta:?}, first={first:?}, reuse={reuse}"
                    );
                    if let Some(s1) = &factors.s1 {
                        assert_eq!(
                            first_round_factored_fused(
                                &form,
                                &source,
                                s1,
                                factors.norm.as_ref().unwrap(),
                                &f,
                                reuse,
                            ),
                            (sums, initial),
                        );
                    }
                }
                for r in [f.zero(), f.one(), value(-3, &f)] {
                    let expected = first_fold_with_factors(
                        &dense,
                        &source,
                        r,
                        &WordFactors::default(),
                        &f,
                        false,
                    );
                    for fused in [false, true] {
                        assert_eq!(
                            first_fold_with_factors(&form, &source, r, &factors, &f, fused),
                            expected,
                            "mode={mode}, eta={eta:?}, first={first:?}, fused={fused}"
                        );
                        let mut values = Vec::new();
                        let sums = first_fold_into(
                            &form,
                            &source,
                            r,
                            &factors,
                            &f,
                            WordOptions {
                                fused_fold: fused,
                                direct_init: true,
                                factored_fold_reduction: true,
                                ..WordOptions::default()
                            },
                            &mut values,
                        );
                        assert_eq!(
                            (values, sums),
                            expected,
                            "combined reduction: mode={mode}, eta={eta:?}, first={first:?}, fused={fused}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn factored_word_kernels_match_dense_signed_bit_reference() {
    for modulus in [(1u128 << 61) - 1, (1u128 << 127) - 1] {
        let f = F::make_cfg(&Uint::from(modulus)).unwrap();
        for batch in [1, 3, 17] {
            let layout = Layout::new(batch).unwrap();
            let source = source_fixture(&layout);
            for mode in 0..4 {
                let (form, factors, dense) = factored_fixture(&layout, &f, mode);
                let reference = Reference::new(&dense, &source, &f);
                let mut initial = f.zero();
                let mut sums = [f.zero(); 2];
                for (w, x) in reference.weights[0]
                    .chunks_exact(2)
                    .zip(reference.tables[0].chunks_exact(2))
                {
                    let even = f.mul(&w[0], &x[0]);
                    initial = f.add(&initial, &f.add(&even, &f.mul(&w[1], &x[1])));
                    sums[0] = f.add(&sums[0], &even);
                    sums[1] = f.add(&sums[1], &f.mul(&f.sub(&w[1], &w[0]), &f.sub(&x[1], &x[0])));
                }
                for reuse in [false, true] {
                    assert_eq!(
                        first_round_with_factors(&form, &source, &factors, &f, reuse),
                        (sums, initial)
                    );
                    if let (Some(s1), Some(norm)) = (&factors.s1, &factors.norm) {
                        assert_eq!(
                            first_round_factored_fused(&form, &source, s1, norm, &f, reuse),
                            (sums, initial),
                        );
                    }
                }
                for r in [
                    f.zero(),
                    f.one(),
                    value(-1, &f),
                    F::from_with_cfg(modulus / 2, &f),
                ] {
                    let mut reference = Reference::new(&dense, &source, &f);
                    reference.fold(r, &f);
                    let expected: Vec<_> = reference.weights[0]
                        .iter()
                        .zip(&reference.tables[0])
                        .map(|(&w, &d)| [w, d])
                        .collect();
                    let expected_sums =
                        expected
                            .chunks_exact(2)
                            .fold([f.zero(); 2], |mut sum, pair| {
                                sum[0] = f.add(&sum[0], &f.mul(&pair[0][0], &pair[0][1]));
                                sum[1] = f.add(
                                    &sum[1],
                                    &f.mul(
                                        &f.sub(&pair[1][0], &pair[0][0]),
                                        &f.sub(&pair[1][1], &pair[0][1]),
                                    ),
                                );
                                sum
                            });
                    let mut values = vec![[value(-91, &f); 2]; expected.len() + 16];
                    for options in [
                        WordOptions::default(),
                        WordOptions {
                            direct_init: true,
                            ..WordOptions::default()
                        },
                        WordOptions {
                            factored_fold_reduction: true,
                            ..WordOptions::default()
                        },
                        WordOptions {
                            fused_fold: true,
                            direct_init: true,
                            factored_fold_reduction: true,
                            ..WordOptions::default()
                        },
                    ] {
                        let sums =
                            first_fold_into(&form, &source, r, &factors, &f, options, &mut values);
                        assert_eq!(values, expected, "batch={batch} mode={mode} {options:?}");
                        assert_eq!(sums, expected_sums);
                    }
                }
            }
        }
    }
}

#[test]
fn factored_word_proofs_preserve_transcript_and_reject_tampering() {
    let f = test_field();
    for (batch, grinding) in [(1, 0), (3, 0), (3, 2), (16, 0)] {
        let layout = Layout::new(batch).unwrap();
        let source = source_fixture(&layout);
        let schedule = test_schedule(grinding);
        for mode in 1..4 {
            let (form, factors, dense) = factored_fixture(&layout, &f, mode);
            let target = Reference::new(&dense, &source, &f).target(&f);
            let mut baseline = Blake3Transcript::new();
            let expected = prove(
                &mut baseline,
                &layout,
                &source,
                dense,
                target,
                dummy_norm(&f),
                None,
                &schedule,
                &f,
            )
            .unwrap();
            let mut transcript = Blake3Transcript::new();
            let actual = prove_with_correction(
                &mut transcript,
                &layout,
                &source,
                form,
                factors,
                target,
                dummy_norm(&f),
                None,
                &schedule,
                &f,
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(transcript.state_digest(), baseline.state_digest());
            let (form, factors, _) = factored_fixture(&layout, &f, mode);
            let mut verifier = Blake3Transcript::new();
            assert_eq!(
                verify_with_factors(
                    &mut verifier,
                    &layout,
                    &form,
                    &factors,
                    target,
                    &actual.0,
                    &schedule,
                    &f
                )
                .unwrap(),
                actual.2
            );
            assert_eq!(verifier.state_digest(), transcript.state_digest());
            let mut altered = actual.0.clone();
            altered.binding_terminal[0] = f.add(&altered.binding_terminal[0], &f.one());
            assert!(
                verify_with_factors(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &form,
                    &factors,
                    target,
                    &altered,
                    &schedule,
                    &f
                )
                .is_err()
            );
            let (form, factors, _) = factored_fixture(&layout, &f, mode);
            assert!(
                prove_with_correction(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &source,
                    form,
                    factors,
                    f.add(&target, &f.one()),
                    dummy_norm(&f),
                    None,
                    &schedule,
                    &f
                )
                .is_err()
            );
        }
    }
}

#[test]
fn direct_dense_initialization_reuses_and_recovers_from_panics() {
    let f = test_field();
    let mut values = Vec::new();
    for fused in [false, true] {
        for len in [2050, 512, 3072, 1024] {
            let slot = |i| [value(i as i64 * 3 - 19, &f), value(7 - i as i64, &f)];
            let expected: Vec<_> = (0..len).map(slot).collect();
            let actual = initialize_dense(&mut values, len, &f, true, fused, slot);
            assert_eq!(values, expected);
            assert_eq!(actual, dense_sums(&expected, 1, &f));
        }
        let capacity = values.capacity();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize_dense(&mut values, 2048, &f, true, fused, |i| {
                assert_ne!(i, 777, "injected worker failure");
                [f.one(); 2]
            });
        }));
        assert!(panicked.is_err());
        assert!(values.is_empty());
        assert_eq!(values.capacity(), capacity);
        initialize_dense(&mut values, 1024, &f, true, fused, |_| [f.zero(); 2]);
        assert_eq!(values, vec![[f.zero(); 2]; 1024]);
        let odd_length = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize_dense(&mut values, 3, &f, true, fused, |_| [f.one(); 2]);
        }));
        assert!(odd_length.is_err());
    }
}

#[test]
fn optimized_dense_folds_match_independent_reference_through_terminal() {
    let f = test_field();
    let layout = Layout::new(3).unwrap();
    let source = source_fixture(&layout);
    for mode in [0, 3] {
        for challenges in 0..3 {
            let (form, factors, dense_form) = factored_fixture(&layout, &f, mode);
            let mut reference = Reference::new(&dense_form, &source, &f);
            let mut values = Vec::new();
            let mut stride = 1;
            let mut transcript = Blake3Transcript::new();
            transcript.absorb_slice(b"word-dense-reference-challenges");
            for round in 0..rounds(&layout) {
                let r = match challenges {
                    0 => f.zero(),
                    1 => f.one(),
                    _ => sample(&mut transcript, &f).unwrap(),
                };
                let sums = if round == 0 {
                    first_fold_into(
                        &form,
                        &source,
                        r,
                        &factors,
                        &f,
                        WordOptions {
                            fused_fold: true,
                            direct_init: true,
                            factored_fold_reduction: true,
                            ..WordOptions::default()
                        },
                        &mut values,
                    )
                } else {
                    let result = dense_fold(&mut values, stride, r, &f);
                    stride *= 2;
                    result
                };
                reference.fold(r, &f);
                let active: Vec<_> = values.iter().step_by(stride).copied().collect();
                let expected: Vec<_> = reference.weights[0]
                    .iter()
                    .zip(&reference.tables[0])
                    .map(|(&w, &x)| [w, x])
                    .collect();
                assert_eq!(
                    active, expected,
                    "mode={mode} challenge={challenges} round={round}"
                );
                assert_eq!(sums, dense_sums(&expected, 1, &f));
            }
        }
    }
}

#[test]
fn workspace_and_factored_options_preserve_complete_word_proofs() {
    let mut workspace = WordWorkspace::default();
    let mut largest = 0;
    for (batch, grinding, modulus) in [
        (3, 0, (1u128 << 127) - 1),
        (1, 2, (1u128 << 61) - 1),
        (17, 0, (1u128 << 127) - 1),
        (3, 0, (1u128 << 61) - 1),
    ] {
        let f = F::make_cfg(&Uint::from(modulus)).unwrap();
        let layout = Layout::new(batch).unwrap();
        let source = source_fixture(&layout);
        let schedule = test_schedule(grinding);
        for mode in [0, 3] {
            let (_, _, dense) = factored_fixture(&layout, &f, mode);
            let target = Reference::new(&dense, &source, &f).target(&f);
            let mut baseline = Blake3Transcript::new();
            let expected = prove_with_options(
                &mut baseline,
                &layout,
                &source,
                dense,
                WordFactors::default(),
                target,
                dummy_norm(&f),
                None,
                &schedule,
                &f,
                &mut WordWorkspace::default(),
                WordOptions::default(),
            )
            .unwrap();
            for options in [
                WordOptions {
                    direct_init: true,
                    ..WordOptions::default()
                },
                WordOptions {
                    scratch: true,
                    ..WordOptions::default()
                },
                WordOptions {
                    scratch: true,
                    direct_init: true,
                    ..WordOptions::default()
                },
                WordOptions {
                    factored_initial_fused: true,
                    ..WordOptions::default()
                },
                WordOptions {
                    factored_fold_reduction: true,
                    ..WordOptions::default()
                },
                WordOptions {
                    reuse_initial: true,
                    fused_fold: true,
                    direct_init: true,
                    scratch: true,
                    factored_initial_fused: true,
                    factored_fold_reduction: true,
                },
            ] {
                let (form, factors, _) = factored_fixture(&layout, &f, mode);
                let mut transcript = Blake3Transcript::new();
                let actual = prove_with_options(
                    &mut transcript,
                    &layout,
                    &source,
                    form,
                    factors,
                    target,
                    dummy_norm(&f),
                    None,
                    &schedule,
                    &f,
                    &mut workspace,
                    options,
                )
                .unwrap();
                assert_eq!(actual, expected, "batch={batch} mode={mode} {options:?}");
                assert_eq!(transcript.state_digest(), baseline.state_digest());
                if options.scratch {
                    largest = largest.max(layout.source_bits() / 32);
                }
                assert_eq!(workspace.capacity_slots(), largest);
                let (form, factors, _) = factored_fixture(&layout, &f, mode);
                let mut verifier = Blake3Transcript::new();
                assert_eq!(
                    verify_with_factors(
                        &mut verifier,
                        &layout,
                        &form,
                        &factors,
                        target,
                        &actual.0,
                        &schedule,
                        &f,
                    )
                    .unwrap(),
                    actual.2
                );
                assert_eq!(verifier.state_digest(), transcript.state_digest());
                let (form, factors, _) = factored_fixture(&layout, &f, mode);
                assert!(
                    prove_with_options(
                        &mut Blake3Transcript::new(),
                        &layout,
                        &source,
                        form,
                        factors,
                        f.add(&target, &f.one()),
                        dummy_norm(&f),
                        None,
                        &schedule,
                        &f,
                        &mut workspace,
                        options,
                    )
                    .is_err()
                );
                assert_eq!(workspace.capacity_slots(), largest);
            }
        }
    }
}

fn construction_inputs(layout: &Layout, f: &Cfg, first: F) -> (PreparedClaim, NormClaims) {
    let [weights_s1, weights_s2] = form_fixture(layout, f).coefficients;
    let instance_vars = layout.capacity().ilog2() as usize;
    (
        PreparedClaim {
            weights_s1,
            weights_s2,
            target: value(-71, f),
        },
        NormClaims {
            instance_point: (0..instance_vars)
                .map(|i| value(7 * i as i64 - 19, f))
                .collect(),
            point: (0..rounds(layout))
                .map(|i| {
                    if i == 0 {
                        first
                    } else {
                        value(13 * i as i64 - 37, f)
                    }
                })
                .collect(),
            terminal: value(31, f),
            slack: value(19, f),
        },
    )
}

#[test]
fn fused_construction_initial_matches_independent_dense_reference() {
    for modulus in [(1u128 << 61) - 1, (1u128 << 127) - 1] {
        let f = F::make_cfg(&Uint::from(modulus)).unwrap();
        for batch in [1, 3, 17] {
            let layout = Layout::new(batch).unwrap();
            let source = source_fixture(&layout);
            for (eta, first) in [
                (f.zero(), f.zero()),
                (f.one(), f.one()),
                (value(-29, &f), f.zero()),
                (value(-29, &f), value(-37, &f)),
            ] {
                let (ring, norm) = construction_inputs(&layout, &f, first);
                let full_weights = eq_table(&norm.point, &f).unwrap();
                for parallel in [false, true] {
                    let (form, target, initial) = BindingForm::new_owned_with_strategy_and_initial(
                        &layout,
                        ring.clone(),
                        &norm,
                        eta,
                        &f,
                        BindingStrategy::Optimized,
                        parallel,
                        false,
                        false,
                        Some(&source),
                    )
                    .unwrap();
                    let initial = initial.unwrap();
                    let (baseline, baseline_target) = BindingForm::new_owned_with_strategy(
                        &layout,
                        ring.clone(),
                        &norm,
                        eta,
                        &f,
                        BindingStrategy::Optimized,
                        parallel,
                        false,
                        false,
                    )
                    .unwrap();
                    assert_eq!(target, baseline_target);
                    assert_eq!(form.coefficients, baseline.coefficients);
                    assert_eq!(form.slacks, baseline.slacks);
                    assert_eq!(form.padding, baseline.padding);
                    for side in 0..2 {
                        let ring_values = [&ring.weights_s1, &ring.weights_s2][side];
                        for (i, &original) in ring_values.iter().enumerate() {
                            let index = (i / N) * 2 * N + side * N + i % N;
                            assert_eq!(
                                form.coefficients[side][i],
                                f.add(&original, &f.mul(&eta, &full_weights[index]))
                            );
                        }
                    }
                    let reference = Reference::new(&form, &source, &f);
                    let mut expected = [f.zero(); 3];
                    for (w, x) in reference.weights[0]
                        .chunks_exact(2)
                        .zip(reference.tables[0].chunks_exact(2))
                    {
                        let even = f.mul(&w[0], &x[0]);
                        expected[0] = f.add(&expected[0], &even);
                        expected[1] = f.add(
                            &expected[1],
                            &f.mul(&f.sub(&w[1], &w[0]), &f.sub(&x[1], &x[0])),
                        );
                        expected[2] = f.add(&expected[2], &f.add(&even, &f.mul(&w[1], &x[1])));
                    }
                    assert_eq!(
                        initial,
                        InitialClaim {
                            round: [expected[0], expected[1]],
                            sum: expected[2]
                        }
                    );
                    // The independently selectable duplicate-product optimization
                    // must agree when composed with construction fusion.
                    let mut coefficients = [ring.weights_s1.clone(), ring.weights_s2.clone()];
                    let reused = construct_and_initial(
                        &mut coefficients,
                        &source,
                        &eq_table(&norm.point[..COEFFICIENT_LOG], &f).unwrap(),
                        &eq_table(&norm.point[COEFFICIENT_LOG + 1..], &f).unwrap(),
                        eta,
                        [
                            f.sub(&f.one(), &norm.point[COEFFICIENT_LOG]),
                            norm.point[COEFFICIENT_LOG],
                        ],
                        &f,
                        parallel,
                        true,
                    )
                    .unwrap();
                    assert_eq!(reused, initial);
                    assert_eq!(coefficients, form.coefficients);
                }
            }
        }
    }
}

#[test]
fn fused_construction_preserves_proof_and_grinding_boundaries() {
    let f = test_field();
    for (batch, grinding) in [(1, 0), (3, 2), (17, 0)] {
        let layout = Layout::new(batch).unwrap();
        let source = source_fixture(&layout);
        let schedule = test_schedule(grinding);
        let eta = value(-29, &f);
        let (ring, norm) = construction_inputs(&layout, &f, value(-37, &f));
        let (baseline, _) = BindingForm::new_owned_with_strategy(
            &layout,
            ring.clone(),
            &norm,
            eta,
            &f,
            BindingStrategy::Optimized,
            false,
            false,
            false,
        )
        .unwrap();
        let target = Reference::new(&baseline, &source, &f).target(&f);
        let mut baseline_transcript = Blake3Transcript::new();
        baseline_transcript.absorb_slice(b"construction-fusion-prefix");
        let expected = prove_with_options(
            &mut baseline_transcript,
            &layout,
            &source,
            baseline,
            WordFactors::default(),
            target,
            dummy_norm(&f),
            None,
            &schedule,
            &f,
            &mut WordWorkspace::default(),
            WordOptions::default(),
        )
        .unwrap();
        for (strategy, parallel) in [
            (BindingStrategy::Optimized, false),
            (BindingStrategy::Optimized, true),
            (BindingStrategy::Legacy, false),
        ] {
            let (form, _, initial) = BindingForm::new_owned_with_strategy_and_initial(
                &layout,
                ring.clone(),
                &norm,
                eta,
                &f,
                strategy,
                parallel,
                false,
                false,
                Some(&source),
            )
            .unwrap();
            assert_eq!(
                initial.is_some(),
                matches!(strategy, BindingStrategy::Optimized)
            );
            let mut transcript = Blake3Transcript::new();
            transcript.absorb_slice(b"construction-fusion-prefix");
            let actual = prove_with_options_and_initial(
                &mut transcript,
                &layout,
                &source,
                form,
                WordFactors::default(),
                target,
                dummy_norm(&f),
                None,
                &schedule,
                &f,
                &mut WordWorkspace::default(),
                WordOptions {
                    direct_init: true,
                    ..WordOptions::default()
                },
                initial,
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                transcript.state_digest(),
                baseline_transcript.state_digest()
            );
            let (form, _) = BindingForm::new_owned_with_strategy(
                &layout,
                ring.clone(),
                &norm,
                eta,
                &f,
                BindingStrategy::Optimized,
                false,
                false,
                false,
            )
            .unwrap();
            let mut verifier = Blake3Transcript::new();
            verifier.absorb_slice(b"construction-fusion-prefix");
            assert_eq!(
                verify(
                    &mut verifier,
                    &layout,
                    &form,
                    target,
                    &actual.0,
                    &schedule,
                    &f
                )
                .unwrap(),
                actual.2
            );
            assert_eq!(verifier.state_digest(), transcript.state_digest());
            let (form, _, initial) = BindingForm::new_owned_with_strategy_and_initial(
                &layout,
                ring.clone(),
                &norm,
                eta,
                &f,
                strategy,
                parallel,
                false,
                false,
                Some(&source),
            )
            .unwrap();
            assert!(
                prove_with_options_and_initial(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &source,
                    form,
                    WordFactors::default(),
                    f.add(&target, &f.one()),
                    dummy_norm(&f),
                    None,
                    &schedule,
                    &f,
                    &mut WordWorkspace::default(),
                    WordOptions::default(),
                    initial,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn fused_construction_preserves_source_and_projection_rejections() {
    let f = test_field();
    let layout = Layout::new(3).unwrap();
    let source = source_fixture(&layout);
    let (ring, norm) = construction_inputs(&layout, &f, f.zero());
    let eta = f.one();
    for flip in [16 * N + 15, 16 * layout.batch() * 2 * N] {
        let mut malformed = source.clone();
        malformed.flip(flip);
        let mut errors = Vec::new();
        let mut digests = Vec::new();
        for fused in [false, true] {
            let (form, _, initial) = BindingForm::new_owned_with_strategy_and_initial(
                &layout,
                ring.clone(),
                &norm,
                eta,
                &f,
                BindingStrategy::Optimized,
                true,
                false,
                false,
                fused.then_some(&malformed),
            )
            .unwrap();
            let target = Reference::new(&form, &malformed, &f).target(&f);
            let mut transcript = Blake3Transcript::new();
            let result = prove_with_options_and_initial(
                &mut transcript,
                &layout,
                &malformed,
                form,
                WordFactors::default(),
                target,
                dummy_norm(&f),
                None,
                &test_schedule(2),
                &f,
                &mut WordWorkspace::default(),
                WordOptions::default(),
                initial,
            );
            errors.push(result.unwrap_err().to_string());
            digests.push(transcript.state_digest());
        }
        assert_eq!(errors[0], errors[1]);
        assert_eq!(digests[0], digests[1]);
    }
    for invalid_side in 0..2 {
        let mut malformed = ring.clone();
        [&mut malformed.weights_s1, &mut malformed.weights_s2][invalid_side][N + 1] =
            crate::piop::spartan::noncanonical_test_value(&f);
        let mut errors = Vec::new();
        for fused in [false, true] {
            errors.push(
                BindingForm::new_owned_with_strategy_and_initial(
                    &layout,
                    malformed.clone(),
                    &norm,
                    eta,
                    &f,
                    BindingStrategy::Optimized,
                    true,
                    false,
                    false,
                    fused.then_some(&source),
                )
                .err()
                .unwrap()
                .to_string(),
            );
        }
        assert_eq!(errors[0], errors[1]);
    }
    // Legacy construction does not claim a fused initial polynomial.
    let (legacy, _, initial) = BindingForm::new_owned_with_strategy_and_initial(
        &layout,
        ring,
        &norm,
        eta,
        &f,
        BindingStrategy::Legacy,
        false,
        false,
        false,
        Some(&source),
    )
    .unwrap();
    assert!(initial.is_none());
    let target = Reference::new(&legacy, &source, &f).target(&f);
    assert!(
        prove_with_options_and_initial(
            &mut Blake3Transcript::new(),
            &layout,
            &source,
            legacy,
            WordFactors::default(),
            f.add(&target, &f.one()),
            dummy_norm(&f),
            None,
            &test_schedule(0),
            &f,
            &mut WordWorkspace::default(),
            WordOptions::default(),
            initial,
        )
        .is_err()
    );
}
