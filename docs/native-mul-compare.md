# Multiplication benchmarks

Use two Cargo benchmark targets. `mul_f2z` measures F2Z experiments;
`mul_compare` measures native proof systems and PCS comparisons. Shared modules
under `benches/mul/` own flags, validation, case expansion, workers, and raw output.
No multiplication runner scripts or environment aliases are needed.

```sh
cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  proof --workload u64 --log-n 15 --w 3

cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  proof --workload u32-full,u64,u128 --log-n 15..=20 \
  --w 1,3,8 --split=0,1 --threads 1,8 --f2z-profile 100,128 \
  --reps 5 --out results/f2z --dry-run

cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  proof --workload u32-mod32,u64,u128 --backends all --log-n 15..=20 \
  --threads 1,8 --skip-unsupported --out results/compare
```

Remove `--dry-run` to execute. Numeric sweep axes accept scalars, comma-separated
lists, and inclusive ranges, including negative split shifts (`--split=-1..=1`).
Repeated values, unknown names, malformed values, and an empty runnable sweep
are errors. Cases execute sequentially in fresh processes with explicit thread
counts. F2Z axes never multiply the competitor cases.

`RUSTFLAGS`, Cargo profiles, and features remain ordinary build settings. For
example, prefix a command with `RUSTFLAGS="-C target-cpu=native"`. Latency phase
capture requires the native Perfetto trace processor on PATH, or its path in
`PERFETTO_TRACE_PROCESSOR`. The benchmark never downloads one.

## Modes and defaults

| Target / mode | Boundary | Default sizes | Reps / warmups |
|---|---|---|---|
| F2Z `proof` | Witness and public preparation excluded; pack, commit, and full proof included | 15..=25 | 5 / 1 |
| Compare `proof` | Native witness generation through proof completion; verification separate | 15 | 5 / 1 |
| `witness` | Native witness construction; row audit outside timer | 10 | 5 / 1 |
| `pcs` | Materialize, commit, and open a terminal claim; claim setup and verify separate | u32: 15..=25; BabyBear: 15..=24 | 21 / 1 |
| F2Z `piop` | Whole fixed-prime Spartan PIOP; projected inputs prepared outside timer | 15 | 5 / 1 |
| F2Z `outer` | Outer sumcheck kernel; construction and verification excluded | 12,15,17,19 | 5 / 1 |
| F2Z `outer --preset regression` | Paired production/generic calls with their own preparation boundaries, identical proofs required | 12,15,17,19 | 5 / 1 |
| F2Z `bounds` | Witness-to-proof with selected decoding bound and opening codec roundtrip | 15 | 5 / 1 |

Defaults are recorded in every resolved case. Standalone BabyBear proof runs
both profiles 100 and 128 unless `--f2z-profile` selects one; other workloads
default to 100. Default packing is W=1 and split=0. Bounds defaults to eight
threads; other modes default to available parallelism. Set `--threads` for
comparisons. `--reps` and `--warmups` override trial counts.

```sh
cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  witness --workload u32-full,u64,u128,baby-bear --log-n 10 --threads 1

cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  pcs --workload u32-full,baby-bear --backends all --log-n 15 --threads 8

cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  piop --workload u32-full --log-n 15 --variants standard,skip1,skip2,skip3,skip4

cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  outer --workload u32-full,u64,u128 --log-n 12 --preset regression

cargo bench --bench mul_f2z --features span-metrics,bench-internals -- \
  bounds --workload u32-full,u64,u128,baby-bear --bound johnson,unique --log-n 15
```

Outer `current` defaults to all three integer widths plus the `field` baseline
and also supports `zero`; regression compares production and generic
implementations for standard/skip variants. `unique` includes fold grinding;
`unique-ungrinded` selects the corresponding ungrounded bound. Bounds uses a
shared BLAKE3 XOF corpus across widths. Witness mode omits security profile axes.

## Workloads and backend settings

`u32-full` proves the exact 64-bit product; `u32-mod32` compares independent
wrapping products. F2Z's wrapping comparison includes the high product limb in
its full integer relation. `u64` and `u128` retain full double-width products.
BabyBear uses `a*b = c + p*k` with p=2013265921.

`--ligerito custom:1:4,udrg:1:4` selects explicit F2Z opener configurations;
`custom:3:4` selects Johnson decoding at rate 1/8. Use `--bound` or
`--ligerito`, not both. The profile determines the security target.

Integer F2Z workloads reuse `MulLayout<T>` and `MulWitness<T>`. W is a logical
packing width; the layout validates its shape-dependent upper bound. W=1 and
u32 W=8 open directly; other configurations use virtual packing. Results record
both committed and opening geometries, not just W. BabyBear and the fixed-prime
PCS experiment retain W=1/split=0; PCS uses profile 100.

Native proof backends are `f2z`, `binius64`, `binius64-ligerito`, `plonky3-fri`,
`plonky3-whir`, and `limber`. Plonky3 native multiplication supports u32-mod32.
PCS backends are `f2z`, `plonky3-whir`, `binius64-basefold`, and
`f2z-ligerito-binary`, on u32-full or BabyBear witnesses.

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
cargo bench --bench mul_f2z --features span-metrics,bench-internals,bench-peak-memory -- \
  proof --workload u64 --log-n 15 --w 3 --threads 8 --memory heap --out results/heap
```

Instrumented builds cannot emit latency samples. Heap peaks cover one verified
trial after preparation, include its live baseline allocations, and exclude
reporting. Native WHIR heap runs require explicit parameters from the latency
campaign. Heap-only campaigns have memory records, not fabricated timings.

Each new output directory contains `manifest.json`, `samples.jsonl`, and worker
logs. Existing artifacts are never overwritten by the benchmark. The manifest
has schema `mul-bench/v1`, status, provenance, and complete resolved cases. Cases
include mode, workload, backend, size, root seed, threads, and every applicable
configuration. Effective metadata records the corpus, measurement boundary,
packing, security, and selected backend parameters. Provenance includes source
and executable hashes, compiled feature flags, lockfile identity, and machine
information. Samples reference their case ID and record kind, index, verification,
and per-trial metrics. Skips name their case and reason.

Python is the only report aggregator:

```sh
python3 scripts/mul_report.py results/compare --out reports/compare
python3 scripts/mul_report.py results/f2z --select w=3 --select threads=8 \
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
