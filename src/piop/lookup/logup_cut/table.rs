use crate::poly::univariate::binary_gf128::Gf128 as Gf;

use super::{ChunkSpec, DyadicPlan, PackedLayout, chunk_specs};

pub fn encode_table_row(row: usize) -> Gf {
    Gf::from_polynomial_words([row as u64, (row as u128 >> 64) as u64])
}

pub struct ChunkProductTable<'a> {
    pub y: &'a [Gf],
    pub chunks: &'a [ChunkSpec],
    pub layout: &'a PackedLayout,
}

impl ChunkProductTable<'_> {
    pub fn eval(&self, point: &[Gf]) -> Gf {
        assert_eq!(self.y.len(), self.chunks.iter().map(|chunk| chunk.width).sum::<usize>());
        assert_eq!(self.chunks.len(), self.layout.blocks.len());
        assert_eq!(point.len(), self.layout.dim);

        let one = Gf::ONE;
        self.chunks
            .iter()
            .zip(&self.layout.blocks)
            .map(|(chunk, block)| {
                assert_eq!(block.dim, chunk.width);
                let selector = point[chunk.width..]
                    .iter()
                    .enumerate()
                    .fold(one, |acc, (bit, &z)| {
                        if (block.offset >> (chunk.width + bit)) & 1 == 0 {
                            acc * (one + z)
                        } else {
                            acc * z
                        }
                    });
                let local = point[..chunk.width]
                    .iter()
                    .zip(&self.y[chunk.factor_start..chunk.factor_start + chunk.width])
                    .fold(one, |acc, (&z, &factor)| acc * (one + z + z * factor));
                selector * local
            })
            .sum()
    }

    pub fn materialize(&self, out: &mut [Gf]) {
        assert_eq!(out.len(), self.layout.padded_len);
        out.fill(Gf::ZERO);
        for (chunk, block) in self.chunks.iter().zip(&self.layout.blocks) {
            let table = &mut out[block.offset..block.offset + block.len()];
            table[0] = Gf::ONE;
            for pattern in 1..table.len() {
                let bit = pattern.trailing_zeros() as usize;
                table[pattern] = table[pattern ^ (1 << bit)] * self.y[chunk.factor_start + bit];
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogupCutEstimate {
    pub chunk_bits: usize,
    pub n_chunks: u64,
    pub last_chunk_bits: usize,
    pub dyadic_blocks: Vec<u64>,
    pub root_merge_nodes: u64,
    pub upper_product_nodes: u64,
    pub source_real_rows: u64,
    pub source_padded_rows: u64,
    pub table_real_rows: u64,
    pub table_padded_rows: u64,
    pub table_bytes: u64,
    pub aux_commit_rows: u64,
    pub aux_commit_bytes: u64,
    pub estimated_scratch_bytes: u64,
}

impl LogupCutEstimate {
    pub fn new(ell1: usize, ell2: usize, chunk_bits: usize) -> Self {
        let chunks = chunk_specs(ell1, chunk_bits);
        let plan = DyadicPlan::new(chunks.len());
        let source_real_rows = chunks.len().checked_mul(ell2).expect("source size overflow");
        let source_padded_rows = source_real_rows.next_power_of_two();
        let table_real_rows = chunks
            .iter()
            .try_fold(0usize, |total, chunk| {
                total.checked_add(1usize.checked_shl(chunk.width as u32)?)
            })
            .expect("table size overflow");
        let table_padded_rows = table_real_rows.next_power_of_two();
        let field_bytes = core::mem::size_of::<Gf>();
        let estimated_scratch_bytes = field_bytes
            .checked_mul(
                11usize
                    .checked_mul(source_padded_rows)
                    .and_then(|source| {
                        10usize
                            .checked_mul(table_padded_rows)
                            .and_then(|table| source.checked_add(table))
                    })
                    .expect("scratch estimate overflow"),
            )
            .expect("scratch estimate overflow");

        Self {
            chunk_bits,
            n_chunks: chunks.len() as u64,
            last_chunk_bits: chunks.last().unwrap().width,
            dyadic_blocks: plan.blocks.iter().map(|block| block.n_chunks as u64).collect(),
            root_merge_nodes: plan.root_merge_nodes() as u64,
            upper_product_nodes: (chunks.len() - 1) as u64,
            source_real_rows: source_real_rows as u64,
            source_padded_rows: source_padded_rows as u64,
            table_real_rows: table_real_rows as u64,
            table_padded_rows: table_padded_rows as u64,
            table_bytes: (table_padded_rows * field_bytes) as u64,
            aux_commit_rows: table_padded_rows as u64,
            aux_commit_bytes: (table_padded_rows * field_bytes) as u64,
            estimated_scratch_bytes: estimated_scratch_bytes as u64,
        }
    }
}
