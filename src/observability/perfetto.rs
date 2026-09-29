//! Optional native trace export. Numeric measurements never query this trace.

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

use std::sync::atomic::{AtomicUsize, Ordering};

const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub(super) fn enabled() -> bool {
    ACTIVE.load(Ordering::Relaxed) != 0
}

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(tracing_perfetto_sdk::init_in_process);
}

struct ActiveExport;
impl Drop for ActiveExport {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

#[must_use = "close all spans, then call finish to flush and save the trace"]
pub struct TraceRecording<W> {
    session: TracingSession,
    writer: W,
    _active: ActiveExport,
}

impl<W: Write + Send + 'static> TraceRecording<W> {
    /// Supply a writer opened through `BenchmarkOutput`. Session startup is
    /// outside the measured region. Keep each recording bounded to one trial.
    pub fn start(writer: W) -> io::Result<Self> {
        init();
        let mut session = TracingSession::in_process().map_err(io::Error::other)?;
        session.setup(&trace_config());
        session.start_blocking();
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            session,
            writer,
            _active: ActiveExport,
        })
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
