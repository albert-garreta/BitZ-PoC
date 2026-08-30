//! Transcript-derived prime-field parameters for the paper SHA-256 profile.
//!
//! The commitment remains over `GF(2^128)`.  Only after that commitment is
//! bound into the Fiat--Shamir transcript do prover and verifier derive the
//! prime `q` used by Spartan and by the integer-to-field projection.

use crypto_primitives::{crypto_bigint_monty::F128, crypto_bigint_uint::Uint, PrimeField};
use thiserror::Error;

use crate::{
    ext_proj::{sample_prime_in_interval, PrimeSamplingError, ProjArith},
    piop::spartan::{
        absorb_spartan_message,
        profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, LegacySha128Design},
        SpartanField,
    },
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

/// τ point arity of the repeated outer zerocheck: eight local constraint
/// variables plus one per batch doubling.
pub const SHA256_TAU_LOCAL_VARS: u32 = 8;

/// The public statement facts the security-profile derivation consumes for
/// a `2^log_compressions` SHA-256 batch: per-row integer defects are far
/// below any sampled prime (Boolean assignment, coefficients `< 2^33`, a
/// few hundred entries per row — `< 2^96` conservatively), the Step-5.1
/// lift sums `2^t` terms, and the opening is the VIRTUAL path (fold width
/// capped from `q_bits`, so the one-chunk fold bound does not gate q).
pub const fn sha256_instance_facts(log_compressions: u32) -> IopInstanceFacts {
    IopInstanceFacts {
        defect_log2_bound: 96,
        lift_arity_log2: log_compressions,
        opening_t: log_compressions,
        opening_word_bits: 1,
        direct_opening: false,
        tau_arity: SHA256_TAU_LOCAL_VARS + log_compressions,
        piop_degree: 3,
        step50_magnitude_log2: 0,
    }
}

/// Public interval and grinding parameters determined by the batch size and
/// the selected security profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256PrimeProfile {
    log_compressions: usize,
    min_prime: u128,
    max_prime: u128,
    initial_grinding: usize,
    outer_grinding: usize,
    terminal_grinding: usize,
}

impl Sha256PrimeProfile {
    /// The paper's 112/113-bit interval with the 128-design grinding
    /// schedule (`initial 20|21|22`, `outer 18|19`) — exactly
    /// [`LegacySha128Design`] instantiated at this batch size, which is
    /// how those historical tables are derived today.
    pub fn new(log_compressions: usize) -> Result<Self, Sha256PrimeError> {
        let exponent = validate_batch_exponent(log_compressions)?;
        let params = LegacySha128Design::instantiate(&sha256_instance_facts(exponent))
            .map_err(|_| Sha256PrimeError::EmptyPrimeInterval)?;
        Ok(Self::adopt(&params, log_compressions))
    }

    /// Adopts an instantiated security profile (single-prime policy).
    pub fn from_security(
        params: &IopSecurityParams,
        log_compressions: usize,
    ) -> Result<Self, Sha256PrimeError> {
        validate_batch_exponent(log_compressions)?;
        if params.projection_full_width || params.reduction.is_some() {
            return Err(Sha256PrimeError::ProfileStrategyMismatch);
        }
        Ok(Self::adopt(params, log_compressions))
    }

    fn adopt(params: &IopSecurityParams, log_compressions: usize) -> Self {
        Self {
            log_compressions,
            min_prime: params.projection_min,
            max_prime: params.projection_max,
            initial_grinding: params.initial_grinding_bits as usize,
            outer_grinding: params.piop_round_grinding_bits as usize,
            terminal_grinding: params.terminal_grinding_bits as usize,
        }
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
        self.initial_grinding
    }

    /// Proof-of-work bits before each cubic outer-sumcheck challenge.
    pub const fn outer_round_grinding_bits(self) -> usize {
        self.outer_grinding
    }

    /// Proof-of-work bits before the terminal opening challenges.
    pub const fn terminal_grinding_bits(self) -> usize {
        self.terminal_grinding
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

fn validate_batch_exponent(log_compressions: usize) -> Result<u32, Sha256PrimeError> {
    if !(SHA256_MIN_LOG_COMPRESSIONS..=SHA256_MAX_LOG_COMPRESSIONS).contains(&log_compressions) {
        return Err(Sha256PrimeError::UnsupportedBatchExponent {
            actual: log_compressions,
        });
    }
    u32::try_from(log_compressions)
        .map_err(|_| Sha256PrimeError::UnsupportedBatchExponent {
            actual: log_compressions,
        })
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
    /// The security profile is not a single-prime configuration.
    #[error("the SHA-256 path requires a single-prime security profile")]
    ProfileStrategyMismatch,
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
