//! In-process span measurements and typed interval queries.
//! Native Perfetto export is optional and is never used to calculate metrics.

#[path = "observability/collector.rs"]
mod collector;
#[path = "observability/memory.rs"]
pub mod memory;
#[cfg(feature = "bench-perfetto")]
#[path = "observability/perfetto.rs"]
pub mod perfetto;

pub use collector::{Recording, SpanMetricsLayer};
use std::{
    collections::HashMap,
    io::{self, Write},
    time::Duration,
};

/// Install once at an executable boundary; libraries compose `layer()` instead.
pub fn install() -> io::Result<()> {
    use tracing_subscriber::prelude::*;
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer()))
        .map_err(io::Error::other)
}

/// Create an independent collector for this subscriber.
pub fn layer() -> SpanMetricsLayer {
    SpanMetricsLayer::default()
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Interval {
    pub id: u64,
    pub parent: Option<u64>,
    pub track_id: u64,
    pub depth: usize,
    pub name: String,
    pub component: Option<String>,
    pub start_ns: u64,
    pub end_ns: u64,
}

impl Interval {
    /// Dynamic operation identities use `component`; ordinary spans use their name.
    pub fn label(&self) -> &str {
        self.component.as_deref().unwrap_or(&self.name)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_nanos(self.end_ns - self.start_ns)
    }
}

/// Resolve one completed operation. Ambiguity or missing instrumentation is an
/// error, never a fabricated zero-duration sample.
pub fn span<'a>(intervals: &'a [Interval], label: &str) -> io::Result<&'a Interval> {
    let mut matching = intervals.iter().filter(|span| span.label() == label);
    let span = matching.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, format!("missing span {label}"))
    })?;
    if matching.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("multiple spans for {label}; select a trial or aggregate intervals"),
        ));
    }
    Ok(span)
}

pub fn duration(intervals: &[Interval], label: &str) -> io::Result<Duration> {
    span(intervals, label).map(Interval::duration)
}

/// Inclusive operation totals inside a completed phase, excluding the reporting
/// envelope itself. Recordings must isolate one trial; cross-track work is
/// included by its real endpoints and repeated labels are unioned.
pub fn phase_totals(intervals: &[Interval], phase: &str) -> io::Result<Vec<(String, f64)>> {
    let parent = span(intervals, phase)?;
    Ok(totals(intervals.iter().filter(|s| {
        s.id != parent.id && s.start_ns >= parent.start_ns && s.end_ns <= parent.end_ns
    })))
}

/// A small executable-boundary convenience for one operation. The ordinary
/// tracing span defines its boundaries; the span collector supplies its elapsed time.
/// Recording setup and querying are outside the operation's span.
pub fn measure<T>(span: tracing::Span, operation: impl FnOnce() -> T) -> io::Result<(T, Duration)> {
    let name = span
        .metadata()
        .ok_or_else(|| {
            io::Error::other("measurement span is disabled; install the span metrics layer first")
        })?
        .name();
    let recording = Recording::start()?;
    let value = span.in_scope(operation);
    let intervals = recording.intervals()?;
    // Use the supplied span's static name, not an optional component override.
    let mut matching = intervals.iter().filter(|s| s.name == name);
    let root = matching
        .next()
        .ok_or_else(|| io::Error::other(format!("missing measurement {name}")))?;
    if matching.next().is_some() {
        return Err(io::Error::other(format!("ambiguous measurement {name}")));
    }
    Ok((value, root.duration()))
}

/// Per-label inclusive wall seconds in first-observed order. Union repeated or
/// parallel occurrences of the *same* operation; never sum nested labels into
/// a total. Callers select the trial/phase before aggregation.
pub fn totals<'a>(intervals: impl IntoIterator<Item = &'a Interval>) -> Vec<(String, f64)> {
    let mut groups: Vec<(String, Vec<(u64, u64)>)> = Vec::new();
    let mut indices = HashMap::new();
    for span in intervals {
        let index = *indices.entry(span.label()).or_insert_with(|| {
            groups.push((span.label().to_owned(), Vec::new()));
            groups.len() - 1
        });
        groups[index].1.push((span.start_ns, span.end_ns));
    }
    groups
        .into_iter()
        .map(|(label, ranges)| (label, union_ns(ranges) as f64 / 1e9))
        .collect()
}

fn union_ns(ranges: impl IntoIterator<Item = (u64, u64)>) -> u64 {
    let mut ranges: Vec<_> = ranges.into_iter().collect();
    ranges.sort_unstable();
    let mut end = 0;
    let mut ns = 0;
    for (start, next_end) in ranges {
        ns += next_end.saturating_sub(end.max(start));
        end = end.max(next_end);
    }
    ns
}

/// Render completed intervals, never measure them. Inclusive and exclusive
/// wall durations use interval unions; labels and repeated occurrences remain
/// in first-observed order. Optional RSS growth is a separate observation.
pub fn write_profile(
    mut writer: impl Write,
    header: &str,
    intervals: &[Interval],
    memory: Option<&std::collections::BTreeMap<String, memory::PeakGrowth>>,
) -> io::Result<()> {
    let denominator = union_ns(intervals.iter().map(|s| (s.start_ns, s.end_ns)));
    writeln!(writer, "┌─ prove profile: {header}")?;
    for (label, seconds) in totals(intervals) {
        let occurrences: Vec<_> = intervals.iter().filter(|s| s.label() == label).collect();
        let inclusive = union_ns(occurrences.iter().map(|s| (s.start_ns, s.end_ns)));
        // Subtract children per occurrence BEFORE unioning the same label.
        // A child's time can overlap another occurrence's own work (including
        // recursive occurrences of this label); subtracting group unions loses it.
        let mut self_ranges = Vec::new();
        for occurrence in &occurrences {
            let mut children: Vec<_> = intervals
                .iter()
                .filter(|s| s.parent == Some(occurrence.id))
                .map(|s| (s.start_ns, s.end_ns))
                .collect();
            children.sort_unstable();
            let mut cursor = occurrence.start_ns;
            for (start, end) in children {
                if cursor < start {
                    self_ranges.push((cursor, start));
                }
                cursor = cursor.max(end);
            }
            if cursor < occurrence.end_ns {
                self_ranges.push((cursor, occurrence.end_ns));
            }
        }
        let exclusive = union_ns(self_ranges);
        let share = if denominator == 0 {
            0.0
        } else {
            inclusive as f64 * 100.0 / denominator as f64
        };
        let indent = "  ".repeat(occurrences[0].depth);
        write!(
            writer,
            "│ {indent}{label}: {:.3?} ({share:.1}%)",
            Duration::from_secs_f64(seconds)
        )?;
        if occurrences.len() > 1 {
            write!(writer, " n={}", occurrences.len())?;
        }
        if exclusive < inclusive {
            write!(writer, " self={:.3?}", Duration::from_nanos(exclusive))?;
        }
        if let Some(row) = memory.and_then(|rows| rows.get(&label)) {
            write!(
                writer,
                " Δrss={:.1} MiB peak={:.1} MiB",
                row.bytes as f64 / 1048576.0,
                row.peak_bytes as f64 / 1048576.0
            )?;
        }
        writeln!(writer)?;
    }
    writeln!(writer, "└─")?;
    writer.flush()
}
