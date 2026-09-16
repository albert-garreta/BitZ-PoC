use crypto_primitives::{
    FromWithConfig, PrimeField, crypto_bigint_monty::F128, crypto_bigint_uint::Uint,
};
use f2z::{
    piop::{
        spartan::{
            SpartanField,
            transcript_messages::{
                OuterTerminalEvaluations, PrimeSamplingParameters, SkipPolynomialMessage,
                SpartanFieldElements, SpartanMessage, SpartanStatement,
            },
        },
        sumcheck::transcript_messages::SumcheckHeader,
    },
    transcript::{
        Blake3Transcript,
        logging::{self, Event, Operation, TranscriptLogs},
        messages::{Absorbable, LegacyFieldValues},
        traits::Transcript,
    },
};
use serde_json::json;
use tracing_subscriber::prelude::*;

fn chunks(object: &impl Absorbable) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    object.visit_chunks(&mut |bytes| chunks.push(bytes.to_vec()));
    chunks
}

#[test]
fn composite_statements_preserve_frames_and_make_one_row() {
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("objects.jsonl")).unwrap();
    let mut transcript = trial.wrap("prover", Blake3Transcript::new());
    let mut baseline = Blake3Transcript::new();
    let span = tracing::info_span!("caller", stage = "statement", round = 0);
    let _entered = span.enter();
    let mut expected = Vec::new();
    for skip in [None, Some(3)] {
        let modulus = 65537u128.to_le_bytes();
        let statement = SpartanStatement {
            protocol: b"protocol",
            modulus: &modulus,
            matrix_digest: &[1; 32],
            assignment_binding: &[2; 32],
            skip_variables: skip,
        };
        let mut legacy = Vec::new();
        for (tag, payload) in [
            (&b"protocol"[..], &b"protocol"[..]),
            (&b"field-modulus"[..], &modulus[..]),
            (&b"matrix-statement"[..], &[1; 32][..]),
            (&b"f2z/spartan/assignment-oracle/v1"[..], &[2; 32][..]),
        ] {
            legacy.extend(chunks(&SpartanMessage { tag, payload }));
        }
        if let Some(value) = skip {
            legacy.extend(chunks(&SpartanMessage {
                tag: b"univariate-skip-vars",
                payload: &[value],
            }));
        }
        assert_eq!(chunks(&statement), legacy);
        for bytes in &legacy {
            baseline.absorb_inner(bytes);
        }
        transcript.absorb(&statement);
        expected.push(legacy);
        span.record("round", 1);
    }
    let params = PrimeSamplingParameters {
        domain: b"prime",
        min: 1 << 98,
        max: (1 << 99) - 1,
    };
    let legacy: Vec<_> = [
        SpartanMessage {
            tag: b"prime-domain",
            payload: params.domain,
        },
        SpartanMessage {
            tag: b"prime-min",
            payload: &params.min.to_le_bytes(),
        },
        SpartanMessage {
            tag: b"prime-max",
            payload: &params.max.to_le_bytes(),
        },
    ]
    .iter()
    .flat_map(chunks)
    .collect();
    assert_eq!(chunks(&params), legacy);
    for bytes in &legacy {
        baseline.absorb_inner(bytes);
    }
    transcript.absorb(&params);
    expected.push(legacy);
    assert_eq!(
        transcript.get_challenge::<u64>(),
        baseline.get_challenge::<u64>()
    );
    transcript.absorb(&SpartanMessage {
        tag: b"prime-q",
        payload: &65537u128.to_le_bytes(),
    });
    drop(transcript);
    let path = trial.finish().unwrap().unwrap();
    let records: Vec<_> = logging::read_events(&path)
        .unwrap()
        .map(|event| match event.unwrap() {
            Event::Logical(e) => e,
            _ => panic!("expected logical record"),
        })
        .collect();
    assert_eq!(records.len(), 5);
    for (record, expected) in records.iter().zip(expected) {
        assert!(record.complete);
        assert_eq!(record.role, "prover");
        assert_eq!(record.context["stage"], "statement");
        assert_eq!(
            record
                .wire
                .iter()
                .map(|w| w.fields.bytes().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(records[0].purpose, "spartan.statement");
    assert_eq!(records[0].context["round"], 0);
    assert_eq!(records[1].context["round"], 1);
    assert!(records[0].value.get("skip_variables").is_none());
    assert_eq!(records[1].value["skip_variables"], 3);
    assert_eq!(
        records[1].value["modulus_hex"],
        "0x00000000000000000000000000010001"
    );
    assert_eq!(records[2].purpose, "projection_prime.parameters");
    assert_eq!(records[3].operation, Operation::Squeeze);
    assert_eq!(records[4].value["tag"], "prime-q");
}

#[test]
fn headers_and_named_field_values_keep_their_original_encodings() {
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("headers.jsonl")).unwrap();
    let mut transcript = trial.wrap("prover", Blake3Transcript::new());
    let cfg = F128::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap();
    for (variables, degrees) in [(0, vec![3]), (7, vec![2, 3, 5])] {
        let header = SumcheckHeader::<F128>::grouped(variables, &degrees, &cfg);
        let values: Vec<_> = std::iter::once(variables)
            .chain(std::iter::once(degrees.len()))
            .chain(degrees.iter().copied())
            .map(|x| F128::from_with_cfg(x as u64, &cfg))
            .collect();
        assert_eq!(chunks(&header), chunks(&LegacyFieldValues(&values)));
        assert_eq!(
            header.log_value(),
            json!({"variables": variables, "group_count": degrees.len(), "degree_bounds": degrees})
        );
        transcript.absorb(&header);
    }
    let single = SumcheckHeader::<F128>::single(7, 3, &cfg);
    let fields = [7u64, 3].map(|x| F128::from_with_cfg(x, &cfg));
    assert_eq!(chunks(&single), chunks(&LegacyFieldValues(&fields)));
    assert_eq!(
        single.log_value(),
        json!({"variables": 7, "degree_bound": 3})
    );
    transcript.absorb(&single);
    drop(transcript);
    let path = trial.finish().unwrap().unwrap();
    let records: Vec<_> = logging::read_events(path)
        .unwrap()
        .map(|event| match event.unwrap() {
            Event::Logical(e) => e,
            _ => panic!("expected logical header"),
        })
        .collect();
    assert_eq!(records.len(), 3);
    for (record, fields) in records.iter().zip([3, 5, 2]) {
        assert_eq!(record.purpose, "sumcheck.header");
        assert_eq!(record.operation, Operation::Absorb);
        assert_eq!(record.wire.len(), fields * 6);
        assert!(record.complete);
    }
    assert_eq!(records[1].value["degree_bounds"], json!([2, 3, 5]));
    assert_eq!(records[2].value, single.log_value());

    let values = [11u128, 22, 33].map(|x| F128::from_with_cfg(x, &cfg));
    let terminal = OuterTerminalEvaluations(&values);
    assert_eq!(chunks(&terminal), chunks(&SpartanFieldElements(&values)));
    for (name, expected) in ["az_hex", "bz_hex", "cz_hex"]
        .into_iter()
        .zip([11u128, 22, 33])
    {
        assert_eq!(terminal.log_value()[name], format!("0x{expected:032x}"));
    }

    for skip_variables in 1..=4 {
        let block = 1i32 << skip_variables;
        let points: Vec<_> = (0..block / 2 - 1)
            .flat_map(|i| [-1 - i, block + i])
            .collect();
        let values: Vec<_> = (1..=points.len() + 1)
            .map(|x| F128::from_with_cfg(x as u64, &cfg))
            .collect();
        let message = SkipPolynomialMessage {
            skip_variables,
            points: &points,
            values: &values,
        };
        assert_eq!(chunks(&message), chunks(&SpartanFieldElements(&values)));
        let display = message.log_value();
        assert_eq!(display["leading_coefficient"]["degree"], 2 * (block - 1));
        let evaluations = display["evaluations"].as_array().unwrap();
        assert_eq!(evaluations.len(), points.len());
        for ((entry, point), value) in evaluations.iter().zip(&points).zip(&values) {
            assert_eq!(entry["point"], *point);
            assert_eq!(
                entry["value_hex"],
                f2z::transcript::messages::numeric_hex(&value.canonical_element_encoding())
            );
        }
    }
}

#[test]
fn disabled_composite_logging_never_constructs_diagnostic_values() {
    struct NoDisplay<'a>(PrimeSamplingParameters<'a>);
    impl Absorbable for NoDisplay<'_> {
        fn kind(&self) -> &'static str {
            self.0.kind()
        }
        fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
            self.0.visit_chunks(emit);
        }
        fn log_value(&self) -> serde_json::Value {
            panic!("disabled diagnostic evaluated")
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("disabled.jsonl");
    let logs = TranscriptLogs::new(false);
    let trial = logs.begin_trial(&path).unwrap();
    let mut transcript = trial.wrap("prover", Blake3Transcript::new());
    let object = NoDisplay(PrimeSamplingParameters {
        domain: b"prime",
        min: 3,
        max: 65537,
    });
    transcript.absorb(&object);
    let mut baseline = Blake3Transcript::new();
    baseline.absorb(&object.0);
    assert_eq!(
        transcript.get_challenge::<u64>(),
        baseline.get_challenge::<u64>()
    );
    drop(transcript);
    assert_eq!(trial.finish().unwrap(), None);
    assert!(!path.exists());
}

#[test]
fn human_rendering_keeps_complete_nested_values_and_legacy_records() {
    let dir = tempfile::tempdir().unwrap();
    let logs = TranscriptLogs::new(true);
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
    let trial = logs.begin_trial(dir.path().join("nested.jsonl")).unwrap();
    let mut transcript = trial.wrap("verifier", Blake3Transcript::new());
    let value = json!({"configuration": {"counts": [183, 90, 60]}, "evaluations": [
        {"point": -1, "value_hex": "0x000000000000000000000000000000ff"},
        {"point": 8, "value_hex": "0xffffffffffffffffffffffffffffffff"}],
        "empty": [], "escaped": "line\nbreak"});
    transcript.absorb(&f2z::transcript::messages::DescribedFrame {
        bytes: b"complete",
        kind: "nested",
        value: || value.clone(),
    });
    let _: u64 = transcript.get_challenge();
    drop(transcript);
    let path = trial.finish().unwrap().unwrap();
    let text_path = logging::render_text(&path, dir.path().join("nested.txt")).unwrap();
    let text = std::fs::read_to_string(text_path).unwrap();
    assert!(text.contains("ABSORB nested\n"));
    assert!(text.contains("configuration:\n    counts: [183,90,60]"));
    assert!(text.contains("  evaluations:\n    -\n      point: -1"));
    for row in value["evaluations"].as_array().unwrap() {
        assert!(text.contains(row["value_hex"].as_str().unwrap()));
    }
    assert!(text.contains("empty: []"));
    assert!(text.contains("escaped: \"line\\nbreak\""));
    assert_eq!(text.matches("draws:").count(), 1);
    let legacy = dir.path().join("legacy.jsonl");
    let bytes: Vec<_> = logging::read_byte_events(&path)
        .unwrap()
        .map(|e| serde_json::to_string(&e.unwrap()).unwrap())
        .collect();
    std::fs::write(&legacy, bytes.join("\n") + "\n").unwrap();
    let text = logging::render_text(&legacy, dir.path().join("legacy.txt")).unwrap();
    assert!(std::fs::read_to_string(text).unwrap().contains("bytes:"));
}
