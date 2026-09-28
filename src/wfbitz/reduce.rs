//! Their grand-product reduction (step 4) to a factored inner-product claim
//! over the committed bits: each leaf is `1 + (y_row − 1)·bit(column, row)`,
//! and at GKR's terminal point subtracting one leaves
//! `Σ u1[row]·u2[column]·bit(column, row)`.

use super::fold::Fold;
use super::forest::{Forest, NibbleRows};
use super::gkr::gpgkr_verify;
use super::params::{ClaimError, LinearClaimGf, Shape};
use super::pcs::OpeningQuery;
use super::transcript::{ProverState, VerifierState};
use super::LeafProtocol;
use crate::ligerito_flock::FlockCommitHint;
use field::Gf128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;

/// A reduction the verifier rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceError {
    /// The GKR replay failed.
    GKR,
    /// The derived weight counts do not match the table shape.
    Claim(ClaimError),
}

fn eq_table(point: &[Gf]) -> Vec<Gf> {
    build_eq_x_r_vec(point, &()).expect("non-empty point")
}

/// Splits the terminal point into column and row coordinates (columns occupy
/// the low index bits) and derives the two weight factors.
fn query_from_terminal(
    fold: &Fold,
    shape: &Shape,
    point: Vec<Gf>,
    claim: Gf,
) -> Result<OpeningQuery, ClaimError> {
    let (u1, alfa_c, inner_product_claim) = exit_from_terminal(fold, point, claim);
    let u2 = eq_table(&alfa_c);
    let claim = LinearClaimGf::from_shape(shape, u1, u2, inner_product_claim)?;
    Ok(OpeningQuery::InnerProduct { claim })
}

/// The GKR's exit split the way a composition consumes it: the row-bit
/// weights `(A(b) − 1)·eq(b, α_b)`, the column point `α_c` and the
/// inner-product target `claim − 1` (the multilinear extension of the
/// constant-one table is one everywhere). GKR's terminal point is
/// `[α_c (r2 column vars) | α_b (r1 row vars)]`.
pub(crate) fn exit_from_terminal(fold: &Fold, mut point: Vec<Gf>, claim: Gf) -> (Vec<Gf>, Vec<Gf>, Gf) {
    let inner_product_claim = claim - Gf::one();
    let r1 = fold.row_images.len().max(1).ilog2() as usize;
    let r2 = point.len() - r1;
    let alfa_b = point.split_off(r2);
    let alfa_c = point;
    let u1: Vec<Gf> = fold
        .row_images
        .iter()
        .zip(eq_table(&alfa_b))
        .map(|(a, b)| (*a - Gf::one()) * b)
        .collect();
    (u1, alfa_c, inner_product_claim)
}

/// The fold's GKR over any grid's packed columns, its exit in the
/// composition's form (see [`exit_from_terminal`]).
pub(crate) fn gkr_exit_prove(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    packed_cols: &[Vec<u64>],
) -> (Vec<Gf>, Vec<Gf>, Gf) {
    let forest = Forest::new(
        shape.log_rows(),
        shape.log_columns(),
        packed_cols,
        &fold.row_images,
    );
    let (point, claim) = forest.prove(transcript, &fold.zeta);
    exit_from_terminal(fold, point, claim)
}

/// [`gkr_exit_prove`]'s verifier.
pub(crate) fn gkr_exit_verify(
    transcript: &mut VerifierState<'_>,
    fold: &Fold,
    shape: &Shape,
) -> Result<(Vec<Gf>, Vec<Gf>, Gf), ReduceError> {
    let (point, claim) = gpgkr_verify(transcript, fold.e0, &fold.zeta, shape.log_rows() as u32)
        .ok_or(ReduceError::GKR)?;
    Ok(exit_from_terminal(fold, point, claim))
}

pub(crate) fn gkr_reduce_prove(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    hint: &FlockCommitHint,
) -> Result<OpeningQuery, ClaimError> {
    gkr_reduce_prove_packed(transcript, fold, shape, hint.packed_cols())
}

/// [`gkr_reduce_prove`] with the committed grid's nibble rows already built
/// ([`super::forest::nibble_rows_for`]).
pub(crate) fn gkr_reduce_prove_with(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    hint: &FlockCommitHint,
    nibble: Option<NibbleRows>,
    protocol: LeafProtocol,
) -> Result<OpeningQuery, ClaimError> {
    let mut forest = Forest::new(shape.log_rows(), shape.log_columns(), hint.packed_cols(), &fold.row_images);
    if let Some(nibble) = nibble {
        forest = forest.with_nibble_rows(nibble);
    }
    query_from_forest(transcript, fold, shape, forest, protocol)
}

/// [`gkr_reduce_prove`] over any grid's 64-lane packed columns
/// (`packed_cols[g][b]` = bit `b` of columns `64g..64g+63`): the committed
/// rows' packing, or a derived grid's for a virtual opening.
pub(crate) fn gkr_reduce_prove_packed(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    packed_cols: &[Vec<u64>],
) -> Result<OpeningQuery, ClaimError> {
    gkr_reduce_prove_packed_with(transcript, fold, shape, packed_cols, LeafProtocol::Sequential)
}

pub(crate) fn gkr_reduce_prove_packed_with(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    packed_cols: &[Vec<u64>],
    protocol: LeafProtocol,
) -> Result<OpeningQuery, ClaimError> {
    let forest = Forest::new(
        shape.log_rows(),
        shape.log_columns(),
        packed_cols,
        &fold.row_images,
    );
    query_from_forest(transcript, fold, shape, forest, protocol)
}

fn query_from_forest(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    forest: Forest<'_>,
    protocol: LeafProtocol,
) -> Result<OpeningQuery, ClaimError> {
    match protocol {
        LeafProtocol::Sequential => {
            let (point, claim) = forest.prove(transcript, &fold.zeta);
            query_from_terminal(fold, shape, point, claim)
        }
        LeafProtocol::Skip4 => {
            let (binding, claim) = forest.prove_skipping(transcript, &fold.zeta);
            query_from_skipped(fold, shape, binding, claim)
        }
        LeafProtocol::Skip3 => {
            let (binding, claim) = forest.prove_skipping_with(transcript, &fold.zeta, 3);
            query_from_skipped(fold, shape, binding, claim)
        }
    }
}

fn query_from_skipped(
    fold: &Fold,
    shape: &Shape,
    binding: super::leaf_skip::LeafBinding,
    claim: Gf,
) -> Result<OpeningQuery, ClaimError> {
    let row_weights = binding.row_weights(&fold.row_images, shape.log_columns())
        .ok_or(ClaimError::RowWeightCountMismatch)?;
    let column_point = &binding.suffix_point[..shape.log_columns()];
    // The one-column case has no column coordinates.
    let columns = super::gkr::eq_table(column_point);
    let claim = LinearClaimGf::from_shape(shape, row_weights, columns, claim - Gf::one())?;
    Ok(OpeningQuery::InnerProduct { claim })
}

pub(crate) fn gkr_reduce_verify(
    transcript: &mut VerifierState<'_>,
    fold: &Fold,
    shape: &Shape,
) -> Result<OpeningQuery, ReduceError> {
    gkr_reduce_verify_with(transcript, fold, shape, LeafProtocol::Sequential)
}

pub(crate) fn gkr_reduce_verify_with(
    transcript: &mut VerifierState<'_>,
    fold: &Fold,
    shape: &Shape,
    protocol: LeafProtocol,
) -> Result<OpeningQuery, ReduceError> {
    match protocol {
        LeafProtocol::Sequential => {
            let (point, claim) = gpgkr_verify(transcript, fold.e0, &fold.zeta, shape.log_rows() as u32)
                .ok_or(ReduceError::GKR)?;
            query_from_terminal(fold, shape, point, claim).map_err(ReduceError::Claim)
        }
        LeafProtocol::Skip3 | LeafProtocol::Skip4 => {
            let (mut point, claim) = gpgkr_verify(transcript, fold.e0, &fold.zeta, shape.log_rows() as u32 - 1)
                .ok_or(ReduceError::GKR)?;
            point.reverse();
            let (binding, claim) = if protocol == LeafProtocol::Skip3 {
                super::leaf_skip::verify_layer3(transcript, claim, point.into())
            } else {
                super::leaf_skip::verify_layer(transcript, claim, point.into())
            }.ok_or(ReduceError::GKR)?;
            query_from_skipped(fold, shape, binding, claim).map_err(ReduceError::Claim)
        }
    }
}
