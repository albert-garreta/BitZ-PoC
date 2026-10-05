use super::super::{
    piop::{
        CompactionLeafProof, CompactionProof, NormProof, PrimeProductForestProof,
        PrimeProductTreeProof, QuadraticRelationProof,
    },
    verification_trace,
};
use super::*;
use crate::sumcheck::outer::OuterEvaluations;

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

struct ConstantsOnly;
impl CoefficientSink for ConstantsOnly {
    fn enabled(&self) -> bool {
        false
    }
    fn add(&mut self, _: usize, _: F) {}
}

// Bit-level current-layout oracle, independent of the compiled word template.
fn add_linear_constraints(
    coefficients: &mut impl CoefficientSink,
    weights: &crate::poly::mle::EqualityWeights<F>,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    field: &Cfg,
) -> Result<F, FalconError> {
    let offsets = layout.offsets();
    let mut constant = field.zero();
    let emit = coefficients.enabled();
    for instance in coefficients.instances(layout.batch()) {
        let base = instance * layout.signature_stride();
        let mut row = 0usize;
        let mut residual = |terms: &[(usize, i128)], c: i128| {
            let row_index = row;
            row += 1;
            if !emit && c == 0 {
                return;
            }
            let weight = weights.at(instance * layout.linear_stride() + row_index);
            for &(index, coefficient) in terms.iter().filter(|_| emit) {
                coefficients.add(base + index, mul_i(weight, coefficient, field));
            }
            if c != 0 {
                constant = field.add(&constant, &mul_i(weight, c, field));
            }
        };

        residual(&[(offsets.shared_one, 1)], -1);
        for bit in 0..256 {
            let expected = (statement.messages[instance][bit / 8] >> (bit % 8)) & 1;
            residual(&[(offsets.message + bit, 1)], -i128::from(expected));
        }
        for (byte, expected) in encode_signature_ct(&statement.signatures[instance])?
            .into_iter()
            .enumerate()
        {
            // The authenticated source contains bits, so each byte sum is in
            // [0,255] and equality in the proof field fixes all eight bits.
            let terms: [(usize, i128); 8] = std::array::from_fn(|bit| {
                (offsets.encoded_signature + 8 * byte + bit, 1i128 << bit)
            });
            residual(&terms, -i128::from(expected));
        }

        for i in 0..HASH_TO_POINT_SAMPLES {
            let mut division = Vec::with_capacity(34);
            for bit in 0..16 {
                division.push((offsets.hash_words + 16 * i + bit, 1i128 << bit));
            }
            push_unsigned_terms(
                &mut division,
                offsets.hash_quotients + 3 * i,
                3,
                -i128::from(Q),
            );
            push_value_terms(&mut division, offsets.hash_remainders + 14 * i, -1);
            residual(&division, 0);

            let mut prefix = Vec::with_capacity(23);
            push_unsigned_terms(&mut prefix, offsets.hash_prefixes + 11 * (i + 1), 11, 1);
            push_unsigned_terms(&mut prefix, offsets.hash_prefixes + 11 * i, 11, -1);
            prefix.push((offsets.hash_accept_ands + i, 1));
            residual(&prefix, -1);
        }
        let mut prefix_zero = Vec::with_capacity(11);
        push_unsigned_terms(&mut prefix_zero, offsets.hash_prefixes, 11, 1);
        residual(&prefix_zero, 0);
        residual(
            &[(offsets.hash_prefixes + 11 * HASH_TO_POINT_SAMPLES + 10, 1)],
            -1,
        );

        if let Some(offset) = layout.public_key_offset() {
            for j in 0..N {
                let terms: [(usize, i128); 14] =
                    std::array::from_fn(|bit| (offset + 14 * j + bit, 1i128 << bit));
                residual(&terms, -i128::from(statement.public_keys[instance].h[j]));
            }
        }

        if row != layout.linear_rows() {
            return Err(piop(format!(
                "linear row inventory mismatch: built {row} rows"
            )));
        }
    }
    Ok(constant)
}

fn push_value_terms(terms: &mut Vec<(usize, i128)>, base: usize, scale: i128) {
    for bit in 0..14 {
        let weight = if bit == 13 { 4097 } else { 1i128 << bit };
        terms.push((base + bit, scale * weight));
    }
}

fn push_unsigned_terms(terms: &mut Vec<(usize, i128)>, base: usize, width: usize, scale: i128) {
    for bit in 0..width {
        terms.push((base + bit, scale * (1i128 << bit)));
    }
}

fn add_norm_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = factored_weights(&proof.norm.point, field)?;
    let instance_weights =
        eq_table(&proof.norm.instance_point, field).map_err(|error| piop(error.to_string()))?;
    add_norm_claims_prepared(
        coefficients,
        target,
        scale,
        eta,
        layout,
        proof,
        field,
        &weights,
        &instance_weights,
    )
}

fn add_compaction_product_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = factored_weights(&proof.compact_products.point, field)?;
    add_compaction_product_claims_prepared(
        coefficients,
        target,
        scale,
        eta,
        layout,
        proof,
        field,
        &weights,
    )
}

fn add_product_tree_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let offsets = layout.offsets();
    let instances = coefficients.instances(layout.batch());
    for _ in 0..instances.start {
        *scale = field.mul(scale, &eta);
    }
    for instance in instances {
        let base = instance * layout.signature_stride();
        let tree = &proof.compaction[instance].output;
        let weights = eq_table(&tree.terminal_point, field).map_err(|e| piop(e.to_string()))?;
        let mut constant = field.zero();
        for (i, &leaf_weight) in weights.iter().enumerate() {
            let weight = field.mul(scale, &leaf_weight);
            if i < N {
                constant = field.add(
                    &constant,
                    &field.mul(
                        &weight,
                        &field.add(
                            &proof.compaction_gamma,
                            &field.mul(&proof.compaction_rank_scale, &unsigned(i as u128, field)),
                        ),
                    ),
                );
                add_unsigned_scaled(
                    coefficients,
                    base + offsets.hash_point + 14 * i,
                    14,
                    weight,
                    field,
                );
            } else {
                constant = field.add(&constant, &weight);
            }
        }
        add_claim_target(target, *scale, tree.terminal_claim, constant, field);
        *scale = field.mul(scale, &eta);
    }
    Ok(())
}

fn emit_binding_reference(binding: &BindingForm<'_>, sink: &mut impl CoefficientSink) -> F {
    let constant = add_linear_constraints(
        sink,
        &binding.linear_weights,
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
    native::add_leaf_claims(
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
    if let Some(native) = &binding.native_claim {
        // Independent bit-level expansion of sum W*(c - bounded14(v) + 6144).
        let offsets = binding.layout.offsets();
        let field = binding.field;
        let mut constant = field.zero();
        for (index, &weight) in native.weights.iter().enumerate() {
            let instance = index / N;
            let coefficient = index % N;
            let base = instance * binding.layout.signature_stride() + 14 * coefficient;
            let scaled = field.mul(&scale, &weight);
            constant = field.add(&constant, &mul_i(scaled, 6_144, field));
            for bit in 0..14 {
                sink.add(
                    base + offsets.hash_point + bit,
                    mul_i(scaled, 1 << bit, field),
                );
                let decoded_weight = if bit == 13 { 4_097 } else { 1 << bit };
                sink.add(
                    base + offsets.s1 + bit,
                    mul_i(scaled, -decoded_weight, field),
                );
            }
        }
        target = field.add(
            &target,
            &field.sub(&field.mul(&scale, &native.target), &constant),
        );
    }
    target
}

fn terminal_fixture(field: &Cfg) -> FalconPiopProof {
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
        point: Vec::new(),
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
        compact_products: relation,
        fingerprint_nonce: None,
        compaction_gamma: field.zero(),
        compaction_rank_scale: field.zero(),
        compaction: Vec::new(),
        compaction_forest: PrimeProductForestProof { layers: Vec::new() },
        compaction_leaf: CompactionLeafProof {
            instance_point: Vec::new(),
            instance_nonce: None,
            sumcheck: SumcheckProof {
                round_polynomials: Vec::new(),
            },
            terminal: [field.zero(); 3],
            point: Vec::new(),
            grinding_nonces: Vec::new(),
        },
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

fn full_terminal_fixture(layout: &FalconSourceLayout, field: &Cfg) -> FalconPiopProof {
    let batch_vars = layout.capacity().ilog2() as usize;
    let mut proof = terminal_fixture(field);
    proof.norm.point = point(10 + batch_vars, field);
    proof.norm.instance_point = point(batch_vars, field);
    proof.norm.terminal = [
        [unsigned(3, field), unsigned(5, field)],
        [unsigned(7, field), unsigned(11, field)],
    ];
    proof.norm.slack = unsigned(41, field);
    proof.compact_products.point = point(11 + batch_vars, field);
    proof.compaction_gamma = unsigned(29, field);
    proof.compaction_rank_scale = unsigned(31, field);
    proof.compaction = (0..layout.batch())
        .map(|instance| {
            let tree = |side: usize| {
                let terminal_point = point(11, field);
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
    proof.compaction_leaf = CompactionLeafProof {
        instance_point: point(batch_vars, field),
        instance_nonce: None,
        sumcheck: SumcheckProof {
            round_polynomials: Vec::new(),
        },
        terminal: [
            unsigned(17, field),
            unsigned(19, field),
            unsigned(23, field),
        ],
        point: point(11 + batch_vars, field),
        grinding_nonces: Vec::new(),
    };
    proof
}

fn native_claim_fixture(
    layout: &FalconSourceLayout,
    field: &Cfg,
) -> super::super::native_ring::PreparedNativeClaim {
    super::super::native_ring::PreparedNativeClaim {
        weights: (0..layout.batch() * N)
            .map(|i| unsigned((i * i + 17 * i + 37) as u128, field))
            .collect(),
        target: unsigned(43, field),
    }
}

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

#[test]
fn shared_prime_streaming_overlay_matches_independent_bit_oracle() {
    let field = config();
    let layout = FalconSourceLayout::new_shared_prime(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 2]);
    let proof = full_terminal_fixture(&layout, &field);
    let linear_point = point(linear_rounds(&layout), &field);
    for scale in [
        field.zero(),
        field.one(),
        field.neg(&field.one()),
        unsigned(29, &field),
    ] {
        let mut transcript = Blake3Transcript::new();
        let integer = prepare_binding_form_optional(
            &mut transcript,
            &layout,
            &statement,
            &proof,
            &linear_point,
            &field,
            None,
        )
        .unwrap();
        let mut expected = DenseSink::new(layout.source_bits(), &field);
        let integer_target = emit_binding_reference(&integer, &mut expected);
        assert_eq!(integer.target().unwrap(), integer_target);
        let mut row = vec![field.zero(); layout.capacity()];
        for (s, value) in row[..layout.batch()].iter_mut().enumerate() {
            *value = unsigned((7 + s * 13) as u128, &field);
        }
        let column: Vec<_> = (0..layout.signature_stride())
            .map(|h| {
                if h < layout.live_bits() && h % 5 != 0 {
                    unsigned((17 + h * h) as u128, &field)
                } else {
                    field.zero()
                }
            })
            .collect();
        for (i, value) in expected.values.iter_mut().enumerate() {
            *value = field.add(
                &field.mul(&scale, value),
                &field.mul(&row[i / column.len()], &column[i % column.len()]),
            );
        }
        let ring_target = unsigned(53, &field);
        let joined = JoinedBinding::new(
            integer,
            Some(super::super::shared_ring::ProjectedClaim {
                row,
                column,
                target: ring_target,
            }),
            scale,
        )
        .unwrap();
        assert_eq!(
            joined.target().unwrap(),
            field.add(&ring_target, &field.mul(&scale, &integer_target))
        );
        let mut actual = vec![field.zero(); layout.source_bits()];
        joined
            .for_each_coefficient(&mut |i, value| {
                actual[i] = field.add(&actual[i], &value);
                Ok(())
            })
            .unwrap();
        assert_eq!(actual, expected.values);
        for width in [1, 2, 4, 8, 16] {
            actual.fill(field.zero());
            for s in 0..layout.capacity() {
                let mut next = s * layout.signature_stride();
                joined
                    .for_each_partition_block(s, width, &mut |base, values| {
                        assert!(base >= next);
                        next = base + width;
                        actual[base..base + width].copy_from_slice(values);
                        Ok(())
                    })
                    .unwrap()
                    .unwrap();
            }
            assert_eq!(actual, expected.values);
        }
        assert!(
            joined
                .for_each_partition_block(0, 32, &mut |_, _| Ok(()))
                .unwrap()
                .is_err()
        );
        let weights = eq_table(&point(3, &field), &field).unwrap();
        let mut folded = vec![field.zero(); layout.source_bits() / weights.len()];
        for s in 0..layout.capacity() {
            let mut next = s * layout.signature_stride() / weights.len();
            joined
                .for_each_partition_folded_final(s, &weights, &mut |index, value| {
                    assert!(index >= next);
                    next = index + 1;
                    folded[index] = value;
                    Ok(())
                })
                .unwrap()
                .unwrap();
        }
        for (chunk, actual) in expected.values.chunks_exact(weights.len()).zip(folded) {
            let expected = chunk
                .iter()
                .zip(&weights)
                .fold(field.zero(), |sum, (v, w)| {
                    field.add(&sum, &field.mul(v, w))
                });
            assert_eq!(actual, expected);
        }
        for s in 0..layout.capacity() {
            let byte = |index: usize| ((index / 8).wrapping_mul(37).wrapping_add(91) & 255) as u8;
            let base = s * layout.signature_stride();
            let mut expected_buckets = vec![[field.zero(); 8]; 256];
            for (i, chunk) in expected.values[base..base + layout.signature_stride()]
                .chunks_exact(8)
                .enumerate()
            {
                let b = usize::from(byte(base + 8 * i));
                if b == 0 {
                    continue;
                }
                for (sum, value) in expected_buckets[b].iter_mut().zip(chunk) {
                    *sum = field.add(sum, value);
                }
            }
            let mut buckets = vec![[field.zero(); 8]; 256];
            let mut next = 0;
            joined
                .for_each_partition_byte_bucket(
                    s,
                    &mut |index, lanes| {
                        assert!((1..=8).contains(&lanes));
                        Ok(byte(index))
                    },
                    &mut |b, values| {
                        assert!(usize::from(b) >= next);
                        next = usize::from(b) + 1;
                        buckets[usize::from(b)] = *values;
                        Ok(())
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(buckets, expected_buckets);
        }
        let endpoint = point(source_rounds(&layout), &field);
        assert_eq!(
            joined.evaluate(&endpoint).unwrap(),
            evaluate_mle_in_place(&mut expected.values, &endpoint, &field).unwrap()
        );
    }
}

#[test]
fn native_binding_matches_dense_reference_at_degenerate_points() {
    let field = config();
    for groups in [&[0, 0, 0][..], &[0, 1, 0, 2, 1], &[0, 1, 2]] {
        let layout = FalconSourceLayout::new(groups.len()).unwrap();
        let statement = statement_with_key_groups(groups);
        let proof = full_terminal_fixture(&layout, &field);
        let local_rows = layout.linear_stride().ilog2() as usize;
        let local_bits = layout.signature_stride().ilog2() as usize;
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
                native_claim_fixture(&layout, &field),
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
            let mut actual = DenseSink::new(layout.source_bits(), &field);
            // The compact word operator must preserve every source coefficient,
            // including overlapping isolated-bit terms and encoded signed words.
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
            assert_eq!(
                actual_endpoint,
                evaluate_mle_in_place(&mut expected.values, &endpoint, &field).unwrap(),
            );
        }
    }
}

#[test]
fn compact_binding_targets_match_constants_oracle_at_boolean_points() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
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
                    native_claim_fixture(&layout, &field),
                )
                .unwrap();
                binding.eta = eta;
                assert_eq!(
                    binding.target().unwrap(),
                    emit_binding_reference(&binding, &mut ConstantsOnly)
                );
            }
        }
    }
}

#[test]
fn native_binding_preserves_inner_sumcheck_transcript() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
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
        native_claim_fixture(&layout, &field),
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
    let reference_coefficients = |index: usize| Ok(reference.values[index]);
    let expected = prove_inner_sumcheck(
        &field,
        &mut reference_transcript,
        claim,
        PackedInput::new(
            &reference_coefficients,
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
    let reference_challenge = reference_transcript.get_challenge::<u128>();
    // K=3 uses direct word-to-byte buckets; K=4 retains block expansion.
    for prefix in [3, 4] {
        let mut actual_transcript = transcript.clone();
        let actual = prove_inner_sumcheck(
            &field,
            &mut actual_transcript,
            claim,
            PackedInput::new(
                &streamed,
                &words,
                source_rounds(&layout),
                layout.source_bits(),
                prefix,
            ),
            (),
            &mut crate::sumcheck::UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual_transcript.get_challenge::<u128>(),
            reference_challenge
        );
        assert_eq!(
            binding.evaluate(&actual.point).unwrap(),
            actual.terminal_evaluations[0]
        );
    }
}

#[test]
fn bounded_norm_binding_matches_signed_words_and_slack_for_padded_batch() {
    check_norm_binding(FalconSourceLayout::new(3).unwrap());
}

#[test]
fn public_signature_bytes_are_constrained_to_the_committed_source() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
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
                native_claim_fixture(&layout, &field),
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

#[test]
fn native_affine_claim_authenticates_the_same_committed_coefficients() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let mut traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
    let statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY, PUBLIC_KEY, PUBLIC_KEY],
        &[MESSAGE, MESSAGE, MESSAGE],
        &[SIGNATURE, SIGNATURE, SIGNATURE],
    )
    .unwrap();
    let pack = |traces: &[super::super::FalconVerificationTrace]| {
        FalconSourceWitness::from_traces(
            layout,
            &[MESSAGE, MESSAGE, MESSAGE],
            &[SIGNATURE, SIGNATURE, SIGNATURE],
            traces,
        )
        .unwrap()
    };
    let source = pack(&traces);
    let mut transcript = Blake3Transcript::new();
    for row in source.rows() {
        for word in row {
            transcript.absorb_slice(&word.to_le_bytes());
        }
    }
    let mut verifier = transcript.clone();
    let (native_proof, claim) = super::super::native_ring::prove(
        &mut transcript,
        &layout,
        &statement,
        &traces,
        &field,
        100,
    )
    .unwrap();
    assert_eq!(
        super::super::native_ring::verify(
            &mut verifier,
            &layout,
            &statement,
            &native_proof,
            &field,
            100,
        )
        .unwrap(),
        claim
    );
    let proof = full_terminal_fixture(&layout, &field);
    let binding = prepare_binding_form(
        &mut transcript,
        &layout,
        &statement,
        &proof,
        &point(linear_rounds(&layout), &field),
        &field,
        claim,
    )
    .unwrap();
    let mut dense = DenseSink::new(layout.source_bits(), &field);
    let mut target = field.zero();
    binding
        .add_native_claim(&mut dense, &mut target, unsigned(47, &field))
        .unwrap();
    assert_eq!(dot_packed_source(&dense.values, &source, &field), target);
    assert!(
        dense.values[layout.batch() * layout.signature_stride()..]
            .iter()
            .all(|x| *x == field.zero())
    );

    traces[1].hash_to_point.point[17] ^= 1;
    assert_ne!(
        dot_packed_source(&dense.values, &pack(&traces), &field),
        target
    );
    traces[1].hash_to_point.point[17] ^= 1;
    traces[2].s1[31] += if traces[2].s1[31] == 6_144 { -1 } else { 1 };
    assert_ne!(
        dot_packed_source(&dense.values, &pack(&traces), &field),
        target
    );
}

#[test]
fn compact_prefix_binds_word_products_and_weighted_norm_to_source() {
    let layout = FalconSourceLayout::new(3).unwrap();
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
    let (proof, row_weights, bridge_claim) = prove_binding_prefix(
        &mut fresh_transcript(),
        &layout,
        &statement,
        &traces,
        &source,
        100,
    )
    .unwrap();
    let PiopProof::Native(integer) = &proof.piop else {
        panic!("native PIOP expected")
    };
    assert_eq!(integer.compact_products.point.len(), 13);
    assert_eq!(proof.binding_point.len(), 19);
    assert_eq!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &proof, 100).unwrap(),
        bridge_claim
    );
    let field = field_from_modulus(integer.modulus).unwrap();
    assert_eq!(
        row_weights,
        canonical_eq_weights(&proof.binding_point[..layout.row_vars()], &field).unwrap(),
    );
    let col_weights =
        canonical_eq_weights(&proof.binding_point[layout.row_vars()..], &field).unwrap();
    let mut dot = field.zero();
    for (column, words) in source.rows().iter().enumerate() {
        let col_weight = unsigned(col_weights[column], &field);
        for (word_index, &word) in words.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let row = 64 * word_index + remaining.trailing_zeros() as usize;
                dot = field.add(
                    &dot,
                    &field.mul(&col_weight, &unsigned(row_weights[row], &field)),
                );
                remaining &= remaining - 1;
            }
        }
    }
    assert_eq!(dot, proof.binding_terminal[1]);
    let mut bad = proof.clone();
    let PiopProof::Native(integer) = &mut bad.piop else {
        panic!("native PIOP expected")
    };
    integer.norm.instance_point[0] = field.add(&integer.norm.instance_point[0], &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
    let mut bad = proof;
    let PiopProof::Native(integer) = &mut bad.piop else {
        panic!("native PIOP expected")
    };
    integer.compact_products.terminal.cx =
        field.add(&integer.compact_products.terminal.cx, &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
}

fn check_norm_binding(layout: FalconSourceLayout) {
    let field = config();
    let mut traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
    traces[0].s1[1] = 6_144;
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
    let mut proof = terminal_fixture(&field);
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

#[test]
fn shared_forest_weights_require_candidate_and_output_endpoints_to_match() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let proof = full_terminal_fixture(&layout, &field);
    assert!(native::LeafWeights::new(&layout, &proof, &field).is_ok());
    for output in [false, true] {
        let mut bad = proof.clone();
        let pair = &mut bad.compaction[1];
        let tree = if output {
            &mut pair.output
        } else {
            &mut pair.candidate
        };
        tree.terminal_point[0] = field.add(&tree.terminal_point[0], &field.one());
        assert!(native::LeafWeights::new(&layout, &bad, &field).is_err());
    }
}
