use super::{Corpus, Timing, Workload};
use f2z::{
    piop::spartan::{
        Lambda100, PreparedU32MulRelation, PreparedU64MulRelation, PreparedU128MulRelation,
        U32MulLayout, U32MulWitness, U64MulLayout, U64MulWitness, U128MulLayout, U128MulWitness,
        commit_u32_mul_witness, commit_u64_mul_witness, commit_u128_mul_witness,
        f2z::U32MulLigerito, prove_u32_mul, prove_u64_mul, prove_u128_mul, verify_u32_mul,
        verify_u64_mul, verify_u128_mul,
    },
    transcript::Blake3Transcript,
    observability::Recording,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// `F2Z_U64_SPLIT_SHIFT=k`: lower the u64 F2Z row side by `k` variables
/// below the layout's default split (raise the column side by `k`). An
/// explicit campaign knob: the runner clears the ambient value, sets it per
/// campaign, records it, and checks every sample against it.
fn u64_split_shift() -> i8 {
    std::env::var("F2Z_U64_SPLIT_SHIFT")
        .map(|value| value.parse().expect("F2Z_U64_SPLIT_SHIFT must be an i8"))
        .unwrap_or(0)
}

enum Relation {
    U32(PreparedU32MulRelation),
    U64(PreparedU64MulRelation),
    U128(PreparedU128MulRelation),
}
pub(super) struct Context {
    corpus: Arc<Corpus>,
    relation: Relation,
    split_shift: i8,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let n = corpus.len();
        let split_shift = if corpus.workload == Workload::U64 { u64_split_shift() } else { 0 };
        let relation = match corpus.workload {
            Workload::U32 => {
                let relation = PreparedU32MulRelation::new_with_profile_and_ligerito::<Lambda100>(
                    U32MulLayout::new(n).unwrap(),
                    super::common::ligerito_selection(100),
                )
                .expect("u32 Johnson relation");
                Relation::U32(relation)
            }
            Workload::U64 => Relation::U64(
                PreparedU64MulRelation::new_with_profile_and_ligerito::<Lambda100>(
                    U64MulLayout::new(n)
                        .unwrap()
                        .with_split_shift(split_shift)
                        .unwrap(),
                    super::common::ligerito_selection(100),
                )
                .unwrap(),
            ),
            Workload::U128 => Relation::U128(
                PreparedU128MulRelation::new_with_profile_and_ligerito::<Lambda100>(
                    U128MulLayout::new(n).unwrap(),
                    super::common::ligerito_selection(100),
                )
                .unwrap(),
            ),
        };
        Self { corpus, relation, split_shift }
    }
    pub(super) fn config(&self) -> Value {
        let mut config = json!({"profile":"Lambda100","target_bits":100,"piop":"Spartan over transcript-sampled prime","pcs":"F2Z/Ligerito","word_bits":1});
        if let Relation::U32(relation) = &self.relation {
            let security = relation.security();
            let params = relation.params();
            let ligerito = relation.ligerito_configuration();
            let terms: Vec<_> = security
                .accounting
                .terms
                .iter()
                .map(|term| {
                    json!({
                        "name": term.name, "bits": term.bits,
                        "grinding_bits": term.grinding_bits, "floor": term.floor,
                    })
                })
                .collect();
            config["relation"] = json!("x*y = z + 2^32*w; four committed 32-bit limbs");
            config["security_scope"] = json!("round-by-round-economic");
            config["ligerito_regime"] = json!(if security.ood.is_some() {
                "johnson"
            } else {
                "udr"
            });
            config["ood_present"] = json!(security.ood.is_some());
            config["modeled_min_bits"] = json!(security.accounting.achieved_bits());
            config["security_terms"] = json!(terms);
            config["projection_prime_min"] = json!(security.projection_min.to_string());
            config["projection_prime_max"] = json!(security.projection_max.to_string());
            config["ood_grinding_bits"] = json!(security.ood.map(|p| p.grinding_bits));
            config["geometry"] =
                json!({"t":params.row_vars, "s":params.col_vars, "word_bits":params.word_bits});
            config["ligerito"] = super::common::ligerito_report(ligerito, security.ood);
        }
        match &self.relation {
            Relation::U64(p) => {
                config["ligerito"] =
                    super::common::ligerito_report(p.ligerito_configuration(), p.security().ood);
                let params = p.layout().f2z_params();
                config["u64_split_shift"] = json!(self.split_shift);
                config["f2z_t"] = json!(params.row_vars);
                config["f2z_s"] = json!(params.col_vars);
            }
            Relation::U128(p) => {
                config["ligerito"] =
                    super::common::ligerito_report(p.ligerito_configuration(), p.security().ood)
            }
            _ => {}
        }
        config
    }
    pub(super) fn run(&self) -> Timing {
        let recording = Recording::start(Vec::new()).expect("start F2Z trial");
        let proof_bytes = self.prove_and_verify();
        let raw = recording.intervals().expect("query F2Z trial");
        let trial = super::trace_capture::TrialScopes::from_spans(&raw, "benchmark");
        let mut timing = Timing::from_trial(&trial, proof_bytes);
        for (label, name, tag) in [
            ("native-mul:commit", "commit", "commit"),
            ("step2:project_prove", "prime_projection", "preparation"),
            ("step3:piop_prove", "piop", "constraint-proof"),
            ("step4:bitify_prove", "claim_bridge", "opening-proof"),
            ("step5:open_prove", "opening", "opening-proof"),
        ] {
            let mut matching = raw.iter().filter(|s| s.label() == label);
            let s = matching.next().unwrap_or_else(|| panic!("missing F2Z {label}"));
            assert!(matching.next().is_none(), "duplicate F2Z {label}");
            timing.add(name, tag, s.start_ns, s.end_ns);
        }
        super::common::print_regression_phases(&::f2z::observability::totals(
            raw.iter().filter(|s| s.end_ns <= trial.verification.start_ns),
        ));
        for span in raw.iter().filter(|s| s.label() == "mc:forest") {
            timing.add("gkr", "gkr", span.start_ns, span.end_ns);
        }
        timing
    }

    pub(super) fn prove_and_verify(&self) -> usize {
        let root = tracing::info_span!("Verified trial", component = "benchmark.verified-trial", tag_end_to_end = true).entered();
        let total = tracing::info_span!("Witness to proof", component = "benchmark.witness-to-proof").entered();
        // Serialized proof size: the commitment root, the Spartan payload as
        // 16-byte field elements, the transmitted nonces outside the opening
        // as 8-byte words, and the F2Z opening's exact codec bytes.
        let proof_bytes = match &self.relation {
            Relation::U32(relation) => {
                let witness = {
                    let _s = tracing::info_span!("Witness generation", component = "benchmark.witness-evaluation", tag_witness_generation = true).entered();
                    U32MulWitness::from_inputs(&self.corpus.narrow_inputs()).expect("u32 witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(
                        u128::from(witness.product_values()[i] as u32),
                        self.corpus.workload.output(a, b)
                    );
                }
                let online = tracing::info_span!("native-mul:online").entered();
                let hint = {
                    let _s = tracing::info_span!("native-mul:commit").entered();
                    commit_u32_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u32 commitment")
                };
                let mut transcript = Blake3Transcript::new();
                let proof = prove_u32_mul(&mut transcript, relation, &witness, &hint)
                    .expect("u32 full proof");
                drop(online);
                drop(total);
                {
                    let _s = tracing::info_span!("Verification", component = "benchmark.verification", tag_verification = true).entered();
                    verify_u32_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u32 full verification");
                }
                drop(root);
                super::common::proof_fingerprint::nonlinear(&proof, &hint.commitment.root, &transcript);
                let bytes = hint.commitment.root.len()
                    + proof.spartan_payload_elements() * 16
                    + (proof.grinding_nonce_count(relation.security())
                        - proof.f2z().grinding_nonces.len())
                        * 8
                    + proof.f2z().to_bytes().len();
                std::hint::black_box(proof);
                bytes
            }
            Relation::U64(relation) => {
                let witness = {
                    let _s = tracing::info_span!("Witness generation", component = "benchmark.witness-evaluation", tag_witness_generation = true).entered();
                    U64MulWitness::from_inputs(self.corpus.inputs())
                        .and_then(|w| w.with_split_shift(self.split_shift))
                        .expect("u64 witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(witness.product(i), self.corpus.workload.output(a, b));
                }
                let online = tracing::info_span!("native-mul:online").entered();
                let hint = {
                    let _s = tracing::info_span!("native-mul:commit").entered();
                    commit_u64_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u64 commitment")
                };
                let mut transcript = Blake3Transcript::new();
                let proof = prove_u64_mul(&mut transcript, relation, &witness, &hint)
                    .expect("u64 full proof");
                drop(online);
                drop(total);
                {
                    let _s = tracing::info_span!("Verification", component = "benchmark.verification", tag_verification = true).entered();
                    verify_u64_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u64 full verification");
                }
                drop(root);
                super::common::proof_fingerprint::nonlinear(&proof, &hint.commitment.root, &transcript);
                let bytes = hint.commitment.root.len() + proof.size_bytes(relation.security());
                std::hint::black_box(proof);
                bytes
            }
            Relation::U128(relation) => {
                let witness = {
                    let _s = tracing::info_span!("Witness generation", component = "benchmark.witness-evaluation", tag_witness_generation = true).entered();
                    U128MulWitness::from_inputs(self.corpus.wide_inputs()).expect("u128 witness")
                };
                for (i, &(x, y)) in self.corpus.wide_inputs().iter().enumerate() {
                    let [_, _, lo, hi] = self.corpus.workload.wide_row(x, y);
                    assert_eq!(witness.product(i), (lo, hi));
                }
                let online = tracing::info_span!("native-mul:online").entered();
                let hint = {
                    let _s = tracing::info_span!("native-mul:commit").entered();
                    commit_u128_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u128 commitment")
                };
                let mut transcript = Blake3Transcript::new();
                let proof = prove_u128_mul(&mut transcript, relation, &witness, &hint)
                    .expect("u128 full proof");
                drop(online);
                drop(total);
                {
                    let _s = tracing::info_span!("Verification", component = "benchmark.verification", tag_verification = true).entered();
                    verify_u128_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u128 full verification");
                }
                drop(root);
                super::common::proof_fingerprint::nonlinear(&proof, &hint.commitment.root, &transcript);
                let bytes = hint.commitment.root.len() + proof.size_bytes(relation.security());
                std::hint::black_box(proof);
                bytes
            }
        };
        proof_bytes
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let split_shift = if corpus.workload == Workload::U64 { u64_split_shift() } else { 0 };
    let started_recording = f2z::observability::Recording::start(Vec::new()).expect("start operation capture");
    let started = tracing::info_span!("mul_e2e_compare/f2z:started").entered();
    if corpus.workload.is_wide() {
        let w = U128MulWitness::from_inputs(corpus.wide_inputs()).unwrap();
        let ms = { drop(started); f2z::observability::duration(&started_recording.intervals().expect("complete operation capture"), "mul_e2e_compare/f2z:started").expect("query completed operation") }.as_secs_f64() * 1e3;
        let rows = (0..corpus.len())
            .map(|i| {
                [
                    w.x_values()[i],
                    w.y_values()[i],
                    w.z_lo_values()[i],
                    w.z_hi_values()[i],
                ]
            })
            .collect();
        return super::WitnessAudit::check_wide(corpus, rows, ms, "F2Z integer assignment");
    }
    let (rows, generation_ms) = match corpus.workload {
        Workload::U32 => {
            let w = U32MulWitness::from_inputs(&corpus.narrow_inputs()).unwrap();
            let ms = { drop(started); f2z::observability::duration(&started_recording.intervals().expect("complete operation capture"), "mul_e2e_compare/f2z:started").expect("query completed operation") }.as_secs_f64() * 1e3;
            (
                (0..corpus.len())
                    .map(|i| {
                        let product = w.product_values()[i];
                        [
                            w.x_values()[i],
                            w.y_values()[i],
                            u64::from(product as u32),
                            product >> 32,
                        ]
                    })
                    .collect(),
                ms,
            )
        }
        Workload::U64 => {
            let w = U64MulWitness::from_inputs(corpus.inputs())
                .and_then(|w| w.with_split_shift(split_shift))
                .unwrap();
            let ms = { drop(started); f2z::observability::duration(&started_recording.intervals().expect("complete operation capture"), "mul_e2e_compare/f2z:started").expect("query completed operation") }.as_secs_f64() * 1e3;
            (
                (0..corpus.len())
                    .map(|i| {
                        [
                            w.x_values()[i],
                            w.y_values()[i],
                            w.z_lo_values()[i],
                            w.z_hi_values()[i],
                        ]
                    })
                    .collect(),
                ms,
            )
        }
        Workload::U128 => unreachable!(),
    };
    super::WitnessAudit::check(corpus, rows, generation_ms, "F2Z integer assignment", false)
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    #[test]
    #[ignore = "requires PERFETTO_TRACE_PROCESSOR; exercises F2Z span metrics"]
    fn f2z_trials_use_perfetto_without_the_legacy_profiler() {
        use super::*;
        let _trace = super::super::common::test_tracing();
        use tracing_subscriber::prelude::*;
        let context = Context::setup(Arc::new(Corpus::new(Workload::U32, 15, 7)));
        let bytes = tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || context.prove_and_verify());
        tracing::subscriber::with_default(tracing_subscriber::registry().with(f2z::observability::layer()), || {
            for _ in 0..6 {
                let timing = context.run();
                timing.validate();
                assert_eq!(timing.proof_bytes, bytes);
                assert!(timing.metrics().piop_ms > 0.0);
            }
        });
    }
    use super::*;

    #[test]
    fn u32_comparison_requires_johnson_and_ood() {
        let corpus = Corpus::from_inputs(Workload::U32, vec![(u64::from(u32::MAX), 2); 1 << 15]);
        let context = Context::setup(Arc::new(corpus));
        let Relation::U32(relation) = &context.relation else {
            unreachable!()
        };
        assert_eq!(
            relation.ligerito(),
            Some(U32MulLigerito::CustomJohnson {
                log_inv_rate: 1,
                initial_k: 4
            })
        );
        assert!(relation.security().ood.is_some());
        assert!(relation.security().accounting.controllable_bits() >= 100.0);
        let config = context.config();
        assert_eq!(config["security_scope"], "round-by-round-economic");
        assert_eq!(config["ligerito_regime"], "johnson");
        assert_eq!(config["target_bits"], 100);
        assert_eq!(config["ood_present"], true);
        let identity = &config["ligerito"];
        f2z::ligerito_flock::ResolvedLigerito::validate_report(identity).unwrap();
        assert_eq!(identity["target_bits"], 100);
        assert_eq!(identity["configuration"]["target_security_bits"], 100);
        assert_eq!(identity["configuration"]["levels"][0]["regime"], "johnson_ood");
        assert_eq!(identity["configuration"]["levels"][0]["log_inv_rate"], 1);
        assert!(!config["ood_grinding_bits"].is_null());
    }
    #[test]
    fn johnson_real_proof_rejects_changed_commitment_low_result_and_carry() {
        let inputs: Vec<_> = (0..1 << 15)
            .map(|i| match i % 4 {
                0 => (0, 0),
                1 => (u32::MAX, u32::MAX),
                2 => (65_536, 65_536),
                _ => (i as u32, (i as u32).wrapping_mul(0x9e37_79b9)),
            })
            .collect();
        let corpus = Corpus::from_inputs(
            Workload::U32,
            inputs
                .iter()
                .map(|&(a, b)| (u64::from(a), u64::from(b)))
                .collect(),
        );
        let context = Context::setup(Arc::new(corpus));
        let Relation::U32(relation) = &context.relation else {
            unreachable!()
        };
        let witness = U32MulWitness::from_inputs(&inputs).unwrap();
        let hint = commit_u32_mul_witness(relation, witness.f2z_bit_rows()).unwrap();
        let proof = prove_u32_mul(&mut Blake3Transcript::new(), relation, &witness, &hint).unwrap();
        verify_u32_mul(
            &mut Blake3Transcript::new(),
            relation,
            &hint.commitment,
            &proof,
        )
        .unwrap();
        let mut commitment = hint.commitment.clone();
        commitment.root[0] ^= 1;
        assert!(
            verify_u32_mul(&mut Blake3Transcript::new(), relation, &commitment, &proof).is_err()
        );
        for change_carry in [false, true] {
            let mut rows: Vec<_> = witness.mod32_rows().collect();
            if change_carry {
                rows[1].w ^= 1;
            } else {
                rows[1].z ^= 1;
            }
            let bad = U32MulWitness::from_mod32_rows(&rows).unwrap();
            let bad_hint = commit_u32_mul_witness(relation, bad.f2z_bit_rows()).unwrap();
            if let Ok(bad_proof) =
                prove_u32_mul(&mut Blake3Transcript::new(), relation, &bad, &bad_hint)
            {
                assert!(
                    verify_u32_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &bad_hint.commitment,
                        &bad_proof
                    )
                    .is_err()
                );
            }
        }
    }
}
