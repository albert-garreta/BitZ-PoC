use serde_json::json;
use std::time::Instant;

/// Diagnostic wall times; kept out of the normal benchmark's subscriber so
/// span logging cannot bias the matched latency runs. Nested spans overlap.
pub(crate) struct StageTimings;

struct StageTiming {
    start: Instant,
    fields: serde_json::Map<String, serde_json::Value>,
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
                || span.name().starts_with("inner_packed:")
                || span.name().starts_with("inner_overlay:")
                || span.name().starts_with("op:")
                || span.name().starts_with("lig:")
                || span.name().starts_with("wfbitz:")
                || span.name() == "spartan:grinding"
            {
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
        if let Some(span) = ctx.span(&id) {
            if let Some(timing) = span.extensions().get::<StageTiming>() {
                eprintln!(
                    "{}",
                    json!({"event": "stage", "name": span.name(),
                    "parent": span.parent().map(|parent| parent.name()),
                    "span_id": span.id().into_u64(),
                    "ancestor_ids": span.scope().from_root().map(|ancestor| ancestor.id().into_u64()).collect::<Vec<_>>(),
                    "path": span.scope().from_root().map(|ancestor| ancestor.name()).collect::<Vec<_>>(),
                    "fields": timing.fields,
                    "elapsed_ms": timing.start.elapsed().as_secs_f64() * 1000.0})
                );
            }
        }
    }
}
