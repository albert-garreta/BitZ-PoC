//! BitZ proves integer linear claims over a committed binary witness.
//!
//! [`wfbitz`] is the integer PCS: exponent folds and a product GKR reduce
//! each claim to a binary inner product, followed by ring switching and
//! Flock's recursive Ligerito opening. Parameters enforce the exact exponent
//! bound, canonical residues, and a full-order generator before proving.
//!
//! [`wfbitz::virt`] opens derived bits through a public F2-linear map;
//! [`wfbitz::chained`] supplies structured reductions with dense fallbacks.
//! [`ligerito_flock`] provides shared commitments, validated ladders, statement
//! framing, and Round 0. [`ligerito`] contains shared packing and ring-switch
//! kernels, also used by the joint [`binary_pcs`].
//!
//! Native and Flock challenge schedules remain separate. The relation's
//! security profile determines authenticated grinding before protected draws;
//! execution and security reports use the same native schedule.

pub mod binary_pcs;
#[cfg(feature = "binius64-bench")]
pub mod binius_ligerito;
pub mod f2map;
#[cfg(feature = "hybrid")]
pub mod hybrid;
pub mod ligerito;
pub mod ligerito_flock;
#[cfg(feature = "span-metrics")]
pub mod observability;
pub mod pcs;
pub mod piop;
pub mod poly;
pub mod prime_sampling;
pub mod proof_codec;
pub mod wfbitz;

pub mod transcript;
pub mod utils;

pub use ligerito_flock::{FlockRsError, LigConfig, commit_rs_flock_with, lig_configs};
// Round 0 of the paper's `c:core_iop` (the out-of-domain sample) and the
// standalone protocol the `bitz` CLI / bench run around it.
pub use circuit::linear_map::binary::{PreparedVirtualMap, PreparedVirtualMapError};
pub use ligerito_flock::{
    OodRound, OodRoundParams, absorb_standalone_mod_q_claim, absorb_standalone_mod_q_statement,
    ood_round_bits, ood_round_params, sha_lig_ood_params,
};
pub use pcs::IntegerMatrixLayout;
pub use poly::univariate::binary_b127::B127;
pub use poly::univariate::binary_gf128::Gf128;

pub mod sumcheck;
