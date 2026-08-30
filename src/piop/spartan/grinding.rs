//! Typed Fiat--Shamir proof-of-work grinding for Spartan prover messages.
//!
//! A grinding boundary has a type-level protocol domain and a canonical
//! `u64` round index.  Prover and verifier first bind that boundary and the
//! configured difficulty into the current transcript, squeeze a 256-bit seed,
//! and then search/check
//!
//! ```text
//! BLAKE3(seed || nonce.to_le_bytes()).leading_zeros() >= difficulty.
//! ```
//!
//! The nonce is absorbed into the transcript as canonical little-endian bytes
//! before the next Fiat--Shamir challenge is drawn.  The parallel prover scans
//! fixed, ordered waves and returns the minimum hit in the first successful
//! wave, so enabling `parallel` does not change the proof or transcript.

use core::marker::PhantomData;

use thiserror::Error;

use crate::transcript::traits::{ConstTranscribable, GenTranscribable, Transcript};

/// Transcript frame for every Spartan grinding boundary.
const GRINDING_TRANSCRIPT_DOMAIN: &[u8] = b"f2z/spartan/fiat-shamir-grinding/v1";
/// Frame separating the canonical nonce from the seed-derivation inputs.
const GRINDING_NONCE_DOMAIN: &[u8] = b"f2z/spartan/fiat-shamir-grinding/nonce/v1";

/// BLAKE3 outputs 256 bits, so no larger difficulty can be satisfied.
pub const MAX_GRINDING_BITS: u32 = 256;

/// The type-level domain of one class of prover-message grinding boundaries.
///
/// Use a distinct zero-sized marker type for protocol stages whose nonces are
/// not interchangeable.  For example, SHA commitment, Spartan outer-round,
/// and Spartan terminal messages should each implement this trait with a
/// different `DOMAIN` value.
pub trait GrindingDomain {
    /// A non-empty, versioned protocol-stage domain separator.
    const DOMAIN: &'static [u8];
}

/// One indexed grinding boundary in a [`GrindingDomain`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrindingRound<D> {
    index: u64,
    _domain: PhantomData<fn() -> D>,
}

impl<D> GrindingRound<D> {
    /// Constructs the boundary for `index`.
    pub const fn new(index: u64) -> Self {
        Self {
            index,
            _domain: PhantomData,
        }
    }

    /// Returns the canonical round index bound into the transcript.
    pub const fn index(&self) -> u64 {
        self.index
    }
}

/// A 256-bit proof-of-work seed derived from the current transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GrindingSeed([u8; 32]);

impl GrindingSeed {
    /// Constructs a seed from its canonical bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the canonical seed bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl GenTranscribable for GrindingSeed {
    fn read_transcription_bytes_exact(bytes: &[u8]) -> Self {
        Self(
            bytes
                .try_into()
                .expect("a grinding seed transcript challenge is exactly 32 bytes"),
        )
    }

    fn write_transcription_bytes_exact(&self, buf: &mut [u8]) {
        assert_eq!(buf.len(), Self::NUM_BYTES);
        buf.copy_from_slice(&self.0);
    }
}

impl ConstTranscribable for GrindingSeed {
    const NUM_BYTES: usize = 32;
}

/// Failures while deriving, finding, or checking a grinding nonce.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GrindingError {
    /// Zero silently disables grinding, while values above 256 cannot be
    /// satisfied by a BLAKE3 digest.
    #[error("grinding difficulty must be in 1..={MAX_GRINDING_BITS}, got {bits}")]
    InvalidDifficulty { bits: u32 },

    /// An empty protocol-stage domain defeats type-level domain separation.
    #[error("grinding domains must be non-empty")]
    EmptyDomain,

    /// No `u64` nonce satisfies the requested seed and difficulty.
    #[error("the u64 grinding nonce space was exhausted")]
    NonceSpaceExhausted,

    /// The supplied nonce does not meet the configured difficulty.
    #[error("grinding nonce {nonce} does not satisfy {bits} leading zero bits")]
    InvalidNonce { nonce: u64, bits: u32 },
}

/// Binds a typed boundary to `transcript` and squeezes its 256-bit PoW seed.
///
/// Validation happens before the transcript is mutated.  The exact absorbed
/// order is the global frame, type-level domain, round index as `u64` LE, and
/// difficulty as `u32` LE.
pub fn derive_grinding_seed<D, T>(
    transcript: &mut T,
    round: GrindingRound<D>,
    bits: u32,
) -> Result<GrindingSeed, GrindingError>
where
    D: GrindingDomain,
    T: Transcript,
{
    validate_boundary::<D>(bits)?;
    transcript.absorb_slice(GRINDING_TRANSCRIPT_DOMAIN);
    transcript.absorb_slice(D::DOMAIN);
    transcript.absorb_slice(&round.index.to_le_bytes());
    transcript.absorb_slice(&bits.to_le_bytes());
    Ok(transcript.get_challenge())
}

/// Finds the smallest valid nonce and absorbs its canonical encoding.
///
/// With the `parallel` feature, sufficiently expensive searches use ordered
/// parallel waves.  The result remains byte-for-byte identical to a serial
/// scan from nonce zero.
pub fn grind_and_absorb<D, T>(
    transcript: &mut T,
    round: GrindingRound<D>,
    bits: u32,
) -> Result<u64, GrindingError>
where
    D: GrindingDomain,
    T: Transcript,
{
    let seed = derive_grinding_seed(transcript, round, bits)?;
    let nonce = find_grinding_nonce(&seed, bits)?;
    absorb_grinding_nonce(transcript, nonce);
    Ok(nonce)
}

/// Checks a proof nonce and absorbs its canonical encoding.
///
/// As in the existing Flock challenger bridge, the nonce is absorbed even
/// when it is invalid.  This gives prover and verifier one unambiguous proof
/// item boundary; callers must reject the returned error and must not derive
/// further protocol challenges from a failed verification.
pub fn verify_and_absorb<D, T>(
    transcript: &mut T,
    round: GrindingRound<D>,
    bits: u32,
    nonce: u64,
) -> Result<(), GrindingError>
where
    D: GrindingDomain,
    T: Transcript,
{
    let seed = derive_grinding_seed(transcript, round, bits)?;
    let valid = grinding_nonce_is_valid_unchecked(&seed, nonce, bits);
    absorb_grinding_nonce(transcript, nonce);
    if valid {
        Ok(())
    } else {
        Err(GrindingError::InvalidNonce { nonce, bits })
    }
}

/// Finds the smallest `u64` nonce satisfying `bits` for an explicit seed.
pub fn find_grinding_nonce(seed: &GrindingSeed, bits: u32) -> Result<u64, GrindingError> {
    validate_difficulty(bits)?;

    #[cfg(feature = "parallel")]
    {
        // Below this point rayon's fork/join cost exceeds the expected search.
        const PARALLEL_SEARCH_MIN_BITS: u32 = 11;
        if bits >= PARALLEL_SEARCH_MIN_BITS {
            return find_grinding_nonce_parallel(seed, bits);
        }
    }

    find_grinding_nonce_sequential(seed, bits)
}

/// Checks `nonce` against an explicit seed after validating the difficulty.
pub fn grinding_nonce_is_valid(
    seed: &GrindingSeed,
    nonce: u64,
    bits: u32,
) -> Result<bool, GrindingError> {
    validate_difficulty(bits)?;
    Ok(grinding_nonce_is_valid_unchecked(seed, nonce, bits))
}

fn validate_boundary<D: GrindingDomain>(bits: u32) -> Result<(), GrindingError> {
    validate_difficulty(bits)?;
    if D::DOMAIN.is_empty() {
        return Err(GrindingError::EmptyDomain);
    }
    Ok(())
}

fn validate_difficulty(bits: u32) -> Result<(), GrindingError> {
    if !(1..=MAX_GRINDING_BITS).contains(&bits) {
        return Err(GrindingError::InvalidDifficulty { bits });
    }
    Ok(())
}

fn absorb_grinding_nonce(transcript: &mut impl Transcript, nonce: u64) {
    transcript.absorb_slice(GRINDING_NONCE_DOMAIN);
    transcript.absorb_slice(&nonce.to_le_bytes());
}

#[allow(clippy::arithmetic_side_effects)]
fn find_grinding_nonce_sequential(seed: &GrindingSeed, bits: u32) -> Result<u64, GrindingError> {
    find_first_valid_nonce(seed, bits, 0, u64::MAX).ok_or(GrindingError::NonceSpaceExhausted)
}

#[cfg(feature = "parallel")]
#[allow(clippy::arithmetic_side_effects)]
fn find_grinding_nonce_parallel(seed: &GrindingSeed, bits: u32) -> Result<u64, GrindingError> {
    use rayon::prelude::*;

    // Each wave has 32 independently scanned 2^12-nonce chunks.  Taking the
    // minimum hit from the first successful wave is exactly a serial scan.
    const CHUNK_SIZE: u64 = 1 << 12;
    const CHUNKS_PER_WAVE: usize = 32;
    const WAVE_SIZE: u64 = CHUNK_SIZE * CHUNKS_PER_WAVE as u64;

    let mut wave_start = 0_u64;
    loop {
        let wave_end = wave_start.saturating_add(WAVE_SIZE - 1);
        let wave_len = wave_end - wave_start + 1;
        let chunk_count = usize::try_from(wave_len.div_ceil(CHUNK_SIZE))
            .expect("a fixed grinding wave has at most 32 chunks");
        let hit = (0..chunk_count)
            .into_par_iter()
            .filter_map(|chunk| {
                let chunk_start = wave_start + chunk as u64 * CHUNK_SIZE;
                let chunk_end = chunk_start.saturating_add(CHUNK_SIZE - 1).min(wave_end);
                find_first_valid_nonce(seed, bits, chunk_start, chunk_end)
            })
            .min();
        if let Some(nonce) = hit {
            return Ok(nonce);
        }
        if wave_end == u64::MAX {
            return Err(GrindingError::NonceSpaceExhausted);
        }
        wave_start = wave_end + 1;
    }
}

#[allow(clippy::arithmetic_side_effects)]
fn find_first_valid_nonce(seed: &GrindingSeed, bits: u32, start: u64, end: u64) -> Option<u64> {
    let mut nonce = start;
    loop {
        if grinding_nonce_is_valid_unchecked(seed, nonce, bits) {
            return Some(nonce);
        }
        if nonce == end {
            return None;
        }
        nonce += 1;
    }
}

fn grinding_nonce_is_valid_unchecked(seed: &GrindingSeed, nonce: u64, bits: u32) -> bool {
    let mut preimage = [0_u8; 40];
    preimage[..32].copy_from_slice(seed.as_bytes());
    preimage[32..].copy_from_slice(&nonce.to_le_bytes());
    let digest = blake3::hash(&preimage);
    leading_zero_bits(digest.as_bytes()) >= bits
}

#[allow(clippy::arithmetic_side_effects)]
fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut count = 0_u32;
    for &byte in bytes {
        if byte == 0 {
            count += u8::BITS;
        } else {
            return count + byte.leading_zeros();
        }
    }
    count
}

// ---------------------------------------------------------------------
// Per-challenge grinding transcripts (the forest/GKR + ring-switch hooks)
// ---------------------------------------------------------------------

/// The forest/opening grinding domain: every challenge drawn inside the
/// exponent-fold opening region (forest sumcheck rounds, claim
/// unifications, the ring-switch/batching draws) is preceded by one typed
/// boundary in this domain when the security profile sets a nonzero
/// difficulty. Paper §Instantiation: each GKR round carries `3/|K|`
/// (~2^-126.4), so λ = 128 takes two bits per round; the ring-switch
/// round's `1/|K|` takes one — a uniform per-draw difficulty of
/// `max(2, 1)` covers both.
pub enum ForestRoundGrinding {}

impl GrindingDomain for ForestRoundGrinding {
    const DOMAIN: &'static [u8] = b"f2z/forest/round-grinding/v1";
}

/// Prover-side transcript adapter: before every challenge drawn through
/// it, grinds one boundary in the domain `D` (default
/// [`ForestRoundGrinding`]) at the configured difficulty and records the
/// nonce. At difficulty 0 it is a transparent pass-through — not one
/// transcript byte moves.
///
/// Wrap exactly the region whose rounds the difficulty covers and call
/// [`Self::finish`] to recover the nonces for the proof; leave any inner
/// Ligerito call OUTSIDE the wrapper (flock carries its own grinding
/// configuration).
pub struct ProverGrindingTranscript<'a, T, D = ForestRoundGrinding> {
    inner: &'a mut T,
    bits: u32,
    next_index: u64,
    nonces: Vec<u64>,
    _domain: PhantomData<fn() -> D>,
}

impl<'a, T: Transcript, D: GrindingDomain> ProverGrindingTranscript<'a, T, D> {
    /// Wraps `inner` at `bits` difficulty per drawn challenge.
    pub fn new(inner: &'a mut T, bits: u32) -> Self {
        Self {
            inner,
            bits,
            next_index: 0,
            nonces: Vec::new(),
            _domain: PhantomData,
        }
    }

    /// The nonces ground so far, in draw order (empty at difficulty 0).
    pub fn finish(self) -> Vec<u64> {
        self.nonces
    }
}

impl<T: Transcript, D: GrindingDomain> Transcript for ProverGrindingTranscript<'_, T, D> {
    fn get_challenge<C: ConstTranscribable>(&mut self) -> C {
        if self.bits > 0 {
            let round = GrindingRound::<D>::new(self.next_index);
            self.next_index = self.next_index.wrapping_add(1);
            let nonce = grind_and_absorb(self.inner, round, self.bits)
                .expect("per-round grinding difficulty is validated by the profile");
            self.nonces.push(nonce);
        }
        self.inner.get_challenge()
    }

    fn get_prime<R, P>(&mut self) -> R
    where
        R: crypto_primitives::ConstIntSemiring + ConstTranscribable,
        P: crate::utils::primality::PrimalityTest<R>,
    {
        self.inner.get_prime::<R, P>()
    }

    fn absorb_inner(&mut self, v: &[u8]) {
        self.inner.absorb_inner(v);
    }
}

/// Verifier-side twin of [`ProverGrindingTranscript`]: before every drawn
/// challenge it checks (and absorbs) the next proof nonce at the same
/// difficulty. Nonce failures and count mismatches are deferred to
/// [`Self::finish`] so the transcript stays deterministic — the caller
/// MUST propagate that result before accepting the proof.
pub struct VerifierGrindingTranscript<'a, 'n, T, D = ForestRoundGrinding> {
    inner: &'a mut T,
    bits: u32,
    next_index: u64,
    nonces: &'n [u64],
    consumed: usize,
    failure: Option<GrindingError>,
    _domain: PhantomData<fn() -> D>,
}

impl<'a, 'n, T: Transcript, D: GrindingDomain> VerifierGrindingTranscript<'a, 'n, T, D> {
    /// Wraps `inner`, checking `nonces` at `bits` difficulty per draw.
    pub fn new(inner: &'a mut T, bits: u32, nonces: &'n [u64]) -> Self {
        Self {
            inner,
            bits,
            next_index: 0,
            nonces,
            consumed: 0,
            failure: None,
            _domain: PhantomData,
        }
    }

    /// Succeeds iff every drawn challenge consumed one valid nonce and no
    /// nonce is left over.
    #[must_use = "an unverified grinding region proves nothing"]
    pub fn finish(self) -> Result<(), GrindingError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if self.consumed != self.nonces.len() {
            return Err(GrindingError::InvalidNonce {
                nonce: self.nonces.get(self.consumed).copied().unwrap_or(0),
                bits: self.bits,
            });
        }
        Ok(())
    }
}

impl<T: Transcript, D: GrindingDomain> Transcript for VerifierGrindingTranscript<'_, '_, T, D> {
    fn get_challenge<C: ConstTranscribable>(&mut self) -> C {
        if self.bits > 0 {
            let round = GrindingRound::<D>::new(self.next_index);
            self.next_index = self.next_index.wrapping_add(1);
            // A missing nonce absorbs a canonical zero so the transcript
            // stays deterministic; `finish` reports the failure.
            let nonce = self.nonces.get(self.consumed).copied().unwrap_or(0);
            self.consumed = self.consumed.saturating_add(1);
            if let Err(error) = verify_and_absorb(self.inner, round, self.bits, nonce) {
                self.failure.get_or_insert(error);
            }
        }
        self.inner.get_challenge()
    }

    fn get_prime<R, P>(&mut self) -> R
    where
        R: crypto_primitives::ConstIntSemiring + ConstTranscribable,
        P: crate::utils::primality::PrimalityTest<R>,
    {
        self.inner.get_prime::<R, P>()
    }

    fn absorb_inner(&mut self, v: &[u8]) {
        self.inner.absorb_inner(v);
    }
}

#[cfg(test)]
mod tests {
    use crate::transcript::{traits::Transcript, Blake3Transcript};

    use super::*;

    enum InitialMessage {}
    enum OuterRound {}
    enum Empty {}

    impl GrindingDomain for InitialMessage {
        const DOMAIN: &'static [u8] = b"test/spartan-grinding/initial/v1";
    }

    impl GrindingDomain for OuterRound {
        const DOMAIN: &'static [u8] = b"test/spartan-grinding/outer-round/v1";
    }

    impl GrindingDomain for Empty {
        const DOMAIN: &'static [u8] = b"";
    }

    fn transcript() -> Blake3Transcript {
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"fixed public statement");
        transcript.absorb_slice(b"fixed prover message");
        transcript
    }

    #[test]
    fn prover_and_verifier_are_deterministic_and_continue_in_lockstep() {
        let mut prover = transcript();
        let mut verifier = transcript();

        let nonce =
            grind_and_absorb(&mut prover, GrindingRound::<InitialMessage>::new(0), 9).unwrap();
        verify_and_absorb(
            &mut verifier,
            GrindingRound::<InitialMessage>::new(0),
            9,
            nonce,
        )
        .unwrap();

        assert_eq!(
            prover.get_challenge::<u128>(),
            verifier.get_challenge::<u128>()
        );

        let mut repeated = transcript();
        assert_eq!(
            grind_and_absorb(&mut repeated, GrindingRound::<InitialMessage>::new(0), 9,).unwrap(),
            nonce
        );
    }

    #[test]
    fn invalid_nonce_is_rejected_and_still_absorbed_canonically() {
        const BITS: u32 = 8;
        let mut prover = transcript();
        let valid =
            grind_and_absorb(&mut prover, GrindingRound::<InitialMessage>::new(0), BITS).unwrap();

        let mut seed_transcript = transcript();
        let seed = derive_grinding_seed(
            &mut seed_transcript,
            GrindingRound::<InitialMessage>::new(0),
            BITS,
        )
        .unwrap();
        let invalid = (0..=u64::MAX)
            .find(|&nonce| nonce != valid && !grinding_nonce_is_valid(&seed, nonce, BITS).unwrap())
            .unwrap();

        let mut verifier = transcript();
        assert_eq!(
            verify_and_absorb(
                &mut verifier,
                GrindingRound::<InitialMessage>::new(0),
                BITS,
                invalid,
            ),
            Err(GrindingError::InvalidNonce {
                nonce: invalid,
                bits: BITS,
            })
        );

        // Reproduce the documented framing explicitly. This pins both the
        // nonce's byte order and the fact that invalid nonces are absorbed.
        seed_transcript.absorb_slice(GRINDING_NONCE_DOMAIN);
        seed_transcript.absorb_slice(&invalid.to_le_bytes());
        assert_eq!(
            verifier.get_challenge::<u128>(),
            seed_transcript.get_challenge::<u128>()
        );
    }

    #[test]
    fn domain_round_and_difficulty_separate_seeds() {
        let initial = derive_grinding_seed(
            &mut transcript(),
            GrindingRound::<InitialMessage>::new(0),
            8,
        )
        .unwrap();
        let other_domain =
            derive_grinding_seed(&mut transcript(), GrindingRound::<OuterRound>::new(0), 8)
                .unwrap();
        let other_round = derive_grinding_seed(
            &mut transcript(),
            GrindingRound::<InitialMessage>::new(1),
            8,
        )
        .unwrap();
        let other_difficulty = derive_grinding_seed(
            &mut transcript(),
            GrindingRound::<InitialMessage>::new(0),
            9,
        )
        .unwrap();

        assert_ne!(initial, other_domain);
        assert_ne!(initial, other_round);
        assert_ne!(initial, other_difficulty);
    }

    #[test]
    fn invalid_boundary_configuration_does_not_mutate_transcript() {
        for bits in [0, MAX_GRINDING_BITS + 1] {
            let mut actual = transcript();
            let mut untouched = actual.clone();
            assert_eq!(
                derive_grinding_seed(&mut actual, GrindingRound::<InitialMessage>::new(0), bits,),
                Err(GrindingError::InvalidDifficulty { bits })
            );
            assert_eq!(
                actual.get_challenge::<u128>(),
                untouched.get_challenge::<u128>()
            );
        }

        let mut actual = transcript();
        let mut untouched = actual.clone();
        assert_eq!(
            derive_grinding_seed(&mut actual, GrindingRound::<Empty>::new(0), 1),
            Err(GrindingError::EmptyDomain)
        );
        assert_eq!(
            actual.get_challenge::<u128>(),
            untouched.get_challenge::<u128>()
        );
    }

    #[test]
    fn grinding_transcripts_stay_in_lockstep_and_gate_the_nonces() {
        const BITS: u32 = 6;
        let mut prover_inner = transcript();
        let mut prover: ProverGrindingTranscript<_, ForestRoundGrinding> =
            ProverGrindingTranscript::new(&mut prover_inner, BITS);
        let a: u128 = prover.get_challenge();
        prover.absorb_slice(b"round message");
        let b: u128 = prover.get_challenge();
        let nonces = prover.finish();
        assert_eq!(nonces.len(), 2);

        let mut verifier_inner = transcript();
        let mut verifier : VerifierGrindingTranscript<_, ForestRoundGrinding> =
            VerifierGrindingTranscript::new(&mut verifier_inner, BITS, &nonces);
        let va: u128 = verifier.get_challenge();
        verifier.absorb_slice(b"round message");
        let vb: u128 = verifier.get_challenge();
        verifier.finish().unwrap();
        assert_eq!((a, b), (va, vb));

        // A tampered nonce is caught at finish.
        let mut bad = nonces.clone();
        bad[1] ^= 1;
        let mut verifier_inner = transcript();
        let mut verifier : VerifierGrindingTranscript<_, ForestRoundGrinding> =
            VerifierGrindingTranscript::new(&mut verifier_inner, BITS, &bad);
        let _: u128 = verifier.get_challenge();
        verifier.absorb_slice(b"round message");
        let _: u128 = verifier.get_challenge();
        assert!(verifier.finish().is_err());

        // Leftover nonces are caught at finish.
        let mut verifier_inner = transcript();
        let mut verifier : VerifierGrindingTranscript<_, ForestRoundGrinding> =
            VerifierGrindingTranscript::new(&mut verifier_inner, BITS, &nonces);
        let _: u128 = verifier.get_challenge();
        assert!(verifier.finish().is_err());
    }

    #[test]
    fn zero_difficulty_grinding_transcript_is_a_transparent_passthrough() {
        let mut wrapped_inner = transcript();
        let mut wrapped: ProverGrindingTranscript<_, ForestRoundGrinding> =
            ProverGrindingTranscript::new(&mut wrapped_inner, 0);
        wrapped.absorb_slice(b"message");
        let a: u128 = wrapped.get_challenge();
        assert!(wrapped.finish().is_empty());

        let mut plain = transcript();
        plain.absorb_slice(b"message");
        let b: u128 = plain.get_challenge();
        assert_eq!(a, b);

        let mut verifier_inner = transcript();
        let mut verifier: VerifierGrindingTranscript<_, ForestRoundGrinding> =
            VerifierGrindingTranscript::new(&mut verifier_inner, 0, &[]);
        verifier.absorb_slice(b"message");
        let c: u128 = verifier.get_challenge();
        verifier.finish().unwrap();
        assert_eq!(a, c);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_search_returns_the_serial_minimum() {
        let seed = GrindingSeed::from_bytes([0x5a; 32]);
        let serial = find_grinding_nonce_sequential(&seed, 11).unwrap();
        let parallel = find_grinding_nonce_parallel(&seed, 11).unwrap();
        assert_eq!(parallel, serial);
        assert_eq!(find_grinding_nonce(&seed, 11).unwrap(), serial);
    }
}
