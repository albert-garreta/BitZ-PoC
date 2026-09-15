//! Their grand-product reduction (step 4) to a factored inner-product claim
//! over the committed bits: each leaf is `1 + (y_row − 1)·bit(column, row)`,
//! and at GKR's terminal point subtracting one leaves
//! `Σ u1[row]·u2[column]·bit(column, row)`.

use super::fold::Fold;
use super::gkr::{GrandProductCircuit, gpgkr_prove, gpgkr_verify};
use super::params::{ClaimError, LinearClaimGf, Shape};
use super::pcs::OpeningQuery;
use super::transcript::{ProverState, VerifierState};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::cfg_chunks_mut;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;

/// A reduction the verifier rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceError {
    /// The GKR replay failed.
    GKR,
    /// The derived weight counts do not match the table shape.
    Claim(ClaimError),
}

/// Bit `(column, row)` of the committed table, from its per-column rows.
fn bit(rows: &[Vec<u64>], column: usize, row: usize) -> bool {
    (rows[column][row / 64] >> (row % 64)) & 1 == 1
}

fn eq_table(point: &[Gf]) -> Vec<Gf> {
    build_eq_x_r_vec(point, &()).expect("non-empty point")
}

/// Splits the terminal point into column and row coordinates (columns occupy
/// the low index bits) and derives the two weight factors.
fn query_from_terminal(
    fold: &Fold,
    shape: &Shape,
    mut point: Vec<Gf>,
    claim: Gf,
) -> Result<OpeningQuery, ClaimError> {
    // The multilinear extension of the constant-one table is one everywhere.
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
    let u2 = eq_table(&alfa_c);
    let claim = LinearClaimGf::from_shape(shape, u1, u2, inner_product_claim)?;
    Ok(OpeningQuery::InnerProduct { claim })
}

pub(crate) fn gkr_reduce_prove(
    transcript: &mut ProverState,
    fold: &Fold,
    shape: &Shape,
    rows: &[Vec<u64>],
) -> Result<OpeningQuery, ClaimError> {
    let columns = shape.columns();
    // One chunk per row `b`: leaf `(b, c)` at `b * columns + c`.
    let mut leafs = vec![Gf::one(); columns * shape.rows()];
    cfg_chunks_mut!(leafs, columns)
        .enumerate()
        .for_each(|(b, chunk)| {
            let image = fold.row_images[b];
            for (c, leaf) in chunk.iter_mut().enumerate() {
                if bit(rows, c, b) {
                    *leaf = image;
                }
            }
        });
    let circuit = GrandProductCircuit::new(leafs);
    let (_roots, witnesses) = circuit.batched_eval(columns);
    let (point, claim) = gpgkr_prove(transcript, &fold.zeta, witnesses);
    query_from_terminal(fold, shape, point, claim)
}

pub(crate) fn gkr_reduce_verify(
    transcript: &mut VerifierState<'_>,
    fold: &Fold,
    shape: &Shape,
) -> Result<OpeningQuery, ReduceError> {
    let (point, claim) = gpgkr_verify(transcript, fold.e0, &fold.zeta, shape.log_rows() as u32)
        .ok_or(ReduceError::GKR)?;
    query_from_terminal(fold, shape, point, claim).map_err(ReduceError::Claim)
}
