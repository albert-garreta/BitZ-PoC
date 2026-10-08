use std::sync::OnceLock;

const Q: u32 = 12_289;
#[cfg(any(test, feature = "bench-internals"))]
const SCHOOLBOOK_THRESHOLD: usize = 32;

/// Reusable storage for an ordinary Falcon polynomial product.
///
/// Coefficients are congruent to the integer product modulo 12289. The output
/// has length `2 * n`, including its zero final coefficient; retaining the high
/// coefficients is necessary for the ring-quotient witness.
pub(crate) struct PolynomialWorkspace {
    output: Vec<i64>,
    storage: ProductStorage,
}

enum ProductStorage {
    #[cfg(any(test, feature = "bench-internals"))]
    Karatsuba {
        left: Vec<i64>,
        right: Vec<i64>,
        scratch: Vec<i64>,
    },
    Ntt {
        left: Vec<u32>,
        right: Vec<u32>,
        tables: &'static NttTables,
    },
}

impl PolynomialWorkspace {
    pub(crate) fn new(n: usize) -> Self {
        #[cfg(feature = "bench-internals")]
        if let Some(backend) = std::env::var_os("BITZ_FALCON_POLYNOMIAL_BACKEND") {
            match backend.to_str() {
                Some("scratch") => return Self::karatsuba(n),
                Some("ntt") => {}
                _ => panic!("BITZ_FALCON_POLYNOMIAL_BACKEND must be scratch or ntt"),
            }
        }
        Self::ntt(n)
    }

    #[cfg(any(test, feature = "bench-internals"))]
    pub(crate) fn karatsuba(n: usize) -> Self {
        assert!(matches!(n, 512 | 1024));
        Self {
            output: vec![0; 2 * n],
            storage: ProductStorage::Karatsuba {
                left: vec![0; n],
                right: vec![0; n],
                scratch: vec![0; 4 * n],
            },
        }
    }

    pub(crate) fn ntt(n: usize) -> Self {
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
            storage: ProductStorage::Ntt {
                left: vec![0; 2 * n],
                right: vec![0; 2 * n],
                tables,
            },
        }
    }

    pub(crate) fn product(&mut self, h: &[u16], s2: &[i16]) -> &[i64] {
        let n = self.output.len() / 2;
        assert_eq!(h.len(), n);
        assert_eq!(s2.len(), n);
        match &mut self.storage {
            #[cfg(any(test, feature = "bench-internals"))]
            ProductStorage::Karatsuba {
                left,
                right,
                scratch,
            } => {
                for ((left, right), (&h, &s2)) in
                    left.iter_mut().zip(right.iter_mut()).zip(h.iter().zip(s2))
                {
                    *left = i64::from(h);
                    *right = i64::from(s2);
                }
                karatsuba_into(left, right, &mut self.output, scratch);
            }
            ProductStorage::Ntt {
                left,
                right,
                tables,
            } => {
                for i in 0..n {
                    left[i] = u32::from(h[i]) % Q;
                    // Adding 3q makes every i16 nonnegative before reduction.
                    right[i] = (i32::from(s2[i]) + 3 * Q as i32) as u32 % Q;
                }
                left[n..].fill(0);
                right[n..].fill(0);
                forward_ntt(left, tables);
                forward_ntt(right, tables);
                for (left, right) in left.iter_mut().zip(right.iter()) {
                    *left = *left * *right % Q;
                }
                inverse_ntt(left, tables);
                for (output, &value) in self.output.iter_mut().zip(left.iter()) {
                    *output = i64::from(value);
                }
            }
        }
        &self.output
    }
}

/// Exact product, retained for callers requiring integer coefficients.
/// Falcon's 14-bit public and signed15 witness coefficients at degree 1024
/// leave ample headroom in i64, including recursive sums.
#[cfg(test)]
fn integer_polynomial_product(a: &[i64], b: &[i64]) -> Vec<i64> {
    let n = a.len();
    assert_eq!(n, b.len());
    assert!(n == 0 || n.is_power_of_two());
    let mut out = vec![0; 2 * n];
    let mut scratch = vec![0; 4 * n];
    karatsuba_into(a, b, &mut out, &mut scratch);
    out
}

#[cfg(any(test, feature = "bench-internals"))]
fn karatsuba_into(a: &[i64], b: &[i64], out: &mut [i64], scratch: &mut [i64]) {
    let n = a.len();
    if n <= SCHOOLBOOK_THRESHOLD {
        out.fill(0);
        for (i, &x) in a.iter().enumerate() {
            for (j, &y) in b.iter().enumerate() {
                out[i + j] += x * y;
            }
        }
        return;
    }
    let h = n / 2;
    let (lo, hi) = out.split_at_mut(n);
    karatsuba_into(&a[..h], &b[..h], lo, scratch);
    karatsuba_into(&a[h..], &b[h..], hi, scratch);
    // S(n) = 2n + S(n/2) < 4n; siblings reuse the same scratch.
    let (asum, scratch) = scratch.split_at_mut(h);
    let (bsum, scratch) = scratch.split_at_mut(h);
    let (mid, scratch) = scratch.split_at_mut(n);
    for i in 0..h {
        asum[i] = a[i] + a[h + i];
        bsum[i] = b[i] + b[h + i];
    }
    karatsuba_into(asum, bsum, mid, scratch);
    for i in 0..n {
        mid[i] -= lo[i] + hi[i];
    }
    for i in 0..n {
        out[i + h] += mid[i];
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

    fn compare(
        h: &[u16],
        s2: &[i16],
        karatsuba: &mut PolynomialWorkspace,
        ntt: &mut PolynomialWorkspace,
    ) {
        let expected = schoolbook(h, s2);
        assert_eq!(karatsuba.product(h, s2), expected);
        let actual = ntt.product(h, s2);
        for (&actual, &expected) in actual.iter().zip(&expected) {
            assert_eq!(actual, expected.rem_euclid(i64::from(Q)));
        }
        assert_eq!(actual.last(), Some(&0));
    }

    #[test]
    fn workspace_products_match_schoolbook_and_clear_reused_storage() {
        for n in [512, 1024] {
            let mut karatsuba = PolynomialWorkspace::karatsuba(n);
            let mut ntt = PolynomialWorkspace::ntt(n);
            let mut h = vec![Q as u16 - 1; n];
            let mut s2 = vec![-16384; n];
            compare(&h, &s2, &mut karatsuba, &mut ntt);
            s2.fill(16383);
            compare(&h, &s2, &mut karatsuba, &mut ntt);
            for i in 0..n {
                s2[i] = if i & 1 == 0 { -16384 } else { 16383 };
            }
            compare(&h, &s2, &mut karatsuba, &mut ntt);
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
                compare(&h, &s2, &mut karatsuba, &mut ntt);
            }
            h.fill(0);
            s2.fill(0);
            compare(&h, &s2, &mut karatsuba, &mut ntt);
            for (i, j) in [(0, 0), (n - 1, 1), (n - 1, n - 1)] {
                h[i] = Q as u16 - 1;
                s2[j] = -16384;
                compare(&h, &s2, &mut karatsuba, &mut ntt);
                h[i] = 0;
                s2[j] = 0;
            }
            compare(&h, &s2, &mut karatsuba, &mut ntt);
        }
    }

    #[test]
    fn exact_wrapper_matches_schoolbook() {
        for n in [0, 1, 16, 32, 64, 512, 1024] {
            let h: Vec<_> = (0..n).map(|i| (i * 79 % Q as usize) as u16).collect();
            let s2: Vec<_> = (0..n).map(|i| (i % 47) as i16 - 23).collect();
            let left: Vec<_> = h.iter().copied().map(i64::from).collect();
            let right: Vec<_> = s2.iter().copied().map(i64::from).collect();
            assert_eq!(
                integer_polynomial_product(&left, &right),
                schoolbook(&h, &s2)
            );
        }
    }
}
