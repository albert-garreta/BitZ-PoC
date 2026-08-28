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
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual, sha_lig_configs,
        validate_ligerito_commitment, verify_mle_eval_mod_q_ligerito_virtual, FlockCommitHint,
        FlockRsError,
    },
    pcs::{Fq, IntEvalParams, ProjectCanonicalU128, FQ_BITS, FQ_MOD},
    transcript::traits::Transcript,
};

use super::super::{
    absorb_spartan_message,
    f2z::{f2z_generator, hash_code, profile_code, spartan_f2z_field_config, SpartanF2zField},
    make_equality_factors,
    matrix::eq_table,
    squeeze_field,
    sumcheck::{
        prove_outer_sumcheck_with_reducer, OptimizedSumcheckReducer, OuterSumcheckProof,
        R1csProductMles,
    },
    PreparedConstraintMatrices, SpartanError, SpartanF2zProof, SpartanField, Virtualized,
};

use super::constraints::{SHA256_CONSTRAINT_LOCAL_VARS, SHA256_F_LOCAL_VARS, SHA256_H_LOCAL_VARS};

const SHA256_SPARTAN_DOMAIN: &[u8] = b"f2z/spartan-sha256-compressions/v1";
const SHA256_ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-sha256-assignment/v1";
const SHA256_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-sha256-opening/v1";

/// Outer Spartan reduction paired with a virtual opening of the complete
/// factorized assignment functional.
pub type Sha256CompressionProof = SpartanF2zProof<OuterSumcheckProof<SpartanF2zField>, Virtualized>;

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

    /// Ligerito configuration derivation failed.
    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    /// Spartan's outer reduction rejected.
    #[error(transparent)]
    Spartan(#[from] SpartanError),

    /// The F2Z opening rejected.
    #[error("the SHA-256 virtual opening rejected: {0:?}")]
    F2z(FlockRsError),
}

/// Derives the audited Ligerito configuration pair for a SHA batch.
pub fn sha256_compression_configs(
    p_f: &IntEvalParams,
) -> Result<(LigProverConfig, LigVerifierConfig), Sha256F2zError> {
    validate_source_params(p_f)?;
    sha_lig_configs(packed_vars(p_f)).map_err(Sha256F2zError::LigeritoConfig)
}

/// Commits packed extended-source rows `[1 | f]` under an explicit config.
pub fn commit_sha256_compression_witness_with_config(
    p_f: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, Sha256F2zError> {
    validate_source_params(p_f)?;
    validate_rows(p_f, &rows)?;
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

/// Proves the repeated SHA-256 R1CS and opens its factorized assignment claim
/// against the commitment to `[1 | f]`.
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_spartan_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    h_rows: &[Vec<u64>],
    products: R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    validate_prover_inputs(matrices, map, p_h, p_f, h_rows, &products, hint_f, pc)?;
    let assignment_binding = assignment_binding(matrices, map, p_h, p_f, &hint_f.commitment)?;
    absorb_sha256_statement(transcript, matrices, p_h, &assignment_binding);

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
    let (row_weights_q, col_weights, claimed) = {
        let _scope = crate::utils::prof::scope("sha256-f2z:opening_prepare_prover");
        factorized_opening_claim(
            matrices,
            p_h,
            &outer.eval_points,
            &rho,
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
#[allow(clippy::too_many_arguments)]
pub fn prove_sha256_compressions_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    h_rows: &[Vec<u64>],
    products: R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
) -> Result<Sha256CompressionProof, Sha256F2zError> {
    let (pc, _) = sha256_compression_configs(p_f)?;
    prove_sha256_compressions_spartan_and_f2z_with_config(
        transcript, matrices, map, p_h, p_f, h_rows, products, hint_f, &pc,
    )
}

/// Verifies the repeated SHA-256 proof under an explicit Ligerito config.
#[allow(clippy::too_many_arguments)]
pub fn verify_sha256_compressions_spartan_and_f2z_with_config<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    validate_verifier_inputs(matrices, map, p_h, p_f, commitment_f, proof, vc)?;
    let assignment_binding = assignment_binding(matrices, map, p_h, p_f, commitment_f)?;
    absorb_sha256_statement(transcript, matrices, p_h, &assignment_binding);

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
    let claimed = batched_product_claim(
        &outer.az_mle_claim,
        &outer.bz_mle_claim,
        &outer.cz_mle_claim,
        &rho,
    );
    let (row_weights_q, col_weights, claimed) = {
        let _scope = crate::utils::prof::scope("sha256-f2z:opening_prepare_verifier");
        factorized_opening_claim(matrices, p_h, &outer.eval_points, &rho, claimed)?
    };
    absorb_opening_claim(
        transcript,
        &assignment_binding,
        &outer.eval_points,
        &rho,
        &row_weights_q,
        &col_weights,
        claimed,
    )?;

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
            &col_weights,
            f2z_generator(),
            claimed,
            FQ_BITS,
            vc,
        )
    };
    result.map_err(Sha256F2zError::F2z)
}

/// Verifies using the production configuration derived from `p_f`.
#[allow(clippy::too_many_arguments)]
pub fn verify_sha256_compressions_spartan_and_f2z<T: Transcript + Send>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    map: &RepeatedVirtualMap,
    p_h: &IntEvalParams,
    p_f: &IntEvalParams,
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
    claimed: SpartanF2zField,
) -> Result<(Vec<u128>, Vec<Fq>, Fq), Sha256F2zError> {
    let expected = SHA256_CONSTRAINT_LOCAL_VARS + p_h.t;
    if row_point.len() != expected {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let (local_point, instance_point) = row_point.split_at(SHA256_CONSTRAINT_LOCAL_VARS);
    let row_weights_q = eq_table(instance_point, matrices.config())
        .map_err(SpartanError::from)?
        .into_iter()
        .map(|value| value.canonical_u128())
        .collect::<Vec<_>>();
    let local_columns = matrices
        .bind_and_batch(local_point, rho)
        .map_err(SpartanError::from)?;
    let col_weights = local_columns
        .evaluations
        .into_iter()
        .map(|value| Fq(value.canonical_u128()))
        .collect::<Vec<_>>();
    Ok((row_weights_q, col_weights, Fq(claimed.canonical_u128())))
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
    if p_f.word_bits != 1
        || p_f.s != SHA256_F_LOCAL_VARS
        || p_f.t < LOG_PACKING
        || p_f.t.saturating_add(p_f.s) > 126
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
    validate_source_params(p_f)?;
    if p_h.word_bits != 1
        || p_h.t != p_f.t
        || p_h.s != SHA256_H_LOCAL_VARS
        || matrices.num_row_vars() != SHA256_CONSTRAINT_LOCAL_VARS
        || matrices.num_column_vars() != SHA256_H_LOCAL_VARS
        || map.instances() != p_h.rows()
        || map.rows() != cell_count(p_h)
        || map.cols() != cell_count(p_f)
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    let expected = SpartanF2zField::canonical_modulus_encoding(&spartan_f2z_field_config());
    if matrices.field_modulus_encoding() != expected {
        return Err(Sha256F2zError::UnsupportedFieldModulus);
    }
    Ok(())
}

fn validate_rows(p: &IntEvalParams, rows: &[Vec<u64>]) -> Result<(), Sha256F2zError> {
    let words = p.rows().div_ceil(64);
    if rows.len() != p.cols() || rows.iter().any(|row| row.len() != words) {
        return Err(Sha256F2zError::InvalidGeometry);
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
    h_rows: &[Vec<u64>],
    products: &R1csProductMles<SpartanF2zField>,
    hint_f: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<(), Sha256F2zError> {
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
    commitment_f: &Commitment,
    proof: &Sha256CompressionProof,
    vc: &LigVerifierConfig,
) -> Result<(), Sha256F2zError> {
    validate_common(matrices, map, p_h, p_f)?;
    validate_ligerito_commitment(commitment_f, vc).map_err(Sha256F2zError::F2z)?;
    if commitment_f.params.m != p_f.t + p_f.s
        || proof.spartan().sumcheck.round_polynomials.len() != SHA256_CONSTRAINT_LOCAL_VARS + p_h.t
    {
        return Err(Sha256F2zError::InvalidGeometry);
    }
    Ok(())
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

fn absorb_sha256_statement(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<SpartanF2zField>,
    p_h: &IntEvalParams,
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
    absorb_spartan_message(transcript, b"assignment-oracle", assignment_binding);
}

#[allow(clippy::too_many_arguments)]
fn absorb_opening_claim(
    transcript: &mut impl Transcript,
    assignment_binding: &[u8; 32],
    row_point: &[SpartanF2zField],
    rho: &SpartanF2zField,
    row_weights_q: &[u128],
    col_weights: &[Fq],
    claimed: Fq,
) -> Result<(), Sha256F2zError> {
    let mut hash = Hasher::new();
    hash.update(SHA256_OPENING_CLAIM_DOMAIN);
    hash.update(assignment_binding);
    hash_usize(&mut hash, row_point.len())?;
    for coordinate in row_point {
        hash.update(&coordinate.canonical_element_encoding());
    }
    hash.update(&rho.canonical_element_encoding());
    hash_usize(&mut hash, row_weights_q.len())?;
    for weight in row_weights_q {
        hash.update(&weight.to_le_bytes());
    }
    hash_usize(&mut hash, col_weights.len())?;
    for weight in col_weights {
        hash.update(&weight.0.to_le_bytes());
    }
    hash.update(&claimed.0.to_le_bytes());
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
        piop::spartan::sha256::{
            generate_sha256_compression_witnesses, prepare_sha256_compression_batch,
        },
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

    #[test]
    fn sha256_virtualized_spartan_f2z_roundtrip() {
        const LOG_COMPRESSIONS: usize = 7;
        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(LOG_COMPRESSIONS).unwrap();
        let inputs = (0..1usize << LOG_COMPRESSIONS)
            .map(input)
            .collect::<Vec<_>>();
        let (f_rows, h_rows, products, _) =
            generate_sha256_compression_witnesses(&inputs, &p_f, &p_h, matrices.config()).unwrap();
        let (pc, vc) = sha256_compression_configs(&p_f).unwrap();
        let hint = commit_sha256_compression_witness_with_config(&p_f, f_rows, &pc).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
            &mut prover_transcript,
            &matrices,
            &map,
            &p_h,
            &p_f,
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
            &hint.commitment,
            &inconsistent_proof,
            &vc,
        )
        .is_err());
    }
}
