//! # F2Z — an integer-MLE-evaluation PCS over an `F_2` commitment
//!
//! F2Z proves `MLE[INT(D)](r) = y ∈ F_q` for data `D` committed with a
//! cheap characteristic-2 (`F_2`-RAA Brakedown) commitment, by folding
//! the row variables **in the exponent** of `K = GF(2^128)`
//! (`α^{v_c} = ∏_b α^{w_b·D[(b,c)]}`, certified by a GKR grand-product
//! forest that touches the commitment only through `K`-linear queries)
//! and reading the column combination off in the clear over `F_q`.
//! Construction due to Lev Soukhanov (char2-fieldswitch §9); this crate
//! is the standalone extraction of the most-optimized implementation
//! from the `zinc-plus` repository (branch `f2-int-unified-ligerito`).
//!
//! Entry points: [`pcs::commit_bits`], [`pcs::prove_mle_eval_mod_q`],
//! [`pcs::verify_mle_eval_mod_q`] (and the plain integer-evaluation
//! pair [`pcs::prove`] / [`pcs::verify`], plus the batched variants).
//! See `docs/DESIGN.md` for the protocol and the optimization history.

pub mod code;
pub mod merkle;
pub mod pcs;
pub mod piop;
pub mod poly;
pub mod transcript;
pub mod utils;

pub use pcs::{
    IntEvalParams, commit_bits, prove, prove_mle_eval_mod_q, verify, verify_mle_eval_mod_q,
};
pub use poly::univariate::binary_gf128::BinaryFieldGF128;
