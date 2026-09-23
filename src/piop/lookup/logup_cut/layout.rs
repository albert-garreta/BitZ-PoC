#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackedBlock {
    pub offset: usize,
    pub dim: usize,
}

impl PackedBlock {
    pub fn len(self) -> usize {
        1usize << self.dim
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedLayout {
    pub dim: usize,
    pub real_len: usize,
    pub padded_len: usize,
    pub blocks: Vec<PackedBlock>,
}

impl PackedLayout {
    /// Pack descending power-of-two blocks tightly in semantic order.
    pub fn new(block_dims: impl IntoIterator<Item = usize>) -> Self {
        let mut blocks = Vec::new();
        let mut real_len = 0usize;
        let mut previous_dim = usize::MAX;
        for dim in block_dims {
            assert!(dim <= previous_dim, "packed block dimensions must descend");
            let len = 1usize.checked_shl(dim as u32).expect("packed block too large");
            assert_eq!(real_len % len, 0, "packed block is not naturally aligned");
            blocks.push(PackedBlock { offset: real_len, dim });
            real_len = real_len.checked_add(len).expect("packed layout too large");
            previous_dim = dim;
        }
        assert!(real_len != 0);
        let padded_len = real_len.next_power_of_two();
        Self {
            dim: padded_len.ilog2() as usize,
            real_len,
            padded_len,
            blocks,
        }
    }
}
