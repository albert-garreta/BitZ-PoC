//! The algebraic Falcon verifier's shared opening
//! (`hybrid::opening::verify_joint_with_security`) in the circuit: the ring
//! switch and the zero-padding bases over the opening's block-grinding
//! transcript, then Flock's compact-terminal Ligerito verifier
//! (`recursive_verifier_with_basis_initial_compact`) driven through the
//! work-plan challenger (`hybrid::opening::grinding::GrindingChallenger` over
//! `ZincChallenger`).
//!
//! As in [`super::ligerito`], every challenge is public and everything that
//! depends on challenges and query positions alone (eq tables, induced and
//! residual bases, the Reed–Solomon matrix of the final level) is computed
//! natively. The circuit keeps the committed prover messages and their
//! absorption, the hashing of opened rows and paths, the final level's
//! complete re-encoding and Merkle tree, the proof-of-work predicates and
//! linear checks.

use circuit::Circuit;
use field::Gf128 as Gf;
use flock_core::lincheck::build_eq_table;
use flock_core::pcs::ligerito::{VerifierConfig, ceil_log2, eval_sk_at_vks, induce_sumcheck_evaluate_at_residual};

use crate::hybrid::opening::Geometry;
use crate::hybrid::opening::recursion as opening;
use crate::ligerito_flock::grinding_plan::GrindingPlan;
use crate::piop::spartan::falcon512_algebraic::PreparedFalconAlgebraic;
use crate::piop::spartan::falcon512_algebraic::recursion_hooks as hooks;
use crate::transcript::traits::Transcript;

use super::bytes::{Byte, bytes_of_bits, gf_bytes, public_bytes, witness_u64};
use super::falcon_verifier::BlockTranscript;
use super::ligerito::{Quad, enforced_sum, verify_opens};
use super::merkle::{self, Digest};
use super::sha::{ShaCounts, assert_pow};
use super::spartan::SpartanTranscript;
use super::val::{G, Lc, Public, witness};

/// Costs of the in-circuit opening.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpeningCounts {
    /// Leaf and node hashing (opened rows, paths, the final tree).
    pub merkle: ShaCounts,
    /// The work plan's and Flock's proof-of-work predicates.
    pub pow: ShaCounts,
    pub opened_elements: usize,
    pub siblings: usize,
    /// Codeword elements of the final level, re-encoded and hashed.
    pub final_codeword: usize,
}

const ATOMIC_LABEL: &[u8] = b"bitz/flock/atomic/v1";
const BASIS_LABEL: &[u8] = b"flock-ligerito-basis-v0";

/// The in-circuit `GrindingChallenger<ZincChallenger>`: Flock's challenger
/// calls with the work plan's atomic frames and extra proof of work, over
/// the opening's block-grinding transcript.
struct PlanChallenger<'b, 'a, 'n, 'p, CS: Circuit> {
    bt: &'b mut BlockTranscript<'a, 'n, CS>,
    plan: &'p GrindingPlan,
    next: usize,
    active: bool,
    /// The plan's own nonces (`None` while recording) and their cursor.
    nonces: Option<&'p [u64]>,
    cursor: usize,
    pow: ShaCounts,
}

impl<'b, 'a, 'n, 'p, CS: Circuit> PlanChallenger<'b, 'a, 'n, 'p, CS> {
    fn public(&self) -> Public {
        self.bt.t.public().clone()
    }

    fn observe_label(&mut self, label: &[u8]) {
        self.active = false;
        self.bt.absorb_slice(label);
    }

    fn observe_public(&mut self, bytes: &[u8]) {
        self.active = false;
        self.bt.absorb_slice(bytes);
    }

    fn observe_committed(&mut self, bytes: &[Byte<CS::Bool>]) {
        self.active = false;
        self.bt.absorb_committed(bytes);
    }

    fn observe_f128(&mut self, g: &G<CS::Bool>) {
        self.observe_committed(&gf_bytes(g));
    }

    /// A committed prover message, absorbed.
    fn message(&mut self, value: Gf) -> G<CS::Bool> {
        let g = witness(self.bt.cs, value);
        self.observe_f128(&g);
        g
    }

    /// `GrindingChallenger::begin`: the atomic frame of the next block.
    fn begin(&mut self, native: Option<u32>) -> u32 {
        let block = self.plan.blocks.get(self.next).expect("the work plan covers the opening");
        assert_eq!(block.native_bits, native, "the opening follows the work plan");
        let bits = block.bits;
        let label = block.label.clone();
        self.observe_label(ATOMIC_LABEL);
        self.observe_public(&(self.next as u64).to_le_bytes());
        self.observe_label(label.as_bytes());
        self.observe_public(&bits.to_le_bytes());
        self.observe_public(&[u8::from(native.is_some())]);
        self.next += 1;
        self.active = true;
        bits
    }

    /// `ZincChallenger::verify_pow`: a seed squeeze (only hashed), the
    /// predicate (`bits = 0` forces the zero nonce), the nonce absorbed by
    /// the inner challenger (the open block stays active).
    fn checked_pow(&mut self, bits: u32, nonce: Option<u64>) {
        self.bt.begin_sampling();
        let seed = self.bt.t.duplex.squeeze_bytes(self.bt.cs, 16);
        if bits == 0 {
            if nonce.is_some_and(|n| n != 0) {
                self.public().borrow_mut().fail("a zero-difficulty nonce must be zero");
            }
            self.bt.absorb_slice(&0u64.to_le_bytes());
        } else {
            let nonce = witness_u64(self.bt.cs, nonce.unwrap_or(0));
            assert_pow(self.bt.cs, &seed, &nonce, bits, &mut self.pow);
            self.bt.absorb_committed(&bytes_of_bits(&nonce));
        }
    }

    fn before_sample(&mut self) {
        if self.active {
            return;
        }
        let bits = self.begin(None);
        if bits > 0 {
            let nonce = self.nonces.map(|n| n.get(self.cursor).copied().unwrap_or(0));
            self.cursor += 1;
            self.checked_pow(bits, nonce);
        }
    }

    fn sample_f128(&mut self) -> Gf {
        self.before_sample();
        self.bt.get_field_challenge(&())
    }

    /// A draw of which only `v.lo mod 2^k` is used (a query position).
    fn sample_low_bits(&mut self, k: usize) -> usize {
        self.before_sample();
        self.bt.begin_sampling();
        self.bt.t.duplex.squeeze_low_bits(self.bt.cs, 16, k) as usize
    }

    fn sample_f128_vec(&mut self, n: usize) -> Vec<Gf> {
        if n != 0 {
            self.before_sample();
        }
        (0..n).map(|_| self.bt.get_field_challenge(&())).collect()
    }

    /// `GrindingChallenger::verify_pow` with Flock's nonce.
    fn verify_pow(&mut self, native_bits: u32, nonce: Option<u64>) {
        let bits = self.begin(Some(native_bits));
        self.checked_pow(bits, nonce);
    }

    fn finish(&self) {
        assert_eq!(self.next, self.plan.blocks.len(), "the circuit runs the whole work plan");
        if let Some(n) = self.nonces
            && n.len() != self.cursor
        {
            self.public().borrow_mut().fail("work plan nonce count");
        }
    }
}

/// `try_sample_distinct_queries` (positions `v.lo mod block_len`, sorted).
fn sample_distinct_queries<CS: Circuit>(ch: &mut PlanChallenger<'_, '_, '_, '_, CS>, block_len: usize, count: usize) -> Vec<usize> {
    assert!(count <= block_len && block_len.is_power_of_two());
    let k = block_len.trailing_zeros() as usize;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let q = ch.sample_low_bits(k);
        if seen.insert(q) {
            out.push(q);
        }
    }
    out.sort_unstable();
    out
}

/// The Reed–Solomon matrix of the final level: `M[row][column]`, the
/// codeword of the `column`-th unit message (`ligero_commit`'s encoding is
/// lane-wise and linear).
fn encoding_matrix(log_msg_cols: usize, log_inv_rate: usize) -> Vec<Vec<Gf>> {
    let cols = 1usize << log_msg_cols;
    let log_block_len = log_msg_cols + log_inv_rate;
    let mut identity = vec![Gf::ZERO; cols * cols];
    for c in 0..cols {
        identity[c * cols + c] = Gf::ONE;
    }
    let ntt = flock_core::ntt::additive_ntt_f128::AdditiveNttF128::standard(log_block_len);
    let witness = flock_core::pcs::ligerito::ligero_commit(
        &identity,
        log_msg_cols,
        log_msg_cols,
        log_inv_rate,
        &ntt,
        crate::ligerito_flock::MERKLE_HASH,
    );
    (0..1usize << log_block_len)
        .map(|row| witness.mat[row * cols..(row + 1) * cols].to_vec())
        .collect()
}

/// One opened level's committed rows and multi-proof siblings; a proof of
/// another shape (already a public failure) reads as zeros past its end.
fn witness_opened<CS: Circuit>(
    cs: &mut CS,
    level: Option<&flock_core::pcs::ligerito::RecursiveProof>,
    queries: usize,
    width: usize,
    siblings: usize,
) -> (Vec<Vec<G<CS::Bool>>>, Vec<Digest<CS::Bool>>) {
    let element = |q: usize, j: usize| {
        level.and_then(|p| p.opened_rows.get(q)).and_then(|row| row.get(j)).copied().unwrap_or(Gf::ZERO)
    };
    let rows = (0..queries).map(|q| (0..width).map(|j| witness(cs, element(q, j))).collect()).collect();
    let sibs = (0..siblings)
        .map(|k| merkle::witness_digest(cs, level.and_then(|p| p.merkle_proof.get(k)).copied().unwrap_or([0; 32])))
        .collect();
    (rows, sibs)
}

/// The circuit of the shared opening of `point` against the joint
/// sumcheck's committed terminal value (the opening target `μ`), the
/// commitment `root` and the statement digest.
#[allow(clippy::too_many_arguments)]
pub fn verify<CS: Circuit, const N: usize>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    prepared: &PreparedFalconAlgebraic,
    geometry: &Geometry<N>,
    digest: &[u8; 32],
    root: &[u8; 32],
    point: &[Gf],
    mu: &G<CS::Bool>,
    v: Option<&hooks::ProofView<'_>>,
    opening_bits: u32,
) -> OpeningCounts {
    let public = t.public().clone();
    let fail = |why: &str| public.borrow_mut().fail(why.to_string());
    opening::validate(geometry).expect("public geometry");
    assert_eq!(point.len(), geometry.bit_log(), "joint source opening point");
    let resolved = hooks::ligerito(prepared);
    let vc: &VerifierConfig = resolved.verifier();
    let plan = hooks::pcs_plan(prepared);
    assert!(plan.matches(resolved.security()), "shared Ligerito grinding configuration");
    if let Some(v) = v
        && opening::check_shape(vc, v.ligerito).is_err()
    {
        fail("Ligerito proof shape");
    }
    let occupied = opening::occupied_lanes(geometry);
    let lanes = geometry.lanes();
    let mut counts = OpeningCounts::default();
    public.borrow_mut().set_label("opening");
    let mut stages = super::falcon_verifier::Stages::new(if v.is_some() { "witness opening" } else { "record opening" })
        .with_base([t.duplex.counts.circuit, t.pow.circuit, 0]);

    let mut bt = BlockTranscript::new(cs, t, hooks::OPENING_GRINDING_DOMAIN, opening_bits, v.map(|v| v.opening_nonces));
    opening::bind_allocation(geometry, &mut bt);

    // Ring switch: Σ_v eq(r_lo, v) s_v = μ, s_v absorbed, β₀ = Σ_u eq(r″, u) s_u
    // over the bit transpose of s.
    let s: Vec<G<CS::Bool>> = (0..128).map(|i| witness(bt.cs, v.and_then(|v| v.s_v.get(i)).copied().unwrap_or(Gf::ZERO))).collect();
    if let Some(v) = v
        && v.s_v.len() != 128
    {
        fail("ring switch shape");
    }
    let eq_lo = build_eq_table(&point[..7]);
    let mut claim = Lc::of(mu);
    for (e, x) in eq_lo.iter().zip(&s) {
        claim.add(*e, x);
    }
    claim.assert_zero(bt.cs, &public, "ring switch claim");
    let mut bytes: Vec<Byte<CS::Bool>> = vec![Byte::P(0x20)];
    for x in &s {
        bytes.extend(gf_bytes(x));
    }
    bt.absorb_committed(&bytes);
    let r2: Vec<Gf> = bt.get_field_challenges(7, &());
    let eq_r2 = build_eq_table(&r2);
    // s_u: bit v of s_u is bit u of s_v.
    let bits: Vec<_> = s.iter().map(G::bits).collect();
    let mut target = Lc::new();
    for (u, e) in eq_r2.iter().enumerate() {
        let s_u: [CS::Bool; 128] = std::array::from_fn(|v| bits[v][u].clone());
        target.add(*e, &G::of_bits(s_u));
    }
    let target = target.settle(bt.cs);
    let padding = opening::sample_padding(&mut bt, geometry);

    stages.mark("ring switch", [bt.t.duplex.counts.circuit, bt.t.pow.circuit, counts.merkle.circuit]);
    let pcs_nonces = v.map(|v| v.pcs_nonces);
    let mut ch = PlanChallenger { bt: &mut bt, plan, next: 0, active: false, nonces: pcs_nonces, cursor: 0, pow: ShaCounts::default() };
    let lig = v.map(|v| v.ligerito);
    let log_n = geometry.packed_log();
    let eval = |prefix: &[Gf], log_y: usize| opening::residual(prefix, log_y, &point[7..], &eq_r2, &padding);

    // ---- recursive_verifier_with_basis_initial_impl -------------------------
    let initial_k = vc.initial_k;
    let r = vc.recursive_steps;
    assert!(r >= 1 && vc.recursive_ks.len() == r && vc.log_inv_rates.len() == r + 1);
    assert_eq!(vc.ood_samples.iter().sum::<usize>(), 0, "the circuit's regime takes no OOD samples");
    // The opening's initial root is the statement digest (the initial rows
    // are authenticated against the commitment root below).
    if let Some(p) = lig
        && &p.initial_root != digest
    {
        fail("initial root differs from the statement");
    }
    ch.observe_label(BASIS_LABEL);
    ch.observe_f128(&target);
    ch.observe_public(digest);

    let log_msg_cols_0 = log_n - initial_k;
    let block_len_0 = 1usize << (log_msg_cols_0 + vc.log_inv_rates[0]);
    let num_interleaved_0 = 1usize << initial_k;
    assert_eq!(num_interleaved_0, lanes, "level 0 folds the virtual lanes");

    let mut tx = 0usize;
    let mut next_message = |ch: &mut PlanChallenger<'_, '_, '_, '_, CS>| -> (G<CS::Bool>, G<CS::Bool>) {
        let msg = lig.and_then(|p| p.sumcheck_transcript.get(tx)).copied();
        tx += 1;
        let u0 = ch.message(msg.map_or(Gf::ZERO, |m| m.u_0));
        let u2 = ch.message(msg.map_or(Gf::ZERO, |m| m.u_2));
        (u0, u2)
    };
    let mut fold_nonce = 0usize;
    let mut next_fold_nonce = || {
        let n = lig.map(|p| p.fold_grinding_nonces.get(fold_nonce).copied().unwrap_or(0));
        fold_nonce += 1;
        n
    };
    let fold_bits = |lvl: usize| -> u32 { vc.fold_grinding_bits.get(lvl).copied().unwrap_or(0) as u32 };

    let mut t_r = Lc::of(&target);
    let (u0, u2) = next_message(&mut ch);
    let mut running = Quad::from_msg(&u0, &u2, &t_r);
    let mut r_lane_fold = Vec::with_capacity(initial_k);
    for j in 0..initial_k {
        let bits = fold_bits(0).saturating_sub(j as u32);
        if bits > 0 {
            let nonce = next_fold_nonce();
            ch.verify_pow(bits, nonce);
        }
        let ri = ch.sample_f128();
        r_lane_fold.push(ri);
        t_r = Lc::of(&running.eval(ri).settle(ch.bt.cs));
        let (u0, u2) = next_message(&mut ch);
        running = Quad::from_msg(&u0, &u2, &t_r);
    }

    let root_digest = |ch: &mut PlanChallenger<'_, '_, '_, '_, CS>, index: usize| -> Digest<CS::Bool> {
        let raw = lig.and_then(|p| p.recursive_roots.get(index)).copied().unwrap_or([0u8; 32]);
        let d = merkle::witness_digest(ch.bt.cs, raw);
        ch.observe_committed(&merkle::digest_bytes(&d));
        d
    };
    let root_1 = root_digest(&mut ch, 0);
    stages.mark("L0 folds", [ch.bt.t.duplex.counts.circuit, ch.bt.t.pow.circuit + ch.pow.circuit, counts.merkle.circuit]);

    ch.verify_pow(vc.grinding_bits[0] as u32, lig.map(|p| p.grinding_nonces.first().copied().unwrap_or(0)));
    let num_queries_0 = vc.queries[0];
    let queries_0 = sample_distinct_queries(&mut ch, block_len_0, num_queries_0);
    let alpha_0 = ch.sample_f128_vec(ceil_log2(num_queries_0));
    // Level 0: the occupied lanes committed, the others public zeros.
    if let Some(p) = lig
        && (p.initial_proof.opened_rows.len() != num_queries_0
            || p.initial_proof.opened_rows.iter().any(|row| row.len() != occupied.len()))
    {
        fail("compact initial source opening shape");
    }
    let sib0 = merkle::multi_proof_len(block_len_0, &queries_0);
    if let Some(p) = lig
        && p.initial_proof.merkle_proof.len() != sib0
    {
        fail("initial multi-proof length");
    }
    let (compact0, sibs0) = witness_opened(ch.bt.cs, lig.map(|p| &p.initial_proof), num_queries_0, occupied.len(), sib0);
    let rows0: Vec<Vec<G<CS::Bool>>> = compact0
        .into_iter()
        .map(|compact| {
            let mut row = vec![G::P(Gf::ZERO); lanes];
            for (&lane, x) in occupied.iter().zip(compact) {
                row[lane] = x;
            }
            row
        })
        .collect();
    counts.opened_elements += num_queries_0 * occupied.len();
    counts.siblings += sib0;
    let root_0 = merkle::public_digest(*root);
    let before = counts.merkle.circuit;
    let leaves = verify_opens(ch.bt.cs, &root_0, block_len_0, &queries_0, &rows0, &sibs0, &mut counts.merkle);
    stages.merkle("L0", num_queries_0, lanes, leaves, counts.merkle.circuit - before);

    stages.mark("L0 opening", [ch.bt.t.duplex.counts.circuit, ch.bt.t.pow.circuit + ch.pow.circuit, counts.merkle.circuit]);
    let n1 = log_n - initial_k;
    let enforced_0 = enforced_sum(&rows0, &r_lane_fold, &alpha_0).settle(ch.bt.cs);
    let (i0, i2) = next_message(&mut ch);
    let intro_0 = Quad::from_msg(&i0, &i2, &Lc::of(&enforced_0));
    let beta_0 = ch.sample_f128();
    running = running.fold(&intro_0, beta_0);
    t_r.add(beta_0, &enforced_0);

    struct LevelCtx {
        log_msg_cols: usize,
        queries: Vec<usize>,
        alpha: Vec<Gf>,
        ris_start: usize,
        beta: Gf,
    }
    let mut level_ctxs = vec![LevelCtx { log_msg_cols: n1, queries: queries_0, alpha: alpha_0, ris_start: initial_k, beta: beta_0 }];
    let mut ris: Vec<Gf> = r_lane_fold.clone();
    let mut prev_root = root_1;
    let mut prev_log_num_interleaved = vc.recursive_ks[0];
    let mut prev_log_msg_cols = n1 - prev_log_num_interleaved;
    let mut prev_log_inv_rate = vc.log_inv_rates[1];
    let mut n_current = n1;

    for i in 0..r {
        let k_i = vc.recursive_ks[i];
        assert!(n_current >= k_i);
        let mut level_rs = Vec::with_capacity(k_i);
        for j in 0..k_i {
            let bits = fold_bits(i + 1).saturating_sub(j as u32);
            if bits > 0 {
                let nonce = next_fold_nonce();
                ch.verify_pow(bits, nonce);
            }
            let ri = ch.sample_f128();
            ris.push(ri);
            level_rs.push(ri);
            t_r = Lc::of(&running.eval(ri).settle(ch.bt.cs));
            let (u0, u2) = next_message(&mut ch);
            running = Quad::from_msg(&u0, &u2, &t_r);
        }
        n_current -= k_i;
        let prev_block_len = 1usize << (prev_log_msg_cols + prev_log_inv_rate);
        let prev_num_interleaved = 1usize << prev_log_num_interleaved;

        if i == r - 1 {
            // The compact terminal: the complete final message F (lane-minor),
            // yr = F folded at the level's challenges, observed.
            assert_eq!(prev_log_msg_cols, n_current, "final shape");
            let message_len = (1usize << prev_log_msg_cols) * prev_num_interleaved;
            if let Some(p) = lig
                && p.final_proof.len() != message_len
            {
                fail("compact final message length");
            }
            let message: Vec<G<CS::Bool>> = (0..message_len)
                .map(|k| witness(ch.bt.cs, lig.map_or(Gf::ZERO, |p| p.final_proof.get(k).copied().unwrap_or(Gf::ZERO))))
                .collect();
            let fold = build_eq_table(&level_rs);
            let yr: Vec<G<CS::Bool>> = message
                .chunks(prev_num_interleaved)
                .map(|lanes| {
                    let mut lc = Lc::new();
                    for (e, x) in fold.iter().zip(lanes) {
                        lc.add(*e, x);
                    }
                    lc.settle(ch.bt.cs)
                })
                .collect();
            for y in &yr {
                ch.observe_f128(y);
            }
            ch.verify_pow(vc.grinding_bits[i + 1] as u32, lig.map(|p| p.grinding_nonces.get(i + 1).copied().unwrap_or(0)));
            let num_queries_last = vc.queries[i + 1];
            let _queries_last = sample_distinct_queries(&mut ch, prev_block_len, num_queries_last);
            let _alpha_last = ch.sample_f128_vec(ceil_log2(num_queries_last));
            // The complete message's codeword and tree reproduce the last root
            // (each subtree's codeword rows and hashes independent).
            let matrix = encoding_matrix(prev_log_msg_cols, prev_log_inv_rate);
            let leaf_blocks = std::sync::atomic::AtomicUsize::new(0);
            let leaf = |cs: &mut CS, i: usize, c: &mut ShaCounts| {
                let row: Vec<G<CS::Bool>> = (0..prev_num_interleaved)
                    .map(|lane| {
                        let mut lc = Lc::new();
                        for (col, w) in matrix[i].iter().enumerate() {
                            lc.add(*w, &message[col * prev_num_interleaved + lane]);
                        }
                        // Settled: a hash block reads committed bits.
                        lc.settle(cs)
                    })
                    .collect();
                let before = c.circuit;
                let digest = merkle::leaf_hash(cs, &row, c);
                leaf_blocks.fetch_add(c.circuit - before, std::sync::atomic::Ordering::Relaxed);
                digest
            };
            counts.final_codeword += matrix.len() * prev_num_interleaved;
            let before = counts.merkle.circuit;
            let top = merkle::tree_root_with(ch.bt.cs, matrix.len(), &leaf, &mut counts.merkle);
            merkle::assert_digest_eq(ch.bt.cs, &top, &prev_root);
            let leaves = leaf_blocks.load(std::sync::atomic::Ordering::Relaxed);
            stages.merkle("final level", matrix.len(), prev_num_interleaved, leaves, counts.merkle.circuit - before);
            // Complete-message binding: the last batching draw cancels.
            let _beta_last = ch.sample_f128();

            let yr_log_n = n_current;
            let induced: Vec<Vec<Gf>> = level_ctxs
                .iter()
                .map(|ctx| {
                    let sks_vks = eval_sk_at_vks(ctx.log_msg_cols);
                    let ris_for_basis = &ris[ctx.ris_start..ctx.ris_start + ctx.log_msg_cols - yr_log_n];
                    induce_sumcheck_evaluate_at_residual(ctx.log_msg_cols, &sks_vks, &ctx.queries, &ctx.alpha, ris_for_basis, yr_log_n)
                })
                .collect();
            let evb = eval(&ris, yr_log_n);
            assert_eq!(evb.len(), yr.len());
            let mut inner = Lc::new();
            for (y, value) in yr.iter().enumerate() {
                let mut combined = evb[y];
                for (k, residual) in induced.iter().enumerate() {
                    combined += level_ctxs[k].beta * residual[y];
                }
                inner.add(combined, value);
            }
            inner.add_lc(&t_r);
            inner.assert_zero(ch.bt.cs, &public, "ligerito residual identity");
            if let Some(p) = lig
                && (tx != p.sumcheck_transcript.len() || fold_nonce != p.fold_grinding_nonces.len())
            {
                fail("Ligerito proof trailing data");
            }
            stages.mark("final level", [ch.bt.t.duplex.counts.circuit, ch.bt.t.pow.circuit + ch.pow.circuit, counts.merkle.circuit]);
            break;
        }

        let root_next = root_digest(&mut ch, i + 1);
        ch.verify_pow(vc.grinding_bits[i + 1] as u32, lig.map(|p| p.grinding_nonces.get(i + 1).copied().unwrap_or(0)));
        let num_queries_i = vc.queries[i + 1];
        let queries_i = sample_distinct_queries(&mut ch, prev_block_len, num_queries_i);
        let alpha_i = ch.sample_f128_vec(ceil_log2(num_queries_i));
        let sib = merkle::multi_proof_len(prev_block_len, &queries_i);
        if let Some(p) = lig
            && p.recursive_proofs.get(i).is_none_or(|level| {
                level.merkle_proof.len() != sib
                    || level.opened_rows.len() != num_queries_i
                    || level.opened_rows.iter().any(|row| row.len() != prev_num_interleaved)
            })
        {
            fail("recursive opening shape");
        }
        let (rows, sibs) = witness_opened(ch.bt.cs, lig.and_then(|p| p.recursive_proofs.get(i)), num_queries_i, prev_num_interleaved, sib);
        counts.opened_elements += num_queries_i * prev_num_interleaved;
        counts.siblings += sib;
        let before = counts.merkle.circuit;
        let leaves = verify_opens(ch.bt.cs, &prev_root, prev_block_len, &queries_i, &rows, &sibs, &mut counts.merkle);
        stages.merkle(format!("level {}", i + 1), num_queries_i, prev_num_interleaved, leaves, counts.merkle.circuit - before);
        let enforced_i = enforced_sum(&rows, &level_rs, &alpha_i).settle(ch.bt.cs);
        let (i0, i2) = next_message(&mut ch);
        let intro_i = Quad::from_msg(&i0, &i2, &Lc::of(&enforced_i));
        let beta_i = ch.sample_f128();
        running = running.fold(&intro_i, beta_i);
        t_r.add(beta_i, &enforced_i);
        level_ctxs.push(LevelCtx { log_msg_cols: n_current, queries: queries_i, alpha: alpha_i, ris_start: ris.len(), beta: beta_i });
        stages.mark(format!("level {}", i + 1), [ch.bt.t.duplex.counts.circuit, ch.bt.t.pow.circuit + ch.pow.circuit, counts.merkle.circuit]);
        prev_root = root_next;
        let k_next = vc.recursive_ks[i + 1];
        assert!(n_current >= k_next);
        prev_log_num_interleaved = k_next;
        prev_log_msg_cols = n_current - k_next;
        prev_log_inv_rate = vc.log_inv_rates[i + 2];
    }
    ch.finish();
    stages.print();
    counts.pow = ch.pow;
    drop(ch);
    bt.finish("opening");
    counts
}
