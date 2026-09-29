//! # BitZ — an integer-MLE-evaluation PCS over an `F_2` commitment
//!
//! BitZ proves `MLE[INT(D)](r) = y ∈ F_q` for data `D` committed over a
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
//! codec. Both bind their statement (the commitment, the Ligerito ladder, the
//! shape, the row weights, the prime width and `α`) before the first
//! challenge and refuse a Johnson-regime ladder without Round 0 (the `_with_ood`
//! variants run it); [`ligerito_flock::StandaloneModQOpening`] is the whole
//! standalone protocol the `bitz` CLI measures (transcript-sampled prime and
//! point, Round 0, the claim). See `docs/DESIGN.md` for the protocol and the
//! serialization format.
//!
//! ## F₂-virtualization
//!
//! Claims about a DERIVED vector `h = M·f` (a public canonical CSC
//! [`F₂`-linear map][circuit::linear_map::binary::PreparedVirtualMap] of the committed bits) are
//! opened against the commitment to `f` alone —
//! [`ligerito_flock::prove_mle_eval_mod_q_ligerito_virtual`] /
//! [`ligerito_flock::verify_mle_eval_mod_q_ligerito_virtual`]: the
//! synthesis supplies both `f` and `h`; per-chunk forests and pre-sumchecks
//! run on `h` without ever touching the oracle, the terminal claims are
//! transposed through `Mᵀ` at the
//! commitment field (XOR is addition in char 2), and the transposed
//! arbitrary-weight inner product is opened NATIVELY by the dual-basis
//! ring switch ([`dual_basis`], the paper's bilinear-embedding batching
//! protocol): a 128-element plane message `h_i`, one zero-evader `ρ`,
//! and ONE Ligerito call — no bridge sumcheck, no point opening. The
//! verifier's `M`-dependent cost is `O(L·nnz + #cols)` field ops. When `M`
//! is the identity on a shared row layout the opening
//! routes to the plain base path instead (the identity fast path,
//! `BITZ_VIRT_ID_FAST`), skipping the derived-vector machinery entirely.
//! [`piop::spartan::cm`] wires a full R1CS through this path — the
//! paper's CM relation: batched `x ∧ y = z` via one LINEAR constraint
//! per gate with `w = x ⊕ y` as a virtual (derived, uncommitted) block.

pub mod binary_pcs;
pub mod wfbitz;
#[cfg(feature = "binius64-bench")]
pub mod binius_ligerito;
pub mod dual_basis;
pub mod ext_proj;
pub mod f2map;
#[cfg(feature = "hybrid")]
pub mod hybrid;
pub mod ligerito;
pub mod ligerito_flock;
pub mod merged_forest;
#[cfg(feature = "span-metrics")]
pub mod observability;
pub mod pcs;
pub mod piop;
pub mod poly;
pub mod proof_codec;

pub mod transcript;
pub mod utils;
pub(crate) mod virt_batch;

pub use ligerito_flock::{
    FlockRsError, IntEvalRsLigModQProof, LigConfig, commit_rs_flock, commit_rs_flock_with,
    lig_configs, prove_mle_eval_mod_q_ligerito, verify_mle_eval_mod_q_ligerito,
};
// Round 0 of the paper's `c:core_iop` (the out-of-domain sample) and the
// standalone protocol the `bitz` CLI / bench run around it.
pub use ligerito_flock::{
    OodRound, OodRoundParams, StandaloneModQOpening, absorb_standalone_mod_q_claim,
    absorb_standalone_mod_q_statement, ood_round_bits, ood_round_params,
    prove_mle_eval_mod_q_ligerito_with_ood, sha_lig_ood_params,
    verify_mle_eval_mod_q_ligerito_runtime, verify_mle_eval_mod_q_ligerito_with_ood,
};
pub use circuit::linear_map::binary::{PreparedVirtualMap, PreparedVirtualMapError};
pub use pcs::IntegerMatrixLayout;
pub use poly::univariate::binary_b127::B127;
pub use poly::univariate::binary_gf128::Gf128;


pub mod sumcheck;
