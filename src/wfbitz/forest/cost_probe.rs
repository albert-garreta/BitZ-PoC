//! Cost probes for the grand product (ignored tests; run one at a time with
//! `--ignored --nocapture` on a release build, `-C target-cpu=native`,
//! `--features bitz-parity,parallel`):
//!
//! - [`probe_ghash_mul_costs`]: one GHASH multiplication in each form the
//!   prover uses — reduced (6 PMULL), by a pass-fixed preprocessed scalar
//!   (5), unreduced into a 256-bit accumulator (4), the Gruen slot (two
//!   reduced + two unreduced) and the fused fold-and-round column — as a
//!   dependent chain (latency) and over independent L1-resident operands
//!   (throughput), plus independent products streamed from DRAM; at one
//!   thread and on every core;
//! - [`probe_table_read_sweep`]: one data-dependent 16-byte table read, and
//!   one bucket read-modify-write, as the table outgrows L1, L2 and the SLC;
//! - [`probe_wfbitz_table_reads`]: the table-driven kernels of the grand
//!   product (level 0's bit rounds, JIT round and JIT fold, and the level-4
//!   product pass) on a random instance at the scheme's split, each rerun
//!   with its table reads, its multiplications or its bucket scatters
//!   knocked out, so the time differences price each part in place.
//!
//! Knobs: `COST_THREADS` (default `1,10`), `COST_SHAPES` (`t:s` pairs,
//! default `14:10,15:11,16:12`, i.e. n = 24, 26, 28 at `t = ⌈0.6n⌉ − 1`),
//! `COST_REPS` (default 7). Times are medians of the reps after one
//! untimed warm-up call. Every result line starts with `COST `.

use core::arch::aarch64::{uint64x2_t, vdupq_n_u64, veorq_u64, vextq_u64, vld1q_u64, vst1q_u64};
use std::hint::black_box;
use std::mem::MaybeUninit;
use std::time::Instant;

use field::gf128::neon::{
    clmul_256, fold_x64, ld, mul_fixed_wide, pmull_hi, pmull_lo, prep_fixed, reduce_256,
};

use super::*;

/// The kernel as the prover runs it.
const PROD: u8 = 0;
/// Table reads replaced by a register value built from the same index.
const NOREAD: u8 = 1;
/// Bucket scatters replaced by XORs into registers.
const NOSCATTER: u8 = 2;
/// Multiplications replaced by XORs of the same operands.
const NOMUL: u8 = 3;
/// Index derivation and table reads only, XOR-accumulated.
const READONLY: u8 = 4;
/// Index derivation only (the pattern bytes XOR-accumulated).
const TRAVERSE: u8 = 5;
/// The bit gathers and transposes only: each pattern block folded into a
/// register eight bytes at a time, no per-column work at all.
const PATTERNS: u8 = 6;

fn mode_name(mode: u8) -> &'static str {
    match mode {
        PROD => "prod",
        NOREAD => "noread",
        NOSCATTER => "noscatter",
        NOMUL => "nomul",
        READONLY => "readonly",
        TRAVERSE => "traverse",
        _ => "patterns",
    }
}

type Acc = (uint64x2_t, uint64x2_t);

#[inline(always)]
fn vzero() -> uint64x2_t {
    // SAFETY: a plain NEON constant.
    unsafe { vdupq_n_u64(0) }
}

#[inline(always)]
fn dup(x: usize) -> uint64x2_t {
    // SAFETY: a plain NEON broadcast.
    unsafe { vdupq_n_u64(x as u64) }
}

#[inline(always)]
fn xor(a: uint64x2_t, b: uint64x2_t) -> uint64x2_t {
    // SAFETY: a plain NEON XOR.
    unsafe { veorq_u64(a, b) }
}

#[inline(always)]
fn acc_add(a: &mut Acc, p: Acc) {
    a.0 = xor(a.0, p.0);
    a.1 = xor(a.1, p.1);
}

#[inline(always)]
fn to_gf(v: uint64x2_t) -> Gf {
    let mut out = [0u64; 2];
    // SAFETY: `out` is a valid 16-byte word pair.
    unsafe { vst1q_u64(out.as_mut_ptr(), v) };
    Gf::from_polynomial_words(out)
}

fn acc_to_gf(a: Acc) -> Gf {
    to_gf(reduce_256(a.0, a.1))
}

fn accs_to_gf(a: &[uint64x2_t; 4]) -> Gf {
    to_gf(xor(xor(a[0], a[1]), xor(a[2], a[3])))
}

/// The XOR of a pattern block's eight words.
#[inline(always)]
fn fold_block(block: &[u8; 64]) -> u64 {
    let mut x = 0u64;
    for c in block.chunks_exact(8) {
        x ^= u64::from_le_bytes(c.try_into().expect("8 bytes"));
    }
    x
}

#[inline(always)]
fn st_uninit(x: &mut MaybeUninit<Gf>, v: uint64x2_t) {
    x.write(to_gf(v));
}

/// The prover's reduced multiply ([`kernels`]'s `mul_red`): 4-PMULL
/// schoolbook, two `fold_x64` stages — 6 PMULLs.
#[inline(always)]
unsafe fn mul_red(a: uint64x2_t, b: uint64x2_t, g: uint64x2_t, z: uint64x2_t) -> uint64x2_t {
    // SAFETY: as `neon::pmull_lo`.
    unsafe {
        let t00 = pmull_lo(a, b);
        let t11 = pmull_hi(a, b);
        let bsw = vextq_u64(b, b, 1);
        let mid = veorq_u64(pmull_lo(a, bsw), pmull_hi(a, bsw));
        let t1 = fold_x64(mid, t11, g, z);
        fold_x64(t00, t1, g, z)
    }
}

/// `v0 + rho·(v1 − v0)` with `rho` preprocessed — 5 PMULLs.
#[inline(always)]
unsafe fn fold1(
    rl: uint64x2_t,
    rh: uint64x2_t,
    g: uint64x2_t,
    z: uint64x2_t,
    v0: uint64x2_t,
    v1: uint64x2_t,
) -> uint64x2_t {
    // SAFETY: as `neon::pmull_lo`.
    unsafe {
        let (tl, tm) = mul_fixed_wide(veorq_u64(v1, v0), rl, rh);
        veorq_u64(v0, fold_x64(tl, tm, g, z))
    }
}

/// The Gruen slot with `send_one = false`: `end += (w·l0)⊗r0`,
/// `inf += (w·(l1⊕l0))⊗(r1⊕r0)` — two reduced and two unreduced products.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
unsafe fn slot(
    w: uint64x2_t,
    l0: uint64x2_t,
    l1: uint64x2_t,
    r0: uint64x2_t,
    r1: uint64x2_t,
    g: uint64x2_t,
    z: uint64x2_t,
    end: &mut Acc,
    inf: &mut Acc,
) {
    // SAFETY: as `neon::pmull_lo`.
    unsafe {
        let el = mul_red(w, l0, g, z);
        acc_add(end, clmul_256(el, r0));
        let ed = mul_red(w, veorq_u64(l1, l0), g, z);
        acc_add(inf, clmul_256(ed, veorq_u64(r1, r0)));
    }
}

// ---------------------------------------------------------------------
// Harness.
// ---------------------------------------------------------------------

fn knob_list(name: &str, default: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn threads_list() -> Vec<usize> {
    knob_list("COST_THREADS", "1,10").iter().map(|x| x.parse().expect("COST_THREADS")).collect()
}

fn reps() -> usize {
    std::env::var("COST_REPS").ok().and_then(|x| x.parse().ok()).unwrap_or(7)
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("pool")
}

/// Median seconds of `reps` calls after one untimed warm-up call.
fn time_it(reps: usize, mut f: impl FnMut()) -> f64 {
    f();
    let mut v: Vec<f64> = (0..reps)
        .map(|_| {
            let started = Instant::now();
            f();
            started.elapsed().as_secs_f64()
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    v[v.len() / 2]
}

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn elems(n: usize, seed: u64) -> Vec<Gf> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n).map(|_| Gf::new(xorshift(&mut state), xorshift(&mut state))).collect()
}

/// Clock of the core running the calling thread, in GHz: 16 dependent
/// integer adds per loop iteration (one cycle each on these cores), the
/// loop counter on a parallel chain; best of five.
fn clock_ghz() -> f64 {
    const ITERS: u64 = 10_000_000;
    let mut best = f64::MAX;
    for _ in 0..5 {
        let mut x: u64 = 0;
        let mut n: u64 = ITERS;
        let started = Instant::now();
        // SAFETY: register-only arithmetic and a local backward branch.
        unsafe {
            core::arch::asm!(
                "2:",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "add {x}, {x}, #1",
                "subs {n}, {n}, #1",
                "b.ne 2b",
                x = inout(reg) x,
                n = inout(reg) n,
                options(nomem, nostack),
            );
        }
        let elapsed = started.elapsed().as_secs_f64();
        black_box((x, n));
        best = best.min(elapsed);
    }
    16.0 * ITERS as f64 / best / 1e9
}

/// Runs `f` once on every thread of `pool` (concurrently) and returns the
/// per-thread results in thread order.
fn on_every_thread<T: Send>(pool: &rayon::ThreadPool, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    pool.broadcast(|ctx| f(ctx.index()))
}

fn fmt_list(v: &[f64]) -> String {
    let parts: Vec<String> = v.iter().map(|x| format!("{x:.3}")).collect();
    parts.join("/")
}

// ---------------------------------------------------------------------
// Probe 1: GHASH multiplications.
// ---------------------------------------------------------------------

#[inline(never)]
fn chain_red(x0: Gf, y: Gf, iters: usize) -> Gf {
    // SAFETY: as `neon::pmull_lo`.
    unsafe {
        let g = vdupq_n_u64(0x87);
        let z = vzero();
        let yv = ld(&y);
        let mut x = ld(&x0);
        for _ in 0..iters {
            x = mul_red(x, yv, g, z);
        }
        to_gf(x)
    }
}

#[inline(never)]
fn chain_op(x0: Gf, y: Gf, iters: usize) -> Gf {
    let mut x = x0;
    for _ in 0..iters {
        x = x * y;
    }
    x
}

#[inline(never)]
fn batch_op(a: &[Gf], b: &[Gf], out: &mut [Gf]) {
    for ((o, x), y) in out.iter_mut().zip(a).zip(b) {
        *o = *x * *y;
    }
}

/// `out[i] = v0[i] + rho·(v1[i] − v0[i])`, two independent columns per
/// iteration — the prover's pass-fixed fold.
#[inline(never)]
fn fixed_batch(v0: &[Gf], v1: &[Gf], rho: Gf, out: &mut [MaybeUninit<Gf>]) {
    let n = out.len();
    assert!(v0.len() >= n && v1.len() >= n && n % 2 == 0);
    // SAFETY: as `neon::pmull_lo`; indices below `n`.
    unsafe {
        let (rl, rh) = prep_fixed(&rho);
        let g = vdupq_n_u64(0x87);
        let z = vzero();
        let mut i = 0;
        while i < n {
            let p0 = fold1(rl, rh, g, z, ld(v0.get_unchecked(i)), ld(v1.get_unchecked(i)));
            let p1 = fold1(rl, rh, g, z, ld(v0.get_unchecked(i + 1)), ld(v1.get_unchecked(i + 1)));
            st_uninit(out.get_unchecked_mut(i), p0);
            st_uninit(out.get_unchecked_mut(i + 1), p1);
            i += 2;
        }
    }
}

/// The L1-resident GHASH kernels on the calling thread: `(name, ns per
/// multiplication or term)`.
fn ghash_kernels_here(reps: usize, seed: u64) -> Vec<(&'static str, f64)> {
    const N: usize = 512; // 8 KB per operand vector
    const PASSES: usize = 2048; // 2^20 operations per timed call
    let ops = N * PASSES;
    let per = |secs: f64, ops: usize| secs * 1e9 / ops as f64;
    let a = elems(N, seed + 1);
    let b = elems(N, seed + 2);
    let c = elems(N, seed + 3);
    let d = elems(N, seed + 4);
    let w = elems(N, seed + 5);
    let rho = elems(1, seed + 6)[0];
    let mut out = vec![MaybeUninit::new(Gf::zero()); N];
    let mut outg = vec![Gf::zero(); N];
    let mut res = Vec::new();

    let chain = 1usize << 20;
    let t = time_it(reps, || {
        black_box(chain_red(black_box(a[0]), black_box(b[0]), chain));
    });
    res.push(("mul_red chain (latency)", per(t, chain)));
    let t = time_it(reps, || {
        black_box(chain_op(black_box(a[0]), black_box(b[0]), chain));
    });
    res.push(("Gf*Gf chain (latency)", per(t, chain)));

    let t = time_it(reps, || {
        for _ in 0..PASSES {
            kernels::product_into(black_box(&a), black_box(&b), &mut out);
            black_box(&mut out);
        }
    });
    res.push(("mul_red batch (product_into)", per(t, ops)));
    let t = time_it(reps, || {
        for _ in 0..PASSES {
            batch_op(black_box(&a), black_box(&b), &mut outg);
            black_box(&mut outg);
        }
    });
    res.push(("Gf*Gf batch", per(t, ops)));
    let t = time_it(reps, || {
        for _ in 0..PASSES {
            fixed_batch(black_box(&a), black_box(&b), rho, &mut out);
            black_box(&mut out);
        }
    });
    res.push(("fixed-scalar fold batch", per(t, ops)));
    let t = time_it(reps, || {
        let mut acc = Gf::zero();
        for _ in 0..PASSES {
            acc += kernels::dot(black_box(&a), black_box(&b));
        }
        black_box(acc);
    });
    res.push(("unreduced product+XOR (dot)", per(t, ops)));
    let t = time_it(reps, || {
        let mut acc = Gf::zero();
        for _ in 0..PASSES {
            let (x, y) = kernels::neon::round_sums_task(
                black_box(&a),
                black_box(&b),
                black_box(&c),
                black_box(&d),
                black_box(&w),
                false,
            );
            acc += x + y;
        }
        black_box(acc);
    });
    res.push(("Gruen slot (2 red + 2 unred)", per(t, ops)));
    let (mut q0l, mut q1l, q2l, q3l) = (elems(N, seed + 20), elems(N, seed + 21), elems(N, seed + 22), elems(N, seed + 23));
    let (mut q0r, mut q1r, q2r, q3r) = (elems(N, seed + 24), elems(N, seed + 25), elems(N, seed + 26), elems(N, seed + 27));
    let t = time_it(reps, || {
        let mut acc = Gf::zero();
        for _ in 0..PASSES {
            let (x, y) = kernels::neon::fused_task(
                &mut q0l, &mut q1l, &q2l, &q3l, &mut q0r, &mut q1r, &q2r, &q3r, &rho, &w, false,
            );
            acc += x + y;
        }
        black_box(acc);
    });
    res.push(("fused fold+round column (4 fixed + slot)", per(t, ops)));
    res
}

#[test]
#[ignore]
fn probe_ghash_mul_costs() {
    let reps = reps();
    for threads in threads_list() {
        let pool = pool(threads);
        let clocks = on_every_thread(&pool, |_| clock_ghz());
        println!("COST probe=clock threads={threads} ghz={}", fmt_list(&clocks));
        // Every thread runs the L1 kernels concurrently on its own operands.
        let per_thread = on_every_thread(&pool, |i| ghash_kernels_here(reps, 1000 * i as u64 + 7));
        let names: Vec<&str> = per_thread[0].iter().map(|(n, _)| *n).collect();
        for (k, name) in names.iter().enumerate() {
            let mut v: Vec<f64> = per_thread.iter().map(|r| r[k].1).collect();
            let rate: f64 = v.iter().map(|ns| 1.0 / ns).sum();
            v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
            println!(
                "COST probe=mul threads={threads} kernel=\"{name}\" fastest_ns={:.3} slowest_ns={:.3} aggregate_ns={:.4} fastest_cycles={:.2} per_thread_ns={}",
                v[0],
                v[v.len() - 1],
                1.0 / rate,
                v[0] * clocks.iter().cloned().fold(0.0, f64::max),
                fmt_list(&v)
            );
        }
        // Independent products streamed from DRAM: 3 × 256 MB.
        let n = 1usize << 24;
        let a = elems(n, 91);
        let b = elems(n, 92);
        let mut out: Vec<MaybeUninit<Gf>> = Vec::with_capacity(n);
        // SAFETY: `MaybeUninit` needs no initialisation.
        unsafe { out.set_len(n) };
        let chunk = 1usize << 12;
        let t = pool.install(|| {
            time_it(reps, || {
                out.par_chunks_mut(chunk)
                    .zip(a.par_chunks(chunk))
                    .zip(b.par_chunks(chunk))
                    .for_each(|((o, x), y)| kernels::product_into(x, y, o));
            })
        });
        println!(
            "COST probe=mul threads={threads} kernel=\"mul_red stream 2^24 (DRAM)\" aggregate_ns={:.4}",
            t * 1e9 / n as f64
        );
        let t = pool.install(|| {
            time_it(reps, || {
                let acc: Gf = a
                    .par_chunks(chunk)
                    .zip(b.par_chunks(chunk))
                    .map(|(x, y)| kernels::dot(x, y))
                    .reduce(Gf::zero, |p, q| p + q);
                black_box(acc);
            })
        });
        println!(
            "COST probe=mul threads={threads} kernel=\"unreduced dot stream 2^24 (DRAM)\" aggregate_ns={:.4}",
            t * 1e9 / n as f64
        );
    }
}

// ---------------------------------------------------------------------
// Probe 2: table reads and bucket scatters vs the table's size.
// ---------------------------------------------------------------------

/// `idx.len()·offsets.len()` data-dependent 16-byte reads of `tab` (a
/// power-of-two number of entries), pass `p` reading entry
/// `(idx[i] + offsets[p]) & mask`; four XOR accumulators.
#[inline(never)]
fn gather(tab: &[Gf], idx: &[u32], offsets: &[u32]) -> Gf {
    let mask = (tab.len() - 1) as u32;
    // SAFETY: every index is masked into the table.
    unsafe {
        let base = tab.as_ptr().cast::<u64>();
        let mut acc = [vzero(); 4];
        for &off in offsets {
            for c in idx.chunks_exact(4) {
                let i0 = (c.get_unchecked(0).wrapping_add(off) & mask) as usize;
                let i1 = (c.get_unchecked(1).wrapping_add(off) & mask) as usize;
                let i2 = (c.get_unchecked(2).wrapping_add(off) & mask) as usize;
                let i3 = (c.get_unchecked(3).wrapping_add(off) & mask) as usize;
                acc[0] = xor(acc[0], vld1q_u64(base.add(2 * i0)));
                acc[1] = xor(acc[1], vld1q_u64(base.add(2 * i1)));
                acc[2] = xor(acc[2], vld1q_u64(base.add(2 * i2)));
                acc[3] = xor(acc[3], vld1q_u64(base.add(2 * i3)));
            }
        }
        accs_to_gf(&acc)
    }
}

/// [`gather`]'s loop with the load replaced by a broadcast of the index.
#[inline(never)]
fn gather_baseline(mask: u32, idx: &[u32], offsets: &[u32]) -> Gf {
    let mut acc = [vzero(); 4];
    for &off in offsets {
        for c in idx.chunks_exact(4) {
            acc[0] = xor(acc[0], dup((c[0].wrapping_add(off) & mask) as usize));
            acc[1] = xor(acc[1], dup((c[1].wrapping_add(off) & mask) as usize));
            acc[2] = xor(acc[2], dup((c[2].wrapping_add(off) & mask) as usize));
            acc[3] = xor(acc[3], dup((c[3].wrapping_add(off) & mask) as usize));
        }
    }
    accs_to_gf(&acc)
}

/// `tab[(idx[i] + off) & mask] ^= w[i]`: one read-modify-write per update.
#[inline(never)]
fn scatter(tab: &mut [Gf], idx: &[u32], offsets: &[u32], w: &[Gf]) {
    let mask = (tab.len() - 1) as u32;
    assert_eq!(w.len(), idx.len());
    // SAFETY: every index is masked into the table.
    unsafe {
        let base = tab.as_mut_ptr().cast::<u64>();
        for &off in offsets {
            for (i, &x) in idx.iter().enumerate() {
                let p = base.add(2 * (x.wrapping_add(off) & mask) as usize);
                vst1q_u64(p, veorq_u64(vld1q_u64(p), ld(w.get_unchecked(i))));
            }
        }
    }
}

#[test]
#[ignore]
fn probe_table_read_sweep() {
    let reps = reps();
    const IDX: usize = 4096;
    const READS: usize = 1 << 23;
    let passes = READS / IDX;
    let sizes_log: Vec<u32> = knob_list("COST_TABLE_LOG_BYTES", "12,13,14,15,16,17,18,19,20,22,24,26,28,30")
        .iter()
        .map(|x| x.parse().expect("COST_TABLE_LOG_BYTES"))
        .collect();
    let max_entries = 1usize << (sizes_log.iter().max().expect("sizes") - 4);
    let table = elems(max_entries, 5);
    for threads in threads_list() {
        let pool = pool(threads);
        for &log_bytes in &sizes_log {
            let entries = 1usize << (log_bytes - 4);
            let tab = &table[..entries];
            let rows = on_every_thread(&pool, |i| {
                let mut state = 0x5EED_0000 + i as u64;
                let idx: Vec<u32> = (0..IDX).map(|_| xorshift(&mut state) as u32).collect();
                let offsets: Vec<u32> = (0..passes).map(|_| xorshift(&mut state) as u32).collect();
                let w = elems(IDX, 40 + i as u64);
                let g = time_it(reps, || {
                    black_box(gather(tab, &idx, &offsets));
                });
                let b = time_it(reps, || {
                    black_box(gather_baseline((entries - 1) as u32, &idx, &offsets));
                });
                // A private copy to scatter into, capped at 16 MB per thread.
                let s = if log_bytes <= 24 {
                    let mut mine = tab.to_vec();
                    time_it(reps, || scatter(&mut mine, &idx, &offsets, &w))
                } else {
                    f64::NAN
                };
                (g, b, s)
            });
            let per = |secs: f64| secs * 1e9 / READS as f64;
            let gathers: Vec<f64> = rows.iter().map(|r| per(r.0)).collect();
            let bases: Vec<f64> = rows.iter().map(|r| per(r.1)).collect();
            let scatters: Vec<f64> = rows.iter().map(|r| per(r.2)).collect();
            let agg = |v: &[f64]| 1.0 / v.iter().map(|x| 1.0 / x).sum::<f64>();
            let mut sorted = gathers.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
            println!(
                "COST probe=read threads={threads} table_bytes=2^{log_bytes} read_ns_fastest={:.3} read_ns_slowest={:.3} read_ns_aggregate={:.4} index_only_ns={:.3} scatter_ns_fastest={:.3} scatter_ns_aggregate={:.4}",
                sorted[0],
                sorted[sorted.len() - 1],
                agg(&gathers),
                bases.iter().cloned().fold(f64::MAX, f64::min),
                scatters.iter().cloned().fold(f64::NAN, f64::min),
                agg(&scatters),
            );
        }
    }
}

// ---------------------------------------------------------------------
// Probe 3: the table-driven kernels in place, with knock-outs.
// ---------------------------------------------------------------------

/// The JIT round's per-task scratch, as `kernels::neon::SumBuckets`:
/// three 256-entry tables of unreduced accumulators (`end`, `inf_lo`,
/// `inf_hi`).
struct Bk {
    data: Vec<[u64; 4]>,
}

impl Bk {
    fn new() -> Self {
        Self {
            data: vec![[0u64; 4]; 3 * 256],
        }
    }

    fn clear(&mut self) {
        self.data.fill([0u64; 4]);
    }
}

#[inline(always)]
unsafe fn bucket_xor(base: *mut u64, idx: usize, lo: uint64x2_t, hi: uint64x2_t) {
    // SAFETY: the caller keeps `idx` below the table's 256 entries.
    unsafe {
        let p = base.add(4 * idx);
        vst1q_u64(p, veorq_u64(vld1q_u64(p), lo));
        vst1q_u64(p.add(2), veorq_u64(vld1q_u64(p.add(2)), hi));
    }
}

/// `kernels::neon::jit_bucket_group` (`send_one = false`) in mode `MODE`.
#[inline(always)]
unsafe fn jit_round_group<const MODE: u8>(
    tab_o: [&[Gf]; 2],
    pat: &[[u8; 64]; 4],
    eq_t: &[Gf],
    bk: &mut Bk,
    acc: &mut [uint64x2_t; 4],
    wide: &mut [Acc; 2],
) {
    // SAFETY: byte indices land inside 256-entry tables; positions < 64.
    unsafe {
        let base = bk.data.as_mut_ptr().cast::<u64>();
        let end = base;
        let inf_lo = base.add(4 * 256);
        let inf_hi = base.add(8 * 256);
        macro_rules! column {
            ($m:expr, $j:expr) => {{
                let m = $m;
                let a_lo = *pat[0].get_unchecked(m) as usize;
                let a_hi = *pat[1].get_unchecked(m) as usize;
                let i_lo = *pat[2].get_unchecked(m) as usize;
                let i_hi = *pat[3].get_unchecked(m) as usize;
                let w = ld(eq_t.get_unchecked(m));
                let (o_lo, o_hi) = if MODE == NOREAD || MODE == TRAVERSE {
                    (veorq_u64(w, dup(i_lo)), veorq_u64(w, dup(i_hi)))
                } else {
                    (ld(tab_o[0].get_unchecked(i_lo)), ld(tab_o[1].get_unchecked(i_hi)))
                };
                if MODE == PROD || MODE == NOREAD {
                    let (pl, ph) = clmul_256(w, o_lo);
                    bucket_xor(end, a_lo, pl, ph);
                    let (dl, dh) = clmul_256(w, veorq_u64(o_hi, o_lo));
                    bucket_xor(inf_lo, a_lo, dl, dh);
                    bucket_xor(inf_hi, a_hi, dl, dh);
                } else if MODE == NOSCATTER {
                    acc_add(&mut wide[0], clmul_256(w, o_lo));
                    acc_add(&mut wide[1], clmul_256(w, veorq_u64(o_hi, o_lo)));
                    acc[$j] = veorq_u64(acc[$j], dup(a_lo | (a_hi << 8)));
                } else if MODE == NOMUL {
                    bucket_xor(end, a_lo, veorq_u64(w, o_lo), o_lo);
                    let d = veorq_u64(o_hi, o_lo);
                    bucket_xor(inf_lo, a_lo, veorq_u64(w, d), d);
                    bucket_xor(inf_hi, a_hi, veorq_u64(w, d), d);
                } else {
                    acc[$j] = veorq_u64(acc[$j], veorq_u64(veorq_u64(o_lo, o_hi), dup(a_lo | (a_hi << 8))));
                }
            }};
        }
        let mut m = 0usize;
        while m < 64 {
            column!(m, 0);
            column!(m + 1, 1);
            column!(m + 2, 2);
            column!(m + 3, 3);
            m += 4;
        }
    }
}

/// `kernels::neon::jit_bucket_finish` with `send_one = false`.
unsafe fn bucket_finish(tab_e: [&[Gf]; 2], bk: &Bk) -> (Gf, Gf) {
    // SAFETY: indices below 256.
    unsafe {
        let contract = |t: &[Gf], table: &[[u64; 4]]| -> Acc {
            let mut acc_a = (vzero(), vzero());
            let mut acc_b = (vzero(), vzero());
            let mut a = 0usize;
            while a < 256 {
                let e0 = table.get_unchecked(a);
                let v0 = reduce_256(vld1q_u64(e0.as_ptr()), vld1q_u64(e0.as_ptr().add(2)));
                acc_add(&mut acc_a, clmul_256(ld(t.get_unchecked(a)), v0));
                let e1 = table.get_unchecked(a + 1);
                let v1 = reduce_256(vld1q_u64(e1.as_ptr()), vld1q_u64(e1.as_ptr().add(2)));
                acc_add(&mut acc_b, clmul_256(ld(t.get_unchecked(a + 1)), v1));
                a += 2;
            }
            acc_add(&mut acc_a, acc_b);
            acc_a
        };
        let end = contract(tab_e[0], &bk.data[..256]);
        let mut inf = contract(tab_e[0], &bk.data[256..512]);
        acc_add(&mut inf, contract(tab_e[1], &bk.data[512..768]));
        (acc_to_gf(end), acc_to_gf(inf))
    }
}

/// [`Forest::jit_round_sums`] (`send_one = false`) in mode `MODE`.
fn jit_round_v<const MODE: u8>(
    forest: &Forest<'_>,
    tables: &Tables,
    ell: usize,
    k: usize,
    eq_c: &[Gf],
    eq_y: &[Gf],
) -> (Gf, Gf) {
    let low_bits = forest.t - ell - 1 - k;
    let cols = 1usize << forest.s;
    let groups = cols.div_ceil(64);
    let rows = 1usize << (low_bits - 1);
    let eq_t = transposed_eq(eq_c);
    let buckets = matches!(MODE, PROD | NOREAD | NOSCATTER | NOMUL);
    let row = |y1: usize, bk: &mut Bk| -> (Gf, Gf) {
        if buckets {
            bk.clear();
        }
        let mut words = [0u64; 8];
        let mut pats = [[0u8; 64]; 4];
        let tab: [&[Gf]; 4] = std::array::from_fn(|corner| {
            let (p, b1) = (corner >> 1, corner & 1);
            tables.at((p << low_bits) | (b1 << (low_bits - 1)) | y1)
        });
        let mut acc = [vzero(); 4];
        let mut wide = [(vzero(), vzero()); 2];
        let mut pacc = 0u64;
        for g in 0..groups {
            for (corner, pat) in pats.iter_mut().enumerate() {
                let (p, b1) = (corner >> 1, corner & 1);
                let y = y1 | (b1 << (low_bits - 1));
                forest.corner_words(ell, k, p, y, g, &mut words);
                *pat = transposed_patterns(&mut words);
            }
            if MODE == PATTERNS {
                pacc ^= pats.iter().map(fold_block).fold(0, |a, b| a ^ b);
                continue;
            }
            // SAFETY: 256-entry tables, 64 weights.
            unsafe {
                jit_round_group::<MODE>([tab[2], tab[3]], &pats, &eq_t[g << 6..(g + 1) << 6], bk, &mut acc, &mut wide)
            };
        }
        let (end, inf) = if buckets {
            // SAFETY: 256-entry tables.
            unsafe { bucket_finish([tab[0], tab[1]], bk) }
        } else {
            (Gf::zero(), Gf::zero())
        };
        let extra = accs_to_gf(&acc) + acc_to_gf(wide[0]) + acc_to_gf(wide[1]) + Gf::new(pacc, 0);
        let w = eq_y[y1];
        ((end + extra) * w, inf * w)
    };
    let partials: Vec<(Gf, Gf)> = (0..rows)
        .into_par_iter()
        .with_min_len(1)
        .map_init(Bk::new, |bk, y1| row(y1, bk))
        .collect();
    partials
        .into_iter()
        .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
}

/// `kernels::neon::jit_fold_group` (`send_one = false`) in mode `MODE`.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
unsafe fn jit_fold_group_v<const MODE: u8>(
    tab: [&[Gf]; 8],
    pat: &[[u8; 64]; 8],
    rl: uint64x2_t,
    rh: uint64x2_t,
    eq_t: &[Gf],
    out_l: [&mut [MaybeUninit<Gf>]; 2],
    out_r: [&mut [MaybeUninit<Gf>]; 2],
    sums: &mut (Acc, Acc),
    acc: &mut [uint64x2_t; 4],
) {
    let [out_l0, out_l1] = out_l;
    let [out_r0, out_r1] = out_r;
    let width = out_l0.len();
    // SAFETY: as `jit_fold_group`; a store's column is checked against
    // `width` (never taken for a full group).
    unsafe {
        let g = vdupq_n_u64(0x87);
        let z = vzero();
        let (mut eb, mut ib) = ((vzero(), vzero()), (vzero(), vzero()));
        macro_rules! column {
            ($m:expr, $end:expr, $inf:expr, $j:expr) => {{
                let m = $m;
                let c = kernels::col_of(m);
                if c < width {
                    let w = ld(eq_t.get_unchecked(m));
                    let v = |i: usize| {
                        let p = *pat[i].get_unchecked(m) as usize;
                        if MODE == NOREAD || MODE == TRAVERSE {
                            veorq_u64(w, dup(p))
                        } else {
                            ld(tab[i].get_unchecked(p))
                        }
                    };
                    if MODE == PROD || MODE == NOREAD {
                        let fl0 = fold1(rl, rh, g, z, v(0b000), v(0b010));
                        let fl1 = fold1(rl, rh, g, z, v(0b001), v(0b011));
                        let fr0 = fold1(rl, rh, g, z, v(0b100), v(0b110));
                        let fr1 = fold1(rl, rh, g, z, v(0b101), v(0b111));
                        st_uninit(out_l0.get_unchecked_mut(c), fl0);
                        st_uninit(out_l1.get_unchecked_mut(c), fl1);
                        st_uninit(out_r0.get_unchecked_mut(c), fr0);
                        st_uninit(out_r1.get_unchecked_mut(c), fr1);
                        slot(w, fl0, fl1, fr0, fr1, g, z, $end, $inf);
                    } else if MODE == NOMUL {
                        let fl0 = veorq_u64(v(0b000), v(0b010));
                        let fl1 = veorq_u64(v(0b001), v(0b011));
                        let fr0 = veorq_u64(v(0b100), v(0b110));
                        let fr1 = veorq_u64(v(0b101), v(0b111));
                        st_uninit(out_l0.get_unchecked_mut(c), fl0);
                        st_uninit(out_l1.get_unchecked_mut(c), fl1);
                        st_uninit(out_r0.get_unchecked_mut(c), fr0);
                        st_uninit(out_r1.get_unchecked_mut(c), fr1);
                        acc[$j] = veorq_u64(acc[$j], veorq_u64(veorq_u64(fl0, fl1), veorq_u64(fr0, fr1)));
                    } else {
                        let x = veorq_u64(
                            veorq_u64(veorq_u64(v(0), v(1)), veorq_u64(v(2), v(3))),
                            veorq_u64(veorq_u64(v(4), v(5)), veorq_u64(v(6), v(7))),
                        );
                        acc[$j] = veorq_u64(acc[$j], x);
                    }
                }
            }};
        }
        let mut m = 0usize;
        while m < 64 {
            column!(m, &mut sums.0, &mut sums.1, 0);
            column!(m + 1, &mut eb, &mut ib, 1);
            m += 2;
        }
        acc_add(&mut sums.0, eb);
        acc_add(&mut sums.1, ib);
    }
}

/// [`Forest::jit_fold_into`] (`send_one = false`) in mode `MODE`.
#[allow(clippy::too_many_arguments)]
fn jit_fold_v<const MODE: u8>(
    forest: &Forest<'_>,
    tables: &Tables,
    ell: usize,
    k: usize,
    rho: Gf,
    eq_c: &[Gf],
    eq_y: &[Gf],
    out: &mut [MaybeUninit<Gf>],
) -> (Gf, Gf) {
    let low_bits = forest.t - ell - 1 - k;
    let cols = 1usize << forest.s;
    let groups = cols.div_ceil(64);
    let half = out.len() / 2;
    let (e_half, o_half) = out.split_at_mut(half);
    let (e_lo, e_hi) = e_half.split_at_mut(half / 2);
    let (o_lo, o_hi) = o_half.split_at_mut(half / 2);
    let eq_t = transposed_eq(eq_c);
    // SAFETY: plain word loads.
    let (rl, rh) = unsafe { prep_fixed(&rho) };
    let partials: Vec<(Gf, Gf)> = e_lo
        .par_chunks_mut(cols)
        .zip(e_hi.par_chunks_mut(cols))
        .zip(o_lo.par_chunks_mut(cols))
        .zip(o_hi.par_chunks_mut(cols))
        .enumerate()
        .map(|(y2, (((el, eh), ol), oh))| {
            let mut sums = ((vzero(), vzero()), (vzero(), vzero()));
            let mut acc = [vzero(); 4];
            let mut pacc = 0u64;
            let mut words = [0u64; 8];
            let mut pats = [[0u8; 64]; 8];
            let tab: [&[Gf]; 8] = std::array::from_fn(|corner| {
                let (p, b1, b2) = (corner >> 2, (corner >> 1) & 1, corner & 1);
                tables.at((p << low_bits) | (b1 << (low_bits - 1)) | (b2 << (low_bits - 2)) | y2)
            });
            for g in 0..groups {
                for (corner, pat) in pats.iter_mut().enumerate() {
                    let (p, b1, b2) = (corner >> 2, (corner >> 1) & 1, corner & 1);
                    let y = y2 | (b2 << (low_bits - 2)) | (b1 << (low_bits - 1));
                    forest.corner_words(ell, k, p, y, g, &mut words);
                    *pat = transposed_patterns(&mut words);
                }
                if MODE == PATTERNS {
                    pacc ^= pats.iter().map(fold_block).fold(0, |a, b| a ^ b);
                    continue;
                }
                let base_c = g << 6;
                let width = 64.min(cols - base_c);
                let range = base_c..base_c + width;
                // SAFETY: 256-entry tables, 64 weights, `width` columns.
                unsafe {
                    jit_fold_group_v::<MODE>(
                        tab,
                        &pats,
                        rl,
                        rh,
                        &eq_t[g << 6..(g + 1) << 6],
                        [&mut el[range.clone()], &mut eh[range.clone()]],
                        [&mut ol[range.clone()], &mut oh[range]],
                        &mut sums,
                        &mut acc,
                    )
                };
            }
            let w = eq_y[y2];
            ((acc_to_gf(sums.0) + accs_to_gf(&acc) + Gf::new(pacc, 0)) * w, acc_to_gf(sums.1) * w)
        })
        .collect();
    partials
        .into_iter()
        .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
}

/// `kernels::neon::jit_product_group` in mode `MODE`.
#[inline(always)]
unsafe fn product_group_v<const MODE: u8>(
    tab: [&[Gf]; 2],
    pat: &[[u8; 64]; 2],
    out: &mut [MaybeUninit<Gf>],
    acc: &mut [uint64x2_t; 4],
) {
    let width = out.len();
    // SAFETY: as `jit_product_group`; stores are checked against `width`.
    unsafe {
        let g = vdupq_n_u64(0x87);
        let z = vzero();
        let k1 = vdupq_n_u64(0x9E37_79B9_7F4A_7C15);
        if MODE == PROD || MODE == NOREAD || MODE == NOMUL {
            // The production loop shape, so the knock-outs differ from the
            // prover's kernel in the knocked-out operation only.
            for m in 0..64 {
                let c = kernels::col_of(m);
                if c < width {
                    let (p0, p1) = (pat[0][m] as usize, pat[1][m] as usize);
                    let (a, b) = if MODE == NOREAD {
                        (veorq_u64(k1, dup(p0)), veorq_u64(k1, dup(p1 << 3)))
                    } else {
                        (ld(tab[0].get_unchecked(p0)), ld(tab[1].get_unchecked(p1)))
                    };
                    let v = if MODE == NOMUL { veorq_u64(a, b) } else { mul_red(a, b, g, z) };
                    st_uninit(out.get_unchecked_mut(c), v);
                }
            }
            return;
        }
        macro_rules! column {
            ($m:expr, $j:expr) => {{
                let m = $m;
                let c = kernels::col_of(m);
                if c < width {
                    let (p0, p1) = (*pat[0].get_unchecked(m) as usize, *pat[1].get_unchecked(m) as usize);
                    let (a, b) = if MODE == NOREAD || MODE == TRAVERSE {
                        (veorq_u64(k1, dup(p0)), veorq_u64(k1, dup(p1 << 3)))
                    } else {
                        (ld(tab[0].get_unchecked(p0)), ld(tab[1].get_unchecked(p1)))
                    };
                    if MODE == PROD || MODE == NOREAD {
                        st_uninit(out.get_unchecked_mut(c), mul_red(a, b, g, z));
                    } else if MODE == NOMUL {
                        st_uninit(out.get_unchecked_mut(c), veorq_u64(a, b));
                    } else {
                        acc[$j] = veorq_u64(acc[$j], veorq_u64(a, b));
                    }
                }
            }};
        }
        let mut m = 0usize;
        while m < 64 {
            column!(m, 0);
            column!(m + 1, 1);
            column!(m + 2, 2);
            column!(m + 3, 3);
            m += 4;
        }
    }
}

/// [`Forest::product_level_into`] in mode `MODE`: the prover's harness
/// verbatim (rows of the empty, pre-sized `out` written through its spare
/// capacity), the group kernel swapped. `READONLY`/`TRAVERSE` write only
/// their accumulator, into each row's first slot, and leave `out` empty.
fn product_level_v<const MODE: u8>(forest: &Forest<'_>, tables: &Tables, out: &mut Vec<Gf>) {
    let t = forest.t;
    let ell = MATERIALISED_LEVEL;
    let cols = 1usize << forest.s;
    let groups = cols.div_ceil(64);
    let rows = 1usize << (t - ell - 1);
    let len = rows * cols;
    assert!(out.is_empty() && out.capacity() >= len);
    let spare = &mut out.spare_capacity_mut()[..len];
    spare.par_chunks_mut(cols).enumerate().for_each(|(y, chunk)| {
        let tab = [tables.at(y), tables.at(y | (1 << (t - ell - 1)))];
        let mut words = [0u64; 8];
        let mut pats = [[0u8; 64]; 2];
        let mut acc = [vzero(); 4];
        let mut pacc = 0u64;
        for g in 0..groups {
            for b in 0..2 {
                forest.corner_words(ell, 0, b, y, g, &mut words);
                pats[b] = transposed_patterns(&mut words);
            }
            if MODE == PATTERNS {
                pacc ^= fold_block(&pats[0]) ^ fold_block(&pats[1]);
                continue;
            }
            let base_c = g << 6;
            let width = 64.min(cols - base_c);
            // SAFETY: 256-entry tables, `width` columns.
            unsafe { product_group_v::<MODE>(tab, &pats, &mut chunk[base_c..base_c + width], &mut acc) };
        }
        if MODE == READONLY || MODE == TRAVERSE || MODE == PATTERNS {
            chunk[0].write(accs_to_gf(&acc) + Gf::new(pacc, 0));
        }
    });
    if MODE == PROD || MODE == NOREAD || MODE == NOMUL {
        // SAFETY: every slot of every row was written by the kernel.
        unsafe { out.set_len(len) };
    }
}

/// `kernels::scatter_add` in mode `MODE` (`PROD`, `NOSCATTER` = the weights
/// XORed into registers, `TRAVERSE` = the indices only).
#[inline(always)]
fn scatter_v<const MODE: u8>(bucket: &mut [Gf], idx: &[u8; 64], eq_t: &[Gf], acc: &mut [uint64x2_t; 4]) {
    if MODE == PROD {
        kernels::scatter_add(bucket, idx, eq_t);
        return;
    }
    if MODE == PATTERNS {
        acc[0] = xor(acc[0], dup(fold_block(idx) as usize));
        return;
    }
    assert_eq!(eq_t.len(), 64);
    // SAFETY: positions < 64.
    unsafe {
        macro_rules! term {
            ($m:expr, $j:expr) => {{
                let m = $m;
                let i = *idx.get_unchecked(m) as usize;
                let v = if MODE == NOSCATTER { veorq_u64(ld(eq_t.get_unchecked(m)), dup(i)) } else { dup(i) };
                acc[$j] = veorq_u64(acc[$j], v);
            }};
        }
        let mut m = 0usize;
        while m < 64 {
            term!(m, 0);
            term!(m + 1, 1);
            term!(m + 2, 2);
            term!(m + 3, 3);
            m += 4;
        }
    }
}

/// [`Forest::bit_row_pair`] (`send_one = false`) with its scatter in mode
/// `MODE`.
#[allow(clippy::too_many_arguments)]
fn bit_row_pair_v<const MODE: u8>(
    forest: &Forest<'_>,
    tables: &Tables,
    ell: usize,
    kk: usize,
    y0: usize,
    y_bits: usize,
    eq_t: &[Gf],
    eq_y: &[Gf],
    bk: &mut Buckets,
) -> (Gf, Gf) {
    let low_bits = y_bits + 1;
    let cols = 1usize << forest.s;
    let groups = cols.div_ceil(64);
    let zero = Gf::zero();
    bk.tuples.fill(zero);
    let corner_y = |y: usize, corner: usize| y | ((corner & 1) << y_bits);
    let corner_p = |corner: usize| corner >> 1;
    let mut tmp = [0u64; 8];
    let mut words = [0u64; 8];
    let mut acc = [vzero(); 4];
    for g in 0..groups {
        for i in 0..2 {
            for corner in 0..4 {
                forest.corner_words(ell, kk, corner_p(corner), corner_y(y0 + i, corner), g, &mut tmp);
                words[4 * i + corner] = tmp[0];
            }
        }
        let idx = transposed_patterns(&mut words);
        scatter_v::<MODE>(&mut bk.tuples, &idx, &eq_t[g << 6..(g + 1) << 6], &mut acc);
    }
    let mut marginal = [[zero; 16]; 2];
    for (k, &v) in bk.tuples.iter().enumerate() {
        marginal[0][k & 15] += v;
        marginal[1][k >> 4] += v;
    }
    let (mut end, mut inf) = (accs_to_gf(&acc), zero);
    for (i, m) in marginal.iter().enumerate() {
        let y = y0 + i;
        let q = |corner: usize| (corner_p(corner) << low_bits) | corner_y(y, corner);
        let (t_e_lo, t_e_hi) = (tables.at(q(0)), tables.at(q(1)));
        let (t_o_lo, t_o_hi) = (tables.at(q(2)), tables.at(q(3)));
        let w = eq_y[y];
        let we: [Gf; 2] = [w * t_e_lo[0], w * t_e_lo[1]];
        let mut c_end = [zero; 16];
        let mut c_inf = [zero; 16];
        for (t, (c_end_t, c_inf_t)) in c_end.iter_mut().zip(c_inf.iter_mut()).enumerate() {
            let (e_lo, e_hi, o_lo, o_hi) = (t & 1, (t >> 1) & 1, (t >> 2) & 1, (t >> 3) & 1);
            *c_end_t = we[e_lo] * t_o_lo[o_lo];
            *c_inf_t = w * (t_e_hi[e_hi] + t_e_lo[e_lo]) * (t_o_hi[o_hi] + t_o_lo[o_lo]);
        }
        end = end + kernels::dot(&c_end, m);
        inf = inf + kernels::dot(&c_inf, m);
    }
    (end, inf)
}

/// [`Forest::bit_row`] (`send_one = false`) with its scatters in mode
/// `MODE`.
#[allow(clippy::too_many_arguments)]
fn bit_row_v<const MODE: u8>(
    forest: &Forest<'_>,
    tables: &Tables,
    ell: usize,
    kk: usize,
    nb: usize,
    y: usize,
    y_bits: usize,
    eq_t: &[Gf],
    bk: &mut Buckets,
) -> (Gf, Gf) {
    let low_bits = y_bits + 1;
    let entries = 1usize << nb;
    let pairs = entries * entries;
    let cols = 1usize << forest.s;
    let groups = cols.div_ceil(64);
    bk.reset(nb);
    let corner_y = |corner: usize| y | ((corner & 1) << y_bits);
    let corner_p = |corner: usize| corner >> 1;
    let mut tmp = [0u64; 8];
    let mut words = [0u64; 8];
    let mut lh = [0u8; 64];
    let mut hl = [0u8; 64];
    let mut acc = [vzero(); 4];
    for g in 0..groups {
        let eq_g = &eq_t[g << 6..(g + 1) << 6];
        if nb <= 2 {
            words = [0u64; 8];
            for corner in 0..4 {
                forest.corner_words(ell, kk, corner_p(corner), corner_y(corner), g, &mut tmp);
                words[corner * nb..(corner + 1) * nb].copy_from_slice(&tmp[..nb]);
            }
            let idx = transposed_patterns(&mut words);
            scatter_v::<MODE>(&mut bk.tuples, &idx, eq_g, &mut acc);
        } else {
            let mut idx = [[0u8; 64]; 2];
            for (out, e_corner, o_corner) in [(0usize, 0usize, 2usize), (1, 1, 3)] {
                forest.corner_words(ell, kk, corner_p(o_corner), corner_y(o_corner), g, &mut tmp);
                words[..4].copy_from_slice(&tmp[..4]);
                forest.corner_words(ell, kk, corner_p(e_corner), corner_y(e_corner), g, &mut tmp);
                words[4..8].copy_from_slice(&tmp[..4]);
                idx[out] = transposed_patterns(&mut words);
            }
            for m in 0..64 {
                lh[m] = (idx[0][m] & 0xF0) | (idx[1][m] & 0x0F);
                hl[m] = (idx[1][m] & 0xF0) | (idx[0][m] & 0x0F);
            }
            scatter_v::<MODE>(&mut bk.ll, &idx[0], eq_g, &mut acc);
            scatter_v::<MODE>(&mut bk.hh, &idx[1], eq_g, &mut acc);
            scatter_v::<MODE>(&mut bk.lh, &lh, eq_g, &mut acc);
            scatter_v::<MODE>(&mut bk.hl, &hl, eq_g, &mut acc);
        }
    }
    if nb <= 2 {
        let mask = entries - 1;
        for (tuple, &value) in bk.tuples[..1 << (4 * nb)].iter().enumerate() {
            let pe_lo = tuple & mask;
            let pe_hi = (tuple >> nb) & mask;
            let po_lo = (tuple >> (2 * nb)) & mask;
            let po_hi = (tuple >> (3 * nb)) & mask;
            bk.ll[pe_lo * entries + po_lo] += value;
            bk.lh[pe_lo * entries + po_hi] += value;
            bk.hl[pe_hi * entries + po_lo] += value;
            bk.hh[pe_hi * entries + po_hi] += value;
        }
    }
    let q = |corner: usize| (corner_p(corner) << low_bits) | corner_y(corner);
    let (t_e_lo, t_e_hi) = (tables.at(q(0)), tables.at(q(1)));
    let (t_o_lo, t_o_hi) = (tables.at(q(2)), tables.at(q(3)));
    let ll = kernels::contract(t_e_lo, t_o_lo, &bk.ll[..pairs]);
    let hh = kernels::contract(t_e_hi, t_o_hi, &bk.hh[..pairs]);
    let inf = hh
        + kernels::contract(t_e_hi, t_o_lo, &bk.hl[..pairs])
        + kernels::contract(t_e_lo, t_o_hi, &bk.lh[..pairs])
        + ll;
    (ll + accs_to_gf(&acc), inf)
}

/// [`Forest::bit_round`] (`send_one = false`) with prebuilt tables and eq
/// tables, its scatters in mode `MODE`.
fn bit_round_v<const MODE: u8>(
    forest: &Forest<'_>,
    tables: &Tables,
    ell: usize,
    j: usize,
    eq_t: &[Gf],
    eq_y: &[Gf],
) -> (Gf, Gf) {
    let kk = j - 1;
    let nb = 1usize << (ell + kk);
    let y_bits = forest.t - ell - 1 - j;
    let rows = 1usize << y_bits;
    let paired = nb == 1 && rows >= 2;
    let tasks = if paired { rows / 2 } else { rows };
    let task = |i: usize, bk: &mut Buckets| -> (Gf, Gf) {
        if paired {
            bit_row_pair_v::<MODE>(forest, tables, ell, kk, 2 * i, y_bits, eq_t, eq_y, bk)
        } else {
            let (end, inf) = bit_row_v::<MODE>(forest, tables, ell, kk, nb, i, y_bits, eq_t, bk);
            let w = eq_y[i];
            (end * w, inf * w)
        }
    };
    let partials: Vec<(Gf, Gf)> = (0..tasks)
        .into_par_iter()
        .with_min_len(1)
        .map_init(Buckets::new, |bk, i| task(i, bk))
        .collect();
    partials
        .into_iter()
        .fold((Gf::zero(), Gf::zero()), |(a, b), (x, y)| (a + x, b + y))
}

/// One knock-out table: the mode timings of a kernel (ms) and the derived
/// per-operation prices.
struct Row {
    kernel: String,
    ms: Vec<(u8, f64)>,
}

impl Row {
    fn get(&self, mode: u8) -> f64 {
        self.ms.iter().find(|(m, _)| *m == mode).map(|x| x.1).unwrap_or(f64::NAN)
    }

    fn print(&self, head: &str, extra: &str) {
        let modes: Vec<String> = self.ms.iter().map(|(m, v)| format!("{}_ms={v:.3}", mode_name(*m))).collect();
        println!("COST probe=insitu {head} kernel={} {} {extra}", self.kernel, modes.join(" "));
    }
}

fn ms(secs: f64) -> f64 {
    secs * 1e3
}

/// `(ns, ns)` per operation of a time difference in ms.
fn per_op(delta_ms: f64, ops: usize) -> f64 {
    delta_ms * 1e6 / ops as f64
}

#[test]
#[ignore]
fn probe_wfbitz_table_reads() {
    let reps = reps();
    let shapes: Vec<(usize, usize)> = knob_list("COST_SHAPES", "14:10,15:11,16:12")
        .iter()
        .map(|x| {
            let (t, s) = x.split_once(':').expect("t:s");
            (t.parse().expect("t"), s.parse().expect("s"))
        })
        .collect();
    for (t, s) in shapes {
        let n = t + s;
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ ((t as u64) << 32 | s as u64);
        let mut rng = || xorshift(&mut state);
        let groups = (1usize << s).div_ceil(64);
        let packed: Vec<Vec<u64>> = (0..groups).map(|_| (0..1usize << t).map(|_| rng()).collect()).collect();
        let images: Vec<Gf> = (0..1usize << t).map(|_| Gf::new(rng(), rng())).collect();
        let external: Vec<Gf> = (0..s + t).map(|_| Gf::new(rng(), rng())).collect();
        let challenges: Vec<Gf> = (0..3).map(|_| Gf::new(rng(), rng())).collect();
        let rho = Gf::new(rng(), rng());
        let forest = Forest::new(t, s, &packed, &images);
        let cols = 1usize << s;
        let (ell, k) = (0usize, MATERIALISED_LEVEL);
        let low_bits = t - ell - 1 - k;
        let eq_c = eq_table(&external[..s]);
        let eq_y1 = eq_table(&external[s..s + low_bits - 1]);
        let eq_y2 = eq_table(&external[s..s + low_bits - 2]);
        let tables = forest.fold_table(ell, k, &challenges);
        let tables3 = forest.fold_table(MATERIALISED_LEVEL, 0, &[]);
        let fold_len = 4 * (1usize << (low_bits - 2)) * cols;
        let product_len = (1usize << (t - MATERIALISED_LEVEL - 1)) * cols;
        let mut arena: Vec<Gf> = Vec::with_capacity(fold_len);
        let mut fold_out: Vec<MaybeUninit<Gf>> = Vec::with_capacity(fold_len);
        // SAFETY: `MaybeUninit` needs no initialisation.
        unsafe { fold_out.set_len(fold_len) };
        let mut product: Vec<Gf> = Vec::with_capacity(product_len);
        let mut product_out: Vec<Gf> = Vec::with_capacity(product_len);
        let bit_tables: Vec<Tables> = (1..=3).map(|j| forest.fold_table(ell, j - 1, &challenges[..j - 1])).collect();
        let eq_t_full = transposed_eq(&eq_c);
        let bit_eq_y: Vec<Vec<Gf>> = (1..=3).map(|j| eq_table(&external[s..s + (t - ell - 1 - j)])).collect();

        for threads in threads_list() {
            let pool = pool(threads);
            let head = format!("n={n} t={t} s={s} threads={threads}");
            pool.install(|| {
                // Correctness of the replicas: PROD must reproduce the prover.
                let want = forest.jit_round_sums(&tables, ell, k, &eq_c, &eq_y1, false);
                assert_eq!(jit_round_v::<PROD>(&forest, &tables, ell, k, &eq_c, &eq_y1), want, "jit round replica");
                let want = forest.jit_fold_round::<false>(&tables, ell, k, rho, &eq_c, &eq_y2, false, &mut arena);
                assert_eq!(
                    jit_fold_v::<PROD>(&forest, &tables, ell, k, rho, &eq_c, &eq_y2, &mut fold_out),
                    want,
                    "jit fold replica"
                );
                // SAFETY: the replica wrote every slot.
                let got: &[Gf] = unsafe { std::slice::from_raw_parts(fold_out.as_ptr().cast::<Gf>(), fold_len) };
                assert!(got == &arena[..], "jit fold replica output");
                product.clear();
                forest.product_level_into(&tables3, &mut product);
                product_out.clear();
                product_level_v::<PROD>(&forest, &tables3, &mut product_out);
                assert!(product_out == product, "product replica output");
                for j in 1..=3 {
                    let want = forest.bit_round(ell, j, &challenges[..j - 1], &external, false);
                    let got = bit_round_v::<PROD>(&forest, &bit_tables[j - 1], ell, j, &eq_t_full, &bit_eq_y[j - 1]);
                    assert_eq!(got, want, "bit round {j} replica");
                }

                // Table builds (the multiplications that make the reads possible).
                let tb = ms(time_it(reps, || {
                    black_box(forest.fold_table(ell, k, &challenges));
                }));
                let tb3 = ms(time_it(reps, || {
                    black_box(forest.fold_table(MATERIALISED_LEVEL, 0, &[]));
                }));
                println!(
                    "COST probe=insitu {head} kernel=tables k3_fold_ms={tb:.3} level3_value_ms={tb3:.3} entries={}",
                    tables.data.len()
                );

                // JIT round.
                let rows = 1usize << (low_bits - 1);
                let c = rows * cols;
                let prod_ms = ms(time_it(reps, || {
                    black_box(forest.jit_round_sums(&tables, ell, k, &eq_c, &eq_y1, false));
                }));
                let mut row = Row { kernel: "jit_round".into(), ms: Vec::new() };
                macro_rules! jr {
                    ($mode:expr) => {
                        row.ms.push((
                            $mode,
                            ms(time_it(reps, || {
                                black_box(jit_round_v::<$mode>(&forest, &tables, ell, k, &eq_c, &eq_y1));
                            })),
                        ))
                    };
                }
                jr!(PROD);
                jr!(NOREAD);
                jr!(NOSCATTER);
                jr!(NOMUL);
                jr!(READONLY);
                jr!(TRAVERSE);
                jr!(PATTERNS);
                let (p, nr, ns, nm, ro, tr) =
                    (row.get(PROD), row.get(NOREAD), row.get(NOSCATTER), row.get(NOMUL), row.get(READONLY), row.get(TRAVERSE));
                row.print(
                    &head,
                    &format!(
                        "prover_ms={prod_ms:.3} columns={c} reads={} scatters32={} unreduced={} read_marginal_ns={:.3} scatter_marginal_ns={:.3} unreduced_marginal_ns={:.3} read_standalone_ns={:.3} traverse_ns_per_column={:.3} prod_ns_per_column={:.3} patterns_ns_per_column={:.3} patterns_share={:.3}",
                        2 * c,
                        3 * c,
                        2 * c,
                        per_op(p - nr, 2 * c),
                        per_op(p - ns, 3 * c),
                        per_op(p - nm, 2 * c),
                        per_op(ro - tr, 2 * c),
                        per_op(tr, c),
                        per_op(p, c),
                        per_op(row.get(PATTERNS), c),
                        row.get(PATTERNS) / p,
                    ),
                );

                // JIT fold.
                let rows = 1usize << (low_bits - 2);
                let c = rows * cols;
                let prod_ms = ms(time_it(reps, || {
                    black_box(forest.jit_fold_round::<false>(&tables, ell, k, rho, &eq_c, &eq_y2, false, &mut arena));
                }));
                let mut row = Row { kernel: "jit_fold".into(), ms: Vec::new() };
                macro_rules! jf {
                    ($mode:expr) => {
                        row.ms.push((
                            $mode,
                            ms(time_it(reps, || {
                                black_box(jit_fold_v::<$mode>(&forest, &tables, ell, k, rho, &eq_c, &eq_y2, &mut fold_out));
                            })),
                        ))
                    };
                }
                jf!(PROD);
                jf!(NOREAD);
                jf!(NOMUL);
                jf!(READONLY);
                jf!(TRAVERSE);
                jf!(PATTERNS);
                let (p, nr, nm, ro, tr) = (row.get(PROD), row.get(NOREAD), row.get(NOMUL), row.get(READONLY), row.get(TRAVERSE));
                row.print(
                    &head,
                    &format!(
                        "prover_ms={prod_ms:.3} columns={c} reads={} muls={} pmull={} stores={} read_marginal_ns={:.3} mul_marginal_ns={:.3} pmull_marginal_ns={:.4} read_standalone_ns={:.3} store_ns={:.3} traverse_ns_per_column={:.3} prod_ns_per_column={:.3} patterns_ns_per_column={:.3} patterns_share={:.3}",
                        8 * c,
                        8 * c,
                        40 * c,
                        4 * c,
                        per_op(p - nr, 8 * c),
                        per_op(p - nm, 8 * c),
                        per_op(p - nm, 40 * c),
                        per_op(ro - tr, 8 * c),
                        per_op(nm - ro, 4 * c),
                        per_op(tr, c),
                        per_op(p, c),
                        per_op(row.get(PATTERNS), c),
                        row.get(PATTERNS) / p,
                    ),
                );

                // Level 4 as products of level 3's table values.
                let c = product_len;
                let prod_ms = ms(time_it(reps, || {
                    product.clear();
                    forest.product_level_into(&tables3, &mut product);
                    black_box(&product);
                }));
                let mut row = Row { kernel: "product_level".into(), ms: Vec::new() };
                macro_rules! pl {
                    ($mode:expr) => {
                        row.ms.push((
                            $mode,
                            ms(time_it(reps, || {
                                product_out.clear();
                                product_level_v::<$mode>(&forest, &tables3, &mut product_out);
                                black_box(&product_out);
                            })),
                        ))
                    };
                }
                pl!(PROD);
                pl!(NOREAD);
                pl!(NOMUL);
                pl!(READONLY);
                pl!(TRAVERSE);
                pl!(PATTERNS);
                // The prover's pass again, to bound the drift across the variants.
                let prod2_ms = ms(time_it(reps, || {
                    product.clear();
                    forest.product_level_into(&tables3, &mut product);
                    black_box(&product);
                }));
                let (p, nr, nm, ro, tr) = (row.get(PROD), row.get(NOREAD), row.get(NOMUL), row.get(READONLY), row.get(TRAVERSE));
                row.print(
                    &head,
                    &format!(
                        "prover_ms={prod_ms:.3} prover2_ms={prod2_ms:.3} columns={c} reads={} muls={c} stores={c} read_marginal_ns={:.3} mul_marginal_ns={:.3} read_standalone_ns={:.3} store_ns={:.3} traverse_ns_per_column={:.3} prod_ns_per_column={:.3} patterns_ns_per_column={:.3} patterns_share={:.3} patterns_share_vs_prover={:.3}",
                        2 * c,
                        per_op(p - nr, 2 * c),
                        per_op(p - nm, c),
                        per_op(ro - tr, 2 * c),
                        per_op(nm - ro, c),
                        per_op(tr, c),
                        per_op(p, c),
                        per_op(row.get(PATTERNS), c),
                        row.get(PATTERNS) / p,
                        row.get(PATTERNS) / prod_ms,
                    ),
                );

                // Level 0's bit rounds.
                for j in 1..=3 {
                    let y_bits = t - ell - 1 - j;
                    let nb = 1usize << (j - 1);
                    let rows = 1usize << y_bits;
                    let scatters = match nb {
                        1 => (rows / 2) * cols,
                        2 => rows * cols,
                        _ => 4 * rows * cols,
                    };
                    let prod_ms = ms(time_it(reps, || {
                        black_box(forest.bit_round(ell, j, &challenges[..j - 1], &external, false));
                    }));
                    let bt = &bit_tables[j - 1];
                    let ey = &bit_eq_y[j - 1];
                    let mut row = Row { kernel: format!("bit_round{j}"), ms: Vec::new() };
                    macro_rules! br {
                        ($mode:expr) => {
                            row.ms.push((
                                $mode,
                                ms(time_it(reps, || {
                                    black_box(bit_round_v::<$mode>(&forest, bt, ell, j, &eq_t_full, ey));
                                })),
                            ))
                        };
                    }
                    br!(PROD);
                    br!(NOSCATTER);
                    br!(TRAVERSE);
                    br!(PATTERNS);
                    let (p, ns, tr) = (row.get(PROD), row.get(NOSCATTER), row.get(TRAVERSE));
                    row.print(
                        &head,
                        &format!(
                            "prover_ms={prod_ms:.3} nb={nb} columns={} scatters16={scatters} scatter_marginal_ns={:.3} weights_ns={:.3} traverse_ns_per_scatter={:.3} prod_ns_per_scatter={:.3} patterns_share={:.3}",
                            rows * cols,
                            per_op(p - ns, scatters),
                            per_op(ns - tr, scatters),
                            per_op(tr, scatters),
                            per_op(p, scatters),
                            row.get(PATTERNS) / p,
                        ),
                    );
                }
            });
        }
    }
}

// ---------------------------------------------------------------------
// Probe 4: forming the table indices, gathered vs selected.
// ---------------------------------------------------------------------

/// What forming one table index (one 8-bit pattern of one column) costs:
/// a full pass of level `ell`'s JIT patterns (both corners of every `(y,
/// g)`), gathered and transposed as before, against the same pass selected
/// from the nibble rows, and the nibble rows' own one-time build. Each
/// pass XOR-folds its blocks so nothing is elided.
#[test]
#[ignore]
fn probe_index_costs() {
    use super::nibble::{self as nib, NibbleRows};
    let reps = reps();
    let shapes: Vec<(usize, usize)> = knob_list("COST_SHAPES", "15:11,16:12,17:13")
        .iter()
        .map(|x| {
            let (t, s) = x.split_once(':').expect("t:s");
            (t.parse().expect("t"), s.parse().expect("s"))
        })
        .collect();
    for (t, s) in shapes {
        let n = t + s;
        let mut state = 0x1D3A_5EED ^ ((t as u64) << 32 | s as u64);
        let groups = (1usize << s).div_ceil(64);
        let packed: Vec<Vec<u64>> = (0..groups).map(|_| (0..1usize << t).map(|_| xorshift(&mut state)).collect()).collect();
        let images: Vec<Gf> = (0..1usize << t).map(|_| Gf::new(xorshift(&mut state), xorshift(&mut state))).collect();
        let forest = Forest::new(t, s, &packed, &images);
        let h = t - 4;
        let indices = 2 * (1usize << h) * groups * 64;
        for threads in threads_list() {
            let pool = pool(threads);
            pool.install(|| {
                let per = |secs: f64, count: usize| secs * 1e9 / count as f64;
                let build = time_it(reps, || {
                    black_box(NibbleRows::new(t, &packed, groups));
                });
                let rows = NibbleRows::new(t, &packed, groups);
                for ell in [0usize, 3] {
                    let gather = time_it(reps, || {
                        let x = (0..1usize << h)
                            .into_par_iter()
                            .map(|y| {
                                let mut acc = 0u64;
                                let mut words = [0u64; 8];
                                for g in 0..groups {
                                    for p in 0..2 {
                                        forest.corner_words(ell, MATERIALISED_LEVEL - ell, p, y, g, &mut words);
                                        acc ^= fold_block(&transposed_patterns(&mut words));
                                    }
                                }
                                acc
                            })
                            .reduce(|| 0, |a, b| a ^ b);
                        black_box(x);
                    });
                    let sel = nib::jit_selectors(ell);
                    let select = time_it(reps, || {
                        let x = (0..1usize << h)
                            .into_par_iter()
                            .map(|y| {
                                let mut acc = 0u64;
                                let mut out = [[0u8; 64]; 2];
                                for g in 0..groups {
                                    nib::select(rows.block(y, g), &sel, &mut out);
                                    acc ^= fold_block(&out[0]) ^ fold_block(&out[1]);
                                }
                                acc
                            })
                            .reduce(|| 0, |a, b| a ^ b);
                        black_box(x);
                    });
                    println!(
                        "COST probe=index n={n} t={t} s={s} threads={threads} level={ell} indices={indices} gather_ms={:.3} gather_ns_per_index={:.4} select_ms={:.3} select_ns_per_index={:.4} build_ms={:.3} build_ns_per_entry={:.4}",
                        gather * 1e3,
                        per(gather, indices),
                        select * 1e3,
                        per(select, indices),
                        build * 1e3,
                        per(build, indices / 2),
                    );
                }
            });
        }
    }
}
