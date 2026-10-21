//! A block-layout claim and a virtual claim opened through one commitment:
//! the BitZ paper's §2.4.1 (one commitment to the concatenation of the
//! sub-relations' witnesses; each sub-relation runs until its claim on the
//! committed bits; the claims are proved together).
//!
//! Each claim runs its own fold and GKR on its own derived grid, under its
//! own parameters (shape and modulus) and an integer-only native schedule.
//! The block claim then transposes to its rank-one committed claim
//! `⟨R ⊗ u₂, f⟩ = t_b` ([`super::block`]), the virtual claim to the map's
//! structured committed weights `⟨w, f⟩ = t_v`
//! ([`VirtualMap::structured_weights`]); one draw `γ` batches them, and one
//! sumcheck, ring switch and Ligerito opening discharge
//! `⟨w + γ·(R ⊗ u₂), f⟩ = t_v + γ·t_b`. The verifier evaluates the batched
//! weights' extension at the sumcheck's point without forming them: the
//! map's [`VirtualMap::transposed_mle`] plus `γ·R(r₁)·u₂(r₂)`.
//!
//! Both statements bind the same root and committed shape; the map's
//! structured weights must leave its second term free (the block claim's).

use circuit::linear_map::binary::VirtualMap;
use field::Gf128 as Gf;

use super::{
    BitZProver, BitZVerifier, BlockStatement, Pcs, ProveError, ProverState, VerifierState,
    VerifyError, VirtualStatement, WINDOW,
    gkr::eq_table,
    params::Root,
    pcs::OpeningQuery,
    reduce,
    transcript::PublicTranscript,
};
use crate::{ligerito_flock::FlockCommitHint, pcs::IntegerMatrixLayout};

const SHARED_STATEMENT_LABEL: &[u8] = b"bitz/shared-statement/v1";
/// The draw that batches the two committed claims.
const SHARED_BATCH_LABEL: &[u8] = b"bitz/shared/batch/v1";

fn bind<M: VirtualMap>(
    block: &BlockStatement<'_>,
    glue: &VirtualStatement<'_, M>,
    root: Root,
    pcs: &Pcs,
    transcript: &mut impl PublicTranscript,
) {
    transcript.public_message(SHARED_STATEMENT_LABEL);
    transcript.public_message(&root.0);
    transcript.public_message(block.frame().as_slice());
    transcript.public_message(glue.frame().as_slice());
    transcript.public_message(pcs);
    transcript.public_message(block.claim());
    transcript.public_message(glue.claim());
}

/// Both statements are on the PCS's committed grid.
fn shapes_match<M: VirtualMap>(block: &BlockStatement<'_>, glue: &VirtualStatement<'_, M>, pcs: &Pcs) -> bool {
    block.committed_shape() == glue.committed_shape()
        && !glue.is_direct()
        && pcs.bit_len() == 1usize << block.committed_shape().log_bits()
}

/// `Σ_i values[i]·eq(i, point)`, index bit `j` ↔ `point[j]`.
fn mle(values: &[Gf], point: &[Gf]) -> Gf {
    eq_table(point).iter().zip(values).fold(Gf::zero(), |acc, (e, v)| acc + *e * *v)
}

/// Proves `block`'s claim on its derived rows `block_rows` and `glue`'s on
/// `glue_rows` through the commitment `hint` (made under `pcs` on both
/// statements' committed grid), in one opening; `ood` as in
/// [`BitZProver::prove`].
#[allow(clippy::too_many_arguments)]
pub fn prove_shared<M: VirtualMap>(
    block: &BlockStatement<'_>,
    block_rows: &[Vec<u64>],
    glue: &VirtualStatement<'_, M>,
    glue_rows: &[Vec<u64>],
    pcs: &Pcs,
    hint: &FlockCommitHint,
    transcript: &mut ProverState,
    ood: Option<crate::ligerito_flock::OodClaim<'_>>,
) -> Result<(), ProveError> {
    if !shapes_match(block, glue, pcs) {
        return Err(ProveError::Block(super::BlockError::ParameterMismatch));
    }
    for (params, rows) in [(block.claim_params(), block_rows), (glue.claim_params(), glue_rows)] {
        let shape = params.shape();
        if rows.len() != shape.columns() || rows.iter().any(|row| row.len() * 64 != shape.rows()) {
            return Err(ProveError::Witness);
        }
    }
    let internal = || ProveError::Opening(super::pcs::ProveError::Internal);
    bind(block, glue, Root(*hint.root()), pcs, transcript);

    // Each claim's fold and GKR on its derived grid; the block claim's
    // derived packing and nibble rows feed its folds too (as the block
    // opener's), the virtual claim's the same way.
    super::trace_start();
    let mut reduce_claim = |params: &super::BitZParams, claim: &super::LinearClaim, rows: &[Vec<u64>]| -> Result<OpeningQuery, ProveError> {
        let started = std::time::Instant::now();
        let shape = *params.shape();
        transcript.start_native(pcs.native_prefix(&shape).map_err(|_| internal())?).map_err(|_| internal())?;
        let layout = IntegerMatrixLayout { row_vars: shape.log_rows(), col_vars: shape.log_columns() };
        let packed = crate::ligerito::pack_columns_from_rows(&layout, rows);
        let nibble = super::forest::nibble_rows_for(shape.log_rows(), shape.log_columns(), &packed);
        let fold = BitZProver::new(*params, WINDOW)
            .send_fold_with(claim, rows, nibble.as_ref(), transcript)
            .map_err(ProveError::Fold)?;
        let query = reduce::gkr_reduce_prove_packed_with(transcript, &fold, &shape, &packed, nibble)
            .map_err(ProveError::Reduction)?;
        transcript.end_native().map_err(|_| internal())?;
        super::trace("shared: fold+gkr", started);
        Ok(query)
    };
    let block_query = reduce_claim(block.claim_params(), block.claim(), block_rows)?;
    let glue_query = reduce_claim(glue.claim_params(), glue.claim(), glue_rows)?;

    let started = std::time::Instant::now();
    let OpeningQuery::InnerProduct { claim: block_claim } = block.transpose_query(block_query).map_err(ProveError::Block)? else {
        return Err(ProveError::Block(super::BlockError::UnsupportedQuery));
    };
    let OpeningQuery::InnerProduct { claim: glue_claim } = glue_query else {
        return Err(ProveError::Virtual(super::VirtualError::UnsupportedQuery));
    };
    let glue_shape = glue.claim_params().shape();
    let mut weights = glue
        .map()
        .structured_weights(glue_claim.row_weights(), glue_claim.column_weights(), glue_shape.log_rows())
        .ok_or(ProveError::Virtual(super::VirtualError::UnsupportedQuery))?;
    super::trace("shared: transposes", started);

    // The batch: the block claim's rank-one weights as the second term.
    transcript.public_message(SHARED_BATCH_LABEL);
    let gamma: Gf = transcript.verifier_message();
    let columns = 1usize << weights.s;
    if weights.t != block.committed_shape().log_rows()
        || block_claim.row_weights().len() != 1 << weights.t
        || block_claim.column_weights().len() != columns
        || weights.term2.iter().any(|&k| k != u32::MAX)
    {
        return Err(ProveError::Virtual(super::VirtualError::UnsupportedQuery));
    }
    weights.rows.push(block_claim.row_weights().to_vec());
    weights.term2 = vec![(weights.rows.len() - 1) as u32; columns];
    weights.weight2 = block_claim.column_weights().iter().map(|&u| gamma * u).collect();
    let target = glue_claim.target() + gamma * block_claim.target();
    let started = std::time::Instant::now();
    let result = pcs.prove_lin_structured(hint, weights, target, transcript, ood).map_err(ProveError::Opening);
    super::trace("shared: opening (all)", started);
    result
}

/// Checks a [`prove_shared`] proof, consuming the transcript.
pub fn verify_shared<M: VirtualMap>(
    block: &BlockStatement<'_>,
    glue: &VirtualStatement<'_, M>,
    pcs: &Pcs,
    com: Root,
    mut transcript: VerifierState<'_>,
    ood: Option<crate::ligerito_flock::OodClaim<'_>>,
) -> Result<(), VerifyError> {
    if !shapes_match(block, glue, pcs) || !glue.map().has_transposed_mle() {
        return Err(VerifyError::Block(super::BlockError::ParameterMismatch));
    }
    let internal = || VerifyError::Opening(super::pcs::VerifyError::Internal);
    bind(block, glue, com, pcs, &mut transcript);
    let mut reduce_claim = |params: &super::BitZParams, claim: &super::LinearClaim| -> Result<OpeningQuery, VerifyError> {
        let shape = *params.shape();
        transcript.start_native(pcs.native_prefix(&shape).map_err(|_| internal())?).map_err(|_| internal())?;
        let fold = BitZVerifier::new(*params, WINDOW).receive_fold(claim, &mut transcript).map_err(VerifyError::Fold)?;
        let query = reduce::gkr_reduce_verify(&mut transcript, &fold, &shape).map_err(VerifyError::Reduction)?;
        transcript.end_native().map_err(|_| internal())?;
        Ok(query)
    };
    let block_query = reduce_claim(block.claim_params(), block.claim())?;
    let glue_query = reduce_claim(glue.claim_params(), glue.claim())?;
    let OpeningQuery::InnerProduct { claim: block_claim } = block.transpose_query(block_query).map_err(VerifyError::Block)? else {
        return Err(VerifyError::Block(super::BlockError::UnsupportedQuery));
    };
    let OpeningQuery::InnerProduct { claim: glue_claim } = glue_query else {
        return Err(VerifyError::Virtual(super::VirtualError::UnsupportedQuery));
    };
    let glue_shape = glue.claim_params().shape();
    if glue_claim.row_weights().len() != glue_shape.rows() || glue_claim.column_weights().len() != glue_shape.columns() {
        return Err(VerifyError::Virtual(super::VirtualError::UnsupportedQuery));
    }
    transcript.public_message(SHARED_BATCH_LABEL);
    let gamma: Gf = transcript.verifier_message();
    let t = block.committed_shape().log_rows();
    let target = glue_claim.target() + gamma * block_claim.target();
    let map = glue.map();
    let (row_w, col_w, glue_t) = (glue_claim.row_weights(), glue_claim.column_weights(), glue_shape.log_rows());
    let (block_rows, block_cols) = (block_claim.row_weights(), block_claim.column_weights());
    let mut lazy_failed = false;
    pcs.verify_lin_lazy(
        &com,
        target,
        |point| {
            let glue_weight = map.transposed_mle(row_w, col_w, glue_t, point);
            lazy_failed = glue_weight.is_none() || point.len() < t;
            if lazy_failed {
                return Gf::zero();
            }
            glue_weight.unwrap_or(Gf::zero()) + gamma * mle(block_rows, &point[..t]) * mle(block_cols, &point[t..])
        },
        &mut transcript,
        ood,
    )
    .map_err(VerifyError::Opening)?;
    if lazy_failed {
        return Err(VerifyError::Virtual(super::VirtualError::UnsupportedQuery));
    }
    transcript.check_eof().map_err(|_| VerifyError::TrailingData)
}
