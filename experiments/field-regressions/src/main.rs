//! Isolated regression experiments. No production dispatch is modified.
#[cfg(feature = "arithmetic-campaign")]
mod campaign;
#[cfg(feature = "arithmetic-campaign")]
#[allow(dead_code)]
#[path = "../../../src/utils/delayed_reduction.rs"]
mod production_delayed;
#[cfg(feature = "arithmetic-campaign")]
mod witgen {
    pub use circuit::witgen::Z;
}
#[cfg(feature = "arithmetic-campaign")]
#[allow(dead_code, unexpected_cfgs)]
#[path = "../../../crates/circuit/src/matrix_products.rs"]
mod production_projection;
#[cfg(feature = "arithmetic-campaign")]
#[allow(dead_code, unexpected_cfgs)]
mod production_raw {
    include!(concat!(env!("OUT_DIR"), "/raw_ctx.rs"));
}
#[cfg(feature = "arithmetic-campaign")]
mod production_p256 {
    include!(concat!(env!("OUT_DIR"), "/p256_mul.rs"));
}
#[cfg(feature = "arithmetic-campaign")]
mod production_batch {
    include!(concat!(env!("OUT_DIR"), "/batch_inverse.rs"));
}
#[cfg(feature = "arithmetic-campaign")]
mod production_fixed { include!(concat!(env!("OUT_DIR"), "/fixed_gf.rs")); }
#[cfg(feature = "arithmetic-campaign")]
mod production_ood { include!(concat!(env!("OUT_DIR"), "/ood.rs")); }
#[cfg(feature = "arithmetic-campaign")]
mod production_packing { include!(concat!(env!("OUT_DIR"), "/packing.rs")); }
#[cfg(feature = "arithmetic-campaign")]
mod utils { pub use f2z::utils::*; }
use flock_core::{field::F128, ntt::AdditiveNttF128};
use num_traits::Inv;
use std::{hint::black_box, time::Instant};
mod allocation;
mod arithmetic;
mod extra_cases;
#[cfg(feature = "legacy-matrix")]
mod matrix;
mod scalar_kernel;
use arithmetic::*;

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn values(&mut self, n: usize) -> Vec<F128> {
        (0..n)
            .map(|_| F128::new(self.next(), self.next()))
            .collect()
    }
}

/// Each candidate is called once per pass, not through a function pointer per term.
/// Inputs are immutable; a pass must overwrite its entire output or be pure.
pub struct Case<'a> {
    name: &'static str,
    bytes: usize,
    run: Box<dyn FnMut() + 'a>,
}
impl<'a> Case<'a> {
    pub fn new(name: &'static str, bytes: usize, run: impl FnMut() + 'a) -> Self {
        Self {
            name,
            bytes,
            run: Box::new(run),
        }
    }
}

pub fn measure(family: &str, size: &str, cases: &mut [Case<'_>], samples: usize, rng: &mut Rng) {
    if !case_requested(family, size) {
        return;
    }
    if let Ok(spec) = std::env::var("FIELD_REGRESSION_BASELINES") {
        let names: std::collections::HashMap<String, String> =
            serde_json::from_str(&spec).expect("invalid baseline map");
        if let Some(name) = names.get(family) {
            let index = cases
                .iter()
                .position(|c| c.name == name)
                .expect("required baseline missing");
            cases.swap(0, index);
        }
    }
    // Calibrate against the baseline; use identical iterations for every candidate.
    let mut iterations = 1usize;
    loop {
        let start = Instant::now();
        for _ in 0..iterations {
            (cases[0].run)();
        }
        if start.elapsed().as_secs_f64() >= 0.003 || iterations >= 1 << 20 {
            break;
        }
        iterations *= 2;
    }
    let allocations: Vec<_> = cases
        .iter_mut()
        .map(|case| allocation::audit(&mut *case.run))
        .collect();
    let mut order: Vec<usize> = (0..cases.len()).collect();
    for rep in 0..samples {
        for i in (1..order.len()).rev() {
            order.swap(i, rng.next() as usize % (i + 1));
        }
        for &i in &order {
            let case = &mut cases[i];
            let start = Instant::now();
            for _ in 0..iterations {
                (case.run)();
            }
            let ns = start.elapsed().as_nanos() as f64 / iterations as f64;
            println!(
                "{family},{size},{},{rep},{iterations},{ns:.3},{},{},{},{}",
                case.name,
                case.bytes,
                allocations[i].0,
                allocations[i].1,
                if case.name == "reset_only" { 0 } else { 1 }
            );
        }
    }
    eprintln!(
        "completed {family}/{size}: {} variants, {samples} paired samples",
        cases.len()
    );
}

pub fn case_requested(family: &str, size: &str) -> bool {
    static FILTER: std::sync::OnceLock<Option<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    match FILTER.get_or_init(|| {
        std::env::var("FIELD_REGRESSION_CASES")
            .ok()
            .map(|v| v.split(',').map(String::from).collect())
    }) {
        None => true,
        Some(cases) => cases.contains(&format!("{family}/{size}")),
    }
}

fn verify() {
    let mut rng = Rng(0x864acfea42);
    assert_eq!(F128::ZERO.inv(), F128::ZERO);
    assert_eq!(shared(F128::ZERO).inv(), None);
    let mut pairs = vec![
        (F128::ZERO, F128::ZERO),
        (F128::ONE, F128::ONE),
        (F128::new(u64::MAX, u64::MAX), F128::new(u64::MAX, u64::MAX)),
    ];
    for i in 0..128 {
        for j in 0..128 {
            pairs.push((from_u128(1u128 << i), from_u128(1u128 << j)));
        }
    }
    for _ in 0..4096 {
        pairs.push((
            F128::new(rng.next(), rng.next()),
            F128::new(rng.next(), rng.next()),
        ));
    }
    for (a, b) in pairs {
        let want = oracle(a, b);
        for kernel in KERNELS {
            assert_eq!(kernel.mul(a, b), want, "{} {a:?} {b:?}", kernel.name());
        }
        #[cfg(feature = "ntt-candidate")]
        {
            let result = flock_candidate::field::F128::new(a.lo, a.hi)
                * flock_candidate::field::F128::new(b.lo, b.hi);
            assert_eq!((result.lo, result.hi), (want.lo, want.hi));
        }
        assert_eq!(shared(a).square(), shared(oracle(a, a)));
        assert_eq!(field::F128::from_bytes(shared(a).to_bytes()), shared(a));
        verify_fixed(a, b);
    }
    for a in [
        F128::ONE,
        F128::new(u64::MAX, u64::MAX),
        F128::new(0, 1 << 63),
        F128::new(1 << 63, 0),
    ] {
        assert_eq!(oracle(a, a.inv()), F128::ONE);
        assert_eq!(shared(a).inv(), Some(shared(a.inv())));
    }
    for _ in 0..64 {
        let a = F128::new(rng.next(), rng.next());
        assert_eq!(shared(a).inv(), Some(shared(a.inv())));
    }
    for n in [0, 1, 2, 3, 7, 8, 15, 16, 17, 31, 32, 33, 255, 1024] {
        let (a, b) = (rng.values(n), rng.values(n));
        let want = dot::<Baseline>(&a, &b);
        assert_eq!(dot::<Shared>(&a, &b), want);
        assert_eq!(dot::<ScalarLanes>(&a, &b), want);
        assert_eq!(wide_dot::<1>(&a, &b), want);
        assert_eq!(wide_dot::<2>(&a, &b), want);
        assert_eq!(wide_dot::<4>(&a, &b), want);
        assert_eq!(wide_dot::<8>(&a, &b), want);
        assert_eq!(wide_dot::<16>(&a, &b), want);
        assert_eq!(vec2_dot(&a, &b), want);
        let products: Vec<_> = a.iter().zip(&b).map(|(&a, &b)| a * b).collect();
        let mut output = vec![F128::ZERO; n];
        shared_products::<2>(&a, &b, &mut output);
        assert_eq!(output, products);
        shared_products::<4>(&a, &b, &mut output);
        assert_eq!(output, products);
        shared_products::<8>(&a, &b, &mut output);
        assert_eq!(output, products);
    }
    eprintln!(
        "correctness: all basis products, 4096 random products, square/encoding/inversion, dot tails passed"
    );
}

fn arithmetic_benches(samples: usize, rng: &mut Rng) {
    for n in [16, 1024, 65536, 1_048_576] {
        if !case_requested("dot", &n.to_string()) && !case_requested("products", &n.to_string()) {
            continue;
        }
        let (a, b) = (rng.values(n), rng.values(n));
        let want = dot::<Baseline>(&a, &b);
        for actual in [
            dot::<Shared>(&a, &b),
            dot::<ScalarLanes>(&a, &b),
            wide_dot::<1>(&a, &b),
            wide_dot::<2>(&a, &b),
            wide_dot::<4>(&a, &b),
            wide_dot::<8>(&a, &b),
            wide_dot::<16>(&a, &b),
            vec2_dot(&a, &b),
        ] {
            assert_eq!(actual, want);
        }
        let bytes = n * 32;
        let mut cases = vec![
            Case::new("flock", bytes, || {
                black_box(dot::<Baseline>(black_box(&a), black_box(&b)));
            }),
            Case::new("shared", bytes, || {
                black_box(dot::<Shared>(black_box(&a), black_box(&b)));
            }),
            Case::new("scalar_lanes", bytes, || {
                black_box(dot::<ScalarLanes>(black_box(&a), black_box(&b)));
            }),
            Case::new("wide1", bytes, || {
                black_box(wide_dot::<1>(black_box(&a), black_box(&b)));
            }),
            Case::new("wide2", bytes, || {
                black_box(wide_dot::<2>(black_box(&a), black_box(&b)));
            }),
            Case::new("wide4", bytes, || {
                black_box(wide_dot::<4>(black_box(&a), black_box(&b)));
            }),
            Case::new("wide8", bytes, || {
                black_box(wide_dot::<8>(black_box(&a), black_box(&b)));
            }),
            Case::new("wide16", bytes, || {
                black_box(wide_dot::<16>(black_box(&a), black_box(&b)));
            }),
            Case::new("vec2", bytes, || {
                black_box(vec2_dot(black_box(&a), black_box(&b)));
            }),
        ];
        measure("dot", &n.to_string(), &mut cases, samples, rng);
        let want: Vec<_> = a.iter().zip(&b).map(|(&x, &y)| x * y).collect();
        let mut cases = Vec::new();
        for &kernel in KERNELS {
            let mut out = vec![F128::ZERO; n];
            kernel.products(&a, &b, &mut out);
            assert_eq!(out, want, "{}", kernel.name());
            let (a, b) = (&a, &b);
            cases.push(Case::new(kernel.name(), n * 48, move || {
                kernel.products(black_box(a), black_box(b), black_box(&mut out));
                black_box(&out);
            }));
        }
        for variant in 0..3 {
            let mut output = vec![F128::ZERO; n];
            let (a, b) = (&a, &b);
            let apply = move |output: &mut [F128]| match variant {
                0 => shared_products::<2>(black_box(a), black_box(b), output),
                1 => shared_products::<4>(black_box(a), black_box(b), output),
                2 => shared_products::<8>(black_box(a), black_box(b), output),
                _ => unreachable!(),
            };
            apply(&mut output);
            assert_eq!(output, want);
            cases.push(Case::new(
                ["shared_unroll2", "shared_unroll4", "shared_unroll8"][variant],
                n * 48,
                move || {
                    apply(black_box(&mut output));
                    black_box(&output);
                },
            ));
        }
        measure("products", &n.to_string(), &mut cases, samples, rng);
    }
    for n in [16, 1024, 65536, 1_048_576] {
        if !case_requested("chain", &n.to_string()) {
            continue;
        }
        let a = rng.values(n);
        let mut cases = Vec::new();
        for &kernel in KERNELS {
            assert_eq!(kernel.chain(&a), Kernel::Flock.chain(&a));
            let a = &a;
            cases.push(Case::new(kernel.name(), a.len() * 16, move || {
                black_box(kernel.chain(black_box(a)));
            }));
        }
        measure("chain", &n.to_string(), &mut cases, samples, rng);
    }
}

fn ntt_benches(samples: usize, rng: &mut Rng) {
    for (log, lanes) in [
        (8, 1),
        (8, 32),
        (12, 8),
        (15, 32),
        (16, 32),
        (17, 32),
        (18, 32),
    ] {
        let size = format!("log{log}_lanes{lanes}");
        if !case_requested("ntt", &size) {
            continue;
        }
        let start = Instant::now();
        let ntt = AdditiveNttF128::standard(log);
        eprintln!("NTT setup log={log}: {} ns", start.elapsed().as_nanos());
        let input = rng.values((1 << log) * lanes);
        let mut reference = input.clone();
        ntt.forward_transform_interleaved_scalar(&mut reference, lanes);
        #[cfg(feature = "ntt-candidate")]
        let candidate_ntt = {
            let start = Instant::now();
            let ntt = flock_candidate::ntt::AdditiveNttF128::standard(log);
            eprintln!(
                "NTT candidate setup log={log}: {} ns",
                start.elapsed().as_nanos()
            );
            ntt
        };
        #[cfg(feature = "ntt-candidate")]
        let candidate_input: Vec<_> = input
            .iter()
            .map(|v| flock_candidate::field::F128::new(v.lo, v.hi))
            .collect();
        let mut cases = Vec::new();
        for variant in 0..4 {
            let (ntt, input) = (&ntt, &input);
            let mut output = input.clone();
            transform(ntt, &mut output, lanes, variant);
            assert_eq!(
                output, reference,
                "NTT variant {variant} log={log} lanes={lanes}"
            );
            let name = [
                "flock",
                "shared_generic",
                "shared_twiddle",
                "prepared_twiddle",
            ][variant];
            cases.push(Case::new(name, input.len() * 32, move || {
                // Reset is deliberately included for ALL variants. Its cost is reported separately.
                output.copy_from_slice(black_box(input));
                transform(black_box(ntt), black_box(&mut output), lanes, variant);
                black_box(&output);
            }));
        }
        #[cfg(feature = "ntt-candidate")]
        {
            let mut output = candidate_input.clone();
            candidate_ntt.forward_transform_interleaved(&mut output, lanes);
            assert!(
                output
                    .iter()
                    .zip(&reference)
                    .all(|(a, b)| a.lo == b.lo && a.hi == b.hi),
                "candidate NTT mismatch"
            );
            let (candidate_ntt, candidate_input) = (&candidate_ntt, &candidate_input);
            cases.push(Case::new(
                "preserved_schedule",
                input.len() * 32,
                move || {
                    output.copy_from_slice(black_box(candidate_input));
                    candidate_ntt.forward_transform_interleaved(black_box(&mut output), lanes);
                    black_box(&output);
                },
            ));
        }
        let mut copy = input.clone();
        let input_ref = &input;
        cases.push(Case::new("reset_only", input.len() * 32, move || {
            copy.copy_from_slice(black_box(input_ref));
            black_box(&copy);
        }));
        measure(
            "ntt",
            &format!("log{log}_lanes{lanes}"),
            &mut cases,
            samples,
            rng,
        );
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("all");
    let samples = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(32);
    let seed = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(42);
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();
    eprintln!(
        "arch={} shared_kernel={} sha3={} seed={seed}",
        std::env::consts::ARCH,
        field::gf128::KERNEL,
        cfg!(target_feature = "sha3")
    );
    let baseline_pool = flock_core::all_core_pool().current_num_threads();
    #[cfg(feature = "ntt-candidate")]
    let candidate_pool = flock_candidate::all_core_pool().current_num_threads();
    #[cfg(not(feature = "ntt-candidate"))]
    let candidate_pool = 0usize;
    let runtime = serde_json::json!({"arch":std::env::consts::ARCH,"shared_kernel":field::gf128::KERNEL,"flock_kernel":if cfg!(target_arch="aarch64") { "arm-pmull" } else { "x86-pclmul-or-portable" },"candidate_mul_kernel":if cfg!(all(target_arch="aarch64",target_feature="aes")) { "arm-pmull-scalar-lanes" } else { field::gf128::KERNEL }, "arithmetic_campaign":cfg!(feature="arithmetic-campaign"),
        "aes":cfg!(target_feature="aes"),"sha3":cfg!(target_feature="sha3"),"pclmulqdq":cfg!(target_feature="pclmulqdq"),
        "avx512f":cfg!(target_feature="avx512f"),"vpclmulqdq":cfg!(target_feature="vpclmulqdq"),
        "caller_threads":rayon::current_num_threads(),"baseline_all_core_threads":baseline_pool,"candidate_all_core_threads":candidate_pool,
        "candidate_snapshot":cfg!(feature="ntt-candidate"),"seed":seed});
    eprintln!("RUNTIME {runtime}");
    verify();
    println!("family,size,variant,rep,iterations,ns,bytes,allocations,allocated_bytes,correctness");
    let mut rng = Rng(seed);
    if mode == "all" || mode == "gf" {
        arithmetic_benches(samples, &mut rng);
        extra_cases::run(samples, &mut rng);
        ntt_benches(samples, &mut rng);
    }
    #[cfg(feature = "arithmetic-campaign")]
    if mode == "arithmetic" {
        arithmetic_benches(samples, &mut rng);
        extra_cases::run(samples, &mut rng);
        campaign::run(samples, &mut rng);
    }
    #[cfg(feature = "arithmetic-campaign")]
    if mode == "integer-focus" {
        campaign::integer_focus(samples, &mut rng);
    }
    #[cfg(feature = "arithmetic-campaign")]
    if mode == "operation-metrics" {
        campaign::operation_metrics(samples, &mut rng);
    }
    #[cfg(feature = "arithmetic-campaign")]
    if mode == "optimization" {
        campaign::optimize(samples, &mut rng);
    }
    #[cfg(feature = "legacy-matrix")]
    if mode == "all" || mode == "matrix" {
        matrix::run(samples, &mut rng);
    }
    eprintln!("CORRECTNESS_COMPLETE");
}
