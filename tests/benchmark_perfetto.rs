#[path = "../benches/common/output.rs"]
mod output;
use bitz::observability::perfetto::TraceRecording;
use output::{BenchmarkOutput, FileMode};
use std::{
    io::{self, Write},
    sync::Mutex,
};
use tracing_subscriber::prelude::*;
// The optional native exporter is process-global.
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn shared_output_preserves_creation_policy() {
    let _lock = TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = BenchmarkOutput::new(dir.path());
    let subscriber = tracing_subscriber::registry().with(bitz::observability::layer());
    tracing::subscriber::with_default(subscriber, || {
        for trial in 0..2 {
            let name = format!("trial-{trial}.pftrace");
            let metrics = bitz::observability::Recording::start().unwrap();
            let recording =
                TraceRecording::start(output.buffered(&name, FileMode::CreateNew).unwrap())
                    .unwrap();
            tracing::info_span!("trial", trial).in_scope(|| std::hint::black_box(42));
            let intervals = metrics.intervals().unwrap();
            assert_eq!(intervals.len(), 1);
            assert!(
                !bitz::observability::duration(&intervals, "trial")
                    .unwrap()
                    .is_zero()
            );
            recording.finish().unwrap();
            let bytes = std::fs::read(dir.path().join(&name)).unwrap();
            let fields = perfetto_sdk::pb_decoder::PbDecoder::new(&bytes)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(!fields.is_empty(), "SDK must emit valid protobuf packets");
            assert!(
                bytes.windows(5).any(|window| window == b"trial"),
                "export must include the entered span"
            );
            assert_eq!(
                output.file(&name, FileMode::CreateNew).unwrap_err().kind(),
                io::ErrorKind::AlreadyExists
            );
        }
    });
}

struct FailingWriter {
    fail_write: bool,
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

#[test]
fn write_and_flush_errors_are_returned() {
    let _lock = TEST_LOCK.lock().unwrap();
    for fail_write in [true, false] {
        let recording = TraceRecording::start(FailingWriter { fail_write }).unwrap();
        let error = recording.finish().unwrap_err();
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
