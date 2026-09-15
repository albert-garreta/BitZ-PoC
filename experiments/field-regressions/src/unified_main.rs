//! Focused driver using the same measurement loop and baseline source as the
//! full campaign. Keeping unrelated cases out shortens edit/measure cycles.
#![allow(dead_code, unexpected_cfgs)]
use flock_core::field::Gf128;
use std::time::Instant;
#[path = "allocation.rs"]
mod allocation;
include!(concat!(env!("OUT_DIR"), "/measurement.rs"));
mod baseline_delayed {
    include!(concat!(env!("OUT_DIR"), "/delayed_reference.rs"));
}
mod baseline_raw {
    include!(concat!(env!("OUT_DIR"), "/raw_ctx.rs"));
}
mod prime {
    use crate::baseline_delayed::{
        MontyLinearAccumulator128 as Linear, MontyProductAccumulator128 as Product,
    };
    use num_traits::Zero;
    include!(concat!(env!("OUT_DIR"), "/baseline_sums.rs"));
}
#[path = "campaign/unified.rs"]
mod unified;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.get(1).map(String::as_str), Some("unified"));
    let samples = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(32);
    let seed = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(1501);
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .unwrap();
    eprintln!(
        "RUNTIME {}",
        serde_json::json!({"arch":std::env::consts::ARCH,"shared_kernel":field::gf128::KERNEL,"caller_threads":1,"aes":cfg!(target_feature="aes"),"pclmulqdq":cfg!(target_feature="pclmulqdq"),"sse4.1":cfg!(target_feature="sse4.1"),"seed":seed,"focused":true,"arithmetic_baseline_commit":"6271724d75570513aa1963381fbf5989933e5b42"})
    );
    println!("family,size,variant,rep,iterations,ns,bytes,allocations,allocated_bytes,correctness");
    unified::run(samples, &mut Rng(seed));
    eprintln!("CORRECTNESS_COMPLETE");
}
