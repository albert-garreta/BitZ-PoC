//! Slice kernels for the two-round lookahead, preserving scalar messages.
use crate::field::{Gf128, Gf128Product};

pub(super) fn accumulate(f: &[Gf128], b: &[Gf128]) -> [Gf128Product; 8] {
    assert_eq!(f.len(), b.len());
    assert!(f.len().is_multiple_of(4));
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: target features are statically enabled and all loads are bounded.
    return unsafe { x86::accumulate(f, b) };

    #[cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    )))]
    scalar_accumulate(f, b)
}

fn scalar_accumulate(f: &[Gf128], b: &[Gf128]) -> [Gf128Product; 8] {
    let mut acc = [Gf128Product::zero(); 8];
    for (f, b) in f.chunks_exact(4).zip(b.chunks_exact(4)) {
        super::lookahead_accum_group(f.try_into().unwrap(), b.try_into().unwrap(), &mut acc);
    }
    acc
}

pub(super) fn fold_two(source: &[Gf128], base: usize, out: &mut [Gf128], a: Gf128, b: Gf128) {
    assert!(base <= source.len() / 4 && out.len() <= source.len() / 4 - base);
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: target features are statically enabled; the check covers inputs.
    return unsafe { x86::fold_two(source, base, out, a, b) };

    #[cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    )))]
    scalar_fold_two(source, base, out, a, b);
}

fn scalar_fold_two(source: &[Gf128], base: usize, out: &mut [Gf128], a: Gf128, b: Gf128) {
    for (j, out) in out.iter_mut().enumerate() {
        let f = &source[4 * (base + j)..];
        let lo = f[0] + (f[0] + f[1]) * a;
        let hi = f[2] + (f[2] + f[3]) * a;
        *out = lo + (lo + hi) * b;
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "pclmulqdq",
    target_feature = "sse4.1",
    target_feature = "vpclmulqdq"
))]
mod x86 {
    use super::*;
    use crate::field::gf128_kernels::x86_64::{WideGhashX4, ghash_mul_x4};
    use core::arch::x86_64::*;

    /// Transpose four consecutive four-element groups into four SIMD columns.
    #[target_feature(enable = "avx512f,avx512bw,vpclmulqdq,pclmulqdq,sse4.1")]
    unsafe fn columns(p: *const Gf128) -> [__m512i; 4] {
        unsafe {
            let r0 = _mm512_loadu_si512(p.cast());
            let r1 = _mm512_loadu_si512(p.add(4).cast());
            let r2 = _mm512_loadu_si512(p.add(8).cast());
            let r3 = _mm512_loadu_si512(p.add(12).cast());
            let lo = _mm512_set_epi64(11, 10, 9, 8, 3, 2, 1, 0);
            let hi = _mm512_set_epi64(15, 14, 13, 12, 7, 6, 5, 4);
            let even = _mm512_set_epi64(13, 12, 9, 8, 5, 4, 1, 0);
            let odd = _mm512_set_epi64(15, 14, 11, 10, 7, 6, 3, 2);
            let a = _mm512_permutex2var_epi64(r0, lo, r1);
            let b = _mm512_permutex2var_epi64(r0, hi, r1);
            let c = _mm512_permutex2var_epi64(r2, lo, r3);
            let d = _mm512_permutex2var_epi64(r2, hi, r3);
            [
                _mm512_permutex2var_epi64(a, even, c),
                _mm512_permutex2var_epi64(a, odd, c),
                _mm512_permutex2var_epi64(b, even, d),
                _mm512_permutex2var_epi64(b, odd, d),
            ]
        }
    }

    #[target_feature(enable = "avx512f,avx512bw,vpclmulqdq,pclmulqdq,sse4.1")]
    pub(super) unsafe fn accumulate(f: &[Gf128], b: &[Gf128]) -> [Gf128Product; 8] {
        unsafe {
            // Store the eight independent products from lookahead_accum_group;
            // combine them only after horizontally XORing each SIMD lane.
            let mut p = [WideGhashX4::zero(); 8];
            let mut i = 0;
            while i + 16 <= f.len() {
                let a = columns(f.as_ptr().add(i));
                let b = columns(b.as_ptr().add(i));
                let da0 = _mm512_xor_si512(a[0], a[1]);
                let da1 = _mm512_xor_si512(a[2], a[3]);
                let db0 = _mm512_xor_si512(b[0], b[1]);
                let db1 = _mm512_xor_si512(b[2], b[3]);
                let sa = _mm512_xor_si512(a[0], a[2]);
                let sb = _mm512_xor_si512(b[0], b[2]);
                let dsa = _mm512_xor_si512(da0, da1);
                let dsb = _mm512_xor_si512(db0, db1);
                p[0].mul_acc(a[0], b[0]);
                p[1].mul_acc(da0, db0);
                p[2].mul_acc(a[1], b[1]);
                p[3].mul_acc(a[2], b[2]);
                p[4].mul_acc(da1, db1);
                p[5].mul_acc(sa, sb);
                p[6].mul_acc(dsa, dsb);
                p[7].mul_acc(_mm512_xor_si512(sa, dsa), _mm512_xor_si512(sb, dsb));
                i += 16;
            }
            let [m1, m2, m3, p1, p2, n1, n2, n3] = p.map(|v| v.fold());
            let mut acc = [m1 ^ p1, m2 ^ p2, m1, m1 ^ m2 ^ m3, m2, n1, n1 ^ n2 ^ n3, n2];
            let tail = scalar_accumulate(&f[i..], &b[i..]);
            for (a, b) in acc.iter_mut().zip(tail) {
                *a ^= b;
            }
            acc
        }
    }

    #[target_feature(enable = "avx512f,avx512bw,vpclmulqdq,pclmulqdq,sse4.1")]
    pub(super) unsafe fn fold_two(
        source: &[Gf128],
        base: usize,
        out: &mut [Gf128],
        a: Gf128,
        b: Gf128,
    ) {
        unsafe {
            let a = _mm512_broadcast_i32x4(_mm_set_epi64x(a.hi as i64, a.lo as i64));
            let b = _mm512_broadcast_i32x4(_mm_set_epi64x(b.hi as i64, b.lo as i64));
            let mut i = 0;
            while i + 4 <= out.len() {
                let f = columns(source.as_ptr().add(4 * (base + i)));
                let lo = _mm512_xor_si512(f[0], ghash_mul_x4(a, _mm512_xor_si512(f[0], f[1])));
                let hi = _mm512_xor_si512(f[2], ghash_mul_x4(a, _mm512_xor_si512(f[2], f[3])));
                let value = _mm512_xor_si512(lo, ghash_mul_x4(b, _mm512_xor_si512(lo, hi)));
                _mm512_storeu_si512(out.as_mut_ptr().add(i).cast(), value);
                i += 4;
            }
            // Convert broadcasts only for the short scalar tail.
            let aa = _mm512_castsi512_si128(a);
            let bb = _mm512_castsi512_si128(b);
            scalar_fold_two(
                source,
                base + i,
                &mut out[i..],
                Gf128 {
                    lo: _mm_extract_epi64::<0>(aa) as u64,
                    hi: _mm_extract_epi64::<1>(aa) as u64,
                },
                Gf128 {
                    lo: _mm_extract_epi64::<0>(bb) as u64,
                    hi: _mm_extract_epi64::<1>(bb) as u64,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_lookahead_matches_scalar_with_offsets_and_tails() {
        let mut seed = 0x123456789abcdefu64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            Gf128 {
                lo: seed,
                hi: seed.rotate_left(29),
            }
        };
        let f: Vec<_> = (0..300).map(|_| next()).collect();
        let b: Vec<_> = (0..300).map(|_| next()).collect();
        for len in [4, 12, 16, 20, 128, 252] {
            let expected = scalar_accumulate(&f[1..len + 1], &b[3..len + 3]);
            let actual = accumulate(&f[1..len + 1], &b[3..len + 3]);
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert_eq!(actual, expected);
            }
        }
        for len in [1, 3, 4, 7, 32, 65] {
            let a = next();
            let b = next();
            let mut actual = vec![Gf128::ZERO; len];
            let mut expected = actual.clone();
            fold_two(&f, 2, &mut actual, a, b);
            scalar_fold_two(&f, 2, &mut expected, a, b);
            assert_eq!(actual, expected);
        }
    }
}
