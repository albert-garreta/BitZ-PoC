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

## Optional Perfetto export

Numeric measurements use the Rust collector described in [span measurements](span-metrics.md).
`span-metrics` enables that collector without the native SDK. `bench-perfetto`
adds the optional SDK and C++ build requirement; it does not change the numeric source.
Native SHA comparison can save diagnostic `.pftrace` files beside its usual
artifacts. Open them locally in <https://ui.perfetto.dev>.

```rust,ignore
use bitz::observability::perfetto::TraceRecording;
let export = TraceRecording::start(output.buffered("trial.pftrace", FileMode::CreateNew)?)?;
let proof = tracing::info_span!("opening_proof", component = "pcs.opening")
    .in_scope(|| open(&prepared, &commitment))?;
export.finish()?; // Outside the measured work; propagates write/flush errors.
```

Install the span layer once at the executable boundary. The native SDK initializes
only when an export starts. Close entered spans and join workers before finishing.
Dropping an unfinished export does not publish a complete trace. The native
export buffer is 64 MiB and can lose events when full, so use bounded trials and
check exported traces for incomplete slices before interpreting them. Numeric
collector overflow is independently detected and returned as an error.

```sh
cargo test --test benchmark_spans --features span-metrics
cargo test --test benchmark_perfetto --features bench-perfetto
```

Both suites run without an external trace processor. The first checks numeric
collection and queries; the second checks protobuf export, output creation policy,
and propagated write/flush errors.

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
The verified-trial span is the explicit span end-to-end boundary, inside the
larger `benchmark_trial` orchestration span.

### In-process numeric metrics

The shared `bitz::observability` module owns the span collector and interval queries.
BitZ, Binius64, Binius64-Ligerito, Plonky3 and Limber adapters all use it.
`trace_capture.rs` contains reporting projections only.

```rust,ignore
let recording = bitz::observability::Recording::start()?;
let result = tracing::info_span!("operation", component = "example.operation")
    .in_scope(|| operation())?;
let intervals = recording.intervals()?;
let elapsed = bitz::observability::duration(&intervals, "example.operation")?;
```

Every entered scope must exit before querying. Intervals contain unique entry
IDs, same-thread execution parents, track IDs, names, optional components, and
integer nanosecond endpoints. Repeated entries remain separate. Reports union
overlapping intervals of the same label across threads rather than double counting.
Collection, accuracy limits, and explicit failure behavior are specified in
[span measurements](span-metrics.md).

Queries run after the measured work and can complete inside an outer recording,
as needed for tuning candidates. No subprocess is launched. Multiplication's
memory-only child runs the same proof/verification body without a timing collector.
New SHA/ECDSA and SHA-chain rows identify `timing: "spans"`; multiplication records
it in provenance. Historical result files keep their original measurement labels.
