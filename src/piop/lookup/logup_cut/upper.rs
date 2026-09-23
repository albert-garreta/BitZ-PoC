use crate::{
    cfg_iter,
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::{MLSumcheck, SumcheckProof},
    },
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::eq_eval},
    transcript::traits::{Transcribable, Transcript},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{CutClaim, DyadicPlan};

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

#[derive(Clone, Copy)]
struct MergeNode {
    children: Option<(usize, usize)>,
    level: usize,
}

pub fn prove_dyadic_upper(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    root_point: &[Gf],
    root_value: Gf,
    cut_values: &[Gf],
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
        prove_root_product(transcript, root_point, root_value, block_roots)
    };
    let (claims, block_layers) =
        prove_block_forests(transcript, plan, r2, block_claims, witnesses);
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

fn merge_shape(leaves: usize) -> (Vec<MergeNode>, usize) {
    let mut nodes = vec![MergeNode {
        children: None,
        level: 0,
    }; leaves];
    let mut current = (0..leaves).collect::<Vec<_>>();
    let mut level = 1;
    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        for pair in current.chunks(2) {
            if pair.len() == 1 {
                next.push(pair[0]);
            } else {
                let node = nodes.len();
                nodes.push(MergeNode {
                    children: Some((pair[0], pair[1])),
                    level,
                });
                next.push(node);
            }
        }
        current = next;
        level += 1;
    }
    (nodes, current[0])
}

fn prove_root_product(
    transcript: &mut impl Transcript,
    root_point: &[Gf],
    root_value: Gf,
    block_roots: Vec<Vec<Gf>>,
) -> (Vec<(Vec<Gf>, Gf)>, Vec<ProductBatchProof>) {
    let leaves = block_roots.len();
    let (nodes, root) = merge_shape(leaves);
    let mut tables = block_roots.into_iter().map(Some).collect::<Vec<_>>();
    tables.resize_with(nodes.len(), || None);
    for node in leaves..nodes.len() {
        let (left, right) = nodes[node].children.unwrap();
        tables[node] = Some(
            cfg_iter!(tables[left].as_ref().unwrap())
                .zip(cfg_iter!(tables[right].as_ref().unwrap()))
                .map(|(&left, &right)| left * right)
                .collect(),
        );
    }

    let max_level = nodes.iter().map(|node| node.level).max().unwrap();
    let mut active = vec![None; nodes.len()];
    active[root] = Some((root_point.to_vec(), root_value));
    let mut proofs = Vec::with_capacity(max_level);
    for level in (1..=max_level).rev() {
        let batch = (leaves..nodes.len())
            .filter(|&node| nodes[node].level == level && active[node].is_some())
            .collect::<Vec<_>>();
        if batch.is_empty() {
            continue;
        }
        let claims = batch
            .iter()
            .map(|&node| active[node].take().unwrap())
            .collect::<Vec<_>>();
        let inputs = batch
            .iter()
            .map(|&node| {
                let (left, right) = nodes[node].children.unwrap();
                (tables[left].take().unwrap(), tables[right].take().unwrap())
            })
            .collect();
        let (proof, point, values) = prove_product_batch(transcript, &claims, inputs);
        proofs.push(proof);
        for (index, &node) in batch.iter().enumerate() {
            let (left, right) = nodes[node].children.unwrap();
            active[left] = Some((point.clone(), values[index].0));
            active[right] = Some((point.clone(), values[index].1));
        }
    }
    (
        active[..leaves]
            .iter_mut()
            .map(|claim| claim.take().unwrap())
            .collect(),
        proofs,
    )
}

fn verify_root_product(
    transcript: &mut impl Transcript,
    root_point: &[Gf],
    root_value: Gf,
    leaves: usize,
    proofs: &[ProductBatchProof],
) -> Option<Vec<(Vec<Gf>, Gf)>> {
    let (nodes, root) = merge_shape(leaves);
    let max_level = nodes.iter().map(|node| node.level).max()?;
    let mut active = vec![None; nodes.len()];
    active[root] = Some((root_point.to_vec(), root_value));
    let mut proof_index = 0;
    for level in (1..=max_level).rev() {
        let batch = (leaves..nodes.len())
            .filter(|&node| nodes[node].level == level && active[node].is_some())
            .collect::<Vec<_>>();
        if batch.is_empty() {
            continue;
        }
        let claims = batch
            .iter()
            .map(|&node| active[node].take().unwrap())
            .collect::<Vec<_>>();
        let (point, values) = verify_product_batch(
            transcript,
            &claims,
            proofs.get(proof_index)?,
        )?;
        proof_index += 1;
        for (index, &node) in batch.iter().enumerate() {
            let (left, right) = nodes[node].children.unwrap();
            active[left] = Some((point.clone(), values[index].0));
            active[right] = Some((point.clone(), values[index].1));
        }
    }
    if proof_index != proofs.len() {
        return None;
    }
    active[..leaves].iter_mut().map(Option::take).collect()
}

fn prove_block_forests(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    block_claims: Vec<(Vec<Gf>, Gf)>,
    mut witnesses: Vec<Vec<Vec<Gf>>>,
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
        let inputs = blocks
            .iter()
            .map(|&block| {
                let child_level = plan.blocks[block].depth - layer - 1;
                let mut child = std::mem::take(&mut witnesses[block][child_level]);
                let right = child.split_off(child.len() / 2);
                (child, right)
            })
            .collect();
        let (proof, point, values) = prove_product_batch(transcript, &claims, inputs);
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
    inputs: Vec<(Vec<Gf>, Vec<Gf>)>,
) -> (ProductBatchProof, Vec<Gf>, Vec<(Gf, Gf)>) {
    assert!(!claims.is_empty() && claims.len() == inputs.len());
    let dimension = claims[0].0.len();
    assert!(claims.iter().all(|claim| claim.0.len() == dimension));
    assert!(inputs.iter().all(|(left, right)| {
        left.len() == 1usize << dimension && right.len() == left.len()
    }));
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

    if dimension == 0 {
        let evals = inputs.into_iter().map(|(l, r)| (l[0], r[0])).collect::<Vec<_>>();
        assert_eq!(claimed_sum, evals.iter().zip(&scales).map(|(&(l, r), &s)| s * l * r).sum());
        let flat = evals.iter().flat_map(|&(l, r)| [l, r]).collect::<Vec<_>>();
        absorb_field_slice(transcript, &flat);
        return (ProductBatchProof { sumcheck: None, evals: evals.clone() }, Vec::new(), evals);
    }

    let shared = claims[1..].iter().all(|claim| claim.0 == claims[0].0);
    let groups = claims
        .iter()
        .zip(inputs)
        .zip(scales)
        .map(|((claim, (left, right)), scale)| {
            crate::piop::sumcheck::eq_factored::EqInnerGroupMixed {
                q: claim.0.as_slice().into(),
                scale,
                bufs: crate::piop::sumcheck::eq_factored::GroupBufs::Dense(vec![(left, right)]),
            }
        })
        .collect();
    let (sumcheck, point, final_evals) = if shared {
        crate::piop::sumcheck::eq_factored::prove_eq_inner_sumcheck_mixed_gruen(
            transcript, groups, &[], &[], &[], &(),
        )
    } else {
        crate::piop::sumcheck::eq_factored::prove_eq_inner_sumcheck_mixed(
            transcript, groups, &[], &[], &[], &(),
        )
    };
    debug_assert_eq!(sumcheck.claimed_sum, claimed_sum);
    let evals = final_evals.into_iter().map(|mut evals| evals.remove(0)).collect::<Vec<_>>();
    let flat = evals.iter().flat_map(|&(l, r)| [l, r]).collect::<Vec<_>>();
    absorb_field_slice(transcript, &flat);
    (
        ProductBatchProof {
            sumcheck: Some(sumcheck),
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
        let shared = claims[1..].iter().all(|claim| claim.0 == claims[0].0);
        let subclaim = if shared {
            crate::piop::sumcheck::eq_factored::verify_eq_inner_sumcheck_gruen(
                transcript,
                &claims[0].0,
                sumcheck,
                &(),
            )
            .ok()?
        } else {
            MLSumcheck::verify_as_subprotocol(transcript, dimension, 3, sumcheck, &()).ok()?
        };
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
