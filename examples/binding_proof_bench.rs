//! Complete-proof qualification of native/block binding and chained virtual
//! openings. Latency is measured without allocator instrumentation; use a
//! separate `bench-peak-memory` build for peak live Rust heap.
use ::f2z::{piop::spartan::*, transcript::Blake3Transcript};
use std::{hint::black_box, time::Instant};
#[cfg(feature = "bench-peak-memory")]
#[path = "../benches/common/peak_memory.rs"]
mod memory;
#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static ALLOC: memory::PeakAlloc = memory::PeakAlloc;
fn report(name: &str, samples: usize, mut prove_verify: impl FnMut() -> (f64, f64)) {
    for _ in 0..2 {
        prove_verify();
    }
    let mut p = Vec::new();
    let mut v = Vec::new();
    #[cfg(feature = "bench-peak-memory")]
    let mut peak = 0;
    for _ in 0..samples {
        #[cfg(feature = "bench-peak-memory")]
        memory::reset_peak();
        let (pt, vt) = prove_verify();
        p.push(pt);
        v.push(vt);
        #[cfg(feature = "bench-peak-memory")]
        {
            peak = peak.max(memory::peak_bytes());
        }
    }
    p.sort_by(f64::total_cmp);
    v.sort_by(f64::total_cmp);
    println!(
        "{name},prove_ms={:.3},verify_ms={:.3},samples={samples}",
        p[samples / 2],
        v[samples / 2]
    );
    #[cfg(feature = "bench-peak-memory")]
    println!("{name},peak_live_heap_bytes={peak}");
}
fn main() {
    let samples = std::env::var("SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15usize);
    if std::env::args().any(|a| a == "sha") {
        let prepared = prepare_sha256_chain_batch(7).unwrap();
        let blocks = (0..128)
            .map(|i| std::array::from_fn(|j| (i as u32 * 16 + j as u32).wrapping_mul(0x9e3779b9)))
            .collect::<Vec<_>>();
        let witness = generate_sha256_chain_witnesses(&prepared, &blocks).unwrap();
        let statement = witness.statement();
        let (pc, vc) = sha256_chain_configs(&prepared).unwrap();
        let hint = commit_sha256_chain_witness_with_config(&prepared, &witness, &pc).unwrap();
        report("sha_chain_128", samples, || {
            let t = Instant::now();
            let proof = prove_sha256_chain_with_config(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &witness,
                &hint,
                &pc,
            )
            .unwrap();
            let p = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            verify_sha256_chain_with_config(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &hint.commitment,
                &proof,
                &vc,
            )
            .unwrap();
            let v = t.elapsed().as_secs_f64() * 1e3;
            black_box(proof);
            (p, v)
        });
    } else {
        let witness = U64MulWitness::from_fn(1 << 15, |i| {
            (
                (i as u64).wrapping_mul(0x9e3779b97f4a7c15) | 1,
                (i as u64).wrapping_mul(0xc2b2ae3d27d4eb4f) | 1,
            )
        })
        .unwrap();
        let prepared = PreparedU64MulRelation::new(*witness.layout()).unwrap();
        let hint = commit_u64_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();
        report("u64_mul_32768", samples, || {
            let t = Instant::now();
            let proof =
                prove_u64_mul(&mut Blake3Transcript::new(), &prepared, &witness, &hint).unwrap();
            let p = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            verify_u64_mul(
                &mut Blake3Transcript::new(),
                &prepared,
                &hint.commitment,
                &proof,
            )
            .unwrap();
            let v = t.elapsed().as_secs_f64() * 1e3;
            black_box(proof);
            (p, v)
        });
    }
}
