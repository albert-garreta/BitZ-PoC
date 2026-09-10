//! Multiplication reduction stopped at its binary inner-product claim.
//!
//! For the hybrid mod-2^32 relation, product = z + 2^32*w is the fixed
//! reconstruction of the two committed 32-bit output limbs. Proving
//! x*y = product with all 64 product bits bound proves the modular relation.
use super::*;
use crate::hybrid::{BinaryClaim, Error};
use crate::ligerito::{fold_values_bits, pack_columns_from_rows, row_bit_vars};
use crate::ligerito_flock::gf_to_f128;
use crate::merged_forest::{MergedForestProof, prove_merged_forest_lazy, verify_merged_forest};
use crate::pcs::{FixedBasePow, chunk_pow2_table, row_bit_weights};
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::transcript::Blake3Transcript;

#[derive(Clone, Debug)]
pub(crate) struct PrefixProof {
    pub initial_nonce: u64,
    pub terminal_nonce: u64,
    pub piop_nonces: Vec<u64>,
    pub spartan: UnivariateSkipSpartanPiopProof<SpartanF2zField>,
    pub sums: Vec<u128>,
    pub forest: MergedForestProof,
}

pub(crate) fn decoding_config(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedU32MulRelation,
    statement: &[u8; 32],
    nonce: u64,
) -> Result<(u128, <SpartanF2zField as PrimeField>::Config), Error> {
    absorb_spartan_message(transcript, b"u32-statement", statement);
    check_boundary_u32::<U32MulInitialGrinding, _>(
        transcript,
        prepared.security().initial_grinding_bits,
        nonce,
    )?;
    let (q, _, config, _) = sample_u32_mul_mod_q(transcript, prepared.security())?;
    Ok((q, config))
}

fn bind_sums(t: &mut Blake3Transcript, digest: &[u8; 32], sums: &[u128]) {
    t.absorb_slice(b"hybrid/u32-bounded-sums/v1");
    t.absorb_slice(digest);
    for sum in sums {
        t.absorb_slice(&sum.to_le_bytes());
    }
}

fn endpoint(p: &crate::pcs::IntEvalParams, weights: &[u128], z: &[Gf], e: Gf) -> BinaryClaim {
    let tw = row_bit_vars(p);
    BinaryClaim {
        low: row_bit_weights(p, weights, f2z_generator(), &z[..tw])
            .into_iter()
            .map(gf_to_f128)
            .collect(),
        high_point: z[tw..].iter().copied().map(gf_to_f128).collect(),
        value: gf_to_f128(e + Gf::one()),
    }
}

pub(crate) fn prove(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedU32MulRelation,
    witness: &U32MulWitness,
    rows: &[Vec<u64>],
    statement: &[u8; 32],
) -> Result<(PrefixProof, BinaryClaim), Error> {
    let piop_scope = crate::utils::prof::scope("hybrid:mul_piop");
    let layout = prepared.layout();
    if witness.layout() != layout {
        return Err(Error::Invalid("multiplication layout"));
    }
    let p = layout.f2z_params();
    validate_bit_rows(&p, rows)?;
    let security = prepared.security();
    let binding = *statement;
    absorb_spartan_message(transcript, b"u32-statement", &binding);
    // Step 2: pre-draw grinding, prime sample, and relation projection into
    // the runtime field.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce =
        grind_boundary_u32::<U32MulInitialGrinding, _>(transcript, security.initial_grinding_bits)?;
    let (q, q_bits, config, arith) = sample_u32_mul_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("spartan-f2z:relation_projection_prove");
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    // Step 3: the native u64 Spartan PIOP over F_q, every drawn challenge
    // preceded by one PIOP grinding boundary at the profile's difficulty
    // (a transparent pass-through at λ = 100).
    let (spartan, terminal_claim, piop_nonces) = {
        let _step3 = crate::utils::prof::scope("step3:piop_prove");
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
        // The exact products are the zero-padded operand blocks of the
        // witness and the assignment is its block table: lend both, no copy.
        let product_len = layout.multiplications().next_power_of_two();
        let products = NativeProducts {
            az: &witness.x_values()[..product_len],
            bz: &witness.y_values()[..product_len],
            cz: &witness.product_values()[..product_len],
        };
        let mut grinder: crate::piop::spartan::grinding::ProverGrindingTranscript<
            _,
            U32MulPiopGrinding,
        > = crate::piop::spartan::grinding::ProverGrindingTranscript::new(
            transcript,
            piop_wrap_bits(security),
        );
        let (spartan, terminal_claim) =
            prove_spartan_piop_u32_native_with_univariate_skip_borrowed(
                &mut grinder,
                &matrices,
                &binding,
                products,
                witness.assignment(),
                U32_MUL_UNIVARIATE_SKIP_VARS,
            )?;
        (spartan, terminal_claim, grinder.finish())
    };

    // Step 4: bitification at the runtime prime, plus the terminal
    // boundary protecting the opening challenges.
    let step4_scope = crate::utils::prof::scope("step4:bitify_prove");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_prover");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    let terminal_nonce = grind_boundary_u32::<U32MulTerminalGrinding, _>(
        transcript,
        security.terminal_grinding_bits,
    )?;
    drop(step4_scope);
    drop(piop_scope);

    let _opening_scope = crate::utils::prof::scope("hybrid:mul_opening");
    let prepared_claim = prepare_u32_bitified_claim(&opening, q_bits, &arith)?;
    let weights = prepared_claim.chunks.chunks();
    if weights.len() != 1 {
        return Err(Error::Invalid("multiple F2Z chunks"));
    }
    let fold_scope = crate::utils::prof::scope("mo:fold_values");
    let sums = fold_values_bits(&p, rows, &weights[0]);
    drop(fold_scope);
    bind_sums(transcript, &bridge_digest, &sums);
    let pack_scope = crate::utils::prof::scope("mo:pack_cols");
    let packed_cols = pack_columns_from_rows(&p, rows);
    drop(pack_scope);
    let pow2_scope = crate::utils::prof::scope("mo:pow2");
    let powers = chunk_pow2_table(&p, &weights[0], f2z_generator());
    drop(pow2_scope);
    let forest_scope = crate::utils::prof::scope("mo:forest");
    let (_, forest, z, e) =
        prove_merged_forest_lazy(transcript, &p, &packed_cols, &powers, p.cols());
    drop(forest_scope);
    let endpoint_scope = crate::utils::prof::scope("mo:endpoint");
    let claim = endpoint(&p, &weights[0], &z, e);
    drop(endpoint_scope);
    Ok((
        PrefixProof {
            initial_nonce,
            terminal_nonce,
            piop_nonces,
            spartan,
            sums,
            forest,
        },
        claim,
    ))
}

pub(crate) fn verify(
    transcript: &mut Blake3Transcript,
    prepared: &PreparedU32MulRelation,
    statement: &[u8; 32],
    proof: &PrefixProof,
) -> Result<BinaryClaim, Error> {
    let layout = prepared.layout();
    let p = layout.f2z_params();
    let security = prepared.security();
    let actual_skip_vars = proof.spartan.outer.skip.skip_vars;
    let expected_skip_vars = U32_MUL_UNIVARIATE_SKIP_VARS as u8;
    if actual_skip_vars != expected_skip_vars {
        return Err(SpartanF2zError::UnexpectedUnivariateSkipVariables {
            expected: expected_skip_vars,
            actual: actual_skip_vars,
        }
        .into());
    }

    let binding = *statement;
    absorb_spartan_message(transcript, b"u32-statement", &binding);

    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    check_boundary_u32::<U32MulInitialGrinding, _>(
        transcript,
        security.initial_grinding_bits,
        proof.initial_nonce,
    )?;
    let (q, q_bits, config, arith) = sample_u32_mul_mod_q(transcript, security)?;
    let matrices = {
        let _scope = crate::utils::prof::scope("spartan-f2z:relation_projection_verify");
        PreparedConstraintMatrices::<SpartanF2zField, bool>::from_skeleton(
            &prepared.skeleton,
            &config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    let terminal_claim = {
        let _step3 = crate::utils::prof::scope("step3:piop_verify");
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_verify");
        let mut grinder: crate::piop::spartan::grinding::VerifierGrindingTranscript<
            _,
            U32MulPiopGrinding,
        > = crate::piop::spartan::grinding::VerifierGrindingTranscript::new(
            transcript,
            piop_wrap_bits(security),
            &proof.piop_nonces,
        );
        let terminal_claim = verify_spartan_univariate_skip_proof(
            &mut grinder,
            &matrices,
            &binding,
            &proof.spartan,
        )?;
        grinder.finish()?;
        terminal_claim
    };

    let step4_scope = crate::utils::prof::scope("step4:bitify_verify");
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:bitify_verifier");
        let opening = bitify_u32_mul_spartan_claim(&terminal_claim, layout, q, &arith)?;
        let bridge_digest =
            bitified_claim_digest(&matrices, &binding, layout, &terminal_claim, &opening, q)?;
        (opening, bridge_digest)
    };
    check_boundary_u32::<U32MulTerminalGrinding, _>(
        transcript,
        security.terminal_grinding_bits,
        proof.terminal_nonce,
    )?;
    drop(step4_scope);

    let prepared_claim = prepare_u32_bitified_claim(&opening, q_bits, &arith)?;
    let weights = prepared_claim.chunks.chunks();
    if weights.len() != 1 || proof.sums.len() != p.cols() {
        return Err(Error::Invalid("F2Z sums shape"));
    }
    // Every true integer fold lies below the group order. Check a tighter,
    // instance-derived bound on the sent integers before exponentiation.
    let bound = weights[0]
        .iter()
        .try_fold(0u128, |sum, w| sum.checked_add(*w))
        .and_then(|s| s.checked_mul((1u128 << p.word_bits) - 1))
        .ok_or(Error::Invalid("integer fold bound overflow"))?;
    if bound == u128::MAX || proof.sums.iter().any(|&s| s > bound) {
        return Err(Error::Invalid("integer fold magnitude"));
    }
    let read_off = proof
        .sums
        .iter()
        .zip(&prepared_claim.col_weights)
        .fold(0, |a, (&s, w)| {
            arith.add(a, arith.mul(arith.reduce(s), w.0))
        });
    if read_off != prepared_claim.claimed.0 {
        return Err(Error::Invalid("integer read-off"));
    }
    bind_sums(transcript, &bridge_digest, &proof.sums);
    let comb = FixedBasePow::new(f2z_generator(), 128, 8);
    let roots: Vec<_> = proof.sums.iter().map(|&s| comb.pow(s)).collect();
    let (z, e) = verify_merged_forest(transcript, &roots, &proof.forest, row_bit_vars(&p), p.s)
        .map_err(|_| Error::Invalid("multiplication GKR"))?;
    Ok(endpoint(&p, &weights[0], &z, e))
}
