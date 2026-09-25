//! Commitment-bound terminal reduction for the Falcon-1024 PIOP.
//!
//! All exact linear Falcon relations and all terminal values emitted by the
//! nonlinear sumchecks are folded into one dense linear form in the committed
//! binary source.  One degree-two inner sumcheck reduces that form to one MLE
//! evaluation, which is authenticated by the ordinary runtime-prime BitZ /
//! Ligerito opening.

use field::{RingOps, Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig, VerifierConfig},
};

use crate::{
    ligerito_flock::{
        FlockCommitHint, IntEvalRsLigModQProof, prove_mle_eval_mod_q_ligerito,
        validate_ligerito_commitment, verify_mle_eval_mod_q_ligerito_runtime,
    },
    pcs::smallest_generator,
    piop::spartan::{
        SpartanBitzField, SpartanField, absorb_field_elements,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::eq_table,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        inner::{
            packed::{ColumnMajorPackedBits, PackedInput},
            prove_inner_sumcheck,
        },
    },
    transcript::traits::Transcript,
};

use super::{
    FalconError, FalconPiopProof, FalconPublicKey, FalconSourceLayout, FalconSourceOffsets,
    FalconSourceWitness, FalconVerificationTrace, HASH_TO_POINT_SAMPLES, N, Q, decode_public_key,
    piop::{
        BINDING_GRINDING_BITS, LINEAR_POINT_GRINDING_BITS, prove_falcon_piop, verify_falcon_piop,
    },
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

const LINEAR_STRIDE: usize = 1 << 20;
const KECCAK_GATE_STRIDE: usize = 1 << 20;
const COMPACTION_LEAVES: usize = 1 << 11;
const KECCAK_PERMUTATIONS: usize = 20;
const KECCAK_ROUNDS: usize = 24;
const KECCAK_LANES: usize = 25;
const RATE_BYTES: usize = 136;

const ROTATION: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

struct LinearPointGrinding;
impl GrindingDomain for LinearPointGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/linear-point/v1";
}

struct BindingGrinding;
impl GrindingDomain for BindingGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/binding/v1";
}

/// Public inputs of a batch.  Signatures remain inside the committed witness;
/// the public key and the 32-byte message are bound before any PIOP challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconPublicStatement {
    pub public_keys: Vec<FalconPublicKey>,
    pub messages: Vec<[u8; 32]>,
}

impl FalconPublicStatement {
    pub fn from_bytes(public_keys: &[&[u8]], messages: &[&[u8]]) -> Result<Self, FalconError> {
        if public_keys.len() != messages.len() || public_keys.is_empty() {
            return Err(piop("public statement batch mismatch"));
        }
        let public_keys = public_keys
            .iter()
            .map(|bytes| decode_public_key(bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let messages = messages
            .iter()
            .map(|message| {
                (*message)
                    .try_into()
                    .map_err(|_| piop("Falcon benchmark messages must contain 32 bytes"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            public_keys,
            messages,
        })
    }

    pub fn batch(&self) -> usize {
        self.public_keys.len()
    }
}

/// One commitment-bound Falcon proof: nonlinear PIOP, shared terminal
/// reduction, and the final BitZ source opening.
pub struct FalconBitzProof {
    pub piop: FalconPiopProof,
    pub linear_point_nonce: Option<u64>,
    pub binding: SumcheckProof<F, 3>,
    pub binding_point: Vec<F>,
    /// `[coefficient MLE, source MLE]` at `binding_point`.
    pub binding_terminal: [F; 2],
    pub binding_nonces: Vec<u64>,
    pub opening: IntEvalRsLigModQProof,
}

/// Prove a batch against one already-created binary source commitment.
#[allow(clippy::too_many_arguments)]
pub fn prove_falcon_bitz(
    transcript: &mut (impl Transcript + Send),
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    hint: &FlockCommitHint,
    target_bits: usize,
    pc: &ProverConfig,
) -> Result<FalconBitzProof, FalconError> {
    validate_prover_inputs(layout, statement, traces, source, hint, target_bits, pc)?;
    bind_statement(transcript, layout, statement, &hint.commitment, target_bits)?;
    let algebraic = prove_falcon_piop(transcript, layout, traces, target_bits)?;
    let field = field_from_modulus(algebraic.modulus)?;

    let linear_point_nonce = grind_linear_point(transcript, target_bits, None)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let (coefficients, target) = build_binding_form(
        transcript,
        layout,
        statement,
        &algebraic,
        &linear_point,
        &field,
    )?;
    if dot_packed_source(&coefficients, source, &field) != target {
        return Err(piop(
            "committed witness does not satisfy the collapsed Falcon relation",
        ));
    }

    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let packed_source = ColumnMajorPackedBits::new(source.rows(), layout.row_vars());
    let coefficient_source = |index: usize| {
        coefficients
            .get(index)
            .copied()
            .ok_or(crate::sumcheck::SumcheckError::InvalidProductDimensions)
    };
    let input = || {
        PackedInput::new(
            &coefficient_source,
            &packed_source,
            source_rounds(layout),
            coefficients.len(),
            4,
        )
    };
    let output = if target_bits == 128 {
        let mut boundary = ProverGrindingRoundBoundary::<BindingGrinding>::with_round_offset(
            BINDING_GRINDING_BITS,
            0,
        );
        let output = prove_inner_sumcheck(&field, transcript, target, input(), (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        (output, boundary.into_nonces())
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        let output = prove_inner_sumcheck(&field, transcript, target, input(), (), &mut boundary)
            .map_err(|error| piop(error.to_string()))?;
        (output, Vec::new())
    };
    let (output, binding_nonces) = output;
    let binding_terminal = output.terminal_evaluations;
    absorb_field_elements(transcript, &binding_terminal, &field);
    bind_opening_claim(
        transcript,
        algebraic.modulus,
        &output.point,
        binding_terminal[1],
        &field,
    );
    let row_weights = canonical_eq_weights(&output.point[..layout.row_vars()], &field)?;
    let opening = prove_mle_eval_mod_q_ligerito(
        transcript,
        hint,
        &layout.bitz_params(),
        &row_weights,
        modulus_bits(algebraic.modulus),
        smallest_generator(),
        pc,
    );

    Ok(FalconBitzProof {
        piop: algebraic,
        linear_point_nonce,
        binding: output.proof,
        binding_point: output.point,
        binding_terminal,
        binding_nonces,
        opening,
    })
}

/// Verify the nonlinear PIOP, rebuild its public linear functional, reduce it
/// to one source MLE value, and authenticate that value against `commitment`.
#[allow(clippy::too_many_arguments)]
pub fn verify_falcon_bitz(
    transcript: &mut (impl Transcript + Send),
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    proof: &FalconBitzProof,
    target_bits: usize,
    vc: &VerifierConfig,
) -> Result<(), FalconError> {
    validate_verifier_inputs(layout, statement, commitment, target_bits, vc)?;
    bind_statement(transcript, layout, statement, commitment, target_bits)?;
    verify_falcon_piop(transcript, layout, &proof.piop, target_bits)?;
    let field = field_from_modulus(proof.piop.modulus)?;

    grind_linear_point(transcript, target_bits, proof.linear_point_nonce)?;
    let linear_point = sample_point(transcript, linear_rounds(layout), &field)?;
    let (mut coefficients, target) = build_binding_form(
        transcript,
        layout,
        statement,
        &proof.piop,
        &linear_point,
        &field,
    )?;
    transcript.absorb_slice(b"bitz/falcon1024-ct/shared-inner/v1");
    let (point, final_claims) = if target_bits == 128 {
        let mut boundary = VerifierGrindingRoundBoundary::<BindingGrinding>::new(
            BINDING_GRINDING_BITS,
            &proof.binding_nonces,
        );
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.binding],
            transcript,
            &[target],
            source_rounds(layout),
            &field,
            &mut boundary,
        )
    } else {
        let mut boundary = crate::sumcheck::UngrindedRoundBoundary;
        SumcheckProof::verify_batch_with_round_boundary(
            [&proof.binding],
            transcript,
            &[target],
            source_rounds(layout),
            &field,
            &mut boundary,
        )
    }
    .map_err(|error| piop(error.to_string()))?;
    if point != proof.binding_point
        || final_claims[0] != field.mul(&proof.binding_terminal[0], &proof.binding_terminal[1])
    {
        return Err(piop("shared inner terminal mismatch"));
    }
    if evaluate_mle_in_place(&mut coefficients, &point, &field)? != proof.binding_terminal[0] {
        return Err(piop("shared coefficient MLE mismatch"));
    }
    absorb_field_elements(transcript, &proof.binding_terminal, &field);
    bind_opening_claim(
        transcript,
        proof.piop.modulus,
        &point,
        proof.binding_terminal[1],
        &field,
    );
    let row_weights = canonical_eq_weights(&point[..layout.row_vars()], &field)?;
    let col_weights = canonical_eq_weights(&point[layout.row_vars()..], &field)?;
    verify_mle_eval_mod_q_ligerito_runtime(
        transcript,
        commitment,
        &proof.opening,
        &layout.bitz_params(),
        &row_weights,
        &col_weights,
        smallest_generator(),
        canonical(proof.binding_terminal[1], &field),
        proof.piop.modulus,
        modulus_bits(proof.piop.modulus),
        None,
        vc,
    )
    .map_err(|error| piop(format!("{error:?}")))
}

fn validate_prover_inputs(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    hint: &FlockCommitHint,
    target_bits: usize,
    pc: &ProverConfig,
) -> Result<(), FalconError> {
    if statement.batch() != layout.batch()
        || traces.len() != layout.batch()
        || source.layout() != layout
        || !matches!(target_bits, 100 | 128)
    {
        return Err(piop("Falcon prover input shape mismatch"));
    }
    for (trace, public_key) in traces.iter().zip(&statement.public_keys) {
        if &trace.public_key != public_key {
            return Err(piop("trace does not match the public Falcon statement"));
        }
    }
    if source.rows() != hint.rows() {
        return Err(piop("source witness does not match the commitment hint"));
    }
    validate_ligerito_commitment(&hint.commitment, pc)
        .map_err(|error| piop(format!("commitment: {error:?}")))
}

fn validate_verifier_inputs(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    target_bits: usize,
    vc: &VerifierConfig,
) -> Result<(), FalconError> {
    if statement.batch() != layout.batch() || !matches!(target_bits, 100 | 128) {
        return Err(piop("Falcon verifier input shape mismatch"));
    }
    validate_ligerito_commitment(commitment, vc)
        .map_err(|error| piop(format!("commitment: {error:?}")))
}

fn bind_statement(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    commitment: &Commitment,
    target_bits: usize,
) -> Result<(), FalconError> {
    transcript.absorb_slice(b"bitz/falcon1024-ct/commitment-bound/v1");
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&(target_bits as u64).to_le_bytes());
    transcript
        .absorb_slice(&bincode::serialize(commitment).map_err(|error| piop(error.to_string()))?);
    for (public_key, message) in statement.public_keys.iter().zip(&statement.messages) {
        for coefficient in public_key.h.iter() {
            transcript.absorb_inner(&coefficient.to_le_bytes());
        }
        transcript.absorb_inner(message);
    }
    Ok(())
}

fn grind_linear_point(
    transcript: &mut impl Transcript,
    target_bits: usize,
    nonce: Option<u64>,
) -> Result<Option<u64>, FalconError> {
    if target_bits == 100 {
        return if nonce.is_none() {
            Ok(None)
        } else {
            Err(piop("unexpected linear-point grinding nonce"))
        };
    }
    match nonce {
        None => grind_and_absorb(
            transcript,
            GrindingRound::<LinearPointGrinding>::new(0),
            LINEAR_POINT_GRINDING_BITS,
        )
        .map(Some)
        .map_err(|error| piop(error.to_string())),
        Some(nonce) => verify_and_absorb(
            transcript,
            GrindingRound::<LinearPointGrinding>::new(0),
            LINEAR_POINT_GRINDING_BITS,
            nonce,
        )
        .map(|()| Some(nonce))
        .map_err(|error| piop(error.to_string())),
    }
}

fn build_binding_form(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    proof: &FalconPiopProof,
    linear_point: &[F],
    field: &Cfg,
) -> Result<(Vec<F>, F), FalconError> {
    let mut coefficients = vec![field.zero(); layout.source_bits()];
    let linear_weights = eq_table(linear_point, field).map_err(|error| piop(error.to_string()))?;
    let linear_constant =
        add_linear_constraints(&mut coefficients, &linear_weights, layout, statement, field)?;
    drop(linear_weights);

    transcript.absorb_slice(b"bitz/falcon1024-ct/terminal-collapse/v1");
    let eta = squeeze(transcript, field)?;
    let mut scale = field.one();
    let mut target = field.sub(&field.zero(), &linear_constant);
    scale = field.mul(&scale, &eta);

    add_norm_claims(
        &mut coefficients,
        &mut target,
        &mut scale,
        eta,
        layout,
        proof,
        field,
    )?;
    add_keccak_claims(
        &mut coefficients,
        &mut target,
        &mut scale,
        eta,
        layout,
        proof,
        field,
    )?;
    add_compaction_product_claims(
        &mut coefficients,
        &mut target,
        &mut scale,
        eta,
        layout,
        proof,
        field,
    )?;
    add_product_tree_claims(
        &mut coefficients,
        &mut target,
        &mut scale,
        eta,
        layout,
        proof,
        field,
    )?;
    Ok((coefficients, target))
}

fn add_linear_constraints(
    coefficients: &mut [F],
    weights: &[F],
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    field: &Cfg,
) -> Result<F, FalconError> {
    let offsets = FalconSourceOffsets::new();
    let mut constant = field.zero();
    for instance in 0..layout.batch() {
        let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
        let mut row = 0usize;
        let mut residual = |terms: &[(usize, i128)], c: i128| {
            let weight = weights[instance * LINEAR_STRIDE + row];
            row += 1;
            for &(index, coefficient) in terms {
                add_coefficient(
                    coefficients,
                    base + index,
                    mul_i(weight, coefficient, field),
                    field,
                );
            }
            constant = field.add(&constant, &mul_i(weight, c, field));
        };

        residual(&[(offsets.shared_one, 1)], -1);
        for bit in 0..256 {
            let expected = (statement.messages[instance][bit / 8] >> (bit % 8)) & 1;
            residual(&[(offsets.message + bit, 1)], -i128::from(expected));
        }
        for bit in 0..8 {
            residual(
                &[(offsets.encoded_signature + bit, 1)],
                -i128::from((0x5au8 >> bit) & 1),
            );
        }

        for i in 0..N {
            let mut terms = Vec::with_capacity(16);
            for bit in 0..11 {
                terms.push((s2_bit_index(&offsets, i, bit), 1));
            }
            terms.push((s2_bit_index(&offsets, i, 11), -1));
            for bit in 0..4 {
                terms.push((offsets.s2_non_min_slack + 4 * i + bit, -(1i128 << bit)));
            }
            residual(&terms, 0);
        }

        for permutation in 0..KECCAK_PERMUTATIONS {
            for round in 0..KECCAK_ROUNDS {
                for y in 0..5 {
                    for x in 0..5 {
                        let (source_x, source_y) = rho_pi_source(x, y);
                        let rotation = ROTATION[source_x][source_y] as usize;
                        for destination_bit in 0..64 {
                            let source_bit = (destination_bit + 64 - rotation) & 63;
                            let rotated_bit = (source_bit + 63) & 63;
                            let mut terms = Vec::with_capacity(15);
                            let mut c = 0i128;
                            push_round_input(
                                &mut terms,
                                &mut c,
                                &offsets,
                                permutation,
                                round,
                                source_x,
                                source_y,
                                source_bit,
                                1,
                            );
                            for source_row in 0..5 {
                                push_round_input(
                                    &mut terms,
                                    &mut c,
                                    &offsets,
                                    permutation,
                                    round,
                                    (source_x + 4) % 5,
                                    source_row,
                                    source_bit,
                                    1,
                                );
                                push_round_input(
                                    &mut terms,
                                    &mut c,
                                    &offsets,
                                    permutation,
                                    round,
                                    (source_x + 1) % 5,
                                    source_row,
                                    rotated_bit,
                                    1,
                                );
                            }
                            let gate = keccak_gate(permutation, round, x, y, destination_bit);
                            terms.push((offsets.keccak_chi_inputs + gate, -1));
                            for bit in 0..3 {
                                terms.push((
                                    offsets.keccak_parity_quotients + 3 * gate + bit,
                                    -(2i128 << bit),
                                ));
                            }
                            residual(&terms, c);
                        }
                    }
                }
            }
        }

        for i in 0..HASH_TO_POINT_SAMPLES {
            let mut division = Vec::with_capacity(34);
            for bit in 0..16 {
                division.push((shake_word_bit_index(&offsets, i, bit), 1i128 << bit));
            }
            push_unsigned_terms(
                &mut division,
                offsets.hash_quotients + 3 * i,
                3,
                -i128::from(Q),
            );
            push_unsigned_terms(&mut division, offsets.hash_remainders + 14 * i, 14, -1);
            residual(&division, 0);

            let mut range = Vec::with_capacity(28);
            push_unsigned_terms(&mut range, offsets.hash_remainders + 14 * i, 14, 1);
            push_unsigned_terms(&mut range, offsets.hash_remainder_slack + 14 * i, 14, 1);
            residual(&range, -12_288);

            let mut q_range = Vec::with_capacity(6);
            push_unsigned_terms(&mut q_range, offsets.hash_quotients + 3 * i, 3, 1);
            push_unsigned_terms(&mut q_range, offsets.hash_quotient_slack + 3 * i, 3, 1);
            residual(&q_range, -5);

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

        for i in 0..N {
            let mut range = Vec::with_capacity(28);
            push_unsigned_terms(&mut range, offsets.s1 + 14 * i, 14, 1);
            push_unsigned_terms(&mut range, offsets.s1_range_slack + 14 * i, 14, 1);
            residual(&range, -12_288);
        }

        for i in 0..N {
            let mut ring = Vec::with_capacity(N * 12 + 51);
            push_unsigned_terms(&mut ring, offsets.hash_point + 14 * i, 14, 1);
            for j in 0..N {
                let coefficient = if i >= j {
                    -i128::from(statement.public_keys[instance].h[i - j])
                } else {
                    i128::from(statement.public_keys[instance].h[i + N - j])
                };
                push_s2_terms(&mut ring, &offsets, j, coefficient);
            }
            push_unsigned_terms(&mut ring, offsets.s1 + 14 * i, 14, -1);
            push_signed_terms(
                &mut ring,
                offsets.ring_quotients + 23 * i,
                23,
                -i128::from(Q),
            );
            residual(&ring, 6_144);
        }

        if row != super::FalconConstraintCounts::per_signature().linear_rows() {
            return Err(piop(format!(
                "linear row inventory mismatch: built {row} rows"
            )));
        }
    }
    Ok(constant)
}

fn add_norm_claims(
    coefficients: &mut [F],
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = eq_table(&proof.norm.point, field).map_err(|error| piop(error.to_string()))?;
    let offsets = FalconSourceOffsets::new();
    for side in 0..2 {
        let mut constant = field.zero();
        for instance in 0..layout.batch() {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            for i in 0..N {
                let weight = field.mul(scale, &weights[instance * N + i]);
                if side == 0 {
                    add_unsigned_scaled(
                        coefficients,
                        base + offsets.s1 + 14 * i,
                        14,
                        weight,
                        field,
                    );
                    constant = field.add(&constant, &mul_i(weight, -6_144, field));
                } else {
                    add_signed_source_scaled(coefficients, base, &offsets, i, weight, field);
                }
            }
        }
        add_claim_target(
            target,
            *scale,
            proof.norm.terminal[side][0],
            constant,
            field,
        );
        *scale = field.mul(scale, &eta);
        let mut constant = field.zero();
        for instance in 0..layout.batch() {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            for i in 0..N {
                let weight = field.mul(scale, &weights[instance * N + i]);
                if side == 0 {
                    add_unsigned_scaled(
                        coefficients,
                        base + offsets.s1 + 14 * i,
                        14,
                        weight,
                        field,
                    );
                    constant = field.add(&constant, &mul_i(weight, -6_144, field));
                } else {
                    add_signed_source_scaled(coefficients, base, &offsets, i, weight, field);
                }
            }
        }
        add_claim_target(
            target,
            *scale,
            proof.norm.terminal[side][1],
            constant,
            field,
        );
        *scale = field.mul(scale, &eta);
    }
    let constant = field.zero();
    for instance in 0..layout.batch() {
        add_unsigned_scaled(
            coefficients,
            instance * FalconSourceLayout::SIGNATURE_STRIDE + offsets.norm_slack,
            27,
            *scale,
            field,
        );
    }
    add_claim_target(target, *scale, proof.norm.slack, constant, field);
    *scale = field.mul(scale, &eta);
    Ok(())
}

fn add_keccak_claims(
    coefficients: &mut [F],
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights =
        eq_table(&proof.keccak_chi.point, field).map_err(|error| piop(error.to_string()))?;
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
            for gate in 0..(KECCAK_PERMUTATIONS * KECCAK_ROUNDS * KECCAK_LANES * 64) {
                let word = gate >> 6;
                let bit = gate & 63;
                let lane = word % 25;
                let x = lane % 5;
                let y = lane / 5;
                let round = (word / 25) % 24;
                let rel0 = field.mul(scale, &weights[instance * KECCAK_GATE_STRIDE + gate]);
                let rel1 = field.mul(scale, &weights[half + instance * KECCAK_GATE_STRIDE + gate]);
                let chi_base = word - lane + y * 5;
                let b1 = offsets.keccak_chi_inputs + 64 * (chi_base + (x + 1) % 5) + bit;
                let b2 = offsets.keccak_chi_inputs + 64 * (chi_base + (x + 2) % 5) + bit;
                let bx = offsets.keccak_chi_inputs + gate;
                let z = offsets.keccak_chi_ands + gate;
                let y_out = offsets.keccak_round_states + gate;
                match coordinate {
                    0 => {
                        constant = field.add(&constant, &rel0);
                        add_coefficient(
                            coefficients,
                            base + b1,
                            field.sub(&field.zero(), &rel0),
                            field,
                        );
                        add_coefficient(coefficients, base + bx, rel1, field);
                    }
                    1 => {
                        add_coefficient(coefficients, base + b2, rel0, field);
                        add_coefficient(coefficients, base + z, rel1, field);
                    }
                    2 => {
                        add_coefficient(coefficients, base + z, rel0, field);
                        let half_rel1 = field.mul(&rel1, &two_inv);
                        add_coefficient(coefficients, base + bx, half_rel1, field);
                        add_coefficient(coefficients, base + z, half_rel1, field);
                        let rc = (super::keccak::ROUND_CONSTANTS[round] >> bit) & 1;
                        if lane == 0 && rc == 1 {
                            constant = field.sub(&constant, &half_rel1);
                            add_coefficient(coefficients, base + y_out, half_rel1, field);
                        } else {
                            add_coefficient(
                                coefficients,
                                base + y_out,
                                field.sub(&field.zero(), &half_rel1),
                                field,
                            );
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        add_claim_target(target, *scale, claims[coordinate], constant, field);
        *scale = field.mul(scale, &eta);
    }
    Ok(())
}

fn add_compaction_product_claims(
    coefficients: &mut [F],
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights =
        eq_table(&proof.compact_products.point, field).map_err(|error| piop(error.to_string()))?;
    let stride = COMPACTION_LEAVES * layout.capacity();
    let offsets = FalconSourceOffsets::new();
    let claims = [
        proof.compact_products.terminal.ax,
        proof.compact_products.terminal.bx,
        proof.compact_products.terminal.cx,
    ];
    for coordinate in 0..3 {
        let mut constant = field.zero();
        for instance in 0..layout.batch() {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            for i in 0..HASH_TO_POINT_SAMPLES {
                let source_row = instance * COMPACTION_LEAVES + i;
                for block in 0..27 {
                    let weight = field.mul(scale, &weights[block * stride + source_row]);
                    let reject = offsets.hash_accept_ands + i;
                    let selector = offsets.compact_selectors + i;
                    let (index, c) = match (block, coordinate) {
                        (0, 0) => (reject, 1),
                        (0, 1) => (offsets.hash_prefixes + 11 * i + 10, 1),
                        (0, 2) => (selector, 0),
                        (1..=11, 0) => (selector, 0),
                        (1..=11, 1) => (offsets.hash_prefixes + 11 * i + block - 1, 0),
                        (1..=11, 2) => (offsets.compact_selected_prefixes + 11 * i + block - 1, 0),
                        (12..=25, 0) => (selector, 0),
                        (12..=25, 1) => (offsets.hash_remainders + 14 * i + block - 12, 0),
                        (12..=25, 2) => {
                            (offsets.compact_selected_remainders + 14 * i + block - 12, 0)
                        }
                        (26, 0) => (offsets.hash_quotients + 3 * i + 2, 0),
                        (26, 1) => (offsets.hash_quotients + 3 * i, 0),
                        (26, 2) => (reject, 0),
                        _ => unreachable!(),
                    };
                    let coefficient = if block == 0 && coordinate < 2 {
                        field.sub(&field.zero(), &weight)
                    } else {
                        weight
                    };
                    add_coefficient(coefficients, base + index, coefficient, field);
                    if c == 1 {
                        constant = field.add(&constant, &weight);
                    }
                }
            }
        }
        add_claim_target(target, *scale, claims[coordinate], constant, field);
        *scale = field.mul(scale, &eta);
    }
    Ok(())
}

fn add_product_tree_claims(
    coefficients: &mut [F],
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let offsets = FalconSourceOffsets::new();
    for instance in 0..layout.batch() {
        let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
        let pair = &proof.compaction[instance];
        for (candidate, tree) in [true, false]
            .into_iter()
            .zip([&pair.candidate, &pair.output])
        {
            let weights =
                eq_table(&tree.terminal_point, field).map_err(|error| piop(error.to_string()))?;
            let mut constant = field.zero();
            for (i, &leaf_weight) in weights.iter().enumerate() {
                let weight = field.mul(scale, &leaf_weight);
                if candidate {
                    constant = field.add(&constant, &weight);
                    if i < HASH_TO_POINT_SAMPLES {
                        add_coefficient(
                            coefficients,
                            base + offsets.compact_selectors + i,
                            field.mul(&weight, &field.sub(&proof.compaction_gamma, &field.one())),
                            field,
                        );
                        add_unsigned_scaled(
                            coefficients,
                            base + offsets.compact_selected_prefixes + 11 * i,
                            11,
                            field.mul(&weight, &proof.compaction_rank_scale),
                            field,
                        );
                        add_unsigned_scaled(
                            coefficients,
                            base + offsets.compact_selected_remainders + 14 * i,
                            14,
                            weight,
                            field,
                        );
                    }
                } else if i < N {
                    constant = field.add(
                        &constant,
                        &field.mul(
                            &weight,
                            &field.add(
                                &proof.compaction_gamma,
                                &field
                                    .mul(&proof.compaction_rank_scale, &unsigned(i as u128, field)),
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
    }
    Ok(())
}

fn add_claim_target(target: &mut F, scale: F, claimed: F, constant: F, field: &Cfg) {
    *target = field.add(target, &field.sub(&field.mul(&scale, &claimed), &constant));
}

fn add_coefficient(values: &mut [F], index: usize, value: F, field: &Cfg) {
    values[index] = field.add(&values[index], &value);
}

fn add_unsigned_scaled(values: &mut [F], base: usize, width: usize, scale: F, field: &Cfg) {
    for bit in 0..width {
        add_coefficient(
            values,
            base + bit,
            field.mul(&scale, &unsigned(1u128 << bit, field)),
            field,
        );
    }
}

fn add_signed_source_scaled(
    values: &mut [F],
    base: usize,
    offsets: &FalconSourceOffsets,
    coefficient: usize,
    scale: F,
    field: &Cfg,
) {
    for bit in 0..12 {
        let signed_weight = if bit == 11 {
            -(1i128 << 11)
        } else {
            1i128 << bit
        };
        add_coefficient(
            values,
            base + s2_bit_index(offsets, coefficient, bit),
            field.mul(&scale, &signed(signed_weight, field)),
            field,
        );
    }
}

fn push_unsigned_terms(terms: &mut Vec<(usize, i128)>, base: usize, width: usize, scale: i128) {
    for bit in 0..width {
        terms.push((base + bit, scale * (1i128 << bit)));
    }
}

fn push_signed_terms(terms: &mut Vec<(usize, i128)>, base: usize, width: usize, scale: i128) {
    for bit in 0..width - 1 {
        terms.push((base + bit, scale * (1i128 << bit)));
    }
    terms.push((base + width - 1, -scale * (1i128 << (width - 1))));
}

fn push_s2_terms(
    terms: &mut Vec<(usize, i128)>,
    offsets: &FalconSourceOffsets,
    coefficient: usize,
    scale: i128,
) {
    for bit in 0..11 {
        terms.push((
            s2_bit_index(offsets, coefficient, bit),
            scale * (1i128 << bit),
        ));
    }
    terms.push((
        s2_bit_index(offsets, coefficient, 11),
        -scale * (1i128 << 11),
    ));
}

fn push_round_input(
    terms: &mut Vec<(usize, i128)>,
    constant: &mut i128,
    offsets: &FalconSourceOffsets,
    permutation: usize,
    round: usize,
    x: usize,
    y: usize,
    bit: usize,
    scale: i128,
) {
    if round > 0 || permutation > 0 {
        let previous_permutation = if round == 0 {
            permutation - 1
        } else {
            permutation
        };
        let previous_round = if round == 0 {
            KECCAK_ROUNDS - 1
        } else {
            round - 1
        };
        let word = (previous_permutation * KECCAK_ROUNDS + previous_round) * 25 + x + 5 * y;
        terms.push((offsets.keccak_round_states + 64 * word + bit, scale));
        return;
    }
    let byte = (x + 5 * y) * 8 + bit / 8;
    let byte_bit = bit % 8;
    if byte < 40 {
        terms.push((offsets.encoded_signature + 8 * (1 + byte) + byte_bit, scale));
    } else if byte < 72 {
        terms.push((offsets.message + 8 * (byte - 40) + byte_bit, scale));
    } else {
        let value = if byte == 72 {
            0x1f
        } else if byte == RATE_BYTES - 1 {
            0x80
        } else {
            0
        };
        *constant += scale * i128::from((value >> byte_bit) & 1);
    }
}

fn s2_bit_index(offsets: &FalconSourceOffsets, coefficient: usize, bit: usize) -> usize {
    let stream = 12 * coefficient + (11 - bit);
    let byte = 1 + super::NONCE_BYTES + stream / 8;
    let byte_bit = 7 - stream % 8;
    offsets.encoded_signature + 8 * byte + byte_bit
}

fn shake_word_bit_index(offsets: &FalconSourceOffsets, sample: usize, bit: usize) -> usize {
    let output_byte = if bit < 8 { 2 * sample + 1 } else { 2 * sample };
    let byte_bit = bit % 8;
    let permutation = output_byte / RATE_BYTES;
    let within_rate = output_byte % RATE_BYTES;
    let lane = within_rate / 8;
    let lane_bit = 8 * (within_rate % 8) + byte_bit;
    let word = (permutation * KECCAK_ROUNDS + KECCAK_ROUNDS - 1) * 25 + lane;
    offsets.keccak_round_states + 64 * word + lane_bit
}

fn keccak_gate(permutation: usize, round: usize, x: usize, y: usize, bit: usize) -> usize {
    64 * (((permutation * KECCAK_ROUNDS + round) * 25) + x + 5 * y) + bit
}

fn rho_pi_source(destination_x: usize, destination_y: usize) -> (usize, usize) {
    let source_y = destination_x;
    let source_x = (3 * (destination_y + 5 - (3 * source_y) % 5)) % 5;
    (source_x, source_y)
}

fn dot_packed_source(coefficients: &[F], source: &FalconSourceWitness, field: &Cfg) -> F {
    let row_vars = source.layout().row_vars();
    source
        .rows()
        .iter()
        .enumerate()
        .fold(field.zero(), |mut sum, (column, words)| {
            for (word_index, &packed) in words.iter().enumerate() {
                let mut remaining = packed;
                while remaining != 0 {
                    let bit = remaining.trailing_zeros() as usize;
                    let index = (column << row_vars) + 64 * word_index + bit;
                    sum = field.add(&sum, &coefficients[index]);
                    remaining &= remaining - 1;
                }
            }
            sum
        })
}

fn evaluate_mle_in_place(values: &mut Vec<F>, point: &[F], field: &Cfg) -> Result<F, FalconError> {
    if values.len() != 1usize << point.len() {
        return Err(piop("MLE evaluation shape mismatch"));
    }
    let mut active = values.len();
    for challenge in point {
        for index in 0..active / 2 {
            let left = values[2 * index];
            values[index] = field.add(
                &left,
                &field.mul(challenge, &field.sub(&values[2 * index + 1], &left)),
            );
        }
        active /= 2;
    }
    Ok(values[0])
}

fn canonical_eq_weights(point: &[F], field: &Cfg) -> Result<Vec<u128>, FalconError> {
    eq_table(point, field)
        .map_err(|error| piop(error.to_string()))
        .map(|weights| {
            weights
                .into_iter()
                .map(|value| canonical(value, field))
                .collect()
        })
}

fn bind_opening_claim(
    transcript: &mut impl Transcript,
    modulus: u128,
    point: &[F],
    value: F,
    field: &Cfg,
) {
    transcript.absorb_slice(b"bitz/falcon1024-ct/source-opening/v1");
    transcript.absorb_slice(&modulus.to_le_bytes());
    for coordinate in point {
        transcript.absorb_slice(&coordinate.canonical_element_encoding(field));
    }
    transcript.absorb_slice(&value.canonical_element_encoding(field));
}

fn sample_point(
    transcript: &mut impl Transcript,
    rounds: usize,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    (0..rounds).map(|_| squeeze(transcript, field)).collect()
}

fn squeeze(transcript: &mut impl Transcript, field: &Cfg) -> Result<F, FalconError> {
    crate::piop::spartan::squeeze_field(transcript, field).map_err(|error| piop(error.to_string()))
}

fn field_from_modulus(modulus: u128) -> Result<Cfg, FalconError> {
    F::make_cfg(&Uint::from(modulus)).map_err(|_| piop("invalid Falcon proof modulus"))
}

fn canonical(value: F, field: &Cfg) -> u128 {
    u128::from(field.to_integer(&value))
}

fn unsigned(value: u128, field: &Cfg) -> F {
    F::from_with_cfg(value, field)
}

fn signed(value: i128, field: &Cfg) -> F {
    let magnitude = unsigned(value.unsigned_abs(), field);
    if value.is_negative() {
        field.sub(&field.zero(), &magnitude)
    } else {
        magnitude
    }
}

fn mul_i(value: F, coefficient: i128, field: &Cfg) -> F {
    field.mul(&value, &signed(coefficient, field))
}

fn linear_rounds(layout: &FalconSourceLayout) -> usize {
    20 + layout.capacity().trailing_zeros() as usize
}

fn source_rounds(layout: &FalconSourceLayout) -> usize {
    layout.row_vars() + layout.col_vars()
}

fn modulus_bits(modulus: u128) -> usize {
    (u128::BITS - modulus.leading_zeros()) as usize
}

fn piop(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        piop::spartan::falcon1024_ct::{
            commit_falcon_source, falcon_ligerito_configs, verification_trace,
        },
        transcript::Blake3Transcript,
    };

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    #[test]
    fn commitment_bound_proof_roundtrips() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let statement = FalconPublicStatement::from_bytes(&[PUBLIC_KEY], &[MESSAGE]).unwrap();
        let source =
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[trace.clone()])
                .unwrap();
        let (pc, vc) = falcon_ligerito_configs(&layout, 100).unwrap();
        let hint = commit_falcon_source(&source, &pc);
        let mut prover = Blake3Transcript::new();
        let proof = prove_falcon_bitz(
            &mut prover,
            &layout,
            &statement,
            &[trace],
            &source,
            &hint,
            100,
            &pc,
        )
        .unwrap();
        let mut verifier = Blake3Transcript::new();
        verify_falcon_bitz(
            &mut verifier,
            &layout,
            &statement,
            &hint.commitment,
            &proof,
            100,
            &vc,
        )
        .unwrap();

        let mut wrong_statement = statement.clone();
        wrong_statement.messages[0][0] ^= 1;
        let mut rejecting_verifier = Blake3Transcript::new();
        assert!(
            verify_falcon_bitz(
                &mut rejecting_verifier,
                &layout,
                &wrong_statement,
                &hint.commitment,
                &proof,
                100,
                &vc,
            )
            .is_err()
        );
    }
}
