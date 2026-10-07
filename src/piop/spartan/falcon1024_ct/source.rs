use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N,
};
use super::{NORM_BITS, PREFIX_BIAS, PREFIX_BITS};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Start offsets of committed columns within one signature stride.
/// The interval ending at `end` is live; the rest of the stride is zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceOffsets {
    pub shared_one: usize,
    pub message: usize,
    pub encoded_signature: usize,
    pub hash_words: usize,
    pub hash_quotients: usize,
    pub hash_remainders: usize,
    pub hash_accept_ands: usize,
    pub hash_prefixes: usize,
    pub hash_point: usize,
    pub s1: usize,
    pub norm_slack: usize,
    pub public_key: Option<usize>,
    pub end: usize,
}

impl FalconSourceOffsets {
    pub(super) const fn new(shared_prime: bool) -> Self {
        let counts = FalconSourceLayout::counts();
        let shared_one = 0;
        let message = shared_one + counts.shared_one;
        let encoded_signature = message + counts.message;
        let hash_words = encoded_signature + counts.encoded_signature;
        let hash_quotients = hash_words + counts.hash_words;
        let hash_remainders = hash_quotients + counts.hash_quotients;
        let hash_accept_ands = hash_remainders + counts.hash_remainders;
        let hash_prefixes = hash_accept_ands + counts.hash_accept_ands;
        let hash_point = hash_prefixes + counts.hash_prefixes;
        let s1 = hash_point + counts.hash_point;
        let norm_slack = s1 + counts.s1;
        let legacy_end = norm_slack + counts.norm_slack;
        let public_key = if shared_prime { Some(legacy_end) } else { None };
        let end = legacy_end + if shared_prime { 14 * N } else { 0 };
        Self {
            shared_one,
            message,
            encoded_signature,
            hash_words,
            hash_quotients,
            hash_remainders,
            hash_accept_ands,
            hash_prefixes,
            hash_point,
            s1,
            norm_slack,
            public_key,
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
    /// Packs already-validated native traces. Integer bit strings are stored
    /// little-endian; remainders and biased s1 use `bounded14_encode`.
    /// Raw Falcon bytes retain byte order and use least-significant-bit-first
    /// byte bits.
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
        let offsets = layout.offsets();
        debug_assert_eq!(offsets.end, layout.live_bits());

        crate::utils::cfg_chunks_mut!(rows, layout.signature_stride() >> p.row_vars)
            .enumerate()
            .take(layout.batch())
            .try_for_each(|(instance, rows)| -> Result<(), FalconError> {
                let signature = signatures[instance];
                let message = messages[instance];
                let trace = &traces[instance];

                if signature.len() != super::CT_SIGNATURE_BYTES {
                    return Err(FalconError::SignatureLength);
                }
                if message.len() != 32 {
                    return Err(FalconError::InvalidBatchCapacity);
                }

                put_unsigned(rows, &p, offsets.shared_one, 1, 1);
                for (byte_index, &byte) in message.iter().enumerate() {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.message + 8 * byte_index,
                        u64::from(byte),
                        8,
                    );
                }

                for (byte_index, &byte) in signature.iter().enumerate() {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.encoded_signature + 8 * byte_index,
                        u64::from(byte),
                        8,
                    );
                }

                for i in 0..HASH_TO_POINT_SAMPLES {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_words + 16 * i,
                        u64::from(trace.hash_to_point.words[i]),
                        16,
                    );

                    let quotient = trace.hash_to_point.quotients[i];
                    let remainder = trace.hash_to_point.remainders[i];
                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_quotients + 3 * i,
                        u64::from(quotient),
                        3,
                    );
                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_remainders + 14 * i,
                        u64::from(bounded14_encode(remainder)),
                        14,
                    );

                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_accept_ands + i,
                        u64::from((quotient & 0b101) == 0b101),
                        1,
                    );
                }
                for (i, &prefix) in trace.hash_to_point.prefix.iter().enumerate() {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_prefixes + PREFIX_BITS * i,
                        u64::from(prefix) + PREFIX_BIAS as u64,
                        PREFIX_BITS,
                    );
                }

                for i in 0..N {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.hash_point + 14 * i,
                        u64::from(trace.hash_to_point.point[i]),
                        14,
                    );
                    let biased_s1 = i64::from(trace.s1[i]) + 6_144;
                    put_unsigned(
                        rows,
                        &p,
                        offsets.s1 + 14 * i,
                        u64::from(bounded14_encode(biased_s1 as u16)),
                        14,
                    );
                }
                debug_assert_eq!(trace.norm + trace.norm_slack, BETA_SQUARED);
                put_unsigned(rows, &p, offsets.norm_slack, trace.norm_slack, NORM_BITS);
                if let Some(public_key) = offsets.public_key {
                    for (i, &coefficient) in trace.public_key.h.iter().enumerate() {
                        if i64::from(coefficient) >= super::Q {
                            return Err(FalconError::PublicKeyCoefficient { index: i });
                        }
                        put_unsigned(rows, &p, public_key + 14 * i, u64::from(coefficient), 14);
                    }
                }
                Ok(())
            })?;
        Ok(Self { layout, rows })
    }

    pub const fn layout(&self) -> &FalconSourceLayout {
        &self.layout
    }

    pub fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }

    /// Native check of the shared layout's decoded H columns against the
    /// public statement. Proof verification additionally binds these rows.
    pub fn check_public_key_bindings(
        &self,
        public_keys: &[super::FalconPublicKey],
    ) -> Result<(), FalconError> {
        super::constraints::check_public_key_bindings(self, public_keys)
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

/// Canonical encoder for the bounded source decoder
/// `low_13_bits + 4097 * top_bit`. Alternate encodings are permitted by the
/// relation, but witness generation chooses the top bit only above 8191.
pub(super) const fn bounded14_encode(value: u16) -> u16 {
    assert!(value <= 12_288);
    if value > 8_191 {
        (1 << 13) | (value - 4_097)
    } else {
        value
    }
}

/// Every 14-bit string decodes into `0..=12288`; injectivity is not required.
#[cfg(test)]
pub(super) const fn bounded14_decode(encoded: u16) -> u16 {
    assert!(encoded < 1 << 14);
    (encoded & 0x1fff) + 4_097 * (encoded >> 13)
}

fn put_unsigned(
    rows: &mut [Vec<u64>],
    p: &crate::pcs::IntegerMatrixLayout,
    flat: usize,
    value: u64,
    width: usize,
) {
    assert!(width <= 64 && (width == 64 || value < 1u64 << width));
    let words_per_column = p.rows() / 64;
    let word = flat / 64;
    let shift = flat % 64;
    rows[word / words_per_column][word % words_per_column] |= value << shift;
    if shift != 0 && width > 64 - shift {
        let next = word + 1;
        rows[next / words_per_column][next % words_per_column] |= value >> (64 - shift);
    }
}
falcon_tests! {
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
    fn bounded_decoder_covers_exactly_the_falcon_coefficient_range() {
        let mut multiplicities = [0u8; 12_289];
        for encoded in 0..1 << 14 {
            let value = bounded14_decode(encoded);
            multiplicities[usize::from(value)] += 1;
        }
        assert!(multiplicities.iter().all(|&count| (1..=2).contains(&count)));
        assert_eq!(
            multiplicities.iter().filter(|&&count| count == 2).count(),
            4_095
        );
        for value in 0..=12_288 {
            let encoded = bounded14_encode(value);
            assert_eq!(bounded14_decode(encoded), value);
            assert_eq!(encoded >> 13 != 0, value > 8_191);
        }
        assert_eq!(bounded14_decode(1 << 13), 4_097);
        assert_eq!(bounded14_decode((1 << 14) - 1), 12_288);
    }

    #[test]
    fn source_packs_all_columns_and_zero_padding() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        for batch in [1, 3] {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let traces = vec![trace.clone(); batch];
            let witness = FalconSourceWitness::from_traces(
                layout,
                &vec![MESSAGE.as_slice(); batch],
                &vec![SIGNATURE.as_slice(); batch],
                &traces,
            )
            .unwrap();
            let offsets = layout.offsets();
            for instance in 0..batch {
                let base = instance * layout.signature_stride();
                let read = |offset, width| read_unsigned(&witness, base + offset, width);
                assert_eq!(read(offsets.shared_one, 1), 1);
                for (i, &byte) in MESSAGE.iter().enumerate() {
                    assert_eq!(read(offsets.message + 8 * i, 8), u64::from(byte));
                }
                for (i, &byte) in SIGNATURE.iter().enumerate() {
                    assert_eq!(read(offsets.encoded_signature + 8 * i, 8), u64::from(byte));
                }
                for i in 0..HASH_TO_POINT_SAMPLES {
                    assert_eq!(
                        read(offsets.hash_words + 16 * i, 16),
                        u64::from(trace.hash_to_point.words[i])
                    );
                    assert_eq!(
                        read(offsets.hash_quotients + 3 * i, 3),
                        u64::from(trace.hash_to_point.quotients[i])
                    );
                    let encoded = read(offsets.hash_remainders + 14 * i, 14) as u16;
                    assert_eq!(bounded14_decode(encoded), trace.hash_to_point.remainders[i]);
                    assert_eq!(
                        read(offsets.hash_accept_ands + i, 1),
                        u64::from(!trace.hash_to_point.accepted[i])
                    );
                }
                for i in 0..=HASH_TO_POINT_SAMPLES {
                    assert_eq!(
                        read(offsets.hash_prefixes + 11 * i, 11),
                        u64::from(trace.hash_to_point.prefix[i])
                    );
                }
                for i in 0..N {
                    assert_eq!(
                        read(offsets.hash_point + 14 * i, 14),
                        u64::from(trace.hash_to_point.point[i])
                    );
                    let encoded = read(offsets.s1 + 14 * i, 14) as u16;
                    assert_eq!(
                        i32::from(bounded14_decode(encoded)) - 6_144,
                        i32::from(trace.s1[i])
                    );
                }
                assert_eq!(read(offsets.norm_slack, 27), trace.norm_slack);
                assert!((offsets.end..layout.signature_stride()).all(|i| !witness.bit(base + i)));
            }
            assert!(
                (batch * layout.signature_stride()..layout.source_bits()).all(|i| !witness.bit(i))
            );
        }
    }

    #[test]
    fn shared_source_preserves_legacy_bits_and_authenticates_appended_public_keys() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let batch = 3;
        let shared = FalconSourceLayout::new_shared_prime(batch).unwrap();
        let legacy = FalconSourceLayout::new(batch).unwrap();
        let traces = vec![trace.clone(); batch];
        let messages = vec![MESSAGE.as_slice(); batch];
        let signatures = vec![SIGNATURE.as_slice(); batch];
        let old =
            FalconSourceWitness::from_traces(legacy, &messages, &signatures, &traces).unwrap();
        let mut new =
            FalconSourceWitness::from_traces(shared, &messages, &signatures, &traces).unwrap();
        let keys = vec![trace.public_key.clone(); batch];
        new.check_public_key_bindings(&keys).unwrap();
        let h = shared.public_key_offset().unwrap();
        for s in 0..batch {
            let base = s * shared.signature_stride();
            assert!((0..legacy.live_bits()).all(|j| old.bit(base + j) == new.bit(base + j)));
            for j in 0..N {
                assert_eq!(
                    read_unsigned(&new, base + h + 14 * j, 14),
                    u64::from(keys[s].h[j])
                );
            }
            assert!((shared.live_bits()..shared.signature_stride()).all(|j| !new.bit(base + j)));
        }
        assert!((batch * shared.signature_stride()..shared.source_bits()).all(|j| !new.bit(j)));
        // Decode H from the committed bits, rather than accepting its trace copy.
        let flat = shared.signature_stride() + h + 14 * 17 + 3;
        let row = flat & ((1 << shared.row_vars()) - 1);
        new.rows[flat >> shared.row_vars()][row / 64] ^= 1 << (row % 64);
        assert!(
            matches!(new.check_public_key_bindings(&keys), Err(FalconError::ConstraintViolation {
            family: "public-key-source", index,
        }) if index == N + 17)
        );
        assert!(old.check_public_key_bindings(&keys).is_err());
    }
}

}
