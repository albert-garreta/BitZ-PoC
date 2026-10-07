//! Registered ring-challenge fields; independent of the BitZ sumcheck fields.
use super::falcon_parameters::{Q, extension_constant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FalconExtension<const K: usize>(pub [u16; K]);

impl<const K: usize> FalconExtension<K> {
    pub const ZERO: Self = Self([0; K]);
    pub const ONE: Self = {
        let mut coordinates = [0; K];
        coordinates[0] = 1;
        Self(coordinates)
    };
    pub const fn new(coordinates: [u16; K]) -> Self {
        Self(coordinates)
    }
    pub fn canonical(self) -> bool {
        self.0.iter().all(|&x| u64::from(x) < Q)
    }
    pub fn is_base_field(self) -> bool {
        self.0[1..].iter().all(|&x| x == 0)
    }
    pub fn add(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| {
            ((u32::from(self.0[i]) + u32::from(rhs.0[i])) % Q as u32) as u16
        }))
    }
    pub fn sub(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| {
            ((u32::from(self.0[i]) + Q as u32 - u32::from(rhs.0[i])) % Q as u32) as u16
        }))
    }
    pub fn mul(self, rhs: Self) -> Self {
        // The catalog's maximum product has 21 coefficients. A fixed stack buffer
        // avoids unstable generic-const expressions and heap allocation.
        let c = i64::from(extension_constant(K));
        let mut product = [0i64; 21];
        for i in 0..K {
            for j in 0..K {
                product[i + j] += i64::from(self.0[i]) * i64::from(rhs.0[j]);
            }
        }
        for j in (K..2 * K - 1).rev() {
            product[j - K] -= c * product[j];
            product[j - K + 1] -= product[j];
        }
        Self(std::array::from_fn(|i| {
            product[i].rem_euclid(Q as i64) as u16
        }))
    }
    pub fn pow(mut self, mut exponent: u64) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }
    /// Full extension degree, not multiplicative primitiveness.
    pub fn generates_extension(self) -> bool {
        let exponents: &[usize] = match K {
            8 => &[4],
            9 => &[3],
            10 => &[5, 2],
            11 => &[1],
            _ => panic!("unregistered Falcon extension"),
        };
        let mut frobenius = self;
        for i in 1..K {
            frobenius = frobenius.pow(Q);
            if exponents.contains(&i) && frobenius == self {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn trim(p: &mut Vec<u64>) {
        while p.last() == Some(&0) {
            p.pop();
        }
    }
    fn remainder(mut a: Vec<u64>, b: &[u64]) -> Vec<u64> {
        let inverse = {
            let mut x = *b.last().unwrap();
            let mut r = 1;
            let mut e = Q - 2;
            while e != 0 {
                if e & 1 != 0 {
                    r = r * x % Q;
                }
                x = x * x % Q;
                e >>= 1;
            }
            r
        };
        trim(&mut a);
        while a.len() >= b.len() {
            let offset = a.len() - b.len();
            let factor = a[a.len() - 1] * inverse % Q;
            for (i, &v) in b.iter().enumerate() {
                a[offset + i] = (a[offset + i] + Q - factor * v % Q) % Q;
            }
            trim(&mut a);
        }
        a
    }
    fn check<const K: usize>() {
        type Word = u64;
        let c = Word::from(extension_constant(K));
        let mut modulus = vec![0; K + 1];
        modulus[0] = c;
        modulus[1] = 1;
        modulus[K] = 1;
        let mut theta = FalconExtension::<K>::ZERO;
        theta.0[1] = 1;
        let mut power = theta;
        for i in 1..=K {
            power = power.pow(Q);
            if (K == 8 && i == 4)
                || (K == 9 && i == 3)
                || (K == 10 && (i == 2 || i == 5))
                || (K == 11 && i == 1)
            {
                let mut a = modulus.clone();
                let mut b = power.sub(theta).0.map(Word::from).to_vec();
                trim(&mut b);
                while !b.is_empty() {
                    let r = remainder(a, &b);
                    a = b;
                    b = r;
                }
                assert_eq!(a.len(), 1, "Rabin proper-divisor gcd for degree {K}");
                assert!(!power.sub(theta).eq(&FalconExtension::ZERO));
            }
        }
        assert_eq!(power, theta, "Rabin final Frobenius condition");
        assert!(theta.generates_extension());
        assert!(!FalconExtension::<K>::ZERO.generates_extension());
        assert!(!FalconExtension::<K>::ONE.generates_extension());
        for seed in 1..=24 {
            let a = FalconExtension::<K>(std::array::from_fn(|i| {
                ((seed * 137 + i * 499) % Q as usize) as u16
            }));
            let b = FalconExtension::<K>(std::array::from_fn(|i| {
                ((seed * 23 + i * 733) % Q as usize) as u16
            }));
            let mut product = vec![0; 2 * K - 1];
            for i in 0..K {
                for j in 0..K {
                    product[i + j] = (product[i + j] + Word::from(a.0[i]) * Word::from(b.0[j])) % Q;
                }
            }
            let mut reference = remainder(product, &modulus);
            reference.resize(K, 0);
            assert_eq!(a.mul(b).0.map(Word::from).as_slice(), reference);
        }
        // Trace to each maximal proper subfield produces a rejected element.
        for d in 1..K {
            if K % d == 0 && (2..K / d).all(|r| (K / d) % r != 0) {
                let mut value = theta;
                let mut trace = FalconExtension::<K>::ZERO;
                for _ in 0..K / d {
                    trace = trace.add(value);
                    for _ in 0..d {
                        value = value.pow(Q);
                    }
                }
                assert!(!trace.generates_extension());
            }
        }
    }
    #[test]
    fn registered_fields() {
        check::<8>();
        check::<9>();
        check::<10>();
        check::<11>();
    }
}
