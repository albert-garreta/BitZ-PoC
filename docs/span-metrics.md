# Shared span measurements

Core operations use ordinary `tracing` spans. The default library does not
install a subscriber or require Perfetto. Benchmark/diagnostic executables opt
into `span-metrics`, install one subscriber, and query the Perfetto SDK's completed
intervals through `src/observability.rs`. Isolated workers import that same source;
they do not maintain a second clock or collector. `src/utils/prof.rs` is retired.

## Runtime requirements

Install the native Perfetto trace processor into this checkout:

```bash
bash scripts/install_trace_processor.sh
export PERFETTO_TRACE_PROCESSOR="$PWD/.tools/perfetto/trace_processor_shell"
```

Run these commands from the repository root. The installer pins Perfetto v58.2,
verifies its SHA-256 checksum, and reuses a matching installation. Alternatively,
set `PERFETTO_TRACE_PROCESSOR` to another native processor or put
`trace_processor_shell` on `PATH`. Queries run locally, in Rust, through the native processor;
there is no Python step or automatic download. In-memory querying currently uses
Unix `/dev/stdin`. Build profiling examples and the `f2z` CLI with
`--features span-metrics`; add their backend features as usual.

## Measurement contract

- Start a `Recording` outside the operation, enter normal spans around the work,
  exit them, then query `recording.intervals()`. For one standalone operation,
  `measure(span, closure)` returns its value and duration. It rejects disabled
  spans instead of executing the operation with a fake zero duration.
- `duration(intervals, label)` requires exactly one matching completed span.
  `totals` unions repeated/parallel occurrences of each label; parents include
  children. `phase_totals` selects intervals inside one uniquely named phase in
  a bounded single-trial recording. These totals are not additive across labels.
- SDK timestamps and durations remain exact integer nanoseconds until existing
  report projections convert them to milliseconds. CSV/JSON fields, trial
  numbering, warmup exclusion, and aggregation formulas are unchanged.
- Native processing happens after the measured operation. Candidate recordings
  are queryable before tuning proceeds to the next candidate; the campaign span
  includes orchestration and query overhead, as an overall tuning duration should.
- Each recording has a 64 MiB discard buffer. Lost, malformed, unfinished, or
  ambiguous measurements are errors, never silently truncated metrics. Keep
  recordings bounded; especially large tuning campaigns may require a larger
  explicitly configured capture budget in future.
- A subscriber is configured at executable boundaries, not implicitly by library
  functions. Tests provide an explicit scoped subscriber. Ordinary proof bodies
  still run with `NoSubscriber`.

## Memory is separate

`observability::memory::MemoryLayer` only observes a supplied process-peak-RSS
probe on span entry/exit. It has no clock and is enabled only by the hybrid
diagnostic probe. Its inclusive growth values can overlap; they are not current
live allocations and must not be summed across nested/parallel regions.

Existing Rust-heap probes remain separate from RSS. Hybrid rows snapshot process
peak RSS before extracting that trial's trace. Whole-process RSS still includes
instrumentation, setup, and any earlier report-processing allocations; it is not
an uninstrumented prover-only measurement. Native multiplication retains its
fresh-process, untraced memory pass and separate latency trials. Do not mix these
different memory boundaries in comparisons.
Hybrid CSV RSS remains Linux-only; its existing unavailable sentinel is retained
on macOS. The diagnostic hybrid probe uses `getrusage` on macOS and Linux.

## Inventory and checks

The shared path covers core F2Z annotations; native multiplication and SHA;
SHA/ECDSA (including the Spartan2 `d3e686e` phase spans); PCS comparisons;
setup/witness audits; tuning; hybrid runs; CLI summaries; diagnostic examples;
isolated Binius64/zkPassport workers; and the standalone field microbenchmark.
`benches/common/trace_capture.rs` now contains pure interval projections, not a
collector. Historical `PerfRuns/` sources and unrelated unit-test microbenchmarks
are not part of this migration.

Executable regression coverage includes `benchmark_perfetto` (native SDK/query
behavior, repeated/parallel spans, nested candidate recordings, failures, profile
projection, and RSS observations), `benchmark_reporting` (output contracts), and
backend integration tests. Dedicated native processor smoke tests are ignored in
the ordinary suite; run them with `--include-ignored`. Configure the processor
for backend integration suites too: their benchmark setup and audit paths now
query spans, even when the assertion is about a proof or its output:

```sh
cargo test --features bench-perfetto --test benchmark_perfetto -- --include-ignored --test-threads=1
cargo test --features bench-internals,native-mul-compare --test native_mul_compare -- --include-ignored --test-threads=1
```

The zkPassport managed runtime requires its supported Linux/x86_64 environment,
circuit artifacts, and SRS. A successful macOS `cargo check` alone is not a
zkPassport runtime test. The pure-Rust Binius64 worker can also run on macOS.

## Migration validation (2026-09-12, macOS ARM64)

These are bounded correctness checks in development builds, not performance
comparisons. The native processor was Perfetto 58.2; proof smoke runs used two
Rayon threads.

- Default and no-default-feature libraries compile, as do minimal/full timing
  feature sets, integration tests, heap-instrumented benches, CLI binaries,
  diagnostic examples, both workers, and the standalone field benchmark.
- Shared Perfetto tests: 11 pass. Native SHA: 28 pass. After the metadata
  follow-up, reporting contracts pass 51 tests and native multiplication passes
  all 44 tests, including the formerly failing configuration assertion.
- The new Spartan2 pin passes six verified SHA/ECDSA trials with its internal
  phase intervals. Canonical PCS validation passes 18 runs / 9,702 spans across
  F2Z, Binius64 BaseFold, and F2Z-Ligerito binary adapters.
- All four hybrid modes produce one warmup plus five verified samples. The CLI
  multiplication run, the hybrid RSS diagnostic (six verified trials), and the
  native multiplication untraced memory child also pass.
- Standalone Binius64: one warmup plus five verified SHA/ECDSA samples pass the
  unchanged Python consumer, including all four original phase keys. Its phase
  projection regression and four shared-observability unit tests also pass.
- Python reporting-consumer tests pass all 51 tests after correcting the rate
  fields in the Ligerito caption fixtures. Production caption behavior is unchanged.

Metadata compatibility follow-up:

- The native multiplication test now validates the versioned identity and reads
  ladder fields under `configuration`, matching the producer's actual schema.
- Binius-Ligerito now emits `LIGERITO_CONFIG` using the typed
  `f2z/binius-ligerito-pcs/v1` identity. It records the whole-protocol target (100),
  the selected opener component target, and every oracle's actual ladder and
  Round-0 grinding settings. This is a PCS configuration identity, not a new
  security theorem or an alternative soundness analysis.
- The sweep validates each mode's own schema: binary ladders are re-derived from
  their packed sizes and component target; the existing single-opener identities
  retain their 106-bit hybrid / 112-bit separate budgets. Missing, duplicated,
  altered, and wrong-mode identities remain errors. CSV columns are unchanged.
- A fresh `--sweep --mode all --shapes 15:1 --iterations 1` completes all four
  verified samples (after each mode's warmup). Its 25-column combined CSV carries
  identities matching the emitted per-mode JSON files.

The zkPassport managed runtime was not exercised on this host: its pinned native
toolchain is Linux/x86_64-only. Its Rust worker passes `cargo check --locked
--offline`. Unit-test-only kernel microbenchmarks retain their own `Instant`
measurements; live benchmark, worker, example, and CLI reporting paths do not.
