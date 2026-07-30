//! **Quad (arity-4) eq-factored sumcheck** for the merged forest's QUAD
//! layers (EXPERIMENTAL, `F2Z_QUAD=1`): proves
//! `Σ_x Σ_t eq(x; q)·scale_t·A_t(x)·B_t(x)·C_t(x)·D_t(x)` — one GKR layer
//! certifying TWO product-tree levels at once (the four multiplicands are
//! the quarters of level ℓ+2). Round polynomials have degree 5 (six
//! nodes `from(0..=5)`); the transcript ops mirror the generic prover
//! exactly (`nvars`/`degree` header, tail `P(1..=5)` absorb, post-draw
//! challenge re-absorb), so [`MLSumcheck::verify_as_subprotocol`] with
//! `degree = 5` verifies the proofs unchanged.
//!
//! Challenges and values live in `GF(2^128)` throughout — the layer merge
//! is SOUND for any generator α (degree-5 soundness 5/|K| per round).
//! Ported from the worktree-gf8 experiment with the order-255 byte-dlog
//! input surfaces stripped: inputs arrive as materialised K vectors, and
//! the win is structural (half the layer passes and line steps over the
//! stored region, even-levels-only chain), not representational. The
//! round bodies are scalar Karatsuba chains (~28 PMULL-class ops/slot) —
//! the fused NEON degree-5 kernel is the known follow-up
//! ([`MLSumcheck`]: crate::piop::sumcheck::MLSumcheck).

use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::transcript::traits::Transcript;
use crate::utils::wide_mul::WideMulAcc;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::SumcheckProof;
use super::prover::{NatEvaluatedPolyWithoutConstant, ProverMsg};

/// One eq-weighted quad group (a tree in phase A; the tree axis itself in
/// phase B): the four multiplicand evaluation vectors over `{0,1}^k`.
pub struct QuadGroup {
    /// The shared eq point (`z_x` / `z_c`). All groups must agree.
    pub q: Vec<Gf>,
    /// Per-group scalar (eq(c, z_c) in phase A; 1 in phase B).
    pub scale: Gf,
    /// The four multiplicands `[Q00, Q10, Q01, Q11]` (`m = a | b≪1`).
    pub bufs: [Vec<Gf>; 4],
}

/// Per-slot degree-4 coefficient accumulation of
/// `Π_{m} (a_m + c·δ_m)` (char 2: `δ = v(0) + v(1)`), Karatsuba over the
/// two pairs, weighted by `w` into the wide accumulators.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
fn quad_slot(
    w: &Gf,
    a: [Gf; 4],
    d: [Gf; 4],
    acc: &mut [<Gf as WideMulAcc>::Wide; 5],
) {
    // (a1 + cδ1)(a2 + cδ2) = p0 + p1 c + p2 c².
    let p0 = a[0] * a[1];
    let p2 = d[0] * d[1];
    let p1 = (a[0] + d[0]) * (a[1] + d[1]) + p0 + p2;
    let q0 = a[2] * a[3];
    let q2 = d[2] * d[3];
    let q1 = (a[2] + d[2]) * (a[3] + d[3]) + q0 + q2;
    // Quartic product coefficients.
    let h0 = p0 * q0;
    let h1 = p0 * q1 + p1 * q0;
    let h2 = p0 * q2 + p1 * q1 + p2 * q0;
    let h3 = p1 * q2 + p2 * q1;
    let h4 = p2 * q2;
    Gf::wide_add_assign(&mut acc[0], &Gf::mul_wide(w, &h0));
    Gf::wide_add_assign(&mut acc[1], &Gf::mul_wide(w, &h1));
    Gf::wide_add_assign(&mut acc[2], &Gf::mul_wide(w, &h2));
    Gf::wide_add_assign(&mut acc[3], &Gf::mul_wide(w, &h3));
    Gf::wide_add_assign(&mut acc[4], &Gf::mul_wide(w, &h4));
}

/// Restructured slot body — the DEFAULT (`F2Z_QUAD_KERNEL=0` restores
/// [`quad_slot`], diagnostic / A-B): the suffix weight is pre-folded into
/// the FIRST pair's operands (`w·a₀`, `w·d₀` — associativity moves it
/// inside the product), the two pair-Karatsubas emit reduced quadratic
/// coefficients, and the quadratic×quadratic cross stage runs 3-segment
/// Karatsuba — SIX wide products instead of nine reduced ones —
/// accumulated UNREDUCED straight into the five coefficient
/// accumulators: the cross stage performs zero reductions. Value-exact
/// vs [`quad_slot`] (associativity + distributivity + the F₂-linear
/// reduction), hence transcript-identical; ~80 PMULL-class ops/slot vs
/// ~125.
#[allow(clippy::arithmetic_side_effects)]
#[inline(always)]
fn quad_slot_k(
    w: &Gf,
    a: [Gf; 4],
    d: [Gf; 4],
    acc: &mut [<Gf as WideMulAcc>::Wide; 5],
) {
    // First pair, w-prefolded: p = (w·A₀)·A₁ coefficients in the round var.
    let wa0 = *w * a[0];
    let wd0 = *w * d[0];
    let p0 = wa0 * a[1];
    let p2 = wd0 * d[1];
    let p1 = (wa0 + wd0) * (a[1] + d[1]) + p0 + p2;
    // Second pair, plain.
    let q0 = a[2] * a[3];
    let q2 = d[2] * d[3];
    let q1 = (a[2] + d[2]) * (a[3] + d[3]) + q0 + q2;
    // Cross stage: h = p·q by 3-segment Karatsuba, all products wide.
    //   h0 = m0; h1 = m01+m0+m1; h2 = m02+m0+m1+m2; h3 = m12+m1+m2;
    //   h4 = m2.
    let m0 = Gf::mul_wide(&p0, &q0);
    let m1 = Gf::mul_wide(&p1, &q1);
    let m2 = Gf::mul_wide(&p2, &q2);
    let m01 = Gf::mul_wide(&(p0 + p1), &(q0 + q1));
    let m02 = Gf::mul_wide(&(p0 + p2), &(q0 + q2));
    let m12 = Gf::mul_wide(&(p1 + p2), &(q1 + q2));
    Gf::wide_add_assign(&mut acc[0], &m0);
    Gf::wide_add_assign(&mut acc[1], &m01);
    Gf::wide_add_assign(&mut acc[1], &m0);
    Gf::wide_add_assign(&mut acc[1], &m1);
    Gf::wide_add_assign(&mut acc[2], &m02);
    Gf::wide_add_assign(&mut acc[2], &m0);
    Gf::wide_add_assign(&mut acc[2], &m1);
    Gf::wide_add_assign(&mut acc[2], &m2);
    Gf::wide_add_assign(&mut acc[3], &m12);
    Gf::wide_add_assign(&mut acc[3], &m1);
    Gf::wide_add_assign(&mut acc[3], &m2);
    Gf::wide_add_assign(&mut acc[4], &m2);
}

/// The restructured-body knob: default ON; `F2Z_QUAD_KERNEL=0` restores
/// the naive slot/node bodies. Read per prove call (NOT once per
/// process) so the byte-identity pin can toggle it in one test process.
fn quad_kernel_on() -> bool {
    std::env::var("F2Z_QUAD_KERNEL").map_or(true, |v| v != "0")
}

/// Prove the quad relation (see the module doc). Returns
/// `(proof, point, finals)` with `finals[t] = [A,B,C,D](point)`.
///
/// `k = q.len()` must be ≥ 1 (the `k = 0` root layer is the caller's
/// phase-B-only special case).
#[allow(clippy::arithmetic_side_effects, clippy::type_complexity)]
pub fn prove_quad_eq_sumcheck(
    transcript: &mut impl Transcript,
    groups: Vec<QuadGroup>,
) -> (SumcheckProof<Gf>, Vec<Gf>, Vec<[Gf; 4]>) {
    let k = groups.first().map_or(0, |g| g.q.len());
    assert!(k >= 1, "quad sumcheck needs >= 1 variable");
    debug_assert!(groups
        .iter()
        .all(|g| g.q == groups[0].q && g.bufs.iter().all(|v| v.len() == 1 << k)));
    let zero = Gf::zero();
    let one = Gf::one();
    let kernel = quad_kernel_on();
    // Six Lagrange nodes 0..=5 (bit-pattern convention) + their power rows
    // for the coefficient → node conversion.
    let nodes: Vec<Gf> = (0u64..6).map(Gf::from).collect();
    let node_pows: Vec<[Gf; 5]> = nodes
        .iter()
        .map(|&c| {
            let c2 = c * c;
            [one, c, c2, c2 * c, c2 * c2]
        })
        .collect();

    let suffix = crate::piop::sumcheck::eq_factored::suffix_tensors(&groups[0].q, &());
    let q_pt = groups[0].q.clone();
    let mut a_scalars: Vec<Gf> = groups.iter().map(|g| g.scale).collect();
    let mut bufs: Vec<[Vec<Gf>; 4]> = groups.into_iter().map(|g| g.bufs).collect();

    let mut buf = vec![0u8; 16];
    transcript.absorb_random_field(&Gf::from(k as u64), &mut buf);
    transcript.absorb_random_field(&Gf::from(5u64), &mut buf);

    let mut randomness: Vec<Gf> = Vec::with_capacity(k);
    let mut messages: Vec<ProverMsg<Gf>> = Vec::with_capacity(k);
    let mut claimed_sum = zero;
    // Pass fusion (the driver's `pending_rho` pattern): a round's fold is
    // deferred into the NEXT round's message pass — one sweep reads the
    // unfolded buffers, folds ρ_{j−1} in registers, accumulates the
    // message over the folded quads, and lands the folded values in
    // place.
    let mut pending_rho: Option<Gf> = None;

    for j in 1..=k {
        let half = 1usize << (k - j);
        let suffix_j = &suffix[j - 1];

        let hs: Vec<[Gf; 5]> = if let Some(rho_prev) = pending_rho.take() {
            // Fused fold + message: buffers hold 4·half unfolded entries.
            let fused = |b: &mut [Vec<Gf>; 4]| -> [Gf; 5] {
                let mut acc = [
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                ];
                for s in 0..half {
                    let base = s << 2;
                    let mut a = [zero; 4];
                    let mut d = [zero; 4];
                    for m in 0..4 {
                        let v = &mut b[m];
                        let f0 = v[base] + rho_prev * (v[base] + v[base + 1]);
                        let f1 = v[base + 2] + rho_prev * (v[base + 2] + v[base + 3]);
                        a[m] = f0;
                        d[m] = f1 + f0;
                        let e = s << 1;
                        v[e] = f0;
                        v[e | 1] = f1;
                    }
                    if kernel {
                        quad_slot_k(&suffix_j[s], a, d, &mut acc);
                    } else {
                        quad_slot(&suffix_j[s], a, d, &mut acc);
                    }
                }
                for v in b.iter_mut() {
                    v.truncate(half << 1);
                }
                acc.map(Gf::from_wide)
            };
            #[cfg(feature = "parallel")]
            {
                let min_len = (512usize / half.max(1)).max(1);
                bufs.par_iter_mut().with_min_len(min_len).map(fused).collect()
            }
            #[cfg(not(feature = "parallel"))]
            {
                bufs.iter_mut().map(fused).collect()
            }
        } else {
            // Round 1: plain message over the (2·half)-entry buffers — no
            // fold yet.
            let compute = |b: &[Vec<Gf>; 4]| -> [Gf; 5] {
                let mut acc = [
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                    Gf::wide_zero(&zero),
                ];
                for s in 0..half {
                    let e = s << 1;
                    let mut a = [zero; 4];
                    let mut d = [zero; 4];
                    for m in 0..4 {
                        let v0 = b[m][e];
                        a[m] = v0;
                        d[m] = b[m][e | 1] + v0;
                    }
                    if kernel {
                        quad_slot_k(&suffix_j[s], a, d, &mut acc);
                    } else {
                        quad_slot(&suffix_j[s], a, d, &mut acc);
                    }
                }
                acc.map(Gf::from_wide)
            };
            #[cfg(feature = "parallel")]
            {
                let min_len = (512usize / half.max(1)).max(1);
                bufs.par_iter().with_min_len(min_len).map(compute).collect()
            }
            #[cfg(not(feature = "parallel"))]
            {
                bufs.iter().map(compute).collect()
            }
        };

        // M(c) = Σ_t A_t·eq1(c; q[j−1])·H_t(c) at the six nodes.
        let qj = &q_pt[j - 1];
        let e0 = one + *qj;
        let mut m_nodes = [zero; 6];
        if kernel {
            // Σ_t a_t·eq1(c)·H_t(c) = eq1(c)·Σ_t (a_t·H_t)(c): fold a_t
            // into the coefficients once per group (5 muls), evaluate
            // nodes 0/1 mul-free (c = 0 → h₀; c = 1 → Σ h_i, char-2
            // bit-pattern nodes), dot the power rows for the rest, and
            // apply the node factor eq1(c) once per node AFTER the group
            // sum. Value-exact (distributivity); ~21 muls per group
            // instead of ~42.
            for (t, h) in hs.iter().enumerate() {
                let a_t = a_scalars[t];
                let ah: [Gf; 5] =
                    [a_t * h[0], a_t * h[1], a_t * h[2], a_t * h[3], a_t * h[4]];
                m_nodes[0] += ah[0];
                m_nodes[1] += ah[0] + ah[1] + ah[2] + ah[3] + ah[4];
                for (c, slot) in m_nodes.iter_mut().enumerate().skip(2) {
                    let pw = &node_pows[c];
                    let mut hv = ah[0];
                    for (i, ahi) in ah.iter().enumerate().skip(1) {
                        hv += *ahi * pw[i];
                    }
                    *slot += hv;
                }
            }
            for (c, slot) in m_nodes.iter_mut().enumerate() {
                let cn = nodes[c];
                let eq1 = e0 * (one + cn) + *qj * cn;
                *slot = eq1 * *slot;
            }
        } else {
            for (t, h) in hs.iter().enumerate() {
                let a_t = a_scalars[t];
                for (c, slot) in m_nodes.iter_mut().enumerate() {
                    let cn = nodes[c];
                    let eq1 = e0 * (one + cn) + *qj * cn;
                    let pw = &node_pows[c];
                    let mut hv = zero;
                    for (i, hi) in h.iter().enumerate() {
                        hv += *hi * pw[i];
                    }
                    *slot += a_t * eq1 * hv;
                }
            }
        }

        if j == 1 {
            claimed_sum = m_nodes[0] + m_nodes[1];
        }
        let tail: Vec<Gf> = m_nodes[1..].to_vec();
        transcript.absorb_random_field_slice(&tail, &mut buf);
        messages.push(ProverMsg(NatEvaluatedPolyWithoutConstant::new(tail)));

        let rho: Gf = transcript.get_field_challenge(&());
        transcript.absorb_random_field(&rho, &mut buf);

        for a in a_scalars.iter_mut() {
            let e = e0 * (one + rho) + *qj * rho;
            *a = *a * e;
        }

        if j < k {
            // Defer the fold into the next round's fused message pass.
            pending_rho = Some(rho);
            randomness.push(rho);
        } else {
            // Final interpolation at ρ_k: after the round-k message pass
            // (which applied any pending fold) the buffers hold TWO
            // folded entries per multiplicand.
            let finals: Vec<[Gf; 4]> = bufs
                .iter()
                .map(|b| {
                    let interp = |v: &[Gf]| -> Gf { v[0] + rho * (v[0] + v[1]) };
                    [interp(&b[0]), interp(&b[1]), interp(&b[2]), interp(&b[3])]
                })
                .collect();
            randomness.push(rho);
            return (SumcheckProof { messages, claimed_sum }, randomness, finals);
        }
    }
    unreachable!("the final round returns")
}
