use crate::{
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::traits::Transcript,
    utils::{cfg_chunks_mut, cfg_iter_mut},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{ChunkSpec, DyadicPlan, PackedBlock, PackedLayout, encode_table_row};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutClaim {
    pub block: usize,
    pub point: Vec<Gf>,
    pub value: Gf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedCut {
    pub delta: Gf,
    pub claim: Gf,
    pub source_layout: PackedLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TensorWeightTerm {
    pub row_weights: Vec<Gf>,
    pub column_point: Vec<Gf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLinearClaim {
    pub terms: Vec<TensorWeightTerm>,
    pub value: Gf,
}

pub fn source_layout(plan: &DyadicPlan, r2: usize) -> PackedLayout {
    PackedLayout::new(plan.blocks.iter().map(|block| block.depth + r2))
}

pub fn merge_cut_claims(
    transcript: &mut impl Transcript,
    plan: &DyadicPlan,
    r2: usize,
    claims: &[CutClaim],
) -> Option<MergedCut> {
    assert_eq!(claims.len(), plan.blocks.len());
    for (index, (claim, block)) in claims.iter().zip(&plan.blocks).enumerate() {
        assert_eq!(claim.block, index);
        assert_eq!(claim.point.len(), block.depth + r2);
    }
    transcript.absorb_slice(b"bitz/logup-cut/merge/v1");
    transcript.absorb_slice(&(plan.n_chunks as u64).to_le_bytes());
    transcript.absorb_slice(&(r2 as u64).to_le_bytes());
    let delta = transcript.get_field_challenge(&());
    if delta == Gf::ZERO {
        return None;
    }
    let mut power = Gf::ONE;
    let mut claim = Gf::ZERO;
    for cut in claims {
        claim += power * cut.value;
        power *= delta;
    }
    Some(MergedCut {
        delta,
        claim,
        source_layout: source_layout(plan, r2),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn fill_pushforward(
    merged: &MergedCut,
    plan: &DyadicPlan,
    chunks: &[ChunkSpec],
    table_layout: &PackedLayout,
    r2: usize,
    claims: &[CutClaim],
    patterns: &[usize],
    source_weights: &mut [Gf],
    pushforward: &mut [Gf],
) {
    let columns = 1usize << r2;
    assert_eq!(chunks.len(), plan.n_chunks);
    assert_eq!(patterns.len(), chunks.len() * columns);
    assert_eq!(table_layout.blocks.len(), chunks.len());
    assert_eq!(source_weights.len(), merged.source_layout.real_len);
    assert_eq!(pushforward.len(), table_layout.padded_len);
    cfg_iter_mut!(source_weights).for_each(|value| *value = Gf::ZERO);
    cfg_iter_mut!(pushforward).for_each(|value| *value = Gf::ZERO);

    let mut chunk_sources = vec![(0usize, 0usize, 0usize); chunks.len()];
    let mut delta_power = Gf::ONE;
    for (block_index, ((block, packed), claim)) in plan
        .blocks
        .iter()
        .zip(&merged.source_layout.blocks)
        .zip(claims)
        .enumerate()
    {
        debug_assert_eq!(claim.block, block_index);
        let chunk_weights = build_eq_x_r_vec(&claim.point[..block.depth], &()).unwrap_or(vec![Gf::ONE]);
        let column_weights = build_eq_x_r_vec(&claim.point[block.depth..], &()).unwrap_or(vec![Gf::ONE]);
        let source = &mut source_weights[packed.offset..packed.offset + packed.len()];
        cfg_chunks_mut!(source, block.n_chunks)
            .enumerate()
            .for_each(|(column, source)| {
                let column_weight = delta_power * column_weights[column];
                for (weight, &chunk_weight) in source.iter_mut().zip(&chunk_weights) {
                    *weight = column_weight * chunk_weight;
                }
            });
        for local_chunk in 0..block.n_chunks {
            chunk_sources[block.chunk_start + local_chunk] =
                (packed.offset, block.depth, local_chunk);
        }
        delta_power *= merged.delta;
    }

    let full_width = chunks[0].width;
    let full_len = 1usize << full_width;
    let full_chunks = chunks
        .iter()
        .take_while(|chunk| chunk.width == full_width)
        .count();
    cfg_chunks_mut!(&mut pushforward[..full_chunks * full_len], full_len)
        .enumerate()
        .for_each(|(chunk, histogram)| {
            let (source_offset, depth, local_chunk) = chunk_sources[chunk];
            for column in 0..columns {
                histogram[patterns[chunk * columns + column]] +=
                    source_weights[source_offset + local_chunk + (column << depth)];
            }
        });
    for chunk in full_chunks..chunks.len() {
        let table_block = table_layout.blocks[chunk];
        let histogram = &mut pushforward[table_block.offset..table_block.offset + table_block.len()];
        let (source_offset, depth, local_chunk) = chunk_sources[chunk];
        for column in 0..columns {
            histogram[patterns[chunk * columns + column]] +=
                source_weights[source_offset + local_chunk + (column << depth)];
        }
    }
}

pub fn eval_source_weight(merged: &MergedCut, claims: &[CutClaim], point: &[Gf]) -> Gf {
    assert_eq!(point.len(), merged.source_layout.dim);
    assert_eq!(claims.len(), merged.source_layout.blocks.len());
    let mut delta_power = Gf::ONE;
    let mut value = Gf::ZERO;
    for (claim, packed) in claims.iter().zip(&merged.source_layout.blocks) {
        value += block_selector(*packed, merged.source_layout.dim, point)
            * delta_power
            * eq_eval(&claim.point, &point[..packed.dim]);
        delta_power *= merged.delta;
    }
    value
}

pub fn eval_table_encoding(layout: &PackedLayout, point: &[Gf]) -> Gf {
    assert_eq!(point.len(), layout.dim);
    layout
        .blocks
        .iter()
        .map(|&block| {
            let local = point[..block.dim]
                .iter()
                .enumerate()
                .fold(encode_table_row(block.offset), |value, (bit, &coordinate)| {
                    value + coordinate * encode_table_row(1usize << bit)
                });
            block_selector(block, layout.dim, point) * local
        })
        .sum()
}

#[allow(clippy::too_many_arguments)]
pub fn derive_source_index_claim(
    merged: &MergedCut,
    plan: &DyadicPlan,
    chunks: &[ChunkSpec],
    table_layout: &PackedLayout,
    r2: usize,
    claims: &[CutClaim],
    tau: Gf,
    fraction_point: &[Gf],
    fraction_num: Gf,
    fraction_den: Gf,
) -> Option<StructuredLinearClaim> {
    if fraction_point.len() != merged.source_layout.dim
        || fraction_num != eval_source_weight(merged, claims, fraction_point)
    {
        return None;
    }
    let ell1 = chunks.iter().map(|chunk| chunk.width).sum();
    let columns = 1usize << r2;
    let mut public_offset = Gf::ZERO;
    let mut terms = Vec::with_capacity(plan.blocks.len());
    for (block, packed) in plan.blocks.iter().zip(&merged.source_layout.blocks) {
        let selector = block_selector(*packed, merged.source_layout.dim, fraction_point);
        let local = &fraction_point[..packed.dim];
        let (chunk_point, column_point) = local.split_at(block.depth);
        let chunk_eq = build_eq_x_r_vec(chunk_point, &()).unwrap_or(vec![Gf::ONE]);
        let mut row_weights = vec![Gf::ZERO; ell1];
        for (local_chunk, &chunk_weight) in chunk_eq.iter().enumerate() {
            let chunk_index = block.chunk_start + local_chunk;
            let chunk = chunks[chunk_index];
            public_offset += selector
                * chunk_weight
                * encode_table_row(table_layout.blocks[chunk_index].offset);
            for bit in 0..chunk.width {
                row_weights[chunk.factor_start + bit] =
                    selector * chunk_weight * encode_table_row(1usize << bit);
            }
        }
        debug_assert_eq!(1usize << column_point.len(), columns);
        terms.push(TensorWeightTerm {
            row_weights,
            column_point: column_point.to_vec(),
        });
    }
    Some(StructuredLinearClaim {
        terms,
        value: fraction_den + tau + public_offset,
    })
}

fn block_selector(block: PackedBlock, global_dim: usize, point: &[Gf]) -> Gf {
    point[block.dim..global_dim]
        .iter()
        .enumerate()
        .fold(Gf::ONE, |value, (bit, &coordinate)| {
            if block.offset >> (block.dim + bit) & 1 == 0 {
                value * (Gf::ONE + coordinate)
            } else {
                value * coordinate
            }
        })
}

fn eq_eval(left: &[Gf], right: &[Gf]) -> Gf {
    left.iter().zip(right).fold(Gf::ONE, |acc, (&l, &r)| {
        acc * (Gf::ONE + l + r)
    })
}
