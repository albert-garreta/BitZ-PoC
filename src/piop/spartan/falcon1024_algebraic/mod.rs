//! Non-hiding proofs of the Falcon-1024 ring equation and exact integer norm.
//!
//! Public inputs are canonical polynomials `h` and `t`. The caller is responsible
//! for connecting `t` to a message and nonce. Both signature components are
//! witnesses, authenticated by one BitZ commitment; this module generates no
//! SHAKE or HashToPoint trace.

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

pub const N: usize = 1024;
pub const Q: i64 = 12_289;
pub const BETA_SQUARED: u64 = 70_265_242;
pub const LIVE_BITS: usize = 2 * N * 15 + 27;
pub const PROTOCOL_ID: &str = "bitz/falcon1024-algebraic/non-zk/v1";
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
