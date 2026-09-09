//! One padded SHA-256 message and one P-256 verification, sharing a bit oracle.
//!
//! Linear rows bypass the nonlinear outer sumcheck and join its matrix claims
//! in one packed inner sumcheck. The terminal scaled claim is opened via F2Z.

mod codec;
mod proof;
mod reduction;
mod relation;
mod security;
mod witness;

#[cfg(test)]
mod tests;

pub use proof::{Sha256EcdsaProof, commit_sha256_ecdsa, prove_sha256_ecdsa, verify_sha256_ecdsa};
pub use relation::{OuterMode, PreparedSha256Ecdsa, Sha256EcdsaStatement, prepare_sha256_ecdsa};
pub use security::{ChallengeBudget, Sha256EcdsaSecurity};
pub use witness::{Sha256EcdsaWitness, generate_sha256_ecdsa_witness};

use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
#[error("SHA-256/ECDSA: {0}")]
pub struct Sha256EcdsaError(pub String);

pub(crate) type Result<T> = std::result::Result<T, Sha256EcdsaError>;
pub(crate) fn error(value: impl Display) -> Sha256EcdsaError {
    Sha256EcdsaError(value.to_string())
}
