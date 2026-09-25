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

// Only terminal data is consumed by the binder; no proof generation is needed.
fn terminal_fixture(keccak_point: Vec<F>, field: &Cfg) -> FalconPiopProof {
    let relation = QuadraticRelationProof {
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
            claims: [field.zero(); 2],
            slack: field.zero(),
            sumchecks: core::array::from_fn(|_| SumcheckProof {
                round_polynomials: Vec::new(),
            }),
            terminal: [[field.zero(); 2]; 2],
            point: Vec::new(),
            grinding_nonces: Vec::new(),
        },
        keccak_chi: relation.clone(),
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
    let weights = eq_table(&proof.keccak_chi.point, field).unwrap();
    let half = KECCAK_GATE_STRIDE * layout.capacity();
    let offsets = FalconSourceOffsets::new();
    let claims = [
        proof.keccak_chi.terminal.ax,
        proof.keccak_chi.terminal.bx,
        proof.keccak_chi.terminal.cx,
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
        add_linear_constraints(&mut dense, &weights, &ring, &layout, &statement, &field).unwrap();
    assert_eq!(
        add_linear_constraints(
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
    let first_round = 1 + 256 + 8 + N;
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
        add_linear_constraints(
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

#[test]
fn full_binding_endpoint_matches_dense_with_distinct_keys_and_padded_instance() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let batch_vars = layout.capacity().ilog2() as usize;
    let mut statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY, PUBLIC_KEY, PUBLIC_KEY],
        &[MESSAGE, MESSAGE, MESSAGE],
    )
    .unwrap();
    statement.public_keys[1].h[0] = (statement.public_keys[1].h[0] + 1) % Q as u16;
    statement.public_keys[2].h[N - 1] = (statement.public_keys[2].h[N - 1] + 3) % Q as u16;
    statement.messages[1][0] ^= 1;
    statement.messages[2][31] ^= 128;
    let mut proof = terminal_fixture(point(21 + batch_vars, &field), &field);
    proof.norm.point = point(10 + batch_vars, &field);
    proof.norm.terminal = [
        [unsigned(3, &field), unsigned(5, &field)],
        [unsigned(7, &field), unsigned(11, &field)],
    ];
    proof.norm.slack = unsigned(41, &field);
    proof.compact_products.point = point(16 + batch_vars, &field);
    proof.compaction_gamma = unsigned(29, &field);
    proof.compaction_rank_scale = unsigned(31, &field);
    proof.compaction = (0..layout.batch())
        .map(|instance| {
            let tree = |side: usize| {
                let mut terminal_point = point(11, &field);
                terminal_point[0] = unsigned((instance * 2 + side + 2) as u128, &field);
                PrimeProductTreeProof {
                    root: field.one(),
                    terminal_point,
                    terminal_claim: unsigned((instance + side + 13) as u128, &field),
                }
            };
            CompactionProof {
                candidate: tree(0),
                output: tree(1),
            }
        })
        .collect();
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
