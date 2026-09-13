//! Product-structured linear SHA-256 PIOP followed by the virtual F2Z opening.
//!
//! The generated SHA circuit has empty R1CS `A` and `B` matrices, so every
//! live row is the integer-linear equation `C h = 0`. Batch rows are challenged
//! as the product of an eight-variable local-row equality tensor and an
//! instance equality tensor. This collapses the local matrix once to
//! `β = Cᵀ eq(·, ξ)`, while the physical witness remains gap-free at
//! `1 + instance * (L - 1) + (local_column - 1)`. Shared-one and public-I/O
//! coefficients are folded into the same local vector. For power-of-two
//! batches, a proof-only local-column × instance view of `h` makes the final
//! coefficient rank one across F2Z's row/column split, so F2Z opens the batched
//! residual directly without an assignment-domain sumcheck. The committed
//! source remains the canonical gap-free `f`; the virtual map binds the
//! proof-only view back to that commitment. Non-power-of-two assignment-row
//! batches retain the legacy inner-sumcheck fallback.

use crate::poly::mle::FactoredMultilinearExtension;
use std::collections::{HashMap, hash_map::Entry};

use blake3::Hasher;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crypto_primitives::{
    PrimeField, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint,
};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};
use thiserror::Error;

use crate::{
    f2map::{
        PackedRepeatedVirtualMap, PackedSourceOrder, PackedSourceRepeatedVirtualMap, VirtualMap,
        cell_count,
    },
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigVirtProof, LigeritoStatementConfig,
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime,
        validate_ligerito_commitment,
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime,
    },
    pcs::{GeneratedModQWeightSource, IntegerMatrixLayout, ModQWeightSource, ProjectCanonicalU128},
    transcript::traits::Transcript,
};

use super::super::{
    SpartanError, SpartanField, absorb_spartan_message,
    f2z::{SpartanF2zField, f2z_generator, hash_code, profile_code},
    grinding::{GrindingDomain, GrindingError, GrindingRound, grind_and_absorb, verify_and_absorb},
    matrix::eq_table,
    squeeze_field,
    sumcheck::{OptimizedSumcheckReducer, SumcheckError, SumcheckLinearReducer, SumcheckProof},
};

use super::{
    constraints::{
        PreparedSha256CompressionBatch, PreparedSha256LinearRelation, SHA256_CONSTRAINTS,
        SHA256_H_BAR_LIVE_BITS, SHA256_PUBLIC_WORD_BITS, SHA256_PUBLIC_WORDS,
        Sha256ConstraintError, sha256_public_f_column, sha256_public_h_column,
    },
    inner_sumcheck::{
        SHA256_INNER_PREFIX_MAX_VARS, prove_sha256_inner_sumcheck_factored,
        verify_sha256_inner_sumcheck,
    },
    prime::{Sha256PrimeError, sample_sha256_mod_q_context},
    witness::{Sha256CompressionStatement, Sha256CompressionWitnessBatch},
};

#[cfg(test)]
use super::prime::Sha256PrimeProfile;

const SHA256_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-sha256-opening/v7";
const SHA256_CONSTANT_ONE_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/constant-one-batch/v3";
const SHA256_PUBLIC_STATEMENT_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-statement/v1";
const SHA256_PUBLIC_IO_BATCH_DOMAIN: &[u8] = b"f2z/spartan-sha256/public-io-batch/v3";
const SHA256_LOCAL_ROW_POINT_DOMAIN: &[u8] = b"f2z/spartan-sha256/local-row-point/v1";
const SHA256_INSTANCE_POINT_DOMAIN: &[u8] = b"f2z/spartan-sha256/instance-point/v1";
const SHA256_SHARED_CONSTANT_CELL: usize = 0;
const SHA256_PROTOCOL_DOMAIN: &[u8] = b"f2z/spartan-sha256-compressions/product-linear/v2";
const SHA256_ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-sha256-assignment/runtime-prime/v2";

/// One runtime-field element in Montgomery form, without cloning the shared
/// 128-bit modulus configuration into every dense table entry.
pub(super) type RawMontgomery = u128;

/// Default number of low witness-position variables handled by the packed
/// native-bit prefix kernel.  This is a prover-local performance choice and is
/// deliberately absent from the proof and transcript.
pub const SHA256_DEFAULT_INNER_PREFIX_VARS: usize = 2;

enum Sha256InitialGrinding {}
enum Sha256PublicBatchGrinding {}

impl GrindingDomain for Sha256InitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/initial/v1";
}

impl GrindingDomain for Sha256PublicBatchGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/public-batch/v1";
}

/// Grinds one boundary, or skips it entirely at difficulty 0 (the λ = 100
/// profiles): no transcript bytes move and the stored nonce is 0.
pub(super) fn grind_boundary<D: GrindingDomain, T: Transcript>(
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
pub(super) fn check_boundary<D: GrindingDomain, T: Transcript>(
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
    inner: SumcheckProof<SpartanF2zField, 3>,
    inner_nonces: Vec<u64>,
    terminal_nonce: u64,
    f2z: IntEvalRsLigVirtProof,
}

impl Sha256CompressionProof {
    /// Initial commit-before-prime grinding nonce.
    pub const fn initial_nonce(&self) -> u64 {
        self.initial_nonce
    }

    /// Empty for direct product openings; populated only by the legacy
    /// non-power-of-two fallback.
    pub const fn inner(&self) -> &SumcheckProof<SpartanF2zField, 3> {
        &self.inner
    }

    /// Nonces adjacent to legacy inner-sumcheck round messages.
    pub fn inner_nonces(&self) -> &[u64] {
        &self.inner_nonces
    }

    /// Nonce before the public/shared-one batching challenges and residual proof.
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

    /// The packed native prefix kernel supports only K = 0, ..., 4.
    #[error("SHA-256 inner prefix must be in 0..={max}, got {actual}")]
    InvalidInnerPrefix { actual: usize, max: usize },

    /// The legacy prover's final inner-sumcheck claim did not equal
    /// `V(r_h) H(r_h)`.
    #[error("SHA-256 inner sumcheck has an inconsistent terminal product")]
    InvalidInnerTerminalClaim,

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

    /// A chain statement's blocks or digest disagree with the witness the
    /// prover was handed.
    #[error("SHA-256 chain statement does not match the witness")]
    ChainStatementMismatch,

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

    /// A Fiat--Shamir grinding nonce could not be produced or checked.
    #[error(transparent)]
    Grinding(#[from] GrindingError),

    /// A SHA sumcheck or sparse linear reduction rejected.
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
    let f_layout = prepared.source_params();
    validate_source_params(f_layout)?;
    let resolved = prepared.ligerito_configuration()?;
    Ok((resolved.prover().clone(), resolved.verifier().clone()))
}

/// Commits packed extended-source rows `[1 | f]` under an explicit config.
pub(super) fn commit_source_rows_with_config(
    f_layout: &IntegerMatrixLayout,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_source_params(f_layout)?;
    validate_rows(f_layout, &rows)?;
    validate_shared_constant(&rows)?;
    let hint = commit_rs_ligerito_rows(f_layout, rows, pc);
    validate_ligerito_commitment(&hint.commitment, pc).map_err(Sha256F2zError::F2z)?;
    Ok(hint)
}

/// Commits the q-independent source row retained by a packed SHA witness.
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

/// Proves with an explicit packed-prefix width and Ligerito configuration.
///
/// `prefix_vars` must be in `0..=4`. It only selects the prover kernel for the
/// non-power-of-two legacy fallback; direct product openings ignore it. It is
/// neither serialized nor absorbed.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_with_prefix_vars_and_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &Sha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
    prefix_vars: usize,
    pc: &LigProverConfig,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let f_layout = prepared.source_params();
    let h_layout = prepared.assignment_params();
    let map = prepared.map();
    let profile = prepared.prime_profile();

    validate_public_statement(prepared.instances(), public_statement)?;
    validate_common_geometry(None, map, h_layout, f_layout)?;
    validate_ligerito_config_for(prepared, pc)?;
    validate_rows(f_layout, witness.source_rows())?;
    validate_shared_constant(witness.source_rows())?;
    validate_rows(h_layout, witness.assignment_rows())?;
    let product_layout = match (
        prepared.product_map(),
        prepared.product_assignment_params(),
        witness.product_assignment_rows(),
    ) {
        (Some(product_map), Some(product_p_h), Some(product_rows)) => {
            validate_product_geometry(product_map, product_p_h, f_layout)?;
            validate_rows(product_p_h, product_rows)?;
            true
        }
        (None, None, None) => false,
        _ => return Err(Sha256F2zError::InvalidGeometry),
    };
    if prefix_vars > SHA256_INNER_PREFIX_MAX_VARS {
        return Err(Sha256F2zError::InvalidInnerPrefix {
            actual: prefix_vars,
            max: SHA256_INNER_PREFIX_MAX_VARS,
        });
    }
    if witness.instances() != prepared.instances()
        || witness.outputs().len() != prepared.instances()
        || hint_f.rows() != witness.source_rows()
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    validate_ligerito_commitment(&hint_f.commitment, pc).map_err(Sha256F2zError::F2z)?;
    if hint_f.commitment.params.m != f_layout.row_vars + f_layout.col_vars {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = tracing::info_span!("sha256:statement_bind_prover").entered();
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

    prepared.ligerito_configuration()?.bind(transcript);
    let ood = crate::ligerito_flock::bind_prover_ood(transcript, hint_f, prepared.security().ood);

    // Commit first, then derive the one runtime prime. The exact signed
    // relation stays q-independent; only its one local-row collapse is
    // performed in the sampled field.
    let step2_scope = tracing::info_span!("step2:project_prove").entered();
    let initial_nonce = {
        let _scope = tracing::info_span!("sha256:initial_grinding_prove").entered();
        grind_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
        )?
    };
    let mod_q = {
        let _scope = tracing::info_span!("sha256:runtime_prime_sample_prover").entered();
        sample_sha256_mod_q_context(transcript, profile)?
    };
    validate_common_geometry(None, map, h_layout, f_layout)?;
    absorb_runtime_sha256_relation(transcript, prepared, mod_q.field_config());
    drop(step2_scope);

    // Collapse the local rows and form the product-structured SHA residual.
    // Power-of-two batches open it directly; partial batches use the legacy
    // assignment-domain sumcheck below. There is no nonlinear outer sumcheck.
    let step3_scope = tracing::info_span!("step3:piop_prove").entered();
    let field_config = mod_q.field_config();
    let local_row_point = squeeze_challenge_point(
        transcript,
        SHA256_LOCAL_ROW_POINT_DOMAIN,
        local_constraint_vars(),
        field_config,
    );
    let instance_point = squeeze_challenge_point(
        transcript,
        SHA256_INSTANCE_POINT_DOMAIN,
        instance_vars(prepared.instances())?,
        field_config,
    );
    // The public and shared-one equalities are folded into the same local
    // coefficient vector. In the product layout its coefficient is already
    // rank one across F2Z rows and columns.
    let terminal_nonce = {
        let _scope = tracing::info_span!("sha256:public_batch_grinding_prove").entered();
        grind_boundary::<Sha256PublicBatchGrinding, _>(
            transcript,
            profile.terminal_grinding_bits() as u32,
        )?
    };
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let reducer = {
        let _scope = tracing::info_span!("sha256:reducer_init_prover").entered();
        OptimizedSumcheckReducer::new(field_config).map_err(SpartanError::from)?
    };
    let local_row_weights = eq_table(&local_row_point, field_config).map_err(SpartanError::from)?;
    let beta = {
        let _scope = tracing::info_span!("sha256:local_relation_collapse_prover").entered();
        collapse_local_linear_columns(prepared, &local_row_weights, &reducer, field_config)
            .map_err(SpartanError::from)?
    };
    let product_batching = {
        let _scope = tracing::info_span!("sha256:product_batch_prepare_prover").entered();
        ProductLinearBatching::new(
            prepared,
            public_statement,
            &instance_point,
            beta,
            public_slot_weights.clone(),
            public_io_batch.clone(),
            constant_one_batch.clone(),
            field_config,
        )?
    };
    let (inner_proof, inner_nonces, f2z) = if product_layout {
        drop(step3_scope);
        let product_p_h = prepared
            .product_assignment_params()
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        let product_map = prepared
            .product_map()
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        let product_rows = witness
            .product_assignment_rows()
            .ok_or(Sha256F2zError::InvalidGeometry)?;

        let step4_scope = tracing::info_span!("step4:bitify_prove").entered();
        let (row_weights, col_weights_q, claimed_q) = {
            let _scope = tracing::info_span!("sha256:direct_opening_prepare_prover").entered();
            product_opening_claim(
                &product_batching,
                product_p_h,
                product_map.order(),
                field_config,
            )?
        };
        let row_weight_source =
            GeneratedModQWeightSource::new(product_p_h, mod_q.q_bits(), |row| {
                row_weights.canonical_weight(row, field_config)
            })
            .map_err(|()| Sha256F2zError::InvalidGeometry)?;
        {
            let _scope = tracing::info_span!("sha256:opening_claim_absorb_prover").entered();
            absorb_product_opening_claim(
                transcript,
                &assignment_binding,
                &local_row_point,
                &instance_point,
                &constant_one_batch,
                &public_slot_weights,
                &public_io_batch,
                &row_weight_source,
                &col_weights_q,
                claimed_q,
            )?;
        }
        drop(step4_scope);

        let f2z = {
            let _step5 = tracing::info_span!("step5:open_prove").entered();
            let _scope = tracing::info_span!("sha256:f2z_prove").entered();
            prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
                transcript,
                hint_f,
                product_rows,
                product_p_h,
                f_layout,
                product_map,
                &row_weight_source,
                mod_q.q(),
                mod_q.q_bits(),
                f2z_generator(),
                prepared.security().forest_round_grinding_bits,
                ood,
                pc,
            )
            .map_err(Sha256F2zError::F2z)?
        };
        (
            SumcheckProof {
                round_polynomials: Vec::new(),
            },
            Vec::new(),
            f2z,
        )
    } else {
        let inner = {
            let _scope = tracing::info_span!("sha256:spartan_inner_prove").entered();
            let factored_matrix_mle = product_batching.factored_matrix_mle(field_config)?;
            let h_bit =
                |flat_column| packed_flat_bit(witness.assignment_rows(), h_layout, flat_column);
            prove_sha256_inner_sumcheck_factored(
                transcript,
                product_batching.initial_claim().clone(),
                h_layout.row_vars + h_layout.col_vars,
                &factored_matrix_mle,
                &h_bit,
                prefix_vars,
                field_config,
                &reducer,
                profile.inner_round_grinding_bits() as u32,
            )
            .map_err(SpartanError::from)?
        };
        if inner.final_claim != inner.v_evaluation.clone() * &inner.h_evaluation {
            return Err(Sha256F2zError::InvalidInnerTerminalClaim);
        }
        drop(step3_scope);

        let step4_scope = tracing::info_span!("step4:bitify_prove").entered();
        let assignment_equality =
            FactoredEqualityWeights::new(&inner.eval_points, h_layout.row_vars, field_config)?;
        let (row_weights, col_weights_q, claimed_q) = {
            let _scope = tracing::info_span!("sha256:opening_prepare_prover").entered();
            linear_opening_claim(
                prepared,
                h_layout,
                assignment_equality,
                &inner.v_evaluation,
                inner.final_claim.clone(),
            )?
        };
        let row_weight_source = GeneratedModQWeightSource::new(h_layout, mod_q.q_bits(), |row| {
            row_weights
                .get(row)
                .map(|weight| field_from_raw(*weight, field_config).canonical_u128())
        })
        .map_err(|()| Sha256F2zError::InvalidGeometry)?;
        {
            let _scope = tracing::info_span!("sha256:opening_claim_absorb_prover").entered();
            absorb_opening_claim(
                transcript,
                &assignment_binding,
                &local_row_point,
                &instance_point,
                &inner.eval_points,
                &inner.v_evaluation,
                &constant_one_batch,
                &public_slot_weights,
                &public_io_batch,
                &row_weight_source,
                &col_weights_q,
                claimed_q,
            )?;
        }
        drop(step4_scope);

        let f2z = {
            let _step5 = tracing::info_span!("step5:open_prove").entered();
            let _scope = tracing::info_span!("sha256:f2z_prove").entered();
            prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
                transcript,
                hint_f,
                witness.assignment_rows(),
                h_layout,
                f_layout,
                map,
                &row_weight_source,
                mod_q.q(),
                mod_q.q_bits(),
                f2z_generator(),
                prepared.security().forest_round_grinding_bits,
                ood,
                pc,
            )
            .map_err(Sha256F2zError::F2z)?
        };
        (inner.sumcheck_proof, inner.round_nonces, f2z)
    };

    Ok(Sha256CompressionProof {
        initial_nonce,
        inner: inner_proof,
        inner_nonces,
        terminal_nonce,
        f2z,
    })
}

/// Proves with the default packed-prefix kernel and an explicit Ligerito
/// configuration.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_with_config<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &Sha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    prove_sha256_compressions_with_prefix_vars_and_config(
        transcript,
        prepared,
        public_statement,
        witness,
        hint_f,
        SHA256_DEFAULT_INNER_PREFIX_VARS,
        pc,
    )
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

/// Proves with an explicit prover-local packed-prefix width and the prepared
/// batch's derived Ligerito configuration.
pub fn prove_sha256_compressions_with_prefix_vars<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256CompressionBatch,
    public_statement: &[Sha256CompressionStatement],
    witness: &Sha256CompressionWitnessBatch,
    hint_f: &FlockCommitHint,
    prefix_vars: usize,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(prepared)?;
    prove_sha256_compressions_with_prefix_vars_and_config(
        transcript,
        prepared,
        public_statement,
        witness,
        hint_f,
        prefix_vars,
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
    let f_layout = prepared.source_params();
    let h_layout = prepared.assignment_params();
    let map = prepared.map();
    let profile = prepared.prime_profile();

    validate_public_statement(prepared.instances(), public_statement)?;
    validate_common_geometry(None, map, h_layout, f_layout)?;
    let product_layout = match (prepared.product_map(), prepared.product_assignment_params()) {
        (Some(product_map), Some(product_p_h)) => {
            validate_product_geometry(product_map, product_p_h, f_layout)?;
            true
        }
        (None, None) => false,
        _ => return Err(Sha256F2zError::InvalidGeometry),
    };
    validate_ligerito_config_for(prepared, vc)?;
    validate_ligerito_commitment(commitment_f, vc).map_err(Sha256F2zError::F2z)?;
    let expected_inner_rounds = if product_layout {
        0
    } else {
        h_layout.row_vars + h_layout.col_vars
    };
    if commitment_f.params.m != f_layout.row_vars + f_layout.col_vars
        || proof.inner.round_polynomials.len() != expected_inner_rounds
        || (product_layout && !proof.inner_nonces.is_empty())
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let assignment_binding = {
        let _scope = tracing::info_span!("sha256:statement_bind_verifier").entered();
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
    prepared.ligerito_configuration()?.bind(transcript);
    let ood = crate::ligerito_flock::bind_verifier_ood(
        transcript,
        packed_vars(f_layout),
        prepared.security().ood,
        proof.f2z.ood.as_ref(),
    )
    .map_err(Sha256F2zError::F2z)?;

    let step2_scope = tracing::info_span!("step2:project_verify").entered();
    {
        let _scope = tracing::info_span!("sha256:initial_grinding_verify").entered();
        check_boundary::<Sha256InitialGrinding, _>(
            transcript,
            profile.initial_grinding_bits() as u32,
            proof.initial_nonce,
        )?;
    }
    let mod_q = {
        let _scope = tracing::info_span!("sha256:runtime_prime_sample_verifier").entered();
        sample_sha256_mod_q_context(transcript, profile)?
    };
    validate_common_geometry(None, map, h_layout, f_layout)?;
    absorb_runtime_sha256_relation(transcript, prepared, mod_q.field_config());
    drop(step2_scope);

    let step3_scope = tracing::info_span!("step3:piop_verify").entered();
    let field_config = mod_q.field_config();
    let local_row_point = squeeze_challenge_point(
        transcript,
        SHA256_LOCAL_ROW_POINT_DOMAIN,
        local_constraint_vars(),
        field_config,
    );
    let instance_point = squeeze_challenge_point(
        transcript,
        SHA256_INSTANCE_POINT_DOMAIN,
        instance_vars(prepared.instances())?,
        field_config,
    );
    {
        let _scope = tracing::info_span!("sha256:public_batch_grinding_verify").entered();
        check_boundary::<Sha256PublicBatchGrinding, _>(
            transcript,
            profile.terminal_grinding_bits() as u32,
            proof.terminal_nonce,
        )?;
    }
    let constant_one_batch = squeeze_constant_one_batch(transcript, field_config);
    let (public_slot_weights, public_io_batch) = squeeze_public_io_batch(transcript, field_config);
    let reducer = {
        let _scope = tracing::info_span!("sha256:reducer_init_verifier").entered();
        OptimizedSumcheckReducer::new(field_config).map_err(SpartanError::from)?
    };
    let local_row_weights = eq_table(&local_row_point, field_config).map_err(SpartanError::from)?;
    let beta = {
        let _scope = tracing::info_span!("sha256:local_relation_collapse_verifier").entered();
        collapse_local_linear_columns(prepared, &local_row_weights, &reducer, field_config)
            .map_err(SpartanError::from)?
    };
    let product_batching = {
        let _scope = tracing::info_span!("sha256:product_batch_prepare_verifier").entered();
        ProductLinearBatching::new(
            prepared,
            public_statement,
            &instance_point,
            beta,
            public_slot_weights.clone(),
            public_io_batch.clone(),
            constant_one_batch.clone(),
            field_config,
        )?
    };
    if product_layout {
        drop(step3_scope);
        let product_p_h = prepared
            .product_assignment_params()
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        let product_map = prepared
            .product_map()
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        let step4_scope = tracing::info_span!("step4:bitify_verify").entered();
        let (row_weights, col_weights_q, claimed_q) = {
            let _scope = tracing::info_span!("sha256:direct_opening_prepare_verifier").entered();
            product_opening_claim(
                &product_batching,
                product_p_h,
                product_map.order(),
                field_config,
            )?
        };
        let row_weight_source =
            GeneratedModQWeightSource::new(product_p_h, mod_q.q_bits(), |row| {
                row_weights.canonical_weight(row, field_config)
            })
            .map_err(|()| Sha256F2zError::InvalidGeometry)?;
        {
            let _scope = tracing::info_span!("sha256:opening_claim_absorb_verifier").entered();
            absorb_product_opening_claim(
                transcript,
                &assignment_binding,
                &local_row_point,
                &instance_point,
                &constant_one_batch,
                &public_slot_weights,
                &public_io_batch,
                &row_weight_source,
                &col_weights_q,
                claimed_q,
            )?;
        }
        drop(step4_scope);

        let _step5 = tracing::info_span!("step5:open_verify").entered();
        let _scope = tracing::info_span!("sha256:f2z_verify").entered();
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            transcript,
            commitment_f,
            &proof.f2z,
            product_p_h,
            f_layout,
            product_map,
            &row_weight_source,
            &col_weights_q,
            f2z_generator(),
            claimed_q,
            mod_q.q(),
            mod_q.q_bits(),
            prepared.security().forest_round_grinding_bits,
            ood,
            vc,
        )
        .map_err(Sha256F2zError::F2z)
    } else {
        let (assignment_point, inner_claim) = {
            let _scope = tracing::info_span!("sha256:spartan_inner_verify").entered();
            verify_sha256_inner_sumcheck(
                transcript,
                product_batching.initial_claim().clone(),
                &proof.inner,
                &proof.inner_nonces,
                h_layout.row_vars + h_layout.col_vars,
                field_config,
                profile.inner_round_grinding_bits() as u32,
            )
            .map_err(SpartanError::from)?
        };
        drop(step3_scope);

        let step4_scope = tracing::info_span!("step4:bitify_verify").entered();
        let collapsed_evaluation = product_batching.evaluate(&assignment_point, field_config)?;
        let assignment_equality =
            FactoredEqualityWeights::new(&assignment_point, h_layout.row_vars, field_config)?;
        let (row_weights, col_weights_q, claimed_q) = {
            let _scope = tracing::info_span!("sha256:opening_prepare_verifier").entered();
            linear_opening_claim(
                prepared,
                h_layout,
                assignment_equality,
                &collapsed_evaluation,
                inner_claim,
            )?
        };
        let row_weight_source = GeneratedModQWeightSource::new(h_layout, mod_q.q_bits(), |row| {
            row_weights
                .get(row)
                .map(|weight| field_from_raw(*weight, field_config).canonical_u128())
        })
        .map_err(|()| Sha256F2zError::InvalidGeometry)?;
        {
            let _scope = tracing::info_span!("sha256:opening_claim_absorb_verifier").entered();
            absorb_opening_claim(
                transcript,
                &assignment_binding,
                &local_row_point,
                &instance_point,
                &assignment_point,
                &collapsed_evaluation,
                &constant_one_batch,
                &public_slot_weights,
                &public_io_batch,
                &row_weight_source,
                &col_weights_q,
                claimed_q,
            )?;
        }
        drop(step4_scope);

        let _step5 = tracing::info_span!("step5:open_verify").entered();
        let _scope = tracing::info_span!("sha256:f2z_verify").entered();
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            transcript,
            commitment_f,
            &proof.f2z,
            h_layout,
            f_layout,
            map,
            &row_weight_source,
            &col_weights_q,
            f2z_generator(),
            claimed_q,
            mod_q.q(),
            mod_q.q_bits(),
            prepared.security().forest_round_grinding_bits,
            ood,
            vc,
        )
        .map_err(Sha256F2zError::F2z)
    }
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

/// Random linear combination of the SHA rows, shared-one residual, and public
/// SHA cells folded into one product-structured oracle.
///
/// Let `v_r = eq(r, ξ)`, `u_i = eq(i, η)`, `U = Σ_{i<N} u_i`, and
/// `β = Cᵀv`. The local vector `d` adds the shared-one and public-I/O
/// coefficients to `β`. Its gap-free physical realization is
///
/// - `V[0] = U d[0]`, and
/// - `V[1 + i(L - 1) + (c - 1)] = u_i d[c]` for `c > 0`.
///
/// Thus the native matrix is collapsed once, independently of the number of
/// instances. The opening claim `μ` is the same product-structured batching
/// of the public values: `α₀ U + α_pub Σ_i u_i public_i`.
struct ProductLinearBatching {
    instances: usize,
    instance_point: Vec<SpartanF2zField>,
    instance_weights: Vec<SpartanF2zField>,
    local_coefficients: Vec<SpartanF2zField>,
    shared_coefficient: SpartanF2zField,
    initial_claim: SpartanF2zField,
}

impl ProductLinearBatching {
    #[allow(clippy::too_many_arguments)]
    fn new(
        prepared: &PreparedSha256CompressionBatch,
        public_statement: &[Sha256CompressionStatement],
        instance_point: &[SpartanF2zField],
        beta: Vec<SpartanF2zField>,
        slot_weights: Vec<SpartanF2zField>,
        public_batch_weight: SpartanF2zField,
        constant_weight: SpartanF2zField,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<Self, Sha256F2zError> {
        validate_public_statement(prepared.instances(), public_statement)?;
        if instance_point.len() != instance_vars(prepared.instances())?
            || beta.len() != SHA256_H_BAR_LIVE_BITS
            || slot_weights.len() != SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS
            || public_batch_weight.cfg() != field_config
            || constant_weight.cfg() != field_config
            || instance_point
                .iter()
                .chain(&beta)
                .chain(&slot_weights)
                .any(|value| value.cfg() != field_config)
        {
            return Err(Sha256F2zError::InvalidGeometry);
        }

        let instance_weights = eq_table(instance_point, field_config)
            .map_err(SpartanError::from)?
            .into_iter()
            .take(prepared.instances())
            .collect::<Vec<_>>();
        let mut active_instance_sum = SpartanF2zField::zero_with_cfg(field_config);
        for weight in &instance_weights {
            active_instance_sum += weight;
        }

        let mut local_coefficients = beta;
        local_coefficients[SHA256_SHARED_CONSTANT_CELL] += &constant_weight;
        for (slot, slot_weight) in slot_weights.iter().enumerate() {
            let word_slot = slot / SHA256_PUBLIC_WORD_BITS;
            let bit = slot % SHA256_PUBLIC_WORD_BITS;
            let local_column = sha256_public_h_column(word_slot, bit);
            let public_coefficient = public_batch_weight.clone() * slot_weight;
            *local_coefficients
                .get_mut(local_column)
                .ok_or(Sha256F2zError::InvalidGeometry)? += &public_coefficient;
        }
        let shared_coefficient =
            active_instance_sum.clone() * &local_coefficients[SHA256_SHARED_CONSTANT_CELL];

        // Compress each eight-bit public dot product into a lookup table once.
        // This computes μ without visiting all 1,024 public bits per instance.
        let byte_tables = weighted_byte_tables(&slot_weights, field_config);
        let mut initial_claim = constant_weight * &active_instance_sum;
        for (instance, statement) in public_statement.iter().enumerate() {
            let mut statement_value = SpartanF2zField::zero_with_cfg(field_config);
            for (word_slot, word) in statement.words().enumerate() {
                for byte in 0..4 {
                    let value = ((word >> (8 * byte)) & 0xff) as usize;
                    statement_value += &byte_tables[4 * word_slot + byte][value];
                }
            }
            let mut coefficient = instance_weights[instance].clone() * &public_batch_weight;
            coefficient *= &statement_value;
            initial_claim += &coefficient;
        }

        Ok(Self {
            instances: prepared.instances(),
            instance_point: instance_point.to_vec(),
            instance_weights,
            local_coefficients,
            shared_coefficient,
            initial_claim,
        })
    }

    const fn initial_claim(&self) -> &SpartanF2zField {
        &self.initial_claim
    }

    fn factored_matrix_mle(
        &self,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<FactoredMultilinearExtension<'_, SpartanF2zField>, Sha256F2zError> {
        FactoredMultilinearExtension::with_leading_value(
            (1 + self.instance_weights.len() * (self.local_coefficients.len() - 1))
                .next_power_of_two()
                .ilog2() as usize,
            self.shared_coefficient.clone(),
            &self.instance_weights,
            &self.local_coefficients[1..],
            field_config,
        )
        .map_err(|_| {
            SpartanError::from(
                crate::piop::spartan::sumcheck::SumcheckError::InvalidProductDimensions,
            )
        })
        .map_err(Sha256F2zError::from)
    }

    #[cfg(test)]
    fn coefficient(
        &self,
        flat_column: usize,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, SumcheckError> {
        if flat_column == SHA256_SHARED_CONSTANT_CELL {
            return Ok(self.shared_coefficient.clone());
        }
        let Some(offset) = flat_column.checked_sub(1) else {
            return Ok(SpartanF2zField::zero_with_cfg(field_config));
        };
        let instance = offset / super::constraints::SHA256_H_INSTANCE_BITS;
        if instance >= self.instances {
            return Ok(SpartanF2zField::zero_with_cfg(field_config));
        }
        let local_column = 1 + offset % super::constraints::SHA256_H_INSTANCE_BITS;
        let coefficient = self
            .local_coefficients
            .get(local_column)
            .ok_or(SumcheckError::InvalidProductDimensions)?;
        Ok(self.instance_weights[instance].clone() * coefficient)
    }

    fn evaluate(
        &self,
        assignment_point: &[SpartanF2zField],
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, Sha256F2zError> {
        let constant_equality = equality_at_zero(assignment_point, field_config)?;
        let zero = SpartanF2zField::zero_with_cfg(field_config);
        let repeated_nonconstant = evaluate_affine_equality_repetition(
            self.instances,
            0,
            super::constraints::SHA256_H_INSTANCE_BITS,
            &[],
            assignment_point,
            Some(&self.instance_point),
            self.local_coefficients
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, coefficient)| **coefficient != zero)
                .map(|(column, coefficient)| (0, column, coefficient.clone())),
            field_config,
        )?;
        Ok(self.shared_coefficient.clone() * &constant_equality + &repeated_nonconstant)
    }

    #[cfg(test)]
    fn evaluate_dense(
        &self,
        assignment_equality: &FactoredEqualityWeights,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, Sha256F2zError> {
        let mut evaluation = self.shared_coefficient.clone()
            * &assignment_equality
                .evaluate(SHA256_SHARED_CONSTANT_CELL, field_config)
                .ok_or(Sha256F2zError::InvalidGeometry)?;
        for instance in 0..self.instances {
            for local_column in 1..self.local_coefficients.len() {
                let flat_column =
                    1 + instance * super::constraints::SHA256_H_INSTANCE_BITS + local_column - 1;
                let equality = assignment_equality
                    .evaluate(flat_column, field_config)
                    .ok_or(Sha256F2zError::InvalidGeometry)?;
                let coefficient = self.instance_weights[instance].clone()
                    * &self.local_coefficients[local_column];
                evaluation += &(coefficient * &equality);
            }
        }
        Ok(evaluation)
    }
}

/// Legacy public-only batching retained for dense equivalence tests.
#[cfg(test)]
struct PublicLinearBatching {
    instances: usize,
    constant_weight: SpartanF2zField,
    public_batch_weight: SpartanF2zField,
    instance_point: Vec<SpartanF2zField>,
    scaled_instance_weights: Vec<SpartanF2zField>,
    slot_weights: Vec<SpartanF2zField>,
    initial_claim: SpartanF2zField,
}

#[cfg(test)]
impl PublicLinearBatching {
    #[allow(clippy::too_many_arguments)]
    fn new(
        prepared: &PreparedSha256CompressionBatch,
        public_statement: &[Sha256CompressionStatement],
        public_instance_point: &[SpartanF2zField],
        slot_weights: Vec<SpartanF2zField>,
        public_batch_weight: SpartanF2zField,
        constant_weight: SpartanF2zField,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<Self, Sha256F2zError> {
        validate_public_statement(prepared.instances(), public_statement)?;
        if public_instance_point.len() != instance_vars(prepared.instances())?
            || slot_weights.len() != SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS
            || public_batch_weight.cfg() != field_config
            || constant_weight.cfg() != field_config
            || slot_weights
                .iter()
                .any(|weight| weight.cfg() != field_config)
        {
            return Err(Sha256F2zError::InvalidGeometry);
        }
        let instance_weights =
            eq_table(public_instance_point, field_config).map_err(SpartanError::from)?;
        let scaled_instance_weights = instance_weights
            .into_iter()
            .take(prepared.instances())
            .map(|weight| weight * &public_batch_weight)
            .collect::<Vec<_>>();

        // A statement has 1,024 public bits.  Compress each eight-bit dot
        // product into a 256-entry lookup table once, then evaluate each of
        // the 32 public words with four lookups.  This preserves the exact
        // field-valued bit dot product while replacing 1,024 per-instance
        // bit tests and up to 1,024 field multiplications with 128 lookups,
        // additions, and one multiplication.
        let byte_tables = weighted_byte_tables(&slot_weights, field_config);
        let mut initial_claim = constant_weight.clone();
        for (instance, statement) in public_statement.iter().enumerate() {
            let mut statement_value = SpartanF2zField::zero_with_cfg(field_config);
            for (word_slot, word) in statement.words().enumerate() {
                for byte in 0..4 {
                    let value = ((word >> (8 * byte)) & 0xff) as usize;
                    statement_value += &byte_tables[4 * word_slot + byte][value];
                }
            }
            initial_claim += &(scaled_instance_weights[instance].clone() * &statement_value);
        }

        Ok(Self {
            instances: prepared.instances(),
            constant_weight,
            public_batch_weight,
            instance_point: public_instance_point.to_vec(),
            scaled_instance_weights,
            slot_weights,
            initial_claim,
        })
    }

    const fn initial_claim(&self) -> &SpartanF2zField {
        &self.initial_claim
    }

    fn coefficient(
        &self,
        flat_column: usize,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, SumcheckError> {
        if flat_column == SHA256_SHARED_CONSTANT_CELL {
            return Ok(self.constant_weight.clone());
        }
        let Some(packed_offset) = flat_column.checked_sub(1) else {
            return Ok(SpartanF2zField::zero_with_cfg(field_config));
        };
        let instance = packed_offset / super::constraints::SHA256_H_INSTANCE_BITS;
        if instance >= self.instances {
            return Ok(SpartanF2zField::zero_with_cfg(field_config));
        }
        let local_column = 1 + packed_offset % super::constraints::SHA256_H_INSTANCE_BITS;
        let Some(slot) = public_bit_index_for_h_column(local_column) else {
            return Ok(SpartanF2zField::zero_with_cfg(field_config));
        };
        Ok(self.scaled_instance_weights[instance].clone() * &self.slot_weights[slot])
    }

    fn evaluate(
        &self,
        assignment_point: &[SpartanF2zField],
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, Sha256F2zError> {
        let constant_equality = equality_at_zero(assignment_point, field_config)?;
        let repeated_public = evaluate_affine_equality_repetition(
            self.instances,
            0,
            super::constraints::SHA256_H_INSTANCE_BITS,
            &[],
            assignment_point,
            Some(&self.instance_point),
            self.slot_weights
                .iter()
                .enumerate()
                .map(|(slot, coefficient)| {
                    let word_slot = slot / SHA256_PUBLIC_WORD_BITS;
                    let bit = slot % SHA256_PUBLIC_WORD_BITS;
                    (
                        0,
                        sha256_public_h_column(word_slot, bit),
                        coefficient.clone(),
                    )
                }),
            field_config,
        )?;
        let mut evaluation = self.constant_weight.clone() * &constant_equality;
        evaluation += &(self.public_batch_weight.clone() * &repeated_public);
        Ok(evaluation)
    }

    #[cfg(test)]
    fn evaluate_dense(
        &self,
        assignment_equality: &FactoredEqualityWeights,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<SpartanF2zField, Sha256F2zError> {
        let mut evaluation = self.constant_weight.clone()
            * &assignment_equality
                .evaluate(SHA256_SHARED_CONSTANT_CELL, field_config)
                .ok_or(Sha256F2zError::InvalidGeometry)?;
        for instance in 0..self.instances {
            for word_slot in 0..SHA256_PUBLIC_WORDS {
                for bit in 0..SHA256_PUBLIC_WORD_BITS {
                    let slot = word_slot * SHA256_PUBLIC_WORD_BITS + bit;
                    let local_column = sha256_public_h_column(word_slot, bit);
                    let flat_column =
                        1 + instance * super::constraints::SHA256_H_INSTANCE_BITS + local_column
                            - 1;
                    let equality = assignment_equality
                        .evaluate(flat_column, field_config)
                        .ok_or(Sha256F2zError::InvalidGeometry)?;
                    let coefficient =
                        self.scaled_instance_weights[instance].clone() * &self.slot_weights[slot];
                    evaluation += &(coefficient * &equality);
                }
            }
        }
        Ok(evaluation)
    }
}

pub(super) fn weighted_byte_tables(
    weights: &[SpartanF2zField],
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Vec<Vec<SpartanF2zField>> {
    debug_assert_eq!(weights.len() % 8, 0);
    weights
        .chunks_exact(8)
        .map(|byte_weights| {
            let mut table = Vec::with_capacity(256);
            table.push(SpartanF2zField::zero_with_cfg(field_config));
            for value in 1usize..256 {
                let bit = value.trailing_zeros() as usize;
                let previous = value & (value - 1);
                table.push(table[previous].clone() + &byte_weights[bit]);
            }
            table
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn linear_opening_claim(
    prepared: &PreparedSha256CompressionBatch,
    h_layout: &IntegerMatrixLayout,
    mut assignment_equality: FactoredEqualityWeights,
    collapsed_evaluation: &SpartanF2zField,
    inner_claim: SpartanF2zField,
) -> Result<(Vec<RawMontgomery>, Vec<u128>, u128), Sha256F2zError> {
    let field_config = collapsed_evaluation.cfg();
    if assignment_equality.len() != h_layout.cells()
        || assignment_equality.low_vars != h_layout.row_vars
        || assignment_equality.low.len() != h_layout.rows()
        || assignment_equality.high.len() != h_layout.cols()
        || prepared.linear_assignment_column_count() > h_layout.cells()
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    assignment_equality.scale(collapsed_evaluation, field_config);
    let row_weights = assignment_equality.low;
    let col_weights = assignment_equality
        .high
        .iter()
        .map(|weight| field_from_raw(*weight, field_config).canonical_u128())
        .collect::<Vec<_>>();
    Ok((row_weights, col_weights, inner_claim.canonical_u128()))
}

enum ProductRowWeights {
    InstanceOnly(Vec<RawMontgomery>),
    LocalAndInstance {
        local: Vec<RawMontgomery>,
        low_instance: Vec<RawMontgomery>,
        local_domain: usize,
    },
}

impl ProductRowWeights {
    fn canonical_weight(
        &self,
        row: usize,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Option<u128> {
        match self {
            Self::InstanceOnly(weights) => weights
                .get(row)
                .map(|weight| field_from_raw(*weight, field_config).canonical_u128()),
            Self::LocalAndInstance {
                local,
                low_instance,
                local_domain,
            } => {
                let local_column = row & (local_domain - 1);
                let instance = row / local_domain;
                let local_weight = field_from_raw(*local.get(local_column)?, field_config);
                let instance_weight = field_from_raw(*low_instance.get(instance)?, field_config);
                Some((local_weight * &instance_weight).canonical_u128())
            }
        }
    }
}

/// Builds the direct rank-one F2Z claim without materializing `u ⊗ d`.
fn product_opening_claim(
    batching: &ProductLinearBatching,
    h_layout: &IntegerMatrixLayout,
    layout: PackedSourceOrder,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<(ProductRowWeights, Vec<u128>, u128), Sha256F2zError> {
    let instance_vars = instance_vars(batching.instances)?;
    if !batching.instances.is_power_of_two()
        || batching.instance_point.len() != instance_vars
        || h_layout.word_bits != 1
        || h_layout.row_vars + h_layout.col_vars != instance_vars + 15
        || batching.local_coefficients.len() != SHA256_H_BAR_LIVE_BITS
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let (row_weights, col_weights) = match layout {
        PackedSourceOrder::LocalMajor => {
            if h_layout.row_vars > instance_vars {
                return Err(Sha256F2zError::InvalidGeometry);
            }
            let low =
                compact_eq_table(&batching.instance_point[..h_layout.row_vars], field_config)?;
            let high =
                compact_eq_table(&batching.instance_point[h_layout.row_vars..], field_config)?;
            if low.len() != h_layout.rows()
                || low.len() * high.len() != batching.instances
                || h_layout.cols() != SHA256_H_BAR_LIVE_BITS.next_power_of_two() * high.len()
            {
                return Err(Sha256F2zError::InvalidGeometry);
            }

            let high_instances = high.len();
            let coefficient_at = |column: usize| {
                let local_column = column / high_instances;
                if local_column >= batching.local_coefficients.len() {
                    return 0;
                }
                let high_instance = column % high_instances;
                let high_weight = field_from_raw(high[high_instance], field_config);
                (batching.local_coefficients[local_column].clone() * &high_weight).canonical_u128()
            };
            #[cfg(feature = "parallel")]
            let columns = (0..h_layout.cols())
                .into_par_iter()
                .map(coefficient_at)
                .collect::<Vec<_>>();
            #[cfg(not(feature = "parallel"))]
            let columns = (0..h_layout.cols()).map(coefficient_at).collect::<Vec<_>>();
            (ProductRowWeights::InstanceOnly(low), columns)
        }
        PackedSourceOrder::InstanceMajor => {
            let local_domain = SHA256_H_BAR_LIVE_BITS.next_power_of_two();
            let local_vars = local_domain.ilog2() as usize;
            if h_layout.row_vars < local_vars || h_layout.row_vars > local_vars + instance_vars {
                return Err(Sha256F2zError::InvalidGeometry);
            }
            let low_instance_vars = h_layout.row_vars - local_vars;
            let low_instance =
                compact_eq_table(&batching.instance_point[..low_instance_vars], field_config)?;
            let high =
                compact_eq_table(&batching.instance_point[low_instance_vars..], field_config)?;
            if local_domain * low_instance.len() != h_layout.rows()
                || low_instance.len() * high.len() != batching.instances
                || high.len() != h_layout.cols()
            {
                return Err(Sha256F2zError::InvalidGeometry);
            }
            let local = batching
                .local_coefficients
                .iter()
                .map(raw_montgomery)
                .chain(std::iter::repeat_n(
                    raw_montgomery(&SpartanF2zField::zero_with_cfg(field_config)),
                    local_domain - batching.local_coefficients.len(),
                ))
                .collect();
            let columns = high
                .into_iter()
                .map(|weight| field_from_raw(weight, field_config).canonical_u128())
                .collect();
            (
                ProductRowWeights::LocalAndInstance {
                    local,
                    low_instance,
                    local_domain,
                },
                columns,
            )
        }
    };

    Ok((
        row_weights,
        col_weights,
        batching.initial_claim.canonical_u128(),
    ))
}

#[cfg(test)]
fn flat_constraint_vars(
    prepared: &PreparedSha256CompressionBatch,
) -> Result<usize, Sha256F2zError> {
    let live_rows = prepared.linear_row_count();
    let domain = live_rows
        .checked_next_power_of_two()
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    Ok(domain.ilog2() as usize)
}

pub(super) const fn local_constraint_vars() -> usize {
    SHA256_CONSTRAINTS.next_power_of_two().ilog2() as usize
}

pub(super) fn instance_vars(instances: usize) -> Result<usize, Sha256F2zError> {
    let domain = instances
        .checked_next_power_of_two()
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    Ok(domain.ilog2() as usize)
}

pub(super) fn squeeze_challenge_point(
    transcript: &mut impl Transcript,
    domain: &[u8],
    vars: usize,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Vec<SpartanF2zField> {
    absorb_spartan_message(transcript, b"challenge-domain", domain);
    (0..vars)
        .map(|_| squeeze_field(transcript, field_config))
        .collect()
}

#[cfg(test)]
fn public_instance_point(
    constraint_point: &[SpartanF2zField],
    instances: usize,
) -> Result<&[SpartanF2zField], Sha256F2zError> {
    let vars = instance_vars(instances)?;
    let start = constraint_point
        .len()
        .checked_sub(vars)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    constraint_point
        .get(start..)
        .ok_or(Sha256F2zError::InvalidGeometry)
}

/// Computes `β = Cᵀ eq(·, ξ)` directly from the retained signed CSC
/// relation. The local row domain has 256 entries, of which exactly the first
/// 184 are live; the zero suffix is represented only by absent matrix rows.
fn collapse_local_linear_columns(
    prepared: &PreparedSha256CompressionBatch,
    local_row_weights: &[SpartanF2zField],
    reducer: &OptimizedSumcheckReducer,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<Vec<SpartanF2zField>, SumcheckError> {
    let relation = prepared.linear_relation().native_matrix();
    if relation.column_count() != SHA256_H_BAR_LIVE_BITS {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    collapse_native_linear_columns(relation, local_row_weights, reducer, field_config)
}

/// [`collapse_local_linear_columns`] over any native signed local relation
/// with [`SHA256_CONSTRAINTS`] live rows: `β_c = Σ_r eq(r, ξ) C[r, c]`.
pub(super) fn collapse_native_linear_columns(
    relation: &crate::sparse_matrix::SparseMatrix<i64>,
    local_row_weights: &[SpartanF2zField],
    reducer: &OptimizedSumcheckReducer,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<Vec<SpartanF2zField>, SumcheckError> {
    let expected_rows = 1usize << local_constraint_vars();
    if relation.row_count() != SHA256_CONSTRAINTS
        || local_row_weights.len() != expected_rows
        || local_row_weights
            .iter()
            .any(|weight| weight.cfg() != field_config)
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let zero = SpartanF2zField::zero_with_cfg(field_config);
    let collapse_column = |local_column: usize| {
        let column = relation
            .column(local_column)
            .ok_or(SumcheckError::InvalidProductDimensions)?;
        let mut accumulator =
            <OptimizedSumcheckReducer as SumcheckLinearReducer>::accumulator_zero(reducer);
        for (local_row, coefficient) in column {
            let weight = local_row_weights
                .get(local_row)
                .ok_or(SumcheckError::InvalidProductDimensions)?;
            let negative_weight;
            let selected_weight = if *coefficient < 0 {
                negative_weight = zero.clone() - weight;
                &negative_weight
            } else {
                weight
            };
            <OptimizedSumcheckReducer as SumcheckLinearReducer>::multiply_accumulate(
                reducer,
                &mut accumulator,
                selected_weight,
                &coefficient.unsigned_abs(),
            );
        }
        <OptimizedSumcheckReducer as SumcheckLinearReducer>::reduce(reducer, accumulator)
    };

    #[cfg(feature = "parallel")]
    if relation.column_count() >= 1 << 12 && rayon::current_num_threads() > 1 {
        return (0..relation.column_count())
            .into_par_iter()
            .map(collapse_column)
            .collect();
    }
    (0..relation.column_count()).map(collapse_column).collect()
}

#[cfg(test)]
fn collapse_flat_linear_column(
    prepared: &PreparedSha256CompressionBatch,
    relation: &PreparedSha256LinearRelation,
    constraint_weights: &[RawMontgomery],
    reducer: &OptimizedSumcheckReducer,
    flat_column: usize,
) -> Result<SpartanF2zField, SumcheckError> {
    let zero = SpartanF2zField::zero_with_cfg(relation.config());
    let live_columns = prepared.linear_assignment_column_count();
    if flat_column >= prepared.assignment_params().cells() {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    if flat_column >= live_columns {
        return Ok(zero);
    }

    let (instance_start, instance_end, local_column) = if flat_column == 0 {
        (0, prepared.instances(), 0)
    } else {
        let offset = flat_column - 1;
        let instance = offset / super::constraints::SHA256_H_INSTANCE_BITS;
        let local_column = 1 + offset % super::constraints::SHA256_H_INSTANCE_BITS;
        (instance, instance + 1, local_column)
    };
    let column = prepared
        .linear_relation()
        .native_matrix()
        .column(local_column)
        .ok_or(SumcheckError::InvalidProductDimensions)?;
    let mut accumulator =
        <OptimizedSumcheckReducer as SumcheckLinearReducer>::accumulator_zero(reducer);
    for instance in instance_start..instance_end {
        for (local_row, coefficient) in column {
            let flat_row = instance
                .checked_mul(SHA256_CONSTRAINTS)
                .and_then(|base| base.checked_add(local_row))
                .ok_or(SumcheckError::InvalidProductDimensions)?;
            let weight = constraint_weights
                .get(flat_row)
                .ok_or(SumcheckError::InvalidProductDimensions)?;
            let weight = field_from_raw(*weight, relation.config());
            let negative_weight;
            let selected_weight = if *coefficient < 0 {
                negative_weight = zero.clone() - &weight;
                &negative_weight
            } else {
                &weight
            };
            <OptimizedSumcheckReducer as SumcheckLinearReducer>::multiply_accumulate(
                reducer,
                &mut accumulator,
                selected_weight,
                &coefficient.unsigned_abs(),
            );
        }
    }
    <OptimizedSumcheckReducer as SumcheckLinearReducer>::reduce(reducer, accumulator)
}

/// Evaluates the repeated flat relation without expanding its `instances`
/// copies.  For every local nonzero `(r, c)`, the repeated coordinates are
///
/// `row(i) = 184 i + r`, `column(i) = 20,456 i + c`.
///
/// Reading `i` from least-significant bit to most-significant bit turns both
/// affine indices into bounded binary carry recurrences. Terms that reach the
/// same pair of carries are algebraically identical for all remaining bits and
/// are merged. If `S` is the maximum carry frontier (independent of the number
/// of instances for this fixed local SHA relation), the contraction costs
/// `O((nnz(C) + S) * log(domain))`, rather than materializing or visiting
/// `instances * nnz(C)` repeated entries.
#[cfg(test)]
fn evaluate_repeated_flat_linear_collapse(
    prepared: &PreparedSha256CompressionBatch,
    relation: &PreparedSha256LinearRelation,
    constraint_point: &[SpartanF2zField],
    assignment_point: &[SpartanF2zField],
) -> Result<SpartanF2zField, Sha256F2zError> {
    if constraint_point.len() != flat_constraint_vars(prepared)?
        || assignment_point.len()
            != prepared.assignment_params().row_vars + prepared.assignment_params().col_vars
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let constant_column = relation
        .matrix()
        .column(SHA256_SHARED_CONSTANT_CELL)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    let constant_evaluation = evaluate_affine_equality_repetition(
        prepared.instances(),
        SHA256_CONSTRAINTS,
        0,
        constraint_point,
        assignment_point,
        None,
        constant_column
            .into_iter()
            .map(|(row, coefficient)| (row, 0, coefficient.clone())),
        relation.config(),
    )?;

    let nonconstant_terms =
        relation
            .matrix()
            .columns()
            .enumerate()
            .skip(1)
            .flat_map(|(column, entries)| {
                entries
                    .into_iter()
                    .map(move |(row, coefficient)| (row, column, coefficient.clone()))
            });
    let nonconstant_evaluation = evaluate_affine_equality_repetition(
        prepared.instances(),
        SHA256_CONSTRAINTS,
        super::constraints::SHA256_H_INSTANCE_BITS,
        constraint_point,
        assignment_point,
        None,
        nonconstant_terms,
        relation.config(),
    )?;

    Ok(constant_evaluation + &nonconstant_evaluation)
}

/// Contracts a weighted set of affine-offset pairs against two equality
/// tensors, optionally also weighting the repetition index by a third
/// equality tensor.  The latter is used for public-I/O batching.
#[allow(clippy::too_many_arguments)]
fn evaluate_affine_equality_repetition<I>(
    instances: usize,
    left_stride: usize,
    right_stride: usize,
    left_point: &[SpartanF2zField],
    right_point: &[SpartanF2zField],
    instance_point: Option<&[SpartanF2zField]>,
    terms: I,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<SpartanF2zField, Sha256F2zError>
where
    I: IntoIterator<Item = (usize, usize, SpartanF2zField)>,
{
    if instances == 0
        || left_point
            .iter()
            .chain(right_point)
            .chain(instance_point.into_iter().flatten())
            .any(|value| value.cfg() != field_config)
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let input_vars = instance_vars(instances)?;
    if instance_point.is_some_and(|point| point.len() != input_vars) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let rounds = left_point.len().max(right_point.len());
    if input_vars > rounds {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let input_capacity = 1usize
        .checked_shl(u32::try_from(input_vars).map_err(|_| Sha256F2zError::InvalidGeometry)?)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    let bounded = instances != input_capacity;

    // Each round has eight possible `(left_bit, right_bit, instance_bit)`
    // products.  Precomputing them removes two field multiplications from
    // every carry-state transition.  `None` marks a one bit beyond a tensor's
    // declared domain and therefore an overflowing affine index.
    let one = SpartanF2zField::one_with_cfg(field_config);
    let bit_factors = |point: &[SpartanF2zField], round: usize| {
        point.get(round).map_or_else(
            || [Some(one.clone()), None],
            |challenge| [Some(one.clone() - challenge), Some(challenge.clone())],
        )
    };
    let mut round_weights: Vec<[Option<SpartanF2zField>; 8]> = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let left = bit_factors(left_point, round);
        let right = bit_factors(right_point, round);
        let input = instance_point.map_or_else(
            || [Some(one.clone()), Some(one.clone())],
            |point| bit_factors(point, round),
        );
        round_weights.push(std::array::from_fn(|index| {
            let mut value = left[index & 1].clone()?;
            value *= right[(index >> 1) & 1].as_ref()?;
            value *= input[(index >> 2) & 1].as_ref()?;
            Some(value)
        }));
    }

    // State is `(left carry, right carry, borrow)`.  For a partial batch,
    // `borrow` is the carry of the little-endian subtraction `i - instances`;
    // it is one exactly when the completed instance index satisfies `i < N`.
    let zero = SpartanF2zField::zero_with_cfg(field_config);
    let terms = terms.into_iter();
    let (lower_bound, _) = terms.size_hint();
    let mut states = HashMap::with_capacity(lower_bound);
    for (left_offset, right_offset, coefficient) in terms {
        if coefficient.cfg() != field_config {
            return Err(Sha256F2zError::InvalidGeometry);
        }
        match states.entry((left_offset, right_offset, false)) {
            Entry::Vacant(entry) => {
                if coefficient != zero {
                    entry.insert(coefficient);
                }
            }
            Entry::Occupied(mut entry) => {
                *entry.get_mut() += &coefficient;
                if *entry.get() == zero {
                    entry.remove();
                }
            }
        }
    }

    for (round, weights) in round_weights.iter().enumerate() {
        let input_active = round < input_vars;
        let instance_bit = if input_active {
            (instances >> round) & 1
        } else {
            0
        };
        let mut next = HashMap::with_capacity(states.len().saturating_mul(2));
        for ((left_carry, right_carry, borrow), value) in states {
            for bit in 0..=usize::from(input_active) {
                let left = left_carry
                    .checked_add(left_stride * bit)
                    .ok_or(Sha256F2zError::InvalidGeometry)?;
                let right = right_carry
                    .checked_add(right_stride * bit)
                    .ok_or(Sha256F2zError::InvalidGeometry)?;
                let left_bit = left & 1;
                let right_bit = right & 1;
                let weight_index = left_bit | (right_bit << 1) | (bit << 2);
                let Some(weight) = weights[weight_index].as_ref() else {
                    continue;
                };
                let next_borrow = bounded && bit < instance_bit + usize::from(borrow);
                let term = value.clone() * weight;
                if term == zero {
                    continue;
                }
                match next.entry((left >> 1, right >> 1, next_borrow)) {
                    Entry::Vacant(entry) => {
                        entry.insert(term);
                    }
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() += &term;
                        if *entry.get() == zero {
                            entry.remove();
                        }
                    }
                }
            }
        }
        states = next;
    }

    let mut evaluation = zero;
    for ((left_carry, right_carry, borrow), value) in states {
        if left_carry == 0 && right_carry == 0 && (!bounded || borrow) {
            evaluation += &value;
        }
    }
    Ok(evaluation)
}

fn equality_at_zero(
    point: &[SpartanF2zField],
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<SpartanF2zField, Sha256F2zError> {
    if point.iter().any(|value| value.cfg() != field_config) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let one = SpartanF2zField::one_with_cfg(field_config);
    Ok(point.iter().fold(one.clone(), |product, challenge| {
        product * &(one.clone() - challenge)
    }))
}

#[cfg(test)]
fn evaluate_flat_linear_collapse_dense(
    prepared: &PreparedSha256CompressionBatch,
    relation: &PreparedSha256LinearRelation,
    constraint_weights: &[RawMontgomery],
    assignment_weights: &FactoredEqualityWeights,
) -> Result<SpartanF2zField, Sha256F2zError> {
    if assignment_weights.len() != prepared.assignment_params().cells()
        || constraint_weights.len()
            != 1usize
                .checked_shl(
                    u32::try_from(flat_constraint_vars(prepared)?)
                        .map_err(|_| Sha256F2zError::InvalidGeometry)?,
                )
                .ok_or(Sha256F2zError::InvalidGeometry)?
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }

    let zero = SpartanF2zField::zero_with_cfg(relation.config());
    let mut evaluation = zero.clone();
    for (local_column, column) in relation.matrix().columns().enumerate() {
        if column.is_empty() {
            continue;
        }
        for instance in 0..prepared.instances() {
            let flat_column = prepared
                .flat_assignment_column(instance, local_column)
                .ok_or(Sha256F2zError::InvalidGeometry)?;
            let mut column_value = zero.clone();
            for (local_row, coefficient) in column {
                let flat_row = prepared
                    .flat_constraint_row(instance, local_row)
                    .ok_or(Sha256F2zError::InvalidGeometry)?;
                let constraint_weight =
                    field_from_raw(constraint_weights[flat_row], relation.config());
                column_value += &(constraint_weight * coefficient);
            }
            let assignment_weight = assignment_weights
                .evaluate(flat_column, relation.config())
                .ok_or(Sha256F2zError::InvalidGeometry)?;
            evaluation += &(column_value * &assignment_weight);
        }
    }
    Ok(evaluation)
}

/// Tensor-factorized little-endian equality table. The low `t` coordinates
/// select F2Z's packed row and the high `s` coordinates select its column, so
/// the two vectors can be passed to the integer opening without constructing
/// the full `2^(t+s)` equality table.
struct FactoredEqualityWeights {
    low: Vec<RawMontgomery>,
    high: Vec<RawMontgomery>,
    low_vars: usize,
    len: usize,
}

impl FactoredEqualityWeights {
    fn new(
        point: &[SpartanF2zField],
        low_vars: usize,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Result<Self, Sha256F2zError> {
        if low_vars > point.len() {
            return Err(Sha256F2zError::InvalidGeometry);
        }
        let low = compact_eq_table(&point[..low_vars], field_config)?;
        let high = compact_eq_table(&point[low_vars..], field_config)?;
        let vars = u32::try_from(point.len()).map_err(|_| Sha256F2zError::InvalidGeometry)?;
        let len = 1usize
            .checked_shl(vars)
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        Ok(Self {
            low,
            high,
            low_vars,
            len,
        })
    }

    const fn len(&self) -> usize {
        self.len
    }

    fn scale(
        &mut self,
        factor: &SpartanF2zField,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) {
        let scale = |weight: &mut RawMontgomery| {
            let value = field_from_raw(*weight, field_config) * factor;
            *weight = raw_montgomery(&value);
        };
        #[cfg(feature = "parallel")]
        if self.high.len() >= 1 << 13 && rayon::current_num_threads() > 1 {
            self.high.par_iter_mut().for_each(scale);
        } else {
            self.high.iter_mut().for_each(scale);
        }
        #[cfg(not(feature = "parallel"))]
        self.high.iter_mut().for_each(scale);
    }

    #[cfg(test)]
    fn evaluate(
        &self,
        index: usize,
        field_config: &<SpartanF2zField as PrimeField>::Config,
    ) -> Option<SpartanF2zField> {
        if index >= self.len {
            return None;
        }
        let low_mask = self.low.len() - 1;
        let low = field_from_raw(*self.low.get(index & low_mask)?, field_config);
        let high = field_from_raw(*self.high.get(index >> self.low_vars)?, field_config);
        Some(low * &high)
    }
}

/// Builds `eq(boolean_index, point)` in little-endian index order while
/// storing only each element's two Montgomery limbs. A full `MontyField`
/// carries its runtime modulus configuration, which would otherwise multiply
/// the memory of the flat SHA domains by roughly five.
pub(super) fn compact_eq_table(
    point: &[SpartanF2zField],
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> Result<Vec<RawMontgomery>, Sha256F2zError> {
    if point.iter().any(|value| value.cfg() != field_config) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let vars = u32::try_from(point.len()).map_err(|_| Sha256F2zError::InvalidGeometry)?;
    let table_len = 1usize
        .checked_shl(vars)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    let mut table = vec![0; table_len];
    table[0] = raw_montgomery(&SpartanF2zField::one_with_cfg(field_config));

    for (coordinate, challenge) in point.iter().enumerate() {
        let half = 1usize
            .checked_shl(u32::try_from(coordinate).map_err(|_| Sha256F2zError::InvalidGeometry)?)
            .ok_or(Sha256F2zError::InvalidGeometry)?;
        let (zero_children, one_children) = table[..2 * half].split_at_mut(half);
        let expand = |(zero_child, one_child): (&mut RawMontgomery, &mut RawMontgomery)| {
            let parent = field_from_raw(*zero_child, field_config);
            let high = parent.clone() * challenge;
            *zero_child = raw_montgomery(&(parent - &high));
            *one_child = raw_montgomery(&high);
        };
        #[cfg(feature = "parallel")]
        if half >= 1 << 13 && rayon::current_num_threads() > 1 {
            zero_children
                .par_iter_mut()
                .zip(one_children.par_iter_mut())
                .for_each(expand);
        } else {
            zero_children
                .iter_mut()
                .zip(one_children.iter_mut())
                .for_each(expand);
        }
        #[cfg(not(feature = "parallel"))]
        zero_children
            .iter_mut()
            .zip(one_children.iter_mut())
            .for_each(expand);
    }

    Ok(table)
}

#[inline]
pub(super) fn raw_montgomery(value: &SpartanF2zField) -> RawMontgomery {
    let words = value.as_montgomery().as_words();
    u128::from(words[0]) | (u128::from(words[1]) << 64)
}

#[inline]
pub(super) fn field_from_raw(
    value: RawMontgomery,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) -> SpartanF2zField {
    MontyField::from_montgomery(FieldUint::from(value), field_config)
}

pub(super) fn validate_source_params(f_layout: &IntegerMatrixLayout) -> Result<(), Sha256F2zError> {
    let host_bits = usize::BITS as usize;
    if f_layout.word_bits != 1
        || f_layout.row_vars < LOG_PACKING
        || f_layout.row_vars.saturating_add(f_layout.col_vars) > 126
        || f_layout.row_vars >= host_bits
        || f_layout.col_vars >= host_bits
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_common_geometry(
    linear_relation: Option<&PreparedSha256LinearRelation>,
    map: &PackedRepeatedVirtualMap,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
) -> Result<(), Sha256F2zError> {
    validate_source_params(f_layout)?;
    if h_layout.word_bits != 1
        || h_layout.row_vars < LOG_PACKING
        || h_layout.row_vars.saturating_add(h_layout.col_vars) > 126
        || map.rows() != cell_count(h_layout)
        || map.cols() != cell_count(f_layout)
        || map.local().rows() != SHA256_H_BAR_LIVE_BITS
        || map.local().cols() != super::constraints::SHA256_F_BAR_LIVE_BITS
        || !map_fixes_constant_assignment(map)
        || !map_fixes_public_statement(map)
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    if linear_relation.is_some_and(|relation| {
        relation.matrix().row_count() != SHA256_CONSTRAINTS
            || relation.matrix().column_count() != SHA256_H_BAR_LIVE_BITS
    }) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn validate_product_geometry(
    map: &PackedSourceRepeatedVirtualMap,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
) -> Result<(), Sha256F2zError> {
    validate_source_params(f_layout)?;
    let instance_vars = map.instances().ilog2() as usize;
    let local_stride = SHA256_H_BAR_LIVE_BITS.next_power_of_two();
    let (t_min, t_max, expected_live_rows) = match map.order() {
        PackedSourceOrder::LocalMajor => (
            LOG_PACKING,
            instance_vars,
            map.instances() * SHA256_H_BAR_LIVE_BITS,
        ),
        PackedSourceOrder::InstanceMajor => (
            local_stride.ilog2() as usize,
            local_stride.ilog2() as usize + instance_vars,
            map.instances() * local_stride,
        ),
    };
    if h_layout.word_bits != 1
        || h_layout.row_vars < t_min
        || h_layout.row_vars > t_max
        || h_layout.row_vars.saturating_add(h_layout.col_vars) != instance_vars + 15
        || map.rows() != cell_count(h_layout)
        || map.cols() != cell_count(f_layout)
        || map.live_rows() != expected_live_rows
        || map.local_stride()
            != match map.order() {
                PackedSourceOrder::LocalMajor => map.instances(),
                PackedSourceOrder::InstanceMajor => local_stride,
            }
        || map.local().rows() != SHA256_H_BAR_LIVE_BITS
        || map.local().cols() != super::constraints::SHA256_F_BAR_LIVE_BITS
        || !map_fixes_constant_assignment_local(map.local())
        || !map_fixes_public_statement_local(map.local())
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

fn map_fixes_constant_assignment(map: &PackedRepeatedVirtualMap) -> bool {
    map_fixes_constant_assignment_local(map.local())
}

pub(super) fn map_fixes_constant_assignment_local(map: &crate::f2map::PreparedVirtualMap) -> bool {
    let mut constant_source = None;
    for (column, entries) in map.matrix().columns().enumerate() {
        for row in entries.row_indices() {
            if *row == SHA256_SHARED_CONSTANT_CELL && constant_source.replace(column).is_some() {
                return false;
            }
        }
    }
    constant_source == Some(SHA256_SHARED_CONSTANT_CELL)
}

fn map_fixes_public_statement(map: &PackedRepeatedVirtualMap) -> bool {
    map_fixes_public_statement_local(map.local())
}

fn map_fixes_public_statement_local(map: &crate::f2map::PreparedVirtualMap) -> bool {
    const PUBLIC_BITS: usize = SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS;

    let mut sources = [None; PUBLIC_BITS];
    for (f_column, entries) in map.matrix().columns().enumerate() {
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

pub(super) fn validate_rows(
    p: &IntegerMatrixLayout,
    rows: &[Vec<u64>],
) -> Result<(), Sha256F2zError> {
    let words = p.rows().div_ceil(64);
    if rows.len() != p.cols() || rows.iter().any(|row| row.len() != words) {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
}

/// Reads the conceptual flat sequence in F2Z's native packed order. Low `t`
/// index bits select the packed row and high `s` bits select the column, so
/// adjacent SHA instances remain adjacent in the legacy sumcheck oracle and
/// canonical virtual-map indices.
fn packed_flat_bit(
    rows: &[Vec<u64>],
    p: &IntegerMatrixLayout,
    flat_cell: usize,
) -> Result<u64, SumcheckError> {
    if flat_cell >= p.cells() || rows.len() != p.cols() {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    let column = flat_cell >> p.row_vars;
    let row = flat_cell & (p.rows() - 1);
    let words = rows
        .get(column)
        .ok_or(SumcheckError::InvalidProductDimensions)?;
    let word = words
        .get(row / u64::BITS as usize)
        .ok_or(SumcheckError::InvalidProductDimensions)?;
    Ok((word >> (row % u64::BITS as usize)) & 1)
}

pub(super) fn validate_shared_constant(rows: &[Vec<u64>]) -> Result<(), Sha256F2zError> {
    let packed = rows
        .get(SHA256_SHARED_CONSTANT_CELL)
        .ok_or(Sha256F2zError::InvalidGeometry)?;
    if packed.first().is_none_or(|word| word & 1 == 0) {
        return Err(Sha256F2zError::InvalidSharedConstant);
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
    match (prepared.product_map(), prepared.product_assignment_params()) {
        (Some(product_map), Some(product_params)) => {
            hash.update(&[1]);
            hash.update(&product_map.digest());
            for value in [
                product_params.row_vars,
                product_params.col_vars,
                product_params.word_bits,
            ] {
                hash_usize(&mut hash, value)?;
            }
        }
        (None, None) => {
            hash.update(&[0]);
        }
        _ => return Err(Sha256F2zError::InvalidGeometry),
    }
    hash.update(public_statement_binding);
    for value in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        prepared.instances(),
        prepared.log_instance_capacity(),
        prepared.assignment_params().row_vars,
        prepared.assignment_params().col_vars,
        prepared.assignment_params().word_bits,
        prepared.source_params().row_vars,
        prepared.source_params().col_vars,
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
    hash_security_params(hash, prepared.security())
}

/// Binds every instantiated security parameter (the profile name, target,
/// intervals, grinding schedule, and Ligerito target).
pub(super) fn hash_security_params(
    hash: &mut Hasher,
    security: &super::super::profile::IopSecurityParams,
) -> Result<(), Sha256F2zError> {
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
    if let Some(ood) = security.ood {
        // Present only when Round 0 (the out-of-domain sample) runs, so
        // Round-0-less statements keep their digest.
        hash.update(&[1]);
        hash.update(&ood.grinding_bits.to_le_bytes());
    }
    hash_usize(hash, security.ligerito_target_bits)?;
    Ok(())
}

pub(super) fn hash_ligerito_config(
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
    if let (Some(product_map), Some(product_params)) =
        (prepared.product_map(), prepared.product_assignment_params())
    {
        absorb_spartan_message(transcript, b"product-boolean-map", &product_map.digest());
        absorb_spartan_message(
            transcript,
            b"product-assignment-row-vars",
            &(product_params.row_vars as u64).to_le_bytes(),
        );
        absorb_spartan_message(
            transcript,
            b"product-assignment-column-vars",
            &(product_params.col_vars as u64).to_le_bytes(),
        );
    } else {
        absorb_spartan_message(transcript, b"product-boolean-map", b"legacy-inner-sumcheck");
    }
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
        b"assignment-row-vars",
        &(prepared.assignment_params().row_vars as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"assignment-column-vars",
        &(prepared.assignment_params().col_vars as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"source-row-vars",
        &(prepared.source_params().row_vars as u64).to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"source-column-vars",
        &(prepared.source_params().col_vars as u64).to_le_bytes(),
    );
    absorb_spartan_message(transcript, b"public-sha256-io", public_statement_binding);
    absorb_spartan_message(transcript, b"assignment-oracle", assignment_binding);
}

fn absorb_runtime_sha256_relation(
    transcript: &mut impl Transcript,
    prepared: &PreparedSha256CompressionBatch,
    field_config: &<SpartanF2zField as PrimeField>::Config,
) {
    absorb_spartan_message(
        transcript,
        b"runtime-field-modulus",
        &SpartanF2zField::canonical_modulus_encoding(field_config),
    );
    // The exact signed relation was bound before prime sampling.  Projection
    // is deterministic from that digest and the runtime modulus, so no dense
    // or padded matrix serialization is needed here.
    absorb_spartan_message(
        transcript,
        b"projected-linear-relation",
        prepared.integer_relation_digest(),
    );
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
fn absorb_opening_claim<S: ModQWeightSource + ?Sized>(
    transcript: &mut impl Transcript,
    assignment_binding: &[u8; 32],
    local_row_point: &[SpartanF2zField],
    instance_point: &[SpartanF2zField],
    assignment_point: &[SpartanF2zField],
    collapsed_evaluation: &SpartanF2zField,
    constant_one_batch: &SpartanF2zField,
    public_slot_weights: &[SpartanF2zField],
    public_io_batch: &SpartanF2zField,
    row_weight_source: &S,
    col_weights: &[u128],
    claimed: u128,
) -> Result<(), Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_OPENING_CLAIM_DOMAIN);
    hash.update(assignment_binding);
    hash_usize(&mut hash, local_row_point.len())?;
    for coordinate in local_row_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash_usize(&mut hash, instance_point.len())?;
    for coordinate in instance_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash_usize(&mut hash, assignment_point.len())?;
    for coordinate in assignment_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&collapsed_evaluation.canonical_element_encoding());
    hash.update(&constant_one_batch.canonical_element_encoding());
    hash_usize(&mut hash, public_slot_weights.len())?;
    for coordinate in public_slot_weights {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&public_io_batch.canonical_element_encoding());
    hash_usize(&mut hash, row_weight_source.row_count())?;
    for row in 0..row_weight_source.row_count() {
        let weight = row_weight_source
            .canonical_weight(row)
            .ok_or(Sha256F2zError::InvalidGeometry)?;
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

#[allow(clippy::too_many_arguments)]
fn absorb_product_opening_claim<S: ModQWeightSource + ?Sized>(
    transcript: &mut impl Transcript,
    assignment_binding: &[u8; 32],
    local_row_point: &[SpartanF2zField],
    instance_point: &[SpartanF2zField],
    constant_one_batch: &SpartanF2zField,
    public_slot_weights: &[SpartanF2zField],
    public_io_batch: &SpartanF2zField,
    row_weight_source: &S,
    col_weights: &[u128],
    claimed: u128,
) -> Result<(), Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_OPENING_CLAIM_DOMAIN);
    hash.update(b"direct-product");
    hash.update(assignment_binding);
    hash_usize(&mut hash, local_row_point.len())?;
    for coordinate in local_row_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash_usize(&mut hash, instance_point.len())?;
    for coordinate in instance_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&constant_one_batch.canonical_element_encoding());
    hash_usize(&mut hash, public_slot_weights.len())?;
    for coordinate in public_slot_weights {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&public_io_batch.canonical_element_encoding());
    hash_usize(&mut hash, row_weight_source.row_count())?;
    for row in 0..row_weight_source.row_count() {
        let weight = row_weight_source
            .canonical_weight(row)
            .ok_or(Sha256F2zError::InvalidGeometry)?;
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

pub(super) fn hash_usize(hash: &mut Hasher, value: usize) -> Result<(), Sha256F2zError> {
    hash.update(
        &u64::try_from(value)
            .map_err(|_| Sha256F2zError::BindingEncodingOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        ConstIntSemiring, FromWithConfig, PrimeField, crypto_bigint_monty::F128,
        crypto_bigint_uint::Uint,
    };

    use crate::{
        pcs::FQ_MOD,
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
        for log_compressions in 7..=super::super::prime::SHA256_MAX_LOG_COMPRESSIONS
        {
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
    fn production_boundary_and_both_regimes_bind_public_outputs() {
        // These algebraic shapes have fewer than the supported 2^20 committed bits.
        for exponent in 4..=6 {
            assert!(prepare_sha256_compression_batch(exponent).is_err());
        }
        let exponent = 7;
        for selection in [
            crate::ligerito_flock::LigeritoSelection::JOHNSON,
            crate::ligerito_flock::LigeritoSelection::MATCHED_UDR,
        ] {
            let prepared = prepare_sha256_compression_batch(exponent)
                .unwrap()
                .with_ligerito(selection)
                .unwrap();
            assert!(prepared.product_assignment_params().is_some());
            assert!(prepared.opening_params().row_vars >= LOG_PACKING);
            assert!(prepared.security().accounting.achieved_bits() >= 100.0);
            let inputs = (0..prepared.instances()).map(input).collect::<Vec<_>>();
            let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
            let mut statements = public_statements(&inputs, witness.outputs());
            let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
            let hint =
                commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();
            let proof = prove_sha256_compressions_with_config(
                &mut Blake3Transcript::new(),
                &prepared,
                &statements,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();
            assert_eq!(proof.f2z().mfs.len(), 2, "production product layout forests");
            verify_sha256_compressions_with_config(
                &mut Blake3Transcript::new(),
                &prepared,
                &statements,
                &hint.commitment,
                &proof,
                &vc,
            )
            .unwrap();

            statements.last_mut().unwrap().claimed_output[7] ^= 1 << 31;
            assert!(
                verify_sha256_compressions_with_config(
                    &mut Blake3Transcript::new(),
                    &prepared,
                    &statements,
                    &hint.commitment,
                    &proof,
                    &vc,
                )
                .is_err(),
                "the final output bit must be bound at 2^{exponent} compressions"
            );
        }
    }

    #[test]
    fn assignment_row_sized_non_power_batch_roundtrip_and_public_binding() {
        let prepared = prepare_sha256_compression_batch_for_assignment_rows(21).unwrap();
        assert_eq!(prepared.instances(), 102);

        let inputs = (0..prepared.instances()).map(input).collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let public_statement = public_statements(&inputs, witness.outputs());
        let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_with_config(
            &mut prover_transcript,
            &prepared,
            &public_statement,
            &witness,
            &hint,
            &pc,
        )
        .unwrap();

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

        let mut false_statement = public_statement.clone();
        false_statement.last_mut().unwrap().claimed_output[7] ^= 1 << 31;
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
            .is_err(),
            "the last public output bit of the partial final repetition must be bound",
        );
    }

    #[test]
    fn explicit_inner_sumcheck_layout_roundtrips_across_forest_counts() {
        use super::super::super::{
            Sha256OpeningLayout, prepare_sha256_compression_batch_with_profile_and_layout,
        };
        use crate::piop::spartan::profile::Lambda100;
        // 2^7 compressions: 2^22 assignment cells. t = 13 keeps one forest
        // (113-bit primes, c_w = 113); t = 15 narrows c_w to 111 and needs two.
        const LOG_COMPRESSIONS: usize = 7;
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        for (row_vars, expected_forests) in [(13usize, 1usize), (15, 2)] {
            let prepared = prepare_sha256_compression_batch_with_profile_and_layout::<Lambda100>(
                LOG_COMPRESSIONS,
                Sha256OpeningLayout::InnerSumcheck { row_vars },
            )
            .unwrap();
            assert_eq!(
                prepared.opening_layout(),
                Sha256OpeningLayout::InnerSumcheck { row_vars }
            );
            assert!(prepared.product_assignment_params().is_none());
            let h_layout = *prepared.opening_params();
            assert_eq!(
                (h_layout.row_vars, h_layout.row_vars + h_layout.col_vars),
                (row_vars, 22)
            );
            let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
            let public_statement = public_statements(&inputs, witness.outputs());
            let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
            let hint =
                commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();
            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_sha256_compressions_with_config(
                &mut prover_transcript,
                &prepared,
                &public_statement,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();
            assert_eq!(
                proof.f2z().mfs.len(),
                expected_forests,
                "forests at t={row_vars}"
            );
            assert_eq!(
                proof.inner().round_polynomials.len(),
                22,
                "inner rounds at t={row_vars}"
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
        }
        for row_vars in [LOG_PACKING - 1, 22] {
            assert!(matches!(
                prepare_sha256_compression_batch_with_profile_and_layout::<Lambda100>(
                    LOG_COMPRESSIONS,
                    Sha256OpeningLayout::InnerSumcheck { row_vars },
                ),
                Err(crate::piop::spartan::Sha256ConstraintError::InvalidOpeningRowVars { .. })
            ));
        }
    }

    #[test]
    fn transposed_product_layout_roundtrips_without_an_inner_sumcheck() {
        use super::super::super::{
            Sha256OpeningLayout, prepare_sha256_compression_batch_with_profile_and_layout,
        };
        use crate::piop::spartan::profile::Lambda100;
        // 2^7 compressions: 2^22 assignment cells, local stride 2^15. Every
        // admissible split has t >= 15, so two forests at 113-bit primes.
        const LOG_COMPRESSIONS: usize = 7;
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        for row_vars in [15usize, 17, 22] {
            let prepared = prepare_sha256_compression_batch_with_profile_and_layout::<Lambda100>(
                LOG_COMPRESSIONS,
                Sha256OpeningLayout::ProductTransposed { row_vars },
            )
            .unwrap();
            let product_map = prepared.product_map().unwrap();
            assert_eq!(product_map.order(), PackedSourceOrder::InstanceMajor);
            let h_layout = *prepared.opening_params();
            assert_eq!(
                (h_layout.row_vars, h_layout.row_vars + h_layout.col_vars),
                (row_vars, 22)
            );
            let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
            let public_statement = public_statements(&inputs, witness.outputs());
            let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
            let hint =
                commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();
            let mut prover_transcript = Blake3Transcript::new();
            let proof = prove_sha256_compressions_with_config(
                &mut prover_transcript,
                &prepared,
                &public_statement,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();
            assert_eq!(proof.f2z().mfs.len(), 2, "forests at t={row_vars}");
            assert!(proof.inner().round_polynomials.is_empty());
            assert_eq!(proof.f2z().us[0].len(), 1 << (22 - row_vars));
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
        }
        for row_vars in [14usize, 23] {
            assert!(matches!(
                prepare_sha256_compression_batch_with_profile_and_layout::<Lambda100>(
                    LOG_COMPRESSIONS,
                    Sha256OpeningLayout::ProductTransposed { row_vars },
                ),
                Err(crate::piop::spartan::Sha256ConstraintError::InvalidOpeningRowVars { .. })
            ));
        }
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

        let mut invalid_prefix_transcript = Blake3Transcript::new();
        let mut untouched = invalid_prefix_transcript.clone();
        assert!(matches!(
            prove_sha256_compressions_with_prefix_vars_and_config(
                &mut invalid_prefix_transcript,
                &prepared,
                &public_statement,
                &witness,
                &hint,
                SHA256_INNER_PREFIX_MAX_VARS + 1,
                &pc,
            ),
            Err(Sha256F2zError::InvalidInnerPrefix { actual: 5, max: 4 })
        ));
        assert_eq!(
            invalid_prefix_transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>(),
            "invalid prover-local K must reject before transcript absorption"
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
        assert!(proof.inner().round_polynomials.is_empty());
        assert!(proof.inner_nonces().is_empty());

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

        let mut tampered_inner = proof.clone();
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let zero = SpartanF2zField::zero_with_cfg(&field_config);
        tampered_inner
            .inner
            .round_polynomials
            .push([zero.clone(), zero.clone(), zero]);
        let mut tampered_inner_transcript = Blake3Transcript::new();
        assert!(
            verify_sha256_compressions_with_config(
                &mut tampered_inner_transcript,
                &prepared,
                &public_statement,
                &hint.commitment,
                &tampered_inner,
                &vc,
            )
            .is_err()
        );

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
            .expect("a false public claim still produces a candidate proof");

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
        // Changing it while retaining the source commitment must not produce
        // an accepting linear proof.
        const DERIVED_ASSIGNMENT_CELL: usize = 769;
        let mut inconsistent_witness = witness.clone();
        let product_p_h = prepared.product_assignment_params().unwrap();
        let flat_cell = DERIVED_ASSIGNMENT_CELL * prepared.instances();
        let column = flat_cell >> product_p_h.row_vars;
        let row = flat_cell & (product_p_h.rows() - 1);
        inconsistent_witness
            .product_assignment_rows_mut_for_tests()
            .unwrap()[column][row / 64] ^= 1 << (row % 64);
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

        // Flip one committed, non-public hint bit and update both assignment
        // views by the public Boolean map. This preserves h = M f exactly, so
        // rejection must come from the batched C h = 0 constraints rather
        // than from an inconsistent uncommitted h oracle.
        const PRIVATE_LOCAL_SOURCE_CELL: usize = 1_000;
        assert!((0..SHA256_PUBLIC_WORDS).all(|word_slot| {
            (0..SHA256_PUBLIC_WORD_BITS)
                .all(|bit| sha256_public_f_column(word_slot, bit) != PRIVATE_LOCAL_SOURCE_CELL)
        }));
        let mut local_residuals =
            vec![num_bigint::BigInt::default(); prepared.linear_relation().row_count()];
        for local_h in prepared
            .map()
            .local()
            .matrix()
            .column(PRIVATE_LOCAL_SOURCE_CELL)
            .unwrap()
            .row_indices()
        {
            for (row, coefficient) in prepared
                .linear_relation()
                .matrix()
                .column(*local_h)
                .unwrap()
            {
                local_residuals[row] += coefficient;
            }
        }
        assert!(
            local_residuals
                .iter()
                .any(|residual| residual != &num_bigint::BigInt::default())
        );

        let source_cell = PRIVATE_LOCAL_SOURCE_CELL;
        let canonical_h_cells = prepared
            .map()
            .column_rows(source_cell)
            .unwrap()
            .collect::<Vec<_>>();
        let product_h_cells = prepared
            .product_map()
            .unwrap()
            .column_rows(source_cell)
            .unwrap()
            .collect::<Vec<_>>();
        let mut constraint_attack = witness.clone();
        let f_layout = prepared.source_params();
        let source_column = source_cell >> f_layout.row_vars;
        let source_row = source_cell & (f_layout.rows() - 1);
        constraint_attack.source_rows_mut_for_tests()[source_column][source_row / 64] ^=
            1 << (source_row % 64);
        for flat_cell in canonical_h_cells {
            let canonical_p_h = prepared.assignment_params();
            let column = flat_cell >> canonical_p_h.row_vars;
            let row = flat_cell & (canonical_p_h.rows() - 1);
            constraint_attack.assignment_rows_mut_for_tests()[column][row / 64] ^= 1 << (row % 64);
        }
        for flat_cell in product_h_cells {
            let column = flat_cell >> product_p_h.row_vars;
            let row = flat_cell & (product_p_h.rows() - 1);
            constraint_attack
                .product_assignment_rows_mut_for_tests()
                .unwrap()[column][row / 64] ^= 1 << (row % 64);
        }
        let attack_hint =
            commit_sha256_compression_witness_with_config(&prepared, &constraint_attack, &pc)
                .unwrap();
        let mut attack_prover_transcript = Blake3Transcript::new();
        let attack_proof = prove_sha256_compressions_with_config(
            &mut attack_prover_transcript,
            &prepared,
            &public_statement,
            &constraint_attack,
            &attack_hint,
            &pc,
        )
        .unwrap();
        let mut attack_verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_sha256_compressions_with_config(
                &mut attack_verifier_transcript,
                &prepared,
                &public_statement,
                &attack_hint.commitment,
                &attack_proof,
                &vc,
            )
            .is_err()
        );
    }

    #[test]
    fn local_rows_product_layout_rejects_a_false_public_output() {
        const LOG_COMPRESSIONS: usize = 7;
        let prepared =
            super::super::constraints::prepare_sha256_compression_batch_for_product_t_test(
                LOG_COMPRESSIONS,
                15,
            )
            .unwrap().with_ligerito(crate::ligerito_flock::LigeritoSelection::JOHNSON).unwrap();
        assert_eq!(prepared.product_layout_name(), Some("local_rows"));

        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let mut false_statement = public_statements(&inputs, witness.outputs());
        false_statement[5].claimed_output[0] ^= 1;
        let (pc, vc) = sha256_compression_configs(&prepared).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc).unwrap();

        // Both sides bind the same false statement. Rejection therefore comes
        // from the committed-wire claim, not from transcript divergence.
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_with_config(
            &mut prover_transcript,
            &prepared,
            &false_statement,
            &witness,
            &hint,
            &pc,
        )
        .expect("a false public claim still produces a candidate proof");
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

    #[test]
    fn compact_equality_table_matches_field_table() {
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let point = (0..12)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 7, &field_config)
            })
            .collect::<Vec<_>>();
        let dense = eq_table(&point, &field_config).unwrap();
        let compact = compact_eq_table(&point, &field_config).unwrap();
        let factored = FactoredEqualityWeights::new(&point, 7, &field_config).unwrap();
        let scale = SpartanF2zField::from_with_cfg(37_u64, &field_config);
        let mut scaled_factored = FactoredEqualityWeights::new(&point, 7, &field_config).unwrap();
        scaled_factored.scale(&scale, &field_config);

        assert_eq!(compact.len(), dense.len());
        assert_eq!(factored.len(), dense.len());
        for (index, (raw, expected)) in compact.into_iter().zip(dense).enumerate() {
            assert_eq!(field_from_raw(raw, &field_config), expected);
            assert_eq!(
                factored.evaluate(index, &field_config),
                Some(expected.clone())
            );
            assert_eq!(
                scaled_factored.evaluate(index, &field_config),
                Some(expected * &scale)
            );
        }
    }

    #[test]
    fn local_beta_matches_manual_c_transpose_eq8_collapse() {
        let prepared = super::super::constraints::prepare_sha256_compression_batch_for_test(0)
            .expect("one-instance test relation");
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let local_row_point = (0..local_constraint_vars())
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate as u64 + 5, &field_config))
            .collect::<Vec<_>>();
        let local_row_weights = eq_table(&local_row_point, &field_config).unwrap();
        let reducer = OptimizedSumcheckReducer::new(&field_config).unwrap();
        let beta =
            collapse_local_linear_columns(&prepared, &local_row_weights, &reducer, &field_config)
                .unwrap();
        let projected = prepared.project_linear_relation(&field_config).unwrap();

        assert_eq!(local_row_weights.len(), 1 << 8);
        assert_eq!(beta.len(), SHA256_H_BAR_LIVE_BITS);
        for (column, expected) in projected.matrix().columns().zip(&beta) {
            let mut manual = SpartanF2zField::zero_with_cfg(&field_config);
            for (local_row, coefficient) in column {
                manual += &(coefficient.clone() * &local_row_weights[local_row]);
            }
            assert_eq!(&manual, expected);
        }
    }

    #[test]
    fn product_linear_batching_matches_the_gap_free_witness_and_terminal_evaluation() {
        let prepared = super::super::constraints::prepare_sha256_compression_batch_for_test(1)
            .expect("two-instance test relation");
        let inputs = [input(5), input(9)];
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let statements = public_statements(&inputs, witness.outputs());
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let local_row_point = (0..local_constraint_vars())
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate as u64 + 3, &field_config))
            .collect::<Vec<_>>();
        let local_row_weights = eq_table(&local_row_point, &field_config).unwrap();
        let reducer = OptimizedSumcheckReducer::new(&field_config).unwrap();
        let beta =
            collapse_local_linear_columns(&prepared, &local_row_weights, &reducer, &field_config)
                .unwrap();
        let instance_point = [SpartanF2zField::from_with_cfg(13_u64, &field_config)];
        let slot_weights = (0..SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS)
            .map(|slot| SpartanF2zField::from_with_cfg(slot as u64 + 17, &field_config))
            .collect::<Vec<_>>();
        let batching = ProductLinearBatching::new(
            &prepared,
            &statements,
            &instance_point,
            beta,
            slot_weights,
            SpartanF2zField::from_with_cfg(29_u64, &field_config),
            SpartanF2zField::from_with_cfg(31_u64, &field_config),
            &field_config,
        )
        .unwrap();

        let factored = batching.factored_matrix_mle(&field_config).unwrap();
        assert_eq!(
            factored.live_len(),
            prepared.linear_assignment_column_count()
        );
        let mut witness_sum = SpartanF2zField::zero_with_cfg(&field_config);
        for flat_cell in 0..prepared.linear_assignment_column_count() {
            if witness.assignment_bit(flat_cell).unwrap() {
                witness_sum += &batching.coefficient(flat_cell, &field_config).unwrap();
            }
        }
        assert_eq!(witness_sum, *batching.initial_claim());

        let product_p_h = prepared.product_assignment_params().unwrap();
        let product_rows = witness.product_assignment_rows().unwrap();
        let order = prepared.product_map().unwrap().order();
        let (row_weights, col_weights, claimed) =
            product_opening_claim(&batching, product_p_h, order, &field_config).unwrap();
        let mut direct_sum = SpartanF2zField::zero_with_cfg(&field_config);
        for flat_cell in 0..product_p_h.cells() {
            if packed_flat_bit(product_rows, product_p_h, flat_cell).unwrap() == 0 {
                continue;
            }
            let row = flat_cell & (product_p_h.rows() - 1);
            let column = flat_cell >> product_p_h.row_vars;
            let mut term = SpartanF2zField::from_with_cfg(
                row_weights.canonical_weight(row, &field_config).unwrap(),
                &field_config,
            );
            term *= &SpartanF2zField::from_with_cfg(col_weights[column], &field_config);
            direct_sum += &term;
        }
        assert_eq!(direct_sum, *batching.initial_claim());
        assert_eq!(claimed, batching.initial_claim().canonical_u128());

        let assignment_point = (0..prepared.assignment_params().row_vars
            + prepared.assignment_params().col_vars)
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate as u64 + 37, &field_config))
            .collect::<Vec<_>>();
        let assignment_equality = FactoredEqualityWeights::new(
            &assignment_point,
            prepared.assignment_params().row_vars,
            &field_config,
        )
        .unwrap();
        assert_eq!(
            batching.evaluate(&assignment_point, &field_config).unwrap(),
            batching
                .evaluate_dense(&assignment_equality, &field_config)
                .unwrap()
        );
    }

    #[test]
    fn product_linear_batching_handles_partial_instance_domains_without_inversion() {
        let prepared = prepare_sha256_compression_batch_for_assignment_rows(21)
            .expect("102-instance packed test relation");
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let instance_point = (0..instance_vars(prepared.instances()).unwrap())
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate as u64 + 41, &field_config))
            .collect::<Vec<_>>();
        let instance_weights = eq_table(&instance_point, &field_config).unwrap();
        let active_sum = instance_weights.iter().take(prepared.instances()).fold(
            SpartanF2zField::zero_with_cfg(&field_config),
            |mut sum, weight| {
                sum += weight;
                sum
            },
        );
        let statements = (0..prepared.instances())
            .map(|instance| Sha256CompressionStatement::new(input(instance), [0; 8]))
            .collect::<Vec<_>>();
        let constant_weight = SpartanF2zField::from_with_cfg(43_u64, &field_config);
        let batching = ProductLinearBatching::new(
            &prepared,
            &statements,
            &instance_point,
            vec![SpartanF2zField::zero_with_cfg(&field_config); SHA256_H_BAR_LIVE_BITS],
            vec![
                SpartanF2zField::zero_with_cfg(&field_config);
                SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS
            ],
            SpartanF2zField::from_with_cfg(47_u64, &field_config),
            constant_weight.clone(),
            &field_config,
        )
        .unwrap();
        let expected = active_sum * &constant_weight;

        assert_eq!(
            batching.coefficient(SHA256_SHARED_CONSTANT_CELL, &field_config),
            Ok(expected.clone())
        );
        assert_eq!(*batching.initial_claim(), expected);
        assert_eq!(
            batching
                .factored_matrix_mle(&field_config)
                .unwrap()
                .live_len(),
            prepared.linear_assignment_column_count()
        );
    }

    #[test]
    fn public_linear_batching_matches_the_packed_witness_and_terminal_evaluation() {
        let prepared = super::super::constraints::prepare_sha256_compression_batch_for_test(1)
            .expect("two-instance test relation");
        let inputs = [input(5), input(9)];
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let statements = public_statements(&inputs, witness.outputs());
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let instance_point = [SpartanF2zField::from_with_cfg(7_u64, &field_config)];
        let slot_weights = (0..SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS)
            .map(|slot| SpartanF2zField::from_with_cfg(slot as u64 + 11, &field_config))
            .collect::<Vec<_>>();
        let public_batch_weight = SpartanF2zField::from_with_cfg(19_u64, &field_config);
        let constant_weight = SpartanF2zField::from_with_cfg(23_u64, &field_config);
        let batching = PublicLinearBatching::new(
            &prepared,
            &statements,
            &instance_point,
            slot_weights,
            public_batch_weight,
            constant_weight,
            &field_config,
        )
        .unwrap();

        let mut witness_sum = SpartanF2zField::zero_with_cfg(&field_config);
        for flat_cell in 0..prepared.assignment_params().cells() {
            if witness.assignment_bit(flat_cell).unwrap() {
                witness_sum += &batching.coefficient(flat_cell, &field_config).unwrap();
            }
        }
        assert_eq!(witness_sum, *batching.initial_claim());

        let assignment_point = (0..prepared.assignment_params().row_vars
            + prepared.assignment_params().col_vars)
            .map(|coordinate| SpartanF2zField::from_with_cfg(coordinate as u64 + 29, &field_config))
            .collect::<Vec<_>>();
        let dense_equality = eq_table(&assignment_point, &field_config).unwrap();
        let factored = FactoredEqualityWeights::new(
            &assignment_point,
            prepared.assignment_params().row_vars,
            &field_config,
        )
        .unwrap();
        let mut dense_evaluation = SpartanF2zField::zero_with_cfg(&field_config);
        for (flat_cell, equality) in dense_equality.iter().enumerate() {
            let coefficient = batching.coefficient(flat_cell, &field_config).unwrap();
            dense_evaluation += &(coefficient * equality);
        }
        assert_eq!(
            batching.evaluate(&assignment_point, &field_config).unwrap(),
            dense_evaluation
        );
        assert_eq!(
            batching.evaluate_dense(&factored, &field_config).unwrap(),
            dense_evaluation
        );
    }

    #[test]
    fn native_prover_collapse_matches_sparse_verifier_evaluation() {
        let prepared = super::super::constraints::prepare_sha256_compression_batch_for_test(1)
            .expect("two-instance test relation");
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let relation = prepared
            .project_linear_relation(&field_config)
            .expect("project linear relation");

        let constraint_point = (0..flat_constraint_vars(&prepared).unwrap())
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 2, &field_config)
            })
            .collect::<Vec<_>>();
        let constraint_weights = compact_eq_table(&constraint_point, &field_config).unwrap();
        let reducer = OptimizedSumcheckReducer::new(&field_config).unwrap();
        let collapsed = (0..prepared.assignment_params().cells())
            .map(|flat_column| {
                collapse_flat_linear_column(
                    &prepared,
                    &relation,
                    &constraint_weights,
                    &reducer,
                    flat_column,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();

        let assignment_point = (0..prepared.assignment_params().row_vars
            + prepared.assignment_params().col_vars)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 19, &field_config)
            })
            .collect::<Vec<_>>();
        let assignment_weights = FactoredEqualityWeights::new(
            &assignment_point,
            prepared.assignment_params().row_vars,
            &field_config,
        )
        .unwrap();
        let zero = SpartanF2zField::zero_with_cfg(&field_config);
        let dense_evaluation =
            collapsed
                .iter()
                .enumerate()
                .fold(zero, |mut sum, (flat_column, coefficient)| {
                    let weight = assignment_weights
                        .evaluate(flat_column, &field_config)
                        .unwrap();
                    sum += &(coefficient.clone() * &weight);
                    sum
                });
        let sparse_evaluation = evaluate_flat_linear_collapse_dense(
            &prepared,
            &relation,
            &constraint_weights,
            &assignment_weights,
        )
        .unwrap();
        let repeated_evaluation = evaluate_repeated_flat_linear_collapse(
            &prepared,
            &relation,
            &constraint_point,
            &assignment_point,
        )
        .unwrap();

        assert_eq!(dense_evaluation, sparse_evaluation);
        assert_eq!(dense_evaluation, repeated_evaluation);
    }

    #[test]
    fn affine_equality_repetition_matches_dense_partial_domain() {
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let left_point = (0..6)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 3, &field_config)
            })
            .collect::<Vec<_>>();
        let right_point = (0..6)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 17, &field_config)
            })
            .collect::<Vec<_>>();
        let instance_point = (0..3)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 31, &field_config)
            })
            .collect::<Vec<_>>();
        let terms = [
            (0, 1, SpartanF2zField::from_with_cfg(5_u64, &field_config)),
            (2, 4, SpartanF2zField::from_with_cfg(7_u64, &field_config)),
            (6, 9, SpartanF2zField::from_with_cfg(11_u64, &field_config)),
        ];
        let optimized = evaluate_affine_equality_repetition(
            5,
            7,
            11,
            &left_point,
            &right_point,
            Some(&instance_point),
            terms.clone(),
            &field_config,
        )
        .unwrap();

        let left_equality = eq_table(&left_point, &field_config).unwrap();
        let right_equality = eq_table(&right_point, &field_config).unwrap();
        let instance_equality = eq_table(&instance_point, &field_config).unwrap();
        let mut dense = SpartanF2zField::zero_with_cfg(&field_config);
        for (instance, instance_weight) in instance_equality.iter().take(5).enumerate() {
            for (left_offset, right_offset, coefficient) in &terms {
                let mut term = coefficient.clone() * &left_equality[7 * instance + left_offset];
                term *= &right_equality[11 * instance + right_offset];
                term *= instance_weight;
                dense += &term;
            }
        }
        assert_eq!(optimized, dense);

        let shared_terms = [
            (1, 0, SpartanF2zField::from_with_cfg(13_u64, &field_config)),
            (6, 0, SpartanF2zField::from_with_cfg(17_u64, &field_config)),
        ];
        let optimized_shared = evaluate_affine_equality_repetition(
            5,
            7,
            0,
            &left_point,
            &right_point,
            None,
            shared_terms.clone(),
            &field_config,
        )
        .unwrap();
        let mut dense_shared = SpartanF2zField::zero_with_cfg(&field_config);
        for instance in 0..5 {
            for (left_offset, _, coefficient) in &shared_terms {
                dense_shared += &(coefficient.clone()
                    * &left_equality[7 * instance + left_offset]
                    * &right_equality[0]);
            }
        }
        assert_eq!(optimized_shared, dense_shared);
    }

    #[test]
    fn non_power_sha_terminal_contractions_match_dense_evaluation() {
        let prepared = prepare_sha256_compression_batch_for_assignment_rows(21)
            .expect("102-instance packed test relation");
        assert_eq!(prepared.instances(), 102);
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let relation = prepared
            .project_linear_relation(&field_config)
            .expect("project linear relation");
        let constraint_point = (0..flat_constraint_vars(&prepared).unwrap())
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 2, &field_config)
            })
            .collect::<Vec<_>>();
        let assignment_point = (0..prepared.assignment_params().row_vars
            + prepared.assignment_params().col_vars)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 41, &field_config)
            })
            .collect::<Vec<_>>();
        let constraint_weights = compact_eq_table(&constraint_point, &field_config).unwrap();
        let assignment_weights = FactoredEqualityWeights::new(
            &assignment_point,
            prepared.assignment_params().row_vars,
            &field_config,
        )
        .unwrap();
        let dense_relation = evaluate_flat_linear_collapse_dense(
            &prepared,
            &relation,
            &constraint_weights,
            &assignment_weights,
        )
        .unwrap();
        let optimized_relation = evaluate_repeated_flat_linear_collapse(
            &prepared,
            &relation,
            &constraint_point,
            &assignment_point,
        )
        .unwrap();
        assert_eq!(optimized_relation, dense_relation);

        let statements = (0..prepared.instances())
            .map(|instance| Sha256CompressionStatement::new(input(instance), [instance as u32; 8]))
            .collect::<Vec<_>>();
        let instance_point =
            public_instance_point(&constraint_point, prepared.instances()).unwrap();
        let slot_weights = (0..SHA256_PUBLIC_WORDS * SHA256_PUBLIC_WORD_BITS)
            .map(|slot| SpartanF2zField::from_with_cfg(slot as u64 + 71, &field_config))
            .collect::<Vec<_>>();
        let public_batch_weight = SpartanF2zField::from_with_cfg(83_u64, &field_config);
        let constant_weight = SpartanF2zField::from_with_cfg(89_u64, &field_config);
        let batching = PublicLinearBatching::new(
            &prepared,
            &statements,
            instance_point,
            slot_weights.clone(),
            public_batch_weight.clone(),
            constant_weight.clone(),
            &field_config,
        )
        .unwrap();
        assert_eq!(
            batching.evaluate(&assignment_point, &field_config).unwrap(),
            batching
                .evaluate_dense(&assignment_weights, &field_config)
                .unwrap()
        );

        let instance_weights = eq_table(instance_point, &field_config).unwrap();
        let mut dense_initial_claim = constant_weight;
        for (instance, statement) in statements.iter().enumerate() {
            for (word_slot, word) in statement.words().enumerate() {
                for bit in 0..SHA256_PUBLIC_WORD_BITS {
                    if word >> bit & 1 == 1 {
                        let slot = word_slot * SHA256_PUBLIC_WORD_BITS + bit;
                        let coefficient = instance_weights[instance].clone()
                            * &public_batch_weight
                            * &slot_weights[slot];
                        dense_initial_claim += &coefficient;
                    }
                }
            }
        }
        assert_eq!(*batching.initial_claim(), dense_initial_claim);
    }

    #[test]
    fn factored_opening_matches_the_dense_scaled_equality() {
        let prepared = super::super::constraints::prepare_sha256_compression_batch_for_test(0)
            .expect("one-instance test relation");
        let field_config = F128::make_cfg(&Uint::from(FQ_MOD)).expect("fixed test field");
        let h_layout = prepared.assignment_params();
        let assignment_point = (0..h_layout.row_vars + h_layout.col_vars)
            .map(|coordinate| {
                SpartanF2zField::from_with_cfg((coordinate as u64) + 3, &field_config)
            })
            .collect::<Vec<_>>();
        let dense_equality = eq_table(&assignment_point, &field_config).unwrap();
        let factored =
            FactoredEqualityWeights::new(&assignment_point, h_layout.row_vars, &field_config)
                .unwrap();
        let collapsed = SpartanF2zField::from_with_cfg(17_u64, &field_config);
        let initial_claim = SpartanF2zField::from_with_cfg(31_u64, &field_config);
        let (rows, columns, claimed) =
            linear_opening_claim(&prepared, h_layout, factored, &collapsed, initial_claim.clone())
                .unwrap();

        for (flat_cell, equality) in dense_equality.iter().enumerate() {
            let row = flat_cell & (h_layout.rows() - 1);
            let column = flat_cell >> h_layout.row_vars;
            let row_weight = field_from_raw(rows[row], &field_config);
            let column_weight = SpartanF2zField::from_with_cfg(columns[column], &field_config);
            assert_eq!(row_weight * &column_weight, equality.clone() * &collapsed);
        }
        assert_eq!(claimed, initial_claim.canonical_u128());
    }
}
