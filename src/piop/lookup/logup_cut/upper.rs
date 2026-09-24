use crate::{
    cfg_iter,
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::{
            MLSumcheck, SumcheckProof,
            prover::{NatEvaluatedPolyWithoutConstant, ProverMsg},
        },
    },
    poly::{
        coefficient::PolynomialField,
        univariate::binary_gf128::Gf128 as Gf,
        utils::eq_eval,
    },
    transcript::traits::{Transcribable, Transcript},
    utils::wide_mul::WideMulAcc,
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

struct ProductScratch {
    groups: Vec<ProductGroupScratch>,
    grids: Vec<[Gf; 9]>,
    eq_low: Vec<Gf>,
    eq_high: Vec<Gf>,
}

#[derive(Default)]
struct ProductGroupScratch {
    left_back: Vec<Gf>,
    right_back: Vec<Gf>,
}

impl ProductScratch {
    fn new(
        dimension: usize,
        group_dimensions: impl IntoIterator<Item = usize>,
    ) -> Self {
        let groups = group_dimensions
            .into_iter()
            .map(|dimension| {
                let back_len = if dimension == 0 {
                    0
                } else {
                    1usize << (dimension - 1)
                };
                ProductGroupScratch {
                    left_back: Vec::with_capacity(back_len),
                    right_back: Vec::with_capacity(back_len),
                }
            })
            .collect::<Vec<_>>();
        let grid_capacity = groups.len();
        Self {
            groups,
            grids: Vec::with_capacity(grid_capacity),
            eq_low: Vec::with_capacity(1usize << (dimension / 2)),
            eq_high: Vec::with_capacity(1usize << dimension.div_ceil(2)),
        }
    }
}

#[derive(Clone, Debug)]
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
            self.block_layers
                .iter()
                .map(ProductBatchProof::proof_size_bytes)
                .sum(),
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
    cut_value: impl Fn(usize, usize) -> Gf + Sync,
) -> (DyadicUpperProof, Vec<CutClaim>) {
    bind_plan(transcript, plan, r2);
    let columns = 1usize << r2;
    assert_eq!(root_point.len(), r2);
    let cut_value = &cut_value;

    let mut block_roots = Vec::with_capacity(plan.blocks.len());
    let mut witnesses = Vec::with_capacity(plan.blocks.len());
    for block in &plan.blocks {
        let leaves = (0..block.n_chunks)
            .flat_map(|local_chunk| {
                (0..columns)
                    .map(move |column| cut_value(column, block.chunk_start + local_chunk))
            })
            .collect::<Vec<_>>();
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

    let max_depth = plan.blocks.iter().map(|block| block.depth).max().unwrap_or(0);
    let max_dimension = r2 + max_depth.saturating_sub(1);
    let mut scratch = ProductScratch::new(
        max_dimension,
        plan.blocks
            .iter()
            .map(|block| r2 + block.depth.saturating_sub(1)),
    );
    let (block_claims, root_merge) = if block_roots.len() == 1 {
        (vec![(root_point.to_vec(), root_value)], Vec::new())
    } else {
        prove_root_product(
            transcript,
            root_point,
            root_value,
            block_roots,
            &mut scratch,
        )
    };
    let (claims, block_layers) = prove_block_forests(
        transcript,
        plan,
        r2,
        block_claims,
        witnesses,
        &mut scratch,
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
    scratch: &mut ProductScratch,
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
        let (proof, point, values) = prove_product_batch(transcript, &claims, inputs, scratch);
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
    scratch: &mut ProductScratch,
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
                let mut child = core::mem::take(&mut witnesses[block][child_level]);
                let right = child.split_off(child.len() / 2);
                (child, right)
            })
            .collect();
        let (proof, point, values) =
            prove_product_batch(transcript, &claims, inputs, scratch);
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
    mut inputs: Vec<(Vec<Gf>, Vec<Gf>)>,
    scratch: &mut ProductScratch,
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

    assert!(scratch.groups.len() >= inputs.len());
    let ProductScratch {
        groups: group_scratch,
        grids,
        eq_low,
        eq_high,
    } = scratch;
    let mut groups = claims
        .iter()
        .zip(scales)
        .zip(inputs.iter_mut())
        .zip(group_scratch.iter_mut())
        .map(|(((claim, scale), (left, right)), scratch)| {
            let needed = left.len() / 2;
            assert!(scratch.left_back.capacity() >= needed);
            assert!(scratch.right_back.capacity() >= needed);
            ProductGroup {
                point: &claim.0,
                prefix: scale,
                left,
                right,
                left_back: &mut scratch.left_back,
                right_back: &mut scratch.right_back,
            }
        })
        .collect::<Vec<_>>();
    prove_product_groups(
        transcript,
        claimed_sum,
        &mut groups,
        grids,
        eq_low,
        eq_high,
    )
}

pub(super) fn prove_product_single(
    transcript: &mut impl Transcript,
    claim_point: &[Gf],
    claimed_sum: Gf,
    left: &mut Vec<Gf>,
    right: &mut Vec<Gf>,
    left_back: &mut Vec<Gf>,
    right_back: &mut Vec<Gf>,
    grids: &mut Vec<[Gf; 9]>,
    eq_low: &mut Vec<Gf>,
    eq_high: &mut Vec<Gf>,
) -> (ProductBatchProof, Vec<Gf>, (Gf, Gf)) {
    assert!(left_back.capacity() >= left.len() / 2);
    assert!(right_back.capacity() >= right.len() / 2);
    let mut groups = [ProductGroup {
        point: claim_point,
        prefix: Gf::ONE,
        left,
        right,
        left_back,
        right_back,
    }];
    let (proof, point, mut evals) = prove_product_groups(
        transcript,
        claimed_sum,
        &mut groups,
        grids,
        eq_low,
        eq_high,
    );
    (proof, point, evals.pop().unwrap())
}

struct ProductGroup<'a> {
    point: &'a [Gf],
    prefix: Gf,
    left: &'a mut Vec<Gf>,
    right: &'a mut Vec<Gf>,
    left_back: &'a mut Vec<Gf>,
    right_back: &'a mut Vec<Gf>,
}

fn prove_product_groups(
    transcript: &mut impl Transcript,
    claimed_sum: Gf,
    groups: &mut [ProductGroup<'_>],
    grids: &mut Vec<[Gf; 9]>,
    eq_low: &mut Vec<Gf>,
    eq_high: &mut Vec<Gf>,
) -> (ProductBatchProof, Vec<Gf>, Vec<(Gf, Gf)>) {
    let dimension = groups[0].point.len();
    assert!(groups.iter().all(|group| {
        group.point.len() == dimension
            && group.left.len() == 1usize << dimension
            && group.right.len() == group.left.len()
    }));
    if dimension == 0 {
        let evals = groups
            .iter()
            .map(|group| (group.left[0], group.right[0]))
            .collect::<Vec<_>>();
        assert_eq!(
            claimed_sum,
            groups
                .iter()
                .zip(&evals)
                .map(|(group, &(left, right))| group.prefix * left * right)
                .sum(),
        );
        let flat = evals.iter().flat_map(|&(left, right)| [left, right]).collect::<Vec<_>>();
        absorb_field_slice(transcript, &flat);
        return (
            ProductBatchProof {
                sumcheck: None,
                evals: evals.clone(),
            },
            Vec::new(),
            evals,
        );
    }

    let mut transcript_buf = [0u8; 16];
    transcript.absorb_random_field(&Gf::interpolation_node(dimension as u64, &()), &mut transcript_buf);
    transcript.absorb_random_field(&Gf::interpolation_node(3, &()), &mut transcript_buf);
    let mut point = Vec::with_capacity(dimension);
    let mut messages = Vec::with_capacity(dimension);
    let shared = groups[1..]
        .iter()
        .all(|group| group.point == groups[0].point);

    let mut first = [Gf::ZERO; 4];
    let mut first_claim = Gf::ZERO;
    for group in groups.iter_mut() {
        prepare_eq_sqrt(&group.point[1..], eq_low, eq_high);
        let coeffs = product_round_sqrt(
            group.left,
            group.right,
            eq_low,
            eq_high,
            1usize << (dimension - 1),
        );
        first_claim += group.prefix
            * (coeffs[0] + group.point[0] * (coeffs[1] + coeffs[2]));
        add_product_coefficients(
            &mut first,
            coeffs,
            group.prefix,
            group.point[0],
            shared,
        );
    }
    debug_assert_eq!(claimed_sum, first_claim);
    let mut pending = [product_round_challenge(
        transcript,
        &mut transcript_buf,
        &mut messages,
        first,
        shared,
    ), Gf::ZERO];
    point.push(pending[0]);
    for group in groups.iter_mut() {
        group.prefix *= (Gf::ONE + group.point[0]) * (Gf::ONE + pending[0])
            + group.point[0] * pending[0];
    }

    let mut pending_len = 1;
    let mut round = 1;
    let evals: Vec<(Gf, Gf)> = loop {
        if round == dimension {
            let challenge = pending[0];
            break groups
                .iter()
                .map(|group| {
                    (
                        group.left[0] + challenge * (group.left[0] + group.left[1]),
                        group.right[0] + challenge * (group.right[0] + group.right[1]),
                    )
                })
                .collect();
        }
        if round + 1 == dimension {
            let tail = |values: &[Gf]| match pending_len {
                1 => [
                    values[0] + pending[0] * (values[0] + values[1]),
                    values[2] + pending[0] * (values[2] + values[3]),
                ],
                2 => {
                    let a0 = values[0] + pending[0] * (values[0] + values[1]);
                    let a1 = values[2] + pending[0] * (values[2] + values[3]);
                    let b0 = values[4] + pending[0] * (values[4] + values[5]);
                    let b1 = values[6] + pending[0] * (values[6] + values[7]);
                    [
                        a0 + pending[1] * (a0 + a1),
                        b0 + pending[1] * (b0 + b1),
                    ]
                }
                _ => unreachable!(),
            };
            let mut coeffs = [Gf::ZERO; 4];
            for group in groups.iter() {
                let [l0, l1] = tail(group.left);
                let [r0, r1] = tail(group.right);
                let a0 = l0 * r0;
                let a2 = (l0 + l1) * (r0 + r1);
                let contribution = [a0, l1 * r1 + a0 + a2, a2];
                add_product_coefficients(
                    &mut coeffs,
                    contribution,
                    group.prefix,
                    group.point[round],
                    shared,
                );
            }
            let challenge = product_round_challenge(
                transcript,
                &mut transcript_buf,
                &mut messages,
                coeffs,
                shared,
            );
            point.push(challenge);
            break groups
                .iter()
                .map(|group| {
                    let [l0, l1] = tail(group.left);
                    let [r0, r1] = tail(group.right);
                    (
                        l0 + challenge * (l0 + l1),
                        r0 + challenge * (r0 + r1),
                    )
                })
                .collect();
        }

        let quads = 1usize << (dimension - round - 2);
        grids.clear();
        for group in groups.iter_mut() {
            prepare_eq_sqrt(&group.point[round + 2..], eq_low, eq_high);
            group.left_back.resize(4 * quads, Gf::ZERO);
            group.right_back.resize(4 * quads, Gf::ZERO);
            grids.push(product_grid_sqrt(
                group.left,
                group.right,
                group.left_back,
                group.right_back,
                &pending[..pending_len],
                eq_low,
                eq_high,
                quads,
            ));
            core::mem::swap(group.left, group.left_back);
            core::mem::swap(group.right, group.right_back);
        }

        let mut current = [Gf::ZERO; 4];
        for (group, grid) in groups.iter().zip(grids.iter()) {
            let q_next = group.point[round + 1];
            let contribution = core::array::from_fn(|u| {
                (Gf::ONE + q_next) * grid[3 * u] + q_next * grid[3 * u + 1]
            });
            add_product_coefficients(
                &mut current,
                contribution,
                group.prefix,
                group.point[round],
                shared,
            );
        }
        let first_challenge = product_round_challenge(
            transcript,
            &mut transcript_buf,
            &mut messages,
            current,
            shared,
        );
        point.push(first_challenge);
        for group in groups.iter_mut() {
            group.prefix *= (Gf::ONE + group.point[round]) * (Gf::ONE + first_challenge)
                + group.point[round] * first_challenge;
        }

        let challenge_square = first_challenge.square();
        let mut next = [Gf::ZERO; 4];
        for (group, grid) in groups.iter().zip(grids.iter()) {
            let nodes = core::array::from_fn::<_, 3, _>(|v| {
                grid[v] + first_challenge * grid[3 + v] + challenge_square * grid[6 + v]
            });
            let contribution = [nodes[0], nodes[1] + nodes[0] + nodes[2], nodes[2]];
            add_product_coefficients(
                &mut next,
                contribution,
                group.prefix,
                group.point[round + 1],
                shared,
            );
        }
        let second_challenge = product_round_challenge(
            transcript,
            &mut transcript_buf,
            &mut messages,
            next,
            shared,
        );
        point.push(second_challenge);
        for group in groups.iter_mut() {
            group.prefix *= (Gf::ONE + group.point[round + 1]) * (Gf::ONE + second_challenge)
                + group.point[round + 1] * second_challenge;
        }

        if round + 2 == dimension {
            let fold_pair = |values: &[Gf]| {
                let low = values[0] + first_challenge * (values[0] + values[1]);
                let high = values[2] + first_challenge * (values[2] + values[3]);
                low + second_challenge * (low + high)
            };
            break groups
                .iter()
                .map(|group| (fold_pair(group.left), fold_pair(group.right)))
                .collect();
        }
        pending = [first_challenge, second_challenge];
        pending_len = 2;
        round += 2;
    };

    if ((dimension - 1) / 2) & 1 == 1 {
        for group in groups.iter_mut() {
            core::mem::swap(group.left, group.left_back);
            core::mem::swap(group.right, group.right_back);
        }
    }
    let sumcheck = SumcheckProof {
        messages,
        claimed_sum,
    };
    let flat = evals.iter().flat_map(|&(left, right)| [left, right]).collect::<Vec<_>>();
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

fn product_round_sqrt(
    left: &[Gf],
    right: &[Gf],
    eq_low: &[Gf],
    eq_high: &[Gf],
    half: usize,
) -> [Gf; 3] {
    let low_len = eq_low.len();
    let high_len = half.div_ceil(low_len);
    let block = |high: usize| {
        let start = high * low_len;
        let len = (half - start).min(low_len);
        let coeffs = <Gf as WideMulAcc>::eqf_single_pair_round(
            &left[2 * start..2 * (start + len)],
            &right[2 * start..2 * (start + len)],
            &eq_low[..len],
            len,
        )
        .expect("GF(2^128) single-pair kernel");
        [coeffs.0, coeffs.1, coeffs.2].map(|coefficient| {
            <Gf as WideMulAcc>::mul_wide(&coefficient, &eq_high[high])
        })
    };
    reduce_round_blocks(high_len, block)
}

fn product_grid_sqrt(
    left: &mut [Gf],
    right: &mut [Gf],
    left_out: &mut [Gf],
    right_out: &mut [Gf],
    pending: &[Gf],
    eq_low: &[Gf],
    eq_high: &[Gf],
    quads: usize,
) -> [Gf; 9] {
    let low_len = eq_low.len();
    let input_arity = 4 << pending.len();
    let input_chunk = input_arity * low_len;
    let output_chunk = 4 * low_len;
    let block = |high: usize,
                 left: &mut [Gf],
                 right: &mut [Gf],
                 left_out: &mut [Gf],
                 right_out: &mut [Gf]| {
        let len = left_out.len() / 4;
        let grid = <Gf as WideMulAcc>::eqf_grid_pass(
            left,
            right,
            pending,
            &eq_low[..len],
            len,
        )
        .expect("GF(2^128) grid kernel");
        left_out.copy_from_slice(&left[..4 * len]);
        right_out.copy_from_slice(&right[..4 * len]);
        grid.map(|coefficient| {
            <Gf as WideMulAcc>::mul_wide(&coefficient, &eq_high[high])
        })
    };

    #[cfg(feature = "parallel")]
    let blocks = left[..input_arity * quads]
        .par_chunks_mut(input_chunk)
        .zip(right[..input_arity * quads].par_chunks_mut(input_chunk))
        .zip(left_out[..4 * quads].par_chunks_mut(output_chunk))
        .zip(right_out[..4 * quads].par_chunks_mut(output_chunk))
        .enumerate()
        .map(|(high, (((left, right), left_out), right_out))| {
            block(high, left, right, left_out, right_out)
        })
        .reduce(wide_zero, wide_add);
    #[cfg(not(feature = "parallel"))]
    let blocks = left[..input_arity * quads]
        .chunks_mut(input_chunk)
        .zip(right[..input_arity * quads].chunks_mut(input_chunk))
        .zip(left_out[..4 * quads].chunks_mut(output_chunk))
        .zip(right_out[..4 * quads].chunks_mut(output_chunk))
        .enumerate()
        .map(|(high, (((left, right), left_out), right_out))| {
            block(high, left, right, left_out, right_out)
        })
        .fold(wide_zero(), wide_add);
    blocks.map(<Gf as WideMulAcc>::from_wide)
}

fn reduce_round_blocks(
    high_len: usize,
    block: impl Fn(usize) -> [<Gf as WideMulAcc>::Wide; 3] + Sync + Send,
) -> [Gf; 3] {
    #[cfg(feature = "parallel")]
    let blocks = (0..high_len)
        .into_par_iter()
        .map(block)
        .reduce(wide_zero, wide_add);
    #[cfg(not(feature = "parallel"))]
    let blocks = (0..high_len)
        .map(block)
        .fold(wide_zero(), wide_add);
    blocks.map(<Gf as WideMulAcc>::from_wide)
}

fn wide_zero<const N: usize>() -> [<Gf as WideMulAcc>::Wide; N] {
    core::array::from_fn(|_| <Gf as WideMulAcc>::wide_zero(&Gf::ZERO))
}

fn wide_add<const N: usize>(
    mut left: [<Gf as WideMulAcc>::Wide; N],
    right: [<Gf as WideMulAcc>::Wide; N],
) -> [<Gf as WideMulAcc>::Wide; N] {
    for (left, right) in left.iter_mut().zip(&right) {
        <Gf as WideMulAcc>::wide_add_assign(left, right);
    }
    left
}

fn add_product_coefficients(
    sum: &mut [Gf; 4],
    product: [Gf; 3],
    prefix: Gf,
    coordinate: Gf,
    shared: bool,
) {
    let [a0, a1, a2] = product.map(|coefficient| prefix * coefficient);
    if shared {
        sum[0] += a0;
        sum[1] += a1;
        sum[2] += a2;
    } else {
        let eq0 = Gf::ONE + coordinate;
        sum[0] += eq0 * a0;
        sum[1] += eq0 * a1 + a0;
        sum[2] += eq0 * a2 + a1;
        sum[3] += a2;
    }
}

fn product_round_challenge(
    transcript: &mut impl Transcript,
    transcript_buf: &mut [u8; 16],
    messages: &mut Vec<ProverMsg<Gf>>,
    coeffs: [Gf; 4],
    shared: bool,
) -> Gf {
    let tail = if shared {
        vec![coeffs[1], coeffs[2]]
    } else {
        vec![coeffs[1], coeffs[2], coeffs[3]]
    };
    transcript.absorb_random_field_slice(&tail, transcript_buf);
    messages.push(ProverMsg(NatEvaluatedPolyWithoutConstant::new(tail)));
    let challenge = transcript.get_field_challenge::<Gf>(&());
    transcript.absorb_random_field(&challenge, transcript_buf);
    challenge
}

fn prepare_eq_sqrt(point: &[Gf], low: &mut Vec<Gf>, high: &mut Vec<Gf>) {
    let split = point.len() / 2;
    fill_eq(&point[..split], low);
    fill_eq(&point[split..], high);
}

fn fill_eq(point: &[Gf], out: &mut Vec<Gf>) {
    out.clear();
    out.push(Gf::ONE);
    for &coordinate in point {
        let len = out.len();
        out.resize(2 * len, Gf::ZERO);
        let (low, high) = out.split_at_mut(len);
        for (low, high) in low.iter_mut().zip(high) {
            let value = *low;
            *low = value * (Gf::ONE + coordinate);
            *high = value * coordinate;
        }
    }
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
