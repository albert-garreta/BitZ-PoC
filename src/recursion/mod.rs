//! BitZ's verifier as a circuit in BitZ's own integer-R1CS + F2-virtual
//! relation (the class the SHA-256+ECDSA relation lives in), for recursion.
//!
//! The circuit verifies proofs of the `sha256-fs` protocol variant: every
//! transcript is the SHA-256 compression duplex ([`crate::transcript::sha`]),
//! Merkle trees and proof of work hash with SHA-256, so the circuit is
//! SHA-256 compressions (one shared block template,
//! [`circuit::sha256::compress_block`]) plus linear checks.
//!
//! Every Fiat–Shamir challenge is a PUBLIC input of the circuit ([`val`]):
//! the in-circuit transcript recomputes it from the committed proof and
//! asserts equality, and all verifier work that depends only on public
//! values (eq tables, row images, matrix and virtual-map evaluations,
//! residual bases) is computed natively when the circuit is built. What
//! remains in the circuit is the hashing of proof data and linear checks
//! over it, plus the few products of two proof values.
//!
//! Design follows the flock-recursion line's `flockrec` crate (public
//! challenges, settled linear combinations).

pub mod arith;
pub mod bitz_pcs;
pub mod bitz_transcript;
pub mod bytes;
pub mod ecdsa_verifier;
#[cfg(all(feature = "falcon-hybrid", feature = "bench-internals"))]
pub mod falcon_opening;
#[cfg(all(feature = "falcon-hybrid", feature = "bench-internals"))]
pub mod falcon_verifier;
pub mod flockrec_verifier;
pub mod ligerito;
pub mod merkle;
pub mod sha;
pub mod spartan;
pub mod val;

#[cfg(test)]
mod tests;
