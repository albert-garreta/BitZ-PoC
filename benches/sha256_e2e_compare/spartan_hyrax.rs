//! Limber's prime-field Spartan prover with its native Hyrax PCS.

use std::{hint::black_box, marker::PhantomData, sync::Arc, time::Instant};

use bellpepper::gadgets::sha256::sha256_block_no_padding;
use bellpepper_core::{
    ConstraintSystem, SynthesisError,
    boolean::{AllocatedBit, Boolean},
    num::AllocatedNum,
};
use bincode::Options;
use ff::{Field, PrimeField, PrimeFieldBits};
use limber::{
    provider::T256HyraxEngine,
    spartan::SpartanSNARK,
    traits::{Engine, circuit::SpartanCircuit, snark::R1CSSNARKTrait},
};

use super::{CapturedSpan, Corpus, SemanticSpan, TraceCapture, TrialMetrics, humanize, operation};

type E = T256HyraxEngine;
type Scalar = <E as Engine>::Scalar;

#[derive(Clone)]
struct Sha256CompressionCircuit<S: PrimeField> {
    corpus: Arc<Corpus>,
    _scalar: PhantomData<S>,
}

impl<S: PrimeField + PrimeFieldBits> Sha256CompressionCircuit<S> {
    fn new(corpus: Arc<Corpus>) -> Self {
        Self {
            corpus,
            _scalar: PhantomData,
        }
    }

    fn public_bits(&self) -> Vec<bool> {
        let mut values = Vec::with_capacity(self.corpus.cases.len() * 24 * 32);
        for case in &self.corpus.cases {
            for word in case.block.iter().chain(&case.output) {
                values.extend((0..32).rev().map(|bit| (word >> bit) & 1 == 1));
            }
        }
        values
    }
}

impl<EngineT: Engine> SpartanCircuit<EngineT> for Sha256CompressionCircuit<EngineT::Scalar> {
    fn public_values(&self) -> Result<Vec<EngineT::Scalar>, SynthesisError> {
        Ok(self
            .public_bits()
            .into_iter()
            .map(|bit| {
                if bit {
                    EngineT::Scalar::ONE
                } else {
                    EngineT::Scalar::ZERO
                }
            })
            .collect())
    }

    fn shared<CS: ConstraintSystem<EngineT::Scalar>>(
        &self,
        _: &mut CS,
    ) -> Result<Vec<AllocatedNum<EngineT::Scalar>>, SynthesisError> {
        Ok(Vec::new())
    }

    fn precommitted<CS: ConstraintSystem<EngineT::Scalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<EngineT::Scalar>],
    ) -> Result<Vec<AllocatedNum<EngineT::Scalar>>, SynthesisError> {
        for (instance, case) in self.corpus.cases.iter().enumerate() {
            let mut block_bits = Vec::with_capacity(512);
            for (word_index, &word) in case.block.iter().enumerate() {
                for bit_index in (0..32).rev() {
                    let value = (word >> bit_index) & 1 == 1;
                    let bit = AllocatedBit::alloc(
                        cs.namespace(|| format!("sha[{instance}].m[{word_index}].b[{bit_index}]")),
                        Some(value),
                    )?;
                    let public = AllocatedNum::alloc_input(
                        cs.namespace(|| {
                            format!("sha[{instance}].public_m[{word_index}].b[{bit_index}]")
                        }),
                        || {
                            Ok(if value {
                                EngineT::Scalar::ONE
                            } else {
                                EngineT::Scalar::ZERO
                            })
                        },
                    )?;
                    cs.enforce(
                        || "public message bit",
                        |_| Boolean::from(bit.clone()).lc(CS::one(), EngineT::Scalar::ONE),
                        |lc| lc + CS::one(),
                        |lc| lc + public.get_variable(),
                    );
                    block_bits.push(Boolean::from(bit));
                }
            }

            let output = sha256_block_no_padding(
                cs.namespace(|| format!("sha[{instance}].compress")),
                &block_bits,
            )?;
            assert_eq!(output.len(), 256);
            let expected = case.output.iter().flat_map(|word| {
                (0..32)
                    .rev()
                    .map(move |bit_index| (word >> bit_index) & 1 == 1)
            });
            for (bit_index, (bit, expected)) in output.iter().zip(expected).enumerate() {
                assert_eq!(
                    bit.get_value(),
                    Some(expected),
                    "Bellpepper SHA output mismatch"
                );
                let public = AllocatedNum::alloc_input(
                    cs.namespace(|| format!("sha[{instance}].public_h[{bit_index}]")),
                    || {
                        Ok(if expected {
                            EngineT::Scalar::ONE
                        } else {
                            EngineT::Scalar::ZERO
                        })
                    },
                )?;
                cs.enforce(
                    || "public output bit",
                    |_| bit.lc(CS::one(), EngineT::Scalar::ONE),
                    |lc| lc + CS::one(),
                    |lc| lc + public.get_variable(),
                );
            }
        }
        Ok(Vec::new())
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<EngineT::Scalar>>(
        &self,
        _: &mut CS,
        _: &[AllocatedNum<EngineT::Scalar>],
        _: &[AllocatedNum<EngineT::Scalar>],
        _: Option<&[EngineT::Scalar]>,
    ) -> Result<(), SynthesisError> {
        Ok(())
    }
}

pub struct Context {
    circuit: Sha256CompressionCircuit<Scalar>,
    pk: <SpartanSNARK<E> as R1CSSNARKTrait<E>>::ProverKey,
    vk: <SpartanSNARK<E> as R1CSSNARKTrait<E>>::VerifierKey,
    pub setup_ms: f64,
}

impl Context {
    pub fn setup(corpus: &Corpus) -> Self {
        let started = Instant::now();
        let circuit = Sha256CompressionCircuit::new(Arc::new(corpus.clone()));
        let (pk, vk) =
            SpartanSNARK::<E>::setup(circuit.clone()).expect("Limber Spartan/Hyrax setup succeeds");
        Self {
            circuit,
            pk,
            vk,
            setup_ms: started.elapsed().as_secs_f64() * 1e3,
        }
    }

    pub fn run(&self, capture: &TraceCapture) -> (TrialMetrics, Vec<SemanticSpan>) {
        capture.begin();
        let root_start = capture.now_ns();
        let prep = SpartanSNARK::<E>::prep_prove(&self.pk, self.circuit.clone(), true)
            .expect("Limber SHA witness preparation succeeds");
        let (proof, _) = SpartanSNARK::<E>::prove(&self.pk, self.circuit.clone(), prep, true)
            .expect("Limber Spartan/Hyrax proof succeeds");
        let proof_ready = capture.now_ns();
        let proof_bytes = bincode::serialize(&proof)
            .expect("serialize Limber Spartan/Hyrax proof")
            .len();
        let verify_start = capture.now_ns();
        let observed = proof
            .verify(&self.vk)
            .expect("Limber Spartan/Hyrax proof verifies");
        let expected =
            <Sha256CompressionCircuit<Scalar> as SpartanCircuit<E>>::public_values(&self.circuit)
                .expect("public values");
        assert_eq!(observed, expected);
        let verify_end = capture.now_ns();
        let raw = capture.finish();
        black_box(&proof);
        let spans = semantic_spans(&raw, root_start, proof_ready, verify_start, verify_end);
        (TrialMetrics::from_spans(&spans, proof_bytes), spans)
    }
}

pub fn tamper_self_test() {
    let corpus = Corpus::new(1, 0x5350_4152_5441_4e54);
    let context = Context::setup(&corpus);
    let prep = SpartanSNARK::<E>::prep_prove(&context.pk, context.circuit.clone(), true)
        .expect("Spartan self-test witness preparation succeeds");
    let (proof, _) = SpartanSNARK::<E>::prove(&context.pk, context.circuit.clone(), prep, true)
        .expect("Spartan self-test proof succeeds");
    let expected =
        <Sha256CompressionCircuit<Scalar> as SpartanCircuit<E>>::public_values(&context.circuit)
            .expect("Spartan self-test public values");
    assert_eq!(
        proof.verify(&context.vk).expect("valid Spartan proof"),
        expected
    );

    let mut changed_block = expected.clone();
    changed_block[0] += Scalar::ONE;
    assert_ne!(
        proof.verify(&context.vk).expect("valid original proof"),
        changed_block,
        "changing a public Spartan block bit must invalidate the statement"
    );
    let mut changed_output = expected.clone();
    changed_output[16 * 32] += Scalar::ONE;
    assert_ne!(
        proof.verify(&context.vk).expect("valid original proof"),
        changed_output,
        "changing a public Spartan output bit must invalidate the statement"
    );

    let encoded = bincode::serialize(&proof).expect("serialize Spartan proof for tamper test");
    let strict = || {
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .reject_trailing_bytes()
    };
    let decoded: SpartanSNARK<E> = strict()
        .deserialize(&encoded)
        .expect("strictly decode valid Spartan proof");
    assert_eq!(
        decoded.verify(&context.vk).expect("decoded Spartan proof"),
        expected
    );

    let mut corrupted = encoded.clone();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 1;
    if let Ok(decoded) = strict().deserialize::<SpartanSNARK<E>>(&corrupted) {
        assert!(
            decoded.verify(&context.vk).is_err()
                || decoded.verify(&context.vk).ok().as_ref() != Some(&expected),
            "a corrupted Spartan proof must not verify as the original statement"
        );
    }

    let mut trailing = encoded;
    trailing.push(0);
    assert!(
        strict().deserialize::<SpartanSNARK<E>>(&trailing).is_err(),
        "trailing Spartan proof data must be rejected"
    );
}

fn semantic_spans(
    raw: &[CapturedSpan],
    root_start: u64,
    proof_ready: u64,
    verify_start: u64,
    verify_end: u64,
) -> Vec<SemanticSpan> {
    let find = |name: &str| {
        raw.iter()
            .filter(|span| span.name == name)
            .min_by_key(|span| span.start_ns)
            .unwrap_or_else(|| panic!("missing Limber span {name}"))
    };
    let synth = find("precommitted_witness_synthesize");
    let commit_pre = find("commit_witness_precommitted");
    let commit_rest = find("commit_witness_rest");
    let pcs = find("pcs_prove");
    let online_start = commit_pre.start_ns.min(commit_rest.start_ns);
    let piop_start = commit_pre.end_ns.max(commit_rest.end_ns);

    let mut spans = vec![
        make_span(
            "spartan-root",
            None,
            "spartan-hyrax.verified-trial",
            "Complete verified Spartan/Hyrax trial",
            "end-to-end",
            vec!["end-to-end"],
            root_start,
            verify_end,
            Some("end-to-end"),
            false,
        ),
        make_span(
            "spartan-witness-to-proof",
            Some("spartan-root"),
            "spartan-hyrax.witness-to-proof",
            "Spartan witness generation through proof readiness",
            "proving",
            vec!["proving"],
            root_start,
            proof_ready,
            None,
            false,
        ),
        make_span(
            "spartan-online",
            Some("spartan-root"),
            "spartan-hyrax.online",
            "Spartan/Hyrax online prover",
            "proving",
            vec!["proving"],
            online_start,
            proof_ready,
            Some("proving"),
            false,
        ),
        make_span(
            "spartan-witness-before-commit",
            Some("spartan-witness-to-proof"),
            "spartan-hyrax.witness",
            "Bellpepper assignment evaluation",
            "witness-generation",
            vec!["witness-generation"],
            synth.start_ns,
            commit_pre.start_ns,
            None,
            true,
        ),
        make_span(
            "spartan-witness-after-commit",
            Some("spartan-witness-to-proof"),
            "spartan-hyrax.witness-finalize",
            "Bellpepper assignment finalization",
            "witness-generation",
            vec!["witness-generation"],
            commit_pre.end_ns,
            synth.end_ns,
            None,
            false,
        ),
        make_span(
            "spartan-commit-pre",
            Some("spartan-online"),
            "spartan-hyrax.commit-precommitted",
            "Hyrax precommitted witness commitment",
            "commit",
            vec!["commit", "pcs", "proving"],
            commit_pre.start_ns,
            commit_pre.end_ns,
            Some("commit"),
            true,
        ),
        make_span(
            "spartan-commit-rest",
            Some("spartan-online"),
            "spartan-hyrax.commit-rest",
            "Hyrax rest-witness commitment",
            "commit",
            vec!["commit", "pcs", "proving"],
            commit_rest.start_ns,
            commit_rest.end_ns,
            Some("commit"),
            true,
        ),
        make_span(
            "spartan-piop",
            Some("spartan-online"),
            "spartan-hyrax.piop",
            "Spartan outer and inner sumchecks",
            "constraint-proof",
            vec!["constraint-proof", "sumcheck", "proving"],
            piop_start,
            pcs.start_ns,
            Some("constraint-proof"),
            true,
        ),
        make_span(
            "spartan-iop",
            Some("spartan-online"),
            "spartan-hyrax.iop",
            "Hyrax evaluation argument",
            "opening-proof",
            vec!["opening-proof", "pcs", "proving"],
            pcs.start_ns,
            proof_ready,
            Some("opening-proof"),
            true,
        ),
        make_span(
            "spartan-verify",
            Some("spartan-root"),
            "spartan-hyrax.verify",
            "Verify Spartan/Hyrax proof",
            "verification",
            vec!["verification"],
            verify_start,
            verify_end,
            Some("verification"),
            true,
        ),
    ];
    spans.extend(
        raw.iter()
            .enumerate()
            .filter(|(_, raw)| raw.start_ns >= root_start && raw.end_ns <= proof_ready)
            .map(|(index, raw)| SemanticSpan {
                id: format!("spartan-detail-{index}"),
                parent: Some("spartan-witness-to-proof".to_owned()),
                operation: format!("spartan-hyrax.{}", operation(&raw.name)),
                name: humanize(&raw.name),
                short_name: "Procedure".to_owned(),
                primary_phase: "proving",
                phase_tags: vec!["proving"],
                start_ns: raw.start_ns,
                end_ns: raw.end_ns,
                scope_kind: "procedure",
                scope_tag: None,
                primary_sequence: false,
                math_latex: vec![],
            }),
    );
    spans
}

#[allow(clippy::too_many_arguments)]
fn make_span(
    id: &str,
    parent: Option<&str>,
    operation: &str,
    name: &str,
    primary_phase: &'static str,
    phase_tags: Vec<&'static str>,
    start_ns: u64,
    end_ns: u64,
    scope_tag: Option<&'static str>,
    primary_sequence: bool,
) -> SemanticSpan {
    SemanticSpan {
        id: id.to_owned(),
        parent: parent.map(str::to_owned),
        operation: operation.to_owned(),
        name: name.to_owned(),
        short_name: name.to_owned(),
        primary_phase,
        phase_tags,
        start_ns,
        end_ns,
        scope_kind: "phase",
        scope_tag,
        primary_sequence,
        math_latex: match primary_phase {
            "witness-generation" => {
                vec![r"\widehat H_i=\operatorname{Compress}_{\mathrm{SHA256}}(\mathrm{IV},M_i)"]
            }
            "commit" => vec![r"C_W=\operatorname{Hyrax.Commit}(W;r)"],
            "constraint-proof" => {
                vec![r"\sum_x\operatorname{eq}(\tau,x)((Az)(x)(Bz)(x)-(Cz)(x))=0"]
            }
            "opening-proof" => vec![r"\operatorname{Hyrax.Open}(C_W,r,W(r))"],
            _ => vec![],
        },
    }
}
