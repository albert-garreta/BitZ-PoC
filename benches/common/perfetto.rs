//! Optional Perfetto recording. Spans stay ordinary `tracing` spans; the SDK
//! owns clocks, thread tracks, and buffering. This is not a metrics collector.

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
    io::{self, Write},
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
    pub fn finish(mut self) -> io::Result<()> {
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
        writer.flush()
    }
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
