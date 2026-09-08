//! Per-scope profile of one `prove_u64_mul` at a chosen size:
//! `cargo run --release --features unchecked --example u64_mul_probe -- 21`.
use std::time::Instant;

use f2z::{
    piop::spartan::{PreparedU64MulRelation, U64MulWitness, commit_u64_mul_witness, prove_u64_mul, verify_u64_mul},
    transcript::Blake3Transcript,
    utils::prof,
};

fn main() {
    let e: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(18);
    let reps: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    prof::force_enable();
    let witness = U64MulWitness::from_fn(1 << e, |i| {
        let x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let y = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        (x, y)
    })
    .unwrap();
    let prepared = PreparedU64MulRelation::new(*witness.layout()).unwrap();
    let hint = commit_u64_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();
    // warm-up
    let _ = prof::take_totals();
    let p = prove_u64_mul(&mut Blake3Transcript::new(), &prepared, &witness, &hint).unwrap();
    verify_u64_mul(&mut Blake3Transcript::new(), &prepared, &hint.commitment, &p).unwrap();
    let _ = prof::take_totals();
    let mut totals: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut wall = vec![];
    for _ in 0..reps {
        let t = Instant::now();
        let p = prove_u64_mul(&mut Blake3Transcript::new(), &prepared, &witness, &hint).unwrap();
        wall.push(t.elapsed().as_secs_f64() * 1e3);
        for (label, secs) in prof::take_totals() {
            totals.entry(label.to_string()).or_default().push(secs * 1e3);
        }
        std::hint::black_box(p);
    }
    let med = |v: &mut Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] };
    println!("2^{e}: prove wall median {:.1} ms over {reps} reps", med(&mut wall));
    let mut rows: Vec<_> = totals.into_iter().collect();
    rows.sort_by(|a, b| b.1.iter().cloned().fold(0.0, f64::max).partial_cmp(&a.1.iter().cloned().fold(0.0, f64::max)).unwrap());
    for (label, mut v) in rows {
        println!("  {:<48} {:8.1} ms", label, med(&mut v));
    }
}
