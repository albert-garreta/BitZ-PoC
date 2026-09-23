//! Full BitZ grand-product comparison: the existing merged forest against the
//! dyadic Logup cut, both continued through ring switch and Ligerito.
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=10 \
//!   cargo run --release --example logup_cut_compare -- 16 6 5 4,5,6,7,8
//! ```

use std::{
    process::Command,
    time::{Duration, Instant},
};

use bitz::{
    ligerito::packed_vars,
    ligerito_flock::{
        commit_rs_ligerito_rows, historical_sha_lig_configs,
        int_eval_rs_lig_proof_size_bytes, prove_rs_ligerito, verify_rs_ligerito,
    },
    pcs::{IntegerMatrixLayout, smallest_generator},
    piop::lookup::logup_cut::{
        DyadicPlan, LogupCutConfig, LogupCutEstimate, LogupCutProfile, LogupCutScratch,
        chunk_specs, logup_cut_proof_size, prove_logup_cut_profiled, verify_logup_cut,
    },
    transcript::Blake3Transcript,
};

fn summary(samples: &[f64]) -> (f64, f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let mut deviations = samples.iter().map(|value| (value - median).abs()).collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    (sorted[0], median, sorted[sorted.len() - 1], deviations[deviations.len() / 2])
}

fn print_samples(label: &str, samples: &[f64]) {
    let (best, median, worst, mad) = summary(samples);
    println!(
        "  {label:<9} best {best:9.3} ms | median {median:9.3} ms | worst {worst:9.3} ms | MAD {mad:8.3} ms"
    );
}

fn print_phase(
    label: &str,
    profiles: &[LogupCutProfile],
    select: impl Fn(&LogupCutProfile) -> Duration,
) {
    let samples = profiles
        .iter()
        .map(|profile| select(profile).as_secs_f64() * 1e3)
        .collect::<Vec<_>>();
    print_samples(label, &samples);
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let row_vars = args.first().and_then(|value| value.parse().ok()).unwrap_or(16);
    let col_vars = args.get(1).and_then(|value| value.parse().ok()).unwrap_or(6);
    let runs = args.get(2).and_then(|value| value.parse().ok()).unwrap_or(5usize);
    let widths = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("4,5,6,7,8")
        .split(',')
        .map(|value| value.parse::<usize>().expect("chunk width"))
        .collect::<Vec<_>>();
    let component_bits = args.get(4).and_then(|value| value.parse().ok()).unwrap_or(100);
    let memory_limit_mib = args.get(5).and_then(|value| value.parse::<u64>().ok()).unwrap_or(0);
    let memory_limit_bytes = memory_limit_mib.saturating_mul(1 << 20);
    assert!(runs > 0 && !widths.is_empty());

    let layout = IntegerMatrixLayout { row_vars, col_vars, word_bits: 1 };
    let row_len = layout.rows();
    let words = row_len.div_ceil(64);
    let rows = (0..layout.cols())
        .map(|column| {
            (0..words)
                .map(|word| {
                    let mut value = (column as u64 + 1)
                        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                        ^ (word as u64 + 7).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                    value ^= value >> 30;
                    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
                    value ^ (value >> 31)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let row_weights = (0..layout.rows()).map(|row| row as u128 + 1).collect::<Vec<_>>();
    let col_weights = vec![1u128; layout.cols()];
    let claimed_eval = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(word, &bits)| {
                    let mut bits = bits;
                    let mut sum = 0u128;
                    while bits != 0 {
                        let bit = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        let index = 64 * word + bit;
                        if index < row_weights.len() {
                            sum += row_weights[index];
                        }
                    }
                    sum
                })
                .sum::<u128>()
        })
        .sum::<u128>();

    let (pc, vc) = historical_sha_lig_configs(packed_vars(&layout)).expect("Ligerito config");
    let hint = commit_rs_ligerito_rows(&layout, rows, &pc);
    let alpha = smallest_generator();
    let mut revision = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| !output.stdout.is_empty());
    if dirty {
        revision.push_str("-dirty");
    }
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1usize;

    println!("BitZ full grand-product backend comparison");
    println!("revision:       {revision}");
    println!("architecture:   {}", std::env::consts::ARCH);
    println!("field kernel:   {}", field::gf128::KERNEL);
    #[cfg(target_arch = "x86_64")]
    assert!(
        !std::arch::is_x86_feature_detected!("pclmulqdq")
            || field::gf128::KERNEL == "pclmul-karatsuba-barrett",
        "portable field kernel selected on a PCLMULQDQ host; use -C target-cpu=native"
    );
    #[cfg(target_arch = "aarch64")]
    assert!(
        !std::arch::is_aarch64_feature_detected!("pmull") || field::gf128::KERNEL == "neon",
        "portable field kernel selected on a PMULL host; use -C target-cpu=native"
    );
    println!("threads:        {threads}");
    println!("shape:          row_vars={row_vars}, col_vars={col_vars}, W=1");
    println!("witness bits:   {}", layout.cells());
    println!("runs:           {runs}");
    println!("chunk widths:   {widths:?}");
    println!("memory cap:     {memory_limit_mib} MiB (0 = disabled)");
    println!("commitment:     outside timed region\n");

    let mut baseline_prove = Vec::with_capacity(runs);
    let mut baseline_verify = Vec::with_capacity(runs);
    let mut baseline_bytes = 0usize;
    let mut expected_v = None;
    for _ in 0..runs {
        let mut prover_transcript = Blake3Transcript::new();
        let start = Instant::now();
        let proof = prove_rs_ligerito(
            &mut prover_transcript,
            &hint,
            &layout,
            &row_weights,
            alpha,
            &pc,
        );
        baseline_prove.push(start.elapsed().as_secs_f64() * 1e3);
        baseline_bytes = int_eval_rs_lig_proof_size_bytes(&proof);
        expected_v = Some(proof.v.clone());

        let mut verifier_transcript = Blake3Transcript::new();
        let start = Instant::now();
        verify_rs_ligerito(
            &mut verifier_transcript,
            &hint.commitment,
            &proof,
            &layout,
            &row_weights,
            &col_weights,
            1u128,
            alpha,
            claimed_eval,
            &vc,
        )
        .expect("baseline verification");
        baseline_verify.push(start.elapsed().as_secs_f64() * 1e3);
        assert_eq!(prover_transcript.state_digest(), verifier_transcript.state_digest());
    }
    println!("baseline: merged forest + ring switch + Ligerito");
    print_samples("prove", &baseline_prove);
    print_samples("verify", &baseline_verify);
    println!(
        "  throughput: {:.2} Mbit/s",
        layout.cells() as f64 / (summary(&baseline_prove).1 * 1e3)
    );
    println!("  proof:     {baseline_bytes} bytes\n");

    let expected_v = expected_v.unwrap();
    for chunk_bits in widths {
        let config = LogupCutConfig {
            chunk_bits,
            aux_component_bits: component_bits,
            memory_limit_bytes,
        };
        let estimate = LogupCutEstimate::new(row_len, layout.cols(), chunk_bits);
        if memory_limit_bytes != 0 && estimate.estimated_scratch_bytes > memory_limit_bytes {
            println!("dyadic Logup cut: w={chunk_bits}");
            println!("  skipped: estimated scratch {} bytes exceeds memory cap\n", estimate.estimated_scratch_bytes);
            continue;
        }
        let mut scratch = LogupCutScratch::new(layout, config).expect("Logup-cut scratch");
        let chunks = chunk_specs(row_len, chunk_bits);
        let plan = DyadicPlan::new(chunks.len());
        let mut prove_samples = Vec::with_capacity(runs);
        let mut verify_samples = Vec::with_capacity(runs);
        let mut profiles = Vec::with_capacity(runs);
        let mut size = None;
        for _ in 0..runs {
            let mut prover_transcript = Blake3Transcript::new();
            let start = Instant::now();
            let (proof, profile) = prove_logup_cut_profiled(
                &mut prover_transcript,
                &hint,
                &layout,
                &row_weights,
                alpha,
                &pc,
                config,
                &mut scratch,
            )
            .expect("Logup-cut proof");
            prove_samples.push(start.elapsed().as_secs_f64() * 1e3);
            profiles.push(profile);
            assert_eq!(proof.v, expected_v);
            size = Some(logup_cut_proof_size(&proof));

            let mut verifier_transcript = Blake3Transcript::new();
            let start = Instant::now();
            verify_logup_cut(
                &mut verifier_transcript,
                &hint.commitment,
                &proof,
                &layout,
                &row_weights,
                alpha,
                &vc,
                config,
            )
            .expect("Logup-cut verification");
            verify_samples.push(start.elapsed().as_secs_f64() * 1e3);
            assert_eq!(prover_transcript.state_digest(), verifier_transcript.state_digest());
        }
        let size = size.unwrap();
        println!("dyadic Logup cut: w={chunk_bits}");
        println!(
            "  chunks:    {} ({:?})",
            chunks.len(),
            plan.blocks.iter().map(|block| block.n_chunks).collect::<Vec<_>>()
        );
        println!("  scratch:   {} bytes retained", scratch.retained_bytes());
        println!(
            "  domains:   source {} real / {} padded, table {} real / {} padded",
            estimate.source_real_rows,
            estimate.source_padded_rows,
            estimate.table_real_rows,
            estimate.table_padded_rows,
        );
        print_samples("prove", &prove_samples);
        print_samples("verify", &verify_samples);
        println!("  prover phases:");
        print_phase("setup", &profiles, |profile| profile.common_setup);
        print_phase("table", &profiles, |profile| profile.public_table);
        print_phase("patterns", &profiles, |profile| profile.pattern_and_cut_values);
        print_phase("roots", &profiles, |profile| profile.roots);
        print_phase("upper GKR", &profiles, |profile| profile.upper_gkr);
        print_phase("pushforward", &profiles, |profile| profile.pushforward);
        print_phase("aux commit", &profiles, |profile| profile.auxiliary_commit);
        print_phase("denoms", &profiles, |profile| profile.denominators);
        print_phase("frac witness", &profiles, |profile| profile.fraction_witness);
        print_phase("frac prove", &profiles, |profile| profile.fraction_prove);
        print_phase("table IP", &profiles, |profile| profile.table_inner_product);
        print_phase("aux prepare", &profiles, |profile| profile.auxiliary_open_prepare);
        print_phase("aux open", &profiles, |profile| profile.auxiliary_open);
        print_phase("adapter", &profiles, |profile| profile.structured_adapter);
        print_phase("main open", &profiles, |profile| profile.main_opening);
        print_phase("IOP subtotal", &profiles, |profile| {
            profile
                .total
                .saturating_sub(profile.auxiliary_commit)
                .saturating_sub(profile.auxiliary_open_prepare)
                .saturating_sub(profile.auxiliary_open)
        });
        println!(
            "  throughput: {:.2} Mbit/s",
            layout.cells() as f64 / (summary(&prove_samples).1 * 1e3)
        );
        println!("  proof:     {} bytes", size.total());
        println!(
            "    v {} | root merge {} | block forest {} | aux root/round0 {} | rational {} | table IP {}",
            size.public_values,
            size.root_merge,
            size.block_forest,
            size.auxiliary_commitment + size.auxiliary_round0,
            size.rational,
            size.table_inner_product,
        );
        println!(
            "    aux opening {} | adapter {} | ring switch {} | main Ligerito {}\n",
            size.auxiliary_opening,
            size.structured_adapter,
            size.main_ring_switch,
            size.main_ligerito,
        );
    }
}
