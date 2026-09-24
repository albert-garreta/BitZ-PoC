//! Dyadic Logup cut for BitZ grand products.

mod chunks;
mod fraction;
mod inner_product;
mod layout;
mod pushforward;
mod protocol;
mod product;
mod structured;
mod table;
mod upper;

pub use chunks::{ChunkSpec, DyadicBlock, DyadicPlan, chunk_specs};
pub use fraction::{
    FractionClaim, FractionProofProfile, FractionTreeWitness, RationalClaims, RationalProof,
    prove_rational, prove_rational_profiled, verify_rational,
};
pub use inner_product::{EvalClaim, InnerProductProof, prove_inner_product, verify_inner_product};
pub use layout::{PackedBlock, PackedLayout};
pub use pushforward::{
    CutClaim, MergedCut, StructuredLinearClaim, TensorWeightTerm,
    derive_source_index_claim, eval_source_weight, eval_table_encoding, fill_pushforward,
    merge_cut_claims, source_layout,
};
pub use protocol::{
    LogupCutConfig, LogupCutError, LogupCutProfile, LogupCutProof, LogupCutProofSize,
    LogupCutScratch, logup_cut_proof_size, prove_logup_cut, prove_logup_cut_profiled,
    verify_logup_cut,
};
pub use product::ProductWorkspace;
pub use structured::{
    StructuredSumcheckProof, prove_structured_sumcheck, verify_structured_sumcheck,
};
pub use table::{ChunkProductTable, LogupCutEstimate, encode_table_row};
pub use upper::DyadicUpperProof;
pub(crate) use upper::{DyadicUpperProfile, prove_dyadic_upper, verify_dyadic_upper};

#[cfg(test)]
mod tests;
