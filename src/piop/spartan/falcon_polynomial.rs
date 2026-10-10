/// Exact integer product, split into low and high degree-sized halves.
/// Falcon's 14-bit public and signed15 witness coefficients at degree 1024
/// leave ample headroom in i64, including recursive Karatsuba sums.
pub(crate) fn public_signature_product<const N: usize>(
    h: &[u16; N],
    s2: &[i16; N],
) -> Box<[[i64; N]; 2]> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[test]
    fn fixed_products_match_schoolbook_at_both_degrees() {
        check::<512>();
        check::<1024>();
    }
}
