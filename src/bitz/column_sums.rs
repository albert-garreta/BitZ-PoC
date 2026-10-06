//! Inline integer fold values and their explicit, padding-free encoding.

use field::DecodeError;

/// Two independently bounded sums. For a split at bit `w`, the represented
/// integer is `lower + 2^w * upper`. Folding preserves both components without
/// carrying between them; their separate values are authenticated by the PCS.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LargeNumber {
    pub lower: u128,
    pub upper: u32,
}

impl LargeNumber {
    pub const ENCODED_BYTES: usize = 20;

    pub fn checked_new(lower: u128, upper: u128) -> Option<Self> {
        Some(Self {
            lower,
            upper: u32::try_from(upper).ok()?,
        })
    }

    pub fn to_le_bytes(self) -> [u8; Self::ENCODED_BYTES] {
        let mut bytes = [0; Self::ENCODED_BYTES];
        bytes[..16].copy_from_slice(&self.lower.to_le_bytes());
        bytes[16..].copy_from_slice(&self.upper.to_le_bytes());
        bytes
    }

    pub fn from_le_bytes(bytes: [u8; Self::ENCODED_BYTES]) -> Self {
        Self {
            lower: u128::from_le_bytes(bytes[..16].try_into().expect("16-byte lower sum")),
            upper: u32::from_le_bytes(bytes[16..].try_into().expect("4-byte upper sum")),
        }
    }

    pub fn within(self, bound: Self) -> bool {
        self.lower <= bound.lower && self.upper <= bound.upper
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u128 {}
    impl Sealed for super::LargeNumber {}
}

/// Storage used by the shared integer folding kernel. Callers establish that
/// the sum of all nonnegative weights fits each component before folding.
/// Consequently every digit-table entry and parallel partial sum also fits.
pub trait FoldValue: sealed::Sealed + Copy + Send + Sync {
    const ZERO: Self;
    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn wrapping_add(self, rhs: Self) -> Self;
}

impl FoldValue for u128 {
    const ZERO: Self = 0;
    #[inline]
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }
}

impl FoldValue for LargeNumber {
    const ZERO: Self = Self { lower: 0, upper: 0 };
    #[inline]
    fn checked_add(self, rhs: Self) -> Option<Self> {
        Some(Self {
            lower: self.lower.checked_add(rhs.lower)?,
            upper: self.upper.checked_add(rhs.upper)?,
        })
    }
    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        Self {
            lower: self.lower.wrapping_add(rhs.lower),
            upper: self.upper.wrapping_add(rhs.upper),
        }
    }
}

/// A table of inline column values. The variant is selected by the prepared
/// protocol, not by an untrusted tag in the encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnSums {
    Unsplit(Vec<u128>),
    Split(Vec<LargeNumber>),
}

/// The caller supplies the expected representation and the actual sum bounds.
#[derive(Clone, Copy, Debug)]
pub enum ColumnSumBounds {
    Unsplit(u128),
    Split(LargeNumber),
}

impl ColumnSums {
    pub fn len(&self) -> usize {
        match self {
            Self::Unsplit(sums) => sums.len(),
            Self::Split(sums) => sums.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn lower_encoded_len(&self) -> usize {
        16 * self.len()
    }

    pub fn upper_encoded_len(&self) -> usize {
        match self {
            Self::Unsplit(_) => 0,
            Self::Split(sums) => 4 * sums.len(),
        }
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            Self::Unsplit(sums) => 16 * sums.len(),
            Self::Split(sums) => LargeNumber::ENCODED_BYTES * sums.len(),
        }
    }

    /// Column-major bytes, without a tag or length prefix. This is a transport
    /// encoding; protocols may bind a different, legacy transcript encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.encoded_len());
        match self {
            Self::Unsplit(sums) => {
                for sum in sums {
                    bytes.extend_from_slice(&sum.to_le_bytes());
                }
            }
            Self::Split(sums) => {
                for sum in sums {
                    bytes.extend_from_slice(&sum.to_le_bytes());
                }
            }
        }
        bytes
    }

    /// Reject wrong lengths before allocating and reject out-of-bound sums.
    /// The expected column count and bounds must come from prepared parameters
    /// and verifier-derived row weights, never from the incoming bytes.
    pub fn decode(
        bytes: &[u8],
        columns: usize,
        bounds: ColumnSumBounds,
    ) -> Result<Self, DecodeError> {
        let width = match bounds {
            ColumnSumBounds::Unsplit(_) => 16,
            ColumnSumBounds::Split(_) => LargeNumber::ENCODED_BYTES,
        };
        let expected = columns
            .checked_mul(width)
            .ok_or(DecodeError::NonCanonical)?;
        if bytes.len() != expected {
            return Err(DecodeError::Length {
                expected,
                actual: bytes.len(),
            });
        }
        match bounds {
            ColumnSumBounds::Unsplit(bound) => bytes
                .as_chunks::<16>()
                .0
                .iter()
                .map(|bytes| {
                    let value = u128::from_le_bytes(*bytes);
                    if value <= bound {
                        Ok(value)
                    } else {
                        Err(DecodeError::NonCanonical)
                    }
                })
                .collect::<Result<_, _>>()
                .map(Self::Unsplit),
            ColumnSumBounds::Split(bound) => bytes
                .as_chunks::<{ LargeNumber::ENCODED_BYTES }>()
                .0
                .iter()
                .map(|bytes| {
                    let value = LargeNumber::from_le_bytes(*bytes);
                    if value.within(bound) {
                        Ok(value)
                    } else {
                        Err(DecodeError::NonCanonical)
                    }
                })
                .collect::<Result<_, _>>()
                .map(Self::Split),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_is_explicit_and_bounds_are_checked() {
        let bound = LargeNumber {
            lower: (1u128 << 126) - 8192,
            upper: (1u32 << 26) - 8192,
        };
        let split = ColumnSums::Split(vec![
            LargeNumber::ZERO,
            LargeNumber {
                lower: 0x0102,
                upper: 0x0304,
            },
            bound,
        ]);
        let bytes = split.encode();
        assert_eq!(bytes.len(), 60);
        assert_eq!(bytes.len(), split.encoded_len());
        assert_eq!(&bytes[20..22], &[2, 1]);
        assert_eq!(&bytes[36..40], &[4, 3, 0, 0]);
        assert_eq!(
            ColumnSums::decode(&bytes, 3, ColumnSumBounds::Split(bound)).unwrap(),
            split
        );
        assert!(ColumnSums::decode(&bytes[..59], 3, ColumnSumBounds::Split(bound)).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(ColumnSums::decode(&trailing, 3, ColumnSumBounds::Split(bound)).is_err());
        assert!(ColumnSums::decode(&bytes, usize::MAX, ColumnSumBounds::Split(bound)).is_err());
        for invalid in [
            LargeNumber {
                lower: bound.lower + 1,
                ..bound
            },
            LargeNumber {
                upper: bound.upper + 1,
                ..bound
            },
        ] {
            assert!(
                ColumnSums::decode(&invalid.to_le_bytes(), 1, ColumnSumBounds::Split(bound))
                    .is_err()
            );
        }
        let unsplit = ColumnSums::Unsplit(vec![0, bound.lower]);
        assert_eq!(
            ColumnSums::decode(&unsplit.encode(), 2, ColumnSumBounds::Unsplit(bound.lower))
                .unwrap(),
            unsplit
        );
        assert!(
            ColumnSums::decode(
                &unsplit.encode(),
                2,
                ColumnSumBounds::Unsplit(bound.lower - 1)
            )
            .is_err()
        );
        assert!(ColumnSums::decode(&bytes, 3, ColumnSumBounds::Unsplit(u128::MAX)).is_err());
    }

    #[test]
    fn components_never_carry_into_each_other() {
        let max = LargeNumber {
            lower: u128::MAX,
            upper: u32::MAX,
        };
        assert_eq!(LargeNumber::from_le_bytes(max.to_le_bytes()), max);
        assert!(LargeNumber::checked_new(0, u128::from(u32::MAX) + 1).is_none());
        assert!(
            max.checked_add(LargeNumber { lower: 1, upper: 0 })
                .is_none()
        );
        assert!(
            max.checked_add(LargeNumber { lower: 0, upper: 1 })
                .is_none()
        );
        assert_eq!(
            LargeNumber {
                lower: (1u128 << 113) - 1,
                upper: 7
            }
            .checked_add(LargeNumber { lower: 1, upper: 0 }),
            Some(LargeNumber {
                lower: 1u128 << 113,
                upper: 7
            })
        );
        assert!(!std::mem::needs_drop::<LargeNumber>());
        assert_eq!(
            ColumnSums::Split(vec![LargeNumber::ZERO; 16384])
                .encode()
                .len(),
            327680
        );
    }
}
