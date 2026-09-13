//! `binius64-ligerito`: Binius64's native multiplication circuit and PIOP
//! (the same wires as the `binius64` backend), with every oracle committed
//! and opened by the F2Z opener — rate 1/8, Johnson-regime Ligerito with fold
//! and query grinding, Round 0 — and the whole protocol gated at 100 bits by
//! a union bound, the yardstick of the `f2z` rows.
use super::trace_capture::BiniusLigeritoPhases;
use super::{Corpus, Timing, TraceCapture, Workload, binius};
use binius_frontend::Circuit;
use f2z::binius_ligerito::Prepared;
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) struct Context {
    corpus: Arc<Corpus>,
    circuit: Circuit,
    wires: Vec<binius::Wires>,
    prepared: Prepared,
}

impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let (circuit, wires) = binius::compile(&corpus);
        let prepared = Prepared::new(circuit.constraint_system()).expect("binius64-ligerito setup");
        Self {
            corpus,
            circuit,
            wires,
            prepared,
        }
    }

    pub(super) fn config(&self) -> Value {
        let piop = match self.corpus.workload {
            Workload::U32 => {
                "Binius64 native multiplication with 32-bit inputs and low-32-bit result"
            }
            Workload::U64 => "Binius64 native 64 x 64 -> 128 integer multiplication",
            Workload::U128 => {
                "Binius64 bignum 128 x 128 -> 256 multiplication (four native imul limb products with carry chains)"
            }
        };
        let cs = self.circuit.constraint_system();
        let security = self.prepared.security();
        let witness = self.prepared.opener(0);
        json!({
            "piop": piop,
            "pcs": "F2Z opener: ring switching + Johnson-regime Ligerito with fold/query grinding and Round 0",
            "log_inv_rate": f2z::binary_pcs::LOG_INV_RATE,
            "regime": "johnson-ood",
            "target_bits": security.target_bits,
            "whole_protocol_bits": security.algebraic_bits,
            "binding_term": security.binding_term().map(|t| format!("{}:{:.2}", t.name, -t.error_bound.log2())),
            "ligerito_component_bits": self.prepared.component_bits(),
            "level0_queries": witness.level0_queries(),
            "level0_query_grinding_bits": witness.level0_query_grinding_bits(),
            "level0_fold_grinding_bits": witness.level0_fold_grinding_bits(),
            "ood_grinding_bits": witness.ood_grinding_bits(),
            "oracle_logs": self.prepared.oracle_specs().iter().map(|s| s.log_msg_len).collect::<Vec<_>>(),
            "word_constraints": {"and": cs.n_and_constraints(), "imul": cs.n_imul_constraints(),
                "zero": cs.n_zero_constraints(), "bmul": cs.n_bmul_constraints()},
        })
    }

    pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
        capture.begin();
        let start = capture.now_ns();
        let witness = binius::populate(&self.corpus, &self.circuit, &self.wires, false)
            .expect("Binius witness evaluation")
            .into_value_vec();
        let wend = capture.now_ns();
        let proof = self
            .prepared
            .prove(&witness)
            .expect("binius64-ligerito full proof");
        let bytes = proof.to_bytes();
        let ready = capture.now_ns();
        let vstart = capture.now_ns();
        let decoded = self
            .prepared
            .proof_from_bytes(&bytes)
            .expect("binius64-ligerito proof decodes");
        self.prepared
            .verify(witness.inout(), &decoded)
            .expect("binius64-ligerito full verification");
        let end = capture.now_ns();
        let phases = BiniusLigeritoPhases::from_spans(&capture.finish());
        let mut t = Timing::new(start, wend, ready, vstart, end, bytes.len());
        t.add("commit", "commit", phases.commit.0, phases.commit.1);
        for (start, end) in phases.piop {
            t.add("piop", "constraint-proof", start, end);
        }
        for (start, end) in phases.opening {
            t.add("opening", "opening-proof", start, end);
        }
        t
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn span_metrics_cover_repeated_verified_multiplication_trials() {
        use super::super::CaptureLayer;
        use super::*;
        use tracing_subscriber::prelude::*;

        // The F2Z opener requires packed log >= 13.
        let context = Context::setup(Arc::new(Corpus::new(Workload::U32, 11, 7)));
        let witness = binius::populate(&context.corpus, &context.circuit, &context.wires, false)
            .unwrap()
            .into_value_vec();
        let expected =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                context.prepared.prove(&witness).unwrap().to_bytes()
            });
        let layer = CaptureLayer::default();
        let capture = layer.capture();
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), || {
            capture.begin();
            let proof = context.prepared.prove(&witness).unwrap().to_bytes();
            assert_eq!(proof, expected, "instrumentation changed the proof bytes");
            let phases = BiniusLigeritoPhases::from_spans(&capture.finish());
            assert_eq!(
                phases.opening.len(),
                context.prepared.oracle_specs().len() + 1
            );
            // Warmup followed by five independent samples: capture state must reset.
            for _ in 0..6 {
                let timing = context.run(&capture);
                timing.validate();
                assert_eq!(timing.proof_bytes, proof.len());
                let first_opening = timing
                    .phases
                    .iter()
                    .find(|p| p.tag == "opening-proof")
                    .unwrap();
                assert!(
                    timing
                        .phases
                        .iter()
                        .any(|p| { p.tag == "constraint-proof" && p.start >= first_opening.end }),
                    "Round 0 must remain before subsequent PIOP work"
                );
            }
        });
    }
}
