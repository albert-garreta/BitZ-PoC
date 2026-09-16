# Outer sumcheck optimization results — Apple M1 Max

The retained change parallelizes the generic native-to-field first fold. Production native kernels, the generic skip prefix, task-size/cutoff constants, equality-factor accumulation, field arithmetic, and proof/transcript formats are unchanged. No public types, trait bounds, or scratch/workspace APIs were added.

## Decisions

| Experiment | Outcome |
| --- | --- |
| Parallel generic first fold | Retained: substantial improvement on larger parallel workloads. The three tables share one parallel traversal over disjoint output chunks. |
| Parallel generic skip prefix | Not retained: substantial large-input gains, but repeatable small-input slowdowns against the original source failed the regression gate. |
| First-fold/coefficient fusion | Not retained. Chunk fusion was 6.3–6.5% slower overall in the focused ablation; pair-at-a-time fusion was 11.2–11.9% slower. Adding fusion to parallel folding gave less than the required 5% incremental benefit and introduced regressions in some cases. |
| Scratch reuse between proofs | Kept only as a private experiment. Overall latency changed by +0.12% / −0.12% in the two focused sessions. It saved eight allocations per ordinary proof but retained the table storage between calls. |
| Task-size/cutoff and equality-accumulation tuning | Canceled at the user's request. Early scouting logs are preserved; no such changes were retained. |

The private scratch prototype passed reuse checks across changing sizes and field contexts, recovery after shape errors, and concurrent proofs with separate workspaces. At 524,288 rows, ordinary scratch retained about 18.02 MiB: the smaller *incremental* peak during the next call is not an 18 MiB reduction in total memory footprint.

The experimental parallel prefix accumulation allocates small task-local messages. The memory pass observed more allocation calls, but the same peak heap usage as the baseline at the measured shapes. Ordinary parallel folding added no allocations.

## Final retained change versus original source

This confirmation compares only the retained first-fold parallelism against the original library source. It uses the public-API benchmark with 10 threads, the same 96 input/size/protocol cases listed below, 21 measured proofs per case, and two process sessions in opposite order (original/final, then final/original). Each call verifies its proof after timing; the unit tests separately compare serial and concurrent proof values and transcript digests.

| Protocol family | Latency reduction, session 1 | Latency reduction, session 2 |
| --- | ---: | ---: |
| Ordinary | 38.12% | 39.06% |
| Zerocheck | 40.30% | 40.58% |
| Skip K=1..4 (source unchanged) | −0.39% | 0.74% |

These are geometric means of per-case median latency ratios. No case was more than 3% slower in both sessions. `final-source.csv` contains the 192 per-session case summaries with medians and P10–P90 sample deciles. The raw `source-fixed-*` logs contain 8,064 measured proofs across the original/final builds; warmups are excluded. The earlier `source-*` logs refer to the rejected prefix candidate, not this retained result.

A separate one-thread guard used all four input representations, 4,096 and 131,072 rows, ordinary/zerocheck, and 21 samples in each of two reversed-order sessions. Ordinary latency reductions were −0.30% / −0.14%; zerocheck reductions were −1.18% / +0.98%. No case was more than 3% slower in both sessions (`final-serial.csv`).

Both archived experimental patches also pass `git apply --check` against an isolated copy of `ef7a8935`.

The retained production change is **26 net Rust lines** in `src/sumcheck/outer/api.rs`; the rest is tests, benchmark support, documentation, and archived experiments. It adds no extra input copies, no allocation beyond the existing output, and no mutable input API. This is a generic outer-API improvement, not a measured whole-F2Z speedup.

## Experimental comparison (includes the rejected prefix optimization)

Baseline commit: `ef7a8935`. Rust 1.98.1, aarch64 macOS, Apple M1 Max, 64 GiB RAM. Bench profile: fat LTO, one codegen unit, debug symbols. No x86 performance claim is made.

The main interleaved experiment used 4,096, 32,768, 131,072, and 524,288 rows; 1, 4, and 10 Rayon threads; field elements plus `u32/u64`, `u64/u128`, and `u128/Uint<4>` A/B versus C inputs; ordinary, zerocheck, and skip K=1..4 protocols. Each configuration has one reference/parity call, one excluded warmup per candidate, and 21 measured calls. Two process sessions reverse the initial candidate order. All 24,192 measured comparison proofs verified and matched baseline proof values and continuing transcript digests.

The following numbers are reductions in geometric-mean latency across the stated input/size matrix, not whole-F2Z speedups:

| Threads | Ordinary, sessions 1 / 2 | Zerocheck, sessions 1 / 2 | Skip K=1..4, sessions 1 / 2 |
| --- | ---: | ---: | ---: |
| 1 | +0.03% / −0.19% | −0.41% / +0.24% | −0.28% / +0.01% |
| 4 | 28.32% / 28.52% | 31.10% / 31.28% | 52.50% / 52.80% |
| 10 | 38.21% / 34.00% | 41.49% / 36.42% | 66.18% / 62.95% |

No individual matrix case in this private comparison was more than 3% slower in both sessions. However, this harness also changed baseline plumbing; it was not sufficient to approve the production change. `confirmation.csv` contains per-case medians and P10–P90 sample deciles. These are the controlled experimental implementations; final-source comparisons are recorded separately below.

Measurement includes allocations performed inside the outer call. Input generation, field/skip preparation, the verifier, comparison checks, and output printing are outside the timer. No allocator instrumentation was enabled for latency runs. Memory statistics come from a separate allocator-enabled binary. Larger tables remain immutable and borrowed; there is no in-place compaction or input table cloning.

## Original-source regression check

We rebuilt the actual `ef7a8935` library source in an isolated checkout using the same public-API benchmark, toolchain, dependency sources, and compilation profile. The first direct comparison still showed large average gains with both first-fold and prefix parallelism, but found repeatable regressions in serial prefix fallbacks for `u128/Uint<4>` inputs: skip K=2 and K=4 at 4,096 rows, and K=4 at 32,768 rows. See `source-before-serial-fix.csv`.

Follow-up attempts changed accumulator borrowing, isolated serial loops, shared arithmetic macros, and selected serial/parallel instantiations once at entry. None cleared the regression check consistently. Those observations do not establish a precise compiler-level cause. We restored the prefix implementation exactly to the original source; the retained first-fold change is evaluated separately. `prefix-const-attempt.patch` archives the final failed prefix attempt, while `candidates.patch` preserves the original experimental implementations.

Promotion requires a reproducible improvement of at least 5% and no case over 3% slower in both confirmation sessions. These are local acceptance thresholds, not statistical confidence bounds.

## Native production reference

`native-summary.csv` records two alternating baseline/final runs of `u32_mul_outer_skip`: 21 measured proofs after a warmup, 10 threads, exponents 15/17/19, standard and skip K=1..4. This check used the earlier candidate build, before rejecting prefix parallelism; the retained change also touches only the generic API. Every proof verified. The benchmarks showed timing drift in unchanged inner/PIOP work as well as outer work, so no native speedup is claimed.

The original and tested experimental-candidate native benchmark binaries have **byte-identical `__TEXT,__text` sections** (2,026,240 bytes; SHA-256 `af43278eb208a7ee9790081306d315f65015db346068c546717abe23a8965e04`). Some whole-PIOP medians fluctuated by more than 3%; the unchanged machine code and reversing outer timing differences are evidence of measurement noise, not a demonstrated source regression.

## Validation

- 21 outer tests passed, including ordinary/zerocheck parallel-versus-serial parity and two concurrent proofs below and at the first-fold parallel threshold.
- 20 outer tests passed with the parallel feature disabled.
- 246 Spartan tests passed on the earlier candidate build; two existing tests remain ignored. Native production code was untouched throughout, and the final generic-code rollback was followed by the outer tests above.
- The supplemental profiler capture validates as 88 runs and 352 spans. `report/intervals.html` shows actual driver-level first-message, first-fold, prefix-message, prefix-fold, and tail intervals. It is a separate instrumented experiment, not the primary latency sample set. Interval totals use unions; P10–P90 are sample deciles, not confidence intervals.

## Reproduction

Run the retained public-API benchmark without allocator instrumentation:

```sh
RAYON_NUM_THREADS=10 OUTER_SHAPES='12 15 17 19' \
  OUTER_INPUTS='u32 u64 u128 field' \
  OUTER_PROTOCOLS='ordinary zero skip-1 skip-2 skip-3 skip-4' OUTER_REPS=21 \
  cargo bench --offline --bench outer_optimizations
```

Use `--features bench-peak-memory` only in a separate memory pass. The benchmark verifies every proof after timing it.

To reproduce the private candidates, create an isolated checkout of `ef7a8935`, apply `candidates.patch` there, and build `outer_optimizations` with `--features bench-internals`. Set `OUTER_VARIANTS` to space-separated choices: `baseline`, `fuse`, `register-fuse`, `first-par`, `prefix-msg`, `prefix-fold`, `prefix`, `scratch`, or combinations joined with `+`. The patch preserves the final experimental harness, including optional trace and allocator support. The initial interleaved confirmation preceded those optional hooks; the focused fusion, memory, and trace captures use the archived version. Set `OUTER_CHECK_WORKSPACE=1` to run the private scratch checks, or `OUTER_TRACE=1` for phase timestamps.

For the native reference, use the same toolchain/profile and `F2Z_BENCH_SHAPES='15 17 19' F2Z_BENCH_REPS=21 F2Z_BENCH_PASS=latency RAYON_NUM_THREADS=10`, plus `PERFETTO_TRACE_PROCESSOR` pointing to the trace processor, with `cargo bench --bench u32_mul_outer_skip --features span-metrics`.

Raw samples are retained as compressed `.log.gz` files alongside analysis scripts and experimental patches. Warmups are excluded from the CSV summaries. Regenerate the final tables with:

```sh
python3 experiments/outer-optimizations-20260915/summarize_source.py
SOURCE_PREFIX=source-serial SOURCE_OUTPUT=final-serial.csv \
  python3 experiments/outer-optimizations-20260915/summarize_source.py
```

For an original-source comparison, build the same standalone benchmark in an isolated checkout of `ef7a8935`, adding its manifest entry there. Ensure Cargo actually recompiles that checkout's library (especially if sharing a target directory), and copy each executable before building the other source. The final binary hashes and environment are recorded in `metadata.json`. Run baseline/final sequentially, reverse process order for the second session, and avoid builds, allocator counters, or instrumented tracing during latency collection.
