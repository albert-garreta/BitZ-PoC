//! Flock's succinct recursive Ligerito verifier
//! (`flock_core::pcs::ligerito::recursive_verifier_with_basis_succinct`) in
//! the circuit, driven through BitZ's challenger adapters
//! (`bitz::pcs::{VerifierChallenger, AtomicChallenger}`) over the in-circuit
//! BitZ transcript, with public challenges.
//!
//! Everything that depends only on challenges and the public query positions
//! — eq tables, the residual bases (outer, OOD, induced) — is computed
//! natively. The circuit keeps: the narg echo of every prover message
//! (absorbed, committed), the leaf and path hashing of every opened row,
//! and linear checks (sumcheck rounds, row folds, the residual identity).

use circuit::Circuit;
use field::Gf128 as Gf;
use flock_core::lincheck::build_eq_table;
use flock_core::pcs::ligerito::{
    LigeritoProof, VerifierConfig, ceil_log2, eval_sk_at_vks, induce_sumcheck_evaluate_at_residual,
};

use crate::bitz::pcs::{LIGERITO_BASIS_LABEL, POW_PREFIX, POW_TAG, VECTOR_SQUEEZE_TAG};
use crate::ligerito_flock::grinding_plan::{ChallengeBlock, GrindingPlan};

use super::bitz_transcript::BitzTranscript;
use super::bytes::gf_bytes;
use super::merkle::{self, Digest};
use super::sha::{ShaCounts, assert_pow};
use super::val::{G, Lc, Public};

/// [`crate::bitz::pcs::atomic`]'s cursor over Flock's work plan.
struct PlanCursor<'a> {
    plan: &'a GrindingPlan,
    next: usize,
    active: bool,
    valid: bool,
    digest: [u8; 32],
}

impl<'a> PlanCursor<'a> {
    fn new(plan: &'a GrindingPlan) -> Self {
        let mut digest = [0; 32];
        if plan
            .blocks
            .iter()
            .any(|block| block.bits > block.native_bits.unwrap_or(0))
        {
            let mut hash = blake3::Hasher::new();
            hash.update(b"bitz/flock-work-plan/v1");
            for block in &plan.blocks {
                hash.update(&(block.label.len() as u64).to_le_bytes());
                hash.update(block.label.as_bytes());
                hash.update(&[u8::from(block.native_bits.is_some())]);
                hash.update(&block.native_bits.unwrap_or(0).to_le_bytes());
                hash.update(&block.bits.to_le_bytes());
            }
            digest = *hash.finalize().as_bytes();
        }
        Self {
            plan,
            next: 0,
            active: false,
            valid: true,
            digest,
        }
    }

    fn begin(&mut self, native: Option<u32>) -> Option<(usize, &'a ChallengeBlock)> {
        let Some(block) = self.plan.blocks.get(self.next) else {
            self.valid = false;
            return None;
        };
        self.valid &= block.native_bits == native;
        let index = self.next;
        self.next += 1;
        self.active = true;
        Some((index, block))
    }
}

/// The in-circuit `AtomicChallenger<VerifierChallenger>`.
pub struct Challenger<'t, 'p, B: circuit::BoolWitness> {
    pub t: &'t mut BitzTranscript<B>,
    target_pending: bool,
    label_seen: bool,
    cursor: Option<PlanCursor<'p>>,
    /// Compressions of the PoW checks (the transcript counts its own).
    pub pow_counts: ShaCounts,
}

impl<'t, 'p, B: circuit::BoolWitness> Challenger<'t, 'p, B> {
    pub fn new(t: &'t mut BitzTranscript<B>, plan: Option<&'p GrindingPlan>) -> Self {
        Self {
            t,
            target_pending: true,
            label_seen: false,
            cursor: plan.map(PlanCursor::new),
            pow_counts: ShaCounts::default(),
        }
    }

    fn public(&self) -> Public {
        self.t.public().clone()
    }

    fn observed(&mut self) {
        if let Some(cursor) = &mut self.cursor {
            cursor.active = false;
        }
    }

    pub fn observe_label<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, label: &[u8]) {
        self.observed();
        if self.target_pending && !self.label_seen {
            assert_eq!(label, LIGERITO_BASIS_LABEL, "the opening starts with its basis label");
            self.label_seen = true;
        }
        self.t.public_message(cs, label);
    }

    /// The first observed field element: the opening target, absorbed as a
    /// public message of its (committed) value.
    pub fn observe_target<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, target: &G<B>) {
        self.observed();
        assert!(self.target_pending && self.label_seen, "the target follows the basis label");
        self.target_pending = false;
        self.t.absorb(cs, &gf_bytes(target));
    }

    /// A proof element echoed in narg: committed, absorbed.
    pub fn observe_f128<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) -> G<B> {
        self.observed();
        assert!(!self.target_pending);
        self.t.prover_gf(cs)
    }

    /// `observe_bytes` of a 32-byte root echoed in narg (`u32` length, then
    /// the bytes); `public` when the root is already a public value.
    pub fn observe_root<CS: Circuit<Bool = B>>(
        &mut self,
        cs: &mut CS,
        public: Option<[u8; 32]>,
    ) -> Digest<B> {
        self.observed();
        self.t.public_message(cs, &32u32);
        let skipped = self.t.narg.take(4);
        if self.t.narg.has_bytes() {
            assert_eq!(skipped, 32u32.to_le_bytes(), "root length prefix");
        }
        match public {
            Some(bytes) => {
                let narg = self.t.narg.take(32);
                if self.t.narg.has_bytes() && narg != bytes {
                    self.public().borrow_mut().fail("echoed root differs from the commitment");
                }
                self.t.public_message(cs, &bytes);
                merkle::public_digest(bytes)
            }
            None => {
                let raw: [u8; 32] = self.t.narg.take(32).try_into().unwrap();
                let digest = merkle::witness_digest(cs, raw);
                self.t.absorb(cs, &merkle::digest_bytes(&digest));
                digest
            }
        }
    }

    fn frame<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, digest: [u8; 32], index: usize, block: &ChallengeBlock) {
        self.t.public_message(cs, b"bitz/bitz/flock-atomic/v1".as_slice());
        self.t.public_message(cs, &digest);
        self.t.public_message(cs, &(index as u64));
        self.t.public_message(cs, &(block.label.len() as u64));
        self.t.public_message(cs, block.label.as_bytes());
        self.t.public_message(cs, &[u8::from(block.native_bits.is_some())]);
        self.t.public_message(cs, &block.native_bits.unwrap_or(0));
        self.t.public_message(cs, &block.bits);
    }

    fn begin<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, native: Option<u32>) -> u32 {
        let Some(cursor) = &mut self.cursor else {
            return native.unwrap_or(0);
        };
        let digest = cursor.digest;
        let Some((index, block)) = cursor.begin(native) else {
            return native.unwrap_or(0);
        };
        if block.bits > block.native_bits.unwrap_or(0) {
            self.frame(cs, digest, index, block);
        }
        block.bits
    }

    /// `VerifierChallenger::checked_pow`: the PoW tag and difficulty, a seed
    /// squeeze, the narg nonce, and the predicate (`bits = 0` forces the
    /// canonical zero nonce).
    fn checked_pow<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, bits: u32) {
        self.t.public_message(cs, POW_TAG);
        self.t.public_message(cs, &bits);
        let seed = self.t.duplex.squeeze_bytes(cs, 16);
        let nonce = self.t.prover_u64(cs);
        if bits == 0 {
            circuit::gf128::assert_zero(cs, &nonce);
        } else {
            let mut prefix = super::bytes::public_bytes(POW_PREFIX);
            prefix.extend(seed);
            assert_pow(cs, &prefix, &nonce, bits, &mut self.pow_counts);
        }
    }

    fn before_sample<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) {
        if self.cursor.as_ref().is_none_or(|cursor| cursor.active) {
            return;
        }
        let bits = self.begin(cs, None);
        if bits > 0 {
            self.checked_pow(cs, bits);
        }
    }

    pub fn sample_f128<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS) -> Gf {
        self.before_sample(cs);
        self.t.verifier_gf(cs)
    }

    /// A field draw of which only the low `k ≤ 64` bits of its low word
    /// are used (a query position): only those are published.
    pub fn sample_low_bits<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, k: usize) -> u64 {
        self.before_sample(cs);
        self.t.verifier_low_bits(cs, 16, k) as u64
    }

    pub fn sample_f128_vec<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, n: usize) -> Vec<Gf> {
        if n != 0 {
            self.before_sample(cs);
        }
        self.t.public_message(cs, VECTOR_SQUEEZE_TAG);
        self.t.public_message(cs, &(n as u64));
        (0..n).map(|_| self.t.verifier_gf(cs)).collect()
    }

    pub fn verify_pow<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, bits: u32) {
        let effective = self.begin(cs, Some(bits));
        self.checked_pow(cs, effective);
    }

    /// `AtomicChallenger::finish`: the whole plan ran.
    pub fn finish(self) {
        if let Some(cursor) = &self.cursor {
            assert!(
                cursor.valid && cursor.next == cursor.plan.blocks.len(),
                "the circuit runs Flock's whole work plan"
            );
        }
        assert!(!self.target_pending);
    }
}

/// `RoundQuad`: `c + r·b + r²·a`.
#[derive(Clone)]
pub(super) struct Quad<B: circuit::BoolWitness> {
    c: Lc<B>,
    b: Lc<B>,
    a: Lc<B>,
}

impl<B: circuit::BoolWitness> Quad<B> {
    pub(super) fn from_msg(u0: &G<B>, u2: &G<B>, t_r: &Lc<B>) -> Self {
        let mut b = t_r.clone();
        b.add(Gf::ONE, u2);
        Self {
            c: Lc::of(u0),
            b,
            a: Lc::of(u2),
        }
    }

    pub(super) fn eval(&self, r: Gf) -> Lc<B> {
        let mut out = self.c.clone();
        out.add_scaled(r, &self.b);
        out.add_scaled(r * r, &self.a);
        out
    }

    pub(super) fn fold(&self, rhs: &Self, alpha: Gf) -> Self {
        let f = |x: &Lc<B>, y: &Lc<B>| {
            let mut out = x.clone();
            out.add_scaled(alpha, y);
            out
        };
        Self {
            c: f(&self.c, &rhs.c),
            b: f(&self.b, &rhs.b),
            a: f(&self.a, &rhs.a),
        }
    }
}

/// `try_sample_distinct_queries`: draws until `count` distinct positions,
/// then sorts.
fn sample_distinct_queries<CS: Circuit>(
    cs: &mut CS,
    ch: &mut Challenger<'_, '_, CS::Bool>,
    block_len: usize,
    count: usize,
) -> Vec<usize> {
    assert!(count <= block_len);
    assert!(block_len.is_power_of_two(), "positions are low bits of the draw");
    let k = block_len.trailing_zeros() as usize;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let q = ch.sample_low_bits(cs, k) as usize;
        if seen.insert(q) {
            out.push(q);
        }
    }
    out.sort_unstable();
    out
}

/// One opened level: rows and the multi-proof siblings, committed.
fn witness_level<CS: Circuit>(
    cs: &mut CS,
    proof: Option<(&[Vec<Gf>], &[[u8; 32]])>,
    queries: usize,
    width: usize,
    siblings: usize,
) -> (Vec<Vec<G<CS::Bool>>>, Vec<Digest<CS::Bool>>) {
    let rows = (0..queries)
        .map(|q| {
            (0..width)
                .map(|j| {
                    let v = proof.map_or(Gf::ZERO, |(rows, _)| rows[q][j]);
                    super::val::witness(cs, v)
                })
                .collect()
        })
        .collect();
    let sibs = (0..siblings)
        .map(|k| {
            let h = proof.map_or([0u8; 32], |(_, p)| p[k]);
            merkle::witness_digest(cs, h)
        })
        .collect();
    (rows, sibs)
}

/// Verifies the opened rows of one level against `root` (the subtrees'
/// leaves and paths as independent sub-circuits); returns the in-circuit
/// compressions of the leaf hashes (the rest of `counts`' increase is the
/// paths' nodes).
pub(super) fn verify_opens<CS: Circuit>(
    cs: &mut CS,
    root: &Digest<CS::Bool>,
    block_len: usize,
    queries: &[usize],
    rows: &[Vec<G<CS::Bool>>],
    siblings: &[Digest<CS::Bool>],
    counts: &mut ShaCounts,
) -> usize {
    let leaf_blocks = std::sync::atomic::AtomicUsize::new(0);
    let leaf = |cs: &mut CS, q: usize, c: &mut ShaCounts| {
        let before = c.circuit;
        let digest = merkle::leaf_hash(cs, &rows[q], c);
        leaf_blocks.fetch_add(c.circuit - before, std::sync::atomic::Ordering::Relaxed);
        digest
    };
    let top = merkle::multi_proof_root_with(cs, block_len, queries, &leaf, siblings, counts);
    merkle::assert_digest_eq(cs, &top, root);
    leaf_blocks.into_inner()
}

/// `induce_sumcheck_enforced_sum`: `Σ_i eq(α)_i Σ_j eq(r)_j row_i[j]`.
pub(super) fn enforced_sum<B: circuit::BoolWitness>(rows: &[Vec<G<B>>], v: &[Gf], alpha: &[Gf]) -> Lc<B> {
    let eq = build_eq_table(v);
    let alpha_w: Vec<Gf> = if rows.is_empty() {
        Vec::new()
    } else {
        build_eq_table(alpha).into_iter().take(rows.len()).collect()
    };
    let mut sum = Lc::new();
    for (i, row) in rows.iter().enumerate() {
        for (j, y) in row.iter().enumerate() {
            sum.add(alpha_w[i] * eq[j], y);
        }
    }
    sum
}

struct LevelCtx {
    log_msg_cols: usize,
    queries: Vec<usize>,
    alpha: Vec<Gf>,
    ris_start: usize,
    beta: Gf,
}

struct OodCtx {
    z: Vec<Gf>,
    ris_start: usize,
    beta: Gf,
}

/// Costs of one in-circuit opening.
#[derive(Clone, Copy, Debug, Default)]
pub struct LigeritoCounts {
    pub merkle: ShaCounts,
    pub pow: ShaCounts,
    pub opened_elements: usize,
    pub siblings: usize,
}

/// The circuit of `recursive_verifier_with_basis_succinct`: `target` the
/// (committed) opening target, `initial_root` the public commitment root,
/// `eval_b_residual` the public basis evaluation at the residual point.
#[allow(clippy::too_many_arguments)]
pub fn verify<CS: Circuit>(
    cs: &mut CS,
    config: &VerifierConfig,
    proof: Option<&LigeritoProof>,
    log_n: usize,
    target: &G<CS::Bool>,
    initial_root: [u8; 32],
    eval_b_residual: &dyn Fn(&[Gf], usize) -> Vec<Gf>,
    ch: &mut Challenger<'_, '_, CS::Bool>,
) -> LigeritoCounts {
    let public = ch.public();
    let mut counts = LigeritoCounts::default();
    let initial_k = config.initial_k;
    let r = config.recursive_steps;
    assert!(r >= 1 && config.recursive_ks.len() == r && config.log_inv_rates.len() == r + 1);

    ch.observe_label(cs, LIGERITO_BASIS_LABEL);
    ch.observe_target(cs, target);
    ch.observe_root(cs, Some(initial_root));

    let log_msg_cols_0 = log_n - initial_k;
    let block_len_0 = 1usize << (log_msg_cols_0 + config.log_inv_rates[0]);
    let num_interleaved_0 = 1usize << initial_k;

    // Settled after every round, so the running combinations stay small.
    let mut t_r = Lc::of(target);
    let (u0, u2) = (ch.observe_f128(cs), ch.observe_f128(cs));
    let mut running = Quad::from_msg(&u0, &u2, &t_r);

    let fold_bits = |lvl: usize| -> u32 { config.fold_grinding_bits.get(lvl).copied().unwrap_or(0) as u32 };
    let ood_count = |lvl: usize| -> usize { config.ood_samples.get(lvl).copied().unwrap_or(0) };
    assert_eq!(ood_count(0), 0, "L0 is bound by the opening's own claim");
    let mut ood_ctxs: Vec<OodCtx> = Vec::new();

    let mut r_lane_fold = Vec::with_capacity(initial_k);
    for j in 0..initial_k {
        let bits = fold_bits(0).saturating_sub(j as u32);
        if bits > 0 {
            ch.verify_pow(cs, bits);
        }
        let ri = ch.sample_f128(cs);
        r_lane_fold.push(ri);
        t_r = Lc::of(&running.eval(ri).settle(cs));
        let (u0, u2) = (ch.observe_f128(cs), ch.observe_f128(cs));
        running = Quad::from_msg(&u0, &u2, &t_r);
    }

    let root_1 = ch.observe_root(cs, None);
    for _ in 0..ood_count(1) {
        let z = ch.sample_f128_vec(cs, log_n - initial_k);
        let y = ch.observe_f128(cs);
        let (i0, i2) = (ch.observe_f128(cs), ch.observe_f128(cs));
        let intro = Quad::from_msg(&i0, &i2, &Lc::of(&y));
        let beta = ch.sample_f128(cs);
        running = running.fold(&intro, beta);
        t_r.add(beta, &y);
        ood_ctxs.push(OodCtx { z, ris_start: initial_k, beta });
    }

    ch.verify_pow(cs, config.grinding_bits[0] as u32);
    let num_queries_0 = config.queries[0];
    let queries_0 = sample_distinct_queries(cs, ch, block_len_0, num_queries_0);
    let alpha_0 = ch.sample_f128_vec(cs, ceil_log2(num_queries_0));
    let sib0 = merkle::multi_proof_len(block_len_0, &queries_0);
    let (rows0, sibs0) = witness_level(
        cs,
        proof.map(|p| (p.initial_proof.opened_rows.as_slice(), p.initial_proof.merkle_proof.as_slice())),
        num_queries_0,
        num_interleaved_0,
        sib0,
    );
    if let Some(p) = proof {
        assert_eq!(p.initial_proof.merkle_proof.len(), sib0, "L0 multi-proof shape");
    }
    counts.opened_elements += num_queries_0 * num_interleaved_0;
    counts.siblings += sib0;
    let root_0 = merkle::public_digest(initial_root);
    verify_opens(cs, &root_0, block_len_0, &queries_0, &rows0, &sibs0, &mut counts.merkle);

    let n1 = log_n - initial_k;
    let enforced_0 = enforced_sum(&rows0, &r_lane_fold, &alpha_0).settle(cs);
    let (i0, i2) = (ch.observe_f128(cs), ch.observe_f128(cs));
    let intro_0 = Quad::from_msg(&i0, &i2, &Lc::of(&enforced_0));
    let beta_0 = ch.sample_f128(cs);
    running = running.fold(&intro_0, beta_0);
    t_r.add(beta_0, &enforced_0);
    let mut level_ctxs = vec![LevelCtx {
        log_msg_cols: n1,
        queries: queries_0,
        alpha: alpha_0,
        ris_start: initial_k,
        beta: beta_0,
    }];
    let mut ris: Vec<Gf> = r_lane_fold.clone();

    let mut prev_root = root_1;
    let mut prev_log_num_interleaved = config.recursive_ks[0];
    let mut prev_log_msg_cols = n1 - prev_log_num_interleaved;
    let mut prev_log_inv_rate = config.log_inv_rates[1];
    let mut n_current = n1;

    for i in 0..r {
        let k_i = config.recursive_ks[i];
        assert!(n_current >= k_i);
        let mut level_rs = Vec::with_capacity(k_i);
        for j in 0..k_i {
            let bits = fold_bits(i + 1).saturating_sub(j as u32);
            if bits > 0 {
                ch.verify_pow(cs, bits);
            }
            let ri = ch.sample_f128(cs);
            ris.push(ri);
            level_rs.push(ri);
            t_r = Lc::of(&running.eval(ri).settle(cs));
            let (u0, u2) = (ch.observe_f128(cs), ch.observe_f128(cs));
            running = Quad::from_msg(&u0, &u2, &t_r);
        }
        n_current -= k_i;

        let prev_block_len = 1usize << (prev_log_msg_cols + prev_log_inv_rate);
        let prev_num_interleaved = 1usize << prev_log_num_interleaved;

        if i == r - 1 {
            let yr: Vec<G<CS::Bool>> = (0..1usize << n_current).map(|_| ch.observe_f128(cs)).collect();
            ch.verify_pow(cs, config.grinding_bits[i + 1] as u32);
            let num_queries_last = config.queries[i + 1];
            let queries_last = sample_distinct_queries(cs, ch, prev_block_len, num_queries_last);
            let alpha_last = ch.sample_f128_vec(cs, ceil_log2(num_queries_last));
            let sib = merkle::multi_proof_len(prev_block_len, &queries_last);
            let (rows, sibs) = witness_level(
                cs,
                proof.map(|p| (p.final_proof.opened_rows.as_slice(), p.final_proof.merkle_proof.as_slice())),
                num_queries_last,
                prev_num_interleaved,
                sib,
            );
            counts.opened_elements += num_queries_last * prev_num_interleaved;
            counts.siblings += sib;
            verify_opens(cs, &prev_root, prev_block_len, &queries_last, &rows, &sibs, &mut counts.merkle);
            let enforced_last = enforced_sum(&rows, &level_rs, &alpha_last).settle(cs);
            let beta_last = ch.sample_f128(cs);
            t_r.add(beta_last, &enforced_last);
            level_ctxs.push(LevelCtx {
                log_msg_cols: n_current,
                queries: queries_last,
                alpha: alpha_last,
                ris_start: ris.len(),
                beta: beta_last,
            });

            // The residual identity, every basis public.
            let yr_log_n = n_current;
            let yr_len = 1usize << yr_log_n;
            let induced: Vec<Vec<Gf>> = level_ctxs
                .iter()
                .map(|ctx| {
                    let sks_vks = eval_sk_at_vks(ctx.log_msg_cols);
                    let ris_for_basis = &ris[ctx.ris_start..ctx.ris_start + ctx.log_msg_cols - yr_log_n];
                    induce_sumcheck_evaluate_at_residual(
                        ctx.log_msg_cols,
                        &sks_vks,
                        &ctx.queries,
                        &ctx.alpha,
                        ris_for_basis,
                        yr_log_n,
                    )
                })
                .collect();
            let mut ood_residuals: Vec<Vec<Gf>> = Vec::with_capacity(ood_ctxs.len());
            for ctx in &ood_ctxs {
                let folded = ctx.z.len() - yr_log_n;
                let mut scalar = ctx.beta;
                for b in 0..folded {
                    scalar *= Gf::ONE + ctx.z[b] + ris[ctx.ris_start + b];
                }
                let mut tail = build_eq_table(&ctx.z[folded..]);
                for v in tail.iter_mut() {
                    *v *= scalar;
                }
                ood_residuals.push(tail);
            }
            let evb = eval_b_residual(&ris, yr_log_n);
            assert_eq!(evb.len(), yr_len);
            let mut inner = Lc::new();
            for y in 0..yr_len {
                let mut combined = evb[y];
                for (k, residual) in induced.iter().enumerate() {
                    combined += level_ctxs[k].beta * residual[y];
                }
                for resid in &ood_residuals {
                    combined += resid[y];
                }
                inner.add(combined, &yr[y]);
            }
            inner.add_lc(&t_r);
            inner.assert_zero(cs, &public, "ligerito residual identity");
            counts.pow = ch.pow_counts;
            return counts;
        }

        let root_next = ch.observe_root(cs, None);
        for _ in 0..ood_count(i + 2) {
            let z = ch.sample_f128_vec(cs, n_current);
            let y = ch.observe_f128(cs);
            let (i0, i2) = (ch.observe_f128(cs), ch.observe_f128(cs));
            let intro = Quad::from_msg(&i0, &i2, &Lc::of(&y));
            let beta = ch.sample_f128(cs);
            running = running.fold(&intro, beta);
            t_r.add(beta, &y);
            ood_ctxs.push(OodCtx { z, ris_start: ris.len(), beta });
        }

        ch.verify_pow(cs, config.grinding_bits[i + 1] as u32);
        let num_queries_i = config.queries[i + 1];
        let queries_i = sample_distinct_queries(cs, ch, prev_block_len, num_queries_i);
        let alpha_i = ch.sample_f128_vec(cs, ceil_log2(num_queries_i));
        let sib = merkle::multi_proof_len(prev_block_len, &queries_i);
        let (rows, sibs) = witness_level(
            cs,
            proof.map(|p| {
                (
                    p.recursive_proofs[i].opened_rows.as_slice(),
                    p.recursive_proofs[i].merkle_proof.as_slice(),
                )
            }),
            num_queries_i,
            prev_num_interleaved,
            sib,
        );
        counts.opened_elements += num_queries_i * prev_num_interleaved;
        counts.siblings += sib;
        verify_opens(cs, &prev_root, prev_block_len, &queries_i, &rows, &sibs, &mut counts.merkle);
        let enforced_i = enforced_sum(&rows, &level_rs, &alpha_i).settle(cs);
        let (i0, i2) = (ch.observe_f128(cs), ch.observe_f128(cs));
        let intro_i = Quad::from_msg(&i0, &i2, &Lc::of(&enforced_i));
        let beta_i = ch.sample_f128(cs);
        running = running.fold(&intro_i, beta_i);
        t_r.add(beta_i, &enforced_i);
        level_ctxs.push(LevelCtx {
            log_msg_cols: n_current,
            queries: queries_i,
            alpha: alpha_i,
            ris_start: ris.len(),
            beta: beta_i,
        });

        prev_root = root_next;
        let k_next = config.recursive_ks[i + 1];
        assert!(n_current >= k_next);
        prev_log_num_interleaved = k_next;
        prev_log_msg_cols = n_current - k_next;
        prev_log_inv_rate = config.log_inv_rates[i + 2];
    }
    unreachable!()
}
