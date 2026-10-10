use serde_json::json;
use std::{
    io::Write,
    sync::{Mutex, OnceLock},
    time::Instant,
};

/// Diagnostic wall times; kept out of the normal benchmark's subscriber so
/// span logging cannot bias the matched latency runs. Nested spans overlap.
/// Closed spans are buffered and written when a root span closes, so per-span
/// output does not inflate the stages being timed. `start_ms`/`end_ms` share
/// one process-wide epoch, which lets callers attribute overlapping spans.
pub(crate) struct StageTimings;

struct StageTiming {
    start: Instant,
    fields: serde_json::Map<String, serde_json::Value>,
}

struct Record {
    name: &'static str,
    parent: Option<&'static str>,
    span_id: u64,
    ancestor_ids: Vec<u64>,
    path: Vec<&'static str>,
    fields: serde_json::Map<String, serde_json::Value>,
    start: Instant,
    end: Instant,
}

static PENDING: Mutex<Vec<Record>> = Mutex::new(Vec::new());

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn ms(from: Instant, to: Instant) -> f64 {
    to.saturating_duration_since(from).as_secs_f64() * 1000.0
}

impl tracing::field::Visit for StageTiming {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().into(), value.into());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().into(), value.into());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().into(), value.into());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().into(), value.into());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().into(), format!("{value:?}").into());
    }
}

impl<S> tracing_subscriber::Layer<S> for StageTimings
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            if span.name().starts_with("falcon")
                || span.name().starts_with("shared_source:")
                || span.name().starts_with("inner_packed:")
                || span.name().starts_with("inner_overlay:")
                || span.name().starts_with("op:")
                || span.name().starts_with("lig:")
                || span.name().starts_with("wfbitz:")
                || span.name() == "spartan:grinding"
            {
                epoch();
                let mut timing = StageTiming {
                    start: Instant::now(),
                    fields: serde_json::Map::new(),
                };
                attrs.record(&mut timing);
                span.extensions_mut().insert(timing);
            }
        }
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            if let Some(timing) = span.extensions_mut().get_mut::<StageTiming>() {
                values.record(timing);
            }
        }
    }

    fn on_close(&self, id: tracing::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let end = Instant::now();
        let Some(span) = ctx.span(&id) else { return };
        let record = {
            let extensions = span.extensions();
            let Some(timing) = extensions.get::<StageTiming>() else {
                return;
            };
            Record {
                name: span.name(),
                parent: span.parent().map(|parent| parent.name()),
                span_id: span.id().into_u64(),
                ancestor_ids: span
                    .scope()
                    .from_root()
                    .map(|ancestor| ancestor.id().into_u64())
                    .collect(),
                path: span
                    .scope()
                    .from_root()
                    .map(|ancestor| ancestor.name())
                    .collect(),
                fields: timing.fields.clone(),
                start: timing.start,
                end,
            }
        };
        let root = record.parent.is_none();
        let mut pending = PENDING.lock().unwrap_or_else(|poison| poison.into_inner());
        pending.push(record);
        if root {
            let epoch = epoch();
            let mut out = std::io::stderr().lock();
            for r in pending.drain(..) {
                let _ = writeln!(
                    out,
                    "{}",
                    json!({"event": "stage", "name": r.name, "parent": r.parent,
                    "span_id": r.span_id, "ancestor_ids": r.ancestor_ids, "path": r.path,
                    "fields": r.fields, "elapsed_ms": ms(r.start, r.end),
                    "start_ms": ms(epoch, r.start), "end_ms": ms(epoch, r.end)})
                );
            }
        }
    }
}
