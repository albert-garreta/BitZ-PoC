mod arithmetic;
mod protocol;
mod ring;
mod source;

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
pub const LIVE_BITS: usize = 2 * N * 15 + SLACK_BITS;
pub const PROTOCOL_ID: &str = if N == 1024 {
    "bitz/falcon1024-algebraic/non-zk/v4"
} else {
    "bitz/falcon512-algebraic/non-zk/v4"
};
const PRIME_MIN: u128 = 1 << 125;
const PRIME_MAX: u128 = (1 << 126) - 1;

#[derive(Clone, Copy, Debug)]
struct Schedule {
    norm_instance_bits: u32,
    norm_round_bits: u32,
    merge_bits: u32,
    binding_bits: u32,
}

fn error(e: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(e.to_string())
}
