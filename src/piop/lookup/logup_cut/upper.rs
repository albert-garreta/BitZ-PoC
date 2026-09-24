use crate::{
    cfg_chunks, cfg_chunks_mut, cfg_iter,
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
    block_forests: Vec<Vec<ProductBatchProof>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DyadicUpperProfile {
    pub forest_build: Duration,
    pub root_merge: Duration,
    pub forest_sumcheck: Duration,
}

pub(crate) struct DyadicUpperScratch {
    forests: Vec<Option<PreparedProductForest>>,
    products: ProductWorkspace,
}

struct PreparedProductForest {
    roots: Vec<Gf>,
    levels: Vec<(Vec<Gf>, Vec<Gf>)>,
    depth: usize,
    tree_vars: usize,
}

impl DyadicUpperScratch {
    pub(crate) fn new(plan: &DyadicPlan, tree_vars: usize) -> Self {
        let columns = 1usize << tree_vars;
        let max_depth = plan.blocks.iter().map(|block| block.depth).max().unwrap_or(0);
        let mut products = ProductWorkspace::default();
        if max_depth != 0 {
            products.reserve(
                1,
                columns << (max_depth - 1),
                tree_vars + max_depth - 1,
            );
        }
        Self {
            forests: plan
                .blocks
                .iter()
                .map(|block| {
                    (block.depth != 0)
                        .then(|| PreparedProductForest::new(block.depth, tree_vars))
                })
                .collect(),
            products,
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.forests
            .iter()
            .flatten()
            .map(PreparedProductForest::retained_bytes)
            .sum::<usize>()
            + self.products.retained_bytes()
    }

    /// Moves every block forest's level buffers into `pool`: they are dead
    /// once the upper GKR has run, and the fraction trees built next want
    /// buffers of the same kind.
    pub(crate) fn lend_levels(&mut self, pool: &mut Vec<Vec<Gf>>) {
        for forest in self.forests.iter_mut().flatten() {
            forest.lend_levels(pool);
        }
    }

    /// Refills the lent level buffers from `pool` (largest block and level
    /// first) before a rebuild.
    pub(crate) fn reclaim_levels(&mut self, pool: &mut Vec<Vec<Gf>>) {
        for forest in self.forests.iter_mut().flatten() {
            forest.reclaim_levels(pool);
        }
    }
}

impl PreparedProductForest {
    fn new(depth: usize, tree_vars: usize) -> Self {
        let columns = 1usize << tree_vars;
        Self {
            roots: vec![Gf::ZERO; columns],
            // Filled by `reclaim_levels` before every rebuild, from the pool
            // the fraction tree returns its buffers to.
            levels: (0..depth).map(|_| (Vec::new(), Vec::new())).collect(),
            depth,
            tree_vars,
        }
    }

    fn lend_levels(&mut self, pool: &mut Vec<Vec<Gf>>) {
        for (left, right) in &mut self.levels {
            for side in [left, right] {
                if side.capacity() != 0 {
                    pool.push(core::mem::take(side));
                }
            }
        }
    }

    /// The length is set by truncation wherever the pooled buffer allows,
    /// so nothing is written before the rebuild overwrites every entry.
    fn reclaim_levels(&mut self, pool: &mut Vec<Vec<Gf>>) {
        let columns = 1usize << self.tree_vars;
        for level in (0..self.depth).rev() {
            let len = columns << level;
            let (left, right) = &mut self.levels[level];
            for side in [left, right] {
                if side.capacity() < len {
                    *side = super::fraction::take_fitting(pool, len);
                }
                if side.len() >= len {
                    side.truncate(len);
                } else {
                    side.resize(len, Gf::ZERO);
                }
            }
        }
    }

    fn rebuild(&mut self, leaf: impl Fn(usize, usize) -> Gf + Sync) {
        let columns = 1usize << self.tree_vars;
        let top = self.depth - 1;
        let half = 1usize << top;
        let (left, right) = &mut self.levels[top];
        cfg_chunks_mut!(left, half)
            .zip(cfg_chunks_mut!(right, half))
            .enumerate()
            .for_each(|(column, (left, right))| {
                for index in 0..half {
                    left[index] = leaf(column, index);
                    right[index] = leaf(column, index + half);
                }
            });

        for level in (1..self.depth).rev() {
            let child_len = 1usize << level;
            let parent_len = child_len >> 1;
            let (parents, children) = self.levels.split_at_mut(level);
            let (parent_left, parent_right) = &mut parents[level - 1];
            let (child_left, child_right) = &children[0];
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

        let (left, right) = &self.levels[0];
        debug_assert_eq!(left.len(), columns);
        for (root, (&left, &right)) in self.roots.iter_mut().zip(left.iter().zip(right)) {
            *root = left * right;
        }
    }

    fn retained_bytes(&self) -> usize {
        (self.roots.capacity()
            + self
                .levels
                .iter()
                .map(|(left, right)| left.capacity() + right.capacity())
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
            self.block_forests
                .iter()
                .flatten()
                .map(ProductBatchProof::proof_size_bytes)
                .sum(),
        )
    }
}

fn prove_product_forest_from_claim(
    transcript: &mut impl Transcript,
    forest: &PreparedProductForest,
    mut point: Vec<Gf>,
    mut value: Gf,
    workspace: &mut ProductWorkspace,
) -> (Vec<ProductBatchProof>, Vec<Gf>, Gf) {
    debug_assert_eq!(point.len(), forest.tree_vars);
    let mut proofs = Vec::with_capacity(forest.depth);
    for (level, (left, right)) in forest.levels.iter().enumerate() {
        debug_assert_eq!(point.len(), forest.tree_vars + level);
        let (proof, next_point, values) = prove_product_batch(
            transcript,
            &[(point, value)],
            &[(ProductInput::Table(left), ProductInput::Table(right))],
            workspace,
        );
        let (left, right) = values[0];
        let selector = transcript.get_field_challenge::<Gf>(&());
        point = next_point;
        point.insert(level, selector);
        value = left + selector * (left + right);
        proofs.push(proof);
    }
    (proofs, point, value)
}

fn verify_product_forest_from_claim(
    transcript: &mut impl Transcript,
    proofs: &[ProductBatchProof],
    mut point: Vec<Gf>,
    mut value: Gf,
    depth: usize,
) -> Option<(Vec<Gf>, Gf)> {
    if proofs.len() != depth {
        return None;
    }
    let tree_vars = point.len();
    for (level, proof) in proofs.iter().enumerate() {
        if point.len() != tree_vars + level {
            return None;
        }
        let (next_point, values) = verify_product_batch(transcript, &[(point, value)], proof)?;
        let (left, right) = values[0];
        let selector = transcript.get_field_challenge::<Gf>(&());
        point = next_point;
        point.insert(level, selector);
        value = left + selector * (left + right);
    }
    Some((point, value))
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
    assert_eq!(scratch.forests.len(), plan.blocks.len());

    let mut block_roots = Vec::with_capacity(plan.blocks.len());
    for (block, forest) in plan.blocks.iter().zip(&mut scratch.forests) {
        let started = Instant::now();
        if block.depth == 0 {
            block_roots.push(
                (0..columns)
                    .map(|column| cut_value(column, block.chunk_start))
                    .collect(),
            );
            assert!(forest.is_none());
        } else {
            let forest = forest.as_mut().expect("preallocated block forest");
            forest.rebuild(|column, local_chunk| {
                cut_value(column, block.chunk_start + local_chunk)
            });
            block_roots.push(forest.roots.clone());
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
            let started = Instant::now();
            let (proof, point, value) = prove_product_forest_from_claim(
                transcript,
                forest,
                point,
                value,
                &mut scratch.products,
            );
            profile.forest_sumcheck += started.elapsed();
            block_forests.push(proof);
            claims.push(CutClaim {
                block: block_index,
                point,
                value,
            });
        } else {
            debug_assert_eq!(block.depth, 0);
            block_forests.push(Vec::new());
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
        let (point, value) = if block.depth == 0 {
            if !forest.is_empty() {
                return None;
            }
            (point, value)
        } else {
            verify_product_forest_from_claim(transcript, forest, point, value, block.depth)?
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
