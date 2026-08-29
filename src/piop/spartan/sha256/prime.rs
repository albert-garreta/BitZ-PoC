//! Transcript-derived prime-field parameters for the paper SHA-256 profile.
//!
//! The commitment remains over `GF(2^128)`.  Only after that commitment is
//! bound into the Fiat--Shamir transcript do prover and verifier derive the
//! prime `q` used by Spartan and by the integer-to-field projection.

use crypto_primitives::{crypto_bigint_monty::F128, crypto_bigint_uint::Uint, PrimeField};
use thiserror::Error;

use crate::{
    ext_proj::{sample_prime_in_interval, PrimeSamplingError, ProjArith},
    piop::spartan::{absorb_spartan_message, SpartanField},
    transcript::traits::Transcript,
};

use super::super::SpartanF2zField;

const PRIME_SAMPLING_DOMAIN: &[u8] = b"f2z/spartan-sha256/runtime-prime/v1";

/// Smallest paper-supported batch: `2^7` independent compressions.
pub const SHA256_MIN_LOG_COMPRESSIONS: usize = 7;
/// Largest paper-supported batch: `2^16` independent compressions.
pub const SHA256_MAX_LOG_COMPRESSIONS: usize = 16;
/// Width of the fixed commitment/exponent field.
pub const SHA256_COMMITMENT_FIELD_BITS: usize = 128;

/// Bit length of the paper's prime interval for one batch size: primes are
/// sampled from `[2^{b-1}, 2^b)` with `b = min(113, 128 - log_compressions)`
/// (113 bits up to `2^15` compressions, 112 at `2^16` where the injective
/// no-wrap lift `(2^t + 1)(q - 1) <= 2^128 - 1` bites). Total for every
/// exponent so shape helpers can consult it outside the supported window.
pub const fn sha256_prime_interval_bits(log_compressions: usize) -> usize {
    let headroom = SHA256_COMMITMENT_FIELD_BITS.saturating_sub(log_compressions);
    if headroom < 113 {
        headroom
    } else {
        113
    }
}

/// Public interval and grinding parameters determined by the batch size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256PrimeProfile {
    log_compressions: usize,
    min_prime: u128,
    max_prime: u128,
}

impl Sha256PrimeProfile {
    /// Derives the paper's 112/113-bit prime interval for one batch size.
    pub fn new(log_compressions: usize) -> Result<Self, Sha256PrimeError> {
        if !(SHA256_MIN_LOG_COMPRESSIONS..=SHA256_MAX_LOG_COMPRESSIONS).contains(&log_compressions)
        {
            return Err(Sha256PrimeError::UnsupportedBatchExponent {
                actual: log_compressions,
            });
        }

        let instances = 1_u128 << log_compressions;
        let interval_bits = sha256_prime_interval_bits(log_compressions);
        let min_prime = 1_u128 << (interval_bits - 1);
        // The integer lift used by the paper must fit injectively in the
        // 128-bit commitment/exponent field:
        //
        //     (2^t + 1) (q - 1) <= 2^128 - 1.
        let no_wrap_max = 1 + u128::MAX / (instances + 1);
        let bit_max = (1_u128 << interval_bits) - 1;
        let max_prime = bit_max.min(no_wrap_max);
        if min_prime > max_prime {
            return Err(Sha256PrimeError::EmptyPrimeInterval);
        }

        Ok(Self {
            log_compressions,
            min_prime,
            max_prime,
        })
    }

    /// `t` where the batch contains `2^t` compressions.
    pub const fn log_compressions(self) -> usize {
        self.log_compressions
    }

    /// Inclusive lower endpoint of the sampled-prime interval.
    pub const fn min_prime(self) -> u128 {
        self.min_prime
    }

    /// Inclusive upper endpoint of the sampled-prime interval.
    pub const fn max_prime(self) -> u128 {
        self.max_prime
    }

    /// Initial proof-of-work bits before sampling `q` and the Spartan point.
    pub const fn initial_grinding_bits(self) -> usize {
        match self.log_compressions {
            7 | 8 => 20,
            9..=15 => 21,
            16 => 22,
            _ => unreachable!(),
        }
    }

    /// Proof-of-work bits before each cubic outer-sumcheck challenge.
    pub const fn outer_round_grinding_bits(self) -> usize {
        if self.log_compressions == 16 {
            19
        } else {
            18
        }
    }

    /// Proof-of-work bits before the terminal opening challenges.
    pub const fn terminal_grinding_bits(self) -> usize {
        self.outer_round_grinding_bits()
    }

    /// Checks the exact 128-bit no-wrap inequality without overflowing.
    pub const fn accepts_prime(self, q: u128) -> bool {
        q >= self.min_prime
            && q <= self.max_prime
            && q > 2
            && q & 1 == 1
            && q - 1 <= u128::MAX / ((1_u128 << self.log_compressions) + 1)
    }
}

/// Runtime arithmetic context shared by relation projection and Spartan.
pub struct Sha256ModQContext {
    profile: Sha256PrimeProfile,
    q: u128,
    q_bits: usize,
    field_config: <SpartanF2zField as PrimeField>::Config,
    projection: ProjArith,
}

impl core::fmt::Debug for Sha256ModQContext {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Sha256ModQContext")
            .field("profile", &self.profile)
            .field("q", &self.q)
            .field("q_bits", &self.q_bits)
            .finish_non_exhaustive()
    }
}

impl Sha256ModQContext {
    fn new(profile: Sha256PrimeProfile, q: u128) -> Result<Self, Sha256PrimeError> {
        if !profile.accepts_prime(q) {
            return Err(Sha256PrimeError::PrimeOutsideProfile { q });
        }
        let field_config = F128::make_cfg(&Uint::from(q))
            .map_err(|_| Sha256PrimeError::InvalidFieldConfiguration)?;
        F128::validate_config(&field_config)
            .map_err(|_| Sha256PrimeError::InvalidFieldConfiguration)?;
        let q_bits = (u128::BITS - q.leading_zeros()) as usize;
        debug_assert!(q_bits <= 113);
        Ok(Self {
            profile,
            q,
            q_bits,
            field_config,
            projection: ProjArith::new(q),
        })
    }

    /// Public batch-dependent profile used to sample this modulus.
    pub const fn profile(&self) -> Sha256PrimeProfile {
        self.profile
    }

    /// Sampled prime modulus in canonical integer form.
    pub const fn q(&self) -> u128 {
        self.q
    }

    /// Actual bit length of `q` (112 or 113 in the supported profile).
    pub const fn q_bits(&self) -> usize {
        self.q_bits
    }

    /// Runtime Montgomery configuration for the 128-bit-backed prime field.
    pub const fn field_config(&self) -> &<SpartanF2zField as PrimeField>::Config {
        &self.field_config
    }

    /// Canonical scalar arithmetic modulo the same `q`.
    pub const fn projection(&self) -> &ProjArith {
        &self.projection
    }
}

/// Samples `q` from the transcript and immediately binds its canonical
/// 16-byte encoding before any field-valued challenge is drawn.
pub fn sample_sha256_mod_q_context(
    transcript: &mut impl Transcript,
    profile: Sha256PrimeProfile,
) -> Result<Sha256ModQContext, Sha256PrimeError> {
    absorb_spartan_message(transcript, b"prime-domain", PRIME_SAMPLING_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"log-compressions",
        &(profile.log_compressions as u64).to_le_bytes(),
    );
    absorb_spartan_message(transcript, b"prime-min", &profile.min_prime.to_le_bytes());
    absorb_spartan_message(transcript, b"prime-max", &profile.max_prime.to_le_bytes());
    let q = sample_prime_in_interval(transcript, profile.min_prime, profile.max_prime)?;
    absorb_spartan_message(transcript, b"prime-q", &q.to_le_bytes());
    Sha256ModQContext::new(profile, q)
}

/// Errors in deriving the paper runtime-prime profile.
#[derive(Debug, Error)]
pub enum Sha256PrimeError {
    /// Production supports exactly the paper batch window.
    #[error("SHA-256 runtime-prime profile requires log-compressions in [7, 16], got {actual}")]
    UnsupportedBatchExponent { actual: usize },
    /// The derived interval unexpectedly contains no candidate.
    #[error("the SHA-256 runtime-prime interval is empty")]
    EmptyPrimeInterval,
    /// A caller attempted to construct a context with an out-of-profile value.
    #[error("prime {q} is outside the SHA-256 runtime-prime profile")]
    PrimeOutsideProfile { q: u128 },
    /// The runtime field backend rejected the sampled modulus.
    #[error("failed to construct the runtime 128-bit prime field")]
    InvalidFieldConfiguration,
    /// Transcript prime sampling failed.
    #[error(transparent)]
    Sampling(#[from] PrimeSamplingError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    #[test]
    fn paper_intervals_have_the_expected_width_and_no_wrap_bound() {
        for t in SHA256_MIN_LOG_COMPRESSIONS..=SHA256_MAX_LOG_COMPRESSIONS {
            let profile = Sha256PrimeProfile::new(t).unwrap();
            let min_bits = (u128::BITS - profile.min_prime().leading_zeros()) as usize;
            assert_eq!(min_bits, if t == 16 { 112 } else { 113 });
            assert!(profile.max_prime() < (1_u128 << 113));
            assert!(profile.min_prime() <= profile.max_prime());
            assert!(profile.max_prime() - 1 <= u128::MAX / ((1_u128 << t) + 1));
        }

        let t14 = Sha256PrimeProfile::new(14).unwrap();
        assert_eq!(t14.max_prime(), (1_u128 << 113) - 1);
        let t15 = Sha256PrimeProfile::new(15).unwrap();
        assert!(t15.max_prime() < (1_u128 << 113) - 1);
        let t16 = Sha256PrimeProfile::new(16).unwrap();
        assert!(t16.max_prime() < (1_u128 << 112) - 1);
    }

    #[test]
    fn transcript_sampling_is_deterministic_and_builds_one_field_context() {
        let profile = Sha256PrimeProfile::new(7).unwrap();
        let mut first = Blake3Transcript::new();
        let mut second = Blake3Transcript::new();
        let first = sample_sha256_mod_q_context(&mut first, profile).unwrap();
        let second = sample_sha256_mod_q_context(&mut second, profile).unwrap();
        assert_eq!(first.q(), second.q());
        assert_eq!(first.q_bits(), 113);
        assert!(profile.accepts_prime(first.q()));
        assert_eq!(first.projection().q(), first.q());
        assert_eq!(
            F128::canonical_modulus_encoding(first.field_config()),
            first.q().to_le_bytes()
        );
    }

    #[test]
    fn rejects_batch_sizes_outside_the_paper_profile() {
        assert!(matches!(
            Sha256PrimeProfile::new(6),
            Err(Sha256PrimeError::UnsupportedBatchExponent { actual: 6 })
        ));
        assert!(matches!(
            Sha256PrimeProfile::new(17),
            Err(Sha256PrimeError::UnsupportedBatchExponent { actual: 17 })
        ));
    }
}
