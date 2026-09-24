use crate::{
    piop::lookup::gkr_product::absorb_field_slice,
    poly::univariate::binary_gf128::Gf128 as Gf,
    transcript::traits::Transcript,
    utils::wide_mul::WideMulAcc,
};

use super::{
    product::{ProductInput, ProductWorkspace},
    upper::{ProductBatchProof, prove_product_batch, verify_product_batch},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const DOMAIN: &[u8] = b"bitz/logup-cut/rational/v1";
#[cfg(feature = "parallel")]
const PARALLEL_THRESHOLD: usize = 1 << 13;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionClaim {
    pub point: Vec<Gf>,
    pub num: Gf,
    pub den: Gf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalClaims {
    pub left: FractionClaim,
    pub right: FractionClaim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FractionTreeProof {
    ac_evals: Vec<Gf>,
    products: Vec<ProductBatchProof>,
    numerators: Vec<(Gf, Gf)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalProof {
    roots: [Gf; 4],
    left: FractionTreeProof,
    right: FractionTreeProof,
}

impl FractionTreeProof {
    fn proof_size_bytes(&self) -> usize {
        (self.ac_evals.len() + 2 * self.numerators.len()) * 16
            + self.products.iter().map(ProductBatchProof::proof_size_bytes).sum::<usize>()
    }
}

impl RationalProof {
    pub(crate) fn proof_size_bytes(&self) -> usize {
        4 * 16 + self.left.proof_size_bytes() + self.right.proof_size_bytes()
    }
}

#[derive(Clone, Debug, Default)]
pub struct FractionTreeWitness {
    nums: Vec<Vec<Gf>>,
    dens: Vec<Vec<Gf>>,
    ac: Vec<Vec<Gf>>,
    num_padding: Vec<Gf>,
    den_padding: Vec<Gf>,
    ac_padding: Vec<Gf>,
    products: Vec<Gf>,
    eq_low: Vec<Gf>,
    eq_high: Vec<Gf>,
}

impl FractionTreeWitness {
    pub fn rebuild(
        &mut self,
        numerator: &[Gf],
        denominator: &[Gf],
        dimension: usize,
        numerator_padding: Gf,
        denominator_padding: Gf,
    ) {
        let full_len = 1usize << dimension;
        assert!(!numerator.is_empty() && numerator.len() <= full_len);
        assert_eq!(numerator.len(), denominator.len());
        assert_ne!(denominator_padding, Gf::ZERO);
        assert!(denominator.iter().all(|value| *value != Gf::ZERO));

        self.nums.resize_with(dimension + 1, Vec::new);
        self.nums.truncate(dimension + 1);
        self.dens.resize_with(dimension + 1, Vec::new);
        self.dens.truncate(dimension + 1);
        self.ac.resize_with(dimension, Vec::new);
        self.ac.truncate(dimension);
        self.num_padding.resize(dimension + 1, Gf::ZERO);
        self.den_padding.resize(dimension + 1, Gf::ZERO);
        self.ac_padding.resize(dimension, Gf::ZERO);
        self.products.resize(full_len, Gf::ZERO);
        self.eq_low
            .reserve((1usize << (dimension / 2)).saturating_sub(self.eq_low.capacity()));
        self.eq_high.reserve(
            (1usize << dimension.div_ceil(2)).saturating_sub(self.eq_high.capacity()),
        );
        self.nums[0].clear();
        self.nums[0].extend_from_slice(numerator);
        self.dens[0].clear();
        self.dens[0].extend_from_slice(denominator);
        self.num_padding.fill(Gf::ZERO);
        self.den_padding.fill(Gf::ZERO);
        self.ac_padding.fill(Gf::ZERO);
        self.num_padding[0] = numerator_padding;
        self.den_padding[0] = denominator_padding;

        for level in 0..dimension {
            let parent_len = self.nums[level].len().div_ceil(2);
            let num_padding = self.num_padding[level];
            let den_padding = self.den_padding[level];
            self.nums[level + 1].resize(parent_len, Gf::ZERO);
            self.dens[level + 1].resize(parent_len, Gf::ZERO);
            self.ac[level].resize(parent_len, Gf::ZERO);
            let (child_nums, parent_nums) = self.nums.split_at_mut(level + 1);
            let (child_dens, parent_dens) = self.dens.split_at_mut(level + 1);
            let child_num = &child_nums[level];
            let child_den = &child_dens[level];
            let next_num = &mut parent_nums[0];
            let next_den = &mut parent_dens[0];
            let next_ac = &mut self.ac[level];
            let fill = |index: usize, num: &mut Gf, den: &mut Gf, ac_out: &mut Gf| {
                let even = 2 * index;
                let a = child_num[even];
                let b = child_den[even];
                let c = child_num.get(even + 1).copied().unwrap_or(num_padding);
                let d = child_den.get(even + 1).copied().unwrap_or(den_padding);
                let ac = a * c;
                let bd = b * d;
                *num = (a + b) * (c + d) + ac + bd;
                *den = bd;
                *ac_out = ac;
            };
            #[cfg(feature = "parallel")]
            if parent_len >= PARALLEL_THRESHOLD {
                next_num
                    .par_iter_mut()
                    .zip(next_den.par_iter_mut())
                    .zip(next_ac.par_iter_mut())
                    .enumerate()
                    .for_each(|(index, ((num, den), ac))| fill(index, num, den, ac));
            } else {
                next_num
                    .iter_mut()
                    .zip(next_den.iter_mut())
                    .zip(next_ac.iter_mut())
                    .enumerate()
                    .for_each(|(index, ((num, den), ac))| fill(index, num, den, ac));
            }
            #[cfg(not(feature = "parallel"))]
            next_num
                .iter_mut()
                .zip(next_den.iter_mut())
                .zip(next_ac.iter_mut())
                .enumerate()
                .for_each(|(index, ((num, den), ac))| fill(index, num, den, ac));

            self.ac_padding[level] = self.num_padding[level].square();
            self.num_padding[level + 1] = Gf::ZERO;
            self.den_padding[level + 1] = self.den_padding[level].square();
        }
    }

    pub fn retained_bytes(&self) -> usize {
        (self.nums.iter().map(Vec::capacity).sum::<usize>()
            + self.dens.iter().map(Vec::capacity).sum::<usize>()
            + self.ac.iter().map(Vec::capacity).sum::<usize>()
            + self.num_padding.capacity()
            + self.den_padding.capacity()
            + self.ac_padding.capacity()
            + self.products.capacity()
            + self.eq_low.capacity()
            + self.eq_high.capacity())
            * core::mem::size_of::<Gf>()
    }

    pub fn dimension(&self) -> usize {
        self.ac.len()
    }

    pub fn root(&self) -> (Gf, Gf) {
        let level = self.dimension();
        (self.nums[level][0], self.dens[level][0])
    }

    fn prove(
        &mut self,
        transcript: &mut impl Transcript,
        product_workspace: &mut ProductWorkspace,
    ) -> (FractionTreeProof, FractionClaim) {
        let (mut claim_num, mut claim_den) = self.root();
        let mut claim_point = Vec::with_capacity(self.dimension());
        let mut ac_evals = Vec::with_capacity(self.dimension());
        let mut product_proofs = Vec::with_capacity(self.dimension());
        let mut numerators = Vec::with_capacity(self.dimension());
        for level in (0..self.dimension()).rev() {
            let ac_eval = mle_eval(
                &self.ac[level],
                self.ac_padding[level],
                &claim_point,
                &mut self.eq_low,
                &mut self.eq_high,
            );
            ac_evals.push(ac_eval);
            absorb_field_slice(transcript, &[ac_eval]);
            let gamma = transcript.get_field_challenge::<Gf>(&());
            assert_ne!(gamma, Gf::ZERO, "negligible zero fraction challenge");

            let product_padding = self.den_padding[level] + gamma * self.num_padding[level];
            let active = self.ac[level].len();
            self.products.resize(2 * active, product_padding);
            let (left_products, right_products) = self.products.split_at_mut(active);
            left_products.fill(product_padding);
            right_products.fill(product_padding);
            let fill = |index: usize, left: &mut Gf, right: &mut Gf| {
                let even = 2 * index;
                *left = self.dens[level][even] + gamma * self.nums[level][even];
                if even + 1 < self.nums[level].len() {
                    *right = self.dens[level][even + 1] + gamma * self.nums[level][even + 1];
                }
            };
            #[cfg(feature = "parallel")]
            if active >= PARALLEL_THRESHOLD {
                left_products[..active]
                    .par_iter_mut()
                    .zip(right_products[..active].par_iter_mut())
                    .enumerate()
                    .for_each(|(index, (left, right))| fill(index, left, right));
            } else {
                left_products[..active]
                    .iter_mut()
                    .zip(right_products[..active].iter_mut())
                    .enumerate()
                    .for_each(|(index, (left, right))| fill(index, left, right));
            }
            #[cfg(not(feature = "parallel"))]
            left_products[..active]
                .iter_mut()
                .zip(right_products[..active].iter_mut())
                .enumerate()
                .for_each(|(index, (left, right))| fill(index, left, right));

            let folded = claim_den + gamma * claim_num + gamma.square() * ac_eval;
            let inputs = [(
                ProductInput::PaddedTable(left_products, product_padding),
                ProductInput::PaddedTable(right_products, product_padding),
            )];
            let (product_proof, product_point, values) = prove_product_batch(
                transcript,
                &[(claim_point.clone(), folded)],
                &inputs,
                product_workspace,
            );
            product_proofs.push(product_proof);
            let (left, right) = values[0];
            let [a, c] = mle_eval_interleaved_pair(
                &self.nums[level],
                self.num_padding[level],
                &product_point,
                &mut self.eq_low,
                &mut self.eq_high,
            );
            numerators.push((a, c));
            absorb_field_slice(transcript, &[a, c]);
            let b = left + gamma * a;
            let d = right + gamma * c;
            let selector = transcript.get_field_challenge::<Gf>(&());
            claim_point.clear();
            claim_point.push(selector);
            claim_point.extend_from_slice(&product_point);
            claim_num = a + selector * (a + c);
            claim_den = b + selector * (b + d);
        }
        (
            FractionTreeProof {
                ac_evals,
                products: product_proofs,
                numerators,
            },
            FractionClaim {
                point: claim_point,
                num: claim_num,
                den: claim_den,
            },
        )
    }
}

pub fn prove_rational(
    transcript: &mut impl Transcript,
    left: &mut FractionTreeWitness,
    right: &mut FractionTreeWitness,
    product_workspace: &mut ProductWorkspace,
) -> (RationalProof, RationalClaims) {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(left.dimension() as u64).to_le_bytes());
    transcript.absorb_slice(&(right.dimension() as u64).to_le_bytes());
    let left_root = left.root();
    let right_root = right.root();
    assert_ne!(left_root.1, Gf::ZERO);
    assert_ne!(right_root.1, Gf::ZERO);
    assert_eq!(left_root.0 * right_root.1, right_root.0 * left_root.1);
    let roots = [left_root.0, left_root.1, right_root.0, right_root.1];
    absorb_field_slice(transcript, &roots);
    let (left_proof, left_claim) = left.prove(transcript, product_workspace);
    let (right_proof, right_claim) = right.prove(transcript, product_workspace);
    (
        RationalProof {
            roots,
            left: left_proof,
            right: right_proof,
        },
        RationalClaims {
            left: left_claim,
            right: right_claim,
        },
    )
}

pub fn verify_rational(
    transcript: &mut impl Transcript,
    proof: &RationalProof,
    left_dimension: usize,
    right_dimension: usize,
) -> Option<RationalClaims> {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(left_dimension as u64).to_le_bytes());
    transcript.absorb_slice(&(right_dimension as u64).to_le_bytes());
    let [left_num, left_den, right_num, right_den] = proof.roots;
    if left_den == Gf::ZERO
        || right_den == Gf::ZERO
        || left_num * right_den != right_num * left_den
    {
        return None;
    }
    absorb_field_slice(transcript, &proof.roots);
    Some(RationalClaims {
        left: verify_tree(
            transcript,
            &proof.left,
            left_dimension,
            left_num,
            left_den,
        )?,
        right: verify_tree(
            transcript,
            &proof.right,
            right_dimension,
            right_num,
            right_den,
        )?,
    })
}

fn verify_tree(
    transcript: &mut impl Transcript,
    proof: &FractionTreeProof,
    dimension: usize,
    mut claim_num: Gf,
    mut claim_den: Gf,
) -> Option<FractionClaim> {
    if proof.ac_evals.len() != dimension
        || proof.products.len() != dimension
        || proof.numerators.len() != dimension
    {
        return None;
    }
    let mut claim_point = Vec::new();
    for round in 0..dimension {
        let ac_eval = proof.ac_evals[round];
        absorb_field_slice(transcript, &[ac_eval]);
        let gamma = transcript.get_field_challenge::<Gf>(&());
        if gamma == Gf::ZERO {
            return None;
        }
        let folded = claim_den + gamma * claim_num + gamma.square() * ac_eval;
        let (product_point, values) = verify_product_batch(
            transcript,
            &[(claim_point.clone(), folded)],
            &proof.products[round],
        )?;
        let (left, right) = values[0];
        let (a, c) = proof.numerators[round];
        absorb_field_slice(transcript, &[a, c]);
        let b = left + gamma * a;
        let d = right + gamma * c;
        let selector = transcript.get_field_challenge::<Gf>(&());
        claim_point.clear();
        claim_point.push(selector);
        claim_point.extend_from_slice(&product_point);
        claim_num = a + selector * (a + c);
        claim_den = b + selector * (b + d);
    }
    Some(FractionClaim {
        point: claim_point,
        num: claim_num,
        den: claim_den,
    })
}

fn mle_eval(
    values: &[Gf],
    padding: Gf,
    point: &[Gf],
    eq_low: &mut Vec<Gf>,
    eq_high: &mut Vec<Gf>,
) -> Gf {
    assert!(!values.is_empty() && values.len() <= 1usize << point.len());
    prepare_eq_sqrt(point, eq_low, eq_high);
    padding + dot_sqrt(values, padding, eq_low, eq_high)
}

fn mle_eval_interleaved_pair(
    values: &[Gf],
    padding: Gf,
    point: &[Gf],
    eq_low: &mut Vec<Gf>,
    eq_high: &mut Vec<Gf>,
) -> [Gf; 2] {
    assert!(!values.is_empty() && values.len() <= 2 * (1usize << point.len()));
    prepare_eq_sqrt(point, eq_low, eq_high);
    let low_len = eq_low.len();
    let pair_len = values.len().div_ceil(2);
    let high_len = pair_len.div_ceil(low_len);
    let block = |high: usize| {
        let pair_base = high * low_len;
        let block_len = (pair_len - pair_base).min(low_len);
        let mut even = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        let mut odd = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        for low in 0..block_len {
            let source = 2 * (pair_base + low);
            <Gf as WideMulAcc>::wide_add_assign(
                &mut even,
                &<Gf as WideMulAcc>::mul_wide(
                    &(values[source] + padding),
                    &eq_low[low],
                ),
            );
            if let Some(&value) = values.get(source + 1) {
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut odd,
                    &<Gf as WideMulAcc>::mul_wide(&(value + padding), &eq_low[low]),
                );
            }
        }
        let even = <Gf as WideMulAcc>::from_wide(even);
        let odd = <Gf as WideMulAcc>::from_wide(odd);
        [
            <Gf as WideMulAcc>::mul_wide(&even, &eq_high[high]),
            <Gf as WideMulAcc>::mul_wide(&odd, &eq_high[high]),
        ]
    };
    let zero = || [
        <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
        <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
    ];
    let merge = |
        mut left: [<Gf as WideMulAcc>::Wide; 2],
        right: [<Gf as WideMulAcc>::Wide; 2],
    | {
        <Gf as WideMulAcc>::wide_add_assign(&mut left[0], &right[0]);
        <Gf as WideMulAcc>::wide_add_assign(&mut left[1], &right[1]);
        left
    };
    #[cfg(feature = "parallel")]
    let sums = if pair_len >= PARALLEL_THRESHOLD && high_len > 1 {
        (0..high_len).into_par_iter().map(block).reduce(zero, merge)
    } else {
        (0..high_len).map(block).fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let sums = (0..high_len).map(block).fold(zero(), merge);
    [
        padding + <Gf as WideMulAcc>::from_wide(sums[0].clone()),
        padding + <Gf as WideMulAcc>::from_wide(sums[1].clone()),
    ]
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

fn dot_sqrt(values: &[Gf], padding: Gf, eq_low: &[Gf], eq_high: &[Gf]) -> Gf {
    let low_len = eq_low.len();
    let blocks = values.len().div_ceil(low_len);
    debug_assert!(blocks <= eq_high.len());
    let block = |high: usize| {
        let base = high * low_len;
        let values = &values[base..(base + low_len).min(values.len())];
        let mut inner = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        for (&value, &weight) in values.iter().zip(eq_low) {
            <Gf as WideMulAcc>::wide_add_assign(
                &mut inner,
                &<Gf as WideMulAcc>::mul_wide(&(value + padding), &weight),
            );
        }
        let inner = <Gf as WideMulAcc>::from_wide(inner);
        <Gf as WideMulAcc>::mul_wide(&inner, &eq_high[high])
    };
    let zero = || <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
    let merge = |mut left: <Gf as WideMulAcc>::Wide, right: <Gf as WideMulAcc>::Wide| {
        <Gf as WideMulAcc>::wide_add_assign(&mut left, &right);
        left
    };
    #[cfg(feature = "parallel")]
    let sum = if values.len() >= PARALLEL_THRESHOLD && blocks > 1 {
        (0..blocks).into_par_iter().map(block).reduce(zero, merge)
    } else {
        (0..blocks).map(block).fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let sum = (0..blocks).map(block).fold(zero(), merge);
    <Gf as WideMulAcc>::from_wide(sum)
}
