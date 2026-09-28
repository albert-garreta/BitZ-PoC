//! Their `pcs` crate over this crate's flock engine: the checked commit, the
//! statement frames, the inner-product sumcheck, the ring switch and the
//! Ligerito opening, with flock's challenger events framed their way.

use core::mem::size_of;
use std::borrow::Cow;

use bincode::Options;
use flock_core::challenger::Challenger;
use field::Gf128 as FlockF128;
use flock_core::merkle::{Hash, HashKind};
use flock_core::pcs::commit::PcsParams;
use flock_core::pcs::ligerito::{
    LigeritoProfile, LigeritoProof, LigeritoSecurityConfig, ProverConfig, VerifierConfig,
    embedded_security_config, recursive_prover_with_basis, recursive_verifier_with_basis_succinct,
};
use flock_core::pcs::pack::{LOG_PACKING, PACKING_WIDTH as CLAIM_COUNT};
use flock_core::pcs::ring_switch::{
    build_eq_split, claim_check, eval_rs_eq_finish_from_prefix_binary_q, eval_rs_eq_prefix,
    fold_1b_rows_naive, fold_1b_rows_split, fold_b128_elems, fold_b128_elems_split,
    inner_product, split_n_lo, tensor_algebra_transpose,
};
use flock_core::zerocheck::PaddingSpec;
use flock_core::zerocheck::univariate_skip::build_eq;
use spongefish::Encoding;

use super::colstats::ZStats;
use super::params::{LinearClaimGf, SumClaimGf, Root, Shape};
use super::sumcheck;
use super::transcript::{ProverState, PublicTranscript, VerifierState};
use crate::ligerito_flock::{
    FlockCommitHint, add_ood_basis, commit_rs_ligerito_rows, ood_residual_evals,
};
use field::Gf128 as Gf;

/// flock's field element is this crate's `Gf128`: the conversions are the identity.
#[inline]
fn f128_to_gf(f: FlockF128) -> Gf {
    f
}

#[inline]
fn gf_to_f128(g: Gf) -> FlockF128 {
    g
}

const MLE_STATEMENT_LABEL: &[u8] = b"bitz/pcs/mle-opening/v1";
const INNER_PRODUCT_STATEMENT_LABEL: &[u8] = b"bitz/pcs/bit-inner-product/v2";
const INNER_PRODUCT_SUM_STATEMENT_LABEL: &[u8] = b"bitz/pcs/bit-inner-product-sum/v1";
const SUMCHECK_LABEL: &[u8] = b"bitz/pcs/inner-product-sumcheck/v1";
const MLE_CLAIMS_LABEL: &[u8] = b"bitz/pcs/mle-claims/v1";
const CHALLENGES_LABEL: &[u8] = b"bitz/pcs/ring-switch-challenges/v1";
/// The composed caller's Round-0 claim, batched into the Ligerito basis
/// (this crate's addition; absent in their protocol).
const OOD_CLAIM_LABEL: &[u8] = b"bitz/wfbitz/round0-claim/v1";
const PROOF_HINT_LIMIT: usize = 64 * 1024 * 1024;

const VECTOR_SQUEEZE_TAG: &[u8] = b"pcs/flock/sample-vector/v1";
const POW_TAG: &[u8] = b"pcs/flock/pow/v1";
const LIGERITO_BASIS_LABEL: &[u8] = b"flock-ligerito-basis-v0";
const MAX_OBSERVED_BYTES: usize = 32;

/// Errors from PCS configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Invalid(&'static str),
}

/// Errors from commitment creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    RowsShapeMismatch,
}

/// Errors from opening proof creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProveError {
    PackedWitnessLengthMismatch,
    WeightLengthMismatch,
    PointLengthMismatch,
    ProverDataMismatch,
    InvalidClaim,
    SerializationFailed,
    ProofTooLarge,
    Internal,
}

/// Errors from opening proof verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    WeightLengthMismatch,
    PointLengthMismatch,
    MalformedProof,
    VerificationFailed,
    Internal,
}

/// Controls statement binding for one opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementBinding {
    /// Binds the PCS parameters, commitment, query and target.
    Bind,
    /// Uses a statement the caller already bound.
    AlreadyBound,
}

/// One opening query: an MLE claim or a factored inner-product claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpeningQuery {
    Mle { point: Vec<Gf>, target: Gf },
    InnerProduct { claim: LinearClaimGf },
    /// A short sum of factored claims (a structured virtual opening's
    /// transposed weights), reduced by [`sumcheck::prove_sum`].
    InnerProductSum { claim: SumClaimGf },
}

/// Their `Pcs`: flock parameters for one bit length, with the checked
/// Ligerito configs. The profile is `Fast` on their k = 4 ladder — the
/// crate's embedded `fast` TOML, the same file their `bitz-k4` branch
/// carries.
#[derive(Clone, Debug)]
pub struct Pcs {
    params: PcsParams,
    prover_config: ProverConfig,
    verifier_config: VerifierConfig,
    final_log_n: usize,
    bit_len: usize,
    packed_len: usize,
}

impl Pcs {
    /// Their scheme as shipped: flock's embedded `fast` ladder for the size.
    pub fn new(shape: &Shape, merkle_hash: HashKind) -> Result<Self, ConfigError> {
        let m = shape.log_bits();
        if m < super::params::MIN_LOG_BITS {
            return Err(ConfigError::Invalid("below the embedded ladders' smallest size"));
        }
        let profile = LigeritoProfile::Fast;
        let security = security_config(m, profile, merkle_hash)?;
        Self::with_security(shape, &security, profile)
    }

    /// The same scheme over any validated flock ladder (the crate's own
    /// resolved selections: unique decoding with fold grinding, a Johnson
    /// ladder at another rate): the commitment is made under the ladder's
    /// level 0 (its rate and interleaving), the opening runs it. `profile`
    /// is the tag the parameter frame carries (the ladder's family).
    pub fn with_security(
        shape: &Shape,
        security: &LigeritoSecurityConfig,
        profile: LigeritoProfile,
    ) -> Result<Self, ConfigError> {
        let m = shape.log_bits();
        if security.m != m {
            return Err(ConfigError::Invalid("security config is for another size"));
        }
        let bit_len = 1usize
            .checked_shl(m as u32)
            .ok_or(ConfigError::Invalid("bit length overflow"))?;
        let merkle_hash = security
            .merkle_hash()
            .map_err(|_| ConfigError::Invalid("merkle hash"))?;
        let log_inv_rate = security
            .levels
            .first()
            .map(|level| level.log_inv_rate)
            .ok_or(ConfigError::Invalid("no levels"))?;
        let params = PcsParams {
            m,
            log_inv_rate,
            log_batch_size: security.initial_k,
            profile,
            merkle_hash,
        };
        let log_n = m
            .checked_sub(LOG_PACKING)
            .ok_or(ConfigError::Invalid("m below packing width"))?;
        let (prover_config, verifier_config) = security
            .to_prover_verifier_configs()
            .map_err(|_| ConfigError::Invalid("prover config"))?;
        validate_pcs_verifier_prover(&params, &prover_config, &verifier_config)?;
        let final_log_n = validate_verifier_config(&verifier_config, log_n, params.log_batch_size)?;
        let packed_len = 1usize
            .checked_shl(log_n as u32)
            .ok_or(ConfigError::Invalid("packed length overflow"))?;
        Ok(Self {
            params,
            prover_config,
            verifier_config,
            final_log_n,
            bit_len,
            packed_len,
        })
    }

    /// The scheme over explicit flock prover/verifier configurations (a
    /// relation that carries its opener as configurations rather than a
    /// security config, like MultiSwap's UDR ladder): the parameter frame
    /// takes the level-0 rate, the initial batch and the hash from the
    /// prover configuration, and both configurations are validated against
    /// the shape as `with_security` validates the derived ones.
    pub fn from_configs(
        shape: &Shape,
        prover_config: ProverConfig,
        verifier_config: VerifierConfig,
    ) -> Result<Self, ConfigError> {
        let m = shape.log_bits();
        let bit_len = 1usize
            .checked_shl(m as u32)
            .ok_or(ConfigError::Invalid("bit length overflow"))?;
        let log_inv_rate = *prover_config
            .log_inv_rates
            .first()
            .ok_or(ConfigError::Invalid("no levels"))?;
        let params = PcsParams {
            m,
            log_inv_rate,
            log_batch_size: prover_config.initial_k,
            profile: LigeritoProfile::Fast,
            merkle_hash: prover_config.merkle_hash,
        };
        let log_n = m
            .checked_sub(LOG_PACKING)
            .ok_or(ConfigError::Invalid("m below packing width"))?;
        validate_pcs_verifier_prover(&params, &prover_config, &verifier_config)?;
        let final_log_n = validate_verifier_config(&verifier_config, log_n, params.log_batch_size)?;
        let packed_len = 1usize
            .checked_shl(log_n as u32)
            .ok_or(ConfigError::Invalid("packed length overflow"))?;
        Ok(Self {
            params,
            prover_config,
            verifier_config,
            final_log_n,
            bit_len,
            packed_len,
        })
    }

    /// Commits the per-column bit rows (their packed witness in this
    /// crate's row layout) with flock, under this scheme's parameters.
    pub fn commit(
        &self,
        shape: &Shape,
        rows: Vec<Vec<u64>>,
    ) -> Result<(Root, FlockCommitHint), CommitError> {
        if shape.log_bits() != self.params.m
            || rows.len() != shape.columns()
            || rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(CommitError::RowsShapeMismatch);
        }
        let hint = commit_rs_ligerito_rows(&shape.layout(), rows, &self.prover_config);
        let root = Root(*hint.root());
        Ok((root, hint))
    }

    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    pub fn packed_len(&self) -> usize {
        self.packed_len
    }

    pub fn params(&self) -> &PcsParams {
        &self.params
    }

    pub fn prover_config(&self) -> &ProverConfig {
        &self.prover_config
    }

    pub fn verifier_config(&self) -> &VerifierConfig {
        &self.verifier_config
    }

    /// Their `CommitScheme::prove_lin`.
    ///
    /// `ood`: an out-of-domain claim `MLE[P](point) = y` on the packed
    /// message, bound by the caller before any of its challenges (the
    /// crate's Round 0), to batch into the opening; `None` = as shipped.
    pub fn prove_lin(
        &self,
        hint: &FlockCommitHint,
        query: &OpeningQuery,
        statement_binding: StatementBinding,
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), ProveError> {
        self.prove_lin_rows(
            hint,
            hint.rows(),
            hint.packed_cols(),
            query,
            statement_binding,
            transcript,
            ood,
        )
    }

    /// [`Pcs::prove_lin`] with the committed bits presented in the claim's
    /// shape: `rows` (per-column bit rows, 64 bits per word) and, when the
    /// claim has more than a few columns, their 64-lane packing (empty
    /// otherwise). The direct scheme passes the hint's own rows; a virtual
    /// opening passes the committed bits as the one row of a single-column
    /// claim. The bits are the hint's either way, only regrouped.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_lin_rows(
        &self,
        hint: &FlockCommitHint,
        rows: &[Vec<u64>],
        packed_cols: &[Vec<u64>],
        query: &OpeningQuery,
        statement_binding: StatementBinding,
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), ProveError> {
        self.prove_lin_rows_with(
            hint,
            rows,
            packed_cols,
            query,
            statement_binding,
            transcript,
            ood,
            OpeningPath::from_env(),
        )
    }

    /// [`Pcs::prove_lin_rows`] along an explicit [`OpeningPath`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_lin_rows_with(
        &self,
        hint: &FlockCommitHint,
        rows: &[Vec<u64>],
        packed_cols: &[Vec<u64>],
        query: &OpeningQuery,
        statement_binding: StatementBinding,
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
        path: OpeningPath,
    ) -> Result<(), ProveError> {
        if hint.packed_message().len() != self.packed_len {
            return Err(ProveError::PackedWitnessLengthMismatch);
        }
        validate_prover_data(self, hint)?;
        let root = *hint.root();
        match query {
            OpeningQuery::Mle { point, target } => {
                let ring_switch = RingSwitch::new(point, self.params.m)?;
                if statement_binding == StatementBinding::Bind {
                    bind_mle_statement(self, &root, point, *target, transcript);
                }
                self.prove_mle(hint, &ring_switch, *target, transcript, ood, path, None)
            }
            OpeningQuery::InnerProduct { claim } => {
                validate_inner_product_claim(self, claim)?;
                if statement_binding == StatementBinding::Bind {
                    let started = std::time::Instant::now();
                    bind_inner_product_statement(self, &root, claim, transcript);
                    super::trace("  bind statement", started);
                }
                transcript.public_message(SUMCHECK_LABEL);
                let started = std::time::Instant::now();
                // The column statistics give the ring switch's claims only on
                // the committed rows themselves (the packed message is theirs).
                let column_stats = core::ptr::eq(rows, hint.rows())
                    && path.zstats.applies(
                        claim.row_weights().len().ilog2() as usize,
                        claim.column_weights().len().ilog2() as usize,
                    );
                let reduced = sumcheck::prove_with(claim, rows, packed_cols, transcript, column_stats)?;
                super::trace("  sumcheck", started);
                let ring_switch = RingSwitch::new(&reduced.point, self.params.m)?;
                bind_mle_statement(self, &root, &reduced.point, reduced.target, transcript);
                let claims = reduced.ring_switch_claims;
                self.prove_mle(hint, &ring_switch, reduced.target, transcript, ood, path, claims)
            }
            OpeningQuery::InnerProductSum { claim } => {
                validate_sum_claim(self, claim)?;
                if statement_binding == StatementBinding::Bind {
                    bind_sum_statement(self, &root, claim, transcript);
                }
                transcript.public_message(SUMCHECK_LABEL);
                let started = std::time::Instant::now();
                let reduced = sumcheck::prove_sum(claim, rows, packed_cols, transcript)?;
                super::trace("  sumcheck (sum)", started);
                let ring_switch = RingSwitch::new(&reduced.point, self.params.m)?;
                bind_mle_statement(self, &root, &reduced.point, reduced.target, transcript);
                self.prove_mle(hint, &ring_switch, reduced.target, transcript, ood, path, None)
            }
        }
    }

    /// Their `CommitScheme::verify_lin`.
    pub fn verify_lin(
        &self,
        commitment: &Root,
        query: &OpeningQuery,
        statement_binding: StatementBinding,
        transcript: &mut VerifierState<'_>,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), VerifyError> {
        match query {
            OpeningQuery::Mle { point, target } => {
                let ring_switch = RingSwitch::new(point, self.params.m)?;
                if statement_binding == StatementBinding::Bind {
                    bind_mle_statement(self, &commitment.0, point, *target, transcript);
                }
                self.verify_mle(commitment, &ring_switch, *target, transcript, ood)
            }
            OpeningQuery::InnerProduct { claim } => {
                validate_inner_product_claim(self, claim)?;
                if statement_binding == StatementBinding::Bind {
                    bind_inner_product_statement(self, &commitment.0, claim, transcript);
                }
                transcript.public_message(SUMCHECK_LABEL);
                let reduced = sumcheck::verify(claim, transcript)?;
                let ring_switch = RingSwitch::new(&reduced.point, self.params.m)?;
                bind_mle_statement(self, &commitment.0, &reduced.point, reduced.target, transcript);
                self.verify_mle(commitment, &ring_switch, reduced.target, transcript, ood)
            }
            OpeningQuery::InnerProductSum { claim } => {
                validate_sum_claim(self, claim)?;
                if statement_binding == StatementBinding::Bind {
                    bind_sum_statement(self, &commitment.0, claim, transcript);
                }
                transcript.public_message(SUMCHECK_LABEL);
                let reduced = sumcheck::verify_sum(claim, transcript)?;
                let ring_switch = RingSwitch::new(&reduced.point, self.params.m)?;
                bind_mle_statement(self, &commitment.0, &reduced.point, reduced.target, transcript);
                self.verify_mle(commitment, &ring_switch, reduced.target, transcript, ood)
            }
        }
    }

    /// Their `prove_mle`: the 128 ring-switch claims, seven batching
    /// challenges, the dense packed basis, then Ligerito. `claims`: the
    /// ring-switch claims when the sumcheck already has them.
    #[allow(clippy::too_many_arguments)]
    fn prove_mle(
        &self,
        hint: &FlockCommitHint,
        ring_switch: &RingSwitch,
        target: Gf,
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
        path: OpeningPath,
        claims: Option<Vec<Gf>>,
    ) -> Result<(), ProveError> {
        let started_rs = std::time::Instant::now();
        let packed = hint.packed_message();
        let started = std::time::Instant::now();
        let suffix_point = &ring_switch.point[LOG_PACKING..];
        let (prefix_tensor, suffix) = if path.split && suffix_point.len() >= SPLIT_MIN_VARS {
            let (lo, hi) = build_eq_split(suffix_point, split_n_lo(suffix_point.len()));
            (build_eq(&ring_switch.point[..LOG_PACKING]), SuffixEq::Split { lo, hi })
        } else {
            let (prefix, tensor) = build_eq_split(&ring_switch.point, LOG_PACKING);
            (prefix, SuffixEq::Dense(tensor))
        };
        super::trace("    rs eq", started);
        if suffix.len() != packed.len() {
            return Err(ProveError::PackedWitnessLengthMismatch);
        }
        let claims = match claims {
            Some(claims) => claims,
            None => {
                let started = std::time::Instant::now();
                let claims = match &suffix {
                    SuffixEq::Dense(tensor) => fold_1b_rows_naive(packed, tensor),
                    SuffixEq::Split { lo, hi } => {
                        fold_1b_rows_split(packed, lo, hi, &PaddingSpec::dense(self.params.m))
                    }
                };
                super::trace("    rs fold 1b", started);
                claims
            }
        };
        let claims: [FlockF128; CLAIM_COUNT] = claims
            .try_into()
            .map_err(|_: Vec<FlockF128>| ProveError::Internal)?;
        if claim_check(&prefix_tensor, &claims) != gf_to_f128(target) {
            return Err(ProveError::InvalidClaim);
        }

        // write_claims
        transcript.public_message(MLE_CLAIMS_LABEL);
        let claims_gf: [Gf; CLAIM_COUNT] = core::array::from_fn(|index| f128_to_gf(claims[index]));
        transcript.prover_message(&claims_gf);
        let batching_point = sample_challenges(transcript);

        // reduce_dense
        let started = std::time::Instant::now();
        let batching_weights = build_eq(&batching_point);
        let mut packed_target = batch_claims(&claims, &batching_weights);
        let mut packed_basis = match &suffix {
            SuffixEq::Dense(tensor) => fold_b128_elems(tensor, &batching_weights),
            SuffixEq::Split { lo, hi } => fold_b128_elems_split(lo, hi, &batching_weights),
        };
        debug_assert_eq!(packed_basis.len(), suffix.len());
        super::trace("    rs basis", started);

        // The caller's Round-0 claim joins the batch: `η_ood·eq(·, point)`
        // into the basis, `η_ood·y` into the target.
        if let Some((point, y)) = ood {
            if point.len() != ring_switch.suffix_dimension() {
                return Err(ProveError::Internal);
            }
            let started = std::time::Instant::now();
            let eta = bind_ood_claim(transcript, point, y);
            add_ood_basis(&mut packed_basis, packed, point, eta, None);
            packed_target = packed_target + eta * y;
            super::trace("    rs ood", started);
        }

        super::trace("  ring switch", started_rs);

        // ReducedProver::prove
        let started_lig = std::time::Instant::now();
        let data = hint.flock_prover_data();
        let mut challenger = ProverChallenger::new_ligerito(transcript, packed_target);
        let witness: Cow<'_, [FlockF128]> = if path.borrow {
            Cow::Borrowed(packed)
        } else {
            Cow::Owned(packed.to_vec())
        };
        let ligerito = recursive_prover_with_basis(
            &self.prover_config,
            witness,
            packed_basis,
            packed_target,
            &data.codeword,
            &data.merkle_tree,
            &mut challenger,
        );
        if challenger.failed() {
            return Err(ProveError::Internal);
        }
        super::trace_elapsed("    lig pow grind", challenger.grinding);
        super::trace("  ligerito", started_lig);
        write_opening_proof(&ligerito, transcript)
    }

    /// Their `verify_mle`.
    fn verify_mle(
        &self,
        commitment: &Root,
        ring_switch: &RingSwitch,
        target: Gf,
        transcript: &mut VerifierState<'_>,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), VerifyError> {
        let proof = read_opening_proof(transcript)?;
        validate_ligerito_proof_shape(&proof, &self.verifier_config, self.final_log_n, &commitment.0)?;

        // read_claims
        transcript.public_message(MLE_CLAIMS_LABEL);
        let claims_gf = transcript
            .prover_message::<[Gf; CLAIM_COUNT]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        let claims: [FlockF128; CLAIM_COUNT] = claims_gf.map(gf_to_f128);

        // target_matches
        let prefix_tensor = build_eq(&ring_switch.point[..LOG_PACKING]);
        if claim_check(&prefix_tensor, &claims) != gf_to_f128(target) {
            return Err(VerifyError::VerificationFailed);
        }
        let batching_point = sample_challenges(transcript);

        // reduce_succinct
        let batching_weights = build_eq(&batching_point);
        let mut packed_target = batch_claims(&claims, &batching_weights);
        let suffix_point = &ring_switch.point[LOG_PACKING..];
        let ood = match ood {
            Some((point, y)) => {
                if point.len() != ring_switch.suffix_dimension() {
                    return Err(VerifyError::VerificationFailed);
                }
                let eta = bind_ood_claim(transcript, point, y);
                packed_target = packed_target + eta * y;
                Some((point, eta))
            }
            None => None,
        };
        let evaluate_basis = |ris: &[FlockF128], yr_log_n: usize| -> Vec<FlockF128> {
            if yr_log_n > 32 || ris.len().checked_add(yr_log_n) != Some(suffix_point.len()) {
                return Vec::new();
            }
            let Some(yr_len) = 1usize.checked_shl(yr_log_n as u32) else {
                return Vec::new();
            };
            let prefix = eval_rs_eq_prefix(suffix_point, ris);
            let suffix = &suffix_point[ris.len()..];
            let mut out: Vec<FlockF128> = (0..yr_len)
                .map(|y| {
                    eval_rs_eq_finish_from_prefix_binary_q(&prefix, suffix, y as u32, &batching_weights)
                })
                .collect();
            if let Some((point, eta)) = ood {
                for (slot, term) in out.iter_mut().zip(ood_residual_evals(ris, yr_log_n, point, eta)) {
                    *slot = *slot + term;
                }
            }
            out
        };

        // verify_succinct
        let mut challenger = VerifierChallenger::new_ligerito(transcript, packed_target);
        let valid = recursive_verifier_with_basis_succinct(
            &self.verifier_config,
            &proof,
            ring_switch.suffix_dimension(),
            packed_target,
            &commitment.0,
            evaluate_basis,
            &mut challenger,
        );
        if challenger.failed() {
            return Err(VerifyError::MalformedProof);
        }
        if !valid {
            return Err(VerifyError::VerificationFailed);
        }
        Ok(())
    }
}

/// `MLE[P](point)` of the committed packed message `P` (`m − 7`
/// coordinates, low first): the value an out-of-domain (Round-0) claim at
/// `point` carries into [`Pcs::prove_lin`]'s `ood`.
pub fn packed_evaluation(hint: &FlockCommitHint, point: &[Gf]) -> Gf {
    crate::ligerito_flock::ood_eval(hint.packed_message(), point)
}

/// How [`Pcs::prove_lin`] runs the ring switch and hands the opening to
/// Ligerito. Every path proves the same bytes (`opening_paths_prove_
/// identically` pins them against each other); they differ only in
/// passes, memory traffic and allocations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpeningPath {
    /// Ligerito reads the committed packed message in place (its prover
    /// takes a `Cow`) instead of a fresh copy: no serial 32/128 MB memcpy
    /// and first touch per proof at `n = 28/30`, and no copy handed on to
    /// flock's scratch pool after the first fold (`WFBITZ_RS_BORROW`,
    /// default on).
    pub(crate) borrow: bool,
    /// `eq(·, point[7..])` stays two factors of `2^{⌊(m−7)/2⌋}` and
    /// `2^{⌈(m−7)/2⌉}` entries (flock's `build_eq_split` at `split_n_lo`)
    /// instead of the `2^{m−7}`-entry tensor: the claims come from flock's
    /// split fold (the subset-sum tables built once, no tensor stream) and
    /// the basis from its split fold, one multiply per entry, into a pooled
    /// buffer. No tensor build (`2^{m−6}` multiplies), no fresh 32/128 MB
    /// per proof for the tensor or the basis (`WFBITZ_RS_SPLIT`, default
    /// on; needs `m ≥ 11`).
    pub(crate) split: bool,
    /// The inner-product sumcheck computes its row-folded table from the
    /// column statistics ([`super::colstats`]), which also give the 128
    /// claims: one pass over the bits instead of the row fold plus the
    /// ring switch's fold (`WFBITZ_ZSTATS`: `0` off, `1` wherever it fits,
    /// default where it pays).
    pub(crate) zstats: ZStats,
}

impl OpeningPath {
    /// The environment's path, read once.
    pub(crate) fn from_env() -> Self {
        static PATH: std::sync::OnceLock<OpeningPath> = std::sync::OnceLock::new();
        *PATH.get_or_init(|| {
            let flag = |name: &str, default: bool| match std::env::var(name).as_deref() {
                Ok("0") => false,
                Ok("1") => true,
                _ => default,
            };
            OpeningPath {
                borrow: flag("WFBITZ_RS_BORROW", true),
                split: flag("WFBITZ_RS_SPLIT", true),
                zstats: ZStats::from_env(),
            }
        })
    }
}

/// The fewest suffix variables the split takes: flock's split fold walks
/// whole 16-element chunks of the low factor (`split_n_lo ≥ 4`).
const SPLIT_MIN_VARS: usize = 4;

/// `eq(·, point[7..])` over the packed positions: the tensor, or its two
/// factors, entry `j·|lo| + i` being `lo[i]·hi[j]` (exactly: the field
/// multiply is exact, so the product has the tensor entry's bits).
enum SuffixEq {
    Dense(Vec<FlockF128>),
    Split { lo: Vec<FlockF128>, hi: Vec<FlockF128> },
}

impl SuffixEq {
    fn len(&self) -> usize {
        match self {
            Self::Dense(tensor) => tensor.len(),
            Self::Split { lo, hi } => lo.len() * hi.len(),
        }
    }
}

/// Their 40-byte frame: `m`, `log_inv_rate`, `log_batch_size`, the profile
/// tag (`Fast` = 0) and the hash tag (`Sha256` = 0, `Blake3` = 1), each u64.
impl Encoding<[u8]> for Pcs {
    fn encode(&self) -> impl AsRef<[u8]> {
        let profile_tag = match self.params.profile {
            LigeritoProfile::Fast => 0,
            LigeritoProfile::Slim => 1,
            LigeritoProfile::Secure => 2,
            // Not a profile their enum has; never selected here.
            _ => u64::MAX,
        };
        let hash_tag = match self.params.merkle_hash {
            HashKind::Sha256 => 0,
            HashKind::Blake3 => 1,
        };
        let tags = [
            self.params.m as u64,
            self.params.log_inv_rate as u64,
            self.params.log_batch_size as u64,
            profile_tag,
            hash_tag,
        ];
        let mut encoded = [0u8; 5 * size_of::<u64>()];
        for (chunk, tag) in encoded.chunks_exact_mut(size_of::<u64>()).zip(tags) {
            chunk.copy_from_slice(&tag.to_le_bytes());
        }
        encoded
    }
}

/// The security ladder for `(m, profile)` with the commitment's Merkle hash
/// stamped over the TOML's own — their `pcs::ligerito::security_config`
/// on the `bitz-k4` branch, reading the same `fast` TOMLs this crate embeds.
fn security_config(
    m: usize,
    profile: LigeritoProfile,
    merkle_hash: HashKind,
) -> Result<LigeritoSecurityConfig, ConfigError> {
    let toml = embedded_security_config(m, profile)
        .ok_or(ConfigError::Invalid("no security config for this size and profile"))?;
    let mut security = LigeritoSecurityConfig::from_toml_str(toml)
        .map_err(|_| ConfigError::Invalid("security config"))?;
    security.hash = match merkle_hash {
        HashKind::Sha256 => "sha256",
        HashKind::Blake3 => "blake3",
    }
    .to_owned();
    security
        .validate()
        .map_err(|_| ConfigError::Invalid("security config"))?;
    Ok(security)
}

fn validate_pcs_verifier_prover(
    params: &PcsParams,
    prover: &ProverConfig,
    verifier: &VerifierConfig,
) -> Result<(), ConfigError> {
    let shared_fields_match = prover.log_inv_rates == verifier.log_inv_rates
        && prover.recursive_steps == verifier.recursive_steps
        && prover.initial_log_msg_cols == verifier.initial_log_msg_cols
        && prover.initial_log_num_interleaved == verifier.initial_log_num_interleaved
        && prover.initial_k == verifier.initial_k
        && prover.recursive_log_msg_cols == verifier.recursive_log_msg_cols
        && prover.recursive_ks == verifier.recursive_ks
        && prover.queries == verifier.queries
        && prover.grinding_bits == verifier.grinding_bits
        && prover.fold_grinding_bits == verifier.fold_grinding_bits
        && prover.ood_samples == verifier.ood_samples
        && prover.merkle_hash == verifier.merkle_hash;
    if !shared_fields_match {
        return Err(ConfigError::Invalid("prover and verifier differ"));
    }
    if verifier.log_inv_rates.first().copied() != Some(params.log_inv_rate) {
        return Err(ConfigError::Invalid("log_inv_rate mismatch"));
    }
    if verifier.merkle_hash != params.merkle_hash {
        return Err(ConfigError::Invalid("merkle_hash mismatch"));
    }
    Ok(())
}

fn checked_pow2(log: usize) -> Option<usize> {
    u32::try_from(log)
        .ok()
        .and_then(|shift| 1usize.checked_shl(shift))
}

fn valid_query_shape(log_columns: usize, log_rate: usize, queries: usize) -> bool {
    queries > 0
        && log_columns
            .checked_add(log_rate)
            .and_then(checked_pow2)
            .is_some_and(|block_len| queries <= block_len)
}

/// Their `validate_verifier_config`; returns `final_log_n`.
fn validate_verifier_config(
    config: &VerifierConfig,
    log_n: usize,
    expected_initial_k: usize,
) -> Result<usize, ConfigError> {
    let r = config.recursive_steps;
    let level_count = r
        .checked_add(1)
        .ok_or(ConfigError::Invalid("recursive_steps overflow"))?;
    if r == 0 {
        return Err(ConfigError::Invalid("recursive_steps is zero"));
    }
    if config.recursive_ks.len() != r {
        return Err(ConfigError::Invalid("recursive_ks length"));
    }
    if config.recursive_log_msg_cols.len() != r {
        return Err(ConfigError::Invalid("recursive_log_msg_cols length"));
    }
    if config.log_inv_rates.len() != level_count {
        return Err(ConfigError::Invalid("log_inv_rates length"));
    }
    if config.queries.len() != level_count {
        return Err(ConfigError::Invalid("queries length"));
    }
    if config.grinding_bits.len() != level_count {
        return Err(ConfigError::Invalid("grinding_bits length"));
    }
    if config.fold_grinding_bits.len() != level_count {
        return Err(ConfigError::Invalid("fold_grinding_bits length"));
    }
    if config.ood_samples.len() != level_count {
        return Err(ConfigError::Invalid("ood_samples length"));
    }
    if config.initial_k != expected_initial_k {
        return Err(ConfigError::Invalid("initial_k mismatch"));
    }
    if config.initial_log_num_interleaved != config.initial_k {
        return Err(ConfigError::Invalid("initial_log_num_interleaved mismatch"));
    }
    if config.ood_samples[0] != 0 {
        return Err(ConfigError::Invalid("ood_samples[0] is nonzero"));
    }
    if config.log_inv_rates.contains(&0) {
        return Err(ConfigError::Invalid("log_inv_rates contains zero"));
    }
    if config
        .grinding_bits
        .iter()
        .any(|&bits| u32::try_from(bits).is_err())
    {
        return Err(ConfigError::Invalid("grinding_bits exceeds u32"));
    }
    if config
        .fold_grinding_bits
        .iter()
        .any(|&bits| u32::try_from(bits).is_err())
    {
        return Err(ConfigError::Invalid("fold_grinding_bits exceeds u32"));
    }
    let mut remaining = log_n
        .checked_sub(config.initial_k)
        .ok_or(ConfigError::Invalid("initial_k exceeds log_n"))?;
    if config.initial_log_msg_cols != remaining {
        return Err(ConfigError::Invalid("initial_log_msg_cols mismatch"));
    }
    if checked_pow2(config.initial_k).is_none() {
        return Err(ConfigError::Invalid("initial_k exceeds platform width"));
    }
    if !valid_query_shape(remaining, config.log_inv_rates[0], config.queries[0]) {
        return Err(ConfigError::Invalid("queries[0]"));
    }
    for level in 0..r {
        let k = config.recursive_ks[level];
        if k == 0 {
            return Err(ConfigError::Invalid("recursive_ks contains zero"));
        }
        if checked_pow2(k).is_none() {
            return Err(ConfigError::Invalid("recursive_ks exceeds platform width"));
        }
        remaining = remaining
            .checked_sub(k)
            .ok_or(ConfigError::Invalid("recursive_ks exceed log_n"))?;
        if config.recursive_log_msg_cols[level] != remaining {
            return Err(ConfigError::Invalid("recursive_log_msg_cols mismatch"));
        }
        if !valid_query_shape(
            remaining,
            config.log_inv_rates[level + 1],
            config.queries[level + 1],
        ) {
            return Err(ConfigError::Invalid("queries"));
        }
    }
    if remaining > 32 {
        return Err(ConfigError::Invalid("final_log_n exceeds 32"));
    }
    if checked_pow2(remaining).is_none() {
        return Err(ConfigError::Invalid("final_log_n exceeds platform width"));
    }
    Ok(remaining)
}

fn validate_prover_data(pcs: &Pcs, hint: &FlockCommitHint) -> Result<(), ProveError> {
    let expected = &pcs.params;
    let actual = &hint.commitment.params;
    if expected.m != actual.m
        || expected.log_inv_rate != actual.log_inv_rate
        || expected.log_batch_size != actual.log_batch_size
        || expected.profile != actual.profile
        || expected.merkle_hash != actual.merkle_hash
    {
        return Err(ProveError::ProverDataMismatch);
    }
    Ok(())
}

fn validate_inner_product_claim(pcs: &Pcs, claim: &LinearClaimGf) -> Result<(), QueryError> {
    if claim
        .row_weights()
        .len()
        .checked_mul(claim.column_weights().len())
        != Some(pcs.bit_len)
    {
        return Err(QueryError::WeightLengthMismatch);
    }
    Ok(())
}

fn validate_sum_claim(pcs: &Pcs, claim: &SumClaimGf) -> Result<(), QueryError> {
    if claim.rows().checked_mul(claim.columns()) != Some(pcs.bit_len) {
        return Err(QueryError::WeightLengthMismatch);
    }
    Ok(())
}

/// Query validation shared by prover and verifier entry points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum QueryError {
    WeightLengthMismatch,
    PointLengthMismatch,
    Internal,
}

impl From<QueryError> for ProveError {
    fn from(error: QueryError) -> Self {
        match error {
            QueryError::WeightLengthMismatch => Self::WeightLengthMismatch,
            QueryError::PointLengthMismatch => Self::PointLengthMismatch,
            QueryError::Internal => Self::Internal,
        }
    }
}

impl From<QueryError> for VerifyError {
    fn from(error: QueryError) -> Self {
        match error {
            QueryError::WeightLengthMismatch => Self::WeightLengthMismatch,
            QueryError::PointLengthMismatch => Self::PointLengthMismatch,
            QueryError::Internal => Self::Internal,
        }
    }
}

// ---------------------------------------------------------------------
// Statement frames and the ring switch
// ---------------------------------------------------------------------

fn bind_mle_statement(
    pcs: &Pcs,
    root: &[u8; 32],
    point: &[Gf],
    target: Gf,
    transcript: &mut impl PublicTranscript,
) {
    transcript.public_message(MLE_STATEMENT_LABEL);
    transcript.public_message(root);
    transcript.public_message(pcs);
    transcript.public_message(&(point.len() as u64));
    for coordinate in point {
        transcript.public_message(coordinate);
    }
    transcript.public_message(&target);
}

fn bind_inner_product_statement(
    pcs: &Pcs,
    root: &[u8; 32],
    claim: &LinearClaimGf,
    transcript: &mut impl PublicTranscript,
) {
    transcript.public_message(INNER_PRODUCT_STATEMENT_LABEL);
    transcript.public_message(root);
    transcript.public_message(pcs);
    transcript.public_message(claim);
}

fn bind_sum_statement(
    pcs: &Pcs,
    root: &[u8; 32],
    claim: &SumClaimGf,
    transcript: &mut impl PublicTranscript,
) {
    transcript.public_message(INNER_PRODUCT_SUM_STATEMENT_LABEL);
    transcript.public_message(root);
    transcript.public_message(pcs);
    transcript.public_message(claim);
}

/// Binds the caller's Round-0 claim (the point, the value) and draws its
/// batching scalar `η_ood`.
fn bind_ood_claim(transcript: &mut impl PublicTranscript, point: &[Gf], y: Gf) -> Gf {
    transcript.public_message(OOD_CLAIM_LABEL);
    transcript.public_message(&(point.len() as u64));
    for coordinate in point {
        transcript.public_message(coordinate);
    }
    transcript.public_message(&y);
    transcript.verifier_message_f128()
}

/// Samples the seven MLE ring-switch challenges.
fn sample_challenges(transcript: &mut impl PublicTranscript) -> [FlockF128; LOG_PACKING] {
    transcript.public_message(CHALLENGES_LABEL);
    core::array::from_fn(|_| gf_to_f128(transcript.verifier_message_f128()))
}

fn batch_claims(claims: &[FlockF128; CLAIM_COUNT], batching_weights: &[FlockF128]) -> FlockF128 {
    debug_assert_eq!(batching_weights.len(), CLAIM_COUNT);
    let transposed_claims = tensor_algebra_transpose(claims);
    inner_product(&transposed_claims, batching_weights)
}

/// A validated MLE point in flock's representation.
struct RingSwitch {
    point: Vec<FlockF128>,
}

impl RingSwitch {
    fn new(point: &[Gf], variable_count: usize) -> Result<Self, QueryError> {
        if point.len() != variable_count {
            return Err(QueryError::PointLengthMismatch);
        }
        if variable_count < LOG_PACKING {
            return Err(QueryError::Internal);
        }
        Ok(Self {
            point: point.iter().map(|&g| gf_to_f128(g)).collect(),
        })
    }

    fn suffix_dimension(&self) -> usize {
        self.point.len() - LOG_PACKING
    }
}

// ---------------------------------------------------------------------
// The Ligerito proof as a hint
// ---------------------------------------------------------------------

fn proof_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(PROOF_HINT_LIMIT as u64)
        .reject_trailing_bytes()
}

fn write_opening_proof(proof: &LigeritoProof, transcript: &mut ProverState) -> Result<(), ProveError> {
    let proof_bytes = proof_options().serialize(proof).map_err(|error| {
        if matches!(error.as_ref(), bincode::ErrorKind::SizeLimit) {
            ProveError::ProofTooLarge
        } else {
            ProveError::SerializationFailed
        }
    })?;
    transcript.hint_bytes(&proof_bytes);
    Ok(())
}

fn read_opening_proof(transcript: &mut VerifierState<'_>) -> Result<LigeritoProof, VerifyError> {
    let proof_bytes = transcript
        .hint_bytes(PROOF_HINT_LIMIT)
        .map_err(|_| VerifyError::MalformedProof)?;
    proof_options()
        .deserialize(&proof_bytes)
        .map_err(|_| VerifyError::MalformedProof)
}

fn rows_match(rows: &[Vec<FlockF128>], expected_rows: usize, expected_width: usize) -> bool {
    rows.len() == expected_rows && rows.iter().all(|row| row.len() == expected_width)
}

fn positive_fold_nonce_count(config: &VerifierConfig) -> Result<usize, VerifyError> {
    let initial = config.initial_k.min(config.fold_grinding_bits[0]);
    config
        .recursive_ks
        .iter()
        .zip(config.fold_grinding_bits.iter().skip(1))
        .try_fold(initial, |sum, (&k, &bits)| sum.checked_add(k.min(bits)))
        .ok_or(VerifyError::Internal)
}

/// Their `validate_ligerito_proof_shape`: every prover-supplied dimension
/// against the config, before the replay.
fn validate_ligerito_proof_shape(
    lig: &LigeritoProof,
    config: &VerifierConfig,
    final_log_n: usize,
    expected_root: &Hash,
) -> Result<(), VerifyError> {
    if &lig.initial_root != expected_root {
        return Err(VerifyError::VerificationFailed);
    }
    let r = config.recursive_steps;
    if lig.recursive_roots.len() != r
        || lig.recursive_proofs.len() != r - 1
        || lig.grinding_nonces.len() != r + 1
    {
        return Err(VerifyError::VerificationFailed);
    }
    let expected_ood = config
        .ood_samples
        .iter()
        .skip(1)
        .try_fold(0usize, |sum, &count| sum.checked_add(count))
        .ok_or(VerifyError::Internal)?;
    let expected_fold_nonces = positive_fold_nonce_count(config)?;
    let initial_sumchecks = 1usize
        .checked_add(config.initial_k)
        .ok_or(VerifyError::Internal)?;
    let expected_sumchecks = config
        .recursive_ks
        .iter()
        .try_fold(initial_sumchecks, |sum, &k| sum.checked_add(k))
        .and_then(|sum| sum.checked_add(r))
        .and_then(|sum| sum.checked_add(expected_ood))
        .ok_or(VerifyError::Internal)?;
    if lig.ood_values.len() != expected_ood
        || lig.fold_grinding_nonces.len() != expected_fold_nonces
        || lig.sumcheck_transcript.len() != expected_sumchecks
    {
        return Err(VerifyError::VerificationFailed);
    }
    let initial_width = checked_pow2(config.initial_k).ok_or(VerifyError::Internal)?;
    if !rows_match(&lig.initial_proof.opened_rows, config.queries[0], initial_width) {
        return Err(VerifyError::VerificationFailed);
    }
    for (level, recursive) in lig.recursive_proofs.iter().enumerate() {
        let width = checked_pow2(config.recursive_ks[level]).ok_or(VerifyError::Internal)?;
        if !rows_match(&recursive.opened_rows, config.queries[level + 1], width) {
            return Err(VerifyError::VerificationFailed);
        }
    }
    let last_k = *config.recursive_ks.last().ok_or(VerifyError::Internal)?;
    let final_width = checked_pow2(last_k).ok_or(VerifyError::Internal)?;
    let final_yr_len = checked_pow2(final_log_n).ok_or(VerifyError::Internal)?;
    if !rows_match(&lig.final_proof.opened_rows, config.queries[r], final_width)
        || lig.final_proof.yr.len() != final_yr_len
    {
        return Err(VerifyError::VerificationFailed);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Flock challenger adapters over the project transcript (their
// `pcs::challenger`)
// ---------------------------------------------------------------------

#[derive(Clone, Copy)]
struct OpeningTargetPrefix {
    expected_target: FlockF128,
    label_seen: bool,
}

pub(crate) struct ProverChallenger<'a> {
    transcript: &'a mut ProverState,
    failed: bool,
    opening_target: Option<OpeningTargetPrefix>,
    /// The wall time of every proof-of-work grind so far (diagnostic).
    grinding: std::time::Duration,
}

impl<'a> ProverChallenger<'a> {
    pub(crate) fn new_ligerito(transcript: &'a mut ProverState, expected_target: FlockF128) -> Self {
        Self {
            transcript,
            failed: false,
            opening_target: Some(OpeningTargetPrefix {
                expected_target,
                label_seen: false,
            }),
            grinding: std::time::Duration::ZERO,
        }
    }

    pub(crate) fn failed(&self) -> bool {
        self.failed || self.opening_target.is_some()
    }
}

pub(crate) struct VerifierChallenger<'a, 'proof> {
    transcript: &'a mut VerifierState<'proof>,
    failed: bool,
    opening_target: Option<OpeningTargetPrefix>,
}

impl<'a, 'proof> VerifierChallenger<'a, 'proof> {
    pub(crate) fn new_ligerito(
        transcript: &'a mut VerifierState<'proof>,
        expected_target: FlockF128,
    ) -> Self {
        Self {
            transcript,
            failed: false,
            opening_target: Some(OpeningTargetPrefix {
                expected_target,
                label_seen: false,
            }),
        }
    }

    pub(crate) fn failed(&self) -> bool {
        self.failed || self.opening_target.is_some()
    }

    fn read<T>(&mut self) -> Option<T>
    where
        T: Encoding<[u8]> + spongefish::NargDeserialize,
    {
        match self.transcript.prover_message() {
            Ok(value) => Some(value),
            Err(_) => {
                self.failed = true;
                None
            }
        }
    }
}

impl Challenger for ProverChallenger<'_> {
    fn observe_label(&mut self, label: &[u8]) {
        if let Some(prefix) = &mut self.opening_target
            && !prefix.label_seen
        {
            if label != LIGERITO_BASIS_LABEL {
                self.failed = true;
            }
            prefix.label_seen = true;
        }
        self.transcript.public_message(label);
    }

    fn observe_f128(&mut self, value: FlockF128) {
        if let Some(prefix) = self.opening_target.take() {
            if !prefix.label_seen || value != prefix.expected_target {
                self.failed = true;
            }
            self.transcript.public_message(&f128_to_gf(value));
            return;
        }
        self.transcript.prover_message(&f128_to_gf(value));
    }

    fn observe_f128_slice(&mut self, values: &[FlockF128]) {
        self.transcript.prover_message(
            &u32::try_from(values.len()).expect("observed field slice exceeds u32"),
        );
        for &value in values {
            self.observe_f128(value);
        }
    }

    fn observe_bytes(&mut self, bytes: &[u8]) {
        self.transcript.prover_message_bytes(bytes);
    }

    fn sample_f128(&mut self) -> FlockF128 {
        gf_to_f128(self.transcript.verifier_message::<Gf>())
    }

    fn sample_f128_vec(&mut self, n: usize) -> Vec<FlockF128> {
        self.transcript.public_message(VECTOR_SQUEEZE_TAG);
        self.transcript.public_message(&(n as u64));
        (0..n).map(|_| self.sample_f128()).collect()
    }

    fn grind_pow(&mut self, bits: u32) -> u64 {
        self.transcript.public_message(POW_TAG);
        self.transcript.public_message(&bits);
        let seed = super::codec::gf_to_bytes(self.transcript.verifier_message::<Gf>());
        let started = std::time::Instant::now();
        let nonce = find_pow(&seed, bits);
        self.grinding += started.elapsed();
        self.transcript.prover_message(&nonce.to_le_bytes());
        nonce
    }

    fn verify_pow(&mut self, _nonce: u64, _bits: u32) -> bool {
        unreachable!("the prover challenger cannot verify proof of work")
    }
}

impl Challenger for VerifierChallenger<'_, '_> {
    fn observe_label(&mut self, label: &[u8]) {
        if let Some(prefix) = &mut self.opening_target
            && !prefix.label_seen
        {
            if label != LIGERITO_BASIS_LABEL {
                self.failed = true;
            }
            prefix.label_seen = true;
        }
        self.transcript.public_message(label);
    }

    fn observe_f128(&mut self, value: FlockF128) {
        if let Some(prefix) = self.opening_target.take() {
            if !prefix.label_seen || value != prefix.expected_target {
                self.failed = true;
            }
            self.transcript.public_message(&f128_to_gf(value));
            return;
        }
        if self.read::<Gf>() != Some(f128_to_gf(value)) {
            self.failed = true;
        }
    }

    fn observe_f128_slice(&mut self, values: &[FlockF128]) {
        let len = u32::try_from(values.len()).expect("observed field slice exceeds u32");
        if self.read::<u32>() != Some(len) {
            self.failed = true;
        }
        for &value in values {
            self.observe_f128(value);
        }
    }

    fn observe_bytes(&mut self, bytes: &[u8]) {
        match self.transcript.prover_message_bytes::<MAX_OBSERVED_BYTES>() {
            Ok(observed) if observed == bytes => {}
            Ok(_) | Err(_) => self.failed = true,
        }
    }

    fn sample_f128(&mut self) -> FlockF128 {
        gf_to_f128(self.transcript.verifier_message::<Gf>())
    }

    fn sample_f128_vec(&mut self, n: usize) -> Vec<FlockF128> {
        self.transcript.public_message(VECTOR_SQUEEZE_TAG);
        self.transcript.public_message(&(n as u64));
        (0..n).map(|_| self.sample_f128()).collect()
    }

    fn grind_pow(&mut self, _bits: u32) -> u64 {
        unreachable!("the verifier challenger cannot grind proof of work")
    }

    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        self.transcript.public_message(POW_TAG);
        self.transcript.public_message(&bits);
        let seed = super::codec::gf_to_bytes(self.transcript.verifier_message::<Gf>());
        let encoded = self.read::<[u8; 8]>().map(u64::from_le_bytes);
        let matches_stream = encoded == Some(nonce);
        let valid = pow_valid(&seed, nonce, bits);
        if !matches_stream || !valid {
            self.failed = true;
        }
        matches_stream && valid
    }
}

/// The smallest nonce that passes [`pow_valid`] — exactly what a serial
/// scan from zero returns, found by the crate's multi-lane BLAKE3 scan
/// across the thread pool ([`crate::utils::blake3x4::smallest_pow_nonce`]:
/// the same `blake3(prefix ‖ nonce_le)`, every chunk below the hit's
/// scanned to completion, so the result does not depend on scheduling).
fn find_pow(seed: &[u8; 16], bits: u32) -> u64 {
    if bits == 0 {
        return 0;
    }
    let mut prefix = [0u8; POW_PREFIX.len() + 16];
    prefix[..POW_PREFIX.len()].copy_from_slice(POW_PREFIX);
    prefix[POW_PREFIX.len()..].copy_from_slice(seed);
    #[cfg(feature = "parallel")]
    let found = crate::utils::blake3x4::smallest_pow_nonce(&prefix, bits);
    #[cfg(not(feature = "parallel"))]
    let found = crate::utils::blake3x4::first_pow_nonce(&prefix, 0, u64::MAX, bits);
    let nonce = found.expect("proof-of-work nonce exhausted");
    debug_assert!(pow_valid(seed, nonce, bits));
    nonce
}

/// The proof-of-work domain prefix: their `bitz-pcs-pow-v1` padded by one
/// zero byte, so the 32-byte `prefix ‖ seed` is whole 32-bit words and the
/// crate's scan ([`crate::utils::blake3x4`]) takes its NEON/AVX2 lanes
/// instead of the reference hash (their 31-byte prefix ran every query and
/// fold grind on the scalar path: 2-3 ms per proof at any size, most of
/// the opening below n = 24). Not byte-compatible with worldfnd's PoW.
const POW_PREFIX: &[u8] = b"bitz-pcs-pow-v1\0";

fn pow_valid(seed: &[u8; 16], nonce: u64, bits: u32) -> bool {
    if bits == 0 {
        return nonce == 0;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(POW_PREFIX);
    hasher.update(seed);
    hasher.update(&nonce.to_le_bytes());
    let digest = hasher.finalize();
    leading_zero_bits(digest.as_bytes()) >= bits
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0;
    for byte in bytes {
        let zeros = byte.leading_zeros();
        total += zeros;
        if zeros != 8 {
            break;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::params::{LinearClaimGf, MIN_LOG_BITS, Shape};
    use crate::wfbitz::{build_prover, build_verifier};

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn random_gf(state: &mut u64) -> Gf {
        Gf::new(xorshift(state), xorshift(state))
    }

    /// A ladder for grids below the embedded ones' floor: `initial_k`
    /// lane folds at rate 1/2, then folds of two with the rate halving per
    /// level down to at most three variables, 16 queries a level, no
    /// grinding. Small, not secure: it only has to run every code path.
    fn small_pcs(shape: &Shape) -> Pcs {
        let log_n = shape.log_bits() - LOG_PACKING;
        let initial_k = if log_n >= 10 { 4 } else { 2 };
        let mut remaining = log_n - initial_k;
        let mut rates = vec![1usize];
        let mut queries = vec![16usize.min(1 << (remaining + 1))];
        let (mut ks, mut cols) = (Vec::new(), Vec::new());
        while remaining > 3 {
            remaining -= 2;
            ks.push(2);
            cols.push(remaining);
            rates.push(rates.len() + 1);
            queries.push(16usize.min(1 << (remaining + rates.len())));
        }
        let levels = rates.len();
        let prover = ProverConfig {
            log_inv_rates: rates.clone(),
            recursive_steps: ks.len(),
            initial_log_msg_cols: log_n - initial_k,
            initial_log_num_interleaved: initial_k,
            initial_k,
            recursive_log_msg_cols: cols.clone(),
            recursive_ks: ks.clone(),
            queries: queries.clone(),
            grinding_bits: vec![0; levels],
            fold_grinding_bits: vec![0; levels],
            ood_samples: vec![0; levels],
            merkle_hash: HashKind::Blake3,
        };
        let verifier = VerifierConfig {
            log_inv_rates: rates,
            recursive_steps: ks.len(),
            initial_log_msg_cols: log_n - initial_k,
            initial_log_num_interleaved: initial_k,
            initial_k,
            recursive_log_msg_cols: cols,
            recursive_ks: ks,
            queries,
            grinding_bits: vec![0; levels],
            fold_grinding_bits: vec![0; levels],
            ood_samples: vec![0; levels],
            merkle_hash: HashKind::Blake3,
        };
        Pcs::from_configs(shape, prover, verifier).expect("small ladder")
    }

    /// The embedded ladder where there is one, the small one below it.
    fn pcs_for(shape: &Shape) -> Pcs {
        if shape.log_bits() >= MIN_LOG_BITS {
            Pcs::new(shape, HashKind::Blake3).expect("embedded ladder")
        } else {
            small_pcs(shape)
        }
    }

    /// The original opening: the row fold, the suffix tensor, a copy for
    /// Ligerito.
    const REFERENCE: OpeningPath = OpeningPath {
        borrow: false,
        split: false,
        zstats: ZStats::Off,
    };

    /// Every path the knobs select (the statistics forced wherever they
    /// fit, so the small grids take them too).
    fn paths() -> Vec<OpeningPath> {
        vec![
            OpeningPath { borrow: true, ..REFERENCE },
            OpeningPath { split: true, ..REFERENCE },
            OpeningPath { zstats: ZStats::Force, ..REFERENCE },
            OpeningPath { borrow: true, split: true, zstats: ZStats::Force },
            OpeningPath { borrow: true, split: true, zstats: ZStats::Auto },
        ]
    }

    /// Every opening path proves the reference's bytes, which verify: an
    /// inner-product claim (the sumcheck, then the ring switch) and a bare
    /// MLE claim, each with and without a Round-0 claim, on grids below
    /// and at the embedded ladders' floor, with rows too few for the
    /// column statistics (`t = 9`) and enough (`t ≥ 11`).
    #[test]
    fn opening_paths_prove_identically() {
        for (t, s) in [(9usize, 4usize), (11, 3), (12, 6), (14, 8), (16, 6)] {
            let shape = Shape::new(t, s).unwrap();
            let pcs = pcs_for(&shape);
            let mut state = 0x0be7_1a57 ^ (t * 97 + s) as u64;
            let words = shape.rows() / 64;
            let rows: Vec<Vec<u64>> = (0..shape.columns())
                .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
                .collect();
            let (root, hint) = pcs.commit(&shape, rows).unwrap();

            // Σ_{b,c} w1[b]·w2[c]·bit(c, b).
            let w1: Vec<Gf> = (0..shape.rows()).map(|_| random_gf(&mut state)).collect();
            let w2: Vec<Gf> = (0..shape.columns()).map(|_| random_gf(&mut state)).collect();
            let mut target = Gf::zero();
            for (column, weight) in hint.rows().iter().zip(&w2) {
                let mut sum = Gf::zero();
                for (index, &word) in column.iter().enumerate() {
                    let mut bits = word;
                    while bits != 0 {
                        sum += w1[index * 64 + bits.trailing_zeros() as usize];
                        bits &= bits - 1;
                    }
                }
                target += *weight * sum;
            }
            let inner_product = OpeningQuery::InnerProduct {
                claim: LinearClaimGf::from_shape(&shape, w1, w2, target).unwrap(),
            };
            // The witness MLE at a random point, through the ring switch's
            // own claims: `Σ_u eq(point[..7], u)·ŝ_v[u]`.
            let point: Vec<Gf> = (0..shape.log_bits()).map(|_| random_gf(&mut state)).collect();
            let (prefix, suffix) = build_eq_split(&point, LOG_PACKING);
            let value = claim_check(&prefix, &fold_1b_rows_naive(hint.packed_message(), &suffix));
            let mle = OpeningQuery::Mle { point, target: value };
            let ood_point: Vec<Gf> = (0..shape.log_bits() - LOG_PACKING)
                .map(|_| random_gf(&mut state))
                .collect();
            let ood_value = packed_evaluation(&hint, &ood_point);

            for query in [&inner_product, &mle] {
                for ood in [None, Some((ood_point.as_slice(), ood_value))] {
                    let prove = |path: OpeningPath| {
                        let mut transcript = build_prover(b"wfbitz-opening-paths/v1", b"instance");
                        pcs.prove_lin_rows_with(
                            &hint,
                            hint.rows(),
                            hint.packed_cols(),
                            query,
                            StatementBinding::Bind,
                            &mut transcript,
                            ood,
                            path,
                        )
                        .unwrap();
                        transcript.finish()
                    };
                    let reference = prove(REFERENCE);
                    let mut verifier = build_verifier(b"wfbitz-opening-paths/v1", b"instance", &reference);
                    pcs.verify_lin(&root, query, StatementBinding::Bind, &mut verifier, ood)
                        .unwrap();
                    for path in paths() {
                        let proof = prove(path);
                        let same = proof.narg_string == reference.narg_string && proof.hints == reference.hints;
                        assert!(same, "t={t} s={s} ood={} {path:?}", ood.is_some());
                    }
                }
            }
        }
    }
}
