//! Compare wide-integer consumption against preparation once for sparse reuse.
//! This uses the paired measurement loop; it does not qualify an environment.
#![allow(dead_code)]
use flock_core::field::Gf128;
use std::{hint::black_box, time::Instant};
#[path = "allocation.rs"]
mod allocation;
include!(concat!(env!("OUT_DIR"), "/measurement.rs"));
use f2z::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, MultiswapIntegerRelation,
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let samples = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(32);
    let seed = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(1501);
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();
    let mut rng = Rng(seed);
    eprintln!(
        "RUNTIME {}",
        serde_json::json!({"arch":std::env::consts::ARCH,"caller_threads":1,"seed":seed,"shared_kernel":field::gf128::KERNEL,"qualification":"unqualified"})
    );
    println!("family,size,variant,rep,iterations,ns,bytes,allocations,allocated_bytes,correctness");
    for (shape, dims, batch) in [
        ("mini", MultiswapDims::mini(), 1),
        ("wired_b1", MultiswapDims::multiswap(0), 1),
        ("wired_b4", MultiswapDims::multiswap(0), 4),
    ] {
        for (bits, q) in [(100, (1u128 << 100) - 15), (128, u128::MAX - 158)] {
            let size = format!("{shape}_q{bits}");
            if !case_requested("multiswap_products", &size) {
                continue;
            }
            let circuit = MultiswapCircuit::build_batch(dims, batch).unwrap();
            let relation = MultiswapIntegerRelation::new(&circuit).unwrap();
            let assignment = MultiswapAssignment::new(&circuit).unwrap();
            let field = field::FpCtx::from_prime_u128(q);
            let a = assignment.products_prepared(&relation, &field);
            let b = assignment.products_fused(&relation, &field);
            assert_eq!((&a.az, &a.bz, &a.cz), (&b.az, &b.bz, &b.cz));
            let bytes = 2 * assignment.layout().capacity() * 32 * 8;
            measure(
                "multiswap_products",
                &size,
                &mut [
                    Case::new("prepared", bytes, || {
                        black_box(
                            assignment.products_prepared(black_box(&relation), black_box(&field)),
                        );
                    }),
                    Case::new("prepared_repeat", bytes, || {
                        black_box(
                            assignment.products_prepared(black_box(&relation), black_box(&field)),
                        );
                    }),
                    Case::new("fused", bytes, || {
                        black_box(
                            assignment.products_fused(black_box(&relation), black_box(&field)),
                        );
                    }),
                ],
                samples,
                &mut rng,
            );
        }
    }
    eprintln!("CORRECTNESS_COMPLETE");
}
