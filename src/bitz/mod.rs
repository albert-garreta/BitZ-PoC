//! BitZ transcript parity — `f2z-benchmark`'s clean-room implementation of
//! F2Z ("BitZ", protocol id `bitz/v1`), driven from this crate's field and
//! flock engine so the two produce the same narg string, hint stream and
//! challenge sequence on the same instance.
//!
//! The message and challenge order, the framing labels and the wire codecs
//! mirror `worldfnd/f2z-benchmark` at `0c75fd8` (branch `bitz-k4` for the
//! k = 4 Ligerito ladder) step for step; nothing here touches the crate's
//! own protocol, which stays byte-identical to what it was. Where the two
//! agree on the values — the GF(2^128) representation, the packed-witness
//! layout, the flock commit and the Ligerito engine — this module reuses
//! the crate's code; where they differ only in how the value is computed,
//! it follows their algorithm first (a per-layer grand-product GKR that
//! binds the in-tree variables MSB-first, a dense degree-2 reduction
//! sumcheck) so the transcript can be pinned before the fast kernels are
//! re-derived for their variable order.
//!
//! Layout, one file per protocol layer of theirs:
//! - [`transcript`]: the spongefish wrapper with the hint channel.
//! - [`params`]: shape and parameter gates, the linear claim, the frames.
//! - [`fold`]: step 3, the integer column folds and the batching point.
//! - [`gkr`]: step 4, the batched grand-product GKR.
//! - [`reduce`]: the reduction of the GKR exit claim to an inner product.
//! - [`sumcheck`]: the dense degree-2 sumcheck to one MLE claim.
//! - [`pcs`]: step 6, statement binding, ring switch and the Ligerito
//!   opening through a challenger that frames flock's events their way.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

pub mod codec;
pub mod fold;
pub mod forest;
pub mod gkr;
pub(crate) mod kernels;
pub mod params;
pub mod pcs;
pub mod reduce;
pub mod sumcheck;
pub mod transcript;

use crate::ligerito_flock::FlockCommitHint;
use crate::pcs::FixedBasePow;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

pub use params::{BitZParams, LinearClaim, ParamsError, Root, Shape, ShapeError};
pub use pcs::{OpeningQuery, Pcs, StatementBinding};
pub use transcript::{Proof, ProverState, VerifierState, build_prover, build_verifier};

/// Their comb window: `FixedBasePow` covers the full 128-bit exponent range
/// either way; the window only trades table size against multiplies.
pub const WINDOW: usize = 8;

/// The prover's derived setup: the parameters and the comb over their
/// generator.
pub struct BitZProver {
    params: BitZParams,
    comb: FixedBasePow,
}

/// The verifier's derived setup, the same two things.
pub struct BitZVerifier {
    params: BitZParams,
    comb: FixedBasePow,
}

/// A proof the prover cannot produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProveError {
    /// The bit rows are not the length the shape calls for.
    Witness,
    /// The fold round failed.
    Fold(fold::SendError),
    /// The derived GKR weight counts do not match the table shape.
    Reduction(params::ClaimError),
    /// The opening failed, so the reduction's claim was never discharged.
    Opening(pcs::ProveError),
}

/// A proof the verifier rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The fold round failed its own checks.
    Fold(fold::ReceiveError),
    /// The reduction failed.
    Reduction(reduce::ReduceError),
    /// The opening did not discharge the reduction's claim.
    Opening(pcs::VerifyError),
    /// A stream held bytes the protocol never read.
    TrailingData,
}

impl BitZProver {
    pub fn new(params: BitZParams, window: usize) -> Self {
        let comb = FixedBasePow::new(params.generator(), 128, window);
        Self { params, comb }
    }

    pub fn params(&self) -> &BitZParams {
        &self.params
    }

    pub(crate) fn comb(&self) -> &FixedBasePow {
        &self.comb
    }

    /// Their `BitZProver::prove`: bind the root and the parameters, fold,
    /// reduce through GKR, open.
    ///
    /// `rows` are the committed per-column bit rows (the layout
    /// [`crate::ligerito_flock::commit_rs_ligerito_rows`] takes), `hint` the
    /// commitment they produced under `pcs`.
    pub fn prove(
        &self,
        claim: &LinearClaim,
        pcs: &Pcs,
        hint: &FlockCommitHint,
        transcript: &mut ProverState,
    ) -> Result<(), ProveError> {
        let shape = *self.params.shape();
        let rows = hint.rows();
        if rows.len() != shape.columns()
            || rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(ProveError::Witness);
        }
        let root = Root(*hint.root());

        // Step 1: bind. The root and the parameters; the claim itself enters
        // through the caller's own events.
        transcript.public_message(&root.0);
        transcript.public_message(&self.params);

        // Steps 3 and 4: integer column folds, then GKR to a factored bit claim.
        let started = std::time::Instant::now();
        let fold = self
            .send_fold(claim, rows, transcript)
            .map_err(ProveError::Fold)?;
        trace("fold+images", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_prove(transcript, &fold, &shape, hint)
            .map_err(ProveError::Reduction)?;
        trace("gkr", started);

        // Step 6: inner-product sumcheck, ring switching, and the opening.
        let started = std::time::Instant::now();
        let result = pcs
            .prove_lin(hint, &query, StatementBinding::Bind, transcript)
            .map_err(ProveError::Opening);
        trace("opening (all)", started);
        result
    }
}

impl BitZVerifier {
    pub fn new(params: BitZParams, window: usize) -> Self {
        let comb = FixedBasePow::new(params.generator(), 128, window);
        Self { params, comb }
    }

    pub fn params(&self) -> &BitZParams {
        &self.params
    }

    pub(crate) fn comb(&self) -> &FixedBasePow {
        &self.comb
    }

    /// Their `BitZVerifier::verify`, consuming the transcript so both streams
    /// are checked for exhaustion here.
    pub fn verify(
        &self,
        claim: &LinearClaim,
        pcs: &Pcs,
        com: Root,
        mut transcript: VerifierState<'_>,
    ) -> Result<(), VerifyError> {
        transcript.public_message(&com.0);
        transcript.public_message(&self.params);

        let fold = self
            .receive_fold(claim, &mut transcript)
            .map_err(VerifyError::Fold)?;
        let query = reduce::gkr_reduce_verify(&mut transcript, &fold, self.params.shape())
            .map_err(VerifyError::Reduction)?;

        pcs.verify_lin(&com, &query, StatementBinding::Bind, &mut transcript)
            .map_err(VerifyError::Opening)?;

        transcript
            .check_eof()
            .map_err(|_| VerifyError::TrailingData)?;
        Ok(())
    }
}

/// Phase timing to stderr when `BITZ_TRACE` is set (diagnostic only).
pub(crate) fn trace(label: &str, started: std::time::Instant) {
    if std::env::var_os("BITZ_TRACE").is_some() {
        eprintln!("bitz: {label:<18} {:>9.1?}", started.elapsed());
    }
}

/// `eq(r, z)` for one coordinate pair, the way their `poly::eq::eq_eval`
/// computes it: `r·z + (1 − r)(1 − z)`.
pub(crate) fn eq_factor(r: Gf, z: Gf) -> Gf {
    let one = Gf::one();
    r * z + (one - r) * (one - z)
}
