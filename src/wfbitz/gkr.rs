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

impl Weighing {
    /// `On` unless `WFBITZ_WEIGH=0`, read once.
    pub(crate) fn from_env() -> Self {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let on = *ON.get_or_init(|| !matches!(std::env::var("WFBITZ_WEIGH").as_deref(), Ok("0")));
        if on { Self::On } else { Self::Off }
    }
}

/// The challenges a dense pass still has to fold into its table: none,
/// the previous round's, or a window's two (`Two(ρa, ρb)`, `ρa` bound
/// first, on the higher index bit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    None,
    One(Gf),
    Two(Gf, Gf),
}

impl Pending {
    /// How many index bits the table still carries unfolded.
    pub(crate) fn bits(&self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) => 1,
            Self::Two(..) => 2,
        }
    }
}

/// When [`prove_dense_rounds`] takes two in-tree rounds in one pass
/// ([`kernels::window_pass`]): the pass that folds the pending challenges
/// in also accumulates both rounds' [`kernels::Grid8`], so the table is
/// swept once per two rounds. That spends about 5 % more multiplies to cut
/// a layer's dense traffic by a fifth to a third — a gain only where the
/// passes wait on memory, which at one thread they do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DoubleFold {
    pub(crate) when: When,
    /// Once a layer's windows open, keep them open down to its last
    /// in-tree rounds (else each window needs the size gate).
    pub(crate) stay: bool,
    /// Also make a layer's read-only first pass (nothing pending) a
    /// window.
    pub(crate) plain0: bool,
    /// Columns per window task, a power of two.
    pub(crate) chunk: usize,
}

/// Where [`DoubleFold`] opens windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum When {
    Never,
    Always,
    /// With at least `threads` threads, on tables of at least `2^log`
    /// entries per half.
    Auto { threads: usize, log: usize },
}

impl DoubleFold {
    /// One round per pass.
    pub(crate) const OFF: Self = Self { when: When::Never, stay: true, plain0: false, chunk: 256 };

    /// `WFBITZ_DOUBLE` (`0`, `1`, or by default auto), `WFBITZ_DOUBLE_THREADS`
    /// (default 6: measured at `n = 28, 30`, windows lose 1–4 % of the
    /// prove at 1, 2 and 4 threads and win 1.6–4.2 % at 6 and 10),
    /// `WFBITZ_DOUBLE_LOG` (default 20, 16 MiB per half: larger than the
    /// caches), `WFBITZ_DOUBLE_STAY` (default 1; 0 measured the same),
    /// `WFBITZ_DOUBLE_PLAIN0` (default 0; 1 measured slower) and
    /// `WFBITZ_DOUBLE_CHUNK` (default 256 columns: phase B's eight runs are
    /// 32 KiB, half an efficiency core's L1D), read once.
    pub(crate) fn from_env() -> Self {
        static MODE: std::sync::OnceLock<DoubleFold> = std::sync::OnceLock::new();
        *MODE.get_or_init(|| {
            let var = |name: &str| std::env::var(name).ok();
            let number = |name: &str, default: usize| var(name).and_then(|v| v.parse().ok()).unwrap_or(default);
            let when = match var("WFBITZ_DOUBLE").as_deref() {
                Some("0") => When::Never,
                Some("1") => When::Always,
                _ => When::Auto {
                    threads: number("WFBITZ_DOUBLE_THREADS", 6),
                    log: number("WFBITZ_DOUBLE_LOG", 20),
                },
            };
            Self {
                when,
                stay: var("WFBITZ_DOUBLE_STAY").as_deref() != Some("0"),
                plain0: var("WFBITZ_DOUBLE_PLAIN0").as_deref() == Some("1"),
                chunk: number("WFBITZ_DOUBLE_CHUNK", 256).max(1).next_power_of_two(),
            }
        })
    }

    /// Whether a pass over halves of `half` entries with `pending` to fold
    /// takes two rounds, `open` whether an earlier pass of the layer did.
    fn takes(&self, pending: Pending, half: usize, open: bool) -> bool {
        if pending == Pending::None && !self.plain0 {
            return false;
        }
        match self.when {
            When::Never => false,
            When::Always => true,
            When::Auto { threads, log } => {
                #[cfg(feature = "parallel")]
                let have = rayon::current_num_threads();
                #[cfg(not(feature = "parallel"))]
                let have = 1usize;
                have >= threads && ((open && self.stay) || half >= 1usize << log)
            }
        }
    }
}

/// Whether a layer at the point with little-endian coordinates `external`
/// (the low `s` of them column coordinates) can carry `eq_c` in its left
/// half: it needs column coordinates, at least one in-tree round, and
/// every weight invertible, i.e. no column coordinate equal to 0 or 1 —
/// the weights leave the half again before the column rounds.
pub(crate) fn weighable(external: &[Gf], s: usize) -> bool {
    s > 0 && external.len() > s && external[..s].iter().all(|&z| z != Gf::zero() && z != Gf::one())
}

/// `1/x` for every entry (none zero): one inversion and `3(n − 1)`
/// multiplies.
fn batch_inverse(xs: &[Gf]) -> Vec<Gf> {
    let mut prefix = Vec::with_capacity(xs.len());
    let mut acc = Gf::one();
    for &x in xs {
        prefix.push(acc);
        acc = acc * x;
    }
    let mut inv = acc.inverse_or_zero();
    let mut out = vec![Gf::zero(); xs.len()];
    for i in (0..xs.len()).rev() {
        out[i] = inv * prefix[i];
        inv = inv * xs[i];
    }
    out
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
    prove_layer_tensor(ps, point, mle_l, mle_r, skip, factor, next_point, 0, Weighing::Off, DoubleFold::OFF)
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
    weighing: Weighing,
    double: DoubleFold,
) -> (Point, Gf) {
    prove_dense_rounds(ps, point, mle_l, mle_r, skip, Pending::None, factor, next_point, s, weighing, double)
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
/// once per round instead of twice. With `p` folds pending at entry the
/// halves hold `2^{total − first + p}` entries each (the unfolded table).
/// The folded tables occupy the prefixes. With `weighing`, the in-tree
/// rounds keep the column weights in the left half (it arrives so when
/// `Carried`), and the first column round divides them back out of its
/// `2^s` entries.
///
/// Where `double` says so, a pass with a fold pending takes two in-tree
/// rounds at once ([`kernels::window_pass`]) and leaves both challenges
/// pending, the next window folding them with one arity-4 fold. A window
/// needs both rounds in-tree (it never straddles the column bits) and
/// neither coordinate 0 (the value at one would need the ninth grid node);
/// otherwise the round runs alone, after folding the older of two pending
/// challenges in a pass of its own. Every message is the same field
/// element, sent in the same order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_dense_rounds(
    ps: &mut ProverState,
    point: Point,
    mut mle_l: &mut [Gf],
    mut mle_r: &mut [Gf],
    first: usize,
    mut pending: Pending,
    mut factor: Gf,
    mut next_point: VecDeque<Gf>,
    s: usize,
    weighing: Weighing,
    double: DoubleFold,
) -> (Point, Gf) {
    // The eq table a round needs is the one over the coordinates not yet
    // bound, in little-endian (external) order: the external prefix.
    let external: Vec<Gf> = point.iter().rev().copied().collect();
    let total = point.len();
    debug_assert_eq!(mle_l.len(), 1usize << (total - first + pending.bits()));
    let eq_c = if s > 0 && total > s { eq_table(&external[..s]) } else { Vec::new() };
    let one = [Gf::one()];
    let may_weigh = weighing != Weighing::Off && weighable(&external, s);
    let mut weighted = weighing == Weighing::Carried;
    assert!(!weighted || may_weigh, "a weighted left half on a layer that cannot carry weights");
    let point = Vec::from(point);
    // Whether a window has opened on this layer ([`DoubleFold::stay`]).
    let mut open = false;

    let mut round = first;
    while round < total {
        let z = point[round];
        let remaining = total - 1 - round;
        let h = 1usize << remaining;
        let in_tree = s > 0 && remaining >= s;
        if weighted && !in_tree {
            // All in-tree bits are bound: the left half's `2^s` entries are
            // indexed by column. Apply the pending folds and take `eq_c`
            // out; the column rounds weigh their slots as usual.
            match std::mem::replace(&mut pending, Pending::None) {
                Pending::None => {}
                Pending::One(rho) => {
                    fold_halves(mle_l, rho);
                    fold_halves(mle_r, rho);
                }
                two => {
                    kernels::fold_prefix(mle_l, two);
                    kernels::fold_prefix(mle_r, two);
                }
            }
            mle_l = &mut mle_l[..2 * h];
            mle_r = &mut mle_r[..2 * h];
            assert_eq!(mle_l.len(), eq_c.len());
            for (value, inverse) in mle_l.iter_mut().zip(batch_inverse(&eq_c)) {
                *value = *value * inverse;
            }
            weighted = false;
        }

        // Rounds `round` and `round + 1` in one pass: `u` is this round's
        // bit, `v` the next one's, `x″` the `remaining − 1` bits below.
        let z_next = if round + 1 < total { point[round + 1] } else { Gf::zero() };
        if s > 0
            && remaining > s
            && z != Gf::zero()
            && z_next != Gf::zero()
            && double.takes(pending, mle_l.len(), open)
        {
            // The weight of `x″ = (y″, c)` is both rounds' eq weight
            // without its top factors: `eq_y″[y″]·eq_c[c]`.
            let eq_y = eq_table(&external[s..remaining - 1]);
            let weights = kernels::Weights { eq_c: &eq_c, eq_y: &eq_y, split: kernels::split_target() };
            let left = if weighted {
                kernels::Left::Weighted
            } else if may_weigh && pending != Pending::None {
                weighted = true;
                kernels::Left::Weigh
            } else {
                kernels::Left::Plain
            };
            let grid = kernels::window_pass(mle_l, mle_r, pending, weights, left, double.chunk);
            // Both challenges stay pending over the `2h` folded entries.
            mle_l = &mut mle_l[..2 * h];
            mle_r = &mut mle_r[..2 * h];
            let (sum_endpoint, sum_inf) = grid.this_round(z_next);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r0: Gf = ps.verifier_message();
            next_point.push_back(r0);
            factor = factor * eq_factor(r0, z);
            let (sum_endpoint, sum_inf) = grid.next_round(r0);
            ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);
            let r1: Gf = ps.verifier_message();
            next_point.push_back(r1);
            factor = factor * eq_factor(r1, z_next);
            pending = Pending::Two(r0, r1);
            open = true;
            round += 2;
            continue;
        }
        // A single round folds one challenge: of a window's two, the older
        // goes first, in a pass of its own.
        if let Pending::Two(older, newer) = pending {
            let len = kernels::fold_prefix(mle_l, Pending::One(older));
            kernels::fold_prefix(mle_r, Pending::One(older));
            mle_l = &mut mle_l[..len];
            mle_r = &mut mle_r[..len];
            pending = Pending::One(newer);
        }

        // At z = 0 the incoming claim determines the value at zero, so send
        // the value at one. Otherwise send the value at zero as usual.
        let send_one = z == Gf::zero();
        let (eq_flat, eq_y);
        let split = kernels::split_target();
        let weights = if s > 0 && remaining >= s {
            eq_y = eq_table(&external[s..remaining]);
            kernels::Weights { eq_c: &eq_c, eq_y: &eq_y, split }
        } else {
            eq_flat = eq_table(&external[..remaining]);
            kernels::Weights { eq_c: &eq_flat, eq_y: &one, split }
        };

        let (sum_endpoint, sum_inf) = match pending {
            Pending::One(rho) => {
                debug_assert_eq!(mle_l.len(), 4 * h);
                let (q01_l, q23_l) = mle_l.split_at_mut(2 * h);
                let (q0_l, q1_l) = q01_l.split_at_mut(h);
                let (q2_l, q3_l) = q23_l.split_at(h);
                let (q01_r, q23_r) = mle_r.split_at_mut(2 * h);
                let (q0_r, q1_r) = q01_r.split_at_mut(h);
                let (q2_r, q3_r) = q23_r.split_at(h);
                let left = if weighted {
                    kernels::Left::Weighted
                } else if in_tree && may_weigh {
                    weighted = true;
                    kernels::Left::Weigh
                } else {
                    kernels::Left::Plain
                };
                kernels::fused_fold_round(
                    q0_l, q1_l, q2_l, q3_l, q0_r, q1_r, q2_r, q3_r, rho, weights, send_one, left,
                )
            }
            Pending::None => {
                debug_assert!(!weighted, "a weighted left half always has a fold pending");
                debug_assert_eq!(mle_l.len(), 2 * h);
                let (lo_l, hi_l) = mle_l.split_at(h);
                let (lo_r, hi_r) = mle_r.split_at(h);
                kernels::round_sums(lo_l, hi_l, lo_r, hi_r, weights, send_one)
            }
            Pending::Two(..) => unreachable!("folded to one above"),
        };
        ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);

        let r: Gf = ps.verifier_message();
        next_point.push_back(r);
        pending = Pending::One(r);
        // The folded table (once `r` is applied) has `h` entries per half;
        // until then the unfolded `2h` stay.
        mle_l = &mut mle_l[..2 * h];
        mle_r = &mut mle_r[..2 * h];
        factor = factor * eq_factor(r, z);
        round += 1;
    }
    debug_assert!(!weighted, "the column rounds take the weights out");

    // The last round is a single one (a window leaves at least the column
    // rounds after it), so at most one fold is pending.
    match pending {
        Pending::None => {}
        Pending::One(rho) => {
            mle_l[0] = mle_l[0] + rho * (mle_l[1] - mle_l[0]);
            mle_r[0] = mle_r[0] + rho * (mle_r[1] - mle_r[0]);
        }
        Pending::Two(..) => unreachable!("a window never takes the last round"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::transcript::build_prover;

    fn field(state: &mut u64) -> Gf {
        let mut next = || {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        };
        Gf::from_polynomial_words([next(), next()])
    }

    /// The dense rounds send the same bytes and reach the same point and
    /// claim whether the column weights ride in the left half or in every
    /// slot: from a plain start, from a half that arrives weighted with a
    /// fold pending, and when a column coordinate is 0 or 1 (no weighing).
    #[test]
    fn dense_rounds_weigh_identically() {
        for (tree, s) in [(1usize, 3usize), (2, 1), (3, 4), (4, 6), (5, 2), (6, 7), (3, 0), (0, 4)] {
            let total = tree + s;
            for special in [None, Some(Gf::zero()), Some(Gf::one())] {
                let mut state = 0x9E37_79B9 ^ (tree * 97 + s) as u64;
                let mut external: Vec<Gf> = (0..total).map(|_| field(&mut state)).collect();
                if let (Some(value), true) = (special, s > 0) {
                    external[s / 2] = value;
                }
                let point: Point = external.iter().rev().copied().collect();
                let run = |l: &[Gf], r: &[Gf], first: usize, pending: Option<Gf>, weighing: Weighing| {
                    let (mut l, mut r) = (l.to_vec(), r.to_vec());
                    let mut ps = build_prover(b"dense-weigh/v1", b"instance");
                    let next_point: VecDeque<Gf> = point.iter().take(first).copied().collect();
                    let pending = pending.map_or(Pending::None, Pending::One);
                    let (p, claim) = prove_dense_rounds(
                        &mut ps, point.clone(), &mut l, &mut r, first, pending, Gf::one(), next_point, s, weighing,
                        DoubleFold::OFF,
                    );
                    (p, claim, ps.finish().narg_string)
                };
                let label = format!("tree={tree} s={s} special={special:?}");
                let l: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let r: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let want = run(&l, &r, 0, None, Weighing::Off);
                assert!(run(&l, &r, 0, None, Weighing::On) == want, "plain start {label}");
                if tree == 0 {
                    continue;
                }
                // One round in, its challenge pending: `2^total` entries per
                // half, the low `s` index bits the column.
                let rho = field(&mut state);
                let want = run(&l, &r, 1, Some(rho), Weighing::Off);
                assert!(run(&l, &r, 1, Some(rho), Weighing::On) == want, "pending start {label}");
                if weighable(&external, s) {
                    let eq_c = eq_table(&external[..s]);
                    let mask = (1usize << s) - 1;
                    let weighted: Vec<Gf> = l.iter().enumerate().map(|(i, &x)| eq_c[i & mask] * x).collect();
                    assert!(run(&weighted, &r, 1, Some(rho), Weighing::Carried) == want, "carried {label}");
                }
            }
        }
    }

    /// Two rounds per pass prove the same as one: the same bytes, point
    /// and claim — from a plain start (also with a read-only first window),
    /// from a pending fold, from a weighted half; with odd and even in-tree
    /// counts; with a coordinate 0 at, next to and inside the in-tree
    /// rounds (single-round fallbacks) and column coordinates 0 or 1 (a
    /// layer that cannot weigh); with windows that stay open or close at
    /// the size gate, and tasks of every width.
    #[test]
    fn dense_windows_prove_identically() {
        let doubles = [
            DoubleFold { when: When::Always, stay: true, plain0: false, chunk: 256 },
            DoubleFold { when: When::Always, stay: true, plain0: true, chunk: 4 },
            DoubleFold { when: When::Auto { threads: 1, log: 6 }, stay: false, plain0: false, chunk: 64 },
            DoubleFold { when: When::Auto { threads: 1, log: 7 }, stay: true, plain0: true, chunk: 1 },
        ];
        for (tree, s) in [(1usize, 3usize), (2, 1), (3, 4), (4, 6), (5, 2), (6, 7), (7, 3), (8, 1), (9, 2), (3, 0), (0, 4)] {
            let total = tree + s;
            let mut specials: Vec<Option<(usize, Gf)>> = vec![None];
            if s > 0 {
                specials.push(Some((s / 2, Gf::zero())));
                specials.push(Some((s / 2, Gf::one())));
            }
            for k in [s, s + 1, s + 2, total.saturating_sub(2), total - 1] {
                if k >= s && k < total && !specials.contains(&Some((k, Gf::zero()))) {
                    specials.push(Some((k, Gf::zero())));
                }
            }
            for special in specials {
                let mut state = 0x51ED_2701 ^ (tree * 131 + s) as u64;
                let mut external: Vec<Gf> = (0..total).map(|_| field(&mut state)).collect();
                if let Some((k, value)) = special {
                    external[k] = value;
                }
                let point: Point = external.iter().rev().copied().collect();
                let run = |l: &[Gf], r: &[Gf], first: usize, pending: Pending, weighing: Weighing, double: DoubleFold| {
                    let (mut l, mut r) = (l.to_vec(), r.to_vec());
                    let mut ps = build_prover(b"dense-windows/v1", b"instance");
                    let next_point: VecDeque<Gf> = point.iter().take(first).copied().collect();
                    let (p, claim) = prove_dense_rounds(
                        &mut ps, point.clone(), &mut l, &mut r, first, pending, Gf::one(), next_point, s, weighing, double,
                    );
                    (p, claim, ps.finish().narg_string)
                };
                let l: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let r: Vec<Gf> = (0..1usize << total).map(|_| field(&mut state)).collect();
                let rho = field(&mut state);
                for weighing in [Weighing::Off, Weighing::On] {
                    let label = format!("tree={tree} s={s} special={special:?} {weighing:?}");
                    let want = run(&l, &r, 0, Pending::None, weighing, DoubleFold::OFF);
                    for double in doubles {
                        assert!(run(&l, &r, 0, Pending::None, weighing, double) == want, "plain start {label} {double:?}");
                    }
                    if tree == 0 {
                        continue;
                    }
                    let want = run(&l, &r, 1, Pending::One(rho), weighing, DoubleFold::OFF);
                    for double in doubles {
                        assert!(run(&l, &r, 1, Pending::One(rho), weighing, double) == want, "pending start {label} {double:?}");
                    }
                }
                if tree > 0 && weighable(&external, s) {
                    let eq_c = eq_table(&external[..s]);
                    let mask = (1usize << s) - 1;
                    let weighted: Vec<Gf> = l.iter().enumerate().map(|(i, &x)| eq_c[i & mask] * x).collect();
                    let want = run(&l, &r, 1, Pending::One(rho), Weighing::Off, DoubleFold::OFF);
                    for double in doubles {
                        let got = run(&weighted, &r, 1, Pending::One(rho), Weighing::Carried, double);
                        assert!(got == want, "carried tree={tree} s={s} special={special:?} {double:?}");
                    }
                }
            }
        }
    }
}
