//! Product-sumcheck kernels over top-variable splits. Message products accumulate
//! with XOR, then each sum is reduced once per kernel call. XOR accumulation has
//! no integer carry-overflow bound and preserves the transcript.

use crate::field::{Gf128, Gf128Product};

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "pclmulqdq",
    target_feature = "sse4.1",
    target_feature = "vpclmulqdq"
))]
use {
    crate::field::gf128_kernels::x86_64::{f128x4_loadu, f128x4_set, ghash_mul_x4, WideGhashX4},
    core::arch::x86_64::{__m512i, _mm512_storeu_si512, _mm512_xor_si512},
};

/// `(sum c_hi*z_hi, sum (c_lo+c_hi)*(z_lo+z_hi))`.
pub(super) fn message(c: [&[Gf128]; 2], z: [&[Gf128]; 2]) -> (Gf128, Gf128) {
    let n = c[0].len();
    assert!(c[1].len() == n && z[0].len() == n && z[1].len() == n);
    let mut e1 = Gf128Product::zero();
    let mut einf = Gf128Product::zero();
    #[allow(unused_mut)]
    let mut i = 0;
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: the cfg supplies every required instruction; all four slices
    // have length n, and every vector load is bounded by i + 4 <= n.
    unsafe {
        let mut one = WideGhashX4::zero();
        let mut inf = WideGhashX4::zero();
        while i + 4 <= n {
            let c0 = f128x4_loadu(c[0].as_ptr().add(i));
            let c1 = f128x4_loadu(c[1].as_ptr().add(i));
            let z0 = f128x4_loadu(z[0].as_ptr().add(i));
            let z1 = f128x4_loadu(z[1].as_ptr().add(i));
            one.mul_acc(c1, z1);
            inf.mul_acc(_mm512_xor_si512(c0, c1), _mm512_xor_si512(z0, z1));
            i += 4;
        }
        e1 ^= one.fold();
        einf ^= inf.fold();
    }
    for i in i..n {
        e1 ^= c[1][i].mul_unreduced(z[1][i]);
        einf ^= (c[0][i] + c[1][i]).mul_unreduced(z[0][i] + z[1][i]);
    }
    (e1.reduce(), einf.reduce())
}

/// Fold the two halves of each table and accumulate the following message.
/// The two entries of each argument select the next round's low/high halves.
pub(super) fn fold_and_message(
    c: [&mut [Gf128]; 2],
    c_high: [&[Gf128]; 2],
    z: [&mut [Gf128]; 2],
    z_high: [&[Gf128]; 2],
    r: Gf128,
) -> (Gf128, Gf128) {
    let n = c[0].len();
    assert!(c[1].len() == n && z[0].len() == n && z[1].len() == n);
    assert!(c_high.iter().chain(&z_high).all(|v| v.len() == n));
    let mut e1 = Gf128Product::zero();
    let mut einf = Gf128Product::zero();
    #[allow(unused_mut)]
    let mut i = 0;
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: all slices have length n and i + 4 <= n bounds every vector
    // load/store. Mutable quarters are disjoint by their Rust references.
    unsafe {
        let rv = f128x4_set(r, r, r, r);
        let mut one = WideGhashX4::zero();
        let mut inf = WideGhashX4::zero();
        while i + 4 <= n {
            let mut values = [_mm512_xor_si512(rv, rv); 4];
            for (slot, (low, high)) in c.iter().zip(c_high).chain(z.iter().zip(z_high)).enumerate()
            {
                let lo = f128x4_loadu(low.as_ptr().add(i));
                let hi = f128x4_loadu(high.as_ptr().add(i));
                values[slot] = _mm512_xor_si512(lo, ghash_mul_x4(rv, _mm512_xor_si512(lo, hi)));
            }
            let [c0, c1, z0, z1] = values;
            _mm512_storeu_si512(c[0].as_mut_ptr().add(i).cast::<__m512i>(), c0);
            _mm512_storeu_si512(c[1].as_mut_ptr().add(i).cast::<__m512i>(), c1);
            _mm512_storeu_si512(z[0].as_mut_ptr().add(i).cast::<__m512i>(), z0);
            _mm512_storeu_si512(z[1].as_mut_ptr().add(i).cast::<__m512i>(), z1);
            one.mul_acc(c1, z1);
            inf.mul_acc(_mm512_xor_si512(c0, c1), _mm512_xor_si512(z0, z1));
            i += 4;
        }
        e1 ^= one.fold();
        einf ^= inf.fold();
    }
    for i in i..n {
        let c0 = c[0][i] + r * (c_high[0][i] + c[0][i]);
        let c1 = c[1][i] + r * (c_high[1][i] + c[1][i]);
        let z0 = z[0][i] + r * (z_high[0][i] + z[0][i]);
        let z1 = z[1][i] + r * (z_high[1][i] + z[1][i]);
        c[0][i] = c0;
        c[1][i] = c1;
        z[0][i] = z0;
        z[1][i] = z1;
        e1 ^= c1.mul_unreduced(z1);
        einf ^= (c0 + c1).mul_unreduced(z0 + z1);
    }
    (e1.reduce(), einf.reduce())
}
