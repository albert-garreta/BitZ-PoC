use super::common::plonky3 as stacks;
use super::{Corpus, Timing, TraceCapture, Workload, captured};
use p3_air::{Air, AirBuilder, BaseAir, WindowAccess};
use p3_field::{PrimeCharacteristicRing, PrimeField64, extension::BinomialExtensionField};
use p3_matrix::dense::RowMajorMatrix;
use p3_multi_stark::{
    MultiStarkProof, ProverInstance, ProverInstances, VerifierInstance, VerifierInstances,
    config::MultiStarkConfig, prove, setup, verify,
};
use p3_sumcheck::layout::{Layout, SuffixProver, Table, Witness};
use p3_whir::{DomainSeparator, FoldingFactor, ProtocolParameters, SecurityAssumption};
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
    ($module:ident,$stack:ident) => {
        mod $module {
            use super::stacks::$stack as stack;
            use super::*;
            type F = stack::Val;
            type EF = BinomialExtensionField<F, 5>;
            type WhirLayout = SuffixProver<F, EF>;
            struct Config {
                pcs: stack::Pcs<EF>,
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
                    4
                }
                fn build_witness(&self, tables: Vec<Table<F>>) -> Witness<F> {
                    WhirLayout::new_witness(tables, 4)
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
            }
            fn challenger(config: &Config) -> stack::Challenger {
                let mut c = stack::challenger();
                let mut domain = DomainSeparator::new(Vec::new());
                config.pcs.add_domain_separator::<8>(&mut domain);
                domain.observe_domain_separator(&mut c);
                c
            }
            impl Context {
                pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
                    let air = MulAir {
                        u32_inputs: corpus.workload == Workload::U32,
                    };
                    let width = <MulAir as BaseAir<F>>::width(&air);
                    let num_vars = corpus.len().ilog2() as usize
                        + width.next_power_of_two().ilog2() as usize;
                    let pcs = stack::pcs::<EF>(
                        num_vars,
                        ProtocolParameters {
                            security_level: 100,
                            pow_bits: 12,
                            round_log_inv_rates: vec![],
                            folding_factor: FoldingFactor::Constant(4),
                            soundness_type: SecurityAssumption::JohnsonBound,
                            starting_log_inv_rate: 1,
                        },
                    )
                    .expect("WHIR parameters");
                    let config = Config { pcs };
                    let (pk, vk) = setup(&config, &[&air], &mut challenger(&config));
                    Self {
                        corpus,
                        air,
                        config,
                        pk,
                        vk,
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
backend!(goldilocks, goldilocks);
backend!(babybear, baby_bear);

pub(super) enum Context {
    U32(goldilocks::Context),
    BabyBear(babybear::Context),
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        match corpus.workload {
            Workload::U32 => Self::U32(goldilocks::Context::setup(corpus)),
            Workload::U64 | Workload::U128 => {
                panic!("the Plonky3 adapter has no {} workload", corpus.workload.slug())
            }
            Workload::BabyBear => Self::BabyBear(babybear::Context::setup(corpus)),
        }
    }
    pub(super) fn run(&self, c: &TraceCapture) -> Timing {
        match self {
            Self::U32(x) => x.run(c),
            Self::BabyBear(x) => x.run(c),
        }
    }
    pub(super) fn config(&self) -> Value {
        json!({"piop":"Plonky3 multi-stark AIR zerocheck/sumcheck","pcs":"WHIR","base_field":match self {Self::U32(_)=>"Goldilocks",Self::BabyBear(_)=>"BabyBear"},"extension_degree":5,"target_bits":100,"max_pow_bits":12,"folding":4,"log_inv_rate":1,"soundness":"JohnsonBound"})
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
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
            panic!("the Plonky3 adapter has no {} workload", corpus.workload.slug())
        }
    }
}
