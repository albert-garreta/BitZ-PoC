//! Original-Falcon parameters and exact compile-time ring security accounting.

pub const Q: u64 = 12_289;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconParameters {
    pub n: usize,
    pub norm_bound: u64,
    pub oversampling: usize,
    pub signature_bits: usize,
}

impl FalconParameters {
    /// Tables from Falcon's reference common.c and codec.c, indexed by log2(N).
    pub const fn for_degree(n: usize) -> Self {
        match n {
            512 => Self {
                n,
                norm_bound: 34_034_726,
                oversampling: 205,
                signature_bits: 12,
            },
            1024 => Self {
                n,
                norm_bound: 70_265_242,
                oversampling: 287,
                signature_bits: 12,
            },
            _ => panic!("Falcon degree must be 512 or 1024"),
        }
    }

    pub const fn samples(self) -> usize {
        self.n + self.oversampling
    }
    pub const fn public_key_bytes(self) -> usize {
        1 + (14 * self.n).div_ceil(8)
    }
    pub const fn signature_bytes(self) -> usize {
        41 + (self.signature_bits * self.n).div_ceil(8)
    }
    pub const fn norm_bits(self) -> usize {
        (64 - self.norm_bound.leading_zeros()) as usize
    }

    pub const fn prefix_bits(self) -> usize {
        self.n.ilog2() as usize + 1
    }
    pub const fn compaction_log(self) -> usize {
        self.samples().next_power_of_two().ilog2() as usize
    }
    pub const fn permutations(self) -> usize {
        (2 * self.samples()).div_ceil(136)
    }
    pub const fn slab_count(self) -> usize {
        self.permutations().count_ones() as usize
    }
    pub const fn slab(self, index: usize) -> (usize, usize) {
        let mut remaining = self.permutations();
        let mut first = 0;
        let mut i = 0;
        while remaining != 0 {
            let count = 1usize << remaining.ilog2();
            if i == index {
                return (first, count);
            }
            first += count;
            remaining -= count;
            i += 1;
        }
        panic!("invalid Falcon Keccak slab index")
    }
}

/// Little-endian 192-bit integer. All supported security comparisons fit exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cardinality(pub [u64; 3]);

impl Cardinality {
    const fn mul_small(self, rhs: u64) -> Self {
        let mut out = [0; 3];
        let mut carry = 0u128;
        let mut i = 0;
        while i < 3 {
            let value = self.0[i] as u128 * rhs as u128 + carry;
            out[i] = value as u64;
            carry = value >> 64;
            i += 1;
        }
        assert!(carry == 0, "security integer overflow");
        Self(out)
    }

    const fn le(self, rhs: Self) -> bool {
        let mut i = 3;
        while i > 0 {
            i -= 1;
            if self.0[i] != rhs.0[i] {
                return self.0[i] < rhs.0[i];
            }
        }
        true
    }
    pub fn as_f64(self) -> f64 {
        self.0[0] as f64 + (self.0[1] as f64) * 2f64.powi(64) + (self.0[2] as f64) * 2f64.powi(128)
    }
}

pub const fn field_cardinality(k: usize) -> Cardinality {
    assert!(k >= 1 && k <= 11);
    let mut value = Cardinality([1, 0, 0]);
    let mut i = 0;
    while i < k {
        value = value.mul_small(Q);
        i += 1;
    }
    value
}

pub const fn extension_constant(k: usize) -> u16 {
    match k {
        9 => 60,
        10 => 4,
        11 => 14,
        _ => panic!("unregistered Falcon extension degree"),
    }
}

pub const fn ring_budget_ok(n: usize, k: usize, bits: usize, max_batch: usize) -> bool {
    if !matches!(n, 512 | 1024)
        || !matches!(bits, 100 | 128)
        || max_batch == 0
        || max_batch > 1024
        || k < 9
        || k > 11
    {
        return false;
    }
    let d = max_batch.next_power_of_two().ilog2() as usize;
    let numerator = 4 * d + 2 * n + 1;
    let denominator = field_cardinality(k);
    let mut lhs = Cardinality([numerator as u64, 0, 0]);
    let mut i = 0;
    while i < bits + 2 {
        lhs = lhs.mul_small(2);
        i += 1;
    }
    lhs.le(denominator)
}

pub const fn auto_k(n: usize, bits: usize, max_batch: usize) -> usize {
    let mut k = 9;
    while k <= 11 {
        if ring_budget_ok(n, k, bits, max_batch) {
            return k;
        }
        k += 1;
    }
    panic!("no registered extension meets the ring security budget")
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    #[test]
    fn exact_security_selection_matches_big_integers() {
        let q = BigUint::from(Q);
        for n in [512, 1024] {
            for bits in [100, 128] {
                for batch in 1usize..=1024 {
                    let d = batch.next_power_of_two().ilog2() as usize;
                    let mut expected = None;
                    for k in 9..=11 {
                        let valid =
                            (BigUint::from(4 * d + 2 * n + 1) << (bits + 2)) <= q.pow(k as u32);
                        assert_eq!(ring_budget_ok(n, k, bits, batch), valid);
                        if valid && expected.is_none() {
                            expected = Some(k);
                        }
                    }
                    assert_eq!(Some(auto_k(n, bits, batch)), expected);
                }
            }
        }
    }
    #[test]
    fn supported_geometry_and_prefix_counter() {
        for log in 9..=10 {
            let p = FalconParameters::for_degree(1 << log);
            assert!(p.norm_bound < 1 << p.norm_bits());
            assert!(p.oversampling < p.n);
            assert_eq!((14 * p.n) % 8, 0);
            assert_eq!((p.signature_bits * p.n) % 8, 0);
            assert!(p.samples() < 1 << p.prefix_bits());
            for prefix in 0..=p.samples() {
                assert_eq!(prefix < p.n, (prefix >> (p.prefix_bits() - 1)) == 0);
            }
            let mut total = 0;
            for i in 0..p.slab_count() {
                let (first, count) = p.slab(i);
                assert_eq!(first, total);
                assert!(count.is_power_of_two());
                total += count;
            }
            assert_eq!(total, p.permutations());
        }
        assert_eq!(FalconParameters::for_degree(512).signature_bytes(), 809);
        assert_eq!(FalconParameters::for_degree(1024).signature_bytes(), 1577);
    }
}
