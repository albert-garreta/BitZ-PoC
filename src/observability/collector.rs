//! Per-subscriber, per-thread interval capture. Only recording boundaries take
//! the shared buffer registry lock; span callbacks use their thread's buffer.

use super::Interval;
use std::{
    cell::RefCell,
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;
static NEXT_COLLECTOR: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static BUFFERS: RefCell<HashMap<u64, Weak<ThreadBuffer>>> = RefCell::new(HashMap::new());
}

/// Collect elapsed entry/exit intervals without an external processor.
pub struct SpanMetricsLayer {
    collector: Arc<Collector>,
    #[cfg(feature = "bench-perfetto")]
    export: tracing_perfetto_sdk::PerfettoLayer,
}

impl Default for SpanMetricsLayer {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_MAX_BYTES)
    }
}

impl SpanMetricsLayer {
    /// Limit retained interval payloads across all overlapping recordings.
    /// Overflow invalidates those recordings until the next independent trial.
    pub fn with_capacity(max_bytes: usize) -> Self {
        Self {
            collector: Arc::new(Collector {
                id: NEXT_COLLECTOR.fetch_add(1, Ordering::Relaxed),
                epoch: Instant::now(),
                max_bytes,
                sequence: AtomicU64::new(1),
                generation: AtomicU64::new(0),
                active: AtomicUsize::new(0),
                bytes: AtomicUsize::new(0),
                failed: AtomicBool::new(false),
                recordings: Mutex::new(()),
                buffers: Mutex::new(Vec::new()),
            }),
            #[cfg(feature = "bench-perfetto")]
            export: tracing_perfetto_sdk::PerfettoLayer::new(),
        }
    }
}

struct Collector {
    id: u64,
    epoch: Instant,
    max_bytes: usize,
    sequence: AtomicU64,
    generation: AtomicU64,
    active: AtomicUsize,
    bytes: AtomicUsize,
    failed: AtomicBool,
    recordings: Mutex<()>,
    buffers: Mutex<Vec<Arc<ThreadBuffer>>>,
}

struct ThreadBuffer {
    track_id: u64,
    data: Mutex<ThreadData>,
}

#[derive(Default)]
struct ThreadData {
    entries: Vec<Entry>,
    completed: Vec<Interval>,
}

struct Entry {
    span: Id,
    generation: u64,
    interval: Option<Interval>,
}

#[derive(Default)]
struct Component(Option<String>);
impl Visit for Component {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "component" {
            self.0 = Some(value.to_owned());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "component" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

impl Collector {
    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).expect("span clock exceeds u64 nanoseconds")
    }

    fn buffer(&self) -> Arc<ThreadBuffer> {
        BUFFERS.with(|buffers| {
            let mut local = buffers.borrow_mut();
            if let Some(buffer) = local.get(&self.id).and_then(Weak::upgrade) {
                return buffer;
            }
            local.retain(|_, buffer| buffer.strong_count() != 0);
            let mut all = self.buffers.lock().unwrap();
            let buffer = Arc::new(ThreadBuffer {
                track_id: all.len() as u64,
                data: Mutex::new(ThreadData::default()),
            });
            all.push(Arc::clone(&buffer));
            local.insert(self.id, Arc::downgrade(&buffer));
            buffer
        })
    }

    fn reserve(&self, bytes: usize) -> bool {
        let reserved = self
            .bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes)
                    .filter(|next| *next <= self.max_bytes)
            })
            .is_ok();
        if !reserved {
            self.failed.store(true, Ordering::Relaxed);
        }
        reserved
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpanMetricsLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut component = Component::default();
        attrs.record(&mut component);
        ctx.span(id)
            .expect("registered span")
            .extensions_mut()
            .insert(component);
        #[cfg(feature = "bench-perfetto")]
        self.export.on_new_span(attrs, id, ctx);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(component) = extensions.get_mut::<Component>() {
                values.record(component);
            }
        }
        #[cfg(feature = "bench-perfetto")]
        self.export.on_record(id, values, ctx);
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let collector = &self.collector;
        let buffer = collector.buffer();
        let mut data = buffer.data.lock().unwrap();
        let generation = collector.generation.load(Ordering::Acquire);
        let interval = if collector.active.load(Ordering::Acquire) != 0 {
            let span = ctx.span(id).expect("entered span");
            let name = span.metadata().name();
            let component = span
                .extensions()
                .get::<Component>()
                .and_then(|c| c.0.clone());
            let bytes = std::mem::size_of::<Interval>()
                + name.len()
                + component.as_ref().map_or(0, String::len);
            if collector.reserve(bytes) {
                let parent = data
                    .entries
                    .iter()
                    .rev()
                    .filter(|entry| entry.generation == generation)
                    .find_map(|entry| entry.interval.as_ref());
                Some(Interval {
                    id: collector.sequence.fetch_add(1, Ordering::Relaxed),
                    parent: parent.map(|p| p.id),
                    track_id: buffer.track_id,
                    depth: parent.map_or(0, |p| p.depth + 1),
                    name: name.to_owned(),
                    component,
                    start_ns: collector.now(),
                    end_ns: 0,
                })
            } else {
                None
            }
        } else {
            None
        };
        data.entries.push(Entry {
            span: id.clone(),
            generation,
            interval,
        });
        drop(data);
        #[cfg(feature = "bench-perfetto")]
        if super::perfetto::enabled() {
            self.export.on_enter(id, ctx);
        }
    }

    fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
        let collector = &self.collector;
        let end_ns = collector.now();
        let buffer = collector.buffer();
        let mut data = buffer.data.lock().unwrap();
        if let Some(index) = data.entries.iter().rposition(|entry| entry.span == *id) {
            if index + 1 != data.entries.len() {
                collector.failed.store(true, Ordering::Relaxed);
            }
            let entry = data.entries.remove(index);
            if entry.generation == collector.generation.load(Ordering::Acquire)
                && let Some(mut interval) = entry.interval
            {
                interval.end_ns = end_ns;
                data.completed.push(interval);
            }
        } else {
            collector.failed.store(true, Ordering::Relaxed);
        }
        drop(data);
        #[cfg(feature = "bench-perfetto")]
        if super::perfetto::enabled() {
            self.export.on_exit(id, _ctx);
        }
    }

    #[cfg(feature = "bench-perfetto")]
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        if super::perfetto::enabled() {
            self.export.on_event(event, ctx);
        }
    }
}

/// A bounded view of the active subscriber's spans. Independent subscribers
/// have independent collectors; nested recordings share their retained events.
#[must_use = "exit measured spans and join workers before querying the recording"]
pub struct Recording {
    collector: Arc<Collector>,
    start_id: u64,
}

impl Recording {
    pub fn start() -> io::Result<Self> {
        let collector = tracing::dispatcher::get_default(|dispatch| {
            dispatch
                .downcast_ref::<SpanMetricsLayer>()
                .map(|layer| Arc::clone(&layer.collector))
        })
        .ok_or_else(|| io::Error::other("install the span metrics layer before recording"))?;
        let start_id;
        {
            let _guard = collector.recordings.lock().unwrap();
            if collector.active.load(Ordering::Acquire) == 0 {
                let buffers = collector.buffers.lock().unwrap();
                // Freeze every registered thread through publication of the new
                // generation. A racing callback cannot see half of this reset.
                let mut data: Vec<_> = buffers.iter().map(|b| b.data.lock().unwrap()).collect();
                for data in &mut data {
                    data.completed.clear();
                }
                collector.generation.fetch_add(1, Ordering::Release);
                collector.bytes.store(0, Ordering::Relaxed);
                collector.failed.store(false, Ordering::Relaxed);
                start_id = collector.sequence.fetch_add(1, Ordering::Relaxed);
                collector.active.store(1, Ordering::Release);
            } else {
                start_id = collector.sequence.fetch_add(1, Ordering::Relaxed);
                collector.active.fetch_add(1, Ordering::Release);
            }
        }
        Ok(Self {
            collector,
            start_id,
        })
    }

    /// Return only this recording's completed entries. An enclosing span which
    /// was entered before recording started does not make a child capture incomplete.
    pub fn intervals(self) -> io::Result<Vec<Interval>> {
        let collector = &self.collector;
        let _guard = collector.recordings.lock().unwrap();
        let end_id = collector.sequence.fetch_add(1, Ordering::Relaxed);
        let end_ns = collector.now();
        let buffers = collector.buffers.lock().unwrap();
        let mut intervals = Vec::new();
        let included = |span: &Interval| self.start_id < span.id && span.id < end_id;
        for buffer in buffers.iter() {
            let data = buffer.data.lock().unwrap();
            if data
                .entries
                .iter()
                .filter_map(|entry| entry.interval.as_ref())
                .any(included)
            {
                return Err(io::Error::other(
                    "incomplete span capture; exit spans and join workers before querying",
                ));
            }
            for span in data.completed.iter().filter(|span| included(span)) {
                if span.end_ns > end_ns || span.end_ns < span.start_ns {
                    return Err(io::Error::other("span crosses the recording boundary"));
                }
                intervals.push(span.clone());
            }
        }
        if collector.failed.load(Ordering::Relaxed) {
            return Err(io::Error::other(
                "invalid or overflowing span capture; increase SpanMetricsLayer::with_capacity or shorten the recording",
            ));
        }
        if intervals.is_empty() {
            return Err(io::Error::other(
                "empty span capture; no enabled spans were entered",
            ));
        }
        // Execution parents may precede a nested recording. Rebase its forest
        // without changing endpoints or attributing another trial's intervals.
        intervals.sort_unstable_by_key(|span| span.id);
        let origin = intervals.iter().map(|span| span.start_ns).min().unwrap();
        let mut depths = HashMap::new();
        for span in &mut intervals {
            span.parent = span.parent.filter(|parent| depths.contains_key(parent));
            span.depth = span.parent.map_or(0, |parent| depths[&parent] + 1);
            depths.insert(span.id, span.depth);
            span.start_ns -= origin;
            span.end_ns -= origin;
        }
        intervals.sort_unstable_by_key(|span| (span.start_ns, span.end_ns, span.id));
        Ok(intervals)
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let _guard = self.collector.recordings.lock().unwrap();
        self.collector.active.fetch_sub(1, Ordering::Release);
    }
}
