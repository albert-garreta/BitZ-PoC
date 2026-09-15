//! x86 round coefficients with products and accumulators kept in SIMD registers.
use super::{Gf128, Gf128Product, Gf, gf, hw};
use core::arch::x86_64::*;
use f2z::utils::wide_mul::WideMulAcc;

type Coeff = (Gf, Gf, Gf);
const _: () = assert!(core::mem::size_of::<Gf>() == 16);

#[derive(Clone, Copy)]
struct Wide {
    lo: __m128i,
    hi: __m128i,
    mid: __m128i,
}

impl Wide {
    #[inline(always)]
    unsafe fn zero() -> Self {
        unsafe {
            let z = _mm_setzero_si128();
            Self {
                lo: z,
                hi: z,
                mid: z,
            }
        }
    }

    #[inline(always)]
    unsafe fn acc(&mut self, a: __m128i, b: __m128i) {
        unsafe {
            let lo = _mm_clmulepi64_si128::<0>(a, b);
            let hi = _mm_clmulepi64_si128::<0x11>(a, b);
            let ax = _mm_xor_si128(a, _mm_shuffle_epi32::<0x4e>(a));
            let bx = _mm_xor_si128(b, _mm_shuffle_epi32::<0x4e>(b));
            let mid = _mm_xor_si128(_mm_clmulepi64_si128::<0>(ax, bx), _mm_xor_si128(lo, hi));
            self.lo = _mm_xor_si128(self.lo, lo);
            self.hi = _mm_xor_si128(self.hi, hi);
            self.mid = _mm_xor_si128(self.mid, mid);
        }
    }

    #[inline(always)]
    unsafe fn limbs(self) -> Gf128Product {
        unsafe {
            let lo = _mm_xor_si128(self.lo, _mm_slli_si128::<8>(self.mid));
            let hi = _mm_xor_si128(self.hi, _mm_srli_si128::<8>(self.mid));
            let [r0, r1]: [u64; 2] = core::mem::transmute(lo);
            let [r2, r3]: [u64; 2] = core::mem::transmute(hi);
            Gf128Product::from_polynomial_words([r0, r1, r2, r3])
        }
    }
}

#[inline(always)]
unsafe fn mul(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        let mut wide = Wide::zero();
        wide.acc(a, b);
        let lo = _mm_xor_si128(wide.lo, _mm_slli_si128::<8>(wide.mid));
        let hi = _mm_xor_si128(wide.hi, _mm_srli_si128::<8>(wide.mid));
        let poly = _mm_set_epi64x(0, 0x87);
        let rh = _mm_clmulepi64_si128::<1>(hi, poly);
        let rl = _mm_clmulepi64_si128::<0>(hi, poly);
        let ov = _mm_srli_si128::<8>(rh);
        let corr = _mm_xor_si128(
            _mm_xor_si128(ov, _mm_slli_epi64::<1>(ov)),
            _mm_xor_si128(_mm_slli_epi64::<2>(ov), _mm_slli_epi64::<7>(ov)),
        );
        _mm_xor_si128(
            _mm_xor_si128(lo, rl),
            _mm_xor_si128(_mm_slli_si128::<8>(rh), corr),
        )
    }
}

#[inline(always)]
unsafe fn slot(acc: &mut [Wide; 3], w: __m128i, l: &[Gf], r: &[Gf]) {
    unsafe {
        let l0 = mul(w, _mm_loadu_si128(l.as_ptr().cast()));
        let l1 = mul(w, _mm_loadu_si128(l.as_ptr().add(1).cast()));
        let r0 = _mm_loadu_si128(r.as_ptr().cast());
        let r1 = _mm_loadu_si128(r.as_ptr().add(1).cast());
        acc[0].acc(l0, r0);
        acc[1].acc(l1, r1);
        acc[2].acc(_mm_xor_si128(l0, l1), _mm_xor_si128(r0, r1));
    }
}

#[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
#[inline(always)]
unsafe fn pair4(values: &[Gf]) -> (__m512i, __m512i) {
    unsafe {
        let a = _mm512_loadu_si512(values.as_ptr().cast());
        let b = _mm512_loadu_si512(values.as_ptr().add(4).cast());
        let even = _mm512_setr_epi64(0, 1, 4, 5, 8, 9, 12, 13);
        let odd = _mm512_setr_epi64(2, 3, 6, 7, 10, 11, 14, 15);
        (
            _mm512_permutex2var_epi64(a, even, b),
            _mm512_permutex2var_epi64(a, odd, b),
        )
    }
}

#[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
#[inline(always)]
unsafe fn slot4(acc: &mut [hw::WideGhashX4; 3], w: __m512i, l: &[Gf], r: &[Gf]) {
    unsafe {
        let (l0, l1) = pair4(l);
        let (r0, r1) = pair4(r);
        let l0 = hw::ghash_mul_x4(w, l0);
        let l1 = hw::ghash_mul_x4(w, l1);
        acc[0].mul_acc(l0, r0);
        acc[1].mul_acc(l1, r1);
        acc[2].mul_acc(_mm512_xor_si512(l0, l1), _mm512_xor_si512(r0, r1));
    }
}

/// The public length and compile-time ISA determine the schedule. Two pairs
/// share one traversal and one final reduction of each coefficient.
#[inline(always)]
pub(super) fn round<const TWO: bool>(l: &[Gf], r: &[Gf], w: &[Gf]) -> Coeff {
    let n = w.len();
    if n < 4 {
        if TWO {
            return Gf::eqf_two_pair_round(
                &l[..2 * n],
                &r[..2 * n],
                &l[2 * n..],
                &r[2 * n..],
                w,
                n,
            )
            .unwrap();
        }
        return super::consumers::round::<3>(l, r, w);
    }
    packed_round::<TWO>(l, r, w)
}

#[inline(never)]
fn packed_round<const TWO: bool>(l: &[Gf], r: &[Gf], w: &[Gf]) -> Coeff {
    let n = w.len();
    assert!(l.len() >= n * if TWO { 4 } else { 2 });
    assert!(r.len() >= n * if TWO { 4 } else { 2 });
    // SAFETY: this module is compiled only with PCLMUL/SSE4.1. Gf is a
    // transparent pair of u64 limbs; every vector load stays inside the
    // asserted input slices. Packed loads run only for complete four-slot blocks.
    unsafe {
        let mut result = [Gf128Product::zero(); 3];
        let mut done = 0;
        #[cfg(all(target_feature = "avx512f", target_feature = "vpclmulqdq"))]
        if n >= 4 {
            let mut acc = [hw::WideGhashX4::zero(); 3];
            while done + 4 <= n {
                let weight = _mm512_loadu_si512(w.as_ptr().add(done).cast());
                slot4(&mut acc, weight, &l[done * 2..], &r[done * 2..]);
                if TWO {
                    slot4(
                        &mut acc,
                        weight,
                        &l[2 * n + done * 2..],
                        &r[2 * n + done * 2..],
                    );
                }
                done += 4;
            }
            for i in 0..3 {
                result[i] ^= acc[i].fold();
            }
        }
        let mut acc = [Wide::zero(); 3];
        while done < n {
            let weight = _mm_loadu_si128(w.as_ptr().add(done).cast());
            slot(&mut acc, weight, &l[done * 2..], &r[done * 2..]);
            if TWO {
                slot(
                    &mut acc,
                    weight,
                    &l[2 * n + done * 2..],
                    &r[2 * n + done * 2..],
                );
            }
            done += 1;
        }
        for i in 0..3 {
            result[i] ^= acc[i].limbs();
        }
        let c0: Gf128 = result[0].reduce();
        let c2: Gf128 = result[2].reduce();
        (gf(c0), gf(result[1].reduce() + c0 + c2), gf(c2))
    }
}
