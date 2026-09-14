// Adapted from vendored Flock (Apache-2.0 OR MIT).
// Copyright 2025 The Binius Developers; Irreducible, Inc.
// Modifications copyright 2026 Succinct Labs, Benedikt Bunz, William Wang.
/// Same six-PMULL arithmetic as both libraries. Adapted from vendored Flock's
/// ghash_mul_binius: feed scalar lanes directly, preserving its product graph,
/// and return the shared crate's representation. Experiment only.
#[inline(always)]
pub fn mul(a: field::F128, b: field::F128) -> field::F128 {
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
        field::F128::new(vgetq_lane_u64::<0>(out), vgetq_lane_u64::<1>(out))
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
    {
        a * b
    }
}
