//! Prover-only choices for the retained Falcon binding kernels.

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

    pub fn for_full(batch: usize, degree: usize, security_bits: usize, extension: usize) -> Self {
        Self::for_workload(batch, degree, security_bits == 100 && extension == 9)
    }

    fn for_workload(batch: usize, degree: usize, security_100: bool) -> Self {
        #[cfg(feature = "parallel")]
        let workers = rayon::current_num_threads();
        #[cfg(not(feature = "parallel"))]
        let workers = 1;
        let supported = cfg!(all(target_os = "linux", target_arch = "x86_64"))
            && batch == 1024
            && matches!(degree, 512 | 1024)
            && security_100
            && matches!(workers, 1 | 2 | 4 | 8 | 16);
        if supported || cfg!(test) {
            Self {
                optimized: true,
                group_cap: if degree == 512 { 16 } else { 8 },
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
