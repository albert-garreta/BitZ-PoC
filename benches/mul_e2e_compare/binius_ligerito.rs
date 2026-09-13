//! `binius64-ligerito`: Binius64's native multiplication circuit and PIOP
//! (the same wires as the `binius64` backend), with every oracle committed
//! and opened by the F2Z opener — Johnson-regime Ligerito with fold and query
//! grinding and Round 0, at the campaign's Binius rate
//! (`F2Z_BINIUS_LOG_INV_RATE`, default 1 = rate 1/2) — and the whole protocol
//! gated at 100 bits under `F2Z_BINIUS_LIGERITO_ACCOUNTING`: `union` (default;
//! a union bound over every term) or `rbr` (the round-by-round minimum, the
//! figure F2Z's own rows report).
use super::{Corpus, Timing, TraceCapture, Workload, binius};
use binius_frontend::Circuit;
use f2z::binius_ligerito::{Accounting, Prepared};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

pub(super) struct Context {
    corpus: Arc<Corpus>,
    circuit: Circuit,
    wires: Vec<binius::Wires>,
    prepared: Prepared,
}

impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let (circuit, wires) = binius::compile(&corpus);
        let prepared =
            Prepared::with_options(circuit.constraint_system(), binius::log_inv_rate(), accounting())
                .expect("binius64-ligerito setup");
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
            "log_inv_rate": self.prepared.log_inv_rate(),
            "regime": "johnson-ood",
            "accounting": security.accounting.name(),
            "target_bits": security.target_bits,
            "whole_protocol_bits": security.algebraic_bits,
            "union_bound_bits": security.union_bound_bits,
            "round_by_round_bits": security.round_by_round_bits,
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
        let (proof, phases) = self
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
        let _ = capture.finish();
        let mut t = Timing::new(start, wend, ready, vstart, end, bytes.len());
        // The prover reports its own phase durations (commit, PIOP prefix,
        // opening = every Round 0 + ring switch + Ligerito); lay them out
        // consecutively inside the online span for the tagged unions.
        let ns = |d: Duration| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        let commit_end = (wend + ns(phases.commit)).min(ready);
        let piop_end = (commit_end + ns(phases.piop)).min(ready);
        let opening_end = (piop_end + ns(phases.opening)).min(ready);
        t.add("commit", "commit", wend, commit_end);
        t.add("piop", "constraint-proof", commit_end, piop_end);
        t.add("opening", "opening-proof", piop_end, opening_end);
        t
    }
}

/// `F2Z_BINIUS_LIGERITO_ACCOUNTING`: `union` (default) or `rbr`.
fn accounting() -> Accounting {
    match std::env::var("F2Z_BINIUS_LIGERITO_ACCOUNTING").as_deref() {
        Err(_) | Ok("union") | Ok("union-bound") => Accounting::UnionBound,
        Ok("rbr") | Ok("round-by-round") => Accounting::RoundByRound,
        Ok(other) => panic!("F2Z_BINIUS_LIGERITO_ACCOUNTING must be union or rbr, not {other:?}"),
    }
}
