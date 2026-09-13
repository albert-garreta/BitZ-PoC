//! Perfetto recording and typed interval queries. The SDK owns all clocks,
//! thread tracks, and buffering; the native trace processor reconstructs slices.

use perfetto_sdk::{
    heap_buffer::HeapBuffer,
    pb_msg::{PbMsg, PbMsgWriter},
    protos::config::{
        data_source_config::DataSourceConfig,
        trace_config::{
            BufferConfigFillPolicy, TraceConfig, TraceConfigBufferConfig, TraceConfigDataSource,
        },
        track_event::track_event_config::TrackEventConfig,
    },
    tracing_session::TracingSession,
};
use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, Write},
    process::{Command, Stdio},
    sync::{Arc, Mutex, Once, mpsc},
    time::Duration,
};

const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(tracing_perfetto_sdk::init_in_process);
}

/// Compose with the application's existing subscriber; never install a second one.
pub fn layer() -> tracing_perfetto_sdk::PerfettoLayer {
    init();
    tracing_perfetto_sdk::PerfettoLayer::new()
}

#[must_use = "close all spans, then call finish to flush and save the trace"]
pub struct Recording<W> {
    session: TracingSession,
    writer: W,
}

impl<W: Write + Send + 'static> Recording<W> {
    /// Supply a writer opened through `BenchmarkOutput`. Session startup is
    /// outside the measured region. Keep each recording bounded to one trial.
    pub fn start(writer: W) -> io::Result<Self> {
        init();
        let mut session = TracingSession::in_process().map_err(io::Error::other)?;
        session.setup(&trace_config());
        session.start_blocking();
        Ok(Self { session, writer })
    }

    /// Flush producers before stopping, then save through the supplied writer.
    /// Use the asynchronous flush API because the blocking SDK API hides failure.
    pub fn finish(self) -> io::Result<()> {
        self.into_inner().map(|_| ())
    }

    /// Complete capture and recover the flushed writer (including an in-memory sink).
    pub fn into_inner(mut self) -> io::Result<W> {
        let (send, receive) = mpsc::sync_channel(1);
        self.session.flush_async(FLUSH_TIMEOUT, move |success| {
            let _ = send.send(success);
        });
        let flushed = receive.recv_timeout(FLUSH_TIMEOUT + Duration::from_secs(1));
        self.session.stop_blocking();
        if !matches!(flushed, Ok(true)) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Perfetto producer flush failed",
            ));
        }

        // The SDK invokes its callback on an internal thread and cannot return
        // I/O errors. Retain the first error and report it after the blocking read.
        let result = Arc::new(Mutex::new(Ok(self.writer)));
        let sink = Arc::clone(&result);
        self.session.read_trace_blocking(move |data, _has_more| {
            if let Ok(mut result) = sink.lock()
                && let Ok(writer) = &mut *result
                && let Err(error) = writer.write_all(data)
            {
                *result = Err(error);
            }
        });
        let result = Arc::try_unwrap(result)
            .map_err(|_| io::Error::other("Perfetto retained the completed read callback"))?;
        let mut writer = result
            .into_inner()
            .map_err(|_| io::Error::other("Perfetto sink poisoned"))??;
        writer.flush()?;
        Ok(writer)
    }
}

impl Recording<Vec<u8>> {
    /// Query only after the measured scopes have exited. No temporary trace file
    /// or Python process is needed on the supported Unix benchmark platforms.
    pub fn intervals(self) -> io::Result<Vec<Interval>> {
        TraceProcessor::from_env().intervals(&self.into_inner()?)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Interval {
    pub id: u64,
    pub parent: Option<u64>,
    pub track_id: u64,
    pub name: String,
    pub component: Option<String>,
    pub start_ns: u64,
    pub end_ns: u64,
}

/// A local native executable, never a downloader or an external tracing service.
pub struct TraceProcessor {
    executable: OsString,
}

impl TraceProcessor {
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var_os("PERFETTO_TRACE_PROCESSOR")
                .unwrap_or_else(|| "trace_processor_shell".into()),
        )
    }

    pub fn intervals(&self, trace: &[u8]) -> io::Result<Vec<Interval>> {
        if !cfg!(unix) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "in-memory Perfetto queries currently require Unix /dev/stdin",
            ));
        }
        let mut child = Command::new(&self.executable)
            .args(["/dev/stdin", "-Q", INTERVAL_QUERY])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|error| io::Error::new(error.kind(), format!(
                "cannot start native Perfetto processor {:?}: {error}; set PERFETTO_TRACE_PROCESSOR",
                self.executable)))?;
        let mut input = child.stdin.take().expect("piped processor stdin");
        // Drain stdout/stderr while supplying the trace: either pipe can exceed
        // the OS buffer for large traces, including parser diagnostics.
        std::thread::scope(|scope| {
            let write = scope.spawn(move || input.write_all(trace));
            let output = child.wait_with_output();
            let written = write
                .join()
                .map_err(|_| io::Error::other("Perfetto input thread panicked"))?;
            let output = output?;
            if !output.status.success() {
                return Err(io::Error::other(format!(
                    "Perfetto query failed ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            written?;
            decode_intervals(&output.stdout)
        })
    }
}

// One result set. JSON is the query's typed wire format inside CSV, preserving
// nulls and arbitrary span names without depending on the CLI's NULL sentinel.
const INTERVAL_QUERY: &str = r#"
WITH records AS (
SELECT json_object('record', 'status',
  'incomplete', (SELECT count(*) FROM slice WHERE dur < 0),
  'errors', (SELECT count(*) FROM stats WHERE value != 0 AND severity IN ('error', 'data_loss'))
) AS record
UNION ALL
SELECT json_object('record', 'span', 'id', id, 'parent', parent_id,
  'track_id', track_id, 'name', name,
  'component', EXTRACT_ARG(arg_set_id, 'debug.component'),
  'start_ns', ts - (SELECT min(ts) FROM slice),
  'end_ns', ts + dur - (SELECT min(ts) FROM slice))
FROM slice
)
SELECT replace(record, '"', '""') AS record FROM records"#;

#[derive(serde::Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum QueryRecord {
    Status { incomplete: u64, errors: u64 },
    Span(Interval),
}

fn decode_intervals(bytes: &[u8]) -> io::Result<Vec<Interval>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let mut reader = csv::Reader::from_reader(bytes);
    if reader.headers().map_err(io::Error::other)? != &csv::StringRecord::from(vec!["record"]) {
        return Err(invalid("unexpected Perfetto query columns"));
    }
    let mut status = false;
    let mut spans = Vec::new();
    for row in reader.records() {
        let row = row.map_err(io::Error::other)?;
        match serde_json::from_str::<QueryRecord>(&row[0]).map_err(io::Error::other)? {
            QueryRecord::Status { incomplete, errors } if !status => {
                if incomplete != 0 || errors != 0 {
                    return Err(invalid(
                        "incomplete or lossy Perfetto capture; refusing timing metrics",
                    ));
                }
                status = true;
            }
            QueryRecord::Span(span) if status => spans.push(span),
            _ => return Err(invalid("missing or duplicate Perfetto capture status")),
        }
    }
    if !status || spans.is_empty() {
        return Err(invalid(
            "empty Perfetto capture; is its tracing layer installed?",
        ));
    }
    let by_id: HashMap<_, _> = spans.iter().map(|span| (span.id, span)).collect();
    if by_id.len() != spans.len() {
        return Err(invalid("duplicate Perfetto slice ID"));
    }
    for span in &spans {
        if span.end_ns < span.start_ns {
            return Err(invalid("reversed Perfetto interval"));
        }
        if let Some(id) = span.parent {
            let parent = by_id
                .get(&id)
                .ok_or_else(|| invalid("missing Perfetto parent"))?;
            if parent.id >= span.id
                || parent.track_id != span.track_id
                || parent.start_ns > span.start_ns
                || span.end_ns > parent.end_ns
            {
                return Err(invalid("invalid Perfetto parent interval"));
            }
        }
    }
    spans.sort_by_key(|span| (span.start_ns, span.end_ns, span.id));
    Ok(spans)
}

fn trace_config() -> Vec<u8> {
    let writer = PbMsgWriter::new();
    let buffer = HeapBuffer::new(writer.stream_writer());
    let mut message = PbMsg::new(&writer).expect("allocate Perfetto configuration");
    {
        let mut config = TraceConfig { msg: &mut message };
        config.set_buffers(|buffer: &mut TraceConfigBufferConfig| {
            buffer.set_size_kb(64 * 1024);
            buffer.set_fill_policy(BufferConfigFillPolicy::Discard);
        });
        config.set_data_sources(|source: &mut TraceConfigDataSource| {
            source.set_config(|source: &mut DataSourceConfig| {
                source.set_name("track_event");
                source.set_track_event_config(|events: &mut TrackEventConfig| {
                    events.set_disabled_categories("*");
                    events.set_enabled_categories("tracing");
                });
            });
        });
    }
    message.finalize();
    let mut bytes = vec![0; writer.stream_writer().get_written_size()];
    buffer.copy_into(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn decode(rows: &[Value]) -> io::Result<Vec<Interval>> {
        let mut csv = csv::Writer::from_writer(Vec::new());
        csv.write_record(["record"]).unwrap();
        for row in rows {
            csv.write_record([row.to_string()]).unwrap();
        }
        decode_intervals(&csv.into_inner().unwrap())
    }

    fn fixture() -> Vec<Value> {
        vec![
            json!({"record":"status", "incomplete":0, "errors":0}),
            json!({"record":"span", "id":0, "parent":null, "track_id":10,
                "name":"comma, quote\"\nnewline", "component":null,
                "start_ns":0, "end_ns":9007199254740993_u64}),
            json!({"record":"span", "id":1, "parent":0, "track_id":10,
                "name":"child", "component":"test.child", "start_ns":1, "end_ns":2}),
        ]
    }

    #[test]
    fn query_preserves_exact_nanoseconds_nulls_and_escaped_names() {
        let spans = decode(&fixture()).unwrap();
        assert_eq!(spans[0].end_ns, 9007199254740993);
        assert_eq!(spans[0].name, "comma, quote\"\nnewline");
        assert_eq!(spans[0].component, None);
        assert_eq!(spans[1].parent, Some(0));
        assert_eq!(spans[1].component.as_deref(), Some("test.child"));
    }

    #[test]
    fn query_rejects_incomplete_lossy_empty_or_malformed_output() {
        for field in ["incomplete", "errors"] {
            let mut rows = fixture();
            rows[0][field] = json!(1);
            assert!(decode(&rows).is_err());
        }
        assert!(decode(&fixture()[..1]).is_err());
        assert!(decode(&fixture()[1..]).is_err());
        assert!(decode_intervals(b"wrong\nheader\n").is_err());
        assert!(decode_intervals(b"record\nnot-json\n").is_err());
        for (field, value) in [
            ("end_ns", json!(0)),
            ("parent", json!(3)),
            ("parent", json!(1)),
            ("id", json!(0)),
            ("track_id", json!(11)),
        ] {
            let mut rows = fixture();
            rows[2][field] = value;
            assert!(decode(&rows).is_err(), "accepted invalid {field}");
        }
    }

    #[test]
    fn missing_processor_is_an_error_not_a_clock_fallback() {
        let processor = TraceProcessor::new("/nonexistent-f2z-perfetto-test/trace_processor_shell");
        assert_eq!(
            processor.intervals(&[]).unwrap_err().kind(),
            if cfg!(unix) {
                io::ErrorKind::NotFound
            } else {
                io::ErrorKind::Unsupported
            }
        );
    }
}
