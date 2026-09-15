//! Double-fold grid candidate. Arithmetic helpers below derive from the
//! production binary_gf128 NEON implementation. The new schedule separates
//! direct prefix folding from node accumulation and reduces once per pass.
use f2z::poly::univariate::binary_gf128::Gf128 as Gf;

pub(super) fn run<const TILE: usize>(l: &mut [Gf], r: &mut [Gf], p: &[Gf], w: &[Gf]) -> [Gf; 9] {
    assert!(TILE > 0);
    assert_eq!(p.len(), 2);
    assert_eq!(l.len(), 16 * w.len());
    assert_eq!(r.len(), l.len());
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    {
        neon::run::<TILE>(l, r, p, w)
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
    {
        portable(l, r, p, w)
    }
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
fn portable(l: &mut [Gf], r: &mut [Gf], p: &[Gf], w: &[Gf]) -> [Gf; 9] {
    use f2z::utils::wide_mul::WideMulAcc;
    let mut acc = [Gf::wide_zero(&Gf::zero()); 9];
    for (b, &w) in w.iter().enumerate() {
        let fold = |v: &[Gf]| -> [Gf; 4] {
            core::array::from_fn(|i| {
                let q = &v[16 * b + 4 * i..16 * b + 4 * i + 4];
                let a = q[0] + (q[1] + q[0]) * p[0];
                let c = q[2] + (q[3] + q[2]) * p[0];
                a + (a + c) * p[1]
            })
        };
        let a = fold(l);
        let c = fold(r);
        l[4 * b..4 * b + 4].copy_from_slice(&a);
        r[4 * b..4 * b + 4].copy_from_slice(&c);
        let grid = |a: [Gf; 4]| {
            [
                a[0],
                a[1],
                a[0] + a[1],
                a[2],
                a[3],
                a[2] + a[3],
                a[0] + a[2],
                a[1] + a[3],
                a[0] + a[1] + a[2] + a[3],
            ]
        };
        for ((s, a), c) in acc.iter_mut().zip(grid(a.map(|a| a * w))).zip(grid(c)) {
            Gf::wide_add_assign(s, &Gf::mul_wide(&a, &c));
        }
    }
    let e = acc.map(Gf::from_wide);
    core::array::from_fn(|i| {
        let base = (i % 3) * 3;
        match i / 3 {
            0 => e[base],
            2 => e[base + 2],
            _ => e[base] + e[base + 1] + e[base + 2],
        }
    })
}

#[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
mod neon {
    use super::Gf as BinaryFieldGF128;
    use core::arch::aarch64::*;
    fn clmul_64x64(a: u64, b: u64) -> [u64; 2] {
        let p = crate::arithmetic::clmul(a, b);
        [p as u64, (p >> 64) as u64]
    }
    #[inline(always)]
    pub(crate) unsafe fn pmull_lo(a: uint64x2_t, b: uint64x2_t) -> uint64x2_t {
        // SAFETY: caller guarantees NEON+AES (PMULL) are enabled (this
        // module only compiles with `target_feature = "neon"`, and the
        // build sets `-C target-cpu=native` on the PMULL targets — the
        // same contract as `clmul_64x64`). The lane-0 extracts feeding
        // `vmull_p64` compile to a single `pmull` on the vector regs.
        unsafe { vreinterpretq_u64_p128(vmull_p64(vgetq_lane_u64(a, 0), vgetq_lane_u64(b, 0))) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pmull_hi(a: uint64x2_t, b: uint64x2_t) -> uint64x2_t {
        // SAFETY: as `pmull_lo`.
        unsafe {
            vreinterpretq_u64_p128(vmull_high_p64(
                vreinterpretq_p64_u64(a),
                vreinterpretq_p64_u64(b),
            ))
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn fold_x64(
        t0: uint64x2_t,
        t1: uint64x2_t,
        g: uint64x2_t,
        z: uint64x2_t,
    ) -> uint64x2_t {
        // SAFETY: as `pmull_lo`.
        unsafe {
            let shifted = vextq_u64(z, t1, 1); // t1.lo · X^64
            let folded = pmull_hi(t1, g); // t1.hi ⊗ g (≤ 70 bits)
            veorq_u64(t0, veorq_u64(shifted, folded))
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn mul_fixed_wide(
        av: uint64x2_t,
        rl: uint64x2_t,
        rh: uint64x2_t,
    ) -> (uint64x2_t, uint64x2_t) {
        // SAFETY: as `pmull_lo`.
        unsafe {
            (
                veorq_u64(pmull_lo(av, rl), pmull_hi(av, rl)),
                veorq_u64(pmull_lo(av, rh), pmull_hi(av, rh)),
            )
        }
    }
    #[inline(always)]
    unsafe fn mul_fixed(
        av: uint64x2_t,
        rl: uint64x2_t,
        rh: uint64x2_t,
        g: uint64x2_t,
        z: uint64x2_t,
    ) -> uint64x2_t {
        // SAFETY: as `pmull_lo`.
        unsafe {
            let (tl, tm) = mul_fixed_wide(av, rl, rh);
            fold_x64(tl, tm, g, z)
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn clmul_256(a: uint64x2_t, b: uint64x2_t) -> (uint64x2_t, uint64x2_t) {
        // SAFETY: as `pmull_lo`; `vextq_u64`/`veorq_u64`/`vdupq_n_u64`
        // are plain NEON.
        unsafe {
            let t00 = pmull_lo(a, b); // a0·b0
            let t11 = pmull_hi(a, b); // a1·b1
            let bsw = vextq_u64(b, b, 1); // [b1, b0]
            let t01 = pmull_lo(a, bsw); // a0·b1
            let t10 = pmull_hi(a, bsw); // a1·b0
            let mid = veorq_u64(t01, t10);
            let z = vdupq_n_u64(0);
            let mid_lo = vextq_u64(z, mid, 1); // mid << 64
            let mid_hi = vextq_u64(mid, z, 1); // mid >> 64
            (veorq_u64(t00, mid_lo), veorq_u64(t11, mid_hi))
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn reduce_256(lo: uint64x2_t, hi: uint64x2_t) -> uint64x2_t {
        // SAFETY: as `pmull_lo`.
        unsafe {
            let g = vdupq_n_u64(0x87);
            // hi ⊗ g = x2·g + X^64·(x3·g): words w0 = y0, w1 = y1 ^ z0,
            // w2 = z1 (≤ 7 bits).
            let p0 = pmull_lo(hi, g); // x2·g = [y0, y1]
            let p1 = pmull_hi(hi, g); // x3·g = [z0, z1]
            let z = vdupq_n_u64(0);
            let p1_lo = vextq_u64(z, p1, 1); // [0, z0]
            let r = veorq_u64(lo, veorq_u64(p0, p1_lo));
            // word-2 overflow: w2 = z1; X^128·w2 ≡ w2·g (≤ 14 bits, word 0).
            let o = vextq_u64(p1, z, 1); // [z1, 0]
            veorq_u64(r, pmull_lo(o, g))
        }
    }
    #[inline(always)]
    unsafe fn fold1_fixed(
        rl: uint64x2_t,
        rh: uint64x2_t,
        g: uint64x2_t,
        z: uint64x2_t,
        v0: uint64x2_t,
        v1: uint64x2_t,
    ) -> uint64x2_t {
        // SAFETY: as `pmull_lo`.
        unsafe { veorq_u64(v0, mul_fixed(veorq_u64(v1, v0), rl, rh, g, z)) }
    }
    #[inline(always)]
    unsafe fn fold_quad_logical(
        v: &[BinaryFieldGF128],
        b: usize,
        d: usize,
        w1l: uint64x2_t,
        w1h: uint64x2_t,
        w2l: uint64x2_t,
        w2h: uint64x2_t,
        w3l: uint64x2_t,
        w3h: uint64x2_t,
        g: uint64x2_t,
        z: uint64x2_t,
    ) -> [uint64x2_t; 4] {
        // SAFETY: as `pmull_lo`; indices in bounds by the caller's
        // contract (`v` has `(quads ≪ 2) ≪ d` entries).
        unsafe {
            match d {
                0 => core::array::from_fn(|i| ld(&v[(b << 2) | i])),
                1 => core::array::from_fn(|i| {
                    let p = ((b << 2) | i) << 1;
                    fold1_fixed(w1l, w1h, g, z, ld(&v[p]), ld(&v[p + 1]))
                }),
                _ => core::array::from_fn(|i| {
                    let p = ((b << 2) | i) << 2;
                    let v0 = ld(&v[p]);
                    let v1 = ld(&v[p + 1]);
                    let v2 = ld(&v[p + 2]);
                    let v3 = ld(&v[p + 3]);
                    let d1 = veorq_u64(v1, v0);
                    let d2 = veorq_u64(v2, v0);
                    let d3 = veorq_u64(d1, veorq_u64(v3, v2));
                    let (mut tl, mut tm) = mul_fixed_wide(d1, w1l, w1h);
                    let (l2, m2) = mul_fixed_wide(d2, w2l, w2h);
                    let (l3, m3) = mul_fixed_wide(d3, w3l, w3h);
                    tl = veorq_u64(tl, veorq_u64(l2, l3));
                    tm = veorq_u64(tm, veorq_u64(m2, m3));
                    veorq_u64(v0, fold_x64(tl, tm, g, z))
                }),
            }
        }
    }
    #[inline(always)]
    unsafe fn node_grid(a: &[uint64x2_t; 4]) -> [uint64x2_t; 9] {
        // SAFETY: plain NEON XORs.
        unsafe {
            let d00 = veorq_u64(a[1], a[0]);
            let d01 = veorq_u64(a[3], a[2]);
            [
                a[0],
                a[1],
                d00,
                a[2],
                a[3],
                d01,
                veorq_u64(a[2], a[0]),
                veorq_u64(a[3], a[1]),
                veorq_u64(d01, d00),
            ]
        }
    }
    #[inline(always)]
    unsafe fn st(x: &mut BinaryFieldGF128, v: uint64x2_t) {
        // SAFETY: `out` is a valid 16-byte word pair.
        unsafe {
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), v);
            *x = BinaryFieldGF128::from_polynomial_words(out);
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn ld(x: &BinaryFieldGF128) -> uint64x2_t {
        // SAFETY: `uint.as_words()` is a valid 16-byte word pair.
        unsafe { vld1q_u64(x.as_words().as_ptr()) }
    }
    #[inline(always)]
    pub(crate) unsafe fn prep_fixed(rho: &BinaryFieldGF128) -> (uint64x2_t, uint64x2_t) {
        let w = rho.as_words();
        let rg = clmul_64x64(w[1], 0x87);
        let rl = [w[0], rg[0]];
        let rh = [w[1], w[0] ^ rg[1]];
        // SAFETY: valid 16-byte word pairs.
        unsafe { (vld1q_u64(rl.as_ptr()), vld1q_u64(rh.as_ptr())) }
    }
    // Each source quad is loaded in full before its four compacted stores.
    // Across tiles, unread source starts at 16*end, above every prefix write.
    #[inline(never)]
    unsafe fn fold_tile(
        l: &mut [BinaryFieldGF128],
        r: &mut [BinaryFieldGF128],
        start: usize,
        end: usize,
        prep: [(uint64x2_t, uint64x2_t); 3],
    ) {
        unsafe {
            let z = vdupq_n_u64(0);
            let g = vdupq_n_u64(0x87);
            for b in start..end {
                let a = fold_quad_logical(
                    l, b, 2, prep[0].0, prep[0].1, prep[1].0, prep[1].1, prep[2].0, prep[2].1, g, z,
                );
                let c = fold_quad_logical(
                    r, b, 2, prep[0].0, prep[0].1, prep[1].0, prep[1].1, prep[2].0, prep[2].1, g, z,
                );
                for i in 0..4 {
                    st(&mut l[4 * b + i], a[i]);
                    st(&mut r[4 * b + i], c[i]);
                }
            }
        }
    }
    #[inline(never)]
    unsafe fn accumulate(
        l: &[BinaryFieldGF128],
        r: &[BinaryFieldGF128],
        w: &[BinaryFieldGF128],
        acc: &mut [(uint64x2_t, uint64x2_t); 9],
    ) {
        unsafe {
            let z = vdupq_n_u64(0);
            let g = vdupq_n_u64(0x87);
            for ((l, r), w) in l.chunks_exact(4).zip(r.chunks_exact(4)).zip(w) {
                let (wl, wh) = prep_fixed(w);
                let a = core::array::from_fn(|i| mul_fixed(ld(&l[i]), wl, wh, g, z));
                let c = core::array::from_fn(|i| ld(&r[i]));
                for ((s, a), c) in acc.iter_mut().zip(node_grid(&a)).zip(node_grid(&c)) {
                    let (lo, hi) = clmul_256(a, c);
                    s.0 = veorq_u64(s.0, lo);
                    s.1 = veorq_u64(s.1, hi);
                }
            }
        }
    }
    pub(super) fn run<const TILE: usize>(
        l: &mut [BinaryFieldGF128],
        r: &mut [BinaryFieldGF128],
        p: &[BinaryFieldGF128],
        w: &[BinaryFieldGF128],
    ) -> [BinaryFieldGF128; 9] {
        // The outer wrapper establishes full slice bounds and two challenges.
        // AES-gated build supplies PMULL; helpers access complete word pairs.
        unsafe {
            let z = vdupq_n_u64(0);
            let mut acc = [(z, z); 9];
            let prep = [
                prep_fixed(&p[0]),
                prep_fixed(&p[1]),
                prep_fixed(&(p[0] * p[1])),
            ];
            for start in (0..w.len()).step_by(TILE) {
                let end = (start + TILE).min(w.len());
                fold_tile(l, r, start, end, prep);
                accumulate(
                    &l[4 * start..4 * end],
                    &r[4 * start..4 * end],
                    &w[start..end],
                    &mut acc,
                );
            }
            let e = acc.map(|(lo, hi)| reduce_256(lo, hi));
            core::array::from_fn(|i| {
                let base = (i % 3) * 3;
                let v = match i / 3 {
                    0 => e[base],
                    2 => e[base + 2],
                    _ => veorq_u64(e[base], veorq_u64(e[base + 1], e[base + 2])),
                };
                let mut out = [0; 2];
                vst1q_u64(out.as_mut_ptr(), v);
                BinaryFieldGF128::from_polynomial_words(out)
            })
        }
    }
}
