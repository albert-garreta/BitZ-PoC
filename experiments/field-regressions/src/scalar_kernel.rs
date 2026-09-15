// Adapted from vendored Flock (Apache-2.0 OR MIT).
// Copyright 2025 The Binius Developers; Irreducible, Inc.
// Modifications copyright 2026 Succinct Labs, Benedikt Bunz, William Wang.
/// ARM retains the six-PMULL Binius graph; x86 uses five-CLMUL
/// Karatsuba/Barrett. Both return the shared crate representation.
#[inline(always)]
pub fn mul(a: field::Gf128, b: field::Gf128) -> field::Gf128 {
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    unsafe {
        use core::arch::aarch64::*;
        #[inline(always)]
        unsafe fn pmull(a: u64, b: u64) -> uint64x2_t {
            unsafe { vreinterpretq_u64_p128(vmull_p64(a, b)) }
        }
        let zero = vdupq_n_u64(0);
        let t0 = pmull(a.lo, b.lo);
        let t1a = pmull(a.lo, b.hi);
        let t1b = pmull(a.hi, b.lo);
        let t2 = pmull(a.hi, b.hi);
        let mut t1 = veorq_u64(t1a, t1b);
        t1 = veorq_u64(t1, vextq_u64::<1>(zero, t2));
        t1 = veorq_u64(t1, pmull(vgetq_lane_u64::<1>(t2), 0x87));
        let out = veorq_u64(t0, vextq_u64::<1>(zero, t1));
        let out = veorq_u64(out, pmull(vgetq_lane_u64::<1>(t1), 0x87));
        field::Gf128::new(vgetq_lane_u64::<0>(out), vgetq_lane_u64::<1>(out))
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
    {
        // Karatsuba with CLMUL Barrett reduction. Keep the high-half folds
        // independent instead of serializing the long scalar shift/XOR graph.
        // SAFETY: PCLMUL is enabled by the compile-time gate.
        unsafe {
            use core::arch::x86_64::*;
            #[inline(always)]
            unsafe fn clmul(a: u64, b: u64) -> __m128i {
                unsafe {
                    _mm_clmulepi64_si128::<0>(
                        _mm_set_epi64x(0, a as i64),
                        _mm_set_epi64x(0, b as i64),
                    )
                }
            }
            let d0 = clmul(a.lo, b.lo);
            let d2 = clmul(a.hi, b.hi);
            let d1 = _mm_xor_si128(_mm_xor_si128(clmul(a.lo ^ a.hi, b.lo ^ b.hi), d0), d2);
            let [p0, p1]: [u64; 2] = core::mem::transmute(d0);
            let [c0, c1]: [u64; 2] = core::mem::transmute(d1);
            let [p2, p3]: [u64; 2] = core::mem::transmute(d2);
            let [r0, r1]: [u64; 2] = core::mem::transmute(clmul(p2 ^ c1, 0x87));
            let [r2, ov]: [u64; 2] = core::mem::transmute(clmul(p3, 0x87));
            field::Gf128::new(
                p0 ^ r0 ^ ov ^ (ov << 1) ^ (ov << 2) ^ (ov << 7),
                p1 ^ c0 ^ r1 ^ r2,
            )
        }
    }
    #[cfg(not(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq")
    )))]
    {
        a * b
    }
}
