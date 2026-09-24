#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkSpec {
    pub chunk: usize,
    pub factor_start: usize,
    pub width: usize,
}

pub fn chunk_specs(ell1: usize, chunk_bits: usize) -> Vec<ChunkSpec> {
    assert!(ell1 != 0);
    assert!((1..=ell1).contains(&chunk_bits));

    (0..ell1.div_ceil(chunk_bits))
        .map(|chunk| {
            let factor_start = chunk * chunk_bits;
            ChunkSpec {
                chunk,
                factor_start,
                width: chunk_bits.min(ell1 - factor_start),
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DyadicBlock {
    pub chunk_start: usize,
    pub n_chunks: usize,
    pub depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DyadicPlan {
    pub n_chunks: usize,
    pub blocks: Vec<DyadicBlock>,
}

impl DyadicPlan {
    pub fn new(n_chunks: usize) -> Self {
        assert!(n_chunks != 0);

        let mut blocks = Vec::with_capacity(n_chunks.count_ones() as usize);
        let mut chunk_start = 0;
        let mut remaining = n_chunks;
        while remaining != 0 {
            let depth = remaining.ilog2() as usize;
            let block_chunks = 1usize << depth;
            debug_assert_eq!(chunk_start % block_chunks, 0);
            blocks.push(DyadicBlock {
                chunk_start,
                n_chunks: block_chunks,
                depth,
            });
            chunk_start += block_chunks;
            remaining -= block_chunks;
        }

        debug_assert_eq!(chunk_start, n_chunks);
        Self { n_chunks, blocks }
    }

    pub fn root_merge_nodes(&self) -> usize {
        self.blocks.len() - 1
    }

    pub fn block_product_nodes(&self) -> usize {
        self.n_chunks - self.blocks.len()
    }
}
