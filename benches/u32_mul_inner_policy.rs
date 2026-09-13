//! Controlled inner-sumcheck policy benchmark for u32 multiplication.
//!
//! The outer sumcheck is always delayed Barrett and the assignment remains
//! native `u64` through inner round zero. Only native witness folding and
//! field-valued coefficient accumulation vary.

mod common;

use std::{hint::black_box};

use f2z::piop::spartan::{
    SpartanInnerFieldAccumulation, SpartanInnerNativeFold, SpartanInnerPolicy, U32MulWitness,
    prepare_u32_mul_relation, project_u32_mul_native_witness,
    prove_spartan_piop_u32_native_barrett_with_inner_policy, spartan_f2z_field_config,
    verify_spartan_proof,
};
use f2z::transcript::Blake3Transcript;
use rand::{RngExt, SeedableRng, rngs::StdRng};

const ASSIGNMENT_BINDING: [u8; 32] = [0x49; 32];

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|left, right| left.total_cmp(right));
    samples[samples.len() / 2]
}

fn env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive decimal integer")),
        Err(_) => default,
    }
}

fn exponents() -> Vec<usize> {
    common::shapes(None).map_or_else(
        || (15..=23).collect(),
        |shapes| {
            shapes
                .iter()
                .map(|part| {
                    let exponent = part
                        .parse::<usize>()
                        .expect("F2Z_BENCH_SHAPES contains integers");
                    assert!((15..=23).contains(&exponent));
                    exponent
                })
                .collect()
        },
    )
}

fn inner_policy() -> SpartanInnerPolicy {
    let native_witness_fold = match std::env::var("F2Z_INNER_NATIVE_FOLD").as_deref() {
        Ok("immediate") => SpartanInnerNativeFold::Immediate,
        Ok("delayed") | Err(_) => SpartanInnerNativeFold::Delayed,
        Ok(value) => panic!("unsupported F2Z_INNER_NATIVE_FOLD={value}; use immediate or delayed"),
    };
    let field_coefficients = match std::env::var("F2Z_INNER_FIELD_ACCUM").as_deref() {
        Ok("immediate") => SpartanInnerFieldAccumulation::Immediate,
        Ok("delayed") | Err(_) => SpartanInnerFieldAccumulation::Delayed,
        Ok(value) => {
            let threshold = value
                .strip_prefix("threshold:")
                .unwrap_or_else(|| {
                    panic!(
                        "unsupported F2Z_INNER_FIELD_ACCUM={value}; use immediate, delayed, or threshold:<pairs>"
                    )
                })
                .parse()
                .expect("the field-accumulation threshold is a usize");
            SpartanInnerFieldAccumulation::DelayedAtOrAbovePairs(threshold)
        }
    };
    SpartanInnerPolicy {
        native_witness_fold,
        field_coefficients,
    }
}

const fn native_fold_name(policy: SpartanInnerPolicy) -> &'static str {
    match policy.native_witness_fold {
        SpartanInnerNativeFold::Immediate => "immediate",
        SpartanInnerNativeFold::Delayed => "delayed",
    }
}

fn field_accumulation_name(policy: SpartanInnerPolicy) -> String {
    match policy.field_coefficients {
        SpartanInnerFieldAccumulation::Immediate => "immediate".to_owned(),
        SpartanInnerFieldAccumulation::Delayed => "delayed".to_owned(),
        SpartanInnerFieldAccumulation::DelayedAtOrAbovePairs(threshold) => {
            format!("threshold-{threshold}")
        }
    }
}

fn phase_ms(phases: &[(String, f64)], label: &str) -> f64 {
    phases
        .iter()
        .filter(|(phase, _)| *phase == label)
        .map(|(_, seconds)| seconds * 1e3)
        .sum()
}

fn spartan_payload_elements(
    proof: &f2z::piop::spartan::SpartanPiopProof<f2z::piop::spartan::SpartanF2zField>,
) -> usize {
    4 * proof.outer.sumcheck.round_polynomials.len() + 3 + 3 * proof.inner.round_polynomials.len()
}

fn bench_exponent(
    exponent: usize,
    reps: usize,
    root_seed: u64,
    policy: SpartanInnerPolicy,
    order: usize,
) {
    let multiplications = 1usize << exponent;
    let shape_seed = root_seed ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng = StdRng::seed_from_u64(shape_seed);
    let witness = U32MulWitness::from_fn(multiplications, |_| {
        (rng.random::<u32>(), rng.random::<u32>())
    })
    .expect("valid u32 multiplication witness");
    let layout = *witness.layout();
    let field_config = spartan_f2z_field_config();
    let relation = prepare_u32_mul_relation(layout, &field_config).expect("valid relation");
    let (assignment, products) = project_u32_mul_native_witness(&witness).into_parts();

    let mut warm_transcript = Blake3Transcript::new();
    let (warm_proof, warm_claim) = prove_spartan_piop_u32_native_barrett_with_inner_policy(
        &mut warm_transcript,
        &relation,
        &ASSIGNMENT_BINDING,
        products.clone(),
        assignment.clone(),
        policy,
    )
    .expect("warm-up proving succeeds");
    let mut warm_verifier = Blake3Transcript::new();
    let verified_claim = verify_spartan_proof(
        &mut warm_verifier,
        &relation,
        &ASSIGNMENT_BINDING,
        &warm_proof,
    )
    .expect("warm-up verification succeeds");
    assert_eq!(warm_claim, verified_claim);
    drop(warm_proof);


    let native_fold = native_fold_name(policy);
    let field_accumulation = field_accumulation_name(policy);
    let policy_name = format!("fold-{native_fold}__field-{field_accumulation}");
    let mut prove_samples = Vec::with_capacity(reps);
    let mut outer_samples = Vec::with_capacity(reps);
    let mut bind_samples = Vec::with_capacity(reps);
    let mut inner_samples = Vec::with_capacity(reps);
    let mut native_coefficient_samples = Vec::with_capacity(reps);
    let mut native_fold_samples = Vec::with_capacity(reps);
    let mut field_round_samples = Vec::with_capacity(reps);
    let mut verify_samples = Vec::with_capacity(reps);
    let mut payload_elements = 0;

    for sample in 0..reps {
        let sample_products = products.clone();
        let sample_assignment = assignment.clone();
        let mut prover_transcript = Blake3Transcript::new();
        let recording = f2z::observability::Recording::start(Vec::new()).expect("start inner-policy prove");
        let proving = tracing::info_span!("benchmark:proving").entered();
        let (proof, claim) = prove_spartan_piop_u32_native_barrett_with_inner_policy(
            &mut prover_transcript,
            &relation,
            &ASSIGNMENT_BINDING,
            sample_products,
            sample_assignment,
            policy,
        )
        .expect("proving succeeds");
        drop(proving);
        let intervals = recording.intervals().expect("query inner-policy prove");
        let prove_ms = common::span_ms(&intervals, "benchmark:proving");
        let phases = f2z::observability::phase_totals(&intervals, "benchmark:proving").unwrap();
        let outer_ms = phase_ms(&phases, "spartan:outer_sumcheck");
        let bind_ms = phase_ms(&phases, "spartan:bind_and_batch");
        let inner_ms = phase_ms(&phases, "spartan:inner_sumcheck");
        let native_coefficients_ms = phase_ms(&phases, "spartan:inner_native_coefficients");
        let native_witness_fold_ms = phase_ms(&phases, "spartan:inner_native_witness_fold");
        let field_rounds_ms = phase_ms(&phases, "spartan:inner_field_rounds");

        let mut verifier_transcript = Blake3Transcript::new();
        let (verified_claim, duration) = f2z::observability::measure(tracing::info_span!("benchmark:verification"), || verify_spartan_proof(
            &mut verifier_transcript,
            &relation,
            &ASSIGNMENT_BINDING,
            &proof,
        )
        .expect("verification succeeds")).expect("measure inner-policy verification");
        let verify_ms = duration.as_secs_f64() * 1e3;
        assert_eq!(claim, verified_claim);
        payload_elements = spartan_payload_elements(&proof);
        let payload_bytes = 16 * payload_elements;
        black_box(&proof);

        prove_samples.push(prove_ms);
        outer_samples.push(outer_ms);
        bind_samples.push(bind_ms);
        inner_samples.push(inner_ms);
        native_coefficient_samples.push(native_coefficients_ms);
        native_fold_samples.push(native_witness_fold_ms);
        field_round_samples.push(field_rounds_ms);
        verify_samples.push(verify_ms);
        println!(
            "SAMPLE order={order} policy={policy_name} native_fold={native_fold} field_accumulation={field_accumulation} exponent={exponent} multiplications={multiplications} integer_witness_values={} sample={sample} spartan_prove_ms={prove_ms:.9} spartan_outer_ms={outer_ms:.9} spartan_bind_ms={bind_ms:.9} spartan_inner_ms={inner_ms:.9} inner_native_coefficients_ms={native_coefficients_ms:.9} inner_native_witness_fold_ms={native_witness_fold_ms:.9} inner_field_rounds_ms={field_rounds_ms:.9} spartan_verify_ms={verify_ms:.9} spartan_proof_payload_bytes={payload_bytes} verified=true",
            layout.assignment_len(),
        );
    }

    let prove_median = median(prove_samples);
    let outer_median = median(outer_samples);
    let bind_median = median(bind_samples);
    let inner_median = median(inner_samples);
    let native_coefficient_median = median(native_coefficient_samples);
    let native_fold_median = median(native_fold_samples);
    let field_round_median = median(field_round_samples);
    let verify_median = median(verify_samples);
    let payload_bytes = 16 * payload_elements;
    println!(
        "RESULT order={order} policy={policy_name} native_fold={native_fold} field_accumulation={field_accumulation} exponent={exponent} multiplications={multiplications} r1cs_rows={multiplications} integer_witness_values={} spartan_prove_ms={prove_median:.9} spartan_outer_ms={outer_median:.9} spartan_bind_ms={bind_median:.9} spartan_inner_ms={inner_median:.9} inner_native_coefficients_ms={native_coefficient_median:.9} inner_native_witness_fold_ms={native_fold_median:.9} inner_field_rounds_ms={field_round_median:.9} spartan_verify_ms={verify_median:.9} spartan_proof_payload_bytes={payload_bytes} shape_seed={shape_seed:#018x} verified_samples={reps}",
        layout.assignment_len(),
    );
}

fn main() {
    f2z::observability::install().expect("install Perfetto subscriber");
    common::enforce_known_env();
    let _ = flock_core::init_perf_thread_pool();
    let reps = env_usize("F2Z_BENCH_REPS", 5);
    assert!(reps > 0);
    let root_seed = common::seed(None, 0x5533_326d_756c_0064);
    let order = env_usize("F2Z_BENCH_ORDER", 1);
    let policy = inner_policy();

    println!("u32 inner-sumcheck policy benchmark");
    #[cfg(feature = "parallel")]
    println!("rayon threads: {}", rayon::current_num_threads());
    println!(
        "policy: fold={}, field={}; repetitions: {reps}; root seed: {root_seed:#018x}",
        native_fold_name(policy),
        field_accumulation_name(policy),
    );

    for exponent in exponents() {
        flock_core::scratch::clear();
        bench_exponent(exponent, reps, root_seed, policy, order);
    }
    flock_core::scratch::clear();
}
