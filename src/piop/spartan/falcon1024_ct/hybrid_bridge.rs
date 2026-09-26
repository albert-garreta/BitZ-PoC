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
    ligerito::fold_values_bits_multi,
    merged_forest::{
        ForestScratch, MergedForestProof, prove_merged_forest_lazy_multi_from_rows_with_scratch,
        verify_merged_forest,
    },
    pcs::{
        FlatPowers, IntegerMatrixLayout, PowerTable, chunk_pow2_flat, chunk_row_weights,
        mod_q_chunk_width, mod_q_num_chunks,
    },
    piop::spartan::{SpartanBitzField, SpartanField, grinding::GrindingDomain, matrix::eq_table},
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::traits::Transcript,
};
use field::Uint;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

/// Conservative numerator for the merged limb forest. With depth d and s
/// tree-index variables (including the limb index), the initial root projection,
/// degree-at-most-three sumcheck rounds, and d line folds cost at most
/// s + 3*(d*(d-1)/2 + d*s) + d. At the largest supported layout this is 887.
/// The existing budget leaves room for all supported batch geometries.
pub(super) const ERROR_NUMERATOR: usize = 4096;

fn binary_claim(
    p: &IntegerMatrixLayout,
    powers: &[FlatPowers],
    point: &[Gf],
    value: Gf,
) -> BinaryClaim {
    let (row_point, high) = point.split_at(p.row_vars);
    let (column_point, limb_point) = high.split_at(p.col_vars);
    let limb_weights = build_eq_x_r_vec(limb_point, &()).expect("nonempty limb point");
    let mut low = build_eq_x_r_vec(row_point, &()).expect("nonempty row point");
    for (r, weight) in low.iter_mut().enumerate() {
        let factor = powers
            .iter()
            .zip(&limb_weights)
            .fold(Gf::zero(), |acc, (table, &limb)| {
                acc + limb * (table.power(r, 0) + Gf::one())
            });
        *weight *= factor;
    }
    BinaryClaim {
        low,
        high_point: column_point.to_vec(),
        value: value + Gf::one(),
    }
}

struct BridgeGrinding;
impl GrindingDomain for BridgeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/bridge-grinding/v2";
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    pub sums: Vec<Vec<u128>>,
    pub forest: MergedForestProof,
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
    t.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v2");
    t.absorb_slice(&(sums.len() as u64).to_le_bytes());
    for column in sums {
        t.absorb_slice(&(column.len() as u64).to_le_bytes());
        for x in column {
            t.absorb_slice(&x.to_le_bytes());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
    scratch: &ForestScratch,
) -> Result<(Proof, Vec<BinaryClaim>), FalconError> {
    let p = source.layout().bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    if row_weights.len() != p.rows() || row_weights.iter().any(|&weight| weight >= modulus) {
        return Err(err("bridge row weights"));
    }
    let q_bits = 128 - modulus.leading_zeros() as usize;
    let chunks = chunk_row_weights(
        row_weights,
        mod_q_chunk_width(&p),
        mod_q_num_chunks(&p, q_bits),
    );
    if chunks.len() != 2 {
        return Err(err("bridge needs two bounded prime limbs"));
    }
    let fold_span = tracing::info_span!("falcon_bridge:integer_folds").entered();
    let chunk_refs: Vec<_> = chunks.iter().map(Vec::as_slice).collect();
    let sums = fold_values_bits_multi(&p, source.rows(), &chunk_refs);
    drop(fold_span);
    bind(t, &sums);
    let generator = crate::pcs::smallest_generator();
    let power_span = tracing::info_span!("falcon_bridge:power_tables").entered();
    let powers: Vec<_> = chunks
        .iter()
        .map(|w| chunk_pow2_flat(&p, w, generator))
        .collect();
    drop(power_span);
    let mut grinder = ProverBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits);
    let forest_span = tracing::info_span!("falcon_bridge:merged_forest").entered();
    let (_, forest, z, e) = prove_merged_forest_lazy_multi_from_rows_with_scratch(
        &mut grinder,
        &p,
        source.rows(),
        &powers,
        scratch,
    )
    .map_err(|_| err("unsupported bridge forest schedule"))?;
    drop(forest_span);
    let residual_span = tracing::info_span!("falcon_bridge:residual").entered();
    let claim = binary_claim(&p, &powers, &z, e);
    drop(residual_span);
    let nonces = grinder.finish();
    Ok((
        Proof {
            sums,
            forest,
            nonces,
        },
        vec![claim],
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
    if chunks.len() != 2 || proof.sums.len() != chunks.len() {
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
    let roots: Vec<_> = proof
        .sums
        .iter()
        .flatten()
        .map(|&s| Gf::from(comb.pow_public(&Uint::from_words([s as u64, (s >> 64) as u64]))))
        .collect();
    let (z, e) = verify_merged_forest(
        &mut grinder,
        &roots,
        &proof.forest,
        p.row_vars,
        p.col_vars + 1,
    )
    .map_err(|_| err("bridge forest"))?;
    grinder.finish().map_err(|_| err("bridge grinding"))?;
    let powers: Vec<_> = chunks
        .iter()
        .map(|w| chunk_pow2_flat(&p, w, generator))
        .collect();
    Ok(vec![binary_claim(&p, &powers, &z, e)])
}

#[cfg(test)]
mod tests {
    use super::super::{FalconSourceLayout, verification_trace};
    use super::*;
    use crate::transcript::Blake3Transcript;

    #[test]
    fn merged_limb_forest_fits_security_budget() {
        for batch in [1, 3, 32, 256, 1024] {
            let p = FalconSourceLayout::new_hybrid(batch).unwrap().bitz_params();
            let d = p.row_vars;
            let s = p.col_vars + 1;
            let numerator = s + 3 * (d * (d - 1) / 2 + d * s) + d;
            assert!(numerator <= ERROR_NUMERATOR);
        }
    }

    #[test]
    fn batched_limbs_authenticate_source_and_reject_compensating_changes() {
        const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
        const MSG: &[u8] = include_bytes!("fixtures/message.bin");
        const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");
        let layout = FalconSourceLayout::new_hybrid(1).unwrap();
        let trace = verification_trace(PK, MSG, SIG).unwrap();
        let source =
            FalconSourceWitness::from_traces(layout.clone(), &[MSG], &[SIG], &[trace]).unwrap();
        let p = layout.bitz_params();
        let modulus = (1u128 << 127) - 1;
        let field = F::make_cfg(&Uint::from(modulus)).unwrap();
        let point: Vec<_> = (0..p.row_vars + p.col_vars)
            .map(|i| F::from_with_cfg(Uint::from(3 + 2 * i as u128), &field))
            .collect();
        let row_weights = weights(&point[..p.row_vars], &field);
        let column_weights = weights(&point[p.row_vars..], &field);
        // Independent prime-field evaluation over the original committed bits.
        let mut value = 0;
        for (c, row) in source.rows().iter().enumerate() {
            let mut column = 0;
            for r in 0..p.rows() {
                if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                    column = field.add_u128(column, row_weights[r]);
                }
            }
            value = field.add_u128(value, field.mul_u128(column, column_weights[c]));
        }
        let value = F::from_with_cfg(Uint::from(value), &field);
        let mut pt = Blake3Transcript::new();
        let (proof, claims) = prove(
            &mut pt,
            &source,
            &point,
            modulus,
            2,
            &row_weights,
            &ForestScratch::default(),
        )
        .unwrap();
        assert_eq!(claims.len(), 1);
        let mut vt = Blake3Transcript::new();
        let verified = verify(&mut vt, &layout, &point, value, modulus, &proof, 2).unwrap();
        assert_eq!(claims[0].low, verified[0].low);
        assert_eq!(claims[0].high_point, verified[0].high_point);
        assert_eq!(claims[0].value, verified[0].value);
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());
        let column_eq = build_eq_x_r_vec(&claims[0].high_point, &()).unwrap();
        let mut binary_value = Gf::zero();
        for (c, row) in source.rows().iter().enumerate() {
            let mut column = Gf::zero();
            for r in 0..p.rows() {
                if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                    column += claims[0].low[r];
                }
            }
            binary_value += column_eq[c] * column;
        }
        assert_eq!(binary_value, claims[0].value);

        let reject = |changed: &Proof| {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &point,
                    value,
                    modulus,
                    changed,
                    2,
                )
                .is_err()
            );
        };
        for limb in 0..2 {
            let mut changed = proof.clone();
            changed.sums[limb][0] ^= 1;
            reject(&changed);
        }
        // Preserve the prime read-off exactly while shifting mass between
        // limbs. Only the authenticated forest can detect this forgery.
        let mut changed = proof.clone();
        let radix = 1u128 << mod_q_chunk_width(&p);
        let c = changed.sums[0]
            .iter()
            .position(|&sum| sum >= radix)
            .unwrap();
        changed.sums[0][c] -= radix;
        changed.sums[1][c] += 1;
        let error = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &point,
            value,
            modulus,
            &changed,
            2,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("bridge forest"));
        let mut changed = proof.clone();
        changed.forest.layers[0].pair.0 += Gf::one();
        reject(&changed);
    }
}
