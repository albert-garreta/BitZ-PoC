//! Their `pcs` crate over this crate's flock engine: the checked commit, the
//! statement frames, the inner-product sumcheck, the ring switch and the
//! Ligerito opening, with flock's challenger events framed their way.

use core::mem::size_of;

use bincode::Options;
use flock_core::challenger::Challenger;
use flock_core::field::F128 as FlockF128;
use flock_core::merkle::{Hash, HashKind};
use flock_core::pcs::commit::PcsParams;
use flock_core::pcs::ligerito::{
    LigeritoProfile, LigeritoProof, LigeritoSecurityConfig, ProverConfig, VerifierConfig,
    embedded_security_config, recursive_prover_with_basis, recursive_verifier_with_basis_succinct,
};
use flock_core::pcs::pack::{LOG_PACKING, PACKING_WIDTH as CLAIM_COUNT};
use flock_core::pcs::ring_switch::{
    build_eq_split, claim_check, eval_rs_eq_finish_from_prefix_binary_q, eval_rs_eq_prefix,
    fold_1b_rows_naive, fold_b128_elems, inner_product, tensor_algebra_transpose,
};
use flock_core::zerocheck::univariate_skip::build_eq;
use spongefish::Encoding;

use super::params::{LinearClaimGf, Root, Shape};
use super::sumcheck;
use super::transcript::{ProverState, PublicTranscript, VerifierState};
use crate::ligerito_flock::{FlockCommitHint, commit_rs_ligerito_rows, f128_to_gf, gf_to_f128};
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

const MLE_STATEMENT_LABEL: &[u8] = b"bitz/pcs/mle-opening/v1";
const INNER_PRODUCT_STATEMENT_LABEL: &[u8] = b"bitz/pcs/bit-inner-product/v2";
const SUMCHECK_LABEL: &[u8] = b"bitz/pcs/inner-product-sumcheck/v1";
const MLE_CLAIMS_LABEL: &[u8] = b"bitz/pcs/mle-claims/v1";
const CHALLENGES_LABEL: &[u8] = b"bitz/pcs/ring-switch-challenges/v1";
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
    pub fn new(shape: &Shape, merkle_hash: HashKind) -> Result<Self, ConfigError> {
        let m = shape.log_bits();
        let bit_len = 1usize
            .checked_shl(m as u32)
            .ok_or(ConfigError::Invalid("bit length overflow"))?;
        let profile = LigeritoProfile::Fast;
        let security = security_config(m, profile, merkle_hash)?;
        let params = PcsParams {
            m,
            log_inv_rate: profile.log_inv_rate(),
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
    pub fn prove_lin(
        &self,
        hint: &FlockCommitHint,
        query: &OpeningQuery,
        statement_binding: StatementBinding,
        transcript: &mut ProverState,
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
                self.prove_mle(hint, &ring_switch, *target, transcript)
            }
            OpeningQuery::InnerProduct { claim } => {
                validate_inner_product_claim(self, claim)?;
                if statement_binding == StatementBinding::Bind {
                    bind_inner_product_statement(self, &root, claim, transcript);
                }
                transcript.public_message(SUMCHECK_LABEL);
                let reduced = sumcheck::prove(claim, hint.rows(), transcript)?;
                let ring_switch = RingSwitch::new(&reduced.point, self.params.m)?;
                bind_mle_statement(self, &root, &reduced.point, reduced.target, transcript);
                self.prove_mle(hint, &ring_switch, reduced.target, transcript)
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
    ) -> Result<(), VerifyError> {
        match query {
            OpeningQuery::Mle { point, target } => {
                let ring_switch = RingSwitch::new(point, self.params.m)?;
                if statement_binding == StatementBinding::Bind {
                    bind_mle_statement(self, &commitment.0, point, *target, transcript);
                }
                self.verify_mle(commitment, &ring_switch, *target, transcript)
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
                self.verify_mle(commitment, &ring_switch, reduced.target, transcript)
            }
        }
    }

    /// Their `prove_mle`: the 128 ring-switch claims, seven batching
    /// challenges, the dense packed basis, then Ligerito.
    fn prove_mle(
        &self,
        hint: &FlockCommitHint,
        ring_switch: &RingSwitch,
        target: Gf,
        transcript: &mut ProverState,
    ) -> Result<(), ProveError> {
        let packed = hint.packed_message();
        let (prefix_tensor, suffix_tensor) = build_eq_split(&ring_switch.point, LOG_PACKING);
        if suffix_tensor.len() != packed.len() {
            return Err(ProveError::PackedWitnessLengthMismatch);
        }
        let claims = fold_1b_rows_naive(packed, &suffix_tensor);
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
        let batching_weights = build_eq(&batching_point);
        let packed_target = batch_claims(&claims, &batching_weights);
        let packed_basis = fold_b128_elems(&suffix_tensor, &batching_weights);
        debug_assert_eq!(packed_basis.len(), suffix_tensor.len());

        // ReducedProver::prove
        let data = hint.flock_prover_data();
        let mut challenger = ProverChallenger::new_ligerito(transcript, packed_target);
        let ligerito = recursive_prover_with_basis(
            &self.prover_config,
            packed.to_vec(),
            packed_basis,
            packed_target,
            &data.codeword,
            &data.merkle_tree,
            &mut challenger,
        );
        if challenger.failed() {
            return Err(ProveError::Internal);
        }
        write_opening_proof(&ligerito, transcript)
    }

    /// Their `verify_mle`.
    fn verify_mle(
        &self,
        commitment: &Root,
        ring_switch: &RingSwitch,
        target: Gf,
        transcript: &mut VerifierState<'_>,
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
        let packed_target = batch_claims(&claims, &batching_weights);
        let suffix_point = &ring_switch.point[LOG_PACKING..];
        let evaluate_basis = |ris: &[FlockF128], yr_log_n: usize| -> Vec<FlockF128> {
            if yr_log_n > 32 || ris.len().checked_add(yr_log_n) != Some(suffix_point.len()) {
                return Vec::new();
            }
            let Some(yr_len) = 1usize.checked_shl(yr_log_n as u32) else {
                return Vec::new();
            };
            let prefix = eval_rs_eq_prefix(suffix_point, ris);
            let suffix = &suffix_point[ris.len()..];
            (0..yr_len)
                .map(|y| {
                    eval_rs_eq_finish_from_prefix_binary_q(&prefix, suffix, y as u32, &batching_weights)
                })
                .collect()
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
        let nonce = find_pow(&seed, bits);
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

fn find_pow(seed: &[u8; 16], bits: u32) -> u64 {
    if bits == 0 {
        return 0;
    }
    let mut nonce = 0u64;
    loop {
        if pow_valid(seed, nonce, bits) {
            return nonce;
        }
        nonce = nonce.checked_add(1).expect("proof-of-work nonce exhausted");
    }
}

fn pow_valid(seed: &[u8; 16], nonce: u64, bits: u32) -> bool {
    if bits == 0 {
        return nonce == 0;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"bitz-pcs-pow-v1");
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
