//! RLC-family A/B (EXPERIMENTAL, docs/rlc-family-proto-prompt.md): the XOR
//! triple (k = 3, j = 2, a₃ = a₁ ⊕ a₂) proved three ways against ONE
//! commitment and statement —
//!
//! * `single` — one claim alone via the batched-vx path (the cost unit);
//! * `rlc3`   — the RLC claim family (ONE 4-case forest + monomial
//!   discharge);
//! * `vx3`    — the batched virtual-XOR path (ONE 4·2^s-tree forest);
//! * `ind3`   — three independent single-claim vx proofs (no batching).
//!
//! Alternated in-window reps, medians reported (repo measurement
//! protocol). Verifies every variant once. With `OBLONG_PROFILE=1` one
//! extra profiled prove of `rlc3` and `vx3` dumps the phase tree per shape.
//!
//! ```text
//! F2Z_AB_N="22 24 26" F2Z_AB_REPS=5 RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --example rlc_ab --features unchecked
//! ```

use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{
    RlcFamilyClaim, VirtualXorClaim, VirtualXorVerifyClaim, commit_rs_ligerito_rows,
    mle_eval_mod_q_lig_rlc_family_proof_size_bytes, mle_eval_mod_q_lig_xor_proof_size_bytes,
    prove_mle_eval_mod_q_ligerito_claims_only, prove_mle_eval_mod_q_ligerito_rlc_family,
    sha_lig_configs, verify_mle_eval_mod_q_ligerito_claims_only,
    verify_mle_eval_mod_q_ligerito_rlc_family,
};
use f2z::pcs::{
    FQ_BITS, FQ_MOD, Fq, IntEvalParams, ShaF2Layout, extract_virtual_xor_rows,
    smallest_generator, virtual_xor_params,
};
use f2z::transcript::Blake3Transcript;
use std::time::Instant;

/// n → the A/B layout: 4 UAIR columns (log_cols = 2) of 32-bit words
/// (bit_vars = 5), the remaining n − 2 variables split t' vs s as evenly
/// as possible (x tensor t' = ⌊(n−2)/2⌋).
fn ab_layout(n: usize) -> ShaF2Layout {
    let log_cols = 2usize;
    let bit_vars = 5usize;
    let t_x = (n - log_cols) / 2;
    let s = n - log_cols - t_x;
    let tw = t_x - bit_vars;
    ShaF2Layout {
        p: IntEvalParams { t: bit_vars + log_cols + tw, s, word_bits: 1 },
        num_cols: 1 << log_cols,
        log_cols,
        bit_vars,
        num_vars: tw + s,
        tw,
        x_fold_extra: 0,
    }
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
    let profile = std::env::var("OBLONG_PROFILE").is_ok_and(|v| v == "1");

    for &n in &ns {
        let layout = ab_layout(n);
        let p = &layout.p;
        let p_x = virtual_xor_params(&layout);
        let m_p = packed_vars(p);
        let (pc, vc) = sha_lig_configs(m_p).expect("lig cfg");

        // Pseudorandom committed bit rows (memory-honest packed-rows path).
        let words = p.rows().div_ceil(64);
        let rows: Vec<Vec<u64>> = (0..p.cols())
            .map(|c| {
                (0..words)
                    .map(|w| {
                        ((c as u64) << 32 | w as u64)
                            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            .rotate_left(((c + w) & 63) as u32)
                    })
                    .collect()
            })
            .collect();
        let hint = commit_rs_ligerito_rows(p, rows, &pc);

        // Statement: 3 claims on cols {0}, {1}, {0,1}, distinct row points,
        // shared column point.
        let family_cols = [0usize, 1];
        let forms = [0b01usize, 0b10, 0b11];
        let col_lists: Vec<Vec<usize>> =
            vec![vec![family_cols[0]], vec![family_cols[1]], family_cols.to_vec()];
        let rws: Vec<Vec<u128>> = (0..3)
            .map(|i| {
                (0..p_x.rows())
                    .map(|b| {
                        (b as u128)
                            .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                            .wrapping_add(11 + i as u128)
                            % FQ_MOD
                    })
                    .collect()
            })
            .collect();
        let colw: Vec<Fq> = (0..p_x.cols())
            .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
            .collect();
        let cs: Vec<u128> = (0..3)
            .map(|i| {
                let a_rows =
                    extract_virtual_xor_rows(&layout, hint.rows(), &col_lists[i], 0, None);
                let mut y = Fq::from(0u128);
                for (c, row) in a_rows.iter().enumerate() {
                    let mut acc = Fq::from(0u128);
                    for (wi, &word) in row.iter().enumerate() {
                        let mut bits = word;
                        while bits != 0 {
                            let t = bits.trailing_zeros() as usize;
                            acc = acc + Fq::from(rws[i][(wi << 6) | t]);
                            bits &= bits.wrapping_sub(1);
                        }
                    }
                    y = y + colw[c] * acc;
                }
                y.0
            })
            .collect();
        let claims: Vec<RlcFamilyClaim<'_>> = (0..3)
            .map(|i| RlcFamilyClaim { form: forms[i], row_weights_q: &rws[i], claimed: cs[i] })
            .collect();
        let vx_of = |idx: &[usize]| -> Vec<VirtualXorClaim<'_>> {
            idx.iter()
                .map(|&i| VirtualXorClaim {
                    cols: &col_lists[i],
                    constant: 0,
                    external_rows: None,
                    row_weights_q: &rws[i],
                })
                .collect()
        };

        // Optional single-claim comparisons (F2Z_AB_SINGLES=1): the same
        // lone claim through the family API (j=1, k=1), and a lone XOR
        // claim through both APIs (family j=2 k=1 form=11 — the elided
        // pure-XOR family — vs the vx extraction path).
        let singles = std::env::var("F2Z_AB_SINGLES").is_ok_and(|v| v == "1");
        let c_xor = cs[2];
        let single_family_col = [family_cols[0]];
        let rlc1_claims = vec![RlcFamilyClaim {
            form: 0b1,
            row_weights_q: &rws[0],
            claimed: cs[0],
        }];
        let rlcx1_claims = vec![RlcFamilyClaim {
            form: 0b11,
            row_weights_q: &rws[2],
            claimed: c_xor,
        }];
        if singles {
            let mut t_rlc1 = Vec::with_capacity(reps);
            let mut t_rlcx1 = Vec::with_capacity(reps);
            let mut t_vxx1 = Vec::with_capacity(reps);
            let mut t_single1 = Vec::with_capacity(reps);
            for _ in 0..reps {
                let t0 = Instant::now();
                let pr = {
                    let mut pt = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_claims_only(
                        &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[0]), alpha, &pc,
                    )
                };
                t_single1.push(t0.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(&pr);

                let t0 = Instant::now();
                let pr = {
                    let mut pt = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_rlc_family(
                        &mut pt, &hint, &layout, &single_family_col, &rlc1_claims, alpha, &pc,
                    )
                };
                t_rlc1.push(t0.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(&pr);

                let t0 = Instant::now();
                let pr = {
                    let mut pt = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_claims_only(
                        &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[2]), alpha, &pc,
                    )
                };
                t_vxx1.push(t0.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(&pr);

                let t0 = Instant::now();
                let pr = {
                    let mut pt = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_rlc_family(
                        &mut pt, &hint, &layout, &family_cols, &rlcx1_claims, alpha, &pc,
                    )
                };
                t_rlcx1.push(t0.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(&pr);
            }
            // Sanity: the two family singles verify.
            {
                let mut pt = Blake3Transcript::new();
                let pr = prove_mle_eval_mod_q_ligerito_rlc_family(
                    &mut pt, &hint, &layout, &single_family_col, &rlc1_claims, alpha, &pc,
                );
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &pr, &layout, &single_family_col, &rlc1_claims,
                    &colw, alpha, &vc,
                )
                .expect("rlc1 verifies");
                let mut pt = Blake3Transcript::new();
                let pr = prove_mle_eval_mod_q_ligerito_rlc_family(
                    &mut pt, &hint, &layout, &family_cols, &rlcx1_claims, alpha, &pc,
                );
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &pr, &layout, &family_cols, &rlcx1_claims, &colw,
                    alpha, &vc,
                )
                .expect("rlcx1 (pure-XOR single) verifies");
            }
            println!(
                "n={n} singles: vx-single {:.1} ms | rlc1 {:.1} ms | vx-xor1 {:.1} ms | rlc-xor1 {:.1} ms",
                median(t_single1),
                median(t_rlc1),
                median(t_vxx1),
                median(t_rlcx1),
            );
        }

        // Timed variants, alternated in-window.
        let mut t_single = Vec::with_capacity(reps);
        let mut t_rlc3 = Vec::with_capacity(reps);
        let mut t_vx3 = Vec::with_capacity(reps);
        let mut t_ind3 = Vec::with_capacity(reps);
        let mut sizes = (0usize, 0usize, 0usize); // rlc3, vx3, ind3
        for rep in 0..reps {
            let t0 = Instant::now();
            let pr_single = {
                let mut pt = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_claims_only(
                    &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[0]), alpha, &pc,
                )
            };
            t_single.push(t0.elapsed().as_secs_f64() * 1e3);
            std::hint::black_box(&pr_single);

            let t0 = Instant::now();
            let pr_rlc = {
                let mut pt = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_rlc_family(
                    &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
                )
            };
            t_rlc3.push(t0.elapsed().as_secs_f64() * 1e3);
            std::hint::black_box(&pr_rlc);

            let t0 = Instant::now();
            let pr_vx3 = {
                let mut pt = Blake3Transcript::new();
                prove_mle_eval_mod_q_ligerito_claims_only(
                    &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[0, 1, 2]), alpha, &pc,
                )
            };
            t_vx3.push(t0.elapsed().as_secs_f64() * 1e3);
            std::hint::black_box(&pr_vx3);

            let t0 = Instant::now();
            let pr_inds: Vec<_> = (0..3)
                .map(|i| {
                    let mut pt = Blake3Transcript::new();
                    prove_mle_eval_mod_q_ligerito_claims_only(
                        &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[i]), alpha, &pc,
                    )
                })
                .collect();
            t_ind3.push(t0.elapsed().as_secs_f64() * 1e3);
            std::hint::black_box(&pr_inds);

            if rep == 0 {
                sizes = (
                    mle_eval_mod_q_lig_rlc_family_proof_size_bytes(&pr_rlc),
                    mle_eval_mod_q_lig_xor_proof_size_bytes(&pr_vx3),
                    pr_inds.iter().map(mle_eval_mod_q_lig_xor_proof_size_bytes).sum(),
                );
                // Sanity: every variant verifies.
                let vx_vc = |idx: &[usize]| -> Vec<VirtualXorVerifyClaim<'_, Fq>> {
                    idx.iter()
                        .map(|&i| VirtualXorVerifyClaim {
                            cols: &col_lists[i],
                            constant: 0,
                            has_external: false,
                            row_weights_q: &rws[i],
                            col_weights: &colw,
                            claimed: Fq::from(cs[i]),
                        })
                        .collect()
                };
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &pr_rlc, &layout, &family_cols, &claims, &colw,
                    alpha, &vc,
                )
                .expect("rlc3 verifies");
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_claims_only(
                    &mut vt, &hint.commitment, &pr_vx3, &layout, alpha, FQ_BITS, &vx_vc(&[0, 1, 2]),
                    &vc,
                )
                .expect("vx3 verifies");
                for (i, pr) in pr_inds.iter().enumerate() {
                    let mut vt = Blake3Transcript::new();
                    verify_mle_eval_mod_q_ligerito_claims_only(
                        &mut vt, &hint.commitment, pr, &layout, alpha, FQ_BITS, &vx_vc(&[i]), &vc,
                    )
                    .unwrap_or_else(|e| panic!("ind3[{i}] verifies: {e:?}"));
                }
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_claims_only(
                    &mut vt, &hint.commitment, &pr_single, &layout, alpha, FQ_BITS, &vx_vc(&[0]),
                    &vc,
                )
                .expect("single verifies");
            }
        }
        let (mu, mr, mb, mi) =
            (median(t_single), median(t_rlc3), median(t_vx3), median(t_ind3));
        println!(
            "n={n} (t'={}, s={}, m={}) reps={reps}\n  single {mu:8.1} ms\n  rlc3   {mr:8.1} ms  ({:.2}x single)  proof {} B\n  vx3    {mb:8.1} ms  ({:.2}x single)  proof {} B\n  ind3   {mi:8.1} ms  ({:.2}x single)  proof {} B",
            p_x.t,
            p_x.s,
            m_p + 7,
            mr / mu,
            sizes.0,
            mb / mu,
            sizes.1,
            mi / mu,
            sizes.2,
        );

        if profile {
            let mut pt = Blake3Transcript::new();
            let pr = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            std::hint::black_box(&pr);
            f2z::utils::prof::dump_and_reset(&format!("rlc3 n={n}"));
            let mut pt = Blake3Transcript::new();
            let pr = prove_mle_eval_mod_q_ligerito_claims_only(
                &mut pt, &hint, &layout, FQ_BITS, &vx_of(&[0, 1, 2]), alpha, &pc,
            );
            std::hint::black_box(&pr);
            f2z::utils::prof::dump_and_reset(&format!("vx3 n={n}"));
        }
    }
}
