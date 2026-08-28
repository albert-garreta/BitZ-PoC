//! End-to-end benchmark for independent SHA-256 compressions through the
//! repeated Spartan relation and virtual F2Z opening.
//!
//! The headline prover boundary matches Flock's `prove_fast`: witness
//! synthesis, bit packing, commitment, Spartan, and the PCS opening are all
//! timed. Public relation construction and deterministic input generation are
//! excluded. Every measured proof is verified.
//!
//! Defaults to `2^10, 2^11, 2^12, 2^13, 2^14, 2^16` compressions with three
//! measured repetitions after one warm-up. Override with, for example:
//!
//! ```text
//! OBLONG_PROFILE=1 F2Z_SHA_LOG2S="10 12" F2Z_SHA_REPS=1 \
//!   cargo bench --bench sha256_compressions --features unchecked
//! ```

use std::{hint::black_box, time::Instant};

use f2z::{
    f2map::VirtualMap,
    piop::spartan::{
        commit_sha256_compression_witness_with_config, generate_sha256_compression_witnesses,
        prepare_sha256_compression_batch, prove_sha256_compressions_spartan_and_f2z_with_config,
        sha256_compression_configs, verify_sha256_compressions_spartan_and_f2z_with_config,
        Sha256CompressionInput, SpartanField, SHA256_CONSTRAINT_STRIDE, SHA256_F_BAR_LIVE_BITS,
        SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS, SHA256_H_STRIDE,
    },
    transcript::Blake3Transcript,
};

#[derive(Clone, Copy, Debug)]
struct Timings {
    witness_ms: f64,
    commit_ms: f64,
    proof_ms: f64,
    end_to_end_ms: f64,
    verify_ms: f64,
    spartan_prove_ms: Option<f64>,
    opening_prepare_prove_ms: Option<f64>,
    f2z_prove_ms: Option<f64>,
    spartan_verify_ms: Option<f64>,
    opening_prepare_verify_ms: Option<f64>,
    f2z_verify_ms: Option<f64>,
    spartan_elements: usize,
    spartan_bytes: usize,
    f2z_bytes: usize,
}

#[derive(Clone, Copy)]
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

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn exponents() -> Vec<usize> {
    let value = std::env::var("F2Z_SHA_LOG2S").unwrap_or_else(|_| "10 11 12 13 14 16".to_owned());
    let exponents = value
        .split([',', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let exponent = part
                .parse::<usize>()
                .expect("F2Z_SHA_LOG2S contains integer exponents");
            assert!(
                exponent >= 7,
                "SHA virtual F2Z needs at least 2^7 compressions for bit packing"
            );
            exponent
        })
        .collect::<Vec<_>>();
    assert!(!exponents.is_empty(), "F2Z_SHA_LOG2S must not be empty");
    exponents
}

fn phase_ms(phases: &[(&'static str, f64)], label: &str) -> Option<f64> {
    phases
        .iter()
        .find(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    sorted[sorted.len() / 2]
}

fn best(samples: &[f64]) -> f64 {
    samples.iter().copied().fold(f64::INFINITY, f64::min)
}

fn fmt_ms(milliseconds: f64) -> String {
    if milliseconds < 1.0 {
        format!("{:8.2} us", milliseconds * 1e3)
    } else if milliseconds < 1_000.0 {
        format!("{milliseconds:8.2} ms")
    } else {
        format!("{:8.2} s ", milliseconds / 1e3)
    }
}

fn run_once(
    inputs: &[Sha256CompressionInput],
    matrices: &f2z::piop::spartan::PreparedConstraintMatrices<f2z::piop::spartan::SpartanF2zField>,
    map: &f2z::f2map::RepeatedVirtualMap,
    p_f: &f2z::pcs::IntEvalParams,
    p_h: &f2z::pcs::IntEvalParams,
    pc: &flock_core::pcs::ligerito::ProverConfig,
    vc: &flock_core::pcs::ligerito::VerifierConfig,
) -> Timings {
    let total_started = Instant::now();

    let started = Instant::now();
    let (f_rows, h_rows, products, outputs) =
        generate_sha256_compression_witnesses(inputs, p_f, p_h, matrices.config())
            .expect("SHA witness synthesis succeeds");
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;
    black_box(&outputs);

    let started = Instant::now();
    let hint = commit_sha256_compression_witness_with_config(p_f, f_rows, pc)
        .expect("SHA source commitment succeeds");
    let commit_ms = started.elapsed().as_secs_f64() * 1e3;

    let _ = f2z::utils::prof::take_totals();
    let mut prover_transcript = Blake3Transcript::new();
    let started = Instant::now();
    let proof = prove_sha256_compressions_spartan_and_f2z_with_config(
        &mut prover_transcript,
        matrices,
        map,
        p_h,
        p_f,
        &h_rows,
        products,
        &hint,
        pc,
    )
    .expect("SHA proof succeeds");
    let proof_ms = started.elapsed().as_secs_f64() * 1e3;
    let end_to_end_ms = total_started.elapsed().as_secs_f64() * 1e3;
    let prove_phases = f2z::utils::prof::take_totals();

    let f2z_bytes = proof.f2z().to_bytes().len();
    let spartan_elements = 4 * proof.spartan().sumcheck.round_polynomials.len() + 3;
    let field_bytes = proof
        .spartan()
        .az_mle_claim
        .canonical_element_encoding()
        .len();
    let spartan_bytes = spartan_elements * field_bytes;

    let mut verifier_transcript = Blake3Transcript::new();
    let started = Instant::now();
    verify_sha256_compressions_spartan_and_f2z_with_config(
        &mut verifier_transcript,
        matrices,
        map,
        p_h,
        p_f,
        &hint.commitment,
        &proof,
        vc,
    )
    .expect("SHA proof verifies");
    let verify_ms = started.elapsed().as_secs_f64() * 1e3;
    let verify_phases = f2z::utils::prof::take_totals();
    black_box(&proof);

    Timings {
        witness_ms,
        commit_ms,
        proof_ms,
        end_to_end_ms,
        verify_ms,
        spartan_prove_ms: phase_ms(&prove_phases, "sha256-f2z:spartan_outer_prove"),
        opening_prepare_prove_ms: phase_ms(&prove_phases, "sha256-f2z:opening_prepare_prover"),
        f2z_prove_ms: phase_ms(&prove_phases, "sha256-f2z:f2z_prove"),
        spartan_verify_ms: phase_ms(&verify_phases, "sha256-f2z:spartan_outer_verify"),
        opening_prepare_verify_ms: phase_ms(&verify_phases, "sha256-f2z:opening_prepare_verifier"),
        f2z_verify_ms: phase_ms(&verify_phases, "sha256-f2z:f2z_verify"),
        spartan_elements,
        spartan_bytes,
        f2z_bytes,
    }
}

fn optional_median(samples: &[Timings], field: impl Fn(&Timings) -> Option<f64>) -> Option<f64> {
    samples
        .iter()
        .map(field)
        .collect::<Option<Vec<_>>>()
        .map(|values| median(&values))
}

fn bench_exponent(exponent: usize, reps: usize, root_seed: u64) {
    let compressions = 1usize
        .checked_shl(u32::try_from(exponent).expect("exponent fits u32"))
        .expect("compression count fits usize");
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);

    let setup_started = Instant::now();
    let (matrices, map, p_f, p_h) =
        prepare_sha256_compression_batch(exponent).expect("valid SHA relation");
    let (pc, vc) = sha256_compression_configs(&p_f).expect("valid Ligerito config");
    let setup_ms = setup_started.elapsed().as_secs_f64() * 1e3;

    let input_sets = (0..=reps)
        .map(|sample| {
            make_inputs(
                compressions,
                shape_seed ^ (sample as u64).wrapping_mul(0xd6e8_feb8_6659_fd93),
            )
        })
        .collect::<Vec<_>>();

    println!();
    println!("=== 2^{exponent} = {compressions} independent SHA-256 compressions ===");
    println!(
        "  source: live={} padded={} bits/compression | derived: live={} padded={} bits/compression",
        SHA256_F_BAR_LIVE_BITS, SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS, SHA256_H_STRIDE,
    );
    println!(
        "  R1CS: {} live / {} padded rows per compression | repeated map nnz={} | setup {}",
        f2z::piop::spartan::SHA256_CONSTRAINTS,
        SHA256_CONSTRAINT_STRIDE,
        map.nnz(),
        fmt_ms(setup_ms),
    );

    let warm = run_once(&input_sets[0], &matrices, &map, &p_f, &p_h, &pc, &vc);
    black_box(warm);

    let mut samples = Vec::with_capacity(reps);
    for sample in 0..reps {
        let timing = run_once(
            &input_sets[sample + 1],
            &matrices,
            &map,
            &p_f,
            &p_h,
            &pc,
            &vc,
        );
        println!(
            "  SAMPLE exponent={exponent} sample={} compressions={compressions} end_to_end_ms={:.6} witness_ms={:.6} commit_ms={:.6} proof_ms={:.6} verify_ms={:.6} verified=true",
            sample + 1,
            timing.end_to_end_ms,
            timing.witness_ms,
            timing.commit_ms,
            timing.proof_ms,
            timing.verify_ms,
        );
        samples.push(timing);
    }

    let e2e = samples
        .iter()
        .map(|sample| sample.end_to_end_ms)
        .collect::<Vec<_>>();
    let witness = samples
        .iter()
        .map(|sample| sample.witness_ms)
        .collect::<Vec<_>>();
    let commit = samples
        .iter()
        .map(|sample| sample.commit_ms)
        .collect::<Vec<_>>();
    let proof = samples
        .iter()
        .map(|sample| sample.proof_ms)
        .collect::<Vec<_>>();
    let verify = samples
        .iter()
        .map(|sample| sample.verify_ms)
        .collect::<Vec<_>>();
    let e2e_median = median(&e2e);
    let e2e_best = best(&e2e);
    let proof_median = median(&proof);
    let throughput = compressions as f64 / (e2e_median / 1e3);
    let proof_throughput = compressions as f64 / (proof_median / 1e3);
    let last = samples.last().expect("positive repetition count");

    println!(
        "  end-to-end prove: {} median | {} best | {:10.0} compressions/s",
        fmt_ms(e2e_median),
        fmt_ms(e2e_best),
        throughput,
    );
    println!(
        "  median split: witness {} | commit {} | Spartan+virtual-F2Z {} ({:10.0} compressions/s)",
        fmt_ms(median(&witness)),
        fmt_ms(median(&commit)),
        fmt_ms(proof_median),
        proof_throughput,
    );
    println!("  verify: {}", fmt_ms(median(&verify)));

    let spartan_prove = optional_median(&samples, |sample| sample.spartan_prove_ms);
    let opening_prove = optional_median(&samples, |sample| sample.opening_prepare_prove_ms);
    let f2z_prove = optional_median(&samples, |sample| sample.f2z_prove_ms);
    let spartan_verify = optional_median(&samples, |sample| sample.spartan_verify_ms);
    let opening_verify = optional_median(&samples, |sample| sample.opening_prepare_verify_ms);
    let f2z_verify = optional_median(&samples, |sample| sample.f2z_verify_ms);
    if let (Some(spartan), Some(opening), Some(f2z)) = (spartan_prove, opening_prove, f2z_prove) {
        println!(
            "  proof internals: outer Spartan {} | claim factorization {} | virtual F2Z {}",
            fmt_ms(spartan),
            fmt_ms(opening),
            fmt_ms(f2z),
        );
    }
    if let (Some(spartan), Some(opening), Some(f2z)) = (spartan_verify, opening_verify, f2z_verify)
    {
        println!(
            "  verify internals: outer Spartan {} | claim factorization {} | virtual F2Z {}",
            fmt_ms(spartan),
            fmt_ms(opening),
            fmt_ms(f2z),
        );
    }
    println!(
        "  proof: virtual F2Z {} B | Spartan payload {} elements / {} B",
        last.f2z_bytes, last.spartan_elements, last.spartan_bytes,
    );

    let profile = |value: Option<f64>| value.unwrap_or(f64::NAN);
    println!(
        "  RESULT exponent={exponent} compressions={compressions} repetitions={reps} warmups=1 end_to_end_median_ms={e2e_median:.6} end_to_end_best_ms={e2e_best:.6} throughput_compressions_per_s={throughput:.3} witness_median_ms={:.6} commit_median_ms={:.6} proof_median_ms={proof_median:.6} proof_throughput_compressions_per_s={proof_throughput:.3} spartan_prove_median_ms={:.6} opening_prepare_prove_median_ms={:.6} f2z_prove_median_ms={:.6} verify_median_ms={:.6} spartan_verify_median_ms={:.6} opening_prepare_verify_median_ms={:.6} f2z_verify_median_ms={:.6} spartan_payload_elements={} spartan_payload_bytes={} f2z_proof_bytes={} verified_samples={reps} shape_seed={shape_seed:#018x}",
        median(&witness),
        median(&commit),
        profile(spartan_prove),
        profile(opening_prove),
        profile(f2z_prove),
        median(&verify),
        profile(spartan_verify),
        profile(opening_verify),
        profile(f2z_verify),
        last.spartan_elements,
        last.spartan_bytes,
        last.f2z_bytes,
    );
}

fn main() {
    let _ = flock_core::init_perf_thread_pool();
    let reps = env_usize("F2Z_SHA_REPS", 3);
    assert!(reps > 0, "F2Z_SHA_REPS must be positive");
    let root_seed = std::env::var("F2Z_SHA_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0x4632_5a5f_5348_4132);

    println!("SHA-256: synthesized [1|f], h=Mf, Ah/Bh/Ch; repeated outer Spartan + virtual F2Z");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {}", rayon::current_num_threads());
    println!("repetitions: {reps}; warmups: 1; root seed: {root_seed:#018x}");
    if std::env::var_os("OBLONG_PROFILE").is_none() {
        println!("phase profiling: disabled (set OBLONG_PROFILE=1 for internal splits)");
    }

    for exponent in exponents() {
        flock_core::scratch::clear();
        bench_exponent(exponent, reps, root_seed);
    }
    flock_core::scratch::clear();
}
