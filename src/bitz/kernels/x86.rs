//! Four-lane GHASH kernels. The parent module gates every instruction used
//! here, including the scalar tail's PCLMUL and the horizontal fold's SSE4.1.
//! Only field operations are regrouped; no proof messages or domains change.

use core::arch::x86_64::*;
use std::mem::MaybeUninit;

use super::{Gf, col_of, generic};
use field::gf128::kernels::x86_64::{WideGhashX4, f128x4_loadu, f128x4_set, ghash_mul_x4};

pub(crate) use generic::scatter_add;

/// Reduced buckets keep the JIT scratch in 12 KiB. Reducing each four-lane
/// product before its XOR scatter is identical to reducing the scalar
/// buckets after accumulation: reduction is F₂-linear.
pub(crate) struct SumBuckets {
    data: Vec<Gf>,
}

impl SumBuckets {
    pub(crate) fn new() -> Self {
        Self {
            data: vec![Gf::zero(); 3 * 256],
        }
    }

    pub(crate) fn clear(&mut self) {
        self.data.fill(Gf::zero());
    }
}

/// XOR four products into their buckets in lane order. Repeated indices
/// must read the preceding lane's update, including four equal indices.
#[inline]
unsafe fn bucket_xor4(bucket: &mut [Gf], indices: &[u8; 64], first: usize, products: __m512i) {
    // SAFETY: callers supply 256 initialized entries and first <= 60. Each
    // byte index selects one Gf; unaligned 128-bit loads/stores are valid.
    // Every read-modify-write completes before the next lane reads its slot.
    unsafe {
        macro_rules! lane {
            ($lane:literal) => {{
                let p = bucket
                    .as_mut_ptr()
                    .add(indices[first + $lane] as usize)
                    .cast::<__m128i>();
                let product = _mm512_extracti32x4_epi32::<$lane>(products);
                _mm_storeu_si128(p, _mm_xor_si128(_mm_loadu_si128(p), product));
            }};
        }
        lane!(0);
        lane!(1);
        lane!(2);
        lane!(3);
    }
}

pub(crate) fn jit_bucket_group(
    tab_o: [&[Gf]; 2],
    pat: &[[u8; 64]; 4],
    eq_t: &[Gf],
    send_one: bool,
    bk: &mut SumBuckets,
) {
    assert_eq!(eq_t.len(), 64);
    assert!(tab_o.iter().all(|table| table.len() >= 256));
    let (end, inf) = bk.data.split_at_mut(256);
    let (inf_lo, inf_hi) = inf.split_at_mut(256);
    for first in (0..64).step_by(4) {
        // SAFETY: all weights and pattern positions are in bounds, every
        // table has 256 entries, and the module gates the full SIMD ISA.
        unsafe {
            let positions = [first, first + 1, first + 2, first + 3];
            let o_lo = lookup(tab_o[0], &pat[2], positions);
            let o_hi = lookup(tab_o[1], &pat[3], positions);
            let w = load(eq_t, first);
            let (a_end, o_end) = if send_one {
                (&pat[1], o_hi)
            } else {
                (&pat[0], o_lo)
            };
            let endpoint = ghash_mul_x4(w, o_end);
            let delta = ghash_mul_x4(w, _mm512_xor_si512(o_lo, o_hi));
            bucket_xor4(end, a_end, first, endpoint);
            bucket_xor4(inf_lo, &pat[0], first, delta);
            bucket_xor4(inf_hi, &pat[1], first, delta);
        }
    }
}

pub(crate) fn jit_bucket_finish(tab_e: [&[Gf]; 2], send_one: bool, bk: &SumBuckets) -> (Gf, Gf) {
    assert!(tab_e.iter().all(|table| table.len() >= 256));
    let t_end = if send_one { tab_e[1] } else { tab_e[0] };
    (
        dot(&t_end[..256], &bk.data[..256]),
        dot(&tab_e[0][..256], &bk.data[256..512]) + dot(&tab_e[1][..256], &bk.data[512..768]),
    )
}

/// Share each loaded weight across the four independent bucket streams.
/// Positions within a bucket remain sequential: equal byte indices must
/// accumulate both weights, rather than overwrite a gathered stale value.
pub(crate) fn scatter_add4(buckets: [&mut [Gf]; 4], idx: &[[u8; 64]; 4], eq_t: &[Gf]) {
    assert_eq!(eq_t.len(), 64);
    assert!(buckets.iter().all(|b| b.len() >= 256));
    // SAFETY: Gf is repr(C) over two u64s, with size/alignment 16. Every
    // u8 index selects a complete initialized element. The four mutable
    // slices are disjoint; each update is stored before the next position.
    unsafe {
        let base = buckets.map(|b| b.as_mut_ptr());
        for m in 0..64 {
            let weight = _mm_loadu_si128(eq_t.as_ptr().add(m).cast());
            for j in 0..4 {
                let p = base[j].add(idx[j][m] as usize).cast::<__m128i>();
                _mm_storeu_si128(p, _mm_xor_si128(_mm_loadu_si128(p), weight));
            }
        }
    }
}

pub(crate) struct Sums {
    end: WideGhashX4,
    inf: WideGhashX4,
    tail: [Gf; 2],
}

impl Sums {
    pub(crate) fn zero() -> Self {
        // SAFETY: the whole module is compiled only with the complete ISA.
        unsafe {
            Self {
                end: WideGhashX4::zero(),
                inf: WideGhashX4::zero(),
                tail: [Gf::zero(); 2],
            }
        }
    }

    pub(crate) fn finish(self) -> (Gf, Gf) {
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

pub(crate) fn round_sums_task(
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
pub(crate) fn fused_task<const LEFT: u8>(
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

pub(crate) fn jit_sums_group(
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
pub(crate) fn jit_fold_group<const PRE_SCALED: bool, const WEIGH_LEFT: bool>(
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

pub(crate) fn scale_in_place(values: &mut [Gf], scalar: &Gf) {
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

/// Multiply each entry by the weight at its complementary index.
pub(crate) fn multiply_reversed_in_place(values: &mut [Gf], weights: &[Gf]) {
    assert_eq!(values.len(), weights.len());
    let n = values.len();
    let end = n / 4 * 4;
    // SAFETY: each vector reads/stores four complete entries. Reversing
    // 128-bit lanes preserves the two-word representation of each field
    // element; weights and the mutable output are disjoint slices.
    unsafe {
        for i in (0..end).step_by(4) {
            let w = load(weights, n - i - 4);
            let reversed = _mm512_shuffle_i64x2::<0x1B>(w, w);
            store(values, i, ghash_mul_x4(load(values, i), reversed));
        }
    }
    for i in end..n {
        values[i] *= weights[n - 1 - i];
    }
}

pub(crate) fn jit_product_group(tab: [&[Gf]; 2], pat: &[[u8; 64]; 2], out: &mut [MaybeUninit<Gf>]) {
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

pub(crate) fn product_into(a: &[Gf], b: &[Gf], out: &mut [MaybeUninit<Gf>]) {
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

pub(crate) fn dot(a: &[Gf], b: &[Gf]) -> Gf {
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

pub(crate) fn contract(t_e: &[Gf], t_o: &[Gf], bucket: &[Gf]) -> Gf {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn element(i: usize) -> Gf {
        Gf::new(
            (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            (i as u64)
                .wrapping_mul(0xD6E8_FEB8_6659_FD93)
                .rotate_left(19),
        )
    }

    /// Experimental complete-table lookups. Production keeps the checked
    /// scalar assembly until the complete fold benchmark justifies a change.
    #[inline]
    unsafe fn lookup_complete<const GATHER: bool>(
        table: &[Gf],
        patterns: &[u8; 64],
        positions: [usize; 4],
    ) -> __m512i {
        let [a, b, c, d] = positions.map(|i| patterns[i] as usize);
        // SAFETY: the caller checks table.len() >= 256 before the loop.
        // Gf is repr(C) with two u64 words, in low/high polynomial order.
        // All byte indices select complete initialized field elements.
        unsafe {
            if GATHER {
                let [a, b, c, d] = [a, b, c, d].map(|i| (2 * i) as i64);
                let indices = _mm512_set_epi64(d + 1, d, c + 1, c, b + 1, b, a + 1, a);
                _mm512_i64gather_epi64::<8>(indices, table.as_ptr().cast::<i64>())
            } else {
                let a = _mm_loadu_si128(table.as_ptr().add(a).cast());
                let b = _mm_loadu_si128(table.as_ptr().add(b).cast());
                let c = _mm_loadu_si128(table.as_ptr().add(c).cast());
                let d = _mm_loadu_si128(table.as_ptr().add(d).cast());
                let lanes = _mm512_castsi128_si512(a);
                let lanes = _mm512_inserti32x4::<1>(lanes, b);
                let lanes = _mm512_inserti32x4::<2>(lanes, c);
                // Every previously unspecified upper lane is overwritten.
                _mm512_inserti32x4::<3>(lanes, d)
            }
        }
    }

    /// LOOKUP=0 runs the unchanged production kernel; 1 uses 128-bit loads
    /// and inserts; 2 uses eight 64-bit gathers. Only lookup assembly differs.
    #[allow(clippy::too_many_arguments)]
    fn jit_fold_lookup<const PRE_SCALED: bool, const WEIGH_LEFT: bool, const LOOKUP: u8>(
        tab: [&[Gf]; 8],
        pat: &[[u8; 64]; 8],
        rho: &Gf,
        eq_t: &[Gf],
        send_one: bool,
        out_l: [&mut [MaybeUninit<Gf>]; 2],
        out_r: [&mut [MaybeUninit<Gf>]; 2],
        sums: &mut Sums,
    ) {
        assert!(LOOKUP <= 2);
        // The checked kernel also accepts short tables whose referenced
        // indices fit, including empty tables when there are no outputs.
        if LOOKUP == 0 || tab.iter().any(|table| table.len() < 256) {
            return jit_fold_group::<PRE_SCALED, WEIGH_LEFT>(
                tab, pat, rho, eq_t, send_one, out_l, out_r, sums,
            );
        }
        let [l0, l1] = out_l;
        let [r0, r1] = out_r;
        let n = l0.len();
        assert!(n <= 64 && [l1.len(), r0.len(), r1.len()].iter().all(|&len| len >= n));
        assert_eq!(eq_t.len(), 64);
        let end = n / 4 * 4;
        // SAFETY: all complete tables and output shapes are checked before
        // any vector access. col_of permutes 0..64, and only full vectors
        // are written. The enclosing module gates every required ISA.
        unsafe {
            let scalar = broadcast(*rho);
            for c in (0..end).step_by(4) {
                let positions = std::array::from_fn(|j| col_of(c + j));
                let v: [__m512i; 8] = std::array::from_fn(|j| {
                    if LOOKUP == 1 {
                        lookup_complete::<false>(tab[j], &pat[j], positions)
                    } else {
                        lookup_complete::<true>(tab[j], &pat[j], positions)
                    }
                });
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

    fn fold_patterns(mode: usize, group: usize) -> [[u8; 64]; 8] {
        let mut state = (group as u64 + 1).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        std::array::from_fn(|j| {
            std::array::from_fn(|m| match mode {
                0 => 0,
                1 => 255,
                2 => ((j + m + group) % 2 * 255) as u8,
                _ => {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                }
            })
        })
    }

    fn initialized(out: &[Vec<MaybeUninit<Gf>>; 4]) -> [Vec<Gf>; 4] {
        out.each_ref().map(|values| {
            values
                .iter()
                // SAFETY: these tests initialize all entries, including
                // canaries, before passing their inner slices to a kernel.
                .map(|value| unsafe { value.assume_init() })
                .collect()
        })
    }

    fn check_fold_lookups<const PRE: bool, const WEIGH: bool>(short_tables: bool) {
        let original: [Vec<Gf>; 8] = std::array::from_fn(|j| {
            // Offset each table by one Gf to exercise unaligned SIMD loads.
            (0..258).map(|i| element(301 * j + i + 1)).collect()
        });
        let weights: [Gf; 64] = std::array::from_fn(|m| {
            if m % 5 == 0 {
                Gf::zero()
            } else {
                element(3001 + m)
            }
        });
        for rho in [Gf::zero(), Gf::one(), element(4001)] {
            let tables: [Vec<Gf>; 8] = std::array::from_fn(|j| {
                let scale = if !PRE {
                    Gf::one()
                } else if j & 2 == 0 {
                    Gf::one() + rho
                } else {
                    rho
                };
                original[j].iter().map(|&v| scale * v).collect()
            });
            let tab: [&[Gf]; 8] = std::array::from_fn(|j| {
                let len = if short_tables {
                    [1, 2, 3, 7, 16, 63, 128, 255][j]
                } else {
                    256
                };
                &tables[j][1..len + 1]
            });
            for mode in 0..4 {
                let mut patterns = fold_patterns(mode, 7);
                if short_tables {
                    for (pattern, table) in patterns.iter_mut().zip(tab) {
                        for index in pattern {
                            *index = (*index as usize % table.len()) as u8;
                        }
                    }
                }
                for n in 0..=64 {
                    for send_one in [false, true] {
                        let initial: [Vec<_>; 4] = std::array::from_fn(|j| {
                            (0..n + 2)
                                .map(|i| MaybeUninit::new(element(4101 + 67 * j + i)))
                                .collect()
                        });
                        let mut expected = initial.clone();
                        let [a, b, c, d] = &mut expected;
                        let mut oracle = generic::Sums::zero();
                        generic::jit_fold_group::<PRE, WEIGH>(
                            tab,
                            &patterns,
                            &rho,
                            &weights,
                            send_one,
                            [&mut a[1..n + 1], &mut b[1..n + 1]],
                            [&mut c[1..n + 1], &mut d[1..n + 1]],
                            &mut oracle,
                        );
                        let expected_sum = oracle.finish();
                        let expected = initialized(&expected);
                        macro_rules! check {
                            ($lookup:literal) => {{
                                let mut actual = initial.clone();
                                let [a, b, c, d] = &mut actual;
                                let mut sums = Sums::zero();
                                jit_fold_lookup::<PRE, WEIGH, $lookup>(
                                    tab,
                                    &patterns,
                                    &rho,
                                    &weights,
                                    send_one,
                                    [&mut a[1..n + 1], &mut b[1..n + 1]],
                                    [&mut c[1..n + 1], &mut d[1..n + 1]],
                                    &mut sums,
                                );
                                assert_eq!(
                                    sums.finish(), expected_sum,
                                    "lookup {} pre {PRE} weigh {WEIGH} mode {mode} n {n} endpoint {send_one}",
                                    $lookup
                                );
                                assert_eq!(initialized(&actual), expected);
                            }};
                        }
                        check!(0);
                        check!(1);
                        check!(2);
                    }
                }
            }
        }
    }

    #[test]
    fn jit_fold_lookup_candidates_match_generic_at_every_tail() {
        for short_tables in [false, true] {
            check_fold_lookups::<false, false>(short_tables);
            check_fold_lookups::<false, true>(short_tables);
            check_fold_lookups::<true, false>(short_tables);
            check_fold_lookups::<true, true>(short_tables);
        }
    }

    #[test]
    fn jit_fold_lookup_candidates_allow_empty_tables_with_empty_outputs() {
        macro_rules! check {
            ($lookup:literal) => {{
                let mut out: [[MaybeUninit<Gf>; 0]; 4] = [[]; 4];
                let [a, b, c, d] = &mut out;
                let mut sums = Sums::zero();
                jit_fold_lookup::<true, true, $lookup>(
                    [&[]; 8],
                    &[[255; 64]; 8],
                    &Gf::one(),
                    &[Gf::one(); 64],
                    true,
                    [a, b],
                    [c, d],
                    &mut sums,
                );
                assert_eq!(sums.finish(), (Gf::zero(), Gf::zero()));
            }};
        }
        check!(0);
        check!(1);
        check!(2);
    }

    #[test]
    fn jit_fold_lookup_candidates_reject_short_shapes_before_writes() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let full = [Gf::one(); 256];
        let patterns = [[255; 64]; 8];
        let weights = [Gf::one(); 64];
        // A malformed case is rejected before any output or sum mutation.
        let reject = |tab, eq: &[Gf], lengths: [usize; 4]| {
            macro_rules! check {
                ($lookup:literal) => {{
                    let mut out: [Vec<_>; 4] = std::array::from_fn(|j| {
                        vec![MaybeUninit::new(element(5001 + j)); lengths[j]]
                    });
                    let initial = initialized(&out);
                    let [a, b, c, d] = &mut out;
                    let mut sums = Sums::zero();
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            jit_fold_lookup::<true, true, $lookup>(
                                tab,
                                &patterns,
                                &Gf::one(),
                                eq,
                                true,
                                [a, b],
                                [c, d],
                                &mut sums,
                            );
                        }))
                        .is_err()
                    );
                    assert_eq!(initialized(&out), initial);
                    assert_eq!(sums.finish(), (Gf::zero(), Gf::zero()));
                }};
            }
            check!(1);
            check!(2);
        };
        for index in 0..8 {
            for len in [0, 255] {
                let mut tab = [&full[..]; 8];
                tab[index] = &full[..len];
                reject(tab, &weights, [64; 4]);
            }
        }
        for len in [0, 63] {
            reject([&full[..]; 8], &weights[..len], [64; 4]);
        }
        for lengths in [
            [65; 4],
            [64, 63, 64, 64],
            [64, 64, 63, 64],
            [64, 64, 64, 63],
        ] {
            reject([&full[..]; 8], &weights, lengths);
        }
    }

    #[test]
    #[ignore = "matched x86 JIT fold lookup microbenchmark; run explicitly on the benchmark host"]
    fn benchmark_jit_fold_lookup_candidates() {
        use std::{hint::black_box, time::Instant};

        let rho = element(6001);
        let tables: [Vec<Gf>; 8] = std::array::from_fn(|j| {
            let scale = if j & 2 == 0 { Gf::one() + rho } else { rho };
            (0..256)
                .map(|i| scale * element(6101 + 257 * j + i))
                .collect()
        });
        let tab = tables.each_ref().map(Vec::as_slice);
        for groups in [1usize, 16, 256] {
            let iterations = 4096 / groups;
            for mode in [0, 2, 3] {
                let inputs: Vec<_> = (0..groups)
                    .map(|g| {
                        let weights: [Gf; 64] = std::array::from_fn(|m| element(8201 + 67 * g + m));
                        (fold_patterns(mode, g), weights)
                    })
                    .collect();
                for send_one in [false, true] {
                    let mut outputs: [[Vec<MaybeUninit<Gf>>; 4]; 3] = std::array::from_fn(|_| {
                        std::array::from_fn(|_| vec![MaybeUninit::new(Gf::zero()); groups * 64])
                    });
                    let mut row = |method: usize| {
                        let mut sums = Sums::zero();
                        let [a, b, c, d] = &mut outputs[method];
                        for (g, (patterns, weights)) in inputs.iter().enumerate() {
                            let span = g * 64..(g + 1) * 64;
                            macro_rules! run {
                                ($lookup:literal) => {
                                    jit_fold_lookup::<true, true, $lookup>(
                                        black_box(tab),
                                        black_box(patterns),
                                        black_box(&rho),
                                        black_box(weights),
                                        send_one,
                                        [&mut a[span.clone()], &mut b[span.clone()]],
                                        [&mut c[span.clone()], &mut d[span]],
                                        &mut sums,
                                    )
                                };
                            }
                            match method {
                                0 => run!(0),
                                1 => run!(1),
                                2 => run!(2),
                                _ => unreachable!(),
                            }
                        }
                        black_box(&outputs[method]);
                        black_box(sums.finish())
                    };
                    // Untimed warmup also checks accumulated sums across
                    // all groups, including repeated indices and weights.
                    let expected = row(0);
                    assert_eq!(row(1), expected);
                    assert_eq!(row(2), expected);
                    let mut samples: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
                    for order in [
                        [0, 1, 2],
                        [2, 1, 0],
                        [1, 2, 0],
                        [0, 2, 1],
                        [2, 0, 1],
                        [1, 0, 2],
                    ] {
                        for method in order {
                            let start = Instant::now();
                            for _ in 0..iterations {
                                black_box(row(method));
                            }
                            samples[method]
                                .push(start.elapsed().as_secs_f64() * 1e9 / iterations as f64);
                        }
                    }
                    let median = samples.map(|mut ns| {
                        ns.sort_by(f64::total_cmp);
                        (ns[2] + ns[3]) / 2.0
                    });
                    assert_eq!(initialized(&outputs[1]), initialized(&outputs[0]));
                    assert_eq!(initialized(&outputs[2]), initialized(&outputs[0]));
                    eprintln!(
                        "jit_fold_lookup groups={groups} mode={mode} send_one={send_one} checked_ns={:.1} insert_ns={:.1} gather_ns={:.1} insert_ratio={:.4} gather_ratio={:.4}",
                        median[0],
                        median[1],
                        median[2],
                        median[1] / median[0],
                        median[2] / median[0]
                    );
                }
            }
        }
    }

    #[test]
    fn reversed_multiplication_matches_scalar_at_every_tail_length() {
        for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 31, 32, 33, 63, 64, 65] {
            let original: Vec<Gf> = (0..n).map(element).collect();
            let weights: Vec<Gf> = (0..n).map(|i| element(2 * i + 7)).collect();
            let mut got = original.clone();
            multiply_reversed_in_place(&mut got, &weights);
            let want: Vec<_> = original
                .iter()
                .zip(weights.iter().rev())
                .map(|(&a, &b)| a * b)
                .collect();
            assert_eq!(got, want, "n {n}");
        }
    }

    #[test]
    fn scatter_four_matches_scalar_with_collisions_padding_and_guards() {
        for live in [0, 1, 31, 32, 33, 63, 64] {
            let eq: [Gf; 64] =
                std::array::from_fn(|i| if i < live { element(i + 1) } else { Gf::zero() });
            for mode in 0..4 {
                let idx: [[u8; 64]; 4] = std::array::from_fn(|j| {
                    std::array::from_fn(|m| match mode {
                        0 => 0,
                        1 => 255,
                        2 => ((m + j) % 2 * 255) as u8,
                        _ => ((m * 17 + j * 31) & 255) as u8,
                    })
                });
                let mut got: [Vec<Gf>; 4] =
                    std::array::from_fn(|j| (0..258).map(|i| element(300 * j + i + 1)).collect());
                let mut want = got.clone();
                let [a, b, c, d] = &mut got;
                scatter_add4(
                    [
                        &mut a[1..257],
                        &mut b[1..257],
                        &mut c[1..257],
                        &mut d[1..257],
                    ],
                    &idx,
                    &eq,
                );
                for (bucket, pattern) in want.iter_mut().zip(&idx) {
                    generic::scatter_add(&mut bucket[1..257], pattern, &eq);
                }
                assert_eq!(got, want, "live {live} mode {mode}");
            }
        }
    }

    #[test]
    #[should_panic]
    fn scatter_four_rejects_short_bucket_before_using_byte_indices() {
        let mut buckets = [[Gf::zero(); 255]; 4];
        scatter_add4(
            buckets.each_mut().map(|b| &mut b[..]),
            &[[255; 64]; 4],
            &[Gf::zero(); 64],
        );
    }

    #[test]
    fn jit_bucket_scatter_preserves_colliding_lanes_and_guards() {
        let products: [Gf; 4] = std::array::from_fn(|i| element(701 + i));
        for indices in [
            [0, 0, 0, 0],
            [255, 255, 255, 255],
            [0, 255, 0, 255],
            [0, 1, 2, 3],
        ] {
            let mut pattern = [0; 64];
            pattern[60..].copy_from_slice(&indices);
            let mut actual: Vec<_> = (0..258).map(element).collect();
            let mut expected = actual.clone();
            // Repeating the scatter covers updates into live buckets as
            // well as cancellation. The prefix and suffix are canaries.
            for _ in 0..3 {
                for (&index, &product) in indices.iter().zip(&products) {
                    expected[1 + index as usize] += product;
                }
                // SAFETY: the module ISA is enabled; the slice has exactly
                // 256 entries and the four pattern positions end at 64.
                unsafe {
                    bucket_xor4(
                        &mut actual[1..257],
                        &pattern,
                        60,
                        f128x4_loadu(products.as_ptr()),
                    );
                }
                assert_eq!(actual, expected, "indices {indices:?}");
            }
        }
    }

    #[test]
    fn jit_bucket_rejects_short_shapes_before_simd_access() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let full = vec![Gf::one(); 256];
        let short = &full[..255];
        let patterns = [[255; 64]; 4];
        let weights = [Gf::one(); 64];
        for tab_o in [[short, &full[..]], [&full[..], short]] {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    jit_bucket_group(tab_o, &patterns, &weights, false, &mut SumBuckets::new());
                }))
                .is_err()
            );
        }
        for n in [0, 63] {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    jit_bucket_group(
                        [&full, &full],
                        &patterns,
                        &weights[..n],
                        false,
                        &mut SumBuckets::new(),
                    );
                }))
                .is_err()
            );
        }
        for tab_e in [[short, &full[..]], [&full[..], short]] {
            assert!(
                catch_unwind(|| {
                    jit_bucket_finish(tab_e, true, &SumBuckets::new());
                })
                .is_err()
            );
        }
    }
}
