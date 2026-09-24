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
}

/// The verifier half of the transcript.
pub struct VerifierState<'a> {
    inner: spongefish::VerifierState<'a>,
    hints: &'a [u8],
}

/// Operations shared by prover and verifier transcripts for public messages.
pub trait PublicTranscript {
    /// Absorbs a message both parties already know.
    fn public_message<T: Encoding<[u8]> + ?Sized>(&mut self, message: &T);

    /// Squeezes a `Gf` challenge.
    fn verifier_message_f128(&mut self) -> Gf;
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

    fn verifier_message_f128(&mut self) -> Gf {
        self.inner.verifier_message()
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

    fn verifier_message_f128(&mut self) -> Gf {
        self.inner.verifier_message()
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
        self.inner.check_eof()?;
        if self.hints.is_empty() {
            Ok(())
        } else {
            Err(VerificationError)
        }
    }
}
