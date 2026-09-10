use crypto_primitives::{PrimeField, crypto_bigint_uint::Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig, VerifierConfig},
};

use super::{
    Result, error,
    reduction::{Config, Projection, outer_vars},
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

fn configs(p: &PreparedSha256Ecdsa) -> Result<(ProverConfig, VerifierConfig)> {
    Ok((p.ligerito.prover().clone(), p.ligerito.verifier().clone()))
}

pub fn commit_sha256_ecdsa(
    p: &PreparedSha256Ecdsa,
    witness: &Sha256EcdsaWitness,
) -> Result<FlockCommitHint> {
    let (pc, _) = configs(p)?;
    if witness.statement.log_compressions as usize != p.log_n {
        return Err(error("witness layout mismatch"));
    }
    Ok(commit_rs_ligerito_rows(&p.p_f, witness.f_rows.clone(), &pc))
}

fn bind_statement<T: Transcript>(
    t: &mut T,
    p: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    security: &Sha256EcdsaSecurity,
) -> Result<()> {
    if statement.log_compressions as usize != p.log_n {
        return Err(error("statement layout mismatch"));
    }
    absorb_spartan_message(t, b"protocol", b"f2z/sha256-ecdsa/split-inner/early-ood/v2");
    absorb_spartan_message(t, b"relation", &p.local.digest);
    absorb_spartan_message(t, b"map", &p.map.digest());
    absorb_spartan_message(t, b"statement", &statement.bytes());
    absorb_spartan_message(
        t,
        b"mode",
        &[match p.mode {
            OuterMode::Split => 0,
            OuterMode::AllRows => 1,
        }],
    );
    absorb_spartan_message(t, b"security-target", &p.lambda.to_le_bytes());
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
    p.ligerito.bind(t);
    Ok(())
}

fn start<T: Transcript>(t: &mut T, p: &PreparedSha256Ecdsa, security: &Sha256EcdsaSecurity, nonce: Option<u64>) -> Result<(u128, Config, Vec<F>, u64)> {
    let nonce = boundary::<InitialGrinding, _>(t, security.initial, nonce)?;
    let q = sample_prime_in_interval(t, 1u128 << 112, (1u128 << 113) - 1).map_err(error)?;
    absorb_spartan_message(t, b"q", &q.to_le_bytes());
    let cfg = F::make_cfg(&Uint::from(q)).map_err(|_| error("invalid sampled modulus"))?;
    let tau = (0..outer_vars(p)).map(|_| squeeze_field(t, &cfg)).collect();
    Ok((q, cfg, tau, nonce))
}

fn challenges<T: Transcript>(t: &mut T, p: &PreparedSha256Ecdsa, cfg: &Config) -> (F, Vec<F>, F) {
    // The outer terminal triple has already been absorbed. In particular,
    // gamma must not be chosen before the prover fixes those three claims.
    absorb_spartan_message(t, b"shared-inner", b"rho-sigma-gamma/v1");
    let rho = squeeze_field(t, cfg);
    let sigma = (0..p.linear_vars())
        .map(|_| squeeze_field(t, cfg))
        .collect();
    let gamma = squeeze_field(t, cfg);
    (rho, sigma, gamma)
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
    p: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    witness: &Sha256EcdsaWitness,
    hint: &FlockCommitHint,
    prefix_vars: usize,
) -> Result<Sha256EcdsaProof> {
    let (pc, _) = configs(p)?;
    if &witness.statement != statement || hint.rows() != witness.f_rows {
        return Err(error("statement or commitment witness mismatch"));
    }
    if prefix_vars > 4 {
        return Err(error("inner prefix must be in 0..=4"));
    }
    validate_ligerito_commitment(&hint.commitment, &pc).map_err(|e| error(format!("{e:?}")))?;
    let security = p.security()?;
    bind_statement(t, p, statement, &hint.commitment, &security)?;
    let ood = crate::ligerito_flock::bind_prover_ood(t, hint, security.ood);
    let (q, cfg, tau, initial_nonce) = start(t, p, &security, None)?;
    let reducer = OptimizedSumcheckReducer::new(&cfg).map_err(error)?;
    let projection = Projection::new(p, q, &cfg);
    let (outer, outer_nonces) = {
        let _scope = crate::utils::prof::scope("ecdsa:outer_prove");
        let products = projection.outer_products(p, witness, q, &cfg);
        prove_outer_sumcheck_with_reducer_grinded::<OuterGrinding, _, _>(
            t,
            F::zero_with_cfg(&cfg),
            &tau,
            make_equality_factors(&tau, &cfg).map_err(error)?,
            products,
            &cfg,
            &reducer,
            security.outer,
        )
        .map_err(error)?
    };
    let batch_nonce = boundary::<BatchGrinding, _>(t, security.batch, None)?;
    let (rho, sigma, gamma) = challenges(t, p, &cfg);
    let coefficients = projection.combine(
        p,
        statement,
        &outer.proof,
        &outer.eval_points,
        &rho,
        &sigma,
        &gamma,
        &cfg,
    )?;
    let inner = {
        let _scope = crate::utils::prof::scope("ecdsa:shared_inner_prove");
        prove_composite_inner_sumcheck(
            t,
            coefficients.target.clone(),
            p.p_h.t + p.p_h.s,
            &coefficients.inner_source(&cfg)?,
            &|i| Ok(witness.h_bit(i, &p.p_h)),
            prefix_vars,
            &cfg,
            &reducer,
            security.inner,
        )
        .map_err(error)?
    };
    if coefficients.evaluate(&inner.eval_points, &cfg)? != inner.v_evaluation {
        return Err(error("inner coefficient evaluation mismatch"));
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
    let rows: Vec<_> = eq_table(&inner.eval_points[..p.p_h.t], &cfg)
        .map_err(error)?
        .into_iter()
        .map(|mut x| {
            x *= &inner.v_evaluation;
            x.canonical_u128()
        })
        .collect();
    let mut flock_nonces = Vec::new();
    let chunks = ModQWeightChunks::from_dense(&p.p_h, &rows, 113)
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
            &p.p_h,
            &p.p_f,
            &p.map,
            &chunks,
            q,
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
    t: &mut T,
    p: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    proof: &Sha256EcdsaProof,
) -> Result<()> {
    let (_, vc) = configs(p)?;
    validate_ligerito_commitment(commitment, &vc).map_err(|e| error(format!("{e:?}")))?;
    let security = p.security()?;
    bind_statement(t, p, statement, commitment, &security)?;
    let ood = crate::ligerito_flock::bind_verifier_ood(t, packed_vars(&p.p_f), security.ood, proof.opening.ood.as_ref())
        .map_err(|e| error(format!("{e:?}")))?;
    let (q, cfg, tau, _) = start(t, p, &security, Some(proof.initial_nonce))?;
    let rx = proof
        .outer
        .verify_grinded::<OuterGrinding>(
            t,
            F::zero_with_cfg(&cfg),
            &tau,
            &cfg,
            &proof.outer_nonces,
            security.outer,
        )
        .map_err(error)?
        .eval_points;
    boundary::<BatchGrinding, _>(t, security.batch, Some(proof.batch_nonce))?;
    let (rho, sigma, gamma) = challenges(t, p, &cfg);
    let coefficients = Projection::new(p, q, &cfg).combine(
        p,
        statement,
        &proof.outer,
        &rx,
        &rho,
        &sigma,
        &gamma,
        &cfg,
    )?;
    let (point, value) = verify_sha256_inner_sumcheck(
        t,
        coefficients.target.clone(),
        &proof.inner,
        &proof.inner_nonces,
        p.p_h.t + p.p_h.s,
        &cfg,
        security.inner,
    )
    .map_err(error)?;
    let scale = coefficients.evaluate(&point, &cfg)?;
    bind_opening(t, &point, &scale, &value);
    let rows: Vec<_> = eq_table(&point[..p.p_h.t], &cfg)
        .map_err(error)?
        .into_iter()
        .map(|mut x| {
            x *= &scale;
            x.canonical_u128()
        })
        .collect();
    let cols: Vec<_> = eq_table(&point[p.p_h.t..], &cfg)
        .map_err(error)?
        .iter()
        .map(ProjectCanonicalU128::canonical_u128)
        .collect();
    let chunks = ModQWeightChunks::from_dense(&p.p_h, &rows, 113)
        .map_err(|_| error("invalid row weights"))?;
    let mut atomic = AtomicSecurity {
        plan: &security.flock,
        nonces: AtomicNonces::Verify {
            values: &proof.flock_nonces,
            cursor: 0,
        },
    };
    let arithmetic = crate::ext_proj::ProjArith::new(q);
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off_with_security(
        t,
        commitment,
        &proof.opening,
        &p.p_h,
        &p.p_f,
        &p.map,
        &chunks,
        q,
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
            sum == value.canonical_u128()
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
