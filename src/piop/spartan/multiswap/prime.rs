//! Transcript-derived prime-field parameters for the MultiSwap profile.
//!
//! Two primes, following the paper's Strategy 2 (\S{}Instantiation): run the
//! whole PIOP over a large fingerprint field and concentrate the (small)
//! grinding budget on the single Step 5.0 modulus-reduction draw.
//!
//! **Fingerprint prime `Q`** — sampled from `[2^127, 2^128)` after the F2Z
//! commitment is bound into the transcript (the Zaratan order).  Spartan and
//! the integer-to-field projection run over `F_Q`.
//!
//! **Reduction prime `q'`** — Step 5.0 (paper Remark "Description of
//! Step 5.0"): the terminal bitified tensor claim over `F_Q` is lifted to an
//! exact integer claim and re-projected onto a fresh prime `q'` sampled from
//! `[2^112, 2^113)`, the largest interval the one-chunk exponent-fold
//! geometry admits (`c_w = 127 - t - W = 113` at `t = 13`).  Only this draw
//! is grinded.
//!
//! # Soundness accounting (target: 114 bits, Limber parity)
//!
//! Every committed value is a nonnegative integer below `2^2048`, matrix
//! coefficients are below `2^2048`, and a live row has at most 353 entries
//! per matrix, so a nonzero integer row defect is below `2^8210`.
//!
//! - **Fingerprint draw**: a fixed nonzero defect has at most
//!   `floor(8210/127) = 64` prime divisors in `[2^127, 2^128)`, an interval
//!   holding more than `2^120.0` primes (Rosser bounds), so one draw hits a
//!   bad prime with probability at most `64 / 2^120.0 < 2^-114.0`.  No
//!   grinding needed.
//! - **Spartan rounds over `F_Q`**: each cubic/quadratic round message and
//!   each batching challenge carries error at most `3/Q <= 2^-125.4`.  No
//!   grinding needed — this is what the large field buys.
//! - **Step 5.0 reduction draw**: the lifted integer `mu'` and the true
//!   tensor evaluation are both below `d * Q^2 <= 2^{25+256}`, so their
//!   difference has at most `floor(282/112) = 2` prime divisors in
//!   `[2^112, 2^113)`, an interval holding more than `2^105.2` primes: at
//!   most `2^-104.2` per draw, topped up by
//!   [`MultiswapPrimeProfile::reduction_grinding_bits`] = 10 bits of
//!   proof-of-work to `2^-114.2`.
//! - **F2Z opening at `q'`**: the exponent-fold GKR rounds live in
//!   `GF(2^128)` (`<= 3/2^128` each) and the Ligerito/ring-switch layers use
//!   the 128-bit paper profile with its own internal query grinding.
//!
//! Every step is at or below `2^-114`, matching the floors Limber's own
//! implementation accepts (`LAMBDA_BOUND2 = 117`, fingerprint `~2^-114`) —
//! at a total grinding cost of `2^10` hashes instead of the earlier
//! single-prime profile's `2^21 + 2^18`.

use crypto_primes::{Flavor, is_prime};
use crypto_primitives::{PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint};
use thiserror::Error;

use crate::{
    ext_proj::{PrimeSamplingError, sample_prime_in_interval},
    piop::spartan::{SpartanField, absorb_spartan_message},
    poly::univariate::binary_gf128::BinaryFieldGF128,
    transcript::traits::Transcript,
};

use super::super::SpartanF2zField;

const FINGERPRINT_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-multiswap/fingerprint-prime/v2";
const REDUCTION_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-multiswap/reduction-prime/v2";

/// Draw attempts before concluding the transcript is adversarial: a random
/// 128-bit odd integer is prime with probability about `2/(127 ln 2)`.
const FINGERPRINT_SAMPLING_ATTEMPTS: usize = 64 * 128;

use crate::piop::spartan::profile::{IopInstanceFacts, IopSecurityParams};

/// `⌈log₂⌉` bound on any nonzero integer row defect of the wired MultiSwap
/// Mod-R1CS: values and matrix coefficients are `< 2^2048` and a live row
/// has at most 353 entries per matrix, so
/// `|(Az)_i (Bz)_i - (Cz)_i| < (353·2^4096)² < 2^8210`.
pub const MULTISWAP_DEFECT_LOG2_BOUND: u32 = 8210;

/// `⌈log₂⌉` bound on the Step-5.0 lift difference: the lifted integer and
/// the true tensor evaluation are both `< d·Q² <= 2^(25+256)`, so their
/// difference is `< 2^282`.
pub const MULTISWAP_STEP50_MAGNITUDE_LOG2: u32 = 282;

/// The public statement facts the security-profile derivation consumes for
/// a MultiSwap instance with the given F2Z shape and τ arity.
pub const fn multiswap_instance_facts(
    opening_t: u32,
    opening_word_bits: u32,
    tau_arity: u32,
) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: MULTISWAP_DEFECT_LOG2_BOUND,
        lift_arity_log2: opening_t,
        opening_t,
        opening_word_bits,
        direct_opening: true,
        tau_arity,
        piop_degree: 3,
        // tau_arity = log2(gate capacity) + 2; each gate commits 2^12 bits.
        // Thus log2(d) = tau_arity + 10, and the conservative bound is 257+log2(d).
        step50_magnitude_log2: 267 + tau_arity,
    }
}

/// Public interval and grinding parameters of the MultiSwap profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiswapPrimeProfile {
    fingerprint_min: u128,
    fingerprint_max: u128,
    reduction_min: u128,
    reduction_max: u128,
    reduction_grinding: usize,
}

impl Default for MultiswapPrimeProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiswapPrimeProfile {
    /// The fixed `[2^127, 2^128)` fingerprint and `[2^112, 2^113)`
    /// reduction intervals with the 10-bit reduction grind — the values
    /// [`crate::piop::spartan::profile::Limber114`] derives at the wired
    /// MultiSwap shape (pinned by a test below).
    pub const fn new() -> Self {
        Self {
            fingerprint_min: 1u128 << 127,
            fingerprint_max: u128::MAX,
            reduction_min: 1u128 << 112,
            reduction_max: (1u128 << 113) - 1,
            reduction_grinding: 10,
        }
    }

    /// Adopts an instantiated security profile (Strategy 2 required).
    pub fn from_security(params: &IopSecurityParams) -> Result<Self, MultiswapPrimeError> {
        if !params.projection_full_width {
            return Err(MultiswapPrimeError::ProfileStrategyMismatch);
        }
        let reduction = params
            .reduction
            .ok_or(MultiswapPrimeError::ProfileStrategyMismatch)?;
        Ok(Self {
            fingerprint_min: params.projection_min,
            fingerprint_max: params.projection_max,
            reduction_min: reduction.min,
            reduction_max: reduction.max,
            reduction_grinding: reduction.grinding_bits as usize,
        })
    }

    /// Inclusive endpoints of the fingerprint-prime interval.
    pub const fn fingerprint_interval(self) -> (u128, u128) {
        (self.fingerprint_min, self.fingerprint_max)
    }

    /// Inclusive endpoints of the Step 5.0 reduction-prime interval.
    pub const fn reduction_interval(self) -> (u128, u128) {
        (self.reduction_min, self.reduction_max)
    }

    /// Proof-of-work bits immediately before the reduction-prime draw.
    pub const fn reduction_grinding_bits(self) -> usize {
        self.reduction_grinding
    }

    /// Interval and parity admissibility of one fingerprint value.
    pub const fn accepts_fingerprint_prime(self, q: u128) -> bool {
        q >= self.fingerprint_min && q & 1 == 1
    }

    /// Interval and parity admissibility of one reduction value.
    pub const fn accepts_reduction_prime(self, q: u128) -> bool {
        q >= self.reduction_min && q <= self.reduction_max && q & 1 == 1
    }
}

/// Runtime arithmetic context of the sampled fingerprint prime `Q`.
pub struct MultiswapFingerprintContext {
    profile: MultiswapPrimeProfile,
    q: u128,
    field_config: <SpartanF2zField as PrimeField>::Config,
}

impl core::fmt::Debug for MultiswapFingerprintContext {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MultiswapFingerprintContext")
            .field("profile", &self.profile)
            .field("q", &self.q)
            .finish_non_exhaustive()
    }
}

impl MultiswapFingerprintContext {
    fn new(profile: MultiswapPrimeProfile, q: u128) -> Result<Self, MultiswapPrimeError> {
        if !profile.accepts_fingerprint_prime(q) {
            return Err(MultiswapPrimeError::PrimeOutsideProfile { q });
        }
        let field_config = F128::make_cfg(&Uint::from(q))
            .map_err(|_| MultiswapPrimeError::InvalidFieldConfiguration)?;
        F128::validate_config(&field_config)
            .map_err(|_| MultiswapPrimeError::InvalidFieldConfiguration)?;
        Ok(Self {
            profile,
            q,
            field_config,
        })
    }

    /// Public profile used to sample this modulus.
    pub const fn profile(&self) -> MultiswapPrimeProfile {
        self.profile
    }

    /// Sampled fingerprint prime in canonical integer form.
    pub const fn q(&self) -> u128 {
        self.q
    }

    /// Bit length of `Q` (always 128 in this profile).
    pub const fn q_bits(&self) -> usize {
        128
    }

    /// Runtime Montgomery configuration of the 128-bit-backed prime field.
    pub const fn field_config(&self) -> &<SpartanF2zField as PrimeField>::Config {
        &self.field_config
    }
}

/// Samples the fingerprint prime `Q` from `[2^127, 2^128)` and binds its
/// canonical encoding before any field-valued challenge is drawn.
///
/// The interval exceeds the general sampler's `2^126` operating cap, so
/// this dedicated sampler draws exactly uniform odd candidates (the
/// `2^126` odd residues divide the `u128` draw space evenly, so masking a
/// uniform draw is exact) and tests them with the same deterministic
/// BPSW-style `crypto_primes` check that guards every runtime Spartan
/// field configuration.  Rejected draws advance the transcript for prover
/// and verifier identically.
pub fn sample_multiswap_fingerprint_context(
    transcript: &mut impl Transcript,
    profile: MultiswapPrimeProfile,
) -> Result<MultiswapFingerprintContext, MultiswapPrimeError> {
    absorb_spartan_message(transcript, b"prime-domain", FINGERPRINT_SAMPLING_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"prime-min",
        &profile.fingerprint_min.to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"prime-max",
        &profile.fingerprint_max.to_le_bytes(),
    );

    let mut q = None;
    for _ in 0..FINGERPRINT_SAMPLING_ATTEMPTS {
        let draw = transcript_u128(transcript) & ((1u128 << 126) - 1);
        let candidate = (1u128 << 127) | (draw << 1) | 1;
        if is_prime(Flavor::Any, &crypto_bigint::U128::from(candidate)) {
            q = Some(candidate);
            break;
        }
    }
    let q = q.ok_or(MultiswapPrimeError::FingerprintSearchExhausted {
        attempts: FINGERPRINT_SAMPLING_ATTEMPTS,
    })?;
    absorb_spartan_message(transcript, b"prime-q", &q.to_le_bytes());
    MultiswapFingerprintContext::new(profile, q)
}

/// Samples the Step 5.0 reduction prime `q'` from `[2^112, 2^113)` and
/// binds its canonical encoding.
///
/// Returns the prime and its exact bit length (always 113 here), the pair
/// the runtime mod-`q'` opening requires.
pub fn sample_multiswap_reduction_prime(
    transcript: &mut impl Transcript,
    profile: MultiswapPrimeProfile,
) -> Result<(u128, usize), MultiswapPrimeError> {
    absorb_spartan_message(transcript, b"prime-domain", REDUCTION_SAMPLING_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"prime-min",
        &profile.reduction_min.to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"prime-max",
        &profile.reduction_max.to_le_bytes(),
    );
    let q_prime =
        sample_prime_in_interval(transcript, profile.reduction_min, profile.reduction_max)?;
    absorb_spartan_message(transcript, b"prime-q", &q_prime.to_le_bytes());
    if !profile.accepts_reduction_prime(q_prime) {
        return Err(MultiswapPrimeError::PrimeOutsideProfile { q: q_prime });
    }
    let q_bits = (u128::BITS - q_prime.leading_zeros()) as usize;
    debug_assert_eq!(q_bits, 113);
    Ok((q_prime, q_bits))
}

fn transcript_u128(transcript: &mut impl Transcript) -> u128 {
    let draw: BinaryFieldGF128 = transcript.get_field_challenge(&());
    let words = draw.words();
    u128::from(words[0]) | (u128::from(words[1]) << 64)
}

/// Errors in deriving the MultiSwap runtime-prime contexts.
#[derive(Debug, Error)]
pub enum MultiswapPrimeError {
    /// A caller attempted to construct a context with an out-of-profile value.
    #[error("prime {q} is outside the MultiSwap runtime-prime profile")]
    PrimeOutsideProfile { q: u128 },
    /// The security profile is not a two-prime Strategy-2 configuration.
    #[error("the MultiSwap path requires a two-prime Strategy-2 security profile")]
    ProfileStrategyMismatch,
    /// The runtime field backend rejected the sampled modulus.
    #[error("failed to construct the runtime 128-bit prime field")]
    InvalidFieldConfiguration,
    /// The fingerprint sampler exhausted its transcript-draw budget.
    #[error("no fingerprint prime found after {attempts} transcript candidates")]
    FingerprintSearchExhausted { attempts: usize },
    /// Reduction-prime sampling failed.
    #[error(transparent)]
    Sampling(#[from] PrimeSamplingError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::spartan::profile::{IopSecurityProfile, Limber114};
    use crate::transcript::Blake3Transcript;

    /// The byte-identity keystone: the `Limber114` profile derives exactly
    /// the constants `MultiswapPrimeProfile::new()` has always carried, so
    /// the profile-wired path absorbs identical transcript bytes.
    #[test]
    fn limber114_derives_the_legacy_profile_verbatim() {
        let params = Limber114::instantiate(&multiswap_instance_facts(13, 1, 15)).unwrap();
        assert_eq!(
            MultiswapPrimeProfile::from_security(&params).unwrap(),
            MultiswapPrimeProfile::new()
        );
    }

    #[test]
    fn fingerprint_sampling_is_deterministic_and_full_width() {
        let profile = MultiswapPrimeProfile::new();
        let mut first = Blake3Transcript::new();
        let mut second = Blake3Transcript::new();
        let first = sample_multiswap_fingerprint_context(&mut first, profile).unwrap();
        let second = sample_multiswap_fingerprint_context(&mut second, profile).unwrap();
        assert_eq!(first.q(), second.q());
        assert!(first.q() >= 1u128 << 127);
        assert_eq!(first.q_bits(), 128);
        assert!(profile.accepts_fingerprint_prime(first.q()));
        assert!(is_prime(Flavor::Any, &crypto_bigint::U128::from(first.q())));
    }

    #[test]
    fn reduction_sampling_is_deterministic_and_always_113_bits() {
        let profile = MultiswapPrimeProfile::new();
        let mut first = Blake3Transcript::new();
        let mut second = Blake3Transcript::new();
        let (first_q, first_bits) = sample_multiswap_reduction_prime(&mut first, profile).unwrap();
        let (second_q, _) = sample_multiswap_reduction_prime(&mut second, profile).unwrap();
        assert_eq!(first_q, second_q);
        assert_eq!(first_bits, 113);
        assert!(profile.accepts_reduction_prime(first_q));
    }

    #[test]
    fn the_two_sampling_domains_yield_distinct_primes() {
        let profile = MultiswapPrimeProfile::new();
        let mut transcript = Blake3Transcript::new();
        let fingerprint = sample_multiswap_fingerprint_context(&mut transcript, profile).unwrap();
        let (reduction, _) = sample_multiswap_reduction_prime(&mut transcript, profile).unwrap();
        assert!(fingerprint.q() > reduction);
    }
}
