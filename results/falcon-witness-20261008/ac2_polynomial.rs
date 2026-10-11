/// Exact Karatsuba product of equal, power-of-two length vectors.
/// Falcon's 14-bit public and signed15 witness coefficients at degree 1024
/// leave ample headroom in i64, including recursive sums.
pub(crate) fn integer_polynomial_product(a: &[i64], b: &[i64]) -> Vec<i64> {
    let n = a.len();
    debug_assert_eq!(n, b.len());
    let mut out = vec![0; 2 * n];
    if n <= 32 {
        for (i, &x) in a.iter().enumerate() {
            for (j, &y) in b.iter().enumerate() {
                out[i + j] += x * y;
            }
        }
        return out;
    }
    let h = n / 2;
    let lo = integer_polynomial_product(&a[..h], &b[..h]);
    let hi = integer_polynomial_product(&a[h..], &b[h..]);
    let asum: Vec<_> = (0..h).map(|i| a[i] + a[h + i]).collect();
    let bsum: Vec<_> = (0..h).map(|i| b[i] + b[h + i]).collect();
    let mid = integer_polynomial_product(&asum, &bsum);
    for i in 0..n {
        out[i] += lo[i];
        out[i + n] += hi[i];
        out[i + h] += mid[i] - lo[i] - hi[i];
    }
    out
}
