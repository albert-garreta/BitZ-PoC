//! Four-lane GHASH sumcheck kernels. The module is compiled only when the
//! entire instruction set is enabled; protocol traversal stays with callers.
use crate::Gf128;
use crate::gf128::kernels::x86_64::{WideGhashX4, f128x4_loadu, f128x4_set, ghash_mul_x4};
use core::arch::x86_64::*;

#[inline(always)]
unsafe fn store4(p: *mut Gf128, v: __m512i) {
    // SAFETY: callers provide four writable elements; unaligned stores accept
    // Gf128's 16-byte alignment. The module gate enables AVX-512F.
    unsafe { _mm512_storeu_si512(p.cast(), v) };
}

#[inline(always)]
unsafe fn broadcast(v: Gf128) -> __m512i {
    // SAFETY: all four fields are ordinary register values and AVX-512F is enabled.
    unsafe { f128x4_set(v, v, v, v) }
}

/// Split eight contiguous field elements into their even and odd positions.
#[inline(always)]
unsafe fn split_pairs(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
    // SAFETY: the module gate enables AVX-512F; indices are fixed register lanes.
    unsafe {
        let even = _mm512_set_epi64(13, 12, 9, 8, 5, 4, 1, 0);
        let odd = _mm512_set_epi64(15, 14, 11, 10, 7, 6, 3, 2);
        (
            _mm512_permutex2var_epi64(a, even, b),
            _mm512_permutex2var_epi64(a, odd, b),
        )
    }
}

#[inline(always)]
unsafe fn fold_pair(a: __m512i, b: __m512i, rho: __m512i) -> __m512i {
    // SAFETY: the module gate enables every instruction in ghash_mul_x4.
    unsafe { _mm512_xor_si512(a, ghash_mul_x4(rho, _mm512_xor_si512(a, b))) }
}

/// Four consecutive logical values after D low-variable folds. All input
/// loads finish before the caller writes the corresponding prefix.
#[inline(always)]
unsafe fn folded4<const D: usize>(v: &[Gf128], i: usize, rho: &[__m512i; 2]) -> __m512i {
    // SAFETY: the public entry point's shape check covers 4 << D elements.
    // D is 0, 1, or 2, selected solely from the public number of pending rounds.
    unsafe {
        let p = v.as_ptr().add(i << D);
        let a = f128x4_loadu(p);
        if D == 0 {
            return a;
        }
        let b = f128x4_loadu(p.add(4));
        let (a0, a1) = split_pairs(a, b);
        let first = fold_pair(a0, a1, rho[0]);
        if D == 1 {
            return first;
        }
        let c = f128x4_loadu(p.add(8));
        let d = f128x4_loadu(p.add(12));
        let (b0, b1) = split_pairs(c, d);
        let second = fold_pair(b0, b1, rho[0]);
        let (c0, c1) = split_pairs(first, second);
        fold_pair(c0, c1, rho[1])
    }
}

/// [a+b, c+d, a+c, b+d] from [a,b,c,d].
#[inline(always)]
unsafe fn differences(v: __m512i) -> __m512i {
    // SAFETY: the module gate enables AVX-512F; indices are fixed register lanes.
    unsafe {
        let left = _mm512_permutexvar_epi64(_mm512_set_epi64(7, 6, 5, 4, 7, 6, 3, 2), v);
        let right = _mm512_permutexvar_epi64(_mm512_set_epi64(3, 2, 1, 0, 5, 4, 1, 0), v);
        _mm512_xor_si512(left, right)
    }
}

#[inline(always)]
unsafe fn total_difference(d: __m512i) -> __m512i {
    // SAFETY: the module gate enables AVX-512F; indices are fixed register lanes.
    unsafe {
        let a = _mm512_permutexvar_epi64(_mm512_set_epi64(1, 0, 1, 0, 1, 0, 1, 0), d);
        let b = _mm512_permutexvar_epi64(_mm512_set_epi64(3, 2, 3, 2, 3, 2, 3, 2), d);
        _mm512_xor_si512(a, b)
    }
}

fn grid<const D: usize>(
    l: &mut [Gf128],
    r: &mut [Gf128],
    pending: &[Gf128],
    suffix: &[Gf128],
    quads: usize,
) -> [Gf128; 9] {
    // SAFETY: the parent SumcheckKernels entry point validates all slice lengths,
    // and this module is gated on the complete instruction set. Each iteration
    // loads both quads before storing their folded prefixes. Prefix stores end
    // before the next unread source quad, including the first iteration.
    unsafe {
        let mut rho = [_mm512_setzero_si512(); 2];
        for (dst, &value) in rho.iter_mut().zip(pending) {
            *dst = broadcast(value);
        }
        // Lanes represent grid coordinates. Keeping just three wide vectors
        // avoids the register pressure of nine accumulators across four quads.
        let mut acc = [WideGhashX4::zero(); 3];
        for (b, &weight) in suffix[..quads].iter().enumerate() {
            let lv = folded4::<D>(l, 4 * b, &rho);
            let rv = folded4::<D>(r, 4 * b, &rho);
            if D != 0 {
                store4(l.as_mut_ptr().add(4 * b), lv);
                store4(r.as_mut_ptr().add(4 * b), rv);
            }
            let lw = ghash_mul_x4(lv, broadcast(weight));
            let ld = differences(lw);
            let rd = differences(rv);
            acc[0].mul_acc(lw, rv);
            acc[1].mul_acc(ld, rd);
            acc[2].mul_acc(total_difference(ld), total_difference(rd));
        }
        let mut values = [[Gf128::ZERO; 4]; 3];
        for (dst, a) in values.iter_mut().zip(acc) {
            store4(dst.as_mut_ptr(), a.reduce_lanes());
        }
        let [a, b, c, d] = values[0];
        let [ab, cd, ac, bd] = values[1];
        let total = values[2][0];
        // Same node-grid-to-coefficient conversion as the scalar grid kernel.
        [
            a,
            c,
            ac,
            a + b + ab,
            c + d + cd,
            ac + bd + total,
            ab,
            cd,
            total,
        ]
    }
}

pub(super) fn grid_pass(
    l: &mut [Gf128],
    r: &mut [Gf128],
    pending: &[Gf128],
    suffix: &[Gf128],
    quads: usize,
) -> [Gf128; 9] {
    match pending.len() {
        0 => grid::<0>(l, r, pending, suffix, quads),
        1 => grid::<1>(l, r, pending, suffix, quads),
        2 => grid::<2>(l, r, pending, suffix, quads),
        _ => unreachable!("grid pass supports at most two deferred folds"),
    }
}

pub(super) fn fold_in_place(v: &mut [Gf128], rho: &Gf128, half: usize) {
    // SAFETY: the parent validates at least 2*half inputs. Each four-output
    // block loads all eight inputs before its store; stores cannot overtake reads.
    unsafe {
        let challenge = [broadcast(*rho), _mm512_setzero_si512()];
        let bulk = half & !3;
        for i in (0..bulk).step_by(4) {
            let folded = folded4::<1>(v, i, &challenge);
            store4(v.as_mut_ptr().add(i), folded);
        }
        for i in bulk..half {
            v[i] = v[2 * i] + *rho * (v[2 * i] + v[2 * i + 1]);
        }
    }
}

/// Load four slots, optionally fold each pair of physical values, and return
/// separate vectors for each slot's logical even/odd entries.
#[inline(always)]
unsafe fn round_inputs<const D: usize>(
    v: &mut [Gf128],
    slot: usize,
    rho: &[__m512i; 2],
) -> (__m512i, __m512i) {
    // SAFETY: the round shape covers eight logical values starting at 2*slot.
    // Both halves load before either store, so prefix writes preserve unread data.
    unsafe {
        let a = folded4::<D>(v, 2 * slot, rho);
        let b = folded4::<D>(v, 2 * slot + 4, rho);
        if D != 0 {
            store4(v.as_mut_ptr().add(2 * slot), a);
            store4(v.as_mut_ptr().add(2 * slot + 4), b);
        }
        split_pairs(a, b)
    }
}

fn round<const D: usize>(
    l: &mut [Gf128],
    r: &mut [Gf128],
    rho: Gf128,
    w: &[Gf128],
    half: usize,
) -> (Gf128, Gf128, Gf128) {
    // SAFETY: validated entry point and the module's instruction-set gate.
    // Scalar tails consume the same public indices and preserve untouched suffixes.
    unsafe {
        let challenges = [broadcast(rho), _mm512_setzero_si512()];
        let mut acc = [WideGhashX4::zero(); 3];
        let bulk = half & !3;
        for b in (0..bulk).step_by(4) {
            let (l0, l1) = round_inputs::<D>(l, b, &challenges);
            let (r0, r1) = round_inputs::<D>(r, b, &challenges);
            let weight = f128x4_loadu(w.as_ptr().add(b));
            let l0w = ghash_mul_x4(l0, weight);
            let l1w = ghash_mul_x4(l1, weight);
            acc[0].mul_acc(l0w, r0);
            acc[1].mul_acc(l1w, r1);
            acc[2].mul_acc(_mm512_xor_si512(l0w, l1w), _mm512_xor_si512(r0, r1));
        }
        let [mut a0, mut a11, mut a2] = acc.map(|a| a.fold().reduce());
        for b in bulk..half {
            let fold = |v: &mut [Gf128], i: usize| {
                if D == 0 {
                    v[i]
                } else {
                    let result = v[2 * i] + rho * (v[2 * i] + v[2 * i + 1]);
                    v[i] = result;
                    result
                }
            };
            let l0 = fold(l, 2 * b) * w[b];
            let l1 = fold(l, 2 * b + 1) * w[b];
            let r0 = fold(r, 2 * b);
            let r1 = fold(r, 2 * b + 1);
            a0 += l0 * r0;
            a11 += l1 * r1;
            a2 += (l0 + l1) * (r0 + r1);
        }
        (a0, a11 + a0 + a2, a2)
    }
}

pub(super) fn fused_fold_round(
    l: &mut [Gf128],
    r: &mut [Gf128],
    rho: &Gf128,
    w: &[Gf128],
    half: usize,
) -> (Gf128, Gf128, Gf128) {
    round::<1>(l, r, *rho, w, half)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Gf128Ops, SumcheckKernels};

    fn sample(state: &mut u64) -> Gf128 {
        let mut word = || {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        };
        Gf128::new(word(), word())
    }

    #[test]
    fn avx512_grid_matches_scalar_at_every_fold_depth() {
        let mut state = 0x57863abeca2u64;
        for quads in [0, 1, 2, 3, 4, 5, 17, 64, 1025] {
            for depth in 0..=2 {
                for special in [
                    None,
                    Some(Gf128::ZERO),
                    Some(Gf128::ONE),
                    Some(Gf128::new(u64::MAX, u64::MAX)),
                ] {
                    let pending: Vec<_> = (0..depth)
                        .map(|_| special.unwrap_or_else(|| sample(&mut state)))
                        .collect();
                    let len = (4 * quads << depth) + 7;
                    let mut l: Vec<_> = (0..len).map(|_| sample(&mut state)).collect();
                    let mut r: Vec<_> = (0..len).map(|_| sample(&mut state)).collect();
                    let w: Vec<_> = (0..quads).map(|_| sample(&mut state)).collect();
                    let (mut expected_l, mut expected_r) = (l.clone(), r.clone());
                    let expected = super::super::x86::grid_pass(
                        &mut expected_l,
                        &mut expected_r,
                        &pending,
                        &w,
                        quads,
                    );
                    // Exercise the trait's actual dispatch, not just the AVX entry point.
                    assert_eq!(
                        Gf128Ops.eqf_grid_pass(&mut l, &mut r, &pending, &w, quads),
                        expected
                    );
                    assert_eq!(l, expected_l, "all L storage: quads={quads}, depth={depth}");
                    assert_eq!(r, expected_r, "all R storage: quads={quads}, depth={depth}");
                }
            }
        }
    }

    #[test]
    fn avx512_fused_round_and_fold_match_scalar_with_tails() {
        let mut state = 0x17557863abeca2u64;
        for half in [0, 1, 2, 3, 4, 5, 6, 7, 8, 17, 64, 1025] {
            for rho in [
                Gf128::ZERO,
                Gf128::ONE,
                Gf128::new(u64::MAX, u64::MAX),
                sample(&mut state),
            ] {
                let len = 4 * half + 7;
                let mut l: Vec<_> = (0..len).map(|_| sample(&mut state)).collect();
                let mut r: Vec<_> = (0..len).map(|_| sample(&mut state)).collect();
                let w: Vec<_> = (0..half).map(|_| sample(&mut state)).collect();
                let (mut expected_l, mut expected_r) = (l.clone(), r.clone());
                for values in [&mut expected_l, &mut expected_r] {
                    for i in 0..2 * half {
                        values[i] = values[2 * i] + rho * (values[2 * i] + values[2 * i + 1]);
                    }
                }
                let mut only_fold = l.clone();
                Gf128Ops.eqf_fold_in_place(&mut only_fold, &rho, 2 * half);
                assert_eq!(only_fold, expected_l, "fold: half={half}");
                let expected = Gf128Ops.eqf_single_pair_round(&expected_l, &expected_r, &w, half);
                assert_eq!(
                    Gf128Ops.eqf_fused_fold_round(&mut l, &mut r, &rho, &w, half),
                    expected
                );
                assert_eq!(l, expected_l, "fused L: half={half}");
                assert_eq!(r, expected_r, "fused R: half={half}");
            }
        }
    }
}
