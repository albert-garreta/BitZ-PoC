use super::super::{
    piop::{
        CompactionProof, NormProof, PrimeProductForestProof, PrimeProductTreeProof,
        QuadraticRelationProof,
    },
    verification_trace,
};
use super::*;
use crate::sumcheck::outer::OuterEvaluations;

const KECCAK_GATE_STRIDE: usize = 1 << 20;
use crate::transcript::Blake3Transcript;

const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] = include_bytes!("fixtures/public_key.bin");
const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
    include_bytes!("fixtures/signature_ct.bin");

fn config() -> Cfg {
    field_from_modulus((1u128 << 127) - 1).unwrap()
}

fn point(rounds: usize, field: &Cfg) -> Vec<F> {
    (0..rounds)
        .map(|i| unsigned(3 + 2 * i as u128, field))
        .collect()
}

fn evaluator<'a>(point: &[F], field: &'a Cfg) -> EvaluatingSink<'a> {
    let local_vars = FalconSourceLayout::SIGNATURE_STRIDE.ilog2() as usize;
    EvaluatingSink {
        weights: factored_weights(point, field).unwrap(),
        local_weights: factored_weights(&point[..local_vars], field).unwrap(),
        instance_weights: eq_table(&point[local_vars..], field).unwrap(),
        field,
        value: field.zero(),
    }
}

struct DenseSink<'a> {
    values: Vec<F>,
    field: &'a Cfg,
    subtract: bool,
}

impl<'a> DenseSink<'a> {
    fn new(len: usize, field: &'a Cfg) -> Self {
        Self {
            values: vec![field.zero(); len],
            field,
            subtract: false,
        }
    }
}

impl CoefficientSink for DenseSink<'_> {
    fn add(&mut self, index: usize, coefficient: F) {
        self.values[index] = if self.subtract {
            self.field.sub(&self.values[index], &coefficient)
        } else {
            self.field.add(&self.values[index], &coefficient)
        };
    }
}

// Preserve the old per-instance construction as an oracle for the optimized
// cache and verifier contraction. These weights include the instance factor.
fn reference_ring_weights(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    weights: &crate::poly::mle::EqualityWeights<F>,
    field: &Cfg,
) -> Vec<Vec<F>> {
    let ring_start = layout.linear_rows() - N;
    statement
        .public_keys
        .iter()
        .enumerate()
        .map(|(instance, key)| {
            let rows: Vec<_> = (0..N)
                .map(|i| weights.at(instance * layout.linear_stride() + ring_start + i))
                .collect();
            ring_adjoint(&key.h, &rows, field)
        })
        .collect()
}

fn add_linear_with_reference_ring(
    sink: &mut impl CoefficientSink,
    weights: &crate::poly::mle::EqualityWeights<F>,
    ring: &[Vec<F>],
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    field: &Cfg,
) -> Result<F, FalconError> {
    let constant = add_linear_constraints(sink, weights, layout, statement, field)?;
    for (instance, row) in ring.iter().enumerate() {
        for (j, &weight) in row.iter().enumerate() {
            add_signed_source_scaled(
                sink,
                instance * layout.signature_stride(),
                &layout.offsets(),
                j,
                weight,
                field,
            );
        }
    }
    Ok(constant)
}

fn emit_binding_reference(binding: &BindingForm<'_>, sink: &mut impl CoefficientSink) -> F {
    let ring = reference_ring_weights(
        binding.layout,
        binding.statement,
        &binding.linear_weights,
        binding.field,
    );
    let constant = add_linear_with_reference_ring(
        sink,
        &binding.linear_weights,
        &ring,
        binding.layout,
        binding.statement,
        binding.field,
    )
    .unwrap();
    let mut target = binding.field.sub(&binding.field.zero(), &constant);
    let mut scale = binding.eta;
    add_norm_claims(
        sink,
        &mut target,
        &mut scale,
        binding.eta,
        binding.layout,
        binding.proof,
        binding.field,
    )
    .unwrap();
    add_keccak_claims(
        sink,
        &mut target,
        &mut scale,
        binding.eta,
        binding.layout,
        binding.proof,
        binding.field,
    )
    .unwrap();
    add_compaction_product_claims(
        sink,
        &mut target,
        &mut scale,
        binding.eta,
        binding.layout,
        binding.proof,
        binding.field,
    )
    .unwrap();
    add_product_tree_claims(
        sink,
        &mut target,
        &mut scale,
        binding.eta,
        binding.layout,
        binding.proof,
        binding.field,
    )
    .unwrap();
    target
}

// Only terminal data is consumed by the binder; no proof generation is needed.
fn terminal_fixture(keccak_point: Vec<F>, field: &Cfg) -> FalconPiopProof {
    let relation = QuadraticRelationProof {
        point_nonce: None,
        sumcheck: SumcheckProof {
            round_polynomials: Vec::new(),
        },
        terminal: OuterEvaluations {
            ax: unsigned(7, field),
            bx: unsigned(11, field),
            cx: unsigned(13, field),
        },
        point: keccak_point,
        grinding_nonces: Vec::new(),
    };
    FalconPiopProof {
        modulus: field.modulus_u128(),
        norm: NormProof {
            instance_point: Vec::new(),
            instance_nonce: None,
            claims: [field.zero(); 2],
            slack: field.zero(),
            sumchecks: core::array::from_fn(|_| SumcheckProof {
                round_polynomials: Vec::new(),
            }),
            terminal: [[field.zero(); 2]; 2],
            point: Vec::new(),
            grinding_nonces: Vec::new(),
        },
        keccak_chi: Some(relation.clone()),
        compact_products: relation,
        fingerprint_nonce: None,
        compaction_gamma: field.zero(),
        compaction_rank_scale: field.zero(),
        compaction: Vec::new(),
        compaction_forest: PrimeProductForestProof { layers: Vec::new() },
    }
}

#[test]
fn ring_adjoint_matches_explicit_negacyclic_matrix() {
    let field = config();
    let h = core::array::from_fn(|i| ((97 * i + 3 * i * i) % Q as usize) as u16);
    let weights: Vec<_> = (0..N)
        .map(|i| unsigned((i * i + 31 * i + 7) as u128, &field))
        .collect();
    let adjoint = ring_adjoint(&h, &weights, &field);
    let mut expected = vec![field.zero(); N];
    // Deliberately retain the original output-row/input-column orientation.
    for i in 0..N {
        for j in 0..N {
            let coefficient = if i >= j {
                -i128::from(h[i - j])
            } else {
                i128::from(h[N + i - j])
            };
            expected[j] = field.add(&expected[j], &mul_i(weights[i], coefficient, &field));
        }
    }
    assert_eq!(adjoint, expected);
    for index in [0, 1, N / 2, N - 1] {
        let mut basis = [0; N];
        basis[index] = 1;
        let actual = ring_adjoint(&basis, &weights, &field);
        for j in 0..N {
            let row = (index + j) % N;
            let expected = if index + j < N {
                field.sub(&field.zero(), &weights[row])
            } else {
                weights[row]
            };
            assert_eq!(actual[j], expected);
        }
    }
}

#[test]
fn field_ring_adjoint_matches_large_coefficient_matrix() {
    let field = config();
    let h = core::array::from_fn(|i| signed(-((1i128 << 90) + (i * i + 17) as i128), &field));
    let weights: Vec<_> = (0..N)
        .map(|i| unsigned((i * i + 31 * i + 7) as u128, &field))
        .collect();
    let actual = ring_adjoint_field(&h, &weights, &field);
    let mut expected = vec![field.zero(); N];
    for i in 0..N {
        for j in 0..N {
            let coefficient = if i >= j {
                field.sub(&field.zero(), &h[i - j])
            } else {
                h[N + i - j]
            };
            expected[j] = field.add(&expected[j], &field.mul(&weights[i], &coefficient));
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn equality_prefix_matches_dense_sums_at_all_boundaries() {
    let field = config();
    for rounds in 0..=10 {
        let mut point = point(rounds, &field);
        if rounds >= 3 {
            point[0] = field.zero();
            point[2] = field.one();
        }
        let weights = eq_table(&point, &field).unwrap();
        let mut sum = field.zero();
        for end in 0..=weights.len() {
            assert_eq!(eq_prefix_sum(&point, end, &field), sum);
            if end < weights.len() {
                sum = field.add(&sum, &weights[end]);
            }
        }
    }
}

// Original three-coordinate construction with an unfactored equality table.
// Keeping this separate catches errors in coordinate fusion and padding masks.
fn keccak_coordinate_reference(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) {
    let weights = eq_table(&proof.keccak_chi.as_ref().unwrap().point, field).unwrap();
    let half = KECCAK_GATE_STRIDE * layout.capacity();
    let offsets = FalconSourceOffsets::new();
    let claims = [
        proof.keccak_chi.as_ref().unwrap().terminal.ax,
        proof.keccak_chi.as_ref().unwrap().terminal.bx,
        proof.keccak_chi.as_ref().unwrap().terminal.cx,
    ];
    let two_inv = unsigned((field.modulus_u128() + 1) / 2, field);
    for coordinate in 0..3 {
        let mut constant = field.zero();
        for instance in 0..layout.batch() {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            for gate in 0..(20 * 24 * 25 * 64) {
                let word = gate >> 6;
                let bit = gate & 63;
                let lane = word % 25;
                let x = lane % 5;
                let y = lane / 5;
                let round = (word / 25) % 24;
                let rel0 = field.mul(scale, &weights[instance * KECCAK_GATE_STRIDE + gate]);
                let rel1 = field.mul(scale, &weights[half + instance * KECCAK_GATE_STRIDE + gate]);
                let chi_base = word - lane + y * 5;
                let b1 = base + offsets.keccak_chi_inputs + 64 * (chi_base + (x + 1) % 5) + bit;
                let b2 = base + offsets.keccak_chi_inputs + 64 * (chi_base + (x + 2) % 5) + bit;
                let bx = base + offsets.keccak_chi_inputs + gate;
                let z = base + offsets.keccak_chi_ands + gate;
                let y_out = base + offsets.keccak_round_states + gate;
                match coordinate {
                    0 => {
                        constant = field.add(&constant, &rel0);
                        coefficients.add(b1, field.sub(&field.zero(), &rel0));
                        coefficients.add(bx, rel1);
                    }
                    1 => {
                        coefficients.add(b2, rel0);
                        coefficients.add(z, rel1);
                    }
                    2 => {
                        coefficients.add(z, rel0);
                        let half_rel1 = field.mul(&rel1, &two_inv);
                        coefficients.add(bx, half_rel1);
                        coefficients.add(z, half_rel1);
                        if lane == 0
                            && (super::super::keccak::ROUND_CONSTANTS[round] >> bit) & 1 == 1
                        {
                            constant = field.sub(&constant, &half_rel1);
                            coefficients.add(y_out, half_rel1);
                        } else {
                            coefficients.add(y_out, field.sub(&field.zero(), &half_rel1));
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        *target = field.add(
            target,
            &field.sub(&field.mul(scale, &claims[coordinate]), &constant),
        );
        *scale = field.mul(scale, &eta);
    }
}

#[test]
fn fused_keccak_matches_dense_reference_for_padded_batch() {
    let field = config();
    for batch in [1, 3] {
        let layout = FalconSourceLayout::new(batch).unwrap();
        let proof = terminal_fixture(
            point(21 + layout.capacity().ilog2() as usize, &field),
            &field,
        );
        let mut dense = DenseSink::new(layout.source_bits(), &field);
        let eta = unsigned(17, &field);
        let initial_scale = unsigned(19, &field);
        let initial_target = unsigned(23, &field);
        let mut scale = initial_scale;
        let mut target = initial_target;
        add_keccak_claims(
            &mut dense,
            &mut target,
            &mut scale,
            eta,
            &layout,
            &proof,
            &field,
        )
        .unwrap();
        let mut constants_target = initial_target;
        let mut constants_scale = initial_scale;
        add_keccak_claims(
            &mut ConstantsOnly,
            &mut constants_target,
            &mut constants_scale,
            eta,
            &layout,
            &proof,
            &field,
        )
        .unwrap();
        assert_eq!((constants_target, constants_scale), (target, scale));
        dense.subtract = true;
        let mut reference_target = initial_target;
        let mut reference_scale = initial_scale;
        keccak_coordinate_reference(
            &mut dense,
            &mut reference_target,
            &mut reference_scale,
            eta,
            &layout,
            &proof,
            &field,
        );
        assert_eq!((target, scale), (reference_target, reference_scale));
        assert!(
            dense
                .values
                .iter()
                .all(|&coefficient| coefficient == field.zero())
        );
        // Reuse the now-zero dense allocation to check the verifier's separate
        // instance-contraction path against an ordinary dense MLE fold.
        dense.subtract = false;
        let mut dense_target = initial_target;
        let mut dense_scale = initial_scale;
        add_keccak_claims(
            &mut dense,
            &mut dense_target,
            &mut dense_scale,
            eta,
            &layout,
            &proof,
            &field,
        )
        .unwrap();
        let endpoint = point(source_rounds(&layout), &field);
        let mut evaluating = evaluator(&endpoint, &field);
        let mut scalar_target = initial_target;
        let mut scalar_scale = initial_scale;
        add_keccak_claims(
            &mut evaluating,
            &mut scalar_target,
            &mut scalar_scale,
            eta,
            &layout,
            &proof,
            &field,
        )
        .unwrap();
        assert_eq!((scalar_target, scalar_scale), (dense_target, dense_scale));
        assert_eq!(
            evaluating.value,
            evaluate_mle_in_place(&mut dense.values, &endpoint, &field).unwrap()
        );
    }
}

#[test]
fn linear_binding_matches_committed_bits_and_detects_parity_changes_in_padded_batch() {
    let field = config();
    let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
    let layout = FalconSourceLayout::new(3).unwrap();
    let statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY, PUBLIC_KEY, PUBLIC_KEY],
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
    )
    .unwrap();
    let mut traces = [trace.clone(), trace.clone(), trace];
    let source = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
        &traces,
    )
    .unwrap();
    let weights = factored_weights(&point(linear_rounds(&layout), &field), &field).unwrap();
    let ring_start = super::super::FalconConstraintCounts::per_signature().linear_rows() - N;
    let ring: Vec<_> = statement
        .public_keys
        .iter()
        .enumerate()
        .map(|(instance, key)| {
            let row_weights: Vec<_> = (0..N)
                .map(|i| weights.at(instance * LINEAR_STRIDE + ring_start + i))
                .collect();
            ring_adjoint(&key.h, &row_weights, &field)
        })
        .collect();
    let mut dense = DenseSink::new(layout.source_bits(), &field);
    let constant =
        add_linear_with_reference_ring(&mut dense, &weights, &ring, &layout, &statement, &field)
            .unwrap();
    assert_eq!(
        add_linear_with_reference_ring(
            &mut ConstantsOnly,
            &weights,
            &ring,
            &layout,
            &statement,
            &field
        )
        .unwrap(),
        constant
    );
    assert_eq!(
        field.add(
            &dot_packed_source(&dense.values, &source, &field),
            &constant
        ),
        field.zero()
    );
    let offsets = FalconSourceOffsets::new();
    for instance in 0..layout.batch() {
        let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
        assert!(
            dense.values[base + offsets.end..base + FalconSourceLayout::SIGNATURE_STRIDE]
                .iter()
                .all(|&value| value == field.zero())
        );
    }
    assert!(
        dense.values[layout.batch() * FalconSourceLayout::SIGNATURE_STRIDE..]
            .iter()
            .all(|&value| value == field.zero())
    );

    // Mutate C[0,37] in instance1 and one theta quotient in instance2.
    // Calculate the ten affected theta rows from the forward rho/pi map.
    let column_bit = 37;
    let column_delta = if traces[1].hash_to_point.shake.column_parities[0] >> column_bit & 1 == 0 {
        field.one()
    } else {
        signed(-1, &field)
    };
    traces[1].hash_to_point.shake.column_parities[0] ^= 1u64 << column_bit;
    let first_round = 1 + 256 + super::super::CT_SIGNATURE_BYTES + N;
    let mut column_weight = field.sub(
        &field.zero(),
        &weights.at(LINEAR_STRIDE + first_round + column_bit),
    );
    for (x, input_bit) in [(1, column_bit), (4, (column_bit + 1) % 64)] {
        for y in 0..5 {
            let lane = y + 5 * ((2 * x + 3 * y) % 5);
            let bit = (input_bit + ROTATION[x][y] as usize) % 64;
            column_weight = field.add(
                &column_weight,
                &weights.at(LINEAR_STRIDE + first_round + 320 + lane * 64 + bit),
            );
        }
    }
    let quotient_bit = 99;
    let quotient_delta = if traces[2].hash_to_point.shake.parity_quotients[quotient_bit] == 0 {
        field.one()
    } else {
        signed(-1, &field)
    };
    traces[2].hash_to_point.shake.parity_quotients[quotient_bit] ^= 1;
    let expected = field.add(
        &field.mul(&column_delta, &column_weight),
        &mul_i(
            field.mul(
                &quotient_delta,
                &weights.at(2 * LINEAR_STRIDE + first_round + 320 + quotient_bit),
            ),
            -2,
            &field,
        ),
    );
    assert_ne!(expected, field.zero());
    let changed = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
        &traces,
    )
    .unwrap();
    assert_eq!(
        field.add(
            &dot_packed_source(&dense.values, &changed, &field),
            &constant
        ),
        expected
    );

    let endpoint = point(source_rounds(&layout), &field);
    let mut evaluating = evaluator(&endpoint, &field);
    assert_eq!(
        add_linear_with_reference_ring(
            &mut evaluating,
            &weights,
            &ring,
            &layout,
            &statement,
            &field
        )
        .unwrap(),
        constant
    );
    assert_eq!(
        evaluating.value,
        evaluate_mle_in_place(&mut dense.values, &endpoint, &field).unwrap()
    );
}

fn full_terminal_fixture(layout: &FalconSourceLayout, field: &Cfg) -> FalconPiopProof {
    let batch_vars = layout.capacity().ilog2() as usize;
    let mut proof = terminal_fixture(point(21 + batch_vars, field), field);
    proof.norm.point = point(10 + batch_vars, field);
    proof.norm.instance_point = point(batch_vars, field);
    proof.norm.terminal = [
        [unsigned(3, field), unsigned(5, field)],
        [unsigned(7, field), unsigned(11, field)],
    ];
    proof.norm.slack = unsigned(41, field);
    proof.compact_products.point = point(13 + batch_vars, field);
    proof.compaction_gamma = unsigned(29, field);
    proof.compaction_rank_scale = unsigned(31, field);
    proof.compaction = (0..layout.batch())
        .map(|instance| {
            let tree = |side: usize| {
                let mut terminal_point = point(11, field);
                terminal_point[0] = unsigned((instance * 2 + side + 2) as u128, field);
                PrimeProductTreeProof {
                    root: field.one(),
                    terminal_point,
                    terminal_claim: unsigned((instance + side + 13) as u128, field),
                }
            };
            CompactionProof {
                candidate: tree(0),
                output: tree(1),
            }
        })
        .collect();
    if layout.is_hybrid() {
        proof.keccak_chi = None;
    }
    proof
}

#[test]
fn full_binding_endpoint_matches_dense_with_distinct_keys_and_padded_instance() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let mut statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY, PUBLIC_KEY, PUBLIC_KEY],
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
    )
    .unwrap();
    statement.public_keys[1].h[0] = (statement.public_keys[1].h[0] + 1) % Q as u16;
    statement.public_keys[2].h[N - 1] = (statement.public_keys[2].h[N - 1] + 3) % Q as u16;
    statement.messages[1][0] ^= 1;
    statement.messages[2][31] ^= 128;
    statement.signatures[1].nonce[0] ^= 1;
    statement.signatures[2].nonce[39] ^= 128;
    statement.signatures[1].s2[0] = -2047;
    statement.signatures[2].s2[N - 1] = 2047;
    let proof = full_terminal_fixture(&layout, &field);
    let linear_point = point(linear_rounds(&layout), &field);
    let mut transcript = Blake3Transcript::new();
    let binding = prepare_binding_form(
        &mut transcript,
        &layout,
        &statement,
        &proof,
        &linear_point,
        &field,
    )
    .unwrap();
    let mut dense = DenseSink::new(layout.source_bits(), &field);
    let target = binding.emit(&mut dense).unwrap();
    assert_eq!(binding.target().unwrap(), target);
    let endpoint = point(source_rounds(&layout), &field);
    let mut padding_endpoint = endpoint.clone();
    padding_endpoint[22] = field.one();
    padding_endpoint[23] = field.one();
    assert_eq!(binding.evaluate(&padding_endpoint).unwrap(), field.zero());
    assert_eq!(
        binding.evaluate(&endpoint).unwrap(),
        evaluate_mle_in_place(&mut dense.values, &endpoint, &field).unwrap()
    );
}

#[cfg(feature = "falcon-hybrid")]
fn statement_with_key_groups(groups: &[usize]) -> FalconPublicStatement {
    let mut statement = FalconPublicStatement::from_bytes(
        &vec![PUBLIC_KEY.as_slice(); groups.len()],
        &vec![MESSAGE.as_slice(); groups.len()],
        &vec![SIGNATURE.as_slice(); groups.len()],
    )
    .unwrap();
    for (instance, (&group, key)) in groups.iter().zip(&mut statement.public_keys).enumerate() {
        key.h[0] = (key.h[0] + group as u16) % Q as u16;
        key.h[N - 1] = (key.h[N - 1] + 7 * group as u16) % Q as u16;
        statement.messages[instance][0] ^= instance as u8;
        statement.signatures[instance].nonce[0] ^= instance as u8;
        statement.signatures[instance].nonce[39] ^= (instance as u8).wrapping_mul(17);
        statement.signatures[instance].s2[0] = -2047 + instance as i16;
        statement.signatures[instance].s2[N - 1] = 2047 - instance as i16;
    }
    statement
}

#[cfg(feature = "falcon-hybrid")]
#[test]
fn shared_key_binding_matches_per_instance_reference_at_degenerate_points() {
    let field = config();
    for groups in [&[0, 0, 0][..], &[0, 1, 0, 2, 1], &[0, 1, 2]] {
        let layout = FalconSourceLayout::new_hybrid(groups.len()).unwrap();
        let statement = statement_with_key_groups(groups);
        let proof = full_terminal_fixture(&layout, &field);
        let local_rows = layout.linear_stride().ilog2() as usize;
        let local_bits = layout.signature_stride().ilog2() as usize;
        // The ring row interval is not a complete aligned Boolean subcube.
        assert_ne!((layout.linear_rows() - N) % N, 0);
        for selected_instance in [None, Some(layout.batch() - 1), Some(layout.capacity() - 1)] {
            let mut linear_point = point(linear_rounds(&layout), &field);
            linear_point[0] = field.zero();
            if let Some(instance) = selected_instance {
                for (bit, coordinate) in linear_point[local_rows..].iter_mut().enumerate() {
                    *coordinate = unsigned(((instance >> bit) & 1) as u128, &field);
                }
            }
            let mut transcript = Blake3Transcript::new();
            let mut reference_transcript = transcript.clone();
            reference_transcript.absorb_slice(b"bitz/falcon1024-ct/terminal-collapse/v1");
            let eta = squeeze(&mut reference_transcript, &field).unwrap();
            let binding = prepare_binding_form(
                &mut transcript,
                &layout,
                &statement,
                &proof,
                &linear_point,
                &field,
            )
            .unwrap();
            assert_eq!(binding.eta, eta);
            assert_eq!(
                transcript.get_challenge::<u128>(),
                reference_transcript.get_challenge::<u128>()
            );

            let mut expected = DenseSink::new(layout.source_bits(), &field);
            let expected_target = emit_binding_reference(&binding, &mut expected);
            assert_eq!(binding.target().unwrap(), expected_target);
            let mut endpoint = point(source_rounds(&layout), &field);
            endpoint[1] = field.one();
            let actual_endpoint = binding.evaluate(&endpoint).unwrap();
            let mut padding_endpoint = endpoint.clone();
            padding_endpoint[local_bits..].fill(field.one());
            assert_eq!(binding.evaluate(&padding_endpoint).unwrap(), field.zero());
            assert!(
                binding.prover_ring_cache.get().is_none(),
                "verification built prover adjoints"
            );

            let mut actual = DenseSink::new(layout.source_bits(), &field);
            assert_eq!(binding.emit(&mut actual).unwrap(), expected_target);
            assert_eq!(
                actual
                    .values
                    .iter()
                    .zip(&expected.values)
                    .position(|(a, b)| a != b),
                None,
                "coefficient mismatch for keys {groups:?}, selected instance {selected_instance:?}",
            );
            // The compact word operator must preserve every source coefficient,
            // including overlapping isolated-bit terms and encoded signed words.
            actual.values.fill(field.zero());
            let mut previous = None;
            binding
                .for_each_coefficient(&mut |index, value| {
                    assert!(previous.is_none_or(|previous| index > previous));
                    previous = Some(index);
                    actual.add(index, value);
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                actual
                    .values
                    .iter()
                    .zip(&expected.values)
                    .position(|(a, b)| a != b),
                None,
                "compact coefficient mismatch for keys {groups:?}, selected instance {selected_instance:?}",
            );
            // Replaying reuses the word cache and has exactly the same output.
            actual.subtract = true;
            binding
                .for_each_coefficient(&mut |index, value| {
                    actual.add(index, value);
                    Ok(())
                })
                .unwrap();
            assert!(actual.values.iter().all(|value| *value == field.zero()));
            let cache = binding.prover_ring_cache.get().unwrap();
            let distinct = groups
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            assert_eq!(cache.adjoints.len(), distinct);
            assert_eq!(cache.instance_keys.len(), layout.batch());
            assert_eq!(
                actual_endpoint,
                evaluate_mle_in_place(&mut expected.values, &endpoint, &field).unwrap(),
            );
        }
    }
}

#[cfg(feature = "falcon-hybrid")]
#[test]
fn compact_binding_targets_match_constants_oracle_at_boolean_points() {
    let field = config();
    let layout = FalconSourceLayout::new_hybrid(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 0]);
    for selected in [0, 1, 2, 3] {
        let mut proof = full_terminal_fixture(&layout, &field);
        for (bit, coordinate) in proof.norm.point.iter_mut().enumerate() {
            *coordinate = unsigned(((selected * 1024 + 17) >> bit & 1) as u128, &field);
        }
        for (bit, coordinate) in proof.norm.instance_point.iter_mut().enumerate() {
            *coordinate = unsigned(((selected >> bit) & 1) as u128, &field);
        }
        for candidate in [
            0,
            HASH_TO_POINT_SAMPLES - 1,
            HASH_TO_POINT_SAMPLES,
            COMPACTION_LEAVES - 1,
        ] {
            for (bit, coordinate) in proof.compact_products.point.iter_mut().enumerate() {
                *coordinate = unsigned(
                    (((selected * COMPACTION_LEAVES + candidate) >> bit) & 1) as u128,
                    &field,
                );
            }
            for eta in [field.zero(), field.one(), unsigned(29, &field)] {
                let linear_point = point(linear_rounds(&layout), &field);
                let mut transcript = Blake3Transcript::new();
                let mut binding = prepare_binding_form(
                    &mut transcript,
                    &layout,
                    &statement,
                    &proof,
                    &linear_point,
                    &field,
                )
                .unwrap();
                binding.eta = eta;
                assert_eq!(
                    binding.target().unwrap(),
                    binding.emit(&mut ConstantsOnly).unwrap()
                );
            }
        }
    }
}

#[cfg(feature = "falcon-hybrid")]
#[test]
fn shared_key_binding_preserves_inner_sumcheck_transcript() {
    let field = config();
    let layout = FalconSourceLayout::new_hybrid(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 0]);
    let proof = full_terminal_fixture(&layout, &field);
    let linear_point = point(linear_rounds(&layout), &field);
    let mut transcript = Blake3Transcript::new();
    let binding = prepare_binding_form(
        &mut transcript,
        &layout,
        &statement,
        &proof,
        &linear_point,
        &field,
    )
    .unwrap();
    let mut reference = DenseSink::new(layout.source_bits(), &field);
    let target = emit_binding_reference(&binding, &mut reference);
    assert_eq!(binding.target().unwrap(), target);
    // A fixed arbitrary bit table suffices to compare this linear claim; its
    // dot product is the sumcheck target, independent of the outer fixtures.
    let mut words = vec![0u64; layout.source_bits() / 64];
    for (index, word) in words[..layout.batch() * layout.signature_stride() / 64]
        .iter_mut()
        .enumerate()
    {
        *word = 0xa5a5_0123_ffff_8001u64.rotate_left((index % 64) as u32);
    }
    let claim = reference
        .values
        .iter()
        .enumerate()
        .fold(field.zero(), |sum, (index, value)| {
            if (words[index / 64] >> (index % 64)) & 1 == 1 {
                field.add(&sum, value)
            } else {
                sum
            }
        });
    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let mut reference_transcript = transcript.clone();
    let old_coefficients = |index: usize| Ok(reference.values[index]);
    let expected = prove_inner_sumcheck(
        &field,
        &mut reference_transcript,
        claim,
        PackedInput::new(
            &old_coefficients,
            &words,
            source_rounds(&layout),
            layout.source_bits(),
            4,
        ),
        (),
        &mut crate::sumcheck::UngrindedRoundBoundary,
    )
    .unwrap();
    let streamed = StreamingMle::new(&binding);
    let actual = prove_inner_sumcheck(
        &field,
        &mut transcript,
        claim,
        PackedInput::new(
            &streamed,
            &words,
            source_rounds(&layout),
            layout.source_bits(),
            4,
        ),
        (),
        &mut crate::sumcheck::UngrindedRoundBoundary,
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        transcript.get_challenge::<u128>(),
        reference_transcript.get_challenge::<u128>()
    );
    assert_eq!(
        binding.evaluate(&actual.point).unwrap(),
        actual.terminal_evaluations[0]
    );
}

#[test]
fn norm_binding_matches_weighted_words_and_slack_for_padded_batch() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let mut traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
    traces[1].s1[0] = 0;
    traces[2].s1[N - 1] = -6_144;
    traces[1].norm_slack += 7;
    traces[2].norm_slack += 13;
    traces[1].norm -= 7;
    traces[2].norm -= 13;
    let source = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
        &traces,
    )
    .unwrap();
    let mut proof = terminal_fixture(Vec::new(), &field);
    proof.norm.instance_point = point(2, &field);
    proof.norm.point = point(12, &field);
    let instance_weights = eq_table(&proof.norm.instance_point, &field).unwrap();
    let weights = eq_table(&proof.norm.point, &field).unwrap();
    for (instance, trace) in traces.iter().enumerate() {
        for i in 0..N {
            for (side, word) in [trace.s1[i] as i128, trace.signature.s2[i] as i128]
                .into_iter()
                .enumerate()
            {
                let unweighted = field.mul(&weights[instance * N + i], &signed(word, &field));
                proof.norm.terminal[side][0] = field.add(
                    &proof.norm.terminal[side][0],
                    &field.mul(&instance_weights[instance], &unweighted),
                );
                proof.norm.terminal[side][1] =
                    field.add(&proof.norm.terminal[side][1], &unweighted);
            }
        }
        proof.norm.slack = field.add(
            &proof.norm.slack,
            &field.mul(
                &instance_weights[instance],
                &unsigned(trace.norm_slack.into(), &field),
            ),
        );
    }
    struct DotSink<'a> {
        source: &'a FalconSourceWitness,
        field: &'a Cfg,
        value: F,
    }
    impl CoefficientSink for DotSink<'_> {
        fn add(&mut self, index: usize, coefficient: F) {
            let row_vars = self.source.layout().row_vars();
            let row = index & ((1 << row_vars) - 1);
            let bit = self.source.rows()[index >> row_vars][row / 64] >> (row % 64) & 1;
            if bit != 0 {
                self.value = self.field.add(&self.value, &coefficient);
            }
        }
    }
    let eta = unsigned(19, &field);
    let evaluate = |proof: &FalconPiopProof| {
        let mut sink = DotSink {
            source: &source,
            field: &field,
            value: field.zero(),
        };
        let mut target = field.zero();
        let mut scale = eta;
        add_norm_claims(
            &mut sink,
            &mut target,
            &mut scale,
            eta,
            &layout,
            proof,
            &field,
        )
        .unwrap();
        (sink.value, target)
    };
    let (actual, target) = evaluate(&proof);
    assert_eq!(actual, target);
    let mut bad = proof.clone();
    bad.norm.terminal[0][0] = bad.norm.terminal[0][1];
    let (actual, target) = evaluate(&bad);
    assert_ne!(actual, target);
    let mut bad = proof;
    bad.norm.slack = traces.iter().fold(field.zero(), |sum, trace| {
        field.add(&sum, &unsigned(trace.norm_slack.into(), &field))
    });
    let (actual, target) = evaluate(&bad);
    assert_ne!(actual, target);
}

#[cfg(feature = "falcon-hybrid")]
#[test]
fn public_signature_bytes_are_constrained_to_the_committed_source() {
    let field = config();
    let layout = FalconSourceLayout::new_hybrid(3).unwrap();
    let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
    let source = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE.as_slice(); 3],
        &[SIGNATURE.as_slice(); 3],
        &[trace.clone(), trace.clone(), trace],
    )
    .unwrap();
    let statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY.as_slice(); 3],
        &[MESSAGE.as_slice(); 3],
        &[SIGNATURE.as_slice(); 3],
    )
    .unwrap();
    let proof = full_terminal_fixture(&layout, &field);

    struct SourceDot<'a> {
        source: &'a FalconSourceWitness,
        field: &'a Cfg,
        value: F,
    }
    impl CoefficientSink for SourceDot<'_> {
        fn add(&mut self, index: usize, coefficient: F) {
            if self.source.bit(index) {
                self.value = self.field.add(&self.value, &coefficient);
            }
        }
    }

    // Select one public byte equation exactly. The source and transcript stay
    // fixed: rejection must come from the equality, not changed challenges.
    for (instance, nonce_byte, coefficient) in [
        (0, Some(0), None),
        (1, Some(39), None),
        (2, None, Some(0)),
        (2, None, Some(N - 1)),
    ] {
        let mut changed = statement.clone();
        if let Some(byte) = nonce_byte {
            changed.signatures[instance].nonce[byte] ^= 1;
        }
        if let Some(index) = coefficient {
            let value = &mut changed.signatures[instance].s2[index];
            *value += if *value == 2047 { -1 } else { 1 };
        }
        let encoded = super::super::encode_signature_ct(&changed.signatures[instance]).unwrap();
        let byte = encoded
            .iter()
            .zip(SIGNATURE)
            .position(|(changed, original)| changed != original)
            .unwrap();
        let row = instance * layout.linear_stride() + 1 + 256 + byte;
        let linear_point: Vec<_> = (0..linear_rounds(&layout))
            .map(|bit| unsigned(((row >> bit) & 1) as u128, &field))
            .collect();
        let weights = factored_weights(&linear_point, &field).unwrap();
        for (public, expected) in [
            (&statement, field.zero()),
            (
                &changed,
                signed(
                    i128::from(SIGNATURE[byte]) - i128::from(encoded[byte]),
                    &field,
                ),
            ),
        ] {
            let mut sink = SourceDot {
                source: &source,
                field: &field,
                value: field.zero(),
            };
            let constant =
                add_linear_constraints(&mut sink, &weights, &layout, public, &field).unwrap();
            assert_eq!(field.add(&sink.value, &constant), expected);

            let mut transcript = Blake3Transcript::new();
            let mut binding = prepare_binding_form(
                &mut transcript,
                &layout,
                public,
                &proof,
                &linear_point,
                &field,
            )
            .unwrap();
            binding.eta = field.zero();
            assert_eq!(
                binding.target().unwrap(),
                field.sub(&field.zero(), &constant)
            );
        }
        assert_ne!(SIGNATURE[byte], encoded[byte]);
    }
}

#[cfg(feature = "falcon-hybrid")]
#[test]
fn compact_prefix_binds_word_products_and_weighted_norm_to_source() {
    let layout = FalconSourceLayout::new_hybrid(3).unwrap();
    let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
    let traces = vec![trace; 3];
    let statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY, PUBLIC_KEY, PUBLIC_KEY],
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
    )
    .unwrap();
    let source = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
        &traces,
    )
    .unwrap();
    // Hash the packed source before challenges, as a lightweight fixed-source
    // test harness. The complete adapter replaces this with its PCS roots.
    let mut hasher = blake3::Hasher::new();
    for column in source.rows() {
        for word in column {
            hasher.update(&word.to_le_bytes());
        }
    }
    let commitment = hasher.finalize();
    let fresh_transcript = || {
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(commitment.as_bytes());
        transcript
    };
    let (proof, claim) = prove_binding_prefix(
        &mut fresh_transcript(),
        &layout,
        &statement,
        &traces,
        &source,
        100,
    )
    .unwrap();
    assert!(proof.piop.keccak_chi.is_none());
    assert_eq!(proof.piop.compact_products.point.len(), 15);
    assert_eq!(proof.binding_point.len(), 20);
    let verified =
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &proof, 100).unwrap();
    assert_eq!(verified.modulus, claim.modulus);
    assert_eq!(verified.value, claim.value);
    assert_eq!(verified.row_weights, claim.row_weights);
    assert_eq!(verified.col_weights, claim.col_weights);
    let field = field_from_modulus(claim.modulus).unwrap();
    let mut dot = field.zero();
    for (column, words) in source.rows().iter().enumerate() {
        let col_weight = unsigned(claim.col_weights[column], &field);
        for (word_index, &word) in words.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let row = 64 * word_index + remaining.trailing_zeros() as usize;
                dot = field.add(
                    &dot,
                    &field.mul(&col_weight, &unsigned(claim.row_weights[row], &field)),
                );
                remaining &= remaining - 1;
            }
        }
    }
    assert_eq!(canonical(dot, &field), claim.value);
    let mut bad = proof.clone();
    bad.piop.norm.instance_point[0] = field.add(&bad.piop.norm.instance_point[0], &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
    let mut bad = proof;
    bad.piop.compact_products.terminal.cx =
        field.add(&bad.piop.compact_products.terminal.cx, &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
}
