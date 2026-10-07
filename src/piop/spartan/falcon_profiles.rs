//! Compile-time Falcon configurations, independent of the BitZ sumcheck fields.
use super::falcon::FalconError;
pub use super::falcon_parameters::{FalconParameters, auto_k, ring_budget_ok};
#[cfg(feature = "falcon-hybrid")]
use std::{marker::PhantomData, ops::Deref};

pub trait FalconBackend {
    const N: usize;
    const K: usize;
    #[cfg(feature = "falcon-hybrid")]
    type Prepared;
    #[cfg(feature = "falcon-hybrid")]
    type Statement;
    #[cfg(feature = "falcon-hybrid")]
    fn prepare(batch: usize, bits: usize, max_batch: usize) -> Result<Self::Prepared, FalconError>;
}
#[doc(hidden)]
pub struct BackendSelection<const N: usize, const K: usize>;
pub trait FalconProfile {
    const N: usize;
    const K: usize;
    const SECURITY_BITS: usize;
    const MAX_BATCH: usize;
    type Backend: FalconBackend;
}
#[cfg(feature = "falcon-hybrid")]
pub type FalconPublicStatement<P> = <<P as FalconProfile>::Backend as FalconBackend>::Statement;

#[cfg(feature = "falcon-hybrid")]
pub struct PreparedFalconHybrid<P: FalconProfile> {
    inner: <P::Backend as FalconBackend>::Prepared,
    marker: PhantomData<P>,
}
#[cfg(feature = "falcon-hybrid")]
impl<P: FalconProfile> PreparedFalconHybrid<P> {
    pub fn new(batch: usize) -> Result<Self, FalconError> {
        if batch == 0 || batch > P::MAX_BATCH {
            return Err(FalconError::InvalidBatchCapacity);
        }
        // Traits can be implemented without the macro: validate these inputs too.
        if P::N != P::Backend::N
            || P::K != P::Backend::K
            || !ring_budget_ok(P::N, P::K, P::SECURITY_BITS, P::MAX_BATCH)
        {
            return Err(FalconError::Piop(
                "profile misses ring security budget".into(),
            ));
        }
        let inner = P::Backend::prepare(batch, P::SECURITY_BITS, P::MAX_BATCH)?;
        Ok(Self {
            inner,
            marker: PhantomData,
        })
    }
    pub const fn degree(&self) -> usize {
        P::N
    }
    pub const fn extension_degree(&self) -> usize {
        P::K
    }
}
#[cfg(feature = "falcon-hybrid")]
impl<P: FalconProfile> Deref for PreparedFalconHybrid<P> {
    type Target = <P::Backend as FalconBackend>::Prepared;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Declare a profile. Invalid dimensions and insufficient fields fail in const evaluation.
///
/// ```compile_fail
/// bitz::falcon_profile! {pub TooSmall {n:512,security_bits:128,max_batch:1024,
///     ring_extension:Explicit(10),}}
/// ```
/// ```compile_fail
/// bitz::falcon_profile! {pub BadDegree {n:513,security_bits:100,max_batch:1024,
///     ring_extension:Auto,}}
/// ```
#[macro_export]
macro_rules! falcon_profile {
    ($vis:vis $name:ident { n: $n:expr, security_bits: $bits:expr, max_batch: $batch:expr,
        ring_extension: Auto $(,)? }) => {
        $crate::falcon_profile!(@define $vis $name, $n, $bits, $batch,
            $crate::piop::spartan::falcon_parameters::auto_k($n,$bits,$batch));
    };
    ($vis:vis $name:ident { n: $n:expr, security_bits: $bits:expr, max_batch: $batch:expr,
        ring_extension: Explicit($k:expr) $(,)? }) => {
        $crate::falcon_profile!(@define $vis $name, $n, $bits, $batch, $k);
    };
    (@define $vis:vis $name:ident, $n:expr, $bits:expr, $batch:expr, $k:expr) => {
        const _: () = assert!($crate::piop::spartan::falcon_parameters::ring_budget_ok(
            $n,$k,$bits,$batch),
            "Falcon extension misses the requested ring security budget");
        $vis struct $name;
        impl $crate::piop::spartan::falcon_profiles::FalconProfile for $name {
            const N:usize=$n;const K:usize=$k;const SECURITY_BITS:usize=$bits;const MAX_BATCH:usize=$batch;
            type Backend=$crate::piop::spartan::falcon_profiles::BackendSelection<{$n},{$k}>;
        }
    };
}

// One production implementation, instantiated with concrete array dimensions.
// The degree-1024 fixture tests run once; profile tests exercise
// the other instantiations without compiling those 1024-specific fixtures again.
macro_rules! backend {
    ($module:ident, $n:expr, $k:expr, $tests:meta, $d:tt) => {
        #[doc(hidden)]
        pub mod $module {
            const DEGREE: usize = $n;
            const EXTENSION: usize = $k;
            macro_rules! falcon_tests {
                                ($d($d item:item)*) => { $d(#[cfg($tests)] $d item)* };
                            }
            include!("falcon/backend.rs");
        }
        impl FalconBackend for BackendSelection<$n, $k> {
            const N: usize = $n;
            const K: usize = $k;
            #[cfg(feature = "falcon-hybrid")]
            type Prepared = $module::PreparedFalconHybrid;
            #[cfg(feature = "falcon-hybrid")]
            type Statement = $module::FalconPublicStatement;
            #[cfg(feature = "falcon-hybrid")]
            fn prepare(
                batch: usize,
                bits: usize,
                max_batch: usize,
            ) -> Result<Self::Prepared, FalconError> {
                $module::PreparedFalconHybrid::prepare(batch, bits, max_batch)
            }
        }
    };
}
backend!(n512_k9, 512, 9, any(), $);
backend!(n512_k10, 512, 10, any(), $);
backend!(n512_k11, 512, 11, any(), $);
backend!(n1024_k9, 1024, 9, any(), $);
backend!(n1024_k10, 1024, 10, any(), $);
backend!(n1024_k11, 1024, 11, test, $);
crate::falcon_profile! { pub Falcon512_100 {n:512,security_bits:100,max_batch:1024,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon512_128 {n:512,security_bits:128,max_batch:1024,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon1024_100 {n:1024,security_bits:100,max_batch:1024,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon1024_128 {n:1024,security_bits:128,max_batch:1024,ring_extension:Auto,} }
