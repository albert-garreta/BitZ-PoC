//! Isolated comparison of three size-N GKR constructions.
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" F2_FOREST_SCHEDULE=l8 \
//!   cargo run --release --example gkr_components -- 14 12 5 10
//! ```

use std::{hint::black_box, io::Write, process::Command, time::Instant};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use bitz::{
    Gf128 as Gf,
    merged_forest::{
        ForestSchedule, MergedForestProfile, prove_merged_forest_lazy_from_rows_profiled,
        prove_merged_forest_profiled,
        schedule::{ForestPath, SchedulePolicy, resolve_schedule}, verify_merged_forest,
    },
    pcs::IntegerMatrixLayout,
    piop::lookup::logup_cut::{
        FractionProofProfile, FractionTreeWitness, ProductWorkspace, prove_rational,
        prove_rational_profiled, verify_rational,
    },
    transcript::Blake3Transcript,
};

#[derive(Clone, Copy)]
struct Sample {
    witness: f64,
    sumcheck: f64,
}

fn summary(samples: &[Sample], select: impl Fn(&Sample) -> f64) -> (f64, f64, f64, f64) {
    let mut values = samples.iter().map(select).collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let median = values[values.len() / 2];
    let mut deviations = values.iter().map(|value| (value - median).abs()).collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    (values[0], median, values[values.len() - 1], deviations[deviations.len() / 2])
}

fn fraction_duration_summary(
    profiles: &[FractionProofProfile],
    select: impl Fn(&FractionProofProfile) -> std::time::Duration,
) -> (f64, f64, f64, f64) {
    let mut values = profiles
        .iter()
        .map(|profile| select(profile).as_secs_f64() * 1e3)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let median = values[values.len() / 2];
    let mut deviations = values.iter().map(|value| (value - median).abs()).collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    (values[0], median, values[values.len() - 1], deviations[deviations.len() / 2])
}

fn forest_duration_summary(
    profiles: &[MergedForestProfile],
    select: impl Fn(&MergedForestProfile) -> std::time::Duration,
) -> (f64, f64, f64, f64) {
    let mut values = profiles
        .iter()
        .map(|profile| select(profile).as_secs_f64() * 1e3)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let median = values[values.len() / 2];
    let mut deviations = values.iter().map(|value| (value - median).abs()).collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    (values[0], median, values[values.len() - 1], deviations[deviations.len() / 2])
}

fn print_samples(label: &str, samples: &[Sample]) {
    let witness = summary(samples, |sample| sample.witness);
    let sumcheck = summary(samples, |sample| sample.sumcheck);
    let total = summary(samples, |sample| sample.witness + sample.sumcheck);
    println!("{label}");
    println!(
        "  witness build  best {:9.3} ms | median {:9.3} ms | worst {:9.3} ms | MAD {:8.3} ms",
        witness.0, witness.1, witness.2, witness.3,
    );
    println!(
        "  sumcheck       best {:9.3} ms | median {:9.3} ms | worst {:9.3} ms | MAD {:8.3} ms",
        sumcheck.0, sumcheck.1, sumcheck.2, sumcheck.3,
    );
    println!(
        "  total          best {:9.3} ms | median {:9.3} ms | worst {:9.3} ms | MAD {:8.3} ms",
        total.0, total.1, total.2, total.3,
    );
}

fn eager_sample(
    rows: &[Vec<u64>],
    powers: &[Vec<Gf>],
    row_vars: usize,
    col_vars: usize,
) -> (Sample, u64) {
    let row_len = 1usize << row_vars;
    let started = Instant::now();
    let leaves: Vec<Gf> = bitz::cfg_into_iter!(0..row_len * rows.len(), 1 << 14)
        .map(|index| {
            let column = index >> row_vars;
            let row = index & (row_len - 1);
            if (rows[column][row >> 6] >> (row & 63)) & 1 == 0 {
                Gf::ONE
            } else {
                powers[row][0]
            }
        })
        .collect();
    let mut dense_leaves = started.elapsed();
    let mut transcript = Blake3Transcript::new();
    let mut profile = MergedForestProfile::default();
    let output = prove_merged_forest_profiled(
        &mut transcript,
        &leaves,
        row_vars,
        col_vars,
        &mut profile,
    );
    let guard = output.3.as_words()[0];
    let started = Instant::now();
    drop(leaves);
    dense_leaves += started.elapsed();
    black_box(output);
    (
        Sample {
            witness: (dense_leaves + profile.witness_build).as_secs_f64() * 1e3,
            sumcheck: profile.sumcheck.as_secs_f64() * 1e3,
        },
        guard,
    )
}

fn lazy_sample(
    layout: &IntegerMatrixLayout,
    rows: &[Vec<u64>],
    packed: &[Vec<u64>],
    powers: &[Vec<Gf>],
) -> (Sample, MergedForestProfile, u64) {
    let mut transcript = Blake3Transcript::new();
    let mut profile = MergedForestProfile::default();
    let output = prove_merged_forest_lazy_from_rows_profiled(
        &mut transcript,
        layout,
        rows,
        packed,
        powers,
        layout.cols(),
        &mut profile,
    );
    let guard = output.3.as_words()[0];
    black_box(output);
    let sample = Sample {
        witness: profile.witness_build.as_secs_f64() * 1e3,
        sumcheck: profile.sumcheck.as_secs_f64() * 1e3,
    };
    (sample, profile, guard)
}

fn fraction_sample(
    numerators: &mut Vec<Gf>,
    denominators: &mut Vec<Gf>,
    log_n: usize,
    left: &mut FractionTreeWitness,
    right: &mut FractionTreeWitness,
    workspace: &mut ProductWorkspace,
) -> (Sample, FractionProofProfile, u64) {
    let started = Instant::now();
    left.rebuild_swapped(numerators, denominators, log_n, Gf::ZERO, Gf::ONE);
    let root = left.root();
    right.rebuild(&[root.0], &[root.1], 0, Gf::ZERO, Gf::ONE);
    let mut witness = started.elapsed();

    let mut transcript = Blake3Transcript::new();
    let mut profile = FractionProofProfile::default();
    let started = Instant::now();
    let output = prove_rational_profiled(
        &mut transcript,
        left,
        right,
        workspace,
        &mut profile,
    );
    let sumcheck = started.elapsed();
    let guard = output.1.left.num.as_words()[0] ^ output.1.left.den.as_words()[1];
    black_box(output);
    let started = Instant::now();
    left.release_leaves(numerators, denominators);
    witness += started.elapsed();
    let sample = Sample {
        witness: witness.as_secs_f64() * 1e3,
        sumcheck: sumcheck.as_secs_f64() * 1e3,
    };
    (sample, profile, guard)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let row_vars = args.first().and_then(|value| value.parse().ok()).unwrap_or(14usize);
    let col_vars = args.get(1).and_then(|value| value.parse().ok()).unwrap_or(12usize);
    let runs = args.get(2).and_then(|value| value.parse().ok()).unwrap_or(5usize);
    let requested_threads = args.get(3).and_then(|value| value.parse().ok());
    assert!(row_vars >= 6 && runs > 0);

    #[cfg(feature = "parallel")]
    if let Some(threads) = requested_threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("configure Rayon worker count");
    }
    #[cfg(not(feature = "parallel"))]
    assert!(requested_threads.is_none_or(|threads| threads == 1));

    let log_n = row_vars + col_vars;
    let n = 1usize << log_n;
    let row_len = 1usize << row_vars;
    let cols = 1usize << col_vars;
    let layout = IntegerMatrixLayout { row_vars, col_vars, word_bits: 1 };

    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1usize;
    let policy = SchedulePolicy::from_env().expect("valid F2_FOREST_SCHEDULE");
    let schedule = resolve_schedule(policy, &layout, ForestPath::Single, threads)
        .expect("supported forest schedule");
    assert_eq!(
        schedule,
        ForestSchedule::L8,
        "this comparison needs F2_FOREST_SCHEDULE=l8 for the 1/2/3-round low-entropy skips",
    );
    assert!(
        std::env::var("BITZ_LUT3").map_or(true, |value| value != "0"),
        "the low-entropy comparison needs BITZ_LUT3 enabled",
    );
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

    println!("GKR component comparison");
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
    println!("N:             2^{log_n} = {n}");
    println!("tree depth:    {row_vars} ({row_len} leaves/tree)");
    println!("trees:         2^{col_vars} = {cols}");
    println!("runs:          {runs}");
    println!("threads:       {threads}");
    println!("low entropy:   {} (1/2/3 bottom-layer skips)", schedule.name());
    println!("forest inputs: prepared rows, packed columns, and power table");
    println!("lookup image:  excluded");
    println!("gamma fold:    included in fraction sumcheck");
    println!("dense leaves:  build/release included in naive witness build");
    println!("fraction tree: build/leaf release included in fraction witness build");
    println!("lazy JIT work: included in low-entropy sumcheck\n");
    std::io::stdout().flush().expect("flush benchmark header");

    let mut state = 0x7a31_5d8e_c021_49f7u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let rows = (0..cols)
        .map(|_| (0..row_len / 64).map(|_| next()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let powers = (0..row_len)
        .map(|_| vec![Gf::from_polynomial_words([next() | 1, next()])])
        .collect::<Vec<_>>();
    let mut packed = (0..cols.div_ceil(64))
        .map(|_| vec![0u64; row_len])
        .collect::<Vec<_>>();
    let mut leaves = Vec::with_capacity(n);
    for c in 0..cols {
        for i in 0..row_len {
            let bit = (rows[c][i >> 6] >> (i & 63)) & 1;
            packed[c >> 6][i] |= bit << (c & 63);
            leaves.push(if bit == 0 { Gf::ONE } else { powers[i][0] });
        }
    }
    let mut eager_transcript = Blake3Transcript::new();
    let mut eager_profile = MergedForestProfile::default();
    let eager = prove_merged_forest_profiled(
        &mut eager_transcript,
        &leaves,
        row_vars,
        col_vars,
        &mut eager_profile,
    );
    let mut lazy_transcript = Blake3Transcript::new();
    let mut lazy_profile = MergedForestProfile::default();
    let lazy = prove_merged_forest_lazy_from_rows_profiled(
        &mut lazy_transcript,
        &layout,
        &rows,
        &packed,
        &powers,
        cols,
        &mut lazy_profile,
    );
    assert_eq!(eager.0, lazy.0);
    assert_eq!(eager.2, lazy.2);
    assert_eq!(eager.3, lazy.3);
    assert_eq!(eager_transcript.state_digest(), lazy_transcript.state_digest());
    for (output, transcript) in [(&eager, eager_transcript), (&lazy, lazy_transcript)] {
        let mut verifier = Blake3Transcript::new();
        let verified = verify_merged_forest(
            &mut verifier,
            &output.0,
            &output.1,
            row_vars,
            col_vars,
        )
        .expect("forest verification");
        assert_eq!(verified, (output.2.clone(), output.3));
        assert_eq!(verifier.state_digest(), transcript.state_digest());
    }
    drop(eager);
    drop(lazy);
    drop(leaves);

    let mut eager_samples = Vec::with_capacity(runs);
    let mut lazy_samples = Vec::with_capacity(runs);
    let mut lazy_profiles = Vec::with_capacity(runs);
    let mut guard = 0u64;
    for run in 0..runs {
        let order = if run & 1 == 0 { [0, 1] } else { [1, 0] };
        for method in order {
            let sample = if method == 0 {
                let sample = eager_sample(&rows, &powers, row_vars, col_vars);
                eager_samples.push(sample.0);
                sample
            } else {
                let sample = lazy_sample(&layout, &rows, &packed, &powers);
                lazy_samples.push(sample.0);
                lazy_profiles.push(sample.1);
                (sample.0, sample.2)
            };
            guard = guard.rotate_left(9).wrapping_add(sample.1);
        }
    }
    drop(rows);
    drop(packed);
    drop(powers);

    let mut numerators = (0..n)
        .map(|_| Gf::from_polynomial_words([next(), next()]))
        .collect::<Vec<_>>();
    let mut denominators = (0..n)
        .map(|_| Gf::from_polynomial_words([next() | 1, next()]))
        .collect::<Vec<_>>();
    let mut left = FractionTreeWitness::default();
    let mut right = FractionTreeWitness::default();
    let mut workspace = ProductWorkspace::default();
    workspace.reserve(1, n / 2, log_n);
    left.rebuild_swapped(
        &mut numerators,
        &mut denominators,
        log_n,
        Gf::ZERO,
        Gf::ONE,
    );
    let root = left.root();
    right.rebuild(&[root.0], &[root.1], 0, Gf::ZERO, Gf::ONE);
    let mut fraction_transcript = Blake3Transcript::new();
    let (fraction_proof, fraction_claims) =
        prove_rational(&mut fraction_transcript, &mut left, &mut right, &mut workspace);
    let mut fraction_verifier = Blake3Transcript::new();
    let verified = verify_rational(&mut fraction_verifier, &fraction_proof, log_n, 0)
        .expect("fraction verification");
    assert_eq!(verified, fraction_claims);
    assert_eq!(fraction_verifier.state_digest(), fraction_transcript.state_digest());
    left.release_leaves(&mut numerators, &mut denominators);

    let mut fraction_samples = Vec::with_capacity(runs);
    let mut fraction_profiles = Vec::with_capacity(runs);
    for _ in 0..runs {
        let sample = fraction_sample(
            &mut numerators,
            &mut denominators,
            log_n,
            &mut left,
            &mut right,
            &mut workspace,
        );
        fraction_samples.push(sample.0);
        fraction_profiles.push(sample.1);
        guard = guard.rotate_left(9).wrapping_add(sample.2);
    }

    print_samples("naive product GKR", &eager_samples);
    println!();
    print_samples("low-entropy product GKR", &lazy_samples);
    for (label, values) in [
        (
            "bit/JIT generation",
            forest_duration_summary(&lazy_profiles, |p| p.bit_generation),
        ),
        (
            "LUT table build",
            forest_duration_summary(&lazy_profiles, |p| p.lut_tables),
        ),
        (
            "LUT prefix messages",
            forest_duration_summary(&lazy_profiles, |p| p.lut_prefix_messages),
        ),
        (
            "LUT prefix folds",
            forest_duration_summary(&lazy_profiles, |p| p.lut_prefix_folds),
        ),
        (
            "LUT dense-tail msgs",
            forest_duration_summary(&lazy_profiles, |p| p.lut_tail_messages),
        ),
        (
            "LUT dense-tail folds",
            forest_duration_summary(&lazy_profiles, |p| p.lut_tail_folds),
        ),
        (
            "upper-layer msgs",
            forest_duration_summary(&lazy_profiles, |p| p.upper_messages),
        ),
        (
            "upper-layer folds",
            forest_duration_summary(&lazy_profiles, |p| p.upper_folds),
        ),
        (
            "phase-A close/setup",
            forest_duration_summary(&lazy_profiles, |p| p.phase_a_close),
        ),
        (
            "phase-B sumchecks",
            forest_duration_summary(&lazy_profiles, |p| p.phase_b),
        ),
        (
            "unaccounted driver",
            forest_duration_summary(&lazy_profiles, |p| {
                p.sumcheck.saturating_sub(
                    p.bit_generation
                        + p.lut_tables
                        + p.lut_prefix_messages
                        + p.lut_prefix_folds
                        + p.lut_tail_messages
                        + p.lut_tail_folds
                        + p.upper_messages
                        + p.upper_folds
                        + p.phase_a_close
                        + p.phase_b,
                )
            }),
        ),
    ] {
        println!(
            "    {label:<22} best {:9.3} ms | median {:9.3} ms | worst {:9.3} ms | MAD {:8.3} ms",
            values.0, values.1, values.2, values.3,
        );
    }
    println!();
    print_samples("Logup fraction-sum GKR", &fraction_samples);
    for (label, values) in [
        (
            "ac evaluations",
            fraction_duration_summary(&fraction_profiles, |p| p.ac_evals),
        ),
        (
            "gamma folds",
            fraction_duration_summary(&fraction_profiles, |p| p.gamma_folds),
        ),
        (
            "product sumchecks",
            fraction_duration_summary(&fraction_profiles, |p| p.product_sumchecks),
        ),
        (
            "numerator evals",
            fraction_duration_summary(&fraction_profiles, |p| p.numerator_evals),
        ),
    ] {
        println!(
            "    {label:<18} best {:9.3} ms | median {:9.3} ms | worst {:9.3} ms | MAD {:8.3} ms",
            values.0, values.1, values.2, values.3,
        );
    }

    let eager_total = summary(&eager_samples, |sample| sample.witness + sample.sumcheck).1;
    let lazy_total = summary(&lazy_samples, |sample| sample.witness + sample.sumcheck).1;
    let fraction_total = summary(&fraction_samples, |sample| sample.witness + sample.sumcheck).1;
    let eager_sumcheck = summary(&eager_samples, |sample| sample.sumcheck).1;
    let lazy_sumcheck = summary(&lazy_samples, |sample| sample.sumcheck).1;
    println!("\nmedian ratios");
    println!("  naive / low-entropy sumcheck: {:7.3}x", eager_sumcheck / lazy_sumcheck);
    println!("  naive / low-entropy total:    {:7.3}x", eager_total / lazy_total);
    println!("  fraction / naive total:       {:7.3}x", fraction_total / eager_total);
    println!("guard:          {guard:016x}");
}
