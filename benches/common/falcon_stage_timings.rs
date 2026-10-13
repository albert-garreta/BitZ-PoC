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
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        // Aggregate stages avoid the substantial overhead of tracing every round.
        static LIGHT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*LIGHT.get_or_init(|| std::env::var_os("BITZ_FALCON_LIGHT_PROFILE").is_some()) {
            return true;
        }
        // Detailed experiment builds expose per-round diagnostics. Production
        // builds retain the lightweight stage set used by the frozen baseline.
        let detailed = cfg!(feature = "bench-internals")
            && (metadata.name().starts_with("falcon_algebraic:word_")
                || metadata.name().starts_with("falcon_integer:")
                || matches!(metadata.name(), "lig:grind_pow" | "spartan:grinding"));
        detailed
            || metadata.name().starts_with("falcon_factored_ring:")
            || matches!(
                metadata.name(),
                "falcon_algebraic:commit"
                    | "falcon_algebraic:prove"
                    | "falcon_algebraic:verify"
                    | "falcon_algebraic:witness"
                    | "falcon_algebraic:ring"
                    | "falcon_algebraic:arithmetic"
                    | "falcon_algebraic:binding"
                    | "falcon_algebraic:integer_outer"
                    | "falcon_algebraic:binding_construction"
                    | "falcon_algebraic:word_first_fold"
                    | "falcon_algebraic:bitz"
                    | "falcon_algebraic:pcs"
                    | "falcon_algebraic_ring:coordinates"
                    | "falcon_algebraic_ring:coordinate_work"
                    | "falcon_algebraic_ring:carries"
                    | "falcon_algebraic_ring:collapse"
                    | "shared_source:encode"
                    | "shared_source:merkle"
                    | "falcon_bridge:integer_folds"
                    | "falcon_bridge:column_packing"
                    | "falcon_bridge:power_tables"
                    | "falcon_bridge:wfbitz_forest"
                    | "op:initial_messages"
                    | "op:deferred_first_fold"
                    | "op:ring_switch"
                    | "op:extra_bases"
                    | "op:ligerito"
            )
    }

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
