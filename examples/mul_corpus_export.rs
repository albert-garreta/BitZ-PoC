//! Exports the operand corpus of one multiplication case exactly as the
//! benchmark harness generates it, with the assignment digest every native
//! backend must reproduce, so an external worker (the Zinc+ bench) proves
//! the same operands and the table can tie its rows to the same digest.
//!
//!     cargo run --release --example mul_corpus_export -- --workload u64 --log-n 15 --out corpora/
//!
//! `u32-mod32` uses the versioned blake3-XOF corpus (`native-mul/mod32/inputs/v1`)
//! and its row digest; `u64` and `u128` use the harness's seeded `StdRng` stream
//! (`shape_seed(root, log_n)` from the per-workload root seed) and the
//! `bitz/u64-mul-compare/integer-witness/v1` / `bitz/u128-mul-compare/integer-witness/v1`
//! assignment digests.
#[path = "../benches/common/mul_witness.rs"]
mod mul_witness;
#[path = "../benches/mul/mod32.rs"]
mod mod32;

use bitz::piop::spartan::mul::MulWitness;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{fs, io::Write, path::PathBuf};

pub const SCHEMA: &str = "bitz/mul-corpus/v1";

fn arg(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == name {
            return args.next();
        }
    }
    None
}

fn main() {
    let workload = arg("--workload").expect("--workload u32-mod32|u64|u128");
    let log_n: usize = arg("--log-n").expect("--log-n N").parse().expect("integer exponent");
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| ".".into()));
    let root_seed = arg("--seed").map(|s| s.parse().expect("integer seed"));
    fs::create_dir_all(&out).expect("output directory");
    let (root_seed, effective_seed, generator, domain, operand_bytes, digest, bytes) = match workload.as_str() {
        "u32-mod32" => {
            let seed = root_seed.unwrap_or(mul_witness::U32_SEED);
            let inputs = mod32::inputs(log_n, seed);
            let digest = mod32::digest_rows(
                inputs.iter().map(|&(a, b)| [a, b, u64::from((a as u32).wrapping_mul(b as u32))]),
                inputs.len(),
            );
            let mut bytes = Vec::with_capacity(inputs.len() * 8);
            for &(a, b) in &inputs {
                bytes.extend_from_slice(&(a as u32).to_le_bytes());
                bytes.extend_from_slice(&(b as u32).to_le_bytes());
            }
            (seed, seed, "blake3-xof(domain, seed, exponent)", "native-mul/mod32/rows/v1", 4, digest, bytes)
        }
        "u64" => {
            let seed = root_seed.unwrap_or(mul_witness::U64_SEED);
            let effective = mul_witness::shape_seed(seed, log_n);
            let mut rng = StdRng::seed_from_u64(effective);
            let inputs: Vec<(u64, u64)> = (0..1usize << log_n).map(|_| (rng.random::<u64>(), rng.random())).collect();
            let digest = mul_witness::u64_digest(&MulWitness::<u64>::from_inputs(&inputs).expect("canonical u64 witness"));
            let mut bytes = Vec::with_capacity(inputs.len() * 16);
            for &(a, b) in &inputs {
                bytes.extend_from_slice(&a.to_le_bytes());
                bytes.extend_from_slice(&b.to_le_bytes());
            }
            (seed, effective, "rand-0.10 StdRng::seed_from_u64(shape_seed).random::<u64>() pairs", "bitz/u64-mul-compare/integer-witness/v1", 8, digest, bytes)
        }
        "u128" => {
            let seed = root_seed.unwrap_or(mul_witness::U128_SEED);
            let effective = mul_witness::shape_seed(seed, log_n);
            let mut rng = StdRng::seed_from_u64(effective);
            let inputs: Vec<(u128, u128)> = (0..1usize << log_n).map(|_| (rng.random::<u128>(), rng.random())).collect();
            let digest = mul_witness::u128_digest(&MulWitness::<u128>::from_inputs(&inputs).expect("canonical u128 witness"));
            let mut bytes = Vec::with_capacity(inputs.len() * 32);
            for &(a, b) in &inputs {
                bytes.extend_from_slice(&a.to_le_bytes());
                bytes.extend_from_slice(&b.to_le_bytes());
            }
            (seed, effective, "rand-0.10 StdRng::seed_from_u64(shape_seed).random::<u128>() pairs", "bitz/u128-mul-compare/integer-witness/v1", 16, digest, bytes)
        }
        other => panic!("unknown workload {other}; expected u32-mod32, u64 or u128"),
    };
    let stem = format!("corpus-{workload}-2p{log_n}-seed{root_seed}");
    let bin = out.join(format!("{stem}.bin"));
    fs::File::create(&bin).and_then(|mut f| f.write_all(&bytes)).expect("write corpus");
    let file_sha256 = blake3::hash(&bytes).to_hex().to_string();
    let json = serde_json::json!({
        "schema": SCHEMA, "workload": workload, "log_n": log_n, "count": 1usize << log_n,
        "seed": root_seed, "effective_seed": effective_seed, "generator": generator,
        "operand_bytes": operand_bytes, "layout": "little-endian (x, y) pairs",
        "digest_domain": domain, "corpus_digest": digest, "file_blake3": file_sha256,
        "file": bin.file_name().unwrap().to_string_lossy(),
    });
    fs::write(out.join(format!("{stem}.json")), serde_json::to_string_pretty(&json).unwrap() + "\n").expect("write manifest");
    println!("{}", serde_json::to_string(&json).unwrap());
}
