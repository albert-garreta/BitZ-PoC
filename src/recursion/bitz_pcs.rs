//! The BitZ PCS verifier ([`crate::bitz::BitZVerifier::verify`]) in the
//! circuit: the fold round (committed folds, their range and reconstruction
//! checks, the images `g^{η_j}`), the batched product GKR, the inner-product
//! sumcheck, the ring switch and the Ligerito opening.
//!
//! Row images `g^{w_i}`, every eq table, the factored GKR exit weights and
//! the ring-switch basis are public (functions of challenges and the claim's
//! public weights) and computed natively.

use std::collections::VecDeque;

use circuit::Circuit;
use field::Gf128 as Gf;
use flock_core::pcs::pack::LOG_PACKING;
use flock_core::pcs::ring_switch::{eval_rs_eq_finish_from_prefix_binary_q, eval_rs_eq_prefix};
use flock_core::zerocheck::univariate_skip::build_eq;

use crate::bitz::grinding::Stage;
use crate::bitz::pcs::{
    CHALLENGES_LABEL, INNER_PRODUCT_STATEMENT_LABEL, MLE_CLAIMS_LABEL, MLE_STATEMENT_LABEL,
    OOD_CLAIM_LABEL, PROOF_HINT_LIMIT, SUMCHECK_LABEL, proof_options,
};
use crate::bitz::{BitZParams, Pcs, Shape};
use crate::ligerito_flock::ood_residual_evals;

use super::arith::{ModTerm, Zq, assert_mod_q, mul};
use super::bitz_transcript::BitzTranscript;
use super::bytes::{Byte, bytes_of_bits, gf_bytes, public_bytes};
use super::ligerito::{self, Challenger, LigeritoCounts};
use super::val::{G, Lc, gf_of, u128_of};

/// A residue's 16 wire bytes (`FqWire`): public, or the committed bits
/// (zero-padded to 128).
pub fn zq_bytes<B: circuit::BoolWitness>(x: &Zq<B>) -> Vec<Byte<B>> {
    match x {
        Zq::P(v) => public_bytes(&v.to_le_bytes()),
        Zq::V(bits) => {
            let mut b = bits.clone();
            b.resize(128, B::from(false));
            bytes_of_bits(&b)
        }
    }
}

/// A linear claim with public weights and a (possibly committed) target.
pub struct ClaimIn<B: circuit::BoolWitness> {
    pub row_weights: Vec<u128>,
    pub column_weights: Vec<u128>,
    pub target: Zq<B>,
}

impl<B: circuit::BoolWitness> ClaimIn<B> {
    /// [`crate::bitz::LinearClaim`]'s encoding: the public weights, then the
    /// target.
    fn encode_parts(&self) -> (Vec<u8>, Vec<Byte<B>>) {
        let mut public = Vec::with_capacity(16 + 16 * (self.row_weights.len() + self.column_weights.len()));
        for weights in [&self.row_weights, &self.column_weights] {
            public.extend_from_slice(&(weights.len() as u64).to_le_bytes());
            for &w in weights {
                public.extend_from_slice(&w.to_le_bytes());
            }
        }
        (public, zq_bytes(&self.target))
    }

    /// Absorbs the encoding.
    fn absorb_into<CS: Circuit<Bool = B>>(&self, cs: &mut CS, t: &mut BitzTranscript<B>) {
        let (public, target) = self.encode_parts();
        t.absorb_split(cs, &public, &target);
    }
}

/// What the fold round leaves for GKR (everything public but the target's
/// congruence).
pub struct FoldOut {
    pub folds: Vec<u128>,
    pub zeta: Vec<Gf>,
    pub e0: Gf,
    pub row_images: Vec<Gf>,
}

/// [`crate::bitz::BitZVerifier::receive_fold`] with PUBLIC folds: each fold
/// is a public value of the outer statement (16 bytes), so its range check,
/// its image `g^{η_j}` and the batched output `e0` are native; the circuit
/// keeps only the congruence `Σ_j v_j η_j ≡ target (mod q)` (one row when
/// the target is committed).
pub fn receive_fold<CS: Circuit>(
    cs: &mut CS,
    params: &BitZParams,
    claim: &ClaimIn<CS::Bool>,
    t: &mut BitzTranscript<CS::Bool>,
) -> FoldOut {
    let shape = params.shape();
    let q = params.q();
    let public = t.public().clone();
    let bound = params.fold_bound();
    let folds: Vec<u128> = (0..shape.columns())
        .map(|_| {
            let raw = t.narg.take(16);
            let value = public
                .borrow_mut()
                .take(128, || u128::from_le_bytes(raw.try_into().unwrap()));
            t.public_message(cs, &value.to_le_bytes());
            value
        })
        .collect();
    if folds.iter().any(|&f| f > bound) {
        public.borrow_mut().fail("fold out of range");
    }
    let terms: Vec<ModTerm<'_, CS::Bool>> = vec![ModTerm { coeff: q - 1, value: &claim.target }];
    let mut constant = 0u128;
    for (&w, &f) in claim.column_weights.iter().zip(&folds) {
        constant = (constant + super::arith::mulmod(w, f % q, q)) % q;
    }
    assert_mod_q(cs, &terms, constant, q, &public, "fold reconstruction");

    let zeta = t.native_vector(cs, Stage::FoldPoint, shape.log_columns());
    let comb = crate::bitz::FixedBasePow::new(params.generator(), 128, crate::bitz::WINDOW);
    let images: Vec<Gf> = folds.iter().map(|&f| comb.pow(f)).collect();
    let e0 = crate::ligerito::mle_eval(&images, &zeta);
    let row_images: Vec<Gf> = claim.row_weights.iter().map(|&w| comb.pow(w)).collect();
    FoldOut {
        folds,
        zeta,
        e0,
        row_images,
    }
}

/// [`crate::bitz::gkr::gpgkr_verify`].
pub fn gkr_verify<CS: Circuit>(
    cs: &mut CS,
    t: &mut BitzTranscript<CS::Bool>,
    claim: G<CS::Bool>,
    point: &[Gf],
    rounds: u32,
) -> (Vec<Gf>, G<CS::Bool>) {
    let public = t.public().clone();
    let mut point: VecDeque<Gf> = point.iter().rev().copied().collect();
    let mut claim = claim;
    for _ in 0..rounds {
        let mut prefix = Gf::ONE;
        let mut next_point: VecDeque<Gf> = VecDeque::new();
        let mut lc = Lc::of(&claim);
        for &z in &point {
            let se = t.prover_gf(cs);
            let si = t.prover_gf(cs);
            let (sum0, sum1) = if z == Gf::ZERO {
                (lc.clone(), Lc::of(&se))
            } else {
                let zinv = z.inverse_or_zero();
                let mut s1 = lc.scaled(zinv);
                s1.add((Gf::ONE + z) * zinv, &se);
                (Lc::of(&se), s1)
            };
            let r = t.native_scalar(cs, Stage::GkrRound);
            next_point.push_back(r);
            let factor = crate::bitz::eq_factor(r, z);
            // claim = factor·(sum0 + r·((sum1 − sum0) + (r − 1)·si)).
            let mut next = sum0.scaled(factor * (Gf::ONE + r));
            next.add_scaled(factor * r, &sum1);
            next.add(factor * r * (r + Gf::ONE), &si);
            lc = next;
            prefix *= factor;
        }
        let l = t.prover_gf(cs);
        let rr = t.prover_gf(cs);
        let product = mul(cs, &l, &rr);
        let mut check = lc;
        check.add(prefix, &product);
        check.assert_zero(cs, &public, "gkr layer product");
        let r2 = t.native_scalar(cs, Stage::GkrClose);
        next_point.push_front(r2);
        let mut next = Lc::of(&l);
        next.add(r2, &l).add(r2, &rr);
        claim = next.settle(cs);
        point = next_point;
    }
    let mut point = Vec::from(point);
    point.reverse();
    (point, claim)
}

/// The weights of an inner-product claim: given, or only their shape and
/// the extension of their product at the sumcheck's point, computed when the
/// point is known (the rounds never read the weights).
pub enum Weights<'a> {
    Given(&'a [Gf], &'a [Gf]),
    Lazy { log_rows: usize, log_cols: usize, eval: Box<dyn FnOnce(&[Gf]) -> Gf + 'a> },
}

impl Weights<'_> {
    fn shape(&self) -> (usize, usize) {
        match self {
            Self::Given(rows, cols) => (rows.len().ilog2() as usize, cols.len().ilog2() as usize),
            Self::Lazy { log_rows, log_cols, .. } => (*log_rows, *log_cols),
        }
    }
}

/// [`crate::bitz::sumcheck::verify`]: returns the point and the committed
/// evaluation.
pub fn sumcheck_verify<CS: Circuit>(
    cs: &mut CS,
    t: &mut BitzTranscript<CS::Bool>,
    weights: Weights<'_>,
    target: &G<CS::Bool>,
) -> (Vec<Gf>, G<CS::Bool>) {
    let public = t.public().clone();
    let (row_rounds, col_rounds) = weights.shape();
    let rounds = row_rounds + col_rounds;
    let mut point = Vec::with_capacity(rounds);
    let mut target = Lc::of(target);
    for _ in 0..rounds {
        let c0 = t.prover_gf(cs);
        let c1 = t.prover_gf(cs);
        let c2 = t.prover_gf(cs);
        let mut check = target.clone();
        check.add(Gf::ONE, &c1).add(Gf::ONE, &c2);
        check.assert_zero(cs, &public, "inner-product sumcheck round");
        let r = t.native_scalar(cs, Stage::BinaryRound);
        let mut next = Lc::of(&c0);
        next.add(r, &c1).add(r * r, &c2);
        target = Lc::of(&next.settle(cs));
        point.push(r);
    }
    // The weights' extensions at the point (rows take the first rounds).
    let weight = match weights {
        Weights::Given(rows, cols) => fold_evaluate(rows, &point[..row_rounds]) * fold_evaluate(cols, &point[row_rounds..]),
        Weights::Lazy { eval, .. } => eval(&point),
    };
    let evaluation = t.prover_gf(cs);
    let mut check = target;
    check.add(weight, &evaluation);
    check.assert_zero(cs, &public, "inner-product sumcheck final");
    (point, evaluation)
}

/// `MLE(values)(point)` by successive folds of the lowest coordinate,
/// `v'[i] = v[2i] + r·(v[2i] + v[2i+1])` (the sumcheck's own fold), on the
/// thread pool while the table is large.
pub fn fold_evaluate(values: &[Gf], point: &[Gf]) -> Gf {
    assert_eq!(values.len(), 1 << point.len(), "one coordinate per halving");
    const PAR: usize = 1 << 14;
    let fold_into = |src: &[Gf], dst: &mut [Gf], r: Gf| {
        #[cfg(feature = "parallel")]
        if dst.len() >= PAR {
            use rayon::prelude::*;
            dst.par_chunks_mut(PAR / 2).zip(src.par_chunks(PAR)).for_each(|(d, s)| {
                for (o, p) in d.iter_mut().zip(s.chunks_exact(2)) {
                    *o = p[0] + r * (p[0] + p[1]);
                }
            });
            return;
        }
        for (o, p) in dst.iter_mut().zip(src.chunks_exact(2)) {
            *o = p[0] + r * (p[0] + p[1]);
        }
    };
    let Some((&first, rest)) = point.split_first() else {
        return values[0];
    };
    let mut table = vec![Gf::ZERO; values.len() / 2];
    fold_into(values, &mut table, first);
    let mut len = table.len();
    let mut scratch = vec![Gf::ZERO; len / 2];
    for &r in rest {
        fold_into(&table[..len], &mut scratch[..len / 2], r);
        len /= 2;
        std::mem::swap(&mut table, &mut scratch);
    }
    table[0]
}

/// Costs of one in-circuit opening, by part.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpeningCounts {
    pub ligerito: LigeritoCounts,
}

/// [`crate::bitz::pcs::Pcs::verify_mle`] after the MLE statement binding.
#[allow(clippy::too_many_arguments)]
pub fn verify_mle<CS: Circuit>(
    cs: &mut CS,
    pcs: &Pcs,
    root: [u8; 32],
    point: &[Gf],
    target: &G<CS::Bool>,
    t: &mut BitzTranscript<CS::Bool>,
    ood: Option<(&[Gf], G<CS::Bool>)>,
) -> OpeningCounts {
    let public = t.public().clone();
    // The Ligerito proof rides in the hint stream (bincode, length-prefixed).
    let proof = if t.hints.has_bytes() {
        let len = u32::from_le_bytes(t.hint_raw(4).try_into().unwrap()) as usize;
        assert!(len <= PROOF_HINT_LIMIT);
        let bytes = t.hint_raw(len);
        Some(
            bincode::Options::deserialize::<flock_core::pcs::ligerito::LigeritoProof>(proof_options(), &bytes)
                .expect("a well-formed Ligerito proof"),
        )
    } else {
        None
    };
    t.public_message(cs, MLE_CLAIMS_LABEL);
    let claims: Vec<G<CS::Bool>> = (0..1usize << LOG_PACKING).map(|_| t.prover_gf(cs)).collect();
    let prefix = build_eq(&point[..LOG_PACKING]);
    let mut check = Lc::of(target);
    for (w, c) in prefix.iter().zip(&claims) {
        check.add(*w, c);
    }
    check.assert_zero(cs, &public, "ring-switch claim check");
    t.public_message(cs, CHALLENGES_LABEL);
    let batching_point = t.native_vector(cs, Stage::RingBatch, LOG_PACKING);
    let weights = build_eq(&batching_point);
    // batch_claims: the 128×128 bit transpose (free rewiring), then the
    // weighted sum.
    let mut packed_target = Lc::new();
    for b in 0..claims.len() {
        let column: circuit::gf128::Gf<CS::Bool> = std::array::from_fn(|i| claims[i].bits()[b].clone());
        packed_target.add_bits(weights[b], &column);
    }
    let suffix_point = &point[LOG_PACKING..];
    let ood_eta = match ood {
        Some((ood_point, y)) => {
            assert_eq!(ood_point.len(), suffix_point.len());
            t.public_message(cs, OOD_CLAIM_LABEL);
            t.public_message(cs, &(ood_point.len() as u64));
            for c in ood_point {
                t.public_message(cs, c);
            }
            t.absorb(cs, &gf_bytes(&y));
            let eta = t.native_scalar(cs, Stage::OodBatch);
            packed_target.add(eta, &y);
            Some((ood_point.to_vec(), eta))
        }
        None => None,
    };
    let packed_target = packed_target.settle(cs);
    let evaluate_basis = |ris: &[Gf], yr_log_n: usize| -> Vec<Gf> {
        let yr_len = 1usize << yr_log_n;
        assert_eq!(ris.len() + yr_log_n, suffix_point.len());
        let prefix = eval_rs_eq_prefix(suffix_point, ris);
        let suffix = &suffix_point[ris.len()..];
        let mut out: Vec<Gf> = (0..yr_len)
            .map(|y| eval_rs_eq_finish_from_prefix_binary_q(&prefix, suffix, y as u32, &weights))
            .collect();
        if let Some((point, eta)) = &ood_eta {
            for (slot, term) in out.iter_mut().zip(ood_residual_evals(ris, yr_log_n, point, *eta)) {
                *slot += term;
            }
        }
        out
    };
    t.finish_native();
    let mut ch = Challenger::new(t, pcs.flock_plan());
    let counts = ligerito::verify(
        cs,
        pcs.verifier_config(),
        proof.as_ref(),
        suffix_point.len(),
        &packed_target,
        root,
        &evaluate_basis,
        &mut ch,
    );
    ch.finish();
    OpeningCounts { ligerito: counts }
}

/// The shared tail of every opening after the GKR exit: the factored
/// inner-product claim (`row/column` weights public, target committed) on
/// the grid the sumcheck runs over, then the ring switch and Ligerito.
#[allow(clippy::too_many_arguments)]
fn finish_opening<CS: Circuit>(
    cs: &mut CS,
    pcs: &Pcs,
    root: [u8; 32],
    weights: Weights<'_>,
    target: &G<CS::Bool>,
    bind: bool,
    t: &mut BitzTranscript<CS::Bool>,
    ood: Option<(&[Gf], G<CS::Bool>)>,
) -> OpeningCounts {
    let binary = weights.shape();
    let native = pcs
        .native_schedule(None, Some(binary), ood.as_ref().map(|(p, _)| (*p, Gf::ZERO)))
        .expect("public native schedule");
    t.continue_native(cs, native);
    if bind {
        let Weights::Given(rows, cols) = &weights else {
            unreachable!("a bound claim carries its weights")
        };
        let (rows, cols): (&[Gf], &[Gf]) = (rows, cols);
        t.public_message(cs, INNER_PRODUCT_STATEMENT_LABEL);
        t.public_message(cs, &root);
        t.public_message(cs, pcs);
        let mut claim_bytes = Vec::with_capacity(16 + 16 * (rows.len() + cols.len()));
        for weights in [rows, cols] {
            claim_bytes.extend_from_slice(&(weights.len() as u64).to_le_bytes());
            for w in weights.iter() {
                claim_bytes.extend_from_slice(&w.to_bytes());
            }
        }
        t.absorb_split(cs, &claim_bytes, &gf_bytes(target));
    }
    t.public_message(cs, SUMCHECK_LABEL);
    let (mle_point, evaluation) = sumcheck_verify(cs, t, weights, target);
    t.public_message(cs, MLE_STATEMENT_LABEL);
    t.public_message(cs, &root);
    t.public_message(cs, pcs);
    t.public_message(cs, &(mle_point.len() as u64));
    for c in &mle_point {
        t.public_message(cs, c);
    }
    t.absorb(cs, &gf_bytes(&evaluation));
    let counts = verify_mle(cs, pcs, root, &mle_point, &evaluation, t, ood);
    t.check_eof();
    counts
}

/// The GKR exit as the factored claim: `u1 = (A(b) − 1)·eq(b, α_b)`,
/// `u2 = eq(·, α_c)`, target `claim − 1`.
fn gkr_exit<CS: Circuit>(
    cs: &mut CS,
    fold: &FoldOut,
    point: Vec<Gf>,
    claim: &G<CS::Bool>,
) -> (Vec<Gf>, Vec<Gf>, G<CS::Bool>) {
    let r1 = fold.row_images.len().max(1).ilog2() as usize;
    let r2 = point.len() - r1;
    let (alfa_c, alfa_b) = point.split_at(r2);
    let eq_b = crate::bitz::gkr::eq_table(alfa_b);
    let u1: Vec<Gf> = fold
        .row_images
        .iter()
        .zip(&eq_b)
        .map(|(a, b)| (*a + Gf::ONE) * *b)
        .collect();
    let u2 = crate::bitz::gkr::eq_table(alfa_c);
    let mut ipc = Lc::of(claim);
    ipc.add_const(Gf::ONE);
    (u1, u2, ipc.settle(cs))
}

/// [`crate::bitz::BitZVerifier::verify`]: the direct opening of a claim.
pub fn verify_direct<CS: Circuit>(
    cs: &mut CS,
    params: &BitZParams,
    pcs: &Pcs,
    root: [u8; 32],
    claim: &ClaimIn<CS::Bool>,
    t: &mut BitzTranscript<CS::Bool>,
    ood: Option<(&[Gf], G<CS::Bool>)>,
) -> OpeningCounts {
    t.public_message(cs, &root);
    t.public_message(cs, params);
    claim.absorb_into(cs, t);
    let shape: Shape = *params.shape();
    let binary = (shape.log_rows(), shape.log_columns());
    let native = pcs
        .native_schedule(Some(shape), Some(binary), ood.as_ref().map(|(p, _)| (*p, Gf::ZERO)))
        .expect("public native schedule");
    t.start_native(cs, native);
    let fold = receive_fold(cs, params, claim, t);
    let (point, gkr_claim) = gkr_verify(cs, t, G::P(fold.e0), &fold.zeta, shape.log_rows() as u32);
    let (u1, u2, ipc) = gkr_exit(cs, &fold, point, &gkr_claim);
    finish_opening(cs, pcs, root, Weights::Given(&u1, &u2), &ipc, true, t, ood)
}

/// [`crate::bitz::BitZVerifier::verify_virtual`]: a claim on the derived
/// grid `h = M·f`, the claim's target committed; the transposition onto the
/// committed grid is public (native, through the statement's map).
pub fn verify_virtual<CS: Circuit, M: circuit::linear_map::binary::VirtualMap>(
    cs: &mut CS,
    statement: &crate::bitz::VirtualStatement<'_, M>,
    pcs: &Pcs,
    root: [u8; 32],
    target: &Zq<CS::Bool>,
    t: &mut BitzTranscript<CS::Bool>,
    ood: Option<(&[Gf], G<CS::Bool>)>,
) -> OpeningCounts {
    let params = statement.claim_params();
    let native_claim = statement.claim();
    let claim = ClaimIn {
        row_weights: native_claim.row_weights().to_vec(),
        column_weights: native_claim.column_weights().to_vec(),
        target: target.clone(),
    };
    t.public_message(cs, crate::bitz::virt::VIRTUAL_STATEMENT_LABEL);
    t.public_message(cs, &root);
    t.public_message(cs, statement.frame().as_slice());
    t.public_message(cs, pcs);
    claim.absorb_into(cs, t);
    let shape: Shape = *params.shape();
    let binary = if statement.is_direct() {
        (shape.log_rows(), shape.log_columns())
    } else {
        (statement.committed_shape().log_bits(), 0)
    };
    let native = pcs
        .native_schedule(Some(shape), Some(binary), ood.as_ref().map(|(p, _)| (*p, Gf::ZERO)))
        .expect("public native schedule");
    t.start_native(cs, native);
    let fold = receive_fold(cs, params, &claim, t);
    let (point, gkr_claim) = gkr_verify(cs, t, G::P(fold.e0), &fold.zeta, shape.log_rows() as u32);
    let (u1, u2, ipc) = gkr_exit(cs, &fold, point, &gkr_claim);
    let factored = crate::bitz::params::LinearClaimGf::from_shape(&shape, u1, u2, Gf::ZERO)
        .expect("factored claim shape");
    // The transposed weights matter only through their extension at the
    // sumcheck's point: computed then (once per prover, see
    // `PublicStream::share`), never formed by a follower.
    let public = t.public().clone();
    let eval = move |point: &[Gf]| -> Gf {
        let value = public.borrow_mut().share(|| {
            let crate::bitz::OpeningQuery::InnerProduct { claim: transposed } = statement
                .transpose_query(crate::bitz::OpeningQuery::InnerProduct { claim: factored })
                .expect("public transposition")
            else {
                unreachable!("the transposition keeps the inner-product form")
            };
            let (rows, cols) = (transposed.row_weights(), transposed.column_weights());
            let split = rows.len().ilog2() as usize;
            u128_of(fold_evaluate(rows, &point[..split]) * fold_evaluate(cols, &point[split..]))
        });
        gf_of(value)
    };
    let weights = Weights::Lazy { log_rows: binary.0, log_cols: binary.1, eval: Box::new(eval) };
    finish_opening(cs, pcs, root, weights, &ipc, false, t, ood)
}
