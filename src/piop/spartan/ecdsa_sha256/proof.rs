use crypto_primitives::{PrimeField, crypto_bigint_uint::Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig, VerifierConfig},
};

use super::{
    Config, Result, error,
    inner_reduction::{InnerSumcheckClaim, ModQCoefficients},
    relation::{OuterMode, PreparedSha256Ecdsa, Sha256EcdsaStatement},
    security::Sha256EcdsaSecurity,
    witness::Sha256EcdsaWitness,
};
use crate::{
    ext_proj::sample_prime_in_interval,
    f2map::VirtualMap,
    ligerito::packed_vars,
    ligerito_flock::{
        FlockCommitHint, IntEvalRsLigVirtProof,
        atomic::{AtomicNonces, AtomicSecurity},
        commit_rs_ligerito_rows,
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus_with_security,
        validate_ligerito_commitment,
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off_with_security,
    },
    pcs::{ModQWeightChunks, ProjectCanonicalU128},
    piop::spartan::{
        SpartanField, absorb_spartan_message,
        f2z::{SpartanF2zField as F, f2z_generator},
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::{eq_table, make_equality_factors},
        sha256::inner_sumcheck::{prove_composite_inner_sumcheck, verify_sha256_inner_sumcheck},
        squeeze_field,
        sumcheck::{
            OptimizedSumcheckReducer, OuterSumcheckProof, SumcheckProof,
            prove_outer_sumcheck_with_reducer_grinded,
        },
    },
    transcript::traits::Transcript,
};

enum OuterGrinding {}
impl GrindingDomain for OuterGrinding {
    const DOMAIN: &'static [u8] = b"f2z/sha256-ecdsa/outer/v1";
}

/// A single source commitment supports both PIOP reductions and the final opening.
#[derive(Clone)]
pub struct Sha256EcdsaProof {
    pub(crate) initial_nonce: u64,
    pub(crate) batch_nonce: u64,
    pub(crate) flock_nonces: Vec<u64>,
    pub(crate) outer: OuterSumcheckProof<F>,
    pub(crate) outer_nonces: Vec<u64>,
    pub(crate) inner: SumcheckProof<F, 3>,
    pub(crate) inner_nonces: Vec<u64>,
    pub(crate) opening: IntEvalRsLigVirtProof,
}

fn configs(prepared: &PreparedSha256Ecdsa) -> Result<(ProverConfig, VerifierConfig)> {
    Ok((
        prepared.ligerito.prover().clone(),
        prepared.ligerito.verifier().clone(),
    ))
}

pub fn commit_sha256_ecdsa(
    prepared: &PreparedSha256Ecdsa,
    witness: &Sha256EcdsaWitness,
) -> Result<FlockCommitHint> {
    let (pc, _) = configs(prepared)?;
    if witness.statement.log_compressions as usize != prepared.log_n {
        return Err(error("witness layout mismatch"));
    }
    Ok(commit_rs_ligerito_rows(
        &prepared.f_layout,
        witness.f_rows.clone(),
        &pc,
    ))
}

fn bind_statement<T: Transcript>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    security: &Sha256EcdsaSecurity,
) -> Result<()> {
    if statement.log_compressions as usize != prepared.log_n {
        return Err(error("statement layout mismatch"));
    }
    absorb_spartan_message(t, b"protocol", b"f2z/sha256-ecdsa/split-inner/early-ood/v2");
    absorb_spartan_message(t, b"relation", &prepared.local.digest);
    absorb_spartan_message(t, b"map", &prepared.map.digest());
    absorb_spartan_message(t, b"statement", &statement.bytes());
    absorb_spartan_message(
        t,
        b"mode",
        &[match prepared.mode {
            OuterMode::Split => 0,
            OuterMode::AllRows => 1,
        }],
    );
    absorb_spartan_message(t, b"security-target", &prepared.lambda.to_le_bytes());
    for block in &security.blocks {
        absorb_spartan_message(t, b"challenge-block", block.label.as_bytes());
        absorb_spartan_message(t, b"block-work", &block.grinding_bits.to_le_bytes());
        absorb_spartan_message(
            t,
            b"block-count",
            &(block.multiplicity_bound as u64).to_le_bytes(),
        );
    }
    absorb_spartan_message(
        t,
        b"commitment",
        &bincode::serialize(commitment).map_err(error)?,
    );
    prepared.ligerito.bind(t);
    Ok(())
}

struct InitialSpartanChallenges {
    modulus: u128,
    field_config: Config,
    outer_eq_challenges: Vec<F>,
    grinding_nonce: u64,
}

/// Prove or verify initial grinding, then derive the field and outer equality challenges.
fn derive_initial_challenges<T: Transcript>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    security: &Sha256EcdsaSecurity,
    nonce: Option<u64>,
) -> Result<InitialSpartanChallenges> {
    let grinding_nonce = boundary::<InitialGrinding, _>(t, security.initial, nonce)?;
    let modulus = sample_prime_in_interval(t, 1u128 << 112, (1u128 << 113) - 1).map_err(error)?;
    absorb_spartan_message(t, b"q", &modulus.to_le_bytes());
    let field_config =
        F::make_cfg(&Uint::from(modulus)).map_err(|_| error("invalid sampled modulus"))?;
    let outer_eq_challenges = (0..prepared.outer_sumcheck_num_vars())
        .map(|_| squeeze_field(t, &field_config))
        .collect();
    Ok(InitialSpartanChallenges {
        modulus,
        field_config,
        outer_eq_challenges,
        grinding_nonce,
    })
}

fn sample_inner_batch_challenges<T: Transcript>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    cfg: &Config,
) -> (F, Vec<F>, F) {
    // Sample only after the outer terminal triple has been absorbed.
    absorb_spartan_message(t, b"shared-inner", b"rho-sigma-gamma/v1");
    let matrix_batch_challenge = squeeze_field(t, cfg);
    let linear_row_point = (0..prepared.linear_vars())
        .map(|_| squeeze_field(t, cfg))
        .collect();
    let linear_batch_weight = squeeze_field(t, cfg);
    (
        matrix_batch_challenge,
        linear_row_point,
        linear_batch_weight,
    )
}

fn bind_opening<T: Transcript>(t: &mut T, point: &[F], scale: &F, value: &F) {
    absorb_spartan_message(t, b"scaled-assignment-claim", b"v1");
    for x in point.iter().chain([scale, value]) {
        absorb_spartan_message(t, b"field", &x.canonical_element_encoding());
    }
}

/// Proves one standard SHA-256 message followed by P-256 ECDSA verification.
/// `prefix_vars` (0..=4) changes only the packed inner prover's implementation.
pub fn prove_sha256_ecdsa<T: Transcript + Send>(
    t: &mut T,
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    witness: &Sha256EcdsaWitness,
    hint: &FlockCommitHint,
    prefix_vars: usize,
) -> Result<Sha256EcdsaProof> {
    let (pc, _) = configs(prepared)?;
    if &witness.statement != statement || hint.rows() != witness.f_rows {
        return Err(error("statement or commitment witness mismatch"));
    }
    if prefix_vars > 4 {
        return Err(error("inner prefix must be in 0..=4"));
    }
    validate_ligerito_commitment(&hint.commitment, &pc).map_err(|e| error(format!("{e:?}")))?;
    let security = prepared.security()?;
    bind_statement(t, prepared, statement, &hint.commitment, &security)?;
    let ood = crate::ligerito_flock::bind_prover_ood(t, hint, security.ood);
    let InitialSpartanChallenges {
        modulus,
        field_config: cfg,
        outer_eq_challenges,
        grinding_nonce: initial_nonce,
    } = derive_initial_challenges(t, prepared, &security, None)?;
    let reducer = OptimizedSumcheckReducer::new(&cfg).map_err(error)?;
    let mod_q_coefficients = ModQCoefficients::from_relation(prepared, modulus, &cfg);
    let (outer, outer_nonces) = {
        let _scope = crate::utils::prof::scope("ecdsa:outer_prove");
        let products = witness.build_outer_product_mles(prepared, modulus, &cfg);
        prove_outer_sumcheck_with_reducer_grinded::<OuterGrinding, _, _>(
            t,
            F::zero_with_cfg(&cfg),
            &outer_eq_challenges,
            make_equality_factors(&outer_eq_challenges, &cfg).map_err(error)?,
            products,
            &cfg,
            &reducer,
            security.outer,
        )
        .map_err(error)?
    };
    let batch_nonce = boundary::<BatchGrinding, _>(t, security.batch, None)?;
    let (matrix_batch_challenge, linear_row_point, linear_batch_weight) =
        sample_inner_batch_challenges(t, prepared, &cfg);
    let inner_claim = InnerSumcheckClaim::from_outer_claims(
        prepared,
        statement,
        &outer.proof,
        outer.eval_points,
        matrix_batch_challenge,
        linear_row_point,
        linear_batch_weight,
        &cfg,
    )?;
    let batched_matrix_mle =
        mod_q_coefficients.build_batched_matrix_mle(prepared, &inner_claim, &cfg)?;
    let inner = {
        let _scope = crate::utils::prof::scope("ecdsa:shared_inner_prove");
        prove_composite_inner_sumcheck(
            t,
            inner_claim.claimed_sum().clone(),
            prepared.h_layout.row_vars + prepared.h_layout.col_vars,
            &batched_matrix_mle.as_mle(&cfg)?,
            &|i| Ok(witness.h_bit(i, &prepared.h_layout)),
            prefix_vars,
            &cfg,
            &reducer,
            security.inner,
        )
        .map_err(error)?
    };
    if batched_matrix_mle.evaluate(&inner.eval_points, &cfg)? != inner.v_evaluation {
        return Err(error("batched matrix MLE evaluation mismatch"));
    }
    if inner.v_evaluation.clone() * &inner.h_evaluation != inner.final_claim {
        return Err(error("witness does not satisfy the shared inner claim"));
    }
    bind_opening(
        t,
        &inner.eval_points,
        &inner.v_evaluation,
        &inner.final_claim,
    );
    let rows: Vec<_> = eq_table(&inner.eval_points[..prepared.h_layout.row_vars], &cfg)
        .map_err(error)?
        .into_iter()
        .map(|mut x| {
            x *= &inner.v_evaluation;
            x.canonical_u128()
        })
        .collect();
    let mut flock_nonces = Vec::new();
    let chunks = ModQWeightChunks::from_dense(&prepared.h_layout, &rows, 113)
        .map_err(|_| error("invalid row weights"))?;
    let mut atomic = AtomicSecurity {
        plan: &security.flock,
        nonces: AtomicNonces::Prove(&mut flock_nonces),
    };
    let opening = {
        let _scope = crate::utils::prof::scope("ecdsa:f2z_prove");
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus_with_security(
            t,
            hint,
            &witness.h_rows,
            &prepared.h_layout,
            &prepared.f_layout,
            &prepared.map,
            &chunks,
            modulus,
            113,
            f2z_generator(),
            security.forest,
            ood,
            &pc,
            Some(&mut atomic),
        )
    };
    Ok(Sha256EcdsaProof {
        initial_nonce,
        batch_nonce,
        flock_nonces,
        outer: outer.proof,
        outer_nonces,
        inner: inner.sumcheck_proof,
        inner_nonces: inner.round_nonces,
        opening,
    })
}

pub fn verify_sha256_ecdsa<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    proof: &Sha256EcdsaProof,
) -> Result<()> {
    let (_, vc) = configs(prepared)?;
    validate_ligerito_commitment(commitment, &vc).map_err(|e| error(format!("{e:?}")))?;
    let security = prepared.security()?;
    bind_statement(transcript, prepared, statement, commitment, &security)?;
    let ood = crate::ligerito_flock::bind_verifier_ood(
        transcript,
        packed_vars(&prepared.f_layout),
        security.ood,
        proof.opening.ood.as_ref(),
    )
    .map_err(|e| error(format!("{e:?}")))?;
    let InitialSpartanChallenges {
        modulus,
        field_config: cfg,
        outer_eq_challenges,
        grinding_nonce: _,
    } = derive_initial_challenges(transcript, prepared, &security, Some(proof.initial_nonce))?;
    let outer_row_point = proof
        .outer
        .verify_grinded::<OuterGrinding>(
            transcript,
            F::zero_with_cfg(&cfg),
            &outer_eq_challenges,
            &cfg,
            &proof.outer_nonces,
            security.outer,
        )
        .map_err(error)?
        .eval_points;
    boundary::<BatchGrinding, _>(transcript, security.batch, Some(proof.batch_nonce))?;
    let (matrix_batch_challenge, linear_row_point, linear_batch_weight) =
        sample_inner_batch_challenges(transcript, prepared, &cfg);
    let mod_q_coefficients = ModQCoefficients::from_relation(prepared, modulus, &cfg);
    let inner_claim = InnerSumcheckClaim::from_outer_claims(
        prepared,
        statement,
        &proof.outer,
        outer_row_point,
        matrix_batch_challenge,
        linear_row_point,
        linear_batch_weight,
        &cfg,
    )?;
    let (inner_eval_point, inner_final_claim) = verify_sha256_inner_sumcheck(
        transcript,
        inner_claim.claimed_sum().clone(),
        &proof.inner,
        &proof.inner_nonces,
        prepared.h_layout.row_vars + prepared.h_layout.col_vars,
        &cfg,
        security.inner,
    )
    .map_err(error)?;
    // inner_final_claim ≡ scale · h(inner_eval_point) (mod q).
    let scale = mod_q_coefficients.evaluate_batched_matrix_mle(
        prepared,
        &inner_claim,
        &inner_eval_point,
        &cfg,
    )?;
    bind_opening(transcript, &inner_eval_point, &scale, &inner_final_claim);
    // rows[b] = (scale · eq(b, inner_eval_point[..row_vars])) mod q ∈ [0, q).
    let rows: Vec<_> = eq_table(&inner_eval_point[..prepared.h_layout.row_vars], &cfg)
        .map_err(error)?
        .into_iter()
        .map(|mut x| {
            x *= &scale;
            x.canonical_u128()
        })
        .collect();
    // cols[c] = eq(c, inner_eval_point[row_vars..]) mod q ∈ [0, q).
    // Σ_{b,c} rows[b] · h[b,c] · cols[c] ≡ inner_final_claim (mod q).
    let cols: Vec<_> = eq_table(&inner_eval_point[prepared.h_layout.row_vars..], &cfg)
        .map_err(error)?
        .iter()
        .map(ProjectCanonicalU128::canonical_u128)
        .collect();
    // R = 2^row_vars, C = 2^col_vars; h: R × C.
    // w = 127 - row_vars - 1 ≥ 113 ⇒ L = ⌈113 / w⌉ = 1.
    // chunks: L × R; folds = chunks · h: L × C.
    // folds[ℓ][c] = Σ_b chunks[ℓ][b] · h[b,c].
    // chunks[0][b] = rows[b].
    let chunks = ModQWeightChunks::from_dense(&prepared.h_layout, &rows, 113)
        .map_err(|_| error("invalid row weights"))?;
    let mut atomic = AtomicSecurity {
        plan: &security.flock,
        nonces: AtomicNonces::Verify {
            values: &proof.flock_nonces,
            cursor: 0,
        },
    };
    let arithmetic = crate::ext_proj::ProjArith::new(modulus);
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off_with_security(
        transcript,
        commitment,
        &proof.opening,
        &prepared.h_layout,
        &prepared.f_layout,
        &prepared.map,
        &chunks,
        modulus,
        113,
        f2z_generator(),
        security.forest,
        ood,
        &vc,
        cols.len(),
        |values, width, count| {
            // This profile has one 113-bit chunk; the F2Z preflight enforces the
            // fold magnitudes before this canonical mod-q read-off is accepted.
            if count != 1 || values.len() < cols.len() || width < 113 {
                return false;
            }
            let sum = cols.iter().zip(values).fold(0, |sum, (&c, &v)| {
                arithmetic.add(sum, arithmetic.mul(c, arithmetic.reduce(v)))
            });
            sum == inner_final_claim.canonical_u128()
        },
        Some(&mut atomic),
    )
    .map_err(|e| error(format!("{e:?}")))
}

enum InitialGrinding {}
impl GrindingDomain for InitialGrinding {
    const DOMAIN: &'static [u8] = b"f2z/sha256-ecdsa/initial/v1";
}
enum BatchGrinding {}
impl GrindingDomain for BatchGrinding {
    const DOMAIN: &'static [u8] = b"f2z/sha256-ecdsa/batch/v1";
}
fn boundary<D: GrindingDomain, T: Transcript>(
    t: &mut T,
    bits: u32,
    nonce: Option<u64>,
) -> Result<u64> {
    let _scope = crate::utils::prof::scope(if nonce.is_some() {
        "ecdsa:boundary_grinding_verify"
    } else {
        "ecdsa:boundary_grinding_prove"
    });
    if bits == 0 {
        if nonce.is_some_and(|n| n != 0) {
            return Err(error("noncanonical zero-work nonce"));
        }
        return Ok(0);
    }
    match nonce {
        Some(n) => {
            verify_and_absorb::<D, _>(t, GrindingRound::new(0), bits, n).map_err(error)?;
            Ok(n)
        }
        None => grind_and_absorb::<D, _>(t, GrindingRound::new(0), bits).map_err(error),
    }
}
