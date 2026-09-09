use super::{BABY_P, Corpus, Timing, TraceCapture, Workload, captured};
#[cfg(test)]
#[allow(unused_imports)] // `cargo bench` sets cfg(test) without running the #[test] user.
use super::Operands;
use limber::{
    imod_r1cs_modp::{IntModR1CSShapeModp, IntModR1CSWitnessModp},
    imod_spartan_modp::{
        IntModSpartanModpProverKey, IntModSpartanModpSNARK, IntModSpartanModpVerifierKey,
    },
    provider::{T256DynPrimeEngine, pcs::integer_modpcs::IntEvalParams},
    traits::mod_engine::{ModEngine, SumcheckEngine, SumcheckField},
};
use num_bigint::BigUint;
use serde_json::{Value, json};
use std::sync::Arc;
type E = T256DynPrimeEngine;

#[derive(Clone, Copy)]
enum Column {
    Witness(usize),
    One,
}
type Lc = Vec<(Column, u64)>;
struct Row {
    a: Lc,
    b: Lc,
    c: Lc,
    modulus: u64,
}
enum Assignment {
    Input(usize, usize),
    Output(usize),
    Bit(usize, usize),
    And(usize, usize),
}
struct Program {
    assignments: Vec<Assignment>,
    rows: Vec<Row>,
    num_vars: usize,
    num_cons: usize,
}
impl Program {
    fn compile(corpus: &Corpus) -> Self {
        let mut p = Self {
            assignments: vec![],
            rows: vec![],
            num_vars: 0,
            num_cons: 0,
        };
        for gate in 0..corpus.len() {
            let a = p.alloc(Assignment::Input(gate, 0));
            let b = p.alloc(Assignment::Input(gate, 1));
            let c = p.alloc(Assignment::Output(gate));
            p.rows.push(Row {
                a: term(a),
                b: term(b),
                c: term(c),
                modulus: if corpus.workload == Workload::BabyBear {
                    BABY_P
                } else {
                    0
                },
            });
            match corpus.workload {
                Workload::U32 => {
                    p.range(a, 32, false);
                    p.range(b, 32, false);
                }
                Workload::BabyBear => {
                    p.range(a, 31, true);
                    p.range(b, 31, true);
                    p.range(c, 31, true);
                }
                Workload::U64 | Workload::U128 => panic!(
                    "the Limber adapter has no {} workload (its rows use u64 coefficients)",
                    corpus.workload.slug()
                ),
            }
        }
        p.num_vars = p.assignments.len().next_power_of_two();
        p.num_cons = p.rows.len().next_power_of_two();
        p
    }
    fn alloc(&mut self, value: Assignment) -> usize {
        let i = self.assignments.len();
        self.assignments.push(value);
        i
    }
    fn range(&mut self, column: usize, bits: usize, canonical_babybear: bool) {
        let indices: Vec<_> = (0..bits)
            .map(|bit| {
                let i = self.alloc(Assignment::Bit(column, bit));
                self.rows.push(Row {
                    a: term(i),
                    b: term(i),
                    c: term(i),
                    modulus: 0,
                });
                i
            })
            .collect();
        self.rows.push(Row {
            a: indices
                .iter()
                .enumerate()
                .map(|(bit, &i)| (Column::Witness(i), 1u64 << bit))
                .collect(),
            b: one(),
            c: term(column),
            modulus: 0,
        });
        if canonical_babybear {
            // p-1 = 15 * 2^27. If the four high bits are all one, the low
            // 27 bits must be zero. All other 31-bit values are below p.
            let mut high = indices[27];
            for &bit in &indices[28..31] {
                let next = self.alloc(Assignment::And(high, bit));
                self.rows.push(Row {
                    a: term(high),
                    b: term(bit),
                    c: term(next),
                    modulus: 0,
                });
                high = next;
            }
            self.rows.push(Row {
                a: term(high),
                b: indices[..27]
                    .iter()
                    .enumerate()
                    .map(|(bit, &i)| (Column::Witness(i), 1u64 << bit))
                    .collect(),
                c: vec![],
                modulus: 0,
            });
        }
    }
    fn shape(&self) -> IntModR1CSShapeModp<E> {
        let entries = |select: fn(&Row) -> &Lc| {
            self.rows
                .iter()
                .enumerate()
                .flat_map(|(row, r)| {
                    select(r).iter().map(move |&(col, k)| {
                        (
                            row,
                            match col {
                                Column::Witness(i) => i,
                                Column::One => self.num_vars,
                            },
                            BigUint::from(k),
                        )
                    })
                })
                .collect()
        };
        let mut moduli: Vec<_> = self.rows.iter().map(|r| BigUint::from(r.modulus)).collect();
        moduli.resize(self.num_cons, BigUint::from(0u32));
        IntModR1CSShapeModp::new(
            self.num_cons,
            self.num_vars,
            0,
            entries(|r| &r.a),
            entries(|r| &r.b),
            entries(|r| &r.c),
            moduli,
        )
        .expect("Limber multiplication shape")
    }
    fn values(&self, corpus: &Corpus) -> Vec<u64> {
        let mut values = Vec::with_capacity(self.num_vars);
        for a in &self.assignments {
            let value = match *a {
                Assignment::Input(i, operand) => {
                    let (a, b) = corpus.inputs()[i];
                    if operand == 0 { a } else { b }
                }
                Assignment::Output(i) => {
                    let (a, b) = corpus.inputs()[i];
                    u64::try_from(corpus.workload.output(a, b)).expect("Limber outputs fit u64")
                }
                Assignment::Bit(i, bit) => (values[i] >> bit) & 1,
                Assignment::And(a, b) => values[a] * values[b],
            };
            values.push(value);
        }
        values.resize(self.num_vars, 0);
        values
    }
    fn quotients(&self, values: &[u64]) -> Result<Vec<BigUint>, String> {
        let eval = |lc: &Lc| {
            lc.iter()
                .map(|&(col, k)| {
                    k as u128
                        * match col {
                            Column::One => 1,
                            Column::Witness(i) => values[i] as u128,
                        }
                })
                .sum::<u128>()
        };
        let mut q = Vec::with_capacity(self.num_cons);
        for r in &self.rows {
            let residual = (eval(&r.a) * eval(&r.b))
                .checked_sub(eval(&r.c))
                .ok_or("negative residual")?;
            if r.modulus == 0 {
                if residual != 0 {
                    return Err("invalid exact row".into());
                }
                q.push(BigUint::from(0u32));
            } else {
                if residual % r.modulus as u128 != 0 {
                    return Err("invalid modular row".into());
                }
                q.push(BigUint::from(residual / r.modulus as u128));
            }
        }
        q.resize(self.num_cons, BigUint::from(0u32));
        Ok(q)
    }
}
fn term(i: usize) -> Lc {
    vec![(Column::Witness(i), 1)]
}
fn one() -> Lc {
    vec![(Column::One, 1)]
}

pub(super) struct Context {
    corpus: Arc<Corpus>,
    program: Program,
    shape: IntModR1CSShapeModp<E>,
    pk: IntModSpartanModpProverKey<E>,
    vk: IntModSpartanModpVerifierKey<E>,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let program = Program::compile(&corpus);
        let shape = program.shape();
        let arity = program.num_cons.max(program.num_vars).ilog2() as usize;
        let params = IntEvalParams::derive(64, 32, 9, arity).expect("Limber IntEval parameters");
        let (pk, vk) = IntModSpartanModpSNARK::<E>::setup_with_params(shape.clone(), params)
            .expect("Limber setup");
        Self {
            corpus,
            program,
            shape,
            pk,
            vk,
        }
    }
    pub(super) fn config(&self) -> Value {
        json!({"piop":"Integer-Mod Spartan","pcs":"Limber IntEval/Hyrax","log_t_f":64,"log_t":32,"k":9,"constraints":self.program.rows.len(),"padded_constraints":self.program.num_cons,"variables":self.program.assignments.len(),"padded_variables":self.program.num_vars,"security_policy":"native Limber parameters; not asserted equal to F2Z or WHIR"})
    }
    pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
        capture.begin();
        let start = capture.now_ns();
        let values = self.program.values(&self.corpus);
        let q = self
            .program
            .quotients(&values)
            .expect("Limber witness satisfies exact and modular constraints");
        let w = values.into_iter().map(BigUint::from).collect();
        let wend = capture.now_ns();
        let (witness, instance) =
            IntModR1CSWitnessModp::<E>::new(&self.shape, self.pk.ck(), w, q, vec![])
                .expect("Limber witness commitment");
        let proof = IntModSpartanModpSNARK::<E>::prove(&self.pk, &instance, &witness)
            .expect("Limber full proof");
        let ready = capture.now_ns();
        let vstart = capture.now_ns();
        proof
            .verify(&self.vk, &instance)
            .expect("Limber full verification");
        let end = capture.now_ns();
        let raw = capture.finish();
        // The pinned Limber driver has no whole-proof serializer. Its
        // unsegmented PIOP sends three coefficients per outer round, two per
        // inner round, five outer evaluations and eval_w. Round counts are
        // fixed by the public padded shape; no length prefixes or sampled
        // modulus need be sent. Count these payload bytes separately from the
        // actually serialized commitments and batch opening, including both W/Q.
        let scalar_bytes = <E as SumcheckEngine>::Scalar::zero(&E::bootstrap_params())
            .to_le_bytes()
            .len();
        let piop_bytes = scalar_bytes
            * (3 * self.program.num_cons.ilog2() as usize
                + 2 * (self.program.num_vars.ilog2() as usize + 1)
                + 6);
        let proof_bytes = instance
            .commitment_bytes()
            .expect("serialize Limber commitments")
            .len()
            + piop_bytes
            + proof
                .eval_arg_bytes()
                .expect("serialize Limber opening")
                .len();
        let commit = captured(&raw, "imod_modp_wq_commit", wend, ready);
        let piop = captured(&raw, "imod_modp_piop", commit.end_ns, ready);
        let opening = captured(&raw, "imod_modp_wq_open", commit.end_ns, ready);
        let mut t = Timing::new(start, wend, ready, vstart, end, proof_bytes);
        t.add("commit", "commit", commit.start_ns, commit.end_ns);
        t.add(
            "prime_projection",
            "preparation",
            commit.end_ns,
            piop.start_ns,
        );
        t.add("piop", "constraint-proof", piop.start_ns, piop.end_ns);
        t.add("opening", "opening-proof", opening.start_ns, opening.end_ns);
        std::hint::black_box((proof, witness, instance));
        t
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn integer_rows_reject_bad_outputs_and_ranges() {
        for workload in [Workload::U32, Workload::BabyBear] {
            let corpus = Corpus::new(workload, 4, 7);
            let p = Program::compile(&corpus);
            let mut values = p.values(&corpus);
            p.quotients(&values).unwrap();
            values[2] ^= 1;
            assert!(p.quotients(&values).is_err());
            if workload == Workload::BabyBear {
                // An out-of-range operand has no canonical witness, so the
                // corpus is assembled directly (its digest is unused here).
                let mut inputs = corpus.inputs().to_vec();
                inputs[0] = (BABY_P, 0);
                let corpus = Corpus {
                    workload,
                    operands: Operands::Narrow(inputs),
                    digest: corpus.digest.clone(),
                };
                assert!(p.quotients(&p.values(&corpus)).is_err());
            } else {
                let mut values = p.values(&corpus);
                values[0] = 1 << 32;
                values[1] = 0;
                values[2] = 0;
                assert!(p.quotients(&values).is_err());
            }
        }
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let program = Program::compile(corpus);
    let started = std::time::Instant::now();
    let values = program.values(corpus);
    let q = program.quotients(&values).expect("integer constraints");
    let generation_ms = started.elapsed().as_secs_f64() * 1e3;
    let mut rows = vec![[0u64; 4]; corpus.len()];
    for (index, assignment) in program.assignments.iter().enumerate() {
        match *assignment {
            Assignment::Input(gate, operand) => rows[gate][operand] = values[index],
            Assignment::Output(gate) => rows[gate][2] = values[index],
            _ => {}
        }
    }
    // Modular multiplication is the only nonzero-modulus row kind, in gate order.
    for (gate, (_, q)) in program
        .rows
        .iter()
        .zip(q)
        .filter(|(row, _)| row.modulus != 0)
        .enumerate()
    {
        rows[gate][3] = u64::try_from(q).expect("native quotient fits canonical u64");
    }
    super::WitnessAudit::check(
        corpus,
        rows,
        generation_ms,
        "Limber integer assignment and quotient",
        false,
    )
}
