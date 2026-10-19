use field::{BatchMulAcc, Fp, FpCtx, FpLinearAcc, FpProductAcc, IntegerEmbedding, Reduce, RingOps, Uint, WideMul, create_prime_field};
use std::hint::black_box;
use std::time::Instant;

type F = Fp<2>;

fn rnd(state: &mut u64) -> u64 {
    *state ^= *state << 13; *state ^= *state >> 7; *state ^= *state << 17; *state
}
fn elem(f: &FpCtx<2>, s: &mut u64) -> F {
    let lo = rnd(s); let hi = rnd(s) >> 20; // < 2^108
    let v = (u128::from(hi) << 64) | u128::from(lo);
    IntegerEmbedding::<u128>::from_integer(f, &v)
}
fn time<R>(label: &str, items: usize, mut body: impl FnMut() -> R) {
    // warm-up + ~3 reps
    body();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        let r = body();
        black_box(r);
        let ns = t.elapsed().as_nanos() as f64;
        best = best.min(ns / items as f64);
    }
    println!("{label:<48} {best:6.2} ns/item");
}

fn main() {
    // the Falcon "small prime" family is 2^111 <= p < 2^112; use a prime there.
    // 2^111 + 65 ... we just need an odd probable prime; pick via trial: use known prime 2^112 - 75? Use 2^127-1 if lookup fails.
    let candidates: [u128; 3] = [(1u128 << 112) - 75, (1u128 << 112) - 143, (1u128 << 127) - 1];
    let mut cfg = None;
    for c in candidates {
        let u = Uint::<2>::from(c);
        if field::is_probable_prime_public(&u) { cfg = Some((c, create_prime_field(u))); break; }
    }
    let (p, f) = cfg.expect("prime");
    println!("modulus bits {}  (p = 2^{:.0} - ...)", f.modulus_bits(), (p as f64).log2());
    let n = 1usize << 22;
    let mut s = 0x9e3779b97f4a7c15u64;
    let a: Vec<F> = (0..n).map(|_| elem(&f, &mut s)).collect();
    let b: Vec<F> = (0..n).map(|_| elem(&f, &mut s)).collect();
    let small: Vec<u64> = (0..n).map(|_| rnd(&mut s) & 0x7fff).collect();
    let r = elem(&f, &mut s);
    let x = elem(&f, &mut s);

    // 1. independent multiplications (throughput)
    let mut c = vec![f.zero(); n];
    time("mul, independent (throughput)", n, || {
        for i in 0..n { c[i] = f.mul(&a[i], &b[i]); }
        c[n - 1]
    });
    // 2. one dependent chain: v = x*v - carry (latency bound), like the keyed-ring recurrence
    time("mul chain, 1 stream (latency)", n, || {
        let mut v = a[0];
        for i in 0..n { v = f.sub(&f.mul(&x, &v), &b[i]); }
        v
    });
    // 3. eight interleaved independent chains
    time("mul chain, 8 interleaved streams", n, || {
        let mut v: [F; 8] = std::array::from_fn(|k| a[k]);
        for i in (0..n).step_by(8) {
            for k in 0..8 { v[k] = f.sub(&f.mul(&x, &v[k]), &b[i + k]); }
        }
        v[0]
    });
    // 4. lerp fold in place over pairs (binder dense_fold shape, stride 1): one mul + 2 add/sub + 32B load/16B store
    let mut pairs: Vec<[F; 2]> = (0..n).map(|i| [a[i], b[i]]).collect();
    time("lerp fold, [Lambda,T] pairs, stride 1 (2 lerps/pair)", n / 2, || {
        for pr in pairs.chunks_exact_mut(2) {
            let l = pr[0]; let h = pr[1];
            pr[0] = [f.add(&l[0], &f.mul(&r, &f.sub(&h[0], &l[0]))), f.add(&l[1], &f.mul(&r, &f.sub(&h[1], &l[1])))];
        }
        pairs[0]
    });
    // 5. full-width mul_acc (binder dense_sums shape)
    time("mul_acc F x F into FpProductAcc", n, || {
        let mut acc = FpProductAcc::<2>::default();
        for i in 0..n { f.mul_acc(&mut acc, &a[i], &b[i]); }
        f.reduce(acc)
    });
    // 6. F x small (u64) mul_acc (native first-round shape)
    time("mul_acc F x u64(15-bit) into FpLinearAcc", n, || {
        let mut acc = FpLinearAcc::<2, 1>::default();
        for i in 0..n { f.mul_acc(&mut acc, &a[i], &small[i]); }
        f.reduce(acc)
    });
    // 7. add only (baseline)
    time("add only", n, || {
        let mut v = a[0];
        for i in 0..n { v = f.add(&v, &b[i]); }
        v
    });

    // --- keyed-ring recurrence variants: value = x*value - wrap*h[j], 512 steps per chain ---
    let hs: Vec<u64> = (0..n).map(|_| rnd(&mut s) % 12289).collect();
    let residues: Vec<F> = (0..12289u64).map(|v| IntegerEmbedding::<u64>::from_integer(&f, &v)).collect();
    let chains = n / 512;
    time("recurrence, 1 chain at a time, mul(wrap, residues[h])", n, || {
        let mut acc = f.zero();
        for c in 0..chains {
            let mut v = a[c]; let wrap = b[c];
            for j in 0..512 { let carry = f.mul(&wrap, &residues[hs[c*512+j] as usize]); v = f.sub(&f.mul(&x, &v), &carry); }
            acc = f.add(&acc, &v);
        }
        acc
    });
    time("recurrence, 1 chain at a time, reduce(mul_wide(wrap, h))", n, || {
        let mut acc = f.zero();
        for c in 0..chains {
            let mut v = a[c]; let wrap = b[c];
            for j in 0..512 { let carry = f.reduce(f.mul_wide(&wrap, &hs[c*512+j])); v = f.sub(&f.mul(&x, &v), &carry); }
            acc = f.add(&acc, &v);
        }
        acc
    });
    time("recurrence, 8 chains const-lanes, mul(wrap, residues[h])", n, || {
        let mut acc = f.zero();
        for g in 0..chains / 8 {
            let mut v: [F; 8] = std::array::from_fn(|k| a[g*8+k]);
            let wrap: [F; 8] = std::array::from_fn(|k| b[g*8+k]);
            for j in 0..512 { for k in 0..8 { let carry = f.mul(&wrap[k], &residues[hs[(g*8+k)*512+j] as usize]); v[k] = f.sub(&f.mul(&x, &v[k]), &carry); } }
            for k in 0..8 { acc = f.add(&acc, &v[k]); }
        }
        acc
    });
    time("recurrence, 8 chains const-lanes, reduce(mul_wide)", n, || {
        let mut acc = f.zero();
        for g in 0..chains / 8 {
            let mut v: [F; 8] = std::array::from_fn(|k| a[g*8+k]);
            let wrap: [F; 8] = std::array::from_fn(|k| b[g*8+k]);
            for j in 0..512 { for k in 0..8 { let carry = f.reduce(f.mul_wide(&wrap[k], &hs[(g*8+k)*512+j])); v[k] = f.sub(&f.mul(&x, &v[k]), &carry); } }
            for k in 0..8 { acc = f.add(&acc, &v[k]); }
        }
        acc
    });
    let lanes = black_box(8usize);
    time("recurrence, runtime lanes=8 + strided stores, reduce(mul_wide)", n, || {
        let mut out = vec![f.zero(); 8 * 512];
        let mut acc = f.zero();
        for g in 0..chains / 8 {
            let mut v = [f.zero(); 8]; let mut wrap = [f.zero(); 8];
            for k in 0..lanes { v[k] = a[g*8+k]; wrap[k] = b[g*8+k]; }
            for j in 0..512 { for k in 0..lanes { out[k*512 + j] = v[k]; if j + 1 < 512 { let carry = f.reduce(f.mul_wide(&wrap[k], &hs[(g*8+k)*512+j])); v[k] = f.sub(&f.mul(&x, &v[k]), &carry); } } }
            acc = f.add(&acc, &out[511]);
        }
        acc
    });
    time("recurrence, 8 const lanes + strided stores, mul(residues)", n, || {
        let mut out = vec![f.zero(); 8 * 512];
        let mut acc = f.zero();
        for g in 0..chains / 8 {
            let mut v: [F; 8] = std::array::from_fn(|k| a[g*8+k]);
            let wrap: [F; 8] = std::array::from_fn(|k| b[g*8+k]);
            for j in 0..512 { for k in 0..8 { out[k*512 + j] = v[k]; if j + 1 < 512 { let carry = f.mul(&wrap[k], &residues[hs[(g*8+k)*512+j] as usize]); v[k] = f.sub(&f.mul(&x, &v[k]), &carry); } } }
            acc = f.add(&acc, &out[511]);
        }
        acc
    });
}
