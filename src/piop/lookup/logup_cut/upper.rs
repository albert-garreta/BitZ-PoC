use crate::{
    cfg_chunks, cfg_chunks_mut, cfg_iter, cfg_iter_mut,
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::{SumcheckProof, eq_factored::FlatDense},
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
        ProductInput, ProductWorkspace, prove_products, prove_products_flat,
        prove_products_materialized,
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
    block_layers: Vec<ProductBatchProof>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DyadicUpperProfile {
    pub forest_build: Duration,
    pub root_merge: Duration,
    pub forest_sumcheck: Duration,
}

pub(crate) struct DyadicUpperScratch {
    roots: Vec<Vec<Gf>>,
    levels: Vec<PreparedProductLevel>,
}

struct PreparedProductLevel {
    blocks: Vec<usize>,
    values: FlatDense<Gf>,
}

impl DyadicUpperScratch {
    pub(crate) fn new(plan: &DyadicPlan, tree_vars: usize) -> Self {
        let columns = 1usize << tree_vars;
        let max_depth = plan.blocks.iter().map(|block| block.depth).max().unwrap_or(0);
        Self {
            roots: plan.blocks.iter().map(|_| vec![Gf::ZERO; columns]).collect(),
            levels: (0..max_depth)
                .map(|level| {
                    let blocks = plan
                        .blocks
                        .iter()
                        .enumerate()
                        .filter_map(|(block, spec)| (spec.depth > level).then_some(block))
                        .collect::<Vec<_>>();
                    let seg = columns << level;
                    let len = blocks.len() * seg;
                    PreparedProductLevel {
                        blocks,
                        values: FlatDense {
                            l: vec![Gf::ZERO; len],
                            r: vec![Gf::ZERO; len],
                            seg,
                        },
                    }
                })
                .collect(),
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        (self.roots
            .iter()
            .map(Vec::capacity)
            .sum::<usize>()
            + self
                .levels
                .iter()
                .map(|level| level.values.l.capacity() + level.values.r.capacity())
                .sum::<usize>())
            * core::mem::size_of::<Gf>()
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
            self.block_layers.iter().map(ProductBatchProof::proof_size_bytes).sum(),
        )
    }
}

pub(crate) fn prove_dyadic_upper(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    root_point: &[Gf],
    root_value: Gf,
    cut_value: &(impl Fn(usize, usize) -> Gf + Sync),
    product_workspace: &mut ProductWorkspace,
    scratch: &mut DyadicUpperScratch,
    profile: &mut DyadicUpperProfile,
) -> (DyadicUpperProof, Vec<CutClaim>) {
    bind_plan(transcript, plan, r2);
    let columns = 1usize << r2;
    assert_eq!(root_point.len(), r2);
    assert_eq!(scratch.roots.len(), plan.blocks.len());

    let started = Instant::now();
    for (block_index, block) in plan.blocks.iter().enumerate() {
        if block.depth == 0 {
            cfg_iter_mut!(&mut scratch.roots[block_index])
                .enumerate()
                .for_each(|(column, root)| {
                    *root = cut_value(column, block.chunk_start);
                });
            continue;
        }

        let top = block.depth - 1;
        let half = 1usize << top;
        let top_level = &mut scratch.levels[top];
        let slot = top_level.blocks.iter().position(|&block| block == block_index).unwrap();
        let start = slot * top_level.values.seg;
        let end = start + top_level.values.seg;
        let left = &mut top_level.values.l[start..end];
        let right = &mut top_level.values.r[start..end];
        cfg_chunks_mut!(left, half)
            .zip(cfg_chunks_mut!(right, half))
            .enumerate()
            .for_each(|(column, (left, right))| {
                for index in 0..half {
                    left[index] = cut_value(column, block.chunk_start + index);
                    right[index] = cut_value(column, block.chunk_start + index + half);
                }
            });

        for level in (1..block.depth).rev() {
            let child_slot = scratch.levels[level]
                .blocks
                .iter()
                .position(|&block| block == block_index)
                .unwrap();
            let parent_slot = scratch.levels[level - 1]
                .blocks
                .iter()
                .position(|&block| block == block_index)
                .unwrap();
            let (parents, children) = scratch.levels.split_at_mut(level);
            let parent = &mut parents[level - 1].values;
            let child = &children[0].values;
            let parent_start = parent_slot * parent.seg;
            let child_start = child_slot * child.seg;
            let parent_left = &mut parent.l[parent_start..parent_start + parent.seg];
            let parent_right = &mut parent.r[parent_start..parent_start + parent.seg];
            let child_left = &child.l[child_start..child_start + child.seg];
            let child_right = &child.r[child_start..child_start + child.seg];
            let child_len = 1usize << level;
            let parent_len = child_len >> 1;
            cfg_chunks_mut!(parent_left, parent_len)
                .zip(cfg_chunks_mut!(parent_right, parent_len))
                .zip(cfg_chunks!(child_left, child_len))
                .zip(cfg_chunks!(child_right, child_len))
                .for_each(|(((left, right), child_left), child_right)| {
                    for index in 0..parent_len {
                        left[index] = child_left[index] * child_right[index];
                        right[index] =
                            child_left[index + parent_len] * child_right[index + parent_len];
                    }
                });
        }

        let level = &scratch.levels[0];
        let slot = level.blocks.iter().position(|&block| block == block_index).unwrap();
        let start = slot * level.values.seg;
        let left = &level.values.l[start..start + columns];
        let right = &level.values.r[start..start + columns];
        cfg_iter_mut!(&mut scratch.roots[block_index])
            .zip(cfg_iter!(left).zip(cfg_iter!(right)))
            .for_each(|(root, (&left, &right))| *root = left * right);
    }
    profile.forest_build += started.elapsed();

    let block_roots = scratch.roots.clone();

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
    let started = Instant::now();
    let mut active = block_claims.into_iter().map(Some).collect::<Vec<_>>();
    let mut block_layers = Vec::with_capacity(scratch.levels.len());
    for (layer, prepared) in scratch.levels.iter_mut().enumerate() {
        let claims = prepared
            .blocks
            .iter()
            .map(|&block| active[block].take().unwrap())
            .collect::<Vec<_>>();
        debug_assert!(claims.iter().all(|claim| claim.0.len() == r2 + layer));
        let (sumcheck, point, values) =
            prove_products_flat(transcript, &claims, &mut prepared.values);
        block_layers.push(ProductBatchProof {
            sumcheck: Some(sumcheck),
            evals: values.clone(),
        });
        let selector = transcript.get_field_challenge::<Gf>(&());
        for ((&block, &(left, right)), claim) in
            prepared.blocks.iter().zip(&values).zip(claims)
        {
            let mut next_point = point.clone();
            next_point.insert(layer, selector);
            debug_assert_eq!(claim.0.len() + 1, next_point.len());
            active[block] = Some((next_point, left + selector * (left + right)));
        }
    }
    profile.forest_sumcheck += started.elapsed();
    let claims = active
        .into_iter()
        .enumerate()
        .map(|(block, claim)| {
            let (point, value) = claim.unwrap();
            CutClaim { block, point, value }
        })
        .collect();
    (
        DyadicUpperProof {
            root_merge,
            block_layers,
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
    let max_depth = plan.blocks.iter().map(|block| block.depth).max().unwrap_or(0);
    if proof.block_layers.len() != max_depth {
        return None;
    }
    let mut active = block_claims.into_iter().map(Some).collect::<Vec<_>>();
    for (layer, proof) in proof.block_layers.iter().enumerate() {
        let blocks = plan
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(block, spec)| (spec.depth > layer).then_some(block))
            .collect::<Vec<_>>();
        let claims = blocks
            .iter()
            .map(|&block| active[block].take().unwrap())
            .collect::<Vec<_>>();
        let (point, values) = verify_product_batch(transcript, &claims, proof)?;
        let selector = transcript.get_field_challenge::<Gf>(&());
        for ((&block, &(left, right)), claim) in blocks.iter().zip(&values).zip(claims) {
            let mut next_point = point.clone();
            next_point.insert(layer, selector);
            if claim.0.len() + 1 != next_point.len() {
                return None;
            }
            active[block] = Some((next_point, left + selector * (left + right)));
        }
    }
    active
        .into_iter()
        .enumerate()
        .map(|(block, claim)| {
            let (point, value) = claim?;
            (point.len() == r2 + plan.blocks[block].depth)
                .then_some(CutClaim { block, point, value })
        })
        .collect()
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
