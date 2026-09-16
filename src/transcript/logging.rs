//! Streaming Fiat–Shamir byte traces. Compose `TranscriptLogs::layer` into the
//! application's subscriber once, then explicitly finish each trial to check I/O.

use super::{
    Blake3Transcript,
    traits::{ConstTranscribable, Transcript},
};
use crate::utils::primality::PrimalityTest;
use crypto_primitives::ConstIntSemiring;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};
use tracing_subscriber::{
    Layer,
    filter::filter_fn,
    fmt::MakeWriter,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
};

pub const TARGET: &str = "f2z::transcript";

/// Describe the protocol meaning at the call site. This span is diagnostic:
/// neither the purpose nor its fields are absorbed into the transcript.
#[macro_export]
macro_rules! transcript_context {
    ($purpose:expr $(, $field:ident = $value:expr)* => $body:expr) => {{
        let _context = $crate::transcript_context!($purpose $(, $field = $value)*);
        $body
    }};
    ($purpose:expr $(, $field:ident = $value:expr)* $(,)?) => {
        tracing::trace_span!(target: "f2z::transcript", "transcript_context",
            purpose = $purpose, $($field = $value,)*
        ).entered()
    };
}

/// Lowercase hex without an intermediate allocation when formatting an event.
pub struct Hex<'a>(pub &'a [u8]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub(super) fn record_bytes(operation: &'static str, part: &'static str, bytes: &[u8]) {
    tracing::trace!(target: "f2z::transcript", operation, part, byte_len = bytes.len(), bytes_hex = %Hex(bytes));
}

#[derive(Clone)]
pub struct TranscriptLogs {
    enabled: bool,
    state: Arc<Mutex<Option<Sink>>>,
    active: Arc<AtomicBool>,
}

struct Sink {
    writer: BufWriter<Box<dyn Write + Send>>,
    error: Option<io::Error>,
    sequence: u64,
}

impl TranscriptLogs {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            state: Arc::new(Mutex::new(None)),
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// All enclosing spans are retained; only byte events inside a wrapped
    /// operation are written. A global event-only filter would lose the spans.
    pub fn layer<S>(&self) -> impl Layer<S> + use<S>
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    {
        let enabled = self.enabled;
        LogicalLayer(self.clone()).with_filter(filter_fn(move |metadata| {
            enabled && (metadata.is_span() || metadata.target() == TARGET)
        }))
    }

    /// Convenience for an executable without other layers. Profiling executables
    /// compose `layer()` with their profiling layer on the same registry instead.
    pub fn install(&self) -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(self.layer()))
    }

    /// Start one file, rejecting overwrites and overlapping trials. Disabled
    /// capture creates no file. Parent directories must already exist.
    pub fn begin_trial(&self, path: impl AsRef<Path>) -> io::Result<Trial<'_>> {
        if !self.enabled {
            return Ok(Trial {
                logs: self,
                path: None,
            });
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("transcript writer poisoned"))?;
        if state.is_some() {
            return Err(io::Error::other("a transcript trial is already active"));
        }
        let file = File::create_new(path.as_ref())?;
        let path = path.as_ref().canonicalize()?;
        *state = Some(Sink {
            writer: BufWriter::new(Box::new(file)),
            error: None,
            sequence: 0,
        });
        self.active.store(true, Ordering::Relaxed);
        Ok(Trial {
            logs: self,
            path: Some(path),
        })
    }
}

#[must_use = "call finish to flush and report write errors"]
pub struct Trial<'a> {
    logs: &'a TranscriptLogs,
    path: Option<PathBuf>,
}

impl Trial<'_> {
    pub fn wrap<'a>(&'a self, role: &'a str, inner: Blake3Transcript) -> LoggedTranscript<'a> {
        LoggedTranscript {
            inner,
            role,
            trial: self,
        }
    }

    /// Close the stream and return its absolute path only after all writes flush.
    pub fn finish(mut self) -> io::Result<Option<PathBuf>> {
        let path = self.path.take();
        if path.is_some() {
            self.logs.active.store(false, Ordering::Relaxed);
            let mut state = self
                .logs
                .state
                .lock()
                .map_err(|_| io::Error::other("transcript writer poisoned"))?;
            let mut sink = state.take().expect("active trial has a writer");
            let flushed = sink.writer.flush();
            if let Some(error) = sink.error {
                return Err(error);
            }
            flushed?;
        }
        Ok(path)
    }
}

impl Drop for Trial<'_> {
    fn drop(&mut self) {
        // Best effort on unwinding; normal callers must use finish for checked I/O.
        if self.path.is_some() {
            self.logs.active.store(false, Ordering::Relaxed);
            if let Ok(mut state) = self.logs.state.lock() {
                state.take();
            }
        }
    }
}

pub struct LoggedTranscript<'a> {
    inner: Blake3Transcript,
    role: &'a str,
    trial: &'a Trial<'a>,
}

impl LoggedTranscript<'_> {
    pub fn state_digest(&self) -> [u8; 32] {
        self.inner.state_digest()
    }

    fn operation(&self, method: &'static str) -> tracing::Span {
        if self.trial.path.is_none() {
            return tracing::Span::none();
        }
        // Created at the call site, never saved on the transcript: parents and
        // recorded fields must reflect the protocol's current scope.
        tracing::trace_span!(target: "f2z::transcript", "transcript_operation", role = self.role, method)
    }
}

impl Transcript for LoggedTranscript<'_> {
    fn operation_span(&self, method: &'static str) -> tracing::Span {
        self.operation(method)
    }
    fn logging_enabled(&self) -> bool {
        self.trial.path.is_some()
    }

    fn absorb_inner(&mut self, bytes: &[u8]) {
        let logical = LogicalScope::new(self.logging_enabled(), "absorb", "bytes");
        let _span = self.operation("absorb_inner").entered();
        self.inner.absorb_inner(bytes);
        logical.finish(|| serde_json::json!({"bytes_hex": Hex(bytes).to_string()}));
    }
    fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
        let logical = LogicalScope::new(self.logging_enabled(), "squeeze", "challenge.raw");
        let _span = self.operation("get_challenge").entered();
        let value: T = self.inner.get_challenge();
        logical.finish(|| super::messages::transcribed_value(&value));
        value
    }
    fn get_prime<R: ConstIntSemiring + ConstTranscribable, T: PrimalityTest<R>>(&mut self) -> R {
        let logical = LogicalScope::new(self.logging_enabled(), "squeeze", "prime");
        let _span = self.operation("get_prime").entered();
        let prime = self.inner.get_prime::<R, T>();
        logical.finish(|| super::messages::transcribed_value(&prime));
        prime
    }
}

#[doc(hidden)]
pub struct LogWriter<'a>(MutexGuard<'a, Option<Sink>>);
impl<'a> MakeWriter<'a> for TranscriptLogs {
    type Writer = LogWriter<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        LogWriter(self.state.lock().expect("transcript writer poisoned"))
    }
}
impl Write for LogWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(sink) = self.0.as_mut() {
            if sink.error.is_none() {
                if let Err(error) = sink.writer.write_all(bytes) {
                    sink.error = Some(error);
                }
            }
        }
        // fmt cannot return errors to the transcript; finish reports the first.
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(sink) = self.0.as_mut() {
            if let Err(error) = sink.writer.flush() {
                sink.error.get_or_insert(error);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub target: String,
    pub fields: ByteEvent,
    pub spans: Vec<SpanContext>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByteEvent {
    pub operation: Operation,
    /// Backend role of these bytes; semantic meaning lives in context spans.
    /// Missing in captures made before encoding roles were added.
    #[serde(default)]
    pub part: Option<String>,
    pub byte_len: usize,
    pub bytes_hex: String,
}
impl ByteEvent {
    pub fn bytes(&self) -> io::Result<Vec<u8>> {
        if self.bytes_hex.len() / 2 != self.byte_len
            || !self.bytes_hex.len().is_multiple_of(2)
            || !self.bytes_hex.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid transcript byte encoding or length",
            ));
        }
        Ok((0..self.bytes_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&self.bytes_hex[i..i + 2], 16).expect("validated hex"))
            .collect())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Absorb,
    Squeeze,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpanContext {
    pub name: String,
    #[serde(flatten)]
    pub fields: BTreeMap<String, serde_json::Value>,
}

/// A synchronous logical operation. It does not hold or mutate a transcript.
/// Nested operations contribute wire steps to their enclosing operation.
#[must_use]
pub struct LogicalScope(Option<tracing::span::EnteredSpan>);
impl LogicalScope {
    pub fn new(enabled: bool, operation: &'static str, kind: &'static str) -> Self {
        Self(enabled.then(|| {
            tracing::trace_span!(target: TARGET,
            "transcript_logical_operation", operation, kind)
            .entered()
        }))
    }
    pub fn finish(self, value: impl FnOnce() -> serde_json::Value) {
        if self.0.is_some() {
            tracing::trace!(target: TARGET, logical_value = %value());
        }
    }
}

#[derive(Default)]
struct Fields(BTreeMap<String, serde_json::Value>);
impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn fmt::Debug) {
        self.0.insert(f.name().into(), format!("{v:?}").into());
    }
    fn record_str(&mut self, f: &tracing::field::Field, v: &str) {
        self.0.insert(f.name().into(), v.into());
    }
    fn record_u64(&mut self, f: &tracing::field::Field, v: u64) {
        self.0.insert(f.name().into(), v.into());
    }
    fn record_i64(&mut self, f: &tracing::field::Field, v: i64) {
        self.0.insert(f.name().into(), v.into());
    }
    fn record_bool(&mut self, f: &tracing::field::Field, v: bool) {
        self.0.insert(f.name().into(), v.into());
    }
}
#[derive(Default)]
struct Pending {
    wire: Vec<TranscriptEvent>,
    value: Option<serde_json::Value>,
    spans: Vec<SpanContext>,
}
struct LogicalLayer(TranscriptLogs);
impl<S> Layer<S> for LogicalLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("registered span");
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let mut extensions = span.extensions_mut();
        extensions.insert(fields);
        if span.name() == "transcript_logical_operation" && span.metadata().target() == TARGET {
            extensions.insert(Pending::default());
        }
    }
    fn on_record(&self, id: &tracing::Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if let Some(fields) = span.extensions_mut().get_mut::<Fields>() {
                values.record(fields);
            }
        }
    }
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        if !self.0.active.load(Ordering::Relaxed) {
            return;
        }
        let Some(scope) = ctx.event_scope(event) else {
            return;
        };
        let scope: Vec<_> = scope.from_root().collect();
        let logical: Vec<_> = scope
            .iter()
            .filter(|s| {
                s.name() == "transcript_logical_operation" && s.metadata().target() == TARGET
            })
            .collect();
        let Some(root) = logical.first() else {
            return;
        };
        let mut fields = Fields::default();
        event.record(&mut fields);
        if let Some(value) = fields.0.get("logical_value").and_then(|v| v.as_str()) {
            if logical.len() == 1 {
                let mut ext = root.extensions_mut();
                let pending = ext.get_mut::<Pending>().unwrap();
                pending.value = serde_json::from_str(value).ok();
                if pending.spans.is_empty() {
                    let root_index = scope.iter().position(|s| s.id() == root.id()).unwrap();
                    pending.spans = scope[..root_index]
                        .iter()
                        .map(|s| SpanContext {
                            name: s.name().into(),
                            fields: s.extensions().get::<Fields>().unwrap().0.clone(),
                        })
                        .collect();
                }
            }
            return;
        }
        // Only wrapped transcript byte operations belong to this capture.
        if !scope
            .iter()
            .any(|s| s.name() == "transcript_operation" && s.metadata().target() == TARGET)
        {
            return;
        }
        let Ok(bytes) = serde_json::from_value::<ByteEvent>(serde_json::json!(fields.0)) else {
            return;
        };
        let spans: Vec<_> = scope
            .iter()
            .filter(|s| s.name() != "transcript_logical_operation")
            .map(|s| SpanContext {
                name: s.name().into(),
                fields: s.extensions().get::<Fields>().unwrap().0.clone(),
            })
            .collect();
        let mut ext = root.extensions_mut();
        let pending = ext.get_mut::<Pending>().unwrap();
        if pending.wire.is_empty() {
            // Snapshot the caller at the logical boundary. Per-byte contexts
            // below additionally retain changing retry/encoding spans.
            let root_index = scope.iter().position(|s| s.id() == root.id()).unwrap();
            pending.spans = scope[..root_index]
                .iter()
                .filter(|s| s.name() != "transcript_logical_operation")
                .map(|s| SpanContext {
                    name: s.name().into(),
                    fields: s.extensions().get::<Fields>().unwrap().0.clone(),
                })
                .collect();
            if let Some(identity) = spans
                .iter()
                .rev()
                .find(|s| s.name == "transcript_operation")
            {
                pending.spans.push(identity.clone());
            }
        }
        pending.wire.push(TranscriptEvent {
            target: TARGET.into(),
            fields: bytes,
            spans,
        });
    }
    fn on_close(&self, id: tracing::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let mut ext = span.extensions_mut();
        let Some(pending) = ext.remove::<Pending>() else {
            return;
        };
        if pending.wire.is_empty() && pending.value.is_none() {
            return;
        }
        let fields = &ext.get_mut::<Fields>().unwrap().0;
        let operation: Operation = serde_json::from_value(fields["operation"].clone()).unwrap();
        let kind = fields["kind"].as_str().unwrap();
        let role = pending
            .spans
            .iter()
            .rev()
            .find_map(|s| s.fields.get("role").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_owned();
        // The innermost semantic caller is the purpose; encoder plumbing is not.
        let purpose = pending
            .spans
            .iter()
            .rev()
            .filter_map(|s| s.fields.get("purpose").and_then(|v| v.as_str()))
            .find(|p| {
                !matches!(
                    *p,
                    "spartan.field_elements"
                        | "spartan.message"
                        | "statement.field"
                        | "challenge.accepted_field_element"
                )
            })
            .unwrap_or(kind)
            .to_owned();
        let context = pending
            .spans
            .iter()
            .flat_map(|s| s.fields.iter())
            .filter(|(k, _)| {
                matches!(
                    k.as_str(),
                    "round" | "coordinate" | "layer" | "stage" | "group"
                )
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut state = self.0.state.lock().expect("transcript writer poisoned");
        let Some(sink) = state.as_mut() else {
            return;
        };
        let record = LogicalEvent {
            schema_version: 1,
            sequence: sink.sequence,
            role,
            operation,
            purpose,
            context,
            complete: pending.value.is_some(),
            value: pending.value.unwrap_or(serde_json::Value::Null),
            draw_count: pending
                .wire
                .iter()
                .filter(|w| w.fields.operation == Operation::Squeeze)
                .count(),
            spans: pending.spans,
            wire: pending.wire,
        };
        sink.sequence += 1;
        if sink.error.is_none() {
            let result = serde_json::to_writer(&mut sink.writer, &record)
                .map_err(io::Error::other)
                .and_then(|()| sink.writer.write_all(b"\n"));
            if let Err(error) = result {
                sink.error = Some(error);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogicalEvent {
    pub schema_version: u32,
    pub sequence: u64,
    pub role: String,
    pub operation: Operation,
    pub purpose: String,
    pub context: BTreeMap<String, serde_json::Value>,
    pub complete: bool,
    pub value: serde_json::Value,
    pub draw_count: usize,
    pub spans: Vec<SpanContext>,
    pub wire: Vec<TranscriptEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    Logical(LogicalEvent),
    Legacy(TranscriptEvent),
}
impl Event {
    pub fn into_bytes(self) -> Vec<TranscriptEvent> {
        match self {
            Self::Logical(event) => event.wire,
            Self::Legacy(event) => vec![event],
        }
    }
    fn validate(&self) -> io::Result<()> {
        let wire = match self {
            Self::Logical(e) => {
                if e.schema_version != 1 || !e.complete {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsupported or incomplete logical record",
                    ));
                }
                &e.wire[..]
            }
            Self::Legacy(e) => std::slice::from_ref(e),
        };
        for event in wire {
            event.fields.bytes()?;
        }
        Ok(())
    }
}

/// Stream versioned logical records or earlier byte captures, checking each line.
pub fn read_events(path: impl AsRef<Path>) -> io::Result<impl Iterator<Item = io::Result<Event>>> {
    Ok(BufReader::new(File::open(path)?)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let parse = || -> io::Result<Event> {
                let event: Event = serde_json::from_str(&line?).map_err(io::Error::other)?;
                event.validate()?;
                Ok(event)
            };
            parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("transcript line {}: {e}", index + 1),
                )
            })
        }))
}

/// Expand one logical record at a time into the original ordered byte events.
pub fn read_byte_events(
    path: impl AsRef<Path>,
) -> io::Result<impl Iterator<Item = io::Result<TranscriptEvent>>> {
    Ok(read_events(path)?.flat_map(|event| match event {
        Ok(event) => event.into_bytes().into_iter().map(Ok).collect::<Vec<_>>(),
        Err(error) => vec![Err(error)],
    }))
}

/// Render a checked capture without retaining the complete run in memory.
pub fn render_text(input: impl AsRef<Path>, output: impl AsRef<Path>) -> io::Result<PathBuf> {
    render_text_with_annotations(input, output, |_| None)
}

/// Render with optional descriptions supplied by the caller, such as the public
/// statement behind an absorbed digest. Annotations appear only in the readable
/// output; they do not modify the captured values or wire operations.
pub fn render_text_with_annotations<'a>(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    mut annotate: impl FnMut(&LogicalEvent) -> Option<(&'a str, &'a serde_json::Value)>,
) -> io::Result<PathBuf> {
    let mut writer = BufWriter::new(File::create_new(output.as_ref())?);
    for event in read_events(input)? {
        match event? {
            Event::Logical(e) => {
                writeln!(
                    writer,
                    "#{} {} > {}",
                    e.sequence,
                    e.role,
                    e.spans
                        .iter()
                        .filter(|s| s.name != "transcript_operation")
                        .map(|s| if s.fields.is_empty() {
                            s.name.clone()
                        } else {
                            format!("{} {}", s.name, serde_json::json!(s.fields))
                        })
                        .collect::<Vec<_>>()
                        .join(" > ")
                )?;
                let operation = match e.operation {
                    Operation::Absorb => "ABSORB",
                    Operation::Squeeze => "SQUEEZE",
                };
                writeln!(writer, "{operation} {}", e.purpose)?;
                render_value(&mut writer, &e.value, 2)?;
                if let Some((label, value)) = annotate(&e) {
                    writeln!(writer, "\n{label}")?;
                    serde_json::to_writer_pretty(&mut writer, value)?;
                }
                if e.operation == Operation::Squeeze {
                    writeln!(writer, "  draws: {}", e.draw_count)?;
                }
                writeln!(writer)?;
            }
            Event::Legacy(e) => writeln!(
                writer,
                "{:?} {} bytes: {}",
                e.fields.operation, e.fields.byte_len, e.fields.bytes_hex
            )?,
        }
    }
    writer.flush()?;
    output.as_ref().canonicalize()
}

/// Render full values as indented fields; only scalar arrays stay on one line.
fn render_value(
    writer: &mut impl Write,
    value: &serde_json::Value,
    indent: usize,
) -> io::Result<()> {
    match value {
        serde_json::Value::Object(fields) if !fields.is_empty() => {
            for (name, value) in fields {
                write!(writer, "{:indent$}{name}:", "")?;
                render_field(writer, value, indent)?;
            }
        }
        serde_json::Value::Array(values)
            if values.iter().any(|v| v.is_object() || v.is_array()) =>
        {
            for value in values {
                write!(writer, "{:indent$}-", "")?;
                render_field(writer, value, indent)?;
            }
        }
        _ => {
            write!(writer, "{:indent$}", "")?;
            serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn render_field(
    writer: &mut impl Write,
    value: &serde_json::Value,
    indent: usize,
) -> io::Result<()> {
    let structured = match value {
        serde_json::Value::Object(fields) => !fields.is_empty(),
        serde_json::Value::Array(values) => values.iter().any(|v| v.is_object() || v.is_array()),
        _ => false,
    };
    if structured {
        writeln!(writer)?;
        render_value(writer, value, indent + 2)
    } else {
        write!(writer, " ")?;
        serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
        writeln!(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter {
        fail_on_flush: bool,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_on_flush {
                Ok(bytes.len())
            } else {
                Err(io::Error::other("write failed"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }

    #[test]
    fn finish_reports_buffered_write_and_flush_failures() {
        for fail_on_flush in [false, true] {
            let logs = TranscriptLogs::new(true);
            *logs.state.lock().unwrap() = Some(Sink {
                writer: BufWriter::new(Box::new(FailingWriter { fail_on_flush })),
                error: None,
                sequence: 0,
            });
            let trial = Trial {
                logs: &logs,
                path: Some("injected.jsonl".into()),
            };
            // An error while fmt writes is retained even though its API cannot
            // return it; a small pending write also exercises finish's flush.
            logs.make_writer().write_all(&vec![0; 16 * 1024]).unwrap();
            logs.make_writer().write_all(b"pending").unwrap();
            let error = trial.finish().unwrap_err();
            assert_eq!(
                error.to_string(),
                if fail_on_flush {
                    "flush failed"
                } else {
                    "write failed"
                }
            );
            assert!(logs.state.lock().unwrap().is_none());
        }
    }
}
