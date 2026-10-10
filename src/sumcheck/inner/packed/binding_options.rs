//! Prover-only choices for the retained Falcon binding kernels.

/// Benchmark switches affect only equivalent arithmetic implementations. In
/// production builds each kernel keeps its measured workload policy.
pub(crate) enum BindingKernel {
    Replay,
    Construction,
    Evaluation,
}

impl BindingKernel {
    pub fn enabled(self, baseline: bool) -> bool {
        #[cfg(feature = "bench-internals")]
        {
            use std::sync::OnceLock;

            static OVERRIDES: OnceLock<[Option<bool>; 3]> = OnceLock::new();
            let overrides = OVERRIDES.get_or_init(|| {
                [
                    "BITZ_BENCH_BINDER_REPLAY",
                    "BITZ_BENCH_BINDER_CONSTRUCTION",
                    "BITZ_BENCH_BINDER_EVALUATION",
                ]
                .map(|name| match std::env::var(name) {
                    Err(std::env::VarError::NotPresent) => None,
                    Ok(value) if value == "auto" => None,
                    Ok(value) if value == "on" => Some(true),
                    Ok(value) if value == "off" => Some(false),
                    _ => panic!("{name} must be auto, on, or off"),
                })
            });
            let index = match self {
                Self::Replay => 0,
                Self::Construction => 1,
                Self::Evaluation => 2,
            };
            overrides[index].unwrap_or(baseline)
        }
        #[cfg(not(feature = "bench-internals"))]
        baseline
    }
}

#[cfg(feature = "bench-internals")]
fn benchmark_group_cap(baseline: usize) -> usize {
    use std::sync::OnceLock;

    static OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
    OVERRIDE
        .get_or_init(|| match std::env::var("BITZ_BENCH_BINDER_GROUP_CAP") {
            Err(std::env::VarError::NotPresent) => None,
            Ok(value) if value == "auto" => None,
            Ok(value) => Some(
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|cap| *cap > 0)
                    .expect("BITZ_BENCH_BINDER_GROUP_CAP must be auto or a positive integer"),
            ),
            Err(_) => panic!("BITZ_BENCH_BINDER_GROUP_CAP must be auto or a positive integer"),
        })
        .unwrap_or(baseline)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BindingOptions {
    pub optimized: bool,
    pub group_cap: usize,
}

impl BindingOptions {
    pub const REFERENCE: Self = Self {
        optimized: false,
        group_cap: 1,
    };

    pub fn for_arithmetic(batch: usize, degree: usize, ungrinded_binding: bool) -> Self {
        // Of the supported 100/128-bit schedules, only 100 bits has no binding grind.
        Self::for_workload(
            batch,
            degree,
            false,
            if ungrinded_binding { 100 } else { 128 },
        )
    }

    pub fn for_full(batch: usize, degree: usize, security_bits: usize, extension: usize) -> Self {
        let security = match (security_bits, extension) {
            (100, 9) => 100,
            (128, _) => 128,
            _ => 0,
        };
        Self::for_workload(batch, degree, true, security)
    }

    fn for_workload(batch: usize, degree: usize, full: bool, security_bits: usize) -> Self {
        #[cfg(feature = "parallel")]
        let workers = rayon::current_num_threads();
        #[cfg(not(feature = "parallel"))]
        let workers = 1;
        let supported = cfg!(all(target_os = "linux", target_arch = "x86_64"))
            && batch == 1024
            && matches!(degree, 512 | 1024)
            && matches!(workers, 1 | 2 | 4 | 8 | 16)
            // At 128 bits, degree 1024 remains qualified only on one core.
            && (security_bits == 100
                || (security_bits == 128 && (degree == 512 || workers == 1)));
        if BindingKernel::Replay.enabled(supported || cfg!(test)) {
            let group_cap = if degree == 512 || security_bits == 128 {
                16
            } else {
                8
            };
            #[cfg(feature = "bench-internals")]
            let group_cap = benchmark_group_cap(group_cap);
            Self {
                optimized: true,
                // Arithmetic has no grouped adapter and keeps one task per signature.
                group_cap: if full { group_cap } else { 1 },
            }
        } else {
            Self::REFERENCE
        }
    }
}

impl Default for BindingOptions {
    fn default() -> Self {
        if cfg!(test) {
            Self {
                optimized: true,
                group_cap: 8,
            }
        } else {
            Self::REFERENCE
        }
    }
}
