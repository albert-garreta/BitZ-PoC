//! Fused table folds and messages. SIMD changes only how independent field
//! operations execute; table order and the virtual sumcheck domain are fixed.
use flock_core::field::Gf128 as F;
use std::mem::MaybeUninit;

/// Occupied columns of one position group. Missing pair operands are zero.
#[derive(Clone, Copy)]
pub(crate) struct Lanes<'a> {
    pub columns: usize,
    pub pairs: &'a [[usize; 2]],
}

impl Lanes<'_> {
    fn validate(self) {
        assert!(self.columns > 0);
        assert!(
            self.pairs
                .iter()
                .flatten()
                .all(|&column| column == usize::MAX || column < self.columns)
        );
    }

    fn is_prefix(self) -> bool {
        self.pairs.len() == self.columns.div_ceil(2)
            && self.pairs.iter().enumerate().all(|(i, &[left, right])| {
                left == 2 * i
                    && right
                        == if 2 * i + 1 < self.columns {
                            2 * i + 1
                        } else {
                            usize::MAX
                        }
            })
    }
}

fn get(row: &[F], column: usize) -> F {
    if column == usize::MAX {
        F::ZERO
    } else {
        row[column]
    }
}

fn scalar_message(x: &[F], w: &[F], lanes: Lanes<'_>) -> [F; 2] {
    let mut sum = [F::ZERO; 2];
    for (x, w) in x
        .chunks_exact(lanes.columns)
        .zip(w.chunks_exact(lanes.columns))
    {
        for &[left, right] in lanes.pairs {
            let (x0, x1) = (get(x, left), get(x, right));
            let (w0, w1) = (get(w, left), get(w, right));
            sum[0] += x0 * w0;
            sum[1] += (x0 + x1) * (w0 + w1);
        }
    }
    sum
}

fn scalar_dense(
    x: &[F],
    w: &[F],
    xo: &mut [MaybeUninit<F>],
    wo: &mut [MaybeUninit<F>],
    r: F,
) -> [F; 2] {
    let mut sum = [F::ZERO; 2];
    let mut previous = [F::ZERO; 2];
    for i in 0..xo.len() {
        let xf = x[2 * i] + r * (x[2 * i] + x[2 * i + 1]);
        let wf = w[2 * i] + r * (w[2 * i] + w[2 * i + 1]);
        xo[i].write(xf);
        wo[i].write(wf);
        if i & 1 == 0 {
            previous = [xf, wf];
        } else {
            sum[0] += previous[0] * previous[1];
            sum[1] += (previous[0] + xf) * (previous[1] + wf);
        }
    }
    sum
}

fn scalar_lanes(
    x: &[F],
    w: &[F],
    xo: &mut [MaybeUninit<F>],
    wo: &mut [MaybeUninit<F>],
    lanes: Lanes<'_>,
    next: Lanes<'_>,
    cross_groups: bool,
    r: F,
) -> [F; 2] {
    let mut sum = [F::ZERO; 2];
    let mut previous = [F::ZERO; 2];
    for (group, ((x, w), (xo, wo))) in x
        .chunks_exact(lanes.columns)
        .zip(w.chunks_exact(lanes.columns))
        .zip(
            xo.chunks_exact_mut(next.columns)
                .zip(wo.chunks_exact_mut(next.columns)),
        )
        .enumerate()
    {
        let mut xf = [F::ZERO; 16];
        let mut wf = [F::ZERO; 16];
        for (column, &[left, right]) in lanes.pairs.iter().enumerate() {
            let (x0, x1) = (get(x, left), get(x, right));
            let (w0, w1) = (get(w, left), get(w, right));
            xf[column] = x0 + r * (x0 + x1);
            wf[column] = w0 + r * (w0 + w1);
            xo[column].write(xf[column]);
            wo[column].write(wf[column]);
        }
        if cross_groups {
            if group & 1 == 0 {
                previous = [xf[0], wf[0]];
            } else {
                sum[0] += previous[0] * previous[1];
                sum[1] += (previous[0] + xf[0]) * (previous[1] + wf[0]);
            }
        } else {
            let term = scalar_message(&xf[..next.columns], &wf[..next.columns], next);
            sum[0] += term[0];
            sum[1] += term[1];
        }
    }
    sum
}

pub(crate) fn message(x: &[F], w: &[F], lanes: Lanes<'_>) -> [F; 2] {
    lanes.validate();
    assert_eq!(x.len(), w.len());
    assert!(x.len().is_multiple_of(lanes.columns));
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: static target features; the kernel bounds each four-group load.
    return unsafe { x86::message(x, w, lanes) };
    #[cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    )))]
    scalar_message(x, w, lanes)
}

pub(crate) fn dense(
    x: &[F],
    w: &[F],
    xo: &mut [MaybeUninit<F>],
    wo: &mut [MaybeUninit<F>],
    r: F,
) -> [F; 2] {
    assert_eq!(x.len(), 2 * xo.len());
    assert_eq!(w.len(), x.len());
    assert_eq!(wo.len(), xo.len());
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: static target features and matching input/output lengths.
    return unsafe { x86::dense(x, w, xo, wo, r) };
    #[cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    )))]
    scalar_dense(x, w, xo, wo, r)
}

pub(crate) fn lanes(
    x: &[F],
    w: &[F],
    xo: &mut [MaybeUninit<F>],
    wo: &mut [MaybeUninit<F>],
    lanes: Lanes<'_>,
    next: Lanes<'_>,
    cross_groups: bool,
    r: F,
) -> [F; 2] {
    lanes.validate();
    next.validate();
    assert_eq!(x.len(), w.len());
    assert!(x.len().is_multiple_of(lanes.columns));
    assert_eq!(xo.len(), x.len() / lanes.columns * next.columns);
    assert_eq!(wo.len(), xo.len());
    assert_eq!(lanes.pairs.len(), next.columns);
    assert!(next.columns <= 16);
    assert!(!cross_groups || next.columns == 1);
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    ))]
    // SAFETY: static target features and matching compact table dimensions.
    return unsafe { x86::lanes(x, w, xo, wo, lanes, next, cross_groups, r) };
    #[cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "vpclmulqdq"
    )))]
    scalar_lanes(x, w, xo, wo, lanes, next, cross_groups, r)
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
    use core::arch::x86_64::*;
    use flock_core::field::gf128_kernels::x86_64::{WideGhashX4, ghash_mul_x4};

    // Four adjacent four-field groups -> four SIMD columns.
    #[inline]
    unsafe fn columns(p: *const F) -> [__m512i; 4] {
        unsafe {
            let a = _mm512_loadu_si512(p.cast());
            let b = _mm512_loadu_si512(p.add(4).cast());
            let c = _mm512_loadu_si512(p.add(8).cast());
            let d = _mm512_loadu_si512(p.add(12).cast());
            let lo = _mm512_set_epi64(11, 10, 9, 8, 3, 2, 1, 0);
            let hi = _mm512_set_epi64(15, 14, 13, 12, 7, 6, 5, 4);
            let even = _mm512_set_epi64(13, 12, 9, 8, 5, 4, 1, 0);
            let odd = _mm512_set_epi64(15, 14, 11, 10, 7, 6, 3, 2);
            let ab0 = _mm512_permutex2var_epi64(a, lo, b);
            let ab1 = _mm512_permutex2var_epi64(a, hi, b);
            let cd0 = _mm512_permutex2var_epi64(c, lo, d);
            let cd1 = _mm512_permutex2var_epi64(c, hi, d);
            [
                _mm512_permutex2var_epi64(ab0, even, cd0),
                _mm512_permutex2var_epi64(ab0, odd, cd0),
                _mm512_permutex2var_epi64(ab1, even, cd1),
                _mm512_permutex2var_epi64(ab1, odd, cd1),
            ]
        }
    }

    #[inline]
    unsafe fn broadcast(r: F) -> __m512i {
        unsafe { _mm512_broadcast_i32x4(_mm_set_epi64x(r.hi as i64, r.lo as i64)) }
    }

    #[inline]
    unsafe fn fold(a: __m512i, b: __m512i, r: __m512i) -> __m512i {
        unsafe { _mm512_xor_si512(a, ghash_mul_x4(r, _mm512_xor_si512(a, b))) }
    }

    #[inline]
    unsafe fn store_pairs(p: *mut MaybeUninit<F>, a: __m512i, b: __m512i) {
        unsafe {
            let lo = _mm512_set_epi64(11, 10, 3, 2, 9, 8, 1, 0);
            let hi = _mm512_set_epi64(15, 14, 7, 6, 13, 12, 5, 4);
            _mm512_storeu_si512(p.cast(), _mm512_permutex2var_epi64(a, lo, b));
            _mm512_storeu_si512(p.add(4).cast(), _mm512_permutex2var_epi64(a, hi, b));
        }
    }

    pub(super) unsafe fn dense(
        x: &[F],
        w: &[F],
        xo: &mut [MaybeUninit<F>],
        wo: &mut [MaybeUninit<F>],
        r: F,
    ) -> [F; 2] {
        if xo.len() < 8 {
            return scalar_dense(x, w, xo, wo, r);
        }
        unsafe {
            let rr = broadcast(r);
            let mut acc = [WideGhashX4::zero(); 2];
            let mut i = 0;
            while i + 8 <= xo.len() {
                let x = columns(x.as_ptr().add(2 * i));
                let w = columns(w.as_ptr().add(2 * i));
                let x0 = fold(x[0], x[1], rr);
                let x1 = fold(x[2], x[3], rr);
                let w0 = fold(w[0], w[1], rr);
                let w1 = fold(w[2], w[3], rr);
                store_pairs(xo.as_mut_ptr().add(i), x0, x1);
                store_pairs(wo.as_mut_ptr().add(i), w0, w1);
                acc[0].mul_acc(x0, w0);
                acc[1].mul_acc(_mm512_xor_si512(x0, x1), _mm512_xor_si512(w0, w1));
                i += 8;
            }
            let tail = scalar_dense(&x[2 * i..], &w[2 * i..], &mut xo[i..], &mut wo[i..], r);
            let total = acc.map(|a| a.fold().reduce());
            [total[0] + tail[0], total[1] + tail[1]]
        }
    }

    #[inline]
    unsafe fn offsets(columns: usize) -> __m512i {
        let s = 2 * columns as i64;
        unsafe { _mm512_set_epi64(3 * s + 1, 3 * s, 2 * s + 1, 2 * s, s + 1, s, 1, 0) }
    }

    #[inline]
    unsafe fn load(p: *const F, column: usize, offsets: __m512i) -> __m512i {
        unsafe {
            if column == usize::MAX {
                _mm512_setzero_si512()
            } else {
                _mm512_i64gather_epi64::<8>(offsets, p.add(column).cast())
            }
        }
    }

    /// Read only the occupied prefix, leaving every missing field lane zero.
    #[inline]
    unsafe fn load_prefix(p: *const F, fields: usize) -> __m512i {
        unsafe {
            if fields == 4 {
                _mm512_loadu_si512(p.cast())
            } else {
                _mm512_maskz_loadu_epi64(((1u16 << (2 * fields)) - 1) as u8, p.cast())
            }
        }
    }

    #[inline]
    unsafe fn store_prefix(p: *mut MaybeUninit<F>, value: __m512i, fields: usize) {
        unsafe {
            if fields == 4 {
                _mm512_storeu_si512(p.cast(), value);
            } else {
                _mm512_mask_storeu_epi64(p.cast(), ((1u16 << (2 * fields)) - 1) as u8, value);
            }
        }
    }

    #[inline]
    unsafe fn deinterleave(lo: __m512i, hi: __m512i) -> [__m512i; 2] {
        unsafe {
            let even = _mm512_set_epi64(13, 12, 9, 8, 5, 4, 1, 0);
            let odd = _mm512_set_epi64(15, 14, 11, 10, 7, 6, 3, 2);
            [
                _mm512_permutex2var_epi64(lo, even, hi),
                _mm512_permutex2var_epi64(lo, odd, hi),
            ]
        }
    }

    #[inline]
    unsafe fn prefix_pairs(p: *const F, fields: usize) -> [__m512i; 2] {
        unsafe {
            let lo = load_prefix(p, fields.min(4));
            let hi = if fields > 4 {
                load_prefix(p.add(4), fields - 4)
            } else {
                _mm512_setzero_si512()
            };
            deinterleave(lo, hi)
        }
    }

    /// The Falcon supports are contiguous prefixes of 11, 6, then 3 fields.
    /// Work across adjacent pairs inside each group, with ordinary loads and
    /// masked tails instead of gathers/scatters across four strided groups.
    unsafe fn prefix_lanes<const C: usize>(
        x: &[F],
        w: &[F],
        xo: &mut [MaybeUninit<F>],
        wo: &mut [MaybeUninit<F>],
        r: F,
    ) -> [F; 2] {
        unsafe {
            let out_columns = C.div_ceil(2);
            let rr = broadcast(r);
            let mut acc = [WideGhashX4::zero(); 2];
            for group in 0..x.len() / C {
                let xp = x.as_ptr().add(group * C);
                let wp = w.as_ptr().add(group * C);
                let a = prefix_pairs(xp, C.min(8));
                let b = prefix_pairs(wp, C.min(8));
                let xf = fold(a[0], a[1], rr);
                let wf = fold(b[0], b[1], rr);
                let (xt, wt) = if C > 8 {
                    let a = prefix_pairs(xp.add(8), C - 8);
                    let b = prefix_pairs(wp.add(8), C - 8);
                    (fold(a[0], a[1], rr), fold(b[0], b[1], rr))
                } else {
                    (_mm512_setzero_si512(), _mm512_setzero_si512())
                };
                let xop = xo.as_mut_ptr().add(group * out_columns);
                let wop = wo.as_mut_ptr().add(group * out_columns);
                store_prefix(xop, xf, out_columns.min(4));
                store_prefix(wop, wf, out_columns.min(4));
                if out_columns > 4 {
                    store_prefix(xop.add(4), xt, out_columns - 4);
                    store_prefix(wop.add(4), wt, out_columns - 4);
                }
                let a = deinterleave(xf, xt);
                let b = deinterleave(wf, wt);
                acc[0].mul_acc(a[0], b[0]);
                acc[1].mul_acc(_mm512_xor_si512(a[0], a[1]), _mm512_xor_si512(b[0], b[1]));
            }
            acc.map(|v| v.fold().reduce())
        }
    }

    unsafe fn prefix_message<const C: usize>(x: &[F], w: &[F]) -> [F; 2] {
        unsafe {
            let mut acc = [WideGhashX4::zero(); 2];
            for group in 0..x.len() / C {
                for offset in (0..C).step_by(8) {
                    let a = prefix_pairs(x.as_ptr().add(group * C + offset), (C - offset).min(8));
                    let b = prefix_pairs(w.as_ptr().add(group * C + offset), (C - offset).min(8));
                    acc[0].mul_acc(a[0], b[0]);
                    acc[1].mul_acc(_mm512_xor_si512(a[0], a[1]), _mm512_xor_si512(b[0], b[1]));
                }
            }
            acc.map(|v| v.fold().reduce())
        }
    }

    pub(super) unsafe fn message(x: &[F], w: &[F], lanes: Lanes<'_>) -> [F; 2] {
        if x.len() / lanes.columns < 4 {
            return scalar_message(x, w, lanes);
        }
        unsafe {
            if lanes.is_prefix() {
                match lanes.columns {
                    11 => return prefix_message::<11>(x, w),
                    6 => return prefix_message::<6>(x, w),
                    3 => return prefix_message::<3>(x, w),
                    _ => {}
                }
            }
            let stride = offsets(lanes.columns);
            let mut acc = [WideGhashX4::zero(); 2];
            let mut i = 0;
            while i + 4 * lanes.columns <= x.len() {
                for &[left, right] in lanes.pairs {
                    let x0 = load(x.as_ptr().add(i), left, stride);
                    let x1 = load(x.as_ptr().add(i), right, stride);
                    let w0 = load(w.as_ptr().add(i), left, stride);
                    let w1 = load(w.as_ptr().add(i), right, stride);
                    acc[0].mul_acc(x0, w0);
                    acc[1].mul_acc(_mm512_xor_si512(x0, x1), _mm512_xor_si512(w0, w1));
                }
                i += 4 * lanes.columns;
            }
            let tail = scalar_message(&x[i..], &w[i..], lanes);
            let total = acc.map(|a| a.fold().reduce());
            [total[0] + tail[0], total[1] + tail[1]]
        }
    }

    pub(super) unsafe fn lanes(
        x: &[F],
        w: &[F],
        xo: &mut [MaybeUninit<F>],
        wo: &mut [MaybeUninit<F>],
        lanes: Lanes<'_>,
        next: Lanes<'_>,
        cross_groups: bool,
        r: F,
    ) -> [F; 2] {
        if x.len() / lanes.columns < 4 {
            return scalar_lanes(x, w, xo, wo, lanes, next, cross_groups, r);
        }
        unsafe {
            if !cross_groups && lanes.is_prefix() && next.is_prefix() {
                match lanes.columns {
                    11 => return prefix_lanes::<11>(x, w, xo, wo, r),
                    6 => return prefix_lanes::<6>(x, w, xo, wo, r),
                    3 => return prefix_lanes::<3>(x, w, xo, wo, r),
                    _ => {}
                }
            }
            let rr = broadcast(r);
            let input_stride = offsets(lanes.columns);
            let output_stride = offsets(next.columns);
            let zero = _mm512_setzero_si512();
            let mut acc = [WideGhashX4::zero(); 2];
            let groups = x.len() / lanes.columns;
            let mut group = 0;
            while group + 4 <= groups {
                let mut xf = [zero; 16];
                let mut wf = [zero; 16];
                let xp = x.as_ptr().add(group * lanes.columns);
                let wp = w.as_ptr().add(group * lanes.columns);
                for (column, &[left, right]) in lanes.pairs.iter().enumerate() {
                    xf[column] = fold(
                        load(xp, left, input_stride),
                        load(xp, right, input_stride),
                        rr,
                    );
                    wf[column] = fold(
                        load(wp, left, input_stride),
                        load(wp, right, input_stride),
                        rr,
                    );
                    _mm512_i64scatter_epi64::<8>(
                        xo.as_mut_ptr().add(group * next.columns + column).cast(),
                        output_stride,
                        xf[column],
                    );
                    _mm512_i64scatter_epi64::<8>(
                        wo.as_mut_ptr().add(group * next.columns + column).cast(),
                        output_stride,
                        wf[column],
                    );
                }
                if cross_groups {
                    // Folded rows have one column. The next message pairs
                    // groups (0,1) and (2,3), not columns within a group.
                    let swap = _mm512_set_epi64(5, 4, 7, 6, 1, 0, 3, 2);
                    let x1 = _mm512_permutexvar_epi64(swap, xf[0]);
                    let w1 = _mm512_permutexvar_epi64(swap, wf[0]);
                    acc[0].mul_acc(_mm512_maskz_mov_epi64(0x33, xf[0]), wf[0]);
                    acc[1].mul_acc(
                        _mm512_maskz_mov_epi64(0x33, _mm512_xor_si512(xf[0], x1)),
                        _mm512_xor_si512(wf[0], w1),
                    );
                } else {
                    for &[left, right] in next.pairs {
                        let x0 = if left == usize::MAX { zero } else { xf[left] };
                        let x1 = if right == usize::MAX { zero } else { xf[right] };
                        let w0 = if left == usize::MAX { zero } else { wf[left] };
                        let w1 = if right == usize::MAX { zero } else { wf[right] };
                        acc[0].mul_acc(x0, w0);
                        acc[1].mul_acc(_mm512_xor_si512(x0, x1), _mm512_xor_si512(w0, w1));
                    }
                }
                group += 4;
            }
            let tail = scalar_lanes(
                &x[group * lanes.columns..],
                &w[group * lanes.columns..],
                &mut xo[group * next.columns..],
                &mut wo[group * next.columns..],
                lanes,
                next,
                cross_groups,
                r,
            );
            let total = acc.map(|a| a.fold().reduce());
            [total[0] + tail[0], total[1] + tail[1]]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(i: usize) -> F {
        let lo = (i as u64)
            .wrapping_mul(0x9e3779b97f4a7c15)
            .wrapping_add(0x123456789abcdef);
        F {
            lo,
            hi: lo.rotate_left(29) ^ 0xfedcba9876543210,
        }
    }

    // Initialize the scratch with sentinels, so inspecting it cannot hide an
    // uninitialized output if a new SIMD tail accidentally skips a write.
    fn scratch(n: usize) -> Vec<MaybeUninit<F>> {
        vec![MaybeUninit::new(value(123456)); n]
    }

    fn initialized(values: &[MaybeUninit<F>]) -> Vec<F> {
        // SAFETY: test scratch is initialized before calling any kernel.
        values.iter().map(|v| unsafe { v.assume_init() }).collect()
    }

    #[test]
    fn dense_selected_matches_scalar_with_tails_and_boolean_challenges() {
        for n in (0..35).chain([63, 64, 65, 1024]) {
            let x: Vec<_> = (0..2 * n).map(value).collect();
            let w: Vec<_> = (0..2 * n).map(|i| value(i + 3456)).collect();
            for r in [F::ZERO, F::ONE, value(999)] {
                let (mut ax, mut aw) = (scratch(n), scratch(n));
                let (mut ex, mut ew) = (scratch(n), scratch(n));
                assert_eq!(
                    dense(&x, &w, &mut ax, &mut aw, r),
                    scalar_dense(&x, &w, &mut ex, &mut ew, r),
                    "n={n}, r={r:?}"
                );
                assert_eq!(initialized(&ax), initialized(&ex));
                assert_eq!(initialized(&aw), initialized(&ew));
            }
        }
    }

    fn pairs(active: &[usize]) -> Vec<[usize; 2]> {
        let mut out = Vec::new();
        let mut previous = usize::MAX;
        for (column, &lane) in active.iter().enumerate() {
            if lane / 2 != previous {
                out.push([usize::MAX; 2]);
                previous = lane / 2;
            }
            out.last_mut().unwrap()[lane & 1] = column;
        }
        out
    }

    #[test]
    fn compact_selected_matches_scalar_supports_tails_and_cross_group_pairs() {
        for width in [2, 4, 8, 16] {
            let masks: Vec<_> = if width < 16 {
                (1..1 << width).collect()
            } else {
                vec![1, 2, 3, 0x401, 0x7ff, 0x8000, 0xa5a5, 0xfffe, 0xffff]
            };
            for mask in masks {
                let active: Vec<_> = (0..width).filter(|lane| mask & (1 << lane) != 0).collect();
                let pairs = pairs(&active);
                let mut next_active: Vec<_> = active.iter().map(|lane| lane / 2).collect();
                next_active.dedup();
                let next_pairs = self::pairs(&next_active);
                let shape = Lanes {
                    columns: active.len(),
                    pairs: &pairs,
                };
                let next = Lanes {
                    columns: next_active.len(),
                    pairs: &next_pairs,
                };
                for groups in [0, 1, 2, 3, 4, 5, 6, 7, 8, 17, 64, 65] {
                    let n = groups * shape.columns;
                    let x: Vec<_> = (0..n).map(value).collect();
                    let w: Vec<_> = (0..n).map(|i| value(i + 3456)).collect();
                    assert_eq!(
                        message(&x, &w, shape),
                        scalar_message(&x, &w, shape),
                        "width={width}, mask={mask}, groups={groups}"
                    );
                    for r in [F::ZERO, F::ONE, value(999)] {
                        let len = groups * next.columns;
                        let (mut ax, mut aw) = (scratch(len), scratch(len));
                        let (mut ex, mut ew) = (scratch(len), scratch(len));
                        assert_eq!(
                            lanes(&x, &w, &mut ax, &mut aw, shape, next, width == 2, r),
                            scalar_lanes(&x, &w, &mut ex, &mut ew, shape, next, width == 2, r),
                            "width={width}, mask={mask}, groups={groups}, r={r:?}"
                        );
                        assert_eq!(initialized(&ax), initialized(&ex));
                        assert_eq!(initialized(&aw), initialized(&ew));
                    }
                }
            }
        }
    }

    #[test]
    fn prefix_masks_preserve_guards_on_unaligned_subslices() {
        for columns in [11, 6, 3] {
            let active: Vec<_> = (0..columns).collect();
            let pair_map = pairs(&active);
            let next_columns = columns.div_ceil(2);
            let next_active: Vec<_> = (0..next_columns).collect();
            let next_pairs = pairs(&next_active);
            let shape = Lanes {
                columns,
                pairs: &pair_map,
            };
            let next = Lanes {
                columns: next_columns,
                pairs: &next_pairs,
            };
            for offset in 1..=3 {
                for groups in [5, 9] {
                    let n = groups * columns;
                    let out = groups * next_columns;
                    let x: Vec<_> = (0..n + 8).map(value).collect();
                    let w: Vec<_> = (0..n + 8).map(|i| value(i + 3456)).collect();
                    let (mut ax, mut aw) = (scratch(out + 8), scratch(out + 8));
                    let guards = initialized(&ax);
                    let (mut ex, mut ew) = (scratch(out), scratch(out));
                    assert_eq!(
                        lanes(
                            &x[offset..offset + n],
                            &w[offset..offset + n],
                            &mut ax[offset..offset + out],
                            &mut aw[offset..offset + out],
                            shape,
                            next,
                            false,
                            value(999)
                        ),
                        scalar_lanes(
                            &x[offset..offset + n],
                            &w[offset..offset + n],
                            &mut ex,
                            &mut ew,
                            shape,
                            next,
                            false,
                            value(999)
                        ),
                    );
                    let ax = initialized(&ax);
                    let aw = initialized(&aw);
                    assert_eq!(ax[offset..offset + out], initialized(&ex));
                    assert_eq!(aw[offset..offset + out], initialized(&ew));
                    assert_eq!(ax[..offset], guards[..offset]);
                    assert_eq!(aw[..offset], guards[..offset]);
                    assert_eq!(ax[offset + out..], guards[offset + out..]);
                    assert_eq!(aw[offset + out..], guards[offset + out..]);
                }
            }
        }
    }
}
