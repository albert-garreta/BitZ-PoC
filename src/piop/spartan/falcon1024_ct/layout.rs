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
    }
}

/// Compact arithmetic source shared by the native ideal and norm proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceLayout {
    batch: usize,
    capacity: usize,
    shared_prime: bool,
}

impl FalconSourceLayout {
    pub const SIGNATURE_STRIDE: usize = {
        let bits = (Self::counts().total() + 14 * N).next_power_of_two();
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
            shared_prime: false,
        })
    }

    /// Opt-in source view for the shared-prime ring reduction. The public-key
    /// coefficients follow the legacy columns without changing their offsets.
    pub fn new_shared_prime(batch: usize) -> Result<Self, FalconError> {
        let mut layout = Self::new(batch)?;
        layout.shared_prime = true;
        if layout.live_bits() > Self::SIGNATURE_STRIDE {
            return Err(FalconError::SourceStrideOverflow);
        }
        Ok(layout)
    }

    pub const fn is_shared_prime(&self) -> bool {
        self.shared_prime
    }

    /// Unsigned, little-endian 14-bit public-key coefficients in each signature.
    pub const fn public_key_offset(&self) -> Option<usize> {
        if self.shared_prime {
            Some(Self::counts().total())
        } else {
            None
        }
    }

    pub const fn live_bits(&self) -> usize {
        Self::counts().total() + if self.shared_prime { 14 * N } else { 0 }
    }

    pub const fn signature_stride(&self) -> usize {
        Self::SIGNATURE_STRIDE
    }
    pub const fn offsets(&self) -> super::FalconSourceOffsets {
        super::FalconSourceOffsets::new(self.shared_prime)
    }
    pub const fn linear_rows(&self) -> usize {
        super::FalconConstraintCounts::for_layout(self).linear_rows()
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
        }
    }
}
falcon_tests! {
mod tests {
    use super::*;

    #[test]
    fn shared_prime_layout_appends_only_public_key_bits() {
        for batch in 1..=1024 {
            let legacy = FalconSourceLayout::new(batch).unwrap();
            let shared = FalconSourceLayout::new_shared_prime(batch).unwrap();
            assert!(!legacy.is_shared_prime());
            assert!(shared.is_shared_prime());
            assert_eq!(legacy.public_key_offset(), None);
            assert_eq!(shared.public_key_offset(), Some(100_578));
            assert_eq!(shared.offsets().public_key, shared.public_key_offset());
            assert_eq!(legacy.live_bits(), 100_578);
            assert_eq!(shared.live_bits(), 114_914);
            assert_eq!(shared.offsets().end, shared.live_bits());
            assert_eq!(shared.linear_rows(), 5_482);
            assert!(shared.linear_rows() < shared.linear_stride());
            assert!(shared.live_bits() < shared.signature_stride());
            assert_eq!(shared.source_bits(), legacy.source_bits());
            assert_eq!(shared.row_vars(), legacy.row_vars());
            assert_eq!(shared.col_vars(), legacy.col_vars());
            let mut shared_offsets = shared.offsets();
            shared_offsets.public_key = None;
            shared_offsets.end = legacy.offsets().end;
            assert_eq!(shared_offsets, legacy.offsets());
        }
        assert!(FalconSourceLayout::new_shared_prime(0).is_err());
        assert!(FalconSourceLayout::new_shared_prime(1025).is_err());
    }

    #[test]
    fn source_layout_has_only_required_arithmetic_columns() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let counts = FalconSourceLayout::counts();
            assert_eq!(counts.total(), 100_578);
            assert_eq!(layout.offsets().end, counts.total());
            assert_eq!(layout.signature_stride(), 131_072);
            assert_eq!(layout.capacity(), batch.next_power_of_two());
            assert_eq!(layout.source_bits(), 131_072 * layout.capacity());
            assert_eq!(layout.linear_rows(), 4_458);
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
            assert_eq!(counts.total() - 12_873, 87_705);
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
