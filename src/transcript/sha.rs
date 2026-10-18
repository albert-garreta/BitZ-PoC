//! SHA-256 compression duplex: the Fiat–Shamir core of the `sha256-fs`
//! protocol variant, chosen so a verifier circuit replays it with one
//! SHA-256 compression per absorbed 62 bytes and one per squeezed 32 bytes.
//!
//! The state is a chaining value `cv` (eight words, initially the SHA-256
//! IV). Every compressed block is
//!
//! ```text
//! payload[0..62] | len (1 byte) | kind (1 byte)
//! ```
//!
//! with `kind` one of [`ABSORB`] (a full 62-byte block of an absorb phase
//! that continues), [`ABSORB_END`] (the last block of an absorb phase,
//! `len` its payload length, zero-padded), [`SQUEEZE`] (empty payload) and
//! [`RATCHET`] (empty payload). An absorb phase is flushed lazily: a full
//! pending block is compressed as [`ABSORB`] only when another byte arrives,
//! so a phase of `L ≥ 1` bytes costs `⌈L/62⌉` compressions and always ends
//! in one [`ABSORB_END`] block. A squeeze advances `cv ← f(cv, SQUEEZE)` and
//! serves the 32 big-endian bytes of the new `cv`; leftover bytes of one
//! squeeze block serve the next squeeze of the same phase (`squeeze(a);
//! squeeze(b) == squeeze(a + b)`).
//!
//! The block sequence determines the operation sequence (every block names
//! its kind and length, every absorb phase ends in an [`ABSORB_END`]), so
//! two different operation sequences compress different block sequences.
//! Squeeze outputs are the chaining values right after [`SQUEEZE`] blocks;
//! the transcript is public, so revealing them leaks nothing a verifier
//! does not already have.

/// A full block of an absorb phase that continues.
pub const ABSORB: u8 = 0x01;
/// The last block of an absorb phase.
pub const ABSORB_END: u8 = 0x02;
/// One squeeze step.
pub const SQUEEZE: u8 = 0x03;
/// One ratchet step.
pub const RATCHET: u8 = 0x04;
/// Payload bytes per compressed block.
pub const RATE: usize = 62;

/// The SHA-256 initial chaining value.
pub const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// One SHA-256 compression of a 64-byte block into `state`.
#[inline]
#[allow(deprecated)] // sha2 0.10's block type is generic-array 0.14
pub fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    use sha2::digest::generic_array::GenericArray;
    sha2::compress256(state, &[*GenericArray::from_slice(block)]);
}

/// The 32 big-endian bytes of a chaining value (the SHA-256 digest order).
#[inline]
pub fn state_bytes(state: &[u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (chunk, word) in out.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The framed block `payload | len | kind`.
#[inline]
pub fn framed_block(payload: &[u8], kind: u8) -> [u8; 64] {
    assert!(payload.len() <= RATE, "payload exceeds the duplex rate");
    let mut block = [0u8; 64];
    block[..payload.len()].copy_from_slice(payload);
    block[RATE] = payload.len() as u8;
    block[RATE + 1] = kind;
    block
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Absorbing,
    /// Squeezing, with this many unread bytes left in `squeezed`.
    Squeezing(usize),
}

/// One recorded duplex operation (see [`start_trace`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceOp {
    Absorb(Vec<u8>),
    Squeeze(Vec<u8>),
    Ratchet,
}

/// A trace entry: the duplex it came from (in creation order) and the op.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEntry {
    pub duplex: u32,
    pub op: TraceOp,
}

thread_local! {
    static TRACE: std::cell::RefCell<Option<(u32, Vec<TraceEntry>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Starts recording every operation of every duplex created on this thread
/// from now on (diagnostics: the verifier circuit's transcripts are checked
/// against it op by op).
pub fn start_trace() {
    TRACE.with(|t| *t.borrow_mut() = Some((0, Vec::new())));
}

/// Stops recording and returns the trace.
pub fn take_trace() -> Vec<TraceEntry> {
    TRACE.with(|t| t.borrow_mut().take().map(|(_, e)| e).unwrap_or_default())
}

fn trace_id() -> Option<u32> {
    TRACE.with(|t| {
        t.borrow_mut().as_mut().map(|(next, _)| {
            let id = *next;
            *next += 1;
            id
        })
    })
}

fn trace_push(duplex: Option<u32>, op: impl FnOnce() -> TraceOp) {
    if let Some(duplex) = duplex {
        TRACE.with(|t| {
            if let Some((_, entries)) = t.borrow_mut().as_mut() {
                entries.push(TraceEntry { duplex, op: op() });
            }
        });
    }
}

/// The duplex. Cloning forks the transcript.
#[derive(Clone, Debug)]
pub struct ShaDuplex {
    cv: [u32; 8],
    pending: [u8; RATE],
    pending_len: usize,
    squeezed: [u8; 32],
    mode: Mode,
    compressions: u64,
    trace: Option<u32>,
}

impl PartialEq for ShaDuplex {
    fn eq(&self, other: &Self) -> bool {
        self.cv == other.cv
            && self.pending[..self.pending_len] == other.pending[..other.pending_len]
            && self.squeezed == other.squeezed
            && self.mode == other.mode
    }
}

impl Eq for ShaDuplex {}

impl Default for ShaDuplex {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaDuplex {
    pub fn new() -> Self {
        Self {
            cv: IV,
            pending: [0; RATE],
            pending_len: 0,
            squeezed: [0; 32],
            mode: Mode::Absorbing,
            compressions: 0,
            trace: trace_id(),
        }
    }

    fn compress_framed(&mut self, payload_len: usize, kind: u8) {
        let block = framed_block(&self.pending[..payload_len], kind);
        compress(&mut self.cv, &block);
        self.compressions += 1;
    }

    /// Absorbs bytes into the current (or a new) absorb phase.
    pub fn absorb_bytes(&mut self, input: &[u8]) {
        trace_push(self.trace, || TraceOp::Absorb(input.to_vec()));
        if let Mode::Squeezing(_) = self.mode {
            self.mode = Mode::Absorbing;
        }
        for &byte in input {
            if self.pending_len == RATE {
                self.compress_framed(RATE, ABSORB);
                self.pending_len = 0;
            }
            self.pending[self.pending_len] = byte;
            self.pending_len += 1;
        }
    }

    /// Closes an open absorb phase (one [`ABSORB_END`] block, possibly
    /// empty).
    fn close_absorb(&mut self) {
        if self.mode == Mode::Absorbing {
            self.compress_framed(self.pending_len, ABSORB_END);
            self.pending_len = 0;
            self.mode = Mode::Squeezing(0);
        }
    }

    /// Squeezes `output.len()` bytes.
    pub fn squeeze_bytes(&mut self, output: &mut [u8]) {
        self.close_absorb();
        let Mode::Squeezing(mut left) = self.mode else {
            unreachable!("close_absorb leaves the duplex squeezing")
        };
        for byte in output.iter_mut() {
            if left == 0 {
                compress(&mut self.cv, &framed_block(&[], SQUEEZE));
                self.compressions += 1;
                self.squeezed = state_bytes(&self.cv);
                left = 32;
            }
            *byte = self.squeezed[32 - left];
            left -= 1;
        }
        self.mode = Mode::Squeezing(left);
        trace_push(self.trace, || TraceOp::Squeeze(output.to_vec()));
    }

    /// One-way ratchet: closes the phase and compresses one [`RATCHET`]
    /// block; the next operation starts a fresh phase.
    pub fn ratchet_state(&mut self) {
        trace_push(self.trace, || TraceOp::Ratchet);
        self.close_absorb();
        compress(&mut self.cv, &framed_block(&[], RATCHET));
        self.compressions += 1;
        self.mode = Mode::Absorbing;
    }

    /// The chaining value (public: it is a function of the transcript).
    pub fn chaining_value(&self) -> [u32; 8] {
        self.cv
    }

    /// A digest pinning the whole state, for transcript pins and tests:
    /// the chaining value, the pending bytes and the mode.
    pub fn state_digest(&self) -> [u8; 32] {
        let mut probe = self.clone();
        probe.trace = None;
        let mut out = [0u8; 32];
        probe.squeeze_bytes(&mut out);
        out
    }

    /// SHA-256 compressions performed so far (the in-circuit cost of this
    /// transcript, which the verifier circuit's accounting checks against).
    pub fn compressions(&self) -> u64 {
        self.compressions
    }
}

impl spongefish::DuplexSpongeInterface for ShaDuplex {
    type U = u8;

    fn absorb(&mut self, input: &[u8]) -> &mut Self {
        self.absorb_bytes(input);
        self
    }

    fn squeeze(&mut self, output: &mut [u8]) -> &mut Self {
        self.squeeze_bytes(output);
        self
    }

    fn ratchet(&mut self) -> &mut Self {
        self.ratchet_state();
        self
    }
}

/// The Spartan-level transcript over [`ShaDuplex`]: absorption goes through
/// the trait's framing ([`super::traits::Transcript::absorb_slice`]); a
/// challenge is a squeeze, which already advances the state, so nothing is
/// re-absorbed after it.
#[derive(Clone, Debug, Default)]
pub struct Sha256Transcript {
    duplex: ShaDuplex,
}

impl Sha256Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    /// See [`ShaDuplex::state_digest`].
    pub fn state_digest(&self) -> [u8; 32] {
        self.duplex.state_digest()
    }

    pub fn duplex(&self) -> &ShaDuplex {
        &self.duplex
    }
}

impl super::traits::Transcript for Sha256Transcript {
    fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
        self.duplex.squeeze_bytes(output);
    }

    fn get_challenge<T: super::traits::ConstTranscribable>(&mut self) -> T {
        let mut buf = vec![0u8; T::NUM_BYTES];
        self.duplex.squeeze_bytes(&mut buf);
        // Canonical codecs reject high padding bits; masking them is unbiased
        // for fixed-width binary and integer challenges.
        assert!(T::NUM_BITS <= T::NUM_BYTES * 8);
        for (i, byte) in buf.iter_mut().enumerate() {
            let remaining = T::NUM_BITS.saturating_sub(i * 8).min(8);
            *byte &= ((1u16 << remaining) - 1) as u8;
        }
        T::read_transcription_bytes_exact(&buf)
    }

    fn absorb_inner(&mut self, v: &[u8]) {
        self.duplex.absorb_bytes(v);
    }
}

/// `SHA-256(prefix ‖ nonce_le)` for a prefix of at most 55 − 8 bytes: one
/// standard padded block, one compression.
pub fn sha256_one_block(prefix: &[u8], nonce: u64) -> [u8; 32] {
    let len = prefix.len() + 8;
    assert!(len <= 55, "proof-of-work message exceeds one SHA-256 block");
    let mut block = [0u8; 64];
    block[..prefix.len()].copy_from_slice(prefix);
    block[prefix.len()..len].copy_from_slice(&nonce.to_le_bytes());
    block[len] = 0x80;
    block[56..].copy_from_slice(&((len as u64) * 8).to_be_bytes());
    let mut state = IV;
    compress(&mut state, &block);
    state_bytes(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn one_block_matches_sha256() {
        for len in [0usize, 1, 16, 32, 47] {
            let prefix: Vec<u8> = (0..len as u8).collect();
            let mut message = prefix.clone();
            message.extend_from_slice(&0x0123_4567_89ab_cdefu64.to_le_bytes());
            let expected: [u8; 32] = sha2::Sha256::digest(&message).into();
            assert_eq!(sha256_one_block(&prefix, 0x0123_4567_89ab_cdef), expected);
        }
    }

    #[test]
    fn squeezes_stream_and_phases_are_framed() {
        let mut a = ShaDuplex::new();
        let mut b = ShaDuplex::new();
        a.absorb_bytes(b"abc");
        b.absorb_bytes(b"abc");
        let mut whole = [0u8; 40];
        a.squeeze_bytes(&mut whole);
        let mut first = [0u8; 7];
        let mut rest = [0u8; 33];
        b.squeeze_bytes(&mut first);
        b.squeeze_bytes(&mut rest);
        assert_eq!(&whole[..7], &first);
        assert_eq!(&whole[7..], &rest);
        assert_eq!(a, b);

        // Splitting one absorb phase does not change it; a squeeze between
        // two absorbs does.
        let mut c = ShaDuplex::new();
        c.absorb_bytes(b"ab");
        c.absorb_bytes(b"c");
        let mut d = ShaDuplex::new();
        d.absorb_bytes(b"abc");
        assert_eq!(c.state_digest(), d.state_digest());
        let mut e = ShaDuplex::new();
        e.absorb_bytes(b"ab");
        e.squeeze_bytes(&mut [0u8; 1]);
        e.absorb_bytes(b"c");
        assert_ne!(e.state_digest(), d.state_digest());
    }

    #[test]
    fn absorb_costs_one_compression_per_rate_bytes() {
        for len in [1usize, 61, 62, 63, 124, 125, 200] {
            let mut duplex = ShaDuplex::new();
            duplex.absorb_bytes(&vec![7u8; len]);
            duplex.squeeze_bytes(&mut [0u8; 32]);
            assert_eq!(duplex.compressions() as usize, len.div_ceil(RATE) + 1, "len {len}");
        }
        // A 62-byte phase is one ABSORB_END block, distinct from the same
        // bytes followed by more data.
        let mut short = ShaDuplex::new();
        short.absorb_bytes(&[9u8; 62]);
        let mut long = ShaDuplex::new();
        long.absorb_bytes(&[9u8; 63]);
        assert_ne!(short.state_digest(), long.state_digest());
    }

    #[test]
    fn empty_absorb_phase_still_closes() {
        let mut a = ShaDuplex::new();
        let mut out = [0u8; 16];
        a.squeeze_bytes(&mut out);
        assert_eq!(a.compressions(), 2);
        let mut b = ShaDuplex::new();
        b.absorb_bytes(&[]);
        let mut out_b = [0u8; 16];
        b.squeeze_bytes(&mut out_b);
        assert_eq!(out, out_b);
    }
}
