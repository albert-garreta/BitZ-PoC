//! Their `spartan` crate over [`super::fq::Fq`]: the R1CS matrices lowered
//! to the field with their canonical digest ([`matrix`]), the two sumcheck
//! provers and the reusable verifier ([`sumcheck`]), their composition and
//! the out-of-band proof's byte form ([`piop`]).

pub mod matrix;
pub mod piop;
pub mod poly;
pub mod sumcheck;

pub use matrix::{
    CompactMatrix, CompactMatrixBuilder, ESCAPE, IntegerCoefficient, MapRows, MatrixError,
    PreparedConstraintMatrices, PreparedIntegerMatrices, compact_from_vendored, residue,
};
pub use piop::{
    ScaledMleEvaluationClaim, SpartanError, SpartanPiopProof, prove_spartan_piop,
    prove_spartan_piop_absorbed, prove_spartan_piop_sampled, verify_spartan_proof,
    verify_spartan_proof_absorbed, verify_spartan_proof_sampled,
};
pub use poly::{eq_eval, eq_table};
pub use sumcheck::{
    InnerSumcheckOutput, InnerWitness, OuterSumcheckOutput, OuterSumcheckProof, Products,
    SumcheckError, SumcheckProof, mle_evaluate, prove_inner_sumcheck, prove_outer_sumcheck,
};
