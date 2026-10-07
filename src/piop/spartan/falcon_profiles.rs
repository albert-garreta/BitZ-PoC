//! Compile-time Falcon configurations, independent of the BitZ sumcheck fields.
pub use super::falcon_parameters::{FalconParameters, FalconProtocol, auto_k, ring_budget_ok};
use super::falcon1024_ct::FalconError;
use std::{marker::PhantomData, ops::Deref};

pub trait FalconBackend {
    const N: usize;
    const K: usize;
    type Prepared;
    type Statement;
    fn prepare(
        batch: usize,
        bits: usize,
        protocol: FalconProtocol,
        max_batch: usize,
    ) -> Result<Self::Prepared, String>;
}
#[doc(hidden)]
pub struct BackendSelection<const N: usize, const K: usize>;
pub trait FalconProfile {
    const N: usize;
    const K: usize;
    const SECURITY_BITS: usize;
    const MAX_BATCH: usize;
    const PROTOCOL: FalconProtocol;
    type Backend: FalconBackend;
}
pub type FalconPublicStatement<P> = <<P as FalconProfile>::Backend as FalconBackend>::Statement;

pub struct PreparedFalconHybrid<P: FalconProfile> {
    inner: <P::Backend as FalconBackend>::Prepared,
    marker: PhantomData<P>,
}
impl<P: FalconProfile> PreparedFalconHybrid<P> {
    pub fn new(batch: usize) -> Result<Self, FalconError> {
        if batch == 0 || batch > P::MAX_BATCH {
            return Err(FalconError::InvalidBatchCapacity);
        }
        // Traits can be implemented without the macro: validate these inputs too.
        if P::N != P::Backend::N
            || P::K != P::Backend::K
            || !ring_budget_ok(P::N, P::K, P::SECURITY_BITS, P::MAX_BATCH, P::PROTOCOL)
        {
            return Err(FalconError::Piop(
                "profile misses ring security budget".into(),
            ));
        }
        let inner = P::Backend::prepare(batch, P::SECURITY_BITS, P::PROTOCOL, P::MAX_BATCH)
            .map_err(FalconError::Piop)?;
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
///     protocol:NativeCarry,ring_extension:Explicit(10),}}
/// ```
/// ```compile_fail
/// bitz::falcon_profile! {pub BadDegree {n:513,security_bits:100,max_batch:1024,
///     protocol:NativeCarry,ring_extension:Auto,}}
/// ```
#[macro_export]
macro_rules! falcon_profile {
    ($vis:vis $name:ident { n: $n:expr, security_bits: $bits:expr, max_batch: $batch:expr,
        protocol: $protocol:ident, ring_extension: Auto $(,)? }) => {
        $crate::falcon_profile!(@define $vis $name, $n, $bits, $batch, $protocol,
            $crate::piop::spartan::falcon_parameters::auto_k($n,$bits,$batch,
                $crate::piop::spartan::falcon_parameters::FalconProtocol::$protocol));
    };
    ($vis:vis $name:ident { n: $n:expr, security_bits: $bits:expr, max_batch: $batch:expr,
        protocol: $protocol:ident, ring_extension: Explicit($k:expr) $(,)? }) => {
        $crate::falcon_profile!(@define $vis $name, $n, $bits, $batch, $protocol, $k);
    };
    (@define $vis:vis $name:ident, $n:expr, $bits:expr, $batch:expr, $protocol:ident, $k:expr) => {
        const _: () = assert!($crate::piop::spartan::falcon_parameters::ring_budget_ok(
            $n,$k,$bits,$batch,$crate::piop::spartan::falcon_parameters::FalconProtocol::$protocol),
            "Falcon extension misses the requested ring security budget");
        $vis struct $name;
        impl $crate::piop::spartan::falcon_profiles::FalconProfile for $name {
            const N:usize=$n;const K:usize=$k;const SECURITY_BITS:usize=$bits;const MAX_BATCH:usize=$batch;
            const PROTOCOL:$crate::piop::spartan::falcon_parameters::FalconProtocol=
                $crate::piop::spartan::falcon_parameters::FalconProtocol::$protocol;
            type Backend=$crate::piop::spartan::falcon_profiles::BackendSelection<{$n},{$k}>;
        }
    };
}

// One production implementation, instantiated with concrete array dimensions.
// The historical fixture tests run only in falcon1024_ct; profile tests exercise
// the other instantiations without compiling those 1024-specific fixtures again.
macro_rules! backend {
    ($module:ident, $n:expr, $k:expr, $d:tt) => {
        #[doc(hidden)]
        #[allow(dead_code, unused_imports)]
        pub mod $module {
            const DEGREE: usize = $n;
            const EXTENSION: usize = $k;
            macro_rules! falcon_tests {
                ($d($d item:item)*) => {};
            }
            include!("falcon1024_ct/backend.rs");
        }
        impl FalconBackend for BackendSelection<$n, $k> {
            const N: usize = $n;
            const K: usize = $k;
            type Prepared = $module::PreparedFalconHybrid;
            type Statement = $module::FalconPublicStatement;
            fn prepare(
                batch: usize,
                bits: usize,
                protocol: FalconProtocol,
                max_batch: usize,
            ) -> Result<Self::Prepared, String> {
                $module::PreparedFalconHybrid::with_profile(batch, bits, protocol, max_batch)
                    .map_err(|e| e.to_string())
            }
        }
    };
}
backend!(n2_k8, 2, 8, $);
backend!(n2_k9, 2, 9, $);
backend!(n2_k10, 2, 10, $);
backend!(n2_k11, 2, 11, $);
backend!(n4_k8, 4, 8, $);
backend!(n4_k9, 4, 9, $);
backend!(n4_k10, 4, 10, $);
backend!(n4_k11, 4, 11, $);
backend!(n8_k8, 8, 8, $);
backend!(n8_k9, 8, 9, $);
backend!(n8_k10, 8, 10, $);
backend!(n8_k11, 8, 11, $);
backend!(n16_k8, 16, 8, $);
backend!(n16_k9, 16, 9, $);
backend!(n16_k10, 16, 10, $);
backend!(n16_k11, 16, 11, $);
backend!(n32_k8, 32, 8, $);
backend!(n32_k9, 32, 9, $);
backend!(n32_k10, 32, 10, $);
backend!(n32_k11, 32, 11, $);
backend!(n64_k8, 64, 8, $);
backend!(n64_k9, 64, 9, $);
backend!(n64_k10, 64, 10, $);
backend!(n64_k11, 64, 11, $);
backend!(n128_k8, 128, 8, $);
backend!(n128_k9, 128, 9, $);
backend!(n128_k10, 128, 10, $);
backend!(n128_k11, 128, 11, $);
backend!(n256_k8, 256, 8, $);
backend!(n256_k9, 256, 9, $);
backend!(n256_k10, 256, 10, $);
backend!(n256_k11, 256, 11, $);
backend!(n512_k8, 512, 8, $);
backend!(n512_k9, 512, 9, $);
backend!(n512_k10, 512, 10, $);
backend!(n512_k11, 512, 11, $);
backend!(n1024_k8, 1024, 8, $);
backend!(n1024_k9, 1024, 9, $);
backend!(n1024_k10, 1024, 10, $);
backend!(n1024_k11, 1024, 11, $);
crate::falcon_profile! { pub Falcon512_100 {n:512,security_bits:100,max_batch:1024,protocol:NativeCarry,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon512_128 {n:512,security_bits:128,max_batch:1024,protocol:NativeCarry,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon1024_100 {n:1024,security_bits:100,max_batch:1024,protocol:NativeCarry,ring_extension:Auto,} }
crate::falcon_profile! { pub Falcon1024_128 {n:1024,security_bits:128,max_batch:1024,protocol:NativeCarry,ring_extension:Auto,} }
