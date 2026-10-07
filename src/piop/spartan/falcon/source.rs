use super::{
    BETA_SQUARED, FalconError, FalconSourceLayout, FalconVerificationTrace, HASH_TO_POINT_SAMPLES,
    N,
};
use super::{NORM_BITS, PREFIX_BITS};
use crate::piop::spartan::falcon_bit_layout::{COEFFICIENT_STRIDE, coefficient_bit};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Start offsets of committed columns within one signature stride.
/// Ring coefficients have stride 16; slack occupies spare coefficient lanes.
/// Internal holes and positions at or beyond `occupied_end` are zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceOffsets {
    pub shared_one: usize,
    pub message: usize,
    pub signature_header_nonce: usize,
    pub hash_words: usize,
    pub hash_quotients: usize,
    pub hash_remainders: usize,
    pub hash_accept_ands: usize,
    pub hash_prefixes: usize,
    pub hash_point: usize,
    pub s1: usize,
    pub s2: usize,
    pub public_key: usize,
    pub occupied_end: usize,
}

impl FalconSourceOffsets {
    pub(super) const fn new() -> Self {
        let counts = FalconSourceLayout::counts();
        let s1 = coefficient_bit(N, 0, 0, 0);
        let s2 = coefficient_bit(N, 1, 0, 0);
        let hash_point = coefficient_bit(N, 2, 0, 0);
        let public_key = coefficient_bit(N, 3, 0, 0);
        let shared_one = 4 * N * COEFFICIENT_STRIDE;
        let message = shared_one + counts.shared_one;
        let signature_header_nonce = message + counts.message;
        let hash_words = signature_header_nonce + counts.signature_header_nonce;
        let hash_quotients = hash_words + counts.hash_words;
        let hash_remainders = hash_quotients + counts.hash_quotients;
        let hash_accept_ands = hash_remainders + counts.hash_remainders;
        let hash_prefixes = hash_accept_ands + counts.hash_accept_ands;
        let occupied_end = hash_prefixes + counts.hash_prefixes;
        Self {
            shared_one,
            message,
            signature_header_nonce,
            hash_words,
            hash_quotients,
            hash_remainders,
            hash_accept_ands,
            hash_prefixes,
            hash_point,
            s1,
            s2,
            public_key,
            occupied_end,
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
    /// Header/nonce bytes use least-significant-bit-first byte bits. The exact
    /// signature payload is rearranged into aligned signed coefficient words.
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
        debug_assert_eq!(offsets.occupied_end, layout.occupied_bits());

        crate::utils::cfg_chunks_mut!(rows, layout.signature_stride() >> p.row_vars)
            .enumerate()
            .take(layout.batch())
            .try_for_each(|(instance, rows)| -> Result<(), FalconError> {
                let signature = signatures[instance];
                let message = messages[instance];
                let trace = &traces[instance];

                if signature.len() != super::CT_SIGNATURE_BYTES {
                    return Err(FalconError::SignatureLength {
                        expected: super::CT_SIGNATURE_BYTES,
                    });
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

                for (byte_index, &byte) in signature[..1 + super::NONCE_BYTES].iter().enumerate() {
                    put_unsigned(
                        rows,
                        &p,
                        offsets.signature_header_nonce + 8 * byte_index,
                        u64::from(byte),
                        8,
                    );
                }
                // Read the supplied bytes, not the trace's decoded s2 copy:
                // the commitment must continue to bind the exact signature.
                let mut payload = signature[1 + super::NONCE_BYTES..].iter().copied();
                let mut acc = 0u32;
                let mut acc_len = 0;
                for i in 0..N {
                    while acc_len < super::SIGNATURE_BITS {
                        acc = (acc << 8) | u32::from(payload.next().expect("length checked"));
                        acc_len += 8;
                    }
                    acc_len -= super::SIGNATURE_BITS;
                    let word = (acc >> acc_len) & ((1 << super::SIGNATURE_BITS) - 1);
                    put_unsigned(
                        rows,
                        &p,
                        layout.s2_bit(i, 0),
                        u64::from(word),
                        super::SIGNATURE_BITS,
                    );
                    acc &= (1 << acc_len) - 1;
                }
                debug_assert_eq!(acc_len, 0);
                debug_assert!(payload.next().is_none());

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
                        u64::from(prefix),
                        PREFIX_BITS,
                    );
                }

                for i in 0..N {
                    put_unsigned(
                        rows,
                        &p,
                        layout.hash_point_bit(i, 0),
                        u64::from(trace.hash_to_point.point[i]),
                        14,
                    );
                    let biased_s1 = i64::from(trace.s1[i]) + 6_144;
                    put_unsigned(
                        rows,
                        &p,
                        layout.s1_bit(i, 0),
                        u64::from(bounded14_encode(biased_s1 as u16)),
                        14,
                    );
                }
                debug_assert_eq!(trace.norm + trace.norm_slack, BETA_SQUARED);
                assert!(trace.norm_slack < 1u64 << NORM_BITS);
                for bit in 0..NORM_BITS {
                    put_unsigned(
                        rows,
                        &p,
                        layout.norm_slack_bit(bit),
                        (trace.norm_slack >> bit) & 1,
                        1,
                    );
                }
                for (i, &coefficient) in trace.public_key.h.iter().enumerate() {
                    if i64::from(coefficient) >= super::Q {
                        return Err(FalconError::PublicKeyCoefficient { index: i });
                    }
                    put_unsigned(
                        rows,
                        &p,
                        layout.public_key_bit(i, 0),
                        u64::from(coefficient),
                        14,
                    );
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

    #[cfg(test)]
    pub(super) fn flip_bit(&mut self, flat: usize) {
        let p = self.layout.bitz_params();
        let row = flat & (p.rows() - 1);
        self.rows[flat >> p.row_vars][row / 64] ^= 1 << (row % 64);
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
    use crate::piop::spartan::falcon_profiles::n1024_k11::verification_trace;

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
                    let actual = (0..8).fold(0u8, |value, bit| {
                        value | (u8::from(witness.bit(base + layout.signature_bit(i, bit))) << bit)
                    });
                    assert_eq!(actual, byte);
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
                        read(layout.hash_point_bit(i, 0), 14),
                        u64::from(trace.hash_to_point.point[i])
                    );
                    let encoded = read(layout.s1_bit(i, 0), 14) as u16;
                    assert_eq!(
                        i32::from(bounded14_decode(encoded)) - 6_144,
                        i32::from(trace.s1[i])
                    );
                    let s2 = read(layout.s2_bit(i, 0), super::super::SIGNATURE_BITS);
                    let signed_s2 = s2 as i64
                        - ((s2 >> (super::super::SIGNATURE_BITS - 1)) << super::super::SIGNATURE_BITS) as i64;
                    assert_eq!(signed_s2, i64::from(trace.signature.s2[i]));
                }
                let slack = (0..NORM_BITS).fold(0u64, |value, bit| {
                    value | (u64::from(witness.bit(base + layout.norm_slack_bit(bit))) << bit)
                });
                assert_eq!(slack, trace.norm_slack);
                assert!((0..layout.signature_stride()).filter(|&i| layout.is_padding(i)).all(|i| !witness.bit(base + i)));
            }
            assert!(
                (batch * layout.signature_stride()..layout.source_bits()).all(|i| !witness.bit(i))
            );
        }
    }

    #[test]
    fn source_authenticates_public_keys_and_zero_padding() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let batch = 3;
        let shared = FalconSourceLayout::new(batch).unwrap();
        let traces = vec![trace.clone(); batch];
        let messages = vec![MESSAGE.as_slice(); batch];
        let signatures = vec![SIGNATURE.as_slice(); batch];
        let mut new =
            FalconSourceWitness::from_traces(shared, &messages, &signatures, &traces).unwrap();
        let keys = vec![trace.public_key.clone(); batch];
        new.check_public_key_bindings(&keys).unwrap();
        for s in 0..batch {
            let base = s * shared.signature_stride();
            for j in 0..N {
                assert_eq!(
                    read_unsigned(&new, base + shared.public_key_bit(j, 0), 14),
                    u64::from(keys[s].h[j])
                );
            }
            assert!((0..shared.signature_stride()).filter(|&j| shared.is_padding(j)).all(|j| !new.bit(base + j)));
        }
        assert!((batch * shared.signature_stride()..shared.source_bits()).all(|j| !new.bit(j)));
        // Decode H from the committed bits, rather than accepting its trace copy.
        let flat = shared.signature_stride() + shared.public_key_bit(17, 3);
        let row = flat & ((1 << shared.row_vars()) - 1);
        new.rows[flat >> shared.row_vars()][row / 64] ^= 1 << (row % 64);
        assert!(
            matches!(new.check_public_key_bindings(&keys), Err(FalconError::ConstraintViolation {
            family: "public-key-source", index,
        }) if index == N + 17)
        );
    }

    #[test]
    fn source_reconstructs_supplied_signature_bytes_even_when_trace_differs() {
        let trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let bytes: Vec<u8> = (0..super::super::CT_SIGNATURE_BYTES)
            .map(|i| (i.wrapping_mul(73) ^ (i >> 3)) as u8)
            .collect();
        let source = FalconSourceWitness::from_traces(
            layout,
            &[MESSAGE.as_slice()],
            &[bytes.as_slice()],
            &[trace],
        )
        .unwrap();
        for (byte, &expected) in bytes.iter().enumerate() {
            let actual = (0..8).fold(0u8, |value, bit| {
                value | (u8::from(source.bit(layout.signature_bit(byte, bit))) << bit)
            });
            assert_eq!(actual, expected, "signature byte {byte}");
        }
    }

    #[test]
    fn aligned_source_preserves_coefficient_boundaries() {
        let mut trace = verification_trace(PUBLIC_KEY, MESSAGE, SIGNATURE).unwrap();
        let s1 = [-6_144, -2_047, 0, 2_047, 2_048, 6_144];
        let s2 = [-2_047, -1, 0, 1, 2_046, 2_047];
        let unsigned = [0, 1, 4_096, 4_097, 8_191, 12_288];
        trace.s1[..s1.len()].copy_from_slice(&s1);
        trace.signature.s2[..s2.len()].copy_from_slice(&s2);
        trace.public_key.h[..unsigned.len()].copy_from_slice(&unsigned);
        trace.hash_to_point.point[..unsigned.len()].copy_from_slice(&unsigned);
        let signature = super::super::encode_signature_ct(&trace.signature).unwrap();
        let layout = FalconSourceLayout::new(1).unwrap();
        let source = FalconSourceWitness::from_traces(
            layout,
            &[MESSAGE.as_slice()],
            &[signature.as_slice()],
            &[trace],
        )
        .unwrap();
        for j in 0..s1.len() {
            // Independent literal slot arithmetic checks the public helpers too.
            let encoded_s1 = read_unsigned(&source, 16 * j, 14) as u16;
            assert_eq!(i32::from(bounded14_decode(encoded_s1)) - 6_144, i32::from(s1[j]));
            let encoded_s2 = read_unsigned(&source, 16 * (N + j), 12) as i16;
            assert_eq!((encoded_s2 << 4) >> 4, s2[j]);
            assert_eq!(read_unsigned(&source, 16 * (2 * N + j), 14), u64::from(unsigned[j]));
            assert_eq!(read_unsigned(&source, 16 * (3 * N + j), 14), u64::from(unsigned[j]));
        }
    }
}

}
