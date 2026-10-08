use super::super::hash_to_point_selection::{REJECTION_ROWS, REJECTION_STRIDE};
use super::super::{
    piop::{FalconPiopClaims, HashToPointRejectionClaims, NormClaims},
    verification_trace,
};
use super::*;
use crate::sumcheck::inner::packed::{PackedInput, StreamingMle};

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

#[test]
fn mixed_integer_scaling_matches_field_embedding() {
    let field = config();
    for value in [field.zero(), field.one(), field.neg(&field.one()), unsigned(12345, &field)] {
        for coefficient in [i128::MIN, -12289, -6144, -1, 0, 1, 256, 12288, i128::MAX] {
            assert_eq!(
                mul_i(value, coefficient, &field),
                field.mul(&value, &signed(coefficient, &field)),
            );
        }
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
    selection: &Selection,
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
            let terms: [(usize, i128); 8] =
                std::array::from_fn(|bit| (layout.signature_bit(byte, bit), 1i128 << bit));
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
        }
        for j in 0..HASH_TO_POINT_SAMPLES {
            if j <= selection.cutoff(instance) {
                residual(
                    &[(offsets.hash_accept_ands + j, 1)],
                    -i128::from(!selection.selected(instance, j)),
                );
            } else {
                residual(&[], 0);
            }
        }
        for (k, &selected) in selection.indices(instance).iter().enumerate() {
            let mut terms = Vec::new();
            push_unsigned_terms(&mut terms, layout.hash_point_bit(k, 0), 14, 1);
            push_value_terms(
                &mut terms,
                offsets.hash_remainders + 14 * usize::from(selected),
                -1,
            );
            residual(&terms, 0);
        }

        for j in 0..N {
            let terms: [(usize, i128); 14] =
                std::array::from_fn(|bit| (layout.public_key_bit(j, bit), 1i128 << bit));
            residual(&terms, -i128::from(statement.public_keys[instance].h[j]));
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
    proof: FalconPiopClaimRef<'_>,
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

// Bit-level transpose oracle: expand each row independently before combining
// endpoints, instead of aggregating atoms and streaming polynomial words.
fn add_rejection_claims(
    coefficients: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: FalconPiopClaimRef<'_>,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = factored_weights(proof.h2p_rejection.point, field)?;
    let offsets = layout.offsets();
    for coordinate in 0..3 {
        for instance in coefficients.instances(layout.batch()) {
            let base = instance * layout.signature_stride();
            for j in 0..HASH_TO_POINT_SAMPLES {
                let address = match coordinate {
                    0 => offsets.hash_quotients + 3 * j + 2,
                    1 => offsets.hash_quotients + 3 * j,
                    _ => offsets.hash_accept_ands + j,
                };
                coefficients.add(
                    base + address,
                    field.mul(scale, &weights.at(instance * REJECTION_STRIDE + j)),
                );
            }
        }
        add_claim_target(
            target,
            *scale,
            proof.h2p_rejection.terminal[coordinate],
            field.zero(),
            field,
        );
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
        binding.selection,
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
    add_rejection_claims(
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

fn terminal_fixture(field: &Cfg) -> FalconPiopClaims {
    FalconPiopClaims {
        modulus: field.modulus_u128(),
        norm: NormClaims {
            instance_point: Vec::new(),
            terminal: field.zero(),
            slack: field.zero(),
            point: Vec::new(),
        },
        h2p_rejection: HashToPointRejectionClaims {
            terminal: [field.zero(); 3],
            point: Vec::new(),
        },
    }
}

fn complete_claim_fixture(layout: &FalconSourceLayout, field: &Cfg) -> FalconPiopClaims {
    let batch_vars = layout.capacity().ilog2() as usize;
    let mut claims = terminal_fixture(field);
    claims.norm.point = point(COEFFICIENT_LOG + 1 + batch_vars, field);
    claims.norm.instance_point = point(batch_vars, field);
    claims.norm.terminal = unsigned(3, field);
    claims.norm.slack = unsigned(41, field);
    claims.h2p_rejection = HashToPointRejectionClaims {
        terminal: [
            unsigned(17, field),
            unsigned(19, field),
            unsigned(23, field),
        ],
        point: point(REJECTION_STRIDE.ilog2() as usize + batch_vars, field),
    };
    claims
}

fn selection_fixture(batch: usize) -> Selection {
    let masks = (0..batch)
        .map(|s| {
            let mut mask = vec![0u8; HASH_TO_POINT_SAMPLES.div_ceil(8)];
            let start = (17 * s) % (HASH_TO_POINT_SAMPLES - N + 1);
            for j in start..start + N {
                mask[j / 8] |= 1 << (j % 8);
            }
            mask
        })
        .collect();
    Selection::from_masks(masks, batch).unwrap()
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
fn streaming_binding_forms_match_independent_bit_oracle() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 2]);
    let claims = complete_claim_fixture(&layout, &field);
    let linear_point = point(linear_rounds(&layout), &field);
    let selection = selection_fixture(layout.batch());
    let binding = prepare_binding_form(
        &mut Blake3Transcript::new(),
        &layout,
        &statement,
        &selection,
        claims.as_claim_ref(),
        &linear_point,
        &field,
    )
    .unwrap();
    let mut expected = DenseSink::new(layout.source_bits(), &field);
    let expected_target = emit_binding_reference(&binding, &mut expected);
    assert_eq!(binding.target().unwrap(), expected_target);
    let mut actual = vec![field.zero(); layout.source_bits()];
    binding
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
            binding
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
        binding
            .for_each_partition_block(0, 32, &mut |_, _| Ok(()))
            .unwrap()
            .is_err()
    );
    let weights = eq_table(&point(3, &field), &field).unwrap();
    let mut folded = vec![field.zero(); layout.source_bits() / weights.len()];
    for s in 0..layout.capacity() {
        let mut next = s * layout.signature_stride() / weights.len();
        binding
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
        binding
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
        binding.evaluate(&endpoint).unwrap(),
        evaluate_mle_in_place(&mut expected.values, &endpoint, &field).unwrap()
    );
}

#[test]
fn five_factored_local_forms_match_dense_padded_batch_without_instance_caches() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 2]);
    let proof = complete_claim_fixture(&layout, &field);
    for eta in [field.zero(), field.one(), unsigned(19, &field)] {
        let selection = selection_fixture(layout.batch());
        let mut binding = prepare_binding_form(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            &selection,
            proof.as_claim_ref(),
            &point(linear_rounds(&layout), &field),
            &field,
        )
        .unwrap();
        binding.eta = eta;
        let mut dense = DenseSink::new(layout.source_bits(), &field);
        assert_eq!(
            binding.target().unwrap(),
            emit_binding_reference(&binding, &mut dense)
        );
        let endpoint = point(source_rounds(&layout), &field);
        let actual = binding.evaluate(&endpoint).unwrap();
        assert!(
            binding
                .compact_instances
                .iter()
                .all(|cache| cache.get().is_none())
        );
        assert_eq!(
            actual,
            evaluate_mle_in_place(&mut dense.values, &endpoint, &field).unwrap()
        );
        let mut inactive = endpoint;
        inactive[layout.signature_stride().ilog2() as usize..].fill(field.one());
        assert_eq!(binding.evaluate(&inactive).unwrap(), field.zero());
        assert!(
            binding
                .compact_instances
                .iter()
                .all(|cache| cache.get().is_none())
        );
    }
}

#[test]
fn factored_overlay_matches_dense_sumcheck_and_endpoint() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 2]);
    let claims = complete_claim_fixture(&layout, &field);
    let selection = selection_fixture(layout.batch());
    let binding = prepare_binding_form(
        &mut Blake3Transcript::new(),
        &layout,
        &statement,
        &selection,
        claims.as_claim_ref(),
        &point(linear_rounds(&layout), &field),
        &field,
    )
    .unwrap();
    let mut integer = DenseSink::new(layout.source_bits(), &field);
    let target = emit_binding_reference(&binding, &mut integer);
    assert_eq!(binding.target().unwrap(), target);
    let row: Vec<_> = (0..layout.capacity())
        .map(|s| {
            if s < layout.batch() {
                unsigned((7 + 13 * s) as u128, &field)
            } else {
                field.zero()
            }
        })
        .collect();
    let column: Vec<_> = (0..layout.signature_stride())
        .map(|i| {
            if !layout.is_padding(i) && i % 5 != 0 {
                unsigned(17 + (i as u128).pow(2), &field)
            } else {
                field.zero()
            }
        })
        .collect();
    let mut words = vec![0u64; layout.source_bits() / 64];
    for s in 0..layout.batch() {
        for i in (0..layout.signature_stride()).filter(|&i| !layout.is_padding(i)) {
            let index = s * layout.signature_stride() + i;
            let bit = ((index as u64 * 0x9e37_79b9 ^ (index as u64 >> 3)).count_ones() & 1) as u64;
            words[index / 64] |= bit << (index % 64);
        }
    }
    for scale in [
        field.zero(),
        field.one(),
        field.neg(&field.one()),
        unsigned(29, &field),
    ] {
        let dense: Vec<_> = integer
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                field.add(
                    &field.mul(&scale, value),
                    &field.mul(&row[index / column.len()], &column[index % column.len()]),
                )
            })
            .collect();
        let claim = dense
            .iter()
            .enumerate()
            .fold(field.zero(), |sum, (index, value)| {
                if words[index / 64] >> (index % 64) & 1 != 0 {
                    field.add(&sum, value)
                } else {
                    sum
                }
            });
        let transcript = || {
            let mut t = Blake3Transcript::new();
            t.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
            t
        };
        let mut reference_transcript = transcript();
        let coefficients = |index: usize| Ok(dense[index]);
        let expected = prove_inner_sumcheck(
            &field,
            &mut reference_transcript,
            claim,
            PackedInput::new(
                &coefficients,
                &words,
                source_rounds(&layout),
                layout.source_bits(),
                4,
            ),
            (),
            &mut crate::sumcheck::UngrindedRoundBoundary,
        )
        .unwrap();
        let challenge = reference_transcript.get_challenge::<u128>();
        for prefix in [3, 4] {
            let mut actual_transcript = transcript();
            let actual = prove_inner_sumcheck(
                &field,
                &mut actual_transcript,
                claim,
                FactoredOverlayInput::new(
                    &binding,
                    &words,
                    &row,
                    &column,
                    scale,
                    source_rounds(&layout),
                    layout.source_bits(),
                    prefix,
                ),
                (),
                &mut crate::sumcheck::UngrindedRoundBoundary,
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual_transcript.get_challenge::<u128>(), challenge);
            let split = layout.signature_stride().ilog2() as usize;
            let ring = field.mul(
                &evaluate_mle_in_place(&mut column.clone(), &actual.point[..split], &field)
                    .unwrap(),
                &evaluate_mle_in_place(&mut row.clone(), &actual.point[split..], &field).unwrap(),
            );
            assert_eq!(
                actual.terminal_evaluations[0],
                field.add(
                    &ring,
                    &field.mul(&scale, &binding.evaluate(&actual.point).unwrap()),
                )
            );
            assert_eq!(
                actual.terminal_evaluations[0],
                evaluate_mle_in_place(&mut dense.clone(), &actual.point, &field,).unwrap()
            );
        }
    }
}

#[test]
fn binding_matches_dense_reference_at_degenerate_points() {
    let field = config();
    for groups in [&[0, 0, 0][..], &[0, 1, 0, 2, 1], &[0, 1, 2]] {
        let layout = FalconSourceLayout::new(groups.len()).unwrap();
        let statement = statement_with_key_groups(groups);
        let proof = complete_claim_fixture(&layout, &field);
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
            let selection = selection_fixture(layout.batch());
            let binding = prepare_binding_form(
                &mut transcript,
                &layout,
                &statement,
                &selection,
                proof.as_claim_ref(),
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
        let mut proof = complete_claim_fixture(&layout, &field);
        for (bit, coordinate) in proof.norm.point.iter_mut().enumerate() {
            *coordinate = unsigned(((selected * N + 17) >> bit & 1) as u128, &field);
        }
        for (bit, coordinate) in proof.norm.instance_point.iter_mut().enumerate() {
            *coordinate = unsigned(((selected >> bit) & 1) as u128, &field);
        }
        for row in [
            0,
            HASH_TO_POINT_SAMPLES - 1,
            HASH_TO_POINT_SAMPLES,
            REJECTION_ROWS - 1,
            REJECTION_ROWS,
            REJECTION_STRIDE - 1,
        ] {
            for (bit, coordinate) in proof.h2p_rejection.point.iter_mut().enumerate() {
                *coordinate = unsigned(
                    (((selected * REJECTION_STRIDE + row) >> bit) & 1) as u128,
                    &field,
                );
            }
            for eta in [field.zero(), field.one(), unsigned(29, &field)] {
                let linear_point = point(linear_rounds(&layout), &field);
                let mut transcript = Blake3Transcript::new();
                let selection = selection_fixture(layout.batch());
                let mut binding = prepare_binding_form(
                    &mut transcript,
                    &layout,
                    &statement,
                    &selection,
                    proof.as_claim_ref(),
                    &linear_point,
                    &field,
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
fn binding_preserves_inner_sumcheck_transcript() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let statement = statement_with_key_groups(&[0, 1, 0]);
    let proof = complete_claim_fixture(&layout, &field);
    let linear_point = point(linear_rounds(&layout), &field);
    let mut transcript = Blake3Transcript::new();
    let selection = selection_fixture(layout.batch());
    let binding = prepare_binding_form(
        &mut transcript,
        &layout,
        &statement,
        &selection,
        proof.as_claim_ref(),
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
    let proof = complete_claim_fixture(&layout, &field);

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
            let constant = add_linear_constraints(
                &mut sink,
                &weights,
                &layout,
                public,
                &selection_fixture(layout.batch()),
                &field,
            )
            .unwrap();
            assert_eq!(field.add(&sink.value, &constant), expected);

            let mut transcript = Blake3Transcript::new();
            let selection = selection_fixture(layout.batch());
            let mut binding = prepare_binding_form(
                &mut transcript,
                &layout,
                public,
                &selection,
                proof.as_claim_ref(),
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

#[test]
fn shared_projection_authenticates_fixed_committed_operands() {
    let layout = FalconSourceLayout::new(3).unwrap();
    let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
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
    let mut prover = Blake3Transcript::new();
    for row in source.rows() {
        for word in row {
            prover.absorb_slice(&word.to_le_bytes());
        }
    }
    let mut verifier = prover.clone();
    let (proof, field, claim) =
        super::super::shared_ring::prove(&mut prover, &layout, &statement, &traces, &source, 100)
            .unwrap();
    let (verified_field, verified) =
        super::super::shared_ring::verify(&mut verifier, &layout, &statement, &proof, 100).unwrap();
    assert_eq!(field.modulus_u128(), verified_field.modulus_u128());
    assert_eq!(claim.target, verified.target);
    assert_eq!(
        prover.get_challenge::<u128>(),
        verifier.get_challenge::<u128>()
    );

    let dense: Vec<_> = (0..layout.source_bits())
        .map(|index| {
            field.mul(
                &claim.row[index / layout.signature_stride()],
                &claim.column[index % layout.signature_stride()],
            )
        })
        .collect();
    let dot = |rows: &[Vec<u64>]| {
        let mut result = field.zero();
        for (column, words) in rows.iter().enumerate() {
            for (word_index, &word) in words.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let index = (column << layout.row_vars())
                        + 64 * word_index
                        + bits.trailing_zeros() as usize;
                    result = field.add(&result, &dense[index]);
                    bits &= bits - 1;
                }
            }
        }
        result
    };
    assert_eq!(dot_packed_source(&dense, &source, &field), claim.target);
    assert!(
        dense[layout.batch() * layout.signature_stride()..]
            .iter()
            .all(|v| *v == field.zero())
    );
    let endpoint = point(source_rounds(&layout), &field);
    let split = layout.signature_stride().ilog2() as usize;
    let local = eq_table(&endpoint[..split], &field).unwrap();
    let instances = eq_table(&endpoint[split..], &field).unwrap();
    assert_eq!(
        verified
            .evaluate(&layout, &local, &instances, &field)
            .unwrap(),
        evaluate_mle_in_place(&mut dense.clone(), &endpoint, &field).unwrap()
    );

    // Change packed C and S1 bits independently, after fixing both the source
    // commitment and projection query. Do not regenerate any proof or claim.
    for index in [
        layout.signature_stride() + layout.hash_point_bit(17, 0),
        2 * layout.signature_stride() + layout.s1_bit(31, 13),
    ] {
        assert_ne!(dense[index], field.zero());
        let mut changed = source.rows().to_vec();
        let column = index >> layout.row_vars();
        let row = index & ((1 << layout.row_vars()) - 1);
        changed[column][row / 64] ^= 1 << (row % 64);
        assert_ne!(dot(&changed), claim.target);
    }
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
    let mut prover = fresh_transcript();
    let (proof, row_weights, bridge_claim) =
        prove_binding_prefix(&mut prover, &layout, &statement, &traces, &source, 100).unwrap();
    assert_eq!(bridge_claim.point.len(), source_rounds(&layout));
    let mut verifier = fresh_transcript();
    assert_eq!(
        verify_binding_prefix(&mut verifier, &layout, &statement, &proof, 100).unwrap(),
        bridge_claim,
    );
    assert_eq!(
        prover.get_challenge::<u128>(),
        verifier.get_challenge::<u128>()
    );
    let field = field_from_modulus(bridge_claim.modulus).unwrap();
    assert_eq!(
        row_weights,
        canonical_eq_weights(&bridge_claim.point[..layout.row_vars()], &field).unwrap(),
    );
    let col_weights =
        canonical_eq_weights(&bridge_claim.point[layout.row_vars()..], &field).unwrap();
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
    // Shape errors are rejected before the public mask enters the transcript.
    for case in 0..4 {
        let mut bad = proof.clone();
        match case {
            0 => {
                bad.selection_masks.pop();
            }
            1 => {
                bad.selection_masks[0].pop();
            }
            2 => {
                *bad.selection_masks[0].last_mut().unwrap() |= 1 << (HASH_TO_POINT_SAMPLES % 8);
            }
            _ => {
                let selected =
                    Selection::from_masks(bad.selection_masks.clone(), layout.batch()).unwrap();
                let index = usize::from(selected.indices(0)[0]);
                bad.selection_masks[0][index / 8] ^= 1 << (index % 8);
            }
        }
        let mut verifier = fresh_transcript();
        assert!(verify_binding_prefix(&mut verifier, &layout, &statement, &bad, 100).is_err());
        assert_eq!(
            verifier.get_challenge::<u128>(),
            fresh_transcript().get_challenge::<u128>()
        );
    }
    // A canonical mutation keeps the selected count but changes transcript-bound routing.
    let mut bad = proof.clone();
    let selection = Selection::from_masks(bad.selection_masks.clone(), layout.batch()).unwrap();
    let removed = usize::from(selection.indices(0)[0]);
    let added = (0..HASH_TO_POINT_SAMPLES)
        .find(|&j| !selection.selected(0, j))
        .unwrap();
    for j in [removed, added] {
        bad.selection_masks[0][j / 8] ^= 1 << (j % 8);
    }
    assert!(Selection::from_masks(bad.selection_masks.clone(), layout.batch()).is_ok());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
    let mut bad = proof.clone();
    bad.piop.terminal[0] = field.add(&bad.piop.terminal[0], &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
    let mut bad = proof;
    bad.piop.terminal[3] =
        field.add(&bad.piop.terminal[3], &field.one());
    assert!(
        verify_binding_prefix(&mut fresh_transcript(), &layout, &statement, &bad, 100).is_err()
    );
}

#[test]
fn public_selection_enforces_first_accepted_and_distinct_remainder_decoders() {
    let layout = FalconSourceLayout::new(3).unwrap();
    let field = config();
    let traces = vec![verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap(); 3];
    let source = FalconSourceWitness::from_traces(
        layout,
        &[MESSAGE.as_slice(); 3],
        &[SIGNATURE.as_slice(); 3],
        &traces,
    )
    .unwrap();
    let statement = FalconPublicStatement::from_bytes(
        &[PUBLIC_KEY.as_slice(); 3],
        &[MESSAGE.as_slice(); 3],
        &[SIGNATURE.as_slice(); 3],
    )
    .unwrap();
    let selection = Selection::from_traces(&traces).unwrap();
    let claims = complete_claim_fixture(&layout, &field);
    struct SourceDot<'a> {
        source: &'a FalconSourceWitness,
        field: &'a Cfg,
        flipped: Option<usize>,
        value: F,
    }
    impl CoefficientSink for SourceDot<'_> {
        fn add(&mut self, index: usize, coefficient: F) {
            if self.source.bit(index) ^ (self.flipped == Some(index)) {
                self.value = self.field.add(&self.value, &coefficient);
            }
        }
    }
    let residual = |selection: &Selection, row: usize, flipped: Option<usize>| {
        let linear_point: Vec<_> = (0..linear_rounds(&layout))
            .map(|bit| unsigned(((row >> bit) & 1) as u128, &field))
            .collect();
        let weights = factored_weights(&linear_point, &field).unwrap();
        let mut dot = SourceDot {
            source: &source,
            field: &field,
            flipped,
            value: field.zero(),
        };
        let constant =
            add_linear_constraints(&mut dot, &weights, &layout, &statement, selection, &field)
                .unwrap();
        let expected = field.add(&dot.value, &constant);
        let mut binding = prepare_binding_form(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            selection,
            claims.as_claim_ref(),
            &linear_point,
            &field,
        )
        .unwrap();
        binding.eta = field.zero();
        dot.value = field.zero();
        binding
            .for_each_coefficient(&mut |index, value| {
                dot.add(index, value);
                Ok(())
            })
            .unwrap();
        assert_eq!(field.sub(&dot.value, &binding.target().unwrap()), expected);
        expected
    };
    let first = usize::from(selection.indices(0)[0]);
    let mask_row = 1 + 256 + super::super::CT_SIGNATURE_BYTES + HASH_TO_POINT_SAMPLES + first;
    assert_eq!(residual(&selection, mask_row, None), field.zero());
    let mut masks = selection.masks().to_vec();
    let added = (0..HASH_TO_POINT_SAMPLES)
        .find(|&j| !selection.selected(0, j))
        .unwrap();
    for j in [first, added] {
        masks[0][j / 8] ^= 1 << (j % 8);
    }
    let changed = Selection::from_masks(masks, layout.batch()).unwrap();
    // Skipping the first accepted candidate fails even with the row challenge fixed.
    assert_eq!(residual(&changed, mask_row, None), field.neg(&field.one()));

    let output_row = 1 + 256 + super::super::CT_SIGNATURE_BYTES + 2 * HASH_TO_POINT_SAMPLES;
    assert_eq!(residual(&selection, output_row, None), field.zero());
    for (index, coefficient) in [
        (layout.hash_point_bit(0, 13), 8192i128),
        (layout.offsets().hash_remainders + 14 * first + 13, -4097),
    ] {
        let delta = if source.bit(index) {
            -coefficient
        } else {
            coefficient
        };
        assert_eq!(
            residual(&selection, output_row, Some(index)),
            signed(delta, &field)
        );
    }
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
    proof.norm.point = point(COEFFICIENT_LOG + 3, &field);
    let instance_weights = eq_table(&proof.norm.instance_point, &field).unwrap();
    let weights = eq_table(&proof.norm.point, &field).unwrap();
    for (instance, trace) in traces.iter().enumerate() {
        for i in 0..N {
            let word = signed(trace.s1[i] as i128, &field);
            let weighted = field.mul(&weights[instance * 2 * N + i], &word);
            proof.norm.terminal = field.add(&proof.norm.terminal, &weighted);
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
    let evaluate = |proof: &FalconPiopClaims| {
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
            proof.as_claim_ref(),
            &field,
        )
        .unwrap();
        (sink.value, target)
    };
    let (actual, target) = evaluate(&proof);
    assert_eq!(actual, target);
    let mut bad = proof.clone();
    bad.norm.terminal = field.add(&bad.norm.terminal, &field.one());
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
fn rejection_transpose_authenticates_raw_bits_and_each_terminal() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let mut proof = complete_claim_fixture(&layout, &field);
    let weights = eq_table(&proof.h2p_rejection.point, &field).unwrap();
    proof.h2p_rejection.terminal.fill(field.zero());
    let raw_bit = |index: usize| (index.wrapping_mul(37) ^ (index >> 3)).count_ones() % 2 != 0;
    let offsets = layout.offsets();
    for instance in 0..layout.batch() {
        for j in 0..HASH_TO_POINT_SAMPLES {
            for (coordinate, address) in [
                offsets.hash_quotients + 3 * j + 2,
                offsets.hash_quotients + 3 * j,
                offsets.hash_accept_ands + j,
            ]
            .into_iter()
            .enumerate()
            {
                if raw_bit(instance * layout.signature_stride() + address) {
                    proof.h2p_rejection.terminal[coordinate] = field.add(
                        &proof.h2p_rejection.terminal[coordinate],
                        &weights[instance * REJECTION_STRIDE + j],
                    );
                }
            }
        }
    }
    let eta = unsigned(19, &field);
    let prepared =
        rejection::RejectionWeights::new(&layout, proof.as_claim_ref(), eta, &field).unwrap();
    let mut dense = DenseSink::new(layout.source_bits(), &field);
    prepared.emit(&mut dense, &layout, &field);
    let dot = dense
        .values
        .iter()
        .enumerate()
        .fold(field.zero(), |sum, (index, value)| {
            if raw_bit(index) {
                field.add(&sum, value)
            } else {
                sum
            }
        });
    assert_eq!(dot, prepared.target());
    for coordinate in 0..3 {
        let mut bad = proof.clone();
        bad.h2p_rejection.terminal[coordinate] =
            field.add(&bad.h2p_rejection.terminal[coordinate], &field.one());
        assert_ne!(
            dot,
            rejection::RejectionWeights::new(&layout, bad.as_claim_ref(), eta, &field)
                .unwrap()
                .target()
        );
    }
}

#[test]
fn rejection_weights_require_exact_row_point_dimensions() {
    let field = config();
    let layout = FalconSourceLayout::new(3).unwrap();
    let proof = complete_claim_fixture(&layout, &field);
    let eta = unsigned(19, &field);
    assert!(rejection::RejectionWeights::new(&layout, proof.as_claim_ref(), eta, &field).is_ok());
    for mutation in 0..3 {
        let mut bad = proof.clone();
        match mutation {
            0 => {
                bad.h2p_rejection.point.pop();
            }
            1 => {
                bad.h2p_rejection.point.push(field.zero());
            }
            2 => {
                bad.h2p_rejection.point.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            rejection::RejectionWeights::new(&layout, bad.as_claim_ref(), eta, &field).is_err()
        );
    }
}
