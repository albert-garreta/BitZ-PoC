# GF128 and complete NTT regression gate

The current optimization phase is isolated ARM/Apple Silicon and portable
candidate benchmarking, with no production integration or x86 kernel work.
See [baseline comparisons and source locations](BASELINES.md) for the concrete
implementations, benchmark call sites, matching contracts and coverage gaps.

The latest [regression repair results](REPAIR_RESULTS.md) pass the frozen gates
at one and ten threads. See [repair notes and replay commands](REPAIR_NOTES.md)
for the implementation changes, explicit baseline retentions, and frozen archive.

The new [optimization candidates](OPTIMIZATION.md) run with
`run.py --suite optimization`. That suite extracts the private production
baselines locally and benchmarks 34 operation families. Its scope and
selections are independent of the historical GF/NTT campaign below. Read the
[confirmed results and regression decisions](OPTIMIZATION_RESULTS.md).

The arithmetic-only F2Z/circuit/Flock campaign is documented in [ARITHMETIC.md](ARITHMETIC.md); run it with `run.py --suite arithmetic`.

The focused two-limb MAC and bounded-product optimization is documented in
[INTEGER_FOCUS.md](INTEGER_FOCUS.md); run it with `run.py --suite integer-focus`.


This experiment measures the proposed shared arithmetic against vendored Flock.
The current campaign is **ARM only**, with a **1% maximum slowdown**. Production
sources and dispatch are unchanged. Matrix layouts and full-prover benchmarks
are outside this campaign; `matrix.rs` and older reports remain historical work.

Run from the repository root:

```sh
python3 experiments/field-regressions/run.py \
  --out experiments/field-regressions/results/my-arm-run
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/my-arm-run
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

The output directory must be new. Dependencies must already be cached because
builds run offline. `--target-dir` can reuse an experiment-owned build cache.
`--flags` applies identical Rust flags to every implementation; the default is
`-C target-cpu=native`. `--threads` defaults to one. The runner clears inherited
Rust profile overrides and NTT experiment switches. Avoid unrelated compilation
or heavy work on the host during measurements.

Sources, the required-case manifest, candidate selections, compiler settings,
Cargo lockfile, and executable are frozen before confirmation. A temporary Flock
copy changes only its scalar `Mul` implementation to call the candidate helper
returning the shared field type. Its complete NTT sources are checked byte for
byte against the baseline. Existing fusion, cache blocking, and specialized
butterfly kernels are preserved. This tests the intended scalar substitution
inside the existing transform; it does not substitute every specialized kernel.

The runner builds once before timing. Confirmation uses five fresh processes
with 32 paired samples per case and shuffled variant execution order. Each
process has a different reproducible input/order seed. Calibration uses only
the baseline; every variant gets the same iteration count within that process.
Inconclusive **selected** cases get exactly one further round of five fresh
processes with 64 samples, using the same executable and selection. The retry
replaces the initial decision for those cases regardless of whether it improves
or worsens. Other cases retain their original decisions. There is no repeated
sampling until a desired result appears.

`--cases family/size,...`, smaller `--runs`/`--samples`, and `--no-retry` are useful
for development. They cannot make missing manifest coverage pass the gate.

## Required coverage

`required_cases.json` is the explicit coverage and selection policy. The runner
and gate import the same 0.01 default. A deliberate override is recorded, but
such a run must not be described as passing the agreed 1% policy unless the gate
also passes at its default.

| Operation | Sizes | Frozen selected candidate | Retained comparisons |
|---|---|---|---|
| Independent GF128 products | 16, 1,024, 65,536, 1,048,576 | Scalar lanes preserving Flock's ARM multiplication schedule | Current shared multiplication, unroll 2/4/8, schoolbook, Karatsuba, Barrett |
| Multiplication dependency chains | Same four sizes | Scalar lanes | Current shared and alternative multiplication kernels |
| Batch MAC / dot product | Same four sizes | One deferred-reduction accumulator | Eager shared/Flock, 2/4/8/16 accumulators, two-element SIMD |
| Squaring | Same four sizes | Actual shared implementation | Flock |
| Inversion | 1, 16, 1,024 | Actual shared implementation | Flock |
| Fixed-scalar products and butterflies | Same four sizes × zero/half/full public constants | Scalar lanes | Actual shared, prepared and specialized fixed multiplication |
| Fixed-multiplier preparation | Zero/half/full constants | Diagnostic | Existing prepared formula, measured separately |
| Complete NTT | log8/1 lane, log8/32, log12/8, log15/32, log16/32, log17/32, log18/32 | Preserved Flock schedule with candidate scalar multiplication | Generic, twiddle-aware and prepared replacement loops; reset-only diagnostic |

The last three NTT data arrays are 32, 64, and 128 MiB respectively. All transform
variants include the same reset copy. `reset_only` isolates that cost and cannot
be a replacement. Twiddle/table construction is outside transform timing;
its baseline and candidate observations are recorded separately in process logs.
These NTT setup observations are single timings per process, not a gated setup
latency distribution. Fixed-multiplier preparation has its own paired samples.

## Correctness, memory, and backend evidence

Before timing, arithmetic is checked against an independent bit-serial GF128
oracle: every pair of polynomial basis elements, 4,096 random products, zero,
one, maximum limbs, squaring, and encoding round trips. Inversion checks include
boundary values and the independent identity `a * inverse(a) == 1`. Flock returns
zero for inverse zero; the shared API returns `None`. Both conventions are
preserved. Dot and unrolled-product tests cover empty, odd, and incomplete tails.
Every timed arithmetic candidate checks its output, and every NTT output is
compared with Flock's scalar transform reference.

Allocation audits execute warmed kernels 64 times outside timing and record
the maximum allocation calls and requested bytes per invocation. They include
worker-thread allocations. Historical runs used three passes; the repair notes
explain the correction for periodic Rayon queue allocations.
Pools, inputs, outputs, and prepared constants are
created before the timed region. No candidate may add hot-path allocations or
allocated bytes relative to its paired baseline. These checks measure observed
allocation behavior for these workloads, not every possible execution.

Logs record enabled instruction features, the chosen scalar backend, and actual
caller, baseline all-core, and candidate all-core pool sizes. With the default
one-thread configuration both all-core pools also contain one thread. Large
transform cases exercise size-dependent scheduling but do not establish the
performance of a larger-pool handoff. ARM evidence has no implication for x86
kernel selection; x86 is deliberately outside the current required manifest.

CSV `bytes` is the logical payload of one candidate's arrays; it is not total
process RSS. NTT payload includes input and output, so it is twice the transform
data size. Multiple candidates' buffers coexist in the harness. Allocation byte
counts describe allocations within a warmed call, separately from payload.

## Gate and reports

For every selected case, correctness must pass, there must be no extra hot-path
allocations, and all these conditions must hold independently:

- Upper 95% confidence bound of the median batch-time ratio is at most 1.01.
- Upper 95% confidence bound of the P95 batch-time ratio is at most 1.01.
- Every confirmation-process median ratio is at most 1.01.

Ratios are candidate time divided by baseline time. P95 is a ratio of latency
quantiles, not a percentile of elementwise ratios. The reported estimands are
the median process-specific median and P95 ratios, with pointwise hierarchical
paired bootstrap intervals. Resampling preserves the pairing within processes
and resamples process identities. Timings are per-call averages inside timed
batches; these are not individual-call tail latencies or simultaneous confidence
bounds over all cases.

`pass` means the measured case meets those conditions. `regression` means a
confidence interval lies above the limit, correctness fails, or allocations
increase. `inconclusive` covers uncertain intervals or inconsistent process
medians. `unmeasured` means required confirmation evidence is missing; diagnostics
are explicitly marked and cannot be selected. All statuses except `pass` block
that replacement. Faster cases never compensate for slower cases elsewhere.
Known slow controls remain in the report without being selected for replacement.

Each report contains frozen metadata and manifest, build log, Cargo lockfile,
`initial/` and optional `retry/` raw CSV/logs, and a final `summary.json` and
`measurements.md`. The summary includes P10/P50/P90/P95/P99 batch latencies,
median/P95 ratio intervals, all process medians, payload and allocation counts.
Recorded hashes bind the report to its raw artifacts. Missing sizes, operations,
controls, process data, or required architecture evidence block the gate. Old
reports use a different policy and are not accepted by this gate.

Passing applies only to the measured host, build, input families, and threading.
Core placement, temperature, and unrelated system load are uncontrolled. Passing
these benchmarks does not prove constant-time behavior or full-prover performance.
A blocked selected case keeps the existing implementation. No production choice
is changed automatically by this experiment.
