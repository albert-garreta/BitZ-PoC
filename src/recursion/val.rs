//! Values of the public-challenge verifier circuit.
//!
//! Every Fiat–Shamir challenge of the verifier is PUBLIC: the outer statement
//! carries it, the in-circuit transcript recomputes it from the committed
//! proof and asserts equality. A product of a public and a committed
//! GF(2^128) element is then an F2-linear map with public coefficients — no
//! committed bits, no rank-1 row.
//!
//! Field work is phrased as linear combinations [`Lc`] built in one pass. A
//! combination that later code reads again is SETTLED: its value is
//! committed as fresh bits and the combination asserted equal to them, so
//! downstream maps read single committed bits instead of wide F2
//! combinations.
//!
//! Adapted from the flock-recursion line's `flockrec::val` (same design,
//! BitZ's `field::Gf128`, no GF(2^256)).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

use circuit::gf128::{self, Gf as Bits};
use circuit::{BoolRepresentation, BoolWitness, Circuit, HintResult, PackedBits};
use field::Gf128 as Gf;

pub fn u128_of(f: Gf) -> u128 {
    u128::from(f.lo) | (u128::from(f.hi) << 64)
}

pub fn gf_of(v: u128) -> Gf {
    Gf::new(v as u64, (v >> 64) as u64)
}

/// Public values handed from a discovering pass to a replaying one running
/// on another thread (see [`PublicStream::discover_into`] and
/// [`PublicStream::follow`]).
#[derive(Debug, Default)]
pub struct Feed {
    state: Mutex<FeedState>,
    ready: Condvar,
}

#[derive(Debug, Default)]
struct FeedState {
    values: Vec<u128>,
    widths: Vec<u8>,
    /// Values of the public computation the follower may take instead of
    /// redoing it ([`PublicStream::share`]).
    shared: Vec<u128>,
    closed: bool,
}

impl Feed {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn push(&self, value: u128, width: u8) {
        let mut state = self.state.lock().expect("feed lock");
        state.values.push(value);
        state.widths.push(width);
        self.ready.notify_all();
    }

    fn push_shared(&self, value: u128) {
        let mut state = self.state.lock().expect("feed lock");
        state.shared.push(value);
        self.ready.notify_all();
    }

    fn wait_shared(&self, index: usize) -> u128 {
        let mut state = self.state.lock().expect("feed lock");
        while state.shared.len() <= index {
            assert!(!state.closed, "the shared values ended before their follower");
            state = self.ready.wait(state).expect("feed lock");
        }
        state.shared[index]
    }

    /// No more values: a follower waiting past the end panics.
    pub fn close(&self) {
        self.state.lock().expect("feed lock").closed = true;
        self.ready.notify_all();
    }

    /// The values from `from` on, waiting until there is at least one.
    fn wait_from(&self, from: usize) -> (Vec<u128>, Vec<u8>) {
        let mut state = self.state.lock().expect("feed lock");
        while state.values.len() <= from {
            assert!(!state.closed, "the public stream ended before its follower");
            state = self.ready.wait(state).expect("feed lock");
        }
        (state.values[from..].to_vec(), state.widths[from..].to_vec())
    }
}

/// The public values in circuit order and their constrained widths.
#[derive(Clone, Debug, Default)]
pub struct PublicStream {
    pub values: Vec<u128>,
    /// Constrained width of each value in bits.
    pub widths: Vec<u8>,
    /// Phase label of each value (accounting only).
    pub labels: Vec<&'static str>,
    label: &'static str,
    pos: usize,
    discover: bool,
    /// A check between public values failed (the statement is false).
    pub failed: Option<String>,
    /// Discovering: also hands each value on; replaying: waits here for
    /// values not yet received.
    feed: Option<Arc<Feed>>,
    /// Shared values taken from the feed so far.
    shared_pos: usize,
}

pub type Public = Rc<RefCell<PublicStream>>;

impl PublicStream {
    /// Records the values as the (witness-computing) circuit produces them.
    pub fn discover() -> Public {
        Rc::new(RefCell::new(Self {
            discover: true,
            label: "(start)",
            ..Self::default()
        }))
    }

    /// [`Self::discover`], each value also pushed to `feed`.
    pub fn discover_into(feed: Arc<Feed>) -> Public {
        let public = Self::discover();
        public.borrow_mut().feed = Some(feed);
        public
    }

    /// Replays the values a discovering pass pushes to `feed`, as they come.
    pub fn follow(feed: Arc<Feed>) -> Public {
        let public = Self::replay(Vec::new(), Vec::new());
        public.borrow_mut().feed = Some(feed);
        public
    }

    /// Replays a recorded stream (the outer verifier's view).
    pub fn replay(values: Vec<u128>, widths: Vec<u8>) -> Public {
        Rc::new(RefCell::new(Self {
            values,
            widths,
            label: "(start)",
            ..Self::default()
        }))
    }

    pub fn set_label(&mut self, label: &'static str) {
        self.label = label;
    }

    pub fn is_discovering(&self) -> bool {
        self.discover
    }

    /// The next public value: computed by `found` when discovering, read from
    /// the stream when replaying.
    pub fn take(&mut self, width: u8, found: impl FnOnce() -> u128) -> u128 {
        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        if self.discover {
            let v = found() & mask;
            self.values.push(v);
            self.widths.push(width);
            self.labels.push(self.label);
            if let Some(feed) = &self.feed {
                feed.push(v, width);
            }
            v
        } else {
            if self.pos == self.values.len()
                && let Some(feed) = &self.feed
            {
                let (values, widths) = feed.wait_from(self.pos);
                self.values.extend(values);
                self.widths.extend(widths);
            }
            let v = self.values[self.pos];
            assert_eq!(self.widths[self.pos], width, "public stream shape");
            assert_eq!(v & !mask, 0, "public value exceeds its width");
            self.pos += 1;
            v
        }
    }

    /// A value of the circuit's public computation (a function of the
    /// statement and the stream so far): computed, and handed to a follower
    /// through the feed, which takes it instead of computing it again. Not
    /// part of the stream: a verifier without a feed computes it.
    pub fn share(&mut self, compute: impl FnOnce() -> u128) -> u128 {
        match (&self.feed, self.discover) {
            (Some(feed), true) => {
                let v = compute();
                feed.push_shared(v);
                v
            }
            (Some(feed), false) => {
                let v = feed.wait_shared(self.shared_pos);
                self.shared_pos += 1;
                v
            }
            (None, _) => compute(),
        }
    }

    /// Marks the statement false (a check between public values failed).
    pub fn fail(&mut self, why: impl Into<String>) {
        if self.failed.is_none() {
            self.failed = Some(why.into());
        }
    }

    /// Bytes of the stream as the outer statement carries it.
    pub fn bytes(&self) -> usize {
        self.widths.iter().map(|&w| (w as usize).div_ceil(8)).sum()
    }

    /// Bytes per label, in first-seen order.
    pub fn bytes_by_label(&self) -> Vec<(&'static str, usize, usize)> {
        let mut out: Vec<(&'static str, usize, usize)> = Vec::new();
        for (label, &w) in self.labels.iter().zip(&self.widths) {
            let b = (w as usize).div_ceil(8);
            match out.iter_mut().find(|(l, _, _)| l == label) {
                Some(e) => {
                    e.1 += 1;
                    e.2 += b;
                }
                None => out.push((label, 1, b)),
            }
        }
        out
    }

    pub fn exhausted(&self) -> bool {
        self.discover || self.pos == self.values.len()
    }
}

/// A GF(2^128) value: public, or 128 F2 combinations of committed bits.
#[derive(Clone)]
pub enum G<B: BoolWitness> {
    P(Gf),
    V(Bits<B>),
}

impl<B: BoolWitness> G<B> {
    pub fn public(&self) -> Option<Gf> {
        match self {
            Self::P(v) => Some(*v),
            Self::V(_) => None,
        }
    }

    /// The bits (constants for a public value).
    pub fn bits(&self) -> Bits<B> {
        match self {
            Self::P(v) => gf128::constant(u128_of(*v)),
            Self::V(b) => b.clone(),
        }
    }
}

pub(crate) fn eval_bits<B: BoolWitness, Z, C>(
    ctx: &dyn circuit::WitnessContext<Z, B, C>,
    a: &Bits<B>,
) -> Gf {
    gf_of(
        a.iter()
            .enumerate()
            .fold(0u128, |v, (i, b)| v | (u128::from(ctx.eval_bool(b)) << i)),
    )
}

/// Commits `value` as fresh bits (proof data).
pub fn witness<CS: Circuit>(cs: &mut CS, value: Gf) -> G<CS::Bool> {
    G::V(gf128::witness(cs, u128_of(value)))
}

/// The value of committed bits when the backend computes values.
pub fn try_value<CS: Circuit>(cs: &CS, bits: &[CS::Bool]) -> Option<u128> {
    let mut v = 0u128;
    for (i, b) in bits.iter().enumerate() {
        v |= u128::from(cs.try_eval(b)?) << i;
    }
    Some(v)
}

/// `constant + Σ c_k·y_k` over GF(2^128) with public coefficients.
#[derive(Clone)]
pub struct Lc<B: BoolWitness> {
    constant: Gf,
    terms: Vec<(Gf, Bits<B>)>,
}

impl<B: BoolWitness> Default for Lc<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: BoolWitness> Lc<B> {
    pub fn new() -> Self {
        Self {
            constant: Gf::ZERO,
            terms: Vec::new(),
        }
    }

    pub fn constant(c: Gf) -> Self {
        Self {
            constant: c,
            terms: Vec::new(),
        }
    }

    pub fn add_const(&mut self, c: Gf) -> &mut Self {
        self.constant += c;
        self
    }

    pub fn add(&mut self, c: Gf, g: &G<B>) -> &mut Self {
        match g {
            G::P(v) => self.constant += c * *v,
            G::V(bits) => {
                if c != Gf::ZERO {
                    self.terms.push((c, bits.clone()));
                }
            }
        }
        self
    }

    pub fn add_bits(&mut self, c: Gf, bits: &Bits<B>) -> &mut Self {
        if c != Gf::ZERO {
            self.terms.push((c, bits.clone()));
        }
        self
    }

    pub fn is_public(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    pub fn of(g: &G<B>) -> Self {
        let mut lc = Self::new();
        lc.add(Gf::ONE, g);
        lc
    }

    /// `c · self`.
    pub fn scaled(&self, c: Gf) -> Self {
        if c == Gf::ZERO {
            return Self::new();
        }
        Self {
            constant: self.constant * c,
            terms: self.terms.iter().map(|(k, y)| (*k * c, y.clone())).collect(),
        }
    }

    pub fn add_lc(&mut self, other: &Self) -> &mut Self {
        self.constant += other.constant;
        self.terms.extend(other.terms.iter().cloned());
        self
    }

    pub fn add_scaled(&mut self, c: Gf, other: &Self) -> &mut Self {
        if c != Gf::ZERO {
            self.constant += c * other.constant;
            self.terms
                .extend(other.terms.iter().map(|(k, y)| (*k * c, y.clone())));
        }
        self
    }

    /// The combination's value when the backend computes values.
    pub fn try_value<CS: Circuit<Bool = B>>(&self, cs: &CS) -> Option<Gf> {
        let mut acc = self.constant;
        for (c, y) in &self.terms {
            acc += *c * gf_of(try_value(cs, y)?);
        }
        Some(acc)
    }

    /// The constant when the combination is public.
    pub fn public_value(&self) -> Option<Gf> {
        self.terms.is_empty().then_some(self.constant)
    }

    /// The combination's 128 output bits, each one XOR over its support
    /// (formed by the backend's dense-map hook, which a recorder may keep
    /// unexpanded).
    pub fn expand<CS: Circuit<Bool = B>>(&self, cs: &mut CS) -> Bits<B> {
        // A value backend: the field sum itself (the same bits as the map).
        if let Some(v) = self.try_value(cs) {
            let v = u128_of(v);
            return std::array::from_fn(|i| B::from((v >> i) & 1 == 1));
        }
        let maps: Vec<(&Bits<B>, [u128; 128])> = self
            .terms
            .iter()
            .map(|(c, y)| (y, gf128::mul_const_columns(u128_of(*c))))
            .collect();
        cs.linear_maps_128(&maps, u128_of(self.constant))
    }

    /// The value as a fresh committed element, the combination asserted equal
    /// to it (128 committed bits, 128 cells, 2 rows). Public combinations
    /// stay public.
    pub fn settle<CS: Circuit<Bool = B>>(&self, cs: &mut CS) -> G<B> {
        if self.terms.is_empty() {
            return G::P(self.constant);
        }
        let (constant, terms) = (self.constant, self.terms.clone());
        let packed = cs.hint::<1, 128, 2, _>(move |ctx| {
            let v = terms
                .iter()
                .fold(constant, |acc, (c, y)| acc + *c * eval_bits(ctx, y));
            let v = u128_of(v);
            HintResult::Ok(PackedBits::from_words([v as u64, (v >> 64) as u64]))
        });
        let fresh: Bits<B> = std::array::from_fn(|i| packed.bit(i));
        let mut check = self.clone();
        check.add_bits(Gf::ONE, &fresh);
        let bits = check.expand(cs);
        gf128::assert_zero(cs, &bits);
        G::V(fresh)
    }

    /// Asserts the combination is zero. A public combination is checked
    /// natively; a failure marks `public` (the statement is false).
    pub fn assert_zero<CS: Circuit<Bool = B>>(&self, cs: &mut CS, public: &Public, what: &str) {
        if self.terms.is_empty() {
            if self.constant != Gf::ZERO {
                public.borrow_mut().fail(format!("public check failed: {what}"));
            }
            return;
        }
        let bits = self.expand(cs);
        gf128::assert_zero(cs, &bits);
    }
}

/// `a == b` for GF(2^128) values.
pub fn assert_eq<CS: Circuit>(
    cs: &mut CS,
    a: &G<CS::Bool>,
    b: &G<CS::Bool>,
    public: &Public,
    what: &str,
) {
    let mut lc = Lc::new();
    lc.add(Gf::ONE, a).add(Gf::ONE, b);
    lc.assert_zero(cs, public, what);
}

/// Publishes the low `width` bits of `bits` (as one public value) and asserts
/// the in-circuit bits equal it. Returns the value.
pub fn publish<CS: Circuit>(
    cs: &mut CS,
    public: &Public,
    bits: &[CS::Bool],
    width: u8,
) -> u128 {
    let w = width as usize;
    assert!(w <= bits.len() && w <= 128);
    let value = {
        let found = || try_value(cs, &bits[..w]).expect("discovering public values needs a witness backend");
        public.borrow_mut().take(width, found)
    };
    let diff: Vec<CS::Bool> = bits[..w]
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if (value >> i) & 1 == 1 {
                cs.xor(b.clone(), CS::Bool::from(true))
            } else {
                b.clone()
            }
        })
        .collect();
    gf128::assert_zero(cs, &diff);
    value
}
