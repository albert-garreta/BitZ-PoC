//! Shared sumcheck proofs, transcript boundaries, and equality-weighted outer reductions.
pub mod boundary;
mod error;
pub mod outer;
pub mod proof;
pub use boundary::{RoundBoundaryPolicy, UngrindedRoundBoundary};
pub use error::SumcheckError;
pub use proof::SumcheckProof;
pub mod arithmetic;
