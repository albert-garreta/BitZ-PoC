//! Their `spartan` crate over [`super::fq::Fq`]: the R1CS matrices lowered
//! to the field with their canonical digest ([`matrix`]), the two sumcheck
//! provers and the reusable verifier ([`sumcheck`]), their composition and
//! the out-of-band proof's byte form ([`piop`]).

pub mod matrix;
pub mod piop;
pub mod poly;
pub mod sumcheck;

pub use matrix::{MatrixError, PreparedConstraintMatrices, SparseMatrix, residue};
pub use piop::{
    ScaledMleEvaluationClaim, SpartanError, SpartanPiopProof, prove_spartan_piop,
    verify_spartan_proof,
};
pub use poly::{eq_eval, eq_table};
pub use sumcheck::{
    InnerSumcheckOutput, OuterSumcheckOutput, OuterSumcheckProof, Products, SumcheckError,
    SumcheckProof, mle_evaluate, prove_inner_sumcheck, prove_outer_sumcheck,
};
