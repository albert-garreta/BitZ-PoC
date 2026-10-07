//! Small extensions of F_12289 and prepared public power-basis arithmetic.
//!
//! The registered moduli are Z^K + Z + c for K in {9, 10, 11}.
//! Ordinary elements use this fixed Z-basis; [`PowerCoordinates`] use a
//! runtime generator's basis. Their encodings must not be interchanged.

pub const Q: u64 = 12_289;

#[cfg(all(
    target_arch = "x86_64",
    any(test, all(target_feature = "avx512f", target_feature = "avx512bw"))
))]
mod avx512;

pub const fn extension_constant(k: usize) -> u16 {
    match k {
        9 => 60,
        10 => 4,
        11 => 14,
        _ => panic!("unregistered F_12289 extension degree"),
    }
}

/// Fixed-basis coordinates. Arithmetic requires canonical inputs (< 12289).
/// The public tuple constructor preserves the protocol codec's existing API;
/// check [`Self::canonical`] before operating on untrusted encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Q12289Extension<const K: usize>(pub [u16; K]);

impl<const K: usize> Q12289Extension<K> {
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
    #[cfg(test)]
    fn pow(mut self, mut exponent: u64) -> Self {
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
        let mut theta = Q12289Extension::<K>::ZERO;
        theta.0[1] = 1;
        let mut power = theta;
        for i in 1..=K {
            power = power.pow(Q);
            if (K == 9 && i == 3) || (K == 10 && (i == 2 || i == 5)) || (K == 11 && i == 1) {
                let mut a = modulus.clone();
                let mut b = power.sub(theta).0.map(Word::from).to_vec();
                trim(&mut b);
                while !b.is_empty() {
                    let r = remainder(a, &b);
                    a = b;
                    b = r;
                }
                assert_eq!(a.len(), 1, "Rabin proper-divisor gcd for degree {K}");
                assert!(!power.sub(theta).eq(&Q12289Extension::ZERO));
            }
        }
        assert_eq!(power, theta, "Rabin final Frobenius condition");
        for seed in 1..=24 {
            let a = Q12289Extension::<K>(std::array::from_fn(|i| {
                ((seed * 137 + i * 499) % Q as usize) as u16
            }));
            let b = Q12289Extension::<K>(std::array::from_fn(|i| {
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
    }
    #[test]
    fn registered_fields() {
        check::<9>();
        check::<10>();
        check::<11>();
    }
}

/// Canonical coefficients in a prepared generator's power basis.
///
/// Values from distinct `PowerBasis` contexts must not be mixed, even when K
/// agrees. This is a coordinate vector, not a fixed-basis extension element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerCoordinates<const K: usize>([u16; K]);

impl<const K: usize> PowerCoordinates<K> {
    pub const ZERO: Self = Self([0; K]);

    pub const fn coordinates(&self) -> &[u16; K] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerBasisError {
    UnsupportedDegree,
    NoncanonicalGenerator,
    /// Includes the base field and all proper subfields.
    NongeneratingElement,
}

/// Prepared basis 1, alpha, ..., alpha^(K-1) for a public generator alpha.
///
/// Setup uses variable-time Gaussian elimination on public field coordinates.
/// It must not be used to conceal a secret generator. Once prepared, conversion
/// costs K^2 base-field products; advancing costs K multiply-adds. All returned
/// coordinates are reduced before they can enter an integer lift.
#[derive(Clone, Debug)]
pub struct PowerBasis<const K: usize> {
    columns: [[u16; K]; K],
    inverse: [[u16; K]; K],
    // alpha^K = sum_k feedback[k] * alpha^k.
    feedback: [u16; K],
}

impl<const K: usize> PowerBasis<K> {
    pub fn try_new(alpha: Q12289Extension<K>) -> Result<Self, PowerBasisError> {
        if !matches!(K, 9 | 10 | 11) {
            return Err(PowerBasisError::UnsupportedDegree);
        }
        if !alpha.canonical() {
            return Err(PowerBasisError::NoncanonicalGenerator);
        }
        let mut power = Q12289Extension::ZERO;
        power.0[0] = 1;
        let columns = std::array::from_fn(|_| {
            let column = power.0;
            power = power.mul(alpha);
            column
        });
        // Column j is alpha^j in the fixed basis: fixed = M * power.
        let mut matrix: [[u16; K]; K] =
            std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i]));
        let mut inverse: [[u16; K]; K] =
            std::array::from_fn(|i| std::array::from_fn(|j| u16::from(i == j)));
        for column in 0..K {
            let pivot = (column..K)
                .find(|&row| matrix[row][column] != 0)
                .ok_or(PowerBasisError::NongeneratingElement)?;
            matrix.swap(column, pivot);
            inverse.swap(column, pivot);
            let scale = inverse_mod_q(matrix[column][column]);
            for j in 0..K {
                matrix[column][j] = reduce(u32::from(matrix[column][j]) * scale);
                inverse[column][j] = reduce(u32::from(inverse[column][j]) * scale);
            }
            for row in 0..K {
                if row == column {
                    continue;
                }
                let factor = u32::from(matrix[row][column]);
                for j in 0..K {
                    matrix[row][j] = reduce(
                        u32::from(matrix[row][j]) + Q as u32
                            - u32::from(reduce(factor * u32::from(matrix[column][j]))),
                    );
                    inverse[row][j] = reduce(
                        u32::from(inverse[row][j]) + Q as u32
                            - u32::from(reduce(factor * u32::from(inverse[column][j]))),
                    );
                }
            }
        }
        let feedback = inverse.map(|row| dot(&row, &power.0));
        Ok(Self {
            columns,
            inverse,
            feedback,
        })
    }

    /// Convert canonical fixed-basis coordinates into this power basis.
    pub fn encode(&self, value: Q12289Extension<K>) -> PowerCoordinates<K> {
        assert!(value.canonical(), "noncanonical fixed-basis element");
        PowerCoordinates(self.inverse.map(|row| dot(&row, &value.0)))
    }

    /// Convert back for cold paths or comparisons; avoid this in sequence loops.
    pub fn decode(&self, value: PowerCoordinates<K>) -> Q12289Extension<K> {
        Q12289Extension(std::array::from_fn(|i| {
            let mut sum = 0u64;
            for j in 0..K {
                sum += u64::from(self.columns[j][i]) * u64::from(value.0[j]);
            }
            (sum % Q) as u16
        }))
    }

    /// Multiply by the prepared generator and reduce into canonical coordinates.
    #[inline]
    pub fn advance(&self, value: &mut PowerCoordinates<K>) {
        let top = u32::from(value.0[K - 1]);
        for k in (1..K).rev() {
            value.0[k] = reduce(u32::from(value.0[k - 1]) + u32::from(self.feedback[k]) * top);
        }
        value.0[0] = reduce(u32::from(self.feedback[0]) * top);
    }

    /// Emit start, alpha*start, ... without conversions or allocation per step.
    pub fn fill_powers(&self, start: PowerCoordinates<K>, output: &mut [PowerCoordinates<K>]) {
        let mut value = start;
        for slot in output {
            *slot = value;
            self.advance(&mut value);
        }
    }

    /// Emit two independent sequences with the same public generator.
    ///
    /// The output lengths must agree. Degree 11 uses a paired SIMD kernel when
    /// compiled for x86-64 with AVX512F and AVX512BW (e.g. target-cpu=native on
    /// a supporting CPU); other targets and degrees use the portable recurrence.
    pub fn fill_powers_pair(
        &self,
        starts: [PowerCoordinates<K>; 2],
        outputs: [&mut [PowerCoordinates<K>]; 2],
    ) {
        assert_eq!(
            outputs[0].len(),
            outputs[1].len(),
            "sequence lengths differ"
        );
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw"
        ))]
        if K == 11 {
            // SAFETY: the build target supplies both features, the degree and
            // lengths were checked, and this type retains canonical coordinates.
            unsafe { avx512::fill_powers_pair(&self.feedback, starts, outputs) };
            return;
        }
        let [a, b] = outputs;
        self.fill_powers(starts[0], a);
        self.fill_powers(starts[1], b);
    }
}

#[inline]
fn reduce(value: u32) -> u16 {
    (value % Q as u32) as u16
}

fn dot<const K: usize>(a: &[u16; K], b: &[u16; K]) -> u16 {
    // K <= 11: sum <= 11*12288^2 = 1,660,944,384. Use u64 anyway.
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(&a, &b)| u64::from(a) * u64::from(b))
        .sum();
    (sum % Q) as u16
}

fn inverse_mod_q(value: u16) -> u32 {
    let mut base = u32::from(value);
    let mut result = 1;
    let mut exponent = Q - 2;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = u32::from(reduce(result * base));
        }
        base = u32::from(reduce(base * base));
        exponent >>= 1;
    }
    result
}

#[cfg(test)]
mod power_basis_tests {
    use super::*;

    fn check<const K: usize>() {
        for seed in 0..12 {
            let alpha = Q12289Extension(std::array::from_fn(|i| {
                if seed == 0 {
                    u16::from(i == 1)
                } else {
                    ((seed * 1237 + i * 97) % Q as usize) as u16
                }
            }));
            let basis = PowerBasis::try_new(alpha).unwrap();
            for start in [
                Q12289Extension::ZERO,
                Q12289Extension::ONE,
                Q12289Extension([12288; K]),
                alpha,
                Q12289Extension(std::array::from_fn(|i| {
                    ((i * 931 + seed * 73) % Q as usize) as u16
                })),
            ] {
                let mut expected = start;
                let mut actual = vec![PowerCoordinates::ZERO; 1024];
                basis.fill_powers(basis.encode(start), &mut actual);
                for coordinates in actual {
                    assert!(coordinates.0.iter().all(|&v| u64::from(v) < Q));
                    assert_eq!(basis.decode(coordinates), expected);
                    assert_eq!(coordinates, basis.encode(expected));
                    expected = expected.mul(alpha);
                }
            }
        }
    }

    #[test]
    fn recurrence_matches_extension_multiplication() {
        check::<9>();
        check::<10>();
        check::<11>();
    }

    fn check_pair<const K: usize>() {
        let mut alpha = Q12289Extension::<K>::ONE;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        let starts = [
            basis.encode(alpha),
            basis.encode(Q12289Extension([12288; K])),
        ];
        for n in [0, 1, 2, 3, 31, 32, 33, 512, 1024] {
            let mut expected = [
                vec![PowerCoordinates::ZERO; n],
                vec![PowerCoordinates::ZERO; n],
            ];
            for i in 0..2 {
                basis.fill_powers(starts[i], &mut expected[i]);
            }
            let mut a = vec![PowerCoordinates::ZERO; n];
            let mut b = a.clone();
            basis.fill_powers_pair(starts, [&mut a, &mut b]);
            assert_eq!([a, b], expected);
        }
    }

    #[test]
    fn paired_dispatch_matches_individual_sequences() {
        check_pair::<9>();
        check_pair::<10>();
        check_pair::<11>();
    }

    #[test]
    #[should_panic(expected = "sequence lengths differ")]
    fn paired_sequences_reject_mismatched_lengths() {
        let mut alpha = Q12289Extension::<11>::ZERO;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        basis.fill_powers_pair(
            [PowerCoordinates::ZERO; 2],
            [&mut [], &mut [PowerCoordinates::ZERO]],
        );
    }

    fn trace<const K: usize>(degree: usize) -> Q12289Extension<K> {
        let mut conjugate = Q12289Extension::<K>::ZERO;
        conjugate.0[1] = 1;
        let mut result = Q12289Extension::ZERO;
        for _ in 0..K / degree {
            result = result.add(conjugate);
            for _ in 0..degree {
                conjugate = conjugate.pow(Q);
            }
        }
        assert!(result.0[1..].iter().any(|&v| v != 0));
        result
    }

    #[test]
    fn rejects_noncanonical_and_nongenerating_elements() {
        assert_eq!(
            PowerBasis::<0>::try_new(Q12289Extension::ZERO).unwrap_err(),
            PowerBasisError::UnsupportedDegree
        );
        assert_eq!(
            PowerBasis::<11>::try_new(Q12289Extension([12289; 11])).unwrap_err(),
            PowerBasisError::NoncanonicalGenerator
        );
        for value in [Q12289Extension::<11>::ZERO, Q12289Extension::<11>::ONE] {
            assert_eq!(
                PowerBasis::try_new(value).unwrap_err(),
                PowerBasisError::NongeneratingElement
            );
        }
        assert_eq!(
            PowerBasis::try_new(trace::<9>(3)).unwrap_err(),
            PowerBasisError::NongeneratingElement
        );
        assert_eq!(
            PowerBasis::try_new(trace::<10>(5)).unwrap_err(),
            PowerBasisError::NongeneratingElement
        );
        assert_eq!(
            PowerBasis::<8>::try_new(Q12289Extension::ZERO).unwrap_err(),
            PowerBasisError::UnsupportedDegree
        );
    }
}
