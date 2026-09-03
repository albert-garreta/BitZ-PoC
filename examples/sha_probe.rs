//! Full `utils::prof` scope tree of ONE SHA-256 batch prove (the product
//! layout the `sha256_compressions` bench measures) at a chosen exponent:
//! a step-5.3 microscope for the virtual ring-switch batching kernels. One
//! excluded warm-up prove, then `PROBE_REPS` profiled proves, each dumped
//! separately. Same inputs, thread pool, and configs as the bench.
//!
//! ```text
//! OBLONG_PROFILE=1 PROBE_EXP=14 RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release --example sha_probe --features unchecked
//! ```

use std::time::Instant;

use f2z::{
    f2map::VirtualMap,
    piop::spartan::{
        SHA256_DEFAULT_INNER_PREFIX_VARS, Sha256CompressionInput, Sha256CompressionStatement,
        commit_sha256_compression_witness_with_config, generate_sha256_compression_witnesses,
        prepare_sha256_compression_batch, prove_sha256_compressions_with_prefix_vars_and_config,
        sha256_compression_configs, verify_sha256_compressions_with_config,
    },
    transcript::Blake3Transcript,
};

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
}

fn make_inputs(compressions: usize, seed: u64) -> Vec<Sha256CompressionInput> {
    let mut rng = SplitMix64(seed);
    (0..compressions)
        .map(|_| {
            (
                std::array::from_fn(|_| rng.next_u32()),
                std::array::from_fn(|_| rng.next_u32()),
            )
        })
        .collect()
}

fn main() {
    let exponent: usize = std::env::var("PROBE_EXP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let reps: usize = std::env::var("PROBE_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let verify = std::env::var("PROBE_VERIFY").map_or(true, |v| v != "0");
    let _ = flock_core::init_perf_thread_pool();
    f2z::utils::prof::force_enable();

    let prepared = prepare_sha256_compression_batch(exponent).expect("valid SHA relation");
    let (pc, vc) = sha256_compression_configs(&prepared).expect("valid Ligerito config");
    let compressions = prepared.instances();
    let map = prepared.map();
    println!(
        "2^{exponent} = {compressions} compressions | source {} cells ({} packs) | derived {} cells | map rows {} cols {} nnz {} | threads {}",
        prepared.source_params().cells(),
        prepared.source_params().cells() >> 7,
        prepared.assignment_params().cells(),
        map.rows(),
        map.cols(),
        map.nnz(),
        rayon::current_num_threads(),
    );

    let shape_seed = 0x46325a5f53484132u64 ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for rep in 0..=reps {
        let seed = shape_seed ^ ((rep + 1) as u64).wrapping_mul(0xd6e8_feb8_6659_fd93);
        let inputs = make_inputs(compressions, seed);
        let witness = generate_sha256_compression_witnesses(&prepared, &inputs)
            .expect("SHA witness synthesis succeeds");
        let statements = inputs
            .iter()
            .zip(witness.outputs())
            .map(|(&input, &output)| Sha256CompressionStatement::new(input, output))
            .collect::<Vec<_>>();
        let _ = f2z::utils::prof::take_totals();
        let started = Instant::now();
        let hint = commit_sha256_compression_witness_with_config(&prepared, &witness, &pc)
            .expect("SHA source commitment succeeds");
        let commit_ms = started.elapsed().as_secs_f64() * 1e3;
        let mut transcript = Blake3Transcript::new();
        let started = Instant::now();
        let proof = prove_sha256_compressions_with_prefix_vars_and_config(
            &mut transcript,
            &prepared,
            &statements,
            &witness,
            &hint,
            SHA256_DEFAULT_INNER_PREFIX_VARS,
            &pc,
        )
        .expect("SHA proof succeeds");
        let prove_ms = started.elapsed().as_secs_f64() * 1e3;
        let label = if rep == 0 { "warmup".to_owned() } else { format!("rep {rep}") };
        let f2z_bytes = proof.f2z().to_bytes();
        println!(
            "{label}: commit {commit_ms:.2} ms | prove {prove_ms:.2} ms | forests {} | proof bytes {} | f2z digest {}",
            proof.f2z().mfs.len(),
            f2z_bytes.len(),
            blake3::hash(&f2z_bytes).to_hex(),
        );
        f2z::utils::prof::dump_and_reset(&format!("sha 2^{exponent} {label}"));
        if verify && rep == reps {
            let mut vt = Blake3Transcript::new();
            let started = Instant::now();
            verify_sha256_compressions_with_config(&mut vt, &prepared, &statements, &hint.commitment, &proof, &vc)
                .expect("SHA proof verifies");
            println!("verify {:.2} ms", started.elapsed().as_secs_f64() * 1e3);
            let _ = f2z::utils::prof::take_totals();
        }
    }
}
