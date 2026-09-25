//! The terminal scaled claim opened through the worldfnd/BitZ scheme's
//! virtual opening (`crate::wfbitz::virt`) instead of the forest's.
//!
//! The claim is the same one the forest path opens: over the derived grid
//! `h` (`prepared.h_layout`), `Σ_{b,c} rows[b]·h[b,c]·cols[c] ≡ target
//! (mod q)` with `rows` the scaled row `eq` weights, `cols` the column `eq`
//! weights and `target` the inner sumcheck's final claim, all canonical
//! residues of the sampled 113-bit modulus. The scheme folds and runs its
//! GKR over `h` (the witness's derived rows), rewrites the reduced claim
//! through `prepared.map` onto the committed `f` and opens it with the
//! ladder the commitment was made under; the crate's Round 0 rides along as
//! in the direct opener. The forked transcript is the opener glue's:
//! a 32-byte tag squeezed from the outer transcript seeds the scheme's own
//! sponge, and the proof carries its narg string and hint stream.

use super::{PreparedSha256Ecdsa, Result, error};
use crate::ligerito_flock::{FlockCommitHint, ProverOod, VerifierOod};
use flock_core::pcs::commit::Commitment;
use crate::piop::spartan::protocol::{
    bitz_generator,
    wfbitz_opener::{WfbitzOpeningProof, fork_tag},
};
use crate::transcript::traits::Transcript;
use crate::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Proof as BitzTranscriptProof, Root,
    Shape, VirtualStatement, WINDOW, build_prover, build_verifier,
};
use flock_core::pcs::ligerito::LigeritoProfile;

const SESSION: &[u8] = b"bitz/sha256-ecdsa/wfbitz/v1";

/// The claim grid, the committed grid and the ladder as the scheme sees them.
fn setup(prepared: &PreparedSha256Ecdsa, modulus: u128) -> Result<(BitZParams, Shape, Pcs)> {
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

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_opening<T: Transcript + Send>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    hint: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    rows: Vec<u128>,
    cols: Vec<u128>,
    target: u128,
    modulus: u128,
    ood: ProverOod,
) -> Result<WfbitzOpeningProof> {
    let _scope = tracing::info_span!("ecdsa:wfbitz_prove").entered();
    let (params, committed, pcs) = setup(prepared, modulus)?;
    let claim = LinearClaim::new(&params, rows, cols, target)
        .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
    let statement = VirtualStatement::new(params, committed, &prepared.map, &claim)
        .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
    let ood = ood.into_bound_claim();
    let tag = fork_tag(t);
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
    rows: Vec<u128>,
    cols: Vec<u128>,
    target: u128,
    modulus: u128,
    ood: VerifierOod,
) -> Result<()> {
    let _scope = tracing::info_span!("ecdsa:wfbitz_verify").entered();
    let (params, committed, pcs) = setup(prepared, modulus)?;
    let claim = LinearClaim::new(&params, rows, cols, target)
        .map_err(|e| error(format!("wfbitz claim: {e:?}")))?;
    let statement = VirtualStatement::new(params, committed, &prepared.map, &claim)
        .map_err(|e| error(format!("wfbitz statement: {e:?}")))?;
    let ood = ood.into_bound_claim();
    let tag = fork_tag(t);
    let bitz_proof = BitzTranscriptProof {
        narg_string: proof.narg.clone(),
        hints: proof.hints.clone(),
    };
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
