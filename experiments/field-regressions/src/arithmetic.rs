use field::Wide256;
use flock_core::{field::Gf128, ntt::AdditiveNttF128};

#[inline(always)]
pub fn shared(a: Gf128) -> field::Gf128 {
    field::Gf128::new(a.lo, a.hi)
}
#[inline(always)]
fn flock(a: field::Gf128) -> Gf128 {
    Gf128::new(a.lo, a.hi)
}
pub fn from_u128(a: u128) -> Gf128 {
    Gf128::new(a as u64, (a >> 64) as u64)
}

/// Independent bit-serial oracle: multiply in the quotient ring one bit at a time.
/// Test-only, intentionally not a constant-time implementation.
pub fn oracle(a: Gf128, b: Gf128) -> Gf128 {
    let mut a = (a.hi as u128) << 64 | a.lo as u128;
    let mut b = (b.hi as u128) << 64 | b.lo as u128;
    let mut out = 0;
    for _ in 0..128 {
        if b & 1 != 0 {
            out ^= a;
        }
        let top = a >> 127;
        a = (a << 1) ^ (0x87u128.wrapping_mul(top));
        b >>= 1;
    }
    from_u128(out)
}

pub trait Mul {
    fn mul(a: Gf128, b: Gf128) -> Gf128;
}
pub struct Baseline;
pub struct Shared;
pub struct ScalarLanes;
impl Mul for Baseline {
    #[inline(always)]
    fn mul(a: Gf128, b: Gf128) -> Gf128 {
        a * b
    }
}
impl Mul for Shared {
    #[inline(always)]
    fn mul(a: Gf128, b: Gf128) -> Gf128 {
        flock(shared(a) * shared(b))
    }
}
impl Mul for ScalarLanes {
    #[inline(always)]
    fn mul(a: Gf128, b: Gf128) -> Gf128 {
        flock(crate::scalar_kernel::mul(shared(a), shared(b)))
    }
}

#[inline(never)]
pub fn dot<M: Mul>(a: &[Gf128], b: &[Gf128]) -> Gf128 {
    assert_eq!(a.len(), b.len());
    let mut out = Gf128::ZERO;
    for (&a, &b) in a.iter().zip(b) {
        out += M::mul(a, b);
    }
    out
}

#[inline(never)]
pub fn wide_dot<const K: usize>(a: &[Gf128], b: &[Gf128]) -> Gf128 {
    assert_eq!(a.len(), b.len());
    let mut acc = [Wide256::zero(); K];
    let chunks = a.len() / K * K;
    for base in (0..chunks).step_by(K) {
        for j in 0..K {
            acc[j] += Wide256::mul(shared(a[base + j]), shared(b[base + j]));
        }
    }
    for i in chunks..a.len() {
        acc[0] += Wide256::mul(shared(a[i]), shared(b[i]));
    }
    let mut result = Wide256::zero();
    for item in acc {
        result += item;
    }
    flock(result.reduce())
}

/// Establish lengths once, expose K independent products, and handle the tail.
/// Slice chunks make the established bound visible to LLVM without unchecked access.
#[inline(never)]
pub fn shared_products<const K: usize>(a: &[Gf128], b: &[Gf128], out: &mut [Gf128]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    let mut ac = a.chunks_exact(K);
    let mut bc = b.chunks_exact(K);
    let mut oc = out.chunks_exact_mut(K);
    for ((a, b), out) in ac.by_ref().zip(bc.by_ref()).zip(oc.by_ref()) {
        for i in 0..K {
            out[i] = Shared::mul(a[i], b[i]);
        }
    }
    for ((&a, &b), out) in ac
        .remainder()
        .iter()
        .zip(bc.remainder())
        .zip(oc.into_remainder())
    {
        *out = Shared::mul(a, b);
    }
}

#[inline(never)]
pub fn vec2_dot(a: &[Gf128], b: &[Gf128]) -> Gf128 {
    assert_eq!(a.len(), b.len());
    let mut result = Gf128::ZERO;
    let n = a.len() / 2 * 2;
    for i in (0..n).step_by(2) {
        #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
        let v = unsafe {
            flock_core::field::gf128_kernels::aarch64::ghash_mul_vec2_neon(
                [a[i], a[i + 1]],
                [b[i], b[i + 1]],
            )
        };
        #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
        let v = [a[i] * b[i], a[i + 1] * b[i + 1]];
        result += v[0] + v[1];
    }
    if n < a.len() {
        result += a[n] * b[n];
    }
    result
}

#[derive(Clone, Copy)]
pub enum Kernel {
    Flock,
    Shared,
    ScalarLanes,
    Schoolbook,
    Karatsuba,
    KaratsubaBarrett,
}
pub const KERNELS: &[Kernel] = &[
    Kernel::Flock,
    Kernel::Shared,
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    Kernel::ScalarLanes,
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    Kernel::Schoolbook,
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    Kernel::Karatsuba,
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    Kernel::KaratsubaBarrett,
];
impl Kernel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Flock => "flock",
            Self::Shared => "shared",
            Self::ScalarLanes => "scalar_lanes",
            Self::Schoolbook => "schoolbook",
            Self::Karatsuba => "karatsuba",
            Self::KaratsubaBarrett => "karatsuba_barrett",
        }
    }
    #[inline(always)]
    pub fn mul(self, a: Gf128, b: Gf128) -> Gf128 {
        match self {
            Self::Flock => a * b,
            Self::Shared => Shared::mul(a, b),
            Self::ScalarLanes => ScalarLanes::mul(a, b),
            #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
            Self::Schoolbook => unsafe {
                flock_core::field::gf128_kernels::aarch64::ghash_mul_schoolbook(a, b)
            },
            #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
            Self::Karatsuba => unsafe {
                flock_core::field::gf128_kernels::aarch64::ghash_mul_karatsuba(a, b)
            },
            #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
            Self::KaratsubaBarrett => unsafe {
                flock_core::field::gf128_kernels::aarch64::ghash_mul_karatsuba_barrett(a, b)
            },
            #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
            _ => unreachable!("ARM candidate was not enabled"),
        }
    }
    #[inline(never)]
    pub fn products(self, a: &[Gf128], b: &[Gf128], out: &mut [Gf128]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), out.len());
        // Match once outside the hot loop; LLVM sees a constant kernel inside each arm.
        macro_rules! run {
            ($k:expr) => {
                for ((a, b), out) in a.iter().zip(b).zip(out) {
                    *out = $k.mul(*a, *b);
                }
            };
        }
        match self {
            Self::Flock => run!(Self::Flock),
            Self::Shared => run!(Self::Shared),
            Self::ScalarLanes => run!(Self::ScalarLanes),
            Self::Schoolbook => run!(Self::Schoolbook),
            Self::Karatsuba => run!(Self::Karatsuba),
            Self::KaratsubaBarrett => run!(Self::KaratsubaBarrett),
        }
    }
    #[inline(never)]
    pub fn chain(self, a: &[Gf128]) -> Gf128 {
        let mut out = Gf128::ONE;
        macro_rules! run {
            ($k:expr) => {
                for &a in a {
                    out = $k.mul(out, a);
                }
            };
        }
        match self {
            Self::Flock => run!(Self::Flock),
            Self::Shared => run!(Self::Shared),
            Self::ScalarLanes => run!(Self::ScalarLanes),
            Self::Schoolbook => run!(Self::Schoolbook),
            Self::Karatsuba => run!(Self::Karatsuba),
            Self::KaratsubaBarrett => run!(Self::KaratsubaBarrett),
        }
        out
    }
}

// Prepared fixed multiplication follows src/poly/univariate/binary_gf128.rs::FixedGfMul.
// Keeping it here lets the regression harness test preserving that specialization.
#[derive(Clone, Copy)]
pub struct Prepared {
    t: Gf128,
    rg: u128,
}
impl Prepared {
    #[inline(always)]
    pub fn new(t: Gf128) -> Self {
        Self {
            t,
            rg: clmul(t.hi, 0x87),
        }
    }
    #[inline(always)]
    pub fn mul(self, a: Gf128) -> Gf128 {
        let low = clmul(a.lo, self.t.lo) ^ clmul(a.hi, self.rg as u64);
        let mid = clmul(a.lo, self.t.hi) ^ clmul(a.hi, self.t.lo ^ (self.rg >> 64) as u64);
        from_u128(low ^ (mid << 64) ^ clmul((mid >> 64) as u64, 0x87))
    }
}
#[inline(always)]
pub(crate) fn clmul(a: u64, b: u64) -> u128 {
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    {
        unsafe { core::mem::transmute(core::arch::aarch64::vmull_p64(a, b)) }
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
    {
        // SAFETY: compile-time PCLMUL gate; register-only polynomial product.
        unsafe {
            use core::arch::x86_64::*;
            core::mem::transmute(_mm_clmulepi64_si128::<0>(
                _mm_set_epi64x(0, a as i64),
                _mm_set_epi64x(0, b as i64),
            ))
        }
    }
    #[cfg(not(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq")
    )))]
    {
        let mut out = 0u128;
        for i in 0..64 {
            out ^= ((a as u128) << i) & 0u128.wrapping_sub(((b >> i) & 1) as u128);
        }
        out
    }
}
#[inline(always)]
pub fn half_mul(a: Gf128, b: u64) -> Gf128 {
    let low = clmul(a.lo, b);
    let high = clmul(a.hi, b);
    from_u128(low ^ (high << 64) ^ clmul((high >> 64) as u64, 0x87))
}

pub fn verify_fixed(a: Gf128, b: Gf128) {
    assert_eq!(Prepared::new(b).mul(a), oracle(a, b));
    assert_eq!(half_mul(a, b.lo), oracle(a, Gf128::new(b.lo, 0)));
}

#[inline(never)]
fn generic_ntt<const SPECIAL: bool, const PREPARED: bool>(
    ntt: &AdditiveNttF128,
    data: &mut [Gf128],
    lanes: usize,
) {
    let log = (data.len() / lanes).ilog2() as usize;
    for layer in 0..log {
        let block_size = (1usize << (log - layer)) * lanes;
        for (block, values) in data.chunks_exact_mut(block_size).enumerate() {
            let t = ntt.twiddle(layer, block);
            let (top, bot) = values.split_at_mut(block_size / 2);
            if SPECIAL && t == Gf128::ZERO {
                for (a, b) in top.iter_mut().zip(bot) {
                    *b += *a;
                }
            } else if SPECIAL && t.hi == 0 {
                for (a, b) in top.iter_mut().zip(bot) {
                    *a += half_mul(*b, t.lo);
                    *b += *a;
                }
            } else if PREPARED {
                let prepared = Prepared::new(t);
                for (a, b) in top.iter_mut().zip(bot) {
                    *a += prepared.mul(*b);
                    *b += *a;
                }
            } else {
                for (a, b) in top.iter_mut().zip(bot) {
                    *a += Shared::mul(*b, t);
                    *b += *a;
                }
            }
        }
    }
}
pub fn transform(ntt: &AdditiveNttF128, data: &mut [Gf128], lanes: usize, variant: usize) {
    match variant {
        0 => ntt.forward_transform_interleaved(data, lanes),
        1 => generic_ntt::<false, false>(ntt, data, lanes),
        2 => generic_ntt::<true, false>(ntt, data, lanes),
        3 => generic_ntt::<true, true>(ntt, data, lanes),
        _ => unreachable!(),
    }
}

/// Build declared-width benchmark inputs; identical word-copy boundary to circuit packing.
pub fn integer_from_words<const L: usize>(words: &[u64]) -> field::Z<L> {
    field::Z::from_twos_complement_words(core::array::from_fn(|i| words.get(i).copied().unwrap_or(0)))
}
