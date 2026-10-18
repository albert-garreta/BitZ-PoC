//! The native public pieces of [`super::verify_sha256_ecdsa`] the verifier
//! circuit (`crate::recursion`) calls: everything here touches public data
//! only (statement, configuration, challenges, matrix evaluations).

use flock_core::pcs::commit::Commitment;

use super::proof::{BATCH_GRINDING_DOMAIN_PUB, INITIAL_GRINDING_DOMAIN_PUB, OUTER_GRINDING_DOMAIN};
use super::{Config, F, PreparedSha256Ecdsa, Result, Sha256EcdsaStatement};
use crate::sumcheck::proof::OuterSumcheckProof;
use crate::transcript::traits::Transcript;

pub(crate) use super::bitz::{dense_opening, DENSE_SESSION};
pub(crate) use super::security::Sha256EcdsaSecurity;

pub(crate) const INITIAL_DOMAIN: &[u8] = INITIAL_GRINDING_DOMAIN_PUB;
pub(crate) const BATCH_DOMAIN: &[u8] = BATCH_GRINDING_DOMAIN_PUB;
pub(crate) const OUTER_DOMAIN: &[u8] = OUTER_GRINDING_DOMAIN;

/// Grinding bits and Round-0 parameters of the prepared relation.
pub(crate) struct Grinding {
    pub initial: u32,
    pub batch: u32,
    pub outer: u32,
    pub inner: u32,
    pub ood: Option<crate::ligerito_flock::OodRoundParams>,
}

pub(crate) fn grinding(prepared: &PreparedSha256Ecdsa) -> Result<(Grinding, Sha256EcdsaSecurity)> {
    let security = prepared.security()?;
    Ok((
        Grinding {
            initial: security.initial,
            batch: security.batch,
            outer: security.outer,
            inner: security.inner,
            ood: security.ood,
        },
        security,
    ))
}

/// `bind_statement` (all public).
pub(crate) fn bind_statement<T: Transcript>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    security: &Sha256EcdsaSecurity,
) -> Result<()> {
    super::proof::bind_statement(t, prepared, statement, commitment, security)
}

pub(crate) fn outer_vars(prepared: &PreparedSha256Ecdsa) -> usize {
    prepared.outer_sumcheck_num_vars()
}

pub(crate) fn inner_vars(prepared: &PreparedSha256Ecdsa) -> usize {
    prepared.h_layout.row_vars + prepared.h_layout.col_vars
}

pub(crate) fn packed_vars(prepared: &PreparedSha256Ecdsa) -> usize {
    crate::ligerito::packed_vars(&prepared.f_layout)
}

/// The inner batching challenges (public).
pub(crate) fn inner_batch<T: Transcript>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    cfg: &Config,
) -> Result<(F, Vec<F>, F)> {
    super::proof::sample_inner_batch_challenges(t, prepared, cfg)
}

/// The inner claim with zero outer evaluations: the public part of the
/// claimed sum, and the claim object the matrix evaluation needs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn inner_claim(
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    outer_point: Vec<F>,
    rho: F,
    linear_point: Vec<F>,
    linear_weight: F,
    cfg: &Config,
) -> Result<(super::inner_reduction::InnerSumcheckClaim, F)> {
    let zero = <F as crate::piop::spartan::SpartanField>::zero_with_cfg(cfg);
    let outer = OuterSumcheckProof {
        sumcheck: crate::sumcheck::proof::SumcheckProof { round_polynomials: Vec::new() },
        az_mle_claim: zero,
        bz_mle_claim: zero,
        cz_mle_claim: zero,
    };
    let claim = super::inner_reduction::InnerSumcheckClaim::from_outer_claims(
        prepared,
        statement,
        &outer,
        outer_point,
        rho,
        linear_point,
        linear_weight,
        cfg,
    )?;
    let public = claim.claimed_sum().clone();
    Ok((claim, public))
}

/// The batched matrix MLE at the inner point (the opening's scale).
pub(crate) fn matrix_scale(
    prepared: &PreparedSha256Ecdsa,
    claim: &super::inner_reduction::InnerSumcheckClaim,
    point: &[F],
    cfg: &Config,
) -> Result<F> {
    let mut coefficients = super::inner_reduction::ModQCoefficients::from_relation(prepared, cfg);
    coefficients.evaluate_batched_matrix_mle(prepared, claim, point, cfg)
}

/// The dense opening's claim rows for the recursion's base proofs: `2^17`
/// rows (`q < 2^110`) keep its public folds at `16·2^{bits − 17}` bytes.
pub(crate) const DENSE_CLAIM_LOG_ROWS: usize = 17;

/// Forces the dense virtual opening (the structured chained one has its own
/// verifier, not arithmetized yet) with `2^rows` claim rows (at most the
/// grid's height less one).
pub(crate) fn force_dense(prepared: &mut PreparedSha256Ecdsa, rows: usize) -> Result<()> {
    let bits = prepared.h_layout.row_vars + prepared.h_layout.col_vars;
    prepared.force_dense_opening(rows.min(bits - 1))
}

/// `bind_opening`'s public prefix: the frame and the point and scale.
pub(crate) fn opening_frame<T: Transcript>(t: &mut T, point: &[F], scale: &F, cfg: &Config) {
    use crate::piop::spartan::{SpartanField, absorb_spartan_message};
    absorb_spartan_message(t, b"scaled-assignment-claim", b"v1");
    for x in point.iter().chain([scale]) {
        absorb_spartan_message(t, b"field", &x.canonical_element_encoding(cfg));
    }
}
