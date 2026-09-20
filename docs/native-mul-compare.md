# Multiplication benchmarks

Use two Cargo benchmark targets. `mul_bitz` measures BitZ experiments;
`mul_compare` measures native proof systems and PCS comparisons. Shared modules
under `benches/mul/` own flags, validation, case expansion, workers, and raw output.
The Python launcher below orchestrates these targets; experiment configuration
is owned by Rust, with no legacy environment aliases.

```sh
cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  proof --workload u64 --log-n 15 --w 3

cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  proof --workload u32-full,u64,u128 --log-n 15..=20 \
  --w 1,3,8 --split=0,1 --threads 1,8 --bitz-profile 100,128 \
  --reps 5 --skip-unsupported --out results/bitz --dry-run

cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  proof --workload u32-mod32,u64,u128 --backends all --log-n 15..=20 \
  --threads 1,8 --skip-unsupported --out results/compare
```

Remove `--dry-run` to execute. Numeric sweep axes accept scalars, comma-separated
lists, and inclusive ranges, including negative split shifts (`--split=-1..=1`).
Repeated values, unknown names, malformed values, and a sweep with no runnable
cases are errors. Unsupported configurations fail preflight unless
`--skip-unsupported` records them alongside the runnable cases. Cases execute sequentially in fresh processes with explicit thread
counts. BitZ axes never multiply the competitor cases.

Preflight includes security-profile feasibility. For example, u32-full at
log-n=15, W=8, profile 128 exceeds the projection grinding cap. Large BabyBear
128-bit cases can also exceed it. These are recorded skips when requested; a
proof or verification failure always stops the campaign.

`RUSTFLAGS`, Cargo profiles, and features remain ordinary build settings. For
example, prefix a command with `RUSTFLAGS="-C target-cpu=native"`. Latency phase
capture requires the native Perfetto trace processor on PATH, or its path in
`PERFETTO_TRACE_PROCESSOR`. The benchmark never downloads one.

## General launcher

```sh
python3 scripts/run_multiplication_benchmarks.py bitz --output results/bitz -- \
  proof --workload u64 --w 3 --threads 1
python3 scripts/run_multiplication_benchmarks.py compare --output results/compare -- \
  pcs --workload u32-full,baby-bear --backends all --log-n 15 --threads 1
python3 scripts/run_multiplication_benchmarks.py bitz --features bench-peak-memory -- \
  proof --workload u64 --log-n 15 --threads 1 --memory heap
```

Choose `bitz` or `compare` explicitly to preserve their measurement boundaries.
Everything after `--` is forwarded to the selected Rust CLI. Use launcher
`--output` instead of the benchmark's `--out`; `--worker` is reserved for Rust.
Numeric ranges, unsupported combinations, and mode validation are handled by
Rust. The launcher has no paper preset, backend size ceilings, or case expansion.

Python builds the selected target once using Cargo's bench profile and locked
dependencies, then runs through the machine gate with a 12 GiB swap-growth limit.
Use `--no-gate` or `--swap-grow-gb` before `--` to change that policy. Extra Cargo
features are accepted with repeatable `--features`; instrumented heap binaries
must use `--memory heap`. Existing build settings are retained; if neither form
of Rust flags is set, the launcher uses `-C target-cpu=native`. A locally installed
Perfetto processor is discovered automatically, or set `PERFETTO_TRACE_PROCESSOR`.

Launcher `--dry-run` before `--` prints planned commands without side effects.
Forwarded `--help` and benchmark `--dry-run` may build the executable but do not
acquire the gate, reserve results, or generate reports. The latter validates and
prints Rust's expanded cases. During execution, failures preserve logs and raw
artifacts, return a failing exit code, and never emit a success report. Interrupts
stop workers before the gate releases its lock.

Results default to a fresh `PerfRuns/<timestamp>-multiplication` directory.
`build.log` and `run.log` accompany the Rust `manifest.json`, `samples.jsonl`, and
worker files; shared tables and figures are generated under `reports/` after a
complete verified campaign. The format is `mul-bench/v2`, with `bitz` configuration
fields. Previous formats are rejected; historical artifacts are never rewritten.

## Modes and defaults

| Target / mode | Boundary | Default sizes | Reps / warmups |
|---|---|---|---|
| BitZ `proof` | Witness and public preparation excluded; pack, commit, and full proof included | 15..=25 | 5 / 1 |
| Compare `proof` | Native witness generation through proof completion; verification separate | 15 | 5 / 1 |
| `witness` | Native witness construction; row audit outside timer | 10 | 5 / 1 |
| `pcs` | Materialize, commit, and open a terminal claim; claim setup and verify separate | u32: 15..=25; BabyBear: 15..=24 | 21 / 1 |
| BitZ `piop` | Whole fixed-prime Spartan PIOP; projected inputs prepared outside timer | 15 | 5 / 1 |
| BitZ `outer` | Outer sumcheck kernel; construction and verification excluded | 12,15,17,19 | 5 / 1 |
| BitZ `outer --preset regression` | Paired production/generic calls with their own preparation boundaries, identical proofs required | 12,15,17,19 | 5 / 1 |
| BitZ `bounds` | Witness-to-proof with selected decoding bound and opening codec roundtrip | 15 | 5 / 1 |

Defaults are recorded in every resolved case. Standalone BabyBear proof runs
both profiles 100 and 128 unless `--bitz-profile` selects one; other workloads
default to 100. Default packing is W=1 and split=0. Bounds defaults to eight
threads; other modes default to available parallelism. Set `--threads` for
comparisons. `--reps` and `--warmups` override trial counts.

```sh
cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  witness --workload u32-full,u64,u128,baby-bear --log-n 10 --threads 1

cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  pcs --workload u32-full,baby-bear --backends all --log-n 15 --threads 8

cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  piop --workload u32-full --log-n 15 --variants standard,skip1,skip2,skip3,skip4

cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  outer --workload u32-full,u64,u128 --log-n 12 --preset regression

cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  bounds --workload u32-full,u64,u128,baby-bear --bound johnson,unique --log-n 15
```

Outer `current` defaults to all three integer widths plus the `field` baseline
and also supports `zero`; regression compares production and generic
implementations for standard/skip variants. `unique` includes fold grinding;
`unique-ungrinded` selects the corresponding ungrounded bound. Bounds uses a
shared BLAKE3 XOF corpus across widths. Witness mode omits security profile axes.

## Workloads and backend settings

`u32-full` proves the exact 64-bit product; `u32-mod32` compares independent
wrapping products. BitZ's wrapping comparison includes the high product limb in
its full integer relation. `u64` and `u128` retain full double-width products.
BabyBear uses `a*b = c + p*k` with p=2013265921.

`--binius-ligerito-accounting union|rbr` selects whole-protocol union-bound
or round-by-round accounting for `binius64-ligerito` proofs (default `union`).
The choice is recorded in the case identity and report label. Use `rbr` to
reproduce the accounting used by the dated comparison campaign.

`--ligerito custom:1:4,udrg:1:4` selects explicit BitZ opener configurations;
`custom:3:4` selects Johnson decoding at rate 1/8. Use `--bound` or
`--ligerito`, not both. The profile determines the security target.

Integer BitZ workloads reuse `MulLayout<T>` and `MulWitness<T>`. W is a logical
packing width; the layout validates its shape-dependent upper bound. W=1 and
u32 W=8 open directly; other configurations use virtual packing. Results record
both committed and opening geometries, not just W. BabyBear and the fixed-prime
PCS experiment retain W=1/split=0; PCS uses profile 100.

The Zinc+ rows are measured outside this crate by `scripts/run_zinc_plus_campaign.py`
(see `benchmarks/zinc-plus/README.md`) and imported as `mul-bench/v2` cases with the
`witness-to-proof-external` boundary: their prover time is one number (commitment
included) and their peak resident set is the whole worker process.

Cells that exhaust the machine's memory are excluded, not measured while paging.
`scripts/mul_memory_probe.py` runs one such cell alone under a small swap-growth
guard, samples the process tree's peak resident set, and records the cell as
`measured` or `excluded` (with the observed peak and the installed memory) in a
JSON-lines file; `scripts/mul_table.py --exclusions` prints excluded cells as `--`
and states the reason in the caption.

Native proof backends are `bitz`, `binius64`, `binius64-ligerito`, `plonky3-fri`,
`plonky3-whir`, and `limber`. Plonky3-FRI proves u32-mod32 through its wrapping
AIR and u64/u128 through a full-product AIR over 16-bit limbs with a carry
chain (`benches/mul/native/wide_mul_air.rs`, 382 and 813 trace columns, bit
range checks like the u32 AIR); Plonky3-WHIR remains wired to u32-mod32 only.
PCS backends are `bitz`, `plonky3-whir`, `binius64-basefold`, and
`bitz-ligerito-binary`, on u32-full or BabyBear witnesses.

`--log-inv-rate` controls applicable competitor rates; `--limber-bits` controls
Limber's Brakedown target (default 100). Binius retains its query-phase security
scope; the Ligerito native adapter uses whole-protocol union-bound accounting.
These scopes are recorded, not conflated into a common claimed security model.

Native WHIR tunes eligible configurations when no override is supplied.
`--tuning-reps` controls finalist trials. To choose/replay a configuration, use
`--whir-degree 5 --whir-folding 4 --whir-pow 12 --log-inv-rate 1`, optionally
`--whir-rate-cap 4`. All selected parameters and security schedules are recorded.
PCS WHIR uses its compiled field extension and size-dependent folding default;
unsupported degrees and `--whir-rate-cap` are rejected. Automatic native WHIR
selection searches rates 1/2, 1/4, and 1/8 unless `--log-inv-rate` restricts it.
Configuration ineligibility is an error unless `--skip-unsupported` is set;
failed proofs always abort, including during tuning.

## Memory and results

`--memory rss` adds an isolated one-proof worker to each latency case. It uses
the exact resolved configuration, including the WHIR selection, without tuning
or recording profiler buffers. RSS includes corpus construction, public setup,
and verification. The parent rejects mismatching latency/memory configurations.

Use a separate build and output directory for heap measurements:

```sh
cargo bench --bench mul_bitz --features span-metrics,bench-internals,bench-peak-memory -- \
  proof --workload u64 --log-n 15 --w 3 --threads 8 --memory heap --out results/heap
```

Instrumented builds cannot emit latency samples. Heap peaks cover one verified
trial after preparation, include its live baseline allocations, and exclude
reporting. Native WHIR heap runs require explicit parameters from the latency
campaign. Heap-only campaigns have memory records, not fabricated timings.

Each new output directory contains `manifest.json`, `samples.jsonl`, and worker
logs. Existing artifacts are never overwritten by the benchmark. The manifest
has schema `mul-bench/v2`, status, provenance, and complete resolved cases. Cases
include mode, workload, backend, size, root seed, threads, and every applicable
configuration. Effective metadata records the corpus, measurement boundary,
packing, security, and selected backend parameters. Provenance includes source
and executable hashes, compiled feature flags, lockfile identity, and machine
information. Samples reference their case ID and record kind, index, verification,
and per-trial metrics. Skips name their case and reason.

Python is the only report aggregator:

```sh
python3 scripts/mul_report.py results/compare --out reports/compare
python3 scripts/mul_report.py results/bitz --select w=3 --select threads=8 \
  --metric online_prover_ms --out reports/w3
```

The tool emits JSON, CSV, Markdown, LaTeX, and SVG figures from the same validated
records. It rejects incomplete campaigns, duplicate/missing/unverified samples,
and malformed metrics. Cases with different W, split, profile, thread, seed,
backend configuration, or provenance remain separate. Warmups are excluded;
even medians average the middle pair and percentiles use Type 7. PCS totals are
calculated per trial before aggregation. Proof sizes come from verified runs;
there are no manual overrides or readers for historical formats. Existing
historical result files are left untouched.

### GKR schedules and diagnostics

`--gkr-schedule auto` selects a deterministic storage schedule from public geometry,
claim path, and thread count. Explicit `l2`, `l4`, and `l8` values accept comma-separated
sweeps. The schedule axis applies only to BitZ experiments that run GKR; it does not
multiply witness-only or competitor cases. The manifest records the requested policy
and every distinct resolved forest geometry/schedule in `effective.gkr_schedules`.
RSS and heap workers use the same policy and must reproduce those resolutions.
The measured selection rule and its limits are documented in
[gkr-schedule-validation.md](gkr-schedule-validation.md).

```sh
cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  proof --workload u32-full,u64,u128 --log-n 15,19 --threads 1,8 \
  --gkr-schedule l2,l4,l8 --proof-fingerprints --out results/schedules
```

`--proof-fingerprints` attaches proof-message and transcript digests to verified proof
samples and warmups. Hashing runs outside measured boundaries and is disabled in
memory workers. GKR time is the existing `prove/mc:forest_ms` phase total. Warmups
remain excluded from ordinary report statistics; comparison tools may report the
first proof separately.

`compare_prover_workloads.py` asks the Rust targets to expand multiplication cases
with `--dry-run`, then alternates frozen revision executables. It uses the same
validated campaign loader as `mul_report.py`; it does not support historical
multiplication log formats. Standalone proving and witness-to-proof comparisons
retain distinct timing boundaries.

Memory campaigns use the Rust workers described above and the same `mul_report.py`
loader. The obsolete capture-example memory runner has been removed.
