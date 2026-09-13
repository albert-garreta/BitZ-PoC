//! Shared monotonic capture of native prover tracing spans.
#![allow(dead_code)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};
use tracing::{Subscriber, field::Visit, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, prelude::*, registry::LookupSpan};

#[derive(Clone, Debug)]
pub(crate) struct CapturedSpan {
    pub(crate) id: u64,
    pub(crate) parent: Option<u64>,
    pub(crate) name: String,
    pub(crate) component: Option<String>,
    pub(crate) start_ns: u64,
    pub(crate) end_ns: u64,
}

#[derive(Default)]
struct CaptureState {
    active: bool,
    epoch: Option<Instant>,
    metadata: HashMap<u64, CapturedMetadata>,
    starts: HashMap<u64, Vec<u64>>,
    completed: Vec<CapturedSpan>,
}

#[derive(Clone, Debug)]
struct CapturedMetadata {
    pub(crate) parent: Option<u64>,
    pub(crate) name: String,
    pub(crate) component: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct CaptureLayer {
    state: Arc<Mutex<CaptureState>>,
}

#[derive(Clone)]
pub(crate) struct TraceCapture {
    state: Arc<Mutex<CaptureState>>,
}

impl CaptureLayer {
    pub(crate) fn capture(&self) -> TraceCapture {
        TraceCapture {
            state: Arc::clone(&self.state),
        }
    }

    pub(crate) fn install() -> TraceCapture {
        let layer = Self::default();
        let capture = layer.capture();
        let subscriber = tracing_subscriber::registry().with(layer);
        #[cfg(feature = "bench-perfetto")]
        let subscriber = subscriber.with(super::common::perfetto::layer());
        tracing::subscriber::set_global_default(subscriber)
            .expect("install Binius interval collector once");
        capture
    }
}

#[derive(Default)]
struct FieldVisitor {
    pub(crate) component: Option<String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "component" {
            self.component = Some(format!("{value:?}").trim_matches('"').to_owned());
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "component" {
            self.component = Some(value.to_owned());
        }
    }
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::span::Id, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        let parent = attrs
            .parent()
            .map(|id| id.clone().into_u64())
            .or_else(|| ctx.current_span().id().map(|id| id.clone().into_u64()));
        let mut state = self.state.lock().expect("capture state lock");
        if state.active {
            state.metadata.insert(
                id.clone().into_u64(),
                CapturedMetadata {
                    parent,
                    name: attrs.metadata().name().to_owned(),
                    component: visitor.component,
                },
            );
        }
    }

    fn on_enter(&self, id: &tracing::span::Id, _ctx: Context<'_, S>) {
        let mut state = self.state.lock().expect("capture state lock");
        if !state.active {
            return;
        }
        let Some(epoch) = state.epoch else {
            return;
        };
        let start_ns = ns_since(epoch);
        state
            .starts
            .entry(id.clone().into_u64())
            .or_default()
            .push(start_ns);
    }

    fn on_exit(&self, id: &tracing::span::Id, _ctx: Context<'_, S>) {
        let mut state = self.state.lock().expect("capture state lock");
        if !state.active {
            return;
        }
        let raw_id = id.clone().into_u64();
        let start_ns = state.starts.get_mut(&raw_id).and_then(Vec::pop);
        let metadata = state.metadata.get(&raw_id).cloned();
        if let (Some(start_ns), Some(metadata), Some(epoch)) = (start_ns, metadata, state.epoch) {
            state.completed.push(CapturedSpan {
                id: raw_id,
                parent: metadata.parent,
                name: metadata.name,
                component: metadata.component,
                start_ns,
                end_ns: ns_since(epoch),
            });
        }
    }
}

impl TraceCapture {
    pub(crate) fn begin(&self) {
        let mut state = self.state.lock().expect("capture state lock");
        state.active = true;
        state.epoch = Some(Instant::now());
        state.metadata.clear();
        state.starts.clear();
        state.completed.clear();
    }

    pub(crate) fn now_ns(&self) -> u64 {
        let state = self.state.lock().expect("capture state lock");
        ns_since(state.epoch.expect("capture has begun"))
    }

    pub(crate) fn finish(&self) -> Vec<CapturedSpan> {
        let mut state = self.state.lock().expect("capture state lock");
        state.active = false;
        let mut spans = std::mem::take(&mut state.completed);
        spans.sort_by_key(|span| (span.start_ns, span.end_ns));
        spans
    }
}

fn ns_since(epoch: Instant) -> u64 {
    u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn required_span<'a>(raw: &'a [CapturedSpan], component: &str) -> &'a CapturedSpan {
    let mut spans = raw
        .iter()
        .filter(|s| s.component.as_deref() == Some(component));
    let span = spans
        .next()
        .unwrap_or_else(|| panic!("missing {component} span"));
    assert!(
        spans.next().is_none(),
        "multiple {component} spans in one trial"
    );
    assert!(span.end_ns >= span.start_ns, "reversed {component} span");
    span
}

/// Completed trial scopes. Keep each operation's own endpoints rather than
/// charging the enclosing scopes' entry/exit overhead to witness or verifier.
pub(crate) struct BiniusLigeritoTrial<'a> {
    pub(crate) verified: &'a CapturedSpan,
    pub(crate) witness_to_proof: &'a CapturedSpan,
    pub(crate) witness: &'a CapturedSpan,
    pub(crate) verification: &'a CapturedSpan,
}

impl<'a> BiniusLigeritoTrial<'a> {
    pub(crate) fn from_spans(raw: &'a [CapturedSpan]) -> Self {
        let verified = required_span(raw, "binius-ligerito.verified-trial");
        let witness_to_proof = required_span(raw, "binius-ligerito.witness-to-proof");
        let witness = required_span(raw, "binius-ligerito.witness-evaluation");
        let verification = required_span(raw, "binius-ligerito.verification");
        assert!(
            verified.start_ns <= witness_to_proof.start_ns
                && witness_to_proof.start_ns <= witness.start_ns
                && witness.end_ns <= witness_to_proof.end_ns
                && witness_to_proof.end_ns <= verification.start_ns
                && verification.end_ns <= verified.end_ns,
            "trial spans are not nested/sequenced correctly"
        );
        Self {
            verified,
            witness_to_proof,
            witness,
            verification,
        }
    }
}

/// Reporting attribution, not another timer: Round 0 is opening work even
/// though it executes inside the PIOP prefix. Later oracle commits remain
/// attributed to PIOP, matching the existing benchmark metric definitions.
pub(crate) struct BiniusLigeritoPhases {
    pub(crate) commit: (u64, u64),
    pub(crate) piop: Vec<(u64, u64)>,
    pub(crate) opening: Vec<(u64, u64)>,
}

impl BiniusLigeritoPhases {
    pub(crate) fn from_spans(raw: &[CapturedSpan]) -> Self {
        let matching = |component| {
            raw.iter()
                .filter(move |s| s.component.as_deref() == Some(component))
        };
        let required = |component| {
            let span = required_span(raw, component);
            (span.start_ns, span.end_ns)
        };
        let prefix = required("binius-ligerito.piop");
        let commit = required("binius-ligerito.witness-commit");
        let final_opening = required("binius-ligerito.opening");
        assert!(
            prefix.1 <= final_opening.0,
            "opening precedes the PIOP prefix end"
        );
        let mut opening: Vec<_> = matching("binius-ligerito.round0")
            .map(|s| (s.start_ns, s.end_ns))
            .collect();
        assert!(!opening.is_empty(), "missing binius-ligerito.round0 span");

        // Subtract the union, preserving gaps and actual placement. In
        // particular, never slide Round 0 past the constraint reductions.
        let mut excluded = opening.clone();
        excluded.push(commit);
        excluded.sort_unstable();
        let mut cursor = prefix.0;
        let mut piop = Vec::new();
        for (start, end) in excluded {
            assert!(
                prefix.0 <= start && start <= end && end <= prefix.1,
                "commit/Round 0 outside the PIOP prefix"
            );
            if cursor < start {
                piop.push((cursor, start));
            }
            cursor = cursor.max(end);
        }
        if cursor < prefix.1 {
            piop.push((cursor, prefix.1));
        }
        opening.push(final_opening);
        opening.sort_unstable();
        Self {
            commit,
            piop,
            opening,
        }
    }
}

#[cfg(test)]
pub(crate) mod phase_tests {
    use super::*;

    fn fixture() -> Vec<CapturedSpan> {
        [
            ("binius-ligerito.piop", 10, 100),
            ("binius-ligerito.witness-commit", 20, 30),
            ("binius-ligerito.round0", 30, 40),
            ("binius-ligerito.oracle-commit", 55, 65),
            ("binius-ligerito.round0", 65, 75),
            ("binius-ligerito.round0", 70, 80),
            ("binius-ligerito.opening", 105, 130),
        ]
        .into_iter()
        .enumerate()
        .map(|(id, (component, start_ns, end_ns))| CapturedSpan {
            id: id as u64,
            parent: None,
            name: component.to_owned(),
            component: Some(component.to_owned()),
            start_ns,
            end_ns,
        })
        .collect()
    }

    pub(crate) fn trial_fixture() -> Vec<CapturedSpan> {
        let mut raw = fixture();
        raw.extend(
            [
                ("binius-ligerito.verified-trial", 0, 160),
                ("binius-ligerito.witness-to-proof", 1, 140),
                ("binius-ligerito.witness-evaluation", 2, 10),
                ("binius-ligerito.verification", 145, 155),
            ]
            .into_iter()
            .enumerate()
            .map(|(id, (component, start_ns, end_ns))| CapturedSpan {
                id: 100 + id as u64,
                parent: None,
                name: component.to_owned(),
                component: Some(component.to_owned()),
                start_ns,
                end_ns,
            }),
        );
        raw
    }

    #[test]
    fn trial_scopes_keep_distinct_endpoints() {
        let raw = trial_fixture();
        let trial = BiniusLigeritoTrial::from_spans(&raw);
        assert_eq!((trial.verified.start_ns, trial.verified.end_ns), (0, 160));
        assert_eq!(
            (
                trial.witness_to_proof.start_ns,
                trial.witness_to_proof.end_ns
            ),
            (1, 140)
        );
        assert_eq!((trial.witness.start_ns, trial.witness.end_ns), (2, 10));
        assert_eq!(
            (trial.verification.start_ns, trial.verification.end_ns),
            (145, 155)
        );
    }

    #[test]
    #[should_panic(expected = "missing binius-ligerito.verification span")]
    fn incomplete_trial_is_rejected() {
        let mut raw = trial_fixture();
        raw.pop();
        BiniusLigeritoTrial::from_spans(&raw);
    }

    #[test]
    #[should_panic(expected = "trial spans are not nested/sequenced correctly")]
    fn verification_cannot_overlap_proof_production() {
        let mut raw = trial_fixture();
        raw.last_mut().unwrap().start_ns = 139;
        BiniusLigeritoTrial::from_spans(&raw);
    }

    #[test]
    fn round0_keeps_its_position_and_is_excluded_once_from_piop() {
        let mut raw = fixture();
        raw.reverse(); // Collection order must not determine the timeline.
        let phases = BiniusLigeritoPhases::from_spans(&raw);
        assert_eq!(phases.commit, (20, 30));
        assert_eq!(phases.piop, [(10, 20), (40, 65), (80, 100)]);
        assert_eq!(phases.opening, [(30, 40), (65, 75), (70, 80), (105, 130)]);
    }

    #[test]
    #[should_panic(expected = "missing binius-ligerito.opening span")]
    fn missing_phase_is_not_zero() {
        let mut raw = fixture();
        raw.pop();
        BiniusLigeritoPhases::from_spans(&raw);
    }

    #[test]
    #[should_panic(expected = "multiple binius-ligerito.piop spans")]
    fn mixed_trials_are_rejected() {
        let mut raw = fixture();
        raw.push(raw[0].clone());
        BiniusLigeritoPhases::from_spans(&raw);
    }

    #[test]
    #[should_panic(expected = "commit/Round 0 outside the PIOP prefix")]
    fn invalid_interval_is_not_clipped() {
        let mut raw = fixture();
        raw[2].end_ns = 200;
        BiniusLigeritoPhases::from_spans(&raw);
    }
}
