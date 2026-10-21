//! Ligerito's basis opening with every verifier challenge in GF(2^256)
//! ([`Gf256`], the quadratic extension of [`Gf128`]).
//!
//! The protocol is [`super::recursive_prover_with_basis`] /
//! [`super::recursive_verifier_with_basis_succinct`] step for step: the
//! lane folds, the recursive commitments, the out-of-domain samples, the
//! query-induced bases, the glue and the final residual check. What changes
//! is the field the challenges are drawn from: the sumcheck folds, the OOD
//! points, the query batching `α` and the glue `β` live in a 2^256-element
//! field, so every soundness term that divides by the field size (proximity
//! gaps, OOD binding, sumcheck rounds) is computed over it (`field = "f256"`
//! in [`super::LigeritoSecurityConfig`], see [`super::analysis_log_q`]).
//!
//! The level-0 commitment is the unchanged GF(2^128) codeword: its opened
//! rows stay base elements. From the first fold on, the running message and
//! every recursive codeword are extension-valued. An extension vector is a
//! base vector with one more (lowest) lane bit ([`Gf256::as_base_slice`]),
//! and the encoder and Merkle trees act lane by lane, so they apply
//! unchanged.
//!
//! Transcript: an extension element is observed as its two base coordinates
//! (`a` then `b`) and drawn as two consecutive base draws, so the pattern of
//! observations, draws and proof-of-work — which a host's grinding plan is
//! keyed on — is the base protocol's.

use super::*;
use crate::field::Gf256;

type E = Gf256;

/// `(u_0, u_2)` of one sumcheck round, over the extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SumcheckMessageExt {
    pub u_0: E,
    pub u_2: E,
}

/// A recursive level's openings: extension-valued rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecursiveProofExt {
    pub opened_rows: Vec<Vec<E>>,
    pub merkle_proof: Vec<Hash>,
}

/// The final residual `yr` and the last level's openings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalProofExt {
    pub yr: Vec<E>,
    pub opened_rows: Vec<Vec<E>>,
    pub merkle_proof: Vec<Hash>,
}

/// [`LigeritoProof`] with extension-valued messages. `initial_proof` opens
/// the base-field level-0 codeword.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LigeritoProofExt<Final = FinalProofExt> {
    pub initial_root: Hash,
    pub initial_proof: RecursiveProof,
    pub recursive_roots: Vec<Hash>,
    pub recursive_proofs: Vec<RecursiveProofExt>,
    pub final_proof: Final,
    pub sumcheck_transcript: Vec<SumcheckMessageExt>,
    pub grinding_nonces: Vec<u64>,
    pub ood_values: Vec<E>,
    pub fold_grinding_nonces: Vec<u64>,
}

/// [`CompactLigeritoProof`] over the extension: the last level's whole
/// pre-fold message replaces `yr`, its queried rows and their paths.
pub type CompactLigeritoProofExt = LigeritoProofExt<Vec<E>>;

// -------------------------------------------------------------------
// Transcript adapters
// -------------------------------------------------------------------

fn observe<Ch: Challenger>(challenger: &mut Ch, value: E) {
    challenger.observe_f128(value.a);
    challenger.observe_f128(value.b);
}

fn sample<Ch: Challenger>(challenger: &mut Ch) -> E {
    let a = challenger.sample_f128();
    let b = challenger.sample_f128();
    E::new(a, b)
}

fn try_sample<Ch: Challenger>(challenger: &mut Ch) -> Option<E> {
    let a = challenger.try_sample_f128()?;
    let b = challenger.try_sample_f128()?;
    Some(E::new(a, b))
}

/// `n` extension draws as one framed vector of `2n` base draws.
fn sample_vec<Ch: Challenger>(challenger: &mut Ch, n: usize) -> Vec<E> {
    E::from_base_pairs(&challenger.sample_f128_vec(2 * n))
}

fn try_sample_vec<Ch: Challenger>(challenger: &mut Ch, n: usize) -> Option<Vec<E>> {
    Some(E::from_base_pairs(&challenger.try_sample_f128_vec(2 * n)?))
}

// -------------------------------------------------------------------
// Field helpers
// -------------------------------------------------------------------

/// [`build_eq_table`] over the extension: index bit `j` ↔ `point[j]`.
pub fn build_eq_table_ext(point: &[E]) -> Vec<E> {
    let mut out: Vec<E> = Vec::with_capacity(1usize << point.len());
    out.push(E::ONE);
    for &r in point {
        let one_plus_r = E::ONE + r;
        let len = out.len();
        out.resize(2 * len, E::ZERO);
        for i in 0..len {
            let v = out[i];
            out[i + len] = v * r;
            out[i] = v * one_plus_r;
        }
    }
    out
}

/// One quadratic `c + b·X + a·X²` of the verifier's running sumcheck.
#[derive(Clone, Copy, Debug)]
struct RoundQuadExt {
    c: E,
    b: E,
    a: E,
}

impl RoundQuadExt {
    fn from_msg(msg: SumcheckMessageExt, t_r: E) -> Self {
        Self {
            c: msg.u_0,
            b: t_r + msg.u_2,
            a: msg.u_2,
        }
    }
    fn eval(&self, r: E) -> E {
        self.c + r * self.b + r * r * self.a
    }
    fn fold(p1: &Self, p2: &Self, alpha: E) -> Self {
        Self {
            c: p1.c + alpha * p2.c,
            b: p1.b + alpha * p2.b,
            a: p1.a + alpha * p2.a,
        }
    }
}

const PAR_THRESHOLD: usize = 4096;

/// `(u_0, u_2)` for `Σ_x f(X, x)·b(X, x)` in the lowest variable `X`, and
/// (on request) the full inner product `Σ f·b`.
fn round_msg(f: &[E], b: &[E], with_sum: bool) -> (SumcheckMessageExt, E) {
    use rayon::prelude::*;
    debug_assert_eq!(f.len(), b.len());
    let half = f.len() / 2;
    let term = |j: usize| -> (E, E, E) {
        let (f0, f1, b0, b1) = (f[2 * j], f[2 * j + 1], b[2 * j], b[2 * j + 1]);
        let e0 = f0 * b0;
        let y = if with_sum { e0 + f1 * b1 } else { E::ZERO };
        (e0, (f0 + f1) * (b0 + b1), y)
    };
    let add = |(a0, a2, ay): (E, E, E), (b0, b2, by): (E, E, E)| (a0 + b0, a2 + b2, ay + by);
    let (u_0, u_2, y) = if half < PAR_THRESHOLD {
        (0..half).map(term).fold((E::ZERO, E::ZERO, E::ZERO), add)
    } else {
        (0..half)
            .into_par_iter()
            .with_min_len(PAR_THRESHOLD / 4)
            .map(term)
            .reduce(|| (E::ZERO, E::ZERO, E::ZERO), add)
    };
    (SumcheckMessageExt { u_0, u_2 }, y)
}

/// Bind the lowest variable of `v` to `r`: `v'[j] = v[2j] + r·(v[2j] + v[2j+1])`.
fn fold_lsb(v: &[E], r: E) -> Vec<E> {
    use rayon::prelude::*;
    let half = v.len() / 2;
    let one = |j: usize| v[2 * j] + r * (v[2 * j] + v[2 * j + 1]);
    if half < PAR_THRESHOLD {
        (0..half).map(one).collect()
    } else {
        (0..half).into_par_iter().with_min_len(PAR_THRESHOLD / 4).map(one).collect()
    }
}

/// The stateful sumcheck of [`SumcheckProver`] over the extension.
struct SumcheckProverExt {
    f: Vec<E>,
    basis: Vec<E>,
    t_r: E,
    transcript: Vec<SumcheckMessageExt>,
    pending_glue: Option<(Vec<E>, E)>,
}

impl SumcheckProverExt {
    fn new(f: Vec<E>, b: Vec<E>, h: E) -> (Self, SumcheckMessageExt) {
        assert_eq!(f.len(), b.len());
        let (msg, _) = round_msg(&f, &b, false);
        let inst = Self {
            f,
            basis: b,
            t_r: h,
            transcript: vec![msg],
            pending_glue: None,
        };
        (inst, msg)
    }

    fn fold(&mut self, r: E) -> SumcheckMessageExt {
        self.f = fold_lsb(&self.f, r);
        self.basis = fold_lsb(&self.basis, r);
        let (msg, _) = round_msg(&self.f, &self.basis, false);
        self.transcript.push(msg);
        msg
    }

    fn introduce_new(&mut self, b_new: Vec<E>, h_new: E) -> SumcheckMessageExt {
        assert_eq!(b_new.len(), self.f.len());
        let (msg, _) = round_msg(&self.f, &b_new, false);
        self.transcript.push(msg);
        self.pending_glue = Some((b_new, h_new));
        msg
    }

    fn introduce_new_with_eval(&mut self, b_new: Vec<E>) -> (SumcheckMessageExt, E) {
        assert_eq!(b_new.len(), self.f.len());
        let (msg, h_new) = round_msg(&self.f, &b_new, true);
        self.transcript.push(msg);
        self.pending_glue = Some((b_new, h_new));
        (msg, h_new)
    }

    fn glue(&mut self, beta: E) {
        use rayon::prelude::*;
        let (b_new, h_new) = self.pending_glue.take().expect("glue without introduce");
        assert_eq!(b_new.len(), self.basis.len());
        if self.basis.len() < PAR_THRESHOLD {
            for (acc, &v) in self.basis.iter_mut().zip(b_new.iter()) {
                *acc += beta * v;
            }
        } else {
            self.basis
                .par_iter_mut()
                .zip(b_new.par_iter())
                .with_min_len(PAR_THRESHOLD / 4)
                .for_each(|(acc, &v)| *acc += beta * v);
        }
        self.t_r += beta * h_new;
    }

    fn f(&self) -> &[E] {
        &self.f
    }

    fn finish(self) -> (Vec<E>, Vec<SumcheckMessageExt>) {
        (self.f, self.transcript)
    }
}

/// Commit an extension message: the base encoder on its coordinate view,
/// one more lane bit (an extension lane is two base lanes).
fn ligero_commit_ext(
    poly: &[E],
    log_msg_cols: usize,
    log_num_interleaved: usize,
    log_inv_rate: usize,
    ntt: &AdditiveNttF128,
    kind: HashKind,
) -> LigeroWitness {
    ligero_commit(
        E::as_base_slice(poly),
        log_msg_cols,
        log_num_interleaved + 1,
        log_inv_rate,
        ntt,
        kind,
    )
}

/// Row `q` of an extension witness committed by [`ligero_commit_ext`].
fn row_ext(witness: &LigeroWitness, q: usize) -> Vec<E> {
    E::from_base_pairs(witness.row(q))
}

/// [`verify_level_opens`] for extension rows of `lanes` elements.
fn verify_level_opens_ext(
    root: &Hash,
    block_len: usize,
    queries: &[usize],
    opened_rows: &[Vec<E>],
    lanes: usize,
    multi_proof: &[Hash],
    kind: HashKind,
) -> bool {
    if opened_rows.iter().any(|row| row.len() != lanes) {
        return false;
    }
    let base: Vec<Vec<Gf128>> = opened_rows
        .iter()
        .map(|row| E::as_base_slice(row).to_vec())
        .collect();
    verify_level_opens(root, block_len, queries, &base, 2 * lanes, multi_proof, kind)
}

/// The base protocol's [`FinalBinding`] over the extension.
enum FinalBindingExt {
    CompleteMessage,
    QuerySum(E),
}

/// [`FinalEncoding`] over the extension: the terminal either opens queried
/// rows of the last commitment ([`FinalProofExt`]) or sends its whole
/// pre-fold message (`Vec<E>`), which the verifier re-encodes and checks
/// against the last root. `shape` counts extension lanes; the base encoder
/// sees one more lane bit.
trait FinalEncodingExt: Sized {
    type Captured;

    fn capture(message: &[E]) -> Self::Captured;
    fn finish(captured: Self::Captured, yr: Vec<E>, witness: &LigeroWitness, queries: &[usize])
    -> Self;
    fn polynomial(&self, shape: FinalShape, folds: &[E]) -> Option<Cow<'_, [E]>>;
    fn authenticate(
        &self,
        shape: FinalShape,
        root: &Hash,
        queries: &[usize],
        folds: &[E],
        alpha: &[E],
    ) -> Option<FinalBindingExt>;
}

impl FinalEncodingExt for FinalProofExt {
    type Captured = ();

    fn capture(_: &[E]) {}

    fn finish(_: (), yr: Vec<E>, witness: &LigeroWitness, queries: &[usize]) -> Self {
        Self {
            yr,
            opened_rows: queries.iter().map(|&q| row_ext(witness, q)).collect(),
            merkle_proof: merkle_multi_proof_for(&witness.tree, witness.block_len, queries),
        }
    }

    fn polynomial(&self, shape: FinalShape, folds: &[E]) -> Option<Cow<'_, [E]>> {
        let (columns, _, _) = shape.dimensions()?;
        (self.yr.len() == columns && folds.len() == shape.log_num_interleaved)
            .then(|| Cow::Borrowed(self.yr.as_slice()))
    }

    fn authenticate(
        &self,
        shape: FinalShape,
        root: &Hash,
        queries: &[usize],
        folds: &[E],
        alpha: &[E],
    ) -> Option<FinalBindingExt> {
        let (_, lanes, rows) = shape.dimensions()?;
        (self.opened_rows.len() == queries.len()
            && verify_level_opens_ext(
                root,
                rows,
                queries,
                &self.opened_rows,
                lanes,
                &self.merkle_proof,
                shape.hash,
            ))
        .then(|| {
            FinalBindingExt::QuerySum(enforced_sum_ext(&self.opened_rows, folds, queries, alpha))
        })
    }
}

/// The base encoder's view of an extension terminal: one more lane bit.
fn base_shape(shape: FinalShape) -> Option<FinalShape> {
    Some(FinalShape {
        log_num_interleaved: shape.log_num_interleaved.checked_add(1)?,
        ..shape
    })
}

impl FinalEncodingExt for Vec<E> {
    type Captured = Self;

    fn capture(message: &[E]) -> Self {
        message.to_vec()
    }

    fn finish(captured: Self, _: Vec<E>, _: &LigeroWitness, _: &[usize]) -> Self {
        captured
    }

    fn polynomial(&self, shape: FinalShape, folds: &[E]) -> Option<Cow<'_, [E]>> {
        // The base bound on the encoded table, at twice the lanes.
        base_shape(shape)?.compact_message_len()?;
        let (columns, lanes, _) = shape.dimensions()?;
        if self.len() != columns.checked_mul(lanes)? || folds.len() != shape.log_num_interleaved {
            return None;
        }
        let mut message = self.clone();
        for &r in folds {
            message = fold_lsb(&message, r);
        }
        Some(Cow::Owned(message))
    }

    fn authenticate(
        &self,
        shape: FinalShape,
        root: &Hash,
        queries: &[usize],
        _: &[E],
        _: &[E],
    ) -> Option<FinalBindingExt> {
        let base = base_shape(shape)?;
        if 2 * self.len() != base.compact_message_len()? {
            return None;
        }
        let (_, _, rows) = shape.dimensions()?;
        if queries.iter().any(|&q| q >= rows) {
            return None;
        }
        let ntt = compact_terminal_ntt(shape.log_msg_cols.checked_add(shape.log_inv_rate)?)?;
        let witness = ligero_commit_ext(
            self,
            shape.log_msg_cols,
            shape.log_num_interleaved,
            shape.log_inv_rate,
            ntt,
            shape.hash,
        );
        (witness.root() == *root).then_some(FinalBindingExt::CompleteMessage)
    }
}

/// An opened row's entries, base (level 0) or extension (recursive levels).
trait RowEntry: Copy + Send + Sync {
    fn times(self, e: E) -> E;
}

impl RowEntry for Gf128 {
    fn times(self, e: E) -> E {
        e.mul_base(self)
    }
}

impl RowEntry for E {
    fn times(self, e: E) -> E {
        self * e
    }
}

/// The per-query weights `eq(α, i)` for the first `n_queries` indices.
fn alpha_weights(alpha: &[E], n_queries: usize) -> Vec<E> {
    if n_queries == 0 {
        return Vec::new();
    }
    let table = build_eq_table_ext(alpha);
    debug_assert!(table.len() >= n_queries);
    table.into_iter().take(n_queries).collect()
}

fn inverse_sks_vks(sks_vks: &[Gf128]) -> Vec<Gf128> {
    sks_vks
        .iter()
        .map(|&v| if v.is_zero() { Gf128::ZERO } else { v.inverse_or_zero() })
        .collect()
}

/// `Σ_i eq(α, i)·⟨row_i, eq(v, ·)⟩` — the claimed sum of an induced basis
/// ([`induce_sumcheck_enforced_sum`] over the extension).
fn enforced_sum_ext<R: RowEntry>(
    opened_rows: &[Vec<R>],
    v_challenges: &[E],
    queries: &[usize],
    alpha: &[E],
) -> E {
    assert_eq!(opened_rows.len(), queries.len());
    let eq = build_eq_table_ext(v_challenges);
    let weights = alpha_weights(alpha, queries.len());
    let mut sum = E::ZERO;
    for (row, &w) in opened_rows.iter().zip(weights.iter()) {
        debug_assert_eq!(row.len(), eq.len());
        let dot = row
            .iter()
            .zip(eq.iter())
            .fold(E::ZERO, |acc, (&x, &e)| acc + x.times(e));
        sum += w * dot;
    }
    sum
}

/// [`induce_sumcheck_poly`] over the extension: the basis
/// `Σ_i eq(α, i)·X̂(q_i)` (the novel-basis vector at each query's field
/// point is base-valued, so it accumulates into the two coordinates with
/// base multiplications) and its claimed sum.
fn induce_sumcheck_poly_ext<R: RowEntry>(
    log_msg_cols: usize,
    sks_vks: &[Gf128],
    opened_rows: &[Vec<R>],
    v_challenges: &[E],
    queries: &[usize],
    alpha: &[E],
) -> (Vec<E>, E) {
    use rayon::prelude::*;
    let n = 1usize << log_msg_cols;
    let n_queries = queries.len();
    assert_eq!(opened_rows.len(), n_queries);
    let enforced = enforced_sum_ext(opened_rows, v_challenges, queries, alpha);
    let weights = alpha_weights(alpha, n_queries);
    let inv_sks_vks = inverse_sks_vks(sks_vks);
    let threads = rayon::current_num_threads().max(1);
    let chunk = n_queries.div_ceil(threads).max(1);
    let partials: Vec<(Vec<Gf128>, Vec<Gf128>)> = (0..n_queries)
        .collect::<Vec<_>>()
        .par_chunks(chunk)
        .map(|indices| {
            let mut acc_a = vec![Gf128::ZERO; n];
            let mut acc_b = vec![Gf128::ZERO; n];
            let mut local = vec![Gf128::ZERO; n];
            let mut sks_at_x = vec![Gf128::ZERO; log_msg_cols.max(1)];
            for &i in indices {
                let q_field = Gf128::new(queries[i] as u64, 0);
                evaluate_scaled_basis_inplace(
                    &mut sks_at_x,
                    &mut local,
                    sks_vks,
                    &inv_sks_vks,
                    q_field,
                    Gf128::ONE,
                );
                let w = weights[i];
                for ((a, b), &v) in acc_a.iter_mut().zip(acc_b.iter_mut()).zip(local.iter()) {
                    *a += w.a * v;
                    *b += w.b * v;
                }
            }
            (acc_a, acc_b)
        })
        .collect();
    let mut basis = vec![E::ZERO; n];
    for (pa, pb) in partials {
        for ((slot, &a), &b) in basis.iter_mut().zip(pa.iter()).zip(pb.iter()) {
            *slot += E::new(a, b);
        }
    }
    (basis, enforced)
}

/// [`induce_sumcheck_evaluate_at_residual`] over the extension: the
/// induced basis's MLE at `ris_for_basis ‖ y` for every `y ∈ {0,1}^yr_log_n`.
fn evaluate_at_residual_ext(
    log_msg_cols: usize,
    sks_vks: &[Gf128],
    queries: &[usize],
    alpha: &[E],
    ris_for_basis: &[E],
    yr_log_n: usize,
) -> Vec<E> {
    assert_eq!(ris_for_basis.len() + yr_log_n, log_msg_cols);
    let weights = alpha_weights(alpha, queries.len());
    let inv_sks_vks = inverse_sks_vks(sks_vks);
    let prefix_len = ris_for_basis.len();
    // Per query: Ŵ_k(q) for every k, the prefix product over the bound
    // variables, and the suffix Ŵ values the residual positions need.
    let per_query: Vec<(E, Vec<Gf128>)> = queries
        .iter()
        .map(|&q| {
            let mut sks_at_x = Vec::with_capacity(log_msg_cols.max(1));
            if log_msg_cols > 0 {
                sks_at_x.push(Gf128::new(q as u64, 0));
                for k in 1..log_msg_cols {
                    sks_at_x.push(next_s(sks_at_x[k - 1], sks_vks[k - 1]));
                }
                for k in 0..log_msg_cols {
                    sks_at_x[k] *= inv_sks_vks[k];
                }
            }
            let mut prefix = E::ONE;
            for k in 0..prefix_len {
                prefix *= E::ONE + ris_for_basis[k].mul_base(Gf128::ONE + sks_at_x[k]);
            }
            let suffix = sks_at_x.get(prefix_len..).map(<[_]>::to_vec).unwrap_or_default();
            (prefix, suffix)
        })
        .collect();
    (0..1usize << yr_log_n)
        .map(|y| {
            let mut sum = E::ZERO;
            for (i, (prefix, suffix)) in per_query.iter().enumerate() {
                // Each suffix factor is `1` (bit 0) or `Ŵ_k(q)` (bit 1).
                let mut s = Gf128::ONE;
                for (j, &w) in suffix.iter().enumerate().take(yr_log_n) {
                    if (y >> j) & 1 == 1 {
                        s *= w;
                    }
                }
                sum += (weights[i] * *prefix).mul_base(s);
            }
            sum
        })
        .collect()
}

// -------------------------------------------------------------------
// Prover
// -------------------------------------------------------------------

/// Prove `Σ_x f(x)·b(x) = target` for the base-field packed witness `f`
/// committed as `(l0_codeword, l0_tree)`, with an extension-valued basis
/// `b` and claim. The extension counterpart of
/// [`super::recursive_prover_with_basis`].
#[allow(clippy::too_many_arguments)]
pub fn recursive_prover_with_basis_ext<Ch: Challenger>(
    config: &ProverConfig,
    packed_witness: &[Gf128],
    b_initial: Vec<E>,
    target: E,
    l0_codeword: &[Gf128],
    l0_tree: &[Hash],
    challenger: &mut Ch,
) -> LigeritoProofExt {
    let log_n = packed_witness.len().trailing_zeros() as usize;
    let block_len_0 = 1usize << (log_n - config.initial_k + config.log_inv_rates[0]);
    let num_interleaved_0 = 1usize << config.initial_k;
    let initial_root = *l0_tree.last().expect("nonempty initial Merkle tree");
    assert_eq!(l0_codeword.len(), block_len_0 * num_interleaved_0);
    assert_eq!(l0_tree.len(), 2 * block_len_0 - 1);
    prover_impl(
        config,
        packed_witness,
        b_initial,
        target,
        initial_root,
        |block_len, lanes, queries: &[usize]| RecursiveProof {
            opened_rows: queries
                .iter()
                .map(|&q| l0_codeword[q * lanes..(q + 1) * lanes].to_vec())
                .collect(),
            merkle_proof: merkle_multi_proof_for(l0_tree, block_len, queries),
        },
        challenger,
    )
}

/// [`recursive_prover_with_basis_ext`] against an externally authenticated
/// level-0 oracle, with the compact terminal: the extension counterpart of
/// [`super::recursive_prover_with_basis_initial_compact`]. `initial_root`
/// is the oracle's binding identifier; `open_initial` receives (number of
/// positions, lane count, query indices) and the matching verifier must
/// authenticate every returned row with that geometry and identifier.
#[allow(clippy::too_many_arguments)]
pub fn recursive_prover_with_basis_initial_compact_ext<Ch, O>(
    config: &ProverConfig,
    packed_witness: &[Gf128],
    b_initial: Vec<E>,
    target: E,
    initial_root: Hash,
    open_initial: O,
    challenger: &mut Ch,
) -> CompactLigeritoProofExt
where
    Ch: Challenger,
    O: FnOnce(usize, usize, &[usize]) -> RecursiveProof,
{
    prover_impl(config, packed_witness, b_initial, target, initial_root, open_initial, challenger)
}

#[allow(clippy::too_many_arguments)]
fn prover_impl<Ch, O, Final>(
    config: &ProverConfig,
    packed_witness: &[Gf128],
    b_initial: Vec<E>,
    target: E,
    initial_root: Hash,
    open_initial: O,
    challenger: &mut Ch,
) -> LigeritoProofExt<Final>
where
    Ch: Challenger,
    O: FnOnce(usize, usize, &[usize]) -> RecursiveProof,
    Final: FinalEncodingExt,
{
    let log_n = packed_witness.len().trailing_zeros() as usize;
    let r = config.recursive_steps;
    let initial_k = config.initial_k;
    assert_eq!(packed_witness.len(), 1usize << log_n);
    assert_eq!(b_initial.len(), 1usize << log_n);
    assert_eq!(config.recursive_ks.len(), r);
    assert_eq!(config.log_inv_rates.len(), r + 1);
    assert!(r >= 1);
    assert_eq!(
        config.ood_samples.first().copied().unwrap_or(0),
        0,
        "L0 must not take explicit OOD samples"
    );

    let log_inv_rate_0 = config.log_inv_rates[0];
    let block_len_0 = 1usize << (log_n - initial_k + log_inv_rate_0);
    let num_interleaved_0 = 1usize << initial_k;

    challenger.observe_label(b"flock-ligerito-basis-ext-v0");
    observe(challenger, target);
    challenger.observe_bytes(&initial_root);

    let fold_bits =
        |lvl: usize| -> u32 { config.fold_grinding_bits.get(lvl).copied().unwrap_or(0) as u32 };
    let ood_count = |lvl: usize| -> usize { config.ood_samples.get(lvl).copied().unwrap_or(0) };
    let mut ood_values: Vec<E> = Vec::new();
    let mut fold_grinding_nonces: Vec<u64> = Vec::new();

    let f0: Vec<E> = packed_witness.iter().map(|&v| E::from_base(v)).collect();
    let (mut sc, start_msg) = SumcheckProverExt::new(f0, b_initial, target);
    observe(challenger, start_msg.u_0);
    observe(challenger, start_msg.u_2);

    let mut r_lane_fold = Vec::with_capacity(initial_k);
    for j in 0..initial_k {
        let bits = fold_bits(0).saturating_sub(j as u32);
        if bits > 0 {
            fold_grinding_nonces.push(challenger.grind_pow(bits));
        }
        let rj = sample(challenger);
        let msg = sc.fold(rj);
        observe(challenger, msg.u_0);
        observe(challenger, msg.u_2);
        r_lane_fold.push(rj);
    }

    // Commit f¹.
    let n1 = log_n - initial_k;
    let log_num_interleaved_1 = config.recursive_ks[0];
    assert!(n1 >= log_num_interleaved_1);
    let log_msg_cols_1 = n1 - log_num_interleaved_1;
    let log_inv_rate_1 = config.log_inv_rates[1];
    let ntt_1 = AdditiveNttF128::standard(log_msg_cols_1 + log_inv_rate_1);
    let wtns_1 = ligero_commit_ext(
        sc.f(),
        log_msg_cols_1,
        log_num_interleaved_1,
        log_inv_rate_1,
        &ntt_1,
        config.merkle_hash,
    );
    challenger.observe_bytes(&wtns_1.root());

    for _ in 0..ood_count(1) {
        let z = sample_vec(challenger, n1);
        let (intro, y) = sc.introduce_new_with_eval(build_eq_table_ext(&z));
        observe(challenger, y);
        ood_values.push(y);
        observe(challenger, intro.u_0);
        observe(challenger, intro.u_2);
        let beta = sample(challenger);
        sc.glue(beta);
    }

    let mut grinding_nonces = vec![challenger.grind_pow(config.grinding_bits[0] as u32)];
    let num_queries_0 = config.queries[0];
    let queries_0 = sample_distinct_queries(challenger, block_len_0, num_queries_0);
    let alpha_0 = sample_vec(challenger, ceil_log2(num_queries_0));
    let initial_proof = open_initial(block_len_0, num_interleaved_0, &queries_0);
    assert_eq!(initial_proof.opened_rows.len(), queries_0.len());
    assert!(initial_proof.opened_rows.iter().all(|row| row.len() == num_interleaved_0));

    let sks_vks_n1 = eval_sk_at_vks(n1);
    let (basis_0, enforced_0) = induce_sumcheck_poly_ext(
        n1,
        &sks_vks_n1,
        &initial_proof.opened_rows,
        &r_lane_fold,
        &queries_0,
        &alpha_0,
    );
    let intro_0 = sc.introduce_new(basis_0, enforced_0);
    observe(challenger, intro_0.u_0);
    observe(challenger, intro_0.u_2);
    let beta_0 = sample(challenger);
    sc.glue(beta_0);

    let mut wtns_prev = wtns_1;
    let mut recursive_roots: Vec<Hash> = vec![wtns_prev.root()];
    let mut recursive_proofs: Vec<RecursiveProofExt> = Vec::new();

    for i in 0..r {
        let k_i = config.recursive_ks[i];
        let final_message = (i == r - 1).then(|| Final::capture(sc.f()));
        let mut level_rs = Vec::with_capacity(k_i);
        for j in 0..k_i {
            let bits = fold_bits(i + 1).saturating_sub(j as u32);
            if bits > 0 {
                fold_grinding_nonces.push(challenger.grind_pow(bits));
            }
            let ri = sample(challenger);
            let msg = sc.fold(ri);
            observe(challenger, msg.u_0);
            observe(challenger, msg.u_2);
            level_rs.push(ri);
        }

        if i == r - 1 {
            let (yr, sumcheck_transcript) = sc.finish();
            for &v in &yr {
                observe(challenger, v);
            }
            grinding_nonces.push(challenger.grind_pow(config.grinding_bits[i + 1] as u32));
            let queries_last =
                sample_distinct_queries(challenger, wtns_prev.block_len, config.queries[i + 1]);
            let final_proof = Final::finish(
                final_message.expect("last iteration captures its message"),
                yr,
                &wtns_prev,
                &queries_last,
            );
            return LigeritoProofExt {
                initial_root,
                initial_proof,
                recursive_roots,
                recursive_proofs,
                final_proof,
                sumcheck_transcript,
                grinding_nonces,
                ood_values,
                fold_grinding_nonces,
            };
        }

        let n_next = sc.f().len().trailing_zeros() as usize;
        let log_num_interleaved_next = config.recursive_ks[i + 1];
        assert!(n_next >= log_num_interleaved_next);
        let log_msg_cols_next = n_next - log_num_interleaved_next;
        let log_inv_rate_next = config.log_inv_rates[i + 2];
        let ntt_next = AdditiveNttF128::standard(log_msg_cols_next + log_inv_rate_next);
        let wtns_next = ligero_commit_ext(
            sc.f(),
            log_msg_cols_next,
            log_num_interleaved_next,
            log_inv_rate_next,
            &ntt_next,
            config.merkle_hash,
        );
        let root_next = wtns_next.root();
        challenger.observe_bytes(&root_next);
        recursive_roots.push(root_next);

        for _ in 0..ood_count(i + 2) {
            let z = sample_vec(challenger, n_next);
            let (intro, y) = sc.introduce_new_with_eval(build_eq_table_ext(&z));
            observe(challenger, y);
            ood_values.push(y);
            observe(challenger, intro.u_0);
            observe(challenger, intro.u_2);
            let beta = sample(challenger);
            sc.glue(beta);
        }

        grinding_nonces.push(challenger.grind_pow(config.grinding_bits[i + 1] as u32));
        let num_queries_i = config.queries[i + 1];
        let queries_i = sample_distinct_queries(challenger, wtns_prev.block_len, num_queries_i);
        let alpha_i = sample_vec(challenger, ceil_log2(num_queries_i));
        let opened_rows_i: Vec<Vec<E>> = queries_i.iter().map(|&q| row_ext(&wtns_prev, q)).collect();
        let merkle_proof_i =
            merkle_multi_proof_for(&wtns_prev.tree, wtns_prev.block_len, &queries_i);
        let sks_vks_i = eval_sk_at_vks(n_next);
        let (basis_i, enforced_i) = induce_sumcheck_poly_ext(
            n_next,
            &sks_vks_i,
            &opened_rows_i,
            &level_rs,
            &queries_i,
            &alpha_i,
        );
        recursive_proofs.push(RecursiveProofExt {
            opened_rows: opened_rows_i,
            merkle_proof: merkle_proof_i,
        });
        let intro_i = sc.introduce_new(basis_i, enforced_i);
        observe(challenger, intro_i.u_0);
        observe(challenger, intro_i.u_2);
        let beta_i = sample(challenger);
        sc.glue(beta_i);

        wtns_prev = wtns_next;
    }
    unreachable!()
}

// -------------------------------------------------------------------
// Verifier
// -------------------------------------------------------------------

/// Succinct verifier for [`recursive_prover_with_basis_ext`]:
/// `eval_b_residual(ris, yr_log_n)` returns the basis's MLE at
/// `ris ‖ y` for every `y ∈ {0,1}^yr_log_n`. The extension counterpart of
/// [`super::recursive_verifier_with_basis_succinct`].
#[allow(clippy::too_many_arguments)]
pub fn recursive_verifier_with_basis_succinct_ext<Ch, F>(
    config: &VerifierConfig,
    proof: &LigeritoProofExt,
    log_n: usize,
    target: E,
    expected_initial_root: &Hash,
    eval_b_residual: F,
    challenger: &mut Ch,
) -> bool
where
    Ch: Challenger,
    F: Fn(&[E], usize) -> Vec<E>,
{
    verifier_impl(
        config,
        proof,
        &proof.initial_proof,
        log_n,
        target,
        expected_initial_root,
        eval_b_residual,
        |block_len, lanes, queries, opening| {
            verify_level_opens(
                expected_initial_root,
                block_len,
                queries,
                &opening.opened_rows,
                lanes,
                &opening.merkle_proof,
                config.merkle_hash,
            )
        },
        challenger,
    )
}

/// Verifier for [`recursive_prover_with_basis_initial_compact_ext`]: the
/// extension counterpart of
/// [`super::recursive_verifier_with_basis_initial_compact`].
/// `initial_opening` holds the canonical full level-0 rows reconstructed by
/// the caller's codec; `authenticate_initial` must bind every entry.
#[allow(clippy::too_many_arguments)]
pub fn recursive_verifier_with_basis_initial_compact_ext<Ch, F, A>(
    config: &VerifierConfig,
    proof: &CompactLigeritoProofExt,
    initial_opening: &RecursiveProof,
    log_n: usize,
    target: E,
    expected_initial_root: &Hash,
    eval_b_residual: F,
    authenticate_initial: A,
    challenger: &mut Ch,
) -> bool
where
    Ch: Challenger,
    F: Fn(&[E], usize) -> Vec<E>,
    A: FnOnce(usize, usize, &[usize], &RecursiveProof) -> bool,
{
    let r = config.recursive_steps;
    if r == 0
        || config.recursive_ks.len() != r
        || config.log_inv_rates.len() != r + 1
        || proof.recursive_roots.len() != r
        || proof.recursive_proofs.len() != r - 1
        || proof.grinding_nonces.len() != r + 1
    {
        return false;
    }
    let Some(remaining) = log_n
        .checked_sub(config.initial_k)
        .and_then(|n| config.recursive_ks.iter().try_fold(n, |n, &k| n.checked_sub(k)))
    else {
        return false;
    };
    let shape = FinalShape {
        log_msg_cols: remaining,
        log_num_interleaved: config.recursive_ks[r - 1],
        log_inv_rate: config.log_inv_rates[r],
        hash: config.merkle_hash,
    };
    if base_shape(shape).and_then(FinalShape::compact_message_len) != Some(2 * proof.final_proof.len())
    {
        return false;
    }
    verifier_impl(
        config,
        proof,
        initial_opening,
        log_n,
        target,
        expected_initial_root,
        eval_b_residual,
        authenticate_initial,
        challenger,
    )
}

#[allow(clippy::too_many_arguments)]
fn verifier_impl<Ch, F, A, Final>(
    config: &VerifierConfig,
    proof: &LigeritoProofExt<Final>,
    initial_opening: &RecursiveProof,
    log_n: usize,
    target: E,
    expected_initial_root: &Hash,
    eval_b_residual: F,
    authenticate_initial: A,
    challenger: &mut Ch,
) -> bool
where
    Ch: Challenger,
    F: Fn(&[E], usize) -> Vec<E>,
    A: FnOnce(usize, usize, &[usize], &RecursiveProof) -> bool,
    Final: FinalEncodingExt,
{
    macro_rules! challenge {
        ($draw:expr) => {
            match $draw {
                Some(value) => value,
                None => return false,
            }
        };
    }
    macro_rules! next_msg {
        ($tx:ident) => {{
            if $tx >= proof.sumcheck_transcript.len() {
                return false;
            }
            $tx += 1;
            proof.sumcheck_transcript[$tx - 1]
        }};
    }

    let initial_k = config.initial_k;
    let r = config.recursive_steps;
    if r < 1
        || config.recursive_ks.len() != r
        || config.log_inv_rates.len() != r + 1
        || log_n < initial_k
        || &proof.initial_root != expected_initial_root
        || config.ood_samples.first().copied().unwrap_or(0) != 0
    {
        return false;
    }

    challenger.observe_label(b"flock-ligerito-basis-ext-v0");
    observe(challenger, target);
    challenger.observe_bytes(&proof.initial_root);

    let log_inv_rate_0 = config.log_inv_rates[0];
    let block_len_0 = 1usize << (log_n - initial_k + log_inv_rate_0);
    let num_interleaved_0 = 1usize << initial_k;

    let mut t_r = target;
    let mut tx = 0usize;
    let start_msg = next_msg!(tx);
    observe(challenger, start_msg.u_0);
    observe(challenger, start_msg.u_2);
    let mut running = RoundQuadExt::from_msg(start_msg, t_r);

    let fold_bits =
        |lvl: usize| -> u32 { config.fold_grinding_bits.get(lvl).copied().unwrap_or(0) as u32 };
    let ood_count = |lvl: usize| -> usize { config.ood_samples.get(lvl).copied().unwrap_or(0) };
    let mut fold_nonce = 0usize;
    let mut ood_idx = 0usize;
    struct OodCtx {
        z: Vec<E>,
        ris_start: usize,
        beta: E,
    }
    let mut ood_ctxs: Vec<OodCtx> = Vec::new();

    macro_rules! fold_pow {
        ($bits:expr) => {{
            let bits = $bits;
            if bits > 0 {
                if fold_nonce >= proof.fold_grinding_nonces.len()
                    || !challenger.verify_pow(proof.fold_grinding_nonces[fold_nonce], bits)
                {
                    return false;
                }
                fold_nonce += 1;
            }
        }};
    }

    let mut r_lane_fold = Vec::with_capacity(initial_k);
    for j in 0..initial_k {
        fold_pow!(fold_bits(0).saturating_sub(j as u32));
        let rj = challenge!(try_sample(challenger));
        r_lane_fold.push(rj);
        t_r = running.eval(rj);
        let msg = next_msg!(tx);
        observe(challenger, msg.u_0);
        observe(challenger, msg.u_2);
        running = RoundQuadExt::from_msg(msg, t_r);
    }

    let Some(&root_1) = proof.recursive_roots.first() else {
        return false;
    };
    challenger.observe_bytes(&root_1);

    macro_rules! ood_round {
        ($dim:expr, $ris_start:expr) => {{
            let z = challenge!(try_sample_vec(challenger, $dim));
            if ood_idx >= proof.ood_values.len() {
                return false;
            }
            let y = proof.ood_values[ood_idx];
            ood_idx += 1;
            observe(challenger, y);
            let intro = next_msg!(tx);
            observe(challenger, intro.u_0);
            observe(challenger, intro.u_2);
            let intro_quad = RoundQuadExt::from_msg(intro, y);
            let beta = challenge!(try_sample(challenger));
            running = RoundQuadExt::fold(&running, &intro_quad, beta);
            t_r += beta * y;
            ood_ctxs.push(OodCtx {
                z,
                ris_start: $ris_start,
                beta,
            });
        }};
    }

    let n1 = log_n - initial_k;
    for _ in 0..ood_count(1) {
        ood_round!(n1, initial_k);
    }

    let mut nonce_idx = 0usize;
    if proof.grinding_nonces.is_empty()
        || !challenger.verify_pow(proof.grinding_nonces[0], config.grinding_bits[0] as u32)
    {
        return false;
    }
    nonce_idx += 1;

    let num_queries_0 = config.queries[0];
    let queries_0 = challenge!(try_sample_distinct_queries(challenger, block_len_0, num_queries_0));
    let alpha_0 = challenge!(try_sample_vec(challenger, ceil_log2(num_queries_0)));
    if initial_opening.opened_rows.len() != queries_0.len()
        || initial_opening
            .opened_rows
            .iter()
            .any(|row| row.len() != num_interleaved_0)
        || !authenticate_initial(block_len_0, num_interleaved_0, &queries_0, initial_opening)
    {
        return false;
    }
    let enforced_0 =
        enforced_sum_ext(&initial_opening.opened_rows, &r_lane_fold, &queries_0, &alpha_0);
    let intro_0 = next_msg!(tx);
    observe(challenger, intro_0.u_0);
    observe(challenger, intro_0.u_2);
    let intro_quad_0 = RoundQuadExt::from_msg(intro_0, enforced_0);
    let beta_0 = challenge!(try_sample(challenger));
    running = RoundQuadExt::fold(&running, &intro_quad_0, beta_0);
    t_r += beta_0 * enforced_0;

    struct LevelCtx {
        log_msg_cols: usize,
        queries: Vec<usize>,
        alpha: Vec<E>,
        ris_start: usize,
        beta: E,
    }
    let mut level_ctxs = vec![LevelCtx {
        log_msg_cols: n1,
        queries: queries_0,
        alpha: alpha_0,
        ris_start: initial_k,
        beta: beta_0,
    }];
    let mut ris: Vec<E> = r_lane_fold;

    let mut prev_root = root_1;
    if n1 < config.recursive_ks[0] {
        return false;
    }
    let mut prev_log_num_interleaved = config.recursive_ks[0];
    let mut prev_log_msg_cols = n1 - prev_log_num_interleaved;
    let mut prev_log_inv_rate = config.log_inv_rates[1];
    let mut next_root_idx = 1usize;
    let mut recursive_idx = 0usize;
    let mut n_current = n1;

    for i in 0..r {
        let k_i = config.recursive_ks[i];
        if n_current < k_i {
            return false;
        }
        let mut level_rs = Vec::with_capacity(k_i);
        for j in 0..k_i {
            fold_pow!(fold_bits(i + 1).saturating_sub(j as u32));
            let ri = challenge!(try_sample(challenger));
            ris.push(ri);
            level_rs.push(ri);
            t_r = running.eval(ri);
            let msg = next_msg!(tx);
            observe(challenger, msg.u_0);
            observe(challenger, msg.u_2);
            running = RoundQuadExt::from_msg(msg, t_r);
        }
        n_current -= k_i;
        let prev_block_len = 1usize << (prev_log_msg_cols + prev_log_inv_rate);
        let prev_lanes = 1usize << prev_log_num_interleaved;

        if i == r - 1 {
            if tx != proof.sumcheck_transcript.len()
                || ood_idx != proof.ood_values.len()
                || fold_nonce != proof.fold_grinding_nonces.len()
            {
                return false;
            }
            let shape = FinalShape {
                log_msg_cols: prev_log_msg_cols,
                log_num_interleaved: prev_log_num_interleaved,
                log_inv_rate: prev_log_inv_rate,
                hash: config.merkle_hash,
            };
            if shape.log_msg_cols != n_current {
                return false;
            }
            let Some(yr) = proof.final_proof.polynomial(shape, &level_rs) else {
                return false;
            };
            if yr.len() != 1usize << n_current {
                return false;
            }
            for &v in yr.iter() {
                observe(challenger, v);
            }
            if nonce_idx >= proof.grinding_nonces.len()
                || nonce_idx + 1 != proof.grinding_nonces.len()
                || !challenger.verify_pow(
                    proof.grinding_nonces[nonce_idx],
                    config.grinding_bits[i + 1] as u32,
                )
            {
                return false;
            }
            let num_queries_last = config.queries[i + 1];
            let queries_last =
                challenge!(try_sample_distinct_queries(challenger, prev_block_len, num_queries_last));
            let alpha_last = challenge!(try_sample_vec(challenger, ceil_log2(num_queries_last)));
            // Bind the last commitment to `yr` (see the base verifier): by
            // the queried rows, or by re-encoding the whole message, whose
            // batching term then cancels from both sides (the draw stays).
            let Some(binding) =
                proof.final_proof.authenticate(shape, &prev_root, &queries_last, &level_rs, &alpha_last)
            else {
                return false;
            };
            let beta_last = challenge!(try_sample(challenger));
            if let FinalBindingExt::QuerySum(enforced_last) = binding {
                t_r += beta_last * enforced_last;
                level_ctxs.push(LevelCtx {
                    log_msg_cols: n_current,
                    queries: queries_last,
                    alpha: alpha_last,
                    ris_start: ris.len(),
                    beta: beta_last,
                });
            }

            let yr_log_n = n_current;
            let yr_len = yr.len();
            let mut combined = eval_b_residual(&ris, yr_log_n);
            if combined.len() != yr_len {
                return false;
            }
            for ctx in &level_ctxs {
                if ctx.log_msg_cols < yr_log_n
                    || ctx.ris_start + (ctx.log_msg_cols - yr_log_n) > ris.len()
                {
                    return false;
                }
                let sks_vks = eval_sk_at_vks(ctx.log_msg_cols);
                let ris_for_basis = &ris[ctx.ris_start..ctx.ris_start + ctx.log_msg_cols - yr_log_n];
                let resid = evaluate_at_residual_ext(
                    ctx.log_msg_cols,
                    &sks_vks,
                    &ctx.queries,
                    &ctx.alpha,
                    ris_for_basis,
                    yr_log_n,
                );
                for (slot, v) in combined.iter_mut().zip(resid) {
                    *slot += ctx.beta * v;
                }
            }
            for ctx in &ood_ctxs {
                if ctx.z.len() < yr_log_n || ctx.ris_start + (ctx.z.len() - yr_log_n) > ris.len() {
                    return false;
                }
                let folded = ctx.z.len() - yr_log_n;
                let mut scalar = ctx.beta;
                for b in 0..folded {
                    // eq(a, b) = 1 + a + b in characteristic two.
                    scalar *= E::ONE + ctx.z[b] + ris[ctx.ris_start + b];
                }
                for (slot, v) in combined.iter_mut().zip(build_eq_table_ext(&ctx.z[folded..])) {
                    *slot += scalar * v;
                }
            }
            let inner = yr
                .iter()
                .zip(combined.iter())
                .fold(E::ZERO, |acc, (&y, &c)| acc + y * c);
            return inner == t_r;
        }

        let Some(&root_next) = proof.recursive_roots.get(next_root_idx) else {
            return false;
        };
        next_root_idx += 1;
        challenger.observe_bytes(&root_next);
        for _ in 0..ood_count(i + 2) {
            let ris_start = ris.len();
            ood_round!(n_current, ris_start);
        }
        if nonce_idx >= proof.grinding_nonces.len()
            || !challenger.verify_pow(
                proof.grinding_nonces[nonce_idx],
                config.grinding_bits[i + 1] as u32,
            )
        {
            return false;
        }
        nonce_idx += 1;

        let num_queries_i = config.queries[i + 1];
        let queries_i =
            challenge!(try_sample_distinct_queries(challenger, prev_block_len, num_queries_i));
        let alpha_i = challenge!(try_sample_vec(challenger, ceil_log2(num_queries_i)));
        let Some(rp) = proof.recursive_proofs.get(recursive_idx) else {
            return false;
        };
        recursive_idx += 1;
        if rp.opened_rows.len() != queries_i.len()
            || !verify_level_opens_ext(
                &prev_root,
                prev_block_len,
                &queries_i,
                &rp.opened_rows,
                prev_lanes,
                &rp.merkle_proof,
                config.merkle_hash,
            )
        {
            return false;
        }
        let enforced_i = enforced_sum_ext(&rp.opened_rows, &level_rs, &queries_i, &alpha_i);
        let intro_i = next_msg!(tx);
        observe(challenger, intro_i.u_0);
        observe(challenger, intro_i.u_2);
        let intro_quad_i = RoundQuadExt::from_msg(intro_i, enforced_i);
        let beta_i = challenge!(try_sample(challenger));
        running = RoundQuadExt::fold(&running, &intro_quad_i, beta_i);
        t_r += beta_i * enforced_i;
        level_ctxs.push(LevelCtx {
            log_msg_cols: n_current,
            queries: queries_i,
            alpha: alpha_i,
            ris_start: ris.len(),
            beta: beta_i,
        });

        prev_root = root_next;
        let Some(&k_next) = config.recursive_ks.get(i + 1) else {
            return false;
        };
        if n_current < k_next {
            return false;
        }
        prev_log_num_interleaved = k_next;
        prev_log_msg_cols = n_current - k_next;
        prev_log_inv_rate = config.log_inv_rates[i + 2];
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::challenger::{FsChallenger, RandomChallenger};

    fn rand_e(rng: &mut RandomChallenger) -> E {
        E::new(rng.sample_f128(), rng.sample_f128())
    }

    #[test]
    fn eq_table_ext_matches_base_on_base_points() {
        let mut rng = RandomChallenger::new(0xE0);
        let z: Vec<Gf128> = (0..6).map(|_| rng.sample_f128()).collect();
        let base = build_eq_table(&z);
        let ext = build_eq_table_ext(&z.iter().map(|&v| E::from_base(v)).collect::<Vec<_>>());
        assert_eq!(ext, base.iter().map(|&v| E::from_base(v)).collect::<Vec<_>>());
    }

    /// The MLE of a base vector at an extension point, the slow way.
    fn mle(f: &[Gf128], point: &[E]) -> E {
        let eq = build_eq_table_ext(point);
        f.iter().zip(eq.iter()).fold(E::ZERO, |acc, (&x, &e)| acc + e.mul_base(x))
    }

    #[test]
    fn induced_basis_residual_matches_dense_fold() {
        // The succinct residual evaluation equals binding the dense induced
        // basis at the same extension point.
        let mut rng = RandomChallenger::new(0xE1);
        let log_msg_cols = 7;
        let yr_log_n = 3;
        let lanes_log = 2;
        let queries: Vec<usize> = vec![3, 17, 40, 41, 99, 200];
        let rows: Vec<Vec<Gf128>> = queries
            .iter()
            .map(|_| (0..1 << lanes_log).map(|_| rng.sample_f128()).collect())
            .collect();
        let v: Vec<E> = (0..lanes_log).map(|_| rand_e(&mut rng)).collect();
        let alpha: Vec<E> = (0..ceil_log2(queries.len())).map(|_| rand_e(&mut rng)).collect();
        let sks = eval_sk_at_vks(log_msg_cols);
        let (basis, enforced) = induce_sumcheck_poly_ext(log_msg_cols, &sks, &rows, &v, &queries, &alpha);
        assert_eq!(enforced, enforced_sum_ext(&rows, &v, &queries, &alpha));
        let ris: Vec<E> = (0..log_msg_cols - yr_log_n).map(|_| rand_e(&mut rng)).collect();
        let mut dense = basis.clone();
        for &r in &ris {
            dense = fold_lsb(&dense, r);
        }
        let succinct = evaluate_at_residual_ext(log_msg_cols, &sks, &queries, &alpha, &ris, yr_log_n);
        assert_eq!(dense, succinct);
    }

    fn configs(
        log_n: usize,
        initial_k: usize,
        ks: &[usize],
        ood_samples: Vec<usize>,
        fold_grinding_bits: Vec<usize>,
    ) -> (ProverConfig, VerifierConfig) {
        let r = ks.len();
        let log_inv_rates: Vec<usize> = (0..=r).map(|i| 1 + i).collect();
        let mut recursive_log_msg_cols = Vec::new();
        let mut dim = log_n - initial_k;
        for &k in ks {
            recursive_log_msg_cols.push(dim - k);
            dim -= k;
        }
        let queries = vec![20usize; r + 1];
        let grinding_bits = vec![2usize; r + 1];
        let p = ProverConfig {
            log_inv_rates: log_inv_rates.clone(),
            recursive_steps: r,
            initial_log_msg_cols: log_n - initial_k,
            initial_log_num_interleaved: initial_k,
            initial_k,
            recursive_log_msg_cols: recursive_log_msg_cols.clone(),
            recursive_ks: ks.to_vec(),
            queries: queries.clone(),
            grinding_bits: grinding_bits.clone(),
            fold_grinding_bits: fold_grinding_bits.clone(),
            ood_samples: ood_samples.clone(),
            merkle_hash: Default::default(),
        };
        let v = VerifierConfig {
            log_inv_rates,
            recursive_steps: r,
            initial_log_msg_cols: log_n - initial_k,
            initial_log_num_interleaved: initial_k,
            initial_k,
            recursive_log_msg_cols,
            recursive_ks: ks.to_vec(),
            queries,
            grinding_bits,
            fold_grinding_bits,
            ood_samples,
            merkle_hash: Default::default(),
        };
        (p, v)
    }

    #[test]
    fn extension_opening_roundtrip_and_tamper() {
        let log_n = 12;
        let initial_k = 2;
        let ks = [2usize, 2];
        let (p_cfg, v_cfg) = configs(log_n, initial_k, &ks, vec![0, 1, 1], vec![3, 2, 2]);
        let mut rng = RandomChallenger::new(0xE2);
        let poly: Vec<Gf128> = (0..1usize << log_n).map(|_| rng.sample_f128()).collect();
        let z: Vec<E> = (0..log_n).map(|_| rand_e(&mut rng)).collect();
        let b = build_eq_table_ext(&z);
        let target = mle(&poly, &z);

        let ntt_0 = AdditiveNttF128::standard(log_n - initial_k + 1);
        let wtns_0 = ligero_commit(&poly, log_n - initial_k, initial_k, 1, &ntt_0, HashKind::Sha256);
        let root = wtns_0.root();

        let mut p_ch = FsChallenger::new(b"ext-test");
        let proof = recursive_prover_with_basis_ext(
            &p_cfg,
            &poly,
            b.clone(),
            target,
            &wtns_0.mat,
            &wtns_0.tree,
            &mut p_ch,
        );
        assert_eq!(proof.ood_values.len(), 2);
        assert!(proof.recursive_proofs[0].opened_rows[0].len() == 1 << ks[0]);

        let z_res = z.clone();
        let eval_b = move |ris: &[E], yr_log_n: usize| -> Vec<E> {
            // eq(z, ris ‖ y) = eq(z[..k], ris)·eq(z[k..], y).
            let k = ris.len();
            let mut scalar = E::ONE;
            for (a, b) in z_res[..k].iter().zip(ris.iter()) {
                scalar *= E::ONE + *a + *b;
            }
            assert_eq!(k + yr_log_n, z_res.len());
            build_eq_table_ext(&z_res[k..]).into_iter().map(|v| v * scalar).collect()
        };
        let verify = |proof: &LigeritoProofExt, target: E| {
            let mut ch = FsChallenger::new(b"ext-test");
            recursive_verifier_with_basis_succinct_ext(&v_cfg, proof, log_n, target, &root, &eval_b, &mut ch)
        };
        assert!(verify(&proof, target), "honest extension proof must verify");
        assert!(!verify(&proof, target + E::ONE), "wrong target");

        let mut bad = proof.clone();
        bad.ood_values[0] += E::ONE;
        assert!(!verify(&bad, target), "tampered OOD value");
        let mut bad = proof.clone();
        bad.final_proof.yr[0] += E::new(Gf128::ZERO, Gf128::ONE);
        assert!(!verify(&bad, target), "tampered final message (Y coordinate)");
        let mut bad = proof.clone();
        bad.sumcheck_transcript[1].u_2 += E::ONE;
        assert!(!verify(&bad, target), "tampered sumcheck message");
        let mut bad = proof.clone();
        bad.recursive_proofs[0].opened_rows[0][0] += E::new(Gf128::ZERO, Gf128::ONE);
        assert!(!verify(&bad, target), "tampered recursive row");
        let mut bad = proof.clone();
        bad.initial_proof.opened_rows[0][0] += Gf128::ONE;
        assert!(!verify(&bad, target), "tampered level-0 row");
        let mut bad = proof.clone();
        bad.fold_grinding_nonces[0] ^= 1;
        assert!(!verify(&bad, target), "tampered fold nonce");

        // A false claim the prover cannot make true: proving a wrong target
        // fails verification.
        let mut p_ch = FsChallenger::new(b"ext-test");
        let lying = recursive_prover_with_basis_ext(
            &p_cfg,
            &poly,
            b,
            target + E::ONE,
            &wtns_0.mat,
            &wtns_0.tree,
            &mut p_ch,
        );
        assert!(!verify(&lying, target + E::ONE), "false claim must not verify");
    }

    #[test]
    fn compact_initial_extension_opening_roundtrip_and_tamper() {
        // An externally authenticated level 0 (here: the codeword's own
        // tree under a foreign identifier) and the compact terminal.
        let log_n = 12;
        let initial_k = 2;
        let ks = [2usize, 2];
        let (p_cfg, v_cfg) = configs(log_n, initial_k, &ks, vec![0, 1, 1], vec![3, 2, 2]);
        let mut rng = RandomChallenger::new(0xE3);
        let poly: Vec<Gf128> = (0..1usize << log_n).map(|_| rng.sample_f128()).collect();
        let z: Vec<E> = (0..log_n).map(|_| rand_e(&mut rng)).collect();
        let b = build_eq_table_ext(&z);
        let target = mle(&poly, &z);
        let ntt_0 = AdditiveNttF128::standard(log_n - initial_k + 1);
        let wtns_0 = ligero_commit(&poly, log_n - initial_k, initial_k, 1, &ntt_0, HashKind::Sha256);
        let tree_root = wtns_0.root();
        let statement = [0x5a; 32];

        let mut p_ch = FsChallenger::new(b"ext-compact-test");
        let proof = recursive_prover_with_basis_initial_compact_ext(
            &p_cfg,
            &poly,
            b.clone(),
            target,
            statement,
            |block_len, lanes, queries: &[usize]| RecursiveProof {
                opened_rows: queries.iter().map(|&q| wtns_0.mat[q * lanes..(q + 1) * lanes].to_vec()).collect(),
                merkle_proof: merkle_multi_proof_for(&wtns_0.tree, block_len, queries),
            },
            &mut p_ch,
        );
        // The terminal is the last level's pre-fold message: 2^(12-2-2) = 2^8.
        assert_eq!(proof.final_proof.len(), 1 << (log_n - initial_k - ks[0]));

        let z_res = z.clone();
        let eval_b = move |ris: &[E], yr_log_n: usize| -> Vec<E> {
            let k = ris.len();
            let mut scalar = E::ONE;
            for (a, b) in z_res[..k].iter().zip(ris.iter()) {
                scalar *= E::ONE + *a + *b;
            }
            assert_eq!(k + yr_log_n, z_res.len());
            build_eq_table_ext(&z_res[k..]).into_iter().map(|v| v * scalar).collect()
        };
        let verify = |proof: &CompactLigeritoProofExt, target: E| {
            let mut ch = FsChallenger::new(b"ext-compact-test");
            recursive_verifier_with_basis_initial_compact_ext(
                &v_cfg,
                proof,
                &proof.initial_proof,
                log_n,
                target,
                &statement,
                &eval_b,
                |block_len, lanes, queries, opening| {
                    verify_level_opens(
                        &tree_root,
                        block_len,
                        queries,
                        &opening.opened_rows,
                        lanes,
                        &opening.merkle_proof,
                        HashKind::Sha256,
                    )
                },
                &mut ch,
            )
        };
        assert!(verify(&proof, target), "honest compact extension proof must verify");
        assert!(!verify(&proof, target + E::ONE), "wrong target");
        let mut bad = proof.clone();
        bad.final_proof[3] += E::new(Gf128::ZERO, Gf128::ONE);
        assert!(!verify(&bad, target), "tampered terminal message");
        let mut bad = proof.clone();
        bad.final_proof.pop();
        assert!(!verify(&bad, target), "short terminal message");
        let mut bad = proof.clone();
        bad.initial_proof.opened_rows[1][0] += Gf128::ONE;
        assert!(!verify(&bad, target), "tampered level-0 row");
        let mut bad = proof.clone();
        bad.ood_values[1] += E::ONE;
        assert!(!verify(&bad, target), "tampered OOD value");

        // The compact and expanded terminals share one transcript up to the
        // terminal: the expanded proof's messages are the compact proof's.
        let mut p_ch = FsChallenger::new(b"ext-compact-test");
        let expanded: LigeritoProofExt = prover_impl(
            &p_cfg,
            &poly,
            b,
            target,
            statement,
            |block_len, lanes, queries: &[usize]| RecursiveProof {
                opened_rows: queries.iter().map(|&q| wtns_0.mat[q * lanes..(q + 1) * lanes].to_vec()).collect(),
                merkle_proof: merkle_multi_proof_for(&wtns_0.tree, block_len, queries),
            },
            &mut p_ch,
        );
        assert_eq!(expanded.sumcheck_transcript, proof.sumcheck_transcript);
        assert_eq!(expanded.recursive_roots, proof.recursive_roots);
        assert_eq!(expanded.ood_values, proof.ood_values);
        // The last level folds `ks[1]` variables of the captured message.
        assert_eq!(proof.final_proof.len() >> ks[1], expanded.final_proof.yr.len());
    }
}
