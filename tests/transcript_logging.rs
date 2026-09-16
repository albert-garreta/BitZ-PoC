use crypto_primitives::crypto_bigint_uint::Uint;
use f2z::{
    transcript::{
        Blake3Transcript,
        logging::{Operation, TranscriptLogs, read_byte_events as read_events},
        traits::Transcript,
    },
    utils::primality::PrimalityTest,
};
use std::{cell::Cell, collections::BTreeMap, path::Path};
use tracing_subscriber::prelude::*;

fn replay(path: &Path) -> BTreeMap<String, blake3::Hasher> {
    let mut states = BTreeMap::new();
    for event in read_events(path).unwrap() {
        let event = event.unwrap();
        assert_eq!(event.target, "f2z::transcript");
        let role = event
            .spans
            .iter()
            .find(|s| s.name == "transcript_operation")
            .unwrap()
            .fields["role"]
            .as_str()
            .unwrap()
            .to_owned();
        let state = states.entry(role).or_insert_with(blake3::Hasher::new);
        let bytes = event.fields.bytes().unwrap();
        match event.fields.operation {
            Operation::Absorb => {
                state.update(&bytes);
            }
            Operation::Squeeze => {
                let mut expected = vec![0; bytes.len()];
                state.finalize_xof().fill(&mut expected);
                assert_eq!(bytes, expected, "squeezed bytes differ from BLAKE3 replay");
            }
        }
    }
    states
}

thread_local! { static ATTEMPTS: Cell<usize> = const { Cell::new(0) }; }
#[derive(Debug, Clone)]
struct AcceptThird;
impl PrimalityTest<Uint<2>> for AcceptThird {
    fn is_probably_prime(_: &Uint<2>) -> bool {
        ATTEMPTS.with(|count| {
            count.set(count.get() + 1);
            count.get() == 3
        })
    }
}

#[test]
fn byte_replay_captures_framing_retries_and_changing_context() {
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    // Two layers, one subscriber. The human-facing layer excludes byte events.
    let subscriber = tracing_subscriber::registry().with(logs.layer()).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::sink)
            .with_filter(tracing_subscriber::filter::filter_fn(|m| {
                m.target() != "f2z::transcript"
            })),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    let trial = logs.begin_trial(dir.path().join("trace.jsonl")).unwrap();
    let mut transcript = trial.wrap("prover", Blake3Transcript::new());
    let parent = tracing::info_span!("trial", round = 0, tag = "multiplication");
    let entered = parent.enter();
    tracing::info!("this must not enter the transcript file");
    // An unrelated unwrapped transcript must not contaminate the capture.
    Blake3Transcript::new().absorb_inner(b"unrelated");
    {
        let _phase = tracing::info_span!("commit", root = "example-root").entered();
        f2z::transcript_context!("statement.example" => transcript.absorb_slice(b"statement"));
    }
    parent.record("round", 1);
    {
        let _phase = tracing::info_span!("opening", tag = "challenge").entered();
        let _: u64 = f2z::transcript_context!("sumcheck.round_challenge", round = 7 => transcript.get_challenge());
        ATTEMPTS.set(0);
        transcript.get_prime::<Uint<2>, AcceptThird>();
    }
    let expected = transcript.state_digest();
    drop(transcript);
    drop(entered);
    let path = trial.finish().unwrap().unwrap();
    let events: Vec<_> = read_events(&path).unwrap().map(Result::unwrap).collect();
    assert_eq!(events.len(), 3 + 4 + 3 * 2); // slice framing, challenge feedback, 3 candidates
    for event in &events[3..7] {
        let context = event
            .spans
            .iter()
            .find(|s| {
                s.fields
                    .get("purpose")
                    .is_some_and(|p| p == "sumcheck.round_challenge")
            })
            .unwrap();
        assert_eq!(context.fields["round"], 7);
    }
    assert_eq!(events[3].fields.part.as_deref(), Some("random_bytes"));
    assert_eq!(events[5].fields.part.as_deref(), Some("challenge_feedback"));
    assert_eq!(
        events[8].fields.part.as_deref(),
        Some("prime_candidate_feedback")
    );
    for event in &events[..3] {
        assert_eq!(event.spans[0].fields["round"], 0);
        assert_eq!(event.spans[1].name, "commit");
        assert_eq!(event.spans[1].fields["root"], "example-root");
    }
    for event in &events[3..] {
        assert_eq!(event.spans[0].fields["round"], 1);
        assert_eq!(event.spans[1].name, "opening");
        assert_eq!(event.spans[1].fields["tag"], "challenge");
    }
    assert_eq!(events[0].fields.bytes().unwrap(), [0x06]);
    assert_eq!(events[2].fields.bytes().unwrap(), [0x07]);
    assert_eq!(events[4].fields.bytes().unwrap(), [0x12]);
    assert_eq!(events[6].fields.bytes().unwrap(), [0x34]);
    assert_eq!(*replay(&path)["prover"].finalize().as_bytes(), expected);
}

#[test]
fn disabled_capture_and_file_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    let logs = TranscriptLogs::new(false);
    let _guard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(&path).unwrap();
    let mut wrapped = trial.wrap("prover", Blake3Transcript::new());
    let mut bare = Blake3Transcript::new();
    wrapped.absorb_slice(b"statement");
    bare.absorb_slice(b"statement");
    assert_eq!(wrapped.get_challenge::<u64>(), bare.get_challenge::<u64>());
    assert_eq!(wrapped.state_digest(), bare.state_digest());
    drop(wrapped);
    assert_eq!(trial.finish().unwrap(), None);
    assert!(!path.exists());

    let logs = TranscriptLogs::new(true);
    let trial = logs.begin_trial(&path).unwrap();
    assert!(logs.begin_trial(dir.path().join("overlap")).is_err());
    assert!(!dir.path().join("overlap").exists());
    assert_eq!(trial.finish().unwrap(), Some(path.canonicalize().unwrap()));
    assert!(logs.begin_trial(&path).is_err());
    assert!(logs.begin_trial(dir.path().join("missing/trace")).is_err());
    let next = logs.begin_trial(dir.path().join("next.jsonl")).unwrap();
    next.finish().unwrap();
    std::fs::write(&path, "{\"target\":").unwrap();
    assert!(
        read_events(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("line 1")
    );
}

#[test]
fn u32_mul_2p15_logged_proof_matches_existing_pins() {
    use f2z::piop::spartan::{
        Lambda100, PreparedU32MulRelation, U32MulF2zWidth, U32MulWitness, commit_u32_mul_witness,
        prove_u32_mul, verify_u32_mul,
    };
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _guard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("trace.jsonl")).unwrap();
    let witness = U32MulWitness::from_fn_with_f2z_width(1 << 15, U32MulF2zWidth::W1, |i| {
        (
            (i as u32).wrapping_mul(0x9e37_79b9) | 1,
            (i as u32).wrapping_mul(0x85eb_ca6b) | 1,
        )
    })
    .unwrap();
    let prepared =
        PreparedU32MulRelation::new_with_profile::<Lambda100>(*witness.layout()).unwrap();
    let hint = commit_u32_mul_witness(&prepared, witness.f2z_bit_rows()).unwrap();
    let mut prover = trial.wrap("prover", Blake3Transcript::new());
    let proof = prove_u32_mul(&mut prover, &prepared, &witness, &hint).unwrap();
    let mut verifier = trial.wrap("verifier", Blake3Transcript::new());
    verify_u32_mul(&mut verifier, &prepared, &hint.commitment, &proof).unwrap();
    let expected = [
        (
            "prover",
            prover.state_digest(),
            "3713fa94ff52d93283e951098919aa88c97acfd3b37181ff68c165b430600fc8",
        ),
        (
            "verifier",
            verifier.state_digest(),
            "17a12aa10604f74018b0d8978283849a62a349166fae7895d1f5ed58888aef03",
        ),
    ];
    drop(prover);
    drop(verifier);
    let path = trial.finish().unwrap().unwrap();
    // Every real protocol event carries a semantic name, in both roles.
    // These checks catch missing call-site scopes, even when byte replay passes.
    let mut meanings = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    let mut commitment_payloads = 0;
    for event in read_events(&path).unwrap() {
        let event = event.unwrap();
        let role = event
            .spans
            .iter()
            .find(|s| s.name == "transcript_operation")
            .unwrap()
            .fields["role"]
            .as_str()
            .unwrap();
        let purposes: Vec<_> = event
            .spans
            .iter()
            .filter_map(|s| s.fields.get("purpose").and_then(|p| p.as_str()))
            .collect();
        assert!(!purposes.is_empty(), "unlabelled protocol event: {event:?}");
        if purposes.contains(&"sumcheck.round_challenge") {
            assert!(
                event.spans.iter().any(|s| s.fields.contains_key("round")),
                "sumcheck challenge without a round: {event:?}"
            );
        }
        if purposes.contains(&"statement.commitment_root") && event.fields.byte_len == 32 {
            assert_eq!(event.fields.bytes().unwrap(), hint.commitment.root);
            commitment_payloads += 1;
        }
        meanings
            .entry(role.to_owned())
            .or_default()
            .extend(purposes.into_iter().map(str::to_owned));
    }
    assert_eq!(commitment_payloads, 2);
    for role in ["prover", "verifier"] {
        for purpose in [
            "statement.assignment_binding_digest",
            "statement.commitment_root",
            "statement.opening_claim_digest",
            "projection_prime.candidate",
            "projection_prime.miller_rabin_base",
            "sumcheck.round_polynomial",
            "sumcheck.round_challenge",
            "gkr.product_tree_roots",
            "gkr.layer",
            "ring_switch.evaluation_point",
            "ligerito.commitment_root",
            "ligerito.fold_challenge",
            "ligerito.query_position_candidate",
            "ligerito.ood_point",
            "grinding.seed_challenge",
            "grinding.nonce",
        ] {
            assert!(
                meanings[role].contains(purpose),
                "missing {role} context {purpose}"
            );
        }
    }
    let replayed = replay(&path);
    for (role, digest, pin) in expected {
        assert_eq!(*replayed[role].finalize().as_bytes(), digest);
        assert_eq!(blake3::Hash::from(digest).to_hex().as_str(), pin);
    }
    let mut pin = blake3::Hasher::new();
    for part in [&hint.commitment.root[..], &proof.f2z().to_bytes()] {
        pin.update(&(part.len() as u64).to_le_bytes());
        pin.update(part);
    }
    assert_eq!(
        pin.finalize().to_hex().as_str(),
        "4bc343a54ad6ec4df4c915ef80df11d6120d046098b8485b1aa03f8c0d9b385a"
    );
}

#[test]
fn logical_records_preserve_values_and_reader_round_trips() {
    use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_monty::F128};
    use f2z::{
        piop::spartan::{SpartanField, transcript_messages::SpartanRoundPolynomial},
        transcript::{
            logging::{self, Event},
            messages::TranscriptField,
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _guard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("logical.jsonl")).unwrap();
    let mut t = trial.wrap("prover", Blake3Transcript::new());
    let cfg = F128::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap();
    let coefficients = [1u128, 2, 3, 4].map(|x| F128::from_with_cfg(x, &cfg));
    let polynomial = SpartanRoundPolynomial(&coefficients);
    let context = f2z::transcript_context!("sumcheck.round_polynomial", round = 3);
    t.absorb_frame(&polynomial);
    drop(context);
    // This real sampler's feedback must remain nested in the squeeze row.
    let expected = f2z::transcript_context!("sumcheck.round_challenge", round = 3 =>
        t.get_field_challenge_and_absorb::<F128>(&cfg, &mut [0; 16]));
    ATTEMPTS.set(0);
    let prime = t.get_prime::<Uint<2>, AcceptThird>();
    let digest = t.state_digest();
    drop(t);
    let path = trial.finish().unwrap().unwrap();
    let events: Vec<_> = logging::read_events(&path)
        .unwrap()
        .map(|e| match e.unwrap() {
            Event::Logical(e) => e,
            _ => panic!("expected logical record"),
        })
        .collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].wire.len(), 3);
    assert_eq!(events[0].context["round"], 3);
    assert_eq!(
        events[0].value["coefficients_hex"],
        serde_json::json!(
            coefficients
                .iter()
                .map(|v| logging_hex(&v.canonical_element_encoding()))
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(events[1].value, expected.log_value());
    assert_eq!(events[1].draw_count, 1);
    assert_eq!(events[1].wire.len(), 10); // draw + raw feedback + legacy field feedback
    assert_eq!(events[2].draw_count, 3);
    assert_eq!(
        events[2].value,
        f2z::transcript::messages::transcribed_value(&prime)
    );
    assert_eq!(*replay(&path)["prover"].finalize().as_bytes(), digest);

    let text = logging::render_text(&path, dir.path().join("transcript.txt")).unwrap();
    let rendered = std::fs::read_to_string(&text).unwrap();
    assert!(rendered.contains("coefficients_hex"));
    assert!(rendered.contains(expected.log_value()["value_hex"].as_str().unwrap()));
    assert!(logging::render_text(&path, &text).is_err());
    // Human-only descriptions can expand an object without changing the
    // capture or losing the actual absorbed value above the description.
    let capture_before = std::fs::read(&path).unwrap();
    let annotation = serde_json::json!({"meaning": "round polynomial", "round": 3});
    let annotated = logging::render_text_with_annotations(
        &path,
        dir.path().join("annotated.txt"),
        |event| {
            (event.purpose == events[0].purpose && event.value == events[0].value)
                .then_some(("Underlying object:", &annotation))
        },
    )
    .unwrap();
    let annotated = std::fs::read_to_string(annotated).unwrap();
    assert_eq!(annotated.matches("Underlying object:").count(), 1);
    assert!(annotated.contains(&serde_json::to_string_pretty(&annotation).unwrap()));
    assert!(annotated.contains(&serde_json::to_string_pretty(&events[0].value).unwrap()));
    assert_eq!(std::fs::read(&path).unwrap(), capture_before);
    let legacy = dir.path().join("legacy.jsonl");
    let lines: Vec<_> = events
        .iter()
        .flat_map(|e| &e.wire)
        .map(|w| serde_json::to_string(w).unwrap())
        .collect();
    std::fs::write(&legacy, lines.join("\n") + "\n").unwrap();
    assert_eq!(*replay(&legacy)["prover"].finalize().as_bytes(), digest);

    let malformed = dir.path().join("malformed.jsonl");
    let mut bad = events[0].clone();
    bad.wire[0].fields.bytes_hex = "zz".into();
    std::fs::write(&malformed, serde_json::to_string(&bad).unwrap()).unwrap();
    assert!(
        logging::read_events(&malformed)
            .unwrap()
            .next()
            .unwrap()
            .is_err()
    );
    bad = events[0].clone();
    bad.schema_version = 99;
    std::fs::write(&malformed, serde_json::to_string(&bad).unwrap()).unwrap();
    assert!(
        logging::read_events(&malformed)
            .unwrap()
            .next()
            .unwrap()
            .is_err()
    );
}

fn logging_hex(bytes: &[u8]) -> String {
    f2z::transcript::messages::numeric_hex(bytes)
}

#[test]
fn empty_frame_is_still_one_complete_logical_record() {
    use f2z::poly::univariate::binary_gf128::BinaryFieldGF128;
    use f2z::transcript::{
        logging::{Event, read_events},
        messages::LegacyFieldValues,
    };
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _guard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("empty.jsonl")).unwrap();
    let mut t = trial.wrap("prover", Blake3Transcript::new());
    let before = t.state_digest();
    t.absorb_frame(&LegacyFieldValues::<BinaryFieldGF128>(&[]));
    assert_eq!(before, t.state_digest());
    drop(t);
    let path = trial.finish().unwrap().unwrap();
    let events = read_events(path)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(events.len(), 1);
    let Event::Logical(e) = &events[0] else {
        panic!("expected logical event")
    };
    assert!(e.complete);
    assert_eq!(e.role, "prover");
    assert!(e.wire.is_empty());
    assert_eq!(e.value["values"], serde_json::json!([]));
}
