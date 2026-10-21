//! [`crate::piop::spartan::falcon512_algebraic::PreparedFalconAlgebraic::verify`]
//! in the circuit: a batch of algebraic Falcon-512 proofs (keyed ring, small
//! prime, unsplit bridge, word binder; the `sha256-fs` transcript) as the
//! base of the recursion chain.
//!
//! Public, native through the proxy and the protocol's hooks
//! ([`hooks`]): the statement binding (keys, targets, exceptions, root),
//! the prime and every challenge, the keyed ring's instance polynomials and
//! weights, the binder's terminal weights, the bridge's integer folds
//! (proof data made public: they enter the forest's root claim as
//! exponents), the bridge's row images and binary claim, the joint
//! sumcheck's tensor coefficients, and the opening's padding and residual
//! bases. In the circuit: the keyed ring's quotient (absorbed, range
//! checked, its term of the ring target), the norm and binder sumchecks
//! over committed round polynomials with the norm terminal's one modular
//! square, the forest's committed messages, the joint binary sumcheck, the
//! ring switch and the Ligerito opening ([`super::falcon_opening`]), and
//! every grinding nonce.

use circuit::Circuit;
use field::Gf128 as Gf;

use crate::hybrid::integer_bridge::recursion as bridge;
use crate::hybrid::joint_sumcheck::Coefficients;
use crate::hybrid::joint_sumcheck::recursion as joint;
use crate::hybrid::opening::Geometry;
use crate::piop::spartan::SpartanField;
use crate::piop::spartan::falcon512_algebraic::recursion_hooks as hooks;
use crate::piop::spartan::falcon512_algebraic::{FalconAlgebraicProof, FalconAlgebraicStatement, PreparedFalconAlgebraic};
use crate::transcript::traits::{ConstTranscribable, Transcript};

use super::arith::{Row, Zq, hint_bits, mul, mul_mod_q, mulmod};
use super::bytes::{Byte, bytes_of_bits, gf_bytes, witness_u64};
use super::sha::ShaCounts;
use super::spartan::{ModLc, SpartanTranscript, canonical, modulus, sumcheck_verify_private, witness_residue};
use super::val::{G, Lc, Public, witness};

type F = crate::piop::spartan::SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

/// Costs of one in-circuit verification.
#[derive(Clone, Copy, Debug, Default)]
pub struct FalconCounts {
    /// The Falcon transcript's compressions and its proof-of-work checks.
    pub transcript: ShaCounts,
    pub pow: ShaCounts,
    pub opening: super::falcon_opening::OpeningCounts,
}

/// Per-stage wall times of one pass and its in-circuit SHA-256 compressions
/// (transcript, proof of work, Merkle), printed with `BITZ_REC_STAGES=1`.
pub(super) struct Stages {
    pass: &'static str,
    start: std::time::Instant,
    last: std::time::Instant,
    marks: Vec<(String, f64)>,
    /// Cumulative compressions at each mark, counted from `base`.
    sha: Vec<[usize; 3]>,
    base: [usize; 3],
    /// Per opening level: the Merkle compressions split into leaves and nodes.
    merkle: Vec<String>,
    on: bool,
}

impl Stages {
    pub(super) fn new(pass: &'static str) -> Self {
        let now = std::time::Instant::now();
        Self {
            pass,
            start: now,
            last: now,
            marks: Vec::new(),
            sha: Vec::new(),
            base: [0; 3],
            merkle: Vec::new(),
            on: std::env::var_os("BITZ_REC_STAGES").is_some(),
        }
    }

    /// Compressions counted from `base` (the counters when the pass starts).
    pub(super) fn with_base(mut self, base: [usize; 3]) -> Self {
        self.base = base;
        self
    }

    /// A mark with the cumulative in-circuit compressions: transcript, proof
    /// of work, Merkle.
    pub(super) fn mark(&mut self, stage: impl Into<String>, sha: [usize; 3]) {
        if self.on {
            let now = std::time::Instant::now();
            self.marks.push((stage.into(), (now - self.last).as_secs_f64()));
            self.sha.push(sha);
            self.last = now;
        }
    }

    /// One opening level's Merkle compressions: `total`, of which `leaf` hash
    /// the `rows` leaves of `width` elements and the rest the tree's nodes.
    pub(super) fn merkle(&mut self, level: impl std::fmt::Display, rows: usize, width: usize, leaf: usize, total: usize) {
        if self.on {
            self.merkle.push(format!("{level} {rows} rows x {width}: leaves {leaf} nodes {}", total - leaf));
        }
    }

    pub(super) fn print(&self) {
        if self.on {
            let mut previous = self.base;
            let marks: Vec<String> = self
                .marks
                .iter()
                .zip(&self.sha)
                .map(|((s, t), sha)| {
                    let delta: [usize; 3] = std::array::from_fn(|k| sha[k] - previous[k]);
                    previous = *sha;
                    format!("{s} {t:.3} sha {}/{}/{}", delta[0], delta[1], delta[2])
                })
                .collect();
            eprintln!("[falcon {}] {} | total {:.3}", self.pass, marks.join(" | "), self.start.elapsed().as_secs_f64());
            if !self.merkle.is_empty() {
                eprintln!("[falcon {} merkle] {}", self.pass, self.merkle.join(" | "));
            }
        }
    }
}

/// `VerifierBlockGrindingTranscript` in the circuit: the native transcript
/// over public data (through the proxy) with one grinding boundary before
/// the first challenge after any absorb, and the committed absorbs of proof
/// data, which end a block like any absorb.
pub struct BlockTranscript<'a, 'n, CS: Circuit> {
    pub cs: &'a mut CS,
    pub t: &'a mut SpartanTranscript<CS::Bool>,
    domain: &'static [u8],
    bits: u32,
    active: bool,
    blocks: usize,
    nonces: Option<&'n [u64]>,
    consumed: usize,
}

impl<'a, 'n, CS: Circuit> BlockTranscript<'a, 'n, CS> {
    /// `nonces` is the proof's vector (`None` while recording).
    pub fn new(
        cs: &'a mut CS,
        t: &'a mut SpartanTranscript<CS::Bool>,
        domain: &'static [u8],
        bits: u32,
        nonces: Option<&'n [u64]>,
    ) -> Self {
        Self { cs, t, domain, bits, active: false, blocks: 0, nonces, consumed: 0 }
    }

    fn begin_block(&mut self) {
        if self.active {
            return;
        }
        if self.bits != 0 {
            let nonce = self.nonces.and_then(|n| n.get(self.consumed).copied()).unwrap_or(0);
            self.consumed += 1;
            self.t.grinding_boundary_private(self.cs, self.domain, self.blocks as u64, self.bits, nonce);
        }
        self.blocks += 1;
        self.active = true;
    }

    /// An `absorb_slice` of (partly) committed bytes.
    pub fn absorb_committed(&mut self, bytes: &[Byte<CS::Bool>]) {
        self.active = false;
        self.t.absorb_slice(self.cs, bytes);
    }

    /// Every nonce consumed (a public shape check).
    pub fn finish(self, what: &str) {
        if let Some(nonces) = self.nonces
            && nonces.len() != self.consumed
        {
            self.t.public().borrow_mut().fail(format!("{what}: grinding nonce count"));
        }
    }
}

impl<CS: Circuit> Transcript for BlockTranscript<'_, '_, CS> {
    fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
        self.begin_block();
        self.t.proxy(self.cs).get_challenge()
    }

    fn begin_sampling(&mut self) {
        self.begin_block();
    }

    fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
        self.t.proxy(self.cs).fill_sampling_bytes(output);
    }

    fn absorb_inner(&mut self, v: &[u8]) {
        self.active = false;
        self.t.proxy(self.cs).absorb_inner(v);
    }
}

/// A committed field element absorbed as `joint_sumcheck::observe` does:
/// its low word, then its high word, one `absorb_slice` each.
fn observe_gf<CS: Circuit>(cs: &mut CS, t: &mut SpartanTranscript<CS::Bool>, g: &G<CS::Bool>) {
    let bytes = gf_bytes(g);
    t.absorb_slice(cs, &bytes[..8]);
    t.absorb_slice(cs, &bytes[8..]);
}

/// Asserts `|v| ≤ bound` for the two's-complement value `v` of 64 committed
/// bits (`bound < 2^w`, `w < 63`): bits `w..63` repeat the sign, then
/// `bound + v` and `bound − v` each fit `w + 1` committed bits (two rows).
fn signed_range<CS: Circuit>(cs: &mut CS, x: &[CS::Bool; 64], bound: u128) {
    let w = 128 - bound.leading_zeros() as usize;
    assert!(w < 63, "the bound leaves a sign extension");
    let extension: Vec<CS::Bool> = (w..63).map(|b| cs.xor(x[b].clone(), x[63].clone())).collect();
    circuit::gf128::assert_zero(cs, &extension);
    // v = Σ_{b<w} 2^b x_b − 2^w x_63.
    for negate in [false, true] {
        let inputs: Vec<CS::Bool> = x.to_vec();
        let d = hint_bits(cs, inputs, w + 1, move |bits| {
            let low = bits[..w].iter().enumerate().fold(0i128, |a, (i, &b)| a | (i128::from(b) << i));
            let v = low - (i128::from(bits[63]) << w);
            let d = if negate { bound as i128 - v } else { bound as i128 + v };
            let d = d.max(0) as u128;
            (0..w + 1).map(|i| (d >> i) & 1 == 1).collect()
        });
        // ±v + bound − Σ 2^i d_i = 0.
        let mut row = Row::<CS>::new();
        for (b, bit) in x[..w].iter().enumerate() {
            row.bit_pow2(cs, bit, b, negate);
        }
        row.bit_pow2(cs, &x[63], w, !negate);
        row.constant(bound, false);
        for (i, bit) in d.iter().enumerate() {
            row.bit_pow2(cs, bit, i, true);
        }
        row.assert_zero(cs);
    }
}

/// [`crate::bitz::gkr::gpgkr_verify`] over the bridge's block-grinding
/// transcript (`integer_bridge::ForestVerifier`): each message pair is
/// committed and absorbed as one 32-byte slice, each challenge drawn as a
/// field challenge.
fn bridge_forest<CS: Circuit>(
    bt: &mut BlockTranscript<'_, '_, CS>,
    messages: Option<&[[Gf; 2]]>,
    claim: G<CS::Bool>,
    point: &[Gf],
    rounds: usize,
) -> (Vec<Gf>, G<CS::Bool>) {
    use std::collections::VecDeque;
    let public = bt.t.public().clone();
    let mut cursor = 0usize;
    let mut read_pair = |bt: &mut BlockTranscript<'_, '_, CS>| {
        let pair = messages.map_or([Gf::ZERO; 2], |m| m[cursor]);
        cursor += 1;
        let a = witness(bt.cs, pair[0]);
        let b = witness(bt.cs, pair[1]);
        let mut bytes = gf_bytes(&a);
        bytes.extend(gf_bytes(&b));
        bt.absorb_committed(&bytes);
        (a, b)
    };
    let mut point: VecDeque<Gf> = point.iter().rev().copied().collect();
    let mut claim = claim;
    for _ in 0..rounds {
        let mut prefix = Gf::ONE;
        let mut next_point: VecDeque<Gf> = VecDeque::new();
        let mut lc = Lc::of(&claim);
        for &z in &point {
            let (se, si) = read_pair(bt);
            let (sum0, sum1) = if z == Gf::ZERO {
                (lc.clone(), Lc::of(&se))
            } else {
                let zinv = z.inverse_or_zero();
                let mut s1 = lc.scaled(zinv);
                s1.add((Gf::ONE + z) * zinv, &se);
                (Lc::of(&se), s1)
            };
            let r: Gf = bt.get_field_challenge(&());
            next_point.push_back(r);
            let factor = crate::bitz::eq_factor(r, z);
            // claim = factor·((1 + r)·sum0 + r·sum1 + r(r + 1)·si).
            let mut next = sum0.scaled(factor * (Gf::ONE + r));
            next.add_scaled(factor * r, &sum1);
            next.add(factor * r * (r + Gf::ONE), &si);
            lc = next;
            prefix *= factor;
        }
        let (l, rr) = read_pair(bt);
        let product = mul(bt.cs, &l, &rr);
        let mut check = lc;
        check.add(prefix, &product);
        check.assert_zero(bt.cs, &public, "bridge forest layer product");
        let r2: Gf = bt.get_field_challenge(&());
        next_point.push_front(r2);
        let mut next = Lc::of(&l);
        next.add(r2, &l).add(r2, &rr);
        claim = next.settle(bt.cs);
        point = next_point;
    }
    if let Some(m) = messages
        && m.len() != cursor
    {
        public.borrow_mut().fail("bridge forest trailing messages");
    }
    let mut point = Vec::from(point);
    point.reverse();
    (point, claim)
}

fn residues<const C: usize>(rounds: &[[F; C]], cfg: &Cfg) -> Vec<[u128; C]> {
    rounds.iter().map(|r| std::array::from_fn(|i| canonical(&r[i], cfg))).collect()
}

fn field_of(values: &[u128], cfg: &Cfg) -> Vec<F> {
    values.iter().map(|&c| F::from_with_cfg(c, cfg)).collect()
}

/// The circuit of `PreparedFalconAlgebraic::verify` for one proof (`None`
/// while recording) of `statement`, whose commitment `root` and exception
/// lists are public inputs next to it.
#[allow(clippy::too_many_arguments)]
pub fn verify_falcon_algebraic<CS: Circuit>(
    cs: &mut CS,
    prepared: &PreparedFalconAlgebraic,
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    proof: Option<&FalconAlgebraicProof>,
    public: Public,
    log: Option<&mut Vec<crate::transcript::sha::TraceOp>>,
) -> FalconCounts {
    hooks::check_configuration(prepared).expect("the circuit's configuration");
    let view = proof.map(|p| hooks::view(p).expect("a keyed-ring proof"));
    let v = view.as_ref();
    let d = hooks::difficulties(prepared);
    let mut t = SpartanTranscript::new(public.clone());
    if log.is_some() {
        t.duplex = t.duplex.clone().with_log();
    }
    let fail = |why: &str| public.borrow_mut().fail(why.to_string());
    let mut stages = Stages::new(if proof.is_some() { "witness" } else { "record" });

    // The statement and the prime.
    public.borrow_mut().set_label("statement");
    let digest = hooks::bind_statement(prepared, statement, root, exceptions, &mut t.proxy(cs)).expect("public statement");
    // Round 0 (`ext:` profiles), right after the statement: ζ public, y committed.
    let round0 = hooks::round0(prepared).map(|params| {
        assert!(params.extension, "the circuit runs Round 0 over GF(2^256)");
        let round = v.and_then(|v| v.round0);
        if v.is_some() && round.is_none_or(|r| r.y_ext.is_none() || r.nonce.is_some() != (params.grinding_bits > 0)) {
            fail("Round 0 shape");
        }
        let y = flock_core::field::Gf256::new(round.map_or(Gf::ZERO, |r| r.y), round.and_then(|r| r.y_ext).unwrap_or(Gf::ZERO));
        let nonce = (params.grinding_bits > 0).then(|| round.and_then(|r| r.nonce).unwrap_or(0));
        let packed_log = match hooks::geometry(prepared) {
            hooks::GeometryRef::Single(g) => g.packed_log(),
            hooks::GeometryRef::Split(g) => g.packed_log(),
        };
        super::spartan::ood_round_ext(cs, &mut t, packed_log, params.grinding_bits, nonce, y)
    });
    let field = hooks::sample_field(&mut t.proxy(cs)).expect("public prime");
    let cfg = &field;
    let q = modulus(cfg);
    stages.mark("statement", [t.duplex.counts.circuit, t.pow.circuit, 0]);

    // Keyed ring: the committed quotient, its bounds, its point grinding.
    public.borrow_mut().set_label("keyed ring");
    let polynomials = hooks::keyed_begin(prepared, statement, cfg, &mut t.proxy(cs)).expect("public keyed ring");
    t.proxy(cs).absorb_slice(hooks::QUOTIENT_LABEL);
    if let Some(v) = v
        && v.quotient.len() != hooks::QUOTIENT_LEN
    {
        fail("keyed ring quotient length");
    }
    let bound = hooks::quotient_bound(prepared);
    let quotient: Vec<[CS::Bool; 64]> = (0..hooks::QUOTIENT_LEN)
        .map(|k| witness_u64(cs, v.and_then(|v| v.quotient.get(k)).map_or(0, |&x| x as u64)))
        .collect();
    for x in &quotient {
        signed_range(cs, x, bound);
    }
    let bytes: Vec<Byte<CS::Bool>> = quotient.iter().flat_map(|x| bytes_of_bits(x)).collect();
    t.absorb_slice(cs, &bytes);
    if d.point > 0 {
        let nonce = v.map_or(0, |v| v.point_nonce.expect("a point nonce"));
        t.grinding_boundary_private(cs, hooks::POINT_GRINDING_DOMAIN, 0, d.point, nonce);
    }
    let (points, coefficients) = hooks::sample_keyed_points(&mut t.proxy(cs), cfg).expect("public points");
    // The target is linear in K: Σ_k (Σ_p c_p q_ring ξ_p^k) K_k.
    let mut weights = vec![0u128; hooks::QUOTIENT_LEN];
    for (x, c) in points.iter().zip(&coefficients) {
        let x = canonical(x, cfg);
        let mut power = mulmod(canonical(c, cfg), u128::from(hooks::RING_MODULUS) % q, q);
        for w in weights.iter_mut() {
            *w = (*w + power) % q;
            power = mulmod(power, x, q);
        }
    }
    let ring = hooks::keyed_claim(statement, &polynomials, points, &coefficients, cfg);
    let mut ring_target = ModLc::constant(canonical(&ring.target(), cfg));
    let top = (1u128 << 63) % q;
    for (x, &w) in quotient.iter().zip(&weights) {
        // K_k ≡ Σ_{b<63} 2^b x_b − 2^63 x_63.
        ring_target.add(w, &Zq::V(x[..63].to_vec()), q);
        ring_target.add((q - mulmod(w, top, q)) % q, &Zq::V(vec![x[63].clone()]), q);
    }

    stages.mark("keyed ring", [t.duplex.counts.circuit, t.pow.circuit, 0]);

    // Norm sumcheck: slack, cubic rounds, the terminal's modular square.
    public.borrow_mut().set_label("norm");
    hooks::validate_field(prepared, cfg).expect("public field bounds");
    let (instance_vars, norm_rounds, binder_rounds, padded) = hooks::rounds(prepared);
    t.proxy(cs).absorb_slice(hooks::NORM_DOMAIN);
    if d.norm_instance > 0 {
        let nonce = v.map_or(0, |v| v.norm_instance_nonce.expect("a norm instance nonce"));
        t.grinding_boundary_private(cs, hooks::NORM_INSTANCE_DOMAIN, 0, d.norm_instance, nonce);
    }
    let instance_point: Vec<u128> = (0..instance_vars).map(|_| t.squeeze_field(cs, cfg)).collect();
    let instance_point_f = field_of(&instance_point, cfg);
    let slack = witness_residue(cs, v.map_or(0, |v| canonical(&v.slack, cfg)), q);
    t.absorb_field_elements(cs, std::slice::from_ref(&slack));
    let norm_target = hooks::norm_target(prepared, &instance_point_f, cfg).expect("public norm target");
    let mut initial = ModLc::constant(canonical(&norm_target, cfg));
    initial.add(q - 1, &slack, q);
    let rounds4: Vec<[u128; 4]> = v.map_or_else(|| vec![[0; 4]; norm_rounds], |v| residues(v.norm_rounds, cfg));
    let nonces: Vec<u64> = v.map_or_else(
        || vec![0; if d.norm_round > 0 { norm_rounds } else { 0 }],
        |v| v.norm_nonces.to_vec(),
    );
    if rounds4.len() != norm_rounds || nonces.len() != if d.norm_round > 0 { norm_rounds } else { 0 } {
        panic!("norm sumcheck shape");
    }
    let (norm_point, norm_claim) =
        sumcheck_verify_private(cs, &mut t, &rounds4, initial, cfg, Some((hooks::NORM_ROUND_DOMAIN, d.norm_round, &nonces)));
    let norm_point_f = field_of(&norm_point, cfg);
    let terminal = witness_residue(cs, v.map_or(0, |v| canonical(&v.norm_terminal, cfg)), q);
    let square = mul_mod_q(cs, &terminal, &terminal, q, &public);
    let factor = canonical(&hooks::norm_terminal_factor(&instance_point_f, &norm_point_f, cfg), cfg);
    let mut check = norm_claim;
    check.add((q - factor) % q, &square, q);
    check.assert_zero(cs, q, &public, "integer norm terminal");
    t.absorb_field_elements(cs, std::slice::from_ref(&terminal));

    stages.mark("norm", [t.duplex.counts.circuit, t.pow.circuit, 0]);

    // Merge, then the word binder over the merged target.
    public.borrow_mut().set_label("binder");
    t.proxy(cs).absorb_slice(hooks::MERGE_DOMAIN);
    if d.merge > 0 {
        let nonce = v.map_or(0, |v| v.merge_nonce.expect("a merge nonce"));
        t.grinding_boundary_private(cs, hooks::MERGE_GRINDING_DOMAIN, 0, d.merge, nonce);
    }
    let eta = t.squeeze_field(cs, cfg);
    let mut target = ring_target;
    target.add(eta, &terminal, q);
    target.add(mulmod(eta, eta, q), &slack, q);
    t.proxy(cs).absorb_slice(hooks::BINDER_DOMAIN);
    let rounds3: Vec<[u128; 3]> = v.map_or_else(|| vec![[0; 3]; binder_rounds], |v| residues(v.binding_rounds, cfg));
    let nonces: Vec<u64> = v.map_or_else(
        || vec![0; if d.binding > 0 { binder_rounds } else { 0 }],
        |v| v.binding_nonces.to_vec(),
    );
    if rounds3.len() != binder_rounds || nonces.len() != if d.binding > 0 { binder_rounds } else { 0 } {
        panic!("binder sumcheck shape");
    }
    let (binder_point, binder_claim) =
        sumcheck_verify_private(cs, &mut t, &rounds3, target, cfg, Some((hooks::BINDING_ROUND_DOMAIN, d.binding, &nonces)));
    let binder_point_f = field_of(&binder_point, cfg);
    let weights = hooks::binder_terminal_weights(
        prepared,
        cfg,
        &ring,
        statement,
        &instance_point_f,
        &norm_point_f,
        F::from_with_cfg(eta, cfg),
        &binder_point_f,
    )
    .expect("public binder weights");
    let n = 2 + usize::from(padded);
    assert_eq!(weights.len(), n);
    if let Some(v) = v
        && v.binding_terminal.len() != n
    {
        fail("word binding terminal count");
    }
    let terminals: Vec<Zq<CS::Bool>> = (0..n)
        .map(|i| witness_residue(cs, v.and_then(|v| v.binding_terminal.get(i)).map_or(0, |x| canonical(x, cfg)), q))
        .collect();
    let mut check = binder_claim;
    for (w, x) in weights.iter().zip(&terminals) {
        check.add((q - canonical(w, cfg)) % q, x, q);
    }
    check.assert_zero(cs, q, &public, "word binding terminal");
    t.proxy(cs).absorb_slice(hooks::BINDER_TERMINALS);
    t.absorb_field_elements(cs, &terminals);
    if d.terminal_merge > 0 {
        let nonce = v.map_or(0, |v| v.terminal_merge_nonce.expect("a terminal merge nonce"));
        t.grinding_boundary_private(cs, hooks::TERMINAL_MERGE_DOMAIN, 0, d.terminal_merge, nonce);
    }
    let zeta = t.squeeze_field(cs, cfg);
    // bind_terminal: value = Σ_i v_i ζ^i, settled to its canonical residue.
    let mut value = ModLc::constant(0);
    let mut power = 1u128;
    for x in &terminals {
        value.add(power, x, q);
        power = mulmod(power, zeta, q);
    }
    let value = value.settle(cs, q, &public);
    {
        let mut p = t.proxy(cs);
        p.absorb_slice(hooks::SOURCE_OPENING_LABEL);
        p.absorb_slice(&q.to_le_bytes());
        crate::piop::spartan::absorb_field_elements(&mut p, &binder_point_f, cfg);
    }
    t.absorb_field_elements(cs, &[Zq::P(zeta), value.clone()]);
    let (params, column_point, row_weights) =
        hooks::bridge_claim(prepared, binder_point_f, F::from_with_cfg(zeta, cfg), cfg).expect("public bridge claim");

    stages.mark("binder", [t.duplex.counts.circuit, t.pow.circuit, 0]);

    // Bridge: public folds read off the value, the committed forest.
    public.borrow_mut().set_label("bridge folds");
    let sums: Vec<u128> = (0..params.cols())
        .map(|c| public.borrow_mut().take(128, || v.map_or(0, |v| v.sums.get(c).copied().unwrap_or(0))))
        .collect();
    if let Some(v) = v
        && v.sums != sums.as_slice()
    {
        fail("bridge fold count");
    }
    public.borrow_mut().set_label("bridge");
    let forest_len = bridge::message_count(&params);
    if let Err(e) = bridge::validate(&params, &column_point, cfg, &sums, v.map_or(forest_len, |v| v.forest.len()), &row_weights) {
        fail(&e.to_string());
    }
    let read_off = bridge::read_off(&sums, &column_point, cfg);
    let mut check = ModLc::of(&value);
    check.constant = (q - read_off % q) % q;
    check.assert_zero(cs, q, &public, "bridge prime read-off");
    bridge::bind(&mut t.proxy(cs), &sums);
    let (point, forest_claim) = {
        let mut bt = BlockTranscript::new(cs, &mut t, bridge::GRINDING_DOMAIN, d.bridge, v.map(|v| v.bridge_nonces));
        let zeta = bridge::root_point(&mut bt, &params);
        let root_claim = bridge::root_claim(&sums, &zeta);
        let out = bridge_forest(&mut bt, v.map(|v| v.forest), G::P(root_claim), &zeta, params.row_vars);
        bt.finish("bridge");
        out
    };
    let binary = bridge::binary_claim(&params, &row_weights, &point, Gf::ZERO).expect("public binary claim");
    // The binary claim's value is the forest's terminal plus one.
    let target = {
        let mut lc = Lc::of(&forest_claim);
        lc.add_const(Gf::ONE);
        lc.settle(cs)
    };
    let coefficients = hooks::coefficients(prepared, &params, exceptions, binary).expect("public coefficients");

    stages.mark("bridge", [t.duplex.counts.circuit, t.pow.circuit, 0]);

    // The joint binary sumcheck, then the opening.
    public.borrow_mut().set_label("joint");
    let opening = match hooks::geometry(prepared) {
        hooks::GeometryRef::Single(g) => {
            let (point, mu) = joint_sumcheck(cs, &mut t, g, [&coefficients[0]], &target, d.joint, v);
            stages.mark("joint", [t.duplex.counts.circuit, t.pow.circuit, 0]);
            super::falcon_opening::verify(cs, &mut t, prepared, g, &digest, root, &point, &mu, v, d.opening, round0.as_ref())
        }
        hooks::GeometryRef::Split(g) => {
            let (point, mu) = joint_sumcheck(cs, &mut t, g, coefficients.each_ref(), &target, d.joint, v);
            stages.mark("joint", [t.duplex.counts.circuit, t.pow.circuit, 0]);
            super::falcon_opening::verify(cs, &mut t, prepared, g, &digest, root, &point, &mu, v, d.opening, round0.as_ref())
        }
    };
    stages.mark("opening", [t.duplex.counts.circuit, t.pow.circuit + opening.pow.circuit, opening.merkle.circuit]);
    stages.print();
    if let Some(log) = log {
        *log = t.duplex.log.clone().unwrap_or_default();
    }
    FalconCounts { transcript: t.duplex.counts, pow: t.pow, opening }
}

/// One Falcon proof's verifier circuit as the recursion prover runs it
/// ([`super::ecdsa_verifier::witness_and_record_circuit`]).
pub struct FalconCircuit<'a> {
    pub prepared: &'a PreparedFalconAlgebraic,
    pub statement: &'a FalconAlgebraicStatement,
    pub root: [u8; 32],
    pub exceptions: &'a [Vec<u16>],
    pub proof: &'a FalconAlgebraicProof,
}

impl<'a> FalconCircuit<'a> {
    pub fn new(prepared: &'a PreparedFalconAlgebraic, statement: &'a FalconAlgebraicStatement, proof: &'a FalconAlgebraicProof) -> Self {
        Self {
            prepared,
            statement,
            root: *hooks::source_root(proof),
            exceptions: hooks::exceptions(proof).0,
            proof,
        }
    }
}

impl super::ecdsa_verifier::VerifierCircuit for FalconCircuit<'_> {
    fn run<CS: Circuit>(&self, cs: &mut CS, with_proof: bool, public: Public) {
        let proof = with_proof.then_some(self.proof);
        verify_falcon_algebraic(cs, self.prepared, self.statement, &self.root, self.exceptions, proof, public, None);
    }
}

/// The public inputs the recursive statement carries next to the Falcon
/// statement: the commitment root and the exception lists (their sparse
/// encoding), as bytes.
pub fn public_input_bytes(proof: &FalconAlgebraicProof) -> usize {
    32 + hooks::exceptions(proof).1
}

/// The outer proof's committed rows and their commitment, from the witness
/// pass (before the recording finishes), at `security` bits (100 or 128).
pub fn early_commit(
    g: &circuit::witgen::PackedWitness,
    blocks: &[circuit::witgen::BlockSpan],
    selection: &str,
    security: usize,
) -> Result<(crate::piop::spartan::flockrec::EarlyCommitted, crate::ligerito_flock::FlockCommitHint), String> {
    use crate::ligerito_flock::LigeritoSelection;
    use crate::piop::spartan::flockrec::FlockRecRelation;
    use crate::piop::spartan::profile::{Lambda100, Lambda128};
    let lig = LigeritoSelection::parse(selection, security)?;
    let start = Some(super::sha256_block_external_start());
    let early = match security {
        100 => FlockRecRelation::early_committed_with_profile::<Lambda100>(g, blocks, lig, start),
        128 => FlockRecRelation::early_committed_with_profile::<Lambda128>(g, blocks, lig, start),
        _ => return Err(format!("outer security {security}: 100 or 128")),
    }
    .map_err(|e| format!("{e:?}"))?;
    let hint = early.commit();
    Ok((early, hint))
}

/// The FlockRec relation (at `security` bits: 100 or 128) of a Falcon
/// proof's verifier circuit, from its recording and everything the
/// recording is a function of: the statement, root, exceptions and the
/// public stream.
#[allow(clippy::too_many_arguments)]
pub fn outer_relation(
    st: circuit::compact::StructuredCircuit,
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    stream: &super::val::PublicStream,
    selection: &str,
    opening_row_vars: Option<usize>,
    security: usize,
) -> Result<crate::piop::spartan::flockrec::FlockRecRelation, String> {
    use crate::ligerito_flock::LigeritoSelection;
    use crate::piop::spartan::flockrec::FlockRecRelation;
    use crate::piop::spartan::profile::{Lambda100, Lambda128};
    let digest = source_digest(statement, root, exceptions, stream);
    let (st, lig) = (std::sync::Arc::new(st), LigeritoSelection::parse(selection, security)?);
    let opener = outer_opener(selection, security)?;
    match security {
        100 => FlockRecRelation::new_with_opener::<Lambda100>(st, digest, lig, opening_row_vars, opener),
        128 => FlockRecRelation::new_with_opener::<Lambda128>(st, digest, lig, opening_row_vars, opener),
        _ => return Err(format!("outer security {security}: 100 or 128")),
    }
    .map_err(|e| format!("{e:?}"))
}

/// Everything a Falcon proof's verifier circuit is a function of: the
/// statement, root, exceptions and the public stream.
fn source_digest(
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    stream: &super::val::PublicStream,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"bitz/recursion/falcon512-algebraic/v1");
    let mut bytes = Vec::with_capacity(4 * crate::piop::spartan::falcon512_algebraic::N);
    for (key, target) in statement.public_keys.iter().zip(&statement.targets) {
        bytes.clear();
        for c in key.iter().chain(target) {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        h.update(&bytes);
    }
    h.update(root);
    for e in exceptions {
        h.update(&(e.len() as u16).to_le_bytes());
        for i in e {
            h.update(&i.to_le_bytes());
        }
    }
    for (v, w) in stream.values.iter().zip(&stream.widths) {
        h.update(&v.to_le_bytes());
        h.update(&[*w]);
    }
    *h.finalize().as_bytes()
}

/// The verifier circuit's relation split on one commitment (at `security`
/// bits: 100 or 128): its SHA-256 compressions as one batch of the
/// compression relation, the rest as FlockRec on the stub circuit
/// ([`crate::piop::spartan::split`]), the glue opening at `glue_rows` rows.
#[allow(clippy::too_many_arguments)]
pub fn split_relation(
    st: &circuit::compact::StructuredCircuit,
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    stream: &super::val::PublicStream,
    selection: &str,
    glue_rows: usize,
    security: usize,
) -> Result<crate::piop::spartan::split::SplitRelation, String> {
    use crate::piop::spartan::profile::{Lambda100, Lambda128};
    use crate::piop::spartan::split::SplitRelation;
    let digest = source_digest(statement, root, exceptions, stream);
    let lig = crate::ligerito_flock::LigeritoSelection::parse(selection, security)?;
    // `BITZ_REC_SHA_T`: the batch's claim rows (`14 + r`, `2^r` compressions
    // per committed column; unset: the batch's own split).
    let sha_rows = std::env::var("BITZ_REC_SHA_T").ok().map(|v| v.parse::<usize>().map_err(|e| format!("BITZ_REC_SHA_T: {e}"))).transpose()?;
    match security {
        100 => SplitRelation::new_at::<Lambda100>(st, digest, lig, glue_rows, sha_rows),
        128 => SplitRelation::new_at::<Lambda128>(st, digest, lig, glue_rows, sha_rows),
        _ => return Err(format!("outer security {security}: 100 or 128")),
    }
    .map_err(|e| format!("{e:?}"))
}

/// [`split_relation`] for the final verifier: the circuit recorded on the
/// public stream (no proof).
#[allow(clippy::too_many_arguments)]
pub fn replay_split_relation(
    prepared: &PreparedFalconAlgebraic,
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    stream: &super::val::PublicStream,
    selection: &str,
    glue_rows: usize,
    security: usize,
) -> Result<crate::piop::spartan::split::SplitRelation, String> {
    let replay = super::val::PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = circuit::compact::CompactRecorder::new();
    verify_falcon_algebraic(&mut rec, prepared, statement, root, exceptions, None, replay, None);
    split_relation(&rec.finish_structured(), statement, root, exceptions, stream, selection, glue_rows, security)
}

/// The outer proof's opener: `BITZ_REC_OPENER` = `log` (BitZ logarithmic,
/// the fold bits committed under `BITZ_REC_MU_LIG`, by default the outer
/// selection) or `std` (standard BitZ, the folds sent). Unset: `log`, except
/// for a GF(2^256) outer ladder (`ext:`), whose fold commitment would need
/// its own Round 0 over the extension: `std`.
pub fn outer_opener(selection: &str, security: usize) -> Result<crate::piop::spartan::flockrec::FlockRecOpener, String> {
    use crate::ligerito_flock::LigeritoSelection;
    use crate::piop::spartan::flockrec::FlockRecOpener;
    let logarithmic = || -> Result<FlockRecOpener, String> {
        let mu = std::env::var("BITZ_REC_MU_LIG").unwrap_or_else(|_| selection.to_string());
        Ok(FlockRecOpener::Logarithmic { mu: LigeritoSelection::parse(&mu, security)? })
    };
    match std::env::var("BITZ_REC_OPENER").as_deref() {
        Ok("std") => Ok(FlockRecOpener::Standard),
        Ok("log") => logarithmic(),
        Err(_) if matches!(LigeritoSelection::parse(selection, security)?, LigeritoSelection::CustomJohnsonExt { .. }) => {
            Ok(FlockRecOpener::Standard)
        }
        Err(_) => logarithmic(),
        Ok(other) => Err(format!("BITZ_REC_OPENER={other}: log or std")),
    }
}

/// Whether [`outer_opener`] picks BitZ logarithmic for `selection`.
pub fn outer_opener_is_logarithmic(selection: &str, security: usize) -> bool {
    matches!(
        outer_opener(selection, security),
        Ok(crate::piop::spartan::flockrec::FlockRecOpener::Logarithmic { .. })
    )
}

/// The final verifier's view: the circuit recorded on the public stream
/// (no proof), then its relation.
#[allow(clippy::too_many_arguments)]
pub fn replay_relation(
    prepared: &PreparedFalconAlgebraic,
    statement: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    stream: &super::val::PublicStream,
    selection: &str,
    opening_row_vars: Option<usize>,
    security: usize,
) -> Result<crate::piop::spartan::flockrec::FlockRecRelation, String> {
    let replay = super::val::PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = circuit::compact::CompactRecorder::new();
    verify_falcon_algebraic(&mut rec, prepared, statement, root, exceptions, None, replay, None);
    outer_relation(rec.finish_structured(), statement, root, exceptions, stream, selection, opening_row_vars, security)
}

/// [`crate::hybrid::joint_sumcheck::verify`]: the committed target, rounds
/// and terminal value, the linear round updates with public challenges and
/// the public terminal coefficient. Returns the point and the committed
/// terminal value (the opening's target).
fn joint_sumcheck<CS: Circuit, const N: usize>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    geometry: &Geometry<N>,
    coefficients: [&Coefficients; N],
    target: &G<CS::Bool>,
    bits: u32,
    v: Option<&hooks::ProofView<'_>>,
) -> (Vec<Gf>, G<CS::Bool>) {
    let public = t.public().clone();
    joint::validate(geometry, coefficients).expect("public joint shapes");
    let rounds = geometry.bit_log();
    if let Some(v) = v
        && (v.joint_rounds.len() != rounds || v.joint_nonces.len() != if bits > 0 { rounds } else { 0 })
    {
        public.borrow_mut().fail("joint binary sumcheck shape");
    }
    joint::bind_public(&mut t.proxy(cs), geometry, bits);
    observe_gf(cs, t, target);
    let mut value = Lc::of(target);
    let mut point = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let [u0, u2] = v.and_then(|v| v.joint_rounds.get(round)).copied().unwrap_or([Gf::ZERO; 2]).map(|x| witness(cs, x));
        observe_gf(cs, t, &u0);
        observe_gf(cs, t, &u2);
        if bits > 0 {
            let nonce = v.and_then(|v| v.joint_nonces.get(round)).copied().unwrap_or(0);
            t.grinding_boundary_private(cs, joint::GRINDING_DOMAIN, round as u64, bits, nonce);
        }
        let r: Gf = t.proxy(cs).get_field_challenge(&());
        // value = u0 + r·(value + u2) + r²·u2.
        let mut next = value.scaled(r);
        next.add(Gf::ONE, &u0).add(r + r * r, &u2);
        value = Lc::of(&next.settle(cs));
        point.push(r);
    }
    let coefficient = joint::terminal_coefficient(geometry, coefficients, &point);
    let terminal = witness(cs, v.map_or(Gf::ZERO, |v| v.joint_value));
    value.add(coefficient, &terminal);
    value.assert_zero(cs, &public, "joint binary sumcheck terminal");
    observe_gf(cs, t, &terminal);
    (point, terminal)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::piop::spartan::falcon512_algebraic::{FalconAlgebraicWitness, N};
    use crate::recursion::val::PublicStream;
    use crate::transcript::sha::{TraceOp, start_trace, take_trace};
    use circuit::witgen::CheckingWitgen;

    const Q: i64 = 12_289;

    /// `protocol::tests::fixture`: one small valid signature, repeated.
    pub(crate) fn fixture(batch: usize) -> (FalconAlgebraicStatement, FalconAlgebraicWitness) {
        let mut key = [0; N];
        key[0] = 1;
        key[N - 1] = 7;
        let mut s2 = [0i16; N];
        s2[1] = 3000;
        s2[17] = -13;
        let mut target = [0; N];
        for j in 0..N {
            target[j] = i64::from(s2[j]).rem_euclid(Q) as u16;
        }
        target[0] = (-7 * i64::from(s2[1])).rem_euclid(Q) as u16;
        target[16] = (-7 * i64::from(s2[17])).rem_euclid(Q) as u16;
        let public = FalconAlgebraicStatement { public_keys: vec![key; batch], targets: vec![target; batch] };
        let witness = FalconAlgebraicWitness { s1: vec![[0; N]; batch], s2: vec![s2; batch] };
        (public, witness)
    }

    /// The configuration the circuit implements (the "configuration G"
    /// campaign's): per-site grinding, small prime, keyed ring, 16 bridge rows.
    pub(crate) fn configure() {
        for (k, v) in [
            ("BITZ_FALCON_GRINDING_MODEL", "per-site"),
            ("BITZ_FALCON_PRIME", "small"),
            ("BITZ_ARITHMETIC_RING", "keyed"),
            ("BITZ_FALCON_BRIDGE_ROWS", "16"),
            ("BITZ_ARITHMETIC_BINDING_STRATEGY", "optimized"),
        ] {
            assert_eq!(std::env::var(k).as_deref(), Ok(v), "run with {k}={v}");
        }
    }

    /// `BITZ_REC_SECURITY` (128): the base and the outer proof's target.
    fn security() -> usize {
        std::env::var("BITZ_REC_SECURITY").ok().map_or(128, |v| v.parse().unwrap())
    }

    pub(crate) fn proof_at(batch: usize) -> (PreparedFalconAlgebraic, FalconAlgebraicStatement, FalconAlgebraicProof) {
        configure();
        let prepared = PreparedFalconAlgebraic::new(batch, security()).unwrap();
        let (public, witness) = fixture(batch);
        let committed = prepared.commit(public.clone(), witness).unwrap();
        let proof = prepared.prove(committed).unwrap();
        (prepared, public, proof)
    }

    fn run<CS: Circuit>(
        cs: &mut CS,
        prepared: &PreparedFalconAlgebraic,
        public: &FalconAlgebraicStatement,
        proof: &FalconAlgebraicProof,
        stream: Public,
        log: Option<&mut Vec<TraceOp>>,
    ) -> FalconCounts {
        let (exceptions, _) = hooks::exceptions(proof);
        let root = *hooks::source_root(proof);
        verify_falcon_algebraic(cs, prepared, public, &root, exceptions, Some(proof), stream, log)
    }

    /// The circuit's transcript is the native verifier's, op for op, and an
    /// honest proof satisfies every constraint.
    #[test]
    #[ignore = "needs the configuration G environment"]
    fn falcon_circuit_matches_native_transcript() {
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(16, |v| v.parse().unwrap());
        let (prepared, public, proof) = proof_at(batch);
        println!("{}", hooks::describe(&prepared));
        start_trace();
        prepared.verify(&public, &proof).expect("native verify");
        let trace = take_trace();
        let expected: Vec<TraceOp> = trace.iter().filter(|e| e.duplex == trace[0].duplex).map(|e| e.op.clone()).collect();
        let stream = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        let mut log = Vec::new();
        let started = std::time::Instant::now();
        let counts = run(&mut w, &prepared, &public, &proof, stream.clone(), Some(&mut log));
        println!("witness {:.2} s; {counts:?}", started.elapsed().as_secs_f64());
        println!("native ops {}, circuit ops {}", expected.len(), log.len());
        assert_eq!(crate::recursion::sha::first_divergence(&expected, &log), None);
        assert_eq!(stream.borrow().failed, None);
        assert_eq!(w.first_failure, None);
        let s = stream.borrow();
        println!("public {} B: {:?}", s.bytes(), s.bytes_by_label());
    }

    /// Every altered part of an honest proof makes the circuit unsatisfiable
    /// or a public check fail (the native verifier rejects each as well).
    #[test]
    #[ignore = "needs the configuration G environment"]
    fn falcon_circuit_rejects_tampered_proofs() {
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(16, |v| v.parse().unwrap());
        let (prepared, public, proof) = proof_at(batch);
        for part in hooks::tampered_parts(&proof) {
            let mut altered = proof.clone();
            hooks::tamper(&mut altered, part);
            assert!(prepared.verify(&public, &altered).is_err(), "{part}: the native verifier rejects");
            let stream = PublicStream::discover();
            let mut w = CheckingWitgen::new();
            run(&mut w, &prepared, &public, &altered, stream.clone(), None);
            let failed = stream.borrow().failed.clone();
            println!("{part}: constraint {:?}, public {failed:?}", w.first_failure);
            assert!(w.first_failure.is_some() || failed.is_some(), "{part}: the circuit rejects");
        }
    }

    /// One recursion step: the witness pass and the recording, the FlockRec
    /// relation, its proof, and the final verifier (which rebuilds the
    /// relation from the public stream).
    #[test]
    #[ignore = "heavy: one FlockRec proof of the verifier circuit"]
    fn falcon_recursion_step_proves_and_verifies() {
        use std::time::Instant;
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(16, |v| v.parse().unwrap());
        let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "udrg:2:4".into());
        // Standard BitZ at T = 18 rows; BitZ logarithmic at the reference
        // split.
        let security = security();
        let standard = !outer_opener_is_logarithmic(&selection, security);
        let opening_t: Option<usize> =
            std::env::var("BITZ_REC_T").ok().map(|v| v.parse().unwrap()).or(standard.then_some(18));
        let (prepared, public, proof) = proof_at(batch);
        let circuit = FalconCircuit::new(&prepared, &public, &proof);
        let t0 = Instant::now();
        let passes = crate::recursion::ecdsa_verifier::witness_and_record_circuit(&circuit, |g, blocks| {
            early_commit(g, blocks, &selection, security).expect("early committed rows")
        })
        .expect("the base proof verifies");
        let t1 = Instant::now();
        let st = &passes.circuit;
        println!(
            "circuit: committed {} cells {} rows {} blocks {}; public {} B; witness {:.2} s record {:.2} s early {:.2} s",
            st.committed,
            st.cells,
            st.rows,
            st.instances.len(),
            passes.stream.bytes(),
            passes.witness_s,
            passes.record_s,
            passes.early_s
        );
        let rel = outer_relation(passes.circuit, &public, &circuit.root, circuit.exceptions, &passes.stream, &selection, opening_t, security).unwrap();
        let (early, hint) = passes.early;
        assert!(rel.matches_early(&early), "the relation lays out the committed bits as the witness pass did");
        let wit = rel.witness_with_committed(early.rows, &passes.h).unwrap();
        let t2 = Instant::now();
        let mut tp = crate::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, &wit, &hint).expect("FlockRec prove");
        let t3 = Instant::now();
        drop((wit, rel));
        let vrel = replay_relation(&prepared, &public, &circuit.root, circuit.exceptions, &passes.stream, &selection, opening_t, security).unwrap();
        let mut tv = crate::transcript::Sha256Transcript::new();
        vrel.verify(&mut tv, &hint.commitment, &pi).expect("FlockRec verify");
        let t4 = Instant::now();
        let pi_bytes = pi.bitz.to_bytes().len() + pi.piop_bytes() + bincode::serialize(&hint.commitment).unwrap().len();
        println!("pi: {}", crate::recursion::tests::flockrec_breakdown(&pi));
        println!(
            "base proof {} B; recursion: passes {:.2} s, relation+witness {:.2} s, prove {:.2} s, final verify {:.2} s; pi {pi_bytes} B + public {} B + inputs {} B",
            proof.payload_size_bytes(),
            (t1 - t0).as_secs_f64(),
            (t2 - t1).as_secs_f64(),
            (t3 - t2).as_secs_f64(),
            (t4 - t3).as_secs_f64(),
            passes.stream.bytes(),
            public_input_bytes(&proof),
        );
    }

    /// One recursion step with the split relation: the passes, the SHA
    /// batch + glue on one commitment, its proof and the final verifier
    /// (the relation rebuilt from the public stream); a tampered opening is
    /// rejected. `BITZ_REC_GLUE_T`: the glue opening's rows (18).
    #[test]
    #[ignore = "heavy: one split proof of the verifier circuit"]
    fn falcon_recursion_split_step_proves_and_verifies() {
        use std::time::Instant;
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(16, |v| v.parse().unwrap());
        let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "ext:3:4".into());
        let glue_rows: usize = std::env::var("BITZ_REC_GLUE_T").ok().map_or(18, |v| v.parse().unwrap());
        let security = security();
        let (prepared, public, proof) = proof_at(batch);
        let circuit = FalconCircuit::new(&prepared, &public, &proof);
        let t0 = Instant::now();
        let passes = crate::recursion::ecdsa_verifier::witness_and_record_circuit(&circuit, |_, _| ()).expect("the base proof verifies");
        let t1 = Instant::now();
        let rel = split_relation(&passes.circuit, &public, &circuit.root, circuit.exceptions, &passes.stream, &selection, glue_rows, security).unwrap();
        let t2 = Instant::now();
        let (wit, times) = rel.witness(&passes.circuit, &passes.g, &passes.h).unwrap();
        let t3 = Instant::now();
        let hint = rel.commit(&wit);
        let t4 = Instant::now();
        let mut tp = crate::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, &wit, &hint).expect("split prove");
        let t5 = Instant::now();
        let st = &passes.circuit;
        println!(
            "circuit: committed {} cells {} rows {} blocks {} tail hints {}; glue f/h {}/{} bits, SHA batch 2^{}",
            st.committed,
            st.cells,
            st.rows,
            st.instances.len(),
            st.tail_hints.len(),
            rel.committed_layout().row_vars + rel.committed_layout().col_vars,
            rel.glue().derived_layout().row_vars + rel.glue().derived_layout().col_vars,
            rel.sha().log_instance_capacity(),
        );
        for (name, s) in [("SHA batch", rel.sha().security()), ("glue", rel.glue().security())] {
            println!(
                "{name} grinding: initial {} terminal {} PIOP rounds {} forest rounds {} ring {}; prime {}..{} bits",
                s.initial_grinding_bits,
                s.terminal_grinding_bits,
                s.piop_round_grinding_bits,
                s.forest_round_grinding_bits,
                s.ring_switch_grinding_bits,
                128 - s.projection_min.leading_zeros(),
                128 - s.projection_max.leading_zeros(),
            );
        }
        drop((wit, rel));
        let vrel = replay_split_relation(&prepared, &public, &circuit.root, circuit.exceptions, &passes.stream, &selection, glue_rows, security).unwrap();
        let mut tv = crate::transcript::Sha256Transcript::new();
        vrel.verify(&mut tv, &hint.commitment, &pi).expect("split verify");
        let t6 = Instant::now();
        let mut bad = pi.clone();
        let at = bad.bitz.transcript.narg_string.len() / 3;
        bad.bitz.transcript.narg_string[at] ^= 0x04;
        let mut tv = crate::transcript::Sha256Transcript::new();
        assert!(vrel.verify(&mut tv, &hint.commitment, &bad).is_err(), "tampered opening");
        let pi_bytes = pi.bitz.to_bytes().len() + pi.piop_bytes() + bincode::serialize(&hint.commitment).unwrap().len();
        println!(
            "base proof {} B; split step: passes {:.2} s, relation {:.2} s, witness {:.2} s (glue {:.2}, SHA batch {:.2}, rows {:.2}), commit {:.2} s, prove {:.2} s, final verify {:.2} s; pi {pi_bytes} B (opening {} B) + public {} B",
            proof.payload_size_bytes(),
            (t1 - t0).as_secs_f64(),
            (t2 - t1).as_secs_f64(),
            (t3 - t2).as_secs_f64(),
            times.glue_s,
            times.sha_s,
            times.rows_s,
            (t4 - t3).as_secs_f64(),
            (t5 - t4).as_secs_f64(),
            (t6 - t5).as_secs_f64(),
            pi.bitz.to_bytes().len(),
            passes.stream.bytes(),
        );
    }

    /// Records the circuit `BITZ_REC_RECORDS` times on a discovered stream
    /// (a profiling target for the recording pass).
    #[test]
    #[ignore = "profiling"]
    fn falcon_record_profile() {
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(1024, |v| v.parse().unwrap());
        let times: usize = std::env::var("BITZ_REC_RECORDS").ok().map_or(5, |v| v.parse().unwrap());
        let (prepared, public, proof) = proof_at(batch);
        let circuit = FalconCircuit::new(&prepared, &public, &proof);
        let stream = PublicStream::discover();
        let mut w = circuit::witgen::Witgen::new();
        verify_falcon_algebraic(&mut w, &prepared, &public, &circuit.root, circuit.exceptions, Some(&proof), stream.clone(), None);
        let s = stream.borrow().clone();
        for _ in 0..times {
            let started = std::time::Instant::now();
            let replay = PublicStream::replay(s.values.clone(), s.widths.clone());
            let mut rec = circuit::compact::CompactRecorder::new();
            verify_falcon_algebraic(&mut rec, &prepared, &public, &circuit.root, circuit.exceptions, None, replay, None);
            let st = rec.finish_structured();
            let digest: String = st.digest().iter().map(|b| format!("{b:02x}")).collect();
            println!("record {:.3} s ({} blocks) digest {digest}", started.elapsed().as_secs_f64(), st.instances.len());
        }
    }

    /// Which template hints are read from outside their block: the
    /// output supports, and any other external reference (a boxed committed
    /// layout needs them in one dyadic band).
    #[test]
    #[ignore = "diagnostic"]
    fn falcon_template_external_reads() {
        let batch: usize = std::env::var("BITZ_REC_FALCON_BATCH").ok().map_or(16, |v| v.parse().unwrap());
        let (prepared, public, proof) = proof_at(batch);
        let circuit = FalconCircuit::new(&prepared, &public, &proof);
        let stream = PublicStream::discover();
        let mut w = circuit::witgen::Witgen::new();
        verify_falcon_algebraic(&mut w, &prepared, &public, &circuit.root, circuit.exceptions, Some(&proof), stream.clone(), None);
        let s = stream.borrow().clone();
        let replay = PublicStream::replay(s.values.clone(), s.widths.clone());
        let mut rec = circuit::compact::CompactRecorder::new();
        verify_falcon_algebraic(&mut rec, &prepared, &public, &circuit.root, circuit.exceptions, None, replay, None);
        let st = rec.finish_structured();
        let template = &st.templates[0];
        let hints = template.hints as u32;
        let mut output_support = std::collections::BTreeSet::new();
        for out in &template.outputs {
            for &t in out.terms.iter() {
                if t < hints {
                    output_support.insert(t);
                }
            }
        }
        // External references: instance inputs, tail M rows, lazy groups.
        let block_of = |g: u32| -> Option<(usize, u32)> {
            let i = st.instances.partition_point(|inst| inst.hint_base + hints <= g);
            (i < st.instances.len() && st.instances[i].hint_base <= g).then(|| (i, g - st.instances[i].hint_base))
        };
        let mut external = std::collections::BTreeMap::<u32, usize>::new();
        let mut note = |g: u32, from: Option<usize>| {
            if let Some((i, local)) = block_of(g)
                && Some(i) != from
            {
                *external.entry(local).or_default() += 1;
            }
        };
        for (i, inst) in st.instances.iter().enumerate() {
            for input in &inst.inputs {
                for &t in input.terms.iter() {
                    note(t, Some(i));
                }
            }
        }
        for &t in &st.tail_m_terms {
            note(t, None);
        }
        for group in &st.groups {
            for (_, inputs) in &group.terms {
                for bit in inputs {
                    for &t in bit.terms.iter() {
                        note(t, None);
                    }
                }
            }
        }
        let outside: Vec<u32> = external.keys().copied().filter(|l| !output_support.contains(l)).collect();
        println!(
            "template hints {hints}, cells {}, outputs {}; output support {} hints (span {:?}..{:?}); externally read {} hints, {} outside the output support (e.g. {:?})",
            template.cells(),
            template.outputs.len(),
            output_support.len(),
            output_support.first(),
            output_support.last(),
            external.len(),
            outside.len(),
            &outside[..outside.len().min(8)],
        );
        let terms: usize = template.outputs.iter().map(|o| o.terms.len()).sum();
        println!("output terms {terms}; blocks {}; committed {}; tail hints {}", st.instances.len(), st.committed, st.tail_hints.len());
    }

    /// Prints the prepared shapes the circuit is built from.
    #[test]
    #[ignore = "diagnostic"]
    fn describe_configurations() {
        for batch in [1024usize, 4096, 32768] {
            let prepared = PreparedFalconAlgebraic::new(batch, 128).unwrap();
            println!("== {batch}\n{}", hooks::describe(&prepared));
        }
    }

    /// Prints the base proof's opening shape (rates, queries, folds) for the
    /// `BITZ_FALCON_LIGERITO` selection.
    #[test]
    #[ignore = "diagnostic"]
    fn describe_base_opening() {
        for batch in [1024usize, 4096, 32768] {
            let prepared = PreparedFalconAlgebraic::new(batch, 128).unwrap();
            println!("== {batch}\n{}", prepared.grinding_report());
        }
    }
}
