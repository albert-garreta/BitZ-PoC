//! Small extensions of F_12289 and prepared public power-basis arithmetic.
//!
//! The registered moduli are Z^K + Z + c for K in {9, 10, 11}.
//! Ordinary elements use this fixed Z-basis; [`PowerCoordinates`] use a
//! runtime generator's basis. Their encodings must not be interchanged.

use std::mem::MaybeUninit;

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

/// Shared public extension weights, transposed for two base-field dot products.
///
/// Preparation and evaluation validate canonical inputs. Accumulation is exact
/// in integers until the single reduction of each result coordinate.
#[derive(Clone, Debug)]
pub struct PreparedPublicDotProducts<const K: usize> {
    coordinates: [Vec<u16>; K],
    len: usize,
}

impl<const K: usize> PreparedPublicDotProducts<K> {
    pub fn new(weights: &[Q12289Extension<K>]) -> Self {
        assert!(matches!(K, 9 | 10 | 11), "unsupported extension degree");
        assert!(
            weights.iter().all(|value| value.canonical()),
            "noncanonical weight"
        );
        // This length bound also proves every complete u64 accumulator fits.
        assert!(
            weights.len() <= u32::MAX as usize,
            "public dot product too long"
        );
        Self {
            coordinates: std::array::from_fn(|k| weights.iter().map(|value| value.0[k]).collect()),
            len: weights.len(),
        }
    }

    pub fn evaluate_pair(&self, a: &[u16], b: &[u16]) -> [Q12289Extension<K>; 2] {
        assert_eq!(a.len(), self.len, "public dot product length differs");
        assert_eq!(b.len(), self.len, "public dot product length differs");
        assert!(
            a.iter().chain(b).all(|&value| u64::from(value) < Q),
            "noncanonical coefficient"
        );
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw"
        ))]
        {
            // SAFETY: build features and canonical equal-length arrays satisfy
            // the kernel contract. The constructor bounds the final u64 sum.
            return unsafe { avx512::evaluate_public_pair(&self.coordinates, a, b) }
                .map(Q12289Extension);
        }
        #[allow(unreachable_code)]
        self.evaluate_pair_portable(a, b)
    }

    fn evaluate_pair_portable(&self, a: &[u16], b: &[u16]) -> [Q12289Extension<K>; 2] {
        let mut result = [[0; K]; 2];
        for k in 0..K {
            let mut sums = [0u64; 2];
            for ((&weight, &a), &b) in self.coordinates[k].iter().zip(a).zip(b) {
                sums[0] += u64::from(weight) * u64::from(a);
                sums[1] += u64::from(weight) * u64::from(b);
            }
            result[0][k] = (sums[0] % Q) as u16;
            result[1][k] = (sums[1] % Q) as u16;
        }
        result.map(Q12289Extension)
    }
}

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

    #[test]
    fn zeroed_coordinate_allocations_are_canonical() {
        fn check<const K: usize>() {
            for len in [0, 1, 3, 17, 1024] {
                let values = PowerCoordinates::<K>::zeroed_vec(len);
                assert_eq!(values.len(), len);
                assert!(values.iter().all(|value| *value == PowerCoordinates::ZERO));
            }
        }
        check::<9>();
        check::<10>();
        check::<11>();
    }

    #[test]
    fn uninitialized_coordinate_writers_fill_empty_tails_and_overwrite() {
        fn check<const K: usize, const PAIRS: usize>() {
            let mut alpha = Q12289Extension::<K>::ZERO;
            alpha.0[1] = 1;
            let basis = PowerBasis::try_new(alpha).unwrap();
            let starts: [[PowerCoordinates<K>; 2]; PAIRS] = std::array::from_fn(|pair| {
                std::array::from_fn(|side| {
                    basis.encode(Q12289Extension::new(std::array::from_fn(|k| {
                        ((Q - 1 + pair as u64 * 197 + side as u64 * 17 + k as u64 * 71) % Q) as u16
                    })))
                })
            });
            let subtract = basis.encode(Q12289Extension::new([(Q - 1) as u16; K]));
            let sentinel = PowerCoordinates([u16::MAX; K]);
            for len in [0, 1, 2, 3, 7, 17, 31, 33, 129, 1024] {
                let polynomials: [[Vec<u16>; 2]; PAIRS] = std::array::from_fn(|pair| {
                    std::array::from_fn(|side| {
                        (0..len)
                            .map(|j| [0, 1, (Q - 1) as u16][(j + pair + side) % 3])
                            .collect()
                    })
                });
                let mut actual: [[Box<[MaybeUninit<PowerCoordinates<K>>]>; 2]; PAIRS] =
                    std::array::from_fn(|_| {
                        std::array::from_fn(|_| {
                            let mut output =
                                Box::<[PowerCoordinates<K>]>::new_uninit_slice(len + 2);
                            output[0].write(sentinel);
                            output[len + 1].write(sentinel);
                            output
                        })
                    });
                for adjoints_only in [false, true] {
                    let mut pairs = actual.iter_mut();
                    let outputs = std::array::from_fn(|_| {
                        let [a, b] = pairs.next().unwrap();
                        [&mut a[1..len + 1], &mut b[1..len + 1]]
                    });
                    if adjoints_only {
                        basis.fill_adjoint_pairs_uninit(
                            starts,
                            subtract,
                            std::array::from_fn(|p| {
                                std::array::from_fn(|side| polynomials[p][side].as_slice())
                            }),
                            outputs,
                        );
                    } else {
                        basis.fill_powers_adjoint_pairs_uninit(
                            starts,
                            [subtract; PAIRS],
                            std::array::from_fn(|p| polynomials[p][1].as_slice()),
                            outputs,
                        );
                    }
                    for pair in 0..PAIRS {
                        for side in 0..2 {
                            let output = &actual[pair][side];
                            // SAFETY: guards were initialized before the call;
                            // the writer initialized every interior slot.
                            assert_eq!(unsafe { output[0].assume_init() }, sentinel);
                            assert_eq!(unsafe { output[len + 1].assume_init() }, sentinel);
                            let mut expected = basis.decode(starts[pair][side]);
                            for j in 0..len {
                                let value = unsafe { output[j + 1].assume_init() };
                                assert_eq!(value, basis.encode(expected));
                                assert!(
                                    value.0.iter().all(|&coordinate| u64::from(coordinate) < Q)
                                );
                                expected = expected.mul(alpha);
                                if adjoints_only || side == 1 {
                                    let scalar = u64::from(polynomials[pair][side][len - 1 - j]);
                                    expected = expected.sub(Q12289Extension::new(
                                        basis
                                            .decode(subtract)
                                            .0
                                            .map(|v| (u64::from(v) * scalar % Q) as u16),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        check::<9, 2>();
        check::<10, 4>();
        check::<11, 2>();
        check::<11, 4>();
        check::<11, 8>();
    }

    #[test]
    fn prepared_public_dot_products_match_independent_reference() {
        fn check<const K: usize>() {
            for len in [0, 1, 31, 32, 33, 127, 128, 129, 511, 512, 1024] {
                for maximal in [false, true] {
                    let weights: Vec<_> = (0..len)
                        .map(|i| {
                            Q12289Extension::new(std::array::from_fn(|k| {
                                if maximal {
                                    (Q - 1) as u16
                                } else {
                                    ((i * 97 + k * 53) as u64 % Q) as u16
                                }
                            }))
                        })
                        .collect();
                    let a: Vec<_> = (0..len)
                        .map(|i| {
                            if maximal {
                                (Q - 1) as u16
                            } else {
                                (i as u64 * 137 % Q) as u16
                            }
                        })
                        .collect();
                    let b: Vec<_> = (0..len)
                        .map(|i| ((i as u64 * 23 + Q - 1) % Q) as u16)
                        .collect();
                    let prepared = PreparedPublicDotProducts::new(&weights);
                    let expected = [&a, &b].map(|input| {
                        let mut sums = [0u64; K];
                        for (&coefficient, weight) in input.iter().zip(&weights) {
                            for (total, &coordinate) in sums.iter_mut().zip(&weight.0) {
                                *total += u64::from(coefficient) * u64::from(coordinate);
                            }
                        }
                        Q12289Extension::new(sums.map(|sum| (sum % Q) as u16))
                    });
                    assert_eq!(prepared.evaluate_pair(&a, &b), expected, "K={K}, len={len}");
                    assert_eq!(prepared.evaluate_pair_portable(&a, &b), expected);
                    #[cfg(target_arch = "x86_64")]
                    if std::arch::is_x86_feature_detected!("avx512f")
                        && std::arch::is_x86_feature_detected!("avx512bw")
                    {
                        // SAFETY: runtime features checked; construction above uses
                        // canonical coordinates and equal bounded lengths.
                        let actual =
                            unsafe { avx512::evaluate_public_pair(&prepared.coordinates, &a, &b) };
                        assert_eq!(actual.map(Q12289Extension), expected);
                    }
                }
            }
        }
        check::<9>();
        check::<10>();
        check::<11>();
    }

    #[test]
    #[should_panic(expected = "noncanonical coefficient")]
    fn prepared_public_dot_products_reject_noncanonical_coefficients() {
        let prepared = PreparedPublicDotProducts::new(&[Q12289Extension::<11>::ONE]);
        prepared.evaluate_pair(&[Q as u16], &[0]);
    }
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
#[repr(transparent)]
pub struct PowerCoordinates<const K: usize>([u16; K]);

impl<const K: usize> PowerCoordinates<K> {
    pub const ZERO: Self = Self([0; K]);

    /// Allocate initialized canonical zeros without an elementwise Rust fill.
    pub fn zeroed_vec(len: usize) -> Vec<Self> {
        // SAFETY: this private representation contains only [u16; K]. All-zero
        // bytes are valid and encode canonical zero for every supported basis;
        // Box preserves the element alignment and allocation layout.
        unsafe {
            Box::<[Self]>::new_zeroed_slice(len)
                .assume_init()
                .into_vec()
        }
    }

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

fn coordinate_output<const K: usize>(
    output: &mut [PowerCoordinates<K>],
) -> &mut [MaybeUninit<PowerCoordinates<K>>] {
    // SAFETY: MaybeUninit<T> has T's layout and accepts every initialized T.
    // The exclusive borrow is preserved; writers only replace valid values.
    unsafe { std::slice::from_raw_parts_mut(output.as_mut_ptr().cast(), output.len()) }
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

    /// Replace `value` with `alpha * value - scalar * subtract`.
    ///
    /// Both coordinate vectors must belong to this basis; `scalar` must be
    /// canonical in F_12289. The result is canonical and safe for integer lifts.
    #[inline]
    pub fn advance_sub_scaled(
        &self,
        value: &mut PowerCoordinates<K>,
        scalar: u16,
        subtract: PowerCoordinates<K>,
    ) {
        assert!(u64::from(scalar) < Q, "noncanonical base-field scalar");
        let top = u32::from(value.0[K - 1]);
        let scalar = u32::from(scalar);
        // Q^2 exceeds every subtracted product; the unreduced sum fits in u32.
        const OFFSET: u32 = (Q * Q) as u32;
        for k in (1..K).rev() {
            value.0[k] = reduce(
                u32::from(value.0[k - 1]) + u32::from(self.feedback[k]) * top + OFFSET
                    - scalar * u32::from(subtract.0[k]),
            );
        }
        value.0[0] =
            reduce(u32::from(self.feedback[0]) * top + OFFSET - scalar * u32::from(subtract.0[0]));
    }

    /// Emit start, alpha*start, ... without conversions or allocation per step.
    pub fn fill_powers(&self, start: PowerCoordinates<K>, output: &mut [PowerCoordinates<K>]) {
        self.fill_powers_uninit(start, coordinate_output(output));
    }

    /// Write every output slot once, without reading its previous contents.
    /// All slots are initialized on successful return; inputs and length checks
    /// are the same as the corresponding initialized-output method.
    pub fn fill_powers_uninit(
        &self,
        start: PowerCoordinates<K>,
        output: &mut [MaybeUninit<PowerCoordinates<K>>],
    ) {
        let mut value = start;
        for slot in output {
            slot.write(value);
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
        self.fill_powers_pair_uninit(starts, outputs.map(coordinate_output));
    }

    /// Write every output slot once, without reading its previous contents.
    /// All slots are initialized on successful return; inputs and length checks
    /// are the same as the corresponding initialized-output method.
    pub fn fill_powers_pair_uninit(
        &self,
        starts: [PowerCoordinates<K>; 2],
        outputs: [&mut [MaybeUninit<PowerCoordinates<K>>]; 2],
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
        self.fill_powers_uninit(starts[0], a);
        self.fill_powers_uninit(starts[1], b);
    }

    /// Emit ordinary powers alongside an adjoint sequence for a public polynomial.
    ///
    /// Starting at `starts`, the first sequence advances by alpha and the second
    /// by `alpha * value - polynomial[n-1-j] * subtract` after output j. Both
    /// outputs and the canonical F_12289 polynomial must have the same length n.
    /// All coordinate vectors must belong to this basis. Degree eleven uses a
    /// fused AVX512F+BW kernel when those features are enabled for the build.
    pub fn fill_powers_adjoint_pair(
        &self,
        starts: [PowerCoordinates<K>; 2],
        subtract: PowerCoordinates<K>,
        polynomial: &[u16],
        outputs: [&mut [PowerCoordinates<K>]; 2],
    ) {
        self.fill_powers_adjoint_pair_uninit(
            starts,
            subtract,
            polynomial,
            outputs.map(coordinate_output),
        );
    }

    /// Write every output slot once, without reading its previous contents.
    /// All slots are initialized on successful return; inputs and length checks
    /// are the same as the corresponding initialized-output method.
    pub fn fill_powers_adjoint_pair_uninit(
        &self,
        starts: [PowerCoordinates<K>; 2],
        subtract: PowerCoordinates<K>,
        polynomial: &[u16],
        outputs: [&mut [MaybeUninit<PowerCoordinates<K>>]; 2],
    ) {
        assert_eq!(
            outputs[0].len(),
            outputs[1].len(),
            "sequence lengths differ"
        );
        assert_eq!(
            outputs[0].len(),
            polynomial.len(),
            "polynomial length differs"
        );
        assert!(
            polynomial.iter().all(|&scalar| u64::from(scalar) < Q),
            "noncanonical base-field scalar"
        );
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw"
        ))]
        if K == 11 {
            // SAFETY: build features, degree, lengths, and canonical scalars
            // were checked; PowerCoordinates preserves canonical coordinates.
            unsafe {
                avx512::fill_powers_adjoint_pair(
                    &self.feedback,
                    starts,
                    subtract,
                    polynomial,
                    outputs,
                )
            };
            return;
        }
        let [a, b] = outputs;
        let [mut first, mut second] = starts;
        for j in 0..a.len() {
            a[j].write(first);
            b[j].write(second);
            if j + 1 < a.len() {
                self.advance(&mut first);
                self.advance_sub_scaled(&mut second, polynomial[a.len() - 1 - j], subtract);
            }
        }
    }

    /// Emit pairs of adjoint sequences with one shared subtraction vector.
    ///
    /// Every polynomial and output has the same length. All coordinates belong
    /// to this basis, and polynomial coefficients must be canonical. The degree
    /// eleven SIMD path packs two distinct public polynomials in each vector.
    pub fn fill_adjoint_pairs<const PAIRS: usize>(
        &self,
        starts: [[PowerCoordinates<K>; 2]; PAIRS],
        subtract: PowerCoordinates<K>,
        polynomials: [[&[u16]; 2]; PAIRS],
        outputs: [[&mut [PowerCoordinates<K>]; 2]; PAIRS],
    ) {
        self.fill_adjoint_pairs_uninit(
            starts,
            subtract,
            polynomials,
            outputs.map(|pair| pair.map(coordinate_output)),
        );
    }

    /// Write every output slot once, without reading its previous contents.
    /// All slots are initialized on successful return; inputs and length checks
    /// are the same as the corresponding initialized-output method.
    pub fn fill_adjoint_pairs_uninit<const PAIRS: usize>(
        &self,
        starts: [[PowerCoordinates<K>; 2]; PAIRS],
        subtract: PowerCoordinates<K>,
        polynomials: [[&[u16]; 2]; PAIRS],
        outputs: [[&mut [MaybeUninit<PowerCoordinates<K>>]; 2]; PAIRS],
    ) {
        const {
            assert!(PAIRS > 0);
        }
        let len = polynomials[0][0].len();
        for pair in 0..PAIRS {
            for side in 0..2 {
                assert_eq!(
                    polynomials[pair][side].len(),
                    len,
                    "polynomial lengths differ"
                );
                assert_eq!(outputs[pair][side].len(), len, "sequence lengths differ");
                assert!(
                    polynomials[pair][side]
                        .iter()
                        .all(|&scalar| u64::from(scalar) < Q),
                    "noncanonical base-field scalar"
                );
            }
        }
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw"
        ))]
        if K == 11 {
            // SAFETY: target features, degree, lengths and canonical scalars
            // were checked; PowerCoordinates preserves canonical coordinates.
            unsafe {
                avx512::fill_adjoint_pairs(&self.feedback, starts, subtract, polynomials, outputs)
            };
            return;
        }
        for (pair, outputs) in outputs.into_iter().enumerate() {
            for (side, output) in outputs.into_iter().enumerate() {
                let mut value = starts[pair][side];
                for (j, slot) in output.iter_mut().enumerate() {
                    slot.write(value);
                    if j + 1 < len {
                        self.advance_sub_scaled(
                            &mut value,
                            polynomials[pair][side][len - 1 - j],
                            subtract,
                        );
                    }
                }
            }
        }
    }

    /// Emit independent power/adjoint pairs with one shared public generator.
    ///
    /// Every polynomial and output must have the same length. Coordinates and
    /// subtraction vectors belong to this basis, and polynomial coefficients
    /// must be canonical. Interleaving pairs hides recurrence latency on AVX512;
    /// other targets retain the individual portable recurrences.
    pub fn fill_powers_adjoint_pairs<const PAIRS: usize>(
        &self,
        starts: [[PowerCoordinates<K>; 2]; PAIRS],
        subtract: [PowerCoordinates<K>; PAIRS],
        polynomials: [&[u16]; PAIRS],
        outputs: [[&mut [PowerCoordinates<K>]; 2]; PAIRS],
    ) {
        self.fill_powers_adjoint_pairs_uninit(
            starts,
            subtract,
            polynomials,
            outputs.map(|pair| pair.map(coordinate_output)),
        );
    }

    /// Write every output slot once, without reading its previous contents.
    /// All slots are initialized on successful return; inputs and length checks
    /// are the same as the corresponding initialized-output method.
    pub fn fill_powers_adjoint_pairs_uninit<const PAIRS: usize>(
        &self,
        starts: [[PowerCoordinates<K>; 2]; PAIRS],
        subtract: [PowerCoordinates<K>; PAIRS],
        polynomials: [&[u16]; PAIRS],
        outputs: [[&mut [MaybeUninit<PowerCoordinates<K>>]; 2]; PAIRS],
    ) {
        const {
            assert!(PAIRS > 0);
        }
        let len = polynomials[0].len();
        for i in 0..PAIRS {
            assert_eq!(polynomials[i].len(), len, "polynomial lengths differ");
            assert_eq!(outputs[i][0].len(), len, "sequence lengths differ");
            assert_eq!(outputs[i][1].len(), len, "sequence lengths differ");
            assert!(
                polynomials[i].iter().all(|&scalar| u64::from(scalar) < Q),
                "noncanonical base-field scalar"
            );
        }
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw"
        ))]
        if K == 11 {
            // SAFETY: build features, degree, matching lengths, and canonical
            // scalars were checked; coordinate constructors preserve their range.
            unsafe {
                avx512::fill_powers_adjoint_pairs(
                    &self.feedback,
                    starts,
                    subtract,
                    polynomials,
                    outputs,
                )
            };
            return;
        }
        for (i, output) in outputs.into_iter().enumerate() {
            self.fill_powers_adjoint_pair_uninit(starts[i], subtract[i], polynomials[i], output);
        }
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

    fn check_shared_adjoint_pairs<const K: usize, const PAIRS: usize>() {
        let mut alpha = Q12289Extension::<K>::ONE;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        let subtract = Q12289Extension([12288; K]);
        let fixed_starts: [[Q12289Extension<K>; 2]; PAIRS] = std::array::from_fn(|pair| {
            std::array::from_fn(|side| {
                Q12289Extension(std::array::from_fn(|k| {
                    ((pair * 1237 + side * 731 + k * 193) as u64 % Q) as u16
                }))
            })
        });
        let starts = fixed_starts.map(|pair| pair.map(|value| basis.encode(value)));
        for len in [0, 1, 2, 31, 32, 33, 127, 128, 129, 1024] {
            let polynomials: [[Vec<u16>; 2]; PAIRS] = std::array::from_fn(|pair| {
                std::array::from_fn(|side| {
                    (0..len)
                        .map(|j| [0, 1, 6144, 6145, 12287, 12288][(pair + side + j) % 6])
                        .collect()
                })
            });
            let expected: [[Vec<PowerCoordinates<K>>; 2]; PAIRS] = std::array::from_fn(|pair| {
                std::array::from_fn(|side| {
                    let mut value = fixed_starts[pair][side];
                    (0..len)
                        .map(|j| {
                            let out = basis.encode(value);
                            let mut scalar = Q12289Extension::ZERO;
                            scalar.0[0] = polynomials[pair][side][len - 1 - j];
                            value = alpha.mul(value).sub(scalar.mul(subtract));
                            out
                        })
                        .collect()
                })
            });
            let sentinel = PowerCoordinates([u16::MAX; K]);
            let mut actual: [[Vec<PowerCoordinates<K>>; 2]; PAIRS] =
                std::array::from_fn(|_| std::array::from_fn(|_| vec![sentinel; len + 4]));
            let mut slots = actual.iter_mut();
            let outputs = std::array::from_fn(|_| {
                let [a, b] = slots.next().unwrap();
                [&mut a[2..len + 2], &mut b[2..len + 2]]
            });
            let polynomials = std::array::from_fn(|pair| {
                std::array::from_fn(|side| polynomials[pair][side].as_slice())
            });
            basis.fill_adjoint_pairs(starts, basis.encode(subtract), polynomials, outputs);
            for (pair, reference) in actual.iter().zip(&expected) {
                for (output, reference) in pair.iter().zip(reference) {
                    assert_eq!(&output[2..len + 2], reference);
                    assert_eq!(&output[..2], &[sentinel; 2]);
                    assert_eq!(&output[len + 2..], &[sentinel; 2]);
                }
            }
            #[cfg(target_arch = "x86_64")]
            if K == 11
                && std::arch::is_x86_feature_detected!("avx512f")
                && std::arch::is_x86_feature_detected!("avx512bw")
            {
                for pair in &mut actual {
                    for output in pair {
                        output.fill(sentinel);
                    }
                }
                let mut slots = actual.iter_mut();
                let outputs = std::array::from_fn(|_| {
                    let [a, b] = slots.next().unwrap();
                    [&mut a[2..len + 2], &mut b[2..len + 2]]
                });
                // SAFETY: runtime feature/degree checks above and canonical
                // equal-length inputs; the mutable slices remain disjoint.
                unsafe {
                    avx512::fill_adjoint_pairs(
                        &basis.feedback,
                        starts,
                        basis.encode(subtract),
                        polynomials,
                        outputs.map(|pair| pair.map(coordinate_output)),
                    )
                };
                for (pair, reference) in actual.iter().zip(&expected) {
                    for (output, reference) in pair.iter().zip(reference) {
                        assert_eq!(&output[2..len + 2], reference);
                        assert_eq!(&output[..2], &[sentinel; 2]);
                        assert_eq!(&output[len + 2..], &[sentinel; 2]);
                    }
                }
            }
        }
    }

    #[test]
    fn shared_adjoint_pairs_match_fixed_basis_with_guarded_tails() {
        check_shared_adjoint_pairs::<9, 2>();
        check_shared_adjoint_pairs::<10, 2>();
        check_shared_adjoint_pairs::<11, 1>();
        check_shared_adjoint_pairs::<11, 2>();
        check_shared_adjoint_pairs::<11, 4>();
        check_shared_adjoint_pairs::<11, 8>();
    }

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

    fn check_sub_scaled<const K: usize>() {
        for seed in 0..8 {
            let alpha = Q12289Extension(std::array::from_fn(|i| {
                if seed == 0 {
                    u16::from(i == 1)
                } else {
                    ((seed * 1237 + i * 97) % Q as usize) as u16
                }
            }));
            let basis = PowerBasis::try_new(alpha).unwrap();
            let subtract = Q12289Extension(std::array::from_fn(|i| {
                ((i * 931 + seed * 73) % Q as usize) as u16
            }));
            let mut expected = Q12289Extension([12288; K]);
            let mut actual = basis.encode(expected);
            for scalar in [0, 1, 12288, 2, 6144, 12288, 0, 137] {
                let mut fixed_scalar = Q12289Extension::ZERO;
                fixed_scalar.0[0] = scalar;
                expected = expected.mul(alpha).sub(subtract.mul(fixed_scalar));
                basis.advance_sub_scaled(&mut actual, scalar, basis.encode(subtract));
                assert_eq!(actual, basis.encode(expected));
                assert_eq!(basis.decode(actual), expected);
                assert!(actual.0.iter().all(|&v| u64::from(v) < Q));
            }
        }
    }

    #[test]
    fn shifted_subtraction_matches_fixed_basis_reference() {
        check_sub_scaled::<9>();
        check_sub_scaled::<10>();
        check_sub_scaled::<11>();
    }

    #[test]
    #[should_panic(expected = "noncanonical base-field scalar")]
    fn shifted_subtraction_rejects_noncanonical_scalar() {
        let mut alpha = Q12289Extension::<11>::ZERO;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        let mut value = PowerCoordinates::ZERO;
        basis.advance_sub_scaled(&mut value, 12289, PowerCoordinates::ZERO);
    }

    fn check_adjoint_pair<const K: usize>() {
        for seed in 0..8 {
            let alpha = Q12289Extension(std::array::from_fn(|i| {
                if seed == 0 {
                    u16::from(i == 1)
                } else {
                    ((seed * 1237 + i * 97) % Q as usize) as u16
                }
            }));
            let basis = PowerBasis::try_new(alpha).unwrap();
            let subtract = Q12289Extension::<K>(std::array::from_fn(|i| {
                [0, 1, 6144, 6145, 12288][(i + seed) % 5]
            }));
            let starts = [Q12289Extension::ONE, Q12289Extension([12288; K])];
            for n in [0, 1, 2, 3, 31, 32, 33, 512, 1024] {
                let polynomial: Vec<_> = (0..n)
                    .map(|i| [0, 1, 6144, 6145, 12288][(i + seed) % 5])
                    .collect();
                let mut a = vec![PowerCoordinates::ZERO; n];
                let mut b = a.clone();
                basis.fill_powers_adjoint_pair(
                    starts.map(|v| basis.encode(v)),
                    basis.encode(subtract),
                    &polynomial,
                    [&mut a, &mut b],
                );
                let mut expected = starts;
                for j in 0..n {
                    assert_eq!(a[j], basis.encode(expected[0]));
                    assert_eq!(b[j], basis.encode(expected[1]));
                    assert!(a[j].0.iter().chain(&b[j].0).all(|&v| u64::from(v) < Q));
                    let mut scalar = Q12289Extension::ZERO;
                    scalar.0[0] = polynomial[n - 1 - j];
                    expected[0] = expected[0].mul(alpha);
                    expected[1] = expected[1].mul(alpha).sub(subtract.mul(scalar));
                }
            }
        }
    }

    #[test]
    fn adjoint_pair_matches_fixed_basis_reference() {
        check_adjoint_pair::<9>();
        check_adjoint_pair::<10>();
        check_adjoint_pair::<11>();
    }

    fn check_adjoint_pairs<const K: usize>() {
        let alpha = Q12289Extension(std::array::from_fn(|k| u16::from(k == 1)));
        let basis = PowerBasis::try_new(alpha).unwrap();
        let fixed: [[Q12289Extension<K>; 2]; 2] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                Q12289Extension(std::array::from_fn(|k| {
                    ((i * 311 + j * 79 + k * 101) % Q as usize) as u16
                }))
            })
        });
        let subtract = fixed.map(|v| v[1]);
        for len in [0, 1, 3, 17, 513] {
            let polynomials: [Vec<u16>; 2] = std::array::from_fn(|i| {
                (0..len)
                    .map(|j| ((i * 73 + j * 931) % Q as usize) as u16)
                    .collect()
            });
            let mut buffers = vec![[PowerCoordinates::ZERO; 2]; len * 2];
            // Separate vectors exercise the actual signature-major API layout.
            let mut a = vec![PowerCoordinates::ZERO; len * 2];
            let mut b = a.clone();
            let (a0, a1) = a.split_at_mut(len);
            let (b0, b1) = b.split_at_mut(len);
            basis.fill_powers_adjoint_pairs(
                fixed.map(|v| v.map(|x| basis.encode(x))),
                subtract.map(|x| basis.encode(x)),
                [&polynomials[0], &polynomials[1]],
                [[a0, b0], [a1, b1]],
            );
            for i in 0..2 {
                let [mut first, mut second] = fixed[i];
                for j in 0..len {
                    buffers[i * len + j] = [basis.encode(first), basis.encode(second)];
                    first = first.mul(alpha);
                    let mut scalar = Q12289Extension::ZERO;
                    scalar.0[0] = polynomials[i][len - 1 - j];
                    second = second.mul(alpha).sub(subtract[i].mul(scalar));
                }
            }
            for (j, expected) in buffers.iter().enumerate() {
                assert_eq!(a[j], expected[0]);
                assert_eq!(b[j], expected[1]);
            }
        }
    }

    #[test]
    fn interleaved_dispatch_matches_fixed_basis_reference() {
        check_adjoint_pairs::<9>();
        check_adjoint_pairs::<10>();
        check_adjoint_pairs::<11>();
    }

    #[test]
    #[should_panic(expected = "noncanonical base-field scalar")]
    fn interleaved_pairs_reject_noncanonical_polynomials() {
        let basis = PowerBasis::try_new(Q12289Extension::<11>(std::array::from_fn(|k| {
            u16::from(k == 1)
        })))
        .unwrap();
        basis.fill_powers_adjoint_pairs(
            [[PowerCoordinates::ZERO; 2]; 2],
            [PowerCoordinates::ZERO; 2],
            [&[0], &[Q as u16]],
            [
                [&mut [PowerCoordinates::ZERO], &mut [PowerCoordinates::ZERO]],
                [&mut [PowerCoordinates::ZERO], &mut [PowerCoordinates::ZERO]],
            ],
        );
    }

    #[test]
    #[should_panic(expected = "polynomial lengths differ")]
    fn interleaved_pairs_reject_mismatched_polynomials() {
        let basis = PowerBasis::try_new(Q12289Extension::<11>(std::array::from_fn(|k| {
            u16::from(k == 1)
        })))
        .unwrap();
        basis.fill_powers_adjoint_pairs(
            [[PowerCoordinates::ZERO; 2]; 2],
            [PowerCoordinates::ZERO; 2],
            [&[0], &[]],
            [
                [&mut [PowerCoordinates::ZERO], &mut [PowerCoordinates::ZERO]],
                [&mut [], &mut []],
            ],
        );
    }

    #[test]
    #[should_panic(expected = "polynomial length differs")]
    fn adjoint_pair_rejects_polynomial_length() {
        let mut alpha = Q12289Extension::<11>::ZERO;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        basis.fill_powers_adjoint_pair(
            [PowerCoordinates::ZERO; 2],
            PowerCoordinates::ZERO,
            &[0],
            [&mut [], &mut []],
        );
    }

    #[test]
    #[should_panic(expected = "noncanonical base-field scalar")]
    fn adjoint_pair_rejects_noncanonical_polynomial() {
        let mut alpha = Q12289Extension::<11>::ZERO;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        basis.fill_powers_adjoint_pair(
            [PowerCoordinates::ZERO; 2],
            PowerCoordinates::ZERO,
            &[Q as u16],
            [&mut [PowerCoordinates::ZERO], &mut [PowerCoordinates::ZERO]],
        );
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
