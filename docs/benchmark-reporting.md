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
| `benches/mul_e2e_compare.rs` (also witness benchmark) | Trace, samples, memory samples, summary, metrics CSV, witness checks/summary | Create new; create output directory |
| `benches/sha256_e2e_compare.rs` | Trace, summary, metrics CSV | Trace create new; summary/CSV replace; create directories |
| `benches/baby_bear_pcs_compare.rs` | Trace or stdout; campaign manifest | Create new; create parents |
| `benches/u32_pcs_compare.rs` | Trace or stdout | Create new; create parents |
| `benches/common/whir_tuning.rs` | Tuning records | Create new |
| `benches/support/sha256_ecdsa_fixture.rs` | Validated compact JSON fixtures, including worker exports | Replace; no implicit parents |
| `benches/hybrid_u32_sha256/runner.rs` (also hybrid binary) | Proof bytes, binary/text statement, configuration JSON; CSV on stdout | Replace; no implicit parents |
| `benches/hybrid_u32_sha256/sweep.rs` | Run metadata, child CSV/log capture, combined CSV, configuration JSON | New results directory; replace files inside it |
| `benchmarks/zkpassport/src/main.rs` | SRS cache | Replace temporary file, then rename; create cache directory |

The Binius64 worker shares the fixture helper; its build-time source fingerprint
includes the output module. Both worker manifests/lockfiles carry the serializer
dependencies. Root benchmarks use the dev dependency and the hybrid binary uses
the optional `csv` dependency enabled by `hybrid`.

Remaining writes through raw/buffered handles are text status/results and child
redirection. Remaining comma splits/joins parse CLI lists or format non-CSV
labels; no handwritten CSV output or comma-splitting CSV reader remains.

## Record and format contracts

- Native multiplication samples use integer proof sizes; summary medians use
  floating-point proof sizes and retain the existing upper-middle median.
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

The existing Python campaign runner still adds provenance and protocol
fingerprints before invoking paper exporters. Raw Rust `summary.json` files
are not a substitute for that enriched campaign output.

## Executable checks

```sh
cargo test --test benchmark_output
cargo test --test benchmark_reporting --test native_mul_compare --test native_sha256_compare \
  --features 'bench-internals,native-mul-compare,native-sha256-compare,sha256-ecdsa-compare,hybrid' reporting
cargo test --bin hybrid-u32-sha256 --features hybrid
cargo check --benches --bin hybrid-u32-sha256 \
  --features 'bench-internals,bench-peak-memory,native-mul-compare,native-sha256-compare,sha256-ecdsa-compare,hybrid'
cargo check --manifest-path benchmarks/binius64/Cargo.toml
cargo check --manifest-path benchmarks/zkpassport/Cargo.toml
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Reporting tests run through ordinary test harnesses, not `harness = false`
benchmark entry points. They cover file policies, binary fidelity, stdout,
write/flush errors, JSON field contracts, CSV exact output and invalid child
records. Run only bounded smoke cases, not full campaigns, for I/O validation.

On the development macOS/ARM host both worker compile checks pass. The zkpassport
worker's Linux-native backend was not executed here. The unchanged Python suite
currently has two existing `test_ligerito_results` errors because its historical
fixture lacks `configuration.levels[].log_inv_rate`; the other 49 tests pass.

Bounded end-to-end checks also passed: native multiplication with 16 operations,
Plonky3 FRI, one warmup and one measured sample (including the isolated memory
pass); and an all-Binius hybrid sweep with 512 multiplications, two chained SHA
compressions and one measured sample. These exercised real proof verification,
JSON/JSONL/CSV files, subprocess log capture and combined CSV output.
