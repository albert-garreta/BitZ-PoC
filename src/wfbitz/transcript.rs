//! Their `transcript` crate: spongefish plus a hint channel.
//!
//! Every prover and verifier message passes through this layer. The prover
//! writes the narg string, hints ride in a second byte vector the sponge
//! never sees, and the proof is the pair of both. Framing rules F1–F6 of
//! their crate apply unchanged: protocol id, session and instance are
//! domain tags absorbed at construction; every prover message is absorbed
//! through its canonical encoding; challenges leave only through
//! `verifier_message`; hints bypass the sponge; field wire formats are
//! fixed in [`super::codec`]; records are not self-delimiting.

use spongefish::{
    Decoding, DomainSeparator, Encoding, NargDeserialize, NargSerialize, VerificationError,
    VerificationResult, protocol_id,
};

use field::Gf128 as Gf;
use super::grinding::{self, Cursor, Schedule, Stage};

/// What this protocol is. Changing it invalidates every existing proof.
pub const PROTOCOL_LABEL: &str = "bitz/v1";

/// A finished transcript: the narg string and the hint stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Proof {
    pub narg_string: Vec<u8>,
    pub hints: Vec<u8>,
}

/// The prover half of the transcript.
pub struct ProverState {
    inner: spongefish::ProverState,
    hints: Vec<u8>,
    native: Option<Cursor>,
    #[cfg(test)]
    unscheduled_kernel: bool,
}

/// The verifier half of the transcript.
pub struct VerifierState<'a> {
    inner: spongefish::VerifierState<'a>,
    hints: &'a [u8],
    native: Option<Cursor>,
    #[cfg(test)]
    unscheduled_kernel: bool,
}

/// Operations shared by prover and verifier transcripts for public messages.
pub trait PublicTranscript {
    /// Absorbs a message both parties already know.
    fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T);
}

/// Starts a prover transcript with the session and instance domain tags.
pub fn build_prover<S, I>(session: &S, instance: &I) -> ProverState
where
    S: Encoding<[u8]> + ?Sized,
    I: Encoding<[u8]> + ?Sized,
{
    let inner = DomainSeparator::new(protocol_id(format_args!("{PROTOCOL_LABEL}")))
        .session(session)
        .instance(instance)
        .std_prover();
    ProverState {
        inner,
        hints: Vec::new(),
        native: None,
        #[cfg(test)]
        unscheduled_kernel: false,
    }
}

/// Starts a verifier transcript over a proof, with the same tags.
pub fn build_verifier<'a, S, I>(session: &S, instance: &I, proof: &'a Proof) -> VerifierState<'a>
where
    S: Encoding<[u8]> + ?Sized,
    I: Encoding<[u8]> + ?Sized,
{
    let inner = DomainSeparator::new(protocol_id(format_args!("{PROTOCOL_LABEL}")))
        .session(session)
        .instance(instance)
        .std_verifier(&proof.narg_string);
    VerifierState {
        inner,
        hints: &proof.hints,
        native: None,
        #[cfg(test)]
        unscheduled_kernel: false,
    }
}

/// A length-prefixed byte string as one prover message (their
/// `ProverMessageBytes`).
pub(crate) struct ProverMessageBytes<const MAX_LEN: usize>(Vec<u8>);

impl<const MAX_LEN: usize> ProverMessageBytes<MAX_LEN> {
    pub(crate) fn new(bytes: &[u8]) -> Self {
        let len = u32::try_from(bytes.len()).expect("prover message byte string exceeds u32");
        let mut encoded = Vec::with_capacity(size_of::<u32>() + bytes.len());
        encoded.extend_from_slice(&len.to_le_bytes());
        encoded.extend_from_slice(bytes);
        Self(encoded)
    }

    pub(crate) fn into_bytes(mut self) -> Vec<u8> {
        self.0.drain(..size_of::<u32>());
        self.0
    }
}

impl<const MAX_LEN: usize> Encoding<[u8]> for ProverMessageBytes<MAX_LEN> {
    fn encode(&self) -> impl AsRef<[u8]> {
        &self.0
    }
}

impl<const MAX_LEN: usize> NargDeserialize for ProverMessageBytes<MAX_LEN> {
    fn deserialize_from_narg(buf: &mut &[u8]) -> VerificationResult<Self> {
        let mut rest = *buf;
        let len = u32::deserialize_from_narg(&mut rest)? as usize;
        if len > MAX_LEN || rest.len() < len {
            return Err(VerificationError);
        }
        let message = Self::new(&rest[..len]);
        *buf = &rest[len..];
        Ok(message)
    }
}

impl PublicTranscript for ProverState {
    fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T) {
        self.inner.public_message(message);
    }
}

impl ProverState {
    /// Absorbs a message both parties already know; nothing is written.
    pub fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T) {
        self.inner.public_message(message);
    }

    /// Absorbs a message and writes it to the narg string.
    pub fn prover_message<T: Encoding<[u8]> + NargSerialize + ?Sized>(&mut self, message: &T) {
        self.inner.prover_message(message);
    }

    /// Absorbs and writes one length-prefixed byte string.
    pub fn prover_message_bytes(&mut self, bytes: &[u8]) {
        self.inner
            .prover_message(&ProverMessageBytes::<{ u32::MAX as usize }>::new(bytes));
    }

    /// Squeezes a challenge.
    pub fn verifier_message<T: Decoding<[u8]>>(&mut self) -> T {
        self.inner.verifier_message()
    }

    /// Writes a value to the hint stream; the sponge is untouched.
    pub fn hint<T: NargSerialize + ?Sized>(&mut self, hint: &T) {
        hint.serialize_into_narg(&mut self.hints);
    }

    /// Writes one length-prefixed byte string to the hint stream.
    pub fn hint_bytes(&mut self, bytes: &[u8]) {
        u32::try_from(bytes.len())
            .expect("hint byte string exceeds u32")
            .serialize_into_narg(&mut self.hints);
        self.hints.extend_from_slice(bytes);
    }

    pub fn finish(self) -> Proof {
        if let Some(cursor) = self.native {
            cursor.finish().expect("incomplete native challenge schedule");
        }
        Proof {
            narg_string: self.inner.narg_string().to_vec(),
            hints: self.hints,
        }
    }
}

impl PublicTranscript for VerifierState<'_> {
    fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T) {
        self.inner.public_message(message);
    }
}

impl VerifierState<'_> {
    /// Absorbs a message both parties already know; nothing is read.
    pub fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T) {
        self.inner.public_message(message);
    }

    /// Reads the next prover message and absorbs its canonical re-encoding.
    pub fn prover_message<T: Encoding<[u8]> + NargDeserialize>(&mut self) -> VerificationResult<T> {
        self.inner.prover_message()
    }

    /// Reads and absorbs one bounded, length-prefixed byte string.
    pub fn prover_message_bytes<const MAX_LEN: usize>(&mut self) -> VerificationResult<Vec<u8>> {
        self.inner
            .prover_message::<ProverMessageBytes<MAX_LEN>>()
            .map(ProverMessageBytes::into_bytes)
    }

    /// Squeezes a challenge.
    pub fn verifier_message<T: Decoding<[u8]>>(&mut self) -> T {
        self.inner.verifier_message()
    }

    /// Reads the next value from the hint stream; the sponge is untouched.
    pub fn hint<T: NargDeserialize>(&mut self) -> VerificationResult<T> {
        T::deserialize_from_narg(&mut self.hints)
    }

    /// Reads one bounded, length-prefixed byte string from the hint stream.
    pub fn hint_bytes(&mut self, max_len: usize) -> VerificationResult<Vec<u8>> {
        let mut rest = self.hints;
        let len = u32::deserialize_from_narg(&mut rest)? as usize;
        if len > max_len || rest.len() < len {
            return Err(VerificationError);
        }
        let bytes = rest[..len].to_vec();
        self.hints = &rest[len..];
        Ok(bytes)
    }

    /// Fails unless both the narg string and the hint stream were consumed
    /// exactly.
    pub fn check_eof(self) -> VerificationResult<()> {
        if let Some(cursor) = self.native {
            cursor.finish().map_err(|_| VerificationError)?;
        }
        self.inner.check_eof()?;
        if self.hints.is_empty() {
            Ok(())
        } else {
            Err(VerificationError)
        }
    }
}

const NATIVE_POLICY: &[u8] = b"bitz/native-work/v1";
const NATIVE_BLOCK: &[u8] = b"bitz/native-block/v1";

macro_rules! native_schedule {
    () => {
        pub(crate) fn start_native(&mut self, schedule: Schedule) -> Result<(), grinding::Error> {
            if self.native.is_some() { return Err(grinding::Error::Sequence); }
            if schedule.has_work() {
                self.public_message(NATIVE_POLICY);
                self.public_message(&schedule.frame());
            }
            self.native = Some(schedule.cursor());
            Ok(())
        }

        pub(crate) fn continue_native(&mut self, schedule: Schedule) -> Result<(), grinding::Error> {
            if let Some(cursor) = &self.native {
                cursor.continuation(schedule)
            } else {
                self.start_native(schedule)
            }
        }

        pub(crate) fn finish_native(&self) -> Result<(), grinding::Error> {
            self.native.ok_or(grinding::Error::Incomplete)?.finish()
        }
    };
}

impl ProverState {
    native_schedule!();

    fn protect(&mut self, stage: Stage, count: usize) {
        if count == 0 { return; }
        let Some(cursor) = self.native.as_mut() else {
            #[cfg(test)]
            if self.unscheduled_kernel { return; }
            panic!("native challenge without a prepared schedule");
        };
        let block = cursor.take(stage, count).expect("native challenge schedule");
        if block.bits != 0 {
            self.public_message(NATIVE_BLOCK);
            self.public_message(&block.frame());
            let seed = super::codec::gf_to_bytes(self.verifier_message::<Gf>());
            let nonce = grinding::grind(&seed, block.bits);
            self.prover_message(&nonce.to_le_bytes());
        }
    }

    pub(crate) fn native_scalar(&mut self, stage: Stage) -> Gf {
        self.protect(stage, 1);
        self.verifier_message()
    }

    pub(crate) fn native_array<const N: usize>(&mut self, stage: Stage) -> [Gf; N] {
        self.protect(stage, N);
        core::array::from_fn(|_| self.verifier_message())
    }

    pub(crate) fn native_vector(&mut self, stage: Stage, count: usize) -> Vec<Gf> {
        self.protect(stage, count);
        (0..count).map(|_| self.verifier_message()).collect()
    }
}

impl VerifierState<'_> {
    native_schedule!();

    fn protect(&mut self, stage: Stage, count: usize) -> VerificationResult<()> {
        if count == 0 { return Ok(()); }
        let Some(cursor) = self.native.as_mut() else {
            #[cfg(test)]
            if self.unscheduled_kernel { return Ok(()); }
            return Err(VerificationError);
        };
        let block = cursor.take(stage, count).map_err(|_| VerificationError)?;
        if block.bits != 0 {
            self.public_message(NATIVE_BLOCK);
            self.public_message(&block.frame());
            let seed = super::codec::gf_to_bytes(self.verifier_message::<Gf>());
            let nonce = u64::from_le_bytes(self.prover_message::<[u8; 8]>()?);
            if !grinding::valid(&seed, nonce, block.bits) { return Err(VerificationError); }
        }
        Ok(())
    }

    pub(crate) fn native_scalar(&mut self, stage: Stage) -> VerificationResult<Gf> {
        self.protect(stage, 1)?;
        Ok(self.verifier_message())
    }

    pub(crate) fn native_array<const N: usize>(&mut self, stage: Stage) -> VerificationResult<[Gf; N]> {
        self.protect(stage, N)?;
        Ok(core::array::from_fn(|_| self.verifier_message()))
    }

    pub(crate) fn native_vector(&mut self, stage: Stage, count: usize) -> VerificationResult<Vec<Gf>> {
        self.protect(stage, count)?;
        Ok((0..count).map(|_| self.verifier_message()).collect())
    }
}

// Isolated arithmetic tests intentionally stop at intermediate native messages.
#[cfg(test)]
pub(crate) fn build_kernel_prover<S: Encoding<[u8]> + ?Sized, I: Encoding<[u8]> + ?Sized>(
    session: &S, instance: &I,
) -> ProverState {
    let mut state = build_prover(session, instance);
    state.unscheduled_kernel = true;
    state
}

#[cfg(test)]
pub(crate) fn build_kernel_verifier<'a, S: Encoding<[u8]> + ?Sized, I: Encoding<[u8]> + ?Sized>(
    session: &S, instance: &I, proof: &'a Proof,
) -> VerifierState<'a> {
    let mut state = build_verifier(session, instance, proof);
    state.unscheduled_kernel = true;
    state
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use crate::wfbitz::grinding::{Geometry, Policy};

    #[test]
    fn zero_work_keeps_the_existing_scalar_squeeze_stream() {
        let geometry = Geometry { integer: None, binary: Some((2, 1)), ring: true, ood: true };
        let schedule = Schedule::new(Policy::new(Some(100), 0, 0).unwrap(), geometry).unwrap();
        let mut native = build_prover(b"zero-work", b"fixture");
        let mut reference = build_prover(b"zero-work", b"fixture");
        native.start_native(schedule).unwrap();
        for (stage, count) in [(Stage::BinaryRound, 1), (Stage::BinaryRound, 1),
            (Stage::BinaryRound, 1), (Stage::RingBatch, 7), (Stage::OodBatch, 1)] {
            native.prover_message(&[Gf::one(), Gf::zero()]);
            reference.prover_message(&[Gf::one(), Gf::zero()]);
            let expected: Vec<Gf> = (0..count).map(|_| reference.verifier_message()).collect();
            assert_eq!(native.native_vector(stage, count), expected);
        }
        assert_eq!(native.finish(), reference.finish());
    }

    #[test]
    fn native_work_replays_and_rejects_bad_or_truncated_nonces() {
        let geometry = Geometry { integer: None, binary: None, ring: true, ood: false };
        let schedule = Schedule::new(Policy::new(Some(128), 0, 0).unwrap(), geometry).unwrap();
        let mut prover = build_prover(b"positive-work", b"fixture");
        prover.start_native(schedule).unwrap();
        let expected = prover.native_array::<7>(Stage::RingBatch);
        let proof = prover.finish();
        assert_eq!(proof.narg_string.len(), 8);
        let mut verifier = build_verifier(b"positive-work", b"fixture", &proof);
        verifier.start_native(schedule).unwrap();
        assert_eq!(verifier.native_array::<7>(Stage::RingBatch).unwrap(), expected);
        verifier.check_eof().unwrap();

        let rejected = (0u64..1024).any(|nonce| {
            let changed = Proof { narg_string: nonce.to_le_bytes().to_vec(), hints: Vec::new() };
            let mut verifier = build_verifier(b"positive-work", b"fixture", &changed);
            verifier.start_native(schedule).unwrap();
            verifier.native_array::<7>(Stage::RingBatch).is_err()
        });
        assert!(rejected, "invalid work must be rejected before returning challenges");
        for len in 0..8 {
            let truncated = Proof { narg_string: proof.narg_string[..len].to_vec(), hints: Vec::new() };
            let mut verifier = build_verifier(b"positive-work", b"fixture", &truncated);
            verifier.start_native(schedule).unwrap();
            assert!(verifier.native_array::<7>(Stage::RingBatch).is_err());
        }
    }

    #[test]
    fn a_prepared_schedule_cannot_be_skipped_at_finalization() {
        let geometry = Geometry { integer: None, binary: None, ring: true, ood: false };
        let schedule = Schedule::new(Policy::UNGRINDED, geometry).unwrap();
        let mut prover = build_prover(b"incomplete", b"fixture");
        prover.start_native(schedule).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prover.finish())).is_err());
        let proof = Proof::default();
        let mut verifier = build_verifier(b"incomplete", b"fixture", &proof);
        verifier.start_native(schedule).unwrap();
        assert!(verifier.check_eof().is_err());
        let mut missing = build_verifier(b"missing", b"fixture", &proof);
        assert!(missing.native_scalar(Stage::GkrRound).is_err());
    }
}
