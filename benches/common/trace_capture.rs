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
    pub(crate) fn install() -> TraceCapture {
        let layer = Self::default();
        let capture = TraceCapture {
            state: Arc::clone(&layer.state),
        };
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))
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
