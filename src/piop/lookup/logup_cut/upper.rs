use crate::{
    cfg_iter,
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::SumcheckProof,
    },
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::eq_eval},
    transcript::traits::{Transcribable, Transcript},
};

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
    block_layers: Vec<ProductBatchProof>,
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

pub fn prove_dyadic_upper(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    root_point: &[Gf],
    root_value: Gf,
    cut_values: &[Gf],
    product_workspace: &mut ProductWorkspace,
) -> (DyadicUpperProof, Vec<CutClaim>) {
    bind_plan(transcript, plan, r2);
    let columns = 1usize << r2;
    assert_eq!(root_point.len(), r2);
    assert_eq!(cut_values.len(), plan.n_chunks * columns);

    let mut block_roots = Vec::with_capacity(plan.blocks.len());
    let mut witnesses = Vec::with_capacity(plan.blocks.len());
    for block in &plan.blocks {
        let start = block.chunk_start * columns;
        let end = start + block.n_chunks * columns;
        let leaves = cut_values[start..end].to_vec();
        let mut levels = vec![leaves];
        for _ in 0..block.depth {
            let child = levels.last().unwrap();
            let half = child.len() / 2;
            levels.push(
                cfg_iter!(&child[..half])
                    .zip(cfg_iter!(&child[half..]))
                    .map(|(&left, &right)| left * right)
                    .collect(),
            );
        }
        block_roots.push(levels.last().unwrap().clone());
        witnesses.push(levels);
    }

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
    let (claims, block_layers) = prove_block_forests(
        transcript,
        plan,
        r2,
        block_claims,
        witnesses,
        product_workspace,
    );
    (
        DyadicUpperProof {
            root_merge,
            block_layers,
        },
        claims,
    )
}

pub fn verify_dyadic_upper(
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
    verify_block_forests(
        transcript,
        plan,
        r2,
        block_claims,
        &proof.block_layers,
    )
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

fn prove_block_forests(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    block_claims: Vec<(Vec<Gf>, Gf)>,
    mut witnesses: Vec<Vec<Vec<Gf>>>,
    product_workspace: &mut ProductWorkspace,
) -> (Vec<CutClaim>, Vec<ProductBatchProof>) {
    let mut active = block_claims.into_iter().map(Some).collect::<Vec<_>>();
    let max_depth = plan.blocks.iter().map(|block| block.depth).max().unwrap();
    let mut proofs = Vec::with_capacity(max_depth);
    for layer in 0..max_depth {
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
        let owned_inputs = blocks
            .iter()
            .map(|&block| {
                let child_level = plan.blocks[block].depth - layer - 1;
                let mut child = std::mem::take(&mut witnesses[block][child_level]);
                let right = child.split_off(child.len() / 2);
                (child, right)
            })
            .collect::<Vec<_>>();
        let inputs = owned_inputs
            .iter()
            .map(|(left, right)| (ProductInput::Table(left), ProductInput::Table(right)))
            .collect::<Vec<_>>();
        let (proof, point, values) = prove_product_batch(
            transcript,
            &claims,
            &inputs,
            product_workspace,
        );
        proofs.push(proof);
        let selector = transcript.get_field_challenge(&());
        for ((&block, &(left, right)), claim) in blocks.iter().zip(&values).zip(claims) {
            let mut next_point = point.clone();
            next_point.push(selector);
            debug_assert_eq!(claim.0.len() + 1, next_point.len());
            active[block] = Some((next_point, left + selector * (left + right)));
        }
    }
    let claims = active
        .into_iter()
        .enumerate()
        .map(|(block, claim)| {
            let (point, value) = claim.unwrap();
            let mut normalized = point[r2..].to_vec();
            normalized.extend_from_slice(&point[..r2]);
            CutClaim {
                block,
                point: normalized,
                value,
            }
        })
        .collect();
    (claims, proofs)
}

fn verify_block_forests(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    block_claims: Vec<(Vec<Gf>, Gf)>,
    proofs: &[ProductBatchProof],
) -> Option<Vec<CutClaim>> {
    let mut active = block_claims.into_iter().map(Some).collect::<Vec<_>>();
    let max_depth = plan.blocks.iter().map(|block| block.depth).max()?;
    if proofs.len() != max_depth {
        return None;
    }
    for (layer, proof) in proofs.iter().enumerate() {
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
        let selector = transcript.get_field_challenge(&());
        for ((&block, &(left, right)), claim) in blocks.iter().zip(&values).zip(claims) {
            let mut next_point = point.clone();
            next_point.push(selector);
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
            let depth = plan.blocks[block].depth;
            if point.len() != r2 + depth {
                return None;
            }
            let mut normalized = point[r2..].to_vec();
            normalized.extend_from_slice(&point[..r2]);
            Some(CutClaim {
                block,
                point: normalized,
                value,
            })
        })
        .collect()
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
