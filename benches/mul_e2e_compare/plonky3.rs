use super::common::plonky3 as stacks;
use super::common::whir_tuning::{self, Params};
use super::{Corpus, Timing, TraceCapture, Workload, captured};
use p3_air::{Air, AirBuilder, BaseAir, WindowAccess};
use p3_field::{PrimeCharacteristicRing, PrimeField64, extension::BinomialExtensionField};
use p3_matrix::dense::RowMajorMatrix;
use p3_multi_stark::{
    MultiStarkProof, ProverInstance, ProverInstances, VerifierInstance, VerifierInstances,
    config::MultiStarkConfig, prove, setup, verify,
};
use p3_sumcheck::layout::{Layout, SuffixProver, Table, Witness};
use p3_whir::DomainSeparator;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
struct MulAir {
    u32_inputs: bool,
}
impl<F> BaseAir<F> for MulAir {
    fn width(&self) -> usize {
        if self.u32_inputs { 67 } else { 3 }
    }
    fn max_constraint_degree(&self) -> Option<usize> {
        Some(2)
    }
    fn main_next_row_columns(&self) -> Vec<usize> {
        vec![]
    }
}
impl<AB: AirBuilder> Air<AB> for MulAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.current_slice();
        builder.assert_eq(row[2], row[0].into() * row[1].into());
        if self.u32_inputs {
            for operand in 0..2 {
                let mut value = AB::Expr::ZERO;
                for bit in 0..32 {
                    let v = row[3 + operand * 32 + bit];
                    builder.assert_bool(v);
                    value += v.into() * AB::Expr::from_u64(1u64 << bit);
                }
                builder.assert_eq(row[operand], value);
            }
        }
    }
}
fn generate<F: PrimeField64>(corpus: &Corpus) -> RowMajorMatrix<F> {
    let width = if corpus.workload == Workload::U32 {
        67
    } else {
        3
    };
    assert!(
        !matches!(corpus.workload, Workload::U64 | Workload::U128),
        "the Plonky3 adapter has no {} workload (its AIR decomposes 32-bit operands)",
        corpus.workload.slug()
    );
    let mut values = F::zero_vec(width * corpus.len());
    for (row, &(a, b)) in values.chunks_exact_mut(width).zip(corpus.inputs()) {
        row[0] = F::from_u64(a);
        row[1] = F::from_u64(b);
        row[2] = row[0] * row[1];
        assert_eq!(
            u128::from(row[2].as_canonical_u64()),
            corpus.workload.output(a, b)
        );
        if width == 67 {
            for (operand, value) in [a, b].into_iter().enumerate() {
                for bit in 0..32 {
                    row[3 + operand * 32 + bit] = F::from_u64((value >> bit) & 1);
                }
            }
        }
    }
    RowMajorMatrix::new(values, width)
}

macro_rules! backend {
    ($module:ident,$stack:ident,$degree:literal) => {
        mod $module {
            use super::stacks::$stack as stack;
            use super::*;
            type F = stack::Val;
            type EF = BinomialExtensionField<F, $degree>;
            type WhirLayout = SuffixProver<F, EF>;
            struct Config {
                pcs: stack::Pcs<EF>,
                folding: usize,
            }
            impl MultiStarkConfig for Config {
                type Val = F;
                type Challenge = EF;
                type Challenger = stack::Challenger;
                type Pcs = stack::Pcs<EF>;
                fn pcs(&self) -> &Self::Pcs {
                    &self.pcs
                }
                fn min_num_variables(&self) -> usize {
                    self.folding
                }
                fn build_witness(&self, tables: Vec<Table<F>>) -> Witness<F> {
                    WhirLayout::new_witness(tables, self.folding)
                }
                fn committed_table<'a>(
                    &self,
                    data: &'a p3_whir::WhirProverData<F, EF, stack::Mmcs, WhirLayout>,
                    index: usize,
                ) -> &'a Table<F> {
                    data.table(index)
                }
            }
            pub struct Context {
                corpus: Arc<Corpus>,
                air: MulAir,
                config: Config,
                pk: p3_multi_stark::ProvingKey<Config>,
                vk: p3_multi_stark::VerifyingKey<Config>,
                pub(super) security: Value,
                pub(super) params: Params,
            }
            fn challenger(config: &Config) -> stack::Challenger {
                let mut c = stack::challenger();
                let mut domain = DomainSeparator::new(Vec::new());
                config.pcs.add_domain_separator::<8>(&mut domain);
                domain.observe_domain_separator(&mut c);
                c
            }
            impl Context {
                pub(super) fn setup(corpus: Arc<Corpus>, params: Params) -> Result<Self, String> {
                    let air = MulAir {
                        u32_inputs: corpus.workload == Workload::U32,
                    };
                    let width = <MulAir as BaseAir<F>>::width(&air);
                    let num_vars =
                        corpus.len().ilog2() as usize + width.next_power_of_two().ilog2() as usize;
                    let (protocol, security) =
                        whir_tuning::select_protocol::<F, EF, stack::Challenger>(
                            num_vars,
                            whir_tuning::air_shape::<F, EF, _>(&air, corpus.len().ilog2() as usize),
                            params,
                        )?;
                    let pcs = stack::pcs::<EF>(num_vars, protocol).map_err(|e| e.to_string())?;
                    let config = Config {
                        pcs,
                        folding: params.folding,
                    };
                    let (pk, vk) = setup(&config, &[&air], &mut challenger(&config));
                    Ok(Self {
                        corpus,
                        air,
                        config,
                        pk,
                        vk,
                        security,
                        params,
                    })
                }
                #[cfg(test)]
                pub(super) fn rejection_self_test(&self) {
                    let make_proof = |trace: RowMajorMatrix<F>| -> MultiStarkProof<Config> {
                        prove(
                            &self.config,
                            ProverInstances::new(vec![ProverInstance::new(
                                &self.air,
                                Table::new(trace.transpose()),
                                &self.pk,
                                &[],
                            )]),
                            0,
                            &mut challenger(&self.config),
                        )
                    };
                    let accepts = |proof: &MultiStarkProof<Config>| {
                        verify(
                            &self.config,
                            VerifierInstances::new(vec![VerifierInstance::new(
                                &self.air,
                                &self.vk,
                                self.corpus.len().ilog2() as usize,
                                &[],
                            )]),
                            proof,
                            0,
                            &mut challenger(&self.config),
                        )
                        .is_ok()
                    };
                    let proof = make_proof(generate::<F>(&self.corpus));
                    assert!(accepts(&proof));
                    let encoded = postcard::to_allocvec(&proof).unwrap();
                    let mut changed: MultiStarkProof<Config> =
                        postcard::from_bytes(&encoded).unwrap();
                    changed.opening.whir.initial_ood_answers[0] += EF::ONE;
                    assert!(!accepts(&changed), "PCS must reject an altered OOD answer");
                    let mut changed: MultiStarkProof<Config> =
                        postcard::from_bytes(&encoded).unwrap();
                    changed.sumcheck.claimed_sum = EF::ONE;
                    assert!(
                        !accepts(&changed),
                        "PIOP must reject a nonzero zerocheck claim"
                    );
                    // Release mode bypasses debug constraint checking: the actual
                    // verifier must reject proofs constructed for invalid witnesses.
                    #[cfg(not(debug_assertions))]
                    {
                        let mut wrong = generate::<F>(&self.corpus);
                        wrong.values[2] += F::ONE;
                        assert!(!accepts(&make_proof(wrong)), "wrong multiplication output");
                        if self.air.u32_inputs {
                            let mut wrong = generate::<F>(&self.corpus);
                            wrong.values[0] = F::from_u64(1 << 32);
                            wrong.values[2] = wrong.values[0] * wrong.values[1];
                            assert!(!accepts(&make_proof(wrong)), "operand outside u32 range");
                        }
                    }
                }

                pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
                    capture.begin();
                    let start = capture.now_ns();
                    let trace = generate::<F>(&self.corpus);
                    let table = Table::new(trace.transpose());
                    let wend = capture.now_ns();
                    let proof: MultiStarkProof<Config> = prove(
                        &self.config,
                        ProverInstances::new(vec![ProverInstance::new(
                            &self.air,
                            table,
                            &self.pk,
                            &[],
                        )]),
                        0,
                        &mut challenger(&self.config),
                    );
                    let ready = capture.now_ns();
                    let bytes = postcard::to_allocvec(&proof).expect("encode Plonky3 proof");
                    let vstart = capture.now_ns();
                    verify(
                        &self.config,
                        VerifierInstances::new(vec![VerifierInstance::new(
                            &self.air,
                            &self.vk,
                            self.corpus.len().ilog2() as usize,
                            &[],
                        )]),
                        &proof,
                        0,
                        &mut challenger(&self.config),
                    )
                    .expect("Plonky3 full proof verifies");
                    let end = capture.now_ns();
                    let raw = capture.finish();
                    let encode = captured(&raw, "encode", wend, ready);
                    let commit = captured(&raw, "commit_matrix", encode.end_ns, ready);
                    let opening = raw
                        .iter()
                        .filter(|s| {
                            matches!(s.name.as_str(), "add_virtual_eval" | "eval_at")
                                && s.start_ns >= commit.end_ns
                                && s.end_ns <= ready
                        })
                        .min_by_key(|s| s.start_ns)
                        .expect("WHIR opening boundary");
                    let mut t = Timing::new(start, wend, ready, vstart, end, bytes.len());
                    t.add("commit", "commit", encode.start_ns, commit.end_ns);
                    t.add("piop", "constraint-proof", commit.end_ns, opening.start_ns);
                    t.add("opening", "opening-proof", opening.start_ns, ready);
                    t
                }
            }
        }
    };
}
backend!(goldilocks2, goldilocks, 2);
backend!(goldilocks5, goldilocks, 5);
backend!(babybear4, baby_bear, 4);
backend!(babybear5, baby_bear, 5);

pub(super) enum Context {
    U32Degree2(goldilocks2::Context),
    U32Degree5(goldilocks5::Context),
    BabyBearDegree4(babybear4::Context),
    BabyBearDegree5(babybear5::Context),
}
impl Context {
    #[cfg(test)]
    fn rejection_self_test(&self) {
        match self {
            Self::U32Degree2(x) => x.rejection_self_test(),
            Self::U32Degree5(x) => x.rejection_self_test(),
            Self::BabyBearDegree4(x) => x.rejection_self_test(),
            Self::BabyBearDegree5(x) => x.rejection_self_test(),
        }
    }
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let params = whir_tuning::replay()
            .expect("read WHIR replay configuration")
            .unwrap_or_default();
        Self::setup_with_params(corpus, params).expect("eligible native WHIR parameters")
    }
    pub(super) fn setup_with_params(corpus: Arc<Corpus>, params: Params) -> Result<Self, String> {
        match (corpus.workload, params.extension_degree) {
            (Workload::U32, 2) => goldilocks2::Context::setup(corpus, params).map(Self::U32Degree2),
            (Workload::U32, 5) => goldilocks5::Context::setup(corpus, params).map(Self::U32Degree5),
            (Workload::BabyBear, 4) => {
                babybear4::Context::setup(corpus, params).map(Self::BabyBearDegree4)
            }
            (Workload::BabyBear, 5) => {
                babybear5::Context::setup(corpus, params).map(Self::BabyBearDegree5)
            }
            _ => Err(format!(
                "unsupported WHIR {} degree {}",
                corpus.workload.slug(),
                params.extension_degree
            )),
        }
    }
    pub(super) fn run(&self, c: &TraceCapture) -> Timing {
        match self {
            Self::U32Degree2(x) => x.run(c),
            Self::U32Degree5(x) => x.run(c),
            Self::BabyBearDegree4(x) => x.run(c),
            Self::BabyBearDegree5(x) => x.run(c),
        }
    }
    pub(super) fn security(&self) -> Value {
        match self {
            Self::U32Degree2(x) => x.security.clone(),
            Self::U32Degree5(x) => x.security.clone(),
            Self::BabyBearDegree4(x) => x.security.clone(),
            Self::BabyBearDegree5(x) => x.security.clone(),
        }
    }
    pub(super) fn config(&self) -> Value {
        let (base_field, params) = match self {
            Self::U32Degree2(x) => ("Goldilocks", x.params),
            Self::U32Degree5(x) => ("Goldilocks", x.params),
            Self::BabyBearDegree4(x) => ("BabyBear", x.params),
            Self::BabyBearDegree5(x) => ("BabyBear", x.params),
        };
        json!({"piop":"Plonky3 multi-stark AIR zerocheck/sumcheck","pcs":"WHIR",
            "encoding":"Reed-Solomon", "opening_claim":"prescribed multilinear evaluation",
            "base_field":base_field,"params":params,"security":self.security()})
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn full_proofs_verify_boundaries_and_reject_invalid_claims() {
        for workload in [Workload::U32, Workload::BabyBear] {
            let corpus = Arc::new(super::super::edge_corpus(workload));
            let degrees = if workload == Workload::U32 {
                [2, 5]
            } else {
                [4, 5]
            };
            let mut tested = 0;
            for extension_degree in degrees {
                let params = Params {
                    extension_degree,
                    ..Params::default()
                };
                if let Ok(context) = Context::setup_with_params(Arc::clone(&corpus), params) {
                    context.rejection_self_test();
                    tested += 1;
                }
            }
            assert!(
                tested > 0,
                "an eligible configuration must exercise each workload"
            );
        }
    }
    #[test]
    fn air_rejects_wrong_outputs_and_out_of_range_u32() {
        let corpus = Corpus::new(Workload::U32, 4, 7);
        let air = MulAir { u32_inputs: true };
        let trace = generate::<p3_goldilocks::Goldilocks>(&corpus);
        p3_air::check_constraints(&air, &trace, &[]);
        let mut wrong = trace.clone();
        wrong.values[2] += p3_goldilocks::Goldilocks::ONE;
        assert!(std::panic::catch_unwind(|| p3_air::check_constraints(&air, &wrong, &[])).is_err());
        let mut wrong = trace;
        wrong.values[0] = p3_goldilocks::Goldilocks::from_u64(1 << 32);
        wrong.values[2] = wrong.values[0] * wrong.values[1];
        assert!(std::panic::catch_unwind(|| p3_air::check_constraints(&air, &wrong, &[])).is_err());
        let corpus = Corpus::new(Workload::BabyBear, 4, 7);
        let air = MulAir { u32_inputs: false };
        let mut trace = generate::<p3_baby_bear::BabyBear>(&corpus);
        p3_air::check_constraints(&air, &trace, &[]);
        trace.values[2] += p3_baby_bear::BabyBear::ONE;
        assert!(std::panic::catch_unwind(|| p3_air::check_constraints(&air, &trace, &[])).is_err());
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    fn recover<F: PrimeField64>(corpus: &Corpus) -> super::WitnessAudit {
        let started = std::time::Instant::now();
        let trace = generate::<F>(corpus);
        let generation_ms = started.elapsed().as_secs_f64() * 1e3;
        let rows = trace
            .values
            .chunks_exact(trace.width)
            .map(|row| {
                let a = row[0].as_canonical_u64();
                let b = row[1].as_canonical_u64();
                let c = row[2].as_canonical_u64();
                let q = if corpus.workload == Workload::BabyBear {
                    (a * b - c) / super::BABY_P
                } else {
                    0
                };
                [a, b, c, q]
            })
            .collect();
        super::WitnessAudit::check(
            corpus,
            rows,
            generation_ms,
            "Plonky3 AIR field columns",
            corpus.workload == Workload::BabyBear,
        )
    }
    match corpus.workload {
        Workload::U32 => recover::<p3_goldilocks::Goldilocks>(corpus),
        Workload::BabyBear => recover::<p3_baby_bear::BabyBear>(corpus),
        Workload::U64 | Workload::U128 => {
            panic!(
                "the Plonky3 adapter has no {} workload",
                corpus.workload.slug()
            )
        }
    }
}
