//! Structured-taps A/B (EXPERIMENTAL, docs/rlc-structured-taps-phase0.md;
//! corrected semantics: 32-bit words along the ENTRY axis of W=1
//! bit-vectors, g = 5): the j = 2, k = 6 ROT/SHIFT/word-offset instance
//! proved four ways against ONE commitment and statement —
//!
//! * `single` — one identity claim alone via the batched tap-claims path
//!   (the cost unit);
//! * `tapf`   — the clustered stream family (`{b1,b3,b5}` over 6 streams
//!   and `{b2,b4,b6}` over 7; two eager case forests, per-cluster
//!   cascades, translated-eq openings, ONE proof);
//! * `vx6`    — all six claims through the batched tap-claims path (ONE
//!   8·2^s-tree padded forest + per-claim translated-eq openings);
//! * `ind6`   — six independent single-claim tap proofs.
//!
//! Alternated in-window reps, medians reported (repo measurement
//! protocol; small-n caveat: FS grinding luck is deterministic per
//! statement — vary `F2Z_TAPS_SEED` and average over statements before
//! reading small deltas). Verifies every variant once. With
//! `OBLONG_PROFILE=1` one extra profiled prove of `tapf` and `vx6` dumps
//! the phase tree per shape. `F2Z_AB_NO_FAMILY=1` skips the stream
//! family (its eager case forests are the memory hog — this unlocks the
//! n = 28 row for the single/vx6/ind6 comparison).
//!
//! `F2Z_AB_COLLAPSE=1` runs the SINGLE-TAP shared-point demo instead:
//! the instance's 13 deduped streams as 13 individual claims at ONE
//! point — `clp` (the weight-transform collapse, ≤ 4 inner claims) vs
//! `vx13` (13 batched tap claims, pads to 16 tree-sets) vs `ind13`.
//!
//! `F2Z_AB_MIX6=1` runs the uniform-op k = 6 variant of the original
//! instance at ONE point: 2 identity claims on the source columns plus
//! 4 claims `off^t(ROT^7(a₀ ⊕ a₁))`, t = 0..3 — the op stays OUTSIDE
//! the XOR, so the whole set is 0x44 territory (4 plain inner bodies,
//! no rings) — `clp` (the collapse) vs `vx6m` (the blocked batched tap
//! path on the composed lists) vs `ind6m`.
//!
//! `F2Z_AB_COLS4=1` runs a FOUR-column config (log_cols = 2): identity
//! claims on a₁..a₄ plus the XOR-mixed pairs `ROT¹(a₁) ⊕ off¹(a₂)` and
//! `ROT²(a₃) ⊕ off¹(a₄)`, one shared point, through the blocked
//! batched tap path — `single` vs `vx6` vs `ind6` (δ applies: the
//! sources are unconstrained on the 0x42 route).
//!
//! `F2Z_AB_SCHED=1` runs the COMPOSED-collapse schedule demo instead:
//! 48 claims `off^t(x)` of ONE σ-style mixed combination
//! `x = ROT^7 a_0 ⊕ ROT^18 a_0 ⊕ SHIFT^3 a_0 ⊕ off^1 a_1` at ONE point
//! — `cmp` (the composed collapse, 2 inner tap bodies TOTAL) vs `vx48`
//! (the batched tap-claims path on the offset-folded lists; 24 blocked
//! 2-set forests since the block cap landed) vs `ind48`. Claim values
//! are computed through the FOLDED-list extraction route, so every
//! verified rep doubles as a distributed-extraction cross-check of the
//! weight-transform algebra. `F2Z_AB_ROUNDS` overrides the round count.
//!
//! ```text
//! F2Z_AB_N="22 24 26" F2Z_AB_REPS=5 RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --example taps_ab --features unchecked
//! ```

use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{
    RlcFamilyClaim, TapClaim, TapComposedClaim, TapFamilyCluster, TapPointClaim, TapVerifyClaim,
    commit_rs_ligerito_rows, mle_eval_mod_q_lig_tap_family_size_breakdown,
    mle_eval_mod_q_lig_tap_size_breakdown, mle_eval_mod_q_lig_xor_proof_size_bytes,
    prove_mle_eval_mod_q_ligerito_tap_claims, prove_mle_eval_mod_q_ligerito_tap_collapse,
    prove_mle_eval_mod_q_ligerito_tap_composed, prove_mle_eval_mod_q_ligerito_tap_family,
    sha_lig_configs, verify_mle_eval_mod_q_ligerito_tap_claims,
    verify_mle_eval_mod_q_ligerito_tap_collapse, verify_mle_eval_mod_q_ligerito_tap_composed,
    verify_mle_eval_mod_q_ligerito_tap_family,
};
use f2z::pcs::{FQ_BITS, FQ_MOD, Fq, IntEvalParams, ShaF2Layout, smallest_generator, virtual_xor_params};
use f2z::taps::{TapOp, extract_virtual_tap_rows};
use f2z::transcript::Blake3Transcript;
use std::time::Instant;

/// The instance's word-group width: 32-bit words along the entry axis.
const GRP: usize = 5;

/// n → the taps layout: 2 UAIR bit-columns (log_cols = 1, W = 1,
/// bit_vars = 0) over 2^{n−1} entries; x split t' vs s as even as
/// `tw ≥ 6` allows (the 32-bit group field lives in the clear axis:
/// s ≥ GRP + 2 so word offsets ≤ 2 stay in range). `F2Z_TAPS_DELTA`
/// sets `x_fold_extra` (the sched/vx paths only; δ ≤ g so collapse
/// outers with `2^δ | amt` stay in envelope — the schedule's pure-off
/// outers always are; the stream family requires δ = 0).
fn taps_layout(n: usize, log_cols: usize) -> ShaF2Layout {
    let tw = ((n - log_cols) / 2).max(6);
    let s = n - log_cols - tw;
    assert!(s >= GRP + 2, "clear axis must hold the group field plus offsets");
    let delta: usize = std::env::var("F2Z_TAPS_DELTA").map_or(0, |v| v.parse().unwrap());
    assert!(delta <= GRP, "F2Z_TAPS_DELTA must be ≤ g = {GRP}");
    ShaF2Layout {
        p: IntEvalParams { t: log_cols + tw, s, word_bits: 1 },
        num_cols: 1 << log_cols,
        log_cols,
        bit_vars: 0,
        num_vars: tw + s,
        tw,
        x_fold_extra: delta,
    }
}

/// The k = 6 instance's tap lists (32-bit entry-axis words): identities,
/// two three-tap single-column rotation convolutions, a cross-column
/// mix, and the lossy-SHIFT claim — the pinned spec of the session
/// prompt under the corrected semantics.
fn instance_claims() -> Vec<Vec<TapOp>> {
    let rot =
        |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: false, off };
    let shl =
        |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: true, off };
    vec![
        vec![TapOp::ident(0)],
        vec![TapOp::ident(1)],
        vec![rot(0, 1, 0), rot(0, 2, 1), rot(0, 3, 2)],
        vec![rot(1, 2, 0), rot(1, 5, 1), rot(1, 7, 2)],
        vec![rot(0, 1, 0), rot(1, 4, 0), rot(0, 6, 1)],
        vec![shl(0, 3, 0), shl(1, 5, 1), rot(1, 2, 2)],
    ]
}

/// The pinned two-cluster split: `{b1, b3, b5}` (6 streams, S1 shared) and
/// `{b2, b4, b6}` (7 streams).
#[allow(clippy::type_complexity)]
fn instance_clusters() -> (Vec<Vec<TapOp>>, Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let rot =
        |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: false, off };
    let shl =
        |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: true, off };
    let streams1 =
        vec![TapOp::ident(0), rot(0, 1, 0), rot(0, 2, 1), rot(0, 3, 2), rot(1, 4, 0), rot(0, 6, 1)];
    let streams2 = vec![
        TapOp::ident(1),
        rot(1, 2, 0),
        rot(1, 5, 1),
        rot(1, 7, 2),
        shl(0, 3, 0),
        shl(1, 5, 1),
        rot(1, 2, 2),
    ];
    let forms1 = vec![0b000001usize, 0b001110, 0b110010];
    let forms2 = vec![0b0000001usize, 0b0001110, 0b1110000];
    (vec![streams1, streams2], vec![forms1, forms2], vec![vec![0, 2, 4], vec![1, 3, 5]])
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let alpha = smallest_generator();
    let ns: Vec<usize> = std::env::var("F2Z_AB_N")
        .map(|v| v.split_whitespace().map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|_| vec![22, 24]);
    let reps: usize = std::env::var("F2Z_AB_REPS").map_or(5, |v| v.parse().unwrap());
    let seed: u64 = std::env::var("F2Z_TAPS_SEED").map_or(1, |v| v.parse().unwrap());
    let profile = std::env::var("OBLONG_PROFILE").is_ok_and(|v| v == "1");

    for &n in &ns {
        let layout = taps_layout(n, 1);
        let p = &layout.p;
        let p_x = virtual_xor_params(&layout);
        let m_p = packed_vars(p);
        let (pc, vc) = sha_lig_configs(m_p).expect("lig cfg");

        let words = p.rows().div_ceil(64);
        let rows: Vec<Vec<u64>> = (0..p.cols())
            .map(|c| {
                (0..words)
                    .map(|w| {
                        ((c as u64) << 32 | (w as u64) ^ seed)
                            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            .rotate_left(((c + w) & 63) as u32)
                    })
                    .collect()
            })
            .collect();
        let hint = commit_rs_ligerito_rows(p, rows, &pc);

        if std::env::var("F2Z_AB_SCHED").is_ok_and(|v| v == "1") {
            // The composed collapse on a schedule-shaped instance: every
            // claim `off^t(x)` of ONE σ-style mixed combination — 2
            // inner tap bodies TOTAL — against the batched path on the
            // offset-folded lists (pads to the next power of two) and
            // independent proofs.
            let rounds: usize =
                std::env::var("F2Z_AB_ROUNDS").map_or(48, |v| v.parse().unwrap());
            let rot = |col, amt, off| TapOp {
                col,
                grp_log2: GRP,
                bit_amt: amt,
                bit_dropout: false,
                off,
            };
            let shl =
                |col, amt, off| TapOp { col, grp_log2: GRP, bit_amt: amt, bit_dropout: true, off };
            let src = vec![rot(0, 7, 0), rot(0, 18, 0), shl(0, 3, 0), rot(1, 0, 1)];
            // The composed path needs `rounds − 1 < 2^{s−g}` alone; the
            // FOLDED baseline additionally eats the source's own word
            // offset (its envelope is strictly narrower).
            let max_src_off = src.iter().map(|t| t.off).max().unwrap_or(0);
            assert!(
                rounds + max_src_off <= 1usize << (layout.p.s - GRP),
                "folded-baseline offsets out of range for this shape"
            );
            let rw: Vec<u128> = (0..p_x.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(43 + (u128::from(seed) << 1))
                        % FQ_MOD
                })
                .collect();
            let colw: Vec<Fq> = (0..p_x.cols())
                .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
                .collect();
            let eval_list = |taps: &[TapOp]| -> u128 {
                let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), taps);
                let mut y = Fq::from(0u128);
                for (c, row) in a_rows.iter().enumerate() {
                    let mut acc = Fq::from(0u128);
                    for (wi, &word) in row.iter().enumerate() {
                        let mut bits = word;
                        while bits != 0 {
                            let t = bits.trailing_zeros() as usize;
                            acc = acc + Fq::from(rw[(wi << 6) | t]);
                            bits &= bits.wrapping_sub(1);
                        }
                    }
                    y = y + colw[c] * acc;
                }
                y.0
            };
            // The folded lists (`off += t` per tap) drive both the
            // baseline paths AND the claim values — the independent
            // extraction route the composed algebra must reproduce.
            let folded: Vec<Vec<TapOp>> = (0..rounds)
                .map(|t| {
                    src.iter()
                        .map(|tap| {
                            let mut tap = *tap;
                            tap.off += t;
                            tap
                        })
                        .collect()
                })
                .collect();
            let vals: Vec<u128> = folded.iter().map(|taps| eval_list(taps)).collect();
            let cclaims: Vec<TapComposedClaim<'_>> = (0..rounds)
                .map(|t| TapComposedClaim {
                    source: &src,
                    outer: f2z::taps::TapUniOp {
                        grp_log2: GRP,
                        bit_amt: 0,
                        bit_dropout: false,
                        off: t,
                    },
                    claimed: vals[t],
                })
                .collect();
            let tclaims: Vec<TapClaim<'_>> = folded
                .iter()
                .map(|taps| TapClaim { taps, row_weights_q: &rw })
                .collect();
            let tvclaims: Vec<TapVerifyClaim<'_, Fq>> = folded
                .iter()
                .zip(vals.iter())
                .map(|(taps, &v)| TapVerifyClaim {
                    taps,
                    row_weights_q: &rw,
                    col_weights: &colw,
                    claimed: Fq::from(v),
                })
                .collect();
            // The block cap flattened the batched path's working set
            // (peak ≈ 2 tree-sets for any k), so vx48 runs at every n.
            let run_vx = true;
            let prove_cmp = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_composed(
                    &mut t, &hint, &layout, &rw, &colw, &cclaims, alpha, &pc,
                )
            };
            let prove_vx = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_claims(
                    &mut t, &hint, &layout, FQ_BITS, &tclaims, alpha, &pc,
                )
            };
            let prove_ind = || {
                (0..tclaims.len())
                    .map(|i| {
                        let mut t = Blake3Transcript::new();
                        prove_mle_eval_mod_q_ligerito_tap_claims(
                            &mut t,
                            &hint,
                            &layout,
                            FQ_BITS,
                            &tclaims[i..i + 1],
                            alpha,
                            &pc,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let (mut t_cmp, mut t_vx, mut t_ind) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..reps {
                let t0 = Instant::now();
                drop(prove_cmp());
                t_cmp.push(t0.elapsed().as_secs_f64() * 1e3);
                if run_vx {
                    let t0 = Instant::now();
                    drop(prove_vx());
                    t_vx.push(t0.elapsed().as_secs_f64() * 1e3);
                }
                let t0 = Instant::now();
                drop(prove_ind());
                t_ind.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            let proof_cmp = prove_cmp();
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_composed(
                    &mut vt, &hint.commitment, &proof_cmp, &layout, &rw, &colw, &cclaims,
                    alpha, &vc,
                )
                .expect("composed schedule verifies");
            }
            let v_cmp = t0.elapsed().as_secs_f64() * 1e3;
            let size_tap = |p: &f2z::ligerito_flock::IntEvalRsLigModQTapProof| {
                let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(p);
                b.total() + lig
            };
            let sz_cmp = size_tap(&proof_cmp);
            let (m_cmp, m_ind) = (median(t_cmp), median(t_ind));
            let vx_txt = if run_vx {
                let proof_vx = prove_vx();
                let t0 = Instant::now();
                {
                    let mut vt = Blake3Transcript::new();
                    verify_mle_eval_mod_q_ligerito_tap_claims(
                        &mut vt, &hint.commitment, &proof_vx, &layout, alpha, FQ_BITS,
                        &tvclaims, &vc,
                    )
                    .expect("vx schedule verifies");
                }
                let v_vx = t0.elapsed().as_secs_f64() * 1e3;
                let m_vx = median(t_vx);
                format!(
                    "vx{rounds} {m_vx:.1} ms ({:.1}x of cmp, {:.0} KB, verify {v_vx:.1} ms)",
                    m_vx / m_cmp,
                    size_tap(&proof_vx) as f64 / 1e3,
                )
            } else {
                format!("vx{rounds} skipped (64-set pad vs memory)")
            };
            println!(
                "n={n} SCHED ({rounds} off^t claims of one mixed source): cmp {m_cmp:.1} ms \
                 ({} inner bodies, {:.0} KB, verify {v_cmp:.1} ms) | {vx_txt} | ind{rounds} \
                 {m_ind:.1} ms ({:.1}x of cmp)",
                proof_cmp.tap_us.len(),
                sz_cmp as f64 / 1e3,
                m_ind / m_cmp,
            );
            continue;
        }

        if std::env::var("F2Z_AB_COLS4").is_ok_and(|v| v == "1") {
            // FOUR committed columns (log_cols = 2): identity claims on
            // each plus the XOR-mixed pairs b₁ = ROT¹(a₁) ⊕ off¹(a₂)
            // and b₂ = ROT²(a₃) ⊕ off¹(a₄) (statement columns
            // 1-indexed; code 0-indexed), one shared point, through the
            // blocked batched tap path — `single` (one identity claim)
            // as the unit vs `vx6` (all six, blocks 2+2+2) vs `ind6`.
            let layout = taps_layout(n, 2);
            let p = &layout.p;
            let p_x = virtual_xor_params(&layout);
            let (pc, vc) = sha_lig_configs(packed_vars(p)).expect("lig cfg");
            let words = p.rows().div_ceil(64);
            let rows: Vec<Vec<u64>> = (0..p.cols())
                .map(|c| {
                    (0..words)
                        .map(|w| {
                            ((c as u64) << 32 | (w as u64) ^ seed)
                                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                .rotate_left(((c + w) & 63) as u32)
                        })
                        .collect()
                })
                .collect();
            let hint = commit_rs_ligerito_rows(p, rows, &pc);
            let rot = |col, amt, off| TapOp {
                col,
                grp_log2: GRP,
                bit_amt: amt,
                bit_dropout: false,
                off,
            };
            let claim_taps: Vec<Vec<TapOp>> = vec![
                vec![TapOp::ident(0)],
                vec![TapOp::ident(1)],
                vec![TapOp::ident(2)],
                vec![TapOp::ident(3)],
                vec![rot(0, 1, 0), rot(1, 0, 1)],
                vec![rot(2, 2, 0), rot(3, 0, 1)],
            ];
            let rw: Vec<u128> = (0..p_x.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(61 + (u128::from(seed) << 1))
                        % FQ_MOD
                })
                .collect();
            let colw: Vec<Fq> = (0..p_x.cols())
                .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
                .collect();
            let vals: Vec<u128> = claim_taps
                .iter()
                .map(|taps| {
                    let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), taps);
                    let mut y = Fq::from(0u128);
                    for (c, row) in a_rows.iter().enumerate() {
                        let mut acc = Fq::from(0u128);
                        for (wi, &word) in row.iter().enumerate() {
                            let mut bits = word;
                            while bits != 0 {
                                let t = bits.trailing_zeros() as usize;
                                acc = acc + Fq::from(rw[(wi << 6) | t]);
                                bits &= bits.wrapping_sub(1);
                            }
                        }
                        y = y + colw[c] * acc;
                    }
                    y.0
                })
                .collect();
            let tclaims: Vec<TapClaim<'_>> = claim_taps
                .iter()
                .map(|taps| TapClaim { taps, row_weights_q: &rw })
                .collect();
            let tvclaims: Vec<TapVerifyClaim<'_, Fq>> = claim_taps
                .iter()
                .zip(vals.iter())
                .map(|(taps, &v)| TapVerifyClaim {
                    taps,
                    row_weights_q: &rw,
                    col_weights: &colw,
                    claimed: Fq::from(v),
                })
                .collect();
            let prove_k = |r: core::ops::Range<usize>| {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_claims(
                    &mut t, &hint, &layout, FQ_BITS, &tclaims[r], alpha, &pc,
                )
            };
            let prove_ind = || {
                (0..tclaims.len()).map(|i| prove_k(i..i + 1)).collect::<Vec<_>>()
            };
            let (mut t_single, mut t_vx, mut t_ind) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..reps {
                let t0 = Instant::now();
                drop(prove_k(0..1));
                t_single.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_k(0..tclaims.len()));
                t_vx.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_ind());
                t_ind.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            let proof_vx = prove_k(0..tclaims.len());
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_claims(
                    &mut vt, &hint.commitment, &proof_vx, &layout, alpha, FQ_BITS,
                    &tvclaims, &vc,
                )
                .expect("cols4 vx verifies");
            }
            let v_vx = t0.elapsed().as_secs_f64() * 1e3;
            {
                let mut vt = Blake3Transcript::new();
                let proof_s = prove_k(0..1);
                verify_mle_eval_mod_q_ligerito_tap_claims(
                    &mut vt, &hint.commitment, &proof_s, &layout, alpha, FQ_BITS,
                    &tvclaims[0..1], &vc,
                )
                .expect("cols4 single verifies");
            }
            let size_tap = |p: &f2z::ligerito_flock::IntEvalRsLigModQTapProof| {
                let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(p);
                b.total() + lig
            };
            let (m_s, m_vx, m_ind) = (median(t_single), median(t_vx), median(t_ind));
            println!(
                "n={n} COLS4 (t'={}, s={}, 4 cols: 4 identities + 2 mixed pairs): single \
                 {m_s:.1} ms | vx6 {m_vx:.1} ms ({:.2}x of single, {:.0} KB, verify \
                 {v_vx:.1} ms) | ind6 {m_ind:.1} ms ({:.2}x of vx6)",
                p_x.t,
                p_x.s,
                m_vx / m_s,
                size_tap(&proof_vx) as f64 / 1e3,
                m_ind / m_vx,
            );
            continue;
        }

        if std::env::var("F2Z_AB_MIX6").is_ok_and(|v| v == "1") {
            // The uniform-op k=6 variant: 2 identities + 4 offsets of
            // ROT^7(a₀⊕a₁) at ONE point. ROT^7 is off the δ-envelope
            // for the collapse route (7 ∤ 2^δ), so this mode runs δ=0.
            assert_eq!(
                layout.x_fold_extra, 0,
                "mix6's ROT^7 outer is off the δ-envelope; unset F2Z_TAPS_DELTA"
            );
            let rot = |col, amt, off| TapOp {
                col,
                grp_log2: GRP,
                bit_amt: amt,
                bit_dropout: false,
                off,
            };
            let uni = |amt: usize, off: usize| f2z::taps::TapUniOp {
                grp_log2: GRP,
                bit_amt: amt,
                bit_dropout: false,
                off,
            };
            let rw: Vec<u128> = (0..p_x.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(53 + (u128::from(seed) << 1))
                        % FQ_MOD
                })
                .collect();
            let colw: Vec<Fq> = (0..p_x.cols())
                .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
                .collect();
            let eval_list = |taps: &[TapOp]| -> u128 {
                let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), taps);
                let mut y = Fq::from(0u128);
                for (c, row) in a_rows.iter().enumerate() {
                    let mut acc = Fq::from(0u128);
                    for (wi, &word) in row.iter().enumerate() {
                        let mut bits = word;
                        while bits != 0 {
                            let t = bits.trailing_zeros() as usize;
                            acc = acc + Fq::from(rw[(wi << 6) | t]);
                            bits &= bits.wrapping_sub(1);
                        }
                    }
                    y = y + colw[c] * acc;
                }
                y.0
            };
            // Claim shapes: (XOR set, uniform op); the batched/ind
            // baselines run the composed per-column tap lists, which
            // also supply the claim values (the distributed
            // cross-check route).
            let sets: Vec<Vec<usize>> =
                vec![vec![0], vec![1], vec![0, 1], vec![0, 1], vec![0, 1], vec![0, 1]];
            let ops: Vec<f2z::taps::TapUniOp> = vec![
                uni(0, 0),
                uni(0, 0),
                uni(7, 0),
                uni(7, 1),
                uni(7, 2),
                uni(7, 3),
            ];
            let lists: Vec<Vec<TapOp>> = sets
                .iter()
                .zip(ops.iter())
                .map(|(set, op)| {
                    set.iter().map(|&c| rot(c, op.bit_amt, op.off)).collect()
                })
                .collect();
            let vals: Vec<u128> = lists.iter().map(|taps| eval_list(taps)).collect();
            let pclaims: Vec<TapPointClaim<'_>> = sets
                .iter()
                .zip(ops.iter())
                .zip(vals.iter())
                .map(|((set, &op), &claimed)| TapPointClaim { cols: set, op, claimed })
                .collect();
            let tclaims: Vec<TapClaim<'_>> = lists
                .iter()
                .map(|taps| TapClaim { taps, row_weights_q: &rw })
                .collect();
            let tvclaims: Vec<TapVerifyClaim<'_, Fq>> = lists
                .iter()
                .zip(vals.iter())
                .map(|(taps, &v)| TapVerifyClaim {
                    taps,
                    row_weights_q: &rw,
                    col_weights: &colw,
                    claimed: Fq::from(v),
                })
                .collect();
            let prove_clp = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_collapse(
                    &mut t, &hint, &layout, &rw, &colw, &pclaims, alpha, &pc,
                )
            };
            let prove_vx = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_claims(
                    &mut t, &hint, &layout, FQ_BITS, &tclaims, alpha, &pc,
                )
            };
            let prove_ind = || {
                (0..tclaims.len())
                    .map(|i| {
                        let mut t = Blake3Transcript::new();
                        prove_mle_eval_mod_q_ligerito_tap_claims(
                            &mut t,
                            &hint,
                            &layout,
                            FQ_BITS,
                            &tclaims[i..i + 1],
                            alpha,
                            &pc,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let (mut t_clp, mut t_vx, mut t_ind) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..reps {
                let t0 = Instant::now();
                drop(prove_clp());
                t_clp.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_vx());
                t_vx.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_ind());
                t_ind.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            let proof_clp = prove_clp();
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_collapse(
                    &mut vt, &hint.commitment, &proof_clp, &layout, &rw, &colw, &pclaims,
                    alpha, &vc,
                )
                .expect("mix6 collapse verifies");
            }
            let v_clp = t0.elapsed().as_secs_f64() * 1e3;
            let proof_vx = prove_vx();
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_claims(
                    &mut vt, &hint.commitment, &proof_vx, &layout, alpha, FQ_BITS,
                    &tvclaims, &vc,
                )
                .expect("mix6 vx verifies");
            }
            let v_vx = t0.elapsed().as_secs_f64() * 1e3;
            let size_tap = |p: &f2z::ligerito_flock::IntEvalRsLigModQTapProof| {
                let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(p);
                b.total() + lig
            };
            let (m_clp, m_vx, m_ind) = (median(t_clp), median(t_vx), median(t_ind));
            println!(
                "n={n} MIX6 (2 identities + 4 off^t(ROT^7(a0^a1)), one point): clp {m_clp:.1} \
                 ms ({} inner, {:.0} KB, verify {v_clp:.1} ms) | vx6m {m_vx:.1} ms ({:.2}x of \
                 clp, {:.0} KB, verify {v_vx:.1} ms) | ind6m {m_ind:.1} ms ({:.2}x of clp)",
                proof_clp.xors.len(),
                mle_eval_mod_q_lig_xor_proof_size_bytes(&proof_clp) as f64 / 1e3,
                m_vx / m_clp,
                size_tap(&proof_vx) as f64 / 1e3,
                m_ind / m_clp,
            );
            continue;
        }

        if std::env::var("F2Z_AB_COLLAPSE").is_ok_and(|v| v == "1") {
            assert_eq!(
                layout.x_fold_extra, 0,
                "the collapse demo's stream ops are off the δ-envelope; unset F2Z_TAPS_DELTA"
            );
            // 13 single-tap claims (the instance's deduped streams) at ONE
            // shared point: collapse vs batched tap claims vs independent.
            let (streams2, _, _) = instance_clusters();
            let all_taps: Vec<TapOp> =
                streams2.into_iter().flatten().collect::<Vec<_>>();
            let rw: Vec<u128> = (0..p_x.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(29 + (u128::from(seed) << 1))
                        % FQ_MOD
                })
                .collect();
            let colw: Vec<Fq> = (0..p_x.cols())
                .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
                .collect();
            let vals: Vec<u128> = all_taps
                .iter()
                .map(|&tap| {
                    let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), &[tap]);
                    let mut y = Fq::from(0u128);
                    for (c, row) in a_rows.iter().enumerate() {
                        let mut acc = Fq::from(0u128);
                        for (wi, &word) in row.iter().enumerate() {
                            let mut bits = word;
                            while bits != 0 {
                                let t = bits.trailing_zeros() as usize;
                                acc = acc + Fq::from(rw[(wi << 6) | t]);
                                bits &= bits.wrapping_sub(1);
                            }
                        }
                        y = y + colw[c] * acc;
                    }
                    y.0
                })
                .collect();
            let single_sets: Vec<[usize; 1]> = all_taps.iter().map(|t| [t.col]).collect();
            let pclaims: Vec<TapPointClaim<'_>> = all_taps
                .iter()
                .zip(single_sets.iter())
                .zip(vals.iter())
                .map(|((tap, set), &claimed)| TapPointClaim {
                    cols: set,
                    op: tap.uni(),
                    claimed,
                })
                .collect();
            let single_lists: Vec<[TapOp; 1]> = all_taps.iter().map(|&t| [t]).collect();
            let tclaims: Vec<TapClaim<'_>> = single_lists
                .iter()
                .map(|taps| TapClaim { taps, row_weights_q: &rw })
                .collect();
            let tvclaims: Vec<TapVerifyClaim<'_, Fq>> = single_lists
                .iter()
                .zip(vals.iter())
                .map(|(taps, &v)| TapVerifyClaim {
                    taps,
                    row_weights_q: &rw,
                    col_weights: &colw,
                    claimed: Fq::from(v),
                })
                .collect();
            let prove_clp = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_collapse(
                    &mut t, &hint, &layout, &rw, &colw, &pclaims, alpha, &pc,
                )
            };
            let prove_vx13 = || {
                let mut t = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_tap_claims(
                    &mut t, &hint, &layout, FQ_BITS, &tclaims, alpha, &pc,
                )
            };
            let prove_ind13 = || {
                (0..tclaims.len())
                    .map(|i| {
                        let mut t = Blake3Transcript::new();
                        prove_mle_eval_mod_q_ligerito_tap_claims(
                            &mut t,
                            &hint,
                            &layout,
                            FQ_BITS,
                            &tclaims[i..i + 1],
                            alpha,
                            &pc,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let (mut t_clp, mut t_vx, mut t_ind) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..reps {
                let t0 = Instant::now();
                drop(prove_clp());
                t_clp.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_vx13());
                t_vx.push(t0.elapsed().as_secs_f64() * 1e3);
                let t0 = Instant::now();
                drop(prove_ind13());
                t_ind.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            let proof_clp = prove_clp();
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_collapse(
                    &mut vt, &hint.commitment, &proof_clp, &layout, &rw, &colw, &pclaims,
                    alpha, &vc,
                )
                .expect("collapse verifies");
            }
            let v_clp = t0.elapsed().as_secs_f64() * 1e3;
            let proof_vx = prove_vx13();
            let t0 = Instant::now();
            {
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_tap_claims(
                    &mut vt, &hint.commitment, &proof_vx, &layout, alpha, FQ_BITS, &tvclaims,
                    &vc,
                )
                .expect("vx13 verifies");
            }
            let v_vx = t0.elapsed().as_secs_f64() * 1e3;
            let sz_clp = mle_eval_mod_q_lig_xor_proof_size_bytes(&proof_clp);
            let sz_vx = {
                let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(&proof_vx);
                b.total() + lig
            };
            let (m_clp, m_vx, m_ind) = (median(t_clp), median(t_vx), median(t_ind));
            println!(
                "n={n} COLLAPSE (13 single-tap claims, one point): clp {m_clp:.1} ms ({} inner) \
                 | vx13 {m_vx:.1} ms ({:.2}x of clp) | ind13 {m_ind:.1} ms ({:.2}x) | proofs \
                 clp {:.0} KB, vx13 {:.0} KB | verify clp {v_clp:.1} ms, vx13 {v_vx:.1} ms",
                proof_clp.xors.len(),
                m_vx / m_clp,
                m_ind / m_clp,
                sz_clp as f64 / 1e3,
                sz_vx as f64 / 1e3,
            );
            continue;
        }

        let claim_taps = instance_claims();
        let rws: Vec<Vec<u128>> = (0..claim_taps.len())
            .map(|i| {
                (0..p_x.rows())
                    .map(|b| {
                        (b as u128)
                            .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                            .wrapping_add(11 + i as u128 + (u128::from(seed) << 1))
                            % FQ_MOD
                    })
                    .collect()
            })
            .collect();
        let colw: Vec<Fq> = (0..p_x.cols())
            .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
            .collect();
        let cs: Vec<u128> = claim_taps
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| {
                let a_rows = extract_virtual_tap_rows(&layout, hint.rows(), taps);
                let mut y = Fq::from(0u128);
                for (c, row) in a_rows.iter().enumerate() {
                    let mut acc = Fq::from(0u128);
                    for (wi, &word) in row.iter().enumerate() {
                        let mut bits = word;
                        while bits != 0 {
                            let t = bits.trailing_zeros() as usize;
                            acc = acc + Fq::from(rw[(wi << 6) | t]);
                            bits &= bits.wrapping_sub(1);
                        }
                    }
                    y = y + colw[c] * acc;
                }
                y.0
            })
            .collect();

        // Family clusters.
        let (streams, forms, members) = instance_clusters();
        let cluster_claims: Vec<Vec<RlcFamilyClaim<'_>>> = (0..2)
            .map(|ci| {
                forms[ci]
                    .iter()
                    .zip(members[ci].iter())
                    .map(|(&form, &bi)| RlcFamilyClaim {
                        form,
                        row_weights_q: &rws[bi],
                        claimed: cs[bi],
                    })
                    .collect()
            })
            .collect();
        let clusters: Vec<TapFamilyCluster<'_>> = (0..2)
            .map(|ci| TapFamilyCluster { streams: &streams[ci], claims: &cluster_claims[ci] })
            .collect();

        let tap_claims_of = |idx: &[usize]| -> Vec<TapClaim<'_>> {
            idx.iter()
                .map(|&i| TapClaim { taps: &claim_taps[i], row_weights_q: &rws[i] })
                .collect()
        };
        let tap_vclaims_of = |idx: &[usize]| -> Vec<TapVerifyClaim<'_, Fq>> {
            idx.iter()
                .map(|&i| TapVerifyClaim {
                    taps: &claim_taps[i],
                    row_weights_q: &rws[i],
                    col_weights: &colw,
                    claimed: Fq::from(cs[i]),
                })
                .collect()
        };
        let all6: Vec<usize> = (0..6).collect();

        let prove_single = || {
            let mut t = Blake3Transcript::new();
            prove_mle_eval_mod_q_ligerito_tap_claims(
                &mut t, &hint, &layout, FQ_BITS, &tap_claims_of(&[0]), alpha, &pc,
            )
        };
        let prove_tapf = || {
            let mut t = Blake3Transcript::new();
            prove_mle_eval_mod_q_ligerito_tap_family(&mut t, &hint, &layout, &clusters, alpha, &pc)
        };
        let prove_vx6 = || {
            let mut t = Blake3Transcript::new();
            prove_mle_eval_mod_q_ligerito_tap_claims(
                &mut t, &hint, &layout, FQ_BITS, &tap_claims_of(&all6), alpha, &pc,
            )
        };
        let prove_ind6 = || {
            (0..6)
                .map(|i| {
                    let mut t = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_tap_claims(
                        &mut t, &hint, &layout, FQ_BITS, &tap_claims_of(&[i]), alpha, &pc,
                    )
                })
                .collect::<Vec<_>>()
        };

        // Alternated in-window reps.
        let no_family = std::env::var("F2Z_AB_NO_FAMILY").is_ok_and(|v| v == "1")
            || layout.x_fold_extra > 0;
        let (mut t_single, mut t_tapf, mut t_vx6, mut t_ind6) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for _ in 0..reps {
            let t0 = Instant::now();
            let pr = prove_single();
            t_single.push(t0.elapsed().as_secs_f64() * 1e3);
            drop(pr);
            if !no_family {
                let t0 = Instant::now();
                let pr = prove_tapf();
                t_tapf.push(t0.elapsed().as_secs_f64() * 1e3);
                drop(pr);
            }
            let t0 = Instant::now();
            let pr = prove_vx6();
            t_vx6.push(t0.elapsed().as_secs_f64() * 1e3);
            drop(pr);
            let t0 = Instant::now();
            let pr = prove_ind6();
            t_ind6.push(t0.elapsed().as_secs_f64() * 1e3);
            drop(pr);
        }

        // Verify once each + sizes.
        let proof_single = prove_single();
        let proof_tapf = (!no_family).then(&prove_tapf);
        let proof_vx6 = prove_vx6();
        let proofs_ind = prove_ind6();
        {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, &proof_single, &layout, alpha, FQ_BITS,
                &tap_vclaims_of(&[0]), &vc,
            )
            .expect("single verifies");
        }
        let v_tapf = proof_tapf.as_ref().map(|pr| {
            let t0 = Instant::now();
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_family(
                &mut vt, &hint.commitment, pr, &layout, &clusters, &colw, alpha, &vc,
            )
            .expect("tapf verifies");
            t0.elapsed().as_secs_f64() * 1e3
        });
        let t0 = Instant::now();
        {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, &proof_vx6, &layout, alpha, FQ_BITS,
                &tap_vclaims_of(&all6), &vc,
            )
            .expect("vx6 verifies");
        }
        let v_vx6 = t0.elapsed().as_secs_f64() * 1e3;
        for (i, pr) in proofs_ind.iter().enumerate() {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, pr, &layout, alpha, FQ_BITS, &tap_vclaims_of(&[i]),
                &vc,
            )
            .expect("ind verifies");
        }

        let size_tap = |p: &f2z::ligerito_flock::IntEvalRsLigModQTapProof| {
            let (b, lig) = mle_eval_mod_q_lig_tap_size_breakdown(p);
            b.total() + lig
        };
        let sz_single = size_tap(&proof_single);
        let sz_vx6 = size_tap(&proof_vx6);
        let sz_ind6: usize = proofs_ind.iter().map(&size_tap).sum();

        let (m_single, m_vx6, m_ind6) = (median(t_single), median(t_vx6), median(t_ind6));
        let tapf_txt = match &proof_tapf {
            Some(_) => {
                let m_tapf = median(t_tapf.clone());
                format!("tapf {m_tapf:.1} ({:.2}x)", m_tapf / m_single)
            }
            None => "tapf skipped".to_string(),
        };
        println!(
            "n={n} (t'={}, s={}, tw={}): single {m_single:.1} ms | {tapf_txt} | \
             vx6 {m_vx6:.1} ({:.2}x) | ind6 {m_ind6:.1} ({:.2}x)",
            p_x.t,
            p_x.s,
            layout.tw,
            m_vx6 / m_single,
            m_ind6 / m_single,
        );
        let tapf_sz_txt = match &proof_tapf {
            Some(pr) => {
                let (bf, ligf) = mle_eval_mod_q_lig_tap_family_size_breakdown(pr);
                format!(
                    "tapf {:.0} KB (verify {:.1} ms, {} rings)",
                    (bf.total() + ligf) as f64 / 1e3,
                    v_tapf.unwrap_or(f64::NAN),
                    pr.rings.len(),
                )
            }
            None => "tapf -".to_string(),
        };
        println!(
            "  proofs: single {:.0} KB | {tapf_sz_txt} | vx6 {:.0} KB | ind6 {:.0} KB \
             | verify vx6 {v_vx6:.1} ms | rings vx6 {}",
            sz_single as f64 / 1e3,
            sz_vx6 as f64 / 1e3,
            sz_ind6 as f64 / 1e3,
            proof_vx6.rings.len(),
        );

        if profile {
            if !no_family {
                let pr = prove_tapf();
                drop(pr);
                f2z::utils::prof::dump_and_reset(&format!("tapf n={n}"));
            }
            let pr = prove_vx6();
            drop(pr);
            f2z::utils::prof::dump_and_reset(&format!("vx6 n={n}"));
        }
    }
}
