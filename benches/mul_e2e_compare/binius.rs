use super::{BABY_P, Corpus, Timing, TraceCapture, Workload, captured};
use binius_core::{constraint_system::ValueVec, word::Word};
use binius_frontend::{Circuit, CircuitBuilder, Wire};
use binius_hash::StdHashSuite;
use binius_prover::{OptimalPackedB128, Prover};
use binius_verifier::{
    Verifier,
    config::StdChallenger,
    transcript::{ProverTranscript, VerifierTranscript},
};
use serde_json::{Value, json};
use std::sync::Arc;

struct Wires {
    a: Wire,
    b: Wire,
    c: Wire,
    q: Option<Wire>,
}
pub(super) struct Context {
    corpus: Arc<Corpus>,
    circuit: Circuit,
    wires: Vec<Wires>,
    prover: Prover<OptimalPackedB128, StdHashSuite>,
    verifier: Verifier<StdHashSuite>,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let (circuit, wires) = compile(&corpus);
        let verifier = Verifier::<StdHashSuite>::setup_with_security_bits(
            circuit.constraint_system().clone(),
            1,
            100,
        )
        .expect("Binius setup");
        let prover = Prover::setup(verifier.clone()).expect("Binius prover setup");
        Self {
            corpus,
            circuit,
            wires,
            prover,
            verifier,
        }
    }
    fn populate(&self, corrupt_output: bool) -> Result<ValueVec, String> {
        Ok(populate(&self.corpus, &self.circuit, &self.wires, corrupt_output)?.into_value_vec())
    }
    pub(super) fn config(&self) -> Value {
        json!({"piop":"Binius64 native integer multiplication and bit constraints","pcs":"ring switching/BaseFold","fri_query_target_bits":100,"log_inv_rate":1,"fri_queries":self.verifier.fri_params().n_test_queries()})
    }
    pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
        capture.begin();
        let start = capture.now_ns();
        let witness = self.populate(false).expect("Binius witness evaluation");
        let wend = capture.now_ns();
        let mut transcript = ProverTranscript::new(StdChallenger::default());
        self.prover
            .prove(&witness, &mut transcript)
            .expect("Binius full proof");
        let bytes = transcript.finalize();
        let ready = capture.now_ns();
        let vstart = capture.now_ns();
        let mut vt = VerifierTranscript::new(StdChallenger::default(), bytes.clone());
        self.verifier
            .verify(witness.public(), &mut vt)
            .expect("Binius full verification");
        vt.finalize().expect("consume full Binius proof");
        let end = capture.now_ns();
        let raw = capture.finish();
        let mut t = Timing::new(start, wend, ready, vstart, end);
        let pack = captured(&raw, "prepare_witness", wend, ready);
        t.add(
            "witness_packing",
            "witness-generation",
            pack.start_ns,
            pack.end_ns,
        );
        let commit = captured(&raw, "commit_witness", wend, ready);
        let ring = captured(&raw, "ring_switching", wend, ready);
        t.add("commit", "commit", commit.start_ns, commit.end_ns);
        t.add("piop", "constraint-proof", commit.end_ns, ring.start_ns);
        t.add("opening", "opening-proof", ring.start_ns, ready);
        t.proof_bytes = Some(bytes.len());
        t
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn wrong_product_is_rejected() {
        for workload in [Workload::U32, Workload::BabyBear] {
            let max = if workload == Workload::U32 {
                u32::MAX
            } else {
                (BABY_P - 1) as u32
            };
            let inputs = [(0, 0), (0, max), (1, max), (max, max)].repeat(4);
            let c = Context::setup(Arc::new(Corpus::from_inputs(workload, inputs)));
            // Population computes wires; acceptance is checked by the constraint verifier.
            let valid = c.populate(false).unwrap();
            binius_core::verify::verify_constraints(c.circuit.constraint_system(), &valid).unwrap();
            assert!(c.populate(true).is_err());
        }
    }
}

fn compile(corpus: &Corpus) -> (Circuit, Vec<Wires>) {
    let builder = CircuitBuilder::new();
    let p = builder.add_constant_64(BABY_P);
    let wires = (0..corpus.inputs.len())
        .map(|i| {
            let b = builder.subcircuit(format!("multiply[{i}]"));
            let a = b.add_witness();
            let rhs = b.add_witness();
            let c = b.add_witness();
            b.assert_zero("a is u32", b.shr(a, 32));
            b.assert_zero("b is u32", b.shr(rhs, 32));
            let (hi, lo) = b.imul(a, rhs);
            b.assert_zero("product high word", hi);
            let q = match corpus.workload {
                Workload::U32 => {
                    b.assert_eq("full u64 product", lo, c);
                    None
                }
                Workload::BabyBear => {
                    b.assert_true("a canonical", b.icmp_ult(a, p));
                    b.assert_true("b canonical", b.icmp_ult(rhs, p));
                    b.assert_true("c canonical", b.icmp_ult(c, p));
                    let q = b.add_witness();
                    b.assert_true("quotient bounded", b.icmp_ult(q, p));
                    let (qh, ql) = b.imul(q, p);
                    b.assert_zero("quotient product high", qh);
                    let (reconstructed, carry) = b.iadd(ql, c);
                    // iadd's carry wire contains every carry bit; only its MSB is overflow.
                    b.assert_zero("no reconstruction overflow", b.shr(carry, 63));
                    b.assert_eq("a*b = p*q+c", lo, reconstructed);
                    Some(q)
                }
            };
            Wires { a, b: rhs, c, q }
        })
        .collect();
    let circuit = builder.build();
    (circuit, wires)
}

fn populate<'a>(
    corpus: &Corpus,
    circuit: &'a Circuit,
    wires: &[Wires],
    corrupt_output: bool,
) -> Result<binius_frontend::WitnessFiller<'a>, String> {
    let mut filler = circuit.new_witness_filler();
    for (i, (w, &(a, b))) in wires.iter().zip(&corpus.inputs).enumerate() {
        filler[w.a] = Word(a as u64);
        filler[w.b] = Word(b as u64);
        filler[w.c] = Word(corpus.workload.output(a, b) ^ u64::from(corrupt_output && i == 0));
        if let Some(q) = w.q {
            filler[q] = Word((a as u64 * b as u64) / BABY_P);
        }
    }
    circuit
        .populate_wire_witness(&mut filler)
        .map_err(|error| error.to_string())?;
    Ok(filler)
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let (circuit, wires) = compile(corpus);
    let started = std::time::Instant::now();
    let filler = populate(corpus, &circuit, &wires, false).expect("Binius materialization");
    let generation_ms = started.elapsed().as_secs_f64() * 1e3;
    let rows = wires
        .iter()
        .map(|w| {
            [
                filler[w.a].0,
                filler[w.b].0,
                filler[w.c].0,
                w.q.map_or(0, |q| filler[q].0),
            ]
        })
        .collect();
    super::WitnessAudit::check(corpus, rows, generation_ms, "Binius wire values", false)
}
