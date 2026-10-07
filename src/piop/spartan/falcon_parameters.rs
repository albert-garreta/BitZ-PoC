//! Original-Falcon parameters and exact compile-time ring security accounting.

pub const Q: u64 = 12_289;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FalconProtocol {
    NativeCarry,
    SharedPrime,
}

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
        assert!(
            n.is_power_of_two() && n >= 2 && n <= 1024,
            "Falcon degree needs a registered parameter profile"
        );
        let log = n.ilog2() as usize;
        Self {
            n,
            norm_bound: [
                0, 101498, 208714, 428865, 892039, 1852696, 3842630, 7959734, 16468416, 34034726,
                70265242,
            ][log],
            oversampling: [0, 65, 67, 71, 77, 86, 100, 122, 154, 205, 287][log],
            signature_bits: [0, 10, 11, 11, 12, 12, 12, 12, 12, 12, 12][log],
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
    pub const fn prefix_cutoff(self) -> usize {
        let minimum = (self.oversampling + 1).next_power_of_two();
        if self.n > minimum { self.n } else { minimum }
    }
    pub const fn prefix_bias(self) -> usize {
        self.prefix_cutoff() - self.n
    }
    pub const fn prefix_bits(self) -> usize {
        self.prefix_cutoff().ilog2() as usize + 1
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
    const fn sub(self, rhs: Self) -> Self {
        let mut out = [0; 3];
        let mut borrow = false;
        let mut i = 0;
        while i < 3 {
            let (v, b1) = self.0[i].overflowing_sub(rhs.0[i]);
            let (v, b2) = v.overflowing_sub(borrow as u64);
            out[i] = v;
            borrow = b1 || b2;
            i += 1;
        }
        assert!(!borrow, "negative cardinality");
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

pub const fn generator_cardinality(k: usize) -> Cardinality {
    let full = field_cardinality(k);
    match k {
        8 => full.sub(field_cardinality(4)),
        9 => full.sub(field_cardinality(3)),
        10 => full
            .sub(field_cardinality(5))
            .sub(field_cardinality(2).sub(field_cardinality(1))),
        11 => full.sub(field_cardinality(1)),
        _ => panic!("unregistered Falcon extension degree"),
    }
}

pub const fn extension_constant(k: usize) -> u16 {
    match k {
        8 => 6,
        9 => 60,
        10 => 4,
        11 => 14,
        _ => panic!("unregistered Falcon extension degree"),
    }
}

pub const fn ring_budget_ok(
    n: usize,
    k: usize,
    bits: usize,
    max_batch: usize,
    protocol: FalconProtocol,
) -> bool {
    let _ = FalconParameters::for_degree(n);
    assert!(
        bits == 100 || bits == 128,
        "security target must be 100 or 128"
    );
    assert!(max_batch >= 1 && max_batch <= 1024, "invalid maximum batch");
    let d = max_batch.next_power_of_two().ilog2() as usize;
    let (numerator, denominator) = match protocol {
        FalconProtocol::NativeCarry => (d + 2 * n - 2, generator_cardinality(k)),
        FalconProtocol::SharedPrime => (4 * d + 2 * n + 1, field_cardinality(k)),
    };
    let mut lhs = Cardinality([numerator as u64, 0, 0]);
    let mut i = 0;
    while i < bits + 2 {
        lhs = lhs.mul_small(2);
        i += 1;
    }
    lhs.le(denominator)
}

pub const fn auto_k(n: usize, bits: usize, max_batch: usize, protocol: FalconProtocol) -> usize {
    let mut k = 8;
    while k <= 11 {
        if ring_budget_ok(n, k, bits, max_batch, protocol) {
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
        for log in 1..=10 {
            for bits in [100, 128] {
                for batch in 1usize..=1024 {
                    for protocol in [FalconProtocol::NativeCarry, FalconProtocol::SharedPrime] {
                        let n = 1 << log;
                        let d = batch.next_power_of_two().ilog2() as usize;
                        let selected = auto_k(n, bits, batch, protocol);
                        let mut expected = None;
                        for k in 8..=11 {
                            let full = q.pow(k as u32);
                            let denominator = match protocol {
                                FalconProtocol::SharedPrime => full,
                                FalconProtocol::NativeCarry => match k {
                                    8 => full - q.pow(4),
                                    9 => full - q.pow(3),
                                    10 => full - q.pow(5) - q.pow(2) + &q,
                                    11 => full - &q,
                                    _ => unreachable!(),
                                },
                            };
                            let numerator = match protocol {
                                FalconProtocol::NativeCarry => d + 2 * n - 2,
                                FalconProtocol::SharedPrime => 4 * d + 2 * n + 1,
                            };
                            let valid = (BigUint::from(numerator) << (bits + 2)) <= denominator;
                            assert_eq!(ring_budget_ok(n, k, bits, batch, protocol), valid);
                            if valid && expected.is_none() {
                                expected = Some(k);
                            }
                        }
                        assert_eq!(Some(selected), expected);
                    }
                }
            }
        }
    }
    #[test]
    fn reference_geometry_and_biased_counter() {
        for log in 1..=10 {
            let p = FalconParameters::for_degree(1 << log);
            assert!(p.norm_bound < 1 << p.norm_bits());
            assert!(p.prefix_bias() + p.samples() < 1 << p.prefix_bits());
            for prefix in 0..=p.samples() {
                assert_eq!(
                    prefix < p.n,
                    ((prefix + p.prefix_bias()) >> (p.prefix_bits() - 1)) == 0
                );
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
