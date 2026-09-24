use crate::{
    cfg_iter,
    merged_forest::{
        MergedForestProfile, MergedForestProof, merged_forest_proof_size_bytes,
        allocate_prepared_merged_forest, prepare_merged_forest_strided,
        prove_prepared_merged_forest_from_claim, verify_merged_forest_from_claim,
        PreparedMergedForest,
    },
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::SumcheckProof,
    },
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::eq_eval},
    transcript::traits::{Transcribable, Transcript},
};

use std::time::{Duration, Instant};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    CutClaim, DyadicPlan,
    product::{
        ProductInput, ProductWorkspace, prove_products, prove_products_materialized,
    },
};

const DOMAIN: &[u8] = b"bitz/logup-cut/dyadic-upper/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProductBatchProof {
    sumcheck: Option<SumcheckProof<Gf>>,
    evals: Vec<(Gf, Gf)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DyadicUpperProof {
    root_merge: Vec<ProductBatchProof>,
    block_forests: Vec<Option<MergedForestProof>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DyadicUpperProfile {
    pub forest_build: Duration,
    pub root_merge: Duration,
    pub forest_sumcheck: Duration,
    pub phase_a_messages: Duration,
    pub phase_a_folds: Duration,
    pub phase_a_close: Duration,
    pub phase_b: Duration,
    pub buffer_release: Duration,
}

pub(crate) struct DyadicUpperScratch {
    forests: Vec<Option<PreparedMergedForest>>,
}

impl DyadicUpperScratch {
    pub(crate) fn new(plan: &DyadicPlan, tree_vars: usize) -> Self {
        Self {
            forests: plan
                .blocks
                .iter()
                .map(|block| {
                    (block.depth != 0)
                        .then(|| allocate_prepared_merged_forest(block.depth, tree_vars))
                })
                .collect(),
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.forests
            .iter()
            .flatten()
            .map(PreparedMergedForest::retained_bytes)
            .sum()
    }
}

impl ProductBatchProof {
    pub(super) fn proof_size_bytes(&self) -> usize {
        self.sumcheck
            .as_ref()
            .map_or(0, Transcribable::get_num_bytes)
            + 2 * self.evals.len() * 16
    }
}

impl DyadicUpperProof {
    pub(crate) fn proof_size_bytes(&self) -> (usize, usize) {
        (
            self.root_merge.iter().map(ProductBatchProof::proof_size_bytes).sum(),
            self.block_forests
                .iter()
                .flatten()
                .map(merged_forest_proof_size_bytes)
                .sum(),
        )
    }
}

pub(crate) fn prove_dyadic_upper(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    root_point: &[Gf],
    root_value: Gf,
    cut_values: &[Gf],
    product_workspace: &mut ProductWorkspace,
    scratch: &mut DyadicUpperScratch,
    profile: &mut DyadicUpperProfile,
) -> (DyadicUpperProof, Vec<CutClaim>) {
    bind_plan(transcript, plan, r2);
    let columns = 1usize << r2;
    assert_eq!(root_point.len(), r2);
    assert_eq!(cut_values.len(), plan.n_chunks * columns);
    assert_eq!(scratch.forests.len(), plan.blocks.len());

    let mut block_roots = Vec::with_capacity(plan.blocks.len());
    for (block, forest) in plan.blocks.iter().zip(&mut scratch.forests) {
        let started = Instant::now();
        if block.depth == 0 {
            block_roots.push(
                (0..columns)
                    .map(|column| cut_values[column * plan.n_chunks + block.chunk_start])
                    .collect(),
            );
            assert!(forest.is_none());
        } else {
            let forest = forest.as_mut().expect("preallocated block forest");
            prepare_merged_forest_strided(
                forest,
                cut_values,
                plan.n_chunks,
                block.chunk_start,
                block.depth,
                r2,
            );
            block_roots.push(forest.roots().to_vec());
        }
        profile.forest_build += started.elapsed();
    }

    let started = Instant::now();
    let (block_claims, root_merge) = if block_roots.len() == 1 {
        (vec![(root_point.to_vec(), root_value)], Vec::new())
    } else {
        prove_root_product(
            transcript,
            root_point,
            root_value,
            block_roots,
            product_workspace,
        )
    };
    profile.root_merge = started.elapsed();
    let mut claims = Vec::with_capacity(plan.blocks.len());
    let mut block_forests = Vec::with_capacity(plan.blocks.len());
    for (block_index, ((block, (point, value)), forest)) in plan
        .blocks
        .iter()
        .zip(block_claims)
        .zip(&mut scratch.forests)
        .enumerate()
    {
        if let Some(forest) = forest.as_mut() {
            let mut forest_profile = MergedForestProfile::default();
            let (proof, point, value) =
                prove_prepared_merged_forest_from_claim(
                    transcript,
                    forest,
                    &point,
                    value,
                    &mut forest_profile,
                );
            profile.forest_sumcheck += forest_profile.sumcheck;
            profile.phase_a_messages += forest_profile.upper_messages;
            profile.phase_a_folds += forest_profile.upper_folds;
            profile.phase_a_close += forest_profile.phase_a_close;
            profile.phase_b += forest_profile.phase_b;
            profile.buffer_release += forest_profile.buffer_release;
            block_forests.push(Some(proof));
            claims.push(CutClaim {
                block: block_index,
                point,
                value,
            });
        } else {
            debug_assert_eq!(block.depth, 0);
            block_forests.push(None);
            claims.push(CutClaim {
                block: block_index,
                point,
                value,
            });
        }
    }
    (
        DyadicUpperProof {
            root_merge,
            block_forests,
        },
        claims,
    )
}

pub(crate) fn verify_dyadic_upper(
    transcript: &mut impl Transcript,
    proof: &DyadicUpperProof,
    plan: &DyadicPlan,
    r2: usize,
    root_point: &[Gf],
    root_value: Gf,
) -> Option<Vec<CutClaim>> {
    bind_plan(transcript, plan, r2);
    if root_point.len() != r2 {
        return None;
    }
    let block_claims = if plan.blocks.len() == 1 {
        if !proof.root_merge.is_empty() {
            return None;
        }
        vec![(root_point.to_vec(), root_value)]
    } else {
        verify_root_product(
            transcript,
            root_point,
            root_value,
            plan.blocks.len(),
            &proof.root_merge,
        )?
    };
    if proof.block_forests.len() != plan.blocks.len() {
        return None;
    }
    let mut claims = Vec::with_capacity(plan.blocks.len());
    for (block_index, (block, ((point, value), forest))) in plan
        .blocks
        .iter()
        .zip(block_claims.into_iter().zip(&proof.block_forests))
        .enumerate()
    {
        let (point, value) = match (forest, block.depth) {
            (Some(proof), depth @ 1..) => {
                verify_merged_forest_from_claim(transcript, proof, depth, &point, value).ok()?
            }
            (None, 0) => (point, value),
            _ => return None,
        };
        claims.push(CutClaim {
            block: block_index,
            point,
            value,
        });
    }
    Some(claims)
}

fn bind_plan(transcript: &mut impl Transcript, plan: &DyadicPlan, r2: usize) {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(plan.n_chunks as u64).to_le_bytes());
    transcript.absorb_slice(&(r2 as u64).to_le_bytes());
    transcript.absorb_slice(&(plan.blocks.len() as u64).to_le_bytes());
    for block in &plan.blocks {
        transcript.absorb_slice(&(block.chunk_start as u64).to_le_bytes());
        transcript.absorb_slice(&(block.n_chunks as u64).to_le_bytes());
    }
}

fn prove_root_product(
    transcript: &mut impl Transcript,
    root_point: &[Gf],
    root_value: Gf,
    block_roots: Vec<Vec<Gf>>,
    product_workspace: &mut ProductWorkspace,
) -> (Vec<(Vec<Gf>, Gf)>, Vec<ProductBatchProof>) {
    let leaves = block_roots.len();
    let padded_leaves = leaves.next_power_of_two();
    let mut leaf_tables = block_roots;
    leaf_tables.resize_with(padded_leaves, || vec![Gf::ONE; 1usize << root_point.len()]);
    let mut levels = vec![leaf_tables];
    while levels.last().unwrap().len() > 1 {
        let children = levels.last().unwrap();
        levels.push(
            children
                .chunks_exact(2)
                .map(|pair| {
                    cfg_iter!(&pair[0])
                        .zip(cfg_iter!(&pair[1]))
                        .map(|(&left, &right)| left * right)
                        .collect()
                })
                .collect(),
        );
    }

    let mut claims = vec![(root_point.to_vec(), root_value)];
    let mut proofs = Vec::with_capacity(levels.len() - 1);
    for level in (1..levels.len()).rev() {
        let children = &levels[level - 1];
        debug_assert_eq!(children.len(), 2 * claims.len());
        let inputs = children
            .chunks_exact(2)
            .map(|pair| (ProductInput::Table(&pair[0]), ProductInput::Table(&pair[1])))
            .collect::<Vec<_>>();
        let (proof, point, values) = prove_product_batch(
            transcript,
            &claims,
            &inputs,
            product_workspace,
        );
        proofs.push(proof);
        claims.clear();
        for (left, right) in values {
            claims.push((point.clone(), left));
            claims.push((point.clone(), right));
        }
    }
    debug_assert!(claims[leaves..].iter().all(|claim| claim.1 == Gf::ONE));
    claims.truncate(leaves);
    (claims, proofs)
}

fn verify_root_product(
    transcript: &mut impl Transcript,
    root_point: &[Gf],
    root_value: Gf,
    leaves: usize,
    proofs: &[ProductBatchProof],
) -> Option<Vec<(Vec<Gf>, Gf)>> {
    let depth = leaves.next_power_of_two().ilog2() as usize;
    if proofs.len() != depth {
        return None;
    }
    let mut claims = vec![(root_point.to_vec(), root_value)];
    for proof in proofs {
        let (point, values) = verify_product_batch(transcript, &claims, proof)?;
        claims.clear();
        for (left, right) in values {
            claims.push((point.clone(), left));
            claims.push((point.clone(), right));
        }
    }
    if claims[leaves..].iter().any(|claim| claim.1 != Gf::ONE) {
        return None;
    }
    claims.truncate(leaves);
    Some(claims)
}

pub(super) fn prove_product_batch(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
) -> (ProductBatchProof, Vec<Gf>, Vec<(Gf, Gf)>) {
    let dimension = claims[0].0.len();
    let (sumcheck, point, evals) = prove_products(transcript, claims, inputs, workspace);
    (
        ProductBatchProof {
            sumcheck: (dimension != 0).then_some(sumcheck),
            evals: evals.clone(),
        },
        point,
        evals,
    )
}

pub(super) fn prove_product_batch_materialized(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
    materialized: (&mut [Gf], &mut [Gf]),
) -> (ProductBatchProof, Vec<Gf>, Vec<(Gf, Gf)>) {
    let dimension = claims[0].0.len();
    let (sumcheck, point, evals) = prove_products_materialized(
        transcript,
        claims,
        inputs,
        workspace,
        materialized,
    );
    (
        ProductBatchProof {
            sumcheck: (dimension != 0).then_some(sumcheck),
            evals: evals.clone(),
        },
        point,
        evals,
    )
}

pub(super) fn verify_product_batch(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    proof: &ProductBatchProof,
) -> Option<(Vec<Gf>, Vec<(Gf, Gf)>)> {
    if claims.is_empty() || proof.evals.len() != claims.len() {
        return None;
    }
    let dimension = claims[0].0.len();
    if claims.iter().any(|claim| claim.0.len() != dimension) {
        return None;
    }
    let beta = (claims.len() > 1).then(|| transcript.get_field_challenge::<Gf>(&()));
    let mut scale = Gf::ONE;
    let mut scales = Vec::with_capacity(claims.len());
    let mut claimed_sum = Gf::ZERO;
    for claim in claims {
        scales.push(scale);
        claimed_sum += scale * claim.1;
        if let Some(beta) = beta {
            scale *= beta;
        }
    }

    let point = if dimension == 0 {
        if proof.sumcheck.is_some() {
            return None;
        }
        Vec::new()
    } else {
        let sumcheck = proof.sumcheck.as_ref()?;
        if sumcheck.claimed_sum != claimed_sum {
            return None;
        }
        if claims[1..].iter().any(|claim| claim.0 != claims[0].0) {
            return None;
        }
        let subclaim = crate::piop::sumcheck::eq_factored::verify_eq_inner_sumcheck_gruen(
            transcript,
            &claims[0].0,
            sumcheck,
            &(),
        )
        .ok()?;
        let mut expected = Gf::ZERO;
        for ((claim, &(left, right)), &scale) in claims.iter().zip(&proof.evals).zip(&scales) {
            expected += scale * eq_eval(&subclaim.point, &claim.0, Gf::ONE).ok()? * left * right;
        }
        if expected != subclaim.expected_evaluation {
            return None;
        }
        subclaim.point
    };

    let flat = proof.evals.iter().flat_map(|&(l, r)| [l, r]).collect::<Vec<_>>();
    absorb_field_slice(transcript, &flat);
    if dimension == 0 {
        let expected: Gf = proof
            .evals
            .iter()
            .zip(scales)
            .map(|(&(left, right), scale)| scale * left * right)
            .sum();
        if expected != claimed_sum {
            return None;
        }
    }
    Some((point, proof.evals.clone()))
}
