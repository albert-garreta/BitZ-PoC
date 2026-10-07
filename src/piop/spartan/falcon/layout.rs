use super::{FalconError, HASH_TO_POINT_SAMPLES, N};
use super::{NORM_BITS, PREFIX_BITS};
use crate::pcs::IntegerMatrixLayout;

/// Fixed-width committed arithmetic columns for one Falcon signature.
/// Binary Keccak has its own source; the binder authenticates all reconstructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconTraceCounts {
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
    pub public_key: usize,
}

impl FalconTraceCounts {
    pub const fn total(self) -> usize {
        self.shared_one
            + self.message
            + self.encoded_signature
            + self.hash_words
            + self.hash_quotients
            + self.hash_remainders
            + self.hash_accept_ands
            + self.hash_prefixes
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
        if Self::counts().total() > Self::SIGNATURE_STRIDE {
            return Err(FalconError::SourceStrideOverflow);
        }
        Ok(Self {
            batch,
            capacity: batch.next_power_of_two(),
        })
    }

    /// Unsigned little-endian public-key coefficients within each signature.
    pub const fn public_key_offset(&self) -> usize {
        self.offsets().public_key
    }

    pub const fn live_bits(&self) -> usize {
        Self::counts().total()
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
            // Complete CT signature: header, nonce and signed 12-bit s2 payload.
            encoded_signature: super::CT_SIGNATURE_BYTES * 8,
            hash_words: HASH_TO_POINT_SAMPLES * 16,
            hash_quotients: HASH_TO_POINT_SAMPLES * 3,
            // bounded14 decodes every bit string into 0..=12288.
            hash_remainders: HASH_TO_POINT_SAMPLES * 14,
            hash_accept_ands: HASH_TO_POINT_SAMPLES,
            hash_prefixes: (HASH_TO_POINT_SAMPLES + 1) * PREFIX_BITS,
            hash_point: N * 14,
            // bounded14 value minus 6144, shared by norms and the native ideal.
            s1: N * 14,
            norm_slack: NORM_BITS,
            public_key: 14 * N,
        }
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
            assert_eq!(counts.total(), 114_914);
            assert_eq!(layout.offsets().end, counts.total());
            assert_eq!(layout.signature_stride(), 131_072);
            assert_eq!(layout.capacity(), batch.next_power_of_two());
            assert_eq!(layout.source_bits(), 131_072 * layout.capacity());
            assert_eq!(layout.linear_rows(), 5_482);
            assert_eq!(layout.linear_stride(), 8_192);
            assert_eq!(layout.row_vars(), 13);
            assert_eq!(
                layout.row_vars() + layout.col_vars(),
                layout.source_bits().trailing_zeros() as usize
            );
            assert_eq!(
                counts.shared_one + counts.message + counts.encoded_signature,
                12_873
            );
            assert_eq!(counts.total() - 12_873, 102_041);
            assert_eq!(layout.public_key_offset(), 100_578);
            assert_eq!(counts.public_key, 14 * N);
            assert_eq!(
                counts.hash_words / 16
                    + counts.hash_quotients / 3
                    + counts.hash_remainders / 14
                    + counts.hash_accept_ands
                    + counts.hash_prefixes / 11
                    + counts.hash_point / 14
                    + counts.s1 / 14
                    + counts.norm_slack / 27,
                8_605
            );
        }
        assert!(FalconSourceLayout::new(0).is_err());
        assert!(FalconSourceLayout::new(1025).is_err());
    }
}

}
