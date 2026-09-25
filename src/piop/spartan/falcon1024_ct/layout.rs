use super::{FalconError, HASH_TO_POINT_SAMPLES, N};
use crate::pcs::IntegerMatrixLayout;

/// Fixed-width source columns used by the optimized Falcon relation.
/// Counts are per signature and deliberately include only committed bits;
/// copies and reconstructions are handled by the terminal linear binder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconTraceCounts {
    pub shared_one: usize,
    pub message: usize,
    pub encoded_signature: usize,
    pub s2_non_min_slack: usize,
    pub keccak_chi_inputs: usize,
    pub keccak_chi_ands: usize,
    pub keccak_round_states: usize,
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
}

impl FalconTraceCounts {
    /// Total committed source bits for one verification.
    pub const fn total(self) -> usize {
        self.shared_one
            + self.message
            + self.encoded_signature
            + self.s2_non_min_slack
            + self.keccak_chi_inputs
            + self.keccak_chi_ands
            + self.keccak_round_states
            + self.keccak_parity_quotients
            + self.hash_quotients
            + self.hash_remainders
            + self.hash_remainder_slack
            + self.hash_quotient_slack
            + self.hash_accept_ands
            + self.hash_prefixes
            + self.compact_selectors
            + self.compact_selected_prefixes
            + self.compact_selected_remainders
            + self.hash_point
            + self.s1
            + self.s1_range_slack
            + self.ring_quotients
            + self.norm_slack
    }
}

/// Auditable packed layout for batches of one through 32 signatures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconSourceLayout {
    batch: usize,
    capacity: usize,
}

impl FalconSourceLayout {
    pub const SIGNATURE_STRIDE: usize = 1 << 23;

    pub fn new(batch: usize) -> Result<Self, FalconError> {
        if !(1..=32).contains(&batch) {
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
        // log2(2^23 * capacity) - row_vars.
        10 + self.capacity.trailing_zeros() as usize
    }

    /// Physical `W=1` source-commitment tensor.  Flat source index `i` is
    /// stored at `b = i mod 2^t`, `c = floor(i/2^t)`.
    pub const fn bitz_params(&self) -> IntegerMatrixLayout {
        IntegerMatrixLayout {
            row_vars: self.row_vars(),
            col_vars: self.col_vars(),
            word_bits: 1,
        }
    }

    pub const fn counts() -> FalconTraceCounts {
        FalconTraceCounts {
            shared_one: 1,
            message: 32 * 8,
            // Complete CT signature: header, nonce and 12-bit s2 payload.
            encoded_signature: 1_577 * 8,
            // `sum(low eleven bits) = sign + d`, `d in [0,11]`.
            s2_non_min_slack: N * 4,
            // B lanes entering every chi layer.
            keccak_chi_inputs: 20 * 24 * 25 * 64,
            // 20 permutations * 24 rounds * 25 lanes * 64 bits.
            keccak_chi_ands: 20 * 24 * 25 * 64,
            // Sparse round checkpoints, after chi and iota.
            keccak_round_states: 20 * 24 * 25 * 64,
            // Three-bit exact parity quotient for every theta/rho/pi output.
            keccak_parity_quotients: 20 * 24 * 25 * 64 * 3,
            hash_quotients: HASH_TO_POINT_SAMPLES * 3,
            hash_remainders: HASH_TO_POINT_SAMPLES * 14,
            hash_remainder_slack: HASH_TO_POINT_SAMPLES * 14,
            hash_quotient_slack: HASH_TO_POINT_SAMPLES * 3,
            hash_accept_ands: HASH_TO_POINT_SAMPLES,
            // P_0..P_1311, each at most 1311.
            hash_prefixes: (HASH_TO_POINT_SAMPLES + 1) * 11,
            // `a_i * (1 - msb(P_i))` selects the first 1024 accepts.
            compact_selectors: HASH_TO_POINT_SAMPLES,
            // Products with the selector make the product-tree leaves linear.
            compact_selected_prefixes: HASH_TO_POINT_SAMPLES * 11,
            compact_selected_remainders: HASH_TO_POINT_SAMPLES * 14,
            // Stable-compaction output, tied to the candidates by GKR.
            hash_point: N * 14,
            // Centered values in [-6144,6144].
            s1: N * 14,
            // `12288 - (s1 + 6144)` proves the biased encoding is canonical.
            s1_range_slack: N * 14,
            // Safe signed width for the schoolbook negacyclic quotient.
            ring_quotients: N * 23,
            norm_slack: 27,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_layout_fits_eight_megabit_stride() {
        let counts = FalconSourceLayout::counts();
        assert_eq!(counts.keccak_chi_ands, 768_000);
        assert_eq!(counts.keccak_chi_inputs, 768_000);
        assert_eq!(counts.keccak_round_states, 768_000);
        assert_eq!(counts.keccak_parity_quotients, 2_304_000);
        assert!(counts.total() < FalconSourceLayout::SIGNATURE_STRIDE);
        assert_eq!(counts.total(), 4_785_959);
        for batch in 1..=32 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            assert_eq!(
                layout.row_vars() + layout.col_vars(),
                layout.source_bits().trailing_zeros() as usize
            );
        }
    }
}
