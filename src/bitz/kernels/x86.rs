//! Four-lane GHASH kernels. The parent module gates every instruction used
//! here, including the scalar tail's PCLMUL and the horizontal fold's SSE4.1.
//! Only field operations are regrouped; no proof messages or domains change.

use core::arch::x86_64::*;
use std::mem::MaybeUninit;

use super::{Gf, col_of, generic};
use field::gf128::kernels::x86_64::{WideGhashX4, f128x4_loadu, f128x4_set, ghash_mul_x4};

// Irregular bucket writes remain scalar. They already defer reduction and
// retain the exact same representation as the generic reference.
pub(super) use generic::{
    SumBuckets, jit_bucket_finish, jit_bucket_group, scatter_add, scatter_add4,
};

pub(super) struct Sums {
    end: WideGhashX4,
    inf: WideGhashX4,
    tail: [Gf; 2],
}

impl Sums {
    pub(super) fn zero() -> Self {
        // SAFETY: the whole module is compiled only with the complete ISA.
        unsafe {
            Self {
                end: WideGhashX4::zero(),
                inf: WideGhashX4::zero(),
                tail: [Gf::zero(); 2],
            }
        }
    }

    pub(super) fn finish(self) -> (Gf, Gf) {
        // SAFETY: the module gate includes AVX512F and SSE4.1 for fold().
        unsafe {
            (
                self.end.fold().reduce() + self.tail[0],
                self.inf.fold().reduce() + self.tail[1],
            )
        }
    }

    #[inline]
    fn scalar(&mut self, w: Gf, l0: Gf, l1: Gf, r0: Gf, r1: Gf, send_one: bool) {
        let (le, re) = if send_one { (l1, r1) } else { (l0, r0) };
        self.tail[0] += w * le * re;
        self.tail[1] += w * (l0 + l1) * (r0 + r1);
    }

    #[inline]
    fn scalar_weighted(&mut self, l0: Gf, l1: Gf, r0: Gf, r1: Gf, send_one: bool) {
        let (le, re) = if send_one { (l1, r1) } else { (l0, r0) };
        self.tail[0] += le * re;
        self.tail[1] += (l0 + l1) * (r0 + r1);
    }

    #[inline]
    unsafe fn four(
        &mut self,
        w: __m512i,
        l0: __m512i,
        l1: __m512i,
        r0: __m512i,
        r1: __m512i,
        send_one: bool,
    ) {
        // SAFETY: registers hold four independent polynomial-basis elements.
        unsafe {
            let (le, re) = if send_one { (l1, r1) } else { (l0, r0) };
            self.end.mul_acc(ghash_mul_x4(w, le), re);
            self.inf.mul_acc(
                ghash_mul_x4(w, _mm512_xor_si512(l0, l1)),
                _mm512_xor_si512(r0, r1),
            );
        }
    }

    #[inline]
    unsafe fn four_weighted(
        &mut self,
        l0: __m512i,
        l1: __m512i,
        r0: __m512i,
        r1: __m512i,
        send_one: bool,
    ) {
        // SAFETY: the left pair already contains the per-column weight.
        unsafe {
            let (le, re) = if send_one { (l1, r1) } else { (l0, r0) };
            self.end.mul_acc(le, re);
            self.inf
                .mul_acc(_mm512_xor_si512(l0, l1), _mm512_xor_si512(r0, r1));
        }
    }
}

#[inline]
unsafe fn broadcast(value: Gf) -> __m512i {
    unsafe { f128x4_set(value, value, value, value) }
}

#[inline]
unsafe fn fold(rho: __m512i, lo: __m512i, hi: __m512i) -> __m512i {
    unsafe { _mm512_xor_si512(lo, ghash_mul_x4(rho, _mm512_xor_si512(lo, hi))) }
}

#[inline]
unsafe fn load(values: &[Gf], first: usize) -> __m512i {
    // SAFETY: callers bound first+4 by values.len() before entering the loop.
    unsafe { f128x4_loadu(values.as_ptr().add(first)) }
}

#[inline]
unsafe fn store(values: &mut [Gf], first: usize, value: __m512i) {
    // SAFETY: callers bound first+4 by values.len(); unaligned stores are used.
    unsafe { _mm512_storeu_si512(values.as_mut_ptr().add(first).cast(), value) }
}

#[inline]
unsafe fn write(values: &mut [MaybeUninit<Gf>], first: usize, value: __m512i) {
    // SAFETY: the store initializes exactly four slots within the slice.
    unsafe { _mm512_storeu_si512(values.as_mut_ptr().add(first).cast(), value) }
}

/// Gather table entries using transposed pattern indices. Slice indexing is
/// checked even in the SIMD path; malformed table shapes cannot cause UB.
#[inline]
unsafe fn lookup(table: &[Gf], patterns: &[u8; 64], positions: [usize; 4]) -> __m512i {
    let [a, b, c, d] = positions.map(|i| table[patterns[i] as usize]);
    unsafe { f128x4_set(a, b, c, d) }
}

pub(super) fn round_sums_task(
    lo_l: &[Gf],
    hi_l: &[Gf],
    lo_r: &[Gf],
    hi_r: &[Gf],
    w: &[Gf],
    send_one: bool,
) -> (Gf, Gf) {
    let n = w.len();
    assert!([lo_l, hi_l, lo_r, hi_r].iter().all(|v| v.len() >= n));
    if n < 4 {
        return generic::round_sums_task(lo_l, hi_l, lo_r, hi_r, w, send_one);
    }
    let mut sums = Sums::zero();
    let end = n / 4 * 4;
    for i in (0..end).step_by(4) {
        // SAFETY: all five slices have at least i+4 elements and ISA is gated.
        unsafe {
            sums.four(
                load(w, i),
                load(lo_l, i),
                load(hi_l, i),
                load(lo_r, i),
                load(hi_r, i),
                send_one,
            );
        }
    }
    for i in end..n {
        sums.scalar(w[i], lo_l[i], hi_l[i], lo_r[i], hi_r[i], send_one);
    }
    sums.finish()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fused_task<const LEFT: u8>(
    q0_l: &mut [Gf],
    q1_l: &mut [Gf],
    q2_l: &[Gf],
    q3_l: &[Gf],
    q0_r: &mut [Gf],
    q1_r: &mut [Gf],
    q2_r: &[Gf],
    q3_r: &[Gf],
    rho: &Gf,
    w: &[Gf],
    send_one: bool,
) -> (Gf, Gf) {
    let n = w.len();
    assert!(
        [&*q0_l, &*q1_l, q2_l, q3_l, &*q0_r, &*q1_r, q2_r, q3_r]
            .iter()
            .all(|v| v.len() >= n)
    );
    if n < 4 {
        return generic::fused_task::<LEFT>(
            q0_l, q1_l, q2_l, q3_l, q0_r, q1_r, q2_r, q3_r, rho, w, send_one,
        );
    }
    let mut sums = Sums::zero();
    let end = n / 4 * 4;
    // SAFETY: the module gate covers the vector instructions and all loops
    // stop before the final incomplete vector. Quarters are disjoint slices.
    unsafe {
        let scalar = broadcast(*rho);
        for i in (0..end).step_by(4) {
            let mut l0 = fold(scalar, load(q0_l, i), load(q2_l, i));
            let mut l1 = fold(scalar, load(q1_l, i), load(q3_l, i));
            let r0 = fold(scalar, load(q0_r, i), load(q2_r, i));
            let r1 = fold(scalar, load(q1_r, i), load(q3_r, i));
            if LEFT == super::WEIGH {
                let weight = load(w, i);
                l0 = ghash_mul_x4(weight, l0);
                l1 = ghash_mul_x4(weight, l1);
            }
            store(q0_l, i, l0);
            store(q1_l, i, l1);
            store(q0_r, i, r0);
            store(q1_r, i, r1);
            if LEFT == super::PLAIN {
                sums.four(load(w, i), l0, l1, r0, r1, send_one);
            } else {
                sums.four_weighted(l0, l1, r0, r1, send_one);
            }
        }
    }
    for i in end..n {
        q0_l[i] = q0_l[i] + *rho * (q0_l[i] + q2_l[i]);
        q1_l[i] = q1_l[i] + *rho * (q1_l[i] + q3_l[i]);
        q0_r[i] = q0_r[i] + *rho * (q0_r[i] + q2_r[i]);
        q1_r[i] = q1_r[i] + *rho * (q1_r[i] + q3_r[i]);
        if LEFT == super::WEIGH {
            q0_l[i] *= w[i];
            q1_l[i] *= w[i];
        }
        if LEFT == super::PLAIN {
            sums.scalar(w[i], q0_l[i], q1_l[i], q0_r[i], q1_r[i], send_one);
        } else {
            sums.scalar_weighted(q0_l[i], q1_l[i], q0_r[i], q1_r[i], send_one);
        }
    }
    sums.finish()
}

pub(super) fn jit_sums_group(
    tab: [&[Gf]; 4],
    pat: &[[u8; 64]; 4],
    eq_t: &[Gf],
    send_one: bool,
    sums: &mut Sums,
) {
    assert_eq!(eq_t.len(), 64);
    for i in (0..64).step_by(4) {
        let positions = [i, i + 1, i + 2, i + 3];
        // SAFETY: a complete group has 64 weights; table lookups are checked.
        unsafe {
            let values: [__m512i; 4] = std::array::from_fn(|j| lookup(tab[j], &pat[j], positions));
            sums.four(
                load(eq_t, i),
                values[0],
                values[1],
                values[2],
                values[3],
                send_one,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn jit_fold_group<const PRE_SCALED: bool, const WEIGH_LEFT: bool>(
    tab: [&[Gf]; 8],
    pat: &[[u8; 64]; 8],
    rho: &Gf,
    eq_t: &[Gf],
    send_one: bool,
    out_l: [&mut [MaybeUninit<Gf>]; 2],
    out_r: [&mut [MaybeUninit<Gf>]; 2],
    sums: &mut Sums,
) {
    let [l0, l1] = out_l;
    let [r0, r1] = out_r;
    let n = l0.len();
    assert!(n <= 64 && [l1.len(), r0.len(), r1.len()].iter().all(|&len| len >= n));
    assert_eq!(eq_t.len(), 64);
    let end = n / 4 * 4;
    // Work in consecutive output columns. col_of is an involution, so it
    // also maps each column back to its transposed pattern/weight position.
    // This permits contiguous SIMD stores instead of scattered lane extracts.
    // SAFETY: n<=64 bounds the permuted indices, every output holds n slots,
    // and only complete vectors are stored. Table lookups remain checked.
    unsafe {
        let scalar = broadcast(*rho);
        for c in (0..end).step_by(4) {
            let positions = std::array::from_fn(|j| col_of(c + j));
            let v: [__m512i; 8] = std::array::from_fn(|j| lookup(tab[j], &pat[j], positions));
            let combine = |lo, hi| {
                if PRE_SCALED {
                    _mm512_xor_si512(lo, hi)
                } else {
                    fold(scalar, lo, hi)
                }
            };
            let mut fl0 = combine(v[0], v[2]);
            let mut fl1 = combine(v[1], v[3]);
            let fr0 = combine(v[4], v[6]);
            let fr1 = combine(v[5], v[7]);
            let [wa, wb, wc, wd] = positions.map(|m| eq_t[m]);
            let weight = f128x4_set(wa, wb, wc, wd);
            if WEIGH_LEFT {
                fl0 = ghash_mul_x4(weight, fl0);
                fl1 = ghash_mul_x4(weight, fl1);
            }
            write(l0, c, fl0);
            write(l1, c, fl1);
            write(r0, c, fr0);
            write(r1, c, fr1);
            if WEIGH_LEFT {
                sums.four_weighted(fl0, fl1, fr0, fr1, send_one);
            } else {
                sums.four(weight, fl0, fl1, fr0, fr1, send_one);
            }
        }
    }
    for c in end..n {
        let m = col_of(c);
        let v = |j: usize| tab[j][pat[j][m] as usize];
        let combine = |lo, hi| {
            if PRE_SCALED {
                lo + hi
            } else {
                lo + *rho * (lo + hi)
            }
        };
        let mut fl0 = combine(v(0), v(2));
        let mut fl1 = combine(v(1), v(3));
        let fr0 = combine(v(4), v(6));
        let fr1 = combine(v(5), v(7));
        if WEIGH_LEFT {
            fl0 *= eq_t[m];
            fl1 *= eq_t[m];
        }
        l0[c].write(fl0);
        l1[c].write(fl1);
        r0[c].write(fr0);
        r1[c].write(fr1);
        if WEIGH_LEFT {
            sums.scalar_weighted(fl0, fl1, fr0, fr1, send_one);
        } else {
            sums.scalar(eq_t[m], fl0, fl1, fr0, fr1, send_one);
        }
    }
}

pub(super) fn scale_in_place(values: &mut [Gf], scalar: &Gf) {
    let end = values.len() / 4 * 4;
    // SAFETY: complete vectors only, followed by the scalar remainder.
    unsafe {
        let scalar = broadcast(*scalar);
        for i in (0..end).step_by(4) {
            let product = ghash_mul_x4(load(values, i), scalar);
            store(values, i, product);
        }
    }
    generic::scale_in_place(&mut values[end..], scalar);
}

pub(super) fn jit_product_group(tab: [&[Gf]; 2], pat: &[[u8; 64]; 2], out: &mut [MaybeUninit<Gf>]) {
    assert!(out.len() <= 64);
    let end = out.len() / 4 * 4;
    for c in (0..end).step_by(4) {
        let positions = std::array::from_fn(|j| col_of(c + j));
        // SAFETY: checked lookups and four initialized output slots per store.
        unsafe {
            write(
                out,
                c,
                ghash_mul_x4(
                    lookup(tab[0], &pat[0], positions),
                    lookup(tab[1], &pat[1], positions),
                ),
            );
        }
    }
    for (c, slot) in out.iter_mut().enumerate().skip(end) {
        let m = col_of(c);
        slot.write(tab[0][pat[0][m] as usize] * tab[1][pat[1][m] as usize]);
    }
}

pub(super) fn product_into(a: &[Gf], b: &[Gf], out: &mut [MaybeUninit<Gf>]) {
    let n = out.len();
    assert!(a.len() >= n && b.len() >= n);
    let end = n / 4 * 4;
    for i in (0..end).step_by(4) {
        // SAFETY: inputs and output all contain the complete four-element span.
        unsafe {
            write(out, i, ghash_mul_x4(load(a, i), load(b, i)));
        }
    }
    generic::product_into(&a[end..], &b[end..], &mut out[end..]);
}

pub(super) fn dot(a: &[Gf], b: &[Gf]) -> Gf {
    assert_eq!(a.len(), b.len());
    if a.len() < 4 {
        return generic::dot(a, b);
    }
    let end = a.len() / 4 * 4;
    // SAFETY: the accumulation only reads complete four-element spans.
    let sum = unsafe {
        let mut sum = WideGhashX4::zero();
        for i in (0..end).step_by(4) {
            sum.mul_acc(load(a, i), load(b, i));
        }
        sum.fold().reduce()
    };
    sum + generic::dot(&a[end..], &b[end..])
}

pub(super) fn contract(t_e: &[Gf], t_o: &[Gf], bucket: &[Gf]) -> Gf {
    let n = t_e.len();
    assert_eq!(t_o.len(), n);
    assert!(bucket.len() >= n * n);
    if n < 4 {
        return generic::contract(t_e, t_o, bucket);
    }
    t_e.iter()
        .enumerate()
        .fold(Gf::zero(), |sum, (row, &weight)| {
            sum + weight * dot(t_o, &bucket[row * n..(row + 1) * n])
        })
}
