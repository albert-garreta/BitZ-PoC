//! The in-circuit BitZ transcript ([`crate::bitz::transcript::VerifierState`]
//! over the SHA-256 duplex): public messages, prover messages read from the
//! proof's narg string as committed bytes, verifier messages as public
//! squeezes, the hint stream (committed, never absorbed), and the native
//! challenge schedule with its proof-of-work blocks.

use circuit::Circuit;
use field::Gf128 as Gf;
use spongefish::Encoding;

use crate::bitz::grinding::{Cursor, Schedule, Stage};

use super::bytes::{Byte, witness_bytes, witness_u64};
use super::sha::{Duplex, ShaCounts, assert_pow};
use super::val::{G, Public, gf_of};

const NATIVE_POLICY: &[u8] = b"bitz/native-work/v1";
const NATIVE_BLOCK: &[u8] = b"bitz/native-block/v1";
const NATIVE_POW_PREFIX: &[u8] = b"bitz-native-pow1";

/// A read cursor over proof bytes: the real bytes when generating a witness,
/// zeros when only recording the circuit (the shape never depends on them).
#[derive(Clone, Debug, Default)]
pub struct Reader {
    bytes: Option<Vec<u8>>,
    pos: usize,
}

impl Reader {
    pub fn new(bytes: Option<Vec<u8>>) -> Self {
        Self { bytes, pos: 0 }
    }

    pub fn take(&mut self, n: usize) -> Vec<u8> {
        match &self.bytes {
            Some(bytes) => {
                let end = self.pos + n;
                assert!(end <= bytes.len(), "the circuit reads past the proof stream");
                let out = bytes[self.pos..end].to_vec();
                self.pos = end;
                out
            }
            None => vec![0; n],
        }
    }

    /// Whether every byte was read (always true while recording).
    pub fn exhausted(&self) -> bool {
        self.bytes.as_ref().is_none_or(|b| b.len() == self.pos)
    }

    pub fn has_bytes(&self) -> bool {
        self.bytes.is_some()
    }
}

/// The verifier transcript of one BitZ opening.
pub struct BitzTranscript<B: circuit::BoolWitness> {
    pub duplex: Duplex<B>,
    pub narg: Reader,
    pub hints: Reader,
    native: Option<Cursor>,
}

impl<B: circuit::BoolWitness> BitzTranscript<B> {
    /// [`crate::bitz::build_verifier`]: the protocol id, then the session
    /// and instance encodings, all public.
    pub fn new<CS: Circuit<Bool = B>, S, I>(
        cs: &mut CS,
        public: Public,
        session: &S,
        instance: &I,
        proof: Option<&crate::bitz::Proof>,
        log: bool,
    ) -> Self
    where
        S: Encoding<[u8]> + ?Sized,
        I: Encoding<[u8]> + ?Sized,
    {
        let mut duplex = Duplex::new(public);
        if log {
            duplex = duplex.with_log();
        }
        let protocol = spongefish::protocol_id(format_args!(
            "{}",
            crate::bitz::transcript::PROTOCOL_LABEL
        ));
        duplex.absorb_public(cs, &protocol);
        duplex.absorb_public(cs, session.encode().as_ref());
        duplex.absorb_public(cs, instance.encode().as_ref());
        Self {
            duplex,
            narg: Reader::new(proof.map(|p| p.narg_string.clone())),
            hints: Reader::new(proof.map(|p| p.hints.clone())),
            native: None,
        }
    }

    pub fn public(&self) -> &Public {
        self.duplex.public()
    }

    pub fn counts(&self) -> ShaCounts {
        self.duplex.counts
    }

    /// A public message (its canonical encoding).
    pub fn public_message<CS: Circuit<Bool = B>, T: Encoding<[u8]> + ?Sized>(
        &mut self,
        cs: &mut CS,
        message: &T,
    ) {
        self.duplex.absorb_public(cs, message.encode().as_ref());
    }

    /// Absorbs bytes the caller built (public or committed).
    pub fn absorb<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, bytes: &[Byte<B>]) {
        self.duplex.absorb(cs, bytes);
    }

    /// Absorbs `public ‖ committed` as one string (see [`Duplex::absorb_split`]).
    pub fn absorb_split<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, public: &[u8], committed: &[Byte<B>]) {
        self.duplex.absorb_split(cs, public, committed);
    }

    /// The next `n` narg bytes as committed bytes, absorbed.
    pub fn prover_bytes<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<Byte<B>> {
        let raw = self.narg.take(n);
        let bytes = witness_bytes(cs, &raw);
        self.duplex.absorb(cs, &bytes);
        bytes
    }

    /// The next narg element of GF(2^128), committed and absorbed.
    pub fn prover_gf<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) -> G<B> {
        let raw = self.narg.take(16);
        let value = gf_of(u128::from_le_bytes(raw.try_into().unwrap()));
        let g = super::val::witness(cs, value);
        self.duplex.absorb(cs, &super::bytes::gf_bytes(&g));
        g
    }

    /// The next narg `u64` (a nonce), committed and absorbed.
    pub fn prover_u64<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) -> [B; 64] {
        let raw = self.narg.take(8);
        let value = u64::from_le_bytes(raw.try_into().unwrap());
        let bits = witness_u64(cs, value);
        let bytes = super::bytes::bytes_of_bits(&bits);
        self.duplex.absorb(cs, &bytes);
        bits
    }

    /// The next `n` hint bytes, committed (hints bypass the sponge).
    pub fn hint_bytes<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<Byte<B>> {
        let raw = self.hints.take(n);
        witness_bytes(cs, &raw)
    }

    /// Raw hint bytes for native decoding (witness generation only).
    pub fn hint_raw(&mut self, n: usize) -> Vec<u8> {
        self.hints.take(n)
    }

    /// A verifier message: `n` public squeezed bytes.
    pub fn verifier_bytes<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<u8> {
        self.duplex.squeeze(cs, n)
    }

    /// A verifier message of which only the low `k` bits are published.
    pub fn verifier_low_bits<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize, k: usize) -> u128 {
        self.duplex.squeeze_low_bits(cs, n, k)
    }

    pub fn verifier_gf<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) -> Gf {
        let bytes = self.duplex.squeeze(cs, 16);
        Gf::from_bytes(bytes.try_into().unwrap())
    }

    pub fn start_native<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, schedule: Schedule) {
        assert!(self.native.is_none(), "native schedule already started");
        if schedule.has_work() {
            self.public_message(cs, NATIVE_POLICY);
            self.public_message(cs, &schedule.frame());
        }
        self.native = Some(schedule.cursor());
    }

    pub fn continue_native<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, schedule: Schedule) {
        if let Some(cursor) = &self.native {
            cursor
                .continuation(schedule)
                .expect("public native schedule continuation");
        } else {
            self.start_native(cs, schedule);
        }
    }

    pub fn finish_native(&self) {
        self.native
            .expect("native schedule")
            .finish()
            .expect("the circuit follows the whole native schedule");
    }

    fn protect<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, stage: Stage, count: usize) {
        if count == 0 {
            return;
        }
        let block = self
            .native
            .as_mut()
            .expect("native challenge without a schedule")
            .take(stage, count)
            .expect("the circuit follows the native schedule");
        if block.bits != 0 {
            self.public_message(cs, NATIVE_BLOCK);
            self.public_message(cs, &block.frame());
            let seed = self.duplex.squeeze_bytes(cs, 16);
            let nonce = self.prover_u64(cs);
            let mut prefix = super::bytes::public_bytes(NATIVE_POW_PREFIX);
            prefix.extend(seed);
            let mut counts = ShaCounts::default();
            assert_pow(cs, &prefix, &nonce, block.bits, &mut counts);
            self.duplex.counts.circuit += counts.circuit;
            self.duplex.counts.native += counts.native;
        }
    }

    pub fn native_scalar<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, stage: Stage) -> Gf {
        self.protect(cs, stage, 1);
        self.verifier_gf(cs)
    }

    pub fn native_vector<CS: Circuit<Bool = B>>(
        &mut self,
        cs: &mut CS,
        stage: Stage,
        count: usize,
    ) -> Vec<Gf> {
        self.protect(cs, stage, count);
        (0..count).map(|_| self.verifier_gf(cs)).collect()
    }

    /// Both streams consumed exactly (witness generation).
    pub fn check_eof(&self) {
        if let Some(cursor) = self.native {
            cursor.finish().expect("native schedule complete");
        }
        assert!(self.narg.exhausted(), "narg bytes left unread");
        assert!(self.hints.exhausted(), "hint bytes left unread");
    }
}
