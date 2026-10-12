//! Layout estimates and HashToPoint measurements, not an optimized Falcon prover.
#[path = "../benches/common/falcon_degree_inputs.rs"]
mod falcon_inputs;

use bitz::piop::spartan::{
    falcon_parameters::FalconParameters,
    falcon_profiles::{Falcon1024_128, FalconPublicStatement, PreparedFalconHybrid},
};

const N: usize = 1024;
const HASH_TO_POINT_SAMPLES: usize = FalconParameters::for_degree(N).samples();
use clap::Parser;
use flock_core::challenger::FsChallenger;
use flock_prover::r1cs_hashes::{keccak, keccak3};
use serde_json::{Value, json};
use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use std::{collections::BTreeMap, error::Error};

#[derive(Parser)]
struct Args {
    // Below four, minimum-size Keccak3 slabs do not fit this four-root geometry.
    #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u16).range(4..=1024))]
    batch: u16,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u16).range(1..))]
    threads: u16,
}

// Mirrors the shared opening's geometry, without changing the protocol.
fn geometry(source_bits: Vec<usize>) -> Value {
    let logs: Vec<_> = source_bits
        .iter()
        .map(|bits| bits.ilog2() as usize - 7)
        .collect();
    assert!(source_bits.iter().all(|bits| bits.is_power_of_two()));
    let position_log = logs.iter().max().unwrap() - 3;
    let physical: Vec<usize> = logs
        .iter()
        .map(|log| 1 << (log.max(&position_log) + 7))
        .collect();
    let lanes: usize = logs
        .iter()
        .map(|log| 1 << (log.max(&position_log) - position_log))
        .sum();
    assert!(lanes <= 16);
    let total: usize = physical.iter().sum();
    json!({
        "source_bits": source_bits,
        "physical_source_bits": physical,
        "committed_bits": total,
        "committed_mib": total as f64 / 8.0 / 1048576.0,
        "initial_encoded_mib_at_rate_half": total as f64 / 4.0 / 1048576.0,
        "virtual_opening_mib": (1usize << (position_log + 4 + 7)) as f64 / 8.0 / 1048576.0,
        "source_roots": 1,
        "logical_sources": logs.len(),
        "occupied_opening_lanes": lanes,
    })
}

fn layout(capacity: usize, arithmetic: usize, slabs: &[(usize, usize)]) -> Value {
    let mut sizes = vec![arithmetic];
    for &(permutations, packing) in slabs {
        let count = capacity * permutations;
        let (block_log, block_bits) = match packing {
            1 => (keccak::min_n_keccaks_log(count), keccak::K),
            3 => (keccak3::min_n_blocks_log(count), keccak3::K),
            _ => unreachable!(),
        };
        sizes.push((1 << block_log) * block_bits);
    }
    geometry(sizes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads.into())
        .build_global()?;
    let batch = usize::from(args.batch);
    let prepared = PreparedFalconHybrid::<Falcon1024_128>::new(batch)?;
    let capacity = prepared.capacity();
    let arithmetic = prepared.source_bits_per_signature()[0] * capacity;
    let parameters = FalconParameters::for_degree(N);
    let slabs: Vec<_> = (0..parameters.slab_count())
        .map(|index| (parameters.slab(index).1, parameters.slab_width(index)))
        .collect();
    let current = layout(capacity, arithmetic, &slabs);
    let actual_bits: Vec<_> = prepared
        .source_bits_per_signature()
        .map(|bits| bits * capacity)
        .into();
    assert_eq!(current["source_bits"], json!(actual_bits));

    eprintln!("Generating and natively verifying {batch} Falcon fixtures...");
    falcon_inputs::reject_fixture_overrides()?;
    let cases = falcon_inputs::generate_cases(1024, batch, args.seed)?;
    let keys: Vec<_> = cases.iter().map(|c| c.public_key.as_slice()).collect();
    let messages: Vec<_> = cases.iter().map(|c| c.message.as_slice()).collect();
    let signatures: Vec<_> = cases.iter().map(|c| c.signature.as_slice()).collect();
    let public =
        FalconPublicStatement::<Falcon1024_128>::from_bytes(&keys, &messages, &signatures)?;
    let mut input_digest = blake3::Hasher::new();
    let mut histogram = BTreeMap::<usize, usize>::new();
    let mut total = 0;
    let mut min_samples = usize::MAX;
    let mut max_samples = 0;
    for (index, case) in cases.iter().enumerate() {
        case.hash_into(&mut input_digest);
        let mut shake = Shake256::default();
        shake.update(&public.signatures[index].nonce);
        shake.update(&case.message);
        let mut bytes = vec![0u8; 2 * HASH_TO_POINT_SAMPLES];
        shake.finalize_xof().read(&mut bytes);
        let mut accepted = 0;
        let mut stopping_samples = None;
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            if u16::from_be_bytes([pair[0], pair[1]]) < 61_445 {
                accepted += 1;
                if accepted == N {
                    stopping_samples = Some(i + 1);
                    break;
                }
            }
        }
        let samples = stopping_samples.ok_or("fixture has insufficient accepted samples")?;
        let permutations = (2 * samples).div_ceil(136);
        *histogram.entry(permutations).or_default() += 1;
        total += permutations;
        min_samples = min_samples.min(samples);
        max_samples = max_samples.max(samples);
    }

    // An existing standalone Keccak3 proof verifies. This does NOT test a
    // Falcon integration, SHAKE wiring, or the hybrid's 128-bit security budget.
    eprintln!("Checking the existing three-permutation encoder with a standalone proof...");
    let inputs: Vec<keccak::State> = (0u64..49)
        .map(|i| {
            let mut bytes = [0u8; 200];
            blake3::Hasher::new()
                .update(&i.to_le_bytes())
                .finalize_xof()
                .fill(&mut bytes);
            std::array::from_fn(|bit| bytes[bit / 8] >> (bit % 8) & 1 != 0)
        })
        .collect();
    let mut setup = keccak3::KeccakSetup::new(inputs.len());
    // This vendored setup still hardcodes the legacy six-bit interleaving.
    // Select the current embedded profile's matching L0 width for this smoke check.
    setup.pcs_params.log_batch_size =
        flock_core::pcs::ligerito::embedded_log_batch_size(setup.m(), setup.pcs_params.profile);
    let (proof, commitment, claim) =
        setup.prove_fast(&inputs, &mut FsChallenger::new(b"packing-study"));
    let verified = setup
        .verify(
            &commitment,
            &proof,
            &mut FsChallenger::new(b"packing-study"),
        )
        .map_err(|e| format!("Keccak3 smoke verification: {e:?}"))?;
    assert_eq!(claim, verified);

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "layout estimates, native prefix counts, and standalone Keccak3 smoke verification; no optimized Falcon proof",
            "batch": batch,
            "seed": args.seed,
            "input_digest": input_digest.finalize().to_hex().to_string(),
            "current": current,
            "three_wide_one_slab_20": layout(capacity, arithmetic, &[(20, 3)]),
            "three_wide_12_6_single_2": layout(capacity, arithmetic, &[(12, 3), (6, 3), (2, 1)]),
            "hypothetical_prefix_17_current_style": layout(capacity, arithmetic, &[(16, 1), (1, 1)]),
            "hypothetical_prefix_17_three_wide": layout(capacity, arithmetic, &[(12, 3), (3, 3), (2, 1)]),
            "hash_to_point": {
                "native_signatures_verified": true,
                "prefix_permutations_histogram": histogram,
                "required_prefix_permutations_total": total,
                "current_permutations_total": batch * 20,
                "samples_to_1024_accepted_min": min_samples,
                "samples_to_1024_accepted_max": max_samples,
                "warning": "prefix lengths are fixture measurements; a shorter proving path needs checked sufficiency and a longer fallback",
            },
            "keccak3_standalone_smoke": {
                "permutations": inputs.len(),
                "proof_verified": true,
                "profile": "existing Flock default; not the Falcon hybrid security profile",
                "initial_log_batch_size": setup.pcs_params.log_batch_size,
            },
        }))?
    );
    Ok(())
}
