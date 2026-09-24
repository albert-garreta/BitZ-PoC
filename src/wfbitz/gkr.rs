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
use field::Gf128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;

/// Work-splitting granularity for the per-round passes (their
/// `PARALLEL_MIN_LANES`).
const PARALLEL_MIN_LANES: usize = 1 << 12;

pub(crate) type Point = VecDeque<Gf>;

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
    ps: &mut ProverState,
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

fn prove_layer(ps: &mut ProverState, point: Point, mut wnext: Vec<Gf>) -> (Point, Gf) {
    let mid = wnext.len() / 2;
    let (mle_l, mle_r) = wnext.split_at_mut(mid);
    prove_layer_from(ps, point, mle_l, mle_r, 0, Gf::one(), VecDeque::new())
}

/// The dense rounds of one layer from round `skip` on: the first `skip`
/// coordinates of `point` are already bound (their challenges in
/// `next_point`, their eq factors in `factor`) and `mle_l`/`mle_r` are the
/// two halves folded that far.
pub(crate) fn prove_layer_from(
    ps: &mut ProverState,
    point: Point,
    mle_l: &mut [Gf],
    mle_r: &mut [Gf],
    skip: usize,
    factor: Gf,
    next_point: VecDeque<Gf>,
) -> (Point, Gf) {
    prove_layer_tensor(ps, point, mle_l, mle_r, skip, factor, next_point, 0)
}

/// [`prove_layer_from`] told that the low `s` index bits are column bits:
/// while in-tree bits remain, the round's eq weight is `eq_y[y]·eq_c[c]`
/// with `y` the row of `2^s` entries, so no `2^{remaining}` eq table is
/// built; once only column bits remain the table is small and built as is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_layer_tensor(
    ps: &mut ProverState,
    point: Point,
    mle_l: &mut [Gf],
    mle_r: &mut [Gf],
    skip: usize,
    factor: Gf,
    next_point: VecDeque<Gf>,
    s: usize,
) -> (Point, Gf) {
    prove_dense_rounds(ps, point, mle_l, mle_r, skip, None, factor, next_point, s)
}

/// The dense rounds from round `first` on, each round's fold deferred into
/// the next round's pass ([`kernels::fused_fold_round`]): the previous
/// round's challenge sits in `pending` until the next pass folds it in
/// registers while accumulating that round's sums, so every table is swept
/// once per round instead of twice. With a fold pending at entry the halves
/// hold `2^{total − first + 1}` entries each (the unfolded table), else
/// `2^{total − first}`. The folded tables occupy the prefixes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_dense_rounds(
    ps: &mut ProverState,
    point: Point,
    mut mle_l: &mut [Gf],
    mut mle_r: &mut [Gf],
    first: usize,
    mut pending: Option<Gf>,
    mut factor: Gf,
    mut next_point: VecDeque<Gf>,
    s: usize,
) -> (Point, Gf) {
    // The eq table a round needs is the one over the coordinates not yet
    // bound, in little-endian (external) order: the external prefix.
    let external: Vec<Gf> = point.iter().rev().copied().collect();
    let total = point.len();
    debug_assert_eq!(
        mle_l.len(),
        1usize << (total - first + usize::from(pending.is_some()))
    );
    let eq_c = if s > 0 && total > s { eq_table(&external[..s]) } else { Vec::new() };
    let one = [Gf::one()];

    for (round, z) in point.into_iter().enumerate().skip(first) {
        let remaining = total - 1 - round;
        let h = 1usize << remaining;

        // At z = 0 the incoming claim determines the value at zero, so send
        // the value at one. Otherwise send the value at zero as usual.
        let send_one = z == Gf::zero();
        let (eq_flat, eq_y);
        let weights = if s > 0 && remaining >= s {
            eq_y = eq_table(&external[s..remaining]);
            kernels::Weights { eq_c: &eq_c, eq_y: &eq_y }
        } else {
            eq_flat = eq_table(&external[..remaining]);
            kernels::Weights { eq_c: &eq_flat, eq_y: &one }
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
                kernels::fused_fold_round(
                    q0_l, q1_l, q2_l, q3_l, q0_r, q1_r, q2_r, q3_r, rho, weights, send_one,
                )
            }
            None => {
                debug_assert_eq!(mle_l.len(), 2 * h);
                let (lo_l, hi_l) = mle_l.split_at(h);
                let (lo_r, hi_r) = mle_r.split_at(h);
                kernels::round_sums(lo_l, hi_l, lo_r, hi_r, weights, send_one)
            }
        };
        ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);

        let r: Gf = ps.verifier_message();
        next_point.push_back(r);
        pending = Some(r);
        // The folded table (once `r` is applied) has `h` entries per half;
        // until then the unfolded `2h` stay.
        mle_l = &mut mle_l[..2 * h];
        mle_r = &mut mle_r[..2 * h];
        factor = factor * eq_factor(r, z);
    }

    if let Some(rho) = pending {
        mle_l[0] = mle_l[0] + rho * (mle_l[1] - mle_l[0]);
        mle_r[0] = mle_r[0] + rho * (mle_r[1] - mle_r[0]);
    }
    ps.prover_message(&[mle_l[0], mle_r[0]]);
    let r: Gf = ps.verifier_message();
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
    vs: &mut VerifierState<'_>,
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

fn verify_layer(vs: &mut VerifierState<'_>, mut claim: Gf, point: Point) -> Option<(Point, Gf)> {
    let one = Gf::one();
    let mut prefix = one;
    let mut next_point: Point = VecDeque::new();

    for z in point {
        let [sum_endpoint, suminf]: [Gf; 2] = vs.prover_message().ok()?;
        let (sum0, sum1) = if z == Gf::zero() {
            (claim, sum_endpoint)
        } else {
            let eqjsum0 = (one - z) * sum_endpoint;
            (sum_endpoint, (claim - eqjsum0) / z)
        };
        let r: Gf = vs.verifier_message();
        next_point.push_back(r);
        let factor = eq_factor(r, z);
        let bracket = (sum1 - sum0) + (r - one) * suminf;
        claim = factor * (sum0 + r * bracket);
        prefix = prefix * factor;
    }

    let elem_lr: [Gf; 2] = vs.prover_message().ok()?;
    if prefix * elem_lr[0] * elem_lr[1] != claim {
        return None;
    }
    let r: Gf = vs.verifier_message();
    next_point.push_front(r);
    claim = elem_lr[0] + r * (elem_lr[1] - elem_lr[0]);
    Some((next_point, claim))
}
