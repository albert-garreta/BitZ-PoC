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

use super::eq_factor;
use super::transcript::{ProverState, VerifierState};
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

type Point = VecDeque<Gf>;

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
            let eval: Vec<Gf> = l.iter().zip(r).map(|(&a, &b)| a * b).collect();
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
    let mut suffix_table = SuffixTable::new(&point);
    let mut factor = Gf::one();
    let mid = wnext.len() / 2;
    let (mut mle_l, mut mle_r) = wnext.split_at_mut(mid);
    let mut next_point = VecDeque::with_capacity(point.len() + 1);

    for z in point {
        let eq = suffix_table.pop().expect("one table per coordinate");
        let h = mle_l.len() / 2;
        debug_assert_eq!(eq.len(), h);
        let (lo_l, hi_l) = mle_l.split_at_mut(h);
        let (lo_r, hi_r) = mle_r.split_at_mut(h);

        // At z = 0 the incoming claim determines the value at zero, so send
        // the value at one. Otherwise send the value at zero as usual.
        let send_one = z == Gf::zero();
        let mut sum_endpoint = Gf::zero();
        let mut sum_inf = Gf::zero();
        for i in 0..h {
            let (d_l, d_r) = (hi_l[i] - lo_l[i], hi_r[i] - lo_r[i]);
            let (l_endpoint, r_endpoint) = if send_one {
                (hi_l[i], hi_r[i])
            } else {
                (lo_l[i], lo_r[i])
            };
            sum_endpoint += eq[i] * l_endpoint * r_endpoint;
            sum_inf += eq[i] * d_l * d_r;
        }
        ps.prover_message(&[factor * sum_endpoint, factor * sum_inf]);

        let r: Gf = ps.verifier_message();
        next_point.push_back(r);

        for i in 0..h {
            let (d_l, d_r) = (hi_l[i] - lo_l[i], hi_r[i] - lo_r[i]);
            lo_l[i] += r * d_l;
            lo_r[i] += r * d_r;
        }
        mle_l = &mut mle_l[..h];
        mle_r = &mut mle_r[..h];
        factor = factor * eq_factor(r, z);
    }

    ps.prover_message(&[mle_l[0], mle_r[0]]);
    let r: Gf = ps.verifier_message();
    next_point.push_front(r);
    let claim = mle_l[0] + r * (mle_r[0] - mle_l[0]);
    (next_point, claim)
}

/// The eq tables over every suffix of the point past the selector, largest
/// last so `pop` yields the one the next round needs.
struct SuffixTable(Vec<Vec<Gf>>);

impl SuffixTable {
    fn new(point: &Point) -> Self {
        let mut table = Vec::with_capacity(point.len());
        let mut prev = vec![Gf::one()];
        let c = point.range(point.len().min(1)..);
        for &z in c.rev() {
            let size = prev.len() << 1;
            let mut entry = vec![Gf::zero(); size];
            let (low, hi) = entry.split_at_mut(size >> 1);
            for (i, &e) in prev.iter().enumerate() {
                let tmp = z * e;
                low[i] = e - tmp;
                hi[i] = tmp;
            }
            table.push(prev);
            prev = entry;
        }
        table.push(prev);
        Self(table)
    }

    fn pop(&mut self) -> Option<Vec<Gf>> {
        self.0.pop()
    }
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
