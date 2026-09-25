use flock_core::pcs::ligerito::{
    ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig,
};

use crate::{
    ligerito::packed_vars,
    ligerito_flock::{
        FlockCommitHint, commit_rs_ligerito_rows, validated_udr_lig_configs_for_target,
    },
};

use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N,
};

/// Start offsets of each committed column inside one `2^22`-bit signature
/// stride.  The interval ending at `end` is live; `[end,2^22)` is canonical
/// zero padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceOffsets {
    pub shared_one: usize,
    pub message: usize,
    pub encoded_signature: usize,
    pub s2_non_min_slack: usize,
    pub keccak_chi_inputs: usize,
    pub keccak_chi_ands: usize,
    pub keccak_round_states: usize,
    pub keccak_column_parities: usize,
    pub keccak_column_parity_quotients: usize,
    pub keccak_parity_quotients: usize,
    pub hash_quotients: usize,
    pub hash_remainders: usize,
    pub hash_remainder_slack: usize,
    pub hash_quotient_slack: usize,
    pub hash_accept_ands: usize,
    pub hash_prefixes: usize,
    pub compact_selectors: usize,
    pub compact_selected_prefixes: usize,
    pub compact_selected_remainders: usize,
    pub hash_point: usize,
    pub s1: usize,
    pub s1_range_slack: usize,
    pub ring_quotients: usize,
    pub norm_slack: usize,
    pub end: usize,
}

impl FalconSourceOffsets {
    pub const fn new() -> Self {
        let counts = FalconSourceLayout::counts();
        let shared_one = 0;
        let message = shared_one + counts.shared_one;
        let encoded_signature = message + counts.message;
        let s2_non_min_slack = encoded_signature + counts.encoded_signature;
        let keccak_chi_inputs = s2_non_min_slack + counts.s2_non_min_slack;
        let keccak_chi_ands = keccak_chi_inputs + counts.keccak_chi_inputs;
        let keccak_round_states = keccak_chi_ands + counts.keccak_chi_ands;
        let keccak_column_parities = keccak_round_states + counts.keccak_round_states;
        let keccak_column_parity_quotients = keccak_column_parities + counts.keccak_column_parities;
        let keccak_parity_quotients =
            keccak_column_parity_quotients + counts.keccak_column_parity_quotients;
        let hash_quotients = keccak_parity_quotients + counts.keccak_parity_quotients;
        let hash_remainders = hash_quotients + counts.hash_quotients;
        let hash_remainder_slack = hash_remainders + counts.hash_remainders;
        let hash_quotient_slack = hash_remainder_slack + counts.hash_remainder_slack;
        let hash_accept_ands = hash_quotient_slack + counts.hash_quotient_slack;
        let hash_prefixes = hash_accept_ands + counts.hash_accept_ands;
        let compact_selectors = hash_prefixes + counts.hash_prefixes;
        let compact_selected_prefixes = compact_selectors + counts.compact_selectors;
        let compact_selected_remainders =
            compact_selected_prefixes + counts.compact_selected_prefixes;
        let hash_point = compact_selected_remainders + counts.compact_selected_remainders;
        let s1 = hash_point + counts.hash_point;
        let s1_range_slack = s1 + counts.s1;
        let ring_quotients = s1_range_slack + counts.s1_range_slack;
        let norm_slack = ring_quotients + counts.ring_quotients;
        let end = norm_slack + counts.norm_slack;
        Self {
            shared_one,
            message,
            encoded_signature,
            s2_non_min_slack,
            keccak_chi_inputs,
            keccak_chi_ands,
            keccak_round_states,
            keccak_column_parities,
            keccak_column_parity_quotients,
            keccak_parity_quotients,
            hash_quotients,
            hash_remainders,
            hash_remainder_slack,
            hash_quotient_slack,
            hash_accept_ands,
            hash_prefixes,
            compact_selectors,
            compact_selected_prefixes,
            compact_selected_remainders,
            hash_point,
            s1,
            s1_range_slack,
            ring_quotients,
            norm_slack,
            end,
        }
    }
}

/// The one committed `F_2` source witness for a Falcon batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconSourceWitness {
    layout: FalconSourceLayout,
    rows: Vec<Vec<u64>>,
}

impl FalconSourceWitness {
    /// Packs already-validated native traces.  All integer encodings are
    /// little-endian within their fixed-width source column; the raw Falcon
    /// bytes retain byte order and use least-significant-bit-first byte bits.
    pub fn from_traces(
        layout: FalconSourceLayout,
        messages: &[&[u8]],
        signatures: &[&[u8]],
        traces: &[FalconVerificationTrace],
    ) -> Result<Self, FalconError> {
        if messages.len() != layout.batch()
            || signatures.len() != layout.batch()
            || traces.len() != layout.batch()
        {
            return Err(FalconError::InvalidBatchCapacity);
        }
        let p = layout.bitz_params();
        let mut rows = vec![vec![0u64; p.rows() / 64]; p.cols()];
        let offsets = FalconSourceOffsets::new();
        debug_assert_eq!(offsets.end, FalconSourceLayout::counts().total());

        for instance in 0..layout.batch() {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            let signature = signatures[instance];
            let message = messages[instance];
            let trace = &traces[instance];
            trace.hash_to_point.shake.validate_falcon_shape()?;
            if signature.len() != super::CT_SIGNATURE_BYTES {
                return Err(FalconError::SignatureLength);
            }
            if message.len() != 32 {
                return Err(FalconError::InvalidBatchCapacity);
            }

            put_unsigned(&mut rows, &p, base + offsets.shared_one, 1, 1);
            for (byte_index, &byte) in message.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.message + 8 * byte_index,
                    u64::from(byte),
                    8,
                );
            }

            for (byte_index, &byte) in signature.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.encoded_signature + 8 * byte_index,
                    u64::from(byte),
                    8,
                );
            }
            for (i, &coefficient) in trace.signature.s2.iter().enumerate() {
                let encoded = (i32::from(coefficient) as u32) & 0xfff;
                let low_sum = (encoded & 0x7ff).count_ones();
                let sign = encoded >> 11;
                let slack = low_sum
                    .checked_sub(sign)
                    .expect("canonical CT decoding excludes signed minimum");
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.s2_non_min_slack + 4 * i,
                    u64::from(slack),
                    4,
                );
            }
            for (word_index, &word) in trace.hash_to_point.shake.chi_ands.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_chi_ands + 64 * word_index,
                    word,
                    64,
                );
            }
            for (word_index, &word) in trace.hash_to_point.shake.chi_inputs.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_chi_inputs + 64 * word_index,
                    word,
                    64,
                );
            }
            for (word_index, &word) in trace.hash_to_point.shake.round_states.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_round_states + 64 * word_index,
                    word,
                    64,
                );
            }
            for (word_index, &word) in trace.hash_to_point.shake.column_parities.iter().enumerate()
            {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_column_parities + 64 * word_index,
                    word,
                    64,
                );
            }
            for (bit_index, &quotient) in trace
                .hash_to_point
                .shake
                .column_parity_quotients
                .iter()
                .enumerate()
            {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_column_parity_quotients + 2 * bit_index,
                    u64::from(quotient),
                    2,
                );
            }
            for (bit_index, &quotient) in trace
                .hash_to_point
                .shake
                .parity_quotients
                .iter()
                .enumerate()
            {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.keccak_parity_quotients + bit_index,
                    u64::from(quotient),
                    1,
                );
            }
            for i in 0..HASH_TO_POINT_SAMPLES {
                let quotient = trace.hash_to_point.quotients[i];
                let remainder = trace.hash_to_point.remainders[i];
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_quotients + 3 * i,
                    u64::from(quotient),
                    3,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_remainders + 14 * i,
                    u64::from(remainder),
                    14,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_remainder_slack + 14 * i,
                    u64::from(12_288 - remainder),
                    14,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_quotient_slack + 3 * i,
                    u64::from(5 - quotient),
                    3,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_accept_ands + i,
                    u64::from((quotient & 0b101) == 0b101),
                    1,
                );
            }
            for (i, &prefix) in trace.hash_to_point.prefix.iter().enumerate() {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_prefixes + 11 * i,
                    u64::from(prefix),
                    11,
                );
            }
            for i in 0..HASH_TO_POINT_SAMPLES {
                let selected =
                    trace.hash_to_point.accepted[i] && trace.hash_to_point.prefix[i] < 1024;
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.compact_selectors + i,
                    u64::from(selected),
                    1,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.compact_selected_prefixes + 11 * i,
                    if selected {
                        u64::from(trace.hash_to_point.prefix[i])
                    } else {
                        0
                    },
                    11,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.compact_selected_remainders + 14 * i,
                    if selected {
                        u64::from(trace.hash_to_point.remainders[i])
                    } else {
                        0
                    },
                    14,
                );
            }
            for i in 0..N {
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.hash_point + 14 * i,
                    u64::from(trace.hash_to_point.point[i]),
                    14,
                );
                let biased_s1 = i64::from(trace.s1[i]) + 6_144;
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.s1 + 14 * i,
                    biased_s1 as u64,
                    14,
                );
                put_unsigned(
                    &mut rows,
                    &p,
                    base + offsets.s1_range_slack + 14 * i,
                    (12_288 - biased_s1) as u64,
                    14,
                );
                put_signed(
                    &mut rows,
                    &p,
                    base + offsets.ring_quotients + 23 * i,
                    trace.quotient[i],
                    23,
                );
            }
            debug_assert_eq!(trace.norm + trace.norm_slack, BETA_SQUARED);
            put_unsigned(
                &mut rows,
                &p,
                base + offsets.norm_slack,
                trace.norm_slack,
                27,
            );
        }
        Ok(Self { layout, rows })
    }

    pub const fn layout(&self) -> &FalconSourceLayout {
        &self.layout
    }

    pub fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }

    pub fn into_rows(self) -> Vec<Vec<u64>> {
        self.rows
    }

    pub fn bit(&self, flat: usize) -> bool {
        let p = self.layout.bitz_params();
        let b = flat & (p.rows() - 1);
        let c = flat >> p.row_vars;
        self.rows[c][b / 64] >> (b % 64) & 1 == 1
    }
}

/// Validator-gated production opener configurations for this source shape.
pub fn falcon_ligerito_configs(
    layout: &FalconSourceLayout,
    target_bits: usize,
) -> Result<(LigProverConfig, LigVerifierConfig), FalconError> {
    validated_udr_lig_configs_for_target(packed_vars(&layout.bitz_params()), target_bits)
        .map_err(|_| FalconError::SourceStrideOverflow)
}

/// Commits the complete Falcon source witness with the real BitZ/Flock
/// commitment used by the later GKR opening.
pub fn commit_falcon_source(
    witness: &FalconSourceWitness,
    config: &LigProverConfig,
) -> FlockCommitHint {
    let p = witness.layout.bitz_params();
    commit_rs_ligerito_rows(&p, witness.rows.clone(), config)
}

fn put_unsigned(
    rows: &mut [Vec<u64>],
    p: &crate::pcs::IntegerMatrixLayout,
    flat: usize,
    value: u64,
    width: usize,
) {
    assert!(width <= 64 && (width == 64 || value < 1u64 << width));
    for bit in 0..width {
        if value >> bit & 1 == 1 {
            let index = flat + bit;
            let b = index & (p.rows() - 1);
            let c = index >> p.row_vars;
            rows[c][b / 64] |= 1u64 << (b % 64);
        }
    }
}

fn put_signed(
    rows: &mut [Vec<u64>],
    p: &crate::pcs::IntegerMatrixLayout,
    flat: usize,
    value: i64,
    width: usize,
) {
    assert!(width < 64);
    let min = -(1i64 << (width - 1));
    let max = (1i64 << (width - 1)) - 1;
    assert!((min..=max).contains(&value));
    let encoded = (value as u64) & ((1u64 << width) - 1);
    put_unsigned(rows, p, flat, encoded, width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::spartan::falcon1024_ct::verification_trace;

    const PUBLIC_KEY: &[u8; super::super::PUBLIC_KEY_BYTES] =
        include_bytes!("fixtures/public_key.bin");
    const MESSAGE: &[u8; 32] = include_bytes!("fixtures/message.bin");
    const SIGNATURE: &[u8; super::super::CT_SIGNATURE_BYTES] =
        include_bytes!("fixtures/signature_ct.bin");

    fn read_unsigned(witness: &FalconSourceWitness, flat: usize, width: usize) -> u64 {
        (0..width).fold(0, |value, bit| {
            value | (u64::from(witness.bit(flat + bit)) << bit)
        })
    }

    #[test]
    fn source_packing_matches_native_trace_and_zero_padding() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let witness =
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[trace.clone()])
                .unwrap();
        let offsets = FalconSourceOffsets::new();
        assert!(witness.bit(offsets.shared_one));
        assert_eq!(
            read_unsigned(&witness, offsets.message, 8),
            u64::from(MESSAGE[0])
        );
        assert_eq!(read_unsigned(&witness, offsets.encoded_signature, 8), 0x5a);
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_chi_inputs, 64),
            trace.hash_to_point.shake.chi_inputs[0]
        );
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_chi_ands, 64),
            trace.hash_to_point.shake.chi_ands[0]
        );
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_round_states, 64),
            trace.hash_to_point.shake.round_states[0]
        );
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_column_parities, 64),
            trace.hash_to_point.shake.column_parities[0]
        );
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_column_parity_quotients, 2),
            u64::from(trace.hash_to_point.shake.column_parity_quotients[0])
        );
        assert_eq!(
            read_unsigned(&witness, offsets.keccak_parity_quotients, 1),
            u64::from(trace.hash_to_point.shake.parity_quotients[0])
        );
        for index in [1, 63, 64, 319, 320, 153_599] {
            assert_eq!(
                read_unsigned(
                    &witness,
                    offsets.keccak_column_parity_quotients + 2 * index,
                    2
                ),
                u64::from(trace.hash_to_point.shake.column_parity_quotients[index])
            );
        }
        for index in [1, 63, 64, 1_599, 1_600, 767_999] {
            assert_eq!(
                read_unsigned(&witness, offsets.keccak_parity_quotients + index, 1),
                u64::from(trace.hash_to_point.shake.parity_quotients[index])
            );
        }
        assert_eq!(
            read_unsigned(
                &witness,
                offsets.hash_prefixes + 11 * HASH_TO_POINT_SAMPLES,
                11
            ),
            u64::from(trace.hash_to_point.prefix[HASH_TO_POINT_SAMPLES])
        );
        assert_eq!(
            read_unsigned(&witness, offsets.norm_slack, 27),
            trace.norm_slack
        );
        assert!(
            (offsets.end..FalconSourceLayout::SIGNATURE_STRIDE).all(|index| !witness.bit(index))
        );
    }

    #[test]
    fn source_packing_rejects_malformed_keccak_auxiliaries() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        trace.hash_to_point.shake.column_parities.pop();
        assert!(matches!(
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[trace]),
            Err(FalconError::ConstraintViolation {
                family: "keccak-shape",
                ..
            })
        ));

        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        trace.hash_to_point.shake.parity_quotients[0] = 2;
        assert!(matches!(
            FalconSourceWitness::from_traces(layout, &[MESSAGE], &[SIGNATURE], &[trace]),
            Err(FalconError::ConstraintViolation {
                family: "keccak-theta-quotient-range",
                index: 0,
            })
        ));
    }

    #[test]
    fn batch_padding_stays_zero_after_stride_reduction() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(3).unwrap();
        let witness = FalconSourceWitness::from_traces(
            layout,
            &[MESSAGE, MESSAGE, MESSAGE],
            &[SIGNATURE, SIGNATURE, SIGNATURE],
            &[trace.clone(), trace.clone(), trace],
        )
        .unwrap();
        let offsets = FalconSourceOffsets::new();
        for instance in 0..3 {
            let base = instance * FalconSourceLayout::SIGNATURE_STRIDE;
            assert!(witness.bit(base + offsets.shared_one));
            assert_eq!(
                read_unsigned(&witness, base + offsets.encoded_signature, 8),
                0x5a
            );
        }
        assert_eq!(layout.capacity(), 4);
        assert!(
            (3 * FalconSourceLayout::SIGNATURE_STRIDE..layout.source_bits())
                .all(|index| !witness.bit(index))
        );
    }

    #[test]
    fn production_configs_cover_both_targets() {
        let layout = FalconSourceLayout::new(1).unwrap();
        falcon_ligerito_configs(&layout, 100).unwrap();
        falcon_ligerito_configs(&layout, 128).unwrap();
    }
}
