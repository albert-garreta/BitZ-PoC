use std::sync::OnceLock;

const Q: u32 = 12_289;

/// Reusable storage for an ordinary Falcon polynomial product.
///
/// Coefficients are congruent to the integer product modulo 12289. The output
/// has length `2 * n`, including its zero final coefficient; retaining the high
/// coefficients is necessary for the ring-quotient witness.
pub(crate) struct PolynomialWorkspace {
    output: Vec<i64>,
    left: Vec<u32>,
    right: Vec<u32>,
    tables: &'static NttTables,
}

impl PolynomialWorkspace {
    pub(crate) fn new(n: usize) -> Self {
        static TABLES_512: OnceLock<NttTables> = OnceLock::new();
        static TABLES_1024: OnceLock<NttTables> = OnceLock::new();
        let tables = match n {
            512 => &TABLES_512,
            1024 => &TABLES_1024,
            _ => panic!("unsupported Falcon polynomial degree"),
        }
        .get_or_init(|| NttTables::new(2 * n));
        Self {
            output: vec![0; 2 * n],
            left: vec![0; 2 * n],
            right: vec![0; 2 * n],
            tables,
        }
    }

    pub(crate) fn product(&mut self, h: &[u16], s2: &[i16]) -> &[i64] {
        let n = self.output.len() / 2;
        assert_eq!(h.len(), n);
        assert_eq!(s2.len(), n);
        let (left, right) = (&mut self.left, &mut self.right);
        for i in 0..n {
            left[i] = u32::from(h[i]) % Q;
            // Adding 3q makes every i16 nonnegative before reduction.
            right[i] = (i32::from(s2[i]) + 3 * Q as i32) as u32 % Q;
        }
        left[n..].fill(0);
        right[n..].fill(0);
        forward_ntt(left, self.tables);
        forward_ntt(right, self.tables);
        for (left, right) in left.iter_mut().zip(right.iter()) {
            *left = *left * *right % Q;
        }
        inverse_ntt(left, self.tables);
        for (output, &value) in self.output.iter_mut().zip(left.iter()) {
            *output = i64::from(value);
        }
        &self.output
    }
}

struct NttTables {
    forward: Vec<Vec<u32>>,
    inverse: Vec<Vec<u32>>,
    inverse_len: u32,
}

impl NttTables {
    fn new(len: usize) -> Self {
        // 11 generates F_12289*. Both 1024 and 2048 divide q - 1.
        let root = power(11, (Q - 1) / len as u32);
        let inverse_root = power(root, Q - 2);
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        let mut width = len;
        while width >= 2 {
            let powers = |root| {
                let step = power(root, (len / width) as u32);
                let mut value = 1;
                (0..width / 2)
                    .map(|_| {
                        let next = value;
                        value = value * step % Q;
                        next
                    })
                    .collect()
            };
            forward.push(powers(root));
            inverse.push(powers(inverse_root));
            width /= 2;
        }
        inverse.reverse();
        Self {
            forward,
            inverse,
            inverse_len: power(len as u32, Q - 2),
        }
    }
}

fn power(mut value: u32, mut exponent: u32) -> u32 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result * value % Q;
        }
        value = value * value % Q;
        exponent >>= 1;
    }
    result
}

#[inline(always)]
fn reduce_once(value: u32) -> u32 {
    let reduced = value.wrapping_sub(Q);
    reduced.wrapping_add(Q & 0u32.wrapping_sub(reduced >> 31))
}

fn forward_ntt(values: &mut [u32], tables: &NttTables) {
    // Decimation in frequency leaves evaluations in bit-reversed order.
    for twiddles in &tables.forward {
        let half = twiddles.len();
        for block in values.chunks_exact_mut(2 * half) {
            let (lo, hi) = block.split_at_mut(half);
            for ((lo, hi), &twiddle) in lo.iter_mut().zip(hi).zip(twiddles) {
                let sum = reduce_once(*lo + *hi);
                let difference = reduce_once(*lo + Q - *hi);
                *lo = sum;
                *hi = difference * twiddle % Q;
            }
        }
    }
}

fn inverse_ntt(values: &mut [u32], tables: &NttTables) {
    // Decimation in time consumes that order without a permutation pass.
    for twiddles in &tables.inverse {
        let half = twiddles.len();
        for block in values.chunks_exact_mut(2 * half) {
            let (lo, hi) = block.split_at_mut(half);
            for ((lo, hi), &twiddle) in lo.iter_mut().zip(hi).zip(twiddles) {
                let product = *hi * twiddle % Q;
                *hi = reduce_once(*lo + Q - product);
                *lo = reduce_once(*lo + product);
            }
        }
    }
    for value in values {
        *value = *value * tables.inverse_len % Q;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schoolbook(h: &[u16], s2: &[i16]) -> Vec<i64> {
        let mut expected = vec![0; 2 * h.len()];
        for (i, &left) in h.iter().enumerate() {
            for (j, &right) in s2.iter().enumerate() {
                expected[i + j] += i64::from(left) * i64::from(right);
            }
        }
        expected
    }

    fn compare(h: &[u16], s2: &[i16], workspace: &mut PolynomialWorkspace) {
        let expected = schoolbook(h, s2);
        let actual = workspace.product(h, s2);
        for (&actual, &expected) in actual.iter().zip(&expected) {
            assert_eq!(actual, expected.rem_euclid(i64::from(Q)));
        }
        assert_eq!(actual.last(), Some(&0));
    }

    #[test]
    fn workspace_products_match_schoolbook_and_clear_reused_storage() {
        for n in [512, 1024] {
            let mut workspace = PolynomialWorkspace::new(n);
            let mut h = vec![Q as u16 - 1; n];
            let mut s2 = vec![-16384; n];
            compare(&h, &s2, &mut workspace);
            s2.fill(16383);
            compare(&h, &s2, &mut workspace);
            for i in 0..n {
                s2[i] = if i & 1 == 0 { -16384 } else { 16383 };
            }
            compare(&h, &s2, &mut workspace);
            let mut seed = 0x4641_4c43_4f4e_u64;
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            };
            for _ in 0..8 {
                for i in 0..n {
                    h[i] = (next() % u64::from(Q)) as u16;
                    s2[i] = (next() % 32768) as i16 - 16384;
                }
                compare(&h, &s2, &mut workspace);
            }
            h.fill(0);
            s2.fill(0);
            compare(&h, &s2, &mut workspace);
            for (i, j) in [(0, 0), (n - 1, 1), (n - 1, n - 1)] {
                h[i] = Q as u16 - 1;
                s2[j] = -16384;
                compare(&h, &s2, &mut workspace);
                h[i] = 0;
                s2[j] = 0;
            }
            compare(&h, &s2, &mut workspace);
        }
    }
}

#[cfg(test)]
mod exact_product_tests {
    use super::PolynomialWorkspace;
    /// Exact integer product, split into low and high degree-sized halves.
    /// Falcon's 14-bit public and signed15 witness coefficients at degree 1024
    /// leave ample headroom in i64, including recursive Karatsuba sums.
    fn public_signature_product<const N: usize>(h: &[u16; N], s2: &[i16; N]) -> Box<[[i64; N]; 2]> {
        let left: Box<[i64; N]> = Box::new(std::array::from_fn(|i| i64::from(h[i])));
        let right: Box<[i64; N]> = Box::new(std::array::from_fn(|i| i64::from(s2[i])));
        let mut out = Box::new([[0; N]; 2]);
        integer_polynomial_product(&left[..], &right[..], out.as_flattened_mut());
        out
    }

    /// Accumulate an equal, power-of-two length product into a zeroed output.
    fn integer_polynomial_product(a: &[i64], b: &[i64], out: &mut [i64]) {
        let n = a.len();
        debug_assert!(n.is_power_of_two());
        debug_assert_eq!(n, b.len());
        debug_assert_eq!(out.len(), 2 * n);
        if n <= 32 {
            for (i, &x) in a.iter().enumerate() {
                // Check the output window once so the fixed-size leaf can be
                // vectorized without a bounds check in its multiply-add loop.
                for (value, &y) in out[i..i + b.len()].iter_mut().zip(b) {
                    *value += x * y;
                }
            }
            return;
        }
        let h = n / 2;
        let mut lo = vec![0; n];
        let mut hi = vec![0; n];
        let mut mid = vec![0; n];
        integer_polynomial_product(&a[..h], &b[..h], &mut lo);
        integer_polynomial_product(&a[h..], &b[h..], &mut hi);
        let asum: Vec<_> = (0..h).map(|i| a[i] + a[h + i]).collect();
        let bsum: Vec<_> = (0..h).map(|i| b[i] + b[h + i]).collect();
        integer_polynomial_product(&asum, &bsum, &mut mid);
        for i in 0..n {
            out[i] += lo[i];
            out[i + n] += hi[i];
            out[i + h] += mid[i] - lo[i] - hi[i];
        }
    }

    fn check<const N: usize>() {
        for (h, s2) in [
            ([0; N], [0; N]),
            ([12_288; N], [-16_384; N]),
            ([12_288; N], [16_383; N]),
            (
                std::array::from_fn(|i| ((137 * i + 19) % 12_289) as u16),
                std::array::from_fn(|i| ((499 * i + 73) % 32_768) as i16 - 16_384),
            ),
        ] {
            let actual = public_signature_product(&h, &s2);
            let mut expected = Box::new([[0i64; N]; 2]);
            for (i, &x) in h.iter().enumerate() {
                for (j, &y) in s2.iter().enumerate() {
                    expected[(i + j) / N][(i + j) % N] += i64::from(x) * i64::from(y);
                }
            }
            assert_eq!(actual, expected);
            assert_eq!(actual[1][N - 1], 0);
            let mut workspace = PolynomialWorkspace::new(N);
            let residues = workspace.product(&h, &s2);
            for (&residue, &integer) in residues.iter().zip(actual.as_flattened()) {
                assert_eq!(residue, integer.rem_euclid(12_289));
            }
        }
    }

    #[test]
    fn fixed_products_match_schoolbook_at_both_degrees() {
        check::<512>();
        check::<1024>();
    }
}
