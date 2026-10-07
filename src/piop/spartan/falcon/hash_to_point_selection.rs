//! Canonical public selection masks for ordered HashToPoint compaction.
//!
//! A mask has exactly N set bits. Their increasing positions select the output
//! coefficients. Scalar rows bind every acceptance decision through the last
//! selected candidate; quadratic rows bind rejection bits to quotient bits.

use super::{FalconError, FalconVerificationTrace, HASH_TO_POINT_SAMPLES, N};

pub(super) const MASK_BYTES: usize = HASH_TO_POINT_SAMPLES.div_ceil(8);
pub(super) const REJECTION_ROWS: usize = HASH_TO_POINT_SAMPLES;
pub(super) const REJECTION_STRIDE: usize = REJECTION_ROWS.next_power_of_two();
pub(super) const REJECTION_ROW_LOG: usize = REJECTION_STRIDE.ilog2() as usize;

/// Validated masks and their deterministic increasing-index decoder. The raw
/// masks belong to the proof; this cache is derived, never separately trusted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Selection {
    masks: Vec<Vec<u8>>,
    indices: Vec<Box<[u16; N]>>,
}

impl Selection {
    pub fn from_masks(masks: Vec<Vec<u8>>, batch: usize) -> Result<Self, FalconError> {
        if !(1..=1024).contains(&batch) || masks.len() != batch {
            return Err(FalconError::InvalidBatchCapacity);
        }
        let mut indices = Vec::with_capacity(batch);
        for (instance, mask) in masks.iter().enumerate() {
            if mask.len() != MASK_BYTES
                || (HASH_TO_POINT_SAMPLES % 8 != 0
                    && mask[MASK_BYTES - 1] >> (HASH_TO_POINT_SAMPLES % 8) != 0)
            {
                return invalid_mask(instance);
            }
            let mut selected = Vec::with_capacity(N);
            for j in 0..HASH_TO_POINT_SAMPLES {
                if mask[j / 8] >> (j % 8) & 1 != 0 {
                    selected.push(j as u16);
                }
            }
            if selected.len() != N {
                return invalid_mask(instance);
            }
            indices.push(
                selected
                    .into_boxed_slice()
                    .try_into()
                    .expect("checked count"),
            );
        }
        Ok(Self { masks, indices })
    }

    pub fn from_traces(traces: &[FalconVerificationTrace]) -> Result<Self, FalconError> {
        let mut masks = Vec::with_capacity(traces.len());
        for trace in traces {
            let hash = &trace.hash_to_point;
            let mut mask = vec![0; MASK_BYTES];
            let mut count = 0;
            for (j, &accepted) in hash.accepted.iter().enumerate() {
                if accepted && count < N {
                    if hash.remainders[j] != hash.point[count] {
                        return Err(FalconError::ConstraintViolation {
                            family: "hash-selection-witness",
                            index: j,
                        });
                    }
                    mask[j / 8] |= 1 << (j % 8);
                    count += 1;
                }
            }
            if count != N {
                return Err(FalconError::HashToPointUnderflow { accepted: count });
            }
            masks.push(mask);
        }
        Self::from_masks(masks, traces.len())
    }

    pub fn masks(&self) -> &[Vec<u8>] {
        &self.masks
    }

    pub fn into_masks(self) -> Vec<Vec<u8>> {
        self.masks
    }

    pub fn batch(&self) -> usize {
        self.masks.len()
    }

    pub fn indices(&self, instance: usize) -> &[u16; N] {
        &self.indices[instance]
    }

    #[cfg(test)]
    pub fn selected(&self, instance: usize, candidate: usize) -> bool {
        assert!(candidate < HASH_TO_POINT_SAMPLES);
        self.masks[instance][candidate / 8] >> (candidate % 8) & 1 != 0
    }

    /// Last selected position, inclusive. Rows after it impose no selection.
    pub fn cutoff(&self, instance: usize) -> usize {
        usize::from(self.indices[instance][N - 1])
    }
}

fn invalid_mask<T>(instance: usize) -> Result<T, FalconError> {
    Err(FalconError::ConstraintViolation {
        family: "hash-selection-mask",
        index: instance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(indices: impl IntoIterator<Item = usize>) -> Vec<u8> {
        let mut mask = vec![0; MASK_BYTES];
        for j in indices {
            mask[j / 8] |= 1 << (j % 8);
        }
        mask
    }

    #[test]
    fn masks_decode_in_order_including_first_and_last_draw() {
        let masks = vec![
            mask(0..N),
            mask(HASH_TO_POINT_SAMPLES - N..HASH_TO_POINT_SAMPLES),
        ];
        let selected = Selection::from_masks(masks.clone(), 2).unwrap();
        assert_eq!(selected.batch(), 2);
        assert_eq!(selected.masks(), masks);
        assert_eq!(selected.cutoff(0), N - 1);
        assert_eq!(selected.cutoff(1), HASH_TO_POINT_SAMPLES - 1);
        for instance in 0..2 {
            assert!(
                selected
                    .indices(instance)
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            );
            for &j in selected.indices(instance) {
                assert!(selected.selected(instance, usize::from(j)));
            }
        }
        assert_eq!(selected.into_masks(), masks);
    }

    #[test]
    fn masks_reject_wrong_counts_lengths_and_noncanonical_tail_bits() {
        assert!(Selection::from_masks(vec![mask(0..N - 1)], 1).is_err());
        assert!(Selection::from_masks(vec![mask(0..N + 1)], 1).is_err());
        assert!(Selection::from_masks(vec![mask(0..N)], 2).is_err());
        assert!(Selection::from_masks(Vec::new(), 0).is_err());
        for len in [0, MASK_BYTES - 1, MASK_BYTES + 1] {
            let mut malformed = mask(0..N);
            malformed.resize(len, 0);
            assert!(Selection::from_masks(vec![malformed], 1).is_err());
        }
        let mut tail = mask(0..N);
        tail[MASK_BYTES - 1] |= 1 << (HASH_TO_POINT_SAMPLES % 8);
        assert!(Selection::from_masks(vec![tail], 1).is_err());
    }
}
