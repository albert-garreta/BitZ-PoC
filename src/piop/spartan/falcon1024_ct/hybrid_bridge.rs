//! Prime-field source evaluation reduced to binary linear claims.
//!
//! Canonical prime-field row weights are split into bounded integer limbs.
//! The exponent-fold forest authenticates each integer sum; no conversion
//! between a prime-field MLE and a binary-field MLE is assumed.
use super::hybrid_keccak::grinding::{
    ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript,
};
use super::{FalconError, FalconSourceWitness};
use crate::{
    hybrid::BinaryClaim,
    ligerito::{fold_values_bits, pack_columns_from_rows},
    merged_forest::{MergedForestProof, prove_merged_forest_lazy, verify_merged_forest},
    pcs::{
        chunk_pow2_table, chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks, row_bit_weights,
    },
    piop::spartan::{SpartanBitzField, SpartanField, grinding::GrindingDomain, matrix::eq_table},
    poly::univariate::binary_gf128::Gf128 as Gf,
    transcript::traits::Transcript,
};
use field::Uint;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

struct BridgeGrinding;
impl GrindingDomain for BridgeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/bridge-grinding/v1";
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    pub sums: Vec<Vec<u128>>,
    pub forests: Vec<MergedForestProof>,
    pub nonces: Vec<u64>,
}

fn err(message: &'static str) -> FalconError {
    FalconError::Piop(message.into())
}

fn weights(point: &[F], field: &Cfg) -> Vec<u128> {
    eq_table(point, field)
        .expect("validated bridge point")
        .into_iter()
        .map(|x| u128::from(field.to_integer(&x)))
        .collect()
}

fn bind(t: &mut impl Transcript, sums: &[Vec<u128>]) {
    t.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v1");
    t.absorb_slice(&(sums.len() as u64).to_le_bytes());
    for column in sums {
        t.absorb_slice(&(column.len() as u64).to_le_bytes());
        for x in column {
            t.absorb_slice(&x.to_le_bytes());
        }
    }
}

pub(super) fn prove(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
) -> Result<(Proof, Vec<BinaryClaim>), FalconError> {
    let p = source.layout().bitz_params();
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    let q_bits = 128 - modulus.leading_zeros() as usize;
    let chunks = chunk_row_weights(
        &weights(&point[..p.row_vars], &field),
        mod_q_chunk_width(&p),
        mod_q_num_chunks(&p, q_bits),
    );
    let sums: Vec<_> = chunks
        .iter()
        .map(|w| fold_values_bits(&p, source.rows(), w))
        .collect();
    bind(t, &sums);
    let packed = pack_columns_from_rows(&p, source.rows());
    let generator = crate::pcs::smallest_generator();
    let mut grinder = ProverBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits);
    let mut forests = Vec::new();
    let mut claims = Vec::new();
    for w in &chunks {
        let powers = chunk_pow2_table(&p, w, generator);
        let (_, forest, z, e) =
            prove_merged_forest_lazy(&mut grinder, &p, &packed, &powers, p.cols());
        forests.push(forest);
        claims.push(BinaryClaim {
            low: row_bit_weights(&p, w, generator, &z[..p.row_vars]),
            high_point: z[p.row_vars..].to_vec(),
            value: e + Gf::one(),
        });
    }
    let nonces = grinder.finish();
    Ok((
        Proof {
            sums,
            forests,
            nonces,
        },
        claims,
    ))
}

pub(super) fn verify(
    t: &mut impl Transcript,
    layout: &super::FalconSourceLayout,
    point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
) -> Result<Vec<BinaryClaim>, FalconError> {
    let p = layout.bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    let q_bits = 128 - modulus.leading_zeros() as usize;
    let width = mod_q_chunk_width(&p);
    let chunks = chunk_row_weights(
        &weights(&point[..p.row_vars], &field),
        width,
        mod_q_num_chunks(&p, q_bits),
    );
    if proof.sums.len() != chunks.len() || proof.forests.len() != chunks.len() {
        return Err(err("bridge chunk shape"));
    }
    let cols = weights(&point[p.row_vars..], &field);
    let mut result = 0;
    let mut scale = 1;
    for (w, sums) in chunks.iter().zip(&proof.sums) {
        let bound = w
            .iter()
            .try_fold(0u128, |a, &b| a.checked_add(b))
            .ok_or(err("bridge sum bound"))?;
        if bound == u128::MAX || sums.len() != p.cols() || sums.iter().any(|&s| s > bound) {
            return Err(err("bridge integer fold magnitude"));
        }
        let partial = sums.iter().zip(&cols).fold(0, |a, (&s, &c)| {
            field.add_u128(a, field.mul_u128(field.reduce_u128(s), c))
        });
        result = field.add_u128(result, field.mul_u128(scale, partial));
        scale = field.mul_u128(scale, 1u128 << width);
    }
    if result != u128::from(field.to_integer(&value)) {
        return Err(err("bridge prime read-off"));
    }
    bind(t, &proof.sums);
    let generator = crate::pcs::smallest_generator();
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, generator.into(), 8);
    let mut grinder =
        VerifierBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits, &proof.nonces);
    let mut claims = Vec::new();
    for ((w, sums), forest) in chunks.iter().zip(&proof.sums).zip(&proof.forests) {
        let roots: Vec<_> = sums
            .iter()
            .map(|&s| Gf::from(comb.pow_public(&Uint::from_words([s as u64, (s >> 64) as u64]))))
            .collect();
        let (z, e) = verify_merged_forest(&mut grinder, &roots, forest, p.row_vars, p.col_vars)
            .map_err(|_| err("bridge forest"))?;
        claims.push(BinaryClaim {
            low: row_bit_weights(&p, w, generator, &z[..p.row_vars]),
            high_point: z[p.row_vars..].to_vec(),
            value: e + Gf::one(),
        });
    }
    grinder.finish().map_err(|_| err("bridge grinding"))?;
    Ok(claims)
}
