//! Direct public-API outer-sumcheck benchmark. Input construction and verification
//! are outside the measured call; internal allocation is included.
//! OUTER_SHAPES="12 15 17 19" OUTER_INPUTS="u32 u64 u128 field"
//! OUTER_PROTOCOLS="ordinary zero skip-1 skip-2 skip-3 skip-4" OUTER_REPS=21
//! RAYON_NUM_THREADS controls the pool. Use bench-peak-memory only for a separate
//! memory pass; allocator instrumentation must not be used for latency claims.
use f2z::{
    piop::spartan::SpartanField,
    sumcheck::{UngrindedRoundBoundary, outer::*},
    transcript::Blake3Transcript,
};
use field::{BatchMulAcc, FoldPairs, Fp, FpCtx, IntegerEmbedding, Reduce, RingOps, Uint, WideMul};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{hint::black_box, time::Instant};
fn words(name: &str, default: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

// A single driver monomorphizes exactly the same field provider with each
// native input representation. No benchmark trait is added to the library API.
fn bench_inputs<A, C>(
    kind: &str,
    field: &FpCtx<2>,
    a: &[A],
    b: &[A],
    c: &[C],
    variants: &[String],
    protocols: &[String],
    reps: usize,
    metrics: fn(bool) -> [usize; 3],
) where
    A: Copy + Send + Sync,
    C: Copy + Send + Sync,
    FpCtx<2>: WideMul<Fp<2>, A>
        + WideMul<Fp<2>, C>
        + FoldPairs<A, Fp<2>>
        + FoldPairs<C, Fp<2>>
        + BatchMulAcc<Fp<2>, A>
        + BatchMulAcc<Fp<2>, C>
        + Reduce<<FpCtx<2> as WideMul<Fp<2>, A>>::Product, Output = Fp<2>>
        + Reduce<<FpCtx<2> as WideMul<Fp<2>, C>>::Product, Output = Fp<2>>
        + Reduce<<FpCtx<2> as BatchMulAcc<Fp<2>, A>>::Accumulator, Output = Fp<2>>
        + Reduce<<FpCtx<2> as BatchMulAcc<Fp<2>, C>>::Accumulator, Output = Fp<2>>,
{
    let n = a.len().ilog2() as usize;
    let tau: Vec<_> = (0..n)
        .map(|i| field.from_integer(&(i as u64 + 7)))
        .collect();
    for protocol in protocols {
        let k: usize = protocol
            .strip_prefix("skip-")
            .map_or(0, |s| s.parse().unwrap());
        if n < k {
            continue;
        }
        let prepared = (k > 0).then(|| prepare_univariate_skip(field, k as u8).unwrap());
        let invoke = || {
            let mut transcript = Blake3Transcript::new();
            let before = metrics(true);
            let start = Instant::now();
            let (out, prefix) = if let Some(prepared) = &prepared {
                let out = prove_outer_zerocheck_with_skip_from_slices(
                    field,
                    &mut transcript,
                    prepared,
                    &tau[k..],
                    a,
                    b,
                    c,
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                (out.tail, Some(out.prefix))
            } else {
                let out = if protocol == "ordinary" {
                    prove_outer_sumcheck_from_slices(
                        field,
                        &mut transcript,
                        field.zero(),
                        &tau,
                        a,
                        b,
                        c,
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap()
                } else {
                    prove_outer_zerocheck_from_slices(
                        field,
                        &mut transcript,
                        &tau,
                        a,
                        b,
                        c,
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap()
                };
                (out, None)
            };
            let nanos = start.elapsed().as_nanos();
            let after = metrics(false);
            let allocation_count = after[0].saturating_sub(before[0]);
            let peak_delta = after[2].saturating_sub(before[1]);
            let digest = transcript.state_digest();
            let mut verifier = Blake3Transcript::new();
            if let Some(prefix) = &prefix {
                verify_outer_zerocheck_with_skip(
                    field,
                    &mut verifier,
                    prepared.as_ref().unwrap(),
                    &tau[k..],
                    prefix,
                    &out.proof,
                    out.evaluations,
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
            } else {
                verify_outer_sumcheck(
                    field,
                    &mut verifier,
                    field.zero(),
                    &tau,
                    &out.proof,
                    out.evaluations,
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
            }
            assert_eq!(digest, verifier.state_digest());
            (nanos, out, prefix, digest, allocation_count, peak_delta)
        };
        let (_, reference, prefix, digest, _, _) = invoke();
        // One excluded warmup per candidate. Rotate the measured ordering.
        for sample in 0..=reps {
            for j in 0..variants.len() {
                let name = &variants[(j + sample) % variants.len()];
                let (ns, out, p, d, allocs, peak) = invoke();
                assert_eq!(out, reference, "{kind}/{protocol}/{name}");
                assert_eq!(p, prefix);
                assert_eq!(d, digest);
                black_box(&out);
                println!(
                    "OUTER_SAMPLE {}",
                    serde_json::json!({"candidate":name,"input":kind,"protocol":protocol,"rows":a.len(),"threads":rayon::current_num_threads(),"sample":sample,"warmup":sample==0,"ns":ns.to_string(),"verified":true,"allocations":allocs,"peak_delta_bytes":peak})
                );
            }
        }
    }
}

pub fn run(metrics: fn(bool) -> [usize; 3]) {
    let variants = vec![String::from("current")];
    let protocols = words(
        "OUTER_PROTOCOLS",
        "ordinary zero skip-1 skip-2 skip-3 skip-4",
    );
    let types = words("OUTER_INPUTS", "u32 u64 u128 field");
    let reps = std::env::var("OUTER_REPS").map_or(5, |v| v.parse().unwrap());
    let field = Fp::<2>::make_cfg(&Uint::from((1u128 << 100) - 15)).unwrap();
    for exponent in words("OUTER_SHAPES", "12 15 17 19") {
        let n: usize = exponent.parse().unwrap();
        let mut rng = StdRng::seed_from_u64(0x5533_326d_756c_0073 ^ n as u64);
        let a: Vec<u128> = (0..1usize << n).map(|_| rng.random()).collect();
        let b: Vec<u128> = (0..a.len()).map(|_| rng.random()).collect();
        for kind in &types {
            match kind.as_str() {
                "u32" => {
                    let a: Vec<_> = a.iter().map(|v| *v as u32).collect();
                    let b: Vec<_> = b.iter().map(|v| *v as u32).collect();
                    let c: Vec<_> = a
                        .iter()
                        .zip(&b)
                        .map(|(a, b)| *a as u64 * *b as u64)
                        .collect();
                    bench_inputs(
                        kind, &field, &a, &b, &c, &variants, &protocols, reps, metrics,
                    );
                }
                "u64" => {
                    let a: Vec<_> = a.iter().map(|v| *v as u64).collect();
                    let b: Vec<_> = b.iter().map(|v| *v as u64).collect();
                    let c: Vec<_> = a
                        .iter()
                        .zip(&b)
                        .map(|(a, b)| *a as u128 * *b as u128)
                        .collect();
                    bench_inputs(
                        kind, &field, &a, &b, &c, &variants, &protocols, reps, metrics,
                    );
                }
                "u128" => {
                    let c: Vec<Uint<4>> = a
                        .iter()
                        .zip(&b)
                        .map(|(a, b)| {
                            *field::IntegerOps
                                .mul_wide(&Uint::<2>::from(*a), &Uint::<2>::from(*b))
                                .checked_resize_ct::<4>()
                                .value()
                        })
                        .collect();
                    bench_inputs(
                        kind, &field, &a, &b, &c, &variants, &protocols, reps, metrics,
                    );
                }
                "field" => {
                    let a: Vec<_> = a.iter().map(|v| field.from_integer(v)).collect();
                    let b: Vec<_> = b.iter().map(|v| field.from_integer(v)).collect();
                    let c: Vec<_> = a.iter().zip(&b).map(|(a, b)| field.mul(a, b)).collect();
                    bench_inputs(
                        kind, &field, &a, &b, &c, &variants, &protocols, reps, metrics,
                    );
                }
                _ => panic!("unknown input {kind}"),
            }
        }
    }
}

#[cfg(feature = "bench-peak-memory")]
mod memory {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicUsize, Ordering::Relaxed},
    };
    pub struct Counting;
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    static ALLOCS: AtomicUsize = AtomicUsize::new(0);
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(l) };
            if !p.is_null() {
                ALLOCS.fetch_add(1, Relaxed);
                let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
                PEAK.fetch_max(n, Relaxed);
            }
            p
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(l) };
            if !p.is_null() {
                ALLOCS.fetch_add(1, Relaxed);
                let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
                PEAK.fetch_max(n, Relaxed);
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            LIVE.fetch_sub(l.size(), Relaxed);
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let q = unsafe { System.realloc(p, l, n) };
            if !q.is_null() {
                ALLOCS.fetch_add(1, Relaxed);
                let live = if n >= l.size() {
                    LIVE.fetch_add(n - l.size(), Relaxed) + n - l.size()
                } else {
                    LIVE.fetch_sub(l.size() - n, Relaxed) - (l.size() - n)
                };
                PEAK.fetch_max(live, Relaxed);
            }
            q
        }
    }
    pub fn metrics(reset: bool) -> [usize; 3] {
        let live = LIVE.load(Relaxed);
        if reset {
            PEAK.store(live, Relaxed);
        }
        [ALLOCS.load(Relaxed), live, PEAK.load(Relaxed)]
    }
}
#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static ALLOC: memory::Counting = memory::Counting;
fn main() {
    #[cfg(feature = "bench-peak-memory")]
    run(memory::metrics);
    #[cfg(not(feature = "bench-peak-memory"))]
    run(|_| [0; 3]);
}
