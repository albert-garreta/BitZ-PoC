//! Combined Spartan and virtual-F2Z proof for independent SHA-256 compressions.
//!
//! The repeated constraint matrices factor as
//! `I_instances tensor (A_0, B_0, C_0)`. After Spartan's outer sumcheck, the
//! matrix-bound assignment functional therefore factors into an instance
//! equality table and one local column table. F2Z opens that complete linear
//! functional directly, avoiding Spartan's dense global inner-sumcheck table.

use blake3::Hasher;
use crypto_primitives::PrimeField;
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};
use thiserror::Error;

use crate::{
    f2map::{cell_count, RepeatedVirtualMap, VirtualMap},
    ligerito::{packed_vars, LOG_PACKING},
    ligerito_flock::{
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual,
        prove_mle_eval_mod_q_ligerito_virtual_runtime, sha_paper128_lig_configs,
        validate_ligerito_commitment, verify_mle_eval_mod_q_ligerito_virtual,
        verify_mle_eval_mod_q_ligerito_virtual_runtime, FlockCommitHint, FlockRsError,
        IntEvalRsLigVirtProof, LigeritoStatementConfig,
    },
    pcs::{Fq, IntEvalParams, ProjectCanonicalU128, FQ_BITS, FQ_MOD},
    transcript::traits::Transcript,
};

use super::super::{
    absorb_spartan_message,
    f2z::{f2z_generator, hash_code, profile_code, spartan_f2z_field_config, SpartanF2zField},
    grinding::{grind_and_absorb, verify_and_absorb, GrindingDomain, GrindingError, GrindingRound},
    make_equality_factors,
    matrix::eq_table,
    squeeze_field,
    sumcheck::{
        prove_outer_sumcheck_with_reducer, prove_outer_sumcheck_with_reducer_grinded,
        OptimizedSumcheckReducer, OuterSumcheckProof, R1csProductMles,
    },
    PreparedConstraintMatrices, SpartanError, SpartanF2zProof, SpartanField, Virtualized,
};

use super::{
    constraints::{
        sha256_public_f_column, sha256_public_h_column, PreparedSha256CompressionBatch,
        Sha256ConstraintError, SHA256_CONSTRAINT_LOCAL_VARS, SHA256_F_LOCAL_VARS,
        SHA256_H_LOCAL_VARS, SHA256_PUBLIC_WORDS, SHA256_PUBLIC_WORD_BITS,
    },
    prime::{sample_sha256_mod_q_context, Sha256PrimeError, Sha256PrimeProfile},
    witness::{ExactSha256CompressionWitnessBatch, Sha256CompressionStatement, Sha256WitnessError},
};

const SHA256_SPARTAN_DOMAIN: &[u8] = b"f2z/spartan-sha256-compressions/v3";
const SHA256_ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-sha256-assignment/v3";
const SHA256_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-sha256-opening/v3";
const SHA256_CONSTANT_ONE_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/constant-one-batch/v1";
const SHA256_PUBLIC_STATEMENT_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-statement/v1";
const SHA256_PUBLIC_IO_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-io-batch/v1";
const SHA256_CONSTANT_ASSIGNMENT_COLUMN: usize = 0;
const SHA256_PAPER_SPARTAN_DOMAIN: &[u8] = b"f2z/spartan-sha256-compressions/paper128/v1";
const SHA256_PAPER_ASSIGNMENT_DOMAIN: &[u8] = b"f2z/spartan-sha256-assignment/paper128/v1";

enum Sha256InitialGrinding {}
enum Sha256OuterGrinding {}
enum Sha256TerminalGrinding {}

impl GrindingDomain for Sha256InitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/initial/v1";
}

impl GrindingDomain for Sha256OuterGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/outer/v1";
}

impl GrindingDomain for Sha256TerminalGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/terminal/v1";
}

/// Grinds one boundary, or skips it entirely at difficulty 0 (the λ = 100
/// profiles): no transcript bytes move and the stored nonce is 0.
fn grind_boundary<D: GrindingDomain, T: Transcript>(
    transcript: &mut T,
    bits: u32,
) -> Result<u64, GrindingError> {
    if bits == 0 {
        return Ok(0);
    }
    grind_and_absorb::<D, _>(transcript, GrindingRound::new(0), bits)
}

/// Checks one boundary, or (at difficulty 0) requires the canonical zero
/// nonce without touching the transcript.
fn check_boundary<D: GrindingDomain, T: Transcript>(
    transcript: &mut T,
    bits: u32,
    nonce: u64,
) -> Result<(), GrindingError> {
    if bits == 0 {
        if nonce != 0 {
            return Err(GrindingError::InvalidNonce { nonce, bits });
        }
        return Ok(());
    }
    verify_and_absorb::<D, _>(transcript, GrindingRound::new(0), bits, nonce)
}

/// Outer Spartan reduction paired with a virtual opening of the complete
/// factorized assignment functional.
pub type Sha256CompressionProof = SpartanF2zProof<OuterSumcheckProof<SpartanF2zField>, Virtualized>;

/// Runtime-prime portion of the paper profile, with explicit Spartan
/// Fiat--Shamir grinding nonces. The prime itself is intentionally absent:
/// the verifier re-derives it after binding the commitment.
///
/// This type's `128` name refers to the configured prime-selection, Spartan,
/// commitment-field, and Ligerito targets. It does not by itself certify the
/// still-separate soundness accounting for the existing virtual-F2Z bridge.
#[derive(Clone)]
pub struct Sha256Paper128Proof {
    initial_nonce: u64,
    outer: OuterSumcheckProof<SpartanF2zField>,
    outer_nonces: Vec<u64>,
    terminal_nonce: u64,
    f2z: IntEvalRsLigVirtProof,
}

impl Sha256Paper128Proof {
    /// Initial commit-before-prime grinding nonce.
    pub const fn initial_nonce(&self) -> u64 {
        self.initial_nonce
    }

    /// Cubic Spartan sumcheck proof.
    pub const fn outer(&self) -> &OuterSumcheckProof<SpartanF2zField> {
        &self.outer
    }

    /// Nonces adjacent to the outer sumcheck round messages.
    pub fn outer_nonces(&self) -> &[u64] {
        &self.outer_nonces
    }

    /// Nonce after the terminal `Az/Bz/Cz` evaluations.
    pub const fn terminal_nonce(&self) -> u64 {
        self.terminal_nonce
    }

    /// Virtual F2Z opening proof.
    /// Mutable access to the virtual F2Z opening — soundness-test hook
    /// only (tampering with a proof component must be rejected).
    #[doc(hidden)]
    pub fn f2z_mut_for_tests(&mut self) -> &mut IntEvalRsLigVirtProof {
        &mut self.f2z
    }

    pub const fn f2z(&self) -> &IntEvalRsLigVirtProof {
        &self.f2z
    }
}

/// Failures in the SHA-256 Spartan/F2Z adapter.
#[derive(Debug, Error)]
pub enum Sha256F2zError {
    /// The local relation, repeated map, witness, and F2Z grids disagree.
    #[error("SHA-256 relation or witness geometry is inconsistent")]
    InvalidGeometry,

    /// A canonical transcript length did not fit its fixed-width encoding.
    #[error("SHA-256 transcript metadata is too large")]
    BindingEncodingOverflow,

    /// The prepared relation does not use the fixed F2Z prime.
    #[error("SHA-256 relation uses an unsupported Spartan field")]
    UnsupportedFieldModulus,

    /// The committed source assignment does not contain the required leading
    /// one in every compression instance.
    #[error("SHA-256 source assignment has a malformed constant-one column")]
    InvalidConstantOneColumn,

    /// The public input/output batch must contain one statement per committed
    /// compression instance.
    #[error("SHA-256 public statement length mismatch: expected {expected}, got {actual}")]
    InvalidPublicStatementLength { expected: usize, actual: usize },

    /// Ligerito configuration derivation failed.
    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    /// Runtime-prime profile or sampling failed.
    #[error(transparent)]
    Prime(#[from] Sha256PrimeError),

    /// Exact relation projection failed.
    #[error(transparent)]
    Constraint(#[from] Sha256ConstraintError),

    /// Exact witness-product projection failed.
    #[error(transparent)]
    Witness(#[from] Sha256WitnessError),

    /// A Fiat--Shamir grinding nonce could not be produced or checked.
    #[error(transparent)]
    Grinding(#[from] GrindingError),

    /// Spartan's outer reduction rejected.
    #[error(transparent)]
    Spartan(#[from] SpartanError),

    /// The F2Z opening rejected.
    #[error("the SHA-256 virtual opening rejected: {0:?}")]
    F2z(FlockRsError),
}

/// Derives the paper-128 BLAKE3/UDR Ligerito configuration for a supported
/// SHA batch (`2^7..=2^16` compressions).
pub fn sha256_compression_configs(
    p_f: &IntEvalParams,
) -> Result<(LigProverConfig, LigVerifierConfig), Sha256F2zError> {
    validate_source_params(p_f)?;
    sha_paper128_lig_configs(packed_vars(p_f)).map_err(Sha256F2zError::LigeritoConfig)
}

/// Derives the BLAKE3/UDR Ligerito configuration at the prepared batch's
/// OWN security target (`prepared.security().ligerito_target_bits`) — the
/// one-λ-for-the-whole-system wiring. Identical to
/// [`sha256_compression_configs`] at the default 128-bit profiles.
pub fn sha256_compression_configs_for(
    prepared: &PreparedSha256CompressionBatch,
) -> Result<(LigProverConfig, LigVerifierConfig), Sha256F2zError> {
    let p_f = prepared.source_params();
    validate_source_params(p_f)?;
    crate::ligerito_flock::sha_paper128_lig_configs_bits(
        packed_vars(p_f),
        prepared.security().ligerito_target_bits,
    )
    .map_err(Sha256F2zError::LigeritoConfig)
}

/// Commits packed extended-source rows `[1 | f]` under an explicit config.
pub fn commit_sha256_compression_witness_with_config(
    p_f: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_source_params(p_f)?;
    validate_rows(p_f, &rows)?;
    validate_constant_one_column(p_f, &rows)?;
    let hint = commit_rs_ligerito_rows(p_f, rows, pc);
    validate_ligerito_commitment(&hint.commitment, pc).map_err(Sha256F2zError::F2z)?;
    Ok(hint)
}

/// Commits packed extended-source rows `[1 | f]` with the derived production
/// configuration.
pub fn commit_sha256_compression_witness(
    p_f: &IntEvalParams,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(p_f)?;
    commit_sha256_compression_witness_with_config(p_f, rows, &pc)
}

/// Commits the q-independent source rows retained by an exact SHA witness.
///
/// This convenience API clones the packed source rows because the exact
/// witness must remain available for post-commitment product projection.
pub fn commit_sha256_paper128_witness_with_config(
    prepared: &PreparedSha256CompressionBatch,
    witness: &ExactSha256CompressionWitnessBatch,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_rows(prepared.source_params(), witness.source_rows())?;
    validate_constant_one_column(prepared.source_params(), witness.source_rows())?;
    commit_sha256_compression_witness_with_config(
        prepared.source_params(),
        witness.source_rows().to_vec(),
        pc,
    )
}

/// Commits an exact witness with the paper's 128-bit Ligerito profile.
pub fn commit_sha256_paper128_witness(
    prepared: &PreparedSha256CompressionBatch,
    witness: &ExactSha256CompressionWitnessBatch,
) -> Result<FlockCommitHint, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs_for(prepared)?;
    commit_sha256_paper128_witness_with_config(prepared, witness, &pc)
}

/// Proves the runtime-prime SHA profile: commit first, grind, derive a bounded
/// 112/113-bit prime, project exact integers, and run Spartan/F2Z under that
/// one runtime field configuration.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_paper128_with_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &ExactSha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<Sha256Paper128Proof, Sha256F2zError> {
    let p_f = prepared.source_params();
    let p_h = prepared.assignment_params();
    let map = prepared.map();
    let profile = Sha256PrimeProfile::from_security(prepared.security(), p_h.t)?;

    validate_public_statement(p_h, public_statement)?;
    validate_common_geometry(None, map, p_h, p_f)?;
    validate_rows(p_f, witness.source_rows())?;
    validate_constant_one_column(p_f, witness.source_rows())?;
    validate_rows(p_h, witness.assignment_rows())?;
    if witness.outputs().len() != p_h.rows() || hint_f.rows() != witness.source_rows() {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    validate_ligerito_commitment(&hint_f.commitment, pc).map_err(Sha256F2zError::F2z)?;
    if hint_f.commitment.params.m != p_f.t + p_f.s {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = crate::utils::prof::scope("sha256-paper128:statement_bind_prover");
        let public_statement_binding = public_statement_binding(public_statement)?;
        let assignment_binding =
            paper_assignment_binding(prepared, &hint_f.commitment, pc, &public_statement_binding)?;
        absorb_sha256_paper_statement(
            transcript,
            prepared,
            &public_statement_binding,
            &assignment_binding,
        );
        assignment_binding
    };

    // Paper §2.1 Step 2: pre-draw grinding, prime sample, projection mod q.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce = {
        let _scope = crate::utils::prof::scope("sha256-paper128:initial_grinding_prove");
        grind_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
        )?
    };
    let mod_q = {
        let _scope = crate::utils::prof::scope("sha256-paper128:runtime_prime_sample_prover");
        sample_sha256_mod_q_context(transcript, profile)?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("sha256-paper128:relation_projection_prover");
        prepared.project(mod_q.field_config())?
    };
    let products = {
        let _scope = crate::utils::prof::scope("sha256-paper128:product_projection_prover");
        witness.project_products(mod_q.field_config())?
    };
    validate_common_geometry(Some(&matrices), map, p_h, p_f)?;
    validate_products(&products, p_h)?;
    absorb_projected_sha256_relation(transcript, &matrices);
    drop(step2_scope);

    // Step 3: the Spartan outer PIOP over F_q (round grinding included).
    let step3_scope = crate::utils::prof::scope("step3:piop_prove");
    let field_config = mod_q.field_config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let (equality_factors, reducer) = {
        let _scope = crate::utils::prof::scope("sha256-paper128:spartan_prepare_prover");
        let equality_factors =
            make_equality_factors(&tau, field_config).map_err(SpartanError::from)?;
        let reducer = OptimizedSumcheckReducer::new(field_config).map_err(SpartanError::from)?;
        (equality_factors, reducer)
    };
    let (outer, outer_nonces) = {
        let _scope = crate::utils::prof::scope("sha256-paper128:spartan_outer_prove");
        prove_outer_sumcheck_with_reducer_grinded::<Sha256OuterGrinding, _, _>(
            transcript,
            SpartanF2zField::zero_with_cfg(field_config),
            &tau,
            equality_factors,
            products,
            field_config,
            &reducer,
            profile.outer_round_grinding_bits() as u32,
        )
        .map_err(SpartanError::from)?
    };
    drop(step3_scope);

    // Step 4: bitification — terminal grinding + claim factorization.
    let step4_scope = crate::utils::prof::scope("step4:bitify_prove");
    let terminal_nonce = {
        let _scope = crate::utils::prof::scope("sha256-paper128:terminal_grinding_prove");
        grind_boundary::<Sha256TerminalGrinding, _>(
            transcript,
            profile.terminal_grinding_bits() as u32,
        )?
    };

    let rho = squeeze_field(transcript, field_config);
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let (row_weights_q, col_weights_q, claimed_q) = {
        let _scope = crate::utils::prof::scope("sha256-paper128:opening_prepare_prover");
        factorized_opening_claim(
            &matrices,
            p_h,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            public_statement,
            &public_slot_weights,
            &public_io_batch,
            batched_product_claim(
                &outer.proof.az_mle_claim,
                &outer.proof.bz_mle_claim,
                &outer.proof.cz_mle_claim,
                &rho,
            ),
        )?
    };
    {
        let _scope = crate::utils::prof::scope("sha256-paper128:opening_claim_absorb_prover");
        absorb_opening_claim(
            transcript,
            &assignment_binding,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            &public_slot_weights,
            &public_io_batch,
            &row_weights_q,
            &col_weights_q,
            claimed_q,
        )?;
    }
    drop(step4_scope);

    // Steps 5.1–5.3: the virtual F2Z opening.
    let f2z = {
        let _step5 = crate::utils::prof::scope("step5:open_prove");
        let _scope = crate::utils::prof::scope("sha256-paper128:f2z_prove");
        prove_mle_eval_mod_q_ligerito_virtual_runtime(
            transcript,
            hint_f,
            witness.assignment_rows(),
            p_h,
            p_f,
            map,
            &row_weights_q,
            mod_q.q(),
            mod_q.q_bits(),
            f2z_generator(),
            prepared.security().forest_round_grinding_bits,
            pc,
        )
        .map_err(Sha256F2zError::F2z)?
    };

    Ok(Sha256Paper128Proof {
        initial_nonce,
        outer: outer.proof,
        outer_nonces,
        terminal_nonce,
        f2z,
    })
}

/// Proves with the batch-derived paper Ligerito configuration.
pub fn prove_sha256_compressions_paper128<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &ExactSha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
) -> Result<Sha256Paper128Proof, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs_for(prepared)?;
    prove_sha256_compressions_paper128_with_config(
        transcript,
        prepared,
        public_statement,
        witness,
        hint_f,
        &pc,
    )
}

/// Verifies the runtime-prime SHA profile while independently re-deriving `q`
/// from the commitment-bound transcript.
#[allow(clippy::too_many_arguments)]
pub fn verify_sha256_compressions_paper128_with_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256Paper128Proof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    let p_f = prepared.source_params();
    let p_h = prepared.assignment_params();
    let map = prepared.map();
    let profile = Sha256PrimeProfile::from_security(prepared.security(), p_h.t)?;

    validate_public_statement(p_h, public_statement)?;
    validate_common_geometry(None, map, p_h, p_f)?;
    validate_ligerito_commitment(commitment_f, vc).map_err(Sha256F2zError::F2z)?;
    if commitment_f.params.m != p_f.t + p_f.s
        || proof.outer.sumcheck.round_polynomials.len() != SHA256_CONSTRAINT_LOCAL_VARS + p_h.t
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = crate::utils::prof::scope("sha256-paper128:statement_bind_verifier");
        let public_statement_binding = public_statement_binding(public_statement)?;
        let assignment_binding =
            paper_assignment_binding(prepared, commitment_f, vc, &public_statement_binding)?;
        absorb_sha256_paper_statement(
            transcript,
            prepared,
            &public_statement_binding,
            &assignment_binding,
        );
        assignment_binding
    };
    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    {
        let _scope = crate::utils::prof::scope("sha256-paper128:initial_grinding_verify");
        check_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
            proof.initial_nonce,
        )?;
    }
    let mod_q = {
        let _scope = crate::utils::prof::scope("sha256-paper128:runtime_prime_sample_verifier");
        sample_sha256_mod_q_context(transcript, profile)?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("sha256-paper128:relation_projection_verifier");
        prepared.project(mod_q.field_config())?
    };
    validate_common_geometry(Some(&matrices), map, p_h, p_f)?;
    absorb_projected_sha256_relation(transcript, &matrices);
    drop(step2_scope);

    let step3_scope = crate::utils::prof::scope("step3:piop_verify");
    let field_config = mod_q.field_config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let outer = {
        let _scope = crate::utils::prof::scope("sha256-paper128:spartan_outer_verify");
        proof
            .outer
            .verify_grinded::<Sha256OuterGrinding>(
                transcript,
                SpartanF2zField::zero_with_cfg(field_config),
                &tau,
                field_config,
                &proof.outer_nonces,
                profile.outer_round_grinding_bits() as u32,
            )
            .map_err(SpartanError::from)?
    };
    drop(step3_scope);

    let step4_scope = crate::utils::prof::scope("step4:bitify_verify");
    {
        let _scope = crate::utils::prof::scope("sha256-paper128:terminal_grinding_verify");
        check_boundary::<Sha256TerminalGrinding, _>(
            transcript,
            profile.terminal_grinding_bits() as u32,
            proof.terminal_nonce,
        )?;
    }

    let rho = squeeze_field(transcript, field_config);
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let (row_weights_q, col_weights_q, claimed_q) = {
        let _scope = crate::utils::prof::scope("sha256-paper128:opening_prepare_verifier");
        factorized_opening_claim(
            &matrices,
            p_h,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            public_statement,
            &public_slot_weights,
            &public_io_batch,
            batched_product_claim(
                &outer.az_mle_claim,
                &outer.bz_mle_claim,
                &outer.cz_mle_claim,
                &rho,
            ),
        )?
    };
    {
        let _scope = crate::utils::prof::scope("sha256-paper128:opening_claim_absorb_verifier");
        absorb_opening_claim(
            transcript,
            &assignment_binding,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            &public_slot_weights,
            &public_io_batch,
            &row_weights_q,
            &col_weights_q,
            claimed_q,
        )?;
    }
    drop(step4_scope);

    let _step5 = crate::utils::prof::scope("step5:open_verify");
    let _scope = crate::utils::prof::scope("sha256-paper128:f2z_verify");
    verify_mle_eval_mod_q_ligerito_virtual_runtime(
        transcript,
        commitment_f,
        &proof.f2z,
        p_h,
        p_f,
        map,
        &row_weights_q,
        &col_weights_q,
        f2z_generator(),
        claimed_q,
        mod_q.q(),
        mod_q.q_bits(),
        prepared.security().forest_round_grinding_bits,
        vc,
    )
    .map_err(Sha256F2zError::F2z)
}

/// Verifies with the batch-derived paper Ligerito configuration.
pub fn verify_sha256_compressions_paper128<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256Paper128Proof,
) -> Result<(), Sha256F2zError> {
    let (_, vc) = sha256_compression_configs(prepared.source_params())?;
    verify_sha256_compressions_paper128_with_config(
        transcript,
        prepared,
        public_statement,
        commitment_f,
        proof,
        &vc,
    )
}

/// Proves the repeated SHA-256 R1CS and opens its factorized assignment claim
/// against the commitment to `[1 | f]`.
///
/// `public_statement` is ordered by batch instance and must contain exactly
/// `2^p_h.t` state/block/claimed-output triples.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_spartan_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    h_rows: &[Vec<u64>],
    products: R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    validate_prover_inputs(
        matrices,
        map,
        p_h,
        p_f,
        public_statement,
        h_rows,
        &products,
        hint_f,
        pc,
    )?;
    let public_statement_binding = public_statement_binding(public_statement)?;
    let assignment_binding = assignment_binding(matrices, map, p_h, p_f, &hint_f.commitment)?;
    absorb_sha256_statement(
        transcript,
        matrices,
        p_h,
        &public_statement_binding,
        &assignment_binding,
    );

    let field_config = matrices.config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let equality_factors = make_equality_factors(&tau, field_config).map_err(SpartanError::from)?;
    let reducer = OptimizedSumcheckReducer::new(field_config).map_err(SpartanError::from)?;
    let outer = {
        let _scope = crate::utils::prof::scope("sha256-f2z:spartan_outer_prove");
        prove_outer_sumcheck_with_reducer(
            transcript,
            SpartanF2zField::zero_with_cfg(field_config),
            &tau,
            equality_factors,
            products,
            field_config,
            &reducer,
        )
        .map_err(SpartanError::from)?
    };

    let rho = squeeze_field(transcript, field_config);
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let (row_weights_q, col_weights, claimed) = {
        let _scope = crate::utils::prof::scope("sha256-f2z:opening_prepare_prover");
        factorized_opening_claim(
            matrices,
            p_h,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            public_statement,
            &public_slot_weights,
            &public_io_batch,
            batched_product_claim(
                &outer.proof.az_mle_claim,
                &outer.proof.bz_mle_claim,
                &outer.proof.cz_mle_claim,
                &rho,
            ),
        )?
    };
    absorb_opening_claim(
        transcript,
        &assignment_binding,
        &outer.eval_points,
        &rho,
        &constant_one_batch,
        &public_slot_weights,
        &public_io_batch,
        &row_weights_q,
        &col_weights,
        claimed,
    )?;

    let f2z = {
        let _scope = crate::utils::prof::scope("sha256-f2z:f2z_prove");
        prove_mle_eval_mod_q_ligerito_virtual(
            transcript,
            hint_f,
            h_rows,
            p_h,
            p_f,
            map,
            &row_weights_q,
            FQ_BITS,
            f2z_generator(),
            pc,
        )
    };
    Ok(SpartanF2zProof::new(outer.proof, f2z))
}

/// Proves using the production configuration derived from `p_f`.
///
/// `public_statement` is ordered by batch instance and must contain exactly
/// `2^p_h.t` entries.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    h_rows: &[Vec<u64>],
    products: R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(p_f)?;
    prove_sha256_compressions_spartan_and_f2z_with_config(
        transcript,
        matrices,
        map,
        p_h,
        p_f,
        public_statement,
        h_rows,
        products,
        hint_f,
        &pc,
    )
}

/// Verifies the repeated SHA-256 proof under an explicit Ligerito config.
///
/// `public_statement` is ordered by batch instance and must contain exactly
/// `2^p_h.t` state/block/claimed-output triples.
#[allow(clippy::too_many_arguments)]
pub fn verify_sha256_compressions_spartan_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    validate_verifier_inputs(
        matrices,
        map,
        p_h,
        p_f,
        public_statement,
        commitment_f,
        proof,
        vc,
    )?;
    let public_statement_binding = public_statement_binding(public_statement)?;
    let assignment_binding = assignment_binding(matrices, map, p_h, p_f, commitment_f)?;
    absorb_sha256_statement(
        transcript,
        matrices,
        p_h,
        &public_statement_binding,
        &assignment_binding,
    );

    let field_config = matrices.config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let outer = {
        let _scope = crate::utils::prof::scope("sha256-f2z:spartan_outer_verify");
        proof
            .spartan()
            .verify(
                transcript,
                SpartanF2zField::zero_with_cfg(field_config),
                &tau,
                field_config,
            )
            .map_err(SpartanError::from)?
    };
    let rho = squeeze_field(transcript, field_config);
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let claimed = batched_product_claim(
        &outer.az_mle_claim,
        &outer.bz_mle_claim,
        &outer.cz_mle_claim,
        &rho,
    );
    let (row_weights_q, col_weights, claimed) = {
        let _scope = crate::utils::prof::scope("sha256-f2z:opening_prepare_verifier");
        factorized_opening_claim(
            matrices,
            p_h,
            &outer.eval_points,
            &rho,
            &constant_one_batch,
            public_statement,
            &public_slot_weights,
            &public_io_batch,
            claimed,
        )?
    };
    absorb_opening_claim(
        transcript,
        &assignment_binding,
        &outer.eval_points,
        &rho,
        &constant_one_batch,
        &public_slot_weights,
        &public_io_batch,
        &row_weights_q,
        &col_weights,
        claimed,
    )?;

    let col_weights_fq = col_weights.iter().copied().map(Fq).collect::<Vec<_>>();
    let result = {
        let _scope = crate::utils::prof::scope("sha256-f2z:f2z_verify");
        verify_mle_eval_mod_q_ligerito_virtual(
            transcript,
            commitment_f,
            proof.f2z(),
            p_h,
            p_f,
            map,
            &row_weights_q,
            &col_weights_fq,
            f2z_generator(),
            Fq(claimed),
            FQ_BITS,
            vc,
        )
    };
    result.map_err(Sha256F2zError::F2z)
}

/// Verifies using the production configuration derived from `p_f`.
///
/// `public_statement` is ordered by batch instance and must contain exactly
/// `2^p_h.t` entries.
#[allow(clippy::too_many_arguments)]
pub fn verify_sha256_compressions_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
) -> Result<(), Sha256F2zError> {
    let (_, vc) = sha256_compression_configs(p_f)?;
    verify_sha256_compressions_spartan_and_f2z_with_config(
        transcript,
        matrices,
        map,
        p_h,
        p_f,
        public_statement,
        commitment_f,
        proof,
        &vc,
    )
}

fn factorized_opening_claim(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    p_h: &IntEvalParams,
    row_point: &[SpartanF2zField],
    rho: &SpartanF2zField,
    constant_one_batch: &SpartanF2zField,
    public_statement: &[Sha256CompressionStatement],
    public_slot_weights: &[SpartanF2zField],
    public_io_batch: &SpartanF2zField,
    claimed: SpartanF2zField,
) -> Result<(Vec<u128>, Vec<u128>, u128), Sha256F2zError> {
    let expected = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    if row_point.len() != expected
        || public_slot_weights.len() != SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    validate_public_statement(p_h, public_statement)?;

    let (local_point, instance_point) = row_point.split_at(SHA256_CONSTRAINT_LOCAL_VARS);
    let instance_weights =
        eq_table(instance_point, matrices.config()).map_err(SpartanError::from)?;
    let row_weights_q = instance_weights
        .iter()
        .map(|value| value.canonical_u128())
        .collect::<Vec<_>>();
    let mut local_columns = matrices
        .bind_and_batch(local_point, rho)
        .map_err(SpartanError::from)?;
    // Batch the verifier-owned claim
    //   <eq(instance_point) ⊗ e_0, h> = 1
    // into the matrix-bound assignment claim. Since the equality weights sum
    // to one and the public virtual map fixes h[0] = f[0], this forces the
    // committed constant column to be one at every instance, up to the MLE
    // identity-test soundness error.
    let constant_column = local_columns
        .evaluations
        .get_mut(SHA256_CONSTANT_ASSIGNMENT_COLUMN)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    *constant_column += constant_one_batch;
    let mut claimed = claimed;
    claimed += constant_one_batch;

    // Batch all public SHA word equalities into the same factorized opening:
    //
    //   sum_i eq(instance_point, i) sum_{w,b} lambda[w,b]
    //       (h[i, public_bit(w,b)] - bit(statement[i][w], b)) = 0.
    //
    // All 1024 slot coefficients are independent transcript challenges.  This
    // keeps the terminal identity degree two in the sampled challenges; a
    // tensor point over words followed by powers of two would couple slots and
    // require a larger grinding allowance.
    for word_slot in 0..SHA256_PUBLIC_WORDS {
        for bit in 0..SHA256_PUBLIC_WORD_BITS {
            let slot = word_slot * SHA256_PUBLIC_WORD_BITS + bit;
            let bit_coefficient = public_io_batch.clone() * &public_slot_weights[slot];
            let h_column = sha256_public_h_column(word_slot, bit);
            let column = local_columns
                .evaluations
                .get_mut(h_column)
                .ok_or(Sha256F2zError::InvalidGeometry)?;
            *column += &bit_coefficient;
        }
    }

    let zero = SpartanF2zField::zero_with_cfg(matrices.config());
    let mut public_value = zero.clone();
    for (instance_weight, statement) in instance_weights.iter().zip(public_statement) {
        let mut instance_value = zero.clone();
        for (word_slot, word) in statement.words().enumerate() {
            for bit in 0..SHA256_PUBLIC_WORD_BITS {
                if word >> bit & 1 == 1 {
                    let slot = word_slot * SHA256_PUBLIC_WORD_BITS + bit;
                    instance_value += &public_slot_weights[slot];
                }
            }
        }
        public_value += &(instance_weight.clone() * &instance_value);
    }
    claimed += &(public_io_batch.clone() * &public_value);

    let col_weights = local_columns
        .evaluations
        .into_iter()
        .map(|value| value.canonical_u128())
        .collect::<Vec<_>>();
    Ok((row_weights_q, col_weights, claimed.canonical_u128()))
}

fn batched_product_claim(
    az: &SpartanF2zField,
    bz: &SpartanF2zField,
    cz: &SpartanF2zField,
    rho: &SpartanF2zField,
) -> SpartanF2zField {
    let rho_squared = rho.clone() * rho;
    let mut claim = az.clone();
    claim += &(rho.clone() * bz);
    claim += &(rho_squared * cz);
    claim
}

fn validate_source_params(p_f: &IntEvalParams) -> Result<(), Sha256F2zError> {
    let host_bits = usize::BITS as usize;
    if p_f.word_bits != 1
        || p_f.s != SHA256_F_LOCAL_VARS
        || p_f.t < LOG_PACKING
        || p_f.t.saturating_add(p_f.s) > 126
        || p_f.t >= host_bits
        || p_f.t.saturating_add(SHA256_H_LOCAL_VARS) >= host_bits
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_common(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
) -> Result<(), Sha256F2zError> {
    validate_common_geometry(Some(matrices), map, p_h, p_f)?;
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    if matrices.field_modulus_encoding() != expected {
        return Err(Sha256F2zError::UnsupportedFieldModulus);
    }
    Ok(())
}

fn validate_common_geometry(
    matrices: Option<&PreparedConstraintMatrices<SpartanF2zField>>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
) -> Result<(), Sha256F2zError> {
    validate_source_params(p_f)?;
    if p_h.word_bits != 1
        || p_h.t != p_f.t
        || p_h.s != SHA256_H_LOCAL_VARS
        || map.instances() != p_h.rows()
        || map.rows() != cell_count(p_h)
        || map.cols() != cell_count(p_f)
        || !map_fixes_constant_assignment(map)
        || !map_fixes_public_statement(map)
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    if matrices.is_some_and(|matrices| {
        matrices.num_row_vars() != SHA256_CONSTRAINT_LOCAL_VARS
            || matrices.num_column_vars() != SHA256_H_LOCAL_VARS
    }) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn map_fixes_constant_assignment(map: &RepeatedVirtualMap) -> bool {
    let mut constant_source = None;
    for (column, entries) in map.local().matrix().columns().enumerate() {
        for row in entries.row_indices() {
            if *row == SHA256_CONSTANT_ASSIGNMENT_COLUMN
                && constant_source.replace(column).is_some()
            {
                return false;
            }
        }
    }
    constant_source == Some(SHA256_CONSTANT_ASSIGNMENT_COLUMN)
}

fn map_fixes_public_statement(map: &RepeatedVirtualMap) -> bool {
    const PUBLIC_BITS: usize = SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS;

    let mut sources = [None; PUBLIC_BITS];
    for (f_column, entries) in map.local().matrix().columns().enumerate() {
        for &h_column in entries.row_indices() {
            let Some(index) = public_bit_index_for_h_column(h_column) else {
                continue;
            };
            if sources[index].replace(f_column).is_some() {
                return false;
            }
        }
    }

    sources.into_iter().enumerate().all(|(index, source)| {
        let word_slot = index / SHA256_PUBLIC_WORD_BITS;
        let bit = index % SHA256_PUBLIC_WORD_BITS;
        source == Some(sha256_public_f_column(word_slot, bit))
    })
}

fn public_bit_index_for_h_column(h_column: usize) -> Option<usize> {
    const INPUT_WORDS: usize = 24;
    const OUTPUT_WORD_STRIDE: usize = 33;

    let input_start = sha256_public_h_column(0, 0);
    let input_end = sha256_public_h_column(INPUT_WORDS - 1, SHA256_PUBLIC_WORD_BITS - 1) + 1;
    if (input_start..input_end).contains(&h_column) {
        return Some(h_column - input_start);
    }

    let output_start = sha256_public_h_column(INPUT_WORDS, 0);
    let offset = h_column.checked_sub(output_start)?;
    let output_word = offset / OUTPUT_WORD_STRIDE;
    let bit = offset % OUTPUT_WORD_STRIDE;
    if output_word >= SHA256_PUBLIC_WORDS - INPUT_WORDS || bit >= SHA256_PUBLIC_WORD_BITS {
        return None;
    }
    Some((INPUT_WORDS + output_word) * SHA256_PUBLIC_WORD_BITS + bit)
}

fn validate_rows(p: &IntEvalParams, rows: &[Vec<u64>]) -> Result<(), Sha256F2zError> {
    let words = p.rows().div_ceil(64);
    if rows.len() != p.cols() || rows.iter().any(|row| row.len() != words) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_constant_one_column(
    p: &IntEvalParams,
    rows: &[Vec<u64>],
) -> Result<(), Sha256F2zError> {
    let constant = rows
        .get(SHA256_CONSTANT_ASSIGNMENT_COLUMN)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    let instances = p.rows();
    for (word_index, &word) in constant.iter().enumerate() {
        let remaining = instances.saturating_sub(word_index * u64::BITS as usize);
        let active = remaining.min(u64::BITS as usize);
        let expected = if active == u64::BITS as usize {
            u64::MAX
        } else {
            (1_u64 << active).wrapping_sub(1)
        };
        if word != expected {
            return Err(Sha256F2zError::InvalidConstantOneColumn);
        }
    }
    Ok(())
}

fn validate_products(
    products: &R1csProductMles<SpartanF2zField>,
    p_h: &IntEvalParams,
) -> Result<(), Sha256F2zError> {
    let num_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    let expected = 1usize
        .checked_shl(u32::try_from(num_vars).map_err(|_| Sha256F2zError::InvalidGeometry)?)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    if products.az.num_vars != num_vars
        || products.bz.num_vars != num_vars
        || products.cz.num_vars != num_vars
        || products.az.evaluations.len() != expected
        || products.bz.evaluations.len() != expected
        || products.cz.evaluations.len() != expected
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_prover_inputs(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    h_rows: &[Vec<u64>],
    products: &R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<(), Sha256F2zError> {
    validate_public_statement(p_h, public_statement)?;
    validate_common(matrices, map, p_h, p_f)?;
    validate_rows(p_h, h_rows)?;
    validate_products(products, p_h)?;
    validate_ligerito_commitment(&hint_f.commitment, pc).map_err(Sha256F2zError::F2z)?;
    if hint_f.commitment.params.m != p_f.t + p_f.s {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_verifier_inputs(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    validate_public_statement(p_h, public_statement)?;
    validate_common(matrices, map, p_h, p_f)?;
    validate_ligerito_commitment(commitment_f, vc).map_err(Sha256F2zError::F2z)?;
    if commitment_f.params.m != p_f.t + p_f.s
        || proof.spartan().sumcheck.round_polynomials.len() != SHA256_CONSTRAINT_LOCAL_VARS + p_h.t
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_public_statement(
    p_h: &IntEvalParams,
    public_statement: &[Sha256CompressionStatement],
) -> Result<(), Sha256F2zError> {
    let expected = 1_usize
        .checked_shl(u32::try_from(p_h.t).map_err(|_| Sha256F2zError::InvalidGeometry)?)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    if public_statement.len() != expected {
        return Err(Sha256F2zError::InvalidPublicStatementLength {
            expected,
            actual: public_statement.len(),
        });
    }
    Ok(())
}

fn paper_assignment_binding(
    prepared: &PreparedSha256CompressionBatch,
    commitment: &Commitment,
    config: &impl LigeritoStatementConfig,
    public_statement_binding: &[u8; 32],
) -> Result<[u8; 32], Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_PAPER_ASSIGNMENT_DOMAIN);
    hash.update(&commitment.root);
    hash.update(prepared.integer_relation_digest());
    hash.update(&prepared.map().digest());
    hash.update(public_statement_binding);
    for value in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        prepared.assignment_params().t,
        prepared.assignment_params().s,
        prepared.assignment_params().word_bits,
        prepared.source_params().t,
        prepared.source_params().s,
        prepared.source_params().word_bits,
    ] {
        hash_usize(&mut hash, value)?;
    }
    hash.update(&[profile_code(commitment.params.profile)]);
    hash.update(&[hash_code(commitment.params.merkle_hash)]);
    hash_ligerito_config(&mut hash, config)?;
    Ok(*hash.finalize().as_bytes())
}

fn hash_ligerito_config(
    hash: &mut Hasher,
    config: &impl LigeritoStatementConfig,
) -> Result<(), Sha256F2zError> {
    for value in [
        config.recursive_steps(),
        config.initial_log_msg_cols(),
        config.initial_log_num_interleaved(),
        config.initial_k(),
    ] {
        hash_usize(hash, value)?;
    }
    for values in [
        config.log_inv_rates(),
        config.recursive_log_msg_cols(),
        config.recursive_ks(),
        config.queries(),
        config.grinding_bits(),
        config.fold_grinding_bits(),
        config.ood_samples(),
    ] {
        hash_usize(hash, values.len())?;
        for &value in values {
            hash_usize(hash, value)?;
        }
    }
    hash.update(&[hash_code(config.merkle_hash())]);
    Ok(())
}

fn absorb_sha256_paper_statement(
    transcript: &mut impl Transcript,
    prepared: &PreparedSha256CompressionBatch,
    public_statement_binding: &[u8; 32],
    assignment_binding: &[u8; 32],
) {
    absorb_spartan_message(transcript, b"protocol", SHA256_PAPER_SPARTAN_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"integer-relation",
        prepared.integer_relation_digest(),
    );
    absorb_spartan_message(transcript, b"boolean-map", &prepared.map().digest());
    absorb_spartan_message(
        transcript,
        b"instance-vars",
        &(prepared.assignment_params().t as u64).to_le_bytes(),
    );
    absorb_spartan_message(transcript, b"public-sha256-io", public_statement_binding);
    absorb_spartan_message(transcript, b"assignment-oracle", assignment_binding);
}

fn absorb_projected_sha256_relation(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
) {
    absorb_spartan_message(
        transcript,
        b"runtime-field-modulus",
        matrices.field_modulus_encoding(),
    );
    absorb_spartan_message(transcript, b"projected-matrices", matrices.digest());
}

fn assignment_binding(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    commitment: &Commitment,
) -> Result<[u8; 32], Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_ASSIGNMENT_BINDING_DOMAIN);
    hash.update(&commitment.root);
    hash.update(matrices.digest());
    hash.update(&map.digest());
    hash.update(&FQ_MOD.to_le_bytes());
    for value in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        p_h.t,
        p_h.s,
        p_h.word_bits,
        p_f.t,
        p_f.s,
        p_f.word_bits,
    ] {
        hash_usize(&mut hash, value)?;
    }
    hash.update(&[profile_code(commitment.params.profile)]);
    hash.update(&[hash_code(commitment.params.merkle_hash)]);
    Ok(*hash.finalize().as_bytes())
}

fn public_statement_binding(
    public_statement: &[Sha256CompressionStatement],
) -> Result<[u8; 32], Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_PUBLIC_STATEMENT_DOMAIN);
    hash_usize(&mut hash, public_statement.len())?;
    for statement in public_statement {
        for word in statement.words() {
            hash.update(&word.to_le_bytes());
        }
    }
    Ok(*hash.finalize().as_bytes())
}

fn absorb_sha256_statement(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    p_h: &IntEvalParams,
    public_statement_binding: &[u8; 32],
    assignment_binding: &[u8; 32],
) {
    absorb_spartan_message(transcript, b"protocol", SHA256_SPARTAN_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"field-modulus",
        matrices.field_modulus_encoding(),
    );
    absorb_spartan_message(transcript, b"matrix-statement", matrices.digest());
    absorb_spartan_message(transcript, b"instance-vars", &(p_h.t as u64).to_le_bytes());
    absorb_spartan_message(transcript, b"public-sha256-io", public_statement_binding);
    absorb_spartan_message(transcript, b"assignment-oracle", assignment_binding);
}

fn squeeze_constant_one_batch(
    transcript: &mut impl Transcript,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> SpartanF2zField {
    absorb_spartan_message(
        transcript,
        b"challenge-domain",
        SHA256_CONSTANT_ONE_BATCH_DOMAIN,
    );
    squeeze_field(transcript, field_config)
}

fn squeeze_public_io_batch(
    transcript: &mut impl Transcript,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> (Vec<SpartanF2zField>, SpartanF2zField) {
    absorb_spartan_message(
        transcript,
        b"challenge-domain",
        SHA256_PUBLIC_IO_BATCH_DOMAIN,
    );
    let public_slot_weights = (0..SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS)
        .map(|_| squeeze_field(transcript, field_config))
        .collect();
    let public_io_batch = squeeze_field(transcript, field_config);
    (public_slot_weights, public_io_batch)
}

#[allow(clippy::too_many_arguments)]
fn absorb_opening_claim(
    transcript: &mut impl Transcript,
    assignment_binding: &[u8; 32],
    row_point: &[SpartanF2zField],
    rho: &SpartanF2zField,
    constant_one_batch: &SpartanF2zField,
    public_slot_weights: &[SpartanF2zField],
    public_io_batch: &SpartanF2zField,
    row_weights_q: &[u128],
    col_weights: &[u128],
    claimed: u128,
) -> Result<(), Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_OPENING_CLAIM_DOMAIN);
    hash.update(assignment_binding);
    hash_usize(&mut hash, row_point.len())?;
    for coordinate in row_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&rho.canonical_element_encoding());
    hash.update(&constant_one_batch.canonical_element_encoding());
    hash_usize(&mut hash, public_slot_weights.len())?;
    for coordinate in public_slot_weights {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&public_io_batch.canonical_element_encoding());
    hash_usize(&mut hash, row_weights_q.len())?;
    for weight in row_weights_q {
        hash.update(&weight.to_le_bytes());
    }
    hash_usize(&mut hash, col_weights.len())?;
    for weight in col_weights {
        hash.update(&weight.to_le_bytes());
    }
    hash.update(&claimed.to_le_bytes());
    absorb_spartan_message(
        transcript,
        b"sha256-opening-claim",
        hash.finalize().as_bytes(),
    );
    Ok(())
}

fn hash_usize(hash: &mut Hasher, value: usize) -> Result<(), Sha256F2zError> {
    hash.update(
        &u64::try_from(value)
            .map_err(|_| Sha256F2zError::BindingEncodingOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        f2map::{PreparedVirtualMap, RepeatedVirtualMap},
        ligerito::IntEvalRsError,
        piop::spartan::sha256::{
            generate_sha256_compression_witnesses, generate_sha256_compression_witnesses_exact,
            prepare_sha256_compression_batch, prepare_sha256_compression_batch_integer,
        },
        poly::mle::DenseMultilinearExtension,
        sparse_matrix::SparseMatrix,
        transcript::Blake3Transcript,
    };

    use super::*;

    fn input(instance: usize) -> super::super::witness::Sha256CompressionInput {
        let mut state = [0u32; 8];
        let mut block = [0u32; 16];
        for (word, value) in state.iter_mut().enumerate() {
            *value = (instance as u32)
                .wrapping_mul(0x9e37_79b9)
                .rotate_left(word as u32);
        }
        for (word, value) in block.iter_mut().enumerate() {
            *value = (instance as u32 ^ word as u32)
                .wrapping_mul(0x85eb_ca6b)
                .rotate_right(word as u32);
        }
        (state, block)
    }

    fn public_statements(
        inputs: &[super::super::witness::Sha256CompressionInput],
        outputs: &[[u32; 8]],
    ) -> Vec<Sha256CompressionStatement> {
        inputs
            .iter()
            .copied()
            .zip(outputs.iter().copied())
            .map(|(input, output)| Sha256CompressionStatement::new(input, output))
            .collect()
    }

    fn zero_public_statements(instances: usize) -> Vec<Sha256CompressionStatement> {
        vec![Sha256CompressionStatement::new(([0_u32; 8], [0_u32; 16]), [0_u32; 8]); instances]
    }

    #[test]
    fn paper128_ligerito_profiles_cover_every_supported_sha_batch() {
        for log_compressions in 7..=16 {
            let p_f = IntEvalParams {
                t: log_compressions,
                s: SHA256_F_LOCAL_VARS,
                word_bits: 1,
            };
            let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
            assert_eq!(pc.merkle_hash, flock_core::merkle::HashKind::Blake3);
            assert_eq!(pc.log_inv_rates, vc.log_inv_rates);
            assert_eq!(pc.queries, vc.queries);
            assert_eq!(pc.fold_grinding_bits, vc.fold_grinding_bits);
            assert!(pc.fold_grinding_bits.iter().all(|&bits| bits > 0));

            let prepared = prepare_sha256_compression_batch_integer(log_compressions).unwrap();
            let profile = Sha256PrimeProfile::new(log_compressions).unwrap();
            assert!(
                prepared.max_boolean_residual_bound()
                    < &num_bigint::BigUint::from(profile.min_prime())
            );
        }

        let outside = IntEvalParams {
            t: 6,
            s: SHA256_F_LOCAL_VARS,
            word_bits: 1,
        };
        assert!(sha256_compression_configs(&outside).is_err());
    }

    #[test]
    fn paper128_runtime_prime_roundtrip() {
        // Pinned to the grinded legacy schedule: this test exercises the
        // per-round and initial/terminal grinding machinery, which the
        // λ = 100 default profile deliberately skips.
        const LOG_COMPRESSIONS: usize = 7;
        let prepared = super::super::super::prepare_sha256_compression_batch_integer_with_profile::<
            crate::piop::spartan::profile::LegacySha128Design,
        >(LOG_COMPRESSIONS)
        .unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses_exact(
            &inputs,
            prepared.source_params(),
            prepared.assignment_params(),
        )
        .unwrap();
        let public_statement = public_statements(&inputs, witness.outputs());
        let (pc, vc) = sha256_compression_configs_for(&prepared).unwrap();
        let hint = commit_sha256_paper128_witness_with_config(&prepared, &witness, &pc).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_paper128_with_config(
            &mut prover_transcript,
            &prepared,
            &public_statement,
            &witness,
            &hint,
            &pc,
        )
        .unwrap();
        assert_eq!(
            proof.outer_nonces().len(),
            SHA256_CONSTRAINT_LOCAL_VARS + LOG_COMPRESSIONS
        );

        let mut verifier_transcript = Blake3Transcript::new();
        verify_sha256_compressions_paper128_with_config(
            &mut verifier_transcript,
            &prepared,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
        )
        .unwrap();

        let mut rejected_nonce = false;
        for delta in 1..=8_u64 {
            let mut tampered = proof.clone();
            tampered.initial_nonce = tampered.initial_nonce.wrapping_add(delta);
            let mut tampered_transcript = Blake3Transcript::new();
            if verify_sha256_compressions_paper128_with_config(
                &mut tampered_transcript,
                &prepared,
                &public_statement,
                &hint.commitment,
                &tampered,
                &vc,
            )
            .is_err()
            {
                rejected_nonce = true;
                break;
            }
        }
        assert!(
            rejected_nonce,
            "modified initial grinding nonces must reject"
        );

        let mut false_statement = public_statement.clone();
        false_statement[3].claimed_output[0] ^= 1;
        let mut false_transcript = Blake3Transcript::new();
        assert!(verify_sha256_compressions_paper128_with_config(
            &mut false_transcript,
            &prepared,
            &false_statement,
            &hint.commitment,
            &proof,
            &vc,
        )
        .is_err());
    }

    #[test]
    fn paper128_rejects_shared_false_public_components() {
        const LOG_COMPRESSIONS: usize = 7;
        const ATTACKED_INSTANCE: usize = 5;

        let prepared = prepare_sha256_compression_batch_integer(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses_exact(
            &inputs,
            prepared.source_params(),
            prepared.assignment_params(),
        )
        .unwrap();
        let public_statement = public_statements(&inputs, witness.outputs());
        let (pc, vc) = sha256_compression_configs_for(&prepared).unwrap();
        let hint = commit_sha256_paper128_witness_with_config(&prepared, &witness, &pc).unwrap();

        // Prover and verifier deliberately agree on each false statement, so
        // rejection cannot come merely from transcript divergence: the
        // committed public-wire equality itself must fail.
        for component in 0..3 {
            let mut false_statement = public_statement.clone();
            match component {
                0 => false_statement[ATTACKED_INSTANCE].state[0] ^= 1,
                1 => false_statement[ATTACKED_INSTANCE].block[0] ^= 1,
                2 => false_statement[ATTACKED_INSTANCE].claimed_output[0] ^= 1,
                _ => unreachable!(),
            }

            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_sha256_compressions_paper128_with_config(
                &mut prover_transcript,
                &prepared,
                &false_statement,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();

            let mut verifier_transcript = Blake3Transcript::new();
            assert!(verify_sha256_compressions_paper128_with_config(
                &mut verifier_transcript,
                &prepared,
                &false_statement,
                &hint.commitment,
                &proof,
                &vc,
            )
            .is_err());
        }
    }

    #[test]
    fn sha256_rejects_prover_controlled_zero_constant_column() {
        const LOG_COMPRESSIONS: usize = 7;
        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
        let f_rows = vec![vec![0_u64; p_f.rows().div_ceil(64)]; p_f.cols()];

        assert!(matches!(
            commit_sha256_compression_witness_with_config(&p_f, f_rows.clone(), &pc),
            Err(Sha256F2zError::InvalidConstantOneColumn)
        ));

        // Bypass the honest SHA commitment wrapper: protocol soundness cannot
        // rely on a malicious prover using that wrapper.
        let hint = commit_rs_ligerito_rows(&p_f, f_rows, &pc);
        let h_rows = vec![vec![0_u64; p_h.rows().div_ceil(64)]; p_h.cols()];
        let num_vars = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
        let table_len = 1_usize << num_vars;
        let zero = SpartanF2zField::zero_with_cfg(matrices.config());
        let zero_mle = || {
            DenseMultilinearExtension::from_evaluations_vec(
                num_vars,
                vec![zero.clone(); table_len],
                zero.clone(),
            )
        };
        let products = R1csProductMles {
            az: zero_mle(),
            bz: zero_mle(),
            cz: zero_mle(),
        };
        let public_statement = zero_public_statements(p_h.rows());

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
            &mut prover_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &h_rows,
            products,
            &hint,
            &pc,
        )
        .unwrap();

        let mut verifier_transcript = Blake3Transcript::new();
        assert!(matches!(
            verify_sha256_compressions_spartan_and_f2z_with_config(
                &mut verifier_transcript,
                &matrices,
                &map,
                &p_h,
                &p_f,
                &public_statement,
                &hint.commitment,
                &proof,
                &vc,
            ),
            Err(Sha256F2zError::F2z(FlockRsError::Common(
                IntEvalRsError::ReadOff
            )))
        ));
    }

    #[test]
    fn sha256_rejects_one_zeroed_instance_in_an_honest_batch() {
        const LOG_COMPRESSIONS: usize = 7;
        const ATTACKED_INSTANCE: usize = 5;
        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1_usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let (mut f_rows, mut h_rows, mut products, outputs) =
            generate_sha256_compression_witnesses(&inputs, &p_f, &p_h, matrices.config()).unwrap();
        let mut public_statement = public_statements(&inputs, &outputs);

        let attacked_word = ATTACKED_INSTANCE / u64::BITS as usize;
        let attacked_mask = !(1_u64 << (ATTACKED_INSTANCE % u64::BITS as usize));
        for row in &mut f_rows {
            row[attacked_word] &= attacked_mask;
        }
        for row in &mut h_rows {
            row[attacked_word] &= attacked_mask;
        }
        let constraint_stride = super::super::constraints::SHA256_CONSTRAINT_STRIDE;
        let product_start = ATTACKED_INSTANCE * constraint_stride;
        let product_end = product_start + constraint_stride;
        let zero = SpartanF2zField::zero_with_cfg(matrices.config());
        for product in [&mut products.az, &mut products.bz, &mut products.cz] {
            product.evaluations[product_start..product_end].fill(zero.clone());
        }
        public_statement[ATTACKED_INSTANCE] =
            Sha256CompressionStatement::new(([0_u32; 8], [0_u32; 16]), [0_u32; 8]);

        let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
        assert!(matches!(
            commit_sha256_compression_witness_with_config(&p_f, f_rows.clone(), &pc),
            Err(Sha256F2zError::InvalidConstantOneColumn)
        ));
        let hint = commit_rs_ligerito_rows(&p_f, f_rows, &pc);

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
            &mut prover_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &h_rows,
            products,
            &hint,
            &pc,
        )
        .unwrap();

        let mut verifier_transcript = Blake3Transcript::new();
        assert!(matches!(
            verify_sha256_compressions_spartan_and_f2z_with_config(
                &mut verifier_transcript,
                &matrices,
                &map,
                &p_h,
                &p_f,
                &public_statement,
                &hint.commitment,
                &proof,
                &vc,
            ),
            Err(Sha256F2zError::F2z(FlockRsError::Common(
                IntEvalRsError::ReadOff
            )))
        ));
    }

    #[test]
    fn sha256_rejects_shared_false_public_components() {
        const LOG_COMPRESSIONS: usize = 7;
        const ATTACKED_INSTANCE: usize = 5;

        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1_usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let (f_rows, h_rows, products, outputs) =
            generate_sha256_compression_witnesses(&inputs, &p_f, &p_h, matrices.config()).unwrap();
        let public_statement = public_statements(&inputs, &outputs);

        let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&p_f, f_rows, &pc).unwrap();

        let mut short_statement_transcript = Blake3Transcript::new();
        assert!(matches!(
            prove_sha256_compressions_spartan_and_f2z_with_config(
                &mut short_statement_transcript,
                &matrices,
                &map,
                &p_h,
                &p_f,
                &public_statement[..public_statement.len() - 1],
                &h_rows,
                products.clone(),
                &hint,
                &pc,
            ),
            Err(Sha256F2zError::InvalidPublicStatementLength {
                expected: 128,
                actual: 127
            })
        ));

        // These are stronger than giving an honest proof to a verifier with a
        // different transcript statement: prover and verifier deliberately
        // agree on each false statement, so only the opening-to-wire equality
        // can reject it.
        for component in 0..3 {
            let mut false_statement = public_statement.clone();
            match component {
                0 => false_statement[ATTACKED_INSTANCE].state[0] ^= 1,
                1 => false_statement[ATTACKED_INSTANCE].block[0] ^= 1,
                2 => false_statement[ATTACKED_INSTANCE].claimed_output[0] ^= 1,
                _ => unreachable!(),
            }

            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
                &mut prover_transcript,
                &matrices,
                &map,
                &p_h,
                &p_f,
                &false_statement,
                &h_rows,
                products.clone(),
                &hint,
                &pc,
            )
            .unwrap();

            if component == 0 {
                let mut short_statement_verifier_transcript = Blake3Transcript::new();
                assert!(matches!(
                    verify_sha256_compressions_spartan_and_f2z_with_config(
                        &mut short_statement_verifier_transcript,
                        &matrices,
                        &map,
                        &p_h,
                        &p_f,
                        &false_statement[..false_statement.len() - 1],
                        &hint.commitment,
                        &proof,
                        &vc,
                    ),
                    Err(Sha256F2zError::InvalidPublicStatementLength {
                        expected: 128,
                        actual: 127
                    })
                ));
            }

            let mut verifier_transcript = Blake3Transcript::new();
            assert!(matches!(
                verify_sha256_compressions_spartan_and_f2z_with_config(
                    &mut verifier_transcript,
                    &matrices,
                    &map,
                    &p_h,
                    &p_f,
                    &false_statement,
                    &hint.commitment,
                    &proof,
                    &vc,
                ),
                Err(Sha256F2zError::F2z(FlockRsError::Common(
                    IntEvalRsError::ReadOff
                )))
            ));
        }
    }

    #[test]
    fn public_statement_binding_covers_every_public_component_and_instance_order() {
        let inputs = [input(1), input(2)];
        let outputs = [[3_u32; 8], [4_u32; 8]];
        let statement = public_statements(&inputs, &outputs);
        let binding = public_statement_binding(&statement).unwrap();

        let mut changed_state = statement.clone();
        changed_state[0].state[0] ^= 1;
        assert_ne!(public_statement_binding(&changed_state).unwrap(), binding);

        let mut changed_block = statement.clone();
        changed_block[0].block[0] ^= 1;
        assert_ne!(public_statement_binding(&changed_block).unwrap(), binding);

        let mut changed_output = statement.clone();
        changed_output[0].claimed_output[0] ^= 1;
        assert_ne!(public_statement_binding(&changed_output).unwrap(), binding);

        let mut swapped = statement.clone();
        swapped.swap(0, 1);
        assert_ne!(public_statement_binding(&swapped).unwrap(), binding);
    }

    #[test]
    fn public_statement_length_validation_rejects_overflowing_geometry() {
        let p_h = IntEvalParams {
            t: usize::BITS as usize,
            s: SHA256_H_LOCAL_VARS,
            word_bits: 1,
        };
        assert!(matches!(
            validate_public_statement(&p_h, &[]),
            Err(Sha256F2zError::InvalidGeometry)
        ));
    }

    #[test]
    fn sha256_virtualized_spartan_f2z_roundtrip() {
        const LOG_COMPRESSIONS: usize = 7;
        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let (f_rows, h_rows, products, outputs) =
            generate_sha256_compression_witnesses(&inputs, &p_f, &p_h, matrices.config()).unwrap();
        let public_statement = public_statements(&inputs, &outputs);
        let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&p_f, f_rows, &pc).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
            &mut prover_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &h_rows,
            products.clone(),
            &hint,
            &pc,
        )
        .unwrap();

        let mut verifier_transcript = Blake3Transcript::new();
        verify_sha256_compressions_spartan_and_f2z_with_config(
            &mut verifier_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
        )
        .unwrap();

        let empty_local = PreparedVirtualMap::new(
            SparseMatrix::try_from_columns(
                super::super::constraints::SHA256_H_STRIDE,
                vec![Vec::<(usize, bool)>::new(); super::super::constraints::SHA256_F_STRIDE],
            )
            .unwrap(),
        )
        .unwrap();
        let wrong_map = RepeatedVirtualMap::new(empty_local, 1usize << LOG_COMPRESSIONS).unwrap();
        let mut wrong_map_transcript = Blake3Transcript::new();
        assert!(verify_sha256_compressions_spartan_and_f2z_with_config(
            &mut wrong_map_transcript,
            &matrices,
            &wrong_map,
            &p_h,
            &p_f,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
        )
        .is_err());

        let mut inconsistent_h = h_rows;
        inconsistent_h[1][0] ^= 1;
        let mut inconsistent_prover_transcript = Blake3Transcript::new();
        let inconsistent_proof = prove_sha256_compressions_spartan_and_f2z_with_config(
            &mut inconsistent_prover_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &inconsistent_h,
            products,
            &hint,
            &pc,
        )
        .unwrap();
        let mut inconsistent_verifier_transcript = Blake3Transcript::new();
        assert!(verify_sha256_compressions_spartan_and_f2z_with_config(
            &mut inconsistent_verifier_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
            &public_statement,
            &hint.commitment,
            &inconsistent_proof,
            &vc,
        )
        .is_err());
    }
}
