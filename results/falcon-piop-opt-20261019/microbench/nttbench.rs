//! Scalar vs restructured negacyclic NTT product over P = 15*2^27+1 (N = 512),
//! mirroring keyed_ring::exact, plus the 11-tap quotient batching.
use std::hint::black_box;
use std::time::Instant;

const N: usize = 512;
const P: u32 = 0x7800_0001;
const Q: i64 = 12_289;
const GENERATOR: u64 = 31;
const fn negative_inverse() -> u32 {
    let mut inverse: u32 = 1;
    let mut i = 0;
    while i < 5 { inverse = inverse.wrapping_mul(2u32.wrapping_sub(P.wrapping_mul(inverse))); i += 1; }
    inverse.wrapping_neg()
}
const NEGATIVE_INVERSE: u32 = negative_inverse();
#[inline(always)]
fn mul(a: u32, b: u32) -> u32 {
    let t = u64::from(a) * u64::from(b);
    let m = (t as u32).wrapping_mul(NEGATIVE_INVERSE);
    let u = ((t + u64::from(m) * u64::from(P)) >> 32) as u32;
    if u >= P { u - P } else { u }
}
#[inline(always)]
fn add(a: u32, b: u32) -> u32 { let s = a + b; if s >= P { s - P } else { s } }
#[inline(always)]
fn sub(a: u32, b: u32) -> u32 { if a >= b { a - b } else { a + P - b } }
// branchless variants
#[inline(always)]
fn mul_bl(a: u32, b: u32) -> u32 {
    let t = u64::from(a) * u64::from(b);
    let m = (t as u32).wrapping_mul(NEGATIVE_INVERSE);
    let u = ((t + u64::from(m) * u64::from(P)) >> 32) as u32;
    u.min(u.wrapping_sub(P))
}
#[inline(always)]
fn add_bl(a: u32, b: u32) -> u32 { let s = a + b; s.min(s.wrapping_sub(P)) }
#[inline(always)]
fn sub_bl(a: u32, b: u32) -> u32 { let d = a.wrapping_sub(b); d.min(d.wrapping_add(P)) }

fn pow(base: u64, mut e: u64) -> u64 { let m = u64::from(P); let (mut r, mut b) = (1u64, base % m); while e > 0 { if e & 1 == 1 { r = r * b % m; } b = b * b % m; e >>= 1; } r }

struct Tables { zetas: Vec<u32>, scale: u32 }
impl Tables {
    fn new() -> Self {
        let m = u64::from(P); let r = (1u64 << 32) % m; let r2 = ((1u128 << 64) % u128::from(P)) as u64;
        let psi = pow(GENERATOR, (m - 1) / (2 * N as u64));
        assert_eq!(pow(psi, N as u64), m - 1);
        let bits = N.ilog2();
        let zetas = (0..N).map(|k| { let rev = (k as u64).reverse_bits() >> (64 - bits); (pow(psi, rev) * r % m) as u32 }).collect();
        let n_inv = pow(N as u64, m - 2);
        Self { zetas, scale: (n_inv * r2 % m) as u32 }
    }
    fn forward(&self, a: &mut [u32; N]) {
        let mut k = 0; let mut len = N / 2;
        while len > 0 { let mut start = 0; while start < N { k += 1; let zeta = self.zetas[k];
            for j in start..start + len { let t = mul(zeta, a[j + len]); a[j + len] = sub(a[j], t); a[j] = add(a[j], t); }
            start += 2 * len; } len /= 2; }
    }
    fn inverse(&self, a: &mut [u32; N]) {
        let mut k = N; let mut len = 1;
        while len < N { let mut start = 0; while start < N { k -= 1; let zeta = P - self.zetas[k];
            for j in start..start + len { let t = a[j]; a[j] = add(t, a[j + len]); a[j + len] = mul(zeta, sub(t, a[j + len])); }
            start += 2 * len; } len *= 2; }
        for v in a.iter_mut() { *v = mul(self.scale, *v); }
    }
    // Branchless + chunked: the j-loop is a plain slice loop over two halves so LLVM can vectorize it.
    fn forward_v(&self, a: &mut [u32; N]) {
        let mut k = 0; let mut len = N / 2;
        while len > 0 { let mut start = 0; while start < N { k += 1; let zeta = self.zetas[k];
            let (lo, hi) = a[start..start + 2 * len].split_at_mut(len);
            for (x, y) in lo.iter_mut().zip(hi.iter_mut()) { let t = mul_bl(zeta, *y); *y = sub_bl(*x, t); *x = add_bl(*x, t); }
            start += 2 * len; } len /= 2; }
    }
    fn inverse_v(&self, a: &mut [u32; N]) {
        let mut k = N; let mut len = 1;
        while len < N { let mut start = 0; while start < N { k -= 1; let zeta = P - self.zetas[k];
            let (lo, hi) = a[start..start + 2 * len].split_at_mut(len);
            for (x, y) in lo.iter_mut().zip(hi.iter_mut()) { let t = *x; *x = add_bl(t, *y); *y = mul_bl(zeta, sub_bl(t, *y)); }
            start += 2 * len; } len *= 2; }
        for v in a.iter_mut() { *v = mul_bl(self.scale, *v); }
    }
    fn product(&self, h: &[u16; N], s: &[i16; N], out: &mut [u32; N], vec: bool) {
        let mut right = [0u32; N];
        for ((l, r), (&h, &s)) in out.iter_mut().zip(&mut right).zip(h.iter().zip(s)) { *l = u32::from(h); *r = if s < 0 { P - (-i32::from(s)) as u32 } else { s as u32 }; }
        if vec { self.forward_v(out); self.forward_v(&mut right); for (l, r) in out.iter_mut().zip(&right) { *l = mul_bl(*l, *r); } self.inverse_v(out); }
        else { self.forward(out); self.forward(&mut right); for (l, r) in out.iter_mut().zip(&right) { *l = mul(*l, *r); } self.inverse(out); }
    }
}

fn rnd(s: &mut u64) -> u64 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s }

fn main() {
    let t = Tables::new();
    let sigs = 4096usize;
    let mut seed = 0x1234_5678_9abc_def1u64;
    let hs: Vec<[u16; N]> = (0..sigs).map(|_| std::array::from_fn(|_| (rnd(&mut seed) % Q as u64) as u16)).collect();
    let ss: Vec<[i16; N]> = (0..sigs).map(|_| std::array::from_fn(|_| ((rnd(&mut seed) % 8193) as i16) - 4096)).collect();
    let polys: Vec<[u16; 11]> = (0..sigs).map(|_| std::array::from_fn(|_| (rnd(&mut seed) % Q as u64) as u16)).collect();
    let mut out = [0u32; N];
    // correctness of the vectorized variant
    let mut o1 = [0u32; N]; let mut o2 = [0u32; N];
    t.product(&hs[0], &ss[0], &mut o1, false); t.product(&hs[0], &ss[0], &mut o2, true);
    assert_eq!(o1, o2, "variants differ");
    for (label, vec) in [("product scalar (3 NTT + pointwise)", false), ("product branchless/chunked", true)] {
        let mut best = f64::MAX;
        for _ in 0..3 { let t0 = Instant::now(); for i in 0..sigs { t.product(&hs[i], &ss[i], &mut out, vec); black_box(&out); } best = best.min(t0.elapsed().as_nanos() as f64 / sigs as f64); }
        println!("{label:<44} {:8.0} ns/signature  ({:.1} ms at 32768)", best, best * 32768.0 / 1e6);
    }
    // taps: K[a + m] += A[a] * k[m], 11 taps, i32 x i32 -> i64
    let ks: Vec<[i32; N]> = (0..sigs).map(|_| std::array::from_fn(|_| ((rnd(&mut seed) % (1 << 26)) as i32) - (1 << 25))).collect();
    let mut sum = vec![0i64; N + 10];
    let mut best = f64::MAX;
    for _ in 0..3 { let t0 = Instant::now(); for i in 0..sigs { for (a, &c) in polys[i].iter().enumerate() { let c = i32::from(c); for (total, &v) in sum[a..a + N].iter_mut().zip(ks[i].iter()) { *total += i64::from(c) * i64::from(v); } } } black_box(&sum); best = best.min(t0.elapsed().as_nanos() as f64 / sigs as f64); }
    println!("{:<44} {:8.0} ns/signature  ({:.1} ms at 32768)", "taps, per-tap slice loop (current)", best, best * 32768.0 / 1e6);
    // taps transposed: for each output position, dot of 11 taps (reads k[m-a])
    let mut best = f64::MAX;
    for _ in 0..3 { let t0 = Instant::now(); for i in 0..sigs {
        let k = &ks[i]; let c: [i64; 11] = std::array::from_fn(|a| i64::from(polys[i][a]));
        // interior positions m in 10..N: all 11 taps valid
        for m in 10..N { let mut acc = 0i64; for a in 0..11 { acc += c[a] * i64::from(k[m - a]); } sum[m] += acc; }
        for m in 0..10 { let mut acc = 0i64; for a in 0..=m { acc += c[a] * i64::from(k[m - a]); } sum[m] += acc; }
        for m in N..N + 10 { let mut acc = 0i64; for a in (m - N + 1)..11 { acc += c[a] * i64::from(k[m - a]); } sum[m] += acc; }
    } black_box(&sum); best = best.min(t0.elapsed().as_nanos() as f64 / sigs as f64); }
    println!("{:<44} {:8.0} ns/signature  ({:.1} ms at 32768)", "taps, transposed 11-tap dot per position", best, best * 32768.0 / 1e6);
}
