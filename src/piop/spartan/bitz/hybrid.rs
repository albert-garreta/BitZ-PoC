//! Multiplication reduction stopped at its binary inner-product claim.
//!
//! For the hybrid mod-2^32 relation, product = z + 2^32*w is the fixed
//! reconstruction of the two committed 32-bit output limbs. Proving
//! x*y = product with all 64 product bits bound proves the modular relation.
//! The protocol prefix (statement, prime draw, Spartan PIOP, bitification)
//! is the shared one of [`super::super::protocol`]; only the discharge of
//! the bitified claim — the integer folds bound in the clear plus the GKR
//! forest — is the hybrid's own, since its opener runs at the composition's
//! geometry.
use crate::piop::spartan::mul::{MulLayout, MulWitness};
use crate::piop::spartan::protocol::PreparedRelationPrefix;
use crate::piop::spartan::protocol::ProtocolError;

use super::super::{
    absorb_spartan_message,
    protocol::{
        SpartanPrefixProof, SpartanProof, bitify, check_boundary, bitz_generator, prove_piop,
        sample_mod_q, validate_bit_rows, verify_piop,
    },
    univariate_skip::UnivariateSkipSpartanPiopProof,
};
use super::*;
use crate::hybrid::{BinaryClaim, Error, MulOpener};
use crate::ligerito::{fold_values_bits, pack_columns_from_rows, row_bit_vars};
use crate::merged_forest::{MergedForestProof, prove_merged_forest_lazy, verify_merged_forest};
use crate::pcs::{chunk_pow2_table, row_bit_weights};
use crate::piop::spartan::SpartanField as _;
use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::transcript::{Blake3Transcript, traits::Transcript};
use field::{Fp, RingOps, Uint, Uint as FieldUint};

#[derive(Clone, Debug)]
pub(crate) struct PrefixProof {
    pub field: field::FpCtx<2>,
    pub initial_nonce: u64,
    pub terminal_nonce: u64,
    pub piop_nonces: Vec<u64>,
    pub spartan: UnivariateSkipSpartanPiopProof<SpartanBitzField>,
    /// The integer column folds bound in the clear (the forest path; empty
    /// on the wfbitz path, whose folds travel inside its narg string).
    pub sums: Vec<u128>,
    pub gkr: MulGkr,
}

/// The grand-product reduction of the multiplication side, by opener.
#[derive(Clone, Debug)]
pub(crate) enum MulGkr {
    /// The crate's merged forest on the shared transcript.
    Forest(MergedForestProof),
    /// The worldfnd/BitZ scheme's fold and per-level GKR on a forked
    /// transcript: its narg string (folds, round messages, closing pairs).
    #[cfg(feature = "bitz-parity")]
    Wfbitz { narg: Vec<u8> },
}

#[cfg(feature = "bitz-parity")]
const WFBITZ_SESSION: &[u8] = b"bitz/hybrid/mul-gkr/wfbitz/v1";

impl PrefixProof {
    fn messages(&self) -> SpartanPrefixProof {
        SpartanPrefixProof {
            initial_nonce: self.initial_nonce,
            piop_nonces: self.piop_nonces.clone(),
            spartan: SpartanProof::UnivariateSkip(self.spartan.clone()),
            terminal_nonce: self.terminal_nonce,
        }
    }
}

pub(crate) fn decoding_config(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedRelationPrefix<MulLayout<u32>>,
    statement: &[u8; 32],
    nonce: u64,
) -> Result<
    (
        u128,
        <SpartanBitzField as crate::piop::spartan::SpartanField>::Config,
    ),
    Error,
> {
    let domains = prepared.layout().domains();
    absorb_spartan_message(transcript, domains.statement_tag, statement);
    check_boundary(
        transcript,
        domains.initial_grinding,
        prepared.security().initial_grinding_bits,
        nonce,
    )?;
    let prime = sample_mod_q(
        transcript,
        domains.prime_sampling,
        prepared.security().projection_min,
        prepared.security().projection_max,
    )?;
    Ok((prime.modulus_u128(), prime))
}

fn bind_sums(t: &mut Blake3Transcript, digest: &[u8; 32], sums: &[u128]) {
    t.absorb_slice(b"hybrid/u32-bounded-sums/v1");
    t.absorb_slice(digest);
    for sum in sums {
        t.absorb_slice(&sum.to_le_bytes());
    }
}

fn endpoint(p: &crate::pcs::IntegerMatrixLayout, weights: &[u128], z: &[Gf], e: Gf) -> BinaryClaim {
    let tw = row_bit_vars(p);
    BinaryClaim {
        low: row_bit_weights(p, weights, bitz_generator(), &z[..tw])
            .into_iter()
            .collect(),
        high_point: z[tw..].iter().copied().collect(),
        value: (e + Gf::one()),
    }
}

pub(crate) fn prove(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedRelationPrefix<MulLayout<u32>>,
    witness: &MulWitness<u32>,
    rows: &[Vec<u64>],
    statement: &[u8; 32],
    opener: MulOpener,
) -> Result<(PrefixProof, BinaryClaim), Error> {
    let piop_scope = tracing::info_span!("hybrid:mul_piop").entered();
    let layout = prepared.layout();
    if witness.layout() != layout {
        return Err(Error::Invalid("multiplication layout"));
    }
    let p = layout.bitz_params();
    validate_bit_rows(&p, rows)?;
    absorb_spartan_message(transcript, layout.domains().statement_tag, statement);
    let proved = prove_piop(transcript, prepared, witness, statement)?;
    drop(piop_scope);

    let _opening_scope = tracing::info_span!("hybrid:mul_opening").entered();
    let arith = &proved.prime;
    let chunks = bitify::prepare_chunks(
        &proved.opening,
        &proved.table,
        proved.prime.modulus_bits(),
        arith,
    )?;
    let weights = chunks.chunks();
    if weights.len() != 1 {
        return Err(Error::Invalid("multiple BitZ chunks"));
    }
    let (sums, gkr, claim) = match opener {
        MulOpener::Forest => {
            let fold_scope = tracing::info_span!("mo:fold_values").entered();
            let sums = fold_values_bits(&p, rows, &weights[0]);
            drop(fold_scope);
            bind_sums(transcript, &proved.bridge_digest, &sums);
            let pack_scope = tracing::info_span!("mo:pack_cols").entered();
            let packed_cols = pack_columns_from_rows(&p, rows);
            drop(pack_scope);
            let pow2_scope = tracing::info_span!("mo:pow2").entered();
            let powers = chunk_pow2_table(&p, &weights[0], bitz_generator());
            drop(pow2_scope);
            let forest_scope = tracing::info_span!("mo:forest").entered();
            let (_, forest, z, e) =
                prove_merged_forest_lazy(transcript, &p, &packed_cols, &powers, p.cols());
            drop(forest_scope);
            let endpoint_scope = tracing::info_span!("mo:endpoint").entered();
            let claim = endpoint(&p, &weights[0], &z, e);
            drop(endpoint_scope);
            (sums, MulGkr::Forest(forest), claim)
        }
        #[cfg(not(feature = "bitz-parity"))]
        MulOpener::Wfbitz => {
            return Err(Error::Invalid("the wfbitz opener needs the bitz-parity feature"));
        }
        #[cfg(feature = "bitz-parity")]
        MulOpener::Wfbitz => {
            let _scope = tracing::info_span!("mo:wfbitz").entered();
            let (params, claim) = wfbitz_claim(&p, arith, &weights[0], &proved.opening)?;
            let tag = crate::piop::spartan::protocol::wfbitz_opener::fork_tag(transcript);
            let mut state = crate::wfbitz::build_prover(WFBITZ_SESSION, &tag);
            state.public_message(&proved.bridge_digest);
            let prover = crate::wfbitz::BitZProver::new(params, crate::wfbitz::WINDOW);
            let fold = prover
                .send_fold(&claim, rows, &mut state)
                .map_err(|_| Error::Invalid("wfbitz fold"))?;
            let packed_cols = pack_columns_from_rows(&p, rows);
            let shape = *params.shape();
            let (low, high_point, value) =
                crate::wfbitz::reduce::gkr_exit_prove(&mut state, &fold, &shape, &packed_cols);
            let narg = state.finish().narg_string;
            bind_wfbitz(transcript, &proved.bridge_digest, &narg);
            (Vec::new(), MulGkr::Wfbitz { narg }, binary_claim(low, high_point, value))
        }
    };
    let SpartanPrefixProof {
        initial_nonce,
        piop_nonces,
        spartan,
        terminal_nonce,
    } = proved.messages;
    let SpartanProof::UnivariateSkip(spartan) = spartan else {
        return Err(Error::Invalid("u32 kernel shape"));
    };
    Ok((
        PrefixProof {
            field: proved.prime,
            initial_nonce,
            terminal_nonce,
            piop_nonces,
            spartan,
            sums,
            gkr,
        },
        claim,
    ))
}

/// The scheme's parameters and claim for the multiplication grid: the
/// sampled prime, the crate's generator, the one chunk of row weights,
/// the bitified claim's column weights and target.
#[cfg(feature = "bitz-parity")]
fn wfbitz_claim(
    p: &crate::pcs::IntegerMatrixLayout,
    arith: &field::FpCtx<2>,
    row_weights: &[u128],
    opening: &bitify::BitifiedClaim,
) -> Result<(crate::wfbitz::BitZParams, crate::wfbitz::LinearClaim), Error> {
    if p.word_bits != 1 {
        return Err(Error::Invalid("wfbitz takes bit grids"));
    }
    let shape = crate::wfbitz::Shape::new(p.row_vars, p.col_vars)
        .map_err(|_| Error::Invalid("wfbitz shape"))?;
    let modulus = crate::piop::spartan::protocol::wfbitz_opener::modulus_u128(arith);
    let params = crate::wfbitz::BitZParams::new(shape, modulus, bitz_generator().into())
        .map_err(|_| Error::Invalid("wfbitz parameters (fold bound)"))?;
    let columns = bitify::column_weights(opening, arith)?;
    let claim = crate::wfbitz::LinearClaim::new(&params, row_weights.to_vec(), columns, opening.claimed)
        .map_err(|_| Error::Invalid("wfbitz claim"))?;
    Ok((params, claim))
}

/// The scheme's exit as the composition's claim (its field elements are the
/// vendored field's; the composition's are the crate's).
#[cfg(feature = "bitz-parity")]
fn binary_claim(low: Vec<field::Gf128>, high_point: Vec<field::Gf128>, value: field::Gf128) -> BinaryClaim {
    BinaryClaim {
        low: low.into_iter().map(Gf::from).collect(),
        high_point: high_point.into_iter().map(Gf::from).collect(),
        value: Gf::from(value),
    }
}

/// Binds the forked transcript's narg string (the folds and every GKR
/// message) on the shared transcript before the joint sumcheck draws.
#[cfg(feature = "bitz-parity")]
fn bind_wfbitz(transcript: &mut Blake3Transcript, bridge_digest: &[u8; 32], narg: &[u8]) {
    absorb_spartan_message(transcript, b"hybrid/mul-gkr/wfbitz", bridge_digest);
    absorb_spartan_message(transcript, b"narg", blake3::hash(narg).as_bytes());
}

pub(crate) fn verify(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedRelationPrefix<MulLayout<u32>>,
    statement: &[u8; 32],
    proof: &PrefixProof,
    opener: MulOpener,
) -> Result<BinaryClaim, Error> {
    let layout = prepared.layout();
    let p = layout.bitz_params();
    let actual_skip_vars = proof.spartan.outer.skip.skip_vars;
    let expected_skip_vars = U32_MUL_UNIVARIATE_SKIP_VARS as u8;
    if actual_skip_vars != expected_skip_vars {
        return Err(ProtocolError::UnexpectedUnivariateSkipVariables {
            expected: expected_skip_vars,
            actual: actual_skip_vars,
        }
        .into());
    }

    absorb_spartan_message(transcript, layout.domains().statement_tag, statement);
    let verified = verify_piop(transcript, prepared, statement, &proof.messages())?;
    if proof.field.modulus() != verified.prime.modulus() {
        return Err(Error::Invalid("multiplication field"));
    }
    let arith = &verified.prime;

    let chunks = bitify::prepare_chunks(
        &verified.opening,
        &verified.table,
        verified.prime.modulus_bits(),
        arith,
    )?;
    let col_weights = bitify::column_weights(&verified.opening, arith)?;
    let weights = chunks.chunks();
    if weights.len() != 1 {
        return Err(Error::Invalid("BitZ sums shape"));
    }
    let forest = match (&proof.gkr, opener) {
        (MulGkr::Forest(forest), MulOpener::Forest) => forest,
        #[cfg(feature = "bitz-parity")]
        (MulGkr::Wfbitz { narg }, MulOpener::Wfbitz) => {
            if !proof.sums.is_empty() {
                return Err(Error::Invalid("wfbitz proofs carry no clear sums"));
            }
            let (params, claim) = wfbitz_claim(&p, arith, &weights[0], &verified.opening)?;
            let tag = crate::piop::spartan::protocol::wfbitz_opener::fork_tag(transcript);
            let bitz_proof = crate::wfbitz::Proof {
                narg_string: narg.clone(),
                hints: Vec::new(),
            };
            let mut state = crate::wfbitz::build_verifier(WFBITZ_SESSION, &tag, &bitz_proof);
            state.public_message(&verified.bridge_digest);
            let verifier = crate::wfbitz::BitZVerifier::new(params, crate::wfbitz::WINDOW);
            let fold = verifier
                .receive_fold(&claim, &mut state)
                .map_err(|_| Error::Invalid("wfbitz fold"))?;
            let shape = *params.shape();
            let (low, high_point, value) =
                crate::wfbitz::reduce::gkr_exit_verify(&mut state, &fold, &shape)
                    .map_err(|_| Error::Invalid("wfbitz GKR"))?;
            state
                .check_eof()
                .map_err(|_| Error::Invalid("wfbitz trailing data"))?;
            bind_wfbitz(transcript, &verified.bridge_digest, narg);
            return Ok(binary_claim(low, high_point, value));
        }
        _ => return Err(Error::Invalid("the proof's GKR is not the prepared opener's")),
    };
    if proof.sums.len() != p.cols() {
        return Err(Error::Invalid("BitZ sums shape"));
    }
    // Every true integer fold lies below the group order. Check a tighter,
    // instance-derived bound on the sent integers before exponentiation.
    let bound = weights[0]
        .iter()
        .try_fold(0u128, |sum, w| sum.checked_add(*w))
        .and_then(|s| s.checked_mul((1u128 << p.word_bits) - 1))
        .ok_or(Error::Invalid("integer fold bound overflow"))?;
    if bound == u128::MAX || proof.sums.iter().any(|&s| s > bound) {
        return Err(Error::Invalid("integer fold magnitude"));
    }
    let read_off = proof.sums.iter().zip(&col_weights).fold(0, |a, (&s, w)| {
        arith.add_u128(a, arith.mul_u128(arith.reduce_u128(s), *w))
    });
    if read_off != verified.opening.claimed {
        return Err(Error::Invalid("integer read-off"));
    }
    bind_sums(transcript, &verified.bridge_digest, &proof.sums);
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, bitz_generator().into(), 8);
    let roots: Vec<_> = proof
        .sums
        .iter()
        .map(|&s| Gf::from(comb.pow_public(&field::Uint::from_words([s as u64, (s >> 64) as u64]))))
        .collect();
    let (z, e) = verify_merged_forest(
        transcript,
        &roots,
        forest,
        row_bit_vars(&p),
        p.col_vars,
    )
    .map_err(|_| Error::Invalid("multiplication GKR"))?;
    Ok(endpoint(&p, &weights[0], &z, e))
}
