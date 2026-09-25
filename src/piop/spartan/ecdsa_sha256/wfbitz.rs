//! The terminal scaled claim opened through the worldfnd/BitZ scheme
//! instead of the forest: structured (`crate::wfbitz::chained`) when the
//! relation prepared the block geometry, else the dense virtual opening
//! (`crate::wfbitz::virt`).
//!
//! The claim is the one the forest path opens: over the derived grid `h`,
//! `Σ_d w(d)·h[d] ≡ target (mod q)` with `w(d) = scale·eq(d, point)` an eq
//! tensor over the bits of the native derived index `d` (`point` is the
//! inner sumcheck's evaluation point, `scale` its terminal evaluation), all
//! canonical residues of the sampled 113-bit modulus. The structured path
//! re-splits that tensor over the block grid (rows = bits `k..k+14` of `d`,
//! blocks = the other bits) and opens the transposed claim as a sum of
//! tensors against the block-layout commitment; the dense path folds the
//! claim in the native `h_layout` and transposes it cell by cell. Either
//! way the forked transcript is the opener glue's: a 32-byte tag squeezed
//! from the outer transcript seeds the scheme's own sponge, and the proof
//! carries its narg string and hint stream.

use super::{Config, F, PreparedSha256Ecdsa, Result, error};
use crate::ligerito_flock::{FlockCommitHint, ProverOod, VerifierOod};
use crate::piop::spartan::matrix::eq_table;
use crate::piop::spartan::protocol::{
    bitz_generator,
    wfbitz_opener::{WfbitzOpeningProof, fork_tag},
};
use crate::transcript::traits::Transcript;
use crate::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, ChainedStatement, LinearClaim, Pcs,
    Proof as BitzTranscriptProof, Root, Shape, VirtualStatement, WINDOW, build_prover,
    build_verifier, chained::LOG_ROWS,
};
use circuit::linear_map::binary::VirtualMap;
use field::RingOps;
use flock_core::pcs::commit::Commitment;
use flock_core::pcs::ligerito::LigeritoProfile;

const SESSION: &[u8] = b"bitz/sha256-ecdsa/wfbitz/v1";
const CHAINED_SESSION: &[u8] = b"bitz/sha256-ecdsa/wfbitz-chained/v1";

fn residues(values: Vec<F>, cfg: &Config) -> Vec<u128> {
    values.iter().map(|x| u128::from(cfg.to_integer(x))).collect()
}

/// The eq tensor over `bits` of the point, scaled by `scale`, as residues.
fn scaled_eq(bits: &[F], scale: &F, cfg: &Config) -> Result<Vec<u128>> {
    Ok(eq_table(bits, cfg)
        .map_err(error)?
        .iter()
        .map(|x| u128::from(cfg.to_integer(&cfg.mul(x, scale))))
        .collect())
}

/// The native claim's row and column weights: `scale·eq(point[..t])` and
/// `eq(point[t..])` over `h_layout`.
fn native_weights(
    prepared: &PreparedSha256Ecdsa,
    point: &[F],
    scale: &F,
    cfg: &Config,
) -> Result<(Vec<u128>, Vec<u128>)> {
    let t = prepared.h_layout.row_vars;
    let rows = scaled_eq(&point[..t], scale, cfg)?;
    let cols = residues(eq_table(&point[t..], cfg).map_err(error)?, cfg);
    Ok((rows, cols))
}

/// The block grid's row and column weights: `eq(point[k..k+14])` and
/// `scale·eq(point[..k] ‖ point[k+14..])`.
fn block_weights(
    log_instances: usize,
    point: &[F],
    scale: &F,
    cfg: &Config,
) -> Result<(Vec<u128>, Vec<u128>)> {
    let k = log_instances;
    if point.len() < k + LOG_ROWS {
        return Err(error("the inner point is narrower than the block grid"));
    }
    let rows = residues(eq_table(&point[k..k + LOG_ROWS], cfg).map_err(error)?, cfg);
    let mut high: Vec<F> = point[..k].to_vec();
    high.extend_from_slice(&point[k + LOG_ROWS..]);
    let cols = scaled_eq(&high, scale, cfg)?;
    Ok((rows, cols))
}

/// The claim grid, the committed grid and the ladder as the dense scheme
/// sees them.
fn dense_setup(prepared: &PreparedSha256Ecdsa, modulus: u128) -> Result<(BitZParams, Shape, Pcs)> {
    if prepared.h_layout.word_bits != 1 || prepared.f_layout.word_bits != 1 {
        return Err(error("the wfbitz opener takes bit grids (word width one)"));
    }
    let derived = Shape::new(prepared.h_layout.row_vars, prepared.h_layout.col_vars)
        .map_err(|e| error(format!("derived shape: {e:?}")))?;
    let committed = Shape::new(prepared.f_layout.row_vars, prepared.f_layout.col_vars)
        .map_err(|e| error(format!("committed shape: {e:?}")))?;
    let params = BitZParams::new(derived, modulus, bitz_generator().into())
        .map_err(|e| error(format!("wfbitz parameters: {e:?}")))?;
    let pcs = Pcs::with_security(&committed, prepared.ligerito.security(), LigeritoProfile::Fast)
        .map_err(|e| error(format!("wfbitz pcs: {e:?}")))?;
    Ok((params, committed, pcs))
}

/// The block grid's parameters and ladder.
fn chained_setup(
    prepared: &PreparedSha256Ecdsa,
    chained: &super::relation::WfbitzChained,
    modulus: u128,
) -> Result<(BitZParams, Pcs)> {
    let shape = chained
        .geometry
        .shape()
        .map_err(|e| error(format!("block shape: {e:?}")))?;
    let params = BitZParams::new(shape, modulus, bitz_generator().into())
        .map_err(|e| error(format!("wfbitz parameters: {e:?}")))?;
    let pcs = Pcs::with_security(&shape, chained.ligerito.security(), LigeritoProfile::Fast)
        .map_err(|e| error(format!("wfbitz pcs: {e:?}")))?;
    let _ = prepared;
    Ok((params, pcs))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_opening<T: Transcript + Send>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    hint: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    point: &[F],
    scale: &F,
    cfg: &Config,
    target: u128,
    modulus: u128,
    ood: ProverOod,
) -> Result<WfbitzOpeningProof> {
    let _scope = tracing::info_span!("ecdsa:wfbitz_prove").entered();
    let ood = ood.into_bound_claim();
    let tag = fork_tag(t);
    let state = if let Some(chained) = &prepared.wfbitz {
        let (params, pcs) = chained_setup(prepared, chained, modulus)?;
        let (rows, cols) = block_weights(chained.geometry.log_instances, point, scale, cfg)?;
        let claim = LinearClaim::new(&params, rows, cols, target)
            .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
        let parts = prepared.map.chained_packed_source().ok_or_else(|| error("no chain"))?;
        let tail = prepared.map.chained_packed_source_tail().ok_or_else(|| error("no tail"))?;
        let statement = ChainedStatement::new(
            params,
            chained.geometry.clone(),
            parts,
            tail,
            prepared.map.digest(),
            &claim,
        )
        .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
        let started = std::time::Instant::now();
        let derived = chained.geometry.derived_rows(&prepared.h_layout, h_rows);
        crate::wfbitz::trace("derived rows (block layout)", started);
        let mut state = build_prover(CHAINED_SESSION, &tag);
        BitZProver::new(params, WINDOW)
            .prove_chained(
                &statement,
                &pcs,
                hint,
                &derived,
                &mut state,
                ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| error(format!("wfbitz prove: {e:?}")))?;
        state
    } else {
        let (params, committed, pcs) = dense_setup(prepared, modulus)?;
        let (rows, cols) = native_weights(prepared, point, scale, cfg)?;
        let claim = LinearClaim::new(&params, rows, cols, target)
            .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
        let statement = VirtualStatement::new(params, committed, &prepared.map, &claim)
            .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
        let mut state = build_prover(SESSION, &tag);
        BitZProver::new(params, WINDOW)
            .prove_virtual(
                &statement,
                &pcs,
                hint,
                h_rows,
                &mut state,
                ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| error(format!("wfbitz prove: {e:?}")))?;
        state
    };
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(WfbitzOpeningProof {
        narg: narg_string,
        hints,
        ood: ood.map(|claim| claim.round),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_opening<T: Transcript + Send>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    commitment: &Commitment,
    proof: &WfbitzOpeningProof,
    point: &[F],
    scale: &F,
    cfg: &Config,
    target: u128,
    modulus: u128,
    ood: VerifierOod,
) -> Result<()> {
    let _scope = tracing::info_span!("ecdsa:wfbitz_verify").entered();
    let ood = ood.into_bound_claim();
    let tag = fork_tag(t);
    let bitz_proof = BitzTranscriptProof {
        narg_string: proof.narg.clone(),
        hints: proof.hints.clone(),
    };
    if let Some(chained) = &prepared.wfbitz {
        let (params, pcs) = chained_setup(prepared, chained, modulus)?;
        let (rows, cols) = block_weights(chained.geometry.log_instances, point, scale, cfg)?;
        let claim = LinearClaim::new(&params, rows, cols, target)
            .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
        let parts = prepared.map.chained_packed_source().ok_or_else(|| error("no chain"))?;
        let tail = prepared.map.chained_packed_source_tail().ok_or_else(|| error("no tail"))?;
        let statement = ChainedStatement::new(
            params,
            chained.geometry.clone(),
            parts,
            tail,
            prepared.map.digest(),
            &claim,
        )
        .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
        let state = build_verifier(CHAINED_SESSION, &tag, &bitz_proof);
        BitZVerifier::new(params, WINDOW)
            .verify_chained(
                &statement,
                &pcs,
                Root(commitment.root),
                state,
                ood.as_ref().map(|(claim, _)| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| error(format!("wfbitz verify: {e:?}")))
    } else {
        let (params, committed, pcs) = dense_setup(prepared, modulus)?;
        let (rows, cols) = native_weights(prepared, point, scale, cfg)?;
        let claim = LinearClaim::new(&params, rows, cols, target)
            .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
        let statement = VirtualStatement::new(params, committed, &prepared.map, &claim)
            .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
        let state = build_verifier(SESSION, &tag, &bitz_proof);
        BitZVerifier::new(params, WINDOW)
            .verify_virtual(
                &statement,
                &pcs,
                Root(commitment.root),
                state,
                ood.as_ref().map(|(claim, _)| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| error(format!("wfbitz verify: {e:?}")))
    }
}
