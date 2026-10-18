//! SHA-256 in the verifier circuit: the compression duplex of
//! [`crate::transcript::sha`] (both transcripts), the single-block proof of
//! work, and standard padded SHA-256 of fixed-length messages (Merkle
//! leaves and nodes).
//!
//! Every in-circuit compression is one [`circuit::sha256::compress_block`]
//! instance (one shared template). A compression whose chaining value and
//! block are both public runs natively and costs nothing in the circuit;
//! after a squeeze the duplex PUBLISHES its new chaining value (the 32
//! squeezed bytes are the challenges), so public absorbs that follow a
//! squeeze stay native until the first proof byte arrives.

use std::array;

use circuit::Circuit;
use circuit::sha256::{Word, compress_block};

use crate::transcript::sha::{
    ABSORB, ABSORB_END, IV, RATCHET, RATE, SQUEEZE, TraceOp, compress as compress_native,
    state_bytes,
};

use super::bytes::{Byte, block_words, bytes_of_state, public_bytes};
use super::val::{Public, publish};

/// A chaining value: public words, or eight in-circuit words.
#[derive(Clone)]
pub enum Cv<B: circuit::BoolWitness> {
    P([u32; 8]),
    V([Word<B, 32>; 8]),
}

impl<B: circuit::BoolWitness> Cv<B> {
    fn words(&self) -> [Word<B, 32>; 8] {
        match self {
            Self::P(v) => v.map(|w| Word::constant(u64::from(w))),
            Self::V(w) => w.clone(),
        }
    }

    /// The chaining value as words (constants when public).
    pub fn into_words(self) -> [Word<B, 32>; 8] {
        self.words()
    }
}

/// Counts of compressions, split by where they run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShaCounts {
    /// In-circuit compression blocks.
    pub circuit: usize,
    /// Compressions computed natively (public inputs).
    pub native: usize,
}

/// One compression of `block` into `cv`: native when both are public.
pub fn compress<CS: Circuit>(
    cs: &mut CS,
    cv: &Cv<CS::Bool>,
    block: &[Byte<CS::Bool>],
    counts: &mut ShaCounts,
) -> Cv<CS::Bool> {
    assert_eq!(block.len(), 64);
    if let Cv::P(state) = cv
        && let Some(bytes) = block
            .iter()
            .map(|b| match b {
                Byte::P(v) => Some(*v),
                Byte::V(_) => None,
            })
            .collect::<Option<Vec<u8>>>()
    {
        let mut state = *state;
        let block: [u8; 64] = bytes.try_into().unwrap_or_else(|_| unreachable!());
        compress_native(&mut state, &block);
        counts.native += 1;
        return Cv::P(state);
    }
    counts.circuit += 1;
    Cv::V(compress_block(cs, &block_words(block), &cv.words()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Absorbing,
    Squeezing(usize),
}

/// The in-circuit [`crate::transcript::ShaDuplex`]: the same state machine
/// over public-or-committed bytes.
///
/// A squeeze block's bytes are published only as they are consumed, and only
/// as far as the protocol needs them: a field challenge publishes its 16
/// bytes, a query position only its low bits ([`Duplex::squeeze_low_bits`]),
/// a proof-of-work seed nothing ([`Duplex::squeeze_bytes`]). Once all 32
/// bytes of a block are published the chaining value is public again, so
/// public absorbs that follow run natively.
#[derive(Clone)]
pub struct Duplex<B: circuit::BoolWitness> {
    cv: Cv<B>,
    pending: Vec<Byte<B>>,
    /// The current squeeze block and the published value of each byte.
    block: Vec<Byte<B>>,
    published: [Option<u8>; 32],
    /// All-public blocks waiting while the chaining value is committed: a
    /// run of at least [`PUBLISH_RUN`] is hashed natively after publishing
    /// the chaining value once (32 bytes), a shorter one in the circuit.
    deferred: Vec<[u8; 64]>,
    mode: Mode,
    public: Public,
    pub counts: ShaCounts,
    /// With a witness backend and logging on: the operations with concrete
    /// bytes, in the native trace's format.
    pub log: Option<Vec<TraceOp>>,
}

impl<B: circuit::BoolWitness> Duplex<B> {
    pub fn new(public: Public) -> Self {
        Self {
            cv: Cv::P(IV),
            pending: Vec::with_capacity(RATE),
            block: Vec::new(),
            published: [None; 32],
            deferred: Vec::new(),
            mode: Mode::Absorbing,
            public,
            counts: ShaCounts::default(),
            log: None,
        }
    }

    /// Records concrete operations (witness backends only; see
    /// [`crate::transcript::sha::start_trace`] for the native side).
    pub fn with_log(mut self) -> Self {
        self.log = Some(Vec::new());
        self
    }

    pub fn public(&self) -> &Public {
        &self.public
    }

    /// Whether the whole state is public (a native replay could continue).
    pub fn is_public(&self) -> bool {
        matches!(self.cv, Cv::P(_)) && self.deferred.is_empty() && self.pending.iter().all(Byte::is_public)
    }

    fn compress_framed<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, len: usize, kind: u8) {
        let mut block: Vec<Byte<B>> = self.pending[..len].to_vec();
        block.resize(RATE, Byte::P(0));
        block.push(Byte::P(len as u8));
        block.push(Byte::P(kind));
        self.compress_block(cs, block);
    }

    /// One compression of the duplex's chaining value, deferring all-public
    /// blocks while the chaining value is committed.
    fn compress_block<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, block: Vec<Byte<B>>) {
        let public: Option<Vec<u8>> = block
            .iter()
            .map(|b| match b {
                Byte::P(v) => Some(*v),
                Byte::V(_) => None,
            })
            .collect();
        match (&self.cv, public) {
            (Cv::V(_), Some(bytes)) => self.deferred.push(bytes.try_into().unwrap()),
            _ => {
                self.flush(cs);
                self.cv = compress(cs, &self.cv, &block, &mut self.counts);
            }
        }
    }

    /// Resolves the deferred run (see [`Duplex::deferred`]).
    fn flush<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        if self.deferred.is_empty() {
            return;
        }
        let run = std::mem::take(&mut self.deferred);
        if run.len() >= PUBLISH_RUN {
            if let Cv::V(words) = &self.cv {
                let bits = super::bytes::bits_of_bytes(&bytes_of_state(words));
                let lo = publish(cs, &self.public, &bits[..128], 128);
                let hi = publish(cs, &self.public, &bits[128..], 128);
                let mut raw = [0u8; 32];
                raw[..16].copy_from_slice(&lo.to_le_bytes());
                raw[16..].copy_from_slice(&hi.to_le_bytes());
                self.cv = Cv::P(std::array::from_fn(|w| u32::from_be_bytes(raw[4 * w..4 * w + 4].try_into().unwrap())));
            }
        }
        for bytes in run {
            self.cv = compress(cs, &self.cv, &public_bytes(&bytes), &mut self.counts);
        }
    }

    fn log_bytes<CS: Circuit<Bool = B>>(cs: &CS, bytes: &[Byte<B>]) -> Vec<u8> {
        bytes
            .iter()
            .map(|b| (0..8).fold(0u8, |v, i| v | (u8::from(cs.try_eval(&b.bit(i)).expect("logging needs a witness backend")) << i)))
            .collect()
    }

    pub fn absorb<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, input: &[Byte<B>]) {
        if self.log.is_some() {
            let bytes = Self::log_bytes(cs, input);
            self.log.as_mut().unwrap().push(TraceOp::Absorb(bytes));
        }
        if let Mode::Squeezing(_) = self.mode {
            self.mode = Mode::Absorbing;
        }
        for byte in input {
            if self.pending.len() == RATE {
                self.compress_framed(cs, RATE, ABSORB);
                self.pending.clear();
            }
            self.pending.push(byte.clone());
        }
    }

    /// [`Duplex::absorb`] of public bytes. Whole blocks are hashed (or
    /// deferred) straight from the bytes, without per-byte handles: the same
    /// blocks in the same order as the byte loop forms them.
    pub fn absorb_public<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, input: &[u8]) {
        if self.log.is_some() {
            return self.absorb(cs, &public_bytes(input));
        }
        if let Mode::Squeezing(_) = self.mode {
            self.mode = Mode::Absorbing;
        }
        let mut rest = input;
        while !rest.is_empty() {
            // A full pending block is hashed once another byte follows.
            if self.pending.len() == RATE {
                self.compress_framed(cs, RATE, ABSORB);
                self.pending.clear();
            }
            if self.pending.is_empty() && rest.len() > RATE {
                let mut block = [0u8; 64];
                block[..RATE].copy_from_slice(&rest[..RATE]);
                block[RATE] = RATE as u8;
                block[RATE + 1] = ABSORB;
                self.compress_public_block(cs, block);
                rest = &rest[RATE..];
                continue;
            }
            let take = (RATE - self.pending.len()).min(rest.len());
            self.pending.extend(rest[..take].iter().map(|&b| Byte::P(b)));
            rest = &rest[take..];
        }
    }

    /// [`Duplex::compress_block`] of an all-public block.
    fn compress_public_block<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, bytes: [u8; 64]) {
        if matches!(self.cv, Cv::V(_)) {
            self.deferred.push(bytes);
            return;
        }
        self.flush(cs);
        match &self.cv {
            Cv::P(state) => {
                let mut state = *state;
                compress_native(&mut state, &bytes);
                self.counts.native += 1;
                self.cv = Cv::P(state);
            }
            Cv::V(_) => self.cv = compress(cs, &self.cv, &public_bytes(&bytes), &mut self.counts),
        }
    }

    /// [`Duplex::absorb`] of `public ‖ committed` (one absorbed string).
    pub fn absorb_split<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, public: &[u8], committed: &[Byte<B>]) {
        if self.log.is_some() {
            let mut all = public_bytes(public);
            all.extend_from_slice(committed);
            return self.absorb(cs, &all);
        }
        self.absorb_public(cs, public);
        self.absorb(cs, committed);
    }

    fn close_absorb<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        if self.mode == Mode::Absorbing {
            let len = self.pending.len();
            self.compress_framed(cs, len, ABSORB_END);
            self.pending.clear();
            self.mode = Mode::Squeezing(0);
        }
    }

    /// A fresh squeeze block: `cv ← f(cv, SQUEEZE)`, its bytes unpublished
    /// unless the chaining value was public.
    fn next_block<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        self.compress_block(cs, squeeze_block::<B>(SQUEEZE));
        self.flush(cs);
        match &self.cv {
            Cv::P(state) => {
                let bytes = state_bytes(state);
                self.block = public_bytes(&bytes);
                self.published = bytes.map(Some);
            }
            Cv::V(words) => {
                self.block = bytes_of_state(words);
                self.published = [None; 32];
            }
        }
    }

    /// The chaining value turns public once every byte of its block is.
    fn settle_block(&mut self) {
        if matches!(self.cv, Cv::V(_)) && self.published.iter().all(Option::is_some) {
            let bytes: [u8; 32] = self.published.map(|b| b.unwrap());
            self.cv = Cv::P(std::array::from_fn(|w| u32::from_be_bytes(bytes[4 * w..4 * w + 4].try_into().unwrap())));
        }
    }

    /// Serves `n` squeezed bytes with their block positions.
    fn take<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<(usize, Byte<B>)> {
        self.close_absorb(cs);
        let Mode::Squeezing(mut left) = self.mode else {
            unreachable!()
        };
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if left == 0 {
                self.next_block(cs);
                left = 32;
            }
            let pos = 32 - left;
            out.push((pos, self.block[pos].clone()));
            left -= 1;
        }
        self.mode = Mode::Squeezing(left);
        out
    }

    fn log_squeeze<CS: Circuit<Bool = B>>(&mut self, cs: &CS, bytes: &[Byte<B>]) {
        if self.log.is_some() {
            let concrete = Self::log_bytes(cs, bytes);
            self.log.as_mut().unwrap().push(TraceOp::Squeeze(concrete));
        }
    }

    /// Squeezes `n` bytes and publishes them: block by block, one public
    /// value per run of at most 16 bytes, the block's chaining value public
    /// once all its bytes are.
    pub fn squeeze<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<u8> {
        self.close_absorb(cs);
        let mut out = Vec::with_capacity(n);
        let mut logged: Vec<Byte<B>> = Vec::with_capacity(n);
        while out.len() < n {
            let Mode::Squeezing(left) = self.mode else {
                unreachable!()
            };
            if left == 0 {
                self.next_block(cs);
                self.mode = Mode::Squeezing(32);
                continue;
            }
            let start = 32 - left;
            let len = left.min(n - out.len()).min(16);
            let run: Vec<Byte<B>> = self.block[start..start + len].to_vec();
            logged.extend_from_slice(&run);
            let values: Vec<u8> = if run.iter().all(Byte::is_public) {
                run.iter()
                    .map(|b| match b {
                        Byte::P(v) => *v,
                        Byte::V(_) => unreachable!(),
                    })
                    .collect()
            } else {
                let bits = super::bytes::bits_of_bytes(&run);
                let value = publish(cs, &self.public, &bits, bits.len() as u8);
                value.to_le_bytes()[..len].to_vec()
            };
            for (k, v) in values.iter().enumerate() {
                self.published[start + k] = Some(*v);
            }
            self.settle_block();
            out.extend(values);
            self.mode = Mode::Squeezing(left - len);
        }
        self.log_squeeze(cs, &logged);
        out
    }

    /// Squeezes `n ≤ 16` bytes and publishes only the low `k` bits of their
    /// little-endian value (a query position); the rest stay committed.
    pub fn squeeze_low_bits<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize, k: usize) -> u128 {
        assert!(n <= 16 && k <= 8 * n);
        let taken = self.take(cs, n);
        let bytes: Vec<Byte<B>> = taken.iter().map(|(_, b)| b.clone()).collect();
        self.log_squeeze(cs, &bytes);
        if bytes.iter().all(Byte::is_public) {
            let mut v = 0u128;
            for (i, b) in bytes.iter().enumerate() {
                if let Byte::P(x) = b {
                    v |= u128::from(*x) << (8 * i);
                }
            }
            return v & if k == 128 { u128::MAX } else { (1u128 << k) - 1 };
        }
        let bits = super::bytes::bits_of_bytes(&bytes);
        publish(cs, &self.public, &bits[..k], k as u8)
    }

    /// Squeezes `n` bytes without publishing them (they feed later
    /// in-circuit hashing only, like a proof-of-work seed).
    pub fn squeeze_bytes<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<Byte<B>> {
        let bytes: Vec<Byte<B>> = self.take(cs, n).into_iter().map(|(_, b)| b).collect();
        self.log_squeeze(cs, &bytes);
        bytes
    }

    pub fn ratchet<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        if let Some(log) = &mut self.log {
            log.push(TraceOp::Ratchet);
        }
        self.close_absorb(cs);
        self.compress_block(cs, squeeze_block::<B>(RATCHET));
        self.flush(cs);
        self.mode = Mode::Absorbing;
    }

    /// Resolves any deferred blocks (call before reading the state).
    pub fn finish<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        self.flush(cs);
    }
}

/// The shortest run of public blocks worth publishing the chaining value for
/// (32 public bytes against this many in-circuit compressions).
pub const PUBLISH_RUN: usize = 4;

fn squeeze_block<B: circuit::BoolWitness>(kind: u8) -> Vec<Byte<B>> {
    let mut block = vec![Byte::P(0); 64];
    block[RATE] = Byte::P(0);
    block[RATE + 1] = Byte::P(kind);
    block
}

/// Standard SHA-256 of a message whose length is fixed when the circuit is
/// built (Merkle leaves and nodes): Merkle–Damgård from the IV with the
/// standard padding.
pub fn sha256<CS: Circuit>(
    cs: &mut CS,
    message: &[Byte<CS::Bool>],
    counts: &mut ShaCounts,
) -> [Word<CS::Bool, 32>; 8] {
    let len = message.len();
    let mut padded: Vec<Byte<CS::Bool>> = message.to_vec();
    padded.push(Byte::P(0x80));
    while padded.len() % 64 != 56 {
        padded.push(Byte::P(0));
    }
    padded.extend(public_bytes(&((len as u64) * 8).to_be_bytes()));
    let mut cv: Cv<CS::Bool> = Cv::P(IV);
    for block in padded.chunks(64) {
        cv = compress(cs, &cv, block, counts);
    }
    cv.words()
}

/// The proof-of-work predicate `SHA-256(prefix ‖ nonce_le)` has `bits`
/// leading zero bits (most significant bit of byte 0 first), for a public
/// prefix and a committed nonce: one compression.
pub fn assert_pow<CS: Circuit>(
    cs: &mut CS,
    prefix: &[Byte<CS::Bool>],
    nonce: &[CS::Bool; 64],
    bits: u32,
    counts: &mut ShaCounts,
) {
    let mut message: Vec<Byte<CS::Bool>> = prefix.to_vec();
    message.extend(nonce.chunks(8).map(|c| Byte::V(array::from_fn(|i| c[i].clone()))));
    let digest = sha256(cs, &message, counts);
    let bytes = bytes_of_state(&digest);
    let zeros: Vec<CS::Bool> = (0..bits as usize)
        .map(|k| bytes[k / 8].bit(7 - k % 8))
        .collect();
    circuit::gf128::assert_zero(cs, &zeros);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recursion::bytes::witness_bytes;
    use crate::recursion::val::PublicStream;
    use crate::transcript::ShaDuplex;
    use circuit::compact::CompactRecorder;
    use circuit::witgen::CheckingWitgen;
    use sha2::Digest;

    #[derive(Clone)]
    enum Op {
        Public(Vec<u8>),
        Proof(Vec<u8>),
        Squeeze(usize),
        Ratchet,
    }

    fn ops() -> Vec<Op> {
        let data = |n: usize, seed: u8| -> Vec<u8> {
            (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
        };
        vec![
            Op::Public(data(70, 1)),
            Op::Squeeze(16),
            Op::Public(data(5, 2)),
            Op::Proof(data(48, 3)),
            Op::Squeeze(16),
            Op::Squeeze(40),
            Op::Proof(data(62, 4)),
            Op::Public(data(1, 5)),
            Op::Squeeze(1),
            Op::Public(data(130, 6)),
            Op::Squeeze(32),
            Op::Proof(data(200, 7)),
            Op::Ratchet,
            Op::Proof(data(3, 8)),
            Op::Squeeze(16),
        ]
    }

    fn native(ops: &[Op]) -> (Vec<u8>, u64) {
        let mut d = ShaDuplex::new();
        let mut out = Vec::new();
        for op in ops {
            match op {
                Op::Public(b) | Op::Proof(b) => d.absorb_bytes(b),
                Op::Squeeze(n) => {
                    let mut buf = vec![0u8; *n];
                    d.squeeze_bytes(&mut buf);
                    out.extend(buf);
                }
                Op::Ratchet => d.ratchet_state(),
            }
        }
        (out, d.compressions())
    }

    fn run<CS: Circuit>(cs: &mut CS, public: Public, ops: &[Op]) -> (Vec<u8>, ShaCounts) {
        let mut d = Duplex::new(public);
        let mut out = Vec::new();
        for op in ops {
            match op {
                Op::Public(b) => d.absorb_public(cs, b),
                Op::Proof(b) => {
                    let w = witness_bytes(cs, b);
                    d.absorb(cs, &w);
                }
                Op::Squeeze(n) => out.extend(d.squeeze(cs, *n)),
                Op::Ratchet => d.ratchet(cs),
            }
        }
        (out, d.counts)
    }

    /// [`Duplex::absorb_public`]'s block fast path against the byte loop:
    /// the same challenges, compressions, public stream and recording, from
    /// public and committed chaining values and part-filled blocks.
    #[test]
    fn public_absorbs_match_the_byte_loop() {
        fn drive<CS: Circuit>(cs: &mut CS, public: Public, ops: &[Op], fast: bool) -> (Vec<u8>, ShaCounts) {
            let mut d = Duplex::new(public);
            let mut out = Vec::new();
            for op in ops {
                match op {
                    Op::Public(b) if fast => d.absorb_public(cs, b),
                    Op::Public(b) => d.absorb(cs, &public_bytes(b)),
                    Op::Proof(b) => {
                        let w = witness_bytes(cs, b);
                        d.absorb(cs, &w);
                    }
                    Op::Squeeze(n) => out.extend(d.squeeze(cs, *n)),
                    Op::Ratchet => d.ratchet(cs),
                }
            }
            (out, d.counts)
        }
        fn outcome(ops: &[Op], fast: bool) -> (Vec<u8>, (usize, usize), Vec<u128>, [u8; 32]) {
            let public = PublicStream::discover();
            let mut w = CheckingWitgen::new();
            let (out, counts) = drive(&mut w, public.clone(), ops, fast);
            assert_eq!(w.first_failure, None);
            let stream = public.borrow().clone();
            let mut rec = CompactRecorder::new();
            drive(&mut rec, PublicStream::replay(stream.values.clone(), stream.widths.clone()), ops, fast);
            (out, (counts.circuit, counts.native), stream.values, rec.finish_structured().digest())
        }
        let data = |n: usize, seed: u8| -> Vec<u8> { (0..n).map(|i| (i as u8).wrapping_mul(17).wrapping_add(seed)).collect() };
        let starts: [Vec<Op>; 4] = [
            vec![],
            vec![Op::Proof(data(10, 1))],
            vec![Op::Proof(data(62, 2)), Op::Public(data(7, 3))],
            vec![Op::Public(data(30, 4)), Op::Squeeze(16), Op::Proof(data(3, 5))],
        ];
        for (k, start) in starts.iter().enumerate() {
            for len in [0usize, 1, 61, 62, 63, 123, 124, 125, 186, 187, 300, 1000] {
                let mut ops = start.clone();
                ops.extend([Op::Public(data(len, 9)), Op::Proof(data(5, 6)), Op::Squeeze(16), Op::Public(data(len, 7)), Op::Squeeze(8)]);
                assert_eq!(outcome(&ops, true), outcome(&ops, false), "start {k}, {len} public bytes");
            }
        }
    }

    #[test]
    fn duplex_gadget_matches_native_and_records_one_template() {
        let ops = ops();
        let (expected, compressions) = native(&ops);
        let public = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        let (got, counts) = run(&mut w, public.clone(), &ops);
        assert_eq!(w.first_failure, None);
        assert_eq!(got, expected);
        assert_eq!((counts.circuit + counts.native) as u64, compressions);
        assert!(counts.circuit > 0 && counts.native > 0);

        let stream = public.borrow().clone();
        let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
        let mut rec = CompactRecorder::new();
        let (again, _) = run(&mut rec, replay.clone(), &ops);
        assert_eq!(again, expected);
        assert!(replay.borrow().exhausted());
        let structured = rec.finish_structured();
        assert_eq!(structured.templates.len(), 1);
        assert_eq!(structured.instances.len(), counts.circuit);
        let flat = structured.flatten();
        let (g, h) = w.into_witnesses();
        flat.check_witness(&g, &h).expect("the recording is the witness's");
    }

    #[test]
    fn tampered_proof_bytes_break_the_published_challenges() {
        let ops = ops();
        let public = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        run(&mut w, public.clone(), &ops);
        let stream = public.borrow().clone();
        // The prover changes one proof byte but keeps the honest challenges.
        let mut bad = ops.clone();
        if let Op::Proof(b) = &mut bad[3] {
            b[7] ^= 1;
        }
        let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
        let mut w2 = CheckingWitgen::new();
        run(&mut w2, replay, &bad);
        assert!(w2.first_failure.is_some(), "a forged challenge must not satisfy the circuit");
    }

    #[test]
    fn padded_sha256_and_pow_match_native() {
        for len in [0usize, 40, 55, 56, 64, 128, 256] {
            let message: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let expected: [u8; 32] = sha2::Sha256::digest(&message).into();
            let mut w = CheckingWitgen::new();
            let bytes = witness_bytes(&mut w, &message);
            let mut counts = ShaCounts::default();
            let digest = sha256(&mut w, &bytes, &mut counts);
            let out: Vec<u8> = bytes_of_state(&digest)
                .iter()
                .map(|b| (0..8).fold(0u8, |v, i| v | (u8::from(w.try_eval(&b.bit(i)).unwrap()) << i)))
                .collect();
            assert_eq!(out, expected, "len {len}");
            assert_eq!(counts.circuit, (len + 9).div_ceil(64).max(1) - usize::from(len == 0));
        }
        // PoW: a nonce found natively passes, the next one (almost surely) not.
        let prefix = [7u8; 32];
        let bits = 10;
        let nonce = (0u64..)
            .find(|&n| {
                let d = crate::transcript::sha::sha256_one_block(&prefix, n);
                crate::utils::blake3x4::leading_zero_bits(&d) >= bits
            })
            .unwrap();
        let mut w = CheckingWitgen::new();
        let nb = crate::recursion::bytes::witness_u64(&mut w, nonce);
        let mut counts = ShaCounts::default();
        assert_pow(&mut w, &public_bytes(&prefix), &nb, bits, &mut counts);
        assert_eq!(w.first_failure, None);
        assert_eq!(counts.circuit, 1);
        let mut w = CheckingWitgen::new();
        let nb = crate::recursion::bytes::witness_u64(&mut w, nonce + 1);
        assert_pow(&mut w, &public_bytes(&prefix), &nb, bits, &mut ShaCounts::default());
        assert!(w.first_failure.is_some());
    }
}

/// The first index where two operation logs differ once consecutive absorbs
/// are merged (absorb boundaries are not observable), with both entries.
pub fn first_divergence(a: &[TraceOp], b: &[TraceOp]) -> Option<(usize, Option<TraceOp>, Option<TraceOp>)> {
    fn merged(ops: &[TraceOp]) -> Vec<TraceOp> {
        let mut out: Vec<TraceOp> = Vec::new();
        for op in ops {
            match (out.last_mut(), op) {
                (Some(TraceOp::Absorb(acc)), TraceOp::Absorb(more)) => acc.extend_from_slice(more),
                (Some(TraceOp::Squeeze(acc)), TraceOp::Squeeze(more)) => acc.extend_from_slice(more),
                _ => out.push(op.clone()),
            }
        }
        out.retain(|op| !matches!(op, TraceOp::Absorb(b) if b.is_empty()));
        out
    }
    let (a, b) = (merged(a), merged(b));
    (0..a.len().max(b.len()))
        .find(|&i| a.get(i) != b.get(i))
        .map(|i| (i, a.get(i).cloned(), b.get(i).cloned()))
}
