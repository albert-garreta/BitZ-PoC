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
    f2map::{PackedRepeatedVirtualMap, VirtualMap, cell_count},
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigVirtProof, LigeritoStatementConfig,
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual_runtime,
        validate_ligerito_commitment, validated_udr_lig_configs_for_target,
        verify_mle_eval_mod_q_ligerito_virtual_runtime,
    },
    pcs::{IntEvalParams, ProjectCanonicalU128},
    transcript::traits::Transcript,
};

use super::super::{
    PreparedConstraintMatrices, SpartanError, SpartanField, absorb_spartan_message,
    f2z::{SpartanF2zField, f2z_generator, hash_code, profile_code},
    grinding::{GrindingDomain, GrindingError, GrindingRound, grind_and_absorb, verify_and_absorb},
    make_equality_factors,
    matrix::eq_table,
    squeeze_field,
    sumcheck::{
        OptimizedSumcheckReducer, OuterSumcheckProof, R1csProductMles,
        prove_outer_sumcheck_with_reducer_grinded,
    },
};

use super::{
    constraints::{
        PreparedSha256CompressionBatch, SHA256_CONSTRAINT_LOCAL_VARS, SHA256_H_BAR_LIVE_BITS,
        SHA256_H_INSTANCE_BITS, SHA256_H_LOCAL_VARS, SHA256_PUBLIC_WORD_BITS, SHA256_PUBLIC_WORDS,
        Sha256ConstraintError, sha256_public_f_column, sha256_public_h_column,
    },
    prime::{Sha256PrimeError, Sha256PrimeProfile, sample_sha256_mod_q_context},
    witness::{Sha256CompressionStatement, Sha256CompressionWitnessBatch, Sha256WitnessError},
};

const SHA256_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-sha256-opening/v3";
const SHA256_CONSTANT_ONE_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/constant-one-batch/v1";
const SHA256_PUBLIC_STATEMENT_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-statement/v1";
const SHA256_PUBLIC_IO_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-io-batch/v1";
const SHA256_SHARED_CONSTANT_CELL: usize = 0;
const SHA256_PROTOCOL_DOMAIN: &[u8] = b"f2z/spartan-sha256-compressions/runtime-prime/v1";
const SHA256_ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-sha256-assignment/runtime-prime/v1";

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

/// Runtime-prime SHA-256 proof with explicit Spartan Fiat--Shamir grinding
/// nonces. The prime itself is intentionally absent: the verifier re-derives
/// it after binding the commitment and public statement.
#[derive(Clone)]
pub struct Sha256CompressionProof {
    initial_nonce: u64,
    outer: OuterSumcheckProof<SpartanF2zField>,
    outer_nonces: Vec<u64>,
    terminal_nonce: u64,
    f2z: IntEvalRsLigVirtProof,
}

impl Sha256CompressionProof {
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

    /// The committed source assignment does not contain its shared leading one.
    #[error("SHA-256 source assignment has a malformed shared constant cell")]
    InvalidSharedConstant,

    /// The public input/output batch must contain one statement per committed
    /// compression instance.
    #[error("SHA-256 public statement length mismatch: expected {expected}, got {actual}")]
    InvalidPublicStatementLength { expected: usize, actual: usize },

    /// Ligerito configuration derivation failed.
    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    /// An explicit Ligerito configuration disagrees with the prepared
    /// security profile.
    #[error("Ligerito configuration does not match the prepared SHA-256 profile")]
    MismatchedLigeritoConfig,

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

/// Derives the BLAKE3/UDR Ligerito configuration selected by the prepared
/// batch's security profile.
pub fn sha256_compression_configs(
    prepared: &PreparedSha256CompressionBatch,
) -> Result<(LigProverConfig, LigVerifierConfig), Sha256F2zError> {
    let p_f = prepared.source_params();
    validate_source_params(p_f)?;
    validated_udr_lig_configs_for_target(packed_vars(p_f), prepared.security().ligerito_target_bits)
        .map_err(Sha256F2zError::LigeritoConfig)
}

/// Commits packed extended-source rows `[1 | f]` under an explicit config.
fn commit_source_rows_with_config(
    p_f: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_source_params(p_f)?;
    validate_rows(p_f, &rows)?;
    validate_shared_constant(&rows)?;
    let hint = commit_rs_ligerito_rows(p_f, rows, pc);
    validate_ligerito_commitment(&hint.commitment, pc).map_err(Sha256F2zError::F2z)?;
    Ok(hint)
}

/// Commits the q-independent source rows retained by an exact SHA witness.
///
/// This convenience API clones the packed source rows because the exact
/// witness must remain available for post-commitment product projection.
pub fn commit_sha256_compression_witness_with_config(
    prepared: &PreparedSha256CompressionBatch,
    witness: &Sha256CompressionWitnessBatch,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_ligerito_config_for(prepared, pc)?;
    validate_rows(prepared.source_params(), witness.source_rows())?;
    validate_shared_constant(witness.source_rows())?;
    commit_source_rows_with_config(prepared.source_params(), witness.source_rows().to_vec(), pc)
}

/// Commits a q-independent witness with the prepared security profile.
pub fn commit_sha256_compression_witness(
    prepared: &PreparedSha256CompressionBatch,
    witness: &Sha256CompressionWitnessBatch,
) -> Result<FlockCommitHint, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(prepared)?;
    commit_sha256_compression_witness_with_config(prepared, witness, &pc)
}

/// Proves the runtime-prime SHA profile: commit first, grind, derive a bounded
/// 112/113-bit prime, project exact integers, and run Spartan/F2Z under that
/// one runtime field configuration.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_with_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &Sha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let p_f = prepared.source_params();
    let p_h = prepared.assignment_params();
    let map = prepared.map();
    let profile =
        Sha256PrimeProfile::from_security(prepared.security(), prepared.log_instance_capacity())?;

    validate_public_statement(prepared.instances(), public_statement)?;
    validate_common_geometry(None, map, p_h, p_f)?;
    validate_ligerito_config_for(prepared, pc)?;
    validate_rows(p_f, witness.source_rows())?;
    validate_shared_constant(witness.source_rows())?;
    validate_rows(p_h, witness.assignment_rows())?;
    if witness.outputs().len() != prepared.instances() || hint_f.rows() != witness.source_rows() {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    validate_ligerito_commitment(&hint_f.commitment, pc).map_err(Sha256F2zError::F2z)?;
    if hint_f.commitment.params.m != p_f.t + p_f.s {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = crate::utils::prof::scope("sha256:statement_bind_prover");
        let public_statement_binding = public_statement_binding(public_statement)?;
        let assignment_binding =
            assignment_binding(prepared, &hint_f.commitment, pc, &public_statement_binding)?;
        absorb_sha256_statement(
            transcript,
            prepared,
            &public_statement_binding,
            &assignment_binding,
        );
        assignment_binding
    };

    // Protocol §2.1 Step 2: pre-draw grinding, prime sample, projection mod q.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce = {
        let _scope = crate::utils::prof::scope("sha256:initial_grinding_prove");
        grind_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
        )?
    };
    let mod_q = {
        let _scope = crate::utils::prof::scope("sha256:runtime_prime_sample_prover");
        sample_sha256_mod_q_context(transcript, profile)?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("sha256:relation_projection_prover");
        prepared.project(mod_q.field_config())?
    };
    let products = {
        let _scope = crate::utils::prof::scope("sha256:product_projection_prover");
        witness.project_products(mod_q.field_config())?
    };
    validate_common_geometry(Some(&matrices), map, p_h, p_f)?;
    validate_products(&products, map)?;
    absorb_projected_sha256_relation(transcript, &matrices);
    drop(step2_scope);

    // Step 3: the Spartan outer PIOP over F_q (round grinding included).
    let step3_scope = crate::utils::prof::scope("step3:piop_prove");
    let field_config = mod_q.field_config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + prepared.log_instance_capacity();
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let (equality_factors, reducer) = {
        let _scope = crate::utils::prof::scope("sha256:spartan_prepare_prover");
        let equality_factors =
            make_equality_factors(&tau, field_config).map_err(SpartanError::from)?;
        let reducer = OptimizedSumcheckReducer::new(field_config).map_err(SpartanError::from)?;
        (equality_factors, reducer)
    };
    let (outer, outer_nonces) = {
        let _scope = crate::utils::prof::scope("sha256:spartan_outer_prove");
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
        let _scope = crate::utils::prof::scope("sha256:terminal_grinding_prove");
        grind_boundary::<Sha256TerminalGrinding, _>(
            transcript,
            profile.terminal_grinding_bits() as u32,
        )?
    };

    let rho = squeeze_field(transcript, field_config);
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let (row_weights_q, col_weights_q, claimed_q) = {
        let _scope = crate::utils::prof::scope("sha256:opening_prepare_prover");
        factorized_opening_claim(
            &matrices,
            map,
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
        let _scope = crate::utils::prof::scope("sha256:opening_claim_absorb_prover");
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
        let _scope = crate::utils::prof::scope("sha256:f2z_prove");
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

    Ok(Sha256CompressionProof {
        initial_nonce,
        outer: outer.proof,
        outer_nonces,
        terminal_nonce,
        f2z,
    })
}

/// Proves with the prepared batch's derived Ligerito configuration.
pub fn prove_sha256_compressions<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &Sha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(prepared)?;
    prove_sha256_compressions_with_config(
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
pub fn verify_sha256_compressions_with_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    let p_f = prepared.source_params();
    let p_h = prepared.assignment_params();
    let map = prepared.map();
    let profile =
        Sha256PrimeProfile::from_security(prepared.security(), prepared.log_instance_capacity())?;

    validate_public_statement(prepared.instances(), public_statement)?;
    validate_common_geometry(None, map, p_h, p_f)?;
    validate_ligerito_config_for(prepared, vc)?;
    validate_ligerito_commitment(commitment_f, vc).map_err(Sha256F2zError::F2z)?;
    if commitment_f.params.m != p_f.t + p_f.s
        || proof.outer.sumcheck.round_polynomials.len()
            != SHA256_CONSTRAINT_LOCAL_VARS + prepared.log_instance_capacity()
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = crate::utils::prof::scope("sha256:statement_bind_verifier");
        let public_statement_binding = public_statement_binding(public_statement)?;
        let assignment_binding =
            assignment_binding(prepared, commitment_f, vc, &public_statement_binding)?;
        absorb_sha256_statement(
            transcript,
            prepared,
            &public_statement_binding,
            &assignment_binding,
        );
        assignment_binding
    };
    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    {
        let _scope = crate::utils::prof::scope("sha256:initial_grinding_verify");
        check_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
            proof.initial_nonce,
        )?;
    }
    let mod_q = {
        let _scope = crate::utils::prof::scope("sha256:runtime_prime_sample_verifier");
        sample_sha256_mod_q_context(transcript, profile)?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("sha256:relation_projection_verifier");
        prepared.project(mod_q.field_config())?
    };
    validate_common_geometry(Some(&matrices), map, p_h, p_f)?;
    absorb_projected_sha256_relation(transcript, &matrices);
    drop(step2_scope);

    let step3_scope = crate::utils::prof::scope("step3:piop_verify");
    let field_config = mod_q.field_config();
    let row_vars = SHA256_CONSTRAINT_LOCAL_VARS + prepared.log_instance_capacity();
    let tau = (0..row_vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<SpartanF2zField>>();
    let outer = {
        let _scope = crate::utils::prof::scope("sha256:spartan_outer_verify");
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
        let _scope = crate::utils::prof::scope("sha256:terminal_grinding_verify");
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
        let _scope = crate::utils::prof::scope("sha256:opening_prepare_verifier");
        factorized_opening_claim(
            &matrices,
            map,
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
        let _scope = crate::utils::prof::scope("sha256:opening_claim_absorb_verifier");
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
    let _scope = crate::utils::prof::scope("sha256:f2z_verify");
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

/// Verifies with the prepared batch's derived Ligerito configuration.
pub fn verify_sha256_compressions<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
) -> Result<(), Sha256F2zError> {
    let (_, vc) = sha256_compression_configs(prepared)?;
    verify_sha256_compressions_with_config(
        transcript,
        prepared,
        public_statement,
        commitment_f,
        proof,
        &vc,
    )
}

fn factorized_opening_claim(
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &PackedRepeatedVirtualMap,
    p_h: &IntEvalParams,
    row_point: &[SpartanF2zField],
    rho: &SpartanF2zField,
    constant_one_batch: &SpartanF2zField,
    public_statement: &[Sha256CompressionStatement],
    public_slot_weights: &[SpartanF2zField],
    public_io_batch: &SpartanF2zField,
    claimed: SpartanF2zField,
) -> Result<(Vec<u128>, Vec<u128>, u128), Sha256F2zError> {
    let instance_vars = map.instances().next_power_of_two().ilog2() as usize;
    let expected = SHA256_CONSTRAINT_LOCAL_VARS + instance_vars;
    if row_point.len() != expected
        || p_h.s != 0
        || public_slot_weights.len() != SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    validate_public_statement(map.instances(), public_statement)?;

    let (local_point, instance_point) = row_point.split_at(SHA256_CONSTRAINT_LOCAL_VARS);
    let instance_weights =
        eq_table(instance_point, matrices.config()).map_err(SpartanError::from)?;
    let mut local_columns = matrices
        .bind_and_batch(local_point, rho)
        .map_err(SpartanError::from)?;
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

    // Convert the factorized repeated-matrix functional into the packed
    // assignment layout.  Every live instance is adjacent to the preceding
    // one; the only zero padding is the final suffix of this vector.
    let zero = SpartanF2zField::zero_with_cfg(matrices.config());
    let mut packed_weights = vec![zero.clone(); p_h.rows()];
    let mut active_weight_sum = zero.clone();
    for (instance, instance_weight) in instance_weights.iter().take(map.instances()).enumerate() {
        active_weight_sum += instance_weight;
        let start = 1 + instance * SHA256_H_INSTANCE_BITS;
        let end = start + SHA256_H_INSTANCE_BITS;
        if end > packed_weights.len() {
            return Err(Sha256F2zError::InvalidGeometry);
        }
        for (target, coefficient) in packed_weights[start..end]
            .iter_mut()
            .zip(&local_columns.evaluations[1..SHA256_H_BAR_LIVE_BITS])
        {
            *target = instance_weight.clone() * coefficient;
        }
    }
    packed_weights[SHA256_SHARED_CONSTANT_CELL] = active_weight_sum
        * &local_columns.evaluations[SHA256_SHARED_CONSTANT_CELL]
        + constant_one_batch;

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

    let row_weights_q = packed_weights
        .into_iter()
        .map(|value| value.canonical_u128())
        .collect::<Vec<_>>();
    let col_weights = vec![SpartanF2zField::one_with_cfg(matrices.config()).canonical_u128()];
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
        || p_f.s != 0
        || p_f.t < LOG_PACKING
        || p_f.t.saturating_add(p_f.s) > 126
        || p_f.t >= host_bits
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_common_geometry(
    matrices: Option<&PreparedConstraintMatrices<SpartanF2zField>>,
    map: &PackedRepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
) -> Result<(), Sha256F2zError> {
    validate_source_params(p_f)?;
    if p_h.word_bits != 1
        || p_h.s != 0
        || map.rows() != cell_count(p_h)
        || map.cols() != cell_count(p_f)
        || map.local().rows() != SHA256_H_BAR_LIVE_BITS
        || map.local().cols() != super::constraints::SHA256_F_BAR_LIVE_BITS
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

fn map_fixes_constant_assignment(map: &PackedRepeatedVirtualMap) -> bool {
    let mut constant_source = None;
    for (column, entries) in map.local().matrix().columns().enumerate() {
        for row in entries.row_indices() {
            if *row == SHA256_SHARED_CONSTANT_CELL && constant_source.replace(column).is_some() {
                return false;
            }
        }
    }
    constant_source == Some(SHA256_SHARED_CONSTANT_CELL)
}

fn map_fixes_public_statement(map: &PackedRepeatedVirtualMap) -> bool {
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

fn validate_shared_constant(rows: &[Vec<u64>]) -> Result<(), Sha256F2zError> {
    let packed = rows
        .get(SHA256_SHARED_CONSTANT_CELL)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    if packed.first().is_none_or(|word| word & 1 == 0) {
        return Err(Sha256F2zError::InvalidSharedConstant);
    }
    Ok(())
}

fn validate_products(
    products: &R1csProductMles<SpartanF2zField>,
    map: &PackedRepeatedVirtualMap,
) -> Result<(), Sha256F2zError> {
    let num_vars =
        SHA256_CONSTRAINT_LOCAL_VARS + map.instances().next_power_of_two().ilog2() as usize;
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

fn validate_public_statement(
    expected: usize,
    public_statement: &[Sha256CompressionStatement],
) -> Result<(), Sha256F2zError> {
    if public_statement.len() != expected {
        return Err(Sha256F2zError::InvalidPublicStatementLength {
            expected,
            actual: public_statement.len(),
        });
    }
    Ok(())
}

fn assignment_binding(
    prepared: &PreparedSha256CompressionBatch,
    commitment: &Commitment,
    config: &impl LigeritoStatementConfig,
    public_statement_binding: &[u8; 32],
) -> Result<[u8; 32], Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_ASSIGNMENT_BINDING_DOMAIN);
    hash.update(&commitment.root);
    hash.update(prepared.integer_relation_digest());
    hash.update(&prepared.map().digest());
    hash.update(public_statement_binding);
    for value in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        prepared.instances(),
        prepared.log_instance_capacity(),
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
    hash_security_profile(&mut hash, prepared)?;
    hash_ligerito_config(&mut hash, config)?;
    Ok(*hash.finalize().as_bytes())
}

fn validate_ligerito_config_for(
    prepared: &PreparedSha256CompressionBatch,
    actual: &impl LigeritoStatementConfig,
) -> Result<(), Sha256F2zError> {
    let (expected, _) = sha256_compression_configs(prepared)?;
    if ligerito_config_digest(&expected)? != ligerito_config_digest(actual)? {
        return Err(Sha256F2zError::MismatchedLigeritoConfig);
    }
    Ok(())
}

fn ligerito_config_digest(
    config: &impl LigeritoStatementConfig,
) -> Result<[u8; 32], Sha256F2zError> {
    let mut hash = Hasher::new();
    hash_ligerito_config(&mut hash, config)?;
    Ok(*hash.finalize().as_bytes())
}

fn hash_security_profile(
    hash: &mut Hasher,
    prepared: &PreparedSha256CompressionBatch,
) -> Result<(), Sha256F2zError> {
    let security = prepared.security();
    hash_usize(hash, security.profile_name.len())?;
    hash.update(security.profile_name.as_bytes());
    hash.update(&security.lambda.to_le_bytes());
    hash.update(&security.projection_min.to_le_bytes());
    hash.update(&security.projection_max.to_le_bytes());
    hash.update(&[u8::from(security.projection_full_width)]);
    hash.update(&security.initial_grinding_bits.to_le_bytes());
    hash.update(&security.piop_round_grinding_bits.to_le_bytes());
    hash.update(&security.terminal_grinding_bits.to_le_bytes());
    match security.reduction {
        Some(reduction) => {
            hash.update(&[1]);
            hash.update(&reduction.min.to_le_bytes());
            hash.update(&reduction.max.to_le_bytes());
            hash.update(&reduction.grinding_bits.to_le_bytes());
        }
        None => {
            hash.update(&[0]);
        }
    }
    hash.update(&security.forest_round_grinding_bits.to_le_bytes());
    hash.update(&security.ring_switch_grinding_bits.to_le_bytes());
    hash_usize(hash, security.ligerito_target_bits)?;
    Ok(())
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

fn absorb_sha256_statement(
    transcript: &mut impl Transcript,
    prepared: &PreparedSha256CompressionBatch,
    public_statement_binding: &[u8; 32],
    assignment_binding: &[u8; 32],
) {
    absorb_spartan_message(transcript, b"protocol", SHA256_PROTOCOL_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"integer-relation",
        prepared.integer_relation_digest(),
    );
    absorb_spartan_message(transcript, b"boolean-map", &prepared.map().digest());
    absorb_spartan_message(
        transcript,
        b"instance-vars",
        &(prepared.log_instance_capacity() as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"instance-count",
        &(prepared.instances() as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"assignment-domain-vars",
        &(prepared.assignment_params().t as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"source-domain-vars",
        &(prepared.source_params().t as u64).to_le_bytes(),
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
    use crypto_primitives::ConstIntSemiring;

    use crate::{
        piop::spartan::sha256::{
            generate_sha256_compression_witnesses, prepare_sha256_compression_batch,
            prepare_sha256_compression_batch_for_assignment_rows,
        },
        transcript::{
            Blake3Transcript,
            traits::{ConstTranscribable, Transcript},
        },
        utils::primality::PrimalityTest,
    };

    use super::*;

    struct RecordingTranscript {
        inner: Blake3Transcript,
        pre_challenge_bytes: Vec<u8>,
        challenge_seen: bool,
    }

    impl RecordingTranscript {
        fn new() -> Self {
            Self {
                inner: Blake3Transcript::new(),
                pre_challenge_bytes: Vec::new(),
                challenge_seen: false,
            }
        }

        fn absorbed_before_first_challenge(&self, needle: &[u8]) -> bool {
            self.pre_challenge_bytes
                .windows(needle.len())
                .any(|window| window == needle)
        }
    }

    impl Transcript for RecordingTranscript {
        fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
            self.challenge_seen = true;
            self.inner.get_challenge()
        }

        fn get_prime<R: ConstIntSemiring + ConstTranscribable, P: PrimalityTest<R>>(
            &mut self,
        ) -> R {
            self.challenge_seen = true;
            self.inner.get_prime::<R, P>()
        }

        fn absorb_inner(&mut self, value: &[u8]) {
            if !self.challenge_seen {
                self.pre_challenge_bytes.extend_from_slice(value);
            }
            self.inner.absorb_inner(value);
        }
    }

    fn assert_nonce_mutation_rejects(
        prepared: &PreparedSha256CompressionBatch,
        public_statement: &[Sha256CompressionStatement],
        commitment: &Commitment,
        proof: &Sha256CompressionProof,
        vc: &LigVerifierConfig,
        mutate: impl Fn(&mut Sha256CompressionProof, u64),
    ) {
        let rejected = (1..=32_u64).any(|delta| {
            let mut tampered = proof.clone();
            mutate(&mut tampered, delta);
            let mut transcript = Blake3Transcript::new();
            verify_sha256_compressions_with_config(
                &mut transcript,
                prepared,
                public_statement,
                commitment,
                &tampered,
                vc,
            )
            .is_err()
        });
        assert!(rejected, "at least one modified grinding nonce must reject");
    }

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

    #[test]
    fn ligerito_profiles_cover_every_supported_sha_batch() {
        for log_compressions in 7..=16 {
            let prepared = prepare_sha256_compression_batch(log_compressions).unwrap();
            let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
            assert_eq!(pc.merkle_hash, flock_core::merkle::HashKind::Blake3);
            assert_eq!(pc.log_inv_rates, vc.log_inv_rates);
            assert_eq!(pc.queries, vc.queries);
            assert_eq!(pc.fold_grinding_bits, vc.fold_grinding_bits);
            assert_eq!(pc.fold_grinding_bits.len(), pc.recursive_steps + 1);

            let profile = Sha256PrimeProfile::from_security(
                prepared.security(),
                prepared.log_instance_capacity(),
            )
            .unwrap();
            assert!(
                prepared.max_boolean_residual_bound()
                    < &num_bigint::BigUint::from(profile.min_prime)
            );
        }

        let by_rows = prepare_sha256_compression_batch_for_assignment_rows(21).unwrap();
        assert_eq!(by_rows.instances(), 102);
        assert_eq!(by_rows.log_instance_capacity(), 7);
        assert_eq!(by_rows.assignment_params().cells(), 1 << 21);
        sha256_compression_configs(&by_rows).unwrap();
        Sha256PrimeProfile::from_security(by_rows.security(), by_rows.log_instance_capacity())
            .unwrap();
    }

    #[test]
    fn runtime_prime_roundtrip() {
        // Pinned to the grinded reference schedule: this test exercises the
        // per-round and initial/terminal grinding machinery, which the
        // λ = 100 default profile deliberately skips.
        const LOG_COMPRESSIONS: usize = 7;
        let prepared = super::super::super::prepare_sha256_compression_batch_with_profile::<
            crate::piop::spartan::profile::Sha128ReferenceSchedule,
        >(LOG_COMPRESSIONS)
        .unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let public_statement = public_statements(&inputs, witness.outputs());
        let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();

        let public_binding = public_statement_binding(&public_statement).unwrap();
        let expected_assignment_binding =
            assignment_binding(&prepared, &hint.commitment, &pc, &public_binding).unwrap();

        let short_statement = &public_statement[..public_statement.len() - 1];
        let mut short_prover_transcript = Blake3Transcript::new();
        let mut untouched = short_prover_transcript.clone();
        assert!(matches!(
            prove_sha256_compressions_with_config(
                &mut short_prover_transcript,
                &prepared,
                short_statement,
                &witness,
                &hint,
                &pc,
            ),
            Err(Sha256F2zError::InvalidPublicStatementLength {
                expected: 128,
                actual: 127
            })
        ));
        assert_eq!(
            short_prover_transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>(),
            "preflight rejection must not mutate the transcript"
        );

        let default_prepared = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let (wrong_pc, wrong_vc) = sha256_compression_configs(&default_prepared).unwrap();
        assert!(matches!(
            commit_sha256_compression_witness_with_config(&prepared, &witness, &wrong_pc),
            Err(Sha256F2zError::MismatchedLigeritoConfig)
        ));
        let mut wrong_config_transcript = Blake3Transcript::new();
        assert!(matches!(
            prove_sha256_compressions_with_config(
                &mut wrong_config_transcript,
                &prepared,
                &public_statement,
                &witness,
                &hint,
                &wrong_pc,
            ),
            Err(Sha256F2zError::MismatchedLigeritoConfig)
        ));

        let mut missing_constant = witness.clone();
        missing_constant.source_rows_mut_for_tests()[0][0] &= !1;
        assert!(matches!(
            commit_sha256_compression_witness_with_config(&prepared, &missing_constant, &pc),
            Err(Sha256F2zError::InvalidSharedConstant)
        ));
        let bypass_hint = commit_rs_ligerito_rows(
            prepared.source_params(),
            missing_constant.source_rows().to_vec(),
            &pc,
        );
        let mut bypass_transcript = Blake3Transcript::new();
        let mut untouched = bypass_transcript.clone();
        assert!(matches!(
            prove_sha256_compressions_with_config(
                &mut bypass_transcript,
                &prepared,
                &public_statement,
                &missing_constant,
                &bypass_hint,
                &pc,
            ),
            Err(Sha256F2zError::InvalidSharedConstant)
        ));
        assert_eq!(
            bypass_transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>(),
            "malformed witnesses must reject before transcript absorption"
        );

        let mut prover_transcript = RecordingTranscript::new();
        let proof = prove_sha256_compressions_with_config(
            &mut prover_transcript,
            &prepared,
            &public_statement,
            &witness,
            &hint,
            &pc,
        )
        .unwrap();
        assert!(
            prover_transcript.absorbed_before_first_challenge(&expected_assignment_binding),
            "the commitment, public statement, profile, and PCS config must be bound before q"
        );
        assert_eq!(
            proof.outer_nonces().len(),
            SHA256_CONSTRAINT_LOCAL_VARS + LOG_COMPRESSIONS
        );

        let mut verifier_transcript = Blake3Transcript::new();
        verify_sha256_compressions_with_config(
            &mut verifier_transcript,
            &prepared,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
        )
        .unwrap();

        let mut short_verifier_transcript = Blake3Transcript::new();
        let mut untouched = short_verifier_transcript.clone();
        assert!(matches!(
            verify_sha256_compressions_with_config(
                &mut short_verifier_transcript,
                &prepared,
                short_statement,
                &hint.commitment,
                &proof,
                &vc,
            ),
            Err(Sha256F2zError::InvalidPublicStatementLength {
                expected: 128,
                actual: 127
            })
        ));
        assert_eq!(
            short_verifier_transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>(),
            "verifier preflight rejection must not mutate the transcript"
        );

        let mut wrong_config_transcript = Blake3Transcript::new();
        assert!(matches!(
            verify_sha256_compressions_with_config(
                &mut wrong_config_transcript,
                &prepared,
                &public_statement,
                &hint.commitment,
                &proof,
                &wrong_vc,
            ),
            Err(Sha256F2zError::MismatchedLigeritoConfig)
        ));

        assert_nonce_mutation_rejects(
            &prepared,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
            |proof, delta| {
                proof.initial_nonce = proof.initial_nonce.wrapping_add(delta);
            },
        );
        assert_nonce_mutation_rejects(
            &prepared,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
            |proof, delta| {
                proof.outer_nonces[0] = proof.outer_nonces[0].wrapping_add(delta);
            },
        );
        assert_nonce_mutation_rejects(
            &prepared,
            &public_statement,
            &hint.commitment,
            &proof,
            &vc,
            |proof, delta| {
                proof.terminal_nonce = proof.terminal_nonce.wrapping_add(delta);
            },
        );

        let mut wrong_commitment = hint.commitment.clone();
        wrong_commitment.root[0] ^= 1;
        let mut wrong_commitment_transcript = Blake3Transcript::new();
        assert!(
            verify_sha256_compressions_with_config(
                &mut wrong_commitment_transcript,
                &prepared,
                &public_statement,
                &wrong_commitment,
                &proof,
                &vc,
            )
            .is_err()
        );

        let mut false_statement = public_statement.clone();
        false_statement[3].claimed_output[0] ^= 1;
        let mut false_transcript = Blake3Transcript::new();
        assert!(
            verify_sha256_compressions_with_config(
                &mut false_transcript,
                &prepared,
                &false_statement,
                &hint.commitment,
                &proof,
                &vc,
            )
            .is_err()
        );
    }

    #[test]
    fn runtime_prime_rejects_shared_false_public_components() {
        const LOG_COMPRESSIONS: usize = 7;
        const ATTACKED_INSTANCE: usize = 5;

        let prepared = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let public_statement = public_statements(&inputs, witness.outputs());
        let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();

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
            let proof = prove_sha256_compressions_with_config(
                &mut prover_transcript,
                &prepared,
                &false_statement,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();

            let mut verifier_transcript = Blake3Transcript::new();
            assert!(
                verify_sha256_compressions_with_config(
                    &mut verifier_transcript,
                    &prepared,
                    &false_statement,
                    &hint.commitment,
                    &proof,
                    &vc,
                )
                .is_err()
            );
        }

        // Cell 769 is the first non-input wire in the first local assignment.
        // Changing it while retaining the source commitment and exact R1CS
        // products must not produce an accepting proof.
        const DERIVED_ASSIGNMENT_CELL: usize = 769;
        let mut inconsistent_witness = witness.clone();
        inconsistent_witness.assignment_rows_mut_for_tests()[0][DERIVED_ASSIGNMENT_CELL / 64] ^=
            1 << (DERIVED_ASSIGNMENT_CELL % 64);
        let mut inconsistent_prover_transcript = Blake3Transcript::new();
        if let Ok(inconsistent_proof) = prove_sha256_compressions_with_config(
            &mut inconsistent_prover_transcript,
            &prepared,
            &public_statement,
            &inconsistent_witness,
            &hint,
            &pc,
        ) {
            let mut inconsistent_verifier_transcript = Blake3Transcript::new();
            assert!(
                verify_sha256_compressions_with_config(
                    &mut inconsistent_verifier_transcript,
                    &prepared,
                    &public_statement,
                    &hint.commitment,
                    &inconsistent_proof,
                    &vc,
                )
                .is_err()
            );
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
    fn public_statement_length_validation_rejects_mismatched_count() {
        assert!(matches!(
            validate_public_statement(1, &[]),
            Err(Sha256F2zError::InvalidPublicStatementLength {
                expected: 1,
                actual: 0
            })
        ));
    }
}
