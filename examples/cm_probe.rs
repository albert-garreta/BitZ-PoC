//! Full phase-tree dump of one CM-AND prove + verify (the virtual F2Z
//! pipeline), for hot-path diagnosis:
//!
//! ```text
//! OBLONG_PROFILE=1 RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --features unchecked --example cm_probe -- [log2_gates]
//! ```
//!
//! Prints `utils::prof`'s scope tree (stderr) once for the prove and once
//! for the verify, plus wall-clock totals and the proof size.

use std::time::Instant;

use f2z::piop::spartan::{
    CmAndWitness, SpartanF2zField, commit_cm_and_witness, prepare_cm_and_relation,
    project_cm_and_witness, prove_cm_and_f2z, spartan_f2z_field_config, verify_cm_and_f2z,
};
use f2z::transcript::Blake3Transcript;

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn main() {
    let _ = flock_core::init_perf_thread_pool();
    let log2_gates: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(15);
    let gates = 1usize << log2_gates;
    let config = spartan_f2z_field_config();

    let witness = CmAndWitness::from_fn(gates, |i| {
        let r = splitmix(0xCAFE ^ i as u64);
        (r as u32, (r >> 32) as u32)
    })
    .unwrap();
    let layout = *witness.layout();
    let relation = prepare_cm_and_relation::<SpartanF2zField>(layout, &config).unwrap();
    let hint = commit_cm_and_witness(&layout, witness.f_bit_rows()).unwrap();

    // Warm-up (excluded), also the correctness check.
    let projected = project_cm_and_witness::<SpartanF2zField>(&witness, &config).unwrap();
    let mut pt = Blake3Transcript::new();
    let proof = prove_cm_and_f2z(&mut pt, &relation, projected, &hint).unwrap();
    let mut vt = Blake3Transcript::new();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &proof).unwrap();
    f2z::utils::prof::dump_and_reset("cm_probe");
    drop(proof);

    let projected = project_cm_and_witness::<SpartanF2zField>(&witness, &config).unwrap();
    let mut pt = Blake3Transcript::new();
    let started = Instant::now();
    let proof = prove_cm_and_f2z(&mut pt, &relation, projected, &hint).unwrap();
    eprintln!(
        "== PROVE 2^{log2_gates} gates: {:.2} ms ==",
        started.elapsed().as_secs_f64() * 1e3
    );
    f2z::utils::prof::dump_and_reset("cm_probe");

    let mut vt = Blake3Transcript::new();
    let started = Instant::now();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &proof).unwrap();
    eprintln!(
        "== VERIFY: {:.2} ms ==",
        started.elapsed().as_secs_f64() * 1e3
    );
    f2z::utils::prof::dump_and_reset("cm_probe");

    eprintln!("proof: virtual F2Z {} B", proof.f2z.to_bytes().len());
    let digest = blake3::hash(&proof.f2z.to_bytes());
    eprintln!("f2z proof digest: {}", digest.to_hex());
}
