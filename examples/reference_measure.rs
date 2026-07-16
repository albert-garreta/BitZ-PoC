//! Reference measurement for the F2Z ring-switch + Ligerito opener: prove /
//! verify wall-clock and serialized proof size at a few `n = t + s` shapes.
//!
//! Run with:
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo run --release --example reference_measure
//! ```

use std::time::Instant;

use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{
    LigConfig, commit_rs_flock_with, lig_configs, prove_mle_eval_mod_q_ligerito,
    verify_mle_eval_mod_q_ligerito,
};
use f2z::pcs::{IntEvalParams, mod_q_num_chunks, smallest_generator};

/// `𝔽_q`, `q = 2^100 − 15`.
const Q: u128 = (1u128 << 100) - 15;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Fq(u128);
impl From<u128> for Fq {
    fn from(v: u128) -> Self {
        Fq(v % Q)
    }
}
impl core::ops::Add for Fq {
    type Output = Fq;
    fn add(self, o: Fq) -> Fq {
        let s = self.0 + o.0;
        Fq(if s >= Q { s - Q } else { s })
    }
}
impl core::ops::Mul for Fq {
    type Output = Fq;
    fn mul(self, o: Fq) -> Fq {
        let (mut a, mut b, mut acc) = (self.0, o.0, 0u128);
        while b != 0 {
            if b & 1 == 1 {
                let s = acc + a;
                acc = if s >= Q { s - Q } else { s };
            }
            let d = a << 1;
            a = if d >= Q { d - Q } else { d };
            b >>= 1;
        }
        Fq(acc)
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn measure(t: usize, s: usize, w: usize, reps: usize) {
    let alpha = smallest_generator();
    let q_bits = 100usize;
    let p = IntEvalParams { t, s, word_bits: w };
    let m_p = packed_vars(&p);
    let lch = mod_q_num_chunks(&p, q_bits);
    let (pc, vc) =
        lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }).expect("lig cfg");

    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let data: Vec<u128> =
        (0..p.cells()).map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask).collect();
    let rw_q: Vec<u128> = (0..p.rows())
        .map(|b| {
            (b as u128)
                .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                .wrapping_add(7)
                % Q
        })
        .collect();
    let cw: Vec<Fq> = (0..p.cols())
        .map(|c| Fq::from(((c as u128).wrapping_mul(5) & 7).wrapping_add(1)))
        .collect();
    let mut y = Fq::from(0u128);
    for c in 0..p.cols() {
        let mut vc_acc = Fq::from(0u128);
        for b in 0..p.rows() {
            vc_acc = vc_acc + Fq::from(rw_q[b]) * Fq::from(data[p.cell_index(b, c)]);
        }
        y = y + cw[c] * vc_acc;
    }

    let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);

    let mut prove_ms = Vec::new();
    let mut verify_ms = Vec::new();
    let mut bytes = 0usize;
    for _ in 0..reps {
        let mut pt = f2z::transcript::Blake3Transcript::new();
        let t0 = Instant::now();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
        prove_ms.push(t0.elapsed().as_secs_f64() * 1e3);

        let ser = proof.to_bytes();
        bytes = ser.len();

        let mut vt = f2z::transcript::Blake3Transcript::new();
        let t1 = Instant::now();
        verify_mle_eval_mod_q_ligerito(
            &mut vt,
            &hint.commitment,
            &proof,
            &p,
            &rw_q,
            &cw,
            alpha,
            y,
            q_bits,
            &vc,
        )
        .expect("verify");
        verify_ms.push(t1.elapsed().as_secs_f64() * 1e3);
    }

    let n = t + s;
    println!(
        "n={n:2} (t={t}, s={s}, W={w}, m_p={m_p}, chunks={lch}): \
         prove {:7.2} ms | verify {:6.2} ms | proof {:6} B ({:.1} KiB)",
        median(prove_ms),
        median(verify_ms),
        bytes,
        bytes as f64 / 1024.0,
    );
}

fn main() {
    println!("F2Z ring-switch + Ligerito opener — reference measurement (median of 5)\n");
    measure(10, 6, 1, 5); // n=16 headline
    measure(12, 6, 1, 5); // n=18
    measure(4, 8, 32, 5); // 2-chunk (W=32) regime
}
