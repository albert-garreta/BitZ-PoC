//! Their `gkr` crate: the layer-by-layer sumcheck reduction of a batched
//! grand product, from a claim on the output layer down to the leaves.
//!
//! Each layer binds the joint point MSB-first (the top remaining index bit
//! each round, halves rather than adjacent pairs), sends the two
//! non-constant Gruen coefficients `[factor·Σ_endpoint, factor·Σ_∞]`, then
//! the two children at the bound point and one more challenge to select
//! between them. Points are little-endian outside (`coordinate j ↔ index
//! bit j`) and reversed inside so front-to-back iteration binds the MSB.

use std::collections::VecDeque;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::eq_factor;
use super::kernels;
use super::transcript::{ProverState, VerifierState};
use crate::cfg_into_iter;
use crate::poly::utils::build_eq_x_r_vec;
use field::Gf128 as Gf;

/// Work-splitting granularity for the per-round passes (their
/// `PARALLEL_MIN_LANES`).
const PARALLEL_MIN_LANES: usize = 1 << 12;

pub(crate) type Point = VecDeque<Gf>;

/// Whether [`prove_dense_rounds`] keeps the column weights `eq_c` of its
/// in-tree rounds inside the left half ([`kernels::Left`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weighing {
    /// Every slot multiplies by its column weight.
    Off,
    /// The first fused in-tree round multiplies the left half by `eq_c`,
    /// in place of its slots' own multiplies, and the in-tree rounds after
    /// it skip them — on layers [`weighable`] by their point.
    On,
    /// The left half arrives multiplied by `eq_c` already.
    Carried,
}

/// Whether a layer at the point with little-endian coordinates `external`
/// (the low `s` of them column coordinates) can carry `eq_c` in its left
/// half: it needs column coordinates, at least one in-tree round, and
/// every weight invertible, i.e. no column coordinate equal to 0 or 1 —
/// the weights leave the half again before the column rounds.
pub(crate) fn weighable(external: &[Gf], s: usize) -> bool {
    s > 0
        && external.len() > s
        && external[..s]
            .iter()
            .all(|&z| z != Gf::zero() && z != Gf::one())
}

/// Cost gate for starting to carry column weights. Each subsequent tree
/// round saves two multiplies per slot; removing the weights costs one
/// multiply per column and a scalar inverse. Reserve 256 scalar operations
/// for the inverse and bookkeeping (the inverse uses 127 squares and ten
/// multiplies). This is a performance heuristic, independent of soundness.
pub(crate) fn carrying_worthwhile(s: usize, subsequent_tree_rounds: usize) -> bool {
    let columns = 1usize.checked_shl(s as u32).unwrap_or(usize::MAX);
    let rows = 1usize.checked_shl(subsequent_tree_rounds as u32).unwrap_or(usize::MAX);
    let saved = columns.saturating_mul(rows - 1).saturating_mul(2);
    s > 0 && subsequent_tree_rounds > 0 && saved > columns.saturating_add(256)
}

/// The GKR prover's message interface. A composition may use its own
/// transcript, provided each pair is absorbed before the next challenge.
/// The caller binds the statement and root claim before entering GKR.
pub trait GkrProverTranscript {
    fn write_pair(&mut self, pair: [Gf; 2]);
    fn challenge(&mut self) -> Gf;

    /// Draw after the two terminal children, which has its own native stage.
    fn close_challenge(&mut self) -> Gf {
        self.challenge()
    }
}

/// The matching verifier interface. `read_pair` must absorb exactly the
/// pair returned; the caller rejects any unconsumed proof messages.
pub trait GkrVerifierTranscript {
    fn read_pair(&mut self) -> Option<[Gf; 2]>;
    /// Reject a malformed challenge frame or invalid grinding nonce.
    fn challenge(&mut self) -> Option<Gf>;

    fn close_challenge(&mut self) -> Option<Gf> {
        self.challenge()
    }
}

impl GkrProverTranscript for ProverState {
    fn write_pair(&mut self, pair: [Gf; 2]) {
        self.prover_message(&pair);
    }

    fn challenge(&mut self) -> Gf {
        self.native_scalar(super::grinding::Stage::GkrRound)
    }

    fn close_challenge(&mut self) -> Gf {
        self.native_scalar(super::grinding::Stage::GkrClose)
    }
}

impl GkrVerifierTranscript for VerifierState<'_> {
    fn read_pair(&mut self) -> Option<[Gf; 2]> {
        self.prover_message().ok()
    }

    fn challenge(&mut self) -> Option<Gf> {
        self.native_scalar(super::grinding::Stage::GkrRound).ok()
    }

    fn close_challenge(&mut self) -> Option<Gf> {
        self.native_scalar(super::grinding::Stage::GkrClose).ok()
    }
}

/// The batched product tree: leaves at the bottom, `groups` roots at the
/// top, product pairs `(i, i + half)` on the top index bit.
pub struct GrandProductCircuit {
    leafs: Vec<Gf>,
}

/// All the intermediate witnesses plus the input layer, leaves first.
pub struct LayerWitnesses(Vec<Vec<Gf>>);

impl GrandProductCircuit {
    pub fn new(mut leafs: Vec<Gf>) -> Self {
        if !leafs.is_empty() {
            leafs.resize(leafs.len().next_power_of_two(), Gf::one());
        }
        Self { leafs }
    }

    /// Consumes the leaves: at `2^28` bits they are 4 GiB, and the layers
    /// above them another 4 GiB, so the clone their version makes is what
    /// decides whether the large shape fits in memory.
    pub fn batched_eval(self, groups: usize) -> (Vec<Gf>, LayerWitnesses) {
        let mut witnesses = Vec::with_capacity((self.leafs.len() + 1).ilog2() as usize);
        let mut prev_eval = self.leafs;
        while prev_eval.len() > groups {
            let mid = prev_eval.len() / 2;
            let (l, r) = prev_eval.split_at(mid);
            let eval: Vec<Gf> = cfg_into_iter!(0..mid, PARALLEL_MIN_LANES)
                .map(|i| l[i] * r[i])
                .collect();
            witnesses.push(prev_eval);
            prev_eval = eval;
        }
        (prev_eval, LayerWitnesses(witnesses))
    }
}

/// Proves the reduction from a claim at `point` on the output layer down to
/// a claim on the leaves; returns the leaf point and the claimed leaf MLE
/// value.
pub fn gpgkr_prove(
    ps: &mut impl GkrProverTranscript,
    point: &[Gf],
    witnesses: LayerWitnesses,
) -> (Vec<Gf>, Gf) {
    let mut point = point.to_owned();
    point.reverse();
    let mut point = VecDeque::from(point);
    let mut claim = Gf::zero();
    // Top-most witness layer first: the layers were pushed leaves-first.
    for wnext in witnesses.0.into_iter().rev() {
        (point, claim) = prove_layer(ps, point, wnext);
    }
    let mut point = Vec::from(point);
    point.reverse();
    (point, claim)
}

fn prove_layer(ps: &mut impl GkrProverTranscript, point: Point, mut wnext: Vec<Gf>) -> (Point, Gf) {
    let mid = wnext.len() / 2;
    let (mle_l, mle_r) = wnext.split_at_mut(mid);
    prove_layer_from(ps, point, mle_l, mle_r, 0, Gf::one(), VecDeque::new())
}

/// The dense rounds of one layer from round `skip` on: the first `skip`
/// coordinates of `point` are already bound (their challenges in
/// `next_point`, their eq factors in `factor`) and `mle_l`/`mle_r` are the
/// two halves folded that far.
pub(crate) fn prove_layer_from(
    ps: &mut impl GkrProverTranscript,
    point: Point,
    mle_l: &mut [Gf],
    mle_r: &mut [Gf],
    skip: usize,
    factor: Gf,
    next_point: VecDeque<Gf>,
) -> (Point, Gf) {
    prove_layer_tensor(
        ps,
        point,
        mle_l,
        mle_r,
        skip,
        factor,
        next_point,
        0,
        Weighing::Off,
    )
}

/// [`prove_layer_from`] told that the low `s` index bits are column bits:
/// while in-tree bits remain, the round's eq weight is `eq_y[y]·eq_c[c]`
/// with `y` the row of `2^s` entries, so no `2^{remaining}` eq table is
/// built; once only column bits remain the table is small and built as is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_layer_tensor(
    ps: &mut impl GkrProverTranscript,
    point: Point,
    mle_l: &mut [Gf],
    mle_r: &mut [Gf],
    skip: usize,
    factor: Gf,
    next_point: VecDeque<Gf>,
    s: usize,
    weighing: Weighing,
) -> (Point, Gf) {
    prove_dense_rounds(
        ps, point, mle_l, mle_r, skip, None, factor, next_point, s, weighing,
    )
}

/// The first half of `v` folded with its second: `v[i] + rho·(v[i+half] − v[i])`.
fn fold_halves(v: &mut [Gf], rho: Gf) {
    let half = v.len() / 2;
    let (lo, hi) = v.split_at_mut(half);
    for (a, &b) in lo.iter_mut().zip(hi.iter()) {
        *a = *a + rho * (b - *a);
    }
}

/// The dense rounds from round `first` on, each round's fold deferred into
/// the next round's pass ([`kernels::fused_fold_round`]): the previous
/// round's challenge sits in `pending` until the next pass folds it in
/// registers while accumulating that round's sums, so every table is swept
/// once per round instead of twice. With a fold pending at entry the halves
/// hold `2^{total − first + 1}` entries each (the unfolded table), else
/// `2^{total − first}`. The folded tables occupy the prefixes. With
/// `weighing`, the in-tree rounds keep the column weights in the left half
/// (it arrives so when `Carried`), and the first column round divides them
/// back out of its `2^s` entries.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_dense_rounds(
    ps: &mut impl GkrProverTranscript,
    point: Point,
    mut mle_l: &mut [Gf],
    mut mle_r: &mut [Gf],
    first: usize,
    mut pending: Option<Gf>,
    mut factor: Gf,
    mut next_point: VecDeque<Gf>,
    s: usize,
    weighing: Weighing,
) -> (Point, Gf) {
    // The eq table a round needs is the one over the coordinates not yet
    // bound, in little-endian (external) order: the external prefix.
    let external: Vec<Gf> = point.iter().rev().copied().collect();
    let total = point.len();
    debug_assert_eq!(
        mle_l.len(),
        1usize << (total - first + usize::from(pending.is_some()))
    );
    let eq_c = if s > 0 && total > s {
        eq_table(&external[..s])
    } else {
        Vec::new()
    };
    let one = [Gf::one()];
    let may_weigh = weighing != Weighing::Off && weighable(&external, s);
    let mut weighted = weighing == Weighing::Carried;
    let mut left_normalization = None;
    assert!(!weighted || may_weigh, "a weighted left half on a layer that cannot carry weights");

    for (round, z) in point.into_iter().enumerate().skip(first) {
        let remaining = total - 1 - round;
        let h = 1usize << remaining;
        let in_tree = s > 0 && remaining >= s;
        if weighted && !in_tree {
            // All in-tree bits are bound: the left half's `2^s` entries are
            // indexed by column. Apply the pending fold and take `eq_c` out;
            // the column rounds weigh their slots as usual.
            if let Some(rho) = pending.take() {
                fold_halves(mle_l, rho);
                fold_halves(mle_r, rho);
                mle_l = &mut mle_l[..2 * h];
                mle_r = &mut mle_r[..2 * h];
            }
            assert_eq!(mle_l.len(), eq_c.len());
            // Complementary equality weights have a common product:
            // eq_c[i] * eq_c[n-1-i] = product_j z_j(1-z_j) = c.
            // Multiplying by the reversed table leaves c times the plain
            // left half. Keep 1/c in the message factor, then normalize
            // only the terminal left value. This replaces the serial batch
            // inversion with one independent multiply per column.
            let inverse = (eq_c[0] * eq_c[eq_c.len() - 1]).inverse_or_zero();
            kernels::multiply_reversed_in_place(mle_l, &eq_c);
            factor = factor * inverse;
            left_normalization = Some(inverse);
            weighted = false;
        }

        // At z = 0 the incoming claim determines the value at zero, so send
        // the value at one. Otherwise send the value at zero as usual.
        let send_one = z == Gf::zero();
        let (eq_flat, eq_y);
        let weights = if s > 0 && remaining >= s {
            eq_y = eq_table(&external[s..remaining]);
            kernels::Weights {
                eq_c: &eq_c,
                eq_y: &eq_y,
            }
        } else {
            eq_flat = eq_table(&external[..remaining]);
            kernels::Weights {
                eq_c: &eq_flat,
                eq_y: &one,
            }
        };

        let (sum_endpoint, sum_inf) = match pending.take() {
            Some(rho) => {
                debug_assert_eq!(mle_l.len(), 4 * h);
                let (q01_l, q23_l) = mle_l.split_at_mut(2 * h);
                let (q0_l, q1_l) = q01_l.split_at_mut(h);
                let (q2_l, q3_l) = q23_l.split_at(h);
                let (q01_r, q23_r) = mle_r.split_at_mut(2 * h);
                let (q0_r, q1_r) = q01_r.split_at_mut(h);
                let (q2_r, q3_r) = q23_r.split_at(h);
                let left = if weighted {
                    kernels::Left::Weighted
                } else if in_tree && may_weigh && carrying_worthwhile(s, remaining - s) {
                    weighted = true;
                    kernels::Left::Weigh
                } else {
                    kernels::Left::Plain
                };
                kernels::fused_fold_round(
                    q0_l, q1_l, q2_l, q3_l, q0_r, q1_r, q2_r, q3_r, rho, weights, send_one, left,
                )
            }
            None => {
                debug_assert!(!weighted, "a weighted left half always has a fold pending");
                debug_assert_eq!(mle_l.len(), 2 * h);
                let (lo_l, hi_l) = mle_l.split_at(h);
                let (lo_r, hi_r) = mle_r.split_at(h);
                kernels::round_sums(lo_l, hi_l, lo_r, hi_r, weights, send_one)
            }
        };
        ps.write_pair([factor * sum_endpoint, factor * sum_inf]);

        let r = ps.challenge();
        next_point.push_back(r);
        pending = Some(r);
        // The folded table (once `r` is applied) has `h` entries per half;
        // until then the unfolded `2h` stay.
        mle_l = &mut mle_l[..2 * h];
        mle_r = &mut mle_r[..2 * h];
        factor = factor * eq_factor(r, z);
    }
    debug_assert!(!weighted, "the column rounds take the weights out");

    if let Some(rho) = pending {
        mle_l[0] = mle_l[0] + rho * (mle_l[1] - mle_l[0]);
        mle_r[0] = mle_r[0] + rho * (mle_r[1] - mle_r[0]);
    }
    if let Some(inverse) = left_normalization {
        mle_l[0] = mle_l[0] * inverse;
    }
    ps.write_pair([mle_l[0], mle_r[0]]);
    let r = ps.close_challenge();
    next_point.push_front(r);
    let claim = mle_l[0] + r * (mle_r[0] - mle_l[0]);
    (next_point, claim)
}

/// `eq(·, point)` over the hypercube, index bit `j` ↔ `point[j]`; `[1]`
/// for the empty point.
pub(crate) fn eq_table(point: &[Gf]) -> Vec<Gf> {
    if point.is_empty() {
        return vec![Gf::one()];
    }
    build_eq_x_r_vec(point, &()).expect("non-empty point")
}

/// Replays `rounds` layers from `claim` at `point`; `None` on any failed
/// check or short read.
#[must_use]
pub fn gpgkr_verify(
    vs: &mut impl GkrVerifierTranscript,
    mut claim: Gf,
    point: &[Gf],
    rounds: u32,
) -> Option<(Vec<Gf>, Gf)> {
    let mut point = point.to_owned();
    point.reverse();
    let mut point = VecDeque::from(point);
    for _ in 0..rounds {
        (point, claim) = verify_layer(vs, claim, point)?;
    }
    let mut point = Vec::from(point);
    point.reverse();
    Some((point, claim))
}

fn verify_layer(vs: &mut impl GkrVerifierTranscript, mut claim: Gf, point: Point) -> Option<(Point, Gf)> {
    let one = Gf::one();
    let mut prefix = one;
    let mut next_point: Point = VecDeque::new();

    for z in point {
        let [sum_endpoint, suminf] = vs.read_pair()?;
        let (sum0, sum1) = if z == Gf::zero() {
            (claim, sum_endpoint)
        } else {
            let eqjsum0 = (one - z) * sum_endpoint;
            (sum_endpoint, (claim - eqjsum0) / z)
        };
        let r = vs.challenge()?;
        next_point.push_back(r);
        let factor = eq_factor(r, z);
        let bracket = (sum1 - sum0) + (r - one) * suminf;
        claim = factor * (sum0 + r * bracket);
        prefix = prefix * factor;
    }

    let elem_lr = vs.read_pair()?;
    if prefix * elem_lr[0] * elem_lr[1] != claim {
        return None;
    }
    let r = vs.close_challenge()?;
    next_point.push_front(r);
    claim = elem_lr[0] + r * (elem_lr[1] - elem_lr[0]);
    Some((next_point, claim))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitz::transcript::build_kernel_prover;

    fn field(state: &mut u64) -> Gf {
        let mut next = || {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        };
        Gf::from_polynomial_words([next(), next()])
    }

    #[test]
    fn complementary_equality_weights_remove_column_weights() {
        let mut state = 0xCA77_1ED0;
        for s in 1..=10 {
            let coordinates: Vec<_> = (0..s).map(|_| field(&mut state)).collect();
            let weights = eq_table(&coordinates);
            let constant = coordinates.iter().fold(Gf::one(), |a, &z| a * z * (Gf::one() - z));
            let inverse = constant.inverse_or_zero();
            assert_eq!(constant * inverse, Gf::one());
            let plain: Vec<_> = (0..weights.len()).map(|_| field(&mut state)).collect();
            let mut carried: Vec<_> = plain.iter().zip(&weights).map(|(&x, &w)| x * w).collect();
            kernels::multiply_reversed_in_place(&mut carried, &weights);
            for (i, (&actual, &expected)) in carried.iter().zip(&plain).enumerate() {
                assert_eq!(weights[i] * weights[weights.len() - 1 - i], constant);
                assert_eq!(actual * inverse, expected);
            }
        }
    }

    #[test]
    fn carrying_gate_requires_amortized_savings() {
        assert!(!carrying_worthwhile(14, 0));
        assert!(carrying_worthwhile(14, 1));
        assert!(!carrying_worthwhile(8, 1));
        assert!(carrying_worthwhile(9, 1));
        assert!(!carrying_worthwhile(1, 1));
        assert!(carrying_worthwhile(1, 7));
        assert!(!carrying_worthwhile(0, 12));
    }

    /// The dense rounds send the same bytes and reach the same point and
    /// claim whether the column weights ride in the left half or in every
    /// slot: from a plain start, from a half that arrives weighted with a
    /// fold pending, and when a column coordinate is 0 or 1 (no weighing).
    #[test]
    fn dense_rounds_weigh_identically() {
        for (tree, s) in [(1usize, 3usize), (2, 1), (3, 4), (4, 6), (5, 2), (6, 7), (3, 0), (0, 4), (2, 8), (3, 9)] {
            let total = tree + s;
            for special in [None, Some(Gf::zero()), Some(Gf::one())] {
                let mut state = 0x9E37_79B9 ^ (tree * 97 + s) as u64;
                let mut external: Vec<Gf> = (0..total).map(|_| field(&mut state)).collect();
                if let (Some(value), true) = (special, s > 0) {
                    external[s / 2] = value;
                }
                let point: Point = external.iter().rev().copied().collect();
                let incoming_factor = field(&mut state);
                let run = |l: &[Gf], r: &[Gf], first: usize, pending: Option<Gf>, weighing: Weighing| {
                    let (mut l, mut r) = (l.to_vec(), r.to_vec());
                    let mut ps = build_kernel_prover(b"dense-weigh/v1", b"instance");
                    let next_point: VecDeque<Gf> = point.iter().take(first).copied().collect();
                    let (p, claim) =
                        prove_dense_rounds(&mut ps, point.clone(), &mut l, &mut r, first, pending, incoming_factor, next_point, s, weighing);
                    (p, claim, ps.finish().narg_string)
                };
                let label = format!("tree={tree} s={s} special={special:?}");
                let l: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let r: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let want = run(&l, &r, 0, None, Weighing::Off);
                assert!(
                    run(&l, &r, 0, None, Weighing::On) == want,
                    "plain start {label}"
                );
                if tree == 0 {
                    continue;
                }
                // Exercise every entry depth, including a carried half
                // arriving immediately before the first column round.
                for first in 1..=tree {
                    let len = 1usize << (total - first + 1);
                    let (l, r) = (&l[..len], &r[..len]);
                    let rho = field(&mut state);
                    let want = run(l, r, first, Some(rho), Weighing::Off);
                    assert!(run(l, r, first, Some(rho), Weighing::On) == want, "pending start {first} {label}");
                    if weighable(&external, s) {
                        let eq_c = eq_table(&external[..s]);
                        let mask = (1usize << s) - 1;
                        let weighted: Vec<Gf> = l.iter().enumerate().map(|(i, &x)| eq_c[i & mask] * x).collect();
                        assert!(run(&weighted, r, first, Some(rho), Weighing::Carried) == want, "carried {first} {label}");
                    }
                }
            }
        }
    }
}
