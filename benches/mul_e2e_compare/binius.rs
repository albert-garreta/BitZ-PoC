use super::{BABY_P, Corpus, Timing, TraceCapture, Workload, captured};
#[cfg(test)]
#[allow(unused_imports)]
use super::edge_corpus;
use binius_circuits::bignum::{self, BigUint};
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

pub(super) enum Wires {
    /// One 64-bit-or-narrower gate: operands, output, auxiliary value.
    Narrow {
        a: Wire,
        b: Wire,
        c: Wire,
        q: Option<Wire>,
    },
    /// One `u128` gate: two-limb operands and the four-limb product.
    Wide { x: BigUint, y: BigUint, z: BigUint },
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
            log_inv_rate(),
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
        let piop = match self.corpus.workload {
            Workload::U32 | Workload::BabyBear => "Binius64 native integer multiplication and bit constraints",
            Workload::U64 => "Binius64 native 64 x 64 -> 128 integer multiplication",
            Workload::U128 => "Binius64 bignum 128 x 128 -> 256 multiplication (four native imul limb products with carry chains)",
        };
        json!({"piop":piop,"pcs":"ring switching/BaseFold","fri_query_target_bits":100,"log_inv_rate":log_inv_rate(),"fri_queries":self.verifier.fri_params().n_test_queries()})
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
        let mut t = Timing::new(start, wend, ready, vstart, end, bytes.len());
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
        t
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn wrong_product_is_rejected() {
        for workload in [
            Workload::U32,
            Workload::BabyBear,
            Workload::U64,
            Workload::U128,
        ] {
            let c = Context::setup(Arc::new(edge_corpus(workload)));
            // Population computes wires; acceptance is checked by the constraint verifier.
            let valid = c.populate(false).unwrap();
            binius_core::verify::verify_constraints(c.circuit.constraint_system(), &valid).unwrap();
            assert!(c.populate(true).is_err());
        }
    }
}

/// `log2` of the inverse Reed–Solomon rate of the BaseFold commitment:
/// `F2Z_BINIUS_LOG_INV_RATE` (default 1 = rate 1/2, the Binius64 default).
/// A lower rate needs fewer test queries (smaller proof) at the cost of a
/// larger encoding.
fn log_inv_rate() -> usize {
    std::env::var("F2Z_BINIUS_LOG_INV_RATE")
        .ok()
        .map(|v| v.parse().expect("F2Z_BINIUS_LOG_INV_RATE must be an integer"))
        .unwrap_or(1)
}

pub(super) fn compile(corpus: &Corpus) -> (Circuit, Vec<Wires>) {
    let builder = CircuitBuilder::new();
    let p = builder.add_constant_64(BABY_P);
    let wires = (0..corpus.len())
        .map(|i| {
            let b = builder.subcircuit(format!("multiply[{i}]"));
            if corpus.workload.is_wide() {
                // Native 128 x 128 -> 256 through Binius64's bignum circuit:
                // four `imul` limb products accumulated with carry chains,
                // asserted equal to the four witness limbs of the product.
                let x = BigUint::new_witness(&b, 2);
                let y = BigUint::new_witness(&b, 2);
                let z = BigUint::new_witness(&b, 4);
                let product = bignum::textbook_mul(&b, &x, &y);
                bignum::assert_eq(&b, "product limb", &product, &z);
                return Wires::Wide { x, y, z };
            }
            let a = b.add_witness();
            let rhs = b.add_witness();
            let c = b.add_witness();
            let (hi, lo) = b.imul(a, rhs);
            if corpus.workload == Workload::U64 {
                // Native 64 x 64 -> 128: both product words are witness
                // values and the operands are full words, so no range checks.
                let z_hi = b.add_witness();
                b.assert_eq("product low word", lo, c);
                b.assert_eq("product high word", hi, z_hi);
                return Wires::Narrow {
                    a,
                    b: rhs,
                    c,
                    q: Some(z_hi),
                };
            }
            b.assert_zero("a is u32", b.shr(a, 32));
            b.assert_zero("b is u32", b.shr(rhs, 32));
            b.assert_zero("product high word", hi);
            let q = match corpus.workload {
                Workload::U32 => {
                    b.assert_eq("full u64 product", lo, c);
                    None
                }
                Workload::U64 | Workload::U128 => unreachable!(),
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
            Wires::Narrow { a, b: rhs, c, q }
        })
        .collect();
    let circuit = builder.build();
    (circuit, wires)
}

pub(super) fn populate<'a>(
    corpus: &Corpus,
    circuit: &'a Circuit,
    wires: &[Wires],
    corrupt_output: bool,
) -> Result<binius_frontend::WitnessFiller<'a>, String> {
    let mut filler = circuit.new_witness_filler();
    let corrupt = |i: usize| u64::from(corrupt_output && i == 0);
    match &corpus.operands {
        super::Operands::Narrow(inputs) => {
            for (i, (w, &(a, b))) in wires.iter().zip(inputs).enumerate() {
                let Wires::Narrow { a: wa, b: wb, c: wc, q: wq } = w else {
                    unreachable!("narrow corpus with wide wires")
                };
                let [_, _, c, q] = corpus.workload.native_row(a, b);
                filler[*wa] = Word(a);
                filler[*wb] = Word(b);
                filler[*wc] = Word(c ^ corrupt(i));
                if let Some(wire) = *wq {
                    filler[wire] = Word(q);
                }
            }
        }
        super::Operands::Wide(inputs) => {
            for (i, (w, &(x, y))) in wires.iter().zip(inputs).enumerate() {
                let Wires::Wide { x: wx, y: wy, z: wz } = w else {
                    unreachable!("wide corpus with narrow wires")
                };
                let [_, _, lo, hi] = corpus.workload.wide_row(x, y);
                wx.populate_limbs(&mut filler, &limbs(x));
                wy.populate_limbs(&mut filler, &limbs(y));
                let [z0, z1] = limbs(lo);
                let [z2, z3] = limbs(hi);
                wz.populate_limbs(&mut filler, &[z0 ^ corrupt(i), z1, z2, z3]);
            }
        }
    }
    circuit
        .populate_wire_witness(&mut filler)
        .map_err(|error| error.to_string())?;
    Ok(filler)
}

/// The two 64-bit limbs of a `u128`, least significant first.
fn limbs(value: u128) -> [u64; 2] {
    [value as u64, (value >> 64) as u64]
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let (circuit, wires) = compile(corpus);
    let started = std::time::Instant::now();
    let filler = populate(corpus, &circuit, &wires, false).expect("Binius materialization");
    let generation_ms = started.elapsed().as_secs_f64() * 1e3;
    if corpus.workload.is_wide() {
        let read = |limbs: &[Wire]| {
            limbs
                .iter()
                .rev()
                .fold(0_u128, |acc, &limb| (acc << 64) | u128::from(filler[limb].0))
        };
        let rows = wires
            .iter()
            .map(|w| {
                let Wires::Wide { x, y, z } = w else {
                    unreachable!("wide corpus with narrow wires")
                };
                [read(&x.limbs), read(&y.limbs), read(&z.limbs[..2]), read(&z.limbs[2..])]
            })
            .collect();
        return super::WitnessAudit::check_wide(corpus, rows, generation_ms, "Binius wire values");
    }
    let rows = wires
        .iter()
        .map(|w| {
            let Wires::Narrow { a, b, c, q } = w else {
                unreachable!("narrow corpus with wide wires")
            };
            [
                filler[*a].0,
                filler[*b].0,
                filler[*c].0,
                q.map_or(0, |q| filler[q].0),
            ]
        })
        .collect();
    super::WitnessAudit::check(corpus, rows, generation_ms, "Binius wire values", false)
}
