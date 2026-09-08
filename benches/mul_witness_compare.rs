//! Cross-backend canonical witness equivalence, independent of prover timings.
#![allow(dead_code)]
#[path = "mul_e2e_compare.rs"]
mod benchmark;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    benchmark::witness_main()
}
