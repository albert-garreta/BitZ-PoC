use bitz::observability::{self as metrics, Recording};
use std::{
    io::{self, Write},
    sync::{Arc, Barrier},
};
use tracing_subscriber::prelude::*;

#[test]
fn nested_trials_reentry_parallelism_and_component_updates() {
    let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(metrics::layer()));
    tracing::dispatcher::with_default(&dispatch, || {
        let outer = Recording::start().unwrap();
        let tuning = tracing::info_span!("tuning").entered();
        for _ in 0..2 {
            let recording = Recording::start().unwrap();
            tracing::info_span!("trial").in_scope(|| {
                let operation = tracing::info_span!("operation", component = tracing::field::Empty);
                operation.record("component", "test.operation");
                operation.in_scope(|| operation.in_scope(|| std::hint::black_box(1)));
                let barrier = Arc::new(Barrier::new(2));
                std::thread::scope(|scope| {
                    for _ in 0..2 {
                        let barrier = Arc::clone(&barrier);
                        let operation = &operation;
                        let dispatch = &dispatch;
                        scope.spawn(move || {
                            tracing::dispatcher::with_default(dispatch, || {
                                operation.in_scope(|| {
                                    barrier.wait();
                                });
                            })
                        });
                    }
                });
                let _unused = tracing::info_span!("never_entered");
            });
            let intervals = recording.intervals().unwrap();
            assert_eq!(intervals.len(), 5);
            let trial = metrics::span(&intervals, "trial").unwrap();
            assert_eq!((trial.parent, trial.depth), (None, 0));
            let operations: Vec<_> = intervals.iter().filter(|i| i.name == "operation").collect();
            assert_eq!(operations.len(), 4);
            assert!(
                operations
                    .iter()
                    .all(|i| i.component.as_deref() == Some("test.operation"))
            );
            let tracks: std::collections::HashSet<_> =
                operations.iter().map(|i| i.track_id).collect();
            assert_eq!(tracks.len(), 3);
            let work: u64 = operations.iter().map(|i| i.end_ns - i.start_ns).sum();
            let union = metrics::totals(&intervals)
                .into_iter()
                .find(|(name, _)| name == "test.operation")
                .unwrap()
                .1;
            assert!(union > 0.0 && union < work as f64 / 1e9);
            for i in &intervals {
                if let Some(parent) = i.parent {
                    let parent = intervals.iter().find(|p| p.id == parent).unwrap();
                    assert_eq!(i.track_id, parent.track_id);
                    assert!(parent.start_ns <= i.start_ns && parent.end_ns >= i.end_ns);
                    assert_eq!(i.depth, parent.depth + 1);
                }
            }
        }
        drop(tuning);
        let intervals = outer.intervals().unwrap();
        assert_eq!(intervals.iter().filter(|i| i.name == "trial").count(), 2);
        assert!(metrics::duration(&intervals, "tuning").unwrap().as_nanos() > 0);
    });
}

#[test]
fn empty_incomplete_and_misnested_captures_fail_and_next_trial_recovers() {
    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(metrics::layer()),
        || {
            assert!(
                Recording::start()
                    .unwrap()
                    .intervals()
                    .unwrap_err()
                    .to_string()
                    .contains("empty")
            );
            let recording = Recording::start().unwrap();
            let span = tracing::info_span!("unfinished").entered();
            assert!(
                recording
                    .intervals()
                    .unwrap_err()
                    .to_string()
                    .contains("incomplete")
            );
            drop(span);
            let recording = Recording::start().unwrap();
            let outer = tracing::info_span!("outer").entered();
            let inner = tracing::info_span!("inner").entered();
            drop(outer);
            drop(inner);
            assert!(recording.intervals().is_err());
            for _ in 0..2 {
                let recording = Recording::start().unwrap();
                tracing::info_span!("valid").in_scope(|| std::hint::black_box(42));
                assert_eq!(recording.intervals().unwrap().len(), 1);
            }
        },
    );
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        assert!(Recording::start().is_err());
    });
}

#[test]
fn overflow_is_explicit_and_capacity_resets_per_trial() {
    let capacity = std::mem::size_of::<metrics::Interval>() + "work".len();
    let subscriber =
        tracing_subscriber::registry().with(metrics::SpanMetricsLayer::with_capacity(capacity));
    tracing::subscriber::with_default(subscriber, || {
        let recording = Recording::start().unwrap();
        for _ in 0..2 {
            tracing::info_span!("work").in_scope(|| std::hint::black_box(1));
        }
        assert!(
            recording
                .intervals()
                .unwrap_err()
                .to_string()
                .contains("overflow")
        );
        for _ in 0..2 {
            let recording = Recording::start().unwrap();
            tracing::info_span!("work").in_scope(|| std::hint::black_box(1));
            assert_eq!(recording.intervals().unwrap().len(), 1);
        }
    });
}

#[test]
fn independent_subscribers_do_not_share_intervals() {
    let a = tracing::Dispatch::new(tracing_subscriber::registry().with(metrics::layer()));
    let b = tracing::Dispatch::new(tracing_subscriber::registry().with(metrics::layer()));
    let recording_a = tracing::dispatcher::with_default(&a, || Recording::start().unwrap());
    tracing::dispatcher::with_default(&b, || {
        let recording_b = Recording::start().unwrap();
        tracing::info_span!("b").in_scope(|| std::hint::black_box(1));
        assert_eq!(recording_b.intervals().unwrap()[0].name, "b");
    });
    tracing::dispatcher::with_default(&a, || {
        tracing::info_span!("a").in_scope(|| std::hint::black_box(1))
    });
    assert_eq!(recording_a.intervals().unwrap()[0].name, "a");
}

#[test]
fn typed_duration_queries_reject_ambiguity_and_union_parallel_occurrences() {
    let interval = |name: &str, start_ns, end_ns| metrics::Interval {
        id: 0,
        parent: None,
        track_id: 0,
        depth: 0,
        name: name.into(),
        component: None,
        start_ns,
        end_ns,
    };
    let spans = vec![
        interval("trial", 0, 100),
        interval("work", 10, 30),
        interval("work", 20, 40),
        interval("work", 60, 70),
    ];
    assert_eq!(metrics::duration(&spans, "trial").unwrap().as_nanos(), 100);
    assert!(metrics::duration(&spans, "work").is_err());
    assert!(metrics::duration(&spans, "absent").is_err());
    assert_eq!(
        metrics::totals(&spans),
        vec![("trial".into(), 100.0 / 1e9), ("work".into(), 40.0 / 1e9)]
    );
}

#[test]
fn measured_operation_returns_value_and_exact_span_duration() {
    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(metrics::layer()),
        || {
            let (result, elapsed) = metrics::measure(tracing::info_span!("setup"), || {
                tracing::info_span!("child").in_scope(|| std::hint::black_box(42))
            })
            .unwrap();
            assert_eq!(result, 42);
            assert!(!elapsed.is_zero());
        },
    );
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        assert!(
            metrics::measure(tracing::info_span!("disabled"), || panic!(
                "must not execute"
            ))
            .is_err()
        );
    });
}

#[test]
fn shared_protocol_spans_preserve_relation_labels_and_parentage() {
    let u32_scopes = bitz::protocol_scopes!("u32-spartan-bitz");
    let u64_scopes = bitz::protocol_scopes!("u64-spartan-bitz");
    let intervals = tracing::subscriber::with_default(
        tracing_subscriber::registry().with(metrics::layer()),
        || {
            let recording = Recording::start().unwrap();
            tracing::info_span!("step3:piop_prove").in_scope(|| {
                (u32_scopes.spartan_prove)().in_scope(|| std::hint::black_box(32));
                (u64_scopes.spartan_prove)().in_scope(|| std::hint::black_box(64));
            });
            recording.intervals().unwrap()
        },
    );
    let parent = metrics::span(&intervals, "step3:piop_prove").unwrap();
    for label in [
        "u32-spartan-bitz:spartan_prove",
        "u64-spartan-bitz:spartan_prove",
    ] {
        let child = metrics::span(&intervals, label).unwrap();
        assert_eq!(child.parent, Some(parent.id));
        assert!(child.start_ns >= parent.start_ns && child.end_ns <= parent.end_ns);
        assert!(child.end_ns > child.start_ns);
    }
}

struct FailingWriter {
    fail_write: bool,
}

#[test]
fn profile_projection_preserves_union_totals_memory_and_output_errors() {
    let interval = |id, parent, depth, name: &str, start_ns, end_ns| metrics::Interval {
        id,
        parent,
        track_id: id,
        depth,
        name: name.into(),
        component: None,
        start_ns,
        end_ns,
    };
    let spans = vec![
        interval(1, None, 0, "trial", 0, 100_000_000),
        interval(2, Some(1), 1, "work", 10_000_000, 40_000_000),
        interval(3, Some(1), 1, "work", 30_000_000, 60_000_000),
        interval(4, Some(2), 2, "child", 20_000_000, 30_000_000),
    ];
    let memory = std::collections::BTreeMap::from([(
        "trial".into(),
        metrics::memory::PeakGrowth {
            bytes: 1048576,
            peak_bytes: 2097152,
        },
    )]);
    let mut output = Vec::new();
    metrics::write_profile(&mut output, "fixture", &spans, Some(&memory)).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("trial: 100.000ms (100.0%) self=50.000ms Δrss=1.0 MiB peak=2.0 MiB"),
        "{output}"
    );
    assert!(
        output.contains("work: 50.000ms (50.0%) n=2 self=40.000ms"),
        "{output}"
    );
    let recursive = vec![
        interval(1, None, 0, "work", 0, 100_000_000),
        interval(2, Some(1), 1, "work", 20_000_000, 60_000_000),
        interval(3, Some(2), 2, "child", 30_000_000, 40_000_000),
    ];
    let mut output = Vec::new();
    metrics::write_profile(&mut output, "recursive", &recursive, None).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("work: 100.000ms (100.0%) n=2 self=90.000ms"),
        "{output}"
    );
    for fail_write in [true, false] {
        let error = metrics::write_profile(FailingWriter { fail_write }, "fixture", &spans, None)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            if fail_write {
                "injected write failure"
            } else {
                "injected flush failure"
            }
        );
    }
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            Err(io::Error::other("injected write failure"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("injected flush failure"))
    }
}
