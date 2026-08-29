//! Step-3 random-prime projection for **extension-field** evaluation claims
//! (paper `c:core_iop`, Step 3).
//!
//! When the evaluation field `K = F_q[X]/(h(X))` is a proper extension
//! (degree `e ≥ 2`), the linear claim `⟨π_q(bits), v⟩ = μ` cannot ride the
//! `GF(2^128)` exponent fold directly: the lifted row weights
//! `π_canon^{-1}(v^{(1)})` are integer *polynomials*, not integers. The
//! paper's Step 3 collapses the claim to a prime field first:
//!
//! 1. the prover sends the exact integer-polynomial folds
//!    `μ_c = ⟨bits_c, π_canon^{-1}(v^{(1)})⟩ ∈ ℤ[X]` (Step 1; per-coefficient
//!    chunk folds in this implementation),
//! 2. the verifier samples a **random prime** `q'` from the set
//!    `𝒫 = {primes in [2^{bits−1}, 2^{bits})}` and a point `α' ∈ F_{q'}`,
//! 3. both sides project the row weights,
//!    `γ = π_{q'}^{-1}(π_{q',α'}(π_canon^{-1}(v^{(1)}))) ∈ [0, q')^{2^t}`,
//!    and run the ordinary mod-`q'` opening on `γ`; the verifier finally
//!    checks the certified folds against the Step-1 polynomials at `α'`:
//!    `⟨bits_c, γ⟩ ≡ μ_c(α') (mod q')`.
//!
//! A lie in the sent `μ_c` is a nonzero difference polynomial of degree
//! `< e` with `~(c_w+t+W)`-bit coefficients; by the generalized
//! Schwartz–Zippel / prime-divisibility argument (paper `l:reduction_lemma`)
//! it survives the random `(q', α')` with probability
//! `≈ B/(log q'·|𝒫|) + (e−1)/q'` — negligible at the default 100-bit primes.
//!
//! This module holds the pieces that are *new* relative to the prime-field
//! path: the transcript prime/point sampling (deterministic and identical on
//! both sides), fast mod-`q'` scalar arithmetic for a runtime modulus
//! (Montgomery via `crypto-bigint`'s [`FixedMontyForm`]), and the weight
//! projection `γ`. The opening itself reuses the existing mod-`q` pipeline
//! verbatim (see `ligerito_flock::prove_mle_eval_ext_ligerito`).

use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::transcript::traits::Transcript;
use crate::utils::cfg_into_iter;

use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
use crypto_bigint::{Odd, U128};
use crypto_primes::hazmat::MillerRabin;
use crypto_primes::{is_prime, Flavor};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Protocol parameters of the Step-3 projection. Prover and verifier must
/// agree on these (they are part of the protocol description, like the code
/// or the chunk width).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtProjParams {
    /// Bit-length of the sampled projection primes: `𝒫` is the set of
    /// primes in `[2^{prime_bits−1}, 2^{prime_bits})`. Also the `q_bits`
    /// under which the projected claim's chunk count is derived.
    pub prime_bits: usize,
    /// Number of transcript-derived Miller–Rabin bases (on top of a fixed
    /// base-2 pre-filter). A composite candidate survives all of them with
    /// probability ≤ `4^{-mr_rounds}` per tested composite, so 64 rounds push
    /// that candidate's acceptance probability to `2^{-128}`.
    pub mr_rounds: usize,
}

impl Default for ExtProjParams {
    fn default() -> Self {
        Self {
            prime_bits: 100,
            mr_rounds: 64,
        }
    }
}

impl ExtProjParams {
    /// Panic on parameter combinations the arithmetic below does not
    /// support. `prime_bits ≤ 120` keeps every modular value strictly below
    /// `2^126` (peasant doubling headroom and chunk-shift bounds);
    /// `≥ 32` keeps the candidate range clear of the tiny primes and the
    /// Miller–Rabin base range `[2, q'−2]` nonempty.
    pub fn validate(&self) {
        assert!(
            (32..=120).contains(&self.prime_bits),
            "ExtProjParams::prime_bits must be in [32, 120]; got {}",
            self.prime_bits
        );
        assert!(
            (1..=256).contains(&self.mr_rounds),
            "ExtProjParams::mr_rounds must be in [1, 256]; got {}",
            self.mr_rounds
        );
    }
}

/// One uniform 128-bit integer squeezed from the transcript (the two
/// little-endian words of a `GF(2^128)` challenge — the same encoding
/// [`crate::pcs::fq_challenge`] uses).
fn transcript_u128(transcript: &mut impl Transcript) -> u128 {
    let g: Gf = transcript.get_field_challenge(&());
    let w = g.words();
    u128::from(w[0]) | (u128::from(w[1]) << 64)
}

/// `(a · b) mod m` for an **arbitrary** modulus `m < 2^127` by
/// Russian-peasant doubling — the cold-path fallback used where `m` may be
/// even (Miller–Rabin base-range reduction). Hot paths use [`ProjArith`].
#[allow(clippy::arithmetic_side_effects)] // all values kept < 2^127 by the reductions
fn mulmod_generic(a: u128, b: u128, m: u128) -> u128 {
    debug_assert!(m != 0 && m < (1u128 << 127));
    let mut a = a % m;
    let mut b = b % m;
    let mut r = 0u128;
    while b != 0 {
        if b & 1 == 1 {
            let s = r + a; // r, a < m < 2^127: no overflow
            r = if s >= m { s - m } else { s };
        }
        let d = a + a;
        a = if d >= m { d - m } else { d };
        b >>= 1;
    }
    r
}

/// A 256-bit transcript draw reduced modulo `m` (statistical distance
/// `≤ m/2^256 ≈ 2^{-156}` from uniform at 100-bit `m` — the plain 128-bit
/// draw would be `~2^{-28}`-biased, which matters for the soundness-carrying
/// `α'` and the Miller–Rabin bases).
#[allow(clippy::arithmetic_side_effects)] // reductions keep everything < m < 2^127
fn transcript_uniform_mod(transcript: &mut impl Transcript, m: u128) -> u128 {
    debug_assert!(m > 1 && m < (1u128 << 127));
    let lo = transcript_u128(transcript);
    let hi = transcript_u128(transcript);
    // hi·2^128 + lo ≡ hi·r128 + lo (mod m), r128 = 2^128 mod m.
    let r128 = ((u128::MAX % m) + 1) % m;
    let hi_part = mulmod_generic(hi, r128, m);
    let s = hi_part + (lo % m);
    if s >= m {
        s - m
    } else {
        s
    }
}

/// The odd primes below 256, for the trial-division prefilter of
/// [`sample_proj_prime`]: ~76 % of odd candidates carry one of these
/// factors and are rejected by 53 `u128` remainders instead of a
/// Montgomery setup + modexp. (Candidates are `≥ 2^31`, so divisibility
/// by a table prime always means compositeness.)
const SMALL_PRIMES: [u128; 53] = [
    3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251,
];

/// Number of transcript-derived Miller--Rabin bases used by
/// [`sample_prime_in_interval`]. Exact-uniform base sampling makes the usual
/// strong-liar bound apply without a modulo-bias correction. Seventy-two
/// rounds give `2^-144` per tested composite; even after a union bound over
/// the sampler's fewer than `2^13` attempts at supported widths, the complete
/// invocation's false-prime probability remains below `2^-131`.
pub const TRANSCRIPT_PRIME_MR_ROUNDS: usize = 72;

/// Errors returned by the bounded transcript prime sampler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrimeSamplingError {
    #[error("prime interval is empty: min {min} exceeds max {max}")]
    InvalidInterval { min: u128, max: u128 },
    #[error("prime interval maximum must be below 2^126; got {max}")]
    MaximumTooLarge { max: u128 },
    #[error("prime interval [{min}, {max}] contains no odd candidate")]
    NoOddCandidate { min: u128, max: u128 },
    #[error("exact-uniform transcript sampling exhausted for upper bound {upper_bound}")]
    UniformSamplingExhausted { upper_bound: u128 },
    #[error("no prime found in [{min}, {max}] after {attempts} transcript candidates")]
    PrimeSearchExhausted {
        min: u128,
        max: u128,
        attempts: usize,
    },
}

/// Sample uniformly from `[0, upper_bound)` using full-width rejection
/// sampling. `2^128 mod upper_bound` is the size of the low prefix that must
/// be discarded; the remaining number of `u128` strings is an exact multiple
/// of `upper_bound`.
fn transcript_uniform_below_exact(
    transcript: &mut impl Transcript,
    upper_bound: u128,
) -> Result<u128, PrimeSamplingError> {
    debug_assert!(upper_bound != 0);
    // `wrapping_neg()` computes `2^128 - upper_bound`, so this remainder is
    // exactly `2^128 mod upper_bound` without representing `2^128`.
    let rejection_threshold = upper_bound.wrapping_neg() % upper_bound;
    // For callers in this module upper_bound < 2^126, hence every draw is
    // accepted with probability > 3/4. This cap is unreachable for an honest
    // random-oracle transcript but keeps the API total for adversarial test
    // transcript implementations.
    for _ in 0..256 {
        let draw = transcript_u128(transcript);
        if draw >= rejection_threshold {
            return Ok(draw % upper_bound);
        }
    }
    Err(PrimeSamplingError::UniformSamplingExhausted { upper_bound })
}

/// Run the deterministic prefilters and the soundness-carrying randomized
/// primality checks for one odd candidate.
fn transcript_candidate_is_prime(
    transcript: &mut impl Transcript,
    candidate: u128,
) -> Result<bool, PrimeSamplingError> {
    // Do not reject a table prime when a caller intentionally supplies a
    // small interval; only a proper multiple is known composite.
    if SMALL_PRIMES
        .iter()
        .any(|&small_prime| candidate != small_prime && candidate.is_multiple_of(small_prime))
    {
        return Ok(false);
    }

    let candidate_uint = U128::from_u128(candidate);
    // The preset is the improved BPSW check: Miller--Rabin base 2 followed by
    // the BPSW'21 Lucas test. The randomized rounds below provide the explicit
    // 128-bit composite-acceptance bound rather than relying on the BPSW
    // conjecture alone.
    if !is_prime(Flavor::Any, &candidate_uint) {
        return Ok(false);
    }

    // Random bases only make sense above 3. The BPSW check above handles 3
    // exactly, so the special case does not weaken the sampler.
    if candidate == 3 {
        return Ok(true);
    }
    let Some(odd_candidate) = Odd::new(candidate_uint).into_option() else {
        return Ok(false);
    };
    let mr = MillerRabin::new(odd_candidate);
    let base_count = candidate - 3; // bases [2, candidate - 2]
    for _ in 0..TRANSCRIPT_PRIME_MR_ROUNDS {
        let base = 2 + transcript_uniform_below_exact(transcript, base_count)?;
        if !mr.test(&U128::from_u128(base)).is_probably_prime() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Sample a prime uniformly from the odd primes in the inclusive interval
/// `[min_inclusive, max_inclusive]`.
///
/// Each odd candidate is drawn with exact-uniform rejection sampling (never a
/// `% interval_width` mapping), then filtered by small-prime division, the
/// improved base-2/BPSW test, and 72 exact-uniform transcript-derived
/// Miller--Rabin bases. Because every prime has the same acceptance
/// probability, conditioning on success leaves the output uniform over the
/// primes in the interval.
///
/// The upper bound is restricted to `< 2^126`, matching [`ProjArith`] and
/// leaving headroom for the generic modular arithmetic used by the checks.
/// The function is intentionally bounded and returns an error when the input
/// interval is invalid, contains no odd values, or yields no accepted prime.
#[allow(clippy::arithmetic_side_effects)] // validated interval keeps candidate arithmetic < 2^126
pub fn sample_prime_in_interval(
    transcript: &mut impl Transcript,
    min_inclusive: u128,
    max_inclusive: u128,
) -> Result<u128, PrimeSamplingError> {
    let _g = crate::utils::prof::scope("ext:sample_interval_prime");
    if min_inclusive > max_inclusive {
        return Err(PrimeSamplingError::InvalidInterval {
            min: min_inclusive,
            max: max_inclusive,
        });
    }
    if max_inclusive >= (1u128 << 126) {
        return Err(PrimeSamplingError::MaximumTooLarge { max: max_inclusive });
    }

    let first_odd = if min_inclusive & 1 == 1 {
        min_inclusive
    } else {
        min_inclusive + 1
    };
    let Some(last_odd) = (if max_inclusive & 1 == 1 {
        Some(max_inclusive)
    } else {
        max_inclusive.checked_sub(1)
    }) else {
        return Err(PrimeSamplingError::NoOddCandidate {
            min: min_inclusive,
            max: max_inclusive,
        });
    };
    if first_odd > last_odd {
        return Err(PrimeSamplingError::NoOddCandidate {
            min: min_inclusive,
            max: max_inclusive,
        });
    }

    let odd_candidate_count = ((last_odd - first_odd) >> 1) + 1;
    let candidate_bits = ((128 - max_inclusive.leading_zeros()) as usize).max(1);
    // A broad b-bit interval contains a prime after about b*ln(2)/2 odd
    // draws in expectation. 64*b retains the legacy sampler's conservative
    // cap while returning a typed error instead of reaching `unreachable!`.
    let max_attempts = if odd_candidate_count == 1 {
        1
    } else {
        64 * candidate_bits
    };
    for _ in 0..max_attempts {
        let candidate_index = transcript_uniform_below_exact(transcript, odd_candidate_count)?;
        let candidate = first_odd + 2 * candidate_index;
        if transcript_candidate_is_prime(transcript, candidate)? {
            return Ok(candidate);
        }
    }

    Err(PrimeSamplingError::PrimeSearchExhausted {
        min: min_inclusive,
        max: max_inclusive,
        attempts: max_attempts,
    })
}

/// Sample the Step-3 projection prime `q'` from the transcript: rejection-
/// sample candidates with the top bit and the low bit forced (so
/// `q' ∈ [2^{bits−1}, 2^{bits})`, odd), keep the first one that passes
/// trial division by [`SMALL_PRIMES`], Miller–Rabin with base 2, and
/// `mr_rounds` transcript-derived bases in `[2, q'−2]`. Deterministic in
/// the transcript state, so prover and verifier derive the same prime by
/// running the same code. (Trial division only removes composites, so the
/// sampled set is still exactly the primes of the range, uniformly.)
///
/// A composite acceptance needs every transcript base to be a Miller–Rabin
/// liar. Bases are single 128-bit draws reduced into the range — each
/// residue's probability exceeds uniform by a factor `≤ 1 + 2^{bits−128}`,
/// so acceptance is `≤ ((1 + 2^{bits−128})/4)^{mr_rounds} ≈ 4^{-mr_rounds}`
/// per candidate (`≈ 2^{-128}` at the defaults), and an adversary grinding
/// the Fiat–Shamir transcript gains only `queries · 4^{-mr_rounds}`.
#[allow(clippy::arithmetic_side_effects)] // candidate/base arithmetic bounded by 2^prime_bits < 2^121
pub fn sample_proj_prime(transcript: &mut impl Transcript, proj: &ExtProjParams) -> u128 {
    let _g = crate::utils::prof::scope("ext:sample_prime");
    proj.validate();
    let bits = proj.prime_bits;
    let top = 1u128 << (bits - 1);
    let mask = top | (top - 1);
    // Expected ~bits·ln2/2 ≈ 35 candidates at 100 bits; the hard cap is
    // astronomically unreachable (P ≈ e^{-2000}) and only bounds the loop.
    for _ in 0..64 * bits {
        let cand = (transcript_u128(transcript) & mask) | top | 1;
        if SMALL_PRIMES.iter().any(|&sp| cand.is_multiple_of(sp)) {
            continue;
        }
        let odd = Odd::new(U128::from_u128(cand)).expect("candidate is odd");
        let mr = MillerRabin::new(odd);
        if !mr.test_base_two().is_probably_prime() {
            continue;
        }
        // Base range [2, q'−2]: one draw reduced into [0, q'−4] (the
        // multiplicative-bias bound above), base = 2 + r.
        let range = cand - 3;
        let mut composite = false;
        for _ in 0..proj.mr_rounds {
            let base = 2 + transcript_u128(transcript) % range;
            if !mr.test(&U128::from_u128(base)).is_probably_prime() {
                composite = true;
                break;
            }
        }
        if !composite {
            return cand;
        }
    }
    unreachable!("no {bits}-bit prime in 64·{bits} transcript candidates");
}

/// Sample the Step-3 evaluation point `α' ∈ F_{q'}` from the transcript
/// (256-bit reduction — see [`transcript_uniform_mod`]).
pub fn sample_proj_point(transcript: &mut impl Transcript, q_proj: u128) -> u128 {
    transcript_uniform_mod(transcript, q_proj)
}

/// The low 128 bits of a `U128` as a `u128`.
fn u128_from_uint(x: &U128) -> u128 {
    let w = x.to_words();
    u128::from(w[0]) | (u128::from(w[1]) << 64)
}

/// Scalar arithmetic modulo the sampled (odd) prime `q'`, Montgomery-backed
/// for the hot loops (the `O(2^t)` weight projection and the `O(2^s)`
/// per-column congruence checks). Values enter and leave in canonical
/// `[0, q')` form (inputs are reduced on entry).
pub struct ProjArith {
    params: FixedMontyParams<{ U128::LIMBS }>,
    q: u128,
}

impl ProjArith {
    /// Context for an odd modulus `2 < q' < 2^126`.
    pub fn new(q_proj: u128) -> Self {
        assert!(
            q_proj > 2 && q_proj & 1 == 1,
            "projection modulus must be odd and > 2"
        );
        assert!(
            q_proj < (1u128 << 126),
            "projection modulus must be < 2^126"
        );
        let odd = Odd::new(U128::from_u128(q_proj)).expect("modulus is odd");
        Self {
            params: FixedMontyParams::new_vartime(odd),
            q: q_proj,
        }
    }

    /// The modulus `q'`.
    pub fn q(&self) -> u128 {
        self.q
    }

    /// `x mod q'`. Already-canonical values (the hot-loop common case:
    /// Montgomery outputs, 64-bit coordinates under a 100-bit modulus)
    /// skip the u128 division entirely.
    pub fn reduce(&self, x: u128) -> u128 {
        if x < self.q {
            x
        } else {
            x % self.q
        }
    }

    fn to_monty(&self, x: u128) -> FixedMontyForm<{ U128::LIMBS }> {
        FixedMontyForm::new(&U128::from_u128(self.reduce(x)), &self.params)
    }

    fn from_monty(m: &FixedMontyForm<{ U128::LIMBS }>) -> u128 {
        let w = m.retrieve().to_words();
        u128::from(w[0]) | (u128::from(w[1]) << 64)
    }

    /// A canonical value converted ONCE into Montgomery form, for use as the
    /// fixed factor of many [`Self::mul_plain_by`] calls (power tables).
    pub fn monty_factor(&self, x: u128) -> FixedMontyForm<{ U128::LIMBS }> {
        self.to_monty(x)
    }

    /// `(a · x) mod q'` for plain `a < 2^127` against a prepared
    /// [`Self::monty_factor`] — **one** Montgomery multiplication, no
    /// domain conversions: interpreting plain `a` as a Montgomery residue
    /// makes the reduction built into the multiply land the product back
    /// in plain form (`mont_mul(a, x·R) = a·x·R·R⁻¹ = a·x mod q'`).
    pub fn mul_plain_by(&self, a: u128, x_monty: &FixedMontyForm<{ U128::LIMBS }>) -> u128 {
        let a_form = FixedMontyForm::from_montgomery(U128::from_u128(self.reduce(a)), &self.params);
        u128_from_uint(&(a_form * x_monty).to_montgomery())
    }

    /// `(a · b) mod q'` (inputs reduced on entry).
    pub fn mul(&self, a: u128, b: u128) -> u128 {
        Self::from_monty(&(self.to_monty(a) * self.to_monty(b)))
    }

    /// `(a + b) mod q'` (inputs reduced on entry; sums stay < 2^127).
    #[allow(clippy::arithmetic_side_effects)] // both summands < q' < 2^126
    pub fn add(&self, a: u128, b: u128) -> u128 {
        let s = self.reduce(a) + self.reduce(b);
        if s >= self.q {
            s - self.q
        } else {
            s
        }
    }

    /// The canonical power ladder `[1, x, x², …, x^{n−1}] mod q'`.
    pub fn powers(&self, x: u128, n: usize) -> Vec<u128> {
        let mut out = Vec::with_capacity(n);
        let mut cur = self.reduce(1);
        for _ in 0..n {
            out.push(cur);
            cur = self.mul(cur, x);
        }
        out
    }
}

/// The projected row weights of Step 3:
/// `γ_b = π_{q'}^{-1}(π_{q',α'}(π_canon^{-1}(v^{(1)})_b)) = (Σ_d coords[d][b]·α'^d) mod q'`,
/// from the coordinate-major integer lift `coords[d][b] ∈ [0, q)` of
/// `v^{(1)} ∈ K^{2^t}` (coordinate `d` in the module basis `1, X, …, X^{e−1}`).
/// This is the extension-field replacement for the plain canonical lift the
/// prime-field path feeds to the chunker.
pub fn projected_row_weights(coords: &[Vec<u128>], q_proj: u128, alpha_proj: u128) -> Vec<u128> {
    let _g = crate::utils::prof::scope("ext:project");
    let ext_deg = coords.len();
    assert!(ext_deg >= 1, "at least one coordinate vector");
    let rows = coords[0].len();
    for c in coords {
        assert_eq!(c.len(), rows, "coordinate vectors must share the row count");
    }
    let zq = ProjArith::new(q_proj);
    // α'^d as prepared Montgomery factors (d ≥ 1; the d = 0 term is the
    // plain coordinate itself) — each row term is then ONE Montgomery
    // multiplication via the plain×monty trick, no domain conversions.
    let pow_monty: Vec<FixedMontyForm<{ U128::LIMBS }>> = zq
        .powers(alpha_proj, ext_deg)
        .into_iter()
        .skip(1)
        .map(|p| zq.monty_factor(p))
        .collect();
    cfg_into_iter!(0..rows)
        .map(|b| {
            let mut acc = zq.reduce(coords[0][b]);
            for (d, pw) in pow_monty.iter().enumerate() {
                acc = zq.add(acc, zq.mul_plain_by(coords[d.wrapping_add(1)][b], pw));
            }
            acc
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    /// Reference naive primality by trial division (test-only, small inputs).
    fn is_prime_naive(n: u128) -> bool {
        if n < 2 {
            return false;
        }
        let mut d = 2u128;
        while d * d <= n {
            if n % d == 0 {
                return false;
            }
            d += 1;
        }
        true
    }

    #[test]
    fn proj_arith_matches_naive() {
        let q = (1u128 << 100) - 15; // prime
        let zq = ProjArith::new(q);
        let a = 0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABCu128;
        let b = 0x0123_4567_89AB_CDEF_0011_2233_4455u128;
        assert_eq!(zq.mul(a, b), mulmod_generic(a, b, q));
        assert_eq!(zq.add(a, b), (a % q + b % q) % q);
        let pows = zq.powers(a, 4);
        assert_eq!(pows[0], 1);
        assert_eq!(pows[1], a % q);
        assert_eq!(pows[2], mulmod_generic(a, a, q));
        assert_eq!(pows[3], mulmod_generic(pows[2], a, q));
    }

    /// Prime sampling is deterministic in the transcript state, lands in the
    /// declared range, and (at a test-sized 40 bits) is actually prime.
    #[test]
    fn sampled_prime_is_prime_and_deterministic() {
        let proj = ExtProjParams {
            prime_bits: 40,
            mr_rounds: 8,
        };
        let mut t1 = Blake3Transcript::new();
        t1.absorb_slice(b"ext-proj-test");
        let q1 = sample_proj_prime(&mut t1, &proj);
        let mut t2 = Blake3Transcript::new();
        t2.absorb_slice(b"ext-proj-test");
        let q2 = sample_proj_prime(&mut t2, &proj);
        assert_eq!(q1, q2, "sampling must be deterministic in the transcript");
        assert!(q1 >= (1u128 << 39) && q1 < (1u128 << 40), "prime in range");
        assert!(is_prime_naive(q1), "sampled candidate must be prime");
        // The point lands in [0, q').
        let a1 = sample_proj_point(&mut t1, q1);
        let a2 = sample_proj_point(&mut t2, q2);
        assert_eq!(a1, a2);
        assert!(a1 < q1);
    }

    #[test]
    fn interval_prime_is_deterministic_and_in_range() {
        let min = (1u128 << 31) + 123;
        let max = (1u128 << 32) - 57;
        let mut t1 = Blake3Transcript::new();
        t1.absorb_slice(b"bounded-prime-test");
        let q1 = sample_prime_in_interval(&mut t1, min, max).expect("interval contains primes");
        let mut t2 = Blake3Transcript::new();
        t2.absorb_slice(b"bounded-prime-test");
        let q2 = sample_prime_in_interval(&mut t2, min, max).expect("interval contains primes");

        assert_eq!(q1, q2, "sampling must be deterministic in the transcript");
        assert!((min..=max).contains(&q1));
        assert_eq!(q1 & 1, 1);
        assert!(is_prime_naive(q1));
    }

    #[test]
    fn interval_sampler_handles_single_small_prime() {
        // 251 is in SMALL_PRIMES and must not be mistaken for a proper
        // multiple of itself by the trial-division filter.
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"singleton-small-prime");
        assert_eq!(sample_prime_in_interval(&mut transcript, 251, 251), Ok(251));

        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"small-prime-three");
        assert_eq!(sample_prime_in_interval(&mut transcript, 2, 3), Ok(3));
    }

    #[test]
    fn interval_sampler_rejects_composites() {
        // A strong base-2 pseudoprime whose factors are larger than the small
        // sieve table exercises the BPSW rejection rather than trial division.
        let pseudoprime = 341_550_071_728_321u128;
        let mut filter_transcript = Blake3Transcript::new();
        filter_transcript.absorb_slice(b"bpsw-composite");
        assert_eq!(
            transcript_candidate_is_prime(&mut filter_transcript, pseudoprime),
            Ok(false)
        );

        let mut sample_transcript = Blake3Transcript::new();
        sample_transcript.absorb_slice(b"singleton-composite");
        assert_eq!(
            sample_prime_in_interval(&mut sample_transcript, pseudoprime, pseudoprime),
            Err(PrimeSamplingError::PrimeSearchExhausted {
                min: pseudoprime,
                max: pseudoprime,
                attempts: 1,
            })
        );
    }

    #[test]
    fn interval_sampler_reports_invalid_ranges() {
        let mut transcript = Blake3Transcript::new();
        assert_eq!(
            sample_prime_in_interval(&mut transcript, 12, 10),
            Err(PrimeSamplingError::InvalidInterval { min: 12, max: 10 })
        );
        assert_eq!(
            sample_prime_in_interval(&mut transcript, 2, 2),
            Err(PrimeSamplingError::NoOddCandidate { min: 2, max: 2 })
        );
        assert_eq!(
            sample_prime_in_interval(&mut transcript, 3, 1u128 << 126),
            Err(PrimeSamplingError::MaximumTooLarge { max: 1u128 << 126 })
        );
    }

    /// γ agrees with a naive per-row Horner evaluation.
    #[test]
    fn projected_weights_match_horner() {
        let q = 0x0000_00E8_D4A5_1027u128; // 10^12 + 39, prime
        assert!(is_prime_naive(q));
        let alpha = 0x1234_5678u128 % q;
        let coords: Vec<Vec<u128>> = (0..3)
            .map(|d: u128| {
                (0..16)
                    .map(|b: u128| (b + 1) * (d + 2) * 0x9E37_79B9 % (1u128 << 40))
                    .collect()
            })
            .collect();
        let gamma = projected_row_weights(&coords, q, alpha);
        for b in 0..16 {
            let mut acc = 0u128;
            for d in (0..3).rev() {
                acc = (mulmod_generic(acc, alpha, q) + coords[d][b] % q) % q;
            }
            assert_eq!(gamma[b], acc, "row {b}");
        }
    }
}
