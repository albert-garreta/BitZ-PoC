//! Multiplication reduction stopped at its binary inner-product claim.
//!
//! For the hybrid mod-2^32 relation, product = z + 2^32*w is the fixed
//! reconstruction of the two committed 32-bit output limbs. Proving
//! x*y = product with all 64 product bits bound proves the modular relation.
//! The protocol prefix (statement, prime draw, Spartan PIOP, bitification)
//! is the shared one of [`super::super::protocol`]; only the discharge of
//! the bitified claim — the integer folds bound in the clear plus the GKR
//! forest — is the hybrid's own, since its opener runs at the composition's
//! geometry.
use super::*;
use super::super::{
    absorb_spartan_message,
    protocol::{
        Modular, SpartanPrefixProof, SpartanProof, bitify, check_boundary, f2z_generator,
        prove_prefix, sample_mod_q, validate_bit_rows, verify_prefix,
    },
    univariate_skip::UnivariateSkipSpartanPiopProof,
};
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

impl PrefixProof {
    fn messages(&self) -> SpartanPrefixProof {
        SpartanPrefixProof {
            initial_nonce: self.initial_nonce,
            piop_nonces: self.piop_nonces.clone(),
            spartan: SpartanProof::UnivariateSkip(self.spartan.clone()),
            terminal_nonce: self.terminal_nonce,
        }
    }
}

pub(crate) fn decoding_config(
    transcript: &mut Blake3Transcript,
    prepared: &U32MulPrefixRelation,
    statement: &[u8; 32],
    nonce: u64,
) -> Result<(u128, <SpartanF2zField as PrimeField>::Config), Error> {
    let domains = prepared.layout().domains();
    absorb_spartan_message(transcript, domains.statement_tag, statement);
    check_boundary(
        transcript,
        domains.initial_grinding,
        prepared.security().initial_grinding_bits,
        nonce,
    )?;
    let prime = sample_mod_q(
        transcript,
        domains.prime_sampling,
        prepared.security().projection_min,
        prepared.security().projection_max,
    )?;
    Ok((prime.q, prime.config))
}

fn bind_sums(t: &mut Blake3Transcript, digest: &[u8; 32], sums: &[u128]) {
    t.absorb_slice(b"hybrid/u32-bounded-sums/v1");
    t.absorb_slice(digest);
    for sum in sums {
        t.absorb_slice(&sum.to_le_bytes());
    }
}

fn endpoint(p: &crate::pcs::IntegerMatrixLayout, weights: &[u128], z: &[Gf], e: Gf) -> BinaryClaim {
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
    prepared: &U32MulPrefixRelation,
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
    absorb_spartan_message(transcript, layout.domains().statement_tag, statement);
    let proved = prove_prefix(transcript, prepared, witness, statement, ProveOptions::default())?;
    drop(piop_scope);

    let _opening_scope = crate::utils::prof::scope("hybrid:mul_opening");
    let arith = &proved.prime.arith;
    let chunks = bitify::prepare_chunks(&proved.opening, &proved.table, proved.prime.q_bits, arith)?;
    let weights = chunks.chunks();
    if weights.len() != 1 {
        return Err(Error::Invalid("multiple F2Z chunks"));
    }
    let fold_scope = crate::utils::prof::scope("mo:fold_values");
    let sums = fold_values_bits(&p, rows, &weights[0]);
    drop(fold_scope);
    bind_sums(transcript, &proved.bridge_digest, &sums);
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
    let SpartanPrefixProof {
        initial_nonce,
        piop_nonces,
        spartan,
        terminal_nonce,
    } = proved.messages;
    let SpartanProof::UnivariateSkip(spartan) = spartan else {
        return Err(Error::Invalid("u32 kernel shape"));
    };
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
    prepared: &U32MulPrefixRelation,
    statement: &[u8; 32],
    proof: &PrefixProof,
) -> Result<BinaryClaim, Error> {
    let layout = prepared.layout();
    let p = layout.f2z_params();
    let actual_skip_vars = proof.spartan.outer.skip.skip_vars;
    let expected_skip_vars = U32_MUL_UNIVARIATE_SKIP_VARS as u8;
    if actual_skip_vars != expected_skip_vars {
        return Err(SpartanF2zError::UnexpectedUnivariateSkipVariables {
            expected: expected_skip_vars,
            actual: actual_skip_vars,
        }
        .into());
    }

    absorb_spartan_message(transcript, layout.domains().statement_tag, statement);
    let verified = verify_prefix(transcript, prepared, statement, &proof.messages())?;
    let arith = &verified.prime.arith;

    let chunks = bitify::prepare_chunks(&verified.opening, &verified.table, verified.prime.q_bits, arith)?;
    let col_weights = bitify::column_weights(&verified.opening, arith)?;
    let weights = chunks.chunks();
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
        .zip(&col_weights)
        .fold(0, |a, (&s, w)| arith.add(a, arith.mul(arith.reduce(s), w.0)));
    if read_off != verified.opening.claimed.0 {
        return Err(Error::Invalid("integer read-off"));
    }
    bind_sums(transcript, &verified.bridge_digest, &proof.sums);
    let comb = FixedBasePow::new(f2z_generator(), 128, 8);
    let roots: Vec<_> = proof.sums.iter().map(|&s| comb.pow(s)).collect();
    let (z, e) = verify_merged_forest(
        transcript,
        &roots,
        &proof.forest,
        row_bit_vars(&p),
        p.col_vars,
    )
    .map_err(|_| Error::Invalid("multiplication GKR"))?;
    Ok(endpoint(&p, &weights[0], &z, e))
}
