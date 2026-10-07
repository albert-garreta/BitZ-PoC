use super::NORM_BITS;
use super::{FalconError, HASH_TO_POINT_SAMPLES, N};
use crate::pcs::IntegerMatrixLayout;
use crate::piop::spartan::falcon_bit_layout::{COEFFICIENT_STRIDE, coefficient_bit, slack_bit};

/// Fixed-width committed arithmetic columns for one Falcon signature.
/// Binary Keccak has its own source; the binder authenticates all reconstructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconTraceCounts {
    pub shared_one: usize,
    pub message: usize,
    pub signature_header_nonce: usize,
    pub s2: usize,
    pub hash_words: usize,
    pub hash_quotients: usize,
    pub hash_remainders: usize,
    pub hash_accept_ands: usize,
    pub hash_point: usize,
    pub s1: usize,
    pub norm_slack: usize,
    pub public_key: usize,
}

impl FalconTraceCounts {
    pub const fn total(self) -> usize {
        self.shared_one
            + self.message
            + self.signature_header_nonce
            + self.s2
            + self.hash_words
            + self.hash_quotients
            + self.hash_remainders
            + self.hash_accept_ands
            + self.hash_point
            + self.s1
            + self.norm_slack
            + self.public_key
    }
}

/// Compact arithmetic source shared by the native ideal and norm proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceLayout {
    batch: usize,
    capacity: usize,
}

impl FalconSourceLayout {
    pub const SIGNATURE_STRIDE: usize = {
        let bits = Self::counts().total().next_power_of_two();
        // The shared PCS needs at least 2^9 packed GF(2^128) words.
        if bits < (1 << 16) { 1 << 16 } else { bits }
    };

    /// Supports 1..=1024 signatures, padded to the next power of two.
    pub fn new(batch: usize) -> Result<Self, FalconError> {
        if !(1..=1024).contains(&batch) {
            return Err(FalconError::InvalidBatchCapacity);
        }
        if super::FalconSourceOffsets::new().occupied_end > Self::SIGNATURE_STRIDE {
            return Err(FalconError::SourceStrideOverflow);
        }
        Ok(Self {
            batch,
            capacity: batch.next_power_of_two(),
        })
    }

    /// Base of unsigned public-key coefficients, spaced 16 positions apart.
    pub const fn public_key_offset(&self) -> usize {
        self.offsets().public_key
    }

    pub const fn live_bits(&self) -> usize {
        Self::counts().total()
    }

    /// End of the occupied source extent, including internal padding holes.
    pub const fn occupied_bits(&self) -> usize {
        self.offsets().occupied_end
    }

    pub const fn s1_bit(&self, index: usize, bit: usize) -> usize {
        coefficient_bit(N, 0, index, bit)
    }

    pub const fn s2_bit(&self, index: usize, bit: usize) -> usize {
        coefficient_bit(N, 1, index, bit)
    }

    pub const fn hash_point_bit(&self, index: usize, bit: usize) -> usize {
        coefficient_bit(N, 2, index, bit)
    }

    pub const fn public_key_bit(&self, index: usize, bit: usize) -> usize {
        coefficient_bit(N, 3, index, bit)
    }

    pub const fn norm_slack_bit(&self, bit: usize) -> usize {
        slack_bit(bit)
    }

    /// Local address of an LSB-indexed bit of the exact CT signature bytes.
    /// Payload coefficients are MSB-first on the wire and LSB-first in source.
    pub const fn signature_bit(&self, byte: usize, bit: usize) -> usize {
        if byte < 1 + super::NONCE_BYTES {
            self.offsets().signature_header_nonce + 8 * byte + bit
        } else {
            let stream = 8 * (byte - 1 - super::NONCE_BYTES) + (7 - bit);
            self.s2_bit(
                stream / super::SIGNATURE_BITS,
                super::SIGNATURE_BITS - 1 - stream % super::SIGNATURE_BITS,
            )
        }
    }

    /// Whether a local position must be zero for an active signature.
    pub const fn is_padding(&self, local: usize) -> bool {
        if local >= self.occupied_bits() {
            return true;
        }
        if local >= 4 * N * COEFFICIENT_STRIDE {
            return false;
        }
        let polynomial = local / (N * COEFFICIENT_STRIDE);
        let index = local / COEFFICIENT_STRIDE % N;
        let bit = local % COEFFICIENT_STRIDE;
        if polynomial == 0 && bit == 15 && index < NORM_BITS {
            return false;
        }
        let width = if polynomial == 1 {
            super::SIGNATURE_BITS
        } else {
            14
        };
        bit >= width
    }

    pub const fn signature_stride(&self) -> usize {
        Self::SIGNATURE_STRIDE
    }
    pub const fn offsets(&self) -> super::FalconSourceOffsets {
        super::FalconSourceOffsets::new()
    }
    pub const fn linear_rows(&self) -> usize {
        super::FalconConstraintCounts::per_signature().linear_rows()
    }
    pub const fn linear_stride(&self) -> usize {
        self.linear_rows().next_power_of_two()
    }
    pub const fn batch(&self) -> usize {
        self.batch
    }
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
    pub const fn source_bits(&self) -> usize {
        Self::SIGNATURE_STRIDE * self.capacity
    }
    pub const fn row_vars(&self) -> usize {
        13
    }
    pub const fn col_vars(&self) -> usize {
        Self::SIGNATURE_STRIDE.ilog2() as usize - 13 + self.capacity.trailing_zeros() as usize
    }

    /// Flat source index i is stored at row i mod 2^13, column floor(i/2^13).
    pub const fn bitz_params(&self) -> IntegerMatrixLayout {
        IntegerMatrixLayout {
            row_vars: self.row_vars(),
            col_vars: self.col_vars(),
        }
    }

    pub const fn counts() -> FalconTraceCounts {
        FalconTraceCounts {
            shared_one: 1,
            message: 32 * 8,
            signature_header_nonce: (1 + super::NONCE_BYTES) * 8,
            s2: N * super::SIGNATURE_BITS,
            hash_words: HASH_TO_POINT_SAMPLES * 16,
            hash_quotients: HASH_TO_POINT_SAMPLES * 3,
            // bounded14 decodes every bit string into 0..=12288.
            hash_remainders: HASH_TO_POINT_SAMPLES * 14,
            hash_accept_ands: HASH_TO_POINT_SAMPLES,
            hash_point: N * 14,
            // bounded14 value minus 6144, shared by norms and the native ideal.
            s1: N * 14,
            norm_slack: NORM_BITS,
            public_key: 14 * N,
        }
    }
}

#[cfg(test)]
mod profile_layout_tests {
    use super::*;

    #[test]
    fn aligned_addresses_cover_exactly_the_live_bits_for_each_profile() {
        let layout = FalconSourceLayout::new(3).unwrap();
        let offsets = layout.offsets();
        let mut used = vec![false; layout.signature_stride()];
        let mut mark = |address: usize| {
            assert!(
                !std::mem::replace(&mut used[address], true),
                "aliased source bit {address}"
            );
        };
        for j in 0..N {
            for bit in 0..14 {
                mark(layout.s1_bit(j, bit));
                mark(layout.hash_point_bit(j, bit));
                mark(layout.public_key_bit(j, bit));
            }
            for bit in 0..super::super::SIGNATURE_BITS {
                mark(layout.s2_bit(j, bit));
            }
        }
        for bit in 0..NORM_BITS {
            mark(layout.norm_slack_bit(bit));
        }
        for bit in offsets.shared_one..offsets.occupied_end {
            mark(bit);
        }
        assert_eq!(used.iter().filter(|&&bit| bit).count(), layout.live_bits());
        for (local, &live) in used.iter().enumerate() {
            assert_eq!(live, !layout.is_padding(local), "padding mask at {local}");
        }
        assert_eq!(
            layout.signature_stride(),
            if N == 512 { 65_536 } else { 131_072 }
        );
        assert_eq!(
            layout.occupied_bits(),
            if N == 512 { 57_731 } else { 110_695 }
        );
        assert!(layout.is_padding(layout.occupied_bits()));
    }

    #[test]
    fn ct_payload_mapping_is_a_bijection_onto_signed_coefficient_bits() {
        let layout = FalconSourceLayout::new(1).unwrap();
        let mut mapped = vec![false; layout.signature_stride()];
        for byte in 0..super::super::CT_SIGNATURE_BYTES {
            for bit in 0..8 {
                let address = layout.signature_bit(byte, bit);
                assert!(!std::mem::replace(&mut mapped[address], true));
                assert!(!layout.is_padding(address));
            }
        }
        for j in 0..N {
            for bit in 0..16 {
                assert_eq!(
                    mapped[16 * (N + j) + bit],
                    bit < super::super::SIGNATURE_BITS
                );
            }
        }
        let header = layout.offsets().signature_header_nonce;
        for bit in 0..8 * (1 + super::super::NONCE_BYTES) {
            assert!(mapped[header + bit]);
        }
        assert_eq!(
            mapped.iter().filter(|&&bit| bit).count(),
            super::super::CT_SIGNATURE_BYTES * 8
        );
    }
}
falcon_tests! {
mod tests {
    use super::*;

    #[test]
    fn source_layout_has_only_required_arithmetic_columns() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let counts = FalconSourceLayout::counts();
            assert_eq!(counts.total(), 100_482);
            assert_eq!(layout.occupied_bits(), 110_695);
            assert_eq!(layout.occupied_bits() - counts.total(), 10_213);
            assert_eq!(layout.signature_stride(), 131_072);
            assert_eq!(layout.capacity(), batch.next_power_of_two());
            assert_eq!(layout.source_bits(), 131_072 * layout.capacity());
            assert_eq!(layout.linear_rows(), 6_504);
            assert_eq!(layout.linear_stride(), 8_192);
            assert_eq!(layout.row_vars(), 13);
            assert_eq!(
                layout.row_vars() + layout.col_vars(),
                layout.source_bits().trailing_zeros() as usize
            );
            assert_eq!(
                counts.shared_one + counts.message + counts.signature_header_nonce + counts.s2,
                12_873
            );
            assert_eq!(counts.total() - 12_873, 87_609);
            assert_eq!(layout.public_key_offset(), 49_152);
            assert_eq!(counts.public_key, 14 * N);
            assert_eq!(counts.hash_accept_ands, HASH_TO_POINT_SAMPLES);
        }
        assert!(FalconSourceLayout::new(0).is_err());
        assert!(FalconSourceLayout::new(1025).is_err());
    }
}

}
