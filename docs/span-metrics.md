# Shared span measurements

Benchmark and diagnostic executables enable `span-metrics`, install
`bitz::observability::layer()`, and read completed intervals entirely in Rust.
No Perfetto installation, processor, shell setup, or external query is required.
The default library does not install a subscriber. Isolated Binius64 workers
use this same collector.

```rust,ignore
bitz::observability::install()?; // Once, at the executable boundary.
let recording = bitz::observability::Recording::start()?;
let result = tracing::info_span!("prove").in_scope(|| prove())?;
let intervals = recording.intervals()?;
let duration = bitz::observability::duration(&intervals, "prove")?;
bitz::observability::write_profile(std::io::stdout().lock(), "proof", &intervals, None)?;
```

For one standalone operation, `measure(span, closure)` returns its value and
duration. It rejects a disabled span or missing collector before executing work.
Install one layer per subscriber; it can compose with unrelated subscriber layers.

## What the durations mean

- The collector timestamps span **entry and exit**, not creation and destruction.
  Each entry has a unique ID. Re-entry, recursion, and the same span on multiple
  threads produce separate intervals. Names and the optional `component` field
  are captured at entry; field updates apply to subsequent entries.
- `duration(intervals, label)` requires exactly one occurrence. `totals` unions
  overlapping intervals of each label across threads; it does not sum parallel
  work twice. Parent durations include children. Totals of different labels
  must not be added to infer end-to-end time.
- `phase_totals` selects intervals temporally contained in a unique phase of a
  bounded trial. Execution parents describe the active stack on the same thread;
  explicit logical parents across threads do not turn into execution nesting.
- The implementation uses a monotonic `Instant` clock inside the subscriber.
  Functions need only tracing spans, with no manual timers. Both `spans` and
  `wall-clock` measure elapsed wall time, not CPU usage. Nanosecond representation
  does not imply nanosecond accuracy. Scheduling, preemption, clock resolution,
  and instrumentation overhead affect results.
- For async code use `.instrument`, never retain an entered guard across `.await`.
  The resulting intervals measure entered/poll time, excluding time suspended.
  Use an enclosing synchronous phase when the desired metric is total latency.
- Close measured spans and join worker tasks before querying. Use a global
  subscriber at an executable boundary or propagate its dispatch to worker threads.
  Tracing cannot measure work on threads without the subscriber.

## Recording boundaries and errors

Each subscriber owns an independent collector. Per-thread buffers avoid a shared
lock on every event; recording boundaries synchronize buffer resets and snapshots.
Nested recordings share events and can be queried while an enclosing span remains
open. Only entries begun inside the recording are returned. Repeated independent
trials clear completed events. Dropping a recording cancels its query.

Missing layers, empty recordings, unfinished spans, invalid nesting, and capacity
overflow return errors rather than fabricated or truncated timings. The default
retained interval payload budget is 64 MiB, shared by overlapping recordings.
`SpanMetricsLayer::with_capacity(bytes)` sets another budget. This bounds interval
payloads, not total process RSS or allocator capacity. An overflowing trial is
invalid; the next independent trial resets the budget. Keep captures bounded.

## Modes and optional export

SHA/ECDSA defaults to `--timing spans`, including its isolated Binius64 worker.
`--timing wall-clock` remains available for `bitz-split`, `bitz-all`, and
`spartan-mc`; it measures top-level phases without internal span breakdowns.
New results identify the span backend as `timing: "spans"`. Some specialized
multiplication microbenchmarks and auxiliary setup timers retain their existing
wall-clock boundaries; this refactor does not change those experiments.
Readers retain historical Perfetto results,
and campaign resumption rejects a changed timing backend. There is no automatic
fallback between backends. Historical measurements should not be treated as a
performance baseline without accounting for the instrumentation change.

`bench-perfetto` additionally compiles the native SDK and requires a C++ toolchain.
Use `observability::perfetto::TraceRecording::start(writer)` and `finish()` to
export `.pftrace` bytes for the Perfetto UI. It initializes the native SDK only
when an export starts. Numeric timings still come from the Rust collector;
no trace processor is invoked, even when exporting. Export overhead can affect
measurements, so compare equivalent build and export configurations.

## Memory and verification

`observability::memory::MemoryLayer` separately observes process peak RSS on
span entry/exit. Its inclusive growth can overlap and is not live allocation
usage. Hybrid measurements retain their existing platform availability; native
multiplication retains its fresh-process, untraced memory pass. Whole-process
RSS includes setup, instrumentation, and earlier report allocations.

```sh
cargo test --locked --features span-metrics --test benchmark_spans
cargo test --locked --features bench-perfetto --test benchmark_perfetto
```

These check numeric queries, nested and parallel entries, repeated trials,
subscriber isolation, overflow and incomplete captures, profile projections,
and optional export writer errors without an external processor.
