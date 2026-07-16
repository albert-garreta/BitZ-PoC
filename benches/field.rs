//! Field micro-benchmark: `GF(2^128)` (GHASH) vs `GF(2^127)` (b127),
//! head-to-head on the PCS prover's actual hot patterns.
//!
//! Plain `harness = false` binary (no criterion), in the style of
//! `benches/pcs.rs`. Six patterns, each a proxy for a prover phase:
//!
//! | pattern      | proxy for                                              |
//! |--------------|--------------------------------------------------------|
//! | `mul/batch`  | forest layer products (independent, throughput-bound)  |
//! | `mul/chain`  | dependent product chains (latency-bound)               |
//! | `square`     | α-power place-value / comb window advances (latency)   |
//! | `powers`     | `FixedBasePow` comb, win 8, ~100-bit exponents — the   |
//! |              | `chunk_pow2_table` / root-recompute workload (and the  |
//! |              | reilabs `ghash-powers-bench` shape)                    |
//! | `wide-dot`   | delayed-reduction inner products (`WideMulAcc`)        |
//! | `eqf-round`  | the fused single-pair sumcheck round kernel            |
//! | `eqf-fold`   | the in-place multilinear bind cascade                  |
//!
//! Run (the NEON pipeline is target-feature-gated — the flag is
//! load-bearing):
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo bench --bench field
//! ```
//! Knobs: `F2Z_BENCH_REPS` (timing repetitions per pattern, median
//! reported; default 5). Idle the box; expect ±5 % run-to-run.

use std::hint::black_box;
use std::time::Instant;

use f2z::poly::univariate::binary_b127::BinaryFieldB127;
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128;
use f2z::utils::wide_mul::WideMulAcc;

// ---------------------------------------------------------------------
// Deterministic data (no rand dep in benches — the pcs.rs convention).
// ---------------------------------------------------------------------

/// splitmix64 — deterministic, well-mixed 64-bit stream.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_u128(state: &mut u64) -> u128 {
    (splitmix(state) as u128) | ((splitmix(state) as u128) << 64)
}

// ---------------------------------------------------------------------
// The field abstraction the harness is generic over. Monomorphized —
// no dispatch in the timed loops (mirrors the reilabs bench design).
// ---------------------------------------------------------------------

trait BF:
    Copy
    + PartialEq
    + core::fmt::Display
    + core::ops::Add<Output = Self>
    + core::ops::Sub<Output = Self>
    + core::ops::Mul<Output = Self>
    + WideMulAcc
{
    const NAME: &'static str;
    fn zero() -> Self;
    fn one() -> Self;
    /// Deterministic element from a 128-bit pattern (b127 folds bit 127).
    fn from_u128(v: u128) -> Self;
    fn square(&self) -> Self;
}

impl BF for BinaryFieldGF128 {
    const NAME: &'static str = "GF(2^128) GHASH";
    fn zero() -> Self {
        BinaryFieldGF128::zero()
    }
    fn one() -> Self {
        BinaryFieldGF128::one()
    }
    fn from_u128(v: u128) -> Self {
        BinaryFieldGF128::from(v)
    }
    fn square(&self) -> Self {
        BinaryFieldGF128::square(self)
    }
}

impl BF for BinaryFieldB127 {
    const NAME: &'static str = "GF(2^127) b127";
    fn zero() -> Self {
        BinaryFieldB127::zero()
    }
    fn one() -> Self {
        BinaryFieldB127::one()
    }
    fn from_u128(v: u128) -> Self {
        BinaryFieldB127::from(v)
    }
    fn square(&self) -> Self {
        BinaryFieldB127::square(self)
    }
}

fn gen_vec<F: BF>(n: usize, seed: u64) -> Vec<F> {
    let mut st = seed;
    (0..n).map(|_| F::from_u128(rand_u128(&mut st))).collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Time `body` (which processes `ops` field operations) `reps` times
/// after one warm-up; return median ns/op.
fn time_ns_per_op<R>(reps: usize, ops: usize, mut body: impl FnMut() -> R) -> f64 {
    black_box(body()); // warm-up
    let mut samples = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t0 = Instant::now();
        black_box(body());
        samples.push(t0.elapsed().as_secs_f64() * 1e9 / ops as f64);
    }
    median(samples)
}

// ---------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------

const N_BATCH: usize = 1 << 21; // independent muls
const N_CHAIN: usize = 1 << 21; // dependent muls
const N_SQ: usize = 1 << 22; // dependent squarings
const N_POW: usize = 1 << 16; // comb exponentiations
const POW_BITS: usize = 100; // mod-q row-weight width (q = 2^100 − 15)
const POW_WIN: usize = 8; // chunk_pow2_table's window
const N_WIDE: usize = 1 << 21; // wide-dot terms
const EQF_HALF: usize = 1 << 20; // eqf round slots
const N_FOLD: usize = 1 << 21; // fold-cascade start size

/// out[i] = a[i]·b[i] — the forest-layer shape.
fn batch_mul<F: BF>(a: &[F], b: &[F], out: &mut [F]) {
    for ((x, y), o) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        *o = *x * *y;
    }
}

/// acc = ∏ v[i] — the dependent-latency shape.
fn chain_mul<F: BF>(v: &[F]) -> F {
    let mut acc = F::one();
    for x in v {
        acc = acc * *x;
    }
    acc
}

/// x ← x² N times — the squaring-chain shape.
fn square_chain<F: BF>(x0: F, n: usize) -> F {
    let mut x = x0;
    for _ in 0..n {
        x = x.square();
    }
    x
}

/// Fixed-base comb (the `pcs::FixedBasePow` construction, made generic
/// for the head-to-head): `table[i][d] = α^{d·2^{win·i}}`.
struct Comb<F> {
    table: Vec<Vec<F>>,
    win: usize,
}

impl<F: BF> Comb<F> {
    fn new(alpha: F, max_bits: usize, win: usize) -> Self {
        let num_windows = max_bits.div_ceil(win);
        let radix = 1usize << win;
        let mut table = Vec::with_capacity(num_windows);
        let mut base_i = alpha;
        for _ in 0..num_windows {
            let mut row = Vec::with_capacity(radix);
            let mut cur = F::one();
            for _ in 0..radix {
                row.push(cur);
                cur = cur * base_i;
            }
            table.push(row);
            for _ in 0..win {
                base_i = base_i.square();
            }
        }
        Self { table, win }
    }

    fn pow(&self, mut exp: u128) -> F {
        let mask = (1u128 << self.win).wrapping_sub(1);
        let mut acc = F::one();
        let mut i = 0usize;
        while exp != 0 {
            let d = (exp & mask) as usize;
            if d != 0 {
                acc = acc * self.table[i][d];
            }
            exp >>= self.win;
            i += 1;
        }
        acc
    }
}

/// Σ a[i]·b[i] through the delayed-reduction accumulator — the sumcheck
/// inner-product shape.
fn wide_dot<F: BF>(a: &[F], b: &[F]) -> F {
    let mut acc = F::wide_zero(&F::zero());
    for (x, y) in a.iter().zip(b.iter()) {
        F::wide_add_assign(&mut acc, &F::mul_wide(x, y));
    }
    F::from_wide(acc)
}

/// The bind cascade: fold 2^k → 2^{k-1} → … → 1 in place (each level is
/// `eqf_fold_in_place` at successive halves) — ~n muls total.
fn fold_cascade<F: BF>(v: &mut [F], rhos: &[F]) -> F {
    let mut len = v.len();
    let mut level = 0usize;
    while len > 1 {
        let half = len >> 1;
        assert!(F::eqf_fold_in_place(v, &rhos[level % rhos.len()], half));
        len = half;
        level += 1;
    }
    v[0]
}

// ---------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------

struct Row {
    pattern: &'static str,
    ns: [f64; 2], // [GF128, B127]
}

fn run_field<F: BF>(reps: usize) -> Vec<(&'static str, f64)> {
    let mut out = Vec::new();

    // mul/batch
    let a = gen_vec::<F>(N_BATCH, 0xA11CE);
    let b = gen_vec::<F>(N_BATCH, 0xB0B);
    let mut o = vec![F::zero(); N_BATCH];
    out.push((
        "mul/batch",
        time_ns_per_op(reps, N_BATCH, || {
            batch_mul(black_box(&a), black_box(&b), black_box(&mut o));
            o[N_BATCH - 1]
        }),
    ));

    // mul/chain
    let v = gen_vec::<F>(N_CHAIN, 0xC0FFEE);
    out.push(("mul/chain", time_ns_per_op(reps, N_CHAIN, || chain_mul(black_box(&v)))));

    // square chain
    let x0 = F::from_u128(0x1234_5678_9ABC_DEF0_0FED_CBA9_8765_4321);
    out.push(("square/chain", time_ns_per_op(reps, N_SQ, || square_chain(black_box(x0), N_SQ))));

    // powers (comb win 8, 100-bit exponents)
    let alpha = F::from_u128(2);
    let comb = Comb::new(alpha, 128, POW_WIN);
    let mut st = 0xE44_u64;
    let exps: Vec<u128> =
        (0..N_POW).map(|_| rand_u128(&mut st) & ((1u128 << POW_BITS) - 1)).collect();
    out.push((
        "powers/comb-w8-100b",
        time_ns_per_op(reps, N_POW, || {
            let mut acc = F::zero();
            for &e in &exps {
                acc = acc + comb.pow(black_box(e));
            }
            acc
        }),
    ));

    // wide-dot
    let wa = gen_vec::<F>(N_WIDE, 0xD07);
    let wb = gen_vec::<F>(N_WIDE, 0xD08);
    out.push(("wide-dot", time_ns_per_op(reps, N_WIDE, || wide_dot(black_box(&wa), black_box(&wb)))));

    // eqf round (the fused sumcheck kernel; 3 products + 2 weight-folds
    // per slot → count 5·half products)
    let l = gen_vec::<F>(2 * EQF_HALF, 0xE9F1);
    let r = gen_vec::<F>(2 * EQF_HALF, 0xE9F2);
    let w = gen_vec::<F>(EQF_HALF, 0xE9F3);
    out.push((
        "eqf-round",
        time_ns_per_op(reps, 5 * EQF_HALF, || {
            F::eqf_single_pair_round(black_box(&l), black_box(&r), black_box(&w), EQF_HALF)
                .expect("both fields ship the fused kernel")
        }),
    ));

    // eqf fold cascade (~N_FOLD muls total across all levels)
    let rhos = gen_vec::<F>(24, 0xF01D);
    let base = gen_vec::<F>(N_FOLD, 0xF01E);
    let mut buf = base.clone();
    out.push((
        "eqf-fold",
        time_ns_per_op(reps, N_FOLD, || {
            buf.copy_from_slice(&base);
            fold_cascade(black_box(&mut buf), black_box(&rhos))
        }),
    ));

    out
}

/// B127-only extra: the 3-PMULL Karatsuba product alternative vs the
/// schoolbook default, on the batch-mul shape.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn run_b127_kara(reps: usize) -> f64 {
    let a = gen_vec::<BinaryFieldB127>(N_BATCH, 0xA11CE);
    let b = gen_vec::<BinaryFieldB127>(N_BATCH, 0xB0B);
    let mut o = vec![BinaryFieldB127::zero(); N_BATCH];
    time_ns_per_op(reps, N_BATCH, || {
        for ((x, y), out) in a.iter().zip(b.iter()).zip(o.iter_mut()) {
            *out = x.mul_karatsuba(y);
        }
        o[N_BATCH - 1]
    })
}

fn main() {
    let reps: usize =
        std::env::var("F2Z_BENCH_REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);

    println!("F2Z field bench — GF(2^128) GHASH vs GF(2^127) b127, median of {reps} reps.");
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    println!("(target: aarch64 + neon — the NEON pipelines are active)");
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    println!("(WARNING: scalar pipelines — build with RUSTFLAGS=\"-C target-cpu=native\")");

    let g = run_field::<BinaryFieldGF128>(reps);
    let b = run_field::<BinaryFieldB127>(reps);
    let rows: Vec<Row> = g
        .iter()
        .zip(b.iter())
        .map(|(&(p, gn), &(p2, bn))| {
            assert_eq!(p, p2);
            Row { pattern: p, ns: [gn, bn] }
        })
        .collect();

    println!(
        "\n{:<22} {:>14} {:>14} {:>9}",
        "pattern", "GF128 ns/op", "b127 ns/op", "speedup"
    );
    for row in &rows {
        println!(
            "{:<22} {:>14.3} {:>14.3} {:>8.2}x",
            row.pattern,
            row.ns[0],
            row.ns[1],
            row.ns[0] / row.ns[1]
        );
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let kara = run_b127_kara(reps);
        println!(
            "{:<22} {:>14} {:>14.3}   (vs b127 schoolbook: {:.2}x)",
            "mul/batch b127-kara", "—", kara,
            rows[0].ns[1] / kara
        );
    }
}
