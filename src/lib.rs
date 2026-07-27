//! # F2Z — an integer-MLE-evaluation PCS over an `F_2` commitment
//!
//! F2Z proves `MLE[INT(D)](r) = y ∈ F_q` for data `D` committed over a
//! cheap characteristic-2 (`F_2`) code, by folding the row variables **in
//! the exponent** of `K = GF(2^128)` (`α^{v_c} = ∏_b α^{w_b·D[(b,c)]}`,
//! certified by a GKR grand-product forest that touches the commitment only
//! through `K`-linear queries) and reading the column combination off in the
//! clear over `F_q`. Construction due to Lev Soukhanov (char2-fieldswitch §9);
//! this crate is the standalone extraction of the most-optimized
//! implementation from the `zinc-plus` repository (branch
//! `f2-int-unified-ligerito`).
//!
//! ## Opener
//!
//! The **only** PCS opener is the flock-backed **ring-switch + recursive
//! Ligerito** pipeline ([`ligerito_flock`]): the committed bit-matrix is
//! packed 128 bits per `GF(2^128)` element and RS-encoded/Merkleized by
//! [`flock-core`](flock_core); each mod-`q` limb chunk contributes a
//! [merged product forest][merged_forest] + a de-black-boxing pre-sumcheck,
//! and the L chunk claims are `η`-batched into ONE recursive Ligerito call
//! whose closing residual is evaluated succinctly by the ring-switch
//! tensor-algebra ([`ligerito::tensor_eq_phi_eval`]).
//!
//! Entry points: [`ligerito_flock::commit_rs_flock`] /
//! [`ligerito_flock::commit_rs_flock_with`],
//! [`ligerito_flock::prove_mle_eval_mod_q_ligerito`],
//! [`ligerito_flock::verify_mle_eval_mod_q_ligerito`], with the proof object
//! [`ligerito_flock::IntEvalRsLigModQProof`] and its
//! [`to_bytes`][ligerito_flock::IntEvalRsLigModQProof::to_bytes] /
//! [`from_bytes`][ligerito_flock::IntEvalRsLigModQProof::from_bytes] host
//! codec. See `docs/DESIGN.md` for the protocol and the serialization format.

pub mod ligerito;
pub mod ligerito_flock;
pub mod merged_forest;
pub mod pcs;
pub mod piop;
pub mod poly;
pub mod proof_codec;
pub mod taps;
pub mod transcript;
pub mod utils;

pub use ligerito_flock::{
    FlockRsError, IntEvalRsLigModQProof, LigConfig, commit_rs_flock, commit_rs_flock_with,
    lig_configs, prove_mle_eval_mod_q_ligerito, verify_mle_eval_mod_q_ligerito,
};
// EXPERIMENTAL — mod-q RLC claim families (docs/rlc-family-note-prompt.md):
// k claims on F₂-linear forms of j committed columns via ONE γ-RLC forest
// per chunk + a degree-(j+1) monomial discharge. No proof_codec wiring.
pub use ligerito_flock::{
    IntEvalRsLigRlcFamilyProof, RlcFamilyClaim, mle_eval_mod_q_lig_rlc_family_proof_size_bytes,
    prove_mle_eval_mod_q_ligerito_rlc_family, verify_mle_eval_mod_q_ligerito_rlc_family,
};
// EXPERIMENTAL — structured-tap virtual claims (ROT/SHIFT/entry-offset
// taps; docs/rlc-structured-taps-phase0.md): tapped rows through the
// batched x-forest with translated-eq committed openings.
pub use ligerito_flock::{
    IntEvalRsLigModQTapProof, TapClaim, TapVerifyClaim, mle_eval_mod_q_lig_tap_size_breakdown,
    prove_mle_eval_mod_q_ligerito_tap_claims, verify_mle_eval_mod_q_ligerito_tap_claims,
};
pub use ligerito_flock::{
    IntEvalRsLigTapFamilyProof, TapFamilyCluster, TapFamilyClusterSide,
    mle_eval_mod_q_lig_tap_family_size_breakdown, prove_mle_eval_mod_q_ligerito_tap_family,
    verify_mle_eval_mod_q_ligerito_tap_family,
};
pub use pcs::IntEvalParams;
pub use poly::univariate::binary_b127::BinaryFieldB127;
pub use poly::univariate::binary_gf128::BinaryFieldGF128;
pub use taps::{TapOp, extract_virtual_tap_rows};
