# Benchmark reporting

`benches/common/output.rs` owns benchmark file opening and completion. It depends
only on std, serde, serde_json and csv, so isolated workers can include it with
`#[path]` without importing the prover. Benchmark calculations return typed Rust
records; JSON and CSV are serialization boundaries, not metric lookup tables.

## Using the shared output module

```rust,ignore
let output = BenchmarkOutput::new(results_dir);
output.create_dir_all()?; // Only when this caller's directory policy permits it.

let mut samples = output.jsonl("samples.jsonl", FileMode::CreateNew)?;
samples.write(&sample)?;
samples.flush()?; // Make this sample visible immediately.
samples.finish()?;

output.write_json("summary.json", &summary, FileMode::Replace, JsonStyle::Pretty)?;

let mut csv = output.csv("metrics.csv", FileMode::CreateNew)?;
csv.write_record(CsvRow::HEADER)?;
csv.serialize(&row)?;
csv.flush()?;
```

`CreateNew` rejects an existing file; `Replace` creates or truncates;
`AppendExisting` appends but never creates a missing file. Construction and file
opening do not implicitly create directories. Document JSON has no added newline;
JSONL has one newline per compact record. Document/text/byte helpers flush before
returning. Streaming callers explicitly flush at their existing visibility
boundaries and handle completion failures instead of relying on drop.

`csv_writer(stdout.lock())` uses the same CSV configuration for stdout. CSV has
its own buffer; do not wrap it in a second `BufWriter`. Headers are explicit and
automatic headers are disabled, preserving header-only tables. `file()` exposes
a raw handle for child stdout/stderr redirection; `buffered()` supports existing
text streams. Writers stay in benchmark orchestration, not prover APIs.

## File-sink inventory

All paths below are relative to the repository root. No benchmark-owned Rust
file opener remains outside `benches/common/output.rs`.

| Owner | Artifacts | File policy |
| --- | --- | --- |
| `benches/multiswap.rs` | Optional JSONL trace | Create new; create parents |
| `benches/sha256_compressions.rs` | Optional JSONL trace and text samples/summary | Replace; create parents |
| `benches/sha256_product_layout.rs` | Optional status file | Append existing |
| `benches/sha256_e2e_compare.rs` | Trace, summary, metrics CSV | Trace create new; summary/CSV replace; create directories |
| `benches/mul_bitz.rs`, `benches/mul_compare.rs` | Manifest, raw samples, worker logs | Create new campaign artifacts; Python owns reports |
| `benches/common/whir_tuning.rs` | Tuning records | Create new |
| `benches/support/sha256_ecdsa_fixture.rs` | Validated compact JSON fixtures, including worker exports | Replace; no implicit parents |
| `benches/hybrid_u32_sha256/runner.rs` (also hybrid binary) | Proof bytes, binary/text statement, configuration JSON; CSV on stdout | Replace; no implicit parents |
| `benches/hybrid_u32_sha256/sweep.rs` | Run metadata, child CSV/log capture, combined CSV, configuration JSON | New results directory; replace files inside it |

The Binius64 worker shares the fixture helper; its build-time source fingerprint
includes the output module. The worker manifest/lockfile carry the serializer
dependencies. Root benchmarks use the dev dependency and the hybrid binary uses
the optional `csv` dependency enabled by `hybrid`.

Remaining writes through raw/buffered handles are text status/results and child
redirection. Remaining comma splits/joins parse CLI lists or format non-CSV
labels; no handwritten CSV output or comma-splitting CSV reader remains.

## Record and format contracts

- Multiplication samples use integer proof sizes. The shared Python reporter
  computes medians by averaging the middle pair for even sample counts and
  uses Type 7 percentiles; see the [current format](native-mul-compare.md).
- SHA reuses typed trial metrics. SHA/ECDSA retains explicit null phase fields,
  method-specific details and its existing sample numbering.
- Tuning candidates and PCS campaign statuses are typed variants. Absent
  finalist fields remain omitted; explicit null metadata stays null.
- MultiSwap nanoseconds remain sparse decimal-string fields.
- Protocol/security/configuration payloads remain opaque JSON. Object key order
  can follow struct declaration order; array order, field meanings and numbers
  are unchanged.
- CSV-specific projections preserve native multiplication's JSON-number
  spellings, SHA's nine decimal places, and hybrid's three decimal places.
  Configuration strings receive standard CSV escaping.
- The hybrid sweep preserves child numeric text without parsing and reformatting
  floats. It validates the exact header, record width, mode, iteration sequence,
  sample count, proof size and RSS. Missing metrics stay empty; exact proof sizes
  and separate-mode payload estimates retain distinct columns.

Timing infrastructure, phase boundaries, formulas, protocol code and Python
consumers are outside this refactor.

The multiplication launcher invokes the shared reporter only after a completed,
verified `mul-bench/v2` campaign. Its manifest and samples carry provenance and
resolved configurations; historical multiplication formats are unsupported.
Other comparison runners retain their own reporting formats.

## Executable checks

```sh
cargo test --test benchmark_output
cargo test --test benchmark_reporting --test native_mul_compare --test native_sha256_compare \
  --features 'bench-internals,native-mul-compare,native-sha256-compare,sha256-ecdsa-compare,hybrid' reporting
cargo test --bin hybrid-u32-sha256 --features hybrid
cargo check --benches --bin hybrid-u32-sha256 \
  --features 'bench-internals,bench-peak-memory,native-mul-compare,native-sha256-compare,sha256-ecdsa-compare,hybrid'
cargo check --manifest-path benchmarks/binius64/Cargo.toml
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Reporting tests run through ordinary test harnesses, not `harness = false`
benchmark entry points. They cover file policies, binary fidelity, stdout,
write/flush errors, JSON field contracts, CSV exact output and invalid child
records. Run only bounded smoke cases, not full campaigns, for I/O validation.

For the vendor normalization change, only compile checks are performed. Use
`bash scripts/compile_export.sh` to build and link the retained campaign targets
and affected tests without running them. Tables default to `outputs/tables/`
and figures to `outputs/figures/`; no root `paper/` directory is needed.

## Opt-in Perfetto interval capture

Native multiplication records per-trial phase totals in `samples.jsonl`; see
[the multiplication interface](native-mul-compare.md). For native SHA comparison,
`bench-perfetto` also saves diagnostic `.pftrace` files beside the normal
artifacts (or beside a custom trace path). Open them locally in
<https://ui.perfetto.dev>.

`src/observability.rs` configures `tracing-perfetto-sdk` with an in-process
Perfetto session. It composes with the existing subscriber; it does not install
another global subscriber, launch a tracing service, parse JSON logs, or measure
time itself. The native SDK owns clocks, per-thread intervals and buffering.
Library builds without `span-metrics` do not compile or initialize Perfetto.
Native comparison features enable it for numeric metrics; `bench-perfetto` adds
saved diagnostic files. The optional native SDK
requires a C++ toolchain; the implementation was checked on macOS ARM64.

The integration is deliberately small:

```rust,ignore
let recording = Recording::start(output.buffered("trial.pftrace", FileMode::CreateNew)?)?;
let proof = tracing::info_span!("opening_proof", component = "pcs.opening")
    .in_scope(|| open(&prepared, &commitment))?;
recording.finish()?; // Outside the timed region; propagates write/flush errors.
```

Close every entered span and join worker tasks before `finish`. Fields should be
present before entry; a later `record` is reflected on subsequent entries, not
retroactively on the interval already emitted. Use `.in_scope` for synchronous
work and `.instrument` for futures, never an entered guard across `.await`.
Intervals describe entered wall time, not CPU time or entire async lifetimes.

Each session has a 64 MiB discard-on-full buffer. Flush success does not prove
that the trace fits that buffer. Before using a trace for analysis, check for
incomplete slices and nonzero error/data-loss statistics in the native processor:

```sh
trace_processor_shell trial.pftrace -Q 'SELECT name, ts, dur FROM slice'
trace_processor_shell trial.pftrace -Q 'SELECT * FROM slice WHERE dur < 0'
trace_processor_shell trial.pftrace -Q "SELECT * FROM stats WHERE value != 0 AND severity IN ('error', 'data_loss')"
```

The first query emits CSV with integer nanoseconds. It can be launched from Rust
with `std::process::Command` and read with `csv::Reader`; Python is not required.
Do not sum nested or parallel slice durations to obtain wall time. Group by trial,
exclude warmups, and union the selected intervals before aggregating across trials.

The diagnostic integration has these remaining limitations:

- BitZ's metrics and the setup/campaign timers remain on their legacy timing
  paths. Binius64, Binius64-Ligerito, Plonky3 and Limber now query bounded
  in-memory recordings for their native multiplication/SHA JSON/CSV metrics. Capture
  changes overhead; do not compare timings across the migration as a speedup.
- Only existing `tracing` instrumentation is exported. BitZ's `prof::scope` and
  manual phase timers are not translated into synthetic spans.
- `benchmark_trial` is an orchestration envelope, not `witness_to_proof_ms`.
  Multiplication includes verification and metric extraction; SHA also includes
  its canonical trace reporting. No semantic end-to-end tag is assigned to it.
- Diagnostic files for tuning/pilot, memory-only subprocess, and preflight runs
  are not enabled. Native preflight and WHIR candidates query in-memory recordings.
  The executable test proves a completed inner session can be queried while an
  outer recording remains open, but spawning the native processor per candidate
  has overhead. WHIR objectives use these completed trial measurements; its
  campaign-wide `tuning_ms` timer has not yet been migrated.
- Dropping a recording without `finish` does not publish a complete trace. An
  interrupted/panicking run may leave its reserved output file empty; do not
  include it in analysis.

The initial `tracing-profile` candidate was not selected: version 0.10.11 consumes
span metadata on first entry, cannot re-enter that span, and owns output files and
drop-time completion without our required writer/error interface. The selected
SDK adapter supports repeat/parallel entry and explicit session completion.

```sh
cargo test --test benchmark_perfetto --features bench-perfetto
PERFETTO_TRACE_PROCESSOR=/path/to/native/trace_processor_shell \
  cargo test --test benchmark_perfetto --features bench-perfetto -- --include-ignored
```

The native-processor test is explicitly ignored unless requested because it needs
the separately installed executable. It covers nesting, same-span re-entry across
threads, an unentered span, error returns, field updates before re-entry, trial and
warmup identity, interval union, missing/data-loss checks, and nested tuning
sessions. The ordinary tests cover protobuf output, repeated sessions, create-new
collisions, and injected write/flush failures.

Validated with SDK 1.1.1, adapter 1.0.0 and native trace processor 58.2 on macOS
ARM64. A bounded native-multiplication run used 16 operations, two threads, and
one warmup plus five samples for each of Binius64 and Plonky3 FRI. All 12 proofs
verified; all 12 traces loaded with no incomplete slices or error/data-loss
statistics (1,923 slices per Binius trace; 532 per Plonky3 trace). This was a
correctness smoke, not an overhead or performance comparison. SHA was compile-
checked with and without the feature, not runtime-tested. Linux execution and
Clippy remain unchecked; Clippy is not installed in the pinned 1.98.1 toolchain.

The separate profiler-skill JSONL validator rejects the unchanged native-mul
`trial: {kind, index}` representation: it expects `warmup_index`/`sample_index`.
That existing compatibility mismatch was not repaired in this integration; it
does not affect the native Perfetto checks above. Do not treat the legacy JSONL
as validated against that stricter schema.

## Binius64-Ligerito phase migration

`Prepared::prove` now returns `Result<Proof, Error>`, not a proof paired with
`ProveTimings`. The channel no longer accumulates commitment or Round-0 times.
Ordinary `tracing::info_span!` scopes surround the same protocol operations;
they record component identity and oracle indices, never witness contents.
No subscriber or writer is passed into the prover.

Both native multiplication and SHA derive their existing numeric metrics from
those completed spans using `BiniusLigeritoPhases` in `trace_capture.rs`:

- `commit`: the actual witness-oracle commitment interval, including packing and
  transcript absorption, as before.
- `piop`: the PIOP prefix minus the **union** of the witness commitment and all
  Round-0 intervals. Later oracle commitments remain included, as before.
- `opening`: the union of all Round-0 intervals and the final opening interval.

The JSON/CSV metric names, units and aggregation rules are unchanged. Canonical
trace placement is intentionally corrected: PIOP and opening can have several
disjoint intervals, with distinct span IDs, instead of three artificially
consecutive bars. Unmeasured gaps are not filled by shifting or clipping spans.
The broad PIOP row is not tagged as pure Sumcheck; it also includes other work.
Missing, duplicate or out-of-bounds required spans fail instead of becoming zero.

Perfetto exports these same annotations when enabled. Its `tag_commit` view
includes **all** oracle commitments; the historical benchmark `commit_ms`
column intentionally counts only the witness commitment. Do not equate them.

The initial migration removed the Binius64-Ligerito phase and trial-boundary
clocks. The subsequent native-adapter migration also removed the live collector.
One-time setup timers and BitZ's `prof::scope` calls still require migration;
memory reporting remains separate from timing.
Instrumentation overhead changes, so this is not a performance claim.

Trial-scope migration checks on macOS ARM64: all 27 SHA tests pass, and
multiplication passes 41 of 42 tests. The unchanged
`u32_comparison_requires_johnson_and_ood` test expects
`config.ligerito.target_security_bits`, which is null. The same failure reproduces
in the pre-migration test executable. It is not repaired by this timing change.
The new smoke tests exercise six verified trials each at 2,048 multiplications
and 32 SHA compressions; multiplication also checks proof-byte equality with
tracing on/off.
Both comparison benchmarks and the multiplication witness benchmark compile with
and without Perfetto. The preceding phase migration also passed all three
Binius64-Ligerito protocol tests, all eight output/Perfetto tests (including the
native processor), and the ten unchanged native-runner Python tests; it checked
the hybrid binary with and without Perfetto and the library without default
features. Linux remains untested.

An optimized multiplication smoke (2,048 operations, two threads, one warmup and
five samples) also verified all six proofs and produced six valid Perfetto files.
Each contains 66 complete slices, including the four trial scopes and two real
Round-0 intervals inside the PIOP prefix, with no error/data-loss statistics.
Native-processor checks confirm scope nesting, ordering, and warmup/sample
identity. The JSONL phase unions agree exactly with all corresponding numeric
sample metrics. This checks measurement structure and output, not tracing
overhead or relative performance.

### Trial scopes

The Binius64-Ligerito multiplication and SHA runners mark four ordinary spans:
`verified-trial`, `witness-to-proof`, `witness-evaluation`, and `verification`.
They no longer call `capture.now_ns()`. `BiniusLigeritoTrial` borrows the completed
spans, checks their nesting/order, and supplies endpoints to the existing report
projections. It does not read clocks or install another collector.

Proof encoding stays inside `witness-to-proof`; proof decoding stays inside
`verification`. The original witness, proof and decoded proof remain alive until
after the scopes close, so their destruction and report construction stay out
of the measured trial. JSON/CSV columns, units, warmup handling and aggregation
rules are unchanged. The existing online-prover interval runs from witness exit
to proof readiness, including encoding; post-proof accounting runs from proof
readiness to verification entry.

Each scope uses its own measured endpoints, including witness and verification.
The new annotations can introduce gaps between nested scope endpoints; those
gaps remain in their enclosing totals, not in the child-operation durations.
The verified-trial span is the explicit Perfetto end-to-end boundary, inside the
larger `benchmark_trial` orchestration span.

### Perfetto-backed numeric metrics

The shared `bitz::observability` module owns recording and native queries. All
Binius64, Binius64-Ligerito, Plonky3 and Limber native multiplication/SHA runners
use it, without `TraceCapture` or `CaptureLayer`. `trace_capture.rs` now contains
only reporting projections over native intervals; no timestamps, mutexes,
subscriber callbacks or collector state remain there. BitZ is still awaiting migration.

```rust,ignore
let recording = bitz::observability::Recording::start(Vec::new())?;
let result = tracing::info_span!("operation", component = "example.operation")
    .in_scope(|| operation())?;
let intervals = recording.intervals()?;
```

Install the layer once at the executable boundary, composing it with any other
subscriber layers. Every entered scope must exit before querying. Each interval
contains its unique slice ID, same-track parent, track ID, name, optional
component, and exact nanosecond endpoints relative to the recording's first
slice. Repeated entries remain separate intervals. Existing reporting projections
still compute overlap-safe unions and preserve numeric/CSV output contracts.

Set `PERFETTO_TRACE_PROCESSOR` to the installed native `trace_processor_shell`
executable, or put it on PATH. No download or Python wrapper is launched. The
Rust query interface pipes in-memory trace bytes through `/dev/stdin` on macOS
and Linux; other platforms return an explicit unsupported error. Missing
executables, query failures, incomplete slices, error/data-loss statistics,
malformed records and empty captures are errors, never zero timings or a
fallback to another collector. The native CLI's string quoting is normalized in
the SQL projection before standard CSV and typed JSON decoding.

Querying takes place after the measured scopes close, including after each
completed candidate in the nested-session integration test. It adds orchestration
latency, not operation duration. Native WHIR tuning receives each completed
trial's objective immediately, while campaign-level timers still await migration.
`bench-perfetto` can still save an outer diagnostic recording through the shared
writer. The multiplication memory-only child runs the same proof/verification
body without installing Perfetto or allocating a recording buffer; no timing
metrics are needed in that RSS-only pass. This applies to every migrated native
multiplication adapter, including Plonky3 and Limber.

```sh
PERFETTO_TRACE_PROCESSOR=/path/to/native/trace_processor_shell \
  cargo test --test benchmark_perfetto --features bench-perfetto -- --include-ignored --test-threads=1
PERFETTO_TRACE_PROCESSOR=/path/to/native/trace_processor_shell RAYON_NUM_THREADS=2 \
  cargo test --features bench-internals,native-mul-compare,native-sha256-compare \
  --test native_mul_compare --test native_sha256_compare \
  span_metrics_cover_repeated_verified -- --include-ignored --test-threads=1
```

The native query suite passes all seven tests on macOS ARM64 with processor
58.2, including malformed/empty/incomplete captures, exact integers and string
escaping, same-span parallel/repeated entry, and querying an inner candidate
while an outer session is open. Both Binius64-Ligerito smoke tests pass without
the custom layer (one warmup plus five samples each). The full SHA suite passes
27 tests; multiplication passes 41 of 42, with the same pre-existing security
configuration failure described above. The memory-only path also verifies with
`NoSubscriber`. Linux runtime and Windows support remain unchecked; no overhead
or performance claim follows from these correctness checks.

The subsequent native-adapter migration passes six real trials for each of
Binius64, Plonky3-FRI, Plonky3-WHIR and Limber multiplication, and for Binius64,
Plonky3-WHIR and Limber SHA. The SHA fixture uses 128 compressions for WHIR's
padding floor and two for the other adapters. The corrected smoke passes, as
do the other 27 SHA tests; multiplication passes 42 of 43 with the unchanged
configuration assertion above. Native comparison targets also compile with
`bench-perfetto`. These are debug correctness checks, not performance results.
