//! Addressing shared by the aligned Falcon coefficient sources.

pub(crate) const COEFFICIENT_LOG: usize = 4;
pub(crate) const COEFFICIENT_STRIDE: usize = 1 << COEFFICIENT_LOG;

/// Local bit address in a polynomial-major, coefficient-major table.
pub(crate) const fn coefficient_bit(
    degree: usize,
    polynomial: usize,
    index: usize,
    bit: usize,
) -> usize {
    (polynomial * degree + index) * COEFFICIENT_STRIDE + bit
}

/// Slack bits occupy the spare final lane of the first polynomial's rows.
pub(crate) const fn slack_bit(bit: usize) -> usize {
    bit * COEFFICIENT_STRIDE + COEFFICIENT_STRIDE - 1
}
