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
    utils::prof,
};
use serde_json::{Value, json};
use std::sync::Arc;

enum Relation {
    U32(PreparedU32MulRelation),
    U64(PreparedU64MulRelation),
    U128(PreparedU128MulRelation),
}
pub(super) struct Context {
    corpus: Arc<Corpus>,
    relation: Relation,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let n = corpus.len();
        let relation = match corpus.workload {
            Workload::U32 => {
                let relation = PreparedU32MulRelation::new_with_profile_and_ligerito::<Lambda100>(
                    U32MulLayout::new(n).unwrap(),
                    super::common::ligerito_selection(100),
                )
                .expect("u32 Johnson relation");
                Relation::U32(relation)
            }
            Workload::U64 => {
                Relation::U64(PreparedU64MulRelation::new_with_profile_and_ligerito::<Lambda100>(U64MulLayout::new(n).unwrap(), super::common::ligerito_selection(100)).unwrap())
            }
            Workload::U128 => Relation::U128(
                PreparedU128MulRelation::new_with_profile_and_ligerito::<Lambda100>(U128MulLayout::new(n).unwrap(), super::common::ligerito_selection(100)).unwrap(),
            ),
        };
        Self { corpus, relation }
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
            config["ligerito_regime"] = json!(if security.ood.is_some() { "johnson" } else { "udr" });
            config["ood_present"] = json!(security.ood.is_some());
            config["modeled_min_bits"] = json!(security.accounting.achieved_bits());
            config["security_terms"] = json!(terms);
            config["projection_prime_min"] = json!(security.projection_min.to_string());
            config["projection_prime_max"] = json!(security.projection_max.to_string());
            config["ood_grinding_bits"] =
                json!(security.ood.map(|p| p.grinding_bits));
            config["geometry"] = json!({"t":params.t, "s":params.s, "word_bits":params.word_bits});
            config["ligerito"] = super::common::ligerito_report(ligerito, security.ood);
        }
        match &self.relation {
            Relation::U64(p) => config["ligerito"] = super::common::ligerito_report(p.ligerito_configuration(), p.security().ood),
            Relation::U128(p) => config["ligerito"] = super::common::ligerito_report(p.ligerito_configuration(), p.security().ood),
            _ => {}
        }
        config
    }
    pub(super) fn run(&self) -> Timing {
        let _ = prof::take_intervals();
        let _ = prof::take_totals();
        let root = prof::scope("native-mul:root");
        let total = prof::scope("native-mul:witness_to_proof");
        // Serialized proof size: the commitment root, the Spartan payload as
        // 16-byte field elements, the transmitted nonces outside the opening
        // as 8-byte words, and the F2Z opening's exact codec bytes.
        let proof_bytes = match &self.relation {
            Relation::U32(relation) => {
                let witness = {
                    let _s = prof::scope("native-mul:witness");
                    U32MulWitness::from_inputs(&self.corpus.narrow_inputs()).expect("u32 witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(
                        u128::from(witness.product_values()[i] as u32),
                        self.corpus.workload.output(a, b)
                    );
                }
                let online = prof::scope("native-mul:online");
                let hint = {
                    let _s = prof::scope("native-mul:commit");
                    commit_u32_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u32 commitment")
                };
                let proof = prove_u32_mul(&mut Blake3Transcript::new(), relation, &witness, &hint)
                    .expect("u32 full proof");
                drop(online);
                drop(total);
                {
                    let _s = prof::scope("native-mul:verify");
                    verify_u32_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u32 full verification");
                }
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
                    let _s = prof::scope("native-mul:witness");
                    U64MulWitness::from_inputs(self.corpus.inputs()).expect("u64 witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(witness.product(i), self.corpus.workload.output(a, b));
                }
                let online = prof::scope("native-mul:online");
                let hint = {
                    let _s = prof::scope("native-mul:commit");
                    commit_u64_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u64 commitment")
                };
                let proof = prove_u64_mul(&mut Blake3Transcript::new(), relation, &witness, &hint)
                    .expect("u64 full proof");
                drop(online);
                drop(total);
                {
                    let _s = prof::scope("native-mul:verify");
                    verify_u64_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u64 full verification");
                }
                let bytes = hint.commitment.root.len() + proof.size_bytes(relation.security());
                std::hint::black_box(proof);
                bytes
            }
            Relation::U128(relation) => {
                let witness = {
                    let _s = prof::scope("native-mul:witness");
                    U128MulWitness::from_inputs(self.corpus.wide_inputs()).expect("u128 witness")
                };
                for (i, &(x, y)) in self.corpus.wide_inputs().iter().enumerate() {
                    let [_, _, lo, hi] = self.corpus.workload.wide_row(x, y);
                    assert_eq!(witness.product(i), (lo, hi));
                }
                let online = prof::scope("native-mul:online");
                let hint = {
                    let _s = prof::scope("native-mul:commit");
                    commit_u128_mul_witness(relation, witness.f2z_bit_rows())
                        .expect("u128 commitment")
                };
                let proof = prove_u128_mul(&mut Blake3Transcript::new(), relation, &witness, &hint)
                    .expect("u128 full proof");
                drop(online);
                drop(total);
                {
                    let _s = prof::scope("native-mul:verify");
                    verify_u128_mul(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("u128 full verification");
                }
                let bytes = hint.commitment.root.len() + proof.size_bytes(relation.security());
                std::hint::black_box(proof);
                bytes
            }
        };
        drop(root);
        let raw = prof::take_intervals();
        let _ = prof::take_totals();
        let find = |label| {
            raw.iter()
                .find(|s| s.label == label)
                .unwrap_or_else(|| panic!("missing F2Z {label}"))
        };
        let r = find("native-mul:root");
        let w = find("native-mul:witness");
        let t = find("native-mul:witness_to_proof");
        let v = find("native-mul:verify");
        let mut timing = Timing::new(
            r.start_ns,
            w.end_ns,
            t.end_ns,
            v.start_ns,
            v.end_ns,
            proof_bytes,
        );
        // Account for all online steps without calling prime projection a sumcheck.
        for (label, name, tag) in [
            ("native-mul:commit", "commit", "commit"),
            ("step2:project_prove", "prime_projection", "preparation"),
            ("step3:piop_prove", "piop", "constraint-proof"),
            ("step4:bitify_prove", "claim_bridge", "opening-proof"),
            ("step5:open_prove", "opening", "opening-proof"),
        ] {
            let s = find(label);
            timing.add(name, tag, s.start_ns, s.end_ns);
        }
        timing
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let started = std::time::Instant::now();
    if corpus.workload.is_wide() {
        let w = U128MulWitness::from_inputs(corpus.wide_inputs()).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
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
            let ms = started.elapsed().as_secs_f64() * 1e3;
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
            let w = U64MulWitness::from_inputs(corpus.inputs()).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
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
            U32MulLigerito::CustomJohnson {
                log_inv_rate: 3,
                initial_k: 4
            }
        );
        assert!(relation.security().ood.is_some());
        assert!(relation.security().accounting.controllable_bits() >= 100.0);
        let config = context.config();
        assert_eq!(config["security_scope"], "round-by-round-economic");
        assert_eq!(config["ligerito_regime"], "johnson");
        assert_eq!(config["target_bits"], 100);
        assert_eq!(config["ood_present"], true);
        assert_eq!(config["ligerito"]["target_security_bits"], 100);
        assert_eq!(config["ligerito"]["levels"][0]["regime"], "johnson_ood");
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
