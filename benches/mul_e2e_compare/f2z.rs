use super::{Corpus, Timing, Workload};
use f2z::{
    piop::spartan::{
        BabyBearMulLayout, BabyBearMulWitness, PreparedBabyBearMulRelation, PreparedU32MulRelation,
        PreparedU64MulRelation, PreparedU128MulRelation, SpartanReductionStrategy, U32MulLayout,
        U32MulWitness, U64MulLayout, U64MulWitness, U128MulLayout, U128MulWitness,
        commit_baby_bear_mul_paper_witness, commit_u32_mul_witness, commit_u64_mul_witness,
        commit_u128_mul_witness, prove_baby_bear_mul_paper, prove_u32_mul, prove_u64_mul,
        prove_u128_mul, verify_baby_bear_mul_paper, verify_u32_mul, verify_u64_mul,
        verify_u128_mul,
    },
    transcript::Blake3Transcript,
    utils::prof,
};
use serde_json::{Value, json};
use std::sync::Arc;

enum Relation {
    U32(PreparedU32MulRelation),
    BabyBear(PreparedBabyBearMulRelation),
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
                Relation::U32(PreparedU32MulRelation::new(U32MulLayout::new(n).unwrap()).unwrap())
            }
            Workload::BabyBear => Relation::BabyBear(
                PreparedBabyBearMulRelation::new(BabyBearMulLayout::new(n).unwrap()).unwrap(),
            ),
            Workload::U64 => {
                Relation::U64(PreparedU64MulRelation::new(U64MulLayout::new(n).unwrap()).unwrap())
            }
            Workload::U128 => Relation::U128(
                PreparedU128MulRelation::new(U128MulLayout::new(n).unwrap()).unwrap(),
            ),
        };
        Self { corpus, relation }
    }
    pub(super) fn config(&self) -> Value {
        json!({"profile":"Lambda100","target_bits":100,"piop":"Spartan over transcript-sampled prime","pcs":"F2Z/Ligerito","word_bits":1})
    }
    pub(super) fn run(&self) -> Timing {
        let _ = prof::take_intervals();
        let _ = prof::take_totals();
        let root = prof::scope("native-mul:root");
        let total = prof::scope("native-mul:witness_to_proof");
        // Serialized proof size: Spartan payload as 16-byte field elements,
        // the transmitted nonces as 8-byte words, and the F2Z opening's
        // exact codec bytes (the `f2z` CLI's accounting).
        let proof_bytes: usize;
        match &self.relation {
            Relation::U32(relation) => {
                let witness = {
                    let _s = prof::scope("native-mul:witness");
                    U32MulWitness::from_inputs(&self.corpus.narrow_inputs()).expect("u32 witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(
                        u128::from(witness.product_values()[i]),
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
                let security = relation.security();
                proof_bytes = proof.spartan_payload_elements() * 16
                    + (proof.grinding_nonce_count(security) - proof.f2z().grinding_nonces.len())
                        * 8
                    + proof.f2z().to_bytes().len();
                std::hint::black_box(proof);
            }
            Relation::BabyBear(relation) => {
                let witness = {
                    let _s = prof::scope("native-mul:witness");
                    BabyBearMulWitness::from_inputs(&self.corpus.narrow_inputs())
                        .expect("BabyBear witness")
                };
                for (i, &(a, b)) in self.corpus.inputs().iter().enumerate() {
                    assert_eq!(
                        u128::from(witness.c_values()[i]),
                        self.corpus.workload.output(a, b)
                    );
                }
                let online = prof::scope("native-mul:online");
                let hint = {
                    let _s = prof::scope("native-mul:commit");
                    commit_baby_bear_mul_paper_witness(relation, witness.f2z_bit_rows())
                        .expect("BabyBear commitment")
                };
                let proof = prove_baby_bear_mul_paper(
                    &mut Blake3Transcript::new(),
                    relation,
                    &witness,
                    &hint,
                    SpartanReductionStrategy::DelayedBarrett,
                )
                .expect("BabyBear full proof");
                drop(online);
                drop(total);
                {
                    let _s = prof::scope("native-mul:verify");
                    verify_baby_bear_mul_paper(
                        &mut Blake3Transcript::new(),
                        relation,
                        &hint.commitment,
                        &proof,
                    )
                    .expect("BabyBear full verification");
                }
                let security = relation.security();
                proof_bytes = proof.spartan_payload_elements() * 16
                    + (proof.grinding_nonce_count(security) - proof.f2z().grinding_nonces.len())
                        * 8
                    + proof.f2z().to_bytes().len();
                std::hint::black_box(proof);
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
                proof_bytes = proof.size_bytes(relation.security());
                std::hint::black_box(proof);
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
                proof_bytes = proof.size_bytes(relation.security());
                std::hint::black_box(proof);
            }
        }
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
        let mut timing = Timing::new(r.start_ns, w.end_ns, t.end_ns, v.start_ns, v.end_ns);
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
        timing.proof_bytes = Some(proof_bytes);
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
                    .map(|i| [w.x_values()[i], w.y_values()[i], w.product_values()[i], 0])
                    .collect(),
                ms,
            )
        }
        Workload::BabyBear => {
            let w = BabyBearMulWitness::from_inputs(&corpus.narrow_inputs()).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
            (
                (0..corpus.len())
                    .map(|i| {
                        [
                            w.a_values()[i],
                            w.b_values()[i],
                            w.c_values()[i],
                            w.k_values()[i],
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
