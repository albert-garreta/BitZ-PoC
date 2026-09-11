//! End-to-end benchmark for independent BabyBear multiplications represented
//! by the exact integer relation `a * b = c + p * k`.
//!
//! Spartan proves the integer-valued R1CS relation over a transcript-sampled
//! Step-2 prime (the paper path; commit-before-prime, Zaratan order), and the
//! terminal assignment-MLE claim is discharged against the compact
//! 31/31/31/31-bit witness with F2Z. Every measured proof is verified.
//!
//! Output follows the unified schema (`docs/bench-schema.md`), and every
//! shape is measured TWICE on the same witness — once per security target:
//! the `Lambda100` default (no grinding anywhere) and `Lambda128` (initial,
//! per-PIOP-draw, and forest/GKR grinding all armed; every term this crate
//! controls ≥ 128 bits, the GF(2^128) floor reported as binding). A target
//! whose derived grinding exceeds the economic cap at a shape prints a
//! skip line instead of a row. `F2Z_BENCH_LAMBDA=100|128|sha128-reference-schedule`
//! restricts a run to ONE profile (one row per shape; the two-prime
//! `Limber114` profile is MultiSwap-only and is rejected here).
//!
//! Defaults to the sweep `2^15, ..., 2^25`. Override with `F2Z_BENCH_SHAPES`
//! (deprecated alias `F2Z_BABY_BEAR_MUL_EXPONENTS`):
//!
//! ```text
//! F2Z_BENCH_SHAPES="15 17 19" F2Z_BENCH_REPS=3 \
//!   cargo bench --bench baby_bear_mul --features unchecked
//! ```
//!
//! `F2Z_SPARTAN_REDUCTION` selects `immediate`, `delayed-barrett`, or
//! `delayed-crypto-bigint`; the production default is `delayed-barrett`.
//! NOTE: the pre-schema output (and its `bench-peak-memory` pass) that
//! `scripts/baby_bear_mul_bench_report.py` parses is available at commit
//! b7713d8; the script has not been ported to `schema=f2z/1`.

mod common;

use std::hint::black_box;
use std::time::Instant;

use f2z::piop::spartan::{
    BABY_BEAR_MODULUS, BabyBearMulWitness, BabyBearSpartanF2zError, IopSecurityProfile, Lambda100,
    Lambda128, PreparedBabyBearMulRelation, PrimePolicy, SpartanReductionStrategy,
    commit_baby_bear_mul_paper_witness, prove_baby_bear_mul_paper, sample_baby_bear_operand_with,
    verify_baby_bear_mul_paper,
};
use f2z::transcript::Blake3Transcript;
use rand::{RngExt, SeedableRng, rngs::StdRng};

fn reduction_strategy() -> SpartanReductionStrategy {
    match std::env::var("F2Z_SPARTAN_REDUCTION").as_deref() {
        Ok("immediate") => SpartanReductionStrategy::Immediate,
        Ok("delayed-barrett") | Err(_) => SpartanReductionStrategy::DelayedBarrett,
        Ok("delayed-crypto-bigint") => SpartanReductionStrategy::DelayedCryptoBigint,
        Ok(value) => panic!(
            "unsupported F2Z_SPARTAN_REDUCTION={value}; use immediate, delayed-barrett, or delayed-crypto-bigint"
        ),
    }
}

const fn strategy_name(strategy: SpartanReductionStrategy) -> &'static str {
    match strategy {
        SpartanReductionStrategy::Immediate => "immediate",
        SpartanReductionStrategy::DelayedBarrett => "delayed-barrett",
        SpartanReductionStrategy::DelayedCryptoBigint => "delayed-crypto-bigint",
    }
}

fn exponents() -> Vec<usize> {
    common::shapes(Some("F2Z_BABY_BEAR_MUL_EXPONENTS")).map_or_else(
        || (15..=25).collect(),
        |shapes| {
            shapes
                .iter()
                .map(|part| {
                    let exponent: usize =
                        part.parse().expect("F2Z_BENCH_SHAPES contains integers");
                    assert!(
                        exponent >= 15,
                        "the combined proof requires at least 2^15 gate slots"
                    );
                    exponent
                })
                .collect()
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn bench_profile<P: IopSecurityProfile>(
    exponent: usize,
    witness: &BabyBearMulWitness,
    witness_ms: f64,
    reps: usize,
    strategy: SpartanReductionStrategy,
    threads: usize,
    seed: u64,
    shape_seed: u64,
) {
    let multiplications = 1usize << exponent;
    let layout = *witness.layout();
    let params = layout.f2z_params();

    // One-time public preprocessing under this profile (excluded from
    // prove): raw exact matrices + the instantiated security parameters.
    let setup_started = Instant::now();
    let prepared = match PreparedBabyBearMulRelation::new_with_profile_and_ligerito::<P>(layout, common::ligerito_selection(P::LIGERITO_TARGET_BITS)) {
        Ok(prepared) => prepared,
        Err(error @ (BabyBearSpartanF2zError::Profile(_)
        | BabyBearSpartanF2zError::UnsupportedPaperProfile)) => {
            println!();
            println!(
                "baby_bear_mul gates=2^{exponent} profile={}: SKIPPED - {error}",
                P::NAME
            );
            return;
        }
        Err(error) => panic!("prepare failed: {error}"),
    };
    let setup_ms = common::elapsed_ms(setup_started);
    println!("LIGERITO_CONFIG {}", common::ligerito_report(prepared.ligerito_configuration(), prepared.security().ood));
    let security = prepared.security().clone();

    println!();
    println!(
        "baby_bear_mul gates=2^{exponent} ({multiplications}) p={BABY_BEAR_MODULUS} \
         [{}] profile={} seed={shape_seed:#018x}",
        strategy_name(strategy),
        P::NAME,
    );
    println!(
        "  grinding: initial {} | per-PIOP-draw {} | terminal {} | forest/round {} | \
         ligerito target {}",
        security.initial_grinding_bits,
        security
            .initial_grinding_bits
            .max(security.piop_round_grinding_bits),
        security.terminal_grinding_bits,
        security.forest_round_grinding_bits,
        security.ligerito_target_bits,
    );

    // Excluded warm-up; also the first end-to-end correctness check.
    let warm_hint =
        commit_baby_bear_mul_paper_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut warm_transcript = Blake3Transcript::new();
    let warm_proof =
        prove_baby_bear_mul_paper(&mut warm_transcript, &prepared, witness, &warm_hint, strategy)
            .expect("warm-up prove");
    let mut warm_verifier = Blake3Transcript::new();
    verify_baby_bear_mul_paper(&mut warm_verifier, &prepared, &warm_hint.commitment, &warm_proof)
        .expect("warm-up verify");
    drop((warm_proof, warm_hint));
    let _ = f2z::utils::prof::take_totals();

    let mut prover = common::StepSamples::default();
    let mut verifier = common::StepSamples::default();
    let mut last = None;
    for _ in 0..reps {
        let _ = f2z::utils::prof::take_totals();
        let prove_started = Instant::now();
        let commit_started = Instant::now();
        let hint =
            commit_baby_bear_mul_paper_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
        let commit_ms = common::elapsed_ms(commit_started);
        let mut prover_transcript = Blake3Transcript::new();
        let proof =
            prove_baby_bear_mul_paper(&mut prover_transcript, &prepared, witness, &hint, strategy)
                .expect("prove");
        let prove_ms = common::elapsed_ms(prove_started);
        let prove_phases = f2z::utils::prof::take_totals();

        let verify_started = Instant::now();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_baby_bear_mul_paper(&mut verifier_transcript, &prepared, &hint.commitment, &proof)
            .expect("verify");
        let verify_ms = common::elapsed_ms(verify_started);
        let verify_phases = f2z::utils::prof::take_totals();

        black_box(&proof);
        prover.record_prove(prove_ms, commit_ms, &prove_phases);
        verifier.record_verify(verify_ms, &verify_phases);
        last = Some(proof);
    }
    let proof = last.expect("at least one measured repetition");

    let spartan_elements = proof.spartan_payload_elements();
    let boundary_nonces =
        proof.grinding_nonce_count(&security) - proof.f2z().grinding_nonces.len();
    let report = common::BenchReport {
        bench: "baby_bear_mul",
        shape: format!("2p{exponent}"),
        extra: vec![
            common::ligerito_identity(prepared.ligerito_configuration(), prepared.security().ood),
            ("profile".into(), P::NAME.into()),
            ("multiplications".into(), multiplications.to_string()),
            ("strategy".into(), strategy_name(strategy).into()),
            ("baby_bear_modulus".into(), BABY_BEAR_MODULUS.to_string()),
            ("f2z_t".into(), params.row_vars.to_string()),
            ("f2z_s".into(), params.col_vars.to_string()),
            (
                "forest_grinding_nonces".into(),
                proof.f2z().grinding_nonces.len().to_string(),
            ),
            ("shape_seed".into(), format!("{shape_seed:#018x}")),
        ],
        lambda: Some(security.lambda),
        lambda_achieved: Some(security.accounting.achieved_bits()),
        lambda_bind: Some(security.accounting.binding_term().name.into()),
        threads,
        reps,
        seed: Some(seed),
        witness_ms,
        setup_ms,
        prover: prover.medians(),
        verifier: verifier.medians(),
        proof: common::ProofBytes {
            piop: spartan_elements * 16 + 8 * boundary_nonces,
            open: proof.f2z().to_bytes().len(),
        },
    };
    report.print_human();
}

fn main() {
    let threads = common::init();
    let reps = common::reps(None, 5);
    let strategy = reduction_strategy();
    let seed = common::seed(Some("F2Z_BABY_BEAR_MUL_SEED"), 0x6262_6d75_6c5f_0031);
    let selected = common::security_profile(PrimePolicy::SingleDerived);

    println!("BabyBear a*b = c + p*k: paper-path Spartan PIOP + F2Z assignment opening");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {threads}");
    println!(
        "repetitions: {reps}; root seed: {seed:#018x}; strategy: {}; {}",
        strategy_name(strategy),
        match selected {
            Some(profile) => format!(
                "one row per shape: {} (λ={}, F2Z_BENCH_LAMBDA={})",
                profile.name(),
                profile.lambda(),
                profile.knob_value()
            ),
            None => "two rows per shape (Lambda100 + Lambda128, one shared witness; \
                     F2Z_BENCH_LAMBDA selects one)"
                .to_owned(),
        },
    );

    for exponent in exponents() {
        flock_core::scratch::clear();
        let multiplications = 1usize << exponent;
        let shape_seed = seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut rng = StdRng::seed_from_u64(shape_seed);

        // Witness generation (excluded from prove); shared by both profiles
        // so the two rows are directly comparable.
        let started = Instant::now();
        let witness = BabyBearMulWitness::from_fn(multiplications, |_| {
            (
                sample_baby_bear_operand_with(|| rng.random::<u32>()),
                sample_baby_bear_operand_with(|| rng.random::<u32>()),
            )
        })
        .expect("valid BabyBear multiplication witness");
        let witness_ms = common::elapsed_ms(started);

        match selected {
            None => {
                bench_profile::<Lambda100>(
                    exponent, &witness, witness_ms, reps, strategy, threads, seed, shape_seed,
                );
                bench_profile::<Lambda128>(
                    exponent, &witness, witness_ms, reps, strategy, threads, seed, shape_seed,
                );
            }
            Some(profile) => common::with_profile!(
                profile,
                bench_profile(
                    exponent, &witness, witness_ms, reps, strategy, threads, seed, shape_seed,
                )
            ),
        }
    }
    flock_core::scratch::clear();
}
