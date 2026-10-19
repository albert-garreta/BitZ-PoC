mod arithmetic;
mod compact;
#[cfg(any(test, feature = "bench-internals"))]
mod factored_ring;
#[cfg(any(test, feature = "bench-internals"))]
mod keyed_ring;
mod protocol;
mod ring;
mod source;

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod packing_bench;

pub use super::falcon::FalconError;
pub use protocol::{
    CommittedFalconAlgebraic, FalconAlgebraicProof, FalconAlgebraicSecurity,
    PreparedFalconAlgebraic,
};
pub use source::{FalconAlgebraicStatement, FalconAlgebraicWitness, check_algebraic_statement};

use super::{SpartanBitzField, SpartanField};
use source::{Layout, Source, WitnessData};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

pub const N: usize = DEGREE;
pub const COEFFICIENT_LOG: usize = N.ilog2() as usize;
pub const PARAMETERS: super::falcon_parameters::FalconParameters =
    super::falcon_parameters::FalconParameters::for_degree(N);
pub const SLACK_BITS: usize = PARAMETERS.norm_bits();
pub const Q: i64 = 12_289;
pub const BETA_SQUARED: u64 = PARAMETERS.norm_bound;
/// Live value bits per signature in the original signed15 representation.
pub const LIVE_BITS: usize = 2 * N * 15 + SLACK_BITS;
/// Maximum live value bits per signature in compact split-source commitments.
/// Public exception-index metadata is counted separately.
pub const COMPACT_LIVE_BITS: usize =
    2 * N * compact::BASE_BITS + 3 * compact::MAX_EXCEPTIONS + SLACK_BITS;

/// Public choice of proof relation, shared by prover and verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FalconAlgebraicBinding {
    /// Bit-domain binder over the compact commitment, including partial batches.
    Bits,
    /// Word-domain binder with a merged source opening, including partial batches.
    Words,
}

/// Identifier of the default word-binding relation. Use `prepared.protocol_id()`
/// for the relation selected by a prepared prover or verifier.
pub const PROTOCOL_ID: &str = if N == 1024 {
    "bitz/falcon1024-algebraic/non-zk/v9"
} else {
    "bitz/falcon512-algebraic/non-zk/v9"
};
const BIT_PROTOCOL_ID: &str = if N == 1024 {
    "bitz/falcon1024-algebraic/non-zk/v8"
} else {
    "bitz/falcon512-algebraic/non-zk/v8"
};
const PRIME_MIN: u128 = 1 << 125;
const PRIME_MAX: u128 = (1 << 126) - 1;

#[derive(Clone, Copy, Debug)]
struct Schedule {
    norm_instance_bits: u32,
    norm_round_bits: u32,
    merge_bits: u32,
    binding_bits: u32,
    word_binding: bool,
    terminal_merge_bits: u32,
    /// Prove the per-signature square sums as a cubic term of the word binder
    /// (over the instance variables) instead of a separate integer-outer
    /// sumcheck. Requires `word_binding`.
    merged_norm: bool,
    /// Grinding difficulty of the merged binder's cubic instance rounds; its
    /// quadratic coefficient rounds keep `binding_bits`.
    cubic_binding_bits: u32,
}

fn error(e: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(e.to_string())
}
