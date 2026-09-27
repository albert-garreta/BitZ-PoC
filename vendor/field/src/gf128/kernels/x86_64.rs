// Copyright 2025 The Binius Developers; Irreducible, Inc.
// Modifications copyright 2026 Succinct Labs, Benedikt Bunz, William Wang
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Moved from local Flock at 6271724d.
use super::{Gf128, Gf128Product, ghash_reduce};
use core::arch::x86_64::*;

/// 64×64 carry-less product, returned as a 128-bit vector {lo, hi}.
///
/// # Safety
/// Caller must ensure `pclmulqdq` (and `sse4.1` for the lane extracts in
/// callers) is enabled — statically satisfied since every caller is itself
/// `#[target_feature(enable = "pclmulqdq,sse4.1")]`.
#[inline]
#[target_feature(enable = "pclmulqdq,sse4.1")]
unsafe fn pmull(a: u64, b: u64) -> __m128i {
    let va = _mm_set_epi64x(0, a as i64);
    let vb = _mm_set_epi64x(0, b as i64);
    // IMM8 = 0x00: low qword of a × low qword of b.
    _mm_clmulepi64_si128::<0x00>(va, vb)
}

#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn lane0(v: __m128i) -> u64 {
    _mm_extract_epi64::<0>(v) as u64
}

#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn lane1(v: __m128i) -> u64 {
    _mm_extract_epi64::<1>(v) as u64
}

/// Schoolbook 4 CLMUL — fully independent products, then scalar reduction.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_mul_schoolbook(a: Gf128, b: Gf128) -> Gf128 {
    // SAFETY: function carries the required target features.
    unsafe {
        let p_ll = pmull(a.lo, b.lo);
        let p_lh = pmull(a.lo, b.hi);
        let p_hl = pmull(a.hi, b.lo);
        let p_hh = pmull(a.hi, b.hi);

        let cross = _mm_xor_si128(p_lh, p_hl);
        let cr_lo = lane0(cross);
        let cr_hi = lane1(cross);

        ghash_reduce(
            lane0(p_ll),
            lane1(p_ll) ^ cr_lo,
            lane0(p_hh) ^ cr_hi,
            lane1(p_hh),
        )
    }
}

/// Binius-style: schoolbook 4 CLMUL + recursive 2-stage reduction (2 CLMUL).
/// Direct port of `aarch64::ghash_mul_binius`. `vextq_u64::<1>(zero, t)`
/// (= {0, t.lo}) becomes `_mm_slli_si128::<8>(t)` — an 8-byte left shift
/// that moves the low qword into the high lane and zeroes the low lane.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_mul_binius(a: Gf128, b: Gf128) -> Gf128 {
    // SAFETY: function carries the required target features.
    unsafe {
        let t0 = pmull(a.lo, b.lo);
        let t1a = pmull(a.lo, b.hi);
        let t1b = pmull(a.hi, b.lo);
        let t2 = pmull(a.hi, b.hi);
        let mut t1 = _mm_xor_si128(t1a, t1b);

        // First reduce: t1 = t1 + x^64 · t2 (mod p).
        let t2_shifted = _mm_slli_si128::<8>(t2); // {0, t2.lo}
        t1 = _mm_xor_si128(t1, t2_shifted);
        let t2_red = pmull(lane1(t2), 0x87);
        t1 = _mm_xor_si128(t1, t2_red);

        // Second reduce: t0 = t0 + x^64 · t1 (mod p).
        let t1_shifted = _mm_slli_si128::<8>(t1); // {0, t1.lo}
        let mut t0 = _mm_xor_si128(t0, t1_shifted);
        let t1_red = pmull(lane1(t1), 0x87);
        t0 = _mm_xor_si128(t0, t1_red);

        Gf128 {
            lo: lane0(t0),
            hi: lane1(t0),
        }
    }
}

/// Karatsuba 3 CLMUL — middle term depends on XOR of inputs. Port of
/// `aarch64::ghash_mul_karatsuba`.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_mul_karatsuba(a: Gf128, b: Gf128) -> Gf128 {
    // SAFETY: function carries the required target features.
    unsafe {
        let p0 = pmull(a.lo, b.lo);
        let p1 = pmull(a.hi, b.hi);
        let pm = pmull(a.lo ^ a.hi, b.lo ^ b.hi);

        let p0_lo = lane0(p0);
        let p0_hi = lane1(p0);
        let p1_lo = lane0(p1);
        let p1_hi = lane1(p1);
        let pm_lo = lane0(pm);
        let pm_hi = lane1(pm);

        let cross_lo = pm_lo ^ p0_lo ^ p1_lo;
        let cross_hi = pm_hi ^ p0_hi ^ p1_hi;

        ghash_reduce(p0_lo, p0_hi ^ cross_lo, p1_lo ^ cross_hi, p1_hi)
    }
}

/// Karatsuba 3 CLMUL + Barrett 2 CLMUL = 5 CLMUL total. Port of
/// `aarch64::ghash_mul_karatsuba_barrett`.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_mul_karatsuba_barrett(a: Gf128, b: Gf128) -> Gf128 {
    // SAFETY: function carries the required target features.
    unsafe {
        let d0 = pmull(a.lo, b.lo);
        let d2 = pmull(a.hi, b.hi);
        let dm = pmull(a.lo ^ a.hi, b.lo ^ b.hi);
        let d1 = _mm_xor_si128(_mm_xor_si128(dm, d0), d2);

        let d0_lo = lane0(d0);
        let d0_hi = lane1(d0);
        let d1_lo = lane0(d1);
        let d1_hi = lane1(d1);
        let d2_lo = lane0(d2);
        let d2_hi = lane1(d2);

        let lo_lo = d0_lo;
        let lo_hi = d0_hi ^ d1_lo;
        let hi_lo = d2_lo ^ d1_hi;
        let hi_hi = d2_hi;

        let r_hi = pmull(hi_hi, 0x87);
        let r_lo = pmull(hi_lo, 0x87);

        let r_lo_lo = lane0(r_lo);
        let r_lo_hi = lane1(r_lo);
        let r_hi_lo = lane0(r_hi);
        let r_hi_hi = lane1(r_hi);

        // hi_hi · 0x87 has degree ≤ 70, so r_hi_hi has at most 7 bits.
        let ov = r_hi_hi;
        let corr = ov ^ (ov << 1) ^ (ov << 2) ^ (ov << 7);

        Gf128 {
            lo: lo_lo ^ r_lo_lo ^ corr,
            hi: lo_hi ^ r_lo_hi ^ r_hi_lo,
        }
    }
}

/// 256-bit unreduced schoolbook product, for XOR-accumulation then one
/// deferred `reduce()`. Port of `aarch64::ghash_mul_unreduced_neon`.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_mul_unreduced_x86(a: Gf128, b: Gf128) -> Gf128Product {
    // SAFETY: function carries the required target features.
    unsafe {
        let p_ll = pmull(a.lo, b.lo);
        let p_lh = pmull(a.lo, b.hi);
        let p_hl = pmull(a.hi, b.lo);
        let p_hh = pmull(a.hi, b.hi);

        let cross = _mm_xor_si128(p_lh, p_hl);
        let cr_lo = lane0(cross);
        let cr_hi = lane1(cross);

        Gf128Product::from_polynomial_words([
            lane0(p_ll),
            lane1(p_ll) ^ cr_lo,
            lane0(p_hh) ^ cr_hi,
            lane1(p_hh),
        ])
    }
}

// -----------------------------------------------------------------------
// AVX-512 + VPCLMULQDQ: 4 independent GF(2^128) multiplies per instruction.
//
// Three-product Karatsuba + the two-stage `0x87` reduction, applied
// independently in each 128-bit lane of
// a `__m512i`. A `__m512i` holds 4 contiguous `Gf128` (lane i = {lo_i, hi_i});
// since `Gf128` is `repr(C, align(16))` little-endian, 4 elements load
// directly with `_mm512_loadu_si512` — no shuffles. The reduction is the
// same field element as the scalar `ghash_mul_binius`.
// -----------------------------------------------------------------------

/// Per-lane reduction-poly low word: each 128-bit lane = {lo: 0x87, hi: 0}.
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
unsafe fn ghash_poly_x4() -> __m512i {
    _mm512_set_epi64(0, 0x87, 0, 0x87, 0, 0x87, 0, 0x87)
}

/// Per-128-bit-lane reduce: returns `t0 + x^64 · t1` (mod p) in each lane.
/// Mirrors one stage of `ghash_mul_binius`'s recursive reduction:
/// `t0 ^= (t1 << 64)` then `t0 ^= t1.hi · 0x87` (clmul imm `0x01` = hi qword
/// of `t1` × lo qword of `poly`).
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
unsafe fn gf2_128_reduce_x4(mut t0: __m512i, t1: __m512i) -> __m512i {
    // SAFETY: caller carries avx512f+avx512bw+vpclmulqdq.
    unsafe {
        let poly = ghash_poly_x4();
        t0 = _mm512_xor_si512(t0, _mm512_bslli_epi128::<8>(t1));
        t0 = _mm512_xor_si512(t0, _mm512_clmulepi64_epi128::<0x01>(t1, poly));
        t0
    }
}

/// Per lane, return the exact low, cross, and high polynomial products.
/// The cross term is `x.lo*y.hi + x.hi*y.lo`, so both reduced and deferred
/// consumers retain their existing polynomial representation.
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
unsafe fn karatsuba_parts_x4(x: __m512i, y: __m512i) -> (__m512i, __m512i, __m512i) {
    let lo = _mm512_clmulepi64_epi128::<0x00>(x, y);
    let hi = _mm512_clmulepi64_epi128::<0x11>(x, y);
    // The shuffle swaps the two 64-bit words within each 128-bit lane.
    let x_sum = _mm512_xor_si512(x, _mm512_shuffle_epi32::<0x4e>(x));
    let y_sum = _mm512_xor_si512(y, _mm512_shuffle_epi32::<0x4e>(y));
    let mixed = _mm512_clmulepi64_epi128::<0x00>(x_sum, y_sum);
    let cross = _mm512_xor_si512(mixed, _mm512_xor_si512(lo, hi));
    (lo, cross, hi)
}

/// 4 independent GF(2^128) products. `x` and `y` each hold 4 contiguous
/// `Gf128`; the result holds the 4 reduced products. Field-identical to
/// applying `ghash_mul_binius` to each lane.
///
/// # Safety
/// Caller must ensure `avx512f` + `vpclmulqdq` are available (statically
/// satisfied by the cfg gate and target-feature attribute).
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
pub unsafe fn ghash_mul_x4(x: __m512i, y: __m512i) -> __m512i {
    // SAFETY: caller carries avx512f+avx512bw+vpclmulqdq.
    unsafe {
        let (lo, cross, hi) = karatsuba_parts_x4(x, y);
        gf2_128_reduce_x4(lo, gf2_128_reduce_x4(cross, hi))
    }
}

// -----------------------------------------------------------------------
// Deferred-reduction 4-lane accumulator (port of binius `WideGhashProduct`,
// 4 lanes wide). Widen each product with 3 CLMULs but DON'T reduce; XOR many
// into the accumulator; reduce once at the end. Per 128-bit lane the
// unreduced product is `lo + mid·x^64 + hi·x^128` with `lo = x.lo·y.lo`,
// `hi = x.hi·y.hi`, `mid = x.hi·y.lo ⊕ x.lo·y.hi` — the same limb split the
// scalar `mul_unreduced`/`Gf128Product` uses. `fold()` horizontally XORs
// the 4 lanes into one scalar `Gf128Product`; since `ghash_reduce` is
// F2-linear, fold-then-reduce equals XOR-of-per-lane-reduce, which equals
// the scalar `Σ mul_unreduced` then `reduce`.
// -----------------------------------------------------------------------

/// Load 4 contiguous `Gf128` (lane i = `p[i]`) into a `__m512i`.
///
/// # Safety
/// `p` must point to 4 readable `Gf128`; `avx512f` available (cfg-gated).
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f")]
pub unsafe fn f128x4_loadu(p: *const Gf128) -> __m512i {
    // SAFETY: caller guarantees 4 readable Gf128 at p.
    unsafe { _mm512_loadu_si512(p as *const __m512i) }
}

/// Pack 4 `Gf128` scalars into a `__m512i` (lane 0 = `a`, …, lane 3 = `d`).
///
/// # Safety
/// Requires `avx512f`, as guaranteed by the cfg gate.
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f")]
pub unsafe fn f128x4_set(a: Gf128, b: Gf128, c: Gf128, d: Gf128) -> __m512i {
    // Pure register assembly; avx512f cfg-gated.
    _mm512_set_epi64(
        d.hi as i64,
        d.lo as i64,
        c.hi as i64,
        c.lo as i64,
        b.hi as i64,
        b.lo as i64,
        a.hi as i64,
        a.lo as i64,
    )
}

/// XOR the four 128-bit lanes of `v` into a single `__m128i`.
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[inline]
#[target_feature(enable = "avx512f")]
unsafe fn xor4_lanes(v: __m512i) -> __m128i {
    // Register-only lane extracts + XOR; avx512f cfg-gated.
    let l0 = _mm512_extracti32x4_epi32::<0>(v);
    let l1 = _mm512_extracti32x4_epi32::<1>(v);
    let l2 = _mm512_extracti32x4_epi32::<2>(v);
    let l3 = _mm512_extracti32x4_epi32::<3>(v);
    _mm_xor_si128(_mm_xor_si128(l0, l1), _mm_xor_si128(l2, l3))
}

/// 4-lane unreduced GF(2^128) product accumulator (deferred reduction).
#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
#[derive(Clone, Copy)]
pub struct WideGhashX4 {
    lo: __m512i,
    hi: __m512i,
    mid: __m512i,
}

#[cfg(all(
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
impl WideGhashX4 {
    /// Empty accumulator.
    ///
    /// # Safety
    /// `avx512f` available (cfg-gated).
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn zero() -> Self {
        let z = _mm512_setzero_si512();
        Self {
            lo: z,
            hi: z,
            mid: z,
        }
    }

    /// XOR-accumulate the 4 unreduced products `x[i]·y[i]` into self.
    ///
    /// # Safety
    /// `avx512f` + `vpclmulqdq` available (cfg-gated).
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
    pub unsafe fn mul_acc(&mut self, x: __m512i, y: __m512i) {
        // SAFETY: this function carries every feature the helper requires.
        unsafe {
            let (lo, cross, hi) = karatsuba_parts_x4(x, y);
            self.lo = _mm512_xor_si512(self.lo, lo);
            self.hi = _mm512_xor_si512(self.hi, hi);
            self.mid = _mm512_xor_si512(self.mid, cross);
        }
    }

    /// Reduce each accumulator lane separately, preserving all four sums.
    ///
    /// # Safety
    /// Requires `avx512f`, `avx512bw`, and `vpclmulqdq`.
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,vpclmulqdq")]
    pub unsafe fn reduce_lanes(self) -> __m512i {
        // SAFETY: this function carries every feature the reduction uses.
        unsafe { gf2_128_reduce_x4(self.lo, gf2_128_reduce_x4(self.mid, self.hi)) }
    }

    /// Horizontally XOR the 4 lanes and assemble a scalar `Gf128Product`
    /// (NOT yet reduced, so it can be XORed with a scalar tail accumulator).
    ///
    /// # Safety
    /// `avx512f` + `sse4.1` available (cfg-gated + attr).
    #[inline]
    #[target_feature(enable = "avx512f,sse4.1")]
    pub unsafe fn fold(self) -> Gf128Product {
        // SAFETY: caller carries avx512f+sse4.1.
        unsafe {
            let lo = xor4_lanes(self.lo);
            let hi = xor4_lanes(self.hi);
            let mid = xor4_lanes(self.mid);
            let lo_lo = _mm_extract_epi64::<0>(lo) as u64;
            let lo_hi = _mm_extract_epi64::<1>(lo) as u64;
            let hi_lo = _mm_extract_epi64::<0>(hi) as u64;
            let hi_hi = _mm_extract_epi64::<1>(hi) as u64;
            let mid_lo = _mm_extract_epi64::<0>(mid) as u64;
            let mid_hi = _mm_extract_epi64::<1>(mid) as u64;
            Gf128Product::from_polynomial_words([lo_lo, lo_hi ^ mid_lo, hi_lo ^ mid_hi, hi_hi])
        }
    }
}

/// Dedicated square: carry-less squaring has no cross term (`(a+b)^2 = a^2 + b^2`
/// over GF(2)), so only the two diagonal CLMULs are needed — half the CLMUL of
/// a general multiply.
///
/// # Safety
/// Requires `pclmulqdq` and `sse4.1`, as declared by the target-feature
/// attribute.
#[target_feature(enable = "pclmulqdq,sse4.1")]
pub unsafe fn ghash_square_x86(a: Gf128) -> Gf128 {
    // SAFETY: function carries the required target features.
    unsafe {
        let lo2 = pmull(a.lo, a.lo);
        let hi2 = pmull(a.hi, a.hi);
        ghash_reduce(lane0(lo2), lane1(lo2), lane0(hi2), lane1(hi2))
    }
}

#[cfg(all(
    test,
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "vpclmulqdq"
))]
mod tests {
    use super::*;
    use crate::gf128::kernels::software;

    fn lanes(value: __m512i) -> [Gf128; 4] {
        let mut out = [Gf128::ZERO; 4];
        // SAFETY: the module has the complete ISA and out holds four lanes.
        unsafe { _mm512_storeu_si512(out.as_mut_ptr().cast(), value) };
        out
    }

    fn packed(values: &[Gf128; 4]) -> __m512i {
        // SAFETY: the module has the complete ISA and values holds four lanes.
        unsafe { f128x4_loadu(values.as_ptr()) }
    }

    fn monomial(bit: usize) -> Gf128 {
        if bit < 64 {
            Gf128::new(1u64 << bit, 0)
        } else {
            Gf128::new(0, 1u64 << (bit - 64))
        }
    }

    fn random(state: &mut u64) -> Gf128 {
        let mut next = || {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        };
        Gf128::new(next(), next())
    }

    #[test]
    fn four_lane_products_match_every_monomial_pair() {
        // Every pair of polynomial basis vectors tests all cross-word and
        // reduction boundaries. Adjacent lanes have different right operands.
        for left in 0..128 {
            for right in (0..128).step_by(4) {
                let x = [monomial(left); 4];
                let y = std::array::from_fn(|lane| monomial(right + lane));
                // SAFETY: the native test gate covers every called intrinsic.
                let got = lanes(unsafe { ghash_mul_x4(packed(&x), packed(&y)) });
                for lane in 0..4 {
                    assert_eq!(
                        got[lane],
                        software::ghash_mul(x[lane], y[lane]),
                        "monomials {left}, {}",
                        right + lane,
                    );
                }
            }
        }
    }

    #[test]
    fn four_lane_products_match_scalar_with_offsets_and_boundary_words() {
        let edges = [
            Gf128::ZERO,
            Gf128::ONE,
            Gf128::new(0x87, 0),
            Gf128::new(1 << 63, 0),
            Gf128::new(0, 1),
            Gf128::new(0, 1 << 63),
            Gf128::new(u64::MAX, u64::MAX),
            Gf128::new(0x5555_5555_5555_5555, 0xAAAA_AAAA_AAAA_AAAA),
        ];
        let mut state = 0xAA43_C71D_291F_F091;
        for trial in 0..128 {
            let x: [Gf128; 11] = std::array::from_fn(|i| {
                if trial < edges.len() {
                    edges[(trial + i) % edges.len()]
                } else {
                    random(&mut state)
                }
            });
            let y: [Gf128; 11] = std::array::from_fn(|i| {
                if trial < edges.len() {
                    edges[(trial * 3 + i) % edges.len()]
                } else {
                    random(&mut state)
                }
            });
            for offset in 0..8 {
                // SAFETY: offset+4<=11 and the native ISA is enabled.
                let got = lanes(unsafe {
                    ghash_mul_x4(
                        f128x4_loadu(x.as_ptr().add(offset)),
                        f128x4_loadu(y.as_ptr().add(offset)),
                    )
                });
                for lane in 0..4 {
                    let (a, b) = (x[offset + lane], y[offset + lane]);
                    // The scalar schoolbook implementation is independent of
                    // the new Karatsuba helper and keeps its original schedule.
                    let scalar = unsafe { ghash_mul_schoolbook(a, b) };
                    assert_eq!(
                        got[lane], scalar,
                        "trial {trial}, offset {offset}, lane {lane}"
                    );
                }
            }
        }
    }

    #[test]
    fn wide_lanes_preserve_exact_parts_accumulation_and_cancellation() {
        let mut state = 0x70F1_B55A_9850_1003;
        // SAFETY: the native test gate covers every accumulator instruction.
        let mut acc = unsafe { WideGhashX4::zero() };
        let mut lo = [Gf128::ZERO; 4];
        let mut hi = [Gf128::ZERO; 4];
        let mut cross = [Gf128::ZERO; 4];
        let mut expected = [Gf128Product::zero(); 4];
        let mut inputs = Vec::new();
        for step in 0..129 {
            let x = std::array::from_fn(|_| random(&mut state));
            let y = std::array::from_fn(|lane| {
                if step % 3 == 0 {
                    x[lane]
                } else {
                    random(&mut state)
                }
            });
            inputs.push((x, y));
            unsafe { acc.mul_acc(packed(&x), packed(&y)) };
            for lane in 0..4 {
                let p0 = software::clmul_64x64(x[lane].lo, y[lane].lo);
                let p2 = software::clmul_64x64(x[lane].hi, y[lane].hi);
                let p01 = software::clmul_64x64(x[lane].lo, y[lane].hi);
                let p10 = software::clmul_64x64(x[lane].hi, y[lane].lo);
                lo[lane] += Gf128::new(p0[0], p0[1]);
                hi[lane] += Gf128::new(p2[0], p2[1]);
                cross[lane] += Gf128::new(p01[0] ^ p10[0], p01[1] ^ p10[1]);
                expected[lane] ^= unsafe { ghash_mul_unreduced_x86(x[lane], y[lane]) };
            }
            assert_eq!(lanes(acc.lo), lo, "low parts after {step}");
            assert_eq!(lanes(acc.hi), hi, "high parts after {step}");
            assert_eq!(lanes(acc.mid), cross, "cross parts after {step}");
            assert_eq!(
                lanes(unsafe { acc.reduce_lanes() }),
                expected.map(Gf128Product::reduce),
                "lane reductions after {step}",
            );
            let folded = expected
                .iter()
                .copied()
                .fold(Gf128Product::zero(), |a, b| a ^ b);
            assert_eq!(
                unsafe { acc.fold() },
                folded,
                "exact folded polynomial after {step}"
            );
        }
        // Replaying every product cancels the unreduced accumulators too,
        // rather than merely producing a polynomial whose remainder is zero.
        for (x, y) in inputs.into_iter().rev() {
            unsafe { acc.mul_acc(packed(&x), packed(&y)) };
        }
        assert_eq!(lanes(acc.lo), [Gf128::ZERO; 4]);
        assert_eq!(lanes(acc.hi), [Gf128::ZERO; 4]);
        assert_eq!(lanes(acc.mid), [Gf128::ZERO; 4]);
        assert_eq!(unsafe { acc.fold() }, Gf128Product::zero());
        assert_eq!(lanes(unsafe { acc.reduce_lanes() }), [Gf128::ZERO; 4]);
    }
}
