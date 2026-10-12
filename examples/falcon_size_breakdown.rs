//! Inspect the current shared-prime Falcon proof without changing its protocol.
//! `cargo run --release --features falcon-hybrid --example falcon_size_breakdown`
#[path = "../benches/common/falcon_degree_inputs.rs"]
mod falcon_inputs;

use bitz::piop::spartan::falcon_profiles::{
    Falcon1024_100, Falcon1024_128, FalconPublicStatement, PreparedFalconHybrid,
};
use serde_json::json;
use std::{error::Error, fmt::Write};

struct DebugHasher(blake3::Hasher);
impl Write for DebugHasher {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0.update(text.as_bytes());
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    falcon_inputs::reject_fixture_overrides()?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .build_global()?;
    let cases = falcon_inputs::generate_cases(1024, 1024, 42)?;
    let mut input_hash = blake3::Hasher::new();
    for case in &cases {
        case.hash_into(&mut input_hash);
        case.verify_upstream()?;
    }
    let keys: Vec<_> = cases
        .iter()
        .map(|case| case.public_key.as_slice())
        .collect();
    let messages: Vec<_> = cases.iter().map(|case| case.message.as_slice()).collect();
    let signatures: Vec<_> = cases.iter().map(|case| case.signature.as_slice()).collect();
    macro_rules! measure_profile {
        ($profile:ty) => {{
        let prepared = PreparedFalconHybrid::<$profile>::new(cases.len())?;
        let target = prepared.target_bits();
        let public = FalconPublicStatement::<$profile>::from_bytes(&keys, &messages, &signatures)?;
        let committed = prepared.commit(public)?;
        let source_bits = prepared
            .source_bits_per_signature()
            .map(|bits| bits * prepared.capacity());
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed)?;
        prepared.verify(&statement, &proof)?;
        let parts = proof.payload_size_breakdown();
        assert_eq!(
            parts.iter().map(|(_, bytes)| bytes).sum::<usize>(),
            proof.payload_size_bytes()
        );
        let mut proof_hash = DebugHasher(blake3::Hasher::new());
        write!(&mut proof_hash, "{proof:?}")?;
        println!(
            "{}",
            json!({
                "schema": "bitz/falcon-size-breakdown/v1",
                "protocol": bitz::piop::spartan::falcon::PROTOCOL_ID,
                "batch": cases.len(), "seed": 42, "security_target": target,
                "input_digest": input_hash.finalize().to_hex().to_string(),
                "proof_debug_digest": proof_hash.0.finalize().to_hex().to_string(),
            "source_domain_bits": source_bits,
            "algebraic_security_bits": prepared.security().algebraic_bits,
                "payload_bytes": proof.payload_size_bytes(),
                "components": parts.into_iter().map(|(name, bytes)| json!({"name": name, "bytes": bytes})).collect::<Vec<_>>(),
                "verified": true,
            })
        );
        drop(proof);
        drop(prepared);
        flock_core::scratch::clear();
        }};
    }
    measure_profile!(Falcon1024_100);
    measure_profile!(Falcon1024_128);
    Ok(())
}
