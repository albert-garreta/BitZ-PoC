//! Multiplication reduction stopped at its binary inner-product claim.
//!
//! For the hybrid mod-2^32 relation, product = z + 2^32*w is the fixed
//! reconstruction of the two committed 32-bit output limbs. Proving
//! x*y = product with all 64 product bits bound proves the modular relation.
//! The protocol prefix (statement, prime draw, Spartan PIOP, bitification)
//! is the shared one of [`crate::piop::spartan::protocol`]; only the discharge of
//! the bitified claim — the integer folds bound in the clear plus the GKR
//! forest — is the hybrid's own, since its opener runs at the composition's
//! geometry.
use crate::piop::spartan::mul::{MulLayout, MulWitness};
use crate::piop::spartan::protocol::ProtocolError;
use crate::piop::spartan::protocol::{PreparedRelationPrefix, RelationSpec};

use super::{BinaryClaim, Error};
use crate::ligerito::pack_columns_from_rows;
use crate::piop::spartan::SpartanField as _;
use crate::piop::spartan::bitz::{SpartanBitzField, U32_MUL_UNIVARIATE_SKIP_VARS};
use crate::piop::spartan::{
    absorb_spartan_message,
    protocol::{
        SpartanPrefixProof, SpartanProof, bitify, bitz_generator, check_boundary, prove_piop,
        sample_mod_q, validate_bit_rows, verify_piop,
    },
    univariate_skip::UnivariateSkipSpartanPiopProof,
};
use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::transcript::{Blake3Transcript, traits::Transcript};

#[derive(Clone, Debug)]
pub(crate) struct PrefixProof {
    pub field: field::FpCtx<2>,
    pub initial_nonce: u64,
    pub terminal_nonce: u64,
    pub piop_nonces: Vec<u64>,
    pub spartan: UnivariateSkipSpartanPiopProof<SpartanBitzField>,
    /// Authenticated fold and GKR messages on the multiplication fork.
    pub narg: Vec<u8>,
}

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

pub(crate) fn prove(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedRelationPrefix<MulLayout<u32>>,
    witness: &MulWitness<u32>,
    rows: &[Vec<u64>],
    statement: &[u8; 32],
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
    let (narg, claim) = {
        let _scope = tracing::info_span!("mo:wfbitz").entered();
        let (params, claim) = wfbitz_claim(&p, arith, &weights[0], &proved.opening)?;
        let tag = crate::piop::spartan::protocol::wfbitz_opener::fork_tag(transcript);
        let mut state = crate::wfbitz::build_prover(WFBITZ_SESSION, &tag);
        state.public_message(&proved.bridge_digest);
        state.start_native(prefix_schedule(params.shape(), prepared.security())?)
            .map_err(|_| Error::Invalid("native multiplication schedule"))?;
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
        (narg, binary_claim(low, high_point, value))
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
            narg,
        },
        claim,
    ))
}

/// The scheme's parameters and claim for the multiplication grid: the
/// sampled prime, the crate's generator, the one chunk of row weights,
/// the bitified claim's column weights and target.
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
    let claim =
        crate::wfbitz::LinearClaim::new(&params, row_weights.to_vec(), columns, opening.claimed)
            .map_err(|_| Error::Invalid("wfbitz claim"))?;
    Ok((params, claim))
}

/// The scheme's exit as the composition's claim (its field elements are the
/// vendored field's; the composition's are the crate's).
fn binary_claim(
    low: Vec<field::Gf128>,
    high_point: Vec<field::Gf128>,
    value: field::Gf128,
) -> BinaryClaim {
    BinaryClaim {
        low: low.into_iter().map(Gf::from).collect(),
        high_point: high_point.into_iter().map(Gf::from).collect(),
        value: Gf::from(value),
    }
}

/// Binds the forked transcript's narg string (the folds and every GKR
/// message) on the shared transcript before the joint sumcheck draws.
fn bind_wfbitz(transcript: &mut Blake3Transcript, bridge_digest: &[u8; 32], narg: &[u8]) {
    absorb_spartan_message(transcript, b"hybrid/mul-gkr/wfbitz", bridge_digest);
    absorb_spartan_message(transcript, b"narg", blake3::hash(narg).as_bytes());
}

pub(crate) fn verify(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedRelationPrefix<MulLayout<u32>>,
    statement: &[u8; 32],
    proof: &PrefixProof,
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
    let weights = chunks.chunks();
    if weights.len() != 1 {
        return Err(Error::Invalid("BitZ sums shape"));
    }
    let narg = &proof.narg;
    let (params, claim) = wfbitz_claim(&p, arith, &weights[0], &verified.opening)?;
    let tag = crate::piop::spartan::protocol::wfbitz_opener::fork_tag(transcript);
    let bitz_proof = crate::wfbitz::Proof {
        narg_string: narg.clone(),
        hints: Vec::new(),
    };
    let mut state = crate::wfbitz::build_verifier(WFBITZ_SESSION, &tag, &bitz_proof);
    state.public_message(&verified.bridge_digest);
    state.start_native(prefix_schedule(params.shape(), prepared.security())?)
        .map_err(|_| Error::Invalid("native multiplication schedule"))?;
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
    Ok(binary_claim(low, high_point, value))
}

fn prefix_schedule(
    shape: &crate::wfbitz::Shape,
    security: &crate::piop::spartan::profile::IopSecurityParams,
) -> Result<crate::wfbitz::grinding::Schedule, Error> {
    use crate::wfbitz::grinding::{Policy, Geometry, Schedule};
    let policy = Policy::new(Some(security.lambda), security.forest_round_grinding_bits, security.ring_switch_grinding_bits)
        .map_err(|_| Error::Invalid("native multiplication policy"))?;
    Schedule::new(policy, Geometry::prefix(shape.log_rows(), shape.log_columns()))
        .map_err(|_| Error::Invalid("native multiplication geometry"))
}
